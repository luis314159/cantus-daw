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
//! the learning **held** while a note sits on the harmonic, so a bass note on 50, 100 or
//! 150 Hz passes untouched while the hum it sits on is still taken out (hum does not change;
//! a note does). The canceller works **[`AHEAD_S`] (50 ms) late** — on the input and the
//! reference as they were — while the gate listens to the input as it is, so a note holds
//! the learning before it reaches the canceller (the mode's latency). The gate: a three-pole
//! 3 Hz band of what the hum learned does not explain, above half the hum's level (or twice
//! the band's usual remainder, which rises over 4 s, so a hum that changed is learned
//! again); the hum's level is kept smoothed at 0.5 Hz (what a hold goes back to, 40–60 ms
//! of learning earlier); held harmonics turn with the free ones (the reference follows the
//! hum's frequency, not its phase); the tracker's loop is slower in this mode. Only the
//! bass range (to 160 Hz) is held; above, the canceller always learns (a notch).
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
/// The gentle mode's look-ahead, s: a note is heard this long before it reaches the
/// canceller, so learning is held before the note can be learned.
pub const AHEAD_S: f32 = 0.05;

/// The gentle mode's look-ahead at a sample rate, samples (its latency).
pub fn ahead_for(sample_rate: f32) -> usize {
    (AHEAD_S * sample_rate).round() as usize
}

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
    /// Gentle: the hum learned (each harmonic's complex amplitude), the same smoothed (two
    /// poles), and three snapshots of that (the oldest 40–60 ms old, to unlearn a note's
    /// first milliseconds).
    est: [C; MAX_HARMONICS],
    s_a: [C; MAX_HARMONICS],
    s_b: [C; MAX_HARMONICS],
    back_a: [C; MAX_HARMONICS],
    back_b: [C; MAX_HARMONICS],
    back_c: [C; MAX_HARMONICS],
    /// What the hum learned does not explain, in each harmonic's band (three poles).
    e1: [C; MAX_HARMONICS],
    e2: [C; MAX_HARMONICS],
    e3: [C; MAX_HARMONICS],
    /// Whether learning is held, for how many more samples, how long the band has been
    /// busy without a break.
    held: [bool; MAX_HARMONICS],
    /// The band's usual remainder.
    floor: [f64; MAX_HARMONICS],
    /// The smoothed estimates at the last grid look (the held ones turn with the free).
    turn_prev: [C; MAX_HARMONICS],
    hold_left: [u32; MAX_HARMONICS],
    busy_run: [u32; MAX_HARMONICS],
    /// Samples since this lane first heard something (it learns freely at first); each
    /// harmonic's learning time since its last snapshot.
    heard: u32,
    snap: [u32; MAX_HARMONICS],
}

/// The de-hum module.
#[derive(Clone, Debug)]
pub struct Dehum {
    fs: f64,
    s: HumSettings,
    /// The nominal the tracker centres on, and the fundamental now.
    nominal: f64,
    f0: f64,
    /// The loop's state: the frequency loop's rate of change of the fundamental (Hz per grid
    /// step), or the phase-locked loop's integrator (Hz from the nominal); and the phase the
    /// hum has turned from the reference (the phase-locked loop's error, radians).
    rate: f64,
    phase: f64,
    /// The reference: `e^{jθ}` at the fundamental, and its step.
    z1: C,
    step: C,
    /// Auto's other mains frequency's reference.
    za: C,
    step_a: C,
    /// The tracker's demodulators (two poles), last look, and Auto's.
    q1: [C; MAX_HARMONICS],
    q2: [C; MAX_HARMONICS],
    q3: [C; MAX_HARMONICS],
    q_prev: [C; MAX_HARMONICS],
    qa1: [C; AUTO_HARMONICS],
    qa2: [C; AUTO_HARMONICS],
    qa3: [C; AUTO_HARMONICS],
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
    /// Gentle: the look-ahead (samples) and its rings — the input and the reference, as
    /// they were.
    ahead: usize,
    ring_l: Vec<f32>,
    ring_r: Vec<f32>,
    ring_z: Vec<C>,
    rpos: usize,
    /// Per-sample constants: the demodulators' and the gate's one-pole steps, the
    /// canceller's rate (from the width now), and the gentle mode's lengths in samples.
    cd: f64,
    cg: f64,
    cr: f64,
    /// Gentle: the harmonics (from the fundamental) whose learning a note may hold — those
    /// up to [`GENTLE_TOP_HZ`]; above, the canceller learns always (a notch of the width).
    gated: usize,
    c_up: f64,
    c_down: f64,
    mu: f64,
    learn_len: u32,
    hold_len: u32,
    escape_len: u32,
    snap_len: u32,
}

impl Dehum {
    pub fn new(sample_rate: f32) -> Self {
        let fs = sample_rate as f64;
        let ahead = ahead_for(sample_rate);
        let ring = (ahead + 1).next_power_of_two();
        let mut d = Dehum {
            fs,
            s: HumSettings::default(),
            nominal: 50.0,
            f0: 50.0,
            rate: 0.0,
            phase: 0.0,
            z1: C::ONE,
            step: C::ONE,
            za: C::ONE,
            step_a: C::ONE,
            q1: [C::ZERO; MAX_HARMONICS],
            q2: [C::ZERO; MAX_HARMONICS],
            q3: [C::ZERO; MAX_HARMONICS],
            q_prev: [C::ZERO; MAX_HARMONICS],
            qa1: [C::ZERO; AUTO_HARMONICS],
            qa2: [C::ZERO; AUTO_HARMONICS],
            qa3: [C::ZERO; AUTO_HARMONICS],
            auto_votes: 0,
            auto_looks: 0,
            tick: 0,
            g: [1.0; MAX_HARMONICS],
            width: 2.0,
            coef: [Coef::default(); MAX_HARMONICS],
            lanes: [Lane::default(), Lane::default()],
            ahead,
            ring_l: vec![0.0; ring],
            ring_r: vec![0.0; ring],
            ring_z: vec![C::ZERO; ring],
            rpos: 0,
            cd: 1.0 - math::exp(-2.0 * std::f64::consts::PI * DEMOD_HZ / fs),
            cg: 1.0 - math::exp(-2.0 * std::f64::consts::PI * GATE_HZ / fs),
            cr: 1.0 - math::exp(-2.0 * std::f64::consts::PI * REF_HZ / fs),
            gated: 0,
            c_up: 1.0 - math::exp(-1.0 / (FLOOR_UP_S * fs)),
            c_down: 1.0 - math::exp(-1.0 / (FLOOR_DOWN_S * fs)),
            mu: 0.0,
            learn_len: (LEARN_S * fs) as u32,
            hold_len: (HOLD_S * fs) as u32,
            escape_len: (ESCAPE_S * fs) as u32,
            snap_len: ((SNAP_S * fs) as u32).max(1),
        };
        d.reset();
        d
    }
}

/// The tracker's (and the meters') three-pole demodulators' corner, Hz.
const DEMOD_HZ: f64 = 3.0;
/// Gentle: the three-pole band of what the hum learned does not explain, Hz (a note more
/// than about 10 Hz from a harmonic does not hold it).
const GATE_HZ: f64 = 3.0;
/// Gentle: the top of the bass range it protects, Hz. A note is held for only up to here:
/// higher, chords crowd the harmonics (their partials 20–40 Hz away leak into a band fast
/// enough to catch a note's onset) and the canceller learns always, a notch of the width.
const GENTLE_TOP_HZ: f64 = 160.0;
/// Gentle: the smoothing of the hum learned (two poles), Hz.
const REF_HZ: f64 = 0.5;
/// The tracker's loop: its natural frequency, Hz, and damping.
const LOOP_HZ: f64 = 0.5;
const LOOP_DAMPING: f64 = 0.8;
/// Gentle: a tracker reading counts half when what the hum does not explain in its band is
/// this share of the hum (and less as it grows).
const TRUST_REL: f64 = 0.2;
/// The gentle mode's phase-locked loop: its natural frequency, Hz.
const PLL_HZ: f64 = 0.1;
/// The readings the loop averages: those within this of their weighted median, Hz.
const TRIM_HZ: f64 = 0.3;
/// Gentle: how long it learns freely (twice as fast, no gate) after the first sound, s.
const LEARN_S: f64 = 0.3;
/// Gentle: how long learning stays held after the band is clear again, s.
const HOLD_S: f64 = 0.05;
/// Gentle: a band busy this long without a break is taken for a hum that changed, and
/// learned slowly (a sixteenth of the rate), s.
const ESCAPE_S: f64 = 3.0;
/// Gentle: the snapshots' period, s.
const SNAP_S: f64 = 0.02;
/// Gentle: what the hum learned does not explain, this much of it (relative amplitude),
/// holds the learning.
const GATE_REL: f64 = 0.5;
/// Gentle: the band's usual remainder is risen to over this, s, fallen from over this, s,
/// and a note must stand this many times above it.
const FLOOR_UP_S: f64 = 4.0;
const FLOOR_DOWN_S: f64 = 0.05;
const FLOOR_TIMES: f64 = 2.0;
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
            self.rate = 0.0;
            self.phase = 0.0;
            self.retune();
        }
        if old.gentle != s.gentle {
            self.lanes = [Lane::default(), Lane::default()];
            self.clear_ahead();
            // The loops' states mean other things: carried over continuously.
            self.phase = 0.0;
            self.rate = if s.gentle {
                self.f0 - self.nominal
            } else {
                0.0
            };
        }
    }

    /// Everything to a new unit's state with the settings in force: parameters landed,
    /// filters empty, the reference at phase 0 on the nominal.
    pub fn reset(&mut self) {
        self.nominal = Self::nominal_of(&self.s);
        self.f0 = self.nominal;
        self.rate = 0.0;
        self.phase = 0.0;
        self.z1 = C::ONE;
        self.za = C::ONE;
        self.q1 = [C::ZERO; MAX_HARMONICS];
        self.q2 = [C::ZERO; MAX_HARMONICS];
        self.q3 = [C::ZERO; MAX_HARMONICS];
        self.q_prev = [C::ZERO; MAX_HARMONICS];
        self.qa1 = [C::ZERO; AUTO_HARMONICS];
        self.qa2 = [C::ZERO; AUTO_HARMONICS];
        self.qa3 = [C::ZERO; AUTO_HARMONICS];
        self.auto_votes = 0;
        self.auto_looks = 0;
        self.tick = 0;
        self.lanes = [Lane::default(), Lane::default()];
        self.clear_ahead();
        for k in 0..MAX_HARMONICS {
            self.g[k] = self.g_target(k);
        }
        self.width = self.s.width_hz as f64;
        self.retune();
    }

    fn clear_ahead(&mut self) {
        self.ring_l.fill(0.0);
        self.ring_r.fill(0.0);
        self.ring_z.fill(C::ZERO);
        self.rpos = 0;
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
        self.mu = std::f64::consts::PI * self.width / self.fs;
        self.gated = ((GENTLE_TOP_HZ / self.f0).floor() as usize).min(MAX_HARMONICS);
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
        self.q3.get(k).map_or(0.0, |q| q.norm2().sqrt())
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
        if self.s.gentle {
            for ch in 0..2 {
                self.turn_held(ch);
            }
        }
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

    /// Gentle: turn the held harmonics with the free ones. The reference follows the hum's
    /// frequency, not its phase, so against a drifting hum the phase between them wanders;
    /// a free harmonic's canceller turns with it on its own, a held one cannot. How far the
    /// free ones turned since the last look (each angle over its harmonic number, weighted
    /// by its strength) turns the held ones, each by its harmonic number.
    fn turn_held(&mut self, ch: usize) {
        let lane = &mut self.lanes[ch];
        let (mut acc, mut w) = (0.0f64, 0.0f64);
        let mut any_held = false;
        for k in 0..MAX_HARMONICS {
            if self.g[k] >= 1.0 {
                continue;
            }
            let now = lane.s_b[k];
            if lane.held[k] {
                any_held = true;
                continue;
            }
            let was = lane.turn_prev[k];
            let r = now.mul(C {
                re: was.re,
                im: -was.im,
            });
            let m = r.norm2().sqrt();
            if m > 0.0 {
                acc += m * math::atan2(r.im, r.re) / (k + 1) as f64;
                w += m;
            }
        }
        if any_held && w > 0.0 && lane.heard >= self.learn_len {
            let phi = acc / w;
            for k in 0..MAX_HARMONICS {
                if !lane.held[k] || self.g[k] >= 1.0 {
                    continue;
                }
                let (s, c) = math::sin_cos(phi * (k + 1) as f64);
                let t = C { re: c, im: s };
                lane.est[k] = lane.est[k].mul(t);
                lane.s_a[k] = lane.s_a[k].mul(t);
                lane.s_b[k] = lane.s_b[k].mul(t);
                lane.back_a[k] = lane.back_a[k].mul(t);
                lane.back_b[k] = lane.back_b[k].mul(t);
                lane.back_c[k] = lane.back_c[k].mul(t);
            }
        }
        lane.turn_prev = lane.s_b;
    }

    /// The frequency-locked loop's step; whether the fundamental moved.
    /// The frequency the phase-locked loop's integrator is reckoned from.
    fn nominal_f(&self) -> f64 {
        self.nominal
    }

    fn follow(&mut self) -> bool {
        // Each harmonic's own reading of the hum's offset from the reference, Hz, and its
        // weight (its band's strength); the reading is their weighted median, so a
        // neighbouring tone leaking into one band cannot pull the loop.
        let mut df = [0.0f64; TRACK_HARMONICS];
        let mut w = [0.0f64; TRACK_HARMONICS];
        let mut n = 0;
        for k in 0..TRACK_HARMONICS {
            let q = self.q3[k];
            let p = self.q_prev[k];
            self.q_prev[k] = q;
            if (k + 1) as f64 * self.f0 >= 0.45 * self.fs {
                continue;
            }
            // Gentle: a harmonic a note holds tells of the note, not of the hum.
            if self.s.gentle && (self.lanes[0].held[k] || self.lanes[1].held[k]) {
                continue;
            }
            let r = q.mul(C {
                re: p.re,
                im: -p.im,
            });
            let m = r.norm2().sqrt();
            if m <= 0.0 {
                continue;
            }
            let phi = math::atan2(r.im, r.re);
            df[n] = phi * self.fs / (2.0 * std::f64::consts::PI * (k + 1) as f64 * GRID as f64);
            // Gentle: a band the hum learned explains well is trusted; one where something
            // else sounds (what the hum does not explain is large) much less.
            w[n] = if self.s.gentle {
                let mut c = 0.0;
                for l in &self.lanes {
                    let rel = l.e3[k].norm2() / (TRUST_REL * TRUST_REL * l.s_b[k].norm2() + 1e-24);
                    c += 0.5 / (1.0 + rel);
                }
                m * c
            } else {
                m
            };
            n += 1;
        }
        let total: f64 = w[..n].iter().sum();
        if total < 1e-12 {
            return false;
        }
        for i in 1..n {
            let mut j = i;
            while j > 0 && df[j - 1] > df[j] {
                df.swap(j - 1, j);
                w.swap(j - 1, j);
                j -= 1;
            }
        }
        let mut acc = 0.0;
        let mut median = df[n - 1];
        for i in 0..n {
            acc += w[i];
            if acc >= 0.5 * total {
                median = df[i];
                break;
            }
        }
        // The weighted mean of the readings near the median: robust, and without the
        // median's own bias (which reading it picks moves with each band's ripple).
        let (mut sum, mut ws) = (0.0, 0.0);
        for i in 0..n {
            if (df[i] - median).abs() <= TRIM_HZ {
                sum += w[i] * df[i];
                ws += w[i];
            }
        }
        let e = sum / ws;
        let lo = self.nominal - TRACK_RANGE_HZ;
        let hi = self.nominal + TRACK_RANGE_HZ;
        let t = GRID as f64 / self.fs;
        let mut f = if self.s.gentle {
            // The gentle mode holds estimates against the reference for seconds: the
            // reference is **phase-locked** to the hum. The phase the hum has turned from it
            // (the readings summed) is driven back to zero by a second-order loop, so the
            // phase between them stays put instead of wandering as the frequency's errors
            // add up.
            self.phase += 2.0 * std::f64::consts::PI * e * t;
            let wn = 2.0 * std::f64::consts::PI * PLL_HZ;
            let kp = 2.0 * LOOP_DAMPING * wn / (2.0 * std::f64::consts::PI);
            let ki = wn * wn * t / (2.0 * std::f64::consts::PI);
            self.rate += ki * self.phase;
            self.nominal_f() + self.rate + kp * self.phase
        } else {
            // A second-order frequency loop (a ramp followed without a lag).
            let wt = 2.0 * std::f64::consts::PI * LOOP_HZ * t;
            let g1 = 2.0 * LOOP_DAMPING * wt;
            let g2 = wt * wt;
            let f = self.f0 + g1 * e + self.rate;
            self.rate += g2 * e;
            f
        };
        if f <= lo || f >= hi {
            f = f.clamp(lo, hi);
            self.rate = if self.s.gentle {
                f - self.nominal_f()
            } else {
                0.0
            };
            self.phase = 0.0;
        }
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
        let main: f64 = self.q3[..AUTO_HARMONICS].iter().map(|q| q.norm2()).sum();
        let alt: f64 = self.qa3.iter().map(|q| q.norm2()).sum();
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
        self.rate = 0.0;
        self.phase = 0.0;
        self.za = C::ONE;
        self.q1 = [C::ZERO; MAX_HARMONICS];
        self.q2 = [C::ZERO; MAX_HARMONICS];
        self.q3 = [C::ZERO; MAX_HARMONICS];
        self.q_prev = [C::ZERO; MAX_HARMONICS];
        self.qa1 = [C::ZERO; AUTO_HARMONICS];
        self.qa2 = [C::ZERO; AUTO_HARMONICS];
        self.qa3 = [C::ZERO; AUTO_HARMONICS];
        self.auto_looks = 0;
        // What was learned at the old frequency means nothing at the new one.
        let notch = self.lanes.clone();
        self.lanes = [Lane::default(), Lane::default()];
        for (l, n) in self.lanes.iter_mut().zip(notch) {
            l.s1 = n.s1;
            l.s2 = n.s2;
        }
        true
    }

    /// One stereo sample: the hum taken out of each channel (before the module's mix).
    #[inline]
    // The harmonics' (bins') arrays run in step: an index reads clearest.
    #[allow(clippy::needless_range_loop)]
    pub fn process(&mut self, l: f32, r: f32) -> ((f32, f32), (f32, f32)) {
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
        let cd = self.cd;
        for k in 0..MAX_HARMONICS {
            let d = z[k].conj_scale(2.0 * m);
            self.q1[k].lerp(d, cd);
            let q1 = self.q1[k];
            self.q2[k].lerp(q1, cd);
            let q2 = self.q2[k];
            self.q3[k].lerp(q2, cd);
        }
        if self.s.mode == HumMode::Auto {
            let mut za = self.za;
            for k in 0..AUTO_HARMONICS {
                let d = za.conj_scale(2.0 * m);
                self.qa1[k].lerp(d, cd);
                let q1 = self.qa1[k];
                self.qa2[k].lerp(q1, cd);
                let q2 = self.qa2[k];
                self.qa3[k].lerp(q2, cd);
                za = za.mul(self.za);
            }
            self.za = renorm(self.za.mul(self.step_a));
        }
        let (dry, out) = if self.s.gentle {
            // The look-ahead: the canceller works `ahead` samples late, on the input and the
            // reference as they were then.
            let mask = self.ring_l.len() - 1;
            let p = self.rpos;
            let back = p.wrapping_sub(self.ahead) & mask;
            let xd = (self.ring_l[back], self.ring_r[back]);
            let z1d = self.ring_z[back];
            self.ring_l[p] = l;
            self.ring_r[p] = r;
            self.ring_z[p] = self.z1;
            self.rpos = (p + 1) & mask;
            let mut zd = [C::ZERO; MAX_HARMONICS];
            zd[0] = z1d;
            for k in 1..MAX_HARMONICS {
                zd[k] = zd[k - 1].mul(z1d);
            }
            let out = (
                self.gentle(0, l as f64, xd.0 as f64, &z, &zd),
                self.gentle(1, r as f64, xd.1 as f64, &z, &zd),
            );
            (xd, out)
        } else {
            (
                (l, r),
                (self.notches(0, l as f64), self.notches(1, r as f64)),
            )
        };
        self.z1 = renorm(self.z1.mul(self.step));
        (dry, (out.0 as f32, out.1 as f32))
    }

    /// The gentle mode's look-ahead, samples (its latency; the notches have none).
    pub fn ahead(&self) -> usize {
        self.ahead
    }

    /// The module's latency with its settings, samples.
    pub fn latency(&self) -> usize {
        if self.s.gentle {
            self.ahead
        } else {
            0
        }
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
    // The harmonics' (bins') arrays run in step: an index reads clearest.
    #[allow(clippy::needless_range_loop)]
    fn gentle(
        &mut self,
        ch: usize,
        x: f64,
        xd: f64,
        z: &[C; MAX_HARMONICS],
        zd: &[C; MAX_HARMONICS],
    ) -> f64 {
        let mu = self.mu;
        let cg = self.cg;
        let cr = self.cr;
        let learn = self.learn_len;
        let hold_len = self.hold_len + self.ahead as u32;
        let lane = &mut self.lanes[ch];
        // The hum learned, predicted now (the look-ahead's analysis) and `ahead` samples
        // ago (where the canceller works).
        let mut hum_now = 0.0;
        let mut hum = 0.0;
        let mut taken = 0.0;
        for k in 0..MAX_HARMONICS {
            if self.g[k] >= 1.0 {
                continue;
            }
            let est = lane.est[k];
            hum_now += est.re * z[k].re - est.im * z[k].im;
            let h = est.re * zd[k].re - est.im * zd[k].im;
            hum += h;
            taken += (1.0 - self.g[k]) * h;
        }
        let e_now = x - hum_now;
        let e = xd - hum;
        let learning = lane.heard < learn;
        if learning && (lane.heard > 0 || xd != 0.0) {
            lane.heard += 1;
            if lane.heard == learn {
                // Learned: the smoothed estimates and the snapshots start from it.
                for k in 0..MAX_HARMONICS {
                    let v = lane.est[k];
                    lane.s_a[k] = v;
                    lane.s_b[k] = v;
                    lane.back_a[k] = v;
                    lane.back_b[k] = v;
                    lane.back_c[k] = v;
                    lane.turn_prev[k] = v;
                }
            }
        }
        for k in 0..MAX_HARMONICS {
            if self.g[k] >= 1.0 {
                continue;
            }
            // What the hum learned does not explain, in the harmonic's band (three poles),
            // `ahead` samples before the canceller hears it.
            let d = z[k].conj_scale(2.0 * e_now);
            lane.e1[k].lerp(d, cg);
            let v = lane.e1[k];
            lane.e2[k].lerp(v, cg);
            let v = lane.e2[k];
            lane.e3[k].lerp(v, cg);
            let left = lane.e3[k];
            // The hum learned, without the canceller's own ripple (a neighbour's beat
            // against the reference makes the raw estimate shake): two poles.
            let raw = lane.est[k];
            lane.s_a[k].lerp(raw, cr);
            let v = lane.s_a[k];
            lane.s_b[k].lerp(v, cr);
            let est = lane.s_b[k];
            // What the band usually holds beyond the hum (a neighbour's leak, the hum's own
            // small error): risen to slowly, fallen from quickly — a steady remainder is
            // not a note, and is learned.
            let size = left.norm2().sqrt();
            let c = if size > lane.floor[k] {
                self.c_up
            } else {
                self.c_down
            };
            lane.floor[k] += (size - lane.floor[k]) * c;
            let lim = (GATE_REL * est.norm2().sqrt()).max(FLOOR_TIMES * lane.floor[k]) + GATE_FLOOR;
            // Not the hum gone (that is learned at once).
            let sum = C {
                re: est.re + left.re,
                im: est.im + left.im,
            };
            let gone = sum.norm2() < 0.25 * est.norm2();
            let busy = !learning && k < self.gated && size > lim && !gone;
            if busy {
                if !lane.held[k] {
                    // The note's first milliseconds, learned before it was heard, unlearned.
                    lane.est[k] = lane.back_c[k];
                    lane.s_a[k] = lane.back_c[k];
                    lane.s_b[k] = lane.back_c[k];
                }
                lane.held[k] = true;
                // Until the note has passed the canceller too.
                lane.hold_left[k] = hold_len;
                lane.busy_run[k] = lane.busy_run[k].saturating_add(1);
            } else {
                lane.busy_run[k] = 0;
                if lane.hold_left[k] > 0 {
                    lane.hold_left[k] -= 1;
                } else {
                    lane.held[k] = false;
                }
            }
            let step = if learning {
                2.0 * mu
            } else if !lane.held[k] {
                mu
            } else if lane.busy_run[k] > self.escape_len {
                mu / 16.0
            } else {
                0.0
            };
            if step > 0.0 {
                let u = zd[k].conj_scale(2.0 * step * e);
                lane.est[k].re += u.re;
                lane.est[k].im += u.im;
            }
            // The snapshots count learning time only: going back undoes the last 40–60 ms
            // of learning, never what was learned before a hold.
            if !lane.held[k] {
                lane.snap[k] += 1;
                if lane.snap[k] >= self.snap_len {
                    lane.snap[k] = 0;
                    lane.back_c[k] = lane.back_b[k];
                    lane.back_b[k] = lane.back_a[k];
                    lane.back_a[k] = lane.s_b[k];
                }
            }
        }
        xd - taken
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
