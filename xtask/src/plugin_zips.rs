//! `cargo run -p xtask -- plugin-zips <out-dir>` — Cantus's effects as plug-ins on their own,
//! for people who come for the effects and not the DAW (round 42, BACKLOG §6a; the website's
//! Plug-ins pages link these files):
//!
//! ```text
//! plugin-zips <out>                  build every plug-in (`bundle`), then zip
//! plugin-zips <out> --no-build       zip what `bundle` already laid out in target/bundled/
//! plugin-zips <out> --from <dir>     zip the bundles in <dir>: `bundle`'s layout, or a
//!                                    release stage's `clap/` and `vst3/` (`package`), signed
//! plugin-zips <out> --sums           and write SHA256SUMS.txt over every file in <out>
//! ```
//!
//! Into `<out>`, for the system it runs on (`release::Target::current`), named by
//! `cantus_update::release`: one zip per effect, `Cantus-plugin-<slug>-<platform>.zip`, and one
//! with them all, `Cantus-plugins-all-<platform>.zip`. Each holds `CLAP/Cantus <Name>.clap`,
//! `VST3/Cantus <Name>.vst3/…`, the licence, the copyright, the notices and a README with the
//! folders to copy them into, under one top folder. On a Mac both architectures are built
//! (`bundle --target`), joined with `lipo` and signed ad hoc (`codesign -s -`): one universal
//! zip per effect, as the disk image is universal. No version in the names: the website links
//! them through `releases/latest/download/` (ADR 0048 §7).

use crate::archive::{self, Entry};
use crate::{run, target_dir, workspace};
use cantus_plugins::desc::{self, Descriptor};
use cantus_update::release::{self, Target};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The two architectures a Mac's plug-ins are built for and joined from.
pub const MAC_TRIPLES: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

/// The website's page for these downloads (the README names it).
pub const SITE: &str = "https://cantusdaw.com/plugins/";

/// Where a bundle of each standard is found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub clap: PathBuf,
    pub vst3: PathBuf,
}

impl Source {
    /// `bundle`'s layout (`Cantus <Name>.clap` beside `Cantus <Name>.vst3`), or a release
    /// stage's (`clap/` and `vst3/`, what `package` signs and zips).
    pub fn of(dir: &Path) -> Source {
        if dir.join("clap").is_dir() && dir.join("vst3").is_dir() {
            Source {
                clap: dir.join("clap"),
                vst3: dir.join("vst3"),
            }
        } else {
            Source {
                clap: dir.to_path_buf(),
                vst3: dir.to_path_buf(),
            }
        }
    }
}

/// One plug-in's entries: its CLAP (a file on Windows and Linux, a bundle on a Mac) under
/// `CLAP/` and its VST3 bundle under `VST3/`.
pub fn plugin_entries(full_name: &str, src: &Source) -> Vec<Entry> {
    let mut out = Vec::new();
    let clap = format!("{full_name}.clap");
    let at = src.clap.join(&clap);
    if at.is_dir() {
        out.extend(archive::walk(&at).into_iter().map(|e| Entry {
            name: format!("CLAP/{clap}/{}", e.name),
            src: e.src,
        }));
    } else {
        out.push(Entry {
            name: format!("CLAP/{clap}"),
            src: at,
        });
    }
    let vst3 = format!("{full_name}.vst3");
    out.extend(
        archive::walk(&src.vst3.join(&vst3))
            .into_iter()
            .map(|e| Entry {
                name: format!("VST3/{vst3}/{}", e.name),
                src: e.src,
            }),
    );
    out
}

/// What runs, by its name in the zip: a Mac bundle's program, a Linux library (on Linux the
/// CLAP is the library itself).
pub fn is_exec(t: Target, name: &str) -> bool {
    match t {
        Target::WindowsX64 => false,
        Target::LinuxX64 => name.ends_with(".so") || name.ends_with(".clap"),
        Target::MacUniversal => name.contains("/Contents/MacOS/"),
    }
}

/// The folders each standard is read from on `t` — (what, for you, for everyone). The same
/// folders as the installer's README and the website's install steps (a test holds them).
pub fn folders(t: Target) -> [(&'static str, &'static str, &'static str); 2] {
    match t {
        Target::WindowsX64 => [
            (
                "CLAP",
                r"%LOCALAPPDATA%\Programs\Common\CLAP",
                r"C:\Program Files\Common Files\CLAP",
            ),
            (
                "VST3",
                r"%LOCALAPPDATA%\Programs\Common\VST3",
                r"C:\Program Files\Common Files\VST3",
            ),
        ],
        Target::LinuxX64 => [
            ("CLAP", "~/.clap", "/usr/lib/clap"),
            ("VST3", "~/.vst3", "/usr/lib/vst3"),
        ],
        Target::MacUniversal => [
            (
                "CLAP",
                "~/Library/Audio/Plug-Ins/CLAP",
                "/Library/Audio/Plug-Ins/CLAP",
            ),
            (
                "VST3",
                "~/Library/Audio/Plug-Ins/VST3",
                "/Library/Audio/Plug-Ins/VST3",
            ),
        ],
    }
}

/// The README of a zip holding `names` (one effect, or all of them) for `t`.
pub fn readme(t: Target, version: &str, names: &[&str]) -> String {
    let system = match t {
        Target::WindowsX64 => "Windows (x64)",
        Target::LinuxX64 => "Linux (x86-64, glibc 2.35 or later)",
        Target::MacUniversal => "macOS 11 or later (Apple silicon and Intel)",
    };
    let what = match names {
        [one] => format!("{one} {version}"),
        _ => format!("Cantus's {} effects, {version}", names.len()),
    };
    let mut s = format!(
        "{what}\n{}\n\nCLAP and VST3 plug-ins for {system}, from Cantus, a free and open-source DAW.\n\
         They run in any host that loads CLAP or VST3 plug-ins. {SITE}\n\n",
        "=".repeat(what.chars().count())
    );
    s += "Install\n-------\nCopy each \"Cantus <Name>.clap\" from CLAP/ into a CLAP folder and each\n\
          \"Cantus <Name>.vst3\" from VST3/ into a VST3 folder, then let your host scan again:\n\n";
    for (std, you, all) in folders(t) {
        s += &format!("  {std}, for you:        {you}\n  {std}, for everyone:   {all}\n");
    }
    s += "\nA host that does not read those folders can be pointed at them in its plug-in\n\
          settings. One copy of each is enough: CLAP where the host reads CLAP, else VST3.\n\n";
    s += "Not code-signed yet\n-------------------\n";
    s += match t {
        Target::WindowsX64 => {
            "The files carry no code-signing certificate yet. If Windows marked the zip as\n\
             downloaded from the internet, right-click it > Properties > Unblock before\n\
             extracting, so the host loads the plug-ins without asking.\n\n"
        }
        Target::LinuxX64 => {
            "Nothing to allow on Linux: the libraries are marked executable in the zip.\n\n"
        }
        Target::MacUniversal => {
            "They are signed ad hoc, not by a registered developer, so Gatekeeper stops a\n\
             host from loading them the first time. Either open System Settings > Privacy &\n\
             Security and choose Open Anyway after the host's refusal, or clear the\n\
             download mark once in Terminal (here for the folders for you):\n\n\
             \x20 xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/CLAP/Cantus*.clap\n\
             \x20 xattr -dr com.apple.quarantine ~/Library/Audio/Plug-Ins/VST3/Cantus*.vst3\n\n"
        }
    };
    s += "Check the download\n------------------\nThe release's SHA256SUMS.txt lists every file's SHA-256, \
          and the website shows it\nbeside each button.\n\n";
    s += "Licence\n-------\nGPL-3.0-or-later (LICENSE.txt); COPYRIGHT.txt says which parts of the source are\n\
          MIT OR Apache-2.0, and THIRD-PARTY-NOTICES.txt names everything they are built from.\n\
          \"Cantus\" and its logos are trademarks of Luis Ángel Almazán: a modified copy is\n\
          welcome under its own name (TRADEMARKS.md, https://github.com/luis314159/cantus-daw).\n\n";
    s += "The whole DAW, free, with these effects and its own instruments:\nhttps://cantusdaw.com/download/\n";
    if t == Target::WindowsX64 {
        s = s.replace('\n', "\r\n");
    }
    s
}

/// The texts beside the plug-ins in every zip: (name in the zip, source).
pub fn texts(readme_file: &Path) -> Vec<(String, PathBuf)> {
    let ws = workspace();
    vec![
        ("COPYRIGHT.txt".into(), ws.join("LICENSE")),
        ("LICENSE.txt".into(), ws.join("LICENSE-GPL")),
        ("README.txt".into(), readme_file.to_path_buf()),
        (
            "THIRD-PARTY-NOTICES.txt".into(),
            ws.join(crate::notices::FILE),
        ),
    ]
}

/// Every zip's name for `t`, sorted: one per plug-in, and the one with all.
pub fn names(t: Target) -> Vec<String> {
    let mut v: Vec<String> = desc::ALL
        .iter()
        .map(|d| release::plugin_zip_name(d.slug, t))
        .chain([release::plugins_all_name(t)])
        .collect();
    v.sort();
    v
}

/// Write every zip for `t` into `out` from the bundles in `src`. The README is written into
/// `work`. Returns the names written.
pub fn write(
    out: &Path,
    work: &Path,
    src: &Source,
    t: Target,
    version: &str,
    plugins: &[&Descriptor],
) -> Vec<String> {
    std::fs::create_dir_all(out).unwrap();
    let mut written = Vec::new();
    let zip_one = |name: &str, top: &str, full: &[&str], readme_name: &str| {
        let file = work.join(readme_name);
        std::fs::create_dir_all(work).unwrap();
        std::fs::write(&file, readme(t, version, full)).unwrap();
        let mut entries: Vec<Entry> = texts(&file)
            .into_iter()
            .map(|(name, src)| Entry { name, src })
            .collect();
        for f in full {
            entries.extend(plugin_entries(f, src));
        }
        for e in &entries {
            if !e.src.is_file() {
                eprintln!("{} is missing ({name})", e.src.display());
                std::process::exit(1);
            }
        }
        archive::zip_modes(&out.join(name), top, &entries, &|n| {
            Some(if is_exec(t, n) { 0o755 } else { 0o644 })
        });
    };
    for d in plugins {
        let name = release::plugin_zip_name(d.slug, t);
        zip_one(
            &name,
            &format!("{} {version}", d.full_name),
            &[d.full_name],
            &format!("README-{}.txt", d.slug),
        );
        written.push(name);
    }
    let all: Vec<&str> = plugins.iter().map(|d| d.full_name).collect();
    let name = release::plugins_all_name(t);
    zip_one(
        &name,
        &format!("Cantus plug-ins {version}"),
        &all,
        "README-all.txt",
    );
    written.push(name);
    written.sort();
    written
}

/// A Mac's universal bundles: both architectures built and laid out (`bundle --target`),
/// each program joined with `lipo` and each bundle signed ad hoc.
fn universal(build: bool) -> PathBuf {
    for t in MAC_TRIPLES {
        let mut args = vec!["--target".to_string(), t.to_string()];
        if !build {
            args.push("--no-build".into());
        }
        crate::bundle(&args);
    }
    let out = target_dir().join("bundled-universal");
    let _ = std::fs::remove_dir_all(&out);
    let [arm, intel] = MAC_TRIPLES.map(|t| target_dir().join(format!("bundled-{t}")));
    for d in desc::ALL.iter() {
        for ext in ["clap", "vst3"] {
            let bundle = format!("{}.{ext}", d.full_name);
            for e in archive::walk(&arm.join(&bundle)) {
                crate::copy(&e.src, &out.join(&bundle).join(&e.name));
            }
            let program = Path::new("Contents/MacOS").join(d.full_name);
            run(Command::new("lipo")
                .arg("-create")
                .arg(arm.join(&bundle).join(&program))
                .arg(intel.join(&bundle).join(&program))
                .arg("-output")
                .arg(out.join(&bundle).join(&program)));
            run(Command::new("codesign")
                .args(["--force", "--sign", "-"])
                .arg(out.join(&bundle)));
        }
    }
    out
}

/// The CLI.
pub fn command(args: &[String]) {
    let has = |f: &str| args.iter().any(|a| a == f);
    let from = args
        .iter()
        .position(|a| a == "--from")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let Some(out) = args
        .iter()
        .enumerate()
        .find(|(i, a)| !a.starts_with("--") && (*i == 0 || args[i - 1] != "--from"))
        .map(|(_, a)| PathBuf::from(a))
    else {
        eprintln!("usage: xtask plugin-zips <out-dir> [--from <dir>] [--no-build] [--sums]");
        std::process::exit(2);
    };
    let Some(t) = Target::current() else {
        eprintln!("plug-in zips are made for Windows x64, Linux x86-64 and macOS only");
        std::process::exit(2);
    };
    let build = !has("--no-build");
    let src = match from {
        Some(d) => Source::of(&d),
        None if t == Target::MacUniversal => Source::of(&universal(build)),
        None => {
            crate::bundle(&if build {
                vec![]
            } else {
                vec!["--no-build".to_string()]
            });
            Source::of(&target_dir().join("bundled"))
        }
    };
    let work = target_dir().join("plugin-zips");
    let all: Vec<&Descriptor> = desc::ALL.iter().collect();
    let written = write(&out, &work, &src, t, crate::package::VERSION, &all);
    if written != names(t) {
        eprintln!("wrote {written:?}, expected {:?}", names(t));
        std::process::exit(1);
    }
    if has("--sums") {
        crate::package::write_sums(&out);
    }
    eprintln!("{} plug-in zips in {}:", written.len(), out.display());
    for n in written {
        let len = out.join(&n).metadata().map(|m| m.len()).unwrap_or(0);
        eprintln!("  {n}  ({:.1} MB)", len as f64 / 1_048_576.0);
    }
}
