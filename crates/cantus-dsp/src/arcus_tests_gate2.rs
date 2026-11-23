//! Arcus's gate, asserted (ADR 0157 §"What the tests hold"): the Schelleng region's shape,
//! the attacks at every default velocity, the vibrato's partials moving through the body and
//! not without it, the body and the bridge's reflection passive. The bowed pitch's own gate
//! is `arcus_tests_pitch.rs`. Each prints what it measured (`--nocapture`), for the report.

use super::string::Friction;
use super::tables::{schelleng_max, Instrument};
use super::tests::SR;
use super::tests_gate::{attack_time, bow_string, classify, onsets, Bowed, Motion};
use super::*;
use crate::math;

/// The Helmholtz span at `beta` for a bow at `speed` on the cello's open D: the least and
/// the most force (N) of the grid that keep it, or `None`.
fn span(beta: f64, speed: f32) -> Option<(f64, f64)> {
    let inst = Instrument::Cello;
    let z0 = inst.spec().strings[2].z0();
    let fmax = schelleng_max(z0, speed as f64, beta);
    let mut held = Vec::new();
    for i in 0..18 {
        let frac = 0.03 * math::powf(2.0f64, i as f64 * 0.4);
        let f = fmax * frac;
        let b = bow_string(inst, 2, 50.0, beta, f as f32, speed, 20.0, 0.8, Friction::Hyperbolic);
        if classify(&b, (0.5 * SR) as usize) == Motion::Helmholtz {
            held.push(f);
        }
    }
    let lo = held.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = held.iter().cloned().fold(0.0f64, f64::max);
    (!held.is_empty()).then_some((lo, hi))
}

/// The slope of `ln y` against `ln x`, least squares.
fn slope(pts: &[(f64, f64)]) -> f64 {
    let n = pts.len() as f64;
    let lx: Vec<f64> = pts.iter().map(|p| math::ln(p.0)).collect();
    let ly: Vec<f64> = pts.iter().map(|p| math::ln(p.1)).collect();
    let (mx, my) = (lx.iter().sum::<f64>() / n, ly.iter().sum::<f64>() / n);
    let (mut num, mut den) = (0.0, 0.0);
    for (x, y) in lx.iter().zip(&ly) {
        num += (x - mx) * (y - my);
        den += (x - mx) * (x - mx);
    }
    num / den
}

#[test]
fn the_helmholtz_region_has_schelleng_s_shape() {
    let z0 = Instrument::Cello.spec().strings[2].z0();
    for speed in [0.1f32, 0.2] {
        let mut upper = Vec::new();
        let mut lower = Vec::new();
        for beta in [0.06f64, 0.08, 0.1, 0.13, 0.17, 0.22] {
            let (lo, hi) = span(beta, speed)
                .unwrap_or_else(|| panic!("no Helmholtz motion at β {beta}, {speed} m/s"));
            let fmax = schelleng_max(z0, speed as f64, beta);
            println!(
                "{speed} m/s β {beta:.2}: Helmholtz from {lo:.3} N to {hi:.3} N ({:.2}–{:.2} of Schelleng's F_max {fmax:.2} N; ratio {:.1})",
                lo / fmax,
                hi / fmax,
                hi / lo
            );
            upper.push((beta, hi));
            lower.push((beta, lo));
            assert!(
                hi / fmax > 0.4 && hi / fmax < 2.5,
                "the upper limit at β {beta}: {hi} N against Schelleng's {fmax}"
            );
        }
        let (su, sl) = (slope(&upper), slope(&lower));
        println!("{speed} m/s: the upper limit goes as β^{su:.2}, the lower as β^{sl:.2}");
        // Schelleng: the upper ∝ 1/β, the lower ∝ 1/β² — the region closes towards the
        // bridge. Here the lower limit keeps Schelleng's steepness and the upper falls more
        // gently than his (the torsion's yield lets a sticking bow hold more at a long β —
        // `docs/round42/arcus.md`): both fall, the lower faster, the region widening towards
        // the fingerboard.
        assert!(su < 0.0, "upper slope {su}");
        assert!(sl < su - 0.6, "lower slope {sl} against upper {su}");
        let (lo0, hi0) = (lower[2].1, upper[2].1);
        println!("{speed} m/s β 0.10: lower {lo0:.3} N, upper {hi0:.3} N");
    }
}

/// A bowed note of `inst` at `key` and `velocity` with the instrument's defaults (no
/// vibrato, so nothing but the attack moves): the slips of its first 0.5 s.
fn attack(inst: Instrument, key: u8, velocity: f32) -> Bowed {
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
    let f = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    let st = inst.spec().strings[inst.spec().string_for(key as f32)];
    let _ = st;
    Bowed {
        slips: std::mem::take(&mut a.voices[v].slips),
        out: l,
        period: SR as f64 / f,
        beta: a.voices[v].beta() as f64,
    }
}

/// Whether the transient before the Helmholtz motion slipped more than once a period
/// somewhere (a multiple-flyback attack, Guettler & Askenfelt's 90 ms class), up to `t`.
fn multiple_flyback(b: &Bowed, until_s: f64) -> bool {
    let end = ((until_s * SR as f64) as usize).min(b.slips.len());
    let o = onsets(&b.slips[..end], 0, b.period);
    o.windows(2)
        .any(|w| ((w[1] - w[0]) as f64) < 0.75 * b.period)
}

#[test]
fn the_attack_settles_within_guettler_and_askenfelt_s_limits() {
    let mut worst = (0.0f64, String::new());
    let mut over = Vec::new();
    let mut count = 0;
    for inst in [Instrument::Cello, Instrument::Bass] {
        let spec = inst.spec();
        for st in spec.strings.iter() {
            for step in [0u8, 7] {
                let key = st.open + step;
                for vel in [0.15f32, 0.35, 0.55, 0.75, 0.95] {
                    let b = attack(inst, key, vel);
                    let t = attack_time(&b).unwrap_or(0.5);
                    // Guettler & Askenfelt's limits, 50 ms (about 10 periods of the violin's
                    // open G, where they were judged) and 90 ms (18) for a multiple flyback:
                    // in ms, or in those periods for a string slower than that G (the cello's
                    // C2 has a period of 15 ms, the bass's E1 of 24).
                    let periods = (b.period / SR as f64) / (1.0 / 196.0);
                    let limit =
                        if multiple_flyback(&b, t) { 0.09 } else { 0.05 } * periods.max(1.0);
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
    println!("attacks over their limit: {} of {count}", over.len());
    for l in &over {
        println!("  {l}");
    }
    println!("worst against its limit: {}", worst.1);
    assert!(
        over.is_empty(),
        "{} of {count} attacks over Guettler & Askenfelt's limits",
        over.len()
    );
}

/// The amplitude of harmonic `n` of `f0` in `x` over time: the signal heterodyned to `n·f0`
/// and smoothed over 20 ms (wide enough for the vibrato's excursion, narrow enough for its
/// rate), sampled every 5 ms from `from` s.
fn envelope(x: &[f32], f0: f64, n: usize, from: f64) -> Vec<f64> {
    let sr = SR as f64;
    let w = std::f64::consts::TAU * f0 * n as f64 / sr;
    let len = (0.02 * sr) as usize;
    let step = (0.005 * sr) as usize;
    let start = (from * sr) as usize;
    let mut out = Vec::new();
    let mut i = start;
    while i + len < x.len() {
        let (mut re, mut im) = (0.0, 0.0);
        for (j, v) in x[i..i + len].iter().enumerate() {
            let (s, c) = math::sin_cos(w * (i + j) as f64);
            // A Hann window, so a neighbour's leakage stays out.
            let h = 0.5 - 0.5 * math::cos(std::f64::consts::TAU * j as f64 / len as f64);
            re += h * *v as f64 * c;
            im += h * *v as f64 * s;
        }
        out.push((re * re + im * im).sqrt());
        i += step;
    }
    out
}

/// The depth of an envelope's movement, dB (its 95th over its 5th percentile).
fn swing_db(e: &[f64]) -> f64 {
    let mut v: Vec<f64> = e.to_vec();
    v.sort_by(|a, b| a.total_cmp(b));
    let lo = v[v.len() / 20].max(1e-12);
    let hi = v[v.len() * 19 / 20].max(1e-12);
    20.0 * math::log10(hi / lo)
}

#[test]
fn the_vibrato_moves_each_partial_differently_through_the_body_and_not_without_it() {
    let key = 57u8; // A3 on the D string
    let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    let render = |body: f32| {
        let mut a = Arcus::new(SR);
        a.set(&ArcusSettings {
            body_mix: body,
            vib_delay_ms: 0.0,
            vib_attack_ms: 1.0,
            vib_var: 0.0,
            ..ArcusSettings::default()
        });
        a.note_on(key, 0.6, 1);
        let frames = (3.0 * SR) as usize;
        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
        for at in (0..frames).step_by(256) {
            let n = 256.min(frames - at);
            a.process(&mut l[at..at + n], &mut r[at..at + n]);
        }
        l
    };
    // The vibrato's rate and depth, from the fundamental's phase.
    let dry = render(0.0);
    let fund = envelope(&dry, f0, 1, 0.8);
    assert!(fund.iter().all(|e| *e > 0.0));
    let swings_body: Vec<f64> = (1..=8)
        .map(|n| swing_db(&envelope(&render(1.0), f0, n, 0.8)))
        .collect();
    let swings_dry: Vec<f64> = (1..=8).map(|n| swing_db(&envelope(&dry, f0, n, 0.8))).collect();
    println!("each harmonic's swing with the body (dB): {swings_body:.1?}");
    println!("… without it: {swings_dry:.1?}");
    let mean = swings_body.iter().sum::<f64>() / 8.0;
    let spread = (swings_body.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / 8.0).sqrt();
    let dry_max = swings_dry.iter().cloned().fold(0.0, f64::max);
    let body_max = swings_body.iter().cloned().fold(0.0, f64::max);
    // Through the body the partials swing, each its own way (Fletcher & Sanders 1967;
    // Mellody & Wakefield 2000); without it they hardly move.
    assert!(body_max > 3.0, "the strongest swing with the body: {body_max:.1} dB");
    assert!(spread > 1.0, "the swings' spread: {spread:.2} dB");
    assert!(body_max > 2.0 * dry_max, "with {body_max:.1} dB, without {dry_max:.1} dB");
}

#[test]
fn the_vibrato_s_rate_and_depth_are_its_defaults() {
    let key = 57u8;
    let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        body_mix: 0.0,
        vib_delay_ms: 0.0,
        vib_attack_ms: 1.0,
        vib_var: 0.0,
        ..ArcusSettings::default()
    });
    a.note_on(key, 0.6, 1);
    // The string's frequency read from the voice every control step.
    let mut cents = Vec::new();
    let (mut l, mut r) = (vec![0.0; 32], vec![0.0; 32]);
    for _ in 0..(3.0 * SR / 32.0) as usize {
        a.process(&mut l, &mut r);
        let v = a.newest();
        cents.push(1200.0 * math::log2(v.f0() / f0));
    }
    let tail = &cents[cents.len() / 3..];
    let max = tail.iter().cloned().fold(f64::MIN, f64::max);
    let min = tail.iter().cloned().fold(f64::MAX, f64::min);
    let crossings = tail.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count();
    let secs = tail.len() as f64 * 32.0 / SR as f64;
    let rate = crossings as f64 / secs;
    println!("vibrato at A3: {rate:.2} Hz, ±{:.1} cents", 0.5 * (max - min));
    // Geringer & Allen 2004: ~5.5 Hz; the gate's ±15–25 cents (a cello's A3, its D string's
    // fifth: the depth widens a little above the bottom of the range, MacLeod 2008).
    assert!((5.3..=6.2).contains(&rate), "{rate} Hz");
    let depth = 0.5 * (max - min);
    assert!((15.0..=25.0).contains(&depth), "±{depth} cents");
}

/// The magnitude of `ir`'s spectrum at `hz`.
fn magnitude(ir: &[f32], hz: f64) -> f64 {
    let w = std::f64::consts::TAU * hz / SR as f64;
    let (mut re, mut im) = (0.0, 0.0);
    for (n, x) in ir.iter().enumerate() {
        let (s, c) = math::sin_cos(w * n as f64);
        re += *x as f64 * c;
        im -= *x as f64 * s;
    }
    (re * re + im * im).sqrt()
}

#[test]
fn the_bodies_ring_at_their_signature_modes_and_die_away() {
    use crate::fides::body::{body_modes, Body, BodyMode, BODY_MODES};
    for inst in [Instrument::Cello, Instrument::Bass] {
        let s = ArcusSettings {
            instrument: inst,
            ..ArcusSettings::default()
        };
        let bs = s.body();
        let mut body = Body::new(SR);
        body.set(&bs);
        let n = SR as usize;
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        l[0] = 1.0;
        r[0] = 1.0;
        body.process(&mut l, &mut r, 0, false);
        // Passive and stable: what it rings with dies away (the second half's energy a sliver
        // of the first's), and it comes to rest exactly.
        let e = |x: &[f32]| x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
        let (first, second) = (e(&l[..n / 2]), e(&l[n / 2..]));
        assert!(second < 1e-3 * first, "{inst:?}: {second} after {first}");
        let mut modes = [BodyMode::default(); BODY_MODES];
        let count = body_modes(bs.kind, &mut modes);
        assert!(count >= 4);
        // The signature modes (A0, the corpus bending, B1−, B1+) ring where the table puts
        // them over the body's size: each the peak of its neighbourhood within 1 %.
        for m in modes.iter().take(4) {
            let want = m.hz / bs.size as f64;
            let mut best = (0.0f64, 0.0f64);
            for k in -40..=40 {
                let hz = want * (1.0 + 0.002 * k as f64);
                let mag = magnitude(&l, hz);
                if mag > best.1 {
                    best = (hz, mag);
                }
            }
            println!(
                "{inst:?}: a mode of {want:.1} Hz rings at {:.1} Hz",
                best.0
            );
            assert!((best.0 / want - 1.0).abs() <= 0.01, "{inst:?}: {want} → {}", best.0);
        }
    }
}

#[test]
fn a_plucked_string_loses_energy_every_period() {
    // The loop is passive: with nothing feeding it, a plucked string's energy over each
    // period falls (its loss filter, corner and blocker under 1 at every frequency).
    for inst in [Instrument::Cello, Instrument::Bass] {
        let s = ArcusSettings {
            instrument: inst,
            technique: Technique::Pizzicato,
            body_mix: 0.0,
            ..ArcusSettings::default()
        };
        let mut a = Arcus::new(SR);
        a.set(&s);
        let key = inst.spec().bottom() + 9;
        a.note_on(key, 0.9, 1);
        let n = (2.0 * SR) as usize;
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        a.process(&mut l, &mut r);
        let period = (SR as f64 / (440.0 * math::exp2((key as f64 - 69.0) / 12.0))) as usize;
        let energy: Vec<f64> = l[(0.05 * SR) as usize..]
            .chunks(period * 4)
            .map(|c| c.iter().map(|v| (*v as f64).powi(2)).sum::<f64>())
            .collect();
        let rises = energy.windows(2).filter(|w| w[1] > w[0] * 1.0001).count();
        assert!(rises == 0, "{inst:?}: the energy rose {rises} times");
    }
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --lib arcus::tests_gate2::probe -- --ignored --nocapture"]
fn probe_harmonics() {
    let key = 50u8;
    let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    for (name, body, tors) in [("dry", 0.0f32, true), ("body", 1.0, true), ("dry, no torsion", 0.0, false)] {
        let mut a = Arcus::new(SR);
        a.set(&ArcusSettings {
            body_mix: body,
            vib_depth: 0.0,
            ..ArcusSettings::default()
        });
        if !tors {
            for v in a.voices.iter_mut() {
                v.string.set_torsion(None);
            }
        }
        a.note_on(key, 0.6, 1);
        let frames = (1.5 * SR) as usize;
        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
        a.process(&mut l, &mut r);
        let (s0, s1) = ((0.6 * SR) as usize, (1.4 * SR) as usize);
        let h: Vec<f64> = (1..=12).map(|n| magnitude(&l[s0..s1], f0 * n as f64)).collect();
        let rel: Vec<String> = h.iter().map(|x| format!("{:+.1}", 20.0 * math::log10(x / h[0]))).collect();
        println!("{name}: {}", rel.join(" "));
    }
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --lib arcus::tests_gate2::probe -- --ignored --nocapture"]
fn probe_pitch_quantization() {
    use super::tests::{cents, measure_hz};
    use super::tests_gate::CORNER;
    let inst = Instrument::Cello;
    let z0 = inst.spec().strings[3].z0();
    for corner in [0.0f64, 3000.0, 2000.0] {
        CORNER.with(|c| c.set(corner));
        let mut line = format!("corner {corner:5.0}:");
        for c in [-20.0f64, -12.0, -6.0, -3.0, 0.0, 3.0, 6.0, 12.0, 20.0] {
            let key = 64.0 + c / 100.0;
            let fmax = schelleng_max(z0, 0.15, 0.1) as f32;
            let b = bow_string(inst, 3, key, 0.1, 0.42 * fmax, 0.15, 8.0, 1.2, Friction::Hyperbolic);
            let want = 440.0 * math::exp2((key - 69.0) / 12.0);
            let e = cents(measure_hz(&b.out, want, 0.5, 1.15), want);
            line += &format!(" {c:+.0}→{:+.2}", e);
        }
        println!("{line}");
    }
    CORNER.with(|c| c.set(3000.0));
}
