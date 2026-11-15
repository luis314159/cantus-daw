//! Cedo's contract: the null to the bit (delayed by exactly the latency stated), block-size
//! independence at tolerance 0 (the tempo trigger across block edges, notes inside blocks,
//! settings and seeks at fixed samples), a render from bar N equal to the whole render,
//! and exact rest with its twins.

use super::band::{Band, Crossover};
use super::env::{KeyFilterMode, ReleaseShape};
use super::testkit::{noise, pad, run, shape_fn, sine, Event, Run};
use super::{latency_for, Cedo, CedoSettings, Listen, Trigger, CLASSIC_PUMP, DEPTH_INF};
use crate::fluctus::CurvePoint;

const SR: f32 = 48_000.0;

fn unit(s: &CedoSettings) -> Cedo {
    let mut c = Cedo::new(SR, shape_fn);
    c.set_shape(&CLASSIC_PUMP);
    c.set(s);
    c.reset();
    c
}

/// Music on both channels and a key of kick-like bursts on the beat at 120 BPM.
fn material(n: usize) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let p = pad(n, SR);
    let nz = noise(n, 3);
    let l: Vec<f32> = p.iter().zip(&nz).map(|(a, b)| a + 0.02 * b).collect();
    let r: Vec<f32> = p.iter().zip(&nz).map(|(a, b)| 0.8 * a - 0.02 * b).collect();
    let k: Vec<f32> = (0..n)
        .map(|i| {
            let ph = i % 24_000;
            if ph < 6_000 {
                let t = ph as f32 / SR;
                0.9 * crate::math::exp(-t * 30.0)
                    * crate::math::sin(std::f32::consts::TAU * 55.0 * t)
            } else {
                0.0
            }
        })
        .collect();
    (l, r, k)
}

fn bits(x: &[f32]) -> Vec<u32> {
    x.iter().map(|v| v.to_bits()).collect()
}

#[test]
fn no_depth_no_mix_or_no_trigger_is_a_null_to_the_bit_delayed_by_the_latency() {
    let n = 60_000;
    let (l, r, k) = material(n);
    let silent = vec![0.0f32; n];
    let sc = CedoSettings {
        trigger: Trigger::Sidechain,
        depth_db: 18.0,
        threshold_db: -30.0,
        key_filter: KeyFilterMode::BandPass,
        ..CedoSettings::default()
    };
    let tempo = CedoSettings {
        trigger: Trigger::Tempo,
        depth_db: 18.0,
        ..CedoSettings::default()
    };
    let midi = CedoSettings {
        trigger: Trigger::Midi,
        depth_db: 18.0,
        ..CedoSettings::default()
    };
    let play = [(
        0usize,
        Event::Play {
            song: 0,
            bpm: 120.0,
        },
    )];
    let notes = [(1_000usize, Event::Note), (30_000, Event::Note)];
    // (settings, key, events)
    type Case<'a> = (CedoSettings, Option<&'a [f32]>, &'a [(usize, Event)]);
    let mut cases: Vec<Case> = vec![
        (CedoSettings::default(), Some(&k), &play),
        (sc, None, &play),
        (sc, Some(&silent), &play),
        (
            CedoSettings {
                depth_db: 0.0,
                ..sc
            },
            Some(&k),
            &play,
        ),
        (CedoSettings { mix: 0.0, ..sc }, Some(&k), &play),
        (tempo, None, &[]),
        (
            CedoSettings {
                depth_db: 0.0,
                ..tempo
            },
            None,
            &play,
        ),
        (CedoSettings { mix: 0.0, ..tempo }, None, &play),
        (midi, None, &[]),
        (
            CedoSettings {
                depth_db: 0.0,
                ..midi
            },
            None,
            &notes,
        ),
    ];
    // Every case again with the lookahead and each band split.
    let base = cases.clone();
    for (look, band, cross) in [
        (5.0, Band::Whole, Crossover::MinimumPhase),
        (0.0, Band::Lows, Crossover::MinimumPhase),
        (2.0, Band::Highs, Crossover::LinearPhase),
        (0.0, Band::Lows, Crossover::LinearPhase),
    ] {
        for (s, key, ev) in &base {
            let s = CedoSettings {
                lookahead_ms: look,
                band,
                crossover: cross,
                ..*s
            };
            cases.push((s, *key, ev));
        }
    }
    for (s, key, ev) in &cases {
        let mut c = unit(s);
        let lat = latency_for(s, SR);
        assert_eq!(c.latency(), lat);
        for block in [64, 37] {
            c.reset();
            let (ol, or) = run(
                &mut c,
                &Run {
                    left: &l,
                    right: &r,
                    key: key.map(|k| (k, k)),
                    events: ev,
                    block,
                },
            );
            assert!(
                ol[..lat].iter().chain(&or[..lat]).all(|v| v.to_bits() == 0),
                "{s:?}"
            );
            assert_eq!(bits(&ol[lat..]), bits(&l[..n - lat]), "{s:?}");
            assert_eq!(bits(&or[lat..]), bits(&r[..n - lat]), "{s:?}");
        }
    }
}

/// Settings, their events, and whether a key is routed.
type Moving = (CedoSettings, Vec<(usize, Event)>, bool);

/// The settings the block-size test runs, each with its events.
fn moving_cases(n: usize) -> Vec<Moving> {
    let sc = CedoSettings {
        trigger: Trigger::Sidechain,
        depth_db: 24.0,
        threshold_db: -20.0,
        attack_ms: 3.0,
        hold_ms: 25.0,
        release_ms: 90.0,
        release_shape: ReleaseShape::SCurve,
        key_filter: KeyFilterMode::HighPass,
        key_hz: 40.0,
        lookahead_ms: 4.0,
        link: 0.5,
        band: Band::Lows,
        crossover: Crossover::LinearPhase,
        cross_hz: 150.0,
        ..CedoSettings::default()
    };
    let tempo = CedoSettings {
        trigger: Trigger::Tempo,
        depth_db: DEPTH_INF,
        division: 11,
        swing: 0.6,
        edge_ms: 3.0,
        band: Band::Lows,
        crossover: Crossover::MinimumPhase,
        cross_hz: 200.0,
        mix: 0.8,
        out_db: -1.5,
        ..CedoSettings::default()
    };
    let midi = CedoSettings {
        trigger: Trigger::Midi,
        depth_db: 30.0,
        division: 8,
        edge_ms: 6.0,
        lookahead_ms: 1.0,
        ..CedoSettings::default()
    };
    let play = |bpm| (0usize, Event::Play { song: 3_000, bpm });
    vec![
        (
            sc,
            vec![
                play(120.0),
                (
                    n / 3,
                    Event::Set(CedoSettings {
                        cross_hz: 90.0,
                        link: 1.0,
                        ..sc
                    }),
                ),
                (
                    n / 2,
                    Event::Set(CedoSettings {
                        crossover: Crossover::MinimumPhase,
                        band: Band::Highs,
                        ..sc
                    }),
                ),
                (
                    2 * n / 3,
                    Event::Set(CedoSettings {
                        listen: Listen::Delta,
                        release_shape: ReleaseShape::Linear,
                        ..sc
                    }),
                ),
            ],
            true,
        ),
        (
            tempo,
            vec![
                play(174.0),
                (
                    n / 4,
                    Event::Set(CedoSettings {
                        division: 8,
                        edge_ms: 0.0,
                        ..tempo
                    }),
                ),
                (
                    n / 2,
                    Event::Play {
                        song: 96_000,
                        bpm: 174.0,
                    },
                ),
                (3 * n / 5, Event::Stop),
                (2 * n / 3, Event::Play { song: 0, bpm: 60.0 }),
            ],
            false,
        ),
        (
            midi,
            vec![
                (777, Event::Note),
                (12_345, Event::Note),
                (12_400, Event::Note),
                (
                    n / 2,
                    Event::Set(CedoSettings {
                        lookahead_ms: 8.0,
                        ..midi
                    }),
                ),
                (n / 2 + 999, Event::Note),
                (n - 9_000, Event::Note),
            ],
            false,
        ),
    ]
}

#[test]
fn the_output_does_not_depend_on_the_block_size() {
    let n = 90_000;
    let (l, r, k) = material(n);
    for (s, ev, keyed) in moving_cases(n) {
        let mut outs = Vec::new();
        for block in [1, 37, 64, 512] {
            let mut c = unit(&s);
            outs.push(run(
                &mut c,
                &Run {
                    left: &l,
                    right: &r,
                    key: keyed.then_some((&k[..], &k[..])),
                    events: &ev,
                    block,
                },
            ));
        }
        let moved = outs[0]
            .0
            .iter()
            .zip(&l)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        assert!(moved > n / 10, "{s:?}: it ducked ({moved})");
        for o in &outs[1..] {
            assert_eq!(bits(&o.0), bits(&outs[0].0), "{s:?}");
            assert_eq!(bits(&o.1), bits(&outs[0].1), "{s:?}");
        }
    }
}

#[test]
fn a_render_from_bar_n_equals_the_whole_render() {
    for (bpm, swing, band, cross) in [
        (120.0, 0.0, Band::Whole, Crossover::MinimumPhase),
        (174.0, 0.6, Band::Whole, Crossover::MinimumPhase),
        (60.0, 1.0, Band::Lows, Crossover::MinimumPhase),
        (128.0, 0.3, Band::Highs, Crossover::LinearPhase),
    ] {
        let s = CedoSettings {
            trigger: Trigger::Tempo,
            depth_db: 30.0,
            division: 11,
            swing,
            edge_ms: 2.0,
            lookahead_ms: 3.0,
            band,
            crossover: cross,
            ..CedoSettings::default()
        };
        let bar = (4.0 * 60.0 / bpm * SR as f64).round() as usize;
        let from = 4 * bar; // bar 5
        let n = from + 3 * bar;
        let (mut l, mut r, _) = material(n);
        if band != Band::Whole {
            // A filter remembers what it heard: silence before bar 5 lets it forget.
            for v in l[from - SR as usize..from]
                .iter_mut()
                .chain(&mut r[from - SR as usize..from])
            {
                *v = 0.0;
            }
        }
        let mut whole = unit(&s);
        let (wl, wr) = run(
            &mut whole,
            &Run {
                left: &l,
                right: &r,
                key: None,
                events: &[(0, Event::Play { song: 0, bpm })],
                block: 256,
            },
        );
        let mut part = unit(&s);
        let (pl, pr) = run(
            &mut part,
            &Run {
                left: &l[from..],
                right: &r[from..],
                key: None,
                events: &[(
                    0,
                    Event::Play {
                        song: from as i64,
                        bpm,
                    },
                )],
                block: 100,
            },
        );
        let lat = part.latency();
        assert!(pl.iter().zip(&l[from..]).any(|(a, b)| a != b));
        assert_eq!(
            bits(&pl[lat..]),
            bits(&wl[from + lat..]),
            "{bpm} {swing} {band:?}"
        );
        assert_eq!(
            bits(&pr[lat..]),
            bits(&wr[from + lat..]),
            "{bpm} {swing} {band:?}"
        );
    }
}

/// Plays music, then silence until rest; returns the unit and how long rest took.
fn to_rest(s: &CedoSettings, ev: &[(usize, Event)], keyed: bool) -> (Cedo, usize) {
    let n = 40_000;
    let (mut l, mut r, mut k) = material(n + 400_000);
    for v in l[n..].iter_mut().chain(&mut r[n..]).chain(&mut k[n..]) {
        *v = 0.0;
    }
    let mut c = unit(s);
    run(
        &mut c,
        &Run {
            left: &l[..n],
            right: &r[..n],
            key: keyed.then_some((&k[..n], &k[..n])),
            events: ev,
            block: 128,
        },
    );
    assert!(!c.at_rest());
    let z = vec![0.0f32; 64];
    let mut waited = 0;
    while !c.at_rest() {
        let (mut a, mut b) = (z.clone(), z.clone());
        c.process(&mut a, &mut b, keyed.then_some((&z[..], &z[..])));
        assert!(a.iter().chain(&b).all(|v| v.is_finite()));
        waited += 64;
        assert!(waited < 400_000, "{s:?}: {:?}", c.rest_blocker());
    }
    (c, waited)
}

#[test]
fn it_comes_to_rest_exactly_and_its_twins_agree() {
    let mut cases = Vec::new();
    for (s, _, keyed) in moving_cases(90_000) {
        cases.push((s, keyed));
    }
    cases.push((
        CedoSettings {
            trigger: Trigger::Sidechain,
            depth_db: 40.0,
            threshold_db: -40.0,
            key_filter: KeyFilterMode::BandPass,
            key_hz: 60.0,
            key_q: 4.0,
            band: Band::Lows,
            ..CedoSettings::default()
        },
        true,
    ));
    for (s, keyed) in cases {
        let notes = [(500usize, Event::Note), (20_000, Event::Note)];
        let ev: &[(usize, Event)] = if s.trigger == Trigger::Midi {
            &notes
        } else {
            &[]
        };
        let (rested, waited) = to_rest(&s, ev, keyed);
        println!(
            "{:?}/{:?}: at rest after {:.3} s of silence",
            s.trigger,
            s.band,
            waited as f32 / SR
        );
        // Twins: zeros processed, the same length skipped, reset, and new.
        let (mut proc_, mut skip, mut reset) = (rested.clone(), rested.clone(), rested.clone());
        let gap = 10_007;
        let (mut a, mut b) = (vec![0.0f32; gap], vec![0.0f32; gap]);
        let silent = vec![0.0f32; gap];
        proc_.process(&mut a, &mut b, keyed.then_some((&silent[..], &silent[..])));
        assert!(a.iter().chain(&b).all(|v| v.to_bits() == 0));
        assert!(proc_.at_rest());
        skip.skip(gap);
        reset.reset();
        let mut new = unit(&s);
        let (l, r, k) = material(30_000);
        let mut outs = Vec::new();
        for u in [&mut proc_, &mut skip, &mut reset, &mut new] {
            outs.push(run(
                u,
                &Run {
                    left: &l,
                    right: &r,
                    key: keyed.then_some((&k[..], &k[..])),
                    events: if s.trigger == Trigger::Midi {
                        &notes
                    } else {
                        &[]
                    },
                    block: 128,
                },
            ));
        }
        // The tempo trigger is stopped here (the transport only played in its run), so
        // the twins agree wholly; skipped and processed agree in every case.
        assert_eq!(bits(&outs[1].0), bits(&outs[0].0), "{s:?} skip");
        assert_eq!(bits(&outs[2].0), bits(&outs[3].0), "{s:?} reset");
        assert_eq!(bits(&outs[0].0), bits(&outs[3].0), "{s:?} new");
        assert_eq!(bits(&outs[0].1), bits(&outs[3].1), "{s:?} new");
    }
}

#[test]
fn the_tempo_pump_rests_while_playing_and_a_skip_keeps_its_place() {
    let s = CedoSettings {
        trigger: Trigger::Tempo,
        depth_db: 24.0,
        division: 11,
        swing: 0.4,
        edge_ms: 2.0,
        ..CedoSettings::default()
    };
    let bpm = 132.0;
    let n = 200_000;
    let (mut l, mut r, _) = material(n);
    for v in l[30_000..120_000].iter_mut().chain(&mut r[30_000..120_000]) {
        *v = 0.0;
    }
    let ev = [(0usize, Event::Play { song: 0, bpm })];
    let mut whole = unit(&s);
    let (wl, _) = run(
        &mut whole,
        &Run {
            left: &l,
            right: &r,
            key: None,
            events: &ev,
            block: 64,
        },
    );
    // The same, the silence skipped from the first block at rest.
    let mut c = unit(&s);
    let mut out = vec![0.0f32; n];
    let mut i = 0;
    let mut skipped = 0;
    while i < n {
        let end = (i + 64).min(n);
        let silent = l[i..end].iter().chain(&r[i..end]).all(|v| v.to_bits() == 0);
        c.set_transport(&crate::modfx_clock::Transport {
            playing: true,
            tempo_bpm: bpm,
            samples: i as i64,
        });
        if silent && c.at_rest() {
            c.skip(end - i);
            skipped += end - i;
        } else {
            let (mut a, mut b) = (l[i..end].to_vec(), r[i..end].to_vec());
            c.process(&mut a, &mut b, None);
            out[i..end].copy_from_slice(&a);
        }
        i = end;
    }
    assert!(skipped > 60_000, "skipped {skipped}");
    assert_eq!(bits(&out), bits(&wl));
}

#[test]
fn a_new_unit_is_at_rest_and_silence_keeps_it_there() {
    for t in [Trigger::Sidechain, Trigger::Tempo, Trigger::Midi] {
        let mut c = unit(&CedoSettings {
            trigger: t,
            depth_db: 20.0,
            band: Band::Lows,
            crossover: Crossover::LinearPhase,
            ..CedoSettings::default()
        });
        assert!(c.at_rest(), "{t:?}: {:?}", c.rest_blocker());
        let (mut a, mut b) = (vec![0.0f32; 5_000], vec![0.0f32; 5_000]);
        let z = vec![0.0f32; 5_000];
        c.process(&mut a, &mut b, Some((&z, &z)));
        assert!(c.at_rest());
        assert!(a.iter().chain(&b).all(|v| v.to_bits() == 0));
    }
}

#[test]
fn a_shape_with_a_flat_zero_ducks_nothing_and_a_full_one_ducks_to_the_floor() {
    let n = 48_000;
    let one = vec![1.0f32; n];
    let mut c = unit(&CedoSettings {
        trigger: Trigger::Tempo,
        depth_db: DEPTH_INF,
        ..CedoSettings::default()
    });
    let ev = [(
        0usize,
        Event::Play {
            song: 0,
            bpm: 120.0,
        },
    )];
    c.set_shape(&[]);
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
    assert!(o.iter().all(|v| *v == 1.0));
    c.set_shape(&[CurvePoint {
        x: 0.0,
        y: 1.0,
        tension: 0.0,
        shape: 0,
    }]);
    c.reset();
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
    assert!(o.iter().all(|v| v.to_bits() == 0), "{:?}", &o[..4]);
    let _ = sine(1.0, 1.0, 1, SR);
}
