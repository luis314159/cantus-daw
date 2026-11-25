//! Chorda's recorded pianos, heard (round 40 phase 3, ADR 0125; checkpoint C): the
//! standard listening phrases (`chorda_kit/phrases.rs`) played by a **Chorda whose source
//! is a recorded piano** — the Salamander Grand and bitKlavier Grand's mic images — through
//! the real path: the pack installed by Cantus's own code (download, SHA-256, unpack, the
//! piano build writing bitKlavier's SFZ), read as the instrument picker reads it, switched
//! into Chorda (`chorda_source::with_source`), compiled by `Snapshot::compile` into the
//! Exemplar spec of its zones and rendered by the engine (the export's path).
//!
//! ```text
//! cargo run --release -p cantus-ui --example chorda_pianos -- <out-dir> <content-root> \
//!     [<salamander-root>] [--abx <kit-dir>]
//! ```
//!
//! `<salamander-root>` defaults to `$CANTUS_REFS/piano/content` (tools/sound-refs).
//!
//! Writes `<phrase> - <n> <source>.flac` (24-bit, 48 kHz), every source of a phrase at one
//! loudness (ITU-R BS.1770 integrated, −20 LUFS, lowered together if a peak would pass
//! −1 dBFS), and `levels.csv`. With `--abx`, the blind ABX and preference kit
//! (`chorda_abx`'s page) for bitKlavier's default image against the Salamander.

mod measure;
#[path = "../chorda_kit/phrases.rs"]
#[allow(dead_code)] // shared by the examples: each uses part of it
mod phrases;
#[path = "../chorda_kit/render.rs"]
#[allow(dead_code)] // shared by the examples: each uses part of it
mod render;

use cantus_core::inst::chorda_source::{self, ChordaSource, RecordedPiano};
use cantus_core::{InstrumentKind, NativeSpec};
use render::Stereo;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// `$CANTUS_REFS` joined with `sub`: the sound references' home outside every round's folder
/// (`tools/sound-refs`; default `F:/projects/cantus-refs` on Windows, `~/cantus-refs` elsewhere).
fn refs_dir(sub: &str) -> PathBuf {
    let root = std::env::var_os("CANTUS_REFS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                PathBuf::from("F:/projects/cantus-refs")
            } else {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("cantus-refs")
            }
        });
    root.join(sub)
}

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

/// One recorded piano: its label, its pack and instrument, and the content root it is in.
struct Piano {
    label: &'static str,
    pack: &'static str,
    instrument: &'static str,
    root: PathBuf,
}

/// Install `pack` through Cantus's own pack code unless it is there.
fn install(root: &Path, pack: &str) -> Result<(), String> {
    let mut m = cantus_content::Manager::new(root, cantus_content::Manifest::bundled());
    if m.is_installed(pack) {
        return Ok(());
    }
    if !m.install(pack) {
        return Err(format!("{pack} could not start installing"));
    }
    eprintln!("installing {pack} into {} …", root.display());
    for f in m.wait_idle(std::time::Duration::from_secs(7200)) {
        if let Err(e) = f.result {
            return Err(format!("{pack}: {e}"));
        }
    }
    m.is_installed(pack)
        .then_some(())
        .ok_or_else(|| format!("{pack}: the install did not finish"))
}

/// The Chorda that plays `p`: a fresh Chorda switched to it, as the source list does.
fn chorda_of(p: &Piano) -> Result<NativeSpec, String> {
    let m = cantus_content::Manager::new(&p.root, cantus_content::Manifest::bundled());
    let inst = m
        .instruments()
        .into_iter()
        .find(|i| i.pack == p.pack && i.instrument.id == p.instrument)
        .ok_or_else(|| format!("{} is not installed", p.pack))?;
    let choice = cantus_ui::views::samples_content::sounds::SoundChoice(inst);
    let imp = cantus_ui::views::samples_content::sounds::import_of(&choice)?;
    let zones = cantus_ui::views::samples_content::sounds::spec_of(&imp);
    let base = NativeSpec::new(InstrumentKind::Chorda);
    let to = ChordaSource::Recorded(RecordedPiano {
        pack: p.pack.into(),
        instrument: p.instrument.into(),
    });
    Ok(chorda_source::with_source(&base, &to, Some(&zones)))
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let abx = args.iter().position(|a| a == "--abx").map(|i| {
        let d = args.get(i + 1).cloned().expect("--abx <dir>");
        args.drain(i..=i + 1);
        PathBuf::from(d)
    });
    let measuring = args.iter().any(|a| a == "--measure");
    args.retain(|a| a != "--measure");
    let [out, content, ..] = args.as_slice() else {
        eprintln!(
            "usage: chorda_pianos <out-dir> <content-root> [<salamander-root>] [--abx <dir>]"
        );
        std::process::exit(2);
    };
    let (out, content) = (PathBuf::from(out), PathBuf::from(content));
    if measuring {
        measure_mode(&out, &content);
        return;
    }
    let salamander_root = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| refs_dir("piano").join("content"));
    let pianos = [
        Piano {
            label: "Salamander",
            pack: "salamander-grand-piano",
            instrument: "grand-full",
            root: salamander_root.clone(),
        },
        Piano {
            label: "bitKlavier Stage Lid ORTF",
            pack: "bitklavier-grand-stage-lid-ortf",
            instrument: "grand",
            root: content.clone(),
        },
        Piano {
            label: "bitKlavier Piano Bar",
            pack: "bitklavier-grand-piano-bar",
            instrument: "grand",
            root: content.clone(),
        },
    ];
    for p in &pianos {
        if let Err(e) = install(&p.root, p.pack) {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    render::set_cache(&content.join(".stream-cache"));
    let specs: Vec<NativeSpec> = pianos
        .iter()
        .map(|p| chorda_of(p).unwrap_or_else(|e| panic!("{}: {e}", p.label)))
        .collect();
    std::fs::create_dir_all(&out).expect("the output folder");
    let mut levels = String::from(
        "phrase,source,lufs_before,peak_before_dbfs,gain_db,lufs_after,peak_after_dbfs\n",
    );
    let mut kit: Vec<(String, Vec<Stereo>)> = Vec::new();
    for ph in phrases::all().into_iter().filter(|p| p.listen) {
        let mut renders: Vec<Stereo> = specs
            .iter()
            .zip(&pianos)
            .map(|(s, p)| {
                render::render_spec(s.clone(), &ph)
                    .unwrap_or_else(|e| panic!("{} {}: {e}", p.label, ph.id))
            })
            .collect();
        let n = renders.iter().map(|s| s.len()).min().unwrap_or(0);
        for s in renders.iter_mut() {
            *s = s.slice(0, n);
            let f = (0.03 * render::SR as f64) as usize;
            for i in 0..f.min(n) {
                let g = i as f32 / f as f32;
                s.l[n - 1 - i] *= g;
                s.r[n - 1 - i] *= g;
            }
        }
        let before: Vec<(f64, f64)> = renders.iter().map(|s| (lufs(s), db(s.peak()))).collect();
        let target = before
            .iter()
            .map(|(l, p)| TARGET.min(l + (CEILING_DB - p)))
            .fold(TARGET, f64::min);
        for (i, (p, (s, (l, pk)))) in pianos
            .iter()
            .zip(renders.iter_mut().zip(&before))
            .enumerate()
        {
            let mut g = target - l;
            s.gain(cantus_dsp::math::powf(10.0f64, g / 20.0) as f32);
            let again = target - lufs(s);
            g += again;
            s.gain(cantus_dsp::math::powf(10.0f64, again / 20.0) as f32);
            let path = out.join(format!("{} - {} {}.flac", ph.id, i + 1, p.label));
            render::write_flac(&path, s).expect("the FLAC is written");
            let _ = writeln!(
                levels,
                "{},{},{l:.2},{pk:.2},{g:.2},{:.2},{:.2}",
                ph.id,
                p.label,
                lufs(s),
                db(s.peak())
            );
        }
        eprintln!(
            "{:<12} matched at {target:.1} LUFS ({:.1} s)",
            ph.id,
            n as f64 / render::SR as f64
        );
        kit.push((ph.id.to_string(), renders));
    }
    std::fs::write(out.join("levels.csv"), levels).expect("levels.csv");
    if let Some(dir) = abx {
        write_abx(&dir, &kit);
    }
    eprintln!("written into {}", out.display());
}

/// `chorda_abx`'s page with its three sources renamed: the Salamander, bitKlavier's
/// default image (Stage Lid ORTF, in model 2's place: ABX 1 is it against the Salamander)
/// and its Piano Bar (in model 1's: ABX 2 is the two images).
fn write_abx(dir: &Path, kit: &[(String, Vec<Stereo>)]) {
    const PAGE: &str = include_str!("../abx_kit/abx.html");
    std::fs::create_dir_all(dir.join("audio")).expect("the kit's folder");
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
        format!("{h:016x}.flac")
    };
    let (mut list, mut files) = (String::new(), String::new());
    for (id, renders) in kit {
        for (name, s) in ["salamander", "chorda2", "chorda1"].iter().zip(renders) {
            let file = opaque(id, name);
            render::write_flac(&dir.join("audio").join(&file), s).expect("the FLAC");
            if !files.is_empty() {
                files.push(',');
            }
            let _ = write!(files, "\"{id}|{name}\":\"{file}\"");
        }
        if !list.is_empty() {
            list.push(',');
        }
        let _ = write!(list, "\"{id}\"");
    }
    let page = PAGE
        .replace("__PHRASES__", &format!("[{list}]"))
        .replace("__FILES__", &format!("{{{files}}}"))
        .replace("Chorda listening test", "Recorded pianos: listening test")
        .replace(
            "Prueba de escucha de Chorda",
            "Pianos grabados: prueba de escucha",
        )
        .replace(
            "ABX 1: Chorda 2 / Salamander",
            "ABX 1: bitKlavier / Salamander",
        )
        .replace(
            "ABX 2: Chorda 1 / Chorda 2",
            "ABX 2: bitKlavier Piano Bar / Stage Lid ORTF",
        )
        .replace("\"Chorda model 1\"", "\"bitKlavier Piano Bar\"")
        .replace("\"Chorda modelo 1\"", "\"bitKlavier Piano Bar\"")
        .replace("\"Chorda model 2\"", "\"bitKlavier Stage Lid ORTF\"")
        .replace("\"Chorda modelo 2\"", "\"bitKlavier Stage Lid ORTF\"");
    std::fs::write(dir.join("index.html"), page).expect("index.html");
}

/// Every bitKlavier image and Accurate-Salamander installed (FLAC kept, each checked), their
/// installed sizes written to `<out>/sizes.json`, then the 1-, 2- and 4-image measurements
/// to `<out>/images.csv`, and a phrase from Accurate-Salamander and from four images at once.
fn measure_mode(out: &Path, content: &Path) {
    std::fs::create_dir_all(out).expect("the output folder");
    let m = cantus_content::Manager::new(content, cantus_content::Manifest::bundled());
    let packs: Vec<String> = m
        .manifest()
        .packs
        .iter()
        // The images the measure turns on (and Accurate-Salamander): never all nine at
        // once on a machine short of disk.
        .filter(|p| measure::PACKS.contains(&p.id.as_str()))
        .map(|p| p.id.clone())
        .collect();
    // `CHORDA_INSTALL=<pack>,…`: install those, say what each takes on disk, measure nothing
    // (the manifest's installed sizes).
    let only: Option<Vec<String>> = std::env::var("CHORDA_INSTALL")
        .ok()
        .map(|v| v.split(',').map(|x| x.trim().to_string()).collect());
    let packs = only.clone().unwrap_or(packs);
    drop(m);
    for p in &packs {
        if let Err(e) = install(content, p) {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
    let m = cantus_content::Manager::new(content, cantus_content::Manifest::bundled());
    let mut sizes = String::from("{");
    for r in m.installed() {
        if packs.contains(&r.id) {
            let slug = r.id.trim_start_matches("bitklavier-grand-");
            if sizes.len() > 1 {
                sizes.push(',');
            }
            let _ = write!(sizes, "\"{slug}\":{}", r.bytes);
            eprintln!(
                "{} installed: {:.2} GB in {} files",
                r.id,
                r.bytes as f64 / 1e9,
                r.files
            );
        }
    }
    sizes.push('}');
    if only.is_some() {
        std::fs::write(out.join("sizes-more.json"), sizes).expect("sizes-more.json");
        return;
    }
    std::fs::write(out.join("sizes.json"), sizes).expect("sizes.json");
    render::set_cache(&content.join(".stream-cache"));
    let ballad = phrases::all()
        .into_iter()
        .find(|p| p.id == "ballad")
        .expect("the ballad");
    let name = match std::env::var("CHORDA_MEASURE_N") {
        Ok(n) => format!("images-{}.csv", n.replace(',', "-")),
        Err(_) => "images.csv".into(),
    };
    std::fs::write(out.join(name), measure::run(content, &ballad)).expect("images.csv");
    eprintln!("measured into {}", out.display());
}
