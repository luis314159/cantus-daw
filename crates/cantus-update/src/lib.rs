//! Cantus's updates (ADR 0032): once a day at most, ask GitHub Releases whether a newer
//! Cantus is published; show its notes; download the installer, verify it (size, SHA-256,
//! and the publisher's signature when this build is signed); and run it when the app
//! quits. No telemetry: the two requests carry no identifier, and nothing else is sent.
//!
//! The pieces are plain functions over data, tested without a network:
//! [`version`] (SemVer precedence), [`release`] (the API's answer, the files' names, the
//! `SHA256SUMS.txt` format), [`notes`] (Markdown to lines), [`verify`] (hashes, web pages
//! a proxy sends instead of a file, Authenticode), [`schedule`] (once a day),
//! [`install`] (installed, portable, built from source; handing over to the installer),
//! [`net`] (the HTTPS client behind a trait) and [`updater`] (the threads and the status).

#![forbid(unsafe_code)]

pub mod flexasio;
pub mod install;
pub mod net;
pub mod notes;
pub mod release;
pub mod schedule;
pub mod updater;
pub mod verify;
pub mod version;

pub use install::InstallKind;
pub use release::Release;
pub use updater::{Config, Offer, Status, Updater, REPO};
pub use version::Version;
