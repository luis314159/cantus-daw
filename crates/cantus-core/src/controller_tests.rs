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
    assert_eq!((TwosComplement.steps(1), TwosComplement.steps(127)), (1, -1));
    assert_eq!((TwosComplement.steps(5), TwosComplement.steps(123)), (5, -5));
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
    assert_eq!(m.drive(Input::Abs(1.0), Some(0.0), &mut st), Drive::Set(1.0));
}

#[test]
fn pick_up_never_jumps_and_takes_over_when_the_knob_passes_the_value() {
    let m = knob(MapMode::Absolute);
    assert_eq!(m.takeover, Takeover::Pickup);
    let mut st = DriveState::default();
    // The target is at 0.5; the knob at the bottom moves up — nothing until it reaches 0.5.
    for x in [0.0, 0.1, 0.3, 0.45] {
        assert_eq!(m.drive(Input::Abs(x), Some(0.5), &mut st), Drive::Nothing, "{x}");
    }
    // It passes the value between two messages (a fast turn): picked up.
    assert_eq!(m.drive(Input::Abs(0.6), Some(0.5), &mut st), Drive::Set(0.6));
    // From then on it follows.
    assert_eq!(m.drive(Input::Abs(0.7), Some(0.6), &mut st), Drive::Set(0.7));
    // Something else moved the target (the mouse): picked up again only when passed.
    assert_eq!(m.drive(Input::Abs(0.72), Some(0.2), &mut st), Drive::Nothing);
    assert_eq!(m.drive(Input::Abs(0.5), Some(0.2), &mut st), Drive::Nothing);
    assert_eq!(m.drive(Input::Abs(0.19), Some(0.2), &mut st), Drive::Set(0.19));
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
    assert_eq!(m.drive(Input::Abs(0.5), Some(cur), &mut st), Drive::Set(0.5));
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
    assert_eq!(m.drive(Input::Steps(-500), Some(0.5), &mut st), Drive::Set(0.25));
    assert_eq!(m.drive(Input::Steps(500), Some(0.5), &mut st), Drive::Set(0.75));
    // Sensitivity scales a step.
    let fast = Mapping {
        sensitivity: 4.0,
        ..knob(MapMode::Relative(Encoding::TwosComplement))
    };
    assert_eq!(fast.drive(Input::Steps(2), Some(0.0), &mut st), Drive::Set(8.0 / 128.0));
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
    assert_eq!(m.drive(Input::Press(true), Some(0.0), &mut st), Drive::Set(1.0));
    assert_eq!(m.drive(Input::Press(false), Some(1.0), &mut st), Drive::Nothing);
    assert_eq!(m.drive(Input::Press(true), Some(1.0), &mut st), Drive::Set(0.0));
    // A held press repeated (a controller resending 127) counts once.
    assert_eq!(m.drive(Input::Press(true), Some(0.0), &mut st), Drive::Nothing);
    // A controller as a button: pressed from its middle up.
    m.source.control = Control::Cc(64);
    assert_eq!(m.input(127, true), Input::Press(true));
    assert_eq!(m.input(0, true), Input::Press(false));
}

#[test]
fn momentary_holds_the_top_while_pressed() {
    let m = knob(MapMode::Momentary);
    let mut st = DriveState::default();
    assert_eq!(m.drive(Input::Press(true), Some(0.3), &mut st), Drive::Set(1.0));
    assert_eq!(m.drive(Input::Press(true), Some(1.0), &mut st), Drive::Nothing);
    assert_eq!(m.drive(Input::Press(false), Some(1.0), &mut st), Drive::Set(0.0));
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
    assert_eq!(m.drive(m.input(0, false), Some(1.0), &mut st), Drive::Nothing);
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
    assert_eq!(detect(&knob, &param), Some((Control::Cc(21), MapMode::Absolute)));
    // A 14-bit fader: the coarse controller and its fine twin.
    let fine = vec![(Control::Cc(7), 64), (Control::Cc(39), 12), (Control::Cc(7), 65)];
    assert_eq!(detect(&fine, &param), Some((Control::Cc14(7), MapMode::Absolute)));
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
    assert_eq!(detect(&button, &mute), Some((Control::Cc(64), MapMode::Toggle)));
    assert_eq!(detect(&button, &play), Some((Control::Cc(64), MapMode::Trigger)));
    // A pad: toggles a switch, sets a parameter by velocity, fires an action.
    let pad = vec![(Control::Note(36), 90)];
    assert_eq!(detect(&pad, &mute), Some((Control::Note(36), MapMode::Toggle)));
    assert_eq!(detect(&pad, &param), Some((Control::Note(36), MapMode::Absolute)));
    assert_eq!(detect(&pad, &play), Some((Control::Note(36), MapMode::Trigger)));
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
    assert_eq!(cmd, crate::tempo::edit::set_tempo_at(&p, Ticks::ZERO, 140.0));
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
        p.controllers.mappings.iter().map(|m| m.id.0).collect::<Vec<_>>(),
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
    let restore = Command::RemoveMapping { id: MappingId(1) }.apply(&mut p).unwrap();
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
