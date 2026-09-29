//! Gravis's output stage: the fundamental-locked drive, the subsonic high-pass, the stereo
//! width above a mono crossover, the kick duck and the oversampled clipper
//! (`research/instruments/bass.md` §4.2, §4.4, §4.5).
//!
//! **Oversampling without latency.** A bass is lined up against the kick to the sample, and
//! an instrument's latency is reported but not compensated (ADR 0023), so the 4× stages
//! here use polyphase IIR half-bands (two chains of first-order allpasses in z², designed
//! from the elliptic half-band formulas of Valenzuela & Constantinides, 1983): a group
//! delay of a couple of samples in the bass instead of a linear-phase FIR's thirty.
//!
//! **The drive keeps the fundamental.** The instrument knows its f0 exactly, so no
//! detector is needed: the saturator runs on the whole signal (a sine has nothing else to
//! saturate) at 4×, and only **what the saturation added** — `sat(x) − x` — goes on,
//! through a notch tracking f0 and a high-pass under it, to be added to the clean signal.
//! The fundamental is therefore the clean one, whatever the drive: testable to 0.1 dB.
//! Drive 0 is not computed at all, so it is bit-exact.

use std::f64::consts::PI;

// ---- the half-band --------------------------------------------------------------------

/// Allpass coefficients of a polyphase IIR half-band with `n` coefficients and a
/// transition band `transition` (fraction of the rate, e.g. 0.04). Elliptic design by the
/// Jacobi theta-function series (Valenzuela & Constantinides 1983).
pub fn halfband_coefs(n: usize, transition: f64) -> Vec<f64> {
    let mut k = ((1.0 - 2.0 * transition) * PI / 4.0).tan();
    k *= k;
    let kk = (1.0 - k * k).powf(0.25);
    let e = 0.5 * (1.0 - kk) / (1.0 + kk);
    let e4 = e.powi(4);
    let q = e * (1.0 + e4 * (2.0 + e4 * (15.0 + 150.0 * e4)));
    let order = (2 * n + 1) as f64;
    (0..n)
        .map(|i| {
            let c = (i + 1) as f64;
            let mut num = 0.0;
            let mut j = 1.0;
            for m in 0..40i32 {
                let t = q.powi(m * (m + 1)) * (((2 * m + 1) as f64) * c * PI / order).sin() * j;
                num += t;
                j = -j;
                if t.abs() < 1e-100 {
                    break;
                }
            }
            let num = num * q.powf(0.25);
            let mut den = 0.0;
            let mut j = -1.0;
            for m in 1..40i32 {
                let t = q.powi(m * m) * ((2 * m) as f64 * c * PI / order).cos() * j;
                den += t;
                j = -j;
                if t.abs() < 1e-100 {
                    break;
                }
            }
            let ww = num / (den + 0.5);
            let wwsq = ww * ww;
            let x = ((1.0 - wwsq * k) * (1.0 - wwsq / k)).sqrt() / (1.0 + wwsq);
            (1.0 - x) / (1.0 + x)
        })
        .collect()
}

const MAX_AP: usize = 5;

/// One chain of first-order allpasses (in z² at the high rate, z at the low one).
#[derive(Copy, Clone, Debug, Default)]
struct Chain {
    c: [f64; MAX_AP],
    n: usize,
    x1: [f64; MAX_AP],
    y1: [f64; MAX_AP],
}

impl Chain {
    #[inline]
    fn process(&mut self, mut x: f64) -> f64 {
        for i in 0..self.n {
            let y = self.c[i] * (x - self.y1[i]) + self.x1[i];
            self.x1[i] = x;
            self.y1[i] = if y.abs() < 1e-30 { 0.0 } else { y };
            x = self.y1[i];
        }
        x
    }

    fn reset(&mut self) {
        self.x1 = [0.0; MAX_AP];
        self.y1 = [0.0; MAX_AP];
    }

    fn is_zero(&self) -> bool {
        self.x1.iter().chain(&self.y1).all(|v| *v == 0.0)
    }
}

/// A 2× half-band: two allpass chains, coefficients alternating between them.
#[derive(Copy, Clone, Debug, Default)]
pub struct HalfBand {
    a: Chain,
    b: Chain,
    /// The decimator's odd sample from the previous pair.
    held: f64,
}

impl HalfBand {
    pub fn new(coefs: &[f64]) -> Self {
        let mut h = HalfBand::default();
        for (i, c) in coefs.iter().enumerate().take(2 * MAX_AP) {
            let chain = if i % 2 == 0 { &mut h.a } else { &mut h.b };
            chain.c[chain.n] = *c;
            chain.n += 1;
        }
        h
    }

    /// Interpolate: one sample in, two out.
    #[inline]
    pub fn up(&mut self, x: f64) -> [f64; 2] {
        [self.a.process(x), self.b.process(x)]
    }

    /// Decimate: two samples in, one out.
    #[inline]
    pub fn down(&mut self, x: [f64; 2]) -> f64 {
        let y = 0.5 * (self.a.process(x[0]) + self.b.process(self.held));
        self.held = x[1];
        y
    }

    pub fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
        self.held = 0.0;
    }

    pub fn is_zero(&self) -> bool {
        self.a.is_zero() && self.b.is_zero() && self.held == 0.0
    }
}

/// 4×: two 2× stages each way.
#[derive(Copy, Clone, Debug, Default)]
pub struct Os4 {
    up1: HalfBand,
    up2: HalfBand,
    dn1: HalfBand,
    dn2: HalfBand,
}

impl Os4 {
    pub fn new() -> Self {
        // The first stage carries the audio band (to 0.46 of the base rate); the second
        // only has to reject images a quarter of its rate away.
        let a = halfband_coefs(8, 0.04);
        let b = halfband_coefs(4, 0.15);
        Os4 {
            up1: HalfBand::new(&a),
            up2: HalfBand::new(&b),
            dn1: HalfBand::new(&a),
            dn2: HalfBand::new(&b),
        }
    }

    #[inline]
    pub fn up(&mut self, x: f64) -> [f64; 4] {
        let [p, q] = self.up1.up(x);
        let [a, b] = self.up2.up(p);
        let [c, d] = self.up2.up(q);
        [a, b, c, d]
    }

    #[inline]
    pub fn down(&mut self, x: [f64; 4]) -> f64 {
        let p = self.dn2.down([x[0], x[1]]);
        let q = self.dn2.down([x[2], x[3]]);
        self.dn1.down([p, q])
    }

    pub fn reset(&mut self) {
        self.up1.reset();
        self.up2.reset();
        self.dn1.reset();
        self.dn2.reset();
    }

    pub fn is_zero(&self) -> bool {
        self.up1.is_zero() && self.up2.is_zero() && self.dn1.is_zero() && self.dn2.is_zero()
    }
}

// ---- a TPT state-variable filter, f64 (tracking filters must stay stable while moving) ---

#[derive(Copy, Clone, Debug, Default)]
pub struct Svf64 {
    s1: f64,
    s2: f64,
}

/// (low, band, high)
impl Svf64 {
    #[inline]
    pub fn process(&mut self, x: f64, g: f64, k: f64) -> (f64, f64, f64) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let v1 = a1 * (self.s1 + g * (x - self.s2));
        let v2 = self.s2 + g * v1;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        if self.s1.abs() < 1e-30 {
            self.s1 = 0.0;
        }
        if self.s2.abs() < 1e-30 {
            self.s2 = 0.0;
        }
        let hp = x - k * v1 - v2;
        (v2, v1, hp)
    }

    pub fn reset(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    pub fn is_zero(&self) -> bool {
        self.s1 == 0.0 && self.s2 == 0.0
    }
}

/// `g` of a TPT filter at `hz`.
#[inline]
pub fn tpt_g(hz: f64, sr: f64) -> f64 {
    (PI * hz.clamp(1.0, 0.45 * sr) / sr).tan()
}

/// Butterworth damping of the two sections of a fourth-order filter.
const BW4: [f64; 2] = [1.847_759, 0.765_367];

// ---- the drive ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum DriveType {
    /// Asymmetric: even and odd harmonics, warm.
    #[default]
    Tube,
    /// Soft and symmetric.
    Tape,
    /// A wavefolder: bright, many harmonics.
    Fold,
    /// A hard clip.
    Hard,
}

impl DriveType {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => DriveType::Tape,
            2 => DriveType::Fold,
            3 => DriveType::Hard,
            _ => DriveType::Tube,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DriveSettings {
    /// 0…1; 0 is not computed at all.
    pub amount: f32,
    pub kind: DriveType,
    /// dB of extra drive per octave below `ref_note` (negative: less).
    pub keytrack_db: f32,
    pub ref_note: f32,
    /// Keep the fundamental clean (the pitch-tracked notch).
    pub lock: bool,
}

impl Default for DriveSettings {
    fn default() -> Self {
        DriveSettings {
            amount: 0.0,
            kind: DriveType::Tube,
            keytrack_db: 3.0,
            ref_note: 36.0,
            lock: true,
        }
    }
}

/// The signal level the drive's curves are drawn against: the engines' full velocity.
const NOMINAL: f64 = 0.5;

impl DriveSettings {
    /// The drive's gain in dB for a note: 30 dB of travel, plus the key tracking.
    pub fn gain_db(&self, note: f64) -> f64 {
        let kt = self.keytrack_db as f64 * (self.ref_note as f64 - note) / 12.0;
        (self.amount as f64 * 30.0 + kt).clamp(0.0, 42.0)
    }

    /// The transfer curve at `g` (linear gain), normalised so a nominal-level peak stays
    /// where it was.
    #[inline]
    pub fn curve(&self, u: f64, g: f64) -> f64 {
        let v = g * u / NOMINAL;
        let y = match self.kind {
            DriveType::Tube => {
                let t = |v: f64| (v + 0.3).tanh() - 0.3f64.tanh();
                t(v) / (0.5 * (t(g) - t(-g)))
            }
            DriveType::Tape => (v / (1.0 + v.abs())) / (g / (1.0 + g)),
            DriveType::Fold => (v * std::f64::consts::FRAC_PI_2).sin(),
            DriveType::Hard => v.clamp(-1.0, 1.0),
        };
        y * NOMINAL
    }
}

/// The drive, with its oversampler and the fundamental lock's filters.
#[derive(Copy, Clone, Debug, Default)]
pub struct Drive {
    os: Os4,
    notch: Svf64,
    hp: [Svf64; 2],
    active: bool,
}

impl Drive {
    pub fn new() -> Self {
        Drive {
            os: Os4::new(),
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        self.os.reset();
        self.notch.reset();
        self.hp.iter_mut().for_each(Svf64::reset);
        self.active = false;
    }

    pub fn is_zero(&self) -> bool {
        !self.active
            || (self.os.is_zero() && self.notch.is_zero() && self.hp.iter().all(Svf64::is_zero))
    }

    /// One sample. `f0`: the voice's fundamental now; `note`: its pitch for key tracking.
    #[inline]
    pub fn process(&mut self, x: f32, s: &DriveSettings, f0: f64, note: f64, sr: f64) -> f32 {
        if s.amount <= 0.0 {
            if self.active {
                self.reset();
            }
            return x;
        }
        self.active = true;
        let g = 10f64.powf(s.gain_db(note) / 20.0);
        let up = self.os.up(x as f64);
        let mut d = [0.0; 4];
        for (o, u) in d.iter_mut().zip(up) {
            *o = s.curve(u, g) - u;
        }
        let added = self.os.down(d);
        if !s.lock {
            return (x as f64 + added) as f32;
        }
        // Only what the drive added, minus anything at or under the fundamental.
        let (lp, _, hp) = self.notch.process(added, tpt_g(f0, sr), 0.5);
        let mut y = lp + hp;
        let gh = tpt_g(0.7 * f0, sr);
        for (f, k) in self.hp.iter_mut().zip(BW4) {
            y = f.process(y, gh, k).2;
        }
        (x as f64 + y) as f32
    }
}

// ---- the clipper, the subsonic filter, the width ------------------------------------------

/// A soft clip at 4×: exact below 0.7, a tanh knee to a 0.98 ceiling.
#[derive(Copy, Clone, Debug, Default)]
pub struct Clipper {
    os: Os4,
    active: bool,
}

impl Clipper {
    pub fn new() -> Self {
        Clipper {
            os: Os4::new(),
            active: false,
        }
    }

    #[inline]
    pub fn curve(u: f64) -> f64 {
        const KNEE: f64 = 0.7;
        const ROOM: f64 = 0.28;
        let a = u.abs();
        if a <= KNEE {
            u
        } else {
            u.signum() * (KNEE + ROOM * ((a - KNEE) / ROOM).tanh())
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32, on: bool) -> f32 {
        if !on {
            if self.active {
                self.os.reset();
                self.active = false;
            }
            return x;
        }
        self.active = true;
        let up = self.os.up(x as f64);
        let y = self.os.down(up.map(Self::curve));
        y as f32
    }

    pub fn is_zero(&self) -> bool {
        !self.active || self.os.is_zero()
    }

    pub fn reset(&mut self) {
        self.os.reset();
    }
}

/// A second-order Butterworth high-pass; 0 Hz is off (and bit-exact).
#[derive(Copy, Clone, Debug, Default)]
pub struct Subsonic {
    f: Svf64,
}

impl Subsonic {
    #[inline]
    pub fn process(&mut self, x: f32, hz: f32, sr: f64) -> f32 {
        if hz <= 0.0 {
            if !self.f.is_zero() {
                self.f.reset();
            }
            return x;
        }
        self.f
            .process(x as f64, tpt_g(hz as f64, sr), std::f64::consts::SQRT_2)
            .2 as f32
    }

    pub fn is_zero(&self) -> bool {
        self.f.is_zero()
    }

    pub fn reset(&mut self) {
        self.f.reset();
    }
}

/// The side signal above the mono crossover (fourth-order): everything below it is mono.
#[derive(Copy, Clone, Debug, Default)]
pub struct MonoBelow {
    hp: [Svf64; 2],
}

impl MonoBelow {
    #[inline]
    pub fn process(&mut self, side: f32, hz: f32, sr: f64) -> f32 {
        let g = tpt_g(hz as f64, sr);
        let mut y = side as f64;
        for (f, k) in self.hp.iter_mut().zip(BW4) {
            y = f.process(y, g, k).2;
        }
        y as f32
    }

    pub fn is_zero(&self) -> bool {
        self.hp.iter().all(Svf64::is_zero)
    }

    pub fn reset(&mut self) {
        self.hp.iter_mut().for_each(Svf64::reset);
    }
}

// ---- the kick duck -------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct DuckSettings {
    pub depth_db: f32,
    pub pre_ms: f32,
    pub hold_ms: f32,
    pub release_ms: f32,
    /// 0 linear (in amplitude), 1 exponential (linear in dB), 2 S-curve (in dB).
    pub shape: u8,
}

impl Default for DuckSettings {
    fn default() -> Self {
        DuckSettings {
            depth_db: 6.0,
            pre_ms: 2.0,
            hold_ms: 20.0,
            release_ms: 90.0,
            shape: 1,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
enum DuckStage {
    #[default]
    Idle,
    Down,
    Hold,
    Up,
}

/// A gain that dips for the kick. Driven by the host, which knows when the next kick
/// will land: [`Duck::kick_in`] says "a kick starts in `n` samples", and the dip reaches
/// its depth exactly then — ahead of the kick, with no latency, because it reads the
/// kick's notes rather than its audio.
#[derive(Copy, Clone, Debug, Default)]
pub struct Duck {
    stage: DuckStage,
    /// The gain now, in dB (≤ 0).
    db: f64,
    from: f64,
    pos: u32,
    len: u32,
}

impl Duck {
    /// A kick starts `ahead` samples from now (0: this very sample).
    pub fn kick_in(&mut self, ahead: u32) {
        self.stage = DuckStage::Down;
        self.from = self.db;
        self.pos = 0;
        self.len = ahead;
    }

    pub fn reset(&mut self) {
        *self = Duck::default();
    }

    pub fn is_idle(&self) -> bool {
        self.stage == DuckStage::Idle
    }

    /// The gain in dB now (for the editor's lane).
    pub fn db(&self) -> f64 {
        self.db
    }

    #[inline]
    fn shaped(x: f64, shape: u8, from_db: f64, to_db: f64) -> f64 {
        let x = x.clamp(0.0, 1.0);
        match shape {
            0 => {
                let (a, b) = (10f64.powf(from_db / 20.0), 10f64.powf(to_db / 20.0));
                20.0 * (a + (b - a) * x).max(1e-6).log10()
            }
            2 => from_db + (to_db - from_db) * x * x * (3.0 - 2.0 * x),
            _ => from_db + (to_db - from_db) * x,
        }
    }

    /// The linear gain for this sample.
    #[inline]
    pub fn next(&mut self, s: &DuckSettings, sr: f64) -> f32 {
        let depth = -(s.depth_db.max(0.0) as f64);
        match self.stage {
            DuckStage::Idle => return 1.0,
            DuckStage::Down => {
                self.db = if self.pos >= self.len {
                    depth
                } else {
                    Self::shaped(self.pos as f64 / self.len as f64, s.shape, self.from, depth)
                };
                if self.pos >= self.len {
                    self.stage = DuckStage::Hold;
                    self.pos = 0;
                    self.len = (s.hold_ms as f64 * 0.001 * sr) as u32;
                } else {
                    self.pos += 1;
                }
            }
            DuckStage::Hold => {
                self.db = depth;
                self.pos += 1;
                if self.pos >= self.len {
                    self.stage = DuckStage::Up;
                    self.pos = 0;
                    self.len = ((s.release_ms as f64 * 0.001 * sr) as u32).max(1);
                }
            }
            DuckStage::Up => {
                self.pos += 1;
                self.db = Self::shaped(self.pos as f64 / self.len as f64, s.shape, depth, 0.0);
                if self.pos >= self.len {
                    self.db = 0.0;
                    self.stage = DuckStage::Idle;
                }
            }
        }
        10f64.powf(self.db / 20.0) as f32
    }
}
