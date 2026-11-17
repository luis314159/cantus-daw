//! Cribrum's editor: the auto filter (fx side round, ADR 0127).
//!
//! *Cribrum* is Latin for a sieve, and the window is one: the display at the top is **what
//! passes** — the music's spectrum as grains, lit where the filters let it through and
//! dimmed where they hold it back, under the two filters' response as it moves, each
//! filter's handle dragged across for the cutoff and up and down for the resonance, the
//! reach of its movement drawn as a track under it. Below it **the movement**: the LFO,
//! the follower and the drawn curve, each a key with its depth and a live bar, the chosen
//! one drawn in time with the song beside its controls. Under them **the filter** (A or B,
//! as chosen on the display or the keys) and **the output**.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::cribrum::filter::{effective, Mode, MODELS};
use cantus_dsp::cribrum::{t, CribrumSettings};
use egui::{pos2, vec2, Pos2, Rect, Sense};

#[path = "cribrum_curve.rs"]
mod curve;
#[path = "cribrum_display.rs"]
mod display;
#[path = "cribrum_move.rs"]
mod movement;

pub(crate) const KIND: EffectKind = EffectKind::AutoFilter;

/// Factory presets: data (`presets/effects/autofilter.toml`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The filter the Filter card edits: 0 (A) or 1 (B).
    pub filter: usize,
    /// The source the movement card shows: 0 the LFO, 1 the follower, 2 the curve.
    pub source: usize,
    /// The display's spectrum and its drag.
    view: display::View,
    /// The follower's recent movement, for its lane.
    trail: movement::Trail,
    /// The curve's point whose segment the shape keys edit.
    pub point: Option<usize>,
    /// A drag on the curve, and the number that folds its steps into one undo step.
    grab: Option<curve::Grab>,
    gesture: u64,
    /// Where the display's plot was drawn last frame (tests point at it).
    pub display: Option<Rect>,
    /// Where the movement lane's plot was drawn last frame.
    pub lane: Option<Rect>,
    /// A colourway chosen in the ⋯ menu this frame, for the host to keep.
    colourway: Option<&'static str>,
}

/// Cribrum's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.cribrum);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "cribrum"),
        &mut st,
        ui,
    );
    // The colourway chosen is the user's for every Cribrum, kept in the settings.
    if let Some(id) = st.colourway.take() {
        if let Some(name) = crate::kit::skin::colourway_unit(crate::kit::skin::Unit::Effect(KIND)) {
            app.set_unit_colourway(name, id);
        }
    }
    app.fx.cribrum = st;
    out
}

/// Cribrum's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let cs = cantus_engine::fx::cribrum::settings(host.spec());
    let mut chips = std::mem::take(&mut st.chips);
    // The latency the settings give (the telemetry's only arrives while playing).
    let latency = if cs.os > 1 {
        cantus_dsp::fervor::os::latency(cs.os, true) as f32
    } else {
        0.0
    };
    let chip_lat = chips.latency(latency, sr).to_string();
    let os = cs.os as u32;
    let chip_os = (os > 1).then(|| chips.oversampling(os).to_string());
    let chip: Vec<&str> = std::iter::once(chip_lat.as_str())
        .chain(chip_os.as_deref())
        .collect();
    // The ⋯ menu's colourways (ADR 0127 §Colourways), each row with its swatch.
    let unit = crate::kit::skin::Unit::Effect(KIND);
    let ways = crate::kit::skin::colourways(unit);
    let wearing = crate::kit::skin::colourway_of(unit).map(|w| w.id);
    let dark = Style::current().t.dark;
    let mut more: Vec<crate::kit::menu::Item<'_>> = Vec::with_capacity(ways.len() + 1);
    more.push(crate::kit::menu::Item::Band(Src::Key("cribrum.colourway")));
    for w in ways {
        more.push(crate::kit::menu::Item::Action {
            label: Src::Key(w.name_key),
            check: crate::kit::menu::Check::Swatch(
                crate::kit::skin::swatch(w.sheet, dark),
                wearing == Some(w.id),
            ),
            shortcut: None,
            submenu: false,
            enabled: true,
        });
    }
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: super::modfx::out_meter(&tel),
        more: &more,
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &cs)
    });
    st.chrome = chrome;
    st.chips = chips;
    // A colourway chosen: worn at once on this thread (an exported plug-in's window keeps it
    // for its session); Cantus's window also keeps it in the settings (`framed`).
    if let Some(w) = out
        .more
        .and_then(|i| i.checked_sub(1))
        .and_then(|i| ways.get(i))
    {
        if let Some(name) = crate::kit::skin::colourway_unit(unit) {
            crate::kit::skin::set_colourway(name, w.id);
            st.colourway = Some(w.id);
        }
    }
    out
}

/// The movement card's height at 100 %: two groups of cells, each with its title.
const MOVE_H: f32 = 262.0;

fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    cs: &CribrumSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    // The focal controls (cutoff, mix) are a size up: the lower row is an L card.
    let row3 = controls::card_height(Size::L) * k;
    let row2 = MOVE_H * k;
    let gap = s.d(4.0);
    let row1 = (area.height() - row2 - row3 - 2.0 * gap).max(200.0 * k);
    let r1 = g.full(row1);
    st.filter = st.filter.min(1);
    // The light of the deck: the theme's glows on the ground the cards stand on.
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    display::card(host, st, ui, s, r1, tel, cs);
    let r2 = g.full(row2);
    lift(ui, s, r2, false);
    movement::card(host, st, ui, s, r2, tel, cs);
    let w = [FILTER_W, OUTPUT_W];
    let sum: f32 = w.iter().sum();
    let r3: Vec<Rect> = g.row(row3, &w.map(|x| x / sum)).collect();
    // The filter being edited is the card in hand: lifted higher, its rim lit.
    lift(ui, s, r3[0], true);
    filter_card(host, st, ui, s, r3[0], cs);
    lift(ui, s, r3[1], false);
    output_card(host, ui, s, r3[1]);
    rim(ui, s, r3[0]);
}

/// A card's shadow on the deck, painted before the card: a soft drop below it — deeper
/// for the card in hand (`high`).
fn lift(ui: &egui::Ui, s: &Style, rect: Rect, high: bool) {
    let k = s.k;
    let (dy, feather, alpha) = if high {
        (4.0, 10.0, 0.55)
    } else {
        (2.0, 6.0, 0.35)
    };
    let shade = if s.t.drop.a() > 0 {
        s.t.drop
    } else {
        s.t.dark_shade
    };
    with_brush(&ui.painter_at(rect.expand(12.0 * k)), |b| {
        b.rrect_soft(
            rect.translate(vec2(0.0, dy * k)),
            s.r(6.0),
            crate::kit::Fill::Solid(crate::kit::color::with_alpha(shade, alpha)),
            feather * k,
        );
    });
}

/// The card in hand's rim: a hairline of its accent round it, after it is drawn.
fn rim(ui: &egui::Ui, s: &Style, rect: Rect) {
    with_brush(&ui.painter_at(rect.expand(2.0 * s.k)), |b| {
        b.rrect_stroke(
            rect,
            s.r(6.0),
            1.0 * s.k,
            crate::kit::color::with_alpha(s.hue, 0.55),
        );
    });
}

/// The filter card's and the output card's widths at 100 % (their shares of the row).
const FILTER_W: f32 = 640.0;
const OUTPUT_W: f32 = 440.0;
/// The filter card's column of switches.
const SWITCH_W: f32 = 236.0;

/// The keys of filter `i`'s parameters: model, mode, slope, cutoff, resonance, drive,
/// vowel.
pub const FILTER_KEYS: [[&str; 7]; 2] = [
    [
        "a.model", "a.mode", "a.slope", "a.cut", "a.res", "a.drive", "a.vowel",
    ],
    [
        "b.model", "b.mode", "b.slope", "b.cut", "b.res", "b.drive", "b.vowel",
    ],
];

/// The models' names (`cribrum.model.{i}`).
pub const MODEL_KEYS: [&str; 5] = [
    "cribrum.model.0",
    "cribrum.model.1",
    "cribrum.model.2",
    "cribrum.model.3",
    "cribrum.model.4",
];
/// The models' short names.
pub const MODEL_SHORT: [&str; 5] = [
    "cribrum.model_short.0",
    "cribrum.model_short.1",
    "cribrum.model_short.2",
    "cribrum.model_short.3",
    "cribrum.model_short.4",
];
/// A model's place in the `model` parameter's list.
pub fn model_index(m: cantus_dsp::va::filter::Model) -> usize {
    MODELS.iter().position(|x| *x == m).unwrap_or(0)
}

/// The synced lengths' names, as the table lists them (`cribrum.div.{i}`): the same in
/// every language.
pub const DIV_NAMES: [&str; 19] = [
    "8/1",
    "4/1",
    "2/1",
    "1/1",
    "1/2\u{b7}",
    "1/2",
    "1/4\u{b7}",
    "1/2T",
    "1/4",
    "1/8\u{b7}",
    "1/4T",
    "1/8",
    "1/16\u{b7}",
    "1/8T",
    "1/16",
    "1/16T",
    "1/32",
    "1/32T",
    "1/64",
];

/// A synced length's value as its name.
pub fn div_pill(v: f32, out: &mut String) {
    let i = (v.round().max(0.0) as usize).min(DIV_NAMES.len() - 1);
    out.push_str(DIV_NAMES[i]);
}

/// The responses' names (`cribrum.mode.{i}`).
pub const MODE_KEYS: [&str; 6] = [
    "cribrum.mode.0",
    "cribrum.mode.1",
    "cribrum.mode.2",
    "cribrum.mode.3",
    "cribrum.mode.4",
    "cribrum.mode.5",
];

/// The filter being edited: A or B as a segmented choice, its model, its slope, its
/// response as keys with their curves drawn on them; then cutoff, resonance, drive and
/// vowel (and, for B, how much of the movement it follows).
fn filter_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    cs: &CribrumSettings,
) {
    let f = st.filter;
    let keys = FILTER_KEYS[f];
    let p = if f == 0 { cs.a } else { cs.b };
    let b_off = f == 1 && !cs.b_on;
    let title = if f == 0 {
        "cribrum.card.filter_a"
    } else {
        "cribrum.card.filter_b"
    };
    let (_, clicked, lamp) = controls::card_lamp(
        ui,
        s,
        rect,
        Src::Key(title),
        (f == 1).then_some(cs.b_on),
        |ui, _| {
            controls::column(ui, s, SWITCH_W, |ui| {
                let labels = [Src::Key("cribrum.filter.a"), Src::Key("cribrum.filter.b")];
                let name = frame::tr("cribrum.filter.which");
                if let Some(i) = controls::segmented(ui, s, f, &labels, name, SWITCH_W) {
                    st.filter = i;
                }
                ui.add_enabled_ui(!b_off, |ui| {
                    ui.horizontal(|ui| {
                        let half = (SWITCH_W - 4.0) * 0.5;
                        let voice = effective(p.model, p.mode, p.slope24).0 == Mode::Vowel;
                        ui.add_enabled_ui(!voice, |ui| {
                            model_field(host, ui, s, keys[0], half);
                        });
                        let slopes = [
                            Src::Key("cribrum.slope_short.0"),
                            Src::Key("cribrum.slope_short.1"),
                        ];
                        let (eff, s24) = effective(p.model, p.mode, p.slope24);
                        let can24 = eff != Mode::Vowel && effective(p.model, p.mode, true).1;
                        ui.add_enabled_ui(can24, |ui| {
                            let name = frame::tr("fx.cribrum.slope");
                            let now = usize::from(s24);
                            if let Some(i) = controls::segmented_key(
                                ui, s, host, keys[2], now, &slopes, name, half,
                            ) {
                                host.set(keys[2], i as f32, None);
                            }
                        });
                    });
                    mode_keys(host, ui, s, keys[1], SWITCH_W, p.model);
                });
            });
            ui.add_enabled_ui(!b_off, |ui| {
                controls::param(ui, s, host, keys[3], Size::L);
                controls::param(ui, s, host, keys[4], Size::M);
                controls::param(ui, s, host, keys[5], Size::M);
                let vowel = effective(p.model, p.mode, p.slope24).0 == Mode::Vowel;
                ui.add_enabled_ui(vowel, |ui| {
                    controls::param_with(ui, s, host, keys[6], Size::M, Some(&vowel_pill));
                });
                if f == 1 {
                    controls::param(ui, s, host, "b.track", Size::M);
                }
            });
        },
    );
    if f == 1 && clicked && !controls::card_lamp_offer(ui, s, host, "b.on", lamp.as_ref()) {
        host.set("b.on", if cs.b_on { 0.0 } else { 1.0 }, None);
    }
}

/// A cell on parameter `key` labelled `label` (an i18n key with its `.short`): the
/// depths' cells read "Cutoff", "Reso", "Vowel" under the caption *Depth*.
pub fn param_labelled(
    ui: &mut egui::Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &'static str,
    label: &'static str,
    size: Size,
    format: Option<controls::Pill<'_>>,
) -> egui::Response {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return ui.allocate_response(egui::Vec2::ZERO, Sense::hover());
    };
    let mut c = controls::Cell::param(info, spec.get(key))
        .size(size)
        .label(Src::Key(label));
    if let Some(f) = format {
        c = c.format(f);
    }
    let out = c.show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
    out.resp
}

/// A vowel's value as its letters: "A", "A·E" between two.
pub fn vowel_pill(v: f32, out: &mut String) {
    const L: [&str; 5] = ["A", "E", "I", "O", "U"];
    let v = v.clamp(0.0, 4.0);
    let i = v.round() as usize;
    if (v - i as f32).abs() < 0.08 {
        out.push_str(L[i]);
    } else {
        let lo = v.floor() as usize;
        out.push_str(L[lo]);
        out.push('\u{b7}');
        out.push_str(L[(lo + 1).min(4)]);
    }
}

/// The model as a menu field.
fn model_field(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, key: &str, width: f32) {
    let now = (host.spec().get(key).round().max(0.0) as usize).min(MODELS.len() - 1);
    let id = ui.id().with(("cribrum-model", key));
    let value = frame::tr(MODEL_SHORT[now]);
    let name = frame::tr("fx.cribrum.model");
    if let Some(at) = controls::field_key(ui, s, host, key, id, value, name, width) {
        let items: Vec<_> = MODEL_KEYS
            .iter()
            .enumerate()
            .map(|(i, k)| frame::radio(Src::Key(k), i == now))
            .collect();
        if let Some(i) = controls::list(ui, s, id, at, width * 1.6, &items) {
            host.set(key, i as f32, None);
        }
    }
}

/// The responses as keys, each with its curve drawn on it; one the model does not give is
/// greyed (the editor promises nothing the model does not keep).
fn mode_keys(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    key: &str,
    width: f32,
    model: cantus_dsp::va::filter::Model,
) {
    let now = host.spec().get(key).round().max(0.0) as usize;
    let (rect, _) = ui.allocate_exact_size(vec2(width * s.k, s.d(controls::ROW)), Sense::hover());
    let n = Mode::ALL.len();
    let gap = s.d(2.0);
    let w = (rect.width() - gap * (n - 1) as f32) / n as f32;
    let name = frame::tr("fx.cribrum.mode");
    for (i, mode) in Mode::ALL.into_iter().enumerate() {
        let r = Rect::from_min_size(
            pos2(rect.left() + i as f32 * (w + gap), rect.top()),
            vec2(w, rect.height()),
        );
        let given = effective(model, mode, false).0 == mode;
        let resp = ui.interact(r, ui.id().with(("cribrum-mode", key, i)), Sense::click());
        let mut st = crate::kit::State::of(&resp, false).lit(i == now);
        st.disabled = !given || !ui.is_enabled();
        if ui.is_rect_visible(r) {
            with_brush(ui.painter(), |b| {
                controls::paint_key(b, r, s, st, Src::Str(""));
                let ink = if st.disabled {
                    s.t.disabled
                } else if i == now {
                    s.t.text
                } else {
                    s.hue
                };
                display::mode_glyph(b, r.shrink2(vec2(5.0 * s.k, 4.0 * s.k)), mode, ink, s.k);
            });
        }
        let learnt = controls::stepped_offer(ui, s, host, key, name, &resp);
        if resp.clicked() && i != now && !learnt && given && ui.is_enabled() {
            host.set(key, i as f32, None);
        }
        if resp.hovered() {
            crate::kit::hint::set_text(
                ui.ctx(),
                frame::tr(MODE_KEYS[i]),
                frame::tr(if given {
                    "cribrum.mode_tip"
                } else {
                    "cribrum.mode_not_given"
                }),
                crate::kit::hint::Caps::NONE,
            );
        }
    }
}

/// The output: the routing of the two filters, the oversampling, the stereo spread, the
/// mix and the level.
fn output_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("cribrum.card.output"),
        None,
        |ui, _| {
            let w = 132.0;
            controls::column(ui, s, w, |ui| {
                controls::caption(ui, s, Src::Key("fx.cribrum.route"), w);
                let routes = [Src::Key("cribrum.route.0"), Src::Key("cribrum.route.1")];
                controls::segmented_param(ui, s, host, "route", &routes, w);
                controls::caption(ui, s, Src::Key("fx.cribrum.os"), w);
                let os = [
                    Src::Key("cribrum.os.0"),
                    Src::Key("cribrum.os.1"),
                    Src::Key("cribrum.os.2"),
                ];
                controls::segmented_param(ui, s, host, "os", &os, w);
            });
            controls::param(ui, s, host, "spread", Size::M);
            controls::param(ui, s, host, "mix", Size::L);
            controls::param(ui, s, host, "out", Size::M);
        },
    );
}

/// The inspector's picture: the response as it is set, small, with its words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    display::preview(ui, spec)
}

/// The settings `--editor cribrum` and the screenshot tool open it with: a resonant ladder
/// swept on the quarters by the LFO, the curve's pluck on each bar, the follower on top,
/// and a gentle high-pass after it.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "cribrum").set_many(DEMO, "cmd.fx_preset");
}

/// The demo's settings.
pub const DEMO: &[(&str, f32)] = &[
    ("mix", 1.0),
    ("os", 1.0),
    ("a.model", 1.0),
    ("a.slope", 1.0),
    ("a.cut", 900.0),
    ("a.res", 0.62),
    ("a.drive", 4.0),
    ("b.on", 1.0),
    ("b.mode", 2.0),
    ("b.cut", 90.0),
    ("b.res", 0.2),
    ("b.track", 0.0),
    ("spread", 0.3),
    ("lfo.div", 8.0),
    ("lfo.shape", 0.0),
    ("lfo.stereo", 60.0),
    ("lfo.cut", 1.6),
    ("env.cut", 0.8),
    ("cur.cut", 1.0),
    ("cur.res", 0.15),
];

/// What a playing engine would feed the editor, for screenshots with no engine: the demo
/// chord (with a hat's noise on the eighths) through the unit, its telemetry and the
/// analyser's pairs.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let mut d = cantus_dsp::cribrum::Cribrum::new(48_000.0, cantus_engine::fx::fluctus::shape_at);
    d.set(&cantus_engine::fx::cribrum::settings(&spec));
    d.reset();
    let mut pairs: Vec<f32> = Vec::new();
    let mut noise = 0x2545_f491u32;
    let mut at = 0usize;
    let eighth = (48_000.0 * 60.0 / 128.0 / 2.0) as usize;
    let mut trail = Vec::new();
    super::modfx::demo_run(
        &mut |l, r, pos| {
            for (i, (a, b)) in l.iter_mut().zip(r.iter_mut()).enumerate() {
                // A hat on every eighth of 128 BPM: a short burst of noise.
                let n = at + i;
                let env = (-((n % eighth) as f32) / 900.0).exp();
                noise ^= noise << 13;
                noise ^= noise >> 17;
                noise ^= noise << 5;
                let w = (noise as f32 / u32::MAX as f32 - 0.5) * 0.5 * env;
                *a += w;
                *b += w;
            }
            let dry: Vec<f32> = l.to_vec();
            d.set_transport(&pos);
            d.process(l, r, None);
            for (x, y) in dry.iter().zip(l.iter()) {
                pairs.push(*x);
                pairs.push(*y);
            }
            let mut tel = [0.0f32; 64];
            d.telemetry(&mut tel);
            trail.push(tel[t::ENV]);
            at += l.len();
        },
        1.37,
    );
    let mut tel = [0.0f32; 64];
    d.telemetry(&mut tel);
    app.fx.injected = Some(tel);
    app.fx.feed(&pairs);
    if let Some((_, effect, _)) = app.fx.open {
        // The editor's state belongs to the open effect from the first frame on.
        if app.fx.cribrum.effect != Some(effect) {
            app.fx.cribrum = State {
                effect: Some(effect),
                ..State::default()
            };
        }
    }
    app.fx.cribrum.trail.seed(&trail);
}

/// A point inside `r`, for tests and pictures: `(u, v)` across and down.
pub fn at_share(r: Rect, u: f32, v: f32) -> Pos2 {
    pos2(r.left() + r.width() * u, r.top() + r.height() * v)
}
