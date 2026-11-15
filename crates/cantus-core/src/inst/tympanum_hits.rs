//! A recorded kit's strikes (round 40 phase 3, ADR 0124): which file every microphone of
//! every strike plays, how its level follows velocity, which strikes choke which — read
//! from a pack (an SFZ multi-mic kit, a DrumGizmo kit) by the app's loader
//! (`cantus_ui::tympanum_load`), never written into a song. And the registry the loader
//! fills, which the engine reads lock-free on the audio thread.
//!
//! **One strike, every mic.** A [`Take`] is one recorded strike as every microphone heard
//! it: each [`MicTake`] names that mic's file (and its channel, for a DrumGizmo hit that
//! holds every mic in one file). The engine draws **one** take per hit
//! (`cantus_dsp::tympanum::hit_draw`) and plays every mic of it, so a close mic and an
//! overhead never play different strikes.

use super::KitDef;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// One microphone's part of one strike.
#[derive(Clone, Debug, PartialEq)]
pub struct MicTake {
    /// Index into [`KitDef::mics`].
    pub mic: u8,
    /// Index into [`KitHits::files`].
    pub file: u32,
    /// The file's channel that is this mic (a DrumGizmo hit holds every mic); `None`: the
    /// file's own channels (a mono or stereo file per mic, as SFZ kits ship).
    pub channel: Option<u8>,
    /// Linear gain: the region's own level (`amplitude`, `volume`, the library's mic and
    /// piece faders at their `set_ccN`).
    pub gain: f32,
    /// −1 … 1, the constant-power law (the region's `pan`), for a mono file.
    pub pan: f32,
    /// Frames into the file the take starts (`offset`).
    pub offset: u32,
    /// Cents (`tune`, `transpose`, a tuning controller at its default).
    pub cents: f32,
    /// A two-pole high-pass (`fil_type=hpf_2p`), Hz; 0 none.
    pub highpass: f32,
}

/// One recorded strike, as every microphone heard it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Take {
    pub mics: Vec<MicTake>,
}

/// A velocity layer of an articulation: its range, how its level follows velocity inside
/// it, and its takes (round robins).
#[derive(Clone, Debug, PartialEq)]
pub struct VelLayer {
    pub lo: u8,
    pub hi: u8,
    /// SFZ's `amp_velcurve_N` points (velocity, gain), sorted by velocity, between the
    /// implied (0, 0) and (127, 1); empty: SFZ's default curve, `(v / 127)²`.
    pub curve: Vec<(u8, f32)>,
    /// A DrumGizmo hit's `power` (0 … 1): how hard it was struck, for kits whose layers
    /// are single strikes ordered by strength.
    pub power: Option<f32>,
    pub takes: Vec<Take>,
}

impl VelLayer {
    /// The layer's gain at `velocity` (1 … 127).
    pub fn gain(&self, velocity: u8) -> f32 {
        let v = velocity.min(127);
        if self.curve.is_empty() {
            let x = f32::from(v) / 127.0;
            return x * x;
        }
        let mut lo = (0u8, 0.0f32);
        for &(pv, pg) in self.curve.iter().chain(std::iter::once(&(127u8, 1.0f32))) {
            if v <= pv {
                if pv == lo.0 {
                    return pg;
                }
                let t = f32::from(v - lo.0) / f32::from(pv - lo.0);
                return lo.1 + t * (pg - lo.1);
            }
            lo = (pv, pg);
        }
        1.0
    }
}

/// Every recorded strike of one articulation (a key, or a hi-hat's level on the openness
/// controller).
#[derive(Clone, Debug, PartialEq)]
pub struct ArtHits {
    /// Index into [`KitDef::pieces`].
    pub piece: u16,
    /// Index into the piece's articulations.
    pub articulation: u16,
    pub key: u8,
    /// A hi-hat's recorded openness, 0 closed … 1 open: strikes of one key at several
    /// levels are crossfaded by the openness controller (CC4) at the hit and followed
    /// during it. `None`: the articulation stands alone on its key.
    pub openness: Option<f32>,
    pub layers: Vec<VelLayer>,
    /// The articulations a strike of this one silences (indices into [`KitHits::arts`]):
    /// SFZ's `group`/`off_by`, DrumGizmo's `<chokes>`.
    pub chokes: Vec<u16>,
    /// How long a choked strike fades, ms (SFZ's `ampeg_release` with `off_mode=normal`,
    /// DrumGizmo's `choketime`).
    pub choke_ms: f32,
    /// Strikes of it sounding at once, at most (SFZ's `polyphony`); 0: no limit.
    pub polyphony: u8,
}

/// A recorded kit's strikes and the files they play.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KitHits {
    /// Every file, absolute (forward slashes), in the order the takes index them.
    pub files: Vec<String>,
    pub arts: Vec<ArtHits>,
}

impl KitHits {
    /// The articulations on `key`, in order.
    pub fn on_key(&self, key: u8) -> impl Iterator<Item = (usize, &ArtHits)> {
        self.arts
            .iter()
            .enumerate()
            .filter(move |(_, a)| a.key == key)
    }

    /// The files mic `mic` plays, as indices into [`Self::files`] — what loading a mic
    /// loads, and what turning it off frees.
    pub fn files_of(&self, mic: u8) -> Vec<u32> {
        let mut out: Vec<u32> = self
            .arts
            .iter()
            .flat_map(|a| &a.layers)
            .flat_map(|l| &l.takes)
            .flat_map(|t| &t.mics)
            .filter(|m| m.mic == mic)
            .map(|m| m.file)
            .collect();
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// A recorded kit as its loader read it: what it is, its strikes, and what the reader had
/// to say (which mic rule it used, what it left out).
#[derive(Clone, Debug, PartialEq)]
pub struct RecordedKit {
    pub def: KitDef,
    pub hits: KitHits,
    pub log: Vec<String>,
}

/// Recorded kits a session keeps (a kit read again after its pack was updated is a new
/// entry; the newest wins).
pub const KITS_KEPT: usize = 64;

static KITS: [OnceLock<(String, Arc<RecordedKit>)>; KITS_KEPT] =
    [const { OnceLock::new() }; KITS_KEPT];
static KITS_N: AtomicUsize = AtomicUsize::new(0);
static KITS_NEXT: Mutex<usize> = Mutex::new(0);

/// Keep a kit the loader read, under its [`KitDef::kit`]'s text. Off the audio thread.
/// `None` when the registry is full (the kit then stays silent, and the loader says so).
pub fn register(kit: RecordedKit) -> Option<Arc<RecordedKit>> {
    let text = kit.def.kit.text();
    let kit = Arc::new(kit);
    let mut next = KITS_NEXT.lock().ok()?;
    if *next >= KITS_KEPT {
        return None;
    }
    KITS[*next].set((text, kit.clone())).ok()?;
    *next += 1;
    KITS_N.store(*next, Ordering::Release);
    Some(kit)
}

/// The newest kit registered under `text` (a [`super::KitRef`]'s text form).
/// Allocation- and lock-free: the engine asks on the audio thread.
pub fn recorded(text: &str) -> Option<Arc<RecordedKit>> {
    let n = KITS_N.load(Ordering::Acquire).min(KITS_KEPT);
    KITS[..n]
        .iter()
        .rev()
        .filter_map(OnceLock::get)
        .find(|(t, _)| t == text)
        .map(|(_, k)| k.clone())
}

/// Whether a kit is registered under `text` (no clone: cheaper on the audio thread).
pub fn is_recorded(text: &str) -> bool {
    let n = KITS_N.load(Ordering::Acquire).min(KITS_KEPT);
    KITS[..n]
        .iter()
        .filter_map(OnceLock::get)
        .any(|(t, _)| t == text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layers_gain_follows_its_curve_points_and_sfzs_default() {
        let mut l = VelLayer {
            lo: 22,
            hi: 42,
            curve: vec![(42, 1.0)],
            power: None,
            takes: Vec::new(),
        };
        // Gogodze's layers: `amp_velcurve_42=1` — linear from (0, 0) to the layer's top.
        assert_eq!(l.gain(42), 1.0);
        assert_eq!(l.gain(127), 1.0);
        assert!((l.gain(21) - 0.5).abs() < 1e-6);
        assert_eq!(l.gain(0), 0.0);
        l.curve.clear();
        assert_eq!(l.gain(127), 1.0);
        assert!((l.gain(64) - cantus_dsp::math::powi(64.0f32 / 127.0, 2)).abs() < 1e-6);
    }
}
