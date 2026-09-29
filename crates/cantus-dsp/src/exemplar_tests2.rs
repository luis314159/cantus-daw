//! Exemplar's DSP tests, part two: loops and the zone engine.

use super::play::Playhead;
use super::resample::MAX_TAPS;
use super::tests::source_of;
use super::*;

fn sine_cycle(period: usize, cycles: usize) -> Vec<f32> {
    (0..period * cycles)
        .map(|i| (std::f64::consts::TAU * i as f64 / period as f64).sin() as f32 * 0.5)
        .collect()
}

fn run(p: &mut Playhead, ratio: f64, q: Quality, n: usize) -> Vec<f32> {
    let reader = Reader::new(q, 48_000.0);
    let mut coefs = [0.0f32; MAX_TAPS];
    (0..n)
        .map(|_| p.next(ratio, &reader, &mut coefs)[0])
        .collect()
}

fn max_step(x: &[f32]) -> f32 {
    x.windows(2).fold(0f32, |m, w| m.max((w[1] - w[0]).abs()))
}

#[test]
fn a_loop_an_exact_number_of_periods_long_has_no_seam() {
    // 100-frame period; loop over periods 3..7 of 10; played at unity and resampled.
    let src = source_of(sine_cycle(100, 10), 1, 48_000.0, false);
    let region = Region {
        end: 1_000,
        looping: true,
        ls: 300,
        le: 699,
        ..Region::default()
    };
    for (ratio, q) in [
        (1.0, Quality::High),
        (1.2345, Quality::High),
        (0.77, Quality::Best),
    ] {
        let reader = Reader::new(q, 48_000.0);
        let mut p = Playhead::default();
        p.start(src.clone(), region, ratio, &reader, 0);
        let out = run(&mut p, ratio, q, 20_000);
        let steady = &out[2_000..];
        let inside =
            0.5 * (std::f64::consts::TAU * ratio / 100.0).sin().abs() as f32 * 1.001 + 1e-4;
        assert!(
            max_step(steady) <= inside,
            "ratio {ratio}: a step of {} at the seam (a period has {inside})",
            max_step(steady)
        );
        assert!(!p.is_done(), "a continuous loop never ends");
    }
}

#[test]
fn a_crossfade_smooths_a_bad_loop() {
    // A loop whose ends do not match: 375 frames of a 100-frame period, so the join goes
    // from the trough (−0.5) back to a zero crossing.
    let src = source_of(sine_cycle(100, 20), 1, 48_000.0, false);
    let bad = Region {
        end: 2_000,
        looping: true,
        ls: 500,
        le: 874,
        ..Region::default()
    };
    let reader = Reader::new(Quality::High, 48_000.0);
    let mut p = Playhead::default();
    p.start(src.clone(), bad, 1.0, &reader, 0);
    let raw = max_step(&run(&mut p, 1.0, Quality::High, 5_000)[1_000..]);
    let mut p = Playhead::default();
    p.start(src, Region { xf: 200, ..bad }, 1.0, &reader, 0);
    let faded = max_step(&run(&mut p, 1.0, Quality::High, 5_000)[1_000..]);
    assert!(raw > 0.3, "the bad loop clicks ({raw})");
    assert!(faded < 0.05, "a 200-frame crossfade smooths it ({faded})");
}

#[test]
fn ping_pong_turns_around_without_repeating_or_skipping() {
    let data: Vec<f32> = (0..64).map(|i| i as f32).collect();
    let src = source_of(data, 1, 48_000.0, false);
    let region = Region {
        end: 64,
        looping: true,
        ls: 10,
        le: 20,
        dir: LoopDir::PingPong,
        ..Region::default()
    };
    let reader = Reader::new(Quality::High, 48_000.0);
    let mut p = Playhead::default();
    p.start(src, region, 1.0, &reader, 0);
    let out = run(&mut p, 1.0, Quality::High, 45);
    let want: Vec<f32> = (0..=20)
        .chain((10..20).rev())
        .chain(11..=20)
        .chain((10..20).rev())
        .take(45)
        .map(|i| i as f32)
        .collect();
    assert_eq!(out, want);
}

#[test]
fn a_released_sustain_loop_plays_on_into_the_tail() {
    let data: Vec<f32> = (0..100).map(|i| i as f32).collect();
    let src = source_of(data, 1, 48_000.0, false);
    let region = Region {
        end: 100,
        looping: true,
        ls: 20,
        le: 29,
        ..Region::default()
    };
    let reader = Reader::new(Quality::High, 48_000.0);
    let mut p = Playhead::default();
    p.start(src, region, 1.0, &reader, 0);
    let first = run(&mut p, 1.0, Quality::High, 45);
    p.release_loop();
    let rest = run(&mut p, 1.0, Quality::High, 200);
    // 0..29, 20..29, then five of the next lap; the lap in progress ends and the tail
    // (30…99) follows.
    assert_eq!(
        &first[25..35],
        &[25., 26., 27., 28., 29., 20., 21., 22., 23., 24.]
    );
    assert_eq!(
        &rest[..10],
        &[25., 26., 27., 28., 29., 30., 31., 32., 33., 34.]
    );
    assert!(p.is_done(), "the tail ends the voice");
}

#[test]
fn built_in_numbers_are_the_ones_presets_name() {
    let names = [
        "sine",
        "saw",
        "square",
        "triangle",
        "noise",
        "pluck_c2",
        "pluck_c4",
        "pluck_c6",
        "bell_soft",
        "bell_hard",
        "pad",
        "kick",
        "snare_1",
        "snare_2",
        "snare_3",
        "hat_1",
        "hat_2",
        "hat_open",
        "click",
        "organ",
        "choir",
    ];
    for (i, n) in names.iter().enumerate() {
        assert_eq!(builtin::index_of(n), Some(i as u8 + 1), "{n}");
        let b = builtin::get(i as u8 + 1).unwrap();
        let peak = b.data().iter().fold(0f32, |m, x| m.max(x.abs()));
        assert!(
            (peak - 0.5).abs() < 1e-3,
            "{n} is normalised to −6 dBFS ({peak})"
        );
        if let Some((ls, le)) = b.loop_points {
            assert!((le as usize) < b.frames && ls < le, "{n}");
        }
    }
    assert_eq!(builtin::builtins().len(), names.len());
}

#[test]
fn measured_alias_floors_and_cost() {
    // Printed for the report (`cargo test -p cantus-dsp measured -- --nocapture`); the
    // pass criteria are in `transposing_up_does_not_alias_at_high_or_best`.
    for q in [
        Quality::Draft,
        Quality::Standard,
        Quality::High,
        Quality::Best,
    ] {
        let (a12, _) = super::tests::alias_floor(q, 12.0);
        let (a24, _) = super::tests::alias_floor(q, 24.0);
        println!("{q:?}: worst alias +12 st {a12:.1} dBFS, +24 st {a24:.1} dBFS");
    }
    let src = source_of(sine_cycle(441, 400), 2, 44_100.0, false);
    let reader = Reader::new(Quality::High, 48_000.0);
    let mut coefs = [0.0f32; MAX_TAPS];
    let n = 480_000;
    for st in [1.0f64, 36.0] {
        let ratio = 44_100.0 / 48_000.0 * (st / 12.0).exp2();
        let mut p = Playhead::default();
        p.start(
            src.clone(),
            Region {
                looping: true,
                ls: 0,
                le: 441 * 400 - 1,
                end: 441 * 400,
                ..Region::default()
            },
            ratio,
            &reader,
            0,
        );
        let t = std::time::Instant::now();
        let mut acc = 0.0f32;
        for _ in 0..n {
            acc += p.next(ratio, &reader, &mut coefs)[0];
        }
        let ns = t.elapsed().as_nanos() as f64 / n as f64;
        println!("High, stereo, +{st} st: {ns:.1} ns per output frame ({acc:.1})");
    }
}

// ---- the engine ------------------------------------------------------------------------------

fn engine_with(zones: &[(ZoneParams, Source)]) -> Exemplar {
    let mut e = Exemplar::new(48_000.0);
    let g = Globals {
        amp: [0.1, 0.1, 1_000.0, 1.0, 5.0],
        ..Globals::default()
    };
    e.set_globals(&g);
    for (n, (z, s)) in zones.iter().enumerate() {
        e.put_zone(n, z, s);
    }
    e
}

fn dc(level: f32, frames: usize) -> Source {
    source_of(vec![level; frames], 1, 48_000.0, false)
}

fn render(e: &mut Exemplar, frames: usize, block: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames * 2);
    let (mut l, mut r) = (vec![0.0; block], vec![0.0; block]);
    let mut done = 0;
    while done < frames {
        let n = block.min(frames - done);
        l[..n].fill(0.0);
        r[..n].fill(0.0);
        e.process(&mut l[..n], &mut r[..n]);
        for i in 0..n {
            out.push(l[i]);
            out.push(r[i]);
        }
        done += n;
    }
    out
}

#[test]
fn velocity_crossfades_keep_their_law() {
    // Two layers crossfading over velocities 60…80, power then gain curves.
    for gain_curve in [false, true] {
        let soft = ZoneParams {
            root: 60,
            vhi: 80,
            vfout: 20,
            fade_gain: gain_curve,
            ..ZoneParams::default()
        };
        let loud = ZoneParams {
            root: 60,
            vlo: 60,
            vfin: 20,
            fade_gain: gain_curve,
            ..ZoneParams::default()
        };
        for v7 in [60.0f32, 65.0, 70.0, 75.0, 80.0] {
            let a = engine::crossfade(&soft, 60, v7);
            let b = engine::crossfade(&loud, 60, v7);
            let sum = if gain_curve { a + b } else { a * a + b * b };
            let db = 20.0 * sum.log10();
            assert!(
                db.abs() < 0.01,
                "v {v7} gain={gain_curve}: {a} + {b} gives {db} dB"
            );
        }
    }
}

#[test]
fn round_robin_and_random_repeat_exactly_and_vary() {
    // Four round-robin zones, each a different constant level.
    let zones: Vec<(ZoneParams, Source)> = (1..=4u8)
        .map(|seq| {
            (
                ZoneParams {
                    seq,
                    seqlen: 4,
                    ..ZoneParams::default()
                },
                dc(seq as f32 * 0.1, 48_000),
            )
        })
        .collect();
    let pick = |id: u64| {
        let mut e = engine_with(&zones);
        e.note_on(60, 1.0, id);
        let out = render(&mut e, 64, 64);
        (out[40] * 10.0).round() as u32
    };
    let first: Vec<u32> = (1..=40).map(pick).collect();
    let again: Vec<u32> = (1..=40).map(pick).collect();
    assert_eq!(first, again, "the same notes choose the same zones");
    for slot in 1..=4 {
        assert!(first.contains(&slot), "slot {slot} is used: {first:?}");
    }
    assert!(
        first.iter().all(|s| (1..=4).contains(s)),
        "exactly one zone per note"
    );
    // Live input cycles strictly.
    let mut e = engine_with(&zones);
    let mut live = Vec::new();
    for _ in 0..8 {
        e.note_on(60, 1.0, u64::MAX - 61);
        let out = render(&mut e, 64, 64);
        live.push((out[40] * 10.0).round() as u32);
        e.note_off(60, u64::MAX - 61, 0.0);
        render(&mut e, 4_000, 256);
    }
    assert_eq!(live, [1, 2, 3, 4, 1, 2, 3, 4]);
}

#[test]
fn a_release_sample_is_quieter_by_its_decay_per_second_held() {
    let attack = ZoneParams::default();
    let release = ZoneParams {
        trig: Trig::Release,
        rt_decay: 6.0,
        ..ZoneParams::default()
    };
    let mut e = engine_with(&[(attack, dc(0.0, 480_000)), (release, dc(0.5, 48_000))]);
    e.note_on(60, 1.0, 7);
    render(&mut e, 96_000, 512); // two seconds
    e.note_off(60, 7, 0.0);
    let out = render(&mut e, 2_000, 512);
    let level = 20.0 * (out[2 * 1_000] / 0.5).log10();
    assert!(
        (level + 12.0).abs() < 0.05,
        "held 2 s at 6 dB/s: {level} dB"
    );
}

#[test]
fn an_exclusive_class_chokes_only_its_members() {
    // The closed hat (class 1) chokes the open hat (off by 1); the kick is untouched.
    let open = ZoneParams {
        lo: 46,
        hi: 46,
        root: 46,
        offby: 1,
        ..ZoneParams::default()
    };
    let closed = ZoneParams {
        lo: 42,
        hi: 42,
        root: 42,
        excl: 1,
        ..ZoneParams::default()
    };
    let kick = ZoneParams {
        lo: 36,
        hi: 36,
        root: 36,
        ..ZoneParams::default()
    };
    let mut e = engine_with(&[
        (open, dc(0.25, 480_000)),
        (closed, dc(0.0, 480_000)),
        (kick, dc(0.125, 480_000)),
    ]);
    e.note_on(46, 1.0, 1);
    e.note_on(36, 1.0, 2);
    let before = render(&mut e, 1_000, 256);
    e.note_on(42, 1.0, 3);
    let out = render(&mut e, 2_000, 256);
    let last = before[before.len() - 2];
    assert_eq!(last, 0.375, "both sound before the choke");
    let tail = out[2 * 1_000];
    assert!(
        (tail - 0.125).abs() < 1e-6,
        "the open hat is gone, the kick stays: {tail}"
    );
    // The fade takes at least 5 ms: no step larger than 0.25 / 240.
    let left: Vec<f32> = out.iter().step_by(2).copied().collect();
    let steps = left
        .windows(2)
        .fold(0f32, |m, w| m.max((w[1] - w[0]).abs()));
    assert!(steps <= 0.25 / 240.0 + 1e-6, "the choke clicks: {steps}");
}

#[test]
fn renders_are_the_same_at_every_block_size_and_end_in_exact_silence() {
    let pluck = builtin::get(builtin::index_of("pluck_c4").unwrap()).unwrap();
    let pad = builtin::get(builtin::index_of("pad").unwrap()).unwrap();
    let zones = vec![
        (
            ZoneParams {
                root: 60,
                hi: 71,
                ..ZoneParams::default()
            },
            pluck.source(),
        ),
        (
            ZoneParams {
                lo: 72,
                root: 48,
                region: Region {
                    looping: true,
                    ls: 44_100,
                    le: 129_999,
                    xf: 8_000,
                    ..Region::default()
                },
                ..ZoneParams::default()
            },
            pad.source(),
        ),
    ];
    let play = |block: usize| {
        let mut e = engine_with(&zones);
        let mut g = e.globals().clone();
        g.amp[4] = 200.0;
        g.filter = 1;
        g.cutoff = 3_000.0;
        g.vibrato_ct = 30.0;
        e.set_globals(&g);
        let mut out = Vec::new();
        for (i, key) in [60u8, 64, 67, 79, 84].iter().enumerate() {
            e.note_on(*key, 0.8, i as u64 + 1);
            out.extend(render(&mut e, 3_001, block));
        }
        e.controller(2, 0.7);
        out.extend(render(&mut e, 5_000, block));
        for (i, key) in [60u8, 64, 67, 79, 84].iter().enumerate() {
            e.note_off(*key, i as u64 + 1, 0.0);
        }
        out.extend(render(&mut e, 96_000, block));
        (out, e.active_layers())
    };
    let (reference, left) = play(512);
    assert_eq!(left, 0, "every layer has ended");
    assert!(reference.iter().any(|x| x.abs() > 0.01));
    assert!(reference[reference.len() - 2_000..]
        .iter()
        .all(|x| *x == 0.0));
    for block in [1usize, 7, 64, 4_096] {
        let (out, _) = play(block);
        assert!(out == reference, "block size {block} differs");
    }
    let mut e = engine_with(&zones);
    let (mut l, mut r) = ([0.0f32; 64], [0.0f32; 64]);
    assert!(
        !e.process(&mut l, &mut r),
        "idle is exact silence and says so"
    );
}
