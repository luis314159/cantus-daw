//! Sectio's transients against annotated hits, at the DSP layer (round 37, ADR 0099): the
//! quick loop for voicing the detector. Every folder in `CANTUS_SECTIO_SETS` (`;`-separated)
//! holds `<name>_Drum.wav` beside `<name>_class.txt` (MDB Drums' layout: one hit a line, its
//! time in seconds first); each file is cut into loops of 4 s (a shorter file is one loop),
//! each loop sliced by `sectio::transients` at the slicer's default sensitivity; a hit counts
//! when a point lies within 10 ms. Ignored: it needs the sets (MDB Drums is CC BY-NC-SA,
//! measured locally, never committed; `tests/sectio_accuracy.rs` in cantus-ui writes the
//! rendered set with `CANTUS_SECTIO_DUMP`).
//!
//! ```text
//! CANTUS_SECTIO_SETS="F:\…\mdb;F:\…\rendered" cargo test -p cantus-dsp --release \
//!   --test sectio_onsets -- --ignored --nocapture
//! ```

use cantus_dsp::onset::Options;
use cantus_dsp::sectio::transients_with;
use std::path::Path;

const LOOP_S: f64 = 4.0;
const TOL_S: f64 = 0.010;
const GAP_S: f64 = 0.030;

fn read(path: &Path) -> (Vec<f32>, u32) {
    let mut r = hound::WavReader::open(path).expect("wav");
    let spec = r.spec();
    let ch = spec.channels as usize;
    let all: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.unwrap() as f32 * scale)
                .collect()
        }
    };
    let mono = all
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect();
    (mono, spec.sample_rate)
}

fn hits(path: &Path) -> Vec<f64> {
    let mut t: Vec<f64> = std::fs::read_to_string(path)
        .expect("annotation")
        .lines()
        .filter_map(|l| l.split_whitespace().next()?.parse().ok())
        .collect();
    t.sort_by(f64::total_cmp);
    let mut out: Vec<f64> = Vec::new();
    for x in t {
        if out.last().is_none_or(|l| x - l > GAP_S) {
            out.push(x);
        }
    }
    out
}

#[derive(Default)]
struct Tally {
    truth: usize,
    found: usize,
    hit_t: usize,
    hit_f: usize,
    errs: Vec<f64>,
}

fn measure(dir: &Path, sens: f32, o: &Options) -> Vec<(String, Tally)> {
    let mut wavs: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    wavs.sort();
    let mut out = Vec::new();
    for wav in &wavs {
        let stem = wav
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .replace("_Drum", "");
        let ann = dir.join(format!("{stem}_class.txt"));
        if !ann.exists() {
            continue;
        }
        let all = hits(&ann);
        let (x, rate) = read(wav);
        let per = ((LOOP_S * rate as f64) as usize).min(x.len());
        let mut t = Tally::default();
        let mut start = 0;
        while start + per <= x.len() {
            let t0 = start as f64 / rate as f64;
            let len_s = per as f64 / rate as f64;
            let truth: Vec<f64> = all
                .iter()
                .map(|h| h - t0)
                .filter(|h| *h >= 0.0 && *h < len_s - TOL_S)
                .collect();
            if !truth.is_empty() && truth.len() < 64 {
                let pts: Vec<f64> = transients_with(&x[start..start + per], rate as f32, sens, o)
                    .iter()
                    .map(|p| *p as f64 / rate as f64)
                    .collect();
                let near = |a: f64, l: &[f64]| l.iter().any(|b| (a - b).abs() <= TOL_S);
                let found: Vec<f64> = pts
                    .iter()
                    .copied()
                    .filter(|p| *p > 0.0 || near(0.0, &truth))
                    .collect();
                t.truth += truth.len();
                t.found += found.len();
                t.hit_t += truth.iter().filter(|h| near(**h, &found)).count();
                t.hit_f += found.iter().filter(|p| near(**p, &truth)).count();
                for h in &truth {
                    if let Some(b) = found
                        .iter()
                        .copied()
                        .min_by(|a, b| (a - h).abs().total_cmp(&(b - h).abs()))
                        .filter(|b| (b - h).abs() <= 0.05)
                    {
                        t.errs.push(1e3 * (b - h));
                    }
                }
            }
            start += per;
        }
        out.push((stem, t));
    }
    out
}

#[test]
#[ignore = "needs the annotated sets (CANTUS_SECTIO_SETS)"]
fn transients_against_annotated_hits() {
    let Ok(sets) = std::env::var("CANTUS_SECTIO_SETS") else {
        eprintln!("CANTUS_SECTIO_SETS is not set");
        return;
    };
    let sens: f32 = std::env::var("CANTUS_SECTIO_SENSITIVITY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.5);
    // `CANTUS_ONSET`: `bands` or `none`; default the slicer's.
    let o = match std::env::var("CANTUS_ONSET") {
        Ok(v) => Options {
            bands: v.contains("bands"),
        },
        Err(_) => Options::SLICER,
    };
    eprintln!("{o:?}, sensitivity {sens}");
    for dir in sets.split(';').filter(|d| !d.is_empty()) {
        let rows = measure(Path::new(dir), sens, &o);
        let mut all = Tally::default();
        for (name, t) in &rows {
            if std::env::var_os("CANTUS_SECTIO_EACH").is_some() {
                eprintln!(
                    "  {name:28} {:5.1} % found, {:5.1} % on a hit",
                    100.0 * t.hit_t as f64 / t.truth.max(1) as f64,
                    100.0 * t.hit_f as f64 / t.found.max(1) as f64
                );
            }
            all.truth += t.truth;
            all.found += t.found;
            all.hit_t += t.hit_t;
            all.hit_f += t.hit_f;
            all.errs.extend(&t.errs);
        }
        all.errs.sort_by(f64::total_cmp);
        let q = |p: f64| {
            all.errs
                .get(((all.errs.len().max(1) - 1) as f64 * p) as usize)
                .copied()
                .unwrap_or(f64::NAN)
        };
        eprintln!(
            "{dir}: {:.1} % of {} hits found within 10 ms, {:.1} % of {} points on a hit; error ms 5/25/50/75/95 %: {:+.1} {:+.1} {:+.1} {:+.1} {:+.1}",
            100.0 * all.hit_t as f64 / all.truth.max(1) as f64,
            all.truth,
            100.0 * all.hit_f as f64 / all.found.max(1) as f64,
            all.found,
            q(0.05),
            q(0.25),
            q(0.5),
            q(0.75),
            q(0.95)
        );
    }
}
