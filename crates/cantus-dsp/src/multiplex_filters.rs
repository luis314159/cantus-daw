//! Multiplex's filters: the trapezoidal state-variable filter everything is built from, the
//! Linkwitz–Riley crossovers of the split, the Butterworth shelves of the dynamic path, and
//! their exact frequency responses for the display and the tests.
//!
//! Every filter here is a TPT (trapezoidal, "zero-delay feedback") structure after Simper
//! and Zavalishin: it is the bilinear transform of its analog prototype, pre-warped at its
//! cutoff, so its digital response at `f` is the analog one at `Ω = tan(πf/fs) /
//! tan(πf_c/fs)` — exactly, which is how [`xover_response`] and [`shelf_response`] draw
//! what the audio thread runs. It also stays well-behaved while its cutoff moves every
//! sample, which the dynamic path relies on.
//!
//! ## Why the split sums flat
//!
//! A Linkwitz–Riley pair of order 2N is a Butterworth low-pass of order N squared, and the
//! matching high-pass. With `B_N` the Butterworth polynomial, `B_N(s) B_N(−s) = 1 +
//! (−s²)^N`, so
//!
//! ```text
//! LP + (−1)^N HP = (1 + (−1)^N s^2N) / B_N(s)² = B_N(−s) / B_N(s)
//! ```
//!
//! — an all-pass. For odd N (the 12 dB/oct pair) the high band is inverted to make it
//! so. A tree of such pairs, with the lower bands passed through the all-pass of every
//! crossover above them, therefore sums to a cascade of all-passes: flat magnitude, and
//! the test measures it. The 6 dB/oct "pair" is a one-pole low-pass and its complement
//! `x − LP`, which sums to the input itself.

use std::f32::consts::{FRAC_1_SQRT_2, PI};

/// Crossover slope. The split is a Linkwitz–Riley pair of the matching order (the 6 dB
/// one is complementary); the dynamic path's shelves are Butterworth of order slope / 6.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Slope {
    S6,
    S12,
    S24,
    S48,
}

impl Slope {
    pub const ALL: [Slope; 4] = [Slope::S6, Slope::S12, Slope::S24, Slope::S48];

    /// The nearest slope to a dB/octave value.
    pub fn from_db(db: f32) -> Slope {
        if db < 9.0 {
            Slope::S6
        } else if db < 18.0 {
            Slope::S12
        } else if db < 36.0 {
            Slope::S24
        } else {
            Slope::S48
        }
    }

    pub fn db(self) -> f32 {
        match self {
            Slope::S6 => 6.0,
            Slope::S12 => 12.0,
            Slope::S24 => 24.0,
            Slope::S48 => 48.0,
        }
    }

    /// The Butterworth sections of the split's half (`B_N`), as damping `k = 1/Q`: the
    /// Linkwitz–Riley filter is these twice. Empty for the first-order slopes.
    fn split_k(self) -> &'static [f32] {
        match self {
            Slope::S6 | Slope::S12 => &[],
            Slope::S24 => &[std::f32::consts::SQRT_2],
            // Butterworth 4: Q = 0.5412 and 1.3066.
            Slope::S48 => &[1.847_759, 0.765_366_9],
        }
    }

    /// The shelf's order: a 24 dB/oct slope is a fourth-order shelf.
    pub fn shelf_order(self) -> usize {
        (self.db() / 6.0) as usize
    }

    /// The shelf's second-order sections, as damping `k = 1/Q` (Butterworth of the
    /// shelf's order); a first-order shelf has none and uses a one-pole.
    fn shelf_k(self) -> &'static [f32] {
        match self {
            Slope::S6 => &[],
            Slope::S12 => &[std::f32::consts::SQRT_2],
            Slope::S24 => &[1.847_759, 0.765_366_9],
            // Butterworth 8: Q = 0.5098, 0.6013, 0.9000, 2.5629.
            Slope::S48 => &[1.961_571, 1.662_939_2, 1.111_140_5, 0.390_180_6],
        }
    }
}

/// `tan(π f / fs)`, the TPT filters' frequency coefficient, with `f` kept inside
/// (1 Hz, 0.49 fs) so it stays finite and positive.
#[inline]
pub fn warp(hz: f32, sample_rate: f32) -> f32 {
    let f = hz.clamp(1.0, sample_rate * 0.49);
    (PI * f / sample_rate).tan()
}

/// Coefficients of one state-variable section.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SvfCoefs {
    pub k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

impl SvfCoefs {
    #[inline]
    pub fn new(g: f32, k: f32) -> Self {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        SvfCoefs {
            k,
            a1,
            a2,
            a3: g * a2,
        }
    }
}

impl Default for SvfCoefs {
    fn default() -> Self {
        SvfCoefs::new(0.1, FRAC_1_SQRT_2 * 2.0)
    }
}

/// One channel of a state-variable section (Simper's trapezoidal SVF).
#[derive(Copy, Clone, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// Low-pass, band-pass and high-pass outputs. By construction `lp + k·bp + hp = x`.
    #[inline]
    pub fn tick(&mut self, c: &SvfCoefs, x: f32) -> (f32, f32, f32) {
        let v3 = x - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        if self.ic1.abs() < 1e-20 {
            self.ic1 = 0.0;
        }
        if self.ic2.abs() < 1e-20 {
            self.ic2 = 0.0;
        }
        (v2, v1, x - c.k * v1 - v2)
    }
}

/// One channel of a trapezoidal one-pole low-pass; `G = g / (1 + g)`.
#[derive(Copy, Clone, Debug, Default)]
pub struct OnePole {
    s: f32,
}

impl OnePole {
    #[inline]
    pub fn tick(&mut self, big_g: f32, x: f32) -> f32 {
        let v = (x - self.s) * big_g;
        let lp = v + self.s;
        self.s = lp + v;
        if self.s.abs() < 1e-20 {
            self.s = 0.0;
        }
        lp
    }
}

/// A crossover's coefficients, shared by both channels.
#[derive(Copy, Clone, Debug)]
pub struct XoverCoefs {
    pub slope: Slope,
    /// `tan(π f / fs)` at the crossover.
    pub g: f32,
    /// One per Butterworth section of the split's half.
    svf: [SvfCoefs; 2],
    /// The one-pole's `G`, for the first-order slopes.
    pole: f32,
}

impl XoverCoefs {
    pub fn new(slope: Slope, hz: f32, sample_rate: f32) -> Self {
        Self::from_g(slope, warp(hz, sample_rate))
    }

    pub fn from_g(slope: Slope, g: f32) -> Self {
        let mut svf = [SvfCoefs::default(); 2];
        for (c, k) in svf.iter_mut().zip(slope.split_k()) {
            *c = SvfCoefs::new(g, *k);
        }
        XoverCoefs {
            slope,
            g,
            svf,
            pole: g / (1.0 + g),
        }
    }
}

/// One channel of a crossover: the low and high halves, and the all-pass that the bands
/// below it are passed through so the sum stays flat.
#[derive(Copy, Clone, Debug, Default)]
pub struct Xover {
    /// The shared first section, then the rest of the low half and the rest of the high
    /// half (Linkwitz–Riley is the Butterworth half twice).
    first: [Svf; 2],
    low: [Svf; 3],
    high: [Svf; 3],
    ap: [Svf; 2],
    p_low: [OnePole; 2],
    p_high: [OnePole; 2],
    p_ap: OnePole,
}

impl Xover {
    /// Split one sample into (low, high). Low + high is the all-pass [`Self::allpass`]
    /// would give (for the 6 dB slope, the input itself).
    #[inline]
    pub fn split(&mut self, c: &XoverCoefs, x: f32) -> (f32, f32) {
        match c.slope {
            Slope::S6 => {
                let l = self.p_low[0].tick(c.pole, x);
                (l, x - l)
            }
            Slope::S12 => {
                let l1 = self.p_low[0].tick(c.pole, x);
                let l = self.p_low[1].tick(c.pole, l1);
                let h1 = x - self.p_high[0].tick(c.pole, x);
                let h = h1 - self.p_high[1].tick(c.pole, h1);
                // Odd Butterworth order: the high band is inverted so the pair sums to an
                // all-pass rather than a notch.
                (l, -h)
            }
            Slope::S24 => {
                let (l1, _, h1) = self.first[0].tick(&c.svf[0], x);
                let (l, _, _) = self.low[0].tick(&c.svf[0], l1);
                let (_, _, h) = self.high[0].tick(&c.svf[0], h1);
                (l, h)
            }
            Slope::S48 => {
                // Butterworth 4 = section a · section b; LR8 = that twice.
                let (la, _, ha) = self.first[0].tick(&c.svf[0], x);
                let (lb, _, _) = self.first[1].tick(&c.svf[1], la);
                let (_, _, hb) = self.high[2].tick(&c.svf[1], ha);
                let (lc, _, _) = self.low[0].tick(&c.svf[0], lb);
                let (l, _, _) = self.low[1].tick(&c.svf[1], lc);
                let (_, _, hc) = self.high[0].tick(&c.svf[0], hb);
                let (_, _, h) = self.high[1].tick(&c.svf[1], hc);
                (l, h)
            }
        }
    }

    /// The crossover's all-pass, on a signal that did not go through it.
    #[inline]
    pub fn allpass(&mut self, c: &XoverCoefs, x: f32) -> f32 {
        match c.slope {
            Slope::S6 => x,
            Slope::S12 => 2.0 * self.p_ap.tick(c.pole, x) - x,
            Slope::S24 => {
                let (_, bp, _) = self.ap[0].tick(&c.svf[0], x);
                x - 2.0 * c.svf[0].k * bp
            }
            Slope::S48 => {
                let (_, bp, _) = self.ap[0].tick(&c.svf[0], x);
                let y = x - 2.0 * c.svf[0].k * bp;
                let (_, bp, _) = self.ap[1].tick(&c.svf[1], y);
                y - 2.0 * c.svf[1].k * bp
            }
        }
    }
}

/// A shelf stage of the dynamic path: 1 below its crossover, `r` above it, Butterworth of
/// the slope's order, centred on the crossover in log frequency (its gain there is √r).
///
/// `H(s) = B_N(s / ω_z) / B_N(s / ω_p)` with `ω_p = ω_c r^(1/2N)` and `ω_z = ω_c r^(−1/2N)`,
/// so `|H|² = (1 + (ω/ω_z)^2N) / (1 + (ω/ω_p)^2N)`: monotonic from 1 to r, no bump either
/// way. Each section is one SVF at `ω_p` whose outputs are mixed as `ρ² hp + ρ k bp + lp`,
/// `ρ = r^(1/N)` — so at r = 1 it is `hp + k bp + lp`, the input.
#[derive(Copy, Clone, Debug, Default)]
pub struct Shelf {
    svf: [Svf; 4],
    pole: OnePole,
}

impl Shelf {
    /// Run the stage at pole coefficient `g` (`tan(π f_p / fs)`) and section gain `rho`.
    /// With `mix == false` the filter only keeps its state warm and the input is returned.
    #[inline]
    pub fn tick(&mut self, slope: Slope, g: f32, rho: f32, x: f32, mix: bool) -> f32 {
        let ks = slope.shelf_k();
        if ks.is_empty() {
            let lp = self.pole.tick(g / (1.0 + g), x);
            return if mix { lp + rho * (x - lp) } else { x };
        }
        let mut v = x;
        for (s, k) in self.svf.iter_mut().zip(ks) {
            let c = SvfCoefs::new(g, *k);
            let (lp, bp, hp) = s.tick(&c, v);
            if mix {
                v = rho * rho * hp + rho * k * bp + lp;
            }
        }
        v
    }
}

/// The pole coefficient and section gain of a shelf stage going from 1 to `r`.
#[inline]
pub fn shelf_params(slope: Slope, crossover_hz: f32, r: f32, sample_rate: f32) -> (f32, f32) {
    if r == 1.0 {
        return (warp(crossover_hz, sample_rate), 1.0);
    }
    let n = slope.shelf_order() as f32;
    let r = r.clamp(1e-6, 1e6);
    let rho = r.powf(1.0 / n);
    (warp(crossover_hz * rho.sqrt(), sample_rate), rho)
}

// ---- responses, for drawing and testing ---------------------------------------------

/// A complex number, just enough for frequency responses.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    pub const ONE: C = C { re: 1.0, im: 0.0 };
    pub fn new(re: f64, im: f64) -> C {
        C { re, im }
    }
    pub fn mul(self, o: C) -> C {
        C::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
    pub fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
    pub fn scale(self, k: f64) -> C {
        C::new(self.re * k, self.im * k)
    }
    pub fn div(self, o: C) -> C {
        let d = (o.re * o.re + o.im * o.im).max(1e-300);
        C::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
    pub fn abs(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
}

/// `s = jΩ` for a TPT filter with coefficient `g`, at `hz`.
fn s_at(g: f32, hz: f32, sample_rate: f32) -> C {
    C::new(0.0, warp(hz, sample_rate) as f64 / g as f64)
}

/// A second-order Butterworth-style polynomial `s² + k s + 1`.
fn quad(s: C, k: f32) -> C {
    s.mul(s).add(s.scale(k as f64)).add(C::ONE)
}

/// The low half, the high half and the all-pass of a crossover at `hz`, exactly as the
/// digital filter responds (see the module notes).
pub fn xover_response(c: &XoverCoefs, hz: f32, sample_rate: f32) -> (C, C, C) {
    let s = s_at(c.g, hz, sample_rate);
    match c.slope {
        Slope::S6 => {
            let d = s.add(C::ONE);
            let lp = C::ONE.div(d);
            (lp, s.div(d), C::ONE)
        }
        Slope::S12 => {
            let d = s.add(C::ONE);
            let lp = C::ONE.div(d.mul(d));
            let hp = s.mul(s).div(d.mul(d)).scale(-1.0);
            let ap = C::ONE.add(s.scale(-1.0)).div(d);
            (lp, hp, ap)
        }
        Slope::S24 | Slope::S48 => {
            let ks = c.slope.split_k();
            let mut b = C::ONE;
            let mut b_neg = C::ONE;
            let mut s_n = C::ONE;
            for k in ks {
                b = b.mul(quad(s, *k));
                b_neg = b_neg.mul(quad(s.scale(-1.0), *k));
                s_n = s_n.mul(s).mul(s);
            }
            let b2 = b.mul(b);
            (C::ONE.div(b2), s_n.mul(s_n).div(b2), b_neg.div(b))
        }
    }
}

/// A shelf stage's response at `hz` for pole coefficient `g` and section gain `rho`.
pub fn shelf_response(slope: Slope, g: f32, rho: f32, hz: f32, sample_rate: f32) -> C {
    let s = s_at(g, hz, sample_rate);
    let rho = rho as f64;
    let ks = slope.shelf_k();
    if ks.is_empty() {
        return s.scale(rho).add(C::ONE).div(s.add(C::ONE));
    }
    let mut h = C::ONE;
    for k in ks {
        let k = *k as f64;
        let num = s.mul(s).scale(rho * rho).add(s.scale(rho * k)).add(C::ONE);
        let den = s.mul(s).add(s.scale(k)).add(C::ONE);
        h = h.mul(num.div(den));
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_state_variable_section_splits_its_input_exactly() {
        let c = SvfCoefs::new(warp(700.0, 48_000.0), 1.3);
        let mut s = Svf::default();
        for i in 0..2000 {
            let x = ((i * 7919) % 1000) as f32 / 500.0 - 1.0;
            let (lp, bp, hp) = s.tick(&c, x);
            assert!((lp + c.k * bp + hp - x).abs() < 1e-6);
        }
    }

    #[test]
    fn crossover_halves_meet_where_the_textbook_says() {
        let sr = 48_000.0;
        for slope in Slope::ALL {
            let c = XoverCoefs::new(slope, 1_000.0, sr);
            let (lp, hp, ap) = xover_response(&c, 1_000.0, sr);
            let db = |z: C| 20.0 * z.abs().log10();
            // Linkwitz–Riley halves are −6 dB at the crossover; the one-pole pair −3 dB.
            let want = if slope == Slope::S6 { -3.01 } else { -6.02 };
            assert!((db(lp) - want).abs() < 0.01, "{slope:?} low {}", db(lp));
            assert!((db(hp) - want).abs() < 0.01, "{slope:?} high {}", db(hp));
            // And the halves sum to the all-pass everywhere.
            for hz in [20.0, 200.0, 1_000.0, 5_000.0, 19_000.0] {
                let (lp, hp, ap2) = xover_response(&c, hz, sr);
                let sum = lp.add(hp);
                // (To the precision of the f32 section constants.)
                assert!((sum.re - ap2.re).abs() < 1e-6 && (sum.im - ap2.im).abs() < 1e-6);
                assert!((ap2.abs() - 1.0).abs() < 1e-6);
            }
            assert!((ap.abs() - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn a_shelf_goes_from_one_to_r_through_its_square_root() {
        let sr = 48_000.0;
        for slope in Slope::ALL {
            for r in [0.25f32, 0.5, 2.0, 8.0] {
                let (g, rho) = shelf_params(slope, 1_000.0, r, sr);
                let at = |hz: f32| shelf_response(slope, g, rho, hz, sr).abs() as f32;
                assert!(
                    (at(20.0) - 1.0).abs() < 0.02,
                    "{slope:?} r {r}: DC {}",
                    at(20.0)
                );
                assert!(
                    (at(1_000.0) - r.sqrt()).abs() < r.sqrt() * 0.01,
                    "{slope:?} r {r}: centre {} vs {}",
                    at(1_000.0),
                    r.sqrt()
                );
                // Monotonic between the two plateaus: no bump.
                let mut last = at(20.0);
                let mut hz = 20.0;
                while hz < 20_000.0 {
                    hz *= 1.1;
                    let v = at(hz);
                    if r < 1.0 {
                        assert!(v <= last + 1e-5, "{slope:?} r {r} rises at {hz}");
                    } else {
                        assert!(v >= last - 1e-5, "{slope:?} r {r} falls at {hz}");
                    }
                    last = v;
                }
            }
        }
    }
}
