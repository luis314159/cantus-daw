//! A clip's gain envelope read sample by sample on the audio thread (ADR 0070).
//!
//! The envelope's points are in the clip's content ticks; each lands on the transport
//! sample its timeline tick rounds to (`start − offset + tick`), and between two points
//! the line is travelled by the sample's own position — so a block of any size, and the
//! offline render, read the same value at the same sample. A reversed window reads the
//! line mirrored across itself, exactly where it reads its audio mirrored. Allocation-free:
//! a cursor over the points, its two edges in samples kept until the line moves past them.

use cantus_core::clip_gain::{gain_of, segment, GainEnvelope};

/// The cursor: which segment the last sample lay in, and where its two points fall.
pub(crate) struct EnvRead<'a> {
    env: &'a GainEnvelope,
    /// Timeline tick of content tick 0.
    origin: f64,
    /// `start + end − 1` in samples, for a reversed window: sample `s` reads the line where
    /// a forward window reads it at `mirror − s`.
    mirror: Option<i64>,
    /// The segment: points `i − 1` and `i` (0: before the first, `len`: after the last).
    i: usize,
    lo: i64,
    hi: i64,
    /// The last value and its gain (a level stretch costs no `exp`).
    db: f32,
    gain: f32,
}

impl<'a> EnvRead<'a> {
    /// A cursor for a window from `start` to `end` (samples) whose content tick 0 is on
    /// timeline tick `origin`, placed for sample `first`. `at(tick)` is the transport's
    /// tick-to-sample rounding (the one the window's edges use).
    pub(crate) fn new(
        env: &'a GainEnvelope,
        origin: f64,
        reversed: Option<(i64, i64)>,
        first: i64,
        at: &dyn Fn(f64) -> i64,
    ) -> Self {
        let mut r = EnvRead {
            env,
            origin,
            mirror: reversed.map(|(s, e)| s + e - 1),
            i: 0,
            lo: i64::MIN,
            hi: i64::MAX,
            db: f32::NAN,
            gain: 1.0,
        };
        let x = r.forward(first);
        // The first point at or after `x`: by bisection, once per block.
        let (mut a, mut b) = (0usize, env.points.len());
        while a < b {
            let m = (a + b) / 2;
            if r.pos(m, at) <= x {
                a = m + 1;
            } else {
                b = m;
            }
        }
        r.set(a, at);
        r
    }

    #[inline]
    fn forward(&self, s: i64) -> i64 {
        match self.mirror {
            Some(m) => m - s,
            None => s,
        }
    }

    #[inline]
    fn pos(&self, k: usize, at: &dyn Fn(f64) -> i64) -> i64 {
        at(self.origin + self.env.points[k].tick.0 as f64)
    }

    fn set(&mut self, i: usize, at: &dyn Fn(f64) -> i64) {
        self.i = i;
        self.lo = if i == 0 {
            i64::MIN
        } else {
            self.pos(i - 1, at)
        };
        self.hi = if i >= self.env.points.len() {
            i64::MAX
        } else {
            self.pos(i, at)
        };
    }

    /// The gain at transport sample `s`. Samples come in order (forwards in time), so the
    /// cursor moves a segment at a time.
    #[inline]
    pub(crate) fn gain(&mut self, s: i64, at: &dyn Fn(f64) -> i64) -> f32 {
        let x = self.forward(s);
        while x >= self.hi {
            self.set(self.i + 1, at);
        }
        while x < self.lo {
            self.set(self.i - 1, at);
        }
        let pts = &self.env.points;
        let db = if self.i == 0 {
            pts[0].db
        } else if self.i >= pts.len() {
            pts[pts.len() - 1].db
        } else {
            let (p, q) = (&pts[self.i - 1], &pts[self.i]);
            let span = self.hi - self.lo;
            if span <= 0 {
                q.db
            } else {
                segment(p, q, ((x - self.lo) as f64 / span as f64) as f32)
            }
        };
        if db != self.db {
            self.db = db;
            self.gain = gain_of(db);
        }
        self.gain
    }
}
