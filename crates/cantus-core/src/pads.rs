//! Pads and lanes: what a drum kit is made of, and how a note editor asks about it.
//!
//! A **pad** is a named, pitch-addressed drum inside a kit. A **lane** is a row in a note
//! editor. The two are the same thing for a kit and are not the same thing for a piano,
//! which is exactly why [`Lanes`] exists: the piano roll asks the instrument what a pitch
//! is called and which pitches are worth showing, instead of assuming a keyboard.
//!
//! That is what makes drum mode a property of the *kit* rather than a mode the user has
//! to remember to switch on — and it is what a sampler with named zones will want later,
//! for nothing.
//!
//! ## Pulsus II
//!
//! A kit carries a **model version** ([`PulsusParams::version`]). A kit written before
//! Pulsus II has none in its file and loads as **model 1**, which renders bit-identically
//! to the Pulsus that wrote it — including the two defects Pulsus II fixes (noise seeded
//! from a hit count that a seek resets, and a kick 52 cents sharp of F1). A kit made now
//! is model 2 ([`PULSUS_MODEL`]). Every field Pulsus II added reads from an old file as a
//! value that changes no sample, so an old kit can use the new sections without being
//! upgraded; upgrading is one command and changes only what the version says it does.
//!
//! Pads have a stable [`PadId`]: the rack, the commands and the lanes address a pad by
//! identity, so inserting a lower pad no longer renumbers the ones above it.

use crate::model::{pitch_name, Pitch, StripId};
use cantus_dsp::math;
use serde::{Deserialize, Serialize};

#[path = "pads_field.rs"]
mod field;
#[path = "pads_kit.rs"]
mod kit;
pub use field::{KitField, PadField, PadParamInfo};
pub use kit::FactoryKit;

/// The model a kit made today is. Model 1 is the Pulsus of rounds 12–16, kept so old
/// projects sound exactly as they did.
pub const PULSUS_MODEL: u8 = 2;
/// The most sample layers a pad holds; the engine preallocates this many per pad.
pub const MAX_LAYERS: usize = 16;
/// Choke groups a kit can use (0 is none).
pub const CHOKE_GROUPS: u8 = 16;

/// A pad's identity inside its kit: never reused while the kit lives, never renumbered
/// when a pad is added below it. Zero means "not yet assigned" and is fixed up on load.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct PadId(pub u16);

/// What makes a pad's sound: one of Pulsus's synthesized voices, or a sample.
///
/// The first four are Pulsus I's and keep their indices, because [`PadField::Voice`] is a
/// stepped index and old projects store it. A voice has one or more **models**
/// ([`PadVoice::models`]): the Pulsus kick, the 808 and the 909 are three models of one
/// voice, so a kit keeps its flat field table and a model switch is one undoable number.
///
/// `Sample` is a voice like the others rather than a second instrument, so a kit can mix a
/// synthesized kick with a sampled snare, and the rack, the inspector, the choke groups
/// and the mute all work the same way for both.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PadVoice {
    Kick,
    Snare,
    /// Pulsus I's hi-hat; in Pulsus II the **metal** voice: hat, 808 cymbal, 808 cowbell
    /// and a modal cymbal. The variant keeps its name so old files read.
    HiHat,
    Sample,
    /// Band-passed noise re-triggered a few times, then a room tail.
    Clap,
    /// A modal resonator bank: toms, congas, bongos, bars, woodblocks, plates, bells.
    Tom,
}

impl PadVoice {
    pub const ALL: [PadVoice; 6] = [
        PadVoice::Kick,
        PadVoice::Snare,
        PadVoice::HiHat,
        PadVoice::Sample,
        PadVoice::Clap,
        PadVoice::Tom,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            PadVoice::Kick => "pad.voice.kick",
            PadVoice::Snare => "pad.voice.snare",
            PadVoice::HiHat => "pulsus.voice.metal",
            PadVoice::Sample => "pad.voice.sample",
            PadVoice::Clap => "pulsus.voice.clap",
            PadVoice::Tom => "pulsus.voice.tom",
        }
    }

    pub fn index(self) -> usize {
        match self {
            PadVoice::Kick => 0,
            PadVoice::Snare => 1,
            PadVoice::HiHat => 2,
            PadVoice::Sample => 3,
            PadVoice::Clap => 4,
            PadVoice::Tom => 5,
        }
    }

    pub fn from_index(i: usize) -> PadVoice {
        PadVoice::ALL[i.min(PadVoice::ALL.len() - 1)]
    }

    /// The models of this voice, as label keys; index 0 is the default (for the first
    /// three voices, Pulsus I's own shape).
    pub fn models(self) -> &'static [&'static str] {
        match self {
            PadVoice::Kick => &[
                "pulsus.model.pulsus",
                "pulsus.model.kick808",
                "pulsus.model.kick909",
            ],
            PadVoice::Snare => &["pulsus.model.pulsus", "pulsus.model.modal"],
            PadVoice::HiHat => &[
                "pulsus.model.hat",
                "pulsus.model.cymbal808",
                "pulsus.model.cowbell808",
                "pulsus.model.cymbal_modal",
            ],
            PadVoice::Sample => &["pulsus.model.sampler"],
            PadVoice::Clap => &["pulsus.model.clap"],
            PadVoice::Tom => &["pulsus.model.modal"],
        }
    }
}

/// How a pad chooses among layers that share a velocity zone.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum RoundRobin {
    /// Every layer of the zone sounds (a stack).
    #[default]
    Off,
    /// In order, by the hit's ordinal on the track: survives a seek by construction.
    Cycle,
    /// Seeded from the kit's seed, the pad and the hit.
    Random,
    /// Random, never the same layer twice in a row (FPC's "avoid previous").
    NoRepeat,
}

impl RoundRobin {
    pub const ALL: [RoundRobin; 4] = [
        RoundRobin::Off,
        RoundRobin::Cycle,
        RoundRobin::Random,
        RoundRobin::NoRepeat,
    ];

    pub fn index(self) -> usize {
        RoundRobin::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> RoundRobin {
        RoundRobin::ALL[i.min(3)]
    }

    pub fn label_key(self) -> &'static str {
        match self {
            RoundRobin::Off => "pulsus.rr.off",
            RoundRobin::Cycle => "pulsus.rr.cycle",
            RoundRobin::Random => "pulsus.rr.random",
            RoundRobin::NoRepeat => "pulsus.rr.no_repeat",
        }
    }
}

/// One sample layer of a pad: a file and where in velocity it plays.
///
/// Layers with the **same** velocity range form one zone, and the pad's
/// [`RoundRobin`] picks among them; adjacent zones cross-fade over `xfade` velocity units,
/// equal-power, centred on the boundary between them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PadLayer {
    pub sample: String,
    #[serde(default = "d_vel_lo")]
    pub vel_lo: u8,
    #[serde(default = "d_vel_hi")]
    pub vel_hi: u8,
    /// Width of the cross-fade at each edge of the zone, in velocity units.
    #[serde(default)]
    pub xfade: u8,
    /// Semitones, on top of the pad's tune.
    #[serde(default)]
    pub tune: f32,
    /// dB.
    #[serde(default)]
    pub level_db: f32,
    /// −1..=1, on top of the pad's pan.
    #[serde(default)]
    pub pan: f32,
    /// Where playback starts, ms into the file.
    #[serde(default)]
    pub start_ms: f32,
    #[serde(default)]
    pub reverse: bool,
}

fn d_vel_lo() -> u8 {
    1
}
fn d_vel_hi() -> u8 {
    127
}

impl PadLayer {
    pub fn new(sample: impl Into<String>) -> PadLayer {
        PadLayer {
            sample: sample.into(),
            vel_lo: 1,
            vel_hi: 127,
            xfade: 0,
            tune: 0.0,
            level_db: 0.0,
            pan: 0.0,
            start_ms: 0.0,
            reverse: false,
        }
    }

    /// The layer's gain at a velocity (1…127): 1 inside its zone, an equal-power fade over
    /// `xfade` units centred half a unit outside each inner edge, 0 beyond.
    pub fn velocity_gain(&self, velocity: f32) -> f32 {
        let lo = self.vel_lo.min(self.vel_hi) as f32 - 0.5;
        let hi = self.vel_lo.max(self.vel_hi) as f32 + 0.5;
        let w = self.xfade as f32;
        let fade = |d: f32| -> f32 {
            // d: distance inside the edge; −w/2 → 0, +w/2 → 1.
            if w <= 0.0 {
                return if d >= 0.0 { 1.0 } else { 0.0 };
            }
            let t = (d / w + 0.5).clamp(0.0, 1.0);
            math::sin(t * std::f32::consts::FRAC_PI_2)
        };
        // The ends of the velocity scale have nothing to cross-fade with.
        let fade_in = if self.vel_lo.min(self.vel_hi) <= 1 {
            1.0
        } else {
            fade(velocity - lo)
        };
        let fade_out = if self.vel_lo.max(self.vel_hi) >= 127 {
            1.0
        } else {
            fade(hi - velocity)
        };
        fade_in * fade_out
    }
}

/// Decoded audio, shared between the thread that loaded it and the audio thread.
///
/// Plain data with no I/O: *loading* a file is the engine's job (`cantus_engine`), and
/// this is only the shape the result takes so a snapshot can carry it. The frames are an
/// `Arc<[f32]>` so a snapshot, a voice and the loader's cache all point at one buffer and
/// handing it to the audio thread is an atomic increment, never a copy.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleData {
    pub rate: u32,
    /// 1 or 2. Interleaved when 2.
    pub channels: u8,
    pub frames: std::sync::Arc<[f32]>,
    /// When the sample streams from disk (ADR 0050), the engine's stream — `frames` then
    /// holds only its head. Opaque here: `cantus-core` does no I/O and knows no engine type.
    pub stream: Option<StreamRef>,
}

/// An engine's handle on a sample streamed from disk, carried by [`SampleData`] so the
/// stream travels with the asset (and lives as long as anything holds it).
#[derive(Clone)]
pub struct StreamRef(pub std::sync::Arc<dyn std::any::Any + Send + Sync>);

impl std::fmt::Debug for StreamRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StreamRef")
    }
}

impl PartialEq for StreamRef {
    fn eq(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}

impl SampleData {
    pub fn len_frames(&self) -> usize {
        self.frames.len() / self.channels.max(1) as usize
    }
}

/// One drum in a kit.
///
/// Flat rather than a per-voice enum on purpose: every field is a plain number the command
/// bus can set and undo without knowing which shape the pad is, and
/// [`PadField::applies_to`] is what keeps the inspector from offering a hi-hat a pitch
/// envelope. A union would move that filter into `match` arms in the command path, where
/// it would be load-bearing instead of cosmetic.
///
/// Every field Pulsus II added is read from an old file as [`DrumPad::default`]'s value,
/// which is the value that changes no sample (`#[serde(default)]` on the struct).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumPad {
    /// Identity inside the kit ([`PadId`]); assigned on load when a file has none.
    pub id: PadId,
    /// The note that plays this pad. General MIDI's drum map, so a drum controller and
    /// an imported MIDI file both land where a musician expects.
    pub pitch: Pitch,
    /// The pad's name, which travels **in the kit**.
    pub name: String,
    pub voice: PadVoice,
    /// Semitones, tuning the whole voice at once.
    pub tune: f32,
    /// Main amplitude decay to −60 dB, seconds.
    pub decay: f32,
    /// Kick: how far the pitch falls, semitones (the 909's tune depth, the 808's sweep).
    pub pitch_drop: f32,
    /// Kick: how fast it falls, seconds.
    pub pitch_decay: f32,
    /// Kick: level of the click transient (the 909's attack, the 808's pulse).
    pub click: f32,
    /// Soft saturation. Zero is exactly clean. Model 1 kits saturate only the kick.
    pub drive: f32,
    /// Snare: the noise's own decay, seconds.
    pub noise_decay: f32,
    /// Where the voice's noise or partials sit, Hz.
    pub tone: f32,
    /// Snare: 0 is all body, 1 is all noise ("snappy").
    pub mix: f32,
    /// Metal: the high-pass after the band-pass, Hz.
    pub hp_cutoff: f32,
    /// The pad's own level trim. 0 dB is the pad as the kit was balanced.
    pub level_db: f32,
    /// The pad's place in the kit's stereo image, −1..=1.
    pub pan: f32,
    /// Choke group, 0 is none, 1…16.
    pub choke: u8,
    /// A muted pad keeps its notes and its place in the rack; it just does not sound.
    pub muted: bool,
    /// The file a [`PadVoice::Sample`] pad with no layers plays.
    pub sample: Option<String>,

    // ---- Pulsus II ------------------------------------------------------------------
    /// Which model of the voice ([`PadVoice::models`]).
    pub model: u8,
    /// Sample layers; on a synthesized pad they sound with the synth (a body plus a
    /// sampled click). On a sample pad they replace `sample`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<PadLayer>,
    pub round_robin: RoundRobin,
    /// Where the pad is heard: `None` is the kit's own strip. The engine's multi-out is
    /// not wired yet (see `docs/instruments/pulsus.md`, "the hook").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<StripId>,
    /// Amplitude envelope: attack and hold, ms, before the decay; and the decay's curve
    /// (0 exponential, 1 linear, −1 faster than exponential).
    pub attack_ms: f32,
    pub hold_ms: f32,
    pub curve: f32,
    /// Pitch envelope of the voices that have no sweep of their own: semitones and ms.
    pub pitch_env: f32,
    pub pitch_env_ms: f32,
    /// Filter: 0 off, 1 low-pass, 2 high-pass, 3 band-pass; cutoff Hz; resonance 0…1;
    /// envelope amount in semitones of cutoff, following the amplitude envelope.
    pub filter: u8,
    pub cutoff: f32,
    pub resonance: f32,
    pub filter_env: f32,
    /// Transient shaper, dB: the first milliseconds, and what follows.
    pub trans_attack: f32,
    pub trans_sustain: f32,
    /// 0 soft, 1 tape, 2 clip (all 2× oversampled), 3 raw (not oversampled: grit).
    pub drive_mode: u8,
    /// Start phase of a synthesized body, degrees (0 is click-free, 90 a hard click).
    pub phase: f32,
    /// Velocity: → level (%; 100 is Pulsus I's curve), → tone (%; 50 is Pulsus I's),
    /// → pitch (semitones at 127), → decay (%).
    pub vel_level: f32,
    pub vel_tone: f32,
    pub vel_pitch: f32,
    pub vel_decay: f32,
    /// How long a choked voice takes to reach −60 dB, ms.
    pub choke_fade_ms: f32,
    /// A note-off chokes the pad's own sounding hit (a held pedal, a muted cymbal).
    pub choke_on_off: bool,
    /// Voices this pad may hold at once; 0 is no limit.
    pub polyphony: u8,
    /// Metal: follows the hi-hat pedal (the kit's controller) once one has arrived.
    pub pedal: bool,
    /// Metal: 0 closed … 1 open; the decay moves to `open_decay` as it opens.
    pub openness: f32,
    pub open_decay: f32,
    /// 808 kick: how far the pitch sags as the resonator rings down.
    pub sigh: f32,
    /// Modal snare: how strongly the wires follow the head.
    pub wires: f32,
    /// Modal: material (0 membrane, 1 bar, 2 wood, 3 plate, 4 bell), mode count, the
    /// high modes' extra damping, strike position centre → edge, mallet hardness,
    /// tension glide (semitones), and shell/air loading of a membrane.
    pub material: u8,
    pub modes: u8,
    pub damping: f32,
    pub position: f32,
    pub hardness: f32,
    pub glide: f32,
    pub shell: f32,
    /// 808 cymbal: low band ↔ high band.
    pub band_mix: f32,
    /// Modal cymbal: how long the shimmer takes to swell, ms.
    pub swell_ms: f32,
    /// Clap: how many hands, how far apart (ms), and the band's Q.
    pub bursts: u8,
    pub spacing_ms: f32,
    pub q: f32,
}

impl Default for DrumPad {
    fn default() -> Self {
        DrumPad::new(36, "", PadVoice::Kick)
    }
}

impl DrumPad {
    /// A pad with the kick's numbers, on `pitch`, called `name`. The named constructors
    /// (`pads_kit.rs`) are the shipped sounds; this is the blank they start from. Every
    /// Pulsus II field is at the value that changes no sample.
    pub fn new(pitch: Pitch, name: &str, voice: PadVoice) -> DrumPad {
        DrumPad {
            id: PadId(0),
            pitch,
            name: name.to_string(),
            voice,
            tune: 0.0,
            decay: 0.36,
            pitch_drop: 15.5,
            pitch_decay: 0.04,
            click: 0.35,
            // Only a kick saturates by default: from model 2 on, drive reaches every voice.
            drive: if voice == PadVoice::Kick { 0.2 } else { 0.0 },
            noise_decay: 0.2,
            tone: 1_800.0,
            mix: 0.5,
            hp_cutoff: 7_000.0,
            level_db: 0.0,
            pan: 0.0,
            choke: 0,
            muted: false,
            sample: None,
            model: 0,
            layers: Vec::new(),
            round_robin: RoundRobin::Off,
            output: None,
            attack_ms: 0.0,
            hold_ms: 0.0,
            curve: 0.0,
            pitch_env: 0.0,
            pitch_env_ms: 50.0,
            filter: 0,
            cutoff: 1_000.0,
            resonance: 0.1,
            filter_env: 0.0,
            trans_attack: 0.0,
            trans_sustain: 0.0,
            drive_mode: 0,
            phase: 0.0,
            vel_level: 100.0,
            vel_tone: 50.0,
            vel_pitch: 0.0,
            vel_decay: 0.0,
            choke_fade_ms: 4.0,
            choke_on_off: false,
            polyphony: 0,
            pedal: false,
            openness: 0.0,
            open_decay: 1.2,
            sigh: 0.3,
            wires: 0.5,
            material: 0,
            modes: 12,
            damping: 0.5,
            position: 0.3,
            hardness: 0.5,
            glide: 1.0,
            shell: 0.5,
            band_mix: 0.5,
            swell_ms: 40.0,
            bursts: 4,
            spacing_ms: 10.0,
            q: 2.0,
        }
    }

    /// The model index, clamped to the models this voice has.
    pub fn model_index(&self) -> usize {
        (self.model as usize).min(self.voice.models().len() - 1)
    }
}

/// Kit-wide settings of Pulsus II.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KitSettings {
    /// Seeds the noise, round robin and every other choice a hit makes (with the pad and
    /// the note), so two renders of one kit agree and a different seed differs.
    pub seed: u32,
    /// The hi-hat pedal's controller number (CC4) and whether it is reversed (some e-kit
    /// pedals send 127 for closed).
    pub hat_cc: u8,
    pub hat_invert: bool,
}

impl Default for KitSettings {
    fn default() -> Self {
        KitSettings {
            seed: 1,
            hat_cc: 4,
            hat_invert: false,
        }
    }
}

/// Parameters of **Pulsus**, the built-in drum kit.
///
/// The pads are kept **sorted by pitch**, which is the order a note editor draws its lanes
/// in and the order a channel rack lists its rows in. One order, decided once, so the two
/// views can never disagree about which row is which.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "KitFile")]
pub struct PulsusParams {
    /// The model: 1 is Pulsus I (what a file with no version was written by), 2 is
    /// Pulsus II. See the module documentation for what it changes.
    pub version: u8,
    pub pads: Vec<DrumPad>,
    pub kit: KitSettings,
}

/// What a kit looks like on disk: `version` is absent from the files Pulsus I wrote.
#[derive(Deserialize)]
struct KitFile {
    #[serde(default = "pulsus_one")]
    version: u8,
    pads: Vec<DrumPad>,
    #[serde(default)]
    kit: KitSettings,
}

fn pulsus_one() -> u8 {
    1
}

impl From<KitFile> for PulsusParams {
    fn from(f: KitFile) -> Self {
        let mut k = PulsusParams {
            version: f.version.max(1),
            pads: f.pads,
            kit: f.kit,
        };
        k.assign_ids();
        k
    }
}

impl PulsusParams {
    /// A kit of exactly these pads, sorted by pitch, with ids for any that have none.
    ///
    /// `single(DrumPad::kick())` is how a drum becomes **its own track** — with its own
    /// mixer strip, its own inserts and its own place in the channel rack — which is the
    /// arrangement FL's channel rack is built around. The seven-pad default is the other
    /// arrangement: one track, one strip, every drum inside it. Both are the same type.
    pub fn of(mut pads: Vec<DrumPad>) -> PulsusParams {
        pads.sort_by_key(|p| p.pitch);
        pads.dedup_by_key(|p| p.pitch);
        let mut k = PulsusParams {
            version: PULSUS_MODEL,
            pads,
            kit: KitSettings::default(),
        };
        k.assign_ids();
        k
    }

    pub fn single(pad: DrumPad) -> PulsusParams {
        PulsusParams::of(vec![pad])
    }

    /// Give every pad with no id, or a repeated one, the next free id — in pitch order,
    /// so an old file always gets the same ids.
    pub fn assign_ids(&mut self) {
        let mut seen = std::collections::BTreeSet::new();
        let mut next = self.pads.iter().map(|p| p.id.0).max().unwrap_or(0);
        for pad in &mut self.pads {
            if pad.id.0 == 0 || !seen.insert(pad.id) {
                next = next.saturating_add(1);
                pad.id = PadId(next);
                seen.insert(pad.id);
            }
        }
    }

    /// The id a new pad should take: one above every id the kit holds.
    pub fn next_id(&self) -> PadId {
        PadId(self.pads.iter().map(|p| p.id.0).max().unwrap_or(0) + 1)
    }

    pub fn pad(&self, index: usize) -> Option<&DrumPad> {
        self.pads.get(index)
    }

    pub fn pad_mut(&mut self, index: usize) -> Option<&mut DrumPad> {
        self.pads.get_mut(index)
    }

    pub fn index_of_pitch(&self, pitch: Pitch) -> Option<usize> {
        self.pads.iter().position(|p| p.pitch == pitch)
    }

    pub fn index_of(&self, id: PadId) -> Option<usize> {
        self.pads.iter().position(|p| p.id == id)
    }

    pub fn by_id(&self, id: PadId) -> Option<&DrumPad> {
        self.pads.iter().find(|p| p.id == id)
    }

    pub fn by_id_mut(&mut self, id: PadId) -> Option<&mut DrumPad> {
        self.pads.iter_mut().find(|p| p.id == id)
    }

    /// Every layer file the kit plays, in the order the engine attaches them after the
    /// pads' own samples: every pad's layers, in pad order.
    pub fn layer_paths(&self) -> impl Iterator<Item = &str> {
        self.pads
            .iter()
            .flat_map(|p| p.layers.iter().take(MAX_LAYERS).map(|l| l.sample.as_str()))
    }

    /// What upgrading this kit to the current model changes besides the version: the
    /// drive a model 1 kit stored on voices that did not use it, which model 2 would now
    /// hear. `(pad, field, value)`, for the caller to send as commands.
    pub fn upgrade_edits(&self) -> Vec<(PadId, PadField, f32)> {
        if self.version >= PULSUS_MODEL {
            return Vec::new();
        }
        self.pads
            .iter()
            .filter(|p| p.voice != PadVoice::Kick && p.drive != 0.0)
            .map(|p| (p.id, PadField::Drive, 0.0))
            .collect()
    }
}

impl Default for PulsusParams {
    /// The shipped kit, on the General MIDI drum map: seven pads, the two hats sharing
    /// choke group 1. [`PulsusParams::studio`] is the full Pulsus II kit.
    fn default() -> Self {
        PulsusParams::of(vec![
            DrumPad::kick(),
            DrumPad::rim(),
            DrumPad::snare(),
            DrumPad::closed_hat(),
            DrumPad::low_tom(),
            DrumPad::open_hat(),
            DrumPad::high_tom(),
        ])
    }
}

/// The vertical axis of a note editor, as the *instrument* sees it.
///
/// The plan for this asked for `lanes() -> Option<&[Pitch]>`. It is this instead, because
/// an editor needs three things and a slice of pitches is only one of them: lane → pitch,
/// pitch → lane, and lane → name.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Lanes<'a> {
    /// A keyboard: 128 lanes, one per pitch, named `C4`, `F#2` and so on.
    Chromatic,
    /// One lane per pad, ascending by pitch.
    Pads(&'a [DrumPad]),
}

impl Lanes<'_> {
    /// Is every pitch a lane? True for a keyboard, false for anything with named pads.
    pub fn is_chromatic(&self) -> bool {
        matches!(self, Lanes::Chromatic)
    }

    pub fn count(&self) -> usize {
        match self {
            Lanes::Chromatic => 128,
            Lanes::Pads(p) => p.len(),
        }
    }

    /// The pitch a lane plays, counting from the **bottom**.
    pub fn pitch(&self, lane: usize) -> Option<Pitch> {
        match self {
            Lanes::Chromatic => (lane < 128).then_some(lane as Pitch),
            Lanes::Pads(p) => p.get(lane).map(|p| p.pitch),
        }
    }

    /// The pad a lane is, by identity: what a view keeps when it wants to follow the pad
    /// through an insert below it.
    pub fn pad_id(&self, lane: usize) -> Option<PadId> {
        match self {
            Lanes::Chromatic => None,
            Lanes::Pads(p) => p.get(lane).map(|p| p.id),
        }
    }

    /// Which lane a pitch is on, or `None` when this instrument has no lane for it.
    pub fn lane_of(&self, pitch: Pitch) -> Option<usize> {
        match self {
            Lanes::Chromatic => Some(pitch as usize),
            Lanes::Pads(p) => p.iter().position(|p| p.pitch == pitch),
        }
    }

    /// Which lane a pad is on, by identity.
    pub fn lane_of_pad(&self, id: PadId) -> Option<usize> {
        match self {
            Lanes::Chromatic => None,
            Lanes::Pads(p) => p.iter().position(|p| p.id == id),
        }
    }

    /// What to write beside the lane.
    pub fn name(&self, lane: usize) -> String {
        match self {
            Lanes::Chromatic => pitch_name(lane.min(127) as Pitch),
            Lanes::Pads(p) => p
                .get(lane)
                .map(|p| p.name.clone())
                .unwrap_or_else(String::new),
        }
    }
}

#[cfg(test)]
#[path = "pads_tests.rs"]
mod tests;
