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
//! **The second polarisation** (model 2, round 40): a key of one or two strings may lay one
//! more loop beside them — the strings' horizontal motion, struck weakly (the blow is never
//! quite vertical) and ending on the bridge's stiffer horizontal side, so it loses little
//! there: a slower decay under the vertical one, the long *aftersound* a real grand's
//! single and paired strings sing on (Weinreich 1977). It has its own bridge (`polar_yz` of
//! the vertical's `YZ`), its force adds to the vertical strings'. With none laid, the key is
//! the strings alone, to the bit.
//!
//! **The hammer** (one per string, the hammer's mass shared among the strings it strikes,
//! as Bank 2000 does) is Stulov's hereditary felt, `F = Q₀ (u^p − ε h)` with `h` the felt's
//! memory relaxing over `τ` towards `u^p`, discretised as a one-pole; the force and the
//! string's velocity at the strike point are solved together, implicitly, with four Newton
//! steps every sample of contact — a fixed count, so the cost never depends on the data.

use super::design::{first_phase, second_coefs, second_phase, LoopDesign, MAX_SECTIONS};
use super::tables::Key;
use crate::math;

pub const MAX_STRINGS: usize = 3;
/// The most resonances a key's bridge admittance carries (round 40).
pub const BRIDGE_MODES: usize = 8;
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
    /// Loops laid: the vertical strings (`nv`, sharing the bridge), then the horizontal
    /// polarisation's (each on its own side of the bridge).
    n: usize,
    nv: usize,
    /// The horizontal loops' bridge: its `YZ` against the vertical's.
    polar_yz: f64,
    /// The first loop on the second loss filter (`g_h`): the horizontal loops, or from the
    /// last string of a unison the damper grips less (round 40). `n`: none.
    loose_from: usize,
    fs: f64,
    /// The dispersion sections' denominators (shared by the key's strings).
    a1: [f64; MAX_SECTIONS],
    a2: [f64; MAX_SECTIONS],
    sections: usize,
    /// Each dispersion section's state, string by string: x[n−1], x[n−2], y[n−1], y[n−2]
    /// (section-major, so the strings' chains run side by side).
    bq: [[[f64; 4]; MAX_STRINGS]; MAX_SECTIONS],
    /// The loss filter: gain and the side taps' weight (the horizontal loops' their own:
    /// the damper grips them less).
    g: f64,
    alpha: f64,
    g_h: f64,
    alpha_h: f64,
    /// The bridge: `YZ`, and the string's wave impedance `Z` (kg/s).
    yz: f64,
    z: f64,
    /// The hammer's felt for the current strike, and its instantaneous stiffness.
    felt: Felt,
    q0: f64,
    lambda: f64,
    /// Round 40: the bridge's own resonances — the board's modes near the key in its
    /// admittance, each a band-pass (positive real, so the loop stays passive) at `g` of
    /// the bridge's `YZ`: per mode `[b0, a1, a2, g, s1, s2]` (b1 = 0, b2 = −b0, DF2T).
    /// `nbm` 0: none (the bridge is resistive, as before).
    bm: [[f64; 6]; BRIDGE_MODES],
    nbm: usize,
    /// Round 40: each string's own dispersion (its stiffness a little apart from its
    /// neighbours', as no two strings of a unison are alike): per string the sections'
    /// denominators, used when `own` — so each partial's detune differs and the unison's
    /// partials beat on their own schedules, not as one notch sweeping (a fixed-cents
    /// detune's flanger).
    a1s: [[f64; MAX_SECTIONS]; MAX_STRINGS],
    a2s: [[f64; MAX_SECTIONS]; MAX_STRINGS],
    own: bool,
    /// Round 40: each string's own losses — its loss and its frequency-dependent share
    /// scaled by `1 + ls[i]` (none: alike) — so a unison's partials fade at their own rates
    /// and its beats are as shallow as a real unison's.
    ls: [(f64, f64); MAX_STRINGS],
    ls_on: bool,
}

impl KeyStrings {
    /// Room for strings whose loop is at most `capacity` samples. Allocates.
    pub fn new(capacity: usize, fs: f64) -> KeyStrings {
        KeyStrings {
            strings: (0..MAX_STRINGS).map(|_| Str::new(capacity)).collect(),
            n: 0,
            nv: 0,
            polar_yz: 0.0,
            loose_from: 0,
            fs,
            a1: [0.0; MAX_SECTIONS],
            a2: [0.0; MAX_SECTIONS],
            sections: 0,
            bq: [[[0.0; 4]; MAX_STRINGS]; MAX_SECTIONS],
            g: 0.0,
            alpha: 0.0,
            g_h: 0.0,
            alpha_h: 0.0,
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
            bm: [[0.0; 6]; BRIDGE_MODES],
            nbm: 0,
            a1s: [[0.0; MAX_SECTIONS]; MAX_STRINGS],
            a2s: [[0.0; MAX_SECTIONS]; MAX_STRINGS],
            own: false,
            ls: [(0.0, 0.0); MAX_STRINGS],
            ls_on: false,
        }
    }

    /// Each string's own losses: `(loss, frequency-dependent share)` scales, each `1 + x`
    /// (all zero: alike). After [`Self::lay`]. Allocates nothing.
    pub fn set_string_losses(&mut self, ls: [(f64, f64); MAX_STRINGS]) {
        self.ls = ls;
        self.ls_on = ls.iter().take(self.n).any(|(a, b)| *a != 0.0 || *b != 0.0);
    }

    /// Each string's own dispersion: its sections' pole angles scaled by `1 + eps[i]` (all
    /// zero: the design's, shared). After [`Self::lay`]. Allocates nothing.
    pub fn set_string_stiffness(&mut self, design: &LoopDesign, eps: [f64; MAX_STRINGS]) {
        self.set_string_stiffness_held(design, eps, None);
    }

    /// [`Self::set_string_stiffness`]; with `hold` (the first partial's frequency, rad/sample)
    /// each string's fraction is moved so its first partial stays where its detune put it —
    /// a tuner tunes the unison's fundamentals, and the strings' unlike stiffness shows only
    /// in their upper partials (round 40: unheld, the scatter detuned the fundamentals too,
    /// and a unison so far apart lost its two-stage decay).
    pub fn set_string_stiffness_held(
        &mut self,
        design: &LoopDesign,
        eps: [f64; MAX_STRINGS],
        hold: Option<f64>,
    ) {
        self.own = eps.iter().take(self.n).any(|e| *e != 0.0);
        if !self.own {
            return;
        }
        for (i, e) in eps.iter().enumerate().take(self.n) {
            let mut e = *e;
            if let Some(w) = hold {
                // The scatter a fraction in reach can give back (round 40, candidate 8: a
                // string whose sections took more phase at its first partial than the
                // fraction holds was clamped there, out by up to 50 cents — D3 and A#1 at
                // candidate 7's scatter): the scatter as drawn, else the other way, else
                // half, a quarter or an eighth of either, else none (candidate 9: with only
                // the halves, D#3's strings all fell to none — alike, they swept as one notch).
                let frac = self.strings[i].frac;
                let mut pick = None;
                for k in [
                    e,
                    -e,
                    0.5 * e,
                    -0.5 * e,
                    0.25 * e,
                    -0.25 * e,
                    0.125 * e,
                    -0.125 * e,
                ] {
                    if let Some(a) = held_frac(design, frac, w, k) {
                        pick = Some((k, a));
                        break;
                    }
                }
                let (k, a) = pick.unwrap_or((0.0, frac));
                e = k;
                self.strings[i].frac = a;
            }
            for (j, (r, t)) in design.poles[..design.sections].iter().enumerate() {
                let t2 = (*t * (1.0 + e)).clamp(1e-6, std::f64::consts::PI - 1e-6);
                let (a1, a2) = second_coefs(*r, t2);
                self.a1s[i][j] = a1;
                self.a2s[i][j] = a2;
            }
        }
    }

    /// The bridge's resonances: `(Hz, Q, gain against YZ)` each (at most [`BRIDGE_MODES`];
    /// none: a resistive bridge). Their states cleared. Allocates nothing.
    pub fn set_bridge_modes(&mut self, modes: &[(f64, f64, f64)]) {
        self.nbm = 0;
        for (f, q, g) in modes.iter().take(BRIDGE_MODES) {
            if *g <= 0.0 || *f <= 0.0 || *f >= 0.45 * self.fs {
                continue;
            }
            let w = std::f64::consts::TAU * f / self.fs;
            let alpha = math::sin(w) / (2.0 * q.max(0.5));
            let a0 = 1.0 + alpha;
            self.bm[self.nbm] = [
                alpha / a0,
                -2.0 * math::cos(w) / a0,
                (1.0 - alpha) / a0,
                *g,
                0.0,
                0.0,
            ];
            self.nbm += 1;
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
        self.lay(key, design, detune_ct, strings, 0, 0.0, 0.0);
    }

    /// The samples from the strike point to the agraffe (one way) for `key` on `design`.
    ///
    /// `strike_true` 0 (beta.14's): the key's strike fraction of the integer delay lines
    /// alone. But the dispersion sections carry the rest of a low partial's period (middle
    /// C: the lines ~130 samples of a 183-sample period), so the strike's comb notched
    /// partials 12–13 there instead of 8 — the strike sat at 0.08 of the string, not 0.12,
    /// and partials 8–9 stood ~7 dB over the reference's, 12–13 ~13 under (round 40,
    /// candidate 11). `strike_true` 1: the fraction of the loop's phase delay at the partial
    /// the comb notches (`n = 1/strike`), so the notch lands where the strike puts it;
    /// between, a blend. Never more than leaves a sample each way to the bridge.
    pub fn nut_samples(key: &Key, design: &LoopDesign, strike_true: f64) -> usize {
        let total = design.delay.max(3);
        if strike_true == 0.0 {
            return ((key.strike * total as f64 / 2.0).round() as usize).max(1);
        }
        let n = 1.0 / key.strike.max(1e-3);
        let f0 = key.f1 / (1.0 + key.b).sqrt();
        let phase_delay = design.fs / (f0 * (1.0 + key.b * n * n).sqrt());
        let span = total as f64 + strike_true * (phase_delay - total as f64);
        let most = ((total - 2) / 2).max(1);
        ((key.strike * span / 2.0).round().max(1.0) as usize).min(most)
    }

    /// Lay `strings` vertical strings and `polar` horizontal loops after them (their
    /// bridge's `YZ` `polar_yz` times the vertical's); `detune_ct` covers every loop; the
    /// strike point placed as [`Self::nut_samples`] places it.
    #[allow(clippy::too_many_arguments)]
    pub fn lay(
        &mut self,
        key: &Key,
        design: &LoopDesign,
        detune_ct: [f64; MAX_STRINGS],
        strings: usize,
        polar: usize,
        polar_yz: f64,
        strike_true: f64,
    ) {
        self.nv = strings.clamp(1, MAX_STRINGS);
        self.n = (self.nv + polar).min(MAX_STRINGS);
        self.polar_yz = polar_yz;
        self.loose_from = self.nv;
        self.z = key.impedance;
        self.sections = design.sections;
        for (i, (r, t)) in design.poles[..design.sections].iter().enumerate() {
            let (a1, a2) = second_coefs(*r, *t);
            self.a1[i] = a1;
            self.a2[i] = a2;
        }
        let total = design.delay.max(3);
        let nut = Self::nut_samples(key, design, strike_true);
        let to = ((total - 2 * nut) / 2).max(1);
        let from = (total - 2 * nut - to).max(1);
        self.bq = [[[0.0; 4]; MAX_STRINGS]; MAX_SECTIONS];
        self.nbm = 0;
        self.own = false;
        self.ls_on = false;
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
        self.g_h = g;
        self.alpha_h = alpha;
    }

    /// The loops from `i` on take the second loss filter (never below 1, never past the
    /// horizontal loops' start).
    pub fn set_loose_from(&mut self, i: usize) {
        self.loose_from = i.clamp(1, self.nv);
    }

    /// The horizontal loops' loss filter alone (after [`Self::set_loss`]).
    pub fn set_loss_polar(&mut self, g: f64, alpha: f64) {
        self.g_h = g;
        self.alpha_h = alpha;
    }

    /// Strike: the hammer of `mass` (kg, the whole hammer's) at `velocity` (m/s), string
    /// `i` struck at `weight[i]` of it (the soft pedal moves it off the last string).
    pub fn strike(&mut self, felt: Felt, mass: f64, velocity: f64, weight: [f64; MAX_STRINGS]) {
        self.felt = felt;
        let eps = felt.eps.clamp(0.0, 0.98);
        self.q0 = felt.k / (1.0 - eps);
        self.lambda = math::exp(-1.0 / (felt.tau.max(1e-7) * self.fs));
        // The hammer's mass is shared among the strings it strikes (the vertical ones).
        let n = self.nv.max(1) as f64;
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

    /// The force the hammers press on the strings now (N, summed): what model 2 carries
    /// into the frame as the blow (ADR 0098).
    #[inline]
    pub fn hammer_force(&self) -> f64 {
        self.strings[..self.n]
            .iter()
            .filter(|s| s.hammer.active)
            .map(|s| s.hammer.f)
            .sum()
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
        for m in self.bm.iter_mut() {
            m[4] = 0.0;
            m[5] = 0.0;
        }
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
        let loose = self.loose_from;
        for (i, s) in self.strings.iter_mut().enumerate().take(n) {
            let (g, alpha) = if i < loose {
                (self.g, self.alpha)
            } else {
                (self.g_h, self.alpha_h)
            };
            let (g, alpha) = if self.ls_on {
                let (a, b) = self.ls[i];
                ((1.0 - (1.0 - g) * (1.0 + a)).min(1.0), alpha * (1.0 + b))
            } else {
                (g, alpha)
            };
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
            let lossy = g * (alpha * (raw + s.fir[1]) + (1.0 - 2.0 * alpha) * s.fir[0]);
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
        let nv = self.nv;
        let mut sum = 0.0;
        for (s, x) in self.strings.iter_mut().zip(xs).take(nv) {
            s.arrived = x;
            sum += x;
        }
        // The bridge: every string's reflection, and the force on it.
        let v_b = if self.nbm == 0 {
            2.0 * self.yz * sum / (1.0 + nv as f64 * self.yz)
        } else {
            // Its admittance `YZ + Σ modes`: each mode's instantaneous share and its state,
            // solved with the strings' waves; then the modes take the force they get.
            let (mut c0, mut st) = (0.0, 0.0);
            for m in &self.bm[..self.nbm] {
                c0 += m[3] * self.yz * m[0];
                st += m[4];
            }
            let y = self.yz + c0;
            let v = (2.0 * y * sum + st) / (1.0 + nv as f64 * y);
            let w = 2.0 * sum - nv as f64 * v;
            for m in &mut self.bm[..self.nbm] {
                let b0 = m[3] * self.yz * m[0];
                let out = b0 * w + m[4];
                m[4] = -m[1] * out + m[5];
                m[5] = -b0 * w - m[2] * out;
            }
            v
        };
        for s in self.strings.iter_mut().take(nv) {
            s.from_bridge.write(v_b - s.arrived);
        }
        let mut force = self.z * (2.0 * sum - nv as f64 * v_b);
        // The horizontal loops, each on its own side of the bridge.
        if n > nv {
            let yz = self.yz * self.polar_yz;
            for (s, x) in self.strings.iter_mut().zip(xs).take(n).skip(nv) {
                s.arrived = x;
                let v_h = 2.0 * yz * x / (1.0 + yz);
                s.from_bridge.write(v_h - x);
                force += self.z * (2.0 * x - v_h);
            }
        }
        force
    }

    #[inline(always)]
    fn disperse<const N: usize>(&mut self, xs: &mut [f64; MAX_STRINGS]) {
        let secs = self.sections;
        if self.own {
            for (j, st) in self.bq[..secs].iter_mut().enumerate() {
                for i in 0..N {
                    let (a1, a2) = (self.a1s[i][j], self.a2s[i][j]);
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
            return;
        }
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

/// The fraction (the first-order allpass's coefficient) that gives back, at `w`, the phase a
/// string's dispersion sections take there when their pole angles are scattered by `e` — so
/// the loop's phase at its first partial, its tuning, is what the design had. `None` when no
/// fraction within ±0.99 can (or `w − θ` vanishes).
pub fn held_frac(design: &LoopDesign, frac: f64, w: f64, e: f64) -> Option<f64> {
    let mut dphi = 0.0;
    for (r, t) in design.poles[..design.sections].iter() {
        let t2 = (*t * (1.0 + e)).clamp(1e-6, std::f64::consts::PI - 1e-6);
        dphi += second_phase(w, *r, t2) - second_phase(w, *r, *t);
    }
    // `atan2(a sin w, 1 + a cos w) = θ` is `a = sin θ / sin(w − θ)`.
    let theta = 0.5 * (first_phase(w, frac) - dphi + w);
    let d = math::sin(w - theta);
    if d.abs() <= 1e-12 {
        return None;
    }
    let a = math::sin(theta) / d;
    (a.abs() <= 0.99).then_some(a)
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
    loss_shaped(key, fs, coupling, decay, damp1, damp2, trip, 1.0)
}

/// [`loss`] with the loss's rise above the first partial `steep` times the law's (1: the
/// law itself, to the bit): the highest partial below 4 kHz (the second, on the top keys)
/// losing `steep` times more than it does against the first, the first's decay kept exact
/// wherever the filter's gain may stay at or under 1 — on the very top keys the passive
/// filter takes some of the first partial too (round 40, candidate 7: the top keys' second
/// partials, in the 4–6 kHz band, sang on while their fundamentals were kept living).
pub fn loss_steep(
    key: &Key,
    fs: f64,
    coupling: f64,
    decay: f64,
    damp1: f64,
    damp2: f64,
    steep: f64,
) -> (f64, f64, f64) {
    loss_shaped(
        key,
        fs,
        coupling,
        decay,
        damp1,
        damp2,
        &|k| k as f64 / key.partial(k),
        steep,
    )
}

#[allow(clippy::too_many_arguments)]
fn loss_shaped(
    key: &Key,
    fs: f64,
    coupling: f64,
    decay: f64,
    damp1: f64,
    damp2: f64,
    trip: &dyn Fn(u32) -> f64,
    steep: f64,
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
    let r = if steep == 1.0 {
        math::exp(per_trip(kd) - per_trip(1))
    } else {
        math::exp(steep * (per_trip(kd) - per_trip(1)))
    };
    let alpha = if cd - r * c1 > 0.0 {
        ((1.0 - r) / (2.0 * (cd - r * c1))).clamp(0.0, 0.2499)
    } else {
        0.2499
    };
    let g = (math::exp(per_trip(1)) / (1.0 - 2.0 * alpha * c1)).min(1.0);
    (g, alpha, yz)
}
