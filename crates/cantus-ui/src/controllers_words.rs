//! The words of a stepped target's values (round 29, review 3 of ADR 0062): a choice's
//! name — the same locale keys the editors draw and the exported plug-ins show a host —
//! for the controllers pane's *From* and *To* cells and the status line, and typed back
//! in either language. An effect's choices come from the table below (its prefixes, as
//! `cantus_plugins::params` lists them for a host, with the round-28 kinds' own tables in
//! `EffectKind::choices`); an instrument's from `inst_choices` (the editors' labels, held to
//! them by a test); a hosted plug-in's from its own words for each of its steps.

use crate::app::CantusApp;
use crate::i18n::{t_in, Locale};
use cantus_core::controller::MappingTarget;
use cantus_core::{AutoTarget, EffectKind, InstrumentKind};
use std::collections::HashMap;

/// One step's word: a locale key, a key with a number (`Band {n}`), or text the plug-in or
/// an editor gave.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Word {
    Key(String),
    Format(String, i32),
    Text(String),
}

impl Word {
    /// In the interface's language.
    pub fn show(&self) -> String {
        self.show_in(crate::i18n::locale())
    }

    /// In `locale`.
    pub fn show_in(&self, locale: Locale) -> String {
        match self {
            Word::Key(k) => t_in(locale, k),
            Word::Format(k, n) => crate::i18n::tf_in(locale, k, &[("n", n)]),
            Word::Text(s) => s.clone(),
        }
    }

    /// Whether `typed` (lower case) names it, in either language.
    fn named_by(&self, typed: &str) -> bool {
        match self {
            Word::Text(s) => s.to_lowercase() == typed,
            _ => [Locale::En, Locale::Es]
                .iter()
                .any(|l| self.show_in(*l).to_lowercase() == typed),
        }
    }
}

/// An effect's choice names for `key` in `locale`, from its lowest value up (`None`: it has
/// none) — what the exported plug-ins give a host too (`cantus_plugins::params`, whose test
/// holds the two tables equal).
pub fn effect_choice_words(kind: EffectKind, key: &str, locale: Locale) -> Option<Vec<String>> {
    let info = kind.param(key)?;
    if !info.stepped {
        return None;
    }
    let (lo, hi) = (info.min.round() as i32, info.max.round() as i32);
    Some(
        effect_words(kind, key, lo, hi)?
            .iter()
            .map(|w| w.show_in(locale))
            .collect(),
    )
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

    /// What an editor drew for `key` of `kind` (the test that holds `inst_choices` to the
    /// editors reads it).
    pub fn get(&self, kind: InstrumentKind, key: &str) -> Option<&Vec<Word>> {
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

/// A hosted plug-in's stepped parameter at `a`: its description, its id and its steps (at
/// most [`HOSTED_WORDS`]), when its steps may have words.
fn hosted_steps<'a>(
    app: &'a CantusApp,
    a: &AutoTarget,
) -> Option<(&'a cantus_host::ParamInfo, u32, u32)> {
    let id = match a {
        AutoTarget::Plugin { id, .. } | AutoTarget::PluginInstrument { id, .. } => *id,
        _ => return None,
    };
    let info = app.hosted_param_info(a)?;
    let n = info.step_count()?;
    (1..=HOSTED_WORDS).contains(&n).then_some((info, id, n))
}

/// A hosted parameter's steps beyond this are numbers, not choices.
pub const HOSTED_WORDS: u32 = 64;

/// The plain value of step `k` of `n` of a hosted parameter: its own steps, whatever its
/// plain values are (a JUCE VST3's 0…1, another's 0, 10…40; review 4, #1, #58).
fn hosted_step_value(info: &cantus_host::ParamInfo, k: u32, n: u32) -> f64 {
    info.min + f64::from(k) / f64::from(n) * (info.max - info.min)
}

/// A native target's choice words, from its lowest value up, with that value; `None` for
/// a target that has none (a range, or a stepped number such as a count or semitones).
fn native_words(app: &CantusApp, target: &MappingTarget) -> Option<(i32, Vec<Word>)> {
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
        // A native instrument's from its static table — never from what an editor drew
        // this session (review 4, #7, #22, #33, #45).
        MappingTarget::Param(AutoTarget::Instrument { track, key }) => {
            let kind = p.track(*track)?.instrument.native()?.kind;
            let info = kind.param(key)?;
            if !info.stepped {
                return None;
            }
            Some((
                info.min.round() as i32,
                crate::inst_choices::words(kind, key)?,
            ))
        }
        MappingTarget::Param(AutoTarget::Firmus {
            param: cantus_core::FirmusParam::Osc1Wave | cantus_core::FirmusParam::Osc2Wave,
            ..
        }) => Some((0, crate::inst_choices::firmus_waves())),
        _ => None,
    }
}

/// `target`'s steps' words, from its lowest step up — what a typed name is looked up in
/// (a hosted plug-in is asked for each of its steps: only when a name is typed).
pub fn stepped_words(app: &CantusApp, target: &MappingTarget) -> Option<Vec<Word>> {
    match target {
        MappingTarget::Param(
            a @ (AutoTarget::Plugin { .. } | AutoTarget::PluginInstrument { .. }),
        ) => {
            let (info, id, n) = hosted_steps(app, a)?;
            (0..=n)
                .map(|k| {
                    app.hosted_param_text(a, id, hosted_step_value(info, k, n))
                        .map(Word::Text)
                })
                .collect()
        }
        _ => native_words(app, target).map(|(_, w)| w),
    }
}

/// The word of `target` at `pos` (0…1 of its travel): the name of the step there — a
/// hosted plug-in asked for that one step only (review 4, #27, #63); `None` when its steps
/// have no words.
pub fn word_at_pos(app: &CantusApp, target: &MappingTarget, pos: f32) -> Option<String> {
    match target {
        MappingTarget::Param(
            a @ (AutoTarget::Plugin { .. } | AutoTarget::PluginInstrument { .. }),
        ) => {
            let (info, id, n) = hosted_steps(app, a)?;
            let k = (pos.clamp(0.0, 1.0) * n as f32).round() as u32;
            app.hosted_param_text(a, id, hosted_step_value(info, k, n))
        }
        MappingTarget::Param(a) => {
            let (lo, words) = native_words(app, target)?;
            let r = cantus_core::automation::range::range_of(app.project(), a)?;
            let i = (r.value(pos).round() as i32 - lo).clamp(0, words.len() as i32 - 1);
            words.get(i as usize).map(Word::show)
        }
        _ => None,
    }
}

/// The position (0…1 of `target`'s travel) of the step `typed` names, in either language.
pub fn pos_named(app: &CantusApp, target: &MappingTarget, typed: &str) -> Option<f32> {
    let typed = typed.trim().to_lowercase();
    match target {
        MappingTarget::Param(
            a @ (AutoTarget::Plugin { .. } | AutoTarget::PluginInstrument { .. }),
        ) => {
            let (_, _, n) = hosted_steps(app, a)?;
            let words = stepped_words(app, target)?;
            let k = words.iter().position(|w| w.named_by(&typed))?;
            Some(k as f32 / n as f32)
        }
        MappingTarget::Param(a) => {
            let (lo, words) = native_words(app, target)?;
            let i = words.iter().position(|w| w.named_by(&typed))?;
            let r = cantus_core::automation::range::range_of(app.project(), a)?;
            Some(r.pos((lo + i as i32) as f32))
        }
        _ => None,
    }
}

/// Whether `target`'s steps have names (a hosted plug-in asked for its first step only).
pub fn has_words(app: &CantusApp, target: &MappingTarget) -> bool {
    match target {
        MappingTarget::Param(
            a @ (AutoTarget::Plugin { .. } | AutoTarget::PluginInstrument { .. }),
        ) => hosted_steps(app, a).is_some_and(|(info, id, n)| {
            app.hosted_param_text(a, id, hosted_step_value(info, 0, n))
                .is_some()
        }),
        _ => native_words(app, target).is_some(),
    }
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
                v.push(Word::Format("export.bits".into(), bits));
            }
            Some(v)
        }
        // Which band is heard alone: none, or band n (review 4, #39, #46).
        (EffectKind::Eq, "solo") => {
            let mut v = vec![Word::Key("export.off".into())];
            for b in 1..=hi {
                v.push(Word::Format("export.group.band".into(), b));
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

/// Every effect choice key the pane may show (the i18n test holds both languages to them).
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
