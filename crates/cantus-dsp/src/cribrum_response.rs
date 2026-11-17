//! Cribrum's response as the editor draws it: each filter's small-signal (linear) response
//! — the analog prototype at the bilinear-warped frequency, as the synths' editors draw
//! theirs ([`crate::va::response`]) — in the mode its model gives, and the two filters
//! together (in series the product, in parallel the mean, phase and all).
//!
//! The resonance is drawn up to just short of self-oscillation (where the response is
//! unbounded); the drive's level is in (the filter's gain times its trim), its saturation
//! is not.

use super::filter::{drive_trim, effective, formants, model_reso, Mode, Voice, VOWEL_MAKEUP};
use crate::va::filter::{Model, DIODE_K, DIODE_W};
use crate::va::response::DRAW_RESO_MAX;

/// A complex number, enough for a response.
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
    pub fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
    pub fn mul(self, o: C) -> C {
        C::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
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
    fn powi(self, n: usize) -> C {
        let mut r = C::ONE;
        for _ in 0..n {
            r = r.mul(self);
        }
        r
    }
}

/// `jω` for `f` against a cutoff `fc`, at sample rate `fs`, warped as the bilinear
/// transform warps it (`warp`: the model's own integrator scale).
fn jw(f: f32, fc: f32, fs: f32, warp: f64) -> C {
    let pi = std::f64::consts::PI;
    let nyq = 0.4999 * f64::from(fs);
    let g = crate::math::tan(pi * f64::from(fc).clamp(1.0, nyq) / f64::from(fs)) * warp;
    let w = crate::math::tan(pi * f64::from(f).clamp(0.0, nyq) / f64::from(fs)) / g;
    C::new(0.0, w)
}

/// One filter's response at `f` Hz, sample rate `fs` (the base rate: the oversampled
/// filter is drawn as heard).
pub fn response(v: &Voice, f: f32, fs: f32) -> C {
    let (mode, slope24) = effective(v.model, v.mode, v.slope24);
    let level = f64::from(v.drive.max(0.0) * drive_trim(v.drive));
    if mode == Mode::Vowel {
        let mut sum = C::new(0.0, 0.0);
        for (fc, a, bw) in formants(v.vowel, v.cut, v.res) {
            let s = jw(f, fc, fs, 1.0);
            let k = f64::from((bw / fc.max(1.0)).clamp(0.005, 2.0));
            let den = s.mul(s).add(s.scale(k)).add(C::ONE);
            sum = sum.add(s.scale(k * f64::from(a)).div(den));
        }
        return sum.scale(f64::from(VOWEL_MAKEUP) * level);
    }
    let r = f64::from(model_reso(v.res).min(DRAW_RESO_MAX));
    let h = match v.model {
        Model::Svf => {
            let s = jw(f, v.cut, fs, 1.0);
            let s2 = s.mul(s);
            let den = s2.add(s.scale(2.0 - 2.0 * r)).add(C::ONE);
            let (lp, bp, hp) = (C::ONE.div(den), s.div(den), s2.div(den));
            let bw = s2.add(s.scale(std::f64::consts::SQRT_2)).add(C::ONE);
            match mode {
                Mode::Low if slope24 => lp.div(bw),
                Mode::High if slope24 => hp.mul(s2.div(bw)),
                Mode::Low => lp,
                Mode::High => hp,
                Mode::Band => bp.scale(2.0),
                Mode::Notch => lp.add(hp),
                _ => lp.add(hp.scale(-1.0)),
            }
        }
        Model::Ladder | Model::Cascade => {
            let s = jw(f, v.cut, fs, 1.0);
            let k = 4.0 * r;
            let st = C::ONE.div(C::ONE.add(s));
            let comp = if mode == Mode::Low {
                1.0 + 0.5 * k
            } else {
                1.0
            };
            let u = C::ONE.div(C::ONE.add(st.powi(4).scale(k))).scale(comp);
            let y = |i: usize| u.mul(st.powi(i));
            let mix = |c: &[f64]| {
                c.iter()
                    .enumerate()
                    .fold(C::new(0.0, 0.0), |acc, (i, ci)| acc.add(y(i).scale(*ci)))
            };
            match (mode, slope24) {
                (Mode::Low, true) => y(4),
                (Mode::Low, false) => y(2),
                (Mode::High, true) => mix(&[1.0, -4.0, 6.0, -4.0, 1.0]),
                (Mode::High, false) => mix(&[1.0, -2.0, 1.0]),
                (Mode::Notch, true) => mix(&[1.0, -4.0, 8.0, -8.0, 4.0]),
                (Mode::Notch, false) => mix(&[1.0, -2.0, 2.0]),
                (_, true) => mix(&[0.0, 0.0, 4.0, -8.0, 4.0]),
                (_, false) => mix(&[0.0, 2.0, -2.0]),
            }
        }
        Model::Diode => {
            let s = jw(f, v.cut, fs, 1.0 / f64::from(DIODE_W));
            let k = f64::from(DIODE_K) * r;
            let s2 = s.mul(s);
            let s3 = s2.mul(s);
            let s4 = s3.mul(s);
            let d = s4
                .add(s3.scale(7.0))
                .add(s2.scale(15.0))
                .add(s.scale(10.0))
                .add(C::new(1.0 + k, 0.0));
            let n = if slope24 {
                C::ONE
            } else {
                s2.add(s.scale(3.0)).add(C::ONE)
            };
            n.div(d).scale(1.0 + 0.5 * k)
        }
        Model::SallenKey => {
            let s = jw(f, v.cut, fs, 1.0);
            let den = s.mul(s).add(s.scale(2.0 - 2.0 * r)).add(C::ONE);
            C::ONE.div(den)
        }
    };
    h.scale(level)
}

/// Both filters at `f`: A alone, A then B, or their mean.
pub fn chain(a: &Voice, b: Option<&Voice>, parallel: bool, f: f32, fs: f32) -> C {
    let ha = response(a, f, fs);
    match b {
        None => ha,
        Some(b) if parallel => ha.add(response(b, f, fs)).scale(0.5),
        Some(b) => ha.mul(response(b, f, fs)),
    }
}

/// [`chain`]'s magnitude in decibels, floored at −120.
pub fn chain_db(a: &Voice, b: Option<&Voice>, parallel: bool, f: f32, fs: f32) -> f32 {
    let m = chain(a, b, parallel, f, fs).abs().max(1e-6);
    20.0 * crate::math::log10(m as f32)
}
