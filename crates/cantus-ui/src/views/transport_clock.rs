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
    text::measure(
        ctx,
        &Text::new(template(metre), Role::Display, s.t.text),
        s.tk(),
    )
    .x + 24.0 * s.k
}

pub fn width(ctx: &egui::Context, s: &Style, metre: TimeSignature) -> f32 {
    digits_width(ctx, s, metre) + SIDE * s.k
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
    let lcd = Rect::from_center_size(r.center(), Vec2::new(r.width(), 40.0 * k));
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
            light::sunken(b, lcd, s.r(4.0), t.ink, 1.0, t.depth);
            let dr = Rect::from_min_size(lcd.min, Vec2::new(digits_w, lcd.height()));
            let digits = dr.shrink2(Vec2::new(12.0 * k, 0.0));
            // The clock is sized for three bar digits in this metre (the template, in the
            // same monospaced face): a longer position steps down a size.
            let role = if bbt.len() <= template(widest).len() {
                Role::Display
            } else {
                Role::Hero
            };
            let _ = text::paint(
                b,
                digits,
                &Text::new(bbt, role, t.text)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            let side = Rect::from_min_max(
                Pos2::new(dr.right(), lcd.top() + 4.0 * k),
                Pos2::new(lcd.right() - 8.0 * k, lcd.bottom() - 4.0 * k),
            );
            b.vline(
                b.snap_mid(side.left() - 2.0 * k),
                lcd.top() + 7.0 * k,
                lcd.bottom() - 7.0 * k,
                b.px(),
                with_alpha(t.text_dim, 0.3),
            );
            let half = side.height() * 0.5;
            let top = Rect::from_min_size(side.min, Vec2::new(side.width(), half));
            let bottom = Rect::from_min_max(Pos2::new(side.left(), side.top() + half), side.max);
            let role = fitting(b.ctx(), s, clock, top.width(), Role::Value, Role::Label);
            let _ = text::paint(
                b,
                top,
                &Text::new(clock, role, t.text_dim)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
            if app.rec.pass.is_some() || app.rec.dropped > 0 {
                // Recording: the take's time and a pulsing lamp in the metre's place.
                views::record_options::paint_elapsed(b, app, bottom, s);
            } else {
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
