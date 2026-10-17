//! Warping's bench (round 29, ADR 0061): the stretcher measured — transient sharpness and
//! pre-echo, marker timing, pitch accuracy of shifted tones, phasiness, CPU per second of
//! audio per mode — and the tempo detector's accuracy over a corpus; and the A/B listening
//! set (`cantus-bench listen <dir>`): each mode × ratios 0.5, 0.8, 1.25, 2 × material.

use crate::report::{Better, Metric};
use crate::stats::Summary;
use crate::Plan;
use cantus_dsp::beat::testkit::{click_track, drum_loop};
use cantus_dsp::math;
use cantus_dsp::pitch::testkit::{self as pk, Noise, Vowel};
use cantus_dsp::stretch::{self, measure as m, Map, Mode, Request, Tail};
use std::path::Path;
use std::time::Instant;

pub const SR: f32 = 48_000.0;
/// The ratios of the listening set and of the measurements (output length / input).
pub const RATIOS: [f64; 4] = [0.5, 0.8, 1.25, 2.0];

/// One piece of material: its name, its channels, its transients (source frames).
pub struct Material {
    pub name: &'static str,
    pub channels: Vec<Vec<f32>>,
    pub onsets: Vec<f64>,
}

fn stereo(x: Vec<f32>) -> Vec<Vec<f32>> {
    vec![x.clone(), x]
}

/// Drums: a swung loop built from the testkit's kick, snare and hats, 100 BPM, 4 bars.
pub fn drums() -> Material {
    let d = drum_loop(SR, 100.0, 4, 0.3, 0.05, 11);
    Material {
        name: "drums",
        channels: stereo(d.audio),
        onsets: d.onsets,
    }
}

/// A full mix: one of the demo songs rendered offline (its first 8 seconds, stereo).
pub fn mix() -> Material {
    let songs = crate::paced::songs();
    let (_, p) = songs.first().expect("a demo song");
    let mut s = cantus_core::Snapshot::compile(p, 1);
    let mut cache = cantus_engine::SampleCache::new();
    let _ = cache.attach(&mut s);
    let _ = cache.attach_audio(&mut s, SR as u32);
    let opts = cantus_engine::RenderOptions {
        sample_rate: SR as u32,
        tail_seconds: 0.0,
        block_frames: 256,
        end: Some(cantus_core::Ticks::from_seconds(8.0, p.tempo_bpm)),
        ..Default::default()
    };
    let r = cantus_engine::render(std::sync::Arc::new(s), &opts).expect("the song renders");
    let (l, rr): (Vec<f32>, Vec<f32>) = r.samples.chunks_exact(2).map(|f| (f[0], f[1])).unzip();
    let onsets = cantus_dsp::onset::detect(&l, SR).frames;
    Material {
        name: "mix",
        channels: vec![l, rr],
        onsets,
    }
}

/// A sung phrase: the testkit's voice, a melody with vibrato.
pub fn voice() -> Material {
    let notes = [(0.0, 57.0), (0.6, 59.0), (1.2, 60.0), (2.0, 64.0), (2.8, 62.0)];
    let f0 = move |t: f64| {
        let n = notes.iter().rev().find(|(s, _)| t >= *s).map_or(57.0, |n| n.1);
        let vib = 30.0 * math::sin(2.0 * std::f64::consts::PI * 5.5 * t) as f32;
        cantus_dsp::pitch::midi_to_hz(n + vib / 100.0)
    };
    let x = pk::voice(SR, 3.6, Vowel::A, 0.5, f0);
    let onsets = cantus_dsp::onset::detect(&x, SR).frames;
    Material {
        name: "voice",
        channels: stereo(x),
        onsets,
    }
}

/// A solo instrument: a flute-like line (breathy, harmonics falling off, legato).
pub fn solo() -> Material {
    let notes = [(0.0, 72.0), (0.5, 74.0), (1.0, 76.0), (1.5, 79.0), (2.25, 77.0), (3.0, 76.0)];
    let mut noise = Noise::new(5);
    let mut phase = 0.0f64;
    let n = (3.6 * SR) as usize;
    let x: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f64 / SR as f64;
            let m0 = notes.iter().rev().find(|(s, _)| t >= *s).map_or(72.0, |n| n.1);
            let f = cantus_dsp::pitch::midi_to_hz(m0) as f64
                * (1.0 + 0.004 * math::sin(2.0 * std::f64::consts::PI * 5.0 * t));
            phase += 2.0 * std::f64::consts::PI * f / SR as f64;
            let env = (t / 0.04).min(1.0) * ((3.6 - t) / 0.1).min(1.0);
            let tone = math::sin(phase) + 0.3 * math::sin(2.0 * phase) + 0.1 * math::sin(3.0 * phase);
            (0.35 * env * tone) as f32 + 0.01 * noise.next()
        })
        .collect();
    let onsets = cantus_dsp::onset::detect(&x, SR).frames;
    Material {
        name: "solo",
        channels: stereo(x),
        onsets,
    }
}

/// A chordal pad: detuned saws of a I–vi–IV–V progression, a slow attack, spread wide.
pub fn pad() -> Material {
    let chords = [[48, 52, 55, 60], [45, 52, 57, 60], [41, 48, 53, 57], [43, 50, 55, 59]];
    let n = (4.0 * SR) as usize;
    let mut l = vec![0.0f32; n];
    let mut r = vec![0.0f32; n];
    for (c, chord) in chords.iter().enumerate() {
        let from = c * n / 4;
        for (v, &note) in chord.iter().enumerate() {
            for (d, detune) in [-0.08f64, 0.08].iter().enumerate() {
                let f = cantus_dsp::pitch::midi_to_hz(note as f32) as f64 * (1.0 + detune / 100.0 * 12.0);
                let mut ph = (v * 7 + d * 3) as f64 * 0.37;
                for i in from..from + n / 4 {
                    let t = (i - from) as f64 / SR as f64;
                    ph += f / SR as f64;
                    let saw = 2.0 * (ph - ph.floor()) - 1.0;
                    let env = (t / 0.3).min(1.0) * ((1.0 - t) / 0.05).clamp(0.0, 1.0);
                    let s = (0.05 * env * saw) as f32;
                    if d == 0 {
                        l[i] += s;
                    } else {
                        r[i] += s;
                    }
                }
            }
        }
    }
    Material {
        name: "pad",
        channels: vec![l, r],
        onsets: Vec::new(),
    }
}

/// Every material of the set.
pub fn set() -> Vec<Material> {
    vec![drums(), mix(), voice(), solo(), pad()]
}

/// Render `mat` stretched to `ratio` times its length in `mode`, transposed `semis`.
pub fn render(mat: &Material, mode: Mode, ratio: f64, semis: f64) -> Vec<Vec<f32>> {
    let len_in = mat.channels[0].len();
    let len = (len_in as f64 * ratio).round() as usize;
    let map = Map::new(vec![(0.0, 0.0), (len as f64, len_in as f64)]).expect("a map");
    let refs: Vec<&[f32]> = mat.channels.iter().map(Vec::as_slice).collect();
    let pitch = (mode == Mode::Melodic).then(|| {
        cantus_dsp::pitch::analyse(&refs[0][..], SR)
    });
    stretch::render(
        &refs,
        SR,
        &Request {
            mode,
            map: &map,
            len,
            semitones: semis,
            formants: true,
            onsets: &mat.onsets,
            pitch: pitch.as_ref(),
            tail: Tail::Fade,
        },
    )
}
