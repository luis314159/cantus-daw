//! Purus on the audio path allocates nothing (ADR 0005): every module, every mode, every
//! block size, settings changed between blocks (the de-reverb switched on and off, Auto
//! switching mains, the gentle mode), the meter, rest, `skip` and `reset` — under a counting
//! allocator after `new`.

use cantus_dsp::purus::{DecayMode, HumMode, HumSettings, Purus, PurusSettings, VerbSettings};
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
fn purus_allocates_nothing_after_new() {
    let hum = HumSettings {
        on: true,
        ..HumSettings::default()
    };
    let verb = VerbSettings {
        on: true,
        ..VerbSettings::default()
    };
    let list = [
        PurusSettings::default(),
        PurusSettings {
            hum,
            ..PurusSettings::default()
        },
        PurusSettings {
            hum: HumSettings {
                gentle: true,
                mode: HumMode::Auto,
                harmonics: 16,
                delta: true,
                ..hum
            },
            verb,
            out_db: -3.0,
        },
        PurusSettings {
            hum: HumSettings {
                mode: HumMode::Track,
                mix: 0.5,
                ..hum
            },
            verb: VerbSettings {
                decay: DecayMode::Estimate,
                delta: true,
                keep_ms: 200.0,
                ..verb
            },
            out_db: 0.0,
        },
        PurusSettings {
            verb: VerbSettings {
                amount_db: 30.0,
                smoothing: 1.0,
                mix: 0.3,
                ..verb
            },
            ..PurusSettings::default()
        },
    ];
    let mut p = Purus::new(48_000.0);
    let mut l = vec![0.0f32; 1024];
    let mut r = vec![0.0f32; 1024];
    let mut seed = 1u32;
    let mut t = 0usize;
    COUNTING.store(true, Ordering::SeqCst);
    for round in 0..2 {
        for s in &list {
            p.set(s);
            for block in [1usize, 37, 64, 512, 1024] {
                for _ in 0..8 {
                    for j in 0..block {
                        seed ^= seed << 13;
                        seed ^= seed >> 17;
                        seed ^= seed << 5;
                        let hum = 0.05 * cantus_dsp::math::sin((t as f32) * 0.006_545);
                        l[j] = 0.2 * ((seed as f32 / u32::MAX as f32) - 0.5) + hum;
                        r[j] = l[j] * 0.7;
                        t += 1;
                    }
                    p.process(&mut l[..block], &mut r[..block]);
                }
            }
            let _ = p.meter();
            let _ = p.at_rest();
            let _ = p.latency();
            // Silence long enough to come to rest, then skipped.
            l.fill(0.0);
            r.fill(0.0);
            for _ in 0..(6 * 48) {
                p.process(&mut l, &mut r);
            }
            p.skip(4_096);
            if round == 1 {
                p.reset();
            }
        }
    }
    COUNTING.store(false, Ordering::SeqCst);
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        0,
        "allocations on the audio path"
    );
}
