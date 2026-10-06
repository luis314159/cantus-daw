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
