//! Multiplex's linear-phase split: one linear-phase low-pass per crossover, run by
//! uniformly partitioned FFT convolution, and summed **by telescoping** so the bands add up
//! to a delayed copy of the input by construction:
//!
//! ```text
//! band_k = L_k − L_(k−1)        (L_(−1) = 0, the top "low-pass" = the delay D)
//! y = Σ g_k band_k = g_top · x(t − D) + Σ_c (g_c − g_(c+1)) · L_c(x)
//! ```
//!
//! With every band at the same gain each `g_c − g_(c+1)` is exactly zero and the output is
//! the delayed input times that gain — to the bit, whatever the filters are. Each `L_c` is
//! designed by frequency sampling: the magnitude of the Linkwitz–Riley low half at that
//! crossover and slope, zero phase, windowed (a Tukey window, flat over the middle half) to `taps` samples and
//! centred, so its delay is `taps / 2`. A crossover that moves is redesigned at the next
//! block and cross-faded across it, so a dragged crossover does not zip.
//!
//! Everything is allocated in `new`; `taps` scales with the sample rate so the filters
//! keep their resolution (about 11.7 Hz, 85 ms of impulse) at 44.1 through 192 kHz.

use super::filters::{xover_response, Slope, XoverCoefs};
use super::MAX_XOVERS;
use crate::math;

/// Partitions per filter: the block is `taps / PARTS`.
const PARTS: usize = 16;

/// An iterative radix-2 complex FFT with its twiddles and bit reversal in tables.
#[derive(Clone, Debug)]
pub struct Fft {
    n: usize,
    cos: Vec<f32>,
    sin: Vec<f32>,
    rev: Vec<usize>,
}

impl Fft {
    pub fn new(n: usize) -> Self {
        assert!(n.is_power_of_two() && n >= 2);
        let bits = n.trailing_zeros();
        let rev = (0..n)
            .map(|i| i.reverse_bits() >> (usize::BITS - bits))
            .collect();
        let (cos, sin) = (0..n / 2)
            .map(|k| {
                let a = -std::f64::consts::TAU * k as f64 / n as f64;
                (math::cos(a) as f32, math::sin(a) as f32)
            })
            .unzip();
        Fft { n, cos, sin, rev }
    }

    /// In place; `inverse` conjugates the twiddles and scales by 1/n.
    pub fn run(&self, re: &mut [f32], im: &mut [f32], inverse: bool) {
        let n = self.n;
        for i in 0..n {
            let j = self.rev[i];
            if i < j {
                re.swap(i, j);
                im.swap(i, j);
            }
        }
        let sign = if inverse { -1.0 } else { 1.0 };
        let mut len = 2;
        while len <= n {
            let half = len / 2;
            let step = n / len;
            for start in (0..n).step_by(len) {
                for k in 0..half {
                    let (wr, wi) = (self.cos[k * step], sign * self.sin[k * step]);
                    let (a, b) = (start + k, start + k + half);
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
        if inverse {
            let k = 1.0 / n as f32;
            for (r, i) in re.iter_mut().zip(im.iter_mut()) {
                *r *= k;
                *i *= k;
            }
        }
    }
}

/// The linear-phase split for two channels (the mid and the side).
#[derive(Clone, Debug)]
pub struct Linear {
    sr: f32,
    taps: usize,
    block: usize,
    /// FFT of two blocks (overlap-save), and of the whole filter (design).
    small: Fft,
    big: Fft,
    /// Per crossover, per partition: the kernel's spectrum (`block + 1` bins), in force and
    /// next (when it is being cross-faded to).
    kern: Vec<[Vec<f32>; 2]>,
    next: Vec<[Vec<f32>; 2]>,
    changing: [bool; MAX_XOVERS],
    designed: [(f32, Option<Slope>); MAX_XOVERS],
    /// Per channel: the frequency-domain delay line of input spectra, and where the newest is.
    fdl: [[Vec<f32>; 2]; 2],
    fdl_at: usize,
    /// Per channel: the last two blocks of input.
    frame: [Vec<f32>; 2],
    /// Per crossover, per channel: this block's output of `L_c`.
    out: Vec<[Vec<f32>; 2]>,
    /// The input, delayed by the filters' latency, per channel.
    delay: [Vec<f32>; 2],
    delay_at: usize,
    /// Position inside the block.
    pos: usize,
    /// Each crossover's output for the frame just run.
    out_last: [[f32; 2]; MAX_XOVERS],
    re: Vec<f32>,
    im: Vec<f32>,
    acc: [Vec<f32>; 2],
    acc_old: [Vec<f32>; 2],
    design_re: Vec<f32>,
    design_im: Vec<f32>,
    window: Vec<f32>,
}

impl Linear {
    pub fn new(sample_rate: f32) -> Self {
        // 4096 taps at 44.1–48 kHz, and in proportion above.
        let taps = ((4096.0 * sample_rate / 48_000.0) as usize)
            .next_power_of_two()
            .clamp(1024, 16_384);
        let block = taps / PARTS;
        let bins = block + 1;
        let spectra = || [vec![0.0; PARTS * bins], vec![0.0; PARTS * bins]];
        let window = (0..taps)
            .map(|n| {
                // Flat over the middle half and a raised cosine over the outer quarters
                // (a Tukey window, 1 at taps/2): the low crossovers' long impulses keep more
                // of themselves than under a Blackman–Harris, and the ends still reach 0.
                let d = (n as f64 - (taps / 2) as f64).abs() / (taps / 2) as f64;
                if d <= 0.5 {
                    1.0
                } else {
                    (0.5 + 0.5 * math::cos(std::f64::consts::PI * (d - 0.5) / 0.5)) as f32
                }
            })
            .collect();
        Linear {
            sr: sample_rate,
            taps,
            block,
            small: Fft::new(2 * block),
            big: Fft::new(taps),
            kern: (0..MAX_XOVERS).map(|_| spectra()).collect(),
            next: (0..MAX_XOVERS).map(|_| spectra()).collect(),
            changing: [false; MAX_XOVERS],
            designed: [(0.0, None); MAX_XOVERS],
            fdl: [spectra(), spectra()],
            fdl_at: 0,
            frame: [vec![0.0; 2 * block], vec![0.0; 2 * block]],
            out: (0..MAX_XOVERS)
                .map(|_| [vec![0.0; block], vec![0.0; block]])
                .collect(),
            delay: [vec![0.0; taps / 2 + block], vec![0.0; taps / 2 + block]],
            delay_at: 0,
            pos: 0,
            out_last: [[0.0; 2]; MAX_XOVERS],
            re: vec![0.0; 2 * block],
            im: vec![0.0; 2 * block],
            acc: [vec![0.0; bins], vec![0.0; bins]],
            acc_old: [vec![0.0; bins], vec![0.0; bins]],
            design_re: vec![0.0; taps],
            design_im: vec![0.0; taps],
            window,
        }
    }

    /// Samples of delay: the filters' centre plus one block of collection.
    pub fn latency(&self) -> usize {
        self.taps / 2 + self.block
    }

    pub fn taps(&self) -> usize {
        self.taps
    }

    /// The next frame starts a block: the moment to ask for new filters.
    pub fn at_block_start(&self) -> bool {
        self.pos == 0
    }

    pub fn reset(&mut self) {
        for ch in 0..2 {
            for v in &mut self.fdl[ch] {
                v.fill(0.0);
            }
            self.frame[ch].fill(0.0);
            self.delay[ch].fill(0.0);
        }
        for o in &mut self.out {
            for v in o.iter_mut() {
                v.fill(0.0);
            }
        }
        self.pos = 0;
        self.delay_at = 0;
        self.fdl_at = 0;
        self.out_last = [[0.0; 2]; MAX_XOVERS];
    }

    /// Ask for crossover `c` at `hz` with `slope`; designed at the next block. The first
    /// design of a crossover is taken at once, later ones are cross-faded to.
    pub fn want(&mut self, c: usize, hz: f32, slope: Slope) {
        let (at, sl) = self.designed[c];
        if sl == Some(slope) && (at - hz).abs() <= hz * 1e-4 {
            return;
        }
        let fresh = sl.is_none();
        self.designed[c] = (hz, Some(slope));
        self.design(c, hz, slope);
        if fresh {
            let (k, n) = (&mut self.kern[c], &self.next[c]);
            for side in 0..2 {
                k[side].copy_from_slice(&n[side]);
            }
            self.changing[c] = false;
        } else {
            self.changing[c] = true;
        }
    }

    /// Design `L_c` into `next[c]`: zero-phase Linkwitz–Riley magnitude, windowed, centred,
    /// cut into partitions and transformed.
    fn design(&mut self, c: usize, hz: f32, slope: Slope) {
        let n = self.taps;
        let coefs = XoverCoefs::new(slope, hz, self.sr);
        for k in 0..=n / 2 {
            let f = k as f32 * self.sr / n as f32;
            let mag = xover_response(&coefs, f.max(0.1), self.sr).0.abs() as f32;
            self.design_re[k] = mag;
            self.design_im[k] = 0.0;
            if k > 0 && k < n / 2 {
                self.design_re[n - k] = mag;
                self.design_im[n - k] = 0.0;
            }
        }
        self.big.run(&mut self.design_re, &mut self.design_im, true);
        // Centre on taps/2 and window: the impulse was circular around 0.
        let b = self.block;
        let bins = b + 1;
        for p in 0..PARTS {
            self.re.fill(0.0);
            self.im.fill(0.0);
            for i in 0..b {
                let t = p * b + i;
                let src = (t + n - n / 2) % n;
                self.re[i] = self.design_re[src] * self.window[t];
            }
            self.small.run(&mut self.re, &mut self.im, false);
            let [r, im] = &mut self.next[c];
            r[p * bins..(p + 1) * bins].copy_from_slice(&self.re[..bins]);
            im[p * bins..(p + 1) * bins].copy_from_slice(&self.im[..bins]);
        }
    }
}

impl Linear {
    /// One frame. The left and right (already late by the lookahead) go in; out comes the
    /// same input late by [`Self::latency`] — the telescoped sum's top term — and
    /// [`Self::low`] then reads each crossover's low-pass for this frame.
    #[inline]
    pub fn frame(&mut self, l: f32, r: f32, crossovers: usize) -> (f32, f32) {
        let b = self.block;
        self.frame[0][b + self.pos] = (l + r) * 0.5;
        self.frame[1][b + self.pos] = (l - r) * 0.5;
        let at = self.delay_at;
        let late = (self.delay[0][at], self.delay[1][at]);
        self.delay[0][at] = l;
        self.delay[1][at] = r;
        self.delay_at = (at + 1) % self.delay[0].len();
        for c in 0..crossovers.min(MAX_XOVERS) {
            self.out_last[c] = [self.out[c][0][self.pos], self.out[c][1][self.pos]];
        }
        self.pos += 1;
        if self.pos == b {
            // A block is in: the next block of output comes from it and the one before.
            self.compute(crossovers);
            self.pos = 0;
            for f in &mut self.frame {
                f.copy_within(b.., 0);
            }
        }
        late
    }

    /// Crossover `c`'s low-pass of the mid (`ch` 0) or the side (1), for the frame just
    /// passed to [`Self::frame`].
    #[inline]
    pub fn low(&self, c: usize, ch: usize) -> f32 {
        self.out_last[c][ch]
    }

    /// The next block of every crossover's output, from the two blocks of input collected.
    fn compute(&mut self, crossovers: usize) {
        let b = self.block;
        let bins = b + 1;
        let at = self.fdl_at;
        for ch in 0..2 {
            self.re.copy_from_slice(&self.frame[ch]);
            self.im.fill(0.0);
            self.small.run(&mut self.re, &mut self.im, false);
            self.fdl[ch][0][at * bins..(at + 1) * bins].copy_from_slice(&self.re[..bins]);
            self.fdl[ch][1][at * bins..(at + 1) * bins].copy_from_slice(&self.im[..bins]);
        }
        for c in 0..crossovers.min(MAX_XOVERS) {
            let fading = self.changing[c];
            for ch in 0..2 {
                Self::mac(&self.fdl[ch], &self.next[c], at, bins, &mut self.acc);
                if fading {
                    Self::mac(&self.fdl[ch], &self.kern[c], at, bins, &mut self.acc_old);
                }
                // `next` is the kernel in force too, unless a new one is fading in.
                Self::inverse(&self.small, &self.acc, &mut self.re, &mut self.im, b);
                self.out[c][ch].copy_from_slice(&self.re[b..]);
                if fading {
                    Self::inverse(&self.small, &self.acc_old, &mut self.re, &mut self.im, b);
                    for (i, o) in self.out[c][ch].iter_mut().enumerate() {
                        let a = (i + 1) as f32 / b as f32;
                        *o = self.re[b + i] + (*o - self.re[b + i]) * a;
                    }
                }
            }
            if fading {
                let (k, n) = (&mut self.kern[c], &self.next[c]);
                for side in 0..2 {
                    k[side].copy_from_slice(&n[side]);
                }
                self.changing[c] = false;
            }
        }
        self.fdl_at = (at + 1) % PARTS;
    }

    /// Σ over partitions of input spectrum × kernel spectrum, into `acc` (re, im).
    #[inline]
    fn mac(
        fdl: &[Vec<f32>; 2],
        kern: &[Vec<f32>; 2],
        at: usize,
        bins: usize,
        acc: &mut [Vec<f32>; 2],
    ) {
        acc[0].fill(0.0);
        acc[1].fill(0.0);
        for p in 0..PARTS {
            let x = ((at + PARTS - p) % PARTS) * bins;
            let h = p * bins;
            let (xr, xi) = (&fdl[0][x..x + bins], &fdl[1][x..x + bins]);
            let (hr, hi) = (&kern[0][h..h + bins], &kern[1][h..h + bins]);
            for k in 0..bins {
                acc[0][k] += xr[k] * hr[k] - xi[k] * hi[k];
                acc[1][k] += xr[k] * hi[k] + xi[k] * hr[k];
            }
        }
    }

    /// Rebuild the full (Hermitian) spectrum from `bins` and transform back.
    fn inverse(fft: &Fft, acc: &[Vec<f32>; 2], re: &mut [f32], im: &mut [f32], b: usize) {
        let n = 2 * b;
        re[..=b].copy_from_slice(&acc[0]);
        im[..=b].copy_from_slice(&acc[1]);
        for k in 1..b {
            re[n - k] = acc[0][k];
            im[n - k] = -acc[1][k];
        }
        fft.run(re, im, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fft_round_trips_and_matches_a_dft() {
        let n = 64;
        let fft = Fft::new(n);
        let x: Vec<f32> = (0..n).map(|i| ((i * 37) % 11) as f32 - 5.0).collect();
        let (mut re, mut im) = (x.clone(), vec![0.0; n]);
        fft.run(&mut re, &mut im, false);
        for k in [0usize, 1, 7, 31] {
            let (mut dr, mut di) = (0.0f64, 0.0f64);
            for (t, v) in x.iter().enumerate() {
                let a = -std::f64::consts::TAU * (k * t) as f64 / n as f64;
                dr += *v as f64 * math::cos(a);
                di += *v as f64 * math::sin(a);
            }
            assert!((re[k] as f64 - dr).abs() < 1e-3 && (im[k] as f64 - di).abs() < 1e-3);
        }
        fft.run(&mut re, &mut im, true);
        for (a, b) in re.iter().zip(&x) {
            assert!((a - b).abs() < 1e-5);
        }
    }

    /// A crossover's low-pass is linear phase (its impulse response is symmetric about the
    /// latency) and −6 dB at its frequency, as a Linkwitz–Riley half is.
    #[test]
    fn a_low_pass_is_symmetric_and_meets_the_crossover_at_minus_6() {
        let sr = 48_000.0;
        let mut lin = Linear::new(sr);
        lin.want(0, 1_000.0, Slope::S24);
        let n = lin.latency() * 3;
        let mut h = Vec::with_capacity(n);
        for i in 0..n {
            let x = if i == 0 { 1.0 } else { 0.0 };
            lin.frame(x, x, 1);
            h.push(lin.low(0, 0));
        }
        let d = lin.latency();
        let peak = h.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        for m in 1..lin.taps() / 2 - 1 {
            assert!(
                (h[d + m] - h[d - m]).abs() <= peak * 1e-5,
                "asymmetric at {m}: {} vs {}",
                h[d + m],
                h[d - m]
            );
        }
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (t, v) in h.iter().enumerate() {
            let a = -std::f64::consts::TAU * 1_000.0 * t as f64 / sr as f64;
            re += *v as f64 * math::cos(a);
            im += *v as f64 * math::sin(a);
        }
        let db = 10.0 * math::log10(re * re + im * im);
        assert!((db + 6.02).abs() < 0.05, "{db} dB at the crossover");
    }

    /// How well 4096 taps hold the Linkwitz–Riley shape as the crossover goes down: the
    /// level at the crossover frequency (−6.02 dB is exact). Measured: −6.02 dB from 60 Hz
    /// up, −5.98 at 40 Hz (printed with `MB_REPORT`).
    #[test]
    fn low_crossovers_keep_their_shape_down_to_40_hz() {
        let sr = 48_000.0;
        for hz in [40.0f32, 60.0, 100.0, 200.0, 5_000.0, 15_000.0] {
            let mut lin = Linear::new(sr);
            lin.want(0, hz, Slope::S24);
            let n = lin.latency() * 3;
            let mut h = Vec::with_capacity(n);
            for i in 0..n {
                let x = if i == 0 { 1.0 } else { 0.0 };
                lin.frame(x, x, 1);
                h.push(lin.low(0, 0));
            }
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (t, v) in h.iter().enumerate() {
                let a = -std::f64::consts::TAU * hz as f64 * t as f64 / sr as f64;
                re += *v as f64 * math::cos(a);
                im += *v as f64 * math::sin(a);
            }
            let db = 10.0 * math::log10(re * re + im * im);
            if std::env::var("MB_REPORT").is_ok() {
                eprintln!("linear LR4 at {hz} Hz: {db:.3} dB at the crossover");
            }
            let tol = if hz >= 60.0 { 0.01 } else { 0.1 };
            assert!((db + 6.02).abs() < tol, "{hz} Hz: {db} dB");
        }
    }
}
