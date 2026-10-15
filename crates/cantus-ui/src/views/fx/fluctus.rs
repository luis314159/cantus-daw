//! Fluctus's editor: the volume shaper (ADR 0055).
//!
//! The references are Xfer's LFO Tool and Nicky Romero's Kickstart: the first a curve you
//! draw, the second one knob and a few shapes. Fluctus is the curve, drawn with the kit and
//! edited as the automation editor's clips are (the same six segment shapes and tension):
//! drag a point (Shift: off the sixteenth grid), double-click to add one or take it away,
//! drag a segment up or down to bend it, choose the selected point's segment shape below.
//! The playhead is the song's; the curve's length is a note value. Pumping presets cover
//! the classic sidechain shapes without a key.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx;
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::fluctus::{Curve, FluctusSettings};
use egui::Rect;
use std::fmt::Write as _;

#[path = "fluctus_curve.rs"]
mod curve;

pub use curve::curve_points;

const KIND: EffectKind = EffectKind::VolumeShaper;

/// A preset: its globals, then its points `p{n}.on/x/y/t/s` from `p0` up. What it does not
/// name goes back to its default — points from `p2` on are unused by default, and every
/// preset draws `p0` and `p1` — so a preset's curve is exactly the points it lists.
macro_rules! preset {
    ([$(($gk:literal, $gv:expr)),* $(,)?]; $($n:literal: $x:expr, $y:expr, $t:expr, $s:expr);* $(;)?) => {
        &[
            $(($gk, $gv),)*
            $(
                (concat!("p", $n, ".on"), 1.0),
                (concat!("p", $n, ".x"), $x),
                (concat!("p", $n, ".y"), $y),
                (concat!("p", $n, ".t"), $t),
                (concat!("p", $n, ".s"), $s),
            )*
        ]
    };
}

/// Factory presets: the classic pumps without a key, gates and swells.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.fluctus.pump_quarters",
        preset!([("depth", 1.0), ("div", 8.0), ("smooth", 2.0)];
            0: 0.0, 0.0, -0.6, 0.0; 1: 0.5, 1.0, 0.0, 2.0),
    ),
    (
        "preset.fluctus.pump_hard",
        preset!([("depth", 1.0), ("div", 8.0), ("smooth", 1.0)];
            0: 0.0, 0.0, -0.9, 0.0; 1: 0.3, 1.0, 0.0, 2.0),
    ),
    (
        "preset.fluctus.pump_soft",
        preset!([("depth", 0.7), ("div", 8.0), ("smooth", 4.0)];
            0: 0.0, 0.25, -0.3, 0.0; 1: 0.75, 1.0, 0.0, 2.0),
    ),
    (
        "preset.fluctus.pump_eighths",
        preset!([("depth", 0.9), ("div", 11.0), ("smooth", 1.5)];
            0: 0.0, 0.0, -0.6, 0.0; 1: 0.5, 1.0, 0.0, 2.0),
    ),
    (
        "preset.fluctus.half_time",
        preset!([("depth", 1.0), ("div", 5.0), ("smooth", 3.0)];
            0: 0.0, 0.0, -0.6, 0.0; 1: 0.5, 1.0, 0.0, 2.0),
    ),
    (
        "preset.fluctus.swell_bar",
        preset!([("depth", 1.0), ("div", 3.0), ("smooth", 5.0)];
            0: 0.0, 0.0, 0.5, 0.0; 1: 0.95, 1.0, 0.0, 0.0),
    ),
    (
        "preset.fluctus.gate_sixteenths",
        preset!([("depth", 1.0), ("div", 5.0), ("smooth", 1.0)];
            0: 0.0, 1.0, 0.0, 2.0; 1: 0.125, 0.0, 0.0, 2.0;
            2: 0.25, 1.0, 0.0, 2.0; 3: 0.375, 0.0, 0.0, 2.0;
            4: 0.5, 1.0, 0.0, 2.0; 5: 0.625, 0.0, 0.0, 2.0;
            6: 0.75, 1.0, 0.0, 2.0; 7: 0.875, 0.0, 0.0, 2.0),
    ),
    (
        "preset.fluctus.offbeat",
        preset!([("depth", 1.0), ("div", 8.0), ("smooth", 2.0)];
            0: 0.0, 0.0, 0.0, 2.0; 1: 0.5, 1.0, 0.0, 2.0),
    ),
    (
        "preset.fluctus.triplet_chop",
        preset!([("depth", 1.0), ("div", 8.0), ("smooth", 1.0)];
            0: 0.0, 1.0, 0.0, 2.0; 1: 0.2, 0.0, 0.0, 2.0;
            2: 0.333, 1.0, 0.0, 2.0; 3: 0.533, 0.0, 0.0, 2.0;
            4: 0.667, 1.0, 0.0, 2.0; 5: 0.867, 0.0, 0.0, 2.0),
    ),
    (
        "preset.fluctus.slow_wave",
        preset!([("depth", 0.6), ("div", 3.0), ("smooth", 0.0)];
            0: 0.0, 0.2, 0.4, 5.0; 1: 0.5, 1.0, 0.4, 5.0),
    ),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The point whose segment the shape row edits.
    pub selected: Option<usize>,
    /// A drag on the curve.
    drag: Option<curve::Grab>,
    /// Numbers that fold the steps of one drag into one undo step.
    gesture: u64,
    /// Where the curve's plot was drawn last frame (tests point at it).
    pub plot: Option<Rect>,
    /// The title's words ("1/4 · 128 BPM"), formatted when they change.
    title: (f32, f32, String),
}

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.fluctus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "fluctus"),
        &mut st,
        ui,
    );
    app.fx.fluctus = st;
    out
}

pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: modfx::out_meter(&tel),
        more: &[],
    };
    let fs = cantus_engine::fx::fluctus::settings(host.spec());
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &fs)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

const SHAPE_W: f32 = 300.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    fs: &FluctusSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(220.0 * k);
    let r1: Vec<Rect> = g.row(row1, &[1.0]).collect();
    let sm = s.function(Function::Dynamics);
    curve::card(host, st, ui, &sm, r1[0], tel, fs);

    let w = [
        controls::cells_width(&[Size::XL, Size::M]) + 16.0,
        controls::cells_width(&[Size::M, Size::M]) + 16.0,
        SHAPE_W + 4.0 + controls::cells_width(&[Size::M]) + 16.0,
        controls::cells_width(&[Size::M]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(
        ui,
        &sm,
        r2[0],
        Src::Key("fluctus.card.depth"),
        None,
        |ui, _| {
            controls::param_with(ui, &sm, host, "depth", Size::XL, Some(&modfx::pct_pill));
            controls::param_with(ui, &sm, host, "smooth", Size::M, Some(&modfx::ms_pill));
        },
    );
    let smod = s.function(Function::Mod);
    controls::card(
        ui,
        &smod,
        r2[1],
        Src::Key("fluctus.card.cycle"),
        None,
        |ui, _| {
            controls::param_with(ui, &smod, host, "div", Size::M, Some(&modfx::div_pill));
            controls::param_with(ui, &smod, host, "offset", Size::M, Some(&modfx::pct_pill));
        },
    );
    curve::point_card(host, st, ui, &sm, r2[2], fs, SHAPE_W);
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        r2[3],
        Src::Key("fluctus.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "out", Size::M, Some(&modfx::db_pill));
        },
    );
}

/// The level the curve gives at `phase`, with the depth in it: what the editor draws.
pub fn level_at(fs: &FluctusSettings, curve: &Curve, phase: f32) -> f32 {
    1.0 - fs.depth * (1.0 - curve.at(phase))
}

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let fs = cantus_engine::fx::fluctus::settings(spec);
    let c = Curve::new(&fs.points, cantus_engine::fx::fluctus::shape_at);
    let i = (spec.get("div").round().max(0.0) as usize).min(modfx::DIV_NAMES.len() - 1);
    let words = DivWords(modfx::DIV_NAMES[i]);
    modfx::preview_strip(ui, spec, frame::tr("fluctus.preview"), &words, &|cyc| {
        2.0 * c.at((cyc - cyc.floor()) as f32) - 1.0
    })
}

/// A cycle's note value, for the preview's line.
struct DivWords(&'static str);

impl std::fmt::Display for DivWords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// The settings `--editor fluctus` and the screenshot tool open it with: the classic pump
/// on the quarters, a third point bending the recovery.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "fluctus").set_many(
        &[
            ("depth", 0.9),
            ("p2.on", 1.0),
            ("p2.x", 0.25),
            ("p2.y", 0.8),
            ("p2.t", 0.4),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::fluctus::Fluctus::new(48_000.0, cantus_engine::fx::fluctus::shape_at);
    d.set(&cantus_engine::fx::fluctus::settings(&spec));
    d.reset();
    modfx::demo_run(
        &mut |l, r, pos| {
            d.set_transport(&pos);
            d.process(l, r);
        },
        0.33,
    );
    let mut t = [0.0f32; 64];
    d.telemetry(&mut t);
    app.fx.injected = Some(t);
}

/// "1/4 · 128 BPM" into `out`.
fn title_words(fs_div: f32, tempo: f32, out: &mut String) {
    let i = (fs_div.round().max(0.0) as usize).min(modfx::DIV_NAMES.len() - 1);
    let _ = write!(out, "{} \u{b7} {:.0} BPM", modfx::DIV_NAMES[i], tempo);
}
