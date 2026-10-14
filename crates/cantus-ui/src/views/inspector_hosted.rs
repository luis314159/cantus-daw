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
        return;
    };
    let ok = sh.fmt.tr("hosted.status_ok").to_string();
    sh.text_row("hosted.status", &ok, false, false);
    let latency = tf("hosted.latency_value", &[("n", &h.latency())]);
    sh.text_row("hosted.latency", &latency, true, false);
    sh.text_row("hosted.params", &h.params.len().to_string(), true, false);
    // The plug-in's own window: *Open*, or — open already, perhaps minimised or under
    // other windows — *Show*, which brings it back and to the front (round 27, ADR 0051).
    // It closes with its own close box, F8 or the rack's channel button.
    let open = h.window_open();
    let key = if open {
        "hosted.show_window"
    } else {
        "hosted.open_window"
    };
    if sh.led_row(
        "hosted.window",
        key,
        open,
        Function::Source,
        !h.has_editor(),
    ) {
        sh.acts.push(Act::OpenInstrument(t));
    }
}
