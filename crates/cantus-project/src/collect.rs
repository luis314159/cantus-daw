//! Audio files on save (ADR 0025 §2): every file the project's pool points at ends up in
//! `<project folder>/Audio/`, and the pool's paths become relative to the project file.
//!
//! - A **recording** that still lives outside any saved project (the session's scratch
//!   folder) is **moved** in: it belongs to nobody else.
//! - An **import**, or a file that belongs to another project's folder (a *Save as* to a
//!   new folder), is **copied** in: the original stays where its owner expects it.
//! - A file already inside the project's folder is left where it is; only its path is
//!   written relative.
//! - A **missing** file is left alone; a relative path that would no longer resolve from
//!   the new folder is written absolute, so the file is found if it comes back.
//!
//! Names are kept, made unique in `Audio/` (`Kick.wav`, `Kick (2).wav`). Two sources that
//! point at one file share one copy. Planning reads the disk and moves nothing;
//! [`execute`] does the file work and reports the paths to set — the caller turns them
//! into `SetAudioSourcePath` commands, so the change is one undo step and the saved file
//! carries the new paths.

use cantus_core::{AudioSourceId, Project};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// The folder next to the project file that holds its audio.
pub const AUDIO_DIR: &str = "Audio";

/// What happens to one file.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileAction {
    /// Already in place: only the stored path changes.
    Keep,
    Move,
    Copy,
}

/// One source's part of a collection.
#[derive(Clone, Debug, PartialEq)]
pub struct FileOp {
    pub source: AudioSourceId,
    pub from: PathBuf,
    pub to: PathBuf,
    pub action: FileAction,
    /// The path to store: relative to the project file's folder, `/`-separated.
    pub path: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub ops: Vec<FileOp>,
    /// Sources whose file is not there; with the path to store for them, when it changes.
    pub missing: Vec<(AudioSourceId, Option<String>)>,
}

/// What [`execute`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// `(source, path to store)`, for every source whose stored path should change.
    pub paths: Vec<(AudioSourceId, String)>,
    /// `(old file, new file)` for every file moved or copied (a peak cache can follow).
    pub moved: Vec<(PathBuf, PathBuf)>,
    /// What failed, as `(file, error)`; those sources keep their old path.
    pub errors: Vec<(PathBuf, String)>,
}

/// Where a stored path points, against the folder of the project file (`base`).
pub fn resolve(base: Option<&Path>, path: &str) -> PathBuf {
    let p = Path::new(path);
    match base {
        Some(dir) if p.is_relative() => dir.join(p),
        _ => p.to_path_buf(),
    }
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// `path` relative to `dir`, `/`-separated, when it is inside it.
fn relative(path: &Path, dir: &Path) -> Option<String> {
    let rest = path.strip_prefix(dir).ok()?;
    let parts: Vec<String> = rest
        .components()
        .filter_map(|c| match c {
            Component::Normal(s) => Some(s.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// A file name not yet used in `dir` nor by an earlier op of this plan.
fn unique(dir: &Path, name: &str, taken: &[PathBuf]) -> PathBuf {
    let p = Path::new(name);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "audio".into());
    let ext = p.extension().map(|e| e.to_string_lossy().into_owned());
    let named = |n: usize| {
        let s = if n == 1 {
            stem.clone()
        } else {
            format!("{stem} ({n})")
        };
        match &ext {
            Some(e) => dir.join(format!("{s}.{e}")),
            None => dir.join(s),
        }
    };
    let mut n = 1;
    loop {
        let candidate = named(n);
        if !candidate.exists() && !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Plan the collection of `project`'s audio for saving it at `project_file`. `old_base` is
/// the folder the project was last saved in (`None` for a project never saved).
pub fn plan(project: &Project, old_base: Option<&Path>, project_file: &Path) -> Plan {
    let mut out = Plan::default();
    let Some(dir) = project_file.parent() else {
        return out;
    };
    let dir = if dir.as_os_str().is_empty() {
        Path::new(".")
    } else {
        dir
    };
    let dir_c = canonical(dir);
    let old_c = old_base.map(canonical);
    let audio = dir_c.join(AUDIO_DIR);
    let mut taken: Vec<PathBuf> = Vec::new();
    let mut shared: HashMap<PathBuf, (PathBuf, String)> = HashMap::new();
    for s in &project.audio_pool {
        let from = resolve(old_base, &s.path);
        if !from.is_file() {
            // Leave it, unless its relative path would stop meaning the same place.
            let moved_base = old_c.as_ref() != Some(&dir_c);
            let rewrite = (Path::new(&s.path).is_relative() && moved_base)
                .then(|| from.to_string_lossy().into_owned());
            out.missing.push((s.id, rewrite));
            continue;
        }
        let from_c = canonical(&from);
        if let Some((to, path)) = shared.get(&from_c) {
            // The first source of this file does the file work; this one follows it.
            out.ops.push(FileOp {
                source: s.id,
                from: from_c.clone(),
                to: to.clone(),
                action: FileAction::Keep,
                path: path.clone(),
            });
            continue;
        }
        let (to, action) = if relative(&from_c, &dir_c).is_some() {
            (from_c.clone(), FileAction::Keep)
        } else {
            let name = from_c
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| format!("{}.wav", s.name));
            let to = unique(&audio, &name, &taken);
            taken.push(to.clone());
            let in_old_project = old_c.as_ref().is_some_and(|b| from_c.starts_with(b));
            let action = if s.recorded && !in_old_project {
                FileAction::Move
            } else {
                FileAction::Copy
            };
            (to, action)
        };
        let path = relative(&to, &dir_c).unwrap_or_else(|| to.to_string_lossy().into_owned());
        shared.insert(from_c.clone(), (to.clone(), path.clone()));
        out.ops.push(FileOp {
            source: s.id,
            from: from_c,
            to,
            action,
            path,
        });
    }
    out
}

fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    // Another volume: copy, then remove the original only once the copy is whole.
    std::fs::copy(from, to)?;
    std::fs::remove_file(from)
}

/// Put back what [`execute`] did, when the path changes it was for are refused (round 29,
/// review 1): the files it moved moved back, its copies removed.
pub fn undo(plan: &Plan, out: &Outcome) {
    for (from, to) in out.moved.iter().rev() {
        let moved = plan
            .ops
            .iter()
            .any(|o| &o.from == from && &o.to == to && o.action == FileAction::Move);
        if moved {
            let _ = move_file(to, from);
        } else {
            let _ = std::fs::remove_file(to);
        }
    }
}

/// Do the file work of `plan` against the pool as it is in `project`. Never overwrites a
/// file; a failure leaves that source's path unchanged and is reported.
pub fn execute(project: &Project, plan: &Plan) -> Outcome {
    let mut out = Outcome::default();
    let mut failed: Vec<PathBuf> = Vec::new();
    for op in &plan.ops {
        let done = match op.action {
            FileAction::Keep => Ok(()),
            FileAction::Move | FileAction::Copy => {
                let r = op
                    .to
                    .parent()
                    .map(std::fs::create_dir_all)
                    .unwrap_or(Ok(()))
                    .and_then(|_| {
                        if op.to.exists() {
                            Err(std::io::Error::new(
                                std::io::ErrorKind::AlreadyExists,
                                "a file with that name is already there",
                            ))
                        } else if op.action == FileAction::Move {
                            move_file(&op.from, &op.to)
                        } else {
                            std::fs::copy(&op.from, &op.to).map(|_| ())
                        }
                    });
                if r.is_ok() {
                    out.moved.push((op.from.clone(), op.to.clone()));
                }
                r
            }
        };
        match done {
            Ok(()) if !failed.contains(&op.from) => {
                let current = project.audio_source(op.source).map(|s| s.path.as_str());
                if current != Some(op.path.as_str()) {
                    out.paths.push((op.source, op.path.clone()));
                }
            }
            Ok(()) => {}
            Err(e) => {
                failed.push(op.from.clone());
                out.errors.push((op.from.clone(), e.to_string()));
            }
        }
    }
    for (id, path) in &plan.missing {
        if let Some(p) = path {
            out.paths.push((*id, p.clone()));
        }
    }
    out
}
