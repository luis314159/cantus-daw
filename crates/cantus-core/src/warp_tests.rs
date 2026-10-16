//! The warp model's tests (ADR 0061): the map and its inverse, identity, validity, the
//! edits, and what a file holds.

use super::*;
use crate::audio::AudioRegion;
use crate::model::AudioSourceId;

const SR: u32 = 48_000;

/// A loop at 92 BPM: markers on its first four beats, the third played late.
fn warp() -> Warp {
    let per = SR as f64 * 60.0 / 92.0;
    let mut w = Warp::new(SR, 92.0);
    for (k, t) in [0i64, 960, 1_900, 2_880].iter().enumerate() {
        w.markers.push(WarpMarker {
            frame: k as f64 * per,
            tick: Ticks(*t),
        });
    }
    w
}

#[test]
fn the_map_runs_through_the_markers_and_inverts() {
    let w = warp();
    assert!(w.is_valid());
    for m in &w.markers {
        assert_eq!(w.frame_at(m.tick.0 as f64), m.frame, "on the marker");
        assert!((w.tick_at(m.frame) - m.tick.0 as f64).abs() < 1e-9);
    }
    // Between markers it is linear; beyond, the edge segments go on.
    let mid = w.frame_at(480.0);
    assert!((mid - (w.markers[0].frame + w.markers[1].frame) / 2.0).abs() < 1e-9);
    for t in [-500.0, 0.0, 333.3, 1_000.0, 2_000.0, 5_000.0, 40_000.0] {
        let f = w.frame_at(t);
        assert!((w.tick_at(f) - t).abs() < 1e-6, "{t}");
    }
    let edge = (w.frame_at(4_000.0) - w.frame_at(3_000.0)) / 1_000.0;
    let last = (w.markers[3].frame - w.markers[2].frame) / 980.0;
    assert!((edge - last).abs() < 1e-9);
}

#[test]
fn without_markers_the_source_plays_at_its_own_tempo() {
    let w = Warp::new(SR, 120.0);
    // One beat of content is half a second of source.
    assert_eq!(w.frame_at(PPQ as f64), 24_000.0);
    assert_eq!(w.tick_at(24_000.0), PPQ as f64);
    assert_eq!(w.tempo(), 120.0);
}

#[test]
fn the_identity_warp_is_told_and_nothing_else_is() {
    for (bpm, slip) in [(120.0, 0), (97.3, -37), (140.0, 1_234)] {
        let w = Warp::identity(SR, bpm, slip);
        assert!(w.is_identity(bpm, slip), "{bpm} {slip}");
        // Markers added on it keep it the identity.
        let w2 = w.add_marker(Ticks(4_321)).unwrap();
        assert!(w2.is_identity(bpm, slip));
        // Moving one, transposing, or another tempo does not.
        assert!(!w2.move_marker(1, Ticks(4_000)).is_identity(bpm, slip));
        let mut t = w.clone();
        t.cents = 1;
        assert!(!t.is_identity(bpm, slip));
        assert!(!w.is_identity(bpm + 1.0, slip));
        // Re-pitch has no transposition: its cents do not count.
        t.mode = WarpMode::Repitch;
        assert!(t.is_identity(bpm, slip));
        // Not following the tempo plays at natural speed: untransposed, the identity.
        let mut n = w2.move_marker(1, Ticks(4_000));
        n.follow = false;
        assert!(n.is_identity(bpm, slip));
    }
}

#[test]
fn a_warp_out_of_order_or_out_of_range_is_not_valid() {
    let w = warp();
    let mut bad = w.clone();
    bad.markers[2].tick = bad.markers[1].tick;
    assert!(!bad.is_valid());
    let mut bad = w.clone();
    bad.markers[2].frame = bad.markers[1].frame;
    assert!(!bad.is_valid());
    let mut bad = w.clone();
    bad.markers[0].frame = f64::NAN;
    assert!(!bad.is_valid());
    let mut bad = w.clone();
    bad.cents = MAX_CENTS + 1;
    assert!(!bad.is_valid());
    let mut bad = w.clone();
    bad.bpm = 0.0;
    assert!(!bad.is_valid());
    let mut bad = w;
    bad.rate = 0;
    assert!(!bad.is_valid());
}

#[test]
fn edits_keep_the_warp_valid() {
    let w = warp();
    // A marker where one is refuses; elsewhere it pins the frame heard there.
    assert!(w.add_marker(Ticks(960)).is_none());
    let a = w.add_marker(Ticks(1_500)).unwrap();
    assert!(a.is_valid());
    assert_eq!(a.frame_at(1_500.0), w.frame_at(1_500.0));
    assert_eq!(a.frame_at(700.0), w.frame_at(700.0), "nothing moved");
    // Moving clamps between the neighbours.
    let m = w.move_marker(1, Ticks(99_999));
    assert!(m.is_valid());
    assert_eq!(m.markers[1].tick, Ticks(1_899));
    let m = w.move_marker(1, Ticks(-5));
    assert_eq!(m.markers[1].tick, Ticks(1));
    // The content under a marker slides, never past a neighbour's frame.
    let c = w.move_content(1, 1e12);
    assert!(c.is_valid() && c.markers[1].frame < w.markers[2].frame);
    // On a transient's frame, at the tick it is heard.
    let f = w.frame_at(2_400.0) + 10.0;
    let t = w.add_marker_at_frame(f).unwrap();
    assert!(t.is_valid());
    assert!(t.markers.iter().any(|m| m.frame == f));
    // Removal.
    assert_eq!(w.remove_marker(0).markers.len(), 3);
    assert!(w.reset().markers.is_empty());
}

#[test]
fn quantise_pulls_transients_to_the_grid_by_its_strength() {
    // A loose take at 100 BPM, as recorded: hits a little off the eighths.
    let mut w = Warp::identity(SR, 100.0, 0);
    let per_tick = frames_per_tick(SR, 100.0);
    let late = [0.0, 497.0, 975.0, 1_430.0, 1_930.0];
    let frames: Vec<f64> = late.iter().map(|t| t * per_tick).collect();
    let full = w.quantise(&frames, Ticks(480), 1.0, Ticks::ZERO, Ticks(4_000));
    assert!(full.is_valid());
    for (f, want) in frames.iter().zip([0.0, 480.0, 960.0, 1_440.0, 1_920.0]) {
        assert!(
            (full.tick_at(*f) - want).abs() < 1e-6,
            "{f}: {}",
            full.tick_at(*f)
        );
    }
    let half = w.quantise(&frames, Ticks(480), 0.5, Ticks::ZERO, Ticks(4_000));
    let t = half.tick_at(frames[1]);
    assert!((t - 488.5).abs() <= 0.5, "half way: {t}");
    // Outside the window nothing is touched.
    w = w.quantise(&frames, Ticks(480), 1.0, Ticks(900), Ticks(1_000));
    assert_eq!(w.markers.len(), 2);
}

#[test]
fn a_grid_warps_a_loop_bar_by_bar() {
    // A loop at 92 BPM whose first downbeat is 0.3 s in.
    let per = SR as f64 * 60.0 / 92.0;
    let first = 0.3 * SR as f64;
    let beats: Vec<f64> = (0..16).map(|k| first + k as f64 * per).collect();
    let d = Detected {
        bpm: 92.0,
        confidence: 0.9,
        downbeat: first,
        bars: None,
    };
    let w = Warp::from_grid(SR, 92.0, &beats, 0, 4, d);
    assert!(w.is_valid());
    // The downbeat on the first whole bar after the lead-in, every bar a marker.
    assert!(w
        .markers
        .iter()
        .any(|m| m.frame == first && m.tick == Ticks(4 * PPQ)));
    assert_eq!(w.markers.iter().filter(|m| m.frame >= first).count(), 4);
    assert_eq!(w.frame_at(8.0 * PPQ as f64), beats[4]);
    assert!((w.tempo() - 92.0).abs() < 1e-6);
    // Frame 0 sits at a tick ≥ 0.
    assert!(w.tick_at(0.0) >= -1e-9);
    // A new tempo scales it about its first marker.
    let fast = w.with_tempo(184.0);
    assert!(fast.is_valid());
    assert!((fast.tempo() - 184.0).abs() < 0.5, "{}", fast.tempo());
}

#[test]
fn a_clip_reads_its_time_through_one_function() {
    let mut a = AudioRegion::of(AudioSourceId(1));
    a.slip_frames = 12;
    // Unwarped: seconds at the project's tempo, plus the fine offset.
    assert_eq!(source_frame(&a, 960.0, 120.0, SR), 24_012.0);
    assert!((content_tick(&a, 24_012.0, 120.0, SR) - 960.0).abs() < 1e-9);
    // Warped: through its markers (the fine offset is in them).
    a.warp = Some(Box::new(warp()));
    assert_eq!(source_frame(&a, 960.0, 120.0, SR), warp().markers[1].frame);
    // Not following: natural speed again.
    a.warp.as_mut().unwrap().follow = false;
    assert_eq!(source_frame(&a, 960.0, 120.0, SR), 24_012.0);
}

#[test]
fn a_clip_never_warped_writes_nothing_new() {
    let a = AudioRegion::of(AudioSourceId(3));
    let json = serde_json::to_string(&a).unwrap();
    assert!(!json.contains("warp"), "{json}");
    let mut b = a.clone();
    b.warp = Some(Box::new(warp()));
    let json = serde_json::to_string(&b).unwrap();
    // Defaults are not written: no cents, formants, tail, follow.
    assert!(
        !json.contains("cents") && !json.contains("follow"),
        "{json}"
    );
    assert_eq!(serde_json::from_str::<AudioRegion>(&json).unwrap(), b);
}
