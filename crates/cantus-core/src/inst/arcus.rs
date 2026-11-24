//! Arcus — the bowed string (round 42, ADR 0157): its parameter model. Owned by the `arcus`
//! builder. The DSP is `cantus_dsp::arcus`, the unit `cantus_engine::inst::arcus`, the editor
//! `cantus_ui::views::inst::arcus`, and what each parameter does is in
//! `docs/manual/en/arcus.md`.
//!
//! A string in physical units (its tension, mass and length) bowed by a force in newtons at a
//! speed and a distance from the bridge, met by friction, through Fides's body: the cello and
//! the double bass first (arco, legato, détaché, pizzicato, tremolo, a vibrato whose partials
//! move through the body), the violin and the viola in the same engine (BACKLOG §3e, *Strings
//! and winds*). The name is the Latin for the bow.
//!
//! Keys are forever once shipped. Units are the ones a musician reads; a unit of `%` is
//! stored as 0…1 (the knob prints it ×100). Stepped keys hold an index whose meaning is
//! listed beside it.

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.arcus";
pub const ICON: TrackIcon = TrackIcon::Strings;
/// Offered in the instrument picker once the owner has heard it (the `arcus` builder sets it
/// in the commit that adds it to `InstrumentKind::ALL`).
pub const READY: bool = false;
/// The model version stored with every new instance (ADR 0059's rule): an improved model
/// is a new number, and a song keeps the one it was made with. 1: round 42's first; 2:
/// matched to the recordings (`docs/round42/arcus-match.md`).
pub const MODEL: f32 = 2.0;

/// Set, never automated or modulated: the model a song was made with, and how many voices
/// the unit holds.
pub const SET_KEYS: [&str; 2] = ["model", "poly"];

/// Defines [`PARAMS`] and, in the same order, the index constants of [`ix`].
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
    // ---- the model, the instrument, the voice ----
    MODEL = ("model", "arcus.model", 1.0, 2.0, MODEL, "", false, true),
    // 0 Cello · 1 Double bass · 2 Violin · 3 Viola
    INST = ("inst", "arcus.inst", 0.0, 3.0, 0.0, "", false, true),
    // 0 Arco · 1 Pizzicato · 2 Tremolo
    TECH = ("tech", "arcus.tech", 0.0, 2.0, 0.0, "", false, true),
    POLY = ("poly", "arcus.poly", 1.0, 8.0, 4.0, "", false, true),
    // 0 Poly (double stops) · 1 Mono (every note re-bowed) · 2 Legato (overlaps slur)
    MODE = ("mode", "arcus.mode", 0.0, 2.0, 2.0, "", false, true),
    FINGER = ("finger", "arcus.finger", 0.0, 30.0, 3.0, "ms", false, false),
    // 0: no portamento (only when asked, here or by CC65).
    PORTA = ("porta", "arcus.porta", 0.0, 1000.0, 0.0, "ms", false, false),
    BEND = ("bend", "arcus.bend", 0.0, 24.0, 2.0, "st", false, true),
    OCTAVE = ("octave", "arcus.octave", -2.0, 2.0, 0.0, "", false, true),
    TRANSPOSE = ("transpose", "arcus.transpose", -12.0, 12.0, 0.0, "st", false, true),
    FINE = ("fine", "arcus.fine", -100.0, 100.0, 0.0, "ct", false, false),
    SEED = ("seed", "arcus.seed", 0.0, 9999.0, 0.0, "", false, true),

    // ---- the dynamic ----
    DYN = ("dyn", "arcus.dyn", 0.0, 1.0, 0.6, "%", false, false),
    VEL_DYN = ("vel.dyn", "arcus.vel.dyn", 0.0, 1.0, 1.0, "%", false, false),
    VEL_ATTACK = ("vel.attack", "arcus.vel.attack", 0.0, 1.0, 0.5, "%", false, false),
    // 0 Auto (CC1, breath, aftertouch; CC11 scales) · 1 CC1 · 2 CC11 · 3 Breath · 4 Aftertouch
    // · 5 Velocity only
    DYN_SRC = ("dyn.src", "arcus.dyn.src", 0.0, 5.0, 0.0, "", false, true),

    // ---- the bow ----
    // × the dynamic's own distance from the bridge (under 1 nearer the bridge, over 1 towards
    // the fingerboard), and × its own force: the read-out shows both beside β and newtons.
    BOW_POS = ("bow.pos", "arcus.bow.pos", 0.7, 1.4, 1.0, "×", true, false),
    BOW_FORCE = ("bow.force", "arcus.bow.force", 0.5, 2.0, 1.0, "×", true, false),
    BOW_SPEED = ("bow.speed", "arcus.bow.speed", 0.5, 2.0, 1.0, "%", true, false),
    BOW_ATTACK = ("bow.attack", "arcus.bow.attack", 0.25, 4.0, 1.0, "%", true, false),
    BOW_RELEASE = ("bow.release", "arcus.bow.release", 5.0, 2000.0, 120.0, "ms", true, false),
    // −60 dB: off.
    BOW_NOISE = ("bow.noise", "arcus.bow.noise", -60.0, 12.0, 0.0, "dB", false, false),
    // 0 Hyperbolic · 1 Thermal
    FRICTION = ("bow.friction", "arcus.bow.friction", 0.0, 1.0, 0.0, "", false, true),
    // 0 Alternating · 1 Always down · 2 Always up
    BOW_DIR = ("bow.dir", "arcus.bow.dir", 0.0, 2.0, 0.0, "", false, true),
    BOW_CHANGE = ("bow.change", "arcus.bow.change", 5.0, 80.0, 25.0, "ms", false, false),
    TREM_RATE = ("trem.rate", "arcus.trem.rate", 4.0, 24.0, 14.0, "Hz", false, false),

    // ---- the vibrato ----
    VIB_DEPTH = ("vib.depth", "arcus.vib.depth", 0.0, 2.0, 1.0, "%", false, false),
    VIB_RATE = ("vib.rate", "arcus.vib.rate", 0.5, 2.0, 1.0, "%", true, false),
    VIB_DELAY = ("vib.delay", "arcus.vib.delay", 0.0, 2000.0, 250.0, "ms", false, false),
    VIB_ATTACK = ("vib.attack", "arcus.vib.attack", 1.0, 2000.0, 450.0, "ms", true, false),
    VIB_VAR = ("vib.var", "arcus.vib.var", 0.0, 1.0, 0.25, "%", false, false),
    // 0 CC21 · 1 CC2 (breath) · 2 Aftertouch · 3 Automatic only
    VIB_SRC = ("vib.src", "arcus.vib.src", 0.0, 3.0, 0.0, "", false, true),

    // ---- pizzicato and the string ----
    PIZZ_POS = ("pizz.pos", "arcus.pizz.pos", 0.05, 0.5, 0.22, "%", false, false),
    PIZZ_SOFT = ("pizz.soft", "arcus.pizz.soft", 0.2, 5.0, 1.2, "ms", true, false),
    PIZZ_DECAY = ("pizz.decay", "arcus.pizz.decay", 0.25, 4.0, 1.0, "%", true, false),
    DECAY = ("decay", "arcus.decay", 0.25, 4.0, 1.0, "%", true, false),
    RING = ("ring", "arcus.ring", 0.05, 5.0, 0.8, "s", true, false),

    // ---- MPE ----
    MPE_PRESS = ("mpe.press", "arcus.mpe.press", 0.0, 1.0, 1.0, "%", false, false),
    MPE_SLIDE = ("mpe.slide", "arcus.mpe.slide", 0.0, 1.0, 0.5, "%", false, false),

    // ---- the body and the output ----
    BODY_MIX = ("body.mix", "arcus.body.mix", 0.0, 1.0, 1.0, "%", false, false),
    BODY_SIZE = ("body.size", "arcus.body.size", 0.8, 1.25, 1.0, "%", true, false),
    BODY_RING = ("body.ring", "arcus.body.ring", 0.25, 4.0, 1.0, "%", true, false),
    // 10 Hz: off.
    LOWCUT = ("lowcut", "arcus.lowcut", 10.0, 1000.0, 10.0, "Hz", true, false),
    // 20 kHz: off.
    HIGHCUT = ("highcut", "arcus.highcut", 1000.0, 20000.0, 20000.0, "Hz", true, false),
    GAIN = ("gain", "inst.gain", -48.0, 12.0, 0.0, "dB", false, false),
}

/// How many parameters there are: the length of the engine's value array.
pub const COUNT: usize = PARAMS.len();

/// The instruments, in `inst`'s order (locale keys `arcus.inst_kind.<name>`).
pub const INSTRUMENTS: [&str; 4] = ["cello", "bass", "violin", "viola"];
/// The techniques, in `tech`'s order (locale keys `arcus.tech_kind.<name>`).
pub const TECHNIQUES: [&str; 3] = ["arco", "pizz", "trem"];
/// The play modes, in `mode`'s order (locale keys `arcus.mode_kind.<name>`).
pub const MODES: [&str; 3] = ["poly", "mono", "legato"];
/// The dynamic's sources, in `dyn.src`'s order (locale keys `arcus.dyn_src_kind.<name>`).
pub const DYN_SOURCES: [&str; 6] = ["auto", "cc1", "cc11", "breath", "at", "vel"];
/// The vibrato's sources, in `vib.src`'s order (locale keys `arcus.vib_src_kind.<name>`).
pub const VIB_SOURCES: [&str; 4] = ["cc21", "cc2", "at", "auto"];
/// The friction laws, in `bow.friction`'s order (locale keys `arcus.friction_kind.<name>`).
pub const FRICTIONS: [&str; 2] = ["hyperbolic", "thermal"];
/// The bow's directions, in `bow.dir`'s order (locale keys `arcus.dir_kind.<name>`).
pub const DIRECTIONS: [&str; 3] = ["alt", "down", "up"];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// A new Arcus's four macros, named as a player reads them (Chorda's way): each mapped to
/// what it says — the expression (the dynamic), the contact point, the body, the vibrato.
pub const DEFAULT_MACROS: &[(&str, &str)] = &[
    ("macro1.name", "arcus.macro.expression"),
    ("mod1.src", "macro1"),
    ("mod1.dst", "dyn"),
    ("macro2.name", "arcus.macro.contact"),
    ("mod2.src", "macro2"),
    ("mod2.dst", "bow.pos"),
    ("macro3.name", "arcus.macro.body"),
    ("mod3.src", "macro3"),
    ("mod3.dst", "body.mix"),
    ("macro4.name", "arcus.macro.vibrato"),
    ("mod4.src", "macro4"),
    ("mod4.dst", "vib.depth"),
];
/// … and their depths (a macro at 0 changes nothing).
pub const DEFAULT_DEPTHS: &[(&str, f32)] = &[
    ("mod1.amt", 0.4),
    ("mod2.amt", 0.5),
    ("mod3.amt", -0.4),
    ("mod4.amt", 0.5),
];

/// The modulation sources it produces (tags from `modulation::SOURCES`): the macros and the
/// controllers (the unit evaluates every route on the control grid from these). The order
/// is the engine's source array's.
pub const SOURCES: &[&str] = &[
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "mw", "at",
    "pb", "expr", "sus",
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
        assert_eq!(PARAMS[ix::INST].key, "inst");
        assert_eq!(PARAMS[ix::GAIN].key, "gain");
        assert_eq!(ix::GAIN, COUNT - 1);
        assert_eq!(PARAMS[ix::INST].max as usize + 1, INSTRUMENTS.len());
        assert_eq!(PARAMS[ix::TECH].max as usize + 1, TECHNIQUES.len());
        assert_eq!(PARAMS[ix::MODE].max as usize + 1, MODES.len());
        assert_eq!(PARAMS[ix::DYN_SRC].max as usize + 1, DYN_SOURCES.len());
        assert_eq!(PARAMS[ix::VIB_SRC].max as usize + 1, VIB_SOURCES.len());
        assert_eq!(PARAMS[ix::FRICTION].max as usize + 1, FRICTIONS.len());
        assert_eq!(PARAMS[ix::BOW_DIR].max as usize + 1, DIRECTIONS.len());
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
    fn the_output_extras_default_to_neutral() {
        // The cuts at their ends touch no sample; portamento only when asked.
        assert_eq!(param("lowcut").unwrap().default, 10.0);
        assert_eq!(param("highcut").unwrap().default, 20000.0);
        assert_eq!(param("porta").unwrap().default, 0.0);
        assert_eq!(param("gain").unwrap().default, 0.0);
    }
}
