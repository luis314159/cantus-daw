//! # cantus-plugins
//!
//! Cantus's seven effects — Aequalis, Comprimo, Terminus, Spatium, Multiplex, Sibilus and
//! Porta — as **CLAP** and **VST3** plug-ins for any other host (ADR 0020).
//!
//! Inside Cantus the effects stay native: nothing here is on the DAW's path. This crate
//! is a separate build (`cargo run -p xtask -- bundle`) that wraps the engine's own
//! [`cantus_engine::EffectUnit`] — the same code, reading the same `EffectSpec` — in the
//! two standards' C ABIs, as thinly as they allow:
//!
//! - [`instance`]: what both formats share — parameter values, when a change takes effect
//!   (the DSPs' 16-sample grid), the soft bypass, latency, state.
//! - [`params`]: the parameter list from Cantus's tables, with ids derived from the keys
//!   by a fixed rule, forever.
//! - [`state`]: the saved state is `EffectSpec` JSON, so presets move both ways.
//! - [`clap`] and [`vst3`]: the entry points (`clap_entry`, `GetPluginFactory`).
//!
//! A development build carries all seven plug-ins in one binary; the bundler builds each
//! on its own (`--no-default-features --features gate`), one binary per plug-in.

pub mod clap;
pub mod desc;
#[cfg(feature = "editor")]
pub mod editor;
pub mod ftz;
pub mod instance;
pub mod params;
pub mod state;
pub mod text;
pub mod vst3;

pub use desc::Descriptor;
pub use instance::{ParamEvent, Processor, Shared};
