//! The position (ADR 0052): bars.beats.ticks in the `display` role on an inset panel, the
//! time and the metre beside it. While recording, the take's time and a pulsing lamp take
//! the metre's place. In pattern mode the clock counts from the pattern's start, as FL's
//! does, and its metre is lit in the highlight.

use super::Style;
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{light, with_brush};
use crate::views;
use cantus_core::{Ticks, TimeSignature};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};
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

/// The side panel's width at 100 %.
const SIDE: f32 = 70.0;

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

/// The widest position `metre` can show, as a template of eights.
fn template(metre: TimeSignature) -> &'static str {
    match digits(metre) {
        (1, 3) => "888.8.888",
        (1, _) => "888.8.8888",
        (_, 3) => "888.88.888",
        _ => "888.88.8888",
    }
}

/// The widest the clock's digits get in `metre`, in the `display` role.
fn digits_width(ctx: &egui::Context, s: &Style, metre: TimeSignature) -> f32 {
    // r37:finish — as drawn: bar and beat in the display role, the ticks a size down.
    let tpl = template(metre);
    let (big, small) = tpl.split_at(tpl.rfind('.').unwrap_or(tpl.len()));
    let w = |x: &str, role| text::measure(ctx, &Text::new(x, role, s.t.text), s.tk()).x;
    w(big, Role::Display) + w(small, Role::Hero) + 24.0 * s.k
}

pub fn width(ctx: &egui::Context, s: &Style, metre: TimeSignature) -> f32 {
    // r37:finish — the time and the metre on a line under the digits, not in a column
    // beside them: a console's display, and the room for every group at 1920.
    let _ = SIDE;
    digits_width(ctx, s, metre)
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

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let t = &s.t;
    let widest = crate::views::bars::widest_metre(app.project());
    let digits_w = digits_width(ui.ctx(), s, widest);
    let (_, tick_digits) = digits(widest);
    // r37:finish — tall enough for the digits over the time line.
    let lcd = Rect::from_center_size(
        r.center(),
        Vec2::new(r.width(), (r.height() - 8.0 * k).clamp(40.0 * k, 56.0 * k)),
    );
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
    let resp = ui.interact(lcd, Id::new("toolbar-clock"), Sense::hover());
    CLOCK.with(|c| {
        let mut c = c.borrow_mut();
        let (bbt, clock, metre, hint_name) = &mut *c;
        bbt.clear();
        let _ = write!(bbt, "{bar:>3}.{beat}.{tick:0tick_digits$}");
        clock.clear();
        let _ = write!(clock, "{:02}:{:05.2}", (secs / 60.0) as i64, secs % 60.0);
        metre.clear();
        let _ = write!(metre, "{ts}");
        if resp.hovered() {
            hint_name.clear();
            hint_name.push_str(&t_("transport.position"));
            hint::set_text(ui.ctx(), hint_name, clock, Caps::NONE);
        }
        with_brush(ui.painter(), |b| {
            b.register(lcd, "clock", views::hint::PANEL_TOKENS, &[]);
            // r37:finish — a recessed display under glass.
            let m = light::Material::of(t);
            light::recess(b, lcd, s.r(4.0), t.ink, &m, 1.0, k);
            light::glass(b, b.snap_rect(lcd).shrink(b.px()), s.r(3.0), &m, 1.0);
            // r37:finish — the digits in the upper part, the time line under them.
            let line_h = 14.0 * k;
            let dr = Rect::from_min_max(
                lcd.min,
                Pos2::new(lcd.left() + digits_w, lcd.bottom() - line_h - 2.0 * k),
            );
            let digits = dr.shrink2(Vec2::new(12.0 * k, 0.0));
            // The clock is sized for three bar digits in this metre (the template, in the
            // same monospaced face): a longer position steps down a size.
            let role = if bbt.len() <= template(widest).len() {
                Role::Display
            } else {
                Role::Hero
            };
            // r37:finish — bar and beat large, the ticks a size down beside them (read the
            // bar from across the room, the ticks when you look).
            let split = bbt.rfind('.').filter(|_| role == Role::Display);
            if let Some(at) = split {
                let (big, small) = bbt.split_at(at);
                let st = Text::new(small, Role::Hero, with_alpha(t.text, 0.8))
                    .align(Align2::RIGHT_BOTTOM);
                let sw = text::measure(b.ctx(), &st, s.tk()).x;
                let foot = digits.bottom() - 8.5 * k;
                let sr = Rect::from_min_max(
                    Pos2::new(digits.right() - sw, digits.top()),
                    Pos2::new(digits.right(), foot),
                );
                let _ = text::paint(b, sr, &st, s.tk());
                let br = Rect::from_min_max(digits.min, Pos2::new(sr.left(), foot + 3.0 * k));
                let _ = text::paint(
                    b,
                    br,
                    &Text::new(big, Role::Display, t.text)
                        .align(Align2::RIGHT_BOTTOM)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
            } else {
                let _ = text::paint(
                    b,
                    digits,
                    &Text::new(bbt, role, t.text)
                        .align(Align2::RIGHT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
            }
            // The line under the digits: the time at the left, the metre (or the take's
            // time while recording) at the right, a hairline above them.
            let line = Rect::from_min_max(
                Pos2::new(digits.left(), lcd.bottom() - line_h - 3.0 * k),
                Pos2::new(digits.right(), lcd.bottom() - 3.0 * k),
            );
            b.hline(
                line.left(),
                line.right(),
                b.snap_mid(line.top() - 0.5 * k),
                b.px(),
                with_alpha(t.text_dim, 0.22),
            );
            if app.rec.pass.is_some() || app.rec.dropped > 0 {
                // Recording: the take's time and its pulsing lamp take the whole line (the
                // song's time is in the clock's hint).
                views::record_options::paint_elapsed(b, app, line, s);
            } else {
                let half = line.width() * 0.58;
                let top = Rect::from_min_size(line.min, Vec2::new(half, line.height()));
                let bottom =
                    Rect::from_min_max(Pos2::new(line.left() + half, line.top()), line.max);
                let role = fitting(b.ctx(), s, clock, top.width(), Role::Value, Role::Label);
                let _ = text::paint(
                    b,
                    top,
                    &Text::new(clock, role, t.text_dim)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
                let ink = if pattern { t.highlight } else { t.text_dim };
                let _ = text::paint(
                    b,
                    bottom,
                    &Text::new(metre, Role::Value, ink)
                        .align(Align2::RIGHT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    s.tk(),
                );
            }
        });
    });
}

fn t_(key: &str) -> String {
    t(key)
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
