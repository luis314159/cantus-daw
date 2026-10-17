//! The tempo track's, the metre's and the markers' commands: each validates, and its
//! inverse gives back the project exactly (ADR 0004, ADR 0060).

use super::*;
use crate::command::Command;

fn pt(tick: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp,
    }
}

/// Apply `cmd`, then its inverse: the project comes back exactly; returns the project as
/// `cmd` left it.
fn round_trip(p: &mut Project, cmd: Command) -> Project {
    let before = p.clone();
    let inv = cmd.apply(p).expect("applies");
    let after = p.clone();
    let redo = inv.apply(p).expect("the inverse applies");
    assert_eq!(*p, before, "the inverse restores");
    redo.apply(p).expect("redo applies");
    assert_eq!(*p, after, "redo restores");
    after
}

#[test]
fn the_tempo_track_is_made_edited_and_removed_exactly() {
    let mut p = Project::empty("t");
    // No track: inserting is refused (the track is made whole).
    assert!(Command::InsertTempoPoint {
        index: 1,
        point: pt(960, 90.0, false)
    }
    .apply(&mut p.clone())
    .is_err());
    // A track must start at 0 at the song's tempo, in order, in range.
    for bad in [
        vec![pt(10, 120.0, false)],
        vec![pt(0, 100.0, false)],
        vec![pt(0, 120.0, false), pt(0, 90.0, false)],
        vec![pt(0, 120.0, false), pt(960, 5000.0, false)],
    ] {
        let cmd = Command::SetTempoTrack {
            bpm: 120.0,
            points: bad,
        };
        assert!(cmd.apply(&mut p.clone()).is_err());
    }
    let after = round_trip(
        &mut p,
        Command::SetTempoTrack {
            bpm: 90.0,
            points: vec![pt(0, 90.0, true), pt(32 * 960, 140.0, false)],
        },
    );
    assert_eq!(after.tempo_bpm, 90.0);
    p = after;
    let after = round_trip(
        &mut p,
        Command::InsertTempoPoint {
            index: 2,
            point: pt(40 * 960, 100.0, false),
        },
    );
    assert_eq!(after.tempo_points.len(), 3);
    p = after;
    // Between its neighbours only.
    for (i, point) in [
        (1, pt(40 * 960, 100.0, false)),
        (1, pt(0, 99.0, false)),
        (1, pt(41 * 960, 100.0, false)),
    ] {
        assert!(Command::SetTempoPoint { index: i, point }
            .apply(&mut p.clone())
            .is_err());
    }
    round_trip(
        &mut p,
        Command::SetTempoPoint {
            index: 1,
            point: pt(33 * 960, 150.0, true),
        },
    );
    // The first point is the song's tempo, both ways.
    let after = round_trip(
        &mut p,
        Command::SetTempoPoint {
            index: 0,
            point: pt(0, 100.0, true),
        },
    );
    assert_eq!(after.tempo_bpm, 100.0);
    let after = round_trip(&mut p, Command::SetTempo { bpm: 111.0 });
    assert_eq!(after.tempo_points[0].bpm, 111.0);
    assert!(Command::RemoveTempoPoint { index: 0 }
        .apply(&mut p.clone())
        .is_err());
    let after = round_trip(&mut p, Command::RemoveTempoPoint { index: 1 });
    assert_eq!(after.tempo_points.len(), 2);
    let after = round_trip(
        &mut p,
        Command::SetTempoTrack {
            bpm: 90.0,
            points: Vec::new(),
        },
    );
    assert!(!after.has_tempo_track() && after.tempo_map().is_constant());
}

fn sig(n: u8, d: u8) -> TimeSignature {
    TimeSignature::new(n, d)
}

#[test]
fn metre_changes_keep_the_later_bars() {
    let mut p = Project::empty("t");
    // 7/8 at bar 3, then 3/4 at bar 4.
    let after = round_trip(
        &mut p,
        Command::SetMetreAt {
            tick: Ticks(7680),
            signature: sig(7, 8),
        },
    );
    assert_eq!(after.metre_changes.len(), 2);
    p = after;
    let after = round_trip(
        &mut p,
        Command::SetMetreAt {
            tick: Ticks(11_040),
            signature: sig(3, 4),
        },
    );
    p = after;
    assert_eq!(p.tempo_map().bar_tick(4), Ticks(11_040));
    // Off a bar line: refused.
    assert!(Command::SetMetreAt {
        tick: Ticks(9000),
        signature: sig(5, 4)
    }
    .apply(&mut p.clone())
    .is_err());
    // The 7/8 bar becomes 5/8: the 3/4 still starts bar 4 (now 960 ticks earlier).
    let after = round_trip(
        &mut p.clone(),
        Command::SetMetreAt {
            tick: Ticks(7680),
            signature: sig(5, 8),
        },
    );
    let map = after.tempo_map();
    assert_eq!(map.bar_tick(4), Ticks(7680 + 2400));
    assert_eq!(map.signature_at(map.bar_tick(4)), sig(3, 4));
    // Removing the 7/8 keeps the 3/4 at bar 4 (4/4 bars before it).
    let after = round_trip(&mut p.clone(), Command::RemoveMetreAt { tick: Ticks(7680) });
    let map = after.tempo_map();
    assert_eq!(map.metres().len(), 2);
    assert_eq!(map.bar_tick(4), Ticks(3 * 3840));
    assert_eq!(map.signature_at(Ticks(3 * 3840)), sig(3, 4));
    // Moving the 7/8 back a bar: along 4/4's lines, the 3/4 keeps bar 4.
    let after = round_trip(
        &mut p.clone(),
        Command::MoveMetre {
            from: Ticks(7680),
            to: Ticks(3840),
        },
    );
    let map = after.tempo_map();
    assert_eq!(map.signature_at(Ticks(3840)), sig(7, 8));
    assert_eq!(map.bbt(map.bar_tick(4)), (4, 1, 0));
    assert_eq!(map.signature_at(map.bar_tick(4)), sig(3, 4));
    assert_eq!(map.bar_tick(4), Ticks(3840 + 2 * 3360));
    assert!(
        Command::MoveMetre {
            from: Ticks(7680),
            to: Ticks(11_040)
        }
        .apply(&mut p.clone())
        .is_err(),
        "past its neighbour"
    );
    assert!(Command::RemoveMetreAt { tick: Ticks::ZERO }
        .apply(&mut p.clone())
        .is_err());
    // The song's metre is the first change, both ways.
    let after = round_trip(
        &mut p.clone(),
        Command::SetTimeSignature {
            time_signature: sig(6, 8),
        },
    );
    assert_eq!(after.metre_changes[0].signature, sig(6, 8));
    let after = round_trip(
        &mut p.clone(),
        Command::SetMetreAt {
            tick: Ticks::ZERO,
            signature: sig(2, 4),
        },
    );
    assert_eq!(after.time_signature, sig(2, 4));
    let after = round_trip(
        &mut p,
        Command::SetMetreTrack {
            signature: sig(4, 4),
            metres: Vec::new(),
        },
    );
    assert!(after.tempo_map().is_one_metre());
}

fn marker(id: u64, tick: i64, name: &str) -> Marker {
    Marker {
        id: MarkerId(id),
        tick: Ticks(tick),
        name: name.into(),
        color: None,
    }
}

#[test]
fn markers_are_added_moved_renamed_and_removed_exactly() {
    let mut p = Project::empty("t");
    p = round_trip(
        &mut p,
        Command::AddMarker {
            marker: marker(5, 3840, "Verse"),
        },
    );
    p = round_trip(
        &mut p,
        Command::AddMarker {
            marker: marker(6, 0, "Intro"),
        },
    );
    p = round_trip(
        &mut p,
        Command::AddMarker {
            marker: marker(7, 3840, "Also"),
        },
    );
    let names: Vec<&str> = p.markers.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["Intro", "Verse", "Also"], "by position, then id");
    assert!(Command::AddMarker {
        marker: marker(5, 10, "x")
    }
    .apply(&mut p.clone())
    .is_err());
    assert!(Command::AddMarker {
        marker: marker(9, -10, "x")
    }
    .apply(&mut p.clone())
    .is_err());
    p = round_trip(
        &mut p,
        Command::SetMarker {
            marker: marker(6, 9600, "Outro"),
        },
    );
    assert_eq!(p.markers.last().unwrap().name, "Outro");
    let (before, after) = p.markers_around(Ticks(5000));
    assert_eq!(
        (before.unwrap().id, after.unwrap().id),
        (MarkerId(7), MarkerId(6))
    );
    p = round_trip(&mut p, Command::RemoveMarker { id: MarkerId(5) });
    assert_eq!(p.markers.len(), 2);
    assert!(Command::RemoveMarker { id: MarkerId(5) }
        .apply(&mut p.clone())
        .is_err());
}

/// Review 1 (findings 2, 3, 13, 14): a metre edit keeps every change before it at its
/// tick, lands the edited change exactly where it was asked, and moves the later changes
/// only so they keep their bar numbers — an off-line change (a short bar before it) stays
/// off-line by the same remainder; the song's metre changed by `SetTimeSignature` does the
/// same as `SetMetreAt` at tick 0.
#[test]
fn metre_edits_keep_the_ticks_they_do_not_move() {
    // 4/4, 3/4 from bar 3 (7680); then the song's metre 7/8 makes bar 3 a short bar of
    // 960 ticks... with SetTimeSignature routed through the bars, the 3/4 keeps bar 3.
    let mut p = Project::empty("t");
    Command::SetMetreAt { tick: Ticks(7680), signature: sig(3, 4) }
        .apply(&mut p)
        .unwrap();
    let mut q = p.clone();
    Command::SetTimeSignature { time_signature: sig(7, 8) }
        .apply(&mut q)
        .unwrap();
    assert_eq!(q.metre_changes[1].tick, Ticks(2 * 3360), "keeps bar 3");
    let mut r = p.clone();
    Command::SetMetreAt { tick: Ticks::ZERO, signature: sig(7, 8) }
        .apply(&mut r)
        .unwrap();
    assert_eq!(q.metre_changes, r.metre_changes);
    // An off-line state (from a file): 7/8 at 0, 3/4 at 7680 (a short bar of 960 before it).
    let mut p = Project::empty("t");
    Command::SetMetreTrack {
        signature: sig(7, 8),
        metres: vec![
            MetreChange { tick: Ticks::ZERO, signature: sig(7, 8) },
            MetreChange { tick: Ticks(7680), signature: sig(3, 4) },
        ],
    }
    .apply(&mut p)
    .unwrap();
    let at = p.tempo_map().bar_tick(6);
    Command::SetMetreAt { tick: at, signature: sig(4, 4) }
        .apply(&mut p)
        .unwrap();
    assert_eq!(p.metre_changes[1].tick, Ticks(7680), "the earlier change stays");
    assert_eq!(p.metre_changes[2].tick, at, "the new one lands where asked");
    // A move lands exactly on the tick asked, counted in the metre before it (review 1's
    // example: 3/4, a bar of 1/8 at bar 2, 4/4 at bar 18).
    let mut p = Project::empty("t");
    Command::SetMetreTrack {
        signature: sig(3, 4),
        metres: vec![
            MetreChange { tick: Ticks::ZERO, signature: sig(3, 4) },
            MetreChange { tick: Ticks(2880), signature: sig(1, 8) },
            MetreChange { tick: Ticks(2880 + 16 * 480), signature: sig(4, 4) },
        ],
    }
    .apply(&mut p)
    .unwrap();
    let mut q = p.clone();
    Command::MoveMetre { from: Ticks(2880), to: Ticks(5760) }.apply(&mut q).unwrap();
    assert_eq!(q.metre_changes[1].tick, Ticks(5760));
    let map = q.tempo_map();
    assert_eq!(map.bbt(q.metre_changes[2].tick).0, 18, "the 4/4 keeps bar 18");
    // Off the bar lines of the metre before it, or past the next change: refused.
    for to in [5000, 18_240] {
        assert!(Command::MoveMetre { from: Ticks(2880), to: Ticks(to) }
            .apply(&mut p.clone())
            .is_err(), "{to}");
    }
}

/// Review 1 (finding 25): a drag of a tempo point, of a metre change or of a marker is one
/// undo step however many moves it made, and that step gives everything back.
#[test]
fn a_drag_is_one_undo_step() {
    use crate::bus::CommandBus;
    use crate::command::Origin;
    use crate::MergeKey;
    let mut bus = CommandBus::default();
    bus.execute(
        Command::SetTempoTrack {
            bpm: 120.0,
            points: vec![pt(0, 120.0, false), pt(3840, 100.0, false)],
        },
        Origin::User,
    )
    .unwrap();
    bus.execute(Command::AddMarker { marker: marker(1_000_000, 0, "A") }, Origin::User)
        .unwrap();
    let before = bus.project().clone();
    let n = bus.history().len();
    for k in 1..20 {
        bus.execute_merging(
            Command::SetTempoPoint { index: 1, point: pt(3840 + 10 * k, 100.0 + k as f64, false) },
            Origin::User,
            MergeKey::of(&("tempo-point", 1usize)),
        )
        .unwrap();
    }
    bus.end_gesture();
    for k in 1..20 {
        bus.execute_merging(
            Command::SetMarker { marker: marker(1_000_000, 100 * k, "A") },
            Origin::User,
            MergeKey::of(&("marker-move", 1_000_000u64)),
        )
        .unwrap();
    }
    bus.end_gesture();
    assert_eq!(bus.history().len(), n + 2, "two gestures, two steps");
    bus.undo();
    bus.undo();
    assert_eq!(*bus.project(), before);
}
