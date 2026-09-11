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

use crate::model::{pitch_name, Pitch};
use serde::{Deserialize, Serialize};

/// What makes a pad's sound: one of Pulsus's three synthesized shapes, or a sample.
///
/// The kit ships more pads than shapes: the toms and the rim are the snare's shape at
/// different tunings and mixes, which is a *preset*, not a fourth voice. Saying so is
/// worth more than pretending otherwise, and it is why adding a tom costs no DSP.
///
/// `Sample` is a voice like the other three rather than a second instrument, so a kit can
/// mix a synthesized kick with a sampled snare, and the rack, the inspector, the choke
/// groups and the mute all work the same way for both. The file itself is
/// [`DrumPad::sample`]; the voice only says that it is used.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PadVoice {
    Kick,
    Snare,
    HiHat,
    Sample,
}

impl PadVoice {
    pub const ALL: [PadVoice; 4] = [
        PadVoice::Kick,
        PadVoice::Snare,
        PadVoice::HiHat,
        PadVoice::Sample,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            PadVoice::Kick => "pad.voice.kick",
            PadVoice::Snare => "pad.voice.snare",
            PadVoice::HiHat => "pad.voice.hihat",
            PadVoice::Sample => "pad.voice.sample",
        }
    }

    pub fn index(self) -> usize {
        match self {
            PadVoice::Kick => 0,
            PadVoice::Snare => 1,
            PadVoice::HiHat => 2,
            PadVoice::Sample => 3,
        }
    }

    pub fn from_index(i: usize) -> PadVoice {
        PadVoice::ALL[i.min(3)]
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
}

impl SampleData {
    pub fn len_frames(&self) -> usize {
        self.frames.len() / self.channels.max(1) as usize
    }
}

/// One drum in a kit.
///
/// Flat rather than a per-voice enum on purpose: every field is a plain `f32` the command
/// bus can set and undo without knowing which shape the pad is, and
/// [`PadField::applies_to`] is what keeps the inspector from offering a hi-hat a pitch
/// envelope. A union would move that filter into three `match` arms in the command path,
/// where it would be load-bearing instead of cosmetic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrumPad {
    /// The note that plays this pad. General MIDI's drum map, so a drum controller and
    /// an imported MIDI file both land where a musician expects.
    pub pitch: Pitch,
    /// The pad's name, which travels **in the kit**: renaming it once renames it in every
    /// clip and every project that uses this kit.
    pub name: String,
    pub voice: PadVoice,
    /// Semitones, tuning the whole voice at once.
    pub tune: f32,
    /// Main amplitude decay to −60 dB, seconds.
    pub decay: f32,
    /// Kick: how far the pitch falls, semitones.
    pub pitch_drop: f32,
    /// Kick: how fast it falls, seconds.
    pub pitch_decay: f32,
    /// Kick: level of the click transient.
    pub click: f32,
    /// Kick: soft saturation. Zero is exactly clean.
    pub drive: f32,
    /// Snare: the noise's own decay, seconds.
    pub noise_decay: f32,
    /// Where the voice's noise or partials sit, Hz. The kick's click high-pass, the
    /// snare's noise band-pass, the hi-hat's band-pass.
    pub tone: f32,
    /// Snare: 0 is all body, 1 is all noise.
    pub mix: f32,
    /// Hi-hat: the high-pass after the band-pass, Hz.
    pub hp_cutoff: f32,
    /// The pad's own level trim. 0 dB is the pad as the kit was balanced.
    pub level_db: f32,
    /// The pad's place in the kit's stereo image, −1..=1. Not the track's pan: a kit is a
    /// dozen sources on one channel, and spreading them is part of the instrument.
    pub pan: f32,
    /// Choke group; 0 is none. Triggering a pad silences sounding voices in the same
    /// group — without it an open hat is never cut by a closed one, and the kit stops
    /// being a kit.
    pub choke: u8,
    /// A muted pad keeps its notes and its place in the rack; it just does not sound.
    /// Resolved when the snapshot is compiled, like a muted track.
    #[serde(default)]
    pub muted: bool,
    /// The file a [`PadVoice::Sample`] pad plays. Kept as the path it was given, so a
    /// project points at the musician's sample library rather than copying it; a file
    /// that cannot be found makes the pad silent and the inspector say so, never a
    /// project that will not open.
    #[serde(default)]
    pub sample: Option<String>,
}

impl DrumPad {
    /// A pad with the kick's numbers, on `pitch`, called `name`. The named constructors
    /// below are the shipped sounds; this is the blank they start from.
    pub fn new(pitch: Pitch, name: &str, voice: PadVoice) -> DrumPad {
        DrumPad {
            pitch,
            name: name.to_string(),
            voice,
            tune: 0.0,
            decay: 0.36,
            pitch_drop: 15.5,
            pitch_decay: 0.04,
            click: 0.35,
            drive: 0.2,
            noise_decay: 0.2,
            tone: 1_800.0,
            mix: 0.5,
            hp_cutoff: 7_000.0,
            level_db: 0.0,
            pan: 0.0,
            choke: 0,
            muted: false,
            sample: None,
        }
    }
}

/// An addressable field of a pad — the "plugin parameter id" of a drum.
///
/// The ranges and defaults depend on the pad's voice ([`PadField::info`]): a hi-hat's tone
/// lives around 9 kHz and a snare's around 2 kHz, and offering both the same slider would
/// make one of them unusable.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PadField {
    /// Which voice the pad is, as an index into [`PadVoice::ALL`] — a stepped parameter
    /// like Firmus's waveform selector, so switching a pad from a synthesized snare to a
    /// sample is one undoable command like everything else.
    Voice,
    Level,
    Pan,
    Choke,
    Mute,
    Tune,
    Decay,
    PitchDrop,
    PitchDecay,
    Click,
    Drive,
    NoiseDecay,
    Tone,
    Mix,
    HpCutoff,
}

/// Display and range metadata for a pad field, for a particular voice.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PadParamInfo {
    pub field: PadField,
    pub label_key: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub logarithmic: bool,
    pub stepped: bool,
}

impl PadField {
    pub const ALL: [PadField; 15] = [
        PadField::Voice,
        PadField::Level,
        PadField::Pan,
        PadField::Choke,
        PadField::Mute,
        PadField::Tune,
        PadField::Decay,
        PadField::PitchDrop,
        PadField::PitchDecay,
        PadField::Click,
        PadField::Drive,
        PadField::NoiseDecay,
        PadField::Tone,
        PadField::Mix,
        PadField::HpCutoff,
    ];

    /// Does this voice have this field at all? The inspector and the channel rack both
    /// ask, so a hi-hat is never offered a pitch envelope it does not have.
    pub fn applies_to(self, voice: PadVoice) -> bool {
        use PadField::*;
        match self {
            Voice | Level | Pan | Choke | Mute | Tune | Decay => true,
            PitchDrop | PitchDecay | Click | Drive => voice == PadVoice::Kick,
            NoiseDecay | Mix => voice == PadVoice::Snare,
            Tone => voice != PadVoice::Sample,
            HpCutoff => voice == PadVoice::HiHat,
        }
    }

    /// Every field this voice actually has, in the order an inspector should show them.
    pub fn for_voice(voice: PadVoice) -> impl Iterator<Item = PadField> {
        PadField::ALL
            .into_iter()
            .filter(move |f| f.applies_to(voice))
    }

    pub fn info(self, voice: PadVoice) -> PadParamInfo {
        use PadField::*;
        let i = |label_key, min, max, default, unit, logarithmic, stepped| PadParamInfo {
            field: self,
            label_key,
            min,
            max,
            default,
            unit,
            logarithmic,
            stepped,
        };
        match self {
            Voice => i("pad.voice", 0.0, 3.0, voice.index() as f32, "", false, true),
            Level => i("pad.level", -24.0, 12.0, 0.0, "dB", false, false),
            Pan => i("pad.pan", -1.0, 1.0, 0.0, "", false, false),
            // Zero is "no group", so the range starts there and the numbers above it are
            // names, not amounts.
            Choke => i("pad.choke", 0.0, 8.0, 0.0, "", false, true),
            Mute => i("pad.mute", 0.0, 1.0, 0.0, "", false, true),
            Tune => i("pad.tune", -24.0, 24.0, 0.0, "st", false, false),
            Decay => match voice {
                PadVoice::Kick => i("pad.decay", 0.02, 2.0, 0.36, "s", true, false),
                PadVoice::Snare => i("pad.decay", 0.01, 1.5, 0.13, "s", true, false),
                PadVoice::HiHat => i("pad.decay", 0.01, 2.0, 0.06, "s", true, false),
                // A sample plays to its end by default; the decay is there to shorten
                // one, so its default is longer than any drum hit.
                PadVoice::Sample => i("pad.decay", 0.02, 8.0, 4.0, "s", true, false),
            },
            PitchDrop => i("pad.pitch_drop", 0.0, 36.0, 15.5, "st", false, false),
            PitchDecay => i("pad.pitch_decay", 0.005, 0.4, 0.04, "s", true, false),
            Click => i("pad.click", 0.0, 1.0, 0.35, "", false, false),
            Drive => i("pad.drive", 0.0, 1.0, 0.2, "", false, false),
            NoiseDecay => i("pad.noise_decay", 0.01, 2.0, 0.19, "s", true, false),
            Tone => match voice {
                PadVoice::Kick => i("pad.tone", 300.0, 8_000.0, 1_800.0, "Hz", true, false),
                PadVoice::Snare | PadVoice::Sample => {
                    i("pad.tone", 300.0, 8_000.0, 1_900.0, "Hz", true, false)
                }
                PadVoice::HiHat => i("pad.tone", 2_000.0, 16_000.0, 9_500.0, "Hz", true, false),
            },
            Mix => i("pad.mix", 0.0, 1.0, 0.55, "", false, false),
            HpCutoff => i(
                "pad.hp_cutoff",
                1_000.0,
                16_000.0,
                7_000.0,
                "Hz",
                true,
                false,
            ),
        }
    }

    pub fn get(self, pad: &DrumPad) -> f32 {
        use PadField::*;
        match self {
            Voice => pad.voice.index() as f32,
            Level => pad.level_db,
            Pan => pad.pan,
            Choke => pad.choke as f32,
            Mute => pad.muted as u8 as f32,
            Tune => pad.tune,
            Decay => pad.decay,
            PitchDrop => pad.pitch_drop,
            PitchDecay => pad.pitch_decay,
            Click => pad.click,
            Drive => pad.drive,
            NoiseDecay => pad.noise_decay,
            Tone => pad.tone,
            Mix => pad.mix,
            HpCutoff => pad.hp_cutoff,
        }
    }

    /// Set the field, clamping to its legal range. Returns the value actually stored, so
    /// the command bus records what happened rather than what was asked for.
    pub fn set(self, pad: &mut DrumPad, value: f32) -> f32 {
        use PadField::*;
        let info = self.info(pad.voice);
        let v = if value.is_finite() {
            value.clamp(info.min, info.max)
        } else {
            info.default
        };
        let v = if info.stepped { v.round() } else { v };
        match self {
            // Only the voice. A first version also moved the decay to the new voice's
            // default, and the undo fuzz caught it within a minute: the inverse command
            // restores the *voice*, not the decay it dragged along, so undo was not exact.
            // A decay outside the new voice's range is clamped by the engine and shown
            // pinned by the slider, which is a small cost and an honest one.
            Voice => pad.voice = PadVoice::from_index(v as usize),
            Level => pad.level_db = v,
            Pan => pad.pan = v,
            Choke => pad.choke = v as u8,
            Mute => pad.muted = v >= 0.5,
            Tune => pad.tune = v,
            Decay => pad.decay = v,
            PitchDrop => pad.pitch_drop = v,
            PitchDecay => pad.pitch_decay = v,
            Click => pad.click = v,
            Drive => pad.drive = v,
            NoiseDecay => pad.noise_decay = v,
            Tone => pad.tone = v,
            Mix => pad.mix = v,
            HpCutoff => pad.hp_cutoff = v,
        }
        v
    }
}

impl DrumPad {
    /// The shipped kick, on General MIDI 36.
    pub fn kick() -> DrumPad {
        let mut kick = DrumPad::new(36, "Kick", PadVoice::Kick);
        kick.tone = 2_400.0;
        kick
    }

    /// The shipped rim (GM 37): the snare shape, short and high.
    pub fn rim() -> DrumPad {
        let mut rim = DrumPad::new(37, "Rim", PadVoice::Snare);
        rim.tune = 14.0;
        rim.decay = 0.035;
        rim.mix = 0.25;
        rim.tone = 3_200.0;
        rim.noise_decay = 0.03;
        rim.level_db = -5.0;
        rim
    }

    /// The shipped snare, on GM 38.
    pub fn snare() -> DrumPad {
        let mut snare = DrumPad::new(38, "Snare", PadVoice::Snare);
        snare.decay = 0.13;
        snare.noise_decay = 0.19;
        snare.tone = 1_900.0;
        snare.mix = 0.55;
        snare
    }

    /// The shipped closed hat (GM 42), in choke group 1 with the open one.
    pub fn closed_hat() -> DrumPad {
        let mut closed = DrumPad::new(42, "Closed hat", PadVoice::HiHat);
        closed.decay = 0.055;
        closed.tone = 9_500.0;
        closed.choke = 1;
        closed.level_db = -3.0;
        closed
    }

    /// The shipped low tom (GM 45): the snare shape, all body, tuned down.
    pub fn low_tom() -> DrumPad {
        let mut tom = DrumPad::new(45, "Low tom", PadVoice::Snare);
        tom.tune = -9.0;
        tom.decay = 0.42;
        tom.mix = 0.06;
        tom.level_db = -2.0;
        tom
    }

    /// The shipped open hat (GM 46), in choke group 1 with the closed one.
    pub fn open_hat() -> DrumPad {
        let mut open = DrumPad::new(46, "Open hat", PadVoice::HiHat);
        open.decay = 0.42;
        open.tone = 9_500.0;
        open.choke = 1;
        open.level_db = -4.0;
        open
    }

    /// The shipped high tom (GM 50).
    pub fn high_tom() -> DrumPad {
        let mut tom = DrumPad::new(50, "High tom", PadVoice::Snare);
        tom.tune = -1.0;
        tom.decay = 0.32;
        tom.mix = 0.06;
        tom.level_db = -2.0;
        tom
    }
}

/// Parameters of **Pulsus**, the built-in synthesized drum kit.
///
/// The pads are kept **sorted by pitch**, which is the order a note editor draws its lanes
/// in and the order a channel rack lists its rows in. One order, decided once, so the two
/// views can never disagree about which row is which.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PulsusParams {
    pub pads: Vec<DrumPad>,
}

impl PulsusParams {
    /// A kit of exactly these pads, sorted by pitch.
    ///
    /// `single(DrumPad::kick())` is how a drum becomes **its own track** — with its own
    /// mixer strip, its own inserts and its own place in the channel rack — which is the
    /// arrangement FL's channel rack is built around and the one a musician who wants to
    /// route the kick and the snare differently needs. The seven-pad default is the other
    /// arrangement: one track, one strip, every drum inside it. Both are the same type.
    pub fn of(mut pads: Vec<DrumPad>) -> PulsusParams {
        pads.sort_by_key(|p| p.pitch);
        pads.dedup_by_key(|p| p.pitch);
        PulsusParams { pads }
    }

    pub fn single(pad: DrumPad) -> PulsusParams {
        PulsusParams { pads: vec![pad] }
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
}

impl Default for PulsusParams {
    /// The shipped kit, on the General MIDI drum map.
    ///
    /// Three shapes, seven pads: the toms and the rim are the snare shape retuned, which
    /// is why they cost nothing. The two hats share **choke group 1**, which is the whole
    /// reason the group exists — a closed hat that does not stop an open one is not a
    /// hi-hat.
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
/// pitch → lane, and lane → name. Handing back the slice would push the other two, and the
/// keyboard fallback for all three, into every call site — which is where the "is this a
/// drum or a piano" branch would then live, in as many copies as there are views.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Lanes<'a> {
    /// A keyboard: 128 lanes, one per pitch, named `C4`, `F#2` and so on.
    Chromatic,
    /// One lane per pad, ascending by pitch. A sampler with named zones adds a variant
    /// here and no editor changes, which is the point of the type.
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

    /// The pitch a lane plays, counting from the **bottom** — lane 0 is the lowest, which
    /// is where a kick sits on a kit and where C-1 sits on a keyboard.
    pub fn pitch(&self, lane: usize) -> Option<Pitch> {
        match self {
            Lanes::Chromatic => (lane < 128).then_some(lane as Pitch),
            Lanes::Pads(p) => p.get(lane).map(|p| p.pitch),
        }
    }

    /// Which lane a pitch is on, or `None` when this instrument has no lane for it.
    pub fn lane_of(&self, pitch: Pitch) -> Option<usize> {
        match self {
            Lanes::Chromatic => Some(pitch as usize),
            Lanes::Pads(p) => p.iter().position(|p| p.pitch == pitch),
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
mod tests {
    use super::*;

    #[test]
    fn the_shipped_kit_is_sorted_by_pitch_and_chokes_its_hats() {
        let kit = PulsusParams::default();
        assert!(
            kit.pads.windows(2).all(|w| w[0].pitch < w[1].pitch),
            "pads must be sorted by pitch and unique: the rack and the piano roll both \
             draw them in this order"
        );
        let closed = kit.pads.iter().find(|p| p.pitch == 42).expect("closed hat");
        let open = kit.pads.iter().find(|p| p.pitch == 46).expect("open hat");
        assert_ne!(closed.choke, 0, "the hats must share a choke group");
        assert_eq!(closed.choke, open.choke);
        // Nothing else may join that group by accident: a kick cut by a hat is a bug you
        // would blame on the sequencer for a week.
        for pad in &kit.pads {
            if pad.pitch != 42 && pad.pitch != 46 {
                assert_eq!(pad.choke, 0, "{} is in a choke group", pad.name);
            }
        }
    }

    #[test]
    fn every_pad_field_round_trips_and_clamps() {
        for voice in PadVoice::ALL {
            let mut pad = DrumPad::new(36, "p", voice);
            for field in PadField::for_voice(voice) {
                let info = field.info(voice);
                assert!(
                    info.min <= info.default && info.default <= info.max,
                    "{field:?}"
                );
                if field == PadField::Voice {
                    // Setting the voice changes which ranges apply, so it is checked on
                    // its own: the index round-trips and out-of-range clamps to the last.
                    assert_eq!(field.set(&mut pad, 99.0), 3.0);
                    assert_eq!(pad.voice, PadVoice::Sample);
                    let back = voice.index() as f32;
                    assert_eq!(field.set(&mut pad, back), back);
                    assert_eq!(pad.voice, voice);
                    continue;
                }
                assert_eq!(field.set(&mut pad, info.max + 1_000.0), info.max);
                assert_eq!(field.get(&pad), info.max);
                assert_eq!(field.set(&mut pad, f32::NAN), info.default);
                assert_eq!(field.get(&pad), info.default);
            }
        }
    }

    /// Every shipped pad must sit inside the ranges its own voice declares, or the
    /// inspector would show a slider already pinned to an end stop.
    #[test]
    fn the_shipped_kit_stays_inside_its_own_ranges() {
        for pad in PulsusParams::default().pads {
            for field in PadField::for_voice(pad.voice) {
                let info = field.info(pad.voice);
                let v = field.get(&pad);
                assert!(
                    v >= info.min && v <= info.max,
                    "{}: {field:?} is {v}, outside {}..{}",
                    pad.name,
                    info.min,
                    info.max
                );
            }
        }
    }

    #[test]
    fn chromatic_lanes_are_the_keyboard_they_replaced() {
        let l = Lanes::Chromatic;
        assert!(l.is_chromatic());
        assert_eq!(l.count(), 128);
        for pitch in 0u8..128 {
            assert_eq!(l.lane_of(pitch), Some(pitch as usize));
            assert_eq!(l.pitch(pitch as usize), Some(pitch));
            assert_eq!(l.name(pitch as usize), crate::model::pitch_name(pitch));
        }
        assert_eq!(l.pitch(128), None);
    }

    #[test]
    fn pad_lanes_name_the_drum_and_ignore_pitches_the_kit_has_no_pad_for() {
        let kit = PulsusParams::default();
        let l = Lanes::Pads(&kit.pads);
        assert!(!l.is_chromatic());
        assert_eq!(l.count(), kit.pads.len());
        assert_eq!(l.pitch(0), Some(36));
        assert_eq!(l.name(0), "Kick");
        assert_eq!(l.lane_of(38).map(|i| l.name(i)).as_deref(), Some("Snare"));
        assert_eq!(l.lane_of(60), None, "middle C is not a drum");
        // Lane and pitch are inverses wherever both exist.
        for lane in 0..l.count() {
            let pitch = l.pitch(lane).unwrap();
            assert_eq!(l.lane_of(pitch), Some(lane));
        }
    }
}
