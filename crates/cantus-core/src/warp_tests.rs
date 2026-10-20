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
        w.markers.push(WarpMarker::at(k as f64 * per, Ticks(*t)));
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
    // Outside the window nothing is touched: only the hit inside it moves.
    let narrow = w.quantise(&frames, Ticks(480), 1.0, Ticks(900), Ticks(1_000));
    assert!((narrow.tick_at(frames[2]) - 960.0).abs() < 1e-6);
    for t in [0.0, 480.0, 899.0, 1_000.0, 1_440.0, 3_000.0] {
        assert!(
            (narrow.frame_at(t) - w.frame_at(t)).abs() < 1e-6,
            "tick {t} moved"
        );
    }
    w = narrow;
    assert!(w.is_valid());
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
    use crate::audio_time::{clip_frame_at, clip_tick_at_frame};
    use crate::tempo::{TempoMap, TempoPoint};
    use crate::time::TimeSignature;
    let flat = TempoMap::constant(120.0, TimeSignature::default());
    let (start, offset) = (Ticks(1_920), Ticks(960));
    let mut a = AudioRegion::of(AudioSourceId(1));
    a.slip_frames = 12;
    // Unwarped: seconds at the project's tempo from the content origin, plus the fine
    // offset.
    assert_eq!(
        clip_frame_at(&flat, start, offset, &a, SR as f64, 2_880.0),
        48_012.0
    );
    let t = clip_tick_at_frame(&flat, start, offset, &a, SR as f64, 48_012.0);
    assert!((t - 2_880.0).abs() < 1e-9);
    // Warped: content tick (timeline tick less the origin) through the markers — under
    // any tempo map, so the clip stays on its beats through a ramp.
    a.warp = Some(Box::new(warp()));
    let ramp = TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: 120.0,
                ramp: true,
            },
            TempoPoint {
                tick: Ticks::beats(16),
                bpm: 170.0,
                ramp: false,
            },
        ],
        &[],
    );
    for map in [&flat, &ramp] {
        let at = (start.0 - offset.0 + 960) as f64;
        assert_eq!(
            clip_frame_at(map, start, offset, &a, SR as f64, at),
            warp().markers[1].frame
        );
        let back = clip_tick_at_frame(map, start, offset, &a, SR as f64, warp().markers[2].frame);
        assert!((back - (start.0 - offset.0 + 1_900) as f64).abs() < 1e-6);
    }
    // Not following: natural speed again.
    a.warp.as_mut().unwrap().follow = false;
    assert_eq!(
        clip_frame_at(&flat, start, offset, &a, SR as f64, 2_880.0),
        48_012.0
    );
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

#[test]
fn a_stretched_warp_plays_its_window_longer() {
    let w = warp();
    let s = w.stretched(Ticks(960), 2.0);
    assert!(s.is_valid());
    // The frame heard at a content tick is now heard twice as far from the anchor.
    for t in [960.0, 1_500.0, 2_880.0, 4_000.0] {
        let f = w.frame_at(t);
        let back = s.tick_at(f);
        assert!(
            (back - (960.0 + (t - 960.0) * 2.0)).abs() <= 1.0,
            "{t}: {back}"
        );
    }
    let one = Warp::identity(SR, 120.0, 0).stretched(Ticks::ZERO, 1.5);
    assert!((one.bpm - 180.0).abs() < 1e-9);
    assert!((one.tick_at(48_000.0) - 2.0 * 1.5 * PPQ as f64).abs() < 1e-6);
}

/// Round 29's review: quantising moves the hits inside the range and nothing else — the
/// range's ends are pinned first, so the audio after the last pulled hit keeps its speed.
#[test]
fn quantise_leaves_the_audio_beyond_the_range_alone() {
    let w = Warp::identity(48_000, 120.0, 0);
    let per = frames_per_tick(48_000, 120.0);
    // Hits at ticks 960 and 1 940: the second pulled back to 1 920.
    let hits = [960.0 * per, 1_940.0 * per];
    let (from, to) = (Ticks::ZERO, Ticks(3_840));
    let q = w.quantise(&hits, Ticks(240), 1.0, from, to);
    assert!(q.is_valid());
    assert!((q.tick_at(hits[1]) - 1_920.0).abs() < 1e-6);
    for t in [3_840.0, 5_000.0, 9_000.0] {
        assert!(
            (q.frame_at(t) - w.frame_at(t)).abs() < 1e-6,
            "tick {t}: {} against {}",
            q.frame_at(t),
            w.frame_at(t)
        );
    }
}

/// Round 29's review: the warp a first edit starts from plays the clip exactly as it plays
/// unwarped — under a tempo track, a ramp, a window into the middle of its take — and under
/// one tempo it is the identity.
#[test]
fn the_natural_warp_plays_as_the_clip_plays_unwarped() {
    use crate::tempo::{TempoMap, TempoPoint};
    use crate::time::TimeSignature;
    let map = TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: 120.0,
                ramp: false,
            },
            TempoPoint {
                tick: Ticks::beats(16),
                bpm: 90.0,
                ramp: true,
            },
            TempoPoint {
                tick: Ticks::beats(24),
                bpm: 140.0,
                ramp: false,
            },
        ],
        &[],
    );
    let (start, offset, length, slip) = (Ticks::beats(12), Ticks::beats(6), Ticks::beats(20), 37);
    let w = Warp::natural_of(&map, start, offset, length, slip, 48_000);
    assert!(w.is_valid() && w.follow);
    let origin = (start - offset).0 as f64;
    for k in 0..=200 {
        let t = start.0 as f64 + length.0 as f64 * k as f64 / 200.0;
        let want = crate::audio_time::frame_at(&map, start, offset, slip as f64, 48_000.0, t);
        let got = w.frame_at(t - origin);
        // Within half a frame everywhere (round 29s second review: inside the ramp too).
        assert!((got - want).abs() < 0.5, "tick {t}: {got} against {want}");
    }
    let one = TempoMap::constant(120.0, TimeSignature::default());
    let w = Warp::natural_of(&one, start, offset, length, slip, 48_000);
    assert!(w.is_identity(120.0, slip));
}

// --- Round 29's second review: the natural warp's own markers -------------------------

/// A map holding 120 BPM, then a ramp from `from` to `to` BPM over `beats` beats from beat
/// 8, then holding `to`.
fn ramp_map(from: f64, to: f64, beats: i64) -> crate::tempo::TempoMap {
    use crate::tempo::{TempoMap, TempoPoint};
    use crate::time::TimeSignature;
    TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[
            TempoPoint {
                tick: Ticks::ZERO,
                bpm: 120.0,
                ramp: false,
            },
            TempoPoint {
                tick: Ticks::beats(8),
                bpm: from,
                ramp: true,
            },
            TempoPoint {
                tick: Ticks::beats(8 + beats),
                bpm: to,
                ramp: false,
            },
        ],
        &[],
    )
}

/// Finding 37: a ramp's curve is kept within half a frame everywhere — steep ramps over a
/// beat and over four, a slowing one, the gentle one of the first review — and only the
/// window's ends and its content origin are the user's markers.
#[test]
fn a_natural_warp_keeps_steep_ramps_within_half_a_frame() {
    for (from, to, beats) in [
        (90.0, 140.0, 8),
        (60.0, 180.0, 4),
        (60.0, 180.0, 1),
        (120.0, 60.0, 4),
        (180.0, 60.0, 1),
    ] {
        let map = ramp_map(from, to, beats);
        let (start, offset, length, slip) = (
            Ticks::beats(6) + Ticks(97),
            Ticks::beats(3),
            Ticks::beats(beats + 6),
            -21,
        );
        for rate in [44_100u32, 48_000, 96_000] {
            let w = Warp::natural_of(&map, start, offset, length, slip, rate);
            assert!(w.is_valid() && w.follow);
            let user: Vec<_> = w.markers.iter().filter(|m| !m.auto).collect();
            assert_eq!(user.len(), 3, "{from}->{to}/{beats}: {user:?}");
            let origin = (start - offset).0 as f64;
            let mut t = start.0 as f64;
            while t <= (start + length).0 as f64 {
                let want =
                    crate::audio_time::frame_at(&map, start, offset, slip as f64, rate as f64, t);
                let got = w.frame_at(t - origin);
                assert!(
                    (got - want).abs() < 0.5,
                    "{from}->{to} over {beats} at {rate}: tick {t}: {got} against {want}"
                );
                t += 3.7;
            }
        }
    }
}

/// Finding 18 (a): a user's marker dragged inside a ramp crosses the warp's own markers —
/// they keep their frames and their share of the distance, none is dropped.
#[test]
fn a_marker_dragged_in_a_ramp_crosses_the_warps_own() {
    let map = ramp_map(90.0, 150.0, 8);
    let (start, offset, length) = (Ticks::beats(8), Ticks::ZERO, Ticks::beats(8));
    let w = Warp::natural_of(&map, start, offset, length, 0, SR);
    let autos = w.markers.iter().filter(|m| m.auto).count();
    assert!(autos > 20, "{autos}");
    let a = w.add_marker(Ticks::beats(4) + Ticks(13)).expect("a marker");
    let i = a.marker_at(Ticks::beats(4) + Ticks(13)).unwrap();
    assert!(!a.markers[i].auto);
    // Two beats on: eight sixteenths, many of the warp's own markers.
    let target = Ticks::beats(6) + Ticks(13);
    let b = a.move_marker(i, target);
    assert!(b.is_valid());
    assert_eq!(b.markers[i].tick, target);
    assert_eq!(b.markers[i].frame, a.markers[i].frame);
    assert_eq!(b.markers.len(), a.markers.len());
    assert_eq!(
        b.markers.iter().filter(|m| m.auto).count(),
        a.markers.iter().filter(|m| m.auto).count()
    );
    // The window ends never move.
    for t in [0.0, 8.0 * 960.0] {
        assert!((b.frame_at(t) - w.frame_at(t)).abs() < 1e-6, "tick {t}");
    }
    // Content slid under it too, carrying the warp's own along in order.
    let c = b.move_content(i, b.markers[i].frame + 4_000.0);
    assert!(c.is_valid());
    assert_eq!(c.markers[i].tick, target);
}

/// Finding 18 (b): *Quantise audio* under a ramp puts every hit on its grid line exactly —
/// never a tick off beside a sample of the curve with the audio squeezed into that tick.
#[test]
fn quantise_under_a_ramp_lands_every_hit_on_its_line() {
    let map = ramp_map(80.0, 160.0, 16);
    let (start, offset, length) = (Ticks::beats(8), Ticks::ZERO, Ticks::beats(16));
    let w = Warp::natural_of(&map, start, offset, length, 0, SR);
    // A hit 70 ticks after every other sixteenth line.
    let hits: Vec<f64> = (1..32).map(|k| w.frame_at((k * 480 + 70) as f64)).collect();
    let line = |t: f64| (t / 240.0).round() * 240.0;
    let q = w.quantise_to(&hits, &line, 1.0, offset, offset + length);
    assert!(q.is_valid());
    for (k, f) in (1..).zip(&hits) {
        let heard = q.tick_at(*f);
        assert!(
            (heard - (k * 480) as f64).abs() < 1e-6,
            "hit {k}: heard at {heard}"
        );
    }
}

/// The warp's own markers are written only when they are (a user's marker's file is what it
/// was), and read back.
#[test]
fn the_warps_own_markers_go_to_the_file() {
    let map = ramp_map(90.0, 150.0, 8);
    let w = Warp::natural_of(&map, Ticks::beats(8), Ticks::ZERO, Ticks::beats(8), 0, SR);
    let text = serde_json::to_string(&w).unwrap();
    assert!(text.contains("\"auto\":true"));
    let back: Warp = serde_json::from_str(&text).unwrap();
    assert_eq!(back, w);
    let user = serde_json::to_string(&WarpMarker::at(1.0, Ticks(2))).unwrap();
    assert!(!user.contains("auto"), "{user}");
}
