//! Aula's impulse response before it is partitioned (ADR 0112): read at any rate, made the
//! engine's, and shaped by the controls that change the response itself — stretch, length,
//! the gate, decay, damping, the cuts, reverse and attack. All off the audio thread, all
//! deterministic (`cantus_dsp::math`, no clock, no randomness).
//!
//! The order is the room's, then the listener's: the response is brought to the engine's
//! rate and stretched in one resampling; the gate takes the recording's noise floor away;
//! length trims; decay shortens the tail; damping darkens it as it goes; the cuts shape its
//! spectrum; then it is reversed (if asked) and faded in by attack. Its loudness is set
//! once, from the response as stretched, so the controls after that change the level the
//! way the room would (a shorter tail is quieter).

use crate::aula_fft::Fft;
use crate::aula_kernel::Format;
use crate::math;

/// The longest response Aula holds, seconds (at the engine's rate).
pub const MAX_SECONDS: f32 = 20.0;

/// An impulse response at a rate: one channel per path of its format (left→left,
/// left→right, right→left, right→right for true stereo).
#[derive(Clone, Debug, PartialEq)]
pub struct Ir {
    pub rate: f32,
    pub format: Format,
    pub channels: Vec<Vec<f32>>,
}

impl Ir {
    /// A response from a file's channels: 1 mono, 2 stereo, 4 true stereo; three, or more
    /// than four, keep their first two (stereo).
    pub fn from_channels(rate: f32, mut channels: Vec<Vec<f32>>) -> Ir {
        let format = Format::of_channels(channels.len());
        channels.truncate(format.paths());
        if channels.is_empty() {
            channels.push(Vec::new());
        }
        Ir {
            rate,
            format,
            channels,
        }
    }

    /// Its length in samples (the longest channel).
    pub fn len(&self) -> usize {
        self.channels.iter().map(|c| c.len()).max().unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn seconds(&self) -> f32 {
        self.len() as f32 / self.rate.max(1.0)
    }
}

/// The controls that change the response itself. [`Shape::default`] leaves it as recorded.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Shape {
    /// Time stretch: 0.5 … 2 (a larger room, lower; 1 as recorded).
    pub stretch: f32,
    /// The part kept: 0.02 … 1 (1 the whole).
    pub length: f32,
    pub reverse: bool,
    /// The fade in, ms (0 none).
    pub attack_ms: f32,
    /// The tail's decay: 0.05 … 1 (1 as recorded; ½ reaches −60 dB at the end).
    pub decay: f32,
    /// Where the response falls this far under its peak it ends, dB (−120 off).
    pub gate_db: f32,
    /// Low cut, Hz (10 off), 12 dB per octave.
    pub low_hz: f32,
    /// High cut, Hz (22 000 off), 12 dB per octave.
    pub high_hz: f32,
    /// Damping: the highs die away faster, 0 … 1.
    pub damp: f32,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            stretch: 1.0,
            length: 1.0,
            reverse: false,
            attack_ms: 0.0,
            decay: 1.0,
            gate_db: GATE_OFF,
            low_hz: LOW_OFF,
            high_hz: HIGH_OFF,
            damp: 0.0,
        }
    }
}

pub const GATE_OFF: f32 = -120.0;
pub const LOW_OFF: f32 = 10.0;
pub const HIGH_OFF: f32 = 22_000.0;

/// `src` at `rate`, shaped by `s`, its loudness set (unless `normalize` is off: the tests'
/// raw responses). Allocates.
pub fn shape(src: &Ir, rate: f32, s: &Shape, normalize: bool) -> Ir {
    let ratio = (rate / src.rate.max(1.0)) as f64 * s.stretch.clamp(0.5, 2.0) as f64;
    let max = (MAX_SECONDS * rate) as usize;
    let mut ch: Vec<Vec<f32>> = src
        .channels
        .iter()
        .map(|c| resample(c, ratio, max))
        .collect();
    let gain = if normalize {
        loudness_gain(src.format, &ch)
    } else {
        1.0
    };
    gate(&mut ch, rate, s.gate_db);
    let keep = ((longest(&ch) as f64 * s.length.clamp(0.0, 1.0) as f64).round() as usize).max(1);
    if s.length < 1.0 {
        for c in &mut ch {
            c.truncate(keep);
        }
        let fade = ((keep / 10).min((0.05 * rate) as usize)).max(1);
        fade_out(&mut ch, fade);
    }
    if s.decay < 1.0 {
        let len = longest(&ch).max(1) as f32;
        let k = 6.907_755 * (1.0 / s.decay.max(0.01) - 1.0) / len;
        for c in &mut ch {
            for (i, v) in c.iter_mut().enumerate() {
                *v *= math::exp(-k * i as f32);
            }
        }
    }
    if s.damp > 0.0 {
        damp(&mut ch, rate, s.damp.min(1.0));
    }
    if s.low_hz > LOW_OFF * 1.001 {
        for c in &mut ch {
            biquad(c, rate, s.low_hz, true);
        }
    }
    if s.high_hz < HIGH_OFF * 0.999 && s.high_hz < 0.45 * rate {
        for c in &mut ch {
            biquad(c, rate, s.high_hz, false);
        }
    }
    if s.reverse {
        let len = longest(&ch);
        for c in &mut ch {
            c.resize(len, 0.0);
            c.reverse();
        }
    }
    if s.attack_ms > 0.0 {
        let a = ((s.attack_ms / 1000.0 * rate) as usize).max(1);
        for c in &mut ch {
            for (i, v) in c.iter_mut().take(a).enumerate() {
                let x = math::sin(std::f32::consts::FRAC_PI_2 * i as f32 / a as f32);
                *v *= x * x;
            }
        }
    }
    if gain != 1.0 {
        for c in &mut ch {
            for v in c.iter_mut() {
                *v *= gain;
            }
        }
    }
    Ir {
        rate,
        format: src.format,
        channels: ch,
    }
}

fn longest(ch: &[Vec<f32>]) -> usize {
    ch.iter().map(|c| c.len()).max().unwrap_or(0)
}

/// The gain that gives the loudest output unit energy for a centred source (white noise in
/// comes out as loud as it went in).
pub fn loudness_gain(format: Format, ch: &[Vec<f32>]) -> f32 {
    let energy = |a: &[f32]| a.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();
    let sum = |a: &[f32], b: &[f32]| {
        let n = a.len().max(b.len());
        (0..n)
            .map(|i| {
                let v = a.get(i).copied().unwrap_or(0.0) as f64
                    + b.get(i).copied().unwrap_or(0.0) as f64;
                v * v
            })
            .sum::<f64>()
            / 2.0
    };
    let e = match format {
        Format::Mono => energy(&ch[0]),
        Format::Stereo => energy(&ch[0]).max(energy(&ch[1])),
        Format::TrueStereo => sum(&ch[0], &ch[2]).max(sum(&ch[1], &ch[3])),
    };
    if e > 1e-24 {
        (1.0 / e.sqrt()) as f32
    } else {
        1.0
    }
}

/// Zero crossings each side of the resampling kernel, and table points per crossing.
const ZC: usize = 16;
const PER_ZC: usize = 256;

fn sinc_table() -> Vec<f32> {
    (0..=ZC * PER_ZC)
        .map(|i| {
            let u = i as f64 / PER_ZC as f64;
            let sinc = if i == 0 {
                1.0
            } else {
                let x = std::f64::consts::PI * u;
                math::sin(x) / x
            };
            // Blackman over ±ZC.
            let w = u / ZC as f64;
            let win = 0.42
                + 0.5 * math::cos(std::f64::consts::PI * w)
                + 0.08 * math::cos(2.0 * std::f64::consts::PI * w);
            (sinc * win) as f32
        })
        .collect()
}

/// `x` resampled by `ratio` (output samples per input sample) with a windowed sinc whose
/// cut-off follows the lower rate; at most `max` samples. A ratio of exactly 1 copies.
pub fn resample(x: &[f32], ratio: f64, max: usize) -> Vec<f32> {
    if ratio == 1.0 {
        return x[..x.len().min(max)].to_vec();
    }
    let table = sinc_table();
    let fc = ratio.min(1.0);
    let reach = ZC as f64 / fc;
    let out = ((x.len() as f64 * ratio).round() as usize).min(max);
    let mut y = Vec::with_capacity(out);
    for m in 0..out {
        let pos = m as f64 / ratio;
        let lo = (pos - reach).ceil().max(0.0) as usize;
        let hi = ((pos + reach).floor() as usize).min(x.len().saturating_sub(1));
        let mut acc = 0.0f64;
        for (i, xi) in x.iter().enumerate().take(hi + 1).skip(lo) {
            let u = (pos - i as f64).abs() * fc * PER_ZC as f64;
            let k = u as usize;
            if k >= ZC * PER_ZC {
                continue;
            }
            let f = (u - k as f64) as f32;
            let w = table[k] + (table[k + 1] - table[k]) * f;
            acc += *xi as f64 * w as f64;
        }
        y.push((acc * fc) as f32);
    }
    y
}

/// Cut where the response (the loudest channel, in 10 ms windows) falls `gate_db` under its
/// peak and never rises again; a 10 ms fade.
fn gate(ch: &mut [Vec<f32>], rate: f32, gate_db: f32) {
    if gate_db <= GATE_OFF {
        return;
    }
    let len = longest(ch);
    let win = ((0.01 * rate) as usize).max(1);
    let windows = len.div_ceil(win);
    let peak_of = |w: usize| {
        ch.iter()
            .flat_map(|c| c.iter().skip(w * win).take(win))
            .fold(0.0f32, |m, v| m.max(v.abs()))
    };
    let peaks: Vec<f32> = (0..windows).map(peak_of).collect();
    let top = peaks.iter().fold(0.0f32, |m, v| m.max(*v));
    let floor = top * math::db_to_gain(gate_db);
    let last = peaks.iter().rposition(|p| *p > floor).unwrap_or(0);
    let end = ((last + 1) * win).min(len);
    for c in ch.iter_mut() {
        c.truncate(end);
    }
    fade_out(ch, win);
}

fn fade_out(ch: &mut [Vec<f32>], fade: usize) {
    for c in ch.iter_mut() {
        let n = c.len();
        let f = fade.min(n);
        for i in 0..f {
            let x = math::cos(std::f32::consts::FRAC_PI_2 * (i + 1) as f32 / f as f32);
            c[n - f + i] *= x * x;
        }
    }
}

/// Damping in short frames (512, hop 256, √Hann both ways, which sums to one): each
/// frame's highs lowered by up to `48 · damp` dB at 12 kHz by the response's end, in
/// proportion to frequency and time.
fn damp(ch: &mut [Vec<f32>], rate: f32, amount: f32) {
    const N: usize = 512;
    const HOP: usize = N / 2;
    let fft = Fft::new(N);
    let win: Vec<f32> = (0..N)
        .map(|i| math::sin(std::f32::consts::PI * i as f32 / N as f32))
        .collect();
    let len = longest(ch);
    let total = len.max(1) as f32;
    for pair in (0..ch.len()).step_by(2) {
        let second = pair + 1 < ch.len();
        let get = |c: &[f32], i: isize| {
            if i >= 0 {
                c.get(i as usize).copied().unwrap_or(0.0)
            } else {
                0.0
            }
        };
        let mut out_a = vec![0.0f32; len + N];
        let mut out_b = vec![0.0f32; len + N];
        let mut re = vec![0.0f32; N];
        let mut im = vec![0.0f32; N];
        let mut start: isize = -(HOP as isize);
        while start < len as isize {
            for i in 0..N {
                re[i] = get(&ch[pair], start + i as isize) * win[i];
                im[i] = if second {
                    get(&ch[pair + 1], start + i as isize) * win[i]
                } else {
                    0.0
                };
            }
            fft.run(&mut re, &mut im, false);
            let t = ((start + (N / 2) as isize).max(0) as f32 / total).min(1.0);
            for k in 0..=N / 2 {
                let f = k as f32 * rate / N as f32;
                let g = math::db_to_gain(-48.0 * amount * t * (f / 12_000.0));
                re[k] *= g;
                im[k] *= g;
                if k > 0 && k < N / 2 {
                    re[N - k] *= g;
                    im[N - k] *= g;
                }
            }
            fft.run(&mut re, &mut im, true);
            for i in 0..N {
                let at = start + i as isize;
                if at >= 0 && (at as usize) < len {
                    out_a[at as usize] += re[i] / N as f32 * win[i];
                    out_b[at as usize] += im[i] / N as f32 * win[i];
                }
            }
            start += HOP as isize;
        }
        let la = ch[pair].len();
        ch[pair] = out_a[..la].to_vec();
        if second {
            let lb = ch[pair + 1].len();
            ch[pair + 1] = out_b[..lb].to_vec();
        }
    }
}

/// A 12 dB-per-octave Butterworth cut (RBJ), forward over the response.
fn biquad(x: &mut [f32], rate: f32, hz: f32, high_pass: bool) {
    let w = 2.0 * std::f64::consts::PI * hz as f64 / rate as f64;
    let (s, c) = (math::sin(w), math::cos(w));
    let alpha = s / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
    let (b0, b1, b2) = if high_pass {
        ((1.0 + c) / 2.0, -(1.0 + c), (1.0 + c) / 2.0)
    } else {
        ((1.0 - c) / 2.0, 1.0 - c, (1.0 - c) / 2.0)
    };
    let a0 = 1.0 + alpha;
    let (a1, a2) = (-2.0 * c, 1.0 - alpha);
    let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0, 0.0, 0.0);
    for v in x.iter_mut() {
        let x0 = *v as f64;
        let y0 = b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
        x2 = x1;
        x1 = x0;
        y2 = y1;
        y1 = y0;
        *v = y0 as f32;
    }
}

/// The response's outline for its picture: the peak of every channel over `points` equal
/// spans, linear.
pub fn overview(ir: &Ir, points: usize) -> Vec<f32> {
    let len = ir.len();
    if len == 0 || points == 0 {
        return vec![0.0; points];
    }
    (0..points)
        .map(|p| {
            let a = p * len / points;
            let b = ((p + 1) * len / points).max(a + 1).min(len);
            ir.channels
                .iter()
                .flat_map(|c| c.iter().take(b).skip(a))
                .fold(0.0f32, |m, v| m.max(v.abs()))
        })
        .collect()
}
