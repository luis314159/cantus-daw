//! Chorda — the modelled grand piano: its editor. Owned by this instrument's builder
//! (ADR 0023).
//!
//! **Round 28's contract**: the frame's generic views — the Play view's cells, the Edit
//! view's cell per parameter, the inspector's name line — until its builder draws the
//! piano's own picture.

use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::Style;
use cantus_core::{NativeSpec, TrackId};

/// Factory presets, with tags.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The Play view: the key parameters, live.
pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    super::default_play(app, ui, track, spec, s);
}

/// The Edit view: every parameter.
pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    super::knobs_for_every_param(app, ui, track, spec, s);
}

/// The inspector's small read-only picture.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    super::default_preview(ui, spec)
}

/// `--editor chorda`: an instrument that shows its display off.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    let _ = (app, track);
}

/// For screenshots and tests with no engine: what a playing engine would report.
pub fn demo_feed(app: &mut CantusApp) {
    let _ = app;
}
