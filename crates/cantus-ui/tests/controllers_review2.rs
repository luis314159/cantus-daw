//! Review 2 of ADR 0062 (round 29): each confirmed finding about the interface's
//! controllers after the first fix round, held by a test — a gesture and the history under
//! it, recording's bounds, learning, feedback's echo, 14-bit pairs, frozen targets, the
//! global file, the words.

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
    keys_app_with(cantus_ui::starter_project())
}

fn keys_app_with(p: Project) -> CantusApp {
    let mut a = app(p);
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

/// A pan knob on CC 10 and a width knob on CC 11, both on the first strip.
fn two_knobs() -> (CantusApp, StripId) {
    two_knobs_in(cantus_ui::starter_project())
}

fn two_knobs_in(p: Project) -> (CantusApp, StripId) {
    let mut a = keys_app_with(p);
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
    (a, s)
}

// ---- a gesture and the history under it -------------------------------------------------

/// #0, #19, #23, #40 (high): an undo in the middle of a two-knob gesture sticks — the next
/// move of one knob does not bring the other's undone value back, and redo is kept.
#[test]
fn an_undo_during_a_gesture_sticks() {
    let (mut a, s) = two_knobs();
    let before = a.project().strip(s).unwrap().clone();
    a.controller_heard(&[cc(10, 120), cc(11, 100)]);
    a.controller_heard(&[cc(10, 110), cc(11, 90)]);
    assert_ne!(a.project().strip(s).unwrap().pan, before.pan);
    a.undo();
    assert_eq!(a.project().strip(s).unwrap().pan, before.pan);
    a.controller_heard(&[cc(11, 80)]);
    assert_eq!(
        a.project().strip(s).unwrap().pan,
        before.pan,
        "the undone pan stays undone"
    );
}

/// #19, #23: a mouse edit of a target in the gesture is not overwritten by the next frame
/// of the other knob.
#[test]
fn a_mouse_edit_during_a_gesture_sticks() {
    let (mut a, s) = two_knobs();
    a.controller_heard(&[cc(10, 120), cc(11, 100)]);
    a.exec(Command::SetStripPan { id: s, pan: -0.25 });
    a.controller_heard(&[cc(11, 90)]);
    assert_eq!(a.project().strip(s).unwrap().pan, -0.25);
}

/// #20, #53: a gesture sealed while a mouse step was on top, brought back to the top by an
/// undo, never takes a later, separate turn of the same knob into its step.
#[test]
fn a_later_turn_is_its_own_step_after_an_undo() {
    let (mut a, s) = two_knobs();
    let start = a.bus.history().len();
    a.controller_heard(&[cc(10, 120)]);
    a.exec(Command::SetMetronome {
        enabled: !a.project().metronome,
    });
    // The gesture idles out.
    a.controllers.gesture.last = Some(Instant::now() - Duration::from_secs(2));
    a.controller_heard(&[]);
    a.undo();
    assert_eq!(a.bus.history().len(), start + 1);
    a.controller_heard(&[cc(10, 20)]);
    assert_eq!(a.bus.history().len(), start + 2, "two turns, two steps");
    a.undo();
    assert!(a.project().strip(s).unwrap().pan > 0.5, "the first turn stays");
}

// ---- recording's bounds -------------------------------------------------------------------

fn automation_clips(a: &CantusApp, target: &AutoTarget) -> Vec<Clip> {
    a.project()
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter(|c| c.automation.as_ref().is_some_and(|x| &x.target == target))
        .cloned()
        .collect()
}

/// #17 (high): a knob already turning when a pass starts at tick 0 — its first message
/// older than the play — does not lose the pass's automation.
#[test]
fn a_move_heard_before_a_pass_at_zero_keeps_the_pass() {
    let (mut a, s) = two_knobs();
    let target = AutoTarget::StripPan(s);
    a.engine.seek(Ticks::ZERO);
    a.start_recording();
    a.engine.set_heard_for_tests(Ticks(20), true);
    a.controller_heard(&[msg_at(
        0xB0,
        10,
        30,
        Instant::now() - Duration::from_millis(500),
    )]);
    a.engine.set_heard_for_tests(Ticks::beats(2), true);
    a.controller_heard(&[cc(10, 100)]);
    a.engine.set_heard_for_tests(Ticks::beats(4), false);
    a.stop_recording();
    let clips = automation_clips(&a, &target);
    assert_eq!(clips.len(), 1, "the pass was kept");
    assert!(clips[0].start.0 >= 0);
}

/// #17 (high): moves during the count-in are not written before the pass (nor fail it).
#[test]
fn a_move_during_the_count_in_is_not_written_before_the_pass() {
    let mut p = cantus_ui::starter_project();
    p.recording.count_in_bars = 1;
    let (mut a, s) = two_knobs_in(p);
    let target = AutoTarget::StripPan(s);
    a.engine.seek(Ticks::beats(8));
    a.start_recording();
    // The count-in: a bar before the record start.
    a.engine.set_heard_for_tests(Ticks::beats(5), true);
    a.controller_heard(&[cc(10, 10)]);
    a.engine.set_heard_for_tests(Ticks::beats(10), true);
    a.controller_heard(&[cc(10, 100)]);
    a.engine.set_heard_for_tests(Ticks::beats(12), false);
    a.stop_recording();
    let clips = automation_clips(&a, &target);
    assert_eq!(clips.len(), 1);
    assert!(
        clips[0].start >= Ticks::beats(8),
        "from the record start: {:?}",
        clips[0].start
    );
}

/// #7, #18: the first entry into a loop is no wrap: a move a moment before the loop's start
/// does not land at its end.
#[test]
fn the_first_entry_into_a_loop_is_no_wrap() {
    let mut p = cantus_ui::starter_project();
    p.loop_region = Some(LoopRegion {
        start: Ticks::beats(16),
        end: Ticks::beats(32),
    });
    p.loop_enabled = true;
    let (mut a, s) = two_knobs_in(p);
    let target = AutoTarget::StripPan(s);
    a.engine.seek(Ticks::beats(16));
    a.start_recording();
    a.engine.set_heard_for_tests(Ticks::beats(16) + Ticks(10), true);
    a.controller_heard(&[msg_at(
        0xB0,
        10,
        30,
        Instant::now() - Duration::from_millis(30),
    )]);
    let points = a.modulators.rec.points.get(&target).cloned().unwrap();
    assert!(
        points.iter().all(|(t, _)| *t < Ticks::beats(24).0),
        "not at the loop's end: {points:?}"
    );
}

/// #7, #18: after a real wrap, a message heard before it still lands at the loop's end.
#[test]
fn a_message_before_a_real_wrap_lands_at_the_loops_end() {
    use cantus_ui::controllers_input::tick_in_loop;
    let (start, end) = (Ticks::beats(16).0, Ticks::beats(32).0);
    let lp = Some((start, end));
    assert_eq!(tick_in_loop(start - 10, start + 20, lp, true), end - 10);
    assert_eq!(tick_in_loop(start - 10, start + 20, lp, false), start - 10);
    assert_eq!(tick_in_loop(start + 5, start + 20, lp, true), start + 5);
    assert_eq!(tick_in_loop(start - 10, start + 20, None, true), start - 10);
}

/// #21: the modulator amount row's mouse is read back like the controller's: moved while a
/// pass records, it is recorded, and it is *last tweaked*.
#[test]
fn a_modulator_amount_moved_with_the_mouse_is_recorded() {
    let mut a = keys_app();
    let t = a.project().tracks[0].id;
    let id = a
        .add_modulator(modulator::ModHost::Track(t), ModKind::Lfo, None)
        .unwrap();
    a.engine.seek(Ticks::beats(1));
    a.start_recording();
    let mut m = a.project().modulator(id).unwrap().2.clone();
    m.amount = 0.25;
    assert!(a.edit_modulator(m, Some(MergeKey::of(&"amount-drag"))));
    assert!(a
        .modulators
        .rec
        .points
        .contains_key(&AutoTarget::ModAmount(id)));
    assert_eq!(a.automation.last_tweaked, Some(AutoTarget::ModAmount(id)));
}

// ---- learning -----------------------------------------------------------------------------

fn with_instrument(plugin: &ExternalPlugin) -> (CantusApp, TrackId) {
    let mut p = cantus_ui::starter_project();
    let t = p.tracks[0].id;
    p.track_mut(t).unwrap().instrument = InstrumentSpec::External(plugin.clone());
    (keys_app_with(p), t)
}

/// #1, #24, #51: after the instrument is swapped, its parameter learnt again is a mapping of
/// the new plug-in, alive; the old one is left for an undo of the swap, and it is not what
/// the mapped mark or *Forget* see.
#[test]
fn learning_a_swapped_plug_ins_parameter_makes_a_live_mapping() {
    use cantus_core::external::PluginFormat;
    let vital = ExternalPlugin::new(PluginFormat::Clap, "audio.vital", "Vital");
    let surge = ExternalPlugin::new(PluginFormat::Clap, "org.surge", "Surge");
    let (mut a, t) = with_instrument(&vital);
    let target = MappingTarget::Param(AutoTarget::PluginInstrument { track: t, id: 17 });
    let mut m = Mapping::new(
        MappingId(1),
        source(Control::Cc(1)),
        target.clone(),
        MapMode::Absolute,
    );
    m.plugin = target.plugin_of(a.project());
    a.add_mapping(Scope::Project, m);
    assert!(a.is_mapped(&target));
    a.exec(Command::SetInstrument {
        track: t,
        spec: Box::new(InstrumentSpec::External(surge.clone())),
    });
    assert!(
        !a.is_mapped(&target),
        "the old plug-in's mapping is not this one's"
    );
    a.learn_target(target.clone());
    a.controller_heard(&(0..8).map(|v| cc(30, 60 + v)).collect::<Vec<_>>());
    let maps = &a.project().controllers.mappings;
    assert_eq!(maps.len(), 2, "a new mapping, the old one kept: {maps:?}");
    let new = maps
        .iter()
        .find(|m| m.source.control == Control::Cc(30))
        .unwrap();
    assert_eq!(new.plugin, Some(PluginRef::of(&surge)));
    assert!(new.fits(a.project()));
    a.forget_target(&target);
    let maps = &a.project().controllers.mappings;
    assert_eq!(maps.len(), 1);
    assert_eq!(
        maps[0].plugin,
        Some(PluginRef::of(&vital)),
        "the old one stays"
    );
}

/// #1, #24: learning again a mapping of the plug-in there now keeps it that plug-in's.
#[test]
fn learning_again_sets_the_plug_in() {
    use cantus_core::external::PluginFormat;
    let vital = ExternalPlugin::new(PluginFormat::Clap, "audio.vital", "Vital");
    let (mut a, t) = with_instrument(&vital);
    let target = MappingTarget::Param(AutoTarget::PluginInstrument { track: t, id: 17 });
    // Made before the plug-in was known (an older build): no plug-in.
    let m = Mapping::new(
        MappingId(1),
        source(Control::Cc(1)),
        target.clone(),
        MapMode::Absolute,
    );
    a.add_mapping(Scope::Project, m);
    a.learn_target(target.clone());
    a.controller_heard(&(0..8).map(|v| cc(31, 60 + v)).collect::<Vec<_>>());
    let maps = &a.project().controllers.mappings;
    assert_eq!(maps.len(), 1, "learnt again");
    assert_eq!(maps[0].source.control, Control::Cc(31));
    assert_eq!(maps[0].plugin, Some(PluginRef::of(&vital)));
}

/// #2, #41: a momentary controller button held past the learn window is learnt as a toggle
/// once let go; one that is never let go is a latching button (absolute).
#[test]
fn a_held_momentary_button_is_learnt_as_a_toggle() {
    let mut a = keys_app();
    let t = a.project().tracks[0].id;
    a.learn_target(MappingTarget::TrackMute(t));
    a.controller_heard(&[cc(20, 127)]);
    a.controllers.learning.as_mut().unwrap().first =
        Some(Instant::now() - Duration::from_millis(500));
    a.controller_heard(&[]);
    assert!(
        a.controllers.learning.is_some(),
        "still waiting for the release"
    );
    a.controller_heard(&[cc(20, 0)]);
    assert!(a.controllers.learning.is_none());
    let m = a.project().controllers.mappings[0].clone();
    assert_eq!(
        (m.source.control, m.mode),
        (Control::Cc(20), MapMode::Toggle)
    );
    // Never let go: latching.
    a.learn_target(MappingTarget::TrackSolo(t));
    a.controller_heard(&[cc(21, 127)]);
    a.controllers.learning.as_mut().unwrap().first =
        Some(Instant::now() - Duration::from_secs(2));
    a.controller_heard(&[]);
    let m = a
        .project()
        .controllers
        .mappings
        .iter()
        .find(|m| m.source.control == Control::Cc(21))
        .unwrap()
        .clone();
    assert_eq!(m.mode, MapMode::Absolute);
}

/// #6: a keyboard's program button, bank select first, is learnt as the program change at
/// once.
#[test]
fn a_program_button_with_bank_select_is_learnt_as_the_program() {
    let mut a = keys_app();
    let t = a.project().tracks[0].id;
    a.learn_target(MappingTarget::TrackMute(t));
    a.controller_heard(&[cc(0, 0), cc(32, 0), msg_at(0xC0, 4, 0, Instant::now())]);
    assert!(a.controllers.learning.is_none(), "learnt at once");
    let m = a.project().controllers.mappings[0].clone();
    assert_eq!(m.source.control, Control::Program(4));
}

/// A feedback mapping on CC 10 (the strip's pan) whose value has just been sent.
fn fed_back() -> (CantusApp, StripId, Vec<u8>, Instant) {
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
    let sent_at = Instant::now();
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
    (a, s, sent, sent_at)
}

/// #4, #12, #27: learning takes no feedback echo for the control being learnt.
#[test]
fn learning_takes_no_feedback_echo() {
    let (mut a, s, sent, sent_at) = fed_back();
    a.learn_target(MappingTarget::Param(AutoTarget::StripWidth(s)));
    let mut heard = vec![msg_at(
        sent[0],
        sent[1],
        sent[2],
        sent_at + Duration::from_millis(1),
    )];
    heard.extend((0..8).map(|v| cc(22, 60 + v)));
    a.controller_heard(&heard);
    let m = a
        .project()
        .controllers
        .mappings
        .iter()
        .find(|m| m.target == MappingTarget::Param(AutoTarget::StripWidth(s)))
        .unwrap()
        .clone();
    assert_eq!(m.source.control, Control::Cc(22));
}

// ---- feedback's echo ----------------------------------------------------------------------

/// #3, #10, #28, #31: a real press of the same bytes as the feedback just sent, a little
/// later by its own time, is a press; an echo is judged by when it arrived, however late the
/// interface reads it.
#[test]
fn a_press_after_feedback_is_a_press_and_a_late_read_echo_is_an_echo() {
    let (mut a, s, sent, sent_at) = fed_back();
    // The echo arrives at once; the pan moves meanwhile; the echo is read 200 ms later.
    let arrived = Instant::now();
    a.exec(Command::SetStripPan { id: s, pan: 0.9 });
    std::thread::sleep(Duration::from_millis(200));
    a.controller_heard(&[msg_at(sent[0], sent[1], sent[2], arrived.max(sent_at))]);
    assert_eq!(
        a.project().strip(s).unwrap().pan,
        0.9,
        "the echo moved nothing"
    );
    // The same bytes again, a hand's time later: a move.
    a.controllers.feedback_log = Some(Vec::new());
    a.exec(Command::SetStripPan { id: s, pan: -1.0 });
    a.controllers.activity.clear();
    a.controller_input();
    let again = a
        .controllers
        .feedback_log
        .clone()
        .unwrap()
        .last()
        .unwrap()
        .1
        .clone();
    a.exec(Command::SetStripPan { id: s, pan: 0.5 });
    let later = Instant::now() + Duration::from_millis(100);
    a.controller_heard(&[msg_at(again[0], again[1], again[2], later)]);
    assert!(
        (a.project().strip(s).unwrap().pan + 1.0).abs() < 1e-3,
        "a real press of the same bytes is a move: {}",
        a.project().strip(s).unwrap().pan
    );
}

/// #11, #26: feedback sent for a long time with nothing coming in remembers a bounded few.
#[test]
fn the_echo_list_stays_bounded() {
    let (mut a, s, _, _) = fed_back();
    for i in 0..600 {
        let pan = (i % 200) as f32 / 100.0 - 1.0;
        a.exec(Command::SetStripPan { id: s, pan });
        a.controllers.activity.clear();
        a.controller_input();
    }
    assert!(
        a.controllers.echoes.len() <= cantus_ui::controllers_feedback::ECHO_MAX,
        "{}",
        a.controllers.echoes.len()
    );
}

// ---- 14-bit pairs -------------------------------------------------------------------------

fn wide_fader() -> CantusApp {
    let mut a = keys_app();
    add(
        &mut a,
        Scope::Global,
        1,
        Control::Cc14(7),
        MappingTarget::Param(AutoTarget::MasterWidth),
        MapMode::Absolute,
    );
    a
}

fn width_of(v: u16) -> f32 {
    f32::from(v) / 16_383.0 * 2.0
}

/// #5, #22, #38: once a fine half has been seen, a coarse half alone is read with a fine 0;
/// a burst never shows a half-made value; a controller that sends its fine half first is
/// read whole.
#[test]
fn a_14_bit_pair_reads_a_coarse_half_alone_and_a_fine_half_first() {
    let mut a = wide_fader();
    a.controller_heard(&[cc(7, 1), cc(39, 3)]);
    assert!((a.project().master.width - width_of(131)).abs() < 1e-3);
    a.controller_heard(&[cc(7, 0)]);
    assert_eq!(a.project().master.width, 0.0, "to the bottom");
    a.controller_heard(&[cc(7, 64), cc(39, 5)]);
    assert!((a.project().master.width - width_of(64 << 7 | 5)).abs() < 1e-3);
    // Fine half first (a moment after the pairs above).
    let t = Instant::now() + Duration::from_millis(50);
    a.controller_heard(&[msg_at(0xB0, 39, 9, t), msg_at(0xB0, 7, 100, t)]);
    assert!(
        (a.project().master.width - width_of(100 << 7 | 9)).abs() < 1e-3,
        "{}",
        a.project().master.width
    );
}

// ---- frozen targets -----------------------------------------------------------------------

/// A project whose first strip has a delay and is frozen.
fn frozen_strip() -> (CantusApp, StripId, EffectId) {
    let mut p = cantus_ui::starter_project();
    let s = p.tracks[0].strip.unwrap();
    let delay = p.make_effect(EffectKind::Delay);
    let fx = delay.id;
    p.strip_mut(s).unwrap().effects.push(delay);
    let content =
        cantus_core::freeze::content_key(&p, cantus_core::freeze::FreezeTarget::Strip(s))
            .expect("a key");
    p.strip_mut(s).unwrap().frozen = Some(cantus_core::freeze::Frozen { content });
    (keys_app_with(p), s, fx)
}

/// #25: a knob on a frozen sound and one on a live target in one gesture: the live one moves
/// every frame, the refusal is said once in words.
#[test]
fn a_frozen_target_in_a_gesture_leaves_the_others_moving() {
    let (mut a, _s, fx) = frozen_strip();
    let key = EffectKind::Delay.params()[0].key;
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(10),
        MappingTarget::Param(AutoTarget::Effect {
            effect: fx,
            key: key.into(),
        }),
        MapMode::Absolute,
    );
    add(
        &mut a,
        Scope::Project,
        2,
        Control::Cc(11),
        MappingTarget::Param(AutoTarget::MasterPan),
        MapMode::Absolute,
    );
    for v in [10u8, 40, 90, 120] {
        a.controller_heard(&[cc(10, v), cc(11, v)]);
        let want = f32::from(v) / 127.0 * 2.0 - 1.0;
        assert!(
            (a.project().master.pan - want).abs() < 1e-3,
            "the live knob moved"
        );
    }
    assert!(a.freeze.refusal.is_some(), "said");
    assert!(
        a.status.error && !a.status.text.contains("StripId"),
        "{}",
        a.status.text
    );
}

/// #54, #49: a hosted mapping on a frozen track is still the map's (its control reaches no
/// other instrument), and the pane says *Frozen*, not *Missing*; a plug-in not loaded says
/// so; a target gone is *Missing*.
#[test]
fn a_hosted_mapping_on_a_frozen_track_is_frozen_not_missing() {
    use cantus_core::external::PluginFormat;
    use cantus_ui::controllers::Idle;
    let vital = ExternalPlugin::new(PluginFormat::Clap, "audio.vital", "Vital");
    let mut p = cantus_ui::starter_project();
    let t = p.tracks[0].id;
    p.track_mut(t).unwrap().instrument = InstrumentSpec::External(vital.clone());
    let target = MappingTarget::Param(AutoTarget::PluginInstrument { track: t, id: 5 });
    let mut m = Mapping::new(
        MappingId(1),
        source(Control::Cc(74)),
        target.clone(),
        MapMode::Absolute,
    );
    m.plugin = Some(PluginRef::of(&vital));
    p.controllers.mappings.push(m.clone());
    let unfrozen = keys_app_with(p.clone());
    assert_eq!(
        unfrozen.mapping_state(Scope::Project, &m),
        Some(Idle::NotLoaded)
    );
    let ft = cantus_core::freeze::FreezeTarget::Track(t);
    let content = cantus_core::freeze::content_key(&p, ft).expect("a key");
    p.track_mut(t).unwrap().frozen = Some(cantus_core::freeze::Frozen { content });
    let mut a = keys_app_with(p);
    assert!(a.mapping_alive(Scope::Project, &m));
    assert_eq!(a.mapping_state(Scope::Project, &m), Some(Idle::Frozen));
    a.publish_controller_filter();
    let ev = MidiEvent {
        status: 0xB0,
        data1: 74,
        data2: 1,
    };
    assert!(a.controllers.tap.filter().takes(0, &ev), "still the map's");
    a.controller_heard(&[cc(74, 100)]);
    assert!(a.freeze.refusal.is_some(), "the move is refused and said");
    let gone = Mapping {
        target: MappingTarget::TrackMute(TrackId(987_654)),
        ..m
    };
    assert_eq!(a.mapping_state(Scope::Project, &gone), Some(Idle::Missing));
}

// ---- the global file ----------------------------------------------------------------------

/// #9, #29, #39: a file that is no text, one from a newer Cantus, one with a mapping this
/// build cannot read: each is said and kept beside before anything writes over it.
#[test]
fn a_global_file_that_loses_anything_is_kept_beside() {
    use cantus_ui::controllers::{global_path, load_global_checked};
    let dir = tempfile::tempdir().unwrap();
    let path = global_path(dir.path());
    std::fs::write(&path, [0xFFu8, 0xFE, b'{']).unwrap();
    let (map, damaged) = load_global_checked(&path);
    assert!(map.is_empty() && damaged);
    assert!(path.with_extension("json.bad").exists());
    // A mapping this build cannot read, beside one it can.
    let good = serde_json::to_value(Mapping::new(
        MappingId(1),
        source(Control::Cc(1)),
        MappingTarget::Tempo,
        MapMode::Absolute,
    ))
    .unwrap();
    let text = serde_json::json!({
        "version": 1,
        "mappings": [good, {"id": 2, "target": {"FromTheFuture": 3}}],
    })
    .to_string();
    std::fs::write(&path, &text).unwrap();
    let (map, damaged) = load_global_checked(&path);
    assert_eq!(map.mappings.len(), 1, "what could be read");
    assert!(damaged, "said");
    let kept = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().ends_with(".bad"))
        .any(|e| std::fs::read_to_string(e.path()).ok().as_deref() == Some(text.as_str()));
    assert!(kept, "the whole file kept beside, the earlier .bad too");
    // A newer version.
    std::fs::write(&path, r#"{"version": 99, "mappings": []}"#).unwrap();
    assert!(load_global_checked(&path).1);
}

// ---- words and states ---------------------------------------------------------------------

/// #43: an action is named by its own group, as *Add a mapping* offers it.
#[test]
fn an_action_is_named_by_its_group() {
    let a = keys_app();
    let undo = cantus_ui::keymap::Action::Undo;
    let name = a.mapping_target_name(&MappingTarget::Action(undo.id().into()));
    let group = cantus_ui::i18n::t(undo.category().label_key());
    assert!(name.starts_with(&group), "{name} / {group}");
    assert_ne!(undo.category(), cantus_ui::keymap::Category::Transport);
}

/// #46: *Forget these controllers* on an insert names the insert.
#[test]
fn forgetting_an_inserts_controllers_names_the_insert() {
    let mut a = keys_app();
    let s = strip(&a);
    a.add_effect(s, EffectKind::Compressor);
    let fx = a.project().strip(s).unwrap().effects.last().unwrap().id;
    add(
        &mut a,
        Scope::Project,
        1,
        Control::Cc(21),
        MappingTarget::EffectMix(fx),
        MapMode::Absolute,
    );
    a.forget_slot(fx);
    assert!(a.project().controllers.is_empty());
    let name = a.effect_display_name(fx);
    let bypass = cantus_ui::i18n::t("ctl.target.bypass");
    assert!(a.status.text.contains(&name), "{}", a.status.text);
    assert!(!a.status.text.contains(&bypass), "{}", a.status.text);
}

/// #50: every toggle *Add a mapping* offers that has a state feeds it back.
#[test]
fn every_toggle_with_a_state_feeds_it_back() {
    use cantus_ui::keymap::Action;
    let mut a = keys_app();
    for (action, flip) in [
        (Action::TogglePunch, true),
        (Action::ToggleCountIn, true),
        (Action::ToggleTypingPiano, true),
        (Action::ToggleMeterWave, true),
        (Action::ArmTrack, false),
        (Action::ToggleStripMute, false),
    ] {
        let before = a.action_state(action.id());
        assert!(before.is_some(), "{action:?} has a state");
        if flip {
            a.run_action(action);
            assert_ne!(a.action_state(action.id()), before, "{action:?} follows");
        }
    }
    assert!(a.action_state(Action::Stop.id()).is_none());
}

/// #35: the curve cell's words: percent, either sign, a comma.
#[test]
fn a_curve_is_typed_in_percent() {
    use cantus_ui::views::controllers::parse_curve;
    assert_eq!(parse_curve("+50"), Some(0.5));
    assert_eq!(parse_curve("-35 %"), Some(-0.35));
    assert_eq!(parse_curve("12,5"), Some(0.125));
    assert_eq!(parse_curve("400"), Some(1.0));
    assert_eq!(parse_curve("loud"), None);
}

/// #34, #20: a tempo knob turned across a tempo point moves each point it governs in one
/// step, the batch not growing frame by frame; one undo puts both back.
#[test]
fn a_tempo_gesture_across_a_point_is_one_step() {
    let mut p = cantus_ui::starter_project();
    p.tempo_points = vec![
        cantus_core::tempo::TempoPoint {
            tick: Ticks::ZERO,
            bpm: p.tempo_bpm,
            ramp: false,
        },
        cantus_core::tempo::TempoPoint {
            tick: Ticks::beats(8),
            bpm: 140.0,
            ramp: false,
        },
    ];
    let mut a = keys_app_with(p);
    add(
        &mut a,
        Scope::Global,
        1,
        Control::Cc(16),
        MappingTarget::Tempo,
        MapMode::Relative(Encoding::TwosComplement),
    );
    let before = a.project().clone();
    let steps = a.bus.history().len();
    a.engine.seek(Ticks::beats(2));
    for _ in 0..3 {
        a.controller_heard(&[cc(16, 1)]);
    }
    a.engine.seek(Ticks::beats(10));
    for _ in 0..3 {
        a.controller_heard(&[cc(16, 1)]);
    }
    let size = |a: &CantusApp| match &a.bus.history().last().unwrap().command {
        Command::Batch { commands, .. } => commands.len(),
        _ => 1,
    };
    assert_eq!(a.bus.history().len(), steps + 1, "one step");
    assert_eq!(size(&a), 2, "one command per point");
    let moved = a.project().tempo_points.clone();
    assert_ne!(moved[0].bpm, before.tempo_points[0].bpm);
    assert_ne!(moved[1].bpm, before.tempo_points[1].bpm);
    for _ in 0..5 {
        a.controller_heard(&[cc(16, 1)]);
    }
    assert_eq!(size(&a), 2, "no growth");
    assert_eq!(
        a.project().tempo_points[0].bpm,
        moved[0].bpm,
        "the first point, not turned since, kept exactly"
    );
    a.undo();
    assert_eq!(a.project().tempo_points, before.tempo_points);
}

// ---- the clocks a controller needs ---------------------------------------------------------

/// The frame the ticker asks for after `f` runs inside a frame.
fn asked_after(a: &mut CantusApp, f: impl FnOnce(&mut CantusApp)) -> Option<Duration> {
    let ctx = egui::Context::default();
    a.attach(&ctx);
    let mut f = Some(f);
    let mut out = ctx.run_ui(egui::RawInput::default(), |_ui| {
        if let Some(f) = f.take() {
            f(a);
        }
        a.controller_input();
        cantus_ui::views::workspace_ticker::finish(&ctx);
    });
    out.textures_delta.clear();
    cantus_ui::views::workspace_ticker::last(&ctx).requested
}

/// #33: an idle window is woken when a controller's clock runs out: a learn's window, an
/// armed learn's timeout, a gesture's end.
#[test]
fn an_idle_window_is_woken_for_the_controllers_clocks() {
    use cantus_ui::controllers_input::GESTURE_IDLE;
    use cantus_ui::controllers_learn::{LEARN_TIMEOUT, LEARN_WINDOW};
    let (mut a, s) = two_knobs();
    a.learn_target(MappingTarget::Param(AutoTarget::StripWidth(s)));
    let asked = asked_after(&mut a, |a| a.controller_heard(&[cc(40, 1), cc(40, 2)]));
    assert!(
        asked.is_some_and(|d| d <= LEARN_WINDOW),
        "the learn window: {asked:?}"
    );
    a.cancel_learning();
    a.learn_target(MappingTarget::Param(AutoTarget::StripWidth(s)));
    let asked = asked_after(&mut a, |_| {});
    assert!(
        asked.is_some_and(|d| d <= LEARN_TIMEOUT && d > LEARN_WINDOW),
        "the timeout: {asked:?}"
    );
    a.cancel_learning();
    let asked = asked_after(&mut a, |a| a.controller_heard(&[cc(10, 30)]));
    assert!(
        asked.is_some_and(|d| d <= GESTURE_IDLE),
        "the gesture's end: {asked:?}"
    );
}

// ---- learn offered everywhere --------------------------------------------------------------

/// #36, #27: every channel rack row's pan and volume knobs and its lamp offer learning.
#[test]
fn the_channel_rack_offers_learn_on_every_row() {
    use cantus_ui::kit::automate::probe;
    let mut a = keys_app();
    a.toggle_learn_mode();
    probe::start();
    paint(&ctx(), egui::vec2(1400.0, 900.0), 3, |ui| {
        cantus_ui::views::channel_rack::show(&mut a, ui)
    });
    let drawn = probe::take_learn();
    let tracks: Vec<TrackId> = a.project().tracks.iter().map(|t| t.id).collect();
    for t in tracks.iter().take(4) {
        for want in [
            format!("rack:{:?}", AutoTarget::TrackPan(*t)),
            format!("rack:{:?}", AutoTarget::TrackGain(*t)),
            format!("spot:{:?}", MappingTarget::TrackMute(*t)),
        ] {
            assert!(
                drawn.iter().any(|(n, l)| *n == want && *l),
                "{want}: {drawn:?}"
            );
        }
    }
}

/// #36, #45: the mixer offers learning on every strip's fader, pan and width and its mute
/// and solo lamps — not merely somewhere.
#[test]
fn the_mixer_offers_learn_on_every_strips_controls() {
    use cantus_ui::kit::automate::probe;
    let mut a = keys_app();
    a.toggle_learn_mode();
    probe::start();
    paint(&ctx(), egui::vec2(1600.0, 700.0), 3, |ui| {
        cantus_ui::views::mixer::show(&mut a, ui)
    });
    let drawn = probe::take_learn();
    let strips: Vec<StripId> = a.project().tracks.iter().filter_map(|t| t.strip).collect();
    for s in strips.iter().take(4) {
        for want in [
            format!("desk:{:?}", AutoTarget::StripGain(*s)),
            format!("desk:{:?}", AutoTarget::StripPan(*s)),
            format!("desk:{:?}", AutoTarget::StripWidth(*s)),
            format!("spot:{:?}", MappingTarget::StripMute(*s)),
            format!("spot:{:?}", MappingTarget::StripSolo(*s)),
        ] {
            assert!(
                drawn.iter().any(|(n, l)| *n == want && *l),
                "{want}: {drawn:?}"
            );
        }
    }
}

/// #36: a modulator's amount row in the inspector offers learning.
#[test]
fn a_modulator_row_offers_learn() {
    use cantus_ui::kit::automate::probe;
    let mut a = keys_app();
    let t = a.project().tracks[0].id;
    a.select_track(t);
    let id = a
        .add_modulator(modulator::ModHost::Track(t), ModKind::Lfo, None)
        .unwrap();
    probe::start();
    paint(&ctx(), egui::vec2(420.0, 1400.0), 3, |ui| {
        cantus_ui::views::inspector::show(&mut a, ui)
    });
    let drawn = probe::take_learn();
    assert!(
        drawn.iter().any(|(n, l)| n.contains("amount") && *l),
        "{id:?}: {drawn:?}"
    );
}

/// #36, #47: a send's route menu offers learning for its level, named for the level.
#[test]
fn a_sends_menu_learns_its_level_by_name() {
    use cantus_core::routing::Route;
    use cantus_ui::views::route_menu;
    let mut a = keys_app();
    let s: Vec<StripId> = a.project().strips.iter().map(|s| s.id).collect();
    a.set_strip_send(
        s[1],
        Send {
            level: 0.5,
            ..Send::new(s[5])
        },
    );
    let owner = egui::Id::new("mixer.route_menu");
    let c = ctx();
    route_menu::open(&c, owner, Route::send(s[1], s[5]), egui::pos2(500.0, 300.0));
    paint(&c, egui::vec2(1600.0, 900.0), 3, |ui| {
        cantus_ui::views::mixer::show(&mut a, ui)
    });
    let rows = route_menu::row_labels(&c, owner);
    assert!(
        rows.iter().any(|r| r == "ctl.menu.send_learn"),
        "{rows:?}"
    );
}

/// #37, #46: a learn spot partly under its view's clip takes a click only inside it: below
/// the clip, the control under it gets the click.
#[test]
fn a_learn_spot_is_clipped_by_its_view() {
    let c = ctx();
    let mut a = keys_app();
    let s = strip(&a);
    a.toggle_learn_mode();
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(20.0, 40.0));
    let clip = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 40.0));
    let click = |c: &egui::Context, a: &mut CantusApp, at: egui::Pos2, base: f64| {
        let mut pressed = 0;
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
                    egui::vec2(200.0, 200.0),
                )),
                time: Some(base + i as f64 * 0.1),
                events,
                ..Default::default()
            };
            let mut out = c.run_ui(input, |ui| {
                let r = ui.interact(rect, egui::Id::new("the-lamp"), egui::Sense::click());
                if r.clicked() {
                    pressed += 1;
                }
                cantus_ui::controllers::learn_spot_in(
                    a,
                    ui,
                    rect,
                    clip,
                    MappingTarget::StripMute(s),
                );
            });
            out.textures_delta.clear();
        }
        pressed
    };
    // Below the clip: the lamp's own click.
    let pressed = click(&c, &mut a, egui::pos2(30.0, 52.0), 0.0);
    assert_eq!(pressed, 1, "the lamp under got the click");
    assert!(a.controllers.learning.is_none(), "nothing armed");
    // Inside: the spot's.
    let pressed = click(&c, &mut a, egui::pos2(30.0, 28.0), 1.0);
    assert_eq!(pressed, 0);
    assert_eq!(
        a.controllers.learning.as_ref().map(|l| l.target.clone()),
        Some(MappingTarget::StripMute(s))
    );
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

/// #36: a hosted plug-in's generic editor offers learning on every parameter it draws.
#[test]
fn a_hosted_editor_offers_learn_on_its_parameters() {
    use cantus_ui::kit::automate::probe;
    let mut a = keys_app();
    cantus_ui::views::plugins::demo_setup(&mut a, &test_plugin(), true);
    let effect = a.fx.open.map(|(_, e, _)| e).expect("the editor opened");
    a.toggle_learn_mode();
    probe::start();
    paint_editors(&mut a, egui::vec2(1600.0, 1000.0));
    let drawn = probe::take_learn();
    let hosted: Vec<_> = drawn
        .iter()
        .filter(|(n, _)| n.starts_with("hosted:"))
        .collect();
    let want = format!("hosted:{:?}", AutoTarget::Plugin { effect, id: 0 });
    assert!(
        hosted.iter().any(|(n, l)| *n == want && *l),
        "{want}: {drawn:?}"
    );
    assert!(hosted.iter().all(|(_, l)| *l), "{hosted:?}");
}
