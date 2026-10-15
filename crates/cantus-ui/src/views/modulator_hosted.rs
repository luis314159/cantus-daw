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

/// The parameters of track `t`'s hosted instrument.
pub fn show(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId) {
    let Some(h) = app.plugins.hosted_instrument(t) else {
        return;
    };
    let shown: Vec<_> = h
        .params
        .iter()
        .filter(|p| !p.hidden && p.automatable && p.max != p.min)
        .collect();
    if shown.is_empty() {
        return;
    }
    sh.band("mods.hosted.params", Function::Source, None, false, None);
    // The filter: kept by egui for this track.
    let fid = Id::new(("mod-hosted-filter", t));
    let mut filter = sh.ui.data(|d| d.get_temp::<String>(fid)).unwrap_or_default();
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
    sh.ui.data_mut(|d| d.insert_temp(fid, filter.clone()));
    let needle = filter.to_lowercase();
    let mut matching = shown
        .iter()
        .filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle));
    let spec = app.project().track(t).and_then(|x| x.instrument.external());
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
        let text = h
            .param_text(p.id, shown_v)
            .unwrap_or_else(|| format!("{shown_v:.2}"));
        let range = Range {
            stepped: p.stepped,
            ..Range::lin(lo as f32, hi as f32, p.default as f32)
        };
        let id = p.id;
        let row_id = Id::new(("mod-hosted-param", t, id));
        let name = p.name.clone();
        param_row(sh, app, &name, row_id, shown_v as f32, &range, &text, target, move |v| {
            Act::Merge(
                Command::SetInstrumentPluginParam {
                    track: t,
                    id,
                    value: Some(f64::from(v)),
                },
                MergeKey::of(&("inst-plugin-param", t.0, id)),
            )
        });
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
    text: &str,
    target: AutoTarget,
    act: impl FnOnce(f32) -> Act,
) {
    let row = sh.row();
    // The plug-in's own name, as the label.
    let t = crate::kit::text::Text::new(name, crate::kit::text::Role::Body, sh.s.t.text_dim)
        .align(egui::Align2::RIGHT_CENTER)
        .policy(crate::kit::text::Policy::Abbreviate);
    let _ = crate::kit::text::paint(sh.b, row.label, &t, sh.s.tk());
    let base = range.pos(v);
    let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let (m, n) = app.mod_offer(&target, base, &sh.s, &mut rings);
    let offer = crate::kit::automate::Offer {
        automated: app.is_automated(&target),
        typing: false,
        modulation: Some(m),
    };
    let depth_drag = crate::kit::automate::depth_drag(sh.ui, Some(offer));
    let owned = text.to_string();
    let d = sh.value(
        row.value,
        id,
        v,
        range,
        false,
        name,
        move |_, b| b.push_str(&owned),
        Function::Source,
        false,
    );
    if let Some(nv) = d.value.filter(|_| !depth_drag) {
        sh.acts.push(act(nv));
    }
    if d.ended && !depth_drag {
        sh.acts.push(Act::EndGesture);
    }
    let s = sh.s;
    if offer.automated {
        crate::kit::automate::mark(sh.b, row.value, &s);
    }
    if n > 0 {
        crate::kit::automate::mod_mark(sh.b, row.value, &s);
    }
    let rect = row.value;
    let choice = crate::kit::automate::modulation_choice(sh.ui, &d.resp, Some(offer))
        .or_else(|| crate::kit::automate::menu(sh.ui, &d.resp, offer));
    if let Some(c) = choice {
        sh.acts.push(Act::Mod(super::modulators::ModAct::Param(target, c, rect)));
    }

}
