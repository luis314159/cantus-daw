//! Arca's lean stepper: the DK method's per-sample work for circuits whose only nonlinear
//! elements are triodes (two at most) — every circuit Arca builds — taking the fused
//! matrices [`crate::fragor::dk::Circuit`] computed. The same algorithm as
//! `Circuit::step` (the same predictor, its bounds, Newton's tolerances and step limits,
//! the first-order correction, the patient fallback), written for this one case: the
//! triodes' currents read straight from their tables, the Jacobian's known shape filled
//! directly, a 2 × 2 solved in closed form and a 4 × 4 by elimination. Allocation-free.

// Dense linear algebra on small fixed arrays reads best with its indices.
#![allow(clippy::needless_range_loop)]

use crate::fragor::dk::{
    Fused, Koren, ITER, MAX_C, MAX_NL, MAX_Q, ROWS, STOP, TRIODE_GRID_STEP, TRIODE_PLATE_STEP,
};

/// A circuit's state as the stepper keeps it (what rest returns to).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FastState {
    j: [f64; MAX_C],
    v: [f64; MAX_NL],
    v1: [f64; MAX_NL],
    v2: [f64; MAX_NL],
    v3: [f64; MAX_NL],
}

#[derive(Clone, Debug)]
pub struct Fast {
    f: Fused,
    tubes: [Koren; 2],
    nt: usize,
    iters: u64,
    steps: u64,
    /// The ports three samples back (the cubic predictor's fourth point).
    v3: [f64; MAX_NL],
}

impl Fast {
    /// From a solved circuit's matrices and state (its triodes, two at most, the only
    /// nonlinear elements).
    pub fn new(f: Fused) -> Fast {
        assert!(f.all_triodes, "the lean stepper takes triodes only");
        let mut tubes = [crate::arca::tube::AX7; 2];
        let mut nt = 0;
        for t in f.tubes.iter().flatten() {
            tubes[nt] = *t;
            nt += 1;
        }
        let v3 = f.v2;
        let mut f = f;
        // The cubic predictor is good to millivolts on the oversampled signal: its bounds
        // only stop a wild guess (a grid 5 V, a plate 200 V a sample).
        for t in 0..nt {
            f.predict[2 * t] = 5.0;
            f.predict[2 * t + 1] = 200.0;
        }
        Fast {
            f,
            tubes,
            nt,
            iters: 0,
            steps: 0,
            v3,
        }
    }

    /// New matrices (a knob moved), the state kept.
    pub fn load(&mut self, f: &Fused) {
        let s = self.state();
        let predict = self.f.predict;
        self.f = f.clone();
        self.f.predict = predict;
        self.restore(&s);
    }

    pub fn state(&self) -> FastState {
        FastState {
            j: self.f.j,
            v: self.f.v,
            v1: self.f.v1,
            v2: self.f.v2,
            v3: self.v3,
        }
    }

    pub fn restore(&mut self, s: &FastState) {
        self.f.j = s.j;
        self.f.v = s.v;
        self.f.v1 = s.v1;
        self.f.v2 = s.v2;
        self.v3 = s.v3;
    }

    /// Newton's iterations a sample so far.
    pub fn iterations_per_step(&self) -> f64 {
        self.iters as f64 / self.steps.max(1) as f64
    }

    /// The tubes' currents at `v`, and each tube's `(∂i_g/∂v_gk, ∂i_p/∂v_gk, ∂i_p/∂v_pk)`.
    #[inline]
    fn currents(&self, v: &[f64; MAX_NL], i: &mut [f64; MAX_NL], dd: &mut [[f64; 3]; 2]) {
        for t in 0..self.nt {
            let (ig, dig, ip, dpg, dpp) = self.tubes[t].currents(v[2 * t], v[2 * t + 1]);
            i[2 * t] = ig;
            i[2 * t + 1] = ip;
            dd[t] = [dig, dpg, dpp];
        }
    }

    #[inline]
    fn newton(&mut self, p: &[f64; MAX_NL], iters: usize) -> ([f64; MAX_NL], bool) {
        let d = self.f.dims;
        let mut i = [0.0; MAX_NL];
        let mut dd = [[0.0; 3]; 2];
        for _ in 0..iters {
            self.iters += 1;
            let v = self.f.v;
            self.currents(&v, &mut i, &mut dd);
            let k = &self.f.k;
            // F = p + K i − v;  J = K ∂i/∂v − I;  the step solves J·Δ = −F.
            let mut rhs = [0.0; 4];
            let mut jm = [[0.0; 4]; 4];
            for r in 0..d {
                let mut s = p[r] - v[r];
                for c in 0..d {
                    s += k[r][c] * i[c];
                }
                rhs[r] = -s;
                for t in 0..self.nt {
                    let (g, pl) = (2 * t, 2 * t + 1);
                    let [dig, dpg, dpp] = dd[t];
                    jm[r][g] = k[r][g] * dig + k[r][pl] * dpg;
                    jm[r][pl] = k[r][pl] * dpp;
                }
                jm[r][r] -= 1.0;
            }
            let step = solve(&mut jm, rhs, d);
            let mut big = 0.0f64;
            for r in 0..d {
                big = big.max(step[r].abs() * self.f.stop_scale[r]);
            }
            if big < STOP {
                for r in 0..d {
                    self.f.v[r] += step[r];
                }
                // Converged: the currents at the new voltages, to first order.
                let mut out = i;
                for t in 0..self.nt {
                    let (g, pl) = (2 * t, 2 * t + 1);
                    let [dig, dpg, dpp] = dd[t];
                    out[g] += dig * step[g];
                    out[pl] += dpg * step[g] + dpp * step[pl];
                }
                return (out, true);
            }
            for t in 0..self.nt {
                let (g, pl) = (2 * t, 2 * t + 1);
                self.f.v[g] += step[g].clamp(-TRIODE_GRID_STEP, TRIODE_GRID_STEP);
                self.f.v[pl] += step[pl].clamp(-TRIODE_PLATE_STEP, TRIODE_PLATE_STEP);
            }
        }
        let v = self.f.v;
        self.currents(&v, &mut i, &mut dd);
        (i, false)
    }

    /// One sample: inputs `u` (the signal, then the supplies), the first output relative
    /// to the operating point back.
    #[inline]
    pub fn step(&mut self, u: &[f64; 3]) -> (f64, f64) {
        self.steps += 1;
        let (m, rows, ni, nc, dims) = (self.f.m, self.f.rows, self.f.ni, self.f.nc, self.f.dims);
        let mut q = [0.0; MAX_Q];
        q[..ni].copy_from_slice(&u[..ni]);
        q[ni..m].copy_from_slice(&self.f.j[..nc]);
        let mut acc = [0.0f64; ROWS];
        for (col, qj) in self.f.aq.iter().zip(&q).take(m) {
            for (a, c) in acc.iter_mut().zip(col).take(rows) {
                *a += c * qj;
            }
        }
        if dims > 0 {
            let mut p = [0.0; MAX_NL];
            p[..dims].copy_from_slice(&acc[..dims]);
            let v1 = self.f.v;
            // The cubic through the last four answers (the oversampled signal is smooth: its
            // error falls as the fourth power of the step), bounded as the solver bounds it.
            for k in 0..dims {
                let guess = 4.0 * (self.f.v[k] + self.f.v2[k]) - 6.0 * self.f.v1[k] - self.v3[k];
                // No further than twice the last sample's move: past a valve's knee the
                // cubic overshoots, and Newton pays for it.
                let b = self.f.predict[k].min(2.0 * (self.f.v[k] - self.f.v1[k]).abs() + 1e-3);
                self.f.v[k] += (guess - self.f.v[k]).clamp(-b, b);
            }
            self.v3 = self.f.v2;
            self.f.v2 = self.f.v1;
            self.f.v1 = v1;
            let (i, ok) = self.newton(&p, ITER);
            let i = if ok {
                i
            } else {
                self.f.v = v1;
                self.newton(&p, ITER * 8).0
            };
            for (col, id) in self.f.ai.iter().zip(&i).take(dims) {
                for (a, c) in acc.iter_mut().zip(col).take(rows) {
                    *a += c * id;
                }
            }
        }
        let cap = self.f.cap_row;
        for k in 0..nc {
            self.f.j[k] = 2.0 * self.f.gc[k] * acc[cap + k] - self.f.j[k];
        }
        let o = self.f.out_row;
        let second = if self.f.nout > 1 {
            acc[o + 1] - self.f.y0[1]
        } else {
            0.0
        };
        (acc[o] - self.f.y0[0], second)
    }
}

/// `jm · x = rhs` for `d` of 1, 2 or 4 (partial pivoting for 4).
#[inline]
fn solve(jm: &mut [[f64; 4]; 4], mut rhs: [f64; 4], d: usize) -> [f64; 4] {
    match d {
        0 => rhs,
        1 => {
            rhs[0] /= jm[0][0];
            rhs
        }
        2 => {
            let det = jm[0][0] * jm[1][1] - jm[0][1] * jm[1][0];
            let inv = 1.0 / det;
            let x0 = (rhs[0] * jm[1][1] - jm[0][1] * rhs[1]) * inv;
            let x1 = (jm[0][0] * rhs[1] - rhs[0] * jm[1][0]) * inv;
            [x0, x1, 0.0, 0.0]
        }
        _ => {
            for col in 0..d {
                let mut piv = col;
                for r in col + 1..d {
                    if jm[r][col].abs() > jm[piv][col].abs() {
                        piv = r;
                    }
                }
                jm.swap(col, piv);
                rhs.swap(col, piv);
                let inv = 1.0 / jm[col][col];
                for r in col + 1..d {
                    let f = jm[r][col] * inv;
                    for c in col..d {
                        jm[r][c] -= f * jm[col][c];
                    }
                    rhs[r] -= f * rhs[col];
                }
            }
            for col in (0..d).rev() {
                let mut s = rhs[col];
                for c in col + 1..d {
                    s -= jm[col][c] * rhs[c];
                }
                rhs[col] = s / jm[col][col];
            }
            rhs
        }
    }
}
