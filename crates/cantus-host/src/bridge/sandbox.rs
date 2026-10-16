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

/// How long the helper's main thread may stay silent (no heartbeat) while no call is under
/// way before it is taken for hung.
pub const SILENCE_LIMIT: Duration = Duration::from_secs(5);

/// How long a job may stay unfinished before the helper is taken for hung (its audio
/// passes dry meanwhile, never waited for).
pub const HANG_LIMIT: Duration = Duration::from_secs(2);

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
    /// A state asked for in the background, once it arrives.
    pub state: Option<Vec<u8>>,
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

/// What a background request was for.
#[derive(Clone, Copy, Debug)]
enum Background {
    Text(u32, u32, u64),
    State(u32),
    Ignore,
}

struct Registered {
    inbox: Arc<Inbox>,
    /// Kept so the closer's mapping outlives it.
    _slot: Arc<Slot>,
    closer: Closer,
}

/// One helper process.
pub struct Sandbox {
    pub key: String,
    pub pid: u32,
    to_child: Mutex<Option<mpsc::Sender<Vec<u8>>>>,
    waiters: Mutex<HashMap<u64, mpsc::Sender<(Result<Answer, Failure>, Vec<u8>)>>>,
    background: Mutex<HashMap<u64, Background>>,
    instances: Mutex<HashMap<u32, Registered>>,
    next_id: AtomicU64,
    next_instance: AtomicU32,
    dead: AtomicBool,
    why: Mutex<Option<String>>,
    kill: Arc<AtomicBool>,
    started: Instant,
    /// Milliseconds since `started` of the last heartbeat.
    alive_ms: AtomicU64,
    /// Calls waiting for an answer now.
    busy: AtomicU32,
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
        let mut child = Command::new(&pool.exe)
            .arg("--serve")
            .arg(proto::PROTOCOL.to_string())
            .arg("--crt-fma3")
            .arg(fma3)
            .arg("--group")
            .arg(key)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| {
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
            to_child: Mutex::new(Some(tx)),
            waiters: Mutex::new(HashMap::new()),
            background: Mutex::new(HashMap::new()),
            instances: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            next_instance: AtomicU32::new(1),
            dead: AtomicBool::new(false),
            why: Mutex::new(None),
            kill: Arc::new(AtomicBool::new(false)),
            started: Instant::now(),
            alive_ms: AtomicU64::new(0),
            busy: AtomicU32::new(0),
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
                        s.mark_dead("the plug-in sandbox ended");
                        return;
                    }
                    Err(e) => {
                        s.mark_dead(&format!("the plug-in sandbox broke its protocol: {e}"));
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
        match sandbox.call(None, Call::Hello { protocol: proto::PROTOCOL }, &[], LOAD_TIMEOUT) {
            Ok((Answer::Hello { protocol, .. }, _)) if protocol == proto::PROTOCOL => Ok(sandbox),
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

    /// Why it ended, once it has.
    pub fn why(&self) -> Option<String> {
        lock(&self.why).clone()
    }

    /// Marks it gone: every instance's exchange closed (waking an audio thread that waits
    /// on one), every waiting call answered with an error.
    pub(crate) fn mark_dead(&self, why: &str) {
        {
            let mut w = lock(&self.why);
            if w.is_none() {
                *w = Some(why.to_string());
            }
        }
        if self.dead.swap(true, Ordering::AcqRel) {
            return;
        }
        for r in lock(&self.instances).values() {
            r.closer.close();
        }
        lock(&self.waiters).clear();
        *lock(&self.to_child) = None;
    }

    fn request_kill(&self) {
        self.kill.store(true, Ordering::Release);
    }

    /// Ends the helper (it hung): marks it gone now, kills it in the watchdog.
    pub fn kill(&self, why: &str) {
        self.mark_dead(why);
        self.request_kill();
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
                if let Some((instance, after)) = after {
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
            (Background::State(instance), Ok(Answer::State)) => {
                if let Some(inbox) = self.inbox(instance) {
                    inbox.data().state = Some(blob);
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
        let _ = instance;
        if self.is_dead() {
            return Err(self.gone());
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel();
        lock(&self.waiters).insert(id, tx);
        self.busy.fetch_add(1, Ordering::AcqRel);
        let sent = self.post(Self::frame(id, &call, blob));
        let got = if sent {
            rx.recv_timeout(timeout)
        } else {
            Err(mpsc::RecvTimeoutError::Disconnected)
        };
        self.busy.fetch_sub(1, Ordering::AcqRel);
        match got {
            Ok((Ok(answer), blob)) => Ok((answer, blob)),
            Ok((Err(f), _)) => Err(failure(f)),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                lock(&self.waiters).remove(&id);
                self.kill(&format!(
                    "the plug-in sandbox did not answer in {} s",
                    timeout.as_secs()
                ));
                Err(self.gone())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(self.gone()),
        }
    }

    /// A call whose answer is not waited for.
    pub(crate) fn tell(&self, call: Call, blob: &[u8]) {
        if self.is_dead() {
            return;
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.background).insert(id, Background::Ignore);
        self.post(Self::frame(id, &call, blob));
    }

    /// A text asked for in the background; the answer lands in the instance's inbox.
    pub(crate) fn ask_text(&self, instance: u32, id: u32, value: f64) {
        if self.is_dead() {
            return;
        }
        let rid = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.background).insert(rid, Background::Text(instance, id, value.to_bits()));
        self.post(Self::frame(rid, &Call::ParamText { instance, id, value }, &[]));
    }

    /// A state asked for in the background; it lands in the instance's inbox.
    pub(crate) fn ask_state(&self, instance: u32) {
        if self.is_dead() {
            return;
        }
        let rid = self.next_id.fetch_add(1, Ordering::Relaxed);
        lock(&self.background).insert(rid, Background::State(instance));
        self.post(Self::frame(rid, &Call::SaveState { instance }, &[]));
    }

    pub(crate) fn new_instance(&self) -> u32 {
        self.next_instance.fetch_add(1, Ordering::Relaxed)
    }

    pub(crate) fn register(&self, instance: u32, inbox: Arc<Inbox>, slot: Arc<Slot>, closer: Closer) {
        lock(&self.instances).insert(
            instance,
            Registered {
                inbox,
                _slot: slot,
                closer,
            },
        );
        if self.is_dead() {
            if let Some(r) = lock(&self.instances).get(&instance) {
                r.closer.close();
            }
        }
    }

    /// An instance left: tell the helper, and end the helper when it was the last.
    pub(crate) fn unregister(&self, instance: u32) {
        self.tell(Call::Unload { instance }, &[]);
        let empty = {
            let mut m = lock(&self.instances);
            m.remove(&instance);
            m.is_empty()
        };
        if empty {
            self.tell(Call::Quit, &[]);
            if let Some(pool) = self.pool.upgrade() {
                pool.release(self);
            }
        }
    }

    /// The heartbeat: a helper whose main thread has been silent too long, while no call
    /// is under way (a call has its own time limit), is taken for hung.
    pub(crate) fn check_alive(&self) {
        if self.is_dead() || self.busy.load(Ordering::Acquire) > 0 {
            return;
        }
        let now = self.started.elapsed().as_millis() as u64;
        let last = self.alive_ms.load(Ordering::Acquire);
        if now.saturating_sub(last) > SILENCE_LIMIT.as_millis() as u64 {
            self.kill("the plug-in sandbox stopped answering");
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
                        "the plug-in sandbox ended".to_string()
                    } else {
                        format!("the plug-in sandbox crashed ({status})")
                    };
                    s.mark_dead(&why);
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
    /// How long a job may stay unfinished before its helper is taken for hung.
    hang_limit_ms: AtomicU64,
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
            hang_limit_ms: AtomicU64::new(HANG_LIMIT.as_millis() as u64),
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

    pub fn hang_limit(&self) -> Duration {
        Duration::from_millis(self.hang_limit_ms.load(Ordering::Relaxed))
    }

    pub fn set_hang_limit(&self, limit: Duration) {
        self.hang_limit_ms
            .store(limit.as_millis() as u64, Ordering::Relaxed);
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
