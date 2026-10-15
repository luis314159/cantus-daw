//! The metronome group (ADR 0052): its switch (Ctrl+M), and beside it the two things
//! touched most — the **count-in** (off, 1, 2, 4 bars) and the click's **level** — and a ▾
//! for the rest: its **voice** (classic, woodblock, beep), the **accent** on the downbeat
//! and **when** it sounds (always, while recording, during the count-in). Schema 26 keeps
//! the voice and the accent with the song.

use super::{key_in, key_rect, tip, Style, KEY, STEP};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::keymap::Action;
use crate::kit::cap::{self, KeyFace};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, with_brush, State};
use crate::views::record_options as ro;
use cantus_core::{ClickMode, ClickSound, Command};
use egui::{vec2, Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;

thread_local! {
    static LEVEL: RefCell<String> = RefCell::new(String::with_capacity(12));
}

const COUNT_INS: [(u8, &str); 4] = [
    (0, "rec.opt.off"),
    (1, "rec.opt.bar1"),
    (2, "rec.opt.bar2"),
    (4, "rec.opt.bar4"),
];
const WORDS: [&str; 2] = ["topbar.count_in", "topbar.level"];
/// The ▾'s width at 100 %.
const DOWN_W: f32 = 14.0;

fn columns(ctx: &egui::Context, s: &Style) -> (f32, f32) {
    let tk = s.tk();
    let label = WORDS
        .iter()
        .map(|k| text::measure(ctx, &Text::key(k, Role::Micro, s.t.text).caps(true), tk).x)
        .fold(0.0, f32::max);
    let value = COUNT_INS
        .iter()
        .map(|(_, k)| text::measure(ctx, &Text::key(k, Role::Value, s.t.text), tk).x)
        .chain(std::iter::once(
            text::measure(
                ctx,
                &Text::new("\u{2212}24.0 dB", Role::Value, s.t.text),
                tk,
            )
            .x,
        ))
        .fold(0.0, f32::max);
    (label + 8.0 * s.k, value + 10.0 * s.k)
}

pub fn width(ctx: &egui::Context, s: &Style) -> f32 {
    let (l, v) = columns(ctx, s);
    (KEY + STEP + DOWN_W + STEP) * s.k + l + v
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let on = app.project().metronome;
    let kr = key_rect(r, r.left(), s);
    if key_in(
        ui,
        kr,
        s,
        KeyFace::Glyph(Glyph::Metronome),
        on,
        "metronome",
        || tip(app, "transport.metronome", Some(Action::ToggleMetronome)),
    )
    .clicked()
    {
        app.run_action(Action::ToggleMetronome);
    }
    let (lw, vw) = columns(ui.ctx(), s);
    let x = kr.right() + STEP * k;
    let row_h = s.d(20.0);
    let top = Rect::from_min_size(
        Pos2::new(x, r.center().y - row_h - 1.0 * k),
        Vec2::new(lw + vw, row_h),
    );
    let bottom = top.translate(vec2(0.0, row_h + 2.0 * k));
    count_in(app, ui, top, lw, s);
    level(app, ui, bottom, lw, s);
    let down = Rect::from_min_size(
        Pos2::new(top.right() + STEP * k, kr.top()),
        Vec2::new(DOWN_W * k, kr.height()),
    );
    options_key(app, ui, down, s);
}

/// A row: its word at the left, its value in a well at the right.
fn row(ui: &Ui, r: Rect, lw: f32, s: &Style, st: State, word: &str, value: &str) {
    let k = s.k;
    with_brush(ui.painter(), |b| {
        let lr = Rect::from_min_size(r.min, Vec2::new(lw, r.height()));
        let _ = text::paint(
            b,
            lr.shrink2(Vec2::new(2.0 * k, 0.0)),
            &Text::key(word, Role::Micro, s.t.text_dim)
                .caps(true)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
        let vr = Rect::from_min_max(Pos2::new(r.left() + lw, r.top()), r.max);
        kit::well::paint(b, vr, s, st, None);
        let _ = text::paint(
            b,
            vr.shrink2(Vec2::new(5.0 * k, 0.0)),
            &Text::new(value, Role::Value, s.t.text)
                .align(Align2::CENTER_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
}

fn count_in(app: &mut CantusApp, ui: &mut Ui, r: Rect, lw: f32, s: &Style) {
    let resp = ui.interact(r, Id::new("toolbar-count-in"), Sense::click());
    let bars = app.project().recording.count_in_bars;
    let i = COUNT_INS.iter().position(|(b, _)| *b == bars).unwrap_or(0);
    let popup = egui::Popup::default_response_id(&resp);
    let open = egui::Popup::is_id_open(ui.ctx(), popup);
    let st = State::of(&resp, open);
    let value = crate::views::clips_chrome::word(COUNT_INS[i].1);
    row(ui, r, lw, s, st, WORDS[0], value);
    if st.hovered {
        let name = tip(app, "topbar.count_in_tip", Some(Action::ToggleCountIn));
        let caps = Caps {
            menu: true,
            ..Caps::NONE
        };
        hint::set_text(ui.ctx(), &name, value, caps);
    }
    if open || resp.clicked() {
        egui::Popup::menu(&resp)
            .frame(egui::Frame::NONE)
            .show(|ui| {
                let items: [kit::menu::Item<'_>; 4] =
                    std::array::from_fn(|j| kit::menu::Item::Action {
                        label: Src::Key(COUNT_INS[j].1),
                        check: kit::menu::Check::Radio(j == i),
                        shortcut: None,
                        submenu: false,
                        enabled: true,
                    });
                let (_, c) = kit::widgets::menu_list(ui, &items, 110.0);
                if let Some(j) = c.clicked {
                    set_count_in(app, COUNT_INS[j].0);
                    ui.close();
                }
            });
    }
}

fn set_count_in(app: &mut CantusApp, bars: u8) {
    let mut rs = app.project().recording.clone();
    if rs.count_in_bars != bars {
        rs.count_in_bars = bars;
        app.set_record_settings(rs);
    }
}

/// The click's level: drag (200 pt for −24…+6 dB, Shift for fine), wheel by 0.5 dB,
/// double-click for 0 dB.
fn level(app: &mut CantusApp, ui: &mut Ui, r: Rect, lw: f32, s: &Style) {
    let resp = ui.interact(r, Id::new("toolbar-click-level"), Sense::click_and_drag());
    let db = app.project().recording.click_db;
    // The drag is summed unrounded while it lasts (in the bar's memory): a slow fine drag
    // adds up instead of being rounded away each frame.
    let raw_id = Id::new("toolbar-click-level-raw");
    if resp.drag_started() {
        ui.data_mut(|d| d.insert_temp(raw_id, db));
    }
    let mut v = db;
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let d = resp.drag_delta();
        let raw = ui.data(|d| d.get_temp::<f32>(raw_id)).unwrap_or(db)
            + (d.x - d.y) * fine * 30.0 / 200.0;
        let raw = raw.clamp(-24.0, 6.0);
        ui.data_mut(|d| d.insert_temp(raw_id, raw));
        v = raw;
    }
    if resp.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            v += 0.5 * dy.signum();
        }
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if resp.double_clicked() {
        v = 0.0;
    }
    let v = ((v * 10.0).round() / 10.0).clamp(-24.0, 6.0);
    if v != db {
        let mut rs = app.project().recording.clone();
        rs.click_db = v;
        app.exec_merging(
            Command::SetRecordSettings { settings: rs },
            cantus_core::MergeKey::of(&"click-level"),
        );
    }
    if resp.drag_stopped() || resp.double_clicked() {
        app.end_gesture();
    }
    let st = State::of(&resp, false);
    LEVEL.with(|w| {
        let mut w = w.borrow_mut();
        w.clear();
        if v < 0.0 {
            let _ = write!(w, "\u{2212}{:.1} dB", -v);
        } else {
            let _ = write!(w, "+{v:.1} dB");
        }
        row(ui, r, lw, s, st, WORDS[1], &w);
        if st.hovered || resp.dragged() {
            hint::set_text(ui.ctx(), &t("rec.click_level"), &w, Caps::PARAM);
        }
    });
}

// ---- the options ---------------------------------------------------------------------

fn options_popup() -> Id {
    Id::new("toolbar-metronome-options")
}

fn options_key(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let resp = ui.interact(r, Id::new("toolbar-metronome-more"), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), options_popup());
    let st = State::of(&resp, open);
    with_brush(ui.painter(), |b| {
        cap::toolbar_key(b, r, s, st, KeyFace::Text(""));
        let g = Rect::from_center_size(r.center(), vec2(8.0 * s.k, 8.0 * s.k));
        let ink = if open {
            s.t.highlight_glyph
        } else {
            s.t.text_dim
        };
        glyph::paint(b, g, Glyph::Down, ink);
    });
    if st.hovered {
        let caps = Caps {
            menu: true,
            ..Caps::NONE
        };
        hint::set_text(ui.ctx(), &t("topbar.metronome_options"), "", caps);
    }
    let toggle = resp.clicked();
    if !open && !toggle {
        return;
    }
    egui::Popup::from_response(&resp)
        .id(options_popup())
        .open_memory(toggle.then_some(egui::SetOpenCommand::Toggle))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(ro::frame_for(s))
        .gap(4.0)
        .show(|ui| options(app, ui));
}

/// Open the metronome's options (the screenshots, a test).
pub fn open_options(ctx: &egui::Context) {
    egui::Popup::open_id(ctx, options_popup());
}

/// The metronome's options: voice, accent, when, level, count-in.
fn options(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    ui.spacing_mut().item_spacing = vec2(4.0 * s.k, 3.0 * s.k);
    let mut rs = app.project().recording.clone();
    let before = rs.clone();
    ro::band_row(ui, "topbar.metronome", &s);
    ui.horizontal(|ui| {
        ro::label(ui, "topbar.click_sound", &s);
        let mut i = ClickSound::ALL
            .iter()
            .position(|c| *c == rs.click_sound)
            .unwrap_or(0);
        let labels = [
            Src::Key("topbar.sound.classic"),
            Src::Key("topbar.sound.wood"),
            Src::Key("topbar.sound.beep"),
        ];
        let name = t("topbar.click_sound");
        if kit::widgets::segmented(ui, &mut i, &labels, &name, ro::CONTROL_W).changed() {
            rs.click_sound = ClickSound::ALL[i];
        }
    });
    ui.horizontal(|ui| {
        ro::label(ui, "topbar.accent", &s);
        let mut on = rs.click_accent;
        let name = t("topbar.accent_tip");
        if kit::widgets::led_button(ui, &mut on, Src::Key("topbar.accent_on"), &name, 110.0)
            .changed()
        {
            rs.click_accent = on;
        }
    });
    ui.horizontal(|ui| {
        ro::label(ui, "rec.click_when", &s);
        let modes = [
            ClickMode::Always,
            ClickMode::Recording,
            ClickMode::CountInOnly,
        ];
        let mut i = modes.iter().position(|m| *m == rs.click).unwrap_or(0);
        let labels = [
            Src::Key("rec.click.always"),
            Src::Key("rec.click.recording"),
            Src::Key("rec.click.count_in"),
        ];
        let name = t("rec.click_when");
        if kit::widgets::segmented(ui, &mut i, &labels, &name, ro::CONTROL_W).changed() {
            rs.click = modes[i];
        }
    });
    let mut level_done = false;
    ui.horizontal(|ui| {
        ro::label(ui, "rec.click_level", &s);
        let (v, ended) = ro::level(ui, rs.click_db, &s);
        rs.click_db = v;
        level_done = ended;
    });
    ui.horizontal(|ui| {
        ro::label(ui, "rec.count_in", &s);
        let mut i = COUNT_INS
            .iter()
            .position(|(b, _)| *b == rs.count_in_bars)
            .unwrap_or(0);
        let labels = COUNT_INS.map(|(_, k)| Src::Key(k));
        let name = t("rec.count_in");
        if kit::widgets::segmented(ui, &mut i, &labels, &name, ro::CONTROL_W).changed() {
            rs.count_in_bars = COUNT_INS[i].0;
        }
    });
    if rs != before {
        app.set_record_settings(rs);
    }
    if level_done || ui.input(|i| i.pointer.any_released()) {
        app.end_gesture();
    }
}
