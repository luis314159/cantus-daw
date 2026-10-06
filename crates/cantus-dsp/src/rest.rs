//! **At rest** (ADR 0031 §5.1): the pieces a unit's `at_rest` is built from.
//!
//! A unit is at rest when everything in its state that can reach its output equals what a
//! freshly reset unit with the same settings holds. Then silence in gives exact silence
//! out and leaves the state where it is, so the engine may skip the unit's calls — or
//! reset it — without changing a sample. The predicate is exact, never a threshold: a
//! filter ringing at −400 dB is not at rest, and a unit that cannot tell says `false`.
//!
//! What does *not* count: positions in a ring that holds only zeros (every position of an
//! all-zero ring reads the same), counters that only number telemetry columns, and meters
//! that only the editor reads. What *does*: every filter and envelope state, every glide
//! that has not landed on its target, every delay line, every running phase, every
//! random-number state that advances on silence, and any meter the audio depends on
//! (auto gain, program-dependent release, a loudness match).
//!
//! Long lines are not scanned when the engine asks: a [`ZeroRun`] beside the line counts
//! the zeros written into it in a row, after each block, from the end of what was written
//! (one comparison when the block carried sound), so `at_rest` stays a few comparisons.

/// Every value is zero (of either sign).
#[inline]
pub fn all_zero(x: &[f32]) -> bool {
    x.iter().all(|v| *v == 0.0)
}

/// Every value is zero (of either sign).
#[inline]
pub fn all_zero_f64(x: &[f64]) -> bool {
    x.iter().all(|v| *v == 0.0)
}

/// How many zeros a ring has received in a row, counted back from its newest write. When
/// the count reaches the ring's length, every cell holds a zero.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ZeroRun {
    run: usize,
}

impl Default for ZeroRun {
    /// A new ring is all zeros.
    fn default() -> Self {
        ZeroRun::FULL
    }
}

impl ZeroRun {
    /// The count of a ring that holds only zeros, however long.
    pub const FULL: ZeroRun = ZeroRun { run: usize::MAX };

    /// The ring was cleared.
    #[inline]
    pub fn clear(&mut self) {
        *self = ZeroRun::FULL;
    }

    /// The ring has just received a value that may not be zero, and nothing is known yet.
    #[inline]
    pub fn dirty(&mut self) {
        self.run = 0;
    }

    /// One value was written.
    #[inline]
    pub fn push(&mut self, x: f32) {
        self.run = if x == 0.0 {
            self.run.saturating_add(1)
        } else {
            0
        };
    }

    /// `n` values were written into `ring` (circular), the newest just before index `end`
    /// (so the next write goes to `end`). Scans back from the newest write and stops at the
    /// first value that is not zero: one comparison while sound passes.
    pub fn wrote(&mut self, ring: &[f32], end: usize, n: usize) {
        let len = ring.len();
        if n == 0 || len == 0 {
            return;
        }
        let m = n.min(len);
        let end = end % len;
        // The newest `m` writes are ring[..end] (newest last) and then ring[len - (m - end)..]
        // when they wrap.
        let head = &ring[end - end.min(m)..end];
        let trailing = |s: &[f32]| s.iter().rev().take_while(|v| **v == 0.0).count();
        let t_head = trailing(head);
        let t = if t_head < head.len() {
            t_head
        } else {
            let rest = m - head.len();
            let tail = &ring[len - rest..];
            t_head + trailing(tail)
        };
        self.run = if t < m {
            t
        } else if n <= len {
            self.run.saturating_add(n)
        } else {
            // The whole ring was written in this call, and all of it is zero.
            self.run.max(len)
        };
    }

    /// The last `len` writes were zeros: a ring of that length holds nothing else.
    #[inline]
    pub fn covers(&self, len: usize) -> bool {
        self.run >= len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reference: write into a ring one value at a time and look at every cell.
    fn check(len: usize, writes: &[f32], blocks: &[usize]) {
        let mut ring = vec![0f32; len];
        let mut pos = 0;
        let mut run = ZeroRun::default();
        let mut at = 0;
        for &b in blocks {
            let b = b.min(writes.len() - at);
            for &x in &writes[at..at + b] {
                ring[pos] = x;
                pos = (pos + 1) % len;
            }
            run.wrote(&ring, pos, b);
            at += b;
            assert_eq!(
                run.covers(len),
                ring.iter().all(|v| *v == 0.0),
                "len {len} after {at} writes"
            );
        }
    }

    #[test]
    fn counts_the_zeros_a_ring_holds_at_any_block_size() {
        let mut seed = 7u32;
        for len in [1usize, 2, 5, 16, 33] {
            for trial in 0..40 {
                let writes: Vec<f32> = (0..300)
                    .map(|i| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        // Long runs of zeros with a few values among them.
                        if (seed % 23) == 0 || (i < 20 && trial % 3 == 0) {
                            if seed % 2 == 0 {
                                1.0
                            } else {
                                -0.0
                            }
                        } else {
                            0.0
                        }
                    })
                    .collect();
                let blocks: Vec<usize> = (0..300)
                    .map(|_| {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        1 + (seed as usize % (2 * len + 3))
                    })
                    .collect();
                check(len, &writes, &blocks);
            }
        }
    }

    #[test]
    fn push_counts_like_wrote() {
        let mut a = ZeroRun::default();
        a.push(1.0);
        assert!(!a.covers(1));
        a.push(0.0);
        assert!(a.covers(1) && !a.covers(2));
        a.clear();
        assert!(a.covers(usize::MAX));
    }
}
