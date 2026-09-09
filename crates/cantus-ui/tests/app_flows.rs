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
    let ctx = egui::Context::default();
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
    let ctx = egui::Context::default();
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
