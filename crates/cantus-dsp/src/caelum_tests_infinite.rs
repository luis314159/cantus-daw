//! Infinite measured (ADR 0138's amendment, 2026-11-17): engaging it moves no level and
//! clicks nothing; the wash holds within half a decibel for five minutes at Sustain 100 %,
//! fed or not; its tone (each octave's level) holds as long; Sustain 0 % fades as the old
//! *Fades* did, and the sustain between is a continuous fade; new input layers under a
//! soft ceiling that holds under minutes of loud noise. Every number is printed with
//! `--nocapture`; `docs/round-fx/caelum.md` keeps them.

use super::presets;
use super::tests::{noise, render, same, unit, FS};
use super::*;

fn db(x: f64) -> f64 {
    10.0 * math::log10(x.max(1e-30))
}

/// The power of both sides over samples `a..b`.
fn pw(l: &[f32], r: &[f32], a: usize, b: usize) -> f64 {
    let b = b.min(l.len());
    let mut acc = 0.0f64;
    for j in a..b {
        acc += (l[j] as f64) * (l[j] as f64) + (r[j] as f64) * (r[j] as f64);
    }
    acc / (b - a).max(1) as f64
}

fn secs(t: f32) -> usize {
    (t * FS) as usize
}

/// A note-like input: noise and two tones, 0.25 each.
fn tone_noise(i: usize, seed: u32) -> (f32, f32) {
    let mut s = (i as u32).wrapping_mul(2_654_435_761) ^ seed | 1;
    let t = i as f32 / FS;
    let a = 0.25 * noise(&mut s);
    let b = 0.25 * noise(&mut s);
    (
        a + 0.25 * math::sin(t * 261.63 * std::f32::consts::TAU),
        b + 0.2 * math::sin(t * 392.0 * std::f32::consts::TAU),
    )
}

/// `n` samples of noise (0.3 a side) for `on` of them, then silence.
fn noisy(n: usize, on: usize) -> (Vec<f32>, Vec<f32>) {
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for i in 0..on.min(n) {
        let mut x = (i as u32).wrapping_mul(2_654_435_761) | 1;
        l[i] = 0.3 * noise(&mut x);
        r[i] = 0.3 * noise(&mut x);
    }
    (l, r)
}

/// A long run without keeping it all: the level (both sides) of every `win` samples, and
/// the samples of a few stretches (`(start, len)`) kept for their spectra. The input is
/// `feed(i)`; `change` is set at its sample.
struct Run {
    levels: Vec<f64>,
    kept: Vec<(Vec<f32>, Vec<f32>)>,
}

fn run(
    c: &mut Caelum,
    total: usize,
    feed: &dyn Fn(usize) -> (f32, f32),
    win: usize,
    keep: &[(usize, usize)],
    change: Option<(usize, &CaelumSettings)>,
) -> Run {
    let block = 480usize;
    let mut out = Run {
        levels: Vec::new(),
        kept: keep
            .iter()
            .map(|(_, n)| (Vec::with_capacity(*n), Vec::with_capacity(*n)))
            .collect(),
    };
    let (mut acc, mut count) = (0.0f64, 0usize);
    let mut l = vec![0.0f32; block];
    let mut r = vec![0.0f32; block];
    let mut pos = 0;
    while pos < total {
        if let Some((at, s)) = change {
            if pos == at {
                c.set(s);
            }
        }
        for j in 0..block {
            (l[j], r[j]) = feed(pos + j);
        }
        c.process(&mut l, &mut r, None);
        for j in 0..block {
            let i = pos + j;
            acc += (l[j] as f64) * (l[j] as f64) + (r[j] as f64) * (r[j] as f64);
            count += 1;
            if count == win {
                out.levels.push(db(acc / win as f64));
                acc = 0.0;
                count = 0;
            }
            for (k, (a, n)) in keep.iter().enumerate() {
                if i >= *a && i < a + n {
                    out.kept[k].0.push(l[j]);
                    out.kept[k].1.push(r[j]);
                }
            }
        }
        pos += block;
    }
    out
}

/// The octave bands' levels (125 Hz … 8 kHz) of a stretch, dB: Hann frames of 8192
/// points half overlapped, both sides, each band `fc/√2 … fc·√2`.
const BANDS: [f32; 7] = [125.0, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0];

fn octaves(l: &[f32], r: &[f32]) -> [f64; 7] {
    let n = 8192usize;
    let mut acc = [0.0f64; 7];
    for x in [l, r] {
        let mut start = 0;
        while start + n <= x.len() {
            let mut re: Vec<f32> = (0..n)
                .map(|i| {
                    let w = 0.5 - 0.5 * math::cos(std::f32::consts::TAU * i as f32 / n as f32);
                    x[start + i] * w
                })
                .collect();
            let mut im = vec![0.0f32; n];
            crate::spectrum::fft(&mut re, &mut im);
            for k in 1..n / 2 {
                let hz = k as f32 * FS / n as f32;
                let p = (re[k] as f64) * (re[k] as f64) + (im[k] as f64) * (im[k] as f64);
                for (b, fc) in BANDS.iter().enumerate() {
                    if hz >= fc / std::f32::consts::SQRT_2 && hz < fc * std::f32::consts::SQRT_2 {
                        acc[b] += p;
                    }
                }
            }
            start += n / 2;
        }
    }
    acc.map(db)
}

/// The worst shimmer: both voices (the octave and the fifth) at the full amount.
fn climbing(s: CaelumSettings) -> CaelumSettings {
    CaelumSettings {
        mix: 1.0,
        shimmer: 1.0,
        pitch_b_level: 1.0,
        ..s
    }
}

/// Engaging Infinite moves no level and clicks nothing: on a tail that is dying away and
/// while the input still plays, small to vast, 16 and 8 lines, with the shimmer. The output
/// is the finite twin's up to the switch, and no sample-to-sample step after it is larger
/// than the tail's own (no click); on a dying tail the wash then holds at the level it
/// had at the switch (±50 ms about it), measured over the next two and a half seconds; fed,
/// it stays the finite twin's (the soft ceiling is the finite tail's level).
#[test]
fn engaging_infinite_moves_no_level_and_clicks_nothing() {
    let cases = [
        ("octave shimmer", presets::octave_shimmer()),
        (
            "vast, 1.5 s",
            CaelumSettings {
                size: 0.95,
                decay_s: 1.5,
                shimmer: 0.5,
                ..presets::octave_shimmer()
            },
        ),
        ("glass plate", presets::glass_plate()),
        ("8 lines (light)", presets::light_shimmer()),
        ("black sky (40 s)", presets::black_sky()),
    ];
    let mut worst = 0.0f64;
    for (name, s) in cases {
        let s = CaelumSettings { mix: 1.0, ..s };
        let held = CaelumSettings {
            infinite: true,
            ..s
        };
        // On a dying tail: two seconds of noise, the switch a quarter second after (noise,
        // so that the level at the switch is not a held tone's beating).
        let t0 = secs(2.25);
        let (l, r) = noisy(secs(5.0), secs(2.0));
        let a = render(&mut unit(&s), &l, &r, 512, None, None);
        let b = render(&mut unit(&s), &l, &r, 512, None, Some((t0, &held)));
        // No click: up to the switch the two are one, and the largest step from a sample
        // to the next in the 5 ms after it is no larger than the finite tail's own around
        // it (±50 ms).
        assert!(
            same(&a.0[..t0], &b.0[..t0]) && same(&a.1[..t0], &b.1[..t0]),
            "{name}: before the switch"
        );
        let jump = |x: &[f32], from: usize, to: usize| {
            (from..to).fold(0.0f32, |m, j| m.max((x[j] - x[j - 1]).abs()))
        };
        let own = jump(&a.0, t0 - secs(0.05), t0 + secs(0.05)).max(jump(
            &a.1,
            t0 - secs(0.05),
            t0 + secs(0.05),
        ));
        let at_switch = jump(&b.0, t0, t0 + secs(0.005)).max(jump(&b.1, t0, t0 + secs(0.005)));
        assert!(
            at_switch <= own,
            "{name}: a click at the switch ({at_switch} against the tail's own {own})"
        );
        // The level at the switch: the finite tail's, the 50 ms before and after it
        // averaged in decibels (a dying tail's level at its middle).
        let at = 0.5
            * (db(pw(&a.0, &a.1, t0 - secs(0.05), t0)) + db(pw(&a.0, &a.1, t0, t0 + secs(0.05))));
        let hold = db(pw(&b.0, &b.1, t0 + secs(0.5), t0 + secs(2.75))) - at;
        let wins: Vec<f64> = (0..25)
            .map(|k| {
                let a0 = t0 + k * secs(0.1);
                db(pw(&b.0, &b.1, a0, a0 + secs(0.1))) - at
            })
            .collect();
        let lo = wins.iter().cloned().fold(f64::MAX, f64::min);
        let hi = wins.iter().cloned().fold(f64::MIN, f64::max);
        let fell = db(pw(&a.0, &a.1, t0 + secs(0.5), t0 + secs(2.75))) - at;
        eprintln!(
            "{name}, dying tail: held {hold:+.2} dB against the switch's level (100 ms windows {lo:+.2} … {hi:+.2}); finite {fell:+.1} dB"
        );
        // A vast tank with a short decay loses 14 dB a pass: half a second after its input
        // it is still the input's echoes, not yet a tail, and what "the level at the
        // engage" is spreads over a pass — held within 2 dB of it; every other within 0.5.
        let bound = if name.starts_with("vast") { 2.0 } else { 0.5 };
        assert!(hold.abs() <= bound, "{name}: held {hold:+.2} dB");
        if bound <= 0.5 {
            worst = worst.max(hold.abs());
        }
        // While the input plays: twelve seconds of noise and tones, the switch at eight.
        let t0 = secs(8.0);
        let (l, r) = noisy(secs(12.0), secs(12.0));
        let a = render(&mut unit(&s), &l, &r, 512, None, None);
        let b = render(&mut unit(&s), &l, &r, 512, None, Some((t0, &held)));
        assert!(
            same(&a.0[..t0 + 64], &b.0[..t0 + 64]),
            "{name}: fed, the switch changed what is heard at once"
        );
        let own = jump(&a.0, t0 - secs(0.05), t0 + secs(0.05));
        assert!(
            jump(&b.0, t0, t0 + secs(0.005)) <= own,
            "{name}: fed, a click"
        );
        let mut line = String::new();
        let mut far = 0.0f64;
        for k in 0..4 {
            let a0 = t0 + k * secs(1.0);
            let d = db(pw(&b.0, &b.1, a0, a0 + secs(1.0))) - db(pw(&a.0, &a.1, a0, a0 + secs(1.0)));
            line += &format!(" {d:+.2}");
            far = far.max(d.abs());
        }
        eprintln!("{name}, fed: each second after the switch against the finite twin (dB):{line}");
        assert!(far <= 0.5, "{name}: fed, {far:.2} dB from the finite twin");
        worst = worst.max(far);
    }
    eprintln!("engaging infinite: worst {worst:.2} dB");
}

/// Five minutes at Sustain 100 % (the default), the octave and the fifth at the full
/// amount and without the shimmer: **without input** — noise for four and a half seconds,
/// then nothing — every three seconds (the short-term window) of the wash within half a
/// decibel of the three as the input stopped;
/// **with input** — noise and tones without a pause, Infinite engaged on the finite tail
/// after thirty seconds — every ten seconds within half a decibel of the ten before.
#[test]
fn infinite_holds_five_minutes_fed_or_not() {
    let plain = CaelumSettings {
        mix: 1.0,
        shimmer: 0.0,
        sustain: 1.0,
        ..presets::endless_ascent()
    };
    let both = CaelumSettings {
        sustain: 1.0,
        ..climbing(presets::endless_ascent())
    };
    for (name, s) in [("plain", plain), ("octave and fifth", both)] {
        let mut c = unit(&s);
        let on = secs(4.5);
        let feed = |i: usize| {
            if i < on {
                let mut x = (i as u32).wrapping_mul(2_654_435_761) | 1;
                (0.3 * noise(&mut x), 0.3 * noise(&mut x))
            } else {
                (0.0, 0.0)
            }
        };
        let w = run(&mut c, secs(306.0), &feed, secs(3.0), &[], None).levels;
        // The input stops at 4.5 s: the three seconds from 6 s are the level it stopped at.
        let at = w[2];
        let after = &w[3..];
        let lo = after.iter().cloned().fold(f64::MAX, f64::min) - at;
        let hi = after.iter().cloned().fold(f64::MIN, f64::max) - at;
        let minutes: Vec<String> = (0..=5)
            .map(|m| format!("{:+.2}", w[(2 + 20 * m).min(w.len() - 1)] - at))
            .collect();
        eprintln!(
            "{name}, fed noise 4.5 s: every three seconds for five minutes {lo:+.2} … {hi:+.2} dB of the three after it stopped (each minute: {})",
            minutes.join(" ")
        );
        assert!(lo >= -0.5 && hi <= 0.5, "{name}: {lo:+.2} … {hi:+.2} dB");
    }
    let octave = CaelumSettings {
        mix: 1.0,
        ..presets::octave_shimmer()
    };
    let wide = CaelumSettings {
        infinite: false,
        ..both
    };
    for (name, s) in [("octave shimmer", octave), ("octave and fifth, 20 s", wide)] {
        let mut c = unit(&s);
        let held = CaelumSettings {
            infinite: true,
            ..s
        };
        let t0 = secs(30.0);
        let feed = |i: usize| {
            let mut x = (i as u32).wrapping_mul(2_654_435_761) | 1;
            (0.3 * noise(&mut x), 0.3 * noise(&mut x))
        };
        let w = run(
            &mut c,
            t0 + secs(300.0),
            &feed,
            secs(10.0),
            &[],
            Some((t0, &held)),
        )
        .levels;
        let before = w[2];
        let after = &w[3..];
        let lo = after.iter().cloned().fold(f64::MAX, f64::min) - before;
        let hi = after.iter().cloned().fold(f64::MIN, f64::max) - before;
        let line: Vec<String> = after
            .iter()
            .map(|d| format!("{:+.2}", d - before))
            .collect();
        eprintln!(
            "{name}, fed noise, engaged at 30 s: every 10 s for five minutes {lo:+.2} … {hi:+.2} dB of the 10 s before ({})",
            line.join(" ")
        );
        assert!(lo >= -0.5 && hi <= 0.5, "{name}: {lo:+.2} … {hi:+.2} dB");
        // Noise and two tones: the finite tail itself wavers by a decibel from one ten
        // seconds to the next (held tones beat in the shimmer's loop) and the ceiling's
        // measure of it with them, so thirty seconds are compared, the half minute before
        // the switch with each after it (measured +0.3 … +1.1 dB: the ceiling rides the
        // estimate's peaks).
        let mut c = unit(&s);
        let tones = |i: usize| tone_noise(i, 5);
        let w = run(
            &mut c,
            t0 + secs(300.0),
            &tones,
            secs(30.0),
            &[],
            Some((t0, &held)),
        )
        .levels;
        let line: Vec<String> = w[1..].iter().map(|d| format!("{:+.2}", d - w[0])).collect();
        let far = w[1..].iter().fold(0.0f64, |m, d| m.max((d - w[0]).abs()));
        eprintln!(
            "{name}, fed noise and two tones: each 30 s after the switch against the 30 before ({})",
            line.join(" ")
        );
        assert!(far <= 1.5, "{name}, with tones: {far:.2} dB");
    }
}

/// The held wash keeps its tone: Sustain 100 %, both voices at the full amount, fed four
/// and a half seconds of noise — each octave (125 Hz … 8 kHz) of four-second stretches at a
/// minute, two, …, ten, against the stretch from 10 s: within half a decibel for five
/// minutes, and within a decibel in ten (no octave climbing away: it never turns shrill).
/// Fed noise and two tones, printed too.
#[test]
fn the_held_wash_keeps_its_tone() {
    let s = CaelumSettings {
        sustain: 1.0,
        ..climbing(presets::endless_ascent())
    };
    let len = secs(4.0);
    let at: Vec<(usize, usize)> = (0..=10)
        .map(|m| {
            if m == 0 {
                (secs(10.0), len)
            } else {
                (secs(60.0 * m as f32), len)
            }
        })
        .collect();
    let row = |o: &[f64]| {
        o.iter()
            .map(|d| format!("{d:+.2}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let on = secs(4.5);
    let noisy = |i: usize| {
        if i < on {
            let mut x = (i as u32).wrapping_mul(2_654_435_761) | 1;
            (0.3 * noise(&mut x), 0.3 * noise(&mut x))
        } else {
            (0.0, 0.0)
        }
    };
    let tonal = |i: usize| if i < on { tone_noise(i, 9) } else { (0.0, 0.0) };
    let mut worst = [[0.0f64; 2]; 2];
    for (k, (name, feed)) in [
        ("noise", &noisy as &dyn Fn(usize) -> (f32, f32)),
        ("noise and two tones", &tonal),
    ]
    .into_iter()
    .enumerate()
    {
        let kept = run(&mut unit(&s), secs(605.0), feed, secs(10.0), &at, None).kept;
        let first = octaves(&kept[0].0, &kept[0].1);
        eprintln!(
            "{name}: the held wash at 10 s, by octave (dB): {}",
            row(&first)
        );
        for (m, (l, r)) in kept.iter().enumerate().skip(1) {
            let o = octaves(l, r);
            let d: Vec<f64> = o.iter().zip(&first).map(|(a, b)| a - b).collect();
            let w = d.iter().fold(0.0f64, |m, x| m.max(x.abs()));
            if m <= 5 {
                worst[k][0] = worst[k][0].max(w);
            }
            worst[k][1] = worst[k][1].max(w);
            eprintln!(
                "{name}, minute {m}: each octave against 10 s (dB): {}",
                row(&d)
            );
        }
        eprintln!(
            "{name}: worst octave {:.2} dB in five minutes, {:.2} dB in ten",
            worst[k][0], worst[k][1]
        );
    }
    assert!(
        worst[0][0] <= 0.5,
        "an octave moved {:.2} dB in five minutes",
        worst[0][0]
    );
    assert!(
        worst[0][1] <= 1.0,
        "an octave moved {:.2} dB in ten minutes",
        worst[0][1]
    );
    // Two held tones beat against their own modes (slow interference in a tank that loses
    // nothing): each octave of four seconds wavers, but nothing climbs away.
    assert!(
        worst[1][1] <= 2.0,
        "with tones, an octave moved {:.2} dB",
        worst[1][1]
    );
}

/// Sustain 0 % is the old *Fades*: the climb carries the wash out through the loop's tone
/// (the old: −27.3 dB at ten seconds, −120.6 at a minute, each ten seconds against the
/// first ten of the tail); and the sustain between is a continuous fade, slower the higher.
#[test]
fn sustain_zero_fades_and_the_sustain_between_is_continuous() {
    let base = climbing(presets::endless_ascent());
    let on = secs(4.0);
    let feed = |i: usize| {
        if i < on {
            tone_noise(i, 13)
        } else {
            (0.0, 0.0)
        }
    };
    let mut at_minute = Vec::new();
    for sus in [0.0f32, 0.25, 0.5, 0.75, 0.9, 1.0] {
        let mut c = unit(&CaelumSettings {
            sustain: sus,
            ..base
        });
        let w = run(&mut c, secs(70.0), &feed, secs(10.0), &[], None).levels;
        let line: Vec<String> = w.iter().map(|d| format!("{:+.1}", d - w[1])).collect();
        eprintln!(
            "sustain {:.0} %: each 10 s against the first of the tail (dB): {}",
            sus * 100.0,
            line.join(" ")
        );
        if sus == 0.0 {
            assert!(w[2] - w[1] < -15.0, "sustain 0 fades: {:.1}", w[2] - w[1]);
            assert!(
                w[6] - w[1] < -60.0,
                "sustain 0 fades out: {:.1}",
                w[6] - w[1]
            );
        }
        at_minute.push(w[6] - w[1]);
    }
    for k in 1..at_minute.len() {
        assert!(
            at_minute[k] > at_minute[k - 1],
            "not continuous: {at_minute:?}"
        );
    }
}

/// The ceiling holds: noise without a pause (0.3 RMS a side) for five minutes into an
/// infinite tank, without and with the full shimmer — it plateaus where the same tank
/// finite does (the soft ceiling is a finite tail's level), within a decibel, flat over the
/// last two minutes, under the governor's guard; louder still (0.7 RMS), under the guard.
#[test]
fn the_ceiling_holds_under_loud_input() {
    let guard = db(2.0 * 0.5 * 0.5);
    let loud = |a: f32| {
        move |i: usize| {
            let mut x = (i as u32).wrapping_mul(747_796_405) | 1;
            let l = a * 1.732 * noise(&mut x);
            let r = a * 1.732 * noise(&mut x);
            (l, r)
        }
    };
    let plain = CaelumSettings {
        mix: 1.0,
        shimmer: 0.0,
        ..presets::endless_ascent()
    };
    for (name, s) in [
        ("plain", plain),
        ("octave and fifth", climbing(presets::endless_ascent())),
    ] {
        let f = loud(0.3);
        let w = run(&mut unit(&s), secs(300.0), &f, secs(10.0), &[], None).levels;
        let finite = CaelumSettings {
            infinite: false,
            ..s
        };
        let fin = run(&mut unit(&finite), secs(120.0), &f, secs(10.0), &[], None).levels;
        let tail = &w[18..];
        let spread = tail.iter().cloned().fold(f64::MIN, f64::max)
            - tail.iter().cloned().fold(f64::MAX, f64::min);
        let top = w.iter().cloned().fold(f64::MIN, f64::max);
        let plateau = tail.iter().sum::<f64>() / tail.len() as f64;
        let fin_level = fin[6..].iter().sum::<f64>() / (fin.len() - 6) as f64;
        let line: Vec<String> = w.iter().map(|d| format!("{d:.1}")).collect();
        eprintln!(
            "{name}, noise for five minutes: each 10 s (dB) {}; plateau {plateau:.2}, the finite tank's {fin_level:.2}, spread over the last two minutes {spread:.2}, top {top:.2}, guard {guard:.1}",
            line.join(" ")
        );
        assert!(spread < 0.5, "{name}: no plateau ({spread:.2} dB)");
        assert!(
            (plateau - fin_level).abs() < 1.0,
            "{name}: plateau {plateau:.2} against {fin_level:.2}"
        );
        assert!(top < guard + 0.5, "{name}: past the guard ({top:.2})");
        let f = loud(0.7);
        let w = run(&mut unit(&s), secs(120.0), &f, secs(10.0), &[], None).levels;
        let top = w.iter().cloned().fold(f64::MIN, f64::max);
        eprintln!("{name}, louder noise: top {top:.2} dB");
        assert!(
            top < guard + 0.5,
            "{name}: louder, past the guard ({top:.2})"
        );
    }
}
