//! Vinculum's laws: the three models' static curves and the ballistics they share.
//!
//! Each law is a pure function of a level, written so that a test can solve the same
//! relation independently (in `f64`, by bisection) and compare.
//!
//! - **VCA** ([`vca_gr_db`]): the feed-forward gain computer of Giannoulis, Massberg and
//!   Reiss, *Digital Dynamic Range Compressor Design — A Tutorial and Analysis* (JAES 60:6,
//!   2012), eq. 4 — a threshold, a ratio and a soft knee of width `W` dB, in the log
//!   domain. The reduction is `x − y(x)` dB.
//! - **Opto** ([`OptoLaw`]): a T4-style cell. The electroluminescent panel's light rises
//!   as a power of its drive above a turn-on (`c·(u − u₀)^p`, `u` the output level over
//!   the threshold); the photoresistor's conductance follows the light; the cell is the
//!   shunt arm of a divider, so the gain is `1 / (1 + G)`. Because the panel is driven
//!   from the **output** (a feedback design), the static curve is explicit backwards — the
//!   input is `y·(1 + G(y))` — and its local ratio tends to `1 + p` as the reduction
//!   deepens, with a knee as soft as the panel's turn-on.
//!   The loop is solved at its equilibrium ([`OptoEquilibrium`]) and the cell moves toward
//!   it at the published times of the classic electro-optical leveller and its CdS cell
//!   ([`OptoCell`]: about 10 ms attack, half released in about 60 ms, the rest over
//!   0.5 – 15 s with the cell's memory; the sources in `vinculum_tests_opto.rs`).
//! - **Vari-mu** ([`MuLaw`]): a remote-cutoff valve whose gain falls with its grid bias;
//!   the bias is the rectified **output** over a threshold (a feedback design again), so
//!   the input is the output plus `GR(o)` dB, `o` the output over the threshold. For a
//!   gain falling exponentially with bias the local ratio is `R₀ + K·GR`: **the ratio
//!   rises with the reduction**, the defining trait of the vari-mu. The limit setting
//!   steepens that slope to the published 4 : 1 → 20 : 1 over 12 dB, and once the ratio
//!   reaches its ceiling the law holds it there.
//!
//! [`Dual`] is the ballistics: a smooth branching one-pole (Giannoulis eq. 17) with an
//! optional second, slower stage that charges only while the reduction lasts and holds a
//! share of it as it lets go — a lone transient comes back at the fast speed, sustained
//! work at the slow one (the two-capacitor release of the bus compressors' auto release,
//! the vari-mu's programme-dependent time constants).

use crate::math;

/// `ln(10) / 20`: a dB slope to a natural-log slope.
pub const K: f32 = std::f32::consts::LN_10 / 20.0;
/// The deepest reduction any law gives, dB.
pub const MAX_GR_DB: f32 = 40.0;

/// The Giannoulis gain computer (eq. 4): the output level, dB, for an input level `x` dB,
/// threshold `t` dB, ratio `r`, knee width `w` dB.
#[inline]
pub fn vca_curve(x: f32, t: f32, r: f32, w: f32) -> f32 {
    let over = x - t;
    if 2.0 * over < -w {
        x
    } else if w > 0.0 && 2.0 * over <= w {
        let q = over + 0.5 * w;
        x + (1.0 / r - 1.0) * q * q / (2.0 * w)
    } else {
        t + over / r
    }
}

/// The VCA's reduction, dB (≥ 0): exactly 0 below the knee.
#[inline]
pub fn vca_gr_db(x: f32, t: f32, r: f32, w: f32) -> f32 {
    if 2.0 * (x - t) < -w {
        return 0.0;
    }
    (x - vca_curve(x, t, r, w)).max(0.0)
}

/// The opto cell's light law.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OptoLaw {
    /// The panel's turn-on, as a share of the threshold.
    pub u0: f32,
    /// The power of the light over its drive: the ratio tends to `1 + p`.
    pub p: i32,
    /// The conductance at the threshold is `c·(1 − u₀)^p`.
    pub c: f32,
}

/// The conductance the opto cell shows at the threshold: 1 dB of reduction.
const OPTO_G_AT_T: f32 = 0.122_018_45; // 10^(1/20) − 1

impl OptoLaw {
    fn with(p: i32, u0: f32) -> OptoLaw {
        let c = OPTO_G_AT_T / math::powi(1.0 - u0, p);
        OptoLaw { u0, p, c }
    }

    /// Compress: a 3 : 1 asymptote, the panel lighting from 6 dB under the threshold.
    pub fn compress() -> OptoLaw {
        OptoLaw::with(2, 0.5)
    }

    /// Limit: about 10 : 1 where it works (the local ratio is
    /// `1 + p·u/(u − u₀)·G/(1 + G)`, which with `p = 8` measures 9.2 : 1 at 20 dB over
    /// the threshold and 9.9 : 1 at 30), the panel lighting from 12 dB under the
    /// threshold.
    pub fn limit() -> OptoLaw {
        OptoLaw::with(8, 0.25)
    }

    /// The conductance the light at drive `u` (output over threshold, linear) settles at.
    #[inline]
    pub fn conductance(&self, u: f32) -> f32 {
        let d = u - self.u0;
        if d <= 0.0 {
            0.0
        } else {
            self.c * math::powi(d, self.p)
        }
    }

    /// The divider's gain for a conductance.
    #[inline]
    pub fn gain(g: f32) -> f32 {
        1.0 / (1.0 + g)
    }

    /// The reduction of a conductance, dB.
    #[inline]
    pub fn gr_db(g: f32) -> f32 {
        if g <= 0.0 {
            0.0
        } else {
            20.0 * math::log10(1.0 + g)
        }
    }
}

/// The vari-mu valve's law, in dB: the reduction for an output `o` dB over the threshold.
///
/// A remote-cutoff valve whose gain falls exponentially with its bias (`GR = a·v` dB), the
/// bias the rectified output over the threshold (`v = u − 1`), has the local ratio
/// `1 + K·a·u` — which, written with its own reduction, is **`R = R₀ + K·GR`**: a ratio that
/// rises linearly with the reduction, from `R₀ = 1 + K·a` at the threshold. That is the
/// law here, with the slope `s` free (the exponential valve's is `K`) and a ceiling:
///
/// `GR(o) = (R₀ − 1)/s · (e^{s·o} − 1)` until the ratio reaches `R_max`, then a straight
/// line of slope `R_max − 1`. The output level passes a soft knee of `w` dB first (the
/// rectifier's turn-on), so the reduction begins smoothly under the threshold.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct MuLaw {
    /// The ratio at the threshold.
    pub r0: f32,
    /// How fast the ratio rises: ratio per dB of reduction.
    pub s: f32,
    /// The ratio's ceiling.
    pub rmax: f32,
    /// The knee's width, dB.
    pub w: f32,
    /// Where the ceiling is reached: the reduction and the (kneed) output level there.
    gr_c: f32,
    o_c: f32,
}

impl MuLaw {
    fn with(r0: f32, s: f32, rmax: f32, w: f32) -> MuLaw {
        let gr_c = (rmax - r0) / s;
        let o_c = math::ln_1p(s * gr_c / (r0 - 1.0)) / s;
        MuLaw {
            r0,
            s,
            rmax,
            w,
            gr_c,
            o_c,
        }
    }

    /// Compress: the exponential valve — 1.5 : 1 at the threshold, the ratio rising by
    /// `K` (0.115) per dB of reduction (2.9 : 1 at 12 dB), to a 4 : 1 ceiling; a 6 dB knee.
    pub fn compress() -> MuLaw {
        MuLaw::with(1.5, K, 4.0, 6.0)
    }

    /// Limit: 4 : 1 at the threshold, rising to 20 : 1 at 12 dB of reduction, held there;
    /// a 2 dB knee.
    pub fn limit() -> MuLaw {
        MuLaw::with(4.0, 16.0 / 12.0, 20.0, 2.0)
    }

    /// The rectifier's knee: 0 under `−w/2`, the level itself over `w/2`, a parabola
    /// between.
    #[inline]
    pub fn knee(&self, o: f32) -> f32 {
        let h = 0.5 * self.w;
        if o <= -h {
            0.0
        } else if o < h {
            let q = o + h;
            q * q / (2.0 * self.w)
        } else {
            o
        }
    }

    /// The knee's slope.
    #[inline]
    pub fn knee_slope(&self, o: f32) -> f32 {
        let h = 0.5 * self.w;
        if o <= -h {
            0.0
        } else if o < h {
            (o + h) / self.w
        } else {
            1.0
        }
    }

    /// The reduction, dB, for a kneed output level `o` ≥ 0 dB over the threshold.
    #[inline]
    pub fn gr_db(&self, o: f32) -> f32 {
        if o <= 0.0 {
            0.0
        } else if o <= self.o_c {
            ((self.r0 - 1.0) / self.s * math::exp_m1(self.s * o)).min(MAX_GR_DB)
        } else {
            (self.gr_c + (self.rmax - 1.0) * (o - self.o_c)).min(MAX_GR_DB)
        }
    }

    /// The reduction's slope over the kneed level (0 where it is capped).
    #[inline]
    pub fn gr_slope(&self, o: f32) -> f32 {
        if o <= 0.0 {
            self.r0 - 1.0
        } else if self.gr_db(o) >= MAX_GR_DB {
            0.0
        } else if o <= self.o_c {
            (self.r0 - 1.0) * math::exp(self.s * o)
        } else {
            self.rmax - 1.0
        }
    }

    /// The ratio the law gives at a reduction (what the tests hold it to).
    pub fn ratio_at(&self, gr_db: f32) -> f32 {
        (self.r0 + self.s * gr_db).min(self.rmax)
    }

    /// The feedback loop's equilibrium for an input `x` dB over the threshold: the
    /// reduction `c` the output it leaves asks for, `c = GR(knee(x − c))` — the static
    /// curve. `c − GR(knee(x − c))` rises with `c`, negative at 0 and positive where the
    /// output falls under the knee, so a bisection finds it to the float.
    pub fn equilibrium(&self, x: f32) -> f32 {
        if x <= -0.5 * self.w {
            return 0.0;
        }
        let (mut lo, mut hi) = (0.0f32, (x + 0.5 * self.w).min(MAX_GR_DB));
        for _ in 0..64 {
            let m = 0.5 * (lo + hi);
            if m - self.gr_db(self.knee(x - m)) < 0.0 {
                lo = m;
            } else {
                hi = m;
            }
        }
        0.5 * (lo + hi)
    }
}

/// Intervals in the vari-mu's equilibrium table.
pub const MU_TABLE_N: usize = 256;

/// The vari-mu loop's equilibrium ([`MuLaw::equilibrium`]) as a table over the input
/// level, solved once when the unit is made and interpolated per sample. It covers the
/// knee and the rising ratio — from `−w/2` to the input where the ratio reaches its
/// ceiling — and past that the law is a line, so the curve there is closed: the
/// reduction grows by `(R_max − 1)/R_max` dB per dB, to [`MAX_GR_DB`].
#[derive(Clone, Debug, PartialEq)]
pub struct MuEquilibrium {
    x0: f32,
    x_c: f32,
    inv_h: f32,
    gr_c: f32,
    tail: f32,
    c: [f32; MU_TABLE_N + 1],
}

impl MuEquilibrium {
    pub fn new(law: &MuLaw) -> MuEquilibrium {
        // The ceiling is reached past the knee for both characters, so the line beyond
        // it is the whole law there.
        debug_assert!(law.o_c >= 0.5 * law.w);
        let x0 = -0.5 * law.w;
        let x_c = law.o_c + law.gr_c;
        let h = (x_c - x0) / MU_TABLE_N as f32;
        let mut c = [0f32; MU_TABLE_N + 1];
        for (i, v) in c.iter_mut().enumerate().take(MU_TABLE_N).skip(1) {
            *v = law.equilibrium(x0 + h * i as f32);
        }
        c[MU_TABLE_N] = law.gr_c;
        MuEquilibrium {
            x0,
            x_c,
            inv_h: 1.0 / h,
            gr_c: law.gr_c,
            tail: (law.rmax - 1.0) / law.rmax,
            c,
        }
    }

    /// The equilibrium reduction, dB, for an input `x` dB over the threshold: exactly 0
    /// under the knee.
    #[inline]
    pub fn at(&self, x: f32) -> f32 {
        if x <= self.x0 {
            0.0
        } else if x >= self.x_c {
            (self.gr_c + self.tail * (x - self.x_c)).min(MAX_GR_DB)
        } else {
            let p = (x - self.x0) * self.inv_h;
            let i = (p as usize).min(MU_TABLE_N - 1);
            let f = p - i as f32;
            self.c[i] + (self.c[i + 1] - self.c[i]) * f
        }
    }
}

/// A one-pole coefficient for a time constant of `ms` (63 % of a step in that time).
#[inline]
pub fn coef(ms: f32, sr: f32) -> f32 {
    crate::dynfx_follow::coef(ms, sr)
}

/// The ballistics' coefficients.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ballistics {
    pub attack: f32,
    pub release: f32,
    /// The slow stage: how fast it charges toward the fast one, how fast it lets go, and
    /// the share of it that holds (0 = no slow stage).
    pub charge: f32,
    pub slow: f32,
    pub share: f32,
    /// A state under this is exactly 0.
    pub floor: f32,
}

impl Ballistics {
    /// One stage: attack and release in ms.
    pub fn single(attack_ms: f32, release_ms: f32, floor: f32, sr: f32) -> Ballistics {
        Ballistics {
            attack: coef(attack_ms, sr),
            release: coef(release_ms, sr),
            charge: 0.0,
            slow: 0.0,
            share: 0.0,
            floor,
        }
    }

    /// Two stages: a fast release, and a slow stage charging over `charge_ms`, letting go
    /// over `slow_ms`, holding `share` of what it has.
    #[allow(clippy::too_many_arguments)]
    pub fn dual(
        attack_ms: f32,
        release_ms: f32,
        charge_ms: f32,
        slow_ms: f32,
        share: f32,
        floor: f32,
        sr: f32,
    ) -> Ballistics {
        Ballistics {
            attack: coef(attack_ms, sr),
            release: coef(release_ms, sr),
            charge: coef(charge_ms, sr),
            slow: coef(slow_ms, sr),
            share,
            floor,
        }
    }
}

/// The ballistics' state: the fast stage and the slow one.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Dual {
    pub fast: f32,
    pub slow: f32,
}

impl Dual {
    /// The fast stage toward `target` (smooth branching: the attack's speed up, the
    /// release's down, toward the target either way).
    #[inline]
    pub fn fast_step(fast: f32, target: f32, b: &Ballistics) -> f32 {
        let k = if target > fast { b.attack } else { b.release };
        let next = if k >= 1.0 {
            target
        } else {
            fast + (target - fast) * k
        };
        if next < b.floor {
            0.0
        } else {
            next
        }
    }

    /// The slow stage after the fast one has moved, and what the two hold together.
    #[inline]
    pub fn slow_step(&mut self, b: &Ballistics) -> f32 {
        if b.share <= 0.0 {
            return self.fast;
        }
        let k = if self.fast > self.slow {
            b.charge
        } else {
            b.slow
        };
        let next = self.slow + (self.fast - self.slow) * k;
        self.slow = if next < b.floor { 0.0 } else { next };
        self.held(b)
    }

    /// What the two stages hold together.
    #[inline]
    pub fn held(&self, b: &Ballistics) -> f32 {
        if b.share <= 0.0 {
            self.fast
        } else {
            self.fast.max(self.slow * b.share)
        }
    }

    /// One whole step toward `target`.
    #[inline]
    pub fn step(&mut self, target: f32, b: &Ballistics) -> f32 {
        self.fast = Dual::fast_step(self.fast, target, b);
        self.slow_step(b)
    }

    /// Both stages exactly +0.0.
    pub fn is_zero(&self) -> bool {
        self.fast.to_bits() == 0 && self.slow.to_bits() == 0
    }

    pub fn reset(&mut self) {
        *self = Dual::default();
    }
}

// fx-b:vinculum (opto, the owner's decision of 2026-11-17) — the opto loop's equilibrium
// and the photocell's time, after the published measurements (`vinculum_tests_opto.rs`).

/// Intervals in the opto's equilibrium table.
pub const OPTO_TABLE_N: usize = 512;
/// How far over the threshold (dB of input) the table reaches; past it the law is the
/// line of its asymptotic ratio.
const OPTO_TABLE_TOP_DB: f32 = 64.0;

impl OptoLaw {
    /// The loop's equilibrium for an input `x` dB over the threshold: the reduction, dB,
    /// whose output lights the cell to exactly that reduction — the static curve, where
    /// the input is `y·(1 + G(y))`. That input rises with `y`, so a bisection over the
    /// output finds it to the float. Exactly 0 where the panel stays dark (`x ≤ u₀`); not
    /// capped at [`MAX_GR_DB`] (the table interpolates across the cap without a kink and
    /// caps what it reads).
    pub fn equilibrium(&self, x_db: f32) -> f32 {
        let x = math::db_to_gain(x_db);
        if x <= self.u0 {
            return 0.0;
        }
        let (mut lo, mut hi) = (self.u0, x);
        for _ in 0..64 {
            let m = 0.5 * (lo + hi);
            if m * (1.0 + self.conductance(m)) < x {
                lo = m;
            } else {
                hi = m;
            }
        }
        (20.0 * math::log10(x / (0.5 * (lo + hi)))).max(0.0)
    }
}

/// The opto loop's equilibrium ([`OptoLaw::equilibrium`]) as a table over the input level
/// (dB over the threshold), solved once when the unit is made and interpolated per sample:
/// from the panel's turn-on to [`OPTO_TABLE_TOP_DB`] (with the threshold at its lowest,
/// −50 dBFS, an input of +14 dBFS), then the table's last slope — by then within a
/// hundredth of the cell's asymptotic `p/(p + 1)` dB per dB (the ratio `1 + p`) — to
/// [`MAX_GR_DB`].
#[derive(Clone, Debug, PartialEq)]
pub struct OptoEquilibrium {
    /// The panel's turn-on: under it (as a mean square over the threshold's) the cell is
    /// dark.
    pub on_ms: f32,
    x0: f32,
    inv_h: f32,
    top: f32,
    tail: f32,
    c: [f32; OPTO_TABLE_N + 1],
}

impl OptoEquilibrium {
    pub fn new(law: &OptoLaw) -> OptoEquilibrium {
        let x0 = 20.0 * math::log10(law.u0);
        let h = (OPTO_TABLE_TOP_DB - x0) / OPTO_TABLE_N as f32;
        let mut c = [0f32; OPTO_TABLE_N + 1];
        for (i, v) in c.iter_mut().enumerate().skip(1) {
            *v = law.equilibrium(x0 + h * i as f32);
        }
        OptoEquilibrium {
            on_ms: law.u0 * law.u0,
            x0,
            inv_h: 1.0 / h,
            top: c[OPTO_TABLE_N],
            tail: (c[OPTO_TABLE_N] - c[OPTO_TABLE_N - 1]) / h,
            c,
        }
    }

    /// The equilibrium reduction, dB, for an input `x` dB over the threshold: exactly 0
    /// under the panel's turn-on.
    #[inline]
    pub fn at(&self, x: f32) -> f32 {
        if x <= self.x0 {
            0.0
        } else if x >= OPTO_TABLE_TOP_DB {
            (self.top + self.tail * (x - OPTO_TABLE_TOP_DB)).min(MAX_GR_DB)
        } else {
            let p = (x - self.x0) * self.inv_h;
            let i = (p as usize).min(OPTO_TABLE_N - 1);
            let f = p - i as f32;
            (self.c[i] + (self.c[i + 1] - self.c[i]) * f).min(MAX_GR_DB)
        }
    }
}

/// Intervals in [`OptoTimes::slow`].
pub const OPTO_SLOW_N: usize = 32;

/// The photocell's time, as coefficients for one sample rate (PE, NMHR: the sources named in
/// `vinculum_tests_opto.rs`).
///
/// The cell is three states in the reduction's dB (the CdS cell's resistance is a
/// power of its light, a straight line in log–log, PE p. 6, so its decibels move
/// as its light does): a **fast** one — the free carriers — rising toward the loop's
/// equilibrium at an attack that quickens as the cell is lit harder (PE p. 7,
/// NMHR §1) and falling at the first stage's release; a **slow** one — the trapped
/// carriers — that charges toward the fast one while the cell is lit and lets go at a
/// time set by the **memory**; and the memory itself, which fills toward the slow
/// stage's depth (how hard) at its own pace (how long) and empties once the cell is dark
/// (a rested cell recovers fast, PE p. 7: "this effect is reversible"). The cell holds
/// the larger of the fast stage and a share of the slow one; the share is under a half,
/// so the first half of every release is the fast stage's at any depth.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OptoTimes {
    /// The attack's coefficients dim and fully lit.
    pub attack_dim: f32,
    pub attack_lit: f32,
    /// The first stage's release.
    pub release: f32,
    /// The slow stage's charge.
    pub charge: f32,
    /// The slow stage's release by memory, at memories `0, 1/32, … 1`, interpolated: the
    /// coefficient `1/(τ·sr)` with `τ` geometric from fresh to full (τ ≥ 0.5 s, so the
    /// exact `1 − e^{−1/(τ·sr)}` differs from it by under 3·10⁻⁵ of itself). A table, so
    /// the sample loop runs no exponential (the first pass's `e^{−m·ln(τ₁/τ₀)}` per sample
    /// cost 0.1 % of a core).
    pub slow: [f32; OPTO_SLOW_N + 1],
    /// The share of the slow stage the cell holds.
    pub share: f32,
    /// The memory's fill and rest.
    pub fill: f32,
    pub rest: f32,
    /// `1 / (the reduction, dB, at which the cell is fully lit and the memory full)`.
    pub inv_full_db: f32,
    /// A state under this is exactly 0.
    pub floor: f32,
}

/// The photocell's state ([`OptoTimes`]): the fast and slow stages, dB, and the memory,
/// 0 … 1.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OptoCell {
    pub fast: f32,
    pub slow: f32,
    pub memory: f32,
}

impl OptoCell {
    /// One sample toward the loop's equilibrium `target` (dB); what the cell holds.
    #[inline]
    pub fn step(&mut self, target: f32, t: &OptoTimes) -> f32 {
        let k = if target > self.fast {
            let lit = (target * t.inv_full_db).min(1.0);
            t.attack_dim + (t.attack_lit - t.attack_dim) * lit
        } else {
            t.release
        };
        let fast = self.fast + (target - self.fast) * k;
        self.fast = if fast < t.floor { 0.0 } else { fast };
        if self.fast > self.slow {
            self.slow += (self.fast - self.slow) * t.charge;
        } else if self.slow > 0.0 {
            let p = self.memory * OPTO_SLOW_N as f32;
            let i = (p as usize).min(OPTO_SLOW_N - 1);
            let k = t.slow[i] + (t.slow[i + 1] - t.slow[i]) * (p - i as f32);
            let slow = self.slow + (self.fast - self.slow) * k;
            self.slow = if slow < t.floor { 0.0 } else { slow };
        }
        let depth = (self.slow * t.inv_full_db).min(1.0);
        if depth != self.memory {
            let k = if depth > self.memory { t.fill } else { t.rest };
            let m = self.memory + (depth - self.memory) * k;
            self.memory = if m < t.floor { 0.0 } else { m };
        }
        self.held(t)
    }

    /// What the cell holds: the fast stage, or the share of the slow one if larger.
    #[inline]
    pub fn held(&self, t: &OptoTimes) -> f32 {
        self.fast.max(self.slow * t.share)
    }

    /// Every state exactly +0.0.
    pub fn is_zero(&self) -> bool {
        self.fast.to_bits() == 0 && self.slow.to_bits() == 0 && self.memory.to_bits() == 0
    }

    pub fn reset(&mut self) {
        *self = OptoCell::default();
    }
}
