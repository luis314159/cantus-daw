//! An isolated plug-in's main half, in Cantus (ADR 0063): every call the host makes on the
//! main thread, answered without ever freezing the interface in steady state.
//!
//! - **Values** (`param_value`, called every frame by the generic editor) come from a
//!   mirror the helper keeps current — after every call, and whenever they change — never
//!   from a round trip.
//! - **Texts** (`param_text`) are cached; one not cached yet is asked for in the background
//!   and shown from the next frame (the editor shows the number meanwhile).
//! - **State**: a **last-good state** is kept here — taken after load, after every state
//!   loaded, at the end of a gesture ([`MainThread::checkpoint`]), when the plug-in says
//!   its state changed, and every ten seconds while its values move — in the background.
//!   When the helper is gone, `save_state` answers with it, so saving the project, parking
//!   the slot or reloading the plug-in loses nothing the user did.
//! - **Calls** that must answer (load, a state, a flush with values, an editor, a restart)
//!   wait with a time limit; a helper that misses one is taken for hung and killed.
//! - **Crashes**: when the helper ends or a job stays unfinished past the hang limit,
//!   `idle` says [`Notice::Crashed`] once, with the reason.
//! - **The editor** is a top-level window the helper makes and owns (an owned window across
//!   processes would join the two input queues, and a hung helper would freeze Cantus's
//!   window): `editor_is_floating` is true, and bringing it forward and closing it are
//!   proxied.

use super::audio::{BridgeAudio, Offline, Status};
use super::proto::{Answer, Call, Described, WireNotice};
use super::sandbox::{Inbox, Pending, Sandbox, Sandboxes, Wait, Waited};
use super::slot::Slot;
use crate::backend::{
    AudioThread, Blame, EditorSize, HostError, MainThread, Notice, ParamChange, ParamInfo,
    ParamOut, PluginInfo,
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
/// How long a restart just asked for is waited for before the frame goes on (it is picked
/// up by a later one; review isolation-4 #23).
const RESTART_WAIT: Duration = Duration::from_millis(100);

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
    has_editor: bool,
    last_good: Option<Vec<u8>>,
    /// Something changed since the last-good state was taken, and when it first did.
    dirty_since: Option<Instant>,
    asked_state: bool,
    /// A restart asked for and not answered yet.
    restart_pending: Option<Pending>,
    /// Its window asked for and not answered yet: opened once it is (review isolation-4
    /// #39).
    editor_pending: Option<Pending>,
    /// Background states answered to a request before this id are older than the last
    /// state loaded or saved, and dropped (review #7, #50).
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        sandbox: Arc<Sandbox>,
        pool: Arc<Sandboxes>,
        instance: u32,
        slot: Arc<Slot>,
        status: Arc<Status>,
        inbox: Arc<Inbox>,
        (offline, cancel): (bool, Arc<AtomicBool>),
        sample_rate: f64,
        described: Described,
        first_state: Vec<u8>,
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
            info: described.info,
            params: described.params,
            latency: described.latency,
            has_editor: described.has_editor,
            // The state it starts with is the first good one.
            last_good: (!first_state.is_empty()).then_some(first_state),
            dirty_since: None,
            asked_state: false,
            restart_pending: None,
            editor_pending: None,
            state_after: 0,
            last_refresh: Instant::now(),
            told_crash: false,
            pauses_seen: 0,
        }
    }

    /// What an offline audio half needs to end a hung or cancelled render.
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
        self.sandbox.pid
    }

    fn gone(&self) -> bool {
        self.sandbox.is_dead() || self.status.is_dead()
    }

    fn ask_state(&mut self) {
        if !self.gone() {
            self.sandbox.ask_state(self.instance);
            self.asked_state = true;
            self.last_refresh = Instant::now();
        }
    }

    /// A background state, once answered: kept unless a state was loaded or saved since it
    /// was asked for; a failure asks again a little later (review #8, #50).
    fn take_answered_state(&mut self) {
        let Some((id, state)) = self.inbox.data().state.take() else {
            return;
        };
        self.asked_state = false;
        if id < self.state_after {
            return;
        }
        match state {
            Ok(s) => self.last_good = Some(s),
            Err(_) => {
                self.dirty_since
                    .get_or_insert(Instant::now() + RETRY_AFTER - REFRESH_AFTER);
            }
        }
    }

    /// A state was just loaded or saved: background answers asked before are older.
    fn state_settled(&mut self) {
        self.state_after = self.sandbox.next_request();
        self.dirty_since = None;
        self.asked_state = false;
        self.inbox.data().state = None;
    }

    /// Whether Cantus already knows the helper is in trouble: a job unfinished, or no
    /// heartbeat for a while.
    /// Whether this instance is in trouble — its own job unfinished, its own request
    /// overdue, or its helper hung — not merely a neighbour busy ahead of it (review #2,
    /// #24).
    fn in_trouble(&self) -> bool {
        // SAFETY: the exchange is in the slot's mapping, which `self.slot` keeps alive.
        let x = unsafe { &*self.slot.exchange() };
        self.status.unfinished_for(x).is_some() || self.sandbox.troubled(self.instance)
    }

    fn call(
        &self,
        call: Call,
        blob: &[u8],
        timeout: Duration,
    ) -> Result<(Answer, Vec<u8>), HostError> {
        self.sandbox.call(Some(self.instance), call, blob, timeout)
    }

    /// An offline instance's call: waited for as long as its helper lives, until the
    /// render is cancelled or the call has run the offline limit, which ends the helper —
    /// never given up with the render going on regardless (review isolation-4 #10).
    fn call_offline(
        &self,
        call: Call,
        blob: &[u8],
        limit: Duration,
    ) -> Result<(Answer, Vec<u8>), HostError> {
        let cap = self.pool.limits().offline;
        self.sandbox.call_while_alive(
            Some(self.instance),
            call,
            blob,
            (limit, &self.cancel, limit.max(cap)),
        )
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
            Box::new(move || sandbox.tell(Call::DropAudio { instance }, &[])),
        )))
    }

    fn crashed_notice(&self) -> Option<(String, Blame)> {
        if !self.gone() {
            return None;
        }
        // Its pipe closed first; the watchdog names how it ended a moment later.
        if self.sandbox.why().is_none() && self.sandbox.dead_for() < Duration::from_millis(200) {
            return None;
        }
        Some((self.crash_reason(), self.sandbox.blame(self.instance)))
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
        if !self.gone() && d.asked_texts.insert(key) {
            drop(d);
            self.sandbox.ask_text(self.instance, id, value);
        }
        None
    }

    fn save_state(&mut self) -> Result<Vec<u8>, HostError> {
        match self.fresh_state() {
            // Alive but not answering in time: what it said last (review #4, #44).
            Err(HostError::Busy(_)) if self.last_good.is_some() => {
                Ok(self.last_good.clone().unwrap_or_default())
            }
            r => r,
        }
    }

    fn fresh_state(&mut self) -> Result<Vec<u8>, HostError> {
        self.take_answered_state();
        if !self.gone() && self.last_good.is_some() && self.in_trouble() {
            // Known to be in trouble: what it said last, at once, and a fresh state asked
            // for in the background (review #51).
            if !self.asked_state {
                self.ask_state();
            }
            return Err(HostError::Busy("it is not answering".into()));
        }
        if !self.gone() {
            let limit = self.pool.limits().state;
            match self.call(
                Call::SaveState {
                    instance: self.instance,
                },
                &[],
                limit,
            ) {
                Ok((Answer::State, blob)) => {
                    self.last_good = Some(blob.clone());
                    self.state_settled();
                    return Ok(blob);
                }
                Ok(_) => {}
                // Alive but not answering in time (its dialog is open, a slow call is ahead):
                // busy — `save_state` answers with what it said last, or a fresher state
                // answered meanwhile, never a frozen save (review #4, #44; isolation-4 #1).
                // Its own answer, when it comes, becomes the last-good state.
                Err(HostError::Busy(why)) if self.last_good.is_some() => {
                    self.take_answered_state();
                    return Err(HostError::Busy(why));
                }
                Err(e) if !self.gone() => return Err(e),
                Err(_) => {}
            }
        }
        // Gone: what it last said is what the project keeps.
        self.last_good
            .clone()
            .ok_or_else(|| HostError::Crashed(self.crash_reason()))
    }

    /// The state asked for becomes the last-good one before the call: if the helper dies
    /// meanwhile, or is gone already, it is what saving and reloading use — the project's
    /// state, not the one before it (review #9, #40, #44).
    fn load_state(&mut self, data: &[u8]) -> Result<(), HostError> {
        self.last_good = Some(data.to_vec());
        self.state_settled();
        let limit = self.pool.limits().state;
        let call = Call::LoadState {
            instance: self.instance,
        };
        if self.offline {
            // A render never starts before its state is in (review isolation-4 #10).
            return self.call_offline(call, data, limit).map(|_| ());
        }
        self.call(call, data, limit).map(|_| ())
    }

    fn latency(&self) -> usize {
        self.inbox.data().latency.unwrap_or(self.latency)
    }

    fn has_editor(&self) -> bool {
        self.has_editor
    }

    fn open_editor(
        &mut self,
        _parent: raw_window_handle::RawWindowHandle,
    ) -> Result<EditorSize, HostError> {
        let limit = self.pool.limits().call;
        // One request: a window asked for while its helper was busy is not asked again,
        // only waited for; when it opens, the host is told (review isolation-4 #39).
        let (pending, wait) = match self.editor_pending.take() {
            Some(p) => (p, Wait::Poll),
            None => {
                let title = self.info.name.clone();
                let p = self.sandbox.post_call(
                    Some(self.instance),
                    Call::OpenEditor {
                        instance: self.instance,
                        title,
                    },
                    &[],
                    limit,
                )?;
                (p, Wait::Limit)
            }
        };
        let answered = match self.sandbox.wait_call(&pending, wait) {
            Waited::Answered(r) => (*r)?,
            Waited::Pending => {
                let e = self.sandbox.give_up_waiting(&pending, limit);
                self.editor_pending = Some(pending);
                return Err(e);
            }
        };
        match answered {
            (Answer::Editor { width, height }, _) => Ok(EditorSize { width, height }),
            _ => Err(HostError::Editor(
                "the plug-in sandbox did not open it".into(),
            )),
        }
    }

    fn close_editor(&mut self) {
        self.sandbox.tell(
            Call::CloseEditor {
                instance: self.instance,
            },
            &[],
        );
    }

    fn idle(&mut self) -> Vec<Notice> {
        let mut out = Vec::new();
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
        if self.gone() {
            self.status.mark_dead();
            if let Some((reason, blame)) = self.crashed_notice() {
                if !self.told_crash {
                    self.told_crash = true;
                    out.push(Notice::Crashed { reason, blame });
                }
            }
            return out;
        }
        self.take_answered_state();
        let (notices, moved, params) = {
            let mut d = self.inbox.data();
            (
                std::mem::take(&mut d.notices),
                std::mem::take(&mut d.values_moved),
                d.params.take(),
            )
        };
        if let Some(p) = params {
            // A parameter list asked for and given up on, answered now: taken, and the
            // host told again (review isolation-4 #20).
            self.params = p;
            out.push(Notice::ParamsChanged);
        }
        if let Some(pending) = self.editor_pending.take() {
            match self.sandbox.wait_call(&pending, Wait::Poll) {
                Waited::Answered(r) => match *r {
                    Ok((Answer::Editor { .. }, _)) => out.push(Notice::EditorOpened),
                    Ok(_) => {}
                    Err(e) => log::warn!("{}: its window did not open: {e}", self.info.name),
                },
                Waited::Pending => self.editor_pending = Some(pending),
            }
        }
        let now = Instant::now();
        if moved && self.dirty_since.is_none() {
            // Values alone move with automation too: refreshed at a slower pace.
            if self.last_refresh.elapsed() >= REFRESH_EVERY {
                self.dirty_since = Some(now - REFRESH_AFTER);
            }
        }
        for n in notices {
            match n {
                WireNotice::ParamsChanged => {
                    let limit = self.pool.limits().call;
                    // Given up on (its helper busy), its answer still comes: taken then
                    // (`Background::Params`).
                    if let Ok((Answer::Params(p), _)) = self.call(
                        Call::Params {
                            instance: self.instance,
                        },
                        &[],
                        limit,
                    ) {
                        self.params = p;
                    }
                    self.dirty_since.get_or_insert(now);
                }
                WireNotice::StateDirty => {
                    self.dirty_since.get_or_insert(now);
                }
                WireNotice::LatencyChanged(l) => self.latency = l,
                _ => {}
            }
            out.push(n.notice());
        }
        if let Some(since) = self.dirty_since {
            if !self.asked_state && since.elapsed() >= REFRESH_AFTER {
                self.dirty_since = None;
                self.ask_state();
            }
        }
        out
    }

    /// A restart is not waited for past a moment: `Ok(None)` ("not yet"), and a later call
    /// picks its answer up — one request, never a second, and never a frozen interface
    /// (review #11; isolation-4 #4, #23).
    fn restart(&mut self) -> Result<Option<Box<dyn AudioThread>>, HostError> {
        let limit = self.pool.limits().call;
        // Asked for: waited for a moment; picked up later: only looked at — never a frame
        // that waits a call's limit again (review isolation-4 #4, #23, #26, #51, #60).
        let (pending, wait) = match self.restart_pending.take() {
            Some(p) => (p, Wait::Poll),
            None => (
                self.sandbox.post_call(
                    Some(self.instance),
                    Call::Restart {
                        instance: self.instance,
                    },
                    &[],
                    limit,
                )?,
                Wait::For(RESTART_WAIT),
            ),
        };
        let answered = match self.sandbox.wait_call(&pending, wait) {
            Waited::Answered(r) => (*r)?,
            Waited::Pending => {
                self.restart_pending = Some(pending);
                return Ok(None);
            }
        };
        match answered {
            (Answer::Restarted(None), _) => Ok(None),
            (Answer::Restarted(Some(d)), _) => {
                self.params = d.params;
                self.latency = d.latency;
                self.inbox.data().latency = Some(d.latency);
                self.audio_half().map(Some)
            }
            _ => Err(HostError::Unsupported("restart")),
        }
    }

    fn flush(&mut self, params: &[ParamChange], out: &mut ParamOut) -> bool {
        if params.is_empty() {
            // What the plug-in reported outside processing, which the helper flushed when
            // it asked: delivered here.
            for e in std::mem::take(&mut self.inbox.data().flushed) {
                out.push(e.event());
            }
            return false;
        }
        let wire = params.iter().map(|p| (p.offset, p.id, p.value)).collect();
        let limit = self.pool.limits().call;
        let call = Call::Flush {
            instance: self.instance,
            params: wire,
        };
        let answer = if self.offline {
            // A render's values are in before its first block (review isolation-4 #10).
            self.call_offline(call, &[], limit)
        } else {
            self.call(call, &[], limit)
        };
        match answer {
            Ok((
                Answer::Flushed {
                    processed,
                    out: events,
                },
                _,
            )) => {
                for e in events {
                    out.push(e.event());
                }
                processed
            }
            _ => false,
        }
    }

    fn editor_is_floating(&self) -> bool {
        true
    }

    fn focus_editor(&mut self) -> bool {
        self.sandbox.tell(
            Call::FocusEditor {
                instance: self.instance,
            },
            &[],
        );
        true
    }

    fn checkpoint(&mut self) {
        if !self.gone() {
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
        Some(self.sandbox.pid)
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
            self.sandbox.kill_blaming(
                &format!("{} was stopped while it waited on a window of its own", self.info.name),
                Some(self.instance),
            );
        }
    }

    fn late_blocks(&self) -> u64 {
        self.status.late.load(std::sync::atomic::Ordering::Relaxed)
    }

    fn stopped(&self) -> Option<(String, Blame)> {
        self.crashed_notice()
    }
}

impl Drop for BridgeMain {
    fn drop(&mut self) {
        self.sandbox.unregister(self.instance);
    }
}
