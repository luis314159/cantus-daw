//! What Fides adds to a checkpoint's image (round 41's integrated review, finding 5): the
//! bytes a fork of the unit holds — its 36 voices (each two waveguides' lines sized for
//! 20 Hz, so in proportion to the rate, and two banks of 48 modes), its lanes and its body —
//! counted by the allocator, against the figure `Processor::approx_bytes` takes for
//! `InstrumentKind::Resonator` at 48 kHz.
//!
//! The other units' figures came from the working-set probe
//! (`listen::tests::what_each_unit_adds_to_an_image`, ignored), which a loaded machine cannot
//! read (round 41: its deltas went negative, a freed image's pages reused). Counted here
//! beside their figures (2026-11-21, KB at 48 kHz): Wavetable 1 088 (figure 1 650), Analog
//! 129 (600), Fm 182 (500), Sampler 3 612 (4 300), Bass 111 (400), Electric piano 37 (650),
//! Organ 90 (550), Chorda 5 621 (5 150) — the working set runs 300–700 KB over a small
//! unit's own bytes (its share of the image's pages) and level with a large one's. Fides,
//! 1 347 counted, takes 1 400; Tympanum, 7 171 counted against its round-40 stub's 2,
//! takes 7 200.

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

/// `approx_bytes`'s figure for Fides (KB at 48 kHz).
const FIDES_KB: f64 = 1_400.0;
/// And for Tympanum.
const TYMPANUM_KB: f64 = 7_200.0;

/// The counter is the process's: one test counts at a time.
fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

/// The bytes a fork of `kind`'s unit holds, playing four notes, at `sr`.
fn fork_bytes(kind: InstrumentKind, sr: f32) -> f64 {
    let mut u = make(kind, sr);
    u.configure(&NativeSpec::new(kind), &[]);
    for (i, key) in [48u8, 55, 60, 64].iter().enumerate() {
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
fn fides_holds_what_the_checkpoints_budget_says() {
    let _turn = turn();
    let kb = |b: f64| b / 1024.0;
    let at48 = fork_bytes(InstrumentKind::Resonator, 48_000.0);
    let at44 = fork_bytes(InstrumentKind::Resonator, 44_100.0);
    let at96 = fork_bytes(InstrumentKind::Resonator, 96_000.0);
    for k in InstrumentKind::ALL {
        println!("{k:?}: {:.0} KB at 48 kHz", kb(fork_bytes(k, 48_000.0)));
    }
    println!(
        "Fides: {:.0} KB at 44.1 kHz, {:.0} at 48, {:.0} at 96",
        kb(at44),
        kb(at48),
        kb(at96)
    );
    assert!(
        (kb(at48) / FIDES_KB - 1.0).abs() < 0.1,
        "Fides holds {:.0} KB at 48 kHz; approx_bytes says {FIDES_KB}",
        kb(at48)
    );
    // Its lines grow with the rate; `approx_bytes` scales every unit by the rate's ratio,
    // which holds Fides from above (its fixed part does not grow).
    assert!(at96 > 1.5 * at48 && at96 <= 2.0 * at48, "{at96} vs {at48}");
    assert!(at44 < at48);
}

#[test]
fn tympanum_holds_what_the_checkpoints_budget_says() {
    let _turn = turn();
    let kb = fork_bytes(InstrumentKind::AcousticDrums, 48_000.0) / 1024.0;
    println!("Tympanum: {kb:.0} KB at 48 kHz");
    assert!(
        (kb / TYMPANUM_KB - 1.0).abs() < 0.1,
        "Tympanum holds {kb:.0} KB at 48 kHz; approx_bytes says {TYMPANUM_KB}"
    );
}
