//! The session's conductor (ADR 0076 §7): the presses of the session view, keyboard, MIDI
//! learn and controller grids — sent as `EngineMessage::Session`, which the engine handle
//! keeps for it — become a performance (`cantus_core::session::play`), heard as its
//! projection: the song with what plays written into the arrangement, compiled and sent as
//! any snapshot is, ahead of the boundary it plays from. The view hears back what changes
//! (`EngineHandle::take_session_events`) when it is heard.
//!
//! **The lead.** A press takes effect no sooner than a lead after the playhead the engine is
//! rendering (`engine_position`, already the output latency ahead of what is heard): two of
//! the device's blocks (the snapshot reaches the audio thread at the next one and is
//! installed on the automation grid inside it), the measured worst of the last compiles of
//! a projection, and a small floor ([`LEAD_FLOOR_S`]). A compile that ran past its press's
//! tick anyway is planned again from where it can still land, before it is sent — a quantised
//! launch then plays on the boundary it meant, caught up (the launch tolerance), never a
//! whole bar later; what was planned is what is heard, and what arrangement record writes.
//!
//! **Planning on.** Every plan after the first carries on from the last one as it stood
//! before the lead (`plan_from`): an edit of a follow chance, a loop or a scene, or a press,
//! changes what comes, never what was heard. Looping clips are open-ended in the projection
//! (`open_ended`): an interface that stalls past the horizon leaves them looping.
//!
//! While the transport is stopped a launch starts it, at once, from the playhead. A stop
//! (or a jump of the playhead) ends the performance: with *arrangement record* on, what was
//! heard is written into the arrangement as one undo step — the very clips that played.

use crate::app::CantusApp;
use crate::i18n::t;
use cantus_core::session::edit::{commit_performance, Performance};
use cantus_core::session::play::{open_ended, plan_from, project_play, Change, Input, Plan, Timed};
use cantus_core::session::{SceneId, SlotRef};
use cantus_core::{Project, Snapshot, Ticks, TrackId};
use cantus_engine::messages::{ScenePhase, SessionEvent, SessionMsg, SlotPhase};
use std::time::{Duration, Instant};

/// The lead's floor, seconds, beyond the blocks and the compile.
pub const LEAD_FLOOR_S: f64 = 0.002;
/// How long a launch from stop waits for the engine to say it plays, seconds.
pub const STARTING_S: f64 = 2.0;
/// What a compile is taken to cost before one was measured, seconds.
pub const FIRST_COMPILE_S: f64 = 0.02;
/// How many compiles the running estimate keeps (it takes their 95th percentile).
const COMPILES: usize = 64;
/// The planning horizon, and how far past it looping clips are carried (bars).
pub const HORIZON_BARS: i64 = 32;
pub const OPEN_BARS: i64 = 64;

/// The conductor's state: the presses of the performance playing, its plan, and what the
/// view has been told.
#[derive(Debug, Default)]
pub struct Conductor {
    inputs: Vec<Timed>,
    plan: Plan,
    horizon: Ticks,
    /// The song revision the plan was made against.
    planned_rev: u64,
    /// The projection, for the song revision it was made from.
    projected: Option<(u64, Project)>,
    /// How many of `plan.changes` have been told.
    told: usize,
    /// The presses whose queued (or stopping) state has been told.
    queued_told: usize,
    was_playing: bool,
    /// Play was asked for a launch from stop and the engine has not said it plays yet:
    /// the handle's stop count then, and when. A stop since, or [`STARTING_S`] with no word
    /// from the engine (no device), gives up waiting.
    starting: Option<(u64, Instant)>,
    last_tick: Ticks,
    /// The frame's start: the engine's position then, and when.
    frame: Option<(Instant, Ticks)>,
    /// The last compiles of a projection, seconds.
    compiles: Vec<f64>,
    /// The first press no projection sent has held yet.
    unsent: usize,
    /// Added to every compile's time (tests: a slow compile).
    slow: Duration,
    /// The last projection compiled, and its snapshot: what the next one reuses.
    last: Option<(Project, std::sync::Arc<Snapshot>)>,
    /// Arrangement record: on since this tick.
    pub arrangement_record: Option<Ticks>,
    /// A recording into a slot (`session_play_rec.rs`).
    pub(crate) recording: Option<crate::session_play_rec::SlotRecording>,
    /// Start the transport once the projection is sent (a launch from stop).
    start: bool,
    /// The scene the view last saw playing.
    scene: Option<SceneId>,
    /// The copies of audio takes' beginnings laid ahead (`session_play_rec.rs`), removed
    /// when the performance ends.
    pub(crate) heads: Vec<std::path::PathBuf>,
    /// The engine played at the last frame (a jump back is a locate only while it did).
    rolling: bool,
    /// The handle's seek count at the last frame (a jump back with none is the source's).
    seeks: u64,
}

/// What the followed source (Link, MIDI clock, time code) is doing, as the seam says.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) struct SyncNow {
    /// A source is chosen and the engine has a seam: a play waits for its moment.
    pub on: bool,
    /// The transport follows it (locked, or pulling in).
    pub following: bool,
    /// The tempo the transport plays at while it follows.
    pub tempo: Option<f64>,
}

impl Conductor {
    /// Whether a performance is under way.
    pub fn active(&self) -> bool {
        !self.inputs.is_empty()
    }

    /// The plan heard so far.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// The recording into a slot under way, if any.
    pub fn recording(&self) -> Option<&crate::session_play_rec::SlotRecording> {
        self.recording.as_ref()
    }

    /// What a compile of a projection is taken to cost, seconds: the 95th percentile of the
    /// last [`COMPILES`] (nearest rank — one slow compile among 64 does not set the lead
    /// for the next 64; with fewer than 20 it is their worst).
    pub fn compile_s(&self) -> f64 {
        if self.compiles.is_empty() {
            return FIRST_COMPILE_S;
        }
        let mut v = self.compiles.clone();
        v.sort_by(f64::total_cmp);
        let rank = (v.len() * 95).div_ceil(100).max(1);
        v[rank - 1]
    }

    /// A compile of a projection while it plays took `s` seconds (not the full compile of a
    /// launch from stop, nor a compile again after one that ran late: they are not what the
    /// next press waits for).
    pub fn compiled(&mut self, s: f64) {
        if self.compiles.len() == COMPILES {
            self.compiles.remove(0);
        }
        self.compiles.push(s);
    }
}

/// The lead, seconds, at `block` frames and `rate`, with compiles that take `compile_s`.
pub fn lead_s(block: usize, rate: f64, compile_s: f64) -> f64 {
    2.0 * block as f64 / rate.max(1.0) + compile_s + LEAD_FLOOR_S
}

/// What a press is to the planner (recording and arrangement record are the conductor's).
fn input_of(m: SessionMsg) -> Option<Input> {
    Some(match m {
        SessionMsg::Launch { slot, velocity } => Input::Launch { slot, velocity },
        SessionMsg::Release { slot } => Input::Release { slot },
        SessionMsg::LaunchScene { scene, velocity } => Input::LaunchScene { scene, velocity },
        SessionMsg::ReleaseScene { scene } => Input::ReleaseScene { scene },
        SessionMsg::StopTrack { track } => Input::StopTrack { track },
        SessionMsg::StopAll => Input::StopAll,
        SessionMsg::BackToArrangement { track } => Input::Back { track },
        _ => return None,
    })
}

/// The project the engine compiles: the projection while a performance plays, `fallback`
/// (pattern mode's) otherwise. Cached per song revision and plan.
pub fn play(app: &mut CantusApp, fallback: Option<Project>) -> Option<Project> {
    if app.perform.plan.perf.is_empty() {
        return fallback;
    }
    let rev = app.bus.revision();
    if app
        .perform
        .projected
        .as_ref()
        .is_none_or(|(r, _)| *r != rev)
    {
        let song = app.plan_project();
        let perf = open_ended(
            &song,
            &app.perform.plan.perf,
            app.perform.horizon,
            OPEN_BARS,
        );
        let p = project_play(&song, &perf);
        app.perform.projected = Some((rev, p));
    }
    app.perform.projected.as_ref().map(|(_, p)| p.clone())
}

/// The snapshot the engine plays now (`CantusApp::sync_engine`): pattern mode's, the
/// count-in's, the session's projection or the song — and, while a performance plays, a
/// projection whose compile ran past a press's tick planned again from where it can land
/// and compiled once more before it is sent (D2: what is planned is what is heard).
pub fn compile_to_play(app: &mut CantusApp, rev: u64) -> std::sync::Arc<Snapshot> {
    let mut tries = 0;
    loop {
        let t0 = Instant::now();
        let mut incremental = false;
        let pattern = crate::views::transport::pattern::project_to_play(app);
        let pattern = play(app, pattern);
        let session = app.perform.active() && pattern.is_some();
        let compiled = match crate::count_in::project_to_compile(
            pattern,
            app.bus.project(),
            app.rec.count_in_click,
        ) {
            // A projection compiles only the tracks it changed since the last (D3).
            Some(p) if session => {
                let prev = app.perform.last.take();
                incremental = prev.is_some();
                let mut s =
                    app.engine
                        .compile_reusing(&p, rev, prev.as_ref().map(|(q, s)| (q, &**s)));
                // The scenes' tempos, for the engine to propose to a followed Link session
                // as each is heard (seam 7). The snapshot was just made: no one else holds it.
                if let Some(x) = std::sync::Arc::get_mut(&mut s) {
                    x.scene_tempos.clone_from(&app.perform.plan.perf.tempos);
                }
                app.perform.last = Some((p, s.clone()));
                s
            }
            Some(p) => app.engine.compile(&p, rev),
            None => app.engine.compile(app.bus.project(), rev),
        };
        let snapshot = compiled;
        if !session {
            app.perform.last = None;
            return snapshot;
        }
        if tries == 0 && incremental {
            app.perform.compiled(t0.elapsed().as_secs_f64());
        }
        tries += 1;
        if tries > 2 || !app.restamp_late() {
            app.perform.unsent = app.perform.inputs.len();
            return snapshot;
        }
    }
}

impl CantusApp {
    /// The song the performance is planned on: with a slot recording's MIDI clip laid
    /// ahead of its end in its slot (`session_play_rec.rs`).
    pub(crate) fn plan_project(&self) -> std::borrow::Cow<'_, Project> {
        let ahead = self.perform.recording.as_ref().and_then(|r| {
            r.ahead
                .as_ref()
                .map(|c| (r.slot, c, r.ahead_source.as_ref()))
        });
        match ahead {
            Some((slot, c, source)) if self.project().slot_clip(slot).is_none() => {
                let mut p = self.project().clone();
                // An audio take's beginning, copied aside: what its clip plays until the
                // take is whole.
                if let Some(src) = source {
                    p.audio_pool.push(src.clone());
                }
                if let Some(t) = p.track_mut(slot.track) {
                    let at = t.slots.partition_point(|s| s.scene < slot.scene);
                    match t.slots.get_mut(at).filter(|s| s.scene == slot.scene) {
                        Some(s) => s.clip = Some(c.clone()),
                        None => t.slots.insert(
                            at,
                            cantus_core::session::Slot {
                                scene: slot.scene,
                                clip: Some(c.clone()),
                                stop: true,
                            },
                        ),
                    }
                }
                std::borrow::Cow::Owned(p)
            }
            _ => std::borrow::Cow::Borrowed(self.project()),
        }
    }

    /// What the projection is made of changed (a clip laid ahead, a take landed).
    pub(crate) fn perform_projection_changed(&mut self) {
        self.perform.projected = None;
        self.force_resync();
    }

    /// The tempo map the performance plays through (its scenes' tempos and metres in it).
    pub(crate) fn play_map(&self) -> cantus_core::TempoMap {
        match &self.perform.projected {
            Some((_, p)) => p.tempo_map(),
            None => self.project().tempo_map(),
        }
    }

    /// The lead now, in ticks from `at`.
    /// The device's block and rate (the handle's rate before the engine said its own).
    fn device_block(&self) -> (usize, f64) {
        let stats = self.engine.stats();
        let rate = if stats.sample_rate > 0.0 {
            stats.sample_rate
        } else {
            self.engine.sample_rate.max(1.0)
        };
        (stats.block_frames.max(1), rate)
    }

    fn lead_ticks(&self, at: Ticks) -> Ticks {
        let (block, rate) = self.device_block();
        let s = lead_s(block, rate, self.perform.compile_s());
        Ticks::from_seconds(s, self.heard_bpm(at)).max(Ticks(1))
    }

    /// The followed source as the seam says (`SyncNow`).
    pub(crate) fn sync_now(&self) -> SyncNow {
        use crate::views::transport::sync_service::SyncSource;
        use cantus_engine::transport::sync::Lock;
        if self.settings.sync.source == SyncSource::None {
            return SyncNow::default();
        }
        let Some(sh) = self.sync.shared() else {
            return SyncNow::default();
        };
        let tempo = sh.tempo();
        let tempo = (tempo > 0.0 && tempo.is_finite()).then_some(tempo);
        SyncNow {
            on: true,
            following: matches!(sh.lock_state(), Lock::Locked | Lock::Seeking) || tempo.is_some(),
            tempo,
        }
    }

    /// The tempo the playhead moves at by `at`: the followed source's while the transport
    /// follows one (its ticks go at that tempo, whatever the song's map says), the map's
    /// otherwise — what seconds are turned into ticks with (the lead, the frame's advance).
    pub(crate) fn heard_bpm(&self, at: Ticks) -> f64 {
        self.sync_now()
            .tempo
            .unwrap_or_else(|| self.play_map().bpm_at(at.0 as f64))
    }

    /// A launch from stop waits for the followed source to start the transport.
    fn sync_start_pending(&self) -> bool {
        (self.perform.start || self.perform.starting.is_some()) && self.sync_now().on
    }

    /// The presses made from stop, planned again from `at`: where the followed source
    /// located the playhead (`now`: still at once there), or — the source started the
    /// transport elsewhere than they were planned — from where it plays now, caught up
    /// from `pressed` (the start, as heard). The plan is made again from nothing.
    fn replan_from_stop(&mut self, at: Ticks, now: bool, pressed: Ticks) {
        let mut moved = false;
        for t in self.perform.inputs.iter_mut().filter(|t| t.now) {
            if (t.at, t.now) != (at, now) {
                *t = Timed {
                    at,
                    now,
                    input: t.input,
                    pressed,
                };
                moved = true;
            }
        }
        if !moved {
            return;
        }
        self.perform.plan = Plan::default();
        self.perform.told = 0;
        self.perform.queued_told = 0;
        self.perform.unsent = 0;
        self.perform.horizon = self.play_map().add_bars(at, HORIZON_BARS);
        self.replan_from(at);
    }

    /// Where the engine renders now, estimated from the frame's start.
    fn engine_now(&self) -> Ticks {
        let Some((t0, tick)) = self.perform.frame else {
            return self.engine.engine_position();
        };
        if !self.engine.is_playing() {
            return tick;
        }
        let bpm = self.heard_bpm(tick);
        tick + Ticks::from_seconds((t0.elapsed() + self.perform.slow).as_secs_f64(), bpm)
    }

    /// The presses whose tick a compile overran: planned again no sooner than the engine
    /// can install it (two blocks on). Whether any was.
    fn restamp_late(&mut self) -> bool {
        if !self.engine.is_playing() {
            return false;
        }
        let (block, rate) = self.device_block();
        let now = self.engine_now();
        let bpm = self.heard_bpm(now);
        let two = Ticks::from_seconds(2.0 * block as f64 / rate, bpm);
        let earliest = now + two;
        let mut cut: Option<Ticks> = None;
        let unsent = self.perform.unsent;
        for (i, t) in self.perform.inputs.iter_mut().enumerate().skip(unsent) {
            let Some(e) = self.perform.plan.effect.get(i).copied().flatten() else {
                continue;
            };
            if t.now || e >= earliest {
                continue;
            }
            t.at = t.at.max(earliest);
            cut = Some(cut.map_or(e, |c| c.min(e)));
        }
        if let Some(cut) = cut {
            self.replan_from(cut);
        }
        cut.is_some()
    }

    /// The conductor's frame, before the engine is synced: the presses taken, the plan
    /// carried to its horizon, what was heard told to the view.
    pub fn perform_frame(&mut self) {
        let msgs = self.engine.take_session_msgs();
        let playing = self.engine.is_playing();
        let now = self.engine.engine_position();
        let sync = self.sync_now();
        // A launch from stop gives up waiting on a stop, or after `STARTING_S` with no word
        // from an engine — never while a followed source holds the start (Link's quantum,
        // the master's Start: as long as it takes).
        let gave_up = self.perform.starting.is_some_and(|(stops, since)| {
            stops != self.engine.stops() || (!sync.on && since.elapsed().as_secs_f64() > STARTING_S)
        });
        let started = playing && self.perform.starting.is_some();
        if !playing && !gave_up && self.perform.starting.is_some() {
            // The source located the playhead while its start waits (a song position):
            // the launch from stop plays from there.
            let at = self.engine.position();
            self.replan_from_stop(at, true, at);
        }
        if playing || gave_up {
            self.perform.starting = None;
        }
        // Stopped, or the playhead jumped back while it played: the performance is over
        // (not while a launch from stop waits for the engine to say it plays; and a launch
        // from stop stopped before it played, or with no engine to play it, is over too).
        // A jump back while a source is followed and no locate was sent is the source's —
        // its start, a relock in Link's quantum, the master's song position — not a locate.
        let seeked = self.engine.seeks() != self.perform.seeks;
        let jumped = playing
            && self.perform.rolling
            && now < self.perform.last_tick
            && (seeked || !sync.following);
        let stopped = !playing || jumped;
        if (self.perform.active() || self.perform.recording.is_some())
            && ((self.perform.was_playing && self.perform.starting.is_none() && stopped)
                || (gave_up && !playing))
        {
            self.end_performance(self.perform.last_tick);
        }
        self.perform.was_playing = playing || self.perform.starting.is_some();
        self.perform.rolling = playing;
        self.perform.seeks = self.engine.seeks();
        if started && sync.on && self.perform.active() {
            // The source started the transport behind where the launch from stop was
            // planned (the master's Start from the top): it plays from where it is heard.
            let heard = self.engine.position();
            let planned = self
                .perform
                .inputs
                .iter()
                .filter(|t| t.now)
                .map(|t| t.at)
                .min();
            // (Link starts where it was planned, in phase: the engine renders from there; a
            // sixteenth's margin keeps a start at the planned tick from reading as behind.)
            if planned.is_some_and(|p| now + Ticks(cantus_core::PPQ / 4) < p) {
                self.perform.frame = Some((Instant::now(), now));
                let at = now + self.lead_ticks(now);
                self.replan_from_stop(at, false, heard);
            }
        }
        self.perform.last_tick = now;
        self.perform.frame = Some((Instant::now(), now));
        let mut changed = false;
        for m in msgs {
            changed |= self.perform_press(m);
        }
        if self.perform.active() {
            let map = self.play_map();
            let bar = map.bar_len_at(now).max(Ticks(1));
            let edited = self.bus.revision() != self.perform.planned_rev;
            if changed || edited || now + bar * (HORIZON_BARS / 2) > self.perform.horizon {
                self.perform.horizon = map.add_bars(now, HORIZON_BARS);
                let cut = now + self.lead_ticks(now);
                self.replan_from(cut);
            }
        }
        self.record_frame();
        self.tell_heard();
    }

    /// After the engine was synced: a launch from stop starts the transport, once the
    /// projection it plays is with the engine.
    pub fn perform_after_sync(&mut self) {
        if std::mem::take(&mut self.perform.start) && !self.engine.is_playing() {
            self.engine.play();
            self.perform.starting = Some((self.engine.stops(), Instant::now()));
        }
    }

    /// A press into the performance (presses come in the order of their ticks).
    pub(crate) fn push_input(&mut self, t: Timed) {
        self.perform.inputs.push(t);
    }

    /// The tick a press made now takes effect from, and whether it is at once.
    pub(crate) fn press_at(&self) -> (Ticks, bool) {
        if !self.engine.is_playing() && self.perform.starting.is_none() {
            return (self.engine.position(), true);
        }
        let now = self.engine_now();
        (now + self.lead_ticks(now), false)
    }

    fn perform_press(&mut self, m: SessionMsg) -> bool {
        match m {
            SessionMsg::Resend => {
                self.resend();
                false
            }
            SessionMsg::SetArrangementRecord(on) => {
                self.set_arrangement_record(on);
                false
            }
            SessionMsg::Record { slot } => self.record_slot(slot),
            SessionMsg::StopRecording { slot } => self.stop_slot_recording(slot),
            // Launching the slot that records ends the recording at the bar line; the clip
            // plays on (`SessionMsg::Record`).
            SessionMsg::Launch { slot, .. }
                if self
                    .perform
                    .recording
                    .as_ref()
                    .is_some_and(|r| r.slot == slot) =>
            {
                self.stop_slot_recording(slot)
            }
            _ => {
                let Some(input) = input_of(m) else {
                    return false;
                };
                // A frozen track plays its render: its clips cannot launch.
                let frozen = match input {
                    Input::Launch { slot, .. } => self
                        .project()
                        .track(slot.track)
                        .is_some_and(|t| t.frozen.is_some()),
                    _ => false,
                };
                if frozen {
                    self.set_status(t("session.frozen_track"));
                    return false;
                }
                let (at, now) = self.press_at();
                if now && !matches!(input, Input::Launch { .. } | Input::LaunchScene { .. }) {
                    // Stopped, a stop or a release has nothing to stop.
                    return false;
                }
                if now {
                    self.perform.start = true;
                }
                let pressed = self.engine.position();
                self.push_input(Timed {
                    at,
                    now,
                    input,
                    pressed: if now { at } else { pressed },
                });
                true
            }
        }
    }

    /// Plan again to the horizon, keeping what was planned before `cut`; a performance
    /// that changed is sent again.
    pub(crate) fn replan_from(&mut self, cut: Ticks) {
        let prior = (!self.perform.plan.perf.is_empty() || !self.perform.plan.effect.is_empty())
            .then(|| (std::mem::take(&mut self.perform.plan), cut));
        let song = self.plan_project().into_owned();
        let p = plan_from(
            &song,
            &self.perform.inputs,
            self.perform.horizon,
            prior.as_ref().map(|(p, c)| (p, *c)),
        );
        let old = prior.map(|(p, _)| p).unwrap_or_default();
        if p.perf != old.perf {
            self.perform.projected = None;
            self.force_resync();
        }
        self.perform.planned_rev = self.bus.revision();
        // The changes already told keep their place: what was planned before the cut is
        // kept as it was.
        self.perform.told = self.perform.told.min(p.changes.len());
        self.perform.plan = p;
        self.tell_queued();
    }

    /// Plan again from the lead on.
    pub(crate) fn replan(&mut self) {
        let now = self.engine_now();
        let cut = now + self.lead_ticks(now);
        self.replan_from(cut);
    }

    #[doc(hidden)]
    pub fn perform_wait(&self) -> Duration {
        Duration::from_secs_f64(self.perform.compile_s())
    }

    /// Tests: a sync seam of the handle's own (the dummy engine has none), the followed
    /// state set on it as the engine would.
    #[doc(hidden)]
    pub fn sync_seam_for_tests(
        &mut self,
    ) -> std::sync::Arc<cantus_engine::transport::sync::SyncShared> {
        self.sync.demo_seam()
    }

    /// Tests: the last projection compiled while performing.
    #[doc(hidden)]
    pub fn projection_for_tests(&self) -> Option<std::sync::Arc<Snapshot>> {
        self.perform.last.as_ref().map(|(_, s)| s.clone())
    }

    /// Tests: every compile takes `d` longer than it does.
    #[doc(hidden)]
    pub fn set_compile_delay_for_tests(&mut self, d: Duration) {
        self.perform.slow = d;
    }
    /// Queued and stopping states for the presses not yet told.
    fn tell_queued(&mut self) {
        let heard = self.engine.position();
        while self.perform.queued_told < self.perform.inputs.len() {
            let i = self.perform.queued_told;
            self.perform.queued_told += 1;
            let Some(at) = self.perform.plan.effect.get(i).copied().flatten() else {
                continue;
            };
            // A launch from stop waiting for the followed source's start is shown queued.
            if at <= heard && !(self.perform.inputs[i].now && self.sync_start_pending()) {
                continue;
            }
            match self.perform.inputs[i].input {
                Input::Launch { slot, .. } => {
                    self.engine.push_session_event(SessionEvent::Slot {
                        slot,
                        phase: SlotPhase::Queued { at },
                    });
                }
                Input::LaunchScene { scene, .. } => {
                    self.engine.push_session_event(SessionEvent::Scene {
                        scene,
                        phase: ScenePhase::Queued { at },
                    });
                }
                Input::StopTrack { track } => self.tell_stopping(Some(track), at),
                Input::StopAll => self.tell_stopping(None, at),
                _ => {}
            }
        }
    }

    /// A stop coming at `at`: each clip of `track` (every track's) that plays then.
    fn tell_stopping(&mut self, track: Option<TrackId>, at: Ticks) {
        let playing: Vec<_> = self
            .perform
            .plan
            .perf
            .played
            .iter()
            .filter(|p| p.at < at && p.until >= at)
            .filter(|p| track.is_none_or(|t| p.slot.track == t))
            .copied()
            .collect();
        for p in playing {
            self.engine.push_session_event(SessionEvent::Slot {
                slot: p.slot,
                phase: SlotPhase::Stopping {
                    since: p.at,
                    from: p.from,
                    at,
                },
            });
        }
    }

    /// The plan's changes the playhead has reached, told.
    fn tell_heard(&mut self) {
        // Nothing is heard while the followed source holds the start.
        if self.sync_start_pending() {
            return;
        }
        let heard = self.engine.position();
        while let Some(&(at, c)) = self.perform.plan.changes.get(self.perform.told) {
            if at > heard {
                break;
            }
            self.perform.told += 1;
            self.tell(at, c);
        }
    }

    fn tell(&mut self, at: Ticks, c: Change) {
        let e = match c {
            Change::Play { slot, from } => SessionEvent::Slot {
                slot,
                phase: SlotPhase::Playing { since: at, from },
            },
            Change::Stop { slot } => SessionEvent::Slot {
                slot,
                phase: SlotPhase::Stopped,
            },
            Change::Arrangement { track, on } => SessionEvent::Track {
                track,
                arrangement: on,
            },
            Change::Scene { scene } => {
                if let Some(old) = self.perform.scene.replace(scene) {
                    if old != scene {
                        self.engine.push_session_event(SessionEvent::Scene {
                            scene: old,
                            phase: ScenePhase::Idle,
                        });
                    }
                }
                SessionEvent::Scene {
                    scene,
                    phase: ScenePhase::Playing { since: at },
                }
            }
            Change::Followed { from, to } => SessionEvent::Followed { from, to, at },
            Change::Tempo { bpm, metre } => {
                // (Following Link, the engine proposes a scene's tempo as its tick is
                // heard: `Snapshot::scene_tempos`.)
                SessionEvent::Tempo {
                    at,
                    bpm: bpm.unwrap_or_else(|| self.project().tempo_map().bpm_at(at.0 as f64)),
                    metre,
                }
            }
        };
        self.engine.push_session_event(e);
    }

    /// The whole state again: every slot that plays, every track the session holds, the
    /// scene, arrangement record.
    fn resend(&mut self) {
        let heard = self.engine.position();
        let perf = self.perform.plan.perf.clone();
        for p in perf
            .played
            .iter()
            .filter(|p| p.at <= heard && p.until > heard)
        {
            self.engine.push_session_event(SessionEvent::Slot {
                slot: p.slot,
                phase: SlotPhase::Playing {
                    since: p.at,
                    from: p.from,
                },
            });
        }
        for t in self
            .project()
            .tracks
            .iter()
            .map(|t| t.id)
            .collect::<Vec<_>>()
        {
            let held = perf
                .held
                .iter()
                .any(|&(x, a, b)| x == t && a <= heard && b > heard);
            self.engine.push_session_event(SessionEvent::Track {
                track: t,
                arrangement: !held,
            });
        }
        if let Some(scene) = self.perform.scene {
            self.engine.push_session_event(SessionEvent::Scene {
                scene,
                phase: ScenePhase::Playing { since: heard },
            });
        }
        self.engine
            .push_session_event(SessionEvent::ArrangementRecord(
                self.perform.arrangement_record.is_some(),
            ));
    }

    fn set_arrangement_record(&mut self, on: bool) {
        match (on, self.perform.arrangement_record) {
            (true, None) => {
                let (at, _) = self.press_at();
                self.perform.arrangement_record = Some(at);
            }
            (false, Some(from)) => {
                let (to, _) = self.press_at();
                self.commit_heard(from, to);
                self.perform.arrangement_record = None;
            }
            _ => {}
        }
        self.engine
            .push_session_event(SessionEvent::ArrangementRecord(on));
    }

    /// Write what was heard in `[from, to)` into the arrangement, one undo step.
    fn commit_heard(&mut self, from: Ticks, to: Ticks) {
        let song = self.bus.project().clone();
        let perf: Performance = self
            .perform
            .plan
            .perf
            .within(from, to, |s| song.slot_clip(s).cloned());
        if perf.is_empty() {
            return;
        }
        if let Some(cmd) = commit_performance(&song, &perf) {
            if self.exec(cmd) {
                self.set_status(t("session.recorded"));
            }
        }
    }

    /// The performance ends at `at` (the transport stopped or jumped): what was heard is
    /// committed with arrangement record on, every clip stops, every track plays its
    /// arrangement again.
    pub(crate) fn end_performance(&mut self, at: Ticks) {
        if let Some(from) = self.perform.arrangement_record.take() {
            self.commit_heard(from, at);
            self.engine
                .push_session_event(SessionEvent::ArrangementRecord(false));
        }
        let perf = std::mem::take(&mut self.perform.plan).perf;
        for p in perf.played.iter().filter(|p| p.at <= at && p.until > at) {
            self.engine.push_session_event(SessionEvent::Slot {
                slot: p.slot,
                phase: SlotPhase::Stopped,
            });
        }
        let held: Vec<TrackId> = perf.held.iter().map(|h| h.0).collect();
        for t in held {
            self.engine.push_session_event(SessionEvent::Track {
                track: t,
                arrangement: true,
            });
        }
        if let Some(scene) = self.perform.scene.take() {
            self.engine.push_session_event(SessionEvent::Scene {
                scene,
                phase: ScenePhase::Idle,
            });
        }
        // A slot recording that had not begun goes with it (review finding 5); one under way
        // finishes into its slot as its pass ends.
        if !self.is_recording() {
            self.perform.recording = None;
        }
        self.perform.inputs.clear();
        // What audio takes laid ahead played: the takes themselves are in their slots.
        if self.perform.recording.is_none() {
            for h in std::mem::take(&mut self.perform.heads) {
                let _ = std::fs::remove_file(h);
            }
        }
        self.perform.unsent = 0;
        self.perform.starting = None;
        self.perform.told = 0;
        self.perform.queued_told = 0;
        self.perform.projected = None;
        self.perform.horizon = Ticks::ZERO;
        self.force_resync();
    }

    /// Whether the slot `slot` plays now (as heard).
    pub fn slot_playing(&self, slot: SlotRef) -> bool {
        let heard = self.engine.position();
        self.perform
            .plan
            .perf
            .played
            .iter()
            .any(|p| p.slot == slot && p.at <= heard && p.until > heard)
    }
}
