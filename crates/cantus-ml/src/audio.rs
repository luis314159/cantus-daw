//! The audio a job reads and writes: WAV files, read a segment at a time and written as
//! they are finished, so a ten-minute song costs what a ten-second one does in memory.

use crate::proto::JobError;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

/// A WAV file read on demand: stereo always (mono is doubled, as demucs does).
pub struct Input {
    reader: hound::WavReader<BufReader<File>>,
    pub channels: u16,
    pub sample_rate: u32,
    pub frames: u64,
    float: bool,
    scale: f32,
}

fn input_err(path: &Path, e: impl std::fmt::Display) -> JobError {
    JobError::Input(format!("{}: {e}", path.display()))
}

impl Input {
    pub fn open(path: &Path) -> Result<Input, JobError> {
        let reader = hound::WavReader::open(path).map_err(|e| input_err(path, e))?;
        let spec = reader.spec();
        if !(1..=2).contains(&spec.channels) {
            return Err(input_err(
                path,
                format!("{} channels; mono or stereo only", spec.channels),
            ));
        }
        let float = spec.sample_format == hound::SampleFormat::Float;
        if float && spec.bits_per_sample != 32 {
            return Err(input_err(path, "float samples of other than 32 bits"));
        }
        if !float && !(8..=32).contains(&spec.bits_per_sample) {
            return Err(input_err(path, "an unusual sample size"));
        }
        Ok(Input {
            frames: reader.duration() as u64,
            channels: spec.channels,
            sample_rate: spec.sample_rate,
            float,
            scale: 1.0 / (1u64 << (spec.bits_per_sample.max(1) - 1)) as f32,
            reader,
        })
    }

    /// Frames `start ..` into `left` / `right` (as many as they hold); zeros past the end.
    pub fn read(
        &mut self,
        start: u64,
        left: &mut [f32],
        right: &mut [f32],
    ) -> Result<(), JobError> {
        left.iter_mut().for_each(|v| *v = 0.0);
        right.iter_mut().for_each(|v| *v = 0.0);
        if start >= self.frames {
            return Ok(());
        }
        let n = (left.len() as u64).min(self.frames - start) as usize;
        self.reader
            .seek(start as u32)
            .map_err(|e| JobError::Input(e.to_string()))?;
        let ch = self.channels as usize;
        let mut i = 0usize;
        let total = n * ch;
        let mut put = |i: usize, v: f32| {
            let frame = i / ch;
            if ch == 1 {
                left[frame] = v;
                right[frame] = v;
            } else if i.is_multiple_of(2) {
                left[frame] = v;
            } else {
                right[frame] = v;
            }
        };
        if self.float {
            for s in self.reader.samples::<f32>().take(total) {
                put(i, s.map_err(|e| JobError::Input(e.to_string()))?);
                i += 1;
            }
        } else {
            let scale = self.scale;
            for s in self.reader.samples::<i32>().take(total) {
                put(
                    i,
                    s.map_err(|e| JobError::Input(e.to_string()))? as f32 * scale,
                );
                i += 1;
            }
        }
        if i != total {
            return Err(JobError::Input(
                "the file ends before its header says".into(),
            ));
        }
        Ok(())
    }

    /// The mean and the (unbiased) standard deviation of the mono mix, as `demucs.separate`
    /// normalises a track by them: one pass over the file.
    pub fn mono_stats(&mut self, cancelled: &dyn Fn() -> bool) -> Result<(f64, f64), JobError> {
        const BLOCK: usize = 1 << 16;
        let (mut l, mut r) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
        let (mut sum, mut sq) = (0f64, 0f64);
        let mut at = 0u64;
        while at < self.frames {
            if cancelled() {
                return Ok((0.0, 1.0));
            }
            self.read(at, &mut l, &mut r)?;
            let n = (BLOCK as u64).min(self.frames - at) as usize;
            for i in 0..n {
                // torch: `wav.mean(0)` in float32, then its mean and std.
                let m = ((l[i] + r[i]) / 2.0) as f64;
                sum += m;
                sq += m * m;
            }
            at += n as u64;
        }
        let n = self.frames.max(1) as f64;
        let mean = sum / n;
        let var = if self.frames > 1 {
            ((sq - sum * sum / n) / (n - 1.0)).max(0.0)
        } else {
            0.0
        };
        Ok((mean, var.sqrt()))
    }
}

/// One stereo 32-bit float WAV per source, written in order as frames are final.
pub struct Output {
    writers: Vec<hound::WavWriter<BufWriter<File>>>,
}

impl Output {
    pub fn create(paths: &[std::path::PathBuf], sample_rate: u32) -> Result<Output, JobError> {
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let writers = paths
            .iter()
            .map(|p| {
                hound::WavWriter::create(p, spec)
                    .map_err(|e| JobError::Output(format!("{}: {e}", p.display())))
            })
            .collect::<Result<_, _>>()?;
        Ok(Output { writers })
    }

    /// Append frames to source `s`.
    pub fn write(&mut self, s: usize, left: &[f32], right: &[f32]) -> Result<(), JobError> {
        let w = &mut self.writers[s];
        for (a, b) in left.iter().zip(right) {
            w.write_sample(*a)
                .and_then(|_| w.write_sample(*b))
                .map_err(|e| JobError::Output(e.to_string()))?;
        }
        Ok(())
    }

    pub fn finish(self) -> Result<(), JobError> {
        for w in self.writers {
            w.finalize().map_err(|e| JobError::Output(e.to_string()))?;
        }
        Ok(())
    }
}
