//! `cantus-bench hashes`: the null corpus as hashes, for comparing builds (round 22,
//! ADR 0031 §5.3).
//!
//! The same source must give the same bits whatever the build's `target-cpu`: Rust never
//! fuses `a * b + c` into an FMA on its own, LLVM does not reorder float sums without
//! fast-math, and every transcendental goes through `cantus_dsp::math` (pinned `libm`, R9).
//! `xtask null-hashes` builds this program at x86-64-v2 and x86-64-v3 (and v4 where the
//! machine runs it), runs each, and requires the lists to be equal, line for line.
//!
//! The corpus, each item a SHA-256 of its samples' bits (f32, little-endian):
//!
//! - `math.<fn>.<f32|f64>` — every `cantus_dsp::math` function over 4 096 points;
//! - `fx.<kind>.<default|heavy>` — every native effect, alone, on two seconds of the
//!   bench's busy signal in 128-frame blocks (the kernels' settings, `kernels::heavy`);
//! - `inst.<name>.v8` — every instrument holding eight voices, rendered offline;
//! - `demo.<id>` — each demo song's eight bars where the whole band plays, rendered
//!   offline (`--whole`: every song whole, `demo.<id>.whole`); no samples on disk, so a
//!   sampled pad is silence, the same on every build;
//! - `fixture.<vN>` — every saved-project fixture of `cantus-project`, opened and rendered.

use crate::kernels::{self, BLOCK};
use crate::rig::SAMPLE_RATE;
use cantus_core::{EffectKind, EffectSlot, EffectSpec, Project, Snapshot, Ticks};
use cantus_dsp::math;
use cantus_engine::{render, EffectUnit, RenderOptions};
use sha2::{Digest, Sha256};
use std::sync::Arc;

/// One item of the corpus.
#[derive(Clone, Debug, PartialEq)]
pub struct Hashed {
    pub name: String,
    pub samples: usize,
    pub sha256: String,
    /// The largest magnitude: a corpus of silence would prove nothing.
    pub peak: f64,
}

impl std::fmt::Display for Hashed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}  {}  {}", self.sha256, self.samples, self.name)
    }
}

fn hash_f32(name: impl Into<String>, samples: &[f32]) -> Hashed {
    let mut h = Sha256::new();
    for s in samples {
        h.update(s.to_le_bytes());
    }
    Hashed {
        name: name.into(),
        samples: samples.len(),
        sha256: h.finalize().iter().map(|b| format!("{b:02x}")).collect(),
        peak: samples.iter().fold(0.0f64, |m, s| m.max(s.abs() as f64)),
    }
}

fn hash_f64(name: impl Into<String>, values: &[f64]) -> Hashed {
    let mut h = Sha256::new();
    for s in values {
        h.update(s.to_le_bytes());
    }
    Hashed {
        name: name.into(),
        samples: values.len(),
        sha256: h.finalize().iter().map(|b| format!("{b:02x}")).collect(),
        peak: values.iter().fold(0.0f64, |m, s| m.max(s.abs())),
    }
}

/// `n` points spread over `[lo, hi]`.
fn grid(lo: f64, hi: f64) -> impl Iterator<Item = f64> {
    (0..4096).map(move |i| lo + (hi - lo) * i as f64 / 4095.0)
}

type F1 = (&'static str, f64, f64, fn(f32) -> f32, fn(f64) -> f64);

/// Every `cantus_dsp::math` function, in f32 and in f64.
pub fn math_items() -> Vec<Hashed> {
    let one: [F1; 17] = [
        ("sin", -10.0, 10.0, math::sin, math::sin),
        ("cos", -10.0, 10.0, math::cos, math::cos),
        ("tan", -1.5, 1.5, math::tan, math::tan),
        ("asin", -1.0, 1.0, math::asin, math::asin),
        ("acos", -1.0, 1.0, math::acos, math::acos),
        ("atan", -20.0, 20.0, math::atan, math::atan),
        ("sinh", -5.0, 5.0, math::sinh, math::sinh),
        ("cosh", -5.0, 5.0, math::cosh, math::cosh),
        ("tanh", -8.0, 8.0, math::tanh, math::tanh),
        ("exp", -20.0, 10.0, math::exp, math::exp),
        ("exp2", -30.0, 15.0, math::exp2, math::exp2),
        ("exp_m1", -2.0, 2.0, math::exp_m1, math::exp_m1),
        ("ln", 1e-6, 1e4, math::ln, math::ln),
        ("log2", 1e-6, 1e4, math::log2, math::log2),
        ("log10", 1e-6, 1e4, math::log10, math::log10),
        ("ln_1p", -0.9, 10.0, math::ln_1p, math::ln_1p),
        ("cbrt", -100.0, 100.0, math::cbrt, math::cbrt),
    ];
    let mut out = Vec::new();
    for (name, lo, hi, f32_fn, f64_fn) in one {
        let a: Vec<f32> = grid(lo, hi).map(|x| f32_fn(x as f32)).collect();
        let b: Vec<f64> = grid(lo, hi).map(f64_fn).collect();
        out.push(hash_f32(format!("math.{name}.f32"), &a));
        out.push(hash_f64(format!("math.{name}.f64"), &b));
    }
    // Two arguments: a second grid crossed with the first.
    let pow32: Vec<f32> = grid(1e-3, 10.0)
        .flat_map(|x| [-2.5f32, -0.5, 0.33, 1.7, 3.0].map(|y| math::powf(x as f32, y)))
        .collect();
    let pow64: Vec<f64> = grid(1e-3, 10.0)
        .flat_map(|x| [-2.5, -0.5, 0.33, 1.7, 3.0].map(|y| math::powf(x, y)))
        .collect();
    let atan2_32: Vec<f32> = grid(-3.0, 3.0)
        .map(|x| math::atan2(x as f32, 1.0 - x as f32))
        .collect();
    let hypot64: Vec<f64> = grid(-3.0, 3.0).map(|x| math::hypot(x, 2.0 - x)).collect();
    let powi64: Vec<f64> = grid(-2.0, 2.0)
        .flat_map(|x| [-3, 2, 7].map(|n| math::powi(x, n)))
        .collect();
    out.push(hash_f32("math.powf.f32", &pow32));
    out.push(hash_f64("math.powf.f64", &pow64));
    out.push(hash_f32("math.atan2.f32", &atan2_32));
    out.push(hash_f64("math.hypot.f64", &hypot64));
    out.push(hash_f64("math.powi.f64", &powi64));
    out
}

/// Every native effect at its defaults and at heavy settings, on two seconds of the busy
/// signal: the output, both channels interleaved.
pub fn effect_items() -> Vec<Hashed> {
    let (sl, sr) = kernels::signal(2 * SAMPLE_RATE as usize);
    let mut out = Vec::new();
    for kind in EffectKind::ALL {
        if kind == EffectKind::External {
            continue;
        }
        for (label, spec) in [
            ("default", EffectSpec::new(kind)),
            ("heavy", kernels::heavy(kind)),
        ] {
            let slot = EffectSlot {
                id: cantus_core::EffectId(1),
                bypass: false,
                mix: 1.0,
                sidechain: None,
                midi_from: None,
                spec,
            };
            let mut unit = EffectUnit::new(&slot, SAMPLE_RATE as f32);
            let mut samples = Vec::with_capacity(sl.len() * 2);
            let (mut l, mut r) = (vec![0f32; BLOCK], vec![0f32; BLOCK]);
            for at in (0..sl.len() - BLOCK + 1).step_by(BLOCK) {
                l.copy_from_slice(&sl[at..at + BLOCK]);
                r.copy_from_slice(&sr[at..at + BLOCK]);
                unit.process_keyed(&mut l, &mut r, None);
                for (a, b) in l.iter().zip(&r) {
                    samples.push(*a);
                    samples.push(*b);
                }
            }
            out.push(hash_f32(
                format!("fx.{}.{label}", kernels::kind_id(kind)),
                &samples,
            ));
        }
    }
    out
}

fn render_project(p: &Project, start: Ticks, end: Option<Ticks>) -> Vec<f32> {
    let snap = Arc::new(Snapshot::compile(p, 1));
    let opts = RenderOptions {
        start,
        end,
        tail_seconds: 1.0,
        block_frames: 512,
        ..Default::default()
    };
    render(snap, &opts).expect("the corpus renders").samples
}

/// Every instrument holding eight voices, four beats and a second of tail.
pub fn instrument_items() -> Vec<Hashed> {
    kernels::instruments()
        .into_iter()
        .map(|(name, spec)| {
            let p = crate::sessions::instrument(spec, 8);
            hash_f32(
                format!("inst.{name}.v8"),
                &render_project(&p, Ticks::ZERO, Some(Ticks::beats(4))),
            )
        })
        .collect()
}

/// Each demo song: eight bars where the whole band plays (`demo.<id>`), or with `whole`
/// the whole song (`demo.<id>.whole`, six times longer: ten minutes of audio in all).
pub fn demo_items(whole: bool) -> Vec<Hashed> {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    cantus_ui::demos::ALL
        .iter()
        .map(|d| {
            let p = (d.build)();
            if whole {
                let samples = render_project(&p, Ticks::ZERO, None);
                return hash_f32(format!("demo.{}.whole", d.id), &samples);
            }
            let bar = p.time_signature.ticks_per_bar();
            let from = bar * crate::sessions::full_band_bar(d.id);
            let samples = render_project(&p, from, Some(from + bar * 8));
            hash_f32(format!("demo.{}", d.id), &samples)
        })
        .collect()
}

/// Every project fixture `cantus-project` keeps (`fixtures/v*.cantus`), opened as the app
/// opens a file and rendered.
pub fn fixture_items() -> Vec<Hashed> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../cantus-project/fixtures");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "cantus"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
        .iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(path).ok()?;
            let loaded = cantus_project::parse(&text, true).ok()?;
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            Some(hash_f32(
                format!("fixture.{stem}"),
                &render_project(&loaded.file.project, Ticks::ZERO, None),
            ))
        })
        .collect()
}

/// The whole corpus, in a fixed order; each part's time on stderr. `whole_demos`
/// renders the demo songs whole instead of eight bars each.
pub fn corpus(whole_demos: bool) -> Vec<Hashed> {
    let mut v = Vec::new();
    let mut part = |name: &str, items: &dyn Fn() -> Vec<Hashed>| {
        let t = std::time::Instant::now();
        let got = items();
        eprintln!(
            "cantus-bench hashes: {name}: {} items, {:.1} s",
            got.len(),
            t.elapsed().as_secs_f64()
        );
        v.extend(got);
    };
    part("math", &math_items);
    part("effects", &effect_items);
    part("instruments", &instrument_items);
    part("demos", &|| demo_items(whole_demos));
    part("fixtures", &fixture_items);
    // Every effect, instrument and demo must make sound: two builds agreeing on silence
    // would prove nothing. (A fixture may be an empty project.)
    for i in &v {
        let must_sound = ["fx.", "inst.", "demo."].iter().any(|p| i.name.starts_with(p));
        assert!(
            !must_sound || (i.peak > 1e-4 && i.peak.is_finite()),
            "{} renders silence or garbage (peak {})",
            i.name,
            i.peak
        );
    }
    v
}

/// The file `xtask null-hashes` compares: a header naming the build, then one line per
/// item (`<sha-256>  <samples>  <name>`). The header is not compared.
pub fn report(items: &[Hashed]) -> String {
    let mut s = format!(
        "# cantus null hashes; build {}; {} items\n",
        build_level(),
        items.len()
    );
    for i in items {
        s.push_str(&format!("{i}\n"));
    }
    s
}

/// The level this program was compiled for, from its target features.
pub fn build_level() -> &'static str {
    if cfg!(target_feature = "avx512f") {
        "x86-64-v4"
    } else if cfg!(target_feature = "avx2") {
        "x86-64-v3"
    } else if cfg!(target_feature = "sse4.2") {
        "x86-64-v2"
    } else {
        std::env::consts::ARCH
    }
}
