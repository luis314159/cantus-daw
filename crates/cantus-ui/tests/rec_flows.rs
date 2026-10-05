//! Recording in the application (ADR 0025): arming, monitoring and inputs through the
//! app and through the header's published geometry; a pass started and stopped; a swipe
//! across a take lane and a drag in the punch lane, as real pointer events.

use cantus_core::audio::{AudioSource, MonitorMode, TakeMode, TrackInput};
use cantus_core::{AudioSourceId, Command, LoopRegion, Project, Ticks, TrackId};
use cantus_engine::{EngineHandle, FinishedTake, NoteInput};
use cantus_ui::record_pass::PassPlan;
use cantus_ui::{starter_project, views, CantusApp};

fn app(project: Project) -> CantusApp {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    CantusApp::from_parts(Some(project), EngineHandle::dummy(), false)
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// An app with the starter's instrument track and an audio track after it, selected.
fn with_audio_track() -> (CantusApp, TrackId, TrackId) {
    let mut a = app(starter_project());
    let keys = a.project().tracks[0].id;
    a.add_audio_track();
    let audio = a.selected_track.unwrap();
    (a, keys, audio)
}

/// Show the arrangement for a frame with these events.
fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    frame_with(a, ctx, events, egui::Modifiers::NONE);
}

fn frame_with(
    a: &mut CantusApp,
    ctx: &egui::Context,
    events: Vec<egui::Event>,
    modifiers: egui::Modifiers,
) {
    let mut events = events;
    events.insert(0, egui::Event::ModifiersChanged(modifiers));
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1200.0, 500.0),
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

fn button(pos: egui::Pos2, pressed: bool, modifiers: egui::Modifiers) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    }
}

fn click(a: &mut CantusApp, ctx: &egui::Context, at: egui::Pos2, modifiers: egui::Modifiers) {
    frame_with(a, ctx, vec![egui::Event::PointerMoved(at)], modifiers);
    frame_with(a, ctx, vec![button(at, true, modifiers)], modifiers);
    frame_with(a, ctx, vec![button(at, false, modifiers)], modifiers);
    frame(a, ctx, vec![]);
}

fn drag(a: &mut CantusApp, ctx: &egui::Context, from: egui::Pos2, to: egui::Pos2) {
    let none = egui::Modifiers::NONE;
    frame(a, ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true, none)],
    );
    for i in 1..=4 {
        let at = from + (to - from) * (i as f32 / 4.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false, none)],
    );
    frame(a, ctx, vec![]);
}

#[test]
fn an_audio_track_has_an_input_a_microphone_and_no_clips() {
    let (mut a, keys, audio) = with_audio_track();
    let t = a.project().track(audio).unwrap();
    assert_eq!(t.input, Some(TrackInput::mono(0)));
    assert_eq!(t.input.as_ref().unwrap().monitor, MonitorMode::Auto);
    assert_eq!(t.icon, Some(cantus_core::TrackIcon::Vocal));
    assert!(t.clips.is_empty());
    assert_eq!(a.project().tracks[1].id, audio, "after the selected track");
    a.undo();
    assert_eq!(a.project().tracks.len(), 1);
    assert_eq!(a.project().tracks[0].id, keys);
}

#[test]
fn arming_monitoring_and_inputs_round_trip_and_undo() {
    let (mut a, keys, audio) = with_audio_track();
    let before = a.project().clone();
    let start = a.bus.history().len();
    a.toggle_arm(audio, false);
    a.toggle_arm(keys, false);
    assert!(a.project().track(audio).unwrap().armed && a.project().track(keys).unwrap().armed);
    // Shift: this track alone, in one step.
    let steps = a.bus.history().len();
    a.toggle_arm(audio, true);
    assert!(a.project().track(audio).unwrap().armed && !a.project().track(keys).unwrap().armed);
    assert_eq!(a.bus.history().len(), steps + 1);
    assert_eq!(a.armed_tracks(), (vec![audio], vec![]));

    a.cycle_monitor(audio);
    a.cycle_monitor(audio);
    let input = a.project().track(audio).unwrap().input.clone().unwrap();
    assert_eq!(input.monitor, MonitorMode::Off, "Auto → On → Off");
    a.set_input_trim(audio, 4.5);
    a.set_input_trim(audio, 6.0);
    a.end_gesture();
    // A new input keeps the monitor mode and the trim.
    a.set_track_input(audio, Some(TrackInput::stereo(2)));
    let input = a.project().track(audio).unwrap().input.clone().unwrap();
    assert_eq!(
        (input.first, input.stereo, input.monitor, input.trim_db),
        (2, true, MonitorMode::Off, 6.0)
    );
    a.set_track_input(audio, None);
    assert!(
        a.armed_tracks().0.is_empty(),
        "without an input it records notes"
    );
    while a.bus.history().len() > start {
        a.undo();
    }
    assert_eq!(a.project().tracks, before.tracks);
}

#[test]
fn the_header_publishes_its_record_controls_and_clicking_them_works() {
    let (mut a, keys, audio) = with_audio_track();
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    let c = views::arrangement::record_controls(&a, audio).expect("drawn");
    click(&mut a, &ctx, c.arm, egui::Modifiers::NONE);
    assert!(
        a.project().track(audio).unwrap().armed,
        "a click on the lamp arms"
    );
    let k = views::arrangement::record_controls(&a, keys).unwrap();
    click(&mut a, &ctx, k.arm, egui::Modifiers::SHIFT);
    assert!(a.project().track(keys).unwrap().armed);
    assert!(
        !a.project().track(audio).unwrap().armed,
        "Shift+click arms that track alone"
    );
    // The monitor key cycles; the instrument track has none.
    assert!(k.monitor.is_none() && k.input.is_none());
    let c = views::arrangement::record_controls(&a, audio).unwrap();
    click(
        &mut a,
        &ctx,
        c.monitor
            .expect("an input track has a monitor key")
            .center(),
        egui::Modifiers::NONE,
    );
    let m = a
        .project()
        .track(audio)
        .unwrap()
        .input
        .as_ref()
        .unwrap()
        .monitor;
    assert_eq!(m, MonitorMode::On);
}

fn source(a: &mut CantusApp, name: &str) -> AudioSourceId {
    let id = AudioSourceId(a.bus.alloc_id());
    a.exec(Command::InsertAudioSource {
        source: AudioSource {
            id,
            name: name.into(),
            path: format!("C:/rec/{name}.wav"),
            sample_rate: 48_000,
            channels: 1,
            frames: 480_000,
            sha256: None,
            recorded: true,
        },
    });
    id
}

/// Two takes on lanes 1 and 2 of the audio track, the second playing on lane 0.
fn two_takes(a: &mut CantusApp, track: TrackId) {
    let s1 = source(a, "one");
    let s2 = source(a, "two");
    for (src, lane) in [(s1, 1u8), (s2, 2), (s2, 0)] {
        let mut c = a.bus.new_clip("take", Ticks::ZERO, Ticks::beats(16));
        c.audio = Some(cantus_core::AudioRegion::of(src));
        c.lane = lane;
        a.exec(Command::InsertClip {
            track,
            clip: Box::new(c),
        });
    }
}

#[test]
fn take_lanes_open_under_the_track_and_a_swipe_comps_in_one_step() {
    let (mut a, _, audio) = with_audio_track();
    two_takes(&mut a, audio);
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    let c = views::arrangement::record_controls(&a, audio).unwrap();
    click(
        &mut a,
        &ctx,
        c.disclosure
            .expect("a track with takes has a disclosure")
            .center(),
        egui::Modifiers::NONE,
    );
    assert!(a.project().track(audio).unwrap().takes_open);
    frame(&mut a, &ctx, vec![]);
    let g = a.arrangement.geometry().unwrap();
    let row = g.rows[1];
    assert!(
        row.takes > 0.0,
        "the take lanes add their height under the row"
    );
    // Lane 1 is the first band under the row.
    let lane_h = row.takes / 2.0;
    let y = row.top + row.height + lane_h * 0.5;
    let steps = a.bus.history().len();
    drag(
        &mut a,
        &ctx,
        egui::pos2(g.x(4.0) + 1.0, y),
        egui::pos2(g.x(8.0) - 1.0, y),
    );
    assert_eq!(a.bus.history().len(), steps + 1, "one undo step per swipe");
    let t = a.project().track(audio).unwrap();
    let lane0: Vec<_> = t
        .clips
        .iter()
        .filter(|c| c.lane == 0)
        .map(|c| (c.start, c.end()))
        .collect();
    assert_eq!(
        lane0.len(),
        3,
        "the swipe cut a window on take one into the playing lane: {lane0:?}"
    );
    a.undo();
    assert_eq!(
        a.project()
            .track(audio)
            .unwrap()
            .clips
            .iter()
            .filter(|c| c.lane == 0)
            .count(),
        1
    );
}

#[test]
fn the_punch_lane_appears_with_punch_and_a_drag_sets_the_range() {
    let (mut a, _, _) = with_audio_track();
    a.toggle_punch();
    assert!(a.project().recording.punch_enabled);
    let ctx = ctx();
    frame(&mut a, &ctx, vec![]);
    let g = a.arrangement.geometry().unwrap();
    let (top, bottom) = g.punch.expect("the punch lane is drawn while punch is on");
    let y = (top + bottom) * 0.5;
    drag(
        &mut a,
        &ctx,
        egui::pos2(g.x(8.0) + 20.0, y),
        egui::pos2(g.x(12.0) + 20.0, y),
    );
    let r = a.project().recording.punch.clone().unwrap();
    assert_eq!(
        (r.start, r.end),
        (Ticks::beats(8), Ticks::beats(12)),
        "snapped to the grid"
    );
    a.toggle_punch();
    frame(&mut a, &ctx, vec![]);
    assert!(a.arrangement.geometry().unwrap().punch.is_none());
}

#[test]
fn record_options_count_in_and_punch_from_selection() {
    let (mut a, _, _) = with_audio_track();
    a.run_action(cantus_ui::keymap::Action::ToggleCountIn);
    assert_eq!(a.project().recording.count_in_bars, 2);
    a.run_action(cantus_ui::keymap::Action::ToggleCountIn);
    assert_eq!(a.project().recording.count_in_bars, 0);
    a.set_time_selection(Ticks::beats(4), Ticks::beats(12));
    a.punch_from_selection();
    let rs = &a.project().recording;
    assert!(rs.punch_enabled);
    assert_eq!(
        rs.punch,
        Some(LoopRegion {
            start: Ticks::beats(4),
            end: Ticks::beats(12)
        })
    );
}

#[test]
fn an_armed_audio_track_with_no_device_says_so_and_does_not_start() {
    let (mut a, _, audio) = with_audio_track();
    a.toggle_arm(audio, false);
    a.start_recording();
    assert!(!a.is_recording(), "no input device: nothing to record");
    assert!(a.status.error, "the status says why");
    assert!(a.show_settings, "and the settings are one look away");
}

#[test]
fn an_armed_instrument_track_records_a_take_on_its_own_lane() {
    let (mut a, keys, _) = with_audio_track();
    let before = a.project().track(keys).unwrap().clips.len();
    a.toggle_arm(keys, false);
    a.start_recording();
    assert!(a.is_recording());
    let ev = |tick: i64, pitch: u8, vel: u8| NoteInput {
        track: keys,
        tick: Ticks(tick),
        pitch,
        velocity: vel,
    };
    a.ingest_note_inputs(vec![ev(960, 60, 100), ev(1440, 60, 0)]);
    let steps = a.bus.history().len();
    a.stop_recording();
    assert!(!a.is_recording());
    assert_eq!(a.bus.history().len(), steps + 1, "one undo step per pass");
    let t = a.project().track(keys).unwrap();
    // Stack: the bar of the starter clip under the take moved to a take lane, the rest of
    // it stays, the take plays.
    assert_eq!(t.clips.len(), before + 2);
    let lane = |l: u8| -> Vec<(i64, i64, usize)> {
        t.clips
            .iter()
            .filter(|c| c.lane == l)
            .map(|c| (c.start.0, c.end().0, c.notes.len()))
            .collect()
    };
    assert_eq!(lane(0), vec![(0, 3840, 1), (3840, 15360, 4)]);
    assert_eq!(lane(1), vec![(0, 3840, 4)]);
}

#[test]
fn a_finished_pass_is_one_undo_step_and_reports_dropped_frames() {
    let (mut a, _, audio) = with_audio_track();
    let before = a.project().clone();
    let mut plan = PassPlan::new(Ticks::ZERO);
    plan.audio_tracks = vec![audio];
    plan.mode = TakeMode::Stack;
    let take = FinishedTake {
        track: audio,
        path: "C:/rec/Audio 1 take.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 96_000,
        start_tick: 0.0,
        wraps: vec![],
        dropped: 480,
    };
    let steps = a.bus.history().len();
    assert!(a.finish_pass(plan, vec![take], vec![]));
    assert_eq!(a.bus.history().len(), steps + 1);
    assert_eq!(a.project().audio_pool.len(), 1);
    assert_eq!(a.project().track(audio).unwrap().clips.len(), 1);
    assert!(a.status.error, "lost frames are reported");
    assert_eq!(a.rec.dropped, 480);
    a.undo();
    assert_eq!(a.project().tracks, before.tracks);
    assert!(a.project().audio_pool.is_empty());
}
