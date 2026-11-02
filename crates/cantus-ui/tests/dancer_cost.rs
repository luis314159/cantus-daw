//! What a dancer track costs the window (round 35, ADR 0087): a hidden dancer — no stage,
//! no editor open — costs the whole window what a plain track does: the same triangles and
//! draw calls, and the dancers' frame work allocates nothing once warm.
//!
//! The inspector is held equal (the first track selected in both): selected, a dancer's
//! inspector shows four rows (who dances, the move, what it reacts to, where it shows) on
//! the inspector's own sheet, where Firmus shows twenty — so more of the inspector's later
//! sections come into view, which is the inspector's content, not the dancer's cost.

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::CantusApp;
use egui::{Pos2, Rect};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts this thread's allocations while armed.
struct Counting;

thread_local! {
    static ARMED: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<usize> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let _ = ARMED.try_with(|a| {
            if a.get() {
                let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let _ = ARMED.try_with(|a| {
            if a.get() {
                let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn allocations<R>(f: impl FnOnce() -> R) -> (R, usize) {
    COUNT.with(|c| c.set(0));
    ARMED.with(|a| a.set(true));
    let r = f();
    ARMED.with(|a| a.set(false));
    (r, COUNT.with(|c| c.get()))
}

fn ctx() -> egui::Context {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::font_fallback::enable_music();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// The starter song with a track added: plain, or a dancer hidden.
fn app(dancer: bool) -> CantusApp {
    let mut a = CantusApp::from_parts(
        Some(cantus_ui::starter_project()),
        EngineHandle::dummy(),
        false,
    );
    a.add_track();
    let track = a.project().tracks.last().unwrap().id;
    if dancer {
        assert!(a.exec(Command::SetInstrument {
            track,
            spec: Box::new(InstrumentSpec::Native(NativeSpec::new(
                InstrumentKind::Dancer
            ))),
        }));
        a.set_inst_value(track, "view.mode", 2.0, false);
    }
    let first = a.project().tracks[0].id;
    a.select_track(first);
    a
}

/// Triangles and draw calls of the whole window's frame, after a few to settle.
fn cost(a: &mut CantusApp, c: &egui::Context) -> (usize, usize) {
    let mut last = (0, 0);
    for i in 0..6 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1600.0, 960.0))),
            time: Some(f64::from(i) / 60.0),
            ..Default::default()
        };
        let mut out = c.run_ui(input, |ui| a.frame(ui));
        out.textures_delta.clear();
        let prims = c.tessellate(out.shapes, out.pixels_per_point);
        let tris = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
                _ => 0,
            })
            .sum();
        last = (tris, prims.len());
    }
    last
}

#[test]
fn a_hidden_dancer_track_costs_the_window_what_a_plain_track_does() {
    let c = ctx();
    let (plain_t, plain_d) = cost(&mut app(false), &c);
    let c = ctx();
    let (dance_t, dance_d) = cost(&mut app(true), &c);
    println!("plain {plain_t} triangles / {plain_d} calls; hidden dancer {dance_t} / {dance_d}");
    assert!(dance_d <= plain_d, "{dance_d} draw calls against {plain_d}");
    assert!(
        dance_t <= plain_t + 64,
        "{dance_t} triangles against {plain_t}"
    );
}

/// The dancers' frame work allocates nothing once warm (the specs and the tempo map are
/// read again only when the project changes; buffers are reused) — a hidden dancer, and
/// one dancing stopped (its breath).
#[test]
fn a_warm_frame_of_the_dancers_allocates_nothing() {
    let mut a = app(true);
    for i in 0..3 {
        cantus_ui::dance_live::DanceLive::update(&mut a, f64::from(i) * 0.1);
    }
    let ((), n) = allocations(|| cantus_ui::dance_live::DanceLive::update(&mut a, 1.0));
    assert_eq!(n, 0, "{n} allocations in a warm frame");
    assert_eq!(cantus_ui::dance_live::DanceLive::dancers(&a).len(), 1);
}
