//! A GitHub release, as the REST API's `GET /repos/{owner}/{repo}/releases/latest`
//! answers, and the names of the files a Cantus release carries.
//!
//! The names are made here and only here: `xtask package` names what it builds with these
//! functions, and the app looks for the same names, so the two cannot drift.

use crate::version::Version;
use serde::Deserialize;
use std::collections::HashMap;

/// Which build: the default one, or the one with ASIO drivers (the `asio` feature). An
/// installed Cantus updates to the flavour it is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Flavor {
    Standard,
    Asio,
}

impl Flavor {
    pub const ALL: [Flavor; 2] = [Flavor::Standard, Flavor::Asio];

    fn suffix(self) -> &'static str {
        match self {
            Flavor::Standard => "",
            Flavor::Asio => "-asio",
        }
    }
}

/// `Cantus-0.2.0-windows-x64-setup.exe`, `Cantus-0.2.0-windows-x64-asio-setup.exe`.
pub fn installer_name(version: &str, flavor: Flavor) -> String {
    format!("Cantus-{version}-windows-x64{}-setup.exe", flavor.suffix())
}

/// `Cantus-0.2.0-windows-x64-portable.zip`, `…-asio-portable.zip`.
pub fn portable_name(version: &str, flavor: Flavor) -> String {
    format!(
        "Cantus-{version}-windows-x64{}-portable.zip",
        flavor.suffix()
    )
}

/// The CLAP and VST3 plug-ins alone, for people who use them in another DAW.
pub fn plugins_name(version: &str) -> String {
    format!("Cantus-{version}-plugins-windows-x64.zip")
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

    /// The installer for `flavor`.
    pub fn installer(&self, flavor: Flavor) -> Option<&Asset> {
        self.asset(&installer_name(&self.version_text(), flavor))
    }

    pub fn portable(&self, flavor: Flavor) -> Option<&Asset> {
        self.asset(&portable_name(&self.version_text(), flavor))
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
