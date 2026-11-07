//! A zero-latency convolver for Chorda's second model (ADR 0098): its body's impulse
//! responses. The first [`B`] taps run in direct form, sample by sample; the rest in
//! uniform partitions of [`B`] through a frequency-domain delay line (overlap-save, FFT of
//! `2B`), computed at each block's start from the blocks already complete. Its blocks count
//! from the stream's start (the instrument's grid), never from a host block, so any split
//! gives the same bits. Everything is allocated in [`Convolver::new`]; `tick` allocates
//! nothing.

use crate::math;

/// The partition (samples): Chorda's chunk.
pub const B: usize = 64;
const N: usize = 2 * B;
const BINS: usize = B + 1;

/// A radix-2 complex FFT of size [`N`] with its twiddles computed once.
#[derive(Clone, Debug)]
struct Fft {
    cos: [f64; N / 2],
    sin: [f64; N / 2],
}

impl Fft {
    fn new() -> Fft {
        let mut cos = [0.0; N / 2];
        let mut sin = [0.0; N / 2];
        for k in 0..N / 2 {
            let a = -std::f64::consts::TAU * k as f64 / N as f64;
            cos[k] = math::cos(a);
            sin[k] = math::sin(a);
        }
        Fft { cos, sin }
    }

    /// In place; `inverse` conjugates the twiddles (no scaling).
    fn run(&self, re: &mut [f64; N], im: &mut [f64; N], inverse: bool) {
        let bits = N.trailing_zeros();
        for i in 0..N {
            let j = i.reverse_bits() >> (usize::BITS - bits);
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let sign = if inverse { -1.0 } else { 1.0 };
        let mut len = 2;
        while len <= N {
            let half = len / 2;
            let stride = N / len;
            for start in (0..N).step_by(len) {
                for k in 0..half {
                    let wr = self.cos[k * stride];
                    let wi = sign * self.sin[k * stride];
                    let a = start + k;
                    let b = a + half;
                    let tr = re[b] * wr - im[b] * wi;
                    let ti = re[b] * wi + im[b] * wr;
                    re[b] = re[a] - tr;
                    im[b] = im[a] - ti;
                    re[a] += tr;
                    im[a] += ti;
                }
            }
            len <<= 1;
        }
    }
}

/// One spectrum of `2B` real points: bins 0 … B.
type Spec = ([f64; BINS], [f64; BINS]);

#[derive(Clone, Debug)]
pub struct Convolver {
    fft: Fft,
    /// The first B taps, and the last B inputs (newest at `hpos`).
    head: [f64; B],
    hist: [f64; B],
    hpos: usize,
    /// The partitions after the head (`h[B(k+1) … B(k+2))`), as spectra.
    parts: Vec<Spec>,
    /// The inputs' spectra, newest at `fpos` (a ring as long as `parts`).
    fdl: Vec<Spec>,
    fpos: usize,
    /// The block being collected, the one before it, the tail's output for this block.
    cur: [f64; B],
    prev: [f64; B],
    tail: [f64; B],
    /// Samples into the current block (the stream's grid).
    pos: usize,
    /// Blocks in a row whose input was exactly zero (the state is all zero after enough).
    quiet: usize,
}

impl Convolver {
    /// A convolver with impulse response `h` (any length; at least one tap). Allocates.
    pub fn new(h: &[f64]) -> Convolver {
        let fft = Fft::new();
        let mut head = [0.0; B];
        for (d, s) in head.iter_mut().zip(h) {
            *d = *s;
        }
        let rest = if h.len() > B { &h[B..] } else { &[][..] };
        let parts: Vec<Spec> = rest
            .chunks(B)
            .map(|c| {
                let mut re = [0.0; N];
                let mut im = [0.0; N];
                re[..c.len()].copy_from_slice(c);
                fft.run(&mut re, &mut im, false);
                let mut s: Spec = ([0.0; BINS], [0.0; BINS]);
                s.0.copy_from_slice(&re[..BINS]);
                s.1.copy_from_slice(&im[..BINS]);
                s
            })
            .collect();
        let fdl = vec![([0.0; BINS], [0.0; BINS]); parts.len().max(1)];
        Convolver {
            fft,
            head,
            hist: [0.0; B],
            hpos: 0,
            parts,
            fdl,
            fpos: 0,
            cur: [0.0; B],
            prev: [0.0; B],
            tail: [0.0; B],
            pos: 0,
            quiet: usize::MAX / 2,
        }
    }

    /// The response's length in partitions after the head.
    pub fn partitions(&self) -> usize {
        self.parts.len()
    }

    /// Whether every state is exactly zero (nothing more can come out until something
    /// goes in).
    pub fn at_rest(&self) -> bool {
        self.quiet > self.parts.len() + 2 && self.cur.iter().all(|x| *x == 0.0)
    }

    pub fn reset(&mut self) {
        self.hist = [0.0; B];
        self.hpos = 0;
        for s in self.fdl.iter_mut() {
            *s = ([0.0; BINS], [0.0; BINS]);
        }
        self.fpos = 0;
        self.cur = [0.0; B];
        self.prev = [0.0; B];
        self.tail = [0.0; B];
        self.pos = 0;
        self.quiet = usize::MAX / 2;
    }

    /// Every state cleared, the grid's phase kept (a model switch).
    pub fn clear(&mut self) {
        let pos = self.pos;
        self.reset();
        self.pos = pos;
    }

    /// Frames skipped at rest: the grid's phase moves on (the state stays zero).
    pub fn skip(&mut self, frames: usize) {
        self.pos = (self.pos + frames % B) % B;
    }

    /// The tail's output for the block about to start, from the blocks complete.
    fn block(&mut self) {
        let silent = self.cur.iter().all(|x| *x == 0.0);
        self.quiet = if silent {
            self.quiet.saturating_add(1)
        } else {
            0
        };
        if self.parts.is_empty() {
            self.prev = self.cur;
            self.cur = [0.0; B];
            return;
        }
        // The newest input's spectrum: [previous block, this block].
        let p = self.parts.len();
        self.fpos = (self.fpos + 1) % p;
        if self.quiet > p + 2 {
            // Every spectrum in the line is already zero.
            self.tail = [0.0; B];
            self.prev = self.cur;
            self.cur = [0.0; B];
            return;
        }
        let mut re = [0.0; N];
        let mut im = [0.0; N];
        if silent && self.prev.iter().all(|x| *x == 0.0) {
            self.fdl[self.fpos] = ([0.0; BINS], [0.0; BINS]);
        } else {
            re[..B].copy_from_slice(&self.prev);
            re[B..].copy_from_slice(&self.cur);
            self.fft.run(&mut re, &mut im, false);
            self.fdl[self.fpos].0.copy_from_slice(&re[..BINS]);
            self.fdl[self.fpos].1.copy_from_slice(&im[..BINS]);
        }
        self.prev = self.cur;
        self.cur = [0.0; B];
        // Σ_k X_{m−k} · H_k for k = 1 … P (X_{m−1} is the one just made).
        let mut ar = [0.0; BINS];
        let mut ai = [0.0; BINS];
        for (k, h) in self.parts.iter().enumerate() {
            let x = &self.fdl[(self.fpos + p - k) % p];
            for b in 0..BINS {
                ar[b] += x.0[b] * h.0[b] - x.1[b] * h.1[b];
                ai[b] += x.0[b] * h.1[b] + x.1[b] * h.0[b];
            }
        }
        // Back to time: the real signal's spectrum is Hermitian.
        let mut re = [0.0; N];
        let mut im = [0.0; N];
        re[..BINS].copy_from_slice(&ar);
        im[..BINS].copy_from_slice(&ai);
        for b in 1..B {
            re[N - b] = ar[b];
            im[N - b] = -ai[b];
        }
        self.fft.run(&mut re, &mut im, true);
        let scale = 1.0 / N as f64;
        for (t, r) in self.tail.iter_mut().zip(&re[B..]) {
            *t = r * scale;
        }
    }

    /// One sample in, one out.
    #[inline]
    pub fn tick(&mut self, x: f64) -> f64 {
        if self.pos == 0 {
            self.block();
        }
        self.hpos = (self.hpos + 1) % B;
        self.hist[self.hpos] = x;
        let mut y = self.tail[self.pos];
        // The head, newest first: the ring read in two straight runs.
        let (newer, older) = self.hist.split_at(self.hpos + 1);
        for (h, v) in self
            .head
            .iter()
            .zip(newer.iter().rev().chain(older.iter().rev()))
        {
            y += h * v;
        }
        self.cur[self.pos] = x;
        self.pos = (self.pos + 1) % B;
        y
    }
}
