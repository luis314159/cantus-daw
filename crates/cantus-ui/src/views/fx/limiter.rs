//! Terminus's editor. Owned by Terminus; nothing else edits this file.
//!
//! The benchmark (`research/fabfilter/pro-l.md`) wins on one honest picture — input,
//! output, gain reduction and loudness on one time axis, with the biggest reductions
//! labelled — and on delivery discipline. It loses on *time*: its picture forgets, cannot
//! be scrolled, frozen or asked, and knows nothing of the song. So here:
//!
//! - **The history is kept in song time.** While the transport plays, every 10 ms column
//!   is stored at its bar and beat; playing bars 33–48 again *replaces* bars 33–48. The
//!   **Song** view scrolls and zooms like the arrangement, with a ruler in bars; a peak
//!   label is a button that moves the playhead there. The **Live** view is the familiar
//!   scrolling picture, and it can be frozen.
//! - **Loudness is measured three ways at once:** since the last reset (as a meter does),
//!   over the whole song *as last heard* (each bar at its latest take — no plug-in can know
//!   this), and over the arrangement's time selection. All by BS.1770-4 / R128, through the
//!   same code the conformance tests check (`cantus_dsp::terminus::loudness`).
//! - **The display is the control:** drag the ceiling line, the target line, or the gain
//!   handle on the left edge (Alt drags the ceiling the other way, for level-matched
//!   listening). Gain reduction has a lane and a scale of its own, so it is never lost
//!   behind the waveform.
//! - **Delivery profiles** set the target and the ceiling together and say their rule in a
//!   sentence; the verdict is written, with a lamp or a warning shape, not only coloured.
//! - **Styles say what they are** (`style_words`), and the latency — which the benchmark
//!   hides — is always on screen (the footer's chip), with a warning when the attack is
//!   short enough to clip.
//!
//! The editor runs on any [`FxHost`] (ADR 0020): in Cantus the song view knows the
//! arrangement's time selection and a peak label moves the playhead; in a plug-in window
//! the history is stamped with what the host's transport says, and those two are not
//! offered.

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
use crate::kit::shell::Grid;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{well, with_brush, Brush, Fill, Function, State as KitState, Style};
use cantus_core::{Command, EffectId, EffectKind, EffectSpec, StripId, Ticks, PPQ};
use cantus_dsp::terminus::loudness::{energy_of, ColumnLoudness, Histogram};
use cantus_dsp::terminus::{
    latency_of, transfer_db, Column, LimSettings, LimStyle, Monitor, Terminus, DITHER_BITS,
    T_COLUMNS, T_GR_L, T_GR_R, T_LATENCY, T_MATCH, T_OUT_L, T_OUT_R, T_RATE, T_SEQ, T_WRAP,
};
use cantus_engine::fx::limiter::settings;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

/// Columns of live history kept: a minute.
const LIVE_MAX: usize = 6_000;
/// Song-time resolution: a 32nd of a quarter note (16 ms at 120 BPM).
const BUCKET: i64 = PPQ / 32;
/// The level scale: 0 dB at the top, expanded where limiting happens.
const LOW_DB: f32 = -36.0;
/// Where −12 dB sits, as a share of the level area: the top 12 dB get over half of it.
const KNEE_Y: f32 = 0.55;
/// The gain-reduction lane's scale, dB.
const GR_SPAN: f32 = 12.0;

/// One column of history, as the editor keeps it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Col {
    pub input_db: f32,
    pub output_db: f32,
    pub gr_db: f32,
    pub clip_db: f32,
    /// Momentary and short-term loudness here, LUFS (−∞ before they exist).
    pub momentary: f32,
    pub short_term: f32,
    /// Their energies, for measuring stretches of the song afterwards.
    pub m_energy: f64,
    pub s_energy: f64,
    /// Where the song was, when the transport was playing.
    pub tick: Option<i64>,
}

impl Default for Col {
    fn default() -> Self {
        Col {
            input_db: -160.0,
            output_db: -160.0,
            gr_db: 0.0,
            clip_db: 0.0,
            momentary: f32::NEG_INFINITY,
            short_term: f32::NEG_INFINITY,
            m_energy: 0.0,
            s_energy: 0.0,
            tick: None,
        }
    }
}

impl Col {
    /// Several columns as one pixel: the loudest peaks, the deepest reduction, the last
    /// loudness.
    fn merge(&mut self, o: &Col) {
        // A pixel's moment is its deepest reduction's: that is where a label points.
        if o.gr_db > self.gr_db {
            self.tick = o.tick.or(self.tick);
        }
        self.input_db = self.input_db.max(o.input_db);
        self.output_db = self.output_db.max(o.output_db);
        self.gr_db = self.gr_db.max(o.gr_db);
        self.clip_db = self.clip_db.max(o.clip_db);
        self.momentary = o.momentary;
        self.short_term = o.short_term;
        self.m_energy = o.m_energy;
        self.s_energy = o.s_energy;
        self.tick = self.tick.or(o.tick);
    }
}

/// Loudness over a stretch of the song.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub integrated: Option<f64>,
    pub range: Option<f64>,
    pub max_tp: f32,
    /// Seconds of song measured.
    pub seconds: f64,
    /// First and last bar measured (1-based).
    pub bars: (i64, i64),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum View {
    Live,
    Song,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    Ceiling,
    Target,
    Gain { alt: bool },
    Pan,
}

/// A run of pixels [x0, x1) and the history it shows.
#[derive(Copy, Clone, Debug)]
struct Strip {
    x0: f32,
    x1: f32,
    col: Col,
}

/// Words the editor formats only when what they say changes.
#[derive(Debug, Default)]
struct Words {
    /// The style's use and its engine in words: (style, language).
    style: Option<(usize, crate::i18n::Locale)>,
    style_use: String,
    style_engine: String,
    /// Where the engine's words break onto their second line (per box width).
    style_split: Option<(u32, usize)>,
    /// The verdict: what it was worked out from, whether it passes, its words, and where
    /// its second line starts (per box width).
    verdict: Option<(VerdictKey, bool)>,
    verdict_text: String,
    verdict_split: Option<(u32, usize)>,
    /// The profile's rule, and where its lines break (per box width).
    rule: Option<(Option<usize>, crate::i18n::Locale)>,
    rule_text: String,
    rule_split: Option<(u32, [usize; 2])>,
    /// "Dither: 16 bit", "Shaping: Flat": (dither, shape, language).
    fields: Option<(usize, usize, crate::i18n::Locale)>,
    dither: String,
    shape: String,
    /// "Trigger: Bus 2", per host revision and language.
    trigger: Option<(u64, crate::i18n::Locale)>,
    trigger_text: String,
    /// The key's source as the host said it, per host revision.
    key: Option<(u64, Option<KeySource>)>,
    /// The hover readout of the history: which pixel run, and its words.
    hover: Option<(u32, u32, crate::i18n::Locale)>,
    hover_text: String,
}

/// What a verdict depends on, rounded to what it prints.
type VerdictKey = (
    Option<i64>,
    i32,
    u32,
    u32,
    Option<usize>,
    crate::i18n::Locale,
);

/// What the editor remembers between frames.
#[derive(Debug)]
pub struct State {
    effect: Option<EffectId>,
    /// The telemetry counter already read.
    seen: Option<u32>,
    /// Live history, oldest first; `total` counts every column ever pushed.
    pub live: VecDeque<Col>,
    total: u64,
    /// Song-time history: a bucket of ticks → its latest column, and the pass it came in.
    pub song: BTreeMap<i64, (Col, u32)>,
    pass: u32,
    was_playing: bool,
    last_tick: i64,
    song_rev: u64,
    /// Loudness since the last reset.
    pub loud: Box<ColumnLoudness>,
    pub paused: bool,
    pub auto_reset: bool,
    pub max_tp: f32,
    pub max_gr: f32,
    pub view: View,
    /// Live view: seconds across.
    pub span_s: f32,
    /// Live view: frozen at this column count.
    pub frozen_at: Option<u64>,
    /// Song view: follow the playhead, or stay where the user put it.
    pub follow: bool,
    song_start: f64,
    song_len: f64,
    drag: Option<Grab>,
    drag_from: (f32, f32),
    /// Meter ballistics: output true peak and reduction per channel, dB, and the held peaks.
    meter_out: [f32; 2],
    meter_gr: [f32; 2],
    hold: [(f32, f64); 2],
    /// Scratch histograms for measuring the song and the selection, and their results.
    gate: Box<Histogram>,
    range: Box<Histogram>,
    song_sum: Option<(u64, Summary)>,
    sel_sum: Option<(u64, (i64, i64), Summary)>,
    /// Where the last frame put things, for tests that click and drag.
    pub geometry: Option<Geometry>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    words: Words,
    /// Scratch kept between frames, so drawing the history allocates nothing: the pixel
    /// runs, the peak candidates and those labelled, and the points of a line.
    strips: Vec<Strip>,
    cands: Vec<(usize, f32)>,
    peaks: Vec<(usize, f32)>,
    pts: Vec<Pos2>,
}

/// Where the display put its handles in the last frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Geometry {
    /// The level area.
    pub levels: Rect,
    pub ceiling_y: f32,
    pub target_y: f32,
    pub gain_handle: Rect,
    /// Each peak label, and where in the song it points (if it was heard in song time).
    pub labels: Vec<(Rect, Option<i64>)>,
}

impl Default for State {
    fn default() -> Self {
        State {
            effect: None,
            seen: None,
            live: VecDeque::new(),
            total: 0,
            song: BTreeMap::new(),
            pass: 0,
            was_playing: false,
            last_tick: 0,
            song_rev: 0,
            loud: Box::new(ColumnLoudness::new()),
            paused: false,
            auto_reset: false,
            max_tp: f32::NEG_INFINITY,
            max_gr: 0.0,
            view: View::Live,
            span_s: 6.0,
            frozen_at: None,
            follow: true,
            song_start: 0.0,
            song_len: (PPQ * 4 * 8) as f64,
            drag: None,
            drag_from: (0.0, 0.0),
            meter_out: [-160.0; 2],
            meter_gr: [0.0; 2],
            hold: [(-160.0, 0.0); 2],
            gate: Box::new(Histogram::new()),
            range: Box::new(Histogram::new()),
            song_sum: None,
            sel_sum: None,
            geometry: None,
            chrome: frame::Chrome::default(),
            chips: frame::ChipText::default(),
            words: Words::default(),
            strips: Vec::new(),
            cands: Vec::new(),
            peaks: Vec::new(),
            pts: Vec::new(),
        }
    }
}

impl State {
    /// Forget the loudness measurement and the held peaks (the history stays).
    pub fn reset_loudness(&mut self) {
        self.loud.reset();
        self.max_tp = f32::NEG_INFINITY;
        self.max_gr = 0.0;
    }

    /// One column from the limiter. `tick` is where the song was, if it was playing.
    pub fn push(&mut self, raw: &Column, tick: Option<i64>) {
        if !self.paused {
            self.loud.push(raw.energy as f64);
            self.max_tp = self.max_tp.max(raw.output_db);
            self.max_gr = self.max_gr.max(raw.gr_db);
        }
        let m = self.loud.momentary;
        let s = self.loud.short_term;
        let col = Col {
            input_db: raw.input_db,
            output_db: raw.output_db,
            gr_db: raw.gr_db,
            clip_db: raw.clip_db,
            momentary: m as f32,
            short_term: s as f32,
            m_energy: if m.is_finite() { energy_of(m) } else { 0.0 },
            s_energy: if s.is_finite() { energy_of(s) } else { 0.0 },
            tick,
        };
        self.live.push_back(col);
        self.total += 1;
        if self.live.len() > LIVE_MAX {
            self.live.pop_front();
        }
        if let Some(t) = tick {
            let bucket = t.div_euclid(BUCKET);
            let pass = self.pass;
            self.song
                .entry(bucket)
                .and_modify(|(c, p)| {
                    if *p == pass {
                        c.merge(&col);
                    } else {
                        *c = col;
                        *p = pass;
                    }
                })
                .or_insert((col, pass));
            self.song_rev += 1;
            self.last_tick = t;
        }
    }

    /// Feed columns directly — what the telemetry does each frame — for screenshots and
    /// tests. With `start_tick`, they are stored in song time from there on, one column
    /// every `ticks_per_col`, as one pass of the transport.
    pub fn feed(&mut self, cols: &[Column], start_tick: Option<i64>, ticks_per_col: f64) {
        if start_tick.is_some() {
            self.pass += 1;
        }
        for (i, c) in cols.iter().enumerate() {
            let tick = start_tick.map(|t| t + (i as f64 * ticks_per_col).round() as i64);
            self.push(c, tick);
        }
    }

    /// Read what the engine published since the last frame.
    pub fn ingest(&mut self, tel: &[f32], playing: bool, position: Ticks, bpm: f64) {
        let seq = tel[T_SEQ] as u32;
        let new = match self.seen {
            // The counter went back: a new stream (the unit was rebuilt or reset), unless
            // it wrapped, which takes two days of audio.
            Some(s) if seq < s && s - seq < T_WRAP / 2 => seq,
            Some(s) => (seq + T_WRAP - s) % T_WRAP,
            None => seq,
        }
        .min(T_COLUMNS as u32);
        if playing && !self.was_playing {
            self.pass += 1;
            if self.auto_reset {
                self.reset_loudness();
            }
        }
        if playing && position.0 + PPQ < self.last_tick {
            // The playhead jumped back: a new take of those bars.
            self.pass += 1;
        }
        self.was_playing = playing;
        let rate = if tel[T_RATE] > 0.0 {
            tel[T_RATE]
        } else {
            48_000.0
        } as f64;
        let ticks_per_s = PPQ as f64 * bpm / 60.0;
        let per_col = ticks_per_s * 0.01;
        let latency = tel[T_LATENCY] as f64 / rate * ticks_per_s;
        for j in 0..new {
            let index = (seq + T_WRAP - new + j) % T_WRAP;
            let raw = Column::read(tel, index);
            let tick = playing.then(|| {
                (position.0 as f64 - (new - 1 - j) as f64 * per_col - latency).round() as i64
            });
            self.push(&raw, tick);
        }
        self.seen = Some(seq);
    }

    /// Loudness of the song as last heard, between two ticks (`None`: all of it). Blocks
    /// are the momentary windows every 100 ms of song time; the range uses the short-term
    /// ones — BS.1770's and Tech 3342's own blocks, taken from the history.
    pub fn measure(&mut self, from: Option<i64>, to: Option<i64>, bpm: f64, bar: i64) -> Summary {
        self.gate.clear();
        self.range.clear();
        let secs_per_bucket = BUCKET as f64 / PPQ as f64 * 60.0 / bpm.max(1.0);
        let lo = from.map(|t| t.div_euclid(BUCKET)).unwrap_or(i64::MIN);
        let hi = to.map(|t| t.div_euclid(BUCKET)).unwrap_or(i64::MAX);
        let mut out = Summary {
            max_tp: f32::NEG_INFINITY,
            ..Summary::default()
        };
        let mut clock = 0.0;
        let mut first = None;
        let mut last = 0;
        for (&b, (c, _)) in self.song.range(lo..hi) {
            clock += secs_per_bucket;
            out.seconds += secs_per_bucket;
            if clock >= 0.1 {
                clock -= 0.1;
                if c.m_energy > 0.0 {
                    self.gate.add(c.m_energy);
                }
                if c.s_energy > 0.0 {
                    self.range.add(c.s_energy);
                }
            }
            out.max_tp = out.max_tp.max(c.output_db);
            first.get_or_insert(b);
            last = b;
        }
        out.integrated = self.gate.integrated();
        out.range = self.range.range();
        if let Some(f) = first {
            out.bars = (
                (f * BUCKET).div_euclid(bar) + 1,
                (last * BUCKET).div_euclid(bar) + 1,
            );
        }
        out
    }

    fn song_summary(&mut self, bpm: f64, bar: i64) -> Summary {
        if let Some((rev, s)) = self.song_sum {
            if rev == self.song_rev {
                return s;
            }
        }
        let s = self.measure(None, None, bpm, bar);
        self.song_sum = Some((self.song_rev, s));
        s
    }

    fn selection_summary(&mut self, sel: (i64, i64), bpm: f64, bar: i64) -> Summary {
        if let Some((rev, at, s)) = self.sel_sum {
            if rev == self.song_rev && at == sel {
                return s;
            }
        }
        let s = self.measure(Some(sel.0), Some(sel.1), bpm, bar);
        self.sel_sum = Some((self.song_rev, sel, s));
        s
    }

    /// The meters' ballistics for this frame: peaks fall at 20 dB/s, reduction at 30, and
    /// a held peak lets go after two seconds.
    fn ballistics(&mut self, tel: &[f32], now: f64, dt: f32) {
        let outs = [tel[T_OUT_L], tel[T_OUT_R]];
        let grs = [tel[T_GR_L], tel[T_GR_R]];
        for ch in 0..2 {
            self.meter_out[ch] = outs[ch].max(self.meter_out[ch] - 20.0 * dt);
            self.meter_gr[ch] = grs[ch].max(self.meter_gr[ch] - 30.0 * dt);
            if outs[ch] >= self.hold[ch].0 || now - self.hold[ch].1 > 2.0 {
                self.hold[ch] = (outs[ch], now);
            }
        }
    }

    /// The held peaks and maxima start again (a click on the meters).
    fn reset_holds(&mut self) {
        self.max_tp = f32::NEG_INFINITY;
        self.max_gr = 0.0;
        self.hold = [(-160.0, 0.0); 2];
    }
}

// ---- delivery profiles ------------------------------------------------------------------

/// A delivery target: loudness, the ceiling, how close counts, and whether louder masters
/// are only turned down (streaming) or rejected (broadcast).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Profile {
    pub key: &'static str,
    pub target: f32,
    pub ceiling: f32,
    pub tolerance: f32,
    pub streaming: bool,
}

/// The figures as the platforms publish them (checked September 2026): Spotify, YouTube and
/// Apple Music normalise playback (Apple's Sound Check to −16), EBU R128 asks −23 ±0.5 LU
/// for programmes, ATSC A/85 −24 ±2 LKFS with peaks at −2 dBTP.
pub const PROFILES: [Profile; 5] = [
    Profile {
        key: "lim.profile.spotify",
        target: -14.0,
        ceiling: -1.0,
        tolerance: 1.0,
        streaming: true,
    },
    Profile {
        key: "lim.profile.apple",
        target: -16.0,
        ceiling: -1.0,
        tolerance: 1.0,
        streaming: true,
    },
    Profile {
        key: "lim.profile.youtube",
        target: -14.0,
        ceiling: -1.0,
        tolerance: 1.0,
        streaming: true,
    },
    Profile {
        key: "lim.profile.ebu",
        target: -23.0,
        ceiling: -1.0,
        tolerance: 0.5,
        streaming: false,
    },
    Profile {
        key: "lim.profile.atsc",
        target: -24.0,
        ceiling: -2.0,
        tolerance: 2.0,
        streaming: false,
    },
];

/// Each profile's rule, in a sentence.
const RULE_KEYS: [&str; 5] = [
    "lim.profile.spotify_rule",
    "lim.profile.apple_rule",
    "lim.profile.youtube_rule",
    "lim.profile.ebu_rule",
    "lim.profile.atsc_rule",
];

/// The profile a target and ceiling are, if any (the first match: Spotify before YouTube).
pub fn profile_of(target: f32, ceiling: f32) -> Option<&'static Profile> {
    PROFILES
        .iter()
        .find(|p| (p.target - target).abs() < 0.05 && (p.ceiling - ceiling).abs() < 0.05)
}

/// The verdict on a measured loudness and true peak against a target, in words, and
/// whether it passes.
pub fn verdict(
    integrated: Option<f64>,
    max_tp: f32,
    target: f32,
    ceiling: f32,
    profile: Option<&Profile>,
) -> (bool, String) {
    let Some(i) = integrated else {
        return (false, t("lim.verdict.none"));
    };
    let tol = profile.map(|p| p.tolerance).unwrap_or(1.0) as f64;
    let streaming = profile.map(|p| p.streaming).unwrap_or(true);
    let diff = i - target as f64;
    let tp_ok = max_tp <= ceiling + 0.005;
    let tp_note = if tp_ok {
        String::new()
    } else {
        format!(
            " \u{b7} {}",
            tf(
                "lim.verdict.tp_over",
                &[("db", &format!("{:.1}", max_tp - ceiling))]
            )
        )
    };
    if diff.abs() <= tol {
        return (tp_ok, format!("{}{tp_note}", t("lim.verdict.on_target")));
    }
    let key = match (diff > 0.0, streaming) {
        (true, true) => "lim.verdict.turned_down",
        (true, false) => "lim.verdict.over",
        (false, true) => "lim.verdict.quiet",
        (false, false) => "lim.verdict.under",
    };
    (
        false,
        format!(
            "{}{tp_note}",
            tf(key, &[("lu", &format!("{:.1}", diff.abs()))])
        ),
    )
}

// ---- presets --------------------------------------------------------------------------

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "lim.preset.transparent",
        &[
            ("gain", 4.0),
            ("style", 0.0),
            ("look", 2.0),
            ("link_t", 0.75),
        ],
    ),
    (
        "lim.preset.streaming",
        &[
            ("gain", 6.0),
            ("style", 0.0),
            ("knee", 1.0),
            ("ceiling", -1.0),
            ("target", -14.0),
        ],
    ),
    (
        "lim.preset.loud",
        &[
            ("gain", 10.0),
            ("style", 2.0),
            ("look", 1.0),
            ("clip", 0.4),
            ("ceiling", -1.0),
            ("target", -9.0),
        ],
    ),
    (
        "lim.preset.drums",
        &[
            ("gain", 6.0),
            ("style", 1.0),
            ("look", 1.0),
            ("rel", 80.0),
            ("link_t", 0.5),
        ],
    ),
    (
        "lim.preset.glue",
        &[("gain", 4.0), ("style", 3.0), ("att", 5.0), ("rel", 300.0)],
    ),
    (
        "lim.preset.broadcast",
        &[
            ("gain", 2.0),
            ("style", 4.0),
            ("look", 5.0),
            ("rel", 500.0),
            ("target", -23.0),
        ],
    ),
    (
        "lim.preset.cd",
        &[
            ("gain", 6.0),
            ("ceiling", -0.3),
            ("dither", 5.0),
            ("shape", 2.0),
            ("target", -10.0),
        ],
    ),
    (
        "lim.preset.tracking",
        &[("look", 0.0), ("tp", 0.0), ("style", 4.0), ("rel", 100.0)],
    ),
];

// ---- words ----------------------------------------------------------------------------

const STYLE_KEYS: [&str; 5] = [
    "lim.style.0",
    "lim.style.1",
    "lim.style.2",
    "lim.style.3",
    "lim.style.4",
];

const STYLE_USE_KEYS: [&str; 5] = [
    "lim.style.0_use",
    "lim.style.1_use",
    "lim.style.2_use",
    "lim.style.3_use",
    "lim.style.4_use",
];

const MONITOR_KEYS: [&str; 3] = ["lim.monitor.0", "lim.monitor.1", "lim.monitor.2"];
const MONITOR_TIPS: [&str; 3] = [
    "lim.monitor.0_tip",
    "lim.monitor.1_tip",
    "lim.monitor.2_tip",
];
const SHAPE_KEYS: [&str; 3] = ["lim.shape.0", "lim.shape.1", "lim.shape.2"];

/// What a style's engine is, in words — the answer to "what does Loud do?".
pub fn style_words(style: LimStyle) -> String {
    let s = style.spec();
    let ramp = tf(
        if s.s_curve {
            "lim.words.s_curve"
        } else {
            "lim.words.straight"
        },
        &[("pct", &format!("{:.0}", s.ramp * 100.0))],
    );
    let transient = tf(
        "lim.words.transient",
        &[("ms", &format!("{}", s.transient_release_ms))],
    );
    let release = if s.attack_scale == 0.0 {
        t("lim.words.takes_all")
    } else if (s.attack_scale - 1.0).abs() > 1e-3 {
        tf(
            "lim.words.attack_scale",
            &[("x", &format!("{}", s.attack_scale))],
        )
    } else {
        t("lim.words.release_as_set")
    };
    let program = if s.program > 0.0 {
        tf("lim.words.slows", &[("x", &format!("{}", 1.0 + s.program))])
    } else if s.program < 0.0 {
        tf(
            "lim.words.quickens",
            &[("x", &format!("{}", 1.0 + s.program))],
        )
    } else {
        String::new()
    };
    let mut out = format!("{ramp} \u{b7} {transient} \u{b7} {release}");
    if !program.is_empty() {
        out.push_str(&format!(", {program}"));
    }
    out
}

/// A level with a real minus sign, one decimal: `+7.5`, `−1.0`, `−∞`.
struct Db(f32);

impl std::fmt::Display for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.0;
        if !v.is_finite() || v <= -150.0 {
            f.write_str("\u{2212}\u{221e}")
        } else if v < -0.049 {
            write!(f, "\u{2212}{:.1}", -v)
        } else {
            write!(f, "+{:.1}", v.abs())
        }
    }
}

/// A loudness without its unit, one decimal: `−11.2`, or a dash before there is one.
struct Lufs(f64);

impl std::fmt::Display for Lufs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let v = self.0;
        if !v.is_finite() {
            f.write_str("\u{2014}")
        } else if v < -0.049 {
            write!(f, "\u{2212}{:.1}", -v)
        } else {
            write!(f, "{v:.1}")
        }
    }
}

fn fmt_db(v: f32) -> String {
    Db(v).to_string()
}

fn fmt_lufs(v: f64) -> String {
    Lufs(v).to_string()
}

// ---- geometry -------------------------------------------------------------------------

/// y of a level in the level area: the top 12 dB take `KNEE_Y` of it.
fn y_of(db: f32, r: Rect) -> f32 {
    let d = db.clamp(LOW_DB, 0.0);
    let f = if d >= -12.0 {
        -d / 12.0 * KNEE_Y
    } else {
        KNEE_Y + (-12.0 - d) / (-12.0 - LOW_DB) * (1.0 - KNEE_Y)
    };
    r.top() + f * r.height()
}

fn db_at(y: f32, r: Rect) -> f32 {
    let f = ((y - r.top()) / r.height()).clamp(0.0, 1.0);
    if f <= KNEE_Y {
        -f / KNEE_Y * 12.0
    } else {
        -12.0 - (f - KNEE_Y) / (1.0 - KNEE_Y) * (-12.0 - LOW_DB)
    }
}

fn y_gr(gr: f32, r: Rect) -> f32 {
    r.top() + (gr / GR_SPAN).clamp(0.0, 1.0) * r.height()
}

/// The level lines, dB.
const LEVEL_TICKS: [f32; 7] = [-3.0, -6.0, -9.0, -12.0, -18.0, -24.0, -30.0];
/// The reduction lane's lines, dB.
const GR_TICKS: [f32; 3] = [3.0, 6.0, 9.0];

/// The history's and the meters' lanes in a display of `rect`: a head strip for the
/// selectors and numbers, the reduction lane, the levels, the time axis. Both displays
/// sit in one row, so their lanes line up and the target reads across.
#[derive(Copy, Clone, Debug)]
struct Lanes {
    head: Rect,
    gr: Rect,
    levels: Rect,
    axis: Rect,
}

fn lanes(rect: Rect, k: f32) -> Lanes {
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), 28.0 * k));
    let axis_h = 16.0 * k;
    let top = head.bottom();
    let gr_h = ((rect.bottom() - top - axis_h) * 0.2).round();
    let gr = Rect::from_min_max(pos2(rect.left(), top), pos2(rect.right(), top + gr_h));
    let levels = Rect::from_min_max(
        pos2(rect.left(), gr.bottom() + 1.0),
        pos2(rect.right(), rect.bottom() - axis_h),
    );
    let axis = Rect::from_min_max(pos2(rect.left(), levels.bottom()), rect.max);
    Lanes {
        head,
        gr,
        levels,
        axis,
    }
}

/// Where the song is, for the song view: the playhead, the arrangement's time selection
/// (Cantus only), the bar and beat, the tempo.
#[derive(Copy, Clone, Debug)]
struct SongCtx {
    playhead: f64,
    sel: Option<(i64, i64)>,
    bar: i64,
    beat: i64,
    bpm: f64,
    /// A peak label can move the playhead (Cantus owns the timeline; a plug-in does not).
    can_seek: bool,
}

fn song_ctx(host: &mut dyn FxHost, st: &State) -> SongCtx {
    let tr = host.transport();
    let bpm = tr.ticks_per_second / PPQ as f64 * 60.0;
    if let Some((app, _, _)) = host.daw() {
        let ts = app.project().time_signature;
        return SongCtx {
            playhead: app.engine.position().0 as f64,
            sel: app.time_selection.map(|(a, b)| (a.0, b.0)),
            bar: ts.ticks_per_bar().0.max(1),
            beat: ts.ticks_per_beat().0.max(1),
            bpm,
            can_seek: true,
        };
    }
    SongCtx {
        playhead: tr.playing_at.unwrap_or(st.last_tick) as f64,
        sel: None,
        bar: tr.ticks_per_bar.max(1),
        beat: (tr.ticks_per_bar / 4).max(1),
        bpm,
        can_seek: false,
    }
}

// ---- the frame ------------------------------------------------------------------------

/// The meters' display, the loudness card, the output card, at 100 %.
const METERS_W: f32 = 132.0;
const LOUD_W: f32 = 276.0;
const OUT_W: f32 = 448.0;
/// A boxed line of words.
const LINE: f32 = 16.0;
/// A label | value row, and its value's well.
const ROW_H: f32 = 18.0;
const WELL_W: f32 = 112.0;
/// The limiter's cells: the gain and the ceiling (larger: its two primary parameters, and the
/// ceiling's pill says dBTP), then the rest.
const LIM_CELLS: [Size; 9] = [
    Size::L,
    Size::L,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
];

thread_local! {
    /// The state [`framed`] leaves in the app while the editor borrows its own: made once
    /// and swapped in and out, so a frame builds no new state (its loudness meter and
    /// histograms are boxed).
    static SPARE: RefCell<Option<State>> = const { RefCell::new(None) };
    /// A value formatted for a well, reused.
    static FMT: RefCell<String> = RefCell::new(String::with_capacity(48));
}

/// Run `f` with `args` formatted into the reused buffer.
fn with_fmt<R>(args: std::fmt::Arguments<'_>, f: impl FnOnce(&str) -> R) -> R {
    FMT.with(|c| match c.try_borrow_mut() {
        Ok(mut buf) => {
            buf.clear();
            let _ = buf.write_fmt(args);
            f(&buf)
        }
        Err(_) => f(&std::fmt::format(args)),
    })
}

/// Terminus in Cantus's own window: the host is the project (every drag one undo step,
/// the trigger any strip of the desk, a peak label moves the playhead).
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let spare = SPARE.with(|c| c.borrow_mut().take()).unwrap_or_default();
    let mut st = std::mem::replace(&mut app.fx.limiter, spare);
    // Another limiter's editor was open before: its history is not this one's.
    if st.effect.is_some_and(|e| e != effect) {
        st = State::default();
    }
    st.effect = Some(effect);
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "limiter"),
        &mut st,
        ui,
    );
    let spare = std::mem::replace(&mut app.fx.limiter, st);
    SPARE.with(|c| *c.borrow_mut() = Some(spare));
    out
}

/// Terminus's editor on any host (ADR 0020): the device frame and the limiter inside it.
/// Everything it needs from the host goes through [`FxHost`].
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let transport = host.transport();
    let tel = host.telemetry();
    let bpm = transport.ticks_per_second / PPQ as f64 * 60.0;
    let at = transport.playing_at;
    st.ingest(&tel, at.is_some(), Ticks(at.unwrap_or(0)), bpm);
    let now = ui.input(|i| i.time);
    let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.1);
    st.ballistics(&tel, now, dt);
    let ls = settings(host.spec());
    let rate = if tel[T_RATE] > 0.0 {
        tel[T_RATE]
    } else {
        host.sample_rate()
    };
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency_of(&ls, rate) as f32, rate)];
    let meter = Some(frame::Levels {
        level: st.meter_out,
        hold: [st.hold[0].0, st.hold[1].0],
    });
    let look = frame::Look {
        kind: EffectKind::Limiter,
        chips: &chip,
        meter,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ls)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// A row of the grid as an array: no list is built per frame.
fn row_of<const N: usize>(g: &mut Grid, h: f32, fracs: &[f32; N]) -> [Rect; N] {
    let mut it = g.row(h, fracs);
    std::array::from_fn(|_| it.next().unwrap_or(Rect::NOTHING))
}

/// The limiter inside the frame: the history, the meters and the loudness on top; the
/// style and the output below them; the cells and the delivery at the bottom.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ls: &LimSettings,
) {
    let k = s.k;
    let song = song_ctx(host, st);
    let target = host.spec().get("target");
    if st.view == View::Song && st.follow {
        st.song_start = song.playhead - st.song_len * 0.85;
    }
    let so = s.function(Function::Output);
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row3 = controls::card_height(Size::L) * k;
    let row2 = (band::TITLE + 8.0 + controls::ROW + 5.0 + 3.0 * LINE + 8.0) * k;
    let row1 = (area.height() - row2 - row3 - 2.0 * gap).max(200.0 * k);

    // Row 1: the history, the meters beside it, the loudness.
    let w = area.width() - 2.0 * gap;
    let (mw, lw) = (METERS_W * k, LOUD_W * k);
    let [hist_r, met_r, loud_r] = row_of(&mut g, row1, &[(w - mw - lw) / w, mw / w, lw / w]);
    let disp = |r: Rect| (r.height() / k - band::TITLE - 1.0).max(20.0);
    let (_, _) = controls::card_with(
        ui,
        s,
        hist_r,
        Src::Key("lim.card.history"),
        None,
        Some(disp(hist_r)),
        |ui, cr| {
            if let Some(d) = cr.display {
                history(host, st, ui, s, ls, target, &song, tel, d);
            }
        },
    );
    let (_, _) = controls::card_with(
        ui,
        s,
        met_r,
        Src::Key("lim.card.meters"),
        None,
        Some(disp(met_r)),
        |ui, cr| {
            if let Some(d) = cr.display {
                meters(host, st, ui, s, ls, target, d);
            }
        },
    );
    let (_, _) = controls::card(
        ui,
        &so,
        loud_r,
        Src::Key("lim.card.loudness"),
        None,
        |ui, cr| loudness(st, ui, &so, cr.body, ls, target, &song),
    );

    // Row 2: the style and what it is; what goes out and what you hear.
    let w = area.width() - gap;
    let ow = OUT_W * k;
    let [style_r, out_r] = row_of(&mut g, row2, &[(w - ow) / w, ow / w]);
    style_card(host, st, ui, s, style_r, ls);
    output_card(host, st, ui, &so, out_r, ls, tel);

    // Row 3: the limiter's cells; the delivery target.
    let cw = (controls::cells_width(&LIM_CELLS) + 16.0) * k;
    let [lim_r, del_r] = row_of(&mut g, row3, &[cw / w, (w - cw) / w]);
    limiter_card(host, ui, s, lim_r, ls);
    delivery_card(host, st, ui, &so, del_r, ls, target);
}

/// The help line for whatever `r` covers: its name and what it does.
fn tip(ui: &egui::Ui, hovered: bool, name: &str, what: &str) {
    if hovered {
        hint::set_text(ui.ctx(), name, what, Caps::NONE);
    }
}

/// Where the second line of `text` starts, when its first must fit `width` points:
/// after the last space that fits, or the whole text when it fits or cannot break.
fn break_at(ctx: &egui::Context, text: &str, role: Role, k: f32, width: f32) -> usize {
    let fits = |s: &str| {
        let t = Text::new(s, role, Color32::PLACEHOLDER).caps(false);
        ktext::measure(ctx, &t, k).x <= width
    };
    if fits(text) {
        return text.len();
    }
    let mut best = None;
    for (i, _) in text.match_indices(' ') {
        if fits(&text[..i]) {
            best = Some(i + 1);
        } else {
            break;
        }
    }
    best.unwrap_or(text.len())
}

/// Lines of words in a box, left-aligned, each elided at the end if it still does not fit.
fn lines_in(b: &mut Brush, rect: Rect, lines: &[&str], color: Color32, k: f32) {
    let h = rect.height() / lines.len().max(1) as f32;
    for (i, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let r = Rect::from_min_size(
            pos2(rect.left(), rect.top() + h * i as f32),
            vec2(rect.width(), h),
        );
        let t = Text::new(line, Role::Body, color)
            .caps(false)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = ktext::paint(b, r, &t, k);
    }
}

// ---- the history ------------------------------------------------------------------------

/// The history as pixel runs, for either view, into `out`.
fn fill_strips(st: &State, r: Rect, out: &mut Vec<Strip>) {
    out.clear();
    match st.view {
        View::Live => {
            let n = (st.span_s * 100.0) as usize;
            let end = st.frozen_at.unwrap_or(st.total);
            let first_kept = st.total - st.live.len() as u64;
            let w = r.width();
            let px = w.floor().max(1.0) as usize;
            // Column index i (global) sits at x = right − (end − i)·w/n.
            let per_px = n as f32 / px as f32;
            let mut acc: Option<Strip> = None;
            let start = end.saturating_sub(n as u64).max(first_kept);
            for g in start..end {
                let Some(c) = st.live.get((g - first_kept) as usize) else {
                    continue;
                };
                let x = r.right() - (end - g) as f32 * w / n as f32;
                let (x0, x1) = (x, x + w / n as f32);
                match acc.as_mut() {
                    Some(a) if per_px > 1.0 && x0 < a.x0.floor() + 1.0 => {
                        a.col.merge(c);
                        a.x1 = x1;
                    }
                    _ => {
                        if let Some(a) = acc.take() {
                            out.push(a);
                        }
                        acc = Some(Strip { x0, x1, col: *c });
                    }
                }
            }
            out.extend(acc);
        }
        View::Song => {
            let (a, len) = (st.song_start, st.song_len.max(1.0));
            let lo = (a as i64).div_euclid(BUCKET) - 1;
            let hi = ((a + len) as i64).div_euclid(BUCKET) + 1;
            let bw = BUCKET as f32 / len as f32 * r.width();
            let mut acc: Option<Strip> = None;
            for (&b, (c, _)) in st.song.range(lo..=hi) {
                let x0 = r.left() + ((b * BUCKET) as f64 - a) as f32 / len as f32 * r.width();
                let x1 = x0 + bw;
                match acc.as_mut() {
                    // Buckets narrower than a pixel merge; a gap in the song stays a gap.
                    Some(s) if bw < 1.0 && x0 < s.x0.floor() + 1.0 && x0 <= s.x1 + 0.01 => {
                        s.col.merge(c);
                        s.x1 = x1;
                    }
                    _ => {
                        if let Some(s) = acc.take() {
                            out.push(s);
                        }
                        acc = Some(Strip { x0, x1, col: *c });
                    }
                }
            }
            out.extend(acc);
        }
    }
    out.retain(|s| s.x1 > r.left() && s.x0 < r.right());
}

/// The largest reductions, spaced apart: (strip index, reduction), into `out`.
fn peak_labels(
    strips: &[Strip],
    min_gap: f32,
    max: usize,
    cands: &mut Vec<(usize, f32)>,
    out: &mut Vec<(usize, f32)>,
) {
    cands.clear();
    out.clear();
    cands.extend(
        strips
            .iter()
            .enumerate()
            .filter(|(_, s)| s.col.gr_db >= 0.3)
            .map(|(i, s)| (i, s.col.gr_db)),
    );
    // In place (a stable sort would allocate); ties keep their order in time.
    cands.sort_unstable_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    for &(i, g) in cands.iter() {
        let x = strips[i].x0;
        if out
            .iter()
            .all(|(j, _)| (strips[*j].x0 - x).abs() >= min_gap)
        {
            out.push((i, g));
            if out.len() == max {
                break;
            }
        }
    }
}

/// A caps word and a number on plates, anchored at `at` by `align` (the pair reads left
/// to right): "CEILING −1.0 dBTP". Returns the rect of both, or nothing when it would
/// start left of `min_left`.
#[allow(clippy::too_many_arguments)]
fn tagged(
    b: &mut Brush,
    at: Pos2,
    align: Align2,
    word: &str,
    color: Color32,
    k: f32,
    plate: Option<Color32>,
    min_left: f32,
    args: std::fmt::Arguments<'_>,
) -> Rect {
    let wt = Text::new(word, Role::Micro, color).caps(true);
    let ws = ktext::measure(b.ctx(), &wt, k);
    let gap = 4.0 * k;
    // Where the whole pair goes: measure the number, then place both.
    let (vs, _) = with_fmt(args, |v| {
        let t = Text::new(v, Role::Value, color).caps(false);
        (ktext::measure(b.ctx(), &t, k), ())
    });
    let size = vec2(ws.x + gap + vs.x, ws.y.max(vs.y));
    let r = align.anchor_size(at, size);
    if r.left() < min_left {
        return Rect::NOTHING;
    }
    let w = canvas::put(
        b,
        pos2(r.left(), r.center().y),
        Align2::LEFT_CENTER,
        &wt,
        k,
        plate,
    );
    let v = canvas::putf(
        b,
        pos2(r.right(), r.center().y),
        Align2::RIGHT_CENTER,
        Role::Value,
        color,
        k,
        plate,
        args,
    );
    w.union(v)
}

/// The history: a head with the view's selectors, a gain-reduction lane, the levels
/// under it with the ceiling, the target and the gain strip, and time along the bottom.
#[allow(clippy::too_many_arguments)]
fn history(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    ls: &LimSettings,
    target: f32,
    song: &SongCtx,
    tel: &[f32; 64],
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let loud_c = s.t.hue(Function::Output);
    let ln = lanes(rect, k);
    let (gr_r, lv, axis) = (ln.gr, ln.levels, ln.axis);
    let plot = Rect::from_min_max(gr_r.min, rect.max);
    let resp = ui.interact(plot, ui.id().with("lim-history"), Sense::click_and_drag());

    let view = st.view;
    let (song_start, song_len) = (st.song_start, st.song_len.max(1.0));
    let x_of_tick = |tk_: f64| lv.left() + ((tk_ - song_start) / song_len) as f32 * lv.width();
    let tick_at = |x: f32| song_start + ((x - lv.left()) / lv.width()) as f64 * song_len;

    // The ceiling, the target and the gain handle, and which of them is under the pointer.
    let held = st.drag;
    let hover = resp.hover_pos();
    let cy = y_of(ls.ceiling_db, lv);
    let ty = y_of(target, lv);
    let handle = Rect::from_min_max(lv.left_top(), pos2(lv.left() + 40.0 * k, lv.bottom()));
    let near = |y: f32| {
        hover.is_some_and(|h| lv.contains(h) && (h.y - y).abs() < 5.0 * k && !handle.contains(h))
    };
    let ceiling_hot = held == Some(Grab::Ceiling) || (held.is_none() && near(cy));
    let target_hot = held == Some(Grab::Target) || (held.is_none() && !ceiling_hot && near(ty));
    let gain_hot = matches!(held, Some(Grab::Gain { .. }))
        || (held.is_none() && hover.is_some_and(|h| handle.contains(h)));

    let mut strips = std::mem::take(&mut st.strips);
    fill_strips(st, lv, &mut strips);
    let (mut cands, mut peaks) = (std::mem::take(&mut st.cands), std::mem::take(&mut st.peaks));
    peak_labels(&strips, 88.0 * k, 6, &mut cands, &mut peaks);
    let mut pts = std::mem::take(&mut st.pts);
    let mut geo = st.geometry.take().unwrap_or(Geometry {
        levels: lv,
        ceiling_y: cy,
        target_y: ty,
        gain_handle: handle,
        labels: Vec::new(),
    });
    geo.levels = lv;
    geo.ceiling_y = cy;
    geo.target_y = ty;
    geo.gain_handle = handle;
    geo.labels.clear();

    // The hover readout's words, when the pointer is over a new run.
    let hovered_strip = hover
        .filter(|h| {
            (lv.contains(*h) || gr_r.contains(*h)) && held.is_none() && !handle.contains(*h)
        })
        .and_then(|h| {
            strips
                .iter()
                .find(|sp| h.x >= sp.x0 && h.x < sp.x1.max(sp.x0 + 1.0))
                .map(|sp| (h, *sp))
        });
    if let Some((h, sp)) = hovered_strip {
        let lang = crate::i18n::locale();
        let key = (sp.x0.to_bits(), sp.x1.to_bits(), lang);
        if st.words.hover != Some(key) {
            let when = match sp.col.tick {
                Some(tick) => format!(
                    "{}.{}",
                    tick.div_euclid(song.bar) + 1,
                    tick.rem_euclid(song.bar) / song.beat + 1
                ),
                None => format!(
                    "\u{2212}{:.2} s",
                    (lv.right() - h.x) / lv.width() * st.span_s
                ),
            };
            let c = sp.col;
            // En spaces, so the readout keeps one face (a plain space starts a unit).
            st.words.hover_text = tf(
                "lim.hover",
                &[
                    ("when", &when),
                    ("inp", &fmt_db(c.input_db)),
                    ("out", &fmt_db(c.output_db)),
                    ("gr", &format!("{:.1}", c.gr_db)),
                    ("s", &fmt_lufs(c.short_term as f64)),
                ],
            )
            .replace(' ', "\u{2002}");
            st.words.hover = Some(key);
        }
    }

    let frozen = view == View::Live && st.frozen_at.is_some();
    let near_clip = ls.lookahead_ms * ls.style.spec().ramp < 0.1;
    let matching = ls.monitor == Monitor::Match;
    // The selectors' extent in the head, so the head's own words stay clear of them.
    let head_left = ln.head.left() + 6.0 * k;
    let selectors_right = head_left
        + k * match view {
            View::Live => 112.0 + 4.0 + 120.0 + 4.0 + 84.0,
            View::Song => 112.0 + 4.0 + 84.0 + 4.0 + 96.0 + 4.0 + 76.0,
        };
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        b.register(
            rect,
            "loudness",
            LOUD_TOKENS,
            &[Some(Function::Output.tok())],
        );
        let px = b.px();
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(ln.head.bottom()),
            px,
            ink.grid,
        );
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(gr_r.bottom() + 0.5),
            px,
            ink.grid_strong,
        );
        for g in LEVEL_TICKS {
            b.hline(lv.left(), lv.right(), b.snap_mid(y_of(g, lv)), px, ink.grid);
        }
        for g in GR_TICKS {
            b.hline(
                gr_r.left(),
                gr_r.right(),
                b.snap_mid(y_gr(g, gr_r)),
                px,
                ink.grid,
            );
        }

        // Time: the selection and the bars (song), or seconds ago (live).
        if view == View::Song {
            if let Some((a, bb)) = song.sel {
                let (x0, x1) = (x_of_tick(a as f64), x_of_tick(bb as f64));
                let sel = Rect::from_x_y_ranges(
                    x0.max(lv.left())..=x1.min(lv.right()),
                    gr_r.top()..=lv.bottom(),
                );
                if sel.width() > 0.0 {
                    b.rect(sel, Fill::Solid(Ink::a(s.t.selection, 0.12)));
                }
            }
            let px_per_bar = song.bar as f32 / song_len as f32 * lv.width();
            let every = (60.0 * k / px_per_bar.max(1.0)).ceil().max(1.0) as i64;
            let first = (song_start as i64).div_euclid(song.bar);
            let last = ((song_start + song_len) as i64).div_euclid(song.bar) + 1;
            for bar in first..=last {
                let x = x_of_tick((bar * song.bar) as f64);
                if x < lv.left() || x > lv.right() {
                    continue;
                }
                let strong = bar % every == 0;
                b.vline(
                    b.snap_mid(x),
                    gr_r.top(),
                    lv.bottom(),
                    px,
                    if strong { ink.grid_strong } else { ink.grid },
                );
                if px_per_bar > 80.0 * k {
                    for n in 1..(song.bar / song.beat) {
                        let xb = x_of_tick((bar * song.bar + n * song.beat) as f64);
                        b.vline(
                            b.snap_mid(xb),
                            lv.bottom(),
                            lv.bottom() + 3.0 * k,
                            px,
                            ink.grid_strong,
                        );
                    }
                }
            }
        } else {
            let span = st.span_s;
            let step = live_step(span);
            let mut sec = 0.0;
            while sec <= span {
                let x = lv.right() - sec / span * lv.width();
                b.vline(
                    b.snap_mid(x),
                    lv.bottom(),
                    lv.bottom() + 3.0 * k,
                    px,
                    ink.grid_strong,
                );
                sec += step;
            }
        }

        // The history: input behind, output over it in the device's hue, the reduction
        // hanging in its lane, the clipper's work along the top.
        let c_in = Ink::a(ink.dim, if ink.dark { 0.30 } else { 0.28 });
        let c_out = Ink::a(ink.hue, if ink.dark { 0.42 } else { 0.34 });
        let c_gr = Ink::a(ink.danger, if ink.dark { 0.70 } else { 0.55 });
        for sp in &strips {
            let (x0, x1) = (sp.x0.max(lv.left()), sp.x1.min(lv.right()));
            if x1 <= x0 {
                continue;
            }
            let c = &sp.col;
            b.rect(
                Rect::from_min_max(pos2(x0, y_of(c.input_db, lv)), pos2(x1, lv.bottom())),
                Fill::Solid(c_in),
            );
            b.rect(
                Rect::from_min_max(pos2(x0, y_of(c.output_db, lv)), pos2(x1, lv.bottom())),
                Fill::Solid(c_out),
            );
            if c.gr_db > 0.01 {
                b.rect(
                    Rect::from_min_max(pos2(x0, gr_r.top()), pos2(x1, y_gr(c.gr_db, gr_r))),
                    Fill::Solid(c_gr),
                );
            }
            if c.clip_db > 0.05 {
                let h = (2.0 + c.clip_db * 3.0).min(12.0) * k;
                b.rect(
                    Rect::from_min_max(pos2(x0, lv.top()), pos2(x1, lv.top() + h)),
                    Fill::Solid(s.t.meter_high),
                );
            }
        }
        // The reduction's edge and the short-term loudness, broken where the song has a
        // gap (and the loudness where it has none yet).
        for loudness in [false, true] {
            pts.clear();
            let mut last_x1 = f32::NEG_INFINITY;
            let (w, color) = if loudness {
                (1.5 * k, loud_c)
            } else {
                (1.0 * k, ink.danger)
            };
            for sp in &strips {
                let (x0, x1) = (sp.x0.max(lv.left()), sp.x1.min(lv.right()));
                if x1 <= x0 {
                    continue;
                }
                let c = &sp.col;
                let gap_here = sp.x0 > last_x1 + 1.5;
                last_x1 = sp.x1;
                let xm = (x0 + x1) * 0.5;
                let y = if loudness {
                    (c.short_term.is_finite() && c.short_term > LOW_DB)
                        .then(|| y_of(c.short_term, lv))
                } else {
                    Some(y_gr(c.gr_db, gr_r))
                };
                if gap_here || y.is_none() {
                    if pts.len() > 1 {
                        b.polyline(&pts, w, color);
                    }
                    pts.clear();
                }
                if let Some(y) = y {
                    pts.push(pos2(xm, y));
                }
            }
            if pts.len() > 1 {
                b.polyline(&pts, w, color);
            }
        }

        // The ceiling (the device's hue) and the target (the loudness's), dashed.
        canvas::dashed(
            b,
            pos2(lv.left(), cy),
            pos2(lv.right(), cy),
            if ceiling_hot { 2.0 } else { 1.25 } * k,
            ink.hue,
            6.0 * k,
            4.0 * k,
        );
        canvas::dashed(
            b,
            pos2(handle.right(), ty),
            pos2(lv.right(), ty),
            if target_hot { 2.0 } else { 1.25 } * k,
            loud_c,
            2.0 * k,
            3.0 * k,
        );
        // The gain handle: a strip on the left edge filled to the gain.
        let gtop = handle.bottom() - ls.gain_db / 30.0 * handle.height();
        b.rect(
            b.snap_rect(handle),
            Fill::Solid(Ink::a(s.t.deck_title, 0.88)),
        );
        b.rect(
            Rect::from_min_max(pos2(handle.left(), gtop), handle.max),
            Fill::Solid(Ink::a(ink.hue, if gain_hot { 0.55 } else { 0.35 })),
        );
        b.vline(
            b.snap_mid(handle.right()),
            handle.top(),
            handle.bottom(),
            px,
            ink.grid_strong,
        );
        // The playhead (song view).
        if view == View::Song {
            let x = x_of_tick(song.playhead);
            if x >= lv.left() && x <= lv.right() {
                b.vline(
                    b.snap_mid(x),
                    gr_r.top(),
                    lv.bottom(),
                    1.5 * k,
                    s.t.highlight,
                );
            }
        }
        if let Some((h, _)) = hovered_strip {
            b.vline(b.snap_mid(h.x), gr_r.top(), lv.bottom(), px, ink.dim);
        }

        // ---- the words, last: nothing is painted over them ----
        let plate = Some(ink.plate);
        let micro = |b: &mut Brush, at: Pos2, align: Align2, args: std::fmt::Arguments<'_>| {
            canvas::putf(b, at, align, Role::Micro, ink.dim, tk, plate, args)
        };
        for g in LEVEL_TICKS {
            micro(
                b,
                pos2(lv.right() - 4.0 * k, y_of(g, lv)),
                Align2::RIGHT_CENTER,
                format_args!("\u{2212}{:.0}", -g),
            );
        }
        for g in GR_TICKS {
            micro(
                b,
                pos2(gr_r.right() - 4.0 * k, y_gr(g, gr_r)),
                Align2::RIGHT_CENTER,
                format_args!("\u{2212}{g:.0}"),
            );
        }
        let lane = canvas::put(
            b,
            pos2(gr_r.right() - 26.0 * k, gr_r.top() + 3.0 * k),
            Align2::RIGHT_TOP,
            &Text::key("lim.gr_lane", Role::Micro, ink.dim),
            tk,
            plate,
        );
        let unit = if ls.true_peak { "dBTP" } else { "dB" };
        // Above its line, or under it when the line is too near the lane.
        let (cy_at, c_align) = if cy - lv.top() > 20.0 * k {
            (cy - 3.0 * k, Align2::RIGHT_BOTTOM)
        } else {
            (cy + 3.0 * k, Align2::RIGHT_TOP)
        };
        tagged(
            b,
            pos2(lv.right() - 28.0 * k, cy_at),
            c_align,
            frame::tr("fx.lim.ceiling"),
            ink.hue,
            tk,
            plate,
            handle.right() + 4.0 * k,
            format_args!("{} {unit}", Db(ls.ceiling_db)),
        );
        let (ty_at, t_align) = if lv.bottom() - ty > 20.0 * k {
            (ty + 3.0 * k, Align2::LEFT_TOP)
        } else {
            (ty - 3.0 * k, Align2::LEFT_BOTTOM)
        };
        tagged(
            b,
            pos2(handle.right() + 8.0 * k, ty_at),
            t_align,
            frame::tr("fx.lim.target"),
            loud_c,
            tk,
            plate,
            f32::NEG_INFINITY,
            format_args!("{} LUFS", Lufs(target as f64)),
        );
        canvas::putf(
            b,
            pos2(handle.center().x, (gtop - 3.0 * k).max(lv.top() + 16.0 * k)),
            Align2::CENTER_BOTTOM,
            Role::Value,
            ink.text,
            tk,
            plate,
            format_args!("+{:.1}", ls.gain_db),
        );
        let caption = Rect::from_min_max(
            pos2(handle.left() + 1.0 * k, handle.bottom() - 14.0 * k),
            pos2(handle.right() - 1.0 * k, handle.bottom() - 2.0 * k),
        );
        let t = Text::key("fx.lim.gain", Role::Micro, ink.dim)
            .caps(true)
            .policy(Policy::Abbreviate);
        let _ = ktext::paint(b, caption, &t, tk);

        // The time axis: bars (song) or seconds ago (live), none running into another.
        let mut ruler = canvas::Ruler::new();
        if view == View::Song {
            let px_per_bar = song.bar as f32 / song_len as f32 * lv.width();
            let every = (60.0 * k / px_per_bar.max(1.0)).ceil().max(1.0) as i64;
            let first = (song_start as i64).div_euclid(song.bar);
            let last = ((song_start + song_len) as i64).div_euclid(song.bar) + 1;
            for bar in first..=last {
                let x = x_of_tick((bar * song.bar) as f64);
                if x < lv.left() || x > lv.right() - 12.0 * k || bar % every != 0 {
                    continue;
                }
                let at = pos2(x + 2.0 * k, axis.center().y);
                if !ruler.fits(at.x, 6.0 * k) {
                    continue;
                }
                let r = micro(b, at, Align2::LEFT_CENTER, format_args!("{}", bar + 1));
                ruler.placed(r);
            }
        } else {
            let span = st.span_s;
            let step = live_step(span);
            let mut sec = (span / step).floor() * step;
            while sec >= step {
                let x = lv.right() - sec / span * lv.width();
                let at = pos2(x + 2.0 * k, axis.center().y);
                if x >= lv.left() && ruler.fits(at.x, 6.0 * k) {
                    let r = micro(
                        b,
                        at,
                        Align2::LEFT_CENTER,
                        format_args!("\u{2212}{sec:.0} s"),
                    );
                    ruler.placed(r);
                }
                sec -= step;
            }
            let now_t = Text::key("lim.now", Role::Micro, ink.dim);
            let w = ktext::measure(b.ctx(), &now_t, tk).x;
            if ruler.fits(lv.right() - 2.0 * k - w, 6.0 * k) {
                canvas::put(
                    b,
                    pos2(lv.right() - 2.0 * k, axis.center().y),
                    Align2::RIGHT_CENTER,
                    &now_t,
                    tk,
                    None,
                );
            }
        }

        // Peak labels: the largest reductions, as tags you click to go there — kept in
        // the lane, clear of its scale and its name, and of each other.
        let lane_x1 = gr_r.right() - 26.0 * k;
        for &(i, g) in &peaks {
            let sp = strips[i];
            with_fmt(format_args!("\u{2212}{g:.1} dB"), |text| {
                let t = Text::new(text, Role::Value, ink.text).caps(false);
                let size = ktext::measure(b.ctx(), &t, tk) + vec2(8.0, 4.0) * k;
                let x = ((sp.x0 + sp.x1) * 0.5)
                    .clamp(gr_r.left() + 4.0 * k + size.x * 0.5, lane_x1 - size.x * 0.5);
                let y = (y_gr(g, gr_r) + 3.0 * k)
                    .min(gr_r.bottom() - 2.0 * k - size.y)
                    .max(gr_r.top() + 2.0 * k);
                let tag = b.snap_rect(Rect::from_min_size(pos2(x - size.x * 0.5, y), size));
                if tag.intersects(lane.expand(2.0 * k))
                    || geo.labels.iter().any(|(r, _)| r.intersects(tag))
                {
                    return;
                }
                let hot = song.can_seek
                    && sp.col.tick.is_some()
                    && hover.is_some_and(|h| tag.contains(h));
                let fill = if hot {
                    Ink::a(ink.danger, 0.30)
                } else {
                    ink.plate
                };
                b.rrect(tag, 3.0 * k, Fill::Solid(fill));
                b.rrect_stroke(tag, 3.0 * k, px, ink.danger);
                let _ = ktext::paint(b, tag, &t.policy(Policy::Elide(Elide::End)), tk);
                geo.labels.push((tag, sp.col.tick));
            });
        }

        // The head's right end: a warning, the match gain, a frozen picture.
        let mut x = ln.head.right() - 8.0 * k;
        let cy_head = ln.head.center().y;
        if near_clip {
            let t = Text::key("lim.near_clip", Role::Label, ink.danger).caps(false);
            let w = ktext::measure(b.ctx(), &t, tk).x;
            if x - w > selectors_right + 8.0 * k {
                let r = canvas::put(b, pos2(x, cy_head), Align2::RIGHT_CENTER, &t, tk, plate);
                x = r.left() - 10.0 * k;
            }
        }
        if matching {
            let r = tagged(
                b,
                pos2(x, cy_head),
                Align2::RIGHT_CENTER,
                frame::tr("lim.monitor.2"),
                ink.text,
                tk,
                plate,
                selectors_right + 8.0 * k,
                format_args!("{} dB", Db(tel[T_MATCH])),
            );
            if r.is_positive() {
                x = r.left() - 10.0 * k;
            }
        }
        if frozen {
            let t = Text::key("lim.frozen", Role::Label, ink.hue).caps(false);
            let w = ktext::measure(b.ctx(), &t, tk).x;
            if x - w > selectors_right + 8.0 * k {
                canvas::put(b, pos2(x, cy_head), Align2::RIGHT_CENTER, &t, tk, plate);
            }
        }

        // Empty: what to do.
        if strips.is_empty() {
            let key = if view == View::Song {
                "lim.empty_song"
            } else {
                "lim.empty_live"
            };
            canvas::put(
                b,
                lv.center(),
                Align2::CENTER_CENTER,
                &Text::key(key, Role::Body, ink.dim).caps(false),
                tk,
                None,
            );
        }

        // Hover: what was there, at that moment — last, over everything.
        if let Some((h, _)) = hovered_strip {
            let t = Text::new(&st.words.hover_text, Role::Value, ink.text).caps(false);
            let w = ktext::measure(b.ctx(), &t, tk).x;
            let at = pos2(
                (h.x + 8.0 * k)
                    .min(lv.right() - w - 6.0 * k)
                    .max(lv.left() + 4.0 * k),
                lv.top() + 4.0 * k,
            );
            canvas::put(b, at, Align2::LEFT_TOP, &t, tk, Some(s.t.deck_title));
        }
    });

    // The head: which view, and its span or follow, freeze and the song's keys.
    head_controls(st, ui, s, ln.head, head_left);

    // ---- gestures ----
    let alt = ui.input(|i| i.modifiers.alt);
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(h) = origin.or(resp.interact_pointer_pos()) {
            st.drag_from = (ls.gain_db, ls.ceiling_db);
            st.drag = if handle.contains(h) {
                Some(Grab::Gain { alt })
            } else if (h.y - cy).abs() < 6.0 * k {
                Some(Grab::Ceiling)
            } else if (h.y - ty).abs() < 6.0 * k {
                Some(Grab::Target)
            } else if st.view == View::Song {
                Some(Grab::Pan)
            } else {
                None
            };
        }
    }
    if resp.dragged() {
        match (st.drag, resp.interact_pointer_pos()) {
            (Some(Grab::Ceiling), Some(h)) => {
                let v = (db_at(h.y, lv) * 10.0).round() / 10.0;
                host.set("ceiling", v.clamp(-30.0, 0.0), Some("ceiling"));
            }
            (Some(Grab::Target), Some(h)) => {
                let v = (db_at(h.y, lv) * 10.0).round() / 10.0;
                host.set("target", v.clamp(-36.0, -5.0), Some("target"));
            }
            (Some(Grab::Gain { alt }), Some(_)) => {
                let fine = ui.input(|i| i.modifiers.shift);
                let dy = resp.drag_delta().y;
                let per_px = 30.0 / handle.height() * if fine { 0.2 } else { 1.0 };
                let g = (ls.gain_db - dy * per_px).clamp(0.0, 30.0);
                host.set("gain", g, Some("lim-gain"));
                if alt {
                    // Alt: the ceiling moves the other way, so what you hear stays level.
                    let (g0, c0) = st.drag_from;
                    host.set(
                        "ceiling",
                        (c0 - (g - g0)).clamp(-30.0, 0.0),
                        Some("lim-gain"),
                    );
                }
            }
            (Some(Grab::Pan), _) => {
                st.follow = false;
                st.song_start -= resp.drag_delta().x as f64 / lv.width() as f64 * st.song_len;
            }
            _ => {}
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.clicked() && song.can_seek {
        if let Some(h) = resp.interact_pointer_pos() {
            if let Some((_, Some(tick))) = geo.labels.iter().find(|(r, _)| r.contains(h)) {
                if let Some((app, _, _)) = host.daw() {
                    app.seek(Ticks(*tick));
                }
            }
        }
    }
    if resp.double_clicked() {
        match st.view {
            View::Song => st.follow = true,
            View::Live => st.frozen_at = None,
        }
    }
    // Wheel: in the song view, scroll; with Ctrl, zoom about the pointer.
    if resp.hovered() && view == View::Song {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        if zoom != 1.0 {
            let anchor = hover
                .map(|h| tick_at(h.x))
                .unwrap_or(st.song_start + st.song_len * 0.5);
            let len = (st.song_len / zoom as f64).clamp(PPQ as f64, PPQ as f64 * 4.0 * 512.0);
            st.song_start = anchor - (anchor - st.song_start) * len / st.song_len;
            st.song_len = len;
            st.follow = false;
        } else if scroll.x != 0.0 || scroll.y != 0.0 {
            let d = if scroll.x != 0.0 { scroll.x } else { scroll.y };
            st.song_start -= d as f64 / lv.width() as f64 * st.song_len;
            st.follow = false;
        }
    }
    if ceiling_hot || target_hot || gain_hot {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    } else if song.can_seek
        && geo
            .labels
            .iter()
            .any(|(r, t)| t.is_some() && hover.is_some_and(|h| r.contains(h)))
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("lim.card.history"),
            frame::tr("lim.display_tip"),
            Caps::NONE,
        );
    }
    st.geometry = Some(geo);
    st.strips = strips;
    st.cands = cands;
    st.peaks = peaks;
    st.pts = pts;
}

/// The tokens the history's loudness and clip marks paint with, beside the display's.
const LOUD_TOKENS: &[crate::kit::Tok] = &[
    crate::kit::Tok::MeterHigh,
    crate::kit::Tok::Selection,
    crate::kit::Tok::Highlight,
    crate::kit::Tok::DeckTitle,
];

/// The step of the live view's seconds.
fn live_step(span: f32) -> f32 {
    if span <= 6.0 {
        1.0
    } else if span <= 20.0 {
        5.0
    } else {
        10.0
    }
}

/// The spans of the live view.
const SPANS: [f32; 3] = [6.0, 20.0, 60.0];

/// The head of the history: Live | Song, then the live view's span and freeze, or the
/// song view's follow, fit and forget.
fn head_controls(st: &mut State, ui: &mut egui::Ui, s: &Style, head: Rect, left: f32) {
    let k = s.k;
    let r = Rect::from_min_max(
        pos2(left, head.top()),
        pos2(head.right() - 6.0 * k, head.bottom()),
    );
    let mut h = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(r)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    h.spacing_mut().item_spacing.x = s.d(4.0);
    let views = [Src::Key("lim.view.live"), Src::Key("lim.view.song")];
    let now = (st.view == View::Song) as usize;
    let r =
        h.scope(|ui| controls::segmented(ui, s, now, &views, frame::tr("lim.view.live"), 112.0));
    if let Some(i) = r.inner {
        st.view = if i == 0 { View::Live } else { View::Song };
    }
    let tip_key = if st.view == View::Song {
        "lim.view.song_tip"
    } else {
        "lim.view.live_tip"
    };
    tip(
        &h,
        r.response.contains_pointer(),
        frame::tr("lim.card.history"),
        frame::tr(tip_key),
    );
    match st.view {
        View::Live => {
            let spans = [Src::Str("6 s"), Src::Str("20 s"), Src::Str("60 s")];
            let now = SPANS.iter().position(|v| *v == st.span_s).unwrap_or(0);
            if let Some(i) =
                controls::segmented(&mut h, s, now, &spans, frame::tr("lim.span"), 120.0)
            {
                st.span_s = SPANS[i];
            }
            let frozen = st.frozen_at.is_some();
            let r = controls::led(
                &mut h,
                s,
                frozen,
                Src::Key("lim.freeze"),
                frame::tr("lim.freeze"),
                84.0,
            );
            if r.clicked() {
                st.frozen_at = if frozen { None } else { Some(st.total) };
            }
            tip(
                &h,
                r.hovered(),
                frame::tr("lim.freeze"),
                frame::tr("lim.freeze_tip"),
            );
        }
        View::Song => {
            let r = controls::led(
                &mut h,
                s,
                st.follow,
                Src::Key("lim.follow"),
                frame::tr("lim.follow"),
                84.0,
            );
            if r.clicked() {
                st.follow = !st.follow;
            }
            tip(
                &h,
                r.hovered(),
                frame::tr("lim.follow"),
                frame::tr("lim.follow_tip"),
            );
            let r = controls::key(
                &mut h,
                s,
                Src::Key("lim.whole_song"),
                frame::tr("lim.whole_song"),
                96.0,
                false,
            );
            if r.clicked() {
                if let (Some((&a, _)), Some((&b, _))) =
                    (st.song.first_key_value(), st.song.last_key_value())
                {
                    st.follow = false;
                    st.song_start = (a * BUCKET) as f64;
                    st.song_len = (((b - a + 1) * BUCKET) as f64).max(PPQ as f64 * 4.0);
                }
            }
            tip(
                &h,
                r.hovered(),
                frame::tr("lim.whole_song"),
                frame::tr("lim.whole_song_tip"),
            );
            let r = controls::key(
                &mut h,
                s,
                Src::Key("lim.forget_song"),
                frame::tr("lim.forget_song"),
                76.0,
                false,
            );
            if r.clicked() {
                st.song.clear();
                st.song_rev += 1;
            }
            tip(
                &h,
                r.hovered(),
                frame::tr("lim.forget_song"),
                frame::tr("lim.forget_song_tip"),
            );
        }
    }
}

// ---- the meters -------------------------------------------------------------------------

/// Meters beside the history, on its lanes: reduction per channel in the lane, output
/// true peak per channel and the three loudness bars on the level scale, with the target.
/// Click to reset the held peaks; drag the target.
#[allow(clippy::too_many_arguments)]
fn meters(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    ls: &LimSettings,
    target: f32,
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let loud_c = s.t.hue(Function::Output);
    let ln = lanes(rect, k);
    let (gr_r, lv, axis) = (ln.gr, ln.levels, ln.axis);
    let plot = Rect::from_min_max(gr_r.min, rect.max);
    let resp = ui.interact(plot, ui.id().with("lim-meters"), Sense::click_and_drag());
    let pad = 4.0 * k;
    let bw = ((rect.width() - pad * 9.0) / 6.0).max(1.0);
    let col_x =
        |i: usize| rect.left() + pad + i as f32 * (bw + pad) + if i >= 2 { pad * 2.0 } else { 0.0 };
    let ty = y_of(target, lv);
    let cy = y_of(ls.ceiling_db, lv);
    let loud_x = col_x(3) - 2.0 * k;
    let on_target = |h: Pos2| h.x >= loud_x && (h.y - ty).abs() < 6.0 * k;
    let near_target = resp.hover_pos().is_some_and(on_target);
    let tp_over = st.max_tp > ls.ceiling_db + 0.005;
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        b.register(
            rect,
            "meters",
            METER_TOKENS,
            &[Some(Function::Output.tok())],
        );
        let px = b.px();
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(ln.head.bottom()),
            px,
            ink.grid,
        );
        let lane = Ink::a(ink.grid, 1.0);
        // Reduction: L R, hanging from the top.
        for ch in 0..2 {
            let x = col_x(ch);
            let bar = b.snap_rect(Rect::from_min_max(
                pos2(x, gr_r.top() + 4.0 * k),
                pos2(x + bw, gr_r.bottom() - 2.0 * k),
            ));
            b.rect(bar, Fill::Solid(lane));
            let y = bar.top() + (st.meter_gr[ch] / GR_SPAN).clamp(0.0, 1.0) * bar.height();
            b.rect(
                Rect::from_min_max(bar.min, pos2(bar.right(), y)),
                Fill::Solid(ink.danger),
            );
        }
        // Output true peak: L R, with the held peak.
        for ch in 0..2 {
            let x = col_x(ch);
            let bar = b.snap_rect(Rect::from_min_max(
                pos2(x, lv.top()),
                pos2(x + bw, lv.bottom()),
            ));
            b.rect(bar, Fill::Solid(lane));
            let v = st.meter_out[ch];
            let color = if v > ls.ceiling_db + 0.005 {
                s.t.meter_clip
            } else if v > -6.0 {
                s.t.meter_high
            } else {
                s.t.meter_low
            };
            if v > LOW_DB {
                b.rect(
                    Rect::from_min_max(pos2(bar.left(), y_of(v, lv)), bar.max),
                    Fill::Solid(color),
                );
            }
            if st.hold[ch].0 > LOW_DB {
                let hy = y_of(st.hold[ch].0, lv);
                b.rect(
                    Rect::from_min_max(
                        pos2(bar.left(), hy - 1.0 * k),
                        pos2(bar.right(), hy + 1.0 * k),
                    ),
                    Fill::Solid(ink.text),
                );
            }
        }
        // Loudness: momentary, short-term, integrated; over the target hatched as well
        // as coloured.
        let loud = &st.loud;
        let vals = [
            loud.momentary,
            loud.short_term,
            loud.integrated().unwrap_or(f64::NEG_INFINITY),
        ];
        for (i, v) in vals.iter().enumerate() {
            let x = col_x(3 + i);
            let bar = b.snap_rect(Rect::from_min_max(
                pos2(x, lv.top()),
                pos2(x + bw, lv.bottom()),
            ));
            b.rect(bar, Fill::Solid(lane));
            if v.is_finite() && (*v as f32) > LOW_DB {
                let v = *v as f32;
                b.rect(
                    Rect::from_min_max(pos2(bar.left(), y_of(v.min(target), lv)), bar.max),
                    Fill::Solid(loud_c),
                );
                if v > target {
                    let over = Rect::from_min_max(
                        pos2(bar.left(), y_of(v, lv)),
                        pos2(bar.right(), y_of(target, lv)),
                    );
                    b.rect(over, Fill::Solid(Ink::a(ink.danger, 0.35)));
                    canvas::hatch(b, over, 4.0 * k, px, ink.danger);
                }
            }
        }
        // The target across the loudness bars (drag it); the ceiling across the output.
        b.hline(
            loud_x,
            col_x(5) + bw + 2.0 * k,
            ty,
            if near_target || st.drag == Some(Grab::Target) {
                3.0
            } else {
                2.0
            } * k,
            loud_c,
        );
        b.hline(
            col_x(0) - 2.0 * k,
            col_x(1) + bw + 2.0 * k,
            cy,
            1.0 * k,
            ink.hue,
        );
        // The words: the deepest reduction and the highest true peak held, the lanes'
        // names along the bottom.
        let plate = Some(ink.plate);
        tagged(
            b,
            pos2(rect.left() + 4.0 * k, ln.head.center().y),
            Align2::LEFT_CENTER,
            frame::tr("lim.gr_short"),
            ink.danger,
            tk,
            None,
            f32::NEG_INFINITY,
            format_args!("\u{2212}{:.1}", st.max_gr),
        );
        let _ = canvas::putf(
            b,
            pos2(rect.right() - 4.0 * k, ln.head.center().y),
            Align2::RIGHT_CENTER,
            Role::Value,
            if tp_over { ink.danger } else { ink.text },
            tk,
            plate,
            format_args!("{}", Db(st.max_tp)),
        );
        for (i, name) in ["L", "R", "", "M", "S", "I"].iter().enumerate() {
            if name.is_empty() {
                continue;
            }
            let x = col_x(i);
            let t = Text::new(name, Role::Micro, ink.dim).caps(true);
            canvas::put(
                b,
                pos2(x + bw * 0.5, axis.center().y),
                Align2::CENTER_CENTER,
                &t,
                tk,
                None,
            );
        }
    });
    if near_target || st.drag == Some(Grab::Target) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let pressed_on_target = ui
        .input(|i| i.pointer.press_origin())
        .is_some_and(on_target);
    if resp.drag_started() && pressed_on_target {
        st.drag = Some(Grab::Target);
    }
    if resp.dragged() && st.drag == Some(Grab::Target) {
        if let Some(h) = resp.interact_pointer_pos() {
            let v = (db_at(h.y, lv) * 10.0).round() / 10.0;
            host.set("target", v.clamp(-36.0, -5.0), Some("target"));
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.clicked() {
        st.reset_holds();
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("lim.card.meters"),
            frame::tr("lim.meters_tip"),
            Caps::NONE,
        );
    }
}

/// The tokens the meters paint with, beside the display's.
const METER_TOKENS: &[crate::kit::Tok] = &[
    crate::kit::Tok::MeterLow,
    crate::kit::Tok::MeterHigh,
    crate::kit::Tok::MeterClip,
];

// ---- the loudness card ------------------------------------------------------------------

/// A label | value row: the name at the left in caps (abbreviated when it must be), the
/// value in a well at the right, in `role`. Returns the row's response.
#[allow(clippy::too_many_arguments)]
fn value_row(
    ui: &mut egui::Ui,
    s: &Style,
    w: f32,
    h: f32,
    label: Src<'_>,
    role: Role,
    color: Color32,
    value: std::fmt::Arguments<'_>,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(w * s.k, h * s.k), Sense::hover());
    if ui.is_rect_visible(rect) {
        with_brush(ui.painter(), |b| {
            let well_r = Rect::from_min_max(
                pos2(
                    rect.right() - (WELL_W * s.k).min(rect.width() * 0.5),
                    rect.top(),
                ),
                rect.max,
            );
            label_left(b, s, rect, well_r.left(), label);
            well::paint(b, well_r, s, KitState::REST, None);
            with_fmt(value, |v| {
                let t = Text::new(v, role, color)
                    .caps(false)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, well_r.shrink2(vec2(6.0 * s.k, 0.0)), &t, s.tk());
            });
        });
    }
    resp
}

/// A row's name, left, up to `right` (less a gap).
fn label_left(b: &mut Brush, s: &Style, rect: Rect, right: f32, label: Src<'_>) {
    let lr = Rect::from_min_max(rect.min, pos2(right - 6.0 * s.k, rect.bottom()));
    let t = Text {
        src: label,
        role: Role::Label,
        caps: true,
        align: Align2::LEFT_CENTER,
        policy: Policy::Abbreviate,
        color: s.t.label,
    };
    let _ = ktext::paint(b, lr, &t, s.tk());
}

/// `2.0 LU`, or a dash.
struct Lu(Option<f64>);

impl std::fmt::Display for Lu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(v) => write!(f, "{v:.1} LU"),
            None => f.write_str("\u{2014}"),
        }
    }
}

/// The loudness since the reset: the integrated number and its verdict, the other
/// readings, reset / pause / on play, and the song and the selection as measured from
/// the history.
fn loudness(
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    ls: &LimSettings,
    target: f32,
    song: &SongCtx,
) {
    let k = s.k;
    let w = body.width() / k;
    let profile = PROFILES
        .iter()
        .position(|p| (p.target - target).abs() < 0.05 && (p.ceiling - ls.ceiling_db).abs() < 0.05);
    let integrated = st.loud.integrated();
    // The verdict's words, when what it says changes.
    let lang = crate::i18n::locale();
    let key: VerdictKey = (
        integrated.map(|v| (v * 20.0).round() as i64),
        (st.max_tp.max(-1_000.0) * 200.0).round() as i32,
        target.to_bits(),
        ls.ceiling_db.to_bits(),
        profile,
        lang,
    );
    if st.words.verdict.map(|v| v.0) != Some(key) {
        let (pass, text) = verdict(
            integrated,
            st.max_tp,
            target,
            ls.ceiling_db,
            profile.map(|i| &PROFILES[i]),
        );
        st.words.verdict = Some((key, pass));
        st.words.verdict_text = text;
        st.words.verdict_split = None;
    }
    let pass = st.words.verdict.is_some_and(|v| v.1);
    controls::column(ui, s, w, |ui| {
        ui.spacing_mut().item_spacing.y = s.d(2.0);
        let r = value_row(
            ui,
            s,
            w,
            28.0,
            Src::Key("lim.integrated"),
            Role::Hero,
            s.t.text,
            format_args!("{} LUFS", Lufs(integrated.unwrap_or(f64::NEG_INFINITY))),
        );
        tip(
            ui,
            r.hovered(),
            frame::tr("lim.integrated"),
            frame::tr("lim.integrated_tip"),
        );

        // The verdict: a lamp when it passes, a warning shape when it does not — never
        // the colour alone — and its words on up to two lines.
        // One line when the words fit on one: the rows below keep their room.
        let words_w = w * k - 20.0 * k;
        let ww = words_w.round() as u32;
        if st.words.verdict_split.is_none_or(|(at, _)| at != ww) {
            let split = break_at(
                ui.ctx(),
                &st.words.verdict_text,
                Role::Body,
                s.tk(),
                words_w,
            );
            st.words.verdict_split = Some((ww, split));
        }
        let split = st.words.verdict_split.map_or(0, |v| v.1);
        let text = &st.words.verdict_text;
        let lines = if split >= text.len() { 1.0 } else { 2.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(w * k, lines * LINE * k), Sense::hover());
        let words = Rect::from_min_max(pos2(rect.left() + 20.0 * k, rect.top()), rect.max);
        let (l1, l2) = text.split_at(split.min(text.len()));
        with_brush(ui.painter(), |b| {
            let c = pos2(rect.left() + 8.0 * k, rect.top() + LINE * 0.5 * k);
            match (integrated.is_some(), pass) {
                (true, true) => band::lamp(b, c, s, true, s.t.meter_low, 1.0),
                (false, _) => band::lamp(b, c, s, false, s.t.meter_low, 1.0),
                (true, false) => {
                    let r = 5.5 * k;
                    let tri = [
                        pos2(c.x, c.y - r),
                        pos2(c.x + r, c.y + r * 0.8),
                        pos2(c.x - r, c.y + r * 0.8),
                        pos2(c.x, c.y - r),
                    ];
                    b.polyline(&tri, 1.5 * k, s.t.danger);
                }
            }
            if l2.is_empty() {
                lines_in(b, words, &[l1.trim_end()], s.t.text, s.tk());
            } else {
                lines_in(b, words, &[l1.trim_end(), l2], s.t.text, s.tk());
            }
        });
        let name = profile.map_or(frame::tr("lim.profile.custom"), |i| {
            frame::tr(PROFILES[i].key)
        });
        tip(ui, resp.hovered(), name, &st.words.verdict_text);

        let tp_over = st.max_tp > ls.ceiling_db + 0.005;
        let loud = &st.loud;
        let r = value_row(
            ui,
            s,
            w,
            ROW_H,
            Src::Key("lim.short_term"),
            Role::Value,
            s.t.text,
            format_args!("{} LUFS", Lufs(loud.short_term)),
        );
        tip(
            ui,
            r.hovered(),
            frame::tr("lim.short_term"),
            frame::tr("lim.short_term_tip"),
        );
        let r = value_row(
            ui,
            s,
            w,
            ROW_H,
            Src::Key("lim.momentary"),
            Role::Value,
            s.t.text,
            format_args!("{} LUFS", Lufs(loud.momentary)),
        );
        tip(
            ui,
            r.hovered(),
            frame::tr("lim.momentary"),
            frame::tr("lim.momentary_tip"),
        );
        let r = value_row(
            ui,
            s,
            w,
            ROW_H,
            Src::Key("lim.range"),
            Role::Value,
            s.t.text,
            format_args!("{}", Lu(loud.range())),
        );
        tip(
            ui,
            r.hovered(),
            frame::tr("lim.range"),
            frame::tr("lim.range_tip"),
        );
        let r = value_row(
            ui,
            s,
            w,
            ROW_H,
            Src::Key("lim.true_peak"),
            Role::Value,
            if tp_over { s.t.danger } else { s.t.text },
            format_args!("{} dBTP", Db(st.max_tp)),
        );
        tip(
            ui,
            r.hovered(),
            frame::tr("lim.true_peak"),
            frame::tr("lim.true_peak_tip"),
        );
        let r = value_row(
            ui,
            s,
            w,
            ROW_H,
            Src::Key("lim.max_ms"),
            Role::Value,
            s.t.text,
            // Thin spaces: the pair keeps one face (a plain space starts a unit).
            format_args!(
                "{}\u{2009}/\u{2009}{}",
                Lufs(loud.max_momentary),
                Lufs(loud.max_short_term)
            ),
        );
        tip(
            ui,
            r.hovered(),
            frame::tr("lim.max_ms"),
            frame::tr("lim.max_ms_tip"),
        );

        // Reset, pause, and reset on play.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = s.d(4.0);
            // The two keys take less than the LED, whose words are longer.
            let third = (w - 8.0) * 0.28;
            let led_w = w - 8.0 - 2.0 * third;
            let r = controls::key(
                ui,
                s,
                Src::Key("lim.reset"),
                frame::tr("lim.reset"),
                third,
                false,
            );
            if r.clicked() {
                st.reset_loudness();
            }
            tip(
                ui,
                r.hovered(),
                frame::tr("lim.reset"),
                frame::tr("lim.reset_tip"),
            );
            let pause = if st.paused { "lim.resume" } else { "lim.pause" };
            let r = controls::key(ui, s, Src::Key(pause), frame::tr(pause), third, st.paused);
            if r.clicked() {
                st.paused = !st.paused;
            }
            tip(
                ui,
                r.hovered(),
                frame::tr(pause),
                frame::tr("lim.pause_tip"),
            );
            let r = controls::led(
                ui,
                s,
                st.auto_reset,
                Src::Key("lim.auto_reset"),
                frame::tr("lim.auto_reset"),
                led_w,
            );
            if r.clicked() {
                st.auto_reset = !st.auto_reset;
            }
            tip(
                ui,
                r.hovered(),
                frame::tr("lim.auto_reset"),
                frame::tr("lim.auto_reset_tip"),
            );
        });

        // The song as last heard, and the time selection — from the history in song time.
        if !st.song.is_empty() {
            let ceiling = ls.ceiling_db;
            let sum = st.song_summary(song.bpm, song.bar);
            summary_rows(ui, s, w, "lim.song", &sum, ceiling);
            if let Some(sel) = song.sel {
                let sum = st.selection_summary(sel, song.bpm, song.bar);
                summary_rows(ui, s, w, "lim.selection", &sum, ceiling);
            }
        }
    });
}

/// A stretch of the song, measured: its name and integrated loudness, then its bars, its
/// range and its true peak.
fn summary_rows(
    ui: &mut egui::Ui,
    s: &Style,
    w: f32,
    key: &'static str,
    sum: &Summary,
    ceiling: f32,
) {
    let k = s.k;
    let r = value_row(
        ui,
        s,
        w,
        ROW_H,
        Src::Key(key),
        Role::Value,
        s.t.text,
        format_args!("{} LUFS", Lufs(sum.integrated.unwrap_or(f64::NEG_INFINITY))),
    );
    tip(ui, r.hovered(), frame::tr(key), frame::tr("lim.song_tip"));
    let (rect, resp) = ui.allocate_exact_size(vec2(w * k, ROW_H * k), Sense::hover());
    if ui.is_rect_visible(rect) {
        with_brush(ui.painter(), |b| {
            let tp_w = (WELL_W * k).min(rect.width() * 0.5);
            let lra_w = 64.0 * k;
            let tp_r = Rect::from_min_max(pos2(rect.right() - tp_w, rect.top()), rect.max);
            let lra_r = Rect::from_min_max(
                pos2(tp_r.left() - 4.0 * k - lra_w, rect.top()),
                pos2(tp_r.left() - 4.0 * k, rect.bottom()),
            );
            // "Bars 1–12", dim: it names the rows above and beside.
            with_fmt(
                format_args!(
                    "{} {}\u{2013}{}",
                    frame::tr("lim.bars"),
                    sum.bars.0,
                    sum.bars.1
                ),
                |v| {
                    let lr =
                        Rect::from_min_max(rect.min, pos2(lra_r.left() - 6.0 * k, rect.bottom()));
                    let t = Text::new(v, Role::Label, s.t.text_dim)
                        .caps(true)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::End));
                    let _ = ktext::paint(b, lr, &t, s.tk());
                },
            );
            let value = |b: &mut Brush, r: Rect, color: Color32, args: std::fmt::Arguments<'_>| {
                well::paint(b, r, s, KitState::REST, None);
                with_fmt(args, |v| {
                    let t = Text::new(v, Role::Value, color)
                        .caps(false)
                        .align(Align2::RIGHT_CENTER)
                        .policy(Policy::Elide(Elide::End));
                    let _ = ktext::paint(b, r.shrink2(vec2(6.0 * k, 0.0)), &t, s.tk());
                });
            };
            value(b, lra_r, s.t.text, format_args!("{}", Lu(sum.range)));
            let tp_c = if sum.max_tp > ceiling + 0.005 {
                s.t.danger
            } else {
                s.t.text
            };
            value(b, tp_r, tp_c, format_args!("{} dBTP", Db(sum.max_tp)));
        });
    }
    tip(
        ui,
        resp.hovered(),
        frame::tr(key),
        frame::tr("lim.song_tip"),
    );
}

// ---- the cards --------------------------------------------------------------------------

/// The styles, as one segmented control, and what the chosen one is for and does, in
/// two boxed lines (the whole of each on the help line).
fn style_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ls: &LimSettings,
) {
    let lang = crate::i18n::locale();
    let idx = ls.style.index().min(STYLE_KEYS.len() - 1);
    if st.words.style != Some((idx, lang)) {
        st.words.style_use = t(STYLE_USE_KEYS[idx]);
        st.words.style_engine = style_words(ls.style);
        st.words.style = Some((idx, lang));
        st.words.style_split = None;
    }
    let _ = controls::card(ui, s, rect, Src::Key("fx.lim.style"), None, |ui, cr| {
        let w = cr.body.width() / s.k;
        controls::column(ui, s, w, |ui| {
            let labels = STYLE_KEYS.map(Src::Key);
            let r = ui
                .scope(|ui| controls::segmented(ui, s, idx, &labels, frame::tr("fx.lim.style"), w));
            if let Some(i) = r.inner {
                host.set("style", i as f32, None);
            }
            tip(
                ui,
                r.response.contains_pointer(),
                frame::tr(STYLE_KEYS[idx]),
                &st.words.style_use,
            );
            ui.spacing_mut().item_spacing.y = 0.0;
            // What the style is for, then what its engine does, on up to two lines.
            let name = frame::tr(STYLE_KEYS[idx]);
            let (lr, resp) = ui.allocate_exact_size(vec2(w * s.k, s.s(LINE)), Sense::hover());
            with_brush(ui.painter(), |b| {
                lines_in(b, lr, &[st.words.style_use.as_str()], s.t.text, s.tk())
            });
            tip(ui, resp.hovered(), name, &st.words.style_use);
            let (lr, resp) = ui.allocate_exact_size(vec2(w * s.k, s.s(2.0 * LINE)), Sense::hover());
            let ww = lr.width().round() as u32;
            let text = &st.words.style_engine;
            if st.words.style_split.is_none_or(|(at, _)| at != ww) {
                let a = break_at(ui.ctx(), text, Role::Body, s.tk(), lr.width());
                st.words.style_split = Some((ww, a));
            }
            let a = st.words.style_split.map_or(0, |v| v.1).min(text.len());
            with_brush(ui.painter(), |b| {
                lines_in(
                    b,
                    lr,
                    &[text[..a].trim_end(), &text[a..]],
                    s.t.text_dim,
                    s.tk(),
                )
            });
            tip(ui, resp.hovered(), name, text);
        });
    });
}

/// What goes out and what you hear: true peak, the DC filter, Δ, the monitor; dither and
/// its shaping; what triggers the limiting.
#[allow(clippy::too_many_arguments)]
fn output_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ls: &LimSettings,
    tel: &[f32; 64],
) {
    let lang = crate::i18n::locale();
    let shape = host.spec().get("shape").round().clamp(0.0, 2.0) as usize;
    let bits = |i: usize| {
        if i == 0 {
            t("lim.off")
        } else {
            tf("lim.bits", &[("n", &DITHER_BITS[i])])
        }
    };
    if st.words.fields != Some((ls.dither, shape, lang)) {
        st.words.dither = tf("lim.dither_field", &[("v", &bits(ls.dither))]);
        st.words.shape = tf("lim.shape_field", &[("v", &t(SHAPE_KEYS[shape]))]);
        st.words.fields = Some((ls.dither, shape, lang));
    }
    let rev = host.revision();
    if st.words.key.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.words.key = Some((rev, host.key_source()));
    }
    if st.words.trigger != Some((rev, lang)) {
        let src = match st.words.key.as_ref().and_then(|(_, k)| k.as_ref()) {
            Some(KeySource::Strips {
                current: Some(_),
                label,
                ..
            }) => label.clone(),
            _ => t("lim.trigger_own"),
        };
        st.words.trigger_text = tf("lim.trigger_field", &[("src", &src)]);
        st.words.trigger = Some((rev, lang));
    }
    let _ = controls::card(ui, s, rect, Src::Key("lim.card.output"), None, |ui, cr| {
        let w = cr.body.width() / s.k;
        controls::column(ui, s, w, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = s.d(4.0);
                for (key, label, width, tip_key) in [
                    ("tp", "fx.lim.tp", 96.0, "fx.lim.tp_tip"),
                    ("dc", "fx.lim.dc", 96.0, "fx.lim.dc_tip"),
                    ("delta", "fx.lim.delta", 44.0, "fx.lim.delta_tip"),
                ] {
                    let r = controls::led_param(ui, s, host, key, Src::Key(label), width);
                    tip(ui, r.hovered(), frame::tr(label), frame::tr(tip_key));
                }
                let labels = MONITOR_KEYS.map(Src::Key);
                let mw = (w - 96.0 - 96.0 - 44.0 - 12.0).max(60.0);
                let r =
                    ui.scope(|ui| controls::segmented_param(ui, s, host, "monitor", &labels, mw));
                let now = (host.spec().get("monitor").round().clamp(0.0, 2.0)) as usize;
                if r.response.contains_pointer() {
                    // The match gain, when matching, goes with what the mode does.
                    if ls.monitor == Monitor::Match {
                        hint::set(
                            ui.ctx(),
                            frame::tr("fx.lim.monitor"),
                            Caps::NONE,
                            false,
                            |b| {
                                write!(
                                    b,
                                    "{} dB  \u{b7}  {}",
                                    Db(tel[T_MATCH]),
                                    frame::tr(MONITOR_TIPS[2])
                                )
                            },
                        );
                    } else {
                        tip(
                            ui,
                            true,
                            frame::tr("fx.lim.monitor"),
                            frame::tr(MONITOR_TIPS[now]),
                        );
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = s.d(4.0);
                let (dw, sw) = (120.0, 132.0);
                let tw = (w - dw - sw - 8.0).max(60.0);
                // Dither: off, or a word length.
                let id = ui.id().with("lim-dither");
                let r = ui.scope(|ui| {
                    controls::field(ui, s, id, &st.words.dither, frame::tr("fx.lim.dither"), dw)
                });
                if let Some(at) = r.inner {
                    let names: Vec<String> = (0..DITHER_BITS.len()).map(bits).collect();
                    let items: Vec<_> = names
                        .iter()
                        .enumerate()
                        .map(|(i, n)| frame::radio(Src::Str(n), ls.dither == i))
                        .collect();
                    if let Some(i) = controls::list(ui, s, id, at, dw, &items) {
                        host.set("dither", i as f32, None);
                    }
                }
                tip(
                    ui,
                    r.response.contains_pointer(),
                    frame::tr("fx.lim.dither"),
                    frame::tr("fx.lim.dither_tip"),
                );
                // Its shaping, when it dithers.
                ui.add_enabled_ui(ls.dither > 0, |ui| {
                    let id = ui.id().with("lim-shape");
                    let r = ui.scope(|ui| {
                        controls::field(ui, s, id, &st.words.shape, frame::tr("fx.lim.shape"), sw)
                    });
                    if let Some(at) = r.inner {
                        let items: [_; 3] = std::array::from_fn(|i| {
                            frame::radio(Src::Key(SHAPE_KEYS[i]), shape == i)
                        });
                        if let Some(i) = controls::list(ui, s, id, at, sw.max(180.0), &items) {
                            host.set("shape", i as f32, None);
                        }
                    }
                    tip(
                        ui,
                        r.response.contains_pointer(),
                        frame::tr("fx.lim.shape"),
                        frame::tr(SHAPE_TIPS[shape]),
                    );
                });
                trigger(host, st, ui, s, tw);
            });
        });
    });
}

const SHAPE_TIPS: [&str; 3] = ["lim.shape.0_tip", "lim.shape.1_tip", "lim.shape.2_tip"];

/// What triggers the limiting: any strip of the desk (Cantus), or the plug-in's
/// sidechain input, switched on (another DAW routes it).
fn trigger(host: &mut dyn FxHost, st: &State, ui: &mut egui::Ui, s: &Style, w: f32) {
    let words = &st.words;
    match words.key.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips {
            current, options, ..
        }) => {
            let current = *current;
            let id = ui.id().with("lim-trigger");
            let r = ui.scope(|ui| {
                controls::field(ui, s, id, &words.trigger_text, frame::tr("lim.trigger"), w)
            });
            if let Some(at) = r.inner {
                let mut items = vec![frame::radio(Src::Key("lim.trigger_own"), current.is_none())];
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
            tip(
                ui,
                r.response.contains_pointer(),
                frame::tr("lim.trigger"),
                frame::tr("lim.trigger_tip"),
            );
        }
        Some(KeySource::Switch(on)) => {
            let labels = [
                Src::Key("lim.trigger_own"),
                Src::Key("export.sidechain.external"),
            ];
            let r = ui.scope(|ui| {
                controls::segmented(ui, s, *on as usize, &labels, frame::tr("lim.trigger"), w)
            });
            if let Some(i) = r.inner {
                host.set_key_source(KeyTarget::Switch(i == 1));
            }
            tip(
                ui,
                r.response.contains_pointer(),
                frame::tr("lim.trigger"),
                frame::tr("lim.trigger_tip"),
            );
        }
        None => {}
    }
}

/// How a cell writes its value into its pill, when not in the parameter's own unit.
type Pill<'a> = Option<&'a dyn Fn(f32, &mut String)>;

/// A cell whose help line also says what the parameter does.
fn cell(
    ui: &mut egui::Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &'static str,
    size: Size,
    format: Pill<'_>,
) {
    let r = controls::param_with(ui, s, host, key, size, format);
    if !(r.hovered() && !r.dragged()) {
        return;
    }
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let v = spec.get(key);
    let what = frame::tr(tip_of(key));
    hint::set(
        ui.ctx(),
        frame::tr(info.label_key),
        Caps::PARAM,
        false,
        |b| {
            match format {
                Some(f) => f(v, b),
                None => controls::write_readout(v, info.unit, b),
            }
            b.push_str("  \u{b7}  ");
            b.push_str(what);
            Ok(())
        },
    );
}

/// What each cell's parameter does, for the help line.
fn tip_of(key: &str) -> &'static str {
    match key {
        "gain" => "fx.lim.gain_tip",
        "ceiling" => "fx.lim.ceiling_tip",
        "look" => "fx.lim.look_tip",
        "att" => "fx.lim.att_tip",
        "rel" => "fx.lim.rel_tip",
        "link_t" => "fx.lim.link_t_tip",
        "link_r" => "fx.lim.link_r_tip",
        "knee" => "fx.lim.knee_tip",
        "clip" => "fx.lim.clip_tip",
        _ => "fx.lim.target_tip",
    }
}

fn gain_pill(v: f32, b: &mut String) {
    let _ = write!(b, "+{v:.1} dB");
}

fn ceiling_pill(v: f32, tp: bool, b: &mut String) {
    let _ = write!(b, "{} {}", Db(v), if tp { "dBTP" } else { "dB" });
}

fn look_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.2} ms");
}

fn knee_pill(v: f32, b: &mut String) {
    if v < 0.05 {
        b.push_str(frame::tr("lim.hard"));
    } else {
        let _ = write!(b, "{v:.1} dB");
    }
}

fn clip_pill(v: f32, b: &mut String) {
    if v <= 0.0 {
        b.push_str(frame::tr("lim.off"));
    } else {
        let _ = write!(b, "{:.0}%", v * 100.0);
    }
}

fn target_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{} LUFS", Lufs(v as f64));
}

/// The limiter: the gain it drives with (the primary parameter, larger), the ceiling,
/// the two stages' timing, the knee and the clipper, and each stage's stereo link.
fn limiter_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, ls: &LimSettings) {
    let tp = ls.true_peak;
    let _ = controls::card(ui, s, rect, Src::Key("lim.card.limiter"), None, |ui, _| {
        cell(ui, s, host, "gain", LIM_CELLS[0], Some(&gain_pill));
        let ceiling = move |v: f32, b: &mut String| ceiling_pill(v, tp, b);
        cell(ui, s, host, "ceiling", LIM_CELLS[1], Some(&ceiling));
        cell(ui, s, host, "look", Size::M, Some(&look_pill));
        cell(ui, s, host, "att", Size::M, None);
        cell(ui, s, host, "rel", Size::M, None);
        cell(ui, s, host, "knee", Size::M, Some(&knee_pill));
        cell(ui, s, host, "clip", Size::M, Some(&clip_pill));
        cell(ui, s, host, "link_t", Size::M, None);
        cell(ui, s, host, "link_r", Size::M, None);
    });
}

/// The delivery: the loudness target (a cell), the profile that sets target and ceiling
/// together (a menu field), and the profile's rule in a sentence.
#[allow(clippy::too_many_arguments)]
fn delivery_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    ls: &LimSettings,
    target: f32,
) {
    let k = s.k;
    let lang = crate::i18n::locale();
    let profile = PROFILES
        .iter()
        .position(|p| (p.target - target).abs() < 0.05 && (p.ceiling - ls.ceiling_db).abs() < 0.05);
    if st.words.rule != Some((profile, lang)) {
        st.words.rule_text = t(profile.map_or("lim.profile.custom_rule", |i| RULE_KEYS[i]));
        st.words.rule = Some((profile, lang));
        st.words.rule_split = None;
    }
    let _ = controls::card(
        ui,
        s,
        rect,
        Src::Key("lim.card.delivery"),
        None,
        |ui, cr| {
            cell(ui, s, host, "target", Size::L, Some(&target_pill));
            let w = (cr.body.width() / k - crate::kit::cell::size(Size::L).x - 4.0).max(60.0);
            controls::column(ui, s, w, |ui| {
                ui.spacing_mut().item_spacing.y = s.d(3.0);
                let id = ui.id().with("lim-profile");
                let name = profile.map_or(frame::tr("lim.profile.custom"), |i| {
                    frame::tr(PROFILES[i].key)
                });
                let r = ui
                    .scope(|ui| controls::field(ui, s, id, name, frame::tr("cmd.lim_profile"), w));
                if let Some(at) = r.inner {
                    let names: Vec<String> = PROFILES
                        .iter()
                        .map(|p| {
                            format!(
                                "{}  {} LUFS \u{b7} {} dBTP",
                                t(p.key),
                                Lufs(p.target as f64),
                                Db(p.ceiling)
                            )
                        })
                        .collect();
                    let items: Vec<_> = names
                        .iter()
                        .enumerate()
                        .map(|(i, n)| frame::radio(Src::Str(n), profile == Some(i)))
                        .collect();
                    if let Some(i) = controls::list(ui, s, id, at, w.max(260.0), &items) {
                        let p = PROFILES[i];
                        host.set_many(
                            &[("target", p.target), ("ceiling", p.ceiling), ("tp", 1.0)],
                            "cmd.lim_profile",
                        );
                    }
                }
                tip(
                    ui,
                    r.response.contains_pointer(),
                    frame::tr("cmd.lim_profile"),
                    frame::tr("lim.profile_tip"),
                );
                // The rule, on up to three lines.
                let (lr, resp) =
                    ui.allocate_exact_size(vec2(w * k, 3.0 * LINE * k), Sense::hover());
                let ww = lr.width().round() as u32;
                if st.words.rule_split.is_none_or(|(at, _)| at != ww) {
                    let text = &st.words.rule_text;
                    let a = break_at(ui.ctx(), text, Role::Body, s.tk(), lr.width());
                    let b = a + break_at(ui.ctx(), &text[a..], Role::Body, s.tk(), lr.width());
                    st.words.rule_split = Some((ww, [a, b]));
                }
                let [a, b] = st.words.rule_split.map_or([0, 0], |v| v.1);
                let text = &st.words.rule_text;
                let (a, b) = (a.min(text.len()), b.min(text.len()).max(a.min(text.len())));
                with_brush(ui.painter(), |br| {
                    lines_in(
                        br,
                        lr,
                        &[text[..a].trim_end(), text[a..b].trim_end(), &text[b..]],
                        s.t.text_dim,
                        s.tk(),
                    )
                });
                tip(ui, resp.hovered(), name, text);
            });
        },
    );
}

// ---- the inspector's picture ----------------------------------------------------------

/// The inspector's picture: the static curve (a flat top at the ceiling is what a limiter
/// is), and the settings in two lines.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Limiter));
    let ink = Ink::of(&st);
    let s = settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let sq = Rect::from_min_size(rect.min, vec2(h, h)).shrink(space::S1);
    let lo = -36.0f32;
    let x = |d: f32| sq.left() + (d - lo) / -lo * sq.width();
    let y = |d: f32| sq.bottom() - ((d - lo) / -lo).clamp(0.0, 1.0) * sq.height();
    let lat = latency_of(&s, 48_000.0) as f32 / 48.0;
    let unit = if s.true_peak { "dBTP" } else { "dB" };
    let target = spec.get("target");
    let profile = profile_of(target, s.ceiling_db);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        b.segment(
            pos2(x(lo), y(lo)),
            pos2(x(0.0), y(0.0)),
            b.px(),
            ink.grid_strong,
            false,
        );
        let mut pts = [Pos2::ZERO; 49];
        for (i, p) in pts.iter_mut().enumerate() {
            let d = lo + -lo * i as f32 / 48.0;
            *p = pos2(x(d), y(transfer_db(d + s.gain_db, s.ceiling_db, s.knee_db)));
        }
        b.polyline(&pts, 1.5, ink.hue);
        let (cy, px) = (b.snap_mid(y(s.ceiling_db)), b.px());
        canvas::dashed(
            b,
            pos2(sq.left(), cy),
            pos2(sq.right(), cy),
            px,
            ink.hue,
            4.0,
            3.0,
        );
        let words = Rect::from_min_max(pos2(sq.right() + space::S2, rect.top()), rect.max);
        let (l1, l2) = (
            Rect::from_min_max(words.min, pos2(words.right(), words.center().y)),
            Rect::from_min_max(pos2(words.left(), words.center().y), words.max),
        );
        let style = frame::tr(STYLE_KEYS[s.style.index().min(STYLE_KEYS.len() - 1)]);
        canvas::line_in(
            b,
            l1,
            Role::Label,
            ink.text,
            st.tk(),
            format_args!(
                "{style} \u{b7} +{:.1} dB \u{2192} {} {unit}",
                s.gain_db,
                Db(s.ceiling_db)
            ),
        );
        let latency = frame::tr("lim.latency_word");
        match profile {
            Some(p) => canvas::line_in(
                b,
                l2,
                Role::Label,
                ink.dim,
                st.tk(),
                format_args!("{} \u{b7} {latency} {lat:.1} ms", frame::tr(p.key)),
            ),
            None => canvas::line_in(
                b,
                l2,
                Role::Label,
                ink.dim,
                st.tk(),
                format_args!("{} LUFS \u{b7} {latency} {lat:.1} ms", Lufs(target as f64)),
            ),
        }
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("lim.open_tip"), "", Caps::NONE);
    }
    resp
}

// ---- demo -----------------------------------------------------------------------------

/// The settings `--editor limiter` and the screenshot tool open it with.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    for (k, v) in [
        ("gain", 7.5),
        ("style", 0.0),
        ("look", 2.0),
        ("knee", 1.0),
        ("clip", 0.25),
        ("target", -14.0),
        ("ceiling", -1.0),
    ] {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: k.into(),
            value: v,
        });
    }
}

/// A few bars of a mix-like signal: kick, snare, hats, a bass line and a pad, 48 kHz.
pub fn demo_signal(seconds: f32, bpm: f32) -> (Vec<f32>, Vec<f32>) {
    let sr = 48_000.0f32;
    let n = (seconds * sr) as usize;
    let beat = 60.0 / bpm.max(20.0);
    let tau = std::f32::consts::TAU;
    let mut seed = 0x1234_5678u32;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let mut l = vec![0f32; n];
    let mut r = vec![0f32; n];
    let mut hp = 0f32;
    let mut bass_lp = 0f32;
    for i in 0..n {
        let tsec = i as f32 / sr;
        let b = tsec / beat;
        let pos = b.fract() * beat;
        let bar = (b / 4.0) as usize;
        // A build: the second half of the phrase is louder.
        let lift = if (bar / 2) % 2 == 1 { 1.35 } else { 1.0 };
        let kick = (tau * (48.0 + 90.0 * (-pos * 30.0).exp()) * pos).sin() * (-pos * 7.0).exp();
        let off = ((b + 0.5).fract()) * beat;
        let noise = rnd();
        hp = 0.6 * hp + noise - hp * 0.0;
        let hat = (noise - hp * 0.5) * (-off * 60.0).exp() * 0.18;
        let two = (b as usize) % 2 == 1;
        let snare = if two {
            (0.6 * noise + 0.4 * (tau * 190.0 * pos).sin()) * (-pos * 16.0).exp() * 0.6
        } else {
            0.0
        };
        let notes = [41.2f32, 41.2, 55.0, 49.0];
        let f = notes[(b as usize / 2) % 4];
        let saw = 2.0 * (f * tsec).fract() - 1.0;
        bass_lp += (saw - bass_lp) * 0.06;
        let pad = 0.06
            * ((tau * 220.0 * tsec).sin()
                + (tau * 277.2 * tsec).sin()
                + (tau * 329.6 * tsec).sin());
        let mono = 0.55 * kick + 0.28 * bass_lp + snare;
        l[i] = lift * (mono + hat * 0.8 + pad * 1.1) * 0.5;
        r[i] = lift * (mono + hat * 1.2 + pad * 0.9) * 0.5;
    }
    (l, r)
}

/// What a playing engine would feed the editor, for screenshots with no engine: the demo
/// signal through the real limiter at the editor's settings, one column at a time, stored
/// as if the transport had played it from bar 1.
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
    let bpm = app.project().tempo_bpm;
    // Twelve bars of the project's tempo.
    let beats = 4.0 * 12.0;
    let (l, r) = demo_signal(beats * 60.0 / bpm as f32, bpm as f32);
    let mut dsp = Terminus::new(48_000.0);
    dsp.set(&settings(&spec));
    dsp.reset();
    let st = &mut app.fx.limiter;
    *st = State {
        effect: Some(effect),
        ..State::default()
    };
    let per_col = PPQ as f64 * bpm / 60.0 * 0.01;
    let mut cols = Vec::new();
    let mut last = [0f32; 64];
    for (a, b) in l.chunks(480).zip(r.chunks(480)) {
        let (mut a, mut b) = (a.to_vec(), b.to_vec());
        dsp.process(&mut a, &mut b, None);
        last = dsp.telemetry();
        let seq = last[T_SEQ] as u32;
        cols.push(Column::read(&last, seq.wrapping_sub(1)));
    }
    st.feed(&cols, Some(0), per_col);
    st.seen = Some(last[T_SEQ] as u32);
    st.meter_out = [last[T_OUT_L], last[T_OUT_R]];
    st.meter_gr = [last[T_GR_L], last[T_GR_R]];
    st.hold = [(st.max_tp, 0.0), (st.max_tp - 0.3, 0.0)];
    st.song_start = 0.0;
    st.song_len = (PPQ * 4 * 12) as f64;
    st.follow = false;
    // The song view, with bars 5–8 selected in the arrangement: the picture only a host
    // can draw.
    st.view = View::Song;
    app.fx.injected = Some(last);
    let bar = app.project().time_signature.ticks_per_bar().0;
    app.set_time_selection(Ticks(4 * bar), Ticks(8 * bar));
    app.engine.seek(Ticks(11 * bar + bar / 2));
}
