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
pub mod drumgizmo;
pub mod fetch;
pub mod install;
pub mod kitbuild;
pub mod library;
pub mod manager;
pub mod manifest;
// r34:stems — machine-learning models (ADR 0085).
pub mod models;
pub mod models_pickle;
pub mod models_th;
pub mod pianobuild; // r40:chorda-src — a piano written as an SFZ from its naming (ADR 0125)
pub mod pianobuild_flac; // r40:chorda-src — its WAVs kept as FLAC, checked bit for bit
pub mod strings_analysis; // r42:strings-rec — pitch, loops and clicks, measured offline (ADR 0158)
pub mod stringsbuild; // r42:strings-rec — a recorded strings instrument written at install
pub mod tar;
pub mod user_pack; // r40:chorda-src — Install from a file… (ADR 0125 §3, §4)
pub mod wav;

pub use install::{Layout, Phase, Receipt};
pub use manager::{Finished, InstalledInstrument, Manager, Status};
pub use manifest::{Attribution, Build, Category, Licence, Manifest, Pack, Source};
