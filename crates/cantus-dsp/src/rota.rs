//! Rota, the rotary speaker (ADR 0133): Organum's cabinet
//! ([`crate::organum::rotary::Rotary`] — horn and drum, Doppler, directivity, the wall's
//! reflection, two microphones) as an insert for any stereo signal.
//!
//! ```text
//!  in (L+R)/2 ─ drive (the cabinet's valve amp, 2× oversampled) ─ horn & drum ─ mics L/R ─ mix ─ out
//! ```
//!
//! - **The rotors are Rota's own**, each a speed that glides to its target (a first-order
//!   lag, its own time constants up and down — the published spin-up and spin-down) and an
//!   angle kept in **fixed point** (turns × 2⁶⁴): at a constant speed the angle advances
//!   by the same integer every sample, so a skip of `n` samples adds `n` times it — the
//!   same bits as playing them ([`Rota::skip`], closed form). Stop brakes to a creep and
//!   parks the rotors at the brake angle, as the cabinet's brake does.
//! - **Speeds and ramps** default to Herrera, Hanson & Abel's measurements of a Leslie
//!   (AES 127th Convention, 2009; as setBfree documents them): horn 40.32 / 423.36 rpm,
//!   drum 36 / 357.3 rpm, spin-up 0.161 s / 4.127 s, spin-down 0.321 s / 1.371 s — inside
//!   the spread of the published 122 and 147 measurements (horn 40 – 49 / 400 – 423 rpm,
//!   drum 36 – 49 / 342 – 396 rpm).
//! - **Notes** toggle slow and fast ([`Rota::toggle`], the half-moon switch).
//! - **Drive**: the cabinet's valve amp as a 12AX7 stage's static curve (Koren's triode,
//!   solved off the audio thread into a table), 2× oversampled through the minimum-phase
//!   half-bands. The speaker adds no latency: what delay there is (the microphones'
//!   distance, a sample of the oversampler) is the sound's own travel, not a delay to
//!   compensate.
//!
//! **The null**: the mix starts at 0 — the dry passes untouched, to the bit. **Rest**:
//! once the lines have been silent long enough and the tail is under −110 dB the cabinet's
//! lines and filters are cleared; at rest only the rotors move, and only at a constant
//! speed (a ramp is never at rest).

use crate::fervor::os;
use crate::organum::rotary::{Rotary, RotarySettings, RotorSpeed};

/// A braking rotor slows to this (turns a second), then parks.
const CREEP_TPS: f64 = 0.5;
const PARK_RATE: f64 = 6.0;
/// Rest: silence in for longer than the lines (4 096) and the filters' tails, the output
/// under this.
const SETTLE: u32 = 8_192;
const TAIL_FLOOR: f32 = 3e-6;
const QUIET_TAIL: u32 = 2_048;
/// The drive's table: points over ±`DRIVE_SPAN` volts at the valve's grid.
const TABLE: usize = 1_025;
const DRIVE_SPAN: f64 = 12.0;
const TWO64: f64 = 18_446_744_073_709_551_616.0;
/// A ramp's last stretch: the speed moves at least this much a sample (turns a second;
/// at 48 kHz 0.24 turns a second each second).
const MIN_STEP: f64 = 5e-6;

/// Everything Rota is set to.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RotaSettings {
    pub speed: RotorSpeed,
    pub horn_slow_rpm: f32,
    pub horn_fast_rpm: f32,
    pub drum_slow_rpm: f32,
    pub drum_fast_rpm: f32,
    /// The ramps' times (s): spin-up and spin-down, horn and drum.
    pub horn_up_s: f32,
    pub horn_down_s: f32,
    pub drum_up_s: f32,
    pub drum_down_s: f32,
    /// Horn against drum (dB, positive: more horn).
    pub balance_db: f32,
    /// The microphones' angle (degrees) and distance (cm).
    pub spread_deg: f32,
    pub distance_cm: f32,
    /// An open-backed cabinet.
    pub open: bool,
    /// The amp's drive, 0 … 1 (0: clean).
    pub drive: f32,
    /// Wet share 0 … 1 (0: the null).
    pub mix: f32,
    pub out_db: f32,
}

impl Default for RotaSettings {
    fn default() -> Self {
        RotaSettings {
            speed: RotorSpeed::Slow,
            horn_slow_rpm: 40.32,
            horn_fast_rpm: 423.36,
            drum_slow_rpm: 36.0,
            drum_fast_rpm: 357.3,
            horn_up_s: 0.161,
            horn_down_s: 0.321,
            drum_up_s: 4.127,
            drum_down_s: 1.371,
            balance_db: 0.0,
            spread_deg: 120.0,
            distance_cm: 60.0,
            open: false,
            drive: 0.0,
            mix: 0.0,
            out_db: 0.0,
        }
    }
}

/// A rotor: speed (turns a second) gliding to its target, angle in fixed point.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Spin {
    pub phase: u64,
    pub speed: f64,
    target: f64,
    up: f64,
    down: f64,
    braking: bool,
    park_at: u64,
    parked: bool,
}

impl Spin {
    fn new(phase: u64) -> Spin {
        Spin {
            phase,
            speed: 0.0,
            target: 0.0,
            up: 0.0,
            down: 0.0,
            braking: false,
            park_at: 0,
            parked: false,
        }
    }

    /// The share of the gap a lag closes per sample for time constant `tau` (s).
    fn lag(sr: f64, tau: f32) -> f64 {
        1.0 - crate::math::exp(-1.0 / (tau.max(1e-4) as f64 * sr))
    }

    /// The angle in turns, 0 … 1.
    pub fn turns(&self) -> f64 {
        (self.phase >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// Revolutions a minute now.
    pub fn rpm(&self) -> f64 {
        self.speed * 60.0
    }

    #[inline]
    fn inc(&self, sr: f64) -> u64 {
        (self.speed / sr * TWO64) as u64
    }

    /// Steady: a constant speed (or parked) — what a skip can carry in closed form.
    pub fn steady(&self) -> bool {
        if self.braking {
            self.parked
        } else {
            self.speed == self.target
        }
    }

    #[inline]
    fn step(&mut self, sr: f64) {
        if self.braking {
            if self.parked {
                return;
            }
            let dist = self.park_at.wrapping_sub(self.phase) as f64 / TWO64;
            if self.speed > CREEP_TPS {
                self.speed += (0.0 - self.speed) * self.down;
            } else {
                self.speed = CREEP_TPS.min(PARK_RATE * dist);
            }
            if dist < 1e-6 && self.speed < 1e-5 {
                self.phase = self.park_at;
                self.speed = 0.0;
                self.parked = true;
                return;
            }
        } else if self.speed != self.target {
            let c = if self.target > self.speed {
                self.up
            } else {
                self.down
            };
            // The lag, until its step falls under MIN_STEP: then straight on at that rate,
            // landing exactly — a ramp ends in finite time (seconds), as a rotor's does,
            // where an exponential would creep for minutes.
            let d = self.target - self.speed;
            let step = d * c;
            if d.abs() <= MIN_STEP {
                self.speed = self.target;
            } else if step.abs() < MIN_STEP {
                self.speed += MIN_STEP.copysign(d);
            } else {
                self.speed += step;
            }
        }
        self.phase = self.phase.wrapping_add(self.inc(sr));
    }

    /// `n` samples at a steady speed, exactly as `n` steps.
    fn skip(&mut self, n: u64, sr: f64) {
        if !(self.braking && self.parked) {
            self.phase = self.phase.wrapping_add(self.inc(sr).wrapping_mul(n));
        }
    }
}

/// The drive's curve: a 12AX7 stage (100k plate, 1.5k cathode bypassed, 250 V), its plate's
/// swing against its grid's, normalised to a slope of 1 at rest; tabulated at `TABLE` points
/// over ±`DRIVE_SPAN` V (bisection on Koren's plate current; off the audio thread).
fn drive_table() -> Box<[f32; TABLE]> {
    let t = crate::arca::tube::AX7;
    let (b, rp) = (250.0f64, 100e3f64);
    // The cathode's bias, at rest: Vk = Ip·Rk.
    let plate = |vgk: f64| {
        let (mut lo, mut hi) = (0.0f64, b / rp);
        for _ in 0..80 {
            let i = 0.5 * (lo + hi);
            let (_, _, ip, _, _) = t.currents(vgk, b - i * rp);
            if ip > i {
                lo = i;
            } else {
                hi = i;
            }
        }
        b - 0.5 * (lo + hi) * rp
    };
    let (mut lo, mut hi) = (0.0f64, 5.0f64);
    for _ in 0..80 {
        let vk = 0.5 * (lo + hi);
        let ip = (b - plate(-vk)) / rp;
        if ip * 1.5e3 > vk {
            lo = vk;
        } else {
            hi = vk;
        }
    }
    let vk = 0.5 * (lo + hi);
    let p0 = plate(-vk);
    let slope = (plate(-vk - 1e-3) - plate(-vk + 1e-3)) / 2e-3;
    let mut out = Box::new([0.0f32; TABLE]);
    for (k, o) in out.iter_mut().enumerate() {
        let u = -DRIVE_SPAN + 2.0 * DRIVE_SPAN * k as f64 / (TABLE - 1) as f64;
        *o = ((p0 - plate(u - vk)) / slope) as f32;
    }
    out
}

#[derive(Clone)]
pub struct Rota {
    sr: f32,
    set: RotaSettings,
    rot: Box<Rotary>,
    pub horn: Spin,
    pub drum: Spin,
    table: Box<[f32; TABLE]>,
    os: os::Oversampler,
    mix: f32,
    mix_to: f32,
    out: f32,
    out_to: f32,
    quiet: u32,
    since_loud: u32,
    /// The cabinet's lines and filters hold only zeros.
    snapped: bool,
}

impl Rota {
    /// Allocates (off the audio thread).
    pub fn new(sample_rate: f32) -> Rota {
        let mut r = Rota {
            sr: sample_rate,
            set: RotaSettings::default(),
            rot: Box::new(Rotary::new(sample_rate)),
            horn: Spin::new(0),
            drum: Spin::new(1u64 << 62),
            table: drive_table(),
            os: os::Oversampler::new(2, true),
            mix: 0.0,
            mix_to: 0.0,
            out: 1.0,
            out_to: 1.0,
            quiet: u32::MAX / 2,
            since_loud: u32::MAX / 2,
            snapped: true,
        };
        let s = r.set;
        r.set(&s);
        r.reset();
        r
    }

    pub fn settings(&self) -> &RotaSettings {
        &self.set
    }

    pub fn sample_rate(&self) -> f32 {
        self.sr
    }

    fn targets(&self) -> (f64, f64) {
        let s = &self.set;
        match s.speed {
            RotorSpeed::Stop => (0.0, 0.0),
            RotorSpeed::Slow => (s.horn_slow_rpm as f64 / 60.0, s.drum_slow_rpm as f64 / 60.0),
            RotorSpeed::Fast => (s.horn_fast_rpm as f64 / 60.0, s.drum_fast_rpm as f64 / 60.0),
        }
    }

    pub fn set(&mut self, s: &RotaSettings) {
        self.set = *s;
        let rs = RotarySettings {
            speed: s.speed,
            horn_slow_rpm: s.horn_slow_rpm,
            horn_fast_rpm: s.horn_fast_rpm,
            drum_slow_rpm: s.drum_slow_rpm,
            drum_fast_rpm: s.drum_fast_rpm,
            horn_up_s: s.horn_up_s,
            horn_down_s: s.horn_down_s,
            drum_up_s: s.drum_up_s,
            drum_down_s: s.drum_down_s,
            balance_db: s.balance_db.clamp(-24.0, 24.0),
            open: s.open,
            mic_distance_cm: s.distance_cm.clamp(1.0, 300.0),
            mic_spread_deg: s.spread_deg.clamp(0.0, 180.0),
            mix: 1.0,
            ..RotarySettings::default()
        };
        self.rot.set(&rs);
        let sr = self.sr as f64;
        self.horn.up = Spin::lag(sr, s.horn_up_s);
        self.horn.down = Spin::lag(sr, s.horn_down_s);
        self.drum.up = Spin::lag(sr, s.drum_up_s);
        self.drum.down = Spin::lag(sr, s.drum_down_s);
        let (ht, dt) = self.targets();
        self.horn.target = ht;
        self.drum.target = dt;
        let braking = s.speed == RotorSpeed::Stop;
        let park = (rs.brake_deg as f64 / 360.0).rem_euclid(1.0);
        let park_q = (park * TWO64) as u64;
        for r in [&mut self.horn, &mut self.drum] {
            if !braking || r.park_at != park_q {
                r.parked = false;
            }
            r.braking = braking;
            r.park_at = park_q;
        }
        self.mix_to = s.mix.clamp(0.0, 1.0);
        self.out_to = if s.out_db == 0.0 {
            1.0
        } else {
            crate::math::db_to_gain(s.out_db)
        };
    }

    /// The half-moon switch: slow to fast or fast to slow (a note does it).
    pub fn toggle(&mut self) {
        let mut s = self.set;
        s.speed = match s.speed {
            RotorSpeed::Fast => RotorSpeed::Slow,
            _ => RotorSpeed::Fast,
        };
        self.set(&s);
    }

    pub fn latency(&self) -> usize {
        0
    }

    /// Back to the start: silence, the rotors at their speeds and home angles.
    pub fn reset(&mut self) {
        self.rot.clear();
        self.os.reset();
        let (ht, dt) = self.targets();
        self.horn.phase = 0;
        self.drum.phase = 1u64 << 62;
        self.horn.speed = ht;
        self.drum.speed = dt;
        for r in [&mut self.horn, &mut self.drum] {
            r.parked = false;
            // Braked: parked already, as a cabinet switched on at Stop is.
            if r.braking {
                r.phase = r.park_at;
                r.speed = 0.0;
                r.parked = true;
            }
        }
        self.mix = self.mix_to;
        self.out = self.out_to;
        self.quiet = u32::MAX / 2;
        self.since_loud = u32::MAX / 2;
        self.snapped = true;
    }

    /// The drive's curve at `u` (cubic Hermite through the table).
    #[inline]
    fn curve(&self, u: f32) -> f32 {
        let span = DRIVE_SPAN as f32;
        let pos = ((u + span) / (2.0 * span) * (TABLE - 1) as f32).clamp(1.0, (TABLE - 3) as f32);
        let i = pos as usize;
        let f = pos - i as f32;
        let t = &self.table;
        let (y0, y1, y2, y3) = (t[i - 1], t[i], t[i + 1], t[i + 2]);
        let c1 = 0.5 * (y2 - y0);
        let c2 = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
        let c3 = 0.5 * (y3 - y0) + 1.5 * (y1 - y2);
        ((c3 * f + c2) * f + c1) * f + y1
    }

    #[inline]
    fn drive(&mut self, x: f32) -> f32 {
        let d = self.set.drive.clamp(0.0, 1.0);
        if d == 0.0 {
            return x;
        }
        let g = 0.25 + 15.75 * d * d;
        let mut v = [0.0f32; 8];
        self.os.up(x * g, &mut v);
        for s in v.iter_mut().take(2) {
            *s = self.curve(*s);
        }
        self.os.down(&mut v) / g.sqrt()
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let n = left.len().min(right.len());
        let sr = self.sr as f64;
        for i in 0..n {
            let (a, b) = (left[i], right[i]);
            self.horn.step(sr);
            self.drum.step(sr);
            if a == 0.0 && b == 0.0 {
                self.quiet = self.quiet.saturating_add(1);
            } else {
                self.quiet = 0;
            }
            if self.mix != self.mix_to {
                self.mix += (self.mix_to - self.mix) * 0.002;
                if (self.mix_to - self.mix).abs() < 1e-6 {
                    self.mix = self.mix_to;
                }
            }
            if self.out != self.out_to {
                self.out += (self.out_to - self.out) * 0.002;
                if (self.out_to - self.out).abs() < 1e-6 {
                    self.out = self.out_to;
                }
            }
            if self.mix == 0.0 {
                // The null: the dry untouched (the out level is the wet's alone).
                continue;
            }
            let (wl, wr) = if self.snapped && self.quiet > 0 {
                (0.0, 0.0)
            } else {
                self.snapped = false;
                let x = self.drive(0.5 * (a + b));
                self.rot.horn.angle = self.horn.turns();
                self.rot.drum.angle = self.drum.turns();
                let (wl, wr) = self.rot.tick_moved(x);
                if wl.abs().max(wr.abs()) >= TAIL_FLOOR {
                    self.since_loud = 0;
                } else {
                    self.since_loud = self.since_loud.saturating_add(1);
                }
                if self.quiet >= SETTLE && self.since_loud >= QUIET_TAIL {
                    self.rot.clear();
                    self.os.reset();
                    self.snapped = true;
                }
                (wl, wr)
            };
            let m = self.mix;
            let (mut ya, mut yb) = if m == 1.0 {
                (wl, wr)
            } else {
                (a * (1.0 - m) + wl * m, b * (1.0 - m) + wr * m)
            };
            if self.out != 1.0 {
                ya *= self.out;
                yb *= self.out;
            }
            left[i] = ya + 0.0;
            right[i] = yb + 0.0;
        }
    }

    pub fn at_rest(&self) -> bool {
        self.rest_blocker().is_none()
    }

    pub fn rest_blocker(&self) -> Option<&'static str> {
        // A turning rotor is a state of its own — its angle is its history — so, as a
        // free-running LFO, it is never at rest: only parked (the brake) is.
        if !(self.horn.parked && self.drum.parked) {
            return Some("the rotors turn");
        }
        if self.mix != self.mix_to || self.out != self.out_to {
            return Some("a level is gliding");
        }
        if self.mix != 0.0 && !self.snapped {
            return Some("the cabinet still sounds");
        }
        None
    }

    /// `frames` of silence at rest: the rotors turn on, in closed form.
    pub fn skip(&mut self, frames: usize) {
        let sr = self.sr as f64;
        self.horn.skip(frames as u64, sr);
        self.drum.skip(frames as u64, sr);
        let f = frames.min(u32::MAX as usize) as u32;
        self.quiet = self.quiet.saturating_add(f);
        self.since_loud = self.since_loud.saturating_add(f);
    }

    /// The rotors now: horn and drum angle (turns) and speed (rpm).
    pub fn rotors(&self) -> [(f64, f64); 2] {
        [
            (self.horn.turns(), self.horn.rpm()),
            (self.drum.turns(), self.drum.rpm()),
        ]
    }
}

#[cfg(test)]
#[path = "rota_tests.rs"]
mod tests;
