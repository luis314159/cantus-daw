//! One instance's processing thread in the helper (ADR 0063): it waits for Cantus's jobs on
//! the instance's exchange, runs the plug-in's audio half on each, and reports.
//!
//! **Waiting.** After each job it spins for [`SPIN_AFTER`] — the next job of the same block
//! (an instrument's next note segment) usually comes within it, a cache-line hand-off
//! away — then parks on its bell, which Cantus rings when it finds it parked. The thread is
//! promoted as a *bounded* waiter (`cantus_rt::thread::Waiting::Bounded`): it is busy only
//! for its jobs and that short spin, never through a period, so it may run in the real-time
//! class without being throttled, and it never spins inside it unboundedly.
//!
//! **Bits.** Each job says whether Cantus's thread flushed denormals; the call runs under
//! the same setting (a `DenormalGuard` for the call), so the plug-in's output is the bits it
//! would have made in Cantus's process.
//!
//! **No allocation** after start: the job's parameter and note lists are refilled in place
//! and the plug-in's report goes into a `ParamOut` made once.

use cantus_host::bridge::slot::{op, status, Slot, MAX_NOTES, MAX_PARAMS};
use cantus_host::{Audio, AudioThread, ParamOut};
use cantus_rt::ipc::{Next, Worker};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long the thread spins for the next job before it parks.
pub const SPIN_AFTER: Duration = Duration::from_micros(300);

/// How long it parks before it looks whether it should stop.
const PARK: Duration = Duration::from_millis(100);

/// The plug-in's audio half, shared with the main thread that installs and removes it while
/// no job runs (load, restart, unload).
pub type Half = Arc<Mutex<Option<Box<dyn AudioThread>>>>;

/// Serves jobs until `stop` is set. `ready` is told once the thread is promoted and its
/// buffers made, so the instance's first block is not spent starting up.
pub fn serve(
    slot: Arc<Slot>,
    worker: Worker,
    half: Half,
    stop: Arc<AtomicBool>,
    ready: std::sync::mpsc::Sender<()>,
) {
    let _promotion = cantus_rt::thread::promote_worker(
        cantus_rt::thread::WorkerRole::Live,
        Duration::from_millis(5),
        cantus_rt::thread::Waiting::Bounded { busy_share: 0.5 },
    );
    cantus_rt::memory::prefault_stack();
    let mut params = Vec::with_capacity(MAX_PARAMS);
    let mut notes = Vec::with_capacity(MAX_NOTES);
    let mut out = Box::new(ParamOut::new());
    let _ = ready.send(());
    drop(ready);
    // SAFETY: the exchange is at the start of `slot`, which this thread holds.
    let mut last = unsafe { &*slot.exchange() }.numbers().0;
    while !stop.load(Ordering::Acquire) {
        let n = match worker.next(last, SPIN_AFTER, Some(PARK)) {
            Next::Idle => continue,
            Next::Job(n) => n,
        };
        let started = Instant::now();
        // SAFETY: job `n` was published and is not completed: the job area is ours.
        let job = unsafe { slot.job(&mut params, &mut notes) };
        let frames = job.left.len() as u64;
        out.clear();
        out.lost_in = 0;
        let st = match half.try_lock() {
            Ok(mut guard) => match guard.as_mut() {
                Some(audio) => {
                    let _flush = job.flush.then(cantus_rt::DenormalGuard::new);
                    match job.op {
                        op::PROCESS => audio.process(
                            Audio {
                                left: job.left,
                                right: job.right,
                                key: job.key,
                                transport: job.transport.as_ref(),
                            },
                            &params,
                            &notes,
                            &mut out,
                        ),
                        op::RESET => audio.reset(),
                        op::STOP => audio.stop(),
                        _ => {}
                    }
                    status::OK
                }
                None => status::NO_AUDIO,
            },
            Err(_) => status::NO_AUDIO,
        };
        // SAFETY: still job `n`'s area, not yet completed.
        unsafe { slot.put_result(&out, st) };
        slot.account(started.elapsed().as_nanos() as u64, frames);
        worker.complete(n);
        last = n;
    }
}
