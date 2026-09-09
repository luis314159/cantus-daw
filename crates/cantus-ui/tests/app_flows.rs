//! Headless application tests: the glue between UI actions, the command bus, the project
//! file and the renderer, plus the ADR 0003 guardrail (dense views must render).

use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::views;
use cantus_ui::{starter_project, stress_project, CantusApp};
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
/// interaction code, and prints frame times for the stress project.
fn run_frames(a: &mut CantusApp, frames: usize) -> Duration {
    let ctx = ctx();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 1000.0),
        )),
        ..Default::default()
    };
    let mut worst = Duration::ZERO;
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
    }
    worst
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
    let worst = run_frames(&mut a, 5);
    eprintln!(
        "stress: worst frame {:.2} ms, notes drawn {}",
        worst.as_secs_f64() * 1e3,
        a.piano_roll_notes_drawn
    );
    assert!(a.piano_roll_notes_drawn > 100);
    // Debug builds are several times slower than release; keep a generous ceiling so
    // the test guards against pathological regressions, not against debug overhead.
    assert!(worst < Duration::from_millis(250), "frame took {worst:?}");
}

#[test]
fn transport_icons_are_painted_as_shapes() {
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
    let paths = out
        .shapes
        .iter()
        .filter(|s| matches!(s.shape, egui::Shape::Path(_)))
        .count();
    eprintln!(
        "transport frame: {} shapes, {} paths",
        out.shapes.len(),
        paths
    );
    assert!(
        paths >= 2,
        "play and to-start triangles must be painted (found {paths})"
    );
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
    a.add_effect(track, EffectKind::Delay);
    a.add_effect(track, EffectKind::Reverb);
    assert_eq!(a.project().track(track).unwrap().effects.len(), 2);
    a.sync_engine();
    assert_eq!(a.engine.known_effect_count(), 2);
    let fx = a.project().track(track).unwrap().effects[0].id;
    a.remove_effect(track, fx);
    assert_eq!(a.project().track(track).unwrap().effects.len(), 1);
    a.undo();
    assert_eq!(a.project().track(track).unwrap().effects.len(), 2);
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
    a.selected_notes = vec![first];
    a.copy_selection();
    assert_eq!(a.clipboard.len(), 1);
    let n_before = notes(&a).len();
    a.paste_clipboard();
    assert_eq!(notes(&a).len(), n_before + 1);
    a.cut_selection();
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
    use cantus_ui::layout;

    let mut a = app(starter_project());
    let dir = tempfile::tempdir().unwrap();
    // Never write to the user's real configuration directory from a test.
    a.layouts_path = layout::layouts_path(dir.path());
    let ctx = ctx();

    // Lay the window out once so the panels have sizes to capture.
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let frame = |a: &mut CantusApp| {
        let forced = a.layouts.forced(layout::INSPECTOR);
        let show = a.layouts.current.show_inspector;
        let mut drew_inspector = false;
        ctx.run_ui(input.clone(), |ui| {
            if show {
                drew_inspector = true;
                let p = egui::Panel::right(layout::panel_id(layout::INSPECTOR));
                match forced {
                    Some(w) => p.exact_size(w),
                    None => p.resizable(true).min_size(240.0),
                }
                .show(ui, |ui| views::inspector::show(a, ui));
            }
            egui::CentralPanel::default().show(ui, |ui| views::piano_roll::show(a, ui));
        })
        .textures_delta
        .clear();
        a.layouts.end_frame();
        drew_inspector
    };

    a.layouts.current.inspector = 320.0;
    a.layouts.force();
    assert!(frame(&mut a), "the inspector shows by default");

    // Save the shape under a name, switch the central view, then come back to it.
    a.central = cantus_ui::app::CentralView::Mixer;
    a.save_layout_as("Mixing", &ctx);
    assert!(a.layouts_path.exists(), "saving a layout writes the file");

    a.reset_layout();
    assert_eq!(a.central, cantus_ui::app::CentralView::PianoRoll);
    assert_eq!(a.layouts.current.name, layout::DEFAULT_NAME);

    a.apply_layout("Mixing");
    assert_eq!(a.central, cantus_ui::app::CentralView::Mixer);
    assert_eq!(
        a.layouts.forced(layout::INSPECTOR),
        Some(320.0),
        "switching back must restore the saved width"
    );
    frame(&mut a);

    // Hiding a panel really stops it being drawn.
    a.layouts.current.show_inspector = false;
    assert!(!frame(&mut a), "a hidden panel must not be laid out");

    // And everything survives a round trip through the file.
    a.store_layouts(&ctx);
    let reloaded = cantus_ui::layout::Layouts::load(&a.layouts_path);
    assert_eq!(reloaded.saved.len(), 1);
    assert_eq!(reloaded.saved[0].name, "Mixing");
    assert!(!reloaded.current.show_inspector);
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
    ] {
        assert!(
            text.contains(expected),
            "no dialog painted {expected:?}; painted:
{text}"
        );
    }

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
    a.piano_roll.top_pitch = 100.0;
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
