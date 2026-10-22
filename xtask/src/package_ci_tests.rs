//! The release's stable download name kept equal in the packager, the release workflow
//! and the website; and CI's path filter kept honest (ADR 0048): a commit that changes
//! only documents runs nothing, but every file a test reads still triggers the tests.

use cantus_update::release;
use std::path::{Component, Path, PathBuf};

fn read(rel: &str) -> String {
    std::fs::read_to_string(crate::workspace().join(rel))
        .unwrap_or_else(|e| panic!("read {rel}: {e}"))
        .replace("\r\n", "\n")
}

/// The website's download button: `releases/latest/download/<stable name>` of the
/// repository the updater asks, so the newest release's installer comes straight down.
pub fn site_download_url() -> String {
    format!(
        "https://github.com/{}/releases/latest/download/{}",
        cantus_update::updater::REPO,
        release::STABLE_INSTALLER_NAME
    )
}

#[test]
fn the_stable_installer_name_is_the_same_in_the_packager_the_workflow_and_the_site() {
    let stable = release::STABLE_INSTALLER_NAME;
    // The packager writes it (package_tests holds the file list); the workflow checks the
    // file is there before publishing, so a renamed constant fails the release, not the site.
    let wf = read(".github/workflows/release.yml");
    assert!(
        wf.contains(&format!("dist/{stable}")),
        "release.yml does not check dist/{stable}"
    );
    // The site's button points at it, through the same repository the updater asks.
    let top = read("web/src/top.rs");
    let url = site_download_url();
    assert!(
        top.contains(&format!("\"{url}\"")),
        "web/src/top.rs does not link {url}"
    );
    // The name is a plain file name: no version in it, nothing GitHub would rewrite.
    assert!(stable
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.'));
    assert!(
        !stable.contains(crate::package::VERSION),
        "{stable} names a version"
    );
}

/// The stable names of the three systems' files (ADR 0067): the release workflow checks
/// each is there before publishing, and the website links each through
/// `releases/latest/download/` of the repository the updater asks.
#[test]
fn every_system_s_stable_name_is_in_the_workflow_and_on_the_site() {
    let wf = read(".github/workflows/release.yml");
    let top = read("web/src/top.rs");
    for t in release::Target::ALL {
        let stable = t.stable_name();
        assert!(
            wf.contains(&format!("dist/{stable}")),
            "release.yml does not check dist/{stable}"
        );
        let url = format!(
            "https://github.com/{}/releases/latest/download/{stable}",
            cantus_update::updater::REPO
        );
        assert!(
            top.contains(&format!("\"{url}\"")),
            "web/src/top.rs does not link {url}"
        );
        // The versioned name the updater asks for is the one the workflow checks.
        let versioned = t.file_name("$v");
        assert!(
            wf.contains(&format!("dist/{versioned}")),
            "release.yml does not check dist/{versioned}"
        );
    }
    // The packaging scripts, called by the round-30 contract's interface.
    assert!(wf.contains("bash packaging/linux/build-appimage.sh \"$version\" dist"));
    assert!(wf.contains("bash packaging/macos/build-dmg.sh \"$version\" dist"));
}

/// The `apt-get install -y` packages on the first line of `yaml` that installs some.
fn apt_packages(yaml: &str) -> Vec<String> {
    let line = yaml
        .lines()
        .find(|l| l.contains("apt-get install -y"))
        .expect("an apt-get install line");
    line.split("apt-get install -y")
        .nth(1)
        .unwrap()
        .split_whitespace()
        .map(str::to_owned)
        .collect()
}

/// The release's Linux job builds on the system packages CI tests with (ADR 0067): a
/// package CI's Linux job gains is a package the release's job needs too.
#[test]
fn the_release_s_linux_job_installs_every_package_ci_does() {
    let ci = apt_packages(&read(".github/workflows/ci.yml"));
    let rel = apt_packages(&read(".github/workflows/release.yml"));
    for p in &ci {
        assert!(
            rel.contains(p),
            "release.yml's Linux job does not install {p}"
        );
    }
}

/// GitHub's glob for `paths`: `*` is any run of characters but `/`, `**` any run at all,
/// `?` one character but `/`.
fn glob(pat: &[u8], path: &[u8]) -> bool {
    match pat {
        [] => path.is_empty(),
        [b'*', b'*', rest @ ..] => (0..=path.len()).any(|i| glob(rest, &path[i..])),
        [b'*', rest @ ..] => {
            let stop = path.iter().position(|&c| c == b'/').unwrap_or(path.len());
            (0..=stop).any(|i| glob(rest, &path[i..]))
        }
        [b'?', rest @ ..] => matches!(path, [c, tail @ ..] if *c != b'/' && glob(rest, tail)),
        [p, rest @ ..] => matches!(path, [c, tail @ ..] if c == p && glob(rest, tail)),
    }
}

/// Whether a commit changing only `path` runs a workflow with these `paths` patterns:
/// patterns are read in order, a later match wins, `!` excludes.
fn triggers(patterns: &[String], path: &str) -> bool {
    let mut on = false;
    for p in patterns {
        match p.strip_prefix('!') {
            Some(neg) if glob(neg.as_bytes(), path.as_bytes()) => on = false,
            None if glob(p.as_bytes(), path.as_bytes()) => on = true,
            _ => {}
        }
    }
    on
}

/// The `paths:` list under the trigger `event` (`push`, `pull_request`) of a workflow.
fn paths_of(yaml: &str, event: &str) -> Vec<String> {
    let mut lines = yaml
        .lines()
        .skip_while(|l| l.trim_end() != format!("  {event}:"));
    lines
        .next()
        .unwrap_or_else(|| panic!("no `{event}:` trigger"));
    let mut lines = lines.skip_while(|l| l.trim() != "paths:");
    lines
        .next()
        .unwrap_or_else(|| panic!("no paths under {event}"));
    lines
        .map_while(|l| l.trim().strip_prefix("- "))
        .map(|p| p.trim_matches('"').to_owned())
        .collect()
}

#[test]
fn the_glob_reads_patterns_as_github_does() {
    assert!(glob(b"**", b"a/b/c.rs"));
    assert!(glob(b"**.md", b"README.md"));
    assert!(glob(b"**.md", b"docs/round26/release.md"));
    assert!(glob(b"docs/**", b"docs/a/b.png"));
    assert!(!glob(b"docs/**", b"crates/docs.rs"));
    assert!(glob(b"web/src/top.rs", b"web/src/top.rs"));
    assert!(!glob(b"*.md", b"docs/a.md"));
    assert!(glob(b"*.md", b"a.md"));
}

#[test]
fn ci_runs_for_code_and_for_what_tests_read_but_not_for_documents_alone() {
    let yaml = read(".github/workflows/ci.yml");
    let push = paths_of(&yaml, "push");
    assert_eq!(
        push,
        paths_of(&yaml, "pull_request"),
        "the two lists differ"
    );
    for code in [
        "crates/cantus-ui/src/app.rs",
        "crates/cantus-engine/src/lib.rs",
        "xtask/src/main.rs",
        "third_party/cpal/src/lib.rs",
        "Cargo.lock",
        "Cargo.toml",
        "rust-toolchain.toml",
        "clippy.toml",
        "perf/baselines/gha-windows-latest.json",
        "LICENSE-GPL",
        "LICENSE-APACHE",
        // Documents a test reads.
        "docs/manual/en/README.md",
        "docs/manual/es/README.md",
        "docs/manual/img/mixer.png",
        "THIRD-PARTY-NOTICES.md",
        "PRIVACY.md",
        "web/src/top.rs",
        ".github/workflows/ci.yml",
        ".github/workflows/release.yml",
        "xtask/installer/cantus.nsi",
        "crates/cantus-ui/locales/areas/update.en.txt",
        "crates/cantus-update/tests/fixtures/SHA256SUMS.txt",
        // Markdown read by tests or compiled in, outside the document folders.
        "xtask/notices/head.md",
        "crates/cantus-ui/locales/verses/README.md",
        "third_party/README.md",
    ] {
        assert!(triggers(&push, code), "{code} must run CI");
    }
    for doc in [
        "README.md",
        "CLAUDE.md",
        "CLA.md",
        "docs/STATUS.md",
        "docs/round26/release.md",
        "docs/images/hero.png",
        "docs/perf/proto/forkjoin/src/main.rs",
        "research/mixer-routing/notes.md",
        "web/src/body.rs",
        "web/index.html",
        "tools/wsl/check.sh",
        ".github/workflows/site.yml",
    ] {
        assert!(!triggers(&push, doc), "{doc} alone must not run CI");
    }
}

/// `a/b/../c` → `a/c`, with `/` separators.
fn normal(p: &Path) -> String {
    let mut out: Vec<String> = Vec::new();
    for c in p.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(s) => out.push(s.to_string_lossy().into_owned()),
            _ => {}
        }
    }
    out.join("/")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name();
        if p.is_dir() {
            if name != "target" && name != ".git" {
                rust_files(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

/// The string literal right after `call` in `text` at `at` (whitespace and newlines allowed
/// between, as rustfmt wraps `include_str!(\n    "…",\n)`), if it is one.
fn literal_after(text: &str, at: usize) -> Option<&str> {
    let rest = text[at..].trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(&rest[..rest.find('"')?])
}

/// The first file under `dir` (depth-first), to stand for "a change in this folder".
fn a_file_in(dir: &Path) -> Option<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .collect();
    entries.sort();
    entries.iter().find(|p| p.is_file()).cloned().or_else(|| {
        entries
            .iter()
            .filter(|p| p.is_dir())
            .find_map(|d| a_file_in(d))
    })
}

#[test]
fn every_file_a_test_or_the_build_reads_runs_ci_when_it_changes() {
    let ws = crate::workspace();
    let push = paths_of(&read(".github/workflows/ci.yml"), "push");
    let mut files = Vec::new();
    for d in ["crates", "xtask", "third_party"] {
        rust_files(&ws.join(d), &mut files);
    }
    let (mut embedded, mut read_at_run) = (0, 0);
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        let dir = f.parent().unwrap();
        // Compiled in: relative to the source file.
        for mac in ["include_str!(", "include_bytes!("] {
            for (at, _) in text.match_indices(mac) {
                let Some(lit) = literal_after(&text, at + mac.len()) else {
                    continue;
                };
                let rel = normal(dir.join(lit).strip_prefix(&ws).unwrap_or(&dir.join(lit)));
                assert!(
                    triggers(&push, &rel),
                    "{rel} is compiled in ({}) but CI's paths ignore it",
                    f.display()
                );
                embedded += 1;
            }
        }
        // Read at run time (a test reading a document, the packager reading the
        // installer script): a literal handed to `.join(` or a `read*(` call that names an
        // existing file or folder, relative to the crate or to the workspace. An example
        // is never run by CI (`shoot_readme` *writes* docs/images/), so what it reads at
        // run time cannot change a CI result; what it compiles in is checked above.
        if f.components().any(|c| c.as_os_str() == "examples") {
            continue;
        }
        let krate = f
            .ancestors()
            .find(|a| a.join("Cargo.toml").is_file())
            .unwrap_or(&ws);
        for call in [".join(", "read_to_string(", "fs::read("] {
            for (at, _) in text.match_indices(call) {
                let Some(lit) = literal_after(&text, at + call.len()) else {
                    continue;
                };
                if lit.is_empty() || lit.starts_with('.') && !lit.starts_with("..") {
                    continue;
                }
                // A base the literal is relative to: the workspace or the crate (`ws`,
                // `root`, `workspace()`, `CARGO_MANIFEST_DIR`), or a path with folders in
                // it; `x.join(lang).join("README.md")` is relative to something else.
                let before = text[..at].trim_end();
                let from_base = ["workspace()", "ws", "root", "CARGO_MANIFEST_DIR\"))"]
                    .iter()
                    .any(|b| before.ends_with(b));
                if !from_base && !lit.contains('/') {
                    continue;
                }
                for base in [krate, ws.as_path()] {
                    let p = base.join(lit);
                    let rel = normal(p.strip_prefix(&ws).unwrap_or(&p));
                    if rel.starts_with("target") || rel.is_empty() {
                        continue;
                    }
                    let probe = if p.is_file() {
                        Some(rel.clone())
                    } else if p.is_dir() && rel.contains('/') {
                        a_file_in(&p).map(|q| normal(q.strip_prefix(&ws).unwrap_or(&q)))
                    } else {
                        None
                    };
                    if let Some(probe) = probe {
                        assert!(
                            triggers(&push, &probe),
                            "{probe} is read by {} but CI's paths ignore it",
                            f.display()
                        );
                        read_at_run += 1;
                    }
                }
            }
        }
    }
    assert!(
        embedded > 50,
        "the scan found the embedded files: {embedded}"
    );
    assert!(
        read_at_run > 10,
        "the scan found the files read at run time: {read_at_run}"
    );
}
