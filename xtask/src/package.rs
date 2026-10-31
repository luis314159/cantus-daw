//! `cargo run -p xtask -- package` — the files of a Windows release (ADR 0032):
//!
//! ```text
//! package                 installer, portable zip, plug-ins zip, sums
//! package --no-build      lay out what is already built
//! package --no-installer  zips only (no NSIS needed)
//! package --sign          sign with the signer in the environment (sign.rs)
//! package --sign-test     sign with the test certificate (`sign-test-cert`)
//! package --v4            also the x86-64-v4 (AVX-512) program
//! package --stage-only    build and lay out what SignPath signs (programs, plug-ins and
//!                         the uninstaller, made by an uninstaller-only NSIS pass)
//! package --from-stage D  zips, installer and sums from a signed stage D (ADR 0048 §8)
//! ```
//!
//! **One Windows edition, every driver in it** (ADR 0025 §10, round 21): the program is
//! always built with ASIO next to WASAPI. The task fetches the Steinberg ASIO SDK into a
//! cache outside the repository and checks its pinned SHA-256 (`asio_sdk.rs`), finds
//! LLVM's libclang, builds with `ASIO_SYS_REQUIRE_SDK=1` (so the build fails rather than
//! quietly leaving ASIO out) and then asks the built program which drivers it has
//! (`cantus --audio-drivers`): a release without ASIO is never written.
//!
//! **One program per x86-64 level, and a launcher** (ADR 0031 §5.3, round 22): the program
//! is built twice — `cantus-x86-64-v3.exe` (AVX2, FMA, BMI2) and `cantus-x86-64-v2.exe`,
//! and with `--v4` a third, `cantus-x86-64-v4.exe` (AVX-512) — each in its own target folder
//! (`levels.rs`), and `cantus.exe` is the launcher (`crates/cantus-launch`), built for the
//! plain baseline, which starts the highest level the processor runs. The Start menu, the
//! `.cantus` association, "Open with" and the installer's "run Cantus" all name `cantus.exe`,
//! as before. Every program is asked its level (`--version`) and its drivers; then the
//! launcher, laid out as released, must start one that lists ASIO. The plug-in scanner
//! (`cantus-scan.exe`) and the plug-in sandbox (`cantus-sandbox.exe`, round 29), which
//! the app starts from beside itself, ship beside them.
//!
//! Out: `target/package/dist/` — `Cantus-<v>-windows-x64-setup.exe` (and the same file as
//! `Cantus-windows-x64-setup.exe`, the stable name a website links to — ADR 0048 §7),
//! `Cantus-<v>-windows-x64-portable.zip`, `Cantus-<v>-plugins-windows-x64.zip`,
//! `SHA256SUMS.txt`. The names come from `cantus_update::release`, which the app's
//! updater reads too.

use crate::archive::{self, Entry};
use crate::sign::{self, Signer};
use crate::{levels, nsis, run, target_dir, workspace};
use cantus_launch::Level;
use cantus_update::release;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What one flavour's staging folder holds, by destination.
#[derive(Clone, Debug, Default)]
pub struct Staged {
    /// Into the install folder: `cantus.exe` (the launcher), one program per level, the
    /// plug-in scanner, the licence, the readme.
    pub app: Vec<Entry>,
    /// Into the CLAP folder: `Cantus <Name>.clap`.
    pub clap: Vec<Entry>,
    /// Into the VST3 folder: `Cantus <Name>.vst3/…`.
    pub vst3: Vec<Entry>,
}

impl Staged {
    /// A stage laid out by [`stage`] (`app/`, `clap/`, `vst3/`) — or one SignPath sent
    /// back, signed, with the same layout. `uninstall.exe` at its top is not a staged file.
    pub fn from_dir(stage: &Path) -> Staged {
        Staged {
            app: archive::walk(&stage.join("app")),
            clap: archive::walk(&stage.join("clap")),
            vst3: archive::walk(&stage.join("vst3")),
        }
    }

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
    /// The launcher (`cantus-launch.exe`), released as `cantus.exe`.
    pub launcher: PathBuf,
    /// The program built for each level, released as `cantus-x86-64-vN.exe`.
    pub programs: Vec<(Level, PathBuf)>,
    /// `cantus-scan.exe`, which reads each plug-in file in a process of its own.
    pub scanner: PathBuf,
    /// `cantus-sandbox.exe`, which runs third-party plug-ins out of Cantus's process
    /// (round 29, ADR 0063).
    pub sandbox: PathBuf,
    /// `cantus-mcp.exe`, the stdio bridge MCP clients start to reach the running Cantus
    /// (round 34, ADR 0081). // r34:mcp
    pub bridge: PathBuf,
    /// More files beside the program, `(name in the install folder, source)`: the ML
    /// helper `cantus-ml.exe` and the Visual C++ runtime its inference library needs
    /// (round 34, ADR 0085). // r34:stems
    pub beside: Vec<(String, PathBuf)>, // r34:stems
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
    /// Require every file the release signs (programs, launcher, scanner, plug-in
    /// libraries, installer) to carry its product name and version (ADR 0048 §9). Off only
    /// for the tests' stand-in files.
    pub check_metadata: bool,
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

/// The launcher's name in a release: what every shortcut and registry entry names.
pub const LAUNCHER: &str = "cantus.exe";
/// The plug-in scanner's name (`cantus_host::Scanner::beside_this_program`).
pub const SCANNER: &str = "cantus-scan.exe";
/// The plug-in sandbox's name (`cantus_host::bridge::Sandboxes::default_exe`, ADR 0063).
pub const SANDBOX: &str = "cantus-sandbox.exe";
/// The MCP stdio bridge's name (`cantus_ui::ai_mcp::setup::bridge_path`, ADR 0081). // r34:mcp
pub const BRIDGE: &str = "cantus-mcp.exe";
const TEXTS: [&str; 3] = ["LICENSE.txt", "COPYRIGHT.txt", "README.txt"];

/// A level's program in the release: `cantus-x86-64-v3.exe`. The release is the Windows
/// one wherever this runs (CI tests it on Linux too), so the suffix is not the host's —
/// unlike a program copied to be run here (`check`), which keeps the host's.
pub fn release_program(level: Level) -> String {
    format!("cantus-{}.exe", level.name())
}

/// The install folder's files for a release carrying `levels`, in staging order.
pub fn app_names(levels: &[Level]) -> Vec<String> {
    let mut v = vec![LAUNCHER.to_owned()];
    let mut sorted = levels.to_vec();
    sorted.sort();
    v.extend(sorted.iter().map(|l| release_program(*l)));
    v.push(SCANNER.to_owned());
    v.push(SANDBOX.to_owned());
    v.push(BRIDGE.to_owned()); // r34:mcp
    v.extend(TEXTS.iter().map(|t| t.to_string()));
    v
}

/// Lay out the release under `stage/` (`app/`, `clap/`, `vst3/`).
pub fn stage(inputs: &Inputs, stage: &Path, version: &str) -> Staged {
    let _ = std::fs::remove_dir_all(stage);
    let app = stage.join("app");
    copy_into(&inputs.launcher, &app.join(LAUNCHER));
    for (level, exe) in &inputs.programs {
        copy_into(exe, &app.join(release_program(*level)));
    }
    copy_into(&inputs.scanner, &app.join(SCANNER));
    copy_into(&inputs.sandbox, &app.join(SANDBOX));
    copy_into(&inputs.bridge, &app.join(BRIDGE)); // r34:mcp
    for (name, src) in &inputs.beside {
        // r34:stems — `ml/…` too: a folder of its own for the helper and its runtime.
        copy_into(
            src,
            &app.join(name.replace('/', std::path::MAIN_SEPARATOR_STR)),
        );
    }
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
    let levels: Vec<Level> = inputs.programs.iter().map(|(l, _)| *l).collect();
    Staged {
        app: app_names(&levels)
            .iter()
            .map(String::as_str)
            .chain(inputs.beside.iter().map(|(n, _)| n.as_str())) // r34:stems
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
        .filter(|e| {
            let n = e.name.to_ascii_lowercase();
            // r34:stems — the runtime's DLLs are the program's, not the plug-ins'.
            !n.ends_with(".exe") && !n.ends_with(".dll") && !e.name.starts_with("manual/")
        })
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
        v.push(release::STABLE_INSTALLER_NAME.to_owned());
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
    /// Written by the installer (the PFX/Azure routes sign it through `uninst_sign`), or
    /// a ready, signed `uninstall.exe` (`--from-stage`, the SignPath route).
    pub uninstaller: nsis::Uninstaller,
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
    let staged = stage(inputs, &out.join("stage"), version);
    if inputs.check_metadata {
        meta::require(&staged.binaries(), version);
    }
    if let Some(s) = signer {
        let bins = staged.binaries();
        sign::sign(s, &bins);
        sign::check(&bins);
    }
    package_staged(
        &staged,
        out,
        version,
        installer,
        signer,
        inputs.check_metadata,
    )
}

/// The release files from a staged release — signed already when it comes back from
/// SignPath (`--from-stage`): the zips, the installer when asked (signed here by a local
/// signer), its stable copy and the sums, into `out/dist/`.
pub fn package_staged(
    staged: &Staged,
    out: &Path,
    version: &str,
    installer: Option<&InstallerJob>,
    signer: Option<&Signer>,
    check_metadata: bool,
) -> PathBuf {
    let dist = out.join("dist");
    let _ = std::fs::remove_dir_all(&dist);
    std::fs::create_dir_all(&dist).unwrap();
    let marker = out.join(cantus_update::install::PORTABLE_MARKER);
    std::fs::write(&marker, cantus_update::install::PORTABLE_MARKER_TEXT).unwrap();
    archive::zip(
        &dist.join(release::portable_name(version)),
        &format!("Cantus-{version}"),
        &portable_entries(staged, &marker),
    );
    archive::zip(
        &dist.join(release::plugins_name(version)),
        &format!("Cantus-{version}-plugins"),
        &plugin_entries(staged),
    );
    let work = out.join("nsis");
    let setup = dist.join(release::installer_name(version));
    let nsi = nsis::write_work(
        &work,
        staged,
        &nsis::Config {
            version: version.to_owned(),
            outfile: setup.clone(),
            icon: installer.map(|j| j.icon.clone()),
            license: licence_of(staged),
            uninst_sign: installer.and_then(|j| j.uninst_sign.clone()),
            uninstaller: installer.map(|j| j.uninstaller.clone()).unwrap_or_default(),
        },
    );
    if let Some(job) = installer {
        let mut cmd = Command::new(&job.makensis);
        cmd.args(["/V3", "/INPUTCHARSET", "UTF8"]).arg(&nsi);
        // `sign-file`, run by NSIS on the uninstaller it writes, checks this version.
        cmd.env("CANTUS_SIGN_VERSION", version);
        for (k, v) in &job.env {
            cmd.env(k, v);
        }
        run(&mut cmd);
        if check_metadata {
            meta::require(std::slice::from_ref(&setup), version);
        }
        if let Some(s) = signer {
            sign::sign(s, std::slice::from_ref(&setup));
            sign::check(std::slice::from_ref(&setup));
        }
        // The same signed bytes under the name that never changes (ADR 0048 §7), for
        // `releases/latest/download/<name>` links; the updater asks for the versioned one.
        std::fs::copy(&setup, dist.join(release::STABLE_INSTALLER_NAME)).unwrap();
    }
    write_sums(&dist);
    dist
}

/// The licence the installer's licence page shows: the staged `LICENSE.txt`.
fn licence_of(staged: &Staged) -> PathBuf {
    staged
        .app
        .iter()
        .find(|e| e.name == "LICENSE.txt")
        .expect("the licence is staged")
        .src
        .clone()
}

/// The uninstaller, made before the installer so that it can be signed first (ADR 0048
/// §8, the SignPath route): the script compiled in its uninstaller-only form into a small
/// program carrying no release file, which writes `uninstall.exe` beside itself when run
/// (Windows only). Returns the uninstaller's path, in `work`.
pub fn write_uninstaller(
    staged: &Staged,
    work: &Path,
    makensis: &Path,
    icon: Option<&Path>,
    version: &str,
) -> PathBuf {
    // No "setup"/"install" in the name: Windows' installer detection would ask for UAC.
    let writer = work.join("cantus-uw.exe");
    let nsi = nsis::write_work(
        work,
        staged,
        &nsis::Config {
            version: version.to_owned(),
            outfile: writer.clone(),
            icon: icon.map(Path::to_path_buf),
            license: licence_of(staged),
            uninst_sign: None,
            uninstaller: nsis::Uninstaller::WriterOnly,
        },
    );
    run(Command::new(makensis)
        .args(["/V3", "/INPUTCHARSET", "UTF8"])
        .arg(&nsi));
    run(Command::new(&writer).current_dir(work));
    let out = work.join(UNINSTALLER);
    assert!(out.is_file(), "the writer left no {}", out.display());
    out
}

/// The uninstaller's name, in the install folder and at the top of a stage.
pub const UNINSTALLER: &str = "uninstall.exe";

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

/// The newest icon a build script rendered (the launcher's or the app's: the same
/// `crates/cantus-app/icon.rs`).
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

/// Build the release program for `level` with ASIO (the SDK and libclang from
/// `asio_sdk`), failing the build — not degrading — when either is missing.
fn build_app(level: Level, triple: &str, tools: &crate::asio_sdk::AsioTools) {
    let mut cmd = levels::cargo_build(level, triple);
    cmd.args(["-p", "cantus", "--features", "asio"]);
    for (k, v) in tools.env() {
        cmd.env(k, v);
    }
    run(&mut cmd);
}

/// The plug-in scanner, built with the lowest level's flags: it only reads plug-in files.
fn build_scanner(level: Level, triple: &str) {
    run(levels::cargo_build(level, triple).args(["-p", "cantus-host", "--bin", "cantus-scan"]));
}

/// The plug-in sandbox (ADR 0063), built with the lowest level's flags like the scanner: it
/// runs third-party plug-ins, whose code is theirs, and moves their audio, not ours.
fn build_sandbox(level: Level, triple: &str) {
    run(levels::cargo_build(level, triple).args([
        "-p",
        "cantus-sandbox",
        "--bin",
        "cantus-sandbox",
    ]));
}

/// The MCP stdio bridge (ADR 0081), built with the lowest level's flags: it moves JSON,
/// never audio. // r34:mcp
fn build_bridge(level: Level, triple: &str) {
    run(levels::cargo_build(level, triple).args(["-p", "cantus-mcp", "--bin", "cantus-mcp"]));
}

/// The ML helper (round 34, ADR 0085), with the lowest level's flags: its work is in the
/// inference library it loads, not in its own code. // r34:stems
fn build_ml(level: Level, triple: &str) {
    run(levels::cargo_build(level, triple).args(["-p", "cantus-ml", "--bin", "cantus-ml"]));
}

/// The ML helper's folder in the install folder, and the helper in it: the Visual C++
/// runtime sits beside the helper only (`cantus_ml::client::ClientConfig::default_exe`
/// looks there first). // r34:stems
pub const ML_DIR: &str = "ml";
pub const ML_HELPER: &str = "ml/cantus-ml.exe"; // r34:stems

/// The Visual C++ runtime the ML helper's inference library (ONNX Runtime) needs, from
/// this machine's Visual Studio redistributable folder (`CANTUS_VC_REDIST` to name it):
/// shipped beside `cantus-ml.exe`, where the loader finds it first (review 1 #1). At
/// least 14.40: builds of Visual Studio 17.10 and later crash with an older `msvcp140`
/// (`std::mutex`). // r34:stems
pub const CRT: [&str; 4] = [
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "msvcp140.dll",
    "msvcp140_1.dll",
];

/// A Windows file's version from its resources (`VS_FIXEDFILEINFO`, found by its
/// signature): major, minor, build, revision. `None` when it has none.
pub fn dll_version(path: &Path) -> Option<(u16, u16, u16, u16)> {
    let b = std::fs::read(path).ok()?;
    let sig = 0xFEEF_04BDu32.to_le_bytes();
    let at = b.windows(4).position(|w| w == sig)?;
    let word = |o: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            b.get(at + o..at + o + 4)?.try_into().ok()?,
        ))
    };
    let (ms, ls) = (word(8)?, word(12)?);
    Some((
        (ms >> 16) as u16,
        (ms & 0xffff) as u16,
        (ls >> 16) as u16,
        (ls & 0xffff) as u16,
    ))
}

pub fn find_crt() -> Result<Vec<(String, PathBuf)>, String> {
    let dir = match std::env::var_os("CANTUS_VC_REDIST") {
        Some(d) => PathBuf::from(d),
        None => {
            let mut found: Vec<(Vec<u32>, PathBuf)> = Vec::new();
            for vs in [
                "C:\\Program Files\\Microsoft Visual Studio",
                "C:\\Program Files (x86)\\Microsoft Visual Studio",
            ] {
                for year in std::fs::read_dir(vs).into_iter().flatten().flatten() {
                    for edition in std::fs::read_dir(year.path())
                        .into_iter()
                        .flatten()
                        .flatten()
                    {
                        let msvc = edition.path().join("VC").join("Redist").join("MSVC");
                        for v in std::fs::read_dir(&msvc).into_iter().flatten().flatten() {
                            let ver: Vec<u32> = v
                                .file_name()
                                .to_string_lossy()
                                .split('.')
                                .filter_map(|p| p.parse().ok())
                                .collect();
                            if let Some(crt) = std::fs::read_dir(v.path().join("x64"))
                                .into_iter()
                                .flatten()
                                .flatten()
                                .map(|e| e.path())
                                .find(|p| {
                                    p.file_name()
                                        .is_some_and(|n| n.to_string_lossy().ends_with(".CRT"))
                                })
                            {
                                found.push((ver, crt));
                            }
                        }
                    }
                }
            }
            found.sort();
            let (_, crt) = found
                .pop()
                .ok_or("no Visual C++ redistributable folder (set CANTUS_VC_REDIST)")?;
            crt
        }
    };
    crt_files(&dir)
}

/// The four DLLs in `dir`, refused when the runtime's own version is older than 14.40.
pub fn crt_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    // The DLL's own version, wherever it came from — the folder's name or the variable
    // can say anything (review 2 #16).
    let vc = dir.join(CRT[0]);
    match dll_version(&vc) {
        Some((major, minor, ..)) if (major, minor) >= (14, 40) => {}
        Some((major, minor, build, _)) => {
            return Err(format!(
                "{} is version {major}.{minor}.{build}, older than 14.40",
                vc.display()
            ))
        }
        None => {
            return Err(format!(
                "{} has no version Cantus can read (is it Microsoft's?)",
                vc.display()
            ))
        }
    }
    CRT.iter()
        .map(|n| {
            let p = dir.join(n);
            if p.is_file() {
                Ok((n.to_string(), p))
            } else {
                Err(format!("{} is not there", p.display()))
            }
        })
        .collect()
}

/// The launcher, in the ordinary target and with no `target-cpu`, so that it runs on any
/// x86-64 processor and can say why one is too old.
fn build_launcher(target: &Path) {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(workspace())
        .args(["build", "--release", "-p", "cantus-launch"])
        .env("CARGO_TARGET_DIR", target);
    run(&mut cmd);
}

/// Each program says its level and lists ASIO (where this machine can run it); then the
/// launcher, laid out beside them in `check/` as released, starts one that does too.
pub fn check_programs(inputs: &Inputs, check: &Path) {
    for (level, exe) in &inputs.programs {
        if levels::host_runs(*level) {
            levels::check_level(exe, *level);
            check_drivers(exe);
        } else {
            eprintln!(
                "note: this machine lacks {} and cannot run the {level} program to check it",
                level.missing(&cantus_launch::detected).join(", ")
            );
        }
    }
    let _ = std::fs::remove_dir_all(check);
    crate::copy(&inputs.launcher, &check.join(LAUNCHER));
    for (level, exe) in &inputs.programs {
        crate::copy(exe, &check.join(level.program()));
    }
    let launcher = check.join(LAUNCHER);
    let top = inputs
        .programs
        .iter()
        .map(|(l, _)| *l)
        .filter(|l| levels::host_runs(*l))
        .max();
    match top {
        Some(level) => levels::check_level(&launcher, level),
        None => {
            eprintln!("this machine runs none of the programs: the launcher is not checked");
            return;
        }
    }
    check_drivers(&launcher);
    let _ = std::fs::remove_dir_all(check);
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
    if let Some(i) = args.iter().position(|a| a == "--from-stage") {
        let Some(dir) = args.get(i + 1) else {
            eprintln!("usage: xtask package --from-stage <signed stage folder>");
            std::process::exit(2);
        };
        from_stage(Path::new(dir), has("--no-installer"));
        return;
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

    let levels = levels::release_levels(has("--v4"));
    let triple = levels::host_triple();
    if build {
        let tools = crate::asio_sdk::ensure().unwrap_or_else(|e| {
            eprintln!("{e}");
            std::process::exit(1)
        });
        for level in &levels {
            build_app(*level, &triple, &tools);
        }
        build_scanner(levels[0], &triple);
        build_sandbox(levels[0], &triple);
        build_bridge(levels[0], &triple); // r34:mcp
        build_ml(levels[0], &triple); // r34:stems
        build_launcher(&target);
    }
    let programs: Vec<(Level, PathBuf)> = levels
        .iter()
        .map(|l| (*l, levels::binary(*l, &triple, "cantus")))
        .collect();
    let launcher = target
        .join("release")
        .join(format!("cantus-launch{}", std::env::consts::EXE_SUFFIX));
    let scanner = levels::binary(levels[0], &triple, "cantus-scan");
    let sandbox = levels::binary(levels[0], &triple, "cantus-sandbox");
    let bridge = levels::binary(levels[0], &triple, "cantus-mcp"); // r34:mcp
    let ml = levels::binary(levels[0], &triple, "cantus-ml"); // r34:stems
    for exe in programs
        .iter()
        .map(|(_, p)| p)
        .chain([&launcher, &scanner, &sandbox, &bridge, &ml])
    // r34:mcp (bridge) r34:stems (ml)
    {
        if !exe.is_file() {
            eprintln!("{} is not built", exe.display());
            std::process::exit(1);
        }
    }
    crate::bundle(&if build {
        vec![]
    } else {
        vec!["--no-build".to_string()]
    });

    let ws = workspace();
    let inputs = Inputs {
        launcher,
        programs,
        scanner,
        sandbox,
        bridge, // r34:mcp
        // r34:stems — the ML helper and the C++ runtime beside it, in `ml\`: never in the
        // install folder itself, where every program (the MCP bridge an AI app keeps
        // open, a plug-in's library search) would load our copy (integration review).
        beside: std::iter::once((ML_HELPER.to_string(), ml.clone()))
            .chain(
                find_crt()
                    .unwrap_or_else(|e| {
                        eprintln!("{e}");
                        std::process::exit(1);
                    })
                    .into_iter()
                    .map(|(n, p)| (format!("{ML_DIR}/{n}"), p)),
            )
            .collect(),
        bundled: target.join("bundled"),
        plugins: cantus_plugins::desc::ALL
            .iter()
            .map(|d| d.full_name.to_owned())
            .collect(),
        license: ws.join("LICENSE-GPL"),
        copyright: ws.join("LICENSE"),
        readme: ws.join("xtask/installer/README.txt"),
        docs: release_docs(&ws, &target),
        check_metadata: true,
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
            eprintln!("the icon is not built (cargo build --release -p cantus-launch)");
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
            uninstaller: nsis::Uninstaller::Written,
        })
    };
    let out = target.join("package");
    check_programs(&inputs, &out.join("check"));
    if has("--stage-only") {
        let Some(job) = installer.as_ref() else {
            eprintln!("--stage-only writes the uninstaller with NSIS: drop --no-installer");
            std::process::exit(2);
        };
        stage_only(&inputs, &out, job);
        return;
    }
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

/// `package --stage-only`: build, check and lay out the release in `out/stage/` with its
/// uninstaller at the top — every file SignPath is to sign, and nothing else (ADR 0048 §8).
fn stage_only(inputs: &Inputs, out: &Path, job: &InstallerJob) {
    let dir = out.join("stage");
    let staged = stage(inputs, &dir, VERSION);
    let made = write_uninstaller(
        &staged,
        &out.join("uninstaller"),
        &job.makensis,
        Some(&job.icon),
        VERSION,
    );
    let uninstaller = dir.join(UNINSTALLER);
    std::fs::copy(&made, &uninstaller).unwrap();
    let mut bins = staged.binaries();
    bins.push(uninstaller);
    if inputs.check_metadata {
        meta::require(&bins, VERSION);
    }
    eprintln!(
        "staged for signing in {}: {} files",
        dir.display(),
        bins.len()
    );
}

/// `package --from-stage <dir>`: the release from a stage that came back signed — the
/// zips, the installer carrying the signed uninstaller (not one it writes), its stable
/// copy and the sums. Nothing is built or signed here.
fn from_stage(dir: &Path, no_installer: bool) {
    let dir = stage_dir(dir).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let dir = dir.as_path();
    let staged = Staged::from_dir(dir);
    let uninstaller = dir.join(UNINSTALLER);
    let mut bins = staged.binaries();
    let installer = if no_installer {
        None
    } else {
        if !uninstaller.is_file() {
            eprintln!(
                "{} has no {UNINSTALLER} (made by --stage-only)",
                dir.display()
            );
            std::process::exit(1);
        }
        bins.push(uninstaller.clone());
        let Some(makensis) = nsis::makensis() else {
            eprintln!("makensis not found: install NSIS 3 or set CANTUS_MAKENSIS");
            std::process::exit(1);
        };
        let icon = find_icon(&target_dir().join("release")).unwrap_or_else(|| {
            eprintln!("the icon is not built (cargo build --release -p cantus-launch)");
            std::process::exit(1)
        });
        Some(InstallerJob {
            makensis,
            icon,
            uninst_sign: None,
            env: Vec::new(),
            uninstaller: nsis::Uninstaller::Prebuilt(uninstaller),
        })
    };
    meta::require(&bins, VERSION);
    // The stage must have come back signed: every program, plug-in library and the
    // uninstaller (as the local-signer route checks after signing). A file SignPath's
    // artifact configuration missed stops the release here, by name.
    if cfg!(windows) {
        eprintln!("checking the signatures of {} files:", bins.len());
        sign::check(&bins);
    } else {
        eprintln!("--from-stage checks Authenticode signatures on Windows only");
        std::process::exit(2);
    }
    let out = target_dir().join("package");
    let dist = package_staged(&staged, &out, VERSION, installer.as_ref(), None, true);
    let want = dist_names(VERSION, installer.is_some());
    let got: Vec<String> = archive::walk(&dist).into_iter().map(|e| e.name).collect();
    if got != want {
        eprintln!("dist holds {got:?}, expected {want:?}");
        std::process::exit(1);
    }
    eprintln!("release files in {}", dist.display());
}

/// The stage folder as makensis needs it: absolute. makensis resolves a relative `File`
/// path from the script's own folder (`target/package/nsis/`), not from where xtask ran,
/// so `--from-stage target/package/stage-signed` would name files that are not there.
pub fn stage_dir(dir: &Path) -> Result<PathBuf, String> {
    if !dir.is_dir() {
        return Err(format!(
            "{} is not a folder (the signed stage)",
            dir.display()
        ));
    }
    std::path::absolute(dir).map_err(|e| format!("{}: {e}", dir.display()))
}

/// The version `sign-file` requires of the uninstaller NSIS hands it: the one the job is
/// building (`CANTUS_SIGN_VERSION`, which [`package_staged`] gives makensis), else this
/// xtask's own — the release's.
pub fn sign_version(from_env: Option<String>) -> String {
    from_env
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| VERSION.to_owned())
}

/// `sums <dir>`: write `SHA256SUMS.txt` over every other file of `dir` (after a signed
/// installer replaced the one the sums were made for).
pub fn sums(args: &[String]) {
    let Some(dir) = args.first() else {
        eprintln!("usage: xtask sums <folder>");
        std::process::exit(2);
    };
    write_sums(Path::new(dir));
    eprintln!(
        "{} written",
        Path::new(dir).join(release::SUMS_NAME).display()
    );
}

/// `sign-file <path>`: sign one file with the signer in the environment (NSIS calls this
/// for the uninstaller it writes).
pub fn sign_file(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("usage: xtask sign-file <path>");
        std::process::exit(2);
    };
    // The uninstaller NSIS just wrote: it must name its product and version like every
    // signed file (ADR 0048 §9; NSIS gives it the installer's version resource).
    let version = sign_version(std::env::var("CANTUS_SIGN_VERSION").ok());
    meta::require(&[PathBuf::from(path)], &version);
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

#[cfg(test)]
#[path = "package_ci_tests.rs"]
mod ci_tests;
#[cfg(test)] // r34:stems
#[path = "package_ml_tests.rs"] // r34:stems
mod ml_tests; // r34:stems

#[path = "package_meta.rs"]
pub mod meta;
