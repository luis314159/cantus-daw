//! FlexASIO (Etienne Dechamps, MIT): a free, open-source ASIO driver over WASAPI, for
//! machines without an interface of their own. Cantus never ships a copy: the installer
//! (and the app's audio setup) fetch the **latest** release from
//! GitHub when the user asks, check what arrived, and run it with elevation.
//!
//! ASIO4ALL is deliberately not here: its licence forbids redistribution, so Cantus only
//! links to its page.
//!
//! What is checked, since FlexASIO's assets carry no GitHub digest and its sigstore
//! bundle needs a verifier Cantus does not ship: the asset is the release's
//! `FlexASIO-<version>.exe`; the file is a Windows program (`MZ`), not a web page (a
//! proxy's quarantine page is detected, never run); and it has exactly the size GitHub
//! reported. A digest is checked too whenever GitHub provides one.

use crate::release::{looks_like_html, Asset, ParseError, Release};
use crate::verify::{self, VerifyError};
use std::io::Read;
use std::path::Path;

pub const API: &str = "https://api.github.com/repos/dechamps/FlexASIO/releases/latest";
pub const PAGE: &str = "https://github.com/dechamps/FlexASIO";

/// Read the API's answer for FlexASIO (its tags, `flexasio-1.10b`, are not SemVer).
pub fn parse(bytes: &[u8]) -> Result<Release, ParseError> {
    if looks_like_html(bytes) {
        return Err(ParseError::Html);
    }
    serde_json::from_slice(bytes).map_err(|e| ParseError::NotARelease(e.to_string()))
}

/// The installer among the release's files: `FlexASIO-<version>.exe` (not its
/// `.sigstore` bundle, not anything else).
pub fn installer(r: &Release) -> Option<&Asset> {
    r.assets.iter().find(|a| {
        let n = a.name.to_ascii_lowercase();
        n.starts_with("flexasio-") && n.ends_with(".exe")
    })
}

/// The downloaded file is the installer GitHub described.
pub fn check(path: &Path, asset: &Asset) -> Result<(), VerifyError> {
    let meta = std::fs::metadata(path).map_err(|e| VerifyError::Io(e.to_string()))?;
    if meta.len() == 0 {
        return Err(VerifyError::Empty);
    }
    let mut head = vec![0u8; 1024.min(meta.len() as usize)];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .map_err(|e| VerifyError::Io(e.to_string()))?;
    if looks_like_html(&head) || !head.starts_with(b"MZ") {
        return Err(VerifyError::Html);
    }
    if asset.size > 0 && meta.len() != asset.size {
        return Err(VerifyError::Size {
            expected: asset.size,
            got: meta.len(),
        });
    }
    if let Some(h) = asset.sha256() {
        let got = verify::sha256_file(path).map_err(|e| VerifyError::Io(e.to_string()))?;
        if got != h {
            return Err(VerifyError::Hash { expected: h, got });
        }
    }
    Ok(())
}

/// Run the FlexASIO installer elevated (Windows asks the user through UAC) and wait.
/// The path travels in an environment variable, never inside the command text.
#[cfg(windows)]
pub fn run_elevated(installer: &Path) -> std::io::Result<bool> {
    let script = "$p = Start-Process -FilePath $env:CANTUS_FLEXASIO -Verb RunAs -Wait -PassThru; \
                  exit $p.ExitCode";
    let status = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .env("CANTUS_FLEXASIO", installer)
        .env_remove("PSModulePath")
        .status()?;
    Ok(status.success())
}

#[cfg(not(windows))]
pub fn run_elevated(_installer: &Path) -> std::io::Result<bool> {
    Ok(false)
}
