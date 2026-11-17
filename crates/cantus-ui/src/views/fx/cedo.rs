//! Cedo's editor (ADR 0144, provisional; design sheet v1): the ducker — a ducking engine on
//! its bed over a row or two of cards. The display is the machine and its screen: the cam
//! whose profile is the drawn shape turning on its flywheel (or, in Sidechain, a solenoid
//! fed by the key), the walking beam it lifts, the bellows the beam squeezes as the track
//! is ducked, the level gauge in decibels; the cast nameplate, the readout flags and the
//! pilot lamp along the top; and a dark screen with the shape over one cycle (or the
//! envelope on each hit), what is ducked now swept in by the playhead, the last four
//! seconds of reduction with the hits, and in Lows or Highs the band split's response.
//! The cards hold every parameter: Trigger (with the key's strip or the notes' track),
//! Key filter and Envelope in Sidechain, Band, Output. Every number from the engine's
//! telemetry (`cantus_engine::fx::cedo::t`).
//!
//! The display answers the pointer (`edit`, phase B): the shape's points and bends drawn
//! (`Command::SetCedoShape`), the envelope's handles, the threshold's line, the crossover
//! and the split, the trigger's flag and the gauge (the depth) — each edit one step of undo.

use super::controls;
use super::frame;
use super::fxa_cells::{self as cells, Item};
use super::host::{DawHost, FxHost, KeySource};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use cantus_core::{Command, EffectId, EffectKind, EffectSpec, StripId, TrackId};
use egui::Rect;

#[path = "cedo_art.rs"]
mod art;
#[path = "cedo_display.rs"]
mod display;
#[path = "cedo_edit.rs"]
pub mod edit;
#[path = "cedo_engine.rs"]
mod engine;
#[path = "cedo_plate.rs"]
mod plate;
#[path = "cedo_screen.rs"]
mod screen;
#[path = "cedo_trace.rs"]
mod trace;
#[path = "cedo_view.rs"]
pub(crate) mod view;

const KIND: EffectKind = EffectKind::Ducker;

/// Factory presets: a label key and the parameters it sets (the factory presets are data,
/// `crates/cantus-ui/presets/`, ADR 0101).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[];

/// The window's designed size at 100 % (the frame's `designed_size`): the engine and its
/// screen over two rows of cards at most, every word whole in both languages.
pub const DESIGNED_SIZE: egui::Vec2 = egui::Vec2::new(1340.0, 900.0);

/// The states the design sheets and `fxb_whole` draw it in: a name and the settings over
/// the demo (the first is the starting preset as inserted). The demo draws a shape of its
/// own for the sixteenths' gate (`div` 1/16) and for MIDI (`demo_feed`), routes another
/// strip as the key in Sidechain and another track's notes in MIDI.
pub const DESIGN_STATES: &[(&str, &[(&str, f32)])] = &[
    ("start", &[]),
    (
        "sidechain",
        &[
            ("trig", 0.0),
            ("depth", 18.0),
            ("thr", -22.0),
            ("att", 4.0),
            ("hold", 40.0),
            ("rel", 160.0),
            ("kfilt", 2.0),
            ("khz", 60.0),
            ("look", 2.0),
        ],
    ),
    (
        "lows",
        &[
            ("band", 1.0),
            ("xover", 1.0),
            ("xhz", 150.0),
            ("depth", 18.0),
        ],
    ),
    (
        "gate",
        &[
            ("div", 14.0),
            ("swing", 0.3),
            ("depth", 80.0),
            ("edge", 2.0),
            ("band", 2.0),
            ("xhz", 900.0),
        ],
    ),
    (
        "midi",
        &[("trig", 2.0), ("depth", 24.0), ("vel", 1.0), ("div", 5.0)],
    ),
];

/// The band split's own unit, kept to draw its response (`Cedo::response_db`).
#[derive(Clone)]
pub struct Split {
    dsp: Box<cantus_dsp::cedo::Cedo>,
    set: Option<cantus_dsp::cedo::CedoSettings>,
}

impl std::fmt::Debug for Split {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cedo::Split")
    }
}

impl Split {
    fn new(sample_rate: f32) -> Split {
        Split {
            dsp: Box::new(cantus_dsp::cedo::Cedo::new(
                sample_rate,
                cantus_engine::fx::fluctus::shape_at,
            )),
            set: None,
        }
    }

    /// The unit at the spec's settings (set again only when they move).
    fn follow(&mut self, spec: &EffectSpec) {
        let s = cantus_engine::fx::cedo::settings(spec);
        if self.set != Some(s) {
            self.dsp.set(&s);
            self.set = Some(s);
        }
    }

    pub fn dsp(&self) -> &cantus_dsp::cedo::Cedo {
        &self.dsp
    }
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    view: view::View,
    split: Option<Split>,
    /// The key's source as the host gave it, until its revision moves.
    key: Option<(u64, Option<KeySource>)>,
    /// Where the display was drawn last frame (tests point at it).
    pub display: Option<Rect>,
    /// Where the key's or the notes' field was drawn last frame.
    pub source_field: Option<Rect>,
    /// fxb:cedo — the display's drag, its gesture's number, where its parts were drawn last
    /// frame, and the trigger's switch on its card.
    drag: Option<edit::Drag>,
    gesture: u64,
    drawn: Option<display::Drawn>,
    trigger_switch: Option<Rect>,
}

impl State {
    /// How many points the display's history holds.
    pub fn history_len(&self) -> usize {
        self.view.len()
    }

    /// The plot (the shape or the envelope) as drawn last frame.
    pub fn plot(&self) -> Option<Rect> {
        self.drawn.map(|d| d.screen.plot)
    }

    /// Where point `i` of `spec`'s shape was drawn.
    pub fn shape_point(&self, spec: &EffectSpec, i: usize) -> Option<egui::Pos2> {
        let q = *edit::shape_of(spec).points.get(i)?;
        Some(edit::point_at(self.plot()?, &q))
    }

    /// The point on the drawn curve at `u` of the cycle (the engine's own curve).
    pub fn on_curve(&self, spec: &EffectSpec, u: f32) -> Option<egui::Pos2> {
        let plot = self.plot()?;
        let a = shape_amount(spec, u);
        Some(egui::pos2(
            plot.left() + plot.width() * u,
            plot.top() + plot.height() * a,
        ))
    }

    /// The envelope's handle `n` (0 attack, 1 hold, 2 release), Sidechain.
    pub fn env_handle(&self, spec: &EffectSpec, n: usize) -> Option<egui::Pos2> {
        Some(edit::env_handles(spec, self.plot()?).get(n)?.1)
    }

    /// The threshold's line in the history, at its middle (Sidechain).
    pub fn threshold_at(&self, spec: &EffectSpec) -> Option<egui::Pos2> {
        let h = self.drawn?.screen.hist;
        Some(egui::pos2(h.center().x, edit::threshold_y(spec, h)))
    }

    /// The band strip's crossover line, at its middle (Lows, Highs).
    pub fn crossover_at(&self, spec: &EffectSpec) -> Option<egui::Pos2> {
        let r = self.drawn?.screen.band?;
        Some(egui::pos2(edit::strip_x(r, spec.get("xhz")), r.center().y))
    }

    /// The band strip (Lows, Highs).
    pub fn band_strip(&self) -> Option<Rect> {
        self.drawn?.screen.band
    }

    /// The split's word on the band strip.
    pub fn split_word(&self) -> Option<Rect> {
        self.drawn?.split_word
    }

    /// The trigger's flag on the bed.
    pub fn trigger_flag(&self) -> Option<Rect> {
        self.drawn?.trigger_flag
    }

    /// The level gauge and its bellows.
    pub fn gauge(&self) -> Option<Rect> {
        self.drawn.map(|d| d.gauge)
    }

    /// The trigger's switch on its card: the place of choice `i` (0 Sidechain, 1 Tempo,
    /// 2 MIDI).
    pub fn trigger_choice(&self, i: usize) -> Option<egui::Pos2> {
        let r = self.trigger_switch?;
        Some(egui::pos2(
            r.left() + r.width() * (i as f32 + 0.5) / 3.0,
            r.center().y,
        ))
    }
}

/// The shape's amount at `u` as the engine plays it (the classic pump when the spec has
/// none), bent by the automation editor's shapes.
pub fn shape_amount(spec: &EffectSpec, u: f32) -> f32 {
    let mut pts = [cantus_dsp::fluctus::CurvePoint::default(); cantus_core::fx::cedo::MAX_POINTS];
    let np = cantus_engine::fx::cedo::shape_points(spec, &mut pts);
    let mut curve = cantus_dsp::lapsus::Curve::flat(0.0);
    curve.set(&pts[..np], 0.0, 1.0);
    screen::amount_at(&curve, u)
}

/// Cedo's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.cedo);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "cedo"), &mut st, ui);
    app.fx.cedo = st;
    out
}

/// Cedo's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    let sr = host.sample_rate();
    let reading = view::Reading::of(&tel);
    let latency = if reading.seen {
        reading.latency
    } else {
        cantus_dsp::cedo::latency_for(&cantus_engine::fx::cedo::settings(host.spec()), sr) as f32
    };
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency, sr)];
    let look = frame::Look {
        kind: KIND,
        chips: &chip,
        meter: None,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &reading)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

const TRIGGERS: &[&str] = &["fx.cedo.trig.0", "fx.cedo.trig.1", "fx.cedo.trig.2"];
const TRIGGER: Item = Item::Switch("trig", "cedo.l.trigger", TRIGGERS);
const DIVS: &[&str] = &[
    "modfx.div.0",
    "modfx.div.1",
    "modfx.div.2",
    "modfx.div.3",
    "modfx.div.4",
    "modfx.div.5",
    "modfx.div.6",
    "modfx.div.7",
    "modfx.div.8",
    "modfx.div.9",
    "modfx.div.10",
    "modfx.div.11",
    "modfx.div.12",
    "modfx.div.13",
    "modfx.div.14",
    "modfx.div.15",
    "modfx.div.16",
];
/// The trigger's own controls beside the switch and the source, by trigger.
const SIDECHAIN: &[Item] = &[
    Item::Knob("thr", "cedo.l.threshold"),
    Item::Choice(
        "det",
        "cedo.l.detector",
        &["fx.cedo.det.0", "fx.cedo.det.1"],
    ),
    Item::Knob("rms", "cedo.l.rms"),
    Item::Knob("look", "cedo.l.lookahead"),
];
const TEMPO: &[Item] = &[
    Item::Choice("div", "cedo.l.cycle", DIVS),
    Item::Knob("swing", "cedo.l.swing"),
    Item::Knob("edge", "cedo.l.edge"),
];
const MIDI: &[Item] = &[
    Item::Choice("div", "cedo.l.length", DIVS),
    Item::Knob("edge", "cedo.l.edge"),
    Item::Choice(
        "vel",
        "cedo.l.velocity",
        &["fx.cedo.vel.0", "fx.cedo.vel.1"],
    ),
];
const KEY_FILTER: &[Item] = &[
    Item::Choice(
        "kfilt",
        "cedo.l.filter",
        &["fx.cedo.kfilt.0", "fx.cedo.kfilt.1", "fx.cedo.kfilt.2"],
    ),
    Item::Knob("khz", "cedo.l.frequency"),
    Item::Knob("kq", "cedo.l.q"),
];
const ENVELOPE: &[Item] = &[
    Item::Knob("att", "cedo.l.attack"),
    Item::Knob("hold", "cedo.l.hold"),
    Item::Knob("rel", "cedo.l.release"),
    Item::Choice(
        "rshape",
        "cedo.l.release_shape",
        &["fx.cedo.rshape.0", "fx.cedo.rshape.1", "fx.cedo.rshape.2"],
    ),
];
const BAND: &[Item] = &[
    Item::Choice(
        "band",
        "cedo.l.band",
        &["fx.cedo.band.0", "fx.cedo.band.1", "fx.cedo.band.2"],
    ),
    Item::Knob("xhz", "cedo.l.crossover"),
    Item::Choice(
        "xover",
        "cedo.l.split",
        &["fx.cedo.xover.0", "fx.cedo.xover.1"],
    ),
];
const OUTPUT: &[Item] = &[
    Item::Pill("depth", "cedo.l.depth", depth_pill),
    Item::Knob("link", "cedo.l.link"),
    Item::Knob("mix", "cedo.l.mix"),
    Item::Knob("out", "cedo.l.output"),
    Item::Choice(
        "listen",
        "cedo.l.listen",
        &["fx.cedo.listen.0", "fx.cedo.listen.1", "fx.cedo.listen.2"],
    ),
];

/// The depth in words: "9.0 dB", "∞" at its top (silence).
fn depth_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    if v >= cantus_core::fx::cedo::DEPTH_INF {
        b.push('\u{221e}');
    } else {
        let _ = write!(b, "{v:.1} dB");
    }
}

/// The source column's width at 100 %: the trigger's switch, the key's or the notes' field.
fn source_w(ui: &egui::Ui) -> f32 {
    cells::width(ui, &TRIGGER).max(186.0)
}

/// The cards in order for the trigger in force.
fn cards_of(trig: usize) -> &'static [cells::Card<'static>] {
    match trig {
        0 => &[
            ("cedo.card.trigger", SIDECHAIN),
            ("cedo.card.envelope", ENVELOPE),
            ("cedo.card.key", KEY_FILTER),
            ("cedo.card.band", BAND),
            ("cedo.card.output", OUTPUT),
        ],
        2 => &[
            ("cedo.card.trigger", MIDI),
            ("cedo.card.band", BAND),
            ("cedo.card.output", OUTPUT),
        ],
        _ => &[
            ("cedo.card.trigger", TEMPO),
            ("cedo.card.band", BAND),
            ("cedo.card.output", OUTPUT),
        ],
    }
}

/// The display over its rows of cards.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    reading: &view::Reading,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let row = controls::card_height(controls::Size::M) * k;
    let gap = s.d(4.0);
    let spec = host.spec().clone();
    let trig = screen::trigger(&spec);
    let cards = cards_of(trig);
    let mut widths = [0.0f32; 5];
    let sw = source_w(ui);
    for (w, (title, items)) in widths.iter_mut().zip(cards) {
        let extra = if *title == "cedo.card.trigger" {
            sw + 4.0
        } else {
            0.0
        };
        *w = k * (cells::card_width(ui, items) + extra);
    }
    let n = cards.len();
    let rows = cells::breaks(&widths[..n], 6.0 * k, area.width());
    let top = (area.height() - rows.len() as f32 * (row + gap)).max(440.0 * k);
    with_brush(&ui.painter_at(body), |b| {
        crate::kit::light::ambient(b, body, &s.t);
    });
    let r1 = g.full(top);
    st.display = Some(r1);
    let rev = host.revision();
    if st.key.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.key = Some((rev, host.key_source()));
    }
    let dt = ui.input(|i| i.stable_dt).min(0.1);
    st.view.update(reading, dt);
    // The shape as the engine plays it.
    let mut pts = [cantus_dsp::fluctus::CurvePoint::default(); cantus_core::fx::cedo::MAX_POINTS];
    let np = cantus_engine::fx::cedo::shape_points(&spec, &mut pts);
    let mut curve = cantus_dsp::lapsus::Curve::flat(0.0);
    curve.set(&pts[..np], 0.0, 1.0);
    let split = if screen::band(&spec) != 0 {
        let sr = host.sample_rate().max(8_000.0);
        let sp = st.split.get_or_insert_with(|| Split::new(sr));
        sp.follow(&spec);
        Some(&*sp)
    } else {
        None
    };
    let drawn = display::show(ui, s, r1, &spec, reading, &st.view, &curve, split);
    st.drawn = Some(drawn);
    edit::display(host, st, ui, s, &drawn);
    if reading.seen {
        ui.ctx().request_repaint();
    }
    for &(i, j) in &rows {
        let sum: f32 = widths[i..j].iter().sum::<f32>().max(1.0);
        let mut shares = [0.0f32; 5];
        for (sh, w) in shares.iter_mut().zip(&widths[i..j]) {
            *sh = w / sum;
        }
        let rects: Vec<Rect> = g.row(row, &shares[..j - i]).collect();
        for ((title, items), rect) in cards[i..j].iter().zip(rects) {
            if *title == "cedo.card.trigger" {
                trigger_card(host, st, ui, s, rect, trig, items);
            } else {
                cells::card(host, ui, s, rect, title, items);
            }
        }
    }
}

/// The trigger's card: the switch over the key's strip (Sidechain), the notes' track
/// (MIDI) or the song's tempo (Tempo), then the trigger's own controls.
fn trigger_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    trig: usize,
    items: &[Item],
) {
    let w = source_w(ui);
    controls::card(ui, s, rect, Src::Key("cedo.card.trigger"), None, |ui, _| {
        controls::column(ui, s, w, |ui| {
            // The switch drawn here, its place kept (the pointer tests click it).
            controls::caption(ui, s, Src::Key("cedo.l.trigger"), w);
            let labels: Vec<Src<'_>> = TRIGGERS.iter().map(|c| Src::Key(c)).collect();
            let before = ui.cursor().min;
            controls::segmented_param(ui, s, host, "trig", &labels, w);
            st.trigger_switch = Some(Rect::from_min_max(
                before,
                egui::pos2(before.x + w * s.k, ui.min_rect().bottom()),
            ));
            match trig {
                0 => {
                    controls::caption(ui, s, Src::Key("cedo.l.key"), w);
                    st.source_field = super::dynfx::key_source(
                        host,
                        &mut st.key,
                        ui,
                        s,
                        w,
                        "cedo.key_none",
                        "cedo.key_tip",
                        ["cedo.key.off", "cedo.key.sidechain"],
                    );
                }
                2 => {
                    controls::caption(ui, s, Src::Key("cedo.l.notes"), w);
                    st.source_field = notes_source(host, ui, s, w);
                }
                _ => {
                    controls::caption(ui, s, Src::Key("cedo.l.follows"), w);
                    controls::line(ui, s, frame::tr("cedo.follows_song"), s.t.text_dim, w);
                    st.source_field = None;
                }
            }
        });
        for it in items {
            match *it {
                Item::Knob(key, label) => cells::knob(host, ui, s, key, label, None, None),
                Item::Choice(key, label, names) => {
                    let pill = move |v: f32, b: &mut String| {
                        let i = (v.round().max(0.0) as usize).min(names.len() - 1);
                        b.push_str(frame::tr(names[i]));
                    };
                    cells::knob(
                        host,
                        ui,
                        s,
                        key,
                        label,
                        Some(&pill),
                        Some(cells::width(ui, it)),
                    );
                }
                Item::Pill(key, label, f) => cells::knob(host, ui, s, key, label, Some(&f), None),
                Item::Switch(..) => cells::switch(host, ui, s, it),
            }
        }
    });
}

/// In Cantus, the track whose notes fire the shape (MIDI); in another host, where the notes
/// come from. Returns the field's rect.
fn notes_source(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, w: f32) -> Option<Rect> {
    let Some((app, strip, effect)) = host.daw() else {
        controls::line(ui, s, frame::tr("cedo.notes_host"), s.t.text_dim, w);
        return None;
    };
    let current = app
        .project()
        .effect(strip, effect)
        .and_then(|e| e.midi_from);
    let name = current
        .and_then(|id| app.project().track(id))
        .map(|t| t.name.as_str())
        .filter(|n| !n.is_empty());
    let id = ui.id().with(("cedo-notes", effect.0));
    let value = name.unwrap_or(frame::tr("cedo.notes_none"));
    let r = Rect::from_min_size(ui.cursor().min, egui::vec2(w * s.k, s.d(22.0)));
    let Some(at) = controls::field(ui, s, id, value, frame::tr("cedo.notes_tip"), w) else {
        return Some(r);
    };
    let tracks: Vec<(TrackId, String)> = app
        .project()
        .tracks
        .iter()
        .map(|t| (t.id, t.name.clone()))
        .collect();
    let mut items = vec![frame::radio(Src::Key("cedo.notes_none"), current.is_none())];
    items.extend(
        tracks
            .iter()
            .map(|(tid, n)| frame::radio(Src::Str(n), current == Some(*tid))),
    );
    if let Some(i) = controls::list(ui, s, id, at, w, &items) {
        let source = if i == 0 { None } else { Some(tracks[i - 1].0) };
        if source != current {
            app.exec(Command::SetEffectMidiSource {
                strip,
                effect,
                source,
            });
        }
    }
    Some(r)
}

/// The inspector's picture: the trigger and how it ducks in a line ("Tempo · 1/4 ·
/// 9.0 dB", "Sidechain · −24 dB · 18.0 dB", "Lows" when only a band is ducked).
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(spec.kind));
    let trig = screen::trigger(spec);
    let mut depth = String::new();
    depth_pill(spec.get("depth"), &mut depth);
    let middle = if trig == 0 {
        format!("{:.0} dB", spec.get("thr"))
    } else {
        let div = spec.get("div").round().clamp(0.0, 16.0) as usize;
        crate::i18n::t(&format!("modfx.div.{div}"))
    };
    let mut w = format!(
        "{} \u{b7} {middle} \u{b7} {depth}",
        frame::tr(TRIGGERS[trig])
    );
    let band = screen::band(spec);
    if band != 0 {
        w.push_str(" \u{b7} ");
        w.push_str(frame::tr(if band == 1 {
            "fx.cedo.band.1"
        } else {
            "fx.cedo.band.2"
        }));
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

/// The starting preset (`cantus_core::starting`): the classic pump — Tempo, quarters,
/// 9 dB (the threshold back at its default after the demo's generic −30).
pub const DEMO: &[(&str, f32)] = &[("trig", 1.0), ("depth", 9.0), ("thr", -24.0)];

/// The settings `--editor cedo` and the screenshot tool open it with: the starting preset.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "cedo").set_many(DEMO, "cmd.fx_preset");
}

/// The sixteenths' gate the sheets draw: open for the first half of each sixteenth, shut
/// for the second.
fn gate_shape() -> cantus_core::fx::cedo::Shape {
    use cantus_core::fx::lapsus::{Point, SHAPE_HOLD};
    cantus_core::fx::cedo::Shape {
        points: vec![
            Point::new(0.0, 0.0).shape(SHAPE_HOLD),
            Point::new(0.5, 1.0).shape(SHAPE_HOLD),
        ],
    }
}

/// The breath the MIDI sheet draws: the full depth held a moment, let go along an S,
/// nothing after three quarters of the cycle.
fn breath_shape() -> cantus_core::fx::cedo::Shape {
    use cantus_core::fx::lapsus::{Point, SHAPE_HOLD, SHAPE_S};
    cantus_core::fx::cedo::Shape {
        points: vec![
            Point::new(0.0, 1.0).shape(SHAPE_HOLD),
            Point::new(0.14, 1.0).shape(SHAPE_S).bend(0.7),
            Point::new(0.72, 0.0).shape(SHAPE_HOLD),
        ],
    }
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// unit at the editor's settings fed eight seconds of a pad and a bass at 120 BPM — in
/// Sidechain with a kick from another strip as its key (that strip routed), in Tempo on the
/// song's position, in MIDI on another track's notes (that track chosen) — its telemetry
/// sampled as the editor reads it and the display's history filled from it. The gate's
/// state (1/16) and MIDI play drawn shapes of their own.
pub fn demo_feed(app: &mut CantusApp) {
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let trig = screen::trigger(&spec);
    let shape = match trig {
        2 => Some(breath_shape()),
        1 if spec.get("div").round() as i32 == 14 => Some(gate_shape()),
        _ => None,
    };
    if let Some(sh) = shape {
        app.exec(Command::SetCedoShape {
            strip,
            effect,
            shape: Some(Box::new(sh)),
            pedal: None,
        });
    }
    if trig == 0 {
        let other = app
            .project()
            .strips
            .iter()
            .map(|s| s.id)
            .find(|id| *id != strip && !app.project().routing_would_cycle(*id, Some(strip)));
        if let Some(source) = other {
            app.exec(Command::SetEffectSidechain {
                strip,
                effect,
                source: Some(source),
            });
        }
    }
    if trig == 2 {
        let own = app
            .project()
            .tracks
            .iter()
            .find(|t| t.strip == Some(strip))
            .map(|t| t.id);
        let other = app
            .project()
            .tracks
            .iter()
            .map(|t| t.id)
            .find(|id| Some(*id) != own);
        if let Some(source) = other {
            app.exec(Command::SetEffectMidiSource {
                strip,
                effect,
                source: Some(source),
            });
        }
    }
    let Some(spec) = super::modfx::open_spec(app) else {
        return;
    };
    let (tel, v) = demo_telemetry(&spec);
    app.fx.injected = Some(tel);
    if app.fx.cedo.effect != Some(effect) {
        app.fx.cedo = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    app.fx.cedo.view = v;
}

/// A number in −1 … 1 from a running seed: the same every run.
fn noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// The programme at `t` seconds (120 BPM): the track Cedo ducks (a pad on A minor and a
/// bass), and the kick on every beat with a little of the room (the key).
fn programme(t: f32, seed: &mut u32) -> (f32, f32, f32) {
    use cantus_dsp::math::{exp, sin};
    use std::f32::consts::TAU;
    let beat = 0.5;
    let tb = t % beat;
    let ph = TAU * (48.0 * tb + 62.0 * 0.04 * (1.0 - exp(-tb / 0.04)));
    let kick = 0.85 * exp(-tb / 0.05) * sin(ph) + 0.02 * noise(seed) * exp(-tb / 0.02);
    let mut pad = 0.0;
    for (i, f) in [220.0f32, 261.63, 329.63, 440.0].iter().enumerate() {
        let detune = 1.0 + 0.002 * i as f32;
        pad += 0.07 * sin(TAU * f * detune * t) + 0.025 * sin(TAU * 2.0 * f * t + 0.3 * i as f32);
    }
    let bass = 0.20 * sin(TAU * 55.0 * t) + 0.05 * sin(TAU * 110.0 * t);
    let swell = (t * 1.5).min(1.0);
    let air = 0.012 * noise(seed);
    (
        swell * (pad + bass) + air,
        swell * (pad * 0.96 + bass) - air,
        kick,
    )
}

/// Cedo at `spec` fed eight seconds of the programme: the last telemetry and the history
/// the display would have kept.
pub(crate) fn demo_telemetry(spec: &EffectSpec) -> ([f32; 64], view::View) {
    use cantus_engine::fx::{FxNote, FxUnit as _};
    let sr = 48_000.0f32;
    let mut u = cantus_engine::fx::cedo::Unit::new(sr);
    u.configure(spec);
    let trig = screen::trigger(spec);
    let n = (sr / view::RATE) as usize;
    let secs = 8.2;
    let blocks = (secs * sr) as usize / n;
    let mut v = view::View::default();
    let mut seed = 0x2468_ACE1u32;
    let (mut l, mut r, mut key) = (vec![0.0f32; n], vec![0.0f32; n], vec![0.0f32; n]);
    // The notes of the MIDI track: on the beats of a two-bar figure, at varied velocities.
    let note_at = |beat: i64| -> Option<u8> {
        match beat % 8 {
            0 => Some(127),
            3 => Some(84),
            4 => Some(112),
            6 => Some(64),
            _ => None,
        }
    };
    let spb = (sr * 0.5) as i64;
    for blk in 0..blocks {
        let at = (blk * n) as i64;
        u.set_position(&cantus_engine::fx::SongPosition {
            playing: true,
            tempo_bpm: 120.0,
            tempo_now: 120.0,
            samples: at,
            beats: at as f64 / spb as f64,
            time_sig_num: 4,
            time_sig_den: 4,
            ..Default::default()
        });
        if trig == 2 {
            let mut b = (at + spb - 1) / spb;
            while b * spb < at + n as i64 {
                if let Some(vel) = note_at(b) {
                    u.note(
                        (b * spb - at) as usize,
                        FxNote::On {
                            pitch: 36,
                            velocity: vel,
                        },
                    );
                }
                b += 1;
            }
        }
        for i in 0..n {
            let t = (blk * n + i) as f32 / sr;
            let (a, b, k) = programme(t, &mut seed);
            l[i] = a;
            r[i] = b;
            key[i] = k;
        }
        if trig == 0 {
            u.process(&mut l, &mut r, Some((&key, &key)));
        } else {
            u.process(&mut l, &mut r, None);
        }
        v.update(&view::Reading::of(&u.telemetry()), 1.0 / view::RATE);
    }
    (u.telemetry(), v)
}
