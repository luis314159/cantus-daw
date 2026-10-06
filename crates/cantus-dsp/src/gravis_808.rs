//! Gravis's 808: an integer-phase sine with a pitch envelope, the TR-808's knock, a click,
//! an AHDSR, a slow "sigh" and exact Chebyshev harmonics (`research/instruments/bass.md`
//! §4.2).
//!
//! **Harmonics without aliasing and without touching the fundamental.** For a unit sine,
//! `T_n(cos θ) = cos nθ`: the n-th harmonic exactly. Here they are generated from the phase
//! by the recurrence `sin (n+1)θ = 2 cos θ · sin nθ − sin (n−1)θ`, at levels fixed relative
//! to the fundamental (whose level is 1), before the amplitude envelope, and only while
//! `n·f < 0.45·fs`. So the harmonics control can never alias and never changes the
//! fundamental.
//!
//! **The phase is a `u32`**: exactly periodic over a bar-long sustain and bit-identical on
//! every platform. Reset puts every note at 0° rising (identical hits against the kick).

use super::voice::{coef_60db, Env, EnvTimes, Rng};
use crate::math;

/// Pulsus's kick lands its body at 45 Hz from 15.5 st above; the knock is the TR-808's
/// "more than an octave for about 6 ms" (Werner, Abel & Smith, DAFx-14).
pub const KNOCK_ST: f32 = 24.0;
pub const KNOCK_MS: f32 = 8.0;
/// The TR-808's slow fall: about 2.5 st over about 300 ms.
pub const SIGH_ST: f32 = 2.5;
pub const SIGH_MS: f32 = 700.0;
/// The click's noise burst.
pub const CLICK_MS: f32 = 6.0;
/// The 808's level at full velocity: −6 dBFS.
pub const LEVEL: f32 = 0.5;
/// Harmonics generated: 2…8.
pub const HARMONICS: usize = 8;

/// The 808's settings, in real units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Settings808 {
    /// 0…1: sine towards a soft (band-limited) triangle.
    pub shape: f32,
    pub pitch_st: f32,
    pub pitch_ms: f32,
    pub knock: f32,
    pub click: f32,
    pub click_hz: f32,
    pub env: EnvTimes,
    pub sigh: f32,
    /// 0…1: the harmonics' overall level.
    pub harmonics: f32,
    /// dB per octave of harmonic number.
    pub tilt_db: f32,
    /// −1 odd only … +1 even only.
    pub even: f32,
}

impl Default for Settings808 {
    fn default() -> Self {
        Settings808 {
            shape: 0.0,
            pitch_st: 12.0,
            pitch_ms: 45.0,
            knock: 0.2,
            click: 0.15,
            click_hz: 3000.0,
            env: EnvTimes {
                attack_ms: 1.0,
                hold_ms: 0.0,
                decay_ms: 1200.0,
                sustain: 0.6,
                release_ms: 250.0,
            },
            sigh: 0.0,
            harmonics: 0.0,
            tilt_db: -3.0,
            even: 0.3,
        }
    }
}

impl Settings808 {
    /// The level of each harmonic 2…8 relative to the fundamental: `a[n]`, `a[0] = a[1] = 0`.
    pub fn harmonic_levels(&self) -> [f32; HARMONICS + 1] {
        let mut a = [0.0; HARMONICS + 1];
        if self.harmonics <= 0.0 {
            return a;
        }
        let even = self.even.clamp(-1.0, 1.0);
        for (n, v) in a.iter_mut().enumerate().skip(2) {
            let tilt = math::powf(10f32, self.tilt_db * math::log2(n as f32 / 2.0) / 20.0);
            let parity = if n % 2 == 0 { 1.0 + even } else { 1.0 - even };
            *v = 0.5 * self.harmonics * tilt * parity;
        }
        a
    }
}

/// Coefficients of the soft triangle's odd harmonics (3, 5, 7), relative to the sine.
const TRI: [f32; 3] = [-1.0 / 9.0, 1.0 / 25.0, -1.0 / 49.0];

/// One 808 voice. `Copy`, so a retrigger can hand the sounding voice to the declick slot.
#[derive(Copy, Clone, Debug, Default)]
pub struct Voice808 {
    phase: u32,
    /// Main-cycle wraps, for the sub: one bit per octave down.
    wraps: u32,
    pub env: Env,
    pitch_env: f32,
    pitch_c: f32,
    knock: f32,
    knock_c: f32,
    sigh: f32,
    sigh_c: f32,
    click_env: f32,
    click_c: f32,
    click_hp: (f32, f32),
    rng: Rng,
    amp: f32,
    /// Hertz of the last sample (the drive's crossover follows it).
    pub f0: f64,
    /// Seconds since the note started (for the editor's playhead).
    pub age: u32,
}

impl Voice808 {
    /// Start a note. `reset_phase`: begin at 0° (otherwise the phase runs on, as in legato
    /// or the Free and Random modes, where `phase` is given).
    pub fn start(&mut self, s: &Settings808, sr: f32, amp: f32, phase: Option<u32>, seed: u64) {
        if let Some(p) = phase {
            self.phase = p;
            self.wraps = 0;
        }
        self.env = Env::default();
        self.env.set(&s.env, sr);
        self.env.gate_on();
        self.pitch_env = if s.pitch_st > 0.0 { 1.0 } else { 0.0 };
        self.pitch_c = coef_60db(s.pitch_ms, sr);
        self.knock = if s.knock > 0.0 { 1.0 } else { 0.0 };
        self.knock_c = coef_60db(KNOCK_MS, sr);
        self.sigh = if s.sigh > 0.0 { 1.0 } else { 0.0 };
        self.sigh_c = coef_60db(SIGH_MS, sr);
        self.click_env = if s.click > 0.0 { 1.0 } else { 0.0 };
        self.click_c = coef_60db(CLICK_MS, sr);
        self.click_hp = (0.0, 0.0);
        self.rng = Rng(seed);
        self.amp = amp;
        self.age = 0;
    }

    /// Update the envelope times of a sounding voice (a knob turned while it plays).
    pub fn retime(&mut self, s: &Settings808, sr: f32) {
        self.env.set(&s.env, sr);
    }

    pub fn is_active(&self) -> bool {
        self.env.is_active()
    }

    /// The pitch offset in semitones the envelopes add now.
    #[inline]
    pub fn pitch_offset(&self, s: &Settings808) -> f32 {
        s.pitch_st * self.pitch_env + KNOCK_ST * s.knock * self.knock + SIGH_ST * s.sigh * self.sigh
    }

    /// One sample: `(main, sub)` with the amplitude envelope applied; `base_hz` is the
    /// voice's pitch before its envelopes. `sub_div` is 2 or 4; `sub_wave` 0 sine, 1
    /// triangle, 2 square.
    #[inline]
    pub fn next(
        &mut self,
        s: &Settings808,
        harm: &[f32; HARMONICS + 1],
        base_hz: f64,
        sr: f32,
        sub_div: u32,
        sub_wave: u32,
    ) -> (f32, f32) {
        let off = self.pitch_offset(s);
        let f = if off != 0.0 {
            base_hz * math::exp2(off as f64 / 12.0)
        } else {
            base_hz
        };
        self.f0 = f;
        let inc_f = (f / sr as f64).clamp(0.0, 0.49);
        let inc = (inc_f * 4_294_967_296.0).round() as u32;
        let t = self.phase as f64 * (1.0 / 4_294_967_296.0);
        let theta = std::f64::consts::TAU * t;
        let (s1, c1) = math::sin_cos(theta);
        let mut main = s1;
        // The soft triangle and the harmonics share one recurrence up to the 8th.
        let want_tri = s.shape > 0.0;
        let want_h = s.harmonics > 0.0;
        if want_tri || want_h {
            let limit = 0.45 * sr as f64;
            let (mut prev, mut cur) = (0.0f64, s1);
            for n in 2..=HARMONICS {
                let nxt = 2.0 * c1 * cur - prev;
                prev = cur;
                cur = nxt;
                if n as f64 * f >= limit {
                    break;
                }
                if want_h {
                    main += harm[n] as f64 * cur;
                }
                if want_tri && n % 2 == 1 {
                    main += (s.shape * TRI[(n - 3) / 2]) as f64 * cur;
                }
            }
        }
        // The sub, from the same phase: exact octave(s) down, zero crossings aligned.
        let sub_t = (t + (self.wraps % sub_div) as f64) / sub_div as f64;
        let sub = match sub_wave {
            1 => {
                let x = 4.0 * sub_t;
                if sub_t < 0.25 {
                    x
                } else if sub_t < 0.75 {
                    2.0 - x
                } else {
                    x - 4.0
                }
            }
            2 => square(sub_t, inc_f / sub_div as f64),
            _ => math::sin(std::f64::consts::TAU * sub_t),
        };
        let (next, wrapped) = self.phase.overflowing_add(inc);
        self.phase = next;
        if wrapped {
            self.wraps = self.wraps.wrapping_add(1);
        }
        // Envelopes.
        let env = self.env.next();
        if self.pitch_env != 0.0 {
            self.pitch_env *= self.pitch_c;
            if self.pitch_env < 1e-6 {
                self.pitch_env = 0.0;
            }
        }
        if self.knock != 0.0 {
            self.knock *= self.knock_c;
            if self.knock < 1e-6 {
                self.knock = 0.0;
            }
        }
        if self.sigh != 0.0 {
            self.sigh *= self.sigh_c;
            if self.sigh < 1e-6 {
                self.sigh = 0.0;
            }
        }
        let g = env * self.amp * LEVEL;
        let mut out = main as f32 * g;
        if self.click_env != 0.0 {
            // White noise, high-passed at the click tone: a snap, not a thud.
            let a = math::exp(-std::f32::consts::TAU * s.click_hz / sr);
            let x = self.rng.bipolar();
            let hp = a * (self.click_hp.1 + x - self.click_hp.0);
            self.click_hp = (x, hp);
            out += hp * self.click_env * s.click * self.amp * LEVEL;
            self.click_env *= self.click_c;
            if self.click_env < 1e-6 {
                self.click_env = 0.0;
            }
        }
        self.age = self.age.saturating_add(1);
        (out, sub as f32 * g)
    }

    /// Whether the voice will only ever output zero from here.
    pub fn is_silent(&self) -> bool {
        self.env.is_silent() && self.click_env == 0.0
    }

    pub fn phase(&self) -> u32 {
        self.phase
    }
}

/// A PolyBLEP square at phase `t` with increment `dt`.
#[inline]
pub fn square(t: f64, dt: f64) -> f64 {
    let naive = if t < 0.5 { 1.0 } else { -1.0 };
    naive + blep(t, dt) - blep((t + 0.5).fract(), dt)
}

/// Two-sided polynomial correction of a unit step at phase 0.
#[inline]
pub fn blep(t: f64, dt: f64) -> f64 {
    if dt <= 0.0 {
        0.0
    } else if t < dt {
        let x = t / dt;
        x + x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + x + x + 1.0
    } else {
        0.0
    }
}
