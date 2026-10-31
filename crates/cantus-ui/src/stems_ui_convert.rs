//! A file converted to another rate a block at a time (review 2 #8): read, converted and
//! written in pieces, so a long recording never sits whole in the interface's process.
//!
//! The conversion is the engine's own (`cantus_engine::samples::resample`, a windowed sinc
//! evaluated at the exact rational positions): each block of output is that function run
//! on just the input it needs — a window starting on a multiple of the rate ratio's
//! denominator, wide enough for every tap — so the blocks give back, sample for sample,
//! what converting the whole file at once gives (the tests hold it to the bit).

use cantus_engine::samples::resample::{converted_len, resample};
use std::collections::VecDeque;
use std::path::Path;

/// The most a WAV file can hold: its sizes are 32-bit.
pub const WAV_LIMIT: u64 = u32::MAX as u64 - 64;

/// Bytes of a 32-bit float WAV of `frames` × `channels`.
pub fn wav_bytes(frames: u64, channels: u64) -> u64 {
    frames * channels * 4 + 44
}

/// Interleaved audio read forward, a window at a time.
pub trait Frames {
    fn channels(&self) -> usize;
    fn frames(&self) -> u64;
    /// Frames `[start, start + n)`, zeros past the end. `start` never goes back past where
    /// an earlier call started.
    fn read(&mut self, start: u64, n: usize, out: &mut Vec<f32>) -> Result<(), String>;
}

/// Audio already in memory.
pub struct Memory<'a> {
    pub data: &'a [f32],
    pub channels: usize,
}

impl Frames for Memory<'_> {
    fn channels(&self) -> usize {
        self.channels
    }
    fn frames(&self) -> u64 {
        (self.data.len() / self.channels.max(1)) as u64
    }
    fn read(&mut self, start: u64, n: usize, out: &mut Vec<f32>) -> Result<(), String> {
        let ch = self.channels;
        out.clear();
        out.resize(n * ch, 0.0);
        let total = self.frames();
        if start < total {
            let have = ((total - start) as usize).min(n);
            let from = start as usize * ch;
            out[..have * ch].copy_from_slice(&self.data[from..from + have * ch]);
        }
        Ok(())
    }
}

/// A WAV file read forward: at most its first two channels, integers scaled to ±1.
pub struct Wav {
    samples: Box<dyn Iterator<Item = Result<f32, String>>>,
    file_channels: usize,
    channels: usize,
    frames: u64,
    /// Frames held, from `held_from`.
    held: VecDeque<f32>,
    held_from: u64,
    read_to: u64,
}

impl Wav {
    pub fn open(path: &Path) -> Result<(Wav, u32), String> {
        let r = hound::WavReader::open(path).map_err(|e| e.to_string())?;
        let spec = r.spec();
        let frames = r.duration() as u64;
        let file_channels = spec.channels.max(1) as usize;
        let samples: Box<dyn Iterator<Item = Result<f32, String>>> = match spec.sample_format {
            hound::SampleFormat::Float => Box::new(
                r.into_samples::<f32>()
                    .map(|s| s.map_err(|e| e.to_string())),
            ),
            hound::SampleFormat::Int => {
                let full = (1i64 << (spec.bits_per_sample.clamp(8, 32) - 1)) as f32;
                Box::new(
                    r.into_samples::<i32>()
                        .map(move |s| s.map(|v| v as f32 / full).map_err(|e| e.to_string())),
                )
            }
        };
        Ok((
            Wav {
                samples,
                file_channels,
                channels: file_channels.min(2),
                frames,
                held: VecDeque::new(),
                held_from: 0,
                read_to: 0,
            },
            spec.sample_rate,
        ))
    }
}

impl Frames for Wav {
    fn channels(&self) -> usize {
        self.channels
    }
    fn frames(&self) -> u64 {
        self.frames
    }
    fn read(&mut self, start: u64, n: usize, out: &mut Vec<f32>) -> Result<(), String> {
        let ch = self.channels;
        // Let go of what comes before `start` (of what has been read).
        let drop = start.min(self.read_to).saturating_sub(self.held_from) as usize;
        self.held.drain(..drop * ch);
        self.held_from += drop as u64;
        let want = (start + n as u64).min(self.frames);
        while self.read_to < want {
            let mut frame = [0f32; 2];
            for c in 0..self.file_channels {
                let v = self.samples.next().ok_or("the file ends early")??;
                if let Some(slot) = frame.get_mut(c) {
                    *slot = v;
                }
            }
            if self.read_to >= start {
                self.held.extend(&frame[..ch]);
            } else {
                self.held_from = self.read_to + 1;
            }
            self.read_to += 1;
        }
        out.clear();
        out.resize(n * ch, 0.0);
        let skip = start.saturating_sub(self.held_from) as usize;
        for (o, v) in out.iter_mut().zip(self.held.iter().skip(skip * ch)) {
            *o = *v;
        }
        Ok(())
    }
}

/// A 32-bit float WAV written a block at a time.
pub struct Out {
    w: hound::WavWriter<std::io::BufWriter<std::fs::File>>,
}

impl Out {
    pub fn create(path: &Path, channels: usize, rate: u32) -> Result<Out, String> {
        let spec = hound::WavSpec {
            channels: channels as u16,
            sample_rate: rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        Ok(Out {
            w: hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?,
        })
    }
    pub fn write(&mut self, data: &[f32]) -> Result<(), String> {
        for v in data {
            self.w.write_sample(*v).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn finish(self) -> Result<(), String> {
        self.w.finalize().map_err(|e| e.to_string())
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// `input` (at `from` Hz) converted to `to` Hz into `out`, exactly `frames_out` frames long
/// (cut, or padded with silence; `None`: the converted length). `cancelled` is asked
/// between blocks.
pub fn convert(
    input: &mut dyn Frames,
    from: u32,
    to: u32,
    frames_out: Option<u64>,
    out: &mut Out,
    cancelled: &dyn Fn() -> bool,
) -> Result<u64, String> {
    let ch = input.channels().max(1);
    let n_in = input.frames();
    let natural = converted_len(n_in as usize, from, to) as u64;
    let total = frames_out.unwrap_or(natural);
    let mut buf = Vec::new();
    if from == to || from == 0 || to == 0 {
        let mut at = 0u64;
        while at < total {
            if cancelled() {
                return Err("cancelled".into());
            }
            let n = (total - at).min(1 << 16) as usize;
            input.read(at, n, &mut buf)?;
            out.write(&buf)?;
            at += n as u64;
        }
        return Ok(total);
    }
    let g = gcd(from as u64, to as u64);
    let (l, m) = (to as u64 / g, from as u64 / g);
    // Every tap of the engine's kernel (32 zero crossings at 95 % of the narrower band),
    // with room, rounded to whole periods of the ratio so positions stay exact.
    let fc = (to as f64 / from as f64).min(1.0) * 0.95;
    let half = (32.0 / fc).ceil() as u64 + 2;
    let pad = half.div_ceil(m) * m;
    // Output blocks of whole periods (about 64 k frames).
    let block = l * (65_536 / l).max(1);
    let mut n0 = 0u64;
    while n0 < natural.min(total) {
        if cancelled() {
            return Err("cancelled".into());
        }
        let n1 = (n0 + block).min(natural).min(total);
        let c = n0 / l * m;
        let span = (n1 - n0).div_ceil(l) * m;
        let cs = c.saturating_sub(pad);
        let ce = (c + span + pad + 1).min(n_in);
        input.read(cs, (ce - cs) as usize, &mut buf)?;
        let got = resample(&buf, ch, from, to);
        let k0 = (n0 - cs / m * l) as usize;
        let k1 = k0 + (n1 - n0) as usize;
        out.write(&got[k0 * ch..k1 * ch])?;
        n0 = n1;
    }
    // Silence to the length asked for.
    let mut at = natural.min(total);
    let zeros = vec![0f32; (1 << 14) * ch];
    while at < total {
        let n = (total - at).min(1 << 14) as usize;
        out.write(&zeros[..n * ch])?;
        at += n as u64;
    }
    Ok(total)
}
