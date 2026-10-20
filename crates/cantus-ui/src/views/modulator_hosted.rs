//! A hosted instrument's parameters in the inspector (ADR 0057): the plug-in's own list, as
//! label | value rows with the plug-in's words for each value — so a parameter of a hosted
//! instrument can be set here, and **automated and modulated** from its menu (right-click:
//! *Create automation clip*, *Modulate…*), Alt+drag for a modulation's depth, a modulator's
//! handle dropped on it. A filter narrows a long list; at most [`SHOWN`] rows are laid out.

use super::sheet::{Range, Sheet};
use super::Act;
use crate::app::CantusApp;
use crate::kit::Function;
use cantus_core::{AutoTarget, Command, MergeKey, TrackId};
use egui::Id;

/// Rows laid out at most: the filter finds the rest.
pub const SHOWN: usize = 48;

/// The plug-in's words for each value shown, by parameter, with the value's bits they
/// were asked for: the plug-in is asked again only when the value moves (review 1), and a
/// frame clones a reference, never a string. Every word is forgotten when the stamp
/// changes — another plug-in or instance on the track, or a stepped value the project
/// stores for it (a plug-in may word one parameter by another's switch: a rate in Hz or
/// in notes by its sync switch) (review 2). A continuous value moving (a drag) is its
/// own row's to re-ask, never every row's (review of fix3: a drag re-asked all 48 rows
/// every frame).
#[derive(Clone, Default)]
struct Texts {
    stamp: u64,
    words: std::collections::HashMap<u32, (u64, std::sync::Arc<str>)>,
}

impl Texts {
    /// The words for parameter `id` at the value with `bits`, asked of `ask` only when not
    /// known for this stamp; a plug-in that has not answered yet (`None`, isolated) is
    /// asked again next time and shows `fallback` meanwhile — never kept (round 29, review 3
    /// of ADR 0062, #59).
    fn get(
        &mut self,
        stamp: u64,
        id: u32,
        bits: u64,
        ask: impl FnOnce() -> Option<String>,
        fallback: impl FnOnce() -> String,
    ) -> std::sync::Arc<str> {
        if self.stamp != stamp {
            self.stamp = stamp;
            self.words.clear();
        }
        match self.words.get(&id) {
            Some((b, s)) if *b == bits => s.clone(),
            _ => match ask() {
                Some(s) => {
                    let s: std::sync::Arc<str> = s.into();
                    self.words.insert(id, (bits, s.clone()));
                    s
                }
                None => fallback().into(),
            },
        }
    }
}

/// The stamp of the words of `h`, as the project stores `spec`: its instance and the
/// stored value of each stepped parameter (no allocation).
fn stamp(h: &cantus_host::HostedInst, spec: Option<&cantus_core::ExternalPlugin>) -> u64 {
    stamp_of(h.instance(), &h.params, spec)
}

fn stamp_of(
    instance: impl std::hash::Hash,
    params: &[cantus_host::ParamInfo],
    spec: Option<&cantus_core::ExternalPlugin>,
) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut s = std::collections::hash_map::DefaultHasher::new();
    instance.hash(&mut s);
    for p in params.iter().filter(|p| p.stepped) {
        if let Some(v) = spec.and_then(|e| e.params.get(&p.id)) {
            p.id.hash(&mut s);
            v.to_bits().hash(&mut s);
        }
    }
    s.finish()
}

/// Whether `hay` holds `needle` (already lower case), ignoring case, without allocating
/// for an ASCII name.
fn contains_ci(hay: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !hay.is_ascii() || !needle.is_ascii() {
        return hay.to_lowercase().contains(needle);
    }
    let (h, n) = (hay.as_bytes(), needle.as_bytes());
    h.windows(n.len())
        .any(|w| w.iter().zip(n).all(|(a, b)| a.to_ascii_lowercase() == *b))
}

/// The parameters of track `t`'s hosted instrument.
pub fn show(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId) {
    let Some(h) = app.plugins.hosted_instrument(t) else {
        return;
    };
    let listed = |p: &&cantus_host::ParamInfo| !p.hidden && p.automatable && p.max != p.min;
    if !h.params.iter().any(|p| listed(&p)) {
        return;
    }
    sh.band("mods.hosted.params", Function::Source, None, false, None);
    // The filter: kept by egui for this track.
    let fid = Id::new(("mod-hosted-filter", t));
    let mut filter = sh
        .ui
        .data(|d| d.get_temp::<String>(fid))
        .unwrap_or_default();
    {
        let row = sh.row();
        let mut ui = sh.ui.new_child(egui::UiBuilder::new().max_rect(row.value));
        let w = row.value.width() / sh.s.k;
        crate::views::fx::controls::text_field(
            &mut ui,
            &sh.s,
            fid.with("field"),
            &mut filter,
            &crate::i18n::t("mods.hosted.filter"),
            w,
        );
        sh.label(row.label, "mods.hosted.filter_label", false);
    }
    let needle = filter.to_lowercase();
    if sh.ui.data(|d| d.get_temp::<String>(fid)).as_deref() != Some(filter.as_str()) {
        sh.ui.data_mut(|d| d.insert_temp(fid, filter));
    }
    let tid = Id::new(("mod-hosted-text", t));
    let mut matching = h
        .params
        .iter()
        .filter(listed)
        .filter(|p| contains_ci(&p.name, &needle));
    let spec = app.project().track(t).and_then(|x| x.instrument.external());
    let stamp = stamp(h, spec);
    for p in matching.by_ref().take(SHOWN) {
        let value = spec
            .and_then(|e| e.params.get(&p.id).copied())
            .or_else(|| h.param_value(p.id))
            .unwrap_or(p.default);
        let (lo, hi) = (p.min.min(p.max), p.max.max(p.min));
        let target = AutoTarget::PluginInstrument { track: t, id: p.id };
        let shown_v = app
            .automated_value(&target)
            .filter(|x| x.is_finite())
            .map_or(value, |pos| lo + f64::from(pos) * (hi - lo));
        let bits = shown_v.to_bits();
        let text = sh.ui.data_mut(|d| {
            d.get_temp_mut_or_default::<Texts>(tid).get(
                stamp,
                p.id,
                bits,
                || h.param_text(p.id, shown_v),
                || format!("{shown_v:.2}"),
            )
        });
        let range = Range {
            stepped: p.stepped,
            ..Range::lin(lo as f32, hi as f32, p.default as f32)
        };
        let id = p.id;
        // The instance in the row's id too: the sheet keeps its value's words by it.
        let row_id = Id::new(("mod-hosted-param", t, id, h.instance()));
        param_row(
            sh,
            app,
            &p.name,
            row_id,
            shown_v as f32,
            &range,
            text,
            target,
            move |v| {
                Act::Merge(
                    Command::SetInstrumentPluginParam {
                        track: t,
                        id,
                        value: Some(f64::from(v)),
                    },
                    MergeKey::of(&("inst-plugin-param", t.0, id)),
                )
            },
        );
    }
    let more = matching.count();
    if more > 0 {
        let words = crate::i18n::tf("mods.hosted.more", &[("n", &more)]);
        sh.text_row("mods.hosted.more_label", &words, false, true);
    }
}

/// One parameter's row: its name, the plug-in's words for its value, the drag, the menu.
#[allow(clippy::too_many_arguments)]
fn param_row(
    sh: &mut Sheet<'_>,
    app: &CantusApp,
    name: &str,
    id: Id,
    v: f32,
    range: &Range,
    text: std::sync::Arc<str>,
    target: AutoTarget,
    act: impl FnOnce(f32) -> Act,
) {
    let row = sh.row();
    // The plug-in's own name, as the label.
    let t = crate::kit::text::Text::new(name, crate::kit::text::Role::Body, sh.s.t.text_dim)
        .align(egui::Align2::RIGHT_CENTER)
        .policy(crate::kit::text::Policy::Abbreviate);
    let _ = crate::kit::text::paint(sh.b, row.label, &t, sh.s.tk());
    // The native rows' wiring (review of fix3): while a modulator maps, only the drag is
    // the depth's — a double-click resets, a typed value lands — and the live tick shows.
    let d = super::modulators::wired_value(
        sh,
        app,
        "mods.hosted.param",
        row.value,
        id,
        v,
        range,
        false,
        name,
        move |_, b| b.push_str(&text),
        Function::Source,
        target,
        false,
        true,
    );
    if let Some(nv) = d.value {
        sh.acts.push(act(nv));
    }
    if d.ended {
        sh.acts.push(Act::EndGesture);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_plugins_words_are_forgotten_when_the_stamp_changes() {
        // Review 2: synth B's parameter 0 at 0.5 showed synth A's "Saw" (same id, same
        // value bits) until the value moved.
        let mut texts = super::Texts::default();
        let bits = 0.5f64.to_bits();
        let mut asked = 0;
        let mut ask = |w: &str| {
            asked += 1;
            w.to_string()
        };
        let fb = || "0.50".to_string();
        assert_eq!(&*texts.get(1, 0, bits, || Some(ask("Saw")), fb), "Saw");
        assert_eq!(&*texts.get(1, 0, bits, || Some(ask("never")), fb), "Saw");
        assert_eq!(&*texts.get(2, 0, bits, || Some(ask("50 %")), fb), "50 %");
        assert_eq!(
            &*texts.get(2, 0, 0.6f64.to_bits(), || Some(ask("60 %")), fb),
            "60 %"
        );
        assert_eq!(asked, 3);
    }

    /// Round 29, review 3 of ADR 0062, #59: an isolated plug-in that has not answered yet
    /// shows the number meanwhile and is asked again — its words show once they come.
    #[test]
    fn an_unanswered_value_is_asked_again() {
        let mut texts = super::Texts::default();
        let bits = 0.42f64.to_bits();
        assert_eq!(&*texts.get(1, 0, bits, || None, || "0.42".into()), "0.42");
        assert_eq!(
            &*texts.get(1, 0, bits, || Some("1.2 kHz".into()), || "0.42".into()),
            "1.2 kHz"
        );
    }

    /// Review of fix3: a continuous value moving (a drag merging every frame) leaves the
    /// stamp — every other row's words stay — while a stepped one (a sync switch that
    /// rewords a rate) or another instance changes it.
    #[test]
    fn a_dragged_value_keeps_the_other_rows_words() {
        let info = |id: u32, stepped: bool| cantus_host::ParamInfo {
            id,
            name: format!("p{id}"),
            group: String::new(),
            min: 0.0,
            max: 1.0,
            default: 0.0,
            stepped,
            automatable: true,
            hidden: false,
            read_only: false,
            steps: 0,
        };
        let params = [info(0, false), info(1, true)];
        let mut spec = cantus_core::ExternalPlugin::new(
            cantus_core::PluginFormat::Clap,
            "test.synth",
            "Synth",
        );
        spec.params.insert(0, 0.25);
        spec.params.insert(1, 0.0);
        let at = |spec: &cantus_core::ExternalPlugin, instance: u64| {
            super::stamp_of(instance, &params, Some(spec))
        };
        let before = at(&spec, 7);
        spec.params.insert(0, 0.75);
        assert_eq!(at(&spec, 7), before, "a drag keeps the stamp");
        spec.params.insert(1, 1.0);
        assert_ne!(at(&spec, 7), before, "a switch rewords");
        assert_ne!(at(&spec, 8), at(&spec, 7), "another instance");
    }

    #[test]
    fn the_filter_ignores_case_without_allocating_for_ascii_names() {
        assert!(super::contains_ci("Filter Cutoff", "cutoff"));
        assert!(super::contains_ci("Filter Cutoff", ""));
        assert!(!super::contains_ci("Resonance", "cutoff"));
        assert!(super::contains_ci("Frecuencia ÁGUDA", "águda"));
    }
}
