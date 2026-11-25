//! Listening by measurement: the written file's loudness, true peak and clipping; each
//! stem's level where its instrument plays and where it does not; the high-frequency
//! energy at every note's start and end against the music around it (a click stands out of
//! it by tens of dB); and the hall's tail at the end of the file.

use cantus_export::Audio;
use std::path::Path;

pub fn read_wav(path: &Path) -> Audio {
    let r = hound::WavReader::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let spec = r.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.into_samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let full = (1i64 << (spec.bits_per_sample - 1)) as f32;
            r.into_samples::<i32>()
                .map(|s| s.unwrap() as f32 / full)
                .collect()
        }
    };
    Audio {
        rate: spec.sample_rate,
        channels: spec.channels,
        samples,
    }
}

pub fn mono(a: &Audio) -> Vec<f32> {
    let ch = a.channels.max(1) as usize;
    a.samples
        .chunks_exact(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect()
}

pub fn db(x: f64) -> f64 {
    10.0 * x.max(1e-20).log10()
}

/// The RMS, dBFS, of `x` over `[from, to)` seconds.
pub fn rms_db(x: &[f32], rate: u32, from: f64, to: f64) -> f64 {
    let a = ((from * rate as f64) as usize).min(x.len());
    let b = ((to * rate as f64) as usize).min(x.len());
    if b <= a {
        return -200.0;
    }
    let e: f64 = x[a..b].iter().map(|v| f64::from(*v).powi(2)).sum();
    db(e / (b - a) as f64)
}

/// The integrated loudness (BS.1770, gated), LUFS, of `a` over `[from, to)` seconds.
pub fn lufs(a: &Audio, from: f64, to: f64) -> f64 {
    let ch = a.channels.max(1) as usize;
    let f = |t: f64| ((t * a.rate as f64) as usize).min(a.frames()) * ch;
    let part = Audio {
        rate: a.rate,
        channels: a.channels,
        samples: a.samples[f(from)..f(to)].to_vec(),
    };
    cantus_export::process::measure(&part).0.unwrap_or(-200.0)
}

/// What the file is: integrated loudness, true peak, sample peak, samples at full scale.
pub struct Level {
    pub lufs: Option<f64>,
    pub dbtp: f32,
    pub peak_db: f64,
    pub full_scale: usize,
    pub seconds: f64,
}

pub fn level(a: &Audio) -> Level {
    let (lufs, dbtp) = cantus_export::process::measure(a);
    let peak = a.samples.iter().fold(0f32, |m, v| m.max(v.abs()));
    Level {
        lufs,
        dbtp,
        peak_db: 20.0 * f64::from(peak.max(1e-12)).log10(),
        full_scale: a.samples.iter().filter(|v| v.abs() >= 0.999_9).count(),
        seconds: a.frames() as f64 / a.rate as f64,
    }
}

/// The second difference (a +12 dB per octave tilt: what a click is made of), squared
/// and summed over windows of `win` samples.
fn hf_energy(x: &[f32], win: usize) -> Vec<f64> {
    let mut out = vec![0.0; x.len() / win + 1];
    for n in 2..x.len() {
        let d = f64::from(x[n]) - 2.0 * f64::from(x[n - 1]) + f64::from(x[n - 2]);
        out[n / win] += d * d;
    }
    out
}

/// What a window is heard against: the louder of the medians 10–40 ms before it and
/// 10–100 ms after it. A note's end keeps the level it leaves and an attack grows into the
/// level it reaches (a bowed note's Helmholtz motion settles within ~50 ms), so neither
/// stands out of it; a click stands out of both.
fn reference(e: &[f64], c: usize) -> f64 {
    let median = |r: &[f64]| {
        let mut v = r.to_vec();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let after = (c + 100).min(e.len());
    median(&e[c - 40..c - 10]).max(median(&e[c + 10..after]))
}

/// One boundary measured: its time, the window's HF energy over the median of the 60 ms
/// around it (dB), and what it is.
#[derive(Clone, Debug)]
pub struct Edge {
    pub at: f64,
    pub over_db: f64,
    pub what: &'static str,
}

/// Each boundary's HF energy (±3 ms of it, 1 ms windows) against [`reference`]; a boundary
/// in near silence (under −70 dBFS around it) is left
/// out — nothing there for a click to be heard against or measured from.
pub fn edges(x: &[f32], rate: u32, at: &[(f64, &'static str)]) -> Vec<Edge> {
    let win = (rate / 1_000) as usize;
    let e = hf_energy(x, win);
    let mut out = Vec::new();
    for (t, what) in at {
        let c = (t * rate as f64) as usize / win;
        if c < 45 || c + 100 >= e.len() {
            continue;
        }
        let here = e[c - 3..=c + 3].iter().cloned().fold(0.0, f64::max);
        let med = reference(&e, c);
        if rms_db(x, rate, t - 0.04, t + 0.04) < -70.0 {
            continue;
        }
        out.push(Edge {
            at: *t,
            over_db: db(here / med.max(1e-30)),
            what,
        });
    }
    out
}

/// The loudest single 1 ms window of HF energy against its own neighbourhood anywhere in
/// the file (not only at boundaries): the worst spot, dB, and where.
pub fn worst_spot(x: &[f32], rate: u32) -> (f64, f64) {
    let win = (rate / 1_000) as usize;
    let e = hf_energy(x, win);
    let mut worst = (f64::NEG_INFINITY, 0.0);
    let mut i = 45;
    while i + 100 < e.len() {
        let med = reference(&e, i);
        let t = i as f64 * win as f64 / rate as f64;
        if med > 1e-14 {
            let r = db(e[i] / med);
            if r > worst.0 {
                worst = (r, t);
            }
        }
        i += 1;
    }
    worst
}

/// The tail: the level of each quarter second from `from` to the end, dBFS.
pub fn tail(x: &[f32], rate: u32, from: f64) -> Vec<f64> {
    let end = x.len() as f64 / rate as f64;
    let mut out = Vec::new();
    let mut t = from;
    while t + 0.25 <= end + 1e-9 {
        out.push(rms_db(x, rate, t, t + 0.25));
        t += 0.25;
    }
    out
}

/// Two files the same to the byte.
pub fn same_bytes(a: &Path, b: &Path) -> bool {
    match (std::fs::read(a), std::fs::read(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}
