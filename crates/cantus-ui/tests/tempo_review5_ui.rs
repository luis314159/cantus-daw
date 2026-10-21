//! Review 5 of the tempo track's interface (ADR 0060): the tempo holds while *audio*
//! records — undo and redo of a tempo step too, a tap says so, a DAWproject import stops
//! the take first, a MIDI file does not set the tempo then, a refused knob is sent back —
//! and moves freely during a notes-only pass; a tempo point drags by its place in the song
//! (a zoom mid-drag keeps it under the pointer); the ruler's metre editor says its bar; a
//! marker released out of view ends its drag; the *Bar* grid steps a bar of the metre.

mod support;

use cantus_core::controller::*;
use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::midi::Heard;
use cantus_engine::{EngineHandle, MidiEvent, Processor};
use cantus_ui::controllers::Scope;
use cantus_ui::keymap::Action;
use cantus_ui::{starter_project, views, CantusApp};
use egui::Pos2;
use std::time::Instant;
use support::app;

/// An app on a hand-driven engine with one input channel and an armed audio track, its
/// project in `dir`: `start_recording` begins an audio pass.
fn audio_rig(dir: &std::path::Path) -> (CantusApp, Processor, rtrb::Producer<f32>) {
    audio_rig_on(dir, starter_project())
}

fn audio_rig_on(
    dir: &std::path::Path,
    project: Project,
) -> (CantusApp, Processor, rtrb::Producer<f32>) {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let (h, mut proc) = EngineHandle::manual(48_000.0);
    let (ring, consumer) = rtrb::RingBuffer::<f32>::new(96_000);
    proc.attach_input(consumer, 1);
    let mut a = CantusApp::from_parts(Some(project), h, false);
    a.rec.demo_inputs = Some(vec![0.0]);
    a.path = Some(dir.join("song.cantus"));
    a.add_audio_track();
    a.sync_engine();
    (a, proc, ring)
}

fn recording_audio(a: &CantusApp) -> bool {
    a.rec.pass.as_ref().is_some_and(|p| p.audio)
}

fn cc(n: u8, v: u8) -> Heard {
    Heard {
        port: 0,
        event: MidiEvent {
            status: 0xB0,
            data1: n,
            data2: v,
        },
        at: Instant::now(),
    }
}

/// Findings 1, 2, 3, 10, 15, 21, 27: undo and redo of a tempo step while audio records
/// are refused, and said — Ctrl+Z through the keymap too; after the take they work.
#[test]
fn undo_and_redo_hold_the_tempo_while_audio_records() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig(dir.path());
    a.set_tempo_now(130.0, "first");
    a.end_gesture();
    a.set_tempo_now(140.0, "second");
    a.end_gesture();
    a.undo();
    assert_eq!(a.project().tempo_bpm, 130.0, "a redo step waits");
    a.start_recording();
    assert!(recording_audio(&a), "an audio pass");
    a.undo();
    assert_eq!(a.project().tempo_bpm, 130.0, "undo refused");
    assert!(a.status.text.contains("audio"), "said: {}", a.status.text);
    a.run_action(Action::Undo);
    assert_eq!(a.project().tempo_bpm, 130.0, "Ctrl+Z refused");
    a.redo();
    assert_eq!(a.project().tempo_bpm, 130.0, "redo refused");
    a.stop_recording();
    a.redo();
    assert_eq!(a.project().tempo_bpm, 140.0, "after the take");
}

/// Findings 6, 16: a notes-only pass has nothing laid through the map: the tempo moves.
#[test]
fn a_notes_only_pass_lets_the_tempo_move() {
    let mut a = app(starter_project());
    let bpm = a.project().tempo_bpm;
    a.start_recording();
    assert!(a.is_recording() && !recording_audio(&a), "a notes pass");
    a.set_tempo_now(bpm + 7.0, "tempo");
    assert_eq!(a.project().tempo_bpm, bpm + 7.0);
}

/// Findings 5, 14, 22: a tap while audio records says the tempo is held, not tapped.
#[test]
fn a_tap_while_audio_records_says_it_is_held() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig(dir.path());
    let bpm = a.project().tempo_bpm;
    a.start_recording();
    for i in 0..4 {
        views::transport::tap_at(&mut a, 10.0 + 0.5 * f64::from(i));
    }
    assert_eq!(a.project().tempo_bpm, bpm);
    assert!(a.status.text.contains("audio"), "said: {}", a.status.text);
}

/// Finding 4: a DAWproject imported while audio records stops the take first (it lands
/// in the song it was recorded into), then replaces the song.
#[test]
fn a_dawproject_import_stops_the_take_first() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig(dir.path());
    let mut other = starter_project();
    other.name = "Elsewhere".into();
    other.tempo_bpm = 87.0;
    let file = cantus_project::dawproject::export(&other, "0.1.0");
    a.start_recording();
    assert!(recording_audio(&a));
    assert!(views::interop::import_dawproject(
        &mut a,
        &file.bytes,
        "x.dawproject"
    ));
    assert!(!a.is_recording(), "the take stopped first");
    assert_eq!(a.project().name, "Elsewhere");
}

/// Finding 7: a MIDI file opened while audio records into an empty song does not offer
/// to set the tempo (its notes come in at the song's).
#[test]
fn a_midi_file_during_an_audio_take_keeps_the_tempo() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig_on(dir.path(), Project::new("empty"));
    let bytes = cantus_project::midi::export(&starter_project(), &Default::default()).unwrap();
    let path = dir.path().join("groove.mid");
    std::fs::write(&path, bytes).unwrap();
    a.start_recording();
    assert!(views::interop::empty_song(a.project()));
    assert!(!views::interop::sets_tempo_by_default(&a));
    views::interop::open_midi_import(&mut a, &path);
    assert!(!a.interop.midi_import.as_ref().unwrap().apply_tempo);
}

/// Finding 8: a tempo knob refused while audio records is sent the tempo back (its motor
/// fader or ring returns to it).
#[test]
fn a_refused_tempo_knob_is_sent_the_tempo_back() {
    let dir = tempfile::tempdir().unwrap();
    let (mut a, _proc, _ring) = audio_rig(dir.path());
    a.midi_port_names = vec!["Keys".into()];
    let source = MidiSource {
        port: PortRef {
            name: "Keys".into(),
            ordinal: 0,
        },
        channel: Some(0),
        control: Control::Cc(12),
    };
    let mut m = Mapping::new(
        MappingId(1),
        source,
        MappingTarget::Tempo,
        MapMode::Absolute,
    );
    m.takeover = Takeover::Jump;
    m.feedback = true;
    assert!(a.add_mapping(Scope::Project, m));
    a.controllers.feedback_log = Some(Vec::new());
    a.controller_input();
    let key = (Scope::Project, MappingId(1));
    assert!(a.controllers.sent.contains_key(&key), "the tempo was sent");
    a.start_recording();
    a.controller_heard(&[cc(12, 120)]);
    assert!(
        !a.controllers.sent.contains_key(&key),
        "to be sent again: the knob is not where the tempo is"
    );
}

fn arrangement_out(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) -> String {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(1400.0, 600.0),
        )),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| views::arrangement::show(a, ui));
    });
    out.textures_delta.clear();
    support::painted_text(&out.shapes)
}

fn arrangement(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let _ = arrangement_out(a, ctx, events);
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

fn keys_song() -> Project {
    let mut p = Project::new("song");
    let mut t = p.make_track("Keys");
    t.clips
        .push(p.make_clip("c", Ticks::ZERO, Ticks::beats(64)));
    p.tracks.push(t);
    p
}

/// Finding 17: a tempo point dragged while the view zooms stays under the pointer: the
/// drag holds where in the song it was grabbed, not a screen x.
#[test]
fn a_tempo_point_stays_under_the_pointer_through_a_zoom() {
    let mut p = keys_song();
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: false,
        },
        TempoPoint {
            tick: Ticks(7680 + 1000),
            bpm: 140.0,
            ramp: false,
        },
    ];
    let mut a = app(p);
    a.arrangement.snap = false;
    let ctx = support::ctx();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let at = a.arrangement.tempo.points_drawn()[1];
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(at)]);
    arrangement(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(at), button(at, true)],
    );
    let p1 = at + egui::vec2(30.0, 0.0);
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(p1)]);
    a.arrangement.px_per_beat *= 2.0;
    let p2 = p1 + egui::vec2(10.0, 0.0);
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(p2)]);
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(p2)]);
    let drawn = a.arrangement.tempo.points_drawn()[1];
    arrangement(&mut a, &ctx, vec![button(p2, false)]);
    assert!(
        (drawn.x - p2.x).abs() <= 1.5,
        "under the pointer: {} vs {}",
        drawn.x,
        p2.x
    );
}

/// Finding 18: the ruler's metre editor names the bar it edits.
#[test]
fn the_rulers_metre_editor_names_its_bar() {
    let mut a = app(keys_song());
    let ctx = support::ctx();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    views::arrangement::tempo::add_metre_change(&mut a, Ticks::beats(44));
    arrangement(&mut a, &ctx, vec![]);
    let text = arrangement_out(&mut a, &ctx, vec![]);
    assert!(
        text.to_lowercase().contains("time signature at bar 12"),
        "{text}"
    );
}

/// Finding 23: a marker dragged out of view and released there ends its drag: the next
/// drag of it is a step of its own.
#[test]
fn a_marker_released_out_of_view_ends_its_drag() {
    let mut a = app(keys_song());
    a.arrangement.px_per_beat = 30.0;
    a.arrangement.snap = false;
    let id = a.add_marker(Ticks::beats(8)).unwrap();
    let ctx = support::ctx();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let steps = a.bus.history().len();
    let tab = a.arrangement.markers.tabs()[0].1;
    let from = tab.center();
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(from)]);
    arrangement(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    let mut x = from.x;
    while x < 1500.0 {
        x += 40.0;
        arrangement(
            &mut a,
            &ctx,
            vec![egui::Event::PointerMoved(Pos2::new(x, from.y))],
        );
    }
    let out = Pos2::new(x, from.y);
    arrangement(&mut a, &ctx, vec![button(out, false)]);
    arrangement(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(Pos2::new(700.0, 300.0))],
    );
    assert_eq!(a.bus.history().len(), steps + 1, "the first drag");
    // Into view again, and dragged a little.
    let tick = a.project().marker(id).unwrap().tick;
    a.arrangement.scroll_beats = (tick.0 as f32 / 960.0) - 8.0;
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let tab = a.arrangement.markers.tabs()[0].1;
    let from = tab.center();
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(from)]);
    arrangement(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    let to = from + egui::vec2(60.0, 0.0);
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(to)]);
    arrangement(&mut a, &ctx, vec![button(to, false)]);
    assert_eq!(a.bus.history().len(), steps + 2, "a step of its own");
}

/// Finding 24: with the *Bar* grid, an arrow key moves a clip a bar of the metre (3/4
/// here), and a duplicate of notes lands a bar of it later.
#[test]
fn the_bar_grid_steps_a_bar_of_the_metre() {
    let mut p = keys_song();
    p.time_signature = TimeSignature::new(3, 4);
    let mut c = p.make_clip("waltz", Ticks(2880), Ticks(2880));
    let n = p.make_note(Ticks::ZERO, Ticks(480), 60, 100);
    let note = n.id;
    c.notes.push(n);
    let cid = c.id;
    p.tracks[0].clips.push(c);
    let tid = p.tracks[0].id;
    let mut a = app(p);
    a.arrangement.grid = Ticks::beats(4);
    a.set_clip_selection(vec![(tid, cid)]);
    a.focus_arrangement();
    a.run_action(Action::NudgeRight);
    let c = a.project().track(tid).unwrap().clip(cid).unwrap().clone();
    assert_eq!(c.start, Ticks(5760), "a bar of 3/4 on");
    a.selected_track = Some(tid);
    a.selected_clip = Some(cid);
    a.selected_notes = vec![note];
    a.piano_roll.grid = Ticks::beats(4);
    a.duplicate_selection();
    let c = a.project().track(tid).unwrap().clip(cid).unwrap().clone();
    let mut starts: Vec<Ticks> = c.notes.iter().map(|n| n.start).collect();
    starts.sort();
    assert_eq!(starts, vec![Ticks::ZERO, Ticks(2880)], "a bar of 3/4 later");
}
