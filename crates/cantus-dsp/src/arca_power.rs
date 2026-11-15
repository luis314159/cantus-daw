//! Arca's power amp: a long-tailed-pair phase inverter, two push-pull sides of power
//! pentodes into an output transformer, negative feedback with its presence network (or
//! the top-boost amp's cut control, which it has instead), and the supply that feeds it —
//! a rectifier behind its own resistance charging a filter capacitor, so the plates' and
//! screens' voltage **sags** under a loud chord and recovers as it fades.
//!
//! Per oversampled sample, in `f64`:
//!
//! 1. the inverter: `e = swing · tanh(A·(x − feedback)/swing)`, `+e` to one side, `−e` to
//!    the other, each through its coupling capacitor into its grid leak; once a grid
//!    passes its cathode it conducts (`r_gc`), and the current charges the coupling
//!    capacitor — the bias shifts and the side cuts off sooner (blocking);
//! 2. each side's plate current by [`Pentode::current`] at its grid, the screen (the
//!    sagged supply) and its plate, the plate-to-plate voltage `V = (I₊ − I₋)·Raa/2`
//!    solved with the plates' own dependence on it (two Newton steps from the last);
//! 3. the speaker tap `V·√(R_tap/Raa)` through the transformer's band (its primary
//!    inductance, its leakage);
//! 4. the supply: `C·dV/dt = max(V₀ − V, 0)/R_s − I`; a cathode-biased amp's cathode
//!    likewise (`C_k·dV_k/dt = I − V_k/R_k`).
//!
//! The output is the tap voltage over its peak at the rated power: 1.0 is full power.

use super::tube::Pentode;
use crate::fragor::dk::{fast_exp, fast_ln};

/// How the power valves are biased.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Bias {
    /// A fixed negative grid supply, set for this idle current per valve (A).
    Fixed { idle: f64 },
    /// A shared cathode resistor and its bypass capacitor (class A: the bias moves).
    Cathode { rk: f64, ck: f64 },
}

/// The negative feedback: from the `tap` (ohms) through `r_fb` into the inverter's tail
/// `r_tail`, with a presence network (`pot` ohms in series with `cap` from the tail to
/// ground) or none.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Feedback {
    pub tap: f64,
    pub r_fb: f64,
    pub r_tail: f64,
    pub presence: Option<(f64, f64)>,
}

/// A power amp's values.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PowerSpec {
    pub tube: Pentode,
    /// Valves per side (in parallel).
    pub per_side: f64,
    /// The unloaded supply (V) and what is behind it: the rectifier's and the
    /// transformer's resistance (Ω), the first filter capacitor (F).
    pub supply: f64,
    pub r_supply: f64,
    pub c_filter: f64,
    /// Plate-to-plate load (Ω), and the speaker tap the output is read at (Ω).
    pub raa: f64,
    pub r_speaker: f64,
    pub bias: Bias,
    pub feedback: Option<Feedback>,
    /// The top-boost amp's cut: a pot (Ω) in series with a capacitor (F) between the
    /// inverter's outputs.
    pub cut: Option<(f64, f64)>,
    /// The inverter: gain to each side, the peak each side swings at most (V), and its
    /// output resistance (Ω).
    pub pi_gain: f64,
    pub pi_swing: f64,
    pub pi_r: f64,
    /// The power valves' coupling capacitors and grid leaks.
    pub c_couple: f64,
    pub r_grid: f64,
    /// Rated power (W).
    pub watts: f64,
    /// The transformer's band: primary inductance's corner, leakage's corner (Hz).
    pub low_hz: f64,
    pub high_hz: f64,
}

/// `tanh`, from [`fast_exp`] (the inverter's soft limit).
#[inline]
fn tanh(x: f64) -> f64 {
    let e = fast_exp(-2.0 * x.abs());
    let t = (1.0 - e) / (1.0 + e);
    if x < 0.0 {
        -t
    } else {
        t
    }
}

/// `ln cosh x`, the antiderivative of `tanh`.
#[inline]
fn ln_cosh(x: f64) -> f64 {
    let a = x.abs();
    a + fast_ln(1.0 + fast_exp(-2.0 * a)) - std::f64::consts::LN_2
}

/// The inverter's soft limit with first-order antiderivative antialiasing (Parker,
/// Zavalishin & Le Bivic, DAFx-16): the mean of `tanh` over the segment from the last
/// input to this one, `(F(x) − F(x₁)) / (x − x₁)`, `F = ln cosh` — the limit's corners
/// rounded by half a sample, its images above the band cut by tens of dB.
#[inline]
fn tanh_adaa(x: f64, x1: f64) -> f64 {
    let d = x - x1;
    if d.abs() < 1e-5 {
        tanh(0.5 * (x + x1))
    } else {
        (ln_cosh(x) - ln_cosh(x1)) / d
    }
}

/// The inverter's input pole (Hz).
const PI_IN_HZ: f64 = 16_000.0;

/// A grid's conduction resistance once it passes the cathode.
const R_GC: f64 = 1_500.0;
/// The preamp's own draw from the supply (A).
const I_PRE: f64 = 0.004;

/// A first-order section `y = b0 x + b1 x₁ − a1 y₁` (bilinear).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct First {
    b0: f64,
    b1: f64,
    a1: f64,
    x1: f64,
    y1: f64,
}

impl First {
    /// `(n0 + n1 s) / (d0 + d1 s)`, bilinear at `rate` (no prewarp: its corners are far
    /// below the oversampled Nyquist).
    fn analog(n0: f64, n1: f64, d0: f64, d1: f64, rate: f64) -> First {
        let k = 2.0 * rate;
        let a0 = d0 + d1 * k;
        First {
            b0: (n0 + n1 * k) / a0,
            b1: (n0 - n1 * k) / a0,
            a1: (d0 - d1 * k) / a0,
            x1: 0.0,
            y1: 0.0,
        }
    }
    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.b1 * self.x1 - self.a1 * self.y1;
        self.x1 = x;
        self.y1 = y;
        y
    }
    fn clear(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

/// The power amp's state at a moment (what rest returns to).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Live {
    /// Each side's coupling capacitor voltage.
    vc: [f64; 2],
    /// The supply, the cathode.
    b: f64,
    vk: f64,
    /// Plate-to-plate voltage (the last solution).
    vpp: f64,
    /// The feedback at the last sample (V at the inverter's input).
    fb: f64,
    /// The inverter's last input over its swing (the antialiasing's other end).
    pi_x: f64,
}

#[derive(Clone, Debug)]
pub struct Power {
    spec: PowerSpec,
    rate: f64,
    /// The fixed bias's grid return (V, negative); 0 for cathode bias.
    vbias: f64,
    turns: f64,
    full: f64,
    live: Live,
    idle: Live,
    lo: First,
    hi: [First; 2],
    /// The inverter's input bandwidth: the tone stack's output impedance against the
    /// inverter grid's Miller capacitance (≈ 100 kΩ · 100 pF: a pole near 16 kHz).
    pi_in: First,
    fb_net: First,
    cut: [First; 1],
    /// The feedback's share at mid band (the presence network's flat part).
    beta: f64,
    presence: f64,
    /// The supply as it runs: its resistance (the sag setting's share of the spec's) and
    /// the open-circuit voltage that keeps the idle point where the spec puts it.
    rs: f64,
    v0: f64,
    idle_draw: f64,
}

impl Power {
    /// The power amp of `spec` at `rate` (the oversampled rate), at idle: the supply sagged
    /// to its idle draw, the bias set (fixed: for the idle current; cathode: where the
    /// cathode settles).
    pub fn new(spec: PowerSpec, rate: f64) -> Power {
        let t = spec.tube;
        let n = spec.per_side;
        let mut b = spec.supply;
        let mut vbias = 0.0;
        let mut vk = 0.0;
        let mut draw = 0.0;
        for _ in 0..60 {
            let ip = match spec.bias {
                Bias::Fixed { idle } => {
                    vbias = t.bias_for(idle, b, b);
                    idle
                }
                Bias::Cathode { rk, .. } => {
                    // The cathode where Vk = Rk · (both sides' current): bisection.
                    let (mut lo, mut hi) = (0.0f64, 60.0f64);
                    for _ in 0..100 {
                        let mid = 0.5 * (lo + hi);
                        let i = t.current(-mid, b - mid, b - mid).0;
                        if rk * 2.0 * n * (1.0 + t.screen) * i > mid {
                            lo = mid;
                        } else {
                            hi = mid;
                        }
                    }
                    vk = 0.5 * (lo + hi);
                    t.current(-vk, b - vk, b - vk).0
                }
            };
            draw = 2.0 * n * (1.0 + t.screen) * ip + I_PRE;
            b = spec.supply - spec.r_supply * draw;
        }
        let idle = Live {
            vc: [-vbias, -vbias],
            b,
            vk,
            vpp: 0.0,
            fb: 0.0,
            pi_x: 0.0,
        };
        let w = |hz: f64| std::f64::consts::TAU * hz;
        let wl = w(spec.low_hz);
        let wh = w(spec.high_hz);
        let mut p = Power {
            spec,
            rate,
            vbias,
            turns: (spec.r_speaker / spec.raa).sqrt(),
            full: (2.0 * spec.watts * spec.r_speaker).sqrt(),
            live: idle,
            idle,
            lo: First::analog(0.0, 1.0, wl, 1.0, rate),
            pi_in: First::analog(w(PI_IN_HZ), 0.0, w(PI_IN_HZ), 1.0, rate),
            hi: [First::analog(wh, 0.0, wh, 1.0, rate); 2],
            fb_net: First::default(),
            cut: [First::default()],
            beta: 0.0,
            presence: f64::NAN,
            rs: spec.r_supply,
            v0: spec.supply,
            idle_draw: draw,
        };
        p.set_presence(0.5);
        p
    }

    pub fn spec(&self) -> &PowerSpec {
        &self.spec
    }

    /// The grid bias a fixed-bias amp was set to (V), and the idle supply (V).
    pub fn bias_and_supply(&self) -> (f64, f64) {
        (self.vbias, self.idle.b)
    }

    /// The idle cathode voltage (cathode bias), V.
    pub fn idle_cathode(&self) -> f64 {
        self.idle.vk
    }

    /// The feedback's mid-band share (0 without feedback).
    pub fn beta(&self) -> f64 {
        self.beta
    }

    /// Presence (or, on the top-boost amp, the cut turned the other way): 0 … 1. Keeps
    /// the filters' state.
    pub fn set_presence(&mut self, p: f64) {
        let p = p.clamp(0.0, 1.0);
        if p == self.presence {
            return;
        }
        self.presence = p;
        let rate = self.rate;
        if let Some(f) = self.spec.feedback {
            let (rf, rt) = (f.r_fb, f.r_tail);
            let tap_share = (f.tap / self.spec.r_speaker).sqrt();
            let (x1, y1) = (self.fb_net.x1, self.fb_net.y1);
            self.fb_net = match f.presence {
                Some((pot, cap)) => {
                    let rp = 1.0 + (1.0 - p) * pot;
                    First::analog(
                        rt * tap_share,
                        rt * cap * rp * tap_share,
                        rf + rt,
                        cap * (rf * (rt + rp) + rt * rp),
                        rate,
                    )
                }
                None => First::analog(rt * tap_share, 0.0, rf + rt, 0.0, rate),
            };
            self.fb_net.x1 = x1;
            self.fb_net.y1 = y1;
            self.beta = rt / (rf + rt) * tap_share;
        }
        if let Some((pot, cap)) = self.spec.cut {
            let rc = 1.0 + p * pot;
            let (x1, y1) = (self.cut[0].x1, self.cut[0].y1);
            self.cut[0] = First::analog(
                1.0,
                rc * cap,
                1.0,
                (rc + 2.0 * self.spec.pi_r) * cap,
                rate,
            );
            self.cut[0].x1 = x1;
            self.cut[0].y1 = y1;
        }
    }

    /// The supply's resistance times `f` (the sag setting), the idle point kept.
    pub fn set_sag(&mut self, f: f64) {
        self.rs = self.spec.r_supply * f;
        self.v0 = self.idle.b + self.rs * self.idle_draw;
    }

    /// Back to idle.
    pub fn rest(&mut self) {
        self.live = self.idle;
        self.lo.clear();
        self.pi_in.clear();
        for f in &mut self.hi {
            f.clear();
        }
        self.fb_net.clear();
        self.cut[0].clear();
    }

    /// The supply's voltage now (V): what the display's sag meter reads.
    pub fn supply(&self) -> f64 {
        self.live.b
    }

    /// One oversampled sample: the inverter's input (V) in, the speaker over its peak at
    /// full power out.
    #[inline]
    pub fn step(&mut self, x: f64) -> f64 {
        let s = self.spec;
        let t = s.tube;
        let dt = 1.0 / self.rate;
        let u = self.pi_in.run(x) - self.live.fb;
        let xs = s.pi_gain * u / s.pi_swing;
        let mut e = s.pi_swing * tanh_adaa(xs, self.live.pi_x);
        self.live.pi_x = xs;
        if s.cut.is_some() {
            e = self.cut[0].run(e);
        }
        let vk = self.live.vk;
        let ret = self.vbias;
        let (g1, gg, gc) = (1.0 / s.pi_r, 1.0 / s.r_grid, 1.0 / R_GC);
        let mut vgk = [0.0; 2];
        for (side, sign) in [1.0f64, -1.0].into_iter().enumerate() {
            let a = sign * e - self.live.vc[side];
            let mut vg = (a * g1 + ret * gg) / (g1 + gg);
            if vg > vk {
                vg = (a * g1 + ret * gg + vk * gc) / (g1 + gg + gc);
            }
            let i = (a - vg) * g1;
            self.live.vc[side] += i * dt / s.c_couple;
            vgk[side] = vg - vk;
        }
        let b = self.live.b;
        let vg2 = b - vk;
        let n = s.per_side;
        let half = 0.5 * s.raa;
        let mut v = self.live.vpp;
        let (mut i1, mut i2) = (0.0, 0.0);
        for k in 0..2 {
            let (a1, _, d1) = t.current(vgk[0], vg2, b - vk - 0.5 * v);
            let (a2, _, d2) = t.current(vgk[1], vg2, b - vk + 0.5 * v);
            i1 = n * a1;
            i2 = n * a2;
            if k == 1 {
                break;
            }
            let f = v - (i1 - i2) * half;
            let fp = 1.0 + 0.5 * n * (d1 + d2) * half;
            v -= f / fp;
        }
        self.live.vpp = v;
        let tap = v * self.turns;
        let low = self.lo.run(tap);
        let mid = self.hi[0].run(low);
        let band = self.hi[1].run(mid);
        if s.feedback.is_some() {
            self.live.fb = self.fb_net.run(band);
        }
        let draw = (1.0 + t.screen) * (i1 + i2);
        let charge = (self.v0 - b).max(0.0) / self.rs;
        self.live.b = b + dt / s.c_filter * (charge - draw - I_PRE);
        if let Bias::Cathode { rk, ck } = s.bias {
            self.live.vk = vk + dt / ck * (draw - vk / rk);
        }
        band / self.full
    }
}
