//! Freezing and bouncing in place, the interface's side (round 29, ADR 0064): the renders
//! made on a worker — one at a time, with their progress and their Stop — what a refused
//! edit says, the folders renders go in, and the renders a frozen project still needs
//! (opened elsewhere, another rate) made again in the background while it plays live.
//!
//! **Freeze** renders first and freezes after: nothing changes while the render runs, so
//! Stop leaves the project as it was. When the render is done the freeze is one undo step —
//! with the hosted plug-ins' states the render heard written into the project first (their
//! instances are then unloaded, `cantus_host` parks them). If the project changed what the
//! target holds meanwhile, the render is made again for what it holds now.
//!
//! A render belongs to the project it was asked in (review 1): another project opened
//! drops what was running or waiting for the last one.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::freeze::{self, CannotFreeze, FreezeTarget};
use cantus_core::{Command, Project, Snapshot, StripId};
use cantus_engine::render::{RenderError, RenderProgress};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the project is looked at for frozen targets whose render is not at hand.
const SCAN: Duration = Duration::from_millis(500);
/// The renders kept in the application's folder, at most (the oldest go first; they are
/// made again when wanted).
const APP_CACHE_BYTES: u64 = 4 << 30;

/// What a render is for.
#[derive(Clone, Debug, PartialEq)]
pub enum Want {
    /// The user froze it: freeze when the render is done.
    Freeze(FreezeTarget),
    /// It is frozen and its render is not at hand: make it (it plays live meanwhile).
    Refresh(FreezeTarget),
    /// A bounce in place (`crate::bounce`).
    Bounce(crate::bounce::Plan),
}

/// The render running.
pub struct Job {
    pub want: Want,
    /// What it is called on screen.
    pub name: String,
    pub progress: Arc<RenderProgress>,
    /// The content key it renders (a freeze's, a refresh's).
    content: String,
    /// The commands that go first in the freeze's undo step (hosted states).
    before: Vec<Command>,
    /// The edits made in its region's plug-ins' windows when it started
    /// (`PluginHost::edits_of`): another by its end renders it again (review 3: the states'
    /// bytes were compared, which automation moves, and it never landed).
    edits: u64,
    /// The bypassed plug-ins it was rendered without (not installed: a wire stood in),
    /// said beside its render so it is made again once they are (review 3).
    wires: Vec<(cantus_core::PluginFormat, String)>,
    worker: Option<JoinHandle<Result<PathBuf, RenderError>>>,
    /// A bounce's plan, filled in with its file when done.
    bounce: Option<crate::bounce::Prepared>,
    /// The project it was asked in (`CantusApp::epoch`).
    epoch: u64,
    /// A warped clip's render was late at its compile: said as it lands (warp, round 29's
    /// third review).
    late: bool,
    /// The plug-ins whose state it took as they were last said (not to be had as they are
    /// now): said with its outcome (review isolation-6 #3, #15, #25).
    stale: Vec<cantus_host::RenderFailure>,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job").field("want", &self.want).finish()
    }
}

/// The interface's freezing state (`CantusApp::freeze`).
#[derive(Debug, Default)]
pub struct FreezeState {
    pub job: Option<Job>,
    queue: VecDeque<Want>,
    /// Renders that failed, by content key, and why: not tried again this session.
    failed: HashMap<String, String>,
    /// Background renders the user stopped, by content key: not made again until a freeze
    /// or an unfreeze (review 1: Stop could not stop a refresh).
    stopped: HashSet<String>,
    scanned: Option<Instant>,
    /// The project path the store's folder was last set for.
    dir_for: Option<Option<PathBuf>>,
    /// The low-latency monitoring the engine was last told (limit in samples, or off).
    told: Option<Option<u32>>,
    /// The last edit a freeze refused, and when (the corner card's).
    pub refusal: Option<(FreezeTarget, Instant)>,
    /// The project the queue and the job are for (`CantusApp::epoch`).
    epoch: Option<u64>,
    /// The session is closing: nothing starts any more (review 2).
    closed: bool,
    /// The freeze whose plug-ins' states, as they are now, were asked for: it starts once
    /// they are in (review isolation-6 #3, #15, #25).
    asked: Option<Want>,
    /// Refreshes said to wait for a file (once each).
    said_waiting: HashSet<String>,
    /// A save's copies and prune, done when set (review 3: off the frame).
    pub copying: Option<Arc<std::sync::atomic::AtomicBool>>,
    /// The thread copying and pruning: one at a time, a later save's queued behind it, and
    /// waited for when the session closes (`finish_saving`, review 4).
    saver: Option<JoinHandle<()>>,
    /// The renders the session wants now, by file name: a save's prune never removes one
    /// (review 4: a prune made with its save's list removed what a later save wanted).
    protect: Arc<std::sync::Mutex<HashSet<String>>>,
    /// Tests: the save's thread waits before its copies while this is set.
    #[doc(hidden)]
    pub hold_copies: Arc<std::sync::atomic::AtomicBool>,
}

impl FreezeState {
    /// Whether a render is running or waiting.
    pub fn busy(&self) -> bool {
        self.job.is_some() || !self.queue.is_empty()
    }

    /// The running render's name and how far it is (0…1).
    pub fn progress(&self) -> Option<(&str, f32)> {
        self.job
            .as_ref()
            .map(|j| (j.name.as_str(), j.progress.fraction()))
    }

    /// How many renders wait after the running one.
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    /// What waits while nothing runs (behind an export, a bake, files loading): its name
    /// for the card, which offers Stop (review 2: it waited unseen).
    pub fn waiting(&self) -> Option<&Want> {
        self.job.is_none().then(|| self.queue.front()).flatten()
    }

    /// Whether the running render is a background refresh (an export may stop it).
    pub fn refreshing(&self) -> bool {
        self.job
            .as_ref()
            .is_some_and(|j| matches!(j.want, Want::Refresh(_)))
    }
}

/// Where renders go when the project has no folder (and where they are always looked for).
pub fn app_dir() -> PathBuf {
    crate::app::data_dir().join("cache").join("freeze")
}

/// The project's own folder for renders: `Freeze/<its file's name>` beside it — one per
/// project, so a save prunes only its own (review 3: projects side by side shared one
/// folder, and saving one deleted the other's renders).
pub fn project_dir(path: Option<&Path>) -> Option<PathBuf> {
    let p = path?;
    let stem = p.file_stem()?.to_string_lossy().into_owned();
    Some(p.with_file_name("Freeze").join(stem))
}

/// The folder an older layout kept every project's renders in, side by side: looked in,
/// never pruned.
fn shared_dir(path: Option<&Path>) -> Option<PathBuf> {
    path.map(|p| p.with_file_name("Freeze"))
}

/// A target's name on screen.
pub fn name_of(project: &Project, target: FreezeTarget) -> String {
    match target {
        FreezeTarget::Track(t) => project.track(t).map(|t| t.name.clone()).unwrap_or_default(),
        FreezeTarget::Strip(s) => match project.strip(s) {
            Some(st) if !st.name.is_empty() => st.name.clone(),
            Some(_) => project
                .tracks_on_strip(s)
                .next()
                .map(|t| t.name.clone())
                .unwrap_or_else(|| {
                    tf(
                        "freeze.strip_n",
                        &[("n", &project.strip_number(s).unwrap_or(0))],
                    )
                }),
            None => String::new(),
        },
    }
}

/// Wired at start (`CantusApp::assemble`): the store's folders, low-latency monitoring as
/// the settings say; what a session cut short left half written is cleared, and the
/// application's folder kept to its size.
pub fn wire(app: &mut CantusApp) {
    let dir = app_dir();
    tidy(&dir);
    prune(&dir, APP_CACHE_BYTES);
    // The scratch folders recordings and bounces of unsaved projects go to.
    if let Ok(rd) = std::fs::read_dir(crate::app::data_dir().join("recordings")) {
        for e in rd.flatten() {
            tidy_bounces(&e.path());
        }
    }
    app.engine.frozen.set_app_dir(dir);
    app.freeze.epoch = Some(app.epoch);
    follow_path(app);
    apply_low_latency(app);
}

/// The rates a render is kept for when its project's folder is pruned (one is made again
/// at another).
const RATES: [u32; 6] = [44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

/// A project was saved (`poll_save`): its own folder gets the renders its frozen targets
/// play and those an undo or a redo would want — saved as elsewhere, they come along
/// (review 2: the reopened project rendered them all again) — and keeps no other (review
/// 2: every freeze, edit and refreeze left a full render there). Its folder is its own
/// (review 3: a shared one lost other projects' renders); the renders a freeze running or
/// waiting will want are kept too, and the copies and the prune are made on a thread of
/// their own (review 3: a first save copied gigabytes on the frame), the store looking in
/// the old folder meanwhile.
pub(crate) fn saved(app: &mut CantusApp) {
    let Some(dir) = project_dir(app.path.as_deref()) else {
        return;
    };
    let rate = app.engine.sample_rate.round() as u32;
    let mut keep: std::collections::BTreeSet<String> = app
        .project()
        .frozen_targets()
        .into_iter()
        .filter_map(|t| app.project().frozen(t).map(|f| f.content.clone()))
        .collect();
    for e in app.bus.history().iter().chain(app.bus.redo_history()) {
        freeze::keys_in(&e.command, &mut keep);
        freeze::keys_in(&e.inverse, &mut keep);
    }
    keep.extend(wanted_by_jobs(app));
    // Where each wanted render is now, if not in its folder yet.
    let copies: Vec<Copy> = keep
        .iter()
        .filter_map(|content| {
            let name =
                cantus_engine::freeze::file_name(&cantus_engine::freeze::render_key(content, rate));
            let to = dir.join(&name);
            if to.is_file() {
                return None;
            }
            let from = app.engine.frozen.path_of(content, rate)?;
            Some(Copy {
                from,
                to,
                key: None,
            })
        })
        .collect();
    let wanted: HashSet<String> = keep
        .iter()
        .flat_map(|c| {
            RATES.iter().chain(std::iter::once(&rate)).map(|r| {
                cantus_engine::freeze::file_name(&cantus_engine::freeze::render_key(c, *r))
            })
        })
        .collect();
    // Nothing to bring and no folder to prune: nothing to do (a project with nothing
    // frozen gets no folder).
    if copies.is_empty() && !dir.is_dir() {
        return;
    }
    // What the session wants is what no prune removes — this save's, and a slower one
    // still running behind it (review 4).
    if let Ok(mut p) = app.freeze.protect.lock() {
        *p = wanted;
    }
    // The prune removes only what the folder holds now: a render landing while the copies
    // run is never in it (review 4).
    let seen = listed(&dir);
    run_copies(app, dir, copies, Some(seen));
    app.engine.frozen.forget_missing();
}

/// A render to copy into a project's folder: under its own key, or (`key`) under the key
/// its sound moved to (a file moved by saving).
#[derive(Debug)]
struct Copy {
    from: PathBuf,
    to: PathBuf,
    key: Option<String>,
}

/// The renders and their wires notes in `dir` now, by file name.
fn listed(dir: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x == cantus_engine::freeze::EXTENSION || x == "wires")
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect()
}

/// Beside a project's renders while they are copied in: the folders they come from, one a
/// line — a session cut short looks there (review 4: a quit killed the copy, and the next
/// session made every render again).
fn from_note(dir: &Path) -> PathBuf {
    dir.join("copying-from.txt")
}

/// The folders a project's renders folder notes it is copying from.
fn noted_from(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_to_string(from_note(dir))
        .map(|t| {
            t.lines()
                .filter(|l| !l.trim().is_empty())
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default()
}

/// `copies` into `dir` and then (`prune`: the folder's listing at the save) every render
/// listed there that the session does not want removed — on a thread of its own, behind
/// the one before it (review 4: two ran at once on one folder), the source folders noted
/// until the copies are in.
fn run_copies(app: &mut CantusApp, dir: PathBuf, copies: Vec<Copy>, prune: Option<Vec<String>>) {
    let app_dir = app.engine.frozen.app_dir().map(Path::to_path_buf);
    let mut from: Vec<PathBuf> = copies
        .iter()
        .filter_map(|c| c.from.parent().map(Path::to_path_buf))
        .filter(|d| *d != dir && Some(d) != app_dir.as_ref())
        .collect();
    from.sort();
    from.dedup();
    if !from.is_empty() {
        let _ = std::fs::create_dir_all(&dir);
        let mut text: String = noted_from(&dir)
            .into_iter()
            .filter(|d| !from.contains(d))
            .map(|d| format!("{}\n", d.display()))
            .collect();
        for d in &from {
            text.push_str(&format!("{}\n", d.display()));
        }
        let _ = std::fs::write(from_note(&dir), text);
    }
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = done.clone();
    let before = app.freeze.saver.take();
    let protect = app.freeze.protect.clone();
    let hold = app.freeze.hold_copies.clone();
    let project_file = app.path.clone();
    let spawned = std::thread::Builder::new()
        .name("cantus-freeze-save".into())
        .spawn(move || {
            if let Some(b) = before {
                let _ = b.join();
            }
            while hold.load(std::sync::atomic::Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(5));
            }
            copy_all(&dir, &copies);
            if let Some(seen) = prune {
                prune_seen(&dir, &seen, &protect);
                if let Some(file) = project_file.as_deref() {
                    tidy_orphans(&dir, file);
                }
            }
            if copies.iter().all(|c| c.to.is_file()) {
                let _ = std::fs::remove_file(from_note(&dir));
            }
            flag.store(true, std::sync::atomic::Ordering::Release);
        });
    match spawned {
        Ok(h) => {
            app.freeze.saver = Some(h);
            app.freeze.copying = Some(done);
        }
        Err(e) => log::warn!("the renders' copy: {e}"),
    }
}

/// Copy each render in (written beside, then renamed: never a half file under a render's
/// name), and its wires note with it (review 4: it stayed behind, and the render was never
/// made again once the plug-in was installed).
fn copy_all(dir: &Path, copies: &[Copy]) {
    for c in copies {
        if c.to.is_file() {
            continue;
        }
        let _ = std::fs::create_dir_all(dir);
        let copied = match &c.key {
            Some(key) => cantus_engine::freeze::copy_as(&c.from, &c.to, key).is_ok(),
            None => copy_part(&c.from, &c.to),
        };
        let wires = wires_file(&c.from);
        if copied && wires.is_file() {
            copy_part(&wires, &wires_file(&c.to));
        }
    }
}

/// `from` copied to `to` through a part file renamed when whole.
fn copy_part(from: &Path, to: &Path) -> bool {
    let part = to.with_extension(format!(
        "{}.copy.part",
        to.extension().and_then(|x| x.to_str()).unwrap_or("")
    ));
    let ok = std::fs::copy(from, &part).is_ok() && std::fs::rename(&part, to).is_ok();
    if !ok {
        let _ = std::fs::remove_file(&part);
    }
    ok
}

/// The renders `seen` in `dir` at the save that the session does not want now, removed,
/// with their wires notes; a wires note whose render is gone, too.
fn prune_seen(dir: &Path, seen: &[String], protect: &std::sync::Mutex<HashSet<String>>) {
    let wanted = protect.lock().map(|p| p.clone()).unwrap_or_default();
    let ext = format!(".{}", cantus_engine::freeze::EXTENSION);
    for name in seen.iter().filter(|n| n.ends_with(&ext)) {
        if !wanted.contains(name) {
            let path = dir.join(name);
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(wires_file(&path));
        }
    }
    for name in seen.iter().filter(|n| n.ends_with(".wires")) {
        let path = dir.join(name);
        if !path
            .with_extension(cantus_engine::freeze::EXTENSION)
            .is_file()
        {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// The renders folders beside `dir` (in the same `Freeze` folder) of projects no longer
/// there — renamed, moved: what `dir` now holds too goes from them, and an emptied one
/// goes (review 4: they stayed for ever). A folder whose project is there is never
/// touched.
fn tidy_orphans(dir: &Path, project_file: &Path) {
    let (Some(root), Some(beside), Some(ext)) = (
        dir.parent(),
        project_file.parent(),
        project_file.extension().and_then(|x| x.to_str()),
    ) else {
        return;
    };
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for e in rd.flatten() {
        let other = e.path();
        if !other.is_dir() || other == dir {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        if beside.join(format!("{name}.{ext}")).exists() {
            continue;
        }
        for f in std::fs::read_dir(&other).into_iter().flatten().flatten() {
            if dir.join(f.file_name()).is_file() {
                let _ = std::fs::remove_file(f.path());
            }
        }
        let _ = std::fs::remove_dir(&other);
    }
}

/// Wait up to `patience` for the save's copies to be in (the session closing: a quit
/// would end them halfway — review 4). Whether they are.
pub fn finish_saving(app: &mut CantusApp, patience: Duration) -> bool {
    let t0 = Instant::now();
    while let Some(h) = app.freeze.saver.as_ref() {
        if h.is_finished() {
            if let Some(h) = app.freeze.saver.take() {
                let _ = h.join();
            }
            return true;
        }
        if t0.elapsed() >= patience {
            log::warn!("the frozen renders were still being copied at the close");
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    true
}

/// The renders the running and the waiting freezes will want (a save must not prune them:
/// review 3) — keyed as each will be frozen (disarmed, the plug-ins' states in: review 4).
fn wanted_by_jobs(app: &mut CantusApp) -> Vec<String> {
    let mut out: Vec<String> = app
        .freeze
        .job
        .iter()
        .map(|j| j.content.clone())
        .filter(|c| !c.is_empty())
        .collect();
    let queue: Vec<Want> = app.freeze.queue.iter().cloned().collect();
    for w in queue {
        match w {
            Want::Freeze(t) => {
                let before = hosted_states(app, t);
                if let Ok((_, c)) = as_frozen(app.project(), &before, t) {
                    out.push(c);
                }
            }
            Want::Refresh(t) => {
                out.extend(app.project().frozen(t).map(|f| f.content.clone()));
                out.extend(freeze::content_key(app.project(), t));
            }
            Want::Bounce(_) => {}
        }
    }
    out
}

/// A bounce's half-written files left in an audio folder by a session cut short (review 2:
/// only the renders' folders were tidied): `* bounce*.wav.part` and `* bounce*.raw.part`,
/// over a minute old.
pub fn tidy_bounces(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|d| d > Duration::from_secs(60));
        if old
            && name.contains(" bounce")
            && (name.ends_with(".wav.part") || name.ends_with(".raw.part"))
        {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Partial files a render cut short (a quit, a crash) left in `dir`: `*.part*` (review 1).
/// Only those older than a minute: one being written now is never touched.
pub fn tidy(dir: &Path) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|d| d > Duration::from_secs(60));
        if old && (name.contains(".part") || name.ends_with(".raw.part")) {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The renders in `dir` beyond `limit` bytes, the least recently written first, removed
/// (they are made again when a frozen project wants them).
pub fn prune(dir: &Path, limit: u64) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x == cantus_engine::freeze::EXTENSION)
        })
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort();
    for (_, len, path) in files {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// The store follows the project's folder (opened, saved as) — before the first compile
/// of a project opened (`CantusApp::sync_engine`, review 1: the previous project's folder
/// was looked in, and every frozen chain's plug-ins loaded for a frame).
pub(crate) fn follow_path(app: &mut CantusApp) {
    // Compared as it is: nothing is allocated while the path stays.
    if app.freeze.dir_for.as_ref() != Some(&app.path) {
        let dir = project_dir(app.path.as_deref());
        if let Some(d) = &dir {
            tidy(d);
        }
        if let Some(audio) = app.path.as_deref().map(|p| p.with_file_name("Audio")) {
            tidy_bounces(&audio);
        }
        app.engine.frozen.set_also(also_dirs(app.path.as_deref()));
        app.engine.frozen.set_project_dir(dir);
        app.freeze.dir_for = Some(app.path.clone());
        app.force_resync();
    }
}

/// The folders a project's renders are also looked for in, never written to (review 4):
/// those its folder notes it is still copying from (a *Save as* a quit cut short); the
/// renders folders of the other projects beside it — a render's name is its sound, so a
/// project renamed or copied finds its renders in its old name's folder, and its next save
/// brings them in; and the folder an older layout shared.
fn also_dirs(path: Option<&Path>) -> Vec<PathBuf> {
    let Some(dir) = project_dir(path) else {
        return Vec::new();
    };
    let mut also = noted_from(&dir);
    if let Some(root) = dir.parent() {
        let mut siblings: Vec<PathBuf> = std::fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir() && *p != dir)
            .collect();
        siblings.sort();
        also.extend(siblings);
    }
    also.extend(shared_dir(path));
    also
}

/// Tell the engine what low-latency monitoring the settings ask for, when it changed (or
/// the engine is new: `again`).
pub fn apply_low_latency(app: &mut CantusApp) {
    let want = app.settings.low_latency_samples(app.engine.sample_rate);
    if app.freeze.told != Some(want) {
        app.freeze.told = Some(want);
        app.engine
            .send(cantus_engine::EngineMessage::SetLowLatency(want));
    }
}

/// A new engine (another device or rate) has never been told.
pub fn engine_replaced(app: &mut CantusApp) {
    app.freeze.told = None;
    apply_low_latency(app);
    app.engine.frozen.set_app_dir(app_dir());
    app.freeze.dir_for = None;
    follow_path(app);
}

/// Once a frame: the store's folder, the running render collected, the next one started,
/// and the frozen targets whose render is missing queued.
pub fn tick(app: &mut CantusApp, ctx: &egui::Context) {
    if app.freeze.closed {
        return;
    }
    follow_path(app);
    apply_low_latency(app);
    if app.freeze.epoch.is_none() {
        app.freeze.epoch = Some(app.epoch);
    }
    if app.freeze.epoch != Some(app.epoch) {
        // Another project: what was asked for the last one is not this one's.
        app.freeze.epoch = Some(app.epoch);
        app.freeze.queue.clear();
        app.freeze.failed.clear();
        app.freeze.stopped.clear();
        app.freeze.refusal = None;
        if let Some(j) = &app.freeze.job {
            j.progress.cancel();
            app.plugins.cancel_render();
        }
    }
    collect(app);
    if app.freeze.scanned.is_none_or(|t| t.elapsed() >= SCAN) {
        app.freeze.scanned = Some(Instant::now());
        scan(app);
    }
    if app.freeze.job.is_none() {
        start_next(app);
    }
    if app.freeze.busy() {
        crate::views::workspace_ticker::wake_after(ctx, Duration::from_millis(50));
    }
}

/// Queue a refresh for every frozen target whose render is not at hand (once each).
fn scan(app: &mut CantusApp) {
    let rate = app.engine.sample_rate.round() as u32;
    // The project's renders kept open whatever plays (pattern mode plays every chain
    // live: leaving it found them closed — review 3).
    let contents: Vec<String> = app
        .project()
        .frozen_targets()
        .into_iter()
        .filter_map(|t| app.project().frozen(t).map(|f| f.content.clone()))
        .collect();
    app.engine.frozen.hold(&contents, rate);
    if app.project().pattern_mode {
        return;
    }
    // A render that failed while the live snapshot held it: compile again, so its target
    // plays live while it is made again (review 2: it played silence meanwhile).
    if app.engine.frozen.take_lost() {
        app.force_resync();
    }
    if !app.project().any_frozen() {
        return;
    }
    // What rests inside a frozen region that plays is not heard: made when it wakes
    // (review 2: its plug-ins were unloaded, and it was marked failed for the session).
    let project = app.project().clone();
    let resting = app.engine.frozen.dormant(&project, rate);
    for target in project.frozen_targets() {
        let asleep = match target {
            FreezeTarget::Strip(s) => resting.strips.contains(&s),
            FreezeTarget::Track(t) => resting.tracks.contains(&t) || resting.notes.contains(&t),
        };
        if asleep {
            continue;
        }
        let Some(content) = project.frozen(target).map(|f| f.content.clone()) else {
            continue;
        };
        // Made with a wire for a bypassed plug-in installed since: its latency is
        // missing from it — made again (review 3), whichever copy says so: every copy set
        // aside, those in its own folders removed (review 4: a save's copy had no note).
        let copies = app.engine.frozen.paths_of(&content, rate);
        if copies.iter().any(|p| wired_since_installed(app, p)) {
            app.engine.frozen.set_aside(&content, rate);
            let own = [
                app.engine.frozen.project_dir().map(Path::to_path_buf),
                app.engine.frozen.app_dir().map(Path::to_path_buf),
            ];
            for p in &copies {
                if own
                    .iter()
                    .flatten()
                    .any(|d| p.parent() == Some(d.as_path()))
                {
                    let _ = std::fs::remove_file(wires_file(p));
                    let _ = std::fs::remove_file(p);
                }
            }
            app.force_resync();
        }
        if app.freeze.failed.contains_key(&content)
            || app.freeze.stopped.contains(&content)
            || app.engine.frozen.find(&content, rate).is_some()
        {
            continue;
        }
        let want = Want::Refresh(target);
        let queued = app.freeze.queue.contains(&want)
            || app.freeze.job.as_ref().is_some_and(|j| j.want == want);
        if !queued {
            app.freeze.queue.push_back(want);
        }
    }
    // A render the finds above gave up on (its stream failed): compile again now, not a
    // scan later (review 3).
    if app.engine.frozen.take_lost() {
        app.force_resync();
    }
}

/// Queue a render (a bounce's: `crate::bounce`) and start it if nothing runs — once: the
/// same one running or waiting is not asked again (review 1: a bounce queued twice laid
/// two tracks that both played).
pub(crate) fn queue(app: &mut CantusApp, want: Want) {
    app.freeze.epoch.get_or_insert(app.epoch);
    let running = app.freeze.job.as_ref().is_some_and(|j| j.want == want);
    if !running && !app.freeze.queue.contains(&want) {
        app.freeze.queue.push_back(want);
    }
    start_next(app);
}

impl CantusApp {
    /// Freeze `target` (Freeze on a track or a strip): its render made on a worker, then
    /// frozen in one undo step. Says why when it cannot be.
    pub fn freeze_target(&mut self, target: FreezeTarget) {
        if self.project().frozen(target).is_some() {
            return;
        }
        let want = Want::Freeze(target);
        if self.freeze.queue.contains(&want)
            || self.freeze.job.as_ref().is_some_and(|j| j.want == want)
        {
            return;
        }
        // Said now, not when its turn comes.
        if let Err(why) = freeze::freeze_commands(self.project(), target) {
            self.say_cannot(target, why);
            return;
        }
        self.freeze.stopped.clear();
        self.freeze.epoch.get_or_insert(self.epoch);
        self.freeze.queue.push_back(want);
        start_next(self);
    }

    /// Unfreeze `target`: its live chain back, one undo step.
    pub fn unfreeze_target(&mut self, target: FreezeTarget) {
        if self.project().frozen(target).is_none() {
            return;
        }
        self.freeze.stopped.clear();
        if self.exec(freeze::unfreeze_command(target)) {
            self.set_status(tf(
                "freeze.unfrozen",
                &[("name", &name_of(self.project(), target))],
            ));
        }
    }

    /// The frozen target that holds `track` (its own, or a strip it feeds), if any.
    pub fn freeze_holding_track(&self, track: cantus_core::TrackId) -> Option<FreezeTarget> {
        let p = self.project();
        if !p.any_frozen() {
            return None;
        }
        freeze::Locks::of(p).owner_of_track(track)
    }

    /// The frozen target that holds strip `strip` (itself, or one it feeds), if any.
    pub fn freeze_holding_strip(&self, strip: cantus_core::StripId) -> Option<FreezeTarget> {
        let p = self.project();
        if !p.any_frozen() {
            return None;
        }
        freeze::Locks::of(p).owner_of_strip(strip)
    }

    /// Freeze or unfreeze a track (the track menu's entry, its key): unfreeze what holds
    /// it, else freeze its own strip or its source.
    pub fn toggle_freeze_track(&mut self, track: cantus_core::TrackId) {
        match self.freeze_holding_track(track) {
            Some(target) => self.unfreeze_target(target),
            None => {
                if let Some(target) = self.project().freeze_target_of_track(track) {
                    self.freeze_target(target);
                }
            }
        }
    }

    /// Freeze or unfreeze a strip (the strip menu's, the key with a strip chosen).
    pub fn toggle_freeze_strip(&mut self, strip: cantus_core::StripId) {
        match self.freeze_holding_strip(strip) {
            Some(target) => self.unfreeze_target(target),
            None => self.freeze_target(FreezeTarget::Strip(strip)),
        }
    }

    /// Stop the running render (nothing is frozen or bounced) and what waits after it —
    /// background refreshes too: those are not made again until the user freezes or
    /// unfreezes something (their targets play live).
    pub fn cancel_freeze(&mut self) {
        // The plug-ins a waiting render had prepared go with it (review isolation-6 #1).
        self.freeze.asked = None;
        self.plugins.drop_render(cantus_host::RenderFor::Freeze);
        for w in std::mem::take(&mut self.freeze.queue) {
            if let Want::Refresh(target) = w {
                if let Some(f) = self.project().frozen(target) {
                    self.freeze.stopped.insert(f.content.clone());
                }
            }
        }
        if let Some(j) = &self.freeze.job {
            if matches!(j.want, Want::Refresh(_)) {
                self.freeze.stopped.insert(j.content.clone());
            }
            j.progress.cancel();
            // An isolated plug-in hung in the render is not waited for (round 29, review #1).
            self.plugins.cancel_render();
        }
    }

    fn say_cannot(&mut self, target: FreezeTarget, why: CannotFreeze) {
        let name = name_of(self.project(), target);
        let track_name = |p: &Project, t| p.track(t).map(|t| t.name.clone()).unwrap_or_default();
        let text = match why {
            CannotFreeze::Gone => return,
            CannotFreeze::Frozen => tf("freeze.already", &[("name", &name)]),
            CannotFreeze::Monitoring(t) => tf(
                "freeze.cannot_monitoring",
                &[("name", &name), ("track", &track_name(self.project(), t))],
            ),
            CannotFreeze::SoloedTrack(t) => tf(
                "freeze.cannot_solo",
                &[("name", &name), ("what", &track_name(self.project(), t))],
            ),
            CannotFreeze::SoloedStrip(s) => tf(
                "freeze.cannot_solo",
                &[
                    ("name", &name),
                    ("what", &name_of(self.project(), FreezeTarget::Strip(s))),
                ],
            ),
        };
        self.set_error(text);
    }
}

/// Stop a running background refresh at once and let its plug-ins go (an export about to
/// start: it does not need it — the target plays live in it). Whether nothing runs now.
pub fn yield_refresh(app: &mut CantusApp) -> bool {
    if !app.freeze.refreshing() {
        return app.freeze.job.is_none();
    }
    if let Some(mut j) = app.freeze.job.take() {
        j.progress.cancel();
        app.plugins.cancel_render();
        if let Some(w) = j.worker.take() {
            let _ = w.join();
        }
        app.plugins.finish_render();
        // Asked again after the export (the next scan).
    }
    true
}

/// `commands` run as one undo step `label_key` through the frozen targets whose key they
/// move (`freeze::rekeyed`: a file moved by saving, a missing file found again). `keep`:
/// the sound is the same file's — each render is copied under its new key into `dir`.
/// Whether it ran.
pub fn exec_through(
    app: &mut CantusApp,
    label_key: &str,
    commands: Vec<Command>,
    keep: Option<&Path>,
) -> bool {
    if !app.project().any_frozen() {
        return app.exec(Command::Batch {
            label_key: label_key.into(),
            commands,
        });
    }
    let (batch, moved) = freeze::rekeyed(app.project(), label_key, commands);
    if !app.exec(batch) {
        return false;
    }
    let rate = app.engine.sample_rate.round() as u32;
    if let Some(dir) = keep {
        // Copied on the save's thread, named as the new key inside (review 4: a plain
        // copy on the frame kept the old key and never opened); the old render plays under
        // the new key meanwhile.
        let mut copies = Vec::new();
        for (_, old, new) in &moved {
            if let Some(from) = app.engine.frozen.path_of(old, rate) {
                let key = cantus_engine::freeze::render_key(new, rate);
                let name = cantus_engine::freeze::file_name(&key);
                app.engine.frozen.alias(new, old, rate);
                if let Ok(mut p) = app.freeze.protect.lock() {
                    p.insert(name.clone());
                }
                copies.push(Copy {
                    from,
                    to: dir.join(name),
                    key: Some(key),
                });
            }
        }
        if !copies.is_empty() {
            run_copies(app, dir.to_path_buf(), copies, None);
        }
        app.engine.frozen.forget_missing();
    }
    true
}

/// An edit a frozen target refused (`CantusApp::exec`): the status line says which, and
/// how to give it back.
pub fn refused(app: &mut CantusApp, target: FreezeTarget) {
    app.freeze.refusal = Some((target, Instant::now()));
    let name = name_of(app.project(), target);
    let how = match target {
        FreezeTarget::Track(_) => t("freeze.how_track"),
        FreezeTarget::Strip(_) => t("freeze.how_strip"),
    };
    app.set_error(tf("freeze.refused", &[("name", &name), ("how", &how)]));
}

/// The plug-ins the last `hosted_states` took as they were last said, as a render's
/// warnings (review isolation-6 #3, #15, #25).
fn stale_states(app: &CantusApp) -> Vec<cantus_host::RenderFailure> {
    app.plugins
        .saved_stale()
        .iter()
        .map(|(name, why)| cantus_host::RenderFailure {
            name: name.clone(),
            reason: why.clone(),
            kind: cantus_host::RenderLoss::Stale,
        })
        .collect()
}

/// A freeze made from a render at hand with states as they were last said: said after its
/// own word.
fn say_stale(app: &mut CantusApp, stale: &[cantus_host::RenderFailure], name: &str) {
    let (_, warnings) = crate::views::plugin_notice::render_words(stale, "freeze", name);
    if let Some(w) = warnings {
        let done = app.status.text.clone();
        app.set_error(format!("{done} {w}"));
    }
}

/// The hosted plug-ins' states, as they are now, written into the project for the
/// plug-ins of `target`'s region whose state the project does not hold yet: what the
/// render hears, and what an unloaded plug-in comes back with.
fn hosted_states(app: &mut CantusApp, target: FreezeTarget) -> Vec<Command> {
    let project = app.project().clone();
    let r = freeze::region(&project, target);
    let saved = app.plugins.project_for_save(&project);
    let mut out = Vec::new();
    for (strip, slot) in project.all_effects() {
        // The master's too when the whole mix is in the region (review 4: its live state
        // never reached the project, and the render was made from the stored one).
        let inside = r.strips.contains(&strip) || (r.whole && strip == StripId::MASTER);
        if !inside {
            continue;
        }
        let (Some(was), Some(now)) = (
            slot.spec.external.as_deref(),
            saved
                .all_effects()
                .find(|(_, s)| s.id == slot.id)
                .and_then(|(_, s)| s.spec.external.as_deref()),
        ) else {
            continue;
        };
        if was.state != now.state {
            out.push(Command::SetPluginState {
                strip,
                effect: slot.id,
                state: now.state.clone(),
                params: now.params.clone(),
            });
        }
    }
    for track in &project.tracks {
        if !r.tracks.contains(&track.id) {
            continue;
        }
        let (Some(was), Some(now)) = (
            track.instrument.external(),
            saved.track(track.id).and_then(|t| t.instrument.external()),
        ) else {
            continue;
        };
        if was.state != now.state {
            out.push(Command::SetInstrumentPluginState {
                track: track.id,
                state: now.state.clone(),
                params: now.params.clone(),
            });
        }
    }
    out
}

/// `project` with `commands` applied (a copy).
fn with(project: &Project, commands: &[Command]) -> Project {
    let mut p = project.clone();
    for c in commands {
        let _ = c.clone().apply(&mut p);
    }
    p
}

/// A render's instances: its inserts, its instruments, and the plug-ins it plays a wire
/// for (not installed: noted beside the render, review 3).
pub(crate) type Units = (
    Vec<cantus_engine::EffectUnit>,
    Vec<cantus_engine::inst::NativeUnit>,
    Vec<(cantus_core::PluginFormat, String)>,
);

/// The render instances for `snap`: only the hosted plug-ins it runs — not those of the
/// nodes resting nor of chains a frozen render replaces, nor the master's unless the render
/// hears the master (`master`) — review 1: every plug-in of the project was instantiated
/// for one freeze. Refused, naming it, when one it runs cannot be had (review 1: a refresh
/// without it wrote a pass-through render under the true key).
pub(crate) fn units_for(
    app: &mut CantusApp,
    project: &Project,
    snap: &Snapshot,
    master: bool,
    stored: bool,
) -> Result<Units, String> {
    let mut only = project.clone();
    for s in &mut only.strips {
        let runs = snap.strips.iter().find(|x| x.id == s.id && !x.rest);
        s.effects.retain(|e| {
            e.spec.external.is_none()
                || runs.is_some_and(|x| x.effects.iter().any(|y| y.id == e.id))
        });
    }
    if !master {
        only.master.effects.retain(|e| e.spec.external.is_none());
    }
    for t in &mut only.tracks {
        let runs = snap
            .tracks
            .iter()
            .any(|x| x.id == t.id && !x.rest && x.instrument.external().is_some());
        if !runs && t.instrument.external().is_some() {
            t.instrument = cantus_core::InstrumentSpec::default();
        }
    }
    let rate = app.engine.sample_rate;
    // Its plug-ins' render instances loaded first (an isolated one in a helper of its own),
    // frame by frame: the render waits in the queue meanwhile (round 29, review
    // isolation-5).
    if !app
        .plugins
        .render_ready(cantus_host::RenderFor::Freeze, &only, rate, stored)
    {
        return Err(PLUGINS_LOADING.to_string());
    }
    // A keyed render (a freeze's) from the states the project holds — what its key says
    // (review 2: a window's change rendered under the old key); a bounce from the live ones.
    let (units, insts) = if stored {
        (
            app.plugins.render_units_stored(&only, rate),
            app.plugins.render_instruments_stored(&only, rate),
        )
    } else {
        (
            app.plugins.render_units(&only, rate),
            app.plugins.render_instruments(&only, rate),
        )
    };
    // A bypassed insert, or one on a strip whose inserts are off, is not heard: its
    // stand-in (a wire) serves (review 2: a missing one refused the freeze).
    let heard = |strip: cantus_core::StripId, e: &cantus_core::EffectSlot| {
        !e.bypass && only.strip(strip).is_none_or(|s| s.fx_enabled)
    };
    let lost_effect = only
        .all_effects()
        .filter(|(strip, e)| heard(*strip, e))
        .filter_map(|(_, e)| e.spec.external.as_deref().map(|x| (e.id, x.name.clone())))
        .find(|(id, _)| !units.iter().any(|u| u.id == *id))
        .map(|(_, name)| name);
    let lost = lost_effect.or_else(|| {
        only.tracks
            .iter()
            .filter_map(|t| t.instrument.external().map(|x| (t.id, x.name.clone())))
            .find(|(id, _)| !insts.iter().any(|u| u.track == *id))
            .map(|(_, name)| name)
    });
    // The bypassed ones a wire stands in for because they are not installed: their latency
    // is not in the render (review 3) — it is made again once they are. (One installed
    // but failing to load would only be wired again.)
    let wires: Vec<(cantus_core::PluginFormat, String)> = only
        .all_effects()
        .filter(|(strip, e)| !heard(*strip, e))
        .filter_map(|(_, e)| e.spec.external.as_deref().map(|x| (e.id, x)))
        .filter(|(id, _)| !units.iter().any(|u| u.id == *id))
        .filter(|(_, x)| !app.plugins.installed(x.format, &x.id))
        .map(|(_, x)| (x.format, x.id.clone()))
        .collect();
    match lost {
        Some(name) => {
            drop((units, insts));
            app.plugins.finish_render();
            Err(tf("freeze.plugin_missing", &[("plugin", &name)]))
        }
        None => Ok((units, insts, wires)),
    }
}

/// Beside a render made with wires in place of bypassed plug-ins not installed: which ones.
fn wires_file(render: &Path) -> PathBuf {
    render.with_extension("wires")
}

/// Whether a render was made with a wire for a plug-in installed now (its latency missing
/// from it): made again (review 3).
fn wired_since_installed(app: &CantusApp, render: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(wires_file(render)) else {
        return false;
    };
    let Ok(wires) = serde_json::from_str::<Vec<(cantus_core::PluginFormat, String)>>(&text) else {
        return false;
    };
    wires
        .iter()
        .any(|(format, id)| app.plugins.installed(*format, id))
}

/// Start the next render waiting, if nothing runs and no export or modulator bake does
/// (the hosted plug-ins' render instances are one set at a time — review 1: a bake's end
/// let a freeze's go mid-render), and no file the render plays is still loading (the
/// window would wait for it — review 1).
fn start_next(app: &mut CantusApp) {
    // A bake waiting for its own plug-ins is a render too: one at a time (review
    // isolation-6 #4, #13, #26).
    if app.freeze.job.is_some()
        || app.export_in_progress()
        || app.modulators.pending_bake.is_some()
        || app.warp.pending_bake.is_some()
    {
        return;
    }
    let s = &app.engine.samples;
    if s.loading() || s.pitch_pending() > 0 || s.warp_pending() > 0 {
        return;
    }
    // Its own render's plug-ins still loading: it starts once they are in.
    if app.plugins.render_waiting(cantus_host::RenderFor::Freeze) {
        return;
    }
    while let Some(want) = app.freeze.queue.pop_front() {
        // A freeze takes its plug-ins' states as they are now: asked first, and waited for
        // frame by frame, as a save does (review isolation-6 #3, #15, #25).
        if matches!(want, Want::Freeze(_)) && app.freeze.asked.as_ref() != Some(&want) {
            app.plugins.ask_states();
            app.freeze.asked = Some(want.clone());
        }
        if app.freeze.asked.as_ref() == Some(&want) && !app.plugins.states_in() {
            app.freeze.queue.push_front(want);
            return;
        }
        match start(app, want.clone()) {
            Ok(Some(job)) => {
                app.freeze.asked = None;
                app.freeze.job = Some(job);
                return;
            }
            // Nothing to render after all: what it prepared goes (review isolation-6 #1).
            Ok(None) => {
                app.freeze.asked = None;
                app.plugins.drop_render(cantus_host::RenderFor::Freeze);
            }
            // Its plug-ins are loading: first in the queue again, said once.
            Err(e) if e == PLUGINS_LOADING => {
                app.freeze.queue.push_front(want);
                app.set_status(t("isolation.render.loading"));
                return;
            }
            Err(e) => {
                app.freeze.asked = None;
                app.plugins.drop_render(cantus_host::RenderFor::Freeze);
                app.set_error(e);
            }
        }
    }
}

/// A render's plug-ins are loading (`units_for`): the render waits, never fails.
pub(crate) const PLUGINS_LOADING: &str = "\u{0}plugins-loading";

/// Whether a render on disk is older than a file the target's region plays (edited since,
/// review 1: freezing again reused the render of the file as it was).
fn older_than_its_files(app: &CantusApp, target: FreezeTarget, render: &Path) -> bool {
    let Some(made) = std::fs::metadata(render).and_then(|m| m.modified()).ok() else {
        return false;
    };
    let project = app.project();
    let r = freeze::region(project, target);
    let base = app.audio_base();
    let mut newer = false;
    crate::views::samples_content::missing::each_path(project, &mut |owner, path, _| {
        use crate::views::samples_content::missing::Owner;
        let inside = match &owner {
            Owner::Zone { track, .. } | Owner::Pad { track, .. } | Owner::Layer { track, .. } => {
                r.tracks.contains(track)
            }
            Owner::Source { id } => r.tracks.iter().any(|t| {
                project.track(*t).is_some_and(|t| {
                    t.clips
                        .iter()
                        .any(|c| c.audio.as_ref().is_some_and(|a| a.source == *id))
                })
            }),
        };
        if inside && !newer {
            let p = crate::waveform::resolve(base, path);
            newer = std::fs::metadata(p)
                .and_then(|m| m.modified())
                .is_ok_and(|t| t > made);
        }
    });
    newer
}

/// Begin `want`'s render on a worker; `None` when there is nothing to render (frozen at
/// once from a render at hand, or no longer wanted).
fn start(app: &mut CantusApp, want: Want) -> Result<Option<Job>, String> {
    // The project's folder as it is now (a save as in this very frame).
    follow_path(app);
    let rate = app.engine.sample_rate.round() as u32;
    match want.clone() {
        Want::Freeze(target) => {
            if app.project().frozen(target).is_some() {
                return Ok(None);
            }
            let before = hosted_states(app, target);
            // The states it could not have as they are now (asked first, by `start_next`).
            let stale = stale_states(app);
            let edits = edits_in(app, app.project(), target);
            // Keyed as it will be frozen: disarmed (review 2: an armed track rendered twice).
            let (after, content) = match as_frozen(app.project(), &before, target) {
                Ok(x) => x,
                Err(why) => {
                    app.say_cannot(target, why);
                    return Ok(None);
                }
            };
            // A file it plays is not there (a pack downloading, a drive away): no render
            // of silence under its key (review 2).
            if let Some(file) = missing_file(app, &freeze::region(&after, target)) {
                return Err(tf("freeze.file_missing", &[("file", &file)]));
            }
            retry_back(app, &freeze::region(&after, target));
            // A render for this very sound is at hand: frozen at once — unless a file it
            // plays was edited since it was made.
            if let Some(path) = app.engine.frozen.path_of(&content, rate) {
                if older_than_its_files(app, target, &path) {
                    let _ = std::fs::remove_file(&path);
                    app.engine.frozen.forget_missing();
                } else if app.engine.frozen.find(&content, rate).is_some() {
                    commit_freeze(app, target, before, &content, edits, false);
                    say_stale(app, &stale, &name_of(app.project(), target));
                    return Ok(None);
                }
            }
            render_job(app, want, target, &after, content, before, edits).map(|mut job| {
                job.stale = stale;
                Some(job)
            })
        }
        Want::Refresh(target) => {
            let Some(stored) = app.project().frozen(target).map(|f| f.content.clone()) else {
                return Ok(None);
            };
            // A key that is not the target's any more (review 1): frozen again with the key
            // it has, and that one rendered — never one a scan would not look for.
            let now = freeze::content_key(app.project(), target).ok_or_else(|| t("freeze.gone"))?;
            let content = if now != stored {
                let commands = vec![
                    freeze::unfreeze_command(target),
                    freeze::freeze_command(app.project(), target)
                        .ok_or_else(|| t("freeze.gone"))?,
                ];
                if !app.exec(Command::Batch {
                    label_key: "cmd.freeze".into(),
                    commands,
                }) {
                    return Ok(None);
                }
                now
            } else {
                stored
            };
            if app.engine.frozen.find(&content, rate).is_some() {
                return Ok(None);
            }
            // A file it plays is not there yet: made when it is (it plays live meanwhile),
            // and said once (review 3: it waited without a word).
            let region = freeze::region(app.project(), target);
            if let Some(file) = missing_file(app, &region) {
                if app.freeze.said_waiting.insert(content.clone()) {
                    let name = name_of(app.project(), target);
                    app.set_status(tf(
                        "freeze.waits_for_file",
                        &[("name", &name), ("file", &file)],
                    ));
                }
                return Ok(None);
            }
            retry_back(app, &region);
            let project = app.project().clone();
            let edits = edits_in(app, &project, target);
            match render_job(app, want, target, &project, content, Vec::new(), edits) {
                Ok(job) => Ok(Some(job)),
                // Postponed, never marked failed: the file may come back.
                Err(e) if e == MISSING => Ok(None),
                Err(e) => Err(e),
            }
        }
        Want::Bounce(plan) => crate::bounce::start(app, plan),
    }
}

/// What the engine's preparation says, in the interface's words.
fn prepare_error(e: String) -> String {
    match e.as_str() {
        "gone" => t("freeze.gone"),
        "no folder for frozen renders" => t("freeze.no_folder"),
        e if e.starts_with("missing:") => MISSING.to_string(),
        _ => e,
    }
}

/// The engine found a file the render plays missing (`prepare`'s second line of defence).
const MISSING: &str = "\u{0}missing";

/// The project `target` is frozen in — `before` applied and frozen as `freeze_commands`
/// does it (disarmed) — and its content key.
fn as_frozen(
    project: &Project,
    before: &[Command],
    target: FreezeTarget,
) -> Result<(Project, String), CannotFreeze> {
    let p = with(project, before);
    let cmd = freeze::freeze_commands(&p, target)?;
    let mut q = p;
    cmd.apply(&mut q).map_err(|_| CannotFreeze::Gone)?;
    let content = q
        .frozen(target)
        .map(|f| f.content.clone())
        .ok_or(CannotFreeze::Gone)?;
    Ok((q, content))
}

/// The first file `r`'s tracks play that is not on disk, as the project names it (an
/// audio source, a sampler zone, a kit pad's sample or layer).
pub(crate) fn missing_file(app: &CantusApp, r: &freeze::Region) -> Option<String> {
    let project = app.project();
    let base = app.audio_base();
    let mut out = None;
    crate::views::samples_content::missing::each_path(project, &mut |owner, path, _| {
        use crate::views::samples_content::missing::Owner;
        if out.is_some() {
            return;
        }
        let inside = match &owner {
            Owner::Zone { track, .. } => r.tracks.contains(track),
            // A muted pad does not sound (review 3: one stopped a freeze).
            Owner::Pad { track, pad } | Owner::Layer { track, pad, .. } => {
                r.tracks.contains(track)
                    && project
                        .track(*track)
                        .and_then(|t| t.instrument.pulsus())
                        .and_then(|k| k.pads.iter().find(|p| p.id == *pad))
                        .is_some_and(|p| !p.muted)
            }
            // Only the clips that play: unmuted, on the first lane (review 3: an old take
            // on a lane below stopped a freeze, and postponed a refresh for good).
            Owner::Source { id } => r.tracks.iter().any(|t| {
                project.track(*t).is_some_and(|t| {
                    t.clips.iter().any(|c| {
                        !c.muted && c.lane == 0 && c.audio.as_ref().is_some_and(|a| a.source == *id)
                    })
                })
            }),
        };
        if inside && !crate::waveform::resolve(base, path).is_file() {
            out = Some(path.to_string());
        }
    });
    out
}

/// The files `r`'s tracks play that are back on disk after failing: tried again before a
/// render (the sample cache remembers a failure until told).
pub(crate) fn retry_back(app: &mut CantusApp, r: &freeze::Region) {
    let mut back: Vec<(String, bool)> = Vec::new();
    {
        let project = app.project();
        let base = app.audio_base();
        crate::views::samples_content::missing::each_path(project, &mut |owner, path, _| {
            use crate::views::samples_content::missing::Owner;
            let (inside, audio) = match &owner {
                Owner::Zone { track, .. }
                | Owner::Pad { track, .. }
                | Owner::Layer { track, .. } => (r.tracks.contains(track), false),
                Owner::Source { id } => (
                    r.tracks.iter().any(|t| {
                        project.track(*t).is_some_and(|t| {
                            t.clips
                                .iter()
                                .any(|c| c.audio.as_ref().is_some_and(|a| a.source == *id))
                        })
                    }),
                    true,
                ),
            };
            if inside && crate::waveform::resolve(base, path).is_file() {
                back.push((path.to_string(), audio));
            }
        });
    }
    for (path, audio) in back {
        if app.engine.samples.has_failed(&path, audio) {
            if audio {
                app.engine.samples.retry_audio(&path);
            } else {
                app.engine.samples.retry(std::path::Path::new(&path));
            }
        }
    }
}

fn render_job(
    app: &mut CantusApp,
    want: Want,
    target: FreezeTarget,
    project: &Project,
    content: String,
    before: Vec<Command>,
    edits: u64,
) -> Result<Job, String> {
    let before_render = app.engine.renders_late();
    let job = match cantus_engine::freeze::render::prepare(&mut app.engine, project, target) {
        Ok(j) => j,
        Err(e) => {
            let e = prepare_error(e);
            return Err(if e == MISSING && matches!(want, Want::Freeze(_)) {
                t("freeze.file_missing_any")
            } else {
                e
            });
        }
    };
    app.say_late(before_render);
    let late = app.late_since(before_render);
    let whole = freeze::region(project, target).whole;
    let (units, insts, wires) = match units_for(app, project, &job.snapshot, whole, true) {
        Ok(u) => u,
        Err(why) if why == PLUGINS_LOADING => return Err(why),
        Err(why) => {
            app.freeze.failed.insert(content.clone(), why.clone());
            return Err(why);
        }
    };
    // A save's prune never takes what this render writes (review 4).
    if let (Some(name), Ok(mut p)) = (
        job.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned()),
        app.freeze.protect.lock(),
    ) {
        p.insert(name);
    }
    let progress = Arc::new(RenderProgress::default());
    let p = progress.clone();
    let worker = std::thread::Builder::new()
        .name("cantus-freeze".into())
        .spawn(move || job.run(units, insts, &p))
        .map_err(|e| e.to_string())?;
    Ok(Job {
        name: name_of(project, target),
        want,
        progress,
        content,
        before,
        edits,
        wires,
        worker: Some(worker),
        bounce: None,
        epoch: app.epoch,
        late,
        stale: Vec::new(),
    })
}

/// The edits made so far in the windows of `target`'s region's plug-ins (review 3).
fn edits_in(app: &CantusApp, project: &Project, target: FreezeTarget) -> u64 {
    let r = freeze::region(project, target);
    let effects: Vec<cantus_core::EffectId> = project
        .all_effects()
        .filter(|(s, _)| r.strips.contains(s))
        .map(|(_, e)| e.id)
        .chain(
            project
                .master
                .effects
                .iter()
                .map(|e| e.id)
                .filter(|_| r.whole),
        )
        .collect();
    let tracks: Vec<cantus_core::TrackId> = r.tracks.iter().copied().collect();
    app.plugins.edits_of(&effects, &tracks)
}

/// A bounce's job (`crate::bounce::start` builds it).
pub(crate) fn bounce_job(
    app: &CantusApp,
    want: Want,
    name: String,
    progress: Arc<RenderProgress>,
    worker: JoinHandle<Result<PathBuf, RenderError>>,
    prepared: crate::bounce::Prepared,
) -> Job {
    Job {
        want,
        name,
        progress,
        content: String::new(),
        before: Vec::new(),
        edits: 0,
        wires: Vec::new(),
        worker: Some(worker),
        bounce: Some(prepared),
        epoch: app.epoch,
        late: false,
        stale: Vec::new(),
    }
}

/// Freeze `target` now (its render is on disk): the hosted states and the freeze, one
/// undo step — when what it holds is still `content`.
fn commit_freeze(
    app: &mut CantusApp,
    target: FreezeTarget,
    before: Vec<Command>,
    content: &str,
    edits: u64,
    late: bool,
) {
    // A plug-in of its region turned while it rendered (its window, its own browser):
    // rendered again with what it holds now, never set back (review 2) — counted as edits,
    // never as its state's bytes, which automation and modulators move (review 3: it
    // never landed).
    if edits_in(app, app.project(), target) != edits {
        app.freeze.queue.push_front(Want::Freeze(target));
        return;
    }
    let after = with(app.project(), &before);
    match as_frozen(app.project(), &before, target) {
        Ok((_, now)) if now == content => {}
        Ok(_) => {
            // Edited while it rendered: rendered again for what it holds now.
            app.freeze.queue.push_front(Want::Freeze(target));
            return;
        }
        Err(why) => {
            app.say_cannot(target, why);
            return;
        }
    }
    let cmd = match freeze::freeze_commands(&after, target) {
        Ok(c) => c,
        Err(why) => {
            // Said, not dropped in silence (review 1): a live input switched on, a solo.
            app.say_cannot(target, why);
            return;
        }
    };
    let mut commands = before;
    match cmd {
        Command::Batch { commands: c, .. } => commands.extend(c),
        c => commands.push(c),
    }
    let name = name_of(app.project(), target);
    if app.exec(Command::Batch {
        label_key: "cmd.freeze".into(),
        commands,
    }) {
        let key = crate::views::freeze_view::live_words(app.project(), target, "freeze.done");
        app.say_done(tf(key, &[("name", &name)]), late);
    }
    app.force_resync();
}

/// Write (or clear) beside a render the plug-ins a wire stood in for (review 3).
fn note_wires(render: &Path, wires: &[(cantus_core::PluginFormat, String)]) {
    let file = wires_file(render);
    if wires.is_empty() {
        let _ = std::fs::remove_file(file);
    } else if let Ok(text) = serde_json::to_string(wires) {
        let _ = std::fs::write(file, text);
    }
}

/// The running render, when its worker is done: committed, or why not said.
fn collect(app: &mut CantusApp) {
    let done = app
        .freeze
        .job
        .as_ref()
        .and_then(|j| j.worker.as_ref())
        .is_some_and(|w| w.is_finished());
    if !done {
        return;
    }
    let Some(mut job) = app.freeze.job.take() else {
        return;
    };
    let mut result = job
        .worker
        .take()
        .map(|w| w.join().unwrap_or(Err(RenderError::Cancelled)));
    // Stopped while its file was being written: nothing committed (review 2). A bounce's
    // file goes; a freeze's finished render may stay (found by its key).
    if job
        .progress
        .cancel
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        if let (Some(Ok(path)), Want::Bounce(_)) = (&result, &job.want) {
            let _ = std::fs::remove_file(path);
        }
        result = Some(Err(RenderError::Cancelled));
    }
    // An isolated plug-in that stopped working during the render (round 29, review #6,
    // #47): the render lacks it from that point, so nothing is frozen or bounced from it.
    // The states it took as they were last said are its warnings.
    let mut lost = app.plugins.render_failures();
    lost.extend(job.stale.iter().cloned());
    app.plugins.finish_render();
    app.engine.frozen.forget_missing();
    app.force_resync();
    // Asked in another project (review 1): never laid into this one. A bounce's file
    // goes; a freeze's render stays (found by its key, it serves that sound anywhere).
    if job.epoch != app.epoch {
        if let (Some(Ok(path)), Want::Bounce(_)) = (&result, &job.want) {
            let _ = std::fs::remove_file(path);
        }
        return;
    }
    let (losses, warnings) = crate::views::plugin_notice::render_words(&lost, "freeze", &job.name);
    if let (Some(Ok(path)), Some(text)) = (&result, losses) {
        let _ = std::fs::remove_file(path);
        if !job.content.is_empty() {
            app.freeze.failed.insert(job.content.clone(), text.clone());
        }
        app.set_error(text);
        return;
    }
    // Begun from a plug-in's state as it was last said: done, and said after what the
    // commit says — never written over by it (review isolation-4 #10; isolation-5 #46).
    let warned = matches!(result, Some(Ok(_))).then_some(warnings).flatten();
    match result {
        Some(Ok(path)) => match job.want.clone() {
            Want::Freeze(target) => {
                note_wires(&path, &job.wires);
                commit_freeze(app, target, job.before, &job.content, job.edits, job.late)
            }
            Want::Refresh(_) => note_wires(&path, &job.wires),
            Want::Bounce(_) => {
                if let Some(prepared) = job.bounce.take() {
                    crate::bounce::commit(app, prepared, path);
                }
            }
        },
        Some(Err(RenderError::Cancelled)) => {
            app.set_status(tf("freeze.stopped", &[("name", &job.name)]));
        }
        // Its latency moves during the song: it stays live, said (review 3) — a bounce in
        // a bounce's words, with its track's or strip's name (review 4).
        Some(Err(RenderError::LatencyMoved)) => {
            let text = match &job.want {
                Want::Bounce(plan) => {
                    let what = match plan.through {
                        crate::bounce::Through::Strip(s) => FreezeTarget::Strip(s),
                        _ => FreezeTarget::Track(plan.track),
                    };
                    tf(
                        "bounce.latency_moves",
                        &[("name", &name_of(app.project(), what))],
                    )
                }
                _ => tf("freeze.latency_moves", &[("name", &job.name)]),
            };
            if !job.content.is_empty() {
                app.freeze.failed.insert(job.content.clone(), text.clone());
            }
            app.set_error(text);
        }
        Some(Err(e)) => {
            if !job.content.is_empty() {
                app.freeze.failed.insert(job.content.clone(), e.to_string());
            }
            app.set_error(tf(
                "freeze.failed",
                &[("name", &job.name), ("error", &e.to_string())],
            ));
        }
        None => {}
    }
    if let Some(w) = warned {
        let done = app.status.text.clone();
        app.set_error(format!("{done} {w}"));
    }
}

/// The session ends (`close_session`): the running render stopped and waited for a moment,
/// so its partial files go with it (review 1); what is left is tidied at the next start.
pub fn close(app: &mut CantusApp) {
    app.freeze.closed = true;
    app.freeze.queue.clear();
    let Some(mut job) = app.freeze.job.take() else {
        return;
    };
    job.progress.cancel();
    app.plugins.cancel_render();
    if let Some(w) = job.worker.take() {
        let until = Instant::now() + Duration::from_secs(2);
        while !w.is_finished() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        if w.is_finished() {
            if let Ok(Ok(path)) = w.join() {
                if matches!(job.want, Want::Bounce(_)) {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
    app.plugins.finish_render();
}
