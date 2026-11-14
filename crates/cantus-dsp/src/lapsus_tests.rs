//! Lapsus's DSP measured: the null, the classics' arithmetic, sample-exact bars at three
//! tempos, bar 5 equal to bar 5 whatever came before, block-size independence, joins
//! without clicks, the interpolator's quality and its anti-alias, notes, rest.

use super::*;
use crate::fluctus::CurvePoint;
use crate::modfx_clock::Transport;

const SR: f32 = 48_000.0;

/// The automation editor's curve, S-curve and hold (`cantus_core::automation::Shape::at`),
/// enough for the tests.
fn shape(s: u8, t: f32, u: f32) -> f32 {
    let bend = |u: f32, t: f32| {
        if t == 0.0 {
            u
        } else {
            let p = 1.0 + 7.0 * t.abs();
            if t > 0.0 {
                crate::math::powf(u, p)
            } else {
                1.0 - crate::math::powf(1.0 - u, p)
            }
        }
    };
    match s {
        2 => 0.0,
        1 => {
            if u < 0.5 {
                0.5 * bend(2.0 * u, t.abs())
            } else {
                1.0 - 0.5 * bend(2.0 - 2.0 * u, t.abs())
            }
        }
        _ => bend(u, t),
    }
}

fn pt(x: f32, y: f32) -> CurvePoint {
    CurvePoint {
        x,
        y,
        tension: 0.0,
        shape: 0,
    }
}

fn bent(x: f32, y: f32, tension: f32) -> CurvePoint {
    CurvePoint {
        tension,
        ..pt(x, y)
    }
}

fn held(x: f32, y: f32) -> CurvePoint {
    CurvePoint {
        shape: 2,
        ..pt(x, y)
    }
}

fn s_curve(x: f32, y: f32) -> CurvePoint {
    CurvePoint {
        shape: 1,
        tension: 0.15,
        ..pt(x, y)
    }
}

const TAPE: f32 = 1.0 / 7.0;

/// A time pattern and a volume pattern of the same length, played together (the two banks'
/// slot 1).
#[derive(Copy, Clone, Debug)]
struct Pair {
    time: Pattern,
    vol: Pattern,
}

fn pair(beats: f64, time: &[CurvePoint], vol: &[CurvePoint]) -> Pair {
    Pair {
        time: Pattern::new(Lane::Time, beats, time),
        vol: Pattern::new(Lane::Vol, beats, vol),
    }
}

fn tape_stop() -> Pair {
    pair(
        4.0,
        &[bent(0.0, 0.0, TAPE), pt(1.0, 0.5)],
        &[pt(0.0, 1.0), pt(1.0, 0.0)],
    )
}

/// A bar played, its last beat stopping: from the next bar's line it plays as it arrives.
fn stop_last_beat() -> Pair {
    pair(
        4.0,
        &[pt(0.0, 0.0), bent(0.75, 0.0, TAPE), pt(1.0, 0.125)],
        &[pt(0.0, 1.0), pt(0.75, 1.0), pt(1.0, 0.0)],
    )
}

/// The patterns the bar-5 and split tests run: one of each family.
fn families() -> Vec<(&'static str, Pair)> {
    vec![
        ("tape stop", tape_stop()),
        ("half time ½", pair(2.0, &[pt(0.0, 0.0), pt(1.0, 0.5)], &[])),
        (
            "stutter 1/16",
            pair(
                1.0,
                &[
                    held(0.0, 0.0),
                    held(0.25, 0.25),
                    held(0.5, 0.5),
                    held(0.75, 0.75),
                ],
                &[],
            ),
        ),
        (
            "reverse ½",
            pair(4.0, &[pt(0.0, 0.0), pt(0.5, 0.0), pt(1.0, 1.0)], &[]),
        ),
        (
            "scratch",
            pair(
                1.0,
                &[
                    s_curve(0.0, 0.0),
                    s_curve(0.25, 0.25),
                    s_curve(0.5, 0.0),
                    s_curve(0.75, 0.25),
                ],
                &[],
            ),
        ),
        (
            "gate",
            pair(
                4.0,
                &[],
                &(0..16)
                    .map(|k| held(k as f32 / 16.0, if k % 2 == 0 { 1.0 } else { 0.0 }))
                    .collect::<Vec<_>>(),
            ),
        ),
        (
            "tape start",
            pair(
                4.0,
                &[bent(0.0, 0.0, -TAPE), pt(0.25, 0.125), pt(1.0, 0.125)],
                &[pt(0.0, 0.0), pt(0.25, 1.0), pt(1.0, 1.0)],
            ),
        ),
    ]
}

fn unit_with(p: &Pair, s: LapsusSettings) -> Lapsus {
    let mut u = Lapsus::new(SR, shape);
    u.set_pattern(Lane::Time, 1, &p.time);
    u.set_pattern(Lane::Vol, 1, &p.vol);
    u.set(&LapsusSettings {
        tslot: 1,
        vslot: 1,
        ..s
    });
    u.ensure_memory();
    u.reset();
    u
}

fn unit(p: &Pair) -> Lapsus {
    unit_with(p, LapsusSettings::default())
}

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed | 1;
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32 * 2.0 - 1.0) * 0.5
        })
        .collect()
}

fn sine(n: usize, hz: f64, phase: f64) -> Vec<f32> {
    (0..n)
        .map(|i| {
            crate::math::sin(std::f64::consts::TAU * hz * i as f64 / f64::from(SR) + phase) as f32
        })
        .collect()
}

/// Music-like: three partials and some noise, so a stray step shows.
fn music(n: usize) -> Vec<f32> {
    let (a, b, c, w) = (
        sine(n, 110.0, 0.0),
        sine(n, 440.0, 1.0),
        sine(n, 2_637.0, 2.0),
        noise(n, 7),
    );
    (0..n)
        .map(|i| 0.4 * a[i] + 0.25 * b[i] + 0.1 * c[i] + 0.1 * w[i])
        .collect()
}

fn same(a: &[f32], b: &[f32]) -> bool {
    if let Some(i) = (0..a.len().min(b.len())).find(|&i| a[i].to_bits() != b[i].to_bits()) {
        eprintln!("  first difference at {i}: {:e} against {:e}", a[i], b[i]);
    }
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

/// Play `l`/`r` through `u` from song sample `start` at `tempo`, in blocks from `split`.
fn drive(
    u: &mut Lapsus,
    l: &mut [f32],
    r: &mut [f32],
    tempo: f64,
    start: i64,
    mut split: impl FnMut() -> usize,
) {
    let n = l.len();
    let mut at = 0;
    while at < n {
        let b = split().clamp(1, n - at);
        u.set_transport(&Transport {
            playing: true,
            tempo_bpm: tempo,
            samples: start + at as i64,
        });
        u.process(&mut l[at..at + b], &mut r[at..at + b]);
        at += b;
    }
}

fn bar_sample(bar: i64, tempo: f64) -> i64 {
    ((bar as f64 * 4.0) * 60.0 / tempo * f64::from(SR)).round() as i64
}

#[test]
fn the_defaults_touch_no_sample_with_or_without_memory() {
    let x = music(20_000);
    for memory in [false, true] {
        let mut u = Lapsus::new(SR, shape);
        if memory {
            u.ensure_memory();
        }
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut u, &mut l, &mut r, 120.0, 0, || 333);
        assert!(same(&l, &x) && same(&r, &x), "memory {memory}");
        assert_eq!(u.latency(), 0);
    }
}

#[test]
fn a_flat_curve_is_a_null_through_the_full_path() {
    // Points on the line at 0 and a volume held at 1: the sample-by-sample path, every
    // read a whole sample.
    let p = pair(
        1.0,
        &[held(0.0, 0.0), held(0.5, 0.0)],
        &[held(0.0, 1.0), held(0.7, 1.0)],
    );
    let mut u = unit(&p);
    let x = music(30_000);
    let (mut l, mut r) = (x.clone(), x.clone());
    u.set_transport(&Transport {
        playing: true,
        tempo_bpm: 174.0,
        samples: 1_234,
    });
    u.process_by_ticks(&mut l, &mut r);
    assert!(same(&l, &x) && same(&r, &x));
}

#[test]
fn half_time_reads_the_cycle_at_half_speed() {
    // One bar at 120 BPM (96 000 samples): the output at t is the input at t/2.
    let p = pair(4.0, &[pt(0.0, 0.0), pt(1.0, 0.5)], &[]);
    let mut u = unit(&p);
    let x = sine(96_000, 300.0, 0.3);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut u, &mut l, &mut r, 120.0, 0, || 512);
    let mut worst = 0.0f64;
    for t in (2_000..90_000).step_by(37) {
        let want =
            crate::math::sin(std::f64::consts::TAU * 300.0 * (t as f64 / 2.0) / 48_000.0 + 0.3);
        worst = worst.max((f64::from(l[t]) - want).abs());
    }
    assert!(worst < 1e-4, "half time read within {worst:e}");
}

#[test]
fn a_tape_stop_ends_on_the_bar_to_the_sample_at_60_120_and_174() {
    for tempo in [60.0, 120.0, 174.0] {
        let mut u = unit(&stop_last_beat());
        let bar2 = bar_sample(1, tempo);
        let n = bar2 as usize + 20_000;
        let x = music(n);
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut u, &mut l, &mut r, tempo, 0, || 441);
        // From the bar's own sample, the next bar plays as it arrives, to the bit.
        let b = bar2 as usize;
        assert!(
            same(&l[b..b + 2_000], &x[b..b + 2_000]),
            "{tempo} BPM: the bar starts on its sample"
        );
        assert!(
            l[b - 1].to_bits() != x[b - 1].to_bits(),
            "{tempo} BPM: the sample before is the stop's"
        );
        // The last 10 ms before the join are the stop's end: still, silent.
        let quiet = l[b - 480..b - 150].iter().fold(0f32, |m, v| m.max(v.abs()));
        // The level falls with the speed: 10 ms before the line it is 0.01 … 0.03 of it.
        assert!(quiet < 0.03, "{tempo} BPM: the tape has stopped ({quiet})");
        // And the bar's line is where the engine puts it.
        assert_eq!(bar2, ((4.0 * 60.0 / tempo) * 48_000.0f64).round() as i64);
    }
}

#[test]
fn bar_five_is_bar_five_of_the_whole_render_whatever_came_before() {
    for (name, p) in families() {
        for tempo in [120.0, 174.0] {
            let b5 = bar_sample(4, tempo);
            let n = b5 as usize + 3 * 96_000;
            let x = music(n);
            let mut whole = unit(&p);
            let (mut wl, mut wr) = (x.clone(), x.clone());
            drive(&mut whole, &mut wl, &mut wr, tempo, 0, || 500);
            let mut part = unit(&p);
            let (mut pl, mut pr) = (x[b5 as usize..].to_vec(), x[b5 as usize..].to_vec());
            drive(&mut part, &mut pl, &mut pr, tempo, b5, || 300);
            assert!(
                same(&wl[b5 as usize..], &pl),
                "{name} at {tempo}: left from bar 5"
            );
            assert!(
                same(&wr[b5 as usize..], &pr),
                "{name} at {tempo}: right from bar 5"
            );
        }
    }
}

#[test]
fn every_block_size_renders_the_same_bits_with_notes() {
    for (name, p) in families() {
        let x = music(60_000);
        let run = |split: &mut dyn FnMut() -> usize| {
            let mut u = unit(&p);
            u.set_pattern(Lane::Time, 4, &families()[2].1.time);
            u.set_pattern(Lane::Vol, 6, &families()[5].1.vol);
            let (mut l, mut r) = (x.clone(), x.clone());
            let notes = [
                (9_000usize, Note::On(64)),
                (12_345, Note::On(30)),
                (21_000, Note::Off(64)),
                (25_000, Note::Off(30)),
                (30_001, Note::On(61)),
                (41_000, Note::AllOff),
            ];
            let mut at = 0;
            while at < x.len() {
                let b = split().clamp(1, x.len() - at);
                u.set_transport(&Transport {
                    playing: true,
                    tempo_bpm: 133.0,
                    samples: 777 + at as i64,
                });
                for &(f, nt) in &notes {
                    if f >= at && f < at + b {
                        u.note(f - at, nt);
                    }
                }
                u.process(&mut l[at..at + b], &mut r[at..at + b]);
                at += b;
            }
            (l, r)
        };
        let reference = run(&mut || 512);
        for split in [1usize, 7, 64, 4_096] {
            let out = run(&mut || split);
            assert!(
                same(&out.0, &reference.0) && same(&out.1, &reference.1),
                "{name}: blocks of {split}"
            );
        }
    }
}

/// The largest second difference: where a waveform steps, it shows.
fn roughness(x: &[f32]) -> f32 {
    x.windows(3)
        .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs())
        .fold(0.0, f32::max)
}

#[test]
fn the_joins_do_not_click_and_without_a_crossfade_they_would() {
    // A stutter on a low sine: every repeat a jump of the read.
    let p = &families()[2].1;
    let x = sine(96_000, 220.0, 0.0);
    let smooth_in = roughness(&x);
    let measure = |xfade_ms: f32| {
        let mut u = unit_with(
            p,
            LapsusSettings {
                xfade_ms,
                ..LapsusSettings::default()
            },
        );
        let (mut l, mut r) = (x.clone(), x.clone());
        drive(&mut u, &mut l, &mut r, 120.0, 0, || 256);
        let y = &l[4_000..];
        let (i, m) = y
            .windows(3)
            .map(|w| (w[2] - 2.0 * w[1] + w[0]).abs())
            .enumerate()
            .fold((0, 0.0f32), |a, (i, v)| if v > a.1 { (i, v) } else { a });
        eprintln!(
            "  xfade {xfade_ms}: worst at {} (cycle position {})",
            i + 4_001,
            (i + 4_001) % 24_000
        );
        m
    };
    let (with, without) = (measure(2.0), measure(0.0));
    eprintln!("joins: second difference {with:.5} with 2 ms, {without:.5} without, the sine's own {smooth_in:.5}");
    assert!(
        with < 4.0 * smooth_in,
        "a 2 ms join stays within four times the sine's own curvature"
    );
    assert!(without > 20.0 * smooth_in, "the measure sees a hard join");
}

#[test]
fn the_interpolator_reads_between_samples_cleanly() {
    let mut h = History::default();
    drop(h.supply(Ring::new(8_192), 0));
    for (hz, tol) in [(1_000.0, 3e-5), (10_000.0, 2e-4), (18_000.0, 2e-3)] {
        let x = sine(8_000, hz, 0.1);
        for (n, v) in x.iter().enumerate() {
            h.write(n as i64, *v, *v);
        }
        let now = 7_999i64;
        let mut worst = 0.0f64;
        for k in 0..400 {
            let pos = 3_000.0 + k as f64 * 0.37;
            let want = crate::math::sin(std::f64::consts::TAU * hz * pos / 48_000.0 + 0.1);
            worst = worst.max((f64::from(h.read(now, now as f64 - pos, 0, 1.0).0) - want).abs());
        }
        eprintln!("sinc at {hz} Hz: worst error {worst:e}");
        assert!(worst < tol, "{hz} Hz: {worst:e}");
    }
}

#[test]
fn read_faster_than_real_time_the_band_above_its_nyquist_is_filtered_first() {
    // A 15 kHz tone read at twice the speed would fold to 18 kHz; the speed's kernel
    // (cut-off ½) takes it away first.
    let mut h = History::default();
    drop(h.supply(Ring::new(16_384), 0));
    let x = sine(16_000, 15_000.0, 0.0);
    for (n, v) in x.iter().enumerate() {
        h.write(n as i64, *v, *v);
    }
    let rms = |speed: f32| {
        let mut e = 0.0f64;
        for k in 0..2_000 {
            let pos = 4_000.0 + 2.0 * k as f64 + 0.5;
            let v = f64::from(h.read(15_999, 15_999.0 - pos, 0, speed).0);
            e += v * v;
        }
        (e / 2_000.0).sqrt()
    };
    let (plain, guarded) = (rms(1.0), rms(2.0));
    let db = 20.0 * crate::math::log10(guarded / plain);
    eprintln!("aliasing at 2×: {db:.1} dB against the plain kernel");
    assert!(db < -60.0, "{db}");
}

#[test]
fn a_slow_tape_stop_changes_kernel_without_a_step() {
    // The delay leaves 0 slowly and crosses 1, 2, 16 and 24 samples: the kernels hand over
    // with no step in a 1 kHz tone.
    let p = pair(4.0, &[bent(0.0, 0.0, TAPE), pt(1.0, 0.5)], &[]);
    let mut u = unit_with(
        &p,
        LapsusSettings {
            xfade_ms: 0.0,
            ..LapsusSettings::default()
        },
    );
    let x = sine(20_000, 1_000.0, 0.0);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut u, &mut l, &mut r, 120.0, 0, || 128);
    let (a, b) = (roughness(&x), roughness(&l));
    assert!(
        b < 1.05 * a,
        "the read stays as smooth as the tone: {b} against {a}"
    );
}

#[test]
fn a_held_note_plays_its_slot_and_its_release_gives_the_slot_back() {
    let mut u = unit(&tape_stop());
    u.set_pattern(Lane::Time, 5, &families()[2].1.time);
    u.set_pattern(Lane::Vol, 7, &families()[5].1.vol);
    let x = music(4_096);
    let (mut l, mut r) = (x.clone(), x.clone());
    u.note(100, Note::On(60 + 5));
    u.note(200, Note::On(24 + 7));
    drive(&mut u, &mut l[..1_000], &mut r[..1_000], 120.0, 0, || 1_000);
    assert_eq!((u.active(Lane::Time), u.active(Lane::Vol)), (5, 7));
    assert!(!u.at_rest());
    u.note(10, Note::Off(60 + 5));
    drive(
        &mut u,
        &mut l[1_000..],
        &mut r[1_000..],
        120.0,
        1_000,
        || 512,
    );
    assert_eq!(
        (u.active(Lane::Time), u.active(Lane::Vol)),
        (1, 7),
        "each bank its own"
    );
    // A note out of the bank's range is not a slot.
    u.note(0, Note::On(60 + 40));
    drive(&mut u, &mut l[..64], &mut r[..64], 120.0, 4_096, || 64);
    assert_eq!(u.active(Lane::Time), 1);
}

#[test]
fn retrigger_starts_the_pattern_on_the_note() {
    // A tape stop (and its fade) retriggered mid-bar starts where the notes land: at the
    // note, the read is live again (a new cycle), a bar later it has stopped.
    let s = LapsusSettings {
        retrig: true,
        notes: NoteMode::Switch,
        ..LapsusSettings::default()
    };
    let mut u = unit_with(&stop_last_beat(), s);
    let x = music(150_000);
    let (mut l, mut r) = (x.clone(), x.clone());
    let at = 30_000usize;
    let mut done = 0;
    while done < x.len() {
        let b = 500.min(x.len() - done);
        if at >= done && at < done + b {
            u.note(at - done, Note::On(61));
            u.note(at - done, Note::On(25));
        }
        u.set_transport(&Transport {
            playing: true,
            tempo_bpm: 120.0,
            samples: done as i64,
        });
        u.process(&mut l[done..done + b], &mut r[done..done + b]);
        done += b;
    }
    // The cycle from the note: 96 000 samples on, the bar's line of the retriggered stop.
    let end = at + 96_000;
    assert!(
        same(&l[end..end + 1_000], &x[end..end + 1_000]),
        "the next cycle starts live on its sample"
    );
    let quiet = l[end - 480..end - 150]
        .iter()
        .fold(0f32, |m, v| m.max(v.abs()));
    assert!(quiet < 0.03, "stopped before it ({quiet})");
}

#[test]
fn it_comes_to_rest_and_a_skip_is_exactly_silence_processed() {
    let p = &families()[5].1; // the gate: a volume that moves through the silence
    let mut a = unit(p);
    let mut b = unit(p);
    let x = music(30_000);
    for u in [&mut a, &mut b] {
        let (mut l, mut r) = (x.clone(), x.clone());
        u.process(&mut l, &mut r);
    }
    assert!(!a.at_rest());
    let mut silent = 0;
    while !a.at_rest() {
        let (mut l, mut r) = (vec![0.0; 512], vec![0.0; 512]);
        a.process(&mut l, &mut r);
        b.process(&mut l.clone(), &mut r.clone());
        silent += 512;
        assert!(silent < 48_000 * 10, "rests within ten seconds of silence");
    }
    assert!(b.at_rest());
    // a processes silence; b skips it: then the same input gives the same bits.
    let (mut l, mut r) = (vec![0.0; 7_777], vec![0.0; 7_777]);
    a.process(&mut l, &mut r);
    assert!(
        l.iter().chain(&r).all(|v| v.to_bits() == 0),
        "silence in, silence out"
    );
    b.skip(7_777);
    let y = music(20_000);
    let (mut al, mut ar, mut bl, mut br) = (y.clone(), y.clone(), y.clone(), y.clone());
    a.process(&mut al, &mut ar);
    b.process(&mut bl, &mut br);
    assert!(same(&al, &bl) && same(&ar, &br));
    eprintln!("at rest after {:.2} s of silence", silent as f32 / SR);
}

#[test]
fn the_history_is_sized_by_the_longest_pattern_at_60_bpm() {
    let mut u = Lapsus::new(SR, shape);
    // Every slot *Through* (a bar): one bar at 60 BPM, four seconds.
    assert_eq!(u.memory_need(), Some(4 * 48_000 + read::MAX_TAPS + 8));
    u.set(&LapsusSettings {
        ahead_ms: 100.0,
        ..LapsusSettings::default()
    });
    assert_eq!(u.latency(), 4_800);
    assert_eq!(
        u.memory_need(),
        Some(4 * 48_000 + 4_800 + read::MAX_TAPS + 8)
    );
    u.ensure_memory();
    assert_eq!(u.memory_need(), None);
}

#[test]
fn lookahead_delays_everything_by_its_latency_and_a_flat_curve_is_then_a_pure_delay() {
    let mut u = Lapsus::new(SR, shape);
    u.set(&LapsusSettings {
        ahead_ms: 10.0,
        ..LapsusSettings::default()
    });
    u.ensure_memory();
    u.reset();
    let x = music(10_000);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut u, &mut l, &mut r, 120.0, 0, || 333);
    assert!(l[..480].iter().all(|v| *v == 0.0));
    assert!(same(&l[480..], &x[..x.len() - 480]));
}

#[test]
fn the_time_and_the_volume_run_on_cycles_of_their_own() {
    // A beat-long stutter under a bar-long fade in: after a beat and a half the time cycle
    // is half through its second turn, the volume three eighths through its first.
    let mut u = Lapsus::new(SR, shape);
    u.set_pattern(
        Lane::Time,
        2,
        &Pattern::new(Lane::Time, 1.0, &[held(0.0, 0.0), held(0.5, 0.5)]),
    );
    u.set_pattern(
        Lane::Vol,
        3,
        &Pattern::new(Lane::Vol, 4.0, &[pt(0.0, 0.0), pt(1.0, 1.0)]),
    );
    u.set(&LapsusSettings {
        tslot: 2,
        vslot: 3,
        ..LapsusSettings::default()
    });
    u.ensure_memory();
    u.reset();
    let x = music(36_000);
    let (mut l, mut r) = (x.clone(), x.clone());
    drive(&mut u, &mut l, &mut r, 120.0, 0, || 500);
    let mut tel = [0.0f32; 64];
    u.telemetry(&mut tel);
    assert!((tel[t::PHASE] - 0.5).abs() < 1e-3, "{}", tel[t::PHASE]);
    assert!((tel[t::VPHASE] - 0.375).abs() < 1e-3, "{}", tel[t::VPHASE]);
    assert_eq!((tel[t::BEATS], tel[t::VBEATS]), (1.0, 4.0));
}
