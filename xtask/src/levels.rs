//! Building one program per x86-64 level (ADR 0031 §5.3, round 22).
//!
//! Each level builds in a target folder of its own, `target/cpu-x86-64-v3/`, so the levels
//! never invalidate each other, with `RUSTFLAGS=-C target-cpu=<level>` and an explicit
//! `--target <host triple>`: with `--target`, cargo gives `RUSTFLAGS` to the program only,
//! not to build scripts and proc-macros, which run on the build machine — so an x86-64-v4
//! program can be built on a machine without AVX-512. The launcher (`crates/cantus-launch`)
//! is built in the ordinary target, with no `target-cpu`, so it runs on any processor.

use cantus_launch::Level;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The levels a release carries: v3 and its v2 fallback; `--v4` adds AVX-512.
pub fn release_levels(v4: bool) -> Vec<Level> {
    let mut v = vec![Level::V2, Level::V3];
    if v4 {
        v.push(Level::V4);
    }
    v
}

/// `v2,v3` → the levels, lowest first; an unknown name is an error naming it.
pub fn parse_list(s: &str) -> Result<Vec<Level>, String> {
    let mut v = Vec::new();
    for part in s.split(',').filter(|p| !p.trim().is_empty()) {
        let l = Level::parse(part).ok_or_else(|| format!("{part:?} is not a level (v2, v3, v4)"))?;
        if !v.contains(&l) {
            v.push(l);
        }
    }
    v.sort();
    Ok(v)
}

/// The host's target triple (`rustc -vV`), e.g. `x86_64-pc-windows-msvc`.
pub fn host_triple() -> String {
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let out = Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("run rustc -vV");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .map(str::trim)
        .map(str::to_owned)
        .expect("rustc -vV names the host")
}

/// `target/cpu-x86-64-v3`.
pub fn target(level: Level) -> PathBuf {
    crate::target_dir().join(format!("cpu-{}", level.name()))
}

/// Where a level's release binaries land: `target/cpu-x86-64-v3/<host>/release`.
pub fn release_dir(level: Level, triple: &str) -> PathBuf {
    target(level).join(triple).join("release")
}

/// `name` (without `.exe`) as built for `level`.
pub fn binary(level: Level, triple: &str, name: &str) -> PathBuf {
    release_dir(level, triple).join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

/// `RUSTFLAGS` for a level: what the environment already has, then the level's.
pub fn rustflags(level: Level) -> String {
    let flags = std::env::var("RUSTFLAGS").unwrap_or_default();
    format!("{flags} {}", level.rustflags()).trim().to_owned()
}

/// `cargo build --release --target <host>` for `level`, in its own target folder; the
/// caller adds the packages.
pub fn cargo_build(level: Level, triple: &str) -> Command {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(crate::workspace())
        .args(["build", "--release", "--target", triple])
        .env("CARGO_TARGET_DIR", target(level))
        .env("RUSTFLAGS", rustflags(level));
    cmd
}

/// Whether this machine can run `level`'s programs.
pub fn host_runs(level: Level) -> bool {
    level.missing(&cantus_launch::detected).is_empty()
}

/// What `program --version` says must name `level` (the app prints `cantus 0.1.0
/// (x86-64-v3)`): proof that the flags reached the build.
pub fn says_level(version_line: &str, level: Level) -> bool {
    version_line.contains(&format!("({})", level.name()))
}

/// Run `exe --version` and stop unless it names `level`.
pub fn check_level(exe: &Path, level: Level) {
    let out = Command::new(exe)
        .arg("--version")
        .output()
        .unwrap_or_else(|e| panic!("run {}: {e}", exe.display()));
    let text = String::from_utf8_lossy(&out.stdout);
    eprintln!("{}: {}", exe.display(), text.trim());
    if !says_level(&text, level) {
        eprintln!(
            "{} does not say it was built for {level}: refusing to package it",
            exe.display()
        );
        std::process::exit(1);
    }
}
