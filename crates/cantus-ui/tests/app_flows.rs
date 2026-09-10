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
fn the_mixer_draws_faders_with_a_scale_and_a_readout() {
    // The mixer was the one view `run_frames` never touched, and the fader is most of
    // what it is made of. This renders it and reads back the text it painted.
    let mut a = app(starter_project());
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
    for want in ["+12", "-12", "-30", "-60", "-\u{221e}", "+0.0 dB"] {
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
        // Only the first vertical fader is this track's; the next one belongs to the
        // master strip and moves a different number.
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
        |a| a.project().track(track).unwrap().gain_db,
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
        "Interface size",           // ... and that the UI scale is offered
    ] {
        assert!(
            text.contains(expected),
            "no dialog painted {expected:?}; painted:
{text}"
        );
    }

    // The colour editor is tall enough to scroll the sections above out of view, so it
    // gets its own frame rather than being asserted alongside them.
    a.show_theme_editor = true;
    ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()))
        .textures_delta
        .clear();
    let mut out = ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()));
    out.textures_delta.clear();
    let text = painted_text(&out.shapes);
    // Not every token: there are twenty-two and the rest are below the fold, which is what
    // a scroll area is for. `theme::tests::every_colour_in_a_theme_is_editable_in_the_dialog`
    // is what guarantees none of them is missing.
    for expected in ["SURFACES", "Canvas", "Focus ring"] {
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

    // Alt-drag copies instead: the original stays, and the copy is independent.
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
    let mut a = app(starter_project());
    let id = a.project().tracks[0].id;
    a.exec(Command::SetTrackPan { id, pan: -0.5 });
    a.exec(Command::SetTrackWidth { id, width: 0.0 });
    a.exec(Command::SetTrackPhaseInvert { id, invert: true });
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
    for want in ["L50", "0%", "Mono", "\u{d8}", "-3"] {
        assert!(
            text.contains(want),
            "the mixer never painted {want:?}: {text}"
        );
    }
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
        text.contains("Clip"),
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
    for want in ["Arrangement", "Grid", "1 bar", "Snap", "1 selected"] {
        assert!(
            text.contains(want),
            "the toolbar never painted {want:?}: {text}"
        );
    }
    // The buttons themselves are painted shapes, not text: count the icon-sized
    // interactive rects instead, so the test fails if the row silently loses one.
    let buttons = out
        .shapes
        .iter()
        .filter(|s| {
            matches!(&s.shape, egui::Shape::Rect(r)
                if (r.rect.width() - cantus_ui::design::ICON_BUTTON.x).abs() < 0.5
                    && (r.rect.height() - cantus_ui::design::ICON_BUTTON.y).abs() < 0.5)
        })
        .count();
    assert!(
        buttons >= 1,
        "no icon buttons were painted in the arrangement toolbar"
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
