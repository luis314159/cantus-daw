//! `cargo run -p xtask -- null-hashes`: the x86-64-v2 and v3 programs render the same bits
//! (ADR 0031 §5.3, round 22).
//!
//! ```text
//! null-hashes                     v2 and v3 (and v4 where this machine runs it)
//! null-hashes --levels v2,v3,v4   these levels (one this machine cannot run is skipped)
//! null-hashes --baseline          also the plain x86-64 build the tests' goldens come from
//!                                 (on an Apple silicon or other ARM machine, the only build:
//!                                 `aarch64.txt`, which CI compares with the other systems')
//! null-hashes --no-build          compare what is built
//! null-hashes --whole             the demo songs whole (ten minutes of audio), not 8 bars
//! ```
//!
//! Builds `cantus-bench` once per level (`levels.rs`: its own target folder, the level's
//! `target-cpu`), runs `cantus-bench hashes` with each into `target/null-hashes/`, and
//! requires every list to equal the first, line for line: the corpus (`hashes.rs` in
//! `cantus-bench`) is every `cantus_dsp::math` function, every native effect at default and
//! heavy settings, every instrument, every demo song and every project fixture. Exit 1
//! naming each item that differs.

use crate::levels;
use cantus_launch::Level;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One build to compare: a level, or the plain baseline (`None`) — named after the
/// machine's architecture where that is not x86-64 (`aarch64` on Apple silicon).
fn label(level: Option<Level>) -> &'static str {
    let baseline = if cfg!(target_arch = "x86_64") {
        "x86-64"
    } else {
        std::env::consts::ARCH
    };
    level.map_or(baseline, Level::name)
}

/// The items of a hashes file (`<sha>  <samples>  <name>` per line; `#` lines are the
/// header, which names the build and is not compared).
pub fn items(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.splitn(3, "  ");
            let sha = parts.next()?.trim();
            let samples = parts.next()?.trim();
            let name = parts.next()?.trim();
            Some((name.to_owned(), format!("{sha} {samples}")))
        })
        .collect()
}

/// The names whose hash differs between `a` and `b`, or that only one of them has.
pub fn differences(a: &str, b: &str) -> Vec<String> {
    let (a, b) = (items(a), items(b));
    let mut out = Vec::new();
    for (name, h) in &a {
        match b.iter().find(|(n, _)| n == name) {
            Some((_, h2)) if h2 == h => {}
            Some(_) => out.push(name.clone()),
            None => out.push(format!("{name} (only in the first)")),
        }
    }
    for (name, _) in &b {
        if !a.iter().any(|(n, _)| n == name) {
            out.push(format!("{name} (only in the second)"));
        }
    }
    out
}

fn bench_exe(level: Option<Level>, triple: &str) -> PathBuf {
    match level {
        Some(l) => levels::binary(l, triple, "cantus-bench"),
        None => crate::target_dir()
            .join("release")
            .join(format!("cantus-bench{}", std::env::consts::EXE_SUFFIX)),
    }
}

fn build(level: Option<Level>, triple: &str) {
    match level {
        Some(l) => crate::run(levels::cargo_build(l, triple).args(["-p", "cantus-bench"])),
        None => {
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            crate::run(Command::new(cargo).current_dir(crate::workspace()).args([
                "build",
                "--release",
                "-p",
                "cantus-bench",
            ]))
        }
    }
}

fn run_hashes(exe: &Path, out: &Path, whole: bool) -> String {
    let mut cmd = Command::new(exe);
    cmd.current_dir(crate::workspace()).arg("hashes");
    if whole {
        cmd.arg("--whole");
    }
    crate::run(cmd.arg("--out").arg(out));
    std::fs::read_to_string(out).unwrap_or_else(|e| panic!("read {}: {e}", out.display()))
}

pub fn null_hashes(args: &[String]) {
    let mut wanted: Option<Vec<Level>> = None;
    let mut baseline = false;
    let mut build_them = true;
    let mut whole = false;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--levels" => {
                let list = it.next().map(String::as_str).unwrap_or("");
                wanted = Some(levels::parse_list(list).unwrap_or_else(|e| {
                    eprintln!("xtask null-hashes: {e}");
                    std::process::exit(2)
                }));
            }
            "--baseline" => baseline = true,
            "--no-build" => build_them = false,
            "--whole" => whole = true,
            other => {
                eprintln!("xtask null-hashes: unknown argument {other:?}");
                std::process::exit(2);
            }
        }
    }
    // By default v2 and v3, and v4 when this machine has AVX-512.
    let wanted = wanted.unwrap_or_else(|| {
        let mut v = vec![Level::V2, Level::V3];
        if levels::host_runs(Level::V4) {
            v.push(Level::V4);
        }
        v
    });
    let mut builds: Vec<Option<Level>> = Vec::new();
    if baseline {
        builds.push(None);
    }
    for l in wanted {
        if levels::host_runs(l) {
            builds.push(Some(l));
        } else {
            eprintln!(
                "xtask null-hashes: skipping {l}: this machine lacks {}",
                l.missing(&cantus_launch::detected).join(", ")
            );
        }
    }
    // An x86-64 machine compares its levels; another architecture has its baseline only,
    // whose list CI compares with the other systems' (null-hashes.yml, ADR 0066).
    let alone_ok = !cfg!(target_arch = "x86_64") && baseline && builds.len() == 1;
    if builds.len() < 2 && !alone_ok {
        eprintln!("xtask null-hashes: fewer than two builds this machine can run");
        std::process::exit(2);
    }
    let triple = levels::host_triple();
    let dir = crate::target_dir().join("null-hashes");
    std::fs::create_dir_all(&dir).expect("make target/null-hashes");
    let mut lists: Vec<(Option<Level>, String)> = Vec::new();
    for b in &builds {
        if build_them {
            build(*b, &triple);
        }
        let exe = bench_exe(*b, &triple);
        if !exe.is_file() {
            eprintln!("{} is not built", exe.display());
            std::process::exit(1);
        }
        let out = dir.join(format!("{}.txt", label(*b)));
        lists.push((*b, run_hashes(&exe, &out, whole)));
    }
    let (first, reference) = &lists[0];
    let mut failed = false;
    for (b, text) in &lists[1..] {
        let diff = differences(reference, text);
        if diff.is_empty() {
            eprintln!(
                "{} = {}: {} items, every one the same bits",
                label(*first),
                label(*b),
                items(text).len()
            );
        } else {
            failed = true;
            eprintln!(
                "{} ≠ {}: {} of {} items differ:",
                label(*first),
                label(*b),
                diff.len(),
                items(reference).len()
            );
            for d in diff {
                eprintln!("  {d}");
            }
        }
    }
    eprintln!("hash lists in {}", dir.display());
    if failed {
        std::process::exit(1);
    }
}
