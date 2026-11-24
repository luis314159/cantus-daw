//! What Arcus adds to a checkpoint's image (round 42, ADR 0157): the bytes a fork of the
//! unit holds — its ten voices (each a string's two lines sized for 25 Hz and its torsion's,
//! so in proportion to the rate), its lanes and Fides's body — counted by the allocator,
//! against the figure `Processor::approx_bytes` takes for `InstrumentKind::BowedString` at
//! 48 kHz (`listen_fork.rs`), as `fides_image.rs` holds Fides's.

use cantus_core::{InstrumentKind, NativeSpec};
use cantus_engine::inst::{make, NoteOn};
use std::sync::atomic::{AtomicIsize, Ordering};

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
        LIVE.fetch_add(l.size() as isize, Ordering::SeqCst);
        unsafe { std::alloc::System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: std::alloc::Layout) -> *mut u8 {
        LIVE.fetch_add(l.size() as isize, Ordering::SeqCst);
        unsafe { std::alloc::System.alloc_zeroed(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
        LIVE.fetch_sub(l.size() as isize, Ordering::SeqCst);
        unsafe { std::alloc::System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: std::alloc::Layout, new: usize) -> *mut u8 {
        LIVE.fetch_add(new as isize - l.size() as isize, Ordering::SeqCst);
        unsafe { std::alloc::System.realloc(p, l, new) }
    }
}
#[global_allocator]
static GLOBAL: Counting = Counting;

/// `approx_bytes`'s figure for Arcus (KB at 48 kHz).
const ARCUS_KB: f64 = 425.0;

/// The bytes a fork of Arcus's unit holds, playing four notes in poly, at `sr`.
fn fork_bytes(sr: f32) -> f64 {
    let kind = InstrumentKind::BowedString;
    let mut u = make(kind, sr);
    let mut spec = NativeSpec::new(kind);
    spec.set("mode", 0.0);
    u.configure(&spec, &[]);
    for (i, key) in [43u8, 50, 57, 62].iter().enumerate() {
        u.note_on(NoteOn {
            key: *key,
            velocity: 0.8,
            id: i as u64 + 1,
            channel: 1,
        });
    }
    let (mut l, mut r) = (vec![0f32; 512], vec![0f32; 512]);
    for _ in 0..20 {
        u.process(&mut l, &mut r);
    }
    let before = LIVE.load(Ordering::SeqCst);
    let copy = u.fork().expect("a copy");
    let bytes = LIVE.load(Ordering::SeqCst) - before;
    drop(copy);
    bytes as f64
}

#[test]
fn arcus_holds_what_the_checkpoints_budget_says() {
    let kb = |b: f64| b / 1024.0;
    let at48 = fork_bytes(48_000.0);
    let at44 = fork_bytes(44_100.0);
    let at96 = fork_bytes(96_000.0);
    println!(
        "Arcus: {:.0} KB at 44.1 kHz, {:.0} at 48, {:.0} at 96",
        kb(at44),
        kb(at48),
        kb(at96)
    );
    assert!(
        (kb(at48) / ARCUS_KB - 1.0).abs() < 0.1,
        "Arcus holds {:.0} KB at 48 kHz; approx_bytes says {ARCUS_KB}",
        kb(at48)
    );
    // Its lines grow with the rate; `approx_bytes` scales every unit by the rate's ratio.
    assert!(at96 > 1.3 * at48 && at96 <= 2.0 * at48, "{at96} vs {at48}");
    assert!(at44 < at48);
}
