//! Impetus's editor: the transient processor (ADR 0056).
//!
//! The references (SPL Transient Designer, FL's Transient Processor, Logic's Enveloper)
//! are two knobs and, at best, a gain trace. Here the editor shows both sides of the
//! question:
//!
//! - **What it does to a hit**: a synthetic hit run through the real DSP at these settings
//!   (cached until they change), its envelope before and after — so turning *attack* or
//!   *sustain* is seen before it is heard.
//! - **What it is doing now**: the last three seconds, column by column as the audio made
//!   them (never only the last block): the input's level and the gain, boosts up, cuts down.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::dynfx::{self, ColumnHistory};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx::{db_pill, hz_pill, ms_pill};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::impetus::{tel, Impetus, ImpetusSettings, MAX_GAIN_DB};
use egui::{pos2, vec2, Align2, Pos2, Rect};

const KIND: EffectKind = EffectKind::Transient;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    ("preset.impetus.punch", &[("attack", 0.6), ("speed", 12.0)]),
    (
        "preset.impetus.snap",
        &[("attack", 0.8), ("speed", 6.0), ("band", 1.0), ("xover", 1_500.0)],
    ),
    (
        "preset.impetus.soften",
        &[("attack", -0.6), ("speed", 20.0)],
    ),
    (
        "preset.impetus.tight",
        &[("attack", 0.4), ("sustain", -0.6), ("length", 180.0)],
    ),
    (
        "preset.impetus.room",
        &[("sustain", 0.7), ("length", 600.0)],
    ),
    (
        "preset.impetus.dry",
        &[("sustain", -0.8), ("length", 120.0)],
    ),
    (
        "preset.impetus.kick",
        &[
            ("attack", 0.7),
            ("speed", 18.0),
            ("band", 2.0),
            ("xover", 150.0),
        ],
    ),
    (
        "preset.impetus.pluck",
        &[("attack", 0.5), ("sustain", 0.3), ("speed", 8.0), ("length", 400.0)],
    ),
];

const BAND_KEYS: [&str; 3] = ["impetus.band.0", "impetus.band.1", "impetus.band.2"];

/// The hit drawn in the hit card: its length and how many points.
pub const HIT_MS: f32 = 400.0;
pub const HIT_POINTS: usize = 200;

/// A synthetic hit: 40 ms of silence, then an instant rise over a 180 Hz tone with a
/// bright click, falling over 120 ms.
pub fn hit_sample(i: usize, sr: f32) -> f32 {
    let start = (0.04 * sr) as usize;
    if i < start {
        return 0.0;
    }
    let t = (i - start) as f32 / sr;
    let env = cantus_dsp::math::exp(-t / 0.12);
    let click = cantus_dsp::math::exp(-t / 0.004);
    0.6 * env * cantus_dsp::math::sin(std::f32::consts::TAU * 180.0 * t)
        + 0.3 * click * cantus_dsp::math::sin(std::f32::consts::TAU * 3_100.0 * t)
}

/// The hit's envelope before and after Impetus at these settings, dB, at [`HIT_POINTS`]
/// points over [`HIT_MS`]: the real DSP run on [`hit_sample`].
pub fn hit_response(s: &ImpetusSettings, sr: f32) -> (Vec<f32>, Vec<f32>) {
    let n = (HIT_MS * 0.001 * sr) as usize;
    let x: Vec<f32> = (0..n).map(|i| hit_sample(i, sr)).collect();
    let mut d = Impetus::new(sr);
    d.set(s);
    d.reset();
    let (mut l, mut r) = (x.clone(), x.clone());
    d.process(&mut l, &mut r, None);
    let bin = n / HIT_POINTS;
    let env = |v: &[f32]| -> Vec<f32> {
        v.chunks(bin)
            .take(HIT_POINTS)
            .map(|c| cantus_dsp::dynfx_follow::to_db(c.iter().fold(0f32, |m, v| m.max(v.abs()))))
            .collect()
    };
    (env(&x), env(&l))
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The last three seconds, column by column.
    history: ColumnHistory,
    /// The hit's envelopes, and the settings they were made for.
    hit: Option<(ImpetusSettings, Vec<f32>, Vec<f32>)>,
    /// Where the history and the hit were drawn last frame (tests point at them).
    pub display: Option<Rect>,
    pub hit_rect: Option<Rect>,
    /// Where the band keys were drawn last frame.
    pub band_keys: Option<Rect>,
}

/// Impetus's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.impetus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "impetus"), &mut st, ui);
    app.fx.impetus = st;
    out
}

/// Impetus's editor on any host (ADR 0020).
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = if tel[tel::SAMPLE_RATE] > 0.0 {
        tel[tel::SAMPLE_RATE]
    } else {
        host.sample_rate()
    };
    let is = cantus_engine::fx::impetus::settings(host.spec());
    let live = tel[tel::BLOCKS] > 0.0;
    if live {
        st.history.update(&tel);
    }
    if st.hit.as_ref().is_none_or(|h| h.0 != is) {
        let (a, b) = hit_response(&is, 48_000.0);
        st.hit = Some((is, a, b));
    }
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
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
        body_ui(host, st, ui, s, body, &tel, &is, live)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

#[allow(clippy::too_many_arguments)]
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    is: &ImpetusSettings,
    live: bool,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(180.0 * k);
    let r1: Vec<Rect> = g.row(row1, &[0.62, 0.38]).collect();
    history_card(ui, s, r1[0], st, tel, is, live);
    hit_card(ui, s, r1[1], st);

    let seg_w = 176.0;
    let w = [
        controls::cells_width(&[Size::XL]) + 16.0,
        controls::cells_width(&[Size::XL]) + 16.0,
        controls::cells_width(&[Size::L, Size::L]) + 16.0,
        seg_w + controls::cells_width(&[Size::L]) + 24.0,
        controls::cells_width(&[Size::L]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    let sd = s.function(Function::Dynamics);
    for (i, key, title) in [
        (0usize, "attack", "impetus.card.attack"),
        (1, "sustain", "impetus.card.sustain"),
    ] {
        controls::card(ui, &sd, r2[i], Src::Key(title), None, |ui, _| {
            controls::param_with(ui, &sd, host, key, Size::XL, Some(&dynfx::signed_pct));
        });
    }
    controls::card(
        ui,
        &sd,
        r2[2],
        Src::Key("impetus.card.timing"),
        None,
        |ui, _| {
            controls::param_with(ui, &sd, host, "speed", Size::L, Some(&ms_pill));
            controls::param_with(ui, &sd, host, "length", Size::L, Some(&ms_pill));
        },
    );
    controls::card(
        ui,
        &sd,
        r2[3],
        Src::Key("impetus.card.band"),
        None,
        |ui, _| {
            controls::column(ui, &sd, seg_w, |ui| {
                st.band_keys = Some(Rect::from_min_size(
                    ui.cursor().min,
                    vec2(seg_w * k, sd.d(controls::ROW)),
                ));
                let labels = BAND_KEYS.map(Src::Key);
                controls::segmented_param(ui, &sd, host, "band", &labels, seg_w);
            });
            let split = is.band != cantus_dsp::impetus::Band::Full;
            ui.add_enabled_ui(split, |ui| {
                controls::param_with(ui, &sd, host, "xover", Size::L, Some(&hz_pill));
            });
        },
    );
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        r2[4],
        Src::Key("impetus.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "out", Size::L, Some(&db_pill));
        },
    );
}

use crate::kit::shell::Grid;

/// The last three seconds: the input's level (faint, from the top) and the gain — boosts
/// up from the middle line, cuts down — column by column.
#[allow(clippy::too_many_arguments)]
fn history_card(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    st: &mut State,
    tel: &[f32; 64],
    is: &ImpetusSettings,
    live: bool,
) {
    let s = s.function(Function::Dynamics);
    controls::card(
        ui,
        &s,
        rect,
        Src::Key("impetus.card.history"),
        None,
        |ui, cr| {
            let r = cr.body;
            st.display = Some(r);
            let ink = Ink::of(&s);
            let k = s.k;
            let plot = r.shrink2(vec2(8.0 * k, 10.0 * k));
            let mid = plot.center().y;
            let half = plot.height() * 0.5;
            with_brush(&ui.painter_at(r), |b| {
                canvas::ground_b(b, r, &ink, s.hue_tok);
                for db in [-12.0f32, 12.0] {
                    let y = mid - half * db / MAX_GAIN_DB;
                    b.hline(plot.left(), plot.right(), y, b.px(), ink.grid);
                }
                b.hline(plot.left(), plot.right(), mid, b.px(), ink.grid_strong);
                let cap = st.history.capacity().max(1);
                let w = plot.width() / cap as f32;
                let skip = cap - st.history.len().min(cap);
                for (i, c) in st.history.iter().enumerate() {
                    let x = plot.left() + (skip + i) as f32 * w;
                    // The input's peak, −60 … 0 dBFS, as a faint band around the middle.
                    let lvl = ((cantus_dsp::dynfx_follow::to_db(c[0]) + 60.0) / 60.0)
                        .clamp(0.0, 1.0);
                    if lvl > 0.0 {
                        b.rect(
                            Rect::from_min_max(
                                pos2(x, mid - half * lvl),
                                pos2(x + w.max(k), mid + half * lvl),
                            ),
                            crate::kit::Fill::Solid(Ink::a(ink.faint, 0.45)),
                        );
                    }
                    let up = c[1].max(0.0) / MAX_GAIN_DB;
                    if up > 0.0 {
                        b.rect(
                            Rect::from_min_max(pos2(x, mid - half * up), pos2(x + w.max(k), mid)),
                            crate::kit::Fill::Solid(ink.hue),
                        );
                    }
                    let down = (-c[2]).max(0.0) / MAX_GAIN_DB;
                    if down > 0.0 {
                        b.rect(
                            Rect::from_min_max(pos2(x, mid), pos2(x + w.max(k), mid + half * down)),
                            crate::kit::Fill::Solid(ink.danger),
                        );
                    }
                }
                let t = Text::key("impetus.axis.boost", Role::Micro, ink.dim).caps(true);
                canvas::put(
                    b,
                    pos2(plot.left() + 4.0 * k, plot.top()),
                    Align2::LEFT_TOP,
                    &t,
                    k,
                    Some(ink.plate),
                );
                let t = Text::key("impetus.axis.cut", Role::Micro, ink.dim).caps(true);
                canvas::put(
                    b,
                    pos2(plot.left() + 4.0 * k, plot.bottom()),
                    Align2::LEFT_BOTTOM,
                    &t,
                    k,
                    Some(ink.plate),
                );
                if live {
                    canvas::tag(
                        b,
                        pos2(plot.right() - 4.0 * k, plot.top()),
                        Align2::RIGHT_TOP,
                        frame::tr("impetus.legend.gain"),
                        ink.text,
                        k,
                        Some(ink.plate),
                        format_args!(
                            "+{:.1} / \u{2212}{:.1} dB",
                            tel[tel::GAIN_MAX].max(0.0),
                            tel[tel::GAIN_MIN].max(0.0)
                        ),
                    );
                }
                if is.band != cantus_dsp::impetus::Band::Full {
                    let above = is.band == cantus_dsp::impetus::Band::Highs;
                    canvas::tag(
                        b,
                        pos2(plot.left() + 4.0 * k, mid - 4.0 * k),
                        Align2::LEFT_BOTTOM,
                        frame::tr(BAND_KEYS[is.band as usize]),
                        ink.text,
                        k,
                        Some(ink.plate),
                        format_args!("{} {:.0} Hz", if above { ">" } else { "<" }, is.xover_hz),
                    );
                }
                if tel[tel::KEYED] >= 0.5 {
                    let t = Text::key("impetus.tag.key", Role::Micro, ink.hue).caps(true);
                    canvas::put(
                        b,
                        pos2(plot.right() - 4.0 * k, plot.bottom()),
                        Align2::RIGHT_BOTTOM,
                        &t,
                        k,
                        Some(ink.plate),
                    );
                }
                if is.attack == 0.0 && is.sustain == 0.0 {
                    let t = Text::key("impetus.idle", Role::Body, ink.dim).caps(false);
                    canvas::put(b, r.center(), Align2::CENTER_CENTER, &t, k, Some(ink.plate));
                }
                canvas::edge_b(b, r, &ink);
            });
        },
    );
}

/// A hit through Impetus: its envelope in and out, from the DSP.
fn hit_card(ui: &mut egui::Ui, s: &Style, rect: Rect, st: &mut State) {
    let s = s.function(Function::Dynamics);
    controls::card(
        ui,
        &s,
        rect,
        Src::Key("impetus.card.hit"),
        None,
        |ui, cr| {
            let r = cr.body;
            st.hit_rect = Some(r);
            let ink = Ink::of(&s);
            let k = s.k;
            let plot = Rect::from_min_max(
                pos2(r.left() + 8.0 * k, r.top() + 24.0 * k),
                pos2(r.right() - 8.0 * k, r.bottom() - 30.0 * k),
            );
            let Some((_, input, output)) = st.hit.as_ref() else {
                return;
            };
            let y_of = |db: f32| {
                plot.bottom() - plot.height() * ((db + 48.0) / 54.0).clamp(0.0, 1.0)
            };
            let line = |v: &[f32]| -> Vec<Pos2> {
                v.iter()
                    .enumerate()
                    .map(|(i, db)| {
                        pos2(
                            plot.left() + plot.width() * i as f32 / (v.len() - 1).max(1) as f32,
                            y_of(*db),
                        )
                    })
                    .collect()
            };
            let (a, o) = (line(input), line(output));
            with_brush(&ui.painter_at(r), |b| {
                canvas::ground_b(b, r, &ink, s.hue_tok);
                for db in [-36.0f32, -24.0, -12.0, 0.0] {
                    b.hline(plot.left(), plot.right(), y_of(db), b.px(), ink.grid);
                }
                b.area(&a, plot.bottom(), ink.fill(0.25));
                b.polyline(&a, 1.2 * k, ink.faint);
                b.polyline(&o, 2.0 * k, ink.hue);
                // The legend: in, out.
                let mut x = plot.left();
                for (key, c) in [("impetus.legend.in", ink.faint), ("impetus.legend.out", ink.hue)] {
                    b.hline(x, x + 14.0 * k, r.top() + 12.0 * k, 2.0 * k, c);
                    let t = Text::key(key, Role::Micro, ink.dim).caps(true);
                    let at = canvas::put(
                        b,
                        pos2(x + 18.0 * k, r.top() + 12.0 * k),
                        Align2::LEFT_CENTER,
                        &t,
                        k,
                        None,
                    );
                    x = at.right() + 12.0 * k;
                }
                for (j, key) in ["impetus.law.1", "impetus.law.2"].iter().enumerate() {
                    let line = Rect::from_min_size(
                        pos2(r.left() + 8.0 * k, plot.bottom() + (4.0 + 12.0 * j as f32) * k),
                        vec2(r.width() - 16.0 * k, 12.0 * k),
                    );
                    let t = Text::key(key, Role::Micro, ink.dim)
                        .caps(false)
                        .align(Align2::LEFT_CENTER)
                        .policy(crate::kit::text::Policy::Elide(crate::kit::text::Elide::End));
                    let _ = crate::kit::text::paint(b, line, &t, k);
                }
                canvas::edge_b(b, r, &ink);
            });
        },
    );
}

/// The inspector's picture: the hit in and out, and the amounts in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let is = cantus_engine::fx::impetus::settings(spec);
    let s = Style::current().function(frame::hue_of(spec.kind));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, s.d(44.0)), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let ink = Ink::of(&s);
        let k = s.k;
        let mut words = String::new();
        dynfx::signed_pct(is.attack, &mut words);
        words.push_str(" \u{b7} ");
        dynfx::signed_pct(is.sustain, &mut words);
        with_brush(&ui.painter_at(rect), |b| {
            canvas::ground_b(b, rect, &ink, s.hue_tok);
            // The shape a hit takes: a rise and a fall, bent by the two amounts.
            let plot = rect.shrink2(vec2(6.0 * k, 6.0 * k));
            const N: usize = 40;
            let mut pts = [Pos2::ZERO; N];
            for (j, p) in pts.iter_mut().enumerate() {
                let t = j as f32 / (N - 1) as f32;
                let env = if t < 0.1 { t / 0.1 } else { cantus_dsp::math::exp(-(t - 0.1) * 4.0) };
                let bend = if t < 0.25 {
                    1.0 + 0.5 * is.attack * (1.0 - t / 0.25)
                } else {
                    1.0 + 0.5 * is.sustain * ((t - 0.25) / 0.75)
                };
                *p = pos2(
                    plot.left() + plot.width() * t,
                    plot.bottom() - plot.height() * (env * bend).clamp(0.0, 1.2) / 1.2,
                );
            }
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
                format_args!("{words}"),
            );
            canvas::edge_b(b, rect, &ink);
        });
    }
    resp
}

/// The settings `--editor impetus` and the screenshot tool open it with.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "impetus").set_many(
        &[("attack", 0.6), ("sustain", -0.4), ("speed", 10.0), ("length", 220.0)],
        "cmd.fx_preset",
    );
}

/// A drum-like test signal: a hit every quarter second (kick, then a snare-like click).
pub fn demo_signal(n: usize) -> f32 {
    let beat = n % 12_000;
    let t = beat as f32 / 48_000.0;
    let kick = cantus_dsp::math::exp(-t / 0.09)
        * cantus_dsp::math::sin(std::f32::consts::TAU * 70.0 * t);
    let snare = if (n / 12_000) % 2 == 1 {
        cantus_dsp::math::exp(-t / 0.05) * cantus_dsp::math::sin(n as f32 * 1.7) * 0.5
    } else {
        0.0
    };
    0.6 * kick + snare
}

/// What a playing engine would feed the editor: three seconds of [`demo_signal`] through
/// the DSP, every column kept.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let mut d = Impetus::new(48_000.0);
    d.set(&cantus_engine::fx::impetus::settings(&spec));
    d.reset();
    let mut st = std::mem::take(&mut app.fx.impetus);
    st.effect = app.fx.open.map(|o| o.1);
    let mut t = [0.0f32; 64];
    let block = 512;
    let (mut l, mut r) = (vec![0f32; block], vec![0f32; block]);
    for blk in 0..300 {
        for i in 0..block {
            let v = demo_signal(blk * block + i);
            l[i] = v;
            r[i] = v;
        }
        d.process(&mut l, &mut r, None);
        d.telemetry(&mut t);
        st.history.update(&t);
    }
    app.fx.impetus = st;
    app.fx.injected = Some(t);
}
