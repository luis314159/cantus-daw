//! *Recover your work* (round 22, `recovery`): what a session that is gone left — its
//! autosave, the takes recorded since the last save — offered once at start, with
//! *Restore* and *Discard*. Drawn with the dialogs' kit pieces (`interop::form`), as the
//! demo picker's *unsaved* question is.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::Style;
use crate::recovery_restore::Item;
use crate::views::interop::form;
use egui::{Align2, Vec2};

/// "just now", "12 min ago", "3 h ago", "2 days ago".
pub fn when(at: u64, now: u64) -> String {
    let s = now.saturating_sub(at);
    if s < 60 {
        t("recovery.when.now")
    } else if s < 3_600 {
        tf("recovery.when.minutes", &[("n", &(s / 60))])
    } else if s < 86_400 {
        tf("recovery.when.hours", &[("n", &(s / 3_600))])
    } else {
        tf("recovery.when.days", &[("n", &(s / 86_400))])
    }
}

/// The lines that describe `item`, in order.
pub fn lines(item: &Item, now: u64) -> Vec<String> {
    let mut v = vec![if item.crashed {
        tf("recovery.crashed", &[("name", &item.name)])
    } else {
        tf("recovery.closed", &[("name", &item.name)])
    }];
    match (item.autosave.is_some(), item.autosaved_at) {
        (true, Some(at)) => v.push(tf("recovery.autosaved", &[("when", &when(at, now))])),
        (true, None) => {}
        (false, _) => v.push(t("recovery.no_autosave")),
    }
    let n = item.take_count();
    if n > 0 {
        v.push(tf("recovery.takes", &[("n", &n)]));
    }
    v.push(match &item.project_path {
        Some(p) => tf("recovery.file", &[("path", &p.display())]),
        None => t("recovery.unsaved_project"),
    });
    v
}

/// Draw the offer while there is one.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    let Some(item) = app.recovery.as_ref().and_then(|o| o.items.first()) else {
        return;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let text = lines(item, now);
    let (mut restore, mut discard) = (false, false);
    egui::Window::new(t("recovery.title"))
        .id(egui::Id::new("cantus-recovery"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            let s = Style::current();
            form::band(ui, "recovery.title");
            for (i, line) in text.iter().enumerate() {
                let color = if i == 0 { s.t.text } else { s.t.text_dim };
                ui.label(
                    egui::RichText::new(line)
                        .text_style(crate::design::caption())
                        .color(color),
                );
            }
            ui.add_space(crate::design::space::S2);
            ui.label(
                egui::RichText::new(t("recovery.discard_note"))
                    .text_style(crate::design::caption())
                    .color(s.t.text_dim),
            );
            form::buttons(ui, |ui| {
                restore = form::button(ui, &t("recovery.restore"), true, true);
                discard = form::button(ui, &t("recovery.discard"), false, true);
            });
        });
    if restore {
        app.restore_recovery(0);
    } else if discard {
        app.discard_recovery();
    }
}
