//! What the helper's tests share: WAV files written and read, the fixture model's job.
#![allow(dead_code)]

use cantus_ml::spec::{Family, ModelSpec, RuntimeSpec, SeparateArgs};
use std::path::{Path, PathBuf};

pub const RATE: u32 = 44_100;
pub const SEGMENT: usize = 343_980;
pub const SOURCES: [&str; 4] = ["drums", "bass", "other", "vocals"];

/// A stereo signal with nothing near the Nyquist frequency, and a small offset so the
/// normalisation's mean is not zero.
pub fn music(frames: usize) -> (Vec<f32>, Vec<f32>) {
    (0..frames)
        .map(|i| {
            let t = i as f32;
            let l = 0.4 * (t * 0.013).sin() + 0.2 * (t * 0.071 + 0.3).sin() + 0.02;
            let r = 0.3 * (t * 0.009 + 1.0).sin() + 0.25 * (t * 0.11).sin() - 0.01;
            (l, r)
        })
        .unzip()
}

pub fn write_wav(path: &Path, channels: u16, l: &[f32], r: &[f32]) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: RATE,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..l.len() {
        w.write_sample(l[i]).unwrap();
        if channels == 2 {
            w.write_sample(r[i]).unwrap();
        }
    }
    w.finalize().unwrap();
}

pub fn read_wav(path: &Path) -> (Vec<f32>, Vec<f32>) {
    let mut r = hound::WavReader::open(path).unwrap();
    assert_eq!(r.spec().channels, 2);
    let s: Vec<f32> = r.samples::<f32>().map(|v| v.unwrap()).collect();
    s.chunks(2).map(|c| (c[0], c[1])).unzip()
}

/// The mean of the mono mix, as the normalisation takes it.
pub fn mono_mean(l: &[f32], r: &[f32]) -> f64 {
    l.iter()
        .zip(r)
        .map(|(a, b)| ((a + b) / 2.0) as f64)
        .sum::<f64>()
        / l.len() as f64
}

pub fn fixture_args(
    input: PathBuf,
    out_dir: PathBuf,
    spec: [f32; 4],
    wave: [f32; 4],
) -> SeparateArgs {
    SeparateArgs {
        runtime: RuntimeSpec {
            library: PathBuf::from("unused"),
            threads: 0,
            device: Default::default(),
            sha256: None,
        },
        model: ModelSpec {
            graph: PathBuf::from("unused"),
            family: Family::Fixture {
                spec: spec.to_vec(),
                wave: wave.to_vec(),
                delay_ms: 0,
                crash_at: None,
                panic_at: None,
                hang_at: None,
            },
            sources: SOURCES.iter().map(|s| s.to_string()).collect(),
            sample_rate: RATE,
            segment: SEGMENT,
            check: Vec::new(),
        },
        input,
        out_dir,
        overlap: 0.25,
        shifts: 0,
    }
}

/// Set the fixture's misbehaviour.
pub fn with_fixture(
    mut a: SeparateArgs,
    delay_ms: u64,
    crash_at: Option<usize>,
    hang_at: Option<usize>,
) -> SeparateArgs {
    if let Family::Fixture {
        delay_ms: d,
        crash_at: c,
        hang_at: h,
        ..
    } = &mut a.model.family
    {
        *d = delay_ms;
        *c = crash_at;
        *h = hang_at;
    }
    a
}

/// The largest difference between a stem and `gain · (x − mean) + mean` over `range`.
pub fn worst(
    stem: &(Vec<f32>, Vec<f32>),
    x: &(Vec<f32>, Vec<f32>),
    gain: f32,
    mean: f64,
    range: std::ops::Range<usize>,
) -> f32 {
    range
        .map(|i| {
            let el = gain as f64 * (x.0[i] as f64 - mean) + mean;
            let er = gain as f64 * (x.1[i] as f64 - mean) + mean;
            ((stem.0[i] as f64 - el)
                .abs()
                .max((stem.1[i] as f64 - er).abs())) as f32
        })
        .fold(0f32, f32::max)
}

/// Make the fixture panic at a segment.
pub fn with_panic(mut a: SeparateArgs, at: usize) -> SeparateArgs {
    if let Family::Fixture { panic_at, .. } = &mut a.model.family {
        *panic_at = Some(at);
    }
    a
}
