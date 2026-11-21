//! Nitor on the audio path allocates nothing (ADR 0005): every character, every part, the
//! level match, settings changed between blocks (the amount, the character, the tonic, the
//! crossover, the sub's mode), blocks of every size, the dynamics parked and woken, `skip`,
//! `snap` and `reset` — under a counting allocator after `new`.

use cantus_dsp::nitor::{Character, Nitor, NitorSettings, SubMode};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Counting;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn nitor_allocates_nothing_after_new() {
    let sr = 48_000.0;
    let heavy = NitorSettings {
        amount: 1.0,
        character: Character::Loud,
        sub: 0.6,
        sub_mode: SubMode::Tonic,
        harm: 0.5,
        tight: 0.6,
        lift: 0.7,
        xover_hz: 120.0,
        tonic: 7,
        excite: [0.5, 0.6, 0.7],
        color: 0.3,
        ..NitorSettings::default()
    };
    let mut list = vec![NitorSettings::default(), heavy];
    for c in Character::ALL {
        list.push(NitorSettings {
            amount: 0.6,
            character: c,
            sub_mode: SubMode::Octave,
            sub: 0.4,
            xover_hz: 70.0,
            tonic: c.index() as u8 * 2,
            mix: 0.7,
            level_match: c.index() % 2 == 0,
            ..NitorSettings::default()
        });
    }
    let mut u = Nitor::new(sr);
    // Signals and buffers made before counting.
    let n = 4_096;
    let tone: Vec<f32> = (0..n)
        .map(|i| 0.3 * cantus_dsp::math::sin(i as f32 * 0.02) + 0.1 * cantus_dsp::math::sin(i as f32 * 0.31))
        .collect();
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let silence = vec![0.0f32; n];
    COUNTING.store(true, Ordering::Relaxed);
    for (k, s) in list.iter().enumerate() {
        u.set(s);
        for (b, len) in [1usize, 17, 64, 333, 1024, 4096].iter().enumerate() {
            let src = if (k + b) % 4 == 3 { &silence } else { &tone };
            l[..*len].copy_from_slice(&src[..*len]);
            r[..*len].copy_from_slice(&src[..*len]);
            u.process(&mut l[..*len], &mut r[..*len]);
        }
        let _ = u.meter();
        let _ = u.at_rest();
        let _ = u.rest_blocker();
    }
    // A long silence parks the dynamics; sound wakes them.
    u.set(&heavy);
    for _ in 0..30 {
        l.copy_from_slice(&silence);
        r.copy_from_slice(&silence);
        u.process(&mut l, &mut r);
    }
    l.copy_from_slice(&tone);
    r.copy_from_slice(&tone);
    u.process(&mut l, &mut r);
    u.snap();
    u.skip(1_000);
    u.reset();
    u.process(&mut l, &mut r);
    COUNTING.store(false, Ordering::Relaxed);
    assert_eq!(ALLOCS.load(Ordering::Relaxed), 0);
}
