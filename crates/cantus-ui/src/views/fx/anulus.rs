//! Anulus's editor: the ring modulator (ADR 0055).
//!
//! A ring modulator's sound is hard to predict from its knobs: the partials it makes are
//! sums and differences, not harmonics. So beside the carrier's frequency (in hertz and as
//! the nearest note, with cents) and its movement over two LFO cycles, the editor draws
//! **what a 1 kHz tone becomes**: the input's line and every partial the carrier makes of
//! it — two for a sine, the odd harmonics' pairs for a triangle or a square, at their real
//! levels.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx::{self, LfoCard};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::{self as ktext, Role, Src, Text};
use crate::kit::{with_brush, Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::anulus::{AnulusSettings, Wave};
use cantus_dsp::modfx_lfo::Shape;
use egui::{pos2, vec2, Align2, Rect};
use std::fmt::Write as _;

const KIND: EffectKind = EffectKind::RingMod;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    ("preset.anulus.bell", &[("freq", 587.0), ("mix", 0.6)]),
    (
        "preset.anulus.robot",
        &[("freq", 55.0), ("wave", 2.0), ("mix", 1.0)],
    ),
    ("preset.anulus.dalek", &[("freq", 30.0), ("mix", 1.0)]),
    (
        "preset.anulus.through_zero_tremolo",
        &[("freq", 4.0), ("wave", 1.0), ("mix", 1.0)],
    ),
    (
        "preset.anulus.siren",
        &[
            ("freq", 700.0),
            ("lfo", 12.0),
            ("rate", 0.4),
            ("shape", 1.0),
            ("mix", 0.8),
        ],
    ),
    (
        "preset.anulus.metal_drums",
        &[("freq", 1_250.0), ("wave", 2.0), ("mix", 0.45)],
    ),
    (
        "preset.anulus.stereo_shimmer",
        &[("freq", 2_200.0), ("spread", 90.0), ("mix", 0.3)],
    ),
    (
        "preset.anulus.random_steps",
        &[
            ("freq", 330.0),
            ("lfo", 7.0),
            ("sync", 1.0),
            ("div", 11.0),
            ("shape", 5.0),
            ("mix", 0.7),
        ],
    ),
];

const WAVE_KEYS: [&str; 3] = ["anulus.wave.0", "anulus.wave.1", "anulus.wave.2"];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    ends: (f32, f32, String, String),
    /// The carrier in hertz and as a note, formatted when it changes.
    note: (f32, String),
    /// Where the carrier display was drawn last frame.
    pub display: Option<Rect>,
}

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.anulus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "anulus"), &mut st, ui);
    app.fx.anulus = st;
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
    let a = cantus_engine::fx::anulus::settings(host.spec());
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &a)
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

const WAVE_W: f32 = 210.0;
const PARTIALS_W: f32 = 420.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    a: &AnulusSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(200.0 * k);
    let total = area.width() - gap;
    let pw = PARTIALS_W * k;
    let r1: Vec<Rect> = g.row(row1, &[(total - pw) / total, pw / total]).collect();
    let sm = s.function(Function::Mod);
    carrier_card(host, st, ui, &sm, r1[0], tel, a);
    partials_card(ui, &s.function(Function::Tone), r1[1], a);

    let w = [
        controls::cells_width(&[Size::XL, Size::M]) + 4.0 + WAVE_W + 16.0,
        modfx::lfo_card_width(&LFO),
        controls::cells_width(&[Size::M; 3]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(
        ui,
        &sm,
        r2[0],
        Src::Key("anulus.card.carrier"),
        None,
        |ui, _| {
            controls::param_with(ui, &sm, host, "freq", Size::XL, Some(&freq_pill));
            controls::param_with(ui, &sm, host, "lfo", Size::M, Some(&st_pill));
            controls::column(ui, &sm, WAVE_W, |ui| {
                controls::caption(ui, &sm, Src::Key("anulus.wave_caption"), WAVE_W);
                let labels = WAVE_KEYS.map(Src::Key);
                controls::segmented_param(ui, &sm, host, "wave", &labels, WAVE_W);
            });
        },
    );
    modfx::region(ui, r2[1], a.lfo_st > 0.0, |ui| {
        modfx::lfo_card(host, ui, &sm, r2[1], &LFO);
    });
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        r2[2],
        Src::Key("anulus.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "spread", Size::M, Some(&modfx::deg_pill));
            controls::param_with(ui, &so, host, "mix", Size::M, Some(&modfx::pct_pill));
            controls::param_with(ui, &so, host, "out", Size::M, Some(&modfx::db_pill));
        },
    );
}

/// The note names, from C.
const NOTES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// A frequency as the nearest note and its cents: `A4 +0`.
pub fn note_of(hz: f32, out: &mut String) {
    let m = 69.0 + 12.0 * cantus_dsp::math::log2(hz.max(1e-3) / 440.0);
    let n = m.round();
    let cents = ((m - n) * 100.0).round() as i32;
    let i = (n as i32).rem_euclid(12) as usize;
    let octave = (n as i32).div_euclid(12) - 1;
    let _ = write!(out, "{}{} {:+}", NOTES[i], octave, cents);
}

fn freq_pill(v: f32, b: &mut String) {
    modfx::hz_pill(v, b);
}

fn st_pill(v: f32, b: &mut String) {
    let _ = write!(b, "\u{b1}{v:.1} st");
}

fn carrier_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    a: &AnulusSettings,
) {
    let f = a.freq_hz;
    let span = a.lfo_st.max(1.0);
    if st.ends.0 != f || st.ends.1 != span || st.ends.2.is_empty() {
        st.ends.0 = f;
        st.ends.1 = span;
        st.ends.2.clear();
        st.ends.3.clear();
        let hi = f * cantus_dsp::math::exp2(span / 12.0);
        let lo = f * cantus_dsp::math::exp2(-span / 12.0);
        controls::write_readout(hi, "Hz", &mut st.ends.2);
        controls::write_readout(lo, "Hz", &mut st.ends.3);
    }
    let title = Src::Key("anulus.card.frequency");
    controls::card(ui, s, rect, title, None, |ui, cr| {
        let r = cr.body;
        st.display = Some(r);
        let swing = a.lfo_st / span;
        let curve = |c: f64| {
            let (l, _) = modfx::lfo_at(&a.lfo, 0.0, c);
            (l * swing, l * swing)
        };
        modfx::movement(ui, s, r, host, tel, &curve, &st.ends.2, &st.ends.3);
        // The carrier's note, under the rate.
        let ink = Ink::of(s);
        let k = s.k;
        if st.note.0 != f || st.note.1.is_empty() {
            st.note.0 = f;
            st.note.1.clear();
            controls::write_readout(f, "Hz", &mut st.note.1);
            st.note.1.push_str(" \u{b7} ");
            note_of(f, &mut st.note.1);
        }
        with_brush(&ui.painter_at(r), |b| {
            let t = Text::new(&st.note.1, Role::Value, ink.text);
            // Under the rate's plate (a Value line at the top right), clear of the curve's
            // start and of the centre line.
            let line = ktext::measure(b.ctx(), &t, k).y;
            let at = pos2(r.right() - 6.0 * k, r.top() + 3.0 * k + line + 4.0 * k);
            canvas::put(b, at, Align2::RIGHT_TOP, &t, k, Some(ink.plate));
        });
        if a.mix == 0.0 {
            modfx::idle_note(ui, s, r, "anulus.idle");
        }
    });
}

/// The partials a 1 kHz tone becomes, with their levels (the carrier's harmonics
/// `n`: sine `n = 1`; triangle odd `n` at `1/n²`; square odd `n` at `1/n`, each pair
/// `1 kHz ± n·f` at half the harmonic's level).
pub fn partials(a: &AnulusSettings, out: &mut [(f32, f32); 32]) -> usize {
    let tone = 1_000.0f32;
    let mut count = 0;
    let (top, falloff) = match a.wave {
        Wave::Sine => (1, 0),
        Wave::Triangle => (15, 2),
        Wave::Square => (15, 1),
    };
    let scale = match a.wave {
        Wave::Sine => 1.0,
        Wave::Triangle => 8.0 / (std::f32::consts::PI * std::f32::consts::PI),
        Wave::Square => 4.0 / std::f32::consts::PI,
    };
    let mut n = 1;
    while n <= top && count + 2 <= out.len() {
        let amp = scale / cantus_dsp::math::powi(n as f32, falloff) * 0.5;
        for f in [
            tone + n as f32 * a.freq_hz,
            (tone - n as f32 * a.freq_hz).abs(),
        ] {
            out[count] = (f, amp);
            count += 1;
        }
        n += 2;
    }
    count
}

fn partials_card(ui: &mut egui::Ui, s: &Style, rect: Rect, a: &AnulusSettings) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("anulus.card.partials"),
        None,
        |ui, cr| {
            let r = cr.body;
            let ink = Ink::of(s);
            let k = s.k;
            let plot = r.shrink2(vec2(8.0 * k, 16.0 * k));
            let x_of = |f: f32| {
                plot.left()
                    + plot.width() * (cantus_dsp::math::log10(f.clamp(20.0, 20_000.0) / 20.0) / 3.0)
            };
            let (top, bottom) = (0.0f32, -48.0f32);
            let y_of = |amp: f32| {
                let db = (20.0 * cantus_dsp::math::log10(amp.max(1e-6))).clamp(bottom, top);
                plot.top() + plot.height() * (top - db) / (top - bottom)
            };
            let mut parts = [(0.0f32, 0.0f32); 32];
            let n = partials(a, &mut parts);
            with_brush(&ui.painter_at(r), |b| {
                canvas::ground_b(b, r, &ink, s.hue_tok);
                for db in [-12.0f32, -24.0, -36.0] {
                    let y = plot.top() + plot.height() * (top - db) / (top - bottom);
                    b.hline(plot.left(), plot.right(), y, b.px(), ink.grid);
                }
                // The input: a faint line at 1 kHz, at full level.
                let x_in = x_of(1_000.0);
                canvas::dashed(
                    b,
                    pos2(x_in, plot.top()),
                    pos2(x_in, plot.bottom()),
                    1.2 * k,
                    ink.faint,
                    4.0 * k,
                    3.0 * k,
                );
                for &(f, amp) in &parts[..n] {
                    if !(20.0..=20_000.0).contains(&f) {
                        continue;
                    }
                    let x = x_of(f);
                    b.segment(
                        pos2(x, plot.bottom()),
                        pos2(x, y_of(amp)),
                        2.0 * k,
                        ink.hue,
                        false,
                    );
                    b.dot(pos2(x, y_of(amp)), 2.5 * k, ink.hue);
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
                let t = Text::key("anulus.input", Role::Micro, ink.dim).caps(true);
                let word = canvas::put(
                    b,
                    pos2(x_in + 4.0 * k, plot.top()),
                    Align2::LEFT_TOP,
                    &t,
                    k,
                    Some(ink.plate),
                );
                // The tone's frequency under its name, in its own case (a title in capitals
                // would print "KHZ").
                let t = Text::new("1 kHz", Role::Micro, ink.text);
                canvas::put(
                    b,
                    pos2(word.left(), word.bottom() + 2.0 * k),
                    Align2::LEFT_TOP,
                    &t,
                    k,
                    Some(ink.plate),
                );
                canvas::edge_b(b, r, &ink);
            });
        },
    );
}

/// The rate the inspector's line names: the LFO's, only while it swings the carrier.
pub fn preview_rate(spec: &EffectSpec) -> modfx::PreviewRate<'_> {
    if cantus_engine::fx::anulus::settings(spec).lfo_st > 0.0 {
        modfx::PreviewRate::Spec(spec)
    } else {
        modfx::PreviewRate::Off
    }
}

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let a = cantus_engine::fx::anulus::settings(spec);
    let freq = modfx::Unit(a.freq_hz, "Hz");
    let words = modfx::PreviewWords {
        mode: frame::tr("anulus.preview"),
        rate: preview_rate(spec),
        amount: &freq,
    };
    modfx::preview_strip(ui, spec, &words, &|c| {
        cantus_dsp::anulus::carrier(a.wave, c * 2.0 - (c * 2.0).floor(), 1e-4)
    })
}

/// The settings `--editor anulus` and the screenshot tool open it with: a square carrier
/// a fifth above A, swung by its LFO.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "anulus").set_many(
        &[
            ("freq", 660.0),
            ("wave", 2.0),
            ("lfo", 5.0),
            ("rate", 0.5),
            ("mix", 0.7),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::anulus::Anulus::new(48_000.0);
    d.set(&cantus_engine::fx::anulus::settings(&spec));
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
