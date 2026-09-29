//! Gravis — the bass instrument (808, modelled electric, synth): its parameter model.
//! Owned by this instrument's builder (ADR 0023). The DSP is `cantus_dsp::gravis`, the
//! unit `cantus_engine::inst::bass`, the editor `cantus_ui::views::inst::bass`, and what
//! each parameter does is in `docs/instruments/gravis.md`.
//!
//! One instrument, three engines (`engine`: 808 · Synth · String) behind one mono/legato
//! voice with glide, a phase-locked sub, a fundamental-locked drive and an output stage.
//! Every engine's parameters are stored side by side, so switching engines is lossless and
//! one undo step.
//!
//! Keys are forever once shipped. Units are the ones a musician reads; a unit of `%` is
//! stored as 0…1 (the knob prints it ×100). Stepped keys hold an index whose meaning is
//! listed beside it.

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.bass";
pub const ICON: TrackIcon = TrackIcon::Strings;
/// Offered in the instrument picker once it is built.
pub const READY: bool = true;

/// Defines [`PARAMS`] and, in the same order, the index constants of [`ix`], so the engine
/// reads a value array by constant index without formatting or comparing a string.
macro_rules! params {
    ($( $id:ident = ($key:literal, $label:literal, $min:expr, $max:expr, $def:expr, $unit:literal, $log:expr, $step:expr) ),* $(,)?) => {
        /// Index of each parameter in [`PARAMS`].
        pub mod ix {
            #[allow(
                non_camel_case_types,
                clippy::upper_case_acronyms,
                clippy::enum_variant_names,
                dead_code
            )]
            #[repr(usize)]
            enum I { $($id),* }
            $( pub const $id: usize = I::$id as usize; )*
        }
        pub const PARAMS: &[InstParamInfo] = &[ $( p($key, $label, $min, $max, $def, $unit, $log, $step) ),* ];
    };
}

params! {
    // ---- engine and voice (shared) ----
    // 0 808 · 1 Synth · 2 String
    ENGINE = ("engine", "bass.engine", 0.0, 2.0, 0.0, "", false, true),
    // 0 Legato · 1 Mono (retrigger) · 2 Strings (String engine: one voice per string)
    MODE = ("mode", "bass.mode", 0.0, 2.0, 0.0, "", false, true),
    // 0 Last · 1 Low · 2 High
    PRIORITY = ("priority", "bass.priority", 0.0, 2.0, 0.0, "", false, true),
    GLIDE = ("glide", "bass.glide", 1.0, 2000.0, 60.0, "ms", true, false),
    // 0 Legato only · 1 Always · 2 Off
    GLIDE_TRIGGER = ("glide.trigger", "bass.glide_trigger", 0.0, 2.0, 0.0, "", false, true),
    // 0 Constant time (`glide` is the time) · 1 Constant rate (`glide` is per octave)
    GLIDE_RATE = ("glide.rate", "bass.glide_rate", 0.0, 1.0, 0.0, "", false, true),
    // 0 Exponential · 1 Linear · 2 S-curve
    GLIDE_SHAPE = ("glide.shape", "bass.glide_shape", 0.0, 2.0, 0.0, "", false, true),
    LEGATO_WINDOW = ("legato_window", "bass.legato_window", 0.0, 30.0, 5.0, "ms", false, false),
    DECLICK = ("declick", "bass.declick", 0.0, 5.0, 1.5, "ms", false, false),
    BEND_RANGE = ("bend_range", "bass.bend_range", 0.0, 24.0, 2.0, "st", false, true),
    OCTAVE = ("octave", "bass.octave", -2.0, 2.0, 0.0, "oct", false, true),
    TRANSPOSE = ("transpose", "bass.transpose", -12.0, 12.0, 0.0, "st", false, true),
    FINE = ("fine", "bass.fine", -50.0, 50.0, 0.0, "ct", false, false),
    A4 = ("a4", "bass.a4", 415.0, 466.0, 440.0, "Hz", false, false),
    // 0 Reset (same start every note) · 1 Random (seeded by the note) · 2 Free
    PHASE = ("phase", "bass.phase", 0.0, 2.0, 0.0, "", false, true),
    VELOCITY = ("velocity", "bass.velocity", 0.0, 1.0, 0.6, "%", false, false),
    LFO_RATE = ("lfo.rate", "bass.lfo_rate", 0.05, 40.0, 2.0, "Hz", true, false),
    // 0 Sine · 1 Triangle · 2 Square · 3 Saw down
    LFO_SHAPE = ("lfo.shape", "bass.lfo_shape", 0.0, 3.0, 0.0, "", false, true),

    // ---- 808 ----
    E808_SHAPE = ("808.shape", "bass.808.shape", 0.0, 1.0, 0.0, "%", false, false),
    E808_PITCH = ("808.pitch", "bass.808.pitch", 0.0, 48.0, 12.0, "st", false, false),
    E808_PITCH_TIME = ("808.pitch_time", "bass.808.pitch_time", 1.0, 500.0, 45.0, "ms", true, false),
    E808_KNOCK = ("808.knock", "bass.808.knock", 0.0, 1.0, 0.2, "%", false, false),
    E808_CLICK = ("808.click", "bass.808.click", 0.0, 1.0, 0.15, "%", false, false),
    E808_CLICK_TONE = ("808.click_tone", "bass.808.click_tone", 500.0, 16000.0, 3000.0, "Hz", true, false),
    E808_ATTACK = ("808.attack", "bass.808.attack", 0.0, 50.0, 1.0, "ms", false, false),
    E808_HOLD = ("808.hold", "bass.808.hold", 0.0, 2000.0, 0.0, "ms", false, false),
    E808_DECAY = ("808.decay", "bass.808.decay", 10.0, 10000.0, 1200.0, "ms", true, false),
    E808_SUSTAIN = ("808.sustain", "bass.808.sustain", 0.0, 1.0, 0.6, "%", false, false),
    E808_RELEASE = ("808.release", "bass.808.release", 5.0, 4000.0, 250.0, "ms", true, false),
    E808_SIGH = ("808.sigh", "bass.808.sigh", 0.0, 1.0, 0.0, "%", false, false),
    E808_HARMONICS = ("808.harmonics", "bass.808.harmonics", 0.0, 1.0, 0.0, "%", false, false),
    E808_TILT = ("808.tilt", "bass.808.tilt", -6.0, 6.0, -3.0, "dB/oct", false, false),
    E808_EVEN = ("808.even", "bass.808.even", -1.0, 1.0, 0.3, "%", false, false),

    // ---- Synth ----
    // 0 Saw · 1 Square · 2 Pulse · 3 Triangle
    SYN_WAVE = ("syn.wave", "bass.syn.wave", 0.0, 3.0, 0.0, "", false, true),
    SYN_PW = ("syn.pw", "bass.syn.pw", 0.05, 0.95, 0.5, "%", false, false),
    SYN_UNISON = ("syn.unison", "bass.syn.unison", 1.0, 7.0, 1.0, "", false, true),
    SYN_DETUNE = ("syn.detune", "bass.syn.detune", 0.0, 1.0, 0.3, "%", false, false),
    SYN_SPREAD = ("syn.spread", "bass.syn.spread", 0.0, 1.0, 0.5, "%", false, false),
    SYN_WIDTH = ("syn.width", "bass.syn.width", 0.0, 1.0, 0.0, "%", false, false),
    SYN_MONO_BELOW = ("syn.mono_below", "bass.syn.mono_below", 40.0, 300.0, 120.0, "Hz", true, false),
    SYN_OSC2 = ("syn.osc2", "bass.syn.osc2", 0.0, 1.0, 0.0, "%", false, false),
    SYN_OSC2_SEMI = ("syn.osc2_semi", "bass.syn.osc2_semi", -24.0, 24.0, -12.0, "st", false, true),
    SYN_OSC2_FINE = ("syn.osc2_fine", "bass.syn.osc2_fine", -50.0, 50.0, 7.0, "ct", false, false),
    // 0 Ladder 24 · 1 Diode (303) · 2 SVF low-pass 12 · 3 SVF band-pass
    SYN_FILTER = ("syn.filter", "bass.syn.filter", 0.0, 3.0, 0.0, "", false, true),
    SYN_CUTOFF = ("syn.cutoff", "bass.syn.cutoff", 20.0, 20000.0, 400.0, "Hz", true, false),
    SYN_RESO = ("syn.reso", "bass.syn.reso", 0.0, 1.0, 0.2, "%", false, false),
    SYN_BASS_COMP = ("syn.bass_comp", "bass.syn.bass_comp", 0.0, 1.0, 0.5, "%", false, false),
    SYN_DRIVE = ("syn.drive", "bass.syn.drive", 0.0, 1.0, 0.0, "%", false, false),
    SYN_ENV = ("syn.env", "bass.syn.env", -6.0, 6.0, 2.0, "oct", false, false),
    SYN_KEY_TRACK = ("syn.key_track", "bass.syn.key_track", 0.0, 1.0, 0.5, "%", false, false),
    SYN_F_ATTACK = ("syn.f_attack", "bass.syn.f_attack", 0.5, 2000.0, 1.0, "ms", true, false),
    SYN_F_DECAY = ("syn.f_decay", "bass.syn.f_decay", 5.0, 5000.0, 300.0, "ms", true, false),
    SYN_F_SUSTAIN = ("syn.f_sustain", "bass.syn.f_sustain", 0.0, 1.0, 0.0, "%", false, false),
    SYN_F_RELEASE = ("syn.f_release", "bass.syn.f_release", 5.0, 5000.0, 200.0, "ms", true, false),
    SYN_ATTACK = ("syn.attack", "bass.syn.attack", 0.5, 2000.0, 1.0, "ms", true, false),
    SYN_DECAY = ("syn.decay", "bass.syn.decay", 5.0, 5000.0, 600.0, "ms", true, false),
    SYN_SUSTAIN = ("syn.sustain", "bass.syn.sustain", 0.0, 1.0, 0.8, "%", false, false),
    SYN_RELEASE = ("syn.release", "bass.syn.release", 5.0, 5000.0, 120.0, "ms", true, false),
    SYN_ACCENT = ("syn.accent", "bass.syn.accent", 0.0, 1.0, 0.5, "%", false, false),
    SYN_ACCENT_SWEEP = ("syn.accent_sweep", "bass.syn.accent_sweep", 0.0, 1.0, 1.0, "", false, true),
    SYN_ACCENT_VEL = ("syn.accent_vel", "bass.syn.accent_vel", 1.0, 127.0, 100.0, "", false, true),

    // ---- String ----
    STR_STRINGS = ("str.strings", "bass.str.strings", 4.0, 6.0, 4.0, "", false, true),
    // 0 Standard · 1 Drop (lowest string down a tone) · 2 Half step down
    STR_TUNING = ("str.tuning", "bass.str.tuning", 0.0, 2.0, 0.0, "", false, true),
    STR_SCALE = ("str.scale", "bass.str.scale", 762.0, 914.0, 864.0, "mm", false, false),
    STR_FRETS = ("str.frets", "bass.str.frets", 20.0, 24.0, 21.0, "", false, true),
    STR_FRETLESS = ("str.fretless", "bass.str.fretless", 0.0, 1.0, 0.0, "", false, true),
    // 0 Roundwound · 1 Halfwound · 2 Flatwound · 3 Tape
    STR_TYPE = ("str.type", "bass.str.type", 0.0, 3.0, 0.0, "", false, true),
    STR_STIFFNESS = ("str.stiffness", "bass.str.stiffness", 0.0, 2.0, 1.0, "%", false, false),
    STR_SUSTAIN = ("str.sustain", "bass.str.sustain", 0.3, 20.0, 6.0, "s", true, false),
    STR_BRIGHT = ("str.bright", "bass.str.bright", 0.0, 1.0, 0.45, "%", false, false),
    STR_AGE = ("str.age", "bass.str.age", 0.0, 1.0, 0.2, "%", false, false),
    STR_ACTION = ("str.action", "bass.str.action", 1.0, 4.0, 2.2, "mm", false, false),
    STR_BUZZ = ("str.buzz", "bass.str.buzz", 0.0, 1.0, 0.3, "%", false, false),
    // 0 Finger · 1 Pick · 2 Slap (thumb, pop at or above `str.pop_vel`) · 3 Palm mute
    STR_TECHNIQUE = ("str.technique", "bass.str.technique", 0.0, 3.0, 0.0, "", false, true),
    STR_PLUCK = ("str.pluck", "bass.str.pluck", 20.0, 400.0, 180.0, "mm", false, false),
    STR_HARDNESS = ("str.hardness", "bass.str.hardness", 0.0, 1.0, 0.5, "%", false, false),
    STR_POP_VEL = ("str.pop_vel", "bass.str.pop_vel", 1.0, 127.0, 110.0, "", false, true),
    STR_MUTE = ("str.mute", "bass.str.mute", 0.0, 1.0, 0.0, "%", false, false),
    STR_RELEASE = ("str.release", "bass.str.release", 20.0, 2000.0, 150.0, "ms", true, false),
    STR_NOISE = ("str.noise", "bass.str.noise", 0.0, 1.0, 0.3, "%", false, false),
    // 0 Lowest fret · 1 Position (stay near the hand) · 2 Low strings
    STR_FINGERING = ("str.fingering", "bass.str.fingering", 0.0, 2.0, 1.0, "", false, true),
    STR_PU1 = ("str.pu1", "bass.str.pu1", 20.0, 300.0, 60.0, "mm", false, false),
    STR_PU2 = ("str.pu2", "bass.str.pu2", 20.0, 300.0, 159.0, "mm", false, false),
    STR_PU1_LEVEL = ("str.pu1_level", "bass.str.pu1_level", 0.0, 1.0, 1.0, "%", false, false),
    STR_PU2_LEVEL = ("str.pu2_level", "bass.str.pu2_level", 0.0, 1.0, 1.0, "%", false, false),
    // 0 Single coil · 1 Humbucker
    STR_PU_TYPE = ("str.pu_type", "bass.str.pu_type", 0.0, 1.0, 0.0, "", false, true),
    STR_PU_HEIGHT = ("str.pu_height", "bass.str.pu_height", 1.5, 5.0, 3.0, "mm", false, false),
    STR_PIEZO = ("str.piezo", "bass.str.piezo", 0.0, 1.0, 0.0, "%", false, false),
    STR_TONE = ("str.tone", "bass.str.tone", 0.0, 1.0, 1.0, "%", false, false),
    STR_HUMANIZE = ("str.humanize", "bass.str.humanize", 0.0, 1.0, 0.0, "%", false, false),

    // ---- sub (every engine; never filtered or driven) ----
    SUB_LEVEL = ("sub.level", "bass.sub.level", 0.0, 1.0, 0.0, "%", false, false),
    SUB_OCTAVE = ("sub.octave", "bass.sub.octave", 1.0, 2.0, 1.0, "oct", false, true),
    // 0 Sine · 1 Triangle · 2 Square
    SUB_WAVE = ("sub.wave", "bass.sub.wave", 0.0, 2.0, 0.0, "", false, true),

    // ---- drive and output ----
    DRIVE = ("drive", "bass.drive", 0.0, 1.0, 0.0, "%", false, false),
    // 0 Tube · 1 Tape · 2 Fold · 3 Hard
    DRIVE_TYPE = ("drive.type", "bass.drive_type", 0.0, 3.0, 0.0, "", false, true),
    DRIVE_KEYTRACK = ("drive.keytrack", "bass.drive_keytrack", -6.0, 6.0, 3.0, "dB/oct", false, false),
    DRIVE_REF = ("drive.ref", "bass.drive_ref", 12.0, 60.0, 36.0, "", false, true),
    DRIVE_LOCK = ("drive.lock", "bass.drive_lock", 0.0, 1.0, 1.0, "", false, true),
    SUBSONIC = ("subsonic", "bass.subsonic", 0.0, 40.0, 0.0, "Hz", false, false),
    CLIP = ("clip", "bass.clip", 0.0, 1.0, 0.0, "", false, true),
    // 0 None · 1 The project's kick (its notes) · 2 Sidechain (audio)
    DUCK_SOURCE = ("duck.source", "bass.duck_source", 0.0, 2.0, 0.0, "", false, true),
    DUCK_DEPTH = ("duck.depth", "bass.duck_depth", 0.0, 40.0, 6.0, "dB", false, false),
    DUCK_PRE = ("duck.pre", "bass.duck_pre", 0.0, 10.0, 2.0, "ms", false, false),
    DUCK_HOLD = ("duck.hold", "bass.duck_hold", 0.0, 200.0, 20.0, "ms", false, false),
    DUCK_RELEASE = ("duck.release", "bass.duck_release", 10.0, 500.0, 90.0, "ms", true, false),
    // 0 Linear · 1 Exponential · 2 S-curve
    DUCK_SHAPE = ("duck.shape", "bass.duck_shape", 0.0, 2.0, 1.0, "", false, true),
    // 0 Off · 1 Snap notes to the song's scale · 2 Roots only
    KEY_SNAP = ("key.snap", "bass.key_snap", 0.0, 2.0, 0.0, "", false, true),
    GAIN = ("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
}

/// How many parameters there are: the length of the engine's value array.
pub const COUNT: usize = PARAMS.len();

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`): the amplitude
/// envelope, the filter / modulation envelope, one LFO, the note's velocity, key and seeded
/// random value, the wheel, pressure, bend, expression and the eight macros.
pub const SOURCES: &[&str] = &[
    "env1", "env2", "lfo1", "vel", "key", "rnd", "mw", "at", "pb", "expr", "macro1", "macro2",
    "macro3", "macro4", "macro5", "macro6", "macro7", "macro8",
];

pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    find(PARAMS, key).or_else(|| super::modulation::param(key))
}

/// The index of a key in [`PARAMS`].
pub fn index(key: &str) -> Option<usize> {
    PARAMS.iter().position(|i| i.key == key)
}

pub fn is_text_key(key: &str) -> bool {
    super::modulation::is_text_key(key)
}

pub fn is_asset_key(key: &str) -> bool {
    let _ = key;
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_table_follows_the_list() {
        assert_eq!(PARAMS[ix::ENGINE].key, "engine");
        assert_eq!(PARAMS[ix::E808_PITCH].key, "808.pitch");
        assert_eq!(PARAMS[ix::SYN_CUTOFF].key, "syn.cutoff");
        assert_eq!(PARAMS[ix::STR_PU2].key, "str.pu2");
        assert_eq!(PARAMS[ix::GAIN].key, "gain");
        assert_eq!(ix::GAIN, COUNT - 1);
        for (i, info) in PARAMS.iter().enumerate() {
            assert_eq!(index(info.key), Some(i));
            assert!(
                info.min <= info.default && info.default <= info.max,
                "{}",
                info.key
            );
            if info.logarithmic {
                assert!(info.min > 0.0, "{} is logarithmic from zero", info.key);
            }
        }
    }

    #[test]
    fn effect_controls_default_to_neutral() {
        // Rule 4: drive, harmonics, sub, width, subsonic, clip, ducking and key snapping
        // change no sample at their defaults.
        for key in [
            "drive",
            "808.harmonics",
            "sub.level",
            "syn.width",
            "subsonic",
            "clip",
            "duck.source",
            "key.snap",
            "str.humanize",
        ] {
            assert_eq!(param(key).unwrap().default, 0.0, "{key}");
        }
    }
}
