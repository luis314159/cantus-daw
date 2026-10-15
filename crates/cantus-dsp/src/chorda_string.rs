//! One key of Chorda: its strings (one, two or three), the hammer that strikes them and
//! the bridge they share.
//!
//! **A string** is a bidirectional waveguide split at the hammer's strike point: velocity
//! waves run to the agraffe and back (inverted there) on one side, to the bridge and back
//! on the other. The string's losses and stiffness are lumped at the bridge end: a
//! linear-phase three-tap loss filter (one sample of delay at every frequency, so a decay
//! never moves a pitch), the fractional first-order allpass and the second-order dispersion
//! sections of its [`LoopDesign`]. Everything runs in `f64`: the dispersion sections' poles
//! sit within a few thousandths of the unit circle.
//!
//! **The bridge** is a resistance every string of the key ends on: a string's reflection is
//! `v_b − a` for the wave `a` arriving and the bridge's velocity
//! `v_b = 2 YZ Σa / (1 + n YZ)`. One string alone loses `σ_b = 2 f₀ atanh(YZ)` to it; the
//! strings moving together lose `n` times that, against each other nothing — the two-stage
//! decay. `YZ` is set so the bridge takes the share `coupling` of the measured `b₁`, and the
//! loss filter the rest, so a string alone decays exactly as measured.
//!
//! **The hammer** (one per string, the hammer's mass shared among the strings it strikes,
//! as Bank 2000 does) is Stulov's hereditary felt, `F = Q₀ (u^p − ε h)` with `h` the felt's
//! memory relaxing over `τ` towards `u^p`, discretised as a one-pole; the force and the
//! string's velocity at the strike point are solved together, implicitly, with four Newton
//! steps every sample of contact — a fixed count, so the cost never depends on the data.

use super::design::{second_coefs, LoopDesign, MAX_SECTIONS};
use super::tables::Key;
use crate::math;

pub const MAX_STRINGS: usize = 3;
/// Newton steps of the hammer–string solve, every sample of contact.
pub const HAMMER_STEPS: usize = 4;
/// The hammer leaves the string for good this far away (m), or after this long (s).
const HAMMER_GONE_M: f64 = 1.0e-3;
const HAMMER_GONE_S: f64 = 0.03;

/// A delay line read before it is written: its value is the one written `len` samples ago.
#[derive(Clone, Debug)]
pub struct Ring {
    buf: Vec<f64>,
    len: usize,
    pos: usize,
}

impl Ring {
    /// Allocates: off the audio thread.
    pub fn new(capacity: usize) -> Ring {
        Ring {
            buf: vec![0.0; capacity.max(1)],
            len: 1,
            pos: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    /// The length now.
    pub fn len_now(&self) -> usize {
        self.len
    }

    /// Set the length (within the capacity) and silence the line.
    pub fn reset(&mut self, len: usize) {
        self.len = len.clamp(1, self.buf.len());
        self.pos = 0;
        self.buf[..self.len].fill(0.0);
    }

    #[inline]
    pub fn read(&self) -> f64 {
        self.buf[self.pos]
    }

    #[inline]
    pub fn write(&mut self, x: f64) {
        self.buf[self.pos] = x;
        self.pos += 1;
        if self.pos == self.len {
            self.pos = 0;
        }
    }

    /// The largest magnitude held.
    pub fn peak(&self) -> f64 {
        self.buf[..self.len].iter().fold(0.0, |m, x| m.max(x.abs()))
    }
}

/// The felt's settings, for a key and a strike.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Felt {
    /// Slow-loading stiffness `K` (N/m^p) and the exponent `p`.
    pub k: f64,
    pub p: f64,
    /// Stulov's hereditary amplitude `ε` (0…1) and relaxation time `τ` (s).
    pub eps: f64,
    pub tau: f64,
}

/// One string's hammer contact.
#[derive(Clone, Copy, Debug, Default)]
struct Hammer {
    active: bool,
    /// Hammer and string displacement at the strike point (m), hammer velocity (m/s).
    y_h: f64,
    y_s: f64,
    v_h: f64,
    /// The felt's memory, and the last force (the next solve's first guess).
    h: f64,
    f: f64,
    mass: f64,
    /// Samples since the strike.
    age: u32,
}

#[derive(Clone, Debug)]
struct Str {
    nut: Ring,
    to_bridge: Ring,
    from_bridge: Ring,
    /// The loss filter's two previous inputs.
    fir: [f64; 2],
    /// The fraction section: its coefficient and state.
    frac: f64,
    frac_x: f64,
    frac_y: f64,
    hammer: Hammer,
    /// The wave that last arrived at the bridge (after the filters).
    arrived: f64,
}

impl Str {
    fn new(capacity: usize) -> Str {
        Str {
            nut: Ring::new(capacity),
            to_bridge: Ring::new(capacity),
            from_bridge: Ring::new(capacity),
            fir: [0.0; 2],
            frac: 0.0,
            frac_x: 0.0,
            frac_y: 0.0,
            hammer: Hammer::default(),
            arrived: 0.0,
        }
    }

    fn silence(&mut self) {
        self.fir = [0.0; 2];
        self.frac_x = 0.0;
        self.frac_y = 0.0;
        self.hammer = Hammer::default();
        self.arrived = 0.0;
    }
}

/// A key's strings, hammer and bridge.
#[derive(Clone, Debug)]
pub struct KeyStrings {
    strings: Vec<Str>,
    n: usize,
    fs: f64,
    /// The dispersion sections' denominators (shared by the key's strings).
    a1: [f64; MAX_SECTIONS],
    a2: [f64; MAX_SECTIONS],
    sections: usize,
    /// Each dispersion section's state, string by string: x[n−1], x[n−2], y[n−1], y[n−2]
    /// (section-major, so the strings' chains run side by side).
    bq: [[[f64; 4]; MAX_STRINGS]; MAX_SECTIONS],
    /// The loss filter: gain and the side taps' weight.
    g: f64,
    alpha: f64,
    /// The bridge: `YZ`, and the string's wave impedance `Z` (kg/s).
    yz: f64,
    z: f64,
    /// The hammer's felt for the current strike, and its instantaneous stiffness.
    felt: Felt,
    q0: f64,
    lambda: f64,
}

impl KeyStrings {
    /// Room for strings whose loop is at most `capacity` samples. Allocates.
    pub fn new(capacity: usize, fs: f64) -> KeyStrings {
        KeyStrings {
            strings: (0..MAX_STRINGS).map(|_| Str::new(capacity)).collect(),
            n: 0,
            fs,
            a1: [0.0; MAX_SECTIONS],
            a2: [0.0; MAX_SECTIONS],
            sections: 0,
            bq: [[[0.0; 4]; MAX_STRINGS]; MAX_SECTIONS],
            g: 0.0,
            alpha: 0.0,
            yz: 0.0,
            z: 1.0,
            felt: Felt {
                k: 1.0e9,
                p: 2.5,
                eps: 0.0,
                tau: 1.0e-5,
            },
            q0: 1.0e9,
            lambda: 0.0,
        }
    }

    /// The loop length a key's design needs from this capacity.
    pub fn fits(&self, design: &LoopDesign) -> bool {
        design.delay < self.strings[0].nut.capacity()
    }

    /// Lay the key's strings out for its design, silent: lengths from the strike point,
    /// the dispersion sections, each string's fraction detuned by `detune_ct[i]` cents.
    pub fn tune(&mut self, key: &Key, design: &LoopDesign, detune_ct: [f64; MAX_STRINGS]) {
        self.tune_strings(key, design, detune_ct, key.strings as usize);
    }

    /// [`Self::tune`] with `strings` strings (a test's one string alone).
    pub fn tune_strings(
        &mut self,
        key: &Key,
        design: &LoopDesign,
        detune_ct: [f64; MAX_STRINGS],
        strings: usize,
    ) {
        self.n = strings.clamp(1, MAX_STRINGS);
        self.z = key.impedance;
        self.sections = design.sections;
        for (i, (r, t)) in design.poles[..design.sections].iter().enumerate() {
            let (a1, a2) = second_coefs(*r, *t);
            self.a1[i] = a1;
            self.a2[i] = a2;
        }
        let total = design.delay.max(3);
        let nut = ((key.strike * total as f64 / 2.0).round() as usize).max(1);
        let to = ((total - 2 * nut) / 2).max(1);
        let from = (total - 2 * nut - to).max(1);
        self.bq = [[[0.0; 4]; MAX_STRINGS]; MAX_SECTIONS];
        for (i, s) in self.strings.iter_mut().enumerate().take(self.n) {
            s.nut.reset(2 * nut);
            s.to_bridge.reset(to);
            s.from_bridge.reset(from);
            s.silence();
            s.frac = (design.frac + detune_ct[i] * design.frac_per_cent).clamp(-0.99, 0.99);
        }
    }

    /// Set the loss filter and the bridge (see the module's notes).
    pub fn set_loss(&mut self, g: f64, alpha: f64, yz: f64) {
        self.g = g;
        self.alpha = alpha;
        self.yz = yz;
    }

    /// Strike: the hammer of `mass` (kg, the whole hammer's) at `velocity` (m/s), string
    /// `i` struck at `weight[i]` of it (the soft pedal moves it off the last string).
    pub fn strike(&mut self, felt: Felt, mass: f64, velocity: f64, weight: [f64; MAX_STRINGS]) {
        self.felt = felt;
        let eps = felt.eps.clamp(0.0, 0.98);
        self.q0 = felt.k / (1.0 - eps);
        self.lambda = math::exp(-1.0 / (felt.tau.max(1e-7) * self.fs));
        let n = self.n.max(1) as f64;
        for (i, s) in self.strings.iter_mut().enumerate().take(self.n) {
            let v = velocity * weight[i];
            s.hammer = Hammer {
                active: v > 0.0,
                y_h: 0.0,
                y_s: 0.0,
                v_h: v,
                h: 0.0,
                f: 0.0,
                mass: mass / n,
                age: 0,
            };
        }
    }

    /// Whether a hammer still touches or approaches a string.
    pub fn hammering(&self) -> bool {
        self.strings[..self.n].iter().any(|s| s.hammer.active)
    }

    /// The largest wave the strings hold: what a voice's end is judged by.
    pub fn peak(&self) -> f64 {
        self.strings[..self.n]
            .iter()
            .map(|s| {
                s.nut
                    .peak()
                    .max(s.to_bridge.peak())
                    .max(s.from_bridge.peak())
            })
            .fold(0.0, f64::max)
    }

    /// Silence the strings at once (a voice ending).
    pub fn silence(&mut self) {
        self.bq = [[[0.0; 4]; MAX_STRINGS]; MAX_SECTIONS];
        for s in self.strings.iter_mut().take(self.n) {
            let (a, b, c) = (s.nut.len, s.to_bridge.len, s.from_bridge.len);
            s.nut.reset(a);
            s.to_bridge.reset(b);
            s.from_bridge.reset(c);
            s.silence();
        }
    }

    /// The waves that last arrived at the bridge, after the filters (for the phantom
    /// partials: the strings' tension follows their squares).
    pub fn arrived(&self, i: usize) -> f64 {
        self.strings[i].arrived
    }

    pub fn strings(&self) -> usize {
        self.n
    }

    /// One sample: the force the strings put on the bridge (N).
    #[inline]
    pub fn tick(&mut self) -> f64 {
        let n = self.n;
        let inv_fs = 1.0 / self.fs;
        let half_z = 0.5 / self.z;
        let mut xs = [0.0; MAX_STRINGS];
        for (i, s) in self.strings.iter_mut().enumerate().take(n) {
            let in_n = -s.nut.read();
            let in_b = s.from_bridge.read();
            let mut f2z = 0.0;
            if s.hammer.active {
                let f = hammer_force(
                    &mut s.hammer,
                    in_n + in_b,
                    half_z,
                    inv_fs,
                    self.q0,
                    self.felt.p,
                    self.felt.eps.clamp(0.0, 0.98),
                    self.lambda,
                );
                f2z = f * half_z;
            }
            s.nut.write(in_b + f2z);
            let raw = s.to_bridge.read();
            s.to_bridge.write(in_n + f2z);
            // The loss filter (linear phase, one sample) …
            let lossy =
                self.g * (self.alpha * (raw + s.fir[1]) + (1.0 - 2.0 * self.alpha) * s.fir[0]);
            s.fir[1] = s.fir[0];
            s.fir[0] = raw;
            // … the fraction …
            let y = s.frac * (lossy - s.frac_y) + s.frac_x;
            s.frac_x = lossy;
            s.frac_y = y;
            xs[i] = y;
        }
        // … and the dispersion sections, the strings' chains side by side.
        match n {
            3 => self.disperse::<3>(&mut xs),
            2 => self.disperse::<2>(&mut xs),
            _ => self.disperse::<1>(&mut xs),
        }
        let mut sum = 0.0;
        for (s, x) in self.strings.iter_mut().zip(xs).take(n) {
            s.arrived = x;
            sum += x;
        }
        // The bridge: every string's reflection, and the force on it.
        let v_b = 2.0 * self.yz * sum / (1.0 + n as f64 * self.yz);
        for s in self.strings.iter_mut().take(n) {
            s.from_bridge.write(v_b - s.arrived);
        }
        self.z * (2.0 * sum - n as f64 * v_b)
    }

    #[inline(always)]
    fn disperse<const N: usize>(&mut self, xs: &mut [f64; MAX_STRINGS]) {
        let secs = self.sections;
        for ((st, a1), a2) in self.bq[..secs]
            .iter_mut()
            .zip(&self.a1[..secs])
            .zip(&self.a2[..secs])
        {
            let (a1, a2) = (*a1, *a2);
            for i in 0..N {
                let s = &mut st[i];
                let x = xs[i];
                let out = a2 * x + a1 * s[0] + s[1] - a1 * s[2] - a2 * s[3];
                s[1] = s[0];
                s[0] = x;
                s[3] = s[2];
                s[2] = out;
                xs[i] = out;
            }
        }
    }
}

/// The hammer–string contact for one sample: the force (N), the hammer and the string's
/// strike point moved. `v_in` is the string's velocity there before the force.
#[allow(clippy::too_many_arguments)]
#[inline]
fn hammer_force(
    h: &mut Hammer,
    v_in: f64,
    half_z: f64,
    inv_fs: f64,
    q0: f64,
    p: f64,
    eps: f64,
    lambda: f64,
) -> f64 {
    // Compression with no force this sample, and how much a newton of force takes off it.
    let u0 = (h.y_h + h.v_h * inv_fs) - (h.y_s + v_in * inv_fs);
    let c = inv_fs * inv_fs / h.mass + half_z * inv_fs;
    let gain = 1.0 - eps * (1.0 - lambda);
    let memory = eps * lambda * h.h;
    let mut f = h.f.max(0.0);
    for _ in 0..HAMMER_STEPS {
        let u = u0 - c * f;
        let (g, dg) = if u > 0.0 {
            let up = math::powf(u, p - 1.0);
            (up * u, p * up)
        } else {
            (0.0, 0.0)
        };
        let resid = f - q0 * (gain * g - memory);
        let slope = 1.0 + q0 * gain * dg * c;
        f -= resid / slope;
    }
    let f = f.max(0.0);
    let u = u0 - c * f;
    let g = if u > 0.0 { math::powf(u, p) } else { 0.0 };
    h.h = lambda * h.h + (1.0 - lambda) * g;
    h.v_h -= f * inv_fs / h.mass;
    h.y_h += h.v_h * inv_fs;
    h.y_s += (v_in + f * half_z) * inv_fs;
    h.f = f;
    h.age += 1;
    let gap = h.y_s - h.y_h;
    if (f == 0.0 && gap > HAMMER_GONE_M) || h.age as f64 * inv_fs > HAMMER_GONE_S {
        h.active = false;
    }
    f
}

/// The loss filter `(g, α)` and the bridge's `YZ` for a key (see the module's notes): the
/// measured decay `σ_k = b₁ + b₂ (kπ/L)²` slowed `decay` times, plus a damper's `damp1`
/// (s⁻¹) and `damp2` (m²/s, the same law), the bridge taking the share `coupling` of `b₁`.
/// The filter's two numbers put the decay exactly on the law at the first partial and at
/// the highest one below 4 kHz (and 0.4·fs); between them its cosine follows the law's
/// square closely.
pub fn loss(
    key: &Key,
    fs: f64,
    coupling: f64,
    decay: f64,
    damp1: f64,
    damp2: f64,
) -> (f64, f64, f64) {
    loss_timed(key, fs, coupling, decay, damp1, damp2, &|k| {
        k as f64 / key.partial(k)
    })
}

/// [`loss`], with the loop's own round-trip time (s) at partial `k`: its group delay there.
/// A dispersion section sharp near a partial lengthens that partial's trip, so it loses less
/// a second for the same loss a trip — the filter is set from the trips the loop really has.
pub fn loss_timed(
    key: &Key,
    fs: f64,
    coupling: f64,
    decay: f64,
    damp1: f64,
    damp2: f64,
    trip: &dyn Fn(u32) -> f64,
) -> (f64, f64, f64) {
    let m = decay.max(0.05);
    let (b1, b2) = (key.b1 / m, key.b2 / m);
    let f1 = key.partial(1);
    let yz = math::tanh(coupling.clamp(0.0, 0.95) * b1 / (2.0 * f1));
    let sigma = |k: u32| {
        let beta = k as f64 * std::f64::consts::PI / key.length;
        b1 * (1.0 - coupling.clamp(0.0, 0.95)) + b2 * beta * beta + damp1 + damp2 * beta * beta
    };
    // Per round trip, partial k loses σ_k over the trip's time.
    let per_trip = |k: u32| -sigma(k) * trip(k);
    let top = (4_000.0f64).min(0.4 * fs);
    let kd = (2..=64u32)
        .take_while(|k| *k == 2 || key.partial(*k) <= top)
        .last()
        .unwrap_or(2);
    let c = |k: u32| 1.0 - math::cos(std::f64::consts::TAU * key.partial(k) / fs);
    let (c1, cd) = (c(1), c(kd));
    let r = math::exp(per_trip(kd) - per_trip(1));
    let alpha = if cd - r * c1 > 0.0 {
        ((1.0 - r) / (2.0 * (cd - r * c1))).clamp(0.0, 0.2499)
    } else {
        0.2499
    };
    let g = (math::exp(per_trip(1)) / (1.0 - 2.0 * alpha * c1)).min(1.0);
    (g, alpha, yz)
}
