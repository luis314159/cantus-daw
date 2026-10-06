//! The small-signal (linear) response of each filter model, as the digital filter has it —
//! the analog prototype at the bilinear-warped frequency — for an editor to draw and for
//! the tests to hold the models against.

use super::filter::{FilterSettings, Model, DIODE_K, DIODE_W};

#[derive(Copy, Clone, Debug, PartialEq)]
struct C {
    re: f64,
    im: f64,
}

impl C {
    fn new(re: f64, im: f64) -> C {
        C { re, im }
    }
    fn real(re: f64) -> C {
        C { re, im: 0.0 }
    }
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
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
    fn div(self, o: C) -> C {
        let d = o.re * o.re + o.im * o.im;
        C::new(
            (self.re * o.re + self.im * o.im) / d,
            (self.im * o.re - self.re * o.im) / d,
        )
    }
    fn abs(self) -> f64 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
    fn powi(self, n: usize) -> C {
        let mut r = C::real(1.0);
        for _ in 0..n {
            r = r.mul(self);
        }
        r
    }
}

/// The resonance the linear response is drawn at: at 1.0 and above it is unbounded (the
/// filter oscillates), so the picture stops just short.
pub const DRAW_RESO_MAX: f32 = 0.985;

/// The magnitude (linear) of the model's small-signal response at `f` Hz, with the cutoff
/// at `fc`, at sample rate `fs`. Drive is left out (it moves the level into the
/// nonlinearities, not the small-signal shape); bass compensation is in.
pub fn magnitude(s: &FilterSettings, fc: f32, f: f32, fs: f32) -> f32 {
    let pi = std::f64::consts::PI;
    let nyq = 0.4999 * fs as f64;
    let warp = if s.model == Model::Diode {
        1.0 / DIODE_W as f64
    } else {
        1.0
    };
    let g = crate::math::tan(pi * (fc as f64).clamp(1.0, nyq) / fs as f64) * warp;
    let w = crate::math::tan(pi * (f as f64).clamp(0.0, nyq) / fs as f64) / g;
    let s_ = C::new(0.0, w);
    let r = s.reso.clamp(0.0, DRAW_RESO_MAX) as f64;
    let comp = |k: f64| 1.0 + s.bass_comp.clamp(0.0, 1.0) as f64 * k;
    let tap = s.poles.clamp(1, 4) as usize;
    let h = match s.model {
        Model::Ladder | Model::Cascade => {
            let k = 4.0 * r;
            let one = C::real(1.0);
            let stage = one.div(one.add(s_));
            stage
                .powi(tap)
                .div(one.add(stage.powi(4).scale(k)))
                .scale(comp(k))
        }
        Model::Diode => {
            let k = DIODE_K as f64 * r;
            let s2 = s_.mul(s_);
            let s3 = s2.mul(s_);
            let s4 = s3.mul(s_);
            let d = s4
                .add(s3.scale(7.0))
                .add(s2.scale(15.0))
                .add(s_.scale(10.0))
                .add(C::real(1.0 + k));
            let n = match tap {
                4 => C::real(1.0),
                3 => s_.add(C::real(1.0)),
                2 => s2.add(s_.scale(3.0)).add(C::real(1.0)),
                _ => s3.add(s2.scale(5.0)).add(s_.scale(6.0)).add(C::real(1.0)),
            };
            n.div(d).scale(comp(k))
        }
        Model::Svf => {
            let k = 2.0 - 2.0 * r;
            let s2 = s_.mul(s_);
            let den = s2.add(s_.scale(k)).add(C::real(1.0));
            let lp = C::real(1.0).div(den);
            let hp = s2.div(den);
            let mid = if s.band {
                s_.div(den)
            } else {
                s2.add(C::real(1.0)).div(den)
            };
            let m = s.morph.clamp(0.0, 1.0) as f64;
            if m < 0.5 {
                lp.scale(1.0 - 2.0 * m).add(mid.scale(2.0 * m))
            } else {
                mid.scale(2.0 - 2.0 * m).add(hp.scale(2.0 * m - 1.0))
            }
        }
        Model::SallenKey => {
            let big_k = 2.0 * r;
            let den = s_.mul(s_).add(s_.scale(2.0 - big_k)).add(C::real(1.0));
            C::real(1.0).div(den)
        }
    };
    h.abs() as f32
}

/// [`magnitude`] in decibels, floored at −120.
pub fn magnitude_db(s: &FilterSettings, fc: f32, f: f32, fs: f32) -> f32 {
    20.0 * crate::math::log10(magnitude(s, fc, f, fs).max(1e-6))
}

/// The bass a four-pole model loses at resonance `reso` with its compensation, in dB (a
/// negative number; 0 for the two-poles, which keep their bass).
pub fn bass_loss_db(s: &FilterSettings) -> f32 {
    if !s.model.four_pole() {
        return 0.0;
    }
    let k = s.model.k_threshold() * s.reso.clamp(0.0, 1.25);
    let gain = (1.0 + s.bass_comp.clamp(0.0, 1.0) * k) / (1.0 + k);
    20.0 * crate::math::log10(gain)
}
