//! The tempo block (ADR 0052): the tempo in a well (drag it, wheel it, double-click for
//! 120), **TAP** (the average of the last taps; a pause starts again; one undo step per
//! run, `tap`), and the **time signature**, a well that opens its editor: the beats, the
//! note value, the usual metres in one click (`Command::SetTimeSignature`).

use super::{key_in, key_rect, tip, Style, ROW_KEY, STEP};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::keymap::Action;
use crate::kit::cap::KeyFace;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, with_brush, State};
use cantus_core::{Command, MergeKey, TimeSignature};
use egui::{vec2, Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;

thread_local! {
    static WORDS: RefCell<(String, String)> =
        RefCell::new((String::with_capacity(12), String::with_capacity(8)));
}

fn bpm_w(ctx: &egui::Context, s: &Style) -> f32 {
    text::measure(ctx, &Text::new("888.8", Role::Hero, s.t.text), s.tk()).x
        + text::measure(
            ctx,
            &Text::key("transport.bpm", Role::Micro, s.t.text),
            s.tk(),
        )
        .x
        + 18.0 * s.k
}

fn tap_w(ctx: &egui::Context, s: &Style) -> f32 {
    let w = text::measure(
        ctx,
        &Text::key("topbar.tap", Role::Micro, s.t.text).caps(true),
        s.tk(),
    )
    .x + 12.0 * s.k;
    w.max(ROW_KEY * s.k)
}

fn metre_w(ctx: &egui::Context, s: &Style) -> f32 {
    text::measure(ctx, &Text::new("32/32", Role::Value, s.t.text), s.tk()).x + 12.0 * s.k
}

pub fn width(ctx: &egui::Context, s: &Style) -> f32 {
    bpm_w(ctx, s) + tap_w(ctx, s) + metre_w(ctx, s) + 2.0 * STEP * s.k
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let ctx = ui.ctx().clone();
    // As tall as a key of the row it sits in.
    let well_h = super::key_side(r, s);
    let bpm_r = Rect::from_min_size(
        Pos2::new(r.left(), r.center().y - well_h * 0.5),
        Vec2::new(bpm_w(&ctx, s), well_h),
    );
    tempo(app, ui, bpm_r, s);
    let tap_r = key_rect(r, bpm_r.right() + STEP * k, s);
    let tap_r = Rect::from_min_size(tap_r.min, Vec2::new(tap_w(&ctx, s), tap_r.height()));
    let resp = key_in(
        ui,
        tap_r,
        s,
        KeyFace::Text(crate::views::clips_chrome::word("topbar.tap")),
        app.topbar.tap.taps() > 0,
        "tap",
        || tip(app, "topbar.tap_tip", Some(Action::TapTempo)),
    );
    if resp.clicked() {
        app.run_action(Action::TapTempo);
    }
    let metre_r = Rect::from_min_size(
        Pos2::new(tap_r.right() + STEP * k, bpm_r.top()),
        Vec2::new(metre_w(&ctx, s), well_h),
    );
    metre(app, ui, metre_r, s);
}

/// The tempo, in a well: drag up or down (Shift for fine), the wheel steps it by one,
/// double-click returns to 120.
fn tempo(app: &mut CantusApp, ui: &mut Ui, well: Rect, s: &Style) {
    let k = s.k;
    let resp = ui.interact(well, Id::new("toolbar-tempo"), Sense::click_and_drag());
    let mut bpm = app.project().tempo_bpm;
    let before = bpm;
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.02
        } else {
            0.25
        };
        bpm -= (resp.drag_delta().y * fine) as f64;
    }
    if resp.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            bpm += dy.signum() as f64;
        }
    }
    if resp.double_clicked() {
        bpm = 120.0;
    }
    let bpm = ((bpm * 10.0).round() / 10.0).clamp(20.0, 999.0);
    if (bpm - before).abs() > 1e-4 {
        app.exec_merging(Command::SetTempo { bpm }, MergeKey::of(&"tempo"));
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    let st = State::of(&resp, false);
    if st.hovered {
        hint::set(
            ui.ctx(),
            &t("workspace.tempo"),
            Caps::PARAM,
            resp.dragged(),
            |b| write!(b, "{bpm:.1} {}", t("transport.bpm")),
        );
    }
    let unit_w = text::measure(
        ui.ctx(),
        &Text::key("transport.bpm", Role::Micro, s.t.text),
        s.tk(),
    )
    .x + 10.0 * k;
    WORDS.with(|v| {
        let mut v = v.borrow_mut();
        let value = &mut v.0;
        value.clear();
        let _ = write!(value, "{bpm:.1}");
        with_brush(ui.painter(), |b| {
            kit::well::paint(b, well, s, st, None);
            let vr = Rect::from_min_max(well.min, Pos2::new(well.right() - unit_w, well.bottom()));
            let ink = if resp.dragged() { s.t.focus } else { s.t.text };
            let _ = text::paint(
                b,
                vr.shrink2(Vec2::new(4.0 * k, 0.0)),
                &Text::new(value, Role::Hero, ink)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let ur = Rect::from_min_max(Pos2::new(well.right() - unit_w, well.top()), well.max);
            let _ = text::paint(
                b,
                ur.shrink2(Vec2::new(2.0 * k, 0.0)),
                &Text::key("transport.bpm", Role::Micro, s.t.text_dim).align(Align2::LEFT_CENTER),
                s.tk(),
            );
        });
    });
}

/// The seconds the tap clock has run (its origin is the first tap of the session).
fn tap_now(app: &mut CantusApp) -> f64 {
    app.topbar
        .tap_origin
        .get_or_insert_with(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
}

/// A tap (the TAP key, or its shortcut): the tempo follows from the second tap of a run,
/// each run one undo step.
pub fn tap(app: &mut CantusApp) {
    let now = tap_now(app);
    tap_at(app, now);
}

/// [`tap`] at a given time on the tap clock (tests drive the clock).
pub fn tap_at(app: &mut CantusApp, now: f64) {
    if app.topbar.tap.taps() > 0 && app.topbar.tap.ended(now) {
        app.end_gesture();
    }
    let tap = app.topbar.tap.tap(now);
    if let Some(bpm) = tap.bpm {
        app.exec_merging(Command::SetTempo { bpm }, MergeKey::of(&"tap-tempo"));
        app.set_status(crate::i18n::tf(
            "topbar.tapped",
            &[("bpm", &format!("{bpm:.1}"))],
        ));
    } else {
        app.set_status(t("topbar.tap_more"));
    }
}

/// Once a frame: a run that has ended closes its undo step.
pub fn tap_tick(app: &mut CantusApp) {
    if app.topbar.tap.taps() == 0 {
        return;
    }
    let now = tap_now(app);
    tap_tick_at(app, now);
}

/// [`tap_tick`] at a given time on the tap clock.
pub fn tap_tick_at(app: &mut CantusApp, now: f64) {
    if app.topbar.tap.taps() > 0 && app.topbar.tap.ended(now) {
        app.topbar.tap.clear();
        app.end_gesture();
    }
}

// ---- the time signature --------------------------------------------------------------

/// The metres one click away.
const COMMON: [(u8, u8); 7] = [(2, 4), (3, 4), (4, 4), (5, 4), (6, 8), (7, 8), (12, 8)];
const NOTES: [u8; 4] = [2, 4, 8, 16];

/// Open the editor (screenshots).
pub fn open_metre(ctx: &egui::Context) {
    egui::Popup::open_id(ctx, metre_popup());
}

fn metre_popup() -> Id {
    Id::new("toolbar-metre-editor")
}

fn metre(app: &mut CantusApp, ui: &mut Ui, well: Rect, s: &Style) {
    let k = s.k;
    let resp = ui.interact(well, Id::new("toolbar-metre"), Sense::click());
    let open = egui::Popup::is_id_open(ui.ctx(), metre_popup());
    let ts = app.project().time_signature;
    let st = State::of(&resp, open);
    WORDS.with(|v| {
        let mut v = v.borrow_mut();
        let value = &mut v.1;
        value.clear();
        let _ = write!(value, "{ts}");
        if st.hovered {
            let caps = Caps {
                menu: true,
                ..Caps::NONE
            };
            hint::set_text(ui.ctx(), &t("topbar.metre_tip"), value, caps);
        }
        with_brush(ui.painter(), |b| {
            kit::well::paint(b, well, s, st, None);
            let _ = text::paint(
                b,
                well.shrink2(Vec2::new(4.0 * k, 0.0)),
                &Text::new(value, Role::Value, s.t.text)
                    .align(Align2::CENTER_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
    });
    let toggle = resp.clicked();
    if !open && !toggle {
        return;
    }
    egui::Popup::from_response(&resp)
        .id(metre_popup())
        .open_memory(toggle.then_some(egui::SetOpenCommand::Toggle))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .frame(crate::views::record_options::frame_for(s))
        .gap(4.0)
        .show(|ui| metre_editor(app, ui));
}

/// The time signature's editor: the beats with ‹ ›, the note value, the usual metres.
fn metre_editor(app: &mut CantusApp, ui: &mut Ui) {
    let s = Style::current();
    ui.spacing_mut().item_spacing = vec2(4.0 * s.k, 3.0 * s.k);
    let ts = app.project().time_signature;
    let mut next = ts;
    crate::views::record_options::band_row(ui, "topbar.metre", &s);
    ui.horizontal(|ui| {
        crate::views::record_options::label(ui, "topbar.metre_beats", &s);
        let mut beats = String::new();
        let _ = write!(beats, "{}", ts.numerator);
        let (_, c) =
            kit::widgets::menu_field(ui, &beats, true, false, &t("topbar.metre_beats"), 120.0);
        if c.prev {
            next.numerator = ts.numerator.saturating_sub(1).max(1);
        }
        if c.next {
            next.numerator = (ts.numerator + 1).min(32);
        }
    });
    ui.horizontal(|ui| {
        crate::views::record_options::label(ui, "topbar.metre_note", &s);
        let labels: Vec<String> = NOTES.iter().map(|n| n.to_string()).collect();
        let srcs: Vec<Src<'_>> = labels.iter().map(|l| Src::Str(l)).collect();
        let mut i = NOTES.iter().position(|n| *n == ts.denominator).unwrap_or(1);
        if kit::widgets::segmented(ui, &mut i, &srcs, &t("topbar.metre_note"), 160.0).changed() {
            next.denominator = NOTES[i];
        }
    });
    ui.horizontal(|ui| {
        crate::views::record_options::label(ui, "topbar.metre_common", &s);
        let labels: Vec<String> = COMMON.iter().map(|(n, d)| format!("{n}/{d}")).collect();
        let srcs: Vec<Src<'_>> = labels.iter().map(|l| Src::Str(l)).collect();
        let mut i = COMMON
            .iter()
            .position(|(n, d)| *n == ts.numerator && *d == ts.denominator)
            .unwrap_or(usize::MAX);
        let before = i;
        let w = crate::views::record_options::CONTROL_W;
        if kit::widgets::segmented(ui, &mut i, &srcs, &t("topbar.metre_common"), w).changed()
            && i != before
        {
            next = TimeSignature::new(COMMON[i].0, COMMON[i].1);
        }
    });
    if next != ts {
        app.exec(Command::SetTimeSignature {
            time_signature: next,
        });
    }
}
