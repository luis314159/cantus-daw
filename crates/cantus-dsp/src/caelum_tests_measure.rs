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
