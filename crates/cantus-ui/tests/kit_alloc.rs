//! "No allocation per frame in the painters" (ADR 0024 §13), counted: a global allocator
//! that counts, around painting every part of the kit (text included) into a warm brush.
//! The first pass fills the caches and grows the buffers; every pass after it must not
//! allocate at all.

use cantus_ui::i18n::{default_thread_locale, Locale};
use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::gallery::{self, PARTS};
use cantus_ui::kit::meter::{Levels, MeterBatch};
use cantus_ui::kit::{State, Style, Tokens};
use egui::{Pos2, Rect, Vec2};
use std::alloc::{GlobalAlloc, Layout, System};

struct Counting;

thread_local! {
    // Per thread (round 26): tests measuring at once never mix their numbers.
    static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ALLOCS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static THIS_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNTING.try_with(|c| c.get()).unwrap_or_default() && THIS_THREAD.with(|t| t.get()) {
            {
                let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
            };
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if COUNTING.try_with(|c| c.get()).unwrap_or_default() && THIS_THREAD.with(|t| t.get()) {
            {
                let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
            };
        }
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// Paint every part in every state, then 64 meters; return the allocations each made
/// while `counting` (part name, allocations).
fn paint_everything(b: &mut Brush, s: &Style, counting: bool) -> Vec<(&'static str, usize)> {
    let mut report = Vec::new();
    let mut y = 0.0;
    let mut measure = |name: &'static str, f: &mut dyn FnMut()| {
        THIS_THREAD.with(|t| t.set(true));
        ALLOCS.with(|c| c.set(0));
        COUNTING.with(|c| c.set(counting));
        f();
        COUNTING.with(|c| c.set(false));
        THIS_THREAD.with(|t| t.set(false));
        report.push((name, ALLOCS.try_with(|c| c.get()).unwrap_or_default()));
    };
    for (name, part) in PARTS {
        measure(name, &mut || {
            for (i, (_, st)) in State::ALL.iter().enumerate() {
                let sl = gallery::slot(part);
                let r = Rect::from_min_size(Pos2::new(i as f32 * (sl.x + 8.0), y), sl);
                gallery::paint_part(b, r, s, part, *st);
            }
        });
        y += gallery::slot(part).y + 8.0;
    }
    measure("meters", &mut || {
        let mut meters = MeterBatch::new(s);
        for i in 0..64 {
            let r = Rect::from_min_size(Pos2::new(i as f32 * 18.0, 10.0), Vec2::new(14.0, 120.0));
            meters.paint(
                b,
                r,
                &Levels {
                    level: [-10.0, -12.0],
                    hold: [-6.0, -7.0],
                },
            );
        }
    });
    report
}

#[test]
fn a_warm_frame_of_painting_allocates_nothing() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    let s = Style::new(Tokens::slate());
    let input = || egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 3000.0))),
        ..Default::default()
    };
    let mut brush = Brush::default();
    let mut warm = Vec::new();
    for pass in 0..4 {
        ctx.run_ui(input(), |ui| {
            brush.begin(ui.ctx());
            let report = paint_everything(&mut brush, &s, pass >= 2);
            if pass >= 2 {
                warm.extend(report.into_iter().filter(|(_, n)| *n > 0));
            }
            // The geometry is thrown away rather than handed to epaint: this measures
            // the painters, not egui's own per-frame lists.
            brush.discard();
        })
        .textures_delta
        .clear();
    }
    assert!(
        warm.is_empty(),
        "warm frames allocated while painting: {warm:?}"
    );
}
