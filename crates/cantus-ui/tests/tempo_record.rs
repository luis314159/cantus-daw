//! Review 2 (finding 29): loop recording under a tempo ramp — every pass's window, offset
//! and fine offset put the frame recorded at its start (the wrap, the latency) where it
//! plays, through the map — and a recovered take's start through the pass's own map.

use cantus_core::audio_time;
use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::FinishedTake;
use cantus_ui::record_pass::{audio_windows, PassPlan};

fn ramp() -> TempoMap {
    let p = |beat: i64, bpm: f64, ramp: bool| TempoPoint {
        tick: Ticks::beats(beat),
        bpm,
        ramp,
    };
    TempoMap::new(
        120.0,
        TimeSignature::default(),
        &[p(0, 120.0, false), p(4, 120.0, true), p(12, 60.0, false)],
        &[],
    )
}

#[test]
fn loop_passes_under_a_ramp_play_what_was_recorded_there() {
    let map = ramp();
    let rate = 48_000u32;
    let latency = 300i64;
    // A loop over beats 6–10, inside the ramp; recording from beat 6 with a count-in.
    let (a, b) = (Ticks::beats(6), Ticks::beats(10));
    let loop_frames = (map.seconds_between(a, b) * rate as f64).round() as u64;
    let start_tick =
        map.tick_after_seconds(Ticks::beats(5).0 as f64, -(latency as f64) / rate as f64);
    let first = (map.seconds_between_f64(start_tick, a.0 as f64) * rate as f64).round() as u64;
    let take = FinishedTake {
        track: TrackId(1),
        path: "take.wav".into(),
        sample_rate: rate,
        channels: 1,
        frames: first + 3 * loop_frames,
        start_tick,
        wraps: vec![first + loop_frames, first + 2 * loop_frames],
        wrap_shifts: Vec::new(),
        dropped: 0,
    };
    let mut plan = PassPlan::new(a);
    plan.loop_region = Some((a, b));
    let windows = audio_windows(&take, &plan, &map, latency);
    assert_eq!(windows.len(), 3);
    for (k, w) in windows.iter().enumerate() {
        let (start, _, offset) = w.ticks_in(&map).expect("a window");
        let slip = w.slip_frames_in(start, offset, &map, rate);
        let heard = audio_time::frame_at(
            &map,
            start,
            offset,
            slip as f64,
            rate as f64,
            start.0 as f64,
        );
        // What was recorded at the window's start: pass 0 from the take's first frame, a
        // later pass from its wrap, the latency earlier.
        let want = if k == 0 {
            map.seconds_between_f64(start_tick, start.0 as f64) * rate as f64
        } else {
            (take.wraps[k - 1] as i64 + latency) as f64
        };
        assert!((heard - want).abs() <= 1.0, "pass {k}: {heard} vs {want}");
    }
}

#[test]
fn a_recovered_takes_start_goes_back_through_the_ramp() {
    let mut pass: cantus_ui::recovery::JournalPass = serde_json::from_str(
        r#"{"record_start":5760,"transport_start":5760,"punch":null,"loop_region":null,
            "loop_takes":true,"mode":"Stack","tempo_bpm":120.0,"sample_rate":48000,
            "latency_frames":4800,"takes":[],"finished":false}"#,
    )
    .expect("a journal pass");
    pass.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: false,
        },
        TempoPoint {
            tick: Ticks::beats(4),
            bpm: 120.0,
            ramp: true,
        },
        TempoPoint {
            tick: Ticks::beats(8),
            bpm: 60.0,
            ramp: false,
        },
    ];
    // 0.1 s back from beat 6 (90 BPM), by hand: 4·ln(bpm(x)/90) s from beat x.
    let got = pass.tick_after(5760.0, -4800.0, 48_000);
    let bpm_x = 90.0 * (0.1f64 / 4.0).exp();
    let x = 4.0 + (120.0 - bpm_x) / 15.0;
    assert!((got - x * 960.0).abs() < 1e-6, "{got} vs {}", x * 960.0);
}
