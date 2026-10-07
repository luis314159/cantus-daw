//! Aequalis's two phase modes, beside the zero-latency cascade in `aequalis.rs`.
//!
//! **Natural phase.** The zero-latency cascade matches the analog *magnitude*, but its phase
//! and its magnitude between the three matched points drift from the analog prototype near
//! Nyquist. Natural phase keeps the cascade and adds a short FIR **correction** after it:
//! `C(ω) = H_analog(ω) / H_digital(ω) · e^{−jωD}`, designed on a 512-point grid and cut to
//! [`NP_TAPS`] taps around a delay of [`NP_DELAY`] samples. Where the cascade is already
//! analog (everywhere but the top octave) the correction is a pure delay; near Nyquist it
//! restores the analog curve in magnitude *and* phase. The stereo image is handled as a
//! 2×2 system: bands on L, R, M or S compose as matrices, so the correction is exact for
//! any mix of placements.
//!
//! **Linear phase.** One FIR per needed channel pair, designed by frequency sampling from
//! the bands' analog magnitudes (zero phase), windowed (4-term Blackman–Harris) to `2L`
//! taps around a delay of `L`, and run by uniformly partitioned overlap-save convolution in
//! blocks of `B`. The latency is `L + B`, stated by [`LinearPhase::latency`]. Bands placed
//! on L/R and on M/S **at once** run as one 2×2 stage (four FIRs) rather than two stages
//! in series, so the latency does not double — the benchmark doubles it.
//!
//! Both engines are allocation-free after `new`, count time from the start of the stream
//! (never from the start of a block), and crossfade to a new design instead of switching,
//! so a dragged band does not click. A flat design is exactly the input delayed: a wire.

use super::fft::Fft;
use super::{Coefs, Placement, Proto, Resolution, MAX_SECTIONS};

/// Taps of the natural-phase correction.
pub const NP_TAPS: usize = 128;
/// Its delay, samples: the centre of the correction.
pub const NP_DELAY: usize = 64;
/// Samples a new correction takes to fade in; corrections are designed no more often.
pub const NP_FADE: usize = 256;
const NP_GRID: usize = 512;

/// A band as the phase modes see it: where it acts, and its sections — the analog
/// prototypes and the digital biquads the cascade runs.
#[derive(Copy, Clone, Debug)]
pub(crate) struct Stage {
    pub placement: Placement,
    /// For an auditioned band: the channel(s) the band is not on are silent.
    pub mute_other: bool,
    pub w0: f64,
    pub n: usize,
    pub protos: [Proto; MAX_SECTIONS],
    pub coefs: [Coefs; MAX_SECTIONS],
}

impl Stage {
    fn analog(&self, w: f64) -> C {
        let mut h = C::ONE;
        for p in &self.protos[..self.n] {
            let (re, im) = p.response(w / self.w0);
            h = h.mul(C { re, im });
        }
        h
    }

    fn digital(&self, w: f64) -> C {
        let mut h = C::ONE;
        for c in &self.coefs[..self.n] {
            let (re, im) = c.response(w);
            h = h.mul(C { re, im });
        }
        h
    }

    /// `|H_analog(ω)|`.
    fn analog_mag(&self, w: f64) -> f64 {
        let mut m2 = 1.0;
        for p in &self.protos[..self.n] {
            m2 *= p.mag2(w / self.w0);
        }
        m2.sqrt()
    }
}

/// How the two channels are filtered.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Topology {
    /// Nothing to do: the input, delayed.
    #[default]
    Identity,
    /// One filter on both channels (every band on both).
    Mono,
    /// A filter per channel (bands on L or R).
    Split,
    /// `[[P, Q], [Q, P]]` (bands on M or S).
    MidSide,
    /// Four filters: bands on L/R and on M/S together.
    Full,
}

/// Which topology a set of stages needs.
fn topology_of(stages: &[Stage]) -> Topology {
    if stages.is_empty() {
        return Topology::Identity;
    }
    let lr = stages
        .iter()
        .any(|s| matches!(s.placement, Placement::Left | Placement::Right));
    let ms = stages
        .iter()
        .any(|s| matches!(s.placement, Placement::Mid | Placement::Side));
    match (lr, ms) {
        (false, false) => Topology::Mono,
        (true, false) => Topology::Split,
        (false, true) => Topology::MidSide,
        (true, true) => Topology::Full,
    }
}

// ---- small complex algebra ------------------------------------------------------------

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct C {
    re: f64,
    im: f64,
}

impl C {
    const ZERO: C = C { re: 0.0, im: 0.0 };
    const ONE: C = C { re: 1.0, im: 0.0 };
    fn mul(self, o: C) -> C {
        C {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
    fn add(self, o: C) -> C {
        C {
            re: self.re + o.re,
            im: self.im + o.im,
        }
    }
    fn sub(self, o: C) -> C {
        C {
            re: self.re - o.re,
            im: self.im - o.im,
        }
    }
    fn scale(self, k: f64) -> C {
        C {
            re: self.re * k,
            im: self.im * k,
        }
    }
    fn conj(self) -> C {
        C {
            re: self.re,
            im: -self.im,
        }
    }
    fn norm2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

/// A 2×2 complex matrix acting on (L, R).
#[derive(Copy, Clone, Debug)]
struct M2 {
    a: C,
    b: C,
    c: C,
    d: C,
}

impl M2 {
    const IDENT: M2 = M2 {
        a: C::ONE,
        b: C::ZERO,
        c: C::ZERO,
        d: C::ONE,
    };

    /// `self · o`.
    fn mul(self, o: M2) -> M2 {
        M2 {
            a: self.a.mul(o.a).add(self.b.mul(o.c)),
            b: self.a.mul(o.b).add(self.b.mul(o.d)),
            c: self.c.mul(o.a).add(self.d.mul(o.c)),
            d: self.c.mul(o.b).add(self.d.mul(o.d)),
        }
    }

    /// A band with response `h` placed on part of the image; `mute` silences the rest.
    fn place(h: C, placement: Placement, mute: bool) -> M2 {
        let o = if mute { C::ZERO } else { C::ONE };
        let diag = |x: C, y: C| M2 {
            a: x,
            b: C::ZERO,
            c: C::ZERO,
            d: y,
        };
        // T⁻¹·diag(x, y)·T with M = (L+R)/2, S = (L−R)/2, L = M+S, R = M−S.
        let ms = |x: C, y: C| {
            let p = x.add(y).scale(0.5);
            let q = x.sub(y).scale(0.5);
            M2 {
                a: p,
                b: q,
                c: q,
                d: p,
            }
        };
        match placement {
            Placement::Stereo => diag(h, h),
            Placement::Left => diag(h, o),
            Placement::Right => diag(o, h),
            Placement::Mid => ms(h, o),
            Placement::Side => ms(o, h),
        }
    }

    /// `self · other⁻¹` for a system of topology `topo`, regularised where `other` is
    /// (nearly) singular — at DC under a low cut, in a notch's null, on the silent side of
    /// an audition — so the correction there tends to a plain delay (identity) instead of
    /// to zero, and the FIR stays smooth around it. Where the signal is that far down the
    /// correction does not matter; where it is not, the division is exact.
    fn div(self, other: M2, topo: Topology) -> M2 {
        const EPS: f64 = 1e-12;
        let reg = |a: C, d: C| -> C {
            a.mul(d.conj())
                .add(C { re: EPS, im: 0.0 })
                .scale(1.0 / (d.norm2() + EPS))
        };
        let diag = |x: C, y: C| M2 {
            a: x,
            b: C::ZERO,
            c: C::ZERO,
            d: y,
        };
        match topo {
            Topology::Identity => M2::IDENT,
            Topology::Mono => {
                let c = reg(self.a, other.a);
                diag(c, c)
            }
            Topology::Split => diag(reg(self.a, other.a), reg(self.d, other.d)),
            Topology::MidSide => {
                // Diagonal in the M/S basis: M = p + q, S = p − q.
                let cm = reg(self.a.add(self.b), other.a.add(other.b));
                let cs = reg(self.a.sub(self.b), other.a.sub(other.b));
                let p = cm.add(cs).scale(0.5);
                let q = cm.sub(cs).scale(0.5);
                M2 {
                    a: p,
                    b: q,
                    c: q,
                    d: p,
                }
            }
            Topology::Full => {
                let det = other.a.mul(other.d).sub(other.b.mul(other.c));
                let den = 1.0 / (det.norm2() + EPS * EPS);
                let k = det.conj().scale(den);
                let inv = M2 {
                    a: other.d.mul(k),
                    b: other.b.scale(-1.0).mul(k),
                    c: other.c.scale(-1.0).mul(k),
                    d: other.a.mul(k),
                };
                let mut c = self.mul(inv);
                let blend = EPS * EPS * den;
                c.a = c.a.add(C { re: blend, im: 0.0 });
                c.d = c.d.add(C { re: blend, im: 0.0 });
                c
            }
        }
    }
}

/// Dot product of equal-length slices, in four lanes so it vectorises.
#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    let mut acc = [0.0f32; 4];
    let (ca, ra) = a.split_at(a.len() / 4 * 4);
    let (cb, rb) = b.split_at(ca.len());
    for (x, y) in ca.chunks_exact(4).zip(cb.chunks_exact(4)) {
        acc[0] += x[0] * y[0];
        acc[1] += x[1] * y[1];
        acc[2] += x[2] * y[2];
        acc[3] += x[3] * y[3];
    }
    let mut s = (acc[0] + acc[1]) + (acc[2] + acc[3]);
    for (x, y) in ra.iter().zip(rb) {
        s += x * y;
    }
    s
}

// ---- natural phase --------------------------------------------------------------------

/// A correction: taps stored reversed (oldest sample first), per filter a, b, c, d.
#[derive(Clone)]
struct FirSet {
    topo: Topology,
    taps: [[f32; NP_TAPS]; 4],
}

impl Default for FirSet {
    fn default() -> Self {
        FirSet {
            topo: Topology::Identity,
            taps: [[0.0; NP_TAPS]; 4],
        }
    }
}

/// The natural-phase correction after the cascade.
#[derive(Clone)]
pub struct NaturalPhase {
    fft: Fft,
    sets: Box<[FirSet; 2]>,
    cur: usize,
    /// Samples into a crossfade from `cur` to the other set; 0 when not fading.
    fade: usize,
    primed: bool,
    /// Double-length rings, so the last `NP_TAPS` samples are one contiguous slice.
    hist: Box<[[f32; 2 * NP_TAPS]; 2]>,
    pos: usize,
    spec: Vec<M2>,
    re: Vec<f32>,
    im: Vec<f32>,
    window: [f32; NP_TAPS],
}

impl std::fmt::Debug for NaturalPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NaturalPhase")
            .field("topology", &self.sets[self.cur].topo)
            .field("fade", &self.fade)
            .finish()
    }
}

impl Default for NaturalPhase {
    fn default() -> Self {
        Self::new()
    }
}

impl NaturalPhase {
    pub fn new() -> Self {
        // Tukey: flat over the middle half, cosine tapers to the ends, so the taps near the
        // centre — where the correction lives — are kept as designed.
        let mut window = [0.0f32; NP_TAPS];
        for (m, w) in window.iter_mut().enumerate() {
            let d = (m as f64 - NP_DELAY as f64).abs() / NP_DELAY as f64;
            *w = if d <= 0.5 {
                1.0
            } else {
                (0.5 + 0.5 * (std::f64::consts::PI * (d - 0.5) / 0.5).cos()) as f32
            };
        }
        NaturalPhase {
            fft: Fft::new(NP_GRID),
            sets: Box::default(),
            cur: 0,
            fade: 0,
            primed: false,
            hist: Box::new([[0.0; 2 * NP_TAPS]; 2]),
            pos: 0,
            spec: vec![M2::IDENT; NP_GRID / 2 + 1],
            re: vec![0.0; NP_GRID],
            im: vec![0.0; NP_GRID],
            window,
        }
    }

    pub fn reset(&mut self) {
        *self.hist = [[0.0; 2 * NP_TAPS]; 2];
        self.pos = 0;
        if self.fade > 0 {
            self.cur = 1 - self.cur;
            self.fade = 0;
        }
    }

    pub fn topology(&self) -> Topology {
        self.sets[self.cur].topo
    }

    /// No crossfade under way and the history all zeros (ADR 0031 §5.1).
    pub(crate) fn is_clear(&self) -> bool {
        self.fade == 0 && self.hist.iter().all(|h| crate::rest::all_zero(h))
    }

    /// `frames` of silence not processed: a crossfade under way goes on (between two
    /// corrections of silence, which are silence).
    pub(crate) fn skip(&mut self, frames: usize) {
        if self.fade > 0 {
            self.fade += frames;
            if self.fade > NP_FADE {
                self.fade = 0;
                self.cur = 1 - self.cur;
            }
        }
    }

    /// Design the correction for `stages` (in the cascade's order). The first design is
    /// taken at once; later ones fade in over [`NP_FADE`] samples.
    pub(crate) fn design(&mut self, stages: &[Stage]) {
        if self.fade > 0 {
            // A fade still running lands at once: the next design needs its slot.
            self.cur = 1 - self.cur;
            self.fade = 0;
        }
        let target = if self.primed { 1 - self.cur } else { self.cur };
        let topo = topology_of(stages);
        self.sets[target].topo = topo;
        if topo != Topology::Identity {
            let delay = NP_DELAY as f64;
            for (k, m) in self.spec.iter_mut().enumerate() {
                let w = 2.0 * std::f64::consts::PI * k as f64 / NP_GRID as f64;
                let mut ha = M2::IDENT;
                let mut hd = M2::IDENT;
                for s in stages {
                    ha = M2::place(s.analog(w), s.placement, s.mute_other).mul(ha);
                    hd = M2::place(s.digital(w), s.placement, s.mute_other).mul(hd);
                }
                let (sin, cos) = (-w * delay).sin_cos();
                let z = C { re: cos, im: sin };
                let mut c = ha.div(hd, topo);
                c.a = c.a.mul(z);
                c.b = c.b.mul(z);
                c.c = c.c.mul(z);
                c.d = c.d.mul(z);
                *m = c;
            }
            // Filters by topology: which entries are distinct.
            let pairs: [(usize, usize); 2] = [(0, 1), (2, 3)];
            let count = if topo == Topology::Full { 2 } else { 1 };
            for &(f1, f2) in pairs.iter().take(count) {
                let entry = |m: &M2, f: usize| -> C {
                    match f {
                        0 => m.a,
                        1 => match topo {
                            // Split keeps L in a and R in d; pack d as the second filter.
                            Topology::Split => m.d,
                            _ => m.b,
                        },
                        2 => m.c,
                        _ => m.d,
                    }
                };
                let n = NP_GRID;
                for k in 0..=n / 2 {
                    let x = entry(&self.spec[k], f1);
                    let y = entry(&self.spec[k], f2);
                    // Z = X + jY for two real impulse responses; X and Y are Hermitian.
                    let (xr, xi, yr, yi) = if k == 0 || k == n / 2 {
                        (x.re, 0.0, y.re, 0.0)
                    } else {
                        (x.re, x.im, y.re, y.im)
                    };
                    self.re[k] = (xr - yi) as f32;
                    self.im[k] = (xi + yr) as f32;
                    if k != 0 && k != n / 2 {
                        self.re[n - k] = (xr + yi) as f32;
                        self.im[n - k] = (yr - xi) as f32;
                    }
                }
                self.fft.run(&mut self.re, &mut self.im, true);
                let scale = 1.0 / n as f32;
                let (a, b) = if topo == Topology::Split {
                    (0, 3)
                } else {
                    (f1, f2)
                };
                for m in 0..NP_TAPS {
                    let w = self.window[m] * scale;
                    // Reversed: index 0 holds the tap for the oldest sample.
                    self.sets[target].taps[a][NP_TAPS - 1 - m] = self.re[m] * w;
                    self.sets[target].taps[b][NP_TAPS - 1 - m] = self.im[m] * w;
                }
            }
        }
        if self.primed {
            self.fade = 1;
        } else {
            self.primed = true;
        }
    }

    #[inline]
    fn run_set(set: &FirSet, hl: &[f32], hr: &[f32]) -> (f32, f32) {
        let t = &set.taps;
        match set.topo {
            Topology::Identity => (hl[NP_TAPS - 1 - NP_DELAY], hr[NP_TAPS - 1 - NP_DELAY]),
            Topology::Mono => (dot(&t[0], hl), dot(&t[0], hr)),
            Topology::Split => (dot(&t[0], hl), dot(&t[3], hr)),
            Topology::MidSide => (
                dot(&t[0], hl) + dot(&t[1], hr),
                dot(&t[1], hl) + dot(&t[0], hr),
            ),
            Topology::Full => (
                dot(&t[0], hl) + dot(&t[1], hr),
                dot(&t[2], hl) + dot(&t[3], hr),
            ),
        }
    }

    /// Correct a stereo block in place (delaying it by [`NP_DELAY`]).
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let p = self.pos;
            self.hist[0][p] = *l;
            self.hist[0][p + NP_TAPS] = *l;
            self.hist[1][p] = *r;
            self.hist[1][p + NP_TAPS] = *r;
            self.pos = (p + 1) % NP_TAPS;
            let hl = &self.hist[0][p + 1..p + 1 + NP_TAPS];
            let hr = &self.hist[1][p + 1..p + 1 + NP_TAPS];
            let (yl, yr) = Self::run_set(&self.sets[self.cur], hl, hr);
            if self.fade == 0 {
                *l = yl;
                *r = yr;
            } else {
                let (nl, nr) = Self::run_set(&self.sets[1 - self.cur], hl, hr);
                let w = self.fade as f32 / NP_FADE as f32;
                *l = yl + (nl - yl) * w;
                *r = yr + (nr - yr) * w;
                self.fade += 1;
                if self.fade > NP_FADE {
                    self.fade = 0;
                    self.cur = 1 - self.cur;
                }
            }
        }
    }
}

// ---- linear phase ---------------------------------------------------------------------

/// Samples per convolution block at 48 kHz (scaled with the rate like the FIR).
pub const LP_BLOCK: usize = 512;

/// The rate factor: FIR lengths and blocks scale with the sample rate, so a resolution is
/// the same number of milliseconds (and hertz of detail) at every rate.
pub fn rate_factor(sample_rate: f32) -> usize {
    if sample_rate < 64_000.0 {
        1
    } else if sample_rate < 128_000.0 {
        2
    } else {
        4
    }
}

/// Filter spectra: 4 filters × partitions × bins, split into real and imaginary parts.
#[derive(Clone)]
struct Spectra {
    re: [Vec<f32>; 4],
    im: [Vec<f32>; 4],
    topo: Topology,
}

/// The linear-phase convolver.
#[derive(Clone)]
pub struct LinearPhase {
    fft: Fft,
    /// Block size B.
    b: usize,
    bins: usize,
    /// Half the FIR length, L: the FIR's own delay.
    half: usize,
    parts: usize,
    parts_max: usize,
    factor: usize,
    fdl_re: [Vec<f32>; 2],
    fdl_im: [Vec<f32>; 2],
    head: usize,
    sets: Box<[Spectra; 2]>,
    cur: usize,
    fading: bool,
    primed: bool,
    pending: bool,
    since_design: usize,
    /// The previous and the current input block, per channel (2B).
    inbuf: [Vec<f32>; 2],
    /// The block being played out, per channel (B).
    outbuf: [Vec<f32>; 2],
    pos: usize,
    /// Frames of silence in a row (ADR 0031 §5.1).
    quiet: usize,
    /// The input, kept long enough to be read `L + B` late: the dry signal aligned with the
    /// output (for detectors), and the output itself when the design is flat.
    dry: [Vec<f32>; 2],
    dry_pos: usize,
    zr: Vec<f32>,
    zi: Vec<f32>,
    acc: [Vec<f32>; 4],
    old: [Vec<f32>; 2],
    dr: Vec<f32>,
    di: Vec<f32>,
    fir: [Vec<f32>; 2],
    window: Vec<f32>,
}

impl std::fmt::Debug for LinearPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinearPhase")
            .field("half", &self.half)
            .field("block", &self.b)
            .field("topology", &self.sets[self.cur].topo)
            .finish()
    }
}

impl LinearPhase {
    /// A convolver with room for `res` and every shorter resolution — and no more: its
    /// buffers are sized for that FIR, so a Medium EQ holds a quarter of what a Very High
    /// one does. Allocates: build it off the audio thread and hand it over
    /// (`Aequalis::supply_linear`).
    pub fn new(sample_rate: f32, res: Resolution) -> Self {
        let factor = rate_factor(sample_rate);
        let b = LP_BLOCK * factor;
        let half_max = res.half() * factor;
        let parts_max = 2 * half_max / b;
        let bins = b + 1;
        let spectra = || Spectra {
            re: std::array::from_fn(|_| vec![0.0; parts_max * bins]),
            im: std::array::from_fn(|_| vec![0.0; parts_max * bins]),
            topo: Topology::Identity,
        };
        let design_max = 4 * half_max;
        let mut lp = LinearPhase {
            fft: Fft::new(design_max.max(2 * b)),
            b,
            bins,
            half: 0,
            parts: 0,
            parts_max,
            factor,
            fdl_re: std::array::from_fn(|_| vec![0.0; parts_max * bins]),
            fdl_im: std::array::from_fn(|_| vec![0.0; parts_max * bins]),
            head: 0,
            sets: Box::new([spectra(), spectra()]),
            cur: 0,
            fading: false,
            primed: false,
            pending: true,
            since_design: 0,
            inbuf: std::array::from_fn(|_| vec![0.0; 2 * b]),
            outbuf: std::array::from_fn(|_| vec![0.0; b]),
            pos: 0,
            quiet: usize::MAX,
            dry: std::array::from_fn(|_| vec![0.0; half_max + 2 * b + 1]),
            dry_pos: 0,
            zr: vec![0.0; 2 * b],
            zi: vec![0.0; 2 * b],
            acc: std::array::from_fn(|_| vec![0.0; bins]),
            old: std::array::from_fn(|_| vec![0.0; b]),
            dr: vec![0.0; design_max],
            di: vec![0.0; design_max],
            fir: std::array::from_fn(|_| vec![0.0; 2 * half_max]),
            window: vec![0.0; 2 * half_max],
        };
        lp.set_resolution(res);
        lp
    }

    /// The FIR's half length for a resolution at this rate.
    pub fn half_for(&self, res: Resolution) -> usize {
        res.half() * self.factor
    }

    /// The longest FIR half the buffers have room for, samples.
    pub fn capacity(&self) -> usize {
        self.parts_max * self.b / 2
    }

    /// Whether the buffers have room for `res`.
    pub fn fits(&self, res: Resolution) -> bool {
        self.half_for(res) <= self.capacity()
    }

    /// Choose the FIR length. A change starts the convolver clean (its latency changes).
    /// A resolution the buffers have no room for is refused (`false`), and nothing changes.
    pub fn set_resolution(&mut self, res: Resolution) -> bool {
        let half = self.half_for(res);
        if !self.fits(res) {
            return false;
        }
        if half == self.half {
            return true;
        }
        self.half = half;
        self.parts = 2 * half / self.b;
        let n = 2 * half;
        // Periodic 4-term Blackman–Harris over the FIR, centred on L.
        let (a0, a1, a2, a3) = (0.35875, 0.48829, 0.14128, 0.01168);
        for m in 0..n {
            let x = 2.0 * std::f64::consts::PI * m as f64 / n as f64;
            self.window[m] =
                (a0 - a1 * x.cos() + a2 * (2.0 * x).cos() - a3 * (3.0 * x).cos()) as f32;
        }
        self.window[0] = 0.0;
        // A fresh convolver's buffers are zero already: leave their pages untouched until
        // linear phase is used.
        if self.primed || self.pos != 0 || self.head != 0 {
            self.reset();
        }
        self.primed = false;
        self.pending = true;
        true
    }

    /// Samples of delay: the FIR's centre plus one block of buffering.
    pub fn latency(&self) -> usize {
        self.half + self.b
    }

    pub fn topology(&self) -> Topology {
        self.sets[self.cur].topo
    }

    pub fn reset(&mut self) {
        for c in 0..2 {
            self.fdl_re[c].fill(0.0);
            self.fdl_im[c].fill(0.0);
            self.inbuf[c].fill(0.0);
            self.outbuf[c].fill(0.0);
            self.dry[c].fill(0.0);
        }
        self.head = 0;
        self.pos = 0;
        self.dry_pos = 0;
        self.quiet = usize::MAX;
        if self.fading {
            self.fading = false;
            self.cur = 1 - self.cur;
        }
    }

    /// At rest for an EQ whose clock reads `clock` (ADR 0031 §5.1): no design waiting or
    /// fading in, every buffer all zeros (enough silence has gone in to fill the two input
    /// blocks, every partition of the delay line, the block playing out and the dry ring),
    /// and its blocks falling where a new convolver's would — on the EQ's clock, since a
    /// partitioned convolution's rounding depends on where its blocks fall.
    pub(crate) fn at_rest(&self, clock: u64) -> bool {
        let settle = (self.parts_max + 5) * self.b + self.dry[0].len();
        !self.pending
            && !self.fading
            && self.quiet >= settle
            && self.pos as u64 == clock % self.b as u64
    }

    /// `frames` of silence not processed: the blocks they would have ended go by — a
    /// design waiting for one is made there, as processing would have made it — and the
    /// rings move on. Every block of silence convolved is silence.
    pub(crate) fn skip(&mut self, frames: usize, stages: &[Stage]) {
        let len = self.dry[0].len();
        self.dry_pos = (self.dry_pos + frames % len) % len;
        self.quiet = self.quiet.saturating_add(frames);
        let mut left = frames;
        while left > 0 {
            let to_end = self.b - self.pos;
            if left < to_end {
                self.pos += left;
                break;
            }
            left -= to_end;
            self.pos = 0;
            if self.pending && (!self.primed || self.since_design >= self.design_gap()) {
                let target = if self.primed { 1 - self.cur } else { self.cur };
                self.design(stages, target);
                self.fading = self.primed;
                self.primed = true;
                self.pending = false;
                self.since_design = 0;
            } else {
                self.since_design = self.since_design.saturating_add(1);
            }
            if self.fading {
                self.cur = 1 - self.cur;
                self.fading = false;
            }
            self.head = (self.head + 1) % self.parts_max;
        }
    }

    /// Ask for a new design at the next block the rate limit allows.
    pub fn request_design(&mut self) {
        self.pending = true;
    }

    /// Blocks to wait between designs: a design costs about the same as `N / 8192`
    /// blocks of convolution, so a long FIR is redesigned less often while a band moves.
    fn design_gap(&self) -> usize {
        (2 * self.half / (8192 * self.factor)).max(1)
    }

    /// Process a stereo block in place. `dry_l`/`dry_r` receive the input delayed by the
    /// latency — the signal the output lines up with.
    pub(crate) fn process(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        dry_l: &mut [f32],
        dry_r: &mut [f32],
        stages: &[Stage],
    ) {
        let len = self.dry[0].len();
        let lat = self.half + self.b;
        for i in 0..left.len() {
            let (xl, xr) = (left[i], right[i]);
            self.quiet = if crate::rest::z(xl) && crate::rest::z(xr) {
                self.quiet.saturating_add(1)
            } else {
                0
            };
            self.dry[0][self.dry_pos] = xl;
            self.dry[1][self.dry_pos] = xr;
            let dp = (self.dry_pos + len - lat) % len;
            dry_l[i] = self.dry[0][dp];
            dry_r[i] = self.dry[1][dp];
            self.dry_pos = (self.dry_pos + 1) % len;
            left[i] = self.outbuf[0][self.pos];
            right[i] = self.outbuf[1][self.pos];
            self.inbuf[0][self.b + self.pos] = xl;
            self.inbuf[1][self.b + self.pos] = xr;
            self.pos += 1;
            if self.pos == self.b {
                self.pos = 0;
                self.block(stages);
            }
        }
    }

    fn block(&mut self, stages: &[Stage]) {
        let b = self.b;
        if self.pending && (!self.primed || self.since_design >= self.design_gap()) {
            let target = if self.primed { 1 - self.cur } else { self.cur };
            self.design(stages, target);
            self.fading = self.primed;
            self.primed = true;
            self.pending = false;
            self.since_design = 0;
        } else {
            self.since_design = self.since_design.saturating_add(1);
        }
        // The input block's spectrum, both channels in one transform.
        self.zr.copy_from_slice(&self.inbuf[0]);
        self.zi.copy_from_slice(&self.inbuf[1]);
        self.fft.run(&mut self.zr, &mut self.zi, false);
        let n2 = 2 * b;
        let base = self.head * self.bins;
        for k in 0..=b {
            let k2 = (n2 - k) % n2;
            let (ar, ai, br, bi) = (self.zr[k], self.zi[k], self.zr[k2], self.zi[k2]);
            self.fdl_re[0][base + k] = 0.5 * (ar + br);
            self.fdl_im[0][base + k] = 0.5 * (ai - bi);
            self.fdl_re[1][base + k] = 0.5 * (ai + bi);
            self.fdl_im[1][base + k] = 0.5 * (br - ar);
        }
        // The new output block: the set in force, faded into the next one if a design
        // just landed.
        let cur = self.cur;
        self.convolve(cur);
        if self.fading {
            for c in 0..2 {
                let (src, dst) = (&self.outbuf[c], &mut self.old[c]);
                dst.copy_from_slice(src);
            }
            self.convolve(1 - cur);
            for c in 0..2 {
                for k in 0..b {
                    let w = (k + 1) as f32 / b as f32;
                    let o = self.old[c][k];
                    self.outbuf[c][k] = o + (self.outbuf[c][k] - o) * w;
                }
            }
            self.cur = 1 - cur;
            self.fading = false;
        }
        // Slide the input: this block becomes the previous one.
        for c in 0..2 {
            self.inbuf[c].copy_within(b..2 * b, 0);
        }
        self.head = (self.head + 1) % self.parts_max;
    }

    /// Fill `outbuf` with set `s` applied to the input history.
    fn convolve(&mut self, s: usize) {
        let b = self.b;
        let bins = self.bins;
        let topo = self.sets[s].topo;
        if topo == Topology::Identity {
            // The input `L` late, from the dry ring: exact, not an FFT's rounding.
            let len = self.dry[0].len();
            for c in 0..2 {
                for k in 0..b {
                    let idx = (self.dry_pos + 2 * len - b + k - self.half) % len;
                    self.outbuf[c][k] = self.dry[c][idx];
                }
            }
            return;
        }
        for a in self.acc.iter_mut() {
            a.fill(0.0);
        }
        let set = &self.sets[s];
        let [lr, li, rr, ri] = &mut self.acc;
        for p in 0..self.parts {
            let slot = (self.head + self.parts_max - p) % self.parts_max;
            let xo = slot * bins;
            let ho = p * bins;
            let (xlr, xli) = (
                &self.fdl_re[0][xo..xo + bins],
                &self.fdl_im[0][xo..xo + bins],
            );
            let (xrr, xri) = (
                &self.fdl_re[1][xo..xo + bins],
                &self.fdl_im[1][xo..xo + bins],
            );
            let h = |f: usize| (&set.re[f][ho..ho + bins], &set.im[f][ho..ho + bins]);
            // acc += h · x, complex.
            let mac = |ar: &mut [f32],
                       ai: &mut [f32],
                       (hr, hi): (&[f32], &[f32]),
                       xr: &[f32],
                       xi: &[f32]| {
                for k in 0..ar.len() {
                    ar[k] += hr[k] * xr[k] - hi[k] * xi[k];
                    ai[k] += hr[k] * xi[k] + hi[k] * xr[k];
                }
            };
            match topo {
                Topology::Identity => {}
                Topology::Mono => {
                    mac(lr, li, h(0), xlr, xli);
                    mac(rr, ri, h(0), xrr, xri);
                }
                Topology::Split => {
                    mac(lr, li, h(0), xlr, xli);
                    mac(rr, ri, h(3), xrr, xri);
                }
                Topology::MidSide => {
                    mac(lr, li, h(0), xlr, xli);
                    mac(lr, li, h(1), xrr, xri);
                    mac(rr, ri, h(1), xlr, xli);
                    mac(rr, ri, h(0), xrr, xri);
                }
                Topology::Full => {
                    mac(lr, li, h(0), xlr, xli);
                    mac(lr, li, h(1), xrr, xri);
                    mac(rr, ri, h(2), xlr, xli);
                    mac(rr, ri, h(3), xrr, xri);
                }
            }
        }
        // Back to time: Z = L + jR, both Hermitian.
        let n2 = 2 * b;
        for k in 0..=b {
            let (a_r, a_i, b_r, b_i) = (lr[k], li[k], rr[k], ri[k]);
            self.zr[k] = a_r - b_i;
            self.zi[k] = a_i + b_r;
            if k != 0 && k != b {
                self.zr[n2 - k] = a_r + b_i;
                self.zi[n2 - k] = b_r - a_i;
            }
        }
        self.fft.run(&mut self.zr, &mut self.zi, true);
        let scale = 1.0 / n2 as f32;
        for k in 0..b {
            self.outbuf[0][k] = self.zr[b + k] * scale;
            self.outbuf[1][k] = self.zi[b + k] * scale;
        }
    }

    /// Design set `s` from the stages' analog magnitudes.
    fn design(&mut self, stages: &[Stage], s: usize) {
        let topo = topology_of(stages);
        self.sets[s].topo = topo;
        if topo == Topology::Identity {
            return;
        }
        let half = self.half;
        let n = 2 * half;
        let d = 4 * half;
        let b = self.b;
        // Filters, as pairs designed together: (index, index).
        let pairs: &[(usize, usize)] = match topo {
            Topology::Identity => &[],
            Topology::Mono => &[(0, 0)],
            Topology::Split => &[(0, 3)],
            Topology::MidSide => &[(0, 1)],
            Topology::Full => &[(0, 1), (2, 3)],
        };
        for &(f1, f2) in pairs {
            for k in 0..=d / 2 {
                let w = 2.0 * std::f64::consts::PI * k as f64 / d as f64;
                let (mut st, mut gl, mut gr, mut gm, mut gs) = (1.0, 1.0, 1.0, 1.0, 1.0);
                for sg in stages {
                    let m = sg.analog_mag(w);
                    let o = if sg.mute_other { 0.0 } else { 1.0 };
                    match sg.placement {
                        Placement::Stereo => st *= m,
                        Placement::Left => {
                            gl *= m;
                            gr *= o;
                        }
                        Placement::Right => {
                            gr *= m;
                            gl *= o;
                        }
                        Placement::Mid => {
                            gm *= m;
                            gs *= o;
                        }
                        Placement::Side => {
                            gs *= m;
                            gm *= o;
                        }
                    }
                }
                let value = |f: usize| -> f64 {
                    match topo {
                        Topology::Mono => st * gl * gm,
                        Topology::Split => match f {
                            0 => st * gl,
                            _ => st * gr,
                        },
                        Topology::MidSide => match f {
                            0 => st * 0.5 * (gm + gs),
                            _ => st * 0.5 * (gm - gs),
                        },
                        _ => match f {
                            0 => 0.5 * st * gl * (gm + gs),
                            1 => 0.5 * st * gr * (gm - gs),
                            2 => 0.5 * st * gl * (gm - gs),
                            _ => 0.5 * st * gr * (gm + gs),
                        },
                    }
                };
                let x = value(f1) as f32;
                let y = if f1 == f2 { 0.0 } else { value(f2) as f32 };
                self.dr[k] = x;
                self.di[k] = y;
                if k != 0 && k != d / 2 {
                    self.dr[d - k] = x;
                    self.di[d - k] = y;
                }
            }
            // Zero phase in, two real even responses out.
            self.fft.run(&mut self.dr[..d], &mut self.di[..d], true);
            let scale = 1.0 / d as f32;
            for m in 0..n {
                let idx = (m + d - half) % d;
                let w = self.window[m] * scale;
                self.fir[0][m] = self.dr[idx] * w;
                self.fir[1][m] = self.di[idx] * w;
            }
            // Partition and transform, both filters in one FFT per partition.
            let bins = self.bins;
            for p in 0..self.parts {
                self.zr[..b].copy_from_slice(&self.fir[0][p * b..(p + 1) * b]);
                self.zi[..b].copy_from_slice(&self.fir[1][p * b..(p + 1) * b]);
                self.zr[b..].fill(0.0);
                self.zi[b..].fill(0.0);
                self.fft.run(&mut self.zr, &mut self.zi, false);
                let n2 = 2 * b;
                let o = p * bins;
                for k in 0..=b {
                    let k2 = (n2 - k) % n2;
                    let (ar, ai, br, bi) = (self.zr[k], self.zi[k], self.zr[k2], self.zi[k2]);
                    self.sets[s].re[f1][o + k] = 0.5 * (ar + br);
                    self.sets[s].im[f1][o + k] = 0.5 * (ai - bi);
                    if f2 != f1 {
                        self.sets[s].re[f2][o + k] = 0.5 * (ai + bi);
                        self.sets[s].im[f2][o + k] = 0.5 * (br - ar);
                    }
                }
            }
        }
    }
}
