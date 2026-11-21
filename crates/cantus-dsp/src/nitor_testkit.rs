//! Nitor's test kit: a deterministic programme (bass, chords, kick, snare, hats), sines and
//! noise, a driver that cuts a render into blocks and changes settings at given samples,
//! and the measures (loudness, a tone's level, the gain at a frequency).

use super::{Nitor, NitorSettings};
use crate::math;

pub const SR: f32 = 48_000.0;

/// A small, fast, deterministic generator.
#[derive(Clone, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

pub fn sine(hz: f32, amp: f32, n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| amp * math::sin(core::f32::consts::TAU * hz * i as f32 / SR))
        .collect()
}

/// A song-like stereo programme: a bass on G (G1, then C2, D2, G1 a bar each, 120 bpm), a
/// pad's chords, a kick on every beat, a snare on two and four, hats on the eighths. About
/// −16 LUFS at `gain` 1.
pub fn programme(seconds: f32, gain: f32, seed: u64) -> (Vec<f32>, Vec<f32>) {
    let n = (seconds * SR) as usize;
    let beat = (0.5 * SR) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let mut rng = Rng(seed | 1);
    let midi_hz = |m: f32| 440.0 * math::exp2((m - 69.0) / 12.0);
    let roots = [31.0f32, 36.0, 38.0, 31.0];
    let mut ph = [0.0f32; 4];
    for i in 0..n {
        let bar = (i / (4 * beat)) % 4;
        let in_beat = (i % beat) as f32 / SR;
        let t_eighth = (i % (beat / 2)) as f32 / SR;
        let root = roots[bar];
        // The bass: a rounded saw (three partials), plucked each beat.
        let f = midi_hz(root);
        ph[0] = (ph[0] + f / SR).fract();
        let env_b = math::exp(-in_beat * 3.0);
        let p = core::f32::consts::TAU * ph[0];
        let bass =
            0.22 * env_b * (math::sin(p) + 0.45 * math::sin(2.0 * p) + 0.2 * math::sin(3.0 * p));
        // The pad: the chord's third and fifth an octave and two up, soft, wide.
        let mut pad_l = 0.0;
        let mut pad_r = 0.0;
        for (k, iv) in [24.0f32, 28.0, 31.0].iter().enumerate() {
            let fk = midi_hz(root + iv);
            ph[1 + k] = (ph[1 + k] + fk / SR).fract();
            let v = math::sin(core::f32::consts::TAU * ph[1 + k]);
            let w = 0.035 * (v + 0.3 * math::sin(2.0 * core::f32::consts::TAU * ph[1 + k]));
            if k % 2 == 0 {
                pad_l += w;
                pad_r += 0.7 * w;
            } else {
                pad_l += 0.7 * w;
                pad_r += w;
            }
        }
        // The kick: a falling sine, every beat.
        let kf = 50.0 + 120.0 * math::exp(-in_beat * 40.0);
        let kick =
            0.5 * math::exp(-in_beat * 9.0) * math::sin(core::f32::consts::TAU * kf * in_beat);
        // The snare on two and four; the hats on the eighths.
        let beat_no = (i / beat) % 4;
        let nz = rng.next();
        let snare = if beat_no % 2 == 1 {
            0.18 * math::exp(-in_beat * 22.0) * nz
        } else {
            0.0
        };
        let hat = 0.05 * math::exp(-t_eighth * 90.0) * (nz - rng.next()) * 0.5;
        let mono = bass + kick + snare;
        l[i] = gain * (mono + pad_l + hat);
        r[i] = gain * (mono + pad_r - hat * 0.6);
    }
    (l, r)
}

/// A render: settings changed at given samples, blocks cut by `blocks` (cycled).
pub fn render(
    u: &mut Nitor,
    input: &(Vec<f32>, Vec<f32>),
    blocks: &[usize],
    changes: &[(usize, NitorSettings)],
) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (input.0.clone(), input.1.clone());
    let n = l.len();
    let mut i = 0;
    let mut b = 0;
    let mut c = 0;
    while i < n {
        while c < changes.len() && changes[c].0 <= i {
            u.set(&changes[c].1);
            c += 1;
        }
        let mut len = blocks[b % blocks.len()].max(1).min(n - i);
        if c < changes.len() {
            len = len.min(changes[c].0 - i);
        }
        u.process(&mut l[i..i + len], &mut r[i..i + len]);
        i += len;
        b += 1;
    }
    (l, r)
}

/// Integrated-ish loudness: the K-weighted mean square over everything after `from`, LUFS.
pub fn loudness(x: &(Vec<f32>, Vec<f32>), from: usize) -> f64 {
    let mut k = crate::comprimo::loudness::KWeighting::new(SR);
    let mut sum = 0.0;
    let mut n = 0usize;
    for i in 0..x.0.len() {
        let e = k.tick(x.0[i], x.1[i]);
        if i >= from {
            sum += e;
            n += 1;
        }
    }
    -0.691 + 10.0 * math::log10((sum / n.max(1) as f64).max(1e-30))
}

/// A tone's amplitude in `x[from..]` (a single-bin DFT, Hann-windowed).
pub fn tone(x: &[f32], hz: f32, from: usize) -> f64 {
    let s = &x[from..];
    let n = s.len();
    let (mut re, mut im, mut w_sum) = (0.0f64, 0.0f64, 0.0f64);
    for (i, v) in s.iter().enumerate() {
        let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        let ph = std::f64::consts::TAU * f64::from(hz) * i as f64 / f64::from(SR);
        re += f64::from(*v) * w * math::cos(ph);
        im += f64::from(*v) * w * math::sin(ph);
        w_sum += w;
    }
    2.0 * (re * re + im * im).sqrt() / w_sum
}

pub fn db(x: f64) -> f64 {
    20.0 * math::log10(x.max(1e-12))
}

pub fn rms(x: &[f32]) -> f64 {
    (x.iter().map(|v| f64::from(*v) * f64::from(*v)).sum::<f64>() / x.len().max(1) as f64).sqrt()
}

/// The first sample where two renders' bits differ.
pub fn first_difference(a: &(Vec<f32>, Vec<f32>), b: &(Vec<f32>, Vec<f32>)) -> Option<usize> {
    (0..a.0.len())
        .find(|&i| a.0[i].to_bits() != b.0[i].to_bits() || a.1[i].to_bits() != b.1[i].to_bits())
}

/// Every part on, the Loud character at full amount: the heaviest settings.
pub fn heavy() -> NitorSettings {
    NitorSettings {
        amount: 1.0,
        character: super::Character::Loud,
        sub: 0.6,
        harm: 0.5,
        tight: 0.6,
        lift: 0.7,
        xover_hz: 120.0,
        tonic: 7,
        excite: [0.5, 0.6, 0.7],
        color: 0.3,
        ..NitorSettings::default()
    }
}
