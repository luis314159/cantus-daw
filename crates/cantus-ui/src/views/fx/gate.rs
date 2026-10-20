//! Porta's editor: the gate's own window, where what you see is what you drag.
//!
//! The benchmark gate's display is read-only, its hysteresis is fixed per style, its
//! styles are named but not explained, and you cannot hear what it takes away
//! (`research/fabfilter/pro-g.md`, "How we beat it"). So here:
//!
//! - **The level history is the control.** The open threshold is a solid line and the
//!   close threshold a dashed one, both dragged on the history (Alt moves the open line
//!   alone, leaving the close line where it is); the band between them *is* the
//!   hysteresis. Under the history, a lane shows the gate's phase — open, hold, release,
//!   closed — by colour *and* pattern, so chatter is visible as flicker. The gain change
//!   hangs from the top. When the song plays, bar lines run through it.
//! - **The transfer curve is the control too**: drag the open and close points, the slope
//!   below the threshold (the ratio) and the floor (the range).
//! - **The key's filters are dragged on a spectrum** of what comes in, with the response
//!   they make drawn over it.
//! - **Every style says what it is**, in words, under the buttons.
//! - **Hear the gate, the key, or what the gate removes** — the last is what the benchmark
//!   cannot do.
//! - **The host's advantage**: *Learn* sets the thresholds from a passage as it plays; *Hits*
//!   turns the gate's openings into a MIDI clip on a new drum track, as one undo step.
//!
//! Every drag and knob turn is one step of the project's undo.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost, KeySource, KeyTarget};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::space;
use crate::i18n::{t, tf};
use crate::kit::band;
use crate::kit::hint::{self, Caps};
use crate::kit::menu::Item;
use crate::kit::shell::Grid;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use cantus_core::{
    Command, EffectId, EffectKind, EffectSpec, InstrumentSpec, PulsusParams, StripId, Ticks,
    TrackId,
};
use cantus_dsp::porta::{self, telemetry, Column, GateSettings, Mode, Phase, Style as GateStyle};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};
use std::collections::VecDeque;
use std::fmt::Write as _;

/// The level scale of the history and the transfer curve, dB.
const LO_DB: f32 = -80.0;
/// Columns of history kept: at 5 ms each, four seconds.
pub const HISTORY: usize = 800;
/// The key strip's frequency axis.
const F_LO: f32 = 20.0;
const F_HI: f32 = 20_000.0;

/// Factory presets: a label key and the parameters it sets (the device frame's preset list).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.gate.kick",
        &[
            ("thr", -30.0),
            ("hyst", 6.0),
            ("range", 40.0),
            ("att", 0.5),
            ("hold", 40.0),
            ("rel", 120.0),
            ("look", 2.0),
            ("lpf", 150.0),
        ],
    ),
    (
        "preset.gate.snare",
        &[
            ("thr", -28.0),
            ("hyst", 5.0),
            ("range", 30.0),
            ("att", 0.3),
            ("hold", 50.0),
            ("rel", 180.0),
            ("look", 2.0),
            ("hpf", 150.0),
            ("lpf", 8_000.0),
        ],
    ),
    (
        "preset.gate.toms",
        &[
            ("thr", -32.0),
            ("hyst", 8.0),
            ("range", 24.0),
            ("att", 1.0),
            ("hold", 80.0),
            ("rel", 350.0),
            ("look", 3.0),
            ("style", 1.0),
            ("hpf", 60.0),
            ("lpf", 1_200.0),
        ],
    ),
    (
        "preset.gate.hats",
        &[
            ("thr", -35.0),
            ("hyst", 4.0),
            ("range", 18.0),
            ("att", 0.2),
            ("hold", 15.0),
            ("rel", 60.0),
            ("look", 1.0),
            ("style", 1.0),
            ("hpf", 5_000.0),
            ("key_slope", 48.0),
        ],
    ),
    (
        "preset.gate.vocal",
        &[
            ("thr", -45.0),
            ("hyst", 6.0),
            ("ratio", 3.0),
            ("range", 12.0),
            ("att", 5.0),
            ("hold", 60.0),
            ("rel", 250.0),
            ("look", 5.0),
            ("style", 2.0),
            ("hpf", 100.0),
        ],
    ),
    (
        "preset.gate.guitar",
        &[
            ("thr", -50.0),
            ("hyst", 4.0),
            ("ratio", 4.0),
            ("range", 30.0),
            ("att", 2.0),
            ("hold", 30.0),
            ("rel", 200.0),
            ("style", 3.0),
            ("hpf", 80.0),
            ("lpf", 6_000.0),
        ],
    ),
    (
        "preset.gate.noise",
        &[
            ("thr", -60.0),
            ("hyst", 6.0),
            ("ratio", 2.0),
            ("range", 12.0),
            ("att", 5.0),
            ("hold", 100.0),
            ("rel", 400.0),
            ("style", 1.0),
        ],
    ),
    (
        "preset.gate.duck",
        &[
            ("thr", -30.0),
            ("range", 10.0),
            ("att", 10.0),
            ("hold", 50.0),
            ("rel", 250.0),
            ("look", 5.0),
            ("style", 5.0),
        ],
    ),
    (
        "preset.gate.punch",
        &[
            ("thr", -20.0),
            ("ratio", 1.5),
            ("range", 6.0),
            ("att", 1.0),
            ("hold", 0.0),
            ("rel", 80.0),
            ("style", 4.0),
        ],
    ),
];

/// One column of the history, and where the song was when it was made (`None` while
/// stopped), for the bar lines.
#[derive(Copy, Clone, Debug)]
pub struct Col {
    pub c: Column,
    pub tick: Option<i64>,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    /// The open threshold (Alt: alone, the close line staying put).
    Open {
        alone: bool,
    },
    Close,
    Ratio,
    Range,
    Hpf,
    Lpf,
}

/// What the editor remembers between frames.
#[derive(Debug, Default)]
pub struct State {
    /// Oldest first.
    pub history: VecDeque<Col>,
    /// The effect the history belongs to.
    effect: Option<EffectId>,
    last_seq: Option<f32>,
    last_hits: Option<f32>,
    drag: Option<Grab>,
    /// Key levels gathered while learning, one per column.
    pub learning: Option<Vec<f32>>,
    /// Openings become hits while the song plays.
    pub capturing: bool,
    /// Captured hits: song position (ticks) and the key's peak, dB.
    pub hits: Vec<(i64, f32)>,
    /// The kit pad the hits are written to.
    pub hit_pitch: Option<u8>,
    spectrum: Option<cantus_dsp::SpectrumAnalyser>,
    /// Where the last frame drew the history's plot and the transfer curve, and the dB
    /// range both span — published so a test can aim a drag at a line.
    pub geometry: Option<Geometry>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// What the chosen style is, in words, per style and language: two lines.
    words: Option<(usize, crate::i18n::Locale, String, String)>,
    /// "Hits 12", per count.
    hits_label: Option<(usize, String)>,
    /// The kit's pads (pitch, name), read once.
    pads: Option<Vec<(u8, String)>>,
    /// The key's source as the host said it, per host revision.
    key: Option<(u64, Option<KeySource>)>,
    /// The analysed input, reused.
    scratch: Vec<f32>,
    /// What the tap held when it was last analysed (length, newest, oldest).
    analysed: Option<Pair3>,
}

/// A cheap signature of the tap's contents.
type Pair3 = (usize, Option<(u32, u32)>, Option<(u32, u32)>);

/// The displays as the last frame laid them out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geometry {
    /// The history's plot (without the phase lane): `LO_DB` at the bottom, 0 dB at the top.
    pub history: Rect,
    /// The transfer curve: `LO_DB` … +12 dB on both axes.
    pub transfer: Rect,
}

impl Geometry {
    /// The height of a level on the history.
    pub fn history_y(&self, db: f32) -> f32 {
        self.history.bottom() - ((db - LO_DB) / -LO_DB).clamp(0.0, 1.0) * self.history.height()
    }

    /// Where a level pair lands on the transfer curve.
    pub fn transfer_pos(&self, input_db: f32, output_db: f32) -> Pos2 {
        let span = 12.0 - LO_DB;
        let r = self.transfer;
        pos2(
            r.left() + ((input_db - LO_DB) / span).clamp(0.0, 1.0) * r.width(),
            r.bottom() - ((output_db - LO_DB) / span).clamp(0.0, 1.0) * r.height(),
        )
    }
}

impl State {
    /// Take what the telemetry brought since the last frame: new columns into the
    /// history (with the song position, if playing), new hits into the capture.
    /// `now` is the song position at the end of the last block, `ticks_per_second` the
    /// tempo in ticks; both `None`/ignored while stopped.
    pub fn ingest(&mut self, t: &[f32; 64], now: Option<i64>, ticks_per_second: f64) {
        use telemetry::*;
        let new = match self.last_seq {
            Some(last) => since(t[SEQ], last),
            // Opening on a running gate: what the telemetry holds, up to its 24 columns.
            None => t[SEQ].max(0.0) as u32,
        }
        .min(porta::HISTORY_COLUMNS as u32) as usize;
        self.last_seq = Some(t[SEQ]);
        let col_ticks = (t[COLUMN_MS].max(0.0) as f64 * 0.001 * ticks_per_second).round() as i64;
        for j in porta::HISTORY_COLUMNS - new..porta::HISTORY_COLUMNS {
            let c = unpack(t[COLUMNS_AT + 2 * j], t[COLUMNS_AT + 2 * j + 1]);
            let back = (porta::HISTORY_COLUMNS - 1 - j) as i64;
            let tick = now.map(|n| n - back * col_ticks);
            if let Some(levels) = self.learning.as_mut() {
                levels.push(c.key_db);
            }
            if self.history.len() == HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(Col { c, tick });
        }
        let hits = match self.last_hits {
            Some(last) => since(t[HITS], last),
            None => 0,
        }
        .min(porta::EVENTS as u32) as usize;
        self.last_hits = Some(t[HITS]);
        if let (true, Some(now)) = (self.capturing, now) {
            let sr = t[SAMPLE_RATE].max(1.0) as f64;
            for j in (0..hits).rev() {
                let ago = t[EVENTS_AT + 2 * j] as f64;
                let tick = now - (ago / sr * ticks_per_second).round() as i64;
                self.hits.push((tick.max(0), t[EVENTS_AT + 2 * j + 1]));
            }
        }
    }
}

/// Velocities for hits by their peaks: the loudest is 127, one 40 dB softer is 1, linear in
/// dB between — the dynamics of the playing, kept.
pub fn velocities(peaks_db: &[f32]) -> Vec<u8> {
    let top = peaks_db.iter().copied().fold(f32::MIN, f32::max);
    peaks_db
        .iter()
        .map(|p| (1.0 + 126.0 * ((p - (top - 40.0)) / 40.0).clamp(0.0, 1.0)).round() as u8)
        .collect()
}

/// Hits sorted, with those closer than 15 ms (a loop played twice, a flam counted twice)
/// merged into the louder.
pub fn merge_hits(hits: &[(i64, f32)], ticks_per_second: f64) -> Vec<(i64, f32)> {
    let mut v = hits.to_vec();
    v.sort_by_key(|h| h.0);
    let gap = (0.015 * ticks_per_second).round() as i64;
    let mut out: Vec<(i64, f32)> = Vec::with_capacity(v.len());
    for h in v {
        match out.last_mut() {
            Some(last) if h.0 - last.0 < gap => last.1 = last.1.max(h.1),
            _ => out.push(h),
        }
    }
    out
}

fn ticks_per_second(app: &CantusApp) -> f64 {
    crate::views::bars::bpm_at(app.project(), app.engine.position()) / 60.0
        * cantus_core::PPQ as f64
}

/// The pad the hits are written to: the chosen one, or the kit's first.
fn hit_pitch(st: &State) -> u8 {
    let kit = PulsusParams::default();
    st.hit_pitch
        .or_else(|| kit.pads.first().map(|p| p.pitch))
        .unwrap_or(36)
}

/// The captured hits as notes: merged, stamped in ticks, with velocities from their peaks.
fn hit_notes(st: &State, ticks_per_second: f64) -> Vec<(i64, u8)> {
    let hits = merge_hits(&st.hits, ticks_per_second);
    let peaks: Vec<f32> = hits.iter().map(|h| h.1).collect();
    hits.iter().map(|h| h.0).zip(velocities(&peaks)).collect()
}

/// Stop learning and set the thresholds from what was heard, as one step. Returns what
/// was learned, or `None` if the passage was too short or silent.
pub fn finish_learning(host: &mut dyn FxHost, st: &mut State) -> Option<porta::Learned> {
    let levels = st.learning.take()?;
    let hyst = host.spec().get("hyst");
    let Some(learned) = porta::learn_thresholds(&levels, hyst) else {
        host.status(t("gate.learn_none"));
        return None;
    };
    let hyst = (learned.open_db - learned.close_db).clamp(0.0, 24.0);
    host.set_many(
        &[("thr", learned.open_db), ("hyst", hyst)],
        "cmd.gate_learn",
    );
    host.status(tf(
        "gate.learned",
        &[
            ("open", &format!("{:.1}", learned.open_db)),
            ("close", &format!("{:.1}", learned.open_db - hyst)),
        ],
    ));
    Some(learned)
}

/// The captured hits into a MIDI clip, where the host can make one; cleared once written.
pub fn hits_to_clip(host: &mut dyn FxHost, st: &mut State) -> bool {
    let notes = hit_notes(st, host.transport().ticks_per_second);
    if notes.is_empty() {
        return false;
    }
    let ok = host.notes_to_clip(&notes, hit_pitch(st)) == Some(true);
    if ok {
        st.hits.clear();
    }
    ok
}

impl CantusApp {
    /// Stop learning and set the thresholds from what was heard, as one undo step.
    /// Returns what was learned, or `None` if the passage was too short or silent.
    pub fn finish_gate_learning(
        &mut self,
        strip: StripId,
        effect: EffectId,
    ) -> Option<porta::Learned> {
        let mut st = std::mem::take(&mut self.fx.gate);
        let learned = finish_learning(&mut DawHost::new(self, strip, effect, "gate"), &mut st);
        self.fx.gate = st;
        learned
    }

    /// The captured hits as a MIDI clip on a new drum track (Pulsus, the shipped kit),
    /// one note per hit at the chosen pad, velocity from the hit's peak — one undo step.
    pub fn gate_hits_to_clip(&mut self, strip: StripId) -> bool {
        let notes = hit_notes(&self.fx.gate, ticks_per_second(self));
        if notes.is_empty() {
            return false;
        }
        let pitch = hit_pitch(&self.fx.gate);
        let ok = self.gate_notes_to_clip(strip, &notes, pitch);
        if ok {
            self.fx.gate.hits.clear();
        }
        ok
    }

    /// Notes (tick, velocity) at `pitch` as a MIDI clip on a new drum track named after
    /// `strip`, bar-aligned — one undo step.
    pub(crate) fn gate_notes_to_clip(
        &mut self,
        strip: StripId,
        notes: &[(i64, u8)],
        pitch: u8,
    ) -> bool {
        if notes.is_empty() {
            return false;
        }
        let kit = PulsusParams::default();
        // Whole bars around the hits, through the metre changes (ADR 0060).
        let first = crate::views::bars::bar_start(self.project(), cantus_core::Ticks(notes[0].0)).0;
        let last = notes[notes.len() - 1].0;
        let end = crate::views::bars::map(self.project())
            .bar_end(cantus_core::Ticks(last))
            .0;
        let name = tf(
            "gate.hits_track",
            &[(
                "strip",
                &self.capture_label(cantus_engine::CaptureTarget::Strip(strip)),
            )],
        );
        let mut track = self.bus.new_track(name);
        track.instrument = InstrumentSpec::Pulsus(kit);
        let track_id = track.id;
        let mut clip = self
            .bus
            .new_clip(t("gate.hits_clip"), Ticks(first), Ticks(end - first));
        for (at, vel) in notes {
            let note = self
                .bus
                .new_note(Ticks(at - first), Ticks::SIXTEENTH, pitch, *vel);
            clip.notes.push(note);
        }
        let index = self.project().tracks.len();
        let ok = self.exec(Command::Batch {
            label_key: "cmd.gate_hits".into(),
            commands: vec![
                Command::InsertTrack {
                    index,
                    track: Box::new(track),
                },
                Command::InsertClip {
                    track: track_id,
                    clip: Box::new(clip),
                },
            ],
        });
        if ok {
            self.set_status(tf("gate.hits_made", &[("n", &notes.len())]));
        }
        ok
    }
}

/// What a style is, in words — the answer to "what does Guitar mean?".
pub fn style_words(style: GateStyle) -> String {
    let s = style.spec();
    let mode = match s.mode {
        Mode::Down => t("gate.words.down"),
        Mode::Up => t("gate.words.up"),
        Mode::Duck => t("gate.words.duck"),
    };
    let detector = if s.rms_ms > 0.0 {
        tf("gate.words.rms", &[("ms", &format!("{:.0}", s.rms_ms))])
    } else {
        tf(
            "gate.words.peak",
            &[("ms", &format!("{:.0}", s.peak_fall_ms))],
        )
    };
    let shape = if s.shape < 0.05 {
        t("gate.words.lines")
    } else {
        t("gate.words.curve")
    };
    let mut out = format!("{mode} \u{b7} {detector} \u{b7} {shape}");
    if s.follow {
        out.push_str(" \u{b7} ");
        out.push_str(&t("gate.words.follow"));
    }
    out
}

/// `+1.5 dB`.
struct Db(f32);

impl std::fmt::Display for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:+.1} dB", self.0)
    }
}

/// `4.0:1`, or `∞:1`.
struct Ratio(f32);

impl std::fmt::Display for Ratio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= porta::INFINITE_RATIO {
            f.write_str("\u{221e}:1")
        } else {
            write!(f, "{:.1}:1", self.0)
        }
    }
}

/// `40 dB`, or `∞`.
struct Range(f32);

impl std::fmt::Display for Range {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= porta::INFINITE_RANGE {
            f.write_str("\u{221e}")
        } else {
            write!(f, "{:.0} dB", self.0)
        }
    }
}

fn phase_key(p: Phase) -> &'static str {
    match p {
        Phase::Open => "gate.phase.open",
        Phase::Hold => "gate.phase.hold",
        Phase::Release => "gate.phase.release",
        Phase::Closed => "gate.phase.closed",
    }
}

/// The lane's colour for a phase in these inks; hold is also hatched and release is also
/// paler, so the colour is never the only cue.
fn phase_ink(p: Phase, ink: &Ink) -> Option<Color32> {
    match p {
        Phase::Open => Some(ink.ok),
        Phase::Hold => Some(Ink::a(ink.ok, 0.75)),
        Phase::Release => Some(Ink::a(ink.ok, 0.35)),
        Phase::Closed => None,
    }
}

/// The key card's column of switches, at 100 %: wide enough for "Removed" in a third.
const KEY_COLUMN: f32 = 172.0;

const STYLE_KEYS: [&str; 6] = [
    "gate.style.0",
    "gate.style.1",
    "gate.style.2",
    "gate.style.3",
    "gate.style.4",
    "gate.style.5",
];

/// Porta's editor in Cantus's own window: the host is the project (every drag one undo
/// step, the key any strip of the desk, hits a MIDI clip, the gate played by a track's
/// notes).
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.gate);
    if st.effect != Some(effect) {
        let pitch = st.hit_pitch;
        st = State {
            effect: Some(effect),
            hit_pitch: pitch,
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "gate"), &mut st, ui);
    app.fx.gate = st;
    out
}

/// Porta's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW — the device frame and the gate inside it. Everything it needs from the
/// host goes through [`FxHost`].
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let transport = host.transport();
    let tel = host.telemetry();
    st.ingest(&tel, transport.playing_at, transport.ticks_per_second);
    let sr = if tel[telemetry::SAMPLE_RATE] > 0.0 {
        tel[telemetry::SAMPLE_RATE]
    } else {
        host.sample_rate()
    };
    let gs = cantus_engine::fx::gate::settings(host.spec());
    // With lookahead on, the gate delays its audio by a fixed amount, whatever the knob.
    let latency = if gs.lookahead_ms <= 0.0 {
        0.0
    } else if tel[telemetry::LATENCY] > 0.0 {
        tel[telemetry::LATENCY]
    } else {
        porta::LOOKAHEAD_MAX_MS * 0.001 * sr
    };
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency, sr)];
    let meter = st.history.back().map(|c| frame::Levels {
        level: [c.c.output_db; 2],
        hold: [c.c.output_db; 2],
    });
    let look = frame::Look {
        kind: EffectKind::Gate,
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

/// The gate inside the frame: the style row, the displays, the cells.
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
    let gs = cantus_engine::fx::gate::settings(host.spec());
    let daw = host.makes_clips();
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row1 = (band::TITLE + 8.0 + 66.0 + 8.0) * k;
    let row3 = controls::card_height(Size::L) * k;
    let row2 = (area.height() - row1 - row3 - 2.0 * gap).max(160.0 * k);

    // Row 1: the style and what it is; what opens the gate (Cantus); learning and hits.
    let fr1: &[f32] = if daw {
        &[0.52, 0.20, 0.28]
    } else {
        &[0.72, 0.28]
    };
    let r1: Vec<Rect> = g.row(row1, fr1).collect();
    style_card(host, st, ui, s, r1[0], &gs);
    if daw {
        trigger_card(host, ui, s, r1[1], &gs);
    }
    learn_card(host, st, ui, s, r1[r1.len() - 1]);

    // Row 2: the history over the key strip, and the transfer curve beside them.
    let side = (row2 - band::TITLE * k).min(area.width() * 0.42);
    let lf = (area.width() - side - gap) / (area.width() - gap);
    let r2: Vec<Rect> = g.row(row2, &[lf, 1.0 - lf]).collect();
    let key_h = (band::TITLE + 64.0) * k;
    let hist_r = Rect::from_min_max(r2[0].min, pos2(r2[0].right(), r2[0].bottom() - key_h - gap));
    let key_r = Rect::from_min_max(pos2(r2[0].left(), hist_r.bottom() + gap), r2[0].max);
    let disp = |r: Rect| (r.height() / k - band::TITLE - 1.0).max(20.0);
    let (_, _) = controls::card_with(
        ui,
        s,
        hist_r,
        Src::Key("gate.card.level"),
        None,
        Some(disp(hist_r)),
        |ui, cr| {
            if let Some(d) = cr.display {
                history(host, st, ui, s, &gs, tel, d);
            }
        },
    );
    let (_, _) = controls::card_with(
        ui,
        s,
        key_r,
        Src::Key("gate.card.key_filter"),
        None,
        Some(disp(key_r)),
        |ui, cr| {
            if let Some(d) = cr.display {
                key_strip(host, st, ui, s, &gs, sr, d);
            }
        },
    );
    let (_, _) = controls::card_with(
        ui,
        s,
        r2[1],
        Src::Key("gate.card.transfer"),
        None,
        Some(disp(r2[1])),
        |ui, cr| {
            if let Some(d) = cr.display {
                transfer(host, st, ui, s, &gs, tel, d);
            }
        },
    );

    // Row 3: the cells.
    let w = [
        controls::cells_width(&[Size::L, Size::M, Size::M, Size::M, Size::M]) + 16.0,
        controls::cells_width(&[Size::M; 4]) + 16.0,
        KEY_COLUMN + 4.0 + controls::cells_width(&[Size::M; 2]) + 16.0,
    ];
    let sum: f32 = w.iter().sum();
    let fr3 = [w[0] / sum, w[1] / sum, w[2] / sum];
    let r3: Vec<Rect> = g.row(row3, &fr3).collect();
    detect_card(host, ui, s, r3[0], &gs);
    time_card(host, ui, s, r3[1]);
    key_card(host, st, ui, s, r3[2], &gs);
}

/// The styles, as one segmented control, and what the chosen one is, in words.
fn style_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    gs: &GateSettings,
) {
    let lang = crate::i18n::locale();
    let idx = gs.style.index();
    if st.words.as_ref().is_none_or(|w| w.0 != idx || w.1 != lang) {
        let spec = gs.style.spec();
        let mode = t(match spec.mode {
            Mode::Down => "gate.words.down",
            Mode::Up => "gate.words.up",
            Mode::Duck => "gate.words.duck",
        });
        let detector = if spec.rms_ms > 0.0 {
            tf("gate.words.rms", &[("ms", &format!("{:.0}", spec.rms_ms))])
        } else {
            tf(
                "gate.words.peak",
                &[("ms", &format!("{:.0}", spec.peak_fall_ms))],
            )
        };
        let mut shape = t(if spec.shape < 0.05 {
            "gate.words.lines"
        } else {
            "gate.words.curve"
        });
        if spec.follow {
            shape.push_str(" \u{b7} ");
            shape.push_str(&t("gate.words.follow"));
        }
        st.words = Some((idx, lang, format!("{mode} \u{b7} {detector}"), shape));
    }
    controls::card(ui, s, rect, Src::Key("fx.gate.style"), None, |ui, cr| {
        let w = cr.body.width() / s.k;
        controls::column(ui, s, w, |ui| {
            let labels = STYLE_KEYS.map(Src::Key);
            let r = controls::segmented(ui, s, idx, &labels, frame::tr("fx.gate.style"), w);
            if let Some(i) = r {
                host.set("style", i as f32, None);
            }
            if let Some((_, _, a, b)) = &st.words {
                for line in [a.as_str(), b.as_str()] {
                    let (lr, _) = ui.allocate_exact_size(vec2(w * s.k, s.d(16.0)), Sense::hover());
                    with_brush(ui.painter(), |bb| {
                        let t = Text::new(line, Role::Body, s.t.text_dim)
                            .align(Align2::LEFT_CENTER)
                            .policy(Policy::Elide(Elide::End));
                        let _ = ktext::paint(bb, lr, &t, s.tk());
                    });
                }
            }
        });
    });
}

/// What opens the gate — the key's level or a track's notes (ADR 0022) — and, played by
/// notes, which track. Cantus only: the source is a track of the project.
fn trigger_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    gs: &GateSettings,
) {
    controls::card(ui, s, rect, Src::Key("fx.gate.trig"), None, |ui, cr| {
        let w = cr.body.width() / s.k;
        controls::column(ui, s, w, |ui| {
            let labels = [Src::Key("gate.trig.0"), Src::Key("gate.trig.1")];
            if let Some(i) = controls::segmented(
                ui,
                s,
                gs.midi as usize,
                &labels,
                frame::tr("fx.gate.trig"),
                w,
            ) {
                host.set("trig", i as f32, None);
            }
            if !gs.midi {
                return;
            }
            let Some((app, strip, effect)) = host.daw() else {
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
            let id = ui.id().with(("gate-midi", effect.0));
            let value = name.unwrap_or(frame::tr("gate.midi_none"));
            let at = controls::field(ui, s, id, value, frame::tr("gate.midi_tip"), w);
            if let Some(at) = at {
                let tracks: Vec<(TrackId, String)> = app
                    .project()
                    .tracks
                    .iter()
                    .map(|t| (t.id, t.name.clone()))
                    .collect();
                let mut items = vec![frame::radio(Src::Key("gate.midi_none"), current.is_none())];
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
                controls::caption(ui, s, Src::Key("gate.midi_waiting"), w);
            }
        });
    });
}

/// Learn the thresholds from a passage; keep the openings as hits; write them to a clip.
fn learn_card(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("gate.card.learn"), None, |ui, cr| {
        let w = cr.body.width() / s.k;
        let half = (w - 4.0) * 0.5;
        controls::column(ui, s, w, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = s.d(4.0);
                // Learn: gather while on; set the thresholds when switched off.
                let learning = st.learning.is_some();
                let r = match &st.learning {
                    Some(v) => {
                        let text = tf(
                            "gate.learning",
                            &[(
                                "s",
                                &format!("{:.1}", v.len() as f32 * porta::COLUMN_MS * 0.001),
                            )],
                        );
                        controls::key(
                            ui,
                            s,
                            Src::Str(&text),
                            frame::tr("gate.learn_tip"),
                            half,
                            true,
                        )
                    }
                    None => controls::key(
                        ui,
                        s,
                        Src::Key("gate.learn"),
                        frame::tr("gate.learn_tip"),
                        half,
                        false,
                    ),
                };
                if r.clicked() {
                    if learning {
                        finish_learning(host, st);
                    } else {
                        st.learning = Some(Vec::new());
                    }
                }
                // Hits into a MIDI clip: only a DAW can make one.
                if host.makes_clips() {
                    let n = st.hits.len();
                    if st.hits_label.as_ref().is_none_or(|h| h.0 != n) {
                        st.hits_label = Some((n, tf("gate.hits", &[("n", &n)])));
                    }
                    let label = st.hits_label.as_ref().map_or("", |h| h.1.as_str());
                    if controls::led(
                        ui,
                        s,
                        st.capturing,
                        Src::Str(label),
                        frame::tr("gate.hits_tip"),
                        half,
                    )
                    .clicked()
                    {
                        st.capturing = !st.capturing;
                    }
                }
            });
            if !host.makes_clips() {
                return;
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = s.d(4.0);
                let pads = st.pads.get_or_insert_with(|| {
                    PulsusParams::default()
                        .pads
                        .iter()
                        .map(|p| (p.pitch, p.name.clone()))
                        .collect()
                });
                let pitch = st
                    .hit_pitch
                    .or_else(|| pads.first().map(|p| p.0))
                    .unwrap_or(36);
                let pad = pads
                    .iter()
                    .find(|p| p.0 == pitch)
                    .map_or("", |p| p.1.as_str());
                let id = ui.id().with("gate-pad");
                if let Some(at) = controls::field(ui, s, id, pad, frame::tr("gate.pad_tip"), half) {
                    let items: Vec<Item<'_>> = pads
                        .iter()
                        .map(|(p, n)| frame::radio(Src::Str(n), *p == pitch))
                        .collect();
                    if let Some(i) = controls::list(ui, s, id, at, half, &items) {
                        st.hit_pitch = pads.get(i).map(|p| p.0);
                    }
                }
                let n = st.hits.len();
                ui.add_enabled_ui(n > 0, |ui| {
                    if controls::key(
                        ui,
                        s,
                        Src::Key("gate.to_clip"),
                        frame::tr("gate.to_clip_tip"),
                        half,
                        false,
                    )
                    .clicked()
                    {
                        hits_to_clip(host, st);
                    }
                });
            });
        });
    });
}

/// The level history: input as a dim fill, output over it in the device's hue, the key as
/// a line, the gain change hanging from the top, the phase lane at the bottom, and the
/// open and close thresholds as lines you drag.
#[allow(clippy::too_many_arguments)]
fn history(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    gs: &GateSettings,
    tel: &[f32; 64],
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(rect, ui.id().with("gate-history"), Sense::click_and_drag());
    // A strip at the top for the legend and the readout, the plot, the phase lane.
    let lane_h = 12.0 * k;
    let head_h = 20.0 * k;
    let plot = Rect::from_min_max(
        pos2(rect.left(), rect.top() + head_h),
        pos2(rect.right(), rect.bottom() - lane_h - 1.0),
    );
    let lane = Rect::from_min_max(pos2(rect.left(), rect.bottom() - lane_h), rect.max);
    let y = |d: f32| plot.bottom() - ((d - LO_DB) / -LO_DB).clamp(0.0, 1.0) * plot.height();
    let d_of = |py: f32| LO_DB + (plot.bottom() - py) / plot.height() * -LO_DB;
    let geo = st.geometry.get_or_insert(Geometry {
        history: plot,
        transfer: Rect::NOTHING,
    });
    geo.history = plot;
    let hist = &st.history;
    let dx = plot.width() / HISTORY as f32;
    let x0 = plot.right() - dx * hist.len() as f32;
    let x_of = |i: usize| x0 + dx * (i as f32 + 0.5);
    let (ty, cy) = (y(gs.threshold_db), y(gs.close_db()));
    let hover = resp.hover_pos();
    let near_open = hover.is_some_and(|pt| plot.contains(pt) && (pt.y - ty).abs() < 6.0 * k);
    let near_close =
        !near_open && hover.is_some_and(|pt| plot.contains(pt) && (pt.y - cy).abs() < 6.0 * k);
    let drag = st.drag;
    let open_hot = near_open || matches!(drag, Some(Grab::Open { .. }));
    let close_hot = near_close || drag == Some(Grab::Close);
    let swing = gs.swing_db().max(1.0);
    let gy = |g: f32| plot.top() + (g.abs() / swing).clamp(0.0, 1.0) * plot.height() * 0.33;
    let bar = host.transport().ticks_per_bar.max(1);
    // In Cantus, the bars through the song's metre changes (review 2).
    let map = host
        .daw()
        .map(|(app, _, _)| crate::views::bars::map(app.project()))
        .filter(|m| !m.is_one_metre());
    let new_bar = |a: i64, b: i64| match &map {
        Some(m) => m.bar_start(cantus_core::Ticks(a)) != m.bar_start(cantus_core::Ticks(b)),
        None => a.div_euclid(bar) != b.div_euclid(bar),
    };
    let mut pts = [Pos2::ZERO; HISTORY];
    let n = hist.len().min(HISTORY);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let px = b.px();
        for g in [-12.0f32, -24.0, -36.0, -48.0, -60.0, -72.0] {
            b.hline(plot.left(), plot.right(), b.snap_mid(y(g)), px, ink.grid);
        }
        // Seconds ago; bar lines while the song plays.
        let secs = (HISTORY as f32 * porta::COLUMN_MS * 0.001) as usize;
        for sec in 1..=secs {
            let x = plot.right() - sec as f32 / (porta::COLUMN_MS * 0.001) * dx;
            b.vline(b.snap_mid(x), plot.top(), plot.bottom(), px, ink.grid);
        }
        let mut prev: Option<i64> = None;
        for (i, c) in hist.iter().enumerate() {
            if let (Some(tk_), Some(pt)) = (c.tick, prev) {
                if new_bar(tk_, pt) && tk_ > pt {
                    b.vline(
                        b.snap_mid(x_of(i)),
                        plot.top(),
                        plot.bottom(),
                        px,
                        ink.grid_strong,
                    );
                }
            }
            prev = c.tick;
        }
        // The hysteresis band, then the fills.
        b.rect(
            Rect::from_x_y_ranges(plot.x_range(), ty..=cy),
            Fill::Solid(ink.fill(0.25)),
        );
        let line = |value: &dyn Fn(&Column) -> f32, pts: &mut [Pos2; HISTORY]| {
            for (i, c) in hist.iter().enumerate().take(n) {
                pts[i] = pos2(x_of(i), y(value(&c.c)));
            }
        };
        if n > 1 {
            line(&|c| c.input_db, &mut pts);
            b.area(&pts[..n], plot.bottom(), Ink::a(ink.dim, 0.18));
            line(&|c| c.output_db, &mut pts);
            b.area(&pts[..n], plot.bottom(), ink.fill(1.0));
            b.polyline(&pts[..n], 1.25 * k, ink.hue);
            line(&|c| c.key_db, &mut pts);
            b.polyline(&pts[..n], 1.25 * k, Ink::a(ink.text, 0.85));
            // The gain change, from the top: 0 at the top, the whole swing a third down.
            for (i, c) in hist.iter().enumerate().take(n) {
                pts[i] = pos2(x_of(i), gy(c.c.gain_db));
            }
            b.polyline(&pts[..n], 1.5 * k, ink.danger);
        }
        // Its scale: the whole swing.
        let sx = plot.left() + 32.0 * k;
        b.hline(sx, sx + 12.0 * k, b.snap_mid(gy(swing)), px, ink.danger);
        // The phase lane: colour and pattern per run.
        b.rect(b.snap_rect(lane), Fill::Solid(s.t.deck_title));
        let mut i = 0;
        while i < n {
            let ph = hist[i].c.phase;
            let mut j = i;
            while j < n && hist[j].c.phase == ph {
                j += 1;
            }
            if let Some(color) = phase_ink(ph, &ink) {
                let r = Rect::from_x_y_ranges(
                    x_of(i) - dx * 0.5..=x_of(j - 1) + dx * 0.5,
                    lane.y_range(),
                );
                b.rect(r, Fill::Solid(color));
                if ph == Phase::Hold {
                    canvas::hatch(b, r, 4.0 * k, px, s.t.deck_title);
                }
            }
            i = j;
        }
        // The thresholds: open solid, close dashed; heavier when near or held.
        b.hline(
            plot.left(),
            plot.right(),
            ty,
            if open_hot { 2.5 } else { 1.5 } * k,
            ink.hue,
        );
        canvas::dashed(
            b,
            pos2(plot.left(), cy),
            pos2(plot.right(), cy),
            if close_hot { 2.0 } else { 1.0 } * k,
            ink.hue,
            6.0 * k,
            4.0 * k,
        );
        // The words last, so nothing is painted over them.
        let micro =
            |b: &mut crate::kit::Brush, at: Pos2, align: Align2, args: std::fmt::Arguments<'_>| {
                // Axis words sit over the fills: on plates, so they always read.
                canvas::putf(
                    b,
                    at,
                    align,
                    Role::Micro,
                    ink.dim,
                    tk,
                    Some(ink.plate),
                    args,
                )
            };
        for g in [-12.0f32, -24.0, -36.0, -48.0, -60.0, -72.0] {
            micro(
                b,
                pos2(plot.left() + 3.0 * k, y(g) - 1.0),
                Align2::LEFT_BOTTOM,
                format_args!("{g:.0}"),
            );
        }
        for sec in 1..=secs {
            let x = plot.right() - sec as f32 / (porta::COLUMN_MS * 0.001) * dx;
            micro(
                b,
                pos2(x + 2.0 * k, plot.bottom() - 2.0 * k),
                Align2::LEFT_BOTTOM,
                format_args!("\u{2212}{sec} s"),
            );
        }
        let mut prev: Option<i64> = None;
        for (i, c) in hist.iter().enumerate() {
            if let (Some(tk_), Some(pt)) = (c.tick, prev) {
                if tk_.div_euclid(bar) != pt.div_euclid(bar) && tk_ > pt {
                    micro(
                        b,
                        pos2(x_of(i) + 2.0 * k, plot.top() + 2.0 * k),
                        Align2::LEFT_TOP,
                        format_args!("{}", tk_.div_euclid(bar) + 1),
                    );
                }
            }
            prev = c.tick;
        }
        let gain = frame::tr("gate.gain");
        let plate = Some(ink.plate);
        let swing_at = pos2(sx + 16.0 * k, gy(swing));
        if gs.style.spec().mode == Mode::Up {
            canvas::tag(
                b,
                swing_at,
                Align2::LEFT_CENTER,
                gain,
                ink.danger,
                tk,
                plate,
                format_args!("+{:.0} dB", gs.swing_db()),
            );
        } else {
            canvas::tag(
                b,
                swing_at,
                Align2::LEFT_CENTER,
                gain,
                ink.danger,
                tk,
                plate,
                format_args!("\u{2212}{}", Range(gs.range_db)),
            );
        }
        canvas::tag(
            b,
            pos2(plot.right() - 8.0 * k, ty - 3.0 * k),
            Align2::RIGHT_BOTTOM,
            frame::tr("gate.open"),
            ink.hue,
            tk,
            plate,
            format_args!("{}", Db(gs.threshold_db)),
        );
        canvas::tag(
            b,
            pos2(plot.right() - 8.0 * k, cy + 3.0 * k),
            Align2::RIGHT_TOP,
            frame::tr("gate.close"),
            ink.hue,
            tk,
            plate,
            format_args!("{}", Db(gs.close_db())),
        );
        // The head: the legend, so the lane reads without colour; what is happening now,
        // right to left: the phase, the gain, the key.
        let mid = rect.top() + head_h * 0.5;
        legend(b, pos2(rect.left() + 8.0 * k, mid), s, &ink);
        let phase = Phase::from_code(tel[telemetry::PHASE].max(0.0) as u32);
        let r = canvas::put_key(
            b,
            pos2(rect.right() - 8.0 * k, mid),
            Align2::RIGHT_CENTER,
            phase_key(phase),
            Role::Micro,
            ink.text,
            tk,
        );
        let r = canvas::tag(
            b,
            pos2(r.left() - 12.0 * k, mid),
            Align2::RIGHT_CENTER,
            gain,
            ink.text,
            tk,
            None,
            format_args!("{}", Db(tel[telemetry::GAIN])),
        );
        canvas::tag(
            b,
            pos2(r.left() - 12.0 * k, mid),
            Align2::RIGHT_CENTER,
            frame::tr("gate.key"),
            ink.text,
            tk,
            None,
            format_args!("{}", Db(tel[telemetry::LEVEL].max(-120.0))),
        );
        // Hover: what happened at that moment, left to right on one plate each.
        if let Some(pt) = hover.filter(|pt| rect.contains(*pt)) {
            let i = ((pt.x - x0) / dx).floor();
            if i >= 0.0 && (i as usize) < n {
                let c = hist[i as usize].c;
                let ago = (n - i as usize) as f32 * porta::COLUMN_MS * 0.001;
                b.vline(
                    b.snap_mid(pt.x),
                    plot.top(),
                    plot.bottom(),
                    px,
                    ink.grid_strong,
                );
                let y = plot.bottom() - 16.0 * k;
                let mut x = plot.left() + 8.0 * k;
                let gap = 10.0 * k;
                let r = canvas::tag(
                    b,
                    pos2(x, y),
                    Align2::LEFT_BOTTOM,
                    "",
                    ink.text,
                    tk,
                    plate,
                    format_args!("\u{2212}{ago:.2} s"),
                );
                x = r.right() + gap;
                let r = canvas::tag(
                    b,
                    pos2(x, y),
                    Align2::LEFT_BOTTOM,
                    frame::tr("gate.key"),
                    ink.text,
                    tk,
                    plate,
                    format_args!("{}", Db(c.key_db)),
                );
                x = r.right() + gap;
                let r = canvas::tag(
                    b,
                    pos2(x, y),
                    Align2::LEFT_BOTTOM,
                    gain,
                    ink.text,
                    tk,
                    plate,
                    format_args!("{}", Db(c.gain_db)),
                );
                x = r.right() + gap;
                let r = canvas::tag(
                    b,
                    pos2(x, y),
                    Align2::LEFT_BOTTOM,
                    frame::tr(phase_key(c.phase)),
                    ink.text,
                    tk,
                    plate,
                    format_args!("\u{2192} {:.1} dB", d_of(pt.y)),
                );
                let _ = r;
            }
        }
        if n == 0 {
            canvas::put_key(
                b,
                plot.center(),
                Align2::CENTER_CENTER,
                "gate.empty",
                Role::Body,
                ink.dim,
                tk,
            );
        }
    });

    // Gestures.
    if resp.drag_started() {
        // Decide by where the button went down: by the time a drag is recognised the
        // pointer has already moved off the line.
        let alone = ui.input(|i| i.modifiers.alt);
        let at = ui.input(|i| i.pointer.press_origin()).or(hover);
        let on =
            |line: f32| at.is_some_and(|pt| plot.contains(pt) && (pt.y - line).abs() < 6.0 * k);
        st.drag = if on(ty) {
            Some(Grab::Open { alone })
        } else if on(cy) {
            Some(Grab::Close)
        } else {
            None
        };
    }
    if let (Some(g), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            drag_threshold(host, gs, g, d_of(pt.y));
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.double_clicked() {
        if near_open {
            host.set("thr", -40.0, None);
        } else if near_close {
            host.set("hyst", 4.0, None);
        }
    }
    if near_open || near_close || drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("gate.card.level"),
            frame::tr("gate.history_tip"),
            Caps::NONE,
        );
    }
}

/// Move a threshold to `db`: the open line carries the close line with it (Alt: alone);
/// the close line sets the hysteresis.
fn drag_threshold(host: &mut dyn FxHost, s: &GateSettings, g: Grab, db: f32) {
    match g {
        Grab::Open { alone: false } => {
            host.set("thr", db.clamp(-80.0, 0.0), Some("thr"));
        }
        Grab::Open { alone: true } => {
            let close = s.close_db();
            let thr = db.clamp(close, (close + 24.0).min(0.0)).clamp(-80.0, 0.0);
            host.set("thr", thr, Some("open-alone"));
            host.set("hyst", (thr - close).clamp(0.0, 24.0), Some("open-alone"));
        }
        Grab::Close => {
            host.set("hyst", (s.threshold_db - db).clamp(0.0, 24.0), Some("hyst"));
        }
        _ => {}
    }
}

/// The phases' swatches and names, so the lane reads without its colours.
fn legend(b: &mut crate::kit::Brush, at: Pos2, s: &Style, ink: &Ink) {
    let k = s.k;
    let mut x = at.x;
    for ph in [Phase::Open, Phase::Hold, Phase::Release] {
        let r = b.snap_rect(Rect::from_min_size(
            pos2(x, at.y - 3.0 * k),
            vec2(12.0 * k, 6.0 * k),
        ));
        if let Some(c) = phase_ink(ph, ink) {
            b.rect(r, Fill::Solid(c));
        }
        if ph == Phase::Hold {
            canvas::hatch(b, r, 4.0 * k, b.px(), s.t.deck_title);
        }
        let lr = canvas::put_key(
            b,
            pos2(r.right() + 4.0 * k, at.y),
            Align2::LEFT_CENTER,
            phase_key(ph),
            Role::Micro,
            ink.dim,
            s.tk(),
        );
        x = lr.right().max(r.right()) + 10.0 * k;
    }
}

/// The transfer curve: key level across, output up. The closed curve is solid, the open
/// one (measured from the close threshold) dashed. Drag the open and close points, the
/// slope (ratio) and the floor (range).
#[allow(clippy::too_many_arguments)]
fn transfer(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    gs: &GateSettings,
    tel: &[f32; 64],
    outer: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(
        outer,
        ui.id().with("gate-transfer"),
        Sense::click_and_drag(),
    );
    // A strip at the top for the curve's numbers, as the history's.
    let head_h = 20.0 * k;
    let rect = Rect::from_min_max(pos2(outer.left(), outer.top() + head_h), outer.max);
    let hi = 12.0;
    let span = hi - LO_DB;
    let x = |d: f32| rect.left() + ((d - LO_DB) / span).clamp(0.0, 1.0) * rect.width();
    let y = |d: f32| rect.bottom() - ((d - LO_DB) / span).clamp(0.0, 1.0) * rect.height();
    let dx_of = |px: f32| LO_DB + (px - rect.left()) / rect.width() * span;
    let dy_of = |py: f32| LO_DB + (rect.bottom() - py) / rect.height() * span;
    if let Some(geo) = st.geometry.as_mut() {
        geo.transfer = rect;
    }
    let mode = gs.style.spec().mode;
    let open_pt = pos2(
        x(gs.threshold_db),
        y(porta::transfer_db(gs.threshold_db, gs, false)),
    );
    let close_pt = pos2(
        x(gs.close_db()),
        y(porta::transfer_db(gs.close_db(), gs, true)),
    );
    // The floor: where the gate lands when it shuts, under the threshold (kept inside the
    // display; dragging past the bottom goes on to ∞).
    let floor_pt = pos2(
        x(gs.threshold_db),
        y(gs.threshold_db - gs.swing_db()).min(rect.bottom() - 5.0 * k),
    );
    with_brush(&ui.painter_at(outer), |b| {
        canvas::ground_b(b, outer, &ink, s.hue_tok);
        let px = b.px();
        for g in [-72.0f32, -60.0, -48.0, -36.0, -24.0, -12.0, 0.0] {
            b.hline(rect.left(), rect.right(), b.snap_mid(y(g)), px, ink.grid);
            b.vline(b.snap_mid(x(g)), rect.top(), rect.bottom(), px, ink.grid);
        }
        b.segment(
            pos2(x(LO_DB), y(LO_DB)),
            pos2(x(hi), y(hi)),
            px,
            ink.grid_strong,
            false,
        );
        let mut pts = [Pos2::ZERO; 185];
        let curve = |open: bool, pts: &mut [Pos2; 185]| {
            for (i, p) in pts.iter_mut().enumerate() {
                let d = LO_DB + span * i as f32 / 184.0;
                *p = pos2(x(d), y(porta::transfer_db(d, gs, open)));
            }
        };
        if mode != Mode::Up && gs.hysteresis_db > 0.0 {
            curve(true, &mut pts);
            for w in pts.windows(2).step_by(2) {
                b.segment(w[0], w[1], 1.5 * k, Ink::a(ink.hue, 0.7), false);
            }
        }
        curve(false, &mut pts);
        b.polyline(&pts, 2.5 * k, ink.hue);
        // Handles: open and close points, the floor.
        b.circle(open_pt, 6.0 * k, Fill::Solid(ink.hue));
        if mode != Mode::Up {
            b.circle_stroke(close_pt, 5.0 * k, 2.0 * k, ink.hue);
        }
        if mode == Mode::Down {
            b.rect(
                b.snap_rect(Rect::from_center_size(floor_pt, vec2(9.0, 9.0) * k)),
                Fill::Solid(ink.hue),
            );
        }
        // Where the key is now.
        let level = tel[telemetry::LEVEL];
        if level > LO_DB {
            let out = level + tel[telemetry::GAIN].max(-120.0);
            b.circle(pos2(x(level), y(out)), 4.0 * k, Fill::Solid(ink.danger));
        }
        for g in [-60.0f32, -48.0, -36.0, -24.0, -12.0, 0.0] {
            canvas::putf(
                b,
                pos2(x(g) + 2.0 * k, rect.bottom() - 2.0 * k),
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{g:.0}"),
            );
            canvas::putf(
                b,
                pos2(rect.left() + 2.0 * k, y(g) - 1.0),
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{g:.0}"),
            );
        }
        // The curve's numbers, left to right.
        let mid = outer.top() + head_h * 0.5;
        let r = canvas::tag(
            b,
            pos2(outer.left() + 8.0 * k, mid),
            Align2::LEFT_CENTER,
            frame::tr("fx.gate.ratio"),
            ink.text,
            tk,
            None,
            format_args!("{}", Ratio(gs.ratio)),
        );
        let r = canvas::tag(
            b,
            pos2(r.right() + 12.0 * k, mid),
            Align2::LEFT_CENTER,
            frame::tr("fx.gate.range"),
            ink.text,
            tk,
            None,
            format_args!("{}", Range(gs.range_db)),
        );
        canvas::tag(
            b,
            pos2(r.right() + 12.0 * k, mid),
            Align2::LEFT_CENTER,
            frame::tr("fx.gate.knee"),
            ink.text,
            tk,
            None,
            format_args!("{:.0} dB", gs.knee_db),
        );
    });

    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(pt) = origin.or(resp.interact_pointer_pos()) {
            let near = |q: Pos2| pt.distance(q) < 9.0 * k;
            let d = dx_of(pt.x);
            st.drag = if near(open_pt) {
                Some(Grab::Open {
                    alone: ui.input(|i| i.modifiers.alt),
                })
            } else if mode != Mode::Up && near(close_pt) {
                Some(Grab::Close)
            } else if mode == Mode::Down && near(floor_pt) {
                Some(Grab::Range)
            } else if (mode == Mode::Down && d < gs.threshold_db)
                || (mode == Mode::Up && d > gs.threshold_db)
            {
                Some(Grab::Ratio)
            } else {
                None
            };
        }
    }
    if let (Some(g), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            match g {
                Grab::Open { .. } | Grab::Close => {
                    drag_threshold(host, gs, g, dx_of(pt.x));
                }
                Grab::Range => {
                    let r = (gs.threshold_db - dy_of(pt.y)).clamp(0.0, 100.0);
                    host.set("range", r, Some("range"));
                }
                Grab::Ratio => {
                    // The pointer is a point the curve should pass through: below the
                    // threshold (Upward: above) the output moves `ratio` dB per dB.
                    let over_in = dx_of(pt.x) - gs.threshold_db;
                    let over_out = dy_of(pt.y) - gs.threshold_db;
                    let ratio = if over_in.abs() < 0.5 {
                        gs.ratio
                    } else {
                        (over_out / over_in).clamp(1.0, 100.0)
                    };
                    host.set("ratio", ratio, Some("ratio"));
                }
                _ => {}
            }
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("gate.card.transfer"),
            frame::tr("gate.transfer_tip"),
            Caps::NONE,
        );
    }
}

/// The key's filters, dragged on a spectrum of what comes in.
#[allow(clippy::too_many_arguments)]
fn key_strip(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    gs: &GateSettings,
    sr: f32,
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(
        rect,
        ui.id().with("gate-key-strip"),
        Sense::click_and_drag(),
    );
    let lx = |hz: f32| rect.left() + (hz / F_LO).ln() / (F_HI / F_LO).ln() * rect.width();
    let hz_of =
        |px: f32| F_LO * (F_HI / F_LO).powf(((px - rect.left()) / rect.width()).clamp(0.0, 1.0));
    let (top_db, bot_db) = (18.0f32, -48.0f32);
    let y =
        |d: f32| rect.top() + ((top_db - d) / (top_db - bot_db)).clamp(0.0, 1.0) * rect.height();
    // What comes in (the effect's input), behind.
    {
        let an = st
            .spectrum
            .get_or_insert_with(|| cantus_dsp::SpectrumAnalyser::new(4096));
        let pairs = host.pairs();
        // Only when the tap brought something new: the analysis is the costliest thing
        // the editor does, and an editor at rest must cost next to nothing.
        let sig = (
            pairs.len(),
            pairs.back().map(|p| (p.0.to_bits(), p.1.to_bits())),
            pairs.front().map(|p| (p.0.to_bits(), p.1.to_bits())),
        );
        if st.analysed != Some(sig) {
            st.analysed = Some(sig);
            let take = pairs.len().min(an.size());
            st.scratch.clear();
            for (a, b) in pairs.iter().skip(pairs.len() - take) {
                st.scratch.push(*a);
                st.scratch.push(*b);
            }
            an.push(&st.scratch);
            an.compute();
        }
    }
    let any_pairs = !host.pairs().is_empty();
    let (lo, hi) = (
        if gs.hpf_on() { gs.hpf_hz } else { F_LO },
        if gs.lpf_on() { gs.lpf_hz } else { F_HI },
    );
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let px = b.px();
        for f in [
            50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
        ] {
            b.vline(b.snap_mid(lx(f)), rect.top(), rect.bottom(), px, ink.grid);
        }
        // 0 dB, for the response to sit on.
        canvas::dashed(
            b,
            pos2(rect.left(), y(0.0)),
            pos2(rect.right(), y(0.0)),
            px,
            ink.grid_strong,
            3.0 * k,
            3.0 * k,
        );
        if let Some(an) = st.spectrum.as_ref().filter(|a| a.has_data() || any_pairs) {
            let mut pts = [Pos2::ZERO; 161];
            for (i, p) in pts.iter_mut().enumerate() {
                let hz = F_LO * (F_HI / F_LO).powf(i as f32 / 160.0);
                // The spectrum sits on its own scale: 0 dBFS at the top, −96 at the bottom.
                let level = an.level_db(0, sr, hz, 1.0 / 6.0);
                *p = pos2(
                    lx(hz),
                    rect.top() + ((0.0 - level) / 96.0).clamp(0.0, 1.0) * rect.height(),
                );
            }
            b.area(&pts, rect.bottom(), Ink::a(ink.dim, 0.22));
        }
        // The pass band, lit; the response drawn over it.
        b.rect(
            Rect::from_x_y_ranges(lx(lo)..=lx(hi), rect.y_range()),
            Fill::Solid(ink.fill(0.22)),
        );
        let mut pts = [Pos2::ZERO; 201];
        for (i, p) in pts.iter_mut().enumerate() {
            let hz = F_LO * (F_HI / F_LO).powf(i as f32 / 200.0);
            *p = pos2(lx(hz), y(porta::key_filter_db(gs, sr, hz)));
        }
        b.polyline(&pts, 2.0 * k, ink.hue);
        // The handles: triangles at the corners.
        for (hz, on) in [(lo, gs.hpf_on()), (hi, gs.lpf_on())] {
            let cx = lx(hz);
            let by = rect.bottom() - 1.0;
            b.vline(
                b.snap_mid(cx),
                rect.top() + 4.0 * k,
                by - 10.0 * k,
                px,
                Ink::a(ink.hue, if on { 0.6 } else { 0.3 }),
            );
            let tri = [
                pos2(cx, by - 10.0 * k),
                pos2(cx - 6.0 * k, by),
                pos2(cx + 6.0 * k, by),
            ];
            if on {
                b.polygon(&tri, ink.hue);
            } else {
                b.polyline(&[tri[0], tri[1], tri[2], tri[0]], 1.5 * k, ink.hue);
            }
        }
        // The frequencies, then the handles' names — last, over everything.
        let mut ruler = canvas::Ruler::new();
        for f in [
            50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
        ] {
            let at = pos2(lx(f) + 2.0 * k, rect.bottom() - 2.0 * k);
            if !ruler.fits(at.x, 4.0 * k) {
                continue;
            }
            let r = canvas::putf(
                b,
                at,
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{}", Hz(f)),
            );
            ruler.placed(r);
        }
        for (hz, on, key, right) in [
            (lo, gs.hpf_on(), "gate.hp", false),
            (hi, gs.lpf_on(), "gate.lp", true),
        ] {
            let cx = lx(hz);
            let at = pos2(
                if right { cx - 5.0 * k } else { cx + 5.0 * k },
                rect.top() + 3.0 * k,
            );
            let align = if right {
                Align2::RIGHT_TOP
            } else {
                Align2::LEFT_TOP
            };
            if on {
                canvas::tag(
                    b,
                    at,
                    align,
                    frame::tr(key),
                    ink.text,
                    tk,
                    Some(ink.plate),
                    format_args!("{}", Hz(hz)),
                );
            } else {
                canvas::tag(
                    b,
                    at,
                    align,
                    frame::tr(key),
                    ink.dim,
                    tk,
                    Some(ink.plate),
                    format_args!("{}", frame::tr("gate.off")),
                );
            }
        }
    });

    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(pt) = origin.or(resp.interact_pointer_pos()) {
            let (dl, dh) = ((pt.x - lx(lo)).abs(), (pt.x - lx(hi)).abs());
            st.drag = Some(if dl <= dh { Grab::Hpf } else { Grab::Lpf });
        }
    }
    if let (Some(g), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let hz = hz_of(pt.x);
            match g {
                Grab::Hpf => {
                    let v = if hz <= 21.0 { 20.0 } else { hz.min(10_000.0) };
                    host.set("hpf", v, Some("hpf"));
                }
                Grab::Lpf => {
                    let v = if hz >= 19_000.0 {
                        20_000.0
                    } else {
                        hz.max(100.0)
                    };
                    host.set("lpf", v, Some("lpf"));
                }
                _ => {}
            }
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.double_clicked() {
        host.set_many(&[("hpf", 20.0), ("lpf", 20_000.0)], "cmd.gate_key_reset");
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("gate.card.key_filter"),
            frame::tr("gate.key_strip_tip"),
            Caps::NONE,
        );
    }
}

/// `1.2 kHz`, `350 Hz`, as the knobs print them.
struct Hz(f32);

impl std::fmt::Display for Hz {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{:.1} kHz", self.0 / 1_000.0)
        } else {
            write!(f, "{:.0} Hz", self.0)
        }
    }
}

// ---- the cells ----------------------------------------------------------------------------

fn ratio_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{}", Ratio(v));
}

fn range_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{}", Range(v));
}

fn db1_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.1} dB");
}

fn att_pill(v: f32, b: &mut String) {
    if v < 1.0 {
        let _ = write!(b, "{v:.2} ms");
    } else {
        controls::write_readout(v, "ms", b);
    }
}

fn look_pill(v: f32, b: &mut String) {
    if v <= 0.0 {
        b.push_str(frame::tr("gate.off"));
    } else {
        let _ = write!(b, "{v:.1} ms");
    }
}

fn hpf_pill(v: f32, b: &mut String) {
    if v <= 20.5 {
        b.push_str(frame::tr("gate.off"));
    } else {
        controls::write_readout(v, "Hz", b);
    }
}

fn lpf_pill(v: f32, b: &mut String) {
    if v >= 19_999.0 {
        b.push_str(frame::tr("gate.off"));
    } else {
        controls::write_readout(v, "Hz", b);
    }
}

/// When the gate opens and how far: the threshold (the device's primary parameter, a
/// larger knob), the hysteresis, ratio, range and knee. Played by notes, the level's knobs
/// mean nothing; the velocity's does.
fn detect_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, gs: &GateSettings) {
    controls::card(ui, s, rect, Src::Key("gate.group.detect"), None, |ui, _| {
        if gs.midi {
            controls::param_with(ui, s, host, "range", Size::M, Some(&range_pill));
            controls::param_with(ui, s, host, "vel", Size::M, Some(&db1_pill));
        } else {
            controls::param(ui, s, host, "thr", Size::L);
            controls::param_with(ui, s, host, "hyst", Size::M, Some(&db1_pill));
            controls::param_with(ui, s, host, "ratio", Size::M, Some(&ratio_pill));
            controls::param_with(ui, s, host, "range", Size::M, Some(&range_pill));
            controls::param(ui, s, host, "knee", Size::M);
        }
    });
}

/// How fast it opens, holds and closes, and how far it looks ahead.
fn time_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(ui, s, rect, Src::Key("gate.group.time"), None, |ui, _| {
        controls::param_with(ui, s, host, "att", Size::M, Some(&att_pill));
        controls::param(ui, s, host, "hold", Size::M);
        controls::param(ui, s, host, "rel", Size::M);
        controls::param_with(ui, s, host, "look", Size::M, Some(&look_pill));
    });
}

/// The key: where it comes from, what to hear, its filters and their slope.
fn key_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    gs: &GateSettings,
) {
    controls::card(ui, s, rect, Src::Key("gate.group.key"), None, |ui, _| {
        let w = KEY_COLUMN;
        controls::column(ui, s, w, |ui| {
            key_source(host, st, ui, s, w);
            let removed = if gs.style.spec().mode == Mode::Up {
                "gate.hear.2up"
            } else {
                "gate.hear.2"
            };
            let labels = [
                Src::Key("gate.hear.0"),
                Src::Key("gate.hear.1"),
                Src::Key(removed),
            ];
            controls::segmented_param(ui, s, host, "listen", &labels, w);
            let slopes = [Src::Str("12"), Src::Str("24"), Src::Str("48")];
            let now = [12.0f32, 24.0, 48.0]
                .iter()
                .position(|v| (gs.key_slope - v).abs() < 0.5)
                .unwrap_or(1);
            if let Some(i) =
                controls::segmented(ui, s, now, &slopes, frame::tr("gate.slope_tip"), w)
            {
                host.set("key_slope", [12.0, 24.0, 48.0][i], None);
            }
        });
        controls::param_with(ui, s, host, "hpf", Size::M, Some(&hpf_pill));
        controls::param_with(ui, s, host, "lpf", Size::M, Some(&lpf_pill));
    });
}

/// Where the detector's key comes from: any strip of the desk (Cantus), or the plug-in's
/// sidechain input, switched on (another DAW routes it).
fn key_source(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, w: f32) {
    let rev = host.revision();
    if st.key.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.key = Some((rev, host.key_source()));
    }
    match st.key.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips {
            current,
            label,
            options,
        }) => {
            let current = *current;
            let id = ui.id().with("gate-key");
            let at = controls::field(ui, s, id, label, frame::tr("gate.key_tip"), w);
            if let Some(at) = at {
                let mut items = vec![frame::radio(Src::Key("gate.key_own"), current.is_none())];
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
            let labels = [Src::Key("gate.key.own"), Src::Key("gate.key.sidechain")];
            if let Some(i) =
                controls::segmented(ui, s, on as usize, &labels, frame::tr("gate.key_tip"), w)
            {
                host.set_key_source(KeyTarget::Switch(i == 1));
            }
        }
        None => {}
    }
}

/// The inspector's picture: the transfer curve and the settings in one line.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Gate));
    let ink = Ink::of(&st);
    let gs = cantus_engine::fx::gate::settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let sq = Rect::from_min_size(rect.min, vec2(h, h)).shrink(space::S1);
    let x = |d: f32| sq.left() + ((d - LO_DB) / -LO_DB).clamp(0.0, 1.0) * sq.width();
    let y = |d: f32| sq.bottom() - ((d - LO_DB) / -LO_DB).clamp(0.0, 1.0) * sq.height();
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        b.segment(
            pos2(x(LO_DB), y(LO_DB)),
            pos2(x(0.0), y(0.0)),
            b.px(),
            ink.grid_strong,
            false,
        );
        let mut pts = [Pos2::ZERO; 81];
        for (i, p) in pts.iter_mut().enumerate() {
            let d = LO_DB * (1.0 - i as f32 / 80.0);
            *p = pos2(x(d), y(porta::transfer_db(d, &gs, false)));
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
                "{} {:.0} \u{b7} {} {:.0} dB \u{b7} {} \u{b7} {} \u{b7} {}",
                frame::tr("gate.open"),
                gs.threshold_db,
                frame::tr("gate.close"),
                gs.close_db(),
                Ratio(gs.ratio),
                Range(gs.range_db),
                frame::tr(STYLE_KEYS[gs.style.index().min(5)]),
            ),
        );
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("gate.open_tip"), "", Caps::NONE);
    }
    resp
}

/// The settings `--editor gate` and the screenshot tool open it with: a snare mic, its
/// key high-passed above the kick.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "gate").set_many(
        &[
            ("thr", -30.0),
            ("hyst", 6.0),
            ("range", 40.0),
            ("att", 0.5),
            ("hold", 40.0),
            ("rel", 160.0),
            ("look", 2.0),
            ("hpf", 150.0),
            ("lpf", 8_000.0),
        ],
        "cmd.fx_preset",
    );
}

/// A snare-like hit with its ring, over kick and hi-hat bleed: four seconds of it, run
/// through the real gate, as a playing engine would feed the editor.
pub fn demo_signal(seconds: f32, sr: f32) -> Vec<f32> {
    let n = (seconds * sr) as usize;
    let mut seed = 11u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let beat = (sr * 60.0 / 120.0) as usize;
    (0..n)
        .map(|i| {
            let tb = i % beat;
            let bar_beat = (i / beat) % 4;
            let t = tb as f32 / sr;
            // Snare on 2 and 4, a softer ghost before 4; kick bleed on 1 and 3.
            let snare = match bar_beat {
                1 | 3 => 0.6,
                _ => 0.0,
            };
            let ghost = if bar_beat == 2 && tb > beat / 2 {
                0.12
            } else {
                0.0
            };
            let tg = if ghost > 0.0 {
                (tb - beat / 2) as f32 / sr
            } else {
                t
            };
            let body = (snare + ghost)
                * (-tg * 18.0).exp()
                * ((std::f32::consts::TAU * 190.0 * tg).sin() * 0.6 + noise() * 0.5);
            let kick = if bar_beat.is_multiple_of(2) {
                0.02 * (-t * 12.0).exp() * (std::f32::consts::TAU * 55.0 * t).sin()
            } else {
                0.0
            };
            let hat = 0.006 * noise() * (-((i % (beat / 2)) as f32 / sr) * 30.0).exp();
            body + kick + hat
        })
        .collect()
}

/// What a playing engine would feed the editor, for screenshots with no engine.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(spec) = app.project().effect(strip, effect).map(|e| e.spec.clone()) else {
        return;
    };
    let sr = 48_000.0;
    let mut g = porta::Porta::new(sr);
    g.set(&cantus_engine::fx::gate::settings(&spec));
    let x = demo_signal(4.2, sr);
    let block = 480;
    let mut t = [0.0f32; 64];
    let st = &mut app.fx.gate;
    *st = State {
        effect: Some(effect),
        ..State::default()
    };
    // Hits are captured, as they would be with the button on.
    st.capturing = true;
    let mut pairs = Vec::with_capacity(x.len() * 2);
    // The song "plays" from bar 1 at 120 BPM, so bar lines run through the history.
    let tps = 120.0 / 60.0 * cantus_core::PPQ as f64;
    for (b, chunk) in x.chunks(block).enumerate() {
        let (mut l, mut r) = (chunk.to_vec(), chunk.to_vec());
        g.process(&mut l, &mut r, None);
        g.write_telemetry(&mut t);
        let now = ((b + 1) * block) as f64 / sr as f64 * tps;
        st.ingest(&t, Some(now as i64), tps);
        for (a, o) in chunk.iter().zip(&l) {
            pairs.push(*a);
            pairs.push(*o);
        }
    }
    app.fx.feed(&pairs);
    app.fx.injected = Some(t);
}
