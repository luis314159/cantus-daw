//! Fervor's oversampling: 2×, 4× or 8× as a cascade of half-band stages, with linear-phase
//! FIRs (a latency in whole samples, no phase shift) or polyphase IIR all-pass half-bands
//! (a sample or two of latency, a phase shift near the top of the band).
//!
//! ## Linear phase
//!
//! Each stage is a Kaiser-windowed half-band of `4k + 3` taps: every other tap is zero and
//! the centre is exactly ½, so one phase of the up-sampler is a **pure delay** (an input
//! sample, to the bit) and each stage costs about a quarter of a plain FIR. The first stage
//! (1×↔2×) carries the steep edge — flat to 20 kHz and more than 90 dB down from 28 kHz at
//! 48 kHz; the later ones only have to keep their images away from what the first passes, so
//! they are short. Each stage's up and down filters together delay by `2k + 1` samples at its
//! own rate; a few samples of padding at the top rate round the sum up to whole samples at
//! the base rate, so delay compensation lines the effect up exactly and the dry path is a
//! plain delay ([`latency`]).
//!
//! ## Minimum phase
//!
//! The IIR stages are the classic polyphase pair of first-order all-pass chains (designed
//! by the elliptic method of Valenzuela and Constantinides, as in de Soras's HIIR): no
//! pre-ringing and almost no latency, at the price of a phase shift that grows toward the
//! top of the band. The latency reported is their group delay at low frequencies, rounded,
//! so a dry/wet blend lines up where the energy of most music is.

use std::f64::consts::PI;

/// The factors, by the `os` parameter's index.
pub const FACTORS: [usize; 4] = [1, 2, 4, 8];
/// Stages at most (8× = three stages of 2×).
pub const MAX_STAGES: usize = 3;
/// The longest history a FIR stage keeps (taps of the first stage's even phase).
const MAX_HALF: usize = 48;

/// FIR stage lengths, first stage first: 4k + 3 each.
const FIR_TAPS: [usize; MAX_STAGES] = [95, 31, 15];
/// Kaiser β per stage.
const FIR_BETA: [f64; MAX_STAGES] = [9.0, 8.0, 8.0];
/// IIR stage sizes (all-pass coefficients) and transition bands.
const IIR_COEFS: [usize; MAX_STAGES] = [12, 6, 4];
const IIR_TBW: [f64; MAX_STAGES] = [0.0415, 0.15, 0.3];
const MAX_COEFS: usize = 12;

/// Modified Bessel function of the first kind, order 0 (for the Kaiser window).
fn bessel_i0(x: f64) -> f64 {
    let (mut sum, mut term) = (1.0f64, 1.0f64);
    let q = x * x / 4.0;
    for k in 1..100 {
        term *= q / (k * k) as f64;
        sum += term;
        if term < 1e-17 * sum {
            break;
        }
    }
    sum
}

/// The non-zero side taps of a half-band of `taps` (4k + 3) taps — its even-indexed
/// taps, `2k + 2` of them; the centre (odd index) is exactly ½ and every other odd tap 0.
/// Normalised so the DC gain is exactly one.
fn half_band(taps: usize, beta: f64, out: &mut [f64; MAX_HALF]) -> usize {
    let c = (taps - 1) as f64 / 2.0;
    let norm = bessel_i0(beta);
    let n_even = taps.div_ceil(2);
    let mut sum = 0.0;
    for (i, o) in out.iter_mut().enumerate().take(n_even) {
        let t = (2 * i) as f64 - c;
        let ideal = (PI * t / 2.0).sin() / (PI * t);
        let r = t / c;
        *o = ideal * bessel_i0(beta * (1.0 - r * r).max(0.0).sqrt()) / norm;
        sum += *o;
    }
    // The even taps share the half the centre does not carry.
    for o in out.iter_mut().take(n_even) {
        *o *= 0.5 / sum;
    }
    n_even
}

/// One linear-phase stage: an up-sampler and a down-sampler for one channel.
#[derive(Clone, Debug)]
struct FirStage {
    h: [f32; MAX_HALF],
    n: usize,
    /// k: the up-sampler's pure-delay phase is `k` input samples late.
    k: usize,
    /// Up: the input history (twice over, so a window is one slice).
    up: [f32; 2 * MAX_HALF],
    up_pos: usize,
    /// Down: the even samples' history and the odd samples' (the centre tap's).
    even: [f32; 2 * MAX_HALF],
    odd: [f32; 2 * MAX_HALF],
    down_pos: usize,
}

impl FirStage {
    fn new(stage: usize) -> FirStage {
        let mut h64 = [0.0; MAX_HALF];
        let n = half_band(FIR_TAPS[stage], FIR_BETA[stage], &mut h64);
        let mut h = [0.0f32; MAX_HALF];
        for (d, s) in h.iter_mut().zip(&h64) {
            *d = *s as f32;
        }
        FirStage {
            h,
            n,
            k: (FIR_TAPS[stage] - 3) / 4,
            up: [0.0; 2 * MAX_HALF],
            up_pos: 0,
            even: [0.0; 2 * MAX_HALF],
            odd: [0.0; 2 * MAX_HALF],
            down_pos: 0,
        }
    }

    fn is_clear(&self) -> bool {
        crate::rest::all_zero(&self.up)
            && crate::rest::all_zero(&self.even)
            && crate::rest::all_zero(&self.odd)
    }

    fn reset(&mut self) {
        self.up = [0.0; 2 * MAX_HALF];
        self.even = [0.0; 2 * MAX_HALF];
        self.odd = [0.0; 2 * MAX_HALF];
        self.up_pos = 0;
        self.down_pos = 0;
    }

    /// Push one sample into a doubled ring of length `n`; returns the window's start:
    /// `buf[start..start + n]` is the history, newest first.
    #[inline]
    fn push(buf: &mut [f32; 2 * MAX_HALF], pos: &mut usize, n: usize, x: f32) -> usize {
        *pos = if *pos == 0 { n - 1 } else { *pos - 1 };
        buf[*pos] = x;
        buf[*pos + n] = x;
        *pos
    }

    /// One input sample in, two out (gain 2, so the level is kept).
    #[inline]
    fn up(&mut self, x: f32) -> [f32; 2] {
        let n = self.n;
        let s = Self::push(&mut self.up, &mut self.up_pos, n, x);
        let w = &self.up[s..s + n];
        let mut acc = 0.0f32;
        for (a, b) in w.iter().zip(&self.h[..n]) {
            acc += a * b;
        }
        [2.0 * acc, w[self.k]]
    }

    /// Two input samples (earlier first) in, one out.
    #[inline]
    fn down(&mut self, v: [f32; 2]) -> f32 {
        let n = self.n;
        let s = Self::push(&mut self.even, &mut self.down_pos, n, v[0]);
        self.odd[s] = v[1];
        self.odd[s + n] = v[1];
        let w = &self.even[s..s + n];
        let mut acc = 0.0f32;
        for (a, b) in w.iter().zip(&self.h[..n]) {
            acc += a * b;
        }
        // The centre tap reads the odd sample k + 1 periods back.
        acc + 0.5 * self.odd[s + self.k + 1]
    }
}

// ---- minimum phase: polyphase IIR half-bands ------------------------------------------

/// The all-pass coefficients of a polyphase IIR half-band with `n` coefficients and a
/// transition band `tbw` wide (relative to the stage's high rate, centred on a quarter of
/// it): the elliptic design (Valenzuela & Constantinides; the formulas of HIIR).
fn iir_coefs(n: usize, tbw: f64, out: &mut [f64; MAX_COEFS]) {
    let mut k = ((1.0 - tbw * 2.0) * PI / 4.0).tan();
    k *= k;
    let kksqrt = (1.0 - k * k).powf(0.25);
    let e = 0.5 * (1.0 - kksqrt) / (1.0 + kksqrt);
    let e2 = e * e;
    let e4 = e2 * e2;
    let q = e * (1.0 + e4 * (2.0 + e4 * (15.0 + 150.0 * e4)));
    let order = (n * 2 + 1) as f64;
    for (index, o) in out.iter_mut().enumerate().take(n) {
        let c = (index + 1) as f64;
        let mut num = 0.0;
        let mut i = 0.0f64;
        let mut sign = 1.0;
        loop {
            let term = q.powf(i * (i + 1.0)) * ((i * 2.0 + 1.0) * c * PI / order).sin() * sign;
            num += term;
            sign = -sign;
            i += 1.0;
            if term.abs() <= 1e-100 || i > 200.0 {
                break;
            }
        }
        let mut den = 0.0;
        let mut i = 1.0f64;
        let mut sign = -1.0;
        loop {
            let term = q.powf(i * i) * (i * 2.0 * c * PI / order).cos() * sign;
            den += term;
            sign = -sign;
            i += 1.0;
            if term.abs() <= 1e-100 || i > 200.0 {
                break;
            }
        }
        let ww = num * q.powf(0.25) / (den + 0.5);
        let wwsq = ww * ww;
        let x = ((1.0 - wwsq * k) * (1.0 - wwsq / k)).sqrt() / (1.0 + wwsq);
        *o = (1.0 - x) / (1.0 + x);
    }
}

/// One path of first-order all-pass sections (every other coefficient).
#[derive(Clone, Copy, Debug, Default)]
struct AllPass {
    c: f32,
    x1: f32,
    y1: f32,
}

#[derive(Clone, Debug)]
struct IirStage {
    /// Sections; even indices are path 0, odd path 1.
    up: [AllPass; MAX_COEFS],
    down: [AllPass; MAX_COEFS],
    n: usize,
}

#[inline]
fn chain(s: &mut [AllPass], n: usize, a: &mut f32, b: &mut f32) {
    let mut i = 0;
    while i < n {
        let p = &mut s[i];
        let y = p.c * (*a - p.y1) + p.x1;
        p.x1 = *a;
        p.y1 = flush(y);
        *a = p.y1;
        if i + 1 < n {
            let p = &mut s[i + 1];
            let y = p.c * (*b - p.y1) + p.x1;
            p.x1 = *b;
            p.y1 = flush(y);
            *b = p.y1;
        }
        i += 2;
    }
}

#[inline]
fn flush(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}

impl IirStage {
    fn new(stage: usize) -> IirStage {
        let mut c = [0.0; MAX_COEFS];
        let n = IIR_COEFS[stage];
        iir_coefs(n, IIR_TBW[stage], &mut c);
        let mut up = [AllPass::default(); MAX_COEFS];
        for (s, c) in up.iter_mut().zip(&c).take(n) {
            s.c = *c as f32;
        }
        IirStage { up, down: up, n }
    }

    fn is_clear(&self) -> bool {
        self.up
            .iter()
            .chain(self.down.iter())
            .all(|s| s.x1 == 0.0 && s.y1 == 0.0)
    }

    fn reset(&mut self) {
        for s in self.up.iter_mut().chain(self.down.iter_mut()) {
            s.x1 = 0.0;
            s.y1 = 0.0;
        }
    }

    #[inline]
    fn up(&mut self, x: f32) -> [f32; 2] {
        let (mut a, mut b) = (x, x);
        chain(&mut self.up, self.n, &mut a, &mut b);
        [a, b]
    }

    #[inline]
    fn down(&mut self, v: [f32; 2]) -> f32 {
        let (mut a, mut b) = (v[1], v[0]);
        chain(&mut self.down, self.n, &mut a, &mut b);
        0.5 * (a + b)
    }

    /// Group delay at DC of the stage's up and down filters together, in samples of its
    /// high rate: each is ½·(2τ₀ + 1 + 2τ₁), τ the paths' delays at their own rate.
    fn dc_delay(&self) -> f64 {
        let mut tau = [0.0f64; 2];
        for (i, s) in self.up.iter().enumerate().take(self.n) {
            let c = s.c as f64;
            tau[i % 2] += (1.0 - c) / (1.0 + c);
        }
        2.0 * (2.0 * tau[0] + 1.0 + 2.0 * tau[1]) / 2.0
    }
}

// ---- the oversampler ------------------------------------------------------------------

/// The latency of the linear-phase cascade at `factor`, in base-rate samples: every
/// stage's up and down filters, plus the padding that makes the sum whole.
pub fn latency(factor: usize, minimum: bool) -> usize {
    let (stages, pad) = shape(factor);
    if stages == 0 {
        return 0;
    }
    if minimum {
        let mut d = 0.0;
        for s in 0..stages {
            // A stage's high rate is 2^(s+1) times the base rate.
            d += IirStage::new(s).dc_delay() / (1u32 << (s + 1)) as f64;
        }
        return d.round() as usize;
    }
    let top = 1usize << stages;
    let mut d = pad;
    for (s, taps) in FIR_TAPS.iter().enumerate().take(stages) {
        // 2c samples at the stage's rate, c = (taps − 1) / 2; in top-rate samples.
        d += (taps - 1) * (top >> (s + 1));
    }
    d / top
}

/// Stages and top-rate padding for a factor.
fn shape(factor: usize) -> (usize, usize) {
    let stages = match factor {
        2 => 1,
        4 => 2,
        8 => 3,
        _ => 0,
    };
    let top = 1usize << stages;
    let mut d = 0;
    for (s, taps) in FIR_TAPS.iter().enumerate().take(stages) {
        d += (taps - 1) * (top >> (s + 1));
    }
    (stages, (top - d % top) % top)
}

/// The most top-rate padding any factor needs.
const MAX_PAD: usize = 8;

/// One channel's oversampler: up to `factor` samples, and back down to one.
#[derive(Clone, Debug)]
pub struct Oversampler {
    factor: usize,
    minimum: bool,
    stages: usize,
    fir: [FirStage; MAX_STAGES],
    iir: [IirStage; MAX_STAGES],
    /// The padding delay at the top rate (linear phase).
    pad: usize,
    pad_buf: [f32; MAX_PAD],
    pad_pos: usize,
}

impl Default for Oversampler {
    fn default() -> Self {
        Oversampler::new(1, false)
    }
}

impl Oversampler {
    pub fn new(factor: usize, minimum: bool) -> Oversampler {
        let (stages, pad) = shape(factor);
        Oversampler {
            factor: 1 << stages,
            minimum,
            stages,
            fir: [FirStage::new(0), FirStage::new(1), FirStage::new(2)],
            iir: [IirStage::new(0), IirStage::new(1), IirStage::new(2)],
            pad: if minimum { 0 } else { pad },
            pad_buf: [0.0; MAX_PAD],
            pad_pos: 0,
        }
    }

    /// Change the factor or the filters (clears the state). Allocation-free.
    pub fn configure(&mut self, factor: usize, minimum: bool) {
        let (stages, pad) = shape(factor);
        if stages == self.stages && minimum == self.minimum {
            return;
        }
        self.stages = stages;
        self.factor = 1 << stages;
        self.minimum = minimum;
        self.pad = if minimum { 0 } else { pad };
        self.reset();
    }

    pub fn factor(&self) -> usize {
        self.factor
    }

    pub fn latency(&self) -> usize {
        latency(self.factor, self.minimum)
    }

    /// Every filter's history and the padding hold only zeros, as [`Self::reset`] leaves
    /// them (ADR 0031 §5.1): the ring positions do not matter then.
    pub fn is_clear(&self) -> bool {
        crate::rest::all_zero(&self.pad_buf)
            && self.iir.iter().all(IirStage::is_clear)
            && self.fir.iter().all(FirStage::is_clear)
    }

    pub fn reset(&mut self) {
        for s in &mut self.fir {
            s.reset();
        }
        for s in &mut self.iir {
            s.reset();
        }
        self.pad_buf = [0.0; MAX_PAD];
        self.pad_pos = 0;
    }

    /// One base-rate sample up to `factor` samples, into `out[..factor]`.
    #[inline]
    pub fn up(&mut self, x: f32, out: &mut [f32; 8]) {
        out[0] = x;
        let mut n = 1;
        for s in 0..self.stages {
            // In time order (the filters have state), from a copy of the stage's input.
            let input = *out;
            for (i, x) in input.iter().enumerate().take(n) {
                let pair = if self.minimum {
                    self.iir[s].up(*x)
                } else {
                    self.fir[s].up(*x)
                };
                out[2 * i] = pair[0];
                out[2 * i + 1] = pair[1];
            }
            n *= 2;
        }
    }

    /// `factor` top-rate samples (from [`Self::up`], after the curve) back to one.
    #[inline]
    pub fn down(&mut self, v: &mut [f32; 8]) -> f32 {
        let mut n = self.factor;
        if self.pad > 0 {
            for x in v.iter_mut().take(n) {
                let old = self.pad_buf[self.pad_pos];
                self.pad_buf[self.pad_pos] = *x;
                self.pad_pos = (self.pad_pos + 1) % self.pad;
                *x = old;
            }
        }
        for s in (0..self.stages).rev() {
            for i in 0..n / 2 {
                let pair = [v[2 * i], v[2 * i + 1]];
                v[i] = if self.minimum {
                    self.iir[s].down(pair)
                } else {
                    self.fir[s].down(pair)
                };
            }
            n /= 2;
        }
        v[0]
    }
}
