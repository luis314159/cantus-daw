//! Lenis measured: a resonance cut and its neighbours kept, flat noise left alone, the
//! reduction against depth and sharpness, the time response, the window pairs compared,
//! and the CPU (`#[ignore]`: run alone, in release).
//!
//! `cargo test -p cantus-dsp --lib lenis::tests_measure -- --nocapture`

use super::tests::{peaking, pink, run, unit, white, Stereo, SR};
use super::*;
use crate::aequalis::fft::Fft;

const WN: usize = 4096;

/// Welch's mean power per bin (Hann, 4096 points, hop 1024) of `x`.
fn welch(x: &[f32]) -> Vec<f64> {
    let fft = Fft::new(WN);
    let w: Vec<f32> = (0..WN)
        .map(|i| {
            (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / WN as f64)) as f32
        })
        .collect();
    let mut sum = vec![0.0f64; WN / 2 + 1];
    let mut frames = 0;
    let (mut re, mut im) = (vec![0.0f32; WN], vec![0.0f32; WN]);
    let mut k = 0;
    while k + WN <= x.len() {
        for i in 0..WN {
            re[i] = x[k + i] * w[i];
            im[i] = 0.0;
        }
        fft.run(&mut re, &mut im, false);
        for (b, s) in sum.iter_mut().enumerate() {
            *s += (re[b] as f64).powi(2) + (im[b] as f64).powi(2);
        }
        frames += 1;
        k += WN / 4;
    }
    sum.iter_mut().for_each(|s| *s /= frames.max(1) as f64);
    sum
}

/// The power between `lo` and `hi` Hz, dB.
fn band(p: &[f64], lo: f32, hi: f32) -> f64 {
    let hz = SR as f64 / WN as f64;
    let e: f64 = p
        .iter()
        .enumerate()
        .filter(|(k, _)| {
            let f = *k as f64 * hz;
            f >= lo as f64 && f < hi as f64
        })
        .map(|(_, v)| *v)
        .sum();
    10.0 * (e.max(1e-30)).log10()
}

/// The output aligned with the input: `y` from the latency on, `x` as long.
fn aligned<'a>(x: &'a [f32], y: &'a [f32], lat: usize, skip: usize) -> (&'a [f32], &'a [f32]) {
    let n = y.len() - lat - skip;
    (&x[skip..skip + n], &y[lat + skip..lat + skip + n])
}

/// Pink noise with a peak of `db` at `hz` (quality `q`), both channels the same; and the
/// same noise without the peak.
fn resonant(secs: f32, hz: f32, q: f32, db: f32, seed: u64) -> (Stereo, Vec<f32>) {
    let n = (secs * SR) as usize;
    let clean = pink(n, seed, 0.25);
    let mut x = clean.clone();
    peaking(&mut x, hz, q, db);
    ((x.clone(), x), clean)
}

/// The change (dB) Lenis makes in each band `bands`, after a second's settling.
fn changes(u: &mut Lenis, x: &Stereo, bands: &[(f32, f32)]) -> Vec<f64> {
    let y = run(u, x, 256);
    let lat = u.latency();
    let (xa, ya) = aligned(&x.0, &y.0, lat, SR as usize);
    let (px, py) = (welch(xa), welch(ya));
    bands
        .iter()
        .map(|(lo, hi)| band(&py, *lo, *hi) - band(&px, *lo, *hi))
        .collect()
}

/// The neighbourhoods a 3 kHz resonance's measurement reads: the peak (a sixth of an
/// octave), the broad regions under and over it.
const AROUND_3K: [(f32, f32); 5] = [
    (2_830.0, 3_180.0),
    (200.0, 1_000.0),
    (1_000.0, 2_000.0),
    (5_000.0, 10_000.0),
    (10_000.0, 16_000.0),
];

#[test]
fn a_resonance_on_pink_noise_is_cut_and_its_neighbours_kept() {
    let (x, _) = resonant(8.0, 3_000.0, 8.0, 12.0, 1);
    println!("pink + 12 dB at 3 kHz, Q 8; change dB: peak | 200-1k | 1-2k | 5-10k | 10-16k");
    let mut table = vec![];
    for mode in [Mode::Soft, Mode::Hard] {
        for depth in [0.25f32, 0.5, 0.75, 1.0] {
            let s = LenisSettings {
                depth,
                mode,
                ..LenisSettings::default()
            };
            let c = changes(&mut unit(&s), &x, &AROUND_3K);
            println!(
                "  {mode:?} depth {depth:>4}: {:+6.2} | {:+5.2} | {:+5.2} | {:+5.2} | {:+5.2}",
                c[0], c[1], c[2], c[3], c[4]
            );
            table.push((mode, depth, c));
        }
    }
    for (mode, depth, c) in &table {
        // The broad regions are kept within half a decibel.
        for v in &c[1..] {
            assert!(v.abs() < 0.5, "{mode:?} {depth}: a neighbour moved {v:+.2} dB");
        }
    }
    let at = |m: Mode, d: f32| table.iter().find(|t| t.0 == m && t.1 == d).unwrap().2[0];
    assert!(at(Mode::Soft, 0.5) < -3.0, "soft 50 %: {}", at(Mode::Soft, 0.5));
    assert!(at(Mode::Hard, 0.5) < -6.0, "hard 50 %: {}", at(Mode::Hard, 0.5));
    assert!(at(Mode::Hard, 1.0) <= at(Mode::Hard, 0.5) && at(Mode::Hard, 0.5) < at(Mode::Hard, 0.25) - 2.0, "more depth, more cut");
    assert!(at(Mode::Soft, 1.0) < at(Mode::Soft, 0.25) - 3.0, "more depth, more cut");
}

#[test]
fn flat_noise_is_barely_touched() {
    let n = (8.0 * SR) as usize;
    let thirds: Vec<(f32, f32)> = (0..30)
        .map(|i| {
            let c = 25.0 * math::powf(2.0f32, i as f32 / 3.0);
            (c * math::powf(2.0f32, -1.0 / 6.0), c * math::powf(2.0f32, 1.0 / 6.0))
        })
        .filter(|(_, hi)| *hi < 20_000.0)
        .collect();
    for (name, sig) in [("pink", pink(n, 5, 0.3)), ("white", white(n, 6, 0.3))] {
        for (depth, sel) in [(0.5f32, 0.5f32), (1.0, 0.5), (1.0, 0.2), (1.0, 0.0)] {
            let s = LenisSettings {
                depth,
                selectivity: sel,
                ..LenisSettings::default()
            };
            let x = (sig.clone(), sig.clone());
            let mut u = unit(&s);
            let y = run(&mut u, &x, 256);
            let lat = u.latency();
            let (xa, ya) = aligned(&x.0, &y.0, lat, SR as usize);
            let (ex, ed): (f64, f64) = xa.iter().zip(ya).fold((0.0, 0.0), |(e, d), (a, b)| {
                (e + (*a as f64).powi(2), d + ((*a - *b) as f64).powi(2))
            });
            let (px, py) = (welch(xa), welch(ya));
            let worst = thirds
                .iter()
                .map(|(lo, hi)| (band(&py, *lo, *hi) - band(&px, *lo, *hi)).abs())
                .fold(0.0f64, f64::max);
            let delta_db = 10.0 * (ed / ex).log10();
            println!(
                "{name} noise, depth {depth} selectivity {sel}: the removed part {delta_db:.1} dB under the input; worst third-octave {worst:.2} dB"
            );
            if sel >= 0.5 {
                assert!(delta_db < -30.0, "{name} {depth}: {delta_db:.1} dB");
                assert!(worst < 0.3, "{name} {depth}: {worst:.2} dB");
            }
        }
    }
}

#[test]
fn the_reduction_follows_depth_and_sharpness() {
    // A narrow peak (Q 20, a 15th of an octave) and a broad one (Q 1.5, two thirds),
    // both +10 dB.
    let n = (8.0 * SR) as usize;
    let base = pink(n, 8, 0.25);
    let mut narrow = base.clone();
    peaking(&mut narrow, 4_000.0, 20.0, 10.0);
    let mut broad = base.clone();
    peaking(&mut broad, 1_500.0, 1.5, 10.0);
    println!("sharpness | narrow 4 kHz Q 20 | broad 1.5 kHz Q 1.5 (dB at the peak, depth 50 % hard)");
    let mut rows = vec![];
    for sh in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let s = LenisSettings {
            depth: 0.5,
            sharpness: sh,
            selectivity: 0.3,
            mode: Mode::Hard,
            ..LenisSettings::default()
        };
        let a = changes(&mut unit(&s), &(narrow.clone(), narrow.clone()), &[(3_900.0, 4_100.0)])[0];
        let b = changes(&mut unit(&s), &(broad.clone(), broad.clone()), &[(1_300.0, 1_700.0)])[0];
        println!("  {sh:.2} | {a:+6.2} | {b:+6.2}");
        rows.push((sh, a, b));
    }
    // The narrow peak is cut at every sharpness; the broad bump only at low sharpness.
    assert!(rows.iter().all(|r| r.1 < -2.0));
    assert!(rows[0].2 < rows[4].2 - 1.0, "the broad bump: {:?}", rows);
    assert!(rows[4].2 > -1.5, "the sharpest leaves the broad bump: {:?}", rows[4]);
}

/// The excess the detector reads, on noise and on a resonance, for detector variants.
#[test]
#[ignore]
fn diagnose_the_excess() {
    let n = (6.0 * SR) as usize;
    let noise = pink(n, 5, 0.3);
    let ((res, _), _) = resonant(6.0, 3_000.0, 8.0, 12.0, 1);
    let mut note = harmonic(n, 220.0);
    let plain = note.clone();
    peaking(&mut note, 3_000.0, 8.0, 10.0);
    let mut cliff = pink(n, 6, 0.3);
    for _ in 0..8 {
        let a = 1.0 - math::exp(-std::f32::consts::TAU * 5_000.0 / SR);
        let mut z = 0.0f32;
        for v in cliff.iter_mut() {
            z += a * (*v - z);
            *v = z;
        }
    }
    for (edge, guard) in [(detect::EDGE_DB, detect::GUARD_MUL)] {
        let bridge = format!("edge {edge} guard {guard}");
        let mut stats = vec![];
        for (si, sig) in [&noise, &res, &plain, &note, &cliff].into_iter().enumerate() {
            let mut per = vec![0.0f32; 2_100];
            let mut frames = 0usize;
            let mut u = unit(&LenisSettings {
                depth: 0.5,
                ..LenisSettings::default()
            });
            let hop = u.geometry().hop();
            u.design.edge_db = edge;
            u.design.guard_mul = guard;

            let (mut l, mut r) = (sig.clone(), sig.clone());
            let mut all = vec![];
            let mut at3k = vec![];
            let k3 = (3_000.0 / (SR / u.geometry().n as f32)).round() as usize;
            for (i, (a, b)) in l.chunks_mut(hop).zip(r.chunks_mut(hop)).enumerate() {
                u.process(a, b);
                if i * hop < SR as usize {
                    continue;
                }
                let e = &u.scratch.excess;
                for k in 0..u.geometry().bins() {
                    let f = u.bin_hz(k);
                    if (200.0..16_000.0).contains(&f) {
                        all.push(e[k]);
                    }
                }
                at3k.push(e[k3]);
                frames += 1;
                for k in 0..u.geometry().bins() {
                    per[k] += e[k];
                }
            }
            all.sort_by(|a, b| a.total_cmp(b));
            let q = |p: f64| all[((all.len() - 1) as f64 * p) as usize];
            let mean3k = at3k.iter().sum::<f32>() / at3k.len() as f32;
            stats.push((q(0.5), q(0.99), q(0.999), mean3k));
            if false && (si == 2 || si == 4) {
                let hot: Vec<String> = (0..u.geometry().bins())
                    .filter(|k| per[*k] / frames as f32 > 3.0)
                    .map(|k| format!("{:.0}:{:+.1}", u.bin_hz(k), per[k] / frames as f32))
                    .collect();
                println!("  signal {si}: bins over 3 dB on average: {}", hot.join(" "));
                let sc = &u.scratch;
                for k in (290..345).step_by(3) {
                    println!("    {:.0} Hz: level {:.1} fine {:.1} clip {:.1} env {:.1} excess {:.1}", u.bin_hz(k), sc.fast_level[k], sc.fine[k], sc.clip[k], sc.env[k], sc.excess[k]);
                }
            }
        }
        println!(
            "{bridge}: noise p50 {:+.1} p99 {:+.1} p99.9 {:+.1} | resonance on noise {:+.1} | a 220 Hz note p99 {:+.1} p99.9 {:+.1} | its 3 kHz peak {:+.1} | pink low-passed at 5 kHz p99 {:+.1} p99.9 {:+.1}",
            stats[0].0, stats[0].1, stats[0].2, stats[1].3, stats[2].1, stats[2].2, stats[3].3, stats[4].1, stats[4].2
        );
    }
}

/// A steady note: harmonics of `f0` at 1/k with drawn phases and a slow vibrato.
pub(super) fn harmonic(n: usize, f0: f32) -> Vec<f32> {
    let parts = (16_000.0 / f0) as usize;
    let mut ph: Vec<f32> = (0..parts).map(|k| 0.5 + 0.5 * super::tests::hash(k as u64 + 99)).collect();
    (0..n)
        .map(|i| {
            let t = i as f32 / SR;
            let f = f0 * (1.0 + 0.004 * math::sin(std::f32::consts::TAU * 5.0 * t));
            let mut v = 0.0f32;
            for (k, p) in ph.iter_mut().enumerate() {
                *p = (*p + f * (k + 1) as f32 / SR).fract();
                v += math::sin(std::f32::consts::TAU * *p) / (k + 1) as f32;
            }
            0.2 * v
        })
        .collect()
}

#[test]
fn the_time_response_follows_attack_and_release() {
    let n = (4.0 * SR) as usize;
    let mut x = pink(n, 31, 0.25);
    let (on, off) = (SR as usize, 2 * SR as usize);
    for (i, v) in x.iter_mut().enumerate().take(off).skip(on) {
        let ramp = ((i - on) as f32 / 48.0).min(1.0);
        *v += 0.1 * ramp * math::sin(std::f32::consts::TAU * 3_000.0 * i as f32 / SR);
    }
    println!("a 3 kHz tone over pink noise from 1 s to 2 s, depth 60 % hard:");
    for (att, rel) in [(1.0, 50.0), (5.0, 80.0), (20.0, 200.0), (50.0, 500.0)] {
        let mut u = unit(&LenisSettings {
            depth: 0.6,
            mode: Mode::Hard,
            attack_ms: att,
            release_ms: rel,
            ..LenisSettings::default()
        });
        let hop = u.geometry().hop();
        let k3 = (3_000.0 / (SR / u.geometry().n as f32)).round() as usize;
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut track = vec![];
        for (a, b) in l.chunks_mut(hop).zip(r.chunks_mut(hop)) {
            u.process(a, b);
            track.push(u.reduction_bins(0)[k3]);
        }
        let t = |i: usize| (i * hop) as f32 / SR;
        let held: Vec<f32> = track
            .iter()
            .enumerate()
            .filter(|(i, _)| t(*i) > 1.5 && t(*i) < 2.0)
            .map(|(_, v)| *v)
            .collect();
        let full = held.iter().sum::<f32>() / held.len() as f32;
        let when = |from: f32, f: &dyn Fn(f32) -> bool| {
            track
                .iter()
                .enumerate()
                .find(|(i, v)| t(*i) >= from && f(**v))
                .map(|(i, _)| (t(i) - from) * 1e3)
                .unwrap_or(f32::NAN)
        };
        let half = when(1.0, &|v| v >= 0.5 * full);
        let rise = when(1.0, &|v| v >= 0.9 * full);
        let let_go = when(2.0, &|v| v <= 0.5 * full);
        let fall = when(2.0, &|v| v <= 0.1 * full);
        println!(
            "  attack {att:>4} ms release {rel:>4} ms: held {full:.1} dB; half of it {half:.0} ms after the onset, 90 % {rise:.0} ms; half let go {let_go:.0} ms after the end, 90 % {fall:.0} ms"
        );
        assert!(full > 6.0, "the tone is turned down: {full}");
        assert!(half < 20.0 + 2.0 * att, "half at {half} ms, attack {att}");
        assert!(rise < 60.0 + 3.0 * att, "90 % at {rise} ms, attack {att}");
        assert!(fall < 40.0 + 2.5 * rel, "let go at {fall} ms, release {rel}");
        // Before the tone, nothing.
        let before = track
            .iter()
            .enumerate()
            .filter(|(i, _)| t(*i) > 0.5 && t(*i) < 0.99)
            .fold(0.0f32, |m, (_, v)| m.max(*v));
        assert!(before < 0.5, "before the tone: {before}");
    }
}

/// A struck resonance (an impulse into a 2.5 kHz two-pole with a time constant of 60 ms)
/// every half second over quiet pink noise.
fn strikes(secs: f32) -> (Vec<f32>, Vec<usize>) {
    let n = (secs * SR) as usize;
    let mut x = pink(n, 41, 0.01);
    let period = SR as usize / 2;
    let w = std::f64::consts::TAU * 2_500.0 / SR as f64;
    let rad = math::exp(-1.0 / (0.06 * SR as f64));
    let (a1, a2) = (2.0 * rad * math::cos(w), -rad * rad);
    let (mut y1, mut y2) = (0.0f64, 0.0f64);
    let mut onsets = vec![];
    for (i, v) in x.iter_mut().enumerate() {
        let imp = if i % period == period / 2 {
            onsets.push(i);
            0.08
        } else {
            0.0
        };
        let y = imp + a1 * y1 + a2 * y2;
        y2 = y1;
        y1 = y;
        *v += y as f32;
    }
    (x, onsets)
}

#[test]
fn the_window_pairs_compared() {
    let ((res3k, _), _) = resonant(6.0, 3_000.0, 8.0, 12.0, 1);
    let ((box250, _), _) = resonant(6.0, 250.0, 6.0, 12.0, 2);
    let (hit, onsets) = strikes(6.0);
    println!("frame/half (latency): 3 kHz peak | its neighbours | 250 Hz peak | pre-echo of a strike (dB under it) | the strike's ring cut");
    let mut rows = vec![];
    for (n, m) in [(512, 256), (1024, 256), (1024, 512), (2048, 512), (2048, 1024)] {
        let s = LenisSettings {
            depth: 0.5,
            mode: Mode::Hard,
            ..LenisSettings::default()
        };
        let mk = || {
            let mut u = Lenis::with_geometry(SR, n, m);
            u.set_keeping_shape(&s);
            u
        };
        let c3 = changes(&mut mk(), &(res3k.clone(), res3k.clone()), &AROUND_3K);
        let c250 = changes(&mut mk(), &(box250.clone(), box250.clone()), &[(220.0, 285.0)]);
        let mut u = mk();
        let y = run(&mut u, &(hit.clone(), hit.clone()), 256);
        let lat = u.latency();
        // The error (output − dry) in the 20 ms before each strike, against the strike's
        // own first 20 ms; and how much of the strike's ring (20–200 ms) was taken.
        let (mut pre, mut strike, mut ring_in, mut ring_out) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let w20 = (0.02 * SR) as usize;
        for &o in onsets.iter().skip(1) {
            if o + lat + SR as usize / 5 >= y.0.len() {
                break;
            }
            for i in o - w20..o {
                pre += ((y.0[i + lat] - hit[i]) as f64).powi(2);
            }
            for i in o..o + w20 {
                strike += (hit[i] as f64).powi(2);
            }
            for i in o + w20..o + SR as usize / 5 {
                ring_in += (hit[i] as f64).powi(2);
                ring_out += (y.0[i + lat] as f64).powi(2);
            }
        }
        let pre_db = 10.0 * (pre / strike).log10();
        let ring_db = 10.0 * (ring_out / ring_in).log10();
        let worst = c3[1..].iter().fold(0.0f64, |a, v| a.max(v.abs()));
        println!(
            "  {n}/{m} ({:.1} ms): {:+.2} | {worst:.2} | {:+.2} | {pre_db:.1} | {ring_db:+.2}",
            (2 * m - 1) as f32 / SR * 1e3,
            c3[0],
            c250[0]
        );
        rows.push((n, m, c3[0], worst, c250[0], pre_db));
    }
    for r in &rows {
        assert!(r.2 < -2.5 && r.3 < 0.3, "{r:?}");
        if r.1 * 4 == r.0 {
            // The shipped shapes (a synthesis half of a quarter frame).
            assert!(r.5 < -20.0, "pre-echo {r:?}");
        }

    }
}

#[test]
#[ignore]
fn cpu_at_48k() {
    let secs = 60.0;
    let n = (secs * SR) as usize;
    let x = super::tests::harsh(n, 3);
    println!("CPU, one core, 48 kHz, blocks of 256, a minute of a harsh stereo signal, best of 3:");
    for q in Quality::ALL {
        for ch in [Channels::Linked, Channels::Independent, Channels::MidSide] {
            for depth in [0.6f32, 0.0] {
                let s = LenisSettings {
                    depth,
                    quality: q,
                    channels: ch,
                    ..LenisSettings::default()
                };
                let mut best = f64::MAX;
                for _ in 0..3 {
                    let mut u = unit(&s);
                    let (mut l, mut r) = (x.0.clone(), x.1.clone());
                    let t0 = std::time::Instant::now();
                    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
                        u.process(a, b);
                    }
                    best = best.min(t0.elapsed().as_secs_f64());
                    std::hint::black_box(&l);
                }
                println!("  {q:?} {ch:?} depth {depth}: {:.2} % of a core", 100.0 * best / secs as f64);
            }
        }
    }
}

#[test]
fn a_note_s_harmonics_are_not_resonances_but_a_peak_over_them_is() {
    let n = (6.0 * SR) as usize;
    println!("a steady note (harmonics at 1/k), depth 50 % soft, selectivity 50 %:");
    let mut rows = vec![];
    for (f0, q) in [
        (110.0f32, Quality::Standard),
        (110.0, Quality::Precise),
        (220.0, Quality::Standard),
        (330.0, Quality::Standard),
    ] {
        let plain = harmonic(n, f0);
        let mut peaked = plain.clone();
        peaking(&mut peaked, 3_000.0, 8.0, 10.0);
        let s = LenisSettings {
            depth: 0.5,
            quality: q,
            ..LenisSettings::default()
        };
        let bands = [(400.0, 2_000.0), (2_000.0, 8_000.0), (2_600.0, 3_400.0), (8_000.0, 15_000.0)];
        let a = changes(&mut unit(&s), &(plain.clone(), plain.clone()), &bands);
        let b = changes(&mut unit(&s), &(peaked.clone(), peaked.clone()), &bands);
        println!(
            "  {f0} Hz {q:?}: plain {:+.2} (400-2k) {:+.2} (2-8k) {:+.2} (8-15k) | with +10 dB at 3 kHz, Q 8: the peak {:+.2}, 400-2k {:+.2}",
            a[0], a[1], a[3], b[2], b[0]
        );
        rows.push((f0, q, a, b));
    }
    for (f0, q, a, b) in &rows {
        assert!(a.iter().all(|v| v.abs() < 0.5), "{f0} {q:?}: the plain note {a:?}");
        assert!(b[0].abs() < 0.5, "{f0} {q:?}: {b:?}");
        assert!(b[2] < -0.4, "{f0} {q:?}: the peak over the harmonics {b:?}");
        // Strong (Hard, 80 %, selectivity 25 %) cuts it plainly and still leaves the note.
        let strong = LenisSettings {
            depth: 0.8,
            selectivity: 0.25,
            mode: Mode::Hard,
            quality: *q,
            ..LenisSettings::default()
        };
        let bands = [(800.0, 2_000.0), (2_600.0, 3_400.0), (5_000.0, 15_000.0), (400.0, 800.0)];
        let c = changes(&mut unit(&strong), &{
            let mut p = harmonic(n, *f0);
            peaking(&mut p, 3_000.0, 8.0, 10.0);
            (p.clone(), p)
        }, &bands);
        println!(
            "  {f0} Hz {q:?} strong: the peak {:+.2}, 800-2k {:+.2}, 5-15k {:+.2}; 400-800 {:+.2} (low partials: the focus decides)",
            c[1], c[0], c[2], c[3]
        );
        assert!(c[1] < -2.0 && c[0].abs() < 0.5 && c[2].abs() < 0.5, "{f0} {q:?} strong: {c:?}");
    }
}

#[test]
#[ignore]
fn debug_note_peak() {
    let n = (3.0 * SR) as usize;
    let mut x = harmonic(n, 110.0);
    peaking(&mut x, 3_000.0, 8.0, 10.0);
    let mut u = unit(&LenisSettings {
        depth: 0.5,
        ..LenisSettings::default()
    });
    run(&mut u, &(x.clone(), x), 256);
    let sc = &u.scratch;
    for k in (40..100).step_by(2) {
        println!(
            "{:.0} Hz: level {:.1} excess {:.1} red {:.2} lo {} hi {} br {}",
            u.bin_hz(k),
            sc.level[k],
            sc.excess[k],
            u.reduction_bins(0)[k],
            u.design.lo[k],
            u.design.hi[k],
            u.design.bridge[k]
        );
    }
}
