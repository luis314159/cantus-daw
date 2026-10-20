//! The helper's processing threads (ADR 0063): a small pool that serves every instance in
//! the process, through the lobby Cantus made for it (`cantus_rt::ipc::serve`).
//!
//! **Waiting.** A thread that finished a job keeps scanning the instances' exchanges for
//! [`SPIN_AFTER`] — the next job of the same block (another plug-in of this helper, an
//! instrument's next note segment) is taken a cache-line hand-off later — then parks on
//! the lobby's bell, which Cantus rings only when no thread scans. One short spin per burst
//! of jobs, not one per plug-in: the threads are promoted as *bounded* waiters
//! (`cantus_rt::thread::Waiting::Bounded`), never spinning through a period inside the
//! real-time class (ADR 0031 R15).
//!
//! **Bits.** Each job says whether Cantus's thread flushed denormals; the call runs under
//! the same setting, so the plug-in's output is the bits it would have made in Cantus.
//!
//! **No allocation** in a job: each thread's parameter and note lists are refilled in place
//! and the plug-in's report goes into a `ParamOut` made once. The instance table is a
//! snapshot each thread refreshes when the main thread changes it (a load, an unload); a
//! snapshot replaced is kept by the main thread until no thread holds it, and freed there —
//! a processing thread never frees memory nor unmaps a slot (review #13, #42).

use cantus_host::bridge::slot::{op, status, Slot, MAX_NOTES, MAX_PARAMS};
use cantus_host::{Audio, AudioThread, ParamOut};
use cantus_rt::ipc::{Completer, LobbyBell};
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a thread scans after its last job before it parks.
pub const SPIN_AFTER: Duration = Duration::from_micros(50);

/// How long it parks before it looks whether it should stop.
const PARK: Duration = Duration::from_millis(100);

/// The plug-in's audio half, shared with the main thread that installs and removes it while
/// no job runs (load, restart, unload).
pub type Half = Arc<Mutex<Option<Box<dyn AudioThread>>>>;

/// One instance as the processing threads see it.
pub struct Rt {
    /// Cantus's number for the instance (what a crash in it is laid on).
    pub instance: u32,
    pub slot: Arc<Slot>,
    pub completer: Completer,
    pub half: Half,
}

/// One version of the table.
type Snapshot = Arc<Vec<Arc<Rt>>>;

/// The instances, as the threads read them: a snapshot swapped whole by the main thread.
#[derive(Default)]
pub struct Table {
    now: Mutex<Snapshot>,
    generation: AtomicU64,
    /// Snapshots replaced, kept until no processing thread holds one ([`Table::collect`]).
    retired: Mutex<Vec<Snapshot>>,
}

impl Table {
    /// The main thread's change: `f` edits a copy, which becomes the table; the old one is
    /// retired, not dropped.
    pub fn change(&self, f: impl FnOnce(&mut Vec<Arc<Rt>>)) {
        let mut guard = self.now.lock().unwrap_or_else(|e| e.into_inner());
        let mut next: Vec<Arc<Rt>> = (**guard).clone();
        f(&mut next);
        let old = std::mem::replace(&mut *guard, Arc::new(next));
        self.generation.fetch_add(1, Ordering::Release);
        drop(guard);
        self.retired
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(old);
    }

    /// The main thread frees the retired snapshots no processing thread holds any more
    /// (none can take one again: only the current is handed out). The instances they held
    /// are freed with them, on this thread.
    pub fn collect(&self) {
        self.retired
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|s| Arc::strong_count(s) > 1);
    }

    /// Snapshots retired and not yet freed (tests).
    #[cfg(test)]
    fn retired(&self) -> usize {
        self.retired.lock().map_or(0, |r| r.len())
    }

    fn snapshot(&self) -> Option<Arc<Vec<Arc<Rt>>>> {
        self.now.try_lock().ok().map(|g| g.clone())
    }
}

/// One processing thread: serves the table's instances until `stop`.
pub fn serve(table: Arc<Table>, bell: Arc<LobbyBell>, stop: Arc<AtomicBool>) {
    let _promotion = cantus_rt::thread::promote_worker(
        cantus_rt::thread::WorkerRole::Live,
        Duration::from_millis(5),
        cantus_rt::thread::Waiting::Bounded { busy_share: 0.5 },
    );
    cantus_rt::memory::prefault_stack();
    crate::platform::blame::alt_stack();
    let mut params = Vec::with_capacity(MAX_PARAMS);
    let mut notes = Vec::with_capacity(MAX_NOTES);
    let mut out = Box::new(ParamOut::new());
    let seen = RefCell::new((u64::MAX, Arc::new(Vec::<Arc<Rt>>::new())));
    let refresh = || {
        let g = table.generation.load(Ordering::Acquire);
        let stale = seen.borrow().0 != g;
        if stale {
            if let Some(s) = table.snapshot() {
                *seen.borrow_mut() = (g, s);
            }
        }
    };
    cantus_rt::ipc::serve(
        &bell,
        SPIN_AFTER,
        Some(PARK),
        &stop,
        || {
            refresh();
            let s = seen.borrow();
            s.1.iter()
                .enumerate()
                .find_map(|(i, rt)| rt.completer.exchange().take().map(|n| (i, n)))
        },
        || {
            seen.borrow()
                .1
                .iter()
                .any(|rt| rt.completer.exchange().pending())
        },
        |i, n| {
            // Borrowed, never cloned: this thread's reference is never the last.
            let s = seen.borrow();
            run(&s.1[i], n, &mut params, &mut notes, &mut out);
        },
    );
}

/// One job of one instance, taken by this thread: the plug-in's call, its report, done.
fn run(
    rt: &Rt,
    n: u32,
    params: &mut Vec<cantus_host::ParamChange>,
    notes: &mut Vec<cantus_host::NoteEvent>,
    out: &mut ParamOut,
) {
    let started = Instant::now();
    // SAFETY: job `n` was taken by this thread and is not completed: the area is ours.
    let job = unsafe { rt.slot.job(params, notes) };
    let frames = job.left.len() as u64;
    out.clear();
    out.lost_in = 0;
    crate::platform::blame::set_current(rt.instance);
    let st = match rt.half.try_lock() {
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
                        params,
                        notes,
                        out,
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
    crate::platform::blame::set_current(0);
    // SAFETY: still job `n`'s area, not yet completed.
    unsafe { rt.slot.put_result(out, st) };
    rt.slot.account(started.elapsed().as_nanos() as u64, frames);
    rt.completer.complete(n);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The main thread's side of retirement (review #13, #42): a replaced snapshot is kept
    /// while a holder (a processing thread's `seen`) has it, and freed by `collect` — on the
    /// main thread — once nothing else does. What it does not hold by itself: that a
    /// processing thread only ever lets go of a borrowed or cloned snapshot, never the last
    /// one — which follows from `change` keeping every old snapshot in `retired` until
    /// `collect` sees it unheld (review isolation-3 #25).
    #[test]
    fn a_retired_snapshot_is_freed_by_the_main_thread_once_no_thread_holds_it() {
        let table = Table::default();
        table.change(|_| {});
        let held = table.snapshot().expect("the current snapshot");
        table.change(|_| {});
        table.collect();
        assert_eq!(
            table.retired(),
            1,
            "the held one is kept, the unheld one freed"
        );
        drop(held);
        assert_eq!(table.retired(), 1, "a holder letting go frees nothing");
        table.collect();
        assert_eq!(table.retired(), 0, "the main thread frees it");
    }
}
