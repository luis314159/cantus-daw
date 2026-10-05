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
    pub report: Report,
}

/// Write a project as a `.dawproject`.
pub fn export(project: &Project, app_version: &str) -> Exported {
    let parts = write::parts(project, app_version);
    let mut z = zip::ZipWriter::new();
    z.add("project.xml", parts.project_xml.as_bytes(), true);
    z.add("metadata.xml", parts.metadata_xml.as_bytes(), true);
    for (path, data) in &parts.files {
        z.add(path, data, true);
    }
    Exported {
        bytes: z.finish(),
        report: parts.report,
    }
}

/// A project read from a `.dawproject`, and what it could not keep.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    pub project: Project,
    pub report: Report,
}

/// Read a `.dawproject` into a new project.
pub fn import(bytes: &[u8]) -> Result<Imported, DawProjectError> {
    let files = zip::read(bytes)?;
    read::project(&files)
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
