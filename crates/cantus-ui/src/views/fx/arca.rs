//! Arca's editor: the amp and its cabinet (fx side round, wave 3, ADR 0137 provisional).
//!
//! *Arca* is Latin for the chest, and the window is one: on the left **the chassis**, a dark
//! window into the amp where its valves stand and glow as hard as each one works (the
//! preamp's, the inverter, the power valves), the supply's needle dipping as it sags, the
//! model's character and its oversampling beside them; under it **the control plate** in
//! green hammered enamel — the pilot light, the preamp, the tone stack, the power amp and the
//! output. On the right **the cabinet**: its speakers, the one the microphone faces drawn
//! large with the microphone on its cone (dragged from the cap to the edge, and back from
//! the grille in a side view), the microphone's kind, and the response the choice makes.

use super::controls;
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::{pos2, Rect};

#[path = "arca_art.rs"]
pub mod art;
#[path = "arca_cab.rs"]
mod cab;
#[path = "arca_display.rs"]
mod display;
#[path = "arca_plate.rs"]
mod plate;
#[path = "arca_words.rs"]
pub mod words;

const KIND: EffectKind = EffectKind::Amp;

/// Factory presets: data (`presets/effects/amp.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The cabinet's column, at 100 %.
const CAB_W: f32 = 452.0;

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    view: display::View,
    cab: cab::View,
    /// Where the chassis and the cabinet were drawn last frame (tests point at them).
    pub display: Option<Rect>,
    pub cabinet: Option<Rect>,
}

/// Arca's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.arca);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "arca"), &mut st, ui);
    app.fx.arca = st;
    out
}

/// Arca's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let (set, _) = cantus_engine::fx::arca::settings(host.spec());
    let latency = set.latency() as f32;
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: None,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The chassis over the control plate, the cabinet beside them.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
) {
    let k = s.k;
    let g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let cab_w = CAB_W * k;
    let left = Rect::from_min_max(area.min, pos2(area.right() - cab_w - gap, area.bottom()));
    let right = Rect::from_min_max(pos2(area.right() - cab_w, area.top()), area.max);
    let plate_h = plate::height(k);
    let chassis = Rect::from_min_max(left.min, pos2(left.right(), left.bottom() - plate_h - gap));
    let plate_r = Rect::from_min_max(pos2(left.left(), left.bottom() - plate_h), left.max);
    st.display = Some(chassis);
    st.cabinet = Some(right);
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    let reading = display::Reading::of(tel);
    st.view.update(&reading, dt);
    display::show(host, &st.view, ui, s, chassis, &reading);
    plate::show(host, ui, s, plate_r, &st.view);
    cab::show(host, &mut st.cab, ui, s, right);
    if st.view.moving() {
        ui.ctx().request_repaint();
    }
}

/// The inspector's picture: the amp and the cabinet in a line ("Crunch · 4×12 vintage").
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let w = words::summary(spec);
    controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// The settings `--editor arca` and the screenshot tool open it with: the starting preset,
/// a clean combo.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "arca").set_many(DEMO, "cmd.fx_preset");
}

/// The starting preset (`cantus_core::starting`): the clean amp at a sweet low gain into its
/// open-backed 1×12.
pub const DEMO: &[(&str, f32)] = &[("model", 1.0), ("gain", 0.3), ("cab", 1.0), ("out", 0.0)];

/// What a playing engine would feed the editor, for screenshots with no engine: a strummed
/// chord through the real unit at the editor's settings, its telemetry as the engine sends
/// it.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let tel = demo_telemetry(&spec);
    app.fx.injected = Some(tel);
    if let Some((_, effect, _)) = app.fx.open {
        if app.fx.arca.effect != Some(effect) {
            app.fx.arca = State {
                effect: Some(effect),
                ..State::default()
            };
        }
        app.fx.arca.view.seed(&display::Reading::of(&tel));
    }
}

/// The telemetry of Arca at `spec` a third of a second into a strummed E major chord.
pub fn demo_telemetry(spec: &EffectSpec) -> [f32; 64] {
    use cantus_engine::FxUnit as _;
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::arca::Unit::new(sr);
    u.configure(spec);
    let notes = [82.41f32, 123.47, 164.81, 207.65, 246.94, 329.63];
    let n = 480;
    for blk in 0..34 {
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
            let t = (blk * n + i) as f32 / sr;
            let mut x = 0.0;
            for (j, f) in notes.iter().enumerate() {
                let start = j as f32 * 0.012;
                if t >= start {
                    let age = t - start;
                    let env = cantus_dsp::math::exp(-2.2 * age);
                    let p = std::f32::consts::TAU * f * age;
                    x += env
                        * (0.10 * cantus_dsp::math::sin(p) + 0.05 * cantus_dsp::math::sin(2.0 * p));
                }
            }
            *a = x;
            *b = x;
        }
        u.process(&mut l, &mut r, None);
    }
    u.telemetry()
}
