//! A hosted CLAP/VST3 instrument's section in the inspector (ADR 0028): what it is, who
//! made it, whether it is here, its latency and parameters, and its own window. The sound
//! itself is edited in the plug-in's window; a plug-in that is missing says so and keeps
//! everything the project holds for it.

use super::sheet::Sheet;
use super::Act;
use crate::app::CantusApp;
use crate::i18n::tf;
use crate::kit::hint::{self, Caps};
use crate::kit::Function;
use cantus_core::{ExternalPlugin, TrackId};
use egui::{Id, Sense};

pub fn show(sh: &mut Sheet<'_>, app: &CantusApp, t: TrackId, p: &ExternalPlugin) {
    sh.band("hosted.title", Function::Source, None, false, None);
    sh.text_row("hosted.plugin", &p.name, false, false);
    if !p.vendor.is_empty() {
        sh.text_row("hosted.vendor", &p.vendor, false, false);
    }
    sh.text_row("hosted.format", p.format.badge(), false, false);
    let target = crate::views::plugin_notice::Target::Instrument(t);
    // Stopped working (round 29, ADR 0063; review #32) — loaded, or crashed while it
    // loaded: said in the danger colour with its reason as the tip, and *Reload* under a
    // band of its own.
    if let Some(why) = app.plugins.instrument_crashed(t) {
        stopped_row(sh, t, why);
        // Its own label, not the name row's *Plug-in* (review #28).
        if sh.led_row(
            "isolation.slot.recover",
            "isolation.reload",
            false,
            Function::Source,
            false,
        ) {
            sh.acts.push(Act::Hosting(
                target,
                crate::views::plugin_notice::MenuAct::Reload,
            ));
        }
        if app.plugins.hosted_instrument(t).is_none() {
            let runs = crate::i18n::t(app.plugins.effective_isolation(p.isolation).label_key());
            sh.menu_row(
                "isolation.slot.band",
                &runs,
                super::menus::MenuId::Hosting(t),
                false,
            );
            return;
        }
    }
    let Some(h) = app.plugins.hosted_instrument(t) else {
        let missing = sh.fmt.tr("hosted.status_missing").to_string();
        let row = sh.text_row("hosted.status", &missing, false, true);
        let r = sh
            .ui
            .interact(row.full, Id::new(("hosted-missing", t)), Sense::hover());
        if r.hovered() {
            let why = app
                .plugins
                .instrument_missing(t)
                .unwrap_or("not installed on this computer");
            let tip = tf("hosted.status_missing_tip", &[("why", &why)]);
            hint::set_text(sh.ui.ctx(), &missing, &tip, Caps::NONE);
        }
        // Tried and not loaded (refused, or its helper busy): it can be tried again
        // (review isolation-4 #57).
        if app.plugins.instrument_load_error(t).is_some()
            && sh.led_row(
                "isolation.slot.recover",
                "isolation.reload",
                false,
                Function::Source,
                false,
            )
        {
            sh.acts.push(Act::Hosting(
                target,
                crate::views::plugin_notice::MenuAct::Reload,
            ));
        }
        return;
    };
    // Working: where it runs and what it costs.
    let stopped = app.plugins.instrument_crashed(t).is_some();
    if !stopped {
        let ok = sh.fmt.tr("hosted.status_ok").to_string();
        sh.text_row("hosted.status", &ok, false, false);
    }
    let runs = crate::i18n::t(h.isolation().label_key());
    sh.menu_row(
        "isolation.slot.band",
        &runs,
        super::menus::MenuId::Hosting(t),
        false,
    );
    // A stopped plug-in costs nothing: its last reading is not shown.
    if let Some(c) = h.cpu().filter(|_| !stopped) {
        let cpu = crate::views::plugin_notice::percent(c.share);
        sh.text_row("isolation.cpu", &cpu, true, false);
    }
    let latency = tf("hosted.latency_value", &[("n", &h.latency())]);
    sh.text_row("hosted.latency", &latency, true, false);
    sh.text_row("hosted.params", &h.params.len().to_string(), true, false);
    // The plug-in's own window (round 27, ADR 0051 §3): *Open*; open already — perhaps
    // minimised or under other windows — *Show*, which brings it back and to the front
    // (it closes with its own close box, F8, or the rack's channel button); and *Close*
    // for a plug-in that floats a window of its own, which Cantus cannot bring forward.
    let (key, close) = window_row(h.window_open(), h.window_floating());
    // A stopped plug-in has no window until it is reloaded: the row is off, and says why
    // (review #34).
    let top = sh.ui.cursor().top();
    let clicked = sh.led_row(
        "hosted.window",
        key,
        h.window_open(),
        Function::Source,
        !h.has_editor() || stopped,
    );
    if let Some(why) = app.plugins.instrument_crashed(t) {
        let span = sh.ui.max_rect();
        let row = egui::Rect::from_x_y_ranges(span.x_range(), top..=sh.ui.cursor().top());
        if sh.ui.rect_contains_pointer(row) {
            let why = crate::views::plugin_notice::stopped_words(why);
            let tip = tf("isolation.tip.stopped", &[("reason", &why)]);
            let heading = sh.fmt.tr("hosted.window").to_string();
            hint::set_text(sh.ui.ctx(), &heading, &tip, Caps::NONE);
        }
    }
    if clicked {
        sh.acts.push(if close {
            Act::CloseInstrument(t)
        } else {
            Act::OpenInstrument(t)
        });
    }
    // Its parameters, to set, automate and modulate (ADR 0057).
    super::modulator_hosted::show(sh, app, t);
}

/// The window row's word, and whether a click closes: by the window's state.
fn window_row(open: bool, floating: bool) -> (&'static str, bool) {
    match (open, floating) {
        (false, _) => ("hosted.open_window", false),
        (true, false) => ("hosted.show_window", false),
        (true, true) => ("hosted.close_window", true),
    }
}

/// The status row of a stopped plug-in: its label as any row's, its value in the danger
/// colour with a dot, why it stopped as the tip.
fn stopped_row(sh: &mut Sheet<'_>, t: TrackId, why: &str) {
    let stopped = super::sheet::name(sh.fmt.tr("isolation.stopped_card"));
    let row = sh.row();
    sh.label(row.label, "hosted.status", false);
    let s = sh.s.hue_of(crate::kit::Tok::Danger);
    crate::kit::well::readout(
        sh.b,
        row.value,
        &s,
        crate::kit::State::REST,
        &stopped,
        false,
    );
    sh.b.dot(
        egui::pos2(row.value.right() - s.s(10.0), row.value.center().y),
        s.s(3.0),
        s.t.danger,
    );
    let r = sh
        .ui
        .interact(row.full, Id::new(("hosted-stopped", t)), Sense::hover());
    if r.hovered() {
        let why = crate::views::plugin_notice::stopped_words(why);
        let tip = tf("isolation.tip.stopped", &[("reason", &why)]);
        hint::set_text(sh.ui.ctx(), &stopped, &tip, Caps::NONE);
    }
}

#[cfg(test)]
mod tests {
    use super::window_row;

    /// Review 4: a plug-in that floats its own editor keeps a working *Close*; any other
    /// open window gets *Show*.
    #[test]
    fn the_window_row_shows_or_closes_by_the_windows_state() {
        assert_eq!(window_row(false, false), ("hosted.open_window", false));
        assert_eq!(window_row(false, true), ("hosted.open_window", false));
        assert_eq!(window_row(true, false), ("hosted.show_window", false));
        assert_eq!(window_row(true, true), ("hosted.close_window", true));
    }
}
