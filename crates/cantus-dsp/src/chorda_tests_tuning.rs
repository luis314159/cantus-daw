//! The technician's pass (`DSP_VERSION` 10; ADR 0098's fourth amendment,
//! `docs/round-fx/chorda-tuning.md`): every key of the shipped voicing measured as a tuner
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

/// A model-2 unit voiced `v` on its own keyboard.
pub fn unit(v: &Voicing) -> Chorda {
    let mut c = Chorda::new(FS as f32);
    c.set_keyboard(keyboard_of(v));
    let s = ChordaSettings {
        model: 2,
        ..ChordaSettings::default()
    };
    c.set(&s, 0.0);
    c.set_voicing(v);
    c
}

/// Key `m` struck alone at `vel`, measured against its designed partials (1 … 12).
pub fn key_tune(v: &Voicing, m: u8, vel: f32) -> KeyTune {
    let (l, r) = perform(&mut unit(v), &[(0, Ev::On(m, vel, 1))], FRAMES, 512, 1);
    let kb = keyboard_of(v);
    let key = kb.keys[(m - tables::LOWEST) as usize];
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
    let kb = keyboard_of(&v);
    eprintln!("key  ET-ct | pitch v.35 v.60 v.85 | p1 p2 p3 (v.60) | beat med/worst v.35 v.60 v.85");
    for (i, m) in keys.iter().enumerate() {
        let key = kb.keys[(*m - tables::LOWEST) as usize];
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
            k.pitch, k.beat, k.beat_worst, kb.loops[i].worst_cents, kb.loops[i].sections, kb.keys[i].b
        );
        for p in &k.partials {
            eprintln!(
                "   p{:2} {:7.1} Hz {:+6.1} ct {:6.1} dB beat {:4.1}",
                p.n, p.near_hz, p.cents, p.level_db, p.beat_db
            );
        }
    }
}
