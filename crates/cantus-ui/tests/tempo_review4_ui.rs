//! Review 4 of the tempo track's interface (ADR 0060): the tempo stays while a take
//! records (the top bar, a command, a controller knob), a tempo point dragged straight up
//! keeps its place, *Add a metre change* in a narrow arrangement, a run of markers on one
//! tick that fits keeps every name whole, and a MIDI file dropped on a song that has only a
//! tempo track leaves its tempo track alone.

mod support;

use cantus_core::controller::*;
use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::midi::Heard;
use cantus_engine::MidiEvent;
use cantus_ui::controllers::Scope;
use cantus_ui::{views, CantusApp};
use egui::Pos2;
use std::time::Instant;
use support::app;

fn pt(tick: i64, bpm: f64) -> TempoPoint {
    TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp: false,
    }
}

fn context() -> egui::Context {
    support::ctx()
}

fn arrangement_in(a: &mut CantusApp, ctx: &egui::Context, width: f32, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            Pos2::ZERO,
            egui::vec2(width, 600.0),
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

fn arrangement(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    arrangement_in(a, ctx, 1400.0, events);
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
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

/// Findings 4, 13, 20, 42: while an audio take records, the tempo stays — the top bar, a command,
/// a knob mapped to the tempo — and says why; a pan knob in the same gesture still moves;
/// after the take the tempo can be changed again.
#[test]
fn the_tempo_stays_while_a_take_records() {
    // An audio take (review 5: a notes-only pass leaves the tempo free): a hand-driven
    // engine with one input channel, an armed audio track.
    let dir = tempfile::tempdir().unwrap();
    let (h, mut proc) = cantus_engine::EngineHandle::manual(48_000.0);
    let (_ring, consumer) = rtrb::RingBuffer::<f32>::new(96_000);
    proc.attach_input(consumer, 1);
    let mut a = CantusApp::from_parts(Some(cantus_ui::starter_project()), h, false);
    a.rec.demo_inputs = Some(vec![0.0]);
    a.path = Some(dir.path().join("song.cantus"));
    a.add_audio_track();
    a.sync_engine();
    a.midi_port_names = vec!["Keys".into()];
    let s = a.project().tracks[0].strip.unwrap();
    let source = |n: u8| MidiSource {
        port: PortRef {
            name: "Keys".into(),
            ordinal: 0,
        },
        channel: Some(0),
        control: Control::Cc(n),
    };
    for (id, n, target) in [
        (1, 12, MappingTarget::Tempo),
        (2, 10, MappingTarget::Param(AutoTarget::StripPan(s))),
    ] {
        let mut m = Mapping::new(MappingId(id), source(n), target, MapMode::Absolute);
        m.takeover = Takeover::Jump;
        assert!(a.add_mapping(Scope::Project, m));
    }
    let bpm = a.project().tempo_bpm;
    a.start_recording();
    assert!(a.audio_take_running());
    a.set_tempo_now(bpm + 10.0, "tempo");
    assert_eq!(a.project().tempo_bpm, bpm, "the top bar");
    assert!(a.status.text.contains("record"), "said: {}", a.status.text);
    assert!(!a.exec(Command::SetTempo { bpm: 90.0 }), "a command");
    let pan = a.project().strips.iter().find(|x| x.id == s).unwrap().pan;
    a.controller_heard(&[cc(12, 100), cc(10, 20)]);
    assert_eq!(a.project().tempo_bpm, bpm, "a knob");
    let moved = a.project().strips.iter().find(|x| x.id == s).unwrap().pan;
    assert_ne!(moved, pan, "the pan knob in the same gesture moves");
    a.stop_recording();
    a.set_tempo_now(bpm + 10.0, "tempo");
    assert_eq!(a.project().tempo_bpm, bpm + 10.0, "after the take");
}

/// Finding 43: a tempo point off the grid, dragged straight up, keeps its tick (snap on,
/// a bar's grid); only the pointer's travel along the lane moves it.
#[test]
fn a_tempo_point_dragged_straight_up_keeps_its_place() {
    let mut p = Project::new("song");
    p.tempo_points = vec![pt(0, 120.0), pt(7680 + 1000, 140.0)];
    let mut t = p.make_track("Keys");
    t.clips
        .push(p.make_clip("c", Ticks::ZERO, Ticks::beats(16)));
    p.tracks.push(t);
    let mut a = app(p);
    a.arrangement.snap = true;
    a.arrangement.grid = Ticks::beats(4);
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let at = a.arrangement.tempo.points_drawn()[1];
    let up = at - egui::vec2(0.0, 20.0);
    arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(at)]);
    arrangement(
        &mut a,
        &ctx,
        vec![egui::Event::PointerMoved(at), button(at, true)],
    );
    for k in 1..=4 {
        let p = at - egui::vec2(0.0, 5.0 * k as f32);
        arrangement(&mut a, &ctx, vec![egui::Event::PointerMoved(p)]);
    }
    arrangement(&mut a, &ctx, vec![button(up, false)]);
    let q = a.project().tempo_points[1];
    assert!(q.bpm > 140.0, "the drag moved its tempo: {}", q.bpm);
    assert_eq!(q.tick, Ticks(7680 + 1000), "its place");
}

/// Findings 21, 34, 45: *Add a metre change* in an arrangement too narrow for the editor
/// opens it, never fails.
#[test]
fn a_metre_change_in_a_narrow_arrangement_opens_its_editor() {
    let mut p = Project::new("song");
    let mut t = p.make_track("Keys");
    t.clips
        .push(p.make_clip("c", Ticks::ZERO, Ticks::beats(16)));
    p.tracks.push(t);
    let mut a = app(p);
    let ctx = context();
    for width in [300.0, 120.0] {
        arrangement_in(&mut a, &ctx, width, vec![]);
        arrangement_in(&mut a, &ctx, width, vec![]);
        views::arrangement::tempo::add_metre_change(&mut a, Ticks::beats(8));
        arrangement_in(&mut a, &ctx, width, vec![]);
        assert_eq!(a.arrangement.tempo.metre_editor_at(), Some(Ticks::beats(8)));
        a.undo();
    }
}

/// Finding 36: two markers on one tick whose names fit before the next marker are drawn
/// whole, side by side.
#[test]
fn a_run_of_markers_that_fits_keeps_every_name_whole() {
    let mut p = Project::new("song");
    let mut t = p.make_track("Keys");
    t.clips
        .push(p.make_clip("c", Ticks::ZERO, Ticks::beats(32)));
    p.tracks.push(t);
    let mut a = app(p);
    a.arrangement.px_per_beat = 30.0;
    let long = a.add_marker(Ticks::beats(4)).unwrap();
    a.exec(Command::SetMarker {
        marker: cantus_core::tempo::Marker {
            name: "Pre-chorus build with strings".into(),
            ..a.project().marker(long).unwrap().clone()
        },
    });
    let short = a.add_marker(Ticks::beats(4)).unwrap();
    a.exec(Command::SetMarker {
        marker: cantus_core::tempo::Marker {
            name: "B".into(),
            ..a.project().marker(short).unwrap().clone()
        },
    });
    // The next marker 150 points on: room for both names, not for two equal halves.
    a.add_marker(Ticks::beats(9)).unwrap();
    let ctx = context();
    arrangement(&mut a, &ctx, vec![]);
    arrangement(&mut a, &ctx, vec![]);
    let tabs = a.arrangement.markers.tabs().to_vec();
    let (first, second, next) = (tabs[0], tabs[1], tabs[2]);
    let room = next.1.left() - first.1.left();
    assert!(
        first.2.width() > 0.6 * room,
        "the long name whole: {} of {room}",
        first.2.width()
    );
    assert!(first.1.right() <= second.1.left() && second.1.right() <= next.1.left());
}

/// Finding 49: a MIDI file opened on a song that has a tempo track but no clips does not
/// offer to set the song's tempo from it (nor does a drop: one rule).
#[test]
fn a_midi_file_on_a_tempo_track_alone_leaves_it_alone() {
    let mut p = Project::new("click map");
    p.tempo_points = vec![pt(0, 90.0), pt(16 * 3840, 120.0)];
    let mut a = app(p);
    assert!(!views::interop::empty_song(a.project()));
    let bytes = cantus_project::midi::export(&cantus_ui::starter_project(), &Default::default())
        .expect("a MIDI file");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("groove.mid");
    std::fs::write(&path, bytes).unwrap();
    views::interop::open_midi_import(&mut a, &path);
    let d = a.interop.midi_import.as_ref().expect("the dialog");
    assert!(!d.apply_tempo, "the song's tempo track stays");
    assert!(views::interop::empty_song(&Project::new("empty")));
}
