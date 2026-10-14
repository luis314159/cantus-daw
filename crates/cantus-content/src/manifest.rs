//! The content manifest: which packs exist, where they come from, what they weigh and
//! what they hash to (ADR 0030). The manifest is small and lives in the repository; the
//! samples never do.

use serde::{Deserialize, Serialize};

/// The manifest format this build reads. A higher number is a manifest written for a
/// newer Cantus: refused with a clear message instead of half-understood.
///
/// - 1: CC0 packs (round 20, ADR 0030).
/// - 2: a CC BY pack's credit line, [`Pack::attribution`] (round 26, ADR 0042). A
///   manifest that carries one must say format 2, so a format-1 build refuses it by name
///   instead of failing on a field it does not know.
pub const FORMAT: u32 = 2;

/// The first format that may carry a credit line.
pub const FORMAT_ATTRIBUTION: u32 = 2;

/// The licences content may be under (ADR 0042 §1): CC0 1.0 and CC BY 3.0 / 4.0.
/// Everything else — NC, ND, SA, retired Creative Commons licences, any other id — is
/// refused by the manifest's validation.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Licence {
    Cc0,
    CcBy3,
    CcBy4,
}

impl Licence {
    /// The SPDX ids accepted, exactly as the manifest spells them.
    pub const ACCEPTED: [&'static str; 3] = ["CC0-1.0", "CC-BY-3.0", "CC-BY-4.0"];

    /// The licence an SPDX id names, if content may be under it. Case and spelling are
    /// exact: `cc-by-4.0` or `CC BY 4.0` is not an id.
    pub fn from_spdx(id: &str) -> Option<Licence> {
        match id {
            "CC0-1.0" => Some(Licence::Cc0),
            "CC-BY-3.0" => Some(Licence::CcBy3),
            "CC-BY-4.0" => Some(Licence::CcBy4),
            _ => None,
        }
    }

    pub fn spdx(self) -> &'static str {
        match self {
            Licence::Cc0 => "CC0-1.0",
            Licence::CcBy3 => "CC-BY-3.0",
            Licence::CcBy4 => "CC-BY-4.0",
        }
    }

    /// The name people read (`CC BY 4.0`): a proper name, the same in every language.
    pub fn name(self) -> &'static str {
        match self {
            Licence::Cc0 => "CC0 1.0",
            Licence::CcBy3 => "CC BY 3.0",
            Licence::CcBy4 => "CC BY 4.0",
        }
    }

    /// Whether the licence's one condition is credit to the author (CC BY).
    pub fn needs_credit(self) -> bool {
        !matches!(self, Licence::Cc0)
    }

    /// Whether `url` is this licence's own text: [`Licence::url_prefix`] followed by
    /// nothing, `legalcode`, `legalcode.<lang>` or `deed.<lang>` (with an optional
    /// trailing `/`). No `..`, query, fragment or percent-escape, so a path that merely
    /// starts like the licence (`by/4.0/../../by-nc/4.0/`) is refused; jurisdiction ports
    /// (`by/3.0/us/`) are other licences and are refused too.
    pub fn is_text_url(self, url: &str) -> bool {
        if url.contains("..") || url.contains(['?', '#', '%']) {
            return false;
        }
        let prefix = self.url_prefix();
        let rest = match url.strip_prefix(prefix) {
            Some(rest) => rest,
            None if url == prefix.trim_end_matches('/') => "",
            None => return false,
        };
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        let lang = |l: &str| {
            (2..=10).contains(&l.len())
                && l.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        };
        match rest {
            "" | "legalcode" => true,
            r => r
                .strip_prefix("legalcode.")
                .or_else(|| r.strip_prefix("deed."))
                .is_some_and(lang),
        }
    }

    /// What the licence's text URL starts with (`…/licenses/by/4.0/`); the full rule is
    /// [`Licence::is_text_url`].
    pub fn url_prefix(self) -> &'static str {
        match self {
            Licence::Cc0 => "https://creativecommons.org/publicdomain/zero/1.0/",
            Licence::CcBy3 => "https://creativecommons.org/licenses/by/3.0/",
            Licence::CcBy4 => "https://creativecommons.org/licenses/by/4.0/",
        }
    }
}

/// A CC BY pack's credit, as its licensor asks to be credited (ADR 0042 §3): title,
/// author and link, with the licence from the pack. Shown in *Sound packs…*, in *About ▸
/// Credits*, in the pack's receipt and in `THIRD-PARTY-NOTICES.md`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// No `deny_unknown_fields`: receipts embed it and have no format gate, so a field a newer
// Cantus adds must never make an older one treat an installed pack as missing. The
// manifest's own format number guards what a manifest may carry.
pub struct Attribution {
    /// The work's title, as the licensor names it.
    pub title: String,
    /// Who is credited, as the licensor asks to be named.
    pub author: String,
    /// The work's page: the link the credit carries.
    pub url: String,
    /// The credit line word for word, when the licensor gives one; empty composes it
    /// from the title, author, link and licence ([`Attribution::line`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub line: String,
    /// The author's own words about crediting the samples in music made with them,
    /// quoted as they wrote them (the pack's page shows them, ADR 0042 §4).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

impl Attribution {
    /// The credit line under `licence` (whose text is at `licence_url`): the licensor's
    /// own words when given, otherwise title, author, link and licence in Creative
    /// Commons' recommended order.
    pub fn line(&self, licence: &str, licence_url: &str) -> String {
        if !self.line.trim().is_empty() {
            return self.line.clone();
        }
        let kind = Licence::from_spdx(licence);
        let name: &str = match kind {
            Some(l) => l.name(),
            None => licence,
        };
        if kind == Some(Licence::Cc0) {
            // CC0 is a dedication to the public domain, not a licence.
            return format!(
                "\u{201c}{}\u{201d} by {} ({}), dedicated to the public domain under {name} \
                 ({licence_url})",
                self.title, self.author, self.url
            );
        }
        format!(
            "\u{201c}{}\u{201d} by {} ({}), licensed under {name} ({licence_url})",
            self.title, self.author, self.url
        )
    }

    /// The credit in full (ADR 0042 §3: author, title, licence, link): [`Attribution::line`],
    /// and — when the licensor's own line leaves the licence's link out — the licence's
    /// name and link after it (CC BY 4.0 §3(a)(1)(C) asks for a link to the licence).
    pub fn full(&self, licence: &str, licence_url: &str) -> String {
        let line = self.line(licence, licence_url);
        if self.line.trim().is_empty() || self.line.contains(licence_url) {
            return line;
        }
        let name: &str = match Licence::from_spdx(licence) {
            Some(l) => l.name(),
            None => licence,
        };
        format!("{line} \u{2014} {name}: {licence_url}")
    }
}

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
// No `deny_unknown_fields`: receipts embed it (see [`Attribution`]).
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
    /// The credit line a CC BY pack needs (ADR 0042); absent for CC0. Format 2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribution: Option<Attribution>,
    /// Instruments inside the pack that the instrument picker offers once it is installed
    /// (round 26). Format 2.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub instruments: Vec<PackInstrument>,
}

/// The picker's shelf a pack instrument goes on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shelf {
    Keys,
    Drums,
    Bass,
    Synths,
    #[default]
    Samplers,
    /// Also any shelf a newer Cantus names: a receipt it wrote must still load here.
    #[serde(other)]
    Other,
}

/// A playable instrument inside a pack: an SFZ file Exemplar loads (round 26).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PackInstrument {
    /// `[a-z0-9-]+`, unique within the pack: with the pack's id, the picker's stable key.
    pub id: String,
    pub name: Text,
    /// The instrument file, relative to the pack's folder (`.sfz`).
    pub file: String,
    #[serde(default)]
    pub shelf: Shelf,
    /// Keep at most this many velocity layers per key range, the others merged into
    /// their neighbours: the memory a sampler that holds every sample needs, halved for
    /// half the layers. Absent keeps them all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layers: Option<u8>,
    /// The instrument's gain in dB, so it plays at the level the native instruments are
    /// calibrated to (ADR 0039).
    #[serde(default)]
    pub gain_db: f32,
}

/// Whether `id` is a pack's or an instrument's id: lowercase letters, digits and `-`.
fn is_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// Individual files at pinned upstream URLs: what a curated subset of a large
    /// library is made of.
    Files(Vec<FileEntry>),
    /// One zip archive, of which only the entries under `include` are kept.
    Zip(ZipSource),
    /// One `.tar.gz` archive, with the same rules as a zip (manifest format 2).
    TarGz(ZipSource),
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
            Source::Zip(z) | Source::TarGz(z) => z.size,
        }
    }

    /// The pack's licence, if it is one content may be under (ADR 0042); a validated
    /// manifest has one for every pack.
    pub fn licence_kind(&self) -> Option<Licence> {
        Licence::from_spdx(&self.licence)
    }

    /// Whether the licence asks for credit (CC BY).
    pub fn needs_credit(&self) -> bool {
        self.licence_kind().is_some_and(Licence::needs_credit)
    }

    /// The credit line, for a pack that carries one (every CC BY pack does; a CC0 pack
    /// may, as a courtesy its author asked for).
    pub fn credit_line(&self) -> Option<String> {
        self.attribution
            .as_ref()
            .map(|a| a.line(&self.licence, &self.licence_url))
    }

    /// The credit in full, with the licence's link ([`Attribution::full`]).
    pub fn credit_full(&self) -> Option<String> {
        self.attribution
            .as_ref()
            .map(|a| a.full(&self.licence, &self.licence_url))
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
            if !is_id(&p.id) {
                return Err(bad(
                    "the id must be lowercase letters, digits and '-'".into()
                ));
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
            let Some(licence) = p.licence_kind() else {
                return Err(bad(format!(
                    "licence `{}` is not accepted (ADR 0042: {})",
                    p.licence,
                    Licence::ACCEPTED.join(", ")
                )));
            };
            if !licence.is_text_url(&p.licence_url) {
                return Err(bad(format!(
                    "the licence URL is not {}'s text ({}…)",
                    licence.name(),
                    licence.url_prefix()
                )));
            }
            if let Some(a) = &p.attribution {
                if self.format < FORMAT_ATTRIBUTION {
                    return Err(bad(format!(
                        "a credit line needs manifest format {FORMAT_ATTRIBUTION}"
                    )));
                }
                if a.title.trim().is_empty() || a.author.trim().is_empty() {
                    return Err(bad("the credit line needs the title and the author".into()));
                }
                if !a.url.starts_with("https://") {
                    return Err(bad("the credit line needs an https link".into()));
                }
                if !a.line.is_empty() && a.line.trim().is_empty() {
                    return Err(bad("the licensor's credit line is blank".into()));
                }
            } else if licence.needs_credit() {
                return Err(bad(format!(
                    "a {} pack needs its credit line (title, author, link)",
                    licence.name()
                )));
            }
            if p.credits.is_empty() {
                return Err(bad("no credit: say where the material comes from".into()));
            }
            check_path(&p.licence_file).map_err(|e| bad(format!("licence_file: {e}")))?;
            if !p.instruments.is_empty() && self.format < FORMAT_ATTRIBUTION {
                return Err(bad(format!(
                    "instruments need manifest format {FORMAT_ATTRIBUTION}"
                )));
            }
            let mut inst_ids = std::collections::HashSet::new();
            for i in &p.instruments {
                let bad_inst = |why: &str| bad(format!("instrument {}: {why}", i.id));
                if !is_id(&i.id) || !inst_ids.insert(i.id.as_str()) {
                    return Err(bad_inst(
                        "the id must be unique, lowercase letters, digits and '-'",
                    ));
                }
                if i.name.en.trim().is_empty() {
                    return Err(bad_inst("no name"));
                }
                check_path(&i.file).map_err(|e| bad_inst(&format!("file: {e}")))?;
                if !i.file.to_ascii_lowercase().ends_with(".sfz") {
                    return Err(bad_inst("the file must be an .sfz instrument"));
                }
                if i.layers == Some(0) {
                    return Err(bad_inst("layers must be at least 1"));
                }
                if !i.gain_db.is_finite() || i.gain_db.abs() > 24.0 {
                    return Err(bad_inst("the gain must be within ±24 dB"));
                }
                if let Source::Files(files) = &p.source {
                    if !files.iter().any(|f| f.to.eq_ignore_ascii_case(&i.file)) {
                        return Err(bad_inst("its file is not among the pack's files"));
                    }
                }
            }
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
                Source::Zip(z) | Source::TarGz(z) => {
                    if matches!(p.source, Source::TarGz(_)) && self.format < FORMAT_ATTRIBUTION {
                        return Err(bad(format!(
                            "a tar.gz source needs manifest format {FORMAT_ATTRIBUTION}"
                        )));
                    }
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

/// A path inside a pack: relative, forward slashes, nothing that climbs out, and a name
/// Windows can hold as written — no component ending in a dot or a space (Windows drops
/// them, so `a.` and `a` are one file) and no device name (`CON`, `NUL`, `COM1`…, with or
/// without an extension).
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
    if p.split('/').any(|c| c.ends_with('.') || c.ends_with(' ')) {
        return Err("no component ending in a dot or a space".into());
    }
    if p.split('/').any(is_device) {
        return Err("no Windows device names (CON, NUL, COM1…)".into());
    }
    if p.split('/').any(is_short_name) {
        return Err("no 8.3 short names (LICENC~1.TXT): Windows maps them onto long ones".into());
    }
    if p.split('/').count() > MAX_DEPTH || p.len() > MAX_PATH {
        return Err(format!("at most {MAX_DEPTH} levels and {MAX_PATH} bytes"));
    }
    Ok(())
}

/// The deepest a pack's path may go, and its longest: real packs are far below both, and
/// a hostile archive's chain of thousands of folders is refused before it is made.
pub const MAX_DEPTH: usize = 32;
pub const MAX_PATH: usize = 400;

/// Whether a component has the shape of a Windows 8.3 short name — one to six characters,
/// `~`, digits, and at most a three-letter extension (`LICENC~1.TXT`) — which Windows can
/// resolve to another, long name: a pack never needs one.
fn is_short_name(c: &str) -> bool {
    let (base, ext) = c.split_once('.').unwrap_or((c, ""));
    let Some((head, digits)) = base.rsplit_once('~') else {
        return false;
    };
    (1..=6).contains(&head.chars().count())
        && !head.contains('~')
        && !digits.is_empty()
        && digits.chars().all(|d| d.is_ascii_digit())
        && head.chars().count() + 1 + digits.len() <= 8
        && ext.chars().count() <= 3
        && !ext.contains('.')
}

/// Whether a path component is a Windows device name, whatever its extension.
fn is_device(c: &str) -> bool {
    let stem = c
        .split('.')
        .next()
        .unwrap_or(c)
        .trim_end()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
}

/// The file a path is on Windows: compared case-insensitively, so two entries that fold
/// to the same key would overwrite each other there.
pub fn file_key(p: &str) -> String {
    p.split('/')
        .map(|c| c.trim_end_matches(['.', ' ']).to_lowercase())
        .collect::<Vec<_>>()
        .join("/")
}

fn check_sha(s: &str) -> Result<(), String> {
    if s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
    {
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
