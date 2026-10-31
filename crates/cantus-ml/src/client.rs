//! Cantus's side of the helper: [`MlClient`] queues jobs, starts `cantus-ml` when one first
//! needs it, sends it one job at a time, and turns what it says — or its silence, or its
//! end — into [`Update`]s the interface polls once a frame. Everything that waits (the
//! process start, its pipes, the watchdog) runs on the client's own threads: the caller's
//! thread never blocks, unless it asks to with [`MlClient::wait`].
//!
//! A helper that ends while a job runs fails that job with [`Failure::Crashed`] (how it
//! ended, as the system says); one that says nothing for [`ClientConfig::silence`] while a
//! job runs is ended and the job fails with [`Failure::Hung`]. Either way the next job
//! starts a fresh helper; queued jobs are not lost.

use crate::proto::{self, Event, JobError, Request};
use serde_json::Value;
use std::collections::VecDeque;
use std::io::{BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

pub type JobId = u64;

/// How the client starts and watches the helper.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// The `cantus-ml` executable.
    pub exe: PathBuf,
    /// A running job silent this long means a hung helper. A segment of HT-Demucs takes
    /// about two seconds on a laptop's processor and reports after each; the default leaves
    /// room for a slow machine loading a model from a slow disk.
    pub silence: Duration,
    /// The helper must greet within this.
    pub start: Duration,
    /// Start the helper for tests (`CANTUS_ML_TEST_HOOKS=1`: the fixture models).
    pub test_hooks: bool,
    /// More environment for the helper (tests).
    pub env: Vec<(String, String)>,
    /// A helper with nothing to do this long is ended: its runtime keeps gigabytes and,
    /// on Windows, its library locked (review 1 #3). The next job starts another.
    pub idle: Duration,
}

impl ClientConfig {
    pub fn new(exe: impl Into<PathBuf>) -> ClientConfig {
        ClientConfig {
            exe: exe.into(),
            silence: Duration::from_secs(120),
            start: Duration::from_secs(30),
            idle: Duration::from_secs(180),
            test_hooks: false,
            env: Vec::new(),
        }
    }

    /// The helper beside the running program (`cantus-ml.exe` beside `cantus.exe`, in the
    /// same folder of the macOS bundle), or `CANTUS_ML_EXE` (development and tests).
    pub fn default_exe() -> PathBuf {
        if let Some(p) = std::env::var_os("CANTUS_ML_EXE") {
            return PathBuf::from(p);
        }
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Self::exe_in))
            .unwrap_or_else(|| PathBuf::from(HELPER))
    }

    /// The helper of a program installed in `dir`: on Windows in its own folder `ml\`,
    /// beside the Visual C++ runtime it ships with (which no other program of the install
    /// folder may load), else beside the program (a development build, Linux, macOS).
    pub fn exe_in(dir: &std::path::Path) -> PathBuf {
        let own = dir.join("ml").join(HELPER);
        if cfg!(windows) && own.is_file() {
            own
        } else {
            dir.join(HELPER)
        }
    }
}

/// The helper's file name.
const HELPER: &str = if cfg!(windows) {
    "cantus-ml.exe"
} else {
    "cantus-ml"
};

/// What the client tells its caller.
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    /// The helper began the job (it had waited in the queue until then).
    Started(JobId),
    Progress {
        id: JobId,
        done: f32,
        stage: String,
    },
    Ended(JobId, Outcome),
    /// A line for Cantus's log, from the helper or about it.
    Log {
        level: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Done(Value),
    Cancelled,
    Failed(Failure),
}

/// Why a job failed.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum Failure {
    /// The job's own failure, as the helper reported it.
    #[error(transparent)]
    Job(JobError),
    /// The helper ended while the job ran; how, as the system says.
    #[error("the model's helper process ended while it worked ({0})")]
    Crashed(String),
    /// The helper said nothing for the silence limit and was ended.
    #[error("the model's helper process stopped answering and was ended")]
    Hung,
    /// The helper could not be started, or did not greet in time.
    #[error("the model's helper process could not be started: {0}")]
    Start(String),
    /// Another build's helper, or one that does not speak the protocol.
    #[error("the model's helper process does not speak this Cantus's protocol: {0}")]
    Protocol(String),
}

enum In {
    Start(JobId, String, Value),
    Cancel(JobId),
    Release,
    Shutdown,
    Line(u64, String),
    Closed(u64),
}

/// The client. Dropping it ends the helper.
pub struct MlClient {
    tx: Sender<In>,
    updates: Receiver<Update>,
    pending: VecDeque<Update>,
    next: JobId,
    pid: Arc<AtomicU32>,
    service: Option<std::thread::JoinHandle<()>>,
}

impl MlClient {
    pub fn new(cfg: ClientConfig) -> MlClient {
        let (tx, rx) = channel();
        let (utx, updates) = channel();
        let pid = Arc::new(AtomicU32::new(0));
        let service = {
            let tx = tx.clone();
            let pid = pid.clone();
            std::thread::Builder::new()
                .name("cantus-ml-client".into())
                .spawn(move || Service::new(cfg, tx, utx, pid).run(rx))
                .ok()
        };
        MlClient {
            tx,
            updates,
            pending: VecDeque::new(),
            next: 1,
            pid,
            service,
        }
    }

    /// Queue job `job` with its arguments; its updates carry the id returned.
    pub fn start(&mut self, job: &str, args: Value) -> JobId {
        let id = self.next;
        self.next += 1;
        let _ = self.tx.send(In::Start(id, job.to_string(), args));
        id
    }

    /// Cancel a queued or running job; it ends with [`Outcome::Cancelled`] (or with its own
    /// end, if that came first).
    pub fn cancel(&self, id: JobId) {
        let _ = self.tx.send(In::Cancel(id));
    }

    /// What happened since the last call. Never blocks.
    pub fn poll(&mut self) -> Vec<Update> {
        let mut v: Vec<Update> = self.pending.drain(..).collect();
        v.extend(self.updates.try_iter());
        v
    }

    /// Block until job `id` ends, or `timeout` passes (`None`). Other jobs' updates are
    /// kept for [`MlClient::poll`]. Not for the interface's thread.
    pub fn wait(&mut self, id: JobId, timeout: Duration) -> Option<Outcome> {
        let deadline = Instant::now() + timeout;
        if let Some(i) = self
            .pending
            .iter()
            .position(|u| matches!(u, Update::Ended(j, _) if *j == id))
        {
            if let Some(Update::Ended(_, o)) = self.pending.remove(i) {
                return Some(o);
            }
        }
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match self.updates.recv_timeout(left) {
                Ok(Update::Ended(j, o)) if j == id => return Some(o),
                Ok(u) => self.pending.push_back(u),
                Err(_) => return None,
            }
        }
    }

    /// End the helper as soon as no job runs (at once if none does): its memory and its
    /// files go back (a model's library can then be removed). Queued jobs start another.
    pub fn release(&self) {
        let _ = self.tx.send(In::Release);
    }

    /// The running helper's process id (0 when none runs): for tests that end it.
    pub fn helper_pid(&self) -> u32 {
        self.pid.load(Ordering::Relaxed)
    }
}

impl Drop for MlClient {
    fn drop(&mut self) {
        let _ = self.tx.send(In::Shutdown);
        if let Some(h) = self.service.take() {
            let _ = h.join();
        }
    }
}

/// One job object for every helper this process starts (Windows; nothing elsewhere).
fn job_object() -> Option<&'static crate::platform::Job> {
    static JOB: OnceLock<Option<crate::platform::Job>> = OnceLock::new();
    JOB.get_or_init(crate::platform::Job::new).as_ref()
}

struct Helper {
    child: Child,
    stdin: Option<ChildStdin>,
    started: Instant,
    greeted: bool,
}

struct Service {
    cfg: ClientConfig,
    tx: Sender<In>,
    out: Sender<Update>,
    pid: Arc<AtomicU32>,
    helper: Option<Helper>,
    generation: u64,
    queue: VecDeque<(JobId, String, Value)>,
    /// The job the helper is running, and when it last said anything.
    running: Option<(JobId, Instant)>,
    /// Since when the helper has had nothing to do.
    idle_since: Option<Instant>,
    /// End the helper once no job runs.
    release: bool,
}

/// How a process ended, in words.
fn how(status: Option<std::process::ExitStatus>) -> String {
    let Some(s) = status else {
        return "it could not be asked how".into();
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = s.signal() {
            return format!("signal {sig}");
        }
    }
    match s.code() {
        Some(c) if cfg!(windows) && c < 0 => format!("exception 0x{:08X}", c as u32),
        Some(c) => format!("exit code {c}"),
        None => "no exit code".into(),
    }
}

impl Service {
    fn new(cfg: ClientConfig, tx: Sender<In>, out: Sender<Update>, pid: Arc<AtomicU32>) -> Service {
        Service {
            cfg,
            tx,
            out,
            pid,
            helper: None,
            generation: 0,
            queue: VecDeque::new(),
            running: None,
            idle_since: None,
            release: false,
        }
    }

    fn emit(&self, u: Update) {
        let _ = self.out.send(u);
    }

    fn log(&self, level: &str, message: String) {
        self.emit(Update::Log {
            level: level.into(),
            message,
        });
    }

    fn run(mut self, rx: Receiver<In>) {
        loop {
            self.dispatch();
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(In::Start(id, job, args)) => self.queue.push_back((id, job, args)),
                Ok(In::Cancel(id)) => self.cancel(id),
                Ok(In::Release) => self.release = true,
                Ok(In::Line(g, line)) if g == self.generation => self.line(&line),
                Ok(In::Closed(g)) if g == self.generation => self.ended(),
                Ok(In::Line(..)) | Ok(In::Closed(_)) => {}
                Ok(In::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.watch();
        }
        self.shutdown();
    }

    /// Start the helper if a job waits and none runs; send the next job once it greeted.
    fn dispatch(&mut self) {
        if self.running.is_some() || self.queue.is_empty() {
            return;
        }
        if self.helper.is_none() {
            if let Err(e) = self.spawn() {
                let (id, _, _) = self.queue.pop_front().expect("a job waits");
                self.emit(Update::Ended(id, Outcome::Failed(Failure::Start(e))));
                return;
            }
        }
        let Some(h) = self.helper.as_mut() else {
            return;
        };
        if !h.greeted {
            return;
        }
        let (id, job, args) = self.queue.pop_front().expect("a job waits");
        let sent = h
            .stdin
            .as_mut()
            .map(|w| proto::write_line(w, &Request::Start { id, job, args }).is_ok())
            .unwrap_or(false);
        self.running = Some((id, Instant::now()));
        if !sent {
            // The pipe is gone: the helper is ending; its end fails the job.
            self.log("warn", "the model helper's input closed".into());
        }
    }

    fn spawn(&mut self) -> Result<(), String> {
        let mut cmd = Command::new(&self.cfg.exe);
        cmd.arg("--serve")
            .arg(proto::PROTOCOL.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            // ONNX Runtime's telemetry stays off (ADR 0085): set before the library loads.
            .env("ORT_DISABLE_TELEMETRY", "1");
        if self.cfg.test_hooks {
            cmd.env("CANTUS_ML_TEST_HOOKS", "1");
        }
        for (k, v) in &self.cfg.env {
            cmd.env(k, v);
        }
        crate::platform::no_window(&mut cmd);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("{}: {e}", self.cfg.exe.display()))?;
        if let Some(job) = job_object() {
            if !job.assign(&child) {
                self.log(
                    "warn",
                    "the model helper could not join Cantus's job object".into(),
                );
            }
        }
        self.generation += 1;
        let generation = self.generation;
        let stdout = child.stdout.take().expect("piped");
        let tx = self.tx.clone();
        let reader = std::thread::Builder::new()
            .name("cantus-ml-read".into())
            .spawn(move || {
                let mut r = BufReader::new(stdout);
                let mut line = String::new();
                while let Ok(true) = proto::read_line(&mut r, &mut line) {
                    if tx.send(In::Line(generation, line.clone())).is_err() {
                        return;
                    }
                }
                let _ = tx.send(In::Closed(generation));
            });
        if let Err(e) = reader {
            let _ = child.kill();
            let _ = child.wait();
            return Err(e.to_string());
        }
        self.pid.store(child.id(), Ordering::Relaxed);
        self.helper = Some(Helper {
            stdin: child.stdin.take(),
            child,
            started: Instant::now(),
            greeted: false,
        });
        Ok(())
    }

    fn line(&mut self, line: &str) {
        let e = match serde_json::from_str::<Event>(line) {
            Ok(e) => e,
            Err(err) => {
                self.log(
                    "warn",
                    format!("the model helper said something unreadable: {err}"),
                );
                return;
            }
        };
        if let Some((id, t)) = self.running.as_mut() {
            if e.id() == Some(*id) {
                *t = Instant::now();
            }
        }
        match e {
            Event::Hello { protocol, .. } if protocol != proto::PROTOCOL => {
                let why = format!(
                    "it speaks protocol {protocol}, this Cantus {}",
                    proto::PROTOCOL
                );
                self.kill();
                if let Some((id, _, _)) = self.queue.pop_front() {
                    self.emit(Update::Ended(id, Outcome::Failed(Failure::Protocol(why))));
                }
            }
            Event::Hello { .. } => {
                if let Some(h) = self.helper.as_mut() {
                    h.greeted = true;
                }
            }
            Event::Started { id } => self.emit(Update::Started(id)),
            Event::Progress { id, done, stage } => self.emit(Update::Progress { id, done, stage }),
            Event::Log { level, message } => self.emit(Update::Log { level, message }),
            Event::Done { id, result } => self.finish(id, Outcome::Done(result)),
            Event::Failed { id, error } => self.finish(id, Outcome::Failed(Failure::Job(error))),
            Event::Cancelled { id } => self.finish(id, Outcome::Cancelled),
        }
    }

    fn finish(&mut self, id: JobId, o: Outcome) {
        if self.running.map(|(r, _)| r) == Some(id) {
            self.running = None;
        }
        self.emit(Update::Ended(id, o));
    }

    fn cancel(&mut self, id: JobId) {
        if let Some(i) = self.queue.iter().position(|(j, _, _)| *j == id) {
            self.queue.remove(i);
            self.emit(Update::Ended(id, Outcome::Cancelled));
            return;
        }
        if self.running.map(|(r, _)| r) == Some(id) {
            if let Some(w) = self.helper.as_mut().and_then(|h| h.stdin.as_mut()) {
                let _ = proto::write_line(w, &Request::Cancel { id });
            }
        }
    }

    /// The helper's output closed: it is ending or has ended.
    fn ended(&mut self) {
        let Some(mut h) = self.helper.take() else {
            return;
        };
        self.pid.store(0, Ordering::Relaxed);
        drop(h.stdin.take());
        // The pipe closes as the process ends; give the system a moment to say how.
        let t = Instant::now();
        let mut status = None;
        while t.elapsed() < Duration::from_secs(2) {
            if let Ok(Some(s)) = h.child.try_wait() {
                status = Some(s);
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if status.is_none() {
            let _ = h.child.kill();
            status = h.child.wait().ok();
        }
        let why = how(status);
        if let Some((id, _)) = self.running.take() {
            self.emit(Update::Ended(
                id,
                Outcome::Failed(Failure::Crashed(why.clone())),
            ));
        } else if !h.greeted {
            if let Some((id, _, _)) = self.queue.pop_front() {
                self.emit(Update::Ended(
                    id,
                    Outcome::Failed(Failure::Start(format!("it ended at once ({why})"))),
                ));
            }
        }
        self.log("info", format!("the model helper ended ({why})"));
    }

    fn kill(&mut self) {
        // Lines the killed helper left in the pipe are not this one's (review 1 #6).
        self.generation += 1;
        if let Some(mut h) = self.helper.take() {
            self.pid.store(0, Ordering::Relaxed);
            let _ = h.child.kill();
            let _ = h.child.wait();
        }
    }

    /// The watchdog: a helper that does not greet, a job that says nothing, a helper with
    /// nothing to do.
    fn watch(&mut self) {
        let Some(h) = self.helper.as_ref() else {
            self.release = false;
            return;
        };
        let idle = h.greeted && self.running.is_none() && self.queue.is_empty();
        if !idle {
            self.idle_since = None;
        } else {
            let since = *self.idle_since.get_or_insert_with(Instant::now);
            if self.release || since.elapsed() > self.cfg.idle {
                self.release = false;
                self.idle_since = None;
                self.end_helper();
                return;
            }
        }
        if !h.greeted && h.started.elapsed() > self.cfg.start {
            let secs = self.cfg.start.as_secs_f32();
            self.kill();
            if let Some((id, _, _)) = self.queue.pop_front() {
                self.emit(Update::Ended(
                    id,
                    Outcome::Failed(Failure::Start(format!("it did not answer in {secs} s"))),
                ));
            }
            return;
        }
        if let Some((id, t)) = self.running {
            if t.elapsed() > self.cfg.silence {
                self.kill();
                self.running = None;
                self.emit(Update::Ended(id, Outcome::Failed(Failure::Hung)));
            }
        }
    }

    /// End an idle helper: asked to quit, killed if it does not within 3 s.
    fn end_helper(&mut self) {
        self.generation += 1;
        if let Some(mut h) = self.helper.take() {
            self.pid.store(0, Ordering::Relaxed);
            if let Some(mut w) = h.stdin.take() {
                let _ = proto::write_line(&mut w, &Request::Quit);
                let _ = w.flush();
            }
            let t = Instant::now();
            while t.elapsed() < Duration::from_secs(3) {
                if let Ok(Some(_)) = h.child.try_wait() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = h.child.kill();
            let _ = h.child.wait();
        }
    }

    fn shutdown(&mut self) {
        for (id, _, _) in std::mem::take(&mut self.queue) {
            self.emit(Update::Ended(id, Outcome::Cancelled));
        }
        if let Some(mut h) = self.helper.take() {
            if let Some(mut w) = h.stdin.take() {
                let _ = proto::write_line(&mut w, &Request::Quit);
                let _ = w.flush();
            }
            let t = Instant::now();
            while t.elapsed() < Duration::from_secs(3) {
                if let Ok(Some(_)) = h.child.try_wait() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            let _ = h.child.kill();
            let _ = h.child.wait();
        }
        if let Some((id, _)) = self.running.take() {
            self.emit(Update::Ended(id, Outcome::Cancelled));
        }
        self.pid.store(0, Ordering::Relaxed);
    }
}
