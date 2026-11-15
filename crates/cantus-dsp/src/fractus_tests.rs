//! Fractus measured (1): the null, every module on its own samples at three tempos, a
//! render from bar 5 equal to the whole one's, block-size independence, the read modules'
//! arithmetic to the bit, joins without clicks.

#![allow(clippy::needless_range_loop, clippy::type_complexity)]

use super::*;
use crate::math;
use crate::modfx_clock::random_bipolar;
use std::f64::consts::TAU;

pub(super) const SR: f32 = 48_000.0;

/// A busy stereo test signal by song sample: two tones each side and a noise hit every
/// 12 000 samples.
pub(super) fn music(i: i64) -> [f32; 2] {
    let t = i as f64 / f64::from(SR);
    let s = |f: f64| math::sin(TAU * f * t);
    let env = math::exp(-(i.rem_euclid(12_000) as f64) / 800.0);
    let l = 0.25 * s(220.0) + 0.15 * s(1_375.3) + 0.3 * env * f64::from(random_bipolar(7, i));
    let r = 0.25 * s(331.0) + 0.1 * s(2_750.1) + 0.3 * env * f64::from(random_bipolar(8, i));
    [l as f32, r as f32]
}

pub(super) fn sine(f: f64) -> impl Fn(i64) -> [f32; 2] {
    move |i| {
        let v = (0.5 * math::sin(TAU * f * i as f64 / f64::from(SR))) as f32;
        [v, v]
    }
}

/// `len` samples of `input` from song sample `from`, through `u` in blocks of `block`,
/// the transport playing at `tempo`.
pub(super) fn render(
    u: &mut Fractus,
    from: i64,
    len: usize,
    tempo: f64,
    block: usize,
    input: &dyn Fn(i64) -> [f32; 2],
) -> (Vec<f32>, Vec<f32>) {
    let mut l: Vec<f32> = (0..len).map(|i| input(from + i as i64)[0]).collect();
    let mut r: Vec<f32> = (0..len).map(|i| input(from + i as i64)[1]).collect();
    let mut at = 0;
    while at < len {
        let n = block.min(len - at);
        u.set_transport(&Transport {
            playing: true,
            tempo_bpm: tempo,
            samples: from + at as i64,
        });
        u.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (l, r)
}

pub(super) fn unit(s: &FractusSettings, p: &Pattern) -> Fractus {
    let mut u = Fractus::new(SR);
    u.set(s);
    u.set_pattern(p);
    u
}

/// Samples in a quarter note.
pub(super) fn spb(tempo: f64) -> f64 {
    60.0 / tempo * f64::from(SR)
}

/// The line of 1/16 step `k` from the song's start.
pub(super) fn line16(k: i64, tempo: f64) -> i64 {
    ((k as f64 * 0.25) * 60.0 / tempo * f64::from(SR)).round() as i64
}

/// A pattern that plays every module in one bar of sixteenths.
pub(super) fn busy() -> Pattern {
    let mut p = Pattern::EMPTY;
    p.block(Module::Repeat, 0, 2, 1.0, 1.0);
    p.block(Module::Gate, 0, 4, 0.8, 1.0);
    p.block(Module::Shuffle, 2, 2, 1.0, 1.0);
    p.block(Module::Reverse, 4, 1, 1.0, 1.0);
    p.block(Module::TapeStop, 6, 2, 1.0, 1.0);
    p.block(Module::Crush, 8, 2, 0.7, 1.0);
    p.block(Module::Filter, 10, 3, 1.0, 1.0);
    p.block(Module::Pitch, 12, 1, 1.0, 1.0);
    p.block(Module::Throw, 13, 1, 1.0, 1.0);
    p.block(Module::Cloud, 14, 2, 1.0, 1.0);
    p.block(Module::Repeat, 14, 2, 1.0, 0.6);
    p
}

/// Modules whose reads stay inside their own event.
fn reachless() -> Pattern {
    let mut p = Pattern::EMPTY;
    p.block(Module::Repeat, 0, 2, 1.0, 1.0);
    p.block(Module::Gate, 2, 2, 1.0, 1.0);
    p.block(Module::TapeStop, 6, 2, 1.0, 1.0);
    p.block(Module::Crush, 8, 2, 0.7, 1.0);
    p.block(Module::Filter, 10, 3, 1.0, 1.0);
    p.block(Module::Repeat, 13, 3, 1.0, 0.5);
    p.block(Module::Gate, 14, 1, 1.0, 1.0);
    p
}

pub(super) fn first_diff(a: &[f32], b: &[f32]) -> Option<usize> {
    a.iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
}

fn last_diff(a: &[f32], b: &[f32]) -> Option<usize> {
    a.iter()
        .zip(b)
        .rposition(|(x, y)| x.to_bits() != y.to_bits())
}

#[test]
fn at_the_defaults_it_is_a_wire_to_the_bit() {
    let input = &music;
    for s in [
        FractusSettings::default(),
        FractusSettings {
            mix: 0.4,
            swing: 0.7,
            bars: 4,
            step: STEP_BEATS[6],
            ..FractusSettings::default()
        },
    ] {
        let mut u = unit(&s, &Pattern::EMPTY);
        let (l, r) = render(&mut u, 0, 96_000, 120.0, 512, input);
        for (i, (a, b)) in l.iter().zip(&r).enumerate() {
            let x = music(i as i64);
            assert_eq!(
                (a.to_bits(), b.to_bits()),
                (x[0].to_bits(), x[1].to_bits()),
                "at {i}"
            );
        }
        assert_eq!(u.latency(), 0);
    }
    // The full path, sample by sample, is the same wire.
    let mut u = unit(&FractusSettings::default(), &Pattern::EMPTY);
    let mut l: Vec<f32> = (0..20_000).map(|i| music(i)[0]).collect();
    let mut r: Vec<f32> = (0..20_000).map(|i| music(i)[1]).collect();
    u.set_transport(&Transport {
        playing: true,
        tempo_bpm: 120.0,
        samples: 0,
    });
    u.process_by_ticks(&mut l, &mut r);
    for i in 0..20_000 {
        let x = music(i as i64);
        assert_eq!(l[i].to_bits(), x[0].to_bits());
        assert_eq!(r[i].to_bits(), x[1].to_bits());
    }
}

#[test]
fn each_module_sounds_only_inside_its_steps_at_three_tempos() {
    let input = &music;
    for tempo in [60.0, 120.0, 174.0] {
        for m in Module::ALL {
            if m == Module::Throw || m == Module::Cloud {
                continue; // their echoes and grains outlive the step by design
            }
            for swing in [0.0f32, 0.6] {
                let mut p = Pattern::EMPTY;
                p.block(m, 5, 2, 1.0, 1.0);
                let s = FractusSettings {
                    swing,
                    gate: GateSettings {
                        rate: 0.0,
                        ..GateSettings::default()
                    },
                    ..FractusSettings::default()
                };
                let mut u = unit(&s, &p);
                let len = (4.0 * spb(tempo)) as usize + 1000;
                let (l, _) = render(&mut u, 0, len, tempo, 256, input);
                let dry: Vec<f32> = (0..len).map(|i| music(i as i64)[0]).collect();
                let at = |k: i64| {
                    let sw = if k % 2 == 1 {
                        f64::from(swing) * 0.5 * 0.25
                    } else {
                        0.0
                    };
                    ((k as f64 * 0.25 + sw) * 60.0 / tempo * f64::from(SR)).round() as usize
                };
                let (a, b) = (at(5), at(7));
                let f = first_diff(&l, &dry).expect("it sounds");
                let e = last_diff(&l, &dry).unwrap();
                assert!(f >= a, "{m:?} at {tempo} BPM starts at {f}, before {a}");
                assert!(e < b, "{m:?} at {tempo} BPM ends at {e}, past {b}");
                if matches!(
                    m,
                    Module::Gate | Module::Crush | Module::Filter | Module::Pitch
                ) {
                    assert_eq!(f, a, "{m:?} at {tempo} BPM begins on its sample");
                    assert_eq!(e, b - 1, "{m:?} at {tempo} BPM is gone by the next line");
                }
            }
        }
    }
}

#[test]
fn a_render_from_bar_5_is_the_whole_render_from_there() {
    for tempo in [60.0, 120.0, 174.0] {
        let bar = (4.0 * spb(tempo)).round() as i64;
        let from = (16.0 * 60.0 / tempo * f64::from(SR)).round() as i64;
        let len = (7 * bar) as usize;
        let s = FractusSettings::default();
        let mut whole = unit(&s, &reachless());
        let (wl, wr) = render(&mut whole, 0, len, tempo, 512, &music);
        let mut late = unit(&s, &reachless());
        let (ll, lr) = render(&mut late, from, len - from as usize, tempo, 333, &music);
        assert_eq!(late.reach(tempo), 0);
        assert_eq!(
            first_diff(&wl[from as usize..], &ll),
            None,
            "left at {tempo}"
        );
        assert_eq!(
            first_diff(&wr[from as usize..], &lr),
            None,
            "right at {tempo}"
        );
        // The modules that read before their event, and the cloud: equal once the reads
        // reach only the late render's own audio.
        let s = FractusSettings {
            cloud: CloudSettings {
                level: 0.3,
                ..CloudSettings::default()
            },
            ..FractusSettings::default()
        };
        let mut p = busy();
        p.lanes[Module::Throw.lane()] = [Step::OFF; MAX_STEPS];
        let mut whole = unit(&s, &p);
        let (wl, _) = render(&mut whole, 0, len, tempo, 512, &music);
        let mut late = unit(&s, &p);
        let reach = late.reach(tempo);
        assert!(reach > 0 && (reach as i64) < bar);
        let (ll, _) = render(&mut late, from, len - from as usize, tempo, 333, &music);
        let d = first_diff(&wl[from as usize + reach..], &ll[reach..]);
        assert_eq!(d, None, "busy at {tempo} BPM after {reach} samples");
        assert_ne!(first_diff(&wl[from as usize..], &ll), None);
    }
}

#[test]
fn every_block_size_gives_the_same_bits() {
    let s = FractusSettings {
        swing: 0.3,
        cloud: CloudSettings {
            level: 0.4,
            density_hz: 60.0,
            pitch_rand_st: 12.0,
            reverse: 0.3,
            ..CloudSettings::default()
        },
        repeat: RepeatSettings {
            ramp: 2,
            pitch_st: 2.0,
            decay_db: -2.0,
            duty: 0.7,
            ..RepeatSettings::default()
        },
        shuffle: ShuffleSettings {
            reverse: 0.4,
            ..ShuffleSettings::default()
        },
        ..FractusSettings::default()
    };
    let len = 200_000;
    let mut a = unit(&s, &busy());
    let want = render(&mut a, 0, len, 128.0, 512, &music);
    for block in [1usize, 37, 64] {
        let mut b = unit(&s, &busy());
        let got = render(&mut b, 0, len, 128.0, block, &music);
        assert_eq!(first_diff(&want.0, &got.0), None, "left, block {block}");
        assert_eq!(first_diff(&want.1, &got.1), None, "right, block {block}");
    }
}

#[test]
fn a_repeat_replays_its_slice_to_the_bit_and_its_lines_round_once() {
    for tempo in [120.0, 174.0] {
        let mut p = Pattern::EMPTY;
        p.block(Module::Repeat, 0, 16, 1.0, 1.0);
        let s = FractusSettings {
            repeat: RepeatSettings {
                size: 0.25,
                decay_db: -3.0,
                ..RepeatSettings::default()
            },
            ..FractusSettings::default()
        };
        let mut u = unit(&s, &p);
        let bar = (4.0 * spb(tempo)).round() as usize;
        let (l, _) = render(&mut u, 0, bar, tempo, 512, &music);
        let r = 0.25 * spb(tempo);
        let xf = (0.002 * f64::from(SR)).round() as usize;
        let mut worst = 0.0f64;
        for i in 1..15i64 {
            let b0 = (i as f64 * r).round() as usize;
            let b1 = ((i + 1) as f64 * r).round() as usize;
            worst = worst.max((b0 as f64 - i as f64 * r).abs());
            let g = math::db_to_gain(-3.0 * i as f32);
            for j in xf + 1..(b1 - b0) {
                let want = music(j as i64)[0] * g;
                assert_eq!(l[b0 + j].to_bits(), want.to_bits(), "repeat {i}, {j}");
            }
        }
        assert!(worst <= 0.5, "a repeat's line is the nearest sample");
    }
}

#[test]
fn a_reverse_plays_the_slice_before_it_backwards() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Reverse, 4, 4, 1.0, 1.0);
    let s = FractusSettings {
        reverse: ReverseSettings { slice: 0.5 },
        ..FractusSettings::default()
    };
    let mut u = unit(&s, &p);
    let (l, _) = render(&mut u, 0, 96_000, 120.0, 100, &music);
    let ls = 12_000usize; // 1/8 at 120
    let xf = 96;
    for j in 0..2 {
        let s0 = 24_000 + j * ls;
        for tau in xf + 1..ls - xf - 1 {
            let want = music((s0 - tau) as i64)[0];
            assert_eq!(l[s0 + tau].to_bits(), want.to_bits(), "slice {j}, {tau}");
        }
    }
}

#[test]
fn a_shuffle_plays_whole_slices_from_up_to_its_range_back() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Shuffle, 0, 16, 1.0, 1.0);
    let s = FractusSettings {
        shuffle: ShuffleSettings {
            slice: 0.125,
            range: 3,
            reverse: 0.0,
        },
        ..FractusSettings::default()
    };
    let mut u = unit(&s, &p);
    let (l, _) = render(&mut u, 0, 4 * 96_000, 120.0, 512, &music);
    let ls = 3_000usize;
    let mut seen = [0usize; 4];
    for j in 0..(4 * 96_000 / ls) {
        let s0 = j * ls;
        let k = (0..=3usize)
            .find(|&k| {
                (200..ls - 200).all(|t| {
                    let want = if s0 + t < k * ls {
                        0.0
                    } else {
                        music((s0 + t - k * ls) as i64)[0]
                    };
                    l[s0 + t].to_bits() == want.to_bits()
                })
            })
            .unwrap_or_else(|| panic!("slice {j} is no slice of the last four"));
        seen[k] += 1;
    }
    assert!(
        seen.iter().all(|&c| c > 10),
        "every distance is rolled: {seen:?}"
    );
}

#[test]
fn a_tape_stop_falls_silent_by_its_line_and_the_next_bar_is_dry() {
    for tempo in [60.0, 120.0, 174.0] {
        let mut p = Pattern::EMPTY;
        p.block(Module::TapeStop, 12, 4, 1.0, 1.0);
        let mut u = unit(&FractusSettings::default(), &p);
        let bar = (4.0 * spb(tempo)).round() as usize;
        let (l, _) = render(&mut u, 0, 2 * bar, tempo, 256, &sine(220.0));
        let a = line16(12, tempo) as usize;
        let b = line16(16, tempo) as usize;
        let xf = 96;
        let tail = l[b - xf - 600..b - xf]
            .iter()
            .fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(tail < 0.02, "{tempo} BPM: {tail}");
        let head = l[a..a + 200].iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(head > 0.3);
        for i in b..bar + a {
            assert_eq!(
                l[i].to_bits(),
                sine(220.0)(i as i64)[0].to_bits(),
                "{tempo}: {i}"
            );
        }
    }
}

#[test]
fn a_gate_opens_and_shuts_on_its_pulses() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Gate, 0, 16, 1.0, 1.0);
    let s = FractusSettings {
        gate: GateSettings {
            rate: 0.25,
            duty: 0.5,
            depth: 1.0,
        },
        ..FractusSettings::default()
    };
    let mut u = unit(&s, &p);
    let one = |_: i64| [0.5f32, 0.5];
    let (l, _) = render(&mut u, 0, 96_000, 120.0, 128, &one);
    for i in 1..15usize {
        let p0 = i * 6_000;
        assert_eq!(l[p0 + 200], 0.5, "pulse {i} open");
        assert_eq!(l[p0 + 2_999], 0.5, "open to the half");
        assert_eq!(l[p0 + 3_000 + 200], 0.0, "shut after it");
        assert!(
            l[p0 + 3_000] < 0.5 && l[p0 + 3_000] > 0.49,
            "shutting from its sample"
        );
    }
}

/// The peak of what lies above `hz` (a 4-pole Butterworth high-pass), dB against
/// `ref_peak`.
pub(super) fn splatter_db(x: &[f32], hz: f64, ref_peak: f32) -> f64 {
    let k = math::tan(std::f64::consts::PI * hz / f64::from(SR));
    let mut y: Vec<f64> = x.iter().map(|v| f64::from(*v)).collect();
    for q in [0.541_196_1, 1.306_563] {
        let norm = 1.0 / (1.0 + k / q + k * k);
        let (b0, b1, b2) = (norm, -2.0 * norm, norm);
        let (a1, a2) = (2.0 * (k * k - 1.0) * norm, (1.0 - k / q + k * k) * norm);
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        for v in y.iter_mut() {
            let o = b0 * *v + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = *v;
            y2 = y1;
            y1 = o;
            *v = o;
        }
    }
    let peak = y[2_000..].iter().fold(0.0f64, |m, v| m.max(v.abs()));
    20.0 * math::log10(peak.max(1e-12) / f64::from(ref_peak))
}

#[test]
fn joins_splatter_far_less_than_hard_cuts() {
    // A 233 Hz sine through a 1/32 repeat and a 1/16 reverse, beside the same splices
    // made hard; and a shuffle. Above 4 kHz the sine itself has nothing.
    let tone = sine(233.0);
    let len = 96_000usize;
    let dry: Vec<f32> = (0..len).map(|i| tone(i as i64)[0]).collect();
    // From the fifth sixteenth (24 000), so every read reaches back into the tone.
    let cases: [(&str, Module, fn(usize) -> usize); 3] = [
        ("repeat 1/32", Module::Repeat, |t| {
            if t < 24_000 {
                t
            } else {
                24_000 + (t - 24_000) % 3_000
            }
        }),
        ("reverse 1/16", Module::Reverse, |t| {
            let (j, tau) = (t / 6_000, t % 6_000);
            if t < 24_000 {
                t
            } else {
                j * 6_000 - tau
            }
        }),
        ("shuffle 1/16", Module::Shuffle, |t| t),
    ];
    for (name, m, hard_src) in cases {
        let mut p = Pattern::EMPTY;
        p.block(m, 4, 12, 1.0, 1.0);
        let s = FractusSettings {
            repeat: RepeatSettings {
                size: 0.125,
                ..RepeatSettings::default()
            },
            ..FractusSettings::default()
        };
        let mut u = unit(&s, &p);
        let (l, _) = render(&mut u, 0, len, 120.0, 512, &tone);
        let hard: Vec<f32> = (0..len).map(|t| dry[hard_src(t)]).collect();
        let ours = splatter_db(&l[..90_000], 4_000.0, 0.5);
        let cut = splatter_db(&hard[..90_000], 4_000.0, 0.5);
        eprintln!("{name}: joined {ours:.1} dB, hard {cut:.1} dB above 4 kHz");
        if m != Module::Shuffle {
            assert!(cut > -40.0, "{name}: the hard cuts do splatter ({cut:.1})");
            assert!(ours < cut - 25.0, "{name}: {ours:.1} against {cut:.1}");
        }
        assert!(ours < -55.0, "{name}: {ours:.1} dB");
    }
}
