//! The controller mappings' arithmetic, learning and commands (ADR 0062).

use super::learn::{detect, encoder};
use super::*;
use crate::automation::AutoTarget;

fn src(control: Control) -> MidiSource {
    MidiSource {
        port: PortRef {
            name: "Keys".into(),
            ordinal: 0,
        },
        channel: Some(0),
        control,
    }
}

fn knob(mode: MapMode) -> Mapping {
    Mapping::new(
        MappingId(1),
        src(Control::Cc(21)),
        MappingTarget::Param(AutoTarget::MasterPan),
        mode,
    )
}

#[test]
fn the_three_encodings_read_their_steps() {
    use Encoding::*;
    assert_eq!(
        (TwosComplement.steps(1), TwosComplement.steps(127)),
        (1, -1)
    );
    assert_eq!(
        (TwosComplement.steps(5), TwosComplement.steps(123)),
        (5, -5)
    );
    assert_eq!((BinaryOffset.steps(65), BinaryOffset.steps(63)), (1, -1));
    assert_eq!(BinaryOffset.steps(64), 0);
    assert_eq!((SignBit.steps(1), SignBit.steps(65)), (1, -1));
    assert_eq!((SignBit.steps(3), SignBit.steps(67)), (3, -3));
}

#[test]
fn shape_and_unshape_are_inverse_over_range_curve_and_inversion() {
    for (min, max, curve) in [
        (0.0, 1.0, 0.0),
        (0.2, 0.8, 0.0),
        (1.0, 0.0, 0.0),
        (0.1, 0.9, 0.6),
        (0.9, 0.3, -0.7),
    ] {
        let m = Mapping {
            min,
            max,
            curve,
            ..knob(MapMode::Absolute)
        };
        assert_eq!(m.shape(0.0), min);
        assert!((m.shape(1.0) - max).abs() < 1e-6);
        for i in 0..=20 {
            let x = i as f32 / 20.0;
            let back = m.unshape(m.shape(x));
            // Straight, x itself; where a steep bend flattens, an x that gives the same
            // output (they cannot be told apart in f32).
            if curve == 0.0 {
                assert!((back - x).abs() < 1e-5, "{min} {max}: {x} → {back}");
            }
            assert!(
                (m.shape(back) - m.shape(x)).abs() < 1e-6,
                "{min} {max} {curve}: {x} → {back}"
            );
        }
    }
    // A bend slows the start: a quarter turn gives less than a quarter.
    let bent = Mapping {
        curve: 0.5,
        ..knob(MapMode::Absolute)
    };
    assert!(bent.shape(0.25) < 0.25);
}

#[test]
fn jump_goes_where_the_control_is() {
    let m = Mapping {
        takeover: Takeover::Jump,
        ..knob(MapMode::Absolute)
    };
    let mut st = DriveState::default();
    assert_eq!(
        m.drive(Input::Abs(1.0), Some(0.0), &mut st),
        Drive::Set(1.0)
    );
}

#[test]
fn pick_up_never_jumps_and_takes_over_when_the_knob_passes_the_value() {
    let m = knob(MapMode::Absolute);
    assert_eq!(m.takeover, Takeover::Pickup);
    let mut st = DriveState::default();
    // The target is at 0.5; the knob at the bottom moves up — nothing until it reaches 0.5.
    for x in [0.0, 0.1, 0.3, 0.45] {
        assert_eq!(
            m.drive(Input::Abs(x), Some(0.5), &mut st),
            Drive::Nothing,
            "{x}"
        );
    }
    // It passes the value between two messages (a fast turn): picked up.
    assert_eq!(
        m.drive(Input::Abs(0.6), Some(0.5), &mut st),
        Drive::Set(0.6)
    );
    // From then on it follows.
    assert_eq!(
        m.drive(Input::Abs(0.7), Some(0.6), &mut st),
        Drive::Set(0.7)
    );
    // Something else moved the target (the mouse): picked up again only when passed.
    assert_eq!(
        m.drive(Input::Abs(0.72), Some(0.2), &mut st),
        Drive::Nothing
    );
    assert_eq!(m.drive(Input::Abs(0.5), Some(0.2), &mut st), Drive::Nothing);
    assert_eq!(
        m.drive(Input::Abs(0.19), Some(0.2), &mut st),
        Drive::Set(0.19)
    );
}

#[test]
fn scale_moves_with_the_control_and_meets_it_at_the_end() {
    let m = Mapping {
        takeover: Takeover::Scale,
        ..knob(MapMode::Absolute)
    };
    let mut st = DriveState::default();
    // The control at 0.8, the target at 0.2: the first message only sets where it is.
    assert_eq!(m.drive(Input::Abs(0.8), Some(0.2), &mut st), Drive::Nothing);
    let mut cur = 0.2;
    let mut x = 0.8;
    // Up to the top: the target covers what is left of its way, never jumping.
    while x < 1.0 {
        x = (x + 0.05f32).min(1.0);
        match m.drive(Input::Abs(x), Some(cur), &mut st) {
            Drive::Set(p) => {
                assert!(p > cur && p - cur < 0.25, "{cur} → {p}");
                cur = p;
            }
            d => panic!("{d:?}"),
        }
    }
    assert!((cur - 1.0).abs() < 1e-5, "they meet at the end: {cur}");
    // Now they agree: down moves it directly.
    assert_eq!(
        m.drive(Input::Abs(0.5), Some(cur), &mut st),
        Drive::Set(0.5)
    );
}

#[test]
fn a_relative_encoder_steps_from_where_the_target_is_within_its_range() {
    let m = Mapping {
        min: 0.25,
        max: 0.75,
        ..knob(MapMode::Relative(Encoding::TwosComplement))
    };
    let mut st = DriveState::default();
    let i = m.input(1, true);
    assert_eq!(i, Input::Steps(1));
    let Drive::Set(p) = m.drive(i, Some(0.5), &mut st) else {
        panic!()
    };
    assert!((p - (0.5 + 0.5 / 128.0)).abs() < 1e-6, "{p}");
    // Never past the range's end, whatever the steps.
    assert_eq!(
        m.drive(Input::Steps(-500), Some(0.5), &mut st),
        Drive::Set(0.25)
    );
    assert_eq!(
        m.drive(Input::Steps(500), Some(0.5), &mut st),
        Drive::Set(0.75)
    );
    // Sensitivity scales a step.
    let fast = Mapping {
        sensitivity: 4.0,
        ..knob(MapMode::Relative(Encoding::TwosComplement))
    };
    assert_eq!(
        fast.drive(Input::Steps(2), Some(0.0), &mut st),
        Drive::Set(8.0 / 128.0)
    );
}

#[test]
fn toggle_flips_on_each_press_and_ignores_the_release() {
    let mut m = Mapping::new(
        MappingId(2),
        src(Control::Note(36)),
        MappingTarget::TrackMute(crate::TrackId(5)),
        MapMode::Toggle,
    );
    let mut st = DriveState::default();
    assert_eq!(m.input(100, true), Input::Press(true));
    assert_eq!(m.input(0, false), Input::Press(false));
    assert_eq!(
        m.drive(Input::Press(true), Some(0.0), &mut st),
        Drive::Set(1.0)
    );
    assert_eq!(
        m.drive(Input::Press(false), Some(1.0), &mut st),
        Drive::Nothing
    );
    assert_eq!(
        m.drive(Input::Press(true), Some(1.0), &mut st),
        Drive::Set(0.0)
    );
    // A held press repeated (a controller resending 127) counts once.
    assert_eq!(
        m.drive(Input::Press(true), Some(0.0), &mut st),
        Drive::Nothing
    );
    // A controller as a button: pressed from its middle up.
    m.source.control = Control::Cc(64);
    assert_eq!(m.input(127, true), Input::Press(true));
    assert_eq!(m.input(0, true), Input::Press(false));
}

#[test]
fn momentary_holds_the_top_while_pressed() {
    let m = knob(MapMode::Momentary);
    let mut st = DriveState::default();
    assert_eq!(
        m.drive(Input::Press(true), Some(0.3), &mut st),
        Drive::Set(1.0)
    );
    assert_eq!(
        m.drive(Input::Press(true), Some(1.0), &mut st),
        Drive::Nothing
    );
    assert_eq!(
        m.drive(Input::Press(false), Some(1.0), &mut st),
        Drive::Set(0.0)
    );
}

#[test]
fn an_action_fires_once_per_press() {
    let m = Mapping::new(
        MappingId(3),
        src(Control::Cc(41)),
        MappingTarget::Action("play-pause".into()),
        MapMode::Trigger,
    );
    let mut st = DriveState::default();
    assert_eq!(m.drive(m.input(127, true), None, &mut st), Drive::Fire);
    assert_eq!(m.drive(m.input(127, true), None, &mut st), Drive::Nothing);
    assert_eq!(m.drive(m.input(0, true), None, &mut st), Drive::Nothing);
    assert_eq!(m.drive(m.input(127, true), None, &mut st), Drive::Fire);
    // A program change is a press with no release.
    let p = Mapping {
        source: src(Control::Program(4)),
        ..m.clone()
    };
    let mut st = DriveState::default();
    assert_eq!(p.drive(p.input(0, true), None, &mut st), Drive::Fire);
}

#[test]
fn a_pad_on_a_parameter_sets_it_by_velocity_and_keeps_it_on_release() {
    let m = Mapping {
        source: src(Control::Note(40)),
        takeover: Takeover::Jump,
        ..knob(MapMode::Absolute)
    };
    let mut st = DriveState::default();
    let i = m.input(127, true);
    assert_eq!(m.drive(i, Some(0.0), &mut st), Drive::Set(1.0));
    assert_eq!(
        m.drive(m.input(0, false), Some(1.0), &mut st),
        Drive::Nothing
    );
}

#[test]
fn feedback_sends_the_controls_position_and_a_buttons_light() {
    let m = Mapping {
        min: 0.2,
        max: 0.6,
        ..knob(MapMode::Absolute)
    };
    assert_eq!(m.feedback_value(0.2), 0);
    assert_eq!(m.feedback_value(0.6), 127);
    assert!((63..=64).contains(&m.feedback_value(0.4)));
    let b = Mapping {
        source: src(Control::Note(36)),
        ..knob(MapMode::Toggle)
    };
    assert_eq!((b.feedback_value(1.0), b.feedback_value(0.0)), (127, 0));
    let wide = Mapping {
        source: src(Control::Cc14(7)),
        ..knob(MapMode::Absolute)
    };
    assert_eq!(wide.feedback_value(1.0), 16_383);
}

#[test]
fn learning_tells_the_kind_of_control() {
    let param = MappingTarget::Param(AutoTarget::MasterGain);
    let mute = MappingTarget::TrackMute(crate::TrackId(1));
    let play = MappingTarget::Action("play-pause".into());
    // A knob: absolute.
    let knob: Vec<(Control, u16)> = [60, 61, 63, 66].map(|v| (Control::Cc(21), v)).to_vec();
    assert_eq!(
        detect(&knob, &param),
        Some((Control::Cc(21), MapMode::Absolute))
    );
    // A 14-bit fader: the coarse controller and its fine twin.
    let fine = vec![
        (Control::Cc(7), 64),
        (Control::Cc(39), 12),
        (Control::Cc(7), 65),
    ];
    assert_eq!(
        detect(&fine, &param),
        Some((Control::Cc14(7), MapMode::Absolute))
    );
    // Encoders, by their bands.
    let twos: Vec<(Control, u16)> = [1, 1, 1, 2].map(|v| (Control::Cc(16), v)).to_vec();
    assert_eq!(
        detect(&twos, &param),
        Some((Control::Cc(16), MapMode::Relative(Encoding::TwosComplement)))
    );
    assert_eq!(encoder(&[127, 127, 126]), Some(Encoding::TwosComplement));
    assert_eq!(encoder(&[65, 65, 66]), Some(Encoding::BinaryOffset));
    assert_eq!(encoder(&[63, 63, 62]), Some(Encoding::BinaryOffset));
    assert_eq!(encoder(&[1, 1, 65, 65]), Some(Encoding::SignBit));
    // A knob turned slowly never repeats: not an encoder.
    assert_eq!(encoder(&[1, 2, 3, 4]), None);
    // A button: its two ends.
    let button = vec![(Control::Cc(64), 127), (Control::Cc(64), 0)];
    assert_eq!(
        detect(&button, &mute),
        Some((Control::Cc(64), MapMode::Toggle))
    );
    assert_eq!(
        detect(&button, &play),
        Some((Control::Cc(64), MapMode::Trigger))
    );
    // A pad: toggles a switch, sets a parameter by velocity, fires an action.
    let pad = vec![(Control::Note(36), 90)];
    assert_eq!(
        detect(&pad, &mute),
        Some((Control::Note(36), MapMode::Toggle))
    );
    assert_eq!(
        detect(&pad, &param),
        Some((Control::Note(36), MapMode::Absolute))
    );
    assert_eq!(
        detect(&pad, &play),
        Some((Control::Note(36), MapMode::Trigger))
    );
    assert_eq!(detect(&[], &param), None);
}

#[test]
fn a_port_is_known_by_name_and_ordinal_not_by_index() {
    let names: Vec<String> = ["Keys", "Pads", "Keys"].map(String::from).to_vec();
    let second = PortRef::of(&names, 2);
    assert_eq!((second.name.as_str(), second.ordinal), ("Keys", 1));
    // Plugged in again in another order: still the second "Keys".
    let after: Vec<String> = ["Pads", "Keys", "Keys", "Fader"].map(String::from).to_vec();
    assert_eq!(second.index_in(&after), Some(2));
    assert!(!second.matches(&after, 1));
    assert!(PortRef::any().matches(&after, 3));
}

#[test]
fn sources_overlap_through_any_port_and_channel_and_a_14_bit_pair() {
    let a = src(Control::Cc(21));
    let mut b = a.clone();
    b.channel = None;
    assert!(a.overlaps(&b));
    b.control = Control::Cc(22);
    assert!(!a.overlaps(&b));
    let pair = src(Control::Cc14(7));
    assert!(pair.overlaps(&src(Control::Cc(39))));
    assert!(pair.overlaps(&src(Control::Cc(7))));
    assert!(!pair.overlaps(&src(Control::Cc(8))));
}

#[test]
fn relative_targets_resolve_through_the_focus() {
    let t = crate::TrackId(3);
    let s = crate::StripId(9);
    let dev = [AutoTarget::MasterGain, AutoTarget::MasterPan];
    let focus = Focus {
        track: Some(t),
        strip: Some(s),
        master: false,
        device: &dev,
    };
    assert_eq!(
        MappingTarget::SelectedTrack(DeskParam::Volume).resolve(&focus),
        Some(MappingTarget::Param(AutoTarget::TrackGain(t)))
    );
    assert_eq!(
        MappingTarget::SelectedStrip(DeskParam::Mute).resolve(&focus),
        Some(MappingTarget::StripMute(s))
    );
    assert_eq!(
        MappingTarget::DeviceParam(1).resolve(&focus),
        Some(MappingTarget::Param(AutoTarget::MasterPan))
    );
    assert_eq!(MappingTarget::DeviceParam(2).resolve(&focus), None);
    assert_eq!(
        MappingTarget::SelectedTrack(DeskParam::Arm).resolve(&Focus::default()),
        None
    );
    assert!(MappingTarget::SelectedTrack(DeskParam::Arm).is_portable());
    assert!(!MappingTarget::TrackMute(t).is_portable());
    assert!(MappingTarget::Param(AutoTarget::MasterGain).is_portable());
}

fn project() -> Project {
    let mut p = Project::new("c");
    let t = p.make_track("T");
    p.tracks.push(t);
    p
}

#[test]
fn switch_targets_set_and_read_through_their_commands() {
    let mut p = project();
    let t = p.tracks[0].id;
    let s = p.strips[0].id;
    for target in [
        MappingTarget::TrackMute(t),
        MappingTarget::TrackSolo(t),
        MappingTarget::TrackArm(t),
        MappingTarget::StripMute(s),
        MappingTarget::StripSolo(s),
    ] {
        assert_eq!(target.current_pos(&p, Ticks::ZERO), Some(0.0));
        let cmd = target.command(&p, 1.0, Ticks::ZERO).unwrap();
        cmd.apply(&mut p).unwrap();
        assert_eq!(target.current_pos(&p, Ticks::ZERO), Some(1.0), "{target:?}");
        assert!(target.exists(&p));
    }
    assert!(!MappingTarget::TrackMute(crate::TrackId(999)).exists(&p));
    assert_eq!(
        MappingTarget::TrackMute(crate::TrackId(999)).command(&p, 1.0, Ticks::ZERO),
        None
    );
}

#[test]
fn the_tempo_target_sets_the_tempo_where_the_playhead_is() {
    let mut p = project();
    let mid = 0.5f32;
    let cmd = MappingTarget::Tempo.command(&p, mid, Ticks::ZERO).unwrap();
    assert_eq!(
        cmd,
        crate::tempo::edit::set_tempo_at(&p, Ticks::ZERO, 140.0)
    );
    cmd.apply(&mut p).unwrap();
    assert_eq!(p.tempo_bpm, 140.0);
    assert_eq!(MappingTarget::Tempo.current_pos(&p, Ticks::ZERO), Some(0.5));
}

fn mapping(id: u64) -> Mapping {
    Mapping {
        id: MappingId(id),
        ..knob(MapMode::Absolute)
    }
}

#[test]
fn the_maps_commands_have_exact_inverses_and_refuse_what_is_malformed() {
    let mut p = project();
    let before = p.clone();
    let inv = Command::InsertMapping {
        index: 5,
        mapping: Box::new(mapping(1)),
    }
    .apply(&mut p)
    .unwrap();
    let inv2 = Command::InsertMapping {
        index: 0,
        mapping: Box::new(mapping(2)),
    }
    .apply(&mut p)
    .unwrap();
    assert_eq!(
        p.controllers
            .mappings
            .iter()
            .map(|m| m.id.0)
            .collect::<Vec<_>>(),
        [2, 1]
    );
    let mid = p.clone();
    let mut changed = mapping(1);
    changed.feedback = true;
    changed.min = 0.3;
    let back = Command::SetMapping {
        mapping: Box::new(changed.clone()),
    }
    .apply(&mut p)
    .unwrap();
    assert_eq!(p.controllers.get(MappingId(1)), Some(&changed));
    back.apply(&mut p).unwrap();
    assert_eq!(p, mid);
    // Removing puts it back where it was.
    let restore = Command::RemoveMapping { id: MappingId(1) }
        .apply(&mut p)
        .unwrap();
    restore.apply(&mut p).unwrap();
    assert_eq!(p, mid);
    inv2.apply(&mut p).unwrap();
    inv.apply(&mut p).unwrap();
    assert_eq!(p, before);
    // Refused: a taken id, a range out of the travel, a bad channel, an unknown id.
    Command::InsertMapping {
        index: 0,
        mapping: Box::new(mapping(1)),
    }
    .apply(&mut p)
    .unwrap();
    let snapshot = p.clone();
    for bad in [
        mapping(1),
        Mapping {
            min: 1.5,
            ..mapping(4)
        },
        Mapping {
            curve: f32::NAN,
            ..mapping(4)
        },
        Mapping {
            sensitivity: 0.0,
            ..mapping(4)
        },
        Mapping {
            source: MidiSource {
                channel: Some(16),
                ..src(Control::Cc(1))
            },
            ..mapping(4)
        },
        Mapping {
            target: MappingTarget::DeviceParam(8),
            ..mapping(4)
        },
    ] {
        assert!(Command::InsertMapping {
            index: 0,
            mapping: Box::new(bad)
        }
        .apply(&mut p)
        .is_err());
    }
    assert!(Command::SetMapping {
        mapping: Box::new(mapping(77))
    }
    .apply(&mut p)
    .is_err());
    assert!(Command::RemoveMapping { id: MappingId(77) }
        .apply(&mut p)
        .is_err());
    assert_eq!(p, snapshot);
}

#[test]
fn a_mapping_drag_supersedes_per_mapping() {
    let a = Command::SetMapping {
        mapping: Box::new(mapping(1)),
    };
    let b = Command::SetMapping {
        mapping: Box::new(Mapping {
            min: 0.4,
            ..mapping(1)
        }),
    };
    let c = Command::SetMapping {
        mapping: Box::new(mapping(2)),
    };
    assert!(b.supersedes(&a));
    assert!(!c.supersedes(&a));
}

#[test]
fn a_project_without_mappings_saves_no_controllers_field() {
    let p = project();
    let json = serde_json::to_string(&p).unwrap();
    assert!(!json.contains("controllers"), "{json}");
    let mut q = p.clone();
    q.controllers.mappings.push(mapping(1));
    let json = serde_json::to_string(&q).unwrap();
    assert!(json.contains("controllers"));
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back, q);
}

// ---- review 1 (ADR 0062) ------------------------------------------------------------------

/// #0: a program change has no release: the same one pressed twice fires twice, toggles
/// twice.
#[test]
fn a_program_change_presses_every_time() {
    let fire = Mapping::new(
        MappingId(1),
        src(Control::Program(4)),
        MappingTarget::Action("play-pause".into()),
        MapMode::Trigger,
    );
    let mut st = DriveState::default();
    assert_eq!(fire.read(127, true, None, &mut st), Drive::Fire);
    assert_eq!(fire.read(127, true, None, &mut st), Drive::Fire);
    let toggle = Mapping {
        target: MappingTarget::TrackMute(crate::TrackId(1)),
        mode: MapMode::Toggle,
        ..fire.clone()
    };
    let mut st = DriveState::default();
    assert_eq!(toggle.read(127, true, Some(0.0), &mut st), Drive::Set(1.0));
    assert_eq!(toggle.read(127, true, Some(1.0), &mut st), Drive::Set(0.0));
}

/// #0: a CC button that only ever sends its press value (no 0): each press counts.
#[test]
fn a_cc_button_without_a_release_presses_every_time() {
    let m = Mapping::new(
        MappingId(1),
        src(Control::Cc(41)),
        MappingTarget::Action("stop".into()),
        MapMode::Trigger,
    );
    let mut st = DriveState::default();
    assert_eq!(m.read(127, true, None, &mut st), Drive::Fire);
    assert_eq!(m.read(127, true, None, &mut st), Drive::Fire);
    // A knob turned through the middle is not a stream of presses.
    let mut st = DriveState::default();
    assert_eq!(m.read(70, true, None, &mut st), Drive::Fire);
    assert_eq!(m.read(71, true, None, &mut st), Drive::Nothing);
    assert_eq!(m.read(72, true, None, &mut st), Drive::Nothing);
}

/// #2, #54: an encoder on a stepped target gathers its steps: the target, rounded after
/// every frame as the interface rounds it, still moves.
#[test]
fn an_encoder_moves_a_stepped_target_one_frame_at_a_time() {
    let m = knob(MapMode::Relative(Encoding::TwosComplement));
    let q = |p: f32| (p * 3.0).round() / 3.0; // four steps
    let mut st = DriveState::default();
    let mut now = 0.0f32;
    let mut ticks = 0;
    while now == 0.0 && ticks < 200 {
        if let Drive::Set(p) = m.read(1, true, Some(now), &mut st) {
            now = q(p);
            // What the interface does after applying: the target's rounded position.
            st.last_out = Some(now);
        }
        ticks += 1;
    }
    assert!((now - 1.0 / 3.0).abs() < 1e-6, "one step up: {now}");
    assert!(
        (20..=30).contains(&ticks),
        "after about 128/6 ticks: {ticks}"
    );
    // Something else moves it: the encoder steps from there.
    let d = m.read(1, true, Some(1.0), &mut st);
    assert_eq!(d, Drive::Set(1.0));
}

/// #2: a switch on an encoder flips with one turn either way.
#[test]
fn an_encoder_flips_a_switch_with_one_turn() {
    let m = Mapping::new(
        MappingId(1),
        src(Control::Cc(16)),
        MappingTarget::TrackMute(crate::TrackId(1)),
        MapMode::Relative(Encoding::TwosComplement),
    );
    let mut st = DriveState::default();
    assert_eq!(m.read(1, true, Some(0.0), &mut st), Drive::Set(1.0));
    assert_eq!(m.read(127, true, Some(1.0), &mut st), Drive::Set(0.0));
}

/// #6: a sign-bit encoder learnt clockwise (as two's complement) turned back: one step
/// back, not half the range — and from then on read as sign-bit.
#[test]
fn an_encoder_learnt_one_way_is_read_right_the_other_way() {
    let m = Mapping {
        takeover: Takeover::Jump,
        ..knob(MapMode::Relative(Encoding::TwosComplement))
    };
    let mut st = DriveState::default();
    let Drive::Set(a) = m.read(1, true, Some(0.5), &mut st) else {
        panic!()
    };
    st.last_out = Some(a);
    let Drive::Set(b) = m.read(65, true, Some(a), &mut st) else {
        panic!()
    };
    assert!((b - 0.5).abs() < 1e-5, "one step back: {a} → {b}");
    assert_eq!(st.encoding, Some(Encoding::SignBit));
    assert_eq!(
        Encoding::BinaryOffset.reinterpret(1),
        Some(Encoding::SignBit)
    );
    assert_eq!(
        Encoding::SignBit.reinterpret(127),
        Some(Encoding::TwosComplement)
    );
    assert_eq!(
        Encoding::SignBit.reinterpret(63),
        Some(Encoding::BinaryOffset)
    );
    assert_eq!(Encoding::TwosComplement.reinterpret(3), None);
}

/// #4: a pad on a parameter puts it at its velocity under the default pick up.
#[test]
fn a_pad_sets_its_velocity_under_pick_up() {
    let m = Mapping {
        source: src(Control::Note(40)),
        ..knob(MapMode::Absolute)
    };
    assert_eq!(m.takeover, Takeover::Pickup);
    let mut st = DriveState::default();
    assert_eq!(
        m.read(64, true, Some(0.8), &mut st),
        Drive::Set(64.0 / 127.0)
    );
    assert_eq!(
        m.read(76, true, Some(64.0 / 127.0), &mut st),
        Drive::Set(76.0 / 127.0)
    );
}

/// #10: a latching button (127 on one press, 0 on the next) is learnt to say what it is;
/// a momentary one (press and release heard) toggles.
#[test]
fn a_latching_button_is_learnt_absolute() {
    let mute = MappingTarget::TrackMute(crate::TrackId(1));
    assert_eq!(
        detect(&[(Control::Cc(50), 127)], &mute),
        Some((Control::Cc(50), MapMode::Absolute))
    );
    assert_eq!(
        detect(&[(Control::Cc(50), 127), (Control::Cc(50), 0)], &mute),
        Some((Control::Cc(50), MapMode::Toggle))
    );
}

/// #1, #19: on a ramp the tempo target reads the governing point's tempo — what its
/// command writes — so one encoder step moves that point by one step.
#[test]
fn the_tempo_target_reads_what_it_writes_on_a_ramp() {
    use crate::tempo::TempoPoint;
    let mut p = project();
    p.tempo_bpm = 100.0;
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 100.0,
            ramp: true,
        },
        TempoPoint {
            tick: Ticks::beats(32),
            bpm: 140.0,
            ramp: false,
        },
    ];
    let mid = Ticks::beats(16);
    let pos = MappingTarget::Tempo.current_pos(&p, mid).unwrap();
    let (lo, hi) = TEMPO_TRAVEL;
    assert!(
        (f64::from(pos) - (100.0 - lo) / (hi - lo)).abs() < 1e-6,
        "{pos}"
    );
    let m = Mapping {
        target: MappingTarget::Tempo,
        ..knob(MapMode::Relative(Encoding::TwosComplement))
    };
    let mut st = DriveState::default();
    let Drive::Set(to) = m.read(1, true, Some(pos), &mut st) else {
        panic!()
    };
    MappingTarget::Tempo
        .command(&p, to, mid)
        .unwrap()
        .apply(&mut p)
        .unwrap();
    let step = (hi - lo) / 128.0;
    assert!(
        (p.tempo_points[0].bpm - (100.0 + step)).abs() < 1e-3,
        "{}",
        p.tempo_points[0].bpm
    );
}

/// #41: an insert's bypass and mix as targets.
#[test]
fn an_inserts_bypass_and_mix_set_and_read() {
    let mut p = project();
    let s = p.strips[0].id;
    let fx = p.make_effect(crate::EffectKind::Compressor);
    let id = fx.id;
    p.strip_mut(s).unwrap().effects.push(fx);
    let bypass = MappingTarget::EffectBypass(id);
    bypass
        .command(&p, 1.0, Ticks::ZERO)
        .unwrap()
        .apply(&mut p)
        .unwrap();
    assert!(p.effect(s, id).unwrap().bypass);
    assert_eq!(bypass.current_pos(&p, Ticks::ZERO), Some(1.0));
    let mix = MappingTarget::EffectMix(id);
    mix.command(&p, 0.3, Ticks::ZERO)
        .unwrap()
        .apply(&mut p)
        .unwrap();
    assert!((p.effect(s, id).unwrap().mix - 0.3).abs() < 1e-6);
    assert!((mix.current_pos(&p, Ticks::ZERO).unwrap() - 0.3).abs() < 1e-6);
}

/// #21 and the new amount command: a gesture over an arm or a modulator amount merges.
#[test]
fn arming_and_a_modulator_amount_supersede_per_target() {
    let t = crate::TrackId(1);
    let a = Command::SetTrackArmed { id: t, armed: true };
    let b = Command::SetTrackArmed {
        id: t,
        armed: false,
    };
    assert!(b.supersedes(&a));
    let m = crate::ModulatorId(4);
    let x = Command::SetModulatorAmount { id: m, amount: 0.2 };
    let y = Command::SetModulatorAmount { id: m, amount: 0.7 };
    assert!(y.supersedes(&x));
    assert_eq!(
        crate::AutoTarget::value_of_command(&y),
        Some((crate::AutoTarget::ModAmount(m), 0.7f32 as f64))
    );
}

/// #8: a map read from a file keeps only what a command would accept.
#[test]
fn a_map_from_a_file_is_repaired() {
    let mut map = ControllerMap::default();
    map.mappings.push(mapping(1));
    map.mappings.push(Mapping {
        sensitivity: 0.0,
        ..mapping(2)
    });
    map.mappings.push(mapping(1));
    map.mappings.push(Mapping {
        source: src(Control::Cc(121)),
        ..mapping(3)
    });
    assert_eq!(map.repair(), 3);
    assert_eq!(map.mappings, vec![mapping(1)]);
}

/// #56: a mapping learnt on one hosted plug-in is dormant under another in its place.
#[test]
fn a_hosted_mapping_is_dormant_under_another_plug_in() {
    use crate::external::{ExternalPlugin, PluginFormat};
    let mut p = project();
    let t = p.tracks[0].id;
    let vital = ExternalPlugin::new(PluginFormat::Clap, "audio.vital", "Vital");
    p.track_mut(t).unwrap().instrument = crate::InstrumentSpec::External(vital.clone());
    let target = MappingTarget::Param(crate::AutoTarget::PluginInstrument { track: t, id: 17 });
    let mut m = Mapping::new(
        MappingId(1),
        src(Control::Cc(1)),
        target.clone(),
        MapMode::Absolute,
    );
    m.plugin = target.plugin_of(&p);
    assert_eq!(m.plugin, Some(PluginRef::of(&vital)));
    assert!(m.alive(&p));
    let other = ExternalPlugin::new(PluginFormat::Vst3, "ABCD", "Other");
    p.track_mut(t).unwrap().instrument = crate::InstrumentSpec::External(other);
    assert!(!m.alive(&p), "another plug-in: dormant");
    p.track_mut(t).unwrap().instrument = crate::InstrumentSpec::External(vital);
    assert!(m.alive(&p), "back: alive");
}

// ---- review 2 ------------------------------------------------------------------------

/// Review 2, #8: *scale* moves a stepped target — it keeps its own unrounded position while
/// nothing else moves the target, as an encoder does.
#[test]
fn scale_moves_a_stepped_target() {
    let m = Mapping {
        takeover: Takeover::Scale,
        ..knob(MapMode::Absolute)
    };
    let q = |p: f32| (p * 3.0).round() / 3.0; // four steps
    let mut st = DriveState::default();
    let mut now = 1.0 / 3.0;
    // The knob far from the target (0.9 against 0.33): scale, not jump.
    let _ = m.read(115, true, Some(now), &mut st);
    st.last_out = Some(now);
    let mut moved = false;
    for v in (40..115).rev() {
        if let Drive::Set(p) = m.read(v, true, Some(now), &mut st) {
            now = q(p);
            st.last_out = Some(now);
        }
        if now == 0.0 {
            moved = true;
            break;
        }
    }
    assert!(moved, "the stepped target came down a step: {now}");
}

/// Review 2, #2: a switch target driven by an absolute control takes no takeover — the
/// first press of a latching button sets it.
#[test]
fn a_switch_takes_no_takeover() {
    let m = Mapping {
        takeover: Takeover::Pickup,
        ..Mapping::new(
            MappingId(1),
            src(Control::Cc(20)),
            MappingTarget::TrackMute(crate::TrackId(1)),
            MapMode::Absolute,
        )
    };
    let mut st = DriveState::default();
    assert_eq!(m.read(127, true, Some(0.0), &mut st), Drive::Set(1.0));
    assert_eq!(m.read(0, true, Some(1.0), &mut st), Drive::Set(0.0));
}

/// Review 2, #6: a program-change button whose keyboard sends bank select first is learnt
/// as the program change.
#[test]
fn a_program_after_bank_select_is_learnt_as_the_program() {
    let target = MappingTarget::TrackMute(crate::TrackId(1));
    let heard = [
        (Control::Cc(0), 0),
        (Control::Cc(32), 0),
        (Control::Program(4), 127),
    ];
    assert_eq!(
        detect(&heard, &target),
        Some((Control::Program(4), MapMode::Toggle))
    );
    // A real 14-bit pair is still one.
    let pair = [
        (Control::Cc(0), 3),
        (Control::Cc(32), 5),
        (Control::Cc(0), 4),
    ];
    assert_eq!(detect(&pair, &target).map(|d| d.0), Some(Control::Cc14(0)));
}

/// Review 2, #1/#24/#51: a mapping learnt on another plug-in is not this one's — the map
/// finds only the ones that fit the plug-in loaded now.
#[test]
fn the_map_finds_only_the_mappings_of_the_plug_in_loaded() {
    use crate::external::{ExternalPlugin, PluginFormat};
    let mut p = project();
    let t = p.tracks[0].id;
    let vital = ExternalPlugin::new(PluginFormat::Clap, "audio.vital", "Vital");
    let surge = ExternalPlugin::new(PluginFormat::Clap, "org.surge", "Surge");
    p.track_mut(t).unwrap().instrument = crate::InstrumentSpec::External(vital.clone());
    let target = MappingTarget::Param(crate::AutoTarget::PluginInstrument { track: t, id: 17 });
    let mut m = Mapping::new(
        MappingId(1),
        src(Control::Cc(1)),
        target.clone(),
        MapMode::Absolute,
    );
    m.plugin = target.plugin_of(&p);
    let map = ControllerMap { mappings: vec![m] };
    assert!(map.mappings[0].fits(&p));
    assert_eq!(map.of_target_in(&target, &p).count(), 1);
    p.track_mut(t).unwrap().instrument = crate::InstrumentSpec::External(surge);
    assert!(!map.mappings[0].fits(&p));
    assert_eq!(map.of_target_in(&target, &p).count(), 0);
    assert_eq!(map.of_target(&target).count(), 1, "still there for an undo");
}

/// Review 2, #42: a hosted mapping's plug-in survives a save and a load.
#[test]
fn a_mappings_plug_in_survives_serde() {
    use crate::external::PluginFormat;
    let mut m = knob(MapMode::Absolute);
    m.plugin = Some(PluginRef {
        format: PluginFormat::Clap,
        id: "org.test.synth".into(),
    });
    let text = serde_json::to_string(&m).unwrap();
    assert!(text.contains("org.test.synth"), "{text}");
    let back: Mapping = serde_json::from_str(&text).unwrap();
    assert_eq!(back, m);
}

// ---- review 3 ------------------------------------------------------------------------

/// A target's shape as the interface tells it: two states, or `n` steps.
fn two_state() -> Steps {
    Steps {
        switch: true,
        steps: Some(1),
    }
}

/// Review 3, #0: a two-state parameter (an LED, a band's On) is a switch: a latching
/// button's first press sets it under pick-up; one encoder detent sets it either way.
#[test]
fn a_two_state_parameter_is_a_switch() {
    let m = Mapping {
        takeover: Takeover::Pickup,
        ..knob(MapMode::Absolute)
    };
    let mut st = DriveState::default();
    assert_eq!(
        m.read_in(127, true, Some(0.0), &mut st, two_state()),
        Drive::Set(1.0),
        "the first press"
    );
    assert_eq!(
        m.read_in(0, true, Some(1.0), &mut st, two_state()),
        Drive::Set(0.0)
    );
    let enc = knob(MapMode::Relative(Encoding::TwosComplement));
    let mut st = DriveState::default();
    assert_eq!(
        enc.read_in(1, true, Some(0.0), &mut st, two_state()),
        Drive::Set(1.0),
        "one detent"
    );
    assert_eq!(
        enc.read_in(127, true, Some(1.0), &mut st, two_state()),
        Drive::Set(0.0)
    );
}

/// Review 3, #0: a pad learnt on a two-state parameter toggles it.
#[test]
fn a_pad_on_a_two_state_parameter_is_learnt_as_a_toggle() {
    let target = MappingTarget::Param(AutoTarget::MasterPan);
    let heard = [(Control::Note(36), 100)];
    assert_eq!(
        learn::detect_in(&heard, &target, two_state()),
        Some((Control::Note(36), MapMode::Toggle))
    );
    assert_eq!(
        learn::detect_in(&heard, &target, Steps::default()),
        Some((Control::Note(36), MapMode::Absolute))
    );
}

/// Review 3, #1: an encoder on a stepped choice moves one choice per detent.
#[test]
fn an_encoder_moves_one_choice_per_detent() {
    let m = knob(MapMode::Relative(Encoding::TwosComplement));
    let shape = Steps {
        switch: false,
        steps: Some(8),
    };
    let q = |p: f32| (p * 8.0).round() / 8.0;
    let mut st = DriveState::default();
    let mut now = 0.0;
    for want in 1..=3 {
        if let Drive::Set(p) = m.read_in(1, true, Some(now), &mut st, shape) {
            now = q(p);
            st.last_out = Some(now);
        }
        assert!((now - want as f32 / 8.0).abs() < 1e-6, "{want}: {now}");
    }
}

/// Review 3, #45: a port's name as Linux's ALSA gives it ("client:port C:P") is the
/// device's name; a mapping saved under one client number matches the port under another.
#[test]
fn an_alsa_port_name_is_the_devices_name() {
    assert_eq!(
        clean_port_name("nanoKONTROL2:nanoKONTROL2 _ CTRL 24:0"),
        "nanoKONTROL2 _ CTRL"
    );
    assert_eq!(clean_port_name("Keystation 49"), "Keystation 49");
    assert_eq!(clean_port_name("MIDIIN2 (X-Touch)"), "MIDIIN2 (X-Touch)");
    assert_eq!(
        clean_port_name("Midi Through:Midi Through Port-0 14:0"),
        "Midi Through Port-0"
    );
    let then = vec!["nanoKONTROL2:nanoKONTROL2 _ CTRL 24:0".to_string()];
    let now = vec![
        "Midi Through:Midi Through Port-0 14:0".to_string(),
        "nanoKONTROL2:nanoKONTROL2 _ CTRL 28:0".to_string(),
    ];
    let p = PortRef::of(&then, 0);
    assert_eq!(p.name, "nanoKONTROL2 _ CTRL");
    assert_eq!(p.index_in(&now), Some(1));
    // A mapping saved with the raw name still matches.
    let raw = PortRef {
        name: then[0].clone(),
        ordinal: 0,
    };
    assert_eq!(raw.index_in(&now), Some(1));
}

/// Review 3, #44: a mapping of a swapped-out plug-in shares nothing with the live one.
#[test]
fn a_dormant_plug_in_mapping_is_not_shared() {
    use crate::external::{ExternalPlugin, PluginFormat};
    let mut p = project();
    let t = p.tracks[0].id;
    let vital = ExternalPlugin::new(PluginFormat::Clap, "audio.vital", "Vital");
    p.track_mut(t).unwrap().instrument = crate::InstrumentSpec::External(vital);
    let target = MappingTarget::Param(crate::AutoTarget::PluginInstrument { track: t, id: 7 });
    let mut old = Mapping::new(
        MappingId(1),
        src(Control::Cc(71)),
        target.clone(),
        MapMode::Absolute,
    );
    old.plugin = Some(PluginRef {
        format: PluginFormat::Clap,
        id: "org.surge".into(),
    });
    let mut new = old.clone();
    new.id = MappingId(2);
    new.plugin = target.plugin_of(&p);
    let map = ControllerMap {
        mappings: vec![old, new.clone()],
    };
    assert_eq!(map.sharing(&new).count(), 1);
    assert_eq!(map.sharing_in(&new, &p).count(), 0);
}

/// Review 3, #7: "the selected strip" with the master selected is the master's volume and
/// pan (it has no mute or solo).
#[test]
fn the_selected_strip_can_be_the_master() {
    let focus = Focus {
        master: true,
        ..Focus::default()
    };
    assert_eq!(
        MappingTarget::SelectedStrip(DeskParam::Volume).resolve(&focus),
        Some(MappingTarget::Param(AutoTarget::MasterGain))
    );
    assert_eq!(
        MappingTarget::SelectedStrip(DeskParam::Pan).resolve(&focus),
        Some(MappingTarget::Param(AutoTarget::MasterPan))
    );
    assert_eq!(
        MappingTarget::SelectedStrip(DeskParam::Mute).resolve(&focus),
        None
    );
}

/// Review 3, #46: a strip's effects switch and its record arm are switch targets that set
/// and read back (the master's effects too).
#[test]
fn a_strips_effects_and_record_arm_set_and_read() {
    let mut p = project();
    let t = p.tracks[0].id;
    let s = p.tracks[0].strip.unwrap();
    p.track_mut(t).unwrap().input = Some(crate::audio::TrackInput {
        first: 0,
        stereo: false,
        monitor: Default::default(),
        trim_db: 0.0,
    });
    for target in [
        MappingTarget::StripFx(s),
        MappingTarget::StripFx(StripId::MASTER),
        MappingTarget::StripArm(s),
    ] {
        assert!(target.is_switch() && target.exists(&p), "{target:?}");
        for pos in [0.0f32, 1.0] {
            let cmd = target.command(&p, pos, Ticks::ZERO).expect("a command");
            cmd.apply(&mut p).unwrap();
            assert_eq!(target.current_pos(&p, Ticks::ZERO), Some(pos), "{target:?}");
        }
    }
}
