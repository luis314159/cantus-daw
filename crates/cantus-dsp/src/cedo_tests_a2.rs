//! Cedo, phase A-2: the sharper linear-phase kernel, the fade when the song starts or jumps
//! inside a duck, the RMS detector beside the peak, and MIDI velocity on the depth.

use super::band::{self, Band, Crossover};
use super::env::Detector;
use super::testkit::{db, rms, run, shape_fn, sine, Event, Run};
use super::{latency_for, Cedo, CedoSettings, Trigger, DEPTH_INF};
use crate::fluctus::CurvePoint;

const SR: f32 = 48_000.0;

fn unit(s: &CedoSettings, shape: &[CurvePoint]) -> Cedo {
    let mut c = Cedo::new(SR, shape_fn);
    c.set_shape(shape);
    c.set(s);
    c.reset();
    c
}

const FULL: [CurvePoint; 1] = [CurvePoint {
    x: 0.0,
    y: 1.0,
    tension: 0.0,
    shape: 0,
}];
/// Full depth for the first quarter of each cycle, nothing after.
const SQUARE: [CurvePoint; 2] = [
    CurvePoint {
        x: 0.0,
        y: 1.0,
        tension: 0.0,
        shape: 2,
    },
    CurvePoint {
        x: 0.25,
        y: 0.0,
        tension: 0.0,
        shape: 2,
    },
];

fn sine_gain(c: &mut Cedo, hz: f32) -> f64 {
    let n = 144_000;
    let x = sine(hz, 0.25, n, SR);
    let lat = c.latency();
    let ev = [(
        0usize,
        Event::Play {
            song: 0,
            bpm: 120.0,
        },
    )];
    let (o, _) = run(
        c,
        &Run {
            left: &x,
            right: &x,
            key: None,
            events: &ev,
            block: 256,
        },
    );
    db(rms(&o[n / 2..]) / rms(&x[n / 2 - lat..n - lat]))
}

#[test]
fn the_linear_phase_split_reaches_the_depth_and_leaks_little() {
    // Its latency is fixed and stated: the 60 Hz kernel's centre at every crossover (the
    // default Auto floor, 60 Hz as committed for the default 120 Hz crossover).
    assert_eq!(band::linear_latency(60.0, 48_000.0), 1_765);
    for hz in [60.0f32, 120.0, 500.0, 2_000.0] {
        let s = CedoSettings {
            trigger: Trigger::Tempo,
            depth_db: 24.0,
            band: Band::Lows,
            crossover: Crossover::LinearPhase,
            cross_hz: hz,
            ..CedoSettings::default()
        };
        assert_eq!(latency_for(&s, SR), 1_765);
        assert_eq!(unit(&s, &FULL).latency(), 1_765);
    }
    let mut rows = String::new();
    for band in [Band::Lows, Band::Highs] {
        let s = CedoSettings {
            trigger: Trigger::Tempo,
            depth_db: 24.0,
            band,
            crossover: Crossover::LinearPhase,
            cross_hz: 120.0,
            edge_ms: 0.0,
            ..CedoSettings::default()
        };
        let g = |hz: f32| sine_gain(&mut unit(&s, &FULL), hz);
        if band == Band::Lows {
            let (g30, g60, g240, g480) = (g(30.0), g(60.0), g(240.0), g(480.0));
            rows += &format!(
                "lows: 30 Hz {g30:+.2}, 60 Hz {g60:+.2}, 240 Hz {g240:+.2}, 480 Hz {g480:+.3} dB; "
            );
            assert!(g30 <= -23.5, "lows at 30 Hz: {g30}");
            assert!(g240 > -0.3 && g480 > -0.05, "{g240} {g480}");
        } else {
            let (g60, g30, g240) = (g(60.0), g(30.0), g(240.0));
            rows += &format!("highs: 30 Hz {g30:+.3}, 60 Hz {g60:+.3}, 240 Hz {g240:+.2} dB");
            assert!(g60 > -0.3, "highs leak an octave below: {g60}");
            assert!(g240 <= -23.0, "{g240}");
        }
    }
    println!("linear phase, depth 24 dB at 120 Hz — {rows}");
}

#[test]
fn a_seek_into_a_duck_fades_in_and_a_bar_line_does_not() {
    // 120 BPM, 1/4: a cycle of 24 000 samples ducked over its first 6 000.
    let s = CedoSettings {
        trigger: Trigger::Tempo,
        depth_db: DEPTH_INF,
        edge_ms: 0.0,
        ..CedoSettings::default()
    };
    let n = 60_000;
    let one = vec![1.0f32; n];
    // A seek at sample 10 000 to song sample 49 000: 1 000 samples into a duck.
    let ev = [
        (
            0usize,
            Event::Play {
                song: 0,
                bpm: 120.0,
            },
        ),
        (
            10_000,
            Event::Play {
                song: 49_000,
                bpm: 120.0,
            },
        ),
    ];
    let mut c = unit(&s, &SQUARE);
    let (o, _) = run(
        &mut c,
        &Run {
            left: &one,
            right: &one,
            key: None,
            events: &ev,
            block: 64,
        },
    );
    let fade = (super::shape::START_FADE_MS as f64 * 48.0).round() as usize;
    assert_eq!(o[9_999], 1.0, "before the seek, between ducks");
    assert!(o[10_000] > 0.99, "the seek lands softly: {}", o[10_000]);
    assert!(o[10_000 + fade / 2] > 0.2 && o[10_000 + fade / 2] < 0.8);
    assert_eq!(
        o[10_000 + fade],
        0.0,
        "and is at the shape's depth after the fade"
    );
    // The next cycle (song 72 000, here sample 33 000) ducks at once, as drawn.
    assert_eq!(o[32_999], 1.0);
    assert_eq!(o[33_000], 0.0);
    // A start on a bar line inside a long cycle (8/1) is not faded: a render from bar 5
    // stays the whole render's.
    let long = CedoSettings { division: 0, ..s };
    let mut c = unit(
        &long,
        &[
            CurvePoint {
                x: 0.0,
                y: 1.0,
                tension: 0.0,
                shape: 2,
            },
            CurvePoint {
                x: 0.9,
                y: 0.0,
                tension: 0.0,
                shape: 2,
            },
        ],
    );
    let ev = [(
        0usize,
        Event::Play {
            song: 4 * 96_000,
            bpm: 120.0,
        },
    )];
    let (o, _) = run(
        &mut c,
        &Run {
            left: &one,
            right: &one,
            key: None,
            events: &ev,
            block: 64,
        },
    );
    assert_eq!(o[0], 0.0, "bar 5 lands mid-cycle on a bar line: no fade");
    // A start between bar lines and between cycles' starts is faded.
    let mut c = unit(&s, &SQUARE);
    let ev = [(
        0usize,
        Event::Play {
            song: 1_234,
            bpm: 120.0,
        },
    )];
    let (o, _) = run(
        &mut c,
        &Run {
            left: &one,
            right: &one,
            key: None,
            events: &ev,
            block: 64,
        },
    );
    assert!(o[0] > 0.99 && o[fade] == 0.0, "{} {}", o[0], o[fade]);
}

#[test]
fn the_rms_detector_hears_the_level_and_the_peak_hears_the_peaks() {
    let n = 24_000;
    // A 1 kHz key at 0.5: its peak −6 dBFS, its RMS −9 dBFS.
    let key = sine(1_000.0, 0.5, n, SR);
    let one = vec![1.0f32; n];
    let mut fired = Vec::new();
    for (det, thr) in [
        (Detector::Peak, -7.5f32),
        (Detector::Rms, -7.5),
        (Detector::Rms, -10.0),
    ] {
        let s = CedoSettings {
            trigger: Trigger::Sidechain,
            depth_db: 12.0,
            threshold_db: thr,
            detector: det,
            rms_ms: 10.0,
            ..CedoSettings::default()
        };
        let mut c = unit(&s, &[]);
        let (o, _) = run(
            &mut c,
            &Run {
                left: &one,
                right: &one,
                key: Some((&key, &key)),
                events: &[],
                block: 128,
            },
        );
        fired.push(o.iter().any(|v| *v < 1.0));
    }
    assert_eq!(
        fired,
        vec![true, false, true],
        "peak over −7.5, RMS under it, RMS over −10"
    );
    // The RMS window's time: a key stepping to 0.5 reaches an RMS threshold of −10 dBFS when
    // the one-pole mean square does: t = −τ·ln(1 − thr²/0.25).
    let mut step = vec![0.0f32; n];
    for v in &mut step[2_000..] {
        *v = 0.5;
    }
    let s = CedoSettings {
        trigger: Trigger::Sidechain,
        depth_db: 12.0,
        threshold_db: -10.0,
        detector: Detector::Rms,
        rms_ms: 10.0,
        attack_ms: 0.0,
        ..CedoSettings::default()
    };
    let mut c = unit(&s, &[]);
    let (o, _) = run(
        &mut c,
        &Run {
            left: &one,
            right: &one,
            key: Some((&step, &step)),
            events: &[],
            block: 1,
        },
    );
    let first = o.iter().position(|v| *v < 1.0).unwrap() - 2_000;
    let thr = crate::math::db_to_gain(-10.0) as f64;
    let want = -480.0 * crate::math::ln(1.0 - thr * thr / 0.25);
    println!(
        "RMS 10 ms: fires {first} samples after the step (the one-pole's own crossing {want:.1})"
    );
    assert!((first as f64 - want).abs() <= 1.5, "{first} vs {want}");
}

#[test]
fn velocity_scales_the_depth_when_asked() {
    let n = 60_000;
    let one = vec![1.0f32; n];
    let notes = [
        (1_000usize, Event::Note(127)),
        (25_000, Event::Note(64)),
        (49_000, Event::Note(1)),
    ];
    for vel in [false, true] {
        let s = CedoSettings {
            trigger: Trigger::Midi,
            depth_db: 24.0,
            edge_ms: 0.0,
            velocity: vel,
            ..CedoSettings::default()
        };
        let mut c = unit(&s, &SQUARE);
        let (o, _) = run(
            &mut c,
            &Run {
                left: &one,
                right: &one,
                key: None,
                events: &notes,
                block: 64,
            },
        );
        let at = |i: usize| db(o[i + 2_000] as f64);
        let got = [at(1_000), at(25_000), at(49_000)];
        let want = if vel {
            [-24.0, -24.0 * 64.0 / 127.0, -24.0 / 127.0]
        } else {
            [-24.0; 3]
        };
        for (g, w) in got.iter().zip(want) {
            assert!((g - w).abs() < 0.01, "velocity {vel}: {got:?} vs {want:?}");
        }
        println!("velocity {vel}: full-amount gains {got:.2?} dB");
    }
}

#[test]
fn the_new_options_keep_the_null() {
    let n = 30_000;
    let x: Vec<f32> = sine(220.0, 0.4, n, SR);
    let key = sine(60.0, 0.9, n, SR);
    for s in [
        CedoSettings {
            detector: Detector::Rms,
            depth_db: 0.0,
            threshold_db: -40.0,
            ..CedoSettings::default()
        },
        CedoSettings {
            detector: Detector::Rms,
            depth_db: 12.0,
            threshold_db: -40.0,
            mix: 0.0,
            ..CedoSettings::default()
        },
        CedoSettings {
            trigger: Trigger::Midi,
            velocity: true,
            depth_db: 0.0,
            ..CedoSettings::default()
        },
        CedoSettings {
            trigger: Trigger::Tempo,
            band: Band::Highs,
            crossover: Crossover::LinearPhase,
            depth_db: 0.0,
            ..CedoSettings::default()
        },
    ] {
        let mut c = unit(&s, &FULL);
        let lat = c.latency();
        let ev = [
            (
                0usize,
                Event::Play {
                    song: 777,
                    bpm: 120.0,
                },
            ),
            (500, Event::Note(90)),
            (
                9_000,
                Event::Play {
                    song: 3,
                    bpm: 120.0,
                },
            ),
        ];
        let (o, _) = run(
            &mut c,
            &Run {
                left: &x,
                right: &x,
                key: Some((&key, &key)),
                events: &ev,
                block: 37,
            },
        );
        assert!(o[..lat].iter().all(|v| v.to_bits() == 0));
        assert!(
            o[lat..]
                .iter()
                .zip(&x)
                .all(|(a, b)| a.to_bits() == b.to_bits()),
            "{s:?}"
        );
    }
}
