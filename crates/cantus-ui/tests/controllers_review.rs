//! Review 1 of ADR 0062 (round 29): each confirmed finding about the interface's
//! controllers, held by a test — gestures and undo, the tempo, recording, hosted
//! parameters, feedback, precedence, dormant mappings, learning, the pane's typing, the
//! learn mode on every control.

mod support;
use cantus_core::controller::*;
use cantus_core::*;
use cantus_engine::midi::Heard;
use cantus_engine::MidiEvent;
use cantus_ui::controllers::Scope;
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};
use support::*;

fn msg_at(status: u8, data1: u8, data2: u8, at: Instant) -> Heard {
    Heard {
        port: 0,
        event: MidiEvent {
            status,
            data1,
            data2,
        },
        at,
    }
}

fn cc(n: u8, v: u8) -> Heard {
    msg_at(0xB0, n, v, Instant::now())
}

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

fn add(
    a: &mut CantusApp,
    scope: Scope,
    id: u64,
    control: Control,
    target: MappingTarget,
    mode: MapMode,
) {
    let mut m = Mapping::new(MappingId(id), source(control), target, mode);
    m.takeover = Takeover::Jump;
    assert!(a.add_mapping(scope, m));
}

/// #3, #17: two bursts half a second and more apart, no frame between them, are two undo
/// steps — sealed by the messages' own time.
#[test]
fn bursts_apart_are_two_steps_without_a_frame_between() {
    let mut a = keys_app();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
    );
    let steps = a.bus.history().len();
    let t0 = Instant::now();
    a.controller_heard(&[msg_at(0xB0, 10, 20, t0), msg_at(0xB0, 10, 30, t0)]);
    let later = t0 + Duration::from_millis(600);
    a.controller_heard(&[msg_at(0xB0, 10, 90, later)]);
    assert_eq!(a.bus.history().len(), steps + 2);
}

/// #22: a second knob joining a turning one widens the step: one undo takes both back.
#[test]
fn a_knob_joining_later_is_the_same_step() {
    let mut a = keys_app();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Cc(11),
        MappingTarget::Param(AutoTarget::StripWidth(s)),
        MapMode::Absolute,
    );
    let before = a.project().strip(s).unwrap().clone();
    let steps = a.bus.history().len();
    for v in [10u8, 20, 30] {
        a.controller_heard(&[cc(10, v)]);
    }
    for v in [40u8, 50] {
        a.controller_heard(&[cc(10, v), cc(11, 127 - v)]);
    }
    assert_eq!(a.bus.history().len(), steps + 1, "one step");
    a.undo();
    let now = a.project().strip(s).unwrap();
    assert_eq!((now.pan, now.width), (before.pan, before.width));
}

/// #18, #1: in a gesture with the tempo and a pan, frames that move only the pan leave the
/// tempo point exactly as the tempo knob set it — never rebuilt from a read-back.
#[test]
fn an_untouched_tempo_target_is_not_rewritten() {
    let mut a = keys_app();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Global,
        1,
        Control::Cc(16),
        MappingTarget::Tempo,
        MapMode::Relative(Encoding::TwosComplement),
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
    );
    a.controller_heard(&[cc(16, 1), cc(10, 20)]);
    let tempo = a.project().tempo_bpm;
    assert_ne!(tempo, 120.0, "the knob moved it");
    for v in [30u8, 40, 50, 60] {
        a.controller_heard(&[cc(10, v)]);
        assert_eq!(a.project().tempo_bpm.to_bits(), tempo.to_bits());
    }
}

/// #21: an arm and a fader in one gesture keep one command per target in the step.
#[test]
fn an_arm_in_a_gesture_keeps_the_step_small() {
    let mut a = keys_app();
    let s = strip(&a);
    let t = a.project().tracks[0].id;
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Note(36),
        MappingTarget::TrackArm(t),
        MapMode::Toggle,
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripGain(s)),
        MapMode::Absolute,
    );
    a.controller_heard(&[msg_at(0x90, 36, 100, Instant::now()), cc(10, 50)]);
    let size = |a: &CantusApp| match &a.bus.history().last().unwrap().command {
        Command::Batch { commands, .. } => commands.len(),
        _ => 1,
    };
    let first = size(&a);
    for v in [60u8, 70, 80, 90] {
        a.controller_heard(&[cc(10, v)]);
    }
    assert_eq!(size(&a), first, "the batch does not grow");
}

/// #5, #33: a knob on a modulator's amount while a pass records leaves an automation clip.
#[test]
fn a_modulator_amount_moved_while_recording_is_recorded() {
    let mut a = keys_app();
    let t = a.project().tracks[0].id;
    let m = a
        .add_modulator(modulator::ModHost::Track(t), ModKind::Lfo, None)
        .unwrap();
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::ModAmount(m)),
        MapMode::Absolute,
    );
    a.engine.seek(Ticks::beats(1));
    a.start_recording();
    for (beat, v) in [(1, 0u8), (2, 64), (3, 127)] {
        a.engine.seek(Ticks::beats(beat));
        a.controller_heard(&[cc(10, v)]);
    }
    a.engine.seek(Ticks::beats(5));
    a.stop_recording();
    let clips = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| {
            c.automation
                .as_ref()
                .is_some_and(|x| x.target == AutoTarget::ModAmount(m))
        })
        .count();
    assert_eq!(clips, 1);
}

/// #37: a move is recorded where its message came, not where the frame read it; the pass
/// leaves exactly one step for its automation, which one undo takes away.
#[test]
fn a_move_is_recorded_at_its_message_time_in_one_step() {
    let mut a = keys_app();
    let s = strip(&a);
    let target = AutoTarget::StripPan(s);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        MappingTarget::Param(target.clone()),
        MapMode::Absolute,
    );
    a.engine.seek(Ticks::beats(2));
    a.start_recording();
    // 120 BPM: a message half a second old, read at beat 4, was heard at beat 3.
    a.engine.set_heard_for_tests(Ticks::beats(4), true);
    a.controller_heard(&[msg_at(
        0xB0,
        10,
        127,
        Instant::now() - Duration::from_millis(500),
    )]);
    a.engine.set_heard_for_tests(Ticks::beats(6), false);
    let steps = a.bus.history().len();
    a.stop_recording();
    let at = a.modulators.rec.points.clone();
    assert!(at.is_empty(), "committed");
    let clip = a
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .find(|c| c.automation.as_ref().is_some_and(|x| x.target == target))
        .expect("recorded")
        .clone();
    let start = clip.start.0 as f64;
    let beat3 = Ticks::beats(3).0 as f64;
    assert!(
        (start - beat3).abs() < 40.0,
        "at its message's time: {start} vs {beat3}"
    );
    assert_eq!(
        a.bus.history().len(),
        steps + 1,
        "one step for the automation"
    );
    a.undo();
    assert!(!a
        .project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .any(|c| c.automation.as_ref().is_some_and(|x| x.target == target)));
}

/// #23: a message heard just before a loop's wrap, read after it, is the end of the pass
/// before.
#[test]
fn a_message_before_the_wrap_lands_at_the_loops_end() {
    use cantus_ui::controllers_input::tick_in_loop;
    let (start, end) = (Ticks::beats(16).0, Ticks::beats(32).0);
    assert_eq!(
        tick_in_loop(start - 10, start + 20, Some((start, end))),
        end - 10
    );
    assert_eq!(
        tick_in_loop(start + 5, start + 20, Some((start, end))),
        start + 5
    );
    assert_eq!(tick_in_loop(start - 10, start + 20, None), start - 10);
}

/// #7: a 14-bit fader in the project and its coarse controller in the global map: only
/// the project's moves.
#[test]
fn the_project_takes_a_message_whatever_address_it_maps() {
    let mut a = keys_app();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc14(7),
        MappingTarget::Param(AutoTarget::StripGain(s)),
        MapMode::Absolute,
    );
    add(
        &mut a,
        Scope::Global,
        2,
        Control::Cc(7),
        MappingTarget::Param(AutoTarget::MasterPan),
        MapMode::Absolute,
    );
    a.controller_heard(&[cc(7, 127)]);
    assert_eq!(a.project().master.pan, 0.0, "the global one stayed still");
    assert_ne!(a.project().strip(s).unwrap().gain_db, 0.0);
}

/// #9: a 14-bit pair fed only its coarse half reaches its top; once its fine half has
/// come, a coarse message alone moves nothing half-made.
#[test]
fn a_14_bit_pair_reaches_its_top_and_waits_for_its_fine_half() {
    let mut a = keys_app();
    add(
        &mut a,
        Scope::Global,
        1,
        Control::Cc14(7),
        MappingTarget::Param(AutoTarget::MasterWidth),
        MapMode::Absolute,
    );
    a.controller_heard(&[cc(7, 127)]);
    assert_eq!(a.project().master.width, 2.0, "the top");
    a.controller_heard(&[cc(7, 64), cc(39, 0)]);
    let w = a.project().master.width;
    a.controller_heard(&[cc(7, 0)]);
    assert_eq!(a.project().master.width, w, "the coarse half alone waits");
    a.controller_heard(&[cc(39, 0)]);
    assert_eq!(a.project().master.width, 0.0);
}

/// The test CLAP plug-in, built from source where the host's tests build it.
fn test_plugin() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let status =
        std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status()
            .expect("cargo");
    assert!(status.success());
    target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    })
}

/// #36, #55, #56: a knob on a hosted plug-in's parameter moves it through the plug-in's own
/// range, learnt with the plug-in's identity; a parameter the plug-in does not have is
/// dormant (consumes nothing, shown *Missing*).
#[test]
fn a_knob_drives_a_hosted_parameter_and_a_missing_one_is_dormant() {
    let mut a = keys_app();
    cantus_ui::views::plugins::demo_setup(&mut a, &test_plugin(), true);
    let (s, effect) = match a.fx.open {
        Some((s, e, _)) => (s, e),
        None => panic!("the editor did not open"),
    };
    let target = AutoTarget::Plugin { effect, id: 0 };
    let (min, max) = {
        let (_, info) = a.plugin_param(&target).expect("loaded");
        (info.min, info.max)
    };
    let value = |a: &CantusApp| {
        a.project()
            .strip(s)
            .and_then(|st| st.effect(effect))
            .and_then(|e| e.spec.external.as_deref())
            .and_then(|p| p.params.get(&0).copied())
    };
    // Learnt: the mapping remembers its plug-in.
    a.learn_target(MappingTarget::Param(target.clone()));
    a.controller_heard(&(0..8).map(|v| cc(20, 60 + v)).collect::<Vec<_>>());
    let m = a.project().controllers.mappings[0].clone();
    assert_eq!(
        m.plugin,
        MappingTarget::Param(target.clone()).plugin_of(a.project())
    );
    let mut j = m.clone();
    j.takeover = Takeover::Jump;
    a.set_mapping(Scope::Project, j, None);
    a.controller_heard(&[cc(20, 127)]);
    assert_eq!(value(&a), Some(max));
    a.controller_heard(&[cc(20, 0)]);
    assert_eq!(value(&a), Some(min));
    // A parameter id the plug-in does not have: dormant.
    add(
        &mut a,
        Scope::Project,
        900,
        Control::Cc(21),
        MappingTarget::Param(AutoTarget::Plugin { effect, id: 4_242 }),
        MapMode::Absolute,
    );
    a.publish_controller_filter();
    let ev = MidiEvent {
        status: 0xB0,
        data1: 21,
        data2: 1,
    };
    assert!(
        !a.controllers.tap.filter().takes(0, &ev),
        "consumes nothing"
    );
    let gone = a.project().controllers.get(MappingId(900)).unwrap().clone();
    assert!(!a.mapping_alive(Scope::Project, &gone));
}

/// #11, #39, #15: feedback goes one message at a time, once per change, never while its
/// own control moves, and what comes back on the input is not a move.
#[test]
fn feedback_once_per_change_quiet_while_moving_and_deaf_to_its_echo() {
    use cantus_ui::controllers_feedback::feedback_messages;
    for c in [
        Control::Cc14(7),
        Control::PitchBend,
        Control::Cc(1),
        Control::Note(36),
    ] {
        for msg in feedback_messages(c, 3, 9_000) {
            assert!(msg.len() <= 3 && msg[0] & 0x80 != 0, "{c:?}: {msg:?}");
        }
    }
    assert_eq!(feedback_messages(Control::Cc14(7), 0, 16_383).len(), 2);
    let mut a = keys_app();
    let s = strip(&a);
    let mut m = Mapping::new(
        MappingId(1),
        source(Control::Cc(10)),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
    );
    m.takeover = Takeover::Jump;
    m.feedback = true;
    a.add_mapping(Scope::Project, m);
    a.controllers.feedback_log = Some(Vec::new());
    a.controller_input();
    a.controller_input();
    assert_eq!(
        a.controllers.feedback_log.as_ref().unwrap().len(),
        1,
        "once"
    );
    // Its own control moves: nothing goes back meanwhile.
    a.controllers.feedback_log = Some(Vec::new());
    a.controller_heard(&[cc(10, 100)]);
    a.controller_input();
    assert!(
        a.controllers.feedback_log.as_ref().unwrap().is_empty(),
        "quiet"
    );
    // Afterwards, once.
    a.controllers.activity.insert(
        (Scope::Project, MappingId(1)),
        Instant::now() - Duration::from_secs(1),
    );
    a.controller_input();
    let log = a.controllers.feedback_log.clone().unwrap();
    assert_eq!(log.len(), 1);
    // The echo of what was sent: not a move.
    a.exec(Command::SetStripPan { id: s, pan: -0.5 });
    a.controller_input();
    let sent = a
        .controllers
        .feedback_log
        .clone()
        .unwrap()
        .last()
        .unwrap()
        .1
        .clone();
    let pan = a.project().strip(s).unwrap().pan;
    a.controller_heard(&[msg_at(sent[0], sent[1], sent[2], Instant::now())]);
    assert_eq!(a.project().strip(s).unwrap().pan, pan);
}

/// #7: a global feedback mapping shadowed by a live project mapping on the same control
/// sends nothing.
#[test]
fn a_shadowed_global_mapping_sends_no_feedback() {
    let mut a = keys_app();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
    );
    let mut g = Mapping::new(
        MappingId(2),
        source(Control::Cc(10)),
        MappingTarget::Param(AutoTarget::MasterPan),
        MapMode::Absolute,
    );
    g.feedback = true;
    a.controllers.global.mappings.push(g);
    a.controllers.feedback_log = Some(Vec::new());
    a.controller_input();
    assert!(a.controllers.feedback_log.as_ref().unwrap().is_empty());
}

/// #40: a dormant project mapping does nothing, and the global one on its control hears it.
#[test]
fn a_dormant_mapping_lets_the_global_one_hear_its_control() {
    let mut a = keys_app();
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(13),
        MappingTarget::TrackSolo(TrackId(123_456)),
        MapMode::Toggle,
    );
    add(
        &mut a,
        Scope::Global,
        2,
        Control::Cc(13),
        MappingTarget::Param(AutoTarget::MasterPan),
        MapMode::Absolute,
    );
    let before = a.project().clone();
    a.controller_heard(&[cc(13, 127)]);
    assert_eq!(a.project().master.pan, 1.0);
    let mut after = a.project().clone();
    after.master.pan = before.master.pan;
    after.next_id = before.next_id;
    assert_eq!(after, before, "nothing else changed");
}

/// #42, #43: typing in the pane's cells: a curve in percent, a device's value in its own
/// units.
#[test]
fn typed_values_read_their_units() {
    use cantus_ui::views::controllers::parse_value;
    let mut a = keys_app();
    let pan = MappingTarget::Param(AutoTarget::MasterPan);
    // The curve cell reads as a pan's words: +50 → a bend of 0.5, −35 → −0.35.
    let p = parse_value(&a, &pan, "+50").unwrap();
    assert!((p * 2.0 - 1.0 - 0.5).abs() < 1e-5);
    let p = parse_value(&a, &pan, "-35").unwrap();
    assert!((p * 2.0 - 1.0 + 0.35).abs() < 1e-5);
    let s = strip(&a);
    a.add_effect(s, EffectKind::Compressor);
    let fx = a.project().strip(s).unwrap().effects.last().unwrap().id;
    let find = |unit: &str| {
        EffectKind::Compressor
            .params()
            .iter()
            .find(|i| i.unit == unit && !i.stepped)
            .map(|i| i.key)
    };
    let range = |a: &CantusApp, key: &str| {
        cantus_core::automation::range::range_of(
            a.project(),
            &AutoTarget::Effect {
                effect: fx,
                key: key.into(),
            },
        )
        .unwrap()
    };
    if let Some(key) = find("ms") {
        let t = MappingTarget::Param(AutoTarget::Effect {
            effect: fx,
            key: key.into(),
        });
        let r = range(&a, key);
        let want = r.pos(120.0f32.clamp(r.min, r.max));
        let got = parse_value(&a, &t, "0.12 s").unwrap();
        assert!(
            (got - want).abs() < 1e-4,
            "{key}: 0.12 s is 120 ms: {got} vs {want}"
        );
    }
    if let Some(key) = find("%") {
        let t = MappingTarget::Param(AutoTarget::Effect {
            effect: fx,
            key: key.into(),
        });
        let r = range(&a, key);
        let want = r.pos(0.5f32.clamp(r.min, r.max));
        let got = parse_value(&a, &t, "50%").unwrap();
        assert!(
            (got - want).abs() < 1e-4,
            "{key}: 50% is a half: {got} vs {want}"
        );
    }
}

/// #44: an action that is a state lights its button; one that is not, and a program
/// change, have no feedback to give.
#[test]
fn transport_buttons_feed_back_their_state() {
    use cantus_ui::controllers_feedback::can_feed_back;
    let mut a = keys_app();
    let loop_ = Mapping::new(
        MappingId(1),
        source(Control::Cc(46)),
        MappingTarget::Action("toggle-loop".into()),
        MapMode::Trigger,
    );
    let stop = Mapping {
        target: MappingTarget::Action("stop".into()),
        ..loop_.clone()
    };
    let program = Mapping {
        source: source(Control::Program(3)),
        ..loop_.clone()
    };
    assert!(can_feed_back(&a, &loop_));
    assert!(!can_feed_back(&a, &stop));
    assert!(!can_feed_back(&a, &program));
    assert_eq!(a.action_state("toggle-loop"), Some(0.0));
    a.run_action(cantus_ui::keymap::Action::ToggleLoop);
    assert_eq!(a.action_state("toggle-loop"), Some(1.0));
}

/// #16: a learnt control is the map's at once: its next message is taken.
#[test]
fn a_learnt_control_is_taken_at_once() {
    let mut a = keys_app();
    let s = strip(&a);
    a.learn_target(MappingTarget::Param(AutoTarget::StripPan(s)));
    a.controller_heard(&(0..8).map(|v| cc(22, 60 + v)).collect::<Vec<_>>());
    assert!(!a.controllers.tap.filter().learning());
    let ev = MidiEvent {
        status: 0xB0,
        data1: 22,
        data2: 70,
    };
    assert!(a.controllers.tap.filter().takes(0, &ev));
}

/// #32: an armed learn that hears nothing lets go after its timeout.
#[test]
fn an_armed_learn_times_out() {
    let mut a = keys_app();
    a.learn_target(MappingTarget::Tempo);
    a.controllers.learning.as_mut().unwrap().armed_at -= Duration::from_secs(60);
    a.controller_heard(&[]);
    assert!(a.controllers.learning.is_none());
    assert!(!a.controllers.tap.filter().learning());
}

/// #51: forgetting a strip's controllers is one undo step.
#[test]
fn forgetting_several_controllers_is_one_step() {
    let mut a = keys_app();
    let s = strip(&a);
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::StripPan(s)),
        MapMode::Absolute,
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Cc(11),
        MappingTarget::StripMute(s),
        MapMode::Toggle,
    );
    let steps = a.bus.history().len();
    a.forget_targets(
        &[
            MappingTarget::Param(AutoTarget::StripPan(s)),
            MappingTarget::StripMute(s),
        ],
        "strip",
    );
    assert!(a.project().controllers.is_empty());
    assert_eq!(a.bus.history().len(), steps + 1);
}

/// #31: a damaged global file is kept beside and said so; the map is written atomically.
#[test]
fn a_damaged_global_file_is_kept_and_the_map_written_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = cantus_ui::controllers::global_path(dir.path());
    std::fs::write(&path, "{ not json").unwrap();
    let (map, damaged) = cantus_ui::controllers::load_global_checked(&path);
    assert!(map.is_empty() && damaged);
    assert!(path.with_extension("json.bad").exists(), "kept");
    let mut m = ControllerMap::default();
    m.mappings.push(Mapping::new(
        MappingId(1),
        source(Control::Cc(1)),
        MappingTarget::Tempo,
        MapMode::Absolute,
    ));
    cantus_ui::controllers::save_global(&path, &m).unwrap();
    assert!(!path.with_extension("json.tmp").exists());
    assert_eq!(cantus_ui::controllers::load_global(&path), m);
}

/// #35: a learn spot (a lamp, a transport key) in learn mode takes the click: the button
/// under it does nothing and the target is armed; outside its clip it takes nothing.
#[test]
fn a_learn_spot_takes_the_click_inside_its_clip_only() {
    let ctx = ctx();
    let mut a = keys_app();
    let s = strip(&a);
    a.toggle_learn_mode();
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(20.0, 20.0));
    let at = rect.center();
    let mut pressed = 0;
    let frames = vec![
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    ];
    for (i, events) in frames.into_iter().enumerate() {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(200.0, 200.0),
            )),
            time: Some(i as f64 * 0.1),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let r = ui.interact(rect, egui::Id::new("a-mute-lamp"), egui::Sense::click());
            if r.clicked() {
                pressed += 1;
            }
            cantus_ui::controllers::learn_spot(&mut a, ui, rect, MappingTarget::StripMute(s));
        });
        out.textures_delta.clear();
    }
    assert_eq!(pressed, 0, "the lamp under did nothing");
    assert_eq!(
        a.controllers.learning.as_ref().map(|l| l.target.clone()),
        Some(MappingTarget::StripMute(s))
    );
}

/// #35: the mixer's controls (faders, pans, widths, lamps) and the top bar's keys all offer
/// learning.
#[test]
fn the_mixer_and_the_top_bar_offer_learn_everywhere() {
    use cantus_ui::kit::automate::probe;
    let mut a = keys_app();
    a.toggle_learn_mode();
    probe::start();
    paint(&ctx(), egui::vec2(1600.0, 700.0), 3, |ui| {
        cantus_ui::views::mixer::show(&mut a, ui)
    });
    let drawn = probe::take_learn();
    assert!(
        drawn.iter().any(|(n, _)| n.starts_with("desk:")),
        "{drawn:?}"
    );
    assert!(
        drawn.iter().any(|(n, _)| n.contains("StripMute")),
        "{drawn:?}"
    );
    assert!(drawn.iter().all(|(_, l)| *l), "{drawn:?}");
    probe::start();
    paint(&ctx(), egui::vec2(1920.0, 70.0), 3, |ui| {
        cantus_ui::views::transport::show(&mut a, ui)
    });
    let drawn = probe::take_learn();
    for key in [
        "play-pause",
        "stop",
        "record",
        "toggle-loop",
        "toggle-metronome",
        "tap-tempo",
    ] {
        assert!(
            drawn.iter().any(|(n, l)| n.contains(key) && *l),
            "{key}: {drawn:?}"
        );
    }
    assert!(drawn.iter().any(|(n, _)| n.contains("Tempo")), "{drawn:?}");
}

/// #29, #47: with no MIDI input, the MIDI cell opens the settings, as its words say.
#[test]
fn without_ports_the_midi_cell_opens_the_settings() {
    let ctx = ctx();
    let mut a = app(cantus_ui::starter_project());
    assert!(a.midi_port_names.is_empty());
    // Find the cell by painting once, then click it.
    paint(&ctx, egui::vec2(1920.0, 70.0), 2, |ui| {
        cantus_ui::views::transport::show(&mut a, ui)
    });
    let r = ctx
        .memory(|m| m.area_rect(egui::Id::new("toolbar-midi")))
        .or_else(|| {
            ctx.read_response(egui::Id::new("toolbar-midi"))
                .map(|r| r.rect)
        });
    let Some(r) = r else { panic!("no MIDI cell") };
    let at = r.center();
    for (i, events) in [
        vec![egui::Event::PointerMoved(at)],
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
        vec![egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }],
    ]
    .into_iter()
    .enumerate()
    {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1920.0, 70.0),
            )),
            time: Some(1.0 + i as f64 * 0.1),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| cantus_ui::views::transport::show(&mut a, ui));
        out.textures_delta.clear();
    }
    assert!(a.show_settings, "the settings opened");
    assert!(!a.controllers.learn_mode, "no learn mode without a port");
}
