//! Loading sample files for the kit's sample pads, and attaching them to snapshots.
//!
//! The audio thread cannot read a file, and `cantus-core` does no I/O, so this is the one
//! place a pad's path becomes audio. [`SampleCache::attach`] fills in
//! `TrackSnapshot::samples` — the same snapshot then serves real-time playback and offline
//! render, which is what keeps the two bit-identical without a second code path.
//!
//! **The cache never evicts.** Every `Arc<SampleData>` the audio thread holds has a sibling
//! here, so dropping one there is an atomic decrement and never a free (ADR 0005). A
//! session's worth of drum samples is megabytes; the rule is cheap and it is what makes
//! the audio thread's handling of `Arc`s safe by construction rather than by care.

use cantus_core::{SampleData, Snapshot};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What became of a path the project points at.
#[derive(Clone, Debug, PartialEq)]
pub enum SampleStatus {
    Loaded {
        rate: u32,
        channels: u8,
        frames: usize,
    },
    /// The file could not be read; the pad is silent and the inspector says why.
    Failed(String),
    /// Never asked for — a path no snapshot has referenced yet.
    Unknown,
}

#[derive(Debug, Default)]
pub struct SampleCache {
    loaded: HashMap<PathBuf, Arc<SampleData>>,
    failed: HashMap<PathBuf, String>,
    /// Exemplar's octave mip-maps of loaded files (never evicted either).
    mips: crate::inst::sampler::MipCache,
}

impl SampleCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a WAV, or hand back the copy already loaded. Failures are remembered too, so
    /// a missing file is reported once rather than retried on every snapshot.
    pub fn load(&mut self, path: &Path) -> Result<Arc<SampleData>, String> {
        if let Some(s) = self.loaded.get(path) {
            return Ok(s.clone());
        }
        if let Some(e) = self.failed.get(path) {
            return Err(e.clone());
        }
        // AIFF, FLAC and Ogg by extension (Exemplar's decoders); everything else as WAV.
        let decoded =
            crate::inst::sampler::decode::decode(path).unwrap_or_else(|| decode_wav(path));
        match decoded {
            Ok(data) => {
                let data = Arc::new(data);
                self.loaded.insert(path.to_path_buf(), data.clone());
                Ok(data)
            }
            Err(e) => {
                self.failed.insert(path.to_path_buf(), e.clone());
                Err(e)
            }
        }
    }

    /// Forget a failure, so the next snapshot tries the file again — for when the user
    /// has just put it back.
    pub fn retry(&mut self, path: &Path) {
        self.failed.remove(path);
    }

    pub fn status(&self, path: &Path) -> SampleStatus {
        if let Some(s) = self.loaded.get(path) {
            SampleStatus::Loaded {
                rate: s.rate,
                channels: s.channels,
                frames: s.len_frames(),
            }
        } else if let Some(e) = self.failed.get(path) {
            SampleStatus::Failed(e.clone())
        } else {
            SampleStatus::Unknown
        }
    }

    /// Fill in every kit's `samples`, aligned with its pads. Returns the paths that could
    /// not be loaded, for the status line.
    pub fn attach(&mut self, snapshot: &mut Snapshot) -> Vec<(PathBuf, String)> {
        let mut errors = Vec::new();
        for track in &mut snapshot.tracks {
            // A native instrument's assets, by key (ADR 0023).
            track.assets = match track.instrument.native() {
                Some(n) => {
                    let mut assets: Vec<_> = n
                        .assets()
                        .map(|(key, path)| {
                            let path = Path::new(path);
                            let audio = match self.load(path) {
                                Ok(data) => Some(data),
                                Err(e) => {
                                    errors.push((path.to_path_buf(), e));
                                    None
                                }
                            };
                            (key.to_string(), audio)
                        })
                        .collect();
                    if n.kind == cantus_core::InstrumentKind::Sampler {
                        crate::inst::sampler::attach_mips(n, &mut assets, &mut self.mips);
                    }
                    assets
                }
                None => Vec::new(),
            };
            let Some(kit) = track.instrument.pulsus() else {
                track.samples.clear();
                continue;
            };
            // One per pad, then every pad's layers in pad order (Pulsus II).
            track.samples = kit
                .pads
                .iter()
                .map(|pad| pad.sample.as_deref())
                .chain(kit.layer_paths().map(Some))
                .map(|path| {
                    let path = Path::new(path?);
                    match self.load(path) {
                        Ok(data) => Some(data),
                        Err(e) => {
                            errors.push((path.to_path_buf(), e));
                            None
                        }
                    }
                })
                .collect();
        }
        errors
    }
}

/// Decode a WAV to interleaved `f32`, keeping at most two channels.
fn decode_wav(path: &Path) -> Result<SampleData, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    let in_channels = spec.channels.max(1) as usize;
    let out_channels = in_channels.min(2);
    let mut frames: Vec<f32> = Vec::new();
    let mut push = |i: usize, v: f32| {
        if i % in_channels < out_channels {
            frames.push(v);
        }
    };
    match spec.sample_format {
        hound::SampleFormat::Float => {
            for (i, s) in reader.samples::<f32>().enumerate() {
                push(i, s.map_err(|e| e.to_string())?);
            }
        }
        hound::SampleFormat::Int => {
            let scale = 1.0 / ((1i64 << (spec.bits_per_sample - 1)) as f32);
            for (i, s) in reader.samples::<i32>().enumerate() {
                push(i, s.map_err(|e| e.to_string())? as f32 * scale);
            }
        }
    }
    if frames.is_empty() {
        return Err("the file holds no audio".to_string());
    }
    Ok(SampleData {
        rate: spec.sample_rate,
        channels: out_channels as u8,
        frames: frames.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_wav(path: &Path, channels: u16, rate: u32, frames: usize) {
        let spec = hound::WavSpec {
            channels,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for i in 0..frames {
            for c in 0..channels {
                let v = ((i as f32 * 0.05 + c as f32).sin() * 16_000.0) as i16;
                w.write_sample(v).unwrap();
            }
        }
        w.finalize().unwrap();
    }

    #[test]
    fn loads_once_and_remembers_failures() {
        let dir = tempfile::tempdir().unwrap();
        let ok = dir.path().join("ok.wav");
        write_wav(&ok, 2, 44_100, 1_000);
        let mut cache = SampleCache::new();
        let a = cache.load(&ok).unwrap();
        let b = cache.load(&ok).unwrap();
        assert!(
            Arc::ptr_eq(&a, &b),
            "the second load must be the same buffer"
        );
        assert_eq!(a.channels, 2);
        assert_eq!(a.rate, 44_100);
        assert_eq!(a.len_frames(), 1_000);
        assert!(matches!(
            cache.status(&ok),
            SampleStatus::Loaded { frames: 1_000, .. }
        ));

        let missing = dir.path().join("missing.wav");
        assert!(cache.load(&missing).is_err());
        assert!(matches!(cache.status(&missing), SampleStatus::Failed(_)));
        assert_eq!(cache.status(Path::new("never")), SampleStatus::Unknown);
    }

    #[test]
    fn a_five_channel_file_keeps_its_first_two() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("surround.wav");
        write_wav(&p, 5, 48_000, 200);
        let s = SampleCache::new().load(&p).unwrap();
        assert_eq!(s.channels, 2);
        assert_eq!(s.len_frames(), 200);
    }
}
