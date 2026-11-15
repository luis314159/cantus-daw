//! Caelum's small parts: a sine table read by an integer phase (the LFOs and the grains'
//! windows — a phase that is a count, so a skipped unit's phases are a processed one's to
//! the bit), a one-pole and a two-pole filter whose states the rest test can see, and the
//! Schroeder all-pass the diffusers are made of.

use crate::math;

/// Entries in the sine table (a power of two; one more is stored for the interpolation).
const TABLE: usize = 1024;
const SHIFT: u32 = 32 - 10;
const FRAC: f32 = 1.0 / (1u32 << SHIFT) as f32;

/// One period of a sine, read by a 32-bit phase (0 … 2³² is one turn).
#[derive(Clone, Debug)]
pub struct SineTable {
    t: Vec<f32>,
}

impl SineTable {
    pub fn new() -> Self {
        let t = (0..=TABLE)
            .map(|i| math::sin(std::f64::consts::TAU * i as f64 / TABLE as f64) as f32)
            .collect();
        SineTable { t }
    }

    /// `sin(2π · phase / 2³²)`, linearly interpolated (the error is under 5·10⁻⁶).
    #[inline]
    pub fn sin(&self, phase: u32) -> f32 {
        let i = (phase >> SHIFT) as usize;
        let f = (phase & ((1 << SHIFT) - 1)) as f32 * FRAC;
        let a = self.t[i];
        a + (self.t[i + 1] - a) * f
    }

    /// `cos(2π · phase / 2³²)`.
    #[inline]
    pub fn cos(&self, phase: u32) -> f32 {
        self.sin(phase.wrapping_add(1 << 30))
    }
}

impl Default for SineTable {
    fn default() -> Self {
        Self::new()
    }
}

/// A phase increment for `hz` at `sample_rate`: 2³² is one turn, wrapping (a negative
/// frequency counts down).
pub fn phase_inc(hz: f64, sample_rate: f32) -> u32 {
    let turns = hz / sample_rate as f64;
    let v = (turns - turns.floor()) * 4_294_967_296.0;
    (v as u64 & 0xFFFF_FFFF) as u32
}

/// The TPT one-pole's coefficient for a corner at `hz`.
pub fn one_pole_g(hz: f32, sample_rate: f32) -> f32 {
    let fc = hz.clamp(1.0, 0.45 * sample_rate);
    let g = math::tan(std::f32::consts::PI * fc / sample_rate);
    g / (1.0 + g)
}

/// A one-pole low-pass, trapezoidal (Zavalishin): exact zero at Nyquist, unity at DC.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct OnePole {
    pub s: f32,
}

impl OnePole {
    /// One sample through the low-pass with coefficient `g` ([`one_pole_g`]).
    #[inline]
    pub fn lp(&mut self, x: f32, g: f32) -> f32 {
        let v = (x - self.s) * g;
        let y = v + self.s;
        let s = y + v;
        self.s = if s.abs() < 1e-20 { 0.0 } else { s };
        y
    }

    pub fn clear(&mut self) {
        self.s = 0.0;
    }

    pub fn is_zero(&self) -> bool {
        self.s.to_bits() == 0
    }
}

/// A two-pole state-variable filter, trapezoidal (Simper), Butterworth Q: the low-pass and
/// the high-pass outputs of one state.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Svf2 {
    pub s1: f32,
    pub s2: f32,
}

/// [`Svf2`]'s coefficients for a corner.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SvfCoef {
    a1: f32,
    a2: f32,
    a3: f32,
    k: f32,
    g: f32,
}

impl SvfCoef {
    pub fn new(hz: f32, sample_rate: f32) -> Self {
        let fc = hz.clamp(5.0, 0.45 * sample_rate);
        let g = math::tan(std::f32::consts::PI * fc / sample_rate);
        let k = std::f32::consts::SQRT_2;
        let a1 = 1.0 / (1.0 + g * (g + k));
        SvfCoef {
            a1,
            a2: g * a1,
            a3: g * g * a1,
            k,
            g,
        }
    }
}

impl Svf2 {
    /// One sample: `(low, high)`.
    #[inline]
    pub fn run(&mut self, x: f32, c: &SvfCoef) -> (f32, f32) {
        let v3 = x - self.s2;
        let v1 = c.a1 * self.s1 + c.a2 * v3;
        let v2 = self.s2 + c.a2 * self.s1 + c.a3 * v3;
        let s1 = 2.0 * v1 - self.s1;
        let s2 = 2.0 * v2 - self.s2;
        self.s1 = if s1.abs() < 1e-20 { 0.0 } else { s1 };
        self.s2 = if s2.abs() < 1e-20 { 0.0 } else { s2 };
        let _ = c.g;
        (v2, x - c.k * v1 - v2)
    }

    pub fn clear(&mut self) {
        self.s1 = 0.0;
        self.s2 = 0.0;
    }

    pub fn is_zero(&self) -> bool {
        self.s1.to_bits() == 0 && self.s2.to_bits() == 0
    }
}

/// A Schroeder all-pass of a fixed length, whose delayed term may be read a little later
/// than the length (by `extra` samples, linearly interpolated): modulated, it smears the
/// diffusion's own pattern. Lossless at `extra` = 0; outside a feedback loop the small loss
/// of the interpolation does not matter.
#[derive(Clone, Debug)]
pub struct Allpass {
    pub buf: Vec<f32>,
    len: usize,
    w: usize,
}

impl Allpass {
    /// `len` samples, with room for `room` more of modulation.
    pub fn new(len: usize, room: usize) -> Self {
        Allpass {
            buf: vec![0.0; len.max(1) + room + 2],
            len: len.max(1),
            w: 0,
        }
    }

    /// One sample. While `drain`, +0.0 is written (the caller knows everything here is
    /// under −120 dB and nothing comes in).
    #[inline]
    pub fn tick(&mut self, x: f32, g: f32, extra: f32, drain: bool) -> f32 {
        let size = self.buf.len();
        let d = if extra > 0.0 {
            let e = extra.min((size - self.len - 2) as f32);
            let n = e as usize;
            let f = e - n as f32;
            let i0 = (self.w + size - self.len - n) % size;
            let i1 = (i0 + size - 1) % size;
            let a = self.buf[i0];
            a + (self.buf[i1] - a) * f
        } else {
            self.buf[(self.w + size - self.len) % size]
        };
        let v = x + g * d;
        let v = if v.abs() < 1e-20 || drain { 0.0 } else { v };
        self.buf[self.w] = v;
        self.w += 1;
        if self.w == size {
            self.w = 0;
        }
        d - g * v
    }

    /// Write position moves as `n` ticks would move it (the buffer holds zeros).
    pub fn skip(&mut self, n: usize) {
        self.w = (self.w + n) % self.buf.len();
    }

    pub fn clear(&mut self) {
        self.buf.iter_mut().for_each(|x| *x = 0.0);
    }

    /// The write position back to the start (as new).
    pub fn rewind(&mut self) {
        self.w = 0;
    }

    pub fn is_zero(&self) -> bool {
        crate::rest::all_zero(&self.buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_a_sine() {
        let t = SineTable::new();
        let mut worst = 0.0f32;
        for k in 0..10_000u32 {
            let p = k.wrapping_mul(429_497);
            let want = math::sin(std::f64::consts::TAU * p as f64 / 4_294_967_296.0) as f32;
            worst = worst.max((t.sin(p) - want).abs());
        }
        assert!(worst < 5e-6, "worst {worst}");
        assert!((t.cos(0) - 1.0).abs() < 1e-7);
    }

    #[test]
    fn a_phase_increment_is_a_frequency() {
        let inc = phase_inc(1.0, 48_000.0);
        let after = inc.wrapping_mul(48_000);
        // One turn a second: back within 10⁻⁵ of a turn of the start (the increment is
        // whole, so it is off by under one count in 89 478).
        let off = after.min(after.wrapping_neg()) as f64 / 4_294_967_296.0;
        assert!(off < 1e-5, "{off}");
        assert_eq!(phase_inc(-1.0, 48_000.0), inc.wrapping_neg());
    }

    #[test]
    fn the_allpass_is_lossless() {
        let mut a = Allpass::new(113, 0);
        let mut e_in = 0.0f64;
        let mut e_out = 0.0f64;
        for i in 0..20_000 {
            let x = if i < 500 { ((i * 7919) % 97) as f32 / 97.0 - 0.5 } else { 0.0 };
            let y = a.tick(x, 0.7, 0.0, false);
            e_in += (x as f64) * (x as f64);
            e_out += (y as f64) * (y as f64);
        }
        assert!((e_out / e_in - 1.0).abs() < 1e-4, "{}", e_out / e_in);
    }
}
