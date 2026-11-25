//! r42:arcus-p6 — pass 6's probes and gates (`docs/round42/arcus-match.md` *Pass 6*): the
//! brightness by the bow's absolute force (the friction's memory, `string::Curve::lag_s`, `slip_dc`).

use super::string::Friction;
use super::tables::{Instrument, FRICTION_MEM};
use super::tests::SR;
use super::tests_gate::{bow_gesture, classify, Motion, CORNER, CURVE};

/// The probes' friction curve from `ARCUS_V0`, `ARCUS_MUS`, `ARCUS_MUD` (the default's
/// otherwise).
fn env_curve() {
    let get = |k: &str, d: f32| {
        std::env::var(k)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(d)
    };
    let d = super::string::Curve::default();
    CURVE.with(|c| {
        c.set(Some(super::string::Curve {
            mu_s: get("ARCUS_MUS", d.mu_s),
            mu_d: get("ARCUS_MUD", d.mu_d),
            v0: get("ARCUS_V0", d.v0),
            ..d
        }))
    });
}
use super::tests_p4::centroid;
use super::*;
use crate::math;

/// The variants a probe runs: `ARCUS_MEM` = the friction's memories `τ:D_c` in µs and µm of
/// slip, `a:b,…` (`spec` for the table's), default the table's and none.
fn variants() -> Vec<(String, Option<(f32, f32)>)> {
    let v = std::env::var("ARCUS_MEM").unwrap_or_else(|_| "spec,0:0".into());
    v.split(',')
        .map(|t| {
            let t = t.trim();
            let c = match t {
                "spec" => None,
                _ => {
                    let mut p = t.split(':').map(|x| x.parse::<f32>().unwrap_or(0.0) * 1e-6);
                    Some((p.next().unwrap_or(0.0), p.next().unwrap_or(0.0)))
                }
            };
            (format!("mem {t}"), c)
        })
        .collect()
}

/// Least squares of `y` on the columns of `x` and an intercept: the coefficients (the
/// intercept last).
pub(super) fn regress(x: &[Vec<f64>], y: &[f64]) -> Vec<f64> {
    let k = x[0].len() + 1;
    let mut a = vec![vec![0.0f64; k + 1]; k];
    for (row, yi) in x.iter().zip(y) {
        let mut r = row.clone();
        r.push(1.0);
        for i in 0..k {
            for j in 0..k {
                a[i][j] += r[i] * r[j];
            }
            a[i][k] += r[i] * yi;
        }
    }
    for c in 0..k {
        let p = (c..k)
            .max_by(|i, j| a[*i][c].abs().partial_cmp(&a[*j][c].abs()).unwrap())
            .unwrap();
        a.swap(c, p);
        let d = a[c][c];
        if d.abs() < 1e-12 {
            continue;
        }
        for j in c..=k {
            a[c][j] /= d;
        }
        for i in 0..k {
            if i != c {
                let f = a[i][c];
                for j in c..=k {
                    a[i][j] -= f * a[c][j];
                }
            }
        }
    }
    (0..k).map(|i| a[i][k]).collect()
}

/// The harmonic centroid (in harmonics) of `x` near `f0`.
fn harm_centroid(x: &[f32], f0: f64) -> f64 {
    let (_, hs) = centroid(x, f0);
    let p: f64 = hs.iter().map(|d| math::powf(10.0, d / 10.0)).sum();
    hs.iter()
        .enumerate()
        .map(|(i, d)| (i + 1) as f64 * math::powf(10.0, d / 10.0))
        .sum::<f64>()
        / p
}

/// The cello's A string on Lampis's grid (`tests_p4::probe_lampis`'s: β 0.099–0.197, speed
/// 0.05–0.25 m/s, force 0.1–0.5 N, 1 m/s²) with each contact: the harmonic centroid's
/// regression on log₂ force, β and speed (the real string's +0.10, −0.04, +0.02 octave a
/// doubling), its median (the real 2.01), and how many points are Helmholtz motion.
#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p6::probe_contact_lampis -- --ignored --nocapture"]
fn probe_contact_lampis() {
    let inst = Instrument::Cello;
    CORNER.with(|c| c.set(inst.spec().corner_hz));
    let wide = std::env::var("ARCUS_WIDE").is_ok();
    for (name, c) in variants() {
        FRICTION_MEM.with(|x| x.set(c));
        env_curve();
        let (mut xs, mut ys, mut hcs) = (Vec::new(), Vec::new(), Vec::new());
        let mut helm = 0;
        let mut total = 0;
        let forces: &[f32] = if wide {
            &[0.1, 0.3, 0.5, 1.0, 2.0]
        } else {
            &[0.1, 0.3, 0.5]
        };
        for beta in [0.099, 0.148, 0.197] {
            for v in [0.05f32, 0.13, 0.25] {
                for &force in forces {
                    total += 1;
                    let b = bow_gesture(
                        inst,
                        3,
                        57.0,
                        beta,
                        (force, force),
                        v,
                        1.0,
                        1.2,
                        Friction::Memory,
                    );
                    if classify(&b, (0.6 * SR) as usize) != Motion::Helmholtz {
                        continue;
                    }
                    helm += 1;
                    let hc = harm_centroid(&b.out[(0.6 * SR) as usize..], 220.0);
                    xs.push(vec![
                        math::log2(force as f64),
                        math::log2(beta),
                        math::log2(v as f64),
                    ]);
                    ys.push(math::log2(hc));
                    hcs.push(hc);
                }
            }
        }
        hcs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let k = if xs.len() > 4 {
            regress(&xs, &ys)
        } else {
            vec![f64::NAN; 4]
        };
        println!(
            "{name:14}: Helmholtz {helm}/{total}; slope oct/doubling F {:+.3} β {:+.3} v {:+.3}; centroid median {:.2} ({:.2} … {:.2})",
            k[0],
            k[1],
            k[2],
            hcs.get(hcs.len() / 2).copied().unwrap_or(f64::NAN),
            hcs.first().copied().unwrap_or(f64::NAN),
            hcs.last().copied().unwrap_or(f64::NAN),
        );
    }
    FRICTION_MEM.with(|x| x.set(None));
    CURVE.with(|c| c.set(None));
    CORNER.with(|c| c.set(3000.0));
}

/// One held note of `inst` at `key` and `velocity` (defaults; vibrato and noise off unless
/// `live`): its steady stretch (1.2–2.2 s).
pub(super) fn held(inst: Instrument, key: u8, velocity: f32, live: bool) -> Vec<f32> {
    let mut a = Arcus::new(SR);
    let d = ArcusSettings::default();
    a.set(&ArcusSettings {
        instrument: inst,
        vib_depth: if live { d.vib_depth } else { 0.0 },
        bow_noise_db: if live { d.bow_noise_db } else { -80.0 },
        friction: Friction::Memory,
        ..d
    });
    a.note_on(key, velocity, 1);
    let frames = (2.2 * SR) as usize;
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for at in (0..frames).step_by(256) {
        let n = 256.min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
    }
    l[(1.2 * SR) as usize..].to_vec()
}

/// The keys a probe plays on each instrument: its four open strings' fifths and the top
/// string an octave up.
pub(super) fn keys_of(inst: Instrument) -> Vec<u8> {
    let s = inst.spec();
    let mut k: Vec<u8> = s.strings.iter().map(|x| x.open + 7).collect();
    k.push(s.strings[3].open + 12);
    k
}

/// The whole instrument under the players' map: each key at pp, mf and ff (velocity 0.3, 0.6,
/// 0.9; vibrato and noise off), its centroid (Hz) and the brightening pp → ff in octaves —
/// the real string's law would give about +0.2 (+0.10 a doubling, the force ×4.5).
#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p6::probe_contact_map -- --ignored --nocapture"]
fn probe_contact_map() {
    let insts: Vec<Instrument> = match std::env::var("ARCUS_INST").as_deref() {
        Ok("cello") => vec![Instrument::Cello],
        Ok("bass") => vec![Instrument::Bass],
        Ok("violin") => vec![Instrument::Violin],
        Ok("viola") => vec![Instrument::Viola],
        _ => Instrument::ALL.to_vec(),
    };
    for (name, c) in variants() {
        FRICTION_MEM.with(|x| x.set(c));
        for inst in &insts {
            let mut line = format!("{name:12} {inst:?}:");
            let mut rises = Vec::new();
            for key in keys_of(*inst) {
                let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
                let cs: Vec<f64> = [0.3f32, 0.6, 0.9]
                    .iter()
                    .map(|v| centroid(&held(*inst, key, *v, false), f0).0)
                    .collect();
                let rise = math::log2(cs[2] / cs[0]);
                rises.push(rise);
                line += &format!(
                    " {key}: {:.0}/{:.0}/{:.0} ({rise:+.2})",
                    cs[0], cs[1], cs[2]
                );
            }
            rises.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!("{line} | median rise {:+.2} oct", rises[rises.len() / 2]);
        }
    }
    FRICTION_MEM.with(|x| x.set(None));
}

/// The string alone (`ARCUS_INST` cello or bass; `ARCUS_KEYS`, default three registers)
/// bowed with the players' map at pp, mf and ff with each friction memory (`ARCUS_MEM`) and
/// bridge corner (`ARCUS_CORNERS`, Hz, 0 none; default the instrument's): its harmonic
/// centroid, the tail's level (h11–h20 against the sawtooth's), and Helmholtz or not.
#[test]
#[ignore = "a probe: cargo test -p cantus-dsp --release --lib arcus::tests_p6::probe_string_map -- --ignored --nocapture"]
fn probe_string_map() {
    use super::voice::{attack_of, bowing};
    let inst = match std::env::var("ARCUS_INST").as_deref() {
        Ok("bass") => Instrument::Bass,
        _ => Instrument::Cello,
    };
    let spec = inst.spec();
    let keys: Vec<u8> = std::env::var("ARCUS_KEYS")
        .map(|v| v.split(',').filter_map(|k| k.parse().ok()).collect())
        .unwrap_or_else(|_| match inst {
            Instrument::Bass => vec![35, 45, 55],
            _ => vec![43, 57, 69],
        });
    let corners: Vec<f64> = std::env::var("ARCUS_CORNERS")
        .map(|v| v.split(',').filter_map(|k| k.parse().ok()).collect())
        .unwrap_or_else(|_| vec![spec.corner_hz]);
    for (name, lag) in variants() {
        FRICTION_MEM.with(|x| x.set(lag));
        env_curve();
        let mut rises = Vec::new();
        for &key in &keys {
            let ix = spec.string_for(key as f32);
            let st = spec.strings[ix];
            let f0 = 440.0 * math::exp2((key as f64 - 69.0) / 12.0);
            for &corner in &corners {
                let mut line = format!("{name} key {key} corner {corner:5.0}:");
                let mut hcs = Vec::new();
                for d in [0.3, 0.6, 0.9] {
                    let s = ArcusSettings::default();
                    let bw = bowing(spec, d, 0.0, 0.0, 1.0, st.z0(), SR as f64 / f0);
                    let (accel, fa) = attack_of(&s, d as f32, &bw, false, 1.0 / f0);
                    CORNER.with(|c| c.set(corner));
                    let b = bow_gesture(
                        inst,
                        ix,
                        key as f64,
                        bw.beta,
                        (fa, bw.force as f32),
                        bw.speed as f32,
                        accel,
                        1.4,
                        Friction::Memory,
                    );
                    let m = classify(&b, (0.6 * SR) as usize);
                    let seg = &b.out[(0.6 * SR) as usize..];
                    let hc = harm_centroid(seg, f0);
                    let (_, hs) = centroid(seg, f0);
                    let tail: Vec<f64> = (10..20.min(hs.len()))
                        .map(|i| hs[i] - hs[0] + 20.0 * math::log10((i + 1) as f64))
                        .collect();
                    let tail = tail.iter().sum::<f64>() / tail.len().max(1) as f64;
                    hcs.push(hc);
                    line += &format!(
                        " d{d}: F {:.2} N hc {hc:.2} tail {tail:+.1} dB {m:?} |",
                        bw.force
                    );
                }
                rises.push(math::log2(hcs[2] / hcs[0]));
                println!("{line} rise {:+.2}", math::log2(hcs[2] / hcs[0]));
            }
        }
        rises.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{name}: median rise pp→ff {:+.2} oct",
            rises[rises.len() / 2]
        );
    }
    FRICTION_MEM.with(|x| x.set(None));
    CURVE.with(|c| c.set(None));
    CORNER.with(|c| c.set(3000.0));
}
