//! `THIRD-PARTY-NOTICES.md`: every crate compiled into what Cantus ships, its licence and
//! its licence files, with the hand-written notices (the patched cpal and asio-sys, the
//! Steinberg ASIO SDK and the trademarks, fonts, Bible texts) in front, and the *Content*
//! section — every sound pack's credit — generated from the content manifest (ADR 0042).
//!
//! ```text
//! cargo run -p xtask -- notices           # write THIRD-PARTY-NOTICES.md
//! cargo run -p xtask -- notices --check   # fail if it is stale, a crate has no notice,
//!                                         # or a licence is not one Cantus may ship
//! ```
//!
//! The crates are the dependency closure (normal and build dependencies, never dev ones)
//! of the two things a release carries, the `cantus` program and the `cantus-plugins`
//! library, for the one target released (Windows x64), as `cargo metadata` resolves
//! them from `Cargo.lock`. The texts are the licence files inside each crate as published
//! on crates.io (read from cargo's registry cache, where `cargo metadata` puts them). The
//! hand-written part is `xtask/notices/head.md`; the standard texts a crate declares but
//! does not ship are in `xtask/notices/` (and the root's `LICENSE-APACHE`).
//!
//! A test (`notices_tests.rs`) runs the check, so a new crate in `Cargo.lock` without a
//! regenerated notice fails `cargo test`.

use cantus_content::Manifest;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

pub const FILE: &str = "THIRD-PARTY-NOTICES.md";
/// The one target a release is built for (ADR 0032: one Windows edition).
pub const TARGET: &str = "x86_64-pc-windows-msvc";
/// What a release ships: the program, the plug-in sandbox it starts beside itself
/// (round 29, ADR 0063), and the exported plug-ins.
pub const ROOTS: &[&str] = &["cantus", "cantus-sandbox", "cantus-plugins"];

/// One crate from crates.io in the closure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Crate {
    pub name: String,
    pub version: String,
    /// The `license` field as published (`MIT OR Apache-2.0`, `MIT/Apache-2.0`).
    pub license: String,
    pub authors: Vec<String>,
    pub repository: Option<String>,
    /// The crate's source folder (cargo's registry cache).
    pub dir: PathBuf,
    /// Reached only through build dependencies: used to build Cantus, not inside it.
    pub build_only: bool,
}

/// `cargo metadata` for the released target, from `Cargo.lock` as it is.
pub fn metadata(workspace: &Path) -> Result<Value, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(workspace)
        .args(["metadata", "--format-version", "1", "--locked"])
        .args(["--filter-platform", TARGET])
        .output()
        .map_err(|e| format!("cargo metadata: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("cargo metadata's JSON: {e}"))
}

/// The registry crates `ROOTS` need, sorted by name and version.
pub fn shipped(meta: &Value) -> Vec<Crate> {
    let packages: BTreeMap<&str, &Value> = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["id"].as_str()?, p)))
        .collect();
    let nodes: BTreeMap<&str, &Value> = meta["resolve"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| Some((n["id"].as_str()?, n)))
        .collect();
    let roots: Vec<&str> = packages
        .iter()
        .filter(|(_, p)| p["source"].is_null() && ROOTS.contains(&p["name"].as_str().unwrap_or("")))
        .map(|(id, _)| *id)
        .collect();
    // Reached through normal edges (inside the binaries), and through any edge.
    let reach = |kinds: &[Option<&str>]| -> BTreeSet<&str> {
        let mut seen = BTreeSet::new();
        let mut stack = roots.clone();
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            for d in nodes
                .get(id)
                .and_then(|n| n["deps"].as_array())
                .into_iter()
                .flatten()
            {
                let wanted = d["dep_kinds"].as_array().into_iter().flatten().any(|k| {
                    let kind = k["kind"].as_str();
                    kinds.contains(&kind)
                });
                if wanted {
                    if let Some(p) = d["pkg"].as_str() {
                        stack.push(p);
                    }
                }
            }
        }
        seen
    };
    let inside = reach(&[None]);
    let all = reach(&[None, Some("build")]);
    let mut out: Vec<Crate> = all
        .iter()
        .filter_map(|id| packages.get(id).map(|p| (id, p)))
        .filter(|(_, p)| !p["source"].is_null())
        .map(|(id, p)| Crate {
            name: p["name"].as_str().unwrap_or_default().into(),
            version: p["version"].as_str().unwrap_or_default().into(),
            license: p["license"].as_str().unwrap_or("").into(),
            authors: p["authors"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| a.as_str().map(Into::into))
                .collect(),
            repository: p["repository"].as_str().map(Into::into),
            dir: Path::new(p["manifest_path"].as_str().unwrap_or_default())
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            build_only: !inside.contains(*id),
        })
        .collect();
    out.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    out.dedup_by(|a, b| a.name == b.name && a.version == b.version);
    out
}

/// The licences Cantus (GPL-3.0-or-later) may ship code under. Fonts' licences (OFL,
/// UFL) cover data that is aggregated, not code linked in.
pub const ALLOWED: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "0BSD",
    "Unlicense",
    "BSL-1.0",
    "CC0-1.0",
    "MPL-2.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "CDLA-Permissive-2.0",
    "OFL-1.1",
    "Ubuntu-font-1.0",
    // LAME (MP3 export, ADR 0033): LGPL code linked into a GPL-3.0-or-later program whose
    // whole source is published — compatible (the integrator's ruling, round 22). The
    // crates declare the deprecated id "LGPL-3.0".
    "LGPL-3.0",
    "LGPL-3.0-only",
    "LGPL-3.0-or-later",
    "LGPL-2.1-or-later",
];

/// Whether an SPDX expression (with crates.io's old `/` for OR) lets Cantus ship the
/// crate: every term of an AND allowed, at least one side of an OR.
pub fn allowed(expr: &str) -> bool {
    let norm = expr
        .replace('/', " OR ")
        .replace('(', " ( ")
        .replace(')', " ) ");
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut pos = 0;
    let ok = or_expr(&tokens, &mut pos);
    ok == Some(true) && pos == tokens.len()
}

fn or_expr(t: &[&str], pos: &mut usize) -> Option<bool> {
    let mut v = and_expr(t, pos)?;
    while t.get(*pos) == Some(&"OR") {
        *pos += 1;
        v |= and_expr(t, pos)?;
    }
    Some(v)
}

fn and_expr(t: &[&str], pos: &mut usize) -> Option<bool> {
    let mut v = term(t, pos)?;
    while t.get(*pos) == Some(&"AND") {
        *pos += 1;
        v &= term(t, pos)?;
    }
    Some(v)
}

fn term(t: &[&str], pos: &mut usize) -> Option<bool> {
    let first = *t.get(*pos)?;
    *pos += 1;
    if first == "(" {
        let v = or_expr(t, pos)?;
        if t.get(*pos) != Some(&")") {
            return None;
        }
        *pos += 1;
        return Some(v);
    }
    let mut id = first.to_string();
    if t.get(*pos) == Some(&"WITH") {
        id = format!("{id} WITH {}", t.get(*pos + 1)?);
        *pos += 2;
    }
    Some(ALLOWED.contains(&id.as_str()))
}

/// What is wrong with `text` as the notices for `crates` and the content of `manifest`:
/// crates not named in it, licences not allowed, a CC BY pack whose credit line is
/// missing (ADR 0042 §3), and whether it differs from what would be written now.
pub fn problems(text: &str, crates: &[Crate], manifest: &Manifest, expected: &str) -> Vec<String> {
    let mut out = Vec::new();
    let flat = text.replace("\r\n", "\n");
    for p in &manifest.packs {
        if let Some(line) = p.credit_full() {
            if !flat.contains(&line) {
                out.push(format!(
                    "pack {}: its {} credit is missing: {line}",
                    p.id, p.licence
                ));
            }
        }
    }
    for c in crates {
        if !allowed(&c.license) {
            out.push(format!(
                "{} {}: licence `{}` is not one Cantus may ship",
                c.name, c.version, c.license
            ));
        }
        let row = format!("| {} | {} |", c.name, c.version);
        if !text.contains(&row) {
            out.push(format!("{} {} has no notice", c.name, c.version));
        }
    }
    if expected.contains(crate::notices_render::MISSING) {
        out.push("a standard licence text is missing from xtask/notices/".into());
    }
    if out.is_empty() && text.replace("\r\n", "\n") != expected {
        out.push(format!("{FILE} is stale"));
    }
    out
}

pub fn command(args: &[String]) {
    let ws = crate::workspace();
    let meta = metadata(&ws).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
    let crates = shipped(&meta);
    let manifest = Manifest::bundled();
    let expected = crate::notices_render::render(&ws, &crates, &manifest);
    let path = ws.join(FILE);
    if args.iter().any(|a| a == "--check") {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let found = problems(&text, &crates, &manifest, &expected);
        if found.is_empty() {
            eprintln!("{FILE}: {} crates, every one noticed", crates.len());
            return;
        }
        for p in &found {
            eprintln!("{p}");
        }
        eprintln!("run `cargo run -p xtask -- notices` and commit {FILE}");
        std::process::exit(1);
    }
    std::fs::write(&path, &expected).expect("write the notices");
    eprintln!(
        "{}: {} crates ({} build only), {} KB",
        path.display(),
        crates.len(),
        crates.iter().filter(|c| c.build_only).count(),
        expected.len() / 1024
    );
}
