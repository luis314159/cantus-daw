//! The browser's audition voice on the audio thread (ADR 0030, ADR 0005): starting one,
//! replacing it, letting it finish and stopping it never allocate or free there; every
//! sample it lets go of comes back to the interface to be dropped.

use cantus_core::SampleData;
use cantus_dsp::math;
use cantus_engine::EngineHandle;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;

struct Counting;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static EVENTS: Cell<usize> = const { Cell::new(0) };
}

fn count() {
    let _ = ARMED.try_with(|a| {
        if a.get() {
            let _ = EVENTS.try_with(|n| n.set(n.get() + 1));
        }
    });
}

// SAFETY: every call is forwarded unchanged to the system allocator; the bookkeeping
// touches only const-initialised thread locals, which neither allocate nor panic.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        count();
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn armed<T>(f: impl FnOnce() -> T) -> (T, usize) {
    EVENTS.with(|n| n.set(0));
    ARMED.with(|a| a.set(true));
    let out = f();
    ARMED.with(|a| a.set(false));
    (out, EVENTS.with(|n| n.get()))
}

fn sample(rate: u32, channels: u8, frames: usize) -> Arc<SampleData> {
    let n = frames * channels as usize;
    Arc::new(SampleData {
        rate,
        channels,
        frames: (0..n)
            .map(|i| math::sin((i as f32) * 0.05) * 0.5)
            .collect::<Vec<_>>()
            .into(),
    })
}

#[test]
fn an_audition_starts_replaces_finishes_and_stops_without_touching_the_heap() {
    let (mut ui, mut p) = EngineHandle::manual(48_000.0);
    let mut buf = vec![0.0f32; 2 * 256];
    // Warm up: the first blocks may size things the processor keeps.
    for _ in 0..4 {
        p.process(&mut buf, 2);
    }
    let kick = sample(44_100, 1, 4_000);
    let hat = sample(48_000, 2, 100_000);

    ui.preview(Some((kick.clone(), 1.0)));
    let ((), n) = armed(|| p.process(&mut buf, 2));
    assert_eq!(
        n, 0,
        "starting an audition allocated or freed on the audio thread"
    );
    assert!(buf.iter().any(|s| s.abs() > 0.01), "the audition is heard");

    ui.preview(Some((hat.clone(), 0.5)));
    let ((), n) = armed(|| p.process(&mut buf, 2));
    assert_eq!(n, 0, "replacing an audition touched the heap");

    ui.preview(None);
    let ((), n) = armed(|| {
        for _ in 0..4 {
            p.process(&mut buf, 2);
        }
    });
    assert_eq!(n, 0, "stopping an audition touched the heap");
    p.process(&mut buf, 2);
    assert!(buf.iter().all(|s| *s == 0.0), "silence after the stop");

    // A short sample plays to its end by itself.
    ui.preview(Some((kick.clone(), 1.0)));
    let ((), n) = armed(|| {
        for _ in 0..40 {
            p.process(&mut buf, 2);
        }
    });
    assert_eq!(n, 0);

    // Every sample the voice held came back: only our copies remain once the UI drains.
    ui.poll();
    assert_eq!(
        Arc::strong_count(&kick),
        1,
        "the kick came back to be dropped here"
    );
    assert_eq!(
        Arc::strong_count(&hat),
        1,
        "the hat came back to be dropped here"
    );
}

#[test]
fn an_audition_is_not_in_the_master_meter_or_the_mix() {
    // It is added after the master strip, like the click: the master fader at -inf does
    // not silence it (it is for the person listening), and the meters ignore it.
    let (mut ui, mut p) = EngineHandle::manual(48_000.0);
    let mut buf = vec![0.0f32; 2 * 256];
    ui.preview(Some((sample(48_000, 1, 48_000), 1.0)));
    p.process(&mut buf, 2);
    assert!(buf.iter().any(|s| s.abs() > 0.01));
    ui.poll();
    assert_eq!(ui.meters().master, [0.0, 0.0]);
}
