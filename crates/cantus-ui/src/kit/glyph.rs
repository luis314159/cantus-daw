//! The kit's small vector glyphs, drawn with the brush's primitives on the pixel grid:
//! the ▸ menu arrow every window has at its top-left (FL), chevrons, steppers, the star,
//! the power lamp's symbol, undo and redo, the FX bulb, phase and swap, the resize grip.
//! Each fits a square box and scales with it.

use super::brush::{Brush, Fill};
use egui::{pos2, Color32, Pos2, Rect, Vec2};
use std::f32::consts::PI;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    /// ▸ a window's or a slot's menu.
    Menu,
    /// ▾ a field that opens a list.
    Down,
    /// ‹
    Prev,
    /// ›
    Next,
    /// ☆ / ★
    Star,
    StarFilled,
    /// ⋯
    More,
    /// ?
    Help,
    /// ⏻
    Power,
    /// ↶ ↷
    Undo,
    Redo,
    /// ⤢ size
    Resize,
    /// ✕
    Close,
    /// The FX-on bulb.
    Bulb,
    /// ● record
    Record,
    /// Ø invert phase
    Phase,
    /// ⇄ swap left and right
    Swap,
    /// ▲ send to this strip
    Send,
    /// + add
    Plus,
    /// ◢ resize grip: three diagonal lines
    Grip,
    /// ✓ in a menu's check column
    Check,
    /// A device's mark: a small wave in a circle.
    Device,
    // --- the workspace's (ADR 0024 §5, §9): transport, windows, the hint bar ---
    /// ▶ play
    Play,
    /// ❚❚ pause
    Pause,
    /// ■ stop
    Stop,
    /// ⏮ back to the start
    ToStart,
    /// ⟲ loop
    Loop,
    /// The metronome's triangle and arm.
    Metronome,
    /// A small piano keyboard: the typing keyboard plays notes.
    Keyboard,
    /// The arrangement: clips on lanes.
    Arrangement,
    /// The piano roll: notes beside keys.
    PianoRoll,
    /// The channel rack: rows of steps.
    Rack,
    /// The mixer: three faders.
    Mixer,
    /// The inspector: a panel at the right.
    Inspector,
    /// The browser: a panel at the left.
    Browser,
    /// ⚙ settings
    Gear,
    /// The eyedropper: pick an element and recolour it.
    Pick,
    /// ⚠ something needs attention
    Warning,
    /// A mouse with its right button lit: has a right-click menu.
    MouseMenu,
    /// An automation envelope: can be automated.
    Automate,
    /// A text cursor: a value can be typed.
    Typed,
    /// ↔ ⌕ the interface's zoom.
    Zoom,
    // --- a device window's own buttons (round 20) ---
    /// ‒ minimise: a bar at the foot of the box.
    Minimise,
    /// □ maximise: a window with a thick title edge.
    Maximise,
    /// ❐ restore: two windows, one behind the other.
    Restore,
    /// Swap the device for another: two cards with a turning arrow between them.
    Replace,
    /// A linked clip (ADR 0037): two links of a chain, one through the other.
    Link,
    // --- the mixer's routing (round 25) ---
    /// A key on its side: a sidechain key (a key cable's end, a matrix key cell).
    Key,
    /// Two ports and a cable between them: the desk's cable view.
    Cables,
    /// A funnel: a view's filter options (the cable view's options menu).
    Filter,
    // --- warping (round 29, ADR 0061) ---
    /// A warp marker over a stretched wave: the warp editor, *Quantise audio*.
    Warp,
    // --- isolation (round 29, ADR 0063) ---
    /// A plug pulled apart, a gap between its two halves: a hosted plug-in that stopped
    /// working (the crash card, a slot that stopped).
    PluginStopped,
    // --- controllers (round 29, ADR 0062) ---
    /// A hardware controller: a knob with its pointer beside a fader and its cap — MIDI
    /// learn, the mappings pane.
    Controller,
}

/// Paint `g` centred in `rect` (its smaller side is the glyph's size).
pub fn paint(b: &mut Brush, rect: Rect, g: Glyph, color: Color32) {
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let c = pos2(b.snap_mid(c.x), b.snap_mid(c.y));
    let u = s / 12.0; // the glyph grid: 12 units across
    let p = |x: f32, y: f32| c + Vec2::new(x * u, y * u);
    let w = (1.25 * u).max(b.px());
    match g {
        Glyph::Menu => b.polygon(&[p(-2.0, -4.0), p(3.0, 0.0), p(-2.0, 4.0)], color),
        Glyph::Down => b.polygon(&[p(-4.0, -2.0), p(4.0, -2.0), p(0.0, 3.0)], color),
        Glyph::Prev => {
            b.segment(p(1.5, -4.0), p(-2.0, 0.0), w, color, true);
            b.segment(p(-2.0, 0.0), p(1.5, 4.0), w, color, true);
        }
        Glyph::Next => {
            b.segment(p(-1.5, -4.0), p(2.0, 0.0), w, color, true);
            b.segment(p(2.0, 0.0), p(-1.5, 4.0), w, color, true);
        }
        Glyph::Star | Glyph::StarFilled => {
            let mut pts = [Pos2::ZERO; 10];
            for (i, q) in pts.iter_mut().enumerate() {
                let a = -PI / 2.0 + i as f32 * PI / 5.0;
                let r = if i % 2 == 0 { 5.0 } else { 2.1 };
                *q = p(r * a.cos(), r * a.sin() + 0.4);
            }
            if g == Glyph::StarFilled {
                // Convex pieces: the pentagon and its five points.
                b.polygon(&[pts[1], pts[3], pts[5], pts[7], pts[9]], color);
                for i in 0..5 {
                    let tip = pts[2 * i];
                    let l = pts[(2 * i + 9) % 10];
                    let r = pts[2 * i + 1];
                    b.polygon(&[l, tip, r], color);
                }
            } else {
                for i in 0..10 {
                    b.segment(pts[i], pts[(i + 1) % 10], w * 0.85, color, true);
                }
            }
        }
        Glyph::More => {
            for x in [-3.5, 0.0, 3.5] {
                b.circle(p(x, 0.0), 1.1 * u, Fill::Solid(color));
            }
        }
        Glyph::Help => {
            b.arc(p(0.0, -1.5), 2.6 * u, -PI * 0.95, PI * 0.35, w, color, true);
            b.segment(p(0.9, 0.9), p(0.0, 2.0), w, color, true);
            b.circle(p(0.0, 4.4), 0.95 * u, Fill::Solid(color));
        }
        Glyph::Power => {
            b.arc(p(0.0, 0.8), 4.2 * u, -PI * 0.28, PI * 1.28, w, color, true);
            b.segment(p(0.0, -4.8), p(0.0, 0.2), w, color, true);
        }
        Glyph::Undo | Glyph::Redo => {
            let m = if g == Glyph::Undo { 1.0 } else { -1.0 };
            let (a0, a1) = if g == Glyph::Undo {
                (-PI * 0.9, PI * 0.35)
            } else {
                (PI * 0.65, PI * 1.9)
            };
            b.arc(p(0.0, 0.8), 3.6 * u, a0, a1, w, color, true);
            b.polygon(
                &[p(-5.8 * m, -1.0), p(-1.8 * m, -2.2), p(-3.4 * m, 1.8)],
                color,
            );
        }
        Glyph::Resize => {
            b.segment(p(-3.5, 3.5), p(3.5, -3.5), w, color, true);
            b.polygon(&[p(4.5, -4.5), p(0.5, -4.5), p(4.5, -0.5)], color);
            b.polygon(&[p(-4.5, 4.5), p(-0.5, 4.5), p(-4.5, 0.5)], color);
        }
        Glyph::Close => {
            b.segment(p(-3.5, -3.5), p(3.5, 3.5), w, color, true);
            b.segment(p(-3.5, 3.5), p(3.5, -3.5), w, color, true);
        }
        Glyph::Bulb => {
            b.circle(p(0.0, -1.2), 3.6 * u, Fill::Solid(color));
            b.rrect(
                Rect::from_center_size(p(0.0, 3.4), Vec2::new(3.8 * u, 2.6 * u)),
                0.8 * u,
                Fill::Solid(color),
            );
        }
        Glyph::Record => b.circle(c, 3.0 * u, Fill::Solid(color)),
        Glyph::Phase => {
            b.circle_stroke(c, 3.4 * u, w, color);
            b.segment(p(-4.2, 4.2), p(4.2, -4.2), w, color, true);
        }
        Glyph::Swap => {
            b.segment(p(-4.0, -1.8), p(3.0, -1.8), w, color, false);
            b.polygon(&[p(5.0, -1.8), p(2.0, -4.2), p(2.0, 0.6)], color);
            b.segment(p(4.0, 1.8), p(-3.0, 1.8), w, color, false);
            b.polygon(&[p(-5.0, 1.8), p(-2.0, -0.6), p(-2.0, 4.2)], color);
        }
        Glyph::Send => b.polygon(&[p(-4.0, 2.5), p(0.0, -2.5), p(4.0, 2.5)], color),
        Glyph::Plus => {
            b.segment(p(-4.0, 0.0), p(4.0, 0.0), w, color, false);
            b.segment(p(0.0, -4.0), p(0.0, 4.0), w, color, false);
        }
        Glyph::Grip => {
            for d in [0.0, 3.0, 6.0] {
                b.segment(p(5.0 - d, 5.0), p(5.0, 5.0 - d), w * 0.8, color, false);
            }
        }
        Glyph::Check => {
            b.segment(p(-4.0, 0.2), p(-1.2, 3.2), w, color, true);
            b.segment(p(-1.2, 3.2), p(4.2, -3.6), w, color, true);
        }
        Glyph::Device => {
            b.circle_stroke(c, 5.0 * u, w * 0.9, color);
            let mut pts = [Pos2::ZERO; 13];
            for (i, q) in pts.iter_mut().enumerate() {
                let x = -3.2 + 6.4 * i as f32 / 12.0;
                *q = p(x, -2.0 * (x * 1.25).sin());
            }
            b.polyline(&pts, w, color);
        }
        _ => workspace(b, g, c, u, w, color),
    }
}

/// The workspace's glyphs (ADR 0024 §5, §9), on the same 12-unit grid.
fn workspace(b: &mut Brush, g: Glyph, c: Pos2, u: f32, w: f32, color: Color32) {
    let p = |x: f32, y: f32| c + Vec2::new(x * u, y * u);
    let bar = |b: &mut Brush, x0: f32, y0: f32, x1: f32, y1: f32| {
        b.rect(Rect::from_min_max(p(x0, y0), p(x1, y1)), Fill::Solid(color));
    };
    match g {
        Glyph::Play => b.polygon(&[p(-3.2, -4.6), p(4.6, 0.0), p(-3.2, 4.6)], color),
        Glyph::Pause => {
            bar(b, -3.6, -4.2, -1.0, 4.2);
            bar(b, 1.0, -4.2, 3.6, 4.2);
        }
        Glyph::Stop => b.rrect(
            Rect::from_center_size(c, Vec2::splat(8.0 * u)),
            0.8 * u,
            Fill::Solid(color),
        ),
        Glyph::ToStart => {
            bar(b, -4.4, -4.2, -2.8, 4.2);
            b.polygon(&[p(4.4, -4.2), p(4.4, 4.2), p(-2.2, 0.0)], color);
        }
        Glyph::Loop => {
            b.arc(p(0.0, 0.0), 3.8 * u, PI * 0.15, PI * 1.05, w, color, true);
            b.arc(p(0.0, 0.0), 3.8 * u, PI * 1.15, PI * 2.05, w, color, true);
            b.polygon(&[p(-5.6, -0.6), p(-2.0, -0.6), p(-3.8, 2.0)], color);
            b.polygon(&[p(5.6, 0.6), p(2.0, 0.6), p(3.8, -2.0)], color);
        }
        Glyph::Metronome => {
            b.segment(p(-3.0, 4.2), p(-1.2, -4.4), w, color, true);
            b.segment(p(3.0, 4.2), p(1.2, -4.4), w, color, true);
            b.segment(p(-1.2, -4.4), p(1.2, -4.4), w, color, true);
            b.segment(p(-3.8, 4.2), p(3.8, 4.2), w, color, true);
            b.segment(p(0.0, 2.6), p(3.6, -3.2), w * 0.9, color, true);
        }
        Glyph::Keyboard => {
            b.rrect_stroke(
                Rect::from_min_max(p(-5.0, -3.4), p(5.0, 3.4)),
                0.8 * u,
                w * 0.8,
                color,
            );
            for x in [-2.5, 0.0, 2.5] {
                b.segment(p(x, 0.4), p(x, 3.0), w * 0.7, color, false);
            }
            for x in [-3.6, -1.2, 1.2, 3.6] {
                bar(b, x - 0.5, -3.0, x + 0.5, 0.6);
            }
        }
        Glyph::Arrangement => {
            for (i, (x0, x1)) in [(-5.0, 0.5), (-2.0, 5.0), (-5.0, 2.5)].iter().enumerate() {
                let y = -3.4 + i as f32 * 3.4;
                b.rrect(
                    Rect::from_min_max(p(*x0, y - 1.1), p(*x1, y + 1.1)),
                    0.5 * u,
                    Fill::Solid(color),
                );
            }
        }
        Glyph::PianoRoll => {
            for i in 0..4 {
                let y = -4.2 + i as f32 * 2.3;
                bar(b, -5.2, y, -3.4, y + 1.5);
            }
            bar(b, -1.6, -3.4, 2.2, -1.8);
            bar(b, 0.4, -0.4, 4.8, 1.2);
            bar(b, -1.6, 2.4, 1.0, 4.0);
        }
        Glyph::Rack => {
            for row in 0..3 {
                for col in 0..4 {
                    let x = -4.8 + col as f32 * 2.6;
                    let y = -4.0 + row as f32 * 3.0;
                    let on = (row + col) % 3 == 0;
                    let r = Rect::from_min_max(p(x, y), p(x + 1.9, y + 2.2));
                    if on {
                        b.rect(r, Fill::Solid(color));
                    } else {
                        b.rrect_stroke(r, 0.0, w * 0.6, color);
                    }
                }
            }
        }
        Glyph::Mixer => {
            for (x, cap) in [(-3.6, 1.6), (0.0, -2.0), (3.6, 0.4)] {
                b.segment(p(x, -4.8), p(x, 4.8), w * 0.7, color, false);
                bar(b, x - 1.5, cap - 1.0, x + 1.5, cap + 1.0);
            }
        }
        Glyph::Inspector | Glyph::Browser => {
            let r = Rect::from_min_max(p(-5.0, -4.2), p(5.0, 4.2));
            b.rrect_stroke(r, 0.8 * u, w * 0.8, color);
            let (x0, x1) = if g == Glyph::Inspector {
                (1.4, 5.0)
            } else {
                (-5.0, -1.4)
            };
            bar(b, x0, -4.2, x1, 4.2);
        }
        Glyph::Gear => {
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                let (s, co) = a.sin_cos();
                b.segment(
                    p(co * 3.0, s * 3.0),
                    p(co * 5.0, s * 5.0),
                    w * 1.5,
                    color,
                    false,
                );
            }
            b.circle_stroke(c, 3.2 * u, w * 1.1, color);
        }
        Glyph::Pick => {
            b.segment(p(-4.4, 4.4), p(1.0, -1.0), w * 1.1, color, true);
            b.polygon(
                &[p(-0.4, -2.6), p(2.6, 0.4), p(4.6, -1.6), p(1.6, -4.6)],
                color,
            );
            b.circle(p(3.4, -3.4), 1.9 * u, Fill::Solid(color));
        }
        Glyph::Warning => {
            b.polygon(&[p(0.0, -5.0), p(5.4, 4.4), p(-5.4, 4.4)], color);
        }
        Glyph::MouseMenu => {
            let r = Rect::from_min_max(p(-3.4, -4.8), p(3.4, 4.8));
            b.rrect_stroke(r, 3.2 * u, w * 0.8, color);
            b.segment(p(0.0, -4.8), p(0.0, -1.0), w * 0.7, color, false);
            b.polygon(
                &[p(0.5, -4.2), p(2.9, -3.0), p(2.9, -1.0), p(0.5, -1.0)],
                color,
            );
        }
        Glyph::Automate => {
            let pts = [p(-5.0, 3.0), p(-2.0, -3.0), p(1.0, 1.0), p(5.0, -3.8)];
            b.polyline(&pts, w, color);
            for q in pts {
                b.circle(q, 1.1 * u, Fill::Solid(color));
            }
        }
        Glyph::Typed => {
            b.segment(p(0.0, -4.4), p(0.0, 4.4), w, color, false);
            b.segment(p(-2.2, -4.4), p(2.2, -4.4), w, color, true);
            b.segment(p(-2.2, 4.4), p(2.2, 4.4), w, color, true);
        }
        Glyph::Zoom => {
            b.circle_stroke(p(-1.0, -1.0), 3.4 * u, w, color);
            b.segment(p(1.6, 1.6), p(4.6, 4.6), w * 1.3, color, true);
        }
        _ => window(b, g, c, u, w, color),
    }
}

/// A device window's own buttons (round 20): minimise, maximise, restore, replace.
fn window(b: &mut Brush, g: Glyph, c: Pos2, u: f32, w: f32, color: Color32) {
    let p = |x: f32, y: f32| c + Vec2::new(x * u, y * u);
    match g {
        Glyph::Minimise => {
            b.rect(
                Rect::from_min_max(p(-4.5, 2.6), p(4.5, 2.6) + Vec2::new(0.0, w * 1.15)),
                Fill::Solid(color),
            );
        }
        Glyph::Maximise => {
            let r = Rect::from_min_max(p(-4.5, -4.5), p(4.5, 4.5));
            b.rrect_stroke(r, 0.6 * u, w, color);
            b.rect(
                Rect::from_min_max(r.min, Pos2::new(r.max.x, r.min.y + w * 1.6)),
                Fill::Solid(color),
            );
        }
        Glyph::Restore => {
            // The window behind shows only where the front one does not cover it.
            b.segment(p(-2.0, -2.2), p(-2.0, -5.0), w, color, false);
            b.segment(p(-2.0, -5.0), p(5.0, -5.0), w, color, false);
            b.segment(p(5.0, -5.0), p(5.0, 2.0), w, color, false);
            b.segment(p(5.0, 2.0), p(2.4, 2.0), w, color, false);
            let front = Rect::from_min_max(p(-5.0, -2.0), p(2.4, 5.0));
            b.rrect_stroke(front, 0.5 * u, w, color);
            b.rect(
                Rect::from_min_max(front.min, Pos2::new(front.max.x, front.min.y + w * 1.5)),
                Fill::Solid(color),
            );
        }
        Glyph::Replace => {
            // Two cards, and an arrow turning from one to the other above them.
            let a = Rect::from_min_max(p(-5.2, -0.6), p(-1.2, 5.0));
            let z = Rect::from_min_max(p(1.2, -0.6), p(5.2, 5.0));
            b.rrect_stroke(a, 0.6 * u, w * 0.9, color);
            b.rrect(z, 0.6 * u, Fill::Solid(color));
            b.arc(p(0.0, -0.4), 3.4 * u, PI * 1.12, PI * 1.82, w, color, true);
            b.polygon(&[p(2.0, -4.6), p(4.6, -2.6), p(1.4, -1.8)], color);
        }
        Glyph::Link => {
            // Two rounded links on a slant, overlapping in the middle: a chain.
            let r = 1.9 * u;
            b.rrect_stroke(
                Rect::from_min_max(p(-5.4, -1.9), p(0.9, 1.9)),
                r,
                w * 0.85,
                color,
            );
            b.rrect_stroke(
                Rect::from_min_max(p(-0.9, -1.9), p(5.4, 1.9)),
                r,
                w * 0.85,
                color,
            );
        }
        Glyph::Key => {
            // A key on its side (round 25): the bow at the left, the shaft, two teeth.
            b.circle_stroke(p(-2.8, 0.0), 2.4 * u, w * 0.9, color);
            b.segment(p(-0.4, 0.0), p(5.2, 0.0), w, color, false);
            b.segment(p(3.0, 0.0), p(3.0, 2.4), w, color, false);
            b.segment(p(4.8, 0.0), p(4.8, 1.8), w, color, false);
        }
        Glyph::Cables => {
            // Two ports and the cable hanging between them (round 25): the desk's cable
            // view.
            let mut pts = [Pos2::ZERO; 9];
            for (i, q) in pts.iter_mut().enumerate() {
                let a = PI * i as f32 / 8.0;
                *q = p(-4.2 * a.cos(), -2.6 + 6.4 * a.sin());
            }
            b.polyline(&pts, w, color);
            b.dot(p(-4.2, -3.2), 1.7 * u, color);
            b.dot(p(4.2, -3.2), 1.7 * u, color);
        }
        Glyph::Warp => {
            // A flag on its staff over a wave pulled out to the right.
            b.polygon(&[p(-1.6, -5.0), p(1.6, -5.0), p(0.0, -2.6)], color);
            b.segment(p(0.0, -2.6), p(0.0, 5.0), w * 0.8, color, false);
            // The wave: tight zigzags left of the flag, the same pulled wide to its right.
            let pts = [
                p(-5.2, 1.6),
                p(-4.3, -0.2),
                p(-3.4, 3.4),
                p(-2.5, -0.2),
                p(-1.6, 3.4),
                p(-0.7, 1.6),
                p(1.2, -0.2),
                p(3.1, 3.4),
                p(5.2, 1.6),
            ];
            b.polyline(&pts, w * 0.8, color);
        }
        Glyph::PluginStopped => {
            // The plug's head and prongs, above; the socket's half, below, slanted away;
            // a jagged gap between them.
            b.rrect_stroke(
                Rect::from_min_max(p(-3.4, -2.6), p(3.4, 0.2)),
                1.0 * u,
                w,
                color,
            );
            b.segment(p(-1.5, -2.6), p(-1.5, -5.2), w, color, true);
            b.segment(p(1.5, -2.6), p(1.5, -5.2), w, color, true);
            b.polyline(
                &[p(-3.4, 2.4), p(-1.2, 1.6), p(1.0, 2.6), p(3.4, 1.8)],
                w * 0.8,
                color,
            );
            b.segment(p(0.0, 2.2), p(0.0, 5.4), w, color, true);
        }
        Glyph::Controller => {
            // A knob at the left (its ring and pointer), a fader at the right (its slot and
            // its cap).
            b.circle_stroke(p(-2.4, 0.6), 2.9 * u, w * 0.9, color);
            b.segment(p(-2.4, 0.6), p(-3.9, -1.4), w, color, true);
            b.segment(p(3.4, -4.8), p(3.4, 4.8), w * 0.8, color, true);
            b.rrect(
                Rect::from_min_max(p(2.0, -1.8), p(4.8, 0.6)),
                0.5 * u,
                Fill::Solid(color),
            );
        }
        Glyph::Filter => {
            // A funnel in outline: the wide mouth, the narrowing, the spout cut slanted.
            let pts = [
                p(-5.0, -4.4),
                p(5.0, -4.4),
                p(1.3, 0.3),
                p(1.3, 4.6),
                p(-1.3, 3.2),
                p(-1.3, 0.3),
                p(-5.0, -4.4),
            ];
            b.polyline(&pts, w, color);
        }
        _ => {}
    }
}
