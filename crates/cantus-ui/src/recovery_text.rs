//! What the status line says when a project cannot be opened or saved (round 22): the
//! kind of problem in words, the file's name, and — when a file is refused — that the
//! session is untouched and where a backup or an autosave of it is.

use crate::i18n::{t, tf};
use cantus_project::{Problem, StoreError};
use std::path::Path;

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

/// What an I/O error kind is, in words.
fn io_words(kind: std::io::ErrorKind) -> String {
    use std::io::ErrorKind as K;
    match kind {
        K::NotFound => t("recovery.io.not_found"),
        K::PermissionDenied | K::ReadOnlyFilesystem => t("recovery.io.denied"),
        K::StorageFull | K::QuotaExceeded => t("recovery.io.full"),
        _ => t("recovery.io.other"),
    }
}

/// The message for a project at `path` that could not be opened.
pub fn open_failed(path: &Path, e: &StoreError) -> String {
    let file = file_name(path);
    let why = match e.problem() {
        Problem::Truncated => tf("recovery.open.truncated", &[("file", &file)]),
        Problem::Damaged => tf("recovery.open.damaged", &[("file", &file)]),
        Problem::Newer { found, supported } => tf(
            "recovery.open.newer",
            &[("file", &file), ("found", &found), ("supported", &supported)],
        ),
        Problem::NotAProject => tf("recovery.open.not_project", &[("file", &file)]),
        Problem::Integrity => tf("recovery.open.damaged", &[("file", &file)]),
        Problem::Io(kind) => tf(
            "recovery.open.io",
            &[("file", &file), ("why", &io_words(kind))],
        ),
    };
    // Where the last good copy is, when there is one beside it.
    let spare = cantus_project::recovery_candidates(path)
        .into_iter()
        .find(|p| cantus_project::load(p, false).is_ok());
    match spare {
        Some(p) => tf(
            "recovery.open.with_spare",
            &[("why", &why), ("spare", &file_name(&p))],
        ),
        None => tf("recovery.open.untouched", &[("why", &why)]),
    }
}

/// The message for a save to `path` that failed: the session is still open and unsaved.
pub fn save_failed(path: &Path, e: &StoreError) -> String {
    let why = match e {
        StoreError::Io(io) => io_words(io.kind()),
        other => other.to_string(),
    };
    let folder = path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    tf(
        "recovery.save_failed",
        &[("file", &file_name(path)), ("folder", &folder), ("why", &why)],
    )
}
