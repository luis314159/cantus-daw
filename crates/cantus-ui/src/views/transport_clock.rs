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
use cantus_core::Ticks;
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
const SIDE: f32 = 84.0;

/// The widest the clock's digits get, in the `display` role.
fn digits_width(ctx: &egui::Context, s: &Style) -> f32 {
    text::measure(
        ctx,
        &Text::new("888.8.888", Role::Display, s.t.text),
        s.tk(),
    )
    .x + 24.0 * s.k
}

pub fn width(ctx: &egui::Context, s: &Style) -> f32 {
    digits_width(ctx, s) + SIDE * s.k
}

pub fn show(app: &mut CantusApp, ui: &mut Ui, r: Rect, s: &Style) {
    let k = s.k;
    let t = &s.t;
    let digits_w = digits_width(ui.ctx(), s);
    let lcd = Rect::from_center_size(r.center(), Vec2::new(r.width(), 40.0 * k));
    let mut pos = app.engine.position();
    // Pattern mode counts from the pattern's start.
    let pattern = app.project().pattern_mode;
    if pattern {
        if let Some(region) = app.topbar.pattern.region() {
            pos = (pos - region.start).max(Ticks::ZERO);
        }
    }
    let ts = app.project().time_signature;
    let (bar, beat, tick) = ts.bbt(pos);
    let secs = pos.to_seconds(app.project().tempo_bpm);
    let resp = ui.interact(lcd, Id::new("toolbar-clock"), Sense::hover());
    CLOCK.with(|c| {
        let mut c = c.borrow_mut();
        let (bbt, clock, metre, hint_name) = &mut *c;
        bbt.clear();
        let _ = write!(bbt, "{bar:>3}.{beat}.{tick:03}");
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
            let _ = text::paint(
                b,
                dr.shrink2(Vec2::new(12.0 * k, 0.0)),
                &Text::new(bbt, Role::Display, t.text)
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
            let _ = text::paint(
                b,
                top,
                &Text::new(clock, Role::Value, t.text_dim)
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
