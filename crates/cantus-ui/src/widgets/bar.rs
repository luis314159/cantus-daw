//! A bipolar bar: pan, width, and anything else read as a deviation from a centre.
//!
//! egui's slider is wrong for these twice over. It has no idea where the *meaningful*
//! value is, so a pan at dead centre looks exactly like a pan at 3 % — and it fills from
//! the left, so the bar's length reads as "how much", when what a pan control means is
//! "how far, and which way". This fills **from the detent outwards**, which is the shape
//! a console's pan display has had since consoles had displays.

use crate::design::space;
use crate::theme;
use egui::{
    pos2, vec2, CornerRadius, EventFilter, Key, Rect, Response, Sense, Stroke, StrokeKind, Ui,
    Widget, WidgetInfo,
};

/// Height of the bar.
const BAR_H: f32 = space::S3;
/// Half-width of the marker that rides the value.
const MARK_W: f32 = 3.0;
/// Fine mode: a fifth of the distance.
const FINE: f32 = 0.2;

/// A bipolar or unipolar bar with a detent.
pub struct Bar<'a> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    /// The value the bar fills out from, and where a double-click resets to.
    detent: f32,
    width: f32,
    name: String,
}

impl<'a> Bar<'a> {
    /// A bar over any range, filling out from `detent` — which is also where a
    /// double-click resets it to.
    ///
    /// [`pan`](Self::pan) and [`width`](Self::width) are this with the two shapes the
    /// mixer needs already filled in; a pad's level (0 dB in the middle of −24…+12) and a
    /// step's velocity are the same control with different numbers, and they must not be
    /// a second widget.
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>, detent: f32) -> Self {
        let (min, max) = (*range.start(), *range.end());
        Bar {
            value,
            min,
            max,
            detent: detent.clamp(min, max),
            width: space::GRID * 20.0,
            name: String::new(),
        }
    }

    /// A pan control: −1…1, centred.
    pub fn pan(value: &'a mut f32) -> Self {
        Bar {
            value,
            min: -1.0,
            max: 1.0,
            detent: 0.0,
            width: space::GRID * 20.0,
            name: String::new(),
        }
    }

    /// A width control: 0 (mono) … 2 (twice as wide), resting at 1.
    pub fn width(value: &'a mut f32) -> Self {
        Bar {
            value,
            min: 0.0,
            max: cantus_core::MAX_WIDTH,
            detent: 1.0,
            width: space::GRID * 20.0,
            name: String::new(),
        }
    }

    pub fn size(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

/// How a pan position reads: `C`, `L50`, `R100`. Monospaced at the call site, so the
/// readout does not jitter as it counts.
pub fn pan_text(pan: f32) -> String {
    if pan.abs() < 0.005 {
        "C".to_string()
    } else if pan < 0.0 {
        format!("L{:.0}", -pan * 100.0)
    } else {
        format!("R{:.0}", pan * 100.0)
    }
}

/// How a width reads: `0%`, `100%`, `160%`.
///
/// Always the number, never the word "mono" — the *button* next to it says that, and two
/// controls saying the same word in the same row is how a strip stops being readable.
pub fn width_text(width: f32) -> String {
    format!("{:.0}%", width * 100.0)
}

impl Widget for Bar<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Bar {
            value,
            min,
            max,
            detent,
            width,
            name,
        } = self;
        let (rect, mut response) =
            ui.allocate_exact_size(vec2(width, BAR_H), Sense::click_and_drag());
        let span = (max - min).max(1e-6);
        let start = value.clamp(min, max);
        let mut v = start;
        let fine = if ui.input(|i| i.modifiers.shift) {
            FINE
        } else {
            1.0
        };

        if response.dragged() {
            let dx = response.drag_delta().x;
            if dx != 0.0 {
                v += dx * fine * span / rect.width().max(1.0);
            }
        }
        if response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    EventFilter {
                        horizontal_arrows: true,
                        ..Default::default()
                    },
                );
            });
            super::claim_arrow_keys(ui.ctx(), response.id);
            let steps = ui.input(|i| {
                i.num_presses(Key::ArrowRight) as i32 - i.num_presses(Key::ArrowLeft) as i32
            });
            if steps != 0 {
                v += steps as f32 * span * 0.01 * if fine < 1.0 { 0.1 } else { 1.0 };
            }
        }
        // A pan you cannot put back in the middle is a pan you will fight all day. The
        // detent snaps a value that lands within a pixel of it, and a double-click puts
        // it there outright.
        let per_px = span / rect.width().max(1.0);
        if (v - detent).abs() < per_px {
            v = detent;
        }
        if response.double_clicked() {
            v = detent;
        }
        let v = v.clamp(min, max);
        if v != start {
            *value = v;
            response.mark_changed();
        }

        if ui.is_rect_visible(rect) {
            let pal = theme::p();
            let p = ui.painter();
            let radius = CornerRadius::same(2);
            p.rect_filled(rect, radius, theme::recess(&pal));
            let x_of = |val: f32| rect.left() + (val - min) / span * rect.width();
            let (dx, vx) = (x_of(detent), x_of(v));
            // The fill runs from the detent to the value, so its *direction* is the
            // reading and its length is the amount.
            if (vx - dx).abs() > 0.5 {
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(dx.min(vx), rect.top() + 1.0),
                        pos2(dx.max(vx), rect.bottom() - 1.0),
                    ),
                    radius,
                    pal.gold.gamma_multiply(0.75),
                );
            }
            p.vline(dx, rect.y_range(), Stroke::new(1.0, pal.text_dim));
            let mark = Rect::from_min_max(
                pos2(vx - MARK_W, rect.top() - 1.0),
                pos2(vx + MARK_W, rect.bottom() + 1.0),
            );
            p.rect_filled(mark, radius, pal.panel_raised);
            let edge = if response.dragged() || response.has_focus() {
                Stroke::new(2.0, pal.gold)
            } else if response.hovered() {
                Stroke::new(1.0, pal.text_dim)
            } else {
                Stroke::new(1.0, pal.grid_bar)
            };
            p.rect_stroke(mark, radius, edge, StrokeKind::Inside);
            p.rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, pal.grid_strong),
                StrokeKind::Inside,
            );
        }
        response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), v as f64, &name));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> egui::Context {
        let ctx = egui::Context::default();
        crate::design::install_style(&ctx);
        ctx
    }

    fn frame(ctx: &egui::Context, mut add: impl FnMut(&mut Ui)) -> Vec<egui::epaint::ClippedShape> {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, vec2(400.0, 200.0))),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| add(ui));
        out.textures_delta.clear();
        out.shapes
    }

    fn fills(shapes: &[egui::epaint::ClippedShape], color: egui::Color32) -> Vec<Rect> {
        fn walk(s: &egui::Shape, color: egui::Color32, out: &mut Vec<Rect>) {
            match s {
                egui::Shape::Rect(r) if r.fill == color => out.push(r.rect),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, color, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for s in shapes {
            walk(&s.shape, color, &mut out);
        }
        out
    }

    #[test]
    fn the_fill_runs_from_the_detent_and_says_which_way() {
        let ctx = ctx();
        let fill = theme::p().gold.gamma_multiply(0.75);
        // Centred: no fill at all, because there is nothing to say.
        let mut pan = 0.0f32;
        let shapes = frame(&ctx, |ui| {
            ui.add(Bar::pan(&mut pan).size(100.0));
        });
        assert!(fills(&shapes, fill).is_empty(), "a centred pan drew a fill");

        // Left and right fill on opposite sides of the same line.
        let mut pan = -0.5f32;
        let left = fills(
            &frame(&ctx, |ui| {
                ui.add(Bar::pan(&mut pan).size(100.0));
            }),
            fill,
        );
        let mut pan = 0.5f32;
        let right = fills(
            &frame(&ctx, |ui| {
                ui.add(Bar::pan(&mut pan).size(100.0));
            }),
            fill,
        );
        assert_eq!(left.len(), 1);
        assert_eq!(right.len(), 1);
        assert!(
            left[0].right() <= right[0].left() + 0.01,
            "L50 and R50 filled the same side: {left:?} {right:?}"
        );
        assert!(
            (left[0].width() - right[0].width()).abs() < 0.5,
            "asymmetric"
        );
    }

    #[test]
    fn the_readouts_say_what_the_control_means() {
        crate::i18n::set_locale(crate::i18n::Locale::En);
        assert_eq!(pan_text(0.0), "C");
        assert_eq!(pan_text(-0.5), "L50");
        assert_eq!(pan_text(1.0), "R100");
        assert_eq!(width_text(1.0), "100%");
        assert_eq!(width_text(1.6), "160%");
        assert_eq!(width_text(0.0), "0%");
    }

    #[test]
    fn width_rests_at_unity_and_pan_rests_at_the_centre() {
        // The detent is where a double-click lands and where a drag sticks; getting it
        // wrong on the width control would make "as recorded" unreachable by hand.
        let ctx = ctx();
        let mut w = 1.0f32;
        frame(&ctx, |ui| {
            ui.add(Bar::width(&mut w).size(100.0));
        });
        assert_eq!(w, 1.0, "drawing the width bar moved it");
        let mut pan = 0.0f32;
        frame(&ctx, |ui| {
            ui.add(Bar::pan(&mut pan).size(100.0));
        });
        assert_eq!(pan, 0.0);
    }
}
