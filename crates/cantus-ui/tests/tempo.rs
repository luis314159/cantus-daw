//! The tempo track, the metre changes and the markers in the application (round 29, ADR
//! 0060): reachable from the app the boot path assembles (the keys through `run_action`),
//! the top bar's tempo at the playhead and what editing it sets, the markers' jumps,
//! loops, selections and export range, the View switch, a v8 keymap given the new keys.

use cantus_core::tempo::{MetreChange, TempoPoint};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::keymap::Action;
use cantus_ui::{views, CantusApp};

fn pt(tick: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp,
    }
}

/// The app as the desktop assembles it (`run` → `boot::Boot` → `assemble`), on `project`.
fn assembled(project: Project) -> CantusApp {
    let data = tempfile::tempdir().unwrap();
    // A data folder of its own, never the user's (`boot_window.rs`).
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

fn song() -> Project {
    let mut p = Project::new("song");
    p.tempo_bpm = 90.0;
    p.tempo_points = vec![pt(0, 90.0, true), pt(7680, 140.0, false)];
    p.metre_changes = vec![
        MetreChange {
            tick: Ticks::ZERO,
            signature: TimeSignature::new(4, 4),
        },
        MetreChange {
            tick: Ticks(7680),
            signature: TimeSignature::new(7, 8),
        },
    ];
    let mut t = p.make_track("Keys");
    t.clips
        .push(p.make_clip("c", Ticks::ZERO, Ticks::beats(16)));
    p.tracks.push(t);
    p
}

#[test]
fn the_keys_add_markers_jump_loop_and_select_between_them() {
    let mut app = assembled(song());
    app.seek(Ticks(960));
    app.run_action(Action::AddMarker);
    app.seek(Ticks(7680));
    app.run_action(Action::AddMarker);
    let names: Vec<String> = app
        .project()
        .markers
        .iter()
        .map(|m| m.name.clone())
        .collect();
    assert_eq!(names, ["Marker 1", "Marker 2"]);
    app.seek(Ticks(5000));
    app.run_action(Action::PreviousMarker);
    assert_eq!(app.engine.position(), Ticks(960));
    app.run_action(Action::NextMarker);
    assert_eq!(app.engine.position(), Ticks(7680));
    app.seek(Ticks(3000));
    app.run_action(Action::LoopBetweenMarkers);
    let l = app.project().loop_region.clone().unwrap();
    assert_eq!((l.start, l.end), (Ticks(960), Ticks(7680)));
    assert!(app.project().loop_enabled);
    app.run_action(Action::SelectBetweenMarkers);
    assert_eq!(app.time_selection, Some((Ticks(960), Ticks(7680))));
    // One undo step per marker and per loop.
    assert_eq!(app.bus.undo_label_key(), Some("cmd.tempo.loop_markers"));
    // The tempo track shows while the project has one; the key hides and shows it.
    assert!(app.tempo_lane_shown());
    app.run_action(Action::ToggleTempoTrack);
    assert!(!app.tempo_lane_shown());
}

#[test]
fn the_top_bar_shows_the_tempo_at_the_playhead_and_sets_the_point_under_it() {
    let mut app = assembled(song());
    // In the ramp: 90 + 50 · 5040 / 7680.
    app.seek(Ticks(5040));
    assert!((app.tempo_now() - 122.8125).abs() < 1e-9);
    assert_eq!(app.metre_now(), TimeSignature::new(4, 4));
    // Drawn for a few frames, the bar writes nothing back (found while shooting it: the
    // rounded value it shows was set again every frame and crept up the ramp).
    let before = app.project().clone();
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::raster::shoot(&ctx, egui::vec2(1400.0, 60.0), 1.0, 4, None, |root| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(root, |ui| views::transport::show(&mut app, ui));
    });
    assert_eq!(*app.project(), before);
    // Editing the tempo sets the point that governs the playhead: here the first (the
    // song's tempo, which a ramp starts from); after the point, that point.
    app.set_tempo_now(100.0, "tempo");
    assert_eq!(app.project().tempo_bpm, 100.0);
    assert_eq!(app.project().tempo_points[0].bpm, 100.0);
    app.seek(Ticks(9000));
    app.set_tempo_now(150.0, "tempo");
    assert_eq!(app.project().tempo_points[1].bpm, 150.0);
    // The metre in force at the playhead: the 7/8 change, edited where it is.
    assert_eq!(app.metre_now(), TimeSignature::new(7, 8));
    app.set_metre_now(TimeSignature::new(5, 8));
    assert_eq!(
        app.project().metre_changes[1].signature,
        TimeSignature::new(5, 8)
    );
    assert_eq!(app.project().metre_changes[1].tick, Ticks(7680));
}

#[test]
fn export_between_markers_is_the_span_between_them() {
    let mut app = assembled(song());
    let a = app.add_marker(Ticks(3840)).unwrap();
    let b = app.add_marker(Ticks(11_040)).unwrap();
    app.export_between_markers(b, a);
    let d = app.export.dialog.clone().expect("the dialog opens");
    assert_eq!(
        d.scope,
        cantus_export::Scope::Markers { from: b.0, to: a.0 }
    );
    assert_eq!(
        views::export::range(&app, d.scope),
        Some((Ticks(3840), Ticks(11_040)))
    );
}

#[test]
fn a_version_8_keymap_gets_the_marker_keys() {
    let text: String = cantus_ui::keymap::Keymap::default()
        .to_toml()
        .lines()
        .filter(|l| {
            ![
                "add-marker",
                "previous-marker",
                "next-marker",
                "loop-between-markers",
                "select-between-markers",
                "toggle-tempo-track",
            ]
            .iter()
            .any(|k| l.trim_start().starts_with(k))
        })
        .map(|l| {
            if l.starts_with("version") {
                "version = 8".to_string()
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    for id in ["add-marker", "select-between-markers", "toggle-tempo-track"] {
        assert!(!text.contains(id), "the v8 text still has {id}");
    }
    let map = cantus_ui::keymap::Keymap::from_toml(&text).unwrap();
    // Review 2 (finding 30): all six, not two.
    for a in [
        Action::AddMarker,
        Action::PreviousMarker,
        Action::NextMarker,
        Action::LoopBetweenMarkers,
        Action::SelectBetweenMarkers,
        Action::ToggleTempoTrack,
    ] {
        assert!(!map.bindings_for(a).is_empty(), "{a:?} has no key");
    }
    assert!(map.conflicts().is_empty(), "{:?}", map.conflicts());
}
