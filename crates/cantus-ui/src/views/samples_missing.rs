//! A project whose sound pack is missing (round 26). Anything that plays a pack's files —
//! an Exemplar's zones, a Pulsus pad or its layers, an audio clip's source — names files
//! under the pack's folder. Opened on a computer without them (the pack was removed, its
//! folder renamed, files deleted, or the project came from another computer), those parts
//! would be silent with no word. Instead a card at the window's lower left says which pack,
//! which tracks, and what to do: install it (again) from *Sound packs…*, use this
//! computer's copy when it is at another place (one undo step), or find the pack's folder.
//! The rest of the project loads and plays.
//!
//! **Never on the interface thread's time**: a frame only hashes the project's sample
//! paths (no disk, no allocation). Only when that hash, the project (`epoch`) or the
//! installed packs (`Manager::generation`, which every install and removal bumps — the same
//! version reinstalled too) change are the references gathered and checked on disk, on a
//! worker thread; the result is taken when it is ready.

#[path = "samples_missing_card.rs"]
mod card;

use crate::app::CantusApp;
use cantus_core::inst::sampler as model;
use cantus_core::pads::{PadId, PadLayer};
use cantus_core::AudioSourceId;
use cantus_core::{Command, InstrumentKind, InstrumentSpec, Project, TrackId};
use std::collections::{BTreeMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};

pub use card::frame;

/// What plays a pack file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Owner {
    /// An Exemplar zone's `z{n}.sample`.
    Zone { track: TrackId, key: String },
    /// A Pulsus pad's own sample.
    Pad { track: TrackId, pad: PadId },
    /// One of a Pulsus pad's layers.
    Layer {
        track: TrackId,
        pad: PadId,
        index: usize,
    },
    /// An audio clip's source in the project's pool.
    Source { id: AudioSourceId },
}

/// One reference to a file inside a pack.
#[derive(Clone, Debug, PartialEq)]
pub struct Ref {
    pub owner: Owner,
    /// What the card names: the track (or, for a source no clip uses, the source).
    pub label: String,
    pub path: String,
    pub pack: String,
    /// The path inside the pack's folder.
    pub rest: String,
}

/// Why a pack's files are not found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Why {
    /// No receipt here: the pack is not installed on this computer.
    NotInstalled,
    /// Installed here, but files the project plays are not in its folder (deleted, the
    /// folder renamed, or a newer version without them).
    Incomplete,
    /// Installed here at another place than the project says (another computer): this
    /// many files can be pointed at this computer's copy.
    Elsewhere(usize),
}

/// One pack the open project needs and does not (wholly) find.
#[derive(Clone, Debug, PartialEq)]
pub struct Missing {
    pub pack: String,
    /// Its name in the interface's language (the id when no manifest or receipt names it).
    pub name: String,
    /// What plays it, in the project's order, each once.
    pub labels: Vec<String>,
    pub why: Why,
    /// How many references are not found.
    pub count: usize,
}

/// A worker's answer.
pub struct Scan {
    pub found: Vec<Missing>,
    /// Missing paths, to retry in the engine when they come back.
    pub gone: HashSet<String>,
    /// For each pack installed here: the references that this computer's copy can serve.
    pub fixes: BTreeMap<String, Vec<(Owner, String)>>,
}

/// The detector's state, kept in `Samples`.
#[derive(Default)]
pub struct State {
    key: Option<(u64, u64, u64)>,
    job: Option<Receiver<Scan>>,
    pending: bool,
    pub found: Vec<Missing>,
    fixes: BTreeMap<String, Vec<(Owner, String)>>,
    gone: HashSet<String>,
    /// Cards put away, for this project.
    dismissed: HashSet<(u64, String)>,
    /// How many scans have run (a test watches that edits do not cause any).
    pub scans: u64,
}

impl State {
    /// A scan is running or waiting.
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.pending
    }
}

/// The pack a path lies in — `<this computer's content folder>/<pack>/<rest>`, or, from
/// another computer, `…/Content/<pack>/<rest>`, with a pack the manifest or a receipt
/// knows — and the path inside it. Strings only: no disk.
pub fn pack_of(path: &str, root: &Path, known: &HashSet<String>) -> Option<(String, String)> {
    let norm = path.replace('\\', "/");
    let base = root.to_string_lossy().replace('\\', "/");
    let base = base.trim_end_matches('/');
    if let Some(rest) = norm.strip_prefix(base).and_then(|r| r.strip_prefix('/')) {
        if let Some((id, inside)) = rest.split_once('/') {
            if known.contains(id) {
                return Some((id.to_string(), inside.to_string()));
            }
        }
    }
    let parts: Vec<&str> = norm.split('/').collect();
    (0..parts.len().saturating_sub(2)).rev().find_map(|i| {
        (parts[i].eq_ignore_ascii_case("content") && known.contains(parts[i + 1]))
            .then(|| (parts[i + 1].to_string(), parts[i + 2..].join("/")))
    })
}

/// Every file path the project plays from, with what plays it and what to call it.
fn each_path(project: &Project, f: &mut dyn FnMut(Owner, &str, &str)) {
    for t in &project.tracks {
        match &t.instrument {
            InstrumentSpec::Native(n) if n.kind == InstrumentKind::Sampler => {
                for (key, path) in &n.text {
                    if key.ends_with(".sample") && model::zone_key(key).is_some() {
                        let o = Owner::Zone {
                            track: t.id,
                            key: key.clone(),
                        };
                        f(o, path, &t.name);
                    }
                }
            }
            InstrumentSpec::Pulsus(kit) => {
                for pad in &kit.pads {
                    if let Some(s) = &pad.sample {
                        let o = Owner::Pad {
                            track: t.id,
                            pad: pad.id,
                        };
                        f(o, s, &t.name);
                    }
                    for (index, l) in pad.layers.iter().enumerate() {
                        let o = Owner::Layer {
                            track: t.id,
                            pad: pad.id,
                            index,
                        };
                        f(o, &l.sample, &t.name);
                    }
                }
            }
            _ => {}
        }
    }
    for s in &project.audio_pool {
        let user = project
            .tracks
            .iter()
            .find(|t| {
                t.clips
                    .iter()
                    .any(|c| c.audio.as_ref().is_some_and(|a| a.source == s.id))
            })
            .map_or(s.name.as_str(), |t| t.name.as_str());
        f(Owner::Source { id: s.id }, &s.path, user);
    }
}

/// A hash of every path the project plays from and the names the card would use: what a
/// frame computes (no disk, no allocation) to know whether a scan is due.
pub fn fingerprint(project: &Project) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for t in &project.tracks {
        match &t.instrument {
            InstrumentSpec::Native(n) if n.kind == InstrumentKind::Sampler => {
                for (key, path) in &n.text {
                    if key.ends_with(".sample") {
                        key.hash(&mut h);
                        path.hash(&mut h);
                    }
                }
                t.name.hash(&mut h);
            }
            InstrumentSpec::Pulsus(kit) => {
                for pad in &kit.pads {
                    pad.sample.hash(&mut h);
                    for l in &pad.layers {
                        l.sample.hash(&mut h);
                    }
                }
                t.name.hash(&mut h);
            }
            _ => {}
        }
    }
    for s in &project.audio_pool {
        s.path.hash(&mut h);
    }
    h.finish()
}

/// The references to pack files (strings only).
pub fn references(project: &Project, root: &Path, known: &HashSet<String>) -> Vec<Ref> {
    let mut out = Vec::new();
    each_path(project, &mut |owner, path, label| {
        if let Some((pack, rest)) = pack_of(path, root, known) {
            out.push(Ref {
                owner,
                label: label.to_string(),
                path: path.to_string(),
                pack,
                rest,
            });
        }
    });
    out
}

/// Check `refs` on disk (on a worker thread): which packs are missing, and why.
pub fn check(
    refs: &[Ref],
    root: &Path,
    project_dir: Option<&Path>,
    installed: &HashSet<String>,
    names: &BTreeMap<String, String>,
) -> Scan {
    let mut found: Vec<Missing> = Vec::new();
    let mut gone = HashSet::new();
    let mut fixes: BTreeMap<String, Vec<(Owner, String)>> = BTreeMap::new();
    for r in refs {
        let p = Path::new(&r.path);
        let on_disk = match project_dir {
            Some(d) if p.is_relative() => d.join(p),
            _ => p.to_path_buf(),
        };
        if on_disk.exists() {
            continue;
        }
        gone.insert(r.path.clone());
        let here = installed.contains(&r.pack);
        let local = root.join(&r.pack).join(&r.rest);
        if here && local != on_disk && local.is_file() {
            fixes
                .entry(r.pack.clone())
                .or_default()
                .push((r.owner.clone(), local.to_string_lossy().replace('\\', "/")));
        }
        let i = match found.iter().position(|m| m.pack == r.pack) {
            Some(i) => i,
            None => {
                found.push(Missing {
                    pack: r.pack.clone(),
                    name: names.get(&r.pack).cloned().unwrap_or(r.pack.clone()),
                    labels: Vec::new(),
                    why: if here {
                        Why::Incomplete
                    } else {
                        Why::NotInstalled
                    },
                    count: 0,
                });
                found.len() - 1
            }
        };
        let m = &mut found[i];
        m.count += 1;
        if !m.labels.contains(&r.label) {
            m.labels.push(r.label.clone());
        }
    }
    for m in &mut found {
        if let Some(n) = fixes.get(&m.pack).map(Vec::len).filter(|n| *n > 0) {
            m.why = Why::Elsewhere(n);
        }
    }
    Scan { found, gone, fixes }
}

/// The commands that point `fixes` at their new paths, as one undo step.
pub fn relink_commands(project: &Project, fixes: &[(Owner, String)]) -> Option<Command> {
    let mut commands = Vec::new();
    let mut layers: BTreeMap<(TrackId, PadId), Vec<PadLayer>> = BTreeMap::new();
    for (owner, path) in fixes {
        match owner {
            Owner::Zone { track, key } => commands.push(Command::SetInstrumentText {
                track: *track,
                key: key.clone(),
                text: Some(path.clone()),
            }),
            Owner::Pad { track, pad } => commands.push(Command::SetPadSample {
                track: *track,
                pad: *pad,
                path: Some(path.clone()),
            }),
            Owner::Layer { track, pad, index } => {
                let now = project
                    .track(*track)
                    .and_then(|t| t.instrument.pulsus())
                    .and_then(|k| k.pads.iter().find(|p| p.id == *pad))
                    .map(|p| p.layers.clone());
                let Some(now) = now else { continue };
                let l = layers.entry((*track, *pad)).or_insert(now);
                if let Some(layer) = l.get_mut(*index) {
                    layer.sample = path.clone();
                }
            }
            Owner::Source { id } => commands.push(Command::SetAudioSourcePath {
                id: *id,
                path: path.clone(),
            }),
        }
    }
    for ((track, pad), layers) in layers {
        commands.push(Command::SetPadLayers { track, pad, layers });
    }
    (!commands.is_empty()).then(|| Command::Batch {
        label_key: "cmd.samples_relink".into(),
        commands,
    })
}

/// Point every missing file of `pack` that `folder` holds at it, as one undo step (*Find
/// the folder…*). Runs on the click that asks for it.
pub fn relink_to(app: &mut CantusApp, pack: &str, folder: &Path) -> bool {
    let known: HashSet<String> = std::iter::once(pack.to_string()).collect();
    let root = app.samples.content().root().to_path_buf();
    let fixes: Vec<(Owner, String)> = references(app.project(), &root, &known)
        .into_iter()
        .filter(|r| !Path::new(&r.path).exists())
        .filter_map(|r| {
            let p = folder.join(&r.rest);
            p.is_file()
                .then(|| (r.owner, p.to_string_lossy().replace('\\', "/")))
        })
        .collect();
    let Some(cmd) = relink_commands(app.project(), &fixes) else {
        return false;
    };
    let done = app.exec(cmd);
    app.samples.missing.key = None;
    done
}

/// Point the files this computer's copy of `pack` holds at it (*Use the installed pack*,
/// from the last scan's answer).
pub fn relink(app: &mut CantusApp, pack: &str) -> bool {
    let fixes = app
        .samples
        .missing
        .fixes
        .get(pack)
        .cloned()
        .unwrap_or_default();
    let Some(cmd) = relink_commands(app.project(), &fixes) else {
        return false;
    };
    let done = app.exec(cmd);
    app.samples.missing.key = None;
    done
}

/// Take a finished scan, and start one when the project, its sample paths or the
/// installed packs changed. A frame pays a hash of the paths; never the disk.
pub fn refresh(app: &mut CantusApp) {
    if let Some(rx) = &app.samples.missing.job {
        match rx.try_recv() {
            Ok(scan) => {
                app.samples.missing.job = None;
                take(app, scan);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => app.samples.missing.job = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }
    let generation = app.samples.content().generation();
    let key = (app.epoch, generation, fingerprint(app.project()));
    if app.samples.missing.key != Some(key) {
        app.samples.missing.key = Some(key);
        app.samples.missing.pending = true;
    }
    if app.samples.missing.pending && app.samples.missing.job.is_none() {
        app.samples.missing.pending = false;
        start(app);
    }
}

/// Gather the references (strings) and check them on a worker thread.
fn start(app: &mut CantusApp) {
    let lang = crate::i18n::locale().code();
    let c = app.samples.content();
    let mut names: BTreeMap<String, String> = c
        .manifest()
        .packs
        .iter()
        .map(|p| (p.id.clone(), p.name.get(lang).to_string()))
        .collect();
    for r in c.installed() {
        names
            .entry(r.id.clone())
            .or_insert_with(|| r.name.get(lang).to_string());
    }
    let installed: HashSet<String> = c.installed().iter().map(|r| r.id.clone()).collect();
    let root = c.root().to_path_buf();
    let known: HashSet<String> = names.keys().cloned().collect();
    let refs = references(app.project(), &root, &known);
    let project_dir: Option<PathBuf> = app
        .path
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf);
    app.samples.missing.scans += 1;
    let (tx, rx) = channel();
    let spawned = std::thread::Builder::new()
        .name("cantus-missing-packs".into())
        .spawn(move || {
            let scan = check(&refs, &root, project_dir.as_deref(), &installed, &names);
            let _ = tx.send(scan);
        });
    if spawned.is_ok() {
        app.samples.missing.job = Some(rx);
    }
}

/// Keep a scan's answer; files that came back are tried again by the engine.
fn take(app: &mut CantusApp, scan: Scan) {
    let back: Vec<String> = app
        .samples
        .missing
        .gone
        .iter()
        .filter(|p| !scan.gone.contains(*p))
        .cloned()
        .collect();
    if !back.is_empty() {
        for p in &back {
            app.engine.samples.retry(Path::new(p));
        }
        app.resync_engine();
    }
    let st = &mut app.samples.missing;
    st.found = scan.found;
    st.gone = scan.gone;
    st.fixes = scan.fixes;
}

/// Put a pack's card away for this project.
pub fn dismiss(app: &mut CantusApp, pack: &str) {
    let epoch = app.epoch;
    app.samples
        .missing
        .dismissed
        .insert((epoch, pack.to_string()));
}

/// The cards to show now.
pub fn shown(app: &CantusApp) -> Vec<Missing> {
    let st = &app.samples.missing;
    st.found
        .iter()
        .filter(|m| !st.dismissed.contains(&(app.epoch, m.pack.clone())))
        .cloned()
        .collect()
}
