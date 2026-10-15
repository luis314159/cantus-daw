//! Vibrans's editor: the vibrato.
//!
//! **Round 28's contract**: the device frame around one card with a cell per parameter,
//! until its builder draws the real editor. The public items are the ones every editor
//! of the family has (`views::fx` calls them).

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::Style;
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::Rect;

const KIND: EffectKind = EffectKind::Vibrato;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
}

/// Vibrans's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.vibrans);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "vibrans"),
        &mut st,
        ui,
    );
    app.fx.vibrans = st;
    out
}

/// Vibrans's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: None,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, ui, s, body)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// One card: a cell per parameter.
fn body_ui(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, body: Rect) {
    let mut g = Grid::new(body, s);
    let h = controls::card_height(Size::M) * s.k;
    let Some(rect) = g.row(h, &[1.0]).next() else {
        return;
    };
    controls::card(ui, s, rect, Src::Key(KIND.name_key()), None, |ui, _| {
        for info in KIND.params() {
            controls::param(ui, s, host, info.key, Size::M);
        }
    });
}

/// The inspector's picture: the name, until the builder draws one.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let name = frame::tr(spec.kind.name_key());
    controls::readout(ui, &s, name, false, name, ui.available_width())
}

/// The settings `--editor vibrans` and the screenshot tool open it with.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let _ = (app, strip, effect);
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let _ = app;
}
