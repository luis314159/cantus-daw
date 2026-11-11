//! The position (ADR 0052; round 39, `dir-a`'s phase B): bar.beat large and thin (the clock
//! role, ExtraLight 30, ink 1), the ticks after it (15 Light, ink 3) on the same baseline,
//! no well; and beside them, full, the time (12, ink 2) over "bar · beat" (9 Medium, ink
//! 3). Compact, the time leaves. While recording the numbers turn record red and the take's
//! time takes the time's place. In pattern mode the clock counts from the pattern's start,
//! as FL's does, and its caption is lit in the accent.

use super::paint::{self as p, ink1, ink2, ink3};
use super::Style;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Role, Text};
use crate::kit::with_brush;
use crate::views;
use cantus_core::{Ticks, TimeSignature};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui};
use std::cell::RefCell;
use std::fmt::Write as _;

thread_local! {
    /// The clock's texts, rebuilt in place each frame (no allocation once grown).
    static CLOCK: RefCell<(String, String, String, String)> = RefCell::new((
        String::with_capacity(12),
        String::with_capacity(12),
        String::with_capacity(16),
        String::with_capacity(24),
    ));
}

/// The ticks' size and the time's, at 100 %.
const TICKS_PX: f32 = 15.0;
const TIME_PX: f32 = 12.0;
const CAPTION_PX: f32 = 9.0;

/// The digits of the beat and of the tick in `metre`: 10 beats or more take two, a beat of
/// 1 000 ticks or more (a half note) takes four.
fn digits(metre: TimeSignature) -> (usize, usize) {
    let beat = if metre.numerator >= 10 { 2 } else { 1 };
    let tick = if metre.ticks_per_beat().0 > 1_000 {
        4
    } else {
        3
    };
    (beat, tick)
}

/// How many digits the bar takes: the song's last bar and eight more, or the playhead's
/// bar past that; at least two.
pub fn bar_digits(app: &CantusApp) -> usize {
    let project = app.project();
    let now = crate::views::bars::bbt(project, app.engine.position()).0;
    let end = (crate::views::bars::bbt(project, project.content_end()).0 + 8).max(now);
    let mut n = end.max(10);
    let mut d = 0;
    while n > 0 {
        n /= 10;
        d += 1;
    }
    d
}

/// The widest position `metre` can show with `bars` bar digits, as bar.beat and .ticks
/// templates of eights.
fn template(metre: TimeSignature, bars: usize) -> (&'static str, &'static str) {
    const ONE: [&str; 5] = ["88.8", "888.8", "8888.8", "88888.8", "888888.8"];
    const TWO: [&str; 5] = ["88.88", "888.88", "8888.88", "88888.88", "888888.88"];
    let (beat, tick) = digits(metre);
    let i = bars.clamp(2, 6) - 2;
    (
        if beat == 1 { ONE[i] } else { TWO[i] },
        if tick == 3 { ".888" } else { ".8888" },
    )
}
/// The widths of bar.beat and of the ticks in `metre`.
fn digit_widths(ctx: &egui::Context, s: &Style, metre: TimeSignature, bars: usize) -> (f32, f32) {
    let (big, small) = template(metre, bars);
    let bw = text::measure(ctx, &Text::new(big, Role::Display, s.t.text), s.tk()).x;
    (
        bw + 1.0 * s.k,
        p::measure(ctx, small, Role::Hero, TICKS_PX, s),
    )
}

/// The time's column: the widest of the time, its caption and a take's time.
fn side_w(ctx: &egui::Context, s: &Style) -> f32 {
    p::measure(ctx, "88:88.88", Role::Body, TIME_PX, s)
        .max(p::measure_key(
            ctx,
            "topbar.clock_caption",
            Role::Control,
            CAPTION_PX,
            s,
        ))
        .max(p::measure(ctx, "888:88.8", Role::Label, 11.0, s) + 13.0 * s.k)
}

/// The clock's widths: compact (bar.beat and ticks) and full (the time beside them).
pub fn widths(ctx: &egui::Context, s: &Style, metre: TimeSignature, bars: usize) -> (f32, f32) {
    let (bw, tw) = digit_widths(ctx, s, metre, bars);
    let compact = bw + tw + 2.0 * s.k;
    (compact, compact + 12.0 * s.k + side_w(ctx, s) + 2.0 * s.k)
}

/// [`widths`]' full width (the property tests' older name).
pub fn width(ctx: &egui::Context, s: &Style, metre: TimeSignature, bars: usize) -> f32 {
    widths(ctx, s, metre, bars).1
}

/// The role a reading is painted in: `role` when `text` fits `width`, else `smaller` — a
/// reading longer than the clock was sized for (bar 1 000 on, 100 minutes on) steps down a
/// size and is never cut.
pub(crate) fn fitting(
    ctx: &egui::Context,
    s: &Style,
    text: &str,
    width: f32,
    role: Role,
    smaller: Role,
) -> Role {
    if text::measure(ctx, &Text::new(text, role, s.t.text), s.tk()).x <= width {
        role
    } else {
        smaller
    }
}

/// The words' box for a baseline at `base` (points from the bar's top) in a face `px` high:
/// the box's bottom a quarter of the size under the baseline.
fn on_baseline(r: Rect, x0: f32, x1: f32, base: f32, px: f32, s: &Style) -> Rect {
    let bottom = r.center().y - 22.0 * s.k + (base + px * 0.26) * s.k;
    Rect::from_min_max(
        Pos2::new(x0, bottom - px * 1.4 * s.k),
        Pos2::new(x1, bottom),
    )
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style, compact: bool) {
    let k = s.k;
    let widest = crate::views::bars::widest_metre(app.project());
    let (bw, tw) = digit_widths(ui.ctx(), s, widest, bar_digits(app));
    let (_, tick_digits) = digits(widest);
    let at = app.engine.position();
    let mut pos = at;
    // Pattern mode counts from the pattern's start.
    let pattern = app.project().pattern_mode;
    let region_start = pattern
        .then(|| app.topbar.pattern.region().map(|r| r.start))
        .flatten();
    if let Some(r) = region_start {
        pos = (pos - r).max(Ticks::ZERO);
    }
    // Through the metre changes and the tempo map (ADR 0060); pattern mode counts its own
    // bars in the song's first metre.
    let ts = if pattern {
        app.project().time_signature
    } else {
        crate::views::bars::map(app.project()).signature_at(pos)
    };
    let (bar, beat, tick) = if pattern {
        ts.bbt(pos)
    } else {
        crate::views::bars::bbt(app.project(), pos)
    };
    let secs = clock_seconds(app.project(), at, region_start);
    let resp = ui.interact(r, Id::new("toolbar-clock"), Sense::hover());
    let recording = app.is_recording() || app.rec.pass.is_some();
    let taking = app.rec.pass.is_some() || app.rec.dropped > 0;
    CLOCK.with(|c| {
        let mut c = c.borrow_mut();
        let (big, small, clock, hint_name) = &mut *c;
        big.clear();
        let _ = write!(big, "{bar}.{beat}");
        small.clear();
        let _ = write!(small, ".{tick:0tick_digits$}");
        clock.clear();
        let _ = write!(clock, "{}:{:05.2}", (secs / 60.0) as i64, secs % 60.0);
        if resp.hovered() {
            hint_name.clear();
            hint_name.push_str(&t("transport.position"));
            hint::set_text(ui.ctx(), hint_name, clock, Caps::NONE);
        }
        with_brush(ui.painter(), |b| {
            b.register(r, "clock", views::hint::PANEL_TOKENS, &[]);
            // r39:lapis2 — the position and the time are readouts (the logo's gold in Slate
            // Lapis); the ticks and the caption keep ink 3.
            let ink = if recording {
                s.t.danger
            } else {
                s.t.readout_or(ink1(s), false)
            };
            let x0 = r.left();
            // Bar.beat set right in its widest room, so the ticks never move; a position
            // longer than the room (bar 1 000 on) steps down a size rather than be cut.
            let br = on_baseline(r, x0, x0 + bw, 34.0, 30.0, s);
            let fits =
                text::measure(b.ctx(), &Text::new(big, Role::Display, ink), s.tk()).x <= bw + 0.5;
            if fits {
                let _ = text::paint(
                    b,
                    br,
                    &Text::new(big, Role::Display, ink).align(Align2::RIGHT_BOTTOM),
                    s.tk(),
                );
            } else {
                let br = on_baseline(r, x0, x0 + bw, 34.0, 18.0, s);
                let _ = text::paint(
                    b,
                    br,
                    &Text::new(big, Role::Hero, ink).align(Align2::RIGHT_BOTTOM),
                    s.tk(),
                );
            }
            let tr = on_baseline(r, x0 + bw, x0 + bw + tw + 2.0 * k, 34.0, TICKS_PX, s);
            let ti = if recording { s.t.danger } else { ink3(s) };
            p::words(
                b,
                tr,
                small,
                Role::Hero,
                TICKS_PX,
                ti,
                Align2::LEFT_BOTTOM,
                s,
            );
            if compact {
                return;
            }
            let sx = tr.right() + 12.0 * k;
            let side = Rect::from_min_max(Pos2::new(sx, r.top()), Pos2::new(r.right(), r.bottom()));
            let time_r = on_baseline(side, sx, side.right(), 20.0, TIME_PX, s);
            if taking {
                // Recording: the take's time and its pulsing lamp in the time's place (the
                // song's time is in the clock's hint).
                views::record_options::paint_elapsed(b, app, time_r, s);
            } else {
                // A time longer than its room (past 99 minutes) steps down a size or two
                // rather than be cut.
                let w = time_r.width();
                let px = [TIME_PX, 11.0, 9.0]
                    .into_iter()
                    .find(|px| p::measure(b.ctx(), clock, Role::Body, *px, s) <= w)
                    .unwrap_or(8.0);
                p::words(
                    b,
                    time_r,
                    clock,
                    Role::Body,
                    px,
                    s.t.readout_or(ink2(s), true),
                    Align2::LEFT_BOTTOM,
                    s,
                );
            }
            let cap_r = on_baseline(side, sx, side.right(), 33.0, CAPTION_PX, s);
            let ci = if pattern { s.t.highlight } else { ink3(s) };
            p::words_key(
                b,
                cap_r,
                "topbar.clock_caption",
                Role::Control,
                CAPTION_PX,
                ci,
                Align2::LEFT_BOTTOM,
                s,
            );
        });
    });
}

/// The clock's seconds at `pos` (the playhead): from the song's start, or in pattern mode
/// from the pattern's (`region_start`), through the tempo map where the music plays.
pub fn clock_seconds(
    project: &cantus_core::Project,
    pos: Ticks,
    region_start: Option<Ticks>,
) -> f64 {
    // The pattern plays where it lies in the song, at the tempo there (review 2: counted
    // from the song's start, a pattern at a faster bar ran at the first tempo).
    let from = region_start.unwrap_or(Ticks::ZERO);
    crate::views::bars::seconds_between(project, from, pos.max(from))
}
