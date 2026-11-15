//! Cedo measured: the pump on the song's grid (60, 120, 174 BPM, straight and swung,
//! against the grid in exact rationals), the sidechain's attack, hold and release against
//! their settings, the lookahead catching a kick's transient, the band split's response
//! (flat when not ducking, no bump when ducking), the splatter of the shape's edges, CPU.
//! Run with `--nocapture` for the numbers.

use super::band::{Band, Crossover};
use super::env::{KeyFilterMode, ReleaseShape};
use super::testkit::{db, gain_trace, grid_sample, rms, run, shape_fn, sine, Event, HighPass, Run};
use super::{Cedo, CedoSettings, Trigger, CLASSIC_PUMP, DEPTH_INF};
use crate::fluctus::CurvePoint;
use crate::modfx_clock::DIVISIONS;

const SR: f32 = 48_000.0;

fn unit(s: &CedoSettings, shape: &[CurvePoint]) -> Cedo {
    let mut c = Cedo::new(SR, shape_fn);
    c.set_shape(shape);
    c.set(s);
    c.reset();
    c
}

fn pt(x: f32, y: f32, shape: u8) -> CurvePoint {
    CurvePoint {
        x,
        y,
        tension: 0.0,
        shape,
    }
}

/// Full depth for the first quarter of each cycle, nothing after: hard edges.
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

#[test]
fn the_pump_lands_on_the_song_grid_to_the_sample() {
    let mut checked = 0;
    for bpm in [60i128, 120, 174] {
        for (swing, sw_num, sw_den) in [(0.0f32, 0i128, 1i128), (0.5, 1, 4), (1.0, 1, 2)] {
            for (div, b_den) in [(8usize, 1i128), (11, 2), (14, 4)] {
                assert_eq!(DIVISIONS[div], 1.0 / b_den as f64);
                for look in [0.0f32, 3.0] {
                    let s = CedoSettings {
                        trigger: Trigger::Tempo,
                        depth_db: DEPTH_INF,
                        division: div,
                        swing,
                        edge_ms: 0.0,
                        lookahead_ms: look,
                        ..CedoSettings::default()
                    };
                    let mut c = unit(&s, &SQUARE);
                    let lat = c.latency();
                    let n = (8.0 * 4.0 * 60.0 / bpm as f64 * SR as f64) as usize;
                    let one = vec![1.0f32; n];
                    let (o, _) = run(
                        &mut c,
                        &Run {
                            left: &one,
                            right: &one,
                            key: None,
                            events: &[(
                                0,
                                Event::Play {
                                    song: 0,
                                    bpm: bpm as f64,
                                },
                            )],
                            block: 37,
                        },
                    );
                    let got: Vec<i64> = (1..n)
                        .filter(|&i| o[i] == 0.0 && o[i - 1] == 1.0)
                        .map(|i| (i - lat) as i64)
                        .collect();
                    // Cycle j starts at (j + sw·(j odd))·b quarter notes.
                    let mut want = Vec::new();
                    for j in 1i128.. {
                        let (num, den) = if j % 2 == 0 {
                            (j, b_den)
                        } else {
                            (j * sw_den + sw_num, b_den * sw_den)
                        };
                        let g = grid_sample(num, den, bpm, SR as i128);
                        if g + lat as i64 >= n as i64 {
                            break;
                        }
                        want.push(g);
                    }
                    assert_eq!(got, want, "{bpm} BPM, swing {swing}, 1/{}", 4 * b_den);
                    checked += got.len();
                }
            }
        }
    }
    println!("pump onsets on the exact grid: {checked} of {checked}");
}

#[test]
fn a_smoothed_edge_is_centred_on_the_grid() {
    // 120 BPM, 1/8: a cycle of 12 000 samples, ducked over its first 3 000.
    for look in [0.0f32, 3.0] {
        let s = CedoSettings {
            trigger: Trigger::Tempo,
            depth_db: DEPTH_INF,
            division: 11,
            edge_ms: 2.0,
            lookahead_ms: look,
            ..CedoSettings::default()
        };
        let mut c = unit(&s, &SQUARE);
        let lat = c.latency();
        let n = 96_000;
        let one = vec![1.0f32; n];
        let (o, _) = run(
            &mut c,
            &Run {
                left: &one,
                right: &one,
                key: None,
                events: &[(
                    0,
                    Event::Play {
                        song: 0,
                        bpm: 120.0,
                    },
                )],
                block: 64,
            },
        );
        let h = 48usize;
        let mut worst = 0.0f32;
        for k in 1..7usize {
            // Into the duck at the cycle's start, out of it a quarter later.
            for (g, into) in [(k * 12_000 + lat, true), (k * 12_000 + 3_000 + lat, false)] {
                for d in 0..=h {
                    let pair = o[g + d] + o[g - 1 - d];
                    worst = worst.max((pair - 1.0).abs());
                }
                let (before, after) = if into { (1.0, 0.0) } else { (0.0, 1.0) };
                assert!(o[g - h - 1] == before && o[g + h] == after, "k{k} {into}");
                assert!(
                    o[g - h] != before && o[g + h - 1] != after,
                    "k{k} {into}: the ramp is 2 ms"
                );
            }
        }
        assert!(worst < 2e-6, "{worst}");
        println!(
            "lookahead {look} ms: every 2 ms edge symmetric about the grid (the sample before and the grid's sample sum to 1 within {worst:.1e}), {h} samples either side"
        );
    }
}

#[test]
fn the_sidechain_attack_hold_and_release_match_their_settings() {
    let n = 20_000;
    let mut key = vec![0.0f32; n];
    for v in &mut key[5_000..7_000] {
        *v = 0.5;
    }
    let one = vec![1.0f32; n];
    for shape in [
        ReleaseShape::Linear,
        ReleaseShape::Exponential,
        ReleaseShape::SCurve,
    ] {
        for (att, hold, rel) in [
            (5.0f32, 20.0f32, 50.0f32),
            (0.0, 0.0, 10.0),
            (12.5, 7.0, 120.0),
        ] {
            let s = CedoSettings {
                trigger: Trigger::Sidechain,
                depth_db: DEPTH_INF,
                threshold_db: -20.0,
                attack_ms: att,
                hold_ms: hold,
                release_ms: rel,
                release_shape: shape,
                ..CedoSettings::default()
            };
            let mut c = unit(&s, &[]);
            let (g, _) = run(
                &mut c,
                &Run {
                    left: &one,
                    right: &one,
                    key: Some((&key, &key)),
                    events: &[],
                    block: 1,
                },
            );
            let ms = |x: f32| (x as f64 * 48.0).round() as usize;
            let (a, h, r) = (ms(att), ms(hold), ms(rel).max(1));
            let first = g.iter().position(|v| *v < 1.0).unwrap();
            let full = g.iter().position(|v| *v == 0.0).unwrap();
            let last_full = g.iter().rposition(|v| *v == 0.0).unwrap();
            let back = (last_full..n).find(|&i| g[i] == 1.0).unwrap();
            assert_eq!(first, 5_000, "{shape:?}");
            assert_eq!(full, 5_000 + a.max(1) - 1, "{shape:?} attack");
            assert_eq!(last_full, 6_999 + h, "{shape:?} hold");
            assert_eq!(back, 6_999 + h + r, "{shape:?} release");
            let mid = 1.0 - g[6_999 + h + r / 2];
            let want = 1.0 - shape.at(0.5) as f32;
            assert!((mid - want).abs() < 1e-3, "{shape:?}: {mid}");
            if att == 5.0 {
                println!(
                    "{shape:?}: attack {:.3} ms, hold {:.3} ms, release {:.3} ms (set {att}/{hold}/{rel}); amount half way through the release {mid:.4}",
                    (full + 1 - first) as f32 / 48.0,
                    (last_full - 6_999) as f32 / 48.0,
                    (back - last_full) as f32 / 48.0
                );
            }
        }
    }
}

/// Pulsus's kick (model 2), hit at `hits`.
pub(super) fn kick(n: usize, hits: &[usize]) -> Vec<f32> {
    use crate::pulsus::{PadSettings, Pulsus};
    let mut p = Pulsus::new(SR);
    p.set_kit(2, 1, 4, false);
    p.set_pad_count(1);
    p.set_pad(0, 36, &PadSettings::kick());
    let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
    let mut at = 0;
    for (k, &h) in hits.iter().enumerate() {
        if h > at {
            p.process_add(&mut l[at..h], &mut r[at..h]);
            at = h;
        }
        p.note_on(36, 120, k as u64 + 1);
    }
    p.process_add(&mut l[at..], &mut r[at..]);
    l
}

#[test]
fn the_lookahead_catches_a_kicks_transient_fully() {
    let n = 48_000;
    let hits = [4_000usize, 28_000];
    let k = kick(n, &hits);
    let one = vec![1.0f32; n];
    let thr_db = -30.0f32;
    let thr = crate::math::db_to_gain(thr_db);
    let onsets: Vec<usize> = hits
        .iter()
        .map(|&h| (h..n).find(|&i| k[i].abs() >= thr).unwrap())
        .collect();
    for look in [0.0f32, 2.0, 5.0] {
        let s = CedoSettings {
            trigger: Trigger::Sidechain,
            depth_db: DEPTH_INF,
            threshold_db: thr_db,
            attack_ms: 5.0,
            hold_ms: 60.0,
            release_ms: 100.0,
            lookahead_ms: look,
            ..CedoSettings::default()
        };
        let mut c = unit(&s, &[]);
        let lat = c.latency();
        let (g, _) = run(
            &mut c,
            &Run {
                left: &one,
                right: &one,
                key: Some((&k, &k)),
                events: &[],
                block: 64,
            },
        );
        // The audio's gain at the kick's first sample over the threshold, and the kick's
        // energy that the duck had not yet reached (over the first 10 ms of the hit).
        for &t0 in &onsets {
            let at_onset = g[t0 + lat];
            let mut passed = 0.0f64;
            let mut total = 0.0f64;
            for i in t0..t0 + 480 {
                let e = (k[i] as f64) * (k[i] as f64);
                total += e;
                passed += e * (g[i + lat] as f64) * (g[i + lat] as f64);
            }
            println!(
                "lookahead {look} ms: gain at the kick's onset {:.4} ({:.2} dB); the kick's first 10 ms heard through the bass's duck {:.1} dB",
                at_onset,
                db(at_onset as f64),
                10.0 * crate::math::log10((passed / total).max(1e-30))
            );
            if look >= 5.0 {
                assert_eq!(at_onset, 0.0);
                assert!(passed == 0.0);
            } else if look == 0.0 {
                assert!(at_onset > 0.99);
            }
        }
    }
}

/// A sine's steady gain through `c` (both channels the same), dB.
fn sine_gain(c: &mut Cedo, hz: f32, events: &[(usize, Event)]) -> f64 {
    let n = 96_000;
    let x = sine(hz, 0.25, n, SR);
    let lat = c.latency();
    let (o, _) = run(
        c,
        &Run {
            left: &x,
            right: &x,
            key: None,
            events,
            block: 256,
        },
    );
    db(rms(&o[n / 2..]) / rms(&x[n / 2 - lat..n - lat]))
}

#[test]
fn the_band_split_is_flat_when_not_ducking_and_never_rises_when_ducking() {
    let play = [(
        0usize,
        Event::Play {
            song: 0,
            bpm: 120.0,
        },
    )];
    let full = [pt(0.0, 1.0, 0)];
    println!("ducked response (depth 24 dB, crossover 120 Hz), measured | drawn:");
    for cross in [Crossover::MinimumPhase, Crossover::LinearPhase] {
        for band in [Band::Lows, Band::Highs] {
            let s = CedoSettings {
                trigger: Trigger::Tempo,
                depth_db: 24.0,
                band,
                crossover: cross,
                cross_hz: 120.0,
                edge_ms: 0.0,
                ..CedoSettings::default()
            };
            let mut row = String::new();
            for hz in [30.0f32, 60.0, 120.0, 240.0, 480.0, 1_000.0, 5_000.0] {
                let mut c = unit(&s, &full);
                let got = sine_gain(&mut c, hz, &play);
                let drawn = c.response_db(hz, 1.0) as f64;
                row += &format!(" {hz} Hz {got:+.2}|{drawn:+.2}");
                assert!(got < 0.005, "{cross:?} {band:?} {hz}: rises {got}");
                assert!(
                    (got - drawn).abs() < 0.15,
                    "{cross:?} {band:?} {hz}: {got} vs {drawn}"
                );
            }
            println!("  {cross:?} {band:?}:{row}");
        }
    }
    // Not ducking — between the pump's hits, the amount exactly 0 — the delayed input to
    // the bit, with the filters warm from the ducking before.
    for cross in [Crossover::MinimumPhase, Crossover::LinearPhase] {
        let s = CedoSettings {
            trigger: Trigger::Tempo,
            depth_db: 24.0,
            band: Band::Lows,
            crossover: cross,
            edge_ms: 0.0,
            ..CedoSettings::default()
        };
        let mut c = unit(&s, &SQUARE);
        let n = 96_000;
        let x = super::testkit::pad(n, SR);
        let lat = c.latency();
        let (o, _) = run(
            &mut c,
            &Run {
                left: &x,
                right: &x,
                key: None,
                events: &play,
                block: 64,
            },
        );
        // 120 BPM quarter notes: 24 000 samples, ducked over the first 6 000.
        let mut exact = 0;
        for beat in 0..3 {
            for i in beat * 24_000 + 6_000..(beat + 1) * 24_000 {
                assert_eq!(o[i + lat].to_bits(), x[i].to_bits(), "{cross:?} at {i}");
                exact += 1;
            }
        }
        println!("{cross:?}: {exact} samples between the hits, every one the input's bits");
    }
}

/// The share of `o`'s energy above 4.5 kHz, dB.
fn splatter_db(o: &[f32]) -> f64 {
    let hi = HighPass::new(4_500.0, SR).run(o);
    db(rms(&hi[4_800..]) / rms(&o[4_800..]))
}

#[test]
fn the_shapes_edges_splatter_little() {
    let n = 96_000;
    let x = sine(1_000.0, 0.5, n, SR);
    let play = [(
        0usize,
        Event::Play {
            song: 0,
            bpm: 120.0,
        },
    )];
    let dry = splatter_db(&x);
    println!("a 1 kHz sine, energy above 4.5 kHz (the sine alone: {dry:.1} dB):");
    let mut by_edge = Vec::new();
    for edge in [0.0f32, 0.5, 1.0, 2.0, 5.0] {
        let mut row = String::new();
        for (name, shape) in [("square", &SQUARE[..]), ("classic pump", &CLASSIC_PUMP[..])] {
            let s = CedoSettings {
                trigger: Trigger::Tempo,
                depth_db: 24.0,
                division: 11,
                edge_ms: edge,
                ..CedoSettings::default()
            };
            let mut c = unit(&s, shape);
            let (o, _) = run(
                &mut c,
                &Run {
                    left: &x,
                    right: &x,
                    key: None,
                    events: &play,
                    block: 128,
                },
            );
            let sp = splatter_db(&o);
            row += &format!(" {name} {sp:.1} dB;");
            if name == "square" {
                by_edge.push(sp);
            }
        }
        println!("  edge {edge} ms:{row}");
    }
    // The sidechain's envelope with a 1 ms attack, for comparison.
    for shape in [
        ReleaseShape::Linear,
        ReleaseShape::Exponential,
        ReleaseShape::SCurve,
    ] {
        let key: Vec<f32> = (0..n)
            .map(|i| if i % 12_000 < 3_000 { 0.5 } else { 0.0 })
            .collect();
        let s = CedoSettings {
            trigger: Trigger::Sidechain,
            depth_db: 24.0,
            threshold_db: -20.0,
            attack_ms: 1.0,
            hold_ms: 0.0,
            release_ms: 40.0,
            release_shape: shape,
            ..CedoSettings::default()
        };
        let mut c = unit(&s, &[]);
        let (o, _) = run(
            &mut c,
            &Run {
                left: &x,
                right: &x,
                key: Some((&key, &key)),
                events: &[],
                block: 128,
            },
        );
        let sp = splatter_db(&o);
        println!("  sidechain, 1 ms attack, 40 ms {shape:?} release: {sp:.1} dB");
        // Measured −89.3 dB (the S attack; a straight one gave −78).
        assert!(sp < -87.0, "{shape:?}: {sp}");
    }
    // Measured: −53.9 (none), −77.2, −97.2, −115.3, −138.9 dB; the sine alone −155.
    assert!(by_edge[0] > -60.0, "a hard edge splatters: {}", by_edge[0]);
    assert!(by_edge[2] < -95.0, "1 ms of edge: {by_edge:?}");
    assert!(by_edge[3] < -112.0, "2 ms of edge: {by_edge:?}");
    assert!(by_edge.windows(2).all(|w| w[1] < w[0]), "{by_edge:?}");
}

#[test]
fn notes_fire_the_shape_on_their_sample() {
    let n = 60_000;
    let one = vec![1.0f32; n];
    for look in [0.0f32, 4.0] {
        let s = CedoSettings {
            trigger: Trigger::Midi,
            depth_db: DEPTH_INF,
            division: 11,
            edge_ms: 0.0,
            lookahead_ms: look,
            ..CedoSettings::default()
        };
        let mut c = unit(&s, &SQUARE);
        let lat = c.latency();
        let notes = [3_333usize, 20_000, 20_500, 41_111];
        let ev: Vec<(usize, Event)> = notes.iter().map(|&i| (i, Event::Note)).collect();
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
        let on: Vec<usize> = (1..n)
            .filter(|&i| o[i] == 0.0 && o[i - 1] == 1.0)
            .map(|i| i - lat)
            .collect();
        // 20 500 retriggers inside 20 000's ducked quarter (3 000 samples at 120 BPM, 1/8).
        assert_eq!(on, vec![3_333, 20_000, 41_111], "lookahead {look}");
        let len = 12_000 / 4;
        let off = (20_500 + len..n).find(|&i| o[i + lat] == 1.0).unwrap();
        assert_eq!(off, 20_500 + len);
    }
}

#[test]
fn a_trace_reads_the_reduction_on_each_sample() {
    let n = 2_000;
    let mut key = vec![0.0f32; n];
    key[100] = 1.0;
    let one = vec![1.0f32; n];
    let s = CedoSettings {
        trigger: Trigger::Sidechain,
        depth_db: 12.0,
        threshold_db: -6.0,
        attack_ms: 0.0,
        hold_ms: 5.0,
        release_ms: 5.0,
        release_shape: ReleaseShape::Linear,
        key_filter: KeyFilterMode::Off,
        ..CedoSettings::default()
    };
    let mut c = unit(&s, &[]);
    let t = gain_trace(
        &mut c,
        &Run {
            left: &one,
            right: &one,
            key: Some((&key, &key)),
            events: &[],
            block: 1,
        },
    );
    assert_eq!(t[99], 0.0);
    assert!((t[100] - 12.0).abs() < 1e-3, "{}", t[100]);
}

/// One core's share at 48 kHz: a minute of programme in blocks of 256, best of three. Run
/// alone, in release: `cargo test -p cantus-dsp --release --lib cedo_cpu -- --ignored --nocapture`.
#[test]
#[ignore]
fn cedo_cpu() {
    let n = 48_000 * 60;
    let x = super::testkit::pad(n, SR);
    let k = kick(n, &(0..120).map(|i| i * 24_000).collect::<Vec<_>>());
    let play = [(
        0usize,
        Event::Play {
            song: 0,
            bpm: 120.0,
        },
    )];
    let cases = [
        (
            "sidechain, whole",
            CedoSettings {
                trigger: Trigger::Sidechain,
                depth_db: 18.0,
                threshold_db: -30.0,
                ..CedoSettings::default()
            },
            true,
        ),
        (
            "sidechain, band-pass key, lows minimum phase, lookahead",
            CedoSettings {
                trigger: Trigger::Sidechain,
                depth_db: 18.0,
                threshold_db: -30.0,
                key_filter: KeyFilterMode::BandPass,
                key_hz: 60.0,
                band: Band::Lows,
                lookahead_ms: 5.0,
                link: 0.0,
                ..CedoSettings::default()
            },
            true,
        ),
        (
            "sidechain, lows linear phase",
            CedoSettings {
                trigger: Trigger::Sidechain,
                depth_db: 18.0,
                threshold_db: -30.0,
                band: Band::Lows,
                crossover: Crossover::LinearPhase,
                ..CedoSettings::default()
            },
            true,
        ),
        (
            "tempo, classic pump, edge 2 ms",
            CedoSettings {
                trigger: Trigger::Tempo,
                depth_db: 18.0,
                edge_ms: 2.0,
                ..CedoSettings::default()
            },
            false,
        ),
        (
            "tempo, edge 10 ms, lows linear phase",
            CedoSettings {
                trigger: Trigger::Tempo,
                depth_db: 18.0,
                edge_ms: 10.0,
                band: Band::Lows,
                crossover: Crossover::LinearPhase,
                ..CedoSettings::default()
            },
            false,
        ),
    ];
    for (name, s, keyed) in cases {
        let mut best = f64::MAX;
        for _ in 0..3 {
            let mut c = unit(&s, &CLASSIC_PUMP);
            let t = std::time::Instant::now();
            let _ = run(
                &mut c,
                &Run {
                    left: &x,
                    right: &x,
                    key: keyed.then_some((&k[..], &k[..])),
                    events: &play,
                    block: 256,
                },
            );
            best = best.min(t.elapsed().as_secs_f64());
        }
        println!(
            "{name}: {:.2} % of a core (best of 3, the driver's copies included)",
            100.0 * best / 60.0
        );
    }
}
