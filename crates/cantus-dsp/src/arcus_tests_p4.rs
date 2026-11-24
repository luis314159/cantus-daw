//! Pass 4's probes and gates (round 42, `docs/round42/arcus-match.md` *Pass 4*): the attack's
//! level against the bow's gesture, the loud cello's brightness against its bowing.

use super::tables::Instrument;
use super::tests::SR;
use super::tests_gate::{classify, Bowed, Motion};
use super::*;
use crate::math;

/// One note at `key`, `velocity` with the instrument's defaults (vibrato off): every 10 ms,
/// the output's level (dB), the bow's speed (m/s) and force (N), the region's limits, and the
/// motion of the slips over the window.
pub(super) struct Trace {
    pub rows: Vec<(f64, f64, f32, f32, f32, f32, Motion)>,
}

pub(super) fn trace(inst: Instrument, key: u8, velocity: f32, seconds: f64) -> Trace {
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        instrument: inst,
        vib_depth: 0.0,
        ..ArcusSettings::default()
    });
    a.note_on(key, velocity, 1);
    let v = a.voices.iter().position(|v| v.active()).expect("a voice");
    a.voices[v].record = true;
    let frames = (seconds * SR as f64) as usize;
    let hop = (0.01 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    let f = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
    let mut rows = Vec::new();
    for at in (0..frames).step_by(hop) {
        let n = hop.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
        let e = l[at..at + n].iter().map(|x| (x * x) as f64).sum::<f64>() / n as f64;
        let vc = &a.voices[v];
        let (lo, hi) = vc.limits();
        let slips = &vc.slips;
        let from = slips.len().saturating_sub((0.05 * SR) as usize);
        let b = Bowed {
            slips: slips[from..].to_vec(),
            out: Vec::new(),
            period: SR as f64 / f,
            beta: vc.beta() as f64,
        };
        let m = if b.slips.len() as f64 > 3.0 * b.period {
            classify(&b, 0)
        } else {
            Motion::Sliding
        };
        rows.push((
            (at + n) as f64 / SR as f64,
            10.0 * math::log10(e + 1e-20),
            vc.bow_velocity(),
            vc.bow_force(),
            lo,
            hi,
            m,
        ));
    }
    Trace { rows }
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p4::probe_onset -- --ignored --nocapture"]
fn probe_onset() {
    let notes: Vec<(Instrument, u8, f32)> = match std::env::var("ARCUS_NOTES") {
        Ok(v) => v
            .split(',')
            .filter_map(|n| {
                let p: Vec<&str> = n.split(':').collect();
                let inst = if p[0] == "bass" {
                    Instrument::Bass
                } else {
                    Instrument::Cello
                };
                Some((inst, p.get(1)?.parse().ok()?, p.get(2)?.parse().ok()?))
            })
            .collect(),
        Err(_) => vec![
            (Instrument::Cello, 51, 0.3),
            (Instrument::Cello, 51, 0.6),
            (Instrument::Cello, 51, 0.9),
            (Instrument::Bass, 30, 0.3),
        ],
    };
    for (inst, key, vel) in notes {
        let t = trace(inst, key, vel, 2.4);
        let steady: f64 = {
            let s: Vec<f64> = t.rows.iter().filter(|r| r.0 > 1.2).map(|r| r.1).collect();
            s.iter().sum::<f64>() / s.len() as f64
        };
        println!("{inst:?} key {key} vel {vel}: steady {steady:.1} dB");
        for r in t
            .rows
            .iter()
            .filter(|r| r.0 <= 0.4 || (r.0 * 100.0).round() as i64 % 20 == 0)
        {
            println!(
                "  {:.2}s {:+6.1} dB  v {:.3}  F {:.3} N  [{:.3}, {:.3}]  {:?}",
                r.0,
                r.1 - steady,
                r.2,
                r.3,
                r.4,
                r.5,
                r.6
            );
        }
    }
}

/// The harmonics' power-weighted centroid (Hz) of `x` (a steady stretch) near `f0`, and each
/// harmonic's level (dB re the strongest): the fundamental found first (a DFT over ±4 % in
/// 0.05 % steps — a bowed string sits flat of its free pitch), each harmonic then searched
/// within ±0.5 Hz of `n` times it.
pub(super) fn centroid(x: &[f32], f0: f64) -> (f64, Vec<f64>) {
    let n = x.len();
    let w: Vec<f64> = (0..n)
        .map(|i| 0.5 - 0.5 * math::cos(std::f64::consts::TAU * i as f64 / n as f64))
        .collect();
    let power = |f: f64| {
        let dw = std::f64::consts::TAU * f / SR as f64;
        let (mut re, mut im) = (0.0, 0.0);
        for (i, (s, wi)) in x.iter().zip(&w).enumerate() {
            let (sn, cs) = math::sin_cos(dw * i as f64);
            re += *s as f64 * wi * cs;
            im += *s as f64 * wi * sn;
        }
        re * re + im * im
    };
    let mut f1 = f0;
    let mut best = 0.0;
    for k in -80..=80 {
        let f = f0 * (1.0 + 0.0005 * k as f64);
        let p = power(f);
        if p > best {
            best = p;
            f1 = f;
        }
    }
    let mut hs = Vec::new();
    let (mut num, mut den) = (0.0, 0.0);
    for h in 1..=40 {
        let fc = h as f64 * f1;
        if fc > 10_000.0 {
            break;
        }
        let p = (-4..=4)
            .map(|k| power(fc + 0.125 * k as f64))
            .fold(0.0f64, f64::max);
        hs.push(p);
        num += fc * p;
        den += p;
    }
    let top = hs.iter().cloned().fold(0.0f64, f64::max).max(1e-30);
    (
        num / den.max(1e-30),
        hs.iter()
            .map(|p| 10.0 * math::log10(p / top + 1e-30))
            .collect(),
    )
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p4::probe_brightness -- --ignored --nocapture"]
fn probe_brightness() {
    use super::string::{Friction, Torsion};
    use super::tests_gate::{bow_gesture, CORNER, TORSION};
    use super::voice::{attack_of, bowing};
    let inst = Instrument::Cello;
    let spec = inst.spec();
    let keys: Vec<u8> = std::env::var("ARCUS_KEYS")
        .map(|v| v.split(',').filter_map(|k| k.parse().ok()).collect())
        .unwrap_or_else(|_| vec![50, 57, 62]);
    for key in keys {
        let ix = spec.string_for(key as f32);
        let st = spec.strings[ix];
        let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
        println!("cello key {key} (string {ix}), the string's bridge force, 0.6–1.4 s:");
        for (name, d, fk, bk, corner, tor) in [
            ("pp", 0.3, 1.0, 1.0, spec.corner_hz, true),
            ("mf", 0.6, 1.0, 1.0, spec.corner_hz, true),
            ("ff F×1.4", 0.9, 1.4, 1.0, spec.corner_hz, true),
            ("ff", 0.9, 1.0, 1.0, spec.corner_hz, true),
            ("ff F×2", 0.9, 2.0, 1.0, spec.corner_hz, true),
            ("ff F×0.5", 0.9, 0.5, 1.0, spec.corner_hz, true),
            ("ff β×0.6", 0.9, 1.0, 0.6, spec.corner_hz, true),
            ("ff corner 12k", 0.9, 1.0, 1.0, 12_000.0, true),
            ("ff no corner", 0.9, 1.0, 1.0, 0.0, true),
            ("ff no torsion", 0.9, 1.0, 1.0, spec.corner_hz, false),
            ("pp no torsion", 0.3, 1.0, 1.0, spec.corner_hz, false),
        ] {
            let s = ArcusSettings::default();
            let bw = bowing(spec, d, 0.0, 0.0, 1.0, st.z0(), SR as f64 / f0);
            let (accel, fa) = attack_of(&s, d as f32, &bw, false, 1.0 / f0);
            CORNER.with(|c| c.set(corner));
            TORSION.with(|c| c.set(if tor { None } else { Some(None) }));
            let _ = Torsion::default();
            let beta = bw.beta * bk;
            let b = bow_gesture(
                inst,
                ix,
                key as f64,
                beta,
                (fa * fk as f32, bw.force as f32 * fk as f32),
                bw.speed as f32,
                accel,
                1.4,
                Friction::Hyperbolic,
            );
            let seg = &b.out[(0.6 * SR) as usize..];
            let (c, hs) = centroid(seg, f0);
            let rms = (seg.iter().map(|x| (x * x) as f64).sum::<f64>() / seg.len() as f64).sqrt();
            let m = classify(&b, (0.6 * SR) as usize);
            println!(
                "  {name:14} v {:.3} β {:.3} F {:.2} N ({:.2} of Schelleng): centroid {:6.0} Hz, level {:+5.1} dB, {m:?}; h2..10 {:?}",
                bw.speed,
                beta,
                bw.force * fk,
                bw.force * fk / bw.schelleng,
                c,
                20.0 * math::log10(rms),
                hs[1..10.min(hs.len())].iter().map(|v| v.round() as i32).collect::<Vec<_>>()
            );
        }
        CORNER.with(|c| c.set(3000.0));
        TORSION.with(|c| c.set(None));
    }
}

/// The cello's A string (open, 220 Hz) bowed as Lampis's robot bowed one (Zenodo 10.5281/
/// zenodo.13374477's static set, CC BY 4.0): β, speed and force on its grid, the bow
/// accelerating at 1 m/s²; the string's bridge force's harmonics over 0.6–1.2 s. The real
/// string's (`tmp/arcus/p4/lampis_harm.py`): a sawtooth at every point, h2…h6 −6, −9.5, −12,
/// −13.5, −15 dB re the fundamental, the harmonic centroid 1.8–2.2, +0.10 octave a doubling
/// of force.
#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p4::probe_lampis -- --ignored --nocapture"]
fn probe_lampis() {
    use super::string::Friction;
    use super::tests_gate::{bow_gesture, CORNER, CURVE, TORSION};
    let inst = Instrument::Cello;
    let spec = inst.spec();
    let variant = std::env::var("ARCUS_VARIANT").unwrap_or_default();
    CORNER.with(|c| c.set(spec.corner_hz));
    if variant.contains("nocorner") {
        CORNER.with(|c| c.set(0.0));
    }
    if variant.contains("notorsion") {
        TORSION.with(|c| c.set(Some(None)));
    }
    if variant.contains("lampiscurve") {
        CURVE.with(|c| {
            c.set(Some(super::string::Curve {
                mu_s: 0.9,
                mu_d: 0.59,
                v0: 0.2,
                ..Default::default()
            }))
        });
    }
    if variant.contains("msw") {
        CURVE.with(|c| {
            c.set(Some(super::string::Curve {
                mu_s: 0.8,
                mu_d: 0.4,
                v0: 0.2,
                ..Default::default()
            }))
        });
    }
    let f0 = 220.0;
    let mut cens = Vec::new();
    let scan = std::env::var("ARCUS_GRID").is_ok_and(|g| g == "force");
    let grid: Vec<(f64, f32, f32, f32)> = if scan {
        let mut g = Vec::new();
        for (beta, v) in [(0.119, 0.179f32), (0.077, 0.255)] {
            for accel in [1.0f32, 7.0] {
                for force in [0.3f32, 0.6, 0.9, 1.3, 1.8, 2.5, 3.5] {
                    g.push((beta, v, force, accel));
                }
            }
        }
        g
    } else {
        let mut g = Vec::new();
        for beta in [0.099, 0.148, 0.197] {
            for v in [0.05f32, 0.13, 0.25] {
                for force in [0.1f32, 0.3, 0.5] {
                    g.push((beta, v, force, 1.0));
                }
            }
        }
        g
    };
    for (beta, v, force, accel) in grid {
        {
            {
                let b = bow_gesture(
                    inst,
                    3,
                    57.0,
                    beta,
                    (force, force),
                    v,
                    accel,
                    1.2,
                    Friction::Hyperbolic,
                );
                let seg = &b.out[(0.6 * SR) as usize..];
                let (c, hs) = centroid(seg, f0);
                let p: f64 = hs.iter().map(|d| math::powf(10.0, d / 10.0)).sum();
                let hc: f64 = hs
                    .iter()
                    .enumerate()
                    .map(|(i, d)| (i + 1) as f64 * math::powf(10.0, d / 10.0))
                    .sum::<f64>()
                    / p;
                let m = classify(&b, (0.6 * SR) as usize);
                let h1 = hs[0];
                cens.push(hc);
                println!(
                    "β {beta:.3} v {v:.2} F {force:.1} a {accel}: centroid {hc:4.2} harm ({c:5.0} Hz) {m:?}; h2..10 re h1 {:?}",
                    hs[1..10].iter().map(|d| (d - h1).round() as i32).collect::<Vec<_>>()
                );
            }
        }
    }
    cens.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "variant '{variant}': harmonic centroid median {:.2}, min {:.2}, max {:.2}",
        cens[cens.len() / 2],
        cens[0],
        cens[cens.len() - 1]
    );
}

#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p4::probe_slip_shape -- --ignored --nocapture"]
fn probe_slip_shape() {
    use super::string::Friction;
    use super::tests_gate::{bow_gesture, CORNER};
    let inst = Instrument::Cello;
    CORNER.with(|c| c.set(inst.spec().corner_hz));
    for force in [0.6f32, 2.5] {
        let b = bow_gesture(
            inst,
            3,
            57.0,
            0.077,
            (force, force),
            0.25,
            1.0,
            1.0,
            Friction::Hyperbolic,
        );
        let p = b.period;
        let from = (0.8 * SR) as usize;
        // one period's slip and output, every sample
        let n = p.ceil() as usize + 2;
        let s: Vec<String> = b.slips[from..from + n]
            .iter()
            .map(|x| format!("{:.2}", x))
            .collect();
        let o: Vec<String> = b.out[from..from + n]
            .iter()
            .map(|x| format!("{:.3}", x))
            .collect();
        println!(
            "F {force}: period {p:.1} samples; slip (m/s):\n {}\n out:\n {}",
            s.join(" "),
            o.join(" ")
        );
    }
}

/// The whole instrument (body and correction on) at a key, pp against ff, with and without
/// its vibrato and its hair noise: the steady window's harmonic centroid (1.2–2.2 s).
#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p4::probe_instrument_brightness -- --ignored --nocapture"]
fn probe_instrument_brightness() {
    let keys: Vec<u8> = std::env::var("ARCUS_KEYS")
        .map(|v| v.split(',').filter_map(|k| k.parse().ok()).collect())
        .unwrap_or_else(|_| vec![50, 64, 70, 76]);
    for key in keys {
        let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
        for (name, vib, noise) in [
            ("default", true, true),
            ("no vibrato", false, true),
            ("no noise", true, false),
            ("neither", false, false),
        ] {
            let mut line = format!("key {key} {name:11}");
            for vel in [0.3f32, 0.6, 0.9] {
                let mut a = Arcus::new(SR);
                let d = ArcusSettings::default();
                a.set(&ArcusSettings {
                    vib_depth: if vib { d.vib_depth } else { 0.0 },
                    bow_noise_db: if noise { d.bow_noise_db } else { -80.0 },
                    ..d
                });
                a.note_on(key, vel, 1);
                let frames = (2.2 * SR) as usize;
                let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
                for at in (0..frames).step_by(256) {
                    let n = 256.min(frames - at);
                    a.process(&mut l[at..at + n], &mut r[at..at + n]);
                }
                let (c, hs) = centroid(&l[(1.2 * SR) as usize..], f0);
                line += &format!(
                    " | v{vel}: {c:5.0} Hz h2..8 {:?}",
                    hs[1..8.min(hs.len())]
                        .iter()
                        .map(|d| (d - hs[0]).round() as i32)
                        .collect::<Vec<_>>()
                );
            }
            println!("{line}");
        }
    }
}

/// The attack gate (`tests_gate2`'s) and the Helmholtz span at β 0.1 on the cello's D, with
/// the friction law `ARCUS_FRICTION` (`thermal` or the default).
#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p4::probe_friction_gate -- --ignored --nocapture"]
fn probe_friction_gate() {
    use super::string::Friction;
    use super::tables::schelleng_max;
    use super::tests_gate::{attack_time, bow_string, onsets};
    let friction = if std::env::var("ARCUS_FRICTION").is_ok_and(|f| f == "thermal") {
        Friction::Thermal
    } else {
        Friction::Hyperbolic
    };
    let (mut over, mut count, mut worst) = (0, 0, 0.0f64);
    for inst in [Instrument::Cello, Instrument::Bass] {
        let spec = inst.spec();
        for st in spec.strings.iter() {
            for step in [0u8, 7] {
                let key = st.open + step;
                for vel in [0.15f32, 0.35, 0.55, 0.75, 0.95] {
                    let mut a = Arcus::new(SR);
                    a.set(&ArcusSettings {
                        instrument: inst,
                        vib_depth: 0.0,
                        friction,
                        ..ArcusSettings::default()
                    });
                    a.note_on(key, vel, 1);
                    let v = a.voices.iter().position(|v| v.active()).expect("a voice");
                    a.voices[v].record = true;
                    let frames = (0.5 * SR) as usize;
                    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
                    for at in (0..frames).step_by(256) {
                        let n = 256.min(frames - at);
                        a.process(&mut l[at..at + n], &mut r[at..at + n]);
                    }
                    let f = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
                    let b = Bowed {
                        slips: std::mem::take(&mut a.voices[v].slips),
                        out: l,
                        period: SR as f64 / f,
                        beta: a.voices[v].beta() as f64,
                    };
                    let t = attack_time(&b).unwrap_or(0.5);
                    let end = ((t * SR as f64) as usize).min(b.slips.len());
                    let o = onsets(&b.slips[..end], 0, b.period);
                    let flyback = o
                        .windows(2)
                        .any(|w| ((w[1] - w[0]) as f64) < 0.75 * b.period);
                    let periods = (b.period / SR as f64) / (1.0 / 196.0);
                    let limit = if flyback { 0.09 } else { 0.05 } * periods.max(1.0);
                    count += 1;
                    worst = worst.max(t / limit);
                    if t > limit {
                        over += 1;
                        println!(
                            "  {inst:?} key {key} vel {vel}: {:.0} ms (limit {:.0})",
                            1000.0 * t,
                            1000.0 * limit
                        );
                    }
                }
            }
        }
    }
    println!(
        "{friction:?}: attacks over their limit {over} of {count}, worst {worst:.2} of its limit"
    );
    let inst = Instrument::Cello;
    let z0 = inst.spec().strings[2].z0();
    for speed in [0.1f32, 0.2] {
        let fmax = schelleng_max(z0, speed as f64, 0.1);
        let mut line = String::new();
        for i in 0..18 {
            let frac = 0.03 * math::powf(2.0f64, i as f64 * 0.4);
            let b = bow_string(
                inst,
                2,
                50.0,
                0.1,
                (fmax * frac) as f32,
                speed,
                20.0,
                0.8,
                friction,
            );
            line += match classify(&b, (0.5 * SR) as usize) {
                Motion::Helmholtz => "H",
                Motion::Multiple => "m",
                Motion::Raucous => "R",
                Motion::Sliding => ".",
            };
        }
        println!("{friction:?} cello D β 0.1 at {speed} m/s, F/Fmax 0.03 … ×1.32 each: {line}");
    }
}
