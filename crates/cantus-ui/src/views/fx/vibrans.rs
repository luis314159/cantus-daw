//! Vibrans's editor: the vibrato (ADR 0055).
//!
//! The references (Audio Damage Vibrato, Soundtoys' PitchBlender vibrato, the vibrato of
//! amp simulators, FL's Fruity Chorus at one voice) set the depth in milliseconds of
//! delay or in a percentage, so a faster rate quietly makes the pitch swing wider. Vibrans
//! sets it **in cents** and shows the **pitch** it makes over two cycles — a sine's
//! smooth swing, a triangle's two alternating pitches — with the rise drawn beside it.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx::{self, LfoCard};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::modfx_lfo::Shape;
use cantus_dsp::vibrans::{VibransSettings, T_RISE};
use egui::{pos2, Align2, Pos2, Rect};
use std::fmt::Write as _;

const KIND: EffectKind = EffectKind::Vibrato;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.vibrans.singer",
        &[("depth", 30.0), ("rate", 5.5), ("rise", 350.0)],
    ),
    (
        "preset.vibrans.violin",
        &[("depth", 18.0), ("rate", 6.2), ("rise", 250.0)],
    ),
    ("preset.vibrans.organ", &[("depth", 12.0), ("rate", 6.9)]),
    (
        "preset.vibrans.tape_wow",
        &[("depth", 14.0), ("rate", 0.6), ("spread", 30.0)],
    ),
    (
        "preset.vibrans.seasick",
        &[("depth", 70.0), ("rate", 1.2), ("spread", 90.0)],
    ),
    (
        "preset.vibrans.trill",
        &[("depth", 100.0), ("rate", 7.0), ("shape", 1.0)],
    ),
    (
        "preset.vibrans.sync_wobble",
        &[("depth", 40.0), ("sync", 1.0), ("div", 11.0)],
    ),
    (
        "preset.vibrans.chorus_one",
        &[
            ("depth", 12.0),
            ("rate", 0.9),
            ("mix", 0.5),
            ("spread", 90.0),
        ],
    ),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The display's ends ("+30 ct"), formatted when the depth changes.
    ends: (f32, String, String),
    /// Where the pitch display was drawn last frame.
    pub display: Option<Rect>,
}

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
    let vs = cantus_engine::fx::vibrans::settings(host.spec());
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &vs)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

const SHAPES: [Shape; 2] = [Shape::Sine, Shape::Triangle];

const LFO: LfoCard = LfoCard {
    shapes: &SHAPES,
    edge: false,
    swing: false,
    spread: true,
    offset: false,
};

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    vs: &VibransSettings,
) {
    // In its own face, the unit's accent, never the theme's modulation hue (round 32).
    let s = &modfx::body(s);
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(160.0 * k);
    let r1: Vec<Rect> = g.row(row1, &[0.72, 0.28]).collect();
    let sm = modfx::own(s, Function::Mod);
    controls::card(
        ui,
        &sm,
        r1[0],
        Src::Key("vibrans.card.pitch"),
        None,
        |ui, cr| {
            let r = cr.body;
            st.display = Some(r);
            if st.ends.0 != vs.depth_cents || st.ends.1.is_empty() {
                st.ends.0 = vs.depth_cents;
                st.ends.1.clear();
                st.ends.2.clear();
                let _ = write!(st.ends.1, "+{:.0} ct", vs.depth_cents);
                let _ = write!(st.ends.2, "\u{2212}{:.0} ct", vs.depth_cents);
            }
            let curve = |c: f64| pitch_at(vs, c);
            // A tuner's scale and its pointer at the pitch now (round 32).
            let extras = modfx::Extras {
                marks: modfx::face::Marks::Tuner,
                ..Default::default()
            };
            modfx::movement_with(
                ui, &sm, r, host, tel, &curve, &st.ends.1, &st.ends.2, &extras,
            );
            if vs.depth_cents == 0.0 {
                modfx::idle_note(ui, &sm, r, "vibrans.idle");
            }
        },
    );
    controls::card(
        ui,
        &sm,
        r1[1],
        Src::Key("vibrans.card.rise"),
        None,
        |ui, cr| {
            rise_picture(ui, &sm, cr.body, vs, tel);
        },
    );

    let w = [
        controls::cells_width(&[Size::XL, Size::M]) + 16.0,
        modfx::lfo_card_width(&LFO),
        controls::cells_width(&[Size::M, Size::M]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(
        ui,
        s,
        r2[0],
        Src::Key("vibrans.card.depth"),
        None,
        |ui, _| {
            controls::param_with(ui, s, host, "depth", Size::XL, Some(&cents_pill));
            controls::param_with(ui, s, host, "rise", Size::M, Some(&modfx::ms_pill));
        },
    );
    modfx::lfo_card(host, ui, &sm, r2[1], &LFO);
    let so = modfx::own(s, Function::Output);
    controls::card(
        ui,
        &so,
        r2[2],
        Src::Key("vibrans.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "mix", Size::M, Some(&modfx::pct_pill));
            controls::param_with(ui, &so, host, "out", Size::M, Some(&modfx::db_pill));
        },
    );
}

fn cents_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.0} ct");
}

/// The pitch the vibrato makes at `c` cycles in, −1 … 1 of the depth, left and right: the
/// delay's slope (a sine's pitch is its cosine; a triangle's is two pitches in turn).
pub fn pitch_at(vs: &VibransSettings, c: f64) -> (f32, f32) {
    let e = 1e-3;
    let (a0, b0) = modfx::lfo_at(&vs.lfo, vs.spread, c - e);
    let (a1, b1) = modfx::lfo_at(&vs.lfo, vs.spread, c + e);
    let slope = match vs.lfo.shape {
        Shape::Triangle => 4.0,
        _ => std::f32::consts::TAU,
    };
    let d = |x0: f32, x1: f32| -((x1 - x0) / (2.0 * e as f32)) / slope;
    (d(a0, a1).clamp(-1.0, 1.0), d(b0, b1).clamp(-1.0, 1.0))
}

/// The rise: the depth's share against time since a sound starts after a silence, and
/// where it is now.
fn rise_picture(ui: &mut egui::Ui, s: &Style, rect: Rect, vs: &VibransSettings, tel: &[f32; 64]) {
    let ink = Ink::of(s);
    let k = s.k;
    let plot = rect.shrink2(egui::vec2(10.0 * k, 18.0 * k));
    // The rise and half again across, at least 400 ms.
    let span = (vs.rise_ms * 1.5).max(400.0);
    let x_of = |ms: f32| plot.left() + plot.width() * (ms / span).clamp(0.0, 1.0);
    let y_of = |v: f32| plot.bottom() - plot.height() * v;
    let pts: [Pos2; 3] = [
        pos2(x_of(0.0), y_of(if vs.rise_ms > 0.0 { 0.0 } else { 1.0 })),
        pos2(x_of(vs.rise_ms), y_of(1.0)),
        pos2(x_of(span), y_of(1.0)),
    ];
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        b.hline(
            plot.left(),
            plot.right(),
            plot.bottom(),
            b.px(),
            ink.grid_strong,
        );
        b.hline(plot.left(), plot.right(), plot.top(), b.px(), ink.grid);
        b.area(&pts, plot.bottom(), ink.fill(0.5));
        canvas::trace(b, &pts, 2.0 * k, ink.hue, &ink);
        if tel[cantus_dsp::modfx_clock::tel::BLOCKS] > 0.0 {
            let now = tel[T_RISE].clamp(0.0, 1.0);
            let x = x_of(now * vs.rise_ms.max(0.0));
            b.dot(pos2(x, y_of(now)), 3.5 * k, ink.text);
        }
        canvas::putf(
            b,
            pos2(rect.right() - 6.0 * k, rect.top() + 3.0 * k),
            Align2::RIGHT_TOP,
            Role::Value,
            ink.text,
            k,
            Some(ink.plate),
            format_args!("{:.0} ms", vs.rise_ms),
        );
        let t = Text::key("vibrans.after_silence", Role::Micro, ink.dim).caps(true);
        canvas::put(
            b,
            pos2(rect.left() + 6.0 * k, rect.bottom() - 3.0 * k),
            Align2::LEFT_BOTTOM,
            &t,
            k,
            None,
        );
        canvas::edge_b(b, rect, &ink);
    });
}

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let vs = cantus_engine::fx::vibrans::settings(spec);
    let amount = vs.depth_cents / 100.0;
    let depth = modfx::Unit(vs.depth_cents, "ct");
    let words = modfx::PreviewWords {
        mode: frame::tr("vibrans.preview"),
        rate: modfx::PreviewRate::Spec(spec),
        amount: &depth,
    };
    modfx::preview_strip(ui, spec, &words, &|c| pitch_at(&vs, c).0 * amount)
}

/// The settings `--editor vibrans` and the screenshot tool open it with: a singer's
/// vibrato that arrives after the note.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "vibrans").set_many(
        &[
            ("depth", 35.0),
            ("rate", 5.5),
            ("rise", 400.0),
            ("spread", 30.0),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::vibrans::Vibrans::new(48_000.0);
    d.set(&cantus_engine::fx::vibrans::settings(&spec));
    d.reset();
    modfx::demo_run(
        &mut |l, r, pos| {
            d.set_transport(&pos);
            d.process(l, r);
        },
        0.25,
    );
    let mut t = [0.0f32; 64];
    d.telemetry(&mut t);
    app.fx.injected = Some(t);
}
