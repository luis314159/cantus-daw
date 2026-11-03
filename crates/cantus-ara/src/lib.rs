//! Hosting ARA 2 plug-ins (ADR 0097).
//!
//! ARA (Audio Random Access, Celemony, Apache-2.0) lets a plug-in such as a pitch or
//! spectral editor see a whole recording and the clips that play it, not just the block
//! the host hands it. This crate is the host's side, independent of the plug-in format:
//!
//! - [`sys`]: the C API's bindings, generated once from Celemony's tagged release
//!   (`releases/2.3.0`, see `sys.rs`'s header);
//! - [`vst3`]: ARA's VST3 binding (`IMainFactory`, `IPlugInEntryPoint2`), by hand;
//! - [`document`]: **safe and pure**. The host document (audio sources and playback
//!   regions in seconds) built from a project, and the ordered updates between two states;
//! - [`callbacks`]: the controller interfaces the plug-in calls back into (reading audio,
//!   archives, tempo and bars, model updates, playback requests);
//! - [`host`]: one plug-in's document controller and the objects Cantus keeps in it.
//!
//! Binding a VST3 instance to a document, rendering a clip and the session that runs it
//! are `cantus-host`'s (`cantus_host::ara`).
//!
//! The crate is GPL-3.0-or-later. ARA types and the unsafe they need never enter the MPL
//! crates (`cantus-core` knows only plain data, `cantus_core::ara`) nor the engine
//! (`forbid(unsafe_code)`).

#![allow(clippy::missing_safety_doc)]
#![allow(non_snake_case)]

pub mod callbacks;
pub mod document;
pub mod host;
pub mod sys;
pub mod vst3;
