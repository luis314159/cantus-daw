//! Review 2 of the tempo track in the application (ADR 0060): the tempo lane's drag and
//! the top bar's wheel inside a ramp, the ruler's menu, a metre change at bar 1, a project
//! of its own showing its own tempo track, the markers' tabs (a click seeks, same-tick
//! markers side by side, their colours), names never repeated, and the helpers that measure
//! fades, lengths and the clock where they lie.

use cantus_core::tempo::{MetreChange, TempoPoint};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::{views, CantusApp};
use egui::{pos2, Pos2};

fn pt(tick: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp,
    }
}

fn mc(tick: i64, n: u8, d: u8) -> MetreChange {
    MetreChange {
        tick: Ticks(tick),
        signature: TimeSignature::new(n, d),
    }
}

/// The app as the desktop assembles it, on `project`.
fn assembled(project: Project) -> CantusApp {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    app.exec(Command::ReplaceProject {
        project: Box::new(project),
    });
    app
}

fn song(points: Vec<TempoPoint>) -> Project {
    let mut p = Project::new("song");
    p.tempo_bpm = points.first().map_or(120.0, |p| p.bpm);
    p.tempo_points = points;
    let mut t = p.make_track("Keys");
    t.clips
        .push(p.make_clip("c", Ticks::ZERO, Ticks::beats(16)));
    p.tracks.push(t);
    p
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    ctx
}

/// One frame of the arrangement with `events`.
fn arrangement(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(1400.0, 600.0),
        )),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| views::arrangement::show(a, ui));
    })
    .textures_delta
    .clear();
}

fn button(pos: Pos2, b: egui::PointerButton, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: b,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Findings 1 and 55: a dragged tempo point held still stays where it is — the lane's
/// scale is the one at the press, not one rebuilt from the point being dragged.
#[test]
fn a_dragged_tempo_point_held_still_stays_put() {
    let mut a = assembled(song(vec![pt(0, 120.0, false), pt(7680, 140.0, false)]));
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let at = a.arrangement.tempo.points_drawn()[1];
    let up = at - egui::vec2(0.0, 30.0);
    use egui::PointerButton::Primary;
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(at)]);
    arrangement(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(at), button(at, Primary, true)],
    );
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(up)]);
    let first = a.project().tempo_points[1].bpm;
    assert!(first > 140.0, "the drag moved it: {first}");
    for _ in 0..6 {
        arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(up)]);
        assert_eq!(a.project().tempo_points[1].bpm, first, "held still");
    }
    arrangement(&mut a, &ctx, vec![button(up, Primary, false)]);
}

/// Findings 2 and 44: a wheel step on the top bar's tempo inside a ramp moves the ramp's
/// start by whole BPM from its own value, not to the value at the playhead.
#[test]
fn a_wheel_step_inside_a_ramp_moves_its_start_by_one() {
    let mut a = assembled(song(vec![
        pt(0, 120.0, false),
        pt(3840, 90.0, true),
        pt(11_520, 140.0, false),
    ]));
    a.seek(Ticks(7680));
    let ctx = context();
    let frame = |a: &mut CantusApp, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                Pos2::ZERO,
                egui::vec2(1400.0, 60.0),
            )),
            events,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| views::transport::show(a, ui));
        })
        .textures_delta
        .clear();
    };
    frame(&mut a, vec![]);
    let well = a.topbar.tempo_well.expect("the tempo well is drawn");
    frame(&mut a, vec![egui::Event::PointerMoved(well.center())]);
    frame(
        &mut a,
        vec![
            egui::Event::PointerMoved(well.center()),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, 1.0),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..4 {
        frame(&mut a, vec![egui::Event::PointerMoved(well.center())]);
    }
    let start = a.project().tempo_points[1].bpm;
    assert_eq!(start, 91.0, "one notch, one BPM, from the ramp's own start");
    assert_eq!(a.project().tempo_points[2].bpm, 140.0);
}

/// Findings 15 and 32: the ruler's menu acts where it was opened; the toolbar's ▸ menu at
/// the playhead, whatever was right-clicked before.
#[test]
fn the_ruler_menu_acts_where_it_was_opened() {
    let mut a = assembled(song(vec![]));
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let geo = a.arrangement.geometry().unwrap();
    let bar9 = Ticks::beats(32);
    let at = pos2(geo.x(32.0), geo.ruler_top + 3.0);
    use egui::PointerButton::Secondary;
    arrangement(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(at), button(at, Secondary, true)],
    );
    arrangement(&mut a, &ctx, vec![button(at, Secondary, false)]);
    let here = views::arrangement::menu_point(&a, true);
    assert!(
        (here - bar9).0.abs() <= 2,
        "the ruler's menu at {here:?}, bar 9 is {bar9:?}"
    );
    assert_eq!(
        views::arrangement::menu_point(&a, false),
        Ticks::ZERO,
        "▸: the playhead"
    );
}

/// Findings 16 and 33: *Add a metre change here* at bar 1 changes nothing and leaves no
/// empty undo step: it opens the top bar's metre instead.
#[test]
fn a_metre_change_at_bar_one_opens_the_metre() {
    let mut a = assembled(song(vec![]));
    let before = a.project().clone();
    let undo = a.bus.history().len();
    views::arrangement::tempo::add_metre_change(&mut a, Ticks(300));
    assert_eq!(*a.project(), before);
    assert_eq!(a.bus.history().len(), undo, "no empty undo step");
    // Review 3: its editor opens on the ruler, where it is seen, at bar 1.
    assert_eq!(a.arrangement.tempo.metre_editor_at(), Some(Ticks::ZERO));
}

/// Finding 52: a metre chip dragged too far stops at the bar lines it may take.
#[test]
fn a_metre_chip_stops_at_its_neighbours() {
    let mut p = song(vec![]);
    p.metre_changes = vec![mc(0, 4, 4), mc(7680, 3, 4), mc(13_440, 4, 4)];
    let clamp = |to: i64| views::arrangement::tempo::clamp_move(&p, 1, Ticks(to));
    assert_eq!(
        clamp(15_000),
        Some(Ticks(11_520)),
        "the last 4/4 bar line before the next"
    );
    assert_eq!(
        clamp(100),
        Some(Ticks(3840)),
        "a bar after the change before"
    );
    assert_eq!(clamp(7680), None, "where it is");
}

/// Finding 18: another project shows its own tempo track, whatever was hidden before.
#[test]
fn another_project_shows_its_own_tempo_track() {
    let mut a = assembled(song(vec![pt(0, 120.0, false), pt(7680, 90.0, false)]));
    assert!(a.tempo_lane_shown());
    a.toggle_tempo_lane();
    assert!(!a.tempo_lane_shown());
    a.replace_project(song(vec![pt(0, 90.0, true), pt(7680, 140.0, false)]));
    assert!(a.tempo_lane_shown(), "a new project: its tempo track shows");
    a.toggle_tempo_lane();
    a.exec(Command::ReplaceProject {
        project: Box::new(song(vec![pt(0, 100.0, false), pt(960, 80.0, false)])),
    });
    assert!(a.tempo_lane_shown(), "an imported one too");
}

/// Findings 35, 50 and 51: a click on a marker's name seeks (a double-click renames); two
/// markers on one tick sit side by side.
#[test]
fn a_click_on_a_marker_seeks_and_markers_on_one_tick_sit_side_by_side() {
    let mut a = assembled(song(vec![]));
    let v = a.add_marker(Ticks::beats(4)).unwrap();
    let w = a.add_marker(Ticks::beats(4)).unwrap();
    a.add_marker(Ticks::beats(12)).unwrap();
    a.arrangement.markers = Default::default();
    a.seek(Ticks::ZERO);
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let tabs = a.arrangement.markers.tabs().to_vec();
    let tab = |id| tabs.iter().find(|t| t.0 == id).copied().unwrap();
    let (tv, tw) = (tab(v), tab(w));
    assert!(
        !tv.1.intersects(tw.1),
        "side by side: {:?} {:?}",
        tv.1,
        tw.1
    );
    assert!(
        tv.2.width() > 10.0 && tw.2.width() > 10.0,
        "both names show"
    );
    let on = tv.2.center();
    use egui::PointerButton::Primary;
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(on)]);
    arrangement(
        &mut a,
        &ctx,
        vec![button(on, Primary, true), button(on, Primary, false)],
    );
    arrangement(&mut a, &ctx, vec![]);
    assert_eq!(a.engine.position(), Ticks::beats(4), "the click seeks");
    assert_eq!(
        a.arrangement.markers.renaming(),
        None,
        "and does not rename"
    );
}

/// Finding 34: the colours offered stay open through the click that offered them; Esc
/// closes them.
#[test]
fn a_markers_colours_stay_open_through_the_click_that_opened_them() {
    let mut a = assembled(song(vec![]));
    let id = a.add_marker(Ticks::beats(4)).unwrap();
    a.arrangement.markers = Default::default();
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    let away = pos2(700.0, 400.0);
    use egui::PointerButton::Primary;
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(1400.0, 600.0),
        )),
        events: vec![button(away, Primary, true), button(away, Primary, false)],
        ..Default::default()
    };
    let _ = ctx.run_ui(input, |ui| {
        views::arrangement::markers::offer_colours(&mut a, id, ui.ctx());
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| views::arrangement::show(&mut a, ui));
    });
    assert_eq!(a.arrangement.markers.colours_open(), Some(id), "still open");
    arrangement(
        &mut a,
        &ctx,
        vec![egui::Event::Key {
            key: egui::Key::Escape,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }],
    );
    assert_eq!(
        a.arrangement.markers.colours_open(),
        None,
        "Esc closes them"
    );
}

/// Finding 42: a new marker's name is never one already there.
#[test]
fn a_new_markers_name_is_never_taken() {
    let mut a = assembled(song(vec![]));
    let first = a.add_marker(Ticks::beats(1)).unwrap();
    a.add_marker(Ticks::beats(2)).unwrap();
    a.add_marker(Ticks::beats(3)).unwrap();
    a.exec(Command::RemoveMarker { id: first });
    a.add_marker(Ticks::beats(4)).unwrap();
    let mut names: Vec<String> = a.project().markers.iter().map(|m| m.name.clone()).collect();
    let n = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), n, "{names:?}");
}

/// Findings 7, 24, 41 and 53: a fade is measured where it lies — 120 BPM to beat 8, then
/// 60: a clip over beats 0–16, a fade out of one beat lasts one second, a fade in of one
/// beat half of one.
#[test]
fn fades_are_measured_where_they_lie() {
    let p = song(vec![pt(0, 120.0, false), pt(7680, 60.0, false)]);
    let map = p.tempo_map();
    let (s, e) = (Ticks::ZERO, Ticks::beats(16));
    use cantus_ui::audio_clips::{fade_seconds, fade_ticks};
    assert!((fade_seconds(&map, s, e, Ticks::beats(1), true) - 1.0).abs() < 1e-12);
    assert!((fade_seconds(&map, s, e, Ticks::beats(1), false) - 0.5).abs() < 1e-12);
    assert_eq!(fade_ticks(&map, s, e, 1.0, true), Ticks::beats(1));
    assert_eq!(fade_ticks(&map, s, e, 0.5, false), Ticks::beats(1));
}

/// Finding 54: a clip's length through the metre changes — a bar of 3/4 is one bar.
#[test]
fn a_length_is_counted_in_the_bars_it_spans() {
    let mut p = song(vec![]);
    p.metre_changes = vec![mc(0, 4, 4), mc(15_360, 3, 4)];
    let start = Ticks(15_360 + 2880);
    assert_eq!(views::bars::length_bbt(&p, start, Ticks(2880)), (2, 1, 0));
    assert_eq!(
        views::bars::length_bbt(&p, start, Ticks(2880 + 960)),
        (2, 2, 0)
    );
}

/// Finding 56: in pattern mode the clock's seconds run at the pattern's tempo.
#[test]
fn the_pattern_clock_runs_at_the_patterns_tempo() {
    let p = song(vec![pt(0, 90.0, false), pt(72_960, 140.0, false)]);
    let start = Ticks(72_960);
    let s = views::transport::clock::clock_seconds(&p, start + Ticks::beats(7), Some(start));
    assert!(
        (s - 3.0).abs() < 1e-9,
        "7 beats at 140 BPM are 3 s, not {s}"
    );
}

/// Finding 45: an automation clip's points snap to the bars of the timeline it lies on —
/// after a bar of 7/8, the beat lines are half a beat off the content's own count.
#[test]
fn automation_points_snap_to_the_timelines_bars() {
    let mut p = song(vec![]);
    p.metre_changes = vec![mc(0, 4, 4), mc(7680, 7, 8), mc(7680 + 3360, 4, 4)];
    let origin = Ticks(72_960);
    let snapped = views::auto_curve::snap_point(&p, origin, Ticks(1000), Ticks::beats(1));
    let on = snapped + origin;
    assert_eq!(
        views::bars::snap(&p, on, Ticks::beats(1)),
        on,
        "a beat line of the timeline"
    );
}
