//! A staff's skyline: how high and how low its ink reaches over each half space of x —
//! what keeps marks clear of notes and staves clear of each other without the distance a
//! single box would waste.

/// Cell width in spaces.
const CELL: f32 = 0.5;

#[derive(Clone, Debug)]
pub struct Skyline {
    x0: f32,
    top: Vec<f32>,
    bottom: Vec<f32>,
}

impl Skyline {
    /// A skyline over `[x0, x1]` holding the staff itself (lines 0 to 4).
    pub fn new(x0: f32, x1: f32) -> Skyline {
        let n = (((x1 - x0) / CELL).ceil() as usize).max(1) + 2;
        Skyline {
            x0,
            top: vec![0.0; n],
            bottom: vec![4.0; n],
        }
    }

    fn cells(&self, a: f32, b: f32) -> std::ops::Range<usize> {
        let n = self.top.len();
        let i = (((a - self.x0) / CELL).floor().max(0.0) as usize).min(n - 1);
        let j = (((b - self.x0) / CELL).ceil().max(0.0) as usize).clamp(i + 1, n);
        i..j
    }

    /// Take in a box `[x0, y0, x1, y1]`.
    pub fn add(&mut self, b: [f32; 4]) {
        if !b.iter().all(|v| v.is_finite()) {
            return;
        }
        for c in self.cells(b[0], b[2]) {
            self.top[c] = self.top[c].min(b[1]);
            self.bottom[c] = self.bottom[c].max(b[3]);
        }
    }

    /// The highest ink over `[a, b]`.
    pub fn top_in(&self, a: f32, b: f32) -> f32 {
        self.cells(a, b)
            .map(|c| self.top[c])
            .fold(f32::MAX, f32::min)
    }

    /// The lowest ink over `[a, b]`.
    pub fn bottom_in(&self, a: f32, b: f32) -> f32 {
        self.cells(a, b)
            .map(|c| self.bottom[c])
            .fold(f32::MIN, f32::max)
    }

    /// The overall reach above line 0 and below line 4.
    pub fn extents(&self) -> (f32, f32) {
        let top = self.top.iter().copied().fold(0.0, f32::min);
        let bottom = self.bottom.iter().copied().fold(4.0, f32::max);
        (-top, bottom - 4.0)
    }

    /// The least distance from this staff's top line to the next staff's top line so
    /// that, cell by cell, `pad` spaces of white stay between this one's bottom and the
    /// next one's top.
    pub fn distance_to(&self, next: &Skyline, pad: f32) -> f32 {
        let n = self.bottom.len().min(next.top.len());
        let mut d: f32 = 0.0;
        for c in 0..n {
            d = d.max(self.bottom[c] - next.top[c] + pad);
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skylines_keep_staves_apart_only_where_they_reach() {
        let mut a = Skyline::new(0.0, 20.0);
        let mut b = Skyline::new(0.0, 20.0);
        a.add([2.0, 0.0, 3.0, 9.0]);
        b.add([12.0, -5.0, 13.0, 1.0]);
        // They reach toward each other at different x: only the staff heights count.
        assert!((a.distance_to(&b, 1.0) - 10.0).abs() < 1e-6);
        b.add([2.0, -3.0, 3.0, 0.0]);
        assert!((a.distance_to(&b, 1.0) - 13.0).abs() < 1e-6);
        assert_eq!(a.extents(), (0.0, 5.0));
    }
}
