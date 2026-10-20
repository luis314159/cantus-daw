//! DAWproject: the open interchange format of Bitwig and PreSonus (MIT,
//! github.com/bitwig/dawproject), read by Bitwig Studio, Studio One, Cubase and others
//! (ADR 0021).
//!
//! A `.dawproject` is a ZIP holding `project.xml` (the structure, the mixer and the
//! timeline), `metadata.xml` (title and credits) and whatever files they reference.
//!
//! - [`export`] writes the transport, a track and a channel per Cantus track, a channel
//!   per mixer strip in use, the sends and outputs, the clips with their notes, and each
//!   instrument and insert as a device named after it with its settings as a state file.
//! - [`import`] reads any application's file into a new project and lists, in a
//!   [`Report`], everything it held that Cantus cannot (audio clips, automation, plug-ins,
//!   the clip launcher…). A Cantus file comes back whole.

pub mod audio;
pub mod read;
pub mod read_timeline;
pub mod write;
pub mod xml;
pub mod zip;

use crate::report::Report;
use cantus_core::{EffectKind, Project};
use thiserror::Error;

pub const EXTENSION: &str = "dawproject";
/// The format version written, and the one read.
pub const FORMAT_VERSION: &str = "1.0";
/// What `<Application name>` says, and how a file of our own is recognised.
pub const APPLICATION: &str = "Cantus";
/// Device ids of Cantus's own devices start with this.
pub const DEVICE_PREFIX: &str = "cantus.";
/// The `comment` on a channel that is a Cantus track's own — its level and balance before
/// the mixer strip it feeds, FL's channel volume — rather than a strip. Any application
/// shows it as a comment; Cantus reads it back as what it was.
pub const TRACK_CHANNEL: &str = "cantus:channel-rack";

#[derive(Debug, Error)]
pub enum DawProjectError {
    #[error("{0}")]
    Zip(#[from] zip::ZipError),
    #[error("the archive has no project.xml")]
    NoProject,
    #[error("project.xml is not well-formed XML: {0}")]
    Xml(String),
    #[error("project.xml is not a DAWproject (its root is <{0}>)")]
    NotDawProject(String),
    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for DawProjectError {
    fn from(e: std::io::Error) -> Self {
        DawProjectError::Io(e.to_string())
    }
}

/// What [`export`] made.
#[derive(Clone, Debug, PartialEq)]
pub struct Exported {
    /// The `.dawproject` file.
    pub bytes: Vec<u8>,
    pub report: Report,
}

/// The parts of an export before they are zipped (tests validate them against the XSD).
#[derive(Clone, Debug, PartialEq)]
pub struct Parts {
    pub project_xml: String,
    pub metadata_xml: String,
    /// Device states and any other file the XML references, by path in the archive.
    pub files: Vec<(String, Vec<u8>)>,
    /// The audio files the archive carries: (path in the archive, file on disk), each
    /// once — read one at a time as the archive is written, never all held at once.
    pub audio: Vec<(String, std::path::PathBuf)>,
    pub report: Report,
}

/// Write a project as a `.dawproject`.
pub fn export(project: &Project, app_version: &str) -> Exported {
    export_with(project, app_version, None)
}

/// [`export`], the audio clips' files read from `base` (the project's folder) and carried
/// in the archive (round 29, ADR 0061) — in memory (tests, small projects; a file is
/// written by [`export_to`]). A file that cannot be read is left out and said.
pub fn export_with(
    project: &Project,
    app_version: &str,
    base: Option<&std::path::Path>,
) -> Exported {
    let mut parts = write::parts_with(project, app_version, base);
    let mut z = zip::ZipStream::new(std::io::Cursor::new(Vec::new()));
    let bytes = write_parts(&mut z, &mut parts)
        .and_then(|()| z.finish())
        .map(std::io::Cursor::into_inner)
        .unwrap_or_default();
    Exported {
        bytes,
        report: parts.report,
    }
}

/// [`export_with`] streamed to `dest`: the archive written as it is made, one audio file
/// in memory at a time, ZIP64 past 4 GiB, through a temporary file renamed into place (a
/// failed export leaves no half file). Returns what could not be carried.
pub fn export_to(
    project: &Project,
    app_version: &str,
    base: Option<&std::path::Path>,
    dest: &std::path::Path,
) -> std::io::Result<Report> {
    export_to_within(project, app_version, base, dest, u32::MAX as u64)
}

/// [`export_to`] with ZIP64 records past `limit` bytes (tests: a small one).
#[doc(hidden)]
pub fn export_to_within(
    project: &Project,
    app_version: &str,
    base: Option<&std::path::Path>,
    dest: &std::path::Path,
    limit: u64,
) -> std::io::Result<Report> {
    let mut parts = write::parts_with(project, app_version, base);
    let tmp = dest.with_extension("dawproject.part");
    let out = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    let mut z = zip::ZipStream::with_limit(out, limit);
    let done = write_parts(&mut z, &mut parts).and_then(|()| z.finish());
    match done.and_then(|w| w.into_inner().map_err(|e| e.into_error())) {
        Ok(f) => {
            f.sync_all()?;
            drop(f);
            std::fs::rename(&tmp, dest)?;
            Ok(parts.report)
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Write the parts' entries: the XML and device states deflated, the audio stored (read
/// one file at a time).
fn write_parts<W: std::io::Write + std::io::Seek>(
    z: &mut zip::ZipStream<W>,
    parts: &mut Parts,
) -> std::io::Result<()> {
    z.add("project.xml", parts.project_xml.as_bytes(), true)?;
    z.add("metadata.xml", parts.metadata_xml.as_bytes(), true)?;
    for (path, data) in &parts.files {
        z.add(path, data, true)?;
    }
    // Each audio file streamed from disk into the archive (round 29's second review).
    for (path, file) in &parts.audio {
        let opened = std::fs::File::open(file).and_then(|f| Ok((f.metadata()?.len(), f)));
        match opened {
            Ok((len, f)) => z.add_stored_from(path, std::io::BufReader::new(f), len)?,
            Err(_) => parts.report.dropped(
                "interop.report.x_audio_unread",
                vec![("file", file.to_string_lossy().into_owned())],
            ),
        }
    }
    Ok(())
}

/// A project read from a `.dawproject`, and what it could not keep.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    pub project: Project,
    pub report: Report,
    /// The audio files the archive carries (path in the archive, bytes): the project's
    /// sources name them by that path until the caller writes them to disk.
    pub media: Vec<(String, Vec<u8>)>,
}

/// Read a `.dawproject` into a new project.
pub fn import(bytes: &[u8]) -> Result<Imported, DawProjectError> {
    import_from(bytes, None)
}

/// [`import`] of a `.dawproject` in `folder`: a file it references outside the archive by
/// a relative path is found against that folder (DAWproject's rule) and named by its
/// absolute path; one that is neither in the archive nor on disk is said (round 29's
/// review).
pub fn import_from(
    bytes: &[u8],
    folder: Option<&std::path::Path>,
) -> Result<Imported, DawProjectError> {
    let files = zip::read(bytes)?;
    let mut imported = read::project(&files)?;
    let carried: Vec<String> = imported.media.iter().map(|(p, _)| p.clone()).collect();
    find_external(
        &mut imported.project,
        &mut imported.report,
        &carried,
        folder,
    );
    Ok(imported)
}

/// The most a part of an archive read into memory may hold — the project's XML, its
/// metadata, a device's state — bytes; a larger one refuses the file.
pub const PART_LIMIT: u64 = 64 << 20;

/// A `.dawproject` read from a file ([`import_file`]).
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedFile {
    pub project: Project,
    pub report: Report,
    /// The archive's audio files that could not be written into the media folder.
    pub unwritten: Vec<String>,
}

/// Whether an archive entry is an audio file (never read into memory: copied to disk).
fn is_audio_name(name: &str) -> bool {
    let ext = std::path::Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        ext.as_str(),
        "wav"
            | "wave"
            | "bwf"
            | "w64"
            | "aif"
            | "aiff"
            | "aifc"
            | "caf"
            | "flac"
            | "mp3"
            | "ogg"
            | "oga"
            | "opus"
            | "m4a"
            | "mp4"
            | "aac"
    )
}

/// Where archive entry `inner` is written under `dir`: its folders kept, every part that
/// would leave `dir` (a root, a drive, `..`) dropped, and a name another entry of this import
/// already took (by the letters alone: the Windows file system does not tell their cases
/// apart) numbered — two entries never write one file (round 29's second review: every
/// entry was flattened to its name). `taken` holds the names written so far.
pub fn media_path(
    dir: &std::path::Path,
    inner: &str,
    taken: &mut std::collections::HashSet<String>,
) -> std::path::PathBuf {
    let parts: Vec<&str> = inner
        .split(['/', '\\'])
        .filter(|p| !p.is_empty() && *p != "." && *p != ".." && !p.contains(':'))
        .collect();
    let (folders, file) = match parts.split_last() {
        Some((f, rest)) => (rest.to_vec(), (*f).to_string()),
        None => (Vec::new(), "audio.wav".to_string()),
    };
    let mut base = dir.to_path_buf();
    for f in folders {
        base.push(f);
    }
    let p = std::path::Path::new(&file);
    let stem = p
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let ext = p.extension().map(|e| e.to_string_lossy().into_owned());
    for n in 1.. {
        let name = match (n, &ext) {
            (1, _) => file.clone(),
            (n, Some(e)) => format!("{stem} {n}.{e}"),
            (n, None) => format!("{stem} {n}"),
        };
        let path = base.join(&name);
        if taken.insert(path.to_string_lossy().to_lowercase()) {
            return path;
        }
    }
    unreachable!()
}

/// Read the `.dawproject` at `path` into a new project: its parts read into memory (each at
/// most [`PART_LIMIT`]), every audio file it carries copied straight from the archive into
/// `media` ([`media_path`]), its checksum checked, the sources pointed at it; a file it
/// references outside the archive found against its folder. Neither the archive nor any
/// audio file is ever held whole in memory (round 29's second review: a ZIP64 project
/// was read three times over).
pub fn import_file(
    path: &std::path::Path,
    media: &std::path::Path,
) -> Result<ImportedFile, DawProjectError> {
    import_file_within(path, media, PART_LIMIT)
}

/// [`import_file`] with parts of at most `limit` bytes (tests: a small one).
#[doc(hidden)]
pub fn import_file_within(
    path: &std::path::Path,
    media: &std::path::Path,
    limit: u64,
) -> Result<ImportedFile, DawProjectError> {
    let mut r = std::io::BufReader::new(std::fs::File::open(path)?);
    let entries = zip::directory(&mut r)?;
    let mut parts: Vec<(String, Vec<u8>)> = Vec::with_capacity(entries.len());
    for e in &entries {
        let part = matches!(
            e.name.trim_start_matches("./"),
            "project.xml" | "metadata.xml"
        ) || (!is_audio_name(&e.name) && e.size <= limit);
        let data = if part {
            zip::read_entry(&mut r, e, limit)?
        } else {
            Vec::new() // its name only: an audio file, copied below
        };
        parts.push((e.name.clone(), data));
    }
    let imported = read::project(&parts)?;
    drop(parts);
    let (mut project, mut report) = (imported.project, imported.report);
    let carried: Vec<String> = imported.media.into_iter().map(|(p, _)| p).collect();
    let mut taken = std::collections::HashSet::new();
    let mut unwritten = Vec::new();
    for inner in &carried {
        let Some(e) = entries
            .iter()
            .find(|e| e.name.trim_start_matches("./") == inner)
        else {
            unwritten.push(inner.clone());
            continue;
        };
        let dest = media_path(media, inner, &mut taken);
        let written = dest
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .map_err(|err| zip::ZipError::Write(inner.clone(), err.to_string()))
            .and_then(|()| {
                let f = std::fs::File::create(&dest)
                    .map_err(|err| zip::ZipError::Write(inner.clone(), err.to_string()))?;
                let mut w = std::io::BufWriter::new(f);
                zip::copy_entry(&mut r, e, &mut w)?;
                std::io::Write::flush(&mut w)
                    .map_err(|err| zip::ZipError::Write(inner.clone(), err.to_string()))
            });
        match written {
            Ok(()) => {
                let at = dest.to_string_lossy().into_owned();
                for s in project.audio_pool.iter_mut().filter(|s| s.path == *inner) {
                    s.path.clone_from(&at);
                }
            }
            Err(_) => {
                let _ = std::fs::remove_file(&dest);
                unwritten.push(inner.clone());
            }
        }
    }
    find_external(&mut project, &mut report, &carried, path.parent());
    Ok(ImportedFile {
        project,
        report,
        unwritten,
    })
}

/// The sources a `.dawproject` references outside its archive (`carried`: the ones it holds)
/// found against `folder`, the archive's own (DAWproject's rule), and named by their
/// absolute path; one neither in the archive nor on disk is said.
fn find_external(
    project: &mut Project,
    report: &mut Report,
    carried: &[String],
    folder: Option<&std::path::Path>,
) {
    for s in &mut project.audio_pool {
        if carried.contains(&s.path) {
            continue;
        }
        let p = std::path::Path::new(&s.path);
        let found = match folder {
            Some(f) if p.is_relative() => f.join(p),
            _ => p.to_path_buf(),
        };
        if found.is_file() {
            s.path = found.to_string_lossy().into_owned();
        } else {
            report.dropped(
                "interop.report.d_audio_missing",
                vec![("file", s.path.clone())],
            );
        }
    }
}

/// The name a Cantus effect goes by in another application's device list.
pub fn effect_device_name(kind: EffectKind) -> String {
    match kind {
        EffectKind::Eq => "Aequalis".into(),
        EffectKind::Compressor => "Comprimo".into(),
        EffectKind::Limiter => "Terminus".into(),
        EffectKind::Space => "Spatium".into(),
        EffectKind::Multiband => "Multiplex".into(),
        EffectKind::DeEsser => "Sibilus".into(),
        EffectKind::Gate => "Porta".into(),
        EffectKind::Stereo => "Latitudo".into(),
        EffectKind::Saturator => "Fervor".into(),
        EffectKind::Maximizer => "Culmen".into(),
        EffectKind::Chorus => "Concentus".into(),
        EffectKind::Flanger => "Flexus".into(),
        EffectKind::Phaser => "Phasis".into(),
        EffectKind::Tremolo => "Tremor".into(),
        EffectKind::Vibrato => "Vibrans".into(),
        EffectKind::RingMod => "Anulus".into(),
        EffectKind::FreqShift => "Translatio".into(),
        EffectKind::VolumeShaper => "Fluctus".into(),
        EffectKind::Clipper => "Tonsor".into(),
        EffectKind::Transient => "Impetus".into(),
        EffectKind::TransientSplit => "Discrimen".into(),
        EffectKind::Analyzer => "Speculum".into(),
        EffectKind::AutoTune => "Tonus".into(),
        EffectKind::Delay => "Delay".into(),
        EffectKind::Reverb => "Reverb".into(),
        #[allow(unreachable_patterns)]
        other => kind_id(other),
    }
}

/// `cantus.<kind>`: the kind's name in the project file, lower-cased. Stable because the
/// project file's own names are.
pub fn effect_device_id(kind: EffectKind) -> String {
    format!("{DEVICE_PREFIX}{}", kind_id(kind))
}

fn kind_id(kind: EffectKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_lowercase))
        .unwrap_or_else(|| "effect".into())
}

/// The kind a `cantus.<kind>` device id names.
pub fn effect_kind_of(device_id: &str) -> Option<EffectKind> {
    let id = device_id.strip_prefix(DEVICE_PREFIX)?;
    EffectKind::ALL.into_iter().find(|k| kind_id(*k) == id)
}
