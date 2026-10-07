//! The Steinberg ASIO SDK and LLVM's libclang, which every released Cantus is built with
//! (ADR 0025 §10, round 21: one Windows edition, WASAPI and ASIO together).
//!
//! ```text
//! cargo run -p xtask -- asio-sdk     # fetch and check the SDK; say what a build will use
//! ```
//!
//! The SDK (GPL-3-compatible since 2025, never committed) is downloaded once into a cache
//! **outside the repository** — `%LOCALAPPDATA%\cantus-build-tools\asiosdk-2.3.4\` — and
//! its zip must match the SHA-256 pinned here, so a proxy's quarantine page or a changed
//! file stops the build instead of being compiled in. `CANTUS_ASIO_SDK_ZIP=<path>` uses a
//! zip already on disk (checked the same way); `CPAL_ASIO_DIR` points at an SDK folder
//! directly. The patched `asio-sys` (third_party/) looks in the same cache, so after this
//! task a plain `cargo build -p cantus` has ASIO too.

use std::path::{Path, PathBuf};
use std::process::Command;

/// What Steinberg's `https://www.steinberg.net/asiosdk` redirects to (round 19).
pub const SDK_URL: &str =
    "https://download.steinberg.net/sdk_downloads/ASIO-SDK_2.3.4_2025-10-15.zip";
/// The SHA-256 of that zip (round 19's download; the release workflow pins the same).
pub const SDK_SHA256: &str = "d5ebf0c20dd2c5f43771fd0c1418f4b361bf52434ee670097cfa6b3a335e2eca";
/// The cache folder under `%LOCALAPPDATA%\cantus-build-tools`, and the SDK inside it —
/// the same place `third_party/asio-sys/build.rs` looks (`CANTUS_SDK_CACHE`).
pub const CACHE_DIR: &str = "asiosdk-2.3.4";
pub const SDK_DIR: &str = "ASIOSDK";

/// The SDK and libclang a build uses.
#[derive(Clone, Debug)]
pub struct AsioTools {
    pub sdk: PathBuf,
    pub libclang: PathBuf,
}

impl AsioTools {
    /// The environment for `cargo build`: the SDK, libclang, and a hard error instead of a
    /// build without ASIO.
    pub fn env(&self) -> Vec<(String, String)> {
        vec![
            ("CPAL_ASIO_DIR".into(), self.sdk.display().to_string()),
            ("LIBCLANG_PATH".into(), self.libclang.display().to_string()),
            ("ASIO_SYS_REQUIRE_SDK".into(), "1".into()),
        ]
    }
}

/// `%LOCALAPPDATA%\cantus-build-tools`.
pub fn tools_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("cantus-build-tools"))
}

/// Whether a folder holds the SDK (what `asio-sys` needs from it).
pub fn is_sdk(dir: &Path) -> bool {
    dir.join("common").join("asio.h").is_file() && dir.join("host").join("asiodrivers.h").is_file()
}

/// The folder holding the SDK somewhere under `root` (the zip has one top folder; its
/// name has changed between releases), at most three levels down.
pub fn find_sdk_in(root: &Path) -> Option<PathBuf> {
    fn go(dir: &Path, depth: usize) -> Option<PathBuf> {
        if is_sdk(dir) {
            return Some(dir.to_path_buf());
        }
        if depth == 0 {
            return None;
        }
        let mut subs: Vec<PathBuf> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subs.sort();
        subs.iter().find_map(|d| go(d, depth - 1))
    }
    go(root, 3)
}

/// Why a downloaded SDK zip is refused, or `None` when its hash is the pinned one.
pub fn refuse_zip(bytes: &[u8]) -> Option<String> {
    if cantus_update::release::looks_like_html(bytes) {
        return Some(
            "the download is a web page (a proxy holding it for scanning?): try again in a few minutes"
                .into(),
        );
    }
    let hex = cantus_update::verify::sha256_hex(bytes);
    (hex != SDK_SHA256).then(|| {
        format!(
            "the ASIO SDK zip's SHA-256 is {hex}, not the pinned {SDK_SHA256}: if Steinberg \
             published a new file, check it and update SDK_SHA256 (xtask/src/asio_sdk.rs) and \
             ASIO_SHA256 (.github/workflows/release.yml)"
        )
    })
}

/// Unzip `zip` into `dest` (created afresh) and return the SDK folder inside, renamed to
/// `dest/ASIOSDK`.
pub fn unpack(zip_path: &Path, dest: &Path) -> Result<PathBuf, String> {
    let _ = std::fs::remove_dir_all(dest);
    std::fs::create_dir_all(dest).map_err(|e| format!("create {}: {e}", dest.display()))?;
    let staging = dest.join("unzip");
    let f = std::fs::File::open(zip_path).map_err(|e| format!("{}: {e}", zip_path.display()))?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("{}: {e}", zip_path.display()))?;
    z.extract(&staging)
        .map_err(|e| format!("unzip {}: {e}", zip_path.display()))?;
    let found = find_sdk_in(&staging)
        .ok_or_else(|| format!("{} holds no ASIO SDK (common\\asio.h)", zip_path.display()))?;
    let sdk = dest.join(SDK_DIR);
    std::fs::rename(&found, &sdk).map_err(|e| format!("move the SDK: {e}"))?;
    let _ = std::fs::remove_dir_all(&staging);
    Ok(sdk)
}

/// Download the SDK zip to `to` the way the browser would: Windows PowerShell's
/// `Invoke-WebRequest` (the system's proxy settings, PAC included, and trust store), then
/// Windows' `curl.exe` if that fails. Steinberg's CDN answered `curl.exe` with 403 once
/// here while PowerShell got the file; the hash check decides either way.
fn download(to: &Path) -> Result<(), String> {
    eprintln!("downloading {SDK_URL}");
    let ps = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command"])
        .arg(format!(
            "$ProgressPreference='SilentlyContinue'; Invoke-WebRequest -UseBasicParsing -Uri '{SDK_URL}' -OutFile '{}'",
            to.display()
        ))
        // A module path inherited from PowerShell 7 breaks Windows PowerShell.
        .env_remove("PSModulePath")
        .status();
    if ps.is_ok_and(|s| s.success()) && to.is_file() {
        return Ok(());
    }
    eprintln!("PowerShell could not download it; trying curl.exe");
    let status = Command::new("curl.exe")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--retry",
            "2",
            "--output",
        ])
        .arg(to)
        .arg(SDK_URL)
        .status()
        .map_err(|e| format!("curl.exe: {e}"))?;
    if !status.success() {
        return Err(format!(
            "downloading {SDK_URL} failed ({status}); download it by hand and run again with \
             CANTUS_ASIO_SDK_ZIP=<the zip>"
        ));
    }
    Ok(())
}

/// The SDK: `CPAL_ASIO_DIR`, else the cache, else fetched into the cache (from
/// `CANTUS_ASIO_SDK_ZIP` or Steinberg) and checked.
pub fn sdk() -> Result<PathBuf, String> {
    if let Some(dir) = std::env::var_os("CPAL_ASIO_DIR").map(PathBuf::from) {
        return if is_sdk(&dir) {
            Ok(dir)
        } else {
            Err(format!(
                "CPAL_ASIO_DIR={} does not hold the ASIO SDK",
                dir.display()
            ))
        };
    }
    let root = tools_root().ok_or("LOCALAPPDATA is not set")?;
    let cache = root.join(CACHE_DIR);
    let sdk = cache.join(SDK_DIR);
    if is_sdk(&sdk) {
        return Ok(sdk);
    }
    std::fs::create_dir_all(&root).map_err(|e| format!("create {}: {e}", root.display()))?;
    let zip = root.join(format!("{CACHE_DIR}.zip"));
    let given = std::env::var_os("CANTUS_ASIO_SDK_ZIP").map(PathBuf::from);
    match &given {
        Some(src) => {
            std::fs::copy(src, &zip).map_err(|e| format!("copy {}: {e}", src.display()))?;
        }
        None if !zip.is_file() => download(&zip)?,
        None => {}
    }
    let bytes = std::fs::read(&zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    if let Some(why) = refuse_zip(&bytes) {
        let _ = std::fs::remove_file(&zip);
        return Err(why);
    }
    let sdk = unpack(&zip, &cache)?;
    eprintln!(
        "ASIO SDK checked (SHA-256 {SDK_SHA256}) and unpacked into {}",
        sdk.display()
    );
    Ok(sdk)
}

/// The folder holding `libclang.dll`: `LIBCLANG_PATH`, else LLVM's default install.
pub fn libclang() -> Result<PathBuf, String> {
    std::env::var_os("LIBCLANG_PATH")
        .map(PathBuf::from)
        .into_iter()
        .chain([PathBuf::from(r"C:\Program Files\LLVM\bin")])
        .find(|d| d.join("libclang.dll").is_file())
        .ok_or_else(|| {
            "LLVM's libclang.dll was not found: install LLVM (winget install LLVM.LLVM) or set \
             LIBCLANG_PATH"
                .to_string()
        })
}

/// Everything an ASIO build needs, fetched and checked.
pub fn ensure() -> Result<AsioTools, String> {
    Ok(AsioTools {
        sdk: sdk()?,
        libclang: libclang()?,
    })
}

/// Whether `cantus --audio-drivers`' answer lists the ASIO driver type
/// (`audio drivers: WASAPI, ASIO`).
pub fn lists_asio(answer: &str) -> bool {
    answer.lines().any(|l| {
        l.trim().strip_prefix("audio drivers:").is_some_and(|rest| {
            rest.split(',')
                .any(|d| d.trim().eq_ignore_ascii_case("ASIO"))
        })
    })
}

/// `xtask asio-sdk`: fetch and check, then say what the next build uses.
pub fn command() {
    match ensure() {
        Ok(t) => {
            eprintln!("ASIO SDK: {}", t.sdk.display());
            eprintln!("libclang: {}", t.libclang.display());
            // A build made before the SDK was here used asio-sys's stubs: rebuild it.
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let _ = Command::new(cargo)
                .current_dir(crate::workspace())
                .args(["clean", "-p", "asio-sys"])
                .status();
            eprintln!("`cargo build -p cantus` now builds Cantus with ASIO and WASAPI.");
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}
