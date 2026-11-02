//! A fuzz test per parser (ADR 0089 §5): seeded mutations of valid files — bytes flipped,
//! cut, repeated; numbers made huge, negative or nonsense; structures nested, emptied and
//! duplicated — never panic, and whatever loads also draws without one.
//!
//! `CANTUS_FUZZ_ROUNDS=<n>` runs more rounds per format (the default keeps the suite fast).

use cantus_dance::import::{self, samples, Limits};
use cantus_dance::render::{self, Background, Stage};
use cantus_dance::{pose_at, DanceFrame};
use serde_json::Value;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

fn rounds() -> usize {
    std::env::var("CANTUS_FUZZ_ROUNDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(150)
}

/// Byte-level damage.
fn mutate_bytes(rng: &mut Rng, b: &[u8]) -> Vec<u8> {
    let mut out = b.to_vec();
    for _ in 0..1 + rng.below(4) {
        if out.is_empty() {
            break;
        }
        let at = rng.below(out.len());
        match rng.below(6) {
            0 => out[at] = rng.next() as u8,
            1 => out.truncate(at),
            2 => {
                let len = rng.below(64).min(out.len() - at);
                let piece = out[at..at + len].to_vec();
                for _ in 0..1 + rng.below(8) {
                    out.splice(at..at, piece.iter().copied());
                }
            }
            3 => {
                let digits = [
                    "9e99",
                    "-1",
                    "1e308",
                    "-0",
                    "0",
                    "65535",
                    "4294967296",
                    "NaN",
                ];
                let d = digits[rng.below(digits.len())];
                out.splice(at..at, d.bytes());
            }
            4 => out.insert(at, b"[{\"]}:,"[rng.below(7)]),
            _ => {
                let end = (at + rng.below(32)).min(out.len());
                out.drain(at..end);
            }
        }
    }
    out
}

/// Structure-level damage to a JSON document.
fn mutate_json(rng: &mut Rng, v: &mut Value, depth: usize) {
    // Ids given twice (review 1): an asset, an armature, a layer, a marker repeated whole.
    if depth == 0 && rng.below(4) == 0 {
        for key in ["assets", "armature", "layers", "markers"] {
            if let Some(a) = v.get_mut(key).and_then(Value::as_array_mut) {
                if let Some(first) = a.first().cloned() {
                    a.push(first);
                }
            }
        }
    }
    match v {
        Value::Object(m) if !m.is_empty() => {
            let keys: Vec<String> = m.keys().cloned().collect();
            let k = &keys[rng.below(keys.len())];
            if depth > 6 || rng.below(3) == 0 {
                let choice = rng.below(9);
                let replacement = match choice {
                    0 => Value::Null,
                    1 => serde_json::json!(-1),
                    2 => serde_json::json!(1e300),
                    3 => serde_json::json!("x"),
                    4 => serde_json::json!([]),
                    5 => serde_json::json!({}),
                    6 => serde_json::json!(4_000_000_000u64),
                    7 => {
                        let mut deep = serde_json::json!(1);
                        for _ in 0..40 {
                            deep = serde_json::json!([deep]);
                        }
                        deep
                    }
                    _ => serde_json::json!([0, -5, 1e9, "a", null]),
                };
                m.insert(k.clone(), replacement);
            } else if let Some(child) = m.get_mut(k) {
                mutate_json(rng, child, depth + 1);
            }
        }
        Value::Array(a) if !a.is_empty() => {
            let i = rng.below(a.len());
            match rng.below(4) {
                0 => {
                    let copy = a[i].clone();
                    for _ in 0..rng.below(50) {
                        a.push(copy.clone());
                    }
                }
                1 => {
                    a.remove(i);
                }
                _ => mutate_json(rng, &mut a[i], depth + 1),
            }
        }
        Value::Number(_) => {
            *v = serde_json::json!([-1e9, 0.0, 1e9, -0.5, 3.5][rng.below(5)]);
        }
        _ => {}
    }
}

fn exercise(bytes: &[u8], folder: &Path) {
    let limits = Limits::DEFAULT;
    if let Ok(found) = import::inspect_bytes(bytes, folder, &limits) {
        let settings = import::guessed(&found);
        let ch = import::character(found, "file:fuzz".into(), settings);
        let stage = Stage::new(96, 96, Background::Transparent);
        for i in 0..3 {
            let f = DanceFrame {
                beats: f64::from(i) * 1.37,
                seconds: f64::from(i) * 0.7,
                playing: true,
                bar: i,
                bar_start_beats: f64::from(i) * 4.0,
                ..DanceFrame::default()
            };
            let pose = pose_at(&f, &ch, 0.0);
            let _ = render::render(&stage, &[(&ch, pose)]);
        }
    }
}

fn fuzz(name: &str, files: Vec<samples::File>, main: &str, seed: u64) {
    let dir = tempfile::tempdir().unwrap();
    samples::write(dir.path(), &files).unwrap();
    let original = files.iter().find(|f| f.0 == main).unwrap().1.clone();
    let json: Option<Value> = serde_json::from_slice(&original).ok();
    let mut rng = Rng(seed);
    for round in 0..rounds() {
        let bytes = match (&json, round % 2) {
            (Some(v), 0) => {
                let mut v = v.clone();
                for _ in 0..1 + rng.below(3) {
                    mutate_json(&mut rng, &mut v, 0);
                }
                serde_json::to_vec(&v).unwrap()
            }
            _ => mutate_bytes(&mut rng, &original),
        };
        let r = catch_unwind(AssertUnwindSafe(|| exercise(&bytes, dir.path())));
        if r.is_err() {
            let keep = std::env::temp_dir().join(format!("cantus-fuzz-{name}-{round}.bin"));
            let _ = std::fs::write(&keep, &bytes);
            panic!(
                "{name}: round {round} panicked (the file: {})",
                keep.display()
            );
        }
    }
    // The other files of a package damaged too: its images.
    for (n, b) in files.iter().filter(|f| f.0.ends_with(".png")) {
        for _ in 0..rounds() / 3 {
            std::fs::write(dir.path().join(n), mutate_bytes(&mut rng, b)).unwrap();
            let r = catch_unwind(AssertUnwindSafe(|| exercise(&original, dir.path())));
            assert!(r.is_ok(), "{name}: a damaged {n} panicked");
        }
        std::fs::write(dir.path().join(n), b).unwrap();
    }
}

#[test]
fn lottie_never_panics() {
    fuzz("lottie", samples::lottie("f"), "f.json", 1);
}

#[test]
fn lottie_packages_never_panic() {
    fuzz("dotlottie", samples::dotlottie("f"), "f.lottie", 2);
}

#[test]
fn aseprite_sheets_never_panic() {
    fuzz("aseprite", samples::aseprite("f"), "f.json", 3);
}

#[test]
fn texturepacker_sheets_never_panic() {
    fuzz(
        "texturepacker",
        samples::texturepacker("f", true),
        "f.json",
        4,
    );
}

#[test]
fn dragonbones_never_panics() {
    fuzz("dragonbones", samples::dragonbones("f"), "f_ske.json", 5);
}

/// Our figure inside a precomposition with an image beside it: assets to repeat, ids to
/// collide (review 1, item 1).
#[test]
fn lottie_precomps_never_panic() {
    let mut v = samples::lottie_doc("f");
    let layers = v["layers"].take();
    v["assets"] = serde_json::json!([
        {"id": "p0", "layers": layers},
        {"id": "im", "w": 4, "h": 4, "p": "data:image/png;base64,AAAA"}
    ]);
    v["layers"] = serde_json::json!([{"ty": 0, "refId": "p0", "ind": 1, "ip": 0, "op": 192,
        "st": 0, "ks": {}, "w": 512, "h": 512}]);
    let files = vec![("f.json".to_string(), serde_json::to_vec(&v).unwrap())];
    fuzz("lottie-precomps", files, "f.json", 6);
}
