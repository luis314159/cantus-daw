//! Arca's valves tabulated: each valve's curves sampled once (off the audio thread, then
//! kept for the program's life) and read back by cubic Hermite interpolation — the value
//! and its slope at every node exact from the closed forms, the interpolant's own
//! derivative handed to Newton (so the slope it follows is the curve it solves). A read
//! is an index, four loads and a dozen multiply-adds, where the closed forms cost two
//! exponentials, two or three logarithms, a square root and several divisions.
//!
//! - **Triode** (Koren): `i_p = A(v_pk) · B(u)`, `u = v_gk · R(v_pk)`, with `A = v_pk^ex`,
//!   `R = 1/√(kvb + v_pk²)`, `B(u) = (2/kg1)·(ln(1 + e^{kp(1/μ + u)})/kp)^ex` — Koren's
//!   formula regrouped into three functions of one variable each. The grid current is a
//!   function of `v_gk` alone.
//! - **Pentode** (Koren's form): `i_p = v_g2^ex · B(v_g1/v_g2) · F(v_p)`; `B` tabulated,
//!   `v_g2^ex` computed once a sample (the screen moves slowly), `F` the plate's atan.
//!
//! Every table is built with `cantus_dsp::math`'s functions in a fixed order: the same
//! bits on every machine.

use crate::fragor::dk::{softplus, Koren};
use crate::math;
use std::sync::OnceLock;

/// Values and slopes on an even grid, read by cubic Hermite interpolation; beyond either
/// end, the line through the end node (its value and slope).
#[derive(Clone, PartialEq)]
pub struct Hermite {
    x0: f64,
    h: f64,
    inv_h: f64,
    /// Per node: the value and the slope times `h`.
    n: Vec<[f64; 2]>,
    last: usize,
}

impl Hermite {
    /// `n` nodes from `x0` to `x1`, `func` giving the value and the slope. Allocates.
    pub fn new(x0: f64, x1: f64, n: usize, func: impl Fn(f64) -> (f64, f64)) -> Hermite {
        let h = (x1 - x0) / (n - 1) as f64;
        let nodes = (0..n)
            .map(|k| {
                let (v, s) = func(x0 + h * k as f64);
                [v, s * h]
            })
            .collect();
        Hermite {
            x0,
            h,
            inv_h: 1.0 / h,
            n: nodes,
            last: n - 1,
        }
    }

    /// The value and the slope at `x`.
    #[inline]
    pub fn eval(&self, x: f64) -> (f64, f64) {
        let p = (x - self.x0) * self.inv_h;
        let last = self.last;
        if p <= 0.0 {
            let s = self.n[0][1] * self.inv_h;
            return (self.n[0][0] + s * (x - self.x0), s);
        }
        if p >= last as f64 {
            let xe = self.x0 + self.h * last as f64;
            let s = self.n[last][1] * self.inv_h;
            return (self.n[last][0] + s * (x - xe), s);
        }
        let i = p as usize;
        let t = p - i as f64;
        let pair = &self.n[i..i + 2];
        let (f0, d0, f1, d1) = (pair[0][0], pair[0][1], pair[1][0], pair[1][1]);
        let t2 = t * t;
        let t3 = t2 * t;
        let v = (2.0 * t3 - 3.0 * t2 + 1.0) * f0
            + (t3 - 2.0 * t2 + t) * d0
            + (3.0 * t2 - 2.0 * t3) * f1
            + (t3 - t2) * d1;
        let s = ((6.0 * t2 - 6.0 * t) * (f0 - f1)
            + (3.0 * t2 - 4.0 * t + 1.0) * d0
            + (3.0 * t2 - 2.0 * t) * d1)
            * self.inv_h;
        (v, s)
    }
}

/// Two functions on one even grid (values and slopes), read together.
#[derive(Clone, PartialEq)]
pub struct Hermite2 {
    x0: f64,
    h: f64,
    inv_h: f64,
    /// Per node: value and slope × h of the first, then of the second.
    n: Vec<[f64; 4]>,
    last: usize,
}

impl Hermite2 {
    pub fn new(x0: f64, x1: f64, n: usize, func: impl Fn(f64) -> [f64; 4]) -> Hermite2 {
        let h = (x1 - x0) / (n - 1) as f64;
        let nodes = (0..n)
            .map(|k| {
                let [a, da, b, db] = func(x0 + h * k as f64);
                [a, da * h, b, db * h]
            })
            .collect();
        Hermite2 {
            x0,
            h,
            inv_h: 1.0 / h,
            n: nodes,
            last: n - 1,
        }
    }

    /// Both functions' values and slopes at `x`, inside the grid (the caller keeps `x`
    /// there).
    #[inline]
    pub fn eval(&self, x: f64) -> [f64; 4] {
        let p = ((x - self.x0) * self.inv_h).clamp(0.0, self.last as f64 - 1e-9);
        let i = p as usize;
        let t = p - i as f64;
        let (a, b) = (self.n[i], self.n[i + 1]);
        let t2 = t * t;
        let t3 = t2 * t;
        let (h00, h10, h01, h11) = (
            2.0 * t3 - 3.0 * t2 + 1.0,
            t3 - 2.0 * t2 + t,
            3.0 * t2 - 2.0 * t3,
            t3 - t2,
        );
        let (g0, g1, g2) = (
            (6.0 * t2 - 6.0 * t) * self.inv_h,
            (3.0 * t2 - 4.0 * t + 1.0) * self.inv_h,
            (3.0 * t2 - 2.0 * t) * self.inv_h,
        );
        [
            h00 * a[0] + h10 * a[1] + h01 * b[0] + h11 * b[1],
            g0 * (a[0] - b[0]) + g1 * a[1] + g2 * b[1],
            h00 * a[2] + h10 * a[3] + h01 * b[2] + h11 * b[3],
            g0 * (a[2] - b[2]) + g1 * a[3] + g2 * b[3],
        ]
    }
}

impl std::fmt::Debug for Hermite2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hermite2({} nodes from {})", self.n.len(), self.x0)
    }
}

impl std::fmt::Debug for Hermite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hermite({} nodes from {})", self.n.len(), self.x0)
    }
}

/// Grid current is taken as nothing below this: about a nanoampere there (a millivolt
/// across a 1 MΩ leak, nothing across the stoppers) — the closed form's own cut is −1.2 V.
const GRID_LO: f64 = -0.8;
const GRID_HI: f64 = 8.0;
/// The plate's grid: 0 … 600 V, a quarter volt apart.
const PLATE_HI: f64 = 600.0;

/// A triode's tables.
#[derive(Clone, Debug, PartialEq)]
pub struct KorenTable {
    grid: Hermite,
    /// `A` and `R` on the plate's grid.
    ar: Hermite2,
    b: Hermite,
    /// Below this `u` the plate current is nothing (`kp(1/μ + u) < −50`).
    u_lo: f64,
    tube: Koren,
}

impl KorenTable {
    /// Allocates (off the audio thread).
    pub fn new(t: &Koren) -> KorenTable {
        let t = Koren { table: None, ..*t };
        let grid = Hermite::new(GRID_LO, GRID_HI, 4_601, |v| {
            let (ig, dig, ..) = t.currents(v, 200.0);
            (ig, dig)
        });
        let (ex, kvb) = (t.ex, t.kvb);
        let ar = Hermite2::new(0.0, PLATE_HI, 2_401, |v| {
            let (a, da) = if v <= 0.0 {
                (0.0, 0.0)
            } else {
                let p = math::powf(v, ex);
                (p, ex * p / v)
            };
            let s = (kvb + v * v).sqrt();
            [a, da, 1.0 / s, -v / (s * s * s)]
        });
        let u_lo = -50.0 / t.kp - 1.0 / t.mu;
        let u_hi = 0.6;
        let n = (((u_hi - u_lo) * 4.0 * t.kp).ceil() as usize).max(64) + 1;
        let (kg1, kp, mu) = (t.kg1, t.kp, t.mu);
        let b = Hermite::new(u_lo, u_hi, n, |u| {
            let (sp, sig) = softplus(kp * (1.0 / mu + u));
            let base = sp / kp;
            let pw = math::powf(base, ex - 1.0);
            (2.0 / kg1 * pw * base, 2.0 / kg1 * ex * pw * sig)
        });
        KorenTable {
            grid,
            ar,
            b,
            u_lo,
            tube: t,
        }
    }

    /// As [`Koren::currents`]: `(i_g, ∂i_g/∂v_gk, i_p, ∂i_p/∂v_gk, ∂i_p/∂v_pk)`.
    #[inline]
    pub fn currents(&self, vgk: f64, vpk: f64) -> (f64, f64, f64, f64, f64) {
        let (ig, dig) = if vgk > GRID_LO {
            self.grid.eval(vgk)
        } else {
            (0.0, 0.0)
        };
        if vpk <= 0.0 {
            return (ig, dig, 0.0, 0.0, 0.0);
        }
        if vpk >= PLATE_HI {
            let (_, _, ip, g, p) = self.tube.currents(vgk, vpk);
            return (ig, dig, ip, g, p);
        }
        let [a, da, r, dr] = self.ar.eval(vpk);
        let u = vgk * r;
        if u <= self.u_lo {
            return (ig, dig, 0.0, 0.0, 0.0);
        }
        let (b, db) = self.b.eval(u);
        (ig, dig, a * b, a * db * r, da * b + a * db * vgk * dr)
    }
}

static AX7_TABLE: OnceLock<KorenTable> = OnceLock::new();
static AY7_TABLE: OnceLock<KorenTable> = OnceLock::new();

/// The triode with its table (built on first use: call [`warm`] off the audio thread).
pub fn tabled(t: Koren) -> Koren {
    let cell = if t == super::tube::AX7 {
        &AX7_TABLE
    } else if t == super::tube::AY7 {
        &AY7_TABLE
    } else {
        return t;
    };
    Koren {
        table: Some(cell.get_or_init(|| KorenTable::new(&t))),
        ..t
    }
}

/// A pentode's `B(u)` (see the module's notes).
#[derive(Clone, Debug, PartialEq)]
pub struct PentodeTable {
    pub b: Hermite,
    pub u_lo: f64,
    /// The plate's factor `atan(v_p/kvb)/atan(250/kvb)` and its slope.
    pub plate: Hermite,
}

static PENTODE_TABLES: [OnceLock<PentodeTable>; 4] = [
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
    OnceLock::new(),
];

/// The pentode with its table.
pub fn pentode_tabled(p: super::tube::Pentode) -> super::tube::Pentode {
    use super::tube::{EL34, EL84, L6GC, V6GT};
    let key = super::tube::Pentode { table: None, ..p };
    let i = if key == EL34 {
        0
    } else if key == L6GC {
        1
    } else if key == EL84 {
        2
    } else if key == V6GT {
        3
    } else {
        return p;
    };
    let t = PENTODE_TABLES[i].get_or_init(|| {
        let (kg1, kp, mu, ex) = (p.kg1, p.kp, p.mu, p.ex);
        let u_lo = -50.0 / kp - 1.0 / mu;
        let u_hi = 0.3;
        let n = (((u_hi - u_lo) * 4.0 * kp).ceil() as usize).max(64) + 1;
        PentodeTable {
            b: Hermite::new(u_lo, u_hi, n, |u| {
                let (sp, sig) = softplus(kp * (1.0 / mu + u));
                let base = sp / kp;
                let pw = math::powf(base, ex - 1.0);
                (2.0 / kg1 * pw * base, 2.0 / kg1 * ex * pw * sig)
            }),
            u_lo,
            plate: {
                let kvb = p.kvb;
                let norm = 1.0 / math::atan(250.0 / kvb);
                Hermite::new(-800.0, 1_600.0, 4_801, |vp| {
                    let x = vp / kvb;
                    (math::atan(x) * norm, norm / (kvb * (1.0 + x * x)))
                })
            },
        }
    });
    super::tube::Pentode {
        table: Some(t),
        ..key
    }
}

/// Build every table now (off the audio thread: the first [`super::Arca::new`] does).
pub fn warm() {
    use super::tube::{AX7, AY7, EL34, EL84, L6GC, V6GT};
    tabled(AX7);
    tabled(AY7);
    for p in [EL34, L6GC, EL84, V6GT] {
        pentode_tabled(p);
    }
}
