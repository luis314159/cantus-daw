//! The project's name and the window's title (ADR 0052): the name is shown in the hint
//! panel's first line and renamed in place — a click on it opens the kit's field there,
//! Enter or a click elsewhere keeps the words, Esc leaves (`Command::SetProjectName`, one
//! undo step). The window's title follows the name and the unsaved mark: set when the
//! splash becomes the main window (`boot::become_main_window`) and kept up to date every
//! frame after ([`sync`]).

use super::Style;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::field::{Event, Field};
use crate::kit::hint::{self, Caps};
use cantus_core::Command;
use egui::{vec2, Id, Rect, Sense, Ui};
use std::cell::RefCell;

/// The title last sent, and the name being typed.
#[derive(Debug, Default)]
pub struct State {
    sent: String,
    naming: Option<String>,
}

thread_local! {
    static TITLE: RefCell<String> = RefCell::new(String::with_capacity(96));
}

/// The window's title for a project `name`, `dirty` when it has unsaved changes:
/// "Name • — Cantus", or "Cantus" for a project with no name.
pub fn title_for(name: &str, dirty: bool, out: &mut String) {
    out.clear();
    let name = name.trim();
    if !name.is_empty() {
        out.push_str(name);
        if dirty {
            out.push_str(" \u{2022}");
        }
        out.push_str(" \u{2014} ");
    }
    out.push_str(crate::views::clips_chrome::word("app.title"));
}

/// The application's title now.
pub fn title_of(app: &CantusApp) -> String {
    let mut s = String::new();
    title_for(&app.project().name, app.bus.is_dirty(), &mut s);
    s
}

/// Once a frame: send the window its title when the name or the unsaved mark changed.
pub fn sync(app: &mut CantusApp, ctx: &egui::Context) {
    TITLE.with(|b| {
        let mut b = b.borrow_mut();
        // The words of the title change rarely: compare before building anything new.
        title_for(&app.project().name, app.bus.is_dirty(), &mut b);
        if *b != app.topbar.title.sent {
            app.topbar.title.sent.clone_from(&b);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(b.clone()));
        }
    });
}

/// The title last sent to the window (tests).
pub fn sent(app: &CantusApp) -> &str {
    &app.topbar.title.sent
}

/// Start renaming the project (a click on its name; tests).
pub fn start_naming(app: &mut CantusApp) {
    app.topbar.title.naming = Some(app.project().name.clone());
}

/// The name's line of the hint panel `panel`: a click renames; while renaming, the field.
pub fn name_field(app: &mut CantusApp, ui: &mut Ui, panel: Rect, s: &Style) {
    let k = s.k;
    let line = Rect::from_min_size(
        panel.min + vec2(4.0 * k, 1.0 * k),
        vec2(panel.width() - 8.0 * k, s.d(18.0)),
    );
    let Some(mut words) = app.topbar.title.naming.take() else {
        let resp = ui.interact(line, Id::new("toolbar-project-name"), Sense::click());
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
            hint::set_text(
                ui.ctx(),
                &t("topbar.name_tip"),
                &app.project().name,
                Caps::NONE,
            );
        }
        if resp.clicked() {
            start_naming(app);
        }
        return;
    };
    let hint_words = t("topbar.name");
    let out = Field::new(Id::new("toolbar-project-name-field"))
        .hint(&hint_words)
        .scale(k)
        .opens_when_shown()
        .show(ui, line, &mut words);
    match out.event {
        Some(Event::Commit) => {
            let name = words.trim().to_string();
            if out.edited && !name.is_empty() && name != app.project().name {
                app.exec(Command::SetProjectName { name });
            }
        }
        Some(Event::Cancel) => {}
        None => app.topbar.title.naming = Some(words),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_title_names_the_project_and_its_unsaved_mark() {
        let mut s = String::new();
        title_for("My song", false, &mut s);
        assert_eq!(s, "My song \u{2014} Cantus");
        title_for("My song", true, &mut s);
        assert_eq!(s, "My song \u{2022} \u{2014} Cantus");
        title_for("  ", true, &mut s);
        assert_eq!(s, "Cantus");
    }
}
