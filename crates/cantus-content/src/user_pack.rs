//! *Install from a file…* (round 40 phase 3, ADR 0125 §3 and §4; `chorda-src` — Tympanum's
//! kits use it too): what the user brings — a bought SFZ, Decent Sampler or SF2 instrument,
//! a DrumGizmo kit, an archive of one, or the very file a *Manual* or a changed *From the
//! author's site* pack points to.
//!
//! - **A pack's own file** is recognised by the source's pinned size and SHA-256, whatever
//!   it is called, and installs as that pack — through the ordinary install
//!   (`Manager::install_from_file`), so its licence file, receipt and credit are the
//!   manifest's.
//! - **Anything else** becomes a **user pack**: a folder `user-<name>-<hash>` in the
//!   content root holding a receipt whose licence is [`USER_LICENCE`] (the user's own:
//!   bought, or under terms they accepted — nothing Cantus redistributes, never credited
//!   as a content pack). A loose instrument file is **referenced where it is** (a bought
//!   library of gigabytes is not copied); an archive is unpacked into the folder and
//!   every instrument file in it is offered.

use crate::install::{Phase, Receipt, RECEIPT};
use crate::manifest::{Manifest, PackInstrument, Plays, Shelf, Source, Text};
use sha2::{Digest, Sha256};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// The licence a user pack's receipt says: the user's own terms.
pub const USER_LICENCE: &str = "LicenseRef-user";

/// What every user pack's id starts with ([`user_id`]).
pub const USER_PREFIX: &str = "user-";

/// What an instrument file is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Format {
    Sfz,
    DecentSampler,
    Sf2,
    /// A DrumGizmo kit (`.xml` with a `<drumkit>`): Tympanum's.
    DrumGizmo,
}

impl Format {
    pub fn of(path: &Path) -> Option<Format> {
        let ext = path.extension()?.to_string_lossy().to_ascii_lowercase();
        match ext.as_str() {
            "sfz" => Some(Format::Sfz),
            "dspreset" => Some(Format::DecentSampler),
            "sf2" => Some(Format::Sf2),
            "xml" => {
                let mut head = [0u8; 4096];
                let n = std::fs::File::open(path)
                    .and_then(|mut f| f.read(&mut head))
                    .unwrap_or(0);
                String::from_utf8_lossy(&head[..n])
                    .contains("<drumkit")
                    .then_some(Format::DrumGizmo)
            }
            _ => None,
        }
    }
}

/// What a dropped or chosen file is.
#[derive(Clone, Debug, PartialEq)]
pub enum Recognised {
    /// The source of the manifest's pack `pack`, byte for byte: install it as that pack.
    Pack(String),
    /// An instrument file, played where it is.
    Instrument(Format),
    /// An archive (`.zip`, `.tar.gz`) to unpack as a user pack.
    Archive,
}

/// Why a file cannot be installed.
#[derive(Clone, Debug, PartialEq)]
pub enum UserPackError {
    /// Not an instrument, an archive or a pack's file.
    Unknown,
    Io(String),
    /// An archive with no instrument file in it.
    NoInstrument,
    /// The volume has less free space than the archive says it holds (bytes).
    NoSpace {
        need: u64,
        free: u64,
    },
    /// A DrumGizmo kit that could not be built for Tympanum: why.
    Kit(String),
    /// Stopped by the user: nothing was left behind.
    Cancelled,
}

impl std::fmt::Display for UserPackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserPackError::Unknown => write!(
                f,
                "it is not an SFZ, Decent Sampler, SF2 or DrumGizmo instrument, an archive of one, or a pack's own file"
            ),
            UserPackError::Io(e) => write!(f, "{e}"),
            UserPackError::NoInstrument => write!(f, "the archive holds no instrument file"),
            UserPackError::NoSpace { need, free } => write!(
                f,
                "not enough free space: the archive holds {need} bytes and the disk has {free}"
            ),
            UserPackError::Kit(why) => write!(f, "the DrumGizmo kit could not be built: {why}"),
            UserPackError::Cancelled => write!(f, "cancelled"),
        }
    }
}

fn io(e: std::io::Error) -> UserPackError {
    UserPackError::Io(e.to_string())
}

/// The SHA-256 of a file, lowercase hex.
pub fn sha256_of(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// What `path` is: a pack's source (its size first — a hash only when a size matches), an
/// instrument file, or an archive.
pub fn recognise(manifest: &Manifest, path: &Path) -> Result<Recognised, UserPackError> {
    let len = std::fs::metadata(path).map_err(io)?.len();
    let mut hash: Option<String> = None;
    for p in &manifest.packs {
        let (size, sha) = match &p.source {
            Source::Zip(z) | Source::TarGz(z) => (z.size, &z.sha256),
            Source::Files(_) => continue,
        };
        if size != len {
            continue;
        }
        if hash.is_none() {
            hash = Some(sha256_of(path).map_err(io)?);
        }
        if hash.as_deref() == Some(sha.as_str()) {
            return Ok(Recognised::Pack(p.id.clone()));
        }
    }
    if let Some(f) = Format::of(path) {
        return Ok(Recognised::Instrument(f));
    }
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if name.ends_with(".zip") || name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return Ok(Recognised::Archive);
    }
    Err(UserPackError::Unknown)
}

/// A user pack's id from what the user brought: `user-<name>-<8 hex of its path>`, so the
/// same file installed twice is one pack.
pub fn user_id(path: &Path) -> String {
    let stem = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let mut slug: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    let slug: String = slug.chars().take(40).collect();
    let mut h: u32 = 0x811c_9dc5;
    for b in path.to_string_lossy().bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(0x0100_0193);
    }
    format!("user-{}-{h:08x}", slug.trim_matches('-'))
}

fn instrument_of(file: &Path, rel: String, plays: Plays) -> Option<PackInstrument> {
    let format = Format::of(file)?;
    let name = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let plays = match format {
        Format::DrumGizmo => Plays::Tympanum,
        _ => plays,
    };
    let mut id: String = name
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    id.retain(|c| c != '\0');
    let id = id.trim_matches('-').to_string();
    Some(PackInstrument {
        id: if id.is_empty() {
            "instrument".into()
        } else {
            id
        },
        name: Text {
            en: name.clone(),
            es: name,
        },
        file: rel,
        shelf: if plays == Plays::Tympanum {
            Shelf::Drums
        } else {
            Shelf::Keys
        },
        layers: None,
        gain_db: 0.0,
        voices: None,
        plays,
    })
}

/// Every instrument file under `dir`, relative to it, sorted.
fn instruments_under(dir: &Path, plays: Plays) -> Vec<PackInstrument> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(dir) {
                let rel = rel.to_string_lossy().replace('\\', "/");
                if let Some(i) = instrument_of(&p, rel, plays) {
                    out.push(i);
                }
            }
        }
    }
    out.sort_by(|a, b| a.file.cmp(&b.file));
    // Ids unique within the pack.
    let mut seen = std::collections::HashSet::new();
    for i in out.iter_mut() {
        let base = i.id.clone();
        let mut n = 2;
        while !seen.insert(i.id.clone()) {
            i.id = format!("{base}-{n}");
            n += 1;
        }
    }
    out
}

/// Install `path` as a user pack into the content folder `root`: a loose instrument
/// referenced where it is, an archive unpacked, a DrumGizmo kit built for Tympanum
/// ([`dg`]). `plays` says what plays its instruments (Chorda's source list asks for
/// [`Plays::Chorda`]). Returns the receipt written.
pub fn install_user(root: &Path, path: &Path, plays: Plays) -> Result<Receipt, UserPackError> {
    install_user_with(root, path, plays, &|p: &Path| {
        cantus_rt::disk::space(p).map(|s| s.free)
    })
}

/// What a staging folder's name starts with: `.user-staging-<id>-<pid>` in the content
/// root, renamed into place once whole ([`install_user_with`]).
const STAGING: &str = ".user-staging-";

/// [`install_user`] asking `free_space` for the space on the content folder's volume
/// (tests). r40:fix-f (review of 40a27bb7, finding 5): the pack is made in a fresh
/// staging folder and renamed into place only once whole — a failure leaves nothing
/// behind, a retry starts clean, installing the same file again replaces the pack — and
/// an archive is unpacked only when the volume has room for what it says it holds, at most
/// the free space written.
pub fn install_user_with(
    root: &Path,
    path: &Path,
    plays: Plays,
    free_space: &dyn Fn(&Path) -> Option<u64>,
) -> Result<Receipt, UserPackError> {
    install_user_job(
        root,
        path,
        plays,
        free_space,
        &AtomicBool::new(false),
        &mut |_| {},
    )
}

/// [`install_user_with`] as a job (r40:fix-f: it runs on the content manager's worker,
/// `Manager::install_user`, never on the window's thread): `cancel` stops it — nothing is
/// left behind — and `report` hears how far it is (unpacking, then a DrumGizmo kit's
/// build), as a pack's download is reported.
pub fn install_user_job(
    root: &Path,
    path: &Path,
    plays: Plays,
    free_space: &dyn Fn(&Path) -> Option<u64>,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Phase),
) -> Result<Receipt, UserPackError> {
    std::fs::create_dir_all(root).map_err(io)?;
    let id = user_id(path);
    let dir = root.join(&id);
    let stage = root.join(format!("{STAGING}{id}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir_all(&stage).map_err(io)?;
    let made = fill(&stage, &id, path, plays, free_space, cancel, report).and_then(|receipt| {
        if cancel.load(Ordering::Relaxed) {
            return Err(UserPackError::Cancelled);
        }
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(io)?;
        }
        std::fs::rename(&stage, &dir).map_err(io)?;
        Ok(receipt)
    });
    if made.is_err() {
        let _ = std::fs::remove_dir_all(&stage);
        // A folder of this pack with no receipt (an older Cantus's failed unpack) is no
        // pack: it goes too.
        if dir.exists() && !dir.join(RECEIPT).exists() {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
    made
}

/// Folders an install left behind: staging folders, and user packs with no receipt (an
/// older Cantus's failed unpack — invisible in the lists, never removable there). Removed;
/// returns how many. At start-up (`CantusApp::assemble`), before any install runs.
pub fn clean_orphans(root: &Path) -> usize {
    let mut n = 0;
    for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let stale = name.starts_with(STAGING)
            || (name.starts_with(USER_PREFIX) && !p.join(RECEIPT).exists());
        if stale && std::fs::remove_dir_all(&p).is_ok() {
            n += 1;
        }
    }
    n
}

/// The pack made in `stage`: its instruments found or built, its receipt written.
fn fill(
    stage: &Path,
    id: &str,
    path: &Path,
    plays: Plays,
    free_space: &dyn Fn(&Path) -> Option<u64>,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Phase),
) -> Result<Receipt, UserPackError> {
    let display = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| id.to_string());
    let instruments = match Format::of(path) {
        Some(Format::DrumGizmo) => vec![built_kit(path, stage, String::new(), cancel, report)?],
        Some(_) => {
            let abs: PathBuf = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
            let abs = abs
                .to_string_lossy()
                .trim_start_matches(r"\\?\")
                .replace('\\', "/");
            instrument_of(path, abs, plays).into_iter().collect()
        }
        None => {
            let need = declared_size(path)?;
            let free = free_space(stage);
            if let Some(free) = free {
                if free < need {
                    return Err(UserPackError::NoSpace { need, free });
                }
            }
            unpack_any(path, stage, free.unwrap_or(u64::MAX / 4), cancel, report)?;
            let mut found = instruments_under(stage, plays);
            // A DrumGizmo kit in it is built for Tympanum — one (a kit shipped in several
            // setups is built in its basic one, else its first): each build is gigabytes.
            let is_kit = |i: &PackInstrument| i.file.to_ascii_lowercase().ends_with(".xml");
            let kits: Vec<PackInstrument> = found.iter().filter(|i| is_kit(i)).cloned().collect();
            found.retain(|i| !is_kit(i));
            let pick = kits
                .iter()
                .find(|k| k.file.to_ascii_lowercase().contains("basic"))
                .or_else(|| kits.first());
            if let Some(k) = pick {
                let xml = crate::drumgizmo::path_of(stage, &k.file);
                let built = built_kit(&xml, stage, k.id.clone(), cancel, report)?;
                // The unpacked multichannel source goes once its mics are written: only
                // what plays is kept (installing again reads the user's archive again).
                if found.is_empty() {
                    keep_only(stage, &[dg::SFZ, &dg::sidecar(), dg::FOLDER])?;
                } else {
                    for k in &kits {
                        let xml = crate::drumgizmo::path_of(stage, &k.file);
                        dg::remove_source(&xml);
                    }
                }
                found.insert(0, built);
            }
            if found.is_empty() {
                return Err(UserPackError::NoInstrument);
            }
            found
        }
    };
    let (files, bytes) = (instruments.len() as u64, 0);
    let receipt = Receipt {
        id: id.to_string(),
        version: 1,
        name: Text {
            en: display.clone(),
            es: display,
        },
        licence: USER_LICENCE.into(),
        licence_url: String::new(),
        licence_file: String::new(),
        credits: Vec::new(),
        attribution: None,
        instruments,
        installed_at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        files,
        bytes,
    };
    let json =
        serde_json::to_string_pretty(&receipt).map_err(|e| UserPackError::Io(e.to_string()))?;
    std::fs::write(stage.join(RECEIPT), json).map_err(io)?;
    Ok(receipt)
}

/// A DrumGizmo kit `xml` built into `stage` for Tympanum ([`dg::build`]), as the pack's
/// instrument (`id`, else one from the kit's file name).
fn built_kit(
    xml: &Path,
    stage: &Path,
    id: String,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Phase),
) -> Result<PackInstrument, UserPackError> {
    let name = dg::build(xml, stage, cancel, report).map_err(|e| {
        if cancel.load(Ordering::Relaxed) {
            UserPackError::Cancelled
        } else {
            UserPackError::Kit(e)
        }
    })?;
    let mut i =
        instrument_of(xml, dg::SFZ.into(), Plays::Tympanum).ok_or(UserPackError::Unknown)?;
    if !id.is_empty() {
        i.id = id;
    }
    if !name.trim().is_empty() {
        i.name = Text {
            en: name.clone(),
            es: name,
        };
    }
    Ok(i)
}

/// What an archive says it holds unpacked, bytes: a zip's entries' sizes; a `.tar.gz`'s
/// gzip trailer (its size modulo 4 GiB), at least the archive's own size.
pub fn declared_size(path: &Path) -> Result<u64, UserPackError> {
    use std::io::{Seek, SeekFrom};
    let len = std::fs::metadata(path).map_err(io)?.len();
    let name = path.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".zip") {
        let f = std::fs::File::open(path).map_err(io)?;
        let mut z = zip::ZipArchive::new(f).map_err(|e| UserPackError::Io(e.to_string()))?;
        let mut sum = 0u64;
        for i in 0..z.len() {
            if let Ok(e) = z.by_index_raw(i) {
                sum = sum.saturating_add(e.size());
            }
        }
        return Ok(sum);
    }
    let mut f = std::fs::File::open(path).map_err(io)?;
    let mut tail = [0u8; 4];
    let trailer =
        if len >= 4 && f.seek(SeekFrom::End(-4)).is_ok() && f.read_exact(&mut tail).is_ok() {
            u64::from(u32::from_le_bytes(tail))
        } else {
            0
        };
    Ok(trailer.max(len))
}

/// Unpack a user's archive into `dir`: every entry, nothing outside the folder, at most
/// `budget` bytes written.
fn unpack_any(
    path: &Path,
    dir: &Path,
    budget: u64,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Phase),
) -> Result<(), UserPackError> {
    let z = crate::manifest::ZipSource {
        url: String::new(),
        size: 0,
        sha256: String::new(),
        strip: String::new(),
        include: Vec::new(),
        exclude: vec!["__MACOSX/**".into(), "**/._*".into()],
    };
    let name = path.to_string_lossy().to_ascii_lowercase();
    let r = if name.ends_with(".zip") {
        crate::install::unpack(path, &z, budget, dir, cancel, report)
    } else {
        crate::install::unpack_tar_gz(path, &z, budget, dir, cancel, report)
    };
    r.map(|_| ()).map_err(|e| {
        if cancel.load(Ordering::Relaxed) {
            UserPackError::Cancelled
        } else {
            UserPackError::Io(e.to_string())
        }
    })
}

/// Remove everything in `dir` but the entries named in `keep` (and the receipt).
fn keep_only(dir: &Path, keep: &[&str]) -> Result<(), UserPackError> {
    for e in std::fs::read_dir(dir).map_err(io)?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if keep.contains(&name.as_str()) || name == RECEIPT {
            continue;
        }
        let p = e.path();
        if p.is_dir() {
            std::fs::remove_dir_all(&p).map_err(io)?;
        } else {
            std::fs::remove_file(&p).map_err(io)?;
        }
    }
    Ok(())
}

// r40:fix-f — a user's DrumGizmo kit built for Tympanum at install (finding 4).
#[path = "user_pack_dg.rs"]
pub mod dg;
