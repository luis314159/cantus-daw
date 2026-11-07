//! The hash an old song's Chorda renders to (round 37, ADR 0098): the v53 fixture's Chorda
//! track alone, given a clip of notes and a pedal lane, rendered offline.

use cantus_core::automation::AutoPoint;
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::*;
use cantus_engine::{render, RenderOptions};
use std::sync::Arc;

pub fn chorda_only(old: Project) -> Project {
    // The old song's Chorda, as it was saved and migrated, on a track of its own in a
    // plain project (the fixture's other state — its pattern mode, its other tracks — is
    // not what this measures).
    let spec = old
        .tracks
        .iter()
        .find_map(|t| {
            t.instrument
                .native()
                .filter(|n| n.kind == InstrumentKind::Chorda)
        })
        .cloned()
        .expect("a Chorda");
    let mut p = Project::new("old song");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Chorda");
    t.instrument = InstrumentSpec::Native(spec);
    p.tracks.push(t);
    let mut clip = p.make_clip("Old song", Ticks::ZERO, Ticks::beats(8));
    for (start, len, key, vel) in [
        (0.0, 4.0, 36u8, 90u8),
        (0.0, 4.0, 48, 80),
        (0.5, 1.0, 64, 70),
        (1.0, 1.0, 67, 60),
        (2.0, 2.0, 72, 100),
        (4.0, 0.5, 96, 110),
    ] {
        let n = p.make_note(
            Ticks((start * Ticks::QUARTER.0 as f64) as i64),
            Ticks((len * Ticks::QUARTER.0 as f64) as i64),
            key,
            vel,
        );
        clip.notes.push(n);
    }
    clip.lanes.push(ClipLane::new(
        LaneKind::Cc(64),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.0),
            AutoPoint::new(Ticks::QUARTER, 0.0),
            AutoPoint::new(Ticks::QUARTER, 1.0),
            AutoPoint::new(Ticks::beats(5), 1.0),
            AutoPoint::new(Ticks::beats(5), 0.0),
        ],
    ));
    p.tracks[0].clips.push(clip);
    p
}

pub fn hash_render(p: &Project) -> u64 {
    let r = render(
        Arc::new(Snapshot::compile(p, 1)),
        &RenderOptions {
            tail_seconds: 1.0,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.peak > 0.001, "it sounds: {}", r.peak);
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for x in &r.samples {
        for b in x.to_bits().to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}
