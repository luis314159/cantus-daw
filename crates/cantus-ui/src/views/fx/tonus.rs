//! Tonus's editor: the real-time auto-tuner (round 28, ADR 0053).
//!
//! The references (Auto-Tune's graph mode, Waves Tune Real-Time, FL's Pitcher, Graillon)
//! each show one thing well. Here, in one frame:
//!
//! - **The pitch display** scrolls the last three seconds: what was sung (dim) and what
//!   comes out (the hue) over note lanes that say which notes are targets, removed or left
//!   alone — so a correction is seen, not guessed. Beside it the **detected note** in large
//!   type, its distance from the note in cents, the target and the shift applied, on a
//!   meter centred at zero.
//! - **One row on the front**: where the targets come from (a scale, the notes held on
//!   MIDI, a MIDI melody), the key and the scale, the retune speed and the amount — and a
//!   one-octave keyboard where each note is clicked on or off, or right-clicked to bypass.
//! - **The rest folds away** (*More*): humanise, flex, the singer's vibrato and a created
//!   one, the formants and the throat, transposition and the reference pitch.
//!
//! Every knob turn and click is one step of the project's undo.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::space;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::Item;
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Function, Style};
use cantus_core::{Command, EffectId, EffectKind, EffectSpec, StripId, TrackId};
use cantus_dsp::tonus::{Column, Source, TonusSettings};
use cantus_engine::fx::tonus as tel;
use egui::{pos2, vec2, Rect, Sense};
use std::collections::VecDeque;

#[path = "tonus_demo.rs"]
pub mod demo;
#[path = "tonus_graph.rs"]
mod graph;
#[path = "tonus_keys.rs"]
mod keys;

const KIND: EffectKind = EffectKind::AutoTune;

/// Columns of history the display keeps: about three seconds at 48 kHz.
pub const HISTORY: usize = 560;

/// The note names, C … B, as locale keys.
pub const KEY_NAMES: [&str; 12] = [
    "fx.tonus.key.0",
    "fx.tonus.key.1",
    "fx.tonus.key.2",
    "fx.tonus.key.3",
    "fx.tonus.key.4",
    "fx.tonus.key.5",
    "fx.tonus.key.6",
    "fx.tonus.key.7",
    "fx.tonus.key.8",
    "fx.tonus.key.9",
    "fx.tonus.key.10",
    "fx.tonus.key.11",
];

const SCALE_NAMES: [&str; 13] = [
    "fx.tonus.scale.0",
    "fx.tonus.scale.1",
    "fx.tonus.scale.2",
    "fx.tonus.scale.3",
    "fx.tonus.scale.4",
    "fx.tonus.scale.5",
    "fx.tonus.scale.6",
    "fx.tonus.scale.7",
    "fx.tonus.scale.8",
    "fx.tonus.scale.9",
    "fx.tonus.scale.10",
    "fx.tonus.scale.11",
    "fx.tonus.scale.12",
];

/// Factory presets: a label key and the parameters it sets (the rest at their defaults:
/// a chromatic corrector in C).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.tonus.natural",
        &[
            ("speed", 80.0),
            ("human", 0.5),
            ("amount", 0.85),
            ("flex", 15.0),
        ],
    ),
    ("preset.tonus.tight", &[("speed", 15.0), ("human", 0.2)]),
    ("preset.tonus.hard", &[("speed", 0.0)]),
    (
        "preset.tonus.hard_vibrato",
        &[("speed", 0.0), ("vdepth", 30.0)],
    ),
    (
        "preset.tonus.throat_up",
        &[("speed", 40.0), ("throat", 3.0)],
    ),
    (
        "preset.tonus.throat_down",
        &[("speed", 40.0), ("throat", -3.0)],
    ),
    ("preset.tonus.midi_lead", &[("src", 2.0), ("speed", 0.0)]),
    (
        "preset.tonus.major_soft",
        &[("scale", 1.0), ("speed", 50.0), ("human", 0.4)],
    ),
    (
        "preset.tonus.octave_down",
        &[("trans", -12.0), ("speed", 30.0)],
    ),
];

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// The effect this state belongs to.
    effect: Option<EffectId>,
    /// What was sung and what came out, one column per 5.3 ms, oldest first.
    pub history: VecDeque<Column>,
    last_seq: Option<u64>,
    /// The newest telemetry's state.
    pub header: tel::Header,
    /// The folded settings are shown.
    pub more: bool,
    /// The display's centre pitch, eased from frame to frame.
    centre: f32,
    /// Reused polylines.
    pts: Vec<egui::Pos2>,
    /// Where the last frame drew the display and the keys — for tests to aim at.
    pub geometry: Option<Geometry>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
}

/// The display and the keyboard as the last frame laid them out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geometry {
    /// The pitch graph.
    pub graph: Rect,
    /// The detected note's panel.
    pub readout: Rect,
    /// The keys, C … B.
    pub keys: [Rect; 12],
    /// The graph's note lanes: the lowest (a MIDI note) and how many.
    pub lanes: (i32, i32),
    /// The Speed and Amount knobs, and *Reset notes*.
    pub speed: Rect,
    pub amount: Rect,
    pub reset: Rect,
}

impl State {
    /// Take the columns the telemetry brought since the last frame.
    pub fn ingest(&mut self, t: &cantus_engine::Telemetry) {
        let h = tel::header(t);
        self.header = h;
        if h.column_len == 0 {
            return;
        }
        let cols = tel::COLUMNS as u64;
        let new = match self.last_seq {
            Some(last) => (h.seq + tel::SEQ_WRAP - last) % tel::SEQ_WRAP,
            None => h.seq,
        }
        .min(cols);
        self.last_seq = Some(h.seq);
        let seq = h.seq + tel::SEQ_WRAP;
        for index in seq - new..seq {
            if let Some(c) = tel::column(t, seq, index) {
                if self.history.len() == HISTORY {
                    self.history.pop_front();
                }
                self.history.push_back(c);
            }
        }
    }
}

/// Tonus's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.tonus);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "tonus"), &mut st, ui);
    app.fx.tonus = st;
    out
}

/// Tonus's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let t = host.telemetry();
    st.ingest(&t);
    let sr = if st.header.sample_rate > 0.0 {
        st.header.sample_rate
    } else {
        host.sample_rate()
    };
    let latency = if st.header.latency > 0 {
        st.header.latency as f32
    } else {
        (cantus_dsp::psola::LATENCY_48K as f32 * sr / 48_000.0).round()
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
        body_ui(host, st, ui, s, body)
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

/// The source column's width at 100 %.
const SOURCE_COLUMN: f32 = 236.0;
/// The keyboard's width at 100 %, and the notes card's column of keys.
const KEYBOARD: f32 = 300.0;
const NOTES_COLUMN: f32 = 112.0;
/// The voice card's formant column.
const FORMANT_COLUMN: f32 = 112.0;

/// The device inside the frame: the display, the front row, and the folded row.
fn body_ui(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, body: Rect) {
    let k = s.k;
    let ss = cantus_engine::fx::tonus::settings(host.spec());
    let pitch_s = s.function(Function::Source);
    let tone_s = s.function(Function::Tone);
    let mut g = Grid::new(body, s);
    let gap = s.d(4.0);
    let row_b = controls::card_height(Size::L) * k;
    let row_c = if st.more {
        controls::card_height(Size::M) * k
    } else {
        0.0
    };
    let area = g.rest();
    let row_a =
        (area.height() - row_b - row_c - if st.more { 2.0 } else { 1.0 } * gap).max(160.0 * k);

    // Row A: the pitch display and the detected note.
    let ra: Vec<Rect> = g.row(row_a, &[1.0]).collect();
    let disp = (ra[0].height() / k - crate::kit::band::TITLE - 1.0).max(20.0);
    let mut geo = st.geometry.unwrap_or(Geometry {
        graph: Rect::NOTHING,
        readout: Rect::NOTHING,
        keys: [Rect::NOTHING; 12],
        lanes: (0, 0),
        speed: Rect::NOTHING,
        amount: Rect::NOTHING,
        reset: Rect::NOTHING,
    });
    let _ = controls::card_with(
        ui,
        &pitch_s,
        ra[0],
        Src::Key("tonus.card.pitch"),
        None,
        Some(disp),
        |ui, cr| {
            if let Some(d) = cr.display {
                let (gr, rr, lanes) = graph::show(st, ui, &pitch_s, &ss, d);
                geo.graph = gr;
                geo.readout = rr;
                geo.lanes = lanes;
            }
        },
    );

    // Row B: the correction (source, key, scale, speed, amount) and the notes.
    let wb = widths(
        &[&[Size::L, Size::M], &[]],
        &[SOURCE_COLUMN + 4.0, KEYBOARD + NOTES_COLUMN + 4.0],
    );
    let rb: Vec<Rect> = g.row(row_b, &fracs(&wb)).collect();
    (geo.speed, geo.amount) = correction_card(host, ui, &pitch_s, rb[0], &ss);
    let _ = controls::card(
        ui,
        &pitch_s,
        rb[1],
        Src::Key("tonus.card.notes"),
        None,
        |ui, cr| {
            let size = vec2(
                (cr.body.width() - (NOTES_COLUMN + 4.0) * k).max(120.0 * k),
                cr.body.height(),
            );
            geo.keys = keys::show(host, ui, &pitch_s, size, &ss, &st.header);
            geo.reset = notes_column(host, st, ui, &pitch_s, &ss);
        },
    );
    st.geometry = Some(geo);

    // Row C, folded away until asked for.
    if st.more {
        let wc = widths(
            &[&[Size::M; 3], &[Size::M; 2], &[Size::M; 3]],
            &[0.0, 0.0, FORMANT_COLUMN + 4.0],
        );
        let rc: Vec<Rect> = g.row(row_c, &fracs(&wc)).collect();
        controls::card(
            ui,
            &tone_s,
            rc[0],
            Src::Key("tonus.card.expression"),
            None,
            |ui, _| {
                controls::param(ui, &tone_s, host, "human", Size::M);
                controls::param_with(ui, &tone_s, host, "flex", Size::M, Some(&cents_pill));
                controls::param(ui, &tone_s, host, "vib", Size::M);
            },
        );
        controls::card(
            ui,
            &tone_s,
            rc[1],
            Src::Key("tonus.card.vibrato"),
            None,
            |ui, _| {
                controls::param_with(ui, &tone_s, host, "vdepth", Size::M, Some(&cents_pill));
                controls::param_with(ui, &tone_s, host, "vrate", Size::M, Some(&rate_pill));
            },
        );
        voice_card(host, ui, &tone_s, rc[2]);
    }
}

/// The front row's first card: where the targets come from, the key and the scale (or the
/// MIDI track), the speed and the amount.
fn correction_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ss: &TonusSettings,
) -> (Rect, Rect) {
    let mut knobs = (Rect::NOTHING, Rect::NOTHING);
    controls::card(
        ui,
        s,
        rect,
        Src::Key("tonus.card.correction"),
        None,
        |ui, _| {
            let w = SOURCE_COLUMN;
            controls::column(ui, s, w, |ui| {
                let labels = [
                    Src::Key("fx.tonus.src.0"),
                    Src::Key("fx.tonus.src.1"),
                    Src::Key("fx.tonus.src.2"),
                ];
                let now = ss.source as usize;
                if let Some(i) =
                    controls::segmented(ui, s, now, &labels, frame::tr("tonus.src_tip"), w)
                {
                    host.set("src", i as f32, None);
                }
                if ss.source == Source::Scale {
                    key_and_scale(host, ui, s, ss, w);
                } else {
                    midi_source(host, ui, s, w);
                }
            });
            let speed = controls::param_with(ui, s, host, "speed", Size::L, Some(&speed_pill));
            explain(ui, &speed, "tonus.speed_tip", |b| {
                speed_pill(ss.speed_ms, b)
            });
            let amount = controls::param(ui, s, host, "amount", Size::M);
            explain(ui, &amount, "tonus.amount_tip", |b| {
                controls::write_readout(ss.amount, "%", b)
            });
            knobs = (speed.rect, amount.rect);
        },
    );
    knobs
}

/// The key and the scale, as two menu fields side by side with their captions.
fn key_and_scale(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, ss: &TonusSettings, w: f32) {
    let kw = 78.0;
    let sw = w - kw - 4.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.d(4.0);
        controls::caption(ui, s, Src::Key("fx.tonus.key"), kw);
        controls::caption(ui, s, Src::Key("fx.tonus.scale"), sw);
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = s.d(4.0);
        let key = ss.key as usize % 12;
        let id = ui.id().with("tonus-key");
        let at = controls::field(
            ui,
            s,
            id,
            frame::tr(KEY_NAMES[key]),
            frame::tr("tonus.key_tip"),
            kw,
        );
        if let Some(at) = at {
            let items: Vec<Item<'_>> = (0..12)
                .map(|i| frame::radio(Src::Key(KEY_NAMES[i]), i == key))
                .collect();
            if let Some(i) = controls::list(ui, s, id, at, kw.max(96.0), &items) {
                host.set("key", i as f32, None);
            }
        }
        let scale = (ss.scale as usize).min(12);
        let id = ui.id().with("tonus-scale");
        let at = controls::field(
            ui,
            s,
            id,
            frame::tr(SCALE_NAMES[scale]),
            frame::tr("tonus.scale_tip"),
            sw,
        );
        if let Some(at) = at {
            let items: Vec<Item<'_>> = (0..13)
                .map(|i| frame::radio(Src::Key(SCALE_NAMES[i]), i == scale))
                .collect();
            if let Some(i) = controls::list(ui, s, id, at, sw.max(170.0), &items) {
                host.set("scale", i as f32, None);
            }
        }
    });
}

/// With a MIDI source: in Cantus, the track whose notes are the targets; in another host,
/// where the notes come from.
fn midi_source(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, w: f32) {
    // The messages are sentences: a body line, not a caption's capitals.
    let Some((app, strip, effect)) = host.daw() else {
        controls::line(ui, s, frame::tr("tonus.midi_host"), s.t.text_dim, w);
        return;
    };
    let current = app
        .project()
        .effect(strip, effect)
        .and_then(|e| e.midi_from);
    let name = current
        .and_then(|id| app.project().track(id))
        .map(|t| t.name.as_str())
        .filter(|n| !n.is_empty());
    let id = ui.id().with(("tonus-midi", effect.0));
    let value = name.unwrap_or(frame::tr("tonus.midi_none"));
    if let Some(at) = controls::field(ui, s, id, value, frame::tr("tonus.midi_tip"), w) {
        let tracks: Vec<(TrackId, String)> = app
            .project()
            .tracks
            .iter()
            .map(|t| (t.id, t.name.clone()))
            .collect();
        let mut items = vec![frame::radio(Src::Key("tonus.midi_none"), current.is_none())];
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
    }
    if current.is_none() {
        controls::line(ui, s, frame::tr("tonus.midi_waiting"), s.t.text_dim, w);
    }
}

/// Beside the keyboard: reset the notes, and show or fold the rest.
fn notes_column(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    ss: &TonusSettings,
) -> Rect {
    let w = NOTES_COLUMN;
    let mut reset = Rect::NOTHING;
    controls::column(ui, s, w, |ui| {
        let custom = ss.notes.iter().any(|n| n.index() != 0);
        ui.add_enabled_ui(custom && ss.source == Source::Scale, |ui| {
            let r = controls::key(
                ui,
                s,
                Src::Key("tonus.reset"),
                frame::tr("tonus.reset_tip"),
                w,
                false,
            );
            reset = r.rect;
            if r.clicked() {
                let all: Vec<(&str, f32)> = cantus_core::fx::tonus::NOTE_KEYS
                    .iter()
                    .map(|k| (*k, 0.0))
                    .collect();
                host.set_many(&all, "cmd.tonus_reset_notes");
            }
        });
        let label = if st.more { "tonus.less" } else { "tonus.more" };
        if controls::key(
            ui,
            s,
            Src::Key(label),
            frame::tr("tonus.more_tip"),
            w,
            st.more,
        )
        .clicked()
        {
            st.more = !st.more;
        }
    });
    reset
}

/// The voice: the formants kept or following the pitch, the throat, the transposition and
/// the reference pitch.
fn voice_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("tonus.card.voice"), None, |ui, _| {
        let w = FORMANT_COLUMN;
        controls::column(ui, s, w, |ui| {
            controls::caption(ui, s, Src::Key("fx.tonus.formant"), w);
            let labels = [
                Src::Key("fx.tonus.formant.0"),
                Src::Key("fx.tonus.formant.1"),
            ];
            controls::segmented_param(ui, s, host, "formant", &labels, w);
        });
        controls::param(ui, s, host, "throat", Size::M);
        controls::param_with(ui, s, host, "trans", Size::M, Some(&trans_pill));
        controls::param_with(ui, s, host, "a4", Size::M, Some(&a4_pill));
    });
}

/// Over a knob at rest under the pointer, its explanation with its value (a drag keeps the
/// knob's own name and live value).
fn explain(ui: &egui::Ui, r: &egui::Response, tip: &'static str, value: impl FnOnce(&mut String)) {
    if r.hovered() && !r.dragged() {
        hint::set(ui.ctx(), frame::tr(tip), Caps::PARAM, false, |b| {
            value(b);
            Ok(())
        });
    }
}

/// The speed in words: `Hard` at 0 ms, else the time — the pill's, the preview's.
struct SpeedWords(f32);

impl std::fmt::Display for SpeedWords {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut b = String::new();
        speed_pill(self.0, &mut b);
        f.write_str(&b)
    }
}

/// `Hard` at 0 ms, else the time.
fn speed_pill(v: f32, b: &mut String) {
    if v <= 0.0 {
        b.push_str(frame::tr("tonus.hard"));
    } else {
        controls::write_readout(v, "ms", b);
    }
}

/// `15 ct`: a size in cents, unsigned.
fn cents_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.0} ct");
}

/// `5.5 Hz`: a vibrato's rate, to a tenth.
fn rate_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.1} Hz");
}

/// `+12 st`: whole semitones.
fn trans_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:+.0} st");
}

/// `440.0 Hz`: the reference to a tenth.
fn a4_pill(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = write!(b, "{v:.1} Hz");
}

/// The inspector's picture: the targets, the speed and the amount in one line, over a
/// strip of the twelve notes (the targets lit).
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(KIND));
    let ink = Ink::of(&st);
    let ss = cantus_engine::fx::tonus::settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        let strip = Rect::from_min_size(
            rect.min + vec2(space::S1, space::S1),
            vec2((rect.width() * 0.4).min(h * 2.4), h - 2.0 * space::S1),
        );
        let allowed = cantus_dsp::tonus::scale::resolve(ss.scale, ss.key, &ss.notes);
        let w = strip.width() / 12.0;
        for (pc, a) in allowed.iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(strip.left() + w * pc as f32, strip.top()),
                vec2(w - 1.0, strip.height()),
            );
            let midi = ss.source != Source::Scale;
            let c = match a {
                _ if midi => ink.faint,
                cantus_dsp::tonus::Allowed::Target => ink.hue,
                cantus_dsp::tonus::Allowed::Bypass => ink.dim,
                cantus_dsp::tonus::Allowed::Removed => ink.grid,
            };
            b.rect(b.snap_rect(r), crate::kit::Fill::Solid(c));
        }
        let words = Rect::from_min_max(pos2(strip.right() + space::S2, rect.top()), rect.max);
        let speed = SpeedWords(ss.speed_ms);
        let tk = st.tk();
        let amount = ss.amount * 100.0;
        match ss.source {
            Source::Scale => {
                let (key, scale) = (
                    frame::tr(KEY_NAMES[ss.key as usize % 12]),
                    frame::tr(SCALE_NAMES[(ss.scale as usize).min(12)]),
                );
                canvas::line_in(
                    b,
                    words,
                    Role::Body,
                    ink.text,
                    tk,
                    format_args!("{key} {scale} \u{b7} {speed} \u{b7} {amount:.0}%"),
                );
            }
            src => {
                let what = frame::tr(if src == Source::Midi {
                    "fx.tonus.src.1"
                } else {
                    "fx.tonus.src.2"
                });
                canvas::line_in(
                    b,
                    words,
                    Role::Body,
                    ink.text,
                    tk,
                    format_args!("{what} \u{b7} {speed} \u{b7} {amount:.0}%"),
                );
            }
        }
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("tonus.open_tip"), "", Caps::NONE);
    }
    resp
}

/// The settings `--editor tonus` and the screenshot tool open it with: A minor, a quick
/// natural speed, humanised, E left alone (bypass) and G removed.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "tonus").set_many(
        &[
            ("key", 9.0),
            ("scale", 2.0),
            ("speed", 25.0),
            ("human", 0.3),
            ("n4", 3.0),
            ("n7", 2.0),
        ],
        "cmd.fx_preset",
    );
}

/// A sung line for the demos: A3 a little sharp, C4 flat with a vibrato, a slide up to E4
/// and a held B3, with breaths between (the synthetic voice of the pitch base's tests).
pub fn demo_signal(seconds: f32, sr: f32) -> Vec<f32> {
    use cantus_dsp::pitch::testkit::{self, Vowel};
    let off = |hz: f32, c: f32| hz * cantus_dsp::math::exp2(c / 1200.0);
    let mut x = Vec::new();
    let gap = |x: &mut Vec<f32>, s: f32| x.extend(std::iter::repeat_n(0.0, (s * sr) as usize));
    x.extend(testkit::voice(sr, 0.45, Vowel::A, 0.4, |_| {
        off(220.0, 30.0)
    }));
    gap(&mut x, 0.05);
    let vib = testkit::vibrato(off(261.63, -35.0), 45.0, 5.5);
    x.extend(testkit::voice(sr, 0.55, Vowel::I, 0.4, &vib));
    gap(&mut x, 0.05);
    let slide = testkit::glide(off(293.66, 10.0), off(329.63, 20.0), 0.25);
    x.extend(testkit::voice(sr, 0.5, Vowel::A, 0.4, &slide));
    gap(&mut x, 0.05);
    let held = testkit::vibrato(off(246.94, -20.0), 25.0, 5.0);
    x.extend(testkit::voice(sr, 0.7, Vowel::U, 0.4, &held));
    x.resize((seconds * sr) as usize, 0.0);
    x
}

/// What a playing engine would feed the editor, for screenshots with no engine: the line
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
    let mut unit = cantus_engine::fx::tonus::Unit::new(sr);
    use cantus_engine::FxUnit as _;
    unit.configure(&spec);
    // With a MIDI source, an A minor chord held through the line.
    if cantus_engine::fx::tonus::settings(&spec).source != Source::Scale {
        for pitch in [57u8, 60, 64] {
            unit.note(
                0,
                cantus_engine::FxNote::On {
                    pitch,
                    velocity: 100,
                },
            );
        }
    }
    let x = demo_signal(2.35, sr);
    let more = app.fx.tonus.more;
    let st = &mut app.fx.tonus;
    *st = State {
        effect: Some(effect),
        more,
        ..State::default()
    };
    let mut t = [0.0f32; 64];
    let block = 480;
    let mut pairs = Vec::with_capacity(x.len() * 2);
    for a in x.chunks(block) {
        let (mut l, mut r) = (a.to_vec(), a.to_vec());
        unit.process(&mut l, &mut r, None);
        t = unit.telemetry();
        st.ingest(&t);
        for (i, o) in a.iter().zip(&l) {
            pairs.push(*i);
            pairs.push(*o);
        }
    }
    app.fx.feed(&pairs);
    app.fx.injected = Some(t);
}
