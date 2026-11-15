//! Purus's de-hum: mains hum at 50 or 60 Hz (or found and followed) and its harmonics, with
//! no latency.
//!
//! **Notch** (the default): one second-order section per harmonic, `H = 1 − (1 − g)·BP`,
//! the band-pass's −3 dB width exactly the width set (`α = tan(π·width/fs)`) and `g` the
//! depth's gain at the centre — a true notch of that width as the depth goes deep, and a
//! constant width in hertz at every harmonic (hum's harmonics are exact multiples; the
//! tracker keeps them there). In `f64`: a 0.5 Hz notch at 50 Hz puts its poles 3·10⁻⁵ from
//! the unit circle.
//!
//! **Gentle**: the hum is *subtracted* instead — each harmonic's complex amplitude learned by
//! a least-mean-squares canceller against a reference at `k·f0` (Widrow's adaptive noise
//! canceller: `â += 2μ·e·z̄`, `μ = π·width/fs`, which alone is a notch of that width), and
//! the learning **held** whenever the band around the harmonic holds more than the hum it
//! has learned (a short-term estimate of the band, 25 ms, more than half the hum's level
//! away from the long-term one, and louder): a bass note on 50, 100 or 150 Hz then passes
//! untouched while the hum it sits on is still taken out, because hum does not change and
//! a note does. The long-term estimate is held with it, and starts from the short one 60 ms
//! after the hum is first heard, so learning a hum takes a fraction of a second.
//!
//! **Tracking**: a frequency-locked loop on the mono sum. Each of the first eight harmonics
//! is demodulated against the reference and low-passed (8 Hz, two poles); every 64 samples
//! the phase each turned through since the last look, divided by `k`, says how far the hum
//! is from the reference, the harmonics weighted by their strength; the reference moves a
//! share of that (a 0.25 s loop), never more than ±2 Hz from the nominal. **Auto** also
//! listens at the other mains frequency's first four harmonics and moves to it when it is
//! 6 dB stronger for a second.
//!
//! Every per-sample step is on a grid of the sample count, never of the block, so the output
//! does not depend on the block size. Parameter changes glide on that grid (depth, width,
//! a harmonic switched in or out fades through its own depth).

use crate::math;

/// The most harmonics (the fundamental counted).
pub const MAX_HARMONICS: usize = 16;
/// How far tracking may move the fundamental from the nominal, Hz.
pub const TRACK_RANGE_HZ: f64 = 2.0;
/// Samples between the tracker's looks and the coefficients' updates.
pub const GRID: usize = 64;
/// The harmonics the tracker listens to.
const TRACK_HARMONICS: usize = 8;
/// The harmonics Auto compares.
const AUTO_HARMONICS: usize = 4;

/// Where the hum's fundamental comes from.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HumMode {
    /// At the frequency set, exactly.
    Fixed,
    /// Followed within ±2 Hz of the frequency set.
    Track,
    /// 50 or 60 Hz, whichever is there, then followed.
    Auto,
}

/// The de-hum module's settings (real units).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HumSettings {
    pub on: bool,
    pub mode: HumMode,
    /// The fundamental (Fixed) or the nominal tracking starts from (Track), 40–70 Hz.
    /// Auto starts at 50 and chooses.
    pub freq_hz: f32,
    /// Harmonics taken out, the fundamental counted, 1–16 (above 0.45·fs are skipped).
    pub harmonics: usize,
    /// How deep each notch (or how much of the hum subtracted), dB, 0–80.
    pub depth_db: f32,
    /// Each notch's −3 dB width, Hz, 0.5–20.
    pub width_hz: f32,
    /// Subtract the learned hum, held under a bass note, instead of notching.
    pub gentle: bool,
    /// 0 (the input untouched, bit for bit) to 1.
    pub mix: f32,
    /// The module's output level, dB.
    pub out_db: f32,
    /// Listen to what this module takes out.
    pub delta: bool,
}

impl Default for HumSettings {
    fn default() -> Self {
        HumSettings {
            on: false,
            mode: HumMode::Fixed,
            freq_hz: 50.0,
            harmonics: 8,
            depth_db: 40.0,
            width_hz: 2.0,
            gentle: false,
            mix: 1.0,
            out_db: 0.0,
            delta: false,
        }
    }
}

impl HumSettings {
    /// Clamped into range.
    pub fn sane(&self) -> HumSettings {
        let f = |v: f32, lo: f32, hi: f32, d: f32| if v.is_finite() { v.clamp(lo, hi) } else { d };
        HumSettings {
            freq_hz: f(self.freq_hz, 40.0, 70.0, 50.0),
            harmonics: self.harmonics.clamp(1, MAX_HARMONICS),
            depth_db: f(self.depth_db, 0.0, 80.0, 40.0),
            width_hz: f(self.width_hz, 0.5, 20.0, 2.0),
            mix: f(self.mix, 0.0, 1.0, 1.0),
            out_db: f(self.out_db, -24.0, 24.0, 0.0),
            ..*self
        }
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct C {
    re: f64,
    im: f64,
}

impl C {
    const ZERO: C = C { re: 0.0, im: 0.0 };
    const ONE: C = C { re: 1.0, im: 0.0 };
    #[inline]
    fn mul(self, o: C) -> C {
        C {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
    #[inline]
    fn conj_scale(self, x: f64) -> C {
        C {
            re: self.re * x,
            im: -self.im * x,
        }
    }
    #[inline]
    fn lerp(&mut self, to: C, k: f64) {
        self.re += (to.re - self.re) * k;
        self.im += (to.im - self.im) * k;
    }
    #[inline]
    fn norm2(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
    #[inline]
    fn sub(self, o: C) -> C {
        C {
            re: self.re - o.re,
            im: self.im - o.im,
        }
    }
}

/// A notch's coefficients (normalised by `a0`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Coef {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

fn notch(f_hz: f64, width_hz: f64, g: f64, fs: f64) -> Coef {
    let w0 = 2.0 * std::f64::consts::PI * f_hz / fs;
    let alpha = math::tan(std::f64::consts::PI * width_hz / fs);
    let c = math::cos(w0);
    let a0 = 1.0 + alpha;
    Coef {
        b0: (1.0 + g * alpha) / a0,
        b1: -2.0 * c / a0,
        b2: (1.0 - g * alpha) / a0,
        a1: -2.0 * c / a0,
        a2: (1.0 - alpha) / a0,
    }
}

/// One channel's running state.
#[derive(Clone, Debug, Default, PartialEq)]
struct Lane {
    /// Notch sections (transposed direct form II).
    s1: [f64; MAX_HARMONICS],
    s2: [f64; MAX_HARMONICS],
    /// Gentle: the learned hum, the band's short-term amplitude (two poles), its long-term
    /// one, whether learning is held and for how many more samples.
    est: [C; MAX_HARMONICS],
    fast1: [C; MAX_HARMONICS],
    fast2: [C; MAX_HARMONICS],
    slow: [C; MAX_HARMONICS],
    held: [bool; MAX_HARMONICS],
    hold_left: [u32; MAX_HARMONICS],
    /// Samples since this lane first heard something (to seed the long-term estimate).
    heard: u32,
}

/// The de-hum module.
#[derive(Clone, Debug)]
pub struct Dehum {
    fs: f64,
    s: HumSettings,
    /// The nominal the tracker centres on, and the fundamental now.
    nominal: f64,
    f0: f64,
    /// The reference: `e^{jθ}` at the fundamental, and its step.
    z1: C,
    step: C,
    /// Auto's other mains frequency's reference.
    za: C,
    step_a: C,
    /// The tracker's demodulators (two poles), last look, and Auto's.
    q1: [C; TRACK_HARMONICS],
    q2: [C; TRACK_HARMONICS],
    q_prev: [C; TRACK_HARMONICS],
    qa1: [C; AUTO_HARMONICS],
    qa2: [C; AUTO_HARMONICS],
    auto_votes: u32,
    auto_looks: u32,
    /// Grid position (samples since the last look).
    tick: usize,
    /// Per-harmonic depth gain and width now (gliding to their targets), and the coefficients
    /// designed from them.
    g: [f64; MAX_HARMONICS],
    width: f64,
    coef: [Coef; MAX_HARMONICS],
    lanes: [Lane; 2],
}

impl Dehum {
    pub fn new(sample_rate: f32) -> Self {
        let mut d = Dehum {
            fs: sample_rate as f64,
            s: HumSettings::default(),
            nominal: 50.0,
            f0: 50.0,
            z1: C::ONE,
            step: C::ONE,
            za: C::ONE,
            step_a: C::ONE,
            q1: [C::ZERO; TRACK_HARMONICS],
            q2: [C::ZERO; TRACK_HARMONICS],
            q_prev: [C::ZERO; TRACK_HARMONICS],
            qa1: [C::ZERO; AUTO_HARMONICS],
            qa2: [C::ZERO; AUTO_HARMONICS],
            auto_votes: 0,
            auto_looks: 0,
            tick: 0,
            g: [1.0; MAX_HARMONICS],
            width: 2.0,
            coef: [Coef::default(); MAX_HARMONICS],
            lanes: [Lane::default(), Lane::default()],
        };
        d.reset();
        d
    }
}

/// The two-pole demodulators' corner, Hz (the tracker and the gate's short-term band).
const DEMOD_HZ: f64 = 8.0;
/// The gate's long-term estimate's time constant, s.
const SLOW_S: f64 = 1.0;
/// The tracker's loop time constant, s.
const LOOP_S: f64 = 0.25;
/// Gentle: when the long-term estimate is seeded from the short one, s after the first sound.
const SEED_S: f64 = 0.06;
/// Gentle: how long learning stays held after the band is clear again, s.
const HOLD_S: f64 = 0.05;
/// Gentle: a band this far from the hum learned (relative amplitude) holds the learning.
const GATE_REL: f64 = 0.5;
/// Gentle: below this amplitude nothing holds the learning (−100 dBFS).
const GATE_FLOOR: f64 = 1e-5;

impl Dehum {
    pub fn settings(&self) -> &HumSettings {
        &self.s
    }

    fn nominal_of(s: &HumSettings) -> f64 {
        match s.mode {
            HumMode::Auto => 50.0,
            _ => s.freq_hz as f64,
        }
    }

    fn g_target(&self, k: usize) -> f64 {
        if k < self.s.harmonics && (k + 1) as f64 * self.f0 < 0.45 * self.fs {
            math::db_to_gain(-self.s.depth_db) as f64
        } else {
            1.0
        }
    }

    /// New settings. The depth and width glide; a new nominal moves the fundamental at once.
    pub fn set(&mut self, s: &HumSettings) {
        let s = s.sane();
        let old = self.s;
        self.s = s;
        let nominal_moved =
            old.mode != s.mode || (s.mode != HumMode::Auto && old.freq_hz != s.freq_hz);
        if nominal_moved {
            self.nominal = Self::nominal_of(&s);
            self.f0 = self.nominal;
            self.retune();
        }
        if old.gentle != s.gentle {
            self.lanes = [Lane::default(), Lane::default()];
        }
    }

    /// Everything to a new unit's state with the settings in force: parameters landed,
    /// filters empty, the reference at phase 0 on the nominal.
    pub fn reset(&mut self) {
        self.nominal = Self::nominal_of(&self.s);
        self.f0 = self.nominal;
        self.z1 = C::ONE;
        self.za = C::ONE;
        self.q1 = [C::ZERO; TRACK_HARMONICS];
        self.q2 = [C::ZERO; TRACK_HARMONICS];
        self.q_prev = [C::ZERO; TRACK_HARMONICS];
        self.qa1 = [C::ZERO; AUTO_HARMONICS];
        self.qa2 = [C::ZERO; AUTO_HARMONICS];
        self.auto_votes = 0;
        self.auto_looks = 0;
        self.tick = 0;
        self.lanes = [Lane::default(), Lane::default()];
        for k in 0..MAX_HARMONICS {
            self.g[k] = self.g_target(k);
        }
        self.width = self.s.width_hz as f64;
        self.retune();
    }

    /// The depth and width straight to their targets (a unit at rest: nothing sounds).
    pub fn land(&mut self) {
        for k in 0..MAX_HARMONICS {
            self.g[k] = self.g_target(k);
        }
        self.width = self.s.width_hz as f64;
        self.retune();
    }

    /// The reference steps and every section's coefficients from `f0`, `g` and `width`.
    fn retune(&mut self) {
        let w = 2.0 * std::f64::consts::PI * self.f0 / self.fs;
        let (s, c) = math::sin_cos(w);
        self.step = C { re: c, im: s };
        let alt = if self.nominal == 60.0 { 50.0 } else { 60.0 };
        let (s, c) = math::sin_cos(2.0 * std::f64::consts::PI * alt / self.fs);
        self.step_a = C { re: c, im: s };
        for k in 0..MAX_HARMONICS {
            let f = (k + 1) as f64 * self.f0;
            self.coef[k] = if f < 0.49 * self.fs {
                notch(f, self.width, self.g[k], self.fs)
            } else {
                Coef::default()
            };
        }
    }

    /// The fundamental now, Hz.
    pub fn f0(&self) -> f64 {
        self.f0
    }

    /// The mains frequency the tracker centres on (Auto's choice), Hz.
    pub fn nominal(&self) -> f64 {
        self.nominal
    }

    /// Harmonic `k`'s (0 = the fundamental) level in the input as the tracker hears it
    /// (its band, 8 Hz), linear amplitude.
    pub fn level(&self, k: usize) -> f64 {
        self.q2.get(k).map_or(0.0, |q| q.norm2().sqrt())
    }

    /// Harmonic `k`'s depth now, dB (0 when it is not taken out).
    pub fn depth_now_db(&self, k: usize) -> f64 {
        20.0 * math::log10(self.g.get(k).copied().unwrap_or(1.0).max(1e-12))
    }

    /// Whether gentle learning is held at harmonic `k` on channel `ch` (a note is there).
    pub fn held(&self, ch: usize, k: usize) -> bool {
        self.s.gentle && self.lanes[ch.min(1)].held[k.min(MAX_HARMONICS - 1)]
    }

    /// The notches' magnitude response at `hz` (what the display draws; in gentle mode the
    /// notch the canceller is equivalent to while it learns).
    pub fn response(&self, hz: f64) -> f64 {
        let w = 2.0 * std::f64::consts::PI * hz / self.fs;
        let (s1, c1) = math::sin_cos(w);
        let (s2, c2) = math::sin_cos(2.0 * w);
        let mut m = 1.0;
        for k in 0..MAX_HARMONICS {
            if self.g[k] >= 1.0 {
                continue;
            }
            let c = self.coef[k];
            let nr = c.b0 + c.b1 * c1 + c.b2 * c2;
            let ni = -(c.b1 * s1 + c.b2 * s2);
            let dr = 1.0 + c.a1 * c1 + c.a2 * c2;
            let di = -(c.a1 * s1 + c.a2 * s2);
            m *= ((nr * nr + ni * ni) / (dr * dr + di * di)).sqrt();
        }
        m
    }

    /// Whether every state that reaches the output is under `thr` in magnitude (the unit's
    /// drain asks before it empties the module).
    pub fn quiet_below(&self, thr: f64) -> bool {
        let t2 = thr * thr;
        self.lanes.iter().all(|l| {
            l.s1.iter().chain(l.s2.iter()).all(|v| v.abs() < thr)
                && l.est.iter().all(|c| c.norm2() < t2)
        })
    }

    /// The grid step: glide the parameters, follow the hum, redesign what moved.
    fn on_grid(&mut self) {
        let mut moved = false;
        for k in 0..MAX_HARMONICS {
            let t = self.g_target(k);
            if self.g[k] != t {
                let n = self.g[k] + (t - self.g[k]) * 0.25;
                self.g[k] = if (n - t).abs() < 1e-7 { t } else { n };
                moved = true;
                if self.g[k] == 1.0 {
                    for l in &mut self.lanes {
                        l.s1[k] = 0.0;
                        l.s2[k] = 0.0;
                        l.est[k] = C::ZERO;
                    }
                }
            }
        }
        let wt = self.s.width_hz as f64;
        if self.width != wt {
            let n = self.width + (wt - self.width) * 0.25;
            self.width = if (n - wt).abs() < 1e-6 { wt } else { n };
            moved = true;
        }
        if self.s.mode != HumMode::Fixed {
            moved |= self.follow();
        }
        if self.s.mode == HumMode::Auto {
            moved |= self.choose_mains();
        }
        if moved {
            self.retune();
        }
    }

    /// The frequency-locked loop's step; whether the fundamental moved.
    fn follow(&mut self) -> bool {
        let mut acc = 0.0;
        let mut wsum = 0.0;
        for k in 0..TRACK_HARMONICS {
            let q = self.q2[k];
            let p = self.q_prev[k];
            self.q_prev[k] = q;
            if (k + 1) as f64 * self.f0 >= 0.45 * self.fs {
                continue;
            }
            let r = q.mul(C { re: p.re, im: -p.im });
            let w = r.norm2().sqrt();
            if w <= 0.0 {
                continue;
            }
            let phi = math::atan2(r.im, r.re);
            let df = phi * self.fs / (2.0 * std::f64::consts::PI * (k + 1) as f64 * GRID as f64);
            acc += w * df;
            wsum += w;
        }
        if wsum < 1e-12 {
            return false;
        }
        let gain = GRID as f64 / (self.fs * LOOP_S);
        let f = (self.f0 + gain * acc / wsum)
            .clamp(self.nominal - TRACK_RANGE_HZ, self.nominal + TRACK_RANGE_HZ);
        let moved = f != self.f0;
        self.f0 = f;
        moved
    }

    /// Auto: move to the other mains frequency when it is 6 dB stronger at two looks half
    /// a second apart; whether it moved.
    fn choose_mains(&mut self) -> bool {
        let every = ((0.5 * self.fs) / GRID as f64).round().max(1.0) as u32;
        self.auto_votes += 1;
        if self.auto_votes < every {
            return false;
        }
        self.auto_votes = 0;
        let main: f64 = self.q2[..AUTO_HARMONICS].iter().map(|q| q.norm2()).sum();
        let alt: f64 = self.qa2.iter().map(|q| q.norm2()).sum();
        if !(alt > 4.0 * main && alt > 1e-12) {
            self.auto_looks = 0;
            return false;
        }
        self.auto_looks += 1;
        if self.auto_looks < 2 {
            return false;
        }
        self.nominal = if self.nominal == 60.0 { 50.0 } else { 60.0 };
        self.f0 = self.nominal;
        self.za = C::ONE;
        self.q1 = [C::ZERO; TRACK_HARMONICS];
        self.q2 = [C::ZERO; TRACK_HARMONICS];
        self.q_prev = [C::ZERO; TRACK_HARMONICS];
        self.qa1 = [C::ZERO; AUTO_HARMONICS];
        self.qa2 = [C::ZERO; AUTO_HARMONICS];
        self.auto_looks = 0;
        for l in &mut self.lanes {
            l.est = [C::ZERO; MAX_HARMONICS];
            l.slow = [C::ZERO; MAX_HARMONICS];
            l.heard = 0;
        }
        true
    }

    /// One stereo sample: the hum taken out of each channel (before the module's mix).
    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.tick == 0 {
            self.on_grid();
        }
        self.tick += 1;
        if self.tick == GRID {
            self.tick = 0;
        }
        // The reference's harmonics, this sample.
        let mut z = [C::ZERO; MAX_HARMONICS];
        z[0] = self.z1;
        for k in 1..MAX_HARMONICS {
            z[k] = z[k - 1].mul(self.z1);
        }
        // The tracker and the meters, on the mono sum.
        let m = 0.5 * (l as f64 + r as f64);
        let cd = 1.0 - math::exp(-2.0 * std::f64::consts::PI * DEMOD_HZ / self.fs);
        for k in 0..TRACK_HARMONICS {
            let d = z[k].conj_scale(2.0 * m);
            self.q1[k].lerp(d, cd);
            let q1 = self.q1[k];
            self.q2[k].lerp(q1, cd);
        }
        if self.s.mode == HumMode::Auto {
            let mut za = self.za;
            for k in 0..AUTO_HARMONICS {
                let d = za.conj_scale(2.0 * m);
                self.qa1[k].lerp(d, cd);
                let q1 = self.qa1[k];
                self.qa2[k].lerp(q1, cd);
                za = za.mul(self.za);
            }
            self.za = renorm(self.za.mul(self.step_a));
        }
        let out = if self.s.gentle {
            (self.gentle(0, l as f64, &z, cd), self.gentle(1, r as f64, &z, cd))
        } else {
            (self.notches(0, l as f64), self.notches(1, r as f64))
        };
        self.z1 = renorm(self.z1.mul(self.step));
        (out.0 as f32, out.1 as f32)
    }

    #[inline]
    fn notches(&mut self, ch: usize, x: f64) -> f64 {
        let lane = &mut self.lanes[ch];
        let mut y = x;
        for k in 0..MAX_HARMONICS {
            if self.g[k] >= 1.0 {
                continue;
            }
            let c = &self.coef[k];
            let v = c.b0 * y + lane.s1[k];
            lane.s1[k] = c.b1 * y - c.a1 * v + lane.s2[k];
            lane.s2[k] = c.b2 * y - c.a2 * v;
            y = v;
        }
        y
    }

    #[inline]
    fn gentle(&mut self, ch: usize, x: f64, z: &[C; MAX_HARMONICS], cd: f64) -> f64 {
        let fs = self.fs;
        let mu = std::f64::consts::PI * self.width / fs;
        let cs = 1.0 - math::exp(-1.0 / (SLOW_S * fs));
        let seed = (SEED_S * fs) as u32;
        let hold = (HOLD_S * fs) as u32;
        let lane = &mut self.lanes[ch];
        let mut hum = 0.0;
        let mut taken = 0.0;
        for k in 0..MAX_HARMONICS {
            if self.g[k] >= 1.0 {
                continue;
            }
            let h = lane.est[k].re * z[k].re - lane.est[k].im * z[k].im;
            hum += h;
            taken += (1.0 - self.g[k]) * h;
        }
        let e = x - hum;
        if lane.heard <= seed && (lane.heard > 0 || x != 0.0) {
            lane.heard += 1;
        }
        for k in 0..MAX_HARMONICS {
            if self.g[k] >= 1.0 {
                continue;
            }
            let d = z[k].conj_scale(2.0 * x);
            lane.fast1[k].lerp(d, cd);
            let f1 = lane.fast1[k];
            lane.fast2[k].lerp(f1, cd);
            if lane.heard == seed {
                lane.slow[k] = lane.fast2[k];
            }
            let f = lane.fast2[k];
            let s = lane.slow[k];
            let s2 = s.norm2();
            let off = f.sub(s).norm2();
            let lim = GATE_REL * s2.sqrt() + GATE_FLOOR;
            let busy = lane.heard > seed && off > lim * lim && f.norm2() > s2;
            if busy {
                lane.held[k] = true;
                lane.hold_left[k] = hold;
            } else if lane.hold_left[k] > 0 {
                lane.hold_left[k] -= 1;
            } else {
                lane.held[k] = false;
            }
            if !lane.held[k] {
                let u = z[k].conj_scale(2.0 * mu * e);
                lane.est[k].re += u.re;
                lane.est[k].im += u.im;
                if lane.heard > seed {
                    lane.slow[k].lerp(d, cs);
                }
            }
        }
        x - taken
    }
}

/// Keep a unit phasor on the circle (one Newton step; exact enough every sample).
#[inline]
fn renorm(z: C) -> C {
    let k = 1.5 - 0.5 * z.norm2();
    C {
        re: z.re * k,
        im: z.im * k,
    }
}
