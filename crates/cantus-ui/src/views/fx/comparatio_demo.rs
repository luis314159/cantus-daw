//! Comparatio's demo, for the design sheets and the screenshot tool (no engine running): a
//! mix in progress (a strummed chord, a bass line, soft drums, mostly in the middle) and a
//! finished master to compare it with (the same music brighter, louder, wider, its peaks
//! held under −1 dBTP) — the real unit (`cantus_engine::fx::comparatio::Unit`) given the
//! reference as the live engine gives it, run at the editor's settings over twelve seconds,
//! its telemetry read block by block as the editor reads it; the tonal curves from the
//! DSP's own analyser (`cantus_dsp::comparatio::tonal`).

use super::picture::Picture;
use cantus_core::EffectSpec;
use cantus_dsp::math::{exp, sin, tanh};
use cantus_engine::effects::EffectMemory;
use cantus_engine::fx::comparatio_store::RefTrack;
use cantus_engine::fx::{FxUnit, SongPosition, Telemetry};
use std::f32::consts::TAU;
use std::sync::Arc;

const SR: f32 = 48_000.0;
const SECONDS: f32 = 12.0;

fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// The music, before mastering: (left, right).
fn music(bright: f32, width: f32, seed0: u32) -> (Vec<f32>, Vec<f32>) {
    let n = (SECONDS * SR) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    let beat = (60.0 / 112.0 * SR) as usize;
    let chords = [
        [146.83f32, 220.0, 293.66, 349.23],
        [130.81, 196.0, 261.63, 329.63],
    ];
    let mut seed = seed0;
    let mut hp = 0.0f32;
    for i in 0..n {
        let x = i as f32 / SR;
        let b = i / beat;
        let tb = (i % beat) as f32 / SR;
        let ch = &chords[(b / 8) % 2];
        let mut v = 0.0;
        for (j, f) in ch.iter().enumerate() {
            let env = exp(-tb * 1.8);
            for h in 1..=6 {
                v += 0.025 * env / h as f32 * sin(TAU * f * h as f32 * x + j as f32);
            }
        }
        let bass = 0.16 * sin(TAU * ch[0] * 0.5 * x) * exp(-tb * 2.5);
        let kick = if b.is_multiple_of(2) {
            0.35 * sin(TAU * (55.0 + 90.0 * exp(-tb * 30.0)) * tb) * exp(-tb * 9.0)
        } else {
            0.0
        };
        let wn = noise(&mut seed);
        hp = 0.7 * hp + 0.3 * wn;
        let hat = (wn - hp) * 0.08 * exp(-((i % (beat / 2)) as f32 / SR) * 40.0) * (0.4 + bright);
        let snare = if !b.is_multiple_of(2) {
            wn * 0.12 * exp(-tb * 18.0)
        } else {
            0.0
        };
        // A breakdown from 4.5 to 7.5 s: the drums out, the chord softer.
        let quiet = (4.5..7.5).contains(&x);
        let (kick, snare, hat) = if quiet {
            (0.0, 0.0, 0.0)
        } else {
            (kick, snare, hat)
        };
        let v = if quiet { v * 0.35 } else { v };
        let mid = v + bass + kick + snare;
        let side = v * 0.35 * width + hat * width;
        l[i] = mid + side + hat;
        r[i] = mid - side + hat * 0.8;
    }
    (l, r)
}

/// The finished master: brighter, wider, pushed into a soft clipper and held under −1 dBTP.
fn master() -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = music(1.4, 1.6, 0x0BAD_5EED);
    for ch in [&mut l, &mut r] {
        // Brighter: the highs lifted over a one-pole split at about 3 kHz.
        let mut lp = 0.0f32;
        for v in ch.iter_mut() {
            lp += 0.33 * (*v - lp);
            *v = 0.62 * tanh((*v + 0.9 * (*v - lp)) * 3.2);
        }
    }
    (l, r)
}

/// The mix in progress: darker (a gentle one-pole low-pass near 5 kHz).
fn mix_in_progress() -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = music(0.5, 0.7, 0x5EED_0001);
    for ch in [&mut l, &mut r] {
        let mut lp = 0.0f32;
        for v in ch.iter_mut() {
            lp += 0.5 * (*v - lp);
            *v = 0.6 * *v + 0.4 * lp;
        }
    }
    (l, r)
}

/// The unit at `spec` over the demo: its last telemetry, the console's picture, and the
/// mix's middle as the slot's analyser tap carries it (the editor measures its curves from
/// it, as it does live). The reference's summary is in the store (`RefTrack::analyse`).
pub fn picture(spec: &EffectSpec) -> (Telemetry, Picture, Vec<f32>) {
    let (ml, mr) = mix_in_progress();
    let (rl, rr) = master();
    let mid: Vec<f32> = ml.iter().zip(&mr).map(|(a, b)| 0.5 * (a + b)).collect();
    let mut unit = cantus_engine::fx::comparatio::Unit::new(SR);
    let mut spec = spec.clone();
    spec.ir = Some(Box::new(cantus_core::IrFile::new(PATH)));
    // The key the unit asks with: the path's.
    let key = cantus_engine::fx::aula_store::file_key(PATH);
    let track = Arc::new(RefTrack::new(key, SR as u32, rl, rr));
    track.analyse();
    unit.configure(&spec);
    let _ = unit.supply(EffectMemory::Comparatio(Some(track)));
    unit.reset();
    let mut pic = Picture::default();
    let mut last = [0.0; cantus_engine::fx::TELEMETRY_LEN];
    let mut at = 0usize;
    for (bl, br) in ml.chunks(480).zip(mr.chunks(480)) {
        let mut l = bl.to_vec();
        let mut r = br.to_vec();
        unit.set_position(&SongPosition {
            playing: true,
            tempo_bpm: 112.0,
            tempo_now: 112.0,
            samples: at as i64,
            project_samples: at as i64,
            time_sig_num: 4,
            time_sig_den: 4,
            ..Default::default()
        });
        unit.process(&mut l, &mut r, None);
        last = unit.telemetry();
        pic.update(&last);
        at += bl.len();
    }
    (last, pic, mid)
}

/// The demo's reference, by the path the project names it with.
pub const PATH: &str = "references/Finished master.flac";
