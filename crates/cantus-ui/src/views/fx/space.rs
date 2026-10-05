//! Spatium's editor. Owned by Spatium; nothing else edits this file.
//!
//! What the research on the benchmark reverb found missing, and what this does about it
//! (`research/fabfilter/pro-r.md`, "How we beat it"):
//!
//! - **Decay in seconds, not in percent.** The decay is the device's hero knob; the curve
//!   display's axis is the RT60 itself; the drawn curve's bands are dragged in seconds and
//!   stored as multipliers. The octave table under it prints the curve, **what the engine
//!   achieves** (its fitted filters, reported per octave), and **what was measured** on the
//!   last tail that went through it — so a request the filters cannot make is visible, not
//!   silent.
//! - **A time axis.** The tail view draws the pre-delay, the early reflections as they
//!   are, the late field's build-up and its decay per band, and the measured envelope of
//!   the last event; pre-delay, the reflections' window and level, and the decay are
//!   handles on it.
//! - **Early reflections you can shape** (level and size), **ducking with real units**
//!   (range, threshold, attack, release) keyed from any strip, and a bypass per curve.
//! - **Tempo-synced pre-delay** computed here from the host's tempo; the engine only ever
//!   sees milliseconds.
//!
//! The editor sits in the device frame (ADR 0024 §8): the macros on top as cells in
//! cards — Space and the hero Decay, the pre-delay, the character, the output — the two
//! displays in the middle, and the selected band, the reflections and the ducking under
//! them. It runs on any [`FxHost`] (ADR 0020). Every drag and knob turn is one step of the
//! host's undo.

#[path = "space_curve.rs"]
mod curve;
#[path = "space_tail.rs"]
mod tail;

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost, KeySource, KeyTarget};
use super::multiband::{choice, with_fmt};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::space;
use crate::i18n::{t, tf, Locale};
use crate::kit::band;
use crate::kit::hint::{self, Caps};
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Fill, Function, Style};
use cantus_core::fx::space::{decay_key, post_key, DECAY_FIELDS, POST_FIELDS};
use cantus_core::{Command, EffectId, EffectKind, EffectSpec, MergeKey, StripId};
use cantus_dsp::spatium::fdn::{prime_lengths, Absorber, LateDesign, LINES};
use cantus_dsp::spatium::model::{
    er_level_db, line_lengths_ms, natural_decay, room_at, room_index, sync_ms, Style as Kind,
    SYNC_BEATS,
};
use cantus_dsp::spatium::{mix_gains, SpatiumSettings, OCTAVES};
use cantus_engine::fx::space::{settings, tel};
use egui::{pos2, vec2, Align2, Rect, Sense};
use std::collections::VecDeque;
use std::fmt::Write as _;

/// Below this wet correlation the sides cancel audibly in mono: the meter turns red and
/// the width warning shows.
const MONO_TROUBLE: f32 = -0.5;

pub use tail::{Capture, Measured, WF_HOP, WF_ROWS};

/// Which curve the display's gestures and the band card act on.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Curve {
    #[default]
    Decay,
    Post,
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    pub mode: Curve,
    pub selected: Option<(Curve, usize)>,
    drag: Option<curve::Drag>,
    time_drag: Option<tail::Grab>,
    /// The tail view shows the measured spectrum over time instead of the envelope.
    pub tail_spectrum: bool,
    /// The recent audio of the effect, and the last tail measured from it.
    pub capture: Capture,
    /// What the engine would achieve, computed here when no engine reports it (the
    /// screenshot tool, a stopped engine), for the settings it was made for.
    design: Option<(SpatiumSettings, f32, Box<LateDesign>)>,
    absorber: Option<Box<Absorber>>,
    /// Ducking's gain reduction, one value a frame.
    duck: VecDeque<f32>,
    /// The effect this belongs to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// The key's source as the host said it, per host revision.
    key: Option<(u64, Option<KeySource>)>,
    /// The pairs taken from the host this frame, reused.
    pairs: Vec<(f32, f32)>,
    /// The measured envelope's points on the tail view, reused.
    tail_pts: Vec<egui::Pos2>,
    /// A band's menu on the curve display, opened with a right-click.
    menu: Option<(Curve, usize, egui::Pos2)>,
    words: Words,
}

impl State {
    /// Another effect's editor: its own capture, history and selection.
    fn start_for(&mut self, effect: EffectId) {
        if self.effect != Some(effect) {
            if self.effect.is_some() {
                self.capture = Capture::default();
                self.duck.clear();
                self.selected = None;
            }
            self.effect = Some(effect);
            self.drag = None;
            self.time_drag = None;
            self.menu = None;
            self.key = None;
        }
        self.chrome.reset_for(effect.0);
    }
}

/// Words the editor formats, kept until what they say changes.
#[derive(Debug, Default)]
struct Words {
    /// "room: 2.20 s", for (the Space value's bits, the language).
    natural: Option<(u32, Locale, String)>,
    /// "Decay band 2", for (the band, the language).
    band: Option<((Curve, usize), Locale, String)>,
    /// "Follow 120 BPM", for (the tempo, the language).
    resync: Option<(u64, Locale, String)>,
    /// "1.08 s · 1.97 s" and the sentence it stands for, for the values' bits.
    band_now: Option<([u32; 3], Locale, String, String)>,
}

const STYLE_LABELS: [Src<'static>; 4] = [
    Src::Key("space.style.0"),
    Src::Key("space.style.1"),
    Src::Key("space.style.2"),
    Src::Key("space.style.3"),
];
const STYLE_TIPS: [&str; 4] = [
    "space.style.0_tip",
    "space.style.1_tip",
    "space.style.2_tip",
    "space.style.3_tip",
];
const ROOM_KEYS: [&str; 12] = [
    "space.room.0",
    "space.room.1",
    "space.room.2",
    "space.room.3",
    "space.room.4",
    "space.room.5",
    "space.room.6",
    "space.room.7",
    "space.room.8",
    "space.room.9",
    "space.room.10",
    "space.room.11",
];
/// The rooms' names as a pill has room for them.
const ROOM_SHORT: [&str; 12] = [
    "space.room.0.short",
    "space.room.1.short",
    "space.room.2.short",
    "space.room.3.short",
    "space.room.4.short",
    "space.room.5.short",
    "space.room.6.short",
    "space.room.7.short",
    "space.room.8.short",
    "space.room.9.short",
    "space.room.10.short",
    "space.room.11.short",
];
const SYNC_KEYS: [&str; 12] = [
    "space.sync.0",
    "space.sync.1",
    "space.sync.2",
    "space.sync.3",
    "space.sync.4",
    "space.sync.5",
    "space.sync.6",
    "space.sync.7",
    "space.sync.8",
    "space.sync.9",
    "space.sync.10",
    "space.sync.11",
];
const DSHAPE_KEYS: [&str; 4] = [
    "space.dshape.0",
    "space.dshape.1",
    "space.dshape.2",
    "space.dshape.3",
];
const PSHAPE_KEYS: [&str; 5] = [
    "space.pshape.0",
    "space.pshape.1",
    "space.pshape.2",
    "space.pshape.3",
    "space.pshape.4",
];
const PLACE_KEYS: [&str; 5] = [
    "eq.place.0",
    "eq.place.1",
    "eq.place.2",
    "eq.place.3",
    "eq.place.4",
];

/// Gestures that set several keys at once: every frame of one merges into one step.
const G_SPACE: u64 = 1;
const G_PRE: u64 = 2;
pub(crate) const G_CURVE: u64 = 3;
pub(crate) const G_TAIL: u64 = 4;

/// The cards' widths at 100 %: what their cells and columns need, with the 4-pt gaps
/// between them and 8 pt either side (audit #23: a card sized without its gaps clipped).
const STYLE_W: f32 = 244.0;
const ROOM_W: f32 = 16.0 + 80.0 + 4.0 + 112.0;
const PRE_W: f32 = 16.0 + 64.0 + 4.0 + 100.0;
const CHAR_W: f32 = 16.0 + 3.0 * 64.0 + 2.0 * 4.0;
const OUT_W: f32 = 16.0 + 2.0 * 64.0 + 4.0;
const BAND_W: f32 = 16.0 + 120.0 + 4.0 + 3.0 * 64.0 + 2.0 * 4.0;
const EARLY_W: f32 = 16.0 + 2.0 * 64.0 + 4.0 + 4.0 + 84.0;
const DUCK_W: f32 = 16.0 + 130.0 + 4.0 + 4.0 * 64.0 + 3.0 * 4.0;
/// Factory presets: a label key and the parameters it sets (anything unnamed goes back to
/// its default).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "space.preset.vocal_plate",
        &[
            ("style", 1.0),
            ("space", 0.45),
            ("decay", 1.8),
            ("predelay", 30.0),
            ("bright", 0.6),
            ("mix", 0.2),
            ("e0.used", 1.0),
            ("e0.shape", 3.0),
            ("e0.freq", 180.0),
        ],
    ),
    (
        "space.preset.drum_room",
        &[
            ("space", 0.2),
            ("decay", 0.6),
            ("distance", 0.2),
            ("er", 4.0),
            ("character", 0.15),
            ("mix", 0.3),
        ],
    ),
    (
        "space.preset.concert_hall",
        &[
            ("space", 0.64),
            ("decay", 2.4),
            ("predelay", 20.0),
            ("distance", 0.5),
            ("bright", 0.4),
            ("mix", 0.25),
            ("d0.used", 1.0),
            ("d0.shape", 1.0),
            ("d0.freq", 250.0),
            ("d0.mult", 1.3),
        ],
    ),
    (
        "space.preset.cathedral",
        &[
            ("space", 0.9),
            ("decay", 7.0),
            ("predelay", 45.0),
            ("distance", 0.7),
            ("bright", 0.35),
            ("character", 0.45),
            ("mix", 0.3),
        ],
    ),
    (
        "space.preset.ducked_vocal",
        &[
            ("space", 0.55),
            ("decay", 2.2),
            ("predelay", 60.0),
            ("duck", 10.0),
            ("duck_thr", -30.0),
            ("duck_rel", 350.0),
            ("mix", 0.3),
        ],
    ),
    (
        "space.preset.eighties_snare",
        &[
            ("style", 2.0),
            ("space", 0.5),
            ("decay", 1.4),
            ("character", 0.7),
            ("bright", 0.7),
            ("mix", 0.35),
        ],
    ),
    (
        "space.preset.surf_spring",
        &[
            ("style", 3.0),
            ("space", 0.6),
            ("decay", 2.6),
            ("mix", 0.35),
            ("bright", 0.55),
        ],
    ),
    (
        "space.preset.dark_tail",
        &[
            ("space", 0.75),
            ("decay", 4.0),
            ("bright", 0.15),
            ("d0.used", 1.0),
            ("d0.shape", 2.0),
            ("d0.freq", 3_000.0),
            ("d0.mult", 0.5),
            ("mix", 0.25),
        ],
    ),
];

/// Everything a frame of the editor reads, gathered once.
pub(crate) struct View {
    pub s: SpatiumSettings,
    pub tel: cantus_engine::Telemetry,
    /// Whether the telemetry comes from a running engine (or a demo) at all.
    pub alive: bool,
    pub sample_rate: f32,
    /// The host's tempo, for the synced pre-delay.
    pub bpm: f64,
}

/// A room's name, from its index.
pub fn room_name(index: usize) -> String {
    t(ROOM_KEYS[index.min(ROOM_KEYS.len() - 1)])
}

/// `1.2 s`, `450 ms`.
pub fn seconds(v: f32) -> String {
    let mut s = String::new();
    write_seconds(v, &mut s);
    s
}

/// [`seconds`] into a buffer.
pub(crate) fn write_seconds(v: f32, out: &mut String) {
    let _ = if v < 1.0 {
        write!(out, "{:.0} ms", v * 1_000.0)
    } else if v < 10.0 {
        write!(out, "{v:.2} s")
    } else {
        write!(out, "{v:.1} s")
    };
}

/// `1.2 s`, `450 ms`, for `format_args!`.
pub(crate) struct Secs(pub f32);

impl std::fmt::Display for Secs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.0;
        if v < 1.0 {
            write!(f, "{:.0} ms", v * 1_000.0)
        } else if v < 10.0 {
            write!(f, "{v:.2} s")
        } else {
            write!(f, "{v:.1} s")
        }
    }
}

/// `1.2 kHz`, `350 Hz`, as the knobs print them.
pub(crate) struct Hz(pub f32);

impl std::fmt::Display for Hz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{:.1} kHz", self.0 / 1_000.0)
        } else {
            write!(f, "{:.0} Hz", self.0)
        }
    }
}

impl CantusApp {
    /// Set several of Spatium's parameters as one command; a gesture merges frame to frame
    /// into one undo step.
    pub fn set_space_params(
        &mut self,
        strip: StripId,
        effect: EffectId,
        fields: &[(String, f32)],
        gesture: bool,
    ) {
        let commands: Vec<Command> = fields
            .iter()
            .map(|(k, v)| Command::SetEffectParam {
                strip,
                effect,
                key: k.clone(),
                value: *v,
            })
            .collect();
        let cmd = Command::Batch {
            label_key: "cmd.space_edit".into(),
            commands,
        };
        if gesture {
            let names: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
            self.exec_merging(cmd, MergeKey::of(&("space", effect.0, names)));
        } else {
            self.exec(cmd);
        }
    }

    /// Remove a band: every field it stored goes, so its number is free and the file
    /// forgets it. One undo step.
    pub fn remove_space_band(&mut self, strip: StripId, effect: EffectId, curve: Curve, n: usize) {
        let mut st = std::mem::take(&mut self.fx.space);
        remove_band(
            &mut DawHost::new(self, strip, effect, "space"),
            &mut st,
            curve,
            n,
        );
        self.fx.space = st;
    }
}

/// A band's number as the display draws it: decay bands 1–6, post-EQ bands A–F.
pub fn band_label(curve: Curve, n: usize) -> String {
    match curve {
        Curve::Decay => (n + 1).to_string(),
        Curve::Post => ((b'A' + n as u8) as char).to_string(),
    }
}

/// [`band_label`] as a `format_args!` argument.
pub(crate) struct Label(pub Curve, pub usize);

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Curve::Decay => write!(f, "{}", self.1 + 1),
            Curve::Post => write!(f, "{}", (b'A' + self.1 as u8) as char),
        }
    }
}

/// The key of a band's field on either curve.
pub fn band_key(curve: Curve, n: usize, field: &str) -> String {
    match curve {
        Curve::Decay => decay_key(n, field),
        Curve::Post => post_key(n, field),
    }
}

/// The same key, `&'static`, from the parameters' own table: no formatting.
pub(crate) fn skey(curve: Curve, n: usize, field: &str) -> &'static str {
    let p = match curve {
        Curve::Decay => b'd',
        Curve::Post => b'e',
    };
    EffectKind::Space
        .params()
        .iter()
        .map(|i| i.key)
        .find(|k| {
            let b = k.as_bytes();
            b.len() == field.len() + 3
                && b[0] == p
                && b[1] == b'0' + n as u8
                && b[2] == b'.'
                && &k[3..] == field
        })
        .unwrap_or("")
}

/// Whether a band exists (has been made and not removed).
pub fn band_used(spec: &EffectSpec, curve: Curve, n: usize) -> bool {
    spec.get(skey(curve, n, "used")) >= 0.5
}

/// Remove a band: every field it stored goes. One step of the host's undo.
pub(crate) fn remove_band(host: &mut dyn FxHost, st: &mut State, curve: Curve, n: usize) {
    let fields = match curve {
        Curve::Decay => DECAY_FIELDS,
        Curve::Post => POST_FIELDS,
    };
    let edits: Vec<(&str, Option<f32>)> =
        fields.iter().map(|f| (skey(curve, n, f.0), None)).collect();
    host.apply(&edits, "cmd.space_band_remove", None);
    if st.selected == Some((curve, n)) {
        st.selected = None;
    }
}

/// The fitted design for the settings, kept until they change.
fn local_design(st: &mut State, s: &SpatiumSettings, sample_rate: f32) {
    if st
        .design
        .as_ref()
        .is_some_and(|(k, sr, _)| k == s && *sr == sample_rate)
    {
        return;
    }
    if st
        .absorber
        .as_ref()
        .is_none_or(|a| a.sample_rate() != sample_rate)
    {
        st.absorber = Some(Box::new(Absorber::new(sample_rate)));
    }
    let Some(absorber) = st.absorber.as_ref() else {
        return;
    };
    let room = room_at(s.space);
    let lengths = prime_lengths(&line_lengths_ms::<LINES>(&room, s.style), sample_rate);
    let d = LateDesign::new(absorber, &s.target_curve(), &lengths);
    st.design = Some((*s, sample_rate, Box::new(d)));
}

/// The decay the engine achieves at `hz`: its own report (per octave, interpolated), or
/// the same fit computed here.
pub(crate) fn achieved_t60(st: &State, view: &View, hz: f32) -> Option<f32> {
    if view.s.style == Kind::Spring {
        // The spring tank's fit includes its dispersion; the curve is what it is asked.
        return Some(view.s.target_curve().t60(hz));
    }
    if view.alive {
        let mut v = [0.0f32; OCTAVES.len()];
        for (i, x) in v.iter_mut().enumerate() {
            *x = view.tel[tel::ACHIEVED + i];
        }
        if v.iter().all(|x| *x > 0.0) {
            let pos = (hz / OCTAVES[0])
                .log2()
                .clamp(0.0, (OCTAVES.len() - 1) as f32);
            let i = (pos.floor() as usize).min(OCTAVES.len() - 2);
            let f = pos - i as f32;
            return Some((v[i].ln() * (1.0 - f) + v[i + 1].ln() * f).exp());
        }
    }
    st.design.as_ref().map(|(_, _, d)| d.achieved_t60(hz))
}

// ---- the editor ---------------------------------------------------------------------

/// Spatium's editor in Cantus's own window: the host is the project (every drag one undo
/// step, the ducking keyed from any strip of the desk).
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.space);
    st.start_for(effect);
    let out = editor(&mut DawHost::new(app, strip, effect, "space"), &mut st, ui);
    app.fx.space = st;
    out
}

/// Spatium's editor on any host (ADR 0020): the device frame and the reverb inside it.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let s = settings(host.spec());
    let tel = host.telemetry();
    let sample_rate = host.sample_rate();
    // What the unit sent since the last frame, into the capture the measured displays
    // read. The dry part is taken back out (the slot's own mix too, in Cantus), so the
    // capture is the reverb alone.
    let slot_mix = host
        .daw()
        .and_then(|(app, strip, effect)| app.project().effect(strip, effect).map(|e| e.mix))
        .unwrap_or(1.0);
    st.pairs.clear();
    host.take_pairs(&mut st.pairs);
    let (dry, wet) = mix_gains(s.mix);
    st.capture.push(&st.pairs, sample_rate, slot_mix, dry, wet);
    st.capture.analyse();
    if st.duck.len() == DUCK_HISTORY {
        st.duck.pop_front();
    }
    st.duck.push_back(tel[tel::DUCK_DB]);
    let transport = host.transport();
    let view = View {
        s,
        alive: tel[tel::ALIVE] > 0.5,
        tel,
        sample_rate,
        bpm: transport.ticks_per_second / cantus_core::PPQ as f64 * 60.0,
    };
    // The achieved decay when no engine reports it: the same fit the engine runs.
    if !view.alive {
        local_design(st, &view.s, sample_rate);
    }
    let mut chips = std::mem::take(&mut st.chips);
    let latency = chips.latency(0.0, sample_rate);
    let rho = view.tel[tel::CORRELATION];
    let trouble = view.s.width > 1.0 && rho < MONO_TROUBLE;
    let with_warning = [latency, frame::tr("space.mono_warning")];
    let look = frame::Look {
        kind: EffectKind::Space,
        chips: if trouble {
            &with_warning[..]
        } else {
            &with_warning[..1]
        },
        meter: None,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, sty, body, host| {
        body_ui(host, st, ui, sty, body, &view)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// Columns of ducking history kept: at one a frame, about four seconds.
const DUCK_HISTORY: usize = 240;

/// A row of cards whose widths at 100 % are `widths`: card `grow` takes what is left, and
/// the fractions account for the gutters, so no card is squeezed below its contents.
fn fractions<const N: usize>(
    widths: [f32; N],
    total: f32,
    gutter: f32,
    k: f32,
    grow: usize,
) -> [f32; N] {
    let room = total - gutter * (N as f32 - 1.0);
    let mut w = widths.map(|x| x * k);
    let used: f32 = w.iter().sum();
    w[grow] += room - used;
    w.map(|x| x / room.max(1.0))
}

/// The body: the macros on top, the displays in the middle, the band, the reflections and
/// the ducking under them.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    view: &View,
) {
    let k = s.k;
    let gut = s.d(4.0);
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let h1 = controls::card_height(Size::XL) * k;
    let h3 = controls::card_height(Size::M) * k;
    let h2 = (area.height() - h1 - h3 - 2.0 * gut).max(160.0 * k);
    let fr1 = fractions(
        [STYLE_W, ROOM_W, PRE_W, CHAR_W, OUT_W],
        area.width(),
        gut,
        k,
        0,
    );
    let mut row = g.row(h1, &fr1);
    let r1: [Rect; 5] = std::array::from_fn(|_| row.next().unwrap_or(Rect::NOTHING));
    style_card(host, st, ui, s, r1[0], view);
    room_card(host, ui, s, r1[1], view);
    predelay_card(host, st, ui, s, r1[2], view);
    character_card(host, ui, s, r1[3]);
    output_card(host, ui, s, r1[4], view);

    let fr2 = [0.58, 0.42];
    let mut row = g.row(h2, &fr2);
    let r2: [Rect; 2] = std::array::from_fn(|_| row.next().unwrap_or(Rect::NOTHING));
    let disp = |r: Rect| (r.height() / k - band::TITLE - 1.0).max(20.0);
    let sc = s.function(Function::Space);
    let (_, _) = controls::card_with(
        ui,
        &sc,
        r2[0],
        Src::Key("space.card.decay"),
        None,
        Some(disp(r2[0])),
        |ui, cr| {
            if let Some(d) = cr.display {
                curve::display(host, st, ui, &sc, view, d);
            }
        },
    );
    let (_, _) = controls::card_with(
        ui,
        &sc,
        r2[1],
        Src::Key("space.tail"),
        None,
        Some(disp(r2[1])),
        |ui, cr| {
            if let Some(d) = cr.display {
                tail::display(host, st, ui, &sc, view, d);
            }
        },
    );

    let fr3 = fractions([BAND_W, EARLY_W, DUCK_W], area.width(), gut, k, 2);
    let mut row = g.row(h3, &fr3);
    let r3: [Rect; 3] = std::array::from_fn(|_| row.next().unwrap_or(Rect::NOTHING));
    band_card(host, st, ui, s, r3[0], view);
    early_card(host, ui, s, r3[1], view);
    duck_card(host, st, ui, s, r3[2], view);
}

/// A cell's help line: its name and what it does, while the pointer rests on it.
fn tip(ui: &egui::Ui, resp: &egui::Response, name: &str, what: &'static str) {
    if resp.hovered() && !resp.dragged() {
        hint::set_text(ui.ctx(), name, frame::tr(what), Caps::PARAM);
    }
}

// ---- the macros -------------------------------------------------------------------------

/// The style (what kind of space), freeze, and the room's own decay for reference.
fn style_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    view: &View,
) {
    let k = s.k;
    let lang = crate::i18n::locale();
    let bits = view.s.space.to_bits();
    if st
        .words
        .natural
        .as_ref()
        .is_none_or(|w| w.0 != bits || w.1 != lang)
    {
        let text = tf(
            "space.natural",
            &[("s", &seconds(natural_decay(view.s.space)))],
        );
        st.words.natural = Some((bits, lang, text));
    }
    let natural = st.words.natural.as_ref().map_or("", |w| w.2.as_str());
    controls::card(ui, s, rect, Src::Key("fx.space.style"), None, |ui, cr| {
        let w = cr.body.width() / k;
        controls::column(ui, s, w, |ui| {
            let now = view.s.style.index();
            if let Some(i) = choice(
                ui,
                s,
                now,
                &STYLE_LABELS,
                &STYLE_TIPS,
                frame::tr("fx.space.style"),
                w,
            ) {
                host.set("style", i as f32, None);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0 * k;
                let r =
                    controls::led_param(ui, s, host, "freeze", Src::Key("fx.space.freeze"), 100.0);
                tip(ui, &r, frame::tr("fx.space.freeze"), "space.freeze_tip");
                let f = view.tel[tel::FREEZE];
                if f > 0.0 && f < 1.0 {
                    with_fmt(format_args!("{:.0}%", f * 100.0), |text| {
                        controls::readout(ui, s, text, true, frame::tr("fx.space.freeze"), 56.0)
                    });
                }
            });
            let r = controls::readout(ui, s, natural, true, frame::tr("space.natural_name"), w);
            if r.hovered() {
                hint::set_text(
                    ui.ctx(),
                    frame::tr("space.natural_name"),
                    frame::tr("space.natural_tip"),
                    Caps::NONE,
                );
            }
        });
    });
}

fn secs_pill(v: f32, b: &mut String) {
    write_seconds(v, b);
}

/// A room's name for a pill (its short form, which fits the pill): its spaces kept
/// together, so the pill does not set the second word as a unit.
fn room_pill(v: f32, b: &mut String) {
    for c in frame::tr(ROOM_SHORT[room_index(v).min(ROOM_SHORT.len() - 1)]).chars() {
        b.push(if c == ' ' { '\u{a0}' } else { c });
    }
}

/// Space and the hero, Decay. Space moves the decay with it, keeping the ratio to the
/// room's own decay — the benchmark's hero control, one gesture and one undo step.
fn room_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, view: &View) {
    controls::card(ui, s, rect, Src::Key("space.card.room"), None, |ui, _| {
        let spec = host.spec();
        if let Some(info) = spec.kind.param("space") {
            let out = controls::Cell::param(info, spec.get("space"))
                .size(Size::L)
                .format(&room_pill)
                .show(ui, s);
            if let Some(v) = out.changed {
                let ratio = view.s.decay_s / natural_decay(view.s.space);
                let decay = (natural_decay(v) * ratio).clamp(0.1, 30.0);
                host.apply(
                    &[("space", Some(v)), ("decay", Some(decay))],
                    "cmd.space_edit",
                    Some(G_SPACE),
                );
            }
            if out.done {
                host.end_gesture();
            }
            tip(
                ui,
                &out.resp,
                frame::tr("fx.space.space"),
                "space.space_tip",
            );
        }
        let r = controls::param_with(ui, s, host, "decay", Size::XL, Some(&secs_pill));
        tip(ui, &r, frame::tr("fx.space.decay"), "space.decay_tip");
    });
}

fn ms_pill(v: f32, b: &mut String) {
    controls::write_readout(v, "ms", b);
}

/// Pre-delay: a cell in milliseconds, and beside it the note value it follows, if any.
/// Choosing a note value writes its length at the host's tempo; turning the cell frees
/// it again. If the tempo has moved since, a key offers to follow it.
fn predelay_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    view: &View,
) {
    let k = s.k;
    let bpm = view.bpm;
    let sync = (host.spec().get("sync").round().max(0.0) as usize).min(SYNC_BEATS.len() - 1);
    let lang = crate::i18n::locale();
    let bpm_bits = bpm.to_bits();
    if st
        .words
        .resync
        .as_ref()
        .is_none_or(|w| w.0 != bpm_bits || w.1 != lang)
    {
        let text = tf("space.resync", &[("bpm", &format!("{bpm:.0}"))]);
        st.words.resync = Some((bpm_bits, lang, text));
    }
    let resync = st.words.resync.as_ref().map_or("", |w| w.2.as_str());
    controls::card(ui, s, rect, Src::Key("fx.space.predelay"), None, |ui, _| {
        let spec = host.spec();
        if let Some(info) = spec.kind.param("predelay") {
            let out = controls::Cell::param(info, spec.get("predelay"))
                .size(Size::M)
                .format(&ms_pill)
                .show(ui, s);
            if let Some(v) = out.changed {
                host.apply(
                    &[("predelay", Some(v)), ("sync", Some(0.0))],
                    "cmd.space_edit",
                    Some(G_PRE),
                );
            }
            if out.done {
                host.end_gesture();
            }
            tip(
                ui,
                &out.resp,
                frame::tr("fx.space.predelay"),
                "space.predelay_tip",
            );
        }
        controls::column(ui, s, 100.0, |ui| {
            let id = ui.id().with("space-sync");
            let at = controls::field(
                ui,
                s,
                id,
                frame::tr(SYNC_KEYS[sync]),
                frame::tr("fx.space.sync"),
                100.0,
            );
            if let Some(at) = at {
                // The list is built only while it is open.
                let labels: Vec<String> = (0..SYNC_BEATS.len())
                    .map(|i| {
                        if i == 0 {
                            t(SYNC_KEYS[0])
                        } else {
                            format!("{}  {}", t(SYNC_KEYS[i]), Ms(sync_ms(i, bpm).min(500.0)))
                        }
                    })
                    .collect();
                let items: Vec<crate::kit::menu::Item<'_>> = labels
                    .iter()
                    .enumerate()
                    .map(|(i, l)| frame::radio(Src::Str(l), i == sync))
                    .collect();
                if let Some(i) = controls::list(ui, s, id, at, 150.0, &items) {
                    if i == 0 {
                        host.set("sync", 0.0, None);
                    } else {
                        host.apply(
                            &[
                                ("sync", Some(i as f32)),
                                ("predelay", Some(sync_ms(i, bpm).min(500.0))),
                            ],
                            "cmd.space_edit",
                            None,
                        );
                    }
                }
            }
            if sync > 0 {
                let want = sync_ms(sync, bpm).min(500.0);
                if (want - view.s.predelay_ms).abs() > 0.5
                    && controls::key(
                        ui,
                        s,
                        Src::Str(resync),
                        frame::tr("space.resync_tip"),
                        100.0,
                        true,
                    )
                    .clicked()
                {
                    host.set("predelay", want, None);
                }
            }
        });
        let _ = k;
    });
}

/// `12.0 ms`, `125 ms`, `1.20 s`, as the knobs print them.
struct Ms(f32);

impl std::fmt::Display for Ms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.0;
        if v >= 1_000.0 {
            write!(f, "{:.2} s", v / 1_000.0)
        } else if v >= 100.0 {
            write!(f, "{v:.0} ms")
        } else {
            write!(f, "{v:.1} ms")
        }
    }
}

/// Distance, character and brightness.
fn character_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("space.card.character"),
        None,
        |ui, _| {
            for (key, what) in [
                ("distance", "space.distance_tip"),
                ("character", "space.character_tip"),
                ("bright", "space.bright_tip"),
            ] {
                let r = controls::param(ui, s, host, key, Size::M);
                if let Some(i) = host.spec().kind.param(key) {
                    tip(ui, &r, frame::tr(i.label_key), what);
                }
            }
        },
    );
}

/// Width and mix, and under them the wet signal's level per side and the sides'
/// correlation — the check a wide reverb needs in mono.
fn output_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, view: &View) {
    let k = s.k;
    let so = s.function(Function::Output);
    controls::card(ui, &so, rect, Src::Key("space.output"), None, |ui, cr| {
        let body = cr.body;
        let cells = Rect::from_min_size(body.min, vec2(body.width(), 86.0 * k));
        let mut cu = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(cells)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        cu.spacing_mut().item_spacing.x = 4.0 * k;
        for (key, what) in [("width", "space.width_tip"), ("mix", "space.mix_tip")] {
            let r = controls::param(&mut cu, &so, host, key, Size::M);
            if let Some(i) = host.spec().kind.param(key) {
                tip(&cu, &r, frame::tr(i.label_key), what);
            }
        }
        let bars = Rect::from_min_max(pos2(body.left(), cells.bottom() + 4.0 * k), body.max);
        let resp = ui.interact(bars, ui.id().with("space-out-meters"), Sense::hover());
        if resp.hovered() {
            hint::set_text(
                ui.ctx(),
                frame::tr("space.corr_name"),
                frame::tr("space.corr_tip"),
                Caps::NONE,
            );
        }
        output_meters(ui, &so, bars, view);
    });
}

/// Three bars — left, right, correlation — each with its name and its number, in one
/// brush.
fn output_meters(ui: &egui::Ui, s: &Style, rect: Rect, view: &View) {
    let k = s.k;
    let tk = s.tk();
    let t = &s.t;
    let ink = Ink::of(s);
    let db = |x: f32| if x > 1e-6 { 20.0 * x.log10() } else { -120.0 };
    let (l, r) = (db(view.tel[tel::WET_L]), db(view.tel[tel::WET_R]));
    let rho = view.tel[tel::CORRELATION];
    let level_ink = |d: f32| {
        if d >= 0.0 {
            t.meter_clip
        } else if d > -6.0 {
            t.meter_high
        } else {
            t.meter_low
        }
    };
    // A decorrelated tail swings either side of zero over a 300 ms window; only a
    // reading well below it means the sides are fighting in mono.
    let corr_ink = if rho < MONO_TROUBLE {
        t.danger
    } else if rho < 0.0 {
        t.meter_high
    } else {
        t.meter_low
    };
    let row_h = (rect.height() / 3.0).min(14.0 * k);
    let name_w = 26.0 * k;
    let value_w = 34.0 * k;
    with_brush(&ui.painter_at(rect.expand(2.0)), |b| {
        let px = b.px();
        let rows = [
            ((l + 48.0) / 48.0, level_ink(l)),
            ((r + 48.0) / 48.0, level_ink(r)),
            ((rho + 1.0) * 0.5, corr_ink),
        ];
        for (i, (frac, color)) in rows.iter().enumerate() {
            let y = rect.top() + row_h * i as f32;
            let bar = Rect::from_min_max(
                pos2(rect.left() + name_w, y + row_h * 0.25),
                pos2(rect.right() - value_w, y + row_h * 0.75),
            );
            let bar = b.snap_rect(bar);
            b.rect(bar, Fill::Solid(t.meter_well));
            let mut fill = bar;
            fill.set_width(bar.width() * frac.clamp(0.0, 1.0));
            b.rect(fill, Fill::Solid(*color));
            // Ticks every quarter, so the colour is not the only cue.
            for q in 1..4 {
                let x = b.snap_mid(bar.left() + bar.width() * q as f32 / 4.0);
                b.vline(x, bar.top(), bar.bottom(), px, ink.grid_strong);
            }
        }
        for (i, key) in ["space.left", "space.right", "space.corr"]
            .into_iter()
            .enumerate()
        {
            let y = rect.top() + row_h * (i as f32 + 0.5);
            canvas::put_key(
                b,
                pos2(rect.left(), y),
                Align2::LEFT_CENTER,
                key,
                Role::Micro,
                ink.dim,
                tk,
            );
            let at = pos2(rect.right(), y);
            match i {
                0 | 1 => {
                    let d = if i == 0 { l } else { r };
                    if d > -100.0 {
                        canvas::putf(
                            b,
                            at,
                            Align2::RIGHT_CENTER,
                            Role::Micro,
                            ink.text,
                            tk,
                            None,
                            format_args!("{d:.1}"),
                        );
                    } else {
                        canvas::putf(
                            b,
                            at,
                            Align2::RIGHT_CENTER,
                            Role::Micro,
                            ink.text,
                            tk,
                            None,
                            format_args!("-\u{221e}"),
                        );
                    }
                }
                _ => {
                    let c = if rho < MONO_TROUBLE {
                        ink.danger
                    } else {
                        ink.text
                    };
                    canvas::putf(
                        b,
                        at,
                        Align2::RIGHT_CENTER,
                        Role::Micro,
                        c,
                        tk,
                        None,
                        format_args!("{rho:+.2}"),
                    );
                }
            }
        }
    });
}

// ---- the cards under the displays -----------------------------------------------------

fn mult_pill(v: f32, b: &mut String) {
    let _ = write!(b, "\u{d7}{v:.2}");
}

fn q_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.2}");
}

fn slope_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.0} dB/oct");
}

/// The band selected on the curve: its shape, where it acts (post EQ), what it does to
/// the decay there (decay curve), and its cells. The band's colour marks the card.
fn band_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    view: &View,
) {
    let k = s.k;
    let sel = st.selected.filter(|(c, n)| band_used(host.spec(), *c, *n));
    let Some((curve, n)) = sel else {
        controls::card(ui, s, rect, Src::Key("space.band"), None, |ui, cr| {
            with_brush(ui.painter(), |b| {
                canvas::line_in(
                    b,
                    Rect::from_min_size(cr.body.min, vec2(cr.body.width(), controls::ROW * k)),
                    Role::Body,
                    s.t.text_dim,
                    s.tk(),
                    format_args!("{}", frame::tr("space.band_empty")),
                );
            });
            let r = ui.interact(cr.body, ui.id().with("space-band-empty"), Sense::hover());
            if r.hovered() {
                hint::set_text(
                    ui.ctx(),
                    frame::tr("space.band"),
                    frame::tr("space.band_hint"),
                    Caps::NONE,
                );
            }
        });
        return;
    };
    let lang = crate::i18n::locale();
    if st
        .words
        .band
        .as_ref()
        .is_none_or(|w| w.0 != (curve, n) || w.1 != lang)
    {
        let text = match curve {
            Curve::Decay => tf("space.decay_band_n", &[("n", &band_label(curve, n))]),
            Curve::Post => tf("space.post_band_n", &[("n", &band_label(curve, n))]),
        };
        st.words.band = Some(((curve, n), lang, text));
    }
    let spec = host.spec();
    let shape = spec.get(skey(curve, n, "shape")).round().max(0.0) as usize;
    let on = spec.get(skey(curve, n, "on")) >= 0.5;
    let freq = spec.get(skey(curve, n, "freq"));
    // What the band does to the decay, in the units that matter: the decay there now, and
    // the room's without the band.
    if curve == Curve::Decay {
        let c = view.s.target_curve();
        let (now, base) = (c.t60(freq), c.base_t60(freq));
        let bits = [freq.to_bits(), now.to_bits(), base.to_bits()];
        if st
            .words
            .band_now
            .as_ref()
            .is_none_or(|w| w.0 != bits || w.1 != lang)
        {
            // The room's decay there, and where the band takes it.
            let short = format!("{} \u{2192} {}", Secs(base), Secs(now));
            let long = tf(
                "space.band_now",
                &[
                    ("hz", &Hz(freq).to_string()),
                    ("s", &seconds(now)),
                    ("base", &seconds(base)),
                ],
            );
            st.words.band_now = Some((bits, lang, short, long));
        }
    }
    let title = st.words.band.as_ref().map_or("", |w| w.2.as_str());
    let (now_short, now_long) = st
        .words
        .band_now
        .as_ref()
        .map_or(("", ""), |w| (w.2.as_str(), w.3.as_str()));
    let marked = s.with_hue(curve::band_color(curve, n));
    let cells_s = s.function(if curve == Curve::Post {
        Function::Tone
    } else {
        Function::Space
    });
    let mut remove = false;
    let (_, lamp) = controls::card(ui, &marked, rect, Src::Str(title), Some(on), |ui, _| {
        controls::column(ui, s, 120.0, |ui| {
            let (keys, name): (&[&'static str], _) = match curve {
                Curve::Decay => (&DSHAPE_KEYS, frame::tr("fx.space.band.shape")),
                Curve::Post => (&PSHAPE_KEYS, frame::tr("fx.space.band.shape")),
            };
            let id = ui.id().with(("space-shape", curve == Curve::Post, n));
            let value = frame::tr(keys[shape.min(keys.len() - 1)]);
            if let Some(at) = controls::field(ui, s, id, value, name, 120.0) {
                let items: Vec<crate::kit::menu::Item<'_>> = keys
                    .iter()
                    .enumerate()
                    .map(|(i, key)| frame::radio(Src::Key(key), i == shape))
                    .collect();
                if let Some(i) = controls::list(ui, s, id, at, 140.0, &items) {
                    host.set(skey(curve, n, "shape"), i as f32, None);
                }
            }
            match curve {
                Curve::Post => {
                    let place =
                        (host.spec().get(skey(curve, n, "place")).round().max(0.0) as usize).min(4);
                    let id = ui.id().with(("space-place", n));
                    if let Some(at) = controls::field(
                        ui,
                        s,
                        id,
                        frame::tr(PLACE_KEYS[place]),
                        frame::tr("fx.space.band.place"),
                        120.0,
                    ) {
                        let items: Vec<crate::kit::menu::Item<'_>> = PLACE_KEYS
                            .iter()
                            .enumerate()
                            .map(|(i, key)| frame::radio(Src::Key(key), i == place))
                            .collect();
                        if let Some(i) = controls::list(ui, s, id, at, 140.0, &items) {
                            host.set(skey(curve, n, "place"), i as f32, None);
                        }
                    }
                }
                Curve::Decay => {
                    let r = controls::readout(
                        ui,
                        s,
                        now_short,
                        true,
                        frame::tr("space.band_now_name"),
                        120.0,
                    );
                    if r.hovered() {
                        hint::set_text(
                            ui.ctx(),
                            frame::tr("space.band_now_name"),
                            now_long,
                            Caps::NONE,
                        );
                    }
                }
            }
            if controls::key(
                ui,
                s,
                Src::Key("space.remove_band"),
                frame::tr("space.remove_band"),
                120.0,
                false,
            )
            .clicked()
            {
                remove = true;
            }
        });
        let freq_r = controls::param(ui, &cells_s, host, skey(curve, n, "freq"), Size::M);
        tip(
            ui,
            &freq_r,
            frame::tr("fx.space.band.freq"),
            "space.band.freq_tip",
        );
        match curve {
            Curve::Decay if shape != 3 => {
                let r = controls::param_with(
                    ui,
                    &cells_s,
                    host,
                    skey(curve, n, "mult"),
                    Size::M,
                    Some(&mult_pill),
                );
                tip(
                    ui,
                    &r,
                    frame::tr("fx.space.band.mult"),
                    "space.band.mult_tip",
                );
            }
            Curve::Post if shape < 3 => {
                let r = controls::param(ui, &cells_s, host, skey(curve, n, "gain"), Size::M);
                tip(
                    ui,
                    &r,
                    frame::tr("fx.space.band.gain"),
                    "space.band.gain_tip",
                );
            }
            _ => {}
        }
        if curve == Curve::Post && shape >= 3 {
            let r = controls::param_with(
                ui,
                &cells_s,
                host,
                skey(curve, n, "slope"),
                Size::M,
                Some(&slope_pill),
            );
            tip(
                ui,
                &r,
                frame::tr("fx.space.band.slope"),
                "space.band.slope_tip",
            );
        } else {
            let r = controls::param_with(
                ui,
                &cells_s,
                host,
                skey(curve, n, "q"),
                Size::M,
                Some(&q_pill),
            );
            tip(ui, &r, frame::tr("fx.space.band.q"), "space.band.q_tip");
        }
    });
    if lamp {
        host.set(skey(curve, n, "on"), if on { 0.0 } else { 1.0 }, None);
    }
    if remove {
        remove_band(host, st, curve, n);
    }
}

/// The early reflections: their level and size, and what reaches the listener after the
/// distance — level and window.
fn early_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, view: &View) {
    let room = room_at(view.s.space);
    let window = room.er_end_ms;
    let er_pill = |v: f32, b: &mut String| {
        if v <= -30.0 {
            b.push_str(frame::tr("space.off"));
        } else {
            let _ = write!(b, "{v:+.1} dB");
        }
    };
    let size_pill = move |v: f32, b: &mut String| controls::write_readout(window * v, "ms", b);
    controls::card(ui, s, rect, Src::Key("space.early"), None, |ui, _| {
        let r = controls::param_with(ui, s, host, "er", Size::M, Some(&er_pill));
        tip(ui, &r, frame::tr("fx.space.er"), "space.er_tip");
        let r = controls::param_with(ui, s, host, "er_size", Size::M, Some(&size_pill));
        tip(ui, &r, frame::tr("fx.space.er_size"), "space.er_size_tip");
        controls::column(ui, s, 84.0, |ui| {
            controls::caption(ui, s, Src::Key("space.early_heard"), 84.0);
            let level = er_level_db(view.s.style, view.s.distance, view.s.er_db)
                .filter(|_| view.s.style != Kind::Spring);
            let name = frame::tr("space.early_heard");
            let what = if level.is_some() {
                frame::tr("space.early_heard_tip")
            } else {
                frame::tr("space.early_off")
            };
            let r1 = match level {
                Some(db) => with_fmt(format_args!("{db:+.1} dB"), |x| {
                    controls::readout(ui, s, x, true, name, 84.0)
                }),
                None => controls::readout(ui, s, "\u{2013}", true, name, 84.0),
            };
            let r2 = match level {
                Some(_) => with_fmt(
                    format_args!("{}", Ms(room.er_end_ms * view.s.er_size)),
                    |x| controls::readout(ui, s, x, true, name, 84.0),
                ),
                None => controls::readout(ui, s, "\u{2013}", true, name, 84.0),
            };
            if r1.hovered() || r2.hovered() {
                hint::set_text(ui.ctx(), name, what, Caps::NONE);
            }
        });
    });
}

/// A pill's writer: the value into the text the pill shows.
type Pill<'a> = &'a dyn Fn(f32, &mut String);

fn duck_pill(v: f32, b: &mut String) {
    if v <= 0.0 {
        b.push_str(frame::tr("space.off"));
    } else {
        let _ = write!(b, "{v:.1} dB");
    }
}

fn db1_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.1} dB");
}

/// Ducking: what it listens to, its gain reduction over the last seconds, and its cells.
/// A card in the dynamics hue: it is a compressor on the reverb.
fn duck_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    view: &View,
) {
    let k = s.k;
    let sd = s.function(Function::Dynamics);
    controls::card(ui, &sd, rect, Src::Key("space.ducking"), None, |ui, cr| {
        let col = (cr.body.width() / k - 4.0 * 64.0 - 4.0 * 4.0).max(110.0);
        controls::column(ui, &sd, col, |ui| {
            key_source(host, st, ui, &sd, col);
            let (r, resp) = ui.allocate_exact_size(vec2(col * k, 46.0 * k), Sense::hover());
            duck_history(ui, &sd, r, st, view);
            if resp.hovered() {
                hint::set_text(
                    ui.ctx(),
                    frame::tr("space.gr_name"),
                    frame::tr("space.gr_tip"),
                    Caps::NONE,
                );
            }
        });
        let pills: [Pill<'_>; 4] = [&duck_pill, &db1_pill, &ms_pill, &ms_pill];
        let cells = [
            ("duck", "space.duck_tip"),
            ("duck_thr", "space.duck_thr_tip"),
            ("duck_att", "space.duck_att_tip"),
            ("duck_rel", "space.duck_rel_tip"),
        ];
        for ((key, what), pill) in cells.into_iter().zip(pills) {
            let r = controls::param_with(ui, &sd, host, key, Size::M, Some(pill));
            if let Some(i) = host.spec().kind.param(key) {
                tip(ui, &r, frame::tr(i.label_key), what);
            }
        }
    });
}

/// Where the ducking listens: this strip's own input or any strip of the desk (Cantus),
/// or the plug-in's sidechain input, switched on (another DAW routes it).
fn key_source(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, w: f32) {
    let rev = host.revision();
    if st.key.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.key = Some((rev, host.key_source()));
    }
    let name = frame::tr("space.key_name");
    match st.key.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips {
            current,
            label,
            options,
        }) => {
            let current = *current;
            let id = ui.id().with("space-key");
            let value = if current.is_some() {
                label.as_str()
            } else {
                frame::tr("space.key_own")
            };
            if let Some(at) = controls::field(ui, s, id, value, name, w) {
                let mut items = vec![frame::radio(Src::Key("space.key_own"), current.is_none())];
                items.extend(
                    options
                        .iter()
                        .map(|(sid, text)| frame::radio(Src::Str(text), current == Some(*sid))),
                );
                if let Some(i) = controls::list(ui, s, id, at, w.max(180.0), &items) {
                    let target = if i == 0 {
                        None
                    } else {
                        options.get(i - 1).map(|o| o.0)
                    };
                    drop(items);
                    host.set_key_source(KeyTarget::Strip(target));
                }
            }
        }
        Some(KeySource::Switch(on)) => {
            let on = *on;
            let labels = [
                Src::Key("space.key_own"),
                Src::Key("export.sidechain.external"),
            ];
            let tips = ["space.key_tip", "space.key_tip"];
            if let Some(i) = choice(ui, s, on as usize, &labels, &tips, name, w) {
                host.set_key_source(KeyTarget::Switch(i == 1));
            }
        }
        None => {}
    }
}

/// Ducking's gain reduction over the last seconds, hanging from the top, and the number.
fn duck_history(ui: &egui::Ui, s: &Style, rect: Rect, st: &State, view: &View) {
    let k = s.k;
    let ink = Ink::of(s);
    let hist = &st.duck;
    let range = view.s.duck_db.max(6.0);
    let dx = rect.width() / DUCK_HISTORY as f32;
    let x0 = rect.right() - dx * hist.len() as f32;
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let mut pts = [egui::Pos2::ZERO; DUCK_HISTORY];
        let n = hist.len().min(DUCK_HISTORY);
        for (i, gr) in hist.iter().enumerate().take(n) {
            pts[i] = pos2(
                x0 + dx * i as f32,
                rect.top() + (gr / range).clamp(0.0, 1.0) * rect.height(),
            );
        }
        b.area(&pts[..n], rect.top(), Ink::a(ink.danger, 0.55));
        canvas::putf(
            b,
            pos2(rect.right() - 4.0 * k, rect.bottom() - 3.0 * k),
            Align2::RIGHT_BOTTOM,
            Role::Value,
            ink.text,
            s.tk(),
            Some(ink.plate),
            format_args!("{:.1} dB", view.tel[tel::DUCK_DB]),
        );
        canvas::edge_b(b, rect, &ink);
    });
}

// ---- the inspector's picture ------------------------------------------------------------

/// The inspector picture's words and what they were made from.
type PreviewWords = (usize, u32, u32, Locale, String);

thread_local! {
    /// The picture's words, kept until what they say changes: (room, decay bits,
    /// pre-delay bits, language) and the line.
    static PREVIEW: std::cell::RefCell<Option<PreviewWords>> =
        const { std::cell::RefCell::new(None) };
}

/// The inspector's picture: the decay curve in seconds across the spectrum, and the room,
/// the decay and the pre-delay in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Space));
    let ink = Ink::of(&st);
    let s = settings(spec);
    let c = s.target_curve();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), space::GRID * 14.0),
        Sense::click(),
    );
    let plot = rect.shrink2(vec2(space::S2, space::S2));
    let (lo, hi) = curve::axis_seconds(s.decay_s);
    let x = |hz: f32| plot.left() + (hz / 20.0).log(1_000.0) * plot.width();
    let y = |sec: f32| plot.bottom() - (sec / lo).ln() / (hi / lo).ln() * plot.height();
    let room = room_index(s.space);
    let key = (
        room,
        s.decay_s.to_bits(),
        s.predelay_ms.to_bits(),
        crate::i18n::locale(),
    );
    PREVIEW.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache.as_ref().is_none_or(|p| (p.0, p.1, p.2, p.3) != key) {
            let text = format!(
                "{} \u{b7} {} \u{b7} {}",
                room_name(room),
                seconds(s.decay_s),
                Ms(s.predelay_ms)
            );
            *cache = Some((key.0, key.1, key.2, key.3, text));
        }
        let words = cache.as_ref().map_or("", |p| p.4.as_str());
        with_brush(&ui.painter_at(rect), |b| {
            canvas::ground_b(b, rect, &ink, st.hue_tok);
            b.hline(
                plot.left(),
                plot.right(),
                b.snap_mid(y(s.decay_s)),
                b.px(),
                ink.grid_strong,
            );
            let mut pts = [egui::Pos2::ZERO; 81];
            for (i, p) in pts.iter_mut().enumerate() {
                let hz = 20.0 * 1_000f32.powf(i as f32 / 80.0);
                *p = pos2(x(hz), y(c.t60(hz).clamp(lo, hi)));
            }
            b.polyline(&pts, 1.5, ink.hue);
            canvas::line_in(
                b,
                Rect::from_min_size(
                    rect.min + vec2(space::S2, space::S1),
                    vec2(rect.width() - 2.0 * space::S2 - 60.0, 16.0),
                ),
                Role::Label,
                ink.text,
                st.tk(),
                format_args!("{words}"),
            );
            if s.freeze {
                canvas::put_key(
                    b,
                    rect.right_top() + vec2(-space::S2, space::S1),
                    Align2::RIGHT_TOP,
                    "fx.space.freeze",
                    Role::Micro,
                    st.t.highlight,
                    st.tk(),
                );
            }
            let edge = if resp.hovered() { ink.hue } else { ink.edge };
            canvas::edge_b(b, rect, &Ink { edge, ..ink });
        });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("space.open_tip"), "", Caps::NONE);
    }
    resp
}

// ---- demo ----------------------------------------------------------------------------

/// The settings `--editor space` and the screenshot tool open it with: a hall with a
/// drawn curve, a post EQ, a synced pre-delay and some ducking.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let bpm = app.project().tempo_bpm;
    let fields: Vec<(String, f32)> = [
        ("space", 0.62),
        ("decay", 2.4),
        ("sync", 4.0),
        ("predelay", sync_ms(4, bpm).min(500.0)),
        ("distance", 0.45),
        ("character", 0.35),
        ("bright", 0.4),
        ("mix", 0.3),
        ("duck", 6.0),
        ("duck_thr", -24.0),
        ("duck_rel", 150.0),
        ("d0.used", 1.0),
        ("d0.shape", 1.0),
        ("d0.freq", 220.0),
        ("d0.mult", 1.45),
        ("d1.used", 1.0),
        ("d1.freq", 2_600.0),
        ("d1.mult", 0.55),
        ("d1.q", 1.1),
        ("e0.used", 1.0),
        ("e0.shape", 3.0),
        ("e0.freq", 140.0),
        ("e1.used", 1.0),
        ("e1.shape", 2.0),
        ("e1.freq", 7_500.0),
        ("e1.gain", -4.0),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect();
    app.set_space_params(strip, effect, &fields, false);
    app.fx.space.start_for(effect);
    app.fx.space.selected = Some((Curve::Decay, 1));
    app.fx.space.mode = Curve::Decay;
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// DSP run offline over a snare-like hit and its tail, as (input, output) pairs, and its
/// telemetry.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(slot) = app.project().effect(strip, effect).cloned() else {
        return;
    };
    app.fx.space.start_for(effect);
    use cantus_engine::FxUnit;
    let fs = 48_000.0f32;
    let mut unit = cantus_engine::fx::space::Unit::new(fs);
    unit.configure(&slot.spec);
    unit.reset();
    // A hall longer than the measure waits for is fed past its four seconds, so the
    // picture shows a cut tail (`shoot … space-cut`).
    let seconds = if settings(&slot.spec).decay_s > 4.0 {
        4.6
    } else {
        3.2
    };
    let n = (seconds * fs) as usize;
    let mut seed = 11u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let hit = |t: f32, noise: f32| -> f32 {
        if !(0.0..0.25).contains(&t) {
            return 0.0;
        }
        let body = (t * 190.0 * std::f32::consts::TAU).sin() * (-t / 0.05).exp();
        0.5 * body + 0.45 * noise * (-t / 0.07).exp()
    };
    let input: Vec<f32> = (0..n).map(|i| hit(i as f32 / fs - 0.05, noise())).collect();
    let mut pairs = Vec::with_capacity(n);
    let mut telemetry = None;
    let block = 800;
    for (b, chunk) in input.chunks(block).enumerate() {
        let mut l = chunk.to_vec();
        let mut r = chunk.to_vec();
        unit.process(&mut l, &mut r, None);
        for (i, x) in chunk.iter().enumerate() {
            pairs.push((*x, 0.5 * (l[i] + r[i])));
        }
        let t_now = (b * block) as f32 / fs;
        let st = &mut app.fx.space;
        if t_now < 4.0 {
            if st.duck.len() == DUCK_HISTORY {
                st.duck.pop_front();
            }
            st.duck.push_back(unit.telemetry()[tel::DUCK_DB]);
        }
        if telemetry.is_none() && t_now >= 0.35 {
            telemetry = Some(unit.telemetry());
        }
    }
    let s = settings(&slot.spec);
    let (dry, wet) = mix_gains(s.mix);
    let cap = &mut app.fx.space.capture;
    cap.push(&pairs, fs, slot.mix, dry, wet);
    cap.analyse();
    app.fx.injected = telemetry;
}
