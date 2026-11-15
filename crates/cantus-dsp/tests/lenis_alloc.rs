//! Lenis on the audio path allocates nothing (ADR 0005): every quality, channel mode and
//! curve, the focus bands, the delta, settings changed mid-stream (a new frame size among
//! them), every block size, the meter, `at_rest`, `skip`, `reset` — under a counting
//! allocator after `new`.

use cantus_dsp::lenis::{Channels, FocusBand, Lenis, LenisSettings, Mode, Quality};
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
fn lenis_allocates_nothing_after_new() {
    for sr in [44_100.0f32, 48_000.0, 96_000.0, 192_000.0] {
        let mut u = Lenis::new(sr);
        let mut list = vec![];
        for q in [Quality::Fast, Quality::Standard, Quality::Precise] {
            for ch in [Channels::Linked, Channels::Independent, Channels::MidSide] {
                for mode in [Mode::Soft, Mode::Hard] {
                    let mut s = LenisSettings {
                        depth: 0.8,
                        selectivity: 0.2,
                        quality: q,
                        channels: ch,
                        mode,
                        delta: mode == Mode::Hard,
                        mix: 0.7,
                        out_db: -2.0,
                        ..LenisSettings::default()
                    };
                    s.bands[2] = FocusBand {
                        on: true,
                        hz: 4_000.0,
                        amount: 1.0,
                        width_oct: 2.0,
                    };
                    list.push(s);
                }
            }
        }
        list.push(LenisSettings::default());
        let mut l = vec![0.0f32; 1024];
        let mut r = vec![0.0f32; 1024];
        let mut seed = 1u32;
        COUNTING.store(true, Ordering::SeqCst);
        for (i, s) in list.iter().enumerate() {
            u.set(s);
            for block in [1usize, 37, 64, 512, 1024] {
                for _ in 0..6 {
                    for (a, b) in l[..block].iter_mut().zip(r[..block].iter_mut()) {
                        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                        *a = (seed >> 9) as f32 / (1u32 << 23) as f32 - 1.0;
                        *b = 0.5 * *a;
                    }
                    u.process(&mut l[..block], &mut r[..block]);
                }
            }
            let _ = u.meter().max_reduction_db;
            let _ = u.reduction_bins(1).len();
            if i % 3 == 0 {
                u.reset();
            }
            // Silence until at rest, then a skip.
            l.fill(0.0);
            r.fill(0.0);
            for _ in 0..400 {
                if u.at_rest() {
                    u.skip(4_096);
                    break;
                }
                u.process(&mut l, &mut r);
            }
        }
        COUNTING.store(false, Ordering::SeqCst);
        assert_eq!(ALLOCS.load(Ordering::SeqCst), 0, "{sr} Hz");
    }
}
