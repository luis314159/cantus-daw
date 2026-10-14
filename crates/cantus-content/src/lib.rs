//! Cantus's content manager (ADR 0030).
//!
//! Samples do not live in the repository. A small, versioned manifest
//! ([`manifest`], `manifest/content.json`) lists packs — name, licence, upstream URL,
//! sizes and SHA-256 — and the app downloads a pack on demand into the user's data folder
//! (`<data dir>/Content/<pack>/`), verifies every byte, unpacks it, keeps the licence file
//! beside the samples, and writes a receipt. Installs run on worker threads
//! ([`manager::Manager`]); the browser lists folders and searches an index built off the
//! interface thread ([`library`]). [`credits`] names who made every pack (ADR 0042).
//!
//! Network rules: `https://` only, certificates checked against the operating system's
//! store (so a corporate proxy's root, installed by the administrator, is trusted the way
//! the browser trusts it), proxy settings from the environment, a proxy's HTML page
//! recognised and never written into the library, and nothing is ever bypassed.

pub mod credits;
pub mod fetch;
pub mod install;
pub mod library;
pub mod manager;
pub mod manifest;
pub mod tar;

pub use install::{Layout, Phase, Receipt};
pub use manager::{Finished, InstalledInstrument, Manager, Status};
pub use manifest::{Attribution, Category, Licence, Manifest, Pack, Source};
