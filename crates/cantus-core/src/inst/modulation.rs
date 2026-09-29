//! The modulation model every native instrument shares (ADR 0023,
//! `research/instruments/family.md` §6.6): eight macros and a matrix of connections, stored
//! in the instrument's own [`NativeSpec`](super::NativeSpec) like any other value, so a
//! connection is project data with undo, automation and the AI bridge for free.
//!
//! | Key | What | Range |
//! |---|---|---|
//! | `macro{m}` | macro *m* (1…8) | 0 … 1 |
//! | `macro{m}.name` | its name (text) | |
//! | `mod{n}.src` | slot *n* (1…32): the source's tag, e.g. `env2`, `lfo1`, `vel`, `macro3` (text) | [`SOURCES`] |
//! | `mod{n}.dst` | the target parameter's key (text) | |
//! | `mod{n}.amt` | depth, as a fraction of the target's range; negative inverts | −1 … 1 |
//! | `mod{n}.via` | a source whose value scales the depth (text, optional) | |
//! | `mod{n}.curve` | linear, exponential, logarithmic, S, steps | 0 … 4 |
//! | `mod{n}.on` | the connection is live | 0 / 1 |
//!
//! A slot without both a source and a target is empty. Each instrument says which source
//! tags it produces (`sources()` in its module); the editor offers only those, and a unit
//! ignores a tag it does not know.
//!
//! **Generated** by the round-17 scaffold script: the key tables are constants so that a
//! unit's `configure`, on the audio thread, reads them without formatting a string.

use super::InstParamInfo;
use crate::effect::p;

pub const MACROS: usize = 8;
pub const MOD_SLOTS: usize = 32;

/// Whether a source has one value per voice or one for the instrument.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    Voice,
    Global,
}

/// A modulation source in the shared vocabulary.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub tag: &'static str,
    pub label_key: &'static str,
    pub scope: Scope,
    /// Swings −1…1 (an LFO, pitch bend) rather than 0…1 (an envelope, velocity).
    pub bipolar: bool,
}

const fn src(tag: &'static str, label_key: &'static str, scope: Scope, bipolar: bool) -> Source {
    Source {
        tag,
        label_key,
        scope,
        bipolar,
    }
}

/// Every source tag an instrument may produce, in the order the editor shows them.
pub const SOURCES: &[Source] = &[
    src("env1", "mod.src.env", Scope::Voice, false),
    src("env2", "mod.src.env", Scope::Voice, false),
    src("env3", "mod.src.env", Scope::Voice, false),
    src("env4", "mod.src.env", Scope::Voice, false),
    src("lfo1", "mod.src.lfo", Scope::Voice, true),
    src("lfo2", "mod.src.lfo", Scope::Voice, true),
    src("lfo3", "mod.src.lfo", Scope::Voice, true),
    src("lfo4", "mod.src.lfo", Scope::Voice, true),
    src("lfo5", "mod.src.lfo", Scope::Voice, true),
    src("lfo6", "mod.src.lfo", Scope::Voice, true),
    src("lfo7", "mod.src.lfo", Scope::Voice, true),
    src("lfo8", "mod.src.lfo", Scope::Voice, true),
    src("vel", "mod.src.vel", Scope::Voice, false),
    src("relvel", "mod.src.relvel", Scope::Voice, false),
    src("key", "mod.src.key", Scope::Voice, true),
    src("rnd", "mod.src.rnd", Scope::Voice, true),
    src("mpe.bend", "mod.src.mpe_bend", Scope::Voice, true),
    src("mpe.press", "mod.src.mpe_press", Scope::Voice, false),
    src("mpe.slide", "mod.src.mpe_slide", Scope::Voice, false),
    src("macro1", "mod.src.macro", Scope::Global, false),
    src("macro2", "mod.src.macro", Scope::Global, false),
    src("macro3", "mod.src.macro", Scope::Global, false),
    src("macro4", "mod.src.macro", Scope::Global, false),
    src("macro5", "mod.src.macro", Scope::Global, false),
    src("macro6", "mod.src.macro", Scope::Global, false),
    src("macro7", "mod.src.macro", Scope::Global, false),
    src("macro8", "mod.src.macro", Scope::Global, false),
    src("mw", "mod.src.mw", Scope::Global, false),
    src("at", "mod.src.at", Scope::Global, false),
    src("pb", "mod.src.pb", Scope::Global, true),
    src("expr", "mod.src.expr", Scope::Global, false),
    src("sus", "mod.src.sus", Scope::Global, false),
];

/// The keys of one connection slot, as constants.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SlotKeys {
    pub src: &'static str,
    pub dst: &'static str,
    pub amt: &'static str,
    pub via: &'static str,
    pub curve: &'static str,
    pub on: &'static str,
}

/// `SLOTS[n - 1]` are the keys of slot *n*.
pub const SLOTS: [SlotKeys; MOD_SLOTS] = [
    SlotKeys {
        src: "mod1.src",
        dst: "mod1.dst",
        amt: "mod1.amt",
        via: "mod1.via",
        curve: "mod1.curve",
        on: "mod1.on",
    },
    SlotKeys {
        src: "mod2.src",
        dst: "mod2.dst",
        amt: "mod2.amt",
        via: "mod2.via",
        curve: "mod2.curve",
        on: "mod2.on",
    },
    SlotKeys {
        src: "mod3.src",
        dst: "mod3.dst",
        amt: "mod3.amt",
        via: "mod3.via",
        curve: "mod3.curve",
        on: "mod3.on",
    },
    SlotKeys {
        src: "mod4.src",
        dst: "mod4.dst",
        amt: "mod4.amt",
        via: "mod4.via",
        curve: "mod4.curve",
        on: "mod4.on",
    },
    SlotKeys {
        src: "mod5.src",
        dst: "mod5.dst",
        amt: "mod5.amt",
        via: "mod5.via",
        curve: "mod5.curve",
        on: "mod5.on",
    },
    SlotKeys {
        src: "mod6.src",
        dst: "mod6.dst",
        amt: "mod6.amt",
        via: "mod6.via",
        curve: "mod6.curve",
        on: "mod6.on",
    },
    SlotKeys {
        src: "mod7.src",
        dst: "mod7.dst",
        amt: "mod7.amt",
        via: "mod7.via",
        curve: "mod7.curve",
        on: "mod7.on",
    },
    SlotKeys {
        src: "mod8.src",
        dst: "mod8.dst",
        amt: "mod8.amt",
        via: "mod8.via",
        curve: "mod8.curve",
        on: "mod8.on",
    },
    SlotKeys {
        src: "mod9.src",
        dst: "mod9.dst",
        amt: "mod9.amt",
        via: "mod9.via",
        curve: "mod9.curve",
        on: "mod9.on",
    },
    SlotKeys {
        src: "mod10.src",
        dst: "mod10.dst",
        amt: "mod10.amt",
        via: "mod10.via",
        curve: "mod10.curve",
        on: "mod10.on",
    },
    SlotKeys {
        src: "mod11.src",
        dst: "mod11.dst",
        amt: "mod11.amt",
        via: "mod11.via",
        curve: "mod11.curve",
        on: "mod11.on",
    },
    SlotKeys {
        src: "mod12.src",
        dst: "mod12.dst",
        amt: "mod12.amt",
        via: "mod12.via",
        curve: "mod12.curve",
        on: "mod12.on",
    },
    SlotKeys {
        src: "mod13.src",
        dst: "mod13.dst",
        amt: "mod13.amt",
        via: "mod13.via",
        curve: "mod13.curve",
        on: "mod13.on",
    },
    SlotKeys {
        src: "mod14.src",
        dst: "mod14.dst",
        amt: "mod14.amt",
        via: "mod14.via",
        curve: "mod14.curve",
        on: "mod14.on",
    },
    SlotKeys {
        src: "mod15.src",
        dst: "mod15.dst",
        amt: "mod15.amt",
        via: "mod15.via",
        curve: "mod15.curve",
        on: "mod15.on",
    },
    SlotKeys {
        src: "mod16.src",
        dst: "mod16.dst",
        amt: "mod16.amt",
        via: "mod16.via",
        curve: "mod16.curve",
        on: "mod16.on",
    },
    SlotKeys {
        src: "mod17.src",
        dst: "mod17.dst",
        amt: "mod17.amt",
        via: "mod17.via",
        curve: "mod17.curve",
        on: "mod17.on",
    },
    SlotKeys {
        src: "mod18.src",
        dst: "mod18.dst",
        amt: "mod18.amt",
        via: "mod18.via",
        curve: "mod18.curve",
        on: "mod18.on",
    },
    SlotKeys {
        src: "mod19.src",
        dst: "mod19.dst",
        amt: "mod19.amt",
        via: "mod19.via",
        curve: "mod19.curve",
        on: "mod19.on",
    },
    SlotKeys {
        src: "mod20.src",
        dst: "mod20.dst",
        amt: "mod20.amt",
        via: "mod20.via",
        curve: "mod20.curve",
        on: "mod20.on",
    },
    SlotKeys {
        src: "mod21.src",
        dst: "mod21.dst",
        amt: "mod21.amt",
        via: "mod21.via",
        curve: "mod21.curve",
        on: "mod21.on",
    },
    SlotKeys {
        src: "mod22.src",
        dst: "mod22.dst",
        amt: "mod22.amt",
        via: "mod22.via",
        curve: "mod22.curve",
        on: "mod22.on",
    },
    SlotKeys {
        src: "mod23.src",
        dst: "mod23.dst",
        amt: "mod23.amt",
        via: "mod23.via",
        curve: "mod23.curve",
        on: "mod23.on",
    },
    SlotKeys {
        src: "mod24.src",
        dst: "mod24.dst",
        amt: "mod24.amt",
        via: "mod24.via",
        curve: "mod24.curve",
        on: "mod24.on",
    },
    SlotKeys {
        src: "mod25.src",
        dst: "mod25.dst",
        amt: "mod25.amt",
        via: "mod25.via",
        curve: "mod25.curve",
        on: "mod25.on",
    },
    SlotKeys {
        src: "mod26.src",
        dst: "mod26.dst",
        amt: "mod26.amt",
        via: "mod26.via",
        curve: "mod26.curve",
        on: "mod26.on",
    },
    SlotKeys {
        src: "mod27.src",
        dst: "mod27.dst",
        amt: "mod27.amt",
        via: "mod27.via",
        curve: "mod27.curve",
        on: "mod27.on",
    },
    SlotKeys {
        src: "mod28.src",
        dst: "mod28.dst",
        amt: "mod28.amt",
        via: "mod28.via",
        curve: "mod28.curve",
        on: "mod28.on",
    },
    SlotKeys {
        src: "mod29.src",
        dst: "mod29.dst",
        amt: "mod29.amt",
        via: "mod29.via",
        curve: "mod29.curve",
        on: "mod29.on",
    },
    SlotKeys {
        src: "mod30.src",
        dst: "mod30.dst",
        amt: "mod30.amt",
        via: "mod30.via",
        curve: "mod30.curve",
        on: "mod30.on",
    },
    SlotKeys {
        src: "mod31.src",
        dst: "mod31.dst",
        amt: "mod31.amt",
        via: "mod31.via",
        curve: "mod31.curve",
        on: "mod31.on",
    },
    SlotKeys {
        src: "mod32.src",
        dst: "mod32.dst",
        amt: "mod32.amt",
        via: "mod32.via",
        curve: "mod32.curve",
        on: "mod32.on",
    },
];

/// `MACRO_KEYS[m - 1]` is macro *m*'s value key, `MACRO_NAME_KEYS[m - 1]` its name.
pub const MACRO_KEYS: [&str; MACROS] = [
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8",
];
pub const MACRO_NAME_KEYS: [&str; MACROS] = [
    "macro1.name",
    "macro2.name",
    "macro3.name",
    "macro4.name",
    "macro5.name",
    "macro6.name",
    "macro7.name",
    "macro8.name",
];

/// The numeric keys of the model: the macros, then each slot's depth, curve and switch.
pub const PARAMS: &[InstParamInfo] = &[
    p("macro1", "mod.macro", 0.0, 1.0, 0.0, "", false, false),
    p("macro2", "mod.macro", 0.0, 1.0, 0.0, "", false, false),
    p("macro3", "mod.macro", 0.0, 1.0, 0.0, "", false, false),
    p("macro4", "mod.macro", 0.0, 1.0, 0.0, "", false, false),
    p("macro5", "mod.macro", 0.0, 1.0, 0.0, "", false, false),
    p("macro6", "mod.macro", 0.0, 1.0, 0.0, "", false, false),
    p("macro7", "mod.macro", 0.0, 1.0, 0.0, "", false, false),
    p("macro8", "mod.macro", 0.0, 1.0, 0.0, "", false, false),
    p("mod1.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod1.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod1.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod2.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod2.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod2.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod3.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod3.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod3.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod4.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod4.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod4.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod5.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod5.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod5.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod6.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod6.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod6.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod7.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod7.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod7.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod8.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod8.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod8.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod9.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod9.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod9.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod10.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod10.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod10.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod11.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod11.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod11.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod12.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod12.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod12.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod13.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod13.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod13.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod14.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod14.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod14.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod15.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod15.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod15.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod16.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod16.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod16.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod17.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod17.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod17.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod18.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod18.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod18.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod19.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod19.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod19.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod20.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod20.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod20.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod21.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod21.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod21.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod22.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod22.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod22.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod23.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod23.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod23.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod24.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod24.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod24.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod25.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod25.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod25.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod26.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod26.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod26.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod27.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod27.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod27.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod28.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod28.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod28.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod29.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod29.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod29.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod30.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod30.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod30.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod31.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod31.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod31.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
    p("mod32.amt", "mod.amount", -1.0, 1.0, 0.0, "", false, false),
    p("mod32.curve", "mod.curve", 0.0, 4.0, 0.0, "", false, true),
    p("mod32.on", "mod.on", 0.0, 1.0, 1.0, "", false, true),
];

/// The description of a numeric key of the model, or `None` when `key` is not one.
pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    if !(key.starts_with("macro") || key.starts_with("mod")) {
        return None;
    }
    PARAMS.iter().find(|i| i.key == key)
}

/// Whether `key` is a text key of the model (a macro's name, a slot's source, target or via).
pub fn is_text_key(key: &str) -> bool {
    MACRO_NAME_KEYS.contains(&key)
        || SLOTS
            .iter()
            .any(|s| s.src == key || s.dst == key || s.via == key)
}

pub fn source(tag: &str) -> Option<&'static Source> {
    SOURCES.iter().find(|s| s.tag == tag)
}

/// How a source's value is shaped before it is scaled by the depth.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Curve {
    Linear,
    Exp,
    Log,
    S,
    Steps,
}

impl Curve {
    pub fn from_index(i: f32) -> Curve {
        match i.round() as i32 {
            1 => Curve::Exp,
            2 => Curve::Log,
            3 => Curve::S,
            4 => Curve::Steps,
            _ => Curve::Linear,
        }
    }
}

/// One live connection, read from a spec without allocating.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Connection<'a> {
    /// 1-based slot number.
    pub slot: usize,
    pub src: &'a str,
    pub dst: &'a str,
    pub amount: f32,
    pub via: Option<&'a str>,
    pub curve: Curve,
    pub on: bool,
}

/// Every slot that has both a source and a target, in slot order — muted ones included
/// (`on` says which), because the editor shows them.
pub fn connections(spec: &super::NativeSpec) -> impl Iterator<Item = Connection<'_>> {
    SLOTS.iter().enumerate().filter_map(move |(i, k)| {
        Some(Connection {
            slot: i + 1,
            src: spec.text(k.src)?,
            dst: spec.text(k.dst)?,
            amount: spec.get(k.amt),
            via: spec.text(k.via),
            curve: Curve::from_index(spec.get(k.curve)),
            on: spec.get(k.on) >= 0.5,
        })
    })
}

/// The first slot (1-based) with neither a source nor a target, if any.
pub fn free_slot(spec: &super::NativeSpec) -> Option<usize> {
    SLOTS
        .iter()
        .position(|k| spec.text(k.src).is_none() && spec.text(k.dst).is_none())
        .map(|i| i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inst::{InstrumentKind, NativeSpec};

    #[test]
    fn tables_are_consistent() {
        assert_eq!(SLOTS[0].src, "mod1.src");
        assert_eq!(SLOTS[MOD_SLOTS - 1].on, format!("mod{MOD_SLOTS}.on"));
        let mut tags = std::collections::HashSet::new();
        for s in SOURCES {
            assert!(tags.insert(s.tag), "{} twice", s.tag);
        }
        for i in PARAMS {
            assert!(!is_text_key(i.key), "{}", i.key);
        }
    }

    #[test]
    fn a_connection_is_two_texts_and_a_depth() {
        let mut s = NativeSpec::new(InstrumentKind::Wavetable);
        assert_eq!(free_slot(&s), Some(1));
        s.set_text("mod1.src", Some("lfo1".into())).unwrap();
        assert_eq!(
            connections(&s).count(),
            0,
            "a source alone is not a connection"
        );
        s.set_text("mod1.dst", Some("gain".into())).unwrap();
        s.set("mod1.amt", -0.5).unwrap();
        let c: Vec<_> = connections(&s).collect();
        assert_eq!(c.len(), 1);
        assert_eq!(
            (c[0].slot, c[0].src, c[0].dst, c[0].amount),
            (1, "lfo1", "gain", -0.5)
        );
        assert!(c[0].on);
        assert_eq!(free_slot(&s), Some(2));
    }
}
