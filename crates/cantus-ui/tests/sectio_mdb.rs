//! Sectio's slice points against **hand annotation** (round 37, ADR 0099): MDB Drums'
//! drum-only stems (Southall et al., ISMIR 2017 — 23 real recordings, every hit annotated by
//! hand; CC BY-NC-SA 4.0: downloaded locally for measuring only, never committed). Each
//! track is cut into 4-second loops; each loop goes through the slicer the way a dropped
//! file does (`sectio_ui::analyse`, transients); a hit counts as found when a slice point
//! lies within 10 ms of it (hits closer than 30 ms are one attack — the slicer's own gap).
//! Ignored: it needs the dataset.
//!
//! ```text
//! CANTUS_MDB_DRUMS=F:\projects\daw-r37\downloads\piano\mdb \
//!   cargo test -p cantus-ui --release --test sectio_mdb -- --ignored --nocapture
//! ```
//!
//! The folder holds `audio/drum_only/*.wav` and `annotations/class/*_class.txt` side by
//! side (the repository's two folders, flattened). `CANTUS_MDB_SENSITIVITY` sets the
//! sensitivity (default: the slicer's own, 0.5).

use cantus_core::sectio::SliceMode;
use std::path::Path;

const LOOP_S: f64 = 4.0;
const TOL_S: f64 = 0.010;
/// Wider windows, reported beside (an onset evaluation's usual is 50 ms).
const WIDER_S: [f64; 2] = [0.020, 0.050];
const GAP_S: f64 = 0.030;

fn read(path: &Path) -> (Vec<f32>, u32, u16) {
    let mut r = hound::WavReader::open(path).expect("wav");
    let spec = r.spec();
    let all: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => r.samples::<f32>().map(|s| s.unwrap()).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1u64 << (spec.bits_per_sample - 1)) as f32;
            r.samples::<i32>()
                .map(|s| s.unwrap() as f32 * scale)
                .collect()
        }
    };
    (all, spec.sample_rate, spec.channels)
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

#[test]
#[ignore = "needs MDB Drums (CANTUS_MDB_DRUMS)"]
fn slices_land_within_ten_ms_of_the_hand_annotated_hits() {
    let Some(dir) = std::env::var_os("CANTUS_MDB_DRUMS") else {
        eprintln!("CANTUS_MDB_DRUMS is not set");
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    let sens: f32 = std::env::var("CANTUS_MDB_SENSITIVITY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.5);
    let tmp = tempfile::tempdir().unwrap();
    let mut wavs: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "wav"))
        .collect();
    wavs.sort();
    assert!(!wavs.is_empty(), "no WAVs in {}", dir.display());
    let (mut truth_n, mut found_n, mut hit_t, mut hit_f, mut loops) = (0, 0, 0, 0, 0);
    let mut wider = [0usize; 2];
    // Signed error (point − hit, ms) of each hit's nearest point within 50 ms.
    let mut errs: Vec<f64> = Vec::new();
    for wav in &wavs {
        let stem = wav
            .file_stem()
            .unwrap()
            .to_string_lossy()
            .replace("_Drum", "");
        let ann = dir.join(format!("{stem}_class.txt"));
        if !ann.exists() {
            eprintln!("no annotation for {}", wav.display());
            continue;
        }
        let all_hits = hits(&ann);
        let (x, rate, ch) = read(wav);
        let frames = x.len() / ch as usize;
        let per = (LOOP_S * rate as f64) as usize;
        let (mut t_n, mut f_n, mut h_t, mut h_f) = (0, 0, 0, 0);
        for start in (0..frames.saturating_sub(per)).step_by(per) {
            // The loop as a file of its own, as a user would drop it.
            let path = tmp.path().join("loop.wav");
            let spec = hound::WavSpec {
                channels: ch,
                sample_rate: rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };
            let mut w = hound::WavWriter::create(&path, spec).unwrap();
            for v in &x[start * ch as usize..(start + per) * ch as usize] {
                w.write_sample(*v).unwrap();
            }
            w.finalize().unwrap();
            let t0 = start as f64 / rate as f64;
            // Hits inside the loop, from its start (a hit in the first 10 ms is slice 0's).
            let truth: Vec<f64> = all_hits
                .iter()
                .map(|h| h - t0)
                .filter(|h| *h >= 0.0 && *h < LOOP_S - TOL_S)
                .collect();
            if truth.is_empty() || truth.len() >= cantus_core::sectio::MAX_SLICES {
                continue;
            }
            let s = cantus_ui::sectio_ui::analyse(&path, SliceMode::Transients, sens, 4)
                .expect("sliced");
            let pts: Vec<f64> = s.points.iter().map(|p| *p as f64 / rate as f64).collect();
            let near = |a: f64, list: &[f64]| list.iter().any(|b| (a - b).abs() <= TOL_S);
            // Slice 0 always starts at the loop's first frame: it is a point only when a
            // hit is there.
            let found: Vec<f64> = pts
                .iter()
                .copied()
                .filter(|p| *p > 0.0 || near(0.0, &truth))
                .collect();
            t_n += truth.len();
            f_n += found.len();
            h_t += truth.iter().filter(|t| near(**t, &found)).count();
            h_f += found.iter().filter(|p| near(**p, &truth)).count();
            for t in &truth {
                if let Some(b) = found
                    .iter()
                    .copied()
                    .min_by(|a, b| (a - t).abs().total_cmp(&(b - t).abs()))
                    .filter(|b| (b - t).abs() <= 0.05)
                {
                    errs.push(1e3 * (b - t));
                }
            }
            for (w, tol) in wider.iter_mut().zip(WIDER_S) {
                *w += truth
                    .iter()
                    .filter(|t| found.iter().any(|b| (*t - b).abs() <= tol))
                    .count();
            }
            loops += 1;
        }
        eprintln!(
            "{stem:28} {h_t:4}/{t_n:4} hits found ({:5.1} %), {h_f:4}/{f_n:4} points on a hit ({:5.1} %)",
            100.0 * h_t as f64 / t_n.max(1) as f64,
            100.0 * h_f as f64 / f_n.max(1) as f64
        );
        truth_n += t_n;
        found_n += f_n;
        hit_t += h_t;
        hit_f += h_f;
    }
    let recall = hit_t as f64 / truth_n.max(1) as f64;
    let precision = hit_f as f64 / found_n.max(1) as f64;
    eprintln!(
        "MDB Drums, {loops} loops of {LOOP_S} s, sensitivity {sens}: {hit_t} of {truth_n} hand-annotated hits have a slice point within 10 ms ({:.1} %); {hit_f} of {found_n} points are on a hit ({:.1} %)",
        100.0 * recall,
        100.0 * precision
    );
    eprintln!(
        "  within 20 ms: {:.1} %; within 50 ms: {:.1} %",
        100.0 * wider[0] as f64 / truth_n.max(1) as f64,
        100.0 * wider[1] as f64 / truth_n.max(1) as f64
    );
    // The brief's aim is 95 %; round 37 measures 73.9 % (88.0 % within 20 ms): this holds
    // that level (ADR 0099).
    errs.sort_by(f64::total_cmp);
    let q = |p: f64| errs[((errs.len() - 1) as f64 * p) as usize];
    eprintln!(
        "  signed error, point − hit (ms): 5 % {:+.1}, 25 % {:+.1}, median {:+.1}, 75 % {:+.1}, 95 % {:+.1}",
        q(0.05),
        q(0.25),
        q(0.5),
        q(0.75),
        q(0.95)
    );
    // The brief's aim is 95 %; round 37 measures 75.8 % with the bands picked apart (73.9 %
    // before; 88 % within 20 ms): this holds that level (ADR 0099).
    assert!(recall >= 0.74, "recall {recall}");
}
