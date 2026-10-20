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
use cantus_core::{Command, Project, Snapshot};
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
    worker: Option<JoinHandle<Result<PathBuf, RenderError>>>,
    /// A bounce's plan, filled in with its file when done.
    bounce: Option<crate::bounce::Prepared>,
    /// The project it was asked in (`CantusApp::epoch`).
    epoch: u64,
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

/// The project's own folder for renders: `Freeze` beside its file, as `Audio` is.
pub fn project_dir(path: Option<&Path>) -> Option<PathBuf> {
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
    app.engine.frozen.set_app_dir(dir);
    app.freeze.epoch = Some(app.epoch);
    follow_path(app);
    apply_low_latency(app);
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
        app.engine.frozen.set_project_dir(dir);
        app.freeze.dir_for = Some(app.path.clone());
        app.force_resync();
    }
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
    if app.project().pattern_mode {
        return;
    }
    let rate = app.engine.sample_rate.round() as u32;
    for target in app.project().frozen_targets() {
        let Some(content) = app.project().frozen(target).map(|f| f.content.clone()) else {
            continue;
        };
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
        for (_, old, new) in &moved {
            if let Some(from) = app.engine.frozen.path_of(old, rate) {
                let name =
                    cantus_engine::freeze::file_name(&cantus_engine::freeze::render_key(new, rate));
                let _ = std::fs::create_dir_all(dir);
                let _ = std::fs::copy(&from, dir.join(name));
            }
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

/// The hosted plug-ins' states, as they are now, written into the project for the
/// plug-ins of `target`'s region whose state the project does not hold yet: what the
/// render hears, and what an unloaded plug-in comes back with.
fn hosted_states(app: &mut CantusApp, target: FreezeTarget) -> Vec<Command> {
    let project = app.project().clone();
    let r = freeze::region(&project, target);
    let saved = app.plugins.project_for_save(&project);
    let mut out = Vec::new();
    for (strip, slot) in project.all_effects() {
        if !r.strips.contains(&strip) {
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
) -> Result<
    (
        Vec<cantus_engine::EffectUnit>,
        Vec<cantus_engine::inst::NativeUnit>,
    ),
    String,
> {
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
    let units = app.plugins.render_units(&only, rate);
    let insts = app.plugins.render_instruments(&only, rate);
    let lost_effect = only
        .all_effects()
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
    match lost {
        Some(name) => {
            drop((units, insts));
            app.plugins.finish_render();
            Err(tf("freeze.plugin_missing", &[("plugin", &name)]))
        }
        None => Ok((units, insts)),
    }
}

/// Start the next render waiting, if nothing runs and no export or modulator bake does
/// (the hosted plug-ins' render instances are one set at a time — review 1: a bake's end
/// let a freeze's go mid-render), and no file the render plays is still loading (the
/// window would wait for it — review 1).
fn start_next(app: &mut CantusApp) {
    if app.freeze.job.is_some() || app.export_in_progress() || app.modulators.pending_bake.is_some()
    {
        return;
    }
    let s = &app.engine.samples;
    if s.loading() || s.pitch_pending() > 0 || s.warp_pending() > 0 {
        return;
    }
    while let Some(want) = app.freeze.queue.pop_front() {
        match start(app, want) {
            Ok(Some(job)) => {
                app.freeze.job = Some(job);
                return;
            }
            Ok(None) => {}
            Err(e) => app.set_error(e),
        }
    }
}

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
    let rate = app.engine.sample_rate.round() as u32;
    match want.clone() {
        Want::Freeze(target) => {
            if app.project().frozen(target).is_some() {
                return Ok(None);
            }
            let before = hosted_states(app, target);
            let after = with(app.project(), &before);
            let Some(content) = freeze::content_key(&after, target) else {
                return Ok(None);
            };
            // A render for this very sound is at hand: frozen at once — unless a file it
            // plays was edited since it was made.
            if let Some(path) = app.engine.frozen.path_of(&content, rate) {
                if older_than_its_files(app, target, &path) {
                    let _ = std::fs::remove_file(&path);
                    app.engine.frozen.forget_missing();
                } else if app.engine.frozen.find(&content, rate).is_some() {
                    commit_freeze(app, target, before, &content);
                    return Ok(None);
                }
            }
            render_job(app, want, target, &after, content, before).map(Some)
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
            let project = app.project().clone();
            render_job(app, want, target, &project, content, Vec::new()).map(Some)
        }
        Want::Bounce(plan) => crate::bounce::start(app, plan),
    }
}

/// What the engine's preparation says, in the interface's words.
fn prepare_error(e: String) -> String {
    match e.as_str() {
        "gone" => t("freeze.gone"),
        "no folder for frozen renders" => t("freeze.no_folder"),
        _ => e,
    }
}

fn render_job(
    app: &mut CantusApp,
    want: Want,
    target: FreezeTarget,
    project: &Project,
    content: String,
    before: Vec<Command>,
) -> Result<Job, String> {
    let late = app.engine.renders_late();
    let job = cantus_engine::freeze::render::prepare(&mut app.engine, project, target)
        .map_err(prepare_error)?;
    app.say_late(late);
    let whole = freeze::region(project, target).whole;
    let (units, insts) = match units_for(app, project, &job.snapshot, whole) {
        Ok(u) => u,
        Err(why) => {
            app.freeze.failed.insert(content.clone(), why.clone());
            return Err(why);
        }
    };
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
        worker: Some(worker),
        bounce: None,
        epoch: app.epoch,
    })
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
        worker: Some(worker),
        bounce: Some(prepared),
        epoch: app.epoch,
    }
}

/// Freeze `target` now (its render is on disk): the hosted states and the freeze, one
/// undo step — when what it holds is still `content`.
fn commit_freeze(app: &mut CantusApp, target: FreezeTarget, before: Vec<Command>, content: &str) {
    let after = with(app.project(), &before);
    if freeze::content_key(&after, target).as_deref() != Some(content) {
        // Edited while it rendered: rendered again for what it holds now.
        app.freeze.queue.push_front(Want::Freeze(target));
        return;
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
        app.set_status(tf("freeze.done", &[("name", &name)]));
    }
    app.force_resync();
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
    let result = job
        .worker
        .take()
        .map(|w| w.join().unwrap_or(Err(RenderError::Cancelled)));
    // An isolated plug-in that stopped working during the render (round 29, review #6,
    // #47): the render lacks it from that point, so nothing is frozen or bounced from it.
    let lost = app.plugins.render_failures();
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
    if let (Some(Ok(path)), Some(f)) = (&result, lost.first()) {
        let _ = std::fs::remove_file(path);
        let text = tf(
            "isolation.freeze.stopped",
            &[
                ("name", &f.name),
                (
                    "reason",
                    &crate::views::plugin_notice::reason_words(&f.reason),
                ),
                ("job", &job.name),
            ],
        );
        if !job.content.is_empty() {
            app.freeze.failed.insert(job.content.clone(), text.clone());
        }
        app.set_error(text);
        return;
    }
    match result {
        Some(Ok(path)) => match job.want.clone() {
            Want::Freeze(target) => commit_freeze(app, target, job.before, &job.content),
            Want::Refresh(_) => {}
            Want::Bounce(_) => {
                if let Some(prepared) = job.bounce.take() {
                    crate::bounce::commit(app, prepared, path);
                }
            }
        },
        Some(Err(RenderError::Cancelled)) => {
            app.set_status(tf("freeze.stopped", &[("name", &job.name)]));
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
}

/// The session ends (`close_session`): the running render stopped and waited for a moment,
/// so its partial files go with it (review 1); what is left is tidied at the next start.
pub fn close(app: &mut CantusApp) {
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
