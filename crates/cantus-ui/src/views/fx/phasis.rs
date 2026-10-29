//! Phasis's editor: the phaser (ADR 0055).
//!
//! The references (the MXR Phase 90 and the Small Stone, FL's Fruity Phaser, Soundtoys
//! PhaseMistress, Kilohearts Phaser) offer stages, rate and feedback; few show where the
//! notches are. Phasis draws the **sweep** — the stages' frequency over two cycles between
//! its bounds — and **the response now**: every notch and peak the cascade, the mix and the
//! feedback make at the present frequency, computed as the unit computes it.

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
use cantus_dsp::phasis::{sweep_hz, PhasisSettings, T_FC_L};
use egui::{pos2, vec2, Align2, Pos2, Rect};
use std::fmt::Write as _;

const KIND: EffectKind = EffectKind::Phaser;

/// Factory presets: a label key and the parameters it sets. The pedals as their circuits
/// sweep: four stages, a few hundred hertz to a couple of kilohertz.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.phasis.phase90_script",
        &[("mix", 0.5), ("lo", 220.0), ("hi", 1_600.0), ("rate", 0.6)],
    ),
    (
        "preset.phasis.phase90_block",
        &[
            ("mix", 0.5),
            ("lo", 220.0),
            ("hi", 1_600.0),
            ("rate", 0.6),
            ("fb", 0.3),
        ],
    ),
    (
        "preset.phasis.small_stone",
        &[
            ("mix", 0.5),
            ("lo", 100.0),
            ("hi", 3_500.0),
            ("rate", 0.3),
            ("fb", 0.5),
        ],
    ),
    (
        "preset.phasis.deep_twelve",
        &[
            ("stages", 12.0),
            ("mix", 0.5),
            ("lo", 80.0),
            ("hi", 6_000.0),
            ("rate", 0.12),
            ("fb", -0.5),
        ],
    ),
    (
        "preset.phasis.synced_eighths",
        &[
            ("stages", 6.0),
            ("mix", 0.5),
            ("sync", 1.0),
            ("div", 11.0),
            ("shape", 1.0),
            ("fb", 0.4),
        ],
    ),
    (
        "preset.phasis.stereo_swirl",
        &[
            ("stages", 6.0),
            ("mix", 0.5),
            ("rate", 0.25),
            ("spread", 180.0),
            ("hi", 5_000.0),
        ],
    ),
    (
        "preset.phasis.random_steps",
        &[
            ("stages", 8.0),
            ("mix", 0.5),
            ("sync", 1.0),
            ("div", 14.0),
            ("shape", 5.0),
            ("edge", 0.5),
            ("fb", 0.6),
        ],
    ),
    (
        "preset.phasis.slow_wash",
        &[
            ("stages", 8.0),
            ("mix", 0.4),
            ("rate", 0.05),
            ("lo", 60.0),
            ("hi", 8_000.0),
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
    ends: (f32, f32, String, String),
    /// Where the sweep display was drawn last frame.
    pub display: Option<Rect>,
}

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.phasis);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "phasis"), &mut st, ui);
    app.fx.phasis = st;
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
    let ps = cantus_engine::fx::phasis::settings(host.spec());
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ps)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

const LFO: LfoCard = LfoCard {
    shapes: &Shape::ALL,
    edge: true,
    swing: false,
    spread: true,
    offset: false,
};

const RESPONSE_W: f32 = 380.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ps: &PhasisSettings,
) {
    // In its own face, the unit's accent, never the theme's modulation hue (round 32).
    let s = &modfx::body(s);
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(200.0 * k);
    let total = area.width() - gap;
    let rw = RESPONSE_W * k;
    let r1: Vec<Rect> = g.row(row1, &[(total - rw) / total, rw / total]).collect();
    let sm = modfx::own(s, Function::Mod);
    sweep_card(host, st, ui, &sm, r1[0], tel, ps);
    response_card(ui, &modfx::own(s, Function::Tone), r1[1], tel, ps);

    let w = [
        controls::cells_width(&[Size::XL, Size::M, Size::M]) + 16.0,
        controls::cells_width(&[Size::M; 3]) + 16.0,
        modfx::lfo_card_width(&LFO),
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(
        ui,
        &sm,
        r2[0],
        Src::Key("phasis.card.mix"),
        None,
        |ui, _| {
            controls::param_with(ui, &sm, host, "mix", Size::XL, Some(&modfx::pct_pill));
            controls::param_with(ui, &sm, host, "fb", Size::M, Some(&fb_pill));
            controls::param_with(ui, &sm, host, "out", Size::M, Some(&modfx::db_pill));
        },
    );
    let st_ = modfx::own(s, Function::Tone);
    controls::card(
        ui,
        &st_,
        r2[1],
        Src::Key("phasis.card.stages"),
        None,
        |ui, _| {
            controls::param_with(ui, &st_, host, "stages", Size::M, Some(&stages_pill));
            controls::param(ui, &st_, host, "lo", Size::M);
            controls::param(ui, &st_, host, "hi", Size::M);
        },
    );
    modfx::lfo_card(host, ui, &sm, r2[2], &LFO);
}

fn fb_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{:+.0}%", v * 100.0);
}

fn stages_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.0}");
}

fn sweep_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    ps: &PhasisSettings,
) {
    let (lo, hi) = (ps.lo_hz.min(ps.hi_hz), ps.hi_hz.max(ps.lo_hz));
    if st.ends.0 != lo || st.ends.1 != hi || st.ends.2.is_empty() {
        st.ends.0 = lo;
        st.ends.1 = hi;
        st.ends.2.clear();
        st.ends.3.clear();
        controls::write_readout(hi, "Hz", &mut st.ends.2);
        controls::write_readout(lo, "Hz", &mut st.ends.3);
    }
    controls::card(
        ui,
        s,
        rect,
        Src::Key("phasis.card.sweep"),
        None,
        |ui, cr| {
            let r = cr.body;
            st.display = Some(r);
            // The LFO's value is the sweep's place on the log scale between the bounds.
            let curve = |c: f64| modfx::lfo_at(&ps.lfo, ps.spread, c);
            modfx::movement(ui, s, r, host, tel, &curve, &st.ends.2, &st.ends.3);
            if ps.mix == 0.0 {
                modfx::idle_note(ui, s, r, "phasis.idle");
            }
        },
    );
}

/// The response of the phaser at the stages' frequency `fc`, dB at `f` Hz, as the unit
/// computes it at 48 kHz: `g_d + g_w·A/(1 − fb·z⁻¹·A)`, `A` the cascade of TPT allpasses
/// `(z⁻¹ − c)/(1 − c·z⁻¹)` with `c = (1 − g)/(1 + g)`, `g = tan(π·fc/fs)`.
pub fn response_db(ps: &PhasisSettings, fc: f32, f: f32) -> f32 {
    let fs = 48_000.0f64;
    let (gd, gw) = cantus_dsp::concentus::mix_gains(ps.mix);
    let w = std::f64::consts::TAU * f64::from(f) / fs;
    let g = cantus_dsp::math::tan(std::f64::consts::PI * f64::from(fc).min(0.45 * fs) / fs);
    let c = (1.0 - g) / (1.0 + g);
    let (s1, c1) = cantus_dsp::math::sin_cos(w);
    let z1 = (c1, -s1);
    let mul = |a: (f64, f64), b: (f64, f64)| (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0);
    let div = |a: (f64, f64), b: (f64, f64)| {
        let d = b.0 * b.0 + b.1 * b.1;
        ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
    };
    let stage = div((z1.0 - c, z1.1), (1.0 - c * z1.0, -c * z1.1));
    let mut a = (1.0, 0.0);
    for _ in 0..ps.stages.clamp(2, 12) {
        a = mul(a, stage);
    }
    let fb = f64::from(ps.feedback);
    let loop_ = {
        let za = mul(z1, a);
        (1.0 - fb * za.0, -fb * za.1)
    };
    let wet = div(a, loop_);
    let y = (f64::from(gd) + f64::from(gw) * wet.0, f64::from(gw) * wet.1);
    (10.0 * cantus_dsp::math::log10((y.0 * y.0 + y.1 * y.1).max(1e-12))) as f32
}

fn response_card(ui: &mut egui::Ui, s: &Style, rect: Rect, tel: &[f32; 64], ps: &PhasisSettings) {
    let seen = tel[cantus_dsp::modfx_clock::tel::BLOCKS] > 0.0;
    let fc = if seen {
        tel[T_FC_L]
    } else {
        sweep_hz(ps.lo_hz, ps.hi_hz, 0.0)
    };
    controls::card(
        ui,
        s,
        rect,
        Src::Key("phasis.card.response"),
        None,
        |ui, cr| {
            let r = cr.body;
            let ink = Ink::of(s);
            let k = s.k;
            let plot = r.shrink2(vec2(8.0 * k, 16.0 * k));
            const N: usize = 240;
            let mut pts = [Pos2::ZERO; N];
            let (top, bottom) = (12.0f32, -36.0f32);
            let x_of =
                |f: f32| plot.left() + plot.width() * cantus_dsp::math::log10(f / 20.0) / 3.0;
            for (i, p) in pts.iter_mut().enumerate() {
                let f = 20.0 * cantus_dsp::math::powf(1000.0f32, i as f32 / (N - 1) as f32);
                let db = response_db(ps, fc, f).clamp(bottom, top);
                *p = pos2(
                    x_of(f),
                    plot.top() + plot.height() * (top - db) / (top - bottom),
                );
            }
            with_brush(&ui.painter_at(r), |b| {
                canvas::ground_b(b, r, &ink, s.hue_tok);
                for db in [0.0f32, -12.0, -24.0] {
                    let y = plot.top() + plot.height() * (top - db) / (top - bottom);
                    let c = if db == 0.0 { ink.grid_strong } else { ink.grid };
                    b.hline(plot.left(), plot.right(), y, b.px(), c);
                }
                // The sweep's bounds and where the stages sit now.
                for f in [ps.lo_hz, ps.hi_hz] {
                    let x = x_of(f.clamp(20.0, 20_000.0));
                    canvas::dashed(
                        b,
                        pos2(x, plot.top()),
                        pos2(x, plot.bottom()),
                        b.px(),
                        ink.faint,
                        4.0 * k,
                        3.0 * k,
                    );
                }
                b.vline(
                    x_of(fc.clamp(20.0, 20_000.0)),
                    plot.top(),
                    plot.bottom(),
                    1.0 * k,
                    ink.text,
                );
                b.area(&pts, plot.bottom(), ink.fill(0.5));
                canvas::trace(b, &pts, 1.6 * k, ink.hue, &ink);
                // Every notch the cascade makes now, marked on the axis, in its own face
                // (round 32).
                if let Some(c) = modfx::face::second_voice(s, &ink) {
                    modfx::face::notches(b, &pts, plot.bottom(), plot.height() * 0.12, k, c);
                }
                for (word, at) in [("100", 100.0f32), ("1k", 1_000.0), ("10k", 10_000.0)] {
                    let t = Text::new(word, Role::Micro, ink.dim);
                    canvas::put(
                        b,
                        pos2(x_of(at), r.bottom() - 2.0 * k),
                        Align2::CENTER_BOTTOM,
                        &t,
                        k,
                        None,
                    );
                }
                canvas::putf(
                    b,
                    pos2(r.right() - 6.0 * k, r.top() + 3.0 * k),
                    Align2::RIGHT_TOP,
                    Role::Value,
                    ink.text,
                    k,
                    Some(ink.plate),
                    format_args!("{:.0} Hz", fc),
                );
                canvas::edge_b(b, r, &ink);
            });
        },
    );
}

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let ps = cantus_engine::fx::phasis::settings(spec);
    let words = modfx::PreviewWords {
        mode: frame::tr("phasis.preview"),
        rate: modfx::PreviewRate::Spec(spec),
        amount: &modfx::Pct(ps.mix),
    };
    modfx::preview_strip(ui, spec, &words, &|c| {
        modfx::lfo_at(&ps.lfo, ps.spread, c).0
    })
}

/// The settings `--editor phasis` and the screenshot tool open it with: a Small Stone with
/// its colour switch on.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "phasis").set_many(
        &[
            ("mix", 0.5),
            ("lo", 100.0),
            ("hi", 3_500.0),
            ("rate", 0.3),
            ("fb", 0.5),
            ("spread", 90.0),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::phasis::Phasis::new(48_000.0);
    d.set(&cantus_engine::fx::phasis::settings(&spec));
    d.reset();
    modfx::demo_run(
        &mut |l, r, pos| {
            d.set_transport(&pos);
            d.process(l, r);
        },
        0.8,
    );
    let mut t = [0.0f32; 64];
    d.telemetry(&mut t);
    app.fx.injected = Some(t);
}
