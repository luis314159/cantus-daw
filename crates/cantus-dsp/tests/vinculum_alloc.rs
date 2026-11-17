//! Vinculum allocates nothing once built: every model, every switch, set and processed
//! while a counting allocator watches (the audio thread's rule, ADR 0005).

use cantus_dsp::vinculum::{Character, Model, Vinculum, VinculumSettings};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Counting;

static WATCH: AtomicBool = AtomicBool::new(false);
static COUNT: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCH.load(Ordering::Relaxed) {
            COUNT.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn nothing_is_allocated_after_construction() {
    let mut v = Vinculum::new(48_000.0);
    let mut l = vec![0f32; 512];
    let mut r = vec![0f32; 512];
    let settings: Vec<VinculumSettings> = Model::ALL
        .iter()
        .flat_map(|m| {
            [Character::Compress, Character::Limit].map(|c| VinculumSettings {
                model: *m,
                character: c,
                threshold_db: -30.0,
                auto_release: true,
                mu_time: 5,
                sc_hpf_hz: 120.0,
                mix: 0.6,
                makeup_db: 4.0,
                colour: 0.8,
                ..VinculumSettings::default()
            })
        })
        .collect();
    WATCH.store(true, Ordering::SeqCst);
    for (k, s) in settings.iter().enumerate() {
        v.set(s);
        for block in 0..20 {
            for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
                let t = (block * 512 + i) as f32 / 48_000.0;
                *a = cantus_dsp::math::sin(t * 700.0 + k as f32);
                *b = 0.8 * cantus_dsp::math::sin(t * 1100.0);
            }
            v.process(&mut l, &mut r);
        }
        let _ = v.take_meter();
        let _ = v.at_rest();
    }
    WATCH.store(false, Ordering::SeqCst);
    assert_eq!(COUNT.load(Ordering::SeqCst), 0);
}
