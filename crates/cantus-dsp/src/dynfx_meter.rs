//! What the dynamics family's units tell their editors (ADR 0056), built so that an editor
//! reading the telemetry once a frame **loses nothing** between frames:
//!
//! - [`Hold`] keeps a peak for at least a window of audio (≈ 50 ms), so a transient in a
//!   block the editor never saw is still there when it looks.
//! - [`Columns`] cuts the stream into columns of [`COL_LEN`] samples counted from the start
//!   of the stream (not from a block), reduces three readings over each (a maximum or a
//!   minimum), and keeps the last [`COLS`] in a ring with a running count: the editor
//!   appends every column it has not seen, so its history is the audio's, whatever the
//!   blocks and the frame rate.
//!
//! Neither counts toward rest: only the editor reads them (ADR 0031 §5.1).

/// Samples per column (10.7 ms at 48 kHz).
pub const COL_LEN: usize = 512;
/// Columns kept in the ring (170 ms at 48 kHz: several frames of slack).
pub const COLS: usize = 16;
/// Where the ring starts in the 64 numbers of the telemetry; three numbers a column.
pub const COL_AT: usize = 16;
/// Where the running count of finished columns is.
pub const COL_COUNT: usize = 2;
/// Counters wrap here, where `f32` still counts by one.
pub const WRAP: u32 = 1 << 24;

/// A peak kept for at least `window` samples.
#[derive(Copy, Clone, Debug, Default)]
pub struct Hold {
    cur: f32,
    prev: f32,
    age: u32,
}

impl Hold {
    /// A block's peak `v` over `frames` samples. A non-finite level is not kept.
    pub fn push(&mut self, v: f32, frames: usize, window: u32) {
        if self.age >= window {
            self.prev = self.cur;
            self.cur = 0.0;
            self.age = 0;
        }
        if v.is_finite() {
            self.cur = self.cur.max(v);
        }
        self.age = self.age.saturating_add(frames.min(u32::MAX as usize) as u32);
    }

    pub fn get(&self) -> f32 {
        self.cur.max(self.prev)
    }

    pub fn reset(&mut self) {
        *self = Hold::default();
    }
}

/// How a column reduces a reading.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Fold {
    Max,
    Min,
}

/// Columns of three readings.
#[derive(Clone, Debug)]
pub struct Columns {
    folds: [Fold; 3],
    acc: [f32; 3],
    n: usize,
    count: u32,
    ring: [[f32; 3]; COLS],
}

impl Columns {
    pub fn new(folds: [Fold; 3]) -> Columns {
        let mut c = Columns {
            folds,
            acc: [0.0; 3],
            n: 0,
            count: 0,
            ring: [[0.0; 3]; COLS],
        };
        c.clear_acc();
        c
    }

    fn clear_acc(&mut self) {
        for (a, f) in self.acc.iter_mut().zip(self.folds) {
            *a = match f {
                Fold::Max => f32::NEG_INFINITY,
                Fold::Min => f32::INFINITY,
            };
        }
    }

    /// One sample's readings.
    #[inline]
    pub fn push(&mut self, v: [f32; 3]) {
        for i in 0..3 {
            if v[i].is_finite() {
                self.acc[i] = match self.folds[i] {
                    Fold::Max => self.acc[i].max(v[i]),
                    Fold::Min => self.acc[i].min(v[i]),
                };
            }
        }
        self.n += 1;
        if self.n == COL_LEN {
            self.finish();
        }
    }

    fn finish(&mut self) {
        let mut col = self.acc;
        for c in &mut col {
            if !c.is_finite() {
                *c = 0.0;
            }
        }
        self.ring[(self.count as usize) % COLS] = col;
        self.count = (self.count + 1) % WRAP;
        self.n = 0;
        self.clear_acc();
    }

    /// `frames` samples of the same readings `v` (silence skipped at rest).
    pub fn skip(&mut self, mut frames: usize, v: [f32; 3]) {
        while frames > 0 {
            let take = (COL_LEN - self.n).min(frames);
            for i in 0..3 {
                self.acc[i] = match self.folds[i] {
                    Fold::Max => self.acc[i].max(v[i]),
                    Fold::Min => self.acc[i].min(v[i]),
                };
            }
            self.n += take;
            frames -= take;
            if self.n == COL_LEN {
                self.finish();
            }
        }
    }

    pub fn reset(&mut self) {
        *self = Columns::new(self.folds);
    }

    /// Into the telemetry: the count at [`COL_COUNT`], the ring from [`COL_AT`].
    pub fn write(&self, out: &mut [f32; 64]) {
        out[COL_COUNT] = self.count as f32;
        for (i, c) in self.ring.iter().enumerate() {
            out[COL_AT + 3 * i..COL_AT + 3 * i + 3].copy_from_slice(c);
        }
    }
}

/// The columns an editor has not seen yet, oldest first: from the telemetry's count and
/// ring, given the count it saw last (`None` the first time: everything in the ring).
pub fn new_columns(tel: &[f32; 64], last: Option<u32>, mut f: impl FnMut([f32; 3])) -> u32 {
    let now = tel[COL_COUNT] as u32;
    let fresh = match last {
        Some(l) => ((now + WRAP - l) % WRAP).min(COLS as u32),
        None => now.min(COLS as u32),
    };
    for j in 0..fresh {
        let c = (now + WRAP - fresh + j) % WRAP;
        let i = (c as usize) % COLS;
        let at = COL_AT + 3 * i;
        f([tel[at], tel[at + 1], tel[at + 2]]);
    }
    now
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_are_counted_from_the_stream_and_read_without_loss() {
        let mut c = Columns::new([Fold::Max, Fold::Min, Fold::Max]);
        let mut seen = Vec::new();
        let mut last = None;
        let mut t = [0f32; 64];
        // 40 columns' worth in blocks of 300, read after every third block.
        for blk in 0..(40 * COL_LEN).div_ceil(300) {
            for i in 0..300 {
                let n = blk * 300 + i;
                let col = n / COL_LEN;
                c.push([col as f32, -(col as f32), (n % COL_LEN) as f32]);
            }
            if blk % 3 == 2 {
                c.write(&mut t);
                last = Some(new_columns(&t, last, |v| seen.push(v)));
            }
        }
        c.write(&mut t);
        new_columns(&t, last, |v| seen.push(v));
        assert_eq!(seen.len(), 40);
        for (i, v) in seen.iter().enumerate() {
            assert_eq!(*v, [i as f32, -(i as f32), (COL_LEN - 1) as f32]);
        }
    }

    #[test]
    fn a_skip_counts_columns_as_silence_would() {
        let mut a = Columns::new([Fold::Max; 3]);
        let mut b = Columns::new([Fold::Max; 3]);
        for _ in 0..100 {
            a.push([1.0, 2.0, 3.0]);
            b.push([1.0, 2.0, 3.0]);
        }
        for _ in 0..3_000 {
            a.push([0.0; 3]);
        }
        b.skip(3_000, [0.0; 3]);
        let (mut ta, mut tb) = ([0f32; 64], [0f32; 64]);
        a.write(&mut ta);
        b.write(&mut tb);
        assert_eq!(ta, tb);
    }

    #[test]
    fn a_hold_keeps_a_peak_for_its_window() {
        let mut h = Hold::default();
        h.push(0.9, 480, 2_400);
        for _ in 0..4 {
            h.push(0.1, 480, 2_400);
        }
        assert_eq!(h.get(), 0.9);
        for _ in 0..6 {
            h.push(0.1, 480, 2_400);
        }
        assert_eq!(h.get(), 0.1);
        h.push(f32::NAN, 480, 2_400);
        assert_eq!(h.get(), 0.1);
    }
}
