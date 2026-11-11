//! Aula, the convolution reverb (ADR 0112): its parameter model. Owned by Aula.
//!
//! Keys are forever once shipped (automation, saved projects and the exported plug-ins' ids
//! hold them). **Every default leaves the signal untouched, to the bit**: the mix starts at
//! 0 (an insert loads a starting preset that sounds, ADR 0071), and every control that
//! shapes the response starts where it leaves the response as it is.
//!
//! | Key | What | Range | Default |
//! |---|---|---|---|
//! | `ir` | Cantus's own response (`aula.ir.{i}`), when no file is loaded | 0 … 14 | concert hall |
//! | `predelay` | the wet's delay | 0 … 500 ms | 0 |
//! | `stretch` | the response stretched in time (a larger room, lower) | 50 … 200 % | 100 % |
//! | `length` | the part of the response kept | 2 … 100 % | 100 % |
//! | `attack` | the response faded in | 0 … 1 000 ms | 0 |
//! | `decay` | the tail's decay (50 % reaches −60 dB at the end) | 5 … 100 % | 100 % |
//! | `gate` | where the response falls this far under its peak it ends | −120 (off) … −12 dB | off |
//! | `low` | low cut, 12 dB/oct | 10 (off) … 1 000 Hz | off |
//! | `high` | high cut, 12 dB/oct | 1 … 22 kHz (off) | off |
//! | `damp` | the highs die away faster | 0 … 100 % | 0 |
//! | `width` | the wet's width (mid/side) | 0 … 200 % | 100 % |
//! | `reverse` | the response played backwards | off / on | off |
//! | `mix` | the dry and the wet crossfaded | 0 … 100 % | 0 |
//! | `level` | the output level | −24 … +24 dB | 0 |
//!
//! The response from a file is not a number: it is [`crate::EffectSpec::ir`] ([`IrFile`]),
//! set by [`crate::Command::SetEffectIr`]; while one is set, `ir` is not heard.

use crate::effect::{p, EffectParamInfo};
use serde::{Deserialize, Serialize};

pub const NAME_KEY: &str = "effect.aula";
pub const SPARSE: bool = false;
pub const TAKES_SIDECHAIN: bool = false;
pub const TAKES_NOTES: bool = false;
/// How many responses Cantus makes itself (`cantus_dsp::aula_gen::FACTORY`).
pub const FACTORY_COUNT: usize = 15;
/// The locale prefix of each stepped parameter's choices, by key.
pub const CHOICES: &[(&str, &str)] = &[("ir", "aula.ir"), ("reverse", "aula.onoff")];

pub const PARAMS: &[EffectParamInfo] = &[
    p(
        "ir",
        "fx.aula.ir",
        0.0,
        (FACTORY_COUNT - 1) as f32,
        7.0,
        "",
        false,
        true,
    ),
    p(
        "predelay",
        "fx.aula.predelay",
        0.0,
        500.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p(
        "stretch",
        "fx.aula.stretch",
        0.5,
        2.0,
        1.0,
        "%",
        true,
        false,
    ),
    p(
        "length",
        "fx.aula.length",
        0.02,
        1.0,
        1.0,
        "%",
        false,
        false,
    ),
    p(
        "attack",
        "fx.aula.attack",
        0.0,
        1_000.0,
        0.0,
        "ms",
        false,
        false,
    ),
    p("decay", "fx.aula.decay", 0.05, 1.0, 1.0, "%", false, false),
    p(
        "gate",
        "fx.aula.gate",
        -120.0,
        -12.0,
        -120.0,
        "dB",
        false,
        false,
    ),
    p("low", "fx.aula.low", 10.0, 1_000.0, 10.0, "Hz", true, false),
    p(
        "high",
        "fx.aula.high",
        1_000.0,
        22_000.0,
        22_000.0,
        "Hz",
        true,
        false,
    ),
    p("damp", "fx.aula.damp", 0.0, 1.0, 0.0, "%", false, false),
    p("width", "fx.aula.width", 0.0, 2.0, 1.0, "%", false, false),
    p("reverse", "fx.aula.reverse", 0.0, 1.0, 0.0, "", false, true),
    p("mix", "fx.aula.mix", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "level",
        "fx.aula.level",
        -24.0,
        24.0,
        0.0,
        "dB",
        false,
        false,
    ),
];

pub fn params() -> &'static [EffectParamInfo] {
    PARAMS
}

/// The keys that change the response itself (a new kernel, built off the audio thread);
/// the others the audio thread applies at once.
pub const SHAPE_KEYS: &[&str] = &[
    "ir", "stretch", "length", "attack", "decay", "gate", "low", "high", "damp", "reverse",
];

/// A response from a file (round 39): its path as the project keeps it — relative to the
/// project's folder when it is inside it, absolute otherwise — and, for one from a content
/// pack, the pack's id (its credit is shown with it, ADR 0042).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IrFile {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
}

impl IrFile {
    pub fn new(path: impl Into<String>) -> IrFile {
        IrFile {
            path: path.into(),
            pack: None,
        }
    }

    /// The name shown: the file's name without its folder or extension.
    pub fn name(&self) -> &str {
        let file = self
            .path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(self.path.as_str());
        match file.rfind('.') {
            Some(i) if i > 0 => &file[..i],
            _ => file,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_files_name_is_its_stem() {
        assert_eq!(IrFile::new("C:\\IRs\\Hall A.wav").name(), "Hall A");
        assert_eq!(IrFile::new("irs/plate.v2.wav").name(), "plate.v2");
        assert_eq!(IrFile::new("room").name(), "room");
    }

    #[test]
    fn every_shape_key_is_a_parameter() {
        for k in SHAPE_KEYS {
            assert!(PARAMS.iter().any(|p| p.key == *k), "{k}");
        }
    }
}
