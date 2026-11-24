//! Arcus's bowed string (a child of `arcus`): a waveguide in velocity waves, in physical
//! units, with the bow as a friction junction.
//!
//! **The topology** is Fides's waveguide (`fides::string`): two loops meet at the bow — the
//! bridge loop (`β·P` samples, the loss lumped at the bridge) and the neck loop (the rest of
//! the period `P`, ending at the finger) — on Gravis's tuning maths (`gravis::string`'s public
//! `lagrange`, `lagrange_delay`, `loss_design`, `loss_delay`), so the fundamental of the
//! free string is exact at every control step.
//!
//! **The bow** is the junction of McIntyre, Schumacher & Woodhouse 1983 (doi:10.1121/1.390157)
//! in the form of Smith's waveguide bow (Smith, *Physical Audio Signal Processing*, "The
//! Bow-String Scattering Junction"), solved in **physical units**: the waves are string
//! velocities (m/s), the bow presses with a normal force `F` (N) and moves at `v_b` (m/s),
//! the string's characteristic impedance is `Z₀` (kg/s). With `v_h` the velocity the two
//! arriving waves give the string at the bow, the string's velocity there is
//! `v = v_h + f/(2Z₀)`, where the friction force `f = F·μ(v_b − v)`. Writing `R = F/(2Z₀)`
//! and `u = v_b − v_h`, the junction solves `Δ + R·μ(Δ) = u` for the slip `Δ = v_b − v`:
//!
//! - **sticking** while `|u| ≤ R·μs` (the bow holds the string: `Δ = 0`);
//! - **slipping** otherwise. With the hyperbolic curve `μ = μd + (μs − μd)·v₀/(v₀ + |Δ|)`
//!   the equation is a quadratic in `|Δ|`, solved in closed form — no iteration, the same
//!   bits everywhere: `|Δ|² + (v₀ + Rμd − |u|)|Δ| + v₀(Rμs − |u|) = 0`. In the band where it
//!   has two roots and sticking is also possible, the state decides (hysteresis, the
//!   Friedlander construction's rule): a slipping string keeps slipping on the larger root, a
//!   sticking one keeps sticking.
//! - **The thermal option** (r42:arcus-p4: Woodhouse & Galluzzo 2025, *Tribol. Lett.* 73:128,
//!   doi:10.1007/s11249-025-02062-4, CC BY — the "enhanced" rate-and-state law): the friction
//!   is `μ = h(Δ)·y(T)`, the jump curve `h(Δ) = 0.5(|Δ| + 0.27)^−0.4` scaled by the rosin's
//!   yield at the contact temperature `T` over ambient, `y(T) = Y(T)/Y(T₀)`, fixed by the
//!   paper's steady-sliding fit `μ_steady = 0.2(|v| + 0.011)^−0.4` (`T₀` 26.2 °C where the two
//!   curves cross). `T` is heated by the power the friction dissipates and relaxes through two
//!   poles (a fast one inside the period, a slow one), their steady sum `κ·F·μ·|v|` with `κ`
//!   set so steady sliding at 1 N lands on the paper's `T₀`; the two poles stand in for the
//!   paper's conduction (its eq. 5, not reconstructed here — said so in the report). The slip
//!   solves `Δ + R·y·h(Δ) = |u|` by Newton from `|u|` (a convex function: monotone onto the
//!   larger root), a fixed eight steps — the same bits everywhere.
//!
//! What the bridge hears is the wave arriving there; its force `2·Z₀·v` drives the body.

use super::tables::{MU_D, MU_S, V0};
use crate::gravis::string::{lagrange, lagrange_delay, loss_delay, loss_design};
use crate::math;

/// The lowest fundamental the lines hold, Hz (a bass's E1 bent and vibrated down).
pub const MIN_HZ: f64 = 25.0;
/// r42 fix2-arcus — the shortest loop, samples: the highest fundamental a rate allows is
/// `rate / SHORTEST` ([`BowedString::max_hz`]).
pub const SHORTEST: f64 = 12.0;
/// A sample below this is silence for the gate.
const QUIET: f32 = 1e-8;
/// The DC blocker's corner, as a fraction of the fundamental (Fides's, `fides::string`).
const DC_CORNER: f64 = 1.0 / 64.0;
/// Samples past the lowest period: the Lagrange taps' reach and the blocker's lead.
const LINE_SPARE: usize = 48;
/// The thermal contact's cooling time, s, and its steady heating, per W.
// r42:arcus-p4: the thermal law's fast pole (s) and a scale on its calibrated heating (1:
// the paper's `T₀` at 1 N), the slow pole and the share each takes.
const THERMAL_TAU_S: f32 = 0.0005;
const THERMAL_PER_W: f32 = 1.0;
const THERMAL_SLOW_S: f32 = 0.02;
const THERMAL_FAST_SHARE: f32 = 0.6;
/// The jump curve `h(Δ) = WG_A(|Δ| + WG_B)^−0.4` and the steady fit `WG_S(|v| + WG_C)^−0.4`.
const WG_A: f32 = 0.5;
const WG_B: f32 = 0.27;
const WG_S: f32 = 0.2;
const WG_C: f32 = 0.011;
/// The yield table's points.
const WG_N: usize = 64;
/// Newton's steps on the slip (fixed: the same bits everywhere).
const WG_STEPS: usize = 8;

/// r42:arcus-p4: the rosin's yield against temperature, `y(T) = Y(T)/Y(T₀)`, tabulated from
/// the steady-sliding fit (`T(v) = κ·1 N·μ_steady(v)·v`, `y = μ_steady(v)/h(v)`, `v` 0 … 4 m/s
/// spaced by its square): the heating `κ` (K/W) and the table.
#[derive(Clone, Debug)]
pub struct ThermalLaw {
    pub kappa: f32,
    t: [f32; WG_N],
    y: [f32; WG_N],
}

impl ThermalLaw {
    fn jump(x: f32) -> f32 {
        WG_A * math::powf(x.abs() + WG_B, -0.4)
    }

    fn steady(v: f32) -> f32 {
        WG_S * math::powf(v.abs() + WG_C, -0.4)
    }

    pub fn new() -> Self {
        // `T₀` where the steady and jump curves cross: bisection on their difference.
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..40 {
            let m = 0.5 * (lo + hi);
            if Self::steady(m) > Self::jump(m) {
                lo = m;
            } else {
                hi = m;
            }
        }
        let vx = 0.5 * (lo + hi);
        let kappa = 26.2 / (Self::steady(vx) * vx);
        let (mut t, mut y) = ([0.0; WG_N], [0.0; WG_N]);
        for i in 0..WG_N {
            let u = i as f32 / (WG_N - 1) as f32;
            let v = 4.0 * u * u;
            t[i] = kappa * Self::steady(v) * v;
            y[i] = Self::steady(v) / Self::jump(v);
        }
        ThermalLaw { kappa, t, y }
    }

    /// `y(T)`, linear between the table's points, held at its ends.
    pub fn yield_at(&self, temp: f32) -> f32 {
        if temp <= self.t[0] {
            return self.y[0];
        }
        if temp >= self.t[WG_N - 1] {
            return self.y[WG_N - 1];
        }
        let (mut a, mut b) = (0usize, WG_N - 1);
        while b - a > 1 {
            let m = (a + b) / 2;
            if self.t[m] <= temp {
                a = m;
            } else {
                b = m;
            }
        }
        let f = (temp - self.t[a]) / (self.t[b] - self.t[a]);
        self.y[a] + (self.y[b] - self.y[a]) * f
    }

    /// The slip of `Δ + rs·h(Δ) = |u|` (`rs` the force term times the yield): the larger root,
    /// and the margin by which roots exist (`≥ 0`: they do; its sign change times a slip's end).
    pub fn slip(&self, abs_u: f32, rs: f32) -> (Option<f32>, f32) {
        let g = |x: f32| x + rs * Self::jump(x) - abs_u;
        // `g′ = 1 − 0.4·rs·WG_A·(x + WG_B)^−1.4`: its zero is the curve's lowest point.
        let xm = math::powf(0.4 * rs * WG_A, 1.0 / 1.4) - WG_B;
        let margin = if xm > 0.0 { -g(xm) } else { -g(0.0) };
        if margin < 0.0 {
            return (None, margin);
        }
        let floor = xm.max(0.0);
        let mut x = abs_u;
        for _ in 0..WG_STEPS {
            let d = 1.0 - 0.4 * rs * WG_A * math::powf(x + WG_B, -1.4);
            if d <= 1e-6 {
                break;
            }
            x = (x - g(x) / d).max(floor);
        }
        ((x > 0.0).then_some(x), margin)
    }
}

impl Default for ThermalLaw {
    fn default() -> Self {
        Self::new()
    }
}

/// Which friction law the bow meets the string with, in `bow.friction`'s order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Friction {
    #[default]
    Hyperbolic,
    Thermal,
}

impl Friction {
    pub fn from_index(i: f32) -> Friction {
        if i >= 0.5 {
            Friction::Thermal
        } else {
            Friction::Hyperbolic
        }
    }
}

/// The string's torsion as the bow meets it: the bow drags the string's surface, which
/// moves by the string's translation and its twist, so a sticking bow is not a rigid end for
/// the transverse waves — the twist yields, and its waves (fast, and heavily damped by the
/// string's own losses) carry away what would ring between the bow and the bridge. After
/// Woodhouse & Loach's measurements of torsional waves on violin strings (*Acustica* 85,
/// 1999: torsional Q in the tens, a wave speed several times the transverse — NV, from the
/// literature's summary) and Pitteroff & Woodhouse's bowed-string model with torsion
/// (*Acustica* 84, 1998, NV): `ratio` is the torsional speed over the transverse,
/// `impedance` the torsional impedance at the surface over the transverse, `q` its Q. The
/// defaults (5, 3, 5) were chosen by measurement (`tests_gate::probe_torsion`, the cello's
/// open D): with no torsion the Helmholtz region at 10 cm/s spans 0.3–0.85 of Schelleng's
/// upper force and 8 of 48 attacks settle within 50 ms; with these it spans 0.15–1.0 (the
/// "1 to 10" Schelleng names) and 15 of 48 do, along a Guettler wedge — a heavily damped
/// twist, its Q under the literature's tens (said so in `docs/round42/arcus.md`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Torsion {
    pub ratio: f64,
    pub impedance: f32,
    pub q: f32,
}

impl Default for Torsion {
    fn default() -> Self {
        Torsion {
            ratio: 5.0,
            impedance: 3.0,
            q: 5.0,
        }
    }
}

/// What [`BowedString::design`] needs each control step.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct StringStep {
    /// The loop's fundamental, Hz (the pitch correction already in it).
    pub f0: f64,
    /// T60 of the fundamental, s, and the material's exponent above it.
    pub t60: f64,
    pub p: f64,
    /// The bow's (or the pluck's) distance from the bridge, a fraction of the stopped length.
    pub beta: f64,
    /// The loop's DC blocker in use: always but while a bow is on the string (Fides's rule:
    /// its phase lead would carry the bowed pitch flat).
    pub dc: bool,
    /// The bridge's corner rounding, Hz (a one-pole low-pass on the reflection; 0: none):
    /// the high-frequency loss of the bridge and the bow's width that a decay time alone does
    /// not give, which rounds the Helmholtz corner and damps the ripple between bow and
    /// bridge (Woodhouse's reflection function; Schoonderwaldt 2008's lower limit).
    pub corner_hz: f64,
}

/// One bowed string.
#[derive(Clone, Debug)]
pub struct BowedString {
    sr: f64,
    neck: Vec<f32>,
    bridge: Vec<f32>,
    wn: usize,
    wb: usize,
    dn: f64,
    db: f64,
    dn_step: f64,
    db_step: f64,
    /// The loss filter `g(1 − a)/(1 − a z⁻¹)` and its state.
    g1a: f32,
    a: f32,
    lz: f32,
    /// The corner's one-pole: its step and state.
    cr_c: f32,
    cr: f32,
    dc_on: bool,
    dc_r: f32,
    dc_k: f32,
    dc: (f32, f32),
    /// The string's impedance now, kg/s, and `1/(2Z₀)`.
    z0: f32,
    half_y: f32,
    torsion: Option<Torsion>,
    tor_n: Vec<f32>,
    tor_b: Vec<f32>,
    wtn: usize,
    wtb: usize,
    dtn: f32,
    dtb: f32,
    tor_g: f32,
    tor_share: f32,
    tor_live: bool,
    sticking: bool,
    /// The last sample's excess of the bow's pull over what the stick holds (`|u| − Rμs`).
    excess: f32,
    /// … the slip equation's discriminant then, and the slip.
    disc: f32,
    last_slip: f32,
    curve: Curve,
    theta: f32,
    th_keep: f32,
    th_gain: f32,
    // r42:arcus-p4: the slow pole's temperature, its keep and gain; the law.
    theta2: f32,
    th2_keep: f32,
    th2_gain: f32,
    law: ThermalLaw,
    quiet: u32,
    live: bool,
    /// The highest fundamental this rate allows.
    pub max_hz: f64,
    /// The lowest its lines hold at this rate (the rate over the period they were built to
    /// hold). // r42:arcus-upper
    min_hz: f64, // r42:arcus-upper
}

/// A friction curve's constants: static and dynamic coefficients, and the hyperbolic law's
/// speed scale (m/s); for the thermal option, the contact's cooling time (s) and its steady
/// heating per watt.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Curve {
    pub mu_s: f32,
    pub mu_d: f32,
    pub v0: f32,
    pub tau_s: f32,
    pub per_w: f32,
}

impl Default for Curve {
    fn default() -> Self {
        Curve {
            mu_s: MU_S,
            mu_d: MU_D,
            v0: V0,
            tau_s: THERMAL_TAU_S,
            per_w: THERMAL_PER_W,
        }
    }
}

impl Curve {
    /// The friction a bow at relative slip `|Δ|` meets, on the hyperbolic curve.
    #[inline]
    pub fn hyperbolic(&self, slip: f32) -> f32 {
        self.mu_d + (self.mu_s - self.mu_d) * self.v0 / (self.v0 + slip.abs())
    }
}

/// The hyperbolic junction's slip for `|u| > 0` and `r = F/(2Z₀)`: the larger root of
/// `x² + (v₀ + r·μd − |u|)x + v₀(r·μs − |u|) = 0`, or `None` when it has no positive root.
/// Computed without cancellation (`2c / (−b − √disc)` when `b > 0`).
#[inline]
pub fn slip_root(k: &Curve, abs_u: f32, r: f32) -> Option<f32> {
    let b = k.v0 + r * k.mu_d - abs_u;
    let c = k.v0 * (r * k.mu_s - abs_u);
    let disc = b * b - 4.0 * c;
    if disc < 0.0 {
        return None;
    }
    let sq = disc.sqrt();
    let x = if b <= 0.0 {
        0.5 * (sq - b)
    } else if c < 0.0 {
        // One positive root, `(−b + √disc)/2`, written as `−2c/(b + √disc)`.
        -2.0 * c / (b + sq)
    } else {
        // b > 0 and c ≥ 0: both roots ≤ 0.
        return None;
    };
    (x > 0.0).then_some(x)
}

impl BowedString {
    pub fn new(sample_rate: f32) -> Self {
        Self::with_reach(sample_rate, (sample_rate as f64 / MIN_HZ).ceil() as usize)
    }

    /// r42 fix2-arcus — a string whose lines hold a period of `held` samples (at whatever
    /// rate it later runs: the lowest it plays is `rate / held`). Allocates: never on the
    /// audio thread.
    pub fn with_reach(sample_rate: f32, held: usize) -> Self {
        let sr = sample_rate as f64;
        let held = held.max(64);
        let longest = held + LINE_SPARE;
        BowedString {
            sr,
            neck: vec![0.0; longest],
            bridge: vec![0.0; longest / 2 + 8],
            wn: 0,
            wb: 0,
            dn: 8.0,
            db: 4.0,
            dn_step: 0.0,
            db_step: 0.0,
            g1a: 0.0,
            a: 0.0,
            lz: 0.0,
            cr_c: 1.0,
            cr: 0.0,
            dc_on: false,
            dc_r: 0.0,
            dc_k: 1.0,
            dc: (0.0, 0.0),
            z0: 1.0,
            half_y: 0.5,
            torsion: Some(Torsion::default()),
            tor_n: vec![0.0; longest / 2 + 8],
            tor_b: vec![0.0; longest / 4 + 8],
            wtn: 0,
            wtb: 0,
            dtn: 2.0,
            dtb: 1.0,
            tor_g: 0.0,
            tor_share: 1.0,
            tor_live: false,
            sticking: false,
            excess: 0.0,
            disc: 0.0,
            last_slip: 0.0,
            curve: Curve::default(),
            theta: 0.0,
            th_keep: 0.0,
            th_gain: 0.0,
            theta2: 0.0,
            th2_keep: 0.0,
            th2_gain: 0.0,
            law: ThermalLaw::new(),
            quiet: 0,
            live: false,
            max_hz: sr / SHORTEST,
            min_hz: sr / held as f64, // r42 fix2-arcus: what the lines hold
        }
        .with_curve(Curve::default())
    }

    /// Run at `sample_rate` from now on (the upper strings' voices at `os ×` the rate it was
    /// built for), its waves let go. No allocation: the lines keep their length, so the
    /// lowest fundamental they hold rises with the rate (a voice sizes them for every rate
    /// it runs at: `voice::reach`). // r42:arcus-upper
    pub fn set_rate(&mut self, sample_rate: f32) {
        self.sr = sample_rate as f64;
        self.max_hz = self.sr / SHORTEST;
        let held = (self.neck.len() - LINE_SPARE) as f64;
        self.min_hz = self.sr / held;
        self.set_curve(self.curve);
        self.clear();
    }

    fn with_curve(mut self, k: Curve) -> Self {
        self.set_curve(k);
        self
    }

    /// The friction curve's constants (no allocation; tests and the tool that measures).
    pub fn set_curve(&mut self, k: Curve) {
        self.curve = k;
        // r42:arcus-p4: two poles, their steady sum `κ·per_w` per watt.
        let kappa = self.law.kappa * k.per_w;
        self.th_keep = math::exp(-1.0 / (k.tau_s.max(1e-5) * self.sr as f32));
        self.th_gain = (1.0 - self.th_keep) * kappa * THERMAL_FAST_SHARE;
        self.th2_keep = math::exp(-1.0 / (THERMAL_SLOW_S * self.sr as f32));
        self.th2_gain = (1.0 - self.th2_keep) * kappa * (1.0 - THERMAL_FAST_SHARE);
    }

    pub fn is_live(&self) -> bool {
        self.live
    }

    /// Whether the bow holds the string now (for the editor and tests).
    pub fn is_sticking(&self) -> bool {
        self.sticking
    }

    /// The contact's temperature (the thermal option's state).
    pub fn temperature(&self) -> f32 {
        self.theta + self.theta2
    }

    pub fn clear(&mut self) {
        if self.live || self.lz != 0.0 {
            self.neck.fill(0.0);
            self.bridge.fill(0.0);
        }
        self.lz = 0.0;
        self.cr = 0.0;
        self.dc = (0.0, 0.0);
        if self.tor_live {
            self.tor_n.fill(0.0);
            self.tor_b.fill(0.0);
            self.tor_live = false;
        }
        self.quiet = 0;
        self.live = false;
        self.sticking = false;
        self.excess = 0.0;
        self.disc = 0.0;
        self.last_slip = 0.0;
        self.theta = 0.0;
        self.theta2 = 0.0;
    }

    /// A note starts on it with string impedance `z0`; `keep`: its waves ring on (a re-bow,
    /// a legato finger, a pluck on a ringing string).
    pub fn start(&mut self, z0: f32, step: &StringStep, keep: bool) {
        if !keep {
            self.clear();
        }
        self.set_z0(z0);
        self.design(step, 0);
        self.live = true;
    }

    /// The string under the bow changed (a legato across strings): its impedance.
    pub fn set_z0(&mut self, z0: f32) {
        self.z0 = z0.max(1e-3);
        match self.torsion {
            Some(t) => {
                let zr = self.z0 * t.impedance.max(0.05);
                let z_eff = self.z0 * zr / (self.z0 + zr);
                self.half_y = 0.5 / z_eff;
                self.tor_share = z_eff / self.z0;
                self.tor_g = math::exp(-std::f32::consts::PI / (2.0 * t.q.max(0.5)));
            }
            None => {
                self.half_y = 0.5 / self.z0;
                self.tor_share = 1.0;
            }
        }
    }

    /// The torsion the bow meets (`None`: a rigid contact, the classic point bow).
    pub fn set_torsion(&mut self, t: Option<Torsion>) {
        self.torsion = t;
        if t.is_none() {
            self.tor_n.fill(0.0);
            self.tor_b.fill(0.0);
            self.tor_live = false;
        }
        self.set_z0(self.z0);
    }

    pub fn z0(&self) -> f32 {
        self.z0
    }

    /// r42 fix2-arcus — the lowest fundamental its lines hold at its rate now, Hz.
    pub fn min_hz(&self) -> f64 {
        self.min_hz
    }

    /// The period now, in samples.
    pub fn period(&self) -> f64 {
        self.dn + self.db
    }

    fn loss(&self, f0: f64, s: &StringStep) -> (f64, f64) {
        let f_hi = (8.0 * f0).min(0.35 * self.sr);
        if f_hi <= 1.5 * f0 {
            let (g, _) = loss_design(f0, s.t60, s.t60, 2.0 * f0, self.sr);
            return (g, 0.0);
        }
        let t_hi = s.t60 * math::powf(f_hi / f0, -s.p);
        loss_design(f0, s.t60, t_hi, f_hi, self.sr)
    }

    /// The step's loss and the two loops' lengths, reached over the next `span` samples.
    pub fn design(&mut self, s: &StringStep, span: u32) {
        let f0 = s.f0.clamp(self.min_hz, self.max_hz); // r42:arcus-upper
        let (g, a) = self.loss(f0, s);
        let w0 = std::f64::consts::TAU * f0 / self.sr;
        let (r, dc_delay, dc_gain) = if s.dc {
            let r = math::exp(-std::f64::consts::TAU * f0 * DC_CORNER / self.sr);
            let (d, gain) = dc_response(r, w0);
            (r, d, gain)
        } else {
            (1.0, 0.0, 1.0)
        };
        // The corner's one-pole `c/(1 − (1 − c)z⁻¹)`: its delay at f0 counted, its loss there
        // taken back from the loss filter's.
        // Only while the bow is drawn (it rounds the Helmholtz corner the bow makes and keeps
        // its timing between the samples; a plucked or free string keeps its own decay, which
        // a corner near a high fundamental would shorten).
        let (cr_c, cr_delay, cr_gain) =
            if !s.dc && s.corner_hz > 0.0 && s.corner_hz < 0.45 * self.sr {
                let c = 1.0 - math::exp(-std::f64::consts::TAU * s.corner_hz / self.sr);
                let b = 1.0 - c;
                let (sn, cs) = math::sin_cos(w0);
                let gain = c / (1.0 - 2.0 * b * cs + b * b).sqrt();
                let _ = sn;
                (c, loss_delay(b, w0), gain)
            } else {
                (1.0, 0.0, 1.0)
            };
        self.cr_c = cr_c as f32;
        let g = g / cr_gain;
        let g = if s.dc {
            (g / dc_gain).min(0.999_99)
        } else {
            g.min(0.999_99)
        };
        if s.dc && !self.dc_on {
            self.dc = (0.0, 0.0);
        }
        self.dc_on = s.dc;
        self.g1a = (g * (1.0 - a)) as f32;
        self.a = a as f32;
        self.dc_r = r as f32;
        self.dc_k = (0.5 * (1.0 + r)) as f32;
        let period = self.sr / f0;
        let filt = loss_delay(a, w0) + dc_delay + cr_delay;
        let avail = (period - filt).max(4.2);
        let db = (s.beta.clamp(0.0, 0.5) * avail).clamp(2.05, avail - 2.1);
        let mut dn = avail - db;
        for _ in 0..3 {
            let total = lagrange_delay(dn, w0) + lagrange_delay(db, w0);
            dn = (dn + (avail - total)).max(2.05);
        }
        let dn = dn.min(self.neck.len() as f64 - 6.0);
        let db = db.min(self.bridge.len() as f64 - 6.0);
        if let Some(t) = self.torsion {
            let k = t.ratio.max(2.0);
            self.dtn = ((dn / k).max(1.0) as f32).min(self.tor_n.len() as f32 - 3.0);
            self.dtb = ((db / k).max(1.0) as f32).min(self.tor_b.len() as f32 - 3.0);
        }
        if span == 0 {
            self.dn = dn;
            self.db = db;
            self.dn_step = 0.0;
            self.db_step = 0.0;
        } else {
            self.dn_step = (dn - self.dn) / span as f64;
            self.db_step = (db - self.db) / span as f64;
        }
    }

    #[inline]
    fn read(buf: &[f32], w: usize, d: f64) -> f32 {
        let (m0, h) = lagrange(d);
        let len = buf.len();
        let mut y = 0.0f32;
        for (j, hj) in h.iter().enumerate() {
            let back = m0 + j;
            let idx = if w >= back { w - back } else { w + len - back };
            y += hj * buf[idx];
        }
        y
    }
}

/// A line read `d` samples back, linearly between two (the torsion's lines: no pitch rides on
/// them).
#[inline]
fn lerp_read(buf: &[f32], w: usize, d: f32) -> f32 {
    let len = buf.len();
    let m = (d as usize).max(1);
    let f = d - m as f32;
    let at = |back: usize| {
        let idx = if w >= back { w - back } else { w + len - back };
        buf[idx]
    };
    let (a, b) = (at(m), at(m + 1));
    a + (b - a) * f.max(0.0)
}

/// The DC blocker `(1 + R)/2 · (1 − z⁻¹)/(1 − R z⁻¹)`: its phase delay at `w` (negative, a
/// lead) and its gain there (Fides's, `fides::string`).
fn dc_response(r: f64, w: f64) -> (f64, f64) {
    let (sn, cs) = math::sin_cos(w);
    let lead = math::atan2(sn, 1.0 - cs) - math::atan2(r * sn, 1.0 - r * cs);
    let gain = 0.5 * (1.0 + r) * (2.0 - 2.0 * cs).sqrt() / (1.0 - 2.0 * r * cs + r * r).sqrt();
    (-lead / w, gain)
}

/// The bow on the string for a slice: its velocity (m/s, signed: the bow's direction), its
/// normal force (N) and the hair's roughness (a relative variation of the friction, the
/// bow-hair noise layer; zeros for a perfectly smooth bow).
#[derive(Copy, Clone, Debug)]
pub struct Bow<'a> {
    pub v: &'a [f32],
    pub force: &'a [f32],
    pub rough: &'a [f32],
    pub friction: Friction,
}

impl BowedString {
    /// Render `out.len()` samples, **adding** `gain` × the wave arriving at the bridge into
    /// `out`. `strike` is a velocity injected at the junction (a pluck); `bow` the bow, if it
    /// is on the string. `probe` (tests) receives the slip `Δ` each sample (0 while it
    /// sticks). `grid_end`: the slice ends on the control grid, where the gate may close.
    /// Returns whether it is still live.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        strike: &[f32],
        bow: Option<Bow<'_>>,
        gain: f32,
        out: &mut [f32],
        mut probe: Option<&mut [f32]>,
        grid_end: bool,
    ) -> bool {
        let n = out.len().min(strike.len());
        if !self.live {
            let fed = strike[..n].iter().any(|x| *x != 0.0)
                || bow.is_some_and(|b| b.v[..n].iter().any(|v| *v != 0.0));
            if !fed {
                if let Some(p) = probe.as_deref_mut() {
                    p[..n].fill(0.0);
                }
                return false;
            }
            self.live = true;
        }
        let (lenn, lenb) = (self.neck.len(), self.bridge.len());
        let longest = (self.dn.max(self.db) as u32) + 16;
        for i in 0..n {
            self.dn += self.dn_step;
            self.db += self.db_step;
            let vn = Self::read(&self.neck, self.wn, self.dn);
            let vb = Self::read(&self.bridge, self.wb, self.db);
            let vh = vn + vb;
            let mut inj = strike[i];
            let mut slip = 0.0f32;
            let mut moving = false;
            let twist = self.torsion.is_some() && (self.tor_live || bow.is_some());
            let (rn, rb) = if twist {
                (
                    lerp_read(&self.tor_n, self.wtn, self.dtn),
                    lerp_read(&self.tor_b, self.wtb, self.dtb),
                )
            } else {
                (0.0, 0.0)
            };
            let mut inj_r = 0.0f32;
            if let Some(b) = bow {
                // The surface's velocity: the translation's and the twist's.
                let vh = vh + rn + rb;
                let (vbow, force) = (b.v[i], b.force[i].max(0.0));
                moving = vbow != 0.0 || force != 0.0;
                let r = force * self.half_y * (1.0 + b.rough[i]).max(0.0);
                let u = vbow - vh;
                let au = u.abs();
                let sgn = if u < 0.0 { -1.0 } else { 1.0 };
                let bowed = match b.friction {
                    Friction::Hyperbolic => {
                        let excess = au - r * self.curve.mu_s;
                        let out = if self.sticking && excess <= 0.0 {
                            u
                        } else {
                            match slip_root(&self.curve, au, r) {
                                Some(x) => {
                                    slip = sgn * x;
                                    if self.sticking && self.excess <= 0.0 && excess > 0.0 {
                                        // The stick ended between the samples: at the
                                        // crossing's fraction of the interval (the excess
                                        // capacity's line through zero), so the slip's
                                        // onset keeps its time to a fraction of a sample —
                                        // without it the period locks to whole samples
                                        // (−4.5 cents at E4, whatever the loop).
                                        let tau = self.excess / (self.excess - excess);
                                        let w = 1.0 - tau;
                                        u - w * slip
                                    } else {
                                        u - slip
                                    }
                                }
                                None => {
                                    // The slip ended between the samples: where the
                                    // quadratic's discriminant crossed zero (its roots
                                    // vanish), the stick holding for the rest of it.
                                    let b = self.curve.v0 + r * self.curve.mu_d - au;
                                    let c = self.curve.v0 * (r * self.curve.mu_s - au);
                                    let disc = b * b - 4.0 * c;
                                    if !self.sticking && self.disc >= 0.0 && disc < 0.0 {
                                        let tau = self.disc / (self.disc - disc);
                                        u - tau * self.last_slip
                                    } else {
                                        u
                                    }
                                }
                            }
                        };
                        self.excess = excess;
                        let b = self.curve.v0 + r * self.curve.mu_d - au;
                        let c = self.curve.v0 * (r * self.curve.mu_s - au);
                        self.disc = b * b - 4.0 * c;
                        self.last_slip = slip;
                        out
                    }
                    Friction::Thermal => {
                        // r42:arcus-p4: Woodhouse & Galluzzo 2025 (the module's notes).
                        let y = self.law.yield_at(self.theta + self.theta2);
                        let rs = r * y;
                        let excess = au - rs * WG_A * math::powf(WG_B, -0.4);
                        let (root, margin) = if self.sticking && excess <= 0.0 {
                            (None, 0.0)
                        } else {
                            self.law.slip(au, rs)
                        };
                        let out = match root {
                            Some(x) => {
                                slip = sgn * x;
                                if self.sticking && self.excess <= 0.0 && excess > 0.0 {
                                    let tau = self.excess / (self.excess - excess);
                                    u - (1.0 - tau) * slip
                                } else {
                                    u - slip
                                }
                            }
                            None => {
                                if !self.sticking && self.disc >= 0.0 && margin < 0.0 {
                                    let tau = self.disc / (self.disc - margin);
                                    u - tau * self.last_slip
                                } else {
                                    u
                                }
                            }
                        };
                        self.excess = excess;
                        self.disc = margin;
                        self.last_slip = slip;
                        // Heated by the power the slip dissipates (`F·μ·|Δ|`), relaxing.
                        let power = if slip != 0.0 {
                            force * y * ThermalLaw::jump(slip) * slip.abs()
                        } else {
                            0.0
                        };
                        self.theta = self.theta * self.th_keep + self.th_gain * power;
                        self.theta2 = self.theta2 * self.th2_keep + self.th2_gain * power;
                        out
                    }
                };
                self.sticking = slip == 0.0;
                // The bow's force moves the surface: the translation takes its share, the
                // twist the rest.
                inj += bowed * self.tor_share;
                inj_r = bowed - bowed * self.tor_share;
            } else {
                self.sticking = false;
                if self.theta != 0.0 || self.theta2 != 0.0 {
                    self.theta *= self.th_keep;
                    self.theta2 *= self.th2_keep;
                    if self.theta < 1e-9 {
                        self.theta = 0.0;
                    }
                    if self.theta2 < 1e-9 {
                        self.theta2 = 0.0;
                    }
                }
            }
            if let Some(p) = probe.as_deref_mut() {
                p[i] = slip;
            }
            if twist {
                let g = self.tor_g;
                let (lb, ln) = (self.tor_b.len(), self.tor_n.len());
                self.tor_b[self.wtb] = -g * (rn + inj_r);
                self.tor_n[self.wtn] = -g * (rb + inj_r);
                self.wtb = if self.wtb + 1 == lb { 0 } else { self.wtb + 1 };
                self.wtn = if self.wtn + 1 == ln { 0 } else { self.wtn + 1 };
                self.tor_live = true;
            }
            let to_b = vn + inj;
            let to_n = vb + inj;
            // The bridge: its loss, the blocker, the inversion of a fixed end.
            let mut y = self.g1a * to_b + self.a * self.lz;
            self.lz = y;
            if self.cr_c < 1.0 {
                self.cr += self.cr_c * (y - self.cr);
                y = self.cr;
            }
            if self.dc_on {
                let h = self.dc_k * (y - self.dc.0) + self.dc_r * self.dc.1;
                self.dc = (y, h);
                y = h;
            }
            self.bridge[self.wb] = -y;
            self.wb += 1;
            if self.wb == lenb {
                self.wb = 0;
            }
            // The finger (or the nut): a fixed end, its damping lumped into the loss.
            self.neck[self.wn] = -to_n;
            self.wn += 1;
            if self.wn == lenn {
                self.wn = 0;
            }
            out[i] += gain * to_b;
            let twisting = (rn.abs() + rb.abs()) >= QUIET;
            if to_b.abs() < QUIET && vh.abs() < QUIET && inj == 0.0 && !moving && !twisting {
                self.quiet += 1;
            } else {
                self.quiet = 0;
            }
        }
        if grid_end && self.quiet > longest {
            self.clear();
        }
        self.live
    }
}
