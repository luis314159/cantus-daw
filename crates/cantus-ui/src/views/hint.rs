//! The hint bar and the value pill (ADR 0024 §9).
//!
//! - **The hint panel** sits in the toolbar, FL's way: an inset panel of two lines — the
//!   project's name, dimmed, and the hint for whatever is under the pointer (its name, and
//!   its value, live while it is dragged; a meter says the level under the pointer) — with
//!   the capability icons at its right: automatable, has a menu, can be typed into, needs
//!   attention. Every kit widget writes its hint into the context (`kit::hint`); when
//!   nothing is hovered the second line shows the status message.
//! - **The value pill** follows the pointer, 12 pt to its right, **while any control is
//!   dragged** — better than FL, which shows the value only in the hint bar.
//!
//! Both read the hint slot once per frame and format nothing of their own, so a control
//! being dragged costs no allocation here.

use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{light, with_brush, Brush, Fill, Style, Tok};
use egui::{Align2, Context, Id, LayerId, Order, Pos2, Rect, Vec2};
use std::cell::RefCell;

/// The tokens the hint panel paints with (for inspect mode).
pub const PANEL_TOKENS: &[Tok] = &[Tok::Well, Tok::Text, Tok::TextDim, Tok::Focus];
/// The tokens the value pill paints with.
pub const PILL_TOKENS: &[Tok] = &[Tok::ValuePill, Tok::ValuePillText, Tok::Dark];

thread_local! {
    /// The hint read this frame, copied out of the slot (reused: no allocation once grown).
    static SEEN: RefCell<(String, String, Caps, bool)> =
        RefCell::new((String::with_capacity(48), String::with_capacity(24), Caps::NONE, false));
}

/// Copy the current hint into the thread's buffers and run `f` on it: `(name, value,
/// caps, live)`, or `None` when nothing is hovered.
fn with_hint<R>(ctx: &Context, f: impl FnOnce(Option<(&str, &str, Caps, bool)>) -> R) -> R {
    SEEN.with(|cell| {
        let mut seen = cell.borrow_mut();
        let got = hint::read(ctx, |h| match h {
            Some(h) => {
                seen.0.clear();
                seen.0.push_str(&h.name);
                seen.1.clear();
                seen.1.push_str(&h.value);
                seen.2 = h.caps;
                seen.3 = h.live;
                true
            }
            None => false,
        });
        if got {
            f(Some((&seen.0, &seen.1, seen.2, seen.3)))
        } else {
            f(None)
        }
    })
}

/// Paint the hint panel in `rect`: the project's name over the hint — or, when nothing is
/// hovered, the hint of the editor being worked in (the i18n key `idle`) — and the
/// capability icons at the right.
pub fn paint_panel(b: &mut Brush, rect: Rect, s: &Style, project: &str, idle: Option<&str>) {
    b.register(rect, "hint_panel", PANEL_TOKENS, &[]);
    let t = &s.t;
    let k = s.k;
    let tk = s.tk();
    let r = s.r(3.0);
    light::sunken(b, rect, r, t.well, 1.0, t.depth);
    let ctx = b.ctx().clone();
    with_hint(&ctx, |h| {
        let icons = h.map_or(0, |(_, _, c, _)| {
            [c.automatable, c.menu, c.typed, c.warning]
                .iter()
                .filter(|x| **x)
                .count()
        });
        let icon = 13.0 * k;
        let strip = if icons > 0 {
            icons as f32 * (icon + 4.0 * k) + 6.0 * k
        } else {
            0.0
        };
        let inner = rect.shrink2(Vec2::new(8.0 * k, 2.0 * k));
        let text_r =
            Rect::from_min_max(inner.min, Pos2::new(inner.right() - strip, inner.bottom()));
        // Line one, small and dim: the project. Line two: the hint.
        let split = text_r.height() * 0.4;
        let top = Rect::from_min_size(text_r.min, Vec2::new(text_r.width(), split));
        let bottom = Rect::from_min_max(Pos2::new(text_r.left(), text_r.top() + split), text_r.max);
        let _ = text::paint(
            b,
            top,
            &Text::new(project, Role::Micro, t.text_dim)
                .align(Align2::LEFT_BOTTOM)
                .policy(Policy::Elide(Elide::Middle)),
            tk,
        );
        match h {
            Some((name, value, caps, live)) => {
                // "Cutoff  2.4 kHz": the name, then the value (in the focus colour while
                // it is being dragged, FL's live readout).
                let mut vw = if value.is_empty() {
                    0.0
                } else {
                    text::measure(&ctx, &Text::new(value, Role::Value, t.text), tk).x + 10.0 * k
                };
                // When both do not fit, a live value keeps its room (the number being
                // dragged); otherwise the name does — a routing hint's modifiers are said
                // only here, while its value rides on the chip beside the pointer too.
                let nw =
                    text::measure(&ctx, &Text::new(name, Role::Label, t.text), tk).x + 10.0 * k;
                if !live && vw > 0.0 && nw + vw > bottom.width() {
                    vw = (bottom.width() - nw.min(bottom.width() * 0.8)).max(0.0);
                    if vw < 36.0 * k {
                        vw = 0.0;
                    }
                }
                let name_r = Rect::from_min_max(
                    bottom.min,
                    Pos2::new((bottom.right() - vw).max(bottom.left()), bottom.bottom()),
                );
                let _ = text::paint(
                    b,
                    name_r,
                    &Text::new(name, Role::Label, t.text)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End)),
                    tk,
                );
                if vw > 0.0 {
                    let vr = Rect::from_min_max(
                        Pos2::new(bottom.right() - vw + 6.0 * k, bottom.top()),
                        bottom.max,
                    );
                    let _ = text::paint(
                        b,
                        vr,
                        &Text::new(value, Role::Value, if live { t.focus } else { t.text })
                            .align(Align2::RIGHT_CENTER)
                            .policy(Policy::Elide(Elide::End)),
                        tk,
                    );
                }
                let mut x = rect.right() - 6.0 * k - icon;
                for (on, g) in [
                    (caps.warning, Glyph::Warning),
                    (caps.typed, Glyph::Typed),
                    (caps.menu, Glyph::MouseMenu),
                    (caps.automatable, Glyph::Automate),
                ] {
                    if on {
                        let gr = Rect::from_min_size(
                            Pos2::new(x, rect.center().y - icon * 0.5),
                            Vec2::splat(icon),
                        );
                        let ink = if g == Glyph::Warning {
                            t.danger
                        } else {
                            t.text_dim
                        };
                        glyph::paint(b, gr, g, ink);
                        x -= icon + 4.0 * k;
                    }
                }
            }
            None => {
                if let Some(key) = idle {
                    let _ = text::paint(
                        b,
                        bottom,
                        &Text::key(key, Role::Label, t.text_dim)
                            .align(Align2::LEFT_CENTER)
                            .policy(Policy::Elide(Elide::End)),
                        tk,
                    );
                }
            }
        }
    });
}

/// The value pill: while a control is dragged, its value rides 12 pt right of the pointer.
pub fn value_pill(ctx: &Context) {
    let Some(pos) = ctx.pointer_latest_pos() else {
        return;
    };
    if !ctx.input(|i| i.pointer.any_down()) {
        return;
    }
    with_hint(ctx, |h| {
        let Some((_, value, _, true)) = h else {
            return;
        };
        if value.is_empty() {
            return;
        }
        let s = Style::current();
        let t = s.t;
        let tk = s.tk();
        let k = s.k;
        let text = Text::new(value, Role::Value, t.value_pill_text)
            .align(Align2::CENTER_CENTER)
            .policy(Policy::Elide(Elide::End));
        let w = text::measure(ctx, &text, tk).x + 14.0 * k;
        let h = 20.0 * k;
        let screen = ctx.content_rect();
        let mut r = Rect::from_min_size(
            Pos2::new(pos.x + 12.0 * k, pos.y - h * 0.5),
            Vec2::new(w, h),
        );
        // Keep it on screen: flip to the pointer's left at the right edge.
        if r.right() > screen.right() - 2.0 {
            r = r.translate(Vec2::new(-(w + 24.0 * k), 0.0));
        }
        r = r.translate(Vec2::new(0.0, (screen.top() + 2.0 - r.top()).max(0.0)));
        r = r.translate(Vec2::new(
            0.0,
            (screen.bottom() - 2.0 - r.bottom()).min(0.0),
        ));
        let painter = ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("cantus-value-pill")));
        with_brush(&painter, |b| {
            b.register(r, "value_pill", PILL_TOKENS, &[]);
            let r = b.snap_rect(r);
            b.rrect_soft(
                r.translate(Vec2::new(0.0, 2.0 * k)),
                h * 0.5,
                Fill::Solid(with_alpha(t.dark_shade, 0.35)),
                6.0 * k,
            );
            b.rrect(r, h * 0.5, Fill::Solid(shade(t.value_pill, -0.18)));
            b.rrect(
                r.shrink(b.px()),
                h * 0.5 - b.px(),
                Fill::V(shade(t.value_pill, 0.04), shade(t.value_pill, -0.03)),
            );
            let _ = text::paint(b, r.shrink2(Vec2::new(7.0 * k, 0.0)), &text, tk);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pill_shows_only_while_a_control_is_dragged() {
        let ctx = Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        let input = |down: bool| egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
            events: vec![
                egui::Event::PointerMoved(Pos2::new(100.0, 100.0)),
                egui::Event::PointerButton {
                    pos: Pos2::new(100.0, 100.0),
                    button: egui::PointerButton::Primary,
                    pressed: down,
                    modifiers: Default::default(),
                },
            ],
            ..Default::default()
        };
        let pill_painted = |live: bool, down: bool| {
            let mut out = ctx.run_ui(input(down), |ui| {
                hint::set(ui.ctx(), "Cutoff", Caps::PARAM, live, |b| {
                    use std::fmt::Write as _;
                    write!(b, "2.4 kHz")
                });
                value_pill(ui.ctx());
            });
            out.textures_delta.clear();
            // Nothing else paints in this frame: any shape is the pill.
            out.shapes
                .iter()
                .any(|s| !matches!(s.shape, egui::Shape::Noop))
        };
        assert!(
            pill_painted(true, true),
            "dragging shows the value by the pointer"
        );
        assert!(!pill_painted(false, true), "a hover alone does not");
    }
}
