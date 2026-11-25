//! Settings ▸ Editing (round 31, ADRs 0070 and 0071): automatic crossfades and their
//! curve, and whether an inserted effect starts on its starting preset or at its neutral
//! settings. In the settings window's row grammar: label | segmented control, a note under
//! each.

use super::workspace_settings::{note, LABEL_W};
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::text::Src;
use crate::kit::widgets::segmented;
use cantus_core::crossfade::XfadeCurve;
use egui::Ui;

/// The page's body.
pub fn page(app: &mut CantusApp, ui: &mut Ui) {
    w::band(ui, Src::Key("clipfx.settings.crossfades"));
    ui.horizontal(|ui| {
        w::row_label(ui, "clipfx.settings.auto", LABEL_W, 24.0);
        let mut sel = usize::from(!app.settings.auto_crossfade);
        let labels = [
            Src::Key("clipfx.settings.on"),
            Src::Key("clipfx.settings.off"),
        ];
        if segmented(ui, &mut sel, &labels, &t("clipfx.settings.auto"), 160.0).changed() {
            app.set_auto_crossfade(sel == 0);
        }
    });
    ui.horizontal(|ui| {
        w::row_label(ui, "clipfx.curve", LABEL_W, 24.0);
        let mut sel = XfadeCurve::ALL
            .iter()
            .position(|c| *c == app.settings.crossfade_curve)
            .unwrap_or(0);
        let labels: Vec<Src<'_>> = XfadeCurve::ALL
            .iter()
            .map(|c| Src::Key(c.label_key()))
            .collect();
        if segmented(ui, &mut sel, &labels, &t("clipfx.curve"), 440.0).changed() {
            app.set_auto_crossfade_curve(XfadeCurve::ALL[sel]);
        }
    });
    note(ui, &t("clipfx.settings.auto_note"), false);
    w::band(ui, Src::Key("clipfx.settings.inserts"));
    ui.horizontal(|ui| {
        w::row_label(ui, "clipfx.settings.insert", LABEL_W, 24.0);
        let mut sel = usize::from(app.settings.insert_neutral);
        let labels = [
            Src::Key("clipfx.settings.starting"),
            Src::Key("clipfx.settings.neutral"),
        ];
        if segmented(ui, &mut sel, &labels, &t("clipfx.settings.insert"), 300.0).changed() {
            app.set_insert_neutral(sel == 1);
        }
    });
    note(ui, &t("clipfx.settings.insert_note"), false);
    super::ripple_settings::rows(app, ui); // r40:ripple
    editor_rows(app, ui); // r42:editor-windows
}

/// r42:editor-windows (ADR 0161): how many instrument and effect editors stay open.
fn editor_rows(app: &mut CantusApp, ui: &mut Ui) {
    use crate::editor_windows::EditorsOpen;
    w::band(ui, Src::Key("editors.settings.band"));
    ui.horizontal(|ui| {
        w::row_label(ui, "editors.settings.open", LABEL_W, 24.0);
        let all = [EditorsOpen::OneAtATime, EditorsOpen::Several];
        let mut sel = all
            .iter()
            .position(|m| *m == app.settings.editors_open)
            .unwrap_or(0);
        let labels = all.map(|m| Src::Key(m.label_key()));
        if segmented(ui, &mut sel, &labels, &t("editors.settings.open"), 320.0).changed() {
            app.set_editors_open(all[sel]);
        }
    });
    note(ui, &t("editors.settings.note"), false);
}
