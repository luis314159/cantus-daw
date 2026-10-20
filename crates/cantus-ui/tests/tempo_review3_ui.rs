//! Review 3 of the tempo track's gestures (ADR 0060): the metre editor opens where it is
//! seen; a chip or a marker dragged by its travel, never jumping; a frozen sound's refusal
//! said; markers on one tick share their room; the ruler's menu on the grid; automation in
//! one metre snaps to the lines it draws; the top bar's wheel counts whole notches and
//! leaves Ctrl and Shift alone, its drag moves a ramp's start by the pointer's travel; a
//! tempo point dragged past the lane stays drawn; the export dialog finds two markers.

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

fn assembled(project: Project) -> CantusApp {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let mut app = CantusApp::assemble(&ctx, files, None, EngineHandle::dummy(), None, None, None, false);
    app.exec(Command::ReplaceProject {
        project: Box::new(project),
    });
    app
}

fn song() -> Project {
    let mut p = Project::new("song");
    let mut t = p.make_track("Keys");
    t.clips.push(p.make_clip("c", Ticks::ZERO, Ticks::beats(16)));
    p.tracks.push(t);
    p
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    ctx
}

fn arrangement(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 600.0))),
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

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// Press at `from`, move by `by` in steps, release.
fn drag(a: &mut CantusApp, ctx: &egui::Context, from: Pos2, by: egui::Vec2) {
    arrangement(a, ctx, vec![egui::Event::PointerMoved(from)]);
    arrangement(a, ctx, vec![egui::Event::PointerMoved(from), button(from, true)]);
    for i in 1..=4 {
        let p = from + by * (i as f32 / 4.0);
        arrangement(a, ctx, vec![egui::Event::PointerMoved(p)]);
    }
    let to = from + by;
    arrangement(a, ctx, vec![egui::Event::PointerMoved(to), button(to, false)]);
    arrangement(a, ctx, vec![]);
}

/// Finding 14: *Add a metre change* at bar 1 opens its editor on the ruler, drawn this
/// frame, with the top bar's Tempo group hidden or not — and with the playhead far out of
/// view, a new change's editor is in view.
#[test]
fn the_metre_editor_opens_where_it_is_seen() {
    let mut a = assembled(song());
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    views::arrangement::tempo::add_metre_change(&mut a, Ticks::ZERO);
    arrangement(&mut a, &ctx, vec![]);
    assert_eq!(a.arrangement.tempo.metre_editor_at(), Some(Ticks::ZERO));
    // Esc closes it.
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
    assert_eq!(a.arrangement.tempo.metre_editor_at(), None);
    // A change far out of view (bar 200): added, its editor in view.
    views::arrangement::tempo::add_metre_change(&mut a, Ticks::beats(800));
    arrangement(&mut a, &ctx, vec![]);
    assert_eq!(a.arrangement.tempo.metre_editor_at(), Some(Ticks::beats(800)));
}

/// Findings 7, 15, 48: a chip dragged while a sound is frozen says so; and finding 39: a
/// chip grabbed away from its line moves by the pointer's travel, never by the grab.
#[test]
fn a_chip_drag_moves_by_its_travel_and_says_a_freeze() {
    let mut p = song();
    p.metre_changes = vec![mc(0, 4, 4), mc(15_360, 3, 4)];
    let mut a = assembled(p);
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let (_, chip) = a.arrangement.tempo.chips_drawn()[1];
    // Grabbed at its right end, moved 3 px: on the grid of bars it stays.
    drag(&mut a, &ctx, pos2(chip.right() - 1.0, chip.center().y), egui::vec2(3.0, 0.0));
    assert_eq!(a.project().metre_changes[1].tick, Ticks(15_360));
    // Frozen: the drag is refused, and said.
    let key = cantus_core::freeze::content_key(a.project(), cantus_core::freeze::FreezeTarget::Track(a.project().tracks[0].id)).unwrap();
    let mut q = a.project().clone();
    q.tracks[0].frozen = Some(cantus_core::freeze::Frozen { content: key });
    a.exec(Command::ReplaceProject { project: Box::new(q) });
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let (_, chip) = a.arrangement.tempo.chips_drawn()[1];
    let geo = a.arrangement.geometry().unwrap();
    let bar = geo.x(20.0) - geo.x(16.0);
    drag(&mut a, &ctx, chip.center(), egui::vec2(bar, 0.0));
    assert!(a.freeze.refusal.is_some(), "the freeze is said");
}

/// Finding 39: a marker tab grabbed near its end moves by the pointer's travel.
#[test]
fn a_marker_tab_moves_by_its_travel() {
    let mut a = assembled(song());
    let id = a.add_marker(Ticks::beats(8)).unwrap();
    a.exec(Command::SetMarker {
        marker: cantus_core::tempo::Marker {
            name: "A rather long name for a marker".into(),
            ..a.project().marker(id).unwrap().clone()
        },
    });
    a.arrangement.markers = Default::default();
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let tab = a.arrangement.markers.tabs()[0].1;
    drag(&mut a, &ctx, pos2(tab.right() - 2.0, tab.center().y), egui::vec2(3.0, 0.0));
    assert_eq!(a.project().marker(id).unwrap().tick, Ticks::beats(8));
}

/// Findings 26, 44: two markers on one tick and a third just after share the room before
/// it: no tab runs over the third's, each is wide enough to click.
#[test]
fn same_tick_markers_share_the_room_before_the_next() {
    let mut a = assembled(song());
    a.add_marker(Ticks::beats(8)).unwrap();
    a.add_marker(Ticks::beats(8)).unwrap();
    a.add_marker(Ticks(8 * 960 + 400)).unwrap();
    a.arrangement.markers = Default::default();
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let tabs = a.arrangement.markers.tabs().to_vec();
    assert_eq!(tabs.len(), 3);
    let third = tabs[2].1;
    for (_, t, _) in &tabs[..2] {
        assert!(t.right() <= third.left(), "{t:?} runs into {third:?}");
        assert!(t.width() >= 6.0, "{t:?}");
    }
    assert!(tabs[0].1.right() <= tabs[1].1.left(), "side by side");
}

/// Findings 27, 46: the ruler's menu adds on the grid.
#[test]
fn the_ruler_menu_point_is_on_the_grid() {
    let mut a = assembled(song());
    a.arrangement.snap = true;
    a.arrangement.grid = Ticks::beats(4);
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let geo = a.arrangement.geometry().unwrap();
    let at = pos2(geo.x(32.3), geo.ruler_top + 3.0);
    let press = egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    };
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(at), press]);
    assert_eq!(views::arrangement::menu_point(&a, true), Ticks::beats(32));
}

/// Findings 25, 35: in one metre an automation clip's points snap to its content's own
/// grid, which is where its editor draws the lines.
#[test]
fn one_metre_automation_snaps_to_its_content_grid() {
    let p = song();
    let origin = Ticks(1440);
    let got = views::auto_curve::snap_point(&p, origin, Ticks(1000), Ticks::beats(1));
    assert_eq!(got, Ticks(960), "the content's own beat");
}

/// One frame of the top bar with `events`.
fn top_bar(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, egui::vec2(1400.0, 60.0))),
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
}

fn wheel(at: Pos2, unit: egui::MouseWheelUnit, dy: f32, modifiers: egui::Modifiers) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(at),
        egui::Event::MouseWheel {
            unit,
            delta: egui::vec2(0.0, dy),
            phase: egui::TouchPhase::Move,
            modifiers,
        },
    ]
}

/// Findings 31, 47: a touchpad's fractions add up to whole notches; a wheel with Ctrl or
/// Shift leaves the tempo alone.
#[test]
fn the_tempo_wheel_counts_whole_notches_and_leaves_ctrl_alone() {
    let mut a = assembled(song());
    let ctx = context();
    top_bar(&mut a, &ctx, vec![]);
    let well = a.topbar.tempo_well.unwrap().center();
    // 40 points are one notch: ten frames of 4.
    for _ in 0..10 {
        top_bar(&mut a, &ctx, wheel(well, egui::MouseWheelUnit::Point, 4.0, egui::Modifiers::NONE));
    }
    assert_eq!(a.project().tempo_bpm, 121.0);
    for m in [egui::Modifiers::CTRL, egui::Modifiers::SHIFT, egui::Modifiers::COMMAND] {
        top_bar(&mut a, &ctx, wheel(well, egui::MouseWheelUnit::Line, 1.0, m));
    }
    assert_eq!(a.project().tempo_bpm, 121.0);
}

/// Finding 22: the top bar's drag inside a ramp moves the ramp's start by the pointer's
/// travel (0.25 BPM a point), its end untouched, one undo step.
#[test]
fn the_tempo_drag_inside_a_ramp_moves_its_start_by_the_travel() {
    let mut p = song();
    p.tempo_bpm = 120.0;
    p.tempo_points = vec![pt(0, 120.0, false), pt(3840, 90.0, true), pt(11_520, 140.0, false)];
    let mut a = assembled(p);
    a.seek(Ticks(7680));
    let ctx = context();
    top_bar(&mut a, &ctx, vec![]);
    let well = a.topbar.tempo_well.unwrap().center();
    let undo = a.bus.history().len();
    top_bar(&mut a, &ctx, vec![egui::Event::PointerMoved(well)]);
    top_bar(&mut a, &ctx, vec![egui::Event::PointerMoved(well), button(well, true)]);
    for i in 1..=4 {
        top_bar(&mut a, &ctx, vec![egui::Event::PointerMoved(well - egui::vec2(0.0, 10.0 * i as f32))]);
    }
    let end = well - egui::vec2(0.0, 40.0);
    top_bar(&mut a, &ctx, vec![egui::Event::PointerMoved(end), button(end, false)]);
    let pts = &a.project().tempo_points;
    // 40 points up, less the drag's own start threshold: between 9 and 10 BPM, in tenths.
    let moved = pts[1].bpm - 90.0;
    assert!((8.0..=10.0).contains(&moved), "the ramp's start moved {moved}");
    assert_eq!((moved * 10.0).round(), moved * 10.0);
    assert_eq!(pts[2].bpm, 140.0, "its end untouched");
    assert_eq!(a.bus.history().len(), undo + 1, "one undo step");
}

/// Finding 45: the markers around the playhead sharing a tick, the export dialog still
/// finds a pair that makes a range.
#[test]
fn the_export_dialog_finds_two_markers_that_make_a_range() {
    let mut a = assembled(song());
    let x = a.add_marker(Ticks::beats(4)).unwrap();
    let y = a.add_marker(Ticks::beats(4)).unwrap();
    let z = a.add_marker(Ticks::beats(12)).unwrap();
    a.seek(Ticks::ZERO);
    views::export::open(&mut a, Some(cantus_export::Scope::Song));
    let range = |a: &CantusApp, s| views::export::range(a, s);
    let s = cantus_export::Scope::Markers { from: x.0, to: z.0 };
    assert!(range(&a, s).is_some());
    let _ = y;
    assert!(views::export::available(
        &a,
        views::export_rows::default_markers(&a)
    ));
}
