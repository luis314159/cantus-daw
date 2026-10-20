//! Generated rhythm material with known answers (round 29, ADR 0061): click tracks and drum
//! loops at a tempo, with swing — for the onset and tempo tests and the accuracy corpus in
//! `cantus-bench`. Deterministic (hash-seeded noise).

use crate::math;
use crate::pitch::testkit::Noise;
use std::f32::consts::PI;

/// A piece of generated rhythm and where its events really are.
pub struct Material {
    pub audio: Vec<f32>,
    pub rate: f32,
    pub bpm: f64,
    /// Every hit's first sample.
    pub onsets: Vec<f64>,
    /// Every beat's sample (quarter notes from the first downbeat).
    pub beats: Vec<f64>,
}

/// A kick: a falling sine thump.
fn kick(out: &mut [f32], at: usize, rate: f32, gain: f32) {
    let mut phase = 0.0f32;
    let len = (0.6 * rate) as usize;
    for k in 0..len {
        let Some(o) = out.get_mut(at + k) else { break };
        let t = k as f32 / rate;
        let f = 50.0 + 90.0 * math::exp(-t / 0.03);
        phase += 2.0 * PI * f / rate;
        let env = math::exp(-t / 0.12) * (k as f32 / (0.0005 * rate)).min(1.0) * tail(k, len, rate);
        *o += gain * env * math::sin(phase);
    }
}

/// A snare: noise and a 190 Hz body.
fn snare(out: &mut [f32], at: usize, rate: f32, gain: f32, noise: &mut Noise) {
    let len = (0.35 * rate) as usize;
    for k in 0..len {
        let Some(o) = out.get_mut(at + k) else { break };
        let t = k as f32 / rate;
        let a = (k as f32 / (0.0003 * rate)).min(1.0);
        let body = math::sin(2.0 * PI * 190.0 * t) * math::exp(-t / 0.05);
        *o += gain
            * a
            * tail(k, len, rate)
            * (0.6 * noise.next() * math::exp(-t / 0.07) + 0.4 * body);
    }
}

/// A closed hi-hat: short bright noise (a first difference of white noise).
fn hat(out: &mut [f32], at: usize, rate: f32, gain: f32, noise: &mut Noise) {
    let mut last = 0.0f32;
    for k in 0..(0.05 * rate) as usize {
        let Some(o) = out.get_mut(at + k) else { break };
        let t = k as f32 / rate;
        let n = noise.next();
        let hp = n - last;
        last = n;
        let a = (k as f32 / (0.0002 * rate)).min(1.0);
        *o += gain * a * hp * math::exp(-t / 0.012);
    }
}

/// A click: a 1 ms burst of a 2 kHz tone (accented on the bar's first beat).
fn click(out: &mut [f32], at: usize, rate: f32, gain: f32, hz: f32) {
    for k in 0..(0.02 * rate) as usize {
        let Some(o) = out.get_mut(at + k) else { break };
        let t = k as f32 / rate;
        *o += gain * math::sin(2.0 * PI * hz * t) * math::exp(-t / 0.004);
    }
}

fn beats_of(bpm: f64, rate: f32, first: f64, n: usize) -> Vec<f64> {
    let per = 60.0 / bpm * rate as f64;
    (0..n).map(|i| first + i as f64 * per).collect()
}

/// A metronome at `bpm` for `bars` bars of 4/4, starting `lead` seconds in.
pub fn click_track(rate: f32, bpm: f64, bars: usize, lead: f64) -> Material {
    let per = 60.0 / bpm * rate as f64;
    let first = (lead * rate as f64).round();
    let len = (first + per * (4 * bars) as f64 + 0.5 * rate as f64) as usize;
    let mut audio = vec![0.0f32; len];
    let beats = beats_of(bpm, rate, first, 4 * bars);
    let mut onsets = Vec::new();
    for (i, b) in beats.iter().enumerate() {
        let at = b.round() as usize;
        let accent = i % 4 == 0;
        click(
            &mut audio,
            at,
            rate,
            if accent { 0.8 } else { 0.5 },
            if accent { 2_000.0 } else { 1_500.0 },
        );
        onsets.push(at as f64);
    }
    Material {
        audio,
        rate,
        bpm,
        onsets,
        beats,
    }
}

/// A drum loop at `bpm`: kick on 1 and 3 (and the "and" of 3 every other bar), snare on 2
/// and 4, hats on eighths with `swing` (0 straight … 1 a full triplet feel: the off-beat
/// eighth late by `swing / 3` of a beat), for `bars` bars, starting `lead` seconds in.
pub fn drum_loop(rate: f32, bpm: f64, bars: usize, swing: f64, lead: f64, seed: u64) -> Material {
    let per = 60.0 / bpm * rate as f64;
    let first = (lead * rate as f64).round();
    let len = (first + per * (4 * bars) as f64 + 0.3 * rate as f64) as usize;
    let mut audio = vec![0.0f32; len];
    let mut noise = Noise::new(seed);
    let beats = beats_of(bpm, rate, first, 4 * bars);
    let mut onsets = Vec::new();
    for bar in 0..bars {
        for beat in 0..4 {
            let b = beats[bar * 4 + beat];
            let at = b.round() as usize;
            if beat == 0 || beat == 2 {
                kick(&mut audio, at, rate, 0.9);
            }
            if beat == 1 || beat == 3 {
                snare(&mut audio, at, rate, 0.6, &mut noise);
            }
            hat(&mut audio, at, rate, 0.25, &mut noise);
            onsets.push(at as f64);
            let off = (b + per * (0.5 + swing / 6.0)).round() as usize;
            hat(&mut audio, off, rate, 0.18, &mut noise);
            onsets.push(off as f64);
            if beat == 2 && bar % 2 == 1 {
                kick(&mut audio, off, rate, 0.7);
            }
        }
    }
    onsets.sort_by(f64::total_cmp);
    onsets.dedup();
    Material {
        audio,
        rate,
        bpm,
        onsets,
        beats,
    }
}

/// A 5 ms fade at the end of a generated sound of `len` samples (no click where it stops).
fn tail(k: usize, len: usize, rate: f32) -> f32 {
    let f = (0.005 * rate) as usize;
    ((len - k) as f32 / f as f32).min(1.0)
}
