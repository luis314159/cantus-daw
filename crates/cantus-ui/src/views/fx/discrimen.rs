//! Discrimen's editor: the transient splitter (ADR 0056).
//!
//! The references (Eventide SplitEQ's structural split, the transient/tonal separators)
//! hide the split itself. Here it is the picture: the last three seconds, column by column
//! as the audio made them, the **transient** part over the **body** and the share of each
//! moment that is transient under them; beside it the parts' levels now and what each
//! compressor takes. The detection mode says what it costs: zero latency, or a 2 ms
//! look-ahead that catches every onset from its first sample.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::dynfx::{self, ColumnHistory};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx::{db_pill, ms_pill, pct_pill};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::discrimen::{latency_for, tel, Discrimen, DiscrimenSettings, Listen};
use egui::{pos2, vec2, Align2, Rect};

const KIND: EffectKind = EffectKind::TransientSplit;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.discrimen.more_attack",
        &[("t.gain", 5.0), ("mode", 1.0)],
    ),
    ("preset.discrimen.less_attack", &[("t.gain", -6.0)]),
    (
        "preset.discrimen.drum_room",
        &[("b.gain", -4.0), ("b.comp", 0.4), ("b.tilt", -1.5), ("mode", 1.0)],
    ),
    (
        "preset.discrimen.snappy",
        &[("t.gain", 4.0), ("t.tilt", 2.0), ("b.gain", -5.0), ("mode", 1.0)],
    ),
    (
        "preset.discrimen.bright_attack",
        &[("t.tilt", 4.0), ("b.tilt", -3.0)],
    ),
    (
        "preset.discrimen.squash_body",
        &[("b.comp", 0.8), ("b.gain", 3.0)],
    ),
    (
        "preset.discrimen.only_transients",
        &[("listen", 1.0), ("mode", 1.0)],
    ),
    ("preset.discrimen.only_body", &[("listen", 2.0), ("mode", 1.0)]),
];

const MODE_KEYS: [&str; 2] = ["discrimen.mode.0", "discrimen.mode.1"];
const MODE_WORDS: [&str; 2] = ["discrimen.words.0", "discrimen.words.1"];
const LISTEN_KEYS: [&str; 3] = ["discrimen.listen.0", "discrimen.listen.1", "discrimen.listen.2"];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    history: ColumnHistory,
    /// Where the history was drawn, and the mode and listen keys (tests point at them).
    pub display: Option<Rect>,
    pub mode_keys: Option<Rect>,
    pub listen_keys: Option<Rect>,
}

/// Discrimen's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.discrimen);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "discrimen"),
        &mut st,
        ui,
    );
    app.fx.discrimen = st;
    out
}

/// Discrimen's editor on any host (ADR 0020).
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = if tel[tel::SAMPLE_RATE] > 0.0 {
        tel[tel::SAMPLE_RATE]
    } else {
        host.sample_rate()
    };
    let ds = cantus_engine::fx::discrimen::settings(host.spec());
    let live = tel[tel::BLOCKS] > 0.0;
    if live {
        st.history.update(&tel);
    }
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency_for(&ds, sr) as f32, sr)];
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
        body_ui(host, st, ui, s, body, &tel, &ds, sr, live)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// Whether nothing is processed: the two parts add up to the input.
pub fn idle(ds: &DiscrimenSettings) -> bool {
    let quiet = |p: &cantus_dsp::discrimen::Part| p.gain_db == 0.0 && p.comp == 0.0 && p.tilt_db == 0.0;
    quiet(&ds.transient) && quiet(&ds.body) && ds.listen == Listen::Both
}

#[allow(clippy::too_many_arguments)]
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ds: &DiscrimenSettings,
    sr: f32,
    live: bool,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::L) * k;
    let row1 = (area.height() - row2 - gap).max(180.0 * k);
    let r1: Vec<Rect> = g.row(row1, &[0.64, 0.36]).collect();
    history_card(ui, s, r1[0], st, ds);
    // The right column: the parts now, and under them what is heard.
    let seg_w = 200.0;
    let listen_h = 64.0 * k;
    let now = Rect::from_min_max(r1[1].min, pos2(r1[1].right(), r1[1].bottom() - listen_h - gap));
    let listen = Rect::from_min_max(pos2(r1[1].left(), now.bottom() + gap), r1[1].max);
    now_card(ui, s, now, tel, ds, sr, live);
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        listen,
        Src::Key("discrimen.card.listen"),
        None,
        |ui, _| {
            let w = (listen.width() / k - 24.0).min(seg_w + 60.0);
            controls::column(ui, &so, w, |ui| {
                st.listen_keys = Some(Rect::from_min_size(
                    ui.cursor().min,
                    vec2(w * k, so.d(controls::ROW)),
                ));
                let labels = LISTEN_KEYS.map(Src::Key);
                controls::segmented_param(ui, &so, host, "listen", &labels, w);
            });
        },
    );

    let w = [
        170.0 + controls::cells_width(&[Size::L, Size::L]) + 24.0,
        controls::cells_width(&[Size::M, Size::M, Size::M]) + 16.0,
        controls::cells_width(&[Size::M, Size::M, Size::M]) + 16.0,
        controls::cells_width(&[Size::M]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    let sd = s.function(Function::Dynamics);
    controls::card(
        ui,
        &sd,
        r2[0],
        Src::Key("discrimen.card.detect"),
        None,
        |ui, _| {
            controls::column(ui, &sd, 170.0, |ui| {
                st.mode_keys = Some(Rect::from_min_size(
                    ui.cursor().min,
                    vec2(170.0 * k, sd.d(controls::ROW)),
                ));
                let labels = MODE_KEYS.map(Src::Key);
                controls::segmented_param(ui, &sd, host, "mode", &labels, 170.0);
            });
            let sens = Some(&pct_pill as controls::Pill<'_>);
            dynfx::param_labelled(ui, &sd, host, "sens", Size::L, sens, "discrimen.k.sens");
            let speed = Some(&ms_pill as controls::Pill<'_>);
            dynfx::param_labelled(ui, &sd, host, "speed", Size::L, speed, "discrimen.k.speed");
        },
    );
    for (i, title, keys) in [
        (1usize, "discrimen.card.transient", ["t.gain", "t.comp", "t.tilt"]),
        (2, "discrimen.card.body", ["b.gain", "b.comp", "b.tilt"]),
    ] {
        controls::card(ui, &sd, r2[i], Src::Key(title), None, |ui, _| {
            let db = Some(&db_pill as controls::Pill<'_>);
            let pct = Some(&pct_pill as controls::Pill<'_>);
            dynfx::param_labelled(ui, &sd, host, keys[0], Size::M, db, "discrimen.k.gain");
            dynfx::param_labelled(ui, &sd, host, keys[1], Size::M, pct, "discrimen.k.comp");
            dynfx::param_labelled(ui, &sd, host, keys[2], Size::M, db, "discrimen.k.tilt");
        });
    }
    controls::card(
        ui,
        &so,
        r2[3],
        Src::Key("discrimen.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "out", Size::M, Some(&db_pill));
        },
    );
}

/// The last three seconds: the body's level (from the bottom, faint), the transient's over
/// it (the hue), and under them a strip of how much of each moment was transient.
fn history_card(ui: &mut egui::Ui, s: &Style, rect: Rect, st: &mut State, ds: &DiscrimenSettings) {
    let s = s.function(Function::Dynamics);
    controls::card(
        ui,
        &s,
        rect,
        Src::Key("discrimen.card.history"),
        None,
        |ui, cr| {
            let r = cr.body;
            st.display = Some(r);
            let ink = Ink::of(&s);
            let k = s.k;
            let legend_h = 18.0 * k;
            let mask_h = 16.0 * k;
            let plot = Rect::from_min_max(
                pos2(r.left() + 8.0 * k, r.top() + legend_h),
                pos2(r.right() - 8.0 * k, r.bottom() - mask_h - 10.0 * k),
            );
            let strip = Rect::from_min_max(
                pos2(plot.left(), plot.bottom() + 4.0 * k),
                pos2(plot.right(), r.bottom() - 6.0 * k),
            );
            let level = |v: f32| ((cantus_dsp::dynfx_follow::to_db(v) + 60.0) / 60.0).clamp(0.0, 1.0);
            with_brush(&ui.painter_at(r), |b| {
                canvas::ground_b(b, r, &ink, s.hue_tok);
                for q in [0.25f32, 0.5, 0.75] {
                    let y = plot.bottom() - plot.height() * q;
                    b.hline(plot.left(), plot.right(), y, b.px(), ink.grid);
                }
                let cap = st.history.capacity().max(1);
                let w = plot.width() / cap as f32;
                let skip = cap - st.history.len().min(cap);
                let body_c = Ink::a(ink.text, if ink.dark { 0.28 } else { 0.22 });
                for (i, c) in st.history.iter().enumerate() {
                    let x = plot.left() + (skip + i) as f32 * w;
                    let wx = w.max(k);
                    let hb = level(c[1]) * plot.height();
                    if hb > 0.0 {
                        b.rect(
                            Rect::from_min_max(pos2(x, plot.bottom() - hb), pos2(x + wx, plot.bottom())),
                            crate::kit::Fill::Solid(body_c),
                        );
                    }
                    let ht = level(c[0]) * plot.height();
                    if ht > 0.0 {
                        b.rect(
                            Rect::from_min_max(pos2(x, plot.bottom() - ht), pos2(x + wx, plot.bottom())),
                            crate::kit::Fill::Solid(Ink::a(ink.hue, 0.85)),
                        );
                    }
                    let m = c[2].clamp(0.0, 1.0);
                    if m > 0.0 {
                        b.rect(
                            Rect::from_min_max(
                                pos2(x, strip.bottom() - strip.height() * m),
                                pos2(x + wx, strip.bottom()),
                            ),
                            crate::kit::Fill::Solid(ink.hue),
                        );
                    }
                }
                b.hline(strip.left(), strip.right(), strip.bottom(), b.px(), ink.grid_strong);
                // The legend.
                let mut x = r.left() + 8.0 * k;
                let y = r.top() + legend_h * 0.5;
                for (key, c) in [
                    ("discrimen.legend.transient", ink.hue),
                    ("discrimen.legend.body", body_c),
                ] {
                    b.rect(
                        Rect::from_center_size(pos2(x + 5.0 * k, y), vec2(10.0 * k, 8.0 * k)),
                        crate::kit::Fill::Solid(c),
                    );
                    let t = Text::key(key, Role::Micro, ink.dim).caps(true);
                    let at = canvas::put(b, pos2(x + 14.0 * k, y), Align2::LEFT_CENTER, &t, k, None);
                    x = at.right() + 14.0 * k;
                }
                let t = Text::key("discrimen.legend.mask", Role::Micro, ink.dim).caps(true);
                canvas::put(
                    b,
                    pos2(strip.right() - 2.0 * k, strip.top()),
                    Align2::RIGHT_TOP,
                    &t,
                    k,
                    Some(ink.plate),
                );
                if idle(ds) {
                    let t = Text::key("discrimen.idle", Role::Body, ink.dim).caps(false);
                    canvas::put(b, plot.center(), Align2::CENTER_CENTER, &t, k, Some(ink.plate));
                }
                canvas::edge_b(b, r, &ink);
            });
        },
    );
}

/// The parts now: their levels, what each compressor takes, the transient share, and what
/// the detection mode costs.
#[allow(clippy::too_many_arguments)]
fn now_card(
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    ds: &DiscrimenSettings,
    sr: f32,
    live: bool,
) {
    let s = s.function(Function::Dynamics);
    controls::card(
        ui,
        &s,
        rect,
        Src::Key("discrimen.card.now"),
        None,
        |ui, cr| {
            let r = cr.body.shrink2(vec2(8.0 * s.k, 4.0 * s.k));
            let ink = Ink::of(&s);
            let k = s.k;
            let row = 20.0 * k;
            let dash = "\u{2014}";
            with_brush(&ui.painter_at(cr.body), |b| {
                let mut y = r.top();
                let mut line = |b: &mut crate::kit::Brush, word: &str, args: std::fmt::Arguments<'_>| {
                    let lr = Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), row));
                    dynfx::stat(b, lr, word, ink.text, &ink, k, args);
                    y += row;
                };
                for (key, v) in [
                    ("discrimen.m.transient", tel[tel::T_PEAK]),
                    ("discrimen.m.body", tel[tel::B_PEAK]),
                ] {
                    if live && v > -150.0 {
                        line(b, frame::tr(key), format_args!("{v:+.1} dBFS"));
                    } else {
                        line(b, frame::tr(key), format_args!("{dash}"));
                    }
                }
                line(
                    b,
                    frame::tr("discrimen.m.mask"),
                    format_args!("{:.0} %", 100.0 * tel[tel::MASK].clamp(0.0, 1.0)),
                );
                line(
                    b,
                    frame::tr("discrimen.m.comp_t"),
                    format_args!("{:.1} dB", tel[tel::T_GR].max(0.0)),
                );
                line(
                    b,
                    frame::tr("discrimen.m.comp_b"),
                    format_args!("{:.1} dB", tel[tel::B_GR].max(0.0)),
                );
                let lat = latency_for(ds, sr);
                line(
                    b,
                    frame::tr("discrimen.m.latency"),
                    format_args!("{} smp \u{b7} {:.1} ms", lat, lat as f32 / sr * 1000.0),
                );
                // What the mode means, in two short lines' room.
                let words = MODE_WORDS[usize::from(ds.look_ahead)];
                let t = Text::key(words, Role::Micro, ink.dim)
                    .caps(false)
                    .align(Align2::LEFT_TOP)
                    .policy(crate::kit::text::Policy::Elide(crate::kit::text::Elide::End));
                let text_r = Rect::from_min_max(pos2(r.left(), y + 6.0 * k), pos2(r.right(), y + 20.0 * k));
                let _ = crate::kit::text::paint(b, text_r, &t, k);
            });
        },
    );
}

/// The inspector's picture: the two parts as two bars, and the mode in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let ds = cantus_engine::fx::discrimen::settings(spec);
    let s = Style::current().function(frame::hue_of(spec.kind));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, s.d(44.0)), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let ink = Ink::of(&s);
        let k = s.k;
        with_brush(&ui.painter_at(rect), |b| {
            canvas::ground_b(b, rect, &ink, s.hue_tok);
            let base = rect.center().y + 10.0 * k;
            for (i, db) in [ds.transient.gain_db, ds.body.gain_db].iter().enumerate() {
                let x = rect.left() + 10.0 * k + i as f32 * 18.0 * k;
                let h = (12.0 + db.clamp(-12.0, 12.0)) / 24.0 * 26.0 * k;
                let c = if i == 0 { ink.hue } else { ink.faint };
                b.rect(
                    Rect::from_min_max(pos2(x, base - h), pos2(x + 12.0 * k, base)),
                    crate::kit::Fill::Solid(c),
                );
            }
            canvas::putf(
                b,
                pos2(rect.right() - 6.0 * k, rect.top() + 4.0 * k),
                Align2::RIGHT_TOP,
                Role::Value,
                ink.text,
                k,
                Some(ink.plate),
                format_args!(
                    "{:+.1} / {:+.1} dB \u{b7} {}",
                    ds.transient.gain_db,
                    ds.body.gain_db,
                    frame::tr(MODE_KEYS[usize::from(ds.look_ahead)])
                ),
            );
            canvas::edge_b(b, rect, &ink);
        });
    }
    resp
}

/// The settings `--editor discrimen` and the screenshot tool open it with.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "discrimen").set_many(
        &[
            ("mode", 1.0),
            ("t.gain", 3.0),
            ("t.tilt", 2.0),
            ("b.gain", -2.0),
            ("b.comp", 0.4),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor: three seconds of Impetus's drum-like
/// signal through the DSP, every column kept.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let mut d = Discrimen::new(48_000.0);
    d.set(&cantus_engine::fx::discrimen::settings(&spec));
    d.reset();
    let mut st = std::mem::take(&mut app.fx.discrimen);
    st.effect = app.fx.open.map(|o| o.1);
    let mut t = [0.0f32; 64];
    let block = 512;
    let (mut l, mut r) = (vec![0f32; block], vec![0f32; block]);
    for blk in 0..300 {
        for i in 0..block {
            let n = blk * block + i;
            let v = super::impetus::demo_signal(n) + 0.08 * cantus_dsp::math::sin(n as f32 * 0.031);
            l[i] = v;
            r[i] = v;
        }
        d.process(&mut l, &mut r);
        d.telemetry(&mut t);
        st.history.update(&t);
    }
    app.fx.discrimen = st;
    app.fx.injected = Some(t);
}
