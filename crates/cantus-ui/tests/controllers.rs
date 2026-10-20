//! MIDI learn and controller mapping in the interface (round 29, ADR 0062): learning from a
//! parameter's menu and in learn mode, what each kind of control is learnt as, where a
//! learnt mapping goes, the drive (pick up, relative, toggle, actions), one undo step per
//! gesture, the project before the global map, dormant mappings, the filter the maps
//! publish, recording automation at the message's own time, the latch, feedback, the global
//! map's file, the keymap's actions and the pane.

mod support;
use cantus_core::controller::*;
use cantus_core::*;
use cantus_engine::midi::Heard;
use cantus_engine::MidiEvent;
use cantus_ui::controllers::Scope;
use cantus_ui::kit::automate::Choice;
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};
use support::*;

fn msg(status: u8, data1: u8, data2: u8) -> Heard {
    Heard {
        port: 0,
        event: MidiEvent {
            status,
            data1,
            data2,
        },
        at: Instant::now(),
    }
}

fn cc(n: u8, v: u8) -> Heard {
    msg(0xB0, n, v)
}

/// An app with one MIDI port open, "Keys".
fn keys_app() -> CantusApp {
    let mut a = app(cantus_ui::starter_project());
    a.midi_port_names = vec!["Keys".into()];
    a
}

fn strip(a: &CantusApp) -> StripId {
    a.project().tracks[0].strip.expect("a strip")
}

fn source(control: Control) -> MidiSource {
    MidiSource {
        port: PortRef {
            name: "Keys".into(),
            ordinal: 0,
        },
        channel: Some(0),
        control,
    }
}

/// A mapping of CC `n` to `target`, added to the project.
fn map(
    a: &mut CantusApp,
    n: u8,
    target: MappingTarget,
    mode: MapMode,
    takeover: Takeover,
) -> MappingId {
    let id = MappingId(9_000 + u64::from(n));
    let mut m = Mapping::new(id, source(Control::Cc(n)), target, mode);
    m.takeover = takeover;
    assert!(a.add_mapping(Scope::Project, m));
    id
}

fn pan_of(a: &CantusApp, s: StripId) -> f32 {
    a.project().strip(s).unwrap().pan
}

/// The gesture's idle time passed: the next move is a new undo step.
fn idle(a: &mut CantusApp) {
    if let Some(t) = a.controllers.gesture.last.as_mut() {
        *t -= Duration::from_secs(2);
    }
    a.controller_heard(&[]);
}

#[test]
fn a_parameter_menu_learns_the_next_knob_moved() {
    let mut a = keys_app();
    let s = strip(&a);
    let steps = a.bus.history().len();
    a.modulation_choice(&AutoTarget::StripPan(s), Choice::Learn, egui::Rect::NOTHING);
    assert!(a.controllers.learning.is_some(), "armed");
    assert!(
        a.controllers.tap.filter().learning(),
        "the tap hears everything meanwhile"
    );
    // A knob turned: a few values, never repeated.
    let turn: Vec<Heard> = (60..68).map(|v| cc(21, v)).collect();
    a.controller_heard(&turn);
    assert!(a.controllers.learning.is_none(), "learnt");
    assert!(!a.controllers.tap.filter().learning());
    let maps = &a.project().controllers.mappings;
    assert_eq!(maps.len(), 1);
    let m = &maps[0];
    assert_eq!(m.source, source(Control::Cc(21)));
    assert_eq!(m.target, MappingTarget::Param(AutoTarget::StripPan(s)));
    assert_eq!(m.mode, MapMode::Absolute);
    assert_eq!(m.takeover, Takeover::Pickup, "the settings' default");
    assert_eq!(a.bus.history().len(), steps + 1, "one undo step");
    // The mark and *Forget* in the menu: it is mapped.
    assert!(a.is_mapped(&m.target.clone()));
    // Learning it again replaces its control, not adds one.
    let target = m.target.clone();
    a.learn_target(target.clone());
    a.controller_heard(&(0..8).map(|v| cc(22, 10 + v)).collect::<Vec<_>>());
    let maps = &a.project().controllers.mappings;
    assert_eq!(maps.len(), 1);
    assert_eq!(maps[0].source.control, Control::Cc(22));
    // Forget takes it away (one undo step).
    a.modulation_choice(
        &AutoTarget::StripPan(s),
        Choice::Forget,
        egui::Rect::NOTHING,
    );
    assert!(a.project().controllers.is_empty());
    a.undo();
    assert_eq!(a.project().controllers.mappings.len(), 1);
}

#[test]
fn learning_tells_encoders_buttons_pads_and_14_bit_faders() {
    let mut a = keys_app();
    let s = strip(&a);
    let t = a.project().tracks[0].id;
    // An encoder: the same small step again and again.
    a.learn_target(MappingTarget::Param(AutoTarget::StripGain(s)));
    a.controller_heard(&[cc(16, 1); 8]);
    assert_eq!(
        a.project().controllers.mappings[0].mode,
        MapMode::Relative(Encoding::TwosComplement)
    );
    // A pad: learnt at once, on a mute it toggles.
    a.learn_target(MappingTarget::TrackMute(t));
    a.controller_heard(&[msg(0x99, 36, 100)]);
    let pad = a.project().controllers.mappings.last().unwrap();
    assert_eq!(pad.source.control, Control::Note(36));
    assert_eq!(pad.source.channel, Some(9));
    assert_eq!(pad.mode, MapMode::Toggle);
    // A 14-bit fader: its coarse controller and its fine twin.
    a.learn_target(MappingTarget::Param(AutoTarget::StripWidth(s)));
    let fader: Vec<Heard> = (0..4u8)
        .flat_map(|i| [cc(7, 60 + i), cc(39, 20 * i)])
        .collect();
    a.controller_heard(&fader);
    let wide = a.project().controllers.mappings.last().unwrap();
    assert_eq!(wide.source.control, Control::Cc14(7));
    // A release with nothing pressed is not a control.
    a.learn_target(MappingTarget::TrackSolo(t));
    a.controller_heard(&[msg(0x80, 40, 0)]);
    assert!(a.controllers.learning.is_some(), "still waiting");
    a.cancel_learning();
    assert!(a.controllers.learning.is_none());
}

#[test]
fn a_transport_key_learnt_goes_to_the_global_map_and_its_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = keys_app();
    a.controllers.global_path = Some(cantus_ui::controllers::global_path(dir.path()));
    let play = MappingTarget::Action("play-pause".into());
    a.learn_target(play.clone());
    a.controller_heard(&[cc(41, 127), cc(41, 0)]);
    // The window passes; or enough messages. Wait it out.
    std::thread::sleep(cantus_ui::controllers_learn::LEARN_WINDOW + Duration::from_millis(20));
    a.controller_heard(&[]);
    assert!(a.project().controllers.is_empty(), "not the project's");
    let g = &a.controllers.global.mappings;
    assert_eq!(g.len(), 1);
    assert_eq!(g[0].target, play);
    assert_eq!(g[0].mode, MapMode::Trigger);
    let back =
        cantus_ui::controllers::load_global(&cantus_ui::controllers::global_path(dir.path()));
    assert_eq!(back, a.controllers.global, "saved where it is read");
}

#[test]
fn the_global_file_keeps_what_it_can_and_drops_the_rest() {
    let dir = tempfile::tempdir().unwrap();
    let path = cantus_ui::controllers::global_path(dir.path());
    let good = Mapping::new(
        MappingId(1),
        source(Control::Cc(1)),
        MappingTarget::Tempo,
        MapMode::Absolute,
    );
    // A project's target cannot live in a global map; a malformed one; one unknown.
    let local = Mapping::new(
        MappingId(2),
        source(Control::Cc(2)),
        MappingTarget::TrackMute(TrackId(5)),
        MapMode::Toggle,
    );
    let bad = Mapping {
        min: 3.0,
        ..Mapping::new(
            MappingId(3),
            source(Control::Cc(3)),
            MappingTarget::Tempo,
            MapMode::Absolute,
        )
    };
    let text = serde_json::json!({
        "version": 1,
        "mappings": [
            serde_json::to_value(&good).unwrap(),
            serde_json::to_value(&local).unwrap(),
            serde_json::to_value(&bad).unwrap(),
            {"what": "no mapping"},
        ]
    });
    std::fs::write(&path, text.to_string()).unwrap();
    let map = cantus_ui::controllers::load_global(&path);
    assert_eq!(map.mappings, vec![good]);
    // Absent or not JSON: an empty map, no error.
    assert!(cantus_ui::controllers::load_global(&dir.path().join("none.json")).is_empty());
    std::fs::write(&path, "{ not json").unwrap();
    assert!(cantus_ui::controllers::load_global(&path).is_empty());
}

#[test]
fn a_knob_picks_up_and_a_gesture_is_one_undo_step() {
    let mut a = keys_app();
    let s = strip(&a);
    map(
        &mut a,
        10,
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Pickup,
    );
    let steps = a.bus.history().len();
    assert_eq!(pan_of(&a, s), 0.0, "centred: the knob's middle");
    // The knob at the bottom, turned up: nothing moves until it passes the middle.
    a.controller_heard(&[cc(10, 0), cc(10, 20), cc(10, 50)]);
    assert_eq!(pan_of(&a, s), 0.0, "pick up: no jump");
    assert_eq!(a.bus.history().len(), steps);
    a.controller_heard(&[cc(10, 70)]);
    let p = pan_of(&a, s);
    assert!(
        (p - (70.0 / 127.0 * 2.0 - 1.0)).abs() < 1e-3,
        "picked up: {p}"
    );
    // More turns in the same gesture: still one step.
    for v in [80, 90, 100] {
        a.controller_heard(&[cc(10, v)]);
    }
    assert_eq!(a.bus.history().len(), steps + 1);
    idle(&mut a);
    a.controller_heard(&[cc(10, 110)]);
    assert_eq!(
        a.bus.history().len(),
        steps + 2,
        "a new gesture, a new step"
    );
    a.undo();
    let p = pan_of(&a, s);
    assert!((p - (100.0 / 127.0 * 2.0 - 1.0)).abs() < 1e-3, "{p}");
    // The mouse moves it elsewhere: the knob picks up again before it moves it.
    a.exec(Command::SetStripPan { id: s, pan: -0.8 });
    idle(&mut a);
    a.controller_heard(&[cc(10, 105)]);
    assert_eq!(pan_of(&a, s), -0.8);
}

#[test]
fn two_knobs_turned_together_are_one_step_and_merge_frame_after_frame() {
    let mut a = keys_app();
    let s = strip(&a);
    map(
        &mut a,
        10,
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    map(
        &mut a,
        11,
        MappingTarget::Param(AutoTarget::StripWidth(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    let steps = a.bus.history().len();
    for v in [10u8, 20, 30, 40] {
        a.controller_heard(&[cc(10, v), cc(11, 127 - v)]);
    }
    assert_eq!(a.bus.history().len(), steps + 1);
    assert_eq!(
        a.bus.history().last().unwrap().label_key,
        GESTURE_LABEL,
        "a batch of both"
    );
    a.undo();
    assert_eq!(
        (pan_of(&a, s), a.project().strip(s).unwrap().width),
        (0.0, 1.0)
    );
}

#[test]
fn an_encoder_steps_and_a_button_toggles_and_fires_once() {
    let mut a = keys_app();
    let s = strip(&a);
    let t = a.project().tracks[0].id;
    map(
        &mut a,
        16,
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Relative(Encoding::TwosComplement),
        Takeover::Pickup,
    );
    a.controller_heard(&[cc(16, 1), cc(16, 1), cc(16, 1), cc(16, 1)]);
    let p = pan_of(&a, s);
    assert!((p - 4.0 / 128.0 * 2.0).abs() < 1e-4, "four steps up: {p}");
    a.controller_heard(&[cc(16, 127), cc(16, 127)]);
    let p = pan_of(&a, s);
    assert!((p - 2.0 / 128.0 * 2.0).abs() < 1e-4, "two down: {p}");
    // A pad toggles the track's mute.
    let mut pad = Mapping::new(
        MappingId(77),
        source(Control::Note(36)),
        MappingTarget::TrackMute(t),
        MapMode::Toggle,
    );
    pad.source.channel = None;
    a.add_mapping(Scope::Project, pad);
    a.controller_heard(&[msg(0x99, 36, 100), msg(0x89, 36, 0)]);
    assert!(a.project().track(t).unwrap().muted);
    a.controller_heard(&[msg(0x90, 36, 90)]);
    assert!(!a.project().track(t).unwrap().muted);
    // A button on an action: once per press.
    let mut play = Mapping::new(
        MappingId(78),
        source(Control::Cc(41)),
        MappingTarget::Action("toggle-metronome".into()),
        MapMode::Trigger,
    );
    play.source.port = PortRef::any();
    a.controllers.global.mappings.push(play);
    let before = a.project().metronome;
    a.controller_heard(&[cc(41, 127), cc(41, 127)]);
    assert_eq!(a.project().metronome, !before, "fired once");
    a.controller_heard(&[cc(41, 0), cc(41, 127)]);
    assert_eq!(
        a.project().metronome,
        before,
        "released, pressed: fired again"
    );
}

#[test]
fn the_project_comes_first_and_a_dormant_mapping_does_nothing() {
    let mut a = keys_app();
    let s = strip(&a);
    // The same knob in both maps: the project's moves, the global one does not.
    map(
        &mut a,
        12,
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    let mut g = Mapping::new(
        MappingId(500),
        source(Control::Cc(12)),
        MappingTarget::Param(AutoTarget::MasterPan),
        MapMode::Absolute,
    );
    g.takeover = Takeover::Jump;
    a.controllers.global.mappings.push(g);
    a.controller_heard(&[cc(12, 127)]);
    assert_eq!(pan_of(&a, s), 1.0);
    assert_eq!(a.project().master.pan, 0.0, "the project's mapping took it");
    // A mapping whose target is gone: nothing happens, and the global one hears it again.
    let gone = MappingTarget::TrackSolo(TrackId(123_456));
    a.add_mapping(
        Scope::Project,
        Mapping::new(
            MappingId(501),
            source(Control::Cc(13)),
            gone,
            MapMode::Toggle,
        ),
    );
    a.publish_controller_filter();
    let ev = MidiEvent {
        status: 0xB0,
        data1: 13,
        data2: 127,
    };
    assert!(
        !a.controllers.tap.filter().takes(0, &ev),
        "a dormant mapping consumes nothing"
    );
    let live = MidiEvent {
        status: 0xB0,
        data1: 12,
        data2: 1,
    };
    assert!(
        a.controllers.tap.filter().takes(0, &live),
        "a live one does"
    );
}

#[test]
fn the_filter_follows_the_maps_and_the_ports() {
    let mut a = keys_app();
    let s = strip(&a);
    a.publish_controller_filter();
    let ev = MidiEvent {
        status: 0xB0,
        data1: 10,
        data2: 64,
    };
    assert!(!a.controllers.tap.filter().takes(0, &ev));
    map(
        &mut a,
        10,
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    a.publish_controller_filter();
    assert!(a.controllers.tap.filter().takes(0, &ev));
    // Undo: the knob plays the instrument again.
    a.undo();
    a.publish_controller_filter();
    assert!(!a.controllers.tap.filter().takes(0, &ev));
    a.redo();
    // The port moved to another index (a device plugged in before it): followed.
    a.midi_port_names = vec!["Pads".into(), "Keys".into()];
    a.publish_controller_filter();
    assert!(!a.controllers.tap.filter().takes(0, &ev));
    assert!(a.controllers.tap.filter().takes(1, &ev));
}

#[test]
fn a_mapped_knob_while_recording_leaves_one_automation_clip_in_one_step() {
    let mut a = keys_app();
    let s = strip(&a);
    let target = AutoTarget::StripPan(s);
    map(
        &mut a,
        10,
        MappingTarget::Param(target.clone()),
        MapMode::Absolute,
        Takeover::Jump,
    );
    a.engine.seek(Ticks::beats(1));
    a.start_recording();
    for (beat, v) in [(1, 0u8), (2, 64), (3, 127)] {
        a.engine.seek(Ticks::beats(beat));
        a.controller_heard(&[cc(10, v)]);
    }
    a.engine.seek(Ticks::beats(5));
    let steps = a.bus.history().len();
    a.stop_recording();
    assert!(
        a.bus.history().len() <= steps + 2,
        "the take and the automation"
    );
    let clips: Vec<&Clip> = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| c.automation.as_ref().is_some_and(|x| x.target == target))
        .collect();
    assert_eq!(clips.len(), 1, "one automation clip");
    let clip = clips[0];
    assert_eq!(clip.start, Ticks::beats(1));
    let curve = clip.automation.as_ref().unwrap();
    let at = |beat: i64| {
        curve
            .value_at((Ticks::beats(beat) - clip.start).0 as f64)
            .unwrap()
    };
    assert!(at(1).abs() < 1e-3, "{}", at(1));
    assert!((at(3) - 1.0).abs() < 1e-3, "{}", at(3));
}

#[test]
fn a_message_is_recorded_where_it_was_heard_through_the_tempo_map() {
    use cantus_ui::controllers_input::tick_of_message;
    let p = Project::new("t");
    let map = p.tempo_map();
    // 120 BPM: half a second is one beat.
    let now = Ticks::beats(4).0 as f64;
    assert_eq!(tick_of_message(&map, now, 0.5), Ticks::beats(3).0);
    assert_eq!(tick_of_message(&map, now, 0.0), Ticks::beats(4).0);
    // At 60 BPM a beat is a second.
    let mut q = Project::new("q");
    q.tempo_bpm = 60.0;
    assert_eq!(tick_of_message(&q.tempo_map(), now, 1.0), Ticks::beats(3).0);
}

#[test]
fn a_parameter_a_pass_records_follows_its_controller_not_its_clip() {
    let mut a = keys_app();
    let s = strip(&a);
    let target = AutoTarget::StripPan(s);
    assert!(a.create_automation_clip(target.clone()));
    map(
        &mut a,
        10,
        MappingTarget::Param(target.clone()),
        MapMode::Absolute,
        Takeover::Jump,
    );
    let lane = |a: &CantusApp| {
        a.engine
            .sent_snapshot()
            .and_then(|s| s.automation.iter().find(|l| l.target == target).cloned())
            .expect("the lane")
    };
    a.sync_engine();
    assert!(!lane(&a).segs.is_empty(), "the clip plays");
    a.start_recording();
    a.controller_heard(&[cc(10, 127)]);
    a.sync_engine();
    let l = lane(&a);
    assert!(l.segs.is_empty(), "latched: the controller's value plays");
    assert_eq!(l.base, 1.0);
    a.stop_recording();
    a.sync_engine();
    assert!(!lane(&a).segs.is_empty(), "after the pass, the clip again");
}

#[test]
fn feedback_sends_the_value_back_once_per_change() {
    use cantus_ui::controllers_feedback::feedback_bytes;
    assert_eq!(feedback_bytes(Control::Cc(7), 2, 100), vec![0xB2, 7, 100]);
    assert_eq!(
        feedback_bytes(Control::Cc14(7), 0, 16_383),
        vec![0xB0, 7, 127, 0xB0, 39, 127]
    );
    assert_eq!(
        feedback_bytes(Control::Note(36), 9, 127),
        vec![0x99, 36, 127]
    );
    assert_eq!(
        feedback_bytes(Control::PitchBend, 0, 8192),
        vec![0xE0, 0, 64]
    );
    assert!(feedback_bytes(Control::Program(1), 0, 1).is_empty());
    let mut a = keys_app();
    let s = strip(&a);
    let id = map(
        &mut a,
        10,
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Jump,
    );
    let mut m = a.project().controllers.get(id).unwrap().clone();
    m.feedback = true;
    a.set_mapping(Scope::Project, m, None);
    a.controller_input();
    let sent = a.controllers.sent.get(&(Scope::Project, id)).copied();
    assert_eq!(sent, Some(64), "the pan's middle");
    a.exec(Command::SetStripPan { id: s, pan: 1.0 });
    a.controller_input();
    assert_eq!(a.controllers.sent.get(&(Scope::Project, id)), Some(&127));
}

#[test]
fn the_keymap_runs_learn_mode_link_last_tweaked_and_the_pane() {
    use cantus_ui::keymap::Action;
    use cantus_ui::views::workspace::Pane;
    let mut a = keys_app();
    let s = strip(&a);
    a.run_action(Action::MidiLearnMode);
    assert!(a.controllers.learn_mode);
    a.run_action(Action::MidiLearnMode);
    assert!(!a.controllers.learn_mode);
    // Nothing tweaked yet: nothing armed.
    a.run_action(Action::LinkLastTweaked);
    assert!(a.controllers.learning.is_none());
    a.exec(Command::SetStripPan { id: s, pan: 0.3 });
    a.run_action(Action::LinkLastTweaked);
    assert_eq!(
        a.controllers.learning.as_ref().map(|l| l.target.clone()),
        Some(MappingTarget::Param(AutoTarget::StripPan(s)))
    );
    let shown = a.workspace.contains(Pane::Controllers);
    a.run_action(Action::ViewControllers);
    assert_ne!(a.workspace.contains(Pane::Controllers), shown);
}

#[test]
fn a_keymap_of_version_8_gets_the_controller_keys() {
    use cantus_ui::keymap::{Action, Keymap};
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("keymap.toml");
    std::fs::write(&path, "preset = \"cantus\"\nversion = 8\n\n[bindings]\n").unwrap();
    let map = Keymap::load(&path);
    for a in [
        Action::MidiLearnMode,
        Action::LinkLastTweaked,
        Action::ViewControllers,
    ] {
        assert!(!map.bindings_for(a).is_empty(), "{a:?} has its key");
    }
}

/// The kit's controls in learn mode: a click learns and the value does not move; a drag
/// moves nothing.
#[test]
fn in_learn_mode_a_click_on_a_knob_learns_and_moves_nothing() {
    use cantus_ui::kit::automate::{LearnOffer, ModOffer};
    let run = |events: Vec<Vec<egui::Event>>| -> (f32, Vec<Choice>) {
        let ctx = ctx();
        let mut v = 0.5f32;
        let mut got = Vec::new();
        for (i, ev) in events.into_iter().enumerate() {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(200.0, 200.0),
                )),
                time: Some(i as f64 * 0.1),
                events: ev,
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                let m = ModOffer {
                    learn: Some(LearnOffer {
                        mapped: false,
                        mode: true,
                        armed: false,
                    }),
                    ..Default::default()
                };
                let (_, c) = cantus_ui::kit::widgets::Param::new(&mut v, 0.0..=1.0, 0.0)
                    .automation(false)
                    .modulation(m)
                    .show_full(ui);
                got.extend(c);
            });
            out.textures_delta.clear();
        }
        (v, got)
    };
    let at = egui::pos2(26.0, 26.0);
    let press = |p: bool, at: egui::Pos2| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: p,
        modifiers: Default::default(),
    };
    let (v, got) = run(vec![
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerMoved(at)],
        vec![press(true, at)],
        vec![press(false, at)],
        vec![],
    ]);
    assert_eq!(v, 0.5);
    assert!(got.contains(&Choice::Learn), "{got:?}");
    let end = at + egui::vec2(0.0, -60.0);
    let (v, _) = run(vec![
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerMoved(at)],
        vec![press(true, at)],
        vec![egui::Event::PointerMoved(at + egui::vec2(0.0, -30.0))],
        vec![egui::Event::PointerMoved(end)],
        vec![press(false, end)],
    ]);
    assert_eq!(v, 0.5, "a drag in learn mode moves nothing");
}

fn pane_app() -> CantusApp {
    let mut a = keys_app();
    let s = strip(&a);
    let t = a.project().tracks[0].id;
    map(
        &mut a,
        10,
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
        Takeover::Pickup,
    );
    // Shared: the same knob on the width too.
    let mut w = Mapping::new(
        MappingId(9_500),
        source(Control::Cc(10)),
        MappingTarget::Param(AutoTarget::StripWidth(s)),
        MapMode::Absolute,
    );
    w.takeover = Takeover::Scale;
    a.add_mapping(Scope::Project, w);
    // Dormant: its track is gone.
    a.add_mapping(
        Scope::Project,
        Mapping::new(
            MappingId(600),
            source(Control::Note(40)),
            MappingTarget::TrackSolo(TrackId(987_654)),
            MapMode::Toggle,
        ),
    );
    let _ = t;
    a.controllers.global.mappings.push(Mapping::new(
        MappingId(601),
        source(Control::Cc(41)),
        MappingTarget::Action("play-pause".into()),
        MapMode::Trigger,
    ));
    a
}

fn paint_pane(a: &mut CantusApp, size: egui::Vec2, events: Vec<egui::Event>) -> String {
    let ctx = ctx();
    let mut text = String::new();
    for i in 0..3 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 * 0.1),
            events: if i == 2 { events.clone() } else { Vec::new() },
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| cantus_ui::views::controllers::show(a, ui));
        out.textures_delta.clear();
        text = painted_text(&out.shapes);
    }
    text
}

#[test]
fn the_pane_lists_both_maps_with_their_words_and_flags() {
    let mut a = pane_app();
    let text = paint_pane(&mut a, egui::vec2(1400.0, 600.0), Vec::new());
    for words in [
        "CC 10",
        "Keys",
        "Pick up",
        "Scale",
        "Shared",
        "Missing",
        "Note E2",
        "Trigger",
        "Absolute",
        "This project",
    ] {
        assert!(
            text.to_lowercase().contains(&words.to_lowercase()),
            "{words:?} not in {text}"
        );
    }
    assert!(
        text.contains("Transport"),
        "the global row's target: {text}"
    );
    // Narrow: it scrolls, nothing panics, the rows are still there.
    let narrow = paint_pane(&mut a, egui::vec2(420.0, 300.0), Vec::new());
    assert!(narrow.contains("CC 10"));
    // The filter keeps what matches.
    a.controllers.view.filter = "transport".into();
    let text = paint_pane(&mut a, egui::vec2(1400.0, 600.0), Vec::new());
    assert!(
        text.contains("Transport") && !text.contains("Missing"),
        "{text}"
    );
}

#[test]
fn the_panes_remove_and_feedback_keys_edit_the_map() {
    let mut a = pane_app();
    let id = a.project().controllers.mappings[0].id;
    paint_pane(&mut a, egui::vec2(1400.0, 600.0), Vec::new());
    let at = |a: &CantusApp, what: &str| {
        let name = format!("1#{}#{what}", id.0);
        a.controllers
            .view
            .geo
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, r)| *r)
            .unwrap_or_else(|| panic!("no {name}"))
    };
    let click = |r: egui::Rect| {
        let p = r.center();
        vec![
            egui::Event::PointerMoved(p),
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
            egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Default::default(),
            },
        ]
    };
    let fb = at(&a, "feedback");
    paint_pane(&mut a, egui::vec2(1400.0, 600.0), click(fb));
    assert!(
        a.project().controllers.get(id).unwrap().feedback,
        "feedback on"
    );
    let n = a.project().controllers.mappings.len();
    let x = at(&a, "remove");
    paint_pane(&mut a, egui::vec2(1400.0, 600.0), click(x));
    assert_eq!(a.project().controllers.mappings.len(), n - 1, "removed");
    a.undo();
    assert_eq!(a.project().controllers.mappings.len(), n);
}
