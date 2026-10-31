//! Drums to notes measured on **MDB Drums** (Southall et al., ISMIR 2017: 23 tracks of
//! MedleyDB's drums, annotated class by class; CC BY-NC-SA 4.0 — downloaded locally for
//! measuring only, never committed). Ignored: it needs the dataset.
//!
//! ```text
//! CANTUS_MDB_DRUMS="F:\projects\daw-r34\models\datasets\mdb\MDBDrums-master\MDB Drums" \
//!   cargo test -p cantus-dsp --release --test onset_drums_mdb -- --ignored --nocapture
//! ```
//!
//! `CANTUS_MDB_MIX=1` measures the full mixes instead of the drum stems;
//! `CANTUS_MDB_SENSITIVITY` sets the sensitivity (0.5).

// A measurement harness: tables indexed by class and band, tuples of a track's parts.
#![allow(clippy::needless_range_loop, clippy::type_complexity)]

use cantus_dsp::onset::drums::{transcribe, Class};
use cantus_dsp::onset::eval::{onsets, Score};
use std::path::{Path, PathBuf};

fn read(path: &Path) -> (Vec<f32>, f32) {
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
        .map(|c| c.iter().sum::<f32>() / ch as f32)
        .collect();
    (mono, spec.sample_rate as f32)
}

fn class(code: &str) -> Option<Class> {
    match code {
        "KD" => Some(Class::Kick),
        "SD" => Some(Class::Snare),
        "TT" => Some(Class::Toms),
        "HH" => Some(Class::HiHat),
        "CY" => Some(Class::Cymbals),
        _ => None,
    }
}

#[test]
#[ignore]
fn mdb_drums_f_measure_per_class() {
    let Some(root) = std::env::var_os("CANTUS_MDB_DRUMS").map(PathBuf::from) else {
        eprintln!("CANTUS_MDB_DRUMS is not set");
        return;
    };
    let mix = std::env::var_os("CANTUS_MDB_MIX").is_some();
    let sensitivity: f32 = std::env::var("CANTUS_MDB_SENSITIVITY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0.5);
    let only = std::env::var("CANTUS_MDB_ONLY").ok();
    let mut names: Vec<String> = std::fs::read_dir(root.join("annotations").join("class"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .replace("_class.txt", "")
        })
        .collect();
    names.sort();
    let mut total: Vec<Vec<Score>> = vec![Vec::new(); 5];
    let mut confusion = [[0usize; 6]; 5];
    let held_out = std::env::var_os("CANTUS_MDB_HELD_OUT").is_some();
    for (index, name) in names.iter().enumerate() {
        if held_out && index < DEV {
            continue;
        }
        if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
            continue;
        }
        let ann = std::fs::read_to_string(
            root.join("annotations")
                .join("class")
                .join(format!("{name}_class.txt")),
        )
        .unwrap();
        let truth: Vec<(f64, Class)> = ann
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                let t: f64 = it.next()?.parse().ok()?;
                Some((t, class(it.next()?)?))
            })
            .collect();
        let wav = if mix {
            root.join("audio")
                .join("full_mix")
                .join(format!("{name}_MIX.wav"))
        } else {
            root.join("audio")
                .join("drum_only")
                .join(format!("{name}_Drum.wav"))
        };
        let (x, rate) = read(&wav);
        let t0 = std::time::Instant::now();
        let found = transcribe(&x, rate, sensitivity);
        let secs = t0.elapsed().as_secs_f64();
        let mut line = format!(
            "{name:28} {:5.1}s in {secs:4.1}s",
            x.len() as f64 / rate as f64
        );
        for (i, c) in Class::ALL.iter().enumerate() {
            let t: Vec<f64> = truth.iter().filter(|h| h.1 == *c).map(|h| h.0).collect();
            let f: Vec<f64> = found
                .iter()
                .filter(|h| h.class == *c)
                .map(|h| h.time)
                .collect();
            let s = onsets(&t, &f, 0.05);
            line += &format!(
                "  {:?} {:.2} ({}/{}/{})",
                c, s.f, s.matched, s.truth, s.found
            );
            total[i].push(s);
        }
        println!("{line}");
        // Where the false hits came from: the true class nearest within 50 ms, or none.
        for h in &found {
            let ok = truth
                .iter()
                .any(|t| t.1 == h.class && (t.0 - h.time).abs() <= 0.05);
            if !ok {
                let near = truth
                    .iter()
                    .filter(|t| (t.0 - h.time).abs() <= 0.05)
                    .map(|t| t.1 as usize)
                    .next()
                    .unwrap_or(5);
                confusion[h.class as usize][near] += 1;
            }
        }
    }
    println!("false hits by class found (rows) and the class truly there (cols: K S T H C none)");
    for (i, c) in Class::ALL.iter().enumerate() {
        println!("  {c:?}: {:?}", confusion[i]);
    }
    let mut line = String::from("ALL (micro)");
    let mut all = Vec::new();
    for (i, c) in Class::ALL.iter().enumerate() {
        let s = Score::sum(&total[i]);
        let mean = total[i]
            .iter()
            .filter(|s| s.truth > 0)
            .map(|s| s.f)
            .sum::<f64>()
            / total[i].iter().filter(|s| s.truth > 0).count().max(1) as f64;
        line += &format!(
            "  {:?} F {:.3} P {:.3} R {:.3} (track mean {:.3})",
            c, s.f, s.precision, s.recall, mean
        );
        all.extend(total[i].iter().copied());
    }
    println!("{line}");
    println!("ALL classes (micro) F {:.3}", Score::sum(&all).f);
}

/// The tracks tuning looked at (the first eight by name); the other fifteen are held out.
pub const DEV: usize = 8;

/// Each class's average spectrum at its isolated hits (no other class within 80 ms), on
/// the tuning tracks only: what the templates were drawn against.
#[test]
#[ignore]
fn mdb_class_spectra() {
    let Some(root) = std::env::var_os("CANTUS_MDB_DRUMS").map(PathBuf::from) else {
        return;
    };
    let mut names: Vec<String> = std::fs::read_dir(root.join("annotations").join("class"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .replace("_class.txt", "")
        })
        .collect();
    names.sort();
    let mut acc: Vec<Vec<f64>> = vec![Vec::new(); 5];
    let mut ring: Vec<Vec<f64>> = vec![Vec::new(); 5];
    let mut centres = Vec::new();
    for name in names.iter().take(DEV) {
        let ann = std::fs::read_to_string(
            root.join("annotations")
                .join("class")
                .join(format!("{name}_class.txt")),
        )
        .unwrap();
        let truth: Vec<(f64, Class)> = ann
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                let t: f64 = it.next()?.parse().ok()?;
                Some((t, class(it.next()?)?))
            })
            .collect();
        let (x, rate) = read(
            &root
                .join("audio")
                .join("drum_only")
                .join(format!("{name}_Drum.wav")),
        );
        let s = cantus_dsp::onset::drums::spectrogram(&x, rate);
        let b = s.bands();
        centres = s.centres.clone();
        for (i, c) in Class::ALL.iter().enumerate() {
            for &(t, _) in truth.iter().filter(|h| h.1 == *c) {
                if truth.iter().any(|o| o.1 != *c && (o.0 - t).abs() < 0.08) {
                    continue;
                }
                let f = (t * 100.0).round() as usize;
                if f < 3 || f + 30 >= s.frames {
                    continue;
                }
                if acc[i].is_empty() {
                    acc[i] = vec![0.0; b + 1];
                }
                for k in 0..b {
                    let now: f64 = (f..f + 4).map(|q| s.mag[q * b + k] as f64).sum::<f64>() / 4.0;
                    let before = s.mag[(f - 3) * b + k] as f64;
                    acc[i][k] += (now - before).max(0.0);
                }
                acc[i][b] += 1.0;
                // How long the hit's high-band energy rings over a quarter of its peak.
                let hi: Vec<f64> = (f..f + 30)
                    .map(|q| (b * 2 / 3..b).map(|k| s.mag[q * b + k] as f64).sum())
                    .collect();
                let peak = hi.iter().cloned().fold(0.0, f64::max);
                let r = hi.iter().take_while(|v| **v > 0.25 * peak).count();
                ring[i].push(r as f64 / 100.0);
            }
        }
    }
    for (i, c) in Class::ALL.iter().enumerate() {
        if acc[i].is_empty() {
            continue;
        }
        let n = acc[i][centres.len()];
        let top = acc[i][..centres.len()].iter().cloned().fold(0.0, f64::max);
        let mut line = format!("{c:?} ({n} hits, ring median {:.2}s):", {
            let mut r = ring[i].clone();
            r.sort_by(|a, b| a.total_cmp(b));
            r[r.len() / 2]
        });
        for (k, f) in centres.iter().enumerate().step_by(2) {
            let db = 20.0 * cantus_dsp::math::log10((acc[i][k] / top).max(1e-6));
            line += &format!(" {:.0}:{:.0}", f, db);
        }
        println!("{line}");
    }
}

/// Coordinate search of the decisions' numbers on the tuning tracks (the first eight):
/// the mean of the five classes' F-measures, each track's factorisation computed once.
/// Prints the best numbers; `Tuning::default` is set from them by hand.
#[test]
#[ignore]
fn mdb_tune() {
    use cantus_dsp::onset::drums::{factorise, hits, spectrogram, Tuning};
    let Some(root) = std::env::var_os("CANTUS_MDB_DRUMS").map(PathBuf::from) else {
        return;
    };
    let mut names: Vec<String> = std::fs::read_dir(root.join("annotations").join("class"))
        .unwrap()
        .map(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .replace("_class.txt", "")
        })
        .collect();
    names.sort();
    let tracks: Vec<_> = names
        .iter()
        .take(DEV)
        .map(|name| {
            let ann = std::fs::read_to_string(
                root.join("annotations")
                    .join("class")
                    .join(format!("{name}_class.txt")),
            )
            .unwrap();
            let truth: Vec<(f64, Class)> = ann
                .lines()
                .filter_map(|l| {
                    let mut it = l.split_whitespace();
                    let t: f64 = it.next()?.parse().ok()?;
                    Some((t, class(it.next()?)?))
                })
                .collect();
            let (x, rate) = read(
                &root
                    .join("audio")
                    .join("drum_only")
                    .join(format!("{name}_Drum.wav")),
            );
            let s = spectrogram(&x, rate);
            let f = factorise(&s);
            (truth, s, f)
        })
        .collect();
    // Our own kit's renders, when given (`cantus-engine/tests/transcribe_drums.rs` writes
    // them with `CANTUS_DRUMS_WRITE`): the search then weighs them as much as MDB.
    let extra: Vec<_> = std::env::var_os("CANTUS_DRUMS_EXTRA")
        .map(PathBuf::from)
        .map(|dir| {
            let mut v: Vec<_> = std::fs::read_dir(&dir)
                .unwrap()
                .map(|e| e.unwrap().path())
                .filter(|p| p.to_string_lossy().ends_with("_Drum.wav"))
                .collect();
            v.sort();
            v.into_iter()
                .map(|wav| {
                    let ann = std::fs::read_to_string(
                        wav.to_string_lossy().replace("_Drum.wav", "_class.txt"),
                    )
                    .unwrap();
                    let truth: Vec<(f64, Class)> = ann
                        .lines()
                        .filter_map(|l| {
                            let mut it = l.split_whitespace();
                            let t: f64 = it.next()?.parse().ok()?;
                            Some((t, class(it.next()?)?))
                        })
                        .collect();
                    let (x, rate) = read(&wav);
                    let s = spectrogram(&x, rate);
                    let f = factorise(&s);
                    (truth, s, f)
                })
                .collect()
        })
        .unwrap_or_default();
    let group = |set: &Vec<(
        Vec<(f64, Class)>,
        cantus_dsp::onset::drums::Spectrogram,
        cantus_dsp::onset::drums::Factors,
    )>,
                 tune: &Tuning,
                 sens: f32|
     -> Vec<f64> {
        let mut per = vec![Vec::new(); 5];
        for (truth, s, f) in set {
            let found = hits(s, f, sens, tune);
            for (i, c) in Class::ALL.iter().enumerate() {
                let t: Vec<f64> = truth.iter().filter(|h| h.1 == *c).map(|h| h.0).collect();
                let e: Vec<f64> = found
                    .iter()
                    .filter(|h| h.class == *c)
                    .map(|h| h.time)
                    .collect();
                per[i].push(onsets(&t, &e, 0.05));
            }
        }
        per.iter().map(|v| Score::sum(v).f).collect()
    };
    let score = |tune: &Tuning, sens: f32| -> (f64, Vec<f64>) {
        let a = group(&tracks, tune, sens);
        if extra.is_empty() {
            return (a.iter().sum::<f64>() / 5.0, a);
        }
        let b = group(&extra, tune, sens);
        let mut both = a.clone();
        both.extend(&b);
        ((a.iter().sum::<f64>() + b.iter().sum::<f64>()) / 10.0, both)
    };
    let _unused = |tune: &Tuning, sens: f32| -> (f64, Vec<f64>) {
        let mut per = vec![Vec::new(); 5];
        for (truth, s, f) in &tracks {
            let found = hits(s, f, sens, tune);
            for (i, c) in Class::ALL.iter().enumerate() {
                let t: Vec<f64> = truth.iter().filter(|h| h.1 == *c).map(|h| h.0).collect();
                let e: Vec<f64> = found
                    .iter()
                    .filter(|h| h.class == *c)
                    .map(|h| h.time)
                    .collect();
                per[i].push(onsets(&t, &e, 0.05));
            }
        }
        let fs: Vec<f64> = per.iter().map(|v| Score::sum(v).f).collect();
        (fs.iter().sum::<f64>() / 5.0, fs)
    };
    if let Ok(list) = std::env::var("CANTUS_MDB_SPARSITY") {
        // A scan of the factorisation's sparsity (each re-factorises every track).
        for sp in list.split(',').map(|v| v.parse::<f32>().unwrap()) {
            let mut per = vec![Vec::new(); 5];
            for (truth, s, _) in &tracks {
                let f = cantus_dsp::onset::drums::factorise_with(s, sp);
                let found = hits(s, &f, 0.5, &Tuning::default());
                for (i, c) in Class::ALL.iter().enumerate() {
                    let t: Vec<f64> = truth.iter().filter(|h| h.1 == *c).map(|h| h.0).collect();
                    let e: Vec<f64> = found
                        .iter()
                        .filter(|h| h.class == *c)
                        .map(|h| h.time)
                        .collect();
                    per[i].push(onsets(&t, &e, 0.05));
                }
            }
            let fs: Vec<f64> = per.iter().map(|v| Score::sum(v).f).collect();
            println!(
                "sparsity {sp}: {:.4} {fs:.3?}",
                fs.iter().sum::<f64>() / 5.0
            );
        }
        return;
    }
    let mut best = Tuning::default();
    let mut sens = 0.5f32; // the default's δ (0.12)
    let (mut top, fs) = score(&best, sens);
    println!("start {top:.4} {fs:.3?}");
    for (sl, sm) in [(true, false), (false, true), (true, true)] {
        let t = Tuning {
            split_low: sl,
            split_metal: sm,
            ..best.clone()
        };
        let (m, f) = score(&t, sens);
        println!("split low {sl} metal {sm}: {m:.4} {f:.3?}");
    }
    let leaks = [
        (0, 1),
        (1, 0),
        (1, 2),
        (1, 3),
        (1, 4),
        (2, 0),
        (2, 1),
        (3, 1),
        (3, 4),
        (4, 1),
        (4, 3),
        (0, 2),
    ];
    for round in 0..3 {
        for v in [0.2f32, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8] {
            let (m, _) = score(&best, v);
            if m > top {
                top = m;
                sens = v;
            }
        }
        for v in [0.45f32, 0.55, 0.65, 0.75, 0.85, 0.95] {
            let t = Tuning {
                tom_share: v,
                ..best.clone()
            };
            let (m, _) = score(&t, sens);
            if m > top {
                top = m;
                best = t;
            }
        }
        for v in [70.0, 80.0, 90.0, 100.0, 110.0, 125.0] {
            let t = Tuning {
                tom_body_hz: v,
                ..best.clone()
            };
            let (m, _) = score(&t, sens);
            if m > top {
                top = m;
                best = t;
            }
        }
        for v in [0.8f32, 1.0, 1.5, 2.0, 3.0, 5.0] {
            let t = Tuning {
                cymbal_rise: v,
                ..best.clone()
            };
            let (m, _) = score(&t, sens);
            if m > top {
                top = m;
                best = t;
            }
        }
        for v in [0.08, 0.12, 0.18, 0.25, 0.35, 0.5] {
            let t = Tuning {
                cymbal_ring: v,
                ..best.clone()
            };
            let (m, _) = score(&t, sens);
            if m > top {
                top = m;
                best = t;
            }
        }
        for v in [0.0f32, 0.4, 0.6, 0.8, 1.0, 1.3, 1.7, 2.2, 3.0] {
            let t = Tuning {
                hat_bright: v,
                ..best.clone()
            };
            let (m, _) = score(&t, sens);
            if m > top {
                top = m;
                best = t;
            }
        }
        for v in [0.5f32, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0] {
            let t = Tuning {
                rare: v,
                ..best.clone()
            };
            let (m, _) = score(&t, sens);
            if m > top {
                top = m;
                best = t;
            }
        }
        for &(a, b) in &leaks {
            for v in [0.0f32, 0.1, 0.2, 0.3, 0.45, 0.6, 0.8, 1.0, 1.5, 2.0, 3.0] {
                let mut t = best.clone();
                t.leak[a][b] = v;
                let (m, _) = score(&t, sens);
                if m > top {
                    top = m;
                    best = t;
                }
            }
        }
        for (sl, sm) in [(false, false), (true, false), (false, true), (true, true)] {
            let t = Tuning {
                split_low: sl,
                split_metal: sm,
                ..best.clone()
            };
            let (m, _) = score(&t, sens);
            if m > top {
                top = m;
                best = t;
            }
        }
        let (_, fs) = score(&best, sens);
        println!("round {round}: {top:.4} {fs:.3?} sensitivity {sens}\n{best:?}");
    }
}
