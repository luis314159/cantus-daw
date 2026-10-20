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
    let mut z = zip::ZipStream::new(Vec::new());
    let bytes = write_parts(&mut z, &mut parts)
        .and_then(|()| z.finish())
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
    let mut parts = write::parts_with(project, app_version, base);
    let tmp = dest.with_extension("dawproject.part");
    let out = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
    let mut z = zip::ZipStream::new(out);
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
fn write_parts<W: std::io::Write>(
    z: &mut zip::ZipStream<W>,
    parts: &mut Parts,
) -> std::io::Result<()> {
    z.add("project.xml", parts.project_xml.as_bytes(), true)?;
    z.add("metadata.xml", parts.metadata_xml.as_bytes(), true)?;
    for (path, data) in &parts.files {
        z.add(path, data, true)?;
    }
    for (path, file) in &parts.audio {
        match std::fs::read(file) {
            Ok(bytes) => z.add(path, &bytes, false)?,
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
    for s in &mut imported.project.audio_pool {
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
            imported.report.dropped(
                "interop.report.d_audio_missing",
                vec![("file", s.path.clone())],
            );
        }
    }
    Ok(imported)
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
