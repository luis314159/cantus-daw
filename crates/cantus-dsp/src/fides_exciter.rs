//! Fides's exciters (a child of `fides`): what sets a resonator moving.
//!
//! - **Pluck** and **pick**: a finger's or a plectrum's release, a pulse of the contact's
//!   length whose spectrum falls as `1/f` above 40 Hz (a released displacement), the pick's
//!   sharper and with its scrape.
//! - **Mallet**: the power-law spring's force pulse (Lamina's [`HammerShape`], the
//!   Hunt–Crossley contact `F = k·xᵖ`, `p = 2.5`), shorter the harder it lands.
//! - **Bow**: a velocity envelope the resonator meets through a friction curve —
//!   [`friction`], after the bow table of Smith's bowed-string model as the STK gives it
//!   (MIT; Smith, "Efficient simulation of the reed-bore and bow-string mechanisms",
//!   ICMC 1986). The stick and slip happen inside the resonator's loop.
//! - **Noise**: band-passed noise (continuous, or sparse grains) through its own envelope;
//!   it also lies under any other exciter at `noise.level`.
//! - **Input**: a track's audio, handed in with the block.
//!
//! Every impulsive exciter's pulse has the same area in samples at every sample rate, so a
//! note's low partials are the same at 44.1 and 96 kHz. Every random number is the voice's
//! own [`Rng`], seeded from the instance's seed, the note's key and its id.

use crate::lamina::voice::HammerShape;
use crate::math;

/// Which exciter, in `exc.type`'s order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum ExciterKind {
    #[default]
    Pluck,
    Pick,
    Bow,
    Mallet,
    Noise,
    Input,
}

impl ExciterKind {
    pub const ALL: [ExciterKind; 6] = [
        ExciterKind::Pluck,
        ExciterKind::Pick,
        ExciterKind::Bow,
        ExciterKind::Mallet,
        ExciterKind::Noise,
        ExciterKind::Input,
    ];

    pub fn from_index(i: f32) -> ExciterKind {
        let k = (i.round().max(0.0) as usize).min(Self::ALL.len() - 1);
        Self::ALL[k]
    }

    /// A pulse at the note's start, then nothing (the resonator rings on its own).
    pub fn is_impulsive(self) -> bool {
        matches!(
            self,
            ExciterKind::Pluck | ExciterKind::Pick | ExciterKind::Mallet
        )
    }
}

/// The exciter's settings (the unit's, before modulation).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct ExciterSettings {
    pub kind: ExciterKind,
    pub level_db: f32,
    pub contact_ms: f32,
    pub vel_tone: f32,
    pub tone_hz: f32,
    pub noise: f32,
    pub pos: f32,
    pub bow_pressure: f32,
    pub bow_speed: f32,
    pub bow_attack_ms: f32,
    pub bow_release_ms: f32,
}

impl Default for ExciterSettings {
    fn default() -> Self {
        ExciterSettings {
            kind: ExciterKind::Pluck,
            level_db: 0.0,
            contact_ms: 0.6,
            vel_tone: 0.5,
            tone_hz: 20_000.0,
            noise: 0.0,
            pos: 0.13,
            bow_pressure: 0.5,
            bow_speed: 0.5,
            bow_attack_ms: 120.0,
            bow_release_ms: 200.0,
        }
    }
}

/// The noise generator's settings.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NoiseSettings {
    pub level: f32,
    pub color_hz: f32,
    pub width_oct: f32,
    pub density: f32,
    pub attack_ms: f32,
    pub decay_ms: f32,
    pub sustain: f32,
    pub release_ms: f32,
}

impl Default for NoiseSettings {
    fn default() -> Self {
        NoiseSettings {
            level: 0.0,
            color_hz: 2000.0,
            width_oct: 3.0,
            density: 1.0,
            attack_ms: 5.0,
            decay_ms: 200.0,
            sustain: 0.0,
            release_ms: 100.0,
        }
    }
}

/// The friction a bow meets at a velocity difference `dv` (bow − string), with the
/// curve's slope `slope` (5 − 4·pressure): what fraction of `dv` the bow hands the string.
/// 1 while it sticks (small `dv`), falling as the string slips.
#[inline]
pub fn friction(dv: f32, slope: f32) -> f32 {
    let x = ((dv + 0.001) * slope).abs() + 0.75;
    let x2 = x * x;
    (1.0 / (x2 * x2)).min(1.0)
}

/// xorshift64*: a voice's random numbers, from its seed alone.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rng(pub u64);

impl Rng {
    /// Seeded from the instance's seed, the key and the note's id (never the clock).
    pub fn of(seed: u32, key: u8, id: u64) -> Rng {
        let mut z =
            (seed as u64) << 40 ^ (key as u64) << 32 ^ id.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        z ^= z >> 31;
        z = z.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z ^= z >> 29;
        Rng(z | 1)
    }

    #[inline]
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// −1 … 1.
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        ((self.next() >> 40) as f32 / (1u64 << 23) as f32) - 1.0
    }

    /// 0 … 1.
    #[inline]
    pub fn unit(&mut self) -> f32 {
        (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// A biquad's coefficients and state (direct form I, `f32`).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Biquad {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl Biquad {
    /// A band-pass of 0 dB at `hz`, `oct` octaves wide (RBJ's cookbook).
    pub fn band(hz: f32, oct: f32, sr: f32) -> (f32, f32, f32, f32, f32) {
        let w = std::f32::consts::TAU * (hz / sr).clamp(1e-5, 0.45);
        let (sn, cs) = math::sin_cos(w);
        let alpha = sn * math::sinh(std::f32::consts::LN_2 / 2.0 * oct * w / sn);
        let a0 = 1.0 + alpha;
        (
            alpha / a0,
            0.0,
            -alpha / a0,
            -2.0 * cs / a0,
            (1.0 - alpha) / a0,
        )
    }

    /// A Butterworth low-pass (`high` false) or high-pass at `hz`.
    pub fn pass(hz: f32, high: bool, sr: f32) -> (f32, f32, f32, f32, f32) {
        let w = std::f32::consts::TAU * (hz / sr).clamp(1e-5, 0.45);
        let (sn, cs) = math::sin_cos(w);
        let alpha = sn * std::f32::consts::FRAC_1_SQRT_2;
        let a0 = 1.0 + alpha;
        let (b0, b1) = if high {
            ((1.0 + cs) / 2.0, -(1.0 + cs))
        } else {
            ((1.0 - cs) / 2.0, 1.0 - cs)
        };
        (
            b0 / a0,
            b1 / a0,
            b0 / a0,
            -2.0 * cs / a0,
            (1.0 - alpha) / a0,
        )
    }

    pub fn set(&mut self, c: (f32, f32, f32, f32, f32)) {
        (self.b0, self.b1, self.b2, self.a1, self.a2) = c;
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
            - self.a1 * self.y1
            - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    pub fn clear(&mut self) {
        (self.x1, self.x2, self.y1, self.y2) = (0.0, 0.0, 0.0, 0.0);
    }

    pub fn peak(&self) -> f32 {
        self.x1
            .abs()
            .max(self.x2.abs())
            .max(self.y1.abs())
            .max(self.y2.abs())
    }
}

/// One exciter's values for the next control step: the settings with the modulation and
/// MPE applied (the voice computes them).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct ExcStep {
    /// The exciter's gain, linear (its level and the note's velocity).
    pub level: f32,
    /// The tone low-pass's coefficient (0: open).
    pub tone: f32,
    /// The strike noise's amount.
    pub noise: f32,
    /// The bow's velocity while it is drawn, and its friction curve's slope.
    pub bow_v: f32,
    pub bow_slope: f32,
    /// The noise generator's level (the Noise exciter's own included) and filter.
    pub noise_level: f32,
    pub noise_band: (f32, f32, f32, f32, f32),
}

/// The leaky integrator's lowest corner: a plucked string's partials fall as 1/n above it.
const LEAK_HZ: f32 = 20.0;
/// A pulse's area in samples, at every rate: the scale the resonators are calibrated to.
pub const AREA: f32 = 1.0;

/// An envelope's stage.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
enum Stage {
    #[default]
    Idle,
    Attack,
    Hold,
    Release,
}

/// Below this an envelope, a leak or a bow is over (−120 dB).
const OVER: f32 = 1e-6;

/// One voice's exciter.
#[derive(Clone, Debug)]
pub struct Exciter {
    sr: f32,
    kind: ExciterKind,
    pub rng: Rng,
    held: bool,
    // ---- the pulse ----
    t: u32,
    len: u32,
    amp: f32,
    leaky: bool,
    leak: f32,
    leak_c: f32,
    tone_z: f32,
    // ---- the strike's noise ----
    burst_len: u32,
    burst_amp: f32,
    burst: Biquad,
    // ---- the bow (and the input's gate) ----
    bow: f32,
    bow_stage: Stage,
    bow_inc: f32,
    bow_rel: f32,
    // ---- the noise generator ----
    noise: Biquad,
    env: f32,
    env_stage: Stage,
    env_att: f32,
    env_dec: f32,
    env_rel: f32,
    env_sus: f32,
    density: f32,
}

impl Exciter {
    pub fn new(sample_rate: f32) -> Self {
        Exciter {
            sr: sample_rate,
            kind: ExciterKind::Pluck,
            rng: Rng(1),
            held: false,
            t: 0,
            len: 0,
            amp: 0.0,
            leaky: false,
            leak: 0.0,
            leak_c: 1.0 - math::exp(-std::f32::consts::TAU * LEAK_HZ / sample_rate),
            tone_z: 0.0,
            burst_len: 0,
            burst_amp: 0.0,
            burst: Biquad::default(),
            bow: 0.0,
            bow_stage: Stage::Idle,
            bow_inc: 0.0,
            bow_rel: 0.0,
            noise: Biquad::default(),
            env: 0.0,
            env_stage: Stage::Idle,
            env_att: 0.0,
            env_dec: 0.0,
            env_rel: 0.0,
            env_sus: 0.0,
            density: 1.0,
        }
    }

    pub fn kind(&self) -> ExciterKind {
        self.kind
    }

    /// The exciter's envelope for the modulation (`env1`): the bow's or the noise's, 0…1.
    pub fn env_level(&self) -> f32 {
        match self.kind {
            ExciterKind::Bow | ExciterKind::Input => self.bow,
            _ => self.env,
        }
    }

    /// The bow is on the string (drawn, or lifting): once it is off, the string is free.
    pub fn bow_active(&self) -> bool {
        self.bow_stage != Stage::Idle
    }

    /// How far the bow is drawn (0 … 1 of its velocity; the input's gate).
    pub fn bow_fraction(&self) -> f32 {
        self.bow
    }

    /// A note starts: latch the pulse from the velocity (0…1) and the settings.
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &mut self,
        s: &ExciterSettings,
        n: &NoiseSettings,
        step: &ExcStep,
        velocity: f32,
        sensitivity: f32,
        hammer_area: f32,
        f0: f32,
        rng: Rng,
    ) {
        let sr = self.sr;
        // The leak's corner at half the fundamental: a pluck's partials fall as 1/n from
        // the first, at every pitch.
        let corner = (0.5 * f0).clamp(LEAK_HZ, 0.2 * sr);
        self.leak_c = 1.0 - math::exp(-std::f32::consts::TAU * corner / sr);
        self.kind = s.kind;
        self.rng = rng;
        self.held = true;
        // The power-law spring: a faster strike is a shorter contact, v^(−(p−1)/(p+1)) at
        // vel_tone ½ (Lamina's `contact_s`), scaled around velocity 100.
        let e = 2.0 * s.vel_tone.clamp(0.0, 1.0) * (1.5 / 3.5);
        let vf = (velocity / (100.0 / 127.0)).clamp(0.05, 1.3);
        let contact = s.contact_ms * math::powf(vf, -e);
        let (shape_mean, factor) = match s.kind {
            ExciterKind::Pick => (0.5, 0.25),
            ExciterKind::Mallet => (hammer_area, 1.0),
            _ => (0.5, 1.0),
        };
        let len = (contact * factor * 0.001 * sr).round().max(1.0);
        let vel_gain = (1.0 - sensitivity) + sensitivity * math::powf(velocity.max(0.0), 1.5);
        self.t = 0;
        self.tone_z = 0.0;
        self.leak = 0.0;
        if s.kind.is_impulsive() {
            self.len = len as u32;
            self.amp = vel_gain * AREA / (len * shape_mean);
            self.leaky = s.kind != ExciterKind::Mallet;
            let (band, mul) = match s.kind {
                ExciterKind::Pick => (Biquad::band(4000.0, 2.0, sr), 3.0),
                ExciterKind::Mallet => (Biquad::band(1500.0, 2.5, sr), 1.0),
                _ => (Biquad::band(2500.0, 2.0, sr), 1.5),
            };
            self.burst.clear();
            self.burst.set(band);
            self.burst_len = (len * mul) as u32;
            self.burst_amp = 0.7 * self.amp;
        } else {
            self.len = 0;
            self.amp = 0.0;
            self.burst_len = 0;
        }
        // The bow (and the input's gate): a ramp over the attack.
        self.bow = 0.0;
        self.bow_stage = if matches!(s.kind, ExciterKind::Bow | ExciterKind::Input) {
            Stage::Attack
        } else {
            Stage::Idle
        };
        self.bow_inc = 1.0 / (s.bow_attack_ms.max(0.5) * 0.001 * sr);
        self.bow_rel = math::exp(-4.6 / (s.bow_release_ms.max(1.0) * 0.001 * sr));
        // The noise generator.
        self.noise.clear();
        self.noise.set(step.noise_band);
        self.env = 0.0;
        // The modulated level too: a route that raises it from the first step plays it.
        let plays_noise = s.kind == ExciterKind::Noise || n.level > 0.0 || step.noise_level > 0.0;
        self.env_stage = if plays_noise {
            Stage::Attack
        } else {
            Stage::Idle
        };
        self.env_att = 1.0 / (n.attack_ms.max(0.1) * 0.001 * sr);
        self.env_dec = math::exp(-4.6 / (n.decay_ms.max(0.5) * 0.001 * sr));
        self.env_rel = math::exp(-4.6 / (n.release_ms.max(0.5) * 0.001 * sr));
        self.env_sus = n.sustain.clamp(0.0, 1.0);
        self.density = n.density.clamp(0.0, 1.0);
    }

    /// The control step mid-note (review 2). The noise generator's band follows the
    /// step's — its coefficients only, the state running on (a direct-form filter retuned
    /// every 32 samples moves without a click, as the tone's low-pass does) — so a route or
    /// a knob on the noise's colour or width is heard while the note is held. And a noise
    /// layer that was not playing (its level 0 at the note's start) comes in when its level
    /// rises above 0 while the key is held, on its own envelope from the attack, as a layer
    /// that starts with the note would; once it plays, its level only scales it. After the
    /// key is let go nothing starts. No allocation.
    pub fn follow(&mut self, step: &ExcStep) {
        self.noise.set(step.noise_band);
        if self.env_stage == Stage::Idle && self.held && step.noise_level > 0.0 {
            self.env = 0.0;
            self.env_stage = Stage::Attack;
        }
    }

    /// The key is let go: the bow lifts, the noise releases.
    pub fn release(&mut self) {
        self.held = false;
        if self.bow_stage != Stage::Idle {
            self.bow_stage = Stage::Release;
        }
        if self.env_stage != Stage::Idle {
            self.env_stage = Stage::Release;
        }
    }

    /// Stop at once (the voice is reset).
    pub fn clear(&mut self) {
        self.held = false;
        self.len = 0;
        self.burst_len = 0;
        self.leak = 0.0;
        self.tone_z = 0.0;
        self.burst.clear();
        self.noise.clear();
        self.bow = 0.0;
        self.bow_stage = Stage::Idle;
        self.env = 0.0;
        self.env_stage = Stage::Idle;
    }

    /// Nothing more will come out of it: the pulse and its leak are over, the bow is
    /// lifted and stopped, the noise's envelope is idle.
    pub fn done(&self) -> bool {
        self.t >= self.len
            && self.t >= self.burst_len
            && self.leak.abs() < OVER
            && self.tone_z.abs() < OVER
            && self.bow_stage == Stage::Idle
            && self.env_stage == Stage::Idle
    }
}

impl Exciter {
    /// Render `imp.len()` samples: the strike (its pulse and its noise) **added** into
    /// `imp`, what is sustained (the noise generator, the live input) **added** into `sus`
    /// — each resonator takes the two at its own gains — and the bow's velocity
    /// **written** into `bow` (zeros with no bow). `input` is the live input's block (mono).
    pub fn render(
        &mut self,
        step: &ExcStep,
        hammer: &HammerShape,
        input: Option<&[f32]>,
        imp: &mut [f32],
        sus: &mut [f32],
        bow: &mut [f32],
    ) {
        let n = imp.len().min(sus.len()).min(bow.len());
        for i in 0..n {
            let mut x = 0.0f32;
            let mut y_sus = 0.0f32;
            // The pulse and its leak.
            if self.t < self.len || self.leak != 0.0 {
                let p = if self.t < self.len {
                    let u = (self.t as f32 + 0.5) / self.len as f32;
                    let shape = match self.kind {
                        ExciterKind::Mallet => hammer.at(u),
                        _ => 0.5 - 0.5 * math::cos(std::f32::consts::TAU * u),
                    };
                    self.amp * shape * step.level
                } else {
                    0.0
                };
                if self.leaky {
                    self.leak += self.leak_c * (p - self.leak);
                    if self.t >= self.len && self.leak.abs() < OVER * 1e-3 {
                        self.leak = 0.0;
                    }
                    x += self.leak;
                } else {
                    x += p;
                }
            }
            // The strike's noise.
            if self.t < self.burst_len {
                let w = self.rng.bipolar();
                let u = (self.t as f32 + 0.5) / self.burst_len as f32;
                let env = 0.5 - 0.5 * math::cos(std::f32::consts::TAU * u);
                let b = self.burst.tick(w);
                x += b * env * self.burst_amp * step.noise * step.level;
            } else if self.t == self.burst_len && self.burst_len > 0 {
                self.burst.clear();
            }
            if self.t < self.len.max(self.burst_len) {
                self.t += 1;
            }
            // The tone.
            if step.tone > 0.0 {
                self.tone_z += step.tone * (x - self.tone_z);
                if x == 0.0 && self.tone_z.abs() < OVER * 1e-3 {
                    self.tone_z = 0.0;
                }
                x = self.tone_z;
            }
            // The noise generator.
            if self.env_stage != Stage::Idle {
                match self.env_stage {
                    Stage::Attack => {
                        self.env += self.env_att;
                        if self.env >= 1.0 {
                            self.env = 1.0;
                            self.env_stage = Stage::Hold;
                        }
                    }
                    Stage::Hold => {
                        self.env = self.env_sus + (self.env - self.env_sus) * self.env_dec
                    }
                    Stage::Release => {
                        self.env *= self.env_rel;
                        if self.env < OVER {
                            self.env = 0.0;
                            self.env_stage = Stage::Idle;
                        }
                    }
                    Stage::Idle => {}
                }
                let w = if self.density >= 0.999 {
                    self.rng.bipolar()
                } else {
                    // Sparse grains: an impulse with probability `prob` a sample, scaled so
                    // the power is the continuous noise's.
                    let prob = 0.0005 + 0.2 * self.density * self.density;
                    let u = self.rng.unit();
                    let a = self.rng.bipolar();
                    if u < prob {
                        a / prob.sqrt()
                    } else {
                        0.0
                    }
                };
                let y = self.noise.tick(w);
                y_sus += y * self.env * step.noise_level;
                if self.env_stage == Stage::Idle {
                    self.noise.clear();
                }
            }
            // The bow (the input's gate).
            if self.bow_stage != Stage::Idle {
                match self.bow_stage {
                    Stage::Attack => {
                        self.bow += self.bow_inc;
                        if self.bow >= 1.0 {
                            self.bow = 1.0;
                            self.bow_stage = Stage::Hold;
                        }
                    }
                    Stage::Release => {
                        self.bow *= self.bow_rel;
                        if self.bow < OVER {
                            self.bow = 0.0;
                            self.bow_stage = Stage::Idle;
                        }
                    }
                    _ => {}
                }
            }
            match self.kind {
                ExciterKind::Bow => bow[i] = self.bow * step.bow_v,
                ExciterKind::Input => {
                    bow[i] = 0.0;
                    if let Some(inp) = input {
                        y_sus += inp.get(i).copied().unwrap_or(0.0) * self.bow * step.level;
                    }
                }
                _ => bow[i] = 0.0,
            }
            imp[i] += x;
            sus[i] += y_sus;
        }
    }
}
