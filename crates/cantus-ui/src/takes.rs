//! The mix recorder, UI side: turning the engine's capture ring into WAV files.
//!
//! The audio thread copies what leaves a strip's fader (or the master) into a lock-free
//! ring (`cantus_engine::CaptureTarget`); the UI drains the ring once a frame and hands
//! each chunk to a **writer thread** that owns the file. Nothing here touches the disk on
//! the UI thread, and nothing touches it on the audio thread — the rule ADR 0005 makes for
//! the one and the rule a UI that must not stutter makes for the other.
//!
//! A take is a 32-bit float stereo WAV, which is what the engine produces: no conversion,
//! no dither, the bits that left the fader.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Instant;

/// A finished recording.
#[derive(Clone, Debug, PartialEq)]
pub struct Take {
    /// What was recorded: "Master", "Strip 3 · Keys".
    pub label: String,
    pub path: PathBuf,
    pub seconds: f64,
    /// Largest absolute sample, linear.
    pub peak: f32,
}

struct Active {
    label: String,
    path: PathBuf,
    sample_rate: u32,
    frames: u64,
    peak: f32,
    started: Instant,
    tx: mpsc::Sender<Vec<f32>>,
    writer: JoinHandle<Result<(), String>>,
}

/// The recorder: at most one take running, and the takes made this session.
#[derive(Default)]
pub struct MixRecorder {
    active: Option<Active>,
    pub takes: Vec<Take>,
}

impl std::fmt::Debug for MixRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MixRecorder")
            .field("recording", &self.active.as_ref().map(|a| &a.label))
            .field("takes", &self.takes.len())
            .finish()
    }
}

/// A file name for a take that sorts by time and says what it is: `2026-09-23 14-05-09
/// Strip 3.wav`. Characters a file system refuses are replaced.
pub fn take_file_name(label: &str, stamp: &str) -> String {
    let clean: String = label
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '-' } else { c })
        .collect();
    format!("{stamp} {}.wav", clean.trim())
}

/// A sortable local-free timestamp from the Unix clock: `YYYY-MM-DD HH-MM-SS` in UTC.
pub fn stamp_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3600, rem % 3600 / 60, rem % 60);
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if mo <= 2 { 1 } else { 0 };
    format!("{y:04}-{mo:02}-{d:02} {h:02}-{m:02}-{s:02}")
}

impl MixRecorder {
    pub fn is_recording(&self) -> bool {
        self.active.is_some()
    }

    /// What is being recorded, and for how long so far (seconds of audio written).
    pub fn status(&self) -> Option<(&str, f64)> {
        self.active.as_ref().map(|a| {
            (
                a.label.as_str(),
                a.frames as f64 / a.sample_rate.max(1) as f64,
            )
        })
    }

    /// Start a take into `dir`. Fails if a take is running or the file cannot be made.
    pub fn start(&mut self, label: &str, dir: &Path, sample_rate: u32) -> Result<PathBuf, String> {
        if self.active.is_some() {
            return Err("a take is already running".into());
        }
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let mut path = dir.join(take_file_name(label, &stamp_now()));
        let mut n = 2;
        while path.exists() {
            path = dir.join(take_file_name(&format!("{label} {n}"), &stamp_now()));
            n += 1;
        }
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(&path, spec).map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel::<Vec<f32>>();
        let writer = std::thread::Builder::new()
            .name("cantus-take-writer".into())
            .spawn(move || -> Result<(), String> {
                for chunk in rx {
                    for x in chunk {
                        writer.write_sample(x).map_err(|e| e.to_string())?;
                    }
                }
                writer.finalize().map_err(|e| e.to_string())
            })
            .map_err(|e| e.to_string())?;
        self.active = Some(Active {
            label: label.to_string(),
            path: path.clone(),
            sample_rate,
            frames: 0,
            peak: 0.0,
            started: Instant::now(),
            tx,
            writer,
        });
        Ok(path)
    }

    /// Hand interleaved stereo samples to the running take. Nothing happens without one.
    pub fn pump(&mut self, samples: &[f32]) {
        let Some(a) = self.active.as_mut() else {
            return;
        };
        if samples.is_empty() {
            return;
        }
        a.frames += (samples.len() / 2) as u64;
        a.peak = samples.iter().fold(a.peak, |m, x| m.max(x.abs()));
        // A writer that has died (a full disk) is reported at `stop`; until then the
        // samples are simply not delivered.
        let _ = a.tx.send(samples.to_vec());
    }

    /// Finish the running take: close the channel, wait for the writer to finalize the
    /// file, and add the take to the list.
    pub fn stop(&mut self) -> Option<Result<Take, String>> {
        let a = self.active.take()?;
        drop(a.tx);
        let written = a
            .writer
            .join()
            .map_err(|_| "the take writer panicked".to_string())
            .and_then(|r| r);
        let _ = a.started;
        Some(written.map(|()| {
            let take = Take {
                label: a.label,
                path: a.path,
                seconds: a.frames as f64 / a.sample_rate.max(1) as f64,
                peak: a.peak,
            };
            self.takes.push(take.clone());
            take
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Samples pumped in are the samples in the file, bit for bit, in order.
    #[test]
    fn a_take_is_the_samples_it_was_given() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = MixRecorder::default();
        r.start("Strip 3 · Keys", dir.path(), 48_000).unwrap();
        assert!(r.is_recording());
        let mut all = Vec::new();
        for block in 0..50 {
            let chunk: Vec<f32> = (0..256)
                .map(|i| ((block * 256 + i) as f32 * 0.01).sin() * 0.5)
                .collect();
            r.pump(&chunk);
            all.extend(chunk);
        }
        let (label, secs) = r.status().unwrap();
        assert_eq!(label, "Strip 3 · Keys");
        assert!((secs - 50.0 * 128.0 / 48_000.0).abs() < 1e-9);
        let take = r.stop().unwrap().unwrap();
        assert!(!r.is_recording());
        assert_eq!(r.takes.len(), 1);
        assert!((take.peak - 0.5).abs() < 1e-3);

        let mut reader = hound::WavReader::open(&take.path).unwrap();
        let spec = reader.spec();
        assert_eq!(
            (spec.channels, spec.sample_rate, spec.bits_per_sample),
            (2, 48_000, 32)
        );
        let back: Vec<f32> = reader.samples::<f32>().map(|s| s.unwrap()).collect();
        assert_eq!(back, all, "the file is not what was recorded");
        assert!(take
            .path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with("Strip 3 · Keys.wav"));
    }

    #[test]
    fn one_take_at_a_time_and_names_are_safe() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = MixRecorder::default();
        r.start("Master", dir.path(), 44_100).unwrap();
        assert!(r.start("Master", dir.path(), 44_100).is_err());
        r.stop().unwrap().unwrap();
        assert!(r.stop().is_none(), "stopping twice is nothing");
        assert_eq!(take_file_name("a/b:c", "S"), "S a-b-c.wav");
        let s = stamp_now();
        assert_eq!(s.len(), 19, "{s}");
    }
}
