//! The *Link to…* picker in a big project (round 26's review): 20 tracks × 100 clips, all
//! 2 000 selected. The rows are answered from one summary of the selection (not once per
//! row and clip), the project's list is kept until the project changes (a keystroke only
//! filters it), and only the rows in view are laid out and painted.
//!
//! **Deterministic** (every `cargo test`): the rows laid out stay those in view; a still
//! frame allocates no more with 2 000 rows than with 5.
//! **Timing** (`#[ignore]`; `cargo test --release -p cantus-ui --test links_picker_big --
//! --ignored --nocapture`): opening, a keystroke and a still frame each within one 60-Hz
//! frame (16.7 ms), the whole application's frame included.

mod support;

use cantus_core::*;
use cantus_ui::keymap::Action;
use cantus_ui::CantusApp;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Instant;
use support::{app, ctx};

struct Counting;
thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ON.with(|o| o.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ON.with(|o| o.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

/// `tracks` × `per` clips of their own, each with a note, named "Clip 1" …; every clip
/// selected (`all`), or only the last one.
fn big(tracks: usize, per: usize, all: bool) -> CantusApp {
    let mut p = Project::new("big");
    let mut n = 0;
    for ti in 0..tracks {
        let mut t = p.make_track(format!("Track {ti}"));
        for ci in 0..per {
            n += 1;
            let mut c = p.make_clip(
                format!("Clip {n}"),
                Ticks::beats(4 * ci as i64),
                Ticks::beats(4),
            );
            let note = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 90);
            c.notes.push(note);
            t.clips.push(c);
        }
        p.tracks.push(t);
    }
    let mut a = app(p);
    let mut clips: Vec<(TrackId, ClipId)> = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter().map(move |c| (t.id, c.id)))
        .collect();
    if !all {
        clips.drain(..clips.len() - 1);
    }
    a.set_clip_selection(clips);
    a
}

/// One frame of the picker alone, with `events`; returns the allocations it made.
fn picker_frame(ctx: &egui::Context, a: &mut CantusApp, events: Vec<egui::Event>) -> u64 {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
        ..Default::default()
    };
    let mut n = 0;
    let mut out = ctx.run_ui(input, |ui| {
        COUNT.with(|c| c.set(0));
        ON.with(|o| o.set(true));
        cantus_ui::views::link_picker::frame(a, ui.ctx());
        ON.with(|o| o.set(false));
        n = COUNT.with(|c| c.get());
    });
    out.textures_delta.clear();
    n
}

#[test]
fn only_the_rows_in_view_are_laid_out() {
    let mut a = big(20, 100, false);
    let ctx = ctx();
    a.attach(&ctx);
    a.run_action(Action::LinkTo);
    for _ in 0..3 {
        picker_frame(&ctx, &mut a, Vec::new());
    }
    assert_eq!(a.link_picker.rows().len(), 2000);
    let laid = a.link_picker.row_rects().len();
    assert!(
        (1..=10).contains(&laid),
        "{laid} rows laid out for an 8-row well"
    );
    // Walking down the list keeps the highlighted row in view.
    for _ in 0..30 {
        picker_frame(&ctx, &mut a, key(egui::Key::ArrowDown));
    }
    picker_frame(&ctx, &mut a, Vec::new());
    let h = a.link_picker.highlighted().expect("a row is ready");
    let panel = a.link_picker.panel_rect();
    let (_, r) = a
        .link_picker
        .row_rects()
        .iter()
        .find(|(t, _)| *t == h)
        .copied()
        .expect("the highlighted row is in view");
    assert!(panel.contains_rect(r), "{r:?} outside {panel:?}");
    assert_eq!(a.link_picker.rows()[30].group.first, h, "the 31st row");
}

fn key(k: egui::Key) -> Vec<egui::Event> {
    let ev = |pressed| egui::Event::Key {
        key: k,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    vec![ev(true), ev(false)]
}

/// A still frame of the open picker allocates no more with 2 000 rows than with 12: the
/// strings are made when the list is built, and only the rows in view are painted.
#[test]
fn a_still_frame_allocates_no_more_with_2000_rows_than_with_12() {
    let still = |mut a: CantusApp| {
        let ctx = ctx();
        a.attach(&ctx);
        a.run_action(Action::LinkTo);
        // Past the panel's fade-in (egui recolours text while an area fades).
        for _ in 0..30 {
            picker_frame(&ctx, &mut a, Vec::new());
        }
        picker_frame(&ctx, &mut a, Vec::new())
    };
    // Twelve rows overflow the well too, so both lists wear the scrollbar.
    let small = still(big(1, 12, true));
    let large = still(big(20, 100, true));
    eprintln!("link picker, still: {small} allocations (12 rows), {large} (2 000 rows)");
    assert!(large <= small, "{large} vs {small}");
    assert!(large <= 40, "{large}");
}

/// Milliseconds of one whole-application frame with `events`.
fn app_frame_ms(ctx: &egui::Context, a: &mut CantusApp, events: Vec<egui::Event>) -> f64 {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        events,
        time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
        ..Default::default()
    };
    let t0 = Instant::now();
    let mut out = ctx.run_ui(input, |ui| a.frame(ui));
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    out.textures_delta.clear();
    ms
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn opening_typing_and_a_still_frame_fit_one_frame_with_2000_selected() {
    let mut a = big(20, 100, true);
    let ctx = ctx();
    a.attach(&ctx);
    for _ in 0..10 {
        app_frame_ms(&ctx, &mut a, Vec::new());
    }
    let closed = app_frame_ms(&ctx, &mut a, Vec::new());
    a.run_action(Action::LinkTo);
    let open = app_frame_ms(&ctx, &mut a, Vec::new());
    app_frame_ms(&ctx, &mut a, Vec::new());
    let typed = app_frame_ms(&ctx, &mut a, vec![egui::Event::Text("1".into())]);
    assert_eq!(a.link_picker.filter, "1");
    let mut still: Vec<f64> = (0..30)
        .map(|_| app_frame_ms(&ctx, &mut a, Vec::new()))
        .collect();
    still.sort_by(f64::total_cmp);
    let still = still[still.len() / 2];
    eprintln!(
        "link picker, 2 000 selected: closed {closed:.2} ms, opening {open:.2} ms, \
         a keystroke {typed:.2} ms, still (median) {still:.2} ms"
    );
    for (what, ms) in [
        ("opening", open),
        ("a keystroke", typed),
        ("a still frame", still),
    ] {
        assert!(ms < 16.7, "{what}: {ms:.2} ms");
    }
}

/// When the filter narrows a scrolled list, the row Enter would choose is in view; walking
/// back up to the first row brings its band back with it.
#[test]
fn the_highlighted_row_stays_in_view_and_the_first_band_comes_back() {
    let mut a = big(1, 40, false);
    let ctx = ctx();
    a.attach(&ctx);
    a.run_action(Action::LinkTo);
    for _ in 0..3 {
        picker_frame(&ctx, &mut a, Vec::new());
    }
    let first = a.link_picker.row_rects()[0];
    // Down 15 and up 20: back on the first row, drawn where it was at the opening (its
    // band above it in view).
    for _ in 0..15 {
        picker_frame(&ctx, &mut a, key(egui::Key::ArrowDown));
    }
    for _ in 0..20 {
        picker_frame(&ctx, &mut a, key(egui::Key::ArrowUp));
    }
    picker_frame(&ctx, &mut a, Vec::new());
    assert_eq!(a.link_picker.highlighted(), Some(first.0));
    assert_eq!(
        a.link_picker.row_rects()[0],
        first,
        "scrolled back to the band"
    );
    // To the bottom, then a filter: the highlight falls back to the first row, in view.
    for _ in 0..38 {
        picker_frame(&ctx, &mut a, key(egui::Key::ArrowDown));
    }
    picker_frame(&ctx, &mut a, vec![egui::Event::Text("1".into())]);
    picker_frame(&ctx, &mut a, Vec::new());
    assert_eq!(a.link_picker.filter, "1");
    let h = a.link_picker.highlighted().expect("a row links");
    assert!(
        a.link_picker.row_rects().iter().any(|(t, _)| *t == h),
        "the highlighted row is in view"
    );
}

/// The first letter typed right after an arrow key reaches the filter.
#[test]
fn a_letter_right_after_an_arrow_reaches_the_filter() {
    let mut a = big(1, 12, false);
    let ctx = ctx();
    a.attach(&ctx);
    a.run_action(Action::LinkTo);
    for _ in 0..3 {
        picker_frame(&ctx, &mut a, Vec::new());
    }
    picker_frame(&ctx, &mut a, key(egui::Key::ArrowDown));
    picker_frame(&ctx, &mut a, vec![egui::Event::Text("x".into())]);
    assert_eq!(a.link_picker.filter, "x");
}
