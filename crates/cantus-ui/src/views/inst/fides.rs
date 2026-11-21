//! Fides, the resonator instrument: its editor (round 41, ADR 0150). Owned by the `fides`
//! builder, from a design the owner approves first: the exciter and the resonator drawn as
//! the instrument they make (a marimba's bars, a harp's strings, a bowed cello), the body,
//! the kit shell's Play / Edit / Matrix.
//!
//! **Contract stub**: the kind is not in the picker (`fides::READY` is false), so no song
//! shows this; every function draws nothing or an empty well, so the frame's dispatch
//! exists from the first day.

use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::{well, with_brush, Function, State as KitState, Style};
use cantus_core::{NativeSpec, TrackId};
use egui::{Sense, Vec2};

pub fn play(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    body(app, ui, track, spec, s);
}

pub fn body(app: &mut CantusApp, ui: &mut egui::Ui, track: TrackId, spec: &NativeSpec, s: &Style) {
    let _ = (app, ui, track, spec, s);
}

/// The inspector's small picture: an empty well until the builder draws the instrument.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let _ = spec;
    let s = Style::current().function(Function::Source);
    let w = ui.available_width().max(s.s(160.0));
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(36.0)), Sense::hover());
    with_brush(&ui.painter_at(rect), |b| {
        well::paint(b, rect, &s, KitState::REST, None);
    });
    resp
}

/// `--editor fides`: the default resonator.
pub fn demo_setup(app: &mut CantusApp, track: TrackId) {
    let _ = track;
    if !app.engine.is_live() {
        demo_feed(app);
    }
}

/// For screenshots and tests with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    app.inst.injected = Some([0.0; cantus_engine::TELEMETRY_LEN]);
}

/// Factory presets (the builder's, made by ear: 24 at least, ADR 0101's floor).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];
