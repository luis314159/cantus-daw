//! Synthetic voices for the pitch base's tests, Tonus's tests and demos: a glottal pulse
//! train (Rosenberg's shape) through a cascade of formant resonators, with any f0 curve
//! (vibrato, glides, steps), plus sibilant and breath noise and octave-ambiguous tones.
//! Deterministic (a seeded xorshift; [`crate::math`] only). Not for the audio thread
//! (it allocates).

use crate::math;
use core::f64::consts::PI;

/// A vowel: four formants (Hz) and their bandwidths (Hz).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Vowel {
    pub formants: [(f32, f32); 4],
}

impl Vowel {
    /// /a/ as in "father" (Peterson and Barney's adult male averages).
    pub const A: Vowel = Vowel {
        formants: [
            (730.0, 90.0),
            (1_090.0, 110.0),
            (2_440.0, 170.0),
            (3_400.0, 250.0),
        ],
    };
    /// /i/ as in "beet".
    pub const I: Vowel = Vowel {
        formants: [
            (270.0, 60.0),
            (2_290.0, 100.0),
            (3_010.0, 170.0),
            (3_500.0, 250.0),
        ],
    };
    /// /u/ as in "boot".
    pub const U: Vowel = Vowel {
        formants: [
            (300.0, 60.0),
            (870.0, 90.0),
            (2_240.0, 170.0),
            (3_400.0, 250.0),
        ],
    };
    /// /a/ with the formants of a female voice (about 17 % higher).
    pub const A_FEMALE: Vowel = Vowel {
        formants: [
            (850.0, 100.0),
            (1_220.0, 120.0),
            (2_810.0, 180.0),
            (3_900.0, 260.0),
        ],
    };
}

/// A two-pole resonator.
#[derive(Copy, Clone, Debug, Default)]
struct Resonator {
    a1: f64,
    a2: f64,
    g: f64,
    y1: f64,
    y2: f64,
}

impl Resonator {
    fn new(sr: f64, f: f64, bw: f64) -> Self {
        let r = math::exp(-PI * bw / sr);
        let th = 2.0 * PI * f / sr;
        Resonator {
            a1: 2.0 * r * math::cos(th),
            a2: -r * r,
            g: 1.0 - r,
            y1: 0.0,
            y2: 0.0,
        }
    }

    fn tick(&mut self, x: f64) -> f64 {
        let y = self.g * x + self.a1 * self.y1 + self.a2 * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

/// A deterministic noise source.
#[derive(Clone, Debug)]
pub struct Noise(u64);

impl Noise {
    pub fn new(seed: u64) -> Self {
        Noise(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    /// Uniform in −1 … 1.
    pub fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 / (1u64 << 24) as f32) * 2.0 - 1.0
    }
}

/// Rosenberg's glottal flow at phase `p` (0 … 1 of a period): opening 40 %, closing 16 %.
fn glottal(p: f64) -> f64 {
    const TP: f64 = 0.40;
    const TN: f64 = 0.16;
    if p < TP {
        0.5 * (1.0 - math::cos(PI * p / TP))
    } else if p < TP + TN {
        math::cos(PI * (p - TP) / (2.0 * TN))
    } else {
        0.0
    }
}

/// A sung vowel: `seconds` long, with f0 (Hz) given at each time (seconds), peak-normalised
/// to `peak`. An f0 of 0 or less is silence there (the pulse train stops).
pub fn voice(sr: f32, seconds: f32, vowel: Vowel, peak: f32, f0: impl Fn(f64) -> f32) -> Vec<f32> {
    let sr64 = sr as f64;
    let n = (seconds as f64 * sr64) as usize;
    let mut res: Vec<Resonator> = vowel
        .formants
        .iter()
        .map(|&(f, b)| Resonator::new(sr64, f as f64, b as f64))
        .collect();
    let mut phase = 0.0f64;
    let mut prev = 0.0f64;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let f = f0(i as f64 / sr64) as f64;
        let g = if f > 0.0 {
            phase += f / sr64;
            phase -= phase.floor();
            glottal(phase)
        } else {
            0.0
        };
        // Lip radiation: the derivative of the flow.
        let mut y = g - prev;
        prev = g;
        for r in &mut res {
            y = r.tick(y);
        }
        out.push(y as f32);
    }
    normalise(&mut out, peak);
    out
}

/// Sibilant noise: white noise high-passed at about 4 kHz (a two-pole high-pass, twice).
pub fn sibilant(sr: f32, seconds: f32, peak: f32, seed: u64) -> Vec<f32> {
    let n = (seconds * sr) as usize;
    let mut ns = Noise::new(seed);
    let c = math::exp(-2.0 * core::f32::consts::PI * 4_000.0 / sr);
    let (mut l1, mut l2) = (0.0f32, 0.0f32);
    let mut out: Vec<f32> = (0..n)
        .map(|_| {
            let x = ns.next();
            // High-pass = input − one-pole low-pass, twice.
            l1 = x + (l1 - x) * c;
            let h1 = x - l1;
            l2 = h1 + (l2 - h1) * c;
            h1 - l2
        })
        .collect();
    normalise(&mut out, peak);
    out
}

/// Breath: white noise low-passed at about 1.5 kHz, quiet and aperiodic.
pub fn breath(sr: f32, seconds: f32, peak: f32, seed: u64) -> Vec<f32> {
    let n = (seconds * sr) as usize;
    let mut ns = Noise::new(seed);
    let c = math::exp(-2.0 * core::f32::consts::PI * 1_500.0 / sr);
    let mut l = 0.0f32;
    let mut out: Vec<f32> = (0..n)
        .map(|_| {
            let x = ns.next();
            l = x + (l - x) * c;
            l
        })
        .collect();
    normalise(&mut out, peak);
    out
}

/// An octave-ambiguous tone at `f0`: a weak fundamental under a strong second harmonic
/// and a third (the shape that tempts a tracker an octave up).
pub fn octave_ambiguous(sr: f32, seconds: f32, f0: f32, peak: f32) -> Vec<f32> {
    let n = (seconds * sr) as usize;
    let w = 2.0 * PI * f0 as f64 / sr as f64;
    let mut out: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 * w;
            (0.25 * math::sin(t) + math::sin(2.0 * t + 0.3) + 0.4 * math::sin(3.0 * t + 1.1)) as f32
        })
        .collect();
    normalise(&mut out, peak);
    out
}

/// A vibrato: `centre` Hz moved by `cents` (peak) at `rate` Hz.
pub fn vibrato(centre: f32, cents: f32, rate: f32) -> impl Fn(f64) -> f32 {
    move |t| {
        centre
            * math::exp2(cents / 1200.0 * math::sin(2.0 * core::f32::consts::PI * rate * t as f32))
    }
}

/// A glide from `a` to `b` Hz over `seconds`, then `b`, exponential in frequency.
pub fn glide(a: f32, b: f32, seconds: f32) -> impl Fn(f64) -> f32 {
    move |t| {
        let x = (t as f32 / seconds).clamp(0.0, 1.0);
        a * math::powf(b / a, x)
    }
}

/// Scale to a peak of `peak`.
pub fn normalise(x: &mut [f32], peak: f32) {
    let m = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if m > 0.0 {
        let g = peak / m;
        for v in x {
            *v *= g;
        }
    }
}

/// The fundamental of `x` measured over a stretch, Hz: the zero crossings of its
/// fundamental, isolated by a narrow band-pass around `near` (for checking a shifter's
/// output pitch independently of the tracker).
pub fn measured_f0(x: &[f32], sr: f32, near: f32) -> f32 {
    let sr64 = sr as f64;
    let mut r1 = Resonator::new(sr64, near as f64, near as f64 * 0.25);
    let mut r2 = Resonator::new(sr64, near as f64, near as f64 * 0.25);
    let y: Vec<f64> = x.iter().map(|&v| r2.tick(r1.tick(v as f64))).collect();
    // Skip the filters' settling: the first quarter.
    let start = y.len() / 4;
    let mut first = None;
    let mut last = 0.0;
    let mut count = 0usize;
    for i in start + 1..y.len() {
        if y[i - 1] < 0.0 && y[i] >= 0.0 {
            let t = (i - 1) as f64 + y[i - 1] / (y[i - 1] - y[i]);
            if first.is_none() {
                first = Some(t);
            } else {
                count += 1;
            }
            last = t;
        }
    }
    match first {
        Some(f) if count > 0 => (count as f64 * sr64 / (last - f)) as f32,
        _ => 0.0,
    }
}

/// The amplitude (dB) of `x` at `hz`, by Goertzel over the whole slice with a Hann window.
pub fn amplitude_db(x: &[f32], sr: f32, hz: f32) -> f32 {
    let n = x.len();
    let w = 2.0 * PI * hz as f64 / sr as f64;
    let c = 2.0 * math::cos(w);
    let (mut s1, mut s2) = (0.0f64, 0.0f64);
    let mut wsum = 0.0f64;
    for (i, &v) in x.iter().enumerate() {
        let win = 0.5 - 0.5 * math::cos(2.0 * PI * i as f64 / n as f64);
        wsum += win;
        let s0 = v as f64 * win + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    let power = s1 * s1 + s2 * s2 - c * s1 * s2;
    let amp = 2.0 * power.max(0.0).sqrt() / wsum.max(1e-30);
    (20.0 * math::log10(amp.max(1e-12))) as f32
}

/// The spectral envelope distance (dB RMS) between a reference harmonic voice at `f0_ref`
/// and a shifted one at `f0_out`: the output's harmonics between `lo` and `hi` Hz compared
/// with the reference's envelope there (interpolated between its harmonics, in dB), both
/// normalised to their mean level. 0: the same envelope.
pub fn envelope_distance(
    reference: &[f32],
    f0_ref: f32,
    out: &[f32],
    f0_out: f32,
    sr: f32,
    lo: f32,
    hi: f32,
) -> f32 {
    let harmonics = |x: &[f32], f0: f32| -> Vec<(f32, f32)> {
        (1..)
            .map(|k| k as f32 * f0)
            .take_while(|&f| f < hi * 1.2)
            .map(|f| (f, amplitude_db(x, sr, f)))
            .collect()
    };
    let r = harmonics(reference, f0_ref);
    let o = harmonics(out, f0_out);
    let env = |f: f32| -> f32 {
        let i = r.partition_point(|h| h.0 < f);
        if i == 0 {
            return r[0].1;
        }
        if i >= r.len() {
            return r[r.len() - 1].1;
        }
        let (a, b) = (r[i - 1], r[i]);
        a.1 + (b.1 - a.1) * (f - a.0) / (b.0 - a.0)
    };
    let pairs: Vec<(f32, f32)> = o
        .iter()
        .filter(|h| h.0 >= lo && h.0 <= hi)
        .map(|h| (h.1, env(h.0)))
        .collect();
    let n = pairs.len().max(1) as f32;
    let mo = pairs.iter().map(|p| p.0).sum::<f32>() / n;
    let mr = pairs.iter().map(|p| p.1).sum::<f32>() / n;
    let ms = pairs
        .iter()
        .map(|p| {
            let d = (p.0 - mo) - (p.1 - mr);
            d * d
        })
        .sum::<f32>()
        / n;
    ms.sqrt()
}
