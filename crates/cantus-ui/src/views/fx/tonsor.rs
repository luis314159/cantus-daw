//! Tonsor's editor: the clipper with a true-peak ceiling (ADR 0056).
//!
//! The references (FL's Fruity Soft Clipper, StandardCLIP, KClip) agree on the controls
//! and differ on what they tell: FL shows a curve and nothing of the signal; the others
//! show a reduction meter. Here the editor answers the three questions a clipper raises:
//!
//! - **What does it do to a peak?** The transfer curve, from the same function the DSP
//!   runs (`cantus_dsp::tonsor::shape`): input level across, output up, the ceiling and
//!   the knee marked, the straight line of no change under it, and a ball where the
//!   input's peak sits on the curve now.
//! - **How much, and how often?** The clip meter: the deepest reduction (held), the share
//!   of samples clipped over the last second of audio, a five-second history.
//! - **Is the ceiling really held?** The input's inter-sample overs counted, its true peak
//!   and the output's in dBTP, and the guard's work when the true-peak ceiling is on.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::dynfx::{self, History, Rates};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx::db_pill;
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::hint::{self, Caps};
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::tonsor::{latency_for, shape, tel, TonsorSettings};
use egui::{pos2, vec2, Align2, Pos2, Rect};

const KIND: EffectKind = EffectKind::Clipper;

/// Factory presets: a label key and the parameters it sets (what a preset does not name
/// goes back to its default).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.tonsor.safety",
        &[("ceil", -0.3), ("curve", 0.0), ("os", 2.0), ("tp", 1.0)],
    ),
    (
        "preset.tonsor.streaming",
        &[
            ("ceil", -1.0),
            ("knee", 1.0),
            ("curve", 1.0),
            ("os", 3.0),
            ("tp", 1.0),
        ],
    ),
    (
        "preset.tonsor.soft_master",
        &[
            ("drive", 2.0),
            ("ceil", -0.5),
            ("knee", 3.0),
            ("curve", 1.0),
            ("os", 3.0),
            ("tp", 1.0),
        ],
    ),
    (
        "preset.tonsor.loud_clean",
        &[
            ("drive", 5.0),
            ("ceil", -0.3),
            ("knee", 1.5),
            ("curve", 1.0),
            ("os", 4.0),
            ("tp", 1.0),
        ],
    ),
    (
        "preset.tonsor.drum_bus",
        &[
            ("drive", 6.0),
            ("knee", 2.0),
            ("curve", 0.0),
            ("os", 2.0),
            ("match", 1.0),
        ],
    ),
    (
        "preset.tonsor.snare_crack",
        &[
            ("drive", 9.0),
            ("ceil", -3.0),
            ("curve", 0.0),
            ("os", 1.0),
            ("match", 1.0),
        ],
    ),
    (
        "preset.tonsor.analog_warmth",
        &[
            ("drive", 4.0),
            ("knee", 6.0),
            ("curve", 2.0),
            ("os", 2.0),
            ("match", 1.0),
        ],
    ),
    (
        "preset.tonsor.hard_808",
        &[
            ("drive", 12.0),
            ("ceil", -2.0),
            ("knee", 0.5),
            ("curve", 0.0),
            ("os", 3.0),
            ("match", 1.0),
        ],
    ),
];

/// The curves' names and what each does, by index.
const CURVE_KEYS: [&str; 3] = ["tonsor.curve.0", "tonsor.curve.1", "tonsor.curve.2"];
const CURVE_TIPS: [&str; 3] = [
    "tonsor.curve_tip.0",
    "tonsor.curve_tip.1",
    "tonsor.curve_tip.2",
];
const OS_KEYS: [&str; 5] = [
    "tonsor.os.0",
    "tonsor.os.1",
    "tonsor.os.2",
    "tonsor.os.3",
    "tonsor.os.4",
];

/// The transfer display's range: input and output, dBFS.
pub const IN_RANGE: (f32, f32) = (-30.0, 12.0);
pub const OUT_RANGE: (f32, f32) = (-30.0, 3.0);

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    chips_os: frame::ChipText,
    /// Clipped samples and overs over the last second of audio.
    rates: Rates,
    /// The reduction, one value per block, for the five-second strip.
    history: History,
    /// The ball: the input's peak, falling slowly.
    ball: f32,
    /// Where the transfer curve's plot and the meter were drawn last frame (tests point
    /// at them).
    pub transfer: Option<Rect>,
    pub meter: Option<Rect>,
    /// Where the curves' keys were drawn last frame.
    pub curve_keys: Option<Rect>,
}

/// Tonsor's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.tonsor);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "tonsor"), &mut st, ui);
    app.fx.tonsor = st;
    out
}

/// Tonsor's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = if tel[tel::SAMPLE_RATE] > 0.0 {
        tel[tel::SAMPLE_RATE]
    } else {
        host.sample_rate()
    };
    let ts = cantus_engine::fx::tonsor::settings(host.spec());
    let live = tel[tel::BLOCKS] > 0.0 || tel[tel::SAMPLES] > 0.0;
    if live {
        st.rates
            .push(tel[tel::SAMPLES], [tel[tel::CLIPPED], tel[tel::OVERS], 0.0]);
        st.history
            .push(tel[tel::BLOCKS], tel[tel::REDUCTION].max(tel[tel::GUARD]));
        // The ball falls about 20 dB a second at 60 frames a second.
        let peak = tel[tel::IN_PEAK] - ts.drive_db;
        st.ball = if peak >= st.ball || st.ball == 0.0 {
            peak
        } else {
            st.ball - 0.33
        };
    }
    let mut chips = std::mem::take(&mut st.chips);
    let mut chips_os = std::mem::take(&mut st.chips_os);
    let chip = [
        chips.latency(latency_for(&ts) as f32, sr),
        chips_os.oversampling(ts.factor() as u32),
    ];
    let out_db = tel[tel::OUT_PEAK];
    let meter = live.then_some(frame::Levels {
        level: [out_db; 2],
        hold: [out_db; 2],
    });
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ts, sr, live)
    });
    st.chrome = chrome;
    st.chips = chips;
    st.chips_os = chips_os;
    out
}

/// The output level, dBFS, of a peak at `in_db` dBFS before the drive: what the DSP's curve
/// does to a sample of that size (the true-peak guard aside) — what the display draws.
pub fn transfer(s: &TonsorSettings, in_db: f32) -> f32 {
    let g = |db: f32| cantus_dsp::math::db_to_gain(db);
    let c = g(s.ceiling_db.min(0.0));
    let k = c * g(-s.knee_db.max(0.0));
    let x = g(in_db + s.drive_db);
    let back = if s.match_gain { -s.drive_db } else { 0.0 };
    let y = shape(x, k, c, s.curve) * g(s.out_db + back);
    cantus_dsp::dynfx_follow::to_db(y)
}

/// The height of the controls' row at 100 %.
fn controls_height() -> f32 {
    controls::card_height(Size::L)
}

#[allow(clippy::too_many_arguments)]
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ts: &TonsorSettings,
    sr: f32,
    live: bool,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls_height() * k;
    let row1 = (area.height() - row2 - gap).max(180.0 * k);
    let r1: Vec<Rect> = g.row(row1, &[0.58, 0.42]).collect();
    transfer_card(ui, s, r1[0], st, ts, live);
    meter_card(ui, s, r1[1], st, tel, ts, sr, live);

    let seg_w = 168.0;
    let curve_w = 204.0;
    let w = [
        curve_w + controls::cells_width(&[Size::L]) + 24.0,
        controls::cells_width(&[Size::L]) + 112.0 + 24.0,
        controls::cells_width(&[Size::L]) + 112.0 + 24.0,
        seg_w + 16.0,
        controls::cells_width(&[Size::L]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    let sd = s.function(Function::Dynamics);
    controls::card(
        ui,
        &sd,
        r2[0],
        Src::Key("tonsor.card.curve"),
        None,
        |ui, _| {
            controls::column(ui, &sd, curve_w, |ui| {
                st.curve_keys = Some(Rect::from_min_size(
                    ui.cursor().min,
                    vec2(curve_w * k, sd.d(controls::ROW)),
                ));
                let labels = CURVE_KEYS.map(Src::Key);
                controls::segmented_param(ui, &sd, host, "curve", &labels, curve_w);
            });
            controls::param_with(ui, &sd, host, "knee", Size::L, Some(&db_pill));
        },
    );
    controls::card(
        ui,
        &sd,
        r2[1],
        Src::Key("tonsor.card.drive"),
        None,
        |ui, _| {
            controls::param_with(ui, &sd, host, "drive", Size::L, Some(&db_pill));
            controls::column(ui, &sd, 112.0, |ui| {
                let r = controls::led_param(
                    ui,
                    &sd,
                    host,
                    "match",
                    Src::Key("fx.tonsor.match"),
                    112.0,
                );
                tip(ui, &r, "fx.tonsor.match", "tonsor.match_tip");
            });
        },
    );
    controls::card(
        ui,
        &sd,
        r2[2],
        Src::Key("tonsor.card.ceiling"),
        None,
        |ui, _| {
            controls::param_with(ui, &sd, host, "ceil", Size::L, Some(&db_pill));
            controls::column(ui, &sd, 112.0, |ui| {
                let r = controls::led_param(ui, &sd, host, "tp", Src::Key("fx.tonsor.tp"), 112.0);
                tip(ui, &r, "fx.tonsor.tp", "tonsor.tp_tip");
            });
        },
    );
    controls::card(
        ui,
        &sd,
        r2[3],
        Src::Key("tonsor.card.quality"),
        None,
        |ui, _| {
            controls::column(ui, &sd, seg_w, |ui| {
                let labels = OS_KEYS.map(Src::Key);
                controls::segmented_param(ui, &sd, host, "os", &labels, seg_w);
            });
        },
    );
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        r2[4],
        Src::Key("tonsor.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "out", Size::L, Some(&db_pill));
        },
    );
}

/// The hint of a switch: its name and what it does.
fn tip(ui: &egui::Ui, r: &egui::Response, name: &'static str, what: &'static str) {
    if r.hovered() {
        hint::set_text(ui.ctx(), frame::tr(name), frame::tr(what), Caps::NONE);
    }
}

/// The transfer curve in a card: input level across, output up (dBFS), the ceiling and the
/// knee, the line of no change, and the ball where the input's peak is now.
fn transfer_card(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    st: &mut State,
    ts: &TonsorSettings,
    live: bool,
) {
    let s = s.function(Function::Dynamics);
    controls::card(
        ui,
        &s,
        rect,
        Src::Key("tonsor.card.transfer"),
        None,
        |ui, cr| {
            let r = cr.body;
            let ink = Ink::of(&s);
            let k = s.k;
            let plot = Rect::from_min_max(
                pos2(r.left() + 34.0 * k, r.top() + 10.0 * k),
                pos2(r.right() - 12.0 * k, r.bottom() - 22.0 * k),
            );
            st.transfer = Some(plot);
            let x_of = |db: f32| {
                plot.left()
                    + plot.width() * ((db - IN_RANGE.0) / (IN_RANGE.1 - IN_RANGE.0)).clamp(0.0, 1.0)
            };
            let y_of = |db: f32| {
                plot.bottom()
                    - plot.height()
                        * ((db - OUT_RANGE.0) / (OUT_RANGE.1 - OUT_RANGE.0)).clamp(0.0, 1.0)
            };
            const N: usize = 161;
            let mut pts = [Pos2::ZERO; N];
            for (j, p) in pts.iter_mut().enumerate() {
                let x = IN_RANGE.0 + (IN_RANGE.1 - IN_RANGE.0) * j as f32 / (N - 1) as f32;
                *p = pos2(x_of(x), y_of(transfer(ts, x)));
            }
            with_brush(&ui.painter_at(r), |b| {
                canvas::ground_b(b, r, &ink, s.hue_tok);
                // The grid every 6 dB, numbered every 12.
                let mut db = -24.0f32;
                while db <= 12.0 {
                    let strong = db == 0.0;
                    let c = if strong { ink.grid_strong } else { ink.grid };
                    if db <= OUT_RANGE.1 {
                        b.hline(plot.left(), plot.right(), y_of(db), b.px(), c);
                    }
                    b.vline(x_of(db), plot.top(), plot.bottom(), b.px(), c);
                    if (db as i32) % 12 == 0 {
                        canvas::putf(
                            b,
                            pos2(x_of(db), plot.bottom() + 3.0 * k),
                            Align2::CENTER_TOP,
                            Role::Micro,
                            ink.dim,
                            k,
                            None,
                            format_args!("{db:.0}"),
                        );
                        if db <= OUT_RANGE.1 {
                            canvas::putf(
                                b,
                                pos2(plot.left() - 4.0 * k, y_of(db)),
                                Align2::RIGHT_CENTER,
                                Role::Micro,
                                ink.dim,
                                k,
                                None,
                                format_args!("{db:.0}"),
                            );
                        }
                    }
                    db += 6.0;
                }
                // No change, faint; the ceiling, dashed; the knee's start, a tick.
                b.segment(
                    pos2(x_of(OUT_RANGE.0), y_of(OUT_RANGE.0)),
                    pos2(x_of(OUT_RANGE.1), y_of(OUT_RANGE.1)),
                    1.0 * k,
                    ink.faint,
                    false,
                );
                let post = ts.out_db - if ts.match_gain { ts.drive_db } else { 0.0 };
                let ceil_out = ts.ceiling_db.min(0.0) + post;
                canvas::dashed(
                    b,
                    pos2(plot.left(), y_of(ceil_out)),
                    pos2(plot.right(), y_of(ceil_out)),
                    1.0 * k,
                    ink.danger,
                    4.0 * k,
                    3.0 * k,
                );
                b.area(&pts, plot.bottom(), ink.fill(0.5));
                b.polyline(&pts, 2.0 * k, ink.hue);
                if ts.knee_db > 0.0 {
                    let knee_in = ts.ceiling_db.min(0.0) - ts.knee_db - ts.drive_db;
                    let at = pos2(x_of(knee_in), y_of(transfer(ts, knee_in)));
                    b.dot(at, 3.0 * k, ink.hue);
                    canvas::tag(
                        b,
                        at + vec2(6.0 * k, 6.0 * k),
                        Align2::LEFT_TOP,
                        frame::tr("tonsor.tag.knee"),
                        ink.text,
                        k,
                        Some(ink.plate),
                        format_args!("{:.1} dB", ts.ceiling_db.min(0.0) - ts.knee_db),
                    );
                }
                canvas::tag(
                    b,
                    pos2(plot.left() + 6.0 * k, y_of(ceil_out) + 3.0 * k),
                    Align2::LEFT_TOP,
                    frame::tr("tonsor.tag.ceiling"),
                    ink.text,
                    k,
                    Some(ink.plate),
                    format_args!(
                        "{:.1} {}",
                        ts.ceiling_db.min(0.0),
                        if ts.true_peak { "dBTP" } else { "dBFS" }
                    ),
                );
                // The input's peak now, on the curve.
                if live && st.ball > IN_RANGE.0 {
                    let x = st.ball.min(IN_RANGE.1);
                    let at = pos2(x_of(x), y_of(transfer(ts, x)));
                    b.vline(at.x, at.y, plot.bottom(), 1.0 * k, ink.faint);
                    b.dot(at, 4.0 * k, ink.text);
                }
                // The axes' names, and what the curve is.
                let t = Text::key("tonsor.axis.in", Role::Micro, ink.dim).caps(true);
                canvas::put(
                    b,
                    pos2(plot.right(), plot.bottom() - 4.0 * k),
                    Align2::RIGHT_BOTTOM,
                    &t,
                    k,
                    Some(ink.plate),
                );
                let t = Text::key("tonsor.axis.out", Role::Micro, ink.dim).caps(true);
                canvas::put(
                    b,
                    pos2(plot.left() + 6.0 * k, plot.top() + 4.0 * k),
                    Align2::LEFT_TOP,
                    &t,
                    k,
                    Some(ink.plate),
                );
                let i = (ts.curve as usize).min(2);
                let t = Text::key(CURVE_TIPS[i], Role::Micro, ink.dim).caps(false);
                canvas::put(
                    b,
                    pos2(plot.right() - 6.0 * k, plot.top() + 4.0 * k),
                    Align2::RIGHT_TOP,
                    &t,
                    k,
                    Some(ink.plate),
                );
                canvas::edge_b(b, r, &ink);
            });
        },
    );
}

/// The clip meter: how much, how often, the overs between the samples, and five seconds
/// of reduction.
#[allow(clippy::too_many_arguments)]
fn meter_card(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    st: &mut State,
    tel: &[f32; 64],
    ts: &TonsorSettings,
    sr: f32,
    live: bool,
) {
    let s = s.function(Function::Dynamics);
    controls::card(
        ui,
        &s,
        rect,
        Src::Key("tonsor.card.meter"),
        None,
        |ui, cr| {
            let r = cr.body.shrink2(vec2(8.0 * s.k, 4.0 * s.k));
            st.meter = Some(r);
            let ink = Ink::of(&s);
            let k = s.k;
            let [(share, _), (_, overs_s), _] = st.rates.over(f64::from(sr), sr);
            let red = tel[tel::REDUCTION].max(0.0);
            let guard = tel[tel::GUARD].max(0.0);
            let in_tp = tel[tel::IN_TP];
            let out_tp = tel[tel::OUT_TP];
            let ceiling = ts.ceiling_db.min(0.0);
            let row = 20.0 * k;
            let mut y = r.top();
            with_brush(&ui.painter_at(cr.body), |b| {
                let line = |b: &mut crate::kit::Brush,
                                y: &mut f32,
                                word: &str,
                                color: egui::Color32,
                                args: std::fmt::Arguments<'_>| {
                    let lr = Rect::from_min_size(pos2(r.left(), *y), vec2(r.width(), row));
                    dynfx::stat(b, lr, word, color, &ink, k, args);
                    *y += row;
                };
                let hot = |v: f32| if v > 0.0 { ink.danger } else { ink.text };
                let dash = "\u{2014}";
                if live {
                    line(
                        b,
                        &mut y,
                        frame::tr("tonsor.m.reduction"),
                        hot(red - 0.05),
                        format_args!("{red:.1} dB"),
                    );
                } else {
                    line(b, &mut y, frame::tr("tonsor.m.reduction"), ink.dim, format_args!("{dash}"));
                }
                let bar_r = Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), 5.0 * k));
                dynfx::bar(b, bar_r, red / 12.0, ink.danger, &ink, k);
                y += 9.0 * k;
                line(
                    b,
                    &mut y,
                    frame::tr("tonsor.m.clipped"),
                    hot(share),
                    format_args!(
                        "{:.1} % {}",
                        share * 100.0,
                        frame::tr("tonsor.m.of_samples")
                    ),
                );
                line(
                    b,
                    &mut y,
                    frame::tr("tonsor.m.overs"),
                    hot(overs_s),
                    format_args!("{overs_s:.0} {}", frame::tr("tonsor.m.per_second")),
                );
                let unit = frame::tr("tonsor.m.dbtp");
                if live && in_tp > -150.0 {
                    line(
                        b,
                        &mut y,
                        frame::tr("tonsor.m.in_tp"),
                        hot(in_tp - ceiling),
                        format_args!("{in_tp:+.1} {unit}"),
                    );
                } else {
                    line(b, &mut y, frame::tr("tonsor.m.in_tp"), ink.dim, format_args!("{dash}"));
                }
                if live && out_tp > -150.0 {
                    let over = out_tp > ceiling + 0.05;
                    line(
                        b,
                        &mut y,
                        frame::tr("tonsor.m.out_tp"),
                        if over { ink.danger } else { ink.text },
                        format_args!("{out_tp:+.1} {unit}"),
                    );
                } else {
                    line(b, &mut y, frame::tr("tonsor.m.out_tp"), ink.dim, format_args!("{dash}"));
                }
                if ts.true_peak {
                    line(
                        b,
                        &mut y,
                        frame::tr("tonsor.m.guard"),
                        ink.text,
                        format_args!("{guard:.2} dB"),
                    );
                }
                // Five seconds of reduction, newest at the right.
                y += 6.0 * k;
                let t = Text::key("tonsor.m.history", Role::Micro, ink.dim).caps(true);
                canvas::put(b, pos2(r.left(), y), Align2::LEFT_TOP, &t, k, None);
                y += 16.0 * k;
                let hist = Rect::from_min_max(pos2(r.left(), y), r.max);
                if hist.height() > 8.0 * k {
                    canvas::ground_b(b, hist, &ink, None);
                    st.history.paint_down(b, hist, 12.0, ink.danger, k);
                    canvas::edge_b(b, hist, &ink);
                    let quiet = st.history.values().all(|v| v <= 0.0);
                    if live && quiet && !st.history.is_empty() {
                        let t = Text::key("tonsor.idle", Role::Micro, ink.dim).caps(false);
                        canvas::put(
                            b,
                            hist.center(),
                            Align2::CENTER_CENTER,
                            &t,
                            k,
                            Some(ink.plate),
                        );
                    }
                }
            });
        },
    );
}

/// The inspector's picture: the transfer curve in a strip, and the ceiling in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let ts = cantus_engine::fx::tonsor::settings(spec);
    let s = Style::current().function(frame::hue_of(spec.kind));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, s.d(44.0)), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let ink = Ink::of(&s);
        let k = s.k;
        let plot = rect.shrink2(vec2(6.0 * k, 6.0 * k));
        const N: usize = 49;
        let mut pts = [Pos2::ZERO; N];
        for (j, p) in pts.iter_mut().enumerate() {
            let x = -18.0 + 24.0 * j as f32 / (N - 1) as f32;
            let y = transfer(&ts, x).clamp(-18.0, 3.0);
            *p = pos2(
                plot.left() + plot.width() * j as f32 / (N - 1) as f32,
                plot.bottom() - plot.height() * (y + 18.0) / 21.0,
            );
        }
        with_brush(&ui.painter_at(rect), |b| {
            canvas::ground_b(b, rect, &ink, s.hue_tok);
            b.area(&pts, plot.bottom(), ink.fill(0.5));
            b.polyline(&pts, 1.6 * k, ink.hue);
            canvas::putf(
                b,
                pos2(rect.right() - 6.0 * k, rect.top() + 4.0 * k),
                Align2::RIGHT_TOP,
                Role::Value,
                ink.text,
                k,
                Some(ink.plate),
                format_args!(
                    "{} \u{b7} {:.1} {}",
                    frame::tr(CURVE_KEYS[(ts.curve as usize).min(2)]),
                    ts.ceiling_db.min(0.0),
                    if ts.true_peak { "dBTP" } else { "dBFS" }
                ),
            );
            canvas::edge_b(b, rect, &ink);
        });
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("effect.tonsor"),
            frame::tr("modfx.preview_tip"),
            Caps::NONE,
        );
    }
    resp
}

/// The settings `--editor tonsor` and the screenshot tool open it with: a soft master
/// ceiling, driven.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "tonsor").set_many(
        &[
            ("drive", 4.0),
            ("ceil", -0.5),
            ("knee", 3.0),
            ("curve", 1.0),
            ("os", 3.0),
            ("tp", 1.0),
        ],
        "cmd.fx_preset",
    );
}

/// A hot, drum-like test signal: a decaying hit every half second over two tones.
pub fn demo_signal(n: usize) -> f32 {
    let beat = n % 24_000;
    let env = cantus_dsp::math::exp(-(beat as f32) / 2_400.0);
    let tone = cantus_dsp::math::sin(n as f32 * 0.013) * 0.4
        + cantus_dsp::math::sin(n as f32 * 0.071 + 0.4) * 0.25;
    (0.6 + 0.9 * env) * tone + 0.35 * env * cantus_dsp::math::sin(n as f32 * 0.9)
}

/// What a playing engine would feed the editor, for screenshots with no engine: the DSP
/// run for six seconds over [`demo_signal`].
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let ts = cantus_engine::fx::tonsor::settings(&spec);
    let mut d = cantus_dsp::tonsor::Tonsor::new(48_000.0);
    d.set(&ts);
    d.reset();
    let mut st = std::mem::take(&mut app.fx.tonsor);
    // The editor keeps what it is fed only for its own effect.
    st.effect = app.fx.open.map(|o| o.1);
    let mut t = [0.0f32; 64];
    // Six seconds, ending just after a hit.
    let block = 600;
    let (mut l, mut r) = (vec![0f32; block], vec![0f32; block]);
    for blk in 0..482 {
        for i in 0..block {
            let v = demo_signal(blk * block + i);
            l[i] = v;
            r[i] = v * 0.93;
        }
        d.process(&mut l, &mut r);
        d.telemetry(&mut t);
        st.rates
            .push(t[tel::SAMPLES], [t[tel::CLIPPED], t[tel::OVERS], 0.0]);
        st.history
            .push(t[tel::BLOCKS], t[tel::REDUCTION].max(t[tel::GUARD]));
    }
    st.ball = t[tel::IN_PEAK] - ts.drive_db;
    app.fx.tonsor = st;
    app.fx.injected = Some(t);
}
