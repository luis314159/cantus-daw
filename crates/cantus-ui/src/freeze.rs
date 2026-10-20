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

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::freeze::{self, CannotFreeze, FreezeTarget};
use cantus_core::{Command, Project};
use cantus_engine::render::{RenderError, RenderProgress};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the project is looked at for frozen targets whose render is not at hand.
const SCAN: Duration = Duration::from_millis(500);

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
    scanned: Option<Instant>,
    /// The project folder the store was last given.
    dir_for: Option<Option<PathBuf>>,
    /// The low-latency monitoring the engine was last told (limit in samples, or off).
    told: Option<Option<u32>>,
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
}

/// Where renders go when the project has no folder (and where they are always looked for).
pub fn app_dir() -> PathBuf {
    crate::app::data_dir().join("cache").join("freeze")
}

/// The project's own folder for renders: `Freeze` beside its file, as `Audio` is.
pub fn project_dir(path: Option<&std::path::Path>) -> Option<PathBuf> {
    path.map(|p| p.with_file_name("Freeze"))
}

/// A target's name on screen.
pub fn name_of(project: &Project, target: FreezeTarget) -> String {
    match target {
        FreezeTarget::Track(t) => project
            .track(t)
            .map(|t| t.name.clone())
            .unwrap_or_default(),
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
/// the settings say.
pub fn wire(app: &mut CantusApp) {
    app.engine.frozen.set_app_dir(app_dir());
    follow_path(app);
    apply_low_latency(app);
}

/// The store follows the project's folder (opened, saved as).
fn follow_path(app: &mut CantusApp) {
    let now = Some(project_dir(app.path.as_deref()));
    if app.freeze.dir_for != now {
        app.engine
            .frozen
            .set_project_dir(project_dir(app.path.as_deref()));
        app.freeze.dir_for = now;
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

/// Queue a render (a bounce's: `crate::bounce`) and start it if nothing runs.
pub(crate) fn queue(app: &mut CantusApp, want: Want) {
    if !app.freeze.queue.contains(&want) {
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
        self.freeze.queue.push_back(want);
        start_next(self);
    }

    /// Unfreeze `target`: its live chain back, one undo step.
    pub fn unfreeze_target(&mut self, target: FreezeTarget) {
        if self.project().frozen(target).is_none() {
            return;
        }
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

    /// Stop the running render (nothing is frozen or bounced) and what waits after it.
    pub fn cancel_freeze(&mut self) {
        self.freeze.queue.retain(|w| matches!(w, Want::Refresh(_)));
        if let Some(j) = &self.freeze.job {
            j.progress.cancel();
        }
    }

    fn say_cannot(&mut self, target: FreezeTarget, why: CannotFreeze) {
        let name = name_of(self.project(), target);
        let text = match why {
            CannotFreeze::Gone => return,
            CannotFreeze::Frozen => tf("freeze.already", &[("name", &name)]),
            CannotFreeze::Monitoring(t) => tf(
                "freeze.cannot_monitoring",
                &[
                    ("name", &name),
                    (
                        "track",
                        &self
                            .project()
                            .track(t)
                            .map(|t| t.name.clone())
                            .unwrap_or_default(),
                    ),
                ],
            ),
        };
        self.set_error(text);
    }
}

/// An edit a frozen target refused (`CantusApp::exec`): the status line says which, and
/// how to give it back.
pub fn refused(app: &mut CantusApp, target: FreezeTarget) {
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

/// Start the next render waiting, if nothing runs and no export does (the hosted
/// plug-ins' render instances are one set at a time).
fn start_next(app: &mut CantusApp) {
    if app.freeze.job.is_some() || app.export_in_progress() {
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
            let content = freeze::content_key(&after, target).ok_or("gone")?;
            // A render for this very sound is at hand: frozen at once.
            if app.engine.frozen.find(&content, rate).is_some() {
                commit_freeze(app, target, before, &content);
                return Ok(None);
            }
            render_job(app, want, target, &after, content, before).map(Some)
        }
        Want::Refresh(target) => {
            let Some(content) = app.project().frozen(target).map(|f| f.content.clone()) else {
                return Ok(None);
            };
            if app.engine.frozen.find(&content, rate).is_some() {
                return Ok(None);
            }
            let project = app.project().clone();
            render_job(app, want, target, &project, content, Vec::new()).map(Some)
        }
        Want::Bounce(plan) => crate::bounce::start(app, plan).map(Some),
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
    let job = cantus_engine::freeze::render::prepare(&mut app.engine, project, target)?;
    let units = app.plugins.render_units(project, app.engine.sample_rate);
    let insts = app.plugins.render_instruments(project, app.engine.sample_rate);
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
    })
}

/// A bounce's job (`crate::bounce::start` builds it).
pub(crate) fn bounce_job(
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
    let Ok(cmd) = freeze::freeze_commands(&after, target) else {
        return;
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
        app.set_status(tf("freeze.frozen", &[("name", &name)]));
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
    app.plugins.finish_render();
    app.engine.frozen.forget_missing();
    app.force_resync();
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
