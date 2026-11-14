//! Lapsus's curves (ADR 0132, provisional): Fluctus's shape model (ADR 0055) — points across
//! one cycle, each segment shaped as the automation editor shapes it, wrapping from the last
//! point to the first a cycle later — with room for a pencil's steps, and evaluated in `f64`
//! where it can be: a straight segment of the time curve is a read position, and a read
//! position rounded to `f32` across a bar of 96 000 samples would wander by thousandths of
//! a sample (a −48 dB phase noise at 10 kHz).

use crate::fluctus::{CurvePoint, ShapeFn};

/// Points a curve may hold (the model's `MAX_POINTS`).
pub const MAX_POINTS: usize = 64;

/// One curve, its points sorted by place.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Curve {
    pts: [CurvePoint; MAX_POINTS],
    n: usize,
    /// The value with no point.
    empty: f32,
}

impl Curve {
    /// A curve with no point: `value` everywhere.
    pub const fn flat(value: f32) -> Curve {
        Curve {
            pts: [CurvePoint {
                x: 0.0,
                y: 0.0,
                tension: 0.0,
                shape: 0,
            }; MAX_POINTS],
            n: 0,
            empty: value,
        }
    }

    /// Set the points (at most [`MAX_POINTS`]), each clamped into `lo … hi` and place 0 … 1,
    /// sorted by place with equal places in their order — an insertion sort, which allocates
    /// nothing (the audio thread calls this).
    pub fn set(&mut self, points: &[CurvePoint], lo: f32, hi: f32) {
        let n = points.len().min(MAX_POINTS);
        for (d, p) in self.pts.iter_mut().zip(&points[..n]) {
            *d = CurvePoint {
                x: fin(p.x).clamp(0.0, 1.0),
                y: fin(p.y).clamp(lo, hi),
                tension: fin(p.tension).clamp(-1.0, 1.0),
                shape: p.shape.min(5),
            };
        }
        for i in 1..n {
            let mut j = i;
            while j > 0 && self.pts[j - 1].x > self.pts[j].x {
                self.pts.swap(j - 1, j);
                j -= 1;
            }
        }
        self.n = n;
    }

    /// Whether `points` would make this curve again (the engine's check before a set).
    pub fn same_as(&self, points: &[CurvePoint], lo: f32, hi: f32) -> bool {
        let mut c = *self;
        c.set(points, lo, hi);
        c.n == self.n && c.pts[..c.n] == self.pts[..self.n]
    }

    /// The points, sorted.
    pub fn points(&self) -> &[CurvePoint] {
        &self.pts[..self.n]
    }

    /// The one value the curve takes everywhere, if it is flat.
    pub fn constant(&self) -> Option<f32> {
        let p = self.points();
        match p.first() {
            None => Some(self.empty),
            Some(a) => p.iter().all(|q| q.y == a.y).then_some(a.y),
        }
    }

    /// The value at `phase` (0 … 1; a hair outside wraps).
    pub fn at(&self, phase: f64, shape: ShapeFn) -> f64 {
        self.eval(phase, false, shape)
    }

    /// The value just before the cycle's end: what the curve has reached as the next cycle
    /// begins (a held segment holds; a segment that ends on 1 has arrived).
    pub fn left_end(&self, shape: ShapeFn) -> f64 {
        self.eval(1.0, true, shape)
    }

    fn eval(&self, phase: f64, left: bool, shape: ShapeFn) -> f64 {
        let n = self.n;
        if n == 0 {
            return f64::from(self.empty);
        }
        let p = &self.pts[..n];
        // The segment that holds `phase`: from the last point at (or, from the left,
        // before) it, wrapping from the last point to the first one a cycle on.
        let i = if left {
            p.partition_point(|q| f64::from(q.x) < phase)
        } else {
            p.partition_point(|q| f64::from(q.x) <= phase)
        };
        let (a, b, x0, x1) = if i == 0 {
            (
                p[n - 1],
                p[0],
                f64::from(p[n - 1].x) - 1.0,
                f64::from(p[0].x),
            )
        } else if i == n {
            (
                p[n - 1],
                p[0],
                f64::from(p[n - 1].x),
                f64::from(p[0].x) + 1.0,
            )
        } else {
            (p[i - 1], p[i], f64::from(p[i - 1].x), f64::from(p[i].x))
        };
        let span = x1 - x0;
        let (ya, yb) = (f64::from(a.y), f64::from(b.y));
        if span <= 0.0 {
            return yb;
        }
        let u = ((phase - x0) / span).clamp(0.0, 1.0);
        if a.shape == 0 && a.tension == 0.0 {
            // A straight line, in f64 (see the module).
            return ya + (yb - ya) * u;
        }
        ya + (yb - ya) * f64::from(shape(a.shape, a.tension, u as f32))
    }
}

fn fin(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fluctus::linear;

    fn pt(x: f32, y: f32, shape: u8) -> CurvePoint {
        CurvePoint {
            x,
            y,
            tension: 0.0,
            shape,
        }
    }

    /// The automation editor's hold, for the tests: stay, jump at the next point.
    fn hold_or_line(shape: u8, _t: f32, u: f32) -> f32 {
        if shape == 2 {
            0.0
        } else {
            u
        }
    }

    #[test]
    fn an_empty_curve_is_its_value() {
        let c = Curve::flat(1.0);
        assert_eq!(c.at(0.3, linear), 1.0);
        assert_eq!(c.constant(), Some(1.0));
    }

    #[test]
    fn points_sort_and_wrap() {
        let mut c = Curve::flat(0.0);
        c.set(&[pt(0.5, 1.0, 0), pt(0.0, 0.0, 0)], 0.0, 1.0);
        assert_eq!(c.points()[0].x, 0.0);
        assert_eq!(c.at(0.25, linear), 0.5);
        // From the last point back down to the first a cycle on.
        assert_eq!(c.at(0.75, linear), 0.5);
        assert_eq!(c.left_end(linear), 0.0);
        assert_eq!(c.constant(), None);
    }

    #[test]
    fn a_held_step_holds_to_the_end_and_jumps_at_the_next_cycle() {
        let mut c = Curve::flat(0.0);
        c.set(&[pt(0.0, 0.0, 2), pt(0.75, 0.25, 2)], -1.0, 1.0);
        assert_eq!(c.at(0.5, hold_or_line), 0.0);
        assert_eq!(c.at(0.8, hold_or_line), 0.25);
        assert_eq!(c.left_end(hold_or_line), 0.25);
        assert_eq!(c.at(0.0, hold_or_line), 0.0);
    }

    #[test]
    fn a_segment_ending_on_one_has_arrived_there() {
        let mut c = Curve::flat(0.0);
        c.set(&[pt(0.0, 0.0, 0), pt(1.0, 0.5, 0)], -1.0, 1.0);
        assert_eq!(c.left_end(linear), 0.5);
        assert_eq!(c.at(0.0, linear), 0.0);
        assert!((c.at(0.999_999, linear) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_straight_line_is_exact_in_f64() {
        let mut c = Curve::flat(0.0);
        c.set(&[pt(0.0, 0.0, 0), pt(1.0, 0.5, 0)], -1.0, 1.0);
        let u = 12_345.0 / 96_000.0;
        assert_eq!(c.at(u, linear), 0.5 * u);
    }

    #[test]
    fn same_as_sees_a_change() {
        let mut c = Curve::flat(0.0);
        let p = [pt(0.0, 0.0, 0), pt(0.5, 0.2, 2)];
        c.set(&p, 0.0, 1.0);
        assert!(c.same_as(&p, 0.0, 1.0));
        assert!(!c.same_as(&p[..1], 0.0, 1.0));
        assert!(!c.same_as(&[pt(0.0, 0.0, 0), pt(0.5, 0.3, 2)], 0.0, 1.0));
    }
}
