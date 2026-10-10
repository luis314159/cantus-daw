//! What the audio thread will call — `promote_callback` (first block and every later one),
//! the denormal guard, `reassert` — allocates nothing (ADR 0005 rule 1; the engine's
//! `rt_memory.rs` checks the same with its own counting allocator once graph wires them).

#![cfg(not(miri))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static WATCHED: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: forwards to the system allocator unchanged; only counts calls on a watched thread.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCHED.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: the caller's contract is the system allocator's.
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if WATCHED.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
        // SAFETY: as above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn the_audio_thread_calls_allocate_nothing() {
    std::thread::spawn(|| {
        let period = Duration::from_micros(2_667);
        // On Linux the first promotion may go through rtkit, whose D-Bus message allocates —
        // once per stream start. Everywhere else it is counted too.
        WATCHED.with(|w| w.set(!cfg!(target_os = "linux")));
        let first = cantus_rt::promote_callback(cantus_rt::AudioHost::Wasapi, period);
        WATCHED.with(|w| w.set(true));
        for _ in 0..1000 {
            let g = cantus_rt::DenormalGuard::new();
            cantus_rt::fpenv::reassert();
            g.reassert();
            let again = cantus_rt::promote_callback(cantus_rt::AudioHost::Wasapi, period);
            assert!(again == first);
            drop(g);
        }
        WATCHED.with(|w| w.set(false));
        assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0, "{first}");
    })
    .join()
    .unwrap();
}

/// Forking voices (ADR 0038): publishing, helping and joining allocate nothing, on the
/// forker or on a helper.
#[test]
fn a_fork_allocates_nothing_on_either_side() {
    use cantus_rt::fork::{Forks, Job};
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    let forks = Arc::new(Forks::new());
    let stop = Arc::new(AtomicBool::new(false));
    let helper = {
        let (f, s) = (forks.clone(), stop.clone());
        std::thread::spawn(move || {
            WATCHED.with(|w| w.set(true));
            f.presence(true);
            while !s.load(Ordering::Relaxed) {
                f.help();
            }
            f.presence(false);
            WATCHED.with(|w| w.set(false));
        })
    };
    forks.presence(true);
    while forks.present() < 2 {
        std::thread::yield_now();
    }
    WATCHED.with(|w| w.set(true));
    let mut sums = [0u64; 16];
    for _ in 0..1_000 {
        let mut work: [_; 16] = std::array::from_fn(|_| None);
        for (w, s) in work.iter_mut().zip(sums.iter_mut()) {
            // Long enough that the helper wins some of them (asserted below).
            *w = Some(move || {
                for _ in 0..300 {
                    std::hint::spin_loop();
                }
                *s += 1
            });
        }
        let mut jobs: [Job<'_>; 16] = [const { None }; 16];
        for (j, w) in jobs.iter_mut().zip(work.iter_mut()) {
            *j = w.as_mut().map(|w| w as &mut (dyn FnMut() + Send));
        }
        forks.run(&mut jobs);
    }
    WATCHED.with(|w| w.set(false));
    forks.presence(false);
    stop.store(true, Ordering::Relaxed);
    helper.join().unwrap();
    assert!(sums.iter().all(|s| *s == 1_000));
    // Both sides were exercised: the helper ran jobs too, not only the forker.
    assert!(forks.counts().0 > 0, "the helper never ran a job");
    assert_eq!(ALLOCATIONS.load(Ordering::Relaxed), 0);
}
