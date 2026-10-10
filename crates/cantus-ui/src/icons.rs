//! Vector icons painted with shapes. The bundled fonts lack most symbol glyphs, and painted
//! icons look identical on every platform and scale with the UI.

use crate::design;
use crate::theme;
use cantus_core::TrackIcon;
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
    Glue,
    Duplicate,
    Playback,
    Loop,
    Metronome,
    Keyboard,
    Zoom,
    /// A linked clip (ADR 0037): two links of a chain.
    Link,
}

/// Paint `icon` centred in `rect` (designed for ~16 px, scales with the rect), over a
/// surface of colour `bg` — a couple of icons cut a gap out of a stroke to read correctly.
pub fn paint_on(p: &egui::Painter, rect: Rect, icon: Icon, color: Color32, bg: Color32) {
    let c = rect.center();
    let s = rect.height().min(rect.width()) * 0.5; // half-size
                                                   // `h` is the glyph's half-height; every icon is drawn as multiples of it, so this one
                                                   // number decides how much of the button the icons fill.
    let h = s * 0.72;
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
        Icon::Link => {
            let r = h * 0.42;
            for side in [-1.0f32, 1.0] {
                let (a, z) = if side < 0.0 {
                    (c.x - h, c.x + h * 0.18)
                } else {
                    (c.x - h * 0.18, c.x + h)
                };
                p.rect_stroke(
                    Rect::from_min_max(pos2(a, c.y - r), pos2(z, c.y + r)),
                    r,
                    thin,
                    egui::StrokeKind::Middle,
                );
            }
        }
        Icon::Glue => {
            // Two blocks driven together: the arrowheads are what say "join", because two
            // touching rectangles on their own read as one rectangle with a line in it.
            let gap = h * 0.18;
            for side in [-1.0f32, 1.0] {
                p.rect_filled(
                    Rect::from_min_max(
                        pos2((c.x + side * gap).min(c.x + side * h), c.y - h * 0.42),
                        pos2((c.x + side * h).max(c.x + side * gap), c.y + h * 0.42),
                    ),
                    1.0,
                    color,
                );
                p.add(Shape::convex_polygon(
                    vec![
                        pos2(c.x + side * h * 0.95, c.y - h * 0.95),
                        pos2(c.x + side * h * 0.35, c.y - h * 0.62),
                        pos2(c.x + side * h * 0.95, c.y - h * 0.3),
                    ],
                    color,
                    Stroke::NONE,
                ));
            }
        }
        Icon::Duplicate => {
            // Two clips, one behind the other: the back one outlined, the front one solid,
            // so the pair reads as a copy rather than as two unrelated blocks.
            let size = vec2(h * 1.5, h * 1.1);
            p.rect_stroke(
                Rect::from_min_size(pos2(c.x - h, c.y - h), size),
                1.0,
                thin,
                StrokeKind::Inside,
            );
            p.rect_filled(
                Rect::from_min_size(pos2(c.x - h * 0.3, c.y - h * 0.15), size),
                1.0,
                color,
            );
        }
        Icon::Loop => {
            // A cycle: a wide, flat stadium track, broken at the top right where the
            // arrowhead rides it. Wide and low so it does not read as a circle.
            let r = Rect::from_center_size(c, vec2(2.4 * h, h * 1.1));
            p.rect_stroke(r, r.height() * 0.5, thin, StrokeKind::Middle);
            // Cut the track under the head, so the head reads as motion and not a blob.
            p.line_segment(
                [pos2(c.x + h * 0.15, r.top()), pos2(c.x + h * 0.95, r.top())],
                Stroke::new(thin.width * 2.5, bg),
            );
            p.add(Shape::convex_polygon(
                vec![
                    pos2(c.x + h * 0.3, r.top() - h * 0.32),
                    pos2(c.x + h * 0.95, r.top()),
                    pos2(c.x + h * 0.3, r.top() + h * 0.32),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Icon::Metronome => {
            // A trapezoid case — not a triangle, which reads as a warning sign — with the
            // pendulum leaning inside it and its weight on the arm.
            p.add(Shape::convex_polygon(
                vec![
                    pos2(c.x - h * 0.36, c.y - h),
                    pos2(c.x + h * 0.36, c.y - h),
                    pos2(c.x + h * 0.92, c.y + h),
                    pos2(c.x - h * 0.92, c.y + h),
                ],
                Color32::TRANSPARENT,
                thin,
            ));
            p.line_segment(
                [
                    pos2(c.x - h * 0.06, c.y + h * 0.72),
                    pos2(c.x + h * 0.34, c.y - h * 0.78),
                ],
                thin,
            );
            p.rect_filled(
                Rect::from_center_size(pos2(c.x + h * 0.16, c.y - h * 0.1), vec2(h * 0.5, h * 0.3)),
                0.0,
                color,
            );
        }
        Icon::Zoom => {
            // A magnifier: the glass off-centre, the handle to the corner.
            let glass = pos2(c.x - h * 0.2, c.y - h * 0.2);
            p.circle_stroke(glass, h * 0.6, thin);
            p.line_segment(
                [
                    pos2(glass.x + h * 0.45, glass.y + h * 0.45),
                    pos2(c.x + h * 0.95, c.y + h * 0.95),
                ],
                Stroke::new(thin.width * 1.4, color),
            );
        }
        Icon::Keyboard => {
            // A piano keyboard: three white keys with two black ones between them, the
            // smallest arrangement that still reads as a keyboard and not a grid.
            let r = Rect::from_center_size(c, vec2(2.1 * h, h * 1.5));
            p.rect_stroke(r, 1.0, thin, StrokeKind::Inside);
            let key_w = r.width() / 3.0;
            for i in 1..3 {
                let x = r.left() + key_w * i as f32;
                p.line_segment([pos2(x, r.top()), pos2(x, r.bottom())], thin);
            }
            let black_w = key_w * 0.42;
            for i in 1..3 {
                let x = r.left() + key_w * i as f32 - black_w * 0.5;
                p.rect_filled(
                    Rect::from_min_size(pos2(x, r.top()), vec2(black_w, r.height() * 0.58)),
                    0.0,
                    color,
                );
            }
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

/// Paint a track's icon centred in `rect`.
///
/// Separate from [`Icon`] on purpose: those are *verbs* the toolbar can press, these are
/// *nouns* a track is. Mixing them would put "Kick drum" in a list of things you can click.
///
/// Each of the six is drawn from the same half-height `h` so they sit at one weight beside
/// each other, and each is chosen to be distinguishable at 14 px — which rules out, for
/// instance, drawing a bass and a guitar as two slightly different guitars.
pub fn paint_track_icon(p: &egui::Painter, rect: Rect, icon: TrackIcon, color: Color32) {
    let c = rect.center();
    let s = rect.height().min(rect.width()) * 0.5;
    let h = s * 0.78;
    let thin = Stroke::new((s * 0.16).max(1.0), color);
    match icon {
        TrackIcon::Drums => {
            // A drum with two sticks crossed above it. The first draft was a circle with
            // one diagonal through it, which at 14 px is the polarity Ø this app already
            // uses on every channel strip — an icon must not be another icon's twin.
            let head = Rect::from_center_size(pos2(c.x, c.y + h * 0.55), vec2(h * 1.9, h * 0.85));
            p.rect_stroke(head, h * 0.42, thin, StrokeKind::Inside);
            for dir in [-1.0f32, 1.0] {
                p.line_segment(
                    [
                        pos2(c.x + dir * h * 0.95, c.y - h * 1.0),
                        pos2(c.x - dir * h * 0.25, c.y + h * 0.15),
                    ],
                    thin,
                );
                p.circle_filled(
                    pos2(c.x + dir * h * 0.95, c.y - h * 1.0),
                    (s * 0.16).max(1.2),
                    color,
                );
            }
        }
        TrackIcon::Keys => {
            // Four white keys with two black stubs: unmistakably a keyboard even tiny.
            let w = h * 2.0 / 4.0;
            let top = c.y - h * 0.7;
            let bottom = c.y + h * 0.9;
            p.rect_stroke(
                Rect::from_min_max(pos2(c.x - h, top), pos2(c.x + h, bottom)),
                1.0,
                thin,
                StrokeKind::Inside,
            );
            for i in 1..4 {
                let x = c.x - h + w * i as f32;
                p.line_segment([pos2(x, top), pos2(x, bottom)], thin);
            }
            for i in [0usize, 2] {
                let x = c.x - h + w * (i as f32 + 1.0);
                p.rect_filled(
                    Rect::from_min_max(pos2(x - w * 0.28, top), pos2(x + w * 0.28, c.y + h * 0.1)),
                    0.0,
                    color,
                );
            }
        }
        TrackIcon::Strings => {
            // A body and a neck. Two circles rather than an outline, because an outlined
            // guitar at 14 px is a grey smudge.
            p.circle_stroke(pos2(c.x - h * 0.25, c.y + h * 0.45), h * 0.62, thin);
            p.circle_filled(pos2(c.x - h * 0.25, c.y + h * 0.45), h * 0.18, color);
            p.line_segment(
                [
                    pos2(c.x + h * 0.2, c.y - h * 0.05),
                    pos2(c.x + h * 0.85, c.y - h * 0.95),
                ],
                thin,
            );
            p.line_segment(
                [
                    pos2(c.x + h * 0.6, c.y - h * 1.05),
                    pos2(c.x + h * 1.0, c.y - h * 0.6),
                ],
                thin,
            );
        }
        TrackIcon::Vocal => {
            // A microphone: capsule, stem, foot.
            let cap = Rect::from_center_size(pos2(c.x, c.y - h * 0.35), vec2(h * 0.8, h * 1.25));
            p.rect_filled(cap, h * 0.4, color);
            p.line_segment([pos2(c.x, c.y + h * 0.5), pos2(c.x, c.y + h)], thin);
            p.line_segment(
                [pos2(c.x - h * 0.55, c.y + h), pos2(c.x + h * 0.55, c.y + h)],
                thin,
            );
        }
        TrackIcon::Synth => {
            // One cycle of a sine, which is what a synth track looks like in every DAW.
            let n = 18;
            let mut pts = Vec::with_capacity(n + 1);
            for i in 0..=n {
                let t = i as f32 / n as f32;
                let x = c.x - h + 2.0 * h * t;
                let y = c.y - (t * std::f32::consts::TAU).sin() * h * 0.72;
                pts.push(pos2(x, y));
            }
            p.add(Shape::line(pts, thin));
        }
        TrackIcon::Fx => {
            // A knob: a dial with a pointer, which reads as "something is being processed"
            // rather than as any particular effect.
            p.circle_stroke(c, h * 0.8, thin);
            p.line_segment([c, pos2(c.x + h * 0.5, c.y - h * 0.62)], thin);
        }
    }
}

/// Paint `icon` on the panel background. Use [`paint_on`] where the icon sits on a filled
/// surface, so the icons that cut a gap out of their own stroke cut the right colour.
pub fn paint(p: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    paint_on(p, rect, icon, color, theme::p().panel);
}

/// A clickable icon button. `active` paints it in the accent colour — that is *state*
/// (playing, recording, tool selected), not a press. `tip` is the tooltip, and should read
/// "Name (shortcut)".
///
/// The three interaction states are deliberately distinct: at rest the button is flush with
/// its panel, hover raises the fill one step, and a press deepens it. Keyboard focus is not
/// handled here — `design::paint_focus_ring` draws the ring for every widget in the app.
pub fn button(ui: &mut egui::Ui, icon: Icon, active: bool, tip: impl Into<String>) -> Response {
    let (rect, resp) = ui.allocate_exact_size(design::ICON_BUTTON, Sense::click());
    let pal = theme::p();
    let (fill, stroke) = if active {
        let lift = if resp.hovered() { 0.5 } else { 0.35 };
        (
            pal.gold.gamma_multiply(lift),
            Stroke::new(1.0, pal.gold_bright),
        )
    } else if resp.is_pointer_button_down_on() {
        (pal.grid_bar, Stroke::new(1.0, pal.grid_bar))
    } else if resp.hovered() {
        (pal.grid_strong, Stroke::new(1.0, pal.grid_bar))
    } else {
        (Color32::TRANSPARENT, Stroke::NONE)
    };
    ui.painter()
        .rect(rect, pal.corner_radius, fill, stroke, StrokeKind::Inside);
    let color = if active {
        pal.gold_bright
    } else if resp.hovered() {
        pal.text
    } else {
        pal.text.gamma_multiply(0.85)
    };
    // The cut-out icons need whatever is actually behind them, which is the button's own
    // fill once it has one.
    let behind = if fill == Color32::TRANSPARENT {
        pal.panel
    } else {
        pal.panel.lerp_to_gamma(fill, fill.a() as f32 / 255.0)
    };
    paint_on(
        ui.painter(),
        rect.shrink(design::space::S1),
        icon,
        color,
        behind,
    );
    resp.on_hover_text(tip.into())
}
