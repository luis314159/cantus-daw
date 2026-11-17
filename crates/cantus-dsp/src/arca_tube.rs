//! Arca's valves: the preamp triodes (as [`Koren`] constants for the circuit solver) and the
//! power pentodes (a closed-form plate current, solved per sample by the power amp).
//!
//! | Valve | Where | Constants |
//! |---|---|---|
//! | 12AX7 / ECC83 | every preamp stage | Koren's published fit (μ 100, ex 1.4, kg1 1060, kp 600, kvb 300); grid current Dempwolf & Zölzer's 12AX7 fit |
//! | 12AY7 | the bass amp's first stage | μ 44 (the datasheet's), kp and kg1 fitted to its typical point: 250 V, −4 V → 3.0 mA, 1.75 mA/V |
//! | EL34, 6L6GC, EL84, 6V6GT | the power amps | Koren's pentode form, μ (screen) from the family, kp and kg1 fitted to each datasheet's class-A point (below) |
//!
//! The pentode datasheet points the fits hold (plate = screen = 250 V): EL34 −12.2 V →
//! 100 mA, 11 mA/V; 6L6GC −14 V → 72 mA, 6 mA/V; EL84 −7.3 V → 48 mA, 11.3 mA/V; 6V6GT
//! −12.5 V → 45 mA, 4.1 mA/V (`arca_tests::the_valves_hold_their_datasheet_points`).

use crate::fragor::dk::{fast_exp, fast_ln, softplus, Koren};

/// The 12AX7, Koren's constants; Dempwolf & Zölzer's grid current (Gg, Cg, ξ).
pub const AX7: Koren = Koren {
    mu: 100.0,
    ex: 1.4,
    kg1: 1060.0,
    kp: 600.0,
    kvb: 300.0,
    gg: 6.177e-4,
    cg: 9.901,
    xi: 1.314,
    table: None,
};

/// The 12AY7 (lower gain): μ from the datasheet, kp and kg1 fitted to its typical point.
pub const AY7: Koren = Koren {
    mu: 44.0,
    ex: 1.4,
    kg1: 1683.83,
    kp: 213.90,
    kvb: 300.0,
    gg: 6.177e-4,
    cg: 9.901,
    xi: 1.314,
    table: None,
};

/// A power pentode (or beam tetrode), Koren's form:
///
/// ```text
/// E1 = (v_g2 / kp) · ln(1 + exp(kp · (1/μ + v_g1 / v_g2)))
/// i_p = 2 · E1^ex / kg1 · atan(v_p / kvb) / atan(250 / kvb)
/// ```
///
/// (the plate factor normalised to the fit's 250 V plate, so the datasheet point holds
/// exactly; it falls toward zero as the plate swings down to the cathode — the valve
/// "bottoming").
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pentode {
    pub mu: f64,
    pub ex: f64,
    pub kg1: f64,
    pub kp: f64,
    pub kvb: f64,
    /// The screen's current as a share of the plate's (what the supply also carries).
    pub screen: f64,
    /// `B(u)` tabulated ([`super::table`]).
    pub table: Option<&'static super::table::PentodeTable>,
}

pub const EL34: Pentode = Pentode {
    mu: 11.0,
    ex: 1.35,
    kg1: 511.56,
    kp: 52.005,
    kvb: 24.0,
    screen: 0.12,
    table: None,
};
pub const L6GC: Pentode = Pentode {
    mu: 8.7,
    ex: 1.35,
    kg1: 1086.71,
    kp: 44.783,
    kvb: 12.0,
    screen: 0.08,
    table: None,
};
pub const EL84: Pentode = Pentode {
    mu: 26.5,
    ex: 1.35,
    kg1: 243.665,
    kp: 70.0,
    kvb: 20.0,
    screen: 0.11,
    table: None,
};
pub const V6GT: Pentode = Pentode {
    mu: 9.8,
    ex: 1.35,
    kg1: 1494.86,
    kp: 45.029,
    kvb: 12.0,
    screen: 0.08,
    table: None,
};

/// A screen voltage's terms, computed once a sample: `(v_g2, 1/v_g2, v_g2^ex)`.
#[derive(Copy, Clone, Debug)]
pub struct Screen(pub f64, pub f64, pub f64);

impl Pentode {
    pub fn screen(&self, vg2: f64) -> Screen {
        let v = vg2.max(1.0);
        Screen(v, 1.0 / v, fast_exp(self.ex * fast_ln(v)))
    }

    /// As [`Self::screen`], `v_g2^ex` from its value at `at` (`at_ex`) by the cubic Taylor
    /// series in `δ = v_g2/at − 1` (under 10⁻⁴ relative for a sag of ±15 %; exact at idle):
    /// the screen moves slowly, a power a sample is waste.
    #[inline]
    pub fn screen_near(&self, vg2: f64, at: f64, inv_at: f64, at_ex: f64) -> Screen {
        let v = vg2.max(1.0);
        let d = v * inv_at - 1.0;
        if d.abs() > 0.3 {
            return self.screen(v);
        }
        let e = self.ex;
        let p = 1.0 + d * (e + d * (e * (e - 1.0) * 0.5 + d * (e * (e - 1.0) * (e - 2.0) / 6.0)));
        let _ = at;
        Screen(v, 1.0 / v, at_ex * p)
    }

    /// As [`Self::current`], the screen's terms given, `B(u)` read from its table.
    #[inline]
    pub fn current_at(&self, vg: f64, g2: Screen, vp: f64) -> (f64, f64, f64) {
        let Some(t) = self.table else {
            return self.current(vg, g2.0, vp);
        };
        let u = vg * g2.1;
        let (base, dbase) = if u <= t.u_lo {
            (0.0, 0.0)
        } else {
            let (b, db) = t.b.eval(u);
            (g2.2 * b, g2.2 * db * g2.1)
        };
        let (f, df) = t.plate.eval(vp);
        (base * f, dbase * f, base * df)
    }

    /// Plate current and its derivatives `(i_p, ∂i_p/∂v_g1, ∂i_p/∂v_p)` at grid `vg`,
    /// screen `vg2` and plate `vp` (all against the cathode).
    #[inline]
    pub fn current(&self, vg: f64, vg2: f64, vp: f64) -> (f64, f64, f64) {
        let vg2 = vg2.max(1.0);
        let (sp, sig) = softplus(self.kp * (1.0 / self.mu + vg / vg2));
        let e1 = vg2 / self.kp * sp;
        if e1 <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let pw = fast_exp((self.ex - 1.0) * fast_ln(e1));
        let base = 2.0 / self.kg1 * pw * e1;
        let dbase = 2.0 * self.ex / self.kg1 * pw * sig;
        let norm = 1.0 / atan(250.0 / self.kvb);
        // Smooth through zero (odd): a plate driven past its cathode by a solve's step
        // finds its way back, where a hard floor there would trap Newton in a two-cycle.
        let x = vp / self.kvb;
        let f = if x < 0.0 { -atan(-x) } else { atan(x) } * norm;
        let df = norm / (self.kvb * (1.0 + x * x));
        (base * f, dbase * f, base * df)
    }

    /// The grid voltage that sets `amps` of plate current at screen `vg2` and plate `vp`
    /// (the bias a technician sets: bisection, off the audio thread).
    pub fn bias_for(&self, amps: f64, vg2: f64, vp: f64) -> f64 {
        let (mut lo, mut hi) = (-200.0f64, 0.0f64);
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if self.current(mid, vg2, vp).0 > amps {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        0.5 * (lo + hi)
    }
}

/// `atan x` for `x ≥ 0` (odd: the caller folds the sign), our own (a degree-11 odd polynomial on [0, 1], folded through
/// `π/2 − atan(1/x)` above; error under 2·10⁻⁸ rad): the plate's factor needs no more.
#[inline]
pub fn atan(x: f64) -> f64 {
    let (t, flip) = if x > 1.0 { (1.0 / x, true) } else { (x, false) };
    let z = t * t;
    let mut p = -0.011_721_2;
    p = p * z + 0.052_653_32;
    p = p * z - 0.116_432_87;
    p = p * z + 0.193_543_46;
    p = p * z - 0.332_623_47;
    p = p * z + 0.999_977_26;
    let a = t * p;
    if flip {
        std::f64::consts::FRAC_PI_2 - a
    } else {
        a
    }
}

/// A triode's plate resistance and transconductance at `(v_gk, v_pk)`: `(r_p, g_m)`.
pub fn triode_small_signal(t: &Koren, vgk: f64, vpk: f64) -> (f64, f64) {
    let (_, _, _, gm, gp) = t.currents(vgk, vpk);
    (1.0 / gp.max(1e-12), gm)
}
