//! Concentus's editor: the chorus (Juno, Dimension) (ADR 0055).
//!
//! The references (TAL-Chorus-LX and the Juno's own buttons, Roland's Dimension D and its
//! clones, FL's Fruity Chorus, Arturia's and Waves' ensembles) show buttons and knobs; none
//! shows what the voices do, and none says what happens in mono — the commonest complaint
//! about a wide chorus. Here every voice's delay is drawn over two cycles, and a **mono
//! check** reads the output's L/R correlation and how much level its mono sum loses, with a
//! verdict in words.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::modfx::{self, LfoCard};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Fill, Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::concentus::{Concentus, ConcentusSettings, Mode, T_CORR, T_MONO_DB};
use cantus_dsp::modfx_lfo::Shape;
use egui::{pos2, vec2, Align2, Rect};
use std::fmt::Write as _;

const KIND: EffectKind = EffectKind::Chorus;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    ("preset.concentus.juno_one", &[("mode", 0.0), ("mix", 0.5)]),
    ("preset.concentus.juno_two", &[("mode", 1.0), ("mix", 0.5)]),
    ("preset.concentus.juno_both", &[("mode", 2.0), ("mix", 0.5)]),
    (
        "preset.concentus.dim_subtle",
        &[("mode", 3.0), ("mix", 0.5), ("tone", 12_000.0)],
    ),
    (
        "preset.concentus.dim_wide",
        &[("mode", 6.0), ("mix", 0.5), ("tone", 12_000.0)],
    ),
    (
        "preset.concentus.string_ensemble",
        &[
            ("mode", 7.0),
            ("mix", 0.55),
            ("voices", 3.0),
            ("rate", 0.6),
            ("shape", 0.0),
            ("delay", 9.0),
            ("depth", 3.0),
        ],
    ),
    (
        "preset.concentus.vocal_double",
        &[
            ("mode", 7.0),
            ("mix", 0.35),
            ("voices", 2.0),
            ("rate", 0.25),
            ("delay", 18.0),
            ("depth", 1.5),
            ("tone", 14_000.0),
        ],
    ),
    (
        "preset.concentus.lush_four",
        &[
            ("mode", 7.0),
            ("mix", 0.5),
            ("voices", 4.0),
            ("rate", 0.9),
            ("delay", 12.0),
            ("depth", 4.0),
        ],
    ),
    (
        "preset.concentus.synced_swirl",
        &[
            ("mode", 7.0),
            ("mix", 0.5),
            ("voices", 2.0),
            ("sync", 1.0),
            ("div", 3.0),
            ("delay", 8.0),
            ("depth", 5.0),
        ],
    ),
    (
        "preset.concentus.mono_bass",
        &[
            ("mode", 1.0),
            ("mix", 0.4),
            ("spread", 0.0),
            ("tone", 6_000.0),
        ],
    ),
];

/// The models' names and what each does, by index.
const MODE_KEYS: [&str; 8] = [
    "concentus.mode.0",
    "concentus.mode.1",
    "concentus.mode.2",
    "concentus.mode.3",
    "concentus.mode.4",
    "concentus.mode.5",
    "concentus.mode.6",
    "concentus.mode.7",
];
const MODE_WORDS: [&str; 8] = [
    "concentus.words.0",
    "concentus.words.1",
    "concentus.words.2",
    "concentus.words.3",
    "concentus.words.4",
    "concentus.words.5",
    "concentus.words.6",
    "concentus.words.7",
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The display's top ("6.7 ms"), formatted when it changes.
    top: (f32, String),
    /// Where the voices' display was drawn last frame.
    pub display: Option<Rect>,
}

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.concentus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "concentus"),
        &mut st,
        ui,
    );
    app.fx.concentus = st;
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
    let cs = cantus_engine::fx::concentus::settings(host.spec());
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &cs)
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
    spread: false,
    offset: false,
};

const MODE_W: f32 = 196.0;
const MONO_W: f32 = 212.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    cs: &ConcentusSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(260.0 * k);
    let total = area.width() - 2.0 * gap;
    let (mw, nw) = (MODE_W * k, MONO_W * k);
    let r1: Vec<Rect> = g
        .row(row1, &[mw / total, (total - mw - nw) / total, nw / total])
        .collect();
    let sm = s.function(Function::Space);
    modfx::mode_card(
        host,
        ui,
        &sm,
        r1[0],
        "mode",
        "fx.concentus.mode",
        &MODE_KEYS,
        &MODE_WORDS,
        &|b, r, i, ink| mode_glyph(b, r, i, ink, k),
    );
    voices_card(host, st, ui, &sm, r1[1], tel, cs);
    mono_card(ui, &s.function(Function::Output), r1[2], tel, cs);

    let free = cs.mode == Mode::Free;
    let w = [
        controls::cells_width(&[Size::XL]) + 16.0,
        modfx::lfo_card_width(&LFO),
        controls::cells_width(&[Size::M; 3]) + 16.0,
        controls::cells_width(&[Size::M; 3]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let r2: Vec<Rect> = g.row(row2, &w.map(|x| x / sum)).collect();
    controls::card(
        ui,
        &sm,
        r2[0],
        Src::Key("concentus.card.mix"),
        None,
        |ui, _| {
            controls::param_with(ui, &sm, host, "mix", Size::XL, Some(&modfx::pct_pill));
        },
    );
    let smod = s.function(Function::Mod);
    modfx::region(ui, r2[1], free, |ui| {
        modfx::lfo_card(host, ui, &smod, r2[1], &LFO);
    });
    modfx::region(ui, r2[2], free, |ui| {
        controls::card(
            ui,
            &smod,
            r2[2],
            Src::Key("concentus.card.voices"),
            None,
            |ui, _| {
                controls::param_with(ui, &smod, host, "voices", Size::M, Some(&count_pill));
                controls::param_with(ui, &smod, host, "delay", Size::M, Some(&modfx::ms_pill));
                controls::param_with(ui, &smod, host, "depth", Size::M, Some(&modfx::ms_pill));
            },
        );
    });
    let so = s.function(Function::Output);
    controls::card(
        ui,
        &so,
        r2[3],
        Src::Key("concentus.card.output"),
        None,
        |ui, _| {
            controls::param_with(ui, &so, host, "spread", Size::M, Some(&modfx::pct_pill));
            controls::param_with(ui, &so, host, "tone", Size::M, Some(&tone_pill));
            controls::param_with(ui, &so, host, "out", Size::M, Some(&modfx::db_pill));
        },
    );
}

fn tone_pill(v: f32, b: &mut String) {
    if v >= cantus_dsp::concentus::TONE_OPEN {
        b.push_str(frame::tr("concentus.open"));
    } else {
        controls::write_readout(v, "Hz", b);
    }
}

/// Every voice's delay `c` cycles in, ms: the left and right of a model, or each free voice.
pub fn delays_at(cs: &ConcentusSettings, c: f64) -> [f32; 4] {
    let lfo = Concentus::lfo_of(cs);
    let at = |off: f64| modfx::lfo_at(&lfo, 0.0, c + off).0;
    match cs.mode.voicing() {
        Some(v) => {
            let l = v.centre_ms + v.sweep_ms * at(0.0);
            let r = v.centre_ms + v.sweep_ms * at(0.5 * f64::from(cs.spread));
            [l, r, l, r]
        }
        None => {
            let n = cs.voices.clamp(1, 4) as usize;
            let centre = cs.delay_ms.clamp(1.0, 30.0);
            let sweep = cs.depth_ms.clamp(0.0, 8.0).min((centre - 0.05).max(0.0));
            let mut out = [centre; 4];
            for (i, d) in out.iter_mut().enumerate().take(n) {
                *d = centre + sweep * at(i as f64 / n as f64);
            }
            if n == 1 {
                out[1] = centre + sweep * at(0.5 * f64::from(cs.spread));
            }
            out
        }
    }
}

/// How many curves the voices' display draws.
fn curves(cs: &ConcentusSettings) -> usize {
    match cs.mode {
        Mode::Free => (cs.voices.clamp(1, 4) as usize).max(2),
        _ => 2,
    }
}

fn voices_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    cs: &ConcentusSettings,
) {
    // The scale: the longest delay the model reaches, and a fifth again.
    let (centre, sweep) = match cs.mode.voicing() {
        Some(v) => (v.centre_ms, v.sweep_ms),
        None => (cs.delay_ms, cs.depth_ms),
    };
    let top = ((centre + sweep) * 1.2).max(1.0);
    if st.top.0 != top || st.top.1.is_empty() {
        st.top.0 = top;
        st.top.1.clear();
        let _ = write!(st.top.1, "{top:.1} ms");
    }
    let norm = move |d: f32| 2.0 * d / top - 1.0;
    let n = curves(cs);
    let c2 = *cs;
    let v3 = move |c: f64| norm(delays_at(&c2, c)[2]);
    let v4 = move |c: f64| norm(delays_at(&c2, c)[3]);
    let all: [&dyn Fn(f64) -> f32; 2] = [&v3, &v4];
    let more = &all[..n.saturating_sub(2)];
    let hz = cs.mode.voicing().map(|v| v.rate);
    controls::card(
        ui,
        s,
        rect,
        Src::Key("concentus.card.delays"),
        None,
        |ui, cr| {
            let r = cr.body;
            st.display = Some(r);
            let curve = |c: f64| {
                let d = delays_at(cs, c);
                (norm(d[0]), norm(d[1]))
            };
            let extras = modfx::Extras { more, hz };
            modfx::movement_with(ui, s, r, host, tel, &curve, &st.top.1, "0 ms", &extras);
            if cs.mix == 0.0 {
                modfx::idle_note(ui, s, r, "concentus.idle");
            }
        },
    );
}

/// The mono check: the correlation on a bar from −1 to +1, the mono sum's loss, a verdict.
fn mono_card(ui: &mut egui::Ui, s: &Style, rect: Rect, tel: &[f32; 64], cs: &ConcentusSettings) {
    let seen = tel[cantus_dsp::modfx_clock::tel::BLOCKS] > 0.0;
    let corr = if seen { tel[T_CORR] } else { 1.0 };
    let loss = if seen { tel[T_MONO_DB] } else { 0.0 };
    let verdict = if cs.mix == 0.0 || (corr >= 0.2 && loss > -3.0) {
        "concentus.mono.safe"
    } else if corr < -0.2 || loss <= -6.0 {
        "concentus.mono.thin"
    } else {
        "concentus.mono.mostly"
    };
    controls::card(
        ui,
        s,
        rect,
        Src::Key("concentus.card.mono"),
        None,
        |ui, cr| {
            let ink = Ink::of(s);
            let k = s.k;
            let inner = cr.body;
            let bar = Rect::from_min_size(
                pos2(inner.left() + 6.0 * k, inner.top() + 22.0 * k),
                vec2(inner.width() - 12.0 * k, 12.0 * k),
            );
            with_brush(&ui.painter_at(inner.expand(2.0)), |b| {
                let t = Text::key("concentus.mono.corr", Role::Micro, ink.dim).caps(true);
                canvas::put(
                    b,
                    pos2(bar.left(), inner.top() + 4.0 * k),
                    Align2::LEFT_TOP,
                    &t,
                    k,
                    None,
                );
                b.rrect(bar, 3.0 * k, Fill::Solid(ink.ground));
                let mid = bar.center().x;
                b.vline(mid, bar.top(), bar.bottom(), b.px(), ink.grid_strong);
                let x = mid + corr.clamp(-1.0, 1.0) * bar.width() * 0.5;
                let fill = Rect::from_x_y_ranges(mid.min(x)..=mid.max(x), bar.y_range());
                let color = if corr < 0.0 { ink.danger } else { ink.ok };
                b.rect(fill, Fill::Solid(Ink::a(color, 0.8)));
                b.rrect_stroke(bar, 3.0 * k, b.px(), ink.edge);
                for (word, at, align) in [
                    ("\u{2212}1", bar.left(), Align2::LEFT_TOP),
                    ("0", mid, Align2::CENTER_TOP),
                    ("+1", bar.right(), Align2::RIGHT_TOP),
                ] {
                    let t = Text::new(word, Role::Micro, ink.dim);
                    canvas::put(b, pos2(at, bar.bottom() + 2.0 * k), align, &t, k, None);
                }
                canvas::tag(
                    b,
                    pos2(bar.left(), bar.bottom() + 24.0 * k),
                    Align2::LEFT_TOP,
                    frame::tr("concentus.mono.sum"),
                    ink.text,
                    k,
                    None,
                    format_args!("{loss:+.1} dB"),
                );
                let words = Text::key(verdict, Role::Body, ink.text).caps(false);
                canvas::put(
                    b,
                    pos2(bar.left(), bar.bottom() + 46.0 * k),
                    Align2::LEFT_TOP,
                    &words,
                    k,
                    None,
                );
            });
        },
    );
}

/// A model's picture: one sweep (Juno), two crossed (Dimension), several (Free).
fn mode_glyph(b: &mut crate::kit::Brush, r: Rect, i: usize, ink: egui::Color32, k: f32) {
    const N: usize = 17;
    let lines = match i {
        0..=2 => 1,
        3..=6 => 2,
        _ => 3,
    };
    let rate = match i {
        2 => 3.0,
        6 => 2.0,
        _ => 1.0,
    };
    for v in 0..lines {
        let mut pts = [egui::Pos2::ZERO; N];
        for (j, p) in pts.iter_mut().enumerate() {
            let x = j as f32 / (N - 1) as f32;
            let ph = x * rate + v as f32 / lines as f32;
            let tri = 1.0 - 4.0 * ((ph + 0.25).fract() - 0.5).abs();
            *p = pos2(
                r.left() + r.width() * x,
                r.center().y - tri * r.height() * 0.4,
            );
        }
        let c = if v == 0 {
            ink
        } else {
            crate::kit::color::with_alpha(ink, 0.5)
        };
        b.polyline(&pts, 1.4 * k, c);
    }
}

/// The rate the inspector's line names: a Juno or Dimension model's own, the LFO's in
/// the free modes.
pub fn preview_rate(spec: &EffectSpec) -> modfx::PreviewRate<'_> {
    match cantus_engine::fx::concentus::settings(spec).mode.voicing() {
        Some(v) => modfx::PreviewRate::Hz(v.rate as f32),
        None => modfx::PreviewRate::Spec(spec),
    }
}

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let cs = cantus_engine::fx::concentus::settings(spec);
    let mode = frame::tr(MODE_KEYS[cs.mode.index()]);
    let words = modfx::PreviewWords {
        mode,
        rate: preview_rate(spec),
        amount: &modfx::Pct(cs.mix),
    };
    modfx::preview_strip(ui, spec, &words, &|c| {
        let d = delays_at(&cs, c)[0];
        let (centre, sweep) = match cs.mode.voicing() {
            Some(v) => (v.centre_ms, v.sweep_ms),
            None => (cs.delay_ms, cs.depth_ms.max(0.01)),
        };
        (d - centre) / sweep.max(0.01)
    })
}

/// The settings `--editor concentus` and the screenshot tool open it with: the Juno's
/// second button, at the instrument's own mix.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "concentus")
        .set_many(&[("mode", 1.0), ("mix", 0.5)], "cmd.fx_preset");
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut d = Concentus::new(48_000.0);
    d.set(&cantus_engine::fx::concentus::settings(&spec));
    d.reset();
    modfx::demo_run(
        &mut |l, r, pos| {
            d.set_transport(&pos);
            d.process(l, r);
        },
        1.0,
    );
    let mut t = [0.0f32; 64];
    d.telemetry(&mut t);
    app.fx.injected = Some(t);
}

fn count_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{:.0}", v);
}
