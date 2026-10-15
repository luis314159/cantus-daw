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
pub mod chorda;
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
    /// Chorda: the modelled grand piano (round 28).
    Chorda,
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
            InstrumentKind::Chorda => chorda::$item $( ( $($arg),* ) )?,
        }
    };
}

impl InstrumentKind {
    pub const ALL: [InstrumentKind; 8] = [
        InstrumentKind::Wavetable,
        InstrumentKind::Analog,
        InstrumentKind::Fm,
        InstrumentKind::Sampler,
        InstrumentKind::Bass,
        InstrumentKind::ElectricPiano,
        InstrumentKind::Organ,
        InstrumentKind::Chorda,
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

    /// Keys stored with every new instance, at today's value: a model version (Chorda).
    pub fn stored_versions(self) -> &'static [(&'static str, f32)] {
        match self {
            InstrumentKind::Chorda => &[("model", chorda::MODEL)],
            _ => &[],
        }
    }

    /// Keys whose meaning a later round changed, with the value a spec written before
    /// they existed means: what a file with no schema of its own (a user preset, a
    /// DAWproject state) reads when the key is absent. Exemplar's round robin was seeded
    /// before round 28; Chorda's first model is 1.
    pub fn unversioned_firsts(self) -> &'static [(&'static str, f32)] {
        match self {
            InstrumentKind::Sampler => &[("rr.order", 0.0)],
            InstrumentKind::Chorda => &[("model", 1.0)],
            _ => &[],
        }
    }

    /// Whether a parameter may be automated (a clip may drive it): every one but those a
    /// kind derives off the audio thread — Chorda's tuning, which designs its strings
    /// (ADR 0059).
    pub fn automatable(self, key: &str) -> bool {
        match self {
            InstrumentKind::Chorda => !chorda::DESIGN_KEYS.contains(&key),
            _ => true,
        }
    }

    /// Every parameter the generic editors list (knobs, automation, a plug-in export). A
    /// kind with a structured space (zones, matrix slots) may accept more keys through
    /// [`param`](Self::param) than it lists here.
    pub fn params(self) -> &'static [InstParamInfo] {
        dispatch!(self, m::params())
    }

    /// The description of one numeric key, or `None` when the kind has no such key.
    pub fn param(self, key: &str) -> Option<&'static InstParamInfo> {
        if key == TRIM_KEY {
            return self.trim_info();
        }
        dispatch!(self, m::param(key))
    }

    /// The kind's output calibration (ADR 0039), in dB: what the hidden [`TRIM_KEY`] of a
    /// new instance reads, so it sits at the level the other instruments do. 0 for a kind
    /// that was never recalibrated (it has no trim at all).
    pub fn calibration_db(self) -> f32 {
        self.trim_info().map_or(0.0, |i| i.default)
    }

    fn trim_info(self) -> Option<&'static InstParamInfo> {
        match self {
            InstrumentKind::ElectricPiano => Some(&LAMINA_TRIM),
            InstrumentKind::Sampler => Some(&EXEMPLAR_TRIM),
            _ => None,
        }
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

/// The hidden output trim (ADR 0039, round 24), in dB: a kind recalibrated after projects
/// were saved with it reads its calibration here when the key is not stored, and the
/// migration that came with the calibration stores the old level (0 dB) in every instance
/// saved before, so those keep their sound to the bit. Not listed in
/// [`InstrumentKind::params`]: no knob, no automation lane; only kinds that were
/// recalibrated accept it ([`InstrumentKind::calibration_db`] ≠ 0).
///
/// | Kind | Calibration | What it lifts |
/// |---|---|---|
/// | Lamina | +5 dB | the output (the gain stage, the last one) |
/// | Exemplar | +8 dB | the placeholder sine an instance with nothing loaded plays; a loaded instrument plays its samples at their own level |
pub const TRIM_KEY: &str = "trim";

static LAMINA_TRIM: InstParamInfo =
    crate::effect::p(TRIM_KEY, "inst.trim", -24.0, 24.0, 5.0, "dB", false, false);
static EXEMPLAR_TRIM: InstParamInfo =
    crate::effect::p(TRIM_KEY, "inst.trim", -24.0, 24.0, 8.0, "dB", false, false);

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
        let mut params = BTreeMap::new();
        // A model version is stored with every instance (ADR 0059): a later model never
        // changes a song made with this one.
        for (k, v) in kind.stored_versions() {
            params.insert(k.to_string(), *v);
        }
        NativeSpec {
            kind,
            params,
            text: BTreeMap::new(),
        }
    }

    /// The spec as written to a file that has no schema version of its own to migrate
    /// (a user preset, a DAWproject device state): a recalibrated kind's [`TRIM_KEY`] is
    /// spelled out even at its default, so the file reads back at the level it was
    /// written with whatever a later calibration says (ADR 0039).
    pub fn spell_out_trim(&self) -> NativeSpec {
        let mut s = self.clone();
        if self.kind.calibration_db() != 0.0 && !s.is_stored(TRIM_KEY) {
            s.set(TRIM_KEY, self.get(TRIM_KEY));
        }
        // A key whose default changed is spelled out too (Exemplar's round robin, round 28).
        for (k, _) in self.kind.unversioned_firsts() {
            if !s.is_stored(k) {
                s.set(k, self.get(k));
            }
        }
        s
    }

    /// The spec as read from such a file: a recalibrated kind without a trim was written
    /// before its calibration, at the old level (a trim of 0 dB); one whose trim is the
    /// calibration reads as a new instance does, with nothing stored. The inverse of
    /// [`spell_out_trim`](Self::spell_out_trim) for a spec written after the calibration.
    pub fn read_unversioned(self) -> NativeSpec {
        let mut s = self;
        let cal = s.kind.calibration_db();
        if cal != 0.0 {
            match s.params.get(TRIM_KEY) {
                None => {
                    s.set(TRIM_KEY, 0.0);
                }
                Some(&v) if v == cal => {
                    s.clear(TRIM_KEY);
                }
                Some(_) => {}
            }
        }
        // A key whose default changed: absent, the file was written before it existed (its
        // first meaning); at today's default, it reads as a new instance does.
        for (k, first) in s.kind.unversioned_firsts() {
            match s.params.get(*k) {
                None => {
                    s.set(k, *first);
                }
                Some(&v) => {
                    let stored_anyway = s.kind.stored_versions().iter().any(|(sk, _)| sk == k);
                    if !stored_anyway && s.kind.param(k).is_some_and(|i| i.default == v) {
                        s.clear(k);
                    }
                }
            }
        }
        s
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
    fn a_chorda_stores_its_model_and_a_preset_file_keeps_versions() {
        // Every new Chorda holds the model it was made with (ADR 0059).
        let c = NativeSpec::new(InstrumentKind::Chorda);
        assert_eq!(c.params.get("model"), Some(&chorda::MODEL));
        // A file written before the key existed reads its first meaning.
        let mut old = NativeSpec::new(InstrumentKind::Chorda);
        old.clear("model");
        assert_eq!(old.read_unversioned().params.get("model"), Some(&1.0));
        // An Exemplar preset saved before round 28 keeps its seeded round robin; one saved
        // now spells the order out and reads back as a new instance.
        let before = NativeSpec::new(InstrumentKind::Sampler);
        assert_eq!(before.clone().read_unversioned().get("rr.order"), 0.0);
        let now = NativeSpec::new(InstrumentKind::Sampler);
        let written = now.spell_out_trim();
        assert!(written.is_stored("rr.order"));
        assert_eq!(written.read_unversioned(), now);
    }

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

    #[test]
    fn the_trim_is_hidden_and_only_recalibrated_kinds_have_it() {
        for kind in InstrumentKind::ALL {
            assert!(
                kind.params().iter().all(|i| i.key != TRIM_KEY),
                "{kind:?} lists it"
            );
            let s = NativeSpec::new(kind);
            match kind {
                InstrumentKind::ElectricPiano | InstrumentKind::Sampler => {
                    let info = kind.param(TRIM_KEY).expect("a trim");
                    assert_eq!(info.default, kind.calibration_db());
                    assert!(kind.calibration_db() > 0.0);
                    assert_eq!(s.get(TRIM_KEY), kind.calibration_db());
                    let mut old = s.clone();
                    assert_eq!(old.set(TRIM_KEY, 0.0), Some(0.0));
                    assert_eq!(old.get(TRIM_KEY), 0.0);
                    assert!(!kind.is_text_key(TRIM_KEY));
                    // Unversioned files: written spelled out, read back as they were;
                    // one with no trim is from before the calibration.
                    // (The trim alone: Exemplar's round robin is versioned the same way,
                    // `a_chorda_stores_its_model_and_a_preset_file_keeps_versions`.)
                    let trim = |x: &NativeSpec| x.params.get(TRIM_KEY).copied();
                    let written = s.spell_out_trim();
                    assert!(written.is_stored(TRIM_KEY));
                    assert_eq!(trim(&written.clone().read_unversioned()), trim(&s));
                    assert_eq!(trim(&old.spell_out_trim()), trim(&old));
                    assert_eq!(trim(&old.clone().read_unversioned()), trim(&old));
                    assert_eq!(trim(&s.clone().read_unversioned()), trim(&old));
                }
                _ => {
                    assert_eq!(s.spell_out_trim(), s);
                    assert_eq!(s.clone().read_unversioned(), s);
                    assert!(kind.param(TRIM_KEY).is_none());
                    assert_eq!(kind.calibration_db(), 0.0);
                    assert_eq!(s.clone().set(TRIM_KEY, 0.0), None);
                }
            }
        }
    }
}
