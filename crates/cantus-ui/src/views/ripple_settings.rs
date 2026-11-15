//! r40:ripple — Settings ▸ Editing's *Section drag* (ADR 0116 §3): what a section dragged
//! with no key held does, Insert (the default) or Replace.

use super::workspace_settings::{note, LABEL_W};
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::ripple_ui::SectionDrag;

/// Settings ▸ Editing's rows: *Section drag* — Insert | Replace — and what each does.
pub fn rows(app: &mut CantusApp, ui: &mut egui::Ui) {
    use crate::kit::text::Src;
    w::band(ui, Src::Key("ripple.settings.band"));
    ui.horizontal(|ui| {
        w::row_label(ui, "ripple.settings.drag", LABEL_W, 24.0);
        let mut sel = usize::from(app.settings.section_drag == SectionDrag::Replace);
        let labels = [
            Src::Key("ripple.settings.insert"),
            Src::Key("ripple.settings.replace"),
        ];
        let changed = crate::kit::widgets::segmented(
            ui,
            &mut sel,
            &labels,
            &t("ripple.settings.drag"),
            220.0,
        )
        .changed();
        if changed {
            app.set_section_drag(if sel == 0 {
                SectionDrag::Insert
            } else {
                SectionDrag::Replace
            });
        }
    });
    note(ui, &t("ripple.settings.drag_note"), false);
}
