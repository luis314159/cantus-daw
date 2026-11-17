//! Cedo's editor: the ducker.
//!
//! **The fx side round's wave-B contract**: the device frame around one card with a cell per
//! parameter, until its builder draws the real editor. The public items are the ones every
//! editor of the family has (`views::fx` calls them).

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

const KIND: EffectKind = EffectKind::Ducker;

/// Factory presets: a label key and the parameters it sets (the factory presets are data,
/// `crates/cantus-ui/presets/`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The window's designed size at 100 % (the frame's `designed_size`); the design sheet
/// sets it so every row fits whole in both languages (the window grows, the display never
/// shrinks).
pub const DESIGNED_SIZE: egui::Vec2 = egui::Vec2::new(1340.0, 900.0);

/// The states the design sheets and `fxb_whole` draw it in: a name and the settings over
/// the demo (the first is the starting preset as inserted).
pub const DESIGN_STATES: &[(&str, &[(&str, f32)])] = &[
    ("start", &[]),
    (
        "sidechain",
        &[
            ("trig", 0.0),
            ("depth", 18.0),
            ("kfilt", 2.0),
            ("khz", 60.0),
            ("look", 2.0),
        ],
    ),
    ("lows", &[("band", 1.0), ("xover", 1.0), ("xhz", 150.0)]),
    ("midi", &[("trig", 2.0), ("depth", 24.0), ("vel", 1.0)]),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
}

/// Cedo's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.cedo);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "cedo"), &mut st, ui);
    app.fx.cedo = st;
    out
}

/// Cedo's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
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

/// The settings `--editor cedo` and the screenshot tool open it with.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let _ = (app, strip, effect);
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let _ = app;
}
