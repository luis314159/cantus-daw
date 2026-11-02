//! The companion window and the streaming mode (round 35, ADR 0091 §§2–3), through the
//! application's frame. In a test egui embeds the deferred viewport as a window of the root,
//! so its pass runs here as it does in its own OS window.

use cantus_core::Ticks;
use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{default_thread_locale, Locale};
use cantus_ui::keymap::Action as Key;
use cantus_ui::stage::companion::{Action, Backdrop, StageSettings, VisMode};
use cantus_ui::welcome_project;
use egui::{Pos2, Rect, Vec2};

fn app_with(engine: EngineHandle) -> (CantusApp, egui::Context, tempfile::TempDir) {
    default_thread_locale(Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let dir = tempfile::tempdir().unwrap();
    let mut a = CantusApp::from_parts(Some(welcome_project()), engine, false);
    a.settings_path = dir.path().join("settings.json");
    a.layouts_path = dir.path().join("layouts.json");
    a.keymap_path = dir.path().join("keymap.toml");
    a.appearance_path = dir.path().join("appearance.json");
    a.attach(&ctx);
    (a, ctx, dir)
}

fn app() -> (CantusApp, egui::Context, tempfile::TempDir) {
    app_with(EngineHandle::dummy())
}

fn frame(a: &mut CantusApp, ctx: &egui::Context, t: f64) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
        time: Some(t),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
}

fn frames(a: &mut CantusApp, ctx: &egui::Context, t0: f64, n: usize) {
    for i in 0..n {
        frame(a, ctx, t0 + i as f64 / 60.0);
    }
}

fn shared<R>(a: &CantusApp, f: impl FnOnce(&cantus_ui::stage::companion::Shared) -> R) -> R {
    f(&a.stage.companion.shared.lock().unwrap())
}

#[test]
fn its_key_opens_it_and_it_dances_and_measures_itself() {
    let (mut a, ctx, dir) = app();
    frames(&mut a, &ctx, 0.0, 2);
    assert_eq!(shared(&a, |s| s.cost.frames), 0, "closed: no pass");
    a.run_action(Key::ToggleCompanion);
    assert!(a.settings.stage.companion_open);
    let saved = std::fs::read_to_string(dir.path().join("settings.json")).unwrap();
    assert!(
        saved.contains("\"companion_open\": true"),
        "kept for the next session"
    );
    frames(&mut a, &ctx, 1.0, 6);
    let (passes, redraws, cpu, dancers) = shared(&a, |s| {
        (s.cost.frames, s.redraws, s.cost.cpu_ms, s.dancers.len())
    });
    assert!(passes >= 3, "its pass ran ({passes})");
    assert!(redraws >= 1, "the character drawn");
    assert!(cpu > 0.0, "its cost measured");
    assert_eq!(dancers, 1, "one character, the chosen one");
    a.run_action(Key::ToggleCompanion);
    let before = shared(&a, |s| s.cost.frames);
    frames(&mut a, &ctx, 2.0, 4);
    assert_eq!(
        shared(&a, |s| s.cost.frames),
        before,
        "closed: no pass at all"
    );
}

#[test]
fn its_menu_choices_are_carried_out_and_kept() {
    let (mut a, ctx, dir) = app();
    a.run_action(Key::ToggleCompanion);
    frames(&mut a, &ctx, 0.0, 2);
    let push = |a: &CantusApp, act: Action| {
        a.stage.companion.shared.lock().unwrap().actions.push(act);
    };
    push(&a, Action::NextBackdrop);
    push(&a, Action::NextVis);
    push(&a, Action::ToggleOnTop);
    push(&a, Action::ToggleStreaming);
    push(&a, Action::NextSize);
    frames(&mut a, &ctx, 1.0, 2);
    let s = &a.settings.stage;
    assert_eq!(s.backdrop, Backdrop::Theme);
    assert_eq!(s.vis, VisMode::Level);
    assert!(!s.on_top);
    assert!(s.streaming);
    assert_eq!(s.size, cantus_ui::stage::companion::SIZES[2]);
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.path().join("settings.json")).unwrap())
            .unwrap();
    assert_eq!(saved["stage"]["backdrop"], "theme");
    assert_eq!(saved["stage"]["streaming"], true);
    // The characters step through the cast, then the project's dancers, then round again.
    let n = cantus_ui::stage::companion::character_ids().len();
    for _ in 0..n {
        push(&a, Action::NextCharacter);
        frames(&mut a, &ctx, 2.0, 1);
    }
    assert!(a.settings.stage.follow_project);
    push(&a, Action::NextCharacter);
    frames(&mut a, &ctx, 3.0, 1);
    assert!(!a.settings.stage.follow_project);
    push(&a, Action::Close);
    frames(&mut a, &ctx, 4.0, 1);
    assert!(!a.settings.stage.companion_open);
}

#[test]
fn stopped_it_dances_to_whatever_sounds_and_rests_in_silence() {
    let (mut a, ctx, _dir) = app();
    a.run_action(Key::ToggleCompanion);
    frames(&mut a, &ctx, 0.0, 3);
    assert!(!shared(&a, |s| s.frame.playing), "silence, stopped: still");
    let mut m = a.engine.meters().clone();
    m.master = [0.4, 0.4];
    a.engine.set_meters(m);
    frames(&mut a, &ctx, 1.0, 30);
    let (playing, beats) = shared(&a, |s| (s.frame.playing, s.frame.beats));
    assert!(playing, "a sound plays: it dances");
    // Half a second at 120 BPM: about a beat.
    assert!(
        (0.7..1.3).contains(&beats),
        "its own beat at the tempo ({beats})"
    );
    // The transport runs: the song's position is the beat.
    a.engine.set_heard_for_tests(Ticks::beats(9), true);
    frames(&mut a, &ctx, 2.0, 1);
    let beats = shared(&a, |s| s.frame.beats);
    assert!((beats - 9.0).abs() < 1e-6, "{beats}");
}

#[test]
fn the_visualiser_reads_the_master_and_the_tap_runs_only_while_shown() {
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let (mut a, ctx, _dir) = app_with(engine);
    let mut buf = vec![0.0f32; 800 * 2];
    let mut run = |a: &mut CantusApp, t0: f64, n: usize| {
        for i in 0..n {
            processor.process(&mut buf, 2);
            frame(a, &ctx, t0 + i as f64 / 60.0);
        }
    };
    run(&mut a, 0.0, 2);
    assert!(
        !a.engine.master_tap().unwrap().is_on(),
        "closed: the tap is off"
    );
    a.run_action(Key::ToggleCompanion);
    a.toggle_play();
    run(&mut a, 1.0, 40);
    assert!(
        a.engine.master_tap().unwrap().is_on(),
        "shown: the tap is on"
    );
    let bands = shared(&a, |s| s.vis.bands);
    assert!(
        bands.iter().any(|b| *b > 0.05),
        "the welcome song's spectrum shows: {bands:?}"
    );
    a.stage
        .companion
        .shared
        .lock()
        .unwrap()
        .actions
        .push(Action::NextVis);
    a.stage
        .companion
        .shared
        .lock()
        .unwrap()
        .actions
        .push(Action::NextVis);
    run(&mut a, 2.0, 2);
    assert_eq!(a.settings.stage.vis, VisMode::Off);
    assert!(
        !a.engine.master_tap().unwrap().is_on(),
        "visualiser off: the tap is off"
    );
}

#[test]
fn minimised_it_pauses() {
    let (mut a, ctx, _dir) = app();
    a.run_action(Key::ToggleCompanion);
    frames(&mut a, &ctx, 0.0, 4);
    let before = shared(&a, |s| s.redraws);
    // Its window minimised (in a test the window is embedded: its viewport is the root's):
    // its pass returns at once, says so, and the main frame turns its tap off.
    for i in 0..10 {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
            time: Some(1.0 + i as f64 / 60.0),
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .minimized = Some(true);
        ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
    }
    let (after, minimized) = shared(&a, |s| (s.redraws, s.minimized));
    assert!(minimized, "it knows it is minimised");
    assert_eq!(after, before, "minimised: no drawing");
}

#[test]
fn view_menu_switches_and_the_stage_key_hides_the_dancers() {
    let (mut a, ctx, _dir) = app();
    a.run_action(Key::ToggleStreaming);
    assert!(a.settings.stage.streaming && a.settings.stage.companion_open);
    a.run_action(Key::ToggleStage);
    assert!(a.settings.stage.hidden);
    frames(&mut a, &ctx, 0.0, 2);
    a.run_action(Key::ToggleStage);
    assert!(!a.settings.stage.hidden);
}

#[test]
fn a_settings_file_from_before_the_stage_reads_with_its_defaults() {
    let s: cantus_ui::settings::Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(s.stage, StageSettings::default());
    assert!(s.stage.on_top && !s.stage.companion_open);
    let back: StageSettings =
        serde_json::from_str(&serde_json::to_string(&StageSettings::default()).unwrap()).unwrap();
    assert_eq!(back, StageSettings::default());
}

#[test]
fn following_the_project_it_shows_the_projects_dancers() {
    use cantus_core::{Command, InstrumentKind, InstrumentSpec, NativeSpec};
    let (mut a, ctx, _dir) = app();
    for i in 0..2 {
        let track = a.project().tracks[i].id;
        assert!(a.exec(Command::SetInstrument {
            track,
            spec: Box::new(InstrumentSpec::Native(NativeSpec::new(
                InstrumentKind::Dancer
            ))),
        }));
        a.set_inst_value(track, "x", 0.3 + 0.4 * i as f32, false);
    }
    a.sync_engine();
    a.settings.stage.follow_project = true;
    a.run_action(Key::ToggleCompanion);
    frames(&mut a, &ctx, 0.0, 4);
    let (n, xs, name) = shared(&a, |s| {
        (
            s.dancers.len(),
            s.dancers.iter().map(|d| d.1.x).collect::<Vec<_>>(),
            s.character_name.to_string(),
        )
    });
    assert_eq!(n, 2, "both dancers");
    assert!(
        (xs[0] - 0.3).abs() < 1e-3 && (xs[1] - 0.7).abs() < 1e-3,
        "{xs:?}"
    );
    assert_eq!(name, "The project's dancers");
    assert!(shared(&a, |s| s.redraws) >= 2, "each drawn");
}

/// Review #1: following the project's dancers, the companion keeps them moving while the
/// main window is minimised (eframe then runs only `logic`, not the main frame that reads
/// the dancers' feeds and gathers them).
#[test]
fn following_the_project_it_keeps_dancing_while_the_main_window_is_minimised() {
    use cantus_core::{Command, InstrumentKind, InstrumentSpec, NativeSpec};
    let (mut a, ctx, _dir) = app();
    let track = a.project().tracks[0].id;
    assert!(a.exec(Command::SetInstrument {
        track,
        spec: Box::new(InstrumentSpec::Native(NativeSpec::new(
            InstrumentKind::Dancer
        ))),
    }));
    a.sync_engine();
    a.settings.stage.follow_project = true;
    a.run_action(Key::ToggleCompanion);
    a.engine.set_heard_for_tests(Ticks::beats(1), true);
    frames(&mut a, &ctx, 0.0, 3);
    // The main window minimised: one pass says so, then only `logic` runs.
    let mut input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
        time: Some(1.0),
        ..Default::default()
    };
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .minimized = Some(true);
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
    let before = shared(&a, |s| s.dancers[0].1);
    a.engine
        .set_heard_for_tests(Ticks(Ticks::beats(3).0 + 240), true);
    cantus_ui::stage::companion::logic(&mut a, &ctx);
    let after = shared(&a, |s| s.dancers[0].1);
    assert_ne!(
        (before.phase, before.cycle, before.beat_phase),
        (after.phase, after.cycle, after.beat_phase),
        "the project's dancer moved on under `logic`"
    );
    assert!(
        (after.beat_phase - 0.25).abs() < 1e-3,
        "at the heard position ({})",
        after.beat_phase
    );
}

/// Review #5: the audio device reopened makes a new engine whose tap starts off; the
/// companion re-arms it.
#[test]
fn the_tap_is_armed_again_on_a_new_engine() {
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let (mut a, ctx, _dir) = app_with(engine);
    let mut buf = vec![0.0f32; 800 * 2];
    a.run_action(Key::ToggleCompanion);
    for i in 0..3 {
        processor.process(&mut buf, 2);
        frame(&mut a, &ctx, i as f64 / 60.0);
    }
    assert!(a.engine.master_tap().unwrap().is_on());
    // What `restart_audio` does to the handle: a new engine, its tap off.
    let (engine, _p2) = EngineHandle::manual(48_000.0);
    a.engine = engine;
    assert!(!a.engine.master_tap().unwrap().is_on());
    frame(&mut a, &ctx, 1.0);
    assert!(a.engine.master_tap().unwrap().is_on(), "armed again");
}

/// Review #6: *Export video…* from the companion's menu opens the video dialog.
#[test]
fn its_menu_opens_the_video_export() {
    let (mut a, ctx, _dir) = app();
    a.run_action(Key::ToggleCompanion);
    frames(&mut a, &ctx, 0.0, 2);
    assert!(a.video.dialog.is_none());
    a.stage
        .companion
        .shared
        .lock()
        .unwrap()
        .actions
        .push(Action::ExportVideo);
    frames(&mut a, &ctx, 1.0, 1);
    assert!(a.video.dialog.is_some(), "the video dialog is open");
}

/// Review #3: *Export video…* chosen while the main window is minimised (streaming from
/// the companion alone) brings the main window back first — the dialog opens in it.
#[test]
fn its_export_video_brings_the_minimised_main_window_back() {
    let (mut a, ctx, _dir) = app();
    a.run_action(Key::ToggleCompanion);
    frames(&mut a, &ctx, 0.0, 2);
    a.stage
        .companion
        .shared
        .lock()
        .unwrap()
        .actions
        .push(Action::ExportVideo);
    let mut input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1600.0, 900.0))),
        time: Some(1.0),
        ..Default::default()
    };
    input
        .viewports
        .entry(egui::ViewportId::ROOT)
        .or_default()
        .minimized = Some(true);
    let out = ctx.run_logic(&input, |ctx| {
        cantus_ui::stage::companion::logic(&mut a, ctx)
    });
    let root = out
        .viewport_commands
        .get(&egui::ViewportId::ROOT)
        .cloned()
        .unwrap_or_default();
    assert!(
        root.contains(&egui::ViewportCommand::Minimized(false)),
        "{root:?}"
    );
    assert!(root.contains(&egui::ViewportCommand::Focus), "{root:?}");
    assert!(a.video.dialog.is_some(), "the video dialog is open");
}

/// Review #4: a size no window should have (a settings file edited by hand, a window the
/// system made a pixel wide) is brought back within reason before it is drawn.
#[test]
fn a_size_out_of_reason_is_brought_back() {
    use cantus_ui::stage::companion::{sane_size, MAX_SIZE, MIN_SIZE};
    assert_eq!(sane_size([1.0, 2.0]), MIN_SIZE);
    assert_eq!(sane_size([1e9, 500.0]), [MAX_SIZE[0], 500.0]);
    assert_eq!(sane_size([f32::NAN, 500.0]), StageSettings::default().size);
    let (mut a, ctx, _dir) = app();
    a.settings.stage.size = [1.0, 2.0];
    a.run_action(Key::ToggleCompanion);
    frames(&mut a, &ctx, 0.0, 2);
    assert_eq!(a.settings.stage.size, MIN_SIZE);
    assert_eq!(shared(&a, |s| s.settings.size), MIN_SIZE);
}
