//! Following an outside clock (round 33, ADR 0078 §4): the transport's tempo comes from a
//! sync source instead of the tempo map, which is **followed, not rewritten** — the
//! project's map is untouched and comes back when following ends.
//!
//! While following, the position is a chain of straight stretches ("segments"), each a
//! sample (from the origin), the tick there and the ticks per sample from there on. A
//! source's tempo change, or the follower's small correction of the phase, **adds** a
//! segment at the current sample: the samples already played keep their ticks, so nothing
//! jumps — a playing audio clip is placed through the stretch it started in and plays on
//! from the same frame, a note already due stays due. Only a seek or a loop's wrap starts
//! the chain again (one segment at the new origin).
//!
//! The chain holds [`SEGS`] stretches. When it is full the two neighbours whose union moves
//! the played samples least become one — the straight line from the first's start to the
//! second's end, every joint exact: a sample under them moves by their tempo difference
//! times their lengths' harmonic share. Retunes are a few ppm and half a second apart, so
//! that is a fraction of a sample (`a_full_chain_merges_where_it_costs_least`).

use cantus_core::PPQ;

/// The stretches kept.
pub const SEGS: usize = 32;

/// One stretch: from sample `s` (counted from the origin) at tick `tick`, `tps` ticks a
/// sample.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct Seg {
    pub s: f64,
    pub tick: f64,
    pub tps: f64,
}

/// The chain, oldest first.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct Follow {
    segs: [Seg; SEGS],
    n: usize,
}

impl Follow {
    /// One stretch from sample `s` at `tick`.
    pub fn one(s: f64, tick: f64, tps: f64) -> Follow {
        let mut segs = [Seg::default(); SEGS];
        segs[0] = Seg { s, tick, tps };
        Follow { segs, n: 1 }
    }

    pub fn last(&self) -> Seg {
        self.segs[self.n - 1]
    }

    /// The stretch sample `s` lies in (before the first: the first, extrapolated).
    fn by_sample(&self, s: f64) -> Seg {
        self.segs[..self.n]
            .iter()
            .rev()
            .find(|g| g.s <= s)
            .copied()
            .unwrap_or(self.segs[0])
    }

    /// The stretch tick `t` lies in.
    pub fn by_tick(&self, t: f64) -> Seg {
        self.segs[..self.n]
            .iter()
            .rev()
            .find(|g| g.tick <= t)
            .copied()
            .unwrap_or(self.segs[0])
    }

    /// The tick at sample `s`.
    pub fn pos(&self, s: f64) -> f64 {
        let g = self.by_sample(s);
        g.tick + (s - g.s) * g.tps
    }

    /// The sample (fractional) tick `t` falls on.
    pub fn sample(&self, t: f64) -> f64 {
        let g = self.by_tick(t);
        g.s + (t - g.tick) / g.tps
    }

    /// From sample `s` on, `tps` ticks a sample.
    pub fn push(&mut self, s: f64, tps: f64) {
        let tick = self.pos(s);
        let last = &mut self.segs[self.n - 1];
        if last.s >= s {
            // A stretch of no length yet: retune it (nothing was played on it).
            last.tps = tps;
            return;
        }
        if self.n == SEGS {
            // Merge the neighbours whose union moves the played samples least: a straight
            // line from the first's start to the next one's end.
            let cost = |i: usize| {
                let (a, b, c) = (self.segs[i], self.segs[i + 1], self.segs[i + 2]);
                let (la, lb) = (b.s - a.s, c.s - b.s);
                (a.tps - b.tps).abs() * la * lb / (la + lb)
            };
            let i = (0..self.n - 2)
                .min_by(|x, y| cost(*x).total_cmp(&cost(*y)))
                .unwrap_or(0);
            let (a, c) = (self.segs[i], self.segs[i + 2]);
            self.segs[i].tps = (c.tick - a.tick) / (c.s - a.s);
            self.segs.copy_within(i + 2.., i + 1);
            self.n -= 1;
        }
        self.segs[self.n] = Seg { s, tick, tps };
        self.n += 1;
    }

    pub fn len(&self) -> usize {
        self.n
    }
}

/// Ticks per sample at `bpm`.
pub(crate) fn tps_of(bpm: f64, sample_rate: f64) -> f64 {
    bpm * PPQ as f64 / (60.0 * sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_stretch_moves_nothing_already_played() {
        let mut f = Follow::one(0.0, 0.0, 0.02);
        let before: Vec<f64> = (0..10).map(|k| f.pos(k as f64 * 1000.0)).collect();
        f.push(10_000.0, 0.021);
        let after: Vec<f64> = (0..10).map(|k| f.pos(k as f64 * 1000.0)).collect();
        assert_eq!(before, after);
        // Continuous at the joint, and the inverse agrees on both sides.
        assert!((f.pos(10_000.0) - 200.0).abs() < 1e-9);
        for t in [50.0, 199.0, 200.0, 250.0] {
            assert!((f.pos(f.sample(t)) - t).abs() < 1e-9, "{t}");
        }
    }

    #[test]
    fn a_full_chain_merges_where_it_costs_least() {
        let mut f = Follow::one(0.0, 0.0, 0.02);
        let mut before = Vec::new();
        for k in 1..=40 {
            f.push(k as f64 * 24_000.0, 0.02 * (1.0 + k as f64 * 5e-6));
            before.push(f.pos(k as f64 * 24_000.0 - 7.0));
        }
        assert_eq!(f.len(), SEGS);
        assert!((f.last().tps - 0.02 * (1.0 + 40.0 * 5e-6)).abs() < 1e-15);
        // Every sample already played kept its tick within a sample, through 200 ppm of
        // retunes (far more than a follower makes).
        for (k, b) in before.iter().enumerate() {
            let s = (k + 1) as f64 * 24_000.0 - 7.0;
            let moved = (f.pos(s) - b) / 0.02;
            assert!(moved.abs() < 1.0, "sample {s} moved {moved}");
        }
    }
}
