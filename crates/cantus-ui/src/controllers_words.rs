//! The words of a stepped target's values (round 29, review 3 of ADR 0062): a choice's
//! name — the same locale keys the editors draw and the exported plug-ins show a host —
//! for the controllers pane's *From* and *To* cells and the status line, and typed back
//! in either language. An effect's choices come from the table below (its prefixes, as
//! `cantus_plugins::params` lists them for a host, with the round-28 kinds' own tables in
//! `EffectKind::choices`); an instrument's from the labels its editor drew (noted once per
//! key, never per frame); a hosted plug-in's from its own words for each step.

use crate::app::CantusApp;
use crate::i18n::{t, t_in, Locale};
use cantus_core::controller::MappingTarget;
use cantus_core::{AutoTarget, EffectKind, InstrumentKind};
use std::collections::HashMap;

/// One step's word: a locale key, or text the plug-in or an editor gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Word {
    Key(String),
    Text(String),
}

impl Word {
    /// In the interface's language.
    pub fn show(&self) -> String {
        match self {
            Word::Key(k) => t(k),
            Word::Text(s) => s.clone(),
        }
    }

    /// Whether `typed` (lower case) names it, in either language.
    fn named_by(&self, typed: &str) -> bool {
        match self {
            Word::Key(k) => [Locale::En, Locale::Es]
                .iter()
                .any(|l| t_in(*l, k).to_lowercase() == typed),
            Word::Text(s) => s.to_lowercase() == typed,
        }
    }
}

/// An instrument's choices as its editor drew them, by kind and key; and the language they
/// were noted in (another clears them: some are text in that language).
#[derive(Debug, Default)]
pub struct InstWords {
    words: HashMap<InstrumentKind, HashMap<String, Vec<Word>>>,
    locale: Option<Locale>,
}

impl InstWords {
    /// Note the words an editor draws for `key` of `kind`, the first time only: a lookup,
    /// no allocation, every frame after.
    pub fn note(&mut self, kind: InstrumentKind, key: &str, words: impl FnOnce() -> Vec<Word>) {
        let lang = crate::i18n::locale();
        if self.locale != Some(lang) {
            self.words.clear();
            self.locale = Some(lang);
        }
        if self.words.get(&kind).is_some_and(|m| m.contains_key(key)) {
            return;
        }
        self.words
            .entry(kind)
            .or_default()
            .insert(key.to_string(), words());
    }

    fn get(&self, kind: InstrumentKind, key: &str) -> Option<&Vec<Word>> {
        self.words.get(&kind)?.get(key)
    }
}

/// Words of locale keys.
pub fn keys(labels: &[&str]) -> Vec<Word> {
    labels.iter().map(|k| Word::Key(k.to_string())).collect()
}

/// Words of the kit's label sources.
pub fn srcs(labels: &[crate::kit::text::Src<'_>]) -> Vec<Word> {
    use crate::kit::text::Src;
    labels
        .iter()
        .map(|s| match s {
            Src::Key(k) => Word::Key(k.to_string()),
            Src::Str(x) => Word::Text(x.to_string()),
        })
        .collect()
}

/// `target`'s steps' words, from its lowest value up, with that value; `None` for a
/// target that has none (a range, or a stepped number such as a count or semitones).
pub fn stepped_words(app: &CantusApp, target: &MappingTarget) -> Option<(i32, Vec<Word>)> {
    let p = app.project();
    match target {
        MappingTarget::Param(AutoTarget::Effect { effect, key }) => {
            let host = p.effect_host(*effect)?;
            let kind = p.effect(host, *effect)?.spec.kind;
            let info = kind.param(key)?;
            if !info.stepped {
                return None;
            }
            let (lo, hi) = (info.min.round() as i32, info.max.round() as i32);
            let words = effect_words(kind, key, lo, hi)?;
            Some((lo, words))
        }
        MappingTarget::Param(AutoTarget::Instrument { track, key }) => {
            let kind = p.track(*track)?.instrument.native()?.kind;
            let info = kind.param(key)?;
            if !info.stepped {
                return None;
            }
            let words = app.controllers.inst_words.get(kind, key)?;
            Some((info.min.round() as i32, words.clone()))
        }
        MappingTarget::Param(a @ (AutoTarget::Plugin { .. } | AutoTarget::PluginInstrument { .. })) => {
            let info = app.hosted_param_info(a)?;
            let n = (info.max - info.min).round();
            if !info.stepped || !(1.0..=64.0).contains(&n) {
                return None;
            }
            let id = match a {
                AutoTarget::Plugin { id, .. } | AutoTarget::PluginInstrument { id, .. } => *id,
                _ => return None,
            };
            let words: Option<Vec<Word>> = (0..=n as i32)
                .map(|i| {
                    app.hosted_param_text(a, id, info.min + f64::from(i))
                        .map(Word::Text)
                })
                .collect();
            Some((info.min.round() as i32, words?))
        }
        _ => None,
    }
}

/// The word of the step at `value` (in the target's own units).
pub fn word_at(app: &CantusApp, target: &MappingTarget, value: f32) -> Option<String> {
    let (lo, words) = stepped_words(app, target)?;
    let i = (value.round() as i32 - lo).clamp(0, words.len() as i32 - 1);
    words.get(i as usize).map(Word::show)
}

/// The value (in the target's units) whose word `typed` is, in either language.
pub fn value_named(app: &CantusApp, target: &MappingTarget, typed: &str) -> Option<f32> {
    let (lo, words) = stepped_words(app, target)?;
    let typed = typed.trim().to_lowercase();
    words
        .iter()
        .position(|w| w.named_by(&typed))
        .map(|i| (lo + i as i32) as f32)
}

/// An effect's choices from `lo` to `hi`.
fn effect_words(kind: EffectKind, key: &str, lo: i32, hi: i32) -> Option<Vec<Word>> {
    let field = band_field(kind, key);
    if let Some(prefix) = choice_prefix(kind, &field) {
        return Some(
            (lo..=hi)
                .map(|i| Word::Key(choice_key(kind, prefix, i)))
                .collect(),
        );
    }
    match (kind, field.as_str()) {
        (EffectKind::Limiter | EffectKind::Maximizer, "dither") => {
            let mut v = vec![Word::Key("export.off".into())];
            for bits in [24, 22, 20, 18, 16] {
                v.push(Word::Text(crate::i18n::tf("export.bits", &[("n", &bits)])));
            }
            Some(v)
        }
        _ => None,
    }
}

/// A banded key's field (`b3.shape` → `b.shape`): the band's number left out.
fn band_field(kind: EffectKind, key: &str) -> String {
    let Some((head, field)) = key.split_once('.') else {
        return key.to_string();
    };
    if kind == EffectKind::Maximizer && matches!(head, "lo" | "mid" | "hi" | "all") {
        return format!("b.{field}");
    }
    let digits = head.trim_start_matches(|c: char| c.is_ascii_alphabetic());
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return key.to_string();
    }
    format!("{}.{field}", &head[..head.len() - digits.len()])
}

/// The locale prefix of a stepped parameter's choices (`{prefix}.{i}`), by kind and field —
/// the table the exported plug-ins give a host (`cantus_plugins::params`), the round-28
/// kinds from their own modules (`EffectKind::choices`).
fn choice_prefix(kind: EffectKind, field: &str) -> Option<&'static str> {
    use EffectKind::*;
    Some(match (kind, field) {
        (Eq, "b.shape") | (Compressor, "sc.shape") => "eq.shape",
        (Eq, "b.place") => "eq.place",
        (Eq, "mode") => "eq.mode",
        (Eq, "lp_res") => "eq.res",
        (Compressor, "style") => "comp.style",
        (Compressor, "listen") => "comp.listen",
        (Compressor, "os") => "comp.os",
        (Limiter, "style") => "lim.style",
        (Limiter, "monitor") => "lim.monitor",
        (Limiter, "shape") => "lim.shape",
        (Space, "style") => "space.style",
        (Space, "sync") => "space.sync",
        (Space, "d.shape") => "space.dshape",
        (Space, "e.shape") => "space.pshape",
        (Multiband, "mode") => "mb.mode",
        (Multiband, "listen") => "mb.listen",
        (Multiband, "b.place") => "mb.place",
        (DeEsser, "mode") => "ds.mode",
        (DeEsser, "proc") => "ds.proc",
        (DeEsser, "stereo") => "ds.stereo",
        (DeEsser, "listen") => "ds.listen",
        (Gate, "style") => "gate.style",
        (Gate, "listen") => "gate.hear",
        (Stereo, "listen") => "st.listen",
        (Stereo, "mono_slope") | (Stereo, "tone_slope") => "st.slope",
        (Stereo, "haas_mode") => "st.haas_mode",
        (Maximizer, "style") => "lim.style",
        (Maximizer, "shape") => "lim.shape",
        (Maximizer, "monitor") => "max.monitor",
        (Maximizer, "xmode") => "max.xmode",
        (Maximizer, "b.place") => "mb.place",
        (Saturator, "model") => "sat.model",
        (Saturator, "os") => "sat.os",
        (Saturator, "phase") => "sat.phase",
        (Saturator, "chan") => "sat.chan",
        _ => {
            return kind
                .choices()
                .iter()
                .find(|(k, _)| *k == field)
                .map(|(_, prefix)| *prefix)
        }
    })
}

/// One choice's locale key: `{prefix}.{i}`, except where an editor names it otherwise
/// (Comprimo's fifth listen mode is its *Compare* button).
fn choice_key(kind: EffectKind, prefix: &str, i: i32) -> String {
    match (kind, prefix, i) {
        (EffectKind::Compressor, "comp.listen", 4) => "comp.compare".to_string(),
        _ => format!("{prefix}.{i}"),
    }
}

/// Every effect choice key the pane may show, for the i18n test (both languages).
pub fn effect_choice_keys() -> Vec<String> {
    let mut out = Vec::new();
    for kind in EffectKind::ALL {
        for info in kind.params().iter().filter(|i| i.stepped) {
            let field = band_field(kind, info.key);
            if let Some(prefix) = choice_prefix(kind, &field) {
                for i in info.min.round() as i32..=info.max.round() as i32 {
                    let k = choice_key(kind, prefix, i);
                    if !out.contains(&k) {
                        out.push(k);
                    }
                }
            }
        }
    }
    out
}
