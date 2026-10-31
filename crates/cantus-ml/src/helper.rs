//! The helper's side: read requests on the main thread, run jobs one at a time on a worker
//! thread, write events from either (one lock around the output, one line per event).

use crate::jobs::{self, Ctx, Report, Stop};
use crate::platform::Pipes;
use crate::proto::{self, Event, JobError, Request};
use crate::runtime::Runtime;
use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Progress is sent at most this often (a stage change and the end always go).
const PROGRESS_EVERY: Duration = Duration::from_millis(100);

/// How long a quit waits for the running job to stop before the process ends anyway.
const QUIT_GRACE: Duration = Duration::from_secs(2);

type Out = Arc<Mutex<Box<dyn Write + Send>>>;

fn send(out: &Out, e: &Event) {
    let mut w = out.lock().unwrap_or_else(|p| p.into_inner());
    // Cantus gone: the input's end will end this process.
    let _ = proto::write_line(&mut *w, e);
}

struct Queued {
    id: u64,
    job: String,
    args: serde_json::Value,
    cancel: Arc<AtomicBool>,
}

/// Serve until the input ends or a quit arrives; the process's exit code.
pub fn serve(pipes: Pipes, test_hooks: bool) -> i32 {
    let out: Out = Arc::new(Mutex::new(Box::new(BufWriter::new(pipes.output))));
    serve_on(Box::new(BufReader::new(pipes.input)), out, test_hooks)
}

pub fn serve_on(mut input: Box<dyn std::io::BufRead + Send>, out: Out, test_hooks: bool) -> i32 {
    send(
        &out,
        &Event::Hello {
            protocol: proto::PROTOCOL,
            version: env!("CARGO_PKG_VERSION").to_string(),
            jobs: jobs::names(),
        },
    );
    let cancels: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>> = Arc::default();
    let (tx, rx) = mpsc::channel::<Queued>();
    let worker = {
        let out = out.clone();
        let cancels = cancels.clone();
        std::thread::Builder::new()
            .name("cantus-ml-job".into())
            .spawn(move || work(rx, out, cancels, test_hooks))
            .expect("the job thread starts")
    };
    let mut line = String::new();
    while let Ok(true) = proto::read_line(&mut input, &mut line) {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Request>(&line) {
            Ok(Request::Start { id, job, args }) => {
                let cancel = Arc::new(AtomicBool::new(false));
                lock(&cancels).insert(id, cancel.clone());
                let _ = tx.send(Queued {
                    id,
                    job,
                    args,
                    cancel,
                });
            }
            Ok(Request::Cancel { id }) => {
                if let Some(c) = lock(&cancels).get(&id) {
                    c.store(true, Ordering::Relaxed);
                }
            }
            Ok(Request::Quit) => break,
            Err(e) => send(
                &out,
                &Event::Log {
                    level: "warn".into(),
                    message: format!("a request was not understood: {e}"),
                },
            ),
        }
    }
    // Stop what runs, give it a moment, and go.
    for c in lock(&cancels).values() {
        c.store(true, Ordering::Relaxed);
    }
    drop(tx);
    let t = Instant::now();
    while !worker.is_finished() && t.elapsed() < QUIT_GRACE {
        std::thread::sleep(Duration::from_millis(10));
    }
    0
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn work(
    rx: mpsc::Receiver<Queued>,
    out: Out,
    cancels: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    test_hooks: bool,
) {
    let mut runtime = Runtime::new();
    for q in rx {
        let id = q.id;
        send(&out, &Event::Started { id });
        let end = if q.cancel.load(Ordering::Relaxed) {
            Event::Cancelled { id }
        } else if let Some(f) = jobs::find(&q.job) {
            let mut last: Option<(Instant, String)> = None;
            let po = out.clone();
            let mut progress = |done: f32, stage: &str| {
                let now = Instant::now();
                let due = match &last {
                    None => true,
                    Some((t, s)) => s != stage || done >= 1.0 || now - *t >= PROGRESS_EVERY,
                };
                if due {
                    last = Some((now, stage.to_string()));
                    send(
                        &po,
                        &Event::Progress {
                            id,
                            done,
                            stage: stage.to_string(),
                        },
                    );
                }
            };
            let lo = out.clone();
            let mut log = |level: &str, message: &str| {
                send(
                    &lo,
                    &Event::Log {
                        level: level.to_string(),
                        message: message.to_string(),
                    },
                )
            };
            let mut ctx = Ctx {
                id,
                runtime: &mut runtime,
                report: Report::new(q.cancel.clone(), &mut progress, &mut log),
                test_hooks,
            };
            // A job that panics fails, said at once, instead of a helper silent until the
            // watchdog (review 1 #2).
            let run =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(q.args, &mut ctx)));
            match run {
                Ok(Ok(result)) => Event::Done { id, result },
                Ok(Err(Stop::Cancelled)) => Event::Cancelled { id },
                Ok(Err(Stop::Failed(error))) => Event::Failed { id, error },
                Err(p) => {
                    let what = p
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| p.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "no message".into());
                    Event::Failed {
                        id,
                        error: JobError::Model(format!("the job panicked: {what}")),
                    }
                }
            }
        } else {
            Event::Failed {
                id,
                error: JobError::UnknownJob(q.job.clone()),
            }
        };
        lock(&cancels).remove(&id);
        send(&out, &end);
    }
}
