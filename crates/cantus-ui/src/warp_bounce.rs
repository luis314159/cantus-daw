//! *Bounce warped audio* (ADR 0061): the window a warped clip plays, rendered exactly as the
//! engine plays it (the same attach, the same render), written to a file beside the
//! project's audio on a thread of its own; then the clip plays that file, unwarped — one
//! undo step (the new source and the clip's new region together).

use super::actions::{Bounce, Written};
use crate::app::CantusApp;
use crate::clips::ClipRef;
use crate::i18n::{t, tf};
use cantus_core::{AudioRegion, AudioSource, AudioSourceId, Command, Snapshot, Ticks};
use std::path::{Path, PathBuf};

/// Render clip `id` of `snapshot` (its first window) through a cache of its own and write
/// it to `file` (32-bit float WAV at `rate`).
fn write(
    mut snapshot: Snapshot,
    dir: Option<PathBuf>,
    id: cantus_core::ClipId,
    rate: u32,
    file: &Path,
) -> Result<Written, String> {
    let mut cache = cantus_engine::SampleCache::new();
    cache.set_project_dir(dir);
    let errors = cache.attach_audio(&mut snapshot, rate);
    let data = snapshot
        .tracks
        .iter()
        .flat_map(|t| t.audio.iter())
        .find(|c| c.clip == id)
        .and_then(|c| c.data.clone())
        .ok_or_else(|| {
            errors
                .first()
                .map(|e| e.1.clone())
                .unwrap_or_else(|| t("warp.bounce.nothing"))
        })?;
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let spec = hound::WavSpec {
        channels: data.channels.max(1) as u16,
        sample_rate: data.rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(file, spec).map_err(|e| e.to_string())?;
    for s in data.frames.iter() {
        w.write_sample(*s).map_err(|e| e.to_string())?;
    }
    w.finalize().map_err(|e| e.to_string())?;
    Ok(Written {
        path: file.to_path_buf(),
        rate: data.rate,
        channels: data.channels.max(1),
        frames: data.len_frames() as u64,
    })
}

/// A file name not yet taken in `dir`.
fn free_name(dir: &Path, stem: &str) -> PathBuf {
    let clean: String = stem
        .chars()
        .map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    for n in 0.. {
        let name = if n == 0 {
            format!("{clean}.wav")
        } else {
            format!("{clean} {n}.wav")
        };
        let p = dir.join(name);
        if !p.exists() {
            return p;
        }
    }
    unreachable!()
}

impl CantusApp {
    /// Start bouncing warped clip `r` (one at a time).
    pub fn bounce_warped(&mut self, r: ClipRef) -> bool {
        if self.warp.bounce.is_some() {
            self.set_status(t("warp.bounce.busy"));
            return false;
        }
        let Some(c) = self.project().track(r.0).and_then(|t| t.clip(r.1)).cloned() else {
            return false;
        };
        if self.clip_warp(r).is_none() {
            return false;
        }
        let dir = match self.audio_base() {
            Some(b) => b.join("Audio"),
            None => crate::app::data_dir().join("bounces"),
        };
        let file = free_name(&dir, &tf("warp.bounce.file", &[("name", &c.name)]));
        let snapshot = Snapshot::compile(self.project(), 0);
        let base = self.audio_base().map(Path::to_path_buf);
        let rate = self.engine.sample_rate.round().max(1.0) as u32;
        let (tx, rx) = std::sync::mpsc::channel();
        let id = c.id;
        let spawned = std::thread::Builder::new()
            .name("cantus-warp-bounce".into())
            .spawn(move || {
                let _ = tx.send(write(snapshot, base, id, rate, &file));
            });
        if spawned.is_err() {
            return false;
        }
        self.warp.bounce = Some(Bounce::new(r, c.name.clone(), rx));
        self.set_status(tf("warp.bounce.working", &[("name", &c.name)]));
        true
    }

    /// Take in a finished bounce: the file joins the pool and the clip plays it.
    pub(super) fn poll_bounce(&mut self) {
        let Some(b) = self.warp.bounce.as_ref() else {
            return;
        };
        let Some(result) = b.try_take() else {
            return;
        };
        let b = self.warp.bounce.take().expect("a bounce");
        match result {
            Err(e) => self.set_error(tf("warp.bounce.failed", &[("error", &e)])),
            Ok(w) => {
                let Some(c) = self
                    .project()
                    .track(b.r.0)
                    .and_then(|t| t.clip(b.r.1))
                    .cloned()
                else {
                    return;
                };
                let Some(a) = c.audio.clone() else { return };
                let id = AudioSourceId(self.bus.alloc_id());
                let base = self.audio_base().map(Path::to_path_buf);
                let source = AudioSource {
                    id,
                    name: tf("warp.bounce.file", &[("name", &b.name)]),
                    path: crate::audio_files::stored_path(base.as_deref(), &w.path),
                    sample_rate: w.rate,
                    channels: w.channels,
                    frames: w.frames,
                    sha256: None,
                    recorded: false,
                };
                let region = AudioRegion {
                    source: id,
                    slip_frames: 0,
                    pitch: None,
                    warp: None,
                    ..a
                };
                let commands = vec![
                    Command::InsertAudioSource { source },
                    Command::SetClipAudio {
                        track: b.r.0,
                        clip: b.r.1,
                        audio: Some(region),
                    },
                    Command::SetClipOffset {
                        track: b.r.0,
                        clip: b.r.1,
                        offset: Ticks::ZERO,
                    },
                ];
                if self.exec(Command::Batch {
                    label_key: "cmd.warp.bounce".into(),
                    commands,
                }) {
                    self.set_status(tf("warp.bounce.done", &[("name", &b.name)]));
                }
            }
        }
    }
}

impl Bounce {
    pub(super) fn new(
        r: ClipRef,
        name: String,
        rx: std::sync::mpsc::Receiver<Result<Written, String>>,
    ) -> Self {
        Bounce { r, name, rx }
    }

    fn try_take(&self) -> Option<Result<Written, String>> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(t("warp.bounce.nothing"))),
        }
    }
}
