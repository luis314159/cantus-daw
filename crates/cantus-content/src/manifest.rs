//! The content manifest: which packs exist, where they come from, what they weigh and
//! what they hash to (ADR 0030). The manifest is small and lives in the repository; the
//! samples never do.

use serde::{Deserialize, Serialize};

/// The manifest format this build reads. A higher number is a manifest written for a
/// newer Cantus: refused with a clear message instead of half-understood.
pub const FORMAT: u32 = 1;

/// The manifest bundled with this build.
pub const BUNDLED: &str = include_str!("../manifest/content.json");

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub format: u32,
    /// When the manifest was last edited (ISO date): a local copy newer than the bundled
    /// one wins.
    pub updated: String,
    pub packs: Vec<Pack>,
}

/// A string in the languages Cantus speaks. Content names are data, not interface
/// strings, so they travel with the manifest rather than in the locale files.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub en: String,
    #[serde(default)]
    pub es: String,
}

impl Text {
    /// The text in `lang` (`"es"`), falling back to English.
    pub fn get(&self, lang: &str) -> &str {
        match lang {
            "es" if !self.es.is_empty() => &self.es,
            _ => &self.en,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Drums,
    Percussion,
    OneShots,
    Instruments,
    Fx,
}

impl Category {
    /// The locale key of the category's name.
    pub fn key(self) -> &'static str {
        match self {
            Category::Drums => "samples.cat.drums",
            Category::Percussion => "samples.cat.percussion",
            Category::OneShots => "samples.cat.one_shots",
            Category::Instruments => "samples.cat.instruments",
            Category::Fx => "samples.cat.fx",
        }
    }
}

/// Who made the material and where its licence was read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credit {
    pub name: String,
    /// The upstream project page.
    pub url: String,
    /// The upstream licence text, at the version the pack takes.
    pub licence_url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    /// Stable id, `[a-z0-9-]+`: the folder the pack installs into.
    pub id: String,
    /// Bumped whenever the files change; an installed pack with a lower version has an
    /// update.
    pub version: u32,
    pub name: Text,
    pub description: Text,
    pub category: Category,
    /// The pack the browser offers in one click when nothing is installed.
    #[serde(default)]
    pub recommended: bool,
    /// SPDX identifier (`CC0-1.0`).
    pub licence: String,
    /// The licence's canonical text.
    pub licence_url: String,
    /// Where the licence sits once installed, relative to the pack's folder.
    pub licence_file: String,
    pub credits: Vec<Credit>,
    /// The date the licence was last read on the upstream pages.
    pub checked: String,
    /// Bytes on disk once installed (an estimate the window shows; never trusted).
    pub installed_size: u64,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Individual files at pinned upstream URLs: what a curated subset of a large
    /// library is made of.
    Files(Vec<FileEntry>),
    /// One zip archive, of which only the entries under `include` are kept.
    Zip(ZipSource),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub url: String,
    /// Where it goes, relative to the pack's folder (forward slashes).
    pub to: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZipSource {
    pub url: String,
    pub size: u64,
    pub sha256: String,
    /// A folder every entry starts with, removed when unpacking (`"afr-kicks-2024/"`).
    #[serde(default)]
    pub strip: String,
    /// Patterns of entries to keep, after `strip` (`"Samples/**"`, `"LICENSE"`); empty
    /// keeps everything.
    #[serde(default)]
    pub include: Vec<String>,
    /// Patterns of entries to leave out even when included (`"Samples/wet/**"`).
    #[serde(default)]
    pub exclude: Vec<String>,
}

impl Pack {
    /// Bytes to download.
    pub fn download_size(&self) -> u64 {
        match &self.source {
            Source::Files(f) => f.iter().map(|f| f.size).sum(),
            Source::Zip(z) => z.size,
        }
    }

    /// Whether the licence lets Cantus offer the pack by default (ADR 0001: original
    /// or CC0). Anything else needs the owner's ruling and stays out of the manifest.
    pub fn is_cc0(&self) -> bool {
        self.licence == "CC0-1.0"
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ManifestError {
    #[error("the manifest is not valid JSON: {0}")]
    Json(String),
    #[error("the manifest is format {0}; this Cantus reads format {FORMAT} — update Cantus")]
    TooNew(u32),
    #[error("pack {pack}: {problem}")]
    Pack { pack: String, problem: String },
}

impl Manifest {
    /// Parse and validate.
    pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
        // The format is read first, so a newer manifest with fields this build does not
        // know is reported as newer rather than as malformed.
        #[derive(Deserialize)]
        struct Head {
            format: u32,
        }
        let head: Head =
            serde_json::from_str(text).map_err(|e| ManifestError::Json(e.to_string()))?;
        if head.format > FORMAT {
            return Err(ManifestError::TooNew(head.format));
        }
        let m: Manifest =
            serde_json::from_str(text).map_err(|e| ManifestError::Json(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    /// The manifest compiled into this build.
    pub fn bundled() -> Manifest {
        Manifest::parse(BUNDLED).expect("the bundled manifest is valid (a test checks it)")
    }

    pub fn pack(&self, id: &str) -> Option<&Pack> {
        self.packs.iter().find(|p| p.id == id)
    }

    /// The pack offered when nothing is installed.
    pub fn recommended(&self) -> Option<&Pack> {
        self.packs.iter().find(|p| p.recommended)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        let mut ids = std::collections::HashSet::new();
        for p in &self.packs {
            let bad = |problem: String| ManifestError::Pack {
                pack: p.id.clone(),
                problem,
            };
            if p.id.is_empty()
                || !p
                    .id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            {
                return Err(bad("the id must be lowercase letters, digits and '-'".into()));
            }
            if !ids.insert(p.id.as_str()) {
                return Err(bad("the id is used twice".into()));
            }
            if p.name.en.trim().is_empty() {
                return Err(bad("no name".into()));
            }
            if p.licence.trim().is_empty() || !p.licence_url.starts_with("https://") {
                return Err(bad("a licence needs an SPDX id and an https URL".into()));
            }
            if p.credits.is_empty() {
                return Err(bad("no credit: say where the material comes from".into()));
            }
            check_path(&p.licence_file).map_err(|e| bad(format!("licence_file: {e}")))?;
            match &p.source {
                Source::Files(files) => {
                    if files.is_empty() {
                        return Err(bad("no files".into()));
                    }
                    let mut seen = std::collections::HashSet::new();
                    for f in files {
                        check_url(&f.url).map_err(|e| bad(format!("{}: {e}", f.to)))?;
                        check_path(&f.to).map_err(|e| bad(format!("{}: {e}", f.to)))?;
                        check_sha(&f.sha256).map_err(|e| bad(format!("{}: {e}", f.to)))?;
                        if f.size == 0 {
                            return Err(bad(format!("{}: size 0", f.to)));
                        }
                        if !seen.insert(f.to.to_ascii_lowercase()) {
                            return Err(bad(format!("{}: two files land there", f.to)));
                        }
                    }
                    if !seen.contains(&p.licence_file.to_ascii_lowercase()) {
                        return Err(bad("the licence file is not among the files".into()));
                    }
                }
                Source::Zip(z) => {
                    check_url(&z.url).map_err(|e| bad(format!("url: {e}")))?;
                    check_sha(&z.sha256).map_err(|e| bad(format!("sha256: {e}")))?;
                    if z.size == 0 {
                        return Err(bad("size 0".into()));
                    }
                }
            }
        }
        Ok(())
    }
}

/// A path inside a pack: relative, forward slashes, nothing that climbs out.
pub fn check_path(p: &str) -> Result<(), String> {
    if p.is_empty() {
        return Err("empty path".into());
    }
    if p.starts_with('/') || p.contains('\\') || p.contains(':') {
        return Err("must be relative, with '/'".into());
    }
    if p.split('/').any(|c| c.is_empty() || c == "." || c == "..") {
        return Err("no empty, '.' or '..' components".into());
    }
    Ok(())
}

fn check_sha(s: &str) -> Result<(), String> {
    if s.len() == 64 && s.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)) {
        Ok(())
    } else {
        Err("a SHA-256 is 64 lowercase hex digits".into())
    }
}

/// `https://` everywhere; `file://` and loopback `http://` for tests and offline
/// installers.
fn check_url(u: &str) -> Result<(), String> {
    let loopback = u.starts_with("http://127.0.0.1") || u.starts_with("http://localhost");
    if u.starts_with("https://") || u.starts_with("file://") || loopback {
        Ok(())
    } else {
        Err(format!("{u}: only https:// (or file://) is allowed"))
    }
}
