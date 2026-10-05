//! The record options (ADR 0025): a popover hanging from the ▾ beside the transport's
//! record key (or a right-click on the key itself), in the inspector's row grammar —
//! label | control rows under a section band:
//!
//! - **Count-in**: off, 1, 2 or 4 bars — the transport starts that many bars early with
//!   the click, and the take begins where the playhead was.
//! - **Punch**: on/off, and *set punch from selection*; the range is the red brace in the
//!   ruler.
//! - **Loop takes**: each pass of the loop becomes a take.
//! - **Take mode**: Stack, Replace, Overdub (notes).
//! - **Click**: always, while recording, during the count-in only; and its level.
//!
//! While recording, the clock shows the take's elapsed time beside a pulsing lamp
//! ([`paint_elapsed`]).

use crate::app::CantusApp;
use crate::kit::band;
use crate::kit::cap::{self, KeyFace};
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, well, with_brush, Brush, Fill, State, Style};
use cantus_core::audio::{ClickMode, TakeMode};
use cantus_core::Ticks;
use egui::{pos2, vec2, Align2, Id, Rect, Sense, Ui};
use std::cell::RefCell;
use std::fmt::Write as _;

thread_local! {
    static WORDS: RefCell<String> = RefCell::new(String::with_capacity(24));
}

/// The ▾ key's width at 100 %.
pub const KEY_W: f32 = 14.0;
/// The label column of the popover.
const LABEL_W: f32 = 92.0;
/// The control column.
const CONTROL_W: f32 = 240.0;

fn popup_id() -> Id {
    Id::new("transport-record-options")
}

/// The ▾ beside the record key, `rect` tall; a click (or a right-click on the record key,
/// `record`) opens the options under it.
pub fn key(app: &mut CantusApp, ui: &mut Ui, rect: Rect, record: &egui::Response, s: &Style) {
    let resp = ui.interact(rect, Id::new("toolbar-record-options"), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), popup_id());
    let st = State::of(&resp, open);
    with_brush(ui.painter(), |b| {
        cap::toolbar_key(b, rect, s, st, KeyFace::Text(""));
        let g = Rect::from_center_size(rect.center(), vec2(8.0 * s.k, 8.0 * s.k));
        glyph::paint(
            b,
            g,
            Glyph::Down,
            if open {
                s.t.highlight_glyph
            } else {
                s.t.text_dim
            },
        );
    });
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            kit_word("rec.options"),
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let toggle = resp.clicked() || record.secondary_clicked();
    if !open && !toggle {
        // Closed: nothing is built (the bar at rest allocates nothing).
        return;
    }
    let anchor = resp.union(record.clone());
    egui::Popup::from_response(&anchor)
        .id(popup_id())
        .open_memory(toggle.then_some(egui::SetOpenCommand::Toggle))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(frame(s))
        .gap(4.0)
        .show(|ui| body(app, ui));
}

fn kit_word(key: &'static str) -> &'static str {
    crate::views::clips_chrome::word(key)
}

fn frame(s: &Style) -> egui::Frame {
    egui::Frame::NONE
        .fill(s.t.panel)
        .stroke(egui::Stroke::new(1.0, shade(s.t.panel, -0.28)))
        .corner_radius(egui::CornerRadius::same(s.r(4.0).round() as u8))
        .inner_margin(egui::Margin::same(6))
        .shadow(crate::design::overlay_shadow(&crate::theme::p()))
}

/// A row's label, right-aligned in the label column.
fn label(ui: &mut Ui, key: &'static str, s: &Style) {
    let (r, _) = ui.allocate_exact_size(vec2(LABEL_W * s.k, s.d(24.0)), Sense::hover());
    with_brush(ui.painter(), |b| {
        let t = Text::key(key, Role::Body, s.t.text_dim)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Abbreviate);
        let _ = text::paint(b, r.shrink2(vec2(4.0 * s.k, 0.0)), &t, s.tk());
    });
}

fn band_row(ui: &mut Ui, key: &'static str, s: &Style) {
    let w = (LABEL_W + CONTROL_W + 8.0) * s.k;
    let (r, _) = ui.allocate_exact_size(vec2(w, s.s(band::BAND)), Sense::hover());
    with_brush(ui.painter(), |b| {
        band::section(b, r, s, State::REST, Src::Key(key), None, false);
    });
}

/// The popover's rows.
fn body(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    ui.spacing_mut().item_spacing = vec2(4.0 * s.k, 3.0 * s.k);
    let mut rs = app.project().recording.clone();
    let before = rs.clone();
    band_row(ui, "rec.options", &s);

    ui.horizontal(|ui| {
        label(ui, "rec.count_in", &s);
        let mut i = match rs.count_in_bars {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 3,
        };
        let labels = [
            Src::Key("rec.opt.off"),
            Src::Key("rec.opt.bar1"),
            Src::Key("rec.opt.bar2"),
            Src::Key("rec.opt.bar4"),
        ];
        if kit::widgets::segmented(ui, &mut i, &labels, kit_word("rec.count_in"), CONTROL_W)
            .changed()
        {
            rs.count_in_bars = [0, 1, 2, 4][i];
        }
    });
    ui.horizontal(|ui| {
        label(ui, "rec.punch", &s);
        let mut on = rs.punch_enabled;
        if kit::widgets::led_button(
            ui,
            &mut on,
            Src::Key("rec.punch_on"),
            kit_word("rec.punch"),
            90.0,
        )
        .changed()
        {
            rs.punch_enabled = on;
            if on && rs.punch.is_none() {
                rs.punch = app
                    .time_selection
                    .map(|(a, b)| cantus_core::LoopRegion { start: a, end: b });
            }
        }
        let from = kit::widgets::readout(
            ui,
            kit_word("rec.from_selection"),
            false,
            kit_word("rec.punch_from_selection"),
            (CONTROL_W - 94.0).max(60.0),
        );
        if from.clicked() {
            if let Some((a, b)) = app.time_selection {
                rs.punch = Some(cantus_core::LoopRegion { start: a, end: b });
                rs.punch_enabled = true;
            } else {
                app.set_status(crate::i18n::t("rec.no_selection"));
            }
        }
    });
    ui.horizontal(|ui| {
        label(ui, "rec.loop_takes", &s);
        let mut on = rs.loop_takes;
        if kit::widgets::led_button(
            ui,
            &mut on,
            Src::Key("rec.loop_takes_on"),
            kit_word("rec.loop_takes"),
            90.0,
        )
        .changed()
        {
            rs.loop_takes = on;
        }
    });
    ui.horizontal(|ui| {
        label(ui, "rec.take_mode", &s);
        let modes = [TakeMode::Stack, TakeMode::Replace, TakeMode::Overdub];
        let mut i = modes.iter().position(|m| *m == rs.take_mode).unwrap_or(0);
        let labels = modes.map(|m| Src::Key(crate::record_session::take_mode_key(m)));
        if kit::widgets::segmented(ui, &mut i, &labels, kit_word("rec.take_mode"), CONTROL_W)
            .changed()
        {
            rs.take_mode = modes[i];
        }
    });
    band_row(ui, "rec.click", &s);
    ui.horizontal(|ui| {
        label(ui, "rec.click_when", &s);
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
        if kit::widgets::segmented(ui, &mut i, &labels, kit_word("rec.click_when"), CONTROL_W)
            .changed()
        {
            rs.click = modes[i];
        }
    });
    let mut level_done = false;
    ui.horizontal(|ui| {
        label(ui, "rec.click_level", &s);
        let (v, ended) = level(ui, rs.click_db, &s);
        rs.click_db = v;
        level_done = ended;
    });
    if rs != before {
        app.set_record_settings(rs);
    }
    if level_done || ui.input(|i| i.pointer.any_released()) {
        app.end_gesture();
    }
}

/// The click's level: a value in a well, dragged (200 pt for −24…+6 dB, Shift for fine),
/// double-click for 0 dB.
fn level(ui: &mut Ui, db: f32, s: &Style) -> (f32, bool) {
    let (r, resp) = ui.allocate_exact_size(vec2(90.0 * s.k, s.d(22.0)), Sense::click_and_drag());
    let mut v = db;
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let d = resp.drag_delta();
        v = (v + (d.x - d.y) * fine * 30.0 / 200.0).clamp(-24.0, 6.0);
    }
    if resp.double_clicked() {
        v = 0.0;
    }
    v = (v * 10.0).round() / 10.0;
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    let st = State::of(&resp, false);
    WORDS.with(|w| {
        let mut w = w.borrow_mut();
        w.clear();
        if v < 0.0 {
            let _ = write!(w, "\u{2212}{:.1} dB", -v);
        } else {
            let _ = write!(w, "+{v:.1} dB");
        }
        if st.hovered || resp.dragged() {
            hint::set_text(ui.ctx(), kit_word("rec.click_level"), &w, Caps::PARAM);
        }
        with_brush(ui.painter(), |b| well::readout(b, r, s, st, &w, true));
    });
    (v, resp.drag_stopped() || resp.double_clicked())
}

/// While a pass runs, in the clock's side panel: a pulsing lamp and the take's elapsed
/// time (the count-in counts down as "−1.2"), and a warning glyph when the last pass lost
/// frames.
pub fn paint_elapsed(b: &mut Brush, app: &CantusApp, rect: Rect, s: &Style) {
    let t = &s.t;
    let k = s.k;
    let Some(live) = app.rec.pass.as_ref() else {
        if app.rec.dropped > 0 {
            let g = Rect::from_center_size(
                pos2(rect.right() - 8.0 * k, rect.center().y),
                vec2(11.0 * k, 11.0 * k),
            );
            glyph::paint(b, g, Glyph::Warning, t.danger);
        }
        return;
    };
    // Frames lost while recording: the warning stays at the right, the time beside it.
    let mut rect = rect;
    if app.rec.dropped > 0 {
        let g = Rect::from_center_size(
            pos2(rect.right() - 6.0 * k, rect.center().y),
            vec2(11.0 * k, 11.0 * k),
        );
        glyph::paint(b, g, Glyph::Warning, t.danger);
        rect.max.x -= 14.0 * k;
    }
    let now = b.ctx().input(|i| i.time) as f32;
    let pulse = 0.55 + 0.45 * (now * 4.0).sin().abs();
    let c = pos2(rect.left() + 6.0 * k, rect.center().y);
    b.circle(c, 4.0 * k, Fill::Solid(with_alpha(t.danger, pulse)));
    let pos = app.engine.position();
    let bpm = app.project().tempo_bpm;
    let start = live.plan.record_start;
    WORDS.with(|w| {
        let mut w = w.borrow_mut();
        w.clear();
        if pos < start {
            // Counting in: the bars and beats left.
            let left = start - pos;
            let ts = app.project().time_signature;
            let beats = (left.0 as f64 / ts.ticks_per_beat().0.max(1) as f64).ceil() as i64;
            let _ = write!(w, "\u{2212}{beats}");
        } else {
            let secs = (pos - start).max(Ticks::ZERO).to_seconds(bpm);
            let _ = write!(w, "{}:{:04.1}", (secs / 60.0) as i64, secs % 60.0);
        }
        let tr = Rect::from_min_max(pos2(c.x + 7.0 * k, rect.top()), rect.max);
        let tx = Text::new(&w, Role::Value, t.danger)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, tr, &tx, s.tk());
    });
}

/// Open the record options (the screenshots, a test).
pub fn open(ctx: &egui::Context) {
    egui::Popup::open_id(ctx, popup_id());
}
