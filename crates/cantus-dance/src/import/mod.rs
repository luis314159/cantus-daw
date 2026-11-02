//! Characters people bring (ADR 0089): checked and sandboxed before anything is drawn.
//!
//! The formats (the measured choice of ADR 0089): **Lottie** (`.json`, `.lottie`) by our
//! own player, sprite sheets from **Aseprite** and **TexturePacker**, and **DragonBones**
//! (JSON 5.x). Whatever the format, an import:
//! - **runs no script** (a Lottie expression, a `.lottie` state machine — ignored, and
//!   said on the card);
//! - reads only the file and what its own folder holds (never the network, never a path
//!   outside its folder — [`paths`]), with every size bounded — bytes, nesting, layers,
//!   frames, image pixels — before memory is given to it ([`limits`]);
//! - is copied into the project's folder (`Characters/<name>/`, [`package`]), so the
//!   project carries it, with its licence and its mapping in `character.json`;
//! - shows its licence ([`crate::Licence`]) before it is used;
//! - maps its own animations onto [`crate::Move`]s (by name first, then by the person's
//!   choice — [`mapping`]), so automation and notes drive it like an included character.

use crate::character::{Character, CharacterInfo, Licence, Origin};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod dragonbones;
pub mod image;
pub mod imported;
pub mod limits;
pub mod lottie;
pub mod mapping;
pub mod package;
pub mod paths;
pub mod raster;
pub mod samples;
pub mod sheet;

pub use imported::{Imported, Settings, Source};
pub use limits::Limits;
pub use mapping::{Clip, Mapping};
pub use package::{load, Package};
pub use raster::Filter;

/// Why a file could not become a character, in words a person can act on.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("this kind of file is not a character Cantus can read")]
    Unsupported,
    #[error("the file could not be read: {0}")]
    Io(#[from] std::io::Error),
    #[error("the file is not valid: {0}")]
    Invalid(String),
    #[error("the file is larger than Cantus allows ({0})")]
    TooLarge(String),
}

impl ImportError {
    /// The locale key of its kind (the card shows it with the detail).
    pub fn label_key(&self) -> &'static str {
        match self {
            ImportError::Unsupported => "dance_import.error.unsupported",
            ImportError::Io(_) => "dance_import.error.io",
            ImportError::Invalid(_) => "dance_import.error.invalid",
            ImportError::TooLarge(_) => "dance_import.error.too_large",
        }
    }

    /// The detail, as the parser said it (names and numbers from the file).
    pub fn detail(&self) -> String {
        match self {
            ImportError::Unsupported => String::new(),
            ImportError::Io(e) => e.to_string(),
            ImportError::Invalid(s) | ImportError::TooLarge(s) => s.clone(),
        }
    }
}

/// Something the card says about a file: what was ignored or assumed. `key` is a locale
/// key under `dance_import.note.*`; `arg` fills its `{arg}`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub key: &'static str,
    pub arg: String,
}

impl Note {
    pub fn new(key: &'static str, arg: String) -> Note {
        Note { key, arg }
    }
}

/// A file checked and read, before it is used: what the card shows, and the source it
/// becomes.
pub struct Found {
    pub source: Box<dyn Source>,
    /// Its name as the file states it (empty: the file's own name is used).
    pub name: String,
    pub author: String,
    /// What the file states (`Unknown` when it states nothing).
    pub licence: Licence,
    pub notes: Vec<Note>,
    /// What it holds, for the card: (`dance_import.fact.*` key, value).
    pub facts: Vec<(&'static str, String)>,
    /// The other files it names, relative to its folder (copied with it).
    pub files: Vec<String>,
    pub default_filter: Filter,
}

impl std::fmt::Debug for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Found")
            .field("format", &self.source.format())
            .field("clips", &self.source.clips().len())
            .field("name", &self.name)
            .field("notes", &self.notes)
            .finish()
    }
}

/// Read and check the character file at `path` (the format found from its contents),
/// with the default limits.
pub fn inspect(path: &Path) -> Result<Found, ImportError> {
    inspect_with(path, &Limits::DEFAULT)
}

pub fn inspect_with(path: &Path, limits: &Limits) -> Result<Found, ImportError> {
    guarded(|| inspect_unguarded(path, limits))
}

/// Run a reader so that a bug in it (a panic on a file nobody foresaw) comes back as an
/// error in words, never taking the app down: the dancer then draws the default character
/// and says why.
pub fn guarded<T>(f: impl FnOnce() -> Result<T, ImportError>) -> Result<T, ImportError> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(_) => Err(ImportError::Invalid(
            "Cantus could not read this file (it stopped on something unexpected)".into(),
        )),
    }
}

fn inspect_unguarded(path: &Path, limits: &Limits) -> Result<Found, ImportError> {
    let folder = path.parent().unwrap_or(Path::new("."));
    let max = limits.json_bytes.max(limits.zip_bytes as usize);
    let bytes = paths::read_bounded(path, max)?;
    let mut found = inspect_bytes(&bytes, folder, limits)?;
    if found.name.is_empty() {
        found.name = display_name(path);
    }
    if found.licence == Licence::Unknown {
        if let Some(l) = package::licence_beside(path) {
            found.licence = l;
        }
    }
    Ok(found)
}

/// The same from bytes already read; `folder` is where the files it names are.
pub fn inspect_bytes(bytes: &[u8], folder: &Path, limits: &Limits) -> Result<Found, ImportError> {
    if bytes.starts_with(b"PK\x03\x04") {
        return lottie::read_package(bytes, limits);
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace());
    let Some(start) = start.filter(|&i| bytes[i] == b'{') else {
        return Err(ImportError::Unsupported);
    };
    let v = limits::parse_json(&bytes[start..], limits)?;
    if lottie::is_lottie(&v) {
        lottie::read(&v, folder, limits)
    } else if dragonbones::is_skeleton(&v) {
        dragonbones::read(&v, folder, limits)
    } else if v.get("SubTexture").is_some() {
        Err(ImportError::Invalid(
            "this is a DragonBones atlas; choose its skeleton (…_ske.json)".into(),
        ))
    } else if sheet::is_sheet(&v) {
        sheet::read(&v, folder, limits)
    } else {
        Err(ImportError::Unsupported)
    }
}

/// A file's name without its folder and its extensions (`hero_ske.json` → `hero`).
pub fn display_name(path: &Path) -> String {
    let stem = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("character");
    let stem = stem.split('.').next().unwrap_or(stem);
    let stem = stem.strip_suffix("_ske").unwrap_or(stem);
    if stem.is_empty() {
        "character".into()
    } else {
        stem.to_string()
    }
}

/// A character from `found` with `settings` (the card's choices); `id` is its
/// `file:<path>` key.
pub fn character(found: Found, id: String, settings: Settings) -> Imported {
    let info = CharacterInfo {
        id,
        name: found.name.clone(),
        author: found.author.clone(),
        origin: Origin::Imported {
            format: found.source.format(),
        },
        licence: found.licence.clone(),
    };
    Imported::new(info, found.source, settings)
}

/// The settings a file starts with: its names guessed, its default sampling.
pub fn guessed(found: &Found) -> Settings {
    Settings {
        mapping: Mapping::guess(found.source.clips()),
        filter: found.default_filter,
        own_speed: false,
    }
}

/// Read a character from `path` (the format found from its contents, not only its name),
/// mapped by its names — what the card's preview dances before anything is chosen.
///
/// A copy in a project (its `character.json` beside it, naming this file) is read with the
/// choices kept there — the mapping, the licence, the name — as [`load`] reads it.
pub fn import(path: &Path) -> Result<Arc<dyn Character>, ImportError> {
    let mut found = inspect(path)?;
    let id = format!("file:{}", path.display());
    let kept = path
        .parent()
        .and_then(|dir| package::read_package(dir).ok())
        .filter(|p| {
            path.file_name()
                .is_some_and(|n| n.to_string_lossy() == p.file)
        });
    let settings = match kept {
        Some(p) => {
            found.licence = p.licence.licence();
            if !p.name.is_empty() {
                found.name = p.name;
            }
            if !p.author.is_empty() {
                found.author = p.author;
            }
            p.settings
        }
        None => guessed(&found),
    };
    Ok(Arc::new(character(found, id, settings)))
}

/// Where Cantus keeps the characters of a project that has no folder yet, under its data
/// folder. The interface passes it as `project_dir` while the project is untitled, so the
/// key stays `file:Characters/…` and the first save only copies the folder beside it.
pub fn untitled_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("untitled")
}

/// What `ch` costs to draw, in milliseconds a frame: the mean over `frames` frames of its
/// routine on a 600×600 stage at size 1 (the budget is 1.5 ms — ADR 0087 §3). Shown on
/// the card; a measurement of this machine, never a reason to draw differently.
pub fn cost_ms(ch: &dyn Character, frames: u32) -> f64 {
    use crate::render::{render_into, Background, Stage};
    let stage = Stage::new(600, 600, Background::Transparent);
    let Some(mut pm) = tiny_skia::Pixmap::new(600, 600) else {
        return 0.0;
    };
    let frames = frames.max(1);
    let poses: Vec<crate::Pose> = (0..frames)
        .map(|i| {
            let beats = f64::from(i) * 0.37;
            let f = crate::DanceFrame {
                beats,
                seconds: beats * 0.5,
                playing: true,
                bar: (beats / 4.0).floor() as i32,
                bar_start_beats: (beats / 4.0).floor() * 4.0,
                ..crate::DanceFrame::default()
            };
            crate::pose_at(&f, ch, 0.0)
        })
        .collect();
    // One frame first, so a cache fills and the measure is of a frame in play.
    render_into(&mut pm, &stage, &[(ch, poses[0])]);
    let t0 = std::time::Instant::now();
    for p in &poses {
        render_into(&mut pm, &stage, &[(ch, *p)]);
    }
    t0.elapsed().as_secs_f64() * 1000.0 / f64::from(frames)
}
