//! The helper processes, from Cantus's side (ADR 0063): one per group of isolated plug-ins
//! — every *Together* plug-in in one, a vendor's in one, an *Individually* plug-in in its
//! own — started on demand, watched, and killed when they hang.
//!
//! Each helper has three threads here, none of them real-time: a **writer** that owns its
//! standard input, a **reader** that owns its standard output and routes what arrives —
//! every answer and every piece of news into its instance's [`Inbox`] — and a **watchdog**
//! that owns the process, polls whether it has ended and kills it when asked. When the
//! helper ends — a crash, a kill, its own exit — every instance in it is marked gone at
//! once: their exchanges are closed, so an audio thread waiting on one is woken there and
//! then.
//!
//! **Nothing here waits for the helper** (round 29, review isolation-5): a request is sent
//! and its answer lands in the inbox, which the instance's main half takes each frame —
//! loads, states, values, windows, restarts, parameter lists. A request that takes long
//! shows its plug-in's waiting card (*Show it*, *Stop it*); one that runs past its own
//! limit with the helper silent gets the helper ended.
//!
//! On Windows every helper is put in one job object made with *kill on job close*, so no
//! helper outlives Cantus, whatever way Cantus ends; on Linux the helper asks for a
//! `SIGKILL` when its parent dies, and ends when its standard input closes.

use super::audio::Status;
use super::proto::{
    self, After, Answer, Call, Described, Failure, Message, Request, WireEvent, WireNotice,
};
use super::slot::Slot;
use crate::backend::{HostError, ParamInfo, PluginInfo};
use cantus_rt::ipc::Closer;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
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

/// The size of a helper's lobby region (the words its processing threads share).
pub const LOBBY_BYTES: usize = 4096;

/// Where in the lobby region the helper's main thread writes the instance it is calling
/// into (0 between calls): what names the plug-in that crashed it (review #49).
pub const MAIN_AT: usize = 2048;

/// Where the helper's crash handler writes the instance the faulting thread was calling
/// into, as the process dies (0: no handler ran, as for an abort on Windows).
pub const CRASH_AT: usize = 2052;

/// Where the helper's processing threads count the jobs they finished (review isolation-4
/// #62).
pub const JOBS_DONE: usize = 2056;

/// Where Cantus marks a helper stalled: one more than [`JOBS_DONE`] read when a job of it
/// was not even taken by its deadline (0: not stalled). Any job finishing ends it.
pub const STALLED: usize = 2060;

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
        self.word(MAIN_AT)
    }

    /// The instance the helper crashed in, if its crash handler said ([`CRASH_AT`]).
    pub fn crash_at(&self) -> u32 {
        self.word(CRASH_AT)
    }

    fn word(&self, at: usize) -> u32 {
        self.atom(at).load(Ordering::Acquire)
    }

    fn atom(&self, at: usize) -> &AtomicU32 {
        // SAFETY: an aligned u32 inside the region (`at` < `LOBBY_BYTES`), which `self`
        // keeps mapped; both processes use it as an atomic.
        unsafe { &*(self.mem.as_ptr().add(at) as *const AtomicU32) }
    }

    /// Whether the helper is stalled: a job of it was not taken by its deadline, and none
    /// has finished since — its other instances pass dry at once instead of each waiting
    /// out a deadline in the same callback (review isolation-4 #62). The processing threads
    /// clear the mark when they finish a job. Real-time safe.
    pub fn stalled(&self) -> bool {
        let s = self.atom(STALLED).load(Ordering::Acquire);
        s != 0 && self.atom(JOBS_DONE).load(Ordering::Acquire).wrapping_add(1) == s
    }

    /// The count of finished jobs, read before a late job is looked at.
    pub fn jobs_done(&self) -> u32 {
        self.atom(JOBS_DONE).load(Ordering::Acquire)
    }

    /// A job was not even taken by its deadline: the helper is stalled until any job of it
    /// finishes. `done` is [`Self::jobs_done`] read before the job was seen untaken, so a
    /// job that finished in between moves the count past the mark (review isolation-5 #8).
    /// Real-time safe.
    pub fn mark_stalled(&self, done: u32) {
        self.atom(STALLED)
            .store(done.wrapping_add(1), Ordering::Release);
    }
}

/// Time limits of the main-thread requests, from when the helper starts each one: past it,
/// with the helper silent, the helper is taken for hung. A load and a state are given long
/// (a sampler reading its samples without pumping is slow, not hung); nothing waits on
/// them meanwhile, and a request served past [`WAIT_CARD_AFTER`] shows its plug-in's
/// waiting card.
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(30);
pub const STATE_TIMEOUT: Duration = Duration::from_secs(30);
pub const CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// How long Cantus, quitting, gives its helpers to end their plug-ins by themselves (their
/// destructors write preferences, release licences) before their job object ends them
/// (round 29, review isolation-5 #24, #54).
pub const QUIT_GRACE: Duration = Duration::from_secs(3);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// What an instance's helper has said, and the mirror of its values. Every answer lands
/// here (the reader thread puts it here; the main half takes it each frame): nothing on
/// Cantus's interface thread ever waits for one (round 29, review isolation-5).
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
    /// The load's answer: what the plug-in is and its first state, or why it failed.
    pub loaded: Option<Result<(Described, Vec<u8>), String>>,
    /// A state asked for, once it is answered: the request's id, and the state or why there
    /// is none.
    pub state: Option<(u64, Result<Vec<u8>, String>)>,
    /// A state given to it, once it is answered: the request's id, and whether it took it.
    pub state_loaded: Option<(u64, Result<(), String>)>,
    /// A parameter list asked for, once it is answered.
    pub params: Option<Vec<ParamInfo>>,
    /// A window asked for, once it is answered: the request's id, and whether it opened.
    pub editor: Option<(u64, Result<(), String>)>,
    /// A restart asked for, once it is answered: the request's id, and what the plug-in is
    /// now (`None`: its old audio half was still on its way back — ask again).
    pub restarted: Option<(u64, Result<Option<Described>, String>)>,
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

/// What a request is for: where its answer lands, and how long it may take.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Purpose {
    Hello,
    Load,
    SaveState,
    LoadState,
    Flush,
    Params,
    Text(u32, u64),
    Editor,
    Restart,
    /// An instance's unload: its record goes once it is answered.
    Unload(u32),
    /// Nothing to keep (a close, a focus, a dropped audio half, a quit, a surface).
    Ignore,
}

struct Registered {
    inbox: Arc<Inbox>,
    /// Kept so the closer's mapping outlives it; its exchange says whether a job runs.
    slot: Arc<Slot>,
    closer: Closer,
    /// What its audio halves read: ready once it is loaded, failed when it never will be.
    status: Arc<Status>,
}

/// A request under way: when it went, when the helper started it (`Message::Started`),
/// how long it may run once started, the instance it names, what it is for.
#[derive(Clone, Copy, Debug)]
struct Flight {
    at: Instant,
    started: Option<Instant>,
    limit: Duration,
    instance: Option<u32>,
    purpose: Purpose,
}

pub use crate::backend::Blame;

/// What the helper's crash handler writes when the faulting thread was calling into no
/// instance (a plug-in's own thread, the main loop between calls): nobody to blame — but
/// for the only plug-in of its helper.
pub const OUTSIDE: u32 = u32::MAX;

/// How a helper ended, for whom its end is laid on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum End {
    /// It ended by itself (a crash, an exit): what it was doing says whose.
    Observed,
    /// Cantus ended it (a hang, a silence, a broken protocol, a stop), laying it on this
    /// instance or on nobody — never on a bystander whose job happened to run (review
    /// isolation-4 #3, #30, #44, #55).
    Decided(Option<u32>),
}

/// Who a helper's end is laid on (review #38, #45, #19, #20; isolation-4 #2, #3, #6, #47):
///
/// 1. the instance the crash handler named — loaded, or leaving (its destructor: instance
///    numbers are never reused in a helper) — even over a hang Cantus decided while the
///    crash was being reported;
/// 2. the culprit Cantus named when it ended the helper itself;
/// 3. the only instance of the helper, whatever thread it crashed on;
/// 4. for an end Cantus decided, or the handler's *outside*: nobody;
/// 5. no handler ran (a fail-fast abort on Windows): the one candidate among the instance
///    the main thread was calling into (loaded or leaving) and those whose job was running.
///
/// More than one candidate is nobody's (`Blame::Unknown`): never an innocent neighbour
/// remembered as a crasher.
pub(crate) fn culprits(
    end: End,
    crash_at: u32,
    main_at: u32,
    running: &[u32],
    instances: &[u32],
) -> HashSet<u32> {
    let named = |n: u32| n != 0 && n != OUTSIDE;
    if named(crash_at) {
        return HashSet::from([crash_at]);
    }
    if let End::Decided(Some(c)) = end {
        return HashSet::from([c]);
    }
    if instances.len() == 1 && (!named(main_at) || instances.contains(&main_at)) {
        return instances.iter().copied().collect();
    }
    if end != End::Observed || crash_at == OUTSIDE {
        return HashSet::new();
    }
    let mut candidates: HashSet<u32> = running.iter().copied().collect();
    if named(main_at) {
        candidates.insert(main_at);
    }
    if candidates.len() == 1 {
        return candidates;
    }
    HashSet::new()
}

/// Whether the gap since the last judgement (milliseconds) is longer than the silence
/// limit: Cantus itself was not running (review #49).
fn paused(last_ms: u64, now_ms: u64, silence: Duration) -> bool {
    last_ms != 0 && now_ms.saturating_sub(last_ms) > silence.as_millis() as u64
}

/// How long a request is served (or its helper's main thread stays in a plug-in's own
/// loop) before the plug-in's waiting card shows (round 29, review isolation-5).
pub const WAIT_CARD_AFTER: Duration = Duration::from_secs(2);

/// How long the helper's main thread may stay in a plug-in's own loop before a request
/// queued behind it is taken as one that will not be served soon (a save then keeps what
/// that plug-in said last, and says so).
pub const NESTED_GRACE: Duration = Duration::from_secs(1);

/// A plug-in whose crash ended its helper while it was leaving it — its destructor, often
/// after Cantus let go of it — for the host to count (review isolation-4 #45, #46).
#[derive(Clone, Debug)]
pub struct DepartedCrash {
    /// The helper's [`Sandbox::serial`].
    pub helper: u64,
    pub info: PluginInfo,
}

/// What outlives a helper's last user: what its end is read from, and whom to tell.
struct Ending {
    lobby: Arc<LobbyMem>,
    /// Every instance's plug-in while it is loaded or leaving (until its unload is
    /// answered).
    infos: Mutex<HashMap<u32, PluginInfo>>,
    /// Quit was sent: the helper ends by itself, and is given a grace to.
    quit: AtomicBool,
    /// Departed culprits already told.
    told: Mutex<HashSet<u32>>,
    serial: u64,
    pool: Weak<Sandboxes>,
}

impl Ending {
    /// The culprits among the instances that left: told to the pool, once each.
    fn tell_departed(&self, culprits: &HashSet<u32>, live: &[u32]) {
        let Some(pool) = self.pool.upgrade() else {
            return;
        };
        let infos = lock(&self.infos);
        let mut told = lock(&self.told);
        for c in culprits {
            if live.contains(c) || !told.insert(*c) {
                continue;
            }
            if let Some(info) = infos.get(c) {
                lock(&pool.departed).push(DepartedCrash {
                    helper: self.serial,
                    info: info.clone(),
                });
            }
        }
    }

    /// The helper crashed after its last user let go: laid on what it was doing then.
    fn orphan_crashed(&self) {
        let laid = culprits(
            End::Observed,
            self.lobby.crash_at(),
            self.lobby.main_at(),
            &[],
            &[],
        );
        self.tell_departed(&laid, &[]);
    }
}

/// The next helper's [`Sandbox::serial`].
static SERIAL: AtomicU64 = AtomicU64::new(1);

/// One helper process.
pub struct Sandbox {
    pub key: String,
    pub pid: u32,
    /// A number of its own, never reused as a process id may be: what a death is counted
    /// once by (review isolation-4 #35, #53, #59).
    pub serial: u64,
    /// Its processing threads' lobby.
    pub lobby: Arc<LobbyMem>,
    to_child: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
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
    /// Whether any frame came from it yet: until then it is judged by its handshake's own
    /// limit, not by the silence limit (review isolation-4 #50).
    heard: AtomicBool,
    /// Requests under way, by id.
    inflight: Mutex<HashMap<u64, Flight>>,
    /// Its main thread is in a loop nested in this instance's code (0: whose, unknown),
    /// since then: it serves nothing meanwhile.
    nested: Mutex<Option<(u32, Instant)>>,
    /// Milliseconds since `started` of the last judgement of its life (a pause guard).
    judged_ms: AtomicU64,
    /// Pauses of Cantus seen so far: each instance's job clock starts again after one.
    pauses: AtomicU64,
    /// The instances its end is laid on, once it ended; empty when nothing said which.
    blamed: Mutex<Option<HashSet<u32>>>,
    /// The user stopped it from this instance's waiting card (and its plug-in's name).
    user_stop: Mutex<Option<(u32, String)>>,
    ending: Arc<Ending>,
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
    /// Starts a helper and returns at once: its handshake is the first request it serves,
    /// answered on the reader thread — a helper slow to start never holds the interface
    /// (round 29, review isolation-5); one that never answers it in its load limit is
    /// ended, and the plug-ins asked of it fail to load.
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
            .arg(&lobby.name);
        if let Some(n) = pool.threads() {
            cmd.arg("--threads").arg(n.to_string());
        }
        cmd.stdin(Stdio::piped())
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
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let ending = Arc::new(Ending {
            lobby: lobby.clone(),
            infos: Mutex::new(HashMap::new()),
            quit: AtomicBool::new(false),
            told: Mutex::new(HashSet::new()),
            serial,
            pool: Arc::downgrade(pool),
        });
        let sandbox = Arc::new(Sandbox {
            key: key.to_string(),
            pid: child.id(),
            serial,
            lobby,
            to_child: Mutex::new(Some(tx)),
            instances: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            next_instance: AtomicU32::new(1),
            dead: AtomicBool::new(false),
            dead_ms: AtomicU64::new(0),
            why: Mutex::new(None),
            kill: Arc::new(AtomicBool::new(false)),
            started: Instant::now(),
            alive_ms: AtomicU64::new(0),
            heard: AtomicBool::new(false),
            inflight: Mutex::new(HashMap::new()),
            nested: Mutex::new(None),
            judged_ms: AtomicU64::new(0),
            pauses: AtomicU64::new(0),
            blamed: Mutex::new(None),
            user_stop: Mutex::new(None),
            ending: ending.clone(),
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
                        // Cantus ends it: laid on nobody (review isolation-4 #44).
                        s.kill_blaming(
                            &format!("the plug-in sandbox broke its protocol: {e}"),
                            None,
                        );
                        return;
                    }
                }
            })
            .map_err(|e| HostError::Load(e.to_string()))?;
        // The watchdog.
        let me = Arc::downgrade(&sandbox);
        let kill = sandbox.kill.clone();
        let live = pool.live.clone();
        live.fetch_add(1, Ordering::AcqRel);
        std::thread::Builder::new()
            .name("cantus-sandbox-watchdog".into())
            .spawn(move || {
                watch(child, kill, me, ending);
                live.fetch_sub(1, Ordering::AcqRel);
            })
            .map_err(|e| HostError::Load(e.to_string()))?;
        // The handshake: the first request it serves.
        sandbox.post(
            None,
            Call::Hello {
                protocol: proto::PROTOCOL,
            },
            &[],
            Purpose::Hello,
        );
        Ok(sandbox)
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

    /// Marks it gone, as it ended by itself: every instance's exchange closed (waking an
    /// audio thread that waits on one). `why` is kept if none was given before; `None` (its
    /// pipe closed) leaves it to the watchdog, which knows how the process ended.
    pub(crate) fn mark_dead(&self, why: Option<&str>) {
        self.mark_dead_blaming(why, End::Observed);
    }

    /// [`Self::mark_dead`], with how it ended: by itself, or ended by Cantus (laid on the
    /// culprit Cantus names, or on nobody).
    fn mark_dead_blaming(&self, why: Option<&str>, end: End) {
        if let Some(why) = why {
            let mut w = lock(&self.why);
            if w.is_none() {
                *w = Some(why.to_string());
            }
        }
        {
            let mut b = lock(&self.blamed);
            if b.is_none() {
                let (laid, live) = self.find_culprits(end);
                // A plug-in that crashed while it left is counted all the same.
                self.ending.tell_departed(&laid, &live);
                *b = Some(laid);
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
            r.status.mark_dead();
        }
        *lock(&self.to_child) = None;
    }

    /// The culprits of an end, and the instances still loaded.
    fn find_culprits(&self, end: End) -> (HashSet<u32>, Vec<u32>) {
        let instances = lock(&self.instances);
        let running: Vec<u32> = instances
            .iter()
            // SAFETY: the exchange is in the slot's mapping, which `r` keeps alive.
            .filter(|(_, r)| unsafe { &*r.slot.exchange() }.running())
            .map(|(n, _)| *n)
            .collect();
        let all: Vec<u32> = instances.keys().copied().collect();
        let laid = culprits(
            end,
            self.lobby.crash_at(),
            self.lobby.main_at(),
            &running,
            &all,
        );
        (laid, all)
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

    /// Ends the helper (it hung), laid on nobody: marks it gone now, kills it in the
    /// watchdog.
    pub fn kill(&self, why: &str) {
        self.kill_blaming(why, None);
    }

    /// [`Self::kill`], laying it on `culprit` (the instance whose job or call hung).
    pub fn kill_blaming(&self, why: &str, culprit: Option<u32>) {
        self.mark_dead_blaming(Some(why), End::Decided(culprit));
        self.request_kill();
    }

    /// The user stopped `instance` (its plug-in `name`) from its waiting card: its helper
    /// ends — with every plug-in in it. Told so to each, and counted as no crash (round 29,
    /// review isolation-5 #28, #44, #67).
    pub(crate) fn stop_by_user(&self, instance: u32, name: &str) {
        *lock(&self.user_stop) = Some((instance, name.to_string()));
        self.kill_blaming("stopped by the user", Some(instance));
    }

    /// Whom the user stopped, if the user ended it: the instance and its plug-in's name.
    pub(crate) fn user_stop(&self) -> Option<(u32, String)> {
        lock(&self.user_stop).clone()
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

    /// Whether the helper is hung, and why. Before it said anything, it is judged by its
    /// handshake's own limit (review isolation-4 #50). A helper that shows life is never
    /// hung — a plug-in's dialog keeps it alive (review #36, #44). A silent one is hung
    /// unless the request it is serving now is still within its own limit — a request
    /// queued behind it is not judged until the helper starts it (#3, #37).
    fn hung(&self) -> Option<String> {
        if !self.heard.load(Ordering::Acquire) {
            let late = lock(&self.inflight)
                .values()
                .any(|f| f.at.elapsed() > f.limit);
            return late.then(|| "the plug-in sandbox did not start in time".to_string());
        }
        if self.silent_for() <= self.limits().silence {
            return None;
        }
        match self.serving() {
            Some((_, f)) => {
                let took = f.started.map_or(Duration::ZERO, |s| s.elapsed());
                (took > f.limit).then(|| {
                    format!(
                        "the plug-in sandbox did not answer in {} s",
                        f.limit.as_secs().max(1)
                    )
                })
            }
            None => Some("the plug-in sandbox stopped answering".into()),
        }
    }

    /// The request the helper is serving now (the last it started, not yet answered), and
    /// its id.
    fn serving(&self) -> Option<(u64, Flight)> {
        lock(&self.inflight)
            .iter()
            .filter(|(_, f)| f.started.is_some())
            .max_by_key(|(_, f)| f.started)
            .map(|(id, f)| (*id, *f))
    }

    /// What a hang of the main thread is laid on: the instance it is calling into, else the
    /// one whose request it is serving — never the caller of a request it had not started
    /// (review isolation-4 #3, #30, #44, #55).
    fn hung_on(&self) -> Option<u32> {
        let at = self.lobby.main_at();
        if at != 0 && at != OUTSIDE {
            return Some(at);
        }
        self.serving().and_then(|(_, f)| f.instance)
    }

    /// Whether `instance` keeps its helper waiting — a request of its served for a while,
    /// or the helper's main thread in a loop of its code (a dialog, perhaps behind Cantus):
    /// its waiting card shows (round 29, review isolation-5).
    pub(crate) fn waiting_on(&self, instance: u32) -> bool {
        if self.is_dead() {
            return false;
        }
        if let Some((n, since)) = *lock(&self.nested) {
            if n == instance && since.elapsed() > WAIT_CARD_AFTER {
                return true;
            }
        }
        self.serving().is_some_and(|(_, f)| {
            f.instance == Some(instance)
                && f.started.is_some_and(|s| s.elapsed() > WAIT_CARD_AFTER)
        })
    }

    /// Whether the helper's main thread is in a loop of `instance`'s code (a dialog).
    pub(crate) fn nested_on(&self, instance: u32) -> bool {
        lock(&self.nested).is_some_and(|(n, _)| n == instance)
    }

    /// Whether request `id` will not be served soon though its helper lives: queued behind
    /// a plug-in's own loop (a dialog) for longer than [`NESTED_GRACE`], or behind a
    /// request served past its own limit. A save then keeps what that plug-in said last,
    /// and says so (round 29, review isolation-5).
    pub(crate) fn held_up(&self, id: u64) -> bool {
        let Some(f) = lock(&self.inflight).get(&id).copied() else {
            return false;
        };
        if f.started.is_some() {
            return false;
        }
        if lock(&self.nested).is_some_and(|(_, since)| since.elapsed() > NESTED_GRACE) {
            return true;
        }
        self.serving().is_some_and(|(sid, s)| {
            sid != id && s.started.is_some_and(|t| t.elapsed() > s.limit)
        })
    }

    /// When request `id` was started by the helper (`None`: not yet, or answered).
    pub(crate) fn started_at(&self, id: u64) -> Option<Instant> {
        lock(&self.inflight).get(&id).and_then(|f| f.started)
    }

    /// Request `id`'s own limit.
    pub(crate) fn limit_of(&self, id: u64) -> Option<Duration> {
        lock(&self.inflight).get(&id).map(|f| f.limit)
    }

    /// Cantus (or the whole system) was paused — a debugger, a laptop asleep — for longer
    /// than the silence limit since the last judgement: the clocks start again from now,
    /// instead of every helper being taken for hung at once (review #49). `true` when so.
    fn rebased_after_a_pause(&self) -> bool {
        let now = (self.started.elapsed().as_millis() as u64).max(1);
        let last = self.judged_ms.swap(now, Ordering::AcqRel);
        if !paused(last, now, self.limits().silence) {
            return false;
        }
        self.alive_ms.store(now, Ordering::Release);
        let t = Instant::now();
        for f in lock(&self.inflight).values_mut() {
            f.at = t;
            f.started = f.started.map(|_| t);
        }
        if let Some((_, since)) = lock(&self.nested).as_mut() {
            *since = t;
        }
        self.pauses.fetch_add(1, Ordering::AcqRel);
        true
    }

    /// How many pauses of Cantus were seen: an instance whose count moved starts its late
    /// job's clock again (review isolation-4 #12, #22).
    pub(crate) fn pauses(&self) -> u64 {
        self.pauses.load(Ordering::Acquire)
    }

    fn route(&self, msg: Message, blob: Vec<u8>) {
        // Every frame comes from the helper's main thread: each one shows it lives (a
        // reply after a slow call is not followed by silence until the next heartbeat).
        let ms = self.started.elapsed().as_millis() as u64;
        self.alive_ms.store(ms, Ordering::Release);
        self.heard.store(true, Ordering::Release);
        match msg {
            Message::Alive => {}
            Message::Started { id } => {
                if let Some(f) = lock(&self.inflight).get_mut(&id) {
                    f.started = Some(Instant::now());
                }
            }
            Message::Nested { instance } => {
                *lock(&self.nested) = Some((instance, Instant::now()));
            }
            Message::Unnested => {
                *lock(&self.nested) = None;
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
                let flight = lock(&self.inflight).remove(&id);
                if let Some((instance, after)) = after.map(|b| *b) {
                    if let Some(inbox) = self.inbox(instance) {
                        inbox.absorb(after);
                    }
                }
                if let Some(f) = flight {
                    self.land(id, f, result, blob);
                }
            }
        }
    }

    /// An answer, where its request's purpose says it goes: the handshake checked here,
    /// everything else into its instance's inbox (and its audio halves told it is ready).
    fn land(&self, id: u64, f: Flight, result: Result<Answer, Failure>, blob: Vec<u8>) {
        let words = |f: Failure| failure(f).to_string();
        if f.purpose == Purpose::Hello {
            match result {
                Ok(Answer::Hello { protocol, .. }) if protocol == proto::PROTOCOL => {
                    // The helper has mapped its lobby: the name is no longer needed (#43).
                    self.lobby.mem.unlink_name();
                }
                Ok(Answer::Hello { protocol, .. }) => self.kill(&format!(
                    "the plug-in sandbox could not start: it speaks protocol {protocol}, \
                     Cantus {}",
                    proto::PROTOCOL
                )),
                _ => self.kill("the plug-in sandbox could not start: it did not greet"),
            }
            return;
        }
        if let Purpose::Unload(n) = f.purpose {
            lock(&self.ending.infos).remove(&n);
            return;
        }
        let Some(instance) = f.instance else {
            return;
        };
        let (inbox, status, slot) = {
            let instances = lock(&self.instances);
            let Some(r) = instances.get(&instance) else {
                return;
            };
            (r.inbox.clone(), r.status.clone(), r.slot.clone())
        };
        let mut d = inbox.data();
        match (f.purpose, result) {
            (Purpose::Load, Ok(Answer::Loaded(described))) => {
                // The helper has mapped the slot: its name is no longer needed (review #43).
                slot.unlink_name();
                // An offline instance never renders with another state than its own.
                if status.offline && described.refused_state.is_some() {
                    status.fail();
                } else {
                    status.set_ready();
                }
                d.latency = Some(described.latency);
                d.loaded = Some(Ok((described, blob)));
            }
            (Purpose::Load, r) => {
                status.fail();
                d.loaded = Some(Err(match r {
                    Err(f) => words(f),
                    Ok(_) => "the plug-in sandbox did not load it".into(),
                }));
            }
            (Purpose::SaveState, r) => {
                // A failure is delivered too, so the instance asks again later (review #8).
                d.state = Some((
                    id,
                    match r {
                        Ok(Answer::State) => Ok(blob),
                        Ok(_) => Err("no state in the answer".into()),
                        Err(f) => Err(words(f)),
                    },
                ));
            }
            (Purpose::LoadState, r) => {
                d.state_loaded = Some((id, r.map(|_| ()).map_err(words)));
            }
            (Purpose::Flush, Ok(Answer::Flushed { out, .. })) => d.flushed.extend(out),
            (Purpose::Params, Ok(Answer::Params(p))) => d.params = Some(p),
            (Purpose::Text(pid, bits), Ok(Answer::Text(text))) => {
                if d.texts.len() > 4096 {
                    d.texts.clear();
                }
                d.asked_texts.remove(&(pid, bits));
                d.texts.insert((pid, bits), text);
            }
            (Purpose::Text(pid, bits), Err(_)) => {
                d.asked_texts.remove(&(pid, bits));
            }
            (Purpose::Editor, r) => {
                d.editor = Some((
                    id,
                    match r {
                        Ok(Answer::Editor { .. }) => Ok(()),
                        Ok(_) => Err("the plug-in sandbox did not open it".into()),
                        Err(f) => Err(words(f)),
                    },
                ));
            }
            (Purpose::Restart, r) => {
                d.restarted = Some((
                    id,
                    match r {
                        Ok(Answer::Restarted(described)) => Ok(described),
                        Ok(_) => Err("restart".into()),
                        Err(f) => Err(words(f)),
                    },
                ));
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

    /// Sends a request and returns its id at once (0 when the helper is gone): its answer
    /// lands where `purpose` says, and nothing waits for it here. A request that may open
    /// a window (a licence dialog in a load, the editor) lets the helper take the
    /// foreground first, so the window is not left behind Cantus (review isolation-4 #54).
    pub(crate) fn post(
        &self,
        instance: Option<u32>,
        call: Call,
        blob: &[u8],
        purpose: Purpose,
    ) -> u64 {
        if self.is_dead() {
            return 0;
        }
        let limits = self.limits();
        let limit = match &call {
            Call::Hello { .. } | Call::Load { .. } => limits.load,
            Call::SaveState { .. } | Call::LoadState { .. } | Call::Unload { .. } => limits.state,
            _ => limits.call,
        };
        if matches!(
            call,
            Call::Load { .. }
                | Call::LoadState { .. }
                | Call::OpenEditor { .. }
                | Call::Restart { .. }
                | Call::Surface
        ) {
            allow_foreground(self.pid);
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        // Quit is never answered after the helper's end: not under way.
        if !matches!(call, Call::Quit) {
            lock(&self.inflight).insert(
                id,
                Flight {
                    at: Instant::now(),
                    started: None,
                    limit,
                    instance,
                    purpose,
                },
            );
        }
        let sent = match lock(&self.to_child).as_ref() {
            Some(tx) => tx.send(Self::frame(id, &call, blob)).is_ok(),
            None => false,
        };
        if !sent {
            lock(&self.inflight).remove(&id);
            return 0;
        }
        id
    }

    /// Brings the helper's waiting window (a plug-in's dialog) to the front: served by its
    /// reader thread at once (review isolation-4 #54; isolation-5).
    pub(crate) fn surface(&self) {
        self.post(None, Call::Surface, &[], Purpose::Ignore);
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
        info: &PluginInfo,
        (inbox, status): (Arc<Inbox>, Arc<Status>),
        slot: Arc<Slot>,
        closer: Closer,
    ) {
        lock(&self.ending.infos).insert(instance, info.clone());
        lock(&self.instances).insert(
            instance,
            Registered {
                inbox,
                slot,
                closer,
                status,
            },
        );
        if self.is_dead() {
            if let Some(r) = lock(&self.instances).get(&instance) {
                r.closer.close();
                r.status.mark_dead();
            }
        }
    }

    /// An instance left: its exchange closed first — a block the engine still sends it
    /// passes dry at once instead of waiting out its deadline for a job nobody will take
    /// (review #5) — then the helper told, and ended when it was the last: told to quit,
    /// it is given the time to end its plug-ins before it is ended (review isolation-4
    /// #46). Its plug-in stays on record until its unload is answered, so a crash in its
    /// destructor is laid on it (#45).
    pub(crate) fn unregister(&self, instance: u32) {
        let (gone, empty) = {
            let mut m = lock(&self.instances);
            let gone = m.remove(&instance);
            (gone, m.is_empty())
        };
        if let Some(r) = &gone {
            r.closer.close();
        }
        self.post(
            Some(instance),
            Call::Unload { instance },
            &[],
            Purpose::Unload(instance),
        );
        drop(gone);
        if empty {
            self.ending.quit.store(true, Ordering::Release);
            self.post(None, Call::Quit, &[], Purpose::Ignore);
            if let Some(pool) = self.pool.upgrade() {
                pool.release(self);
            }
        }
    }

    /// The heartbeat: a helper whose main thread shows no life past the silence limit,
    /// with no request under way within its own limit, is taken for hung ([`Self::hung`])
    /// and ended, laid on what it was doing — or on nobody.
    pub(crate) fn check_alive(&self) {
        if self.is_dead() || self.rebased_after_a_pause() {
            return;
        }
        if let Some(why) = self.hung() {
            self.kill_blaming(&why, self.hung_on());
        }
    }

    pub fn instances(&self) -> usize {
        lock(&self.instances).len()
    }
}

/// Lets the helper take the foreground for a window it opens on Cantus's behalf (a
/// plug-in's dialog), so it is not left behind Cantus's window (review isolation-4 #54).
#[cfg(windows)]
fn allow_foreground(pid: u32) {
    // SAFETY: a plain call with a process id; it only widens who may take the foreground.
    unsafe {
        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid);
    }
}

#[cfg(not(windows))]
fn allow_foreground(_pid: u32) {}

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
/// it ends. Once nothing in Cantus uses it, a helper told to quit is given the call limit
/// to end its plug-ins by itself — their destructors write preferences, release licences —
/// and a crash on the way out is still laid on the plug-in that did it (review isolation-4
/// #45, #46); one never told is ended at once.
fn watch(mut child: Child, kill: Arc<AtomicBool>, me: Weak<Sandbox>, ending: Arc<Ending>) {
    let mut orphaned: Option<Instant> = None;
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
                } else if !status.success() && orphaned.is_some() {
                    ending.orphan_crashed();
                }
                return;
            }
            Ok(None) => {}
            Err(_) => return,
        }
        if me.strong_count() == 0 {
            let grace = if ending.quit.load(Ordering::Acquire) {
                ending
                    .pool
                    .upgrade()
                    .map_or(CALL_TIMEOUT, |p| p.limits().call)
            } else {
                Duration::ZERO
            };
            let since = *orphaned.get_or_insert_with(Instant::now);
            if since.elapsed() >= grace {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
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
    /// How many processing threads each new helper has (`None`: its own choice).
    threads: Mutex<Option<usize>>,
    /// Plug-ins that crashed while they left their helper, for the host to count.
    departed: Mutex<Vec<DepartedCrash>>,
    /// Helper processes still running (their watchdogs count them).
    live: Arc<AtomicUsize>,
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
            threads: Mutex::new(None),
            departed: Mutex::new(Vec::new()),
            live: Arc::new(AtomicUsize::new(0)),
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

    /// The plug-ins whose crash ended their helper while they were leaving it, since last
    /// asked: the host counts them (review isolation-4 #45).
    pub fn take_departed_crashes(&self) -> Vec<DepartedCrash> {
        std::mem::take(&mut *lock(&self.departed))
    }

    /// How many processing threads the helpers started from now on have (`None`: as many
    /// as the cores beside Cantus's allow) — tests that need two jobs at once on any
    /// machine (review isolation-4 #32).
    pub fn set_threads(&self, threads: Option<usize>) {
        *lock(&self.threads) = threads;
    }

    fn threads(&self) -> Option<usize> {
        *lock(&self.threads)
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

    /// How many helper processes are still running.
    pub fn running(&self) -> usize {
        self.live.load(Ordering::Acquire)
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

/// The pool goes when Cantus quits (its last user let go): the helpers, told to quit as
/// their last plug-ins left, are given [`QUIT_GRACE`] to end by themselves before the job
/// object (or the parent-death signal) ends what is left (round 29, review isolation-5
/// #24, #54).
impl Drop for Sandboxes {
    fn drop(&mut self) {
        let start = Instant::now();
        while self.live.load(Ordering::Acquire) > 0 && start.elapsed() < QUIT_GRACE {
            std::thread::sleep(Duration::from_millis(5));
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn set(v: &[u32]) -> HashSet<u32> {
        v.iter().copied().collect()
    }

    /// Who an end is laid on (review #0, #38, #45, #20, #39; isolation-4 #2, #3, #6, #47).
    #[test]
    fn an_end_is_laid_on_one_plugin_or_on_nobody() {
        let all = [1, 2, 3];
        let seen = End::Observed;
        // A culprit Cantus names when it ends the helper alone, whoever else was busy.
        assert_eq!(culprits(End::Decided(Some(1)), 0, 3, &[2], &all), set(&[1]));
        // The crash handler's word alone.
        assert_eq!(culprits(seen, 2, 3, &[1], &all), set(&[2]));
        // The handler ran on a thread of no instance: nobody.
        assert_eq!(culprits(seen, OUTSIDE, 3, &[1], &all), set(&[]));
        // No handler: the one candidate...
        assert_eq!(culprits(seen, 0, 0, &[2], &all), set(&[2]));
        assert_eq!(culprits(seen, 0, 3, &[], &all), set(&[3]));
        // ...or nobody when several were busy (a fail-fast abort under the pool).
        assert_eq!(culprits(seen, 0, 0, &[1, 2], &all), set(&[]));
        assert_eq!(culprits(seen, 0, 3, &[1], &all), set(&[]));
        // One plug-in in it: its own end.
        assert_eq!(culprits(seen, 0, 0, &[], &[7]), set(&[7]));
    }

    /// The only plug-in of a helper is laid its end whatever thread it crashed on — its
    /// own, a framework's timer (review isolation-4 #6, #18, #29, #49).
    #[test]
    fn the_only_plugin_is_laid_its_end_from_any_thread() {
        assert_eq!(culprits(End::Observed, OUTSIDE, 0, &[], &[7]), set(&[7]));
    }

    /// A plug-in that crashed while it left (its destructor) — by the handler's word or,
    /// with none, the main thread's — never lays it on one that stays (review isolation-4
    /// #2, #45, #56).
    #[test]
    fn a_plugin_crashing_as_it_leaves_is_laid_its_own_end() {
        let seen = End::Observed;
        assert_eq!(culprits(seen, 9, 0, &[], &[7]), set(&[9]));
        assert_eq!(culprits(seen, 0, 9, &[], &[7]), set(&[9]));
        assert_eq!(culprits(seen, 9, 0, &[2], &[1, 2, 3]), set(&[9]));
    }

    /// A crash recorded while Cantus ended the helper for a hang stands (review isolation-4
    /// #47).
    #[test]
    fn a_recorded_crash_stands_over_a_decided_hang() {
        assert_eq!(
            culprits(End::Decided(Some(1)), 2, 0, &[], &[1, 2, 3]),
            set(&[2])
        );
    }

    /// A hang Cantus judged with nothing named is nobody's — not a bystander's whose job was
    /// running — but the only plug-in's (review isolation-4 #3, #30, #44, #55).
    #[test]
    fn a_hang_with_nothing_named_is_nobodys() {
        assert_eq!(
            culprits(End::Decided(None), 0, 0, &[2], &[1, 2, 3]),
            set(&[])
        );
        assert_eq!(culprits(End::Decided(None), 0, 0, &[], &[7]), set(&[7]));
    }

    /// A gap longer than the silence limit since the last judgement is Cantus's own pause.
    #[test]
    fn a_pause_of_cantus_is_not_the_helpers_silence() {
        let s = Duration::from_secs(5);
        assert!(!paused(0, 60_000, s), "the first judgement");
        assert!(!paused(10_000, 14_000, s));
        assert!(paused(10_000, 70_000, s));
    }
}
