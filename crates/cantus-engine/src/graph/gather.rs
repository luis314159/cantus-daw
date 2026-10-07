//! The canonical gather (ADR 0031 §1.3): how a node sums what its inputs left for it, in
//! an order fixed by the plan, so that a parallel schedule adds exactly what the serial
//! engine added, in exactly the same order — the same bits.
//!
//! > A node's input is an accumulator that starts at **+0.0** and receives its
//! > contributions **in canonical order**: first the tracks that feed it, in snapshot
//! > track order; then, for each strip in `strip_order`, that strip's output edge if it
//! > leads here, then its sends `k = 0, 1, …` that lead here. On a mono device the master
//! > adds each contribution's left, then its right, into the one channel.
//!
//! That is the order in which the serial engine *scattered* into a strip's buffer
//! (`d.buf_l[i] += …` as each source finished), so a gather in it reproduces those sums bit
//! for bit (`tests/graph_order.rs` holds it on random topologies). Each contribution is the
//! expression the serial engine computed: a producer's buffer added as it is (a track's
//! leveled output, a strip's output edge, a delayed edge's line — `x · 1.0` is `x`), or a
//! strip's post-chain pair times a gain (an undelayed send, `l · (g · level)`).
//!
//! Two consequences, both exact. A contribution that did not come (an idle track, a strip
//! that did not run) is simply not in the list: under round-to-nearest a sum is −0.0 only
//! when both addends are, and the accumulator starts at +0.0, so `x + (±0.0) == x` for
//! every value it can hold. And who computed which input, and when, cannot change a sample.

/// One input, as its producer left it for this pass.
#[derive(Copy, Clone, Debug)]
pub enum Contribution<'a> {
    /// A buffer pair added as it is.
    Plain(&'a [f32], &'a [f32]),
    /// A buffer pair times a gain per channel, added.
    Scaled(&'a [f32], &'a [f32], f32, f32),
}

impl Contribution<'_> {
    /// Frames this contribution holds.
    pub fn len(&self) -> usize {
        match self {
            Contribution::Plain(l, _) | Contribution::Scaled(l, _, _, _) => l.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Add one contribution into a stereo accumulator. `first`: the accumulator holds nothing
/// yet, and starts at +0.0 here (`0.0 + x`, never `x`: a −0.0 contribution must become
/// +0.0, as it did in a zeroed buffer).
#[inline]
pub fn add_stereo(acc_l: &mut [f32], acc_r: &mut [f32], c: Contribution<'_>, first: bool) {
    let n = acc_l.len().min(acc_r.len());
    match c {
        Contribution::Plain(l, r) => {
            let (l, r) = (&l[..n], &r[..n]);
            if first {
                for (a, x) in acc_l[..n].iter_mut().zip(l) {
                    *a = 0.0 + *x;
                }
                for (a, x) in acc_r[..n].iter_mut().zip(r) {
                    *a = 0.0 + *x;
                }
            } else {
                for (a, x) in acc_l[..n].iter_mut().zip(l) {
                    *a += *x;
                }
                for (a, x) in acc_r[..n].iter_mut().zip(r) {
                    *a += *x;
                }
            }
        }
        Contribution::Scaled(l, r, gl, gr) => {
            let (l, r) = (&l[..n], &r[..n]);
            if first {
                for (a, x) in acc_l[..n].iter_mut().zip(l) {
                    *a = 0.0 + *x * gl;
                }
                for (a, x) in acc_r[..n].iter_mut().zip(r) {
                    *a = 0.0 + *x * gr;
                }
            } else {
                for (a, x) in acc_l[..n].iter_mut().zip(l) {
                    *a += *x * gl;
                }
                for (a, x) in acc_r[..n].iter_mut().zip(r) {
                    *a += *x * gr;
                }
            }
        }
    }
}

/// Gather `inputs` in the order given into a stereo accumulator; `false` when none came
/// (the accumulator is then untouched).
pub fn gather_stereo<'a>(
    acc_l: &mut [f32],
    acc_r: &mut [f32],
    inputs: impl IntoIterator<Item = Contribution<'a>>,
) -> bool {
    let mut first = true;
    for c in inputs {
        add_stereo(acc_l, acc_r, c, first);
        first = false;
    }
    !first
}

/// Add one contribution into the device's interleaved buffer, the master's accumulator
/// (zeroed at the top of the block): left into channel 0 and right into channel 1, or
/// left and then right into the one channel of a mono device.
#[inline]
pub fn add_interleaved(out: &mut [f32], channels: usize, c: Contribution<'_>) {
    let channels = channels.max(1);
    let n = out.len() / channels;
    let (l, r, gl, gr, scaled) = match c {
        Contribution::Plain(l, r) => (l, r, 1.0, 1.0, false),
        Contribution::Scaled(l, r, gl, gr) => (l, r, gl, gr, true),
    };
    let (l, r) = (&l[..n], &r[..n]);
    match (channels, scaled) {
        (1, false) => {
            for ((o, a), b) in out.iter_mut().zip(l).zip(r) {
                *o += *a;
                *o += *b;
            }
        }
        (1, true) => {
            for ((o, a), b) in out.iter_mut().zip(l).zip(r) {
                *o += *a * gl;
                *o += *b * gr;
            }
        }
        (2, false) => {
            for ((f, a), b) in out.chunks_exact_mut(2).zip(l).zip(r) {
                f[0] += *a;
                f[1] += *b;
            }
        }
        (_, false) => {
            for ((f, a), b) in out.chunks_exact_mut(channels).zip(l).zip(r) {
                f[0] += *a;
                f[1] += *b;
            }
        }
        (_, true) => {
            for ((f, a), b) in out.chunks_exact_mut(channels).zip(l).zip(r) {
                f[0] += *a * gl;
                f[1] += *b * gr;
            }
        }
    }
}

/// Gather `inputs` in the order given into the interleaved buffer; `false` when none came.
pub fn gather_interleaved<'a>(
    out: &mut [f32],
    channels: usize,
    inputs: impl IntoIterator<Item = Contribution<'a>>,
) -> bool {
    let mut any = false;
    for c in inputs {
        add_interleaved(out, channels, c);
        any = true;
    }
    any
}
