//! A GitHub release, as the REST API's `GET /repos/{owner}/{repo}/releases/latest`
//! answers, and the names of the files a Cantus release carries.
//!
//! The names are made here and only here: `xtask package` names what it builds with these
//! functions, and the app looks for the same names, so the two cannot drift.

use crate::version::Version;
use serde::Deserialize;
use std::collections::HashMap;

/// `Cantus-0.2.0-windows-x64-setup.exe`. There is one Windows edition, with every driver
/// (WASAPI and ASIO) in it (ADR 0025 §10, round 21): no "-asio" variant any more.
pub fn installer_name(version: &str) -> String {
    format!("Cantus-{version}-windows-x64-setup.exe")
}

/// The installer again, under a name that never changes, published beside the versioned
/// one (ADR 0048 §7): a fixed link — the website's download button,
/// `https://github.com/<repo>/releases/latest/download/Cantus-windows-x64-setup.exe` —
/// then always brings the newest release's installer. The same bytes, listed in
/// `SHA256SUMS.txt` too; the updater keeps asking for [`installer_name`].
pub const STABLE_INSTALLER_NAME: &str = "Cantus-windows-x64-setup.exe";

/// `Cantus-0.2.0-windows-x64-portable.zip`.
pub fn portable_name(version: &str) -> String {
    format!("Cantus-{version}-windows-x64-portable.zip")
}

/// The CLAP and VST3 plug-ins alone, for people who use them in another DAW.
pub fn plugins_name(version: &str) -> String {
    format!("Cantus-{version}-plugins-windows-x64.zip")
}

/// `Cantus-0.2.0-linux-x86_64.AppImage`: Linux's one file (ADR 0067), what
/// `packaging/linux/build-appimage.sh` writes and what an AppImage's updater asks for.
pub fn appimage_name(version: &str) -> String {
    format!("Cantus-{version}-linux-x86_64.AppImage")
}

/// `Cantus-0.2.0-macos-universal.dmg`: one disk image for Apple silicon and Intel (both
/// architectures `lipo`ed into one `Cantus.app`), what `packaging/macos/build-dmg.sh`
/// writes and what a Mac's updater asks for.
pub fn dmg_name(version: &str) -> String {
    format!("Cantus-{version}-macos-universal.dmg")
}

/// The AppImage under the name that never changes (as [`STABLE_INSTALLER_NAME`]).
pub const STABLE_APPIMAGE_NAME: &str = "Cantus-linux-x86_64.AppImage";
/// The disk image under the name that never changes (as [`STABLE_INSTALLER_NAME`]).
pub const STABLE_DMG_NAME: &str = "Cantus-macos-universal.dmg";

/// The systems a release carries a file for. Anything else (Linux on ARM, a native
/// Windows-on-ARM build, a BSD) has no file of its own: the updater then only points at
/// the release's page.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// The NSIS installer (Windows on ARM runs it emulated, as it runs the x64 program).
    WindowsX64,
    /// The AppImage.
    LinuxX64,
    /// The disk image, for both Mac architectures.
    MacUniversal,
}

impl Target {
    /// Every target, in the order the release lists them.
    pub const ALL: [Target; 3] = [Target::WindowsX64, Target::LinuxX64, Target::MacUniversal];

    /// The target for an operating system and architecture as Rust names them
    /// (`std::env::consts::{OS, ARCH}`).
    pub fn of(os: &str, arch: &str) -> Option<Target> {
        match (os, arch) {
            ("windows", "x86_64") => Some(Target::WindowsX64),
            ("linux", "x86_64") => Some(Target::LinuxX64),
            ("macos", "x86_64" | "aarch64") => Some(Target::MacUniversal),
            _ => None,
        }
    }

    /// This build's target.
    pub fn current() -> Option<Target> {
        Target::of(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// The versioned file name: what the updater asks for.
    pub fn file_name(self, version: &str) -> String {
        match self {
            Target::WindowsX64 => installer_name(version),
            Target::LinuxX64 => appimage_name(version),
            Target::MacUniversal => dmg_name(version),
        }
    }

    /// The stable name: what the website links through `releases/latest/download/`.
    pub fn stable_name(self) -> &'static str {
        match self {
            Target::WindowsX64 => STABLE_INSTALLER_NAME,
            Target::LinuxX64 => STABLE_APPIMAGE_NAME,
            Target::MacUniversal => STABLE_DMG_NAME,
        }
    }
}

/// One line per file: `<sha-256 hex>  <name>`, the format `sha256sum -c` reads.
pub const SUMS_NAME: &str = "SHA256SUMS.txt";

/// One file attached to a release.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(default)]
    pub size: u64,
    pub browser_download_url: String,
    /// GitHub's own digest of the uploaded file, `sha256:<hex>` (on assets since 2025;
    /// `null` on older ones).
    #[serde(default)]
    pub digest: Option<String>,
}

impl Asset {
    /// The SHA-256 GitHub computed when the file was uploaded, lowercase hex.
    pub fn sha256(&self) -> Option<String> {
        let hex = self.digest.as_deref()?.strip_prefix("sha256:")?;
        is_sha256_hex(hex).then(|| hex.to_ascii_lowercase())
    }
}

/// A release, as much of it as the updater reads.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Release {
    pub tag_name: String,
    #[serde(default)]
    pub name: Option<String>,
    /// The release notes, Markdown.
    #[serde(default)]
    pub body: Option<String>,
    pub html_url: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<Asset>,
}

/// Why an answer is not a release we can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// The answer is a web page: a proxy's block or quarantine page, a captive portal.
    Html,
    /// JSON, but not a release (or not JSON at all).
    NotARelease(String),
    /// A release whose tag is not a version.
    BadTag(String),
}

/// Whether bytes look like an HTML page rather than what was asked for. A proxy that holds
/// a download for scanning answers `200 OK` with its own page, so the status alone says
/// nothing (ADR 0032 §6).
pub fn looks_like_html(bytes: &[u8]) -> bool {
    let head = &bytes[..bytes.len().min(1024)];
    let text = String::from_utf8_lossy(head).to_ascii_lowercase();
    let t = text.trim_start_matches('\u{feff}').trim_start();
    t.starts_with("<!doctype html")
        || t.starts_with("<html")
        || t.starts_with("<head")
        || (t.starts_with("<!--") && text.contains("<html"))
        || (t.starts_with('<') && text.contains("<title"))
}

impl Release {
    /// Read the API's answer.
    pub fn parse(bytes: &[u8]) -> Result<Release, ParseError> {
        if looks_like_html(bytes) {
            return Err(ParseError::Html);
        }
        let r: Release =
            serde_json::from_slice(bytes).map_err(|e| ParseError::NotARelease(e.to_string()))?;
        if Version::parse(&r.tag_name).is_none() {
            return Err(ParseError::BadTag(r.tag_name));
        }
        Ok(r)
    }

    pub fn version(&self) -> Version {
        Version::parse(&self.tag_name).expect("checked in parse")
    }

    /// The tag without its `v`, as the files are named.
    pub fn version_text(&self) -> String {
        self.version().to_string()
    }

    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }

    /// The installer.
    pub fn installer(&self) -> Option<&Asset> {
        self.asset(&installer_name(&self.version_text()))
    }

    /// The file a copy on `target` updates itself from: the versioned name only (the
    /// stable copy is the same bytes, for the website).
    pub fn update_file(&self, target: Target) -> Option<&Asset> {
        self.asset(&target.file_name(&self.version_text()))
    }

    pub fn portable(&self) -> Option<&Asset> {
        self.asset(&portable_name(&self.version_text()))
    }

    pub fn sums(&self) -> Option<&Asset> {
        self.asset(SUMS_NAME)
    }

    /// Whether this release is newer than `current` and one we offer: not a draft, not a
    /// pre-release (unless the running build is one itself).
    pub fn is_update_for(&self, current: &Version) -> bool {
        let v = self.version();
        if self.draft || ((self.prerelease || v.is_prerelease()) && !current.is_prerelease()) {
            return false;
        }
        v > *current
    }
}

/// The newest release in the API's *list* answer (`GET /repos/{owner}/{repo}/releases`),
/// with the JSON of that one release as it came, to keep for a `304`. Drafts and
/// releases whose tag is not a version are left out; `None` when nothing is left.
///
/// A pre-release build asks for the list because GitHub's `/releases/latest` never
/// names a pre-release: a beta asking it would never hear of the next beta.
pub fn newest(bytes: &[u8]) -> Result<Option<(Release, String)>, ParseError> {
    if looks_like_html(bytes) {
        return Err(ParseError::Html);
    }
    let list: Vec<serde_json::Value> =
        serde_json::from_slice(bytes).map_err(|e| ParseError::NotARelease(e.to_string()))?;
    let mut best: Option<(Release, String)> = None;
    for item in list {
        let Ok(r) = serde_json::from_value::<Release>(item.clone()) else {
            continue;
        };
        if r.draft || Version::parse(&r.tag_name).is_none() {
            continue;
        }
        if best.as_ref().is_none_or(|(b, _)| r.version() > b.version()) {
            best = Some((r, item.to_string()));
        }
    }
    Ok(best)
}

pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Read a `SHA256SUMS` file: `<hex>  <name>` or `<hex> *<name>` per line, `#` comments.
/// Lines that are not that shape are skipped; the names map to lowercase hex.
pub fn parse_sums(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((hex, rest)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        let name = rest.trim_start().trim_start_matches('*').trim();
        if is_sha256_hex(hex) && !name.is_empty() {
            out.insert(name.to_owned(), hex.to_ascii_lowercase());
        }
    }
    out
}

/// Write the lines [`parse_sums`] reads, sorted by name.
pub fn format_sums(entries: &[(String, String)]) -> String {
    let mut sorted: Vec<&(String, String)> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    sorted
        .iter()
        .map(|(name, hex)| format!("{hex}  {name}\n"))
        .collect()
}
