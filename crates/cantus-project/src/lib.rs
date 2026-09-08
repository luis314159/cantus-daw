//! # cantus-project
//!
//! The project file: versioned JSON with migrations, integrity hashes and atomic,
//! backup-preserving writes. See `docs/adr/0006-project-format.md`.

#![forbid(unsafe_code)]

pub mod format;
pub mod store;

pub use format::{
    parse, FormatError, LoadReport, ProjectFile, FILE_EXTENSION, FORMAT_ID, SCHEMA_VERSION,
};
pub use store::{
    autosave, autosave_path, backup_path, load, recovery_candidates, save, Loaded, SaveReport,
    StoreError,
};
