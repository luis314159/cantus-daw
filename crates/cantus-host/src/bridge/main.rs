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

use super::audio::{BridgeAudio, Status};
use super::proto::{Answer, Call, Described, WireNotice};
use super::sandbox::{Inbox, Sandbox, Sandboxes, CALL_TIMEOUT, STATE_TIMEOUT};
use super::slot::Slot;
use crate::backend::{
    AudioThread, EditorSize, HostError, MainThread, Notice, ParamChange, ParamInfo, ParamOut,
    PluginInfo,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// How soon after a change the last-good state is refreshed, at most.
const REFRESH_AFTER: Duration = Duration::from_millis(500);
/// How often the state is refreshed while only values move (automation, a knob the plug-in
/// did not report as a gesture).
const REFRESH_EVERY: Duration = Duration::from_secs(10);

/// The main half.
pub struct BridgeMain {
    pub(crate) sandbox: Arc<Sandbox>,
    pub(crate) pool: Arc<Sandboxes>,
    pub(crate) instance: u32,
    pub(crate) slot: Arc<Slot>,
    pub(crate) status: Arc<Status>,
    pub(crate) inbox: Arc<Inbox>,
    pub(crate) offline: bool,
    pub(crate) sample_rate: f64,
    info: PluginInfo,
    params: Vec<ParamInfo>,
    latency: usize,
    has_editor: bool,
    last_good: Option<Vec<u8>>,
    /// Something changed since the last-good state was taken, and when it first did.
    dirty_since: Option<Instant>,
    asked_state: bool,
    last_refresh: Instant,
    told_crash: bool,
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
        offline: bool,
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
            sample_rate,
            info: described.info,
            params: described.params,
            latency: described.latency,
            has_editor: described.has_editor,
            // The state it starts with is the first good one.
            last_good: (!first_state.is_empty()).then_some(first_state),
            dirty_since: None,
            asked_state: false,
            last_refresh: Instant::now(),
            told_crash: false,
        }
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

    fn take_answered_state(&mut self) {
        if let Some(state) = self.inbox.data().state.take() {
            self.last_good = Some(state);
            self.asked_state = false;
        }
    }

    fn call(&self, call: Call, blob: &[u8], timeout: Duration) -> Result<(Answer, Vec<u8>), HostError> {
        self.sandbox.call(Some(self.instance), call, blob, timeout)
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
                self.offline,
                self.sample_rate,
                self.pool.deadline_floor(),
                self.latency,
            ),
            Box::new(move || sandbox.tell(Call::DropAudio { instance }, &[])),
        )))
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
    unsafe { cantus_rt::ipc::Caller::create(slot.name(), slot.exchange(), &lobby.name, lobby.lobby()) }
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
        self.take_answered_state();
        if !self.gone() {
            match self.call(Call::SaveState { instance: self.instance }, &[], STATE_TIMEOUT) {
                Ok((Answer::State, blob)) => {
                    self.last_good = Some(blob.clone());
                    self.dirty_since = None;
                    return Ok(blob);
                }
                Ok(_) => {}
                Err(e) if !self.gone() => return Err(e),
                Err(_) => {}
            }
        }
        // Gone: what it last said is what the project keeps.
        self.last_good
            .clone()
            .ok_or_else(|| HostError::Crashed(self.crash_reason()))
    }

    fn load_state(&mut self, data: &[u8]) -> Result<(), HostError> {
        match self.call(Call::LoadState { instance: self.instance }, data, STATE_TIMEOUT) {
            Ok(_) => {
                self.last_good = Some(data.to_vec());
                self.dirty_since = None;
                Ok(())
            }
            Err(e) => Err(e),
        }
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
        let title = self.info.name.clone();
        match self.call(
            Call::OpenEditor {
                instance: self.instance,
                title,
            },
            &[],
            CALL_TIMEOUT,
        )? {
            (Answer::Editor { width, height }, _) => Ok(EditorSize { width, height }),
            _ => Err(HostError::Editor("the plug-in sandbox did not open it".into())),
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
        if !self.gone() {
            if let Some(t) = self.status.unfinished_for() {
                if t > self.pool.hang_limit() {
                    self.sandbox.kill(&format!(
                        "{} stopped responding while it processed audio",
                        self.info.name
                    ));
                }
            }
        }
        if self.gone() {
            self.status.mark_dead();
            // Its pipe closed first; the watchdog names how it ended a moment later.
            if self.sandbox.why().is_none() && self.sandbox.dead_for() < Duration::from_millis(200)
            {
                return out;
            }
            if !self.told_crash {
                self.told_crash = true;
                out.push(Notice::Crashed {
                    reason: self.crash_reason(),
                });
            }
            return out;
        }
        self.take_answered_state();
        let (notices, moved) = {
            let mut d = self.inbox.data();
            (
                std::mem::take(&mut d.notices),
                std::mem::take(&mut d.values_moved),
            )
        };
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
                    if let Ok((Answer::Params(p), _)) = self.call(
                        Call::Params {
                            instance: self.instance,
                        },
                        &[],
                        CALL_TIMEOUT,
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

    fn restart(&mut self) -> Result<Option<Box<dyn AudioThread>>, HostError> {
        match self.call(
            Call::Restart {
                instance: self.instance,
            },
            &[],
            CALL_TIMEOUT,
        )? {
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
        match self.call(
            Call::Flush {
                instance: self.instance,
                params: wire,
            },
            &[],
            CALL_TIMEOUT,
        ) {
            Ok((Answer::Flushed { processed, out: events }, _)) => {
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
            self.dirty_since.get_or_insert(Instant::now() - REFRESH_AFTER);
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

    fn late_blocks(&self) -> u64 {
        self.status.late.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Drop for BridgeMain {
    fn drop(&mut self) {
        self.sandbox.unregister(self.instance);
    }
}
