//! # cantus-project
//!
//! The project file: versioned JSON with migrations, integrity hashes and atomic,
//! backup-preserving writes. See `docs/adr/0006-project-format.md`.

#![forbid(unsafe_code)]

pub mod collect;
pub mod dawproject;
pub mod format;
pub mod midi;
pub mod report;
pub mod store;

pub use format::{
    parse, FormatError, LoadReport, ProjectFile, FILE_EXTENSION, FORMAT_ID, SCHEMA_VERSION,
};
pub use report::{Report, ReportItem, Severity};
pub use store::{
    autosave, autosave_path, backup_path, corrupt_path, fallback_autosave_path, load,
    recovery_candidates, save, Loaded, Problem, SaveReport, StoreError,
};
