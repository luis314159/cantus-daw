//! Fides — the resonator instrument (round 41, ADR 0150): its parameter model. Owned by
//! the `fides` builder. The DSP is `cantus_dsp::fides`, the unit
//! `cantus_engine::inst::fides`, the editor `cantus_ui::views::inst::fides`, and what each
//! parameter does is in `docs/manual/en/fides.md`.
//!
//! An exciter (pluck, pick, a bow by a friction curve, a mallet, noise, a live input) into
//! one or two resonators (a waveguide string or tube; modal bars, tines, plates, membranes,
//! drumheads, bells, glass and pans), coupled in parallel or in series, through a body:
//! the class of Ableton's Collision and Tension, Logic's Sculpture and AAS Chromaphone
//! (BACKLOG §3e item 6, §6l A2). The name is the Latin for a lyre's strings — a stringed
//! instrument — and for faith.
//!
//! Keys are forever once shipped. Units are the ones a musician reads; a unit of `%` is
//! stored as 0…1 (the knob prints it ×100). Stepped keys hold an index whose meaning is
//! listed beside it. The two resonators' keys are the same under `a.` and `b.`.

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.fides";
pub const ICON: TrackIcon = TrackIcon::Strings;
/// Offered in the instrument picker once it is built (the `fides` builder sets it).
pub const READY: bool = false;
/// The model version stored with every new instance (ADR 0059's rule): an improved model
/// is a new number, and a song keeps the one it was made with.
pub const MODEL: f32 = 1.0;

/// Set, never automated or modulated: the model a song was made with, and how many voices
/// the unit holds.
pub const SET_KEYS: [&str; 2] = ["model", "poly"];

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
    // ---- the model and the voice ----
    MODEL = ("model", "fides.model", 1.0, 1.0, MODEL, "", false, true),
    POLY = ("poly", "fides.poly", 1.0, 32.0, 16.0, "", false, true),
    // 0 Poly · 1 Mono · 2 Legato
    MODE = ("mode", "fides.mode", 0.0, 2.0, 0.0, "", false, true),
    // 0 A new voice · 1 The same resonator, struck again (it keeps ringing)
    RETRIG = ("retrig", "fides.retrig", 0.0, 1.0, 1.0, "", false, true),
    GLIDE = ("glide", "fides.glide", 0.0, 2000.0, 0.0, "ms", false, false),
    BEND = ("bend", "fides.bend", 0.0, 24.0, 2.0, "st", false, true),
    OCTAVE = ("octave", "fides.octave", -3.0, 3.0, 0.0, "oct", false, true),
    TRANSPOSE = ("transpose", "fides.transpose", -12.0, 12.0, 0.0, "st", false, true),
    FINE = ("fine", "fides.fine", -100.0, 100.0, 0.0, "ct", false, false),
    A4 = ("a4", "fides.a4", 415.0, 466.0, 440.0, "Hz", false, false),
    VELOCITY = ("velocity", "fides.velocity", 0.0, 1.0, 0.7, "%", false, false),
    // The resonators' ring after the key is let go (the shorter of this and their decay).
    RELEASE = ("release", "fides.release", 0.02, 60.0, 0.4, "s", true, false),
    VIB_DEPTH = ("vib.depth", "fides.vib_depth", 0.0, 100.0, 0.0, "ct", false, false),
    VIB_RATE = ("vib.rate", "fides.vib_rate", 0.5, 12.0, 5.5, "Hz", true, false),
    VIB_DELAY = ("vib.delay", "fides.vib_delay", 0.0, 2000.0, 300.0, "ms", false, false),
    // A tremolo on the whole instrument, its phase the song's clock (a vibraphone's motor).
    TREM_DEPTH = ("trem.depth", "fides.trem_depth", 0.0, 1.0, 0.0, "%", false, false),
    TREM_RATE = ("trem.rate", "fides.trem_rate", 0.5, 12.0, 5.0, "Hz", true, false),
    SEED = ("seed", "fides.seed", 0.0, 9999.0, 0.0, "", false, true),

    // ---- the exciter ----
    // 0 Pluck · 1 Pick · 2 Bow · 3 Mallet · 4 Noise · 5 Input (a track's audio)
    EXC = ("exc.type", "fides.exc.type", 0.0, 5.0, 0.0, "", false, true),
    EXC_LEVEL = ("exc.level", "fides.exc.level", -24.0, 12.0, 0.0, "dB", false, false),
    // How long the finger, the pick or the mallet touches, at velocity 100.
    EXC_CONTACT = ("exc.contact", "fides.exc.contact", 0.05, 20.0, 2.0, "ms", true, false),
    // How much a harder strike shortens the contact (brighter), after the power-law spring.
    EXC_VEL_TONE = ("exc.vel_tone", "fides.exc.vel_tone", 0.0, 1.0, 0.5, "%", false, false),
    // A low-pass on what the exciter sends (20 kHz: open).
    EXC_TONE = ("exc.tone", "fides.exc.tone", 200.0, 20000.0, 20000.0, "Hz", true, false),
    // The noise in the strike: a pick's scrape, a mallet's felt.
    EXC_NOISE = ("exc.noise", "fides.exc.noise", 0.0, 1.0, 0.0, "%", false, false),
    // Where it excites, as a fraction of the length from the end (a string's bridge, a
    // bar's end, a membrane's rim).
    EXC_POS = ("exc.pos", "fides.exc.pos", 0.01, 0.5, 0.13, "%", false, false),
    BOW_PRESSURE = ("bow.pressure", "fides.bow.pressure", 0.0, 1.0, 0.5, "%", false, false),
    BOW_SPEED = ("bow.speed", "fides.bow.speed", 0.0, 1.0, 0.5, "%", false, false),
    BOW_ATTACK = ("bow.attack", "fides.bow.attack", 1.0, 3000.0, 120.0, "ms", true, false),
    BOW_RELEASE = ("bow.release", "fides.bow.release", 5.0, 3000.0, 200.0, "ms", true, false),
    // The noise generator: the Noise exciter plays it at full, and it lies under any other.
    NOISE_LEVEL = ("noise.level", "fides.noise.level", 0.0, 1.0, 0.0, "%", false, false),
    NOISE_COLOR = ("noise.color", "fides.noise.color", 50.0, 16000.0, 2000.0, "Hz", true, false),
    NOISE_WIDTH = ("noise.width", "fides.noise.width", 0.1, 8.0, 3.0, "oct", true, false),
    // 1 continuous … 0 sparse grains (rain, crackle).
    NOISE_DENSITY = ("noise.density", "fides.noise.density", 0.0, 1.0, 1.0, "%", false, false),
    NOISE_ATTACK = ("noise.attack", "fides.noise.attack", 0.5, 5000.0, 5.0, "ms", true, false),
    NOISE_DECAY = ("noise.decay", "fides.noise.decay", 1.0, 10000.0, 200.0, "ms", true, false),
    NOISE_SUSTAIN = ("noise.sustain", "fides.noise.sustain", 0.0, 1.0, 0.0, "%", false, false),
    NOISE_RELEASE = ("noise.release", "fides.noise.release", 1.0, 10000.0, 100.0, "ms", true, false),
    // MPE: pressure on the bow's pressure (the exciter's level otherwise), slide on where
    // it excites.
    MPE_PRESS = ("mpe.press", "fides.mpe.press", 0.0, 1.0, 0.5, "%", false, false),
    MPE_SLIDE = ("mpe.slide", "fides.mpe.slide", 0.0, 1.0, 0.5, "%", false, false),

    // ---- resonator A ----
    // 0 String · 1 Bar · 2 Marimba · 3 Tine · 4 Plate · 5 Membrane · 6 Drumhead ·
    // 7 Tube (closed) · 8 Pipe (open) · 9 Bell · 10 Glass · 11 Pan
    A_TYPE = ("a.type", "fides.res.type", 0.0, 11.0, 0.0, "", false, true),
    A_DECAY = ("a.decay", "fides.res.decay", 0.02, 60.0, 4.0, "s", true, false),
    // Wood (the highs die fast) … glass and metal (they ring as long as the fundamental).
    A_MATERIAL = ("a.material", "fides.res.material", 0.0, 1.0, 0.45, "%", false, false),
    // Decay across the keys: how much faster a note an octave up dies.
    A_DECAY_KEY = ("a.decay_key", "fides.res.decay_key", 0.0, 1.0, 0.3, "%", false, false),
    // Stretches (+) or compresses (−) the partials; a string's stiffness.
    A_INHARM = ("a.inharm", "fides.res.inharm", -1.0, 1.0, 0.0, "%", false, false),
    A_LISTEN = ("a.listen", "fides.res.listen", 0.01, 0.5, 0.37, "%", false, false),
    A_MODES = ("a.modes", "fides.res.modes", 1.0, 48.0, 32.0, "", false, true),
    A_TUNE = ("a.tune", "fides.res.tune", -24.0, 24.0, 0.0, "st", false, false),
    A_FINE = ("a.fine", "fides.res.fine", -100.0, 100.0, 0.0, "ct", false, false),
    A_KEYTRACK = ("a.keytrack", "fides.res.keytrack", 0.0, 1.0, 1.0, "%", false, false),
    A_LEVEL = ("a.level", "fides.res.level", -36.0, 12.0, 0.0, "dB", false, false),

    // ---- resonator B ----
    B_ON = ("b.on", "fides.b.on", 0.0, 1.0, 0.0, "", false, true),
    B_TYPE = ("b.type", "fides.res.type", 0.0, 11.0, 7.0, "", false, true),
    B_DECAY = ("b.decay", "fides.res.decay", 0.02, 60.0, 1.0, "s", true, false),
    B_MATERIAL = ("b.material", "fides.res.material", 0.0, 1.0, 0.45, "%", false, false),
    B_DECAY_KEY = ("b.decay_key", "fides.res.decay_key", 0.0, 1.0, 0.3, "%", false, false),
    B_INHARM = ("b.inharm", "fides.res.inharm", -1.0, 1.0, 0.0, "%", false, false),
    B_LISTEN = ("b.listen", "fides.res.listen", 0.01, 0.5, 0.37, "%", false, false),
    B_MODES = ("b.modes", "fides.res.modes", 1.0, 48.0, 32.0, "", false, true),
    B_TUNE = ("b.tune", "fides.res.tune", -24.0, 24.0, 0.0, "st", false, false),
    B_FINE = ("b.fine", "fides.res.fine", -100.0, 100.0, 0.0, "ct", false, false),
    B_KEYTRACK = ("b.keytrack", "fides.res.keytrack", 0.0, 1.0, 1.0, "%", false, false),
    B_LEVEL = ("b.level", "fides.res.level", -36.0, 12.0, 0.0, "dB", false, false),
    // 0 Parallel (the exciter into both) · 1 Serial (A into B, as a marimba's bar into
    // its tube)
    COUPLE = ("couple", "fides.couple", 0.0, 1.0, 0.0, "", false, true),
    // What is heard of A … of B.
    COUPLE_MIX = ("couple.mix", "fides.couple.mix", 0.0, 1.0, 0.5, "%", false, false),

    // ---- the body ----
    // 0 None · 1 Guitar · 2 Violin · 3 Cello · 4 Harp · 5 Box · 6 Shell
    BODY = ("body.type", "fides.body.type", 0.0, 6.0, 1.0, "", false, true),
    BODY_MIX = ("body.mix", "fides.body.mix", 0.0, 1.0, 0.6, "%", false, false),
    BODY_SIZE = ("body.size", "fides.body.size", 0.5, 2.0, 1.0, "%", true, false),
    BODY_DECAY = ("body.decay", "fides.body.decay", 0.25, 4.0, 1.0, "%", true, false),

    // ---- the output ----
    WIDTH = ("width", "fides.width", 0.0, 1.0, 0.4, "%", false, false),
    // 10 Hz: off.
    LOWCUT = ("lowcut", "fides.lowcut", 10.0, 1000.0, 10.0, "Hz", true, false),
    // 20 kHz: off.
    HIGHCUT = ("highcut", "fides.highcut", 1000.0, 20000.0, 20000.0, "Hz", true, false),
    GAIN = ("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
}

/// How many parameters there are: the length of the engine's value array.
pub const COUNT: usize = PARAMS.len();

/// The exciter types, in `exc.type`'s order (locale keys `fides.exc.<name>`).
pub const EXCITERS: [&str; 6] = ["pluck", "pick", "bow", "mallet", "noise", "input"];
/// The resonator types, in `a.type`'s order (locale keys `fides.res.<name>`).
pub const RESONATORS: [&str; 12] = [
    "string", "bar", "marimba", "tine", "plate", "membrane", "drumhead", "tube", "pipe", "bell",
    "glass", "pan",
];
/// The bodies, in `body.type`'s order (locale keys `fides.body.<name>`).
pub const BODIES: [&str; 7] = ["none", "guitar", "violin", "cello", "harp", "box", "shell"];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`): the exciter's
/// envelope (a bow's or the noise's), the vibrato LFO, the note's velocity, key and seeded
/// random value, MPE's bend, pressure and slide, the macros and the controllers. The order
/// is the engine's source array's.
pub const SOURCES: &[&str] = &[
    "env1",
    "lfo1",
    "vel",
    "key",
    "rnd",
    "mpe.bend",
    "mpe.press",
    "mpe.slide",
    "macro1",
    "macro2",
    "macro3",
    "macro4",
    "macro5",
    "macro6",
    "macro7",
    "macro8",
    "mw",
    "at",
    "pb",
    "expr",
    "sus",
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
        assert_eq!(PARAMS[ix::MODEL].key, "model");
        assert_eq!(PARAMS[ix::EXC].key, "exc.type");
        assert_eq!(PARAMS[ix::A_TYPE].key, "a.type");
        assert_eq!(PARAMS[ix::B_LEVEL].key, "b.level");
        assert_eq!(PARAMS[ix::BODY].key, "body.type");
        assert_eq!(PARAMS[ix::GAIN].key, "gain");
        assert_eq!(ix::GAIN, COUNT - 1);
        // The two resonators' keys are the same after their prefix, in the same order.
        for k in 0..=(ix::A_LEVEL - ix::A_TYPE) {
            let (a, b) = (PARAMS[ix::A_TYPE + k], PARAMS[ix::B_TYPE + k]);
            assert_eq!(a.key[2..], b.key[2..]);
            assert_eq!(
                (a.min, a.max, a.unit, a.label_key),
                (b.min, b.max, b.unit, b.label_key)
            );
        }
        assert_eq!(PARAMS[ix::EXC].max as usize + 1, EXCITERS.len());
        assert_eq!(PARAMS[ix::A_TYPE].max as usize + 1, RESONATORS.len());
        assert_eq!(PARAMS[ix::BODY].max as usize + 1, BODIES.len());
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
        for k in SET_KEYS {
            assert!(index(k).is_some(), "{k}");
        }
    }

    #[test]
    fn effect_controls_default_to_neutral() {
        // Rule 4: the extras change no sample at their defaults.
        for key in [
            "vib.depth",
            "trem.depth",
            "exc.noise",
            "noise.level",
            "b.on",
            "glide",
        ] {
            assert_eq!(param(key).unwrap().default, 0.0, "{key}");
        }
        assert_eq!(param("exc.tone").unwrap().default, 20000.0);
        assert_eq!(param("lowcut").unwrap().default, 10.0);
        assert_eq!(param("highcut").unwrap().default, 20000.0);
    }
}
