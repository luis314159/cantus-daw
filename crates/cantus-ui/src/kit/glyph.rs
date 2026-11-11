//! The kit's icon family (round 37, ADR 0103): every glyph drawn on one 12-unit grid with
//! **one stroke weight**, **one corner radius** and **one optical size**, in two tones —
//! the ink for strokes and solid marks, a *tint* of the ink (its colour at [`TINT`]) inside
//! closed shapes — so a toolbar, a strip, a menu and a window's own buttons read as one hand.
//!
//! A glyph is **data**: [`ops`] describes it as a list of [`Op`]s in grid units (no op can
//! carry a width or a radius of its own), and [`paint`] renders the list centred in a box,
//! the stroke snapped to whole physical pixels and every point onto the pixel grid, so the
//! lines are crisp at 100 %, 125 %, 150 % and 200 %. `tests/kit_glyphs.rs` measures every
//! glyph — its extent, its centre and its ink — and fails on one outside the family.
//!
//! Callers size the box: a glyph fills about 0.86 of it (`2 × LIVE / GRID`); a toolbar
//! key gives it 0.72 of its side ([`super::cap::GLYPH_SHARE`]).

use super::brush::{Brush, Fill};
use egui::{pos2, Color32, Pos2, Rect};
use std::f32::consts::PI;

/// The grid a glyph is drawn on: this many units across its box.
pub const GRID: f32 = 12.0;
/// The family's one stroke weight, in units (snapped to whole physical pixels).
pub const STROKE: f32 = 1.2;
/// The family's one corner radius, in units, for frames and blocks.
pub const RADIUS: f32 = 1.0;
/// The tint inside closed shapes: the ink at this opacity.
pub const TINT: f32 = 0.38;
/// How far from the centre ink may reach, stroke included, in units.
pub const LIVE: f32 = 5.7;
/// The least a glyph's larger extent may be, in units: none reads smaller than the rest.
pub const OPTICAL_MIN: f32 = 8.0;

/// A point on the grid, in units from the box's centre (y down).
pub type P = (f32, f32);

/// One mark of a glyph. Strokes are [`STROKE`] wide with round ends and joins; frames and
/// blocks have [`RADIUS`] corners — no op can say otherwise.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Op<'a> {
    /// An open stroke through the points.
    Line(&'a [P]),
    /// A closed stroke through the points, tinted inside (the points make a convex shape).
    Loop(&'a [P]),
    /// A closed stroke through the points, not tinted (a concave shape; its tint, if any,
    /// is given as [`Op::Tint`] pieces).
    Outline(&'a [P]),
    /// A convex piece of tint.
    Tint(&'a [P]),
    /// A solid convex shape in the ink.
    Solid(&'a [P]),
    /// A circle's stroke, tinted inside: centre, radius.
    Ring(P, f32),
    /// A solid disc: centre, radius.
    Disc(P, f32),
    /// An arc's stroke: centre, radius, from and to (radians; 0 points right, π/2 down).
    Arc(P, f32, f32, f32),
    /// A rounded box's stroke, tinted inside: left, top, right, bottom.
    Frame(f32, f32, f32, f32),
    /// A solid rounded box in the ink.
    Block(f32, f32, f32, f32),
    /// A rounded box of tint alone (an unlit step, an empty cell).
    Ghost(f32, f32, f32, f32),
    /// A capsule's stroke (its ends half circles), tinted inside.
    Pill(f32, f32, f32, f32),
}

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
    /// ✦ an agent (round 34, the AI area).
    Agent,
    /// ● record
    Record,
    /// Ø invert phase
    Phase,
    /// ⇄ swap left and right
    Swap,
    /// ▲ send to this strip
    Send,
    /// △ a send that would loop (round 37: the hollow ▲, once drawn by the mixer).
    SendOpen,
    /// Two rings folded into one: mono (round 37: once drawn by the mixer).
    Mono,
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
    /// The score: a small staff with a note on it (round 32's notation pane).
    Score,
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
    /// A magnet: snap to the grid. // r37:finish
    Magnet,
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
    // --- freeze (round 29, ADR 0064) ---
    /// A snowflake: six arms, each with a pair of twigs — a frozen track or strip.
    Frozen,
    // --- controllers (round 29, ADR 0062) ---
    /// A hardware controller: a knob with its pointer beside a fader and its cap — MIDI
    /// learn, the mappings pane.
    Controller,
    // --- the piano roll's lanes (round 31, ADR 0068) ---
    /// A pencil drawing a line: a lane's freehand tool.
    LanePencil,
    /// A straight segment between two dots: a lane's line tool.
    LaneLine,
    /// A bent segment between two dots: a lane's curve tool.
    LaneCurve,
    /// The session view (round 33, ADR 0077): a grid of clip cells, one playing.
    Session,
    /// The Stage tab's mark: a figure dancing (round 35, ADR 0091).
    Stage,
    /// Three sliders at three heights: settings to set (the More panel's *Customise top
    /// bar…*, round 39 close-out).
    Sliders,
}

impl Glyph {
    /// Every glyph, in the order above (the family test walks it).
    pub const ALL: [Glyph; 65] = [
        Glyph::Menu,
        Glyph::Down,
        Glyph::Prev,
        Glyph::Next,
        Glyph::Star,
        Glyph::StarFilled,
        Glyph::More,
        Glyph::Help,
        Glyph::Power,
        Glyph::Undo,
        Glyph::Redo,
        Glyph::Resize,
        Glyph::Close,
        Glyph::Bulb,
        Glyph::Agent,
        Glyph::Record,
        Glyph::Phase,
        Glyph::Swap,
        Glyph::Send,
        Glyph::SendOpen,
        Glyph::Mono,
        Glyph::Plus,
        Glyph::Grip,
        Glyph::Check,
        Glyph::Device,
        Glyph::Play,
        Glyph::Pause,
        Glyph::Stop,
        Glyph::ToStart,
        Glyph::Loop,
        Glyph::Metronome,
        Glyph::Keyboard,
        Glyph::Arrangement,
        Glyph::PianoRoll,
        Glyph::Score,
        Glyph::Rack,
        Glyph::Mixer,
        Glyph::Inspector,
        Glyph::Browser,
        Glyph::Gear,
        Glyph::Pick,
        Glyph::Warning,
        Glyph::MouseMenu,
        Glyph::Automate,
        Glyph::Typed,
        Glyph::Zoom,
        Glyph::Magnet, // r37:finish
        Glyph::Minimise,
        Glyph::Maximise,
        Glyph::Restore,
        Glyph::Replace,
        Glyph::Link,
        Glyph::Key,
        Glyph::Cables,
        Glyph::Filter,
        Glyph::Warp,
        Glyph::PluginStopped,
        Glyph::Frozen,
        Glyph::Controller,
        Glyph::LanePencil,
        Glyph::LaneLine,
        Glyph::LaneCurve,
        Glyph::Session,
        Glyph::Stage,
        Glyph::Sliders,
    ];
}

/// The stroke a glyph in a box of `side` points is drawn with at `ppp`, in points: the
/// family's weight, rounded to whole physical pixels (one at least).
pub fn stroke_width(side: f32, ppp: f32) -> f32 {
    ((STROKE * side / GRID * ppp).round().max(1.0)) / ppp
}

/// Polylines this long or shorter have their points snapped to the pixel grid (crisp
/// straight strokes); longer ones are curves, drawn where they fall.
const SNAP_POINTS: usize = 5;

/// Paint `g` centred in `rect` (its smaller side is the glyph's box) in `color`: strokes
/// and solid marks in the ink, closed shapes tinted inside with it at [`TINT`].
pub fn paint(b: &mut Brush, rect: Rect, g: Glyph, color: Color32) {
    paint_mode(b, rect, g, color, false, ops);
}

/// A glyph's accent (r37:finish thirteenth, ADR 0103 §59): the one part of an area key's
/// little drawing in the theme's highlight — the playhead over the clips, the note being
/// played, the lit steps — drawn over [`paint`]'s body. Nothing for a glyph without one.
pub fn paint_accent(b: &mut Brush, rect: Rect, g: Glyph, color: Color32) {
    paint_mode(b, rect, g, color, false, accent_ops);
}

/// Whether `g` has an accent ([`paint_accent`]).
pub fn has_accent(g: Glyph) -> bool {
    let mut any = false;
    accent_ops(g, &mut |_| any = true);
    any
}

/// The stroke of a fine-line pictogram in a box of `side` points: three quarters of the
/// family's, whole physical pixels (1 px at 100 % on a toolbar key, 2 px at 150 %).
/// // r37:finish
pub fn fine_stroke_width(side: f32, ppp: f32) -> f32 {
    ((0.75 * STROKE * side / GRID * ppp).round().max(1.0)) / ppp
}

/// [`paint`] as a fine-line pictogram (the toolbar's keys, round 37: the judge read the
/// filled glyphs as blobs): the strokes finer, every filled shape an outline with a faint
/// tint, only dots under two pixels left solid. // r37:finish
pub fn paint_line(b: &mut Brush, rect: Rect, g: Glyph, color: Color32) {
    paint_mode(b, rect, g, color, true, ops);
}

/// A glyph's list of strokes and shapes: its body ([`ops`]) or its accent
/// ([`accent_ops`]).
type OpList = fn(Glyph, &mut dyn FnMut(Op<'_>));

fn paint_mode(b: &mut Brush, rect: Rect, g: Glyph, color: Color32, fine: bool, which: OpList) {
    let side = rect.width().min(rect.height());
    if side <= 0.0 || color.a() == 0 {
        return;
    }
    let ppp = b.ppp();
    let u = side / GRID;
    let w = if fine {
        fine_stroke_width(side, ppp)
    } else {
        stroke_width(side, ppp)
    };
    // An odd stroke centres on a pixel's middle, an even one on a pixel's edge; fills
    // end on pixel edges.
    let mid = if ((w * ppp).round() as i32) % 2 == 1 {
        0.5
    } else {
        0.0
    };
    let c = rect.center();
    let c = pos2((c.x * ppp).round() / ppp, (c.y * ppp).round() / ppp);
    let snap = move |v: f32, off: f32| ((v * ppp - off).round() + off) / ppp;
    let at = move |q: P| pos2(c.x + q.0 * u, c.y + q.1 * u);
    let on_line = move |q: P| {
        let p = at(q);
        pos2(snap(p.x, mid), snap(p.y, mid))
    };
    let on_edge = move |q: P| {
        let p = at(q);
        pos2(snap(p.x, 0.0), snap(p.y, 0.0))
    };
    // r37:finish — a fine pictogram is one ink, lines only (judge 7: an orange line over
    // a pale tint read as two inks): no tint inside its shapes.
    let tint = if fine {
        Color32::TRANSPARENT
    } else {
        color.gamma_multiply(TINT)
    };
    let rad = RADIUS * u;
    let mut buf = [Pos2::ZERO; 16];
    which(g, &mut |op| match op {
        Op::Line(pts) => {
            let n = pts.len().min(buf.len());
            let snapped = n <= SNAP_POINTS;
            for (d, q) in buf.iter_mut().zip(pts) {
                *d = if snapped { on_line(*q) } else { at(*q) };
            }
            if n == 2 {
                b.segment(buf[0], buf[1], w, color, true);
            } else if n > 2 {
                b.polyline(&buf[..n], w, color);
                b.dot(buf[0], w * 0.5, color);
                b.dot(buf[n - 1], w * 0.5, color);
            }
        }
        Op::Loop(pts) | Op::Outline(pts) => {
            let n = pts.len().min(buf.len());
            for (d, q) in buf.iter_mut().zip(pts) {
                *d = on_line(*q);
            }
            if matches!(op, Op::Loop(_)) && tint.a() > 0 {
                b.polygon(&buf[..n], tint);
            }
            for i in 0..n {
                b.segment(buf[i], buf[(i + 1) % n], w, color, false);
                b.dot(buf[i], w * 0.5, color);
            }
        }
        Op::Solid(pts) if fine => {
            // A solid shape as its outline over a faint tint.
            let n = pts.len().min(buf.len());
            for (d, q) in buf.iter_mut().zip(pts) {
                *d = on_line(*q);
            }
            if tint.a() > 0 {
                b.polygon(&buf[..n], tint);
            }
            for i in 0..n {
                b.segment(buf[i], buf[(i + 1) % n], w, color, false);
                b.dot(buf[i], w * 0.5, color);
            }
        }
        Op::Tint(pts) | Op::Solid(pts) => {
            let n = pts.len().min(buf.len());
            for (d, q) in buf.iter_mut().zip(pts) {
                *d = at(*q);
            }
            let ink = if matches!(op, Op::Tint(_)) {
                tint
            } else {
                color
            };
            b.polygon(&buf[..n], ink);
        }
        Op::Ring(q, r) => {
            let p = on_line(q);
            if tint.a() > 0 {
                b.dot(p, r * u, tint);
            }
            b.circle_stroke(p, r * u, w, color);
        }
        Op::Disc(q, r) if fine && r * u * ppp > 2.0 => {
            let p = on_line(q);
            if tint.a() > 0 {
                b.dot(p, r * u, tint);
            }
            b.circle_stroke(p, r * u, w, color);
        }
        Op::Disc(q, r) => b.dot(at(q), r * u, color),
        Op::Arc(q, r, a0, a1) => b.arc(on_line(q), r * u, a0, a1, w, color, true),
        Op::Frame(x0, y0, x1, y1) => {
            let r = Rect::from_min_max(on_line((x0, y0)), on_line((x1, y1)));
            if tint.a() > 0 {
                b.rrect(r, rad, Fill::Solid(tint));
            }
            b.rrect_stroke(r, rad, w, color);
        }
        Op::Pill(x0, y0, x1, y1) => {
            let r = Rect::from_min_max(on_line((x0, y0)), on_line((x1, y1)));
            let rr = r.width().min(r.height()) * 0.5;
            if tint.a() > 0 {
                b.rrect(r, rr, Fill::Solid(tint));
            }
            b.rrect_stroke(r, rr, w, color);
        }
        Op::Block(x0, y0, x1, y1) if fine => {
            // A block as a frame over a faint tint.
            let r = Rect::from_min_max(on_line((x0, y0)), on_line((x1, y1)));
            let rr = rad.min(r.width().min(r.height()) * 0.5);
            if tint.a() > 0 {
                b.rrect(r, rr, Fill::Solid(tint));
            }
            b.rrect_stroke(r, rr, w, color);
        }
        Op::Block(x0, y0, x1, y1) | Op::Ghost(x0, y0, x1, y1) => {
            let r = Rect::from_min_max(on_edge((x0, y0)), on_edge((x1, y1)));
            let ink = if matches!(op, Op::Ghost(..)) {
                tint
            } else {
                color
            };
            b.rrect(
                r,
                rad.min(r.width().min(r.height()) * 0.5),
                Fill::Solid(ink),
            );
        }
    });
}

/// A glyph's size for a box of `side` points: the box a caller hands [`paint`] so the
/// glyph is drawn at `drawn` points (the family fills `2 × LIVE / GRID` of its box).
pub fn box_for(drawn: f32) -> f32 {
    drawn * GRID / (2.0 * LIVE)
}

/// Describe `g` as ops in grid units, one call of `o` per op, in painting order.
pub fn ops(g: Glyph, o: &mut dyn FnMut(Op<'_>)) {
    use Op::*;
    match g {
        Glyph::Menu => o(Solid(&[(-1.6, -4.2), (3.4, 0.0), (-1.6, 4.2)])),
        Glyph::Down => o(Solid(&[(-4.2, -2.0), (4.2, -2.0), (0.0, 2.8)])),
        Glyph::Prev => o(Line(&[(1.9, -4.6), (-2.7, 0.0), (1.9, 4.6)])),
        Glyph::Next => o(Line(&[(-1.9, -4.6), (2.7, 0.0), (-1.9, 4.6)])),
        Glyph::Star | Glyph::StarFilled => {
            let pts: [P; 10] = std::array::from_fn(|i| {
                let a = -PI / 2.0 + i as f32 * PI / 5.0;
                let r = if i % 2 == 0 { 5.0 } else { 2.1 };
                (r * a.cos(), r * a.sin() + 0.4)
            });
            let pent = [pts[1], pts[3], pts[5], pts[7], pts[9]];
            let tips: [[P; 3]; 5] =
                std::array::from_fn(|i| [pts[(2 * i + 9) % 10], pts[2 * i], pts[2 * i + 1]]);
            if g == Glyph::StarFilled {
                o(Solid(&pent));
                for t in &tips {
                    o(Solid(t));
                }
            } else {
                o(Tint(&pent));
                for t in &tips {
                    o(Tint(t));
                }
                o(Outline(&pts));
            }
        }
        Glyph::More => {
            for x in [-3.8, 0.0, 3.8] {
                o(Disc((x, 0.0), 1.6));
            }
        }
        Glyph::Help => {
            o(Ring((0.0, 0.0), 5.0));
            o(Arc((0.0, -1.4), 1.8, -PI * 0.95, PI * 0.3));
            o(Line(&[(1.15, -0.05), (0.0, 0.9), (0.0, 1.4)]));
            o(Disc((0.0, 3.0), 0.8));
        }
        Glyph::Power => {
            o(Arc((0.0, 0.6), 4.3, -PI * 0.28, PI * 1.28));
            o(Line(&[(0.0, -5.0), (0.0, 0.4)]));
        }
        Glyph::Undo | Glyph::Redo => {
            let m = if g == Glyph::Undo { 1.0 } else { -1.0 };
            let (a0, a1) = if g == Glyph::Undo {
                (-PI * 0.9, PI * 0.35)
            } else {
                (PI * 0.65, PI * 1.9)
            };
            o(Arc((0.0, 0.8), 3.8, a0, a1));
            o(Solid(&[
                (-5.6 * m, -1.0),
                (-1.6 * m, -2.4),
                (-3.2 * m, 2.0),
            ]));
        }
        Glyph::Resize => {
            o(Line(&[(-3.2, 3.2), (3.2, -3.2)]));
            o(Solid(&[(4.8, -4.8), (0.6, -4.8), (4.8, -0.6)]));
            o(Solid(&[(-4.8, 4.8), (-0.6, 4.8), (-4.8, 0.6)]));
        }
        Glyph::Close => {
            o(Line(&[(-3.8, -3.8), (3.8, 3.8)]));
            o(Line(&[(-3.8, 3.8), (3.8, -3.8)]));
        }
        Glyph::Bulb => {
            // A lamp's glass, solid (it is read as lit or not), and its base.
            o(Disc((0.0, -1.3), 3.8));
            o(Block(-1.9, 2.9, 1.9, 4.0));
            o(Block(-1.2, 4.6, 1.2, 5.5));
        }
        Glyph::Agent => {
            let w: [P; 4] = [(-1.3, -1.1), (1.3, -1.1), (1.3, 1.5), (-1.3, 1.5)];
            let tips: [P; 4] = [(0.0, -4.8), (5.0, 0.2), (0.0, 5.2), (-5.0, 0.2)];
            for i in 0..4 {
                o(Solid(&[w[i], tips[i], w[(i + 1) % 4]]));
            }
            o(Solid(&w));
            o(Disc((4.0, -4.0), 1.2));
        }
        Glyph::Record => o(Disc((0.0, 0.0), 4.4)),
        Glyph::Phase => {
            o(Ring((0.0, 0.0), 3.6));
            o(Line(&[(-4.6, 4.6), (4.6, -4.6)]));
        }
        Glyph::Swap => {
            o(Line(&[(-4.6, -2.2), (2.4, -2.2)]));
            o(Solid(&[(5.0, -2.2), (1.8, -4.8), (1.8, 0.4)]));
            o(Line(&[(4.6, 2.2), (-2.4, 2.2)]));
            o(Solid(&[(-5.0, 2.2), (-1.8, -0.4), (-1.8, 4.8)]));
        }
        Glyph::Send => o(Solid(&[(-4.4, 3.0), (0.0, -3.0), (4.4, 3.0)])),
        Glyph::SendOpen => o(Outline(&[(-4.4, 3.0), (0.0, -3.0), (4.4, 3.0)])),
        Glyph::Mono => {
            o(Ring((-1.9, 0.0), 3.2));
            o(Ring((1.9, 0.0), 3.2));
        }
        Glyph::Plus => {
            o(Line(&[(-4.2, 0.0), (4.2, 0.0)]));
            o(Line(&[(0.0, -4.2), (0.0, 4.2)]));
        }
        Glyph::Grip => {
            // Anchored in its corner, where a window's grip sits.
            o(Line(&[(-2.4, 5.0), (5.0, -2.4)]));
            o(Line(&[(1.2, 5.0), (5.0, 1.2)]));
            o(Disc((4.6, 4.6), 0.9));
        }
        Glyph::Check => o(Line(&[(-4.4, 0.2), (-1.4, 3.4), (4.4, -3.6)])),
        Glyph::Device => {
            o(Ring((0.0, 0.0), 5.0));
            let wave: [P; 13] = std::array::from_fn(|i| {
                let x = -3.0 + 6.0 * i as f32 / 12.0;
                (x, -1.9 * (x * 1.3).sin())
            });
            o(Line(&wave));
        }
        _ => workspace(g, o),
    }
}

/// The workspace's glyphs (ADR 0024 §5, §9): transport, views, the hint bar.
fn workspace(g: Glyph, o: &mut dyn FnMut(Op<'_>)) {
    use Op::*;
    match g {
        Glyph::Play => o(Solid(&[(-3.4, -4.8), (4.8, 0.0), (-3.4, 4.8)])),
        Glyph::Pause => {
            o(Block(-4.0, -4.6, -1.2, 4.6));
            o(Block(1.2, -4.6, 4.0, 4.6));
        }
        Glyph::Stop => o(Block(-4.2, -4.2, 4.2, 4.2)),
        Glyph::ToStart => {
            o(Block(-4.6, -4.4, -2.8, 4.4));
            o(Solid(&[(4.6, -4.4), (4.6, 4.4), (-2.0, 0.0)]));
        }
        Glyph::Loop => {
            o(Arc((0.0, 0.0), 4.0, PI * 0.15, PI * 1.02));
            o(Arc((0.0, 0.0), 4.0, PI * 1.15, PI * 2.02));
            o(Solid(&[(-5.6, -0.4), (-2.4, -0.4), (-4.0, 2.2)]));
            o(Solid(&[(5.6, 0.4), (2.4, 0.4), (4.0, -2.2)]));
        }
        Glyph::Metronome => {
            // r37:finish — a metronome: its case an outline (not a tinted block), its foot,
            // the pendulum swung with its weight.
            o(Outline(&[
                (-3.4, 4.0),
                (3.4, 4.0),
                (1.2, -4.8),
                (-1.2, -4.8),
            ]));
            o(Line(&[(-4.6, 4.8), (4.6, 4.8)]));
            o(Line(&[(0.0, 2.6), (3.4, -3.8)]));
            o(Disc((1.7, -0.6), 1.1));
        }
        Glyph::Keyboard => {
            o(Frame(-5.0, -3.8, 5.0, 3.8));
            for x in [-2.6, 0.0, 2.6] {
                o(Line(&[(x, 0.6), (x, 3.8)]));
            }
            for x in [-2.6, 0.0] {
                o(Block(x - 0.9, -3.8, x + 0.9, 0.6));
            }
            o(Block(2.6 - 0.9, -3.8, 2.6 + 0.9, 0.6));
        }
        Glyph::Arrangement => {
            // r37:finish thirteenth — the playlist: clips on three lanes, each a framed
            // block; the playhead across them is the accent.
            o(Frame(-5.0, -4.8, 0.2, -2.2));
            o(Frame(-2.4, -1.3, 5.0, 1.3));
            o(Frame(-5.0, 2.2, -0.8, 4.8));
            o(Frame(2.6, 2.2, 5.0, 4.8));
        }
        Glyph::PianoRoll => {
            // r37:finish thirteenth — a keyboard down the left (its keys parted), notes at
            // two pitches; the note being played is the accent.
            o(Frame(-5.0, -5.0, -2.4, 5.0));
            o(Line(&[(-5.0, -1.7), (-2.4, -1.7)]));
            o(Line(&[(-5.0, 1.7), (-2.4, 1.7)]));
            o(Block(-1.2, -4.4, 2.4, -2.6));
            o(Block(0.2, 2.6, 4.2, 4.4));
        }
        Glyph::Score => {
            for y in [-3.4, 0.2, 3.8] {
                o(Line(&[(-5.0, y), (5.0, y)]));
            }
            o(Disc((-1.0, 2.0), 1.9));
            o(Line(&[(0.85, 1.6), (0.85, -4.8)]));
            o(Solid(&[
                (0.85, -4.8),
                (3.8, -2.4),
                (3.2, -1.6),
                (0.85, -3.0),
            ]));
        }
        Glyph::Rack => {
            // r37:finish thirteenth — the step sequencer: three channels' rows of four
            // steps, framed; the lit steps are the accent.
            for y in [-5.0f32, -1.3, 2.4] {
                for x in [-5.0f32, -2.4, 0.2, 2.8] {
                    o(Ghost(x, y, x + 2.2, y + 2.6));
                }
            }
        }
        Glyph::Mixer => {
            // r37:finish thirteenth — three faders: their slots and their caps at three
            // heights; the middle cap is the accent.
            for x in [-3.6f32, 0.0, 3.6] {
                o(Line(&[(x, -5.0), (x, 5.0)]));
            }
            o(Frame(-5.0, 0.6, -2.2, 2.8));
            o(Frame(2.2, -0.6, 5.0, 1.6));
        }
        Glyph::Inspector => {
            // r37:finish — the inspector: information, an "i" in its ring (a silhouette of
            // its own; three value tracks read like the other bar-and-dot icons).
            o(Ring((0.0, 0.0), 4.9));
            o(Disc((0.0, -2.4), 0.95));
            o(Line(&[(0.0, -0.6), (0.0, 3.0)]));
        }
        Glyph::Browser => {
            // r37:finish thirteenth — a folder, its tab and its front flap; the sound in
            // it is the accent.
            o(Outline(&[
                (-5.0, 4.8),
                (-5.0, -4.6),
                (-1.6, -4.6),
                (-0.4, -3.2),
                (5.0, -3.2),
                (5.0, 4.8),
                (-5.0, 4.8),
            ]));
            o(Line(&[(-5.0, -1.4), (5.0, -1.4)]));
        }
        Glyph::Gear => {
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                let (s, c) = a.sin_cos();
                let (r0, r1, h) = (2.8, 5.0, 0.95);
                let q = |r: f32, t: f32| (c * r - s * t, s * r + c * t);
                o(Solid(&[
                    q(r0, -h),
                    q(r1, -h * 0.8),
                    q(r1, h * 0.8),
                    q(r0, h),
                ]));
            }
            o(Ring((0.0, 0.0), 3.3));
            o(Ring((0.0, 0.0), 1.1));
        }
        Glyph::Pick => {
            o(Line(&[(-4.4, 4.4), (0.6, -0.6)]));
            o(Solid(&[(-0.8, -2.6), (2.6, 0.8), (4.2, -0.8), (0.8, -4.2)]));
            o(Disc((3.4, -3.4), 1.9));
        }
        Glyph::Warning => {
            o(Loop(&[(0.0, -4.8), (5.0, 4.2), (-5.0, 4.2)]));
            o(Line(&[(0.0, -1.4), (0.0, 1.2)]));
            o(Disc((0.0, 2.8), 0.75));
        }
        Glyph::MouseMenu => {
            o(Pill(-3.4, -4.8, 3.4, 4.8));
            o(Line(&[(0.0, -4.8), (0.0, -1.2)]));
            o(Solid(&[(0.5, -4.2), (2.8, -3.0), (2.8, -1.2), (0.5, -1.2)]));
        }
        Glyph::Magnet => {
            // r37:finish — a horseshoe magnet, its poles up (snap, as FL's).
            o(Arc((0.0, -0.6), 3.9, 0.0, PI));
            o(Line(&[(-3.9, -0.6), (-3.9, -3.0)]));
            o(Line(&[(3.9, -0.6), (3.9, -3.0)]));
            o(Block(-5.2, -5.0, -2.6, -3.0));
            o(Block(2.6, -5.0, 5.2, -3.0));
        }
        Glyph::Automate => {
            let pts: [P; 4] = [(-4.4, 3.0), (-1.7, -3.0), (1.2, 1.0), (4.4, -3.4)];
            o(Line(&pts));
            for q in pts {
                o(Disc(q, 1.2));
            }
        }
        Glyph::Typed => {
            o(Line(&[(0.0, -4.4), (0.0, 4.4)]));
            o(Line(&[(-2.2, -4.4), (2.2, -4.4)]));
            o(Line(&[(-2.2, 4.4), (2.2, 4.4)]));
        }
        Glyph::Zoom => {
            o(Ring((-1.0, -1.0), 3.6));
            o(Line(&[(1.8, 1.8), (4.8, 4.8)]));
        }
        Glyph::LanePencil => {
            o(Loop(&[
                (-1.55, 3.25),
                (4.45, -2.75),
                (2.75, -4.45),
                (-3.25, 1.55),
            ]));
            o(Solid(&[(-1.55, 3.25), (-3.25, 1.55), (-4.8, 4.8)]));
        }
        Glyph::LaneLine => {
            o(Line(&[(-4.2, 3.4), (4.2, -3.4)]));
            o(Disc((-4.2, 3.4), 1.4));
            o(Disc((4.2, -3.4), 1.4));
        }
        Glyph::LaneCurve => {
            let pts: [P; 9] = std::array::from_fn(|i| {
                let k = i as f32 / 8.0;
                (-4.2 + 8.4 * k, 3.4 - 6.8 * k * k)
            });
            o(Line(&pts));
            o(Disc((-4.2, 3.4), 1.4));
            o(Disc((4.2, -3.4), 1.4));
        }
        Glyph::Session => {
            // r37:finish thirteenth — the session: four clip slots, framed; the one playing
            // (its ▶) is the accent.
            o(Frame(-5.0, -5.0, -0.6, -0.6));
            o(Frame(0.6, -5.0, 5.0, -0.6));
            o(Frame(-5.0, 0.6, -0.6, 5.0));
            o(Frame(0.6, 0.6, 5.0, 5.0));
        }
        Glyph::Stage => {
            o(Disc((0.6, -3.6), 1.6));
            o(Line(&[(0.3, -1.6), (-0.2, 1.4)]));
            o(Line(&[(-3.4, -3.6), (0.2, -1.0), (3.4, -0.4)]));
            o(Line(&[(-2.6, 3.8), (-0.2, 1.4), (2.0, 2.4), (2.6, 4.2)]));
            o(Line(&[(-5.0, 5.0), (5.0, 5.0)]));
        }
        Glyph::Sliders => {
            for (y, x) in [(-3.6, 1.6), (0.0, -2.0), (3.6, 0.6)] {
                o(Line(&[(-5.0, y), (x - 1.6, y)]));
                o(Line(&[(x + 1.6, y), (5.0, y)]));
                o(Block(x - 0.8, y - 1.4, x + 0.8, y + 1.4));
            }
        }
        _ => window(g, o),
    }
}

/// A device window's own buttons (round 20), routing, warping, isolation, freeze and
/// controllers.
fn window(g: Glyph, o: &mut dyn FnMut(Op<'_>)) {
    use Op::*;
    match g {
        // At the foot of its box, as every window's minimise is.
        Glyph::Minimise => o(Block(-4.6, 1.6, 4.6, 4.0)),
        Glyph::Maximise => {
            o(Frame(-4.6, -4.6, 4.6, 4.6));
            o(Block(-4.6, -4.6, 4.6, -2.4));
        }
        Glyph::Restore => {
            // The window behind shows only where the front one does not cover it.
            o(Line(&[
                (-1.8, -2.2),
                (-1.8, -4.8),
                (4.8, -4.8),
                (4.8, 1.8),
                (2.4, 1.8),
            ]));
            o(Frame(-4.8, -2.0, 2.4, 4.8));
            o(Block(-4.8, -2.0, 2.4, 0.0));
        }
        Glyph::Replace => {
            // Two cards, and an arrow turning from one to the other above them.
            o(Frame(-5.0, -0.2, -1.2, 5.0));
            o(Block(1.2, -0.2, 5.0, 5.0));
            o(Arc((0.0, -0.4), 3.4, PI * 1.12, PI * 1.8));
            o(Solid(&[(2.0, -4.8), (4.8, -2.6), (1.4, -1.6)]));
        }
        Glyph::Link => {
            // Two links of a chain, one through the other.
            o(Pill(-4.8, -1.9, 0.9, 1.9));
            o(Pill(-0.9, -1.9, 4.8, 1.9));
        }
        Glyph::Key => {
            // A key on its side (round 25): the bow at the left, the shaft, two teeth.
            o(Ring((-2.6, 0.0), 2.4));
            o(Line(&[(-0.2, 0.0), (5.0, 0.0)]));
            o(Line(&[(3.0, 0.0), (3.0, 2.6)]));
            o(Line(&[(5.0, 0.0), (5.0, 2.0)]));
        }
        Glyph::Cables => {
            // Two ports and the cable hanging between them (round 25).
            let pts: [P; 9] = std::array::from_fn(|i| {
                let a = PI * i as f32 / 8.0;
                (-4.0 * a.cos(), -2.4 + 6.4 * a.sin())
            });
            o(Line(&pts));
            o(Disc((-4.0, -3.2), 1.7));
            o(Disc((4.0, -3.2), 1.7));
        }
        Glyph::Filter => {
            // A funnel: the wide mouth, the narrowing, the spout.
            let mouth = [(-5.0, -4.4), (5.0, -4.4), (1.3, 0.3), (-1.3, 0.3)];
            let spout = [(1.3, 0.3), (1.3, 4.6), (-1.3, 3.2), (-1.3, 0.3)];
            o(Tint(&mouth));
            o(Tint(&spout));
            o(Outline(&[
                (-5.0, -4.4),
                (5.0, -4.4),
                (1.3, 0.3),
                (1.3, 4.6),
                (-1.3, 3.2),
                (-1.3, 0.3),
            ]));
        }
        Glyph::Warp => {
            // A flag on its staff over a wave pulled out to the right.
            o(Solid(&[(-1.8, -5.0), (1.8, -5.0), (0.0, -2.4)]));
            o(Line(&[(0.0, -2.4), (0.0, 5.0)]));
            o(Line(&[
                (-5.0, 1.6),
                (-4.1, -0.2),
                (-3.2, 3.4),
                (-2.3, -0.2),
                (-1.4, 3.4),
                (-0.5, 1.6),
                (1.4, -0.2),
                (3.2, 3.4),
                (5.0, 1.6),
            ]));
        }
        Glyph::PluginStopped => {
            // The plug's head and prongs above, its cable below, a jagged gap between.
            o(Frame(-3.4, -2.4, 3.4, 0.4));
            o(Line(&[(-1.5, -2.4), (-1.5, -5.0)]));
            o(Line(&[(1.5, -2.4), (1.5, -5.0)]));
            o(Line(&[(-3.4, 2.4), (-1.2, 1.6), (1.0, 2.6), (3.4, 1.8)]));
            o(Line(&[(0.0, 2.6), (0.0, 5.0)]));
        }
        Glyph::Frozen => {
            // Six arms at 60°, each with two twigs a little out from the middle.
            const ARMS: [P; 6] = [
                (0.0, -1.0),
                (0.866, -0.5),
                (0.866, 0.5),
                (0.0, 1.0),
                (-0.866, 0.5),
                (-0.866, -0.5),
            ];
            let r = 5.0;
            for (i, (x, y)) in ARMS.iter().enumerate() {
                o(Line(&[(0.0, 0.0), (x * r, y * r)]));
                let (bx, by) = (x * r * 0.6, y * r * 0.6);
                for j in [(i + 1) % 6, (i + 5) % 6] {
                    let (nx, ny) = ARMS[j];
                    let t = 1.7;
                    o(Line(&[
                        (bx, by),
                        (bx + (x + nx) * 0.5 * t, by + (y + ny) * 0.5 * t),
                    ]));
                }
            }
        }
        Glyph::Controller => {
            // A knob at the left (its ring and pointer), a fader at the right.
            o(Ring((-2.2, 0.6), 2.8));
            o(Line(&[(-2.2, 0.6), (-3.7, -1.3)]));
            o(Line(&[(3.6, -4.8), (3.6, 4.8)]));
            o(Block(2.0, -2.0, 5.2, 0.6));
        }
        _ => {}
    }
}

/// The ink's extent of `g` in units — its box, stroke and discs included: what
/// The accent of an area key's drawing (r37:finish thirteenth): the part in the highlight.
pub fn accent_ops(g: Glyph, o: &mut dyn FnMut(Op<'_>)) {
    use Op::*;
    match g {
        Glyph::Browser => o(Line(&[
            (-3.0, 1.8),
            (-1.8, 1.8),
            (-0.9, 0.2),
            (0.2, 3.6),
            (1.2, 0.8),
            (2.1, 1.8),
            (3.2, 1.8),
        ])),
        Glyph::Arrangement => {
            o(Line(&[(1.4, -4.0), (1.4, 5.4)]));
            o(Solid(&[(-0.2, -5.6), (3.0, -5.6), (1.4, -3.8)]));
        }
        Glyph::PianoRoll => o(Block(1.0, -0.9, 5.2, 0.9)),
        Glyph::Mixer => o(Block(-1.6, -3.4, 1.6, -1.2)),
        Glyph::Rack => {
            for (x, y) in [
                (-5.0f32, -5.0f32),
                (0.2, -5.0),
                (-2.4, -1.3),
                (2.8, -1.3),
                (-5.0, 2.4),
                (0.2, 2.4),
            ] {
                o(Block(x, y, x + 2.2, y + 2.6));
            }
        }
        Glyph::Session => o(Solid(&[(-4.0, -4.2), (-1.4, -2.8), (-4.0, -1.4)])),
        _ => {}
    }
}

/// `tests/kit_glyphs.rs` holds against [`LIVE`] and [`OPTICAL_MIN`].
pub fn extent(g: Glyph) -> Rect {
    let mut r = Rect::NOTHING;
    let h = STROKE * 0.5;
    let mut add = |x: f32, y: f32, pad: f32| {
        r = r.union(Rect::from_min_max(
            pos2(x - pad, y - pad),
            pos2(x + pad, y + pad),
        ));
    };
    ops(g, &mut |op| match op {
        Op::Line(p) | Op::Loop(p) | Op::Outline(p) => p.iter().for_each(|q| add(q.0, q.1, h)),
        Op::Tint(p) | Op::Solid(p) => p.iter().for_each(|q| add(q.0, q.1, 0.0)),
        Op::Ring(c, rad) => add(c.0, c.1, rad + h),
        Op::Disc(c, rad) => add(c.0, c.1, rad),
        Op::Arc(c, rad, a0, a1) => {
            for i in 0..=24 {
                let a = a0 + (a1 - a0) * i as f32 / 24.0;
                add(c.0 + rad * a.cos(), c.1 + rad * a.sin(), h);
            }
        }
        Op::Frame(x0, y0, x1, y1) | Op::Pill(x0, y0, x1, y1) => {
            add(x0, y0, h);
            add(x1, y1, h);
        }
        Op::Block(x0, y0, x1, y1) | Op::Ghost(x0, y0, x1, y1) => {
            add(x0, y0, 0.0);
            add(x1, y1, 0.0);
        }
    });
    r
}

/// Glyphs placed off centre on purpose: a window's grip sits in its corner, minimise at the
/// foot of its box.
pub const ANCHORED: [Glyph; 2] = [Glyph::Grip, Glyph::Minimise];

/// The icon family's name for each kit glyph (round 39, `icons`' set 5 and its whole map,
/// `docs/round39/design/icons/spec-rest.md`): the drawing a key of 16–36 device pixels
/// shows (`kit::icons`). The set each comes from is said where it is not set 5.
pub fn family_name(g: Glyph) -> &'static str {
    match g {
        Glyph::Menu => "menu",
        Glyph::Down => "down",
        Glyph::Prev => "prev",
        Glyph::Next => "next",
        Glyph::Star => "star",             // set 4
        Glyph::StarFilled => "starFilled", // set 4
        Glyph::More => "more",             // set 3
        Glyph::Help => "help",
        // The bulb is retired: an effect's on is the enable key (set 3).
        Glyph::Power | Glyph::Bulb => "enable",
        Glyph::Undo => "undo", // set 2
        Glyph::Redo => "redo", // set 2
        Glyph::Resize => "resize",
        Glyph::Close => "close",
        Glyph::Agent => "agent",
        Glyph::Record => "record", // set 2 (a strip's arm is set 3's `arm`)
        Glyph::Phase => "phase",   // set 3
        Glyph::Swap => "swap",     // set 3
        Glyph::Send => "send",     // set 3
        Glyph::SendOpen => "sendOpen",
        Glyph::Mono => "mono", // set 3
        Glyph::Plus => "add",  // set 3
        Glyph::Grip => "resizeGrip",
        Glyph::Check => "check", // set 4
        Glyph::Device => "device",
        Glyph::Play => "play",           // set 2
        Glyph::Pause => "pause",         // set 2
        Glyph::Stop => "stop",           // set 2
        Glyph::ToStart => "toStart",     // set 2
        Glyph::Loop => "loop",           // set 2
        Glyph::Metronome => "metronome", // set 2
        Glyph::Keyboard => "keyboard",
        Glyph::Arrangement => "arrangement", // set 2
        Glyph::PianoRoll => "pianoRoll",     // set 2
        Glyph::Score => "score",
        Glyph::Rack => "rack",           // set 2
        Glyph::Mixer => "mixer",         // set 2
        Glyph::Inspector => "inspector", // set 2
        Glyph::Browser => "browser",     // set 2
        Glyph::Gear => "gear",           // set 2
        Glyph::Pick => "pick",           // set 2
        Glyph::Warning => "warning",
        Glyph::MouseMenu => "mouseMenu",
        Glyph::Automate => "automate",
        Glyph::Typed => "typed",
        Glyph::Zoom => "zoom",   // set 2
        Glyph::Magnet => "snap", // set 2
        Glyph::Minimise => "minimise",
        Glyph::Maximise => "maximise",
        Glyph::Restore => "restore",
        Glyph::Replace => "replace",
        Glyph::Link => "link",      // set 3
        Glyph::Key => "key",        // set 3
        Glyph::Cables => "routing", // set 3
        Glyph::Filter => "filter",
        Glyph::Warp => "warp",
        Glyph::PluginStopped => "pluginStopped",
        Glyph::Frozen => "freeze", // set 3
        Glyph::Controller => "controller",
        Glyph::LanePencil => "lanePencil",
        Glyph::LaneLine => "laneLine",
        Glyph::LaneCurve => "laneCurve",
        Glyph::Session => "session",
        Glyph::Stage => "stage",
        Glyph::Sliders => "sliders",
    }
}

/// Set 5's drawing of `name` (and the set-4 drawings the map names) for a canvas of `size`
/// device pixels ([`super::glyph_rest_data`]).
pub fn rest_drawing(name: &str, size: u16) -> Option<&'static str> {
    super::glyph_rest_data::DRAWINGS
        .iter()
        .find(|(n, s, _)| *n == name && *s == size)
        .map(|(_, _, svg)| *svg)
}

/// The glyphs set 5 draws for every key (16, 20, 24, 30 and 36 device pixels).
pub const SET5_KEYS: [Glyph; 19] = [
    Glyph::Help,
    Glyph::Warning,
    Glyph::Agent,
    Glyph::Device,
    Glyph::Keyboard,
    Glyph::Score,
    Glyph::MouseMenu,
    Glyph::Automate,
    Glyph::Filter,
    Glyph::Replace,
    Glyph::Warp,
    Glyph::PluginStopped,
    Glyph::Controller,
    Glyph::LanePencil,
    Glyph::LaneLine,
    Glyph::LaneCurve,
    Glyph::Session,
    Glyph::Stage,
    Glyph::Sliders,
];

#[cfg(test)]
mod family_tests {
    use super::*;

    /// Every glyph of the kit has its family name, and every set-5 key glyph is drawn for
    /// every key height, each drawing parsing and inking something (spec-rest.md).
    #[test]
    fn set_5_draws_every_key_glyph_at_every_key_height() {
        for g in Glyph::ALL {
            assert!(!family_name(g).is_empty(), "{g:?}");
        }
        let t = crate::theme_slate::slate_night_tokens();
        let inks = super::super::icons::Inks {
            main: t.text,
            second: t.text_dim,
            rec: t.danger,
            cut: t.well,
        };
        for g in SET5_KEYS {
            let name = family_name(g);
            for size in super::super::icons::SIZES {
                let body = rest_drawing(name, size)
                    .unwrap_or_else(|| panic!("{name} has no drawing at {size} px"));
                let doc = super::super::icons::document(body, size, &inks);
                let tree = resvg::usvg::Tree::from_str(&doc, &Default::default())
                    .unwrap_or_else(|e| panic!("{name}@{size}: {e}"));
                let mut pix = resvg::tiny_skia::Pixmap::new(size.into(), size.into()).unwrap();
                resvg::render(&tree, Default::default(), &mut pix.as_mut());
                let inked = pix.pixels().iter().filter(|p| p.alpha() > 0).count();
                assert!(inked > 4, "{name}@{size} draws nothing");
                assert!(!doc.contains("var(--"), "{name}@{size}: an ink not named");
            }
        }
        // The small marks: each at its own sizes.
        for (name, sizes) in [
            ("menu", &[10u16, 12, 15, 18][..]),
            ("down", &[8, 10, 12, 15]),
            ("close", &[10, 12, 15, 16, 18, 24]),
            ("resize", &[12, 16, 18, 24]),
            ("typed", &[12, 16, 18, 24]),
        ] {
            for &size in sizes {
                assert!(rest_drawing(name, size).is_some(), "{name}@{size}");
            }
        }
    }
}
