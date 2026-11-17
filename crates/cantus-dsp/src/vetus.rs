//! Vetus, lo-fi (ADR 0130): a sound aged as an old medium ages it.
//!
//! Six modules in a fixed order, each with its switch (crossfaded over 10 ms, so a switch
//! never clicks):
//!
//! 1. **Bits** — a quantiser of 1 … 24 bits, rounding, truncation or μ-law, with or without
//!    dither ([`digital::Bits`]).
//! 2. **Rate** — a sample and hold down to 1 kHz, raw (its aliasing is the sound) or behind
//!    an anti-alias filter, and optionally smoothed after it ([`digital::Rate`]).
//! 3. **Wobble** — wow (slow, synced or free, partly a wandering random as a tape's is) and
//!    flutter (fast): a delay moved as Vibrans moves it (`modfx_line`, `modfx_lfo`,
//!    `vibrans::sweep_for`: the depth in cents whatever the rate).
//! 4. **Age** — dropouts and worn oxide: a gain and a dulling low-pass that are functions of
//!    the song sample ([`age::Age`]).
//! 5. **Noise** — crackle and pops, tape or vinyl hiss, mains hum, all hashes of the song
//!    sample ([`noise::Noise`]); optionally following the music (100 %: only while it plays).
//! 6. **Tone** — the bandwidth and colour of the medium that plays it back (tape, radio,
//!    telephone, gramophone), the noise included: two resonant edges and the medium's bells.
//!
//! **Character** takes every module that is on towards its oldest ([`VetusSettings::aged`]):
//! at 100 % each is at the end its kind of wear leads to (4 bits, 4 kHz, 40 ct of wow …),
//! never younger than it was set.
//!
//! **Nulls**: every module off (the defaults) is a wire to the bit at any Character, mix or
//! block size; bits at 24 without dither is a wire. **Time**: everything that varies is a
//! function of the song sample (the noise, the dropouts, the wear, the dither, the tone's
//! and the age's 16-sample grid), so offline equals real time and a render from bar 5 is the
//! whole render's bar 5. **At rest** (ADR 0031 §5.1) when every glide has landed, the
//! wobble's lines, the hold and every filter are empty — and the noise is off, or follows
//! the music and the music is silent: a noise that plays in silence is never at rest.

#[path = "vetus_age.rs"]
pub mod age;
#[path = "vetus_digital.rs"]
pub mod digital;
#[path = "vetus_filter.rs"]
pub mod filter;
#[path = "vetus_noise.rs"]
pub mod noise;
#[path = "vetus_unit.rs"]
mod unit;

pub use unit::{t, Vetus};

use crate::math;
use crate::modfx_lfo::Rate as LfoRate;
use age::AgeParams;
use digital::{BitsMode, RateFilter};
use noise::NoiseParams;

/// The media of the tone module (`vetus.medium.{i}`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Medium {
    #[default]
    Tape,
    Radio,
    Telephone,
    Gramophone,
}

impl Medium {
    pub const ALL: [Medium; 4] = [
        Medium::Tape,
        Medium::Radio,
        Medium::Telephone,
        Medium::Gramophone,
    ];

    pub fn from_index(v: f32) -> Medium {
        let i = if v.is_finite() { v.round() as i64 } else { 0 };
        Medium::ALL[i.clamp(0, 3) as usize]
    }

    /// The medium's colour: up to two bells (Hz, Q, dB; 0 dB is none), and whether its top
    /// falls at 24 dB an octave.
    pub fn colour(self) -> ([(f32, f32, f32); 2], bool) {
        match self {
            // A tape head's bump.
            Medium::Tape => ([(100.0, 1.0, 2.5), (1_000.0, 1.0, 0.0)], false),
            // A broadcast's forward middle.
            Medium::Radio => ([(1_200.0, 0.7, 4.0), (1_000.0, 1.0, 0.0)], false),
            // The carbon microphone's resonance.
            Medium::Telephone => ([(1_800.0, 1.4, 6.0), (1_000.0, 1.0, 0.0)], true),
            // An acoustic horn's two resonances.
            Medium::Gramophone => ([(1_100.0, 2.0, 6.0), (2_600.0, 3.0, 4.0)], true),
        }
    }
}

/// Vetus's settings, as the table gives them (`cantus_core::fx::vetus`).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct VetusSettings {
    pub mix: f32,
    pub out_db: f32,
    /// Character, 0 … 1.
    pub character: f32,
    pub bits_on: bool,
    pub bits: f32,
    pub dither: bool,
    pub bits_mode: BitsMode,
    pub rate_on: bool,
    pub rate_hz: f32,
    pub rate_filter: RateFilter,
    pub wob_on: bool,
    /// The wow's swing either way, cents; its rate; the share that wanders.
    pub wow_ct: f32,
    pub wow_rate: LfoRate,
    pub drift: f32,
    pub flut_ct: f32,
    pub flut_hz: f32,
    pub age_on: bool,
    pub age: AgeParams,
    pub noise_on: bool,
    pub noise: NoiseParams,
    /// 0 … 1: how much the noise follows the music.
    pub follow: f32,
    pub tone_on: bool,
    pub medium: Medium,
    pub lo: f32,
    pub hi: f32,
    pub res: f32,
}

impl Default for VetusSettings {
    fn default() -> Self {
        VetusSettings {
            mix: 1.0,
            out_db: 0.0,
            character: 0.0,
            bits_on: false,
            bits: 8.0,
            dither: false,
            bits_mode: BitsMode::Round,
            rate_on: false,
            rate_hz: 12_000.0,
            rate_filter: RateFilter::None,
            wob_on: false,
            wow_ct: 12.0,
            wow_rate: LfoRate::Hz(0.5),
            drift: 0.5,
            flut_ct: 4.0,
            flut_hz: 9.0,
            age_on: false,
            age: AgeParams {
                drop: 0.3,
                wear: 0.3,
            },
            noise_on: false,
            noise: NoiseParams {
                crackle: 0.35,
                hiss: 0.25,
                vinyl: false,
                hum: 0.0,
                hum_60: false,
            },
            follow: 0.0,
            tone_on: false,
            medium: Medium::Tape,
            lo: 50.0,
            hi: 12_000.0,
            res: 0.2,
        }
    }
}

/// Where Character takes each module at 100 %: its oldest.
pub mod oldest {
    pub const BITS: f32 = 4.0;
    pub const RATE_HZ: f32 = 4_000.0;
    pub const WOW_CT: f32 = 40.0;
    pub const FLUT_CT: f32 = 14.0;
    pub const DROP: f32 = 0.8;
    pub const WEAR: f32 = 0.8;
    pub const CRACKLE: f32 = 0.8;
    pub const HISS: f32 = 0.6;
    pub const LO: f32 = 300.0;
    pub const HI: f32 = 3_500.0;
    pub const RES: f32 = 0.5;
}

/// `v` moved a share `c` of the way to `old`, only if `old` is above it.
fn up(v: f32, old: f32, c: f32) -> f32 {
    if old > v {
        v + (old - v) * c
    } else {
        v
    }
}

/// `v` moved a share `c` of the way to `old`, only if `old` is below it.
fn down(v: f32, old: f32, c: f32) -> f32 {
    if old < v {
        v + (old - v) * c
    } else {
        v
    }
}

/// As [`down`], in octaves (a frequency).
fn down_log(v: f32, old: f32, c: f32) -> f32 {
    if old < v && v > 0.0 {
        v * math::exp2(math::log2(old / v) * c)
    } else {
        v
    }
}

/// As [`up`], in octaves.
fn up_log(v: f32, old: f32, c: f32) -> f32 {
    if old > v && v > 0.0 {
        v * math::exp2(math::log2(old / v) * c)
    } else {
        v
    }
}

impl VetusSettings {
    /// The settings Character leaves: every module that is on, a share `character` of the
    /// way from its own setting to its oldest (never younger); the rest as set.
    pub fn aged(&self) -> VetusSettings {
        let c = self.character.clamp(0.0, 1.0);
        let mut s = *self;
        if c == 0.0 {
            return s;
        }
        if s.bits_on {
            s.bits = down(s.bits, oldest::BITS, c);
        }
        if s.rate_on {
            s.rate_hz = down_log(s.rate_hz, oldest::RATE_HZ, c);
        }
        if s.wob_on {
            s.wow_ct = up(s.wow_ct, oldest::WOW_CT, c);
            s.flut_ct = up(s.flut_ct, oldest::FLUT_CT, c);
        }
        if s.age_on {
            s.age.drop = up(s.age.drop, oldest::DROP, c);
            s.age.wear = up(s.age.wear, oldest::WEAR, c);
        }
        if s.noise_on {
            s.noise.crackle = up(s.noise.crackle, oldest::CRACKLE, c);
            s.noise.hiss = up(s.noise.hiss, oldest::HISS, c);
        }
        if s.tone_on {
            s.lo = up_log(s.lo, oldest::LO, c);
            s.hi = down_log(s.hi, oldest::HI, c);
            s.res = up(s.res, oldest::RES, c);
        }
        s
    }

    /// The modules' switches in the signal's order (bits, rate, wobble, age, noise, tone).
    pub fn switches(&self) -> [bool; 6] {
        [
            self.bits_on,
            self.rate_on,
            self.wob_on,
            self.age_on,
            self.noise_on,
            self.tone_on,
        ]
    }

    /// Whether the noise makes any sound when on.
    pub fn noise_sounds(&self) -> bool {
        self.noise.crackle > 0.0 || self.noise.hiss > 0.0 || self.noise.hum > 0.0
    }
}

/// The tone module's response at `hz` (dB), as heard: each trapezoidal state-variable
/// section is its analogue prototype at the pre-warped frequency, so this is exact.
pub fn tone_db(s: &VetusSettings, hz: f32, sample_rate: f32) -> f32 {
    let w = |f: f32| {
        let nyq = 0.4999 * f64::from(sample_rate);
        math::tan(std::f64::consts::PI * f64::from(f).clamp(1e-3, nyq) / f64::from(sample_rate))
    };
    let at = w(hz);
    // |N(s)/D(s)|² at s = jΩ for a section at `fc`: (a0 + a1·s + a2·s²) / (1 + s/q + s²).
    let section = |fc: f32, q: f64, num: [f64; 3]| -> f64 {
        let o = at / w(fc);
        let (nr, ni) = (num[0] - num[2] * o * o, num[1] * o);
        let (dr, di) = (1.0 - o * o, o / q);
        (nr * nr + ni * ni) / (dr * dr + di * di)
    };
    let q = std::f64::consts::FRAC_1_SQRT_2 + 2.3 * f64::from(s.res.clamp(0.0, 1.0));
    let (lo, hi) = (s.lo.min(0.45 * sample_rate), s.hi.min(0.45 * sample_rate));
    let mut p = section(lo, q, [0.0, 0.0, 1.0]) * section(hi, q, [1.0, 0.0, 0.0]);
    let (bells, steep) = s.medium.colour();
    if steep {
        p *= section(hi, std::f64::consts::FRAC_1_SQRT_2, [1.0, 0.0, 0.0]);
    }
    for (fc, bq, db) in bells {
        if db != 0.0 {
            let a = math::exp2(f64::from(db) / 40.0 * std::f64::consts::LOG2_10);
            let k = 1.0 / (f64::from(bq) * a);
            // The bell's prototype: (1 + k·A²·s + s²) / (1 + k·s + s²).
            p *= section(fc, 1.0 / k, [1.0, k * a * a, 1.0]);
        }
    }
    (10.0 * math::log10(p.max(1e-30))) as f32
}

#[cfg(test)]
#[path = "vetus_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "vetus_tests2.rs"]
mod tests2;
