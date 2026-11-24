//! The upper strings' gate (round 42, `arcus-upper`; ADR 0157's, on the violin and the viola
//! at their voices' rate): the bowed pitch within a cent on a held-out grid, the attacks inside
//! Guettler & Askenfelt's limits, Schelleng's wedge on the violin's D and E strings (where
//! Schoonderwaldt, Guettler & Askenfelt 2008 measured it), the vibrato's partials moving
//! through the fitted body, the body and the way back measured. Each prints what it measured
//! (`--nocapture`), for the report.

use super::string::{Bow, BowedString, Friction, StringStep};
use super::tables::{schelleng_max, Instrument};
use super::tests::{cents, measure_hz, SR};
use super::tests_gate::{classify, onsets, onsets_after, without_short_slips, Bowed, Motion};
use super::upper::{body, factor, Decimator};
use super::*;
use crate::math;

/// The cents a bowed note at `key` (+ `fine` cents), dynamic `d` and contact `pos` sounds
/// from its target, with the built-in table.
fn offset(inst: Instrument, key: u8, fine: f32, d: f64, pos: f64) -> f64 {
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        instrument: inst,
        vib_depth: 0.0,
        body_mix: 0.0,
        dyn_src: DynSource::Velocity,
        bow_pos: pos as f32,
        fine_ct: fine,
        ..ArcusSettings::default()
    });
    a.note_on(key, (d as f32).max(0.01), 1);
    let frames = (1.5 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    let want = 440.0 * math::exp2((key as f64 + fine as f64 / 100.0 - 69.0) / 12.0);
    cents(measure_hz(&l, want, 0.5, 1.45), want)
}

#[test]
fn the_upper_strings_bowed_pitch_is_within_a_cent_held_out() {
    let mut all: Vec<(f64, String)> = Vec::new();
    for inst in [Instrument::Violin, Instrument::Viola] {
        let spec = inst.spec();
        let (mut key, mut n) = (spec.bottom(), 0usize);
        while key <= spec.top {
            // Held out of the table: keys a few cents off, dynamics and contacts between
            // the measured ones (the cello's grid, `tests_pitch`).
            let held: [(f64, f64, f32); 3] = [
                (
                    0.06 + 0.11 * (n % 9) as f64,
                    -0.9 + 0.37 * (n % 5) as f64,
                    37.0,
                ),
                (
                    0.93 - 0.13 * (n % 7) as f64,
                    0.85 - 0.41 * (n % 4) as f64,
                    -23.0,
                ),
                (
                    0.31 + 0.07 * (n % 5) as f64,
                    0.62 - 0.29 * (n % 6) as f64,
                    49.0,
                ),
            ];
            for (d, p, fine) in held {
                let e = offset(inst, key, fine, d, p);
                all.push((
                    e.abs(),
                    format!("{inst:?} key {key}{fine:+} d {d:.2} pos {p:.2}: {e:+.3}"),
                ));
            }
            key += 3;
            n += 1;
        }
    }
    all.sort_by(|a, b| b.0.total_cmp(&a.0));
    let over = all.iter().filter(|e| e.0 > 1.0).count();
    let median = all[all.len() / 2].0;
    println!(
        "{} points held out, {over} over a cent, median {median:.3} ct; the worst:",
        all.len()
    );
    for e in all.iter().take(12) {
        println!("  {}", e.1);
    }
    assert_eq!(over, 0, "the worst {}", all[0].1);
    assert!(median <= 0.1, "median {median}");
}

/// A note's attack heard by its own voice at its rate: the slips, the period in the voice's
/// samples, the rate.
fn attack(inst: Instrument, key: u8, velocity: f32) -> (Bowed, f64) {
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        instrument: inst,
        vib_depth: 0.0,
        ..ArcusSettings::default()
    });
    a.note_on(key, velocity, 1);
    let v = a.voices.iter().position(|v| v.active()).expect("a voice");
    a.voices[v].record = true;
    let frames = (0.5 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    let sr = SR as f64 * factor(inst) as f64;
    let f = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    (
        Bowed {
            slips: std::mem::take(&mut a.voices[v].slips),
            out: l,
            period: sr / f,
            beta: a.voices[v].beta() as f64,
        },
        sr,
    )
}

/// `tests_gate::attack_time` at the rate `sr`.
fn attack_time(b: &Bowed, sr: f64) -> Option<f64> {
    let long = (0.6 * (1.0 - b.beta) * b.period) as usize;
    let clean = without_short_slips(&b.slips, (0.25 * b.beta * b.period).max(2.0) as usize);
    let o = onsets_after(&clean, 0, long);
    let ok = |g: usize| ((g as f64) / b.period - 1.0).abs() < 0.04;
    for i in 0..o.len().saturating_sub(6) {
        if (0..5).all(|j| ok(o[i + j + 1] - o[i + j])) {
            return Some(o[i] as f64 / sr);
        }
    }
    None
}

#[test]
fn the_upper_strings_attacks_settle_within_guettler_and_askenfelt_s_limits() {
    let (mut over, mut count, mut worst) = (Vec::new(), 0, (0.0f64, String::new()));
    for inst in [Instrument::Violin, Instrument::Viola] {
        for st in inst.spec().strings.iter() {
            for step in [0u8, 7, 14] {
                let key = st.open + step;
                for vel in [0.15f32, 0.35, 0.55, 0.75, 0.95] {
                    let (b, sr) = attack(inst, key, vel);
                    let t = attack_time(&b, sr).unwrap_or(0.5);
                    let end = ((t * sr) as usize).min(b.slips.len());
                    let o = onsets(&b.slips[..end], 0, b.period);
                    let multiple = o
                        .windows(2)
                        .any(|w| ((w[1] - w[0]) as f64) < 0.75 * b.period);
                    // 50 ms (90 for a multiple flyback): Guettler & Askenfelt 1997, judged on
                    // the violin's open G — in those periods for a string slower than it (the
                    // viola's C), as the cello's gate counts them.
                    let periods = (b.period / sr) * 196.0;
                    let limit = if multiple { 0.09 } else { 0.05 } * periods.max(1.0);
                    count += 1;
                    let line = format!(
                        "{inst:?} key {key} vel {vel}: {:.0} ms (limit {:.0})",
                        1000.0 * t,
                        1000.0 * limit
                    );
                    if t / limit > worst.0 {
                        worst = (t / limit, line.clone());
                    }
                    if t > limit {
                        over.push(line);
                    }
                }
            }
        }
    }
    println!(
        "attacks over their limit: {} of {count}; worst {}",
        over.len(),
        worst.1
    );
    for l in &over {
        println!("  {l}");
    }
    assert!(over.is_empty(), "{} of {count} over", over.len());
}

/// Bow the violin's string `ix` open at `beta` with `force` N at `speed` m/s (accelerating at
/// 20 m/s²), the string alone at the violin's voice rate, for `seconds`.
fn bow_violin(ix: usize, beta: f64, force: f32, speed: f32, seconds: f64) -> Bowed {
    let inst = Instrument::Violin;
    let st = inst.spec().strings[ix];
    let sr = SR * factor(inst) as f32;
    let f = st.open_hz();
    let mut s = BowedString::new(SR);
    s.set_rate(sr);
    let step = StringStep {
        f0: f,
        t60: st.t60_s,
        p: st.p,
        beta,
        dc: false,
        corner_hz: inst.spec().corner_hz,
    };
    s.start(st.z0() as f32, &step, false);
    let n = (seconds * sr as f64) as usize;
    let mut out = vec![0.0f32; n];
    let mut slips = vec![0.0f32; n];
    let zeros = [0.0f32; 32];
    let (mut v, fs) = ([0.0f32; 32], [force; 32]);
    for at in (0..n).step_by(32) {
        let k = 32.min(n - at);
        for (j, x) in v.iter_mut().enumerate().take(k) {
            *x = (20.0 * (at + j + 1) as f32 / sr).min(speed);
        }
        let bow = Bow {
            v: &v[..k],
            force: &fs[..k],
            rough: &zeros[..k],
            friction: Friction::Hyperbolic,
        };
        s.render(
            &zeros[..k],
            Some(bow),
            1.0,
            &mut out[at..at + k],
            Some(&mut slips[at..at + k]),
            false,
        );
    }
    Bowed {
        slips,
        out,
        period: sr as f64 / f,
        beta,
    }
}

#[test]
fn the_violin_s_helmholtz_region_has_schelleng_s_wedge() {
    // Schoonderwaldt, Guettler & Askenfelt 2008 bowed the violin's D and E strings at 5–20
    // cm/s over β 0.02–0.2: the upper limit near Schelleng's (in its modified form), the
    // lower falling steeply with β. Here the open D and E at 10 cm/s.
    for ix in [1usize, 3] {
        let st = Instrument::Violin.spec().strings[ix];
        let mut rows = Vec::new();
        for beta in [0.04f64, 0.06, 0.09, 0.13, 0.18] {
            let fmax = schelleng_max(st.z0(), 0.1, beta);
            let mut held = Vec::new();
            for i in 0..18 {
                let f = fmax * 0.03 * math::powf(2.0f64, i as f64 * 0.4);
                let b = bow_violin(ix, beta, f as f32, 0.1, 0.6);
                let from = (0.35 * SR as f64 * factor(Instrument::Violin) as f64) as usize;
                if classify(&b, from) == Motion::Helmholtz {
                    held.push(f);
                }
            }
            let lo = held.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = held.iter().cloned().fold(0.0f64, f64::max);
            rows.push((beta, lo, hi));
        }
        println!("violin string {ix} at 10 cm/s, Helmholtz between (N):");
        for (b, lo, hi) in &rows {
            println!("  β {b:.2}: {lo:.3} – {hi:.3}");
        }
        for (b, lo, hi) in &rows {
            assert!(lo.is_finite() && hi > lo, "string {ix} β {b}: no region");
        }
        // The wedge: the region widens towards the fingerboard (the lower limit falls faster
        // than the upper), and the lower limit falls with β.
        let (first, last) = (rows[0], rows[rows.len() - 1]);
        assert!(last.1 < first.1, "the lower limit falls with β: {rows:?}");
        assert!(
            first.2 / first.1 < last.2 / last.1,
            "the region widens with β: {rows:?}"
        );
    }
}

#[test]
fn the_vibrato_moves_the_violin_s_partials_through_its_fitted_body() {
    // D4 on the D string: low enough that the vibrato's sweep of the sixth harmonic stays
    // inside the measure's 20 ms window (a fixed heterodyne hears a partial sweeping out of
    // its window as a swing of its own).
    let key = 62u8;
    let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    let render = |body: f32| {
        let mut a = Arcus::new(SR);
        a.set(&ArcusSettings {
            instrument: Instrument::Violin,
            body_mix: body,
            vib_delay_ms: 0.0,
            vib_attack_ms: 1.0,
            vib_var: 0.0,
            ..ArcusSettings::default()
        });
        a.note_on(key, 0.6, 1);
        let frames = (2.5 * SR) as usize;
        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
        for at in (0..frames).step_by(256) {
            let n = 256.min(frames - at);
            a.process(&mut l[at..at + n], &mut r[at..at + n]);
        }
        l
    };
    let swing = |x: &[f32], n: usize| {
        // The harmonic's amplitude every 5 ms over 20 ms (Hann), its 95th over 5th percentile.
        let sr = SR as f64;
        let w = std::f64::consts::TAU * f0 * n as f64 / sr;
        let (len, step) = ((0.02 * sr) as usize, (0.005 * sr) as usize);
        let mut e = Vec::new();
        let mut i = (0.8 * sr) as usize;
        while i + len < x.len() {
            let (mut re, mut im) = (0.0, 0.0);
            for (j, v) in x[i..i + len].iter().enumerate() {
                let (s, c) = math::sin_cos(w * (i + j) as f64);
                let h = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * j as f64 / len as f64);
                re += h * *v as f64 * c;
                im += h * *v as f64 * s;
            }
            e.push((re * re + im * im).sqrt().max(1e-12));
            i += step;
        }
        e.sort_by(|a, b| a.total_cmp(b));
        20.0 * math::log10(e[e.len() * 19 / 20] / e[e.len() / 20])
    };
    let (wet, dry) = (render(1.0), render(0.0));
    let sw: Vec<f64> = (1..=6).map(|n| swing(&wet, n)).collect();
    let sd: Vec<f64> = (1..=6).map(|n| swing(&dry, n)).collect();
    println!("violin D4: each harmonic's swing with the body {sw:.1?} dB, without {sd:.1?}");
    let (bw, dw) = (
        sw.iter().cloned().fold(0.0, f64::max),
        sd.iter().cloned().fold(0.0, f64::max),
    );
    // The AM is the body's: present with it, each partial its own, gone without it. Its
    // size is about half what the study measured on real violins (each partial ±1.5–2.2 dB,
    // `study/bowed-realism.md` §2.6): the fitted body is smoother than a violin's fine
    // structure (said in `docs/round42/arcus-upper.md`).
    assert!(
        bw > 1.0 && bw > 2.0 * dw,
        "with {bw:.1} dB, without {dw:.1}"
    );
}

/// The body's response to an impulse, `frames` long, at 48 kHz.
fn body_impulse(inst: Instrument, frames: usize) -> Vec<f32> {
    let s = ArcusSettings {
        instrument: inst,
        ..ArcusSettings::default()
    };
    let mut b = body::UpperBody::new(SR);
    b.set(body::shape_of(&s));
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    l[0] = 1.0;
    r[0] = 1.0;
    b.process(&mut l, &mut r, 0, false);
    l
}

#[test]
fn the_upper_body_is_its_fitted_modes_and_comes_to_rest() {
    let modes = body::VIOLIN;
    assert!(modes.len() >= 8, "fitted modes: {}", modes.len());
    for inst in [Instrument::Violin, Instrument::Viola] {
        let x = body_impulse(inst, 1 << 16);
        assert!(x.iter().all(|v| v.is_finite()));
        let e: f64 = x.iter().map(|v| math::powi(*v as f64, 2)).sum();
        // Its power gain to white noise is one.
        assert!((e - 1.0).abs() < 0.02, "{inst:?}: energy {e}");
        let tail = x[x.len() - 4096..].iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(tail < 1e-4, "{inst:?}: still ringing {tail}");
    }
    // The viola's strongest low mode is the violin's at its scale.
    let peak = |x: &[f32], lo: f64, hi: f64| {
        let n = x.len();
        let (mut re, mut im) = (x.to_vec(), vec![0.0f32; n]);
        crate::spectrum::fft(&mut re, &mut im);
        let hz = SR as f64 / n as f64;
        (1..n / 2)
            .filter(|b| (*b as f64 * hz) > lo && (*b as f64 * hz) < hi)
            .max_by(|a, b| {
                let pa = math::powi(re[*a], 2) + math::powi(im[*a], 2);
                let pb = math::powi(re[*b], 2) + math::powi(im[*b], 2);
                pa.total_cmp(&pb)
            })
            .map_or(0.0, |b| b as f64 * hz)
    };
    let pv = peak(&body_impulse(Instrument::Violin, 1 << 16), 380.0, 650.0);
    let pa = peak(&body_impulse(Instrument::Viola, 1 << 16), 330.0, 570.0);
    println!("the strongest signature mode: violin {pv:.1} Hz, viola {pa:.1} Hz");
    assert!(
        (pv / pa / body::VIOLA_SCALE as f64 - 1.0).abs() < 0.01,
        "{pv} / {pa}"
    );
    // Not in use for the cello: it touches nothing.
    let mut b = body::UpperBody::new(SR);
    b.set(body::shape_of(&ArcusSettings::default()));
    let (mut l, mut r) = (vec![0.3f32; 64], vec![-0.2f32; 64]);
    b.process(&mut l, &mut r, 0, false);
    assert!(l.iter().all(|v| *v == 0.3) && r.iter().all(|v| *v == -0.2));
}

#[test]
fn the_way_back_is_flat_to_20_khz_and_folds_nothing_audible() {
    for os in [2usize, 4] {
        let n = 1 << 14;
        let mut x = vec![0.0f32; n * os];
        x[64 * os] = 1.0;
        let mut d = Decimator::new();
        let mut y = vec![0.0f32; n];
        for (k, chunk) in y.chunks_mut(256).enumerate() {
            d.run(os, &x[k * 256 * os..(k + 1) * 256 * os], chunk);
        }
        let (mut re, mut im) = (y.clone(), vec![0.0f32; n]);
        crate::spectrum::fft(&mut re, &mut im);
        let hz = SR as f64 / n as f64;
        let mut ripple = 0f64;
        for b in 1..n / 2 {
            let f = b as f64 * hz;
            if f <= 20_000.0 {
                // An impulse at the high rate spreads over `os` bands; a tone in the passband
                // comes back whole: the band's share is `1/os`.
                let db = 10.0
                    * math::log10(math::powi(re[b] as f64, 2) + math::powi(im[b] as f64, 2))
                    + 20.0 * math::log10(os as f64);
                ripple = ripple.max(db.abs());
            }
        }
        // What lands under 20 kHz from above the rate: a tone at the high rate, from 28 kHz
        // (whose alias would fall at 20 kHz) to the high rate's Nyquist, and what comes back.
        let mut stop = f64::MIN;
        let hi_rate = SR as f64 * os as f64;
        let mut f = 28_000.0;
        while f < 0.5 * hi_rate {
            // Where it lands at the rate: only what folds under 20 kHz is heard.
            let fold = (f - SR as f64 * (f / SR as f64).round()).abs();
            if fold > 20_000.0 {
                f *= 1.013;
                continue;
            }
            let len = 8192;
            let x: Vec<f32> = (0..len * os)
                .map(|i| math::sin(std::f64::consts::TAU * f * i as f64 / hi_rate) as f32)
                .collect();
            let mut d = Decimator::new();
            let mut y = vec![0.0f32; len];
            for (k, chunk) in y.chunks_mut(256).enumerate() {
                d.run(os, &x[k * 256 * os..(k + 1) * 256 * os], chunk);
            }
            let tail = &y[1024..];
            let rms = (tail.iter().map(|v| math::powi(*v as f64, 2)).sum::<f64>()
                / tail.len() as f64)
                .sqrt();
            stop = stop.max(20.0 * math::log10((rms * 2f64.sqrt()).max(1e-12)));
            f *= 1.013;
        }
        println!(
            "{os}×: passband ripple ±{ripple:.4} dB to 20 kHz; what folds under 20 kHz at most {stop:.1} dB; latency {:.2} samples",
            Decimator::latency(os)
        );
        assert!(ripple < 0.01, "{os}×: ripple {ripple}");
        assert!(stop < -90.0, "{os}×: stop band {stop}");
    }
}
