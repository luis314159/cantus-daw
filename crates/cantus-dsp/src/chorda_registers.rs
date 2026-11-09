//! Model 2's voicing by register (ADR 0098, third amendment): what a technician does when
//! the bass, the middle and the treble each want their own colour — the hammers of each
//! section needled or hardened apart. The keys' bridge forces are summed into three buses
//! (bass, middle, treble), each key crossfaded between the two nearest by its place, and
//! each bus passes a short chain of peaking filters before the body. The cost is fixed
//! (three buses × [`BANDS`] stereo filters), never per voice, and a bus whose gains are all
//! 0 dB is not filtered: at a flat voicing the sum is the keys' own, to the bit.

use crate::math;

/// The bands' centres (Hz): the body's low-mid, the mids, the presence, the bite.
pub const CENTRES: [f64; BANDS] = [125.0, 500.0, 1_600.0, 4_000.0];
pub const BANDS: usize = 4;
/// The registers' centre keys (MIDI): the bass (A0…B2's middle), the middle (C3…B5's),
/// the treble (C6…C8's). A key between two centres feeds both, linearly.
pub const CENTRE_KEYS: [f64; 3] = [34.0, 65.0, 96.0];
/// Each band's width: about an octave and a third.
const Q: f64 = 1.0;

/// One peaking filter, stereo (RBJ's cookbook, normalised by `a0`).
#[derive(Clone, Copy, Debug, Default)]
struct Peak {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    /// Direct form II transposed, per channel.
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
            b0: (1.0 + alpha * a) / a0,
            b1: -2.0 * c / a0,
            b2: (1.0 - alpha * a) / a0,
            a1: -2.0 * c / a0,
            a2: (1.0 - alpha / a) / a0,
            z: [[0.0; 2]; 2],
        }
    }

    #[inline]
    fn tick(&mut self, ch: usize, x: f64) -> f64 {
        let z = &mut self.z[ch];
        let y = self.b0 * x + z[0];
        z[0] = self.b1 * x - self.a1 * y + z[1];
        z[1] = self.b2 * x - self.a2 * y;
        y
    }
}

/// The three buses' filters and their scratch.
#[derive(Clone, Debug)]
pub struct Registers {
    /// Per register: its bands (a 0 dB band is left out) and whether any band is not flat.
    bands: [[Peak; BANDS]; 3],
    live: [[bool; BANDS]; 3],
    any: bool,
}

impl Registers {
    /// The filters for `gains` (dB, per register and band) at `fs`.
    pub fn new(fs: f64, gains: &[[f64; BANDS]; 3]) -> Registers {
        let mut bands = [[Peak::default(); BANDS]; 3];
        let mut live = [[false; BANDS]; 3];
        for r in 0..3 {
            for b in 0..BANDS {
                live[r][b] = gains[r][b] != 0.0;
                bands[r][b] = Peak::new(fs, CENTRES[b], gains[r][b]);
            }
        }
        let any = live.iter().flatten().any(|x| *x);
        Registers { bands, live, any }
    }

    /// Whether any register is voiced (otherwise the engine sums the keys directly).
    pub fn active(&self) -> bool {
        self.any
    }

    /// Key `m`'s (MIDI) share of each register: at most two non-zero, summing to 1.
    pub fn weights(m: u8) -> [f64; 3] {
        let x = m as f64;
        let [b, mid, t] = CENTRE_KEYS;
        if x <= b {
            [1.0, 0.0, 0.0]
        } else if x < mid {
            let u = (x - b) / (mid - b);
            [1.0 - u, u, 0.0]
        } else if x < t {
            let u = (x - mid) / (t - mid);
            [0.0, 1.0 - u, u]
        } else {
            [0.0, 0.0, 1.0]
        }
    }

    /// Register `r`'s bus, in place: its bands in series.
    #[inline]
    pub fn filter(&mut self, r: usize, l: &mut [f64], rt: &mut [f64]) {
        for b in 0..BANDS {
            if !self.live[r][b] {
                continue;
            }
            let p = &mut self.bands[r][b];
            for x in l.iter_mut() {
                *x = p.tick(0, *x);
            }
            for x in rt.iter_mut() {
                *x = p.tick(1, *x);
            }
        }
    }

    pub fn at_rest(&self) -> bool {
        self.bands.iter().flatten().all(|p| p.z == [[0.0; 2]; 2])
    }

    /// Every filter's state cleared.
    pub fn reset(&mut self) {
        for p in self.bands.iter_mut().flatten() {
            p.z = [[0.0; 2]; 2];
        }
    }

    /// With nothing driving them: states under `floor` flushed to 0 (the engine's grid).
    pub fn flush(&mut self, floor: f64) {
        for p in self.bands.iter_mut().flatten() {
            for z in p.z.iter_mut().flatten() {
                if z.abs() < floor {
                    *z = 0.0;
                }
            }
        }
    }
}
