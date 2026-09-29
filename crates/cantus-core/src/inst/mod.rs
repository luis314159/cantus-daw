//! The native instruments' parameter models, one module per instrument, so each instrument
//! is owned by its files (ADR 0023, the twin of ADR 0018 for effects).
//!
//! Every module exports the same things — `NAME_KEY`, `ICON`, `READY`, `SOURCES`,
//! `params()`, `param(key)`, `is_text_key(key)` and `is_asset_key(key)` — and
//! [`InstrumentKind`] dispatches to them. Every kind's `param` and `is_text_key` also accept
//! the shared [`modulation`] keys (macros and the connection matrix). Adding an instrument is a new module, a new variant and one line in
//! each dispatch.
//!
//! A native instrument's whole state is a [`NativeSpec`]: numbers by key (`key → f32`,
//! sparse — only what moved from its default is stored) and text by key (file paths,
//! names). Numbers are what the command bus, undo, the AI bridge, automation and a
//! CLAP/VST3 export all understand; text is for what is not a number. Keys are forever
//! once shipped: projects, presets and automation hold them.

use crate::effect::EffectParamInfo;
use crate::model::TrackIcon;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod analog;
pub mod bass;
pub mod epiano;
pub mod fm;
pub mod modulation;
pub mod organ;
pub mod sampler;
pub mod wavetable;

/// Metadata for one instrument parameter: the same shape as an effect's.
pub type InstParamInfo = EffectParamInfo;

/// Which native instrument a [`NativeSpec`] is. The variant names say what the instrument
/// *is*; the product names (Unda, Calor, …) live in the locale files.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum InstrumentKind {
    /// Unda: the wavetable / hybrid flagship.
    Wavetable,
    /// Calor: the analog-modelling polysynth.
    Analog,
    /// Modulus: FM and additive.
    Fm,
    /// Exemplar: the multisample sampler (SFZ, Decent Sampler, SF2).
    Sampler,
    /// Gravis: bass — 808, modelled electric, synth.
    Bass,
    /// Lamina: the modelled electric piano.
    ElectricPiano,
    /// Organum: the tonewheel organ and its rotary speaker.
    Organ,
}

macro_rules! dispatch {
    ($kind:expr, $m:ident :: $item:ident $( ( $($arg:expr),* ) )?) => {
        match $kind {
            InstrumentKind::Wavetable => wavetable::$item $( ( $($arg),* ) )?,
            InstrumentKind::Analog => analog::$item $( ( $($arg),* ) )?,
            InstrumentKind::Fm => fm::$item $( ( $($arg),* ) )?,
            InstrumentKind::Sampler => sampler::$item $( ( $($arg),* ) )?,
            InstrumentKind::Bass => bass::$item $( ( $($arg),* ) )?,
            InstrumentKind::ElectricPiano => epiano::$item $( ( $($arg),* ) )?,
            InstrumentKind::Organ => organ::$item $( ( $($arg),* ) )?,
        }
    };
}

impl InstrumentKind {
    pub const ALL: [InstrumentKind; 7] = [
        InstrumentKind::Wavetable,
        InstrumentKind::Analog,
        InstrumentKind::Fm,
        InstrumentKind::Sampler,
        InstrumentKind::Bass,
        InstrumentKind::ElectricPiano,
        InstrumentKind::Organ,
    ];

    pub fn name_key(self) -> &'static str {
        dispatch!(self, m::NAME_KEY)
    }

    /// The icon a track hosting it derives when none was chosen.
    pub fn icon(self) -> TrackIcon {
        dispatch!(self, m::ICON)
    }

    /// Whether it is finished enough to be offered in the instrument picker. A scaffold is
    /// in the code (so every dispatch exists from the first day) but not in the menu.
    pub fn ready(self) -> bool {
        dispatch!(self, m::READY)
    }

    /// Every parameter the generic editors list (knobs, automation, a plug-in export). A
    /// kind with a structured space (zones, matrix slots) may accept more keys through
    /// [`param`](Self::param) than it lists here.
    pub fn params(self) -> &'static [InstParamInfo] {
        dispatch!(self, m::params())
    }

    /// The description of one numeric key, or `None` when the kind has no such key.
    pub fn param(self, key: &str) -> Option<&'static InstParamInfo> {
        dispatch!(self, m::param(key))
    }

    /// The modulation sources it produces, as tags of [`modulation::SOURCES`].
    pub fn sources(self) -> &'static [&'static str] {
        dispatch!(self, m::SOURCES)
    }

    /// Whether `key` holds text (a path, a name).
    pub fn is_text_key(self, key: &str) -> bool {
        dispatch!(self, m::is_text_key(key))
    }

    /// Whether a text `key` holds the path of an audio file the engine decodes and hands
    /// the instrument (a sample, a wavetable). Implies [`is_text_key`](Self::is_text_key).
    pub fn is_asset_key(self, key: &str) -> bool {
        dispatch!(self, m::is_asset_key(key))
    }

    /// The kinds offered in the picker, in menu order.
    pub fn ready_kinds() -> impl Iterator<Item = InstrumentKind> {
        Self::ALL.into_iter().filter(|k| k.ready())
    }
}

/// Find `key` in a static list — what most kinds' `param` is.
pub fn find(list: &'static [InstParamInfo], key: &str) -> Option<&'static InstParamInfo> {
    list.iter().find(|i| i.key == key)
}

/// The whole state of one native instrument on a track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NativeSpec {
    pub kind: InstrumentKind,
    /// Only the numbers that differ from their defaults are stored.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, f32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub text: BTreeMap<String, String>,
}

impl NativeSpec {
    pub fn new(kind: InstrumentKind) -> Self {
        NativeSpec {
            kind,
            params: BTreeMap::new(),
            text: BTreeMap::new(),
        }
    }

    /// The value of a numeric key: stored, or its default, or 0 for an unknown key.
    pub fn get(&self, key: &str) -> f32 {
        match self.params.get(key) {
            Some(v) => *v,
            None => self.kind.param(key).map(|i| i.default).unwrap_or(0.0),
        }
    }

    /// Whether the value is stored (moved from its default at some point).
    pub fn is_stored(&self, key: &str) -> bool {
        self.params.contains_key(key)
    }

    /// Set a numeric key, clamping to its range (a non-finite value becomes the default).
    /// Returns the value stored, or `None` for a key the kind does not have.
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

    /// Forget a stored value, so the key reads its default again. Returns what was stored.
    pub fn clear(&mut self, key: &str) -> Option<f32> {
        self.params.remove(key)
    }

    pub fn text(&self, key: &str) -> Option<&str> {
        self.text.get(key).map(String::as_str)
    }

    /// Set or remove a text key. `Err(())` for a key the kind does not hold as text;
    /// `Ok(previous)` otherwise.
    #[allow(clippy::result_unit_err)]
    pub fn set_text(&mut self, key: &str, text: Option<String>) -> Result<Option<String>, ()> {
        if !self.kind.is_text_key(key) {
            return Err(());
        }
        Ok(match text {
            Some(t) => self.text.insert(key.to_string(), t),
            None => self.text.remove(key),
        })
    }

    /// The audio files this instrument needs decoded, as `(key, path)`, in key order.
    pub fn assets(&self) -> impl Iterator<Item = (&str, &str)> {
        self.text
            .iter()
            .filter(|(k, _)| self.kind.is_asset_key(k))
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_describes_itself_consistently() {
        for kind in InstrumentKind::ALL {
            assert!(!kind.name_key().is_empty(), "{kind:?}");
            let mut seen = std::collections::HashSet::new();
            for info in kind.params() {
                assert!(
                    info.min <= info.default && info.default <= info.max,
                    "{kind:?} {}",
                    info.key
                );
                assert!(seen.insert(info.key), "{kind:?} lists {} twice", info.key);
                assert_eq!(kind.param(info.key), Some(info), "{kind:?} {}", info.key);
                assert!(!kind.is_text_key(info.key), "{kind:?} {}", info.key);
            }
            for tag in kind.sources() {
                assert!(modulation::source(tag).is_some(), "{kind:?} source {tag}");
            }
            // The shared model's keys are every kind's keys.
            assert!(kind.param("macro1").is_some() && kind.param("mod32.amt").is_some());
            assert!(kind.is_text_key("mod1.dst") && kind.is_text_key("macro8.name"));
            // An asset key is always a text key.
            for key in ["sample", "z0.sample", "osc1.table", "file"] {
                if kind.is_asset_key(key) {
                    assert!(kind.is_text_key(key), "{kind:?} {key}");
                }
            }
        }
    }

    #[test]
    fn set_clamps_rounds_and_clear_restores_the_default() {
        for kind in InstrumentKind::ALL {
            let mut s = NativeSpec::new(kind);
            for info in kind.params() {
                assert_eq!(s.get(info.key), info.default);
                assert_eq!(s.set(info.key, info.max + 1e6), Some(info.max));
                assert_eq!(s.set(info.key, f32::NAN), Some(info.default));
                assert!(s.clear(info.key).is_some());
                assert!(!s.is_stored(info.key));
            }
            assert_eq!(s.set("no such key", 1.0), None);
            assert!(s.params.is_empty());
        }
    }

    #[test]
    fn a_fresh_spec_serialises_to_its_kind_alone() {
        let s = NativeSpec::new(InstrumentKind::Organ);
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, r#"{"kind":"Organ"}"#);
        assert_eq!(serde_json::from_str::<NativeSpec>(&json).unwrap(), s);
    }
}
