//! Diagnostic (the owner, 2026-11-18: "Bb2, B2, C3 sound out of tune"): each key's first
//! partials in the shipped voicing — their pitch against equal temperament and how deep
//! their unison beats — printed. Run with
//! `cargo test -p cantus-dsp --lib tunemap -- --ignored --nocapture`.

use super::tests_engine::{perform, Ev};
use super::voicing::Voicing;
use super::*;
use crate::math;

fn voiced(v: &Voicing) -> Chorda {
    let mut c = super::tests_model2::piano2();
    c.set_voicing(v);
    c
}

/// The level (dB) of the partial nearest `hz` in hops of `hop` samples, `win` long
/// (Goertzel at the measured frequency).
fn envelope(x: &[f64], fs: f64, hz: f64, win: usize, hop: usize) -> Vec<f64> {
    let w = 2.0 * std::f64::consts::PI * hz / fs;
    let coeff = 2.0 * math::cos(w);
    let mut out = Vec::new();
    let mut s = 0;
    while s + win <= x.len() {
        let (mut q1, mut q2) = (0.0, 0.0);
        for i in 0..win {
            let hann = 0.5 - 0.5 * math::cos(2.0 * std::f64::consts::PI * i as f64 / win as f64);
            let q0 = coeff * q1 - q2 + x[s + i] * hann;
            q2 = q1;
            q1 = q0;
        }
        let p = q1 * q1 + q2 * q2 - coeff * q1 * q2;
        out.push(10.0 * math::log10(p.max(1e-30)));
        s += hop;
    }
    out
}

/// How far a decaying envelope swings about its straight (dB-linear) fit: the beat's depth.
fn swing(env: &[f64]) -> f64 {
    let n = env.len() as f64;
    let mx = (n - 1.0) / 2.0;
    let my = env.iter().sum::<f64>() / n;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (i, y) in env.iter().enumerate() {
        sxy += (i as f64 - mx) * (y - my);
        sxx += (i as f64 - mx) * (i as f64 - mx);
    }
    let slope = sxy / sxx;
    let r: Vec<f64> = env
        .iter()
        .enumerate()
        .map(|(i, y)| y - (my + slope * (i as f64 - mx)))
        .collect();
    let hi = r.iter().cloned().fold(f64::MIN, f64::max);
    let lo = r.iter().cloned().fold(f64::MAX, f64::min);
    hi - lo
}

#[test]
#[ignore]
fn tunemap() {
    let fs = 48_000.0;
    let v = Voicing::default();
    for m in 36..=60u8 {
        let x: Vec<f64> = perform(
            &mut voiced(&v),
            &[(0, Ev::On(m, 0.6, 1))],
            4 * 48_000,
            512,
            1,
        )
        .0
        .iter()
        .map(|s| *s as f64)
        .collect();
        let et = 440.0 * math::powf(2.0f64, (m as f64 - 69.0) / 12.0);
        let mut line = format!("key {m:3}");
        for k in 1..=3u32 {
            let near = et * k as f64;
            let hz = super::testkit::partial_hz(&x, fs, near * 1.003, near * 0.03, 4_800, 32_768);
            let ct = 1200.0 * math::log2(hz / near);
            // 0.1–3 s, 85 ms windows every 21 ms: the swing about the decay.
            let env = envelope(&x[4_800..3 * 48_000], fs, hz, 4_096, 1_024);
            line += &format!("  p{k} {ct:+5.1}c beat {:4.1}dB", swing(&env));
        }
        eprintln!("{line}");
    }
}
