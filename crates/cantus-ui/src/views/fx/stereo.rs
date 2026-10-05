//! Latitudo's editor: stereo control and mid/side, where what you see is what you hear.
//!
//! The benchmarks (iZotope Ozone's Imager, Waves S1, FL Patcher's mid/side preset,
//! FabFilter's M/S modes) each hold a piece: a scope, a width per band, a shuffler, an EQ
//! on mid and side. Here they are one device, and better in the ways that matter:
//!
//! - **The scope tells the truth about mono**: a vectorscope (dots fading with age, and
//!   their polar outline), correlation with the input's beside it and the lowest of the
//!   last three seconds marked, correlation *per band* on the width map — out-of-phase
//!   bass shows where it is, not as an average.
//! - **One map for everything that depends on frequency**: the width by band (drag a
//!   band's bar, a crossover, the mono edge), and under it the mid's and the side's tone
//!   (drag a shelf's dot) — the curves are the DSP's own responses.
//! - **Mono below a frequency keeps the image above it**: the mid takes the crossover's
//!   all-pass, so the bass goes mono and nothing above it moves (the docs show why a
//!   plain side high-pass smears).
//! - **Haas, honestly**: the classic delay says where it will comb in mono; the mono-safe
//!   mode widens without touching the mono sum at all.
//!
//! Every drag and knob turn is one step of the project's undo.

use super::canvas::Ink;
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::space;
use crate::i18n::tf;
use crate::kit::band;
use crate::kit::hint::{self, Caps};
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::latitudo::view::{telemetry, unpack_point};
use cantus_dsp::latitudo::{self, HaasMode, StereoSettings};
use egui::{pos2, vec2, Rect, Sense};
use std::collections::VecDeque;

#[path = "stereo_map.rs"]
mod map;
#[path = "stereo_scope.rs"]
mod scope;

/// Vectorscope points kept: about half a second.
pub const SCOPE_POINTS: usize = 1_536;
/// Frames of correlation kept for the lowest-in-three-seconds mark.
const CORR_FRAMES: usize = 180;

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.st.master_low",
        &[("mono", 100.0), ("mono_slope", 2.0), ("w_lo", 0.85)],
    ),
    (
        "preset.st.master_air",
        &[
            ("mono", 90.0),
            ("w_hi", 1.3),
            ("x_hi", 5_000.0),
            ("s_hi", 1.5),
            ("s_hi_f", 9_000.0),
        ],
    ),
    (
        "preset.st.patcher",
        &[
            ("m_lo", 1.0),
            ("m_lo_f", 120.0),
            ("m_hi", 1.0),
            ("m_hi_f", 10_000.0),
            ("s_lo", -4.0),
            ("s_lo_f", 250.0),
            ("s_hi", 3.0),
            ("s_hi_f", 7_000.0),
        ],
    ),
    ("preset.st.mono_check", &[("listen", 1.0)]),
    ("preset.st.side_solo", &[("listen", 2.0)]),
    ("preset.st.narrow", &[("width", 0.7), ("mono", 120.0)]),
    (
        "preset.st.wide_pad",
        &[
            ("width", 1.35),
            ("mono", 160.0),
            ("w_lo", 0.6),
            ("w_hi", 1.25),
            ("s_hi", 2.0),
        ],
    ),
    (
        "preset.st.haas_safe",
        &[
            ("haas", 12.0),
            ("haas_mode", 2.0),
            ("haas_amt", 0.6),
            ("mono", 150.0),
        ],
    ),
    ("preset.st.haas_classic", &[("haas", 15.0), ("mono", 150.0)]),
    ("preset.st.swap", &[("swap", 1.0)]),
    ("preset.st.polarity", &[("inv_r", 1.0)]),
    (
        "preset.st.vocal",
        &[
            ("mid", 1.0),
            ("side", -3.0),
            ("s_lo", -6.0),
            ("s_lo_f", 300.0),
        ],
    ),
    (
        "preset.st.bass_mono",
        &[("mono", 140.0), ("mono_slope", 3.0)],
    ),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect the history belongs to.
    effect: Option<EffectId>,
    /// The vectorscope's points, oldest first (left, right).
    pub points: VecDeque<(f32, f32)>,
    last_seq: Option<f32>,
    /// The output's correlation, one per frame, newest last.
    corr: VecDeque<f32>,
    /// The scope's zoom, eased from frame to frame.
    zoom: f32,
    /// The scope's polar outline, eased.
    polar: [f32; scope::BINS],
    drag: Option<map::Grab>,
    /// Where the last frame drew the scope and the map's two panels — published so a
    /// test can aim a drag.
    pub geometry: Option<Geometry>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The Haas card's warning, per frequency and language.
    comb: Option<(u32, crate::i18n::Locale, String)>,
}

/// The displays as the last frame laid them out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geometry {
    /// The vectorscope's square.
    pub scope: Rect,
    /// The width panel of the map: 20 Hz … 20 kHz across, 0 … 200 % up.
    pub width: Rect,
    /// The tone panel: 20 Hz … 20 kHz across, −12 … +12 dB up.
    pub tone: Rect,
}

impl Geometry {
    /// Where a frequency falls across the map.
    pub fn x(&self, hz: f32) -> f32 {
        map::x_of(self.width, hz)
    }
    /// The height of a width (1 = 100 %) on the width panel.
    pub fn width_y(&self, w: f32) -> f32 {
        map::wy_of(self.width, w)
    }
    /// The height of a gain on the tone panel.
    pub fn tone_y(&self, db: f32) -> f32 {
        map::ty_of(self.tone, db)
    }
}

impl State {
    /// Take what the telemetry brought since the last frame: new points into the scope,
    /// this frame's correlation into the three-second memory.
    pub fn ingest(&mut self, t: &[f32; 64]) {
        let new = match self.last_seq {
            Some(last) => telemetry::since(t[telemetry::SEQ], last),
            None => t[telemetry::SEQ].max(0.0) as u32,
        }
        .min(latitudo::POINTS as u32) as usize;
        self.last_seq = Some(t[telemetry::SEQ]);
        for j in latitudo::POINTS - new..latitudo::POINTS {
            if self.points.len() == SCOPE_POINTS {
                self.points.pop_front();
            }
            self.points
                .push_back(unpack_point(t[telemetry::POINTS_AT + j]));
        }
        if new > 0 {
            if self.corr.len() == CORR_FRAMES {
                self.corr.pop_front();
            }
            self.corr.push_back(t[telemetry::CORR]);
        }
    }

    /// The lowest correlation of the last three seconds, if anything played.
    pub fn worst(&self) -> Option<f32> {
        self.corr.iter().copied().reduce(f32::min)
    }
}

/// Latitudo's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.stereo);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "stereo"), &mut st, ui);
    app.fx.stereo = st;
    out
}

/// Latitudo's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    st.ingest(&tel);
    let sr = if tel[telemetry::SAMPLE_RATE] > 0.0 {
        tel[telemetry::SAMPLE_RATE]
    } else {
        host.sample_rate()
    };
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(0.0, sr)];
    let any = tel[telemetry::LEVEL_DB] > -120.0;
    let meter = any.then(|| frame::Levels {
        level: [tel[telemetry::PEAK_L], tel[telemetry::PEAK_R]],
        hold: [tel[telemetry::PEAK_L], tel[telemetry::PEAK_R]],
    });
    let look = frame::Look {
        kind: EffectKind::Stereo,
        chips: &chip,
        meter,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, sr)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The cards' widths at 100 %, from what they hold.
fn widths(sizes: &[&[Size]], extra: &[f32]) -> Vec<f32> {
    sizes
        .iter()
        .zip(extra)
        .map(|(z, e)| controls::cells_width(z) + e + 16.0)
        .collect()
}

fn fracs(w: &[f32]) -> Vec<f32> {
    let sum: f32 = w.iter().sum();
    w.iter().map(|x| x / sum).collect()
}

/// The device inside the frame: the scope and the map, then two rows of cards.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    sr: f32,
) {
    let k = s.k;
    let ss = cantus_engine::fx::stereo::settings(host.spec());
    let space_s = s.function(Function::Space);
    let tone_s = s.function(Function::Tone);
    let out_s = s.function(Function::Output);
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row_b = controls::card_height(Size::L) * k;
    let row_c = controls::card_height(Size::S) * k;
    let row_a = (area.height() - row_b - row_c - 2.0 * gap).max(200.0 * k);

    // Row A: the vectorscope (square) and the map.
    let side = (row_a - band::TITLE * k).min(area.width() * 0.36) + 2.0 * k;
    let lf = (side - gap * 0.5) / (area.width() - gap);
    let ra: Vec<Rect> = g.row(row_a, &[lf, 1.0 - lf]).collect();
    let disp = |r: Rect| (r.height() / k - band::TITLE - 1.0).max(20.0);
    let mut geo = st.geometry.unwrap_or(Geometry {
        scope: Rect::NOTHING,
        width: Rect::NOTHING,
        tone: Rect::NOTHING,
    });
    let _ = controls::card_with(
        ui,
        &space_s,
        ra[0],
        Src::Key("st.card.scope"),
        None,
        Some(disp(ra[0])),
        |ui, cr| {
            if let Some(d) = cr.display {
                geo.scope = scope::show(st, ui, &space_s, tel, d);
            }
        },
    );
    let _ = controls::card_with(
        ui,
        &space_s,
        ra[1],
        Src::Key("st.card.map"),
        None,
        Some(disp(ra[1])),
        |ui, cr| {
            if let Some(d) = cr.display {
                let (w, t) = map::show(host, st, ui, &space_s, &tone_s, &ss, tel, sr, d);
                geo.width = w;
                geo.tone = t;
            }
        },
    );
    st.geometry = Some(geo);

    // Row B: the image, the width by band, the output.
    let wb = widths(
        &[
            &[Size::L, Size::M, Size::M, Size::M, Size::M],
            &[Size::M, Size::M, Size::M, Size::S, Size::S],
            &[Size::M],
        ],
        &[0.0, 0.0, OUTPUT_COLUMN + 4.0],
    );
    let rb: Vec<Rect> = g.row(row_b, &fracs(&wb)).collect();
    controls::card(
        ui,
        &space_s,
        rb[0],
        Src::Key("st.card.image"),
        None,
        |ui, _| {
            controls::param(ui, &space_s, host, "width", Size::L);
            controls::param(ui, &space_s, host, "mid", Size::M);
            controls::param(ui, &space_s, host, "side", Size::M);
            controls::param(ui, &space_s, host, "rot", Size::M);
            controls::param(ui, &space_s, host, "bal", Size::M);
        },
    );
    controls::card(
        ui,
        &space_s,
        rb[1],
        Src::Key("st.card.bands"),
        None,
        |ui, _| {
            for key in ["w_lo", "w_mid", "w_hi"] {
                controls::param(ui, &space_s, host, key, Size::M);
            }
            for key in ["x_lo", "x_hi"] {
                controls::param_with(ui, &space_s, host, key, Size::S, Some(&hz_pill));
            }
        },
    );
    output_card(host, ui, &out_s, rb[2]);

    // Row C: the mid's tone, the side's tone, the mono bass, Haas.
    let wc = widths(
        &[&[Size::S; 4], &[Size::S; 4], &[Size::S], &[Size::S; 2]],
        &[0.0, 0.0, SLOPE_COLUMN + 4.0, HAAS_COLUMN + 4.0],
    );
    let rc: Vec<Rect> = g.row(row_c, &fracs(&wc)).collect();
    for (r, title, keys) in [
        (
            rc[0],
            "st.card.mid_tone",
            ["m_lo_f", "m_lo", "m_hi_f", "m_hi"],
        ),
        (
            rc[1],
            "st.card.side_tone",
            ["s_lo_f", "s_lo", "s_hi_f", "s_hi"],
        ),
    ] {
        controls::card(ui, &tone_s, r, Src::Key(title), None, |ui, _| {
            for (i, key) in keys.into_iter().enumerate() {
                let pill: controls::Pill<'_> = if i % 2 == 0 { &hz_pill } else { &db_pill };
                controls::param_with(ui, &tone_s, host, key, Size::S, Some(pill));
            }
        });
    }
    mono_card(host, ui, &space_s, rc[2]);
    haas_card(host, st, ui, &space_s, rc[3], &ss);
}

/// The output card's column of switches, at 100 %.
const OUTPUT_COLUMN: f32 = 196.0;
/// The mono card's slope column.
const SLOPE_COLUMN: f32 = 132.0;
/// The Haas card's mode column.
const HAAS_COLUMN: f32 = 176.0;

/// The output: its gain, what is heard, and the switches on the inputs.
fn output_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("st.card.output"), None, |ui, _| {
        controls::param(ui, s, host, "out", Size::M);
        let w = OUTPUT_COLUMN;
        controls::column(ui, s, w, |ui| {
            let labels = [
                Src::Key("st.listen.0"),
                Src::Key("st.listen.1"),
                Src::Key("st.listen.2"),
            ];
            controls::segmented_param(ui, s, host, "listen", &labels, w);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = s.d(4.0);
                let third = (w - 8.0) / 3.0;
                for (key, label) in [
                    ("swap", "fx.st.swap"),
                    ("inv_l", "fx.st.inv_l"),
                    ("inv_r", "fx.st.inv_r"),
                ] {
                    controls::led_param(ui, s, host, key, Src::Key(label), third);
                }
            });
        });
    });
}

/// Mono below a frequency, and the crossover's slope.
fn mono_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("st.card.mono"), None, |ui, _| {
        controls::param_with(ui, s, host, "mono", Size::S, Some(&mono_pill));
        let w = SLOPE_COLUMN;
        controls::column(ui, s, w, |ui| {
            let labels = [
                Src::Str("6"),
                Src::Str("12"),
                Src::Str("24"),
                Src::Str("48"),
            ];
            let now = host.spec().get("mono_slope").round().clamp(0.0, 3.0) as usize;
            if let Some(i) = controls::segmented(ui, s, now, &labels, frame::tr("st.slope_tip"), w)
            {
                host.set("mono_slope", i as f32, None);
            }
            controls::caption(ui, s, Src::Key("fx.st.mono_slope"), w);
        });
    });
}

/// Haas: the delay, the mono-safe amount, the mode, and what it does to mono.
fn haas_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ss: &StereoSettings,
) {
    controls::card(ui, s, rect, Src::Key("st.card.haas"), None, |ui, _| {
        controls::param_with(ui, s, host, "haas", Size::S, Some(&haas_pill));
        ui.add_enabled_ui(ss.haas_mode == HaasMode::MonoSafe, |ui| {
            controls::param(ui, s, host, "haas_amt", Size::S);
        });
        let w = HAAS_COLUMN;
        controls::column(ui, s, w, |ui| {
            let labels = [
                Src::Key("st.haas.right"),
                Src::Key("st.haas.left"),
                Src::Key("st.haas.safe"),
            ];
            let now = ss.haas_mode.index();
            let tips = ["st.haas.right_tip", "st.haas.left_tip", "st.haas.safe_tip"];
            if let Some(i) = controls::segmented(ui, s, now, &labels, frame::tr(tips[now]), w) {
                host.set("haas_mode", i as f32, None);
            }
            let ink = Ink::of(s);
            match ss.mono_comb_hz() {
                Some(hz) => {
                    let lang = crate::i18n::locale();
                    let q = hz.round() as u32;
                    if st.comb.as_ref().is_none_or(|c| c.0 != q || c.1 != lang) {
                        st.comb = Some((q, lang, tf("st.haas.comb", &[("hz", &q)])));
                    }
                    let text = st.comb.as_ref().map_or("", |c| c.2.as_str());
                    controls::line(ui, s, text, ink.danger, w);
                }
                None if ss.haas_on() => {
                    controls::line(ui, s, frame::tr("st.haas.safe_note"), ink.ok, w)
                }
                None => controls::line(ui, s, frame::tr("st.haas.off"), ink.dim, w),
            }
        });
    });
}

/// `120 Hz`, or `off` at the bottom of the knob.
fn mono_pill(v: f32, b: &mut String) {
    if v <= latitudo::MONO_OFF_HZ + 0.01 {
        b.push_str(frame::tr("st.off"));
    } else {
        controls::write_readout(v, "Hz", b);
    }
}

/// A frequency in a small cell, its unit on the label: `110`, `7.5k`.
fn hz_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = if v >= 1_000.0 {
        write!(b, "{:.1}k", v / 1_000.0)
    } else {
        write!(b, "{v:.0}")
    };
}

/// A gain in a small cell, its unit on the label: `+1.5`.
fn db_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:+.1}");
}

/// `12.0 ms`, or `off` at 0.
fn haas_pill(v: f32, b: &mut String) {
    if v <= 0.0 {
        b.push_str(frame::tr("st.off"));
    } else {
        controls::write_readout(v, "ms", b);
    }
}

/// The inspector's picture: the width over frequency and the settings in one line.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Stereo));
    let ink = Ink::of(&st);
    let ss = cantus_engine::fx::stereo::settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let plot =
        Rect::from_min_size(rect.min, vec2((h * 2.2).min(rect.width() * 0.5), h)).shrink(space::S1);
    with_brush(&ui.painter_at(rect), |b| {
        super::canvas::ground_b(b, rect, &ink, st.hue_tok);
        map::mini(b, plot, &ink, &ss);
        let words = Rect::from_min_max(pos2(plot.right() + space::S2, rect.top()), rect.max);
        let (w, m, r) = (
            frame::tr("fx.st.width"),
            frame::tr("fx.st.mono"),
            frame::tr("fx.st.rot"),
        );
        let (pct, rot) = (ss.width * 100.0, ss.rotation_deg);
        let tk = st.tk();
        if ss.mono_on() {
            let hz = ss.mono_hz;
            super::canvas::line_in(
                b,
                words,
                Role::Body,
                ink.text,
                tk,
                format_args!("{w} {pct:.0}% \u{b7} {m} {hz:.0} Hz \u{b7} {r} {rot:+.1}\u{b0}"),
            );
        } else {
            let off = frame::tr("st.off");
            super::canvas::line_in(
                b,
                words,
                Role::Body,
                ink.text,
                tk,
                format_args!("{w} {pct:.0}% \u{b7} {m} {off} \u{b7} {r} {rot:+.1}\u{b0}"),
            );
        }
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        super::canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("st.open_tip"), "", Caps::NONE);
    }
    resp
}

/// The settings `--editor stereo` and the screenshot tool open it with: a mix bus made
/// wider on top, its bass mono, a touch of air on the side.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "stereo").set_many(
        &[
            ("width", 1.15),
            ("mono", 120.0),
            ("w_lo", 0.7),
            ("w_hi", 1.35),
            ("x_lo", 220.0),
            ("x_hi", 4_500.0),
            ("m_lo", 1.5),
            ("m_lo_f", 110.0),
            ("s_hi", 3.0),
            ("s_hi_f", 7_500.0),
            ("s_lo", -2.0),
            ("s_lo_f", 300.0),
        ],
        "cmd.fx_preset",
    );
}

/// A small stereo mix: a centred voice with vibrato, a wide pad of two detuned voices,
/// a mono bass with some of it out of phase, and hats spread wide.
pub fn demo_signal(seconds: f32, sr: f32) -> (Vec<f32>, Vec<f32>) {
    use std::f32::consts::TAU;
    let n = (seconds * sr) as usize;
    let mut seed = 23u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let (mut l, mut r) = (Vec::with_capacity(n), Vec::with_capacity(n));
    let mut hat = [0.0f32; 2];
    for i in 0..n {
        let t = i as f32 / sr;
        let vib = 1.0 + 0.004 * (TAU * 5.5 * t).sin();
        let voice = 0.22 * (TAU * 330.0 * vib * t).sin() + 0.07 * (TAU * 660.0 * vib * t).sin();
        let pad_l = 0.2 * (TAU * 220.0 * t).sin() + 0.14 * (TAU * 277.2 * t).sin();
        let pad_r = 0.2 * (TAU * 221.7 * t + 2.0).sin() + 0.14 * (TAU * 279.9 * t + 1.0).sin();
        let bass = 0.25 * (TAU * 55.0 * t).sin();
        let wide_bass = 0.08 * (TAU * 82.0 * t).sin();
        let beat = (t * 4.0).fract();
        let env = (-beat * 30.0).exp();
        hat[0] = 0.6 * hat[0] + noise() * 0.12 * env + noise() * 0.03;
        hat[1] = 0.6 * hat[1] + noise() * 0.12 * env + noise() * 0.03;
        l.push(voice + pad_l + bass + wide_bass + hat[0]);
        r.push(voice + pad_r + bass - wide_bass + hat[1]);
    }
    (l, r)
}

/// What a playing engine would feed the editor, for screenshots with no engine: the mix
/// run through the real DSP, its telemetry taken block by block.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(spec) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .map(|e| e.spec.clone())
    else {
        return;
    };
    let sr = 48_000.0;
    let mut dsp = latitudo::Latitudo::new(sr);
    dsp.set(&cantus_engine::fx::stereo::settings(&spec));
    dsp.reset();
    let (l, r) = demo_signal(1.5, sr);
    let st = &mut app.fx.stereo;
    *st = State {
        effect: Some(effect),
        ..State::default()
    };
    let mut t = [0.0f32; 64];
    let block = 480;
    let mut pairs = Vec::with_capacity(l.len() * 2);
    for (a, b) in l.chunks(block).zip(r.chunks(block)) {
        let (mut x, mut y) = (a.to_vec(), b.to_vec());
        dsp.process(&mut x, &mut y);
        dsp.write_telemetry(&mut t);
        st.ingest(&t);
        for (i, o) in a.iter().zip(&x) {
            pairs.push(*i);
            pairs.push(*o);
        }
    }
    app.fx.feed(&pairs);
    app.fx.injected = Some(t);
}
