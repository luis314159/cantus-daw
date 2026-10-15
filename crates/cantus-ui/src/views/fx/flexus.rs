//! Flexus's editor: the through-zero flanger (ADR 0055).
//!
//! The references (FL's Fruity Flanger and Flangus, the MXR and Electric Mistress pedals,
//! Soundtoys' PhaseMistress, Kilohearts Flanger) show knobs; the best show an LFO. Flexus
//! shows the **sweep** — the wet's delay against the dry over two cycles, crossing zero
//! when through-zero is on — and the **comb as it is now**: the response the dry and the
//! wet make together at the sweep's present delay, feedback and polarity included, so the
//! notches you hear are the notches you see.

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
use cantus_dsp::flexus::{FlexusSettings, Mode, TZ_CENTRE_MS, T_SWEEP};
use cantus_dsp::modfx_lfo::Shape;
use egui::{pos2, vec2, Align2, Pos2, Rect};
use std::fmt::Write as _;

const KIND: EffectKind = EffectKind::Flanger;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.flexus.jet",
        &[("mix", 0.5), ("fb", 0.75), ("rate", 0.15), ("depth", 6.0)],
    ),
    (
        "preset.flexus.tape_tz",
        &[
            ("tz", 1.0),
            ("invert", 1.0),
            ("mix", 0.5),
            ("rate", 0.12),
            ("depth", 8.0),
            ("shape", 0.0),
        ],
    ),
    (
        "preset.flexus.mistress",
        &[
            ("mix", 0.5),
            ("fb", 0.55),
            ("rate", 0.08),
            ("delay", 1.0),
            ("depth", 5.0),
        ],
    ),
    (
        "preset.flexus.negative_metal",
        &[("mix", 0.5), ("fb", -0.85), ("rate", 0.4), ("depth", 3.0)],
    ),
    (
        "preset.flexus.synced_sweep",
        &[
            ("mix", 0.5),
            ("fb", 0.6),
            ("sync", 1.0),
            ("div", 3.0),
            ("spread", 90.0),
        ],
    ),
    (
        "preset.flexus.drum_bus",
        &[
            ("mix", 0.35),
            ("fb", 0.3),
            ("sync", 1.0),
            ("div", 2.0),
            ("depth", 2.5),
        ],
    ),
    (
        "preset.flexus.envelope",
        &[
            ("mode", 1.0),
            ("mix", 0.5),
            ("fb", 0.65),
            ("sens", 24.0),
            ("rel", 250.0),
        ],
    ),
    (
        "preset.flexus.manual",
        &[("mode", 2.0), ("mix", 0.5), ("fb", 0.8), ("manual", 0.3)],
    ),
];

const MODE_KEYS: [&str; 3] = ["flexus.mode.0", "flexus.mode.1", "flexus.mode.2"];
const MODE_WORDS: [&str; 3] = ["flexus.words.0", "flexus.words.1", "flexus.words.2"];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    ends: (f32, bool, String, String),
    /// Where the sweep display was drawn last frame.
    pub display: Option<Rect>,
}

pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.flexus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "flexus"), &mut st, ui);
    app.fx.flexus = st;
    out
}

pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let fs = cantus_engine::fx::flexus::settings(host.spec());
    // Through zero, the centre; the plain flanger has none.
    let latency = if fs.through_zero {
        cantus_dsp::flexus::tz_centre(sr) as f32
    } else {
        0.0
    };
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: modfx::out_meter(&tel),
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &fs)
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
const COMB_W: f32 = 300.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    fs: &FlexusSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(220.0 * k);
    let total = area.width() - 2.0 * gap;
    let (mw, cw) = (MODE_W * k, COMB_W * k);
    let r1: Vec<Rect> = g
        .row(row1, &[mw / total, (total - mw - cw) / total, cw / total])
        .collect();
    let sm = s.function(Function::Mod);
    // The modes, and the two switches under them.
    let switches = crate::kit::band::TITLE + 8.0 + controls::ROW * 2.0 + 5.0 + 10.0;
    let modes = Rect::from_min_max(
        r1[0].min,
        pos2(r1[0].right(), r1[0].bottom() - switches * k),
    );
    modfx::mode_card(
        host,
        ui,
        &sm,
        modes,
        "mode",
        "fx.flexus.mode",
        &MODE_KEYS,
        &MODE_WORDS,
        &|b, r, i, ink| mode_glyph(b, r, i, ink, k),
    );
    let sw = Rect::from_min_max(pos2(r1[0].left(), modes.bottom() + gap), r1[0].max);
    controls::card(ui, &sm, sw, Src::Key("flexus.card.path"), None, |ui, cr| {
        let w = cr.body.width() / k;
        controls::column(ui, &sm, w, |ui| {
            controls::led_param(ui, &sm, host, "tz", Src::Key("flexus.tz_led"), w);
            controls::led_param(ui, &sm, host, "invert", Src::Key("flexus.invert_led"), w);
        });
    });
    sweep_card(host, st, ui, &sm, r1[1], tel, fs);
    comb_card(ui, &s.function(Function::Tone), r1[2], tel, fs);

    let w = [
        controls::cells_width(&[Size::XL, Size::M, Size::M]) + 16.0,
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
        Src::Key("flexus.card.mix"),
        None,
        |ui, _| {
            controls::param_with(ui, &sm, host, "mix", Size::XL, Some(&modfx::pct_pill));
            controls::param_with(ui, &sm, host, "fb", Size::M, Some(&fb_pill));
            controls::param_with(ui, &sm, host, "out", Size::M, Some(&modfx::db_pill));
        },
    );
    modfx::region(ui, r2[1], fs.mode == Mode::Lfo, |ui| {
        modfx::lfo_card(host, ui, &sm, r2[1], &LFO);
    });
    controls::card(
        ui,
        &sm,
        r2[2],
        Src::Key("flexus.card.sweep"),
        None,
        |ui, _| {
            ui.add_enabled_ui(!fs.through_zero, |ui| {
                controls::param_with(ui, &sm, host, "delay", Size::M, Some(&modfx::ms_pill));
            });
            controls::param_with(ui, &sm, host, "depth", Size::M, Some(&modfx::ms_pill));
            ui.add_enabled_ui(fs.mode == Mode::Lfo, |ui| {
                controls::param_with(ui, &sm, host, "spread", Size::M, Some(&modfx::deg_pill));
            });
        },
    );
    let title = if fs.mode == Mode::Manual {
        "flexus.card.manual"
    } else {
        "flexus.card.envelope"
    };
    let se = s.function(Function::Dynamics);
    modfx::region(ui, r2[3], fs.mode != Mode::Lfo, |ui| {
        controls::card(ui, &se, r2[3], Src::Key(title), None, |ui, _| {
            if fs.mode == Mode::Manual {
                controls::param_with(ui, &se, host, "manual", Size::M, Some(&modfx::pct_pill));
            } else {
                controls::param_with(ui, &se, host, "att", Size::M, Some(&modfx::ms_pill));
                controls::param_with(ui, &se, host, "rel", Size::M, Some(&modfx::ms_pill));
                controls::param_with(ui, &se, host, "sens", Size::M, Some(&modfx::db_pill));
            }
        });
    });
}

fn fb_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{:+.0}%", v * 100.0);
}

/// The wet's delay at `c` cycles in, ms, left and right — against the dry in through-zero
/// mode (negative: ahead of it).
pub fn delay_at(fs: &FlexusSettings, c: f64, now: f32) -> (f32, f32) {
    let (ml, mr) = match fs.mode {
        Mode::Lfo => modfx::lfo_at(&fs.lfo, fs.spread, c),
        Mode::Envelope => (2.0 * now - 1.0, 2.0 * now - 1.0),
        Mode::Manual => (2.0 * fs.manual - 1.0, 2.0 * fs.manual - 1.0),
    };
    let depth = fs.depth_ms.clamp(0.0, 10.0);
    if fs.through_zero {
        let half = (depth * 0.5).min(TZ_CENTRE_MS - 0.03);
        (half * ml, half * mr)
    } else {
        let d = fs.delay_ms.clamp(0.05, 5.0);
        (d + depth * 0.5 * (ml + 1.0), d + depth * 0.5 * (mr + 1.0))
    }
}

fn sweep_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    fs: &FlexusSettings,
) {
    let depth = fs.depth_ms.clamp(0.0, 10.0);
    // The display's span: ± the half sweep through zero, 0 … the longest delay otherwise.
    let (lo, hi) = if fs.through_zero {
        let h = (depth * 0.5).clamp(0.5, TZ_CENTRE_MS);
        (-h, h)
    } else {
        (0.0, (fs.delay_ms + depth).max(0.5))
    };
    if st.ends.0 != hi || st.ends.1 != fs.through_zero || st.ends.2.is_empty() {
        st.ends.0 = hi;
        st.ends.1 = fs.through_zero;
        st.ends.2.clear();
        st.ends.3.clear();
        let _ = write!(st.ends.2, "{hi:+.1} ms");
        let _ = write!(st.ends.3, "{lo:+.1} ms");
        if !fs.through_zero {
            st.ends.2.clear();
            st.ends.3.clear();
            let _ = write!(st.ends.2, "{hi:.1} ms");
            st.ends.3.push_str("0 ms");
        }
    }
    let now = tel[T_SWEEP].clamp(0.0, 1.0);
    let norm = move |d: f32| 2.0 * (d - lo) / (hi - lo) - 1.0;
    let title = if fs.through_zero {
        "flexus.card.sweep_tz"
    } else {
        "flexus.card.sweep_view"
    };
    controls::card(ui, s, rect, Src::Key(title), None, |ui, cr| {
        let r = cr.body;
        st.display = Some(r);
        let curve = |c: f64| {
            let (a, b) = delay_at(fs, c, now);
            (norm(a), norm(b))
        };
        modfx::movement(ui, s, r, host, tel, &curve, &st.ends.2, &st.ends.3);
        if fs.through_zero {
            // The dry path's place: the line the wet crosses.
            let ink = Ink::of(s);
            let k = s.k;
            let plot = r.shrink2(vec2(10.0 * k, 14.0 * k));
            let y = plot.center().y - norm(0.0) * plot.height() * 0.5;
            with_brush(&ui.painter_at(r), |b| {
                canvas::dashed(
                    b,
                    pos2(plot.left(), y),
                    pos2(plot.right(), y),
                    1.2 * k,
                    ink.text,
                    5.0 * k,
                    4.0 * k,
                );
                let t = Text::key("flexus.dry", Role::Micro, ink.text).caps(true);
                canvas::put(
                    b,
                    pos2(plot.right() - 4.0 * k, y - 3.0 * k),
                    Align2::RIGHT_BOTTOM,
                    &t,
                    k,
                    Some(ink.plate),
                );
            });
        }
        if fs.mix == 0.0 {
            modfx::idle_note(ui, s, r, "flexus.idle");
        }
    });
}

/// A complex number, for the comb's arithmetic.
#[derive(Copy, Clone, Debug)]
struct C(f64, f64);

impl C {
    fn mul(self, o: C) -> C {
        C(self.0 * o.0 - self.1 * o.1, self.0 * o.1 + self.1 * o.0)
    }
    fn add(self, o: C) -> C {
        C(self.0 + o.0, self.1 + o.1)
    }
    fn scale(self, k: f64) -> C {
        C(self.0 * k, self.1 * k)
    }
    fn div(self, o: C) -> C {
        let d = o.0 * o.0 + o.1 * o.1;
        C(
            (self.0 * o.0 + self.1 * o.1) / d,
            (self.1 * o.0 - self.0 * o.1) / d,
        )
    }
    /// `e^{−jθ}`.
    fn delay(theta: f64) -> C {
        let (s, c) = cantus_dsp::math::sin_cos(theta);
        C(c, -s)
    }
}

/// The response of a read `d` samples back at `w` radians per sample, as the line reads
/// it: whole samples exactly, fractions through the 4-point Hermite.
fn read_response(d: f64, w: f64) -> C {
    let i = d.floor();
    let t = d - i;
    if t == 0.0 {
        return C::delay(w * i);
    }
    let weights = [
        -0.5 * t * t * t + t * t - 0.5 * t,
        1.5 * t * t * t - 2.5 * t * t + 1.0,
        -1.5 * t * t * t + 2.0 * t * t + 0.5 * t,
        0.5 * t * t * t - 0.5 * t * t,
    ];
    let mut h = C(0.0, 0.0);
    for (j, wj) in weights.iter().enumerate() {
        h = h.add(C::delay(w * (i + j as f64 - 1.0)).scale(*wj));
    }
    h
}

/// The response of the dry and the wet together at the sweep's present delay (`delay_ms`,
/// the wet against the dry in through-zero), dB at `f` Hz — the unit's own arithmetic at
/// 48 kHz: the Hermite read, and the feedback one sample later round the loop,
/// `g_d·e^{−jωD} + g_w·σ·H_d/(1 − f·e^{−jω}·H_d)`.
pub fn comb_db(fs: &FlexusSettings, delay_ms: f32, f: f32) -> f32 {
    let sr = 48_000.0f64;
    let (gd, gw) = cantus_dsp::concentus::mix_gains(fs.mix);
    let w = std::f64::consts::TAU * f64::from(f) / sr;
    let centre = cantus_dsp::flexus::tz_centre(sr as f32) as f64;
    let (dry_d, wet_d) = if fs.through_zero {
        (centre, centre + f64::from(delay_ms) * 0.001 * sr)
    } else {
        (0.0, f64::from(delay_ms) * 0.001 * sr)
    };
    let h = read_response(wet_d.max(1.0), w);
    let loop_ = C(1.0, 0.0).add(C::delay(w).mul(h).scale(-f64::from(fs.feedback)));
    let sign = if fs.invert { -1.0 } else { 1.0 };
    let wet = h.div(loop_).scale(f64::from(gw) * sign);
    let y = C::delay(w * dry_d).scale(f64::from(gd)).add(wet);
    (10.0 * cantus_dsp::math::log10((y.0 * y.0 + y.1 * y.1).max(1e-12))) as f32
}

fn comb_card(ui: &mut egui::Ui, s: &Style, rect: Rect, tel: &[f32; 64], fs: &FlexusSettings) {
    let now = tel[T_SWEEP].clamp(0.0, 1.0);
    let (d, _) = delay_at(
        fs,
        f64::from(tel[cantus_dsp::modfx_clock::tel::PHASE_L]),
        now,
    );
    controls::card(ui, s, rect, Src::Key("flexus.card.comb"), None, |ui, cr| {
        let r = cr.body;
        let ink = Ink::of(s);
        let k = s.k;
        let plot = r.shrink2(vec2(8.0 * k, 16.0 * k));
        // A comb's teeth are evenly spaced in frequency: a linear axis to 4 kHz shows them
        // as they are, several points to every tooth even with feedback.
        const N: usize = 600;
        const SPAN: f32 = 4_000.0;
        let mut pts = [Pos2::ZERO; N];
        let (top, bottom) = (12.0f32, -36.0f32);
        for (i, p) in pts.iter_mut().enumerate() {
            let f = (SPAN * i as f32 / (N - 1) as f32).max(10.0);
            let db = comb_db(fs, d, f).clamp(bottom, top);
            *p = pos2(
                plot.left() + plot.width() * i as f32 / (N - 1) as f32,
                plot.top() + plot.height() * (top - db) / (top - bottom),
            );
        }
        with_brush(&ui.painter_at(r), |b| {
            canvas::ground_b(b, r, &ink, s.hue_tok);
            for db in [0.0f32, -12.0, -24.0] {
                let y = plot.top() + plot.height() * (top - db) / (top - bottom);
                b.hline(
                    plot.left(),
                    plot.right(),
                    y,
                    b.px(),
                    if db == 0.0 { ink.grid_strong } else { ink.grid },
                );
            }
            b.area(&pts, plot.bottom(), ink.fill(0.5));
            b.polyline(&pts, 1.6 * k, ink.hue);
            for (word, at) in [("1k", 1_000.0f32), ("2k", 2_000.0), ("3k", 3_000.0)] {
                let x = plot.left() + plot.width() * at / SPAN;
                b.vline(x, plot.top(), plot.bottom(), b.px(), ink.grid);
                let t = Text::new(word, Role::Micro, ink.dim);
                canvas::put(
                    b,
                    pos2(x, r.bottom() - 2.0 * k),
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
                format_args!("{d:+.2} ms"),
            );
            canvas::edge_b(b, r, &ink);
        });
    });
}

/// A mode's picture: a sweep (LFO), a swell (envelope), a hand-set line (manual).
fn mode_glyph(b: &mut crate::kit::Brush, r: Rect, i: usize, ink: egui::Color32, k: f32) {
    const N: usize = 17;
    let mut pts = [Pos2::ZERO; N];
    for (j, p) in pts.iter_mut().enumerate() {
        let x = j as f32 / (N - 1) as f32;
        let y = match i {
            0 => cantus_dsp::math::sin(std::f32::consts::TAU * x),
            1 => {
                let e = if x < 0.2 {
                    x / 0.2
                } else {
                    cantus_dsp::math::exp(-(x - 0.2) * 4.0)
                };
                2.0 * e - 1.0
            }
            _ => 0.3,
        };
        *p = pos2(
            r.left() + r.width() * x,
            r.center().y - y * r.height() * 0.4,
        );
    }
    b.polyline(&pts, 1.4 * k, ink);
}

/// The rate the inspector's line names: the LFO's in the LFO mode, none when the
/// envelope or the hand moves the sweep.
pub fn preview_rate(spec: &EffectSpec) -> modfx::PreviewRate<'_> {
    match cantus_engine::fx::flexus::settings(spec).mode {
        Mode::Lfo => modfx::PreviewRate::Spec(spec),
        _ => modfx::PreviewRate::Off,
    }
}

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let fs = cantus_engine::fx::flexus::settings(spec);
    let mode = frame::tr(MODE_KEYS[fs.mode as usize]);
    let words = modfx::PreviewWords {
        mode,
        rate: preview_rate(spec),
        amount: &modfx::Pct(fs.mix),
    };
    modfx::preview_strip(ui, spec, &words, &|c| {
        let (ml, _) = match fs.mode {
            Mode::Lfo => modfx::lfo_at(&fs.lfo, fs.spread, c),
            _ => (2.0 * fs.manual - 1.0, 0.0),
        };
        ml
    })
}

/// The settings `--editor flexus` and the screenshot tool open it with: tape flanging
/// through zero.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "flexus").set_many(
        &[
            ("tz", 1.0),
            ("invert", 1.0),
            ("mix", 0.5),
            ("fb", 0.4),
            ("rate", 0.3),
            ("depth", 8.0),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::flexus::Flexus::new(48_000.0);
    d.set(&cantus_engine::fx::flexus::settings(&spec));
    d.reset();
    modfx::demo_run(
        &mut |l, r, pos| {
            d.set_transport(&pos);
            d.process(l, r);
        },
        0.7,
    );
    let mut t = [0.0f32; 64];
    d.telemetry(&mut t);
    app.fx.injected = Some(t);
}
