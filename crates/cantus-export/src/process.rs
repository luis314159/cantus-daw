//! What happens to the rendered samples before they are encoded: the channels, the
//! sample rate (the engine's windowed-sinc converter), loudness and its true-peak
//! ceiling, and the cut to integers with dither.

use crate::settings::{Dither, Loudness};
use cantus_dsp::terminus::loudness::LoudnessMeter;
use cantus_dsp::terminus::{Ditherer, LimSettings, Shaping, Terminus};

/// Interleaved audio at a rate.
#[derive(Clone, Debug, PartialEq)]
pub struct Audio {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl Audio {
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / self.rate.max(1) as f64
    }

    /// The largest sample, linear.
    pub fn peak(&self) -> f32 {
        self.samples.iter().fold(0f32, |m, x| m.max(x.abs()))
    }
}

/// Mono or stereo. Stereo to mono is the sides summed at half each, so a centred sound
/// keeps its level; mono to stereo is the one channel on both sides.
pub fn to_channels(a: Audio, channels: u16) -> Audio {
    match (a.channels, channels) {
        (from, to) if from == to => a,
        (2, 1) => Audio {
            rate: a.rate,
            channels: 1,
            samples: a
                .samples
                .chunks_exact(2)
                .map(|f| 0.5 * (f[0] + f[1]))
                .collect(),
        },
        (1, 2) => Audio {
            rate: a.rate,
            channels: 2,
            samples: a.samples.iter().flat_map(|x| [*x, *x]).collect(),
        },
        // Anything else: the first two channels (the engine renders mono or stereo).
        (from, to) => {
            let from = from.max(1) as usize;
            let frames: Vec<f32> = a
                .samples
                .chunks_exact(from)
                .flat_map(|f| {
                    let l = f[0];
                    let r = *f.get(1).unwrap_or(&l);
                    if to == 1 {
                        vec![0.5 * (l + r)]
                    } else {
                        vec![l, r]
                    }
                })
                .collect();
            Audio {
                rate: a.rate,
                channels: to,
                samples: frames,
            }
        }
    }
}

/// Convert to `to` Hz with the engine's converter (Kaiser-windowed sinc, ≈100 dB of
/// rejection, its band edge at 95 % of the narrower Nyquist). Same rate: untouched.
pub fn resample(a: Audio, to: u32) -> Audio {
    if a.rate == to {
        return a;
    }
    let samples =
        cantus_engine::samples::resample::resample(&a.samples, a.channels as usize, a.rate, to);
    Audio {
        rate: to,
        channels: a.channels,
        samples,
    }
}

/// Integrated loudness (LUFS, `None` when the gates leave nothing — silence or under
/// 400 ms of sound) and the true peak (dBTP).
///
/// A mono file is measured **as dual mono**, as it is heard — the one channel from both
/// speakers — so "−16 LUFS" means the same loudness for a mono podcast and a stereo song
/// (BS.1770's single channel would read 3 dB lower; AES TD1004's −19 LUFS mono is this
/// same level).
pub fn measure(a: &Audio) -> (Option<f64>, f32) {
    let mut m = LoudnessMeter::new(a.rate as f32);
    const CHUNK: usize = 4096;
    let (mut l, mut r) = (vec![0f32; CHUNK], vec![0f32; CHUNK]);
    let ch = a.channels.max(1) as usize;
    for block in a.samples.chunks(CHUNK * ch) {
        let n = block.len() / ch;
        for (i, f) in block.chunks_exact(ch).enumerate() {
            l[i] = f[0];
            r[i] = *f.get(1).unwrap_or(&f[0]);
        }
        m.process(&l[..n], &r[..n]);
    }
    (m.integrated(), m.true_peak_db())
}

/// What loudness normalisation did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Normalised {
    /// As mixed, LUFS.
    pub measured_lufs: Option<f64>,
    pub measured_dbtp: f32,
    /// The gain applied, dB.
    pub gain_db: f32,
    /// The limiter held the peaks at the ceiling.
    pub limited: bool,
    /// As written, LUFS and dBTP.
    pub lufs: Option<f64>,
    pub dbtp: f32,
}

/// The most gain normalisation adds, dB: a near-silent mix is not raised into noise.
pub const MAX_GAIN_DB: f32 = 30.0;

fn scale(samples: &mut [f32], db: f32) {
    let g = cantus_dsp::math::powf(10f32, db / 20.0);
    samples.iter_mut().for_each(|x| *x *= g);
}

/// `a` raised or lowered by `gain_db` and held under `ceiling` dBTP by Terminus (clean
/// style, true-peak detection, its latency taken back so nothing moves).
fn limited(
    a: &Audio,
    gain_db: f32,
    ceiling: f32,
    cancel: &mut dyn FnMut() -> bool,
) -> Option<Audio> {
    let mut t = Terminus::new(a.rate as f32);
    t.set(&LimSettings {
        ceiling_db: ceiling,
        true_peak: true,
        lookahead_ms: 5.0,
        ..LimSettings::default()
    });
    t.reset();
    let lat = t.latency();
    let ch = a.channels.max(1) as usize;
    let frames = a.frames();
    let g = cantus_dsp::math::powf(10f32, gain_db / 20.0);
    let mut out = vec![0f32; a.samples.len()];
    const CHUNK: usize = 4096;
    let (mut l, mut r) = (vec![0f32; CHUNK], vec![0f32; CHUNK]);
    let mut at = 0usize;
    while at < frames + lat {
        if cancel() {
            return None;
        }
        let n = CHUNK.min(frames + lat - at);
        for i in 0..n {
            let f = at + i;
            if f < frames {
                l[i] = a.samples[f * ch] * g;
                r[i] = a.samples[f * ch + ch - 1] * g;
            } else {
                l[i] = 0.0;
                r[i] = 0.0;
            }
        }
        t.process(&mut l[..n], &mut r[..n], None);
        for i in 0..n {
            let Some(f) = (at + i).checked_sub(lat) else {
                continue;
            };
            if f >= frames {
                break;
            }
            if ch == 1 {
                out[f] = l[i];
            } else {
                out[f * ch] = l[i];
                out[f * ch + 1] = r[i];
            }
        }
        at += n;
    }
    Some(Audio {
        rate: a.rate,
        channels: a.channels,
        samples: out,
    })
}

/// Bring `a` to the target loudness under the ceiling. Returns `None` when cancelled.
///
/// The gain is the target minus the measurement. If that would put a true peak over the
/// ceiling, either the limiter holds it there — and since limiting takes a little
/// loudness away, the gain is corrected and the pass repeated until the result is within
/// 0.05 LU (three passes at most, as a rule) — or, with the limiter off, the gain stops
/// where the peak meets the ceiling.
pub fn normalise(
    a: &mut Audio,
    l: &Loudness,
    cancel: &mut dyn FnMut() -> bool,
) -> Option<Normalised> {
    let (measured, tp) = measure(a);
    let mut report = Normalised {
        measured_lufs: measured,
        measured_dbtp: tp,
        lufs: measured,
        dbtp: tp,
        ..Default::default()
    };
    let (Some(target), Some(m)) = (l.target_lufs, measured) else {
        return Some(report);
    };
    let ceiling = l.ceiling_dbtp;
    let mut gain = ((target as f64 - m) as f32).min(MAX_GAIN_DB);
    if tp + gain <= ceiling {
        scale(&mut a.samples, gain);
        let (lufs, dbtp) = measure(a);
        report.gain_db = gain;
        report.lufs = lufs;
        report.dbtp = dbtp;
        return Some(report);
    }
    if !l.limit {
        gain = ceiling - tp;
        scale(&mut a.samples, gain);
        let (lufs, dbtp) = measure(a);
        report.gain_db = gain;
        report.lufs = lufs;
        report.dbtp = dbtp;
        return Some(report);
    }
    let mut best: Option<(f64, Audio, f32, f32)> = None;
    for _ in 0..6 {
        let out = limited(a, gain, ceiling, cancel)?;
        let (lufs, dbtp) = measure(&out);
        let Some(lufs) = lufs else { break };
        let err = target as f64 - lufs;
        let better = best.as_ref().is_none_or(|(e, ..)| err.abs() < e.abs());
        if better {
            best = Some((err, out, dbtp, gain));
        }
        if err.abs() < 0.05 || gain >= MAX_GAIN_DB {
            break;
        }
        gain = (gain + err as f32).min(MAX_GAIN_DB);
    }
    if let Some((err, out, dbtp, used)) = best {
        *a = out;
        report.gain_db = used;
        report.limited = true;
        report.lufs = Some(target as f64 - err);
        report.dbtp = dbtp;
    }
    Some(report)
}

/// Samples cut to `bits`-bit integers (full scale ±2^(bits−1)), with dither: one
/// Terminus `Ditherer` per channel, seeded the same every time so an export is
/// repeatable. Digital silence stays exact zeros.
pub fn quantise(samples: &[f32], channels: u16, bits: u16, dither: Dither) -> Vec<i32> {
    let ch = channels.max(1) as usize;
    let full = (1i64 << (bits - 1)) as f32;
    let (lo, hi) = (
        -(1i64 << (bits - 1)) as i32,
        ((1i64 << (bits - 1)) - 1) as i32,
    );
    let shaping = match dither {
        Dither::Shaped => Some(Shaping::Optimized),
        Dither::Tpdf => Some(Shaping::Flat),
        Dither::Off => None,
    };
    let mut d: Vec<Ditherer> = (0..ch)
        .map(|c| Ditherer::new(0x9e37_79b9 ^ (c as u32).wrapping_mul(0x85eb_ca6b)))
        .collect();
    samples
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let y = match shaping {
                Some(sh) => d[i % ch].process(x, bits as u32, sh),
                None => x,
            };
            ((y * full).round() as i64).clamp(lo as i64, hi as i64) as i32
        })
        .collect()
}
