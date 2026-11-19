//! The technician's pass (`DSP_VERSION` 10; ADR 0098's fourth amendment,
//! `docs/round40/chorda-tune.md`): every key of the shipped voicing measured as a tuner
//! hears it (`tunekit`) against the curve its keyboard is designed to.

use super::design::{Keyboard, Tuning};
use super::tests_engine::{perform, Ev};
use super::tunekit::{measure, KeyTune};
use super::voicing::Voicing;
use super::*;
use std::sync::{Arc, Mutex, OnceLock};

const FS: f64 = 48_000.0;
/// 3.4 s: the beat's window reaches 3.3 s.
const FRAMES: usize = 163_200;

/// The keyboard `v` is designed for at the knobs' defaults, made once per tuning.
pub fn keyboard_of(v: &Voicing) -> Arc<Keyboard> {
    static K: OnceLock<Mutex<Vec<(Tuning, Arc<Keyboard>)>>> = OnceLock::new();
    let t = v.tuning(Tuning::default());
    let cache = K.get_or_init(|| Mutex::new(Vec::new()));
    if let Some(k) = cache.lock().unwrap().iter().find(|e| e.0 == t) {
        return k.1.clone();
    }
    let kb = Arc::new(Keyboard::compute(t, FS, 8));
    cache.lock().unwrap().push((t, kb.clone()));
    kb
}

/// The curve `v`'s keys are measured against: its keyboard's keys without the technician's
/// table (the smooth stretch the design draws).
pub fn curve_of(v: &Voicing) -> [tables::Key; tables::KEYS] {
    let mut t = v.tuning(Tuning::default());
    t.tune = [0.0; tables::KEYS];
    tables::keyboard_for(&t)
}

/// A model-2 unit voiced `v` on its own keyboard.
pub fn unit(v: &Voicing) -> Chorda {
    let mut c = Chorda::new(FS as f32);
    c.set_keyboard(keyboard_of(v));
    let mut s = ChordaSettings {
        model: 2,
        ..ChordaSettings::default()
    };
    // A diagnostic's sympathetic strings (`CHORDA_TUNE_SYM`; the default's otherwise).
    if let Some(x) = std::env::var("CHORDA_TUNE_SYM")
        .ok()
        .and_then(|x| x.parse().ok())
    {
        s.sympathetic = x;
    }
    if let Some(x) = std::env::var("CHORDA_TUNE_BODY")
        .ok()
        .and_then(|x| x.parse().ok())
    {
        s.body = x;
    }
    c.set(&s, 0.0);
    c.set_voicing(v);
    c
}

/// Key `m` struck alone at `vel`, measured against its designed partials (1 … 12).
pub fn key_tune(v: &Voicing, m: u8, vel: f32) -> KeyTune {
    let (l, r) = perform(&mut unit(v), &[(0, Ev::On(m, vel, 1))], FRAMES, 512, 1);
    let kb = curve_of(v);
    let key = kb[(m - tables::LOWEST) as usize];
    let near: Vec<f64> = (1..=12).map(|n| key.partial(n)).collect();
    let f = |x: &[f32]| x.iter().map(|s| *s as f64).collect::<Vec<f64>>();
    measure(&f(&l), &f(&r), FS, key.f0(), &near)
}

/// Every key in `keys` at `vel`, on `threads` threads, in key order.
pub fn map(v: &Voicing, keys: &[u8], vel: f32, threads: usize) -> Vec<(u8, KeyTune)> {
    let _ = keyboard_of(v);
    let out = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for t in 0..threads {
            let out = &out;
            s.spawn(move || {
                for m in keys.iter().skip(t).step_by(threads) {
                    let k = key_tune(v, *m, vel);
                    out.lock().unwrap().push((*m, k));
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by_key(|e| e.0);
    o
}

/// The voicing a diagnostic reads: the shipped one, `CHORDA_TUNE_SPEC` applied over it.
fn diag_voicing() -> Voicing {
    let mut v = Voicing::default();
    if let Ok(spec) = std::env::var("CHORDA_TUNE_SPEC") {
        assert!(v.apply(&spec), "a key of CHORDA_TUNE_SPEC is not one");
    }
    v
}

/// Diagnostic: every key A0–C8 at three velocities — its pitch against its designed curve
/// (the ear's weighing), partials 1–3, and its beats. `CHORDA_TUNE_SPEC` over the shipped
/// voicing; `CHORDA_TUNE_KEYS=lo-hi`. Run with
/// `cargo test --release -p cantus-dsp --lib tuning_map -- --ignored --nocapture`.
#[test]
#[ignore]
fn tuning_map() {
    let v = diag_voicing();
    let (lo, hi) = std::env::var("CHORDA_TUNE_KEYS")
        .ok()
        .and_then(|s| {
            let (a, b) = s.split_once('-')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((21u8, 108u8));
    let keys: Vec<u8> = (lo..=hi).collect();
    let vels = [0.35f32, 0.6, 0.85];
    let maps: Vec<Vec<(u8, KeyTune)>> = vels.iter().map(|vel| map(&v, &keys, *vel, 8)).collect();
    let kb = curve_of(&v);
    eprintln!(
        "key  ET-ct | pitch v.35 v.60 v.85 | p1 p2 p3 (v.60) | beat med/worst v.35 v.60 v.85"
    );
    for (i, m) in keys.iter().enumerate() {
        let key = kb[(*m - tables::LOWEST) as usize];
        let et = 440.0 * crate::math::powf(2.0f64, (*m as f64 - 69.0) / 12.0);
        let design = 1200.0 * crate::math::log2(key.f1 / et);
        let k6 = &maps[1][i].1;
        let pc = |n: u32| {
            k6.partials
                .iter()
                .find(|p| p.n == n)
                .map_or(f64::NAN, |p| p.cents)
        };
        eprintln!(
            "{m:3} {design:+6.1} | {:+5.1} {:+5.1} {:+5.1} | {:+5.1} {:+5.1} {:+5.1} | {:4.1}/{:4.1} {:4.1}/{:4.1} {:4.1}/{:4.1}",
            maps[0][i].1.pitch,
            maps[1][i].1.pitch,
            maps[2][i].1.pitch,
            pc(1),
            pc(2),
            pc(3),
            maps[0][i].1.beat,
            maps[0][i].1.beat_worst,
            maps[1][i].1.beat,
            maps[1][i].1.beat_worst,
            maps[2][i].1.beat,
            maps[2][i].1.beat_worst,
        );
    }
}

/// Diagnostic: each partial of the keys `CHORDA_TUNE_KEYS` (a comma list) at velocity 0.6 —
/// cents against the design, level, beat — and the loop design's own worst partial.
#[test]
#[ignore]
fn tuning_partials() {
    let v = diag_voicing();
    let keys: Vec<u8> = std::env::var("CHORDA_TUNE_KEYS")
        .unwrap_or_else(|_| "46".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let kb = keyboard_of(&v);
    for (m, k) in map(&v, &keys, 0.6, 8) {
        let i = (m - tables::LOWEST) as usize;
        eprintln!(
            "key {m}: pitch {:+.1} beat {:.1}/{:.1}; design worst {:.2} ct, {} sections, B {:.2e}",
            k.pitch,
            k.beat,
            k.beat_worst,
            kb.loops[i].worst_cents,
            kb.loops[i].sections,
            kb.keys[i].b
        );
        for p in &k.partials {
            eprintln!(
                "   p{:2} {:7.1} Hz {:+6.1} ct {:6.1} dB beat {:4.1}",
                p.n, p.near_hz, p.cents, p.level_db, p.beat_db
            );
        }
    }
}

/// Diagnostic: a recorded keys-at-mf file (`CHORDA_TUNE_WAV`, the voicing lab's `keys_mf.wav`:
/// key `k` struck at `(k − 21)·4` s) measured with the same kit against the shipped voicing's
/// designed partials — so a limit can come from a real piano. Run with
/// `CHORDA_TUNE_WAV=… cargo test -p cantus-dsp --lib tuning_wav -- --ignored --nocapture`.
#[test]
#[ignore]
fn tuning_wav() {
    let Ok(path) = std::env::var("CHORDA_TUNE_WAV") else {
        return;
    };
    let mut rd = hound::WavReader::open(&path).expect("a wav");
    let spec = rd.spec();
    let ch = spec.channels as usize;
    let xs: Vec<f64> = match spec.sample_format {
        hound::SampleFormat::Float => rd.samples::<f32>().map(|s| s.unwrap() as f64).collect(),
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f64;
            rd.samples::<i32>()
                .map(|s| s.unwrap() as f64 / full)
                .collect()
        }
    };
    let fs = spec.sample_rate as f64;
    let n = xs.len() / ch;
    let l: Vec<f64> = (0..n).map(|i| xs[i * ch]).collect();
    let r: Vec<f64> = (0..n).map(|i| xs[i * ch + ch.min(2) - 1]).collect();
    let v = Voicing::default();
    let kb = curve_of(&v);
    eprintln!("key | pitch | p1 p2 p3 p4 p6 p8 | beat med/worst");
    for m in 21u8..=108 {
        let a = ((m as usize - 21) as f64 * 4.0 * fs) as usize;
        let b = (a + (3.4 * fs) as usize).min(n);
        if b <= a + (fs as usize) {
            break;
        }
        let key = kb[(m - tables::LOWEST) as usize];
        let near: Vec<f64> = (1..=12).map(|k| key.partial(k)).collect();
        let k = measure(&l[a..b], &r[a..b], fs, key.f0(), &near);
        let pc = |n: u32| {
            k.partials
                .iter()
                .find(|p| p.n == n)
                .map_or(f64::NAN, |p| p.cents)
        };
        eprintln!(
            "{m:3} | {:+5.1} | {:+5.1} {:+5.1} {:+5.1} {:+5.1} {:+5.1} {:+5.1} | {:4.1}/{:4.1}",
            k.pitch,
            pc(1),
            pc(2),
            pc(3),
            pc(4),
            pc(6),
            pc(8),
            k.beat,
            k.beat_worst
        );
    }
}

/// Write a stereo 24-bit WAV at 48 kHz.
fn write_wav(path: &std::path::Path, l: &[f32], r: &[f32]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 48_000,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("a wav");
    let full = (1 << 23) as f32 - 1.0;
    for (a, b) in l.iter().zip(r) {
        for x in [a, b] {
            w.write_sample((x.clamp(-1.0, 1.0) * full).round() as i32)
                .unwrap();
        }
    }
    w.finalize().unwrap();
}

/// Notes `(start s, length s, key, velocity 0…1)` played on `v`, `tail` seconds after the last.
fn play(v: &Voicing, notes: &[(f64, f64, u8, f32)], tail: f64) -> (Vec<f32>, Vec<f32>) {
    let mut ev = Vec::new();
    let mut end: f64 = 0.0;
    for (i, (t, len, k, vel)) in notes.iter().enumerate() {
        let id = i as u64 + 1;
        ev.push(((t * FS) as usize, Ev::On(*k, *vel, id)));
        ev.push((((t + len) * FS) as usize, Ev::Off(*k, id)));
        end = end.max(t + len);
    }
    ev.sort_by_key(|e| e.0);
    perform(&mut unit(v), &ev, ((end + tail) * FS) as usize, 512, 1)
}

/// The listening set (`compartir\chorda-tune\`): `CHORDA_RENDER_OUT` the folder,
/// `CHORDA_RENDER_VOICING` `before` (candidate 16 as master 2a79ded2 shipped it) or `after`
/// (the shipped voicing). Run with
/// `cargo test -p cantus-dsp --lib tune_renders -- --ignored --nocapture`.
#[test]
#[ignore]
fn tune_renders() {
    let Ok(out) = std::env::var("CHORDA_RENDER_OUT") else {
        return;
    };
    let which = std::env::var("CHORDA_RENDER_VOICING").unwrap_or_else(|_| "after".into());
    let v = if which == "before" {
        Voicing::candidate16()
    } else {
        Voicing::default()
    };
    let dir = std::path::PathBuf::from(out);
    std::fs::create_dir_all(&dir).unwrap();
    let save = |name: &str, (l, r): (Vec<f32>, Vec<f32>)| {
        let peak = l.iter().chain(&r).fold(0.0f32, |m, x| m.max(x.abs()));
        eprintln!("{which} {name}: peak {peak:.3}");
        write_wav(&dir.join(format!("{name}-{which}.wav")), &l, &r);
    };
    let mf = 75.0 / 127.0;
    // A chromatic run A1–C4, up then down.
    let mut run = Vec::new();
    let keys: Vec<u8> = (33..=60).chain((33..60).rev()).collect();
    for (i, k) in keys.iter().enumerate() {
        run.push((i as f64 * 0.4, 0.38, *k, mf));
    }
    save("run-A1-C4", play(&v, &run, 2.5));
    // Octaves, fifths and chords around Bb2–C3 (each 2.5 s, then 0.5 s apart).
    let groups: [&[u8]; 14] = [
        &[34, 46],
        &[35, 47],
        &[36, 48],
        &[46, 58],
        &[47, 59],
        &[48, 60],
        &[46, 53],
        &[47, 54],
        &[48, 55],
        &[34, 46, 50, 53],
        &[35, 47, 51, 54],
        &[36, 48, 52, 55],
        &[46, 50, 53, 58],
        &[48, 52, 55, 60],
    ];
    let mut chords = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        for k in g.iter() {
            chords.push((i as f64 * 3.0, 2.5, *k, mf));
        }
    }
    save("octaves-chords-Bb2-C3", play(&v, &chords, 2.0));
    // Each worst key held 3.5 s.
    let worst: [u8; 14] = [23, 28, 32, 34, 37, 38, 43, 46, 47, 48, 51, 54, 55, 61];
    let held: Vec<_> = worst
        .iter()
        .enumerate()
        .map(|(i, k)| (i as f64 * 4.0, 3.5, *k, mf))
        .collect();
    save("worst-keys-held", play(&v, &held, 1.0));
    // Every key A0–C8, 0.3 s each.
    let sweep: Vec<_> = (21u8..=108)
        .enumerate()
        .map(|(i, k)| (i as f64 * 0.3, 0.28, k, mf))
        .collect();
    save("sweep-A0-C8", play(&v, &sweep, 2.5));
    // The lab's keys_mf (every key at velocity 75, one every 4 s) for its tuning gate.
    if std::env::var("CHORDA_RENDER_KEYS").is_ok() {
        let keys: Vec<_> = (21u8..=108)
            .enumerate()
            .map(|(i, k)| (i as f64 * 4.0, 3.5, k, mf))
            .collect();
        let (l, r) = play(&v, &keys, 0.5);
        write_wav(&dir.join(format!("keys_mf-{which}.wav")), &l, &r);
    }
}

/// Diagnostic: one key's strings alone (no hammer scatter, no board), struck, at couplings
/// `0 … 0.95` and one or three strings — its first partial against the design.
#[test]
#[ignore]
fn tuning_coupling() {
    use super::string::{loss_steep, KeyStrings, MAX_STRINGS};
    let v = Voicing::default();
    let kb = keyboard_of(&v);
    for m in [69u8, 84, 95, 99, 105] {
        let i = (m - tables::LOWEST) as usize;
        let (k, d) = (kb.keys[i], kb.loops[i]);
        for strings in [1usize, 3] {
            let mut line = format!("key {m} strings {strings}:");
            for cp in [0.0, 0.25, 0.5, 0.95] {
                let mut s = KeyStrings::new(d.delay + 16, FS);
                s.tune_strings(&k, &d, [0.0; MAX_STRINGS], strings);
                let (g, a, yz) = loss_steep(&k, FS, cp, 1.0, 0.0, 0.0, 1.0);
                s.set_loss(g, a, yz);
                let felt = super::string::Felt {
                    k: k.hammer_k,
                    p: k.hammer_p,
                    eps: 0.9,
                    tau: 1.0e-5,
                };
                s.strike(felt, k.hammer_mass, 2.0, [1.0; MAX_STRINGS]);
                let x: Vec<f64> = (0..48_000).map(|_| s.tick()).collect();
                let f = super::testkit::partial_hz(&x, FS, k.f1, 0.05 * k.f1, 4_800, 32_768);
                line += &format!(
                    "  cp {cp}: yz {yz:.2e} {:+.2}c",
                    1200.0 * crate::math::log2(f / k.f1)
                );
            }
            eprintln!("{line}");
        }
    }
}

/// Diagnostic: how far the technician's pass scaled each key's bridge resonances
/// (`CHORDA_TUNE_SPEC` over the shipped voicing), A0–C6.
#[test]
#[ignore]
fn tuning_bridge_scale() {
    let v = diag_voicing();
    let mut line = String::new();
    for m in 21u8..=84 {
        let mut c = unit(&v);
        let _ = perform(&mut c, &[(0, Ev::On(m, 0.6, 1))], 2_048, 512, 1);
        let s = c
            .key_strings((m - tables::LOWEST) as usize)
            .bridge_gain_scale();
        line += &format!("{m}:{s:.2} ");
    }
    eprintln!("{line}");
}

/// Diagnostic: the keys `CHORDA_TUNE_KEYS` (a comma list) at each draw `0 … CHORDA_DRAWS`
/// (`CHORDA_TUNE_SPEC` over the shipped voicing) — the deepest beat of the three strongest
/// partials at three velocities, the key's pitch and its first partial at 0.6.
#[test]
#[ignore]
fn tuning_draws() {
    let base = diag_voicing();
    let keys: Vec<u8> = std::env::var("CHORDA_TUNE_KEYS")
        .unwrap_or_else(|_| "46".into())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    let most: u8 = std::env::var("CHORDA_DRAWS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(12);
    let jobs: Vec<(u8, u8)> = keys
        .iter()
        .flat_map(|k| (0..=most).map(move |d| (*k, d)))
        .collect();
    let out = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|s| {
        for t in 0..threads {
            let (out, jobs, base) = (&out, &jobs, &base);
            s.spawn(move || {
                for (m, d) in jobs.iter().skip(t).step_by(threads) {
                    let mut v = *base;
                    v.draws[(*m - tables::LOWEST) as usize] = *d;
                    let r: Vec<KeyTune> = [0.35f32, 0.6, 0.85]
                        .iter()
                        .map(|x| key_tune(&v, *m, *x))
                        .collect();
                    out.lock().unwrap().push((*m, *d, r));
                }
            });
        }
    });
    let mut o = out.into_inner().unwrap();
    o.sort_by_key(|e| (e.0, e.1));
    for (m, d, r) in o {
        let p1 = r[1]
            .partials
            .iter()
            .find(|p| p.n == 1)
            .map_or(f64::NAN, |p| p.cents);
        eprintln!(
            "key {m} draw {d:2}: beat {:4.1} {:4.1} {:4.1} (med {:4.1}) pitch {:+5.1} p1 {:+5.1}",
            r[0].beat_worst, r[1].beat_worst, r[2].beat_worst, r[1].beat, r[1].pitch, p1
        );
    }
}

/// Diagnostic: each key's fraction and its slope per cent on the shipped keyboard — how many
/// cents up and down its strings can be detuned before the fraction meets its ±0.99 clamp.
#[test]
#[ignore]
fn tuning_fractions() {
    let v = diag_voicing();
    let kb = keyboard_of(&v);
    for (i, d) in kb.loops.iter().enumerate() {
        let up = (0.99 - d.frac) / d.frac_per_cent;
        let down = (-0.99 - d.frac) / d.frac_per_cent;
        let (lo, hi) = if up < down { (up, down) } else { (down, up) };
        eprintln!(
            "key {} frac {:+.3} per cent {:+.4}: {lo:+.1} … {hi:+.1} cents",
            i + 21,
            d.frac,
            d.frac_per_cent
        );
    }
}
