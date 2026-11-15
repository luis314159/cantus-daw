//! Loquax's filter banks: band-passes of order 4, 6 or 8 at log-spaced centres.
//!
//! **The design.** Each band is a Butterworth band-pass of order `2M` (M = 2, 3, 4): the
//! low-pass prototype's `M` poles taken through the band-pass transform `s → (s² + 1)/(B s)`
//! give `M` pairs of poles, and each pair is one second-order section — a linear
//! zero-delay-feedback state-variable filter (the trapezoidal, "TPT" form of Zavalishin's
//! *The Art of VA Filter Design*, §4) read at its normalised band-pass tap. The edges are
//! pre-warped (`tan(π f / fs)`) and the centre is their geometric mean in the warped
//! domain, so the digital band is −3.01 dB exactly at its edges and 0 dB at its centre
//! (the sections' product is normalised there), whatever the sample rate.
//!
//! **The layout.** `n` centres from `lo` to `hi`, equally spaced in octaves; each band is
//! `width` spacings wide (1: neighbours cross at −3 dB, so the bank's summed power is flat
//! within a fraction of a decibel). The synthesis bank may be moved by a formant shift;
//! a band the shift pushes past 0.45 of the sample rate is muted.
//!
//! **The state** is kept as structure-of-arrays, `[section][band]`, so the inner loop over
//! the bands has no dependency between iterations and the compiler vectorises it. Every
//! state is flushed to +0.0 under 10⁻¹⁵ ([`crate::va::math::flush`]): after a silence the
//! bank reaches +0.0 exactly (a new bank's state) in a few seconds at most.

use crate::math;
use crate::va::math::flush;

/// The most bands a bank holds.
pub const MAX_BANDS: usize = 64;
/// The fewest bands.
pub const MIN_BANDS: usize = 8;
/// The most second-order sections in a band (order 8).
pub const MAX_SECTIONS: usize = 4;
/// The highest edge a band may reach, as a fraction of the sample rate.
pub const EDGE_MAX: f32 = 0.47;
/// The highest centre a synthesis band may have before it is muted.
pub const CENTRE_MAX: f32 = 0.45;
/// The widest relative bandwidth the transform is given (below 2: complex poles).
const B_MAX: f64 = 1.9;

/// A complex number, for the design only.
#[derive(Copy, Clone, Debug, PartialEq)]
struct C {
    re: f64,
    im: f64,
}

impl C {
    fn new(re: f64, im: f64) -> C {
        C { re, im }
    }
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
    fn mul(self, o: C) -> C {
        C::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    fn scale(self, k: f64) -> C {
        C::new(self.re * k, self.im * k)
    }
    fn abs(self) -> f64 {
        math::hypot(self.re, self.im)
    }
    fn div(self, o: C) -> C {
        let d = o.re * o.re + o.im * o.im;
        C::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
    /// The principal square root.
    fn sqrt(self) -> C {
        let m = self.abs();
        let re = ((m + self.re) * 0.5).max(0.0).sqrt();
        let im = ((m - self.re) * 0.5).max(0.0).sqrt();
        C::new(re, if self.im < 0.0 { -im } else { im })
    }
}

/// One second-order section's coefficients (the TPT state-variable filter's).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Section {
    /// `tan(π f / fs)` of the section's own frequency.
    pub g: f32,
    /// `1 / Q`.
    pub k: f32,
}

/// One band's design: its sections and the gain that makes its centre 0 dB.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct BandDesign {
    pub sections: [Section; MAX_SECTIONS],
    /// How many sections are used (M).
    pub count: usize,
    /// The product of the sections' normalised band-passes at the centre, inverted.
    pub norm: f32,
    /// The band's centre, Hz (the geometric mean of its edges, in the warped domain).
    pub centre: f32,
    /// False for a band past [`CENTRE_MAX`] (muted).
    pub live: bool,
}

/// The Butterworth band-pass of order `2·m` between `lo` and `hi` Hz at `sr`.
pub fn design(lo: f32, hi: f32, m: usize, sr: f32) -> BandDesign {
    let m = m.clamp(1, MAX_SECTIONS);
    let sr64 = f64::from(sr);
    let edge_max = f64::from(EDGE_MAX) * sr64;
    let hi = f64::from(hi).min(edge_max);
    let lo = f64::from(lo).clamp(1.0, hi * 0.999);
    let wl = math::tan(core::f64::consts::PI * lo / sr64);
    let wh = math::tan(core::f64::consts::PI * hi / sr64);
    let w0 = (wl * wh).sqrt();
    let b = ((wh - wl) / w0).min(B_MAX);
    let mut d = BandDesign {
        count: m,
        centre: (math::atan(w0) * sr64 / core::f64::consts::PI) as f32,
        live: true,
        ..BandDesign::default()
    };
    // The prototype's poles in the upper half-plane and on the real axis: for each, the
    // band-pass pole with the positive imaginary part is one section. A pole p and its
    // conjugate give the two sections S and S' of one pair; p real gives one section.
    let mut found = 0usize;
    let mut gain = C::new(1.0, 0.0);
    let centre = C::new(0.0, 1.0);
    for i in 0..m {
        let th = core::f64::consts::PI * (2 * i + m + 1) as f64 / (2 * m) as f64;
        let (s, c) = math::sin_cos(th);
        let p = C::new(c, s);
        if p.im < -1e-12 {
            continue; // the conjugate of one already taken
        }
        let pb = p.scale(b);
        let disc = pb.mul(pb).sub(C::new(4.0, 0.0)).sqrt();
        let roots = [pb.add(disc).scale(0.5), pb.sub(disc).scale(0.5)];
        let mut take = |r: C| {
            if found >= m {
                return;
            }
            let r = if r.im < 0.0 { C::new(r.re, -r.im) } else { r };
            let w = r.abs();
            let q = w / (-2.0 * r.re).max(1e-12);
            // The normalised band-pass (w/Q)·s / (s² + (w/Q)·s + w²) at the centre s = i.
            let wq = w / q;
            let num = centre.scale(wq);
            let den = centre.mul(centre).add(centre.scale(wq)).add(C::new(w * w, 0.0));
            gain = gain.mul(num.div(den));
            d.sections[found] = Section {
                g: (w0 * w) as f32,
                k: (1.0 / q) as f32,
            };
            found += 1;
        };
        if p.im.abs() <= 1e-12 {
            // A real prototype pole: one conjugate pair of band-pass poles.
            take(roots[0]);
        } else {
            take(roots[0]);
            take(roots[1]);
        }
    }
    d.count = found;
    d.norm = (1.0 / gain.abs().max(1e-30)) as f32;
    d
}

/// The bank's band edges: `n` centres from `lo` to `hi` Hz, each `width` spacings wide.
/// Returns (centre, low edge, high edge) for band `i`.
pub fn layout(i: usize, n: usize, lo: f32, hi: f32, width: f32) -> (f32, f32, f32) {
    let n = n.clamp(MIN_BANDS, MAX_BANDS);
    let (lo, hi) = (lo.max(10.0), hi.max(lo * 1.5));
    let span = math::log2(hi / lo);
    let step = span / (n - 1) as f32;
    let c = math::log2(lo) + step * i as f32;
    let half = 0.5 * step * width.clamp(0.25, 3.0);
    (math::exp2(c), math::exp2(c - half), math::exp2(c + half))
}

/// A bank of `n` bands × `M` sections, one channel.
#[derive(Clone, Debug)]
pub struct Bank {
    n: usize,
    m: usize,
    /// Per section and band: `g`, `k`, and the three solved coefficients.
    k: [[f32; MAX_BANDS]; MAX_SECTIONS],
    a1: [[f32; MAX_BANDS]; MAX_SECTIONS],
    a2: [[f32; MAX_BANDS]; MAX_SECTIONS],
    a3: [[f32; MAX_BANDS]; MAX_SECTIONS],
    norm: [f32; MAX_BANDS],
    ic1: [[f32; MAX_BANDS]; MAX_SECTIONS],
    ic2: [[f32; MAX_BANDS]; MAX_SECTIONS],
}

impl Default for Bank {
    fn default() -> Self {
        Bank {
            n: MIN_BANDS,
            m: 2,
            k: [[0.0; MAX_BANDS]; MAX_SECTIONS],
            a1: [[0.0; MAX_BANDS]; MAX_SECTIONS],
            a2: [[0.0; MAX_BANDS]; MAX_SECTIONS],
            a3: [[0.0; MAX_BANDS]; MAX_SECTIONS],
            norm: [0.0; MAX_BANDS],
            ic1: [[0.0; MAX_BANDS]; MAX_SECTIONS],
            ic2: [[0.0; MAX_BANDS]; MAX_SECTIONS],
        }
    }
}

impl Bank {
    /// The bands' designs in; the states stay (a retune glides, a new count clears).
    pub fn tune(&mut self, designs: &[BandDesign], m: usize) {
        let n = designs.len().min(MAX_BANDS);
        let m = m.clamp(1, MAX_SECTIONS);
        if n != self.n || m != self.m {
            self.reset();
        }
        self.n = n;
        self.m = m;
        for (b, d) in designs.iter().take(n).enumerate() {
            self.norm[b] = if d.live { d.norm } else { 0.0 };
            for s in 0..m {
                let sec = d.sections[s.min(d.count.saturating_sub(1))];
                let (g, k) = (sec.g, sec.k);
                let a1 = 1.0 / (1.0 + g * (g + k));
                self.k[s][b] = k;
                self.a1[s][b] = a1;
                self.a2[s][b] = g * a1;
                self.a3[s][b] = g * g * a1;
            }
        }
    }

    pub fn bands(&self) -> usize {
        self.n
    }

    pub fn reset(&mut self) {
        self.ic1 = [[0.0; MAX_BANDS]; MAX_SECTIONS];
        self.ic2 = [[0.0; MAX_BANDS]; MAX_SECTIONS];
    }

    /// Every state is +0.0, bit for bit.
    pub fn at_rest(&self) -> bool {
        let n = self.n;
        (0..self.m).all(|s| {
            crate::rest::all_zero(&self.ic1[s][..n]) && crate::rest::all_zero(&self.ic2[s][..n])
        })
    }

    /// One sample `x` through every band; band `b`'s output into `out[b]`.
    #[inline]
    pub fn run(&mut self, x: f32, out: &mut [f32; MAX_BANDS]) {
        let n = self.n;
        for v in out[..n].iter_mut() {
            *v = x;
        }
        for s in 0..self.m {
            let (k, a1, a2, a3) = (&self.k[s], &self.a1[s], &self.a2[s], &self.a3[s]);
            let (ic1, ic2) = (&mut self.ic1[s], &mut self.ic2[s]);
            for b in 0..n {
                let v0 = out[b];
                let v3 = v0 - ic2[b];
                let v1 = a1[b] * ic1[b] + a2[b] * v3;
                let v2 = ic2[b] + a2[b] * ic1[b] + a3[b] * v3;
                ic1[b] = flush(2.0 * v1 - ic1[b]);
                ic2[b] = flush(2.0 * v2 - ic2[b]);
                // The normalised band-pass tap: 0 dB at the section's own frequency.
                out[b] = k[b] * v1;
            }
        }
        for b in 0..n {
            out[b] *= self.norm[b];
        }
    }
}

/// A two-pole TPT state-variable filter (one channel), for the detector and the
/// sibilance path: low-, band- and high-pass taps.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}

/// Its coefficients.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SvfCoef {
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

impl SvfCoef {
    pub fn new(hz: f32, q: f32, sr: f32) -> SvfCoef {
        let g = crate::va::math::tan_pi((hz / sr).clamp(1e-5, 0.49));
        let k = 1.0 / q.max(0.05);
        let a1 = 1.0 / (1.0 + g * (g + k));
        SvfCoef {
            k,
            a1,
            a2: g * a1,
            a3: g * g * a1,
        }
    }
}

impl Svf {
    /// One sample: (low, band, high).
    #[inline]
    pub fn run(&mut self, x: f32, c: &SvfCoef) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = flush(2.0 * v1 - self.ic1);
        self.ic2 = flush(2.0 * v2 - self.ic2);
        (v2, v1, x - c.k * v1 - v2)
    }

    pub fn at_rest(&self) -> bool {
        self.ic1.to_bits() == 0 && self.ic2.to_bits() == 0
    }

    pub fn reset(&mut self) {
        *self = Svf::default();
    }
}
