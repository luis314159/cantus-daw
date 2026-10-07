//! *Save changes?* before **New** and **Open** replace a project with unsaved changes
//! (round 22; found by the beta manual's author): Save / Don't save / Cancel, as the demo
//! picker already asks. Drawn with the dialogs' kit pieces (`interop::form`).

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::Style;
use crate::views::interop::form;
use egui::{Align2, Vec2};
use std::path::PathBuf;

/// What waits for the answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Then {
    New,
    /// The open dialog.
    Open,
    /// A file already chosen (a drop, a recent file, a test).
    OpenPath(PathBuf),
}

/// The answer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Save,
    DontSave,
    Cancel,
}

impl CantusApp {
    /// *New*: at once when nothing is unsaved, else after the question.
    pub fn request_new(&mut self) {
        self.ask_or(Then::New);
    }

    /// *Open…*: at once when nothing is unsaved, else after the question.
    pub fn request_open(&mut self) {
        self.ask_or(Then::Open);
    }

    /// Open `path`, asking first when the project has unsaved changes.
    pub fn request_open_path(&mut self, path: PathBuf) {
        self.ask_or(Then::OpenPath(path));
    }

    fn ask_or(&mut self, then: Then) {
        if self.bus.is_dirty() {
            self.unsaved_prompt = Some(then);
        } else {
            self.carry_out(then);
        }
    }

    fn carry_out(&mut self, then: Then) {
        match then {
            Then::New => self.new_project(),
            Then::Open => self.open_dialog(),
            Then::OpenPath(p) => self.open(&p),
        }
    }

    /// Answer the question. *Save* that does not save (cancelled, failed) goes no further.
    pub fn answer_unsaved(&mut self, answer: Answer) {
        let Some(then) = self.unsaved_prompt.take() else {
            return;
        };
        match answer {
            Answer::Cancel => {}
            Answer::DontSave => self.carry_out(then),
            Answer::Save => {
                self.save();
                if !self.bus.is_dirty() {
                    self.carry_out(then);
                }
            }
        }
    }
}

/// Draw the question while it is asked.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    if app.unsaved_prompt.is_none() {
        return;
    }
    let name = app.project().name.clone();
    let mut answer = None;
    egui::Window::new(t("unsaved.title"))
        .id(egui::Id::new("cantus-unsaved"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            form::band(ui, "unsaved.title");
            ui.label(
                egui::RichText::new(tf("unsaved.text", &[("name", &name)]))
                    .text_style(crate::design::caption())
                    .color(Style::current().t.text_dim),
            );
            form::buttons(ui, |ui| {
                if form::button(ui, &t("unsaved.save"), true, true) {
                    answer = Some(Answer::Save);
                }
                if form::button(ui, &t("unsaved.dont_save"), false, true) {
                    answer = Some(Answer::DontSave);
                }
                if form::button(ui, &t("unsaved.cancel"), false, true) {
                    answer = Some(Answer::Cancel);
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                answer = Some(Answer::Cancel);
            }
        });
    if let Some(a) = answer {
        app.answer_unsaved(a);
    }
}
