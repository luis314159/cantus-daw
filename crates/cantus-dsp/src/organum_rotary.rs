//! The rotary speaker: a horn and a drum turning in a cabinet, heard by two microphones.
//! Kept free of anything organ-specific so it can become an insert effect of its own
//! ([`Rotary::process`] takes any stereo signal, as the cabinet's mono input takes the sum).
//!
//! ```text
//!         ┌ low-pass ×2 ─ drum line ─┬ mic L: Doppler read · baffle gain · filter ┐
//!  in ─ crossover (LR4)              │ mic R: …                                   ├ balance ─ cabinet ─ L/R
//!         └ high-pass ×2 ─ horn line ┬ mic L: direct + wall image, Doppler, directivity, filter
//!                                    └ mic R: …
//! ```
//!
//! The model follows Smith, Serafin, Abel & Berners, "Doppler simulation and the Leslie"
//! (DAFx-02), implemented from the paper's description:
//!
//! - **Doppler** is a delay line read at the travel time from the moving source to each
//!   microphone (4-point Hermite interpolation), so the pitch swing is whatever the
//!   geometry gives: at 7 turns a second a 17 cm horn tip moves 7.5 m/s, ±38 cents.
//! - **Directivity**: the horn is loudest and brightest pointing at the microphone; a gain
//!   (−15 dB pointing away) and a one-pole low-pass (12 kHz facing, 3 kHz away) follow the
//!   angle between the horn's axis and the path to the microphone.
//! - **One reflection**: the horn's image in the cabinet's back wall, a second read of the
//!   same line with its own distance and angle, 9 dB down (open back: 14 dB).
//! - **The drum** is a rotating baffle in front of a fixed woofer: mostly amplitude and
//!   filter modulation, with a small Doppler (an 8 cm effective radius).
//! - **Speed** is a first-order lag toward the target — separate time constants up and
//!   down, per rotor — and the angle is its integral, both in f64 and stepped per sample,
//!   so a render is the same at every block size. **Brake** decelerates to a creep and
//!   parks the rotor at the brake angle.
//!
//! Defaults are setBfree's documented ones (horn 40/423 rpm, drum 36/357 rpm, spin-up
//! 0.161 s / 4.127 s, spin-down 0.321 s / 1.371 s, radii 17 / 22 cm, crossover 800 Hz),
//! read from its documentation, not its code.

use super::filters::{Bq, OnePole};
use std::f32::consts::{FRAC_1_SQRT_2, TAU};

/// Speed of sound, m/s.
pub const SOUND_MPS: f32 = 343.0;
/// Horn radius (axis to mouth), m.
pub const HORN_RADIUS_M: f32 = 0.17;
/// Drum radius, m, and the effective radius its Doppler is computed with.
pub const DRUM_RADIUS_M: f32 = 0.22;
pub const DRUM_DOPPLER_M: f32 = 0.08;
/// From the rotor axis to the cabinet's front (and back wall), m.
pub const CABINET_HALF_M: f32 = 0.3;
/// The share of the horn that reaches the microphones without turning (leak).
pub const HORN_LEAK: f32 = 0.15;
/// A braking rotor slows to this, turns a second, then parks.
const CREEP_TPS: f64 = 0.5;
/// How fast the parking closes in, per second.
const PARK_RATE: f64 = 6.0;

const LINE: usize = 4096;

/// Stop (brake), chorale (slow) or tremolo (fast).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum RotorSpeed {
    Stop,
    #[default]
    Slow,
    Fast,
}

impl RotorSpeed {
    pub fn from_index(i: f32) -> RotorSpeed {
        match i.round() as i32 {
            0 => RotorSpeed::Stop,
            2 => RotorSpeed::Fast,
            _ => RotorSpeed::Slow,
        }
    }
}

/// Everything the rotary speaker is set to, in real units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RotarySettings {
    pub speed: RotorSpeed,
    pub horn_slow_rpm: f32,
    pub horn_fast_rpm: f32,
    pub drum_slow_rpm: f32,
    pub drum_fast_rpm: f32,
    /// Time constants of the speed lag, s.
    pub horn_up_s: f32,
    pub horn_down_s: f32,
    pub drum_up_s: f32,
    pub drum_down_s: f32,
    /// Horn against drum, dB (positive: more horn).
    pub balance_db: f32,
    pub crossover_hz: f32,
    /// An open-backed cabinet (less reflection, less low-mid boom).
    pub open: bool,
    /// Microphones' distance from the cabinet, cm.
    pub mic_distance_cm: f32,
    /// Angle between the two microphones, degrees.
    pub mic_spread_deg: f32,
    /// Where a braked rotor parks, degrees (0 = pointing at the listener).
    pub brake_deg: f32,
    /// Wet share, 0…1 (as an insert).
    pub mix: f32,
}

impl Default for RotarySettings {
    fn default() -> Self {
        RotarySettings {
            speed: RotorSpeed::Slow,
            horn_slow_rpm: 40.0,
            horn_fast_rpm: 420.0,
            drum_slow_rpm: 36.0,
            drum_fast_rpm: 357.0,
            horn_up_s: 0.16,
            horn_down_s: 0.32,
            drum_up_s: 4.1,
            drum_down_s: 1.4,
            balance_db: 0.0,
            crossover_hz: 800.0,
            open: false,
            mic_distance_cm: 60.0,
            mic_spread_deg: 120.0,
            brake_deg: 90.0,
            mix: 1.0,
        }
    }
}

/// One rotor: its angle (turns) and speed (turns a second).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Rotor {
    pub angle: f64,
    pub speed: f64,
    target: f64,
    up: f64,
    down: f64,
    braking: bool,
    park_at: f64,
    parked: bool,
}

impl Rotor {
    fn new(angle: f64) -> Self {
        Rotor {
            angle,
            speed: 0.0,
            target: 0.0,
            up: 0.0,
            down: 0.0,
            braking: false,
            park_at: 0.0,
            parked: false,
        }
    }

    fn lag(sr: f64, tau_s: f32) -> f64 {
        1.0 - (-1.0 / (tau_s.max(1e-4) as f64 * sr)).exp()
    }

    #[inline]
    fn step(&mut self, sr: f64) {
        if self.braking {
            if self.parked {
                return;
            }
            let dist = (self.park_at - self.angle).rem_euclid(1.0);
            if self.speed > CREEP_TPS {
                self.speed += (0.0 - self.speed) * self.down;
            } else {
                self.speed = CREEP_TPS.min(PARK_RATE * dist);
            }
            if dist < 1e-6 && self.speed < 1e-5 {
                self.angle = self.park_at;
                self.speed = 0.0;
                self.parked = true;
                return;
            }
        } else {
            let c = if self.target > self.speed {
                self.up
            } else {
                self.down
            };
            self.speed += (self.target - self.speed) * c;
        }
        self.angle += self.speed / sr;
        if self.angle >= 1.0 {
            self.angle -= 1.0;
        }
    }

    /// Revolutions a minute now.
    pub fn rpm(&self) -> f32 {
        (self.speed * 60.0) as f32
    }
}

/// A delay line read at a fractional delay.
#[derive(Clone, Debug)]
struct Line {
    buf: Box<[f32]>,
    pos: usize,
}

impl Line {
    fn new() -> Self {
        Line {
            buf: vec![0.0; LINE].into_boxed_slice(),
            pos: 0,
        }
    }

    #[inline]
    fn push(&mut self, x: f32) {
        self.pos = (self.pos + 1) & (LINE - 1);
        self.buf[self.pos] = x;
    }

    /// 4-point, 3rd-order Hermite at `d` samples back (d ≥ 1).
    #[inline]
    fn read(&self, d: f32) -> f32 {
        let d = d.clamp(1.0, (LINE - 4) as f32);
        let i = d as usize;
        let f = d - i as f32;
        let at = |k: usize| self.buf[(self.pos + LINE - k) & (LINE - 1)];
        let (xm1, x0, x1, x2) = (at(i - 1), at(i), at(i + 1), at(i + 2));
        let c1 = 0.5 * (x1 - xm1);
        let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
        let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
        ((c3 * f + c2) * f + c1) * f + x0
    }

    fn clear(&mut self) {
        self.buf.fill(0.0);
    }
}

/// `y += a (x − y)` with `a ≈ 1 − e^(−w)` computed without an exponential, for filters
/// whose cutoff moves every sample.
#[inline]
fn fast_coef(sr: f32, hz: f32) -> f32 {
    let w = TAU * hz / sr;
    w / (1.0 + w)
}

/// One microphone's filters.
#[derive(Copy, Clone, Debug, Default)]
struct Mic {
    horn: OnePole,
    image: OnePole,
    drum: OnePole,
    cab_peak: Bq,
    cab_lp: Bq,
    /// Position, m.
    x: f32,
    y: f32,
}

#[derive(Clone, Debug)]
pub struct Rotary {
    sr: f32,
    s: RotarySettings,
    pub horn: Rotor,
    pub drum: Rotor,
    lp: [Bq; 2],
    hp: [Bq; 2],
    horn_line: Line,
    drum_line: Line,
    mics: [Mic; 2],
    horn_gain: f32,
    drum_gain: f32,
    image_gain: f32,
}

/// Overall level: a steady tone through the default cabinet comes out near its input
/// level.
const NORM: f32 = 0.8;

impl Rotary {
    pub fn new(sample_rate: f32) -> Self {
        let mut r = Rotary {
            sr: sample_rate.max(1.0),
            s: RotarySettings::default(),
            horn: Rotor::new(0.0),
            drum: Rotor::new(0.25),
            lp: [Bq::identity(); 2],
            hp: [Bq::identity(); 2],
            horn_line: Line::new(),
            drum_line: Line::new(),
            mics: [Mic::default(); 2],
            horn_gain: 1.0,
            drum_gain: 1.0,
            image_gain: 0.35,
        };
        let s = r.s;
        r.s.crossover_hz = -1.0; // force every derived value to be computed
        r.set(&s);
        r.snap_speeds();
        r
    }

    pub fn settings(&self) -> &RotarySettings {
        &self.s
    }

    fn targets(&self) -> (f64, f64) {
        let s = &self.s;
        match s.speed {
            RotorSpeed::Stop => (0.0, 0.0),
            RotorSpeed::Slow => (s.horn_slow_rpm as f64 / 60.0, s.drum_slow_rpm as f64 / 60.0),
            RotorSpeed::Fast => (s.horn_fast_rpm as f64 / 60.0, s.drum_fast_rpm as f64 / 60.0),
        }
    }

    /// New settings. Allocation-free; filters are recomputed only when what decides them
    /// changed.
    pub fn set(&mut self, s: &RotarySettings) {
        let sr = self.sr;
        let old = self.s;
        self.s = *s;
        if old.crossover_hz != s.crossover_hz {
            for b in &mut self.lp {
                b.set_lowpass(sr, s.crossover_hz, FRAC_1_SQRT_2);
            }
            for b in &mut self.hp {
                b.set_highpass(sr, s.crossover_hz, FRAC_1_SQRT_2);
            }
        }
        if old.open != s.open || old.crossover_hz < 0.0 {
            let (peak_hz, peak_db, lp_hz) = if s.open {
                (90.0, 1.0, 8_000.0)
            } else {
                (120.0, 3.0, 6_500.0)
            };
            for m in &mut self.mics {
                m.cab_peak.set_peak(sr, peak_hz, peak_db, 1.2);
                m.cab_lp.set_lowpass(sr, lp_hz, 0.7);
            }
            self.image_gain = if s.open { 0.2 } else { 0.35 };
        }
        let d = CABINET_HALF_M + s.mic_distance_cm.max(1.0) * 0.01;
        let half = (s.mic_spread_deg * 0.5).to_radians();
        // Left microphone on the listener's left (+y), right on −y.
        self.mics[0].x = d * half.cos();
        self.mics[0].y = d * half.sin();
        self.mics[1].x = d * half.cos();
        self.mics[1].y = -d * half.sin();
        self.horn_gain = 10f32.powf(s.balance_db / 40.0);
        self.drum_gain = 10f32.powf(-s.balance_db / 40.0);
        let srd = sr as f64;
        self.horn.up = Rotor::lag(srd, s.horn_up_s);
        self.horn.down = Rotor::lag(srd, s.horn_down_s);
        self.drum.up = Rotor::lag(srd, s.drum_up_s);
        self.drum.down = Rotor::lag(srd, s.drum_down_s);
        let (ht, dt) = self.targets();
        self.horn.target = ht;
        self.drum.target = dt;
        let braking = s.speed == RotorSpeed::Stop;
        let park = (s.brake_deg as f64 / 360.0).rem_euclid(1.0);
        for r in [&mut self.horn, &mut self.drum] {
            if !braking || r.park_at != park {
                r.parked = false;
            }
            r.braking = braking;
            r.park_at = park;
        }
    }

    /// Put the rotors at their target speeds at once (a fresh instrument does not spin up
    /// from rest).
    pub fn snap_speeds(&mut self) {
        let (ht, dt) = self.targets();
        self.horn.speed = ht;
        self.drum.speed = dt;
    }

    /// Forget the audio (the rotors keep turning).
    pub fn clear(&mut self) {
        self.horn_line.clear();
        self.drum_line.clear();
        for b in self.lp.iter_mut().chain(self.hp.iter_mut()) {
            b.reset();
        }
        for m in &mut self.mics {
            m.horn.reset();
            m.image.reset();
            m.drum.reset();
            m.cab_peak.reset();
            m.cab_lp.reset();
        }
    }

    /// Back to the start: silence, and the rotors where a new speaker has them.
    pub fn reset(&mut self) {
        self.clear();
        self.horn.angle = 0.0;
        self.drum.angle = 0.25;
        for r in [&mut self.horn, &mut self.drum] {
            r.parked = false;
        }
        self.snap_speeds();
    }

    /// Turn the rotors one sample, in silence.
    #[inline]
    pub fn advance(&mut self) {
        let sr = self.sr as f64;
        self.horn.step(sr);
        self.drum.step(sr);
    }

    /// One mono sample in, one stereo pair of the cabinet out (wet only).
    #[inline]
    pub fn tick(&mut self, x: f32) -> (f32, f32) {
        self.advance();
        let lo = self.lp[0].run(x);
        let lo = self.lp[1].run(lo);
        let hi = self.hp[0].run(x);
        let hi = self.hp[1].run(hi);
        self.horn_line.push(hi);
        self.drum_line.push(lo);
        let sr = self.sr;
        let spm = sr / SOUND_MPS; // samples a metre
        let (hs, hc) = ((self.horn.angle as f32) * TAU).sin_cos();
        let (ds, dc) = ((self.drum.angle as f32) * TAU).sin_cos();
        let (hx, hy) = (HORN_RADIUS_M * hc, HORN_RADIUS_M * hs);
        // The horn's image in the back wall at x = −CABINET_HALF_M.
        let (ix, iy) = (-2.0 * CABINET_HALF_M - hx, hy);
        let (dx, dy) = (DRUM_DOPPLER_M * dc, DRUM_DOPPLER_M * ds);
        let mut out = [0.0f32; 2];
        for (k, m) in self.mics.iter_mut().enumerate() {
            // Direct horn.
            let (vx, vy) = (m.x - hx, m.y - hy);
            let l = (vx * vx + vy * vy).sqrt();
            let c = (hc * vx + hs * vy) / l;
            let face = 0.5 + 0.5 * c;
            let g = 0.18 + 0.82 * face * face;
            m.horn.a = fast_coef(sr, 3_000.0 + 9_000.0 * face * face);
            let direct = m.horn.run(self.horn_line.read(l * spm)) * g;
            // Its image: direction mirrored in the wall.
            let (wx, wy) = (m.x - ix, m.y - iy);
            let li = (wx * wx + wy * wy).sqrt();
            let ci = (-hc * wx + hs * wy) / li;
            let fi = 0.5 + 0.5 * ci;
            let gi = (0.18 + 0.82 * fi * fi) * self.image_gain;
            m.image.a = fast_coef(sr, 2_000.0 + 6_000.0 * fi * fi);
            let image = m.image.run(self.horn_line.read(li * spm)) * gi;
            let dist0 = (m.x * m.x + m.y * m.y).sqrt();
            let leak = HORN_LEAK * self.horn_line.read(dist0 * spm);
            let horn = direct + image + leak;
            // The drum: the baffle's opening toward the microphone is louder and brighter.
            let (ux, uy) = (m.x - dx, m.y - dy);
            let ld = (ux * ux + uy * uy).sqrt();
            let cd = (dc * ux + ds * uy) / ld;
            let fd = 0.5 + 0.5 * cd;
            m.drum.a = fast_coef(sr, 500.0 + 2_500.0 * fd);
            let drum = m.drum.run(self.drum_line.read(ld * spm)) * (0.4 + 0.6 * fd);
            let y = self.horn_gain * horn + self.drum_gain * drum;
            out[k] = m.cab_lp.run(m.cab_peak.run(y)) * NORM;
        }
        (out[0], out[1])
    }

    /// As an insert: the stereo input summed to the cabinet's mono input, the mix applied.
    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let mix = self.s.mix.clamp(0.0, 1.0);
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            let x = 0.5 * (*l + *r);
            let (wl, wr) = self.tick(x);
            *l = *l * (1.0 - mix) + wl * mix;
            *r = *r * (1.0 - mix) + wr * mix;
        }
    }
}
