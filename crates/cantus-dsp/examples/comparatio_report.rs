//! Comparatio's offline report (DSP only): the CC0 DI guitar progression processed two
//! ways — a "mix" and a "reference" — compared by Comparatio's analyses, the switch's
//! level checked on the unit's own output, the match-EQ suggestion fitted by Aequalis;
//! and the CPU at 48 kHz, best of three.
//!
//! ```text
//! cargo run -p cantus-dsp --release --example comparatio_report -- <guitar-wav-dir> <out.md> [load note]
//! ```
//!
//! The guitar: *Strummed Guitar Chords* by Carbonthief, CC0 (`refs/audio/README.md`): the
//! owner's progression Am–F–C–G at 90 BPM, twice, two strums a bar. The WAVs are mono
//! 44.1 kHz; the report runs at their rate (no resampling).

#[path = "comparatio_report/cpu.rs"]
mod cpu;
#[path = "comparatio_report/write.rs"]
mod write;

use cantus_dsp::aequalis::{Aequalis, BandSettings, EqShape};
use cantus_dsp::comparatio::reader::{RefAudio, Sync};
use cantus_dsp::comparatio::tonal::Resolution;
use cantus_dsp::comparatio::{analyse_whole, Comparatio, ComparatioSettings, Listen};
use cantus_dsp::fragor::{BoardSettings, Fragor, Model};
use cantus_dsp::math;
use cantus_dsp::modfx_clock::Transport;
use std::path::Path;

fn read_mono(path: &Path) -> (Vec<f32>, f32) {
    let mut r = hound::WavReader::open(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    let spec = r.spec();
    let x: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.unwrap() as f32 / full)
                .collect()
        }
    };
    let ch = spec.channels as usize;
    let mono = x
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect();
    (mono, spec.sample_rate as f32)
}

/// Am – F – C – G at 90 BPM, twice, a strum on beats 1 and 3, each faded out over 20 ms
/// before the next; peak −6 dBFS.
fn progression(dir: &Path) -> (Vec<f32>, f32) {
    let names = [
        "Sample07 (A Minor).wav",
        "Sample15 (F Major).wav",
        "Sample22 (C Major).wav",
        "Sample17 (G Major).wav",
    ];
    let mut chords = Vec::new();
    let mut sr = 0.0;
    for n in names {
        let (x, rate) = read_mono(&dir.join(n));
        chords.push(x);
        sr = rate;
    }
    let beat = (60.0 / 90.0 * sr as f64).round() as usize;
    let strum = 2 * beat;
    let fade = (0.020 * sr) as usize;
    let mut out = vec![0.0f32; 16 * strum + sr as usize];
    for k in 0..16 {
        let c = &chords[(k / 2) % 4];
        let start = k * strum;
        let len = strum.min(c.len());
        for i in 0..len {
            let g = if i + fade >= strum {
                (strum - i) as f32 / fade as f32
            } else {
                1.0
            };
            out[start + i] += c[i] * g;
        }
    }
    let peak = out.iter().fold(0f32, |m, v| m.max(v.abs()));
    let g = math::powf(10f32, -6.0 / 20.0) / peak;
    out.iter_mut().for_each(|v| *v *= g);
    (out, sr)
}

fn band(shape: EqShape, freq: f32, gain_db: f32, q: f32) -> BandSettings {
    BandSettings {
        active: true,
        shape,
        freq,
        gain_db,
        q,
        ..BandSettings::default()
    }
}

/// A short delay on the right (a Haas widening), at `gain`.
fn haas(x: &[f32], ms: f32, gain: f32, sr: f32) -> Vec<f32> {
    let d = (ms / 1_000.0 * sr) as usize;
    (0..x.len())
        .map(|i| if i >= d { x[i - d] * gain } else { 0.0 })
        .collect()
}

/// The "mix": a light overdrive, a low cut and a little presence, nearly mono.
fn version_a(di: &[f32], sr: f32) -> (Vec<f32>, Vec<f32>) {
    let mut f = Fragor::new(sr);
    let mut s = BoardSettings::default();
    s.chain[0] = Some(Model::Overdrive);
    s.knobs[Model::Overdrive.index()] = [0.35, 0.5, 0.7];
    s.mono = true;
    f.set(&s);
    let (mut l, mut r) = (di.to_vec(), di.to_vec());
    for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
        f.process(a, b);
    }
    let mut eq = Aequalis::new(sr);
    eq.set(
        &[
            band(EqShape::LowCut, 90.0, 0.0, 0.7),
            band(EqShape::Bell, 2_500.0, 2.0, 1.0),
        ],
        0.0,
    );
    for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
        eq.process(a, b);
    }
    let r = haas(&l, 3.0, 0.9, sr);
    // About −15 LUFS, as a mix before mastering.
    let l: Vec<f32> = l
        .iter()
        .zip(&r)
        .map(|(a, b)| 0.5 * (0.8 * a + 0.2 * b))
        .collect();
    let r = r.iter().map(|v| 0.5 * v).collect();
    (l, r)
}

/// The "reference": a heavier muff, more low end, darker top, a wide Haas image, and
/// pushed 9 dB into a soft clip (louder, less crest).
fn version_b(di: &[f32], sr: f32) -> (Vec<f32>, Vec<f32>) {
    let mut f = Fragor::new(sr);
    let mut s = BoardSettings::default();
    s.chain[0] = Some(Model::Muff);
    s.mono = true;
    f.set(&s);
    let (mut l, mut r) = (di.to_vec(), di.to_vec());
    for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
        f.process(a, b);
    }
    let mut eq = Aequalis::new(sr);
    eq.set(
        &[
            band(EqShape::LowShelf, 150.0, 3.0, 0.7),
            band(EqShape::HighCut, 7_000.0, 0.0, 0.7),
        ],
        0.0,
    );
    for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
        eq.process(a, b);
    }
    let r = haas(&l, 14.0, 1.0, sr);
    let peak = l.iter().fold(0f32, |m, v| m.max(v.abs()));
    let g = math::powf(10f32, 9.0 / 20.0) / peak.max(1e-6) * 0.5;
    // Into the clip, then 6 dB down: about −8 LUFS, as a loud master.
    let clip = |v: f32| 0.5 * math::tanh(v * g);
    (
        l.iter().map(|v| clip(*v)).collect(),
        r.iter().map(|v| clip(*v)).collect(),
    )
}

/// The unit itself: the mix, then the reference from 10 s on (integrated match, locked);
/// the output's loudness on each side of the switch.
fn switched(a: &(Vec<f32>, Vec<f32>), b: &(Vec<f32>, Vec<f32>), sr: f32) -> (f64, f64, f32) {
    let mut u = Comparatio::new(sr);
    let base = ComparatioSettings {
        sync: Sync::Locked { offset: 0 },
        ..ComparatioSettings::default()
    };
    u.set(&base);
    let (mut l, mut r) = (a.0.clone(), a.1.clone());
    let sw = (10.0 * sr) as usize / 512 * 512;
    let mut i = 0;
    while i < l.len() {
        if i == sw {
            u.set(&ComparatioSettings {
                listen: Listen::Reference,
                ..base
            });
        }
        let m = 512.min(l.len() - i);
        u.set_transport(&Transport {
            playing: true,
            tempo_bpm: 90.0,
            samples: i as i64,
        });
        let audio = RefAudio {
            left: &b.0,
            right: &b.1,
            id: 1,
        };
        u.process(&mut l[i..i + m], &mut r[i..i + m], Some(audio));
        i += m;
    }
    // Over the second half (the reference heard, the match settled): the output's loudness
    // against the mix's over the same stretch.
    let (from, to) = (sw + sr as usize, (sw + (10.6 * sr) as usize).min(l.len()));
    let lufs = |x: &[f32], y: &[f32]| {
        let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(sr);
        m.process(x, y);
        m.integrated().unwrap_or(f64::NEG_INFINITY)
    };
    (
        lufs(&l[from..to], &r[from..to]),
        lufs(&a.0[from..to], &a.1[from..to]),
        u.meter().gain_db,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = Path::new(args.first().map(String::as_str).unwrap_or("tmp/guitar-wav"));
    let out = Path::new(args.get(1).map(String::as_str).unwrap_or("analysis.md"));
    let (di, sr) = progression(dir);
    let a = version_a(&di, sr);
    let b = version_b(&di, sr);
    let wa = analyse_whole(&a.0, &a.1, sr, Resolution::Third);
    let wb = analyse_whole(&b.0, &b.1, sr, Resolution::Third);
    let sw = switched(&a, &b, sr);
    let cpu = cpu::measure();
    let mut text = write::report(sr, di.len(), &wa, &wb, sw, &cpu);
    if let Some(note) = args.get(2) {
        text.push_str(&format!(
            "

The machine while measuring: {note}
"
        ));
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).expect("the report's folder");
    }
    std::fs::write(out, &text).expect("the report");
    println!("{text}");
}
