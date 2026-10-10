//! File → *Demo songs*: the picker (round 19). A list of the demos — name, style, tempo,
//! length — one selected; *Open* replaces the project as *New* does, and when the project
//! holds unsaved work the picker asks first (save, discard, or stay).
//!
//! Drawn with the kit: rows painted on the dialog's panel, a band of the song's colour at
//! each row's edge, the interop dialogs' buttons.

use crate::app::CantusApp;
use crate::demos::{self, Demo};
use crate::i18n::{t, tf};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{color::with_alpha, with_brush, Fill, Style, Tok};
use crate::views::interop::form;
use egui::{Align2, Color32, Rect, Sense, Vec2};

/// The picker's state: whether it shows, the row chosen, whether it is asking about
/// unsaved work, and what it has measured of each demo.
#[derive(Default)]
pub struct Picker {
    pub open: bool,
    pub selected: usize,
    pub confirm: bool,
    rows: Vec<Row>,
}

/// What a row shows of its demo.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    style: String,
    bpm: f64,
    seconds: f64,
    color: Color32,
}

impl Row {
    fn of(d: &Demo) -> Row {
        let p = (d.build)();
        // The colour of the song's first melodic track: what it sounds like at a glance.
        let c = p
            .tracks
            .iter()
            .find(|t| t.instrument.native().is_some())
            .or(p.tracks.first())
            .map(|t| t.color)
            .unwrap_or(cantus_core::Color::rgb(0x80, 0x80, 0x80));
        Row {
            name: t(d.name_key),
            style: t(d.style_key),
            bpm: p.tempo_bpm,
            seconds: demos::seconds(&p),
            color: Color32::from_rgb(c.r, c.g, c.b),
        }
    }
}

/// Show the picker (the menu entry and its key).
pub fn show(app: &mut CantusApp) {
    let p = &mut app.demos;
    p.open = true;
    p.confirm = false;
    // Measured once per language: building a demo is quick but not free.
    if p.rows.first().map(|r| r.name.as_str()) != Some(t(demos::ALL[0].name_key).as_str()) {
        p.rows = demos::ALL.iter().map(Row::of).collect();
    }
}

/// Open demo `i`, replacing the project — what the picker's *Open* does once any unsaved
/// work has been dealt with.
pub fn open(app: &mut CantusApp, i: usize) {
    if let Some(d) = demos::ALL.get(i) {
        app.replace_project((d.build)());
        app.demos.open = false;
        app.demos.confirm = false;
    }
}

fn length(seconds: f64) -> String {
    let s = seconds.round() as i64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// One row: the song's colour at its edge, its name, and style · tempo · length under it.
/// Returns (clicked, double-clicked).
fn row(ui: &mut egui::Ui, r: &Row, selected: bool) -> (bool, bool) {
    let s = Style::current();
    let w = ui.available_width().max(s.s(340.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(44.0)), Sense::click());
    let meta = tf(
        "demos.picker.meta",
        &[
            ("style", &r.style),
            ("bpm", &format!("{:.0}", r.bpm)),
            ("length", &length(r.seconds)),
        ],
    );
    with_brush(ui.painter(), |b| {
        b.register(
            rect,
            "demo-row",
            &[Tok::Selection, Tok::Text, Tok::TextDim, Tok::Panel],
            &[],
        );
        if selected || resp.hovered() {
            let a = if selected { 0.28 } else { 0.10 };
            b.rrect(rect, s.r(3.0), Fill::Solid(with_alpha(s.t.selection, a)));
        }
        let edge = Rect::from_min_size(
            rect.min + Vec2::new(s.s(6.0), s.s(8.0)),
            Vec2::new(s.s(4.0), rect.height() - s.s(16.0)),
        );
        b.rrect(edge, s.r(2.0), Fill::Solid(r.color));
        let left = rect.left() + s.s(18.0);
        let top = Rect::from_min_max(
            egui::pos2(left, rect.top() + s.s(4.0)),
            egui::pos2(rect.right() - s.s(8.0), rect.center().y + s.s(1.0)),
        );
        let bottom = Rect::from_min_max(
            egui::pos2(left, rect.center().y),
            egui::pos2(rect.right() - s.s(8.0), rect.bottom() - s.s(4.0)),
        );
        let name = Text::new(&r.name, Role::Body, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, top, &name, s.tk());
        let m = Text::new(&meta, Role::Label, s.t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, bottom, &m, s.tk());
    });
    (resp.clicked(), resp.double_clicked())
}

/// Draw the picker when it shows.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.demos.open {
        return;
    }
    let mut open_now = None;
    let mut close = false;
    let mut save_first = false;
    let project_name = app.project().name.clone();
    egui::Window::new(t("demos.picker.title"))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            let p = &mut app.demos;
            if p.confirm {
                form::band(ui, "demos.picker.unsaved");
                ui.label(
                    egui::RichText::new(tf(
                        "demos.picker.unsaved_text",
                        &[("name", &project_name)],
                    ))
                    .text_style(crate::design::caption())
                    .color(Style::current().t.text_dim),
                );
                form::buttons(ui, |ui| {
                    close = form::button(ui, &t("demos.picker.cancel"), false, true);
                    if form::button(ui, &t("demos.picker.discard"), false, true) {
                        open_now = Some(p.selected);
                    }
                    save_first = form::button(ui, &t("demos.picker.save_first"), true, true);
                });
                return;
            }
            form::band(ui, "demos.picker.list");
            for (i, r) in p.rows.clone().iter().enumerate() {
                let (click, double) = row(ui, r, i == p.selected);
                if click {
                    p.selected = i;
                }
                if double {
                    p.selected = i;
                    open_now = Some(i);
                }
            }
            ui.add_space(crate::design::space::S2);
            ui.label(
                egui::RichText::new(t("demos.picker.hint"))
                    .text_style(crate::design::caption())
                    .color(Style::current().t.text_dim),
            );
            form::buttons(ui, |ui| {
                close = form::button(ui, &t("demos.picker.cancel"), false, true);
                if form::button(ui, &t("demos.picker.open"), true, true) {
                    open_now = Some(p.selected);
                }
            });
            // The keyboard: arrows move, Enter opens, Escape closes.
            let (up, down, enter, esc) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                    i.key_pressed(egui::Key::Enter),
                    i.key_pressed(egui::Key::Escape),
                )
            });
            let n = p.rows.len().max(1);
            if up {
                p.selected = (p.selected + n - 1) % n;
            }
            if down {
                p.selected = (p.selected + 1) % n;
            }
            if enter {
                open_now = Some(p.selected);
            }
            close |= esc;
        });
    if close {
        app.demos.open = false;
        app.demos.confirm = false;
        return;
    }
    if save_first {
        self::save_first(app);
        return;
    }
    if let Some(i) = open_now {
        request(app, i);
    }
}

/// The picker's *Save first*: the save is written on a thread and the demo opens once it
/// has landed (`save_job`). The picker closes while it is written — nothing is left in it
/// to cancel — unless the save did not start (its file dialog was cancelled).
pub fn save_first(app: &mut CantusApp) {
    if !(app.demos.open && app.demos.confirm) {
        return;
    }
    let i = app.demos.selected;
    app.save_then(crate::views::unsaved::Then::Demo(i));
    if app.save_in_flight() || !app.bus.is_dirty() {
        app.demos.open = false;
        app.demos.confirm = false;
    }
}

/// Ask to open demo `i`: at once when the project has nothing unsaved (or the question
/// was already answered *Don't save*), otherwise the picker asks first.
pub fn request(app: &mut CantusApp, i: usize) {
    if app.bus.is_dirty() && !app.demos.confirm {
        app.demos.selected = i;
        app.demos.confirm = true;
    } else {
        open(app, i);
    }
}
