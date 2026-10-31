//! The helper's job table: every job `cantus-ml` can run, by the name a `start` request
//! gives. **Adding a job** (audio to MIDI, say) is three steps and touches no protocol:
//!
//! 1. its arguments and result as serde types beside `spec::SeparateArgs` (a file of the
//!    job's own, e.g. `transcribe_spec.rs`), shared by both sides;
//! 2. a function `fn job(args: serde_json::Value, ctx: &mut Ctx) -> Result<serde_json::Value,
//!    Stop>` that parses them ([`args`]), runs — reporting with `ctx.report.progress`,
//!    checking `ctx.report.check()?` between steps, getting its model from
//!    `ctx.runtime.model(…)` and running it with `Model::run` — and returns its result
//!    ([`result`]);
//! 3. one line in [`JOBS`].
//!
//! The client starts it with `MlClient::start("<name>", args)`; the hello names it, so a
//! Cantus can tell an older helper that lacks it.

use crate::proto::JobError;
use crate::runtime::Runtime;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// How a job ends early.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    Cancelled,
    Failed(JobError),
}

impl From<JobError> for Stop {
    fn from(e: JobError) -> Self {
        Stop::Failed(e)
    }
}

/// A job: its arguments in, its result out.
pub type JobFn = fn(serde_json::Value, &mut Ctx) -> Result<serde_json::Value, Stop>;

/// Every job this helper runs.
pub const JOBS: &[(&str, JobFn)] = &[
    ("separate", crate::separate::job),
    ("transcribe", crate::transcribe::job), // r34:transcribe
];

/// What a running job is given. Its fields are apart so a job can hold the runtime (a
/// loaded model borrows it) and report at the same time.
pub struct Ctx<'a> {
    pub id: u64,
    /// The inference runtime, loaded once per helper and kept between jobs.
    pub runtime: &'a mut Runtime,
    pub report: Report<'a>,
    /// Whether tests started this helper (`CANTUS_ML_TEST_HOOKS=1`): only then are the
    /// fixture models taken.
    pub test_hooks: bool,
}

/// Progress, the log and the cancel flag.
pub struct Report<'a> {
    cancel: Arc<AtomicBool>,
    progress: &'a mut dyn FnMut(f32, &str),
    log: &'a mut dyn FnMut(&str, &str),
}

impl<'a> Report<'a> {
    pub fn new(
        cancel: Arc<AtomicBool>,
        progress: &'a mut dyn FnMut(f32, &str),
        log: &'a mut dyn FnMut(&str, &str),
    ) -> Report<'a> {
        Report {
            cancel,
            progress,
            log,
        }
    }

    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// `Err(Stop::Cancelled)` once a cancel has arrived: between steps, `report.check()?`.
    pub fn check(&self) -> Result<(), Stop> {
        if self.cancelled() {
            Err(Stop::Cancelled)
        } else {
            Ok(())
        }
    }

    /// The flag a cancel sets, for code that waits elsewhere (the runtime's own run).
    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    /// `done` from 0 to 1 over the whole job; `stage` names the step.
    pub fn progress(&mut self, done: f32, stage: &str) {
        (self.progress)(done.clamp(0.0, 1.0), stage);
    }

    /// A line for Cantus's log.
    pub fn log(&mut self, level: &str, message: &str) {
        (self.log)(level, message);
    }
}

/// A job's arguments, parsed.
pub fn args<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Result<T, Stop> {
    serde_json::from_value(v).map_err(|e| Stop::Failed(JobError::BadArgs(e.to_string())))
}

/// A job's result, as the protocol carries it.
pub fn result<T: serde::Serialize>(r: &T) -> Result<serde_json::Value, Stop> {
    serde_json::to_value(r).map_err(|e| Stop::Failed(JobError::Output(e.to_string())))
}

/// The job called `name`.
pub fn find(name: &str) -> Option<JobFn> {
    JOBS.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
}

/// The names the hello lists.
pub fn names() -> Vec<String> {
    JOBS.iter().map(|(n, _)| n.to_string()).collect()
}
