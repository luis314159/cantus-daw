//! Reading a clip's take for a conversion (ADR 0086 §5, round 34's review: long files): the
//! frames the clip plays, mixed to mono a block at a time, converted to the model's rate
//! a block at a time and written as they come — an hour's take costs a few seconds of
//! memory, not the hour. The blocks are converted exactly as `resample` converts the
//! whole file: to the bit.

use cantus_engine::samples::resample::{converted_len, resample};
use std::path::Path;

/// Frames read (and converted) at a time: ten seconds at 48 kHz.
pub const BLOCK: usize = 480_000;

/// Where mono blocks go, with their rate.
pub type Blocks<'a> = &'a mut dyn FnMut(&[f32], u32) -> Result<(), String>;

fn stopped() -> String {
    crate::i18n::t("transcribe.stopped")
}

/// Frames `a .. b` of `path` mixed to mono, handed to `each` a block at a time with the
/// file's rate; the rate back. A WAV is read from disk from frame `a` a block at a time (a minute of
/// a two-hour take costs a minute's reading and a block's memory); another format is
/// decoded whole first (the decoders have no seek), then handed on in blocks. `cancel` is
/// asked between blocks.
pub fn read_mono_blocks(
    path: &Path,
    a: u64,
    b: u64,
    cancel: &dyn Fn() -> bool,
    each: Blocks<'_>,
) -> Result<u32, String> {
    if let Ok(mut r) = hound::WavReader::open(path) {
        let spec = r.spec();
        let ch = spec.channels.max(1) as usize;
        let total = r.duration() as u64;
        let (a, b) = (a.min(total), b.min(total));
        r.seek(a as u32).map_err(|e| e.to_string())?;
        let n = (b - a) as usize * ch;
        let rate = spec.sample_rate;
        let scale = 1.0 / (1u64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f32;
        let mut block = Vec::with_capacity(BLOCK.min(n / ch + 1));
        let (mut sum, mut k) = (0.0f32, 0usize);
        let mut push = |v: f32, block: &mut Vec<f32>| -> Result<(), String> {
            sum += v;
            k += 1;
            if k == ch {
                block.push(sum / ch as f32);
                (sum, k) = (0.0, 0);
                if block.len() == BLOCK {
                    if cancel() {
                        return Err(stopped());
                    }
                    each(block, rate)?;
                    block.clear();
                }
            }
            Ok(())
        };
        if spec.sample_format == hound::SampleFormat::Float {
            for s in r.samples::<f32>().take(n) {
                push(s.map_err(|e| e.to_string())?, &mut block)?;
            }
        } else {
            for s in r.samples::<i32>().take(n) {
                push(s.map_err(|e| e.to_string())? as f32 * scale, &mut block)?;
            }
        }
        if cancel() {
            return Err(stopped());
        }
        if !block.is_empty() {
            each(&block, rate)?;
        }
        return Ok(rate);
    }
    let data = cantus_engine::samples::assets::decode_any(path)?;
    let ch = data.channels.max(1) as usize;
    let total = (data.frames.len() / ch) as u64;
    let (a, b) = (a.min(total) as usize, b.min(total) as usize);
    let mut block = Vec::with_capacity(BLOCK.min(b - a));
    for f in a..b {
        block.push(data.frames[f * ch..f * ch + ch].iter().sum::<f32>() / ch as f32);
        if block.len() == BLOCK || f + 1 == b {
            if cancel() {
                return Err(stopped());
            }
            each(&block, data.rate)?;
            block.clear();
        }
    }
    Ok(data.rate)
}

/// Frames `a .. b` of `path`, mixed to mono, in memory, and the file's rate.
pub fn read_mono_range(
    path: &Path,
    a: u64,
    b: u64,
    cancel: &dyn Fn() -> bool,
) -> Result<(Vec<f32>, u32), String> {
    let mut mono = Vec::with_capacity((b.saturating_sub(a) as usize).min(1 << 28));
    let rate = read_mono_blocks(path, a, b, cancel, &mut |x, _| {
        mono.extend_from_slice(x);
        Ok(())
    })?;
    Ok((mono, rate))
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

/// A mono stream converted from `from` to `to` Hz exactly as [`resample`] converts it
/// whole: the stream is cut on input frames that are multiples of `M` (`from / gcd`, where
/// every output phase begins again) and each piece converted with `pad` frames of context
/// either side — more than the kernel's reach — so each output frame sees the frames it
/// would see in the whole file, summed in the same order.
pub struct Resampler {
    from: u32,
    to: u32,
    l: u64,
    m: u64,
    pad: u64,
    step: u64,
    /// The stream's length in input frames (known beforehand: the clip's frames).
    total: u64,
    /// Input frames from `at`.
    buf: Vec<f32>,
    at: u64,
    /// Output is written for every input frame before this one (a multiple of `m`).
    cut: u64,
    written: u64,
}

impl Resampler {
    pub fn new(from: u32, to: u32, total: u64) -> Resampler {
        let (from, to) = (from.max(1), to.max(1));
        let g = gcd(from as u64, to as u64);
        let (l, m) = (to as u64 / g, from as u64 / g);
        // The kernel reaches 32 zero crossings at the narrower band's 95 %: `32 / fc`
        // frames each side. Twice that and a little, rounded up to a multiple of `m`.
        let fc = (to as f64 / from as f64).min(1.0) * 0.95;
        let pad = ((2.0 * 32.0 / fc).ceil() as u64 + 64).div_ceil(m) * m;
        let step = (BLOCK as u64).div_ceil(m).max(1) * m;
        Resampler {
            from,
            to,
            l,
            m,
            pad,
            step,
            total,
            buf: Vec::new(),
            at: 0,
            cut: 0,
            written: 0,
        }
    }

    /// The next input frames; what can be converted already goes to `out`.
    pub fn push(
        &mut self,
        x: &[f32],
        out: &mut dyn FnMut(&[f32]) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.from == self.to {
            self.written += x.len() as u64;
            return out(x);
        }
        self.buf.extend_from_slice(x);
        loop {
            let e = self.cut + self.step;
            let have = self.at + self.buf.len() as u64;
            if e >= self.total || have < (e + self.pad).min(self.total) {
                return Ok(());
            }
            self.convert(e, out)?;
            // Keep the context the next piece needs.
            let keep = self.cut.saturating_sub(self.pad);
            if keep > self.at {
                self.buf.drain(..(keep - self.at) as usize);
                self.at = keep;
            }
        }
    }

    /// The stream ended: the rest goes to `out`. Returns the output's length.
    pub fn finish(
        mut self,
        out: &mut dyn FnMut(&[f32]) -> Result<(), String>,
    ) -> Result<u64, String> {
        if self.from != self.to && self.cut < self.total {
            self.total = self.total.min(self.at + self.buf.len() as u64);
            self.convert(self.total, out)?;
        }
        Ok(self.written)
    }

    /// Output for input frames `cut .. e`, then `cut = e`.
    fn convert(
        &mut self,
        e: u64,
        out: &mut dyn FnMut(&[f32]) -> Result<(), String>,
    ) -> Result<(), String> {
        let s0 = self.cut.saturating_sub(self.pad);
        let end = (e + self.pad).min(self.total);
        let piece = &self.buf[(s0 - self.at) as usize..(end - self.at) as usize];
        let y = resample(piece, 1, self.from, self.to);
        let base = s0 * self.l / self.m;
        let n0 = (self.cut * self.l / self.m - base) as usize;
        let n1 = if e == self.total {
            converted_len(self.total as usize, self.from, self.to) as u64
        } else {
            e * self.l / self.m
        };
        let n1 = (n1 - base) as usize;
        out(&y[n0..n1.min(y.len())])?;
        self.written += (n1.min(y.len()) - n0) as u64;
        self.cut = e;
        Ok(())
    }
}

/// The clip's frames, mono, at `rate`, as a 32-bit float WAV at `out` (a worker's part):
/// read, converted and written a block at a time. `cancel` stops it between blocks;
/// nothing is left half-written.
pub fn write_input(
    src: &super::Source,
    rate: u32,
    out: &Path,
    cancel: &dyn Fn() -> bool,
) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let part = out.with_extension("part");
    let result = (|| {
        let mut w = hound::WavWriter::create(&part, spec).map_err(|e| e.to_string())?;
        let mut write = |y: &[f32]| -> Result<(), String> {
            for v in y {
                w.write_sample(*v).map_err(|e| e.to_string())?;
            }
            Ok(())
        };
        let mut conv: Option<Resampler> = None;
        let (a, b) = src.frames;
        read_mono_blocks(&src.path, a, b, cancel, &mut |x, from| {
            let c = conv.get_or_insert_with(|| Resampler::new(from, rate, b - a));
            c.push(x, &mut write)
        })?;
        if let Some(c) = conv {
            c.finish(&mut write)?;
        }
        if cancel() {
            return Err(stopped());
        }
        w.finalize().map_err(|e| e.to_string())
    })();
    match result {
        Ok(()) => std::fs::rename(&part, out).map_err(|e| e.to_string()),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}
