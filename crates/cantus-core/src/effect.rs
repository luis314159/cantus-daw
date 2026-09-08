//! Native insert effects: their *parameters*, not their DSP (see `cantus-dsp`).
//!
//! Effects use a generic parameter model — a kind plus `key → f32` — so the UI, the command
//! layer and the AI bridge treat every effect (and, later, every plugin) the same way.

use crate::model::EffectId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EffectKind {
    Delay,
    Reverb,
}

/// Display/range metadata for one effect parameter.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EffectParamInfo {
    pub key: &'static str,
    pub label_key: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub unit: &'static str,
    pub logarithmic: bool,
    pub stepped: bool,
}

#[allow(clippy::too_many_arguments)]
const fn p(
    key: &'static str,
    label_key: &'static str,
    min: f32,
    max: f32,
    default: f32,
    unit: &'static str,
    logarithmic: bool,
    stepped: bool,
) -> EffectParamInfo {
    EffectParamInfo {
        key,
        label_key,
        min,
        max,
        default,
        unit,
        logarithmic,
        stepped,
    }
}

pub const DELAY_PARAMS: &[EffectParamInfo] = &[
    p(
        "time_ms",
        "fx.delay.time",
        1.0,
        2000.0,
        375.0,
        "ms",
        true,
        false,
    ),
    p(
        "feedback",
        "fx.delay.feedback",
        0.0,
        0.95,
        0.4,
        "",
        false,
        false,
    ),
    p(
        "tone_hz",
        "fx.delay.tone",
        200.0,
        20_000.0,
        6_000.0,
        "Hz",
        true,
        false,
    ),
    p(
        "ping_pong",
        "fx.delay.ping_pong",
        0.0,
        1.0,
        1.0,
        "",
        false,
        true,
    ),
    p("mix", "fx.mix", 0.0, 1.0, 0.3, "", false, false),
];

pub const REVERB_PARAMS: &[EffectParamInfo] = &[
    p("size", "fx.reverb.size", 0.0, 1.0, 0.7, "", false, false),
    p(
        "damping",
        "fx.reverb.damping",
        0.0,
        1.0,
        0.4,
        "",
        false,
        false,
    ),
    p("width", "fx.reverb.width", 0.0, 1.0, 1.0, "", false, false),
    p(
        "predelay_ms",
        "fx.reverb.predelay",
        0.0,
        100.0,
        10.0,
        "ms",
        false,
        false,
    ),
    p("mix", "fx.mix", 0.0, 1.0, 0.25, "", false, false),
];

impl EffectKind {
    pub const ALL: [EffectKind; 2] = [EffectKind::Delay, EffectKind::Reverb];

    pub fn name_key(self) -> &'static str {
        match self {
            EffectKind::Delay => "effect.delay",
            EffectKind::Reverb => "effect.reverb",
        }
    }

    pub fn params(self) -> &'static [EffectParamInfo] {
        match self {
            EffectKind::Delay => DELAY_PARAMS,
            EffectKind::Reverb => REVERB_PARAMS,
        }
    }

    pub fn param(self, key: &str) -> Option<&'static EffectParamInfo> {
        self.params().iter().find(|i| i.key == key)
    }
}

/// An effect's kind and parameter values. Unknown keys are ignored on load; missing keys
/// read as their default, so adding a parameter never breaks an old file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSpec {
    pub kind: EffectKind,
    pub params: BTreeMap<String, f32>,
}

impl EffectSpec {
    pub fn new(kind: EffectKind) -> Self {
        let params = kind
            .params()
            .iter()
            .map(|i| (i.key.to_string(), i.default))
            .collect();
        EffectSpec { kind, params }
    }

    pub fn get(&self, key: &str) -> f32 {
        match self.params.get(key) {
            Some(v) => *v,
            None => self.kind.param(key).map(|i| i.default).unwrap_or(0.0),
        }
    }

    /// Set a parameter, clamping to its range. `None` for a key the effect does not have.
    pub fn set(&mut self, key: &str, value: f32) -> Option<f32> {
        let info = self.kind.param(key)?;
        let v = if value.is_finite() {
            value.clamp(info.min, info.max)
        } else {
            info.default
        };
        let v = if info.stepped { v.round() } else { v };
        self.params.insert(key.to_string(), v);
        Some(v)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EffectSlot {
    pub id: EffectId,
    pub bypass: bool,
    pub spec: EffectSpec,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_clamping() {
        for kind in EffectKind::ALL {
            let mut s = EffectSpec::new(kind);
            for info in kind.params() {
                assert_eq!(s.get(info.key), info.default);
                assert_eq!(s.set(info.key, info.max + 1.0), Some(info.max));
                assert_eq!(s.set(info.key, f32::NAN), Some(info.default));
            }
            assert!(s.set("nope", 1.0).is_none());
        }
    }

    #[test]
    fn missing_key_reads_default() {
        let mut s = EffectSpec::new(EffectKind::Delay);
        s.params.remove("mix");
        assert_eq!(s.get("mix"), 0.3);
        let json = serde_json::to_string(&s).unwrap();
        let back: EffectSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, s);
    }
}
