mod app_icon;
// CANTUS PATCH (ADR 0148 §10): the windows' winit handles.
#[cfg(feature = "glow")]
pub mod cantus_windows;
mod epi_integration;
mod event_loop_context;
pub mod run;

#[cfg(target_os = "macos")]
pub(crate) mod macos;

/// File storage which can be used by native backends.
#[cfg(feature = "persistence")]
pub mod file_storage;

pub(crate) mod winit_integration;

#[cfg(feature = "glow")]
mod glow_integration;

#[cfg(feature = "wgpu_no_default_features")]
mod wgpu_integration;
