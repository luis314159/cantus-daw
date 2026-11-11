//! Reading an impulse response file for Aula (ADR 0112). WAV of any depth — 8, 16, 24 and
//! 32-bit integer, 32-bit float — with every channel kept: one is mono, two stereo, four
//! true stereo (left→left, left→right, right→left, right→right, the order Waves, Altiverb
//! and REAPER's ReaVerb write). AIFF, FLAC and Ogg go through Exemplar's decoders (at most
//! two channels). Off the audio thread.

use cantus_dsp::aula_shape::Ir;
use std::path::Path;

/// The longest file read, seconds (longer ones are cut; Aula holds 20 s at most).
const MAX_SECONDS: u64 = 60;

pub fn read(path: &Path) -> Result<Ir, String> {
    if let Some(r) = crate::inst::sampler::decode::decode(path) {
        let d = r?;
        let n = d.channels.max(1) as usize;
        let mut channels = vec![Vec::with_capacity(d.frames.len() / n); n];
        for (i, v) in d.frames.iter().enumerate() {
            channels[i % n].push(*v);
        }
        return finish(d.rate, channels);
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    read_wav(&bytes)
}

/// A WAV file's bytes as a response.
pub fn read_wav(bytes: &[u8]) -> Result<Ir, String> {
    let mut reader =
        hound::WavReader::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let n = spec.channels.max(1) as usize;
    let cap = (MAX_SECONDS * u64::from(spec.sample_rate)) as usize * n;
    let total = (reader.len() as usize).min(cap);
    let mut channels: Vec<Vec<f32>> = vec![Vec::with_capacity(total / n + 1); n];
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for (i, s) in reader.samples::<f32>().take(total).enumerate() {
                channels[i % n].push(s.map_err(|e| e.to_string())?);
            }
        }
        hound::SampleFormat::Int => {
            let scale = 1.0 / ((1i64 << (spec.bits_per_sample.clamp(1, 32) - 1)) as f64);
            for (i, s) in reader.samples::<i32>().take(total).enumerate() {
                channels[i % n].push((s.map_err(|e| e.to_string())? as f64 * scale) as f32);
            }
        }
    }
    finish(spec.sample_rate, channels)
}

fn finish(rate: u32, channels: Vec<Vec<f32>>) -> Result<Ir, String> {
    if rate == 0 {
        return Err("the file has no sample rate".into());
    }
    if channels.iter().all(|c| c.is_empty()) {
        return Err("the file holds no audio".into());
    }
    if channels.iter().flatten().any(|v| !v.is_finite()) {
        return Err("the file holds values that are not numbers".into());
    }
    Ok(Ir::from_channels(rate as f32, channels))
}
