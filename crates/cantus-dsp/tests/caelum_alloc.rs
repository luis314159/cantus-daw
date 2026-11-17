//! Caelum on the audio path allocates nothing (ADR 0005): every setting it can be given,
//! every block size, freeze, infinite, the line count changed, the shimmer and the bloom
//! switched — processed and set under a counting allocator after `new`.

use cantus_dsp::caelum::{presets, Caelum, CaelumSettings, Lines};
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
fn caelum_allocates_nothing_after_new() {
    let fs = 48_000.0;
    let mut c = Caelum::new(fs);
    let list: Vec<CaelumSettings> = presets::factory().iter().map(|p| p.settings).collect();
    let extra = [
        CaelumSettings {
            freeze: true,
            ..presets::freeze_swell()
        },
        CaelumSettings {
            lines: Lines::Eight,
            bloom_ms: 4_000.0,
            predelay_ms: 1_000.0,
            ..presets::fifth_cathedral()
        },
        CaelumSettings::default(),
    ];
    let mut l = vec![0.0f32; 1024];
    let mut r = vec![0.0f32; 1024];
    let kl = vec![0.1f32; 1024];
    let kr = vec![0.1f32; 1024];
    let mut seed = 1u32;
    COUNTING.store(true, Ordering::SeqCst);
    for (k, s) in list.iter().chain(extra.iter()).enumerate() {
        c.set(s);
        for block in [1usize, 37, 64, 512, 1024] {
            for _ in 0..8 {
                for j in 0..block {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    l[j] = (seed as f32 / u32::MAX as f32) - 0.5;
                    r[j] = l[j] * 0.7;
                }
                let key = (k % 2 == 1).then_some((&kl[..block], &kr[..block]));
                c.process(&mut l[..block], &mut r[..block], key);
            }
        }
        let _ = c.meter();
        let _ = c.at_rest();
        c.skip(4_096);
    }
    c.reset();
    COUNTING.store(false, Ordering::SeqCst);
    assert_eq!(
        ALLOCS.load(Ordering::SeqCst),
        0,
        "Caelum allocated on the audio path"
    );
}
