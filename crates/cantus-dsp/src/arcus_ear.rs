//! The pitch loop's ear (a child of `arcus`; Fides's model-2 bow uses it too): how far the
//! fundamental a voice sounds is from the pitch it should, heard from its own output.
//!
//! A heterodyne at the target: the output times a reference phasor turning at the target's
//! frequency (it follows the vibrato and every glide, step by step), summed over windows of
//! a whole number of the target's periods — at least `MIN_PERIODS` and `MIN_S` seconds —
//! each window weighted by a triangle (its side lobes fall as 1/f², so the harmonics leak
//! into the fundamental's phase well under a hundredth of a cent). The phase of one window's
//! sum against the last one's, over the time between their centres, is the frequency the
//! fundamental is off by: the same quantity the gate's measure takes (a phase's slope), so
//! the loop holds what is heard — the fundamental's phase — and not a proxy for it. Round
//! 42's first loop timed the main slips; while a light bow's motion settles (a second, on
//! the lowest strings) the slips kept their period but the fundamental's phase drifted, and
//! four of the gate's points were a cent off for it.
//!
//! No transcendental runs per sample: the phasor turns by a complex product (renormalised
//! each control step), the triangle is a counter; an `atan2` at each window's end.

use crate::math;

/// A window holds at least this many of the target's periods and this many seconds.
const MIN_PERIODS: f64 = 4.0;
const MIN_S: f64 = 0.02;

#[derive(Clone, Debug, Default)]
pub struct Ear {
    /// The reference phasor and its turn per sample.
    z: (f64, f64),
    rot: (f64, f64),
    /// The target's frequency, cycles per sample, the rate, and the window's length.
    f: f64,
    sr: f64,
    len: u32,
    n: u32,
    acc: (f64, f64),
    /// The last window's phase and length; none since a reset.
    prev: Option<(f64, u32)>,
    /// What the windows ended since the last `take` heard: the summed offsets (cycles per
    /// sample) and how many.
    heard: (f64, u32),
}

impl Ear {
    /// Start over (a note's start, or the bow placed again).
    pub fn reset(&mut self) {
        *self = Ear {
            z: (1.0, 0.0),
            ..Ear::default()
        };
    }

    /// The control step's target, `f` cycles per sample at the rate `sr` (the next window
    /// is sized by it).
    pub fn aim(&mut self, f: f64, sr: f64) {
        self.sr = sr;
        if self.z == (0.0, 0.0) {
            self.z = (1.0, 0.0);
        }
        self.f = f.max(1e-6);
        let (s, c) = math::sin_cos(std::f64::consts::TAU * self.f);
        self.rot = (c, -s);
        let m = (self.z.0 * self.z.0 + self.z.1 * self.z.1).sqrt();
        if m > 0.0 {
            self.z = (self.z.0 / m, self.z.1 / m);
        }
        if self.len == 0 {
            self.len = self.window();
        }
    }

    fn window(&self) -> u32 {
        // A whole number of the target's periods.
        let periods = MIN_PERIODS.max((MIN_S * self.sr * self.f).ceil());
        ((periods / self.f).round() as u32).max(8)
    }

    /// Hear `x` (the voice's output for a slice).
    #[inline]
    pub fn hear(&mut self, x: &[f32]) {
        for v in x {
            self.hear_one(*v);
        }
    }

    /// Hear one sample.
    #[inline]
    pub fn hear_one(&mut self, v: f32) {
        if self.len == 0 {
            return;
        }
        let v = v as f64;
        let half = self.len / 2;
        let w = if self.n < half {
            self.n + 1
        } else {
            self.len - self.n
        } as f64;
        self.acc.0 += w * v * self.z.0;
        self.acc.1 += w * v * self.z.1;
        let (zr, zi) = self.z;
        self.z = (
            zr * self.rot.0 - zi * self.rot.1,
            zr * self.rot.1 + zi * self.rot.0,
        );
        self.n += 1;
        if self.n >= self.len {
            self.close();
        }
    }

    fn close(&mut self) {
        let len = self.len;
        if self.acc != (0.0, 0.0) {
            let phase = math::atan2(self.acc.1, self.acc.0);
            if let Some((last, last_len)) = self.prev {
                let mut d = phase - last;
                while d > std::f64::consts::PI {
                    d -= std::f64::consts::TAU;
                }
                while d < -std::f64::consts::PI {
                    d += std::f64::consts::TAU;
                }
                let apart = 0.5 * (len as f64 + last_len as f64);
                self.heard.0 += d / (std::f64::consts::TAU * apart);
                self.heard.1 += 1;
            }
            self.prev = Some((phase, len));
        } else {
            self.prev = None;
        }
        self.acc = (0.0, 0.0);
        self.n = 0;
        self.len = self.window();
    }

    /// The cents the fundamental sounded from the target over the windows that ended since
    /// the last call, if any did.
    pub fn take(&mut self) -> Option<f64> {
        let (sum, n) = self.heard;
        if n == 0 {
            return None;
        }
        self.heard = (0.0, 0);
        let off = sum / n as f64;
        Some(1200.0 * math::log2((1.0 + off / self.f).max(1e-6)))
    }
}
