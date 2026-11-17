//! Purus's demo take, for the design sheets and the screenshot tool (no engine running): a
//! strummed E minor on a guitar-like string model in a small tiled room, a ground loop's
//! 50.2 Hz hum with its harmonics under it, a dozen clicks and a passage of crackle — run
//! through the real unit (`cantus_engine::fx::purus::Unit`) at the editor's settings, its
//! telemetry read block by block as the editor reads it, and both sides of it cut into the
//! screen's columns by the live path (`picture::Live`).

use super::picture::{Live, Picture};

use cantus_core::EffectSpec;
use cantus_dsp::math::{exp, sin};
use cantus_engine::fx::{FxUnit, Telemetry};
use std::f32::consts::TAU;

const SR: f32 = 48_000.0;
const SECONDS: f32 = 7.0;

/// A number in −1 … 1 from a running seed: the same every run.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// The take: (left, right).
pub fn take() -> (Vec<f32>, Vec<f32>) {
    let n = (SECONDS * SR) as usize;
    let mut dry = vec![0.0f32; n];
    // E minor strummed every 0.75 s, each string a sum of decaying partials.
    let strings = [82.41f32, 123.47, 164.81, 196.0, 246.94, 329.63];
    let mut at = 0usize;
    let mut chord = 0;
    while at < n {
        for (s, f0) in strings.iter().enumerate() {
            let f = if chord % 2 == 1 && s == 4 { 220.0 } else { *f0 };
            let start = at + s * 220;
            for k in 1..=10 {
                let fk = f * k as f32 * (1.0 + 0.0004 * (k * k) as f32);
                if fk > 9_000.0 {
                    break;
                }
                let a = 0.05 / k as f32;
                let tau = 0.45 / k as f32;
                for (i, v) in dry
                    .iter_mut()
                    .enumerate()
                    .skip(start)
                    .take((tau * 4.0 * SR) as usize)
                {
                    let x = (i - start) as f32 / SR;
                    *v += a * exp(-x / tau) * sin(TAU * fk * x);
                }
            }
        }
        at += (1.4 * SR) as usize;
        chord += 1;
    }
    // The tiled room: four combs and an allpass, its tail about 1.1 s.
    let mut wet = vec![0.0f32; n];
    for (d, g) in [
        (1_557usize, 0.86f32),
        (1_617, 0.85),
        (1_491, 0.86),
        (1_422, 0.87),
    ] {
        let mut line = vec![0.0f32; d];
        let mut p = 0;
        for i in 0..n {
            let y = line[p];
            line[p] = dry[i] + g * y;
            p = (p + 1) % d;
            wet[i] += y * 0.18;
        }
    }
    let mut seed = 0x51ED_2701u32;
    let mut left = vec![0.0f32; n];
    let mut right = vec![0.0f32; n];
    for i in 0..n {
        let x = i as f32 / SR;
        // The hum: 50.2 Hz, its odd harmonics stronger, about −34 dBFS.
        let mut hum = 0.0;
        for k in 1..=9 {
            let a = if k % 2 == 1 { 0.026 } else { 0.010 } / (k as f32).sqrt();
            hum += a * sin(TAU * 50.0 * k as f32 * x + k as f32 * 0.7);
        }
        let room = wet[i];
        left[i] = dry[i] + room + hum;
        right[i] = dry[i] * 0.94 + room * 1.06 + hum;
    }
    // Clicks: a dozen sharp ones; crackle from 3.5 s to 5.5 s.
    for c in 0..12 {
        let i = (((c as f32 + 0.37) / 12.0) * (SECONDS - 0.4) * SR) as usize + 2_000;
        let a = 0.35 + 0.25 * noise(&mut seed).abs();
        let sgn = if noise(&mut seed) > 0.0 { 1.0 } else { -1.0 };
        for (j, w) in [1.0f32, -0.6, 0.3].iter().enumerate() {
            left[i + j] += sgn * a * w;
            right[i + j] += sgn * a * w * 0.9;
        }
    }
    let mut i = (3.5 * SR) as usize;
    while i < (5.5 * SR) as usize {
        let a = 0.04 * noise(&mut seed);
        left[i] += a;
        right[i] += a * 0.8;
        i += 120 + (noise(&mut seed).abs() * 300.0) as usize;
    }
    (left, right)
}

/// The unit at `spec` over the take: its last telemetry and the screen's picture — the
/// unit's (input, output) pairs and its telemetry fed block by block through the live path
/// ([`Live`]), as the engine's tap feeds the editor.
pub fn picture(spec: &EffectSpec) -> (Telemetry, Picture) {
    let (l0, r0) = take();
    let mut unit = cantus_engine::fx::purus::Unit::new(SR);
    unit.configure(spec);
    unit.reset();
    let mut pic = Picture::default();
    let mut live = Live::new(SR);
    let mut last = [0.0; cantus_engine::fx::TELEMETRY_LEN];
    let mut pairs = Vec::with_capacity(480);
    for (bl, br) in l0.chunks(480).zip(r0.chunks(480)) {
        let mut l = bl.to_vec();
        let mut r = br.to_vec();
        unit.process(&mut l, &mut r, None);
        last = unit.telemetry();
        pic.update(&last, 0.01);
        pairs.clear();
        for i in 0..l.len() {
            pairs.push((0.5 * (bl[i] + br[i]), 0.5 * (l[i] + r[i])));
        }
        live.feed(&pairs, &mut pic);
    }
    pic.hold_demo();
    (last, pic)
}
