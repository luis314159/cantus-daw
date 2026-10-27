//! MIDI files carry a clip's controller lanes and its notes' release velocities both ways
//! (ADR 0068): exported as the engine plays them (the curve sampled, a rest where the clip
//! ends), imported back as steps, within MIDI's own resolution.

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::*;
use cantus_project::midi::{self, ExportOptions, ImportOptions};

fn song() -> Project {
    let mut p = Project::new("Lanes");
    let mut t = p.make_track("Lead");
    let mut c = p.make_clip("Phrase", Ticks::ZERO, Ticks::beats(4));
    let mut n = p.make_note(Ticks::ZERO, Ticks::beats(1), 60, 100);
    n.release = 90;
    c.notes.push(n);
    // A note with no release velocity: written as the standard's 64, read back as none.
    c.notes
        .push(p.make_note(Ticks::beats(2), Ticks::beats(1), 64, 80));
    c.lanes = vec![
        ClipLane::new(
            LaneKind::PitchBend,
            vec![
                AutoPoint::new(Ticks::ZERO, 0.5),
                AutoPoint::new(Ticks::beats(1), 1.0),
            ],
        ),
        ClipLane::new(LaneKind::Cc(1), vec![AutoPoint::new(Ticks::ZERO, 0.75)]),
    ];
    t.clips.push(c);
    p.tracks.push(t);
    p
}

#[test]
fn release_velocities_and_lanes_round_trip_through_a_file() {
    let p = song();
    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    let import = midi::read(&bytes, &ImportOptions::default()).unwrap();
    let t = &import.tracks[0];
    let releases: Vec<u8> = t.notes.iter().map(|n| n.release).collect();
    assert_eq!(releases, vec![90, 64]);
    // The mod wheel: one message at the clip's start, its rest where the clip ends.
    let wheel: Vec<(Ticks, f32)> = t
        .controls
        .iter()
        .filter(|c| c.1 == LaneKind::Cc(1))
        .map(|c| (c.0, c.2))
        .collect();
    assert_eq!(wheel.len(), 2, "{wheel:?}");
    assert_eq!(wheel[0].0, Ticks::ZERO);
    assert!((wheel[0].1 - 95.0 / 127.0).abs() < 1e-6, "0.75 is CC 95");
    assert_eq!(wheel[1], (Ticks::beats(4), 0.0));
    // The bend: a ramp sampled every 1/96 of a beat, up to its top, then its centre at the
    // clip's end — every value within one 14-bit step of the curve.
    let bend: Vec<(Ticks, f32)> = t
        .controls
        .iter()
        .filter(|c| c.1 == LaneKind::PitchBend)
        .map(|c| (c.0, c.2))
        .collect();
    assert!(bend.len() > 90, "{}", bend.len());
    let lane = &p.tracks[0].clips[0].lanes[0];
    for (tick, v) in bend.iter().filter(|b| b.0 < Ticks::beats(4)) {
        let want = lane.value_at(tick.0 as f64).unwrap();
        assert!(
            (v - want).abs() <= 1.0 / 16384.0 + 1e-6,
            "{tick:?}: {v} vs {want}"
        );
    }
    assert_eq!(bend.last().copied(), Some((Ticks::beats(4), 0.5)));
}

#[test]
fn an_imported_clip_plays_its_lanes() {
    let p = song();
    let bytes = midi::export(&p, &ExportOptions::default()).unwrap();
    let mut q = Project::new("In");
    let import = midi::read(&bytes, &ImportOptions::default()).unwrap();
    let mut next = 1_000;
    let mut ids = || {
        next += 1;
        next
    };
    let built = midi::import_commands(&q, &import, &Default::default(), &mut ids, &|_| {
        "Lead".to_string()
    });
    for c in built.commands {
        c.apply(&mut q).unwrap();
    }
    let clip = &q.tracks.last().unwrap().clips[0];
    let kinds: Vec<LaneKind> = clip.lanes.iter().map(|l| l.kind).collect();
    assert_eq!(kinds, vec![LaneKind::PitchBend, LaneKind::Cc(1)]);
    // What it plays equals what the original played, to MIDI's resolution.
    let a = Snapshot::compile(&p, 0);
    let b = Snapshot::compile(&q, 0);
    let ta = a.tracks.last().unwrap();
    let tb = b.tracks.last().unwrap();
    for e in ta.controls.iter() {
        let got = tb
            .controls
            .iter()
            .rfind(|f| f.kind == e.kind && f.tick <= e.tick)
            .map(|f| f.value)
            .unwrap();
        assert!((got - e.value).abs() <= 1.0 / 127.0, "{e:?}: {got}");
    }
}

/// Review #13: controllers on a channel with no notes have no clip to live in — the
/// report says they were dropped, never silently.
#[test]
fn controllers_on_a_channel_with_no_notes_are_reported() {
    let track: Vec<u8> = vec![
        0x00, 0x90, 0x3C, 0x64, // C4 on, channel 1
        0x00, 0xB1, 0x01, 0x40, // the mod wheel on channel 2, which plays nothing
        0x00, 0xE1, 0x00, 0x50, // a bend on channel 2
        0x60, 0x80, 0x3C, 0x40, // C4 off
        0x00, 0xFF, 0x2F, 0x00,
    ];
    let mut bytes = b"MThd".to_vec();
    bytes.extend_from_slice(&6u32.to_be_bytes());
    bytes.extend_from_slice(&[0, 0, 0, 1, 0, 96]);
    bytes.extend_from_slice(b"MTrk");
    bytes.extend_from_slice(&(track.len() as u32).to_be_bytes());
    bytes.extend_from_slice(&track);
    let import = midi::read(&bytes, &ImportOptions::default()).unwrap();
    assert_eq!(import.tracks.len(), 1);
    assert!(import.report.has("interop.report.lone_controls"));
}
