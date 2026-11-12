//! The owner's listening kit for Chorda (round 37, ADR 0098): an offline ABX test and a blind
//! preference test, Chorda's model 2 against the Salamander Grand and against model 1.
//!
//! ```text
//! cargo run --release -p cantus-ui --example chorda_abx -- <dir> [<downloads>] [--flac]
//! ```
//!
//! Renders every listening phrase (`chorda_kit/phrases.rs`, `listen`) through the real
//! engine — the Salamander Grand (CC BY 3.0, Alexander Holm; installed through Cantus's own
//! pack code into `<downloads>/content`, default `F:/projects/daw-r37/downloads/piano`),
//! Chorda model 1 and model 2 at their defaults — matches their loudness (ITU-R BS.1770,
//! integrated, the same target for the three, lowered together if a peak would pass
//! −1 dBFS), trims them to one length, and writes into `<dir>`:
//!
//! - `audio/<opaque>.wav` (24-bit, 48 kHz — or `.flac`, the same samples losslessly in about
//!   half the bytes, with `--flac`: the kit as a web page's; names hashed with a salt drawn per kit, so a
//!   file's name never says which piano it is),
//! - `index.html` — one page, no network, no external script: the ABX (16 trials per
//!   comparison, X drawn at random each trial, answers kept in the browser and offered as a
//!   JSON file, the score and its binomial p-value at the end) and a preference question per
//!   phrase and pair, blind,
//! - `README.md` / `LEEME.md` — how to take it, in English and Spanish,
//! - `levels.csv` — what each file measured before and after the match.

#[path = "../chorda_kit/phrases.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod phrases;
#[path = "../chorda_kit/render.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod render;

use render::{Source, Stereo};
use std::fmt::Write as _;
use std::path::PathBuf;

const PAGE: &str = include_str!("../chorda_kit/abx.html");
const README_EN: &str = include_str!("../chorda_kit/abx_readme.en.md");
const README_ES: &str = include_str!("../chorda_kit/abx_readme.es.md");

/// The loudness every file is brought to (LUFS), unless a peak would pass the ceiling.
const TARGET: f64 = -20.0;
const CEILING_DB: f64 = -1.0;

fn lufs(s: &Stereo) -> f64 {
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(render::SR as f32);
    m.process(&s.l, &s.r);
    m.integrated().unwrap_or(-120.0)
}

fn db(x: f32) -> f64 {
    20.0 * cantus_dsp::math::log10(x.max(1e-9) as f64)
}

fn main() {
    let all: Vec<String> = std::env::args().skip(1).collect();
    let flac = all.iter().any(|a| a == "--flac");
    let args: Vec<String> = all.into_iter().filter(|a| a != "--flac").collect();
    let ext = if flac { "flac" } else { "wav" };
    let Some(dir) = args.first().map(PathBuf::from) else {
        eprintln!("usage: chorda_abx <dir> [<downloads>]");
        std::process::exit(2);
    };
    let downloads = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("F:/projects/daw-r37/downloads/piano"));
    let root = render::content_root(&downloads);
    if let Err(e) = render::install(&root) {
        eprintln!("the Salamander Grand could not be installed: {e}");
        std::process::exit(1);
    }
    render::set_cache(&downloads.join("stream-cache"));
    std::fs::create_dir_all(dir.join("audio")).expect("the kit's folder");
    let sources: [(&str, Source); 3] = [
        ("salamander", Source::Salamander),
        (
            "chorda1",
            Source::Chorda {
                model: 1.0,
                values: Vec::new(),
            },
        ),
        (
            "chorda2",
            Source::Chorda {
                model: 2.0,
                values: Vec::new(),
            },
        ),
    ];
    let mut levels = String::from(
        "phrase,source,lufs_before,peak_before_dbfs,gain_db,lufs_after,peak_after_dbfs\n",
    );
    let mut list = String::new();
    let mut files = String::new();
    // A salt per kit (the clock): the names are opaque, and two kits do not share them.
    let salt = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5EED);
    let opaque = |phrase: &str, src: &str| -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ salt;
        for b in phrase.bytes().chain(*b"|").chain(src.bytes()) {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
        h ^= h >> 29;
        format!("{h:016x}.{ext}")
    };
    for ph in phrases::all().into_iter().filter(|p| p.listen) {
        let mut renders: Vec<Stereo> = sources
            .iter()
            .map(|(name, src)| {
                render::render_source(src, &root, &ph)
                    .unwrap_or_else(|e| panic!("{name} {}: {e}", ph.id))
            })
            .collect();
        let n = renders.iter().map(|s| s.len()).min().unwrap_or(0);
        for s in renders.iter_mut() {
            *s = s.slice(0, n);
            // A 30 ms fade at the end, the same for all three.
            let f = (0.03 * render::SR as f64) as usize;
            for i in 0..f.min(n) {
                let g = i as f32 / f as f32;
                s.l[n - 1 - i] *= g;
                s.r[n - 1 - i] *= g;
            }
        }
        let before: Vec<(f64, f64)> = renders.iter().map(|s| (lufs(s), db(s.peak()))).collect();
        // One target for the three: lowered together if any would pass the ceiling.
        let target = before
            .iter()
            .map(|(l, p)| TARGET.min(l + (CEILING_DB - p)))
            .fold(TARGET, f64::min);
        for ((name, _), (s, (l, p))) in sources.iter().zip(renders.iter_mut().zip(&before)) {
            let mut g = target - l;
            s.gain(cantus_dsp::math::powf(10.0f64, g / 20.0) as f32);
            // Once more: BS.1770's absolute gate lets a quiet tail in or out as the level
            // moves, so the first gain lands within about half a LU; the second on it.
            let again = target - lufs(s);
            g += again;
            s.gain(cantus_dsp::math::powf(10.0f64, again / 20.0) as f32);
            let file = opaque(ph.id, name);
            let path = dir.join("audio").join(&file);
            if !files.is_empty() {
                files.push(',');
            }
            let _ = write!(files, "\"{}|{name}\":\"{file}\"", ph.id);
            if flac {
                render::write_flac(&path, s).expect("the FLAC is written");
            } else {
                render::write_wav(&path, s, false).expect("the WAV is written");
            }
            let _ = writeln!(
                levels,
                "{},{name},{l:.2},{p:.2},{g:.2},{:.2},{:.2}",
                ph.id,
                lufs(s),
                db(s.peak())
            );
        }
        eprintln!(
            "{:<12} matched at {target:.1} LUFS ({:.1} s)",
            ph.id,
            n as f64 / render::SR as f64
        );
        if !list.is_empty() {
            list.push(',');
        }
        let _ = write!(list, "\"{}\"", ph.id);
    }
    std::fs::write(dir.join("levels.csv"), levels).expect("levels.csv");
    let page = PAGE
        .replace("__PHRASES__", &format!("[{list}]"))
        .replace("__FILES__", &format!("{{{files}}}"));
    std::fs::write(dir.join("index.html"), page).expect("index.html");
    std::fs::write(dir.join("README.md"), README_EN).expect("README.md");
    std::fs::write(dir.join("LEEME.md"), README_ES).expect("LEEME.md");
    eprintln!("the kit is in {} — open index.html", dir.display());
}
