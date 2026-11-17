//! Libra's editor: the utility (fx side round, ADR 0133 provisional).
//!
//! *Libra* is Latin for the balance (the scales). Its display is the measure of a channel:
//! the **tuner** (a needle on a fine scale, the note large, the reference A dragged), the
//! **goniometer** (dragged across for the balance, up and down for the width) over the
//! **correlation**, and each channel's level with its polarity and delay. Under it the
//! corrections: the **channels** (the operation, polarity, DC, mute), the **delays** (in ms
//! or in samples), the **image** (balance, width) and the **gain**.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use egui::{vec2, Rect, Sense};

#[path = "libra_balance.rs"]
mod balance;
#[path = "libra_display.rs"]
mod display;

const KIND: EffectKind = EffectKind::Utility;

/// Factory presets: data (`presets/effects/utility.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The channel operations' names (`libra.chan.{i}`).
pub const CHAN_KEYS: [&str; 7] = [
    "libra.chan.0",
    "libra.chan.1",
    "libra.chan.2",
    "libra.chan.3",
    "libra.chan.4",
    "libra.chan.5",
    "libra.chan.6",
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    view: display::View,
    /// Where the display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
}

/// Libra's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.libra);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "libra"), &mut st, ui);
    app.fx.libra = st;
    out
}

/// Libra's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: super::modfx::out_meter(&tel),
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

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let row = controls::card_height(Size::M) * k;
    let gap = s.d(4.0);
    let top = (area.height() - row - gap).max(220.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    controls::card(ui, s, r1, Src::Key("libra.card.measure"), None, |ui, cr| {
        let r = cr.body;
        st.display = Some(r);
        st.view.update(host.pairs(), host.sample_rate(), tel);
        display::edit(host, &mut st.view, ui, r, k);
        let ink = super::canvas::Ink::of(s);
        let spec = host.spec().clone();
        with_brush(&ui.painter_at(r), |b| {
            super::canvas::ground_b(b, r, &ink, s.hue_tok);
            display::paint(b, s, r, &mut st.view, &spec, tel, k);
            super::canvas::edge_b(b, r, &ink);
        });
        if super::modfx::out_meter(tel).is_some() {
            ui.ctx().request_repaint();
        }
    });
    let r2: Vec<Rect> = g.row(row, &[0.27, 0.17, 0.25, 0.15, 0.16]).collect();
    channels(host, ui, s, r2[0]);
    switches(host, ui, s, r2[1]);
    delays(host, ui, s, r2[2]);
    image(host, ui, s, r2[3]);
    gain(host, ui, s, r2[4]);
}

/// The channels: the operation as keys.
fn channels(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("libra.card.channels"),
        None,
        |ui, _| {
            let w = 280.0;
            controls::column(ui, s, w, |ui| {
                controls::caption(ui, s, Src::Key("libra.l.operation"), w);
                chan_keys(host, ui, s, w);
            });
        },
    );
}

/// The switches: each channel's polarity, DC removal and mute.
fn switches(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("libra.card.switches"),
        None,
        |ui, _| {
            let w = 168.0;
            controls::column(ui, s, w, |ui| {
                controls::caption(ui, s, Src::Key("libra.l.polarity"), w);
                let half = (w - 4.0) * 0.5;
                ui.horizontal(|ui| {
                    controls::led_param(ui, s, host, "pol.l", Src::Key("libra.l.left"), half);
                    controls::led_param(ui, s, host, "pol.r", Src::Key("libra.l.right"), half);
                });
                ui.horizontal(|ui| {
                    controls::led_param(ui, s, host, "dc", Src::Key("libra.l.dc"), half);
                    controls::led_param(ui, s, host, "mute", Src::Key("libra.l.mute"), half);
                });
            });
        },
    );
}

/// A cell on `key` with its own short label (the card says what it belongs to).
fn labelled(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &'static str,
    label: &'static str,
    pill: Option<controls::Pill<'_>>,
) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let mut c = controls::Cell::param(info, spec.get(key))
        .size(Size::M)
        .width(90.0)
        .label(Src::Key(label));
    if let Some(f) = pill {
        c = c.format(f);
    }
    let out = c.show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

/// The operation's seven keys, four and three.
fn chan_keys(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, width: f32) {
    let k = s.k;
    let now = host.spec().get("chan").round().max(0.0) as usize;
    let (cols, gap, h) = (3usize, s.d(3.0), s.d(controls::ROW));
    let rows = CHAN_KEYS.len().div_ceil(cols) as f32;
    let (rect, _) = ui.allocate_exact_size(
        vec2(width * k, rows * h + (rows - 1.0) * gap),
        Sense::hover(),
    );
    let w = (rect.width() - gap * (cols - 1) as f32) / cols as f32;
    let name = frame::tr("fx.libra.chan");
    for (i, key) in CHAN_KEYS.iter().enumerate() {
        let r = Rect::from_min_size(
            rect.min + vec2((i % cols) as f32 * (w + gap), (i / cols) as f32 * (h + gap)),
            vec2(w, h),
        );
        let resp = ui.interact(r, ui.id().with(("libra-chan", i)), Sense::click());
        let st = crate::kit::State::of(&resp, i == now);
        if ui.is_rect_visible(r) {
            with_brush(ui.painter(), |b| {
                controls::paint_key(b, r, s, st, Src::Key(key))
            });
        }
        let learnt = controls::stepped_offer(ui, s, host, "chan", name, &resp);
        if resp.clicked() && !learnt && i != now {
            host.set("chan", i as f32, None);
        }
        if resp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                frame::tr(key),
                frame::tr(CHAN_TIPS[i]),
                crate::kit::hint::Caps::NONE,
            );
        }
    }
}

const CHAN_TIPS: [&str; 7] = [
    "libra.chan.0.tip",
    "libra.chan.1.tip",
    "libra.chan.2.tip",
    "libra.chan.3.tip",
    "libra.chan.4.tip",
    "libra.chan.5.tip",
    "libra.chan.6.tip",
];

/// A delay's value in the unit chosen: ms, or whole samples at the session's rate.
fn delays(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    let samples = host.spec().get("delay.unit") >= 0.5;
    let sr = host.sample_rate();
    controls::card(ui, s, rect, Src::Key("libra.card.delays"), None, |ui, _| {
        let pill = move |v: f32, out: &mut String| {
            if samples {
                let n = (v * 0.001 * sr).round();
                // The unit is the switch beside them ("Shown in: Samples"): the whole
                // word does not fit a pill.
                let _ = std::fmt::Write::write_fmt(out, format_args!("{n:.0}"));
            } else {
                controls::write_readout(v, "ms", out);
            }
        };
        labelled(host, ui, s, "delay.l", "libra.l.left", Some(&pill));
        labelled(host, ui, s, "delay.r", "libra.l.right", Some(&pill));
        let w = 76.0;
        controls::column(ui, s, w, |ui| {
            controls::caption(ui, s, Src::Key("libra.l.shown_in"), w);
            controls::led_param(ui, s, host, "delay.unit", Src::Key("libra.unit.1"), w);
        });
    });
}

/// The balance as its words: "C", "L 30", "R 100".
fn pan_pill(v: f32, out: &mut String) {
    let p = (v * 100.0).round() as i32;
    if p == 0 {
        out.push('C');
    } else if p < 0 {
        let _ = std::fmt::Write::write_fmt(out, format_args!("L {}", -p));
    } else {
        let _ = std::fmt::Write::write_fmt(out, format_args!("R {p}"));
    }
}

fn image(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("libra.card.image"), None, |ui, _| {
        controls::param_with(ui, s, host, "pan", Size::M, Some(&pan_pill));
        controls::param(ui, s, host, "width", Size::M);
    });
}

fn gain(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("libra.card.gain"), None, |ui, _| {
        controls::param(ui, s, host, "out", Size::M);
    });
}

/// The inspector's picture: the gain and the operations in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let ls = cantus_engine::fx::libra::settings(spec);
    let mut w = String::new();
    controls::write_readout(ls.gain_db, "dB", &mut w);
    if ls.chan != cantus_dsp::libra::Chan::Stereo {
        w.push_str(" \u{b7} ");
        w.push_str(frame::tr(CHAN_KEYS[ls.chan as usize]));
    }
    if ls.invert_l || ls.invert_r {
        w.push_str(" \u{b7} \u{d8}");
    }
    controls::readout(
        ui,
        &s,
        &w,
        false,
        frame::tr(spec.kind.name_key()),
        ui.available_width(),
    )
}

/// The settings `--editor libra` and the screenshot tool open it with: two microphones
/// aligned (the right 0.6 ms late), a little narrower, the gain down.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "libra").set_many(DEMO, "cmd.fx_preset");
}

pub const DEMO: &[(&str, f32)] = &[
    ("out", -3.0),
    ("delay.r", 0.6),
    ("width", 0.85),
    ("pan", -0.1),
    ("dc", 1.0),
];

/// What a playing engine would feed the editor, for screenshots with no engine: a sung A
/// a few cents flat through the unit, its telemetry, the tap and the goniometer's cloud.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::libra::Libra::new(48_000.0, cantus_core::balance_gains);
    d.set(&cantus_engine::fx::libra::settings(&spec));
    d.reset();
    let sr = 48_000.0f32;
    let f0 = 440.0 * 2f32.powf(-7.0 / 1200.0);
    let mut pairs = Vec::new();
    let mut blocks = Vec::new();
    let mut tel = [0.0f32; 64];
    let mut phase = 0.0f32;
    for blk in 0..120 {
        let n = 480;
        let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
        for i in 0..n {
            let t = (blk * n + i) as f32 / sr;
            let vib = 1.0 + 0.003 * (t * 5.5 * std::f32::consts::TAU).sin();
            phase = (phase + f0 * vib / sr).fract();
            let x = (phase * std::f32::consts::TAU).sin();
            let voice = 0.3 * x + 0.12 * (2.0 * phase * std::f32::consts::TAU).sin();
            // The left microphone 0.6 ms further away: the same voice, later.
            let late = (phase - f0 * 0.000_6).rem_euclid(1.0);
            let far = 0.3 * (late * std::f32::consts::TAU).sin()
                + 0.12 * (2.0 * late * std::f32::consts::TAU).sin();
            l[i] = far * 0.9;
            r[i] = voice * 0.8 + 0.02 * (t * 1_300.0).sin();
        }
        let dry = l.clone();
        d.process(&mut l, &mut r);
        for (a, b) in dry.iter().zip(&l) {
            pairs.push(*a);
            pairs.push(*b);
        }
        d.telemetry(&mut tel);
        let mut pts = [0.0f32; 2 * cantus_dsp::libra::POINTS];
        pts.copy_from_slice(
            &tel[cantus_dsp::libra::t::POINTS
                ..cantus_dsp::libra::t::POINTS + 2 * cantus_dsp::libra::POINTS],
        );
        blocks.push(pts);
    }
    tel[cantus_dsp::modfx_clock::tel::PLAYING] = 1.0;
    app.fx.injected = Some(tel);
    app.fx.feed(&pairs);
    if let Some((_, effect, _)) = app.fx.open {
        if app.fx.libra.effect != Some(effect) {
            app.fx.libra = State {
                effect: Some(effect),
                ..State::default()
            };
        }
    }
    let corr = tel[cantus_dsp::libra::t::CORR];
    let tail = blocks.len().saturating_sub(24);
    app.fx.libra.view.seed(&blocks[tail..], corr, f0);
}
