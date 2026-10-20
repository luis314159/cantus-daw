//! Hosting plug-ins made to the market's standards — CLAP and VST3 — inside Cantus.
//!
//! Cantus's own effects are native (`cantus_engine::fx::FxUnit`, ADR 0018) and stay that
//! way: no format layer between the engine and their DSP. This crate is for everything
//! else — a third party's EQ, a synth someone already owns — and puts every format behind
//! one contract ([`backend`]), so the engine, the delay compensation, the sidechain routing
//! and the interface see a hosted plug-in exactly as they see a native one.
//!
//! The contract is split by thread, because both standards are: a plug-in is loaded,
//! activated, asked for its parameters and state, and given a window on the **main**
//! thread ([`MainThread`]), and processes on the **audio** thread ([`AudioThread`]), where
//! nothing may allocate, lock or panic (ADR 0005).
//!
//! Around the formats, and knowing none of them (ADR 0019):
//! - [`scan`]: finding plug-ins, each file read by a separate process ([`scan::Scanner`],
//!   the `cantus-scan` binary) so one that crashes or hangs is blocked, not fatal;
//! - [`db`]: what the scans found, cached by path and modification time;
//! - [`unit`]: a hosted plug-in behind the engine's `FxUnit`;
//! - [`session`]: the plug-ins of an open project — loading, delivering to the engine,
//!   restarts, their windows' gestures as commands, state for saving;
//! - [`window`]: a plug-in's own editor in a native window;
//! - [`bridge`]: plug-ins run out of Cantus's process, in helper processes (ADR 0063), so
//!   one that crashes or hangs cannot take the session down.

pub mod backend;
pub mod bridge;
pub mod clap;
pub mod db;
pub mod inst_unit;
pub mod scan;
pub mod session;
pub mod unit;
pub mod vst3;
pub mod window;

pub use backend::*;
pub use db::{CrashRecord, DbEntry, PluginDb, ScanStatus};
pub use inst_unit::HostedInstrument;
pub use scan::{ScanReport, Scanner};
pub use session::{HostNotice, HostedInst, PluginHost, RenderFailure};
pub use unit::HostedUnit;

/// Marks the calling thread as the main thread to the plug-ins: the one that loads them and
/// calls their main halves (the plug-in sandbox's main thread does it first, ADR 0063).
/// Loading a CLAP plug-in marks its thread too.
pub fn mark_main_thread() {
    clap::host::mark_main_thread();
}

/// The backend for a format, when this build hosts it.
pub fn backend_for(format: Format) -> Option<Box<dyn Backend>> {
    match format {
        Format::Clap => Some(Box::new(clap::ClapBackend)),
        #[cfg(feature = "vst3")]
        Format::Vst3 => Some(Box::new(vst3::Vst3Backend::new())),
        #[cfg(not(feature = "vst3"))]
        Format::Vst3 => None,
    }
}

/// Every format this build hosts, in the order a plug-in offered in several is preferred.
pub fn backends() -> Vec<Box<dyn Backend>> {
    [Format::Clap, Format::Vst3]
        .into_iter()
        .filter_map(backend_for)
        .collect()
}

/// The project's name for a format, and back.
pub fn to_project_format(f: Format) -> cantus_core::PluginFormat {
    match f {
        Format::Clap => cantus_core::PluginFormat::Clap,
        Format::Vst3 => cantus_core::PluginFormat::Vst3,
    }
}

pub fn from_project_format(f: cantus_core::PluginFormat) -> Format {
    match f {
        cantus_core::PluginFormat::Clap => Format::Clap,
        cantus_core::PluginFormat::Vst3 => Format::Vst3,
    }
}
