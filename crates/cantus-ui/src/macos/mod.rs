//! macOS's own parts of the interface (round 30, ADR 0066): the global menu bar. The model
//! and the paths it runs are plain Rust, tested on every system; the AppKit side is
//! `cantus_launch::macmenu` (this crate has no unsafe code).

pub mod menu;
#[cfg(test)]
mod tests;
