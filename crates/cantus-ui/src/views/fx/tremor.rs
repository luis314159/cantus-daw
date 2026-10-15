//! Tremor's editor: tremolo, auto-pan and the harmonic tremolo (ADR 0055).
//!
//! The references (FL's Fruity Panomatic and its tremolo presets, Soundtoys Tremolator,
//! Waves' and Logic's tremolos, Kilohearts Trance Gate) agree on the controls and differ
//! on what they show: most show a knob's value and nothing of the movement. Here the
//! **movement display** draws what the level (or the pan, or the two halves of the
//! harmonic mode) will do over the next two cycles — the shape, the soft edge, the swing,
//! the stereo spread — with the playhead where the unit is now, and the rate in words
//! ("1/8 · 128 BPM").

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx::{self, LfoCard};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::tremor::{Mode, TremorSettings};
use egui::{pos2, Rect};

const KIND: EffectKind = EffectKind::Tremolo;

/// Factory presets: a label key and the parameters it sets (what a preset does not name
/// goes back to its default).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.tremor.surf",
        &[("depth", 0.65), ("sync", 0.0), ("rate", 6.2), ("shape", 1.0)],
    ),
    (
        "preset.tremor.slow_swell",
        &[("depth", 0.5), ("div", 5.0), ("shape", 0.0)],
    ),
    (
        "preset.tremor.helicopter",
        &[("depth", 1.0), ("div", 14.0), ("shape", 2.0), ("edge", 0.1)],
    ),
    (
        "preset.tremor.trance_gate",
        &[
            ("depth", 1.0),
            ("div", 14.0),
            ("shape", 2.0),
            ("edge", 0.05),
            ("swing", 0.5),
        ],
    ),
    (
        "preset.tremor.stereo_shimmer",
        &[
            ("depth", 0.6),
            ("div", 13.0),
            ("shape", 0.0),
            ("spread", 180.0),
        ],
    ),
    (
        "preset.tremor.random_steps",
        &[("depth", 0.6), ("div", 11.0), ("shape", 5.0), ("edge", 0.4)],
    ),
    (
        "preset.tremor.pan_quarters",
        &[("mode", 1.0), ("depth", 0.8), ("div", 8.0), ("shape", 0.0)],
    ),
    (
        "preset.tremor.wide_pan",
        &[("mode", 1.0), ("depth", 1.0), ("div", 3.0), ("shape", 1.0)],
    ),
    (
        "preset.tremor.harmonic",
        &[
            ("mode", 2.0),
            ("depth", 0.8),
            ("sync", 0.0),
            ("rate", 4.5),
            ("xover", 700.0),
        ],
    ),
];

/// The modes' names and what each does, by index.
const MODE_KEYS: [&str; 3] = ["tremor.mode.0", "tremor.mode.1", "tremor.mode.2"];
const MODE_WORDS: [&str; 3] = ["tremor.words.0", "tremor.words.1", "tremor.words.2"];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// Where the movement display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
}

/// Tremor's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.tremor);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "tremor"), &mut st, ui);
    app.fx.tremor = st;
    out
}

/// Tremor's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
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
    let ts = cantus_engine::fx::tremor::settings(host.spec());
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ts)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

const LFO: LfoCard = LfoCard {
    shapes: &cantus_dsp::modfx_lfo::Shape::ALL,
    edge: true,
    swing: true,
    spread: true,
    offset: true,
};

/// The modes' card width at 100 %.
const MODE_W: f32 = 196.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ts: &TremorSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(160.0 * k);
    let total = area.width() - gap;
    let mode_w = MODE_W * k;
    let r1: Vec<Rect> = g
        .row(row1, &[mode_w / total, (total - mode_w) / total])
        .collect();
    modfx::mode_card(
        host,
        ui,
        &s.function(Function::Mod),
        r1[0],
        "mode",
        "fx.tremor.mode",
        &MODE_KEYS,
        &MODE_WORDS,
        &|b, r, i, ink| mode_glyph(b, r, i, ink, k),
    );
    display_card(host, st, ui, s, r1[1], tel, ts);

    let w = [
        controls::cells_width(&[Size::XL]) + 16.0,
        modfx::lfo_card_width(&LFO),
        controls::cells_width(&[Size::M, Size::M]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(ui, s, r2[0], Src::Key("tremor.card.depth"), None, |ui, _| {
        controls::param_with(ui, s, host, "depth", Size::XL, Some(&modfx::pct_pill));
    });
    modfx::lfo_card(host, ui, &s.function(Function::Mod), r2[1], &LFO);
    let so = s.function(Function::Output);
    controls::card(ui, &so, r2[2], Src::Key("tremor.card.output"), None, |ui, _| {
        ui.add_enabled_ui(ts.mode == Mode::Harmonic, |ui| {
            controls::param(ui, &so, host, "xover", Size::M);
        });
        controls::param_with(ui, &so, host, "out", Size::M, Some(&modfx::db_pill));
    });
}

/// The movement display in a card: the level (or the pan, or the two halves) over two
/// cycles.
fn display_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    ts: &TremorSettings,
) {
    let (top, bottom, title) = match ts.mode {
        Mode::Tremolo => ("tremor.top.level", "tremor.bottom.level", "tremor.card.level"),
        Mode::Pan => ("tremor.top.pan", "tremor.bottom.pan", "tremor.card.pan"),
        Mode::Harmonic => ("tremor.top.level", "tremor.bottom.level", "tremor.card.halves"),
    };
    let s = s.function(Function::Mod);
    controls::card(ui, &s, rect, Src::Key(title), None, |ui, cr| {
        let r = cr.body;
        st.display = Some(r);
        let depth = ts.depth;
        let mode = ts.mode;
        let curve = |c: f64| -> (f32, f32) {
            let (l, rr) = modfx::lfo_at(&ts.lfo, ts.spread, c);
            match mode {
                Mode::Tremolo => {
                    let g = |m: f32| 1.0 - depth * (0.5 - 0.5 * m);
                    (2.0 * g(l) - 1.0, 2.0 * g(rr) - 1.0)
                }
                Mode::Pan => (depth * l, depth * rr),
                Mode::Harmonic => {
                    let lo = 1.0 - depth * (0.5 - 0.5 * l);
                    let hi = 1.0 - depth * (0.5 + 0.5 * l);
                    (2.0 * lo - 1.0, 2.0 * hi - 1.0)
                }
            }
        };
        modfx::movement(ui, &s, r, host, tel, &curve, frame::tr(top), frame::tr(bottom));
        if depth == 0.0 {
            modfx::idle_note(ui, &s, r, "tremor.idle");
        }
    });
}

/// A mode's picture: a wave under an envelope (tremolo), a ball between two sides (pan),
/// two waves crossing (harmonic).
fn mode_glyph(b: &mut crate::kit::Brush, r: Rect, i: usize, ink: egui::Color32, k: f32) {
    const N: usize = 25;
    let mut a = [egui::Pos2::ZERO; N];
    let mut c = [egui::Pos2::ZERO; N];
    for j in 0..N {
        let x = j as f32 / (N - 1) as f32;
        let w = cantus_dsp::math::sin(std::f32::consts::TAU * x);
        let x_at = r.left() + r.width() * x;
        let (ya, yc) = match i {
            0 => (0.5 + 0.5 * w, 0.0),
            1 => (w, -w),
            _ => (0.5 + 0.5 * w, 0.5 - 0.5 * w),
        };
        a[j] = pos2(x_at, r.center().y - ya * r.height() * 0.45);
        c[j] = pos2(x_at, r.center().y - yc * r.height() * 0.45);
    }
    b.polyline(&a, 1.5 * k, ink);
    if i != 0 {
        b.polyline(&c, 1.0 * k, crate::kit::color::with_alpha(ink, 0.5));
    } else {
        b.hline(r.left(), r.right(), r.center().y + r.height() * 0.0, k, ink);
    }
}

/// The inspector's picture: the movement in a strip, and the settings in one line.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let ts = cantus_engine::fx::tremor::settings(spec);
    let mode = frame::tr(MODE_KEYS[ts.mode as usize]);
    modfx::preview_strip(ui, spec, mode, &modfx::Pct(ts.depth), &|c| {
        let (l, _) = modfx::lfo_at(&ts.lfo, ts.spread, c);
        match ts.mode {
            Mode::Pan => ts.depth * l,
            _ => 2.0 * (1.0 - ts.depth * (0.5 - 0.5 * l)) - 1.0,
        }
    })
}

/// The settings `--editor tremor` and the screenshot tool open it with: a gated pulse on
/// the eighths, the right channel a quarter cycle ahead.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "tremor").set_many(
        &[
            ("depth", 0.75),
            ("div", 11.0),
            ("shape", 2.0),
            ("edge", 0.35),
            ("swing", 0.25),
            ("spread", 90.0),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor, for screenshots with no engine: the DSP
/// run for a moment at 128 BPM.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::tremor::Tremor::new(48_000.0);
    d.set(&cantus_engine::fx::tremor::settings(&spec));
    d.reset();
    let mut t = [0.0f32; 64];
    modfx::demo_run(
        &mut |l, r, pos| {
            d.set_transport(&pos);
            d.process(l, r);
        },
        0.6,
    );
    d.telemetry(&mut t);
    app.fx.injected = Some(t);
}
