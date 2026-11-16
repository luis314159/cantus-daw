//! Lenis measured: a resonance cut and its neighbours kept, flat noise left alone, the
//! reduction against depth and sharpness, the time response, the window pairs compared,
//! and the CPU (`#[ignore]`: run alone, in release).
//!
//! `cargo test -p cantus-dsp --lib lenis::tests_measure -- --nocapture`

use super::tests::{peaking, pink, run, unit, white, Stereo, SR};
use super::*;
use crate::aequalis::fft::Fft;

const WN: usize = 4096;

fn sq(x: f64) -> f64 {
    x * x
}

/// Welch's mean power per bin (Hann, 4096 points, hop 1024) of `x`.
fn welch(x: &[f32]) -> Vec<f64> {
    let fft = Fft::new(WN);
    let w: Vec<f32> = (0..WN)
        .map(|i| (0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / WN as f64)) as f32)
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
            *s += sq(re[b] as f64) + sq(im[b] as f64);
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
    10.0 * math::log10(e.max(1e-30))
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
            assert!(
                v.abs() < 0.5,
                "{mode:?} {depth}: a neighbour moved {v:+.2} dB"
            );
        }
    }
    let at = |m: Mode, d: f32| table.iter().find(|t| t.0 == m && t.1 == d).unwrap().2[0];
    assert!(
        at(Mode::Soft, 0.5) < -3.0,
        "soft 50 %: {}",
        at(Mode::Soft, 0.5)
    );
    assert!(
        at(Mode::Hard, 0.5) < -4.5,
        "hard 50 %: {}",
        at(Mode::Hard, 0.5)
    );
    assert!(
        at(Mode::Hard, 1.0) < at(Mode::Hard, 0.75) - 2.0
            && at(Mode::Hard, 0.75) < at(Mode::Hard, 0.5) - 2.0
            && at(Mode::Hard, 1.0) < -9.0,
        "more depth, more cut, all the way to 100 %"
    );
    assert!(
        at(Mode::Soft, 1.0) < at(Mode::Soft, 0.25) - 3.0,
        "more depth, more cut, all the way to 100 %"
    );
}

#[test]
fn flat_noise_is_barely_touched() {
    let n = (8.0 * SR) as usize;
    let thirds: Vec<(f32, f32)> = (0..30)
        .map(|i| {
            let c = 25.0 * math::powf(2.0f32, i as f32 / 3.0);
            (
                c * math::powf(2.0f32, -1.0 / 6.0),
                c * math::powf(2.0f32, 1.0 / 6.0),
            )
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
                (e + sq(*a as f64), d + sq((*a - *b) as f64))
            });
            let (px, py) = (welch(xa), welch(ya));
            let worst = thirds
                .iter()
                .map(|(lo, hi)| (band(&py, *lo, *hi) - band(&px, *lo, *hi)).abs())
                .fold(0.0f64, f64::max);
            let delta_db = 10.0 * math::log10(ed / ex);
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
    println!(
        "sharpness | narrow 4 kHz Q 20 | broad 1.5 kHz Q 1.5 (dB at the peak, depth 50 % hard)"
    );
    let mut rows = vec![];
    for sh in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let s = LenisSettings {
            depth: 0.5,
            sharpness: sh,
            selectivity: 0.3,
            mode: Mode::Hard,
            ..LenisSettings::default()
        };
        let a = changes(
            &mut unit(&s),
            &(narrow.clone(), narrow.clone()),
            &[(3_900.0, 4_100.0)],
        )[0];
        let b = changes(
            &mut unit(&s),
            &(broad.clone(), broad.clone()),
            &[(1_300.0, 1_700.0)],
        )[0];
        println!("  {sh:.2} | {a:+6.2} | {b:+6.2}");
        rows.push((sh, a, b));
    }
    // The narrow peak is cut at every sharpness; the broad bump only at low sharpness.
    assert!(rows.iter().all(|r| r.1 < -2.0));
    assert!(rows[0].2 < rows[4].2 - 1.0, "the broad bump: {:?}", rows);
    assert!(
        rows[4].2 > -2.0,
        "the sharpest leaves the broad bump: {:?}",
        rows[4]
    );
}

/// The excess the detector reads, on noise and on a resonance, for detector variants.
#[test]
#[ignore]
#[allow(clippy::needless_range_loop)]
fn diagnose_the_excess() {
    let n = (6.0 * SR) as usize;
    let lowpass = |x: &mut Vec<f32>, hz: f32, poles: usize| {
        for _ in 0..poles {
            let a = 1.0 - math::exp(-std::f32::consts::TAU * hz / SR);
            let mut z = 0.0f32;
            for v in x.iter_mut() {
                z += a * (*v - z);
                *v = z;
            }
        }
    };
    let noise = pink(n, 5, 0.3);
    let ((res, _), _) = resonant(6.0, 3_000.0, 8.0, 12.0, 1);
    let plain = harmonic(n, 220.0);
    let mut note = plain.clone();
    peaking(&mut note, 3_000.0, 8.0, 10.0);
    let mut cliff = pink(n, 6, 0.3);
    lowpass(&mut cliff, 5_000.0, 8);
    // A Q 4, +6 dB peak at 3 kHz on a 12 dB/octave slope (two poles at 1 kHz).
    let mut steep = pink(n, 7, 0.3);
    lowpass(&mut steep, 1_000.0, 2);
    peaking(&mut steep, 3_000.0, 4.0, 6.0);
    let sigs = [&noise, &res, &plain, &note, &cliff, &steep];
    for (hull, edge, moved) in [
        (true, detect::EDGE_DB, detect::EDGE_SLOPE_DB),
        (true, 3.0, 3.0),
        (false, 3.0, 3.0),
    ] {
        let mut stats = vec![];
        for sig in sigs {
            let mut u = unit(&LenisSettings {
                depth: 0.5,
                ..LenisSettings::default()
            });
            u.design.hull = hull;
            u.design.edge_db = edge;
            u.design.edge_slope_db = moved;
            let s = u.s;
            u.design.build(&s, u.sr, u.g.n, u.g.hop(), u.wsum);
            let hop = u.geometry().hop();
            let (mut l, mut r) = (sig.clone(), sig.clone());
            let (mut all, mut low, mut at3k) = (vec![], vec![], vec![]);
            let k3 = (3_000.0 / (SR / u.geometry().n as f32)).round() as usize;
            for (i, (a, b)) in l.chunks_mut(hop).zip(r.chunks_mut(hop)).enumerate() {
                u.process(a, b);
                if i * hop < SR as usize {
                    continue;
                }
                let e = &u.scratch.excess;
                for k in 0..u.geometry().bins() {
                    let f = u.bin_hz(k);
                    if (800.0..16_000.0).contains(&f) {
                        all.push(e[k]);
                    } else if (150.0..800.0).contains(&f) {
                        low.push(e[k]);
                    }
                }
                at3k.push(e[k3]);
            }
            let q = |v: &mut Vec<f32>, p: f64| {
                v.sort_by(|a, b| a.total_cmp(b));
                v[((v.len() - 1) as f64 * p) as usize]
            };
            let mean3k = at3k.iter().sum::<f32>() / at3k.len() as f32;
            stats.push((
                q(&mut all, 0.5),
                q(&mut all, 0.99),
                q(&mut all, 0.999),
                mean3k,
                q(&mut low, 0.99),
            ));
        }
        println!(
            "hull {hull} edge {edge} on a slope {moved}: noise p50 {:+.1} p99 {:+.1} p99.9 {:+.1} (low p99 {:+.1}) | resonance on noise {:+.1} | a 220 Hz note p99 {:+.1} p99.9 {:+.1}, under 800 Hz p99 {:+.1} | its 3 kHz peak {:+.1} | pink low-passed at 5 kHz p99 {:+.1} p99.9 {:+.1} | a Q 4 +6 dB peak on a 12 dB/octave slope {:+.1}",
            stats[0].0, stats[0].1, stats[0].2, stats[0].4, stats[1].3, stats[2].1, stats[2].2, stats[2].4, stats[3].3, stats[4].1, stats[4].2, stats[5].3
        );
    }
}

/// A steady note: harmonics of `f0` at 1/k with drawn phases and a slow vibrato.
pub(super) fn harmonic(n: usize, f0: f32) -> Vec<f32> {
    let parts = (16_000.0 / f0) as usize;
    let mut ph: Vec<f32> = (0..parts)
        .map(|k| 0.5 + 0.5 * super::tests::hash(k as u64 + 99))
        .collect();
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
    let noise = pink(n, 31, 0.25);
    let (on, off) = (SR as usize, 2 * SR as usize);
    let with_tone = |hz: f32| {
        let mut x = noise.clone();
        for (i, v) in x.iter_mut().enumerate().take(off).skip(on) {
            let ramp = ((i - on) as f32 / 48.0).min(1.0);
            *v += 0.1 * ramp * math::sin(std::f32::consts::TAU * hz * i as f32 / SR);
        }
        x
    };
    let (centred, between) = (with_tone(3_000.0), with_tone(3_023.0));
    println!(
        "a tone over pink noise from 1 s to 2 s, depth 60 % hard (the onset's lift off → on):"
    );
    for (att, rel, q, hz, lift) in [
        (1.0, 50.0, Quality::Standard, 3_000.0, false),
        (1.0, 50.0, Quality::Standard, 3_000.0, true),
        (1.0, 50.0, Quality::Standard, 3_023.0, false),
        (1.0, 50.0, Quality::Standard, 3_023.0, true),
        (5.0, 80.0, Quality::Standard, 3_000.0, false),
        (5.0, 80.0, Quality::Standard, 3_000.0, true),
        (5.0, 80.0, Quality::Standard, 3_023.0, false),
        (5.0, 80.0, Quality::Standard, 3_023.0, true),
        (20.0, 200.0, Quality::Standard, 3_000.0, true),
        (50.0, 500.0, Quality::Standard, 3_000.0, true),
        (1.0, 50.0, Quality::Fast, 3_000.0, false),
        (1.0, 50.0, Quality::Fast, 3_000.0, true),
        (1.0, 50.0, Quality::Precise, 3_000.0, false),
        (1.0, 50.0, Quality::Precise, 3_000.0, true),
    ] {
        let mut u = unit(&LenisSettings {
            depth: 0.6,
            mode: Mode::Hard,
            attack_ms: att,
            release_ms: rel,
            quality: q,
            ..LenisSettings::default()
        });
        u.design.lift = lift;
        let hop = u.geometry().hop();
        let x = if hz == 3_000.0 { &centred } else { &between };
        // The tone's bin (the louder of the two either side of an off-centre tone).
        let k3 = (hz / (SR / u.geometry().n as f32)).floor() as usize;
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut track = vec![];
        for (a, b) in l.chunks_mut(hop).zip(r.chunks_mut(hop)) {
            u.process(a, b);
            let red = u.reduction_bins(0);
            track.push(if hz == 3_000.0 {
                red[k3]
            } else {
                red[k3].max(red[k3 + 1])
            });
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
        let most = |from: f32, to: f32| {
            track
                .iter()
                .enumerate()
                .filter(|(i, _)| t(*i) > from && t(*i) < to)
                .fold(0.0f32, |m, (_, v)| m.max(*v))
        };
        let half = when(1.0, &|v| v >= 0.5 * full);
        let rise = when(1.0, &|v| v >= 0.9 * full);
        let let_go = when(2.0, &|v| v <= 0.5 * full);
        let fall = when(2.0, &|v| v <= 0.1 * full);
        // Before the tone, nothing; after its onset, no more than it is held at.
        let before = most(0.5, 0.99);
        let over = most(1.0, 1.1) - full;
        println!(
            "  {q:?} {hz} Hz, lift {lift}, attack {att:>4} ms release {rel:>4} ms: held {full:.1} dB; half of it {half:.0} ms after the onset, 90 % {rise:.0} ms; most in the first 100 ms {over:+.2} dB over the held; half let go {let_go:.0} ms after the end, 90 % {fall:.0} ms; before the tone {before:.2} dB"
        );
        assert!(full > 6.0, "the tone is turned down: {full}");
        assert!(half < 20.0 + 2.0 * att, "half at {half} ms, attack {att}");
        assert!(rise < 40.0 + 3.0 * att, "90 % at {rise} ms, attack {att}");
        if att == 1.0 && q == Quality::Standard && lift {
            assert!(rise <= 11.0, "90 % well within 15 ms: {rise} ms");
        }
        assert!(
            fall < 80.0 + 2.5 * rel,
            "let go at {fall} ms, release {rel}"
        );
        assert!(before < 0.5, "before the tone: {before}");
        assert!(over < 1.5, "an overshoot of {over} dB");
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
#[allow(clippy::needless_range_loop)]
fn the_window_pairs_compared() {
    let ((res3k, _), _) = resonant(6.0, 3_000.0, 8.0, 12.0, 1);
    let ((box250, _), _) = resonant(6.0, 250.0, 6.0, 12.0, 2);
    let (hit, onsets) = strikes(6.0);
    println!("frame/half (latency): 3 kHz peak | its neighbours | 250 Hz peak | pre-echo of a strike (dB under it) | the strike's ring cut");
    let mut rows = vec![];
    for (n, m) in [
        (512, 256),
        (1024, 256),
        (1024, 512),
        (2048, 512),
        (2048, 1024),
    ] {
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
        let c250 = changes(
            &mut mk(),
            &(box250.clone(), box250.clone()),
            &[(220.0, 285.0)],
        );
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
                pre += sq((y.0[i + lat] - hit[i]) as f64);
            }
            for i in o..o + w20 {
                strike += sq(hit[i] as f64);
            }
            for i in o + w20..o + SR as usize / 5 {
                ring_in += sq(hit[i] as f64);
                ring_out += sq(y.0[i + lat] as f64);
            }
        }
        let pre_db = 10.0 * math::log10(pre / strike);
        let ring_db = 10.0 * math::log10(ring_out / ring_in);
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
                println!(
                    "  {q:?} {ch:?} depth {depth}: {:.2} % of a core",
                    100.0 * best / secs as f64
                );
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
        let bands = [
            (400.0, 2_000.0),
            (2_000.0, 8_000.0),
            (2_600.0, 3_400.0),
            (8_000.0, 15_000.0),
        ];
        let a = changes(&mut unit(&s), &(plain.clone(), plain.clone()), &bands);
        let b = changes(&mut unit(&s), &(peaked.clone(), peaked.clone()), &bands);
        println!(
            "  {f0} Hz {q:?}: plain {:+.2} (400-2k) {:+.2} (2-8k) {:+.2} (8-15k) | with +10 dB at 3 kHz, Q 8: the peak {:+.2}, 400-2k {:+.2}",
            a[0], a[1], a[3], b[2], b[0]
        );
        rows.push((f0, q, a, b));
    }
    for (f0, q, a, b) in &rows {
        assert!(
            a.iter().all(|v| v.abs() < 0.5),
            "{f0} {q:?}: the plain note {a:?}"
        );
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
        let bands = [
            (800.0, 2_000.0),
            (2_600.0, 3_400.0),
            (5_000.0, 15_000.0),
            (150.0, 800.0),
        ];
        let c = changes(
            &mut unit(&strong),
            &{
                let mut p = harmonic(n, *f0);
                peaking(&mut p, 3_000.0, 8.0, 10.0);
                (p.clone(), p)
            },
            &bands,
        );
        println!(
            "  {f0} Hz {q:?} strong: the peak {:+.2}, 800-2k {:+.2}, 5-15k {:+.2}; under 800 Hz (the low partials) {:+.2}",
            c[1], c[0], c[2], c[3]
        );
        assert!(
            c[1] < -2.0 && c[0].abs() < 0.5 && c[2].abs() < 0.5 && c[3].abs() <= 0.5,
            "{f0} {q:?} strong: {c:?}"
        );
    }
}

#[test]
fn a_broad_peak_on_a_steep_slope_is_found() {
    // A Q 4, +6 dB peak at 3 kHz on pink noise rolled off at 12 dB an octave from 1 kHz.
    let n = (8.0 * SR) as usize;
    let mut x = pink(n, 7, 0.3);
    for _ in 0..2 {
        let a = 1.0 - math::exp(-std::f32::consts::TAU * 1_000.0 / SR);
        let mut z = 0.0f32;
        for v in x.iter_mut() {
            z += a * (*v - z);
            *v = z;
        }
    }
    let plain = x.clone();
    peaking(&mut x, 3_000.0, 4.0, 6.0);
    let s = LenisSettings {
        depth: 1.0,
        mode: Mode::Hard,
        ..LenisSettings::default()
    };
    // The excess the detector reads at the peak.
    let mut u = unit(&s);
    let hop = u.geometry().hop();
    let k3 = (3_000.0 / (SR / u.geometry().n as f32)).round() as usize;
    let (mut l, mut r) = (x.clone(), x.clone());
    let mut seen = vec![];
    for (i, (a, b)) in l.chunks_mut(hop).zip(r.chunks_mut(hop)).enumerate() {
        u.process(a, b);
        if i * hop > SR as usize {
            seen.push(u.scratch.excess[k3]);
        }
    }
    let excess = seen.iter().sum::<f32>() / seen.len() as f32;
    let bands = [(2_600.0, 3_400.0), (1_000.0, 1_800.0), (5_500.0, 9_000.0)];
    let c = changes(&mut unit(&s), &(x.clone(), x), &bands);
    let p = changes(&mut unit(&s), &(plain.clone(), plain), &bands);
    println!(
        "a Q 4, +6 dB peak on a 12 dB/octave slope: excess {excess:+.1} dB; Hard 100 % cuts it {:+.2} dB (1-1.8 kHz {:+.2}, 5.5-9 kHz {:+.2}); the slope alone {:+.2} / {:+.2} / {:+.2}",
        c[0], c[1], c[2], p[0], p[1], p[2]
    );
    assert!(excess >= 4.0, "{excess}");
    assert!(c[0] < -2.5 && c[1].abs() < 0.5 && c[2].abs() < 0.5, "{c:?}");
    assert!(p.iter().all(|v| v.abs() < 0.3), "the slope alone {p:?}");
}

#[test]
fn an_inharmonic_ring_among_low_partials_is_still_cut() {
    // A 220 Hz note and a ring at 550 Hz (no partial of it), 10 dB over the partials there.
    let n = (6.0 * SR) as usize;
    let mut x = harmonic(n, 220.0);
    for (i, v) in x.iter_mut().enumerate() {
        *v += 0.3 * math::sin(std::f32::consts::TAU * 550.0 * i as f32 / SR);
    }
    let strong = LenisSettings {
        depth: 0.8,
        selectivity: 0.25,
        mode: Mode::Hard,
        ..LenisSettings::default()
    };
    for q in [Quality::Standard, Quality::Precise] {
        let s = LenisSettings {
            quality: q,
            ..strong
        };
        let c = changes(
            &mut unit(&s),
            &(x.clone(), x.clone()),
            &[
                (530.0, 570.0),
                (200.0, 240.0),
                (420.0, 460.0),
                (640.0, 680.0),
            ],
        );
        println!(
            "  {q:?}: the ring {:+.2} dB; the partials at 220, 440, 660 Hz {:+.2} / {:+.2} / {:+.2}",
            c[0], c[1], c[2], c[3]
        );
        assert!(c[0] < -2.0, "{q:?}: {c:?}");
        if q == Quality::Precise {
            // Resolved, the partials beside it are kept.
            assert!(c[1..].iter().all(|v| v.abs() < 1.0), "{c:?}");
        }
    }
}

#[test]
#[ignore]
fn diagnose_the_attack() {
    let n = (2.0 * SR) as usize;
    let mut x = pink(n, 31, 0.25);
    let on = SR as usize;
    for (i, v) in x.iter_mut().enumerate().skip(on) {
        let ramp = ((i - on) as f32 / 48.0).min(1.0);
        *v += 0.1 * ramp * math::sin(std::f32::consts::TAU * 3_000.0 * i as f32 / SR);
    }
    for (q, lift) in [
        (Quality::Standard, true),
        (Quality::Fast, false),
        (Quality::Fast, true),
        (Quality::Precise, false),
        (Quality::Precise, true),
    ] {
        let mut u = unit(&LenisSettings {
            depth: 0.6,
            mode: Mode::Hard,
            attack_ms: 1.0,
            release_ms: 50.0,
            quality: q,
            ..LenisSettings::default()
        });
        let hop = u.geometry().hop();
        let k3 = (3_000.0 / (SR / u.geometry().n as f32)).round() as usize;
        let (mut l, mut r) = (x.clone(), x.clone());
        u.design.lift = lift;
        println!("{q:?} lift {lift}");
        for (i, (a, b)) in l.chunks_mut(hop).zip(r.chunks_mut(hop)).enumerate() {
            u.process(a, b);
            let t = ((i + 1) * hop) as f32 / SR - 1.0;
            if (-0.004..0.045).contains(&t) {
                let (sl, fa) = u.det[0].averages(k3);
                let db = |p: f32| 10.0 * math::log10((p * u.design.norm2) as f64 + 1e-14);
                let (se, fe, lf) = u.scratch.parts[k3];
                let nb: Vec<String> = [-6i32, -4, -3, -2, 2, 3, 4, 6]
                    .iter()
                    .map(|o| format!("{:5.1}", db(u.det[0].averages((k3 as i32 + o) as usize).0)))
                    .collect();
                println!(
                    "  {:5.1} ms slow {:6.1} fast {:6.1} e_slow {:5.1} e_fast {:5.1} lift {:4.1} excess {:5.1} red {:5.2} | {}",
                    t * 1e3,
                    db(sl),
                    db(fa),
                    se,
                    fe,
                    lf,
                    u.scratch.excess[k3],
                    u.reduction_bins(0)[k3],
                    nb.join(" ")
                );
            }
        }
    }
}

#[test]
#[ignore]
fn diagnose_the_lift_calibration() {
    let n = SR as usize;
    for (name, hz) in [
        ("sine 3000", 3_000.0f32),
        ("sine 3023", 3_023.0),
        ("white", 0.0),
    ] {
        let x: Vec<f32> = if hz > 0.0 {
            (0..n)
                .map(|i| 0.1 * math::sin(std::f32::consts::TAU * hz * i as f32 / SR))
                .collect()
        } else {
            white(n, 3, 0.3)
        };
        let mut u = unit(&LenisSettings {
            depth: 0.6,
            ..LenisSettings::default()
        });
        let hop = u.geometry().hop();
        let k3 = (3_000.0 / (SR / u.geometry().n as f32)).round() as usize;
        let (mut l, mut r) = (x.clone(), x.clone());
        let mut acc = [0.0f64; 5];
        let mut cnt = 0;
        for (i, (a, b)) in l.chunks_mut(hop).zip(r.chunks_mut(hop)).enumerate() {
            u.process(a, b);
            if i * hop < SR as usize / 2 {
                continue;
            }
            for (j, band) in [0usize, 1, 2, 4, 8].into_iter().enumerate() {
                let (mut eq, mut es) = (0.0f64, 0.0f64);
                for k in k3 - band..=k3 + band {
                    eq += u.qa[k] as f64;
                    es += u.det[0].averages(k).0 as f64;
                }
                acc[j] += 10.0 * math::log10(eq * u.design.lift_gain as f64 / es);
            }
            cnt += 1;
        }
        let v: Vec<String> = acc
            .iter()
            .map(|a| format!("{:+.2}", a / cnt as f64))
            .collect();
        println!(
            "{name}: short over slow, dB, band 0/1/2/4/8: {}",
            v.join(" ")
        );
    }
}

/// The onset's lift on noise: how often the reduction moves, frame to frame (the "musical
/// noise" of a gain that flickers), and how much of it there is, with the lift off and on.
#[test]
fn the_onset_lift_leaves_noise_alone() {
    let n = (6.0 * SR) as usize;
    let noise = pink(n, 17, 0.3);
    println!("pink noise, Hard, attack 1 ms: reduction p99.9 | bin-frames over 0.5 dB | mean frame-to-frame change (lift off → on)");
    for (depth, selectivity) in [(0.5f32, 0.5f32), (1.0, 0.2), (1.0, 0.0)] {
        let mut row = vec![];
        for lift in [false, true] {
            let mut u = unit(&LenisSettings {
                depth,
                selectivity,
                mode: Mode::Hard,
                attack_ms: 1.0,
                release_ms: 50.0,
                ..LenisSettings::default()
            });
            u.design.lift = lift;
            let hop = u.geometry().hop();
            let bins = u.geometry().bins();
            let (mut l, mut r) = (noise.clone(), noise.clone());
            let mut last = vec![0.0f32; bins];
            let (mut all, mut over, mut moved, mut count) = (vec![], 0usize, 0.0f64, 0usize);
            for (i, (a, b)) in l.chunks_mut(hop).zip(r.chunks_mut(hop)).enumerate() {
                u.process(a, b);
                if i * hop < SR as usize {
                    continue;
                }
                for (k, &v) in u.reduction_bins(0).iter().enumerate() {
                    all.push(v);
                    over += (v > 0.5) as usize;
                    moved += (v - last[k]).abs() as f64;
                    last[k] = v;
                    count += 1;
                }
            }
            all.sort_by(|a, b| a.total_cmp(b));
            let p999 = all[(all.len() - 1) * 999 / 1000];
            row.push((p999, over as f64 / count as f64, moved / count as f64));
        }
        println!(
            "  depth {depth} selectivity {selectivity}: {:.2} → {:.2} dB | {:.4} % → {:.4} % | {:.4} → {:.4} dB",
            row[0].0,
            row[1].0,
            100.0 * row[0].1,
            100.0 * row[1].1,
            row[0].2,
            row[1].2
        );
        assert!(row[1].0 <= row[0].0 + 0.2, "{row:?}");
        assert!(row[1].1 <= row[0].1 * 1.25 + 1e-4, "{row:?}");
        assert!(row[1].2 <= row[0].2 * 1.25 + 1e-4, "{row:?}");
    }
}

/// Notes starting: the onset's lift reads a note's start, so its partials must still not be
/// taken for resonances there — the most any bin under 800 Hz, and 800 Hz–8 kHz, is turned
/// down in the first 100 ms of each of eight notes, with the lift off and on.
#[test]
fn a_note_s_start_is_not_a_resonance() {
    let seg = SR as usize / 2;
    println!("notes of 0.25 s every 0.5 s, strong (Hard 80 %, selectivity 25 %), attack 1 ms: the most cut in the first 100 ms (lift off → on)");
    for (f0, q) in [
        (110.0f32, Quality::Standard),
        (220.0, Quality::Standard),
        (330.0, Quality::Standard),
        (220.0, Quality::Precise),
    ] {
        let note = harmonic(seg / 2, f0);
        let mut x = vec![0.0f32; 9 * seg];
        for s in 1..9 {
            for (i, v) in note.iter().enumerate() {
                let fade = (i as f32 / 48.0).min(1.0) * ((note.len() - i) as f32 / 480.0).min(1.0);
                x[s * seg + i] = v * fade;
            }
        }
        let mut row = vec![];
        for lift in [false, true] {
            let mut u = unit(&LenisSettings {
                depth: 0.8,
                selectivity: 0.25,
                mode: Mode::Hard,
                attack_ms: 1.0,
                release_ms: 50.0,
                low_hz: 20.0,
                quality: q,
                ..LenisSettings::default()
            });
            u.design.lift = lift;
            let hop = u.geometry().hop();
            let (mut l, mut r) = (x.clone(), x.clone());
            let (mut low, mut mid) = (0.0f32, 0.0f32);
            for (i, (a, b)) in l.chunks_mut(hop).zip(r.chunks_mut(hop)).enumerate() {
                u.process(a, b);
                let at = (i + 1) * hop;
                if at < seg || (at % seg) > seg / 5 {
                    continue;
                }
                for (k, &v) in u.reduction_bins(0).iter().enumerate() {
                    let hz = u.bin_hz(k);
                    if (60.0..800.0).contains(&hz) {
                        low = low.max(v);
                    } else if (800.0..8_000.0).contains(&hz) {
                        mid = mid.max(v);
                    }
                }
            }
            row.push((low, mid));
        }
        println!(
            "  {f0} Hz {q:?}: under 800 Hz {:.2} → {:.2} dB, 800 Hz-8 kHz {:.2} → {:.2} dB",
            row[0].0, row[1].0, row[0].1, row[1].1
        );
        assert!(row[1].0 <= row[0].0 + 0.25, "{f0} {q:?}: {row:?}");
        assert!(row[1].1 <= row[0].1 + 0.5, "{f0} {q:?}: {row:?}");
    }
}
