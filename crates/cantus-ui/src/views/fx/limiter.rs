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
//!   sentence; the verdict is written, not only coloured.
//! - **Styles say what they are** (`style_words`), and the latency — which the benchmark
//!   hides — is always on screen, with a warning when the attack is short enough to clip.

use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::Knob;
use cantus_core::{Command, EffectId, EffectSlot, EffectSpec, StripId, Ticks, PPQ};
use cantus_dsp::terminus::loudness::{energy_of, ColumnLoudness, Histogram};
use cantus_dsp::terminus::{
    latency_of, transfer_db, Column, LimSettings, LimStyle, Monitor, Terminus, DITHER_BITS,
    T_COLUMNS, T_GR_L, T_GR_R, T_LATENCY, T_MATCH, T_OUT_L, T_OUT_R, T_RATE, T_SEQ, T_WRAP,
};
use cantus_engine::fx::limiter::settings;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind};
use std::collections::{BTreeMap, VecDeque};

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

fn fmt_db(v: f32) -> String {
    if v <= -150.0 || !v.is_finite() {
        "\u{2212}\u{221e}".to_string()
    } else {
        format!("{v:+.1}").replace('-', "\u{2212}")
    }
}

fn fmt_lufs(v: f64) -> String {
    if v.is_finite() {
        format!("{v:.1}").replace('-', "\u{2212}")
    } else {
        "\u{2014}".to_string()
    }
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

/// Ruler text on a small plate, so it reads over the history.
fn plate(
    p: &egui::Painter,
    at: Pos2,
    align: Align2,
    text: String,
    color: Color32,
    bg: Color32,
) -> Rect {
    let galley = p.layout_no_wrap(text, design::paint::ruler(), color);
    let r = align.anchor_size(at, galley.size());
    let back = r.expand2(vec2(3.0, 1.0));
    p.rect_filled(back, 2.0, bg);
    p.galley(r.min, galley, color);
    back
}

/// Translucent fills read differently on white and on ink: lighter on light themes.
fn fill(c: Color32, dark: bool, amount: f32) -> Color32 {
    c.gamma_multiply(if dark { amount } else { amount * 0.7 })
}

// ---- the body -------------------------------------------------------------------------

pub fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let s = settings(&slot.spec);
    let target = slot.spec.get("target");
    // Another limiter's editor was open before: its history is not this one's.
    if app.fx.limiter.effect.is_some_and(|e| e != effect) {
        app.fx.limiter = State::default();
    }
    app.fx.limiter.effect = Some(effect);
    // What the engine published since the last frame.
    let tel = app.fx_telemetry();
    let playing = app.engine.is_playing();
    let position = app.engine.position();
    let bpm = app.project().tempo_bpm;
    app.fx.limiter.ingest(&tel, playing, position, bpm);
    let now = ui.input(|i| i.time);
    let dt = ui.input(|i| i.stable_dt).clamp(0.0, 0.1);
    {
        let st = &mut app.fx.limiter;
        let outs = [tel[T_OUT_L], tel[T_OUT_R]];
        let grs = [tel[T_GR_L], tel[T_GR_R]];
        for ch in 0..2 {
            // Peak ballistics: instant attack, 20 dB/s fall; reduction falls at 30 dB/s.
            st.meter_out[ch] = outs[ch].max(st.meter_out[ch] - 20.0 * dt);
            st.meter_gr[ch] = grs[ch].max(st.meter_gr[ch] - 30.0 * dt);
            if outs[ch] >= st.hold[ch].0 || now - st.hold[ch].1 > 2.0 {
                st.hold[ch] = (outs[ch], now);
            }
        }
    }
    let rate = if tel[T_RATE] > 0.0 {
        tel[T_RATE]
    } else {
        48_000.0
    };

    display_bar(app, ui, &s, rate);
    let controls_h = space::GRID * 40.0;
    let h = (ui.available_height() - controls_h).max(space::GRID * 50.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::S1;
        let panel_w = space::GRID * 46.0;
        let meters_w = space::GRID * 30.0;
        let w =
            (ui.available_width() - panel_w - meters_w - space::S2 * 2.0).max(space::GRID * 60.0);
        history(app, ui, strip, effect, &s, target, vec2(w, h));
        meters(app, ui, &s, target, vec2(meters_w, h), strip, effect);
        ui.add_space(space::S1);
        ui.allocate_ui_with_layout(
            vec2(panel_w, h),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(panel_w);
                loudness_panel(app, ui, strip, effect, &s, target);
            },
        );
    });
    ui.add_space(space::S1);
    style_row(app, ui, strip, effect, &s);
    knob_row(app, ui, strip, effect, &slot.spec, &s);
    delivery_row(app, ui, strip, effect, slot, &s, &tel);
}

/// Above the display: which view, its span or follow, freeze, and the latency.
fn display_bar(app: &mut CantusApp, ui: &mut egui::Ui, s: &LimSettings, rate: f32) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        let st = &mut app.fx.limiter;
        if ui
            .selectable_label(st.view == View::Live, t("lim.view.live"))
            .on_hover_text(t("lim.view.live_tip"))
            .clicked()
        {
            st.view = View::Live;
        }
        if ui
            .selectable_label(st.view == View::Song, t("lim.view.song"))
            .on_hover_text(t("lim.view.song_tip"))
            .clicked()
        {
            st.view = View::Song;
        }
        ui.separator();
        match st.view {
            View::Live => {
                for secs in [6.0, 20.0, 60.0] {
                    if ui
                        .selectable_label(st.span_s == secs, format!("{secs:.0} s"))
                        .clicked()
                    {
                        st.span_s = secs;
                    }
                }
                let mut frozen = st.frozen_at.is_some();
                if ui
                    .toggle_value(&mut frozen, t("lim.freeze"))
                    .on_hover_text(t("lim.freeze_tip"))
                    .changed()
                {
                    st.frozen_at = frozen.then_some(st.total);
                }
            }
            View::Song => {
                ui.toggle_value(&mut st.follow, t("lim.follow"))
                    .on_hover_text(t("lim.follow_tip"));
                if ui
                    .button(t("lim.whole_song"))
                    .on_hover_text(t("lim.whole_song_tip"))
                    .clicked()
                {
                    if let (Some((&a, _)), Some((&b, _))) =
                        (st.song.first_key_value(), st.song.last_key_value())
                    {
                        st.follow = false;
                        st.song_start = (a * BUCKET) as f64;
                        st.song_len = (((b - a + 1) * BUCKET) as f64).max(PPQ as f64 * 4.0);
                    }
                }
                if ui
                    .button(t("lim.forget_song"))
                    .on_hover_text(t("lim.forget_song_tip"))
                    .clicked()
                {
                    st.song.clear();
                    st.song_rev += 1;
                }
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let lat = latency_of(s, rate);
            ui.label(
                RichText::new(tf(
                    "lim.latency",
                    &[
                        ("ms", &format!("{:.2}", lat as f32 / rate * 1000.0)),
                        ("n", &lat),
                    ],
                ))
                .text_style(design::mono_small())
                .color(pal.text),
            )
            .on_hover_text(t("lim.latency_tip"));
            let ramp_ms = s.lookahead_ms * s.style.spec().ramp;
            if ramp_ms < 0.1 {
                ui.label(
                    RichText::new(t("lim.near_clip"))
                        .text_style(design::caption())
                        .color(pal.danger),
                )
                .on_hover_text(t("lim.near_clip_tip"));
            }
        });
    });
}

/// A run of pixels [x0, x1) and the history it shows.
#[derive(Copy, Clone, Debug)]
struct Strip {
    x0: f32,
    x1: f32,
    col: Col,
}

/// The history as pixel strips, for either view.
fn strips(app: &CantusApp, r: Rect) -> Vec<Strip> {
    let st = &app.fx.limiter;
    let mut out = Vec::new();
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
    out
}

/// The largest reductions, spaced apart: (strip index, reduction).
fn peak_labels(strips: &[Strip], min_gap: f32, max: usize) -> Vec<(usize, f32)> {
    let mut cands: Vec<(usize, f32)> = strips
        .iter()
        .enumerate()
        .filter(|(_, s)| s.col.gr_db >= 0.3)
        .map(|(i, s)| (i, s.col.gr_db))
        .collect();
    cands.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut out: Vec<(usize, f32)> = Vec::new();
    for (i, g) in cands {
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
    out
}

/// The history: a gain-reduction lane on top, levels under it, time along the bottom.
fn history(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &LimSettings,
    target: f32,
    size: egui::Vec2,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let axis_h = space::GRID * 4.0;
    let gr_h = (rect.height() * 0.2).round();
    let gr_r = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + gr_h));
    let lv = Rect::from_min_max(
        pos2(rect.left(), gr_r.bottom() + 1.0),
        pos2(rect.right(), rect.bottom() - axis_h),
    );
    let axis = Rect::from_min_max(pos2(rect.left(), lv.bottom()), rect.max);
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    p.hline(
        rect.x_range(),
        gr_r.bottom(),
        Stroke::new(1.0, pal.grid_strong),
    );

    // Song view follows the playhead unless the user took the view.
    let bar = app.project().time_signature.ticks_per_bar().0;
    let beat = app.project().time_signature.ticks_per_beat().0;
    let position = app.engine.position().0 as f64;
    {
        let st = &mut app.fx.limiter;
        if st.view == View::Song && st.follow {
            st.song_start = position - st.song_len * 0.85;
        }
    }

    // Grid lines; their numbers go on top of the history, on plates.
    for g in LEVEL_TICKS {
        p.hline(lv.x_range(), y_of(g, lv), Stroke::new(1.0, pal.grid));
    }
    for g in GR_TICKS {
        p.hline(gr_r.x_range(), y_gr(g, gr_r), Stroke::new(1.0, pal.grid));
    }

    // Song view: bar lines, the time selection, the ruler.
    let view = app.fx.limiter.view;
    let (song_start, song_len) = (app.fx.limiter.song_start, app.fx.limiter.song_len);
    let x_of_tick = |tk: f64| lv.left() + ((tk - song_start) / song_len) as f32 * lv.width();
    let tick_at = |x: f32| song_start + ((x - lv.left()) / lv.width()) as f64 * song_len;
    if view == View::Song {
        if let Some((a, b)) = app.time_selection {
            let (x0, x1) = (x_of_tick(a.0 as f64), x_of_tick(b.0 as f64));
            let sel =
                Rect::from_x_y_ranges(x0.max(rect.left())..=x1.min(rect.right()), rect.y_range());
            if sel.width() > 0.0 {
                p.rect_filled(sel, 0.0, pal.selection.gamma_multiply(0.10));
            }
        }
        let px_per_bar = bar as f32 / song_len as f32 * lv.width();
        let every = (60.0 / px_per_bar.max(1.0)).ceil().max(1.0) as i64;
        let first = (song_start as i64).div_euclid(bar);
        let last = ((song_start + song_len) as i64).div_euclid(bar) + 1;
        for b in first..=last {
            let x = x_of_tick((b * bar) as f64);
            if x < rect.left() || x > rect.right() {
                continue;
            }
            let strong = b % every == 0;
            p.vline(
                x,
                rect.top()..=lv.bottom(),
                Stroke::new(1.0, if strong { pal.grid_strong } else { pal.grid }),
            );
            if strong {
                p.text(
                    pos2(x + 2.0, axis.center().y),
                    Align2::LEFT_CENTER,
                    format!("{}", b + 1),
                    design::paint::ruler(),
                    pal.text_dim,
                );
            }
            if px_per_bar > 80.0 {
                for k in 1..(bar / beat.max(1)) {
                    let xb = x_of_tick((b * bar + k * beat) as f64);
                    p.vline(
                        xb,
                        lv.bottom()..=lv.bottom() + 3.0,
                        Stroke::new(1.0, pal.grid_strong),
                    );
                }
            }
        }
    } else {
        // Live: seconds ago.
        let span = app.fx.limiter.span_s;
        let step = if span <= 6.0 {
            1.0
        } else if span <= 20.0 {
            5.0
        } else {
            10.0
        };
        let mut sec = 0.0;
        while sec <= span {
            let x = lv.right() - sec / span * lv.width();
            p.vline(
                x,
                lv.bottom()..=lv.bottom() + 3.0,
                Stroke::new(1.0, pal.grid_strong),
            );
            let label = if sec == 0.0 {
                t("lim.now")
            } else {
                format!("\u{2212}{sec:.0} s")
            };
            p.text(
                pos2(x - 2.0, axis.center().y),
                Align2::RIGHT_CENTER,
                label,
                design::paint::ruler(),
                pal.text_dim,
            );
            sec += step;
        }
    }

    // The history.
    let all = strips(app, lv);
    let mut m_in = egui::Mesh::default();
    let mut m_out = egui::Mesh::default();
    let mut m_gr = egui::Mesh::default();
    let mut m_clip = egui::Mesh::default();
    let c_in = fill(pal.text_dim, pal.dark, if pal.dark { 0.30 } else { 0.55 });
    let c_out = fill(pal.selection, pal.dark, 0.55);
    let c_gr = fill(pal.danger, pal.dark, 0.85);
    let c_clip = pal.meter_high;
    let mut lines: Vec<Vec<Pos2>> = vec![Vec::new()];
    let mut gr_line: Vec<Vec<Pos2>> = vec![Vec::new()];
    let mut last_x1 = f32::NEG_INFINITY;
    for sp in &all {
        let (x0, x1) = (sp.x0.max(lv.left()), sp.x1.min(lv.right()));
        if x1 <= x0 {
            continue;
        }
        let c = &sp.col;
        m_in.add_colored_rect(
            Rect::from_min_max(pos2(x0, y_of(c.input_db, lv)), pos2(x1, lv.bottom())),
            c_in,
        );
        m_out.add_colored_rect(
            Rect::from_min_max(pos2(x0, y_of(c.output_db, lv)), pos2(x1, lv.bottom())),
            c_out,
        );
        if c.gr_db > 0.01 {
            m_gr.add_colored_rect(
                Rect::from_min_max(pos2(x0, gr_r.top()), pos2(x1, y_gr(c.gr_db, gr_r))),
                c_gr,
            );
        }
        if c.clip_db > 0.05 {
            let h = (2.0 + c.clip_db * 3.0).min(space::GRID * 3.0);
            m_clip.add_colored_rect(
                Rect::from_min_max(pos2(x0, lv.top()), pos2(x1, lv.top() + h)),
                c_clip,
            );
        }
        // A gap in the song breaks the lines.
        if sp.x0 > last_x1 + 1.5 {
            lines.push(Vec::new());
            gr_line.push(Vec::new());
        }
        last_x1 = sp.x1;
        let xm = (x0 + x1) * 0.5;
        if c.short_term.is_finite() && c.short_term > LOW_DB {
            lines
                .last_mut()
                .unwrap()
                .push(pos2(xm, y_of(c.short_term, lv)));
        } else if !lines.last().unwrap().is_empty() {
            lines.push(Vec::new());
        }
        gr_line
            .last_mut()
            .unwrap()
            .push(pos2(xm, y_gr(c.gr_db, gr_r)));
    }
    p.add(m_in);
    p.add(m_out);
    p.add(m_gr);
    p.add(m_clip);
    for l in gr_line.into_iter().filter(|l| l.len() > 1) {
        p.add(Shape::line(l, Stroke::new(1.0, pal.danger)));
    }
    for l in lines.into_iter().filter(|l| l.len() > 1) {
        p.add(Shape::line(l, Stroke::new(1.5, pal.gold)));
    }
    let bg = theme::recess(&pal).gamma_multiply(0.85);
    for g in LEVEL_TICKS {
        plate(
            &p,
            pos2(lv.right() - space::S1, y_of(g, lv)),
            Align2::RIGHT_CENTER,
            format!("{g:.0}").replace('-', "\u{2212}"),
            pal.text_dim,
            bg,
        );
    }
    for g in GR_TICKS {
        plate(
            &p,
            pos2(gr_r.right() - space::S1, y_gr(g, gr_r)),
            Align2::RIGHT_CENTER,
            format!("\u{2212}{g:.0}"),
            pal.text_dim,
            bg,
        );
    }
    plate(
        &p,
        pos2(gr_r.right() - space::GRID * 8.0, gr_r.top() + 2.0),
        Align2::RIGHT_TOP,
        t("lim.gr_lane"),
        pal.text_dim,
        bg,
    );

    // The ceiling, the target and the gain handle.
    let held = app.fx.limiter.drag;
    let hover = resp.hover_pos();
    let cy = y_of(s.ceiling_db, lv);
    let ty = y_of(target, lv);
    let handle = Rect::from_min_max(
        lv.left_top(),
        pos2(lv.left() + space::GRID * 10.0, lv.bottom()),
    );
    let near = |y: f32| {
        hover.is_some_and(|h| lv.contains(h) && (h.y - y).abs() < 5.0 && !handle.contains(h))
    };
    let ceiling_hot = held == Some(Grab::Ceiling) || (held.is_none() && near(cy));
    let target_hot = held == Some(Grab::Target) || (held.is_none() && !ceiling_hot && near(ty));
    p.add(Shape::dashed_line(
        &[pos2(lv.left(), cy), pos2(lv.right(), cy)],
        Stroke::new(if ceiling_hot { 2.0 } else { 1.2 }, pal.gold),
        6.0,
        4.0,
    ));
    let unit = if s.true_peak { "dBTP" } else { "dB" };
    plate(
        &p,
        pos2(lv.right() - space::GRID * 8.0, cy - 2.0),
        Align2::RIGHT_BOTTOM,
        format!("{} {} {unit}", t("fx.lim.ceiling"), fmt_db(s.ceiling_db)),
        pal.gold,
        bg,
    );
    p.add(Shape::dashed_line(
        &[pos2(lv.left(), ty), pos2(lv.right(), ty)],
        Stroke::new(if target_hot { 2.0 } else { 1.0 }, pal.ok),
        2.0,
        4.0,
    ));
    plate(
        &p,
        pos2(handle.right() + space::S2, ty + 3.0),
        Align2::LEFT_TOP,
        tf("lim.target_line", &[("lufs", &fmt_lufs(target as f64))]),
        pal.ok,
        bg,
    );
    // The gain handle: a strip on the left edge filled to the gain, its value on it.
    let gain_hot = matches!(held, Some(Grab::Gain { .. }))
        || (held.is_none() && hover.is_some_and(|h| handle.contains(h)));
    let gf = s.gain_db / 30.0;
    p.rect_filled(handle, 0.0, fill(pal.panel_raised, pal.dark, 0.9));
    let gtop = handle.bottom() - gf * handle.height();
    p.rect_filled(
        Rect::from_min_max(pos2(handle.left(), gtop), handle.max),
        0.0,
        fill(pal.danger, pal.dark, if gain_hot { 0.5 } else { 0.3 }),
    );
    p.vline(
        handle.right(),
        handle.y_range(),
        Stroke::new(1.0, pal.grid_strong),
    );
    plate(
        &p,
        pos2(handle.center().x, (gtop - 3.0).max(handle.top() + 14.0)),
        Align2::CENTER_BOTTOM,
        format!("+{:.1}", s.gain_db),
        pal.text,
        bg,
    );
    p.text(
        pos2(handle.center().x, handle.bottom() - 2.0),
        Align2::CENTER_BOTTOM,
        t("fx.lim.gain"),
        design::paint::caption(),
        pal.text_dim,
    );

    // The playhead (song view).
    if view == View::Song {
        let x = x_of_tick(position);
        if x >= rect.left() && x <= rect.right() {
            p.vline(
                x,
                rect.top()..=lv.bottom(),
                Stroke::new(1.5, pal.gold_bright),
            );
        }
    }

    // Peak labels: the largest reductions, as tags you can click to go there.
    let labels = peak_labels(&all, space::GRID * 22.0, 6);
    let mut label_hits: Vec<(Rect, Option<i64>)> = Vec::new();
    for (i, g) in &labels {
        let sp = all[*i];
        let x = ((sp.x0 + sp.x1) * 0.5).clamp(lv.left() + 24.0, lv.right() - 24.0);
        let y = (y_gr(*g, gr_r) + 3.0)
            .min(gr_r.bottom() - 16.0)
            .max(gr_r.top() + 2.0);
        let text = format!("\u{2212}{g:.1} dB");
        let galley = p.layout_no_wrap(text, design::paint::ruler(), pal.text);
        let tag = Rect::from_center_size(
            pos2(x, y + galley.size().y * 0.5 + 2.0),
            galley.size() + vec2(8.0, 4.0),
        );
        let hot = hover.is_some_and(|h| tag.contains(h)) && sp.col.tick.is_some();
        p.rect_filled(
            tag,
            pal.corner_radius,
            if hot {
                pal.gold_bright
            } else {
                pal.panel_raised
            },
        );
        p.rect_stroke(
            tag,
            pal.corner_radius,
            Stroke::new(1.0, pal.danger),
            StrokeKind::Inside,
        );
        p.galley(tag.min + vec2(4.0, 2.0), galley, pal.text);
        label_hits.push((tag, sp.col.tick));
    }

    // Hover: what was there, at that moment.
    if let Some(h) = hover.filter(|h| lv.contains(*h) || gr_r.contains(*h)) {
        if held.is_none() && !handle.contains(h) {
            if let Some(sp) = all
                .iter()
                .find(|sp| h.x >= sp.x0 && h.x < sp.x1.max(sp.x0 + 1.0))
            {
                p.vline(
                    h.x,
                    rect.top()..=lv.bottom(),
                    Stroke::new(1.0, pal.text_dim),
                );
                let when = match sp.col.tick {
                    Some(tick) => {
                        let (b, bt, _) = app.project().time_signature.bbt(Ticks(tick));
                        format!("{b}.{bt}")
                    }
                    None => format!(
                        "\u{2212}{:.2} s",
                        (lv.right() - h.x) / lv.width() * app.fx.limiter.span_s
                    ),
                };
                let c = sp.col;
                let text = tf(
                    "lim.hover",
                    &[
                        ("when", &when),
                        ("inp", &fmt_db(c.input_db)),
                        ("out", &fmt_db(c.output_db)),
                        ("gr", &format!("{:.1}", c.gr_db)),
                        ("s", &fmt_lufs(c.short_term as f64)),
                    ],
                );
                let galley = p.layout_no_wrap(text, design::paint::ruler(), pal.text);
                let at = pos2(
                    (h.x + 8.0).min(lv.right() - galley.size().x - 6.0),
                    lv.top() + 4.0,
                );
                let bg = Rect::from_min_size(at - vec2(3.0, 2.0), galley.size() + vec2(6.0, 4.0));
                p.rect_filled(bg, pal.corner_radius, pal.panel_raised);
                p.rect_stroke(
                    bg,
                    pal.corner_radius,
                    Stroke::new(1.0, pal.grid_strong),
                    StrokeKind::Inside,
                );
                p.galley(at, galley, pal.text);
            }
        }
    }

    // Empty and frozen states.
    if all.is_empty() {
        let key = if view == View::Song {
            "lim.empty_song"
        } else {
            "lim.empty_live"
        };
        p.text(
            lv.center(),
            Align2::CENTER_CENTER,
            t(key),
            design::paint::empty_state(),
            pal.text_dim,
        );
    }
    if view == View::Live && app.fx.limiter.frozen_at.is_some() {
        p.text(
            pos2(lv.center().x, gr_r.top() + 3.0),
            Align2::CENTER_TOP,
            t("lim.frozen"),
            design::paint::label(),
            pal.gold,
        );
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    app.fx.limiter.geometry = Some(Geometry {
        levels: lv,
        ceiling_y: cy,
        target_y: ty,
        gain_handle: handle,
        labels: label_hits.clone(),
    });

    // ---- gestures ----
    let alt = ui.input(|i| i.modifiers.alt);
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(h) = origin.or(resp.interact_pointer_pos()) {
            let st = &mut app.fx.limiter;
            st.drag_from = (s.gain_db, s.ceiling_db);
            st.drag = if handle.contains(h) {
                Some(Grab::Gain { alt })
            } else if (h.y - cy).abs() < 6.0 {
                Some(Grab::Ceiling)
            } else if (h.y - ty).abs() < 6.0 {
                Some(Grab::Target)
            } else if st.view == View::Song {
                Some(Grab::Pan)
            } else {
                None
            };
        }
    }
    if resp.dragged() {
        match (app.fx.limiter.drag, resp.interact_pointer_pos()) {
            (Some(Grab::Ceiling), Some(h)) => {
                let v = (db_at(h.y, lv) * 10.0).round() / 10.0;
                app.set_fx_param(strip, effect, "ceiling", v.clamp(-30.0, 0.0), true);
            }
            (Some(Grab::Target), Some(h)) => {
                let v = (db_at(h.y, lv) * 10.0).round() / 10.0;
                app.set_fx_param(strip, effect, "target", v.clamp(-36.0, -5.0), true);
            }
            (Some(Grab::Gain { alt }), Some(_)) => {
                let fine = ui.input(|i| i.modifiers.shift);
                let dy = resp.drag_delta().y;
                let per_px = 30.0 / handle.height() * if fine { 0.2 } else { 1.0 };
                let g = (s.gain_db - dy * per_px).clamp(0.0, 30.0);
                let key = cantus_core::MergeKey::of(&("lim-gain", effect.0));
                app.exec_merging(
                    Command::SetEffectParam {
                        strip,
                        effect,
                        key: "gain".into(),
                        value: g,
                    },
                    key,
                );
                if alt {
                    // Alt: the ceiling moves the other way, so what you hear stays level.
                    let (g0, c0) = app.fx.limiter.drag_from;
                    let c = (c0 - (g - g0)).clamp(-30.0, 0.0);
                    app.exec_merging(
                        Command::SetEffectParam {
                            strip,
                            effect,
                            key: "ceiling".into(),
                            value: c,
                        },
                        key,
                    );
                }
            }
            (Some(Grab::Pan), _) => {
                let st = &mut app.fx.limiter;
                st.follow = false;
                st.song_start -= resp.drag_delta().x as f64 / lv.width() as f64 * st.song_len;
            }
            _ => {}
        }
    }
    if resp.drag_stopped() {
        app.fx.limiter.drag = None;
        app.end_gesture();
    }
    if resp.clicked() {
        if let Some(h) = resp.interact_pointer_pos() {
            if let Some((_, Some(tick))) = label_hits.iter().find(|(r, _)| r.contains(h)) {
                app.seek(Ticks(*tick));
            }
        }
    }
    if resp.double_clicked() {
        let st = &mut app.fx.limiter;
        match st.view {
            View::Song => st.follow = true,
            View::Live => st.frozen_at = None,
        }
    }
    // Wheel: in the song view, scroll; with Ctrl, zoom about the pointer.
    if resp.hovered() && view == View::Song {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        let st = &mut app.fx.limiter;
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
    } else if label_hits
        .iter()
        .any(|(r, t)| t.is_some() && hover.is_some_and(|h| r.contains(h)))
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(t("lim.display_tip"));
}

/// Meters beside the display: reduction per channel in the lane, output true peak per
/// channel and the three loudness scales on the level area's scale, with the target.
#[allow(clippy::too_many_arguments)]
fn meters(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    s: &LimSettings,
    target: f32,
    size: egui::Vec2,
    strip: StripId,
    effect: EffectId,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let axis_h = space::GRID * 4.0;
    let gr_h = (rect.height() * 0.2).round();
    let gr_r = Rect::from_min_max(rect.min, pos2(rect.right(), rect.top() + gr_h));
    let lv = Rect::from_min_max(
        pos2(rect.left(), gr_r.bottom() + 1.0),
        pos2(rect.right(), rect.bottom() - axis_h),
    );
    let st = &app.fx.limiter;
    let bw = (rect.width() - space::S1 * 9.0) / 6.0;
    let col_x = |i: usize| {
        rect.left()
            + space::S1
            + i as f32 * (bw + space::S1)
            + if i >= 2 { space::S1 * 2.0 } else { 0.0 }
    };
    // Reduction: L R, hanging from the top.
    for ch in 0..2 {
        let x = col_x(ch);
        let bar = Rect::from_min_max(
            pos2(x, gr_r.top() + 12.0),
            pos2(x + bw, gr_r.bottom() - 2.0),
        );
        p.rect_filled(bar, 0.0, fill(pal.grid, pal.dark, 1.0));
        let y = bar.top() + (st.meter_gr[ch] / GR_SPAN).clamp(0.0, 1.0) * bar.height();
        p.rect_filled(
            Rect::from_min_max(bar.min, pos2(bar.right(), y)),
            0.0,
            pal.danger,
        );
    }
    p.text(
        pos2(col_x(0), gr_r.top() + 1.0),
        Align2::LEFT_TOP,
        format!("\u{2212}{:.1}", st.max_gr),
        design::paint::ruler(),
        pal.danger,
    );
    // Output true peak: L R, with the held peak.
    for ch in 0..2 {
        let x = col_x(ch);
        let bar = Rect::from_min_max(pos2(x, lv.top()), pos2(x + bw, lv.bottom()));
        p.rect_filled(bar, 0.0, fill(pal.grid, pal.dark, 1.0));
        let v = st.meter_out[ch];
        let color = if v > s.ceiling_db + 0.005 {
            pal.meter_clip
        } else if v > -6.0 {
            pal.meter_high
        } else {
            pal.meter_low
        };
        if v > LOW_DB {
            p.rect_filled(
                Rect::from_min_max(pos2(x, y_of(v, lv)), bar.max),
                0.0,
                color,
            );
        }
        let hy = y_of(st.hold[ch].0, lv);
        if st.hold[ch].0 > LOW_DB {
            p.hline(bar.x_range(), hy, Stroke::new(2.0, pal.text));
        }
        p.text(
            pos2(bar.center().x, lv.bottom() + 2.0),
            Align2::CENTER_TOP,
            if ch == 0 { "L" } else { "R" },
            design::paint::caption(),
            pal.text_dim,
        );
    }
    // Loudness: momentary, short-term, integrated.
    let loud = &st.loud;
    let vals = [
        ("M", loud.momentary),
        ("S", loud.short_term),
        ("I", loud.integrated().unwrap_or(f64::NEG_INFINITY)),
    ];
    for (k, (label, v)) in vals.iter().enumerate() {
        let x = col_x(3 + k);
        let bar = Rect::from_min_max(pos2(x, lv.top()), pos2(x + bw, lv.bottom()));
        p.rect_filled(bar, 0.0, fill(pal.grid, pal.dark, 1.0));
        if v.is_finite() && (*v as f32) > LOW_DB {
            let v = *v as f32;
            p.rect_filled(
                Rect::from_min_max(pos2(x, y_of(v.min(target), lv)), bar.max),
                0.0,
                pal.gold,
            );
            if v > target {
                // What is over the target, hatched in the danger colour.
                let over =
                    Rect::from_min_max(pos2(x, y_of(v, lv)), pos2(bar.right(), y_of(target, lv)));
                p.rect_filled(over, 0.0, fill(pal.danger, pal.dark, 0.35));
                let mut yy = over.top();
                while yy < over.bottom() {
                    p.line_segment(
                        [
                            pos2(over.left(), (yy + bw).min(over.bottom())),
                            pos2(over.right(), yy),
                        ],
                        Stroke::new(1.0, pal.danger),
                    );
                    yy += 4.0;
                }
            }
        }
        p.text(
            pos2(bar.center().x, lv.bottom() + 2.0),
            Align2::CENTER_TOP,
            *label,
            design::paint::caption(),
            pal.text_dim,
        );
    }
    // The target across the loudness bars: drag it.
    let ty = y_of(target, lv);
    let tx = col_x(3) - 2.0..=col_x(5) + bw + 2.0;
    p.hline(tx, ty, Stroke::new(2.0, pal.ok));
    // The ceiling across the output bars.
    let cy = y_of(s.ceiling_db, lv);
    p.hline(
        col_x(0) - 2.0..=col_x(1) + bw + 2.0,
        cy,
        Stroke::new(1.0, pal.gold),
    );
    // Max true peak above the output bars; click the meter to reset what it holds.
    let tp_over = st.max_tp > s.ceiling_db + 0.005;
    p.text(
        pos2(col_x(0), lv.top() - 1.0),
        Align2::LEFT_BOTTOM,
        fmt_db(st.max_tp),
        design::paint::ruler(),
        if tp_over { pal.danger } else { pal.text },
    );
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
    let near_target = resp
        .hover_pos()
        .is_some_and(|h| h.x >= col_x(3) - 2.0 && (h.y - ty).abs() < 6.0);
    if near_target || app.fx.limiter.drag == Some(Grab::Target) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let pressed_on_target = ui
        .input(|i| i.pointer.press_origin())
        .is_some_and(|h| h.x >= col_x(3) - 2.0 && (h.y - ty).abs() < 6.0);
    if resp.drag_started() && pressed_on_target {
        app.fx.limiter.drag = Some(Grab::Target);
    }
    if resp.dragged() && app.fx.limiter.drag == Some(Grab::Target) {
        if let Some(h) = resp.interact_pointer_pos() {
            let v = (db_at(h.y, lv) * 10.0).round() / 10.0;
            app.set_fx_param(strip, effect, "target", v.clamp(-36.0, -5.0), true);
        }
    }
    if resp.drag_stopped() {
        app.fx.limiter.drag = None;
        app.end_gesture();
    }
    if resp.clicked() {
        app.fx.limiter.max_tp = f32::NEG_INFINITY;
        app.fx.limiter.max_gr = 0.0;
        app.fx.limiter.hold = [(-160.0, 0.0); 2];
    }
    resp.on_hover_text(t("lim.meters_tip"));
}

/// A verdict drawn as a shape and words: a filled disc for a pass, a hollow triangle for a
/// miss — never colour alone.
fn verdict_line(ui: &mut egui::Ui, pass: bool, text: &str) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(vec2(space::S3, space::S3), Sense::hover());
        let c = r.center();
        if pass {
            ui.painter().circle_filled(c, 4.5, pal.ok);
        } else {
            let pts = vec![
                pos2(c.x, c.y - 5.0),
                pos2(c.x + 5.0, c.y + 4.0),
                pos2(c.x - 5.0, c.y + 4.0),
            ];
            ui.painter()
                .add(Shape::closed_line(pts, Stroke::new(1.5, pal.danger)));
        }
        ui.add(
            egui::Label::new(
                RichText::new(text)
                    .text_style(design::caption())
                    .color(if pass { pal.ok } else { pal.text }),
            )
            .wrap(),
        );
    });
}

fn reading(ui: &mut egui::Ui, label: &str, value: String, color: Color32) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(label)
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(value)
                    .text_style(design::mono_small())
                    .color(color),
            );
        });
    });
}

/// The loudness panel: the delivery profile and its rule, the readings, the verdict, and
/// the song and selection measured from the history.
fn loudness_panel(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &LimSettings,
    target: f32,
) {
    let pal = theme::p();
    ui.spacing_mut().item_spacing.y = 2.0;
    // The profile.
    let current = profile_of(target, s.ceiling_db);
    let name = current
        .map(|p| t(p.key))
        .unwrap_or_else(|| t("lim.profile.custom"));
    egui::ComboBox::from_id_salt(("lim-profile", effect.0))
        .selected_text(name)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for pr in &PROFILES {
                let label = format!(
                    "{}  {} LUFS \u{b7} {} dBTP",
                    t(pr.key),
                    fmt_lufs(pr.target as f64),
                    fmt_db(pr.ceiling)
                );
                if ui.selectable_label(current == Some(pr), label).clicked() {
                    app.exec(Command::Batch {
                        label_key: "cmd.lim_profile".into(),
                        commands: [("target", pr.target), ("ceiling", pr.ceiling), ("tp", 1.0)]
                            .into_iter()
                            .map(|(k, v)| Command::SetEffectParam {
                                strip,
                                effect,
                                key: k.into(),
                                value: v,
                            })
                            .collect(),
                    });
                }
            }
        })
        .response
        .on_hover_text(t("lim.profile_tip"));
    let rule = match current {
        Some(pr) => t(&format!("{}_rule", pr.key)),
        None => t("lim.profile.custom_rule"),
    };
    ui.add(
        egui::Label::new(
            RichText::new(rule)
                .text_style(design::caption())
                .color(pal.text_dim),
        )
        .wrap(),
    );
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t("fx.lim.target"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        let mut v = target;
        let r = ui.add(
            egui::DragValue::new(&mut v)
                .range(-36.0..=-5.0)
                .speed(0.1)
                .fixed_decimals(1)
                .suffix(" LUFS"),
        );
        if r.changed() {
            app.set_fx_param(strip, effect, "target", v, true);
        }
        if r.drag_stopped() || r.lost_focus() {
            app.end_gesture();
        }
    });
    ui.add_space(space::S1);

    // The big number: integrated since the reset.
    let st = &app.fx.limiter;
    let integrated = st.loud.integrated();
    ui.label(
        RichText::new(t("lim.integrated"))
            .text_style(design::caption())
            .color(pal.text_dim),
    );
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(
                integrated
                    .map(fmt_lufs)
                    .unwrap_or_else(|| "\u{2014}".into()),
            )
            .text_style(design::readout())
            .color(pal.text),
        );
        ui.label(
            RichText::new("LUFS")
                .text_style(design::caption())
                .color(pal.text_dim),
        );
    });
    let (pass, words) = verdict(integrated, st.max_tp, target, s.ceiling_db, current);
    verdict_line(ui, pass, &words);
    ui.add_space(space::S1);
    let lufs_s = |v: f64| format!("{} LUFS", fmt_lufs(v));
    reading(
        ui,
        &t("lim.short_term"),
        lufs_s(st.loud.short_term),
        pal.text,
    );
    reading(ui, &t("lim.momentary"), lufs_s(st.loud.momentary), pal.text);
    reading(
        ui,
        &t("lim.range"),
        st.loud
            .range()
            .map(|r| format!("{r:.1} LU"))
            .unwrap_or_else(|| "\u{2014}".into()),
        pal.text,
    );
    let tp_over = st.max_tp > s.ceiling_db + 0.005;
    reading(
        ui,
        &t("lim.true_peak"),
        format!("{} dBTP", fmt_db(st.max_tp)),
        if tp_over { pal.danger } else { pal.text },
    );
    reading(
        ui,
        &t("lim.max_ms"),
        format!(
            "{} / {}",
            fmt_lufs(st.loud.max_momentary),
            fmt_lufs(st.loud.max_short_term)
        ),
        pal.text,
    );
    ui.horizontal(|ui| {
        let st = &mut app.fx.limiter;
        if ui
            .small_button(t("lim.reset"))
            .on_hover_text(t("lim.reset_tip"))
            .clicked()
        {
            st.reset_loudness();
        }
        let label = if st.paused {
            t("lim.resume")
        } else {
            t("lim.pause")
        };
        if ui.small_button(label).clicked() {
            st.paused = !st.paused;
        }
        ui.checkbox(&mut st.auto_reset, t("lim.auto_reset"))
            .on_hover_text(t("lim.auto_reset_tip"));
    });

    // The song as last heard, and the time selection — from the history in song time.
    let bpm = app.project().tempo_bpm;
    let bar = app.project().time_signature.ticks_per_bar().0;
    let sel = app.time_selection.map(|(a, b)| (a.0, b.0));
    if !app.fx.limiter.song.is_empty() {
        ui.add_space(space::S1);
        let sum = app.fx.limiter.song_summary(bpm, bar);
        summary(ui, &t("lim.song"), &sum, s.ceiling_db);
        if let Some(sel) = sel {
            let sum = app.fx.limiter.selection_summary(sel, bpm, bar);
            summary(ui, &t("lim.selection"), &sum, s.ceiling_db);
        }
    }
}

fn summary(ui: &mut egui::Ui, title: &str, s: &Summary, ceiling: f32) {
    let pal = theme::p();
    ui.label(
        RichText::new(tf(
            "lim.summary_title",
            &[("what", &title), ("from", &s.bars.0), ("to", &s.bars.1)],
        ))
        .text_style(design::caption())
        .color(pal.text_dim),
    )
    .on_hover_text(t("lim.song_tip"));
    let i = s
        .integrated
        .map(|v| format!("{} LUFS", fmt_lufs(v)))
        .unwrap_or_else(|| "\u{2014}".into());
    let lra = s
        .range
        .map(|v| format!("{v:.1} LU"))
        .unwrap_or_else(|| "\u{2014}".into());
    ui.label(
        RichText::new(format!("{i} \u{b7} {lra}"))
            .text_style(design::mono_small())
            .color(pal.text),
    );
    ui.label(
        RichText::new(tf("lim.summary_tp", &[("db", &fmt_db(s.max_tp))]))
            .text_style(design::mono_small())
            .color(if s.max_tp > ceiling + 0.005 {
                pal.danger
            } else {
                pal.text
            }),
    );
}

/// The styles, and what the chosen one is in words.
fn style_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &LimSettings,
) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(t("fx.lim.style"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        for style in LimStyle::ALL {
            let key = format!("lim.style.{}", style.index());
            if ui
                .selectable_label(s.style == style, t(&key))
                .on_hover_text(format!(
                    "{}\n{}",
                    t(&format!("{key}_use")),
                    style_words(style)
                ))
                .clicked()
            {
                app.set_fx_param(strip, effect, "style", style.index() as f32, false);
            }
        }
        ui.add(
            egui::Label::new(
                RichText::new(format!(
                    "{} \u{2014} {}",
                    t(&format!("lim.style.{}_use", s.style.index())),
                    style_words(s.style)
                ))
                .text_style(design::caption())
                .color(pal.text_dim),
            )
            .truncate(),
        );
    });
}

/// Every parameter with a range, as the house knob.
fn knob_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    spec: &EffectSpec,
    s: &LimSettings,
) {
    let tp = s.true_peak;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::S1;
        for key in [
            "gain", "ceiling", "look", "att", "rel", "link_t", "link_r", "knee", "clip",
        ] {
            let Some(info) = spec.kind.param(key) else {
                continue;
            };
            let mut v = spec.get(key);
            let knob = Knob::new(&mut v, info.min..=info.max, info.default)
                .diameter(space::GRID * 11.0)
                .log(info.logarithmic)
                .unit(info.unit)
                .label(t(info.label_key));
            let knob = match key {
                "ceiling" => knob
                    .format(move |x| format!("{} {}", fmt_db(x), if tp { "dBTP" } else { "dB" })),
                "gain" => knob.format(|x| format!("+{x:.1} dB")),
                "knee" => knob.format(|x| {
                    if x < 0.05 {
                        t("lim.hard")
                    } else {
                        format!("{x:.1} dB")
                    }
                }),
                "clip" => knob.format(|x| {
                    if x <= 0.0 {
                        t("lim.off")
                    } else {
                        format!("{:.0}%", x * 100.0)
                    }
                }),
                "look" => knob.format(|x| format!("{x:.2} ms")),
                _ => knob,
            };
            let r = ui
                .add(knob)
                .on_hover_text(t(&format!("{}_tip", info.label_key)));
            if r.changed() {
                app.set_fx_param(strip, effect, key, v, true);
            }
            if r.drag_stopped() || r.double_clicked() {
                app.end_gesture();
            }
        }
    });
}

/// Delivery: true peak, DC, dither and its shaping, what to monitor, the trigger.
fn delivery_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &LimSettings,
    tel: &[f32],
) {
    let pal = theme::p();
    let caption = |ui: &mut egui::Ui, key: &str| {
        ui.label(
            RichText::new(t(key))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
    };
    ui.horizontal(|ui| {
        let mut tp = s.true_peak;
        if ui
            .toggle_value(&mut tp, t("fx.lim.tp"))
            .on_hover_text(t("fx.lim.tp_tip"))
            .changed()
        {
            app.set_fx_param(strip, effect, "tp", if tp { 1.0 } else { 0.0 }, false);
        }
        let mut dc = s.dc_filter;
        if ui
            .toggle_value(&mut dc, t("fx.lim.dc"))
            .on_hover_text(t("fx.lim.dc_tip"))
            .changed()
        {
            app.set_fx_param(strip, effect, "dc", if dc { 1.0 } else { 0.0 }, false);
        }
        ui.separator();
        caption(ui, "fx.lim.dither");
        let bits_label = |i: usize| {
            if i == 0 {
                t("lim.off")
            } else {
                tf("lim.bits", &[("n", &DITHER_BITS[i])])
            }
        };
        egui::ComboBox::from_id_salt(("lim-dither", effect.0))
            .selected_text(bits_label(s.dither))
            .width(space::GRID * 16.0)
            .show_ui(ui, |ui| {
                for i in 0..DITHER_BITS.len() {
                    if ui.selectable_label(s.dither == i, bits_label(i)).clicked() {
                        app.set_fx_param(strip, effect, "dither", i as f32, false);
                    }
                }
            })
            .response
            .on_hover_text(t("fx.lim.dither_tip"));
        ui.add_enabled_ui(s.dither > 0, |ui| {
            let shape = slot.spec.get("shape").round() as usize;
            egui::ComboBox::from_id_salt(("lim-shape", effect.0))
                .selected_text(t(&format!("lim.shape.{shape}")))
                .width(space::GRID * 22.0)
                .show_ui(ui, |ui| {
                    for i in 0..3 {
                        if ui
                            .selectable_label(shape == i, t(&format!("lim.shape.{i}")))
                            .on_hover_text(t(&format!("lim.shape.{i}_tip")))
                            .clicked()
                        {
                            app.set_fx_param(strip, effect, "shape", i as f32, false);
                        }
                    }
                })
                .response
                .on_hover_text(t("fx.lim.shape_tip"));
        });
        ui.separator();
        caption(ui, "fx.lim.monitor");
        for (i, key) in ["lim.monitor.0", "lim.monitor.1", "lim.monitor.2"]
            .iter()
            .enumerate()
        {
            let on = matches!(
                (i, s.monitor),
                (0, Monitor::Normal) | (1, Monitor::Unity) | (2, Monitor::Match)
            );
            if ui
                .selectable_label(on, t(key))
                .on_hover_text(t(&format!("{key}_tip")))
                .clicked()
            {
                app.set_fx_param(strip, effect, "monitor", i as f32, false);
            }
        }
        if s.monitor == Monitor::Match {
            ui.label(
                RichText::new(format!("{} dB", fmt_db(tel[T_MATCH])))
                    .text_style(design::mono_small())
                    .color(pal.text),
            );
        }
        let mut delta = s.delta;
        if ui
            .toggle_value(&mut delta, t("fx.lim.delta"))
            .on_hover_text(t("fx.lim.delta_tip"))
            .changed()
        {
            app.set_fx_param(strip, effect, "delta", if delta { 1.0 } else { 0.0 }, false);
        }
        ui.separator();
        caption(ui, "lim.trigger");
        let current = slot.sidechain;
        let label = match current {
            Some(k) => app.capture_label(cantus_engine::CaptureTarget::Strip(k)),
            None => t("lim.trigger_own"),
        };
        egui::ComboBox::from_id_salt(("lim-key", effect.0))
            .selected_text(label)
            .width(space::GRID * 30.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current.is_none(), t("lim.trigger_own"))
                    .clicked()
                {
                    app.exec(Command::SetEffectSidechain {
                        strip,
                        effect,
                        source: None,
                    });
                }
                let options: Vec<StripId> = app
                    .project()
                    .strips
                    .iter()
                    .map(|x| x.id)
                    .filter(|id| {
                        *id != strip && !app.project().routing_would_cycle(*id, Some(strip))
                    })
                    .collect();
                for id in options {
                    let text = app.capture_label(cantus_engine::CaptureTarget::Strip(id));
                    if ui.selectable_label(current == Some(id), text).clicked() {
                        app.exec(Command::SetEffectSidechain {
                            strip,
                            effect,
                            source: Some(id),
                        });
                    }
                }
            })
            .response
            .on_hover_text(t("lim.trigger_tip"));
    });
}

// ---- the inspector's picture ----------------------------------------------------------

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let pal = theme::p();
    let s = settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    // The static curve: input level across, output up, with the gain applied — a flat top
    // at the ceiling is what a limiter is.
    let sq = Rect::from_min_size(rect.min, vec2(h, h)).shrink(space::S1);
    let lo = -36.0f32;
    let x = |d: f32| sq.left() + (d - lo) / -lo * sq.width();
    let y = |d: f32| sq.bottom() - ((d - lo) / -lo).clamp(0.0, 1.0) * sq.height();
    p.line_segment(
        [pos2(x(lo), y(lo)), pos2(x(0.0), y(0.0))],
        Stroke::new(1.0, pal.grid_bar),
    );
    let pts: Vec<Pos2> = (0..=48)
        .map(|i| {
            let d = lo + -lo * i as f32 / 48.0;
            pos2(x(d), y(transfer_db(d + s.gain_db, s.ceiling_db, s.knee_db)))
        })
        .collect();
    p.add(Shape::line(pts, Stroke::new(1.5, pal.selection)));
    p.hline(sq.x_range(), y(s.ceiling_db), Stroke::new(1.0, pal.gold));
    let lat = latency_of(&s, 48_000.0) as f32 / 48.0;
    let unit = if s.true_peak { "dBTP" } else { "dB" };
    p.text(
        pos2(sq.right() + space::S2, rect.center().y - 7.0),
        Align2::LEFT_CENTER,
        format!(
            "{} \u{b7} +{:.1} dB \u{2192} {} {unit}",
            t(&format!("lim.style.{}", s.style.index())),
            s.gain_db,
            fmt_db(s.ceiling_db)
        ),
        design::paint::ruler(),
        pal.text,
    );
    let profile = profile_of(spec.get("target"), s.ceiling_db)
        .map(|p| t(p.key))
        .unwrap_or_else(|| format!("{} LUFS", fmt_lufs(spec.get("target") as f64)));
    p.text(
        pos2(sq.right() + space::S2, rect.center().y + 7.0),
        Align2::LEFT_CENTER,
        tf(
            "lim.preview_line",
            &[("target", &profile), ("ms", &format!("{lat:.1}"))],
        ),
        design::paint::ruler(),
        pal.text_dim,
    );
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(
            1.0,
            if resp.hovered() {
                pal.gold
            } else {
                pal.grid_strong
            },
        ),
        StrokeKind::Inside,
    );
    resp.on_hover_text(t("lim.open_tip"))
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
