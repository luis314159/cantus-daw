//! Vector icons painted with shapes. The bundled fonts lack most symbol glyphs, and painted
//! icons look identical on every platform and scale with the UI.

use crate::theme;
use egui::{pos2, vec2, Color32, Rect, Response, Sense, Shape, Stroke, StrokeKind};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Icon {
    ToStart,
    Play,
    Pause,
    Stop,
    Record,
    Draw,
    Paint,
    Select,
    Delete,
    Mute,
    Slice,
    Playback,
}

/// Paint `icon` centred in `rect` (designed for ~16 px, scales with the rect).
pub fn paint(p: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let c = rect.center();
    let s = rect.height().min(rect.width()) * 0.5; // half-size
    let h = s * 0.55;
    let thin = Stroke::new((s * 0.18).max(1.2), color);
    match icon {
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![
                    pos2(c.x - h * 0.8, c.y - h),
                    pos2(c.x + h * 1.1, c.y),
                    pos2(c.x - h * 0.8, c.y + h),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Pause => {
            let w = h * 0.55;
            p.rect_filled(
                Rect::from_center_size(pos2(c.x - w, c.y), vec2(w, 2.0 * h)),
                0.0,
                color,
            );
            p.rect_filled(
                Rect::from_center_size(pos2(c.x + w, c.y), vec2(w, 2.0 * h)),
                0.0,
                color,
            );
        }
        Icon::Stop => {
            p.rect_filled(
                Rect::from_center_size(c, vec2(2.0 * h, 2.0 * h)),
                1.0,
                color,
            );
        }
        Icon::Record => {
            p.circle_filled(c, h * 1.05, color);
        }
        Icon::ToStart => {
            p.rect_filled(
                Rect::from_center_size(pos2(c.x - h, c.y), vec2(h * 0.4, 2.0 * h)),
                0.0,
                color,
            );
            p.add(Shape::convex_polygon(
                vec![
                    pos2(c.x + h * 1.1, c.y - h),
                    pos2(c.x - h * 0.5, c.y),
                    pos2(c.x + h * 1.1, c.y + h),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Draw => {
            // Pencil: thick diagonal body with a tip.
            let body = Stroke::new((s * 0.38).max(2.0), color);
            p.line_segment(
                [
                    pos2(c.x - h * 0.6, c.y + h * 0.6),
                    pos2(c.x + h * 0.7, c.y - h * 0.7),
                ],
                body,
            );
            p.add(Shape::convex_polygon(
                vec![
                    pos2(c.x - h * 0.95, c.y + h * 0.95),
                    pos2(c.x - h * 0.45, c.y + h * 0.75),
                    pos2(c.x - h * 0.75, c.y + h * 0.45),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Paint => {
            // Three notes being laid down.
            let w = h * 0.7;
            for (i, dy) in [-0.75f32, 0.0, 0.75].iter().enumerate() {
                let x = c.x - h + i as f32 * w * 1.0;
                let r =
                    Rect::from_min_size(pos2(x, c.y + dy * h - h * 0.3), vec2(w * 0.9, h * 0.55));
                p.rect_filled(r, 1.0, color);
            }
        }
        Icon::Select => {
            // Dashed rectangle.
            let r = Rect::from_center_size(c, vec2(2.0 * h, 2.0 * h));
            let dash = h * 0.45;
            for (a, b) in [
                (r.left_top(), r.right_top()),
                (r.right_top(), r.right_bottom()),
                (r.right_bottom(), r.left_bottom()),
                (r.left_bottom(), r.left_top()),
            ] {
                let len = (b - a).length();
                let dir = (b - a) / len;
                let mut t = 0.0;
                while t < len {
                    let end = (t + dash).min(len);
                    p.line_segment([a + dir * t, a + dir * end], thin);
                    t += dash * 2.0;
                }
            }
        }
        Icon::Delete => {
            p.line_segment([pos2(c.x - h, c.y - h), pos2(c.x + h, c.y + h)], thin);
            p.line_segment([pos2(c.x - h, c.y + h), pos2(c.x + h, c.y - h)], thin);
        }
        Icon::Mute => {
            // A note with a slash through it.
            let r = Rect::from_center_size(c, vec2(2.0 * h, h * 0.9));
            p.rect_stroke(r, 1.0, thin, StrokeKind::Inside);
            p.line_segment(
                [pos2(c.x - h * 0.6, c.y + h), pos2(c.x + h * 0.6, c.y - h)],
                thin,
            );
        }
        Icon::Slice => {
            // A note cut in two by a vertical blade.
            let gap = h * 0.25;
            p.rect_filled(
                Rect::from_min_max(
                    pos2(c.x - h, c.y - h * 0.45),
                    pos2(c.x - gap, c.y + h * 0.45),
                ),
                1.0,
                color,
            );
            p.rect_filled(
                Rect::from_min_max(
                    pos2(c.x + gap, c.y - h * 0.45),
                    pos2(c.x + h, c.y + h * 0.45),
                ),
                1.0,
                color,
            );
            p.line_segment([pos2(c.x, c.y - h), pos2(c.x, c.y + h)], thin);
        }
        Icon::Playback => {
            // Small speaker: box + cone.
            p.rect_filled(
                Rect::from_min_max(
                    pos2(c.x - h, c.y - h * 0.4),
                    pos2(c.x - h * 0.3, c.y + h * 0.4),
                ),
                0.0,
                color,
            );
            p.add(Shape::convex_polygon(
                vec![
                    pos2(c.x - h * 0.3, c.y - h * 0.4),
                    pos2(c.x + h * 0.4, c.y - h),
                    pos2(c.x + h * 0.4, c.y + h),
                    pos2(c.x - h * 0.3, c.y + h * 0.4),
                ],
                color,
                Stroke::NONE,
            ));
            p.line_segment(
                [
                    pos2(c.x + h * 0.7, c.y - h * 0.5),
                    pos2(c.x + h * 0.7, c.y + h * 0.5),
                ],
                thin,
            );
        }
    }
}

/// A clickable icon button. `active` paints the icon in the accent colour; `tip` is the
/// tooltip (name + shortcut).
pub fn button(ui: &mut egui::Ui, icon: Icon, active: bool, tip: impl Into<String>) -> Response {
    let size = vec2(30.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let visuals = ui.style().interact(&resp);
    let pal = theme::p();
    let fill = if active {
        pal.gold.gamma_multiply(0.35)
    } else {
        visuals.bg_fill
    };
    ui.painter().rect(
        rect,
        3.0,
        fill,
        if active {
            Stroke::new(1.0, pal.gold)
        } else {
            visuals.bg_stroke
        },
        StrokeKind::Inside,
    );
    let color = if active { pal.gold_bright } else { pal.text };
    paint(ui.painter(), rect.shrink(5.0), icon, color);
    resp.on_hover_text(tip.into())
}
