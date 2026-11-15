//! Cedo on the audio path allocates nothing (ADR 0005): every trigger, every band split,
//! every block size, settings changed between blocks (the linear-phase kernel rebuilt at a
//! new crossover, the lookahead moved, the trigger switched), the shape replaced, notes,
//! the transport started, sought and stopped, the meter, rest, `skip` and `reset` — under
//! a counting allocator after `new`.

use cantus_dsp::cedo::band::{Band, Crossover};
use cantus_dsp::cedo::env::{KeyFilterMode, ReleaseShape};
use cantus_dsp::cedo::testkit::shape_fn;
use cantus_dsp::cedo::{Cedo, CedoSettings, Listen, Trigger, CLASSIC_PUMP};
use cantus_dsp::fluctus::CurvePoint;
use cantus_dsp::modfx_clock::Transport;
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
fn cedo_allocates_nothing_after_new() {
    let sc = CedoSettings {
        trigger: Trigger::Sidechain,
        depth_db: 20.0,
        threshold_db: -30.0,
        key_filter: KeyFilterMode::BandPass,
        lookahead_ms: 5.0,
        release_shape: ReleaseShape::SCurve,
        band: Band::Lows,
        crossover: Crossover::LinearPhase,
        link: 0.3,
        ..CedoSettings::default()
    };
    let list = [
        sc,
        CedoSettings { cross_hz: 300.0, ..sc },
        CedoSettings { crossover: Crossover::MinimumPhase, band: Band::Highs, listen: Listen::Delta, ..sc },
        CedoSettings { trigger: Trigger::Tempo, swing: 0.7, edge_ms: 10.0, ..sc },
        CedoSettings { trigger: Trigger::Midi, lookahead_ms: 0.0, listen: Listen::Key, ..sc },
        CedoSettings::default(),
    ];
    let other = [
        CurvePoint { x: 0.0, y: 1.0, tension: 0.3, shape: 1 },
        CurvePoint { x: 0.3, y: 0.2, tension: 0.0, shape: 0 },
        CurvePoint { x: 0.6, y: 0.0, tension: 0.0, shape: 2 },
    ];
    let mut c = Cedo::new(48_000.0, shape_fn);
    c.set_shape(&CLASSIC_PUMP);
    let mut l = vec![0.0f32; 1_024];
    let mut r = vec![0.0f32; 1_024];
    let mut k = vec![0.0f32; 1_024];
    let mut song = 0i64;

    COUNTING.store(true, Ordering::Relaxed);
    for round in 0..2 {
        for (i, s) in list.iter().enumerate() {
            c.set(s);
            if i == 3 {
                c.set_shape(&other);
            }
            for &block in &[1usize, 37, 64, 512, 1_024] {
                for (j, v) in l[..block].iter_mut().enumerate() {
                    *v = ((j * 7 + i) % 13) as f32 * 0.05 - 0.3;
                }
                r[..block].copy_from_slice(&l[..block]);
                for v in k[..block].iter_mut() {
                    *v = 0.6;
                }
                c.set_transport(&Transport {
                    playing: round == 0 || block != 64,
                    tempo_bpm: 128.0,
                    samples: song,
                });
                song += block as i64 + if block == 37 { 10_000 } else { 0 };
                c.note(block / 2);
                c.process(&mut l[..block], &mut r[..block], Some((&k[..block], &k[..block])));
                c.process(&mut l[..block], &mut r[..block], None);
                let _ = c.meter();
                let _ = c.at_rest();
                let _ = c.rest_blocker();
                let _ = c.latency();
                let _ = c.response_db(100.0, 0.5);
            }
            c.skip(4_096);
        }
        c.reset();
        c.set_shape(&CLASSIC_PUMP);
    }
    COUNTING.store(false, Ordering::Relaxed);
    assert_eq!(ALLOCS.load(Ordering::Relaxed), 0);
}
