//! Nitor's editor (round 41, ADR 0151): the one-knob enhancer. Owned by the `nitor` builder,
//! from a design sheet the owner approves first: the one big *amount* knob at its heart, the
//! character chosen, the low end and its key, the exciter's bands — a face unlike every other
//! unit's, in both themes.
//!
//! **Contract stub**: the device frame (preset, A/B, the output meter) around an empty body,
//! so every dispatch exists from the first day.

use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::Style;
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::{Rect, Sense, Vec2};

const KIND: EffectKind = EffectKind::Enhancer;

/// Factory presets: data (`presets/effects/enhancer.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The designed size at 100 % (the builder's own once its face is drawn).
pub fn designed_size() -> Vec2 {
    Vec2::new(900.0, 560.0)
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
}

/// Nitor's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.nitor);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "nitor"), &mut st, ui);
    app.fx.nitor = st;
    out
}

/// Nitor's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: super::modfx::out_meter(&tel),
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, _s, body, _host| {
        body_ui(ui, body)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The body: empty until the builder draws Nitor's face.
fn body_ui(ui: &mut egui::Ui, body: Rect) {
    ui.allocate_rect(body, Sense::hover());
}

/// The inspector's small picture: an empty line until the builder draws it.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let w = ui.available_width().max(s.s(160.0));
    let (_rect, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(18.0)), Sense::hover());
    resp
}

/// `--editor nitor`: its defaults.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let _ = (app, strip, effect);
}

/// For screenshots and tests with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let _ = app;
}
