//! Speculum through the whole engine (ADR 0056): a pure pass-through whatever it is set
//! to; and its data path — every frame of a watched Speculum's own signal reaches its scope
//! ring, with the song's position, several at once, nothing lost and nothing allocated on
//! the audio thread.

#[path = "support/modfx.rs"]
mod modfx;
mod support;
use cantus_core::*;
use cantus_dsp::nulltest::assert_null;
use cantus_engine::fx::speculum::{ScopeChunk, MAX_SCOPES};
use cantus_engine::{EngineHandle, Processor};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use support::*;

// ---- allocations on the thread under measurement ---------------------------------------

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ON.try_with(|on| {
            if on.get() {
                let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn speculum_set_any_way_renders_exactly_as_no_speculum() {
    let reference = render_of(&demo_project());
    for at_max in [false, true] {
        let mut p = demo_project();
        let fx = add_effect(&mut p, EffectKind::Analyzer);
        if at_max {
            let s = strip_of(&mut p, 0);
            let slot = s.effects.iter_mut().find(|e| e.id == fx).unwrap();
            for info in EffectKind::Analyzer.params() {
                slot.spec.set(info.key, info.max);
            }
        }
        assert_null(&reference, &render_of(&p), 0.0, &format!("at max {at_max}"));
    }
}

/// A project with a Speculum on the master chain and one on the lead's strip.
fn watched() -> (Project, EffectId, EffectId) {
    let mut p = demo_project();
    let strip = add_effect(&mut p, EffectKind::Analyzer);
    let slot = p.make_effect(EffectKind::Analyzer);
    let master = slot.id;
    p.master.effects.push(slot);
    (p, master, strip)
}

fn engine(p: &Project) -> (EngineHandle, Processor) {
    let (mut h, mut proc) = EngineHandle::manual(48_000.0);
    let s = h.compile(p, 1);
    h.set_snapshot(s);
    h.play();
    // Let the processor take the snapshot and the units.
    let mut buf = vec![0f32; 2 * 64];
    proc.process(&mut buf, 2);
    (h, proc)
}

/// Every frame the master's Speculum saw comes out of its ring, in order, once: it is
/// what the master played (the master fader at 0 dB and centred: the same samples, but for
/// the centre's 0.99999994).
#[test]
fn every_frame_of_a_watched_speculum_reaches_its_ring() {
    let (p, master, strip) = watched();
    let (mut h, mut proc) = engine(&p);
    assert!(h.watch_scope(master));
    assert!(h.watch_scope(strip), "two at once");
    let mut played = Vec::new();
    let mut seen: Vec<f32> = Vec::new();
    let mut strip_frames = 0usize;
    let mut beats: Vec<(f64, usize)> = Vec::new();
    let sizes = [256usize, 333, 1024, 96, 512];
    for k in 0..60 {
        // The watch message is in the queue before this block: it applies from its start.
        let n = sizes[k % sizes.len()];
        let mut buf = vec![0f32; 2 * n];
        proc.process(&mut buf, 2);
        if k > 0 {
            played.extend_from_slice(&buf);
        }
        h.drain_scope(master, |c: &ScopeChunk| {
            if k > 0 {
                for i in 0..c.frames as usize {
                    seen.push(c.l[i]);
                    seen.push(c.r[i]);
                }
                if let Some(b) = c.beat {
                    beats.push((b, c.frames as usize));
                }
            }
        });
        h.drain_scope(strip, |c| strip_frames += c.frames as usize);
        // Every frame keeps both watched.
        h.watch_scope(master);
        h.watch_scope(strip);
        h.release_unwatched_scopes();
    }
    assert_eq!(seen.len(), played.len(), "every frame, once");
    assert!(played.iter().any(|v| v.abs() > 0.01), "the demo plays");
    for (a, b) in seen.iter().zip(&played) {
        assert!((a * 0.999_999_94 - b).abs() <= 1e-6, "{a} vs {b}");
    }
    assert!(strip_frames > 0, "the strip's own ring filled too");
    // The song's position runs on with the frames: 128 BPM at 48 kHz.
    let per_frame = 128.0 / 60.0 / 48_000.0;
    for w in beats.windows(2) {
        let (b0, n0) = w[0];
        let (b1, _) = w[1];
        assert!((b1 - b0 - n0 as f64 * per_frame).abs() < 1e-9, "{b0} + {n0} → {b1}");
    }
}

/// What nobody watches is let go: its ring receives nothing more, and a ninth Speculum is
/// refused while eight are watched.
#[test]
fn an_unwatched_speculum_is_let_go_and_eight_is_the_most() {
    let (p, master, strip) = watched();
    let (mut h, mut proc) = engine(&p);
    assert!(h.watch_scope(master));
    h.release_unwatched_scopes();
    // Not asked for this frame: released.
    h.release_unwatched_scopes();
    let mut buf = vec![0f32; 2 * 512];
    proc.process(&mut buf, 2);
    let mut got = 0;
    h.drain_scope(master, |_| got += 1);
    assert_eq!(got, 0);
    for i in 0..MAX_SCOPES as u64 {
        assert!(h.watch_scope(EffectId(1_000 + i)));
    }
    assert!(!h.watch_scope(strip), "a ninth");
}

/// Copying a watched Speculum's signal allocates nothing on the audio thread.
#[test]
fn the_copy_allocates_nothing() {
    let (p, master, strip) = watched();
    let (mut h, mut proc) = engine(&p);
    h.watch_scope(master);
    h.watch_scope(strip);
    let mut buf = vec![0f32; 2 * 512];
    // Warm: the messages land, the first blocks run.
    for _ in 0..8 {
        proc.process(&mut buf, 2);
        h.drain_scope(master, |_| {});
        h.drain_scope(strip, |_| {});
    }
    COUNT.with(|c| c.set(0));
    ON.with(|on| on.set(true));
    for _ in 0..20 {
        proc.process(&mut buf, 2);
    }
    ON.with(|on| on.set(false));
    assert_eq!(COUNT.with(Cell::get), 0, "allocations on the audio thread");
}
