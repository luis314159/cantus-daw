//! The helper processes, from Cantus's side (ADR 0063): one per group of isolated plug-ins
//! — every *Together* plug-in in one, a vendor's in one, an *Individually* plug-in in its
//! own — started on demand, watched, and killed when they hang.
//!
//! Each helper has three threads here, none of them real-time: a **writer** that owns its
//! standard input, a **reader** that owns its standard output and routes what arrives
//! (replies to the call that waits for them, news to the instance's [`Inbox`]), and a
//! **watchdog** that owns the process, polls whether it has ended and kills it when asked.
//! When the helper ends — a crash, a kill, its own exit — every instance in it is marked
//! gone at once: their exchanges are closed, so an audio thread waiting on one is woken
//! there and then, and every call waiting for an answer gets an error.
//!
//! On Windows every helper is put in one job object made with *kill on job close*, so no
//! helper outlives Cantus, whatever way Cantus ends; on Linux the helper asks for a
//! `SIGKILL` when its parent dies, and ends when its standard input closes.

use super::proto::{self, After, Answer, Call, Failure, Message, Request, WireEvent, WireNotice};
use super::slot::Slot;
use crate::backend::HostError;
use cantus_rt::ipc::Closer;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

/// The helper's executable name.
pub const EXE: &str = if cfg!(windows) {
    "cantus-sandbox.exe"
} else {
    "cantus-sandbox"
};

/// Points tests (and developers) at another helper.
pub const EXE_ENV: &str = "CANTUS_SANDBOX_EXE";

/// How long the helper's main thread may show no life (no heartbeat) before it is taken
/// for hung — unless a request is under way within its own time limit. The heartbeat comes
/// from a thread of its own, sent while the main thread makes progress, which it also does
/// inside a plug-in's modal loop (a file dialog, a window dragged): only a main thread
/// stuck in code that never returns to a message loop falls silent (review #0, #48).
pub const SILENCE_LIMIT: Duration = Duration::from_secs(5);

/// How long a job may stay unfinished before the helper is taken for hung (its audio
/// passes dry meanwhile, never waited for).
pub const HANG_LIMIT: Duration = Duration::from_secs(2);

/// How long an offline job (a render) may stay unfinished before its helper is taken for
/// hung: far beyond real time, for a plug-in that is merely slow.
pub const OFFLINE_HANG_LIMIT: Duration = Duration::from_secs(30);

/// A request whose helper is alive (its heartbeat comes) but that is not answered within
/// this many times its own limit is given up, the helper ended.
pub const ALIVE_STRETCH: u32 = 6;

/// The size of a helper's lobby region (the words its processing threads share).
pub const LOBBY_BYTES: usize = 4096;

/// Where in the lobby region the helper's main thread writes the instance it is calling
/// into (0 between calls): what names the plug-in that crashed it (review #49).
pub const MAIN_AT: usize = 2048;

/// The time limits of a pool's helpers (tests shorten them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    /// A main thread with no heartbeat and no request under way: hung.
    pub silence: Duration,
    pub load: Duration,
    pub state: Duration,
    /// Every other main-thread request.
    pub call: Duration,
    /// A real-time job unfinished this long: hung.
    pub hang: Duration,
    /// An offline job unfinished this long: hung.
    pub offline: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            silence: SILENCE_LIMIT,
            load: LOAD_TIMEOUT,
            state: STATE_TIMEOUT,
            call: CALL_TIMEOUT,
            hang: HANG_LIMIT,
            offline: OFFLINE_HANG_LIMIT,
        }
    }
}

/// A helper's lobby: the region its processing threads share, and the bell Cantus rings
/// when none of them is scanning (`cantus_rt::ipc`). Made here, before the helper starts;
/// every audio half of the helper's instances keeps it alive.
#[derive(Debug)]
pub struct LobbyMem {
    pub name: String,
    mem: cantus_rt::shm::SharedMemory,
    /// Kept: the bell lives while one side holds it.
    _bell: cantus_rt::ipc::LobbyBell,
}

impl LobbyMem {
    fn create() -> std::io::Result<LobbyMem> {
        let name = cantus_rt::shm::unique_name("cantus-lobby");
        let mem = cantus_rt::shm::SharedMemory::create(&name, LOBBY_BYTES)?;
        // SAFETY: the lobby is at the start of the zeroed region, which this value keeps
        // mapped as long as the bell.
        let bell = unsafe {
            cantus_rt::ipc::LobbyBell::create(&name, mem.as_ptr() as *const cantus_rt::ipc::Lobby)?
        };
        Ok(LobbyMem {
            name,
            mem,
            _bell: bell,
        })
    }

    pub fn lobby(&self) -> *const cantus_rt::ipc::Lobby {
        self.mem.as_ptr() as *const cantus_rt::ipc::Lobby
    }

    /// The instance the helper's main thread is calling into now, 0 for none ([`MAIN_AT`]).
    pub fn main_at(&self) -> u32 {
        // SAFETY: an aligned u32 inside the region, which `self` keeps mapped; the helper
        // writes it as an atomic.
        let word = unsafe { &*(self.mem.as_ptr().add(MAIN_AT) as *const AtomicU32) };
        word.load(Ordering::Acquire)
    }
}

/// Time limits of the main-thread calls.
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(30);
pub const STATE_TIMEOUT: Duration = Duration::from_secs(10);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(5);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What an instance's helper has said between calls, and the mirror of its values.
#[derive(Debug, Default)]
pub struct Inbox {
    inner: Mutex<InboxData>,
}

#[derive(Debug, Default)]
pub(crate) struct InboxData {
    pub values: HashMap<u32, f64>,
    /// Values changed since the state was last remembered.
    pub values_moved: bool,
    pub notices: Vec<WireNotice>,
    pub flushed: Vec<WireEvent>,
    pub latency: Option<usize>,
    pub texts: HashMap<(u32, u64), Option<String>>,
    pub asked_texts: HashSet<(u32, u64)>,
    /// A state asked for in the background, once it is answered: the request's id, and
    /// the state or why there is none.
    pub state: Option<(u64, Result<Vec<u8>, String>)>,
}

impl Inbox {
    pub(crate) fn data(&self) -> MutexGuard<'_, InboxData> {
        lock(&self.inner)
    }

    fn absorb(&self, after: After) {
        let mut d = self.data();
        d.notices.extend(after.notices);
        d.latency = Some(after.latency);
        Self::take_values(&mut d, after.values);
    }

    fn take_values(d: &mut InboxData, values: Vec<(u32, f64)>) {
        for (id, v) in values {
            let old = d.values.insert(id, v);
            if old.map(f64::to_bits) != Some(v.to_bits()) {
                d.values_moved = true;
            }
        }
    }
}

/// An answer as it arrives: what the helper said, and its blob.
type Answered = (Result<Answer, Failure>, Vec<u8>);

/// What a background request was for.
#[derive(Clone, Copy, Debug)]
enum Background {
    Text(u32, u32, u64),
    State(u32),
    Ignore,
}

struct Registered {
    inbox: Arc<Inbox>,
    /// Kept so the closer's mapping outlives it; its exchange says whether a job runs.
    slot: Arc<Slot>,
    closer: Closer,
}

/// A request under way: when it went, its time limit, the instance it names.
#[derive(Clone, Copy, Debug)]
struct Flight {
    at: Instant,
    limit: Duration,
    instance: Option<u32>,
}

pub use crate::backend::Blame;

/// One helper process.
pub struct Sandbox {
    pub key: String,
    pub pid: u32,
    /// Its processing threads' lobby.
    pub lobby: Arc<LobbyMem>,
    to_child: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    waiters: Mutex<HashMap<u64, mpsc::Sender<Answered>>>,
    background: Mutex<HashMap<u64, Background>>,
    instances: Mutex<HashMap<u32, Registered>>,
    next_id: AtomicU64,
    next_instance: AtomicU32,
    dead: AtomicBool,
    /// Milliseconds since `started` when it was marked gone.
    dead_ms: AtomicU64,
    why: Mutex<Option<String>>,
    kill: Arc<AtomicBool>,
    started: Instant,
    /// Milliseconds since `started` of the last heartbeat.
    alive_ms: AtomicU64,
    /// Requests under way (waited for or not), by id.
    inflight: Mutex<HashMap<u64, Flight>>,
    /// The instances its end is laid on, once it ended; empty when nothing said which.
    blamed: Mutex<Option<HashSet<u32>>>,
    pool: Weak<Sandboxes>,
}

impl std::fmt::Debug for Sandbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sandbox")
            .field("key", &self.key)
            .field("pid", &self.pid)
            .field("dead", &self.is_dead())
            .finish()
    }
}

impl Sandbox {
    fn spawn(pool: &Arc<Sandboxes>, key: &str) -> Result<Arc<Sandbox>, HostError> {
        let fma3 = match cantus_rt::fpenv::crt_fma3_enabled() {
            Some(false) => "off",
            _ => "on",
        };
        let lobby = Arc::new(
            LobbyMem::create().map_err(|e| HostError::Load(format!("shared memory: {e}")))?,
        );
        let mut cmd = Command::new(&pool.exe);
        cmd.arg("--serve")
            .arg(proto::PROTOCOL.to_string())
            .arg("--crt-fma3")
            .arg(fma3)
            .arg("--group")
            .arg(key)
            .arg("--lobby")
            .arg(&lobby.name)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // A console program started by a windowed one: no console window for it.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| {
            HostError::Load(format!(
                "cannot start the plug-in sandbox ({}): {e}",
                pool.exe.display()
            ))
        })?;
        pool.adopt(&child);
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            return Err(HostError::Load("the plug-in sandbox has no pipes".into()));
        };
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        let sandbox = Arc::new(Sandbox {
            key: key.to_string(),
            pid: child.id(),
            lobby,
            to_child: Mutex::new(Some(tx)),
            waiters: Mutex::new(HashMap::new()),
            background: Mutex::new(HashMap::new()),
            instances: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            next_instance: AtomicU32::new(1),
            dead: AtomicBool::new(false),
            dead_ms: AtomicU64::new(0),
            why: Mutex::new(None),
            kill: Arc::new(AtomicBool::new(false)),
            started: Instant::now(),
            alive_ms: AtomicU64::new(0),
            inflight: Mutex::new(HashMap::new()),
            blamed: Mutex::new(None),
            pool: Arc::downgrade(pool),
        });
        // The writer.
        let mut stdin = stdin;
        std::thread::Builder::new()
            .name("cantus-sandbox-writer".into())
            .spawn(move || {
                use std::io::Write;
                while let Ok(frame) = rx.recv() {
                    if stdin.write_all(&frame).and_then(|_| stdin.flush()).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| HostError::Load(e.to_string()))?;
        // The reader.
        let me = Arc::downgrade(&sandbox);
        let mut stdout = std::io::BufReader::new(stdout);
        std::thread::Builder::new()
            .name("cantus-sandbox-reader".into())
            .spawn(move || loop {
                let frame = proto::read_frame::<Message>(&mut stdout);
                let Some(s) = me.upgrade() else { return };
                match frame {
                    Ok(Some((msg, blob))) => s.route(msg, blob),
                    Ok(None) => {
                        s.mark_dead(None);
                        return;
                    }
                    Err(e) => {
                        s.mark_dead(Some(&format!(
                            "the plug-in sandbox broke its protocol: {e}"
                        )));
                        s.request_kill();
                        return;
                    }
                }
            })
            .map_err(|e| HostError::Load(e.to_string()))?;
        // The watchdog.
        let me = Arc::downgrade(&sandbox);
        let kill = sandbox.kill.clone();
        std::thread::Builder::new()
            .name("cantus-sandbox-watchdog".into())
            .spawn(move || watch(child, kill, me))
            .map_err(|e| HostError::Load(e.to_string()))?;
        // The handshake.
        match sandbox.call(
            None,
            Call::Hello {
                protocol: proto::PROTOCOL,
            },
            &[],
            pool.limits().load,
        ) {
            Ok((Answer::Hello { protocol, .. }, _)) if protocol == proto::PROTOCOL => {
                // The helper has mapped its lobby: the name is no longer needed (review #43).
                sandbox.lobby.mem.unlink_name();
                Ok(sandbox)
            }
            Ok((Answer::Hello { protocol, .. }, _)) => {
                sandbox.request_kill();
                Err(HostError::Load(format!(
                    "the plug-in sandbox speaks protocol {protocol}, Cantus {}",
                    proto::PROTOCOL
                )))
            }
            Ok(_) => {
                sandbox.request_kill();
                Err(HostError::Load("the plug-in sandbox did not greet".into()))
            }
            Err(e) => Err(e),
        }
    }

    pub fn is_dead(&self) -> bool {
        self.dead.load(Ordering::Acquire)
    }

    /// How long it has been gone (zero while it lives).
    pub fn dead_for(&self) -> Duration {
        let at = self.dead_ms.load(Ordering::Acquire);
        if at == 0 {
            return Duration::ZERO;
        }
        let now = self.started.elapsed().as_millis() as u64;
        Duration::from_millis(now.saturating_sub(at))
    }

    /// Why it ended, once it has.
    pub fn why(&self) -> Option<String> {
        lock(&self.why).clone()
    }

    /// Marks it gone: every instance's exchange closed (waking an audio thread that waits
    /// on one), every waiting call answered with an error. `why` is kept if none was given
    /// before; `None` (its pipe closed) leaves it to the watchdog, which knows how the
    /// process ended.
    pub(crate) fn mark_dead(&self, why: Option<&str>) {
        self.mark_dead_blaming(why, None);
    }

    /// [`Self::mark_dead`], laying the end on `culprit` when the caller knows it (a job of
    /// its that never finished, a call of its that was never answered); otherwise on the
    /// instance the helper's main thread was calling into, or whose job was running.
    fn mark_dead_blaming(&self, why: Option<&str>, culprit: Option<u32>) {
        if let Some(why) = why {
            let mut w = lock(&self.why);
            if w.is_none() {
                *w = Some(why.to_string());
            }
        }
        {
            let mut b = lock(&self.blamed);
            if b.is_none() {
                *b = Some(self.find_culprits(culprit));
            }
        }
        if self.dead.swap(true, Ordering::AcqRel) {
            return;
        }
        self.dead_ms.store(
            (self.started.elapsed().as_millis() as u64).max(1),
            Ordering::Release,
        );
        for r in lock(&self.instances).values() {
            r.closer.close();
        }
        lock(&self.waiters).clear();
        *lock(&self.to_child) = None;
    }

    fn find_culprits(&self, culprit: Option<u32>) -> HashSet<u32> {
        let mut out = HashSet::new();
        out.extend(culprit);
        let at = self.lobby.main_at();
        let instances = lock(&self.instances);
        if at != 0 && instances.contains_key(&at) {
            out.insert(at);
        }
        for (n, r) in instances.iter() {
            // SAFETY: the exchange is in the slot's mapping, which `r` keeps alive.
            if unsafe { &*r.slot.exchange() }.running() {
                out.insert(*n);
            }
        }
        // One plug-in in it: its own end, whatever it was.
        if out.is_empty() && instances.len() == 1 {
            out.extend(instances.keys().copied());
        }
        out
    }

    /// Who this helper's end is laid on, from `instance`'s side.
    pub fn blame(&self, instance: u32) -> Blame {
        match lock(&self.blamed).as_ref() {
            Some(set) if set.contains(&instance) => Blame::This,
            Some(set) if !set.is_empty() => Blame::Neighbour,
            _ => Blame::Unknown,
        }
    }

    fn request_kill(&self) {
        self.kill.store(true, Ordering::Release);
    }

    /// Ends the helper (it hung): marks it gone now, kills it in the watchdog.
    pub fn kill(&self, why: &str) {
        self.kill_blaming(why, None);
    }

    /// [`Self::kill`], laying it on `culprit` (the instance whose job or call hung).
    pub fn kill_blaming(&self, why: &str, culprit: Option<u32>) {
        self.mark_dead_blaming(Some(why), culprit);
        self.request_kill();
    }

    /// How long since its main thread last showed life (its heartbeat).
    pub fn silent_for(&self) -> Duration {
        let now = self.started.elapsed().as_millis() as u64;
        let last = self.alive_ms.load(Ordering::Acquire);
        Duration::from_millis(now.saturating_sub(last))
    }

    pub(crate) fn limits(&self) -> Limits {
        self.pool.upgrade().map(|p| p.limits()).unwrap_or_default()
    }

    /// Whether the helper is hung, and why: its main thread shows no life past the silence
    /// limit while no request is under way within its own limit — or a request has gone
    /// unanswered far past its limit even though the helper lives (review #0, #41, #48).
    fn hung(&self) -> Option<String> {
        let limits = self.limits();
        let silent = self.silent_for() > limits.silence;
        let flights = lock(&self.inflight);
        let mut oldest_over: Option<Flight> = None;
        for f in flights.values() {
            let took = f.at.elapsed();
            if took > f.limit * ALIVE_STRETCH {
                return Some(format!(
                    "the plug-in sandbox did not answer in {} s",
                    took.as_secs()
                ));
            }
            if took > f.limit && oldest_over.is_none_or(|o| f.at < o.at) {
                oldest_over = Some(*f);
            }
        }
        if !silent {
            return None;
        }
        if flights.is_empty() {
            return Some("the plug-in sandbox stopped answering".into());
        }
        oldest_over.map(|f| {
            format!(
                "the plug-in sandbox did not answer in {} s",
                f.limit.as_secs().max(1)
            )
        })
    }

    /// The instance the oldest overdue request named (what a hang is laid on).
    fn hung_on(&self) -> Option<u32> {
        lock(&self.inflight)
            .values()
            .filter(|f| f.at.elapsed() > f.limit)
            .min_by_key(|f| f.at)
            .and_then(|f| f.instance)
    }

    fn fly(&self, id: u64, limit: Duration, instance: Option<u32>) {
        lock(&self.inflight).insert(
            id,
            Flight {
                at: Instant::now(),
                limit,
                instance,
            },
        );
    }

    fn route(&self, msg: Message, blob: Vec<u8>) {
        match msg {
            Message::Alive => {
                let ms = self.started.elapsed().as_millis() as u64;
                self.alive_ms.store(ms, Ordering::Release);
            }
            Message::News {
                instance,
                notices,
                values,
                flushed,
            } => {
                if let Some(inbox) = self.inbox(instance) {
                    let mut d = inbox.data();
                    d.notices.extend(notices);
                    d.flushed.extend(flushed);
                    Inbox::take_values(&mut d, values);
                }
            }
            Message::Reply { id, result, after } => {
                lock(&self.inflight).remove(&id);
                if let Some((instance, after)) = after.map(|b| *b) {
                    if let Some(inbox) = self.inbox(instance) {
                        inbox.absorb(after);
                    }
                }
                let waiter = lock(&self.waiters).remove(&id);
                match waiter {
                    Some(tx) => {
                        let _ = tx.send((result, blob));
                    }
                    None => self.background_answer(id, result, blob),
                }
            }
        }
    }

    fn background_answer(&self, id: u64, result: Result<Answer, Failure>, blob: Vec<u8>) {
        let Some(kind) = lock(&self.background).remove(&id) else {
            return;
        };
        match (kind, result) {
            (Background::Text(instance, pid, bits), Ok(Answer::Text(text))) => {
                if let Some(inbox) = self.inbox(instance) {
                    let mut d = inbox.data();
                    if d.texts.len() > 4096 {
                        d.texts.clear();
                    }
                    d.asked_texts.remove(&(pid, bits));
                    d.texts.insert((pid, bits), text);
                }
            }
            (Background::State(instance), result) => {
                // A failure is delivered too, so the instance asks again later (review #8).
                let state = match result {
                    Ok(Answer::State) => Ok(blob),
                    Ok(_) => Err("no state in the answer".to_string()),
                    Err(f) => Err(failure(f).to_string()),
                };
                if let Some(inbox) = self.inbox(instance) {
                    inbox.data().state = Some((id, state));
                }
            }
            _ => {}
        }
    }

    pub(crate) fn inbox(&self, instance: u32) -> Option<Arc<Inbox>> {
        lock(&self.instances)
            .get(&instance)
            .map(|r| r.inbox.clone())
    }

    fn frame(id: u64, call: &Call, blob: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64 + blob.len());
        // Writing into a Vec cannot fail.
        let _ = proto::write_frame(
            &mut buf,
            &Request {
                id,
                call: call.clone(),
            },
            blob,
        );
        buf
    }

    fn post(&self, frame: Vec<u8>) -> bool {
        match lock(&self.to_child).as_ref() {
            Some(tx) => tx.send(frame).is_ok(),
            None => false,
        }
    }

    fn gone(&self) -> HostError {
        HostError::Crashed(
            self.why()
                .unwrap_or_else(|| "the plug-in sandbox ended".into()),
        )
    }

    /// A call that waits for its answer, at most `timeout`. A helper that does not answer
    /// in time is taken for hung and killed. `instance` names the instance whose news the
    /// answer carries.
    pub(crate) fn call(
        &self,
        instance: Option<u32>,
        call: Call,
        blob: &[u8],
        timeout: Duration,
    ) -> Result<(Answer, Vec<u8>), HostError> {
        if self.is_dead() {
            return Err(self.gone());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        lock(&self.waiters).insert(id, tx);
        self.fly(id, timeout, instance);
        if !self.post(Self::frame(id, &call, blob)) {
            lock(&self.inflight).remove(&id);
            return Err(self.gone());
        }
        // Waited for in slices: a helper that lives (its heartbeat comes, as it does inside
        // a plug-in's modal loop) is given longer than the limit; one silent past it is hung.
        let slice = Duration::from_millis(50);
        loop {
            match rx.recv_timeout(slice) {
                Ok((Ok(answer), blob)) => return Ok((answer, blob)),
                Ok((Err(f), _)) => return Err(failure(f)),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    lock(&self.inflight).remove(&id);
                    return Err(self.gone());
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(why) = self.hung() {
                        lock(&self.waiters).remove(&id);
                        let culprit = self.hung_on().or(instance);
                        lock(&self.inflight).remove(&id);
                        self.kill_blaming(&why, culprit);
                        return Err(self.gone());
                    }
                }
            }
        }
    }

    /// A call whose answer is not waited for.
    pub(crate) fn tell(&self, call: Call, blob: &[u8]) {
        if self.is_dead() {
            return;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.background).insert(id, Background::Ignore);
        // Under way like a call (review #41): an unload that takes long is not silence.
        let instance = match &call {
            Call::Unload { instance }
            | Call::DropAudio { instance }
            | Call::CloseEditor { instance }
            | Call::FocusEditor { instance } => Some(*instance),
            _ => None,
        };
        if !matches!(call, Call::Quit) {
            self.fly(id, self.limits().call, instance);
        }
        self.post(Self::frame(id, &call, blob));
    }

    /// A text asked for in the background; the answer lands in the instance's inbox.
    pub(crate) fn ask_text(&self, instance: u32, id: u32, value: f64) {
        if self.is_dead() {
            return;
        }
        let rid = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.background).insert(rid, Background::Text(instance, id, value.to_bits()));
        self.fly(rid, self.limits().call, Some(instance));
        self.post(Self::frame(
            rid,
            &Call::ParamText {
                instance,
                id,
                value,
            },
            &[],
        ));
    }

    /// A state asked for in the background; it lands in the instance's inbox, tagged with
    /// the request's id (returned; 0 when the helper is gone).
    pub(crate) fn ask_state(&self, instance: u32) -> u64 {
        if self.is_dead() {
            return 0;
        }
        let rid = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.background).insert(rid, Background::State(instance));
        self.fly(rid, self.limits().state, Some(instance));
        self.post(Self::frame(rid, &Call::SaveState { instance }, &[]));
        rid
    }

    /// The id the next request will have: answers to requests before it are older.
    pub(crate) fn next_request(&self) -> u64 {
        self.next_id.load(Ordering::Relaxed)
    }

    pub(crate) fn new_instance(&self) -> u32 {
        self.next_instance.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn register(
        &self,
        instance: u32,
        inbox: Arc<Inbox>,
        slot: Arc<Slot>,
        closer: Closer,
    ) {
        lock(&self.instances).insert(
            instance,
            Registered {
                inbox,
                slot,
                closer,
            },
        );
        if self.is_dead() {
            if let Some(r) = lock(&self.instances).get(&instance) {
                r.closer.close();
            }
        }
    }

    /// An instance left: its exchange closed first — a block the engine still sends it
    /// passes dry at once instead of waiting out its deadline for a job nobody will take
    /// (review #5) — then the helper told, and ended when it was the last.
    pub(crate) fn unregister(&self, instance: u32) {
        let (gone, empty) = {
            let mut m = lock(&self.instances);
            let gone = m.remove(&instance);
            (gone, m.is_empty())
        };
        if let Some(r) = &gone {
            r.closer.close();
        }
        self.tell(Call::Unload { instance }, &[]);
        drop(gone);
        if empty {
            self.tell(Call::Quit, &[]);
            if let Some(pool) = self.pool.upgrade() {
                pool.release(self);
            }
        }
    }

    /// The heartbeat: a helper whose main thread shows no life past the silence limit,
    /// with no request under way within its own limit, is taken for hung ([`Self::hung`]).
    pub(crate) fn check_alive(&self) {
        if self.is_dead() {
            return;
        }
        if let Some(why) = self.hung() {
            let culprit = self.hung_on();
            self.kill_blaming(&why, culprit);
        }
    }

    pub fn instances(&self) -> usize {
        lock(&self.instances).len()
    }
}

fn failure(f: Failure) -> HostError {
    match f {
        Failure::Load(s) => HostError::Load(s),
        Failure::NotFound(s) => HostError::NotFound(s),
        Failure::Instantiate(s) => HostError::Instantiate(s),
        Failure::Activate(s) => HostError::Activate(s),
        Failure::Unsupported(s) => HostError::Unsupported(unsupported_name(&s)),
        Failure::State(s) => HostError::State(s),
        Failure::Editor(s) => HostError::Editor(s),
        Failure::NoInstance => HostError::Crashed("the plug-in is no longer loaded".into()),
    }
}

/// `HostError::Unsupported` names what is unsupported with a static string: the ones the
/// backends use, kept.
fn unsupported_name(s: &str) -> &'static str {
    const NAMES: [&str; 6] = [
        "restart",
        "format",
        "plug-in windows on this system",
        "editor",
        "state",
        "no plug-in in this slot",
    ];
    NAMES.into_iter().find(|n| *n == s).unwrap_or("operation")
}

/// How a helper's end reads: the Windows exception or the Unix signal named when it is a
/// common one.
fn crash_words(status: std::process::ExitStatus) -> String {
    #[cfg(windows)]
    {
        let code = status.code().unwrap_or(0) as u32;
        let what = match code {
            0xC000_0005 => "an access violation",
            0xC000_0409 => "a stack buffer overrun or an abort",
            0xC000_00FD => "a stack overflow",
            0xC000_001D => "an illegal instruction",
            0xC000_0094 => "an integer division by zero",
            0x8000_0003 => "a breakpoint",
            3 => "an abort",
            1 => "a forced end",
            _ => "",
        };
        if what.is_empty() {
            format!("it crashed (exit code 0x{code:08X})")
        } else {
            format!("it crashed: {what} (0x{code:08X})")
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        match status.signal() {
            Some(libc_sig) => format!("it crashed (signal {libc_sig})"),
            None => format!("it crashed ({status})"),
        }
    }
}

/// The watchdog: polls the process, kills it when asked, and marks the helper gone when
/// it ends.
fn watch(mut child: Child, kill: Arc<AtomicBool>, me: Weak<Sandbox>) {
    loop {
        if kill.load(Ordering::Acquire) {
            let _ = child.kill();
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if let Some(s) = me.upgrade() {
                    let why = if status.success() {
                        "it ended".to_string()
                    } else {
                        crash_words(status)
                    };
                    s.mark_dead(Some(&why));
                }
                return;
            }
            Ok(None) => {}
            Err(_) => return,
        }
        if me.strong_count() == 0 {
            // Nothing in Cantus uses it any more: end it.
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Every helper process of this Cantus, by group.
pub struct Sandboxes {
    pub exe: PathBuf,
    groups: Mutex<HashMap<String, Arc<Sandbox>>>,
    limits: Mutex<Limits>,
    /// The shortest real-time deadline, in microseconds.
    deadline_floor_us: AtomicU64,
    #[cfg(windows)]
    job: Option<job::Job>,
}

impl std::fmt::Debug for Sandboxes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sandboxes")
            .field("exe", &self.exe)
            .field("groups", &lock(&self.groups).len())
            .finish()
    }
}

impl Sandboxes {
    pub fn new(exe: PathBuf) -> Arc<Sandboxes> {
        Arc::new(Sandboxes {
            exe,
            groups: Mutex::new(HashMap::new()),
            limits: Mutex::new(Limits::default()),
            deadline_floor_us: AtomicU64::new(super::audio::DEADLINE_FLOOR.as_micros() as u64),
            #[cfg(windows)]
            job: job::Job::new(),
        })
    }

    /// The helper Cantus uses: `CANTUS_SANDBOX_EXE` when set, else `cantus-sandbox` next to
    /// the running program — where Cargo and the installer put it.
    pub fn default_exe() -> PathBuf {
        if let Some(p) = std::env::var_os(EXE_ENV) {
            return PathBuf::from(p);
        }
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(EXE)))
            .unwrap_or_else(|| PathBuf::from(EXE))
    }

    /// Whether the helper is there to be started.
    pub fn available(&self) -> bool {
        self.exe.is_file()
    }

    pub fn limits(&self) -> Limits {
        *lock(&self.limits)
    }

    pub fn set_limits(&self, limits: Limits) {
        *lock(&self.limits) = limits;
    }

    pub fn hang_limit(&self) -> Duration {
        self.limits().hang
    }

    pub fn set_hang_limit(&self, limit: Duration) {
        lock(&self.limits).hang = limit;
    }

    /// The shortest deadline a real-time block waits for its plug-in (instances loaded
    /// from now on).
    pub fn deadline_floor(&self) -> Duration {
        Duration::from_micros(self.deadline_floor_us.load(Ordering::Relaxed))
    }

    pub fn set_deadline_floor(&self, floor: Duration) {
        self.deadline_floor_us
            .store(floor.as_micros() as u64, Ordering::Relaxed);
    }

    /// The live helper of `key`, started if there is none.
    pub fn group(self: &Arc<Self>, key: &str) -> Result<Arc<Sandbox>, HostError> {
        let mut groups = lock(&self.groups);
        if let Some(s) = groups.get(key) {
            if !s.is_dead() {
                return Ok(s.clone());
            }
        }
        let s = Sandbox::spawn(self, key)?;
        groups.insert(key.to_string(), s.clone());
        Ok(s)
    }

    fn release(&self, sandbox: &Sandbox) {
        let mut groups = lock(&self.groups);
        if groups
            .get(&sandbox.key)
            .is_some_and(|s| std::ptr::eq(Arc::as_ptr(s), sandbox))
        {
            groups.remove(&sandbox.key);
        }
    }

    /// The live helpers: group and process id.
    pub fn processes(&self) -> Vec<(String, u32)> {
        let mut v: Vec<(String, u32)> = lock(&self.groups)
            .values()
            .filter(|s| !s.is_dead())
            .map(|s| (s.key.clone(), s.pid))
            .collect();
        v.sort();
        v
    }

    #[cfg(windows)]
    fn adopt(&self, child: &Child) {
        if let Some(j) = &self.job {
            j.assign(child);
        }
    }

    #[cfg(not(windows))]
    fn adopt(&self, _child: &Child) {}
}

#[cfg(windows)]
mod job {
    //! One job object for every helper, made with *kill on job close*: when Cantus ends,
    //! however it ends, the system closes the handle and ends the helpers with it.
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    pub struct Job(HANDLE);

    // SAFETY: a job handle is a kernel object usable from any thread.
    unsafe impl Send for Job {}
    // SAFETY: as above; assigning a process is thread-safe in the kernel.
    unsafe impl Sync for Job {}

    impl Job {
        pub fn new() -> Option<Job> {
            // SAFETY: an anonymous job object with default security.
            let h = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if h.is_null() {
                return None;
            }
            // SAFETY: a zeroed limit structure is valid; we set one flag in it and hand
            // its size, as the call asks.
            let ok = unsafe {
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    h,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const std::ffi::c_void,
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            if ok == 0 {
                // SAFETY: our own handle, closed once.
                unsafe { CloseHandle(h) };
                return None;
            }
            Some(Job(h))
        }

        pub fn assign(&self, child: &Child) {
            // SAFETY: the child's process handle is live while `child` is; the job is ours.
            let ok = unsafe { AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE) };
            if ok == 0 {
                log::warn!("a plug-in sandbox could not join Cantus's job object");
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: our own handle, closed once; closing it ends the helpers.
            unsafe { CloseHandle(self.0) };
        }
    }
}
