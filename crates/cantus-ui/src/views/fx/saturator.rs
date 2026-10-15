//! Fervor's editor: saturation you can see before you hear it.
//!
//! The benchmarks (FabFilter Saturn, Soundtoys Decapitator, FL's Blood Overdrive and
//! Fruity WaveShaper) leave the most useful facts hidden: Saturn draws no transfer curve
//! and no spectrum, Decapitator shows nothing at all, Blood Overdrive is six knobs, and
//! WaveShaper draws its curve but not what it does to a sound. So here:
//!
//! - **The curve is drawn as it is heard**: the transfer curve with the drive and bias in
//!   it, the line of no change under it, a ball where the input's peak sits on the curve
//!   right now, and the signal itself as a cloud of (in, out) points. With the *Curve*
//!   model it is the editor: drag points, double-click to add or remove one, drag a
//!   segment's middle to bend it, drag an end up or down.
//! - **The harmonics are measured, not promised**: a test tone at a level you choose goes
//!   through the real DSP — every filter and the oversampling — and its spectrum shows the
//!   harmonics the model adds (even and odd told apart) and, in red, what folds back as
//!   aliasing. Choosing the oversampling is done by watching the red go away.
//! - **Every model says what it is**, in words, and the character knob is named for what
//!   it does in that model.
//! - **Level-matched by default**, so a louder sound never wins the comparison.
//!
//! Every drag and turn is one step of the project's undo (or one gesture of a plug-in
//! host's).

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::space;
use crate::kit::hint::{self, Caps};
use crate::kit::shell::Grid;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{band, with_brush, Fill, Function, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::fervor::{self, telemetry, Model, SatSettings};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};

#[path = "saturator_curve.rs"]
mod curve;
#[path = "saturator_harm.rs"]
mod harm;

pub use curve::{apply_shape, SHAPES};
pub use harm::Harmonics;

/// A test tone at `TEST_LEVELS[level]` through Fervor at these settings, measured: what the
/// harmonics card draws.
pub fn harmonics_of(s: &SatSettings, level: usize) -> Harmonics {
    harm::measure(s, level)
}

/// Factory presets: a label key and the parameters it sets (the device frame's list).
/// What a preset does not name goes back to its default.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.sat.tape_bus",
        &[("model", 0.0), ("drive", 7.0), ("char", 0.5), ("os", 2.0)],
    ),
    (
        "preset.sat.tape_drums",
        &[
            ("model", 0.0),
            ("drive", 18.0),
            ("char", 0.85),
            ("lo_cut", 35.0),
            ("os", 2.0),
        ],
    ),
    (
        "preset.sat.tube_vocal",
        &[
            ("model", 1.0),
            ("drive", 10.0),
            ("char", 0.4),
            ("bias", 0.1),
            ("tone", 1.0),
            ("os", 1.0),
        ],
    ),
    (
        "preset.sat.tube_master",
        &[
            ("model", 1.0),
            ("drive", 4.0),
            ("char", 0.3),
            ("chan", 1.0),
            ("os", 2.0),
        ],
    ),
    (
        "preset.sat.overdrive",
        &[
            ("model", 2.0),
            ("drive", 26.0),
            ("char", 0.6),
            ("tone", 2.0),
            ("hi_cut", 8_000.0),
            ("os", 2.0),
        ],
    ),
    (
        "preset.sat.bass_growl",
        &[
            ("model", 2.0),
            ("drive", 20.0),
            ("char", 1.0),
            ("mix", 0.6),
            ("os", 2.0),
        ],
    ),
    (
        "preset.sat.fuzz",
        &[
            ("model", 3.0),
            ("drive", 30.0),
            ("char", 0.5),
            ("bias", 0.3),
            ("hi_cut", 6_500.0),
            ("os", 3.0),
        ],
    ),
    (
        "preset.sat.clipper",
        &[
            ("model", 4.0),
            ("drive", 3.0),
            ("char", 0.15),
            ("os", 3.0),
            ("auto", 0.0),
        ],
    ),
    (
        "preset.sat.fold",
        &[("model", 5.0), ("drive", 14.0), ("char", 0.3), ("os", 3.0)],
    ),
    (
        "preset.sat.lofi",
        &[
            ("model", 0.0),
            ("drive", 8.0),
            ("bits", 10.0),
            ("rate", 11_025.0),
            ("lo_cut", 120.0),
            ("hi_cut", 6_000.0),
        ],
    ),
    (
        "preset.sat.side_air",
        &[
            ("model", 0.0),
            ("drive", 9.0),
            ("char", 1.0),
            ("chan", 2.0),
            ("mix", 0.5),
            ("os", 2.0),
        ],
    ),
    (
        "preset.sat.shaper_soft",
        &[
            ("model", 6.0),
            ("drive", 4.0),
            ("p0.on", 1.0),
            ("p0.x", 0.35),
            ("p0.y", 0.55),
            ("p1.on", 1.0),
            ("p1.x", 0.75),
            ("p1.y", 0.88),
            ("p1.t", 0.3),
            ("c.t", 0.4),
            ("os", 2.0),
        ],
    ),
    (
        "preset.sat.shaper_rectify",
        &[
            ("model", 6.0),
            ("c.sym", 0.0),
            ("c.lo", 1.0),
            ("p0.on", 1.0),
            ("p0.x", 0.0),
            ("p0.y", 0.0),
            ("os", 2.0),
        ],
    ),
];

/// The test levels the harmonics are measured at, dBFS peak.
pub const TEST_LEVELS: [f32; 4] = [-18.0, -12.0, -6.0, 0.0];

/// What the editor keeps between frames.
#[derive(Debug, Default)]
pub struct State {
    pub effect: Option<EffectId>,
    pub chrome: frame::Chrome,
    pub chips: frame::ChipText,
    /// The oversampling chip (a second formatter, so both chips borrow at once).
    pub chips_os: frame::ChipText,
    /// The test tone's level, an index of [`TEST_LEVELS`].
    pub test_level: usize,
    /// The measured harmonics, kept until the settings or the test level change.
    pub harmonics: Option<Harmonics>,
    /// The curve as drawn: kept until the settings change.
    curve: Option<curve::Drawn>,
    /// A drag on the curve.
    drag: Option<curve::Grab>,
    /// Numbers that fold the steps of one drag into one undo step.
    gesture: u64,
    /// The ball: the input's peak, falling slowly.
    ball: f32,
    /// Where the curve's square was drawn last frame (−1 … 1 across, ±1.25 up and down),
    /// for tests and tools that point at it.
    pub curve_rect: Option<Rect>,
}

/// The models' names, by index.
pub const MODEL_KEYS: [&str; 7] = [
    "sat.model.0",
    "sat.model.1",
    "sat.model.2",
    "sat.model.3",
    "sat.model.4",
    "sat.model.5",
    "sat.model.6",
];

/// What each model is, in words.
const MODEL_WORDS: [&str; 7] = [
    "sat.words.0",
    "sat.words.1",
    "sat.words.2",
    "sat.words.3",
    "sat.words.4",
    "sat.words.5",
    "sat.words.6",
];

/// How each model behaves, the second line.
const MODEL_HOW: [&str; 7] = [
    "sat.how.0",
    "sat.how.1",
    "sat.how.2",
    "sat.how.3",
    "sat.how.4",
    "sat.how.5",
    "sat.how.6",
];

/// What the character does in each model, in words.
const CHAR_WORDS: [&str; 7] = [
    "sat.cw.0", "sat.cw.1", "sat.cw.2", "sat.cw.3", "sat.cw.4", "sat.cw.5", "sat.cw.6",
];

/// What the character knob does in each model.
const CHAR_KEYS: [&str; 7] = [
    "sat.char.0",
    "sat.char.1",
    "sat.char.2",
    "sat.char.3",
    "sat.char.4",
    "sat.char.5",
    "sat.char.6",
];

/// Fervor's editor in Cantus's own window: the host is the project (every drag one undo
/// step).
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.saturator);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            test_level: st.test_level,
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "saturator"),
        &mut st,
        ui,
    );
    app.fx.saturator = st;
    out
}

/// Fervor's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = if tel[telemetry::SAMPLE_RATE] > 0.0 {
        tel[telemetry::SAMPLE_RATE]
    } else {
        host.sample_rate()
    };
    let ss = cantus_engine::fx::saturator::settings(host.spec());
    // The ball falls at about 20 dB a second at 60 frames a second.
    let peak = tel[telemetry::IN_LINEAR].max(0.0);
    st.ball = if peak >= st.ball {
        peak
    } else {
        st.ball * 0.96
    };
    let mut chips = std::mem::take(&mut st.chips);
    let mut chips_os = std::mem::take(&mut st.chips_os);
    let chip = [
        chips.latency(ss.latency() as f32, sr),
        chips_os.oversampling(ss.oversampling as u32),
    ];
    let out_db = tel[telemetry::OUT_PEAK];
    let meter = (tel[telemetry::BLOCKS] > 0.0).then_some(frame::Levels {
        level: [out_db; 2],
        hold: [out_db; 2],
    });
    let look = frame::Look {
        kind: EffectKind::Saturator,
        chips: &chip,
        meter,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ss)
    });
    st.chrome = chrome;
    st.chips = chips;
    st.chips_os = chips_os;
    out
}

/// The widths of the cells' cards at 100 %, left to right: drive, tone, crush, output.
fn cell_widths() -> [f32; 4] {
    [
        controls::cells_width(&[Size::XL, Size::M, Size::M]) + 4.0 + LED_W + 16.0,
        controls::cells_width(&[Size::M; 3]) + 16.0,
        controls::cells_width(&[Size::M; 2]) + 16.0,
        controls::cells_width(&[Size::M; 2]) + 4.0 + OUT_COLUMN + 16.0,
    ]
}

/// The width of the drive card's switch column, and of the output card's.
const LED_W: f32 = 64.0;
const OUT_COLUMN: f32 = 112.0;
/// The model list's width.
const MODEL_W: f32 = 168.0;

/// Fervor inside the frame: the models, the curve and the harmonics; the cells under them.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ss: &SatSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row2 = controls::card_height(Size::XL) * k;
    let row1 = (area.height() - row2 - gap).max(200.0 * k);

    // Row 1: the model, the curve, the harmonics.
    let curve_w = (row1 - band::TITLE * k).min(area.width() * 0.42);
    let model_w = MODEL_W * k;
    let rest = area.width() - model_w - curve_w - 2.0 * gap;
    let total = area.width() - 2.0 * gap;
    let fr1 = [model_w / total, curve_w / total, rest / total];
    let r1: Vec<Rect> = g.row(row1, &fr1).collect();
    model_card(host, ui, s, r1[0], ss);
    curve::card(host, st, ui, s, r1[1], tel, ss);
    harm::card(host, st, ui, s, r1[2], ss);

    // Row 2: the cells.
    let w = cell_widths();
    let sum: f32 = w.iter().sum();
    let fr2 = w.map(|x| x / sum);
    let r2: Vec<Rect> = g.row(row2, &fr2).collect();
    drive_card(host, ui, s, r2[0], ss);
    tone_card(host, ui, &s.function(Function::Tone), r2[1]);
    crush_card(host, ui, &s.function(Function::Mod), r2[2], ss, tel);
    output_card(host, ui, &s.function(Function::Output), r2[3], tel);
}

/// The models as a column of keys, each with a small picture of its curve, and what the
/// chosen one is, in words, under them.
fn model_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, ss: &SatSettings) {
    let now = ss.model.index();
    controls::card(ui, s, rect, Src::Key("fx.sat.model"), None, |ui, cr| {
        let inner = cr.body;
        let k = s.k;
        let row = s.d(26.0);
        let gap = s.d(3.0);
        let mut y = inner.top();
        for (i, key) in MODEL_KEYS.iter().enumerate() {
            let r = Rect::from_min_size(pos2(inner.left(), y), vec2(inner.width(), row));
            y += row + gap;
            let resp = ui.interact(r, ui.id().with(("sat-model", i)), Sense::click());
            let st = crate::kit::State::of(&resp, false).lit(i == now);
            with_brush(ui.painter(), |b| {
                controls::paint_key(b, r, s, st, Src::Str(""));
                let g = Rect::from_min_size(
                    pos2(r.left() + 6.0 * k, r.top() + 4.0 * k),
                    vec2(r.height() - 8.0 * k, r.height() - 8.0 * k),
                );
                let ink = if i == now { s.t.text } else { s.hue };
                glyph(b, g, Model::from_index(i), ink, k);
                let words = Rect::from_min_max(pos2(g.right() + 7.0 * k, r.top()), r.max);
                let t = Text::key(
                    key,
                    Role::Control,
                    if i == now { s.t.text } else { s.t.label },
                )
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, words, &t, s.tk());
            });
            if resp.clicked() && i != now {
                host.set("model", i as f32, None);
            }
            if resp.hovered() {
                hint::set_text(
                    ui.ctx(),
                    frame::tr(key),
                    frame::tr(MODEL_WORDS[i]),
                    Caps::NONE,
                );
            }
        }
        // What the chosen model is, in two short lines; then what its character does.
        y += gap;
        for key in [MODEL_WORDS[now], MODEL_HOW[now]] {
            let line = Rect::from_min_size(
                pos2(inner.left() + 2.0 * k, y),
                vec2(inner.width(), 16.0 * k),
            );
            y += 16.0 * k;
            if line.bottom() > inner.bottom() + 1.0 {
                break;
            }
            with_brush(ui.painter(), |b| {
                let t = Text::key(key, Role::Body, s.t.text_dim)
                    .caps(false)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, line, &t, s.tk());
            });
        }
        y += 8.0 * k;
        let cap = Rect::from_min_size(
            pos2(inner.left() + 2.0 * k, y),
            vec2(inner.width(), 14.0 * k),
        );
        let line = cap
            .translate(vec2(0.0, 14.0 * k))
            .with_max_y(cap.bottom() + 16.0 * k);
        if line.bottom() <= inner.bottom() + 1.0 {
            with_brush(ui.painter(), |b| {
                let c = Text::key(CHAR_KEYS[now], Role::Micro, s.hue)
                    .caps(true)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, cap, &c, s.tk());
                let t = Text::key(CHAR_WORDS[now], Role::Body, s.t.text_dim)
                    .caps(false)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, line, &t, s.tk());
            });
        }
    });
}

/// A model's curve in a small square: the line of no change faint, the curve over it.
pub fn glyph(b: &mut crate::kit::Brush, r: Rect, model: Model, ink: egui::Color32, k: f32) {
    let lut = glyph_lut();
    // Each model where its character shows: the fold folding, the clip's corner.
    let (drive, ch) = match model {
        Model::Tape => (12.0, 0.5),
        Model::Tube => (12.0, 1.0),
        Model::Drive => (14.0, 0.5),
        Model::Fuzz => (20.0, 0.9),
        Model::Clip => (9.0, 0.05),
        Model::Fold => (24.0, 0.0),
        Model::Curve => (0.0, 0.5),
    };
    let scale = if model == Model::Curve { 1.0 } else { 0.5 };
    let sh = fervor::Shaper::new(model, drive, ch, 0.0, &lut[..]);
    let n = 49;
    let mut pts = [Pos2::ZERO; 49];
    let span = 1.0f32;
    for (i, p) in pts.iter_mut().enumerate() {
        let x = -span + 2.0 * span * i as f32 / (n - 1) as f32;
        let y = (sh.apply(x * scale, &lut[..]) / 1.25).clamp(-1.0, 1.0);
        *p = pos2(
            r.center().x + x * r.width() * 0.5,
            r.center().y - y * r.height() * 0.5,
        );
    }
    b.segment(r.left_bottom(), r.right_top(), k, Ink::a(ink, 0.25), false);
    b.polyline(&pts, 1.6 * k, ink);
}

/// The drawn model's picture: an S with a kink, so it reads as "drawn".
fn glyph_lut() -> &'static [f32; fervor::LUT_N + 1] {
    use std::sync::OnceLock;
    static LUT: OnceLock<Box<[f32; fervor::LUT_N + 1]>> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut c = fervor::CurveSpec::default();
        c.points[0] = Some(fervor::Point {
            x: 0.25,
            y: 0.55,
            t: 0.0,
        });
        c.points[1] = Some(fervor::Point {
            x: 0.6,
            y: 0.6,
            t: 0.0,
        });
        c.hi = 0.95;
        let mut lut = Box::new([0.0f32; fervor::LUT_N + 1]);
        c.table(&mut lut);
        lut
    })
}

// ---- the cells ----------------------------------------------------------------------------

use std::fmt::Write as _;

fn db_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:+.1} dB");
}

fn lo_pill(v: f32, b: &mut String) {
    if v <= 20.5 {
        b.push_str(frame::tr("sat.off"));
    } else {
        controls::write_readout(v, "Hz", b);
    }
}

fn hi_pill(v: f32, b: &mut String) {
    if v >= 19_999.0 {
        b.push_str(frame::tr("sat.off"));
    } else {
        controls::write_readout(v, "Hz", b);
    }
}

fn bits_pill(v: f32, b: &mut String) {
    if v >= 23.99 {
        b.push_str(frame::tr("sat.off"));
    } else {
        let _ = write!(b, "{v:.1} bit");
    }
}

fn rate_pill(v: f32, b: &mut String) {
    if v >= 49_999.0 {
        b.push_str(frame::tr("sat.off"));
    } else {
        controls::write_readout(v, "Hz", b);
    }
}

fn pct_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{:.0}%", v * 100.0);
}

fn bias_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{:+.0}%", v * 100.0);
}

/// The drive (the device's one large knob), the model's character under its own name,
/// the bias, and ×100.
fn drive_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, ss: &SatSettings) {
    let s = &s.function(Function::Dynamics);
    controls::card(ui, s, rect, Src::Key("sat.card.drive"), None, |ui, _| {
        controls::param_with(ui, s, host, "drive", Size::XL, Some(&db_pill));
        let model = ss.model.index();
        let ch = host.spec().get("char");
        ui.add_enabled_ui(ss.model != Model::Curve, |ui| {
            if let Some(info) = EffectKind::Saturator.param("char") {
                let out = controls::Cell::param(info, ch)
                    .size(Size::M)
                    .label(Src::Key(CHAR_KEYS[model]))
                    .name(frame::tr(CHAR_KEYS[model]))
                    .format(&pct_pill)
                    .show_in(ui, s, host, "char");
                if let Some(v) = out.changed {
                    host.set("char", v, Some("char"));
                }
                if out.done {
                    host.end_gesture();
                }
            }
        });
        controls::param_with(ui, s, host, "bias", Size::M, Some(&bias_pill));
        controls::column(ui, s, LED_W, |ui| {
            controls::led_param(ui, s, host, "boost", Src::Key("fx.sat.boost"), LED_W);
            controls::caption(ui, s, Src::Key("sat.boost_note"), LED_W);
        });
    });
}

/// Before and after the curve: the low cut that tightens it, the tilt, the high cut.
fn tone_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("sat.card.tone"), None, |ui, _| {
        controls::param_with(ui, s, host, "lo_cut", Size::M, Some(&lo_pill));
        controls::param_with(ui, s, host, "tone", Size::M, Some(&db_pill));
        controls::param_with(ui, s, host, "hi_cut", Size::M, Some(&hi_pill));
    });
}

/// Bits and rate.
fn crush_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    _ss: &SatSettings,
    _tel: &[f32; 64],
) {
    controls::card(ui, s, rect, Src::Key("sat.card.crush"), None, |ui, _| {
        controls::param_with(ui, s, host, "bits", Size::M, Some(&bits_pill));
        controls::param_with(ui, s, host, "rate", Size::M, Some(&rate_pill));
    });
}

/// The blend, the output, the level match and what is coloured.
fn output_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, tel: &[f32; 64]) {
    controls::card(ui, s, rect, Src::Key("sat.card.output"), None, |ui, _| {
        controls::param_with(ui, s, host, "mix", Size::M, Some(&pct_pill));
        controls::param_with(ui, s, host, "out", Size::M, Some(&db_pill));
        controls::column(ui, s, OUT_COLUMN, |ui| {
            controls::led_param(ui, s, host, "auto", Src::Key("sat.match"), OUT_COLUMN);
            let on = host.spec().get("auto") >= 0.5;
            let m = if on { tel[telemetry::MATCH] } else { 0.0 };
            let mut text = String::new();
            let _ = write!(text, "{m:+.1} dB");
            controls::readout(ui, s, &text, true, frame::tr("sat.match_tip"), OUT_COLUMN);
            let labels = [
                Src::Key("sat.chan_short.0"),
                Src::Key("sat.chan_short.1"),
                Src::Key("sat.chan_short.2"),
            ];
            controls::segmented_param(ui, s, host, "chan", &labels, OUT_COLUMN);
        });
    });
}

/// The inspector's picture: the curve, and the settings in one line.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Saturator));
    let ink = Ink::of(&st);
    let ss = cantus_engine::fx::saturator::settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let sq = Rect::from_min_size(rect.min, vec2(h, h)).shrink(space::S1);
    let (shaper, lut) = fervor::static_curve(&ss);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        b.segment(
            sq.left_bottom(),
            sq.right_top(),
            b.px(),
            ink.grid_strong,
            false,
        );
        let mut pts = [Pos2::ZERO; 65];
        for (i, p) in pts.iter_mut().enumerate() {
            let x = -1.0 + 2.0 * i as f32 / 64.0;
            let y = (shaper.apply(x, &lut[..]) / curve::Y_SPAN).clamp(-1.0, 1.0);
            *p = pos2(
                sq.center().x + x * sq.width() * 0.5,
                sq.center().y - y * sq.height() * 0.5,
            );
        }
        b.polyline(&pts, 1.5, ink.hue);
        let words = Rect::from_min_max(pos2(sq.right() + space::S2, rect.top()), rect.max);
        canvas::line_in(
            b,
            words,
            Role::Body,
            ink.text,
            st.tk(),
            format_args!(
                "{} \u{b7} {:+.1} dB \u{b7} {}x \u{b7} {} {:.0}%",
                frame::tr(MODEL_KEYS[ss.model.index()]),
                ss.total_drive_db(),
                ss.oversampling,
                frame::tr("fx.sat.mix"),
                ss.mix * 100.0,
            ),
        );
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("sat.preview_tip"), "", Caps::NONE);
    }
    resp
}

/// The settings `--editor saturator` and the screenshot tool open it with: tape on a drum
/// bus, driven, 4× oversampled.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "saturator").set_many(
        &[
            ("model", 0.0),
            ("drive", 16.0),
            ("char", 0.6),
            ("bias", 0.15),
            ("lo_cut", 30.0),
            ("tone", 1.5),
            ("os", 2.0),
            ("mix", 0.85),
        ],
        "cmd.fx_preset",
    );
}

/// A bass line and a kick under a hat: two seconds of something to colour.
pub fn demo_signal(seconds: f32, sr: f32) -> Vec<f32> {
    let n = (seconds * sr) as usize;
    let mut seed = 5u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let beat = (sr * 60.0 / 120.0) as usize;
    (0..n)
        .map(|i| {
            let tb = (i % beat) as f32 / sr;
            let t = i as f32 / sr;
            let kick = 0.7
                * (-tb * 14.0).exp()
                * (std::f32::consts::TAU * (50.0 + 90.0 * (-tb * 30.0).exp()) * tb).sin();
            let note = [55.0f32, 55.0, 65.4, 49.0][(i / beat) % 4];
            let bass = 0.35 * (std::f32::consts::TAU * note * t).sin()
                + 0.12 * (std::f32::consts::TAU * note * 2.0 * t).sin();
            let hat = 0.05 * noise() * (-(((i + beat / 2) % (beat / 2)) as f32 / sr) * 60.0).exp();
            kick + bass + hat
        })
        .collect()
}

/// What a playing engine would feed the editor, for screenshots with no engine.
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
    let mut f = fervor::Fervor::new(sr);
    f.set(&cantus_engine::fx::saturator::settings(&spec));
    f.reset();
    let x = demo_signal(1.0, sr);
    let mut t = [0.0f32; 64];
    let mut pairs = Vec::with_capacity(x.len() * 2);
    for chunk in x.chunks(480) {
        let (mut l, mut r) = (chunk.to_vec(), chunk.to_vec());
        f.process(&mut l, &mut r);
        f.write_telemetry(&mut t);
        for (a, o) in chunk.iter().zip(&l) {
            pairs.push(*a);
            pairs.push(*o);
        }
    }
    app.fx.feed(&pairs);
    app.fx.injected = Some(t);
}
