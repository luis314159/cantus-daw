//! The five models' per-sample solvers (children of `va::filter`).
//!
//! Each integrator is trapezoidal in the "instantaneous response" form `y = s + g·f`, with
//! state update `s ← 2y − s`. A saturator `sat(v)` is replaced for one pass by the gain
//! `sat(v̂)/v̂` at an estimate `v̂` — the previous sample's value on the first pass, the
//! previous pass's answer after it — which leaves a linear system solved exactly.

use super::{FilterCoefs, Model, Solve, VaFilter};
use crate::va::math::tanh_gain;

/// Converged: the output moved less than this between passes.
const TOL: f32 = 1e-6;

impl VaFilter {
    /// The transistor ladder (`ota == false`: `ẏᵢ = ω(tanh(inᵢ) − tanh(yᵢ))`) or the OTA
    /// cascade (`ota == true`: `ẏᵢ = ω·tanh(inᵢ − yᵢ)`), with `in₁ = x − k·y₄`.
    #[inline]
    pub(super) fn ladder(
        &mut self,
        x: f32,
        g: f32,
        c: &FilterCoefs,
        solve: Solve,
        ota: bool,
    ) -> f32 {
        let k = c.k;
        let xin = x * c.in_gain;
        let s = self.s;
        let mut e = self.estimate();
        let mut eu = xin - k * e[3];
        let (min, max) = solve.passes();
        let mut y = e;
        let mut passes = 0;
        while passes < max {
            passes += 1;
            let (a, b, pole) = if ota {
                let t = [
                    c.input_gain(eu - e[0]),
                    tanh_gain(c.sat * (e[0] - e[1])),
                    tanh_gain(c.sat * (e[1] - e[2])),
                    tanh_gain(c.sat * (e[2] - e[3])),
                ];
                let d = reciprocals([
                    1.0 + g * t[0],
                    1.0 + g * t[1],
                    1.0 + g * t[2],
                    1.0 + g * t[3],
                ]);
                let mut a = [0.0; 4];
                let mut b = [0.0; 4];
                for i in 0..4 {
                    a[i] = g * t[i] * d[i];
                    b[i] = s[i] * d[i];
                }
                (a, b, 0.25 * (t[0] + t[1] + t[2] + t[3]))
            } else {
                let t_in = c.input_gain(eu);
                let t = [
                    tanh_gain(c.sat * e[0]),
                    tanh_gain(c.sat * e[1]),
                    tanh_gain(c.sat * e[2]),
                    tanh_gain(c.sat * e[3]),
                ];
                let t_prev = [t_in, t[0], t[1], t[2]];
                let d = reciprocals([
                    1.0 + g * t[0],
                    1.0 + g * t[1],
                    1.0 + g * t[2],
                    1.0 + g * t[3],
                ]);
                let mut a = [0.0; 4];
                let mut b = [0.0; 4];
                for i in 0..4 {
                    a[i] = g * t_prev[i] * d[i];
                    b[i] = s[i] * d[i];
                }
                (a, b, 0.25 * (t[0] + t[1] + t[2] + t[3]))
            };
            self.pole = pole;
            // y₄ = A·u + B, u = xin − k·y₄.
            let big_a = a[0] * a[1] * a[2] * a[3];
            let big_b = ((b[0] * a[1] + b[1]) * a[2] + b[2]) * a[3] + b[3];
            let y4 = (big_a * xin + big_b) / (1.0 + k * big_a);
            let u = xin - k * y4;
            y[0] = a[0] * u + b[0];
            y[1] = a[1] * y[0] + b[1];
            y[2] = a[2] * y[1] + b[2];
            y[3] = y4;
            let moved = (y4 - e[3]).abs();
            e = y;
            eu = u;
            if passes >= min && moved < TOL {
                break;
            }
        }
        for i in 0..4 {
            self.s[i] = 2.0 * y[i] - s[i];
        }
        self.keep(y);
        self.last_passes = passes;
        y[c.tap]
    }

    /// The diode ladder: four capacitors joined by diode pairs, each branch current
    /// `iⱼ = tanh(Δvⱼ)`; the stages load each other, so the system is tridiagonal, with
    /// the feedback `u = x − k·v₄` as one extra corner. Solved by the Thomas algorithm for
    /// the loop open, then closed with one scalar equation.
    #[inline]
    pub(super) fn diode(&mut self, x: f32, g: f32, c: &FilterCoefs, solve: Solve) -> f32 {
        let k = c.k;
        let xin = x * c.in_gain;
        let s = self.s;
        let mut e = self.estimate();
        let mut eu = xin - k * e[3];
        let (min, max) = solve.passes();
        let mut v = e;
        let mut passes = 0;
        while passes < max {
            passes += 1;
            // Branch conductances at the operating point.
            let t1 = c.input_gain(eu - e[0]);
            let t2 = tanh_gain(c.sat * (e[0] - e[1]));
            let t3 = tanh_gain(c.sat * (e[1] - e[2]));
            let t4 = tanh_gain(c.sat * (e[2] - e[3]));
            let (gt1, gt2, gt3, gt4) = (g * t1, g * t2, g * t3, g * t4);
            self.pole = 0.25 * (t1 + t2 + t3 + t4);
            let d = [1.0 + gt1 + gt2, 1.0 + gt2 + gt3, 1.0 + gt3 + gt4, 1.0 + gt4];
            let off = [-gt2, -gt3, -gt4];
            // Forward elimination (each pivot's reciprocal taken once, used twice).
            let r1 = 1.0 / d[0];
            let w2 = off[0] * r1;
            let r2 = 1.0 / (d[1] - off[0] * w2);
            let w3 = off[1] * r2;
            let r3 = 1.0 / (d[2] - off[1] * w3);
            let w4 = off[2] * r3;
            let r4 = 1.0 / (d[3] - off[2] * w4);
            // Open loop (u = xin), then how v₄ answers a unit change in row 1.
            let rhs1 = s[0] + gt1 * xin;
            let z2 = s[1] - w2 * rhs1;
            let z3 = s[2] - w3 * z2;
            let z4 = s[3] - w4 * z3;
            let v4_open = z4 * r4;
            let p = -w2 * w3 * w4 * r4;
            // Close the loop: row 1 loses g·t₁·k·v₄.
            let v4 = v4_open / (1.0 + p * gt1 * k);
            let z1 = rhs1 - gt1 * k * v4;
            let z2 = s[1] - w2 * z1;
            let z3 = s[2] - w3 * z2;
            v[3] = v4;
            v[2] = (z3 - off[2] * v4) * r3;
            v[1] = (z2 - off[1] * v[2]) * r2;
            v[0] = (z1 - off[0] * v[1]) * r1;
            let moved = (v4 - e[3]).abs();
            e = v;
            eu = xin - k * v4;
            if passes >= min && moved < TOL {
                break;
            }
        }
        for i in 0..4 {
            self.s[i] = 2.0 * v[i] - s[i];
        }
        self.keep(v);
        self.last_passes = passes;
        v[c.tap]
    }

    /// The state-variable filter (Simper's trapezoidal form), its damping raised by the
    /// band-pass amplitude: `k = k₀ + nl·bp²`. Negative `k₀` (resonance above 1) grows
    /// until that term holds it, so self-oscillation is a stable sine at the cutoff.
    #[inline]
    pub(super) fn svf(&mut self, x: f32, g: f32, c: &FilterCoefs, solve: Solve) -> f32 {
        // A soft input stage with two units of headroom.
        let xd = x * c.in_gain;
        let xin = xd * tanh_gain(0.5 * c.sat * xd);
        let (ic1, ic2) = (self.s[0], self.s[1]);
        let mut bp = self.estimate()[0];
        let (min, max) = solve.passes();
        let (mut v1, mut v2, mut kd) = (0.0, 0.0, c.k);
        let mut passes = 0;
        while passes < max {
            passes += 1;
            let b = bp + c.bias * 0.5;
            kd = c.k + c.nl * b * b;
            self.pole = (1.0 - 0.25 * kd * kd).max(0.25).sqrt();
            let a1 = 1.0 / (1.0 + g * (g + kd));
            let a2 = g * a1;
            let a3 = g * a2;
            let v3 = xin - ic2;
            v1 = a1 * ic1 + a2 * v3;
            v2 = ic2 + a2 * ic1 + a3 * v3;
            let moved = (v1 - bp).abs();
            bp = v1;
            if passes >= min && moved < TOL {
                break;
            }
        }
        self.s[0] = 2.0 * v1 - ic1;
        self.s[1] = 2.0 * v2 - ic2;
        self.keep([v1, v2, 0.0, 0.0]);
        self.last_passes = passes;
        let lp = v2;
        let hp = xin - kd * v1 - v2;
        let mid = if c.band { v1 } else { lp + hp };
        let m = c.morph;
        if m <= 0.0 {
            lp
        } else if m < 0.5 {
            lp + (mid - lp) * (2.0 * m)
        } else {
            mid + (hp - mid) * (2.0 * m - 1.0)
        }
    }

    /// The two-pole Sallen-Key: two one-poles, the band-pass difference fed back positively
    /// through a diode limiter, `u = x + sat(K·(y₁ − y₂))`, so `H = 1/(s² + (2 − K)s + 1)`
    /// while it is small.
    #[inline]
    pub(super) fn sallen_key(&mut self, x: f32, g: f32, c: &FilterCoefs, solve: Solve) -> f32 {
        let xd = x * c.in_gain;
        let xin = xd * tanh_gain(0.5 * c.sat * xd);
        let big_k = c.k;
        let a = g / (1.0 + g);
        let b1 = self.s[0] / (1.0 + g);
        let b2 = self.s[1] / (1.0 + g);
        let est = self.estimate();
        let (mut e1, mut e2) = (est[0], est[1]);
        let (min, max) = solve.passes();
        let (mut y1, mut y2) = (e1, e2);
        let mut passes = 0;
        while passes < max {
            passes += 1;
            // The limiter bites at about a diode drop: harder than the stages.
            let kt = big_k * c.input_gain(1.6 * big_k * (e1 - e2));
            let damp = 2.0 - kt;
            self.pole = (1.0 - 0.25 * damp * damp).max(0.25).sqrt();
            y1 = (a * xin - a * kt * b2 + b1) / (1.0 - a * kt * (1.0 - a));
            y2 = a * y1 + b2;
            let moved = (y2 - e2).abs();
            e1 = y1;
            e2 = y2;
            if passes >= min && moved < TOL {
                break;
            }
        }
        self.s[0] = 2.0 * y1 - self.s[0];
        self.s[1] = 2.0 * y2 - self.s[1];
        self.keep([y1, y2, 0.0, 0.0]);
        self.last_passes = passes;
        y2
    }
}

/// `1/q` for four numbers with one division: `1/(q₀q₁q₂q₃)` times the other three.
#[inline]
fn reciprocals(q: [f32; 4]) -> [f32; 4] {
    let q01 = q[0] * q[1];
    let q23 = q[2] * q[3];
    let inv = 1.0 / (q01 * q23);
    let (i01, i23) = (inv * q23, inv * q01);
    [i01 * q[1], i01 * q[0], i23 * q[3], i23 * q[2]]
}

/// The model's name for the debugger and the tests.
pub fn describe(m: Model) -> &'static str {
    match m {
        Model::Ladder => "ladder",
        Model::Cascade => "cascade",
        Model::Diode => "diode",
        Model::Svf => "state-variable",
        Model::SallenKey => "sallen-key",
    }
}
