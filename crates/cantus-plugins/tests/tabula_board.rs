//! fx3:tabula — Tabula exported (ADR 0131 §4): its board travels in the state (saved and
//! loaded whole, as the project saves it), a fresh instance starts on the guitar rig, and a
//! board loaded while the plug-in plays reaches the audio thread — its new pedals adopted,
//! the old ones faded and handed back — without one allocation there.

use cantus_core::fx::tabula::{guitar_rig, Path, PedalId};
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
    Shared::new(cantus_plugins::desc::of(EffectKind::Board).expect("Tabula is exported"))
}

/// A state holding `spec`'s board, as Cantus writes an insert.
fn state_of(spec: &EffectSpec) -> Vec<u8> {
    serde_json::to_vec(spec).unwrap()
}

/// A board of a delay on A beside a phaser on B, a compressor before them.
fn split_board() -> EffectSpec {
    use cantus_core::fx::tabula::{Board, Pedal};
    let mut s = EffectSpec::new(EffectKind::Board);
    let mut a = Pedal::new(PedalId(7), EffectSpec::new(EffectKind::Delay));
    a.path = Path::A;
    a.spec.set("mix", 0.6);
    let mut b = Pedal::new(PedalId(8), EffectSpec::new(EffectKind::Phaser));
    b.path = Path::B;
    s.board = Some(Box::new(Board {
        pedals: vec![
            Pedal::new(PedalId(2), EffectSpec::new(EffectKind::Compressor)),
            a,
            b,
        ],
        macros: Vec::new(),
    }));
    s
}

#[test]
fn a_fresh_board_is_the_guitar_rig_and_its_state_round_trips() {
    let one = shared();
    assert_eq!(
        one.spec().board,
        guitar_rig().board,
        "a fresh instance starts on the rig"
    );
    one.load_state(&state_of(&split_board())).unwrap();
    let saved = one.save_state();
    let two = shared();
    two.load_state(&saved).unwrap();
    assert_eq!(
        two.spec().board,
        split_board().board,
        "the board travels whole"
    );
    assert_eq!(two.spec(), one.spec());
}

#[test]
fn a_board_loaded_while_playing_never_allocates_on_the_audio_thread() {
    let sh = shared();
    let mut p = Processor::new(sh.clone(), 48_000.0);
    let n = 512;
    let tone = |i: usize| cantus_dsp::math::sin(i as f32 * 0.031) * 0.3;
    let mut l: Vec<f32> = (0..n).map(tone).collect();
    let mut r = l.clone();
    for _ in 0..20 {
        p.process(&mut l, &mut r, None, &[]);
    }
    // A new board loaded on the main thread: its new pedals built here.
    sh.load_state(&state_of(&split_board())).unwrap();
    let mut heard = 0f32;
    WATCHING.with(|w| w.set(true));
    for b in 0..120 {
        for (i, (x, y)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            *x = tone(b * n + i);
            *y = *x;
        }
        p.process(&mut l, &mut r, None, &[]);
        heard = heard.max(l.iter().fold(0f32, |m, v| m.max(v.abs())));
    }
    WATCHING.with(|w| w.set(false));
    let count = COUNT.with(|c| c.get());
    assert_eq!(
        count, 0,
        "the audio thread allocated or freed {count} times"
    );
    assert!(heard > 0.01 && heard.is_finite());
    // The rig's pedals came back to the main thread (faded and handed back): freed there.
    sh.board.collect();
    drop(p);
}
