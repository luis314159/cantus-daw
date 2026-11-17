//! fxa:fractus — Fractus exported (ADR 0140, phase B): its lanes are content in its state
//! (saved and loaded whole, as the project saves an insert), a fresh instance starts on its
//! starting stutter, lanes painted in the plug-in's window reach the audio thread while it
//! plays — swapped in, the old ones handed back — without one allocation there, and the DSP
//! (8.6 MB at 48 kHz) is built at activation, never on the audio thread.

use cantus_core::fx::fractus::{last_beat_stutter, Block, Pattern};
use cantus_core::{EffectKind, EffectSpec};
use cantus_plugins::instance::{Processor, Shared};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static WATCHING: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if WATCHING.with(|w| w.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if WATCHING.with(|w| w.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        System.dealloc(ptr, layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        if WATCHING.with(|w| w.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        System.realloc(ptr, layout, new)
    }
}

#[global_allocator]
static A: Counting = Counting;

fn shared() -> std::sync::Arc<Shared> {
    Shared::new(cantus_plugins::desc::of(EffectKind::Glitch).expect("Fractus is exported"))
}

/// A gate in sixteenths over the whole bar, a reverse on beat two at 60 %, a throw on the last
/// sixteenth playing three times in four, the cloud up: lanes and values both.
fn painted() -> EffectSpec {
    let mut s = EffectSpec::new(EffectKind::Glitch);
    let mut p = Pattern::default();
    p.replace(7, &[Block::new(0, 16)]);
    p.replace(
        1,
        &[Block {
            amount: 0.6,
            ..Block::new(4, 4)
        }],
    );
    p.replace(
        8,
        &[Block {
            prob: 0.75,
            ..Block::new(15, 1)
        }],
    );
    s.fractus = Some(Box::new(p));
    s.set("grate", 9.0).unwrap();
    s.set("clevel", 0.3).unwrap();
    s
}

#[test]
fn a_fresh_fractus_starts_on_its_stutter_and_its_lanes_round_trip_in_the_state() {
    let one = shared();
    assert_eq!(
        one.spec().fractus,
        last_beat_stutter().fractus,
        "a fresh instance starts on the starting stutter"
    );
    for (k, v) in &last_beat_stutter().params {
        assert_eq!(one.spec().get(k), *v, "{k}: the starting spec's value");
    }
    one.load_state(&serde_json::to_vec(&painted()).unwrap())
        .unwrap();
    let saved = one.save_state();
    let json: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    assert!(json.get("fractus").is_some(), "the lanes are in the state");
    let two = shared();
    two.load_state(&saved).unwrap();
    assert_eq!(
        two.spec().fractus,
        painted().fractus,
        "the lanes travel whole"
    );
    assert_eq!(two.spec().get("grate"), 9.0);
    assert_eq!(two.spec(), one.spec());
    // A state with no lanes (a host's preset of values only) clears them.
    let mut bare = EffectSpec::new(EffectKind::Glitch);
    bare.set("mix", 0.5).unwrap();
    two.load_state(&serde_json::to_vec(&bare).unwrap()).unwrap();
    assert_eq!(two.spec().fractus, None);
}

#[test]
fn lanes_painted_while_playing_reach_the_audio_thread_without_allocating() {
    let sh = shared();
    // Nothing painted: a wire (the DSP built at activation all the same).
    sh.load_state(&serde_json::to_vec(&EffectSpec::new(EffectKind::Glitch)).unwrap())
        .unwrap();
    WATCHING.with(|w| w.set(true));
    let mut p = Processor::new(sh.clone(), 48_000.0);
    WATCHING.with(|w| w.set(false));
    let built = COUNT.with(|c| c.replace(0));
    assert!(
        built > 0,
        "the DSP is built at activation, on the main thread"
    );
    let n = 480;
    let tone = |i: usize| cantus_dsp::math::sin(i as f32 * 0.031) * 0.3;
    let (mut l, mut r) = (vec![0f32; n], vec![0f32; n]);
    let mut run = |p: &mut Processor, from: usize, blocks: usize, watch: bool| -> f32 {
        let mut most = 0f32;
        WATCHING.with(|w| w.set(watch));
        for b in 0..blocks {
            for (i, (x, y)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
                *x = tone((from + b) * n + i);
                *y = *x;
            }
            let mut dry = [0f32; 480];
            dry.copy_from_slice(&l);
            p.process(&mut l, &mut r, None, &[]);
            most = most.max(
                l.iter()
                    .zip(dry.iter())
                    .fold(0f32, |m, (a, b)| m.max((a - b).abs())),
            );
        }
        WATCHING.with(|w| w.set(false));
        most
    };
    let before = run(&mut p, 0, 20, false);
    assert_eq!(before, 0.0, "nothing painted: the input");
    // The gate over the whole bar, painted in the plug-in's window (main thread). Stopped,
    // the steps rest: the cloud beside them is what is heard.
    let mut lanes = Pattern::default();
    lanes.replace(7, &[Block::new(0, 16)]);
    sh.fractus.set(Some(Box::new(lanes)));
    let mut v = sh.values();
    let i = sh.table.by_key("clevel").unwrap().index;
    v[i] = 0.5;
    sh.set_all(&v);
    let after = run(&mut p, 20, 100, true);
    let count = COUNT.with(|c| c.get());
    assert_eq!(
        count, 0,
        "the audio thread allocated or freed {count} times"
    );
    assert!(
        after > 1e-3 && after.is_finite(),
        "the cloud is heard: {after}"
    );
    sh.fractus.collect();
    drop(p);
}
