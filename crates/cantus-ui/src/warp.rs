//! Warping, from the application's side (round 29, ADR 0061): the analysis asked at every
//! import (off this thread, on the engine's warp service), the **import rule** — a short
//! loop whose tempo is sure is warped to follow the project at once, in the import's own
//! undo step; a long file or a recording plays at its natural speed until the user warps
//! it — and every warp edit as one `SetClipWarp`.
//!
//! The verbs are in `warp_actions.rs`; the editor is `views/warp.rs`, the arrangement's
//! drawing and gestures `views/arrangement_warp.rs`, the inspector's section
//! `views/inspector_warp.rs`.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use cantus_core::warp::{Detected, Warp, WarpMode, WarpOp};
use cantus_core::{Clip, Command, MergeKey, Project, Ticks};
use cantus_engine::warp::{Analysis, Lookup};
use std::path::PathBuf;
use std::sync::Arc;

#[path = "warp_actions.rs"]
mod actions;
pub use actions::*;
#[path = "warp_bounce.rs"]
mod bounce;

/// A file this long or shorter may be a loop that warps itself at import, seconds.
pub const AUTO_MAX_S: f64 = 60.0;
/// The confidence a detected tempo needs to warp a loop by itself.
pub const AUTO_CONFIDENCE: f32 = 0.5;
/// How long an import waits for its analysis before the clip simply stays as it is.
pub const AUTO_PATIENCE_S: f64 = 120.0;

/// An imported clip waiting for its file's analysis.
#[derive(Clone, Debug)]
pub struct AutoWarp {
    pub r: ClipRef,
    pub path: PathBuf,
    pub rate: u32,
    /// The import's undo step: the warp lands in it while it is still the newest.
    pub key: MergeKey,
    pub since: std::time::Instant,
}

/// Quantise audio's choices (the dialog remembers them).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct QuantiseSettings {
    pub grid: Ticks,
    /// 0 … 1.
    pub strength: f32,
    /// Only transients this strong or stronger, as a share of the clip's strongest.
    pub threshold: f32,
}

impl Default for QuantiseSettings {
    fn default() -> Self {
        QuantiseSettings {
            grid: Ticks::SIXTEENTH,
            strength: 1.0,
            threshold: 0.1,
        }
    }
}

/// Warping's state on the app.
#[derive(Debug, Default)]
pub struct WarpUi {
    pub auto: Vec<AutoWarp>,
    /// The selected warp marker (Del removes it), on which clip.
    pub selected: Option<(ClipRef, usize)>,
    pub quantise: QuantiseSettings,
    /// The quantise dialog, open on these clips.
    pub quantise_open: Option<Vec<ClipRef>>,
    /// The warp editor's view.
    pub view: crate::views::warp::WarpView,
    /// A bounce of a warped clip on its way (a thread writing the file).
    pub bounce: Option<actions::Bounce>,
}

/// Whether the analysis says the file is a loop to warp at once (ADR 0061 §4): no longer
/// than [`AUTO_MAX_S`], a whole number of bars or an `acid` tempo, and a sure tempo.
pub fn is_auto_loop(a: &Analysis) -> bool {
    let seconds = a.frames as f64 / a.rate.max(1) as f64;
    let sure = a.acid_bpm.is_some() || a.grid.confidence >= AUTO_CONFIDENCE;
    seconds <= AUTO_MAX_S
        && sure
        && a.bpm() > 0.0
        && (a.grid.bars.is_some() || a.acid_bpm.is_some())
        && a.grid.first_downbeat().is_some()
}

/// The mode a loop is warped in: Rhythm for a percussive one (a transient a beat or
/// more), Complex otherwise.
pub fn mode_for(a: &Analysis) -> WarpMode {
    let seconds = a.frames as f64 / a.rate.max(1) as f64;
    let beats = seconds * a.bpm() / 60.0;
    if beats > 0.0 && a.onsets.len() as f64 >= beats {
        WarpMode::Rhythm
    } else {
        WarpMode::Complex
    }
}

/// The warp an analysis gives a file: markers on every bar of its grid, its tempo, what
/// was detected — in `mode`.
pub fn warp_from(a: &Analysis, beats_per_bar: u32, mode: WarpMode) -> Option<Warp> {
    let d = a.grid.first_downbeat()?;
    let mut w = Warp::from_grid(
        a.rate,
        a.bpm(),
        &a.grid.beats,
        a.grid.downbeat,
        beats_per_bar,
        Detected {
            bpm: a.bpm(),
            confidence: if a.acid_bpm.is_some() {
                1.0
            } else {
                a.grid.confidence
            },
            downbeat: d,
            bars: a.grid.bars,
        },
    );
    w.mode = mode;
    w.is_valid().then_some(w)
}

/// The import rule's commands for clip `r` of `project`, once its file's analysis is in:
/// the warp, and the window grown or shrunk to the loop's bars at the project's tempo.
/// `None` when the file is not a loop to warp by itself, or the clip changed meanwhile.
pub fn auto_warp(project: &Project, r: ClipRef, a: &Analysis) -> Option<Vec<Command>> {
    let clip: &Clip = project.track(r.0)?.clip(r.1)?;
    let region = clip.audio.as_ref()?;
    if region.warp.is_some() || clip.offset != Ticks::ZERO || !is_auto_loop(a) {
        return None;
    }
    let w = warp_from(a, 4, mode_for(a))?;
    let end = cantus_core::audio_time::warped_length(&w, a.frames as f64);
    let length = match a.grid.bars {
        Some(bars) => project.time_signature.ticks_per_bar() * bars as i64,
        None => end,
    }
    .max(Ticks(1));
    let mut out = vec![Command::SetClipWarp {
        track: r.0,
        clip: r.1,
        warp: Some(Box::new(w)),
        op: WarpOp::Set,
    }];
    if length != clip.length {
        out.push(Command::SetClipBounds {
            track: r.0,
            clip: r.1,
            start: clip.start,
            length,
        });
    }
    Some(out)
}

impl CantusApp {
    /// Ask for the analysis of what an import made (each clip's file), so a loop can warp
    /// itself in the import's undo step (`key`) when the analysis lands. Every import path
    /// comes here: *File ▸ Import*, a drop from the system or the browser, a paste.
    pub fn warp_after_import(&mut self, made: &[ClipRef], key: MergeKey) {
        for &r in made {
            let Some((s, path)) = self.pitch_source(r.0, r.1) else {
                continue;
            };
            let svc = self.engine.samples.warp().clone();
            let _ = svc.lookup(&path, s.sample_rate);
            self.warp.auto.push(AutoWarp {
                r,
                path,
                rate: s.sample_rate,
                key,
                since: std::time::Instant::now(),
            });
        }
    }

    /// A recording's take: analysed for its transients and tempo (the editor and *Quantise
    /// audio* use them), never warped by itself — it plays as it was played.
    pub fn analyse_take_for_warp(&mut self, r: ClipRef) {
        if let Some((s, path)) = self.pitch_source(r.0, r.1) {
            let _ = self.engine.samples.warp().lookup(&path, s.sample_rate);
        }
    }

    /// Every frame: the imports whose analysis landed get the import rule. Returns whether
    /// one is still waiting (the window keeps ticking).
    pub fn poll_warp(&mut self) -> bool {
        self.poll_bounce();
        if self.warp.auto.is_empty() {
            return self.warp.bounce.is_some();
        }
        let svc = self.engine.samples.warp().clone();
        let waiting = std::mem::take(&mut self.warp.auto);
        for job in waiting {
            match svc.lookup(&job.path, job.rate) {
                Lookup::Pending if job.since.elapsed().as_secs_f64() < AUTO_PATIENCE_S => {
                    self.warp.auto.push(job);
                }
                Lookup::Ready(a) => self.land_auto_warp(&job, &a),
                _ => {}
            }
        }
        !self.warp.auto.is_empty() || self.warp.bounce.is_some()
    }

    fn land_auto_warp(&mut self, job: &AutoWarp, a: &Arc<Analysis>) {
        let Some(commands) = auto_warp(self.project(), job.r, a) else {
            return;
        };
        // In the import's own step while it is the newest; its own step otherwise.
        let cmd = Command::Batch {
            label_key: "cmd.warp.auto".into(),
            commands,
        };
        if self.exec_merging(cmd, job.key) {
            self.end_gesture();
        }
    }

    /// The analysis of clip `r`'s file, asked of the warp service (queued the first time).
    pub fn warp_analysis(&self, r: ClipRef) -> Option<Arc<Analysis>> {
        let (s, path) = self.pitch_source(r.0, r.1)?;
        match self.engine.samples.warp().lookup(&path, s.sample_rate) {
            Lookup::Ready(a) => Some(a),
            _ => None,
        }
    }

    /// Whether clip `r`'s analysis is still on its way.
    pub fn warp_analysing(&self, r: ClipRef) -> bool {
        let Some((s, path)) = self.pitch_source(r.0, r.1) else {
            return false;
        };
        matches!(
            self.engine.samples.warp().lookup(&path, s.sample_rate),
            Lookup::Pending
        )
    }
}

/// A transposition in label/value units: `+3 st`, `-2 st 50 ct`, `+15 ct`.
pub fn write_cents(out: &mut String, cents: i32) {
    use std::fmt::Write as _;
    let sign = if cents < 0 { '-' } else { '+' };
    let (st, ct) = (cents.abs() / 100, cents.abs() % 100);
    let _ = match (st, ct) {
        (0, 0) => write!(out, "0 st"),
        (0, c) => write!(out, "{sign}{c} ct"),
        (s, 0) => write!(out, "{sign}{s} st"),
        (s, c) => write!(out, "{sign}{s} st {c} ct"),
    };
}

/// The audio files an imported `.dawproject` carries, written under the data folder
/// (`imported audio/<file name>/`) and the project's sources pointed at them; saving the
/// project collects them into its own folder as for any import.
pub fn place_media(project: &mut Project, media: &[(String, Vec<u8>)], name: &str) {
    if media.is_empty() {
        return;
    }
    let stem = std::path::Path::new(name)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "import".into());
    let dir = crate::app::data_dir().join("imported audio").join(stem);
    for (inner, bytes) in media {
        let file = dir.join(
            std::path::Path::new(inner)
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("audio.wav")),
        );
        let written = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&file, bytes));
        if written.is_err() {
            continue;
        }
        for s in project.audio_pool.iter_mut().filter(|s| s.path == *inner) {
            s.path = file.to_string_lossy().into_owned();
        }
    }
}

/// The status line after a file brought warps that were not valid, dropped on load.
pub fn repaired_text(n: usize) -> String {
    if n == 1 {
        crate::i18n::t("warp.status.repaired_one")
    } else {
        crate::i18n::tf("warp.status.repaired", &[("n", &n)])
    }
}
