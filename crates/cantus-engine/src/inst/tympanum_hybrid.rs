//! Tympanum's hybrid voice, the engine's side (round 40, `drums-hyb`; ADR 0124's third
//! amendment): each modelled kit's residual (`cantus_dsp::tympanum_residual`), read from
//! its content pack off the audio thread the first time a snapshot asks for it, kept for
//! the session, and handed to the kit's player in `configure` — lock-free, an `Arc` clone.
//!
//! A pack that is not installed yet is simply not there: the kit plays generated (never
//! silence) and each later snapshot looks again, so a download that finishes is heard at
//! the next edit — or at once when the kit's view is open (it reads the pack and asks
//! for a snapshot).

use super::tympanum::cache::Modelled;
use cantus_core::inst::tympanum::voice::{hybrid_pack, Voice, RESIDUAL_FILE};
use cantus_core::inst::tympanum::KitRef;
use cantus_core::NativeSpec;
use cantus_dsp::tympanum_residual::{model_hash, Player, Residual};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// What reads a pack's file: the app's (`<content>/<pack>/<file>`), or a test's.
pub type Source = dyn Fn(&str, &str) -> Result<Vec<u8>, String> + Send + Sync;

static SOURCE: RwLock<Option<Arc<Source>>> = RwLock::new(None);
static LOADED: [OnceLock<(String, Arc<Residual>)>; 8] = [const { OnceLock::new() }; 8];
static LOCK: Mutex<usize> = Mutex::new(0);
/// Packs found unreadable this session (a broken file is not read again each snapshot).
static REFUSED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Install the reader of packs' files (at start-up, `tympanum_load::install`; a test).
pub fn set_source(s: Arc<Source>) {
    if let Ok(mut g) = SOURCE.write() {
        *g = Some(s);
    }
    if let Ok(mut r) = REFUSED.lock() {
        r.clear();
    }
}

/// The kit's residual, if loaded (lock-free).
pub fn residual(kit: &str) -> Option<Arc<Residual>> {
    LOADED
        .iter()
        .filter_map(OnceLock::get)
        .find(|(k, _)| k == kit)
        .map(|(_, r)| r.clone())
}

/// Why the kit's pack could not be read, if it could not.
pub fn refused(kit: &str) -> Option<String> {
    let pack = hybrid_pack(kit)?;
    REFUSED
        .lock()
        .ok()?
        .iter()
        .find(|(p, _)| p == pack)
        .map(|(_, e)| e.clone())
}

/// Read the kit's residual from its pack, once (off the audio thread). `None` while the
/// pack is not installed (asked again next time) or when it is unreadable (not again).
pub fn warm(kit: &str) -> Option<Arc<Residual>> {
    if let Some(r) = residual(kit) {
        return Some(r);
    }
    let pack = hybrid_pack(kit)?;
    if REFUSED.lock().ok()?.iter().any(|(p, _)| p == pack) {
        return None;
    }
    let source = SOURCE.read().ok()?.clone()?;
    // Not there (not yet downloaded): quietly the model alone.
    let bytes = source(pack, RESIDUAL_FILE).ok()?;
    let r = match Residual::decode(&bytes) {
        Ok(r) => r,
        Err(e) => {
            log::warn!("tympanum: the hybrid pack {pack} could not be read: {e}");
            if let Ok(mut f) = REFUSED.lock() {
                f.push((pack.to_string(), e));
            }
            return None;
        }
    };
    if r.model != model_hash() {
        log::info!(
            "tympanum: {pack} was fitted on other tables than this build's (refit with tools/tympanum-fit/refit_hybrid.sh); it plays"
        );
    }
    let r = Arc::new(r);
    let mut n = LOCK.lock().ok()?;
    if let Some(r) = residual(kit) {
        return Some(r);
    }
    LOADED.get(*n)?.set((kit.to_string(), r.clone())).ok()?;
    *n += 1;
    Some(r)
}

/// Make ready the residuals a snapshot's hybrid kits play (each snapshot, off the audio
/// thread).
pub fn prepare(snapshot: &cantus_core::Snapshot) {
    for t in &snapshot.tracks {
        let Some(n) = t.instrument.native() else {
            continue;
        };
        if n.kind != cantus_core::InstrumentKind::AcousticDrums || Voice::of(n) != Voice::Hybrid {
            continue;
        }
        if let KitRef::Modelled(id) = KitRef::of(n) {
            warm(&id);
        }
    }
}

/// Give a modelled kit's player its residual — the kit's when its voice is hybrid and the
/// pack is loaded, else none (the model alone, to the bit) — its keys' pieces and the
/// kit's seed. Allocation-free.
pub fn configure(player: &mut Player, spec: &NativeSpec, kit: &str, m: &Modelled) {
    let data = if Voice::of(spec) == Voice::Hybrid {
        residual(kit)
    } else {
        None
    };
    let mut keys = [(0u8, 0u8); 128];
    let mut n = 0;
    for (key, piece, _) in &m.pads {
        if n < keys.len() {
            keys[n] = (*key, (*piece).min(255) as u8);
            n += 1;
        }
    }
    player.set(data, &keys[..n], spec.get("seed") as u32);
}

#[cfg(test)]
#[path = "tympanum_hybrid_tests.rs"]
mod tests;
