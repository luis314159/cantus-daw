//! The recovery offer (`recovery`), built from the journals of sessions that are gone,
//! and what *Restore* does with it.

use crate::recovery::{repair_wav, JournalPass, Orphan};
use cantus_core::Project;
use cantus_engine::FinishedTake;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// What the start of a session offers back.
#[derive(Debug, Default)]
pub struct Offer {
    pub items: Vec<Item>,
}

/// One gone session's work.
#[derive(Debug)]
pub struct Item {
    pub orphan: Orphan,
    pub name: String,
    pub project_path: Option<PathBuf>,
    /// The autosave to put in the project's place (it loads), and when it was written.
    pub autosave: Option<(PathBuf, Project)>,
    pub autosaved_at: Option<u64>,
    /// The session crashed (it did not close).
    pub crashed: bool,
    pub passes: Vec<RecoveredPass>,
}

/// A pass whose takes are on disk but in no file of the project.
#[derive(Debug, Clone)]
pub struct RecoveredPass {
    pub journal: JournalPass,
    pub takes: Vec<FinishedTake>,
}

impl Item {
    pub fn take_count(&self) -> usize {
        self.passes.iter().map(|p| p.takes.len()).sum()
    }
}

/// A path as compared: separators and case folded (Windows file names).
fn key(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/").to_lowercase()
}

/// Every audio file `project` refers to, absolute (relative ones against `base`).
fn referenced(project: &Project, base: Option<&Path>) -> HashSet<String> {
    project
        .audio_pool
        .iter()
        .map(|s| {
            let p = Path::new(&s.path);
            match base {
                Some(b) if p.is_relative() => key(&b.join(p)),
                _ => key(p),
            }
        })
        .collect()
}

/// The frame of `pass`'s files where the loop wrapped, from where the transport started:
/// what the engine would have marked (the takes then fall onto their lanes as a finished
/// pass's do).
pub fn wraps(pass: &JournalPass, frames: u64) -> Vec<u64> {
    let Some((a, b)) = pass.loop_region else {
        return Vec::new();
    };
    if b <= a || pass.tempo_bpm <= 0.0 {
        return Vec::new();
    }
    let fpt = f64::from(pass.sample_rate) * 60.0 / (pass.tempo_bpm * cantus_core::PPQ as f64);
    let first = (b - pass.transport_start).0;
    if first <= 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut at = first as f64;
    loop {
        let f = (at * fpt).round() as u64;
        if f >= frames {
            break;
        }
        out.push(f);
        at += (b - a).0 as f64;
    }
    out
}

/// A take of `pass` as the engine would have finished it, read back from its file (the
/// header repaired first). `None` when the file is gone, empty or unreadable.
pub fn finished_take(
    pass: &JournalPass,
    track: cantus_core::TrackId,
    path: &Path,
) -> Option<FinishedTake> {
    let frames = repair_wav(path).ok()?;
    if frames == 0 {
        return None;
    }
    let spec = hound::WavReader::open(path).ok()?.spec();
    let fpt =
        f64::from(spec.sample_rate) * 60.0 / (pass.tempo_bpm.max(1.0) * cantus_core::PPQ as f64);
    Some(FinishedTake {
        track,
        path: path.to_path_buf(),
        sample_rate: spec.sample_rate,
        channels: spec.channels as u8,
        frames,
        start_tick: pass.transport_start.0 as f64 - pass.latency_frames as f64 / fpt,
        wraps: wraps(pass, frames),
        dropped: 0,
    })
}

/// What a gone session left to offer: its autosave when it had unsaved changes, and the
/// takes on disk that no file of the project holds. `None` when there is nothing.
pub fn item(orphan: Orphan) -> Option<Item> {
    let j = &orphan.journal;
    let autosave = j.autosave.clone().filter(|_| j.dirty).and_then(|p| {
        let l = cantus_project::load(&p, true)
            .or_else(|_| cantus_project::load(&p, false))
            .ok()?;
        Some((p, l.project))
    });
    let held = match (&autosave, &j.project_path) {
        (Some((_, p)), _) => referenced(p, None),
        (None, Some(path)) => cantus_project::load(path, false)
            .map(|l| referenced(&l.project, path.parent()))
            .unwrap_or_default(),
        (None, None) => HashSet::new(),
    };
    let passes: Vec<RecoveredPass> = j
        .passes
        .iter()
        .filter_map(|pass| {
            let takes: Vec<FinishedTake> = pass
                .takes
                .iter()
                .filter(|t| !held.contains(&key(&t.path)))
                .filter_map(|t| finished_take(pass, t.track, &t.path))
                .collect();
            (!takes.is_empty()).then(|| RecoveredPass {
                journal: pass.clone(),
                takes,
            })
        })
        .collect();
    if autosave.is_none() && passes.is_empty() {
        return None;
    }
    Some(Item {
        name: autosave
            .as_ref()
            .map(|(_, p)| p.name.clone())
            .unwrap_or_else(|| j.project_name.clone()),
        project_path: j.project_path.clone(),
        autosaved_at: autosave.as_ref().and(j.autosaved_at),
        crashed: !j.closed,
        autosave,
        passes,
        orphan,
    })
}

/// The offer at start: the most recent gone session with something to give back. Journals
/// with nothing are forgotten; older ones with something wait for the next start.
pub fn offer(orphans: Vec<Orphan>) -> Offer {
    let mut items = Vec::new();
    for o in orphans.into_iter().rev() {
        if !items.is_empty() {
            break;
        }
        let json = o.clone();
        match item(o) {
            Some(i) => items.push(i),
            None => json.forget(),
        }
    }
    Offer { items }
}
