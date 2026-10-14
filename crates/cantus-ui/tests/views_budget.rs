//! What a frame of each editing view costs on the stress project (ADR 0024 §13): the
//! allocations and bytes a warm frame asks for, counted by a global allocator, and the
//! time it takes.
//!
//! The arrangement used to clone every track — notes and all — every frame: 18 MB a
//! frame on this project (`rendering-performance.md` §2). The piano roll cloned the clip.
//! Both paint with the project borrowed now; these numbers hold them to it.

mod support;

use cantus_ui::{stress_project, views, CantusApp};
use std::alloc::{GlobalAlloc, Layout, System};
use std::time::{Duration, Instant};
use support::{app, ctx};

struct Counting;

thread_local! {
    // Per thread (round 26): tests measuring at once never mix their numbers.
    static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ALLOCS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static THIS_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn counted(l: &Layout) {
    if COUNTING.try_with(|c| c.get()).unwrap_or_default() && THIS_THREAD.with(|t| t.get()) {
        {
            let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        };
        {
            let _ = BYTES.try_with(|c| c.set(c.get() + l.size()));
        };
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        counted(&l);
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        counted(&Layout::from_size_align(n, l.align()).unwrap_or(l));
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

#[derive(Copy, Clone, Debug)]
enum View {
    Arrangement,
    PianoRoll,
    Rack,
}

/// What a warm frame of `view` costs: (allocations, bytes, median time), the whole egui
/// frame included — context, layout, text — and the tessellation not.
fn frame_cost(view: View, prepare: impl Fn(&mut CantusApp)) -> (usize, usize, Duration) {
    let mut a = app(stress_project());
    let (t, c) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.select_clip(t, c);
    prepare(&mut a);
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        ..Default::default()
    };
    let run = |a: &mut CantusApp| {
        ctx.run_ui(input.clone(), |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| match view {
                    View::Arrangement => views::arrangement::show(a, ui),
                    View::PianoRoll => views::piano_roll::show(a, ui),
                    View::Rack => views::channel_rack::show(a, ui),
                });
        })
    };
    // Warm: fonts, caches, the brush's buffers, the text layouts.
    for _ in 0..6 {
        run(&mut a).textures_delta.clear();
    }
    let (mut allocs, mut bytes) = (usize::MAX, usize::MAX);
    let mut times = Vec::new();
    for _ in 0..7 {
        THIS_THREAD.with(|t| t.set(true));
        ALLOCS.with(|c| c.set(0));
        BYTES.with(|c| c.set(0));
        COUNTING.with(|c| c.set(true));
        let t0 = Instant::now();
        let mut out = run(&mut a);
        let dt = t0.elapsed();
        COUNTING.with(|c| c.set(false));
        THIS_THREAD.with(|t| t.set(false));
        out.textures_delta.clear();
        drop(out);
        allocs = allocs.min(ALLOCS.try_with(|c| c.get()).unwrap_or_default());
        bytes = bytes.min(BYTES.try_with(|c| c.get()).unwrap_or_default());
        times.push(dt);
    }
    times.sort();
    (allocs, bytes, times[times.len() / 2])
}

#[test]
fn a_warm_frame_of_each_view_copies_no_notes_and_stays_in_budget() {
    type Case = (View, &'static str, fn(&mut CantusApp));
    let cases: [Case; 4] = [
        (View::Arrangement, "arrangement", |_| {}),
        (View::PianoRoll, "piano roll", |_| {}),
        (
            View::PianoRoll,
            "piano roll, whole piece (20 000 notes)",
            |a| {
                a.piano_roll.px_per_beat = 0.5;
                a.piano_roll.key_height = 10.0;
                a.piano_roll.top_lane = 100.0;
            },
        ),
        (View::Rack, "channel rack", |_| {}),
    ];
    let mut report = String::new();
    for (view, name, prepare) in cases {
        let (allocs, bytes, time) = frame_cost(view, prepare);
        report.push_str(&format!(
            "{name:<42} {allocs:>5} allocations  {:>8.1} KB  {:>7.2} ms\n",
            bytes as f64 / 1024.0,
            time.as_secs_f64() * 1e3
        ));
        // ADR 0024 §13: ≤ 256 KB a frame for a whole window. The arrangement alone used
        // to copy 18 MB.
        assert!(
            bytes < 256 * 1024,
            "{name}: a warm frame allocated {bytes} bytes\n{report}"
        );
        assert!(
            allocs < 1_500,
            "{name}: a warm frame made {allocs} allocations\n{report}"
        );
        // ADR 0024 §13: the stress piano roll within 8 ms. A claim about a release build;
        // debug is several times slower and only guards against the order of magnitude.
        let budget = if cfg!(debug_assertions) { 120.0 } else { 8.0 };
        assert!(
            time.as_secs_f64() * 1e3 < budget,
            "{name}: a frame took {time:?}, over {budget} ms\n{report}"
        );
    }
    eprintln!("{report}");
}
