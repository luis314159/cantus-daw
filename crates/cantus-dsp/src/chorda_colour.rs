//! Chorda's colour (round 42, `chorda-c17`, ADR 0098's sixth amendment): the owner's request —
//! *"Chorda should be able to reach both, by changing piano, adjusting the mics, or with EQ"*.
//! One control, `colour` (−1 … 1; 0 Chorda's own voicing, which this stage then leaves alone
//! to the bit), moves model 2's sound along the measured difference between Chorda and a
//! recorded grand's middle register: at 1 the long-term spectrum of the middle phrases
//! (middle C, the soft chords, the run, the ballad) stands where the Salamander Grand's does,
//! band by band ([`SHAPE`], measured with `tools/chorda-voicing/probes/c17/sig.py`, smoothed to
//! octave-wide bands); at −1 as far the other way (brighter, Chorda's own character pushed).
//! It acts on the board's sound before the lid and the room, as a technician's voicing of the
//! hammers and the board would: the room hears the coloured piano.
//!
//! A peaking biquad per band (RBJ's cookbook), both channels; no allocation, no state but the
//! filters'. Off (not run at all) when the colour is 0.

use crate::math;

/// The bands' centres (Hz) and each one's gain (dB) at a colour of 1.
pub const CENTRES: [f64; BANDS] = [250.0, 500.0, 1_000.0, 2_000.0, 4_000.0];
pub const BANDS: usize = 5;
/// Measured: the Salamander's middle phrases minus Chorda's (candidate 17, `DSP_VERSION`
/// 16's voicing) at −20 LUFS — middle C, the soft chords, the run, the ballad — each octave's
/// three thirds summed, the median over the four (`probes/c17/shape.py`; dB).
pub const SHAPE: [f64; BANDS] = [2.9, -2.6, 0.6, 3.2, 2.6];
/// Each band's width (an octave).
const Q: f64 = std::f64::consts::SQRT_2;

#[derive(Clone, Copy, Debug, Default)]
struct Peak {
    c: [f64; 5],
    z: [[f64; 2]; 2],
}

impl Peak {
    fn new(fs: f64, f: f64, db: f64) -> Peak {
        let a = math::powf(10.0f64, db / 40.0);
        let w = std::f64::consts::TAU * f / fs;
        let (s, c) = (math::sin(w), math::cos(w));
        let alpha = s / (2.0 * Q);
        let a0 = 1.0 + alpha / a;
        Peak {
            c: [
                (1.0 + alpha * a) / a0,
                -2.0 * c / a0,
                (1.0 - alpha * a) / a0,
                -2.0 * c / a0,
                (1.0 - alpha / a) / a0,
            ],
            z: [[0.0; 2]; 2],
        }
    }

    #[inline]
    fn tick(&mut self, ch: usize, x: f64) -> f64 {
        let (c, z) = (&self.c, &mut self.z[ch]);
        let y = c[0] * x + z[0];
        z[0] = c[1] * x - c[3] * y + z[1];
        z[1] = c[2] * x - c[4] * y;
        y
    }
}

/// The colour stage.
#[derive(Clone, Debug)]
pub struct Colour {
    fs: f64,
    amount: f64,
    peaks: [Peak; BANDS],
}

impl Colour {
    pub fn new(fs: f64) -> Colour {
        Colour {
            fs,
            amount: 0.0,
            peaks: [Peak::default(); BANDS],
        }
    }

    /// The colour (−1 … 1). Its filters keep their states (a moved control glides on).
    pub fn set(&mut self, amount: f64) {
        let amount = amount.clamp(-1.0, 1.0);
        if amount == self.amount {
            return;
        }
        self.amount = amount;
        if amount == 0.0 {
            // Off: not run, and nothing of it left to ring (the unit can rest).
            self.reset();
            return;
        }
        for (i, p) in self.peaks.iter_mut().enumerate() {
            let z = p.z;
            *p = Peak::new(self.fs, CENTRES[i], amount * SHAPE[i]);
            p.z = z;
        }
    }

    /// Whether it acts (a colour of 0 does not, and a resting stage holds no state).
    pub fn on(&self) -> bool {
        self.amount != 0.0
    }

    #[inline]
    pub fn tick(&mut self, o: &mut [f64; 2]) {
        for p in self.peaks.iter_mut() {
            o[0] = p.tick(0, o[0]);
            o[1] = p.tick(1, o[1]);
        }
    }

    pub fn at_rest(&self) -> bool {
        self.peaks.iter().all(|p| p.z == [[0.0; 2]; 2])
    }

    pub fn reset(&mut self) {
        for p in self.peaks.iter_mut() {
            p.z = [[0.0; 2]; 2];
        }
    }

    /// States under `floor` set to 0 (at a grid point with nothing driving the stage).
    pub fn flush(&mut self, floor: f64) {
        for z in self.peaks.iter_mut().flat_map(|p| p.z.iter_mut()).flatten() {
            if z.abs() < floor {
                *z = 0.0;
            }
        }
    }

    /// The stage's gain (dB) at `f` Hz, as set (for tests and the editor's curve).
    pub fn gain_db(&self, f: f64) -> f64 {
        let w = std::f64::consts::TAU * f / self.fs;
        let (c1, s1, c2, s2) = (
            math::cos(w),
            math::sin(w),
            math::cos(2.0 * w),
            math::sin(2.0 * w),
        );
        let mut g = 0.0;
        for (i, p) in self.peaks.iter().enumerate() {
            if self.amount * SHAPE[i] == 0.0 {
                continue;
            }
            let c = &p.c;
            let (nr, ni) = (c[0] + c[1] * c1 + c[2] * c2, -c[1] * s1 - c[2] * s2);
            let (dr, di) = (1.0 + c[3] * c1 + c[4] * c2, -c[3] * s1 - c[4] * s2);
            g += 10.0 * math::log10((nr * nr + ni * ni) / (dr * dr + di * di));
        }
        g
    }
}
