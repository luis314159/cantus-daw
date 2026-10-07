//! The engine's limits (ADR 0031 §1.4): 256 tracks and 256 strips play; a 257th is
//! refused — and said so (`EngineStats::refused_tracks`, `refused_strips`), never dropped
//! silently.

use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

/// `tracks` tracks, each playing a note from the first beat, on `strips` strips (track `i`
/// on strip `i % strips`).
fn project(tracks: usize, strips: usize) -> Project {
    let mut p = Project::new("limits");
    p.tempo_bpm = 120.0;
    while p.strips.len() < strips {
        let s = p.make_strip(format!("S{}", p.strips.len() + 1));
        p.strips.push(s);
    }
    p.strips.truncate(strips);
    for i in 0..tracks {
        let mut t = p.make_track(format!("T{}", i + 1));
        t.strip = Some(p.strips[i % strips].id);
        let mut clip = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
        clip.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(3), 48 + (i % 24) as u8, 100));
        t.clips.push(clip);
        p.tracks.push(t);
    }
    p
}

/// Play `p` for a few blocks; the last meters and the stats.
fn play(p: &Project) -> (Meters, EngineStats) {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let (mut tx, rx) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (etx, mut erx) = rtrb::RingBuffer::<EngineEvent>::new(256);
    let mut proc_ = Processor::without_midi(48_000.0, rx, etx);
    tx.push(EngineMessage::SetSnapshot(snap)).unwrap();
    tx.push(EngineMessage::Play).unwrap();
    let mut buf = vec![0.0f32; 512 * 2];
    let mut meters = Meters::default();
    for _ in 0..4 {
        proc_.process(&mut buf, 2);
        while let Ok(ev) = erx.pop() {
            if let EngineEvent::Meters(m) = ev {
                meters = m;
            }
        }
    }
    (meters, *proc_.stats())
}

#[test]
fn two_hundred_and_fifty_six_tracks_on_as_many_strips_all_play() {
    let p = project(MAX_TRACKS, MAX_STRIPS);
    assert_eq!((MAX_TRACKS, MAX_STRIPS), (256, 256));
    let (m, stats) = play(&p);
    assert_eq!(m.track_count, 256);
    assert_eq!(m.strip_count, 256);
    for i in 0..256 {
        assert!(m.tracks[i] > 0.0, "track {} is silent", i + 1);
        assert!(m.strips[i] > 0.0, "strip {} is silent", i + 1);
    }
    assert!(m.master[0] > 0.0);
    assert_eq!((stats.refused_tracks, stats.refused_strips), (0, 0));
}

#[test]
fn a_257th_track_is_refused_and_counted() {
    let p = project(MAX_TRACKS + 1, 16);
    let (m, stats) = play(&p);
    assert_eq!(m.track_count, 256, "the engine plays its limit");
    assert!(
        (0..256).all(|i| m.tracks[i] > 0.0),
        "and every one of those plays"
    );
    assert_eq!(stats.refused_tracks, 1, "the 257th is refused, and said so");
    assert_eq!(stats.refused_strips, 0);
}

#[test]
fn a_257th_strip_is_refused_and_counted() {
    let p = project(MAX_STRIPS + 1, MAX_STRIPS + 1);
    let (m, stats) = play(&p);
    assert_eq!(m.strip_count, 256);
    assert!((0..256).all(|i| m.strips[i] > 0.0));
    assert_eq!(
        stats.refused_strips, 1,
        "the 257th strip is refused, and said so"
    );
    assert_eq!(stats.refused_tracks, 1);
}
