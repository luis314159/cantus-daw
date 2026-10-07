//! Gravis's synth bass: PolyBLEP oscillators in unison on Szabo's measured supersaw curves,
//! a sub derived from the main oscillator's phase, the ladder / diode / SVF filters and the
//! TB-303's accent with its accumulating sweep (`research/instruments/bass.md` §3.5, §4.4).
//!
//! **Unison** (Szabo, "How to emulate the Super Saw", KTH 2010): seven oscillators at
//! `1 + offset·curve(detune)` with the measured offsets, the detune knob on its measured
//! 11th-order curve, and the mix knob on the measured centre (linear) and side (parabolic)
//! gains. Fewer voices take the offsets nearest the centre.
//!
//! **The main phase** is a `u32` at exactly the note's frequency; the centre voice *is* it,
//! and the sub counts its wraps (a flip-flop per octave), so the sub's zero crossings fall
//! on every second (fourth) main crossing through glides and bends: no drift, no beating,
//! the same peak on every note.
//!
//! **Accent** (Whittle's Devil Fish notes): an accented note (velocity at or above the
//! threshold) opens the filter by its own envelope, gets the shortest decay and is louder;
//! the sweep capacitor charges from that envelope faster than it discharges, so a run of
//! close accents climbs — the "wow" of an acid line.

use super::e808::blep;
use super::filter::{BassFilter, DiodeCal, FilterKind};
use super::voice::{coef_60db, Env, EnvTimes, Rng};
use crate::math;

/// Szabo's measured unison offsets (relative frequency at full detune).
pub const UNISON_OFFSETS: [f64; 7] = [
    -0.110_023_13,
    -0.062_884_39,
    -0.019_523_56,
    0.0,
    0.019_912_21,
    0.062_165_38,
    0.107_452_42,
];
/// Which of the seven play for 1…7 voices: the ones nearest the centre.
const VOICES_FOR: [&[usize]; 8] = [
    &[],
    &[3],
    &[2, 4],
    &[2, 3, 4],
    &[1, 2, 4, 5],
    &[1, 2, 3, 4, 5],
    &[0, 1, 2, 4, 5, 6],
    &[0, 1, 2, 3, 4, 5, 6],
];
/// The centre voice.
const CENTRE: usize = 3;
/// Level at full velocity: −6 dBFS.
pub const LEVEL: f32 = 0.5;
/// The accented note's filter decay, at most.
pub const ACCENT_DECAY_MS: f32 = 200.0;

/// Szabo's detune curve: the knob (0…1) to the fraction of the full offsets.
pub fn detune_curve(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    const C: [f64; 12] = [
        0.003_011_559_6,
        0.671_741_763_4,
        -24.187_882_439_1,
        404.270_393_838_8,
        -3_425.083_659_131_8,
        17_019.951_858_008_0,
        -53_046.964_275_187_5,
        106_649.667_915_829_2,
        -138_150.676_108_054_8,
        111_363.480_872_936_8,
        -50_818.865_204_592_4,
        10_028.731_289_163_4,
    ];
    let mut y = 0.0;
    for c in C.iter().rev() {
        y = y * x + c;
    }
    y.max(0.0)
}

/// Szabo's mix curves: (centre gain, side gain) for the mix knob 0…1.
pub fn mix_gains(x: f64) -> (f64, f64) {
    let x = x.clamp(0.0, 1.0);
    (
        -0.553_66 * x + 0.997_85,
        -0.737_64 * x * x + 1.284_1 * x + 0.044_372,
    )
}

/// The outer voice's detune in cents for a voice count and knob.
pub fn outer_cents(unison: usize, detune: f64) -> f64 {
    let set = VOICES_FOR[unison.clamp(1, 7)];
    let off = set
        .iter()
        .map(|i| UNISON_OFFSETS[*i].abs())
        .fold(0.0, f64::max);
    1200.0 * math::log2(1.0 + off * detune_curve(detune))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Wave {
    #[default]
    Saw,
    Square,
    Pulse,
    Triangle,
}

impl Wave {
    pub fn from_index(i: f32) -> Self {
        match i.round() as i32 {
            1 => Wave::Square,
            2 => Wave::Pulse,
            3 => Wave::Triangle,
            _ => Wave::Saw,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SynthSettings {
    pub wave: Wave,
    pub pw: f32,
    pub unison: usize,
    pub detune: f32,
    pub spread: f32,
    pub width: f32,
    pub osc2: f32,
    /// Frequency ratio of the second oscillator.
    pub osc2_ratio: f64,
    pub filter: FilterKind,
    pub cutoff: f32,
    pub reso: f32,
    pub bass_comp: f32,
    pub drive: f32,
    pub env_oct: f32,
    pub key_track: f32,
    pub fenv: EnvTimes,
    pub aenv: EnvTimes,
    pub accent: f32,
    pub accent_sweep: bool,
    /// Velocity (0…1) at or above which a note is accented.
    pub accent_vel: f32,
}

impl Default for SynthSettings {
    fn default() -> Self {
        SynthSettings {
            wave: Wave::Saw,
            pw: 0.5,
            unison: 1,
            detune: 0.3,
            spread: 0.5,
            width: 0.0,
            osc2: 0.0,
            osc2_ratio: 0.5,
            filter: FilterKind::Ladder,
            cutoff: 400.0,
            reso: 0.2,
            bass_comp: 0.5,
            drive: 0.0,
            env_oct: 2.0,
            key_track: 0.5,
            fenv: EnvTimes {
                attack_ms: 1.0,
                hold_ms: 0.0,
                decay_ms: 300.0,
                sustain: 0.0,
                release_ms: 200.0,
            },
            aenv: EnvTimes {
                attack_ms: 1.0,
                hold_ms: 0.0,
                decay_ms: 600.0,
                sustain: 0.8,
                release_ms: 120.0,
            },
            accent: 0.5,
            accent_sweep: true,
            accent_vel: 100.0 / 127.0,
        }
    }
}

/// How a note's oscillator phases begin.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PhaseStart {
    /// The same fixed spread on every note.
    Reset,
    /// Seeded by the note.
    Random(u64),
    /// Carry on from where they are.
    Free,
}

/// The TB-303's accent: an envelope per accented note, and the sweep capacitor it charges.
#[derive(Copy, Clone, Debug, Default)]
pub struct Accent {
    pub env: f32,
    pub sweep: f32,
    env_c: f32,
    charge: f32,
    discharge: f32,
}

impl Accent {
    pub fn new(sr: f32) -> Self {
        Accent {
            env: 0.0,
            sweep: 0.0,
            env_c: coef_60db(ACCENT_DECAY_MS * 1.5, sr),
            // The capacitor charges through the diode in tens of ms and leaks over hundreds.
            charge: 1.0 - math::exp(-1.0 / (0.030 * sr)),
            discharge: 1.0 - math::exp(-1.0 / (0.350 * sr)),
        }
    }

    pub fn trigger(&mut self) {
        self.env = 1.0;
    }

    pub fn reset(&mut self) {
        self.env = 0.0;
        self.sweep = 0.0;
    }

    /// One sample; returns the cutoff offset in octaves for accent depth `amount`.
    #[inline]
    pub fn next(&mut self, amount: f32, sweep_on: bool) -> f32 {
        if self.env == 0.0 && self.sweep == 0.0 {
            return 0.0;
        }
        let rate = if self.env > self.sweep {
            self.charge
        } else {
            self.discharge
        };
        self.sweep += (self.env - self.sweep) * rate;
        self.env *= self.env_c;
        if self.env < 1e-6 {
            self.env = 0.0;
        }
        if self.sweep < 1e-6 && self.env == 0.0 {
            self.sweep = 0.0;
        }
        let sweep = if sweep_on { 2.5 * self.sweep } else { 0.0 };
        amount * (1.5 * self.env + sweep)
    }
}

/// One synth voice. `Copy`, so a retrigger can hand it to the declick slot.
#[derive(Copy, Clone, Debug, Default)]
pub struct SynthVoice {
    /// The main phase, at exactly the note's frequency: the centre voice and the sub's clock.
    main: u32,
    wraps: u32,
    uni: [u32; 7],
    osc2: u32,
    filter: BassFilter,
    side_filter: BassFilter,
    pub aenv: Env,
    pub fenv: Env,
    pub accented: bool,
    amp: f32,
    pub f0: f64,
    pub cutoff_now: f32,
}

/// The fixed spread Reset starts the unison voices at (golden-ratio steps): identical on
/// every note, and not all in phase, so the first cycle does not spike.
const RESET_PHASES: [f64; 7] = [0.0, 0.618, 0.236, 0.0, 0.854, 0.472, 0.090];

impl SynthVoice {
    pub fn start(
        &mut self,
        s: &SynthSettings,
        sr: f32,
        amp: f32,
        accented: bool,
        start: PhaseStart,
    ) {
        let to_u32 = |x: f64| (x.fract() * 4_294_967_296.0) as u32;
        match start {
            PhaseStart::Reset => {
                self.main = 0;
                self.wraps = 0;
                for (p, r) in self.uni.iter_mut().zip(RESET_PHASES) {
                    *p = to_u32(r);
                }
                self.osc2 = 0;
            }
            PhaseStart::Random(seed) => {
                let mut rng = Rng(seed);
                self.main = 0;
                self.wraps = 0;
                for p in &mut self.uni {
                    *p = (rng.next_u64() >> 32) as u32;
                }
                self.uni[CENTRE] = 0;
                self.osc2 = (rng.next_u64() >> 32) as u32;
            }
            PhaseStart::Free => {}
        }
        self.filter.reset();
        self.side_filter.reset();
        self.aenv = Env::default();
        self.aenv.set(&s.aenv, sr);
        self.aenv.gate_on();
        self.fenv = Env::default();
        self.set_fenv(s, sr, accented);
        self.fenv.gate_on();
        self.accented = accented;
        self.amp = amp;
    }

    fn set_fenv(&mut self, s: &SynthSettings, sr: f32, accented: bool) {
        let mut f = s.fenv;
        if accented {
            f.decay_ms = f.decay_ms.min(ACCENT_DECAY_MS);
        }
        self.fenv.set(&f, sr);
    }

    /// New envelope times for a sounding voice.
    pub fn retime(&mut self, s: &SynthSettings, sr: f32) {
        self.aenv.set(&s.aenv, sr);
        let a = self.accented;
        self.set_fenv(s, sr, a);
    }

    pub fn is_active(&self) -> bool {
        self.aenv.is_active()
    }

    pub fn is_silent(&self) -> bool {
        self.aenv.is_silent()
    }

    #[inline]
    fn wave(w: Wave, t: f64, dt: f64, pw: f64) -> f64 {
        match w {
            Wave::Saw => 2.0 * t - 1.0 - blep(t, dt),
            Wave::Square => super::e808::square(t, dt),
            Wave::Pulse => {
                let naive = if t < pw { 1.0 } else { -1.0 };
                naive + blep(t, dt) - blep((t - pw + 1.0).fract(), dt) - (2.0 * pw - 1.0)
            }
            Wave::Triangle => {
                if t < 0.5 {
                    4.0 * t - 1.0
                } else {
                    3.0 - 4.0 * t
                }
            }
        }
    }

    /// One sample: `(mid, side, sub)`, the amplitude envelope applied. `accent_oct` is the
    /// accent's cutoff offset now, `note` the pitch for key tracking.
    #[allow(clippy::too_many_arguments)]
    #[inline]
    pub fn next(
        &mut self,
        s: &SynthSettings,
        base_hz: f64,
        note: f64,
        sr: f32,
        sub_div: u32,
        sub_wave: u32,
        accent_oct: f32,
        cal: &DiodeCal,
    ) -> (f32, f32, f32) {
        const SCALE: f64 = 4_294_967_296.0;
        let f = base_hz;
        self.f0 = f;
        let inc_f = (f / sr as f64).clamp(0.0, 0.49);
        let inc = (inc_f * SCALE).round() as u32;
        let pw = s.pw.clamp(0.05, 0.95) as f64;
        let n = s.unison.clamp(1, 7);
        let (cg, sg) = if n == 1 {
            (1.0, 0.0)
        } else {
            mix_gains(s.spread as f64)
        };
        let set = VOICES_FOR[n];
        let norm = {
            let e: f64 = set
                .iter()
                .map(|i| if *i == CENTRE { cg * cg } else { sg * sg })
                .sum();
            if e > 0.0 {
                1.0 / e.sqrt()
            } else {
                1.0
            }
        };
        let curve = detune_curve(s.detune as f64);
        let (mut mid, mut side) = (0.0f64, 0.0f64);
        let wide = s.width > 0.0;
        for (k, &i) in set.iter().enumerate() {
            let (t, dt) = if i == CENTRE {
                (self.main as f64 / SCALE, inc_f)
            } else {
                let dt = (inc_f * (1.0 + UNISON_OFFSETS[i] * curve)).clamp(0.0, 0.49);
                let t = self.uni[i] as f64 / SCALE;
                self.uni[i] = self.uni[i].wrapping_add((dt * SCALE).round() as u32);
                (t, dt)
            };
            let g = if i == CENTRE { cg } else { sg };
            let v = Self::wave(s.wave, t, dt, pw) * g * norm;
            mid += v;
            if wide && i != CENTRE {
                side += if k % 2 == 0 { v } else { -v };
            }
        }
        if s.osc2 > 0.0 {
            let dt = (inc_f * s.osc2_ratio).clamp(0.0, 0.49);
            let t = self.osc2 as f64 / SCALE;
            mid += Self::wave(s.wave, t, dt, pw) * s.osc2 as f64;
            self.osc2 = self.osc2.wrapping_add((dt * SCALE).round() as u32);
        }
        // The sub, from the main phase's wraps.
        let t = self.main as f64 / SCALE;
        let sub_t = (t + (self.wraps % sub_div) as f64) / sub_div as f64;
        let sub = match sub_wave {
            1 => {
                if sub_t < 0.5 {
                    4.0 * sub_t - 1.0
                } else {
                    3.0 - 4.0 * sub_t
                }
            }
            2 => super::e808::square(sub_t, inc_f / sub_div as f64),
            _ => math::sin(std::f64::consts::TAU * sub_t),
        };
        let (next, wrapped) = self.main.overflowing_add(inc);
        self.main = next;
        if wrapped {
            self.wraps = self.wraps.wrapping_add(1);
        }
        // Filter.
        let fenv = self.fenv.next();
        let oct = s.env_oct * fenv + s.key_track * ((note - 60.0) / 12.0) as f32 + accent_oct;
        let cutoff = (s.cutoff * math::exp2(oct)).clamp(10.0, 0.45 * sr);
        self.cutoff_now = cutoff;
        let reso = if self.accented {
            (s.reso + 0.15 * s.accent).min(1.0)
        } else {
            s.reso
        };
        let y = self.filter.process(
            s.filter,
            mid as f32,
            cutoff,
            reso,
            s.bass_comp,
            s.drive,
            sr,
            cal,
        );
        let ys = if wide {
            self.side_filter.process(
                s.filter,
                (side * s.width as f64) as f32,
                cutoff,
                reso,
                s.bass_comp,
                s.drive,
                sr,
                cal,
            )
        } else {
            0.0
        };
        let env = self.aenv.next();
        let boost = if self.accented {
            1.0 + 0.6 * s.accent
        } else {
            1.0
        };
        let g = env * self.amp * LEVEL * boost;
        (y * g, ys * g, sub as f32 * env * self.amp * LEVEL)
    }
}
