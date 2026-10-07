//! `cargo run -p xtask -- package` — the files of a Windows release (ADR 0032):
//!
//! ```text
//! package                 installer, portable zip, plug-ins zip, sums
//! package --no-build      lay out what is already built
//! package --no-installer  zips only (no NSIS needed)
//! package --sign          sign with the signer in the environment (sign.rs)
//! package --sign-test     sign with the test certificate (`sign-test-cert`)
//! ```
//!
//! **One Windows edition, every driver in it** (ADR 0025 §10, round 21): the program is
//! always built with ASIO next to WASAPI. The task fetches the Steinberg ASIO SDK into a
//! cache outside the repository and checks its pinned SHA-256 (`asio_sdk.rs`), finds
//! LLVM's libclang, builds with `ASIO_SYS_REQUIRE_SDK=1` (so the build fails rather than
//! quietly leaving ASIO out) and then asks the built program which drivers it has
//! (`cantus --audio-drivers`): a release without ASIO is never written.
//!
//! Out: `target/package/dist/` — `Cantus-<v>-windows-x64-setup.exe`,
//! `Cantus-<v>-windows-x64-portable.zip`, `Cantus-<v>-plugins-windows-x64.zip`,
//! `SHA256SUMS.txt`. The names come from `cantus_update::release`, which the app's
//! updater reads too.

use crate::archive::{self, Entry};
use crate::sign::{self, Signer};
use crate::{nsis, run, target_dir, workspace};
use cantus_update::release;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What one flavour's staging folder holds, by destination.
#[derive(Clone, Debug, Default)]
pub struct Staged {
    /// Into the install folder: `cantus.exe`, the licence, the readme.
    pub app: Vec<Entry>,
    /// Into the CLAP folder: `Cantus <Name>.clap`.
    pub clap: Vec<Entry>,
    /// Into the VST3 folder: `Cantus <Name>.vst3/…`.
    pub vst3: Vec<Entry>,
}

impl Staged {
    /// The binaries a signer signs: the program and every plug-in library.
    pub fn binaries(&self) -> Vec<PathBuf> {
        let lib = |e: &&Entry| {
            let n = e.name.to_ascii_lowercase();
            n.ends_with(".exe") || n.ends_with(".clap") || n.ends_with(".vst3")
        };
        self.app
            .iter()
            .chain(&self.clap)
            .chain(&self.vst3)
            .filter(lib)
            .map(|e| e.src.clone())
            .collect()
    }
}

/// What the release is made from.
#[derive(Clone, Debug)]
pub struct Inputs {
    /// The built `cantus.exe`.
    pub exe: PathBuf,
    /// `target/bundled/` (`xtask bundle`).
    pub bundled: PathBuf,
    /// The plug-ins' names (`Cantus Aequalis`, …).
    pub plugins: Vec<String>,
    pub license: PathBuf,
    pub copyright: PathBuf,
    pub readme: PathBuf,
    /// Documents beside the program, `(name in the install folder, source)`:
    /// `THIRD-PARTY-NOTICES.txt`, `PRIVACY.txt` and the manual's pages
    /// (`manual/en/index.html`, …), which About and *Help ▸ Manual* open (round 22).
    pub docs: Vec<(String, PathBuf)>,
}

fn copy_into(from: &Path, to: &Path) {
    crate::copy(from, to);
}

fn entry(dir: &Path, name: &str) -> Entry {
    Entry {
        name: name.to_owned(),
        src: dir.join(name.replace('/', std::path::MAIN_SEPARATOR_STR)),
    }
}

/// Lay out the release under `stage/` (`app/`, `clap/`, `vst3/`).
pub fn stage(inputs: &Inputs, exe: &Path, stage: &Path, version: &str) -> Staged {
    let _ = std::fs::remove_dir_all(stage);
    let app = stage.join("app");
    copy_into(exe, &app.join("cantus.exe"));
    copy_into(&inputs.license, &app.join("LICENSE.txt"));
    copy_into(&inputs.copyright, &app.join("COPYRIGHT.txt"));
    let readme = std::fs::read_to_string(&inputs.readme)
        .unwrap_or_else(|e| panic!("read {}: {e}", inputs.readme.display()))
        .replace("{version}", version)
        .replace("\r\n", "\n")
        .replace('\n', "\r\n");
    std::fs::write(app.join("README.txt"), readme).unwrap();
    for (name, src) in &inputs.docs {
        copy_into(
            src,
            &app.join(name.replace('/', std::path::MAIN_SEPARATOR_STR)),
        );
    }

    let clap = stage.join("clap");
    let vst3 = stage.join("vst3");
    for name in &inputs.plugins {
        let c = format!("{name}.clap");
        copy_into(&inputs.bundled.join(&c), &clap.join(&c));
        let v = format!("{name}.vst3");
        for e in archive::walk(&inputs.bundled.join(&v)) {
            copy_into(&e.src, &vst3.join(&v).join(&e.name));
        }
    }
    Staged {
        app: ["cantus.exe", "LICENSE.txt", "COPYRIGHT.txt", "README.txt"]
            .iter()
            .copied()
            .chain(inputs.docs.iter().map(|(n, _)| n.as_str()))
            .map(|n| entry(&app, n))
            .collect(),
        clap: archive::walk(&clap),
        vst3: archive::walk(&vst3),
    }
}

/// The portable zip's entries: the app at the top, the marker, the plug-ins in `plugins/`.
pub fn portable_entries(s: &Staged, marker: &Path) -> Vec<Entry> {
    let mut out: Vec<Entry> = s.app.clone();
    out.push(Entry {
        name: cantus_update::install::PORTABLE_MARKER.into(),
        src: marker.to_path_buf(),
    });
    let under = |prefix: &str, list: &[Entry]| {
        list.iter()
            .map(|e| Entry {
                name: format!("{prefix}/{}", e.name),
                src: e.src.clone(),
            })
            .collect::<Vec<_>>()
    };
    out.extend(under("plugins/CLAP", &s.clap));
    out.extend(under("plugins/VST3", &s.vst3));
    out
}

/// The plug-ins zip: `CLAP/`, `VST3/`, the licence, the notices and the readme (not
/// the program's manual).
pub fn plugin_entries(s: &Staged) -> Vec<Entry> {
    let mut out: Vec<Entry> = s
        .app
        .iter()
        .filter(|e| e.name != "cantus.exe" && !e.name.starts_with("manual/"))
        .cloned()
        .collect();
    out.extend(s.clap.iter().map(|e| Entry {
        name: format!("CLAP/{}", e.name),
        src: e.src.clone(),
    }));
    out.extend(s.vst3.iter().map(|e| Entry {
        name: format!("VST3/{}", e.name),
        src: e.src.clone(),
    }));
    out
}

/// The release's file names, sorted — what `dist/` must hold.
pub fn dist_names(version: &str, installer: bool) -> Vec<String> {
    let mut v = vec![
        release::plugins_name(version),
        release::SUMS_NAME.to_owned(),
        release::portable_name(version),
    ];
    if installer {
        v.push(release::installer_name(version));
    }
    v.sort();
    v
}

/// How the installer is made, when it is.
pub struct InstallerJob {
    pub makensis: PathBuf,
    pub icon: PathBuf,
    /// The command that signs the uninstaller NSIS writes (`%1` is the file).
    pub uninst_sign: Option<String>,
    /// Environment for makensis's children (the signer, for the uninstaller).
    pub env: Vec<(String, String)>,
}

/// Make every release file from `inputs` into `out/dist/`: stage, sign, zip, write the
/// NSIS script and run it, then the sums. Returns the dist folder.
pub fn assemble(
    inputs: &Inputs,
    out: &Path,
    version: &str,
    installer: Option<&InstallerJob>,
    signer: Option<&Signer>,
) -> PathBuf {
    let dist = out.join("dist");
    let _ = std::fs::remove_dir_all(&dist);
    std::fs::create_dir_all(&dist).unwrap();
    let marker = out.join(cantus_update::install::PORTABLE_MARKER);
    std::fs::write(&marker, cantus_update::install::PORTABLE_MARKER_TEXT).unwrap();
    let staged = stage(inputs, &inputs.exe, &out.join("stage"), version);
    if let Some(s) = signer {
        let bins = staged.binaries();
        sign::sign(s, &bins);
        sign::check(&bins);
    }
    archive::zip(
        &dist.join(release::portable_name(version)),
        &format!("Cantus-{version}"),
        &portable_entries(&staged, &marker),
    );
    archive::zip(
        &dist.join(release::plugins_name(version)),
        &format!("Cantus-{version}-plugins"),
        &plugin_entries(&staged),
    );
    let work = out.join("nsis");
    let setup = dist.join(release::installer_name(version));
    let nsi = nsis::write_work(
        &work,
        &staged,
        &nsis::Config {
            version: version.to_owned(),
            outfile: setup.clone(),
            icon: installer.map(|j| j.icon.clone()),
            license: staged.app[1].src.clone(),
            uninst_sign: installer.and_then(|j| j.uninst_sign.clone()),
        },
    );
    if let Some(job) = installer {
        let mut cmd = Command::new(&job.makensis);
        cmd.args(["/V3", "/INPUTCHARSET", "UTF8"]).arg(&nsi);
        for (k, v) in &job.env {
            cmd.env(k, v);
        }
        run(&mut cmd);
        if let Some(s) = signer {
            sign::sign(s, std::slice::from_ref(&setup));
            sign::check(std::slice::from_ref(&setup));
        }
    }
    write_sums(&dist);
    dist
}

/// `SHA256SUMS.txt` over every other file in `dist`.
pub fn write_sums(dist: &Path) {
    let mut entries = Vec::new();
    for e in archive::walk(dist) {
        if e.name == release::SUMS_NAME {
            continue;
        }
        let hex = cantus_update::verify::sha256_file(&e.src).unwrap();
        entries.push((e.name, hex));
    }
    std::fs::write(
        dist.join(release::SUMS_NAME),
        release::format_sums(&entries),
    )
    .unwrap();
}

/// The newest icon the app's build script rendered (`crates/cantus-app/build.rs`).
fn find_icon(profile_dir: &Path) -> Option<PathBuf> {
    let build = profile_dir.join("build");
    std::fs::read_dir(build)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("cantus-"))
        .map(|e| e.path().join("out").join("cantus.ico"))
        .filter(|p| p.is_file())
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
}

/// Build the release program with ASIO (the SDK and libclang from `asio_sdk`), failing
/// the build — not degrading — when either is missing.
fn build_app(target: &Path, tools: &crate::asio_sdk::AsioTools) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(workspace())
        .args(["build", "--release", "-p", "cantus", "--features", "asio"])
        .env("CARGO_TARGET_DIR", target);
    for (k, v) in tools.env() {
        cmd.env(k, v);
    }
    run(&mut cmd);
}

/// Ask the built program which audio drivers it has; a release without ASIO stops here.
pub fn check_drivers(exe: &Path) {
    let out = Command::new(exe)
        .arg("--audio-drivers")
        .output()
        .unwrap_or_else(|e| panic!("run {}: {e}", exe.display()));
    let text = String::from_utf8_lossy(&out.stdout);
    eprintln!("{}: {}", exe.display(), text.trim());
    if !crate::asio_sdk::lists_asio(&text) {
        eprintln!(
            "{} does not list the ASIO driver type: refusing to package it",
            exe.display()
        );
        std::process::exit(1);
    }
}

/// The CLI.
pub fn package(args: &[String]) {
    let has = |f: &str| args.iter().any(|a| a == f);
    if has("--asio") || has("--all") {
        eprintln!("note: --asio and --all are gone: there is one edition, with ASIO (round 21)");
    }
    let build = !has("--no-build");
    let target = target_dir();
    let signer = if has("--sign-test") {
        Some(sign::test_signer(&target).unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1)
        }))
    } else if has("--sign") {
        match sign::from_env() {
            Ok(Some(s)) => Some(s),
            Ok(None) => {
                eprintln!("--sign: set CANTUS_SIGN (pfx, store or azure); see xtask/src/sign.rs");
                std::process::exit(1)
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1)
            }
        }
    } else {
        None
    };

    if build {
        let tools = crate::asio_sdk::ensure().unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1)
        });
        build_app(&target, &tools);
    }
    let exe = target.join("release").join("cantus.exe");
    if !exe.is_file() {
        eprintln!("{} is not built", exe.display());
        std::process::exit(1);
    }
    check_drivers(&exe);
    crate::bundle(&if build {
        vec![]
    } else {
        vec!["--no-build".to_string()]
    });

    let ws = workspace();
    let inputs = Inputs {
        exe,
        bundled: target.join("bundled"),
        plugins: cantus_plugins::desc::ALL
            .iter()
            .map(|d| d.full_name.to_owned())
            .collect(),
        license: ws.join("LICENSE-GPL"),
        copyright: ws.join("LICENSE"),
        readme: ws.join("xtask/installer/README.txt"),
        docs: release_docs(&ws, &target),
    };

    let installer = if has("--no-installer") {
        None
    } else {
        let Some(makensis) = nsis::makensis() else {
            eprintln!(
                "makensis not found: install NSIS 3 (docs/round20/packaging.md) or set CANTUS_MAKENSIS; \
                 or pass --no-installer"
            );
            std::process::exit(1);
        };
        let icon = find_icon(&target.join("release")).unwrap_or_else(|| {
            eprintln!("the app's icon is not built (cargo build --release -p cantus)");
            std::process::exit(1)
        });
        let me = std::env::current_exe().expect("xtask's own path");
        let mut env = Vec::new();
        if let Some(Signer::Pfx { file, password }) = &signer {
            env.push(("CANTUS_SIGN".to_string(), "pfx".to_string()));
            env.push(("CANTUS_SIGN_PFX".to_string(), file.display().to_string()));
            env.push(("CANTUS_SIGN_PFX_PASSWORD".to_string(), password.clone()));
        }
        Some(InstallerJob {
            makensis,
            icon,
            uninst_sign: signer
                .as_ref()
                .map(|_| format!("\"{}\" sign-file \"%1\"", me.display())),
            env,
        })
    };
    let out = target.join("package");
    let dist = assemble(&inputs, &out, VERSION, installer.as_ref(), signer.as_ref());
    let want = dist_names(VERSION, installer.is_some());
    let got: Vec<String> = archive::walk(&dist).into_iter().map(|e| e.name).collect();
    if got != want {
        eprintln!("dist holds {got:?}, expected {want:?}");
        std::process::exit(1);
    }
    eprintln!("release files in {}:", dist.display());
    for n in got {
        let len = dist.join(&n).metadata().map(|m| m.len()).unwrap_or(0);
        eprintln!("  {n}  ({:.1} MB)", len as f64 / 1_048_576.0);
    }
}

/// `sign-file <path>`: sign one file with the signer in the environment (NSIS calls this
/// for the uninstaller it writes).
pub fn sign_file(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("usage: xtask sign-file <path>");
        std::process::exit(2);
    };
    match sign::from_env() {
        Ok(Some(s)) => sign::sign(&s, &[PathBuf::from(path)]),
        Ok(None) => eprintln!("CANTUS_SIGN is not set: {path} left unsigned"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

/// The documents a release carries beside the program: the notices, the privacy statement
/// (Markdown, readable as plain text) and the manual rendered to HTML under
/// `target/manual/` (`manual.rs`).
pub fn release_docs(ws: &Path, target: &Path) -> Vec<(String, PathBuf)> {
    let mut docs = vec![
        (
            "THIRD-PARTY-NOTICES.txt".to_owned(),
            ws.join(crate::notices::FILE),
        ),
        ("PRIVACY.txt".to_owned(), ws.join("PRIVACY.md")),
    ];
    let out = target.join("manual");
    docs.extend(
        crate::manual::build(&ws.join("docs/manual"), &out)
            .unwrap_or_else(|e| panic!("the manual: {e}"))
            .into_iter()
            .map(|name| (format!("manual/{name}"), out.join(&name))),
    );
    docs
}
