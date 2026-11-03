//! The recording mode in the interface (round 36, ADR 0092; BACKLOG §4c): *Prepare to
//! record* renders every strip that feeds the master through the freeze machinery
//! (ADR 0064: the same renders, the same files, found again by their key) — and, when the
//! master is baked, one render of the whole mix — then puts the edit lock on the bus;
//! *Leave* takes the lock off and unfreezes what the mode froze.
//!
//! **The state is the session's.** The mode's freezes, its leave and a strip brought live
//! are applied off the history (`CommandBus::apply_session`): never an undo step, never a
//! change to save. The project never reaches a file frozen by the mode ([`as_saved`] thaws
//! the mode's targets in every copy written), so a session cut short reopens out of it,
//! with the renders still on disk to be found by their keys.

use crate::app::CantusApp;
use crate::i18n::{t, tf};
use cantus_core::freeze::{self, FreezeTarget};
use cantus_core::record_mode::{self, EditLock, Plan, Why};
use cantus_core::{Command, Project, TrackId};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

/// The most a render waits for its tail to ring out (`TapOptions::freeze`): what the disk
/// estimate counts on, worded "up to".
pub const MAX_TAIL_SECONDS: f64 = 30.0;

/// What the user chooses before the mode starts (kept for the session).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    /// Fader, pan, width, mute and solo of the baked strips act live at the sum.
    pub mix_live: bool,
    /// The master's chain runs live on the sum; `false`: the whole mix is one render and
    /// nothing of the master (or the mix) moves.
    pub master_live: bool,
    /// A strip that cannot be rendered (a plug-in that cannot render offline) stops the
    /// preparation, instead of staying live and being named.
    pub stop_on_failure: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mix_live: true,
            master_live: true,
            stop_on_failure: false,
        }
    }
}

/// The baked master's render on its worker.
pub struct MasterRun {
    pub key: String,
    progress: Arc<cantus_engine::RenderProgress>,
    worker: Option<JoinHandle<Result<PathBuf, String>>>,
    /// Prepared, waiting for its hosted plug-ins' render instances (and the muted project
    /// they are made for).
    pending: Option<(cantus_engine::record_mode::MasterJob, Project)>,
}

impl std::fmt::Debug for MasterRun {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MasterRun").field("key", &self.key).finish()
    }
}

/// A preparation under way.
#[derive(Debug)]
pub struct Prep {
    pub plan: Plan,
    pub options: Options,
    pub started: Instant,
    pub total: usize,
    pub cpu_before: Option<f32>,
    pub epoch: u64,
    /// The baked master's render, once the strips' are in.
    pub master: Option<MasterRun>,
    /// The baked master's render failed: why.
    pub master_failed: Option<String>,
}

/// The mode on.
#[derive(Debug)]
pub struct Active {
    pub options: Options,
    /// What the mode froze (kept frozen by the user before: not here).
    pub baked: Vec<FreezeTarget>,
    pub already: usize,
    /// What stays live, and why (a strip brought live to arm a track among them).
    pub live: Vec<(FreezeTarget, Why)>,
    /// Planned and not baked: why not.
    pub failed: Vec<(FreezeTarget, String)>,
    pub since: Instant,
    pub bytes: u64,
    pub cpu_before: Option<f32>,
    pub epoch: u64,
    /// The baked master's content key, while it plays.
    pub master: Option<String>,
}

#[derive(Debug, Default)]
pub enum Phase {
    #[default]
    Idle,
    Preparing(Box<Prep>),
    Active(Box<Active>),
}

/// The estimate shown before the mode starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Estimate {
    /// Strips and tracks to render.
    pub targets: usize,
    /// Of them, renders that are on disk already (no work, no new bytes).
    pub reused: usize,
    /// Bytes still to be written, at most (every tail at its longest).
    pub bytes_new: u64,
    /// Bytes of all the renders the mode plays, at most.
    pub bytes_all: u64,
    /// The song's length, seconds (each render is up to this and its tail).
    pub seconds: f64,
    /// Baked strips whose mix stays locked even with the mix live: they feed another baked
    /// strip (a send into a return, an output into a bus) whose render holds them.
    pub locked_mix: usize,
}

/// The mode's state (`FreezeState::record`).
#[derive(Debug, Default)]
pub struct State {
    pub phase: Phase,
    pub options: Options,
    /// The question before it starts: its plan and estimate.
    pub ask: Option<(Plan, Estimate)>,
    /// The "leave" question: keep or discard the renders.
    pub leaving: bool,
    /// A track asked to be armed whose strip is baked: the offer to bring it live.
    pub offer: Option<TrackId>,
    /// The engine's load while the transport plays, smoothed: what the bar compares.
    cpu: Option<f32>,
    /// The same, measured since the mode began.
    cpu_now: Option<f32>,
}

impl State {
    pub fn is_active(&self) -> bool {
        matches!(self.phase, Phase::Active(_))
    }

    pub fn is_preparing(&self) -> bool {
        matches!(self.phase, Phase::Preparing(_))
    }

    /// Any phase but idle: the project is not the user's alone.
    pub fn on(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    pub fn active(&self) -> Option<&Active> {
        match &self.phase {
            Phase::Active(a) => Some(a),
            _ => None,
        }
    }

    /// The smoothed load now, while the mode is on and the transport plays.
    pub fn cpu_now(&self) -> Option<f32> {
        self.cpu_now
    }

    /// The smoothed load before the mode, when the transport played.
    pub fn cpu_before(&self) -> Option<f32> {
        match &self.phase {
            Phase::Active(a) => a.cpu_before,
            Phase::Preparing(p) => p.cpu_before,
            Phase::Idle => self.cpu,
        }
    }

    /// Whether the master is baked now.
    pub fn master_baked(&self) -> bool {
        self.active().is_some_and(|a| a.master.is_some())
    }
}

/// Bytes a render of `seconds` takes: 32-bit float, stereo (ADR 0050's cache file adds a
/// header and a checksum per block, a fraction of a per cent).
pub fn bytes_for(seconds: f64, rate: u32) -> u64 {
    (seconds * rate as f64).ceil() as u64 * 8
}

/// The song's length in seconds through its tempo map (the last clip's end).
pub fn song_seconds(project: &Project) -> f64 {
    project
        .tempo_map()
        .seconds_at(project.content_end().0 as f64)
        .max(0.0)
}

/// Baked strips of `plan` that another baked strip's render holds by more than a
/// pre-fader send (their mix stays locked: ADR 0092 §2).
pub fn locked_mix(project: &Project, plan: &Plan) -> usize {
    plan.bake
        .iter()
        .filter(|t| match t {
            FreezeTarget::Strip(s) => plan.bake.iter().chain(plan.already.iter()).any(|o| {
                let r = freeze::region(project, *o);
                let root = match o {
                    FreezeTarget::Strip(x) => Some(*x),
                    FreezeTarget::Track(_) => None,
                };
                // A pre-fader sender keeps its fader live (ADR 0092 §2).
                o != *t
                    && r.strips.contains(s)
                    && !freeze::pre_only_senders(project, &r, root).contains(s)
            }),
            FreezeTarget::Track(_) => false,
        })
        .count()
}

/// The estimate for `plan`: the renders on disk are not counted as new; each render is
/// counted at its longest (the song and the longest tail a render waits for).
pub fn estimate(app: &CantusApp, plan: &Plan, options: Options) -> Estimate {
    let rate = app.engine.sample_rate.round() as u32;
    let seconds = song_seconds(app.project());
    let one = bytes_for(seconds + MAX_TAIL_SECONDS, rate);
    let reused = plan
        .bake
        .iter()
        .filter(|t| {
            freeze::content_key(app.project(), **t)
                .and_then(|k| app.engine.frozen.path_of(&k, rate))
                .is_some()
        })
        .count();
    // The baked master is one render more.
    let master = usize::from(!options.master_live);
    Estimate {
        targets: plan.bake.len(),
        reused,
        bytes_new: one * (plan.bake.len() - reused + master) as u64,
        bytes_all: one * (plan.bake.len() + master) as u64,
        seconds,
        locked_mix: locked_mix(app.project(), plan),
    }
}

/// Bytes as a person reads them: "230 MB", "1.4 GB".
pub fn size_text(bytes: u64) -> String {
    let mb = bytes as f64 / 1_048_576.0;
    if mb >= 1024.0 {
        format!("{:.1} GB", mb / 1024.0)
    } else {
        format!("{mb:.0} MB")
    }
}

/// What moves the tempo without a command, so a render would play at another speed than
/// the song: a followed sync source (Link, MIDI clock, time code) or the session's scenes.
fn tempo_moves(app: &CantusApp) -> Option<String> {
    use crate::views::transport::sync_service::SyncSource;
    if app.settings.sync.source != SyncSource::None {
        return Some(t("record.cannot.sync"));
    }
    if app.perform.active() {
        return Some(t("record.cannot.session"));
    }
    None
}

/// Why the mode cannot start now, in words (`None`: it can).
pub fn cannot_start(app: &CantusApp) -> Option<String> {
    if app.freeze.record.on() {
        return Some(t("record.cannot.on"));
    }
    if app.is_recording() {
        return Some(t("record.cannot.recording"));
    }
    if app.export_in_progress() {
        return Some(t("record.cannot.exporting"));
    }
    tempo_moves(app)
}

/// Whether `target`'s freeze is the mode's own (applied off the history).
pub fn bakes(app: &CantusApp, target: FreezeTarget) -> bool {
    match &app.freeze.record.phase {
        Phase::Preparing(p) => p.plan.bake.contains(&target),
        _ => false,
    }
}

/// The words a refusal of `cmd` is said in: a desk recall or a cue (ADR 0093) is refused
/// whole when it reaches a baked strip, and says so.
pub fn notice_key(cmd: &Command) -> &'static str {
    use cantus_core::mix_cmd::MixCommand as M;
    match cmd {
        Command::Mix(M::Recall { .. } | M::Apply { .. }) => "record.notice_recall",
        Command::Mix(M::InsertCue { .. } | M::SetCue { .. } | M::RemoveCue { .. }) => {
            "record.notice_cue"
        }
        _ => "record.notice",
    }
}

/// The track `cmd` arms, when it arms one (the mode offers to bring its strip live).
pub fn arm_of(cmd: &Command) -> Option<TrackId> {
    match cmd {
        Command::SetTrackArmed { id, armed: true } => Some(*id),
        _ => None,
    }
}

/// The lock the mode puts on: the armed regions (and those brought live) editable.
fn lock_for(project: &Project, options: Options, live: &[(FreezeTarget, Why)]) -> EditLock {
    let editable: Vec<FreezeTarget> = live
        .iter()
        .filter(|(_, w)| matches!(w, Why::Armed(_)))
        .map(|(t, _)| *t)
        .collect();
    EditLock::new(project, options.mix_live, options.master_live, &editable)
}

/// Apply the mode's own commands, off the history, as one.
fn apply_session(app: &mut CantusApp, commands: Vec<Command>) {
    if commands.is_empty() {
        return;
    }
    let batch = Command::Batch {
        label_key: "cmd.record_leave".into(),
        commands,
    };
    if let Err(e) = app.bus.apply_session(batch) {
        app.set_error(e.to_string());
    }
    app.force_resync();
}

/// The unfreezes of the targets of `list` that are frozen.
fn unfreezes(project: &Project, list: &[FreezeTarget]) -> Vec<Command> {
    list.iter()
        .filter(|t| project.frozen(**t).is_some())
        .map(|t| freeze::unfreeze_command(*t))
        .collect()
}

impl CantusApp {
    /// *Prepare to record…*: the plan and its size, asked before anything is rendered.
    /// Says why when there is nothing to do.
    pub fn record_mode_ask(&mut self) {
        if let Some(why) = cannot_start(self) {
            self.set_error(why);
            return;
        }
        let plan = record_mode::plan(self.project());
        if plan.is_empty() {
            self.set_status(t("record.nothing"));
            return;
        }
        let est = estimate(self, &plan, self.freeze.record.options);
        self.freeze.record.ask = Some((plan, est));
    }

    /// Start the preparation with `options` (the question's *Prepare*, an agent's call).
    pub fn record_mode_begin(&mut self, options: Options) -> Result<(), String> {
        if let Some(why) = cannot_start(self) {
            return Err(why);
        }
        let plan = record_mode::plan(self.project());
        if plan.is_empty() {
            return Err(t("record.nothing"));
        }
        let s = &mut self.freeze.record;
        s.ask = None;
        s.options = options;
        s.cpu_now = None;
        let targets = plan.bake.clone();
        let total = targets.len();
        s.phase = Phase::Preparing(Box::new(Prep {
            total,
            plan,
            options,
            started: Instant::now(),
            cpu_before: s.cpu,
            epoch: self.epoch,
            master: None,
            master_failed: None,
        }));
        for target in targets {
            self.freeze_target(target);
        }
        self.set_status(tf("record.preparing", &[("n", &total)]));
        Ok(())
    }

    /// Stop a preparation: what was frozen is unfrozen (off the history), the renders kept.
    pub fn record_mode_cancel(&mut self) {
        let Phase::Preparing(p) = std::mem::take(&mut self.freeze.record.phase) else {
            return;
        };
        self.cancel_freeze();
        if let Some(m) = &p.master {
            m.progress.cancel();
            // Its plug-ins' render instances, prepared or loading, go with it.
            self.plugins.drop_render(cantus_host::RenderFor::Freeze);
        }
        let undo = unfreezes(self.project(), &p.plan.bake);
        apply_session(self, undo);
        self.set_status(t("record.cancelled"));
    }

    /// *Leave*: the lock off, what the mode froze unfrozen (off the history: nothing to
    /// undo); the renders kept or discarded (`discard`). Refused while a take is written.
    pub fn record_mode_leave(&mut self, discard: bool) -> Result<(), String> {
        match &self.freeze.record.phase {
            Phase::Idle => return Err(t("record.not_on")),
            Phase::Preparing(_) => {
                self.record_mode_cancel();
                return Ok(());
            }
            Phase::Active(_) => {}
        }
        if self.is_recording() {
            return Err(t("record.cannot.leave_recording"));
        }
        self.leave_now(discard);
        self.set_status(t(if discard {
            "record.left_discarded"
        } else {
            "record.left"
        }));
        Ok(())
    }

    fn leave_now(&mut self, discard: bool) {
        let Phase::Active(a) = std::mem::take(&mut self.freeze.record.phase) else {
            return;
        };
        self.bus.set_record_lock(None);
        self.engine.frozen.set_master(None);
        self.freeze.record.leaving = false;
        self.freeze.record.offer = None;
        let rate = self.engine.sample_rate.round() as u32;
        let mut keys: Vec<String> = a
            .baked
            .iter()
            .filter_map(|t| self.project().frozen(*t).map(|f| f.content.clone()))
            .collect();
        keys.extend(a.master.clone());
        let undo = unfreezes(self.project(), &a.baked);
        apply_session(self, undo);
        if discard {
            for key in &keys {
                for path in self.engine.frozen.paths_of(key, rate) {
                    let _ = std::fs::remove_file(path);
                }
                self.engine.frozen.forget(key, rate);
            }
            self.engine.frozen.forget_missing();
        }
        self.freeze.record.cpu_now = None;
        crate::freeze::apply_low_latency(self);
        self.force_resync();
    }

    /// Say that the mode refuses an edit.
    pub fn record_mode_notice(&mut self) {
        self.set_error(t("record.notice"));
    }

    /// Say that the mode refuses an edit, in `key`'s words (`notice_key`).
    pub fn record_mode_notice_for(&mut self, key: &str) {
        self.set_error(t(key));
    }

    /// An arm the mode refused: offer to bring the track's strip live (the user's), or
    /// do it at once (`now`: an agent asked, the call already allowed). Whether it was
    /// handled so.
    pub fn record_mode_arm_refused(&mut self, track: TrackId, now: bool) -> bool {
        if self.baked_holding(track).is_empty() {
            return false;
        }
        if now {
            return self.record_mode_bring_live(track).is_ok();
        }
        self.freeze.record.offer = Some(track);
        true
    }

    /// The mode's baked targets whose render holds `track`.
    pub fn baked_holding(&self, track: TrackId) -> Vec<FreezeTarget> {
        let Some(a) = self.freeze.record.active() else {
            return Vec::new();
        };
        a.baked
            .iter()
            .copied()
            .filter(|t| freeze::region(self.project(), *t).tracks.contains(&track))
            .collect()
    }

    /// Bring live what holds `track` — only the baked renders that hold it, no render
    /// made again: their live chains resume, compensated to the sample — and arm it. The
    /// unfreeze is the session's; the arm is an undo step as ever. A baked master holds the
    /// whole mix: it plays live again (its strips from their renders).
    pub fn record_mode_bring_live(&mut self, track: TrackId) -> Result<(), String> {
        let holding = self.baked_holding(track);
        if holding.is_empty() {
            return Err(t("record.not_baked"));
        }
        let undo = unfreezes(self.project(), &holding);
        apply_session(self, undo);
        let Phase::Active(a) = &mut self.freeze.record.phase else {
            return Err(t("record.not_on"));
        };
        a.baked.retain(|t| !holding.contains(t));
        for t in &holding {
            a.live.push((*t, Why::Armed(track)));
        }
        if a.master.take().is_some() {
            a.options.master_live = true;
            self.engine.frozen.set_master(None);
        }
        let (options, live) = (a.options, a.live.clone());
        let lock = lock_for(self.project(), options, &live);
        self.bus.set_record_lock(Some(lock));
        self.freeze.record.offer = None;
        self.force_resync();
        if !self.exec(Command::SetTrackArmed {
            id: track,
            armed: true,
        }) {
            return Err(self.status.text.clone());
        }
        let name = self
            .project()
            .track(track)
            .map(|t| t.name.clone())
            .unwrap_or_default();
        self.set_status(tf("record.brought_live", &[("name", &name)]));
        Ok(())
    }
}

/// Once a frame: the load measured, a preparation carried to the mode, the mode ended when
/// another project takes the place of this one or the tempo starts moving without it.
pub fn tick(app: &mut CantusApp) {
    if app.engine.is_playing() {
        let load = app.engine.stats().load;
        let s = &mut app.freeze.record;
        let smooth = |v: &mut Option<f32>| {
            *v = Some(match *v {
                Some(old) => old + (load - old) * 0.05,
                None => load,
            })
        };
        if !s.on() {
            smooth(&mut s.cpu);
        } else if s.is_active() {
            smooth(&mut s.cpu_now);
        }
    }
    let epoch = match &app.freeze.record.phase {
        Phase::Idle => return,
        Phase::Preparing(p) => p.epoch,
        Phase::Active(a) => a.epoch,
    };
    if epoch != app.epoch {
        app.freeze.record.phase = Phase::Idle;
        app.freeze.record.leaving = false;
        app.freeze.record.ask = None;
        app.freeze.record.offer = None;
        app.bus.set_record_lock(None);
        app.engine.frozen.set_master(None);
        crate::freeze::apply_low_latency(app);
        return;
    }
    // A sync source followed, a scene playing: the renders would play at another tempo.
    if let Some(why) = tempo_moves(app) {
        if app.is_recording() {
            app.stop_recording();
        }
        match &app.freeze.record.phase {
            Phase::Preparing(_) => app.record_mode_cancel(),
            _ => app.leave_now(false),
        }
        app.set_error(tf("record.left_tempo", &[("why", &why)]));
        return;
    }
    if !app.freeze.record.is_preparing() || app.freeze.busy() || app.freeze.is_asking() {
        return;
    }
    // The strips' renders are in: the master's next, when it is baked.
    if master_step(app) {
        return;
    }
    finish_preparing(app);
}

/// The baked master's render: started once the strips' are in, collected when done.
/// Whether it still runs.
fn master_step(app: &mut CantusApp) -> bool {
    let (wants, started) = match &app.freeze.record.phase {
        Phase::Preparing(p) => (
            !p.options.master_live && p.master_failed.is_none(),
            p.master.is_some(),
        ),
        _ => return false,
    };
    if !wants {
        return false;
    }
    if !started {
        let Phase::Preparing(p) = &app.freeze.record.phase else {
            return false;
        };
        let live = live_roots(app.project(), &p.plan);
        let project = app.bus.project().clone();
        let muted = cantus_core::record_mode::muted(&project, &live);
        let job = cantus_engine::record_mode::prepare_master(&mut app.engine, &project, &live);
        if let Phase::Preparing(p) = &mut app.freeze.record.phase {
            p.master = Some(MasterRun {
                key: job.key.clone(),
                progress: Arc::new(cantus_engine::RenderProgress::default()),
                worker: None,
                pending: Some((job, muted)),
            });
        }
    }
    // Its hosted plug-ins' render instances first — the master's, the live strips' — from
    // the states the project stores, as a freeze's (round 36, the integrated review: a hosted
    // limiter on the master vanished from the render): it waits, frame by frame, while they
    // load; one that cannot be had keeps the master live, named.
    let pending = match &mut app.freeze.record.phase {
        Phase::Preparing(p) => p.master.as_mut().and_then(|m| m.pending.take()),
        _ => None,
    };
    if let Some((job, muted)) = pending {
        let snap = job.snapshot().clone();
        match crate::freeze::units_for(app, &muted, &snap, true, true) {
            Ok((units, insts, _wires)) => {
                let Phase::Preparing(p) = &mut app.freeze.record.phase else {
                    return false;
                };
                let Some(m) = p.master.as_mut() else {
                    return false;
                };
                let pr = m.progress.clone();
                m.worker = std::thread::Builder::new()
                    .name("cantus-master-bake".into())
                    .spawn(move || job.run_with(units, insts, &pr))
                    .ok();
                return true;
            }
            Err(e) if e == crate::freeze::PLUGINS_LOADING => {
                if let Phase::Preparing(p) = &mut app.freeze.record.phase {
                    if let Some(m) = p.master.as_mut() {
                        m.pending = Some((job, muted));
                    }
                }
                return true;
            }
            Err(e) => {
                app.plugins.finish_render();
                if let Phase::Preparing(p) = &mut app.freeze.record.phase {
                    p.master = None;
                    p.master_failed = Some(e);
                }
                return false;
            }
        }
    }
    let Phase::Preparing(p) = &mut app.freeze.record.phase else {
        return false;
    };
    let Some(m) = p.master.as_mut() else {
        return false;
    };
    match m.worker.as_ref() {
        Some(w) if !w.is_finished() => return true,
        _ => {}
    }
    let result = match m.worker.take() {
        Some(w) => w
            .join()
            .unwrap_or_else(|_| Err("the master render stopped".into())),
        None => Err("no thread for the master render".into()),
    };
    // An isolated plug-in that stopped during the render: the render lacks it (as a
    // freeze's would), so the master stays live, named.
    let lost = app.plugins.render_failures();
    app.plugins.finish_render();
    let Phase::Preparing(p) = &mut app.freeze.record.phase else {
        return false;
    };
    let result = match (result, lost.first()) {
        (Ok(path), Some(f)) => {
            let _ = std::fs::remove_file(&path);
            Err(f.name.clone())
        }
        (r, _) => r,
    };
    if let Err(e) = result {
        p.master = None;
        p.master_failed = Some(e);
    }
    false
}

/// What stays live of `plan` (and what failed, which plays live too): muted in the baked
/// master's render.
fn live_roots(project: &Project, plan: &Plan) -> Vec<FreezeTarget> {
    let mut v: Vec<FreezeTarget> = plan.live.iter().map(|(t, _)| *t).collect();
    v.extend(plan.bake.iter().filter(|t| project.frozen(**t).is_none()));
    v
}

/// Every render is done (or failed): the mode begins with what was baked.
fn finish_preparing(app: &mut CantusApp) {
    let Phase::Preparing(p) = std::mem::take(&mut app.freeze.record.phase) else {
        return;
    };
    let mut baked = Vec::new();
    let mut failed = Vec::new();
    for target in &p.plan.bake {
        if app.project().frozen(*target).is_some() {
            baked.push(*target);
        } else {
            // Its own failure, by its key, or the plain words (review: never another's).
            let why = app
                .freeze
                .failure_of(app.project(), *target)
                .unwrap_or_else(|| t("record.failed.unknown"));
            failed.push((*target, why));
        }
    }
    let mut options = p.options;
    // A baked master that failed: the master stays live, said.
    let master = p.master.as_ref().map(|m| m.key.clone());
    if p.master_failed.is_some() {
        options.master_live = true;
    }
    if options.stop_on_failure && !failed.is_empty() {
        let undo = unfreezes(app.project(), &baked);
        apply_session(app, undo);
        let names = failed
            .iter()
            .map(|(t, _)| crate::freeze::name_of(app.project(), *t))
            .collect::<Vec<_>>()
            .join(", ");
        app.set_error(tf("record.stopped", &[("names", &names)]));
        return;
    }
    if baked.is_empty() {
        app.set_error(t("record.none_baked"));
        return;
    }
    let rate = app.engine.sample_rate.round() as u32;
    let bytes = baked
        .iter()
        .filter_map(|t| freeze::content_key(app.project(), *t))
        .chain(master.clone())
        .filter_map(|k| app.engine.frozen.path_of(&k, rate))
        .filter_map(|path| std::fs::metadata(path).ok())
        .map(|m| m.len())
        .sum();
    let live = p.plan.live.clone();
    app.bus
        .set_record_lock(Some(lock_for(app.project(), options, &live)));
    app.engine.frozen.forget_missing();
    app.engine.frozen.set_master(master.clone());
    app.freeze.refusal = None;
    app.freeze.record.cpu_now = None;
    let n = baked.len();
    app.freeze.record.phase = Phase::Active(Box::new(Active {
        options,
        baked,
        already: p.plan.already.len(),
        live,
        failed,
        since: Instant::now(),
        bytes,
        cpu_before: p.cpu_before,
        epoch: app.epoch,
        master,
    }));
    crate::freeze::apply_low_latency(app);
    app.force_resync();
    app.set_status(tf("record.entered", &[("n", &n)]));
    // A master that could not be baked stays live: said last, so it is what is read.
    if let Some(why) = &p.master_failed {
        app.set_error(tf("record.master_failed", &[("why", why)]));
    }
}

/// A project about to be written: the targets the mode froze thawed in the copy, so a file
/// is never written out of the mode's own state (a crash reopens out of the mode).
pub fn as_saved(app: &CantusApp, project: &mut Project) {
    let list: &[FreezeTarget] = match &app.freeze.record.phase {
        Phase::Active(a) => &a.baked,
        Phase::Preparing(p) => &p.plan.bake,
        Phase::Idle => return,
    };
    for target in list {
        match target {
            FreezeTarget::Strip(s) => {
                if let Some(s) = project.strip_mut(*s) {
                    s.frozen = None;
                }
            }
            FreezeTarget::Track(t) => {
                if let Some(t) = project.track_mut(*t) {
                    t.frozen = None;
                }
            }
        }
    }
}

/// The low-latency limit the mode forces on (the setting's, whether or not the user
/// switched it on): armed strips are monitored without waiting for the baked ones.
pub fn forced_limit(app: &CantusApp) -> Option<u32> {
    if !app.freeze.record.is_active() {
        return None;
    }
    let ms = app.settings.low_latency_ms.max(1.0);
    Some((f64::from(ms) * app.engine.sample_rate / 1000.0).round() as u32)
}

/// The round trip the bar shows: frames, and whether it was measured (a ping out and back)
/// or is what the driver reports. **The seam** for a measured figure (r36/external's
/// automatic latency ping): return it here with `measured: true`.
pub fn round_trip(app: &CantusApp) -> Option<(u32, bool)> {
    let frames = app.engine.device_latency().round_trip();
    (frames > 0).then_some((frames, false))
}

/// Shots only: the mode as it looks once prepared (`preparing`: half way), with the plan's
/// targets frozen in the model and no render made. Never called by the program.
#[doc(hidden)]
pub fn stage_for_shot(app: &mut CantusApp, preparing: bool) {
    let plan = record_mode::plan(app.project());
    let options = Options::default();
    if preparing {
        app.freeze.record.phase = Phase::Preparing(Box::new(Prep {
            total: plan.bake.len(),
            plan,
            options,
            started: Instant::now(),
            cpu_before: Some(0.62),
            epoch: app.epoch,
            master: None,
            master_failed: None,
        }));
        return;
    }
    let mut cmds = Vec::new();
    for t in &plan.bake {
        if let Ok(c) = freeze::freeze_commands(app.project(), *t) {
            cmds.push(c);
        }
    }
    apply_session(app, cmds);
    let live = plan.live.clone();
    app.bus
        .set_record_lock(Some(lock_for(app.project(), options, &live)));
    app.freeze.record.cpu_now = Some(0.07);
    app.freeze.record.phase = Phase::Active(Box::new(Active {
        options,
        baked: plan.bake.clone(),
        already: plan.already.len(),
        live,
        failed: Vec::new(),
        since: Instant::now(),
        bytes: 0,
        cpu_before: Some(0.62),
        epoch: app.epoch,
        master: None,
    }));
}
