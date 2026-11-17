//! Fractus measured (2): notes at their samples, the rolls of the song, rest and skip, the
//! cloud (seeded, frozen, pitched, its aliasing), the throw's echoes, the telemetry.

#![allow(clippy::needless_range_loop, clippy::type_complexity)]

use super::tests::{first_diff, line16, music, render, sine, unit, SR};
use super::*;
use crate::math;

/// `render`, with notes (song sample, note) handed to each block at their offsets.
fn render_notes(
    u: &mut Fractus,
    len: usize,
    tempo: f64,
    block: usize,
    input: &dyn Fn(i64) -> [f32; 2],
    notes: &[(i64, Note)],
) -> Vec<f32> {
    let mut l: Vec<f32> = (0..len).map(|i| input(i as i64)[0]).collect();
    let mut r: Vec<f32> = (0..len).map(|i| input(i as i64)[1]).collect();
    let mut at = 0;
    while at < len {
        let n = block.min(len - at);
        u.set_transport(&Transport {
            playing: true,
            tempo_bpm: tempo,
            samples: at as i64,
        });
        for &(s, note) in notes {
            if s >= at as i64 && s < (at + n) as i64 {
                u.note(s as usize - at, note);
            }
        }
        u.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    l
}

fn shut_gate(trigger: Trigger, quantise: bool) -> FractusSettings {
    FractusSettings {
        trigger,
        quantise,
        gate: GateSettings {
            rate: 0.0,
            ..GateSettings::default()
        },
        ..FractusSettings::default()
    }
}

const GATE_KEY: u8 = 48 + Module::Gate as u8;

#[test]
fn a_note_plays_its_module_from_its_sample_at_every_block_size() {
    let s = shut_gate(Trigger::Notes, false);
    let notes = [
        (1_234, Note::On(GATE_KEY, 100)),
        (20_000, Note::Off(GATE_KEY)),
    ];
    let dry: Vec<f32> = (0..40_000).map(|i| music(i)[0]).collect();
    let mut want = None;
    for block in [1usize, 37, 64, 512] {
        let mut u = unit(&s, &Pattern::EMPTY);
        let l = render_notes(&mut u, 40_000, 120.0, block, &music, &notes);
        assert_eq!(first_diff(&l, &dry), Some(1_234), "block {block}");
        let last = l
            .iter()
            .zip(&dry)
            .rposition(|(a, b)| a.to_bits() != b.to_bits());
        assert_eq!(last, Some(20_000 + 96 - 1), "released over the join");
        // The velocity is the amount: 100/127 of the way to silence.
        let g = l[10_000] / dry[10_000];
        assert!((g - (1.0 - 100.0 / 127.0)).abs() < 1e-4, "{g}");
        match &want {
            None => want = Some(l),
            Some(w) => assert_eq!(first_diff(w, &l), None, "block {block}"),
        }
    }
}

#[test]
fn a_quantised_note_waits_for_the_next_line() {
    let s = shut_gate(Trigger::Both, true);
    let notes = [
        (1_234, Note::On(GATE_KEY, 127)),
        (20_000, Note::Off(GATE_KEY)),
    ];
    let mut u = unit(&s, &Pattern::EMPTY);
    let l = render_notes(&mut u, 40_000, 120.0, 256, &music, &notes);
    let dry: Vec<f32> = (0..40_000).map(|i| music(i)[0]).collect();
    assert_eq!(first_diff(&l, &dry), Some(line16(1, 120.0) as usize));
    let last = l
        .iter()
        .zip(&dry)
        .rposition(|(a, b)| a.to_bits() != b.to_bits());
    assert_eq!(last, Some(line16(4, 120.0) as usize - 1));
}

#[test]
fn a_held_repeat_repeats_its_first_slice_for_as_long_as_it_is_held() {
    let s = FractusSettings {
        trigger: Trigger::Notes,
        quantise: false,
        repeat: RepeatSettings {
            size: 0.25,
            ..RepeatSettings::default()
        },
        ..FractusSettings::default()
    };
    let len = 12 * 48_000;
    let notes = [(10_000, Note::On(48, 127))];
    let mut u = unit(&s, &Pattern::EMPTY);
    let l = render_notes(&mut u, len, 120.0, 512, &music, &notes);
    // Eleven seconds on (longer than the history), repeat 87 still plays the first slice.
    let at = 10_000 + 87 * 6_000;
    for j in 200..5_800 {
        assert_eq!(
            l[at + j].to_bits(),
            music(10_000 + j as i64)[0].to_bits(),
            "{j}"
        );
    }
}

#[test]
fn the_rolls_are_the_songs_whatever_renders_it() {
    let mut p = Pattern::EMPTY;
    for k in 0..16 {
        p.block(Module::Gate, k, 1, 1.0, 0.5);
    }
    let s = shut_gate(Trigger::Sequence, true);
    let one = |_: i64| [0.5f32, 0.5];
    let len = 8 * 96_000;
    let played = |l: &[f32]| -> Vec<bool> {
        (0..l.len() / 6_000)
            .map(|k| l[k * 6_000 + 3_000] < 0.25)
            .collect()
    };
    let mut a = unit(&s, &p);
    let (la, _) = render(&mut a, 0, len, 120.0, 512, &one);
    let pa = played(&la);
    let n = pa.iter().filter(|x| **x).count();
    assert!(n > 128 * 3 / 10 && n < 128 * 7 / 10, "{n} of 128");
    let mut b = unit(&s, &p);
    let (lb, _) = render(&mut b, 0, len, 120.0, 37, &one);
    assert_eq!(first_diff(&la, &lb), None);
    // From bar 5 the same steps play.
    let mut c = unit(&s, &p);
    let (lc, _) = render(&mut c, 4 * 96_000, len - 4 * 96_000, 120.0, 256, &one);
    assert_eq!(first_diff(&la[4 * 96_000..], &lc), None);
    // Another seed rolls another song.
    let mut d = unit(&FractusSettings { seed: 2, ..s }, &p);
    let (ld, _) = render(&mut d, 0, len, 120.0, 512, &one);
    assert_ne!(pa, played(&ld));
}

fn heavy() -> FractusSettings {
    FractusSettings {
        cloud: CloudSettings {
            level: 0.4,
            density_hz: 80.0,
            pitch_rand_st: 7.0,
            reverse: 0.3,
            ..CloudSettings::default()
        },
        repeat: RepeatSettings {
            pitch_st: 3.0,
            decay_db: -1.0,
            ..RepeatSettings::default()
        },
        ..FractusSettings::default()
    }
}

/// One block of 512 at song sample `at`: the music, or silence; processed or skipped.
fn block(u: &mut Fractus, at: usize, quiet: bool, skip: bool) -> Vec<f32> {
    u.set_transport(&Transport {
        playing: true,
        tempo_bpm: 120.0,
        samples: at as i64,
    });
    let x = |i: usize, c: usize| {
        if quiet {
            0.0
        } else {
            music((at + i) as i64)[c]
        }
    };
    let mut l: Vec<f32> = (0..512).map(|i| x(i, 0)).collect();
    let mut r: Vec<f32> = (0..512).map(|i| x(i, 1)).collect();
    if skip {
        u.skip(512);
    } else {
        u.process(&mut l, &mut r);
    }
    l.extend(r);
    l
}

#[test]
fn it_comes_to_rest_and_a_skip_is_what_silence_would_have_done() {
    let s = heavy();
    let p = super::tests::busy();
    let bar = 96_000usize;
    let mut a = unit(&s, &p);
    let mut b = unit(&s, &p);
    let mut at = 0usize;
    while at < 2 * bar {
        assert_eq!(
            block(&mut a, at, false, false),
            block(&mut b, at, false, false)
        );
        at += 512;
    }
    let mut rest_at = None;
    while at < 2 * bar + 14 * 48_000 {
        let rested = a.at_rest();
        assert_eq!(rested, b.at_rest());
        if rested && rest_at.is_none() {
            rest_at = Some(at);
        }
        let out = block(&mut a, at, true, false);
        if rested {
            assert!(out.iter().all(|x| x.to_bits() == 0), "silence at rest");
            block(&mut b, at, true, true);
        } else {
            assert_eq!(out, block(&mut b, at, true, false));
        }
        at += 512;
    }
    let rest_at = rest_at.unwrap_or_else(|| panic!("never rests: {:?}", a.rest_blocker()));
    eprintln!(
        "heavy settings at rest {:.2} s into the silence",
        (rest_at - 2 * bar) as f64 / f64::from(SR)
    );
    // Sound again: the twin that skipped plays the same bits.
    for _ in 0..(2 * bar / 512) {
        let x = block(&mut a, at, false, false);
        assert_eq!(x, block(&mut b, at, false, false), "at {at}");
        at += 512;
    }
}

#[test]
fn the_cloud_is_its_seed_and_its_song() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Cloud, 0, 16, 1.0, 1.0);
    let s = heavy();
    let len = 150_000;
    let mut a = unit(&s, &p);
    let (la, _) = render(&mut a, 0, len, 120.0, 512, &music);
    let mut b = unit(&s, &p);
    let (lb, _) = render(&mut b, 0, len, 120.0, 61, &music);
    assert_eq!(first_diff(&la, &lb), None);
    let mut c = unit(&FractusSettings { seed: 9, ..s }, &p);
    let (lc, _) = render(&mut c, 0, len, 120.0, 512, &music);
    assert!(first_diff(&la, &lc).is_some());
    let mut t = [0.0f32; 64];
    a.telemetry(&mut t);
    assert!(t[super::t::GRAINS] > 3.0);
    assert_eq!(t[super::t::ACTIVE] as u32, 1 << Module::Cloud.lane());
}

/// The level of the tone at `hz` in `x` (a Hann-windowed DFT bin), dB against a
/// full-scale sine's.
fn tone_db(x: &[f32], hz: f64) -> f64 {
    let n = x.len();
    let (mut re, mut im, mut wsum) = (0.0f64, 0.0f64, 0.0f64);
    for (i, v) in x.iter().enumerate() {
        let w = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64);
        let (s, c) = math::sin_cos(std::f64::consts::TAU * hz * i as f64 / f64::from(SR));
        re += f64::from(*v) * w * c;
        im += f64::from(*v) * w * s;
        wsum += w;
    }
    20.0 * math::log10((2.0 * (re * re + im * im).sqrt() / wsum).max(1e-12))
}

#[test]
fn a_frozen_cloud_keeps_what_it_held() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Cloud, 0, 16, 1.0, 1.0);
    let base = FractusSettings {
        cloud: CloudSettings {
            size_ms: 120.0,
            density_hz: 30.0,
            spray_ms: 200.0,
            ..CloudSettings::default()
        },
        ..FractusSettings::default()
    };
    let switch = |i: i64| {
        if i < 96_000 {
            sine(440.0)(i)
        } else {
            sine(1_500.0)(i)
        }
    };
    for freeze in [false, true] {
        let mut u = unit(&base, &p);
        let (mut l, _) = render(&mut u, 0, 96_000, 120.0, 512, &switch);
        u.set(&FractusSettings {
            cloud: CloudSettings {
                freeze,
                ..base.cloud
            },
            ..base
        });
        let (rest, _) = render(&mut u, 96_000, 96_000, 120.0, 512, &switch);
        l.extend(rest);
        let tail = &l[150_000..190_000];
        let (a, b) = (tone_db(tail, 440.0), tone_db(tail, 1_500.0));
        eprintln!("freeze {freeze}: 440 Hz {a:.1} dB, 1 500 Hz {b:.1} dB");
        if freeze {
            assert!(a > -30.0 && a - b > 40.0, "frozen: {a:.1} / {b:.1}");
            let mut t = [0.0f32; 64];
            u.telemetry(&mut t);
            assert_eq!(t[super::t::FROZEN], 1.0);
        } else {
            assert!(b > -30.0 && b - a > 40.0, "live: {a:.1} / {b:.1}");
        }
    }
}

/// The level at `hz`, averaged over Hann windows of 8 192 (Welch's method, half
/// overlapping): grains of random phase add as powers.
fn welch_db(x: &[f32], hz: f64) -> f64 {
    welch_n(x, hz, 8_192)
}

/// [`welch_db`] with windows of `n`.
fn welch_n(x: &[f32], hz: f64, n: usize) -> f64 {
    let mut p = 0.0f64;
    let mut count = 0;
    let mut at = 0;
    while at + n <= x.len() {
        let d = tone_db(&x[at..at + n], hz);
        p += math::exp2(d / 10.0 * math::log2(10.0));
        count += 1;
        at += n / 2;
    }
    10.0 * math::log10(p / f64::from(count))
}

#[test]
fn pitched_grains_land_on_their_interval_and_alias_little() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Cloud, 0, 16, 1.0, 1.0);
    for (f, st) in [
        (1_000.0, 12.0f32),
        (9_000.0, 7.0),
        (5_000.0, 19.0),
        (3_000.0, 24.0),
        (9_000.0, 12.0),
        (2_000.0, -12.0),
    ] {
        let s = FractusSettings {
            cloud: CloudSettings {
                size_ms: 300.0,
                density_hz: 20.0,
                // Grains from random places, so their phases do not cancel on the grid.
                spray_ms: 40.0,
                spread: 0.0,
                pitch_st: st,
                ..CloudSettings::default()
            },
            ..FractusSettings::default()
        };
        let mut u = unit(&s, &p);
        let (l, _) = render(&mut u, 0, 96_000, 120.0, 512, &sine(f));
        let x = &l[40_000..90_000];
        let want = f * math::exp2(f64::from(st) / 12.0);
        let w = welch_db(x, want);
        // The worst stray tone, every 50 Hz from 500 Hz up, away from the wanted one.
        let mut worst: (f64, f64) = (-300.0, 0.0);
        let mut hz = 500.0;
        while hz < 23_500.0 {
            if (hz - want).abs() > 600.0 {
                let d = welch_db(x, hz);
                if d > worst.0 {
                    worst = (d, hz);
                }
            }
            hz += 50.0;
        }
        eprintln!(
            "{f} Hz at {st:+} st: {want:.0} Hz at {w:.1} dB, worst stray {:.1} dB at {:.0} Hz",
            worst.0 - w,
            worst.1
        );
        assert!(w > -24.0, "{f} Hz {st}: the wanted tone at {w:.1}");
        assert!(worst.0 - w < -40.0, "{f} Hz {st}: {:.1}", worst.0 - w);
    }
}

#[test]
fn scales_quantise_to_their_intervals() {
    assert_eq!(Scale::Major.quantise(1.0), 2.0);
    assert_eq!(Scale::Major.quantise(-1.0), -1.0);
    assert_eq!(Scale::Minor.quantise(3.4), 3.0);
    assert_eq!(Scale::Minor.quantise(4.2), 5.0);
    assert_eq!(Scale::Pentatonic.quantise(4.0), 5.0);
    assert_eq!(Scale::Octaves.quantise(7.0), 12.0);
    assert_eq!(Scale::Fifths.quantise(-4.0), -5.0);
    assert_eq!(Scale::Chromatic.quantise(3.4), 3.0);
    assert_eq!(Scale::Free.quantise(3.4), 3.4);
    for st in [-30.0f32, -12.5, 0.4, 13.9] {
        let q = Scale::Major.quantise(st);
        let r = q.rem_euclid(12.0) as i32;
        assert!([0, 2, 4, 5, 7, 9, 11].contains(&r), "{st} → {q}");
    }
}

#[test]
fn a_throw_echoes_on_its_time_and_dies_away_into_silence() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Throw, 4, 1, 1.0, 1.0);
    let s = FractusSettings {
        throw: ThrowSettings {
            time: 0.75,
            feedback: 0.5,
            tone_hz: 20_000.0,
            pingpong: false,
            level: 1.0,
        },
        ..FractusSettings::default()
    };
    let click = |i: i64| {
        let v = if i == 24_500 { 1.0 } else { 0.0 };
        [v, v]
    };
    let mut u = unit(&s, &p);
    let (l, _) = render(&mut u, 0, 96_000, 120.0, 512, &click);
    let t = 18_000usize; // a dotted eighth at 120 BPM
    let peak = |a: usize| (a - 3..a + 4).max_by(|x, y| l[*x].abs().total_cmp(&l[*y].abs()));
    assert_eq!(peak(24_500 + t), Some(24_500 + t));
    assert_eq!(peak(24_500 + 2 * t), Some(24_500 + 2 * t));
    let (e1, e2) = (l[24_500 + t], l[24_500 + 2 * t]);
    assert!(e1 > 0.7 && (e2 / e1 - 0.5).abs() < 0.15, "{e1} {e2}");
    // Long after, the echoes are flushed to silence and the unit can rest.
    let (q, _) = render(&mut u, 96_000, 12 * 48_000, 120.0, 512, &|_| [0.0, 0.0]);
    assert!(q[q.len() - 1_000..].iter().all(|x| x.to_bits() == 0));
}

#[test]
fn the_buffers_are_sized_once_and_say_what_they_take() {
    let u = Fractus::new(48_000.0);
    let mb = u.memory_bytes() as f64 / 1e6;
    eprintln!("Fractus at 48 kHz holds {mb:.1} MB");
    assert!(mb < 12.0, "{mb}");
}

/// Upward zero crossings a second in `x`.
fn crossings_hz(x: &[f32]) -> f64 {
    let n = x.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
    n as f64 * f64::from(SR) / x.len() as f64
}

#[test]
fn a_pitch_jump_lands_on_its_interval() {
    for (semis, clean) in [12.0f32, 7.0, -12.0]
        .into_iter()
        .flat_map(|s| [(s, false), (s, true)])
    {
        let mut p = Pattern::EMPTY;
        p.block(Module::Pitch, 0, 16, 1.0, 1.0);
        let s = FractusSettings {
            pitch: PitchSettings {
                semis,
                window_ms: 50.0,
                clean,
            },
            ..FractusSettings::default()
        };
        let mut u = unit(&s, &p);
        // 311 Hz: not a whole number of periods in the window, whose two taps would
        // otherwise meet in opposite phase on a pure tone.
        let (l, _) = render(&mut u, 0, 96_000, 120.0, 512, &sine(311.0));
        let x = &l[20_000..90_000];
        let want = 311.0 * math::exp2(f64::from(semis) / 12.0);
        let mut best = (f64::MIN, 0.0);
        let mut hz = 100.0;
        while hz < 1_500.0 {
            let d = welch_n(x, hz, 32_768);
            if d > best.0 {
                best = (d, hz);
            }
            hz += 0.5;
        }
        eprintln!(
            "pitch {semis:+} (clean {clean}): the strongest at {:.0} Hz, want {want:.1}",
            best.1
        );
        // A delay-line shifter's taps restart every `W / |1 − ratio|`; on a pure tone each
        // restart moves the phase, so the strongest line sits within half that rate of the
        // interval (±10 Hz at +12 with a 50 ms window) — the classic glitch pitch, not a
        // tuner's.
        let rate = (1.0 - math::exp2(f64::from(semis) / 12.0)).abs() / 0.05;
        assert!(
            (best.1 - want).abs() <= 0.5 * rate + 1.0,
            "{semis} (clean {clean}): {:.1} Hz",
            best.1
        );
    }
}

#[test]
fn a_filter_sweep_closes_over_its_event() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Filter, 0, 16, 1.0, 1.0);
    let s = FractusSettings {
        filter: FilterSettings {
            kind: FilterKind::LowPass,
            from_hz: 18_000.0,
            to_hz: 200.0,
            res: 0.2,
            curve: 0.0,
        },
        ..FractusSettings::default()
    };
    let mut u = unit(&s, &p);
    let (l, _) = render(&mut u, 0, 96_000, 120.0, 512, &sine(3_000.0));
    let peak = |a: usize, b: usize| l[a..b].iter().fold(0.0f32, |m, x| m.max(x.abs()));
    // 3 kHz passes while the cut-off is above it and is gone near the end.
    assert!(peak(4_000, 12_000) > 0.45, "{}", peak(4_000, 12_000));
    assert!(peak(84_000, 95_000) < 0.02, "{}", peak(84_000, 95_000));
}

#[test]
fn a_ramped_repeat_halves_its_slice_in_each_part() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Repeat, 0, 16, 1.0, 1.0);
    let s = FractusSettings {
        repeat: RepeatSettings {
            size: 0.5,
            ramp: 3,
            ..RepeatSettings::default()
        },
        ..FractusSettings::default()
    };
    let mut u = unit(&s, &p);
    let (l, _) = render(&mut u, 0, 96_000, 120.0, 512, &music);
    // Four parts of a beat: slices of 1/8, 1/16, 1/32 and 1/64 — each part a run of
    // copies of the event's first slice of its size.
    for (part, r) in [12_000usize, 6_000, 3_000, 1_500].iter().enumerate() {
        let p0 = part * 24_000;
        for k in 1..24_000 / r {
            let at = p0 + k * r;
            for j in (120..*r - 10).filter(|j| at + j < 96_000 - 100) {
                assert_eq!(
                    l[at + j].to_bits(),
                    music(j as i64)[0].to_bits(),
                    "part {part}"
                );
            }
        }
    }
}

#[test]
fn a_tape_start_comes_up_to_speed_from_silence() {
    let mut p = Pattern::EMPTY;
    p.block(Module::TapeStop, 0, 8, 1.0, 1.0);
    let s = FractusSettings {
        tape: TapeSettings {
            kind: TapeKind::Start,
            time: 1.0,
            curve: 0.3,
        },
        ..FractusSettings::default()
    };
    let mut u = unit(&s, &p);
    let (l, _) = render(&mut u, 0, 96_000, 120.0, 512, &sine(400.0));
    let peak = |a: usize, b: usize| l[a..b].iter().fold(0.0f32, |m, x| m.max(x.abs()));
    assert!(
        peak(200, 1_000) < 0.1,
        "it starts still: {}",
        peak(200, 1_000)
    );
    // Up to speed after a beat: the tone, a little behind.
    let hz = crossings_hz(&l[26_000..46_000]);
    assert!((hz - 400.0).abs() < 4.0, "{hz}");
    assert!(peak(26_000, 46_000) > 0.49);
}

#[test]
fn stopped_the_steps_rest_and_notes_still_play() {
    let mut p = Pattern::EMPTY;
    p.block(Module::Gate, 0, 16, 1.0, 1.0);
    let mut u = unit(&shut_gate(Trigger::Both, false), &p);
    let mut l: Vec<f32> = (0..20_000).map(|i| music(i)[0]).collect();
    let mut r: Vec<f32> = (0..20_000).map(|i| music(i)[1]).collect();
    for (k, (a, b)) in l.chunks_mut(500).zip(r.chunks_mut(500)).enumerate() {
        u.set_transport(&Transport {
            playing: false,
            tempo_bpm: 120.0,
            samples: 0,
        });
        if k == 20 {
            u.note(0, Note::On(GATE_KEY, 127));
        }
        u.process(a, b);
    }
    let dry: Vec<f32> = (0..20_000).map(|i| music(i)[0]).collect();
    assert_eq!(first_diff(&l, &dry), Some(10_000), "only the note shuts it");
}

/// The owner's choice (warble or clean, ADR 0137), measured on a pure tone a fifth up: the
/// classic four heads always sound together, their phases weighed by moving windows, so
/// the pitch wavers period by period — the warble; the clean style plays one head at a
/// time between its splices, so most periods are the interval's exactly. Measured as the
/// share of periods (upward zero crossings, interpolated) within 0.5 % of the wanted
/// period, and the level's spread over 5 ms windows (both hold their level).
#[test]
fn the_clean_pitch_jump_holds_its_pitch_and_the_classic_warbles() {
    let measure = |clean: bool| {
        let mut p = Pattern::EMPTY;
        p.block(Module::Pitch, 0, 16, 1.0, 1.0);
        let s = FractusSettings {
            pitch: PitchSettings {
                semis: 7.0,
                window_ms: 50.0,
                clean,
            },
            ..FractusSettings::default()
        };
        let mut u = unit(&s, &p);
        let (l, _) = render(&mut u, 0, 96_000, 120.0, 512, &sine(311.0));
        let x = &l[20_000..90_000];
        let mut ups = Vec::new();
        for i in 1..x.len() {
            if x[i - 1] <= 0.0 && x[i] > 0.0 {
                let f = f64::from(x[i - 1]) / f64::from(x[i - 1] - x[i]);
                ups.push(i as f64 - 1.0 + f);
            }
        }
        let want = f64::from(SR) / (311.0 * math::exp2(7.0 / 12.0));
        let exact = ups
            .windows(2)
            .filter(|w| ((w[1] - w[0]) / want - 1.0).abs() <= 0.005)
            .count() as f64
            / (ups.len() - 1) as f64;
        let w = 240;
        let db: Vec<f64> = x
            .chunks(w)
            .map(|c| {
                let e: f64 = c.iter().map(|v| f64::from(*v) * f64::from(*v)).sum();
                10.0 * math::log10(e / w as f64 + 1e-20)
            })
            .collect();
        let spread = db.iter().cloned().fold(f64::MIN, f64::max)
            - db.iter().cloned().fold(f64::MAX, f64::min);
        (exact, spread)
    };
    let (classic, classic_db) = measure(false);
    let (clean, clean_db) = measure(true);
    eprintln!(
        "periods within 0.5 %: classic {:.0} %, clean {:.0} %; level spread {classic_db:.1} / {clean_db:.1} dB",
        100.0 * classic,
        100.0 * clean
    );
    assert!(
        clean > classic + 0.1,
        "clean {clean:.2} vs classic {classic:.2}"
    );
    assert!(clean_db < 4.0 && classic_db < 4.0);
}
