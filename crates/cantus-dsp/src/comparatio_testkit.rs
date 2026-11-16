//! Signals and drivers for Comparatio's tests: noise, pink noise made exactly in the
//! frequency domain, the EBU test sines, a high-pass that hears clicks, and a driver that
//! plays the unit against a transport in blocks of any size.

use super::reader::RefAudio;
use super::{Comparatio, ComparatioSettings};
use crate::aequalis::fft::Fft;
use crate::math;
use crate::modfx_clock::Transport;

pub const FS: f32 = 48_000.0;

/// Uniform noise in −1 … 1 (xorshift32).
pub fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

/// A signal of `n` samples (a power of two) whose spectrum has magnitude `mag(f)` at every
/// bin and a random phase: periodic in `n`, so its spectrum is exact. Scaled to `rms`.
pub fn shaped(n: usize, seed: u32, rms: f32, mag: impl Fn(f64) -> f64) -> Vec<f32> {
    let fft = Fft::new(n);
    let mut re = vec![0f32; n];
    let mut im = vec![0f32; n];
    let phases = noise(n / 2, seed);
    for k in 1..n / 2 {
        let a = mag(k as f64 / n as f64 * FS as f64);
        let ph = phases[k] as f64 * std::f64::consts::PI;
        let (c, s) = (math::cos(ph), math::sin(ph));
        re[k] = (a * c) as f32;
        im[k] = (a * s) as f32;
        re[n - k] = (a * c) as f32;
        im[n - k] = -(a * s) as f32;
    }
    fft.run(&mut re, &mut im, true);
    let ms: f64 = re.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / n as f64;
    let g = rms as f64 / ms.sqrt();
    re.iter().map(|v| (*v as f64 * g) as f32).collect()
}

/// Pink noise (power ∝ 1/f): flat in fractional-octave bands.
pub fn pink(n: usize, seed: u32, rms: f32) -> Vec<f32> {
    shaped(n, seed, rms, |f| 1.0 / f.sqrt())
}

/// White noise with an exactly flat spectrum: +3.01 dB per octave in fractional-octave bands.
pub fn white(n: usize, seed: u32, rms: f32) -> Vec<f32> {
    shaped(n, seed, rms, |_| 1.0)
}

/// A sine of `peak_dbfs` peak at `hz`, `phase` radians at sample 0.
pub fn sine(hz: f64, peak_dbfs: f64, secs: f64, phase: f64) -> Vec<f32> {
    let a = math::powf(10f64, peak_dbfs / 20.0);
    let n = (secs * FS as f64).round() as usize;
    (0..n)
        .map(|i| (a * math::sin(std::f64::consts::TAU * hz * i as f64 / FS as f64 + phase)) as f32)
        .collect()
}

/// EBU Tech 3341/3342's segments: a 1 kHz sine, each `(dBFS, seconds)`, one after another.
pub fn segments(list: &[(f64, f64)]) -> Vec<f32> {
    let mut x = Vec::new();
    for &(db, secs) in list {
        x.extend(sine(1_000.0, db, secs, 0.0));
    }
    x
}

/// Raised-cosine fades in and out over `len` samples (a tone switched on abruptly
/// overshoots between its first samples, which a true-peak case is not about).
pub fn faded(mut x: Vec<f32>, len: usize) -> Vec<f32> {
    let n = x.len();
    for i in 0..len.min(n / 2) {
        let g = 0.5 - 0.5 * math::cos(std::f64::consts::PI * i as f64 / len as f64);
        x[i] *= g as f32;
        x[n - 1 - i] *= g as f32;
    }
    x
}

pub fn fnv(bits: impl Iterator<Item = u64>) -> u64 {
    super::tests_shared::fnv(bits)
}

pub fn playing(samples: i64) -> Transport {
    Transport {
        playing: true,
        tempo_bpm: 120.0,
        samples,
    }
}

pub fn stopped(samples: i64) -> Transport {
    Transport {
        playing: false,
        ..playing(samples)
    }
}

/// A fourth-order Butterworth high-pass (two sections), `f64`: what is left of a slow
/// signal above `hz` is its click.
pub struct HighPass {
    s: [([f64; 3], [f64; 2], [f64; 2]); 2],
}

impl HighPass {
    pub fn new(hz: f64) -> Self {
        let k = math::tan(std::f64::consts::PI * hz / FS as f64);
        let mut s = [([0.0; 3], [0.0; 2], [0.0; 2]); 2];
        for (i, q) in [0.541_196_100_146_197, 1.306_562_964_876_376_7].into_iter().enumerate() {
            let norm = 1.0 / (1.0 + k / q + k * k);
            s[i] = (
                [norm, -2.0 * norm, norm],
                [2.0 * (k * k - 1.0) * norm, (1.0 - k / q + k * k) * norm],
                [0.0; 2],
            );
        }
        HighPass { s }
    }

    pub fn tick(&mut self, x: f32) -> f64 {
        let mut v = x as f64;
        for (b, a, z) in &mut self.s {
            let y = b[0] * v + z[0];
            z[0] = b[1] * v - a[0] * y + z[1];
            z[1] = b[2] * v - a[1] * y;
            v = y;
        }
        v
    }
}

/// The reference's channels and id, owned.
pub struct OwnedRef {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub id: u64,
}

impl OwnedRef {
    pub fn audio(&self) -> RefAudio<'_> {
        RefAudio {
            left: &self.left,
            right: &self.right,
            id: self.id,
        }
    }
}

/// Play `mix` through `unit` in blocks of `block`, the song playing from `song0`, the
/// settings changed at the given samples (which must be block boundaries), the transport
/// set at each block. Returns the output.
pub fn drive(
    unit: &mut Comparatio,
    mix: (&[f32], &[f32]),
    reference: Option<&OwnedRef>,
    block: usize,
    song0: i64,
    changes: &[(usize, ComparatioSettings)],
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (mix.0.to_vec(), mix.1.to_vec());
    let n = l.len();
    let mut i = 0;
    while i < n {
        for (at, s) in changes {
            if *at == i {
                unit.set(s);
            }
        }
        let m = block.min(n - i);
        unit.set_transport(&playing(song0 + i as i64));
        unit.process(
            &mut l[i..i + m],
            &mut r[i..i + m],
            reference.map(OwnedRef::audio),
        );
        i += m;
    }
    (l, r)
}

/// The integrated loudness of a stereo stretch, by Terminus's meter.
pub fn integrated(l: &[f32], r: &[f32]) -> f64 {
    let mut m = crate::terminus::loudness::LoudnessMeter::new(FS);
    m.process(l, r);
    m.integrated().unwrap_or(f64::NEG_INFINITY)
}
