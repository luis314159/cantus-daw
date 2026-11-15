//! What Tympanum's unit needs ready before a snapshot reaches it (ADR 0124): the kits,
//! built off the audio thread and kept for the session so the unit only looks them up —
//! lock-free, an `Arc` clone — in `configure`.
//!
//! - **Modelled kits**: the definition, each articulation's Pulsus pad, and each mic's
//!   reference level, from `cantus_core::inst::tympanum_kit::modelled_kit`.
//! - **Recorded kits**: read by the app's loader (`cantus_ui::tympanum_load`, installed
//!   here as the [`set_resolver`]) into `cantus_core`'s registry, then converted into the
//!   engine's [`RecKit`].
//! - **The files a kit plays** ([`wants`]): what `SampleCache::attach` loads (or streams)
//!   for a Tympanum track — only the mics that are on and the bleed cells that are on.

use cantus_core::inst::tympanum::{KitRef, KIT_KEY};
use cantus_core::inst::tympanum_kit::{self as kitdef, KitDef, RecordedKit};
use cantus_core::inst_out::{output_key, OutputField};
use cantus_core::NativeSpec;
use cantus_dsp::pulsus::PadSettings;
use cantus_dsp::tympanum::rec::{RecArt, RecKit, RecLayer, RecMicTake, RecTake};
use cantus_dsp::tympanum::MAX_MICS;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

/// A modelled kit, ready.
#[derive(Debug)]
pub struct Modelled {
    pub def: KitDef,
    /// Each articulation's pad: its key, its piece, and the pad at the kit's own values.
    pub pads: Vec<(u8, u16, PadSettings)>,
    /// Per mic: the direct gain of its own piece (or the kit's centre) at its default
    /// place, what its level is relative to.
    pub norm: Vec<f32>,
}

const KEPT: usize = 64;
static MODELLED: [OnceLock<(&'static str, Arc<Modelled>)>; 8] = [const { OnceLock::new() }; 8];
static MODELLED_LOCK: Mutex<usize> = Mutex::new(0);
static RECORDED: [OnceLock<(usize, Arc<RecKit>)>; KEPT] = [const { OnceLock::new() }; KEPT];
static RECORDED_N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static RECORDED_LOCK: Mutex<usize> = Mutex::new(0);

/// The modelled kit `id`, if ready (lock-free).
pub fn modelled(id: &str) -> Option<Arc<Modelled>> {
    MODELLED
        .iter()
        .filter_map(OnceLock::get)
        .find(|(k, _)| *k == id)
        .map(|(_, m)| m.clone())
}

/// Make the modelled kit `id` ready (off the audio thread).
pub fn warm_modelled(id: &str) -> Option<Arc<Modelled>> {
    if let Some(m) = modelled(id) {
        return Some(m);
    }
    let known = kitdef::modelled_kits().iter().find(|k| k.id == id)?;
    let def = kitdef::modelled_kit(id)?;
    let mut pads = Vec::new();
    for p in &def.pieces {
        for a in &p.articulations {
            let pad = kitdef::piece_pad(p, a);
            let mut s = crate::pulsus_kit::settings(&pad, p.index as u8);
            if kitdef::piece_acoustic(p, a) {
                // Model 4: the acoustic drum or cymbal (Tympanum's own; Pulsus never).
                s.model = 4;
            }
            // Each pad its own id (fix-e, ADR 0124 amendment): its place among the kit's
            // articulations, 1-based — the identity Pulsus follows a pad by, limits its
            // polyphony by, chokes its note-off by and seeds each hit with.
            s.id = pads.len() as u16 + 1;
            if let Some(m) = kitdef::piece_measured(p, a) {
                let set = kitdef::measured_set(id);
                // Model 5: measured off a recorded kit — its tables carry the level, the
                // tuning and the decay; the piece's keys move them from there.
                s.model = 5;
                s.measured = (m.table + set)
                    | if m.foot {
                        cantus_dsp::pulsus::MEASURED_FOOT
                    } else {
                        0
                    };
                s.tune = m.tune;
                s.decay = m.decay;
                s.open_decay = 1.0;
                s.gain = 1.0;
                // The stick's burst and where it lands as measured (a kick's beater key and
                // a piece's `pos` move them).
                s.click = 1.0;
                s.position = 0.0;
                if m.table == 3 || m.table == 5 {
                    // A ride's zone (its bow or its bell) or a snare's (its head, the rim,
                    // the stick laid across).
                    s.openness = m.zone;
                }
            }
            pads.push((a.key, p.index, s));
        }
    }
    let norm = def.mics.iter().map(|m| super::mic_norm(&def, m)).collect();
    let ready = Arc::new(Modelled { def, pads, norm });
    let mut n = MODELLED_LOCK.lock().ok()?;
    if let Some(m) = modelled(id) {
        return Some(m);
    }
    let slot = MODELLED.get(*n)?;
    slot.set((known.id, ready.clone())).ok()?;
    *n += 1;
    Some(ready)
}

/// The engine's form of a recorded kit the registry holds (lock-free).
pub fn rec_kit(kit: &Arc<RecordedKit>) -> Option<Arc<RecKit>> {
    let key = Arc::as_ptr(kit) as usize;
    let n = RECORDED_N
        .load(std::sync::atomic::Ordering::Acquire)
        .min(KEPT);
    RECORDED[..n]
        .iter()
        .filter_map(OnceLock::get)
        .find(|(k, _)| *k == key)
        .map(|(_, r)| r.clone())
}

/// Convert a registered kit for the engine (off the audio thread), once.
pub fn warm_recorded(kit: &Arc<RecordedKit>) -> Option<Arc<RecKit>> {
    if let Some(r) = rec_kit(kit) {
        return Some(r);
    }
    let rec = Arc::new(convert(kit));
    let mut n = RECORDED_LOCK.lock().ok()?;
    if let Some(r) = rec_kit(kit) {
        return Some(r);
    }
    RECORDED
        .get(*n)?
        .set((Arc::as_ptr(kit) as usize, rec.clone()))
        .ok()?;
    *n += 1;
    RECORDED_N.store(*n, std::sync::atomic::Ordering::Release);
    Some(rec)
}

/// The recorded kit as the engine plays it: every gain baked per velocity, pans as gains.
pub fn convert(kit: &RecordedKit) -> RecKit {
    let arts = kit
        .hits
        .arts
        .iter()
        .map(|a| RecArt {
            piece: a.piece,
            key: a.key,
            openness: a.openness,
            layers: a
                .layers
                .iter()
                .map(|l| {
                    let mut gain = [0.0f32; 128];
                    for (v, g) in gain.iter_mut().enumerate() {
                        *g = l.gain(v as u8);
                    }
                    RecLayer {
                        lo: l.lo,
                        hi: l.hi,
                        gain,
                        takes: l
                            .takes
                            .iter()
                            .map(|t| RecTake {
                                mics: t
                                    .mics
                                    .iter()
                                    .map(|m| {
                                        let def = kit.def.mics.get(m.mic as usize);
                                        // The mic's own level and pan in the kit's mix, then the
                                        // take's (a mono file: constant power).
                                        let lvl = def.map_or(1.0, |d| {
                                            cantus_dsp::math::db_to_gain(d.level_db)
                                        });
                                        RecMicTake {
                                            mic: m.mic,
                                            file: m.file,
                                            gain: m.gain * lvl,
                                            pan: m.pan,
                                            offset: m.offset,
                                            cents: m.cents,
                                            highpass: m.highpass,
                                        }
                                    })
                                    .collect(),
                            })
                            .collect(),
                    }
                })
                .collect(),
            chokes: a.chokes.clone(),
            choke_s: a.choke_ms / 1000.0,
            polyphony: a.polyphony,
        })
        .collect();
    let pedal = kit
        .hits
        .arts
        .iter()
        .position(|a| a.key == 44 && a.openness.is_some())
        .map(|i| i as u16);
    RecKit {
        mics: kit.def.mics.len().min(MAX_MICS),
        arts,
        pedal,
    }
}

/// What reads a pack's kit: the app's loader, given a kit reference.
pub type Resolver = dyn Fn(&KitRef) -> Result<RecordedKit, String> + Send + Sync;

static RESOLVER: RwLock<Option<Arc<Resolver>>> = RwLock::new(None);
/// Kits the resolver could not read this session, and why (asked once each).
static REFUSED: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Install the app's kit loader (at start-up, `CantusApp::assemble`).
pub fn set_resolver(r: Arc<Resolver>) {
    if let Ok(mut g) = RESOLVER.write() {
        *g = Some(r);
    }
    if let Ok(mut f) = REFUSED.lock() {
        f.clear();
    }
}

/// The content library's generation the refusals were made under (+1; 0: none seen).
static GENERATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// fix-e — the installed packs changed (`cantus_content::Manager::generation`: an install
/// finished, a pack was removed, a user pack added): a kit refused before may be there now,
/// so every refusal is forgotten and asked again with the next snapshot. True when one
/// was (the app then sends the snapshot again, so the kit is read and its files load).
pub fn content_generation(generation: u64) -> bool {
    let now = generation.wrapping_add(1);
    let was = GENERATION.swap(now, std::sync::atomic::Ordering::AcqRel);
    if was == now {
        return false;
    }
    // Only a kit refused before needs the snapshot again.
    REFUSED.lock().is_ok_and(|mut f| {
        let any = !f.is_empty();
        f.clear();
        any
    })
}

/// Why a kit could not be read, if it could not.
pub fn refused(text: &str) -> Option<String> {
    REFUSED
        .lock()
        .ok()?
        .iter()
        .find(|(t, _)| t == text)
        .map(|(_, e)| e.clone())
}

/// The recorded kit `kit` names, read through the resolver the first time it is asked
/// for (off the audio thread), then kept.
pub fn recorded(kit: &KitRef) -> Option<Arc<RecordedKit>> {
    let text = kit.text();
    if let Some(k) = kitdef::recorded(&text) {
        warm_recorded(&k);
        return Some(k);
    }
    if REFUSED.lock().ok()?.iter().any(|(t, _)| *t == text) {
        return None;
    }
    let r = RESOLVER.read().ok()?.clone()?;
    match r(kit) {
        Ok(k) => {
            for line in &k.log {
                log::info!("tympanum: {text}: {line}");
            }
            let k = kitdef::register(k)?;
            warm_recorded(&k);
            Some(k)
        }
        Err(e) => {
            log::warn!("tympanum: {text} could not be read: {e}");
            if let Ok(mut f) = REFUSED.lock() {
                f.push((text, e));
            }
            None
        }
    }
}

/// The text key's kit (the default kit when it names none), without allocating when it
/// is the stored text.
pub fn kit_text(spec: &NativeSpec) -> &str {
    spec.text(KIT_KEY).unwrap_or("model:studio")
}

/// Whether output `m` is on (`o{m}.on`), read from the spec's stored keys.
pub fn mic_on(spec: &NativeSpec, m: usize) -> bool {
    !spec
        .params
        .iter()
        .any(|(k, v)| output_key(k) == Some((m, OutputField::On)) && *v < 0.5)
}

/// The files a Tympanum's kit plays, as `(asset key, path)`: `t{file}` for every file of
/// a mic that is on, in a cell that is on. A modelled kit plays none.
pub fn wants(spec: &NativeSpec) -> Vec<(String, String)> {
    let Some(KitRef::Pack { .. }) = KitRef::parse(kit_text(spec)) else {
        return Vec::new();
    };
    let Some(kit) = recorded(&KitRef::of(spec)) else {
        return Vec::new();
    };
    let mics = kit.def.mics.len().min(MAX_MICS);
    let on: Vec<bool> = (0..mics).map(|m| mic_on(spec, m)).collect();
    let mut cell_off = vec![[false; MAX_MICS]; cantus_dsp::tympanum::rec::MAX_PIECES];
    for (k, v) in &spec.params {
        if let Some((p, m, "on")) = cantus_core::inst::tympanum::bleed_key(k) {
            if *v < 0.5 && p < cell_off.len() {
                cell_off[p][m] = true;
            }
        }
    }
    let mut want = vec![false; kit.hits.files.len()];
    for a in &kit.hits.arts {
        let p = (a.piece as usize).min(cell_off.len() - 1);
        for t in a.layers.iter().flat_map(|l| &l.takes) {
            for mt in &t.mics {
                let m = mt.mic as usize;
                if m < mics && on[m] && !cell_off[p][m] {
                    if let Some(w) = want.get_mut(mt.file as usize) {
                        *w = true;
                    }
                }
            }
        }
    }
    kit.hits
        .files
        .iter()
        .enumerate()
        .filter(|(i, _)| want[*i])
        .map(|(i, f)| (format!("t{i}"), f.clone()))
        .collect()
}

/// Per track: the kit it last had wholly loaded, and that kit's files — what it keeps
/// playing while a new kit's files load (ADR 0124 §2: never a silent kit change).
static READY: Mutex<Vec<Loaded>> = Mutex::new(Vec::new());

/// A track, the kit it had wholly loaded, and that kit's files (asset key, path).
type Loaded = (u64, String, Vec<(String, String)>);

/// The files a Tympanum on `track` needs held: its kit's, and while that kit is not the
/// one it last had loaded, the last one's too (as `p{i}`), so they are not let go.
pub fn wants_all(track: u64, spec: &NativeSpec) -> Vec<(String, String)> {
    let mut out = wants(spec);
    let text = kit_text(spec);
    if let Ok(r) = READY.lock() {
        if let Some((_, t, files)) = r.iter().find(|(id, _, _)| *id == track) {
            if t != text {
                out.extend(
                    files
                        .iter()
                        .map(|(k, p)| (format!("p{}", k.trim_start_matches('t')), p.clone())),
                );
            }
        }
    }
    out
}

/// The kit a Tympanum on `track` plays this snapshot, as its assets: its own kit when its
/// files are all in (`ready`), or offline; else the kit it had, its files as `t{i}` and a
/// marker `k:<that kit's text>` (no file) the unit reads.
pub fn attach(
    track: u64,
    spec: &NativeSpec,
    offline: bool,
    ready: &dyn Fn(&str) -> bool,
) -> Vec<(String, String)> {
    let new = wants(spec);
    let text = kit_text(spec);
    let Ok(mut r) = READY.lock() else {
        return new;
    };
    let all_in = new.iter().all(|(_, p)| ready(p));
    if all_in {
        if !offline {
            match r.iter_mut().find(|(id, _, _)| *id == track) {
                Some(e) => {
                    e.1 = text.to_string();
                    e.2.clone_from(&new);
                }
                None => r.push((track, text.to_string(), new.clone())),
            }
        }
        return new;
    }
    if offline {
        return new;
    }
    match r.iter().find(|(id, _, _)| *id == track) {
        Some((_, t, files)) if t != text => {
            let mut out = files.clone();
            out.push((format!("k:{t}"), String::new()));
            out
        }
        _ => new,
    }
}

/// Make ready what a snapshot's Tympanums play (each snapshot, off the audio thread).
pub fn prepare(snapshot: &cantus_core::Snapshot) {
    for t in &snapshot.tracks {
        let Some(n) = t.instrument.native() else {
            continue;
        };
        if n.kind != cantus_core::InstrumentKind::AcousticDrums {
            continue;
        }
        match KitRef::of(n) {
            KitRef::Modelled(id) => {
                warm_modelled(&id);
            }
            kit @ KitRef::Pack { .. } => {
                recorded(&kit);
            }
        }
    }
    // The default kit is always ready (a new Tympanum plays at once).
    warm_modelled("studio");
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::inst::tympanum_kit::{ArtHits, KitHits, MicTake, RecordedKit, Take, VelLayer};
    use cantus_core::InstrumentKind;

    fn kit(pack: &str, files: &[&str]) -> RecordedKit {
        let mut def = kitdef::modelled_kit("studio").unwrap();
        def.kit = KitRef::Pack {
            pack: pack.into(),
            kit: "kit".into(),
        };
        def.mics.truncate(1);
        RecordedKit {
            def,
            hits: KitHits {
                files: files.iter().map(|f| f.to_string()).collect(),
                arts: vec![ArtHits {
                    piece: 0,
                    articulation: 0,
                    key: 36,
                    openness: None,
                    layers: vec![VelLayer {
                        lo: 1,
                        hi: 127,
                        curve: Vec::new(),
                        power: None,
                        takes: (0..files.len())
                            .map(|i| Take {
                                mics: vec![MicTake {
                                    mic: 0,
                                    file: i as u32,
                                    channel: None,
                                    gain: 1.0,
                                    pan: 0.0,
                                    offset: 0,
                                    cents: 0.0,
                                    highpass: 0.0,
                                }],
                            })
                            .collect(),
                    }],
                    chokes: Vec::new(),
                    choke_ms: 70.0,
                    polyphony: 0,
                }],
            },
            log: Vec::new(),
        }
    }

    fn spec(text: &str) -> NativeSpec {
        let mut s = NativeSpec::new(InstrumentKind::AcousticDrums);
        s.set_text(KIT_KEY, Some(text.into())).unwrap();
        s
    }

    #[test]
    fn the_old_kit_plays_until_the_new_kits_files_are_in_and_offline_never_waits() {
        kitdef::register(kit("cache-a", &["F:/a/1.wav", "F:/a/2.wav"])).unwrap();
        kitdef::register(kit("cache-b", &["F:/b/1.wav"])).unwrap();
        let track = 0xA11CE;
        let (a, b) = (spec("pack:cache-a/kit"), spec("pack:cache-b/kit"));
        // Kit A, all in: it plays, and it is the track's loaded kit.
        let got = attach(track, &a, false, &|_| true);
        assert_eq!(got.len(), 2);
        // Kit B asked for while its file loads: A's files stay wanted, A plays (marked).
        let all = wants_all(track, &b);
        assert!(all.iter().any(|(k, p)| k == "t0" && p == "F:/b/1.wav"));
        assert!(all.iter().any(|(k, p)| k == "p0" && p == "F:/a/1.wav"));
        let now = attach(track, &b, false, &|p| p.starts_with("F:/a"));
        assert!(
            now.iter().any(|(k, _)| k == "k:pack:cache-a/kit"),
            "{now:?}"
        );
        assert!(now.iter().any(|(_, p)| p == "F:/a/2.wav"));
        // Offline never plays the old kit: an export is the song as it is.
        let off = attach(track, &b, true, &|p| p.starts_with("F:/a"));
        assert!(off.iter().all(|(k, _)| !k.starts_with("k:")));
        // B's file in: B plays, and A is let go.
        let now = attach(track, &b, false, &|_| true);
        assert_eq!(now, vec![("t0".to_string(), "F:/b/1.wav".to_string())]);
        assert!(wants_all(track, &b)
            .iter()
            .all(|(k, _)| !k.starts_with('p')));
    }
}
