//! Moving around the three editing views (ADR 0024 §10), driven the way a person does it:
//! real wheel events with real modifiers through egui's input, not calls into the views.
//!
//! The audit found Shift+wheel dead in all three views by reading the code — egui turns a
//! Shift+wheel into a horizontal delta before any view sees it, and the views read the
//! vertical one (`cantus-audit.md` #4). These tests are the proof it now works.

mod support;

use cantus_core::*;
use cantus_ui::views;
use cantus_ui::{starter_project, welcome_project, CantusApp};
use support::{app, ctx};

#[derive(Copy, Clone, Debug)]
enum View {
    Arrangement,
    PianoRoll,
    Rack,
}

fn show(a: &mut CantusApp, view: View, ui: &mut egui::Ui) {
    match view {
        View::Arrangement => views::arrangement::show(a, ui),
        View::PianoRoll => views::piano_roll::show(a, ui),
        View::Rack => views::channel_rack::show(a, ui),
    }
}

/// Run `frames` frames of a view at 1200 × 600 with `events` in the first and the pointer
/// resting at `at`, and `modifiers` held throughout.
fn frames(
    a: &mut CantusApp,
    ctx: &egui::Context,
    view: View,
    at: egui::Pos2,
    modifiers: egui::Modifiers,
    events: Vec<egui::Event>,
    frames: usize,
) {
    for i in 0..frames {
        let mut ev = vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(at),
        ];
        if i == 0 {
            ev.extend(events.iter().cloned());
        }
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 600.0),
            )),
            events: ev,
            time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| show(a, view, ui));
        })
        .textures_delta
        .clear();
    }
}

/// One notch of a mouse wheel, as a mouse (not a trackpad) reports it.
fn notch(dy: f32, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: egui::vec2(0.0, dy),
        phase: egui::TouchPhase::Move,
        modifiers,
    }
}

/// A point in the middle of each view's grid (below its toolbar and rulers).
fn grid_point(view: View) -> egui::Pos2 {
    match view {
        View::Arrangement => egui::pos2(700.0, 120.0),
        View::PianoRoll => egui::pos2(700.0, 250.0),
        View::Rack => egui::pos2(900.0, 110.0),
    }
}

fn time_scroll(a: &CantusApp, view: View) -> f32 {
    match view {
        View::Arrangement => a.arrangement.scroll_beats,
        View::PianoRoll => a.piano_roll.scroll_beats,
        View::Rack => a.rack.scroll_steps,
    }
}

/// A project long enough to scroll through, with its first clip selected.
fn long_app() -> CantusApp {
    let mut p = starter_project();
    let t = p.tracks[0].id;
    p.tracks[0].clips[0].length = Ticks::beats(256);
    let mut a = app(p);
    let c = a.project().tracks[0].clips[0].id;
    a.select_clip(t, c);
    // The rack fits the pattern to its width until told otherwise; zoomed in, it scrolls.
    a.rack.fit = false;
    a.rack.step_w = 40.0;
    a.piano_roll.px_per_beat = 60.0;
    a
}

#[test]
fn shift_wheel_scrolls_time_in_all_three_views() {
    for view in [View::Arrangement, View::PianoRoll, View::Rack] {
        let mut a = long_app();
        let ctx = ctx();
        let at = grid_point(view);
        frames(&mut a, &ctx, view, at, egui::Modifiers::NONE, vec![], 2);
        let before = time_scroll(&a, view);
        let shift = egui::Modifiers::SHIFT;
        frames(&mut a, &ctx, view, at, shift, vec![notch(-3.0, shift)], 30);
        let after = time_scroll(&a, view);
        assert!(
            after > before + 0.5,
            "{view:?}: Shift+wheel did not scroll time ({before} → {after})"
        );
    }
}

#[test]
fn the_plain_wheel_scrolls_the_rows_of_the_arrangement_and_the_rack() {
    // Twenty tracks: more than either view shows.
    let mut p = welcome_project();
    for i in 0..20 {
        let t = p.make_track(format!("Extra {i}"));
        p.tracks.push(t);
    }
    for view in [View::Arrangement, View::Rack] {
        let mut a = app(p.clone());
        let (t, c) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
        a.select_clip(t, c);
        let ctx = ctx();
        let at = grid_point(view);
        frames(&mut a, &ctx, view, at, egui::Modifiers::NONE, vec![], 2);
        let before = match view {
            View::Arrangement => a.arrangement.geometry().unwrap().rows[0].top,
            _ => a.rack.geometry().unwrap().rows[0].top,
        };
        frames(
            &mut a,
            &ctx,
            view,
            at,
            egui::Modifiers::NONE,
            vec![notch(-4.0, egui::Modifiers::NONE)],
            30,
        );
        let after = match view {
            View::Arrangement => a.arrangement.geometry().unwrap().rows[0].top,
            _ => a.rack.geometry().unwrap().rows[0].top,
        };
        assert!(
            after < before - 10.0,
            "{view:?}: the wheel did not scroll the rows ({before} → {after})"
        );
    }
}

#[test]
fn ctrl_wheel_zooms_time_and_ctrl_alt_wheel_zooms_the_rows() {
    let mut a = long_app();
    let ctx = ctx();
    let at = grid_point(View::Arrangement);
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        at,
        egui::Modifiers::NONE,
        vec![],
        2,
    );
    let (px, rows) = (a.arrangement.px_per_beat, a.arrangement.track_height);
    let ctrl = egui::Modifiers::COMMAND;
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        at,
        ctrl,
        vec![notch(2.0, ctrl)],
        30,
    );
    assert!(
        a.arrangement.px_per_beat > px * 1.05,
        "Ctrl+wheel did not zoom time: {px} → {}",
        a.arrangement.px_per_beat
    );
    assert_eq!(
        a.arrangement.track_height, rows,
        "Ctrl+wheel moved the rows too"
    );
    let ctrl_alt = egui::Modifiers {
        alt: true,
        ..egui::Modifiers::COMMAND
    };
    let px = a.arrangement.px_per_beat;
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        at,
        ctrl_alt,
        vec![notch(2.0, ctrl_alt)],
        30,
    );
    assert!(
        a.arrangement.track_height > rows * 1.05,
        "Ctrl+Alt+wheel did not zoom the rows: {rows} → {}",
        a.arrangement.track_height
    );
    assert!(
        (a.arrangement.px_per_beat - px).abs() < 1e-3,
        "Ctrl+Alt+wheel moved time too"
    );
}

#[test]
fn the_middle_button_pans_both_ways() {
    let mut p = welcome_project();
    for i in 0..20 {
        let t = p.make_track(format!("Extra {i}"));
        p.tracks.push(t);
    }
    let mut a = app(p);
    let ctx = ctx();
    let from = egui::pos2(700.0, 300.0);
    let to = egui::pos2(500.0, 150.0);
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        from,
        egui::Modifiers::NONE,
        vec![],
        2,
    );
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Middle,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        from,
        egui::Modifiers::NONE,
        vec![button(from, true)],
        1,
    );
    for i in 1..=6 {
        let at = from + (to - from) * (i as f32 / 6.0);
        frames(
            &mut a,
            &ctx,
            View::Arrangement,
            at,
            egui::Modifiers::NONE,
            vec![],
            1,
        );
    }
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        to,
        egui::Modifiers::NONE,
        vec![button(to, false)],
        1,
    );
    assert!(
        a.arrangement.scroll_beats > 1.0,
        "dragging left with the middle button did not move time: {}",
        a.arrangement.scroll_beats
    );
    assert!(
        a.arrangement.scroll_y > 50.0,
        "dragging up with the middle button did not scroll the rows: {}",
        a.arrangement.scroll_y
    );
}

#[test]
fn dragging_an_end_of_the_scroll_bar_zooms() {
    // FL's bar: the thumb's ends are handles. Pull the right end to the right and the view
    // shows more of the song — it zooms out.
    let mut a = app(starter_project());
    let ctx = ctx();
    let at = egui::pos2(600.0, 200.0);
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        at,
        egui::Modifiers::NONE,
        vec![],
        2,
    );
    let g = a.arrangement.geometry().unwrap();
    let px = a.arrangement.px_per_beat;
    // Where the thumb's right end is: the bar runs along the top of the ruler, and the
    // thumb covers what is shown of the song.
    let visible = g.width / px;
    let total = {
        let per_bar = 4.0;
        let end = 16.0f32;
        ((end / per_bar).ceil() + 8.0) * per_bar
    }
    .max(visible);
    let bar_y = g.ruler_top - 6.0;
    let end_x = g.left + g.width * (visible / total) - 3.0;
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    let from = egui::pos2(end_x, bar_y);
    let to = egui::pos2(end_x + 150.0, bar_y);
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        from,
        egui::Modifiers::NONE,
        vec![],
        1,
    );
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        from,
        egui::Modifiers::NONE,
        vec![button(from, true)],
        1,
    );
    for i in 1..=5 {
        let p = from + (to - from) * (i as f32 / 5.0);
        frames(
            &mut a,
            &ctx,
            View::Arrangement,
            p,
            egui::Modifiers::NONE,
            vec![],
            1,
        );
    }
    frames(
        &mut a,
        &ctx,
        View::Arrangement,
        to,
        egui::Modifiers::NONE,
        vec![button(to, false)],
        1,
    );
    assert!(
        a.arrangement.px_per_beat < px * 0.95,
        "pulling the bar's end out did not zoom out: {px} → {}",
        a.arrangement.px_per_beat
    );
}
