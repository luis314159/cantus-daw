//! Headless application tests: the glue between UI actions, the command bus, the project
//! file and the renderer, plus the ADR 0003 guardrail (dense views must render).

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::views;
use cantus_ui::{starter_project, stress_project, welcome_project, CantusApp};
use std::time::{Duration, Instant};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

/// A context set up the way `CantusApp::new` sets up the real one: bundled fonts and the
/// design system's type scale. Views ask for named text styles, and egui panics on a style
/// it cannot resolve, so a bare `Context::default()` would not render the app at all.
fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

#[test]
fn starter_project_is_playable_and_selected() {
    let a = app(starter_project());
    let p = a.project();
    assert_eq!(p.tracks.len(), 1);
    assert_eq!(p.tracks[0].clips.len(), 1);
    assert_eq!(p.tracks[0].clips[0].notes.len(), 4);
    assert_eq!(a.selected_track, Some(p.tracks[0].id));
    assert_eq!(a.selected_clip, Some(p.tracks[0].clips[0].id));
}

#[test]
fn edit_flows_are_undoable() {
    let mut a = app(starter_project());
    let initial = a.project().clone();

    a.add_track();
    assert_eq!(a.project().tracks.len(), 2);
    let t2 = a.project().tracks[1].id;
    assert_eq!(a.selected_track, Some(t2));

    a.create_clip(t2, Ticks::beats(4), Ticks::beats(8));
    let clip = a.selected_clip.expect("new clip selected");
    let note = a.bus.new_note(Ticks(100), Ticks::QUARTER, 64, 90);
    let nid = note.id;
    assert!(a.exec(Command::InsertNotes {
        track: t2,
        clip,
        notes: vec![note]
    }));
    a.selected_notes = vec![nid];
    a.quantize_selection();
    let n = a
        .project()
        .track(t2)
        .unwrap()
        .clip(clip)
        .unwrap()
        .note(nid)
        .unwrap();
    assert_eq!(n.start, Ticks::ZERO, "quantize snapped to the 1/16 grid");

    a.select_all_notes();
    a.delete_selection();
    assert!(a
        .project()
        .track(t2)
        .unwrap()
        .clip(clip)
        .unwrap()
        .notes
        .is_empty());

    for _ in 0..10 {
        a.undo();
    }
    let mut now = a.project().clone();
    now.next_id = initial.next_id;
    assert_eq!(now, initial, "undo-all restores the starting project");
    assert!(a.selected_track.is_some(), "selection survives undo");
}

#[test]
fn save_open_export_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(starter_project());
    assert!(a.exec(Command::SetTempo { bpm: 140.0 }));
    let path = dir.path().join("song.cantus");
    a.save_to_path(&path);
    assert!(path.exists());
    assert!(!a.bus.is_dirty());
    let saved = a.project().clone();

    a.new_project();
    assert_eq!(a.project().tempo_bpm, 120.0);
    a.open(&path);
    assert_eq!(*a.project(), saved);
    assert_eq!(a.path.as_deref(), Some(path.as_path()));

    let wav = dir.path().join("song.wav");
    a.export_to(wav.clone());
    let start = Instant::now();
    while a.export_in_progress() {
        a.poll_export();
        assert!(start.elapsed() < Duration::from_secs(30), "export hung");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!a.status.error, "{}", a.status.text);
    let reader = hound::WavReader::open(&wav).unwrap();
    assert_eq!(reader.spec().channels, 2);
    assert!(reader.duration() > 48_000, "at least one second of audio");
}

/// One headless egui frame per view. Fails on a panic anywhere in the painting or
/// interaction code; returns the slowest frame and the fastest.
fn run_frames(a: &mut CantusApp, frames: usize) -> (Duration, Duration) {
    let (worst, best, _) = run_frames_cold(a, frames);
    (worst, best)
}

/// [`run_frames`], and the first frame's time too: the cold one, which builds the font
/// atlas and lays out every text the first time.
fn run_frames_cold(a: &mut CantusApp, frames: usize) -> (Duration, Duration, Duration) {
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        ..Default::default()
    };
    let mut worst = Duration::ZERO;
    let mut best = Duration::MAX;
    let mut first = None;
    for _ in 0..frames {
        let started = Instant::now();
        let mut out = ctx.run_ui(input.clone(), |ui| {
            egui::Panel::top(egui::Id::new("t")).show(ui, |ui| views::transport::show(a, ui));
            egui::Panel::right(egui::Id::new("i"))
                .default_size(300.0)
                .show(ui, |ui| views::inspector::show(a, ui));
            egui::Panel::top(egui::Id::new("a"))
                .default_size(300.0)
                .show(ui, |ui| views::arrangement::show(a, ui));
            egui::CentralPanel::default().show(ui, |ui| views::piano_roll::show(a, ui));
        });
        // No GPU here: acknowledge the font-atlas upload epaint expects a renderer to take.
        out.textures_delta.clear();
        worst = worst.max(started.elapsed());
        best = best.min(started.elapsed());
        first.get_or_insert(started.elapsed());
    }
    (worst, best, first.unwrap_or_default())
}

#[test]
fn views_render_headless() {
    let mut a = app(starter_project());
    run_frames(&mut a, 3);
    assert!(a.piano_roll_notes_drawn >= 4);
}

#[test]
fn stress_project_renders_within_budget() {
    let mut a = app(stress_project());
    let (worst, best, cold) = run_frames_cold(&mut a, 5);
    eprintln!(
        "stress: frames {:.2}–{:.2} ms, notes drawn {}",
        best.as_secs_f64() * 1e3,
        worst.as_secs_f64() * 1e3,
        a.piano_roll_notes_drawn
    );
    assert!(a.piano_roll_notes_drawn > 100);
    // Debug builds are several times slower than release; keep a generous ceiling so
    // the test guards against pathological regressions, not against debug overhead. The
    // fastest of the frames is held to it: a loaded machine (the whole suite, a build)
    // only adds time to some, as it once did to the slowest (266 ms, round 12).
    assert!(
        best < Duration::from_millis(250),
        "the fastest frame took {best:?}"
    );
    // The cold frame has a ceiling of its own, generous for a loaded machine (about 17 ms
    // on a quiet one), so a first frame that takes seconds is still caught.
    assert!(
        cold < Duration::from_millis(1500),
        "the first frame took {cold:?}"
    );
}

#[test]
fn the_mixer_draws_faders_with_a_scale_and_a_readout() {
    // The mixer was the one view `run_frames` never touched, and the fader is most of
    // what it is made of. This renders it and reads back the text it painted: the dB
    // scale, printed once in the legend (ADR 0024 §6), and — from Wide up — each
    // fader's value.
    let mut a = app(starter_project());
    views::mixer::set_width_mode(&mut a, views::mixer::layout::WidthMode::Wide);
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 700.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| views::mixer::show(&mut a, ui));
    });
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    for want in [
        "+12",
        "\u{2212}12",
        "\u{2212}30",
        "\u{2212}60",
        "\u{2212}\u{221e}",
        "+0.0",
    ] {
        assert!(
            text.contains(want),
            "the mixer never painted {want:?}: {text}"
        );
    }
}

/// Tab through the mixer and press an arrow at every control that claims them, checking
/// after each press that the note selection did not move. Returns true once a press has
/// actually reached a vertical fader.
///
/// The arrow pressed is the one that control owns — vertical for a fader, horizontal for
/// a pan or width bar. That is not a convenience: an arrow a widget has *not* claimed is
/// how egui moves focus between widgets, so pressing the wrong one walks the focus around
/// the strip instead of moving the control. Which is itself worth knowing, and is why the
/// assertion below that focus stayed put is part of the test.
fn walk_the_mixer_with_arrows(
    ctx: &egui::Context,
    a: &mut CantusApp,
    mut gain: impl FnMut(&CantusApp) -> f32,
    mut check: impl FnMut(&CantusApp),
) -> bool {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 700.0),
        )),
        ..Default::default()
    };
    let frame = |a: &mut CantusApp, key: Option<egui::Key>, dispatch: bool| {
        let mut i = input.clone();
        if let Some(key) = key {
            i.events.push(egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
        }
        ctx.run_ui(i, |ui| {
            if dispatch {
                a.global_shortcuts(ui.ctx());
            }
            egui::CentralPanel::default().show(ui, |ui| views::mixer::show(a, ui));
        })
        .textures_delta
        .clear();
    };
    frame(a, None, false);
    let mut reached_a_fader = false;
    for _ in 0..24 {
        frame(a, Some(egui::Key::Tab), false);
        // egui only installs a widget's focus filter on the frame *after* it gains focus
        // (`Memory::set_focus_lock_filter` checks `had_focus_last_frame`), so give it the
        // idle frame a real user would. Pressing an arrow in the same frame as the Tab
        // moves the focus instead, which is egui's behaviour and not ours to fix here.
        frame(a, None, false);
        if !cantus_ui::widgets::arrow_keys_claimed(ctx) {
            continue;
        }
        let focused = ctx
            .memory(|m| m.focused())
            .expect("something claimed the keys");
        let rect = ctx.read_response(focused).expect("focused response").rect;
        let vertical = rect.height() > rect.width();
        let before = gain(a);
        frame(
            a,
            Some(if vertical {
                egui::Key::ArrowUp
            } else {
                egui::Key::ArrowRight
            }),
            true,
        );
        check(a);
        assert_eq!(
            ctx.memory(|m| m.focused()),
            Some(focused),
            "the control did not claim the arrow it owns, so focus walked away instead"
        );
        // The first vertical fader on the desk is the master's, which is the number
        // `gain` reads; the strips after it move different numbers.
        if vertical && !reached_a_fader {
            assert!(gain(a) > before, "an arrow key never reached the fader");
            reached_a_fader = true;
        }
    }
    reached_a_fader
}

#[test]
fn a_focused_fader_takes_the_arrow_keys_from_the_note_nudge() {
    // Both want the arrows. Without the claim in `cantus_ui::widgets` one press would
    // move the control *and* transpose every selected note — the sort of defect a user
    // reports as "my notes move on their own". The check runs at every control the Tab
    // key stops on, not only at the fader: they all claim the arrows.
    let mut a = app(starter_project());
    let (track, clip) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.select_all_notes();
    let pitches = |a: &CantusApp| -> Vec<u8> {
        a.project()
            .track(track)
            .unwrap()
            .clip(clip)
            .unwrap()
            .notes
            .iter()
            .map(|n| n.pitch)
            .collect()
    };
    let before = pitches(&a);

    let ctx = ctx();
    let reached = walk_the_mixer_with_arrows(
        &ctx,
        &mut a,
        |a| a.project().master.gain_db,
        |a| {
            assert_eq!(
                pitches(a),
                before,
                "an arrow key on a focused mixer control transposed the notes as well"
            );
        },
    );
    assert!(reached, "tabbing through the mixer never reached a fader");
}

#[test]
fn the_toolbar_is_drawn_by_the_kit() {
    let mut a = app(starter_project());
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::top(egui::Id::new("t")).show(ui, |ui| views::transport::show(&mut a, ui));
    });
    out.textures_delta.clear();
    // The keys, wells and panels are the kit's meshes (ADR 0024 §1): no stock shapes.
    let meshes = out
        .shapes
        .iter()
        .filter(|s| matches!(s.shape, egui::Shape::Mesh(_)))
        .count();
    assert!(meshes >= 1, "the toolbar paints through the kit's brush");
    let text = painted_text(&out.shapes);
    for expected in ["FILE", "VIEW", "1.1.000", "120.0"] {
        assert!(
            text.contains(expected),
            "the toolbar did not paint {expected:?}; painted: {text}"
        );
    }
}

#[test]
fn recording_turns_live_notes_into_one_undoable_take() {
    let mut a = app(starter_project());
    let track = a.selected_track.unwrap();
    let clip = a.selected_clip.unwrap();
    let before_notes = a
        .project()
        .track(track)
        .unwrap()
        .clip(clip)
        .unwrap()
        .notes
        .len();
    let history = a.bus.history().len();

    a.start_recording();
    assert!(a.is_recording());
    let ev = |tick: i64, pitch: u8, vel: u8| cantus_engine::NoteInput {
        track,
        tick: Ticks(tick),
        pitch,
        velocity: vel,
    };
    a.ingest_note_inputs(vec![ev(960, 62, 100), ev(1440, 62, 0)]);
    a.ingest_note_inputs(vec![ev(1920, 65, 90)]);
    // Note past the clip end grows the clip.
    a.ingest_note_inputs(vec![ev(1920 + 16 * 960, 65, 0)]);
    a.stop_recording();
    assert!(!a.is_recording());

    let c = a.project().track(track).unwrap().clip(clip).unwrap();
    assert_eq!(c.notes.len(), before_notes + 2);
    let rec = c.notes.iter().find(|n| n.pitch == 62).unwrap();
    assert_eq!(
        (rec.start, rec.length, rec.velocity),
        (Ticks(960), Ticks(480), 100)
    );
    assert!(
        c.end() >= Ticks(1920 + 16 * 960),
        "clip grew to hold the take"
    );
    assert_eq!(a.bus.history().len(), history + 1, "one undo step per take");
    a.undo();
    let c = a.project().track(track).unwrap().clip(clip).unwrap();
    assert_eq!(c.notes.len(), before_notes);
}

#[test]
fn effects_are_added_and_delivered_to_the_engine() {
    let mut a = app(starter_project());
    let track = a.selected_track.unwrap();
    // Inserts live on the strip the track feeds.
    let strip = a
        .project()
        .track(track)
        .unwrap()
        .strip
        .expect("a new track has a strip");
    assert_eq!(
        a.inspected_strip(),
        Some(strip),
        "the inspector follows the track's strip"
    );
    a.add_effect(strip, EffectKind::Delay);
    a.add_effect(strip, EffectKind::Reverb);
    let fx_on = |a: &CantusApp| a.project().strip(strip).unwrap().effects.len();
    assert_eq!(fx_on(&a), 2);
    a.sync_engine();
    assert_eq!(a.engine.known_effect_count(), 2);
    let fx = a.project().strip(strip).unwrap().effects[0].id;
    a.remove_effect(strip, fx);
    assert_eq!(fx_on(&a), 1);
    a.undo();
    assert_eq!(fx_on(&a), 2);
    // Mixer view renders headless too.
    a.central = cantus_ui::app::CentralView::Mixer;
    run_frames(&mut a, 2);
}

#[test]
fn note_editing_operations() {
    let mut a = app(starter_project());
    let track = a.selected_track.unwrap();
    let clip = a.selected_clip.unwrap();
    let notes = |a: &CantusApp| {
        a.project()
            .track(track)
            .unwrap()
            .clip(clip)
            .unwrap()
            .notes
            .clone()
    };
    let initial = notes(&a);
    assert_eq!(initial.len(), 4);

    // Nudge & transpose the whole selection.
    a.select_all_notes();
    a.nudge_selection(Ticks::EIGHTH);
    assert!(notes(&a)
        .iter()
        .zip(&initial)
        .all(|(n, o)| n.start == o.start + Ticks::EIGHTH));
    a.nudge_selection(-Ticks::beats(10)); // clamps at the clip start
    assert_eq!(
        notes(&a).iter().map(|n| n.start).min().unwrap(),
        Ticks::ZERO
    );
    a.transpose_selection(12);
    assert!(notes(&a)
        .iter()
        .zip(&initial)
        .all(|(n, o)| n.pitch == o.pitch + 12));
    a.transpose_selection(-12);

    // Duplicate: 4 more notes, one grid past the selection span.
    a.duplicate_selection();
    assert_eq!(notes(&a).len(), 8);
    assert_eq!(a.selected_notes.len(), 4, "the copies are selected");
    let span_end = initial.iter().map(|n| n.end()).max().unwrap();
    let copies_start = notes(&a)
        .iter()
        .map(|n| n.start)
        .filter(|s| *s >= span_end)
        .min()
        .unwrap();
    assert_eq!(copies_start, span_end.snap(Ticks::SIXTEENTH).max(span_end));

    // Slice at beat 0.5 splits the first note (C4, one quarter long).
    a.selected_notes.clear();
    let before = notes(&a).len();
    assert_eq!(a.slice_at(Ticks::EIGHTH), 1);
    assert_eq!(notes(&a).len(), before + 1);
    let halves: Vec<_> = notes(&a)
        .into_iter()
        .filter(|n| n.pitch == 60 && n.start < Ticks::QUARTER)
        .collect();
    assert_eq!(halves.len(), 2);
    assert!(halves.iter().all(|n| n.length <= Ticks::EIGHTH));

    // Mute toggles, and muted notes vanish from the playback snapshot.
    let first = notes(&a)[0].id;
    a.toggle_mute_notes(vec![first]);
    assert!(notes(&a)[0].muted);
    let snap = Snapshot::compile(a.project(), 0);
    assert!(snap.tracks[0].events.iter().all(|e| e.note != first));
    a.toggle_mute_notes(vec![first]);
    assert!(!notes(&a)[0].muted);

    // Copy / paste at the playhead (stopped at 0 → pastes at 0), cut removes.
    use cantus_ui::keymap::Action;
    a.focus_piano_roll();
    a.selected_notes = vec![first];
    a.run_action(Action::Copy);
    assert_eq!(a.clipboard.board.as_ref().map(|b| b.len()), Some(1));
    let n_before = notes(&a).len();
    a.run_action(Action::Paste);
    assert_eq!(notes(&a).len(), n_before + 1);
    a.run_action(Action::Cut);
    assert_eq!(notes(&a).len(), n_before);

    // Erase with a merge key: two strokes' erases form one undo step each.
    let history = a.bus.history().len();
    let ids: Vec<_> = notes(&a).iter().take(2).map(|n| n.id).collect();
    let key = MergeKey::of(&"erase-test");
    a.erase_notes(vec![ids[0]], Some(key));
    a.erase_notes(vec![ids[1]], Some(key));
    a.end_gesture();
    assert_eq!(notes(&a).len(), n_before - 2);
    assert_eq!(a.bus.history().len(), history + 1);
    a.undo();
    assert_eq!(notes(&a).len(), n_before);
}

#[test]
fn layouts_are_saved_switched_and_reset_through_the_app() {
    use cantus_ui::app::CentralView;
    use cantus_ui::layout;
    use cantus_ui::views::workspace::Pane;

    let mut a = app(starter_project());
    let dir = tempfile::tempdir().unwrap();
    // Never write to the user's real configuration directory from a test.
    a.layouts_path = layout::layouts_path(dir.path());
    a.settings_path = dir.path().join("settings.json");
    a.appearance_path = dir.path().join("appearance.json");
    let ctx = ctx();

    // Lay the whole window out, as the application does, so the dock has sizes to capture.
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let frame = |a: &mut CantusApp| {
        ctx.run_ui(input.clone(), |ui| a.frame(ui))
            .textures_delta
            .clear();
    };
    frame(&mut a);
    assert!(
        a.workspace.is_visible(Pane::Inspector),
        "the inspector shows by default"
    );

    // Give the inspector a width and put the piano roll in front, then save that shape.
    a.workspace.impose_sizes(320.0, 0.0);
    a.show_central(CentralView::PianoRoll);
    frame(&mut a);
    frame(&mut a);
    a.save_layout_as("Mixing", &ctx);
    assert!(a.layouts_path.exists(), "saving a layout writes the file");

    a.reset_layout();
    assert_eq!(a.central, CentralView::Mixer);
    assert_eq!(a.layouts.current.name, layout::DEFAULT_NAME);
    frame(&mut a);
    assert!(a.workspace.is_visible(Pane::Mixer));

    a.apply_layout("Mixing");
    assert_eq!(a.central, CentralView::PianoRoll);
    frame(&mut a);
    frame(&mut a);
    assert!(a.workspace.is_visible(Pane::PianoRoll));
    let (width, _) = a.workspace.measured_sizes();
    assert!(
        (width.expect("the inspector is laid out") - 320.0).abs() < 2.0,
        "switching back must restore the saved width, got {width:?}"
    );

    // Hiding an area really takes it out of the window (I, or the toolbar's key).
    a.toggle_pane(Pane::Inspector);
    frame(&mut a);
    assert!(!a.workspace.contains(Pane::Inspector));

    // And everything survives a round trip through the file.
    a.store_layouts(&ctx);
    let reloaded = cantus_ui::layout::Layouts::load(&a.layouts_path);
    assert_eq!(reloaded.saved.len(), 1);
    assert_eq!(reloaded.saved[0].name, "Mixing");
    assert!(!reloaded.current.show_inspector);
}

#[test]
fn the_function_keys_toggle_the_areas_and_the_window_keeps_drawing() {
    use cantus_ui::views::workspace::Pane;
    let mut a = app(welcome_project());
    let dir = tempfile::tempdir().unwrap();
    a.settings_path = dir.path().join("settings.json");
    a.appearance_path = dir.path().join("appearance.json");
    let ctx = ctx();
    let none = egui::Modifiers::default();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 720.0),
        )),
        ..Default::default()
    };
    ctx.run_ui(input.clone(), |ui| a.frame(ui))
        .textures_delta
        .clear();
    // F9 hides the mixer that shows, F9 again brings it back.
    assert!(a.workspace.is_visible(Pane::Mixer));
    press(&ctx, &mut a, egui::Key::F9, none);
    assert!(!a.workspace.contains(Pane::Mixer));
    press(&ctx, &mut a, egui::Key::F9, none);
    assert!(a.workspace.is_visible(Pane::Mixer));
    // F7 brings the piano roll to the front of the lower zone.
    press(&ctx, &mut a, egui::Key::F7, none);
    assert!(a.workspace.is_visible(Pane::PianoRoll));
    // F5 and I take the arrangement and the inspector out and back.
    press(&ctx, &mut a, egui::Key::F5, none);
    press(&ctx, &mut a, egui::Key::I, none);
    assert!(!a.workspace.contains(Pane::Arrangement));
    assert!(!a.workspace.contains(Pane::Inspector));
    ctx.run_ui(input.clone(), |ui| a.frame(ui))
        .textures_delta
        .clear();
    press(&ctx, &mut a, egui::Key::F5, none);
    press(&ctx, &mut a, egui::Key::I, none);
    assert!(a.workspace.is_visible(Pane::Arrangement));
    assert!(a.workspace.is_visible(Pane::Inspector));
    // While the typing piano plays, the bare I is a note's neighbour and stays the
    // piano's; Alt+I still reaches the inspector.
    a.keyboard_piano = true;
    press(&ctx, &mut a, egui::Key::I, none);
    assert!(
        a.workspace.is_visible(Pane::Inspector),
        "I stays with the piano"
    );
    let alt = egui::Modifiers {
        alt: true,
        ..Default::default()
    };
    press(&ctx, &mut a, egui::Key::I, alt);
    assert!(!a.workspace.contains(Pane::Inspector), "Alt+I always works");
    a.keyboard_piano = false;
    press(&ctx, &mut a, egui::Key::I, alt);
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
}

#[test]
fn an_idle_window_asks_for_no_frames() {
    // ADR 0024 §13: one ticker asks for frames while something moves, none when idle.
    use cantus_ui::views::workspace_ticker as ticker;
    let mut a = app(welcome_project());
    let dir = tempfile::tempdir().unwrap();
    a.settings_path = dir.path().join("settings.json");
    a.appearance_path = dir.path().join("appearance.json");
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 720.0),
        )),
        ..Default::default()
    };
    // egui and the views settle over the first frames (fonts, layouts, first sizes).
    for _ in 0..4 {
        ctx.run_ui(input.clone(), |ui| a.frame(ui))
            .textures_delta
            .clear();
    }
    // A saved project: no autosave falls due while it sits there.
    a.bus.mark_saved();
    let revision = a.bus.revision();
    // Then idle frames: the ticker asks for nothing, frame after frame. (egui's own
    // repaints — a logo landing from its worker — are egui's, not a steady tick.)
    let before = ticker::requests(&ctx);
    for _ in 0..3 {
        ctx.run_ui(input.clone(), |ui| a.frame(ui))
            .textures_delta
            .clear();
        let tick = ticker::last(&ctx);
        assert!(tick.idle(), "nothing moves, yet the ticker asked: {tick:?}");
    }
    assert_eq!(
        ticker::requests(&ctx),
        before,
        "an idle frame must ask for no repaint"
    );
    assert_eq!(a.bus.revision(), revision, "an idle frame changes nothing");
    // Playing asks for frames at no more than 60 Hz.
    a.toggle_play();
    let mut out = ctx.run_ui(input, |ui| a.frame(ui));
    out.textures_delta.clear();
    a.stop();
    let tick = ticker::last(&ctx);
    if a.engine.is_live() || tick.has(ticker::Reason::Playback) {
        assert!(tick.requested.is_some_and(|d| d >= ticker::FRAME));
    }
}

/// Feed one key press through a frame, exactly as a keyboard would.
fn press(ctx: &egui::Context, a: &mut CantusApp, key: egui::Key, modifiers: egui::Modifiers) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 800.0),
        )),
        // `RawInput` carries no modifier state of its own: it arrives with the events.
        events: vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            },
        ],
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        a.global_shortcuts(ui.ctx());
    })
    .textures_delta
    .clear();
}

#[test]
fn the_keymap_drives_what_the_keyboard_does() {
    use cantus_ui::keymap::{Action, Binding, Preset};
    use cantus_ui::views::piano_roll::Tool;

    let mut a = app(starter_project());
    let ctx = ctx();
    let none = egui::Modifiers::default();
    let cmd = egui::Modifiers {
        command: true,
        ..Default::default()
    };

    // A bare letter picks a tool.
    a.piano_roll.tool = Tool::Draw;
    press(&ctx, &mut a, egui::Key::C, none);
    assert_eq!(a.piano_roll.tool, Tool::Slice, "C is the slice tool");

    // Ctrl+A is a different binding from A, and reaches a different action.
    a.selected_notes.clear();
    press(&ctx, &mut a, egui::Key::A, cmd);
    assert!(!a.selected_notes.is_empty(), "Ctrl+A selects all notes");

    // While the typing piano owns the letters, bare letters stop switching tools — which
    // is why every tool also sits on a number.
    a.keyboard_piano = true;
    a.piano_roll.tool = Tool::Draw;
    press(&ctx, &mut a, egui::Key::C, none);
    assert_eq!(a.piano_roll.tool, Tool::Draw, "the piano owns the letters");
    press(&ctx, &mut a, egui::Key::Num6, none);
    assert_eq!(a.piano_roll.tool, Tool::Slice, "numbers still switch tools");
    // Modified letters are never the piano's.
    a.selected_notes.clear();
    press(&ctx, &mut a, egui::Key::A, cmd);
    assert!(
        !a.selected_notes.is_empty(),
        "Ctrl+A works with the piano on"
    );
    a.keyboard_piano = false;

    // Rebinding really changes what the key does.
    a.keymap.bind(Action::ToolMute, Binding::key(egui::Key::C));
    a.piano_roll.tool = Tool::Draw;
    press(&ctx, &mut a, egui::Key::C, none);
    assert_eq!(
        a.piano_roll.tool,
        Tool::Mute,
        "C was rebound to the mute tool"
    );

    // And a preset switch changes a whole set at once: Live puts the pencil on B.
    a.keymap = Preset::Live.keymap();
    a.piano_roll.tool = Tool::Slice;
    press(&ctx, &mut a, egui::Key::B, none);
    assert_eq!(a.piano_roll.tool, Tool::Draw, "Live draws with B");
}

#[test]
fn the_keymap_file_is_written_and_read_back_by_the_app() {
    use cantus_ui::keymap::{self, Action, Binding, Keymap};

    let mut a = app(starter_project());
    let dir = tempfile::tempdir().unwrap();
    a.keymap_path = keymap::keymap_path(dir.path());

    a.keymap.bind(Action::Panic, Binding::key(egui::Key::F1));
    a.store_keymap();
    assert!(a.keymap_path.exists());

    let back = Keymap::load(&a.keymap_path);
    assert_eq!(
        back.bindings_for(Action::Panic),
        [Binding::key(egui::Key::F1)]
    );
    assert!(back.conflicts().is_empty());
}

/// All the text painted in a frame, however deeply egui nested the shapes.
fn painted_text(shapes: &[egui::epaint::ClippedShape]) -> String {
    fn walk(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push_str(" | ");
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = String::new();
    for s in shapes {
        walk(&s.shape, &mut out);
    }
    out
}

#[test]
fn every_dialog_renders_what_it_should_say() {
    // Dialogs are the easiest place for a missing text style or a bad layout to hide,
    // because nothing else opens them.
    let mut a = app(starter_project());
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    a.show_about = true;
    a.show_shortcuts = true;
    a.show_settings = true;
    a.naming_layout = Some("Mixing".into());

    // egui lays a window out on one frame and paints it on the next, so read the second.
    ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()))
        .textures_delta
        .clear();
    let mut out = ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()));
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    for expected in [
        "Cantus",                   // About
        "Keyboard shortcuts",       // the editor's title
        "Play / pause",             // an action label, so the list really rendered
        "Ctrl+Shift+S",             // a binding, so display_for really rendered
        "Alt+wheel edits velocity", // the mouse help a keymap cannot express
        "Layout name",              // the layout naming prompt
        "Settings",                 // the preferences window
        "Output device",            // ... and that its audio section rendered
        "Autosave every",           // ... and its saving section
        "Interface size",           // ... and that the UI scale is offered
    ] {
        assert!(
            text.contains(expected),
            "no dialog painted {expected:?}; painted:
{text}"
        );
    }

    // The appearance page is tall enough to scroll the sections above out of view, so it
    // gets its own frame rather than being asserted alongside them: its colours tab.
    a.show_theme_editor = true;
    a.appearance_page.tab = cantus_ui::views::appearance::Tab::Colours;
    ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()))
        .textures_delta
        .clear();
    let mut out = ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()));
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    // Not every token: there are sixty-one and the rest are below the fold, which is what
    // a scroll area is for.
    for expected in ["SURFACES", "Deepest ground", "surface.ink"] {
        assert!(
            text.contains(expected),
            "the colour editor did not paint {expected:?}; painted:
{text}"
        );
    }
    a.show_theme_editor = false;

    // The editor must survive a keymap that is already broken, and say so.
    a.keymap.bind(
        cantus_ui::keymap::Action::Panic,
        cantus_ui::keymap::Binding::key(egui::Key::Q),
    );
    a.keymap
        .bindings
        .entry(cantus_ui::keymap::Action::Quantize)
        .or_default()
        .push(cantus_ui::keymap::Binding::key(egui::Key::Q));
    assert_eq!(a.keymap.conflicts().len(), 1);
    ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()))
        .textures_delta
        .clear();
    let mut out = ctx.run_ui(input, |ui| a.dialogs(ui.ctx()));
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    assert!(
        text.contains("more than one action"),
        "a conflicting keymap must be reported in the dialog; painted:
{text}"
    );
}

#[test]
fn settings_are_applied_and_persisted() {
    use cantus_ui::i18n::Locale;
    use cantus_ui::settings::{self, Settings, MIN_AUTOSAVE_SECONDS};

    let mut a = app(starter_project());
    let dir = tempfile::tempdir().unwrap();
    a.settings_path = settings::settings_path(dir.path());

    // Choosing a language stores the preference, so the next run opens in it. The locale
    // is process-wide and these tests run in parallel, so this sets the one the suite
    // already uses rather than racing the other tests for it.
    a.set_locale(Locale::En);
    assert_eq!(
        Settings::load(&a.settings_path).locale.as_deref(),
        Some("en")
    );

    // The autosave interval is a setting, not a constant, and is clamped on the way out
    // so a hand-edited zero cannot turn autosave into a per-frame write.
    a.settings.autosave_seconds = 15;
    a.store_settings();
    assert_eq!(
        Settings::load(&a.settings_path)
            .autosave_interval()
            .as_secs(),
        15
    );
    a.settings.autosave_seconds = 0;
    assert_eq!(
        a.settings.autosave_interval().as_secs(),
        MIN_AUTOSAVE_SECONDS
    );

    // A device that is not there — unplugged since the settings were saved, say — falls
    // back to the system default rather than leaving the user with no sound at all
    // (`EngineHandle::start`). What matters is that the app then says which device it
    // actually opened, so the fallback is visible rather than mysterious.
    a.settings.audio.device = Some("No Such Device 9000".into());
    a.restart_audio();
    assert_ne!(
        a.engine.device_name.as_str(),
        "No Such Device 9000",
        "the missing device cannot have been opened"
    );
    if a.engine.is_live() {
        assert!(!a.status.error, "falling back is not an error");
        assert!(
            a.status.text.contains(&a.engine.device_name),
            "the status must name the device actually in use, got {:?}",
            a.status.text
        );
    }
    // The request is still remembered, so the dialog shows what was asked for and the
    // device is used again once it comes back.
    assert_eq!(
        Settings::load(&a.settings_path).audio.device.as_deref(),
        Some("No Such Device 9000")
    );
}

#[test]
fn opening_the_settings_dialog_changes_nothing_by_itself() {
    // A preferences window that writes a preference merely by being looked at is a nasty
    // bug: every widget in it is bound to a value, and one that reports "changed" on its
    // first frame would silently rewrite the user's settings file.
    use cantus_ui::settings;

    let mut a = app(starter_project());
    let dir = tempfile::tempdir().unwrap();
    a.settings_path = settings::settings_path(dir.path());
    let before = a.settings.clone();
    a.show_settings = true;

    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    // Several frames, because egui lays a window out on one and paints it on the next.
    for _ in 0..3 {
        ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()))
            .textures_delta
            .clear();
    }

    assert_eq!(
        a.settings, before,
        "the dialog edited the settings on its own"
    );
    assert!(
        !a.settings_path.exists(),
        "the dialog wrote the settings file without the user changing anything"
    );
    assert!(a.show_settings, "the dialog closed itself");
}

/// What a piano-roll frame actually costs, at two zoom levels.
///
/// The old version of this test ran the stress project at the default zoom, where culling
/// means only a couple of hundred notes are on screen — so it proved nothing about ADR
/// 0003's "60 fps with 20 000 visible notes". This one measures what a frame costs,
/// tessellation included: `run_ui` only builds the shape list, and turning notes into
/// triangles is where the CPU goes.
///
/// Two zooms, because they answer different questions:
///
/// - **`px_per_beat` 4.0** is as far out as a user can actually zoom (the wheel clamps
///   there), so it is the worst case reachable today.
/// - **`px_per_beat` 0.5** is what seeing a whole long piece at once would need, and is
///   the scenario ADR 0003 named. It is not reachable through the UI yet; measuring it
///   says what lifting the clamp would cost.
fn piano_roll_frame_cost(px_per_beat: f32) -> (usize, f64, f64, usize) {
    piano_roll_frame_cost_of(stress_project(), px_per_beat)
}

/// The same measurement with an empty clip isolates what the chrome costs — grid lines,
/// keyboard, ruler, velocity lane — from what the notes cost.
fn piano_roll_frame_cost_of(project: Project, px_per_beat: f32) -> (usize, f64, f64, usize) {
    let mut a = app(project);
    a.piano_roll.px_per_beat = px_per_beat;
    a.piano_roll.key_height = 10.0;
    a.piano_roll.top_lane = 100.0;
    a.piano_roll.scroll_beats = 0.0;

    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        ..Default::default()
    };

    let mut build = Vec::new();
    let mut tess = Vec::new();
    let mut triangles = 0usize;
    for _ in 0..20 {
        let t0 = Instant::now();
        let mut out = ctx.run_ui(input.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| views::piano_roll::show(&mut a, ui));
        });
        let t1 = Instant::now();
        let prims = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
        let t2 = Instant::now();
        out.textures_delta.clear();
        triangles = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
                _ => 0,
            })
            .sum();
        build.push(t1 - t0);
        tess.push(t2 - t1);
    }
    build.sort();
    tess.sort();
    let med = |v: &Vec<Duration>| v[v.len() / 2].as_secs_f64() * 1e3;
    (a.piano_roll_notes_drawn, med(&build), med(&tess), triangles)
}

/// The stress project with every note removed: same tracks, same clip bounds, no notes.
fn empty_stress_project() -> Project {
    let mut p = stress_project();
    for track in &mut p.tracks {
        for clip in &mut track.clips {
            clip.notes.clear();
        }
    }
    p
}

#[test]
fn stress_piano_roll_frame_cost() {
    for px in [4.0f32, 0.5] {
        let (_, b, t, tris) = piano_roll_frame_cost_of(empty_stress_project(), px);
        eprintln!(
            "px_per_beat {px:>4} | CHROME ONLY (no notes)      | build {b:6.2} ms |              tessellate {t:6.2} ms | total {:6.2} ms | {tris:>7} triangles",
            b + t
        );
    }
    for px in [4.0f32, 0.5] {
        let (notes, build, tess, tris) = piano_roll_frame_cost(px);
        eprintln!(
            "px_per_beat {px:>4} | notes {notes:>6} | build {build:6.2} ms | tessellate \
             {tess:6.2} ms | total {:6.2} ms | {tris:>7} triangles | {:.1} tri/note",
            build + tess,
            tris as f64 / notes.max(1) as f64
        );
    }

    // ADR 0003's guardrail, now that it is actually measurable: 20 000 visible notes
    // inside a 60 fps frame. The ceiling is generous against the measured ~7 ms because
    // these numbers move with whatever else the machine is compiling; it guards against a
    // regression of the order the mesh batching fixed, not against a few per cent.
    let (notes, build, tess, _) = piano_roll_frame_cost(0.5);
    assert_eq!(
        notes, 20_000,
        "the guardrail is about 20 000 *visible* notes"
    );
    // 60 fps is a claim about a release build; debug is several times slower, and these
    // numbers move with whatever else the machine happens to be compiling. So the
    // ceiling is per profile and deliberately loose: it catches a regression of the
    // order the mesh batching fixed (that path measured ~370 ms in debug before it was
    // written), not a few per cent of drift.
    let budget = if cfg!(debug_assertions) { 120.0 } else { 16.6 };
    assert!(
        build + tess < budget,
        "a frame with 20 000 visible notes took {:.2} ms, over the {budget} ms ceiling; \
         ADR 0003 asks for 60 fps, so run with --release for the figure that counts",
        build + tess
    );
}

#[test]
fn the_ruler_thins_its_bar_numbers_out_when_zoomed_out() {
    // A bar number every bar becomes an unreadable smear once a bar is a few pixels wide,
    // which is exactly the range the zoom-out limit was lowered into.
    let count_numbers = |px_per_beat: f32| -> usize {
        let mut a = app(stress_project());
        a.piano_roll.px_per_beat = px_per_beat;
        let ctx = ctx();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 900.0),
            )),
            ..Default::default()
        };
        // Two frames: egui lays out on one and paints on the next.
        ctx.run_ui(input.clone(), |ui| {
            egui::CentralPanel::default().show(ui, |ui| views::piano_roll::show(&mut a, ui));
        })
        .textures_delta
        .clear();
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| views::piano_roll::show(&mut a, ui));
        });
        out.textures_delta.clear();
        // Bar numbers are the only text in the ruler that is a bare integer.
        painted_text(&out.shapes)
            .split(" | ")
            .filter(|t| t.parse::<u32>().is_ok())
            .count()
    };

    let wide = count_numbers(48.0);
    let zoomed_out = count_numbers(0.5);
    assert!(
        wide > 5,
        "at editing zoom the ruler should be numbered: {wide}"
    );
    assert!(
        zoomed_out < 60,
        "at the widest zoom the ruler painted {zoomed_out} bar numbers across 1400 px; \
         they would overlap into a smear"
    );
    assert!(
        zoomed_out > 3,
        "the ruler stopped numbering altogether at the widest zoom"
    );
}

/// The pitches heard at each tick, straight from the snapshot the engine would get.
fn heard(a: &CantusApp) -> Vec<(i64, u8)> {
    let s = Snapshot::compile(a.project(), 0);
    let mut v: Vec<(i64, u8)> = s
        .tracks
        .iter()
        .flat_map(|t| t.events.iter())
        .filter(|e| e.is_on())
        .map(|e| (e.tick.0, e.pitch))
        .collect();
    v.sort_unstable();
    v
}

#[test]
fn clip_editing_is_reversible_and_survives_a_save() {
    let mut a = app(starter_project());
    let track = a.project().tracks[0].id;
    let clip = a.project().tracks[0].clips[0].id;
    let before = a.project().clone();
    let sounded = heard(&a);
    a.focus_arrangement();
    a.select_clip(track, clip);

    // Split in the middle. Nothing about the music changes; only the bookkeeping does.
    let mid = {
        let c = a.project().track(track).unwrap().clip(clip).unwrap();
        c.start + c.length / 2
    };
    a.seek(mid);
    a.run_action(cantus_ui::keymap::Action::SplitClip);
    assert_eq!(a.project().track(track).unwrap().clips.len(), 2);
    assert_eq!(heard(&a), sounded, "splitting changed what is played");

    // Glue them back: the clip comes out the way it went in.
    a.select_all_clips();
    a.run_action(cantus_ui::keymap::Action::GlueClips);
    assert_eq!(a.project().track(track).unwrap().clips.len(), 1);
    assert_eq!(heard(&a), sounded);

    // Mute, colour, repeat.
    a.run_action(cantus_ui::keymap::Action::ToggleClipMute);
    assert!(a.project().track(track).unwrap().clips[0].muted);
    assert!(heard(&a).is_empty(), "a muted clip still sounds");
    a.run_action(cantus_ui::keymap::Action::ToggleClipMute);
    a.set_clip_color(Some(Color::PALETTE[3]));
    a.run_action(cantus_ui::keymap::Action::ToggleClipLoop);
    assert!(a.project().track(track).unwrap().clips[0]
        .content_length
        .is_some());

    // Save and reopen: schema 4 round-trips everything above.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("clips.cantus");
    a.save_to_path(&path);
    let reopened = cantus_project::load(&path, true).expect("reopen");
    assert_eq!(
        &reopened.project,
        a.project(),
        "schema 4 did not round-trip"
    );

    // And every one of those edits undoes.
    for _ in 0..20 {
        a.undo();
    }
    let mut now = a.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before, "undo-all did not restore the project");
}

#[test]
fn a_clip_dragged_to_another_track_takes_its_music_with_it() {
    let mut a = app(starter_project());
    a.add_track();
    let (from, to) = (a.project().tracks[0].id, a.project().tracks[1].id);
    let clip = a.project().tracks[0].clips[0].id;
    let pitches: Vec<u8> = a.project().tracks[0].clips[0]
        .notes
        .iter()
        .map(|n| n.pitch)
        .collect();
    let before = heard(&a);
    a.focus_arrangement();
    a.select_clip(from, clip);
    a.commit_clip_drag(
        (from, clip),
        views::arrangement::ClipDragResult::Move {
            delta: Ticks::beats(4),
            rows: 1,
            copy: false,
        },
    );
    assert!(a.project().track(from).unwrap().clips.is_empty());
    let moved = a
        .project()
        .track(to)
        .unwrap()
        .clip(clip)
        .expect("kept its id");
    assert_eq!(
        moved.notes.iter().map(|n| n.pitch).collect::<Vec<_>>(),
        pitches
    );
    // On the events the engine would play, not on the note list: the notes never move
    // when a clip does — the *window* does — so a list that looks unchanged proves
    // nothing about whether the music went with it.
    assert_eq!(
        heard(&a),
        before
            .iter()
            .map(|(t, p)| (t + Ticks::beats(4).0, *p))
            .collect::<Vec<_>>(),
        "the clip changed track but its music stayed where it was"
    );
    assert_eq!(
        a.clip_selection(),
        vec![(to, clip)],
        "the selection followed"
    );

    // Alt-drag copies instead: the original stays. With linking off (ADR 0037) the copy is
    // independent; with it on — the default — it is the same clip (below).
    a.settings.link_copies = false;
    a.commit_clip_drag(
        (to, clip),
        views::arrangement::ClipDragResult::Move {
            delta: Ticks::beats(8),
            rows: -1,
            copy: true,
        },
    );
    assert_eq!(a.project().track(to).unwrap().clips.len(), 1);
    let copy = &a.project().track(from).unwrap().clips[0];
    assert_ne!(copy.id, clip);
    let original = a.project().track(to).unwrap().clip(clip).unwrap();
    assert!(
        copy.notes.iter().all(|n| original.note(n.id).is_none()),
        "a copy that shares note ids is not an independent copy"
    );
    assert_eq!(copy.link, None);
    // With linking on, the Alt-drag copy and the original are one clip in two places.
    a.settings.link_copies = true;
    a.select_clip(to, clip);
    a.commit_clip_drag(
        (to, clip),
        views::arrangement::ClipDragResult::Move {
            delta: Ticks::beats(16),
            rows: 0,
            copy: true,
        },
    );
    let original = a.project().track(to).unwrap().clip(clip).unwrap().clone();
    let linked = a
        .project()
        .track(to)
        .unwrap()
        .clips
        .iter()
        .find(|c| c.id != clip)
        .expect("the linked copy")
        .clone();
    assert!(original.link.is_some());
    assert_eq!(linked.link, original.link);
    assert_eq!(linked.notes, original.notes);
}

#[test]
fn trimming_and_slipping_go_through_the_content_offset() {
    let mut a = app(starter_project());
    let track = a.project().tracks[0].id;
    let clip = a.project().tracks[0].clips[0].id;
    let sounded = heard(&a);
    a.focus_arrangement();
    a.select_clip(track, clip);

    // Trim a beat off the front: the first note is hidden, the rest do not move.
    a.commit_clip_drag(
        (track, clip),
        views::arrangement::ClipDragResult::TrimStart {
            delta: Ticks::beats(1),
        },
    );
    let c = a.project().track(track).unwrap().clip(clip).unwrap();
    assert_eq!(
        c.offset,
        Ticks::beats(1),
        "trimming did not move the offset"
    );
    assert_eq!(c.notes.len(), 4, "trimming threw notes away");
    let after = heard(&a);
    assert_eq!(after, sounded[1..].to_vec());

    // Trim it back: the note comes back exactly where it was.
    a.commit_clip_drag(
        (track, clip),
        views::arrangement::ClipDragResult::TrimStart {
            delta: Ticks::beats(-1),
        },
    );
    assert_eq!(heard(&a), sounded, "trimming is not lossless");

    // Slip: the window stays, the music moves.
    let (start, length) = {
        let c = a.project().track(track).unwrap().clip(clip).unwrap();
        (c.start, c.length)
    };
    a.commit_clip_drag(
        (track, clip),
        views::arrangement::ClipDragResult::Slip {
            delta: Ticks::beats(1),
        },
    );
    let c = a.project().track(track).unwrap().clip(clip).unwrap();
    assert_eq!(
        (c.start, c.length),
        (start, length),
        "slip moved the window"
    );
    assert_ne!(heard(&a), sounded, "slip did not move the music");
}

#[test]
fn the_arrow_keys_move_whatever_the_user_was_last_editing() {
    // The same key means two things, and which one is decided by where the last edit
    // happened. Getting this wrong moves the notes when the user meant the clip.
    let mut a = app(starter_project());
    let track = a.project().tracks[0].id;
    let clip = a.project().tracks[0].clips[0].id;
    let clip_start = |a: &CantusApp| a.project().track(track).unwrap().clip(clip).unwrap().start;
    let note_start =
        |a: &CantusApp| a.project().track(track).unwrap().clip(clip).unwrap().notes[0].start;

    a.select_clip(track, clip);
    a.select_all_notes();
    a.focus_piano_roll();
    let (c0, n0) = (clip_start(&a), note_start(&a));
    a.run_action(cantus_ui::keymap::Action::NudgeRight);
    assert_eq!(clip_start(&a), c0, "editing notes moved the clip");
    assert_ne!(note_start(&a), n0, "the notes did not move");

    a.focus_arrangement();
    let (c1, n1) = (clip_start(&a), note_start(&a));
    a.run_action(cantus_ui::keymap::Action::NudgeRight);
    assert_ne!(clip_start(&a), c1, "the clip did not move");
    assert_eq!(note_start(&a), n1, "editing clips moved the notes");
}

#[test]
fn copying_clips_keeps_their_shape_across_tracks_and_time() {
    let mut a = app(starter_project());
    a.add_track();
    let (t0, t1) = (a.project().tracks[0].id, a.project().tracks[1].id);
    let c0 = a.project().tracks[0].clips[0].id;
    a.create_clip(t1, Ticks::beats(8), Ticks::beats(4));
    let c1 = a.project().tracks[1].clips[0].id;
    let gap = a.project().track(t1).unwrap().clip(c1).unwrap().start
        - a.project().track(t0).unwrap().clip(c0).unwrap().start;

    a.focus_arrangement();
    a.set_clip_selection(vec![(t0, c0), (t1, c1)]);
    a.run_action(cantus_ui::keymap::Action::Copy);
    a.select_track(t0);
    a.seek(Ticks::beats(32));
    a.run_action(cantus_ui::keymap::Action::Paste);

    let pasted = a.clip_selection();
    assert_eq!(pasted.len(), 2, "both clips should have been pasted");
    let first = clip_at(&a, pasted[0]);
    let second = clip_at(&a, pasted[1]);
    assert_eq!(
        first.start,
        Ticks::beats(32),
        "paste landed off the playhead"
    );
    assert_eq!(
        second.start - first.start,
        gap,
        "the clips lost their spacing in time"
    );
    assert_ne!(
        pasted[0].0, pasted[1].0,
        "the clips lost their spacing in tracks"
    );
}

fn clip_at(a: &CantusApp, r: (TrackId, ClipId)) -> cantus_core::Clip {
    a.project().track(r.0).unwrap().clip(r.1).unwrap().clone()
}

#[test]
fn the_arrangement_renders_clips_with_their_state() {
    let mut a = app(starter_project());
    let track = a.project().tracks[0].id;
    let clip = a.project().tracks[0].clips[0].id;
    a.focus_arrangement();
    a.select_clip(track, clip);
    a.run_action(cantus_ui::keymap::Action::ToggleClipMute);
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 500.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| views::arrangement::show(&mut a, ui));
    });
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    assert!(
        text.contains("(muted)"),
        "a muted clip must say so on the clip itself: {text}"
    );
}

#[test]
fn the_pan_law_is_reachable_reversible_and_stored_in_the_project() {
    // The one setting in this round that changes what a file *renders as*. It has to be
    // in the project, not in the preferences, or the same file would mix differently on
    // two machines (ADR 0012).
    let mut a = app(starter_project());
    assert_eq!(
        a.project().pan_law,
        PanLaw::Minus3dB,
        "a new project should start on constant power"
    );
    a.exec(Command::SetPanLaw {
        law: PanLaw::Minus6dB,
    });
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("law.cantus");
    a.save_to_path(&path);
    let back = cantus_project::load(&path, true).expect("reopen");
    assert_eq!(
        back.project.pan_law,
        PanLaw::Minus6dB,
        "the law did not travel"
    );
    a.undo();
    assert_eq!(
        a.project().pan_law,
        PanLaw::Minus3dB,
        "the law did not undo"
    );
}

#[test]
fn the_mixer_shows_pan_width_and_polarity() {
    // The desk carries values and icons, not words (ADR 0024 §6): the pan, the width and
    // the polarity are read through the hint bar (tests/mixer_desk.rs hovers them); here,
    // the desk's order and its numbers.
    let mut a = app(starter_project());
    let id = a.project().tracks[0].strip.unwrap();
    a.exec(Command::SetStripPan { id, pan: -0.5 });
    a.exec(Command::SetStripWidth { id, width: 0.0 });
    a.exec(Command::SetStripPhaseInvert { id, invert: true });
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 900.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| views::mixer::show(&mut a, ui));
    });
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    // The pan law, on the master.
    assert!(
        text.contains("-3"),
        "the mixer never painted the pan law: {text}"
    );
    // The desk: the master first, then every strip by number, fed or not.
    let m = text.find("MASTER").expect("a master strip");
    let one = text.find("| 1 |").expect("strip 1");
    assert!(m < one, "the master must be on the left: {text}");
    assert!(
        text.contains("| 16 |"),
        "sixteen strips out of the box: {text}"
    );
}

/// A track's route is a command, the mixer follows it, and a strip can be added at the
/// end of the desk.
#[test]
fn tracks_are_routed_to_strips_and_strips_can_be_added() {
    use cantus_ui::keymap::Action;
    let mut a = app(three_tracks());
    let p = a.project();
    let (t0, t1) = (p.tracks[0].id, p.tracks[1].id);
    let (s0, s1) = (p.tracks[0].strip.unwrap(), p.tracks[1].strip.unwrap());
    assert_ne!(s0, s1, "new tracks take different strips");
    assert_eq!(p.strip_number(s0), Some(1));
    assert_eq!(p.strip_number(s1), Some(2));

    // Route the second track onto the first's strip: a bus of two.
    a.route_track(t1, Some(s0));
    assert_eq!(a.project().track(t1).unwrap().strip, Some(s0));
    assert_eq!(a.selected_strip, Some(s0), "the mixer follows the route");
    let fed: Vec<TrackId> = a.project().tracks_on_strip(s0).map(|t| t.id).collect();
    assert_eq!(fed, vec![t0, t1]);
    // And what the engine sees agrees: both tracks point at strip index 0.
    let snap = Snapshot::compile(a.project(), 0);
    assert_eq!(snap.tracks[0].strip, Some(0));
    assert_eq!(snap.tracks[1].strip, Some(0));

    let before = a.project().strips.len();
    a.add_strip();
    assert_eq!(a.project().strips.len(), before + 1);
    let new = a.project().strips.last().unwrap().id;
    assert_eq!(a.selected_strip, Some(new));
    a.undo();
    assert_eq!(a.project().strips.len(), before);

    // Selecting a strip nothing feeds shows it without moving the track selection;
    // selecting a fed strip selects the first track on it.
    let was = a.selected_track;
    a.select_strip(s1);
    assert_eq!(a.selected_strip, Some(s1));
    assert_eq!(a.selected_track, was);
    a.select_strip(s0);
    assert_eq!(a.selected_track, Some(t0));
    assert_eq!(a.selected_strip, Some(s0));
    a.run_action(Action::SelectNextTrack);
    assert_eq!(a.selected_track, Some(t1));
    assert_eq!(a.selected_strip, Some(s0), "the strip follows the track");
}

/// Drive the arrangement with real pointer events: press at `from`, travel to `to` in a
/// few steps, release. This is the only way to test a drag *gesture* — the earlier tests
/// called `commit_clip_drag` directly, which is the last step of it and skips every
/// decision the view makes about what the user grabbed.
fn drag_the_arrangement(
    a: &mut CantusApp,
    aim: impl Fn(&views::arrangement::TimelineGeometry) -> (egui::Pos2, egui::Pos2),
    modifiers: egui::Modifiers,
) {
    let ctx = ctx();
    let size = egui::vec2(1200.0, 400.0);
    let show = |a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
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
    };
    let button = |pos: egui::Pos2, pressed: bool| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    // One frame to lay the view out, then ask it where it put things. Mirroring the
    // layout here instead would make the test break the next time a toolbar is added —
    // which is how it was written the first time, and how it broke.
    show(a, &ctx, vec![]);
    let geo = a.arrangement.geometry().expect("the timeline drew a frame");
    let (from, to) = aim(&geo);
    // Hover first: the view hit-tests what is under the pointer before anything is
    // pressed, so a press with no preceding move would land on nothing.
    show(a, &ctx, vec![egui::Event::PointerMoved(from)]);
    show(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=4 {
        let t = i as f32 / 4.0;
        let at = from + (to - from) * t;
        show(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    }
    show(
        a,
        &ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    show(a, &ctx, vec![]);
}

#[test]
fn dragging_a_clip_edge_resizes_it() {
    let mut a = app(starter_project());
    let (track, clip) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    let before = a
        .project()
        .track(track)
        .unwrap()
        .clip(clip)
        .unwrap()
        .clone();
    assert_eq!(before.start, Ticks::ZERO);
    assert_eq!(before.length, Ticks::beats(16));

    // Grab the right edge — a few pixels inside it — and pull it four beats left.
    drag_the_arrangement(
        &mut a,
        |g| {
            let edge = g.x(16.0) - 3.0;
            (
                egui::pos2(edge, g.row_y(0)),
                egui::pos2(edge - 4.0 * g.px_per_beat, g.row_y(0)),
            )
        },
        egui::Modifiers::NONE,
    );
    let after = a.project().track(track).unwrap().clip(clip).unwrap();
    assert_eq!(after.start, Ticks::ZERO, "resizing moved the clip");
    assert_eq!(
        after.length,
        Ticks::beats(12),
        "dragging the right edge did not resize the clip"
    );
}

#[test]
fn dragging_the_left_edge_trims_without_moving_the_notes() {
    let mut a = app(starter_project());
    let (track, clip) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    let pitches: Vec<(Ticks, u8)> = a.project().tracks[0].clips[0]
        .notes
        .iter()
        .map(|n| (n.start, n.pitch))
        .collect();

    drag_the_arrangement(
        &mut a,
        |g| {
            let edge = g.x(0.0) + 3.0;
            (
                egui::pos2(edge, g.row_y(0)),
                egui::pos2(edge + 4.0 * g.px_per_beat, g.row_y(0)),
            )
        },
        egui::Modifiers::NONE,
    );
    let after = a.project().track(track).unwrap().clip(clip).unwrap();
    assert_eq!(after.start, Ticks::beats(4), "the left edge did not move");
    assert_eq!(after.length, Ticks::beats(12), "the clip did not shrink");
    assert_eq!(
        after.offset,
        Ticks::beats(4),
        "the content offset did not follow"
    );
    assert_eq!(
        after
            .notes
            .iter()
            .map(|n| (n.start, n.pitch))
            .collect::<Vec<_>>(),
        pitches,
        "trimming moved the notes"
    );
}

#[test]
fn dragging_the_middle_of_a_clip_moves_it() {
    let mut a = app(starter_project());
    let (track, clip) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    drag_the_arrangement(
        &mut a,
        |g| {
            (
                egui::pos2(g.x(8.0), g.row_y(0)),
                egui::pos2(g.x(12.0), g.row_y(0)),
            )
        },
        egui::Modifiers::NONE,
    );
    let after = a.project().track(track).unwrap().clip(clip).unwrap();
    assert_eq!(after.start, Ticks::beats(4), "the clip did not move");
    assert_eq!(after.length, Ticks::beats(16), "moving resized the clip");
}

#[test]
fn the_clip_menu_opens_on_a_right_click_and_survives_the_pointer_leaving() {
    // A context menu appears *next to* the pointer, so reaching for it takes the pointer
    // off the clip. Drawing it only while its own target is hovered meant it vanished
    // before it could be used — and a menu you cannot reach is the same as no menu.
    let mut a = app(starter_project());
    let ctx = ctx();
    let size = egui::vec2(1200.0, 400.0);
    let frame = |a: &mut CantusApp, events: Vec<egui::Event>| -> String {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| views::arrangement::show(a, ui));
        });
        out.textures_delta.clear();
        painted_text(&out.shapes)
    };
    // Lay the view out, then ask it where the clip is (`TimelineGeometry`).
    frame(&mut a, vec![]);
    let geo = a.arrangement.geometry().expect("the timeline drew a frame");
    let on_clip = egui::pos2(geo.x(8.0), geo.row_y(0));
    frame(&mut a, vec![egui::Event::PointerMoved(on_clip)]);
    let secondary = |pressed| egui::Event::PointerButton {
        pos: on_clip,
        button: egui::PointerButton::Secondary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame(&mut a, vec![secondary(true)]);
    frame(&mut a, vec![secondary(false)]);
    // egui lays the menu out on the frame after the click.
    let opened = frame(&mut a, vec![]);
    assert!(
        opened.contains("Split at the playhead"),
        "right-clicking a clip did not open its menu: {opened}"
    );
    assert!(
        opened.contains("Ctrl+E"),
        "the menu does not show the shortcut it teaches: {opened}"
    );

    // Now move the pointer well away, as anyone reaching for the menu would.
    let away = frame(
        &mut a,
        vec![egui::Event::PointerMoved(egui::pos2(
            geo.x(40.0),
            geo.row_y(2),
        ))],
    );
    assert!(
        away.contains("Split at the playhead"),
        "the clip menu closed as soon as the pointer left the clip: {away}"
    );
}

#[test]
fn clip_editing_is_visible_in_the_menu_bar() {
    // Everything in this round was reachable only from a right-click and from shortcuts
    // nobody had been told about, which is the same as not being there.
    let mut a = app(starter_project());
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 200.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::Panel::top(egui::Id::new("t")).show(ui, |ui| views::transport::show(&mut a, ui));
    });
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    assert!(
        text.contains("CLIP"),
        "the menu bar has no Clip menu: {text}"
    );
}

#[test]
fn the_arrangement_toolbar_is_a_row_of_icons_with_a_grid_picker() {
    // The clip operations were reachable from a menu and a shortcut, which is not the same
    // as being to hand. This is the row that makes them findable.
    let mut a = app(starter_project());
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 400.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| views::arrangement::show(&mut a, ui));
    });
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    // The snap switch is an LED button, whose word is set in capitals (ADR 0024 §4).
    for want in ["Arrangement", "Grid", "1 bar", "SNAP", "1 selected"] {
        assert!(
            text.contains(want),
            "the toolbar never painted {want:?}: {text}"
        );
    }
    // The keys are the kit's toolbar keys: each is painted as a mesh of its own inside the
    // toolbar's strip. Count them, so the test fails if the row silently loses one.
    let keys = out
        .shapes
        .iter()
        .filter(|s| {
            matches!(&s.shape, egui::Shape::Mesh(m)
                if !m.vertices.is_empty()
                    && m.vertices.iter().all(|v| v.pos.y <= 36.0)
                    && m.vertices.iter().any(|v| v.pos.y >= 4.0))
        })
        .count();
    assert!(
        keys >= 8,
        "the arrangement toolbar lost its keys: {keys} meshes in its strip"
    );
}

#[test]
fn every_clip_action_has_a_key_bound_to_it() {
    // "Shortcuts for all of it" is only true if it stays true, so this asserts it for the
    // whole category rather than for the four that had one when it was written.
    use cantus_ui::keymap::{Action, Category, Keymap};
    let map = Keymap::default();
    let unbound: Vec<&str> = Action::ALL
        .into_iter()
        .filter(|a| a.category() == Category::Clips)
        .filter(|a| map.bindings_for(*a).is_empty())
        .map(|a| a.id())
        .collect();
    assert!(
        unbound.is_empty(),
        "clip actions with no shortcut: {unbound:?}"
    );
}

#[test]
fn renaming_a_clip_goes_through_a_command_and_undoes() {
    let mut a = app(starter_project());
    let (track, clip) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    let before = a
        .project()
        .track(track)
        .unwrap()
        .clip(clip)
        .unwrap()
        .name
        .clone();
    a.focus_arrangement();
    a.select_clip(track, clip);
    a.rename_clip((track, clip), "Verse".into());
    assert_eq!(
        a.project().track(track).unwrap().clip(clip).unwrap().name,
        "Verse"
    );
    // An abandoned edit must not wipe the name.
    a.rename_clip((track, clip), "   ".into());
    assert_eq!(
        a.project().track(track).unwrap().clip(clip).unwrap().name,
        "Verse"
    );
    a.undo();
    assert_eq!(
        a.project().track(track).unwrap().clip(clip).unwrap().name,
        before
    );
}

#[test]
fn the_clip_colour_key_cycles_through_the_palette_and_back_off() {
    let mut a = app(starter_project());
    let (track, clip) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.focus_arrangement();
    a.select_clip(track, clip);
    let colour = |a: &CantusApp| a.project().track(track).unwrap().clip(clip).unwrap().color;
    assert_eq!(colour(&a), None, "a new clip follows its track");
    for expected in Color::PALETTE {
        a.run_action(cantus_ui::keymap::Action::ClipColorNext);
        assert_eq!(colour(&a), Some(expected));
    }
    // Off the end of the palette and back to the track's colour: every state is leavable
    // with the same key that reached it.
    a.run_action(cantus_ui::keymap::Action::ClipColorNext);
    assert_eq!(colour(&a), None);
}

// ---- the channel rack, and a piano roll that knows what a pad is ---------------------

/// A project with a Pulsus kit on its only track, and an empty two-bar clip.
fn kit_project() -> Project {
    let mut p = Project::new("kit");
    let mut t = p.make_track("Drums");
    t.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    let clip = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(8));
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn kit_app() -> CantusApp {
    let mut a = app(kit_project());
    let t = a.project().tracks[0].id;
    let c = a.project().tracks[0].clips[0].id;
    a.select_clip(t, c);
    a
}

/// What the engine would actually play: `(tick, pitch)` for every note-on.
fn note_ons(a: &CantusApp) -> Vec<(i64, u8)> {
    let snap = Snapshot::compile(a.project(), 0);
    snap.tracks
        .iter()
        .flat_map(|t| t.events.iter())
        .filter(|e| e.is_on())
        .map(|e| (e.tick.0, e.pitch))
        .collect()
}

#[test]
fn a_kit_names_its_lanes_and_a_synth_does_not() {
    let kit = kit_project();
    let lanes = kit.tracks[0].instrument.lanes();
    assert!(!lanes.is_chromatic(), "a kit must not be a keyboard");
    assert_eq!(lanes.name(0), "Kick");
    assert_eq!(lanes.pitch(0), Some(36));
    assert_eq!(
        kit.tracks[0].instrument.lane_name(42),
        Some("Closed hat"),
        "the instrument must name its own pitches"
    );
    assert_eq!(kit.tracks[0].instrument.lane_name(60), None);

    let synth = starter_project();
    assert!(synth.tracks[0].instrument.lanes().is_chromatic());
    assert_eq!(synth.tracks[0].instrument.lane_name(60), None);
}

/// The piano roll must *show* the pad names, not merely be able to look them up. Asserting
/// on the painted text is the only version of this that would fail if the view kept
/// drawing a keyboard while the model happily answered questions nobody asked it.
#[test]
fn the_piano_roll_draws_pad_names_for_a_kit_and_note_names_for_a_synth() {
    let painted = |mut a: CantusApp| {
        let ctx = ctx();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 700.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| views::piano_roll::show(&mut a, ui));
            },
        );
        out.textures_delta.clear();
        painted_text(&out.shapes)
    };

    let drums = painted(kit_app());
    assert!(
        drums.contains("Kick"),
        "no pad names in the key strip: {drums}"
    );
    assert!(drums.contains("Closed hat"), "missing a pad: {drums}");
    assert!(
        !drums.contains("C4") && !drums.contains("C3"),
        "a kit is still drawing a keyboard: {drums}"
    );

    let synth = painted(app(starter_project()));
    assert!(
        synth.contains("C4") || synth.contains("C5") || synth.contains("C3"),
        "a synth stopped drawing its keyboard: {synth}"
    );
    assert!(!synth.contains("Kick"));
}

/// A clip may hold notes the kit has no pad for — a track that used to be a synth. Those
/// must still have a row, or changing an instrument would make music *invisible*, which is
/// indistinguishable from losing it.
#[test]
fn a_note_the_kit_cannot_play_still_gets_a_row() {
    let mut p = kit_project();
    let note = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
    p.tracks[0].clips[0].notes.push(note);
    let clip = p.tracks[0].clips[0].clone();
    let axis = views::piano_roll::Axis::build(&p.tracks[0].instrument, &clip);
    let lane = axis
        .lane_of(60)
        .expect("a note with no pad must still have a row");
    assert_eq!(
        axis.name(lane),
        "C4",
        "the extra row is named as a keyboard would"
    );
    assert_eq!(axis.len(), PulsusParams::default().pads.len() + 1);
}

/// The rows are the same units the vertical keys move in. On a keyboard a row is a
/// semitone; on a kit it is the next pad, and "an octave" is four pads — a row of a pad
/// grid — because neither a semitone nor an octave names anything on a drum kit.
#[test]
fn transposing_on_a_kit_moves_by_pads_not_by_semitones() {
    let mut a = kit_app();
    let (t, c) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    let note = a.bus.new_note(Ticks::ZERO, Ticks::SIXTEENTH, 36, 100);
    let id = note.id;
    assert!(a.exec(Command::InsertNotes {
        track: t,
        clip: c,
        notes: vec![note]
    }));
    a.selected_notes = vec![id];

    a.transpose_selection(1);
    assert_eq!(
        note_ons(&a),
        vec![(0, 37)],
        "one row up from the kick is the next pad, not pitch 37 by arithmetic accident"
    );
    a.transpose_selection(1);
    assert_eq!(note_ons(&a), vec![(0, 38)], "and again: the snare");
    // The octave keys move four pads, and the kit has seven, so from the snare (row 2)
    // that is row 6.
    a.transpose_selection(12);
    let pads = PulsusParams::default().pads;
    assert_eq!(
        note_ons(&a),
        vec![(0i64, pads[6].pitch)],
        "the octave keys must move four pads on a kit"
    );
    // And back down again, exactly.
    a.transpose_selection(-12);
    a.transpose_selection(-1);
    a.transpose_selection(-1);
    assert_eq!(note_ons(&a), vec![(0, 36)], "the move is not reversible");
}

/// Drive the rack by pointer, through the geometry it publishes, and assert on
/// `Snapshot::compile` — what the engine would play — rather than on the clip's fields.
/// `lane` is a lane of the track's axis, or `None` for a melodic track's single row.
fn click_step(a: &mut CantusApp, track: usize, lane: Option<usize>, step: usize, secondary: bool) {
    let ctx = ctx();
    let size = egui::vec2(1200.0, 500.0);
    let show = |a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| views::channel_rack::show(a, ui));
        })
        .textures_delta
        .clear();
    };
    // Lay it out, then ask it where the cell is. Recomputing the layout here would make
    // the test break the day the toolbar grows a control.
    show(a, &ctx, vec![]);
    let geo = a.rack.geometry().expect("the rack drew a frame");
    let row = match lane {
        Some(l) => geo.row_for_lane(track, l),
        None => geo.row_for_track(track),
    }
    .expect("the rack has a row for that lane");
    let at = geo.cell(row, step).center();
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: if secondary {
            egui::PointerButton::Secondary
        } else {
            egui::PointerButton::Primary
        },
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    show(a, &ctx, vec![egui::Event::PointerMoved(at)]);
    show(a, &ctx, vec![egui::Event::PointerMoved(at), button(true)]);
    show(a, &ctx, vec![egui::Event::PointerMoved(at), button(false)]);
    show(a, &ctx, vec![]);
}

/// The decision the whole rack rests on: a step **is** a note. Switching one on has to
/// change what the engine plays, and switching it off has to put the clip back exactly.
#[test]
fn a_step_is_a_note_and_switching_it_off_puts_the_clip_back() {
    let mut a = kit_app();
    let before = a.project().clone();
    assert!(note_ons(&a).is_empty());

    // Lane 0 of the kit is the kick — the lowest pad, drawn at the bottom.
    let pads = PulsusParams::default().pads;
    click_step(&mut a, 0, Some(0), 4, false);
    assert_eq!(
        note_ons(&a),
        vec![(Ticks::SIXTEENTH.0 * 4, pads[0].pitch)],
        "a step on the kick's row at step 4 is a kick a beat in"
    );

    click_step(&mut a, 0, Some(0), 4, true);
    assert!(note_ons(&a).is_empty(), "right-click did not switch it off");
    let mut now = a.project().clone();
    now.next_id = before.next_id;
    assert_eq!(now, before, "on then off is not the identity");

    // ...and it is one undo step each way.
    click_step(&mut a, 0, Some(0), 4, false);
    a.undo();
    assert!(note_ons(&a).is_empty(), "one step is not one undo step");
}

/// The rack and the piano roll are two editors for one clip, not two data models. Nothing
/// is converted when the view changes, so the project after a round trip is byte-identical
/// and the engine plays the same thing.
#[test]
fn switching_between_the_rack_and_the_piano_roll_converts_nothing() {
    let mut a = kit_app();
    click_step(&mut a, 0, Some(0), 0, false);
    click_step(&mut a, 0, Some(0), 8, false);
    let after_rack = a.project().clone();
    let played = note_ons(&a);
    assert_eq!(played.len(), 2);

    // Draw a frame of the piano roll over the same clip, then a frame of the rack again.
    let ctx = ctx();
    let frame = |a: &mut CantusApp, which: bool| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 700.0),
            )),
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    if which {
                        views::piano_roll::show(a, ui)
                    } else {
                        views::channel_rack::show(a, ui)
                    }
                });
        })
        .textures_delta
        .clear();
    };
    frame(&mut a, true);
    frame(&mut a, false);
    frame(&mut a, true);
    assert_eq!(*a.project(), after_rack, "looking at a clip changed it");
    assert_eq!(note_ons(&a), played);
}

/// A muted pad is silent and nothing else is, and the notes are still there — mute hides a
/// pad, it does not delete it.
#[test]
fn muting_a_pad_silences_that_pad_alone() {
    let mut a = kit_app();
    let (t, c) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    let pads = PulsusParams::default().pads;
    let notes: Vec<_> = [0usize, 2, 3]
        .iter()
        .map(|i| {
            a.bus
                .new_note(Ticks::ZERO, Ticks::SIXTEENTH, pads[*i].pitch, 100)
        })
        .collect();
    assert!(a.exec(Command::InsertNotes {
        track: t,
        clip: c,
        notes
    }));
    assert_eq!(note_ons(&a).len(), 3);

    assert!(a.exec(Command::SetInstrumentParam {
        track: t,
        param: InstrumentParam::Pulsus {
            pad: pads[2].id,
            field: PadField::Mute,
        },
        value: 1.0,
    }));
    let heard = note_ons(&a);
    assert_eq!(
        heard.len(),
        2,
        "muting one pad silenced {} of 3",
        3 - heard.len()
    );
    assert!(!heard.iter().any(|(_, p)| *p == pads[2].pitch));
    assert_eq!(
        a.project().track(t).unwrap().clip(c).unwrap().notes.len(),
        3,
        "muting a pad deleted its notes"
    );

    a.undo();
    assert_eq!(note_ons(&a).len(), 3, "unmuting did not bring the pad back");
}

/// Renaming a pad renames it everywhere at once, because the name lives in the kit — and
/// the piano roll's row picks it up without being told.
#[test]
fn renaming_a_pad_renames_the_row_in_the_piano_roll() {
    let mut a = kit_app();
    let t = a.project().tracks[0].id;
    assert!(a.exec(Command::SetPadName {
        track: t,
        pad: PadId(1),
        name: "Bombo".into(),
    }));
    let axis = a.note_axis();
    assert_eq!(axis.name(0), "Bombo");
    assert_eq!(
        a.project().track(t).unwrap().instrument.lane_name(36),
        Some("Bombo")
    );
    a.undo();
    assert_eq!(a.note_axis().name(0), "Kick");
}

/// ADR 0011's rule, applied to the shape it was written for: pads × steps, every frame.
///
/// A 32-pad kit over 256 steps is 8 192 cells. Drawn one painter call each they would cost
/// roughly ten times as much per item, so the guardrail is on the **shape count**: the
/// whole grid has to arrive as a handful of shapes, one of which is a mesh, not as
/// thousands. Asserting on shapes rather than on milliseconds keeps it honest on a loaded
/// machine — a timing guardrail that fails when another build is running teaches people to
/// re-run instead of read (round 9).
#[test]
fn the_channel_racks_grid_is_one_mesh() {
    let mut p = Project::new("wide");
    let mut t = p.make_track("Drums");
    // A kit at the widest Pulsus allows.
    let mut kit = PulsusParams::default();
    while kit.pads.len() < 32 {
        let mut pad = kit.pads[kit.pads.len() % 7].clone();
        pad.pitch = 60 + kit.pads.len() as u8;
        pad.name = format!("Pad {}", kit.pads.len());
        kit.pads.push(pad);
    }
    kit.pads.sort_by_key(|p| p.pitch);
    t.instrument = InstrumentSpec::Pulsus(kit);
    let mut clip = p.make_clip("Long", Ticks::ZERO, Ticks::beats(64));
    for i in 0..256i64 {
        clip.notes
            .push(p.make_note(Ticks::SIXTEENTH * i, Ticks::SIXTEENTH, 36, 100));
    }
    t.clips.push(clip);
    p.tracks.push(t);

    let mut a = app(p);
    let (tid, cid) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    a.select_clip(tid, cid);
    a.rack.fit = false;
    a.rack.step_w = 6.0;
    a.rack.row_h = 14.0;

    let ctx = ctx();
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| views::channel_rack::show(&mut a, ui));
        },
    );
    out.textures_delta.clear();
    let geo = a.rack.geometry().expect("the rack drew a frame");
    assert!(
        geo.rows.len() >= 33,
        "the wide kit lost its rows: {}",
        geo.rows.len()
    );
    assert!(
        geo.steps >= 256,
        "the long clip lost its steps: {}",
        geo.steps
    );

    fn count(shape: &egui::Shape, rects: &mut usize, meshes: &mut usize) {
        match shape {
            egui::Shape::Rect(_) => *rects += 1,
            egui::Shape::Mesh(_) => *meshes += 1,
            egui::Shape::Vec(v) => v.iter().for_each(|s| count(s, rects, meshes)),
            _ => {}
        }
    }
    let (mut rects, mut meshes) = (0, 0);
    for s in &out.shapes {
        count(&s.shape, &mut rects, &mut meshes);
    }
    assert!(meshes >= 1, "the grid is not batched into a mesh at all");
    // Generous, and still two orders of magnitude below one rectangle per cell.
    assert!(
        rects < 900,
        "the grid is drawing {rects} rectangles: one per cell would be tens of thousands"
    );
}

/// A muted pad still draws, a fine step grid still draws its lit steps: the level-of-detail
/// rule may drop the empty cells, never the music.
#[test]
fn the_lit_steps_survive_the_level_of_detail_rule() {
    let mut a = kit_app();
    let (t, c) = (a.project().tracks[0].id, a.project().tracks[0].clips[0].id);
    let notes: Vec<_> = (0..16i64)
        .map(|i| {
            a.bus
                .new_note(Ticks::SIXTEENTH * i, Ticks::SIXTEENTH, 36, 100)
        })
        .collect();
    assert!(a.exec(Command::InsertNotes {
        track: t,
        clip: c,
        notes
    }));

    let triangles = |a: &mut CantusApp, step_w: f32| {
        a.rack.fit = false;
        a.rack.step_w = step_w;
        let ctx = ctx();
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| views::channel_rack::show(a, ui));
            },
        );
        out.textures_delta.clear();
        let mut n = 0usize;
        for prim in ctx.tessellate(out.shapes, 1.0) {
            if let egui::epaint::Primitive::Mesh(m) = prim.primitive {
                n += m.indices.len() / 3;
            }
        }
        n
    };
    // Wide cells draw the empty ones too; narrow cells stop, which is the whole point.
    let wide = triangles(&mut a, 30.0);
    let narrow = triangles(&mut a, 3.5);
    assert!(
        narrow < wide,
        "the level-of-detail rule is not doing anything: {narrow} against {wide}"
    );
    assert!(narrow > 0, "the narrow grid drew nothing at all");
}

// ---- the playlist: a time selection, and the shape of a track -----------------------

/// A three-track project so track order and heights have something to be relative to.
fn three_tracks() -> Project {
    let mut p = starter_project();
    for name in ["Second", "Third"] {
        let mut t = p.make_track(name);
        let clip = p.make_clip("c", Ticks::beats(4), Ticks::beats(4));
        t.clips.push(clip);
        p.tracks.push(t);
    }
    p
}

/// The ruler is two lanes. Dragging the lower one selects time; dragging the upper one
/// sets the loop. Both are aimed through the geometry the view publishes.
#[test]
fn dragging_the_ruler_selects_time_and_dragging_the_loop_lane_sets_the_loop() {
    let mut a = app(three_tracks());
    let before_loop = a.project().loop_region.clone();
    drag_the_arrangement(
        &mut a,
        |g| {
            let y = (g.loop_lane_bottom + g.body_top) * 0.5;
            (egui::pos2(g.x(4.0), y), egui::pos2(g.x(12.0), y))
        },
        egui::Modifiers::NONE,
    );
    assert_eq!(
        a.time_selection,
        Some((Ticks::beats(4), Ticks::beats(12))),
        "dragging the selection lane did not select bars 2 to 4"
    );
    assert_eq!(
        a.project().loop_region,
        before_loop,
        "a time selection must not move the loop"
    );

    drag_the_arrangement(
        &mut a,
        |g| {
            let y = (g.ruler_top + g.loop_lane_bottom) * 0.5;
            (egui::pos2(g.x(8.0), y), egui::pos2(g.x(16.0), y))
        },
        egui::Modifiers::NONE,
    );
    assert_eq!(
        a.project().loop_region,
        Some(LoopRegion {
            start: Ticks::beats(8),
            end: Ticks::beats(16)
        }),
        "dragging the loop lane did not set the loop"
    );
    assert_eq!(
        a.time_selection,
        Some((Ticks::beats(4), Ticks::beats(12))),
        "setting the loop must not disturb the time selection"
    );
    // Dragging backwards selects the same range, and a zero-width drag selects nothing.
    drag_the_arrangement(
        &mut a,
        |g| {
            let y = (g.loop_lane_bottom + g.body_top) * 0.5;
            (egui::pos2(g.x(20.0), y), egui::pos2(g.x(16.0), y))
        },
        egui::Modifiers::NONE,
    );
    assert_eq!(a.time_selection, Some((Ticks::beats(16), Ticks::beats(20))));
    a.set_time_selection(Ticks::beats(3), Ticks::beats(3));
    assert_eq!(
        a.time_selection, None,
        "an empty selection must be no selection"
    );
}

/// What hangs off the selection today: the loop follows it, and the view zooms to it.
#[test]
fn loop_to_selection_and_zoom_to_selection_hang_off_the_time_selection() {
    use cantus_ui::keymap::Action;
    let mut a = app(three_tracks());
    // Nothing selected: both are no-ops, not panics and not changes.
    let before = a.project().clone();
    a.run_action(Action::LoopToSelection);
    a.run_action(Action::ZoomToSelection);
    assert_eq!(*a.project(), before);

    a.set_time_selection(Ticks::beats(8), Ticks::beats(24));
    a.run_action(Action::LoopToSelection);
    // The loop region is part of what a project *renders* while looping is on, so the
    // snapshot is the thing to ask.
    let snap = Snapshot::compile(a.project(), 0);
    assert_eq!(
        snap.loop_region,
        Some(LoopRegion {
            start: Ticks::beats(8),
            end: Ticks::beats(24)
        }),
        "loop-to-selection must set the region *and* switch looping on"
    );
    a.undo();
    assert_eq!(
        Snapshot::compile(a.project(), 0).loop_region,
        None,
        "loop-to-selection must be one undo step"
    );

    // Zoom: after it, both ends of the selection are inside the drawn timeline.
    run_frames(&mut a, 1);
    a.run_action(Action::ZoomToSelection);
    run_frames(&mut a, 1);
    let g = a.arrangement.geometry().expect("timeline drawn");
    let (x0, x1) = (g.x(8.0), g.x(24.0));
    assert!(
        x0 >= g.left - 1.0,
        "selection start is off screen after zoom: {x0}"
    );
    assert!(
        x1 <= g.left + g.width + 1.0,
        "selection end is off screen: {x1}"
    );
    assert!(
        (x1 - x0) > g.width * 0.8,
        "zoom-to-selection did not fill the view: {} of {}",
        x1 - x0,
        g.width
    );
}

/// Escape in the arrangement clears the time selection along with the clips.
#[test]
fn escape_in_the_arrangement_clears_the_time_selection() {
    use cantus_ui::keymap::Action;
    let mut a = app(three_tracks());
    a.focus_arrangement();
    a.set_time_selection(Ticks::ZERO, Ticks::beats(4));
    a.run_action(Action::ClearSelection);
    assert_eq!(a.time_selection, None);
}

/// A track's height is a project field, it is undoable, and it moves every row below it
/// by exactly what the geometry says — headers and lanes from the same table.
#[test]
fn per_track_height_is_stored_undoable_and_moves_the_rows_below() {
    let mut a = app(three_tracks());
    run_frames(&mut a, 1);
    let g0 = a.arrangement.geometry().unwrap();
    assert_eq!(g0.rows.len(), 3);
    let base = g0.rows[0].height;
    let t0 = a.project().tracks[0].id;

    assert!(a.exec(Command::SetTrackHeight {
        id: t0,
        height: 2.0
    }));
    run_frames(&mut a, 1);
    let g1 = a.arrangement.geometry().unwrap();
    assert!(
        (g1.rows[0].height - base * 2.0).abs() <= 1.0,
        "row 0 is {} for a height of 2 × {base}",
        g1.rows[0].height
    );
    assert!(
        (g1.rows[1].top - (g0.rows[1].top + base)).abs() <= 1.0,
        "row 1 did not move down by the extra height"
    );
    assert_eq!(
        g1.rows[2].height, g0.rows[2].height,
        "other rows kept their height"
    );

    // Out-of-range values are clamped, not stored: a hand-edited file cannot make a
    // two-pixel track.
    assert!(a.exec(Command::SetTrackHeight {
        id: t0,
        height: 40.0
    }));
    assert_eq!(a.project().tracks[0].height, MAX_TRACK_HEIGHT);
    a.undo();
    a.undo();
    assert_eq!(a.project().tracks[0].height, 1.0, "height is not undoable");

    // And it survives the file.
    assert!(a.exec(Command::SetTrackHeight {
        id: t0,
        height: 0.6
    }));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("tall.cantus");
    a.save_to_path(&path);
    a.new_project();
    a.open(&path);
    assert_eq!(
        a.project().tracks[0].height,
        0.6,
        "height did not travel with the file"
    );
}

/// Dragging the divider under a header resizes that track, through the same command.
#[test]
fn dragging_a_header_divider_resizes_the_track() {
    let mut a = app(three_tracks());
    let t0 = a.project().tracks[0].id;
    drag_the_arrangement(
        &mut a,
        |g| {
            let x = g.left - 60.0; // in the header column
            let y = g.rows[0].top + g.rows[0].height; // on the divider
            (egui::pos2(x, y), egui::pos2(x, y + g.rows[0].height))
        },
        egui::Modifiers::NONE,
    );
    let h = a.project().track(t0).unwrap().height();
    assert!(
        (h - 2.0).abs() < 0.15,
        "dragging the divider down by one row should double the track: got {h}"
    );
    // One gesture, one undo step.
    a.undo();
    assert_eq!(a.project().track(t0).unwrap().height, 1.0);
}

/// Dragging a header itself reorders the tracks, which the engine sees as a different
/// track order in the snapshot.
#[test]
fn dragging_a_header_reorders_the_tracks() {
    let mut a = app(three_tracks());
    let before: Vec<TrackId> = a.project().tracks.iter().map(|t| t.id).collect();
    drag_the_arrangement(
        &mut a,
        |g| {
            let x = g.left - 60.0;
            // From the middle of row 0 to the lower half of row 2: land after it.
            (
                egui::pos2(x, g.row_y(0)),
                egui::pos2(x, g.rows[2].top + g.rows[2].height * 0.9),
            )
        },
        egui::Modifiers::NONE,
    );
    let after: Vec<TrackId> = Snapshot::compile(a.project(), 0)
        .tracks
        .iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(
        after,
        vec![before[1], before[2], before[0]],
        "the first track should now be last"
    );
    a.undo();
    let undone: Vec<TrackId> = a.project().tracks.iter().map(|t| t.id).collect();
    assert_eq!(undone, before);
}

#[test]
fn ctrl_arrows_step_through_the_tracks() {
    use cantus_ui::keymap::{Action, Binding, Keymap};
    let mut a = app(three_tracks());
    let ids: Vec<TrackId> = a.project().tracks.iter().map(|t| t.id).collect();
    a.select_track(ids[0]);
    a.run_action(Action::SelectNextTrack);
    assert_eq!(a.selected_track, Some(ids[1]));
    a.run_action(Action::SelectNextTrack);
    a.run_action(Action::SelectNextTrack);
    assert_eq!(
        a.selected_track,
        Some(ids[2]),
        "stepping past the end stays put"
    );
    a.run_action(Action::SelectPrevTrack);
    assert_eq!(a.selected_track, Some(ids[1]));
    // And they are bound to Ctrl+arrows, which must not collide with the bare arrows.
    let map = Keymap::default();
    assert_eq!(
        map.bindings_for(Action::SelectNextTrack),
        &[Binding::cmd(egui::Key::ArrowDown)]
    );
    assert!(map
        .bindings_for(Action::TransposeDown)
        .contains(&Binding::key(egui::Key::ArrowDown)));
}

/// The header lost its mixer strip and the inspector gained it. Asserted on what is
/// painted: a header that still drew a dB readout would pass any test on the model.
#[test]
fn the_fader_left_the_track_header_for_the_inspector() {
    let mut a = app(three_tracks());
    let t0 = a.project().tracks[0].id;
    a.select_track(t0);
    let ctx = ctx();
    let paint = |a: &mut CantusApp, arrangement: bool| {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 700.0),
                )),
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        if arrangement {
                            views::arrangement::show(a, ui)
                        } else {
                            views::inspector::show(a, ui)
                        }
                    });
            },
        );
        out.textures_delta.clear();
        painted_text(&out.shapes)
    };
    let header = paint(&mut a, true);
    assert!(
        !header.contains("dB"),
        "the track header is still drawing a fader readout: {header}"
    );
    let inspector = paint(&mut a, false);
    // The track's own level, named as the track's so it never reads as the strip's
    // fader a second time (ADR 0024 §7).
    assert!(
        inspector.contains(&cantus_ui::i18n::t("inspector.track_level")),
        "no track level in the inspector: {inspector}"
    );
    // A value's unit is set after its digits in a smaller face (ADR 0024 §3), so the
    // galley holds "+0.0" and "dB" without the space between them.
    assert!(
        inspector.contains("+0.0") && inspector.contains("Strip 1"),
        "the inspector's channel is missing its level or its route: {inspector}"
    );
}

/// `None` is "derive from the instrument", and it must keep deriving after the instrument
/// changes — which a stored value would not.
#[test]
fn a_track_icon_follows_the_instrument_until_one_is_chosen() {
    let mut a = app(starter_project());
    let t = a.project().tracks[0].id;
    assert_eq!(a.project().tracks[0].icon(), TrackIcon::Synth);
    let mut kit_track = a.project().tracks[0].clone();
    kit_track.instrument = InstrumentSpec::Pulsus(PulsusParams::default());
    assert_eq!(kit_track.icon(), TrackIcon::Drums);
    assert!(a.exec(Command::SetTrackIcon {
        id: t,
        icon: Some(TrackIcon::Vocal)
    }));
    assert_eq!(a.project().tracks[0].icon(), TrackIcon::Vocal);
    a.undo();
    assert_eq!(a.project().tracks[0].icon(), TrackIcon::Synth);
}

/// Every arrangement action has a key, for the same reason every clip action does.
#[test]
fn every_arrangement_action_has_a_key_bound_to_it() {
    use cantus_ui::keymap::{Action, Category, Keymap};
    let map = Keymap::default();
    let unbound: Vec<&str> = Action::ALL
        .into_iter()
        .filter(|a| a.category() == Category::Arrangement || a.category() == Category::View)
        .filter(|a| map.bindings_for(*a).is_empty())
        .map(|a| a.id())
        .collect();
    assert!(
        unbound.is_empty(),
        "arrangement or view actions with no shortcut: {unbound:?}"
    );
}

// ---- the rack across every track --------------------------------------------------

/// The rack lists every track. A melodic track is one row, a one-pad kit is one row, and a
/// kit with several pads is a heading plus a row per pad — and the strip number on each is
/// the track's position, which is the mixer strip it lands on today.
#[test]
fn the_rack_lists_every_track_and_expands_a_kit_into_its_pads() {
    let mut a = app(welcome_project());
    run_frames(&mut a, 1);
    let ctx = ctx();
    let mut out = ctx.run_ui(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 700.0),
            )),
            ..Default::default()
        },
        |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| views::channel_rack::show(&mut a, ui));
        },
    );
    out.textures_delta.clear();
    let geo = a.rack.geometry().expect("the rack drew a frame");
    // Keys (1 row), Kick (1), Snare (1), Hi-hat (heading + 2 pads) = 6.
    assert_eq!(geo.rows.len(), 6, "{:?}", geo.rows);
    assert_eq!(
        geo.row_for_track(0),
        Some(0),
        "the keys track is one editable row"
    );
    assert_eq!(geo.row_for_track(1), Some(1));
    assert_eq!(geo.row_for_track(2), Some(2));
    assert_eq!(geo.rows[3].lane, None, "a two-pad kit gets a heading");
    assert_eq!(
        geo.row_for_lane(3, 1),
        Some(4),
        "the open hat is drawn above the closed"
    );
    assert_eq!(geo.row_for_lane(3, 0), Some(5));
    let text = painted_text(&out.shapes);
    for name in ["Keys", "Kick", "Snare", "Hi-hat", "Closed hat", "Open hat"] {
        assert!(text.contains(name), "{name} is not in the rack: {text}");
    }
}

/// A step on another track's row edits *that* track's clip in the pattern's window, and a
/// step on a track with no clip there creates one — in one undo step.
#[test]
fn a_step_on_another_channel_edits_that_channel_and_creates_its_clip_if_needed() {
    let mut a = app(welcome_project());
    // Select the keys clip: the pattern is its window, two bars.
    let keys = a.project().tracks[0].id;
    let keys_clip = a.project().tracks[0].clips[0].id;
    a.select_clip(keys, keys_clip);
    let kick = a.project().tracks[1].id;
    let before: Vec<_> = Snapshot::compile(a.project(), 0)
        .track(kick)
        .unwrap()
        .events
        .iter()
        .filter(|e| e.is_on())
        .map(|e| e.tick)
        .collect();

    // Step 2 of the kick's row: an off-beat kick that is not in the groove yet.
    click_step(&mut a, 1, Some(0), 2, false);
    let after: Vec<_> = Snapshot::compile(a.project(), 0)
        .track(kick)
        .unwrap()
        .events
        .iter()
        .filter(|e| e.is_on())
        .map(|e| e.tick)
        .collect();
    assert_eq!(
        after.len(),
        before.len() + 2,
        "one new hit per repetition of the pattern"
    );
    assert!(after.contains(&(Ticks::SIXTEENTH * 2)));
    assert_eq!(
        a.selected_clip,
        Some(keys_clip),
        "editing another channel must not change the selection"
    );

    // A brand-new track has no clip in the window: the first step makes one.
    a.add_track();
    let fresh = a.project().tracks[4].id;
    a.select_clip(keys, keys_clip);
    assert!(a.project().track(fresh).unwrap().clips.is_empty());
    click_step(&mut a, 4, None, 6, false);
    let clips = &a.project().track(fresh).unwrap().clips;
    assert_eq!(clips.len(), 1, "the first step should have created a clip");
    assert_eq!(clips[0].start, Ticks::ZERO);
    assert_eq!(
        clips[0].content_length,
        Some(Ticks::beats(8)),
        "it repeats like the pattern"
    );
    let played: Vec<_> = Snapshot::compile(a.project(), 0)
        .track(fresh)
        .unwrap()
        .events
        .iter()
        .filter(|e| e.is_on())
        .map(|e| (e.tick, e.pitch))
        .collect();
    assert_eq!(
        played[0],
        (Ticks::SIXTEENTH * 6, 60),
        "a melodic row's step is middle C"
    );
    a.undo();
    assert!(
        a.project().track(fresh).unwrap().clips.is_empty(),
        "clip and note must be one undo step"
    );
}

/// The project a session opens with plays a groove: kick, snare and hats on their own
/// tracks, each landing on its own mixer strip, and it renders with headroom.
#[test]
fn the_welcome_project_opens_on_a_groove() {
    let p = welcome_project();
    assert_eq!(p.tracks.len(), 4);
    let names: Vec<&str> = p.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, ["Keys", "Kick", "Snare", "Hi-hat"]);
    for t in &p.tracks[1..] {
        assert!(t.instrument.pulsus().is_some(), "{} is not a kit", t.name);
        assert_eq!(t.icon(), TrackIcon::Drums);
    }
    assert!(p.loop_enabled, "the groove should loop out of the box");
    let snap = Snapshot::compile(&p, 0);
    for t in &snap.tracks {
        assert!(!t.events.is_empty(), "{} is silent", t.name);
    }
    // The hats share a choke group, or the open hat would ring over the closed one.
    let hats = p.tracks[3].instrument.pulsus().unwrap();
    assert_eq!(hats.pads.len(), 2);
    assert_eq!(hats.pads[0].choke, hats.pads[1].choke);
    assert_ne!(hats.pads[0].choke, 0);

    let rendered = cantus_engine::render(
        std::sync::Arc::new(snap),
        &cantus_engine::RenderOptions {
            tail_seconds: 0.2,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        rendered.peak > 0.2,
        "the groove is too quiet: {}",
        rendered.peak
    );
    assert!(rendered.peak < 0.98, "the groove clips: {}", rendered.peak);
}

/// A track becomes a kit and back, keeping its notes; pads are added and removed; a file
/// dropped on a pad makes it a sample pad — each of them one undo step.
#[test]
fn a_track_can_become_a_kit_and_its_pads_can_come_and_go() {
    let mut a = app(starter_project());
    let t = a.project().tracks[0].id;
    let notes_before = note_ons(&a);
    assert!(a.project().tracks[0].instrument.pulsus().is_none());

    a.set_instrument(t, InstrumentSpec::Pulsus(PulsusParams::default()));
    let kit = a.project().tracks[0].instrument.pulsus().unwrap();
    assert_eq!(kit.pads.len(), 7);
    assert_eq!(
        kit.pads[0].name, "Kick",
        "default pads are named in the interface language"
    );
    // The melody is still there, as rows the kit cannot play but the editor still shows.
    assert_eq!(a.note_axis().len(), 7 + 4);
    assert_eq!(
        note_ons(&a),
        notes_before,
        "changing instrument must not move a note"
    );

    a.add_pad(t, PadVoice::Sample);
    let kit = a.project().tracks[0].instrument.pulsus().unwrap();
    assert_eq!(kit.pads.len(), 8);
    assert_eq!(kit.pads[7].pitch, 51, "a new pad lands above the highest");
    assert_eq!(kit.pads[7].voice, PadVoice::Sample);
    assert_eq!(a.rack.selected_pad, 7, "the new pad is selected");

    a.remove_pad(t, 7);
    assert_eq!(
        a.project().tracks[0]
            .instrument
            .pulsus()
            .unwrap()
            .pads
            .len(),
        7
    );
    a.undo();
    assert_eq!(
        a.project().tracks[0]
            .instrument
            .pulsus()
            .unwrap()
            .pads
            .len(),
        8
    );
    a.undo();
    assert_eq!(
        a.project().tracks[0]
            .instrument
            .pulsus()
            .unwrap()
            .pads
            .len(),
        7
    );
    a.undo();
    assert!(
        a.project().tracks[0].instrument.firmus().is_some(),
        "back to the synth"
    );

    // A file on a pad: voice and path in one step.
    a.set_instrument(t, InstrumentSpec::Pulsus(PulsusParams::default()));
    a.set_pad_sample(t, 0, Some("C:/samples/kick.wav".into()));
    let pad = &a.project().tracks[0].instrument.pulsus().unwrap().pads[0];
    assert_eq!(pad.voice, PadVoice::Sample);
    assert_eq!(pad.sample.as_deref(), Some("C:/samples/kick.wav"));
    a.undo();
    let pad = &a.project().tracks[0].instrument.pulsus().unwrap().pads[0];
    assert_eq!(pad.voice, PadVoice::Kick);
    assert_eq!(pad.sample, None, "voice and file must undo together");
}

/// Sends, routing of a selection, solo, EQ and the slot mix all reach the project through
/// the app as commands — refused routes included — and the engine sees them resolved.
#[test]
fn sends_routing_solo_and_eq_go_through_the_app() {
    let mut a = app(three_tracks());
    let p = a.project();
    let (s0, s1, s2) = (
        p.tracks[0].strip.unwrap(),
        p.tracks[1].strip.unwrap(),
        p.tracks[2].strip.unwrap(),
    );
    let aux = p.strips[5].id;
    // A send from strip 1 into an empty strip.
    a.set_strip_send(s0, Send::new(aux));
    a.end_gesture();
    assert_eq!(a.project().strip(s0).unwrap().sends, vec![Send::new(aux)]);
    // The engine sees it, and the aux is ordered after its source.
    let snap = Snapshot::compile(a.project(), 0);
    assert_eq!(snap.strips[0].sends, vec![(5, 1.0)]);
    let pos = |i: usize| snap.strip_order.iter().position(|x| *x == i).unwrap();
    assert!(
        pos(0) < pos(5),
        "a send orders the strips like an output does"
    );
    // A send back the other way would loop: refused, reported, nothing left behind.
    a.set_strip_send(aux, Send::new(s0));
    a.end_gesture();
    assert!(a.project().strip(aux).unwrap().sends.is_empty());
    // Switching a send off keeps its level; removing it takes it away.
    a.set_strip_send(
        s0,
        Send {
            to: aux,
            level: 0.3,
            enabled: false,
        },
    );
    a.end_gesture();
    assert!(Snapshot::compile(a.project(), 0).strips[0].sends.is_empty());
    a.remove_strip_send(s0, aux);
    assert!(a.project().strip(s0).unwrap().sends.is_empty());

    // FL's *route selected to this track*, on a multi-selection: one undo step.
    a.select_strip(s1);
    a.toggle_strip_selected(s2);
    assert_eq!(a.strip_selection(), vec![s1, s2]);
    assert!(a.route_strips_to(&[s1, s2], Some(aux)));
    assert_eq!(a.project().strip(s1).unwrap().output, Some(aux));
    assert_eq!(a.project().strip(s2).unwrap().output, Some(aux));
    a.undo();
    assert_eq!(a.project().strip(s1).unwrap().output, None);
    assert_eq!(a.project().strip(s2).unwrap().output, None);
    assert!(a.send_strips_to(&[s1, s2], aux));
    assert!(a.project().strip(s1).unwrap().send_to(aux).is_some());
    assert!(a.project().strip(s2).unwrap().send_to(aux).is_some());

    // Solo is resolved before the audio thread: the soloed strip and the strip its send
    // reaches stay audible, the third strip does not.
    a.toggle_strip_solo(s1);
    let snap = Snapshot::compile(a.project(), 0);
    assert!(!snap.strips[1].muted, "the soloed strip");
    assert!(!snap.strips[5].muted, "its send's destination");
    assert!(snap.strips[0].muted, "an unconnected strip");
    a.clear_strip_solos();
    assert!(!a.project().any_strip_solo());

    // EQ, swap, FX switch and the slot mix.
    a.exec(Command::SetStripEq {
        id: s0,
        band: 1,
        field: EqField::Gain,
        value: 6.0,
    });
    assert_eq!(a.project().strip(s0).unwrap().eq.bands[1].gain_db, 6.0);
    a.add_effect(s0, EffectKind::Delay);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.exec(Command::SetEffectMix {
        strip: s0,
        effect: fx,
        mix: 0.25,
    });
    assert_eq!(a.project().strip(s0).unwrap().effects[0].mix, 0.25);
    a.exec(Command::SetStripFxEnabled {
        id: s0,
        enabled: false,
    });
    a.exec(Command::SetStripSwapChannels { id: s0, swap: true });
    let snap = Snapshot::compile(a.project(), 0);
    assert!(!snap.strips[0].fx_enabled && snap.strips[0].swap_channels);
}

/// A submix is FL's group in the mixer's one tree: a new strip right after the members,
/// fed by them, going to the master — created and undone as one step. The desk is
/// rearranged with the keyboard, and only an unused strip can be removed.
#[test]
fn a_submix_groups_the_selected_strips_and_strips_move_along_the_desk() {
    use cantus_ui::keymap::Action;
    let mut a = app(three_tracks());
    let (s0, s1) = (
        a.project().tracks[0].strip.unwrap(),
        a.project().tracks[1].strip.unwrap(),
    );
    let before = a.project().strips.len();
    let sub = a.create_submix(&[s0, s1]).expect("a submix");
    assert_eq!(a.project().strips.len(), before + 1);
    assert_eq!(
        a.project().strip_number(sub),
        Some(3),
        "right after the last member"
    );
    assert_eq!(a.project().strip(s0).unwrap().output, Some(sub));
    assert_eq!(a.project().strip(s1).unwrap().output, Some(sub));
    assert_eq!(a.project().strip(sub).unwrap().output, None);
    assert_eq!(a.selected_strip, Some(sub));
    a.undo();
    assert_eq!(a.project().strips.len(), before);
    assert_eq!(a.project().strip(s0).unwrap().output, None);

    a.select_strip(s0);
    a.run_action(Action::MoveStripRight);
    assert_eq!(a.project().strip_number(s0), Some(2));
    assert_eq!(a.project().strip_number(s1), Some(1));
    a.run_action(Action::MoveStripLeft);
    assert_eq!(a.project().strip_number(s0), Some(1));
    // The engine follows the numbering: the snapshot's strips are in desk order.
    a.run_action(Action::MoveStripRight);
    let snap = Snapshot::compile(a.project(), 0);
    assert_eq!(snap.strips[1].id, s0);
    assert_eq!(snap.tracks[0].strip, Some(1));
    a.undo();

    let empty = a.project().strips[10].id;
    a.remove_strip(empty);
    assert_eq!(a.project().strips.len(), before - 1);
    a.remove_strip(s0);
    assert_eq!(
        a.project().strips.len(),
        before - 1,
        "a strip a track feeds cannot be removed"
    );
}

/// Faders in a multi-selection move together, by the same amount, as one undo step.
#[test]
fn ganged_faders_move_together_as_one_undo_step() {
    let mut a = app(three_tracks());
    let (s0, s1, s2) = (
        a.project().tracks[0].strip.unwrap(),
        a.project().tracks[1].strip.unwrap(),
        a.project().tracks[2].strip.unwrap(),
    );
    a.exec(Command::SetStripGain {
        id: s1,
        gain_db: -10.0,
    });
    a.select_strip(s0);
    a.toggle_strip_selected(s1);
    let gain = |a: &CantusApp, s: StripId| a.project().strip(s).unwrap().gain_db;
    // Two frames of one drag.
    a.set_strip_gain(s0, -3.0);
    a.set_strip_gain(s0, -6.0);
    a.end_gesture();
    assert_eq!(gain(&a, s0), -6.0);
    assert!((gain(&a, s1) + 16.0).abs() < 1e-5, "moved by the same 6 dB");
    assert_eq!(gain(&a, s2), 0.0, "not selected, not moved");
    a.undo();
    assert_eq!(gain(&a, s0), 0.0);
    assert_eq!(gain(&a, s1), -10.0, "the whole gesture was one step");
    // A strip outside the selection moves alone.
    a.set_strip_gain(s2, -2.0);
    a.end_gesture();
    assert_eq!(gain(&a, s0), 0.0);
    assert_eq!(gain(&a, s2), -2.0);
}

/// The mixer paints the new controls: solo, swap, FX, the EQ bars, the sends.
#[test]
fn the_mixer_paints_solo_fx_eq_and_sends() {
    let mut a = app(starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    let aux = a.project().strips[3].id;
    a.set_strip_send(s0, Send::new(aux));
    a.end_gesture();
    a.toggle_strip_solo(s0);
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 1000.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| views::mixer::show(&mut a, ui));
    });
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    // The rows are named once, in the legend; "Solo off" shows while a strip is soloed.
    for want in ["| SENDS |", "| FX \u{b7} RECORD |", "| EQ |", "SOLO OFF"] {
        assert!(
            text.contains(want),
            "the mixer never painted {want:?}: {text}"
        );
    }
}

/// Paint the mixer once at a given size and return the text it drew.
fn paint_mixer(a: &mut CantusApp, width: f32) -> String {
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(width, 1000.0),
        )),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default().show(ui, |ui| views::mixer::show(a, ui));
    });
    out.textures_delta.clear();
    painted_text(&out.shapes)
}

/// A new desk pins its last three strips on the right: they are painted even when the
/// window is far too narrow for the middle, and they stay after the scrolling strips.
#[test]
fn the_right_dock_is_pinned_and_always_painted() {
    let mut a = app(starter_project());
    let text = paint_mixer(&mut a, 900.0);
    for n in ["| 14 |", "| 15 |", "| 16 |"] {
        assert!(text.contains(n), "docked strip {n} not painted: {text}");
    }
    assert!(
        text.find("| 1 |").unwrap() < text.find("| 14 |").unwrap(),
        "the middle comes before the right dock"
    );
    // Moving a strip to the left dock pins it beside the master, before strip 1.
    let s5 = a.project().strips[4].id;
    a.set_strips_dock(&[s5], Dock::Left);
    let text = paint_mixer(&mut a, 900.0);
    assert!(
        text.find("| 5 |").unwrap() < text.find("| 1 |").unwrap(),
        "a left-docked strip sits before the middle: {text}"
    );
    a.undo();
    assert_eq!(a.project().strips[4].dock, Dock::Middle);
}

/// A group gathers its members into one run behind a separator, folds them away, and
/// dissolves back into plain strips — each of those one undo step.
#[test]
fn groups_gather_fold_and_dissolve() {
    let mut a = app(starter_project());
    let ids: Vec<StripId> = a.project().strips.iter().map(|s| s.id).collect();
    // Strips 2, 5 and 7 into a group: they become 2, 3, 4.
    let g = a
        .create_strip_group(&[ids[1], ids[4], ids[6]])
        .expect("a group");
    let p = a.project();
    assert_eq!(p.strip_number(ids[1]), Some(2));
    assert_eq!(p.strip_number(ids[4]), Some(3));
    assert_eq!(p.strip_number(ids[6]), Some(4));
    assert_eq!(p.strips_in_group(g).count(), 3);
    let name = p.strip_group(g).unwrap().name.clone();
    let text = paint_mixer(&mut a, 1600.0);
    assert!(
        text.contains(&name),
        "the separator shows the group's name: {text}"
    );
    assert!(text.contains("| 3 |"));

    // Fold: the members are gone from the desk, the separator stays.
    a.exec(Command::SetStripGroupCollapsed {
        id: g,
        collapsed: true,
    });
    let text = paint_mixer(&mut a, 1600.0);
    assert!(text.contains(&name));
    // (The separator paints the member count, 3, so it is strips 2 and 4 that are
    // looked for.)
    assert!(!text.contains("| 2 |") && !text.contains("| 4 |"), "{text}");
    assert!(
        text.contains("| 5 |"),
        "strips outside the group are still there"
    );

    // Dissolve: plain strips again, in the order the group left them.
    a.dissolve_strip_group(g);
    assert!(a.project().strip_groups.is_empty());
    assert!(a.project().strips.iter().all(|s| s.group.is_none()));
    assert_eq!(a.project().strip_number(ids[4]), Some(3));
    a.undo();
    assert_eq!(a.project().strips_in_group(g).count(), 3);
    // Undo the fold and the creation: the desk is back as it was.
    a.undo();
    a.undo();
    assert!(a.project().strip_groups.is_empty());
    assert_eq!(a.project().strip_number(ids[4]), Some(5));
}

/// Compact shows the strip's rows; Wide adds each fader's value; Extra large the insert
/// slots — and every strip stays on the desk.
#[test]
fn width_modes_show_more_of_each_strip() {
    use cantus_ui::views::mixer::layout::WidthMode;
    let mut a = app(starter_project());
    let compact = paint_mixer(&mut a, 1600.0);
    assert!(compact.contains("| SENDS |"));
    assert!(!compact.contains("| VALUE |"), "{compact}");
    views::mixer::set_width_mode(&mut a, WidthMode::Wide);
    let wide = paint_mixer(&mut a, 1600.0);
    assert!(wide.contains("| VALUE |"), "{wide}");
    views::mixer::set_width_mode(&mut a, WidthMode::Xl);
    let xl = paint_mixer(&mut a, 1600.0);
    assert!(xl.contains("| INSERTS |"), "{xl}");
    assert!(compact.contains("| 16 |"), "every strip is still there");
}

/// Aequalis through the app: a band made and removed is one undo step each, and undo
/// leaves the effect's stored parameters exactly as they were — nothing, for a new EQ.
#[test]
fn eq_bands_are_made_and_removed_as_single_undo_steps() {
    use cantus_dsp::EqShape;
    let mut a = app(starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Eq);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    let spec = |a: &CantusApp| a.project().strip(s0).unwrap().effects[0].spec.clone();
    assert!(spec(&a).params.is_empty(), "a new EQ stores nothing");

    let b = a.add_eq_band(s0, fx, EqShape::Bell, 2_500.0, 4.5).unwrap();
    assert_eq!(b, 0);
    assert_eq!(spec(&a).get("b0.freq"), 2_500.0);
    assert_eq!(spec(&a).get("b0.gain"), 4.5);
    let lc = a.add_eq_band(s0, fx, EqShape::LowCut, 80.0, 0.0).unwrap();
    assert_eq!(lc, 1, "the next free number");
    assert_eq!(spec(&a).get("b1.slope"), 24.0);
    let bands = cantus_engine::eq_bands(&spec(&a));
    assert!(bands[0].active && bands[1].active && !bands[2].active);

    // Removing band 0 frees its number without renumbering band 1.
    a.remove_eq_band(s0, fx, 0);
    assert!(spec(&a).params.keys().all(|k| !k.starts_with("b0.")));
    assert_eq!(spec(&a).get("b1.freq"), 80.0);
    a.undo();
    assert_eq!(spec(&a).get("b0.gain"), 4.5, "undo brings the band back");
    a.undo();
    a.undo();
    assert!(
        spec(&a).params.is_empty(),
        "undo leaves the map exactly as it was"
    );

    // The engine reads it: a band on, and the snapshot carries it.
    a.add_eq_band(s0, fx, EqShape::HighShelf, 8_000.0, -3.0);
    let snap = Snapshot::compile(a.project(), 0);
    assert_eq!(snap.strips[0].effects[0].spec.get("b0.shape"), 2.0);
}

/// The editor window paints its display and the selected band's panel.
#[test]
fn the_eq_editor_paints_its_display_and_band_panel() {
    use cantus_dsp::EqShape;
    let mut a = app(starter_project());
    let s0 = a.project().tracks[0].strip.unwrap();
    a.add_effect(s0, EffectKind::Eq);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    a.add_eq_band(s0, fx, EqShape::Bell, 1_000.0, 6.0);
    a.open_eq_editor(s0, fx);
    let ctx = ctx();
    let mut text = String::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 900.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            cantus_ui::views::eq_editor::window(&mut a, ui.ctx());
        });
        out.textures_delta.clear();
        text = painted_text(&out.shapes);
    }
    // The frame sets the name in capitals, and a value's unit a hair from its number.
    let norm = |s: &str| s.to_lowercase().replace(' ', "");
    for want in [
        "Aequalis EQ",
        "Band 1",
        "1.0 kHz",
        "+6.0 dB",
        "Bell",
        "+18",
        "1 of 24 bands",
    ] {
        assert!(
            norm(&text).contains(&norm(want)),
            "the editor never painted {want:?}: {text}"
        );
    }
    a.close_eq_editor();
    assert!(a.eq_editor.open.is_none());
}

/// Comprimo through the app: the key is chosen from the desk, refused when it would loop,
/// undone like anything else; the editor paints its displays and names its style.
#[test]
fn the_compressor_is_keyed_from_the_desk_and_its_editor_explains_itself() {
    let mut a = app(three_tracks());
    let (s0, s1) = (
        a.project().tracks[0].strip.unwrap(),
        a.project().tracks[1].strip.unwrap(),
    );
    a.add_effect(s0, EffectKind::Compressor);
    let fx = a.project().strip(s0).unwrap().effects[0].id;
    assert!(a.exec(Command::SetEffectSidechain {
        strip: s0,
        effect: fx,
        source: Some(s1),
    }));
    assert_eq!(
        a.project().strip(s0).unwrap().effects[0].sidechain,
        Some(s1)
    );
    // The key runs from strip 2 to strip 1, so routing 2 into 1 goes the same way and is
    // fine, while routing 1 into 2 would loop.
    assert!(!a.exec(Command::SetStripOutput {
        id: s0,
        output: Some(s1),
    }));
    assert!(a.exec(Command::SetStripOutput {
        id: s1,
        output: Some(s0),
    }));
    a.undo();
    a.undo();
    assert_eq!(a.project().strip(s0).unwrap().effects[0].sidechain, None);
    // A strip something is keyed from cannot be removed out from under the key.
    let spare = a.project().strips[9].id;
    a.exec(Command::SetEffectSidechain {
        strip: s0,
        effect: fx,
        source: Some(spare),
    });
    a.remove_strip(spare);
    assert!(
        a.project().strip(spare).is_some(),
        "a key's strip was removed"
    );

    a.open_effect_editor(s0, fx);
    assert!(a.comp_editor.open.is_some() && a.eq_editor.open.is_none());
    let ctx = ctx();
    let mut text = String::new();
    for _ in 0..2 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 900.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            cantus_ui::views::comp_editor::window(&mut a, ui.ctx());
        });
        out.textures_delta.clear();
        text = painted_text(&out.shapes);
    }
    // The device frame sets names and labels in capitals, and a value's unit is set a
    // hair apart in a smaller face (no space in the painted text).
    let lower = text.to_lowercase().replace(' ', "");
    for want in [
        "Comprimo compressor",
        "Opto",
        "peak detector",
        "Threshold",
        "4.0:1",
        "-18.0 dB",
        "GR 0.0 dB",
    ] {
        assert!(
            lower.contains(&want.to_lowercase().replace(' ', "")),
            "the editor never painted {want:?}: {text}"
        );
    }
    a.close_comp_editor();
}
