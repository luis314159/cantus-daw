//! A character in a project (ADR 0089 §7): the copy in `Characters/<name>/`, its
//! `character.json` (format, hash, licence, author, the mapping and the person's choices),
//! the loader the screen and a video use, and carrying the folder when a project moves.

use super::imported::Settings;
use super::{inspect_with, paths, Found, ImportError, Limits};
use crate::character::{Character, Licence};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

/// The folder every imported character is copied under, in the project's folder.
pub const FOLDER: &str = "Characters";
/// The file beside the copy that says what it is.
pub const PACKAGE_FILE: &str = "character.json";
/// The files a licence is looked for in, beside a character file (any case).
pub const LICENCE_FILES: [&str; 8] = [
    "license",
    "license.txt",
    "license.md",
    "licence",
    "licence.txt",
    "licence.md",
    "copying",
    "copying.txt",
];

/// `character.json`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Package {
    /// This file's own version.
    pub version: u32,
    /// "Lottie", "Aseprite"…
    pub format: String,
    /// The character file, beside this one.
    pub file: String,
    /// What it was called where it came from.
    pub original: String,
    /// The character file's SHA-256 (hex).
    pub sha256: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    pub licence: StoredLicence,
    pub settings: Settings,
    /// Every other file of the package, relative to its folder (atlases, images, a licence
    /// file) — what its content hash covers with the character file and the choices.
    #[serde(default)]
    pub files: Vec<String>,
    /// The package's content hash (hex): its folder's name ends with its first 8.
    #[serde(default)]
    pub hash: String,
}

/// A licence as `character.json` keeps it.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StoredLicence {
    /// `stated` (the file or a licence file said it, or the person chose it) or `unknown`.
    pub kind: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub source: String,
}

impl StoredLicence {
    pub fn of(l: &Licence) -> StoredLicence {
        match l {
            Licence::Stated { text, source } => StoredLicence {
                kind: "stated".into(),
                text: text.clone(),
                source: source.clone(),
            },
            Licence::Cantus => StoredLicence {
                kind: "stated".into(),
                text: "GPL-3.0-or-later".into(),
                source: "Cantus".into(),
            },
            Licence::Unknown => StoredLicence {
                kind: "unknown".into(),
                text: String::new(),
                source: String::new(),
            },
        }
    }

    pub fn licence(&self) -> Licence {
        if self.kind == "stated" && !self.text.trim().is_empty() {
            Licence::Stated {
                text: self.text.clone(),
                source: self.source.clone(),
            }
        } else {
            Licence::Unknown
        }
    }
}

/// The licences a person can choose on the card when the file states none: (id, the
/// locale key of its name).
pub const CHOICES: [(&str, &str); 6] = [
    ("own-work", "dance_import.licence.own"),
    ("CC0-1.0", "dance_import.licence.cc0"),
    ("CC-BY-4.0", "dance_import.licence.cc_by"),
    ("CC-BY-SA-4.0", "dance_import.licence.cc_by_sa"),
    ("LottieFiles-Simple", "dance_import.licence.lottie_simple"),
    ("other", "dance_import.licence.other"),
];

/// Where a chosen licence says it came from.
pub const CHOSEN: &str = "chosen";

/// What a licence asks of the person, as the card's plain lines (locale keys).
pub fn licence_asks(l: &Licence) -> Vec<&'static str> {
    let Licence::Stated { text, .. } = l else {
        return if *l == Licence::Unknown {
            vec!["dance_import.asks.unknown"]
        } else {
            Vec::new()
        };
    };
    let t = text.to_ascii_uppercase().replace([' ', '_'], "-");
    let mut out = Vec::new();
    let nc = t.contains("-NC") || t.contains("NONCOMMERCIAL") || t.contains("NON-COMMERCIAL");
    let by = t.contains("CC-BY") || t.contains("ATTRIBUTION");
    if by {
        out.push("dance_import.asks.credit");
    }
    if t.contains("-SA") || t.contains("SHAREALIKE") || t.contains("SHARE-ALIKE") {
        out.push("dance_import.asks.share_alike");
    }
    if nc {
        out.push("dance_import.asks.non_commercial");
    }
    if t.contains("-ND") || t.contains("NODERIV") {
        out.push("dance_import.asks.no_derivatives");
    }
    out
}

/// A licence file beside `path` (its first meaningful line, recognised when it is a known
/// licence), if there is one.
pub fn licence_beside(path: &Path) -> Option<Licence> {
    let file = licence_file(path.parent()?)?;
    let bytes = paths::read_bounded(&file, 256 << 10).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let recognised = recognise(&text);
    let first = text
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .chars()
        .take(200)
        .collect::<String>();
    let text = recognised.map(str::to_string).unwrap_or(first);
    if text.is_empty() {
        return None;
    }
    Some(Licence::Stated {
        text,
        source: file.file_name()?.to_string_lossy().into_owned(),
    })
}

/// The licence file in `folder`, if any.
pub fn licence_file(folder: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(folder).ok()?;
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| LICENCE_FILES.contains(&n.to_ascii_lowercase().as_str()))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

/// An SPDX id for a licence text Cantus recognises.
pub fn recognise(text: &str) -> Option<&'static str> {
    let t = text.to_ascii_lowercase();
    let has = |s: &str| t.contains(s);
    Some(
        if has("cc0") || has("creative commons zero") || has("publicdomain/zero") {
            "CC0-1.0"
        } else if has("attribution-noncommercial-sharealike 4.0") || has("by-nc-sa/4.0") {
            "CC-BY-NC-SA-4.0"
        } else if has("attribution-noncommercial 4.0") || has("by-nc/4.0") {
            "CC-BY-NC-4.0"
        } else if has("attribution-sharealike 4.0") || has("by-sa/4.0") {
            "CC-BY-SA-4.0"
        } else if has("attribution-sharealike 3.0") || has("by-sa/3.0") {
            "CC-BY-SA-3.0"
        } else if has("attribution 4.0") || has("licenses/by/4.0") {
            "CC-BY-4.0"
        } else if has("attribution 3.0") || has("licenses/by/3.0") {
            "CC-BY-3.0"
        } else if has("mit license") || has("permission is hereby granted, free of charge") {
            "MIT"
        } else if has("apache license") {
            "Apache-2.0"
        } else if has("gnu general public license") {
            "GPL"
        } else if has("lottie simple license") {
            "LottieFiles-Simple"
        } else {
            return None;
        },
    )
}

/// SHA-256 of `bytes`, in hex.
pub fn sha256(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// A folder name from a character's name: letters, digits, `-` and `_`, at most 48.
pub fn folder_name(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(48)
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() || s.chars().all(|c| c == '.') {
        return "character".into();
    }
    // Windows keeps these names for devices: a folder may not have one.
    let lower = s.to_ascii_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let numbered = chars.len() == 4
        && (lower.starts_with("com") || lower.starts_with("lpt"))
        && (chars[3].is_ascii_digit() || matches!(chars[3], '¹' | '²' | '³'));
    let device = matches!(lower.as_str(), "con" | "prn" | "aux" | "nul") || numbered;
    if device {
        format!("{s}-character")
    } else {
        s
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// One file of a package: its path inside the folder (`/`-separated), its bytes.
pub type Entry = (String, Vec<u8>);

/// A package's content hash: every file in path order (path, length, bytes) and the
/// choices (mapping, sampling, pace, licence). The folder is named by it and never edited
/// in place (review 3): equal names mean equal content.
pub fn content_hash(entries: &[Entry], settings: &Settings, licence: &StoredLicence) -> String {
    let mut sorted: Vec<&Entry> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let mut h = Sha256::new();
    for (path, bytes) in sorted {
        h.update(path.as_bytes());
        h.update([0u8]);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    }
    h.update(b"choices");
    h.update(serde_json::to_vec(&(settings, licence)).unwrap_or_default());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// The files of a package as they are in `dir` (its `character.json` names them).
fn folder_entries(dir: &Path, p: &Package) -> Result<Vec<Entry>, ImportError> {
    let limits = Limits::DEFAULT;
    let max = limits
        .image_bytes
        .max(limits.json_bytes)
        .max(limits.zip_bytes as usize);
    let mut out = vec![(p.file.clone(), paths::read_inside(dir, &p.file, max)?)];
    for f in &p.files {
        out.push((f.clone(), paths::read_inside(dir, f, max)?));
    }
    Ok(out)
}

/// The content hash of the package in `dir`, from its own files (`None`: not a package, or
/// a file of it missing).
pub fn folder_hash(dir: &Path) -> Option<String> {
    let p = read_package(dir).ok()?;
    let entries = folder_entries(dir, &p).ok()?;
    Some(content_hash(&entries, &p.settings, &p.licence))
}

/// Put a package in `<project_dir>/Characters/<name>-<8 hex>/`: the folder of its content
/// hash, reused when one there holds the very same content (checked from its files), else
/// written whole; a folder of that name holding anything else (a hand edited it) is passed
/// over for `…-2`. Gives back the key path and whether the folder was made now.
fn place(
    project_dir: &Path,
    entries: &[Entry],
    mut package: Package,
) -> Result<(String, bool), ImportError> {
    let hash = content_hash(entries, &package.settings, &package.licence);
    package.hash = hash.clone();
    package.files = entries
        .iter()
        .map(|(p, _)| p.clone())
        .filter(|p| *p != package.file)
        .collect();
    package.files.sort();
    let base = format!("{}-{}", folder_name(&package.name), &hash[..8]);
    let root = project_dir.join(FOLDER);
    for n in 1..1000 {
        let name = if n == 1 {
            base.clone()
        } else {
            format!("{base}-{n}")
        };
        let dir = root.join(&name);
        let rel = format!("{FOLDER}/{name}/{}", package.file);
        if dir.exists() {
            if folder_hash(&dir).as_deref() == Some(hash.as_str()) {
                return Ok((rel, false));
            }
            continue;
        }
        std::fs::create_dir_all(&dir)?;
        let written = (|| -> Result<(), ImportError> {
            for (path, bytes) in entries {
                let parts = paths::relative_parts(path)?;
                let mut to = dir.clone();
                for p in &parts {
                    to.push(p);
                }
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&to, bytes)?;
            }
            let json = serde_json::to_vec_pretty(&package)
                .map_err(|e| ImportError::Invalid(e.to_string()))?;
            write_atomic(&dir.join(PACKAGE_FILE), &json)?;
            Ok(())
        })();
        if let Err(e) = written {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
        return Ok((rel, true));
    }
    Err(ImportError::Invalid(
        "too many characters of this name".into(),
    ))
}

/// Copy the character file at `src` (and the files `found` names that could be read, and a
/// licence file beside it) into the project as a content-addressed package (see
/// [`content_hash`]), and give back its key path.
pub fn copy_into(
    project_dir: &Path,
    src: &Path,
    found: &Found,
    settings: &Settings,
    licence: &Licence,
) -> Result<String, ImportError> {
    copy_into_new(project_dir, src, found, settings, licence).map(|(rel, _)| rel)
}

/// [`copy_into`], saying whether the folder was made now (`false`: one already there with
/// the very same content was reused) — what an abandoned import may remove.
pub fn copy_into_new(
    project_dir: &Path,
    src: &Path,
    found: &Found,
    settings: &Settings,
    licence: &Licence,
) -> Result<(String, bool), ImportError> {
    let limits = Limits::DEFAULT;
    let bytes = paths::read_bounded(src, limits.json_bytes.max(limits.zip_bytes as usize))?;
    let file_name = src
        .file_name()
        .and_then(|n| n.to_str())
        .map(str::to_string)
        .ok_or_else(|| ImportError::Invalid("the file has no name".into()))?;
    let src_dir = src.parent().unwrap_or(Path::new("."));
    let mut entries: Vec<Entry> = vec![(file_name.clone(), bytes.clone())];
    for f in &found.files {
        // A file the card already said could not be read (a Lottie's image) is left out;
        // what the character needs to draw was read when it was checked.
        let (Ok(from), Ok(parts)) = (paths::resolve(src_dir, f), paths::relative_parts(f)) else {
            continue;
        };
        let Ok(data) = paths::read_bounded(&from, limits.image_bytes.max(limits.json_bytes)) else {
            continue;
        };
        let path = parts.join("/");
        if !entries.iter().any(|(p, _)| *p == path) {
            entries.push((path, data));
        }
    }
    if let Some(l) = licence_file(src_dir) {
        if let (Ok(data), Some(n)) = (
            paths::read_bounded(&l, 1 << 20),
            l.file_name().and_then(|n| n.to_str()),
        ) {
            if !entries.iter().any(|(p, _)| p == n) {
                entries.push((n.to_string(), data));
            }
        }
    }
    let package = Package {
        version: 2,
        format: found.source.format().to_string(),
        file: file_name.clone(),
        original: file_name,
        sha256: sha256(&bytes),
        name: found.name.clone(),
        author: found.author.clone(),
        licence: StoredLicence::of(licence),
        settings: settings.clone(),
        files: Vec::new(),
        hash: String::new(),
    };
    place(project_dir, &entries, package)
}

/// `character.json` of the folder `dir`.
pub fn read_package(dir: &Path) -> Result<Package, ImportError> {
    let bytes = paths::read_bounded(&dir.join(PACKAGE_FILE), 1 << 20)?;
    serde_json::from_slice(&bytes).map_err(|e| ImportError::Invalid(format!("{PACKAGE_FILE}: {e}")))
}

/// *Edit moves…*: the character at `rel` with other choices — a new package beside it (or
/// one already there with the very same content); the folder at `rel` is never changed (it
/// may be another project's too). Gives back the new key path (`rel` itself when nothing
/// changed); the caller re-keys this project's dancers in one undo step.
pub fn with_settings(
    project_dir: &Path,
    rel: &str,
    settings: &Settings,
) -> Result<String, ImportError> {
    let file = paths::resolve(project_dir, rel)?;
    let dir = file.parent().unwrap_or(project_dir).to_path_buf();
    let p = read_package(&dir)?;
    if p.settings == *settings {
        return Ok(rel.to_string());
    }
    let entries = folder_entries(&dir, &p)?;
    let package = Package {
        settings: settings.clone(),
        ..p
    };
    place(project_dir, &entries, package).map(|(rel, _)| rel)
}

type Stamp = Vec<(u64, Option<SystemTime>)>;

struct Cached {
    path: PathBuf,
    stamp: Stamp,
    character: Arc<dyn Character>,
}

static CACHE: Mutex<Vec<Cached>> = Mutex::new(Vec::new());
/// Characters kept loaded.
pub const CACHE_LEN: usize = 16;

fn stamp(paths: &[PathBuf]) -> Stamp {
    paths
        .iter()
        .map(|p| match std::fs::metadata(p) {
            Ok(m) => (m.len(), m.modified().ok()),
            Err(_) => (u64::MAX, None),
        })
        .collect()
}

fn cache() -> std::sync::MutexGuard<'static, Vec<Cached>> {
    match CACHE.lock() {
        Ok(c) => c,
        Err(p) => p.into_inner(),
    }
}

/// Drop the cached character of `file` (its settings changed).
pub fn forget(file: &Path) {
    cache().retain(|c| c.path != file);
}

/// The imported character at `rel` (relative to `project_dir` — the key's
/// `file:<rel>`), read through `character.json` beside it (or guessed, for a file put
/// there by hand). Cached by path; a changed file is read again.
pub fn load(project_dir: &Path, rel: &str) -> Result<Arc<dyn Character>, ImportError> {
    let file = paths::resolve(project_dir, rel)?;
    let dir = file.parent().unwrap_or(project_dir).to_path_buf();
    let watched = vec![file.clone(), dir.join(PACKAGE_FILE)];
    let now = stamp(&watched);
    if let Some(c) = cache().iter().find(|c| c.path == file && c.stamp == now) {
        return Ok(c.character.clone());
    }
    let found = inspect_with(&file, &Limits::DEFAULT)?;
    let id = format!("file:{}", rel.replace('\\', "/"));
    let (found, settings, licence, name, author) = match read_package(&dir) {
        Ok(p) => {
            let licence = p.licence.licence();
            (found, p.settings, licence, p.name, p.author)
        }
        Err(_) => {
            let s = super::guessed(&found);
            let (l, n, a) = (
                found.licence.clone(),
                found.name.clone(),
                found.author.clone(),
            );
            (found, s, l, n, a)
        }
    };
    let found = Found {
        licence,
        name: if name.is_empty() {
            found.name.clone()
        } else {
            name
        },
        author: if author.is_empty() {
            found.author.clone()
        } else {
            author
        },
        ..found
    };
    let character: Arc<dyn Character> = Arc::new(super::character(found, id, settings));
    let mut c = cache();
    c.retain(|x| x.path != file);
    if c.len() >= CACHE_LEN {
        c.remove(0);
    }
    c.push(Cached {
        path: file,
        stamp: now,
        character: character.clone(),
    });
    Ok(character)
}

/// The folder (`Characters/<name>`) of a key path.
pub fn folder_of(rel: &str) -> Option<String> {
    let parts = paths::relative_parts(rel).ok()?;
    (parts.len() >= 2).then(|| parts[..parts.len() - 1].join("/"))
}

/// What carrying did: the folders copied, the key paths that had to move (a folder of that
/// name beside it held other content — a hand edited it), and the characters that were not
/// where the project keeps them (said, never skipped silently).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Carried {
    pub copied: Vec<String>,
    /// (the key path as it was, the key path it must become).
    pub renamed: Vec<(String, String)>,
    pub missing: Vec<String>,
}

/// Carry the characters at `rels` from `from_dir` to `to_dir` (a save into a new folder,
/// an untitled project's first save). Folders are content-addressed (review 3): one of that
/// name beside it holding the very same content is shared; one holding anything else is
/// passed over for a fresh name (`…-2`), and [`Carried::renamed`] says which keys follow.
/// All or nothing: a copy failing part-way removes what this carry made. Files only (no
/// links), each bounded.
pub fn carry(from_dir: &Path, to_dir: &Path, rels: &[String]) -> Result<Carried, ImportError> {
    let mut out = Carried::default();
    if from_dir == to_dir {
        return Ok(out);
    }
    let mut moved: Vec<(String, String)> = Vec::new();
    let mut made: Vec<PathBuf> = Vec::new();
    let result = (|| -> Result<(), ImportError> {
        for rel in rels {
            let Some(folder) = folder_of(rel) else {
                continue;
            };
            let target = match moved.iter().find(|(f, _)| *f == folder) {
                Some((_, t)) => t.clone(),
                None => {
                    let src = paths::resolve(from_dir, &folder)?;
                    if !src.is_dir() {
                        if !out.missing.contains(&folder) {
                            out.missing.push(folder.clone());
                        }
                        continue;
                    }
                    let want = folder_hash(&src);
                    let mut chosen = None;
                    for n in 1..1000 {
                        let name = if n == 1 {
                            folder.clone()
                        } else {
                            format!("{folder}-{n}")
                        };
                        let dst = paths::resolve(to_dir, &name)?;
                        if !dst.exists() {
                            copy_tree(&src, &dst, 0)?;
                            made.push(dst);
                            out.copied.push(name.clone());
                            chosen = Some(name);
                            break;
                        }
                        if want.is_some() && folder_hash(&dst) == want {
                            chosen = Some(name);
                            break;
                        }
                    }
                    let t = chosen.ok_or_else(|| {
                        ImportError::Invalid("too many characters of this name".into())
                    })?;
                    moved.push((folder.clone(), t.clone()));
                    t
                }
            };
            if target != folder {
                let file = paths::relative_parts(rel)?.pop().unwrap_or_default();
                let new_rel = format!("{target}/{file}");
                if !out.renamed.iter().any(|(r, _)| r == rel) {
                    out.renamed.push((rel.clone(), new_rel));
                }
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        for dir in made {
            let _ = std::fs::remove_dir_all(dir);
        }
        return Err(e);
    }
    Ok(out)
}

fn copy_tree(src: &Path, dst: &Path, depth: usize) -> Result<(), ImportError> {
    if depth > 8 {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for e in std::fs::read_dir(src)?.flatten() {
        let t = e.file_type()?;
        let to = dst.join(e.file_name());
        if t.is_dir() {
            copy_tree(&e.path(), &to, depth + 1)?;
        } else if t.is_file() {
            let data = paths::read_bounded(&e.path(), 64 << 20)?;
            std::fs::write(to, data)?;
        }
    }
    Ok(())
}
