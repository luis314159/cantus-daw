//! Chorda's listening kit (round 37, ADR 0098; kit 5, round 40 — made to be given to other
//! listeners): a blind ABX test and a blind preference test of three pianos — Chorda at a
//! candidate voicing ("Chorda 3"), the Salamander Grand (a recorded Yamaha C5, CC BY 3.0,
//! Alexander Holm) and Chorda 2 (model 2 at beta.14's voicing, through the lab; since
//! `DSP_VERSION` 7 a new track plays candidate 16).
//!
//! ```text
//! cargo run --release -p cantus-ui --example chorda_abx -- <dir>
//!     --candidate <spec file> [--partials <csv>] [--levels <csv>] [--name <label>]
//!     [--downloads <dir>] [--key <dir>] [--wav]
//! ```
//!
//! `--candidate` is the voicing lab's `key=value,…` spec (its first line); `--partials` and
//! `--levels` its per-key tables (`CHORDA_PARTIALS` / `CHORDA_LEVELS`, as the lab reads them;
//! the environment's are used when the flags are absent). The Salamander is installed through
//! Cantus's own pack code into `<downloads>/content` (default
//! `$CANTUS_REFS/piano`, tools/sound-refs) and played by the engine, as is Chorda 2; Chorda 3
//! through the lab (model 2's DSP at the candidate's voicing). Every phrase's three renders are
//! trimmed to one length, given one 30 ms fade and brought to one loudness (ITU-R BS.1770
//! integrated, −20 LUFS, lowered together if a peak would pass −1 dBFS).
//!
//! Written into `<dir>` — what a listener receives, nothing in it says which piano is which:
//!
//! - `audio/<name>.flac` (24-bit, 48 kHz; with `--wav`, 24-bit WAV — the kit to publish as a
//!   claude.ai page, which serves .wav but not .flac): every name 64 random bits
//!   drawn for this kit (`names_look_random` holds that no two share structure), two of them
//!   the same clip under two names (the preference test's catch questions),
//! - `index.html` — the listening kits' one page (`abx_kit/abx.html`, Chorda's subject
//!   `chorda_kit/abx_subject.json`), no external script, no network unless published: a short form
//!   about the listener, the ABX (16 trials), the preferences (every phrase, both pairs, the
//!   two catch questions), an optional second ABX, the reveal and the credits only at the
//!   end; answers kept in the browser, offered as a JSON file, and saved for the owner when
//!   the page is a published claude.ai Artifact (`db`, `user`, `downloads`),
//! - `LEEME.md` / `README.md` — how to take it.
//!
//! And into `--key` (default `<dir>-key`, beside the kit, never sent): `key.json` (which piano
//! each token and file is, the candidate's files) and `levels.csv` (each file's loudness before
//! and after the match).

#[path = "../abx_kit/kit.rs"]
#[allow(dead_code)] // shared by the listening kits: each uses part of it
mod kit;
#[path = "../chorda_kit/lab.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod lab;
#[path = "../abx_kit/names.rs"]
mod names;
#[path = "../chorda_kit/phrases.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod phrases;
#[path = "../chorda_kit/render.rs"]
#[allow(dead_code)] // shared by both examples: each uses part of it
mod render;

use kit::js;
use render::{Source, Stereo};
use std::fmt::Write as _;
use std::path::PathBuf;

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

const SUBJECT: &str = include_str!("../chorda_kit/abx_subject.json");
const README_EN: &str = include_str!("../chorda_kit/abx_readme.en.md");
const README_ES: &str = include_str!("../chorda_kit/abx_readme.es.md");

/// The loudness every file is brought to (LUFS), unless a peak would pass the ceiling.
const TARGET: f64 = -20.0;
const CEILING_DB: f64 = -1.0;

fn lufs(s: &Stereo) -> f64 {
    kit::lufs(&s.l, &s.r, render::SR)
}

fn db(x: f32) -> f64 {
    kit::db(x)
}

struct Args {
    dir: PathBuf,
    downloads: PathBuf,
    key: PathBuf,
    spec_file: String,
    spec: String,
    partials: Option<String>,
    levels: Option<String>,
    name: String,
    flac: bool,
}

fn usage() -> ! {
    eprintln!(
        "usage: chorda_abx <dir> --candidate <spec file> [--partials <csv>] [--levels <csv>] \
         [--name <label>] [--downloads <dir>] [--key <dir>] [--wav]"
    );
    std::process::exit(2);
}

fn args() -> Args {
    let mut it = std::env::args().skip(1);
    let (mut dir, mut downloads, mut key, mut spec_file) = (None, None, None, None);
    let (mut partials, mut levels, mut name, mut flac) = (None, None, None, true);
    while let Some(a) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| usage());
        match a.as_str() {
            "--candidate" => spec_file = Some(val()),
            "--partials" => partials = Some(val()),
            "--levels" => levels = Some(val()),
            "--name" => name = Some(val()),
            "--downloads" => downloads = Some(PathBuf::from(val())),
            "--key" => key = Some(PathBuf::from(val())),
            "--wav" => flac = false,
            "--flac" => flac = true,
            _ if a.starts_with("--") => usage(),
            _ if dir.is_none() => dir = Some(PathBuf::from(a)),
            // Kit 4's form: the downloads folder as the second word.
            _ if downloads.is_none() => downloads = Some(PathBuf::from(a)),
            _ => usage(),
        }
    }
    let dir = dir.unwrap_or_else(|| usage());
    let spec_file = spec_file.unwrap_or_else(|| usage());
    let text = std::fs::read_to_string(&spec_file)
        .unwrap_or_else(|e| panic!("the candidate's spec {spec_file}: {e}"));
    let spec = text.lines().next().unwrap_or("").trim().to_string();
    let partials = partials.or_else(|| std::env::var("CHORDA_PARTIALS").ok());
    let levels = levels.or_else(|| std::env::var("CHORDA_LEVELS").ok());
    for f in partials.iter().chain(levels.iter()) {
        assert!(std::path::Path::new(f).is_file(), "no table {f}");
    }
    let name = name.unwrap_or_else(|| {
        std::path::Path::new(&spec_file)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let key = key.unwrap_or_else(|| {
        let mut k = dir.clone().into_os_string();
        k.push("-key");
        PathBuf::from(k)
    });
    Args {
        dir,
        downloads: downloads.unwrap_or_else(|| refs_dir("piano")),
        key,
        spec_file,
        spec,
        partials,
        levels,
        name,
        flac,
    }
}

/// The three pianos: an id (the key's, the owner's summary's), what plays it.
const PIANOS: [&str; 3] = ["chorda3", "salamander", "chorda2"];

fn sep(s: &mut String) {
    if !s.is_empty() {
        s.push(',');
    }
}

fn main() {
    let a = args();
    // The lab reads its tables from the environment; set before anything renders.
    if let Some(p) = &a.partials {
        std::env::set_var("CHORDA_PARTIALS", p);
    }
    if let Some(l) = &a.levels {
        std::env::set_var("CHORDA_LEVELS", l);
    }
    let ext = if a.flac { "flac" } else { "wav" };
    let root = render::content_root(&a.downloads);
    if let Err(e) = render::install(&root) {
        eprintln!("the Salamander Grand could not be installed: {e}");
        std::process::exit(1);
    }
    render::set_cache(&a.downloads.join("stream-cache"));
    let _ = std::fs::remove_dir_all(a.dir.join("audio"));
    std::fs::create_dir_all(a.dir.join("audio")).expect("the kit's folder");
    std::fs::create_dir_all(&a.key).expect("the key's folder");
    let mut rng = names::Rng::new();
    let kit = format!("abx5-{}", &rng.name()[..10]);
    // Each piano's token in the page, and two more for the catch questions' second names.
    let tok: Vec<String> = (0..5).map(|_| rng.token()).collect();
    let listen: Vec<phrases::Phrase> = phrases::all().into_iter().filter(|p| p.listen).collect();
    // The catch questions: two phrases drawn, one the recorded piano against itself, one the
    // candidate against itself (the same file under two names). (phrase, piano, token)
    let c1 = rng.below(listen.len());
    let c2 = (c1 + 1 + rng.below(listen.len() - 1)) % listen.len();
    let catch = [(listen[c1].id, 1usize, 3usize), (listen[c2].id, 0, 4)];
    // Every file's name drawn first, redrawn until none shares structure with another.
    let count = listen.len() * PIANOS.len() + catch.len();
    let drawn = loop {
        let v: Vec<String> = (0..count).map(|_| rng.name()).collect();
        if names::names_look_random(&v).is_ok() {
            break v;
        }
    };
    let mut drawn = drawn.into_iter();
    let sources: [Source; 1] = [Source::Salamander];
    let mut levels = String::from(
        "phrase,piano,file,lufs_before,peak_before_dbfs,gain_db,lufs_after,peak_after_dbfs\n",
    );
    let (mut list, mut files, mut key_files) = (String::new(), String::new(), String::new());
    let mut bytes = 0u64;
    for ph in &listen {
        let mut renders: Vec<Stereo> = vec![lab::render(&a.spec, ph)];
        for src in &sources {
            renders.push(
                render::render_source(src, &root, ph)
                    .unwrap_or_else(|e| panic!("{} {}: {e}", src.label(), ph.id)),
            );
        }
        // Chorda 2: beta.14's voicing (the lab's empty spec).
        renders.push(lab::render("", ph));
        let n = renders.iter().map(|s| s.len()).min().unwrap_or(0);
        // One length, a 30 ms fade at the end, one loudness for the three (`abx_kit`).
        let m = {
            let mut g: Vec<_> = renders.iter_mut().map(|s| (&mut s.l, &mut s.r)).collect();
            kit::match_group(&mut g, render::SR, 0.03, TARGET, CEILING_DB)
        };
        let (target, before, gains) = (m.target, m.before, m.gains);
        for (i, (s, ((l, p), g))) in renders
            .iter_mut()
            .zip(before.iter().zip(&gains))
            .enumerate()
        {
            // This piano's file, and the catch question's second name for it.
            let mut toks = vec![i];
            toks.extend(
                catch
                    .iter()
                    .filter(|(id, piano, _)| *id == ph.id && *piano == i)
                    .map(|c| c.2),
            );
            let mut first: Option<PathBuf> = None;
            for t in toks {
                let file = format!("{}.{ext}", drawn.next().expect("a name"));
                let path = a.dir.join("audio").join(&file);
                match &first {
                    None => {
                        if a.flac {
                            render::write_flac(&path, s).expect("the FLAC is written");
                        } else {
                            render::write_wav(&path, s, false).expect("the WAV is written");
                        }
                        first = Some(path.clone());
                    }
                    Some(f) => {
                        std::fs::copy(f, &path).expect("the catch question's copy");
                    }
                }
                bytes += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                sep(&mut files);
                let _ = write!(
                    files,
                    "{}:{}",
                    js(&format!("{}|{}", ph.id, tok[t])),
                    js(&file)
                );
                sep(&mut key_files);
                let _ = write!(
                    key_files,
                    "{}:{{\"phrase\":{},\"piano\":{},\"catch\":{}}}",
                    js(&file),
                    js(ph.id),
                    js(PIANOS[i]),
                    t >= 3
                );
                let _ = writeln!(
                    levels,
                    "{},{},{file},{l:.2},{p:.2},{g:.2},{:.2},{:.2}",
                    ph.id,
                    PIANOS[i],
                    lufs(s),
                    db(s.peak())
                );
            }
        }
        eprintln!(
            "{:<12} matched at {target:.1} LUFS ({:.1} s)",
            ph.id,
            n as f64 / render::SR as f64
        );
        sep(&mut list);
        list.push_str(&js(ph.id));
    }
    assert!(drawn.next().is_none(), "every drawn name used");
    // Which piano each token is: veiled in the page, plain in the key.
    let mut table = String::new();
    let of = [PIANOS[0], PIANOS[1], PIANOS[2], PIANOS[1], PIANOS[0]];
    for (t, id) in tok.iter().zip(of) {
        sep(&mut table);
        let _ = write!(table, "{}:{}", js(t), js(id));
    }
    let table = format!("{{{table}}}");
    let seed = (rng.next() >> 32) as u32 | 1;
    let pair = |x: usize, y: usize| format!("[{},{}]", js(&tok[x]), js(&tok[y]));
    let catch_js = |c: &(&str, usize, usize)| {
        format!(
            "{{phrase:{},one:{},two:{}}}",
            js(c.0),
            js(&tok[c.1]),
            js(&tok[c.2])
        )
    };
    let tests = format!(
        "{{abx:{},pref:[{},{}],extra:{},catch:[{},{}]}}",
        pair(0, 1),
        pair(0, 1),
        pair(0, 2),
        pair(2, 0),
        catch_js(&catch[0]),
        catch_js(&catch[1]),
    );
    let page = kit::fill_page([
        &js(&kit),
        &format!("[{list}]"),
        &format!("{{{files}}}"),
        &tests,
        &seed.to_string(),
        &js(&names::veil(&table, seed)),
        &kit::subject_line(SUBJECT),
    ]);
    std::fs::write(a.dir.join("index.html"), &page).expect("index.html");
    std::fs::write(a.dir.join("README.md"), README_EN).expect("README.md");
    std::fs::write(a.dir.join("LEEME.md"), README_ES).expect("LEEME.md");
    bytes += (page.len() + README_EN.len() + README_ES.len()) as u64;
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let opt = |o: &Option<String>| o.as_deref().map(js).unwrap_or_else(|| "null".into());
    let key = format!(
        "{{\n \"kit\": {},\n \"created\": {created},\n \"candidate\": {{\"name\": {}, \"spec_file\": {}, \"spec\": {}, \"partials\": {}, \"levels\": {}}},\n \"tokens\": {table},\n \"catch\": [{}, {}],\n \"files\": {{{key_files}}},\n \"bytes\": {bytes}\n}}\n",
        js(&kit),
        js(&a.name),
        js(&a.spec_file),
        js(&a.spec),
        opt(&a.partials),
        opt(&a.levels),
        js(catch[0].0),
        js(catch[1].0),
    );
    std::fs::write(a.key.join("key.json"), key).expect("key.json");
    std::fs::write(a.key.join("levels.csv"), levels).expect("levels.csv");
    eprintln!(
        "kit {kit} ({}) in {} — {:.1} MB; the key in {}",
        a.name,
        a.dir.display(),
        bytes as f64 / 1e6,
        a.key.display()
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_subject_is_a_subject() {
        let s = super::kit::subject_line(super::SUBJECT);
        assert!(s.contains("\"voice_salamander\""));
    }
}
