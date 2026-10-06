//! An automation curve on screen (ADR 0027): traced once, the same way for a clip in the
//! arrangement and for the large editor, so the two can never disagree with each other or
//! with what plays (`cantus_core::automation` defines every shape; this only samples it).
//!
//! **Level of detail.** A straight segment is two points, a hold is a step, and only a
//! bent segment is sampled — every `step` pixels across its *visible* width. A clip's
//! curve costs a handful of vertices whatever its length, and a view full of them stays
//! inside the frame budget (ADR 0011).

use crate::kit::color::with_alpha;
use crate::kit::{Brush, Fill, Style};
use cantus_core::automation::segment;
use cantus_core::{Automation, Shape, Ticks};
use egui::{pos2, Color32, Pos2, Rect};

/// Where the curve is drawn: content ticks to x, 0…1 to y, and the x range worth tracing.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Frame {
    /// Screen x of content tick 0 (`x = x0 + tick · px_per_tick`).
    pub x0: f32,
    pub px_per_tick: f32,
    /// The value axis: 1 at `top`, 0 at `bottom`.
    pub top: f32,
    pub bottom: f32,
    /// Nothing left or right of this is traced.
    pub clip_left: f32,
    pub clip_right: f32,
}

impl Frame {
    #[inline]
    pub fn x(&self, tick: f64) -> f32 {
        self.x0 + tick as f32 * self.px_per_tick
    }
    #[inline]
    pub fn y(&self, v: f32) -> f32 {
        self.bottom - v.clamp(0.0, 1.0) * (self.bottom - self.top)
    }
    #[inline]
    pub fn tick(&self, x: f32) -> f64 {
        ((x - self.x0) / self.px_per_tick.max(1e-9)) as f64
    }
    #[inline]
    pub fn value(&self, y: f32) -> f32 {
        ((self.bottom - y) / (self.bottom - self.top).max(1e-6)).clamp(0.0, 1.0)
    }
    pub fn point(&self, tick: Ticks, v: f32) -> Pos2 {
        pos2(self.x(tick.0 as f64), self.y(v))
    }
}

/// The curve over content `[ca, cb)` into `out` (appended), as screen points.
pub fn trace(auto: &Automation, ca: Ticks, cb: Ticks, f: &Frame, step: f32, out: &mut Vec<Pos2>) {
    let pts = &auto.points;
    let Some(first) = auto.value_at(ca.0 as f64) else {
        return;
    };
    if cb <= ca {
        return;
    }
    // Only what is on screen is traced: the visible content range.
    let lo = f.tick(f.clip_left).max(ca.0 as f64);
    let hi = f.tick(f.clip_right).min(cb.0 as f64);
    if hi <= lo {
        return;
    }
    let (lo_t, hi_t) = (Ticks(lo.floor() as i64), Ticks(hi.ceil() as i64));
    let (ca, cb) = (ca.max(lo_t), cb.min(hi_t));
    let start_v = auto.value_at(ca.0 as f64).unwrap_or(first);
    out.push(pos2(f.x(ca.0 as f64), f.y(start_v)));
    let mut c = ca;
    let mut i = pts.partition_point(|p| p.tick <= c);
    loop {
        let end = pts.get(i).map(|p| p.tick).filter(|t| *t < cb).unwrap_or(cb);
        let left = (end.0 as f64 - 1e-3).max(c.0 as f64);
        if i > 0 && i < pts.len() {
            let (p, q) = (pts[i - 1], pts[i]);
            let straight = p.shape == Shape::Curve && p.tension == 0.0;
            if p.shape == Shape::Hold {
                out.push(pos2(f.x(end.0 as f64), f.y(p.value)));
            } else if straight {
                out.push(pos2(f.x(end.0 as f64), f.y(segment(p, q, left))));
            } else {
                let (xa, xb) = (f.x(c.0 as f64), f.x(end.0 as f64));
                let n = (((xb - xa) / step.max(0.5)).ceil() as usize).clamp(1, 4096);
                for k in 1..=n {
                    let cc = c.0 as f64 + (end.0 - c.0) as f64 * k as f64 / n as f64;
                    let cc = cc.min(left);
                    out.push(pos2(f.x(cc), f.y(segment(p, q, cc))));
                }
            }
        } else {
            let v = auto.value_at(c.0 as f64).unwrap_or(first);
            out.push(pos2(f.x(end.0 as f64), f.y(v)));
        }
        if end >= cb || i >= pts.len() {
            break;
        }
        // At the point: on to its value (the last of several at one tick).
        c = pts[i].tick;
        let v = auto.value_at(c.0 as f64).unwrap_or(first);
        out.push(pos2(f.x(c.0 as f64), f.y(v)));
        i = pts.partition_point(|p| p.tick <= c);
    }
}

/// How a traced curve is painted.
#[derive(Copy, Clone, Debug)]
pub struct Ink {
    pub line: Color32,
    pub fill: Color32,
    pub width: f32,
}

impl Ink {
    /// The house look: the line in `ink`, the area under it in a wash of it.
    pub fn of(ink: Color32, width: f32, s: &Style) -> Ink {
        Ink {
            line: ink,
            fill: with_alpha(ink, if s.t.dark { 0.22 } else { 0.16 }),
            width,
        }
    }
}

/// Paint a traced curve: the area down to the axis, then the line.
pub fn paint(b: &mut Brush, pts: &[Pos2], base: f32, ink: Ink) {
    if pts.len() < 2 {
        return;
    }
    b.area(pts, base, ink.fill);
    b.polyline(pts, ink.width, ink.line);
}

/// A point: a disc in the ground's colour ringed in the curve's, filled when selected or
/// hovered.
pub fn point(b: &mut Brush, c: Pos2, r: f32, ink: Color32, ground: Color32, lit: bool) {
    b.circle(c, r + b.px(), Fill::Solid(ink));
    if !lit {
        b.circle(c, (r - b.px()).max(b.px()), Fill::Solid(ground));
    }
}

/// A tension handle: a small hollow ring halfway along a segment.
pub fn handle(b: &mut Brush, c: Pos2, r: f32, ink: Color32, lit: bool) {
    if lit {
        b.circle(c, r, Fill::Solid(ink));
    } else {
        b.circle_stroke(c, r, b.px() * 1.25, with_alpha(ink, 0.85));
    }
}

/// Where segment `i`'s tension handle is (`None` when the segment cannot bend): halfway in
/// time, on the curve.
pub fn handle_at(auto: &Automation, i: usize, f: &Frame) -> Option<Pos2> {
    let (p, q) = (auto.points.get(i)?, auto.points.get(i + 1)?);
    if !p.shape.has_tension() || q.tick <= p.tick || p.value == q.value && p.shape == Shape::Curve {
        return None;
    }
    let mid = (p.tick.0 + q.tick.0) as f64 * 0.5;
    Some(pos2(f.x(mid), f.y(segment(*p, *q, mid))))
}

/// The point under `pos` within `radius`, nearest first.
pub fn point_at(auto: &Automation, f: &Frame, pos: Pos2, radius: f32) -> Option<usize> {
    auto.points
        .iter()
        .enumerate()
        .map(|(i, p)| (i, f.point(p.tick, p.value).distance(pos)))
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// The segment whose tension handle is under `pos`.
pub fn handle_under(auto: &Automation, f: &Frame, pos: Pos2, radius: f32) -> Option<usize> {
    (0..auto.points.len().saturating_sub(1))
        .filter_map(|i| handle_at(auto, i, f).map(|c| (i, c.distance(pos))))
        .filter(|(_, d)| *d <= radius)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// How far `pos` is from the curve, vertically, where the curve is at its x.
pub fn distance_to_curve(auto: &Automation, f: &Frame, pos: Pos2) -> Option<f32> {
    let v = auto.value_at(f.tick(pos.x))?;
    Some((f.y(v) - pos.y).abs())
}

/// Clip `r` to the frame's visible x range.
pub fn visible(r: Rect, f: &Frame) -> Rect {
    Rect::from_min_max(
        pos2(r.left().max(f.clip_left), r.top()),
        pos2(r.right().min(f.clip_right), r.bottom()),
    )
}
