//! An isolated plug-in's main half, in Cantus (ADR 0063): every call the host makes on the
//! main thread, answered **without ever waiting on the helper** (round 29, review
//! isolation-5). Each request is sent and its answer lands in the instance's inbox, taken
//! here each frame; a request that takes long shows the plug-in's waiting card.
//!
//! - **Loading** is one request (the plug-in, its state and its values); until it is
//!   answered, the audio half passes dry and `idle` then says [`Notice::Loaded`] — the
//!   host makes the plug-in's unit again with its parameters and latency.
//! - **Values** (`param_value`, called every frame by the generic editor) come from a
//!   mirror the helper keeps current — after every call, and whenever they change.
//! - **Texts** (`param_text`) are cached; one not cached yet is asked for and shown from
//!   the next frame.
//! - **State**: a **last-good state** is kept here — the state it was loaded with, every
//!   state given to it, its answer after load, and a refresh half a second after a change
//!   (the end of a gesture, its own *state changed*) and every ten seconds while its values
//!   move. Whether it is **current** (answered for a request made after the last change) is
//!   known: a save asks for the state anew ([`MainThread::ask_state_now`]) and waits for
//!   the answer frame by frame ([`MainThread::poll_state`]) — up to the request's own limit
//!   from when its helper starts it; one that cannot be had is saved from what it said last,
//!   and the save says so (never a stale state as a fresh one).
//! - **A window** asked for opens when its helper answers (at once in steady state; when a
//!   plug-in's dialog holds the helper, later — [`Notice::EditorOpened`]); a **restart** is
//!   picked up by the frame that finds its answer.
//! - **Crashes**: when the helper ends or a job stays unfinished past the hang limit,
//!   `idle` says [`Notice::Crashed`] once, with the reason.
//! - **The editor** is a top-level window the helper makes and owns (an owned window across
//!   processes would join the two input queues, and a hung helper would freeze Cantus's
//!   window): `editor_is_floating` is true, and bringing it forward and closing it are
//!   proxied.

use super::audio::{BridgeAudio, Offline, Status};
use super::proto::{Call, Described, WireNotice};
use super::sandbox::{Inbox, Purpose, Sandbox, Sandboxes};
use super::slot::Slot;
use crate::backend::{
    AudioThread, Blame, EditorSize, HostError, MainThread, Notice, ParamChange, ParamInfo,
    ParamOut, PluginInfo, StatePoll,
};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How soon after a change the last-good state is refreshed, at most.
const REFRESH_AFTER: Duration = Duration::from_millis(500);
/// How often the state is refreshed while only values move (automation, a knob the plug-in
/// did not report as a gesture).
const REFRESH_EVERY: Duration = Duration::from_secs(10);
/// How long after a failed background state it is asked for again.
const RETRY_AFTER: Duration = Duration::from_secs(2);
/// The most an interface call waits for an answer that usually comes at once (a window
/// opened, a restart): a frame's share, never more (round 29, review isolation-5).
pub const FRAME_WAIT: Duration = Duration::from_millis(30);

/// The main half.
pub struct BridgeMain {
    pub(crate) sandbox: Arc<Sandbox>,
    pub(crate) pool: Arc<Sandboxes>,
    pub(crate) instance: u32,
    pub(crate) slot: Arc<Slot>,
    pub(crate) status: Arc<Status>,
    pub(crate) inbox: Arc<Inbox>,
    pub(crate) offline: bool,
    /// Set to stop an offline instance's render.
    pub(crate) cancel: Arc<AtomicBool>,
    pub(crate) sample_rate: f64,
    info: PluginInfo,
    params: Vec<ParamInfo>,
    latency: usize,
    /// What last made its last-good state not current ([`MainThread::why_changed`]).
    why_changed: Option<&'static str>,
    has_editor: bool,
    /// Its load's request, and whether it was answered (and how).
    load: u64,
    loaded: bool,
    load_failed: Option<String>,
    /// `idle` has yet to say it loaded.
    tell_loaded: bool,
    /// Its audio half made once it loaded, for the host to take ([`MainThread::take_audio`]).
    fresh_audio: Option<Box<dyn AudioThread>>,
    last_good: Option<Vec<u8>>,
    /// The last-good state is the plug-in's as it is now.
    current: bool,
    /// Requests made before this id are older than its last change.
    changed_at: u64,
    /// Something changed since the last-good state was taken, and when it first did.
    dirty_since: Option<Instant>,
    /// The background state asked for, under way.
    asked_state: Option<u64>,
    /// The plug-in said its state changed: said on to the host once the new state is in.
    tell_state_dirty: bool,
    /// To say on at the next `idle`: its state changed (the new one is in), its parameter
    /// list changed (the new one is in).
    say_state_dirty: bool,
    say_params: bool,
    /// The state a save asked for ([`MainThread::ask_state_now`]).
    gather: Option<u64>,
    /// The last state answer: its request, and why it gave none.
    last_answer: Option<(u64, Option<String>)>,
    /// A parameter list asked for (its plug-in said they changed).
    asked_params: bool,
    restart_pending: Option<u64>,
    /// Its window asked for and not answered yet: opened once it is (review isolation-4
    /// #39).
    editor_pending: Option<u64>,
    /// State answers to requests before this id are older than the last state given to it
    /// (review #7, #50).
    state_after: u64,
    last_refresh: Instant,
    told_crash: bool,
    /// The helper's pauses already applied to this instance's job clock.
    pauses_seen: u64,
}

impl std::fmt::Debug for BridgeMain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeMain")
            .field("plugin", &self.info.name)
            .field("sandbox", &self.sandbox.key)
            .field("instance", &self.instance)
            .finish()
    }
}

impl BridgeMain {
    /// The main half of an instance whose load (`load`, carrying `state`) was just sent.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        sandbox: Arc<Sandbox>,
        pool: Arc<Sandboxes>,
        (instance, load): (u32, u64),
        slot: Arc<Slot>,
        (status, inbox): (Arc<Status>, Arc<Inbox>),
        (offline, cancel): (bool, Arc<AtomicBool>),
        sample_rate: f64,
        info: PluginInfo,
        state: &[u8],
    ) -> BridgeMain {
        BridgeMain {
            sandbox,
            pool,
            instance,
            slot,
            status,
            inbox,
            offline,
            cancel,
            sample_rate,
            has_editor: info.editor,
            info,
            params: Vec::new(),
            latency: 0,
            why_changed: None,
            load,
            loaded: false,
            load_failed: None,
            tell_loaded: false,
            fresh_audio: None,
            // The state it is given is what it holds until it says otherwise: the
            // project's, kept if its helper dies before answering.
            last_good: (!state.is_empty()).then(|| state.to_vec()),
            current: true,
            changed_at: 0,
            dirty_since: None,
            asked_state: None,
            tell_state_dirty: false,
            say_state_dirty: false,
            say_params: false,
            gather: None,
            last_answer: None,
            asked_params: false,
            restart_pending: None,
            editor_pending: None,
            state_after: 0,
            last_refresh: Instant::now(),
            told_crash: false,
            pauses_seen: 0,
        }
    }

    /// What an offline audio half needs to wait for its load, and to end a hung or
    /// cancelled render.
    pub(crate) fn offline_parts(&self) -> Option<Offline> {
        self.offline.then(|| Offline {
            sandbox: self.sandbox.clone(),
            instance: self.instance,
            name: self.info.name.clone(),
            cancel: self.cancel.clone(),
        })
    }

    /// The process id of the helper it runs in.
    pub fn pid(&self) -> u32 {
        self.sandbox.pid()
    }

    fn gone(&self) -> bool {
        self.sandbox.is_dead() || self.status.is_dead()
    }

    fn post(&self, call: Call, blob: &[u8], purpose: Purpose) -> u64 {
        self.sandbox.post(Some(self.instance), call, blob, purpose)
    }

    /// Something of the plug-in changed: states asked for before are older.
    fn changed(&mut self, why: &'static str) {
        self.changed_at = self.sandbox.next_request();
        self.current = false;
        self.why_changed = Some(why);
    }

    fn ask_state(&mut self) -> Option<u64> {
        if self.gone() || !self.loaded {
            return None;
        }
        let id = self.post(
            Call::SaveState {
                instance: self.instance,
            },
            &[],
            Purpose::SaveState,
        );
        if id == 0 {
            return None;
        }
        self.asked_state = Some(id);
        self.last_refresh = Instant::now();
        Some(id)
    }

    /// What the helper answered since the last look: the load, states, a parameter list.
    /// Never waits.
    fn take_answers(&mut self) {
        let (loaded, state, state_loaded, params) = {
            let mut d = self.inbox.data();
            (
                d.loaded.take(),
                d.state.take(),
                d.state_loaded.take(),
                d.params.take(),
            )
        };
        match loaded {
            // Given up on already (past its limit): its late answer is not taken.
            Some(Ok(_)) if self.load_failed.is_some() => {}
            Some(Ok((described, first_state))) => {
                self.loaded = true;
                self.tell_loaded = true;
                if let Some(why) = &described.refused_state {
                    log::warn!("{}: its state was refused: {why}", self.info.name);
                    if self.offline {
                        // A render never runs it with another state (review isolation-4
                        // #10, #34).
                        self.load_failed = Some(format!("it refused its state: {why}"));
                    }
                }
                self.describe(described);
                // A state given while it loaded is newer than the one it loaded with, and
                // stands; a change since the load was asked leaves it not current (review
                // isolation-6 #5, #19, #29).
                if !first_state.is_empty() && self.load >= self.state_after {
                    self.last_good = Some(first_state);
                    self.current = self.load >= self.changed_at;
                }
            }
            Some(Err(why)) => self.load_failed = Some(why),
            None => {}
        }
        if let Some((id, result)) = state {
            if self.asked_state == Some(id) {
                self.asked_state = None;
            }
            match result {
                Ok(s) if id >= self.state_after => {
                    self.last_good = Some(s);
                    if id >= self.changed_at {
                        self.current = true;
                    }
                    self.last_answer = Some((id, None));
                    if self.tell_state_dirty && self.current {
                        self.tell_state_dirty = false;
                        self.say_state_dirty = true;
                    }
                }
                Ok(_) => self.last_answer = Some((id, None)),
                Err(why) => {
                    self.last_answer = Some((id, Some(why)));
                    // Asked again a little later (review #8).
                    self.dirty_since
                        .get_or_insert(Instant::now() + RETRY_AFTER - REFRESH_AFTER);
                }
            }
        }
        if let Some((_, Err(why))) = state_loaded {
            log::warn!("{}: its state was refused: {why}", self.info.name);
            self.changed("its state was refused");
        }
        if let Some(p) = params {
            self.asked_params = false;
            self.params = p;
            // Said on now: the host reads the new list (review isolation-4 #20).
            self.say_params = true;
        }
    }

    fn describe(&mut self, d: Described) {
        self.params = d.params;
        self.latency = d.latency;
        self.has_editor = d.has_editor;
        self.inbox.data().latency = Some(d.latency);
        self.info = d.info;
    }

    /// A state was just given: answers to requests before are older — still taken, as an
    /// answer, so a save that asked one is never left waiting for it (review isolation-6
    /// #18, #23); `take_answers` keeps the given state over them.
    fn state_settled(&mut self) {
        self.state_after = self.sandbox.next_request();
        self.dirty_since = None;
        self.asked_state = None;
    }

    /// Why it is gone, in the words its end has for it (a user's stop, a neighbour's).
    fn gone_reason(&self) -> String {
        self.crashed_notice()
            .map(|(why, _)| why)
            .unwrap_or_else(|| self.crash_reason())
    }

    fn crash_reason(&self) -> String {
        self.sandbox
            .why()
            .unwrap_or_else(|| "it ended without a word".to_string())
    }

    /// A new audio half over this instance's slot.
    pub(crate) fn audio_half(&self) -> Result<Box<dyn AudioThread>, HostError> {
        let caller = new_caller(&self.slot, &self.sandbox.lobby)?;
        let sandbox = self.sandbox.clone();
        let instance = self.instance;
        Ok(Box::new(BridgeAudio::new(
            (self.slot.clone(), self.sandbox.lobby.clone()),
            caller,
            self.status.clone(),
            (
                self.offline_parts(),
                self.sample_rate,
                self.pool.deadline_floor(),
                self.latency,
            ),
            Box::new(move || {
                sandbox.post(
                    Some(instance),
                    Call::DropAudio { instance },
                    &[],
                    Purpose::Ignore,
                );
            }),
        )))
    }

    /// Why it stopped, and whose doing that was — a user's stop is no crash of anyone's:
    /// the plug-in stopped is told so, the others in its process that they went with it
    /// (round 29, review isolation-5 #28, #44, #67).
    fn crashed_notice(&self) -> Option<(String, Blame)> {
        if !self.gone() {
            return None;
        }
        // Its pipe closed first; the watchdog names how it ended a moment later.
        if self.sandbox.why().is_none() && self.sandbox.dead_for() < Duration::from_millis(200) {
            return None;
        }
        if let Some((n, name)) = self.sandbox.user_stop() {
            if n == self.instance {
                return Some(("it was stopped by you".into(), Blame::Unknown));
            }
            return Some((
                format!("its process was ended to stop {name}"),
                Blame::Neighbour,
            ));
        }
        Some((self.crash_reason(), self.sandbox.blame(self.instance)))
    }

    /// Where the state a save asked for stands — `None`: still on its way; `Some(None)`:
    /// in (or nothing to ask: never loaded, or current already); `Some(Some(why))`: not to
    /// be had in time, so what it said last stands. Asked again it answers the same (a save
    /// polls it each frame, then takes it).
    fn gathered(&mut self) -> Option<Option<String>> {
        self.take_answers();
        let Some(g) = self.gather else {
            // Nothing asked (never loaded, or gone): what it holds — said stale when it
            // changed since, and why.
            if !self.current && self.loaded {
                return Some(Some(if self.gone() {
                    self.gone_reason()
                } else {
                    "its state was not asked for".into()
                }));
            }
            return Some(None);
        };
        if let Some((id, why)) = &self.last_answer {
            if *id >= g {
                return Some(why.clone());
            }
        }
        if self.gone() {
            return Some(Some(self.gone_reason()));
        }
        // Its own limit, from when its helper started it: a request held behind a plug-in's
        // dialog waits with it (the card says so), never cut short (isolation-5 rule 2).
        let late = self.sandbox.started_at(g).is_some_and(|t| {
            t.elapsed() > self.sandbox.limit_of(g).unwrap_or(self.pool.limits().state)
        });
        if late {
            return Some(Some("it did not answer in time".into()));
        }
        None
    }

    /// Why its load failed, if it did — asked of a render's instance too, whose answers
    /// nobody takes: from the answer itself (an offline instance that refused its state
    /// failed too).
    fn load_failure(&self) -> Option<String> {
        if let Some(why) = &self.load_failed {
            return Some(why.clone());
        }
        match &self.inbox.data().loaded {
            Some(Err(why)) => Some(why.clone()),
            Some(Ok((d, _))) if self.offline => d
                .refused_state
                .as_ref()
                .map(|w| format!("it refused its state: {w}")),
            _ => None,
        }
    }

    /// Its load has run past its own limit since its helper started it, and no window of
    /// its holds the helper (a licence dialog shows the waiting card instead — live; a
    /// render's instance has no card, and its limit holds): a failed load, never a retry
    /// (round 29, review isolation-5 rule 4).
    fn load_overdue(&self) -> Option<Duration> {
        if self.loaded || self.load_failed.is_some() || self.gone() {
            return None;
        }
        let limit = self
            .sandbox
            .limit_of(self.load)
            .unwrap_or(self.pool.limits().load);
        let late = self
            .sandbox
            .started_at(self.load)
            .is_some_and(|t| t.elapsed() > limit);
        (late && (self.offline || !self.sandbox.nested_on(self.instance))).then_some(limit)
    }

    /// Waits up to [`FRAME_WAIT`] for request `id`'s answer to land (it usually comes at
    /// once); `true` when it did.
    fn brief_wait(&self, landed: impl Fn(&Inbox) -> bool) -> bool {
        let start = Instant::now();
        loop {
            if landed(&self.inbox) {
                return true;
            }
            if self.gone() || start.elapsed() >= FRAME_WAIT {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// The audio thread's side of an instance's exchange: its own bell (made with the first
/// caller, opened again by a restart's) and its helper's lobby bell.
pub(crate) fn new_caller(
    slot: &Slot,
    lobby: &super::sandbox::LobbyMem,
) -> Result<cantus_rt::ipc::Caller, HostError> {
    // SAFETY: the exchange lives in `slot`'s mapping and the lobby in `lobby`'s, both kept
    // alive by the audio half that holds the caller.
    unsafe {
        cantus_rt::ipc::Caller::create(slot.name(), slot.exchange(), &lobby.name, lobby.lobby())
    }
    .map_err(|e| HostError::Load(format!("shared memory: {e}")))
}

impl MainThread for BridgeMain {
    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn params(&self) -> Vec<ParamInfo> {
        self.params.clone()
    }

    fn param_value(&self, id: u32) -> Option<f64> {
        self.inbox.data().values.get(&id).copied()
    }

    fn param_text(&self, id: u32, value: f64) -> Option<String> {
        let key = (id, value.to_bits());
        let mut d = self.inbox.data();
        if let Some(t) = d.texts.get(&key) {
            return t.clone();
        }
        if !self.gone() && self.loaded && d.asked_texts.insert(key) {
            drop(d);
            self.post(
                Call::ParamText {
                    instance: self.instance,
                    id,
                    value,
                },
                &[],
                Purpose::Text(id, value.to_bits()),
            );
        }
        None
    }

    /// What it said last — at once, never a wait. Whether that is its state as it is now
    /// is [`MainThread::state_current`]; a save asks anew ([`MainThread::ask_state_now`]).
    fn save_state(&mut self) -> Result<Vec<u8>, HostError> {
        self.take_answers();
        self.last_good
            .clone()
            .ok_or_else(|| HostError::Crashed(self.crash_reason()))
    }

    fn ask_state_now(&mut self) {
        self.take_answers();
        if !self.loaded || self.gone() {
            self.gather = None;
            return;
        }
        // A background request made after the last change is the one waited for.
        self.gather = match self.asked_state {
            Some(id) if id >= self.changed_at => Some(id),
            _ => self.ask_state(),
        };
    }

    /// The state asked for, taken: once in (or given up on) the request is done with, so a
    /// later look without asking anew never reads an old answer as fresh (review
    /// isolation-6 #3, #25).
    fn poll_state(&mut self) -> StatePoll {
        let gathered = self.gathered();
        if gathered.is_some() {
            self.gather = None;
        }
        let last = self.last_good.clone().unwrap_or_default();
        match gathered {
            None => StatePoll::Waiting,
            Some(None) => StatePoll::Fresh(last),
            Some(Some(why)) => StatePoll::Stale(last, why),
        }
    }

    fn state_waiting(&mut self) -> bool {
        self.gathered().is_none()
    }

    fn state_current(&self) -> bool {
        self.current || !self.loaded
    }

    /// The state asked for becomes the last-good one at once: if the helper dies meanwhile,
    /// or is gone already, it is what saving and reloading use (review #9, #40, #44). Sent,
    /// never waited for.
    fn load_state(&mut self, data: &[u8]) -> Result<(), HostError> {
        // Answers already in are taken first: none is lost (review isolation-6 #18, #23).
        self.take_answers();
        self.last_good = Some(data.to_vec());
        self.state_settled();
        self.current = true;
        if self.gone() {
            return Err(HostError::Crashed(self.crash_reason()));
        }
        self.post(
            Call::LoadState {
                instance: self.instance,
            },
            data,
            Purpose::LoadState,
        );
        Ok(())
    }

    fn latency(&self) -> usize {
        self.inbox.data().latency.unwrap_or(self.latency)
    }

    fn has_editor(&self) -> bool {
        self.has_editor
    }

    /// Asks for its window; answered within a frame's share it says so, else the window
    /// opens when its helper answers (`idle` says [`Notice::EditorOpened`]) and this says
    /// it is busy — never a wait (review isolation-4 #39; isolation-5).
    fn open_editor(
        &mut self,
        _parent: raw_window_handle::RawWindowHandle,
    ) -> Result<EditorSize, HostError> {
        let id = match self.editor_pending {
            Some(id) => id,
            None => {
                let id = self.post(
                    Call::OpenEditor {
                        instance: self.instance,
                        title: self.info.name.clone(),
                    },
                    &[],
                    Purpose::Editor,
                );
                if id == 0 {
                    return Err(HostError::Crashed(self.crash_reason()));
                }
                id
            }
        };
        self.brief_wait(|inbox| inbox.data().editor.as_ref().is_some_and(|(e, _)| *e == id));
        let answer = {
            let mut d = self.inbox.data();
            match d.editor.take() {
                Some((e, r)) if e == id => Some(r),
                other => {
                    d.editor = other;
                    None
                }
            }
        };
        match answer {
            Some(r) => {
                self.editor_pending = None;
                r.map(|()| EditorSize {
                    width: 0,
                    height: 0,
                })
                .map_err(HostError::Editor)
            }
            None if self.gone() => Err(HostError::Crashed(self.crash_reason())),
            None => {
                self.editor_pending = Some(id);
                Err(HostError::Busy("its window opens when it is free".into()))
            }
        }
    }

    fn close_editor(&mut self) {
        // A window still on its way is not adopted when it comes (it is closed in order).
        self.editor_pending = None;
        self.post(
            Call::CloseEditor {
                instance: self.instance,
            },
            &[],
            Purpose::Ignore,
        );
    }

    fn idle(&mut self) -> Vec<Notice> {
        let mut out = Vec::new();
        // Its load past its limit is a failed load before its helper's life is judged
        // (review isolation-6 #0).
        self.take_answers();
        if let Some(limit) = self.load_overdue() {
            let secs = limit.as_secs().max(1);
            self.load_failed = Some(format!("it did not load within {secs} s"));
        }
        self.sandbox.check_alive();
        let pauses = self.sandbox.pauses();
        if pauses != self.pauses_seen {
            // Cantus was paused (a laptop asleep): the late job's clock starts again, as
            // the helper's own did (review isolation-4 #12, #22).
            self.pauses_seen = pauses;
            self.status.rebase();
        }
        if !self.gone() {
            // SAFETY: the exchange is in the slot's mapping, which `self.slot` keeps alive.
            let x = unsafe { &*self.slot.exchange() };
            if let Some(t) = self.status.unfinished_for(x) {
                if t > self.pool.hang_limit() {
                    self.sandbox.kill_blaming(
                        &format!(
                            "{} stopped responding while it processed audio",
                            self.info.name
                        ),
                        Some(self.instance),
                    );
                }
            }
        }
        self.take_answers();
        if self.tell_loaded {
            self.tell_loaded = false;
            if self.load_failed.is_none() {
                // Its audio half with its latency, for the host to put in the first one's
                // place.
                self.fresh_audio = self.audio_half().ok();
                out.push(Notice::Loaded);
            }
        }
        if self.gone() {
            self.status.mark_dead();
            if !self.told_crash {
                if let Some((reason, blame)) = self.crashed_notice() {
                    self.told_crash = true;
                    // A helper that never started: a failed load, nobody's crash (#40).
                    out.push(if self.loaded || self.sandbox.greeted() {
                        Notice::Crashed { reason, blame }
                    } else {
                        Notice::LoadFailed(reason)
                    });
                }
            }
            return out;
        }
        if let Some(why) = self.load_failed.clone() {
            // Not a crash: its helper lives, and its neighbours with it.
            self.status.fail();
            if !self.told_crash {
                self.told_crash = true;
                out.push(Notice::LoadFailed(why));
            }
            return out;
        }
        if let Some(id) = self.editor_pending {
            let answer = {
                let mut d = self.inbox.data();
                match d.editor.take() {
                    Some((e, r)) if e == id => Some(r),
                    other => {
                        d.editor = other;
                        None
                    }
                }
            };
            match answer {
                Some(Ok(())) => {
                    self.editor_pending = None;
                    out.push(Notice::EditorOpened);
                }
                Some(Err(e)) => {
                    self.editor_pending = None;
                    log::warn!("{}: its window did not open: {e}", self.info.name);
                }
                None => {}
            }
        }
        let (notices, moved) = {
            let mut d = self.inbox.data();
            (
                std::mem::take(&mut d.notices),
                std::mem::take(&mut d.values_moved),
            )
        };
        let now = Instant::now();
        if moved {
            self.changed("a value moved");
            if self.dirty_since.is_none() && self.last_refresh.elapsed() >= REFRESH_EVERY {
                // Values alone move with automation too: refreshed at a slower pace.
                self.dirty_since = Some(now - REFRESH_AFTER);
            }
        }
        for n in notices {
            match n {
                WireNotice::ParamsChanged => {
                    // Asked for; said on once the new list is in (`take_answers`).
                    if !self.asked_params {
                        self.asked_params = true;
                        self.post(
                            Call::Params {
                                instance: self.instance,
                            },
                            &[],
                            Purpose::Params,
                        );
                    }
                    self.changed("its parameters changed");
                    self.dirty_since.get_or_insert(now);
                    continue;
                }
                WireNotice::StateDirty => {
                    // Said on once the state it changed to is in (the host records it).
                    self.changed("it said its state changed");
                    self.tell_state_dirty = true;
                    self.dirty_since.get_or_insert(now);
                    continue;
                }
                WireNotice::LatencyChanged(l) => self.latency = l,
                _ => {}
            }
            out.push(n.notice());
        }
        if let Some(since) = self.dirty_since {
            if self.asked_state.is_none() && since.elapsed() >= REFRESH_AFTER {
                self.dirty_since = None;
                self.ask_state();
            }
        }
        if std::mem::take(&mut self.say_params) {
            out.push(Notice::ParamsChanged);
        }
        if std::mem::take(&mut self.say_state_dirty) {
            out.push(Notice::StateDirty);
        }
        out
    }

    /// A restart is never waited for past a frame's share: `Ok(None)` ("not yet"), and a
    /// later call picks its answer up — one request, never a second (review #11;
    /// isolation-4 #4, #23; isolation-5).
    fn restart(&mut self) -> Result<Option<Box<dyn AudioThread>>, HostError> {
        if self.gone() {
            return Err(HostError::Crashed(self.crash_reason()));
        }
        let id = match self.restart_pending {
            Some(id) => id,
            None => {
                let id = self.post(
                    Call::Restart {
                        instance: self.instance,
                    },
                    &[],
                    Purpose::Restart,
                );
                if id == 0 {
                    return Err(HostError::Crashed(self.crash_reason()));
                }
                self.restart_pending = Some(id);
                self.brief_wait(|inbox| {
                    inbox
                        .data()
                        .restarted
                        .as_ref()
                        .is_some_and(|(r, _)| *r == id)
                });
                id
            }
        };
        let answer = {
            let mut d = self.inbox.data();
            match d.restarted.take() {
                Some((r, a)) if r == id => Some(a),
                other => {
                    d.restarted = other;
                    None
                }
            }
        };
        match answer {
            None => Ok(None),
            Some(a) => {
                self.restart_pending = None;
                match a {
                    // Its old audio half was still on its way back: asked again next time.
                    Ok(None) => Ok(None),
                    Ok(Some(d)) => {
                        self.describe(d);
                        self.audio_half().map(Some)
                    }
                    Err(e) if e == "restart" => Err(HostError::Unsupported("restart")),
                    Err(e) => Err(HostError::Activate(e)),
                }
            }
        }
    }

    /// Values delivered while nothing processes: sent, never waited for — what the plug-in
    /// reports back comes with a later empty flush, as a flush it asked for does.
    fn flush(&mut self, params: &[ParamChange], out: &mut ParamOut) -> bool {
        if params.is_empty() {
            for e in std::mem::take(&mut self.inbox.data().flushed) {
                out.push(e.event());
            }
            return false;
        }
        self.changed("values were flushed to it");
        let wire = params.iter().map(|p| (p.offset, p.id, p.value)).collect();
        self.post(
            Call::Flush {
                instance: self.instance,
                params: wire,
            },
            &[],
            Purpose::Flush,
        );
        true
    }

    fn editor_is_floating(&self) -> bool {
        true
    }

    fn focus_editor(&mut self) -> bool {
        self.post(
            Call::FocusEditor {
                instance: self.instance,
            },
            &[],
            Purpose::Ignore,
        );
        true
    }

    fn why_changed(&self) -> Option<&'static str> {
        self.why_changed
    }

    fn checkpoint(&mut self) {
        if !self.gone() {
            self.changed("a gesture ended");
            self.dirty_since
                .get_or_insert(Instant::now() - REFRESH_AFTER);
        }
    }

    fn own_time(&self) -> Option<(u64, u64)> {
        let s = self.slot.stats();
        Some((
            s.nanos.load(std::sync::atomic::Ordering::Relaxed),
            s.frames.load(std::sync::atomic::Ordering::Relaxed),
        ))
    }

    fn isolated_in(&self) -> Option<u32> {
        Some(self.sandbox.pid())
    }

    fn helper_id(&self) -> Option<u64> {
        Some(self.sandbox.serial)
    }

    fn waiting(&self) -> bool {
        !self.gone() && self.sandbox.waiting_on(self.instance)
    }

    fn surface(&mut self) {
        if !self.gone() {
            self.sandbox.surface();
        }
    }

    fn stop(&mut self) {
        if !self.gone() {
            self.sandbox.stop_by_user(self.instance, &self.info.name);
        }
    }

    fn late_blocks(&self) -> u64 {
        self.status.late.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Loaded, and said (`idle` said [`Notice::Loaded`] as it took the answer).
    fn loaded(&self) -> bool {
        self.loaded && self.load_failure().is_none()
    }

    fn take_audio(&mut self) -> Option<Box<dyn AudioThread>> {
        self.fresh_audio.take()
    }

    /// A helper's end first (whose doing it was goes with it), else a failed load.
    fn stopped(&self) -> Option<(String, Blame)> {
        if self.gone() {
            return self.crashed_notice();
        }
        self.load_failure().map(|why| (why, Blame::Unknown))
    }
}

impl Drop for BridgeMain {
    fn drop(&mut self) {
        self.sandbox.unregister(self.instance);
    }
}
