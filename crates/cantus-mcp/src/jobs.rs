//! Long jobs (ADR 0081 §2): a tool's worker part runs on a thread of its own with a handle
//! the agent waits on (`wait_job`) or cancels (`cancel_job`); its progress is an atomic the
//! activity view reads; when it ends it wakes the window, and the UI thread takes its
//! result (and plans its edit, `JobDone::Then`, there). A panic in a job is its failure,
//! never the application's.

use crate::door::{Caller, Reply, Waker};
use cantus_ai::{JobCx, JobDone, JobWork, ToolError};
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Where a job is.
#[derive(Clone, Debug, PartialEq)]
pub enum JobState {
    Running,
    Done(Value),
    Failed(String),
    Cancelled,
}

impl JobState {
    pub fn word(&self) -> &'static str {
        match self {
            JobState::Running => "running",
            JobState::Done(_) => "done",
            JobState::Failed(_) => "failed",
            JobState::Cancelled => "cancelled",
        }
    }
}

/// One job, as the activity view and `list_jobs` see it.
pub struct JobEntry {
    pub handle: String,
    pub owner: Caller,
    pub tool: &'static str,
    /// What it does, localised.
    pub said: String,
    /// The project it was started in: its edit lands only there.
    pub epoch: u64,
    /// The project's revision when it started, the change it joins and whether the user
    /// was asked to start it: its edit is decided again when it ends (review 2 #2, #10).
    pub(crate) start: crate::jobs::JobStart,
    /// Its edit waiting on the card: the answer to come, and the job's own answer.
    pub(crate) awaiting: Option<(std::sync::mpsc::Receiver<crate::door::UiReply>, Value, u64)>,
    pub started: Instant,
    pub ended: Option<Instant>,
    pub state: JobState,
    progress: Arc<AtomicU32>,
    cancel: Arc<AtomicBool>,
}

impl JobEntry {
    /// 0…1, as the job last said.
    pub fn progress(&self) -> f32 {
        f32::from_bits(self.progress.load(Ordering::Relaxed))
    }

    pub fn cancel_requested(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// A job that ended well is all done, whatever it last said.
    pub(crate) fn progress_done(&self) {
        if matches!(self.state, JobState::Done(_)) {
            self.progress.store(1f32.to_bits(), Ordering::Relaxed);
        }
    }

    /// What `wait_job` and `list_jobs` say of it.
    pub fn describe(&self, now: Instant) -> Value {
        let mut v = json!({
            "job": self.handle,
            "tool": self.tool,
            "said": self.said,
            "state": if self.state == JobState::Running && self.cancel_requested() {
                "cancelling"
            } else {
                self.state.word()
            },
            "progress": (self.progress() * 1000.0).round() / 1000.0,
            "seconds": now.saturating_duration_since(self.started).as_secs_f64().round(),
        });
        match &self.state {
            JobState::Done(r) => v["result"] = r.clone(),
            JobState::Failed(e) => v["error"] = json!(e),
            _ => {}
        }
        v
    }
}

/// What a job started from (review 2 #2, #10).
#[derive(Clone, Debug, Default)]
pub(crate) struct JobStart {
    /// The project it started in.
    pub epoch: u64,
    pub revision: u64,
    pub change: Option<String>,
    pub asked: bool,
}

/// Someone waiting for a job: the call that started it (answered with the handle if it
/// runs past its inline wait) or a `wait_job`.
pub(crate) struct Waiter {
    pub reply: Reply,
    pub deadline: Instant,
    pub starting_call: bool,
    /// The starting call's own flag: set when its client cancelled it or hung up before
    /// it was answered — the job is cancelled too, as nobody holds its handle (review 4
    /// #7).
    pub cancel: Option<Arc<AtomicBool>>,
}

type Finished = (String, Result<JobDone, ToolError>);

/// Every job of this window.
pub struct Jobs {
    pub(crate) entries: Vec<JobEntry>,
    pub(crate) waiters: Vec<(String, Waiter)>,
    tx: Sender<Finished>,
    rx: Receiver<Finished>,
}

impl Default for Jobs {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Jobs {
            entries: Vec::new(),
            waiters: Vec::new(),
            tx,
            rx,
        }
    }
}

impl Jobs {
    pub fn get(&self, handle: &str) -> Option<&JobEntry> {
        self.entries.iter().find(|e| e.handle == handle)
    }

    pub(crate) fn get_mut(&mut self, handle: &str) -> Option<&mut JobEntry> {
        self.entries.iter_mut().find(|e| e.handle == handle)
    }

    pub fn iter(&self) -> impl Iterator<Item = &JobEntry> {
        self.entries.iter()
    }

    /// Jobs still running, of one client or of all.
    pub fn running(&self, owner: Option<&str>) -> usize {
        self.entries
            .iter()
            .filter(|e| e.state == JobState::Running && owner.is_none_or(|o| e.owner.key == o))
            .count()
    }

    /// Start `work` on a thread; the handle names it.
    pub(crate) fn start(
        &mut self,
        owner: Caller,
        tool: &'static str,
        said: String,
        start: JobStart,
        work: JobWork,
        wake: Waker,
    ) -> Result<String, ToolError> {
        let handle = crate::ids::fresh("job");
        let progress = Arc::new(AtomicU32::new(0f32.to_bits()));
        let cancel = Arc::new(AtomicBool::new(false));
        let (p, c, tx, h) = (
            progress.clone(),
            cancel.clone(),
            self.tx.clone(),
            handle.clone(),
        );
        std::thread::Builder::new()
            .name("cantus-mcp-job".into())
            .spawn(move || {
                let report = move |x: f32| {
                    let x = if x.is_finite() {
                        x.clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    p.store(x.to_bits(), Ordering::Relaxed);
                };
                let asked = move || c.load(Ordering::Relaxed);
                let cx = JobCx {
                    progress: &report,
                    cancelled: &asked,
                };
                let out = catch_unwind(AssertUnwindSafe(|| work(&cx))).unwrap_or_else(|_| {
                    Err(ToolError::Failed(
                        "the job stopped on an internal error".into(),
                    ))
                });
                let _ = tx.send((h, out));
                wake();
            })
            .map_err(|e| ToolError::Failed(format!("could not start the job: {e}")))?;
        self.entries.push(JobEntry {
            handle: handle.clone(),
            owner,
            tool,
            said,
            epoch: start.epoch,
            start,
            awaiting: None,
            started: Instant::now(),
            ended: None,
            state: JobState::Running,
            progress,
            cancel,
        });
        Ok(handle)
    }

    /// The jobs that ended since the last frame.
    pub(crate) fn drain_finished(&mut self) -> Vec<Finished> {
        self.rx.try_iter().collect()
    }

    /// Ended jobs older than `keep` go, and the oldest beyond 64.
    pub(crate) fn prune(&mut self, now: Instant, keep: Duration) {
        self.entries.retain(|e| {
            e.ended
                .is_none_or(|t| now.saturating_duration_since(t) < keep)
        });
        let ended = self.entries.iter().filter(|e| e.ended.is_some()).count();
        let mut excess = ended.saturating_sub(64);
        self.entries.retain(|e| {
            if excess > 0 && e.ended.is_some() {
                excess -= 1;
                false
            } else {
                true
            }
        });
    }
}
