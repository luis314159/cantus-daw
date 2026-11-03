//! External units (round 36, ADR 0094): the commands, their exact inverses, the edit lock,
//! the snapshot, and an undo fuzz in a stream of its own (the shared fuzz's draws stay what
//! they were).

use cantus_core::external_unit::{MidiRoute, Pair};
use cantus_core::*;

fn bus_with_tracks(n: usize) -> (CommandBus, Vec<TrackId>) {
    let mut p = Project::new("ext");
    let mut ids = vec![];
    for i in 0..n {
        let t = p.make_track(format!("T{i}"));
        ids.push(t.id);
        p.tracks.push(t);
    }
    (CommandBus::new(p), ids)
}

fn synth(p: &mut Project, track: TrackId) -> ExternalUnit {
    let id = p.new_external_unit_id();
    let mut u = ExternalUnit::instrument(
        id,
        track,
        MidiRoute {
            port: "Hardware Synth".into(),
            ordinal: 0,
            channel: 2,
        },
        Pair::stereo(4),
    );
    u.name = "Hardware Synth".into();
    u
}

fn measured(frames: u32) -> external_unit::Measured {
    external_unit::Measured::bare(frames, 48_000, 40.0)
}

#[test]
fn a_unit_is_added_changed_measured_and_removed_with_exact_inverses() {
    let (mut bus, t) = bus_with_tracks(2);
    let before = bus.project().clone();
    let mut p = bus.project().clone();
    let u = synth(&mut p, t[0]);
    bus.reserve_ids(p.next_id);
    let id = u.id;
    bus.execute(
        Command::InsertExternalUnit {
            index: 0,
            unit: Box::new(u.clone()),
        },
        Origin::User,
    )
    .unwrap();
    let added = bus.project().clone();
    assert_eq!(added.external_units, vec![u.clone()]);
    bus.execute(
        Command::SetExternalLatency {
            id,
            latency: Some(measured(612)),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(
        bus.project()
            .external_unit(id)
            .unwrap()
            .latency_frames(48_000.0),
        612
    );
    let mut moved = bus.project().external_unit(id).unwrap().clone();
    moved.trim_ms = 1.5;
    moved.name = "Bass synth".into();
    bus.execute(
        Command::SetExternalUnit {
            unit: Box::new(moved),
        },
        Origin::User,
    )
    .unwrap();
    assert_eq!(
        bus.project()
            .external_unit(id)
            .unwrap()
            .latency_frames(48_000.0),
        684
    );
    bus.execute(Command::RemoveExternalUnit { id }, Origin::User)
        .unwrap();
    assert!(bus.project().external_units.is_empty());
    for _ in 0..4 {
        bus.undo().unwrap();
    }
    let mut back = bus.project().clone();
    back.next_id = before.next_id;
    assert_eq!(back, before, "undo all");
    for _ in 0..4 {
        bus.redo().unwrap();
    }
    assert!(bus.project().external_units.is_empty());
}

#[test]
fn the_wrong_unit_is_refused_and_changes_nothing() {
    let (mut bus, t) = bus_with_tracks(1);
    let mut p = bus.project().clone();
    let u = synth(&mut p, t[0]);
    bus.reserve_ids(p.next_id);
    bus.execute(
        Command::InsertExternalUnit {
            index: 0,
            unit: Box::new(u.clone()),
        },
        Origin::User,
    )
    .unwrap();
    let kept = bus.project().clone();
    // Same id again; a second instrument on the track; an invalid unit;
    // an index past the end; a latency that is not a latency; a removal of nothing.
    let mut second = u.clone();
    second.id = ExternalUnitId(9_999);
    let mut invalid = synth(&mut p, t[0]);
    invalid.trim_ms = 9e9;
    let attempts = vec![
        Command::InsertExternalUnit {
            index: 0,
            unit: Box::new(u.clone()),
        },
        Command::InsertExternalUnit {
            index: 1,
            unit: Box::new(second),
        },
        Command::InsertExternalUnit {
            index: 1,
            unit: Box::new(invalid),
        },
        Command::InsertExternalUnit {
            index: 5,
            unit: Box::new(synth(&mut p, t[0])),
        },
        Command::SetExternalLatency {
            id: u.id,
            latency: Some(external_unit::Measured::bare(1, 3, 1.0)),
        },
        Command::RemoveExternalUnit {
            id: ExternalUnitId(1),
        },
    ];
    for c in attempts {
        assert!(bus.execute(c.clone(), Origin::User).is_err(), "{c:?}");
    }
    assert_eq!(*bus.project(), kept);
}

#[test]
fn a_track_may_have_an_instrument_and_an_effect_but_not_two_of_a_kind() {
    let (mut bus, t) = bus_with_tracks(1);
    let mut p = bus.project().clone();
    let a = synth(&mut p, t[0]);
    let fx_id = p.new_external_unit_id();
    let fx = ExternalUnit::effect(fx_id, t[0], Pair::stereo(2), Pair::stereo(2));
    let fx2 = ExternalUnit::effect(
        p.new_external_unit_id(),
        t[0],
        Pair::stereo(4),
        Pair::stereo(4),
    );
    bus.reserve_ids(p.next_id);
    for (i, u) in [a, fx].into_iter().enumerate() {
        bus.execute(
            Command::InsertExternalUnit {
                index: i,
                unit: Box::new(u),
            },
            Origin::User,
        )
        .unwrap();
    }
    assert!(bus
        .execute(
            Command::InsertExternalUnit {
                index: 2,
                unit: Box::new(fx2)
            },
            Origin::User
        )
        .is_err());
}

#[test]
fn every_command_has_a_history_name() {
    let u = ExternalUnit::effect(ExternalUnitId(1), TrackId(1), Pair::mono(0), Pair::mono(0));
    for (c, key) in [
        (
            Command::InsertExternalUnit {
                index: 0,
                unit: Box::new(u.clone()),
            },
            "cmd.external.add",
        ),
        (
            Command::RemoveExternalUnit { id: u.id },
            "cmd.external.remove",
        ),
        (
            Command::SetExternalUnit {
                unit: Box::new(u.clone()),
            },
            "cmd.external.set",
        ),
        (
            Command::SetExternalLatency {
                id: u.id,
                latency: None,
            },
            "cmd.external.latency",
        ),
    ] {
        assert_eq!(c.label_key(), key);
    }
}

#[test]
fn a_frozen_track_refuses_a_unit_that_would_change_its_source() {
    let (mut bus, t) = bus_with_tracks(1);
    let mut p = bus.project().clone();
    let u = synth(&mut p, t[0]);
    bus.reserve_ids(p.next_id);
    let touches = cantus_core::freeze::touches(
        &Command::InsertExternalUnit {
            index: 0,
            unit: Box::new(u.clone()),
        },
        bus.project(),
    );
    assert_eq!(touches, vec![freeze::Touch::TrackSource(t[0])]);
    let none = cantus_core::freeze::touches(
        &Command::SetExternalLatency {
            id: u.id,
            latency: None,
        },
        bus.project(),
    );
    assert!(none.is_empty(), "a measurement changes no sound");
}

#[test]
fn units_survive_the_project_file_and_older_projects_have_none() {
    let (mut bus, t) = bus_with_tracks(1);
    let mut p = bus.project().clone();
    let mut u = synth(&mut p, t[0]);
    u.latency = Some(measured(777));
    u.device = "Studio Interface".into();
    bus.reserve_ids(p.next_id);
    bus.execute(
        Command::InsertExternalUnit {
            index: 0,
            unit: Box::new(u),
        },
        Origin::User,
    )
    .unwrap();
    let json = serde_json::to_string(bus.project()).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(&back, bus.project());
    // A project with no unit writes no key at all, so it is the file it was before.
    let plain = Project::new("plain");
    assert!(!serde_json::to_string(&plain)
        .unwrap()
        .contains("external_units"));
    let mut v: serde_json::Value = serde_json::to_value(&plain).unwrap();
    v.as_object_mut().unwrap().remove("external_units");
    let old: Project = serde_json::from_value(v).unwrap();
    assert!(old.external_units.is_empty());
}

#[test]
fn the_snapshot_carries_each_enabled_units_link_and_a_disabled_one_none() {
    let (mut bus, t) = bus_with_tracks(2);
    let mut p = bus.project().clone();
    let mut u = synth(&mut p, t[0]);
    u.latency = Some(measured(480));
    u.return_db = -6.0;
    bus.reserve_ids(p.next_id);
    bus.execute(
        Command::InsertExternalUnit {
            index: 0,
            unit: Box::new(u.clone()),
        },
        Origin::User,
    )
    .unwrap();
    let snap = Snapshot::compile(bus.project(), 1);
    let on = snap.track(t[0]).unwrap();
    assert_eq!(on.external.len(), 1);
    assert_eq!(on.external[0].latency_at(48_000.0), 480);
    assert!((on.external[0].return_gain - 0.501).abs() < 0.001);
    assert!(snap.track(t[1]).unwrap().external.is_empty());
    let mut off = u;
    off.enabled = false;
    bus.execute(
        Command::SetExternalUnit {
            unit: Box::new(off),
        },
        Origin::User,
    )
    .unwrap();
    assert!(Snapshot::compile(bus.project(), 1)
        .track(t[0])
        .unwrap()
        .external
        .is_empty());
}

// ---- the undo fuzz, in a stream of its own ------------------------------------------------

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Kinds: 0 add, 1 remove, 2 set, 3 latency, 4 refused (an invalid unit).
const KINDS: usize = 5;

fn external_command(rng: &mut Lcg, project: &mut Project) -> (usize, Command) {
    let tracks: Vec<TrackId> = project.tracks.iter().map(|t| t.id).collect();
    let units = project.external_units.clone();
    let route = |rng: &mut Lcg| MidiRoute {
        port: format!("Port{}", rng.below(3)),
        ordinal: rng.below(2) as u8,
        channel: rng.below(16) as u8,
    };
    match rng.below(5) {
        0 if !tracks.is_empty() => {
            let track = tracks[rng.below(tracks.len() as u64) as usize];
            let id = project.new_external_unit_id();
            let u = if rng.below(2) == 0 {
                ExternalUnit::instrument(id, track, route(rng), Pair::stereo(rng.below(8) as u16))
            } else {
                ExternalUnit::effect(
                    id,
                    track,
                    Pair::stereo(rng.below(8) as u16),
                    Pair::stereo(rng.below(8) as u16),
                )
            };
            (
                0,
                Command::InsertExternalUnit {
                    index: rng.below(units.len() as u64 + 1) as usize,
                    unit: Box::new(u),
                },
            )
        }
        1 if !units.is_empty() => (
            1,
            Command::RemoveExternalUnit {
                id: units[rng.below(units.len() as u64) as usize].id,
            },
        ),
        2 if !units.is_empty() => {
            let mut u = units[rng.below(units.len() as u64) as usize].clone();
            u.trim_ms = rng.below(1000) as f32 - 500.0;
            u.return_db = rng.below(60) as f32 - 30.0;
            u.enabled = rng.below(4) != 0;
            u.name = format!("n{}", rng.below(100));
            (2, Command::SetExternalUnit { unit: Box::new(u) })
        }
        3 if !units.is_empty() => (
            3,
            Command::SetExternalLatency {
                id: units[rng.below(units.len() as u64) as usize].id,
                latency: (rng.below(5) != 0).then(|| measured(rng.below(20_000) as u32)),
            },
        ),
        _ => {
            let id = project.new_external_unit_id();
            (
                4,
                Command::InsertExternalUnit {
                    index: 0,
                    unit: Box::new(ExternalUnit {
                        trim_ms: f32::NAN,
                        ..ExternalUnit::effect(id, TrackId(u64::MAX), Pair::mono(0), Pair::mono(0))
                    }),
                },
            )
        }
    }
}

#[test]
fn external_commands_undo_and_redo_among_track_edits() {
    let origins = [
        Origin::User,
        Origin::Script,
        Origin::Ai,
        Origin::Mcp,
        Origin::System,
    ];
    let mut seen = [[0usize; 2]; KINDS];
    for seed in 1..=60u64 {
        let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9).wrapping_add(36));
        let (mut bus, _) = bus_with_tracks(3);
        let start = bus.project().clone();
        for step in 0..90 {
            let mut scratch = bus.project().clone();
            let (k, cmd) = external_command(&mut rng, &mut scratch);
            bus.reserve_ids(scratch.next_id);
            let ok = bus.execute(cmd, origins[rng.below(5) as usize]).is_ok();
            seen[k][usize::from(!ok)] += 1;
            // A track now and then, so units meet tracks that come and go.
            if step % 17 == 0 {
                let mut p = bus.project().clone();
                let t = p.make_track("late");
                bus.reserve_ids(p.next_id);
                let _ = bus.execute(
                    Command::InsertTrack {
                        index: 0,
                        track: Box::new(t),
                    },
                    Origin::User,
                );
            }
        }
        let mut end = bus.project().clone();
        while bus.undo().is_some() {}
        let mut back = bus.project().clone();
        back.next_id = start.next_id;
        assert_eq!(back, start, "seed {seed}: undo all");
        while bus.redo().is_some() {}
        let mut again = bus.project().clone();
        again.next_id = 0;
        end.next_id = 0;
        assert_eq!(again, end, "seed {seed}: redo all");
    }
    assert!(
        seen[..KINDS - 1].iter().all(|k| k[0] > 0) && seen[KINDS - 1][1] > 0,
        "an external command never applied, or the refused one never refused: {seen:?}"
    );
}

#[test]
fn a_measurement_is_rebased_by_the_interfaces_round_trip_and_flagged_on_another_interface() {
    use external_unit::{Measured, Now, Staleness};
    let m = Measured {
        rtt: 600,
        buffer: 256,
        device: "Studio Interface".into(),
        ..Measured::bare(1_000, 48_000, 30.0)
    };
    let at = |rtt, buffer, device: &str, rate| Now {
        device: device.into(),
        rtt,
        buffer,
        rate,
    };
    let same = at(600, 256, "Studio Interface", 48_000);
    assert_eq!(m.staleness(&same), Staleness::Fresh);
    assert_eq!(m.rebased(&same).frames, 1_000);
    // The buffer doubled: the interface's own trip grew by 512 frames, so does the gear's.
    let bigger = at(1_112, 512, "Studio Interface", 48_000);
    assert_eq!(
        m.staleness(&bigger),
        Staleness::Rebased {
            measured_buffer: 256,
            now_buffer: 512
        }
    );
    assert_eq!(m.rebased(&bigger).frames, 1_512);
    assert!(!m.staleness(&bigger).suggests_ping());
    // At another rate the interface's report is converted to the measurement's.
    assert_eq!(
        m.rebased(&at(2_224, 1_024, "Studio Interface", 96_000))
            .frames,
        1_512
    );
    // Another interface: used as it is, and a new measurement is suggested.
    let other = at(300, 64, "Other Interface", 48_000);
    assert_eq!(
        m.staleness(&other),
        Staleness::OtherDevice {
            measured_buffer: 256
        }
    );
    assert!(m.staleness(&other).suggests_ping());
    assert_eq!(m.rebased(&other).frames, 1_000);
    // Nothing recorded about the interface (an older measurement): not re-based.
    assert!(Measured::bare(1_000, 48_000, 30.0)
        .staleness(&same)
        .suggests_ping());
}

#[test]
fn a_track_with_outboard_gear_cannot_be_frozen_and_says_why() {
    use cantus_core::freeze::{freeze_commands, CannotFreeze};
    let (bus, t) = bus_with_tracks(1);
    let mut p = bus.project().clone();
    let u = synth(&mut p, t[0]);
    p.external_units.push(u);
    assert_eq!(
        freeze_commands(&p, FreezeTarget::Track(t[0])),
        Err(CannotFreeze::External(t[0]))
    );
    // Switched off, it is no part of the sound: the freeze goes ahead.
    p.external_units[0].enabled = false;
    assert!(freeze_commands(&p, FreezeTarget::Track(t[0])).is_ok());
}

#[test]
fn a_units_removal_after_its_track_is_gone_undoes_exactly() {
    let (mut bus, t) = bus_with_tracks(2);
    let mut p = bus.project().clone();
    let u = synth(&mut p, t[0]);
    bus.reserve_ids(p.next_id);
    bus.execute(
        Command::InsertExternalUnit {
            index: 0,
            unit: Box::new(u.clone()),
        },
        Origin::User,
    )
    .unwrap();
    let before = bus.project().clone();
    bus.execute(Command::RemoveTrack { id: t[0] }, Origin::User)
        .unwrap();
    bus.execute(Command::RemoveExternalUnit { id: u.id }, Origin::User)
        .unwrap();
    bus.undo().expect("the unit back, its track still gone");
    bus.undo().expect("the track back");
    assert_eq!(*bus.project(), before);
}
