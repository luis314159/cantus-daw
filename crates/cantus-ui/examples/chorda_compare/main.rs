//! Chorda against a reference piano, measured (round 37, ADR 0098).
//!
//! ```text
//! cargo run --release -p cantus-ui --example chorda_compare -- install <downloads>
//! cargo run --release -p cantus-ui --example chorda_compare -- render <downloads> <out> [source…]
//! cargo run --release -p cantus-ui --example chorda_compare -- analyse <out> [source…]
//! cargo run --release -p cantus-ui --example chorda_compare -- all <downloads> <out>
//! ```
//!
//! `install` puts the Salamander Grand (CC BY 3.0, Alexander Holm) into `<downloads>/content`
//! through Cantus's own pack code. `render` plays every phrase (`chorda_kit/phrases.rs`)
//! through the real engine — the Salamander in Exemplar as the picker installs it, Chorda's
//! model 1 and model 2 at their defaults — into `<out>/wav/<source>/<phrase>.wav` (32-bit
//! float, 48 kHz). A source is `salamander`, `chorda1`, `chorda2`, or `name=model:key=v,…`
//! (a variant while voicing). `analyse` measures them (`chorda_kit/analyse.rs`) and writes
//! `<out>/report.md`, `<out>/notes.csv`, `<out>/phrases.csv` and the plots in `<out>/plots/`.
//!
//! `lab <out> <name> <key=v,…>` is the voicing lab (`chorda_kit/lab.rs`): model 2's DSP
//! driven directly with a voicing to try, into `<out>/wav/<name>/` — then `analyse <out>
//! salamander chorda1 <name>` compares it.

#[path = "../chorda_kit/analyse.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod analyse;
#[path = "../chorda_kit/figures.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod figures;
#[path = "../chorda_kit/lab.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod lab;
#[path = "../chorda_kit/phrases.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod phrases;
#[path = "../chorda_kit/plot.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod plot;
#[path = "../chorda_kit/render.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod render;
#[path = "../chorda_kit/report.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod report;

use render::Source;
use std::path::{Path, PathBuf};

/// A source by name: `salamander`, `chorda1`, `chorda2`, or `name=model:key=v,key=v`.
fn source(s: &str) -> (String, Source) {
    match s {
        "salamander" => (s.into(), Source::Salamander),
        "chorda1" => (
            s.into(),
            Source::Chorda {
                model: 1.0,
                values: Vec::new(),
            },
        ),
        "chorda2" => (
            s.into(),
            Source::Chorda {
                model: 2.0,
                values: Vec::new(),
            },
        ),
        other => {
            let (name, rest) = other.split_once('=').expect("name=model:key=v,…");
            let (model, vals) = rest.split_once(':').unwrap_or((rest, ""));
            let values = vals
                .split(',')
                .filter(|kv| !kv.is_empty())
                .map(|kv| {
                    let (k, v) = kv.split_once('=').expect("key=value");
                    (k.to_string(), v.parse::<f32>().expect("a number"))
                })
                .collect();
            (
                name.into(),
                Source::Chorda {
                    model: model.parse().expect("a model number"),
                    values,
                },
            )
        }
    }
}

const DEFAULT_SOURCES: [&str; 3] = ["salamander", "chorda1", "chorda2"];

fn sources(args: &[String]) -> Vec<(String, Source)> {
    if args.is_empty() {
        DEFAULT_SOURCES.iter().map(|s| source(s)).collect()
    } else {
        args.iter().map(|s| source(s)).collect()
    }
}

fn render_all(downloads: &Path, out: &Path, which: &[(String, Source)], only: Option<&str>) {
    let root = render::content_root(downloads);
    render::set_cache(&downloads.join("stream-cache"));
    for (name, src) in which {
        let dir = out.join("wav").join(name);
        std::fs::create_dir_all(&dir).expect("the output folder");
        for ph in phrases::all() {
            if only.is_some_and(|o| !o.split(',').any(|x| x == ph.id)) {
                continue;
            }
            let t = std::time::Instant::now();
            match render::render_source(src, &root, &ph) {
                Ok(s) => {
                    let path = dir.join(format!("{}.wav", ph.id));
                    render::write_wav(&path, &s, true).expect("the WAV is written");
                    eprintln!(
                        "{name:>12} {:<12} {:6.1} s audio in {:5.1} s, peak {:6.1} dBFS",
                        ph.id,
                        s.len() as f64 / render::SR as f64,
                        t.elapsed().as_secs_f64(),
                        20.0 * cantus_dsp::math::log10(s.peak().max(1e-9) as f64)
                    );
                }
                Err(e) => eprintln!("{name}: {}: {e}", ph.id),
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let only = std::env::var("CHORDA_PHRASES").ok();
    match args.first().map(String::as_str) {
        Some("install") => {
            let d = PathBuf::from(&args[1]);
            match render::install(&render::content_root(&d)) {
                Ok(()) => eprintln!("installed"),
                Err(e) => {
                    eprintln!("install failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("render") => {
            let (d, o) = (PathBuf::from(&args[1]), PathBuf::from(&args[2]));
            render_all(&d, &o, &sources(&args[3..]), only.as_deref());
        }
        Some("lab") => {
            let (o, name) = (PathBuf::from(&args[1]), args[2].clone());
            let spec = args.get(3).cloned().unwrap_or_default();
            let dir = o.join("wav").join(&name);
            std::fs::create_dir_all(&dir).expect("the output folder");
            let all: Vec<_> = phrases::all()
                .into_iter()
                .filter(|p| {
                    only.as_deref()
                        .is_none_or(|o| o.split(',').any(|x| x == p.id))
                })
                .collect();
            std::thread::scope(|sc| {
                for ph in &all {
                    let (dir, spec) = (&dir, &spec);
                    sc.spawn(move || {
                        let s = lab::render(spec, ph);
                        render::write_wav(&dir.join(format!("{}.wav", ph.id)), &s, true)
                            .expect("the WAV is written");
                    });
                }
            });
            eprintln!("lab {name}: {spec}");
        }
        Some("analyse") => {
            let o = PathBuf::from(&args[1]);
            let names: Vec<String> = if args.len() > 2 {
                args[2..]
                    .iter()
                    .map(|s| s.split('=').next().unwrap_or(s).to_string())
                    .collect()
            } else {
                DEFAULT_SOURCES.iter().map(|s| s.to_string()).collect()
            };
            figures::write(&o, &names).expect("the report is written");
        }
        Some("all") => {
            let (d, o) = (PathBuf::from(&args[1]), PathBuf::from(&args[2]));
            if let Err(e) = render::install(&render::content_root(&d)) {
                eprintln!("install failed: {e}");
                std::process::exit(1);
            }
            render_all(&d, &o, &sources(&[]), only.as_deref());
            let names: Vec<String> = DEFAULT_SOURCES.iter().map(|s| s.to_string()).collect();
            figures::write(&o, &names).expect("the report is written");
        }
        _ => {
            eprintln!("usage: chorda_compare install|render|analyse|all … (see the file's header)");
            std::process::exit(2);
        }
    }
}
