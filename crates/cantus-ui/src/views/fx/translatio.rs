//! Translatio's editor: the frequency shifter (ADR 0055).
//!
//! A frequency shifter is easy to misread as a pitch shifter. So the editor draws what it
//! does to a **harmonic tone** (220 Hz and its overtones): the partials before, faint, and
//! after, each moved by the same number of hertz — no longer harmonic — with the passes the
//! feedback adds at their levels, and the left and right sidebands in "both". The shift's
//! movement over two LFO cycles sits beside it, and the Hilbert pair's measured quality
//! (its sections and its sideband rejection) is printed in the corner.

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
use cantus_dsp::translatio::{Side, TranslatioSettings, T_REJECTION, T_SECTIONS};
use egui::{pos2, vec2, Align2, Rect};
use std::fmt::Write as _;

const KIND: EffectKind = EffectKind::FreqShift;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.translatio.slow_phase",
        &[("shift", 0.7), ("mix", 0.5)],
    ),
    (
        "preset.translatio.barber_up",
        &[("shift", 3.0), ("fb", 0.7), ("mix", 0.5)],
    ),
    (
        "preset.translatio.barber_down",
        &[("shift", -3.0), ("fb", 0.7), ("mix", 0.5)],
    ),
    (
        "preset.translatio.detune",
        &[("shift", 9.0), ("side", 2.0), ("mix", 0.45)],
    ),
    ("preset.translatio.bell", &[("shift", 173.0), ("mix", 1.0)]),
    (
        "preset.translatio.robot",
        &[("shift", -440.0), ("fb", 0.3), ("mix", 1.0)],
    ),
    (
        "preset.translatio.wobble",
        &[
            ("shift", 0.0),
            ("lfo", 60.0),
            ("sync", 1.0),
            ("div", 8.0),
            ("mix", 0.8),
        ],
    ),
    (
        "preset.translatio.stereo_spin",
        &[("shift", 1.5), ("side", 2.0), ("mix", 1.0)],
    ),
];

const SIDE_KEYS: [&str; 3] = [
    "translatio.side.0",
    "translatio.side.1",
    "translatio.side.2",
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    ends: (f32, f32, String, String),
    /// Where the shift display was drawn last frame.
    pub display: Option<Rect>,
}

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.translatio);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "translatio"),
        &mut st,
        ui,
    );
    app.fx.translatio = st;
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
    let ts = cantus_engine::fx::translatio::settings(host.spec());
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ts)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

const LFO: LfoCard = LfoCard {
    shapes: &Shape::ALL,
    edge: false,
    swing: false,
    spread: false,
    offset: false,
};

const SIDE_W: f32 = 168.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ts: &TranslatioSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(200.0 * k);
    let r1: Vec<Rect> = g.row(row1, &[0.42, 0.58]).collect();
    let sm = s.function(Function::Mod);
    shift_card(host, st, ui, &sm, r1[0], tel, ts);
    partials_card(ui, &s.function(Function::Tone), r1[1], tel, ts);

    let w = [
        controls::cells_width(&[Size::XL, Size::M, Size::M]) + 4.0 + SIDE_W + 16.0,
        modfx::lfo_card_width(&LFO),
        controls::cells_width(&[Size::M; 2]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(
        ui,
        &sm,
        r2[0],
        Src::Key("translatio.card.shift"),
        None,
        |ui, _| {
            controls::param_with(ui, &sm, host, "shift", Size::XL, Some(&shift_pill));
            controls::param_with(ui, &sm, host, "fb", Size::M, Some(&modfx::pct_pill));
            controls::param_with(ui, &sm, host, "lfo", Size::M, Some(&swing_pill));
            controls::column(ui, &sm, SIDE_W, |ui| {
                controls::caption(ui, &sm, Src::Key("translatio.side_caption"), SIDE_W);
                let labels = SIDE_KEYS.map(Src::Key);
                controls::segmented_param(ui, &sm, host, "side", &labels, SIDE_W);
            });
        },
    );
    modfx::region(ui, r2[1], ts.lfo_hz > 0.0, |ui| {
        modfx::lfo_card(host, ui, &sm, r2[1], &LFO);
    });
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        r2[2],
        Src::Key("translatio.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "mix", Size::M, Some(&modfx::pct_pill));
            controls::param_with(ui, &so, host, "out", Size::M, Some(&modfx::db_pill));
        },
    );
}

fn shift_pill(v: f32, b: &mut String) {
    if v.abs() < 10.0 {
        let _ = write!(b, "{v:+.2} Hz");
    } else {
        let _ = write!(b, "{v:+.0} Hz");
    }
}

fn swing_pill(v: f32, b: &mut String) {
    let _ = write!(b, "\u{b1}{v:.0} Hz");
}

fn shift_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    ts: &TranslatioSettings,
) {
    let span = ts.lfo_hz.max(1.0);
    if st.ends.0 != ts.shift_hz || st.ends.1 != span || st.ends.2.is_empty() {
        st.ends.0 = ts.shift_hz;
        st.ends.1 = span;
        st.ends.2.clear();
        st.ends.3.clear();
        let _ = write!(st.ends.2, "{:+.0} Hz", ts.shift_hz + span);
        let _ = write!(st.ends.3, "{:+.0} Hz", ts.shift_hz - span);
    }
    controls::card(
        ui,
        s,
        rect,
        Src::Key("translatio.card.movement"),
        None,
        |ui, cr| {
            let r = cr.body;
            st.display = Some(r);
            let swing = ts.lfo_hz / span;
            let curve = |c: f64| {
                let (l, _) = modfx::lfo_at(&ts.lfo, 0.0, c);
                (l * swing, l * swing)
            };
            modfx::movement(ui, s, r, host, tel, &curve, &st.ends.2, &st.ends.3);
            if ts.mix == 0.0 {
                modfx::idle_note(ui, s, r, "translatio.idle");
            }
        },
    );
}

/// The partials a 220 Hz harmonic tone becomes, left (`0`) and right (`1`): each harmonic
/// moved by the shift (or its mirror below zero), and each feedback pass again, at the
/// feedback's level per pass. Returns how many were written.
pub fn partials(ts: &TranslatioSettings, out: &mut [(f32, f32, u8); 64]) -> usize {
    let mut count = 0;
    let passes = if ts.feedback > 0.0 { 4 } else { 1 };
    for ch in 0..2u8 {
        let sign = match (ts.side, ch) {
            (Side::Up, _) | (Side::Both, 0) => 1.0,
            _ => -1.0,
        };
        for h in 1..=6 {
            let base = 220.0 * h as f32;
            let amp = 1.0 / h as f32;
            for pass in 1..=passes {
                if count >= out.len() {
                    return count;
                }
                let level = amp * cantus_dsp::math::powi(ts.feedback.max(0.0), pass - 1);
                let f = (base + sign * ts.shift_hz * pass as f32).abs();
                out[count] = (f, level, ch);
                count += 1;
            }
        }
    }
    count
}

fn partials_card(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    ts: &TranslatioSettings,
) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("translatio.card.partials"),
        None,
        |ui, cr| {
            let r = cr.body;
            let ink = Ink::of(s);
            let k = s.k;
            let plot = r.shrink2(vec2(8.0 * k, 18.0 * k));
            let x_of = |f: f32| {
                plot.left()
                    + plot.width() * cantus_dsp::math::log10(f.clamp(50.0, 5_000.0) / 50.0) / 2.0
            };
            let (top, bottom) = (0.0f32, -36.0f32);
            let y_of = |amp: f32| {
                let db = (20.0 * cantus_dsp::math::log10(amp.max(1e-6))).clamp(bottom, top);
                plot.top() + plot.height() * (top - db) / (top - bottom)
            };
            let mut parts = [(0.0f32, 0.0f32, 0u8); 64];
            let n = partials(ts, &mut parts);
            let both = ts.side == Side::Both;
            with_brush(&ui.painter_at(r), |b| {
                canvas::ground_b(b, r, &ink, s.hue_tok);
                for db in [-12.0f32, -24.0] {
                    let y = plot.top() + plot.height() * (top - db) / (top - bottom);
                    b.hline(plot.left(), plot.right(), y, b.px(), ink.grid);
                }
                // The tone's own harmonics, faint.
                for h in 1..=6 {
                    let f = 220.0 * h as f32;
                    let x = x_of(f);
                    canvas::dashed(
                        b,
                        pos2(x, plot.bottom()),
                        pos2(x, y_of(1.0 / h as f32)),
                        1.2 * k,
                        ink.faint,
                        3.0 * k,
                        3.0 * k,
                    );
                }
                for &(f, amp, ch) in &parts[..n] {
                    if ch == 1 && !both {
                        continue;
                    }
                    if !(50.0..=5_000.0).contains(&f) {
                        continue;
                    }
                    let x = x_of(f) + if both && ch == 1 { 2.0 * k } else { 0.0 };
                    let c = if ch == 0 { ink.hue } else { ink.ok };
                    b.segment(
                        pos2(x, plot.bottom()),
                        pos2(x, y_of(amp)),
                        2.0 * k,
                        c,
                        false,
                    );
                    b.dot(pos2(x, y_of(amp)), 2.5 * k, c);
                }
                for (word, at) in [("100", 100.0f32), ("1k", 1_000.0)] {
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
                if tel[T_SECTIONS] > 0.0 {
                    canvas::tag(
                        b,
                        pos2(r.right() - 6.0 * k, r.top() + 3.0 * k),
                        Align2::RIGHT_TOP,
                        frame::tr("translatio.rejection"),
                        ink.text,
                        k,
                        Some(ink.plate),
                        format_args!("{:.0} dB", tel[T_REJECTION]),
                    );
                }
                if both {
                    let t = Text::key("translatio.both_legend", Role::Micro, ink.dim).caps(true);
                    canvas::put(
                        b,
                        pos2(r.left() + 6.0 * k, r.top() + 3.0 * k),
                        Align2::LEFT_TOP,
                        &t,
                        k,
                        Some(ink.plate),
                    );
                }
                canvas::edge_b(b, r, &ink);
            });
        },
    );
}

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let ts = cantus_engine::fx::translatio::settings(spec);
    let shift = modfx::Unit(ts.shift_hz, "Hz");
    modfx::preview_strip(ui, spec, frame::tr("translatio.preview"), &shift, &|c| {
        modfx::lfo_at(&ts.lfo, 0.0, c).0 * if ts.lfo_hz > 0.0 { 0.8 } else { 0.0 }
    })
}

/// The settings `--editor translatio` and the screenshot tool open it with: a bell-like
/// shift with a little feedback, the two sidebands apart.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "translatio").set_many(
        &[
            ("shift", 173.0),
            ("side", 2.0),
            ("fb", 0.4),
            ("lfo", 40.0),
            ("rate", 0.3),
            ("mix", 0.8),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::translatio::Translatio::new(48_000.0);
    d.set(&cantus_engine::fx::translatio::settings(&spec));
    d.reset();
    modfx::demo_run(
        &mut |l, r, pos| {
            d.set_transport(&pos);
            d.process(l, r);
        },
        0.6,
    );
    let mut t = [0.0f32; 64];
    d.telemetry(&mut t);
    app.fx.injected = Some(t);
}
