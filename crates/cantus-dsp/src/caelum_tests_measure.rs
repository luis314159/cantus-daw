//! Caelum measured the way a reverb is measured: decay per octave against the setting,
//! the shimmer's energy and its artefacts, stability at infinite and frozen for five
//! minutes, echo density, the bloom's attack, ducking, the wet's level, and the cost.
//! Every number is printed with `--nocapture`; `docs/round-fx/caelum.md` keeps them.

use super::presets;
use super::tests::{noise, program, render, unit, FS};
use super::*;
use crate::spatium::measure::{band_t60_averaged, echo_density, envelope_db, expected_band_t60, t60};

/// The wet impulse responses (mix 1): L→L, L→R, R→L, R→R.
fn responses(s: &CaelumSettings, secs: f32) -> Vec<Vec<f32>> {
    let s = CaelumSettings { mix: 1.0, ..*s };
    let n = (secs * FS) as usize;
    let mut out = Vec::new();
    for side in 0..2 {
        let mut c = unit(&s);
        let mut l = vec![0.0f32; n];
        let mut r = vec![0.0f32; n];
        if side == 0 {
            l[0] = 1.0;
        } else {
            r[0] = 1.0;
        }
        let (ol, or) = render(&mut c, &l, &r, 512, None, None);
        out.push(ol);
        out.push(or);
    }
    out
}

fn db(x: f64) -> f64 {
    10.0 * math::log10(x.max(1e-30))
}

fn power(x: &[f32]) -> f64 {
    x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64
}

/// RT60 per octave against what the design asks (as an octave band would read it), for
/// small to vast, short to long, bright to dark, 16 and 8 lines.
#[test]
fn the_decay_follows_the_setting_in_every_band() {
    let base = CaelumSettings {
        mod_depth: 0.3,
        diffusion: 0.75,
        predelay_ms: 0.0,
        ..CaelumSettings::default()
    };
    let cases = [
        ("small, 1.5 s", CaelumSettings { size: 0.35, decay_s: 1.5, ..base }),
        (
            "large, 4 s, dark",
            CaelumSettings {
                size: 0.7,
                decay_s: 4.0,
                high_hz: 4_000.0,
                high_decay: 0.4,
                low_hz: 200.0,
                low_decay: 1.5,
                ..base
            },
        ),
        (
            "vast, 10 s, bright",
            CaelumSettings {
                size: 0.95,
                decay_s: 10.0,
                high_hz: 9_000.0,
                high_decay: 0.7,
                low_hz: 150.0,
                low_decay: 0.8,
                ..base
            },
        ),
        (
            "8 lines, 3 s",
            CaelumSettings {
                size: 0.6,
                decay_s: 3.0,
                lines: Lines::Eight,
                ..base
            },
        ),
    ];
    let mut worst = 0.0f32;
    for (name, s) in cases {
        let secs = (s.decay_s * 0.75).max(2.0) + 0.5;
        let irs = responses(&s, secs);
        let refs: Vec<&[f32]> = irs.iter().map(|v| &v[..]).collect();
        let design = unit(&s).design().clone();
        let mut line = String::new();
        for hz in [125.0f32, 250.0, 500.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0] {
            let want = expected_band_t60(|f| design.achieved_t60(f), FS, hz).unwrap();
            let got = band_t60_averaged(&refs, FS, hz).unwrap_or(0.0);
            let err = got / want - 1.0;
            worst = worst.max(err.abs());
            line += &format!(" {hz:.0}: {got:.2}/{want:.2} ({:+.0} %)", err * 100.0);
        }
        eprintln!("{name}:{line}");
    }
    eprintln!("worst band error {:.1} %", worst * 100.0);
    assert!(worst < 0.10, "worst band error {:.1} %", worst * 100.0);
}

/// The output's energy (L + R) every ten seconds of a run, dB.
fn energy_windows(c: &mut Caelum, feed: impl Fn(usize) -> f32, secs: usize) -> Vec<f64> {
    let block = 480usize;
    let win = 10 * FS as usize;
    let mut out = Vec::new();
    let mut acc = 0.0f64;
    let mut count = 0usize;
    let mut l = vec![0.0f32; block];
    let mut r = vec![0.0f32; block];
    let total = secs * FS as usize;
    let mut pos = 0;
    while pos < total {
        for j in 0..block {
            l[j] = feed(pos + j);
            r[j] = feed(pos + j + 7_777_777);
        }
        c.process(&mut l, &mut r, None);
        for j in 0..block {
            acc += (l[j] as f64) * (l[j] as f64) + (r[j] as f64) * (r[j] as f64);
        }
        count += block;
        if count >= win {
            out.push(db(acc / count as f64));
            acc = 0.0;
            count = 0;
        }
        pos += block;
    }
    out
}

fn burst(i: usize, len: usize) -> f32 {
    if i >= len {
        return 0.0;
    }
    let mut s = (i as u32).wrapping_mul(2_654_435_761) | 1;
    0.25 * noise(&mut s)
        + 0.25 * math::sin(i as f32 * 261.63 * std::f32::consts::TAU / FS)
        + 0.15 * math::sin(i as f32 * 392.0 * std::f32::consts::TAU / FS)
}

/// Infinite for five minutes, fed four seconds: without the shimmer it holds (within
/// 1 dB of its first ten seconds after the input); with the shimmer at full — both voices,
/// the octave and the fifth — the tail climbs out through the loop's tone and fades, and in
/// neither case does the energy ever rise above those ten seconds' (by 0.5 dB). Fed noise for five
/// minutes without a pause, it plateaus at the governor's ceiling.
#[test]
fn infinite_does_not_grow_over_five_minutes() {
    let hold = CaelumSettings {
        mix: 1.0,
        shimmer: 0.0,
        ..presets::endless_ascent()
    };
    let climb = CaelumSettings {
        shimmer: 1.0,
        pitch_b_level: 1.0,
        ..hold
    };
    for (name, s) in [("infinite", hold), ("infinite + full shimmer", climb)] {
        let mut c = unit(&s);
        let w = energy_windows(&mut c, |i| burst(i, 4 * FS as usize), 300);
        // The first window holds the swell; the second is the first with only the tail.
        let first = w[1];
        let top = w[1..].iter().cloned().fold(f64::MIN, f64::max);
        let last = *w.last().unwrap();
        let line: Vec<String> = w.iter().map(|d| format!("{:.1}", d - first)).collect();
        eprintln!("{name}, fed 4 s: energy per 10 s against the first (dB): {}", line.join(" "));
        assert!(top - first < 0.5, "{name}: it grew by {:.2} dB", top - first);
        if s.shimmer == 0.0 {
            assert!(last - first > -1.0, "{name} let go: {:.1} dB", last - first);
        }
    }
    let ceiling = db(2.0 * 0.5 * 0.5);
    for (name, s) in [("infinite", hold), ("infinite + full shimmer", climb)] {
        let mut c = unit(&s);
        let w = energy_windows(
            &mut c,
            |i| {
                let mut x = (i as u32).wrapping_mul(747_796_405) | 1;
                0.3 * noise(&mut x)
            },
            300,
        );
        let line: Vec<String> = w.iter().map(|d| format!("{d:.1}")).collect();
        let top = w.iter().cloned().fold(f64::MIN, f64::max);
        let tail = &w[6..];
        let spread = tail.iter().cloned().fold(f64::MIN, f64::max)
            - tail.iter().cloned().fold(f64::MAX, f64::min);
        eprintln!(
            "{name}, fed noise for 5 min: energy per 10 s (dB): {}; top {top:.1}, ceiling {ceiling:.1}, spread after 1 min {spread:.2}",
            line.join(" ")
        );
        assert!(top < ceiling + 1.5, "{name}: past the ceiling: {top:.1} dB");
        assert!(spread < 1.0, "{name}: no plateau: {spread:.2} dB");
    }
}

/// Freeze held for five minutes: the level stays within ±0.5 dB of where it froze.
#[test]
fn freeze_holds_its_level_for_five_minutes() {
    let s = CaelumSettings {
        mix: 1.0,
        ..presets::freeze_swell()
    };
    let mut c = unit(&s);
    let (l, r) = program(3 * FS as usize, 13);
    render(&mut c, &l, &r, 512, None, None);
    c.set(&CaelumSettings { freeze: true, ..s });
    let w = energy_windows(&mut c, |_| 0.0, 300);
    let first = w[0];
    let line: Vec<String> = w.iter().map(|d| format!("{:+.2}", d - first)).collect();
    eprintln!("frozen: energy per 10 s against the first (dB): {}", line.join(" "));
    for d in &w {
        assert!((d - first).abs() < 0.5, "{:.2} dB", d - first);
    }
}

/// The shimmer moves energy and makes none: with the absorption flat (no damping) and the
/// loop's tone open, the tail decays at the same rate whatever the amount — within 10 %.
#[test]
fn the_shimmer_moves_energy_and_makes_none() {
    let flat = CaelumSettings {
        size: 0.6,
        decay_s: 3.0,
        high_decay: 1.0,
        low_decay: 1.0,
        shimmer_low_hz: 20.0,
        shimmer_high_hz: 20_000.0,
        predelay_ms: 0.0,
        ..CaelumSettings::default()
    };
    let rt = |s: &CaelumSettings| {
        let irs = responses(s, 3.0);
        let sum: Vec<f32> = (0..irs[0].len())
            .map(|i| irs.iter().map(|v| v[i] * v[i]).sum::<f32>().sqrt())
            .collect();
        t60(&sum, FS).unwrap()
    };
    let none = rt(&flat);
    let mut line = format!("no shimmer {none:.2} s");
    for (amount, b) in [(0.5f32, 0.0f32), (1.0, 0.0), (1.0, 1.0)] {
        let got = rt(&CaelumSettings {
            shimmer: amount,
            pitch_b_level: b,
            ..flat
        });
        line += &format!("; amount {amount} (voice B {b}): {got:.2} s");
        assert!((got / none - 1.0).abs() < 0.10, "{line}");
    }
    eprintln!("broadband RT60 with the absorption flat: {line}");
}

/// The power spectrum of `x` (Hann, `n` points from `start`), bin width `FS / n`.
fn spectrum(x: &[f32], start: usize, n: usize) -> Vec<f64> {
    let mut re: Vec<f32> = (0..n)
        .map(|i| {
            let w = 0.5 - 0.5 * math::cos(std::f32::consts::TAU * i as f32 / n as f32);
            x.get(start + i).copied().unwrap_or(0.0) * w
        })
        .collect();
    let mut im = vec![0.0f32; n];
    crate::spectrum::fft(&mut re, &mut im);
    (0..n / 2)
        .map(|k| (re[k] as f64) * (re[k] as f64) + (im[k] as f64) * (im[k] as f64))
        .collect()
}

/// A tone through the shimmer: a second of 440 Hz, then the tail a second later, its power
/// sorted into the partials the shimmer should make (440 Hz times the voices' ratios and
/// their cascades, ±30 cents) and the rest — the artefacts in the loop: the heads' hand-over
/// sidebands, the interpolation's images, the modulation's spread.
#[test]
fn the_shimmer_moves_a_tail_by_its_interval() {
    let n = 65_536;
    let tone: Vec<f32> = (0..3 * FS as usize)
        .map(|i| {
            if i < FS as usize {
                0.3 * math::sin(std::f32::consts::TAU * 440.0 * i as f32 / FS)
            } else {
                0.0
            }
        })
        .collect();
    let base = CaelumSettings {
        mix: 1.0,
        size: 0.6,
        decay_s: 6.0,
        predelay_ms: 0.0,
        shimmer_low_hz: 20.0,
        shimmer_high_hz: 20_000.0,
        ..CaelumSettings::default()
    };
    for (name, s, ratios) in [
        ("off", base, vec![1.0f32]),
        ("octave", CaelumSettings { shimmer: 0.7, pitch_a: 12.0, ..base }, vec![1.0, 2.0, 4.0, 8.0]),
        (
            "octave and fifth",
            CaelumSettings {
                shimmer: 0.7,
                pitch_a: 12.0,
                pitch_b: 7.0,
                pitch_b_level: 1.0,
                ..base
            },
            {
                let mut v = Vec::new();
                for a in 0..4 {
                    for b in 0..4 {
                        v.push(math::powf(2f32, a as f32) * math::powf(1.498_307f32, b as f32));
                    }
                }
                v
            },
        ),
        ("octave down", CaelumSettings { shimmer: 0.6, pitch_a: -12.0, ..base }, vec![1.0, 0.5, 0.25]),
    ] {
        let mut c = unit(&s);
        let (l, r) = render(&mut c, &tone, &tone, 512, None, None);
        let pl = spectrum(&l, 2 * FS as usize - n / 2, n);
        let pr = spectrum(&r, 2 * FS as usize - n / 2, n);
        let bin = FS / n as f32;
        let (mut on, mut total) = (0.0f64, 0.0f64);
        let mut up = 0.0f64;
        for k in 1..n / 2 {
            let hz = k as f32 * bin;
            if !(60.0..=12_000.0).contains(&hz) {
                continue;
            }
            let p = pl[k] + pr[k];
            total += p;
            if hz > 600.0 {
                up += p;
            }
            if ratios
                .iter()
                .any(|q| (1200.0 * math::log2(hz / (440.0 * q))).abs() <= 30.0)
            {
                on += p;
            }
        }
        let share = on / total;
        eprintln!(
            "{name}: {:.1} % of the tail on its partials (artefacts {:.1} dB under them); {:.1} % above 600 Hz",
            share * 100.0,
            db((total - on) / on),
            up / total * 100.0
        );
        // A shift down reads slowly: its heads hand over half as often, and their
        // sidebands land wider of the partials.
        let floor = if name == "octave down" { 0.75 } else { 0.9 };
        assert!(share > floor, "{name}: {:.1} %", share * 100.0);
        if name == "octave" || name == "octave and fifth" {
            assert!(up / total > 0.3, "{name}: the tail did not climb");
        }
        if name == "off" {
            assert!(up / total < 0.02);
        }
    }
}

/// When the tail turns to noise: the first 20 ms window whose normalised echo density
/// (Abel & Huang) reaches 0.9, ms after the impulse.
fn dense_after(s: &CaelumSettings) -> f32 {
    let irs = responses(&CaelumSettings { predelay_ms: 0.0, ..*s }, 1.5);
    let ed = echo_density(&irs[0], FS, 20.0);
    ed.iter()
        .position(|d| *d >= 0.9)
        .map(|k| k as f32 * 20.0)
        .unwrap_or(f32::INFINITY)
}

/// Echo density: a small space is dense within 40 ms, a large one within 150 ms, the
/// largest (lines of 210 to 480 ms) within 300 ms — the two all-passes in every line and
/// the early taps are what make a vast tank dense; without diffusion it takes a second.
#[test]
fn the_tail_turns_dense_quickly() {
    let base = CaelumSettings {
        decay_s: 6.0,
        ..CaelumSettings::default()
    };
    let small = dense_after(&CaelumSettings { size: 0.3, ..base });
    let large = dense_after(&CaelumSettings { size: 0.7, ..base });
    let vast = dense_after(&CaelumSettings { size: 1.0, ..base });
    let vast_dry = dense_after(&CaelumSettings {
        size: 1.0,
        diffusion: 0.0,
        ..base
    });
    let eight = dense_after(&CaelumSettings {
        size: 0.7,
        lines: Lines::Eight,
        ..base
    });
    eprintln!(
        "dense (echo density 0.9) after: small {small} ms, large {large} ms, vast {vast} ms, vast without diffusion {vast_dry} ms, 8 lines large {eight} ms"
    );
    assert!(small <= 40.0);
    assert!(large <= 150.0);
    assert!(vast <= 300.0);
    assert!(eight <= 200.0);
    assert!(vast_dry >= vast);
}

/// Bloom: the wet's envelope (after a 60 ms burst) peaks within 150 ms without it and only
/// after more than half the bloom time with it — the swell is the setting.
#[test]
fn bloom_is_a_slow_attack() {
    let n = 5 * FS as usize;
    let burst_in: Vec<f32> = (0..n)
        .map(|i| {
            if i < (0.06 * FS) as usize {
                let mut s = (i as u32).wrapping_mul(2_654_435_761) | 1;
                0.5 * noise(&mut s)
            } else {
                0.0
            }
        })
        .collect();
    let mut line = String::new();
    for bloom in [0.0f32, 500.0, 1_500.0, 3_000.0] {
        let s = CaelumSettings {
            mix: 1.0,
            size: 0.6,
            decay_s: 8.0,
            predelay_ms: 0.0,
            bloom_ms: bloom,
            ..CaelumSettings::default()
        };
        let mut c = unit(&s);
        let (l, r) = render(&mut c, &burst_in, &burst_in, 512, None, None);
        let sum: Vec<f32> = l.iter().zip(&r).map(|(a, b)| a + b).collect();
        let env = envelope_db(&sum, FS, 20.0);
        let (k, _) = env
            .iter()
            .enumerate()
            .fold((0, f32::MIN), |m, (k, d)| if *d > m.1 { (k, *d) } else { m });
        let peak_ms = k as f32 * 20.0;
        // The level 100 ms in, against the peak: how gently it starts.
        let early = env[5] - env[k];
        line += &format!(" bloom {bloom} ms: peak at {peak_ms} ms, {early:+.1} dB at 100 ms;");
        if bloom == 0.0 {
            assert!(peak_ms <= 150.0, "{line}");
        } else {
            assert!(peak_ms >= 0.5 * bloom, "{line}");
        }
    }
    eprintln!("the wet's envelope after a burst:{line}");
}

/// Ducking: while the dry plays at −10 dBFS the wet sits 12 dB down (the range), and once
/// the dry stops it comes back at the release's speed (350 ms): within 1 dB of the
/// unducked wet from 1.2 s after.
#[test]
fn ducking_steps_aside_and_comes_back() {
    let n = 4 * FS as usize;
    let tone: Vec<f32> = (0..n)
        .map(|i| {
            if i < 2 * FS as usize {
                0.316 * math::sin(std::f32::consts::TAU * 330.0 * i as f32 / FS)
            } else {
                0.0
            }
        })
        .collect();
    let s = CaelumSettings {
        mix: 1.0,
        ..presets::ducked_vocal()
    };
    let run = |s: &CaelumSettings| {
        let mut c = unit(s);
        render(&mut c, &tone, &tone, 512, None, None)
    };
    let ducked = run(&s);
    let open = run(&CaelumSettings { duck_db: 0.0, ..s });
    let at = |o: &(Vec<f32>, Vec<f32>), a: f32, b: f32| {
        let (i, j) = ((a * FS) as usize, (b * FS) as usize);
        power(&o.0[i..j]) + power(&o.1[i..j])
    };
    let during = db(at(&ducked, 1.0, 2.0) / at(&open, 1.0, 2.0));
    let half = db(at(&ducked, 2.4, 2.6) / at(&open, 2.4, 2.6));
    let after = db(at(&ducked, 3.2, 3.8) / at(&open, 3.2, 3.8));
    eprintln!(
        "ducking 12 dB: {during:+.1} dB while the dry plays, {half:+.1} dB 0.4–0.6 s after it stops, {after:+.1} dB 1.2–1.8 s after"
    );
    assert!((during + 12.0).abs() < 1.0, "{during}");
    assert!(after > -1.0, "{after}");
}

/// The wet's level on pink-ish noise follows `(T / 2 s)^¼` of the input's (within 1.5 dB),
/// across sizes and decays.
#[test]
fn the_wet_level_follows_the_decay() {
    let n = 12 * FS as usize;
    let mut seed = 99u32;
    let mut lp = [0.0f32; 2];
    let mut sig = [vec![0.0f32; n], vec![0.0f32; n]];
    for i in 0..n {
        for c in 0..2 {
            lp[c] += 0.2 * (noise(&mut seed) - lp[c]);
            sig[c][i] = lp[c];
        }
    }
    let mut worst = 0.0f64;
    let mut line = String::new();
    for (size, t) in [(0.3f32, 1.0f32), (0.6, 4.0), (0.9, 16.0), (0.5, 2.0)] {
        let s = CaelumSettings {
            mix: 1.0,
            size,
            decay_s: t,
            high_decay: 1.0,
            ..CaelumSettings::default()
        };
        let mut c = unit(&s);
        let (l, r) = render(&mut c, &sig[0], &sig[1], 512, None, None);
        let from = 8 * FS as usize;
        let got = db((power(&l[from..]) + power(&r[from..])) / (power(&sig[0][from..]) + power(&sig[1][from..])));
        let want = 10.0 * math::log10(math::powf(t.min(30.0) as f64 / 2.0, 0.25));
        worst = worst.max((got - want).abs());
        line += &format!(" size {size}, {t} s: {got:+.2} dB for {want:+.2};");
    }
    eprintln!("the wet's level against the input's:{line} worst {worst:.2} dB");
    assert!(worst < 1.5);
}

/// The cost: a minute of program through the starting preset, 16 and 8 lines, the shimmer
/// on and off, as a share of one core in real time. Run alone (`--ignored`) on a quiet
/// machine; the numbers go in the round's document.
#[test]
#[ignore]
fn cpu_at_eight_and_sixteen_lines() {
    let (l, r) = program(60 * FS as usize, 3);
    for (name, s) in [
        ("16 lines, shimmer two voices", CaelumSettings { pitch_b_level: 1.0, ..presets::starting() }),
        ("16 lines, shimmer one voice", presets::starting()),
        ("16 lines, no shimmer", CaelumSettings { shimmer: 0.0, ..presets::starting() }),
        ("8 lines, shimmer one voice", CaelumSettings { lines: Lines::Eight, ..presets::starting() }),
        ("8 lines, no shimmer", CaelumSettings { lines: Lines::Eight, shimmer: 0.0, ..presets::starting() }),
        ("16 lines, bloom and shimmer", CaelumSettings { bloom_ms: 1500.0, ..presets::starting() }),
    ] {
        let mut best = f64::MAX;
        for _ in 0..3 {
            let mut c = unit(&s);
            let t0 = std::time::Instant::now();
            let out = render(&mut c, &l, &r, 256, None, None);
            let dt = t0.elapsed().as_secs_f64();
            std::hint::black_box(out);
            best = best.min(dt);
        }
        eprintln!("{name}: {:.2} % of a core", best / 60.0 * 100.0);
    }
}
