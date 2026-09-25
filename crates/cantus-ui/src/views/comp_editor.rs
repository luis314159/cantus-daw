//! The Comprimo editor: the compressor's own window, where what you see is what you drag.
//!
//! The research on the benchmark compressor found one thing above all: you can *see* the
//! compression there but not *touch* it — its displays are read-only, its styles are
//! black boxes, its auto gain is a guess and its loudness is momentary only
//! (`research/fabfilter/pro-c.md`, "How we beat it"). So here:
//!
//! - **The display is the control.** The threshold is a line you drag on the level
//!   history and a point you drag on the transfer curve; the ratio is the slope you drag;
//!   the knee is two marks; the range is the dashed floor under the curve; the wheel over
//!   the curve widens the knee.
//! - **Styles are visible detectors.** Each names its detector in words, and the engine
//!   drawer shows the five numbers behind it — peak or RMS window, feedback, program
//!   release, a second release stage, the release curve. Editing one makes the style
//!   "Custom (from …)". A **response graph** draws what the detector does to a burst, with
//!   the attack and release it measures.
//! - **A key you can shape and hear**: from this strip or any other, through a level, a
//!   high-pass and a six-band EQ with its own bypass, drawn over the key's spectrum.
//! - **Listening modes named for what they do**: Output, Wet, Key, Delta — and Compare,
//!   the bypass at the output's measured loudness.
//! - **Measured numbers**: auto gain from gated short-term loudness; momentary,
//!   short-term and integrated loudness on the output; ten seconds of gain-reduction
//!   statistics and the crest factor in and out.
//! - **A history you can read**: fast, slow or the whole take; bars and beats from the
//!   transport; frozen when the transport stops; scroll back with the wheel; hover to read
//!   input, output and reduction at any moment.
//!
//! Every drag and knob turn is one step of the project's undo.

use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use crate::views::fx::FactoryPreset;
use crate::widgets::Knob;
use cantus_core::fx::compressor::{key_band_key, KEY_BANDS, KEY_BAND_FIELDS};
use cantus_core::{Command, EffectId, EffectKind, EffectSlot, EffectSpec, MergeKey, StripId};
use cantus_dsp::aequalis::{design as design_band, Coefs, MAX_SECTIONS};
use cantus_dsp::comprimo::{
    effective_gain_reduction_db, loudness, step_response, tele, CompSettings, Listen, StepResponse,
    Style, StyleSpec, RATIO_INF,
};
use cantus_dsp::{EqShape, SpectrumAnalyser};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};
use std::collections::VecDeque;

/// Columns of history kept: at a frame a column, a little over a minute.
const HISTORY_MAX: usize = 4096;
/// Columns across the history in the Fast and Slow speeds.
const VISIBLE: usize = 480;
/// Frames folded into one column at the Slow speed.
const SLOW_FRAMES: u32 = 4;
/// The scales the displays and meters can show, dB.
pub const RANGES: [f32; 5] = [24.0, 36.0, 48.0, 60.0, 72.0];
/// The ratio's click stops; the last is ∞.
pub const RATIO_STOPS: [f32; 10] = [1.0, 1.5, 2.0, 3.0, 4.0, 6.0, 8.0, 10.0, 20.0, RATIO_INF];
/// How close to a stop (as a share of the knob's travel) a ratio snaps to it.
const RATIO_SNAP: f32 = 0.012;
/// The key EQ's display: frequency span, gain span (±), points evaluated.
const KEY_LO_HZ: f32 = 20.0;
const KEY_HI_HZ: f32 = 20_000.0;
const KEY_DB: f32 = 24.0;
const KEY_POINTS: usize = 160;
const DISPLAY_SR: f32 = 48_000.0;
/// A wheel burst ends this long after its last notch, s.
const WHEEL_GESTURE_S: f64 = 0.5;
/// The response graph's simulation rate: the time constants are in ms, so a lower rate
/// draws the same curve for a sixth of the work.
const RESPONSE_SR: f32 = 16_000.0;
const RESPONSE_POINTS: usize = 220;

/// Factory presets: a label key and the parameters it sets (`views::fx::presets_menu`).
/// What a preset does not name goes back to its default.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "comp.preset.vocal",
        &[
            ("style", 4.0),
            ("thr", -24.0),
            ("ratio", 3.0),
            ("knee", 12.0),
            ("att", 5.0),
            ("rel", 150.0),
            ("auto_rel", 1.0),
            ("auto_gain", 1.0),
            ("sc0.used", 1.0),
            ("sc0.shape", 3.0),
            ("sc0.freq", 120.0),
            ("sc0.slope", 24.0),
        ],
    ),
    (
        "comp.preset.drum_bus",
        &[
            ("style", 3.0),
            ("thr", -20.0),
            ("ratio", 4.0),
            ("knee", 6.0),
            ("att", 20.0),
            ("rel", 100.0),
            ("mix", 0.8),
            ("auto_gain", 1.0),
        ],
    ),
    (
        "comp.preset.parallel",
        &[
            ("style", 1.0),
            ("thr", -36.0),
            ("ratio", 10.0),
            ("knee", 0.0),
            ("att", 1.0),
            ("rel", 60.0),
            ("makeup", 12.0),
            ("dry", 0.0),
        ],
    ),
    (
        "comp.preset.glue",
        &[
            ("style", 6.0),
            ("thr", -18.0),
            ("ratio", 2.0),
            ("knee", 18.0),
            ("att", 30.0),
            ("rel", 300.0),
            ("auto_gain", 1.0),
        ],
    ),
    (
        "comp.preset.master",
        &[
            ("style", 5.0),
            ("thr", -12.0),
            ("ratio", 1.5),
            ("knee", 24.0),
            ("att", 30.0),
            ("rel", 400.0),
            ("look", 5.0),
            ("auto_gain", 1.0),
        ],
    ),
    (
        "comp.preset.duck",
        &[
            ("thr", -30.0),
            ("ratio", 10.0),
            ("knee", 6.0),
            ("range", 12.0),
            ("att", 1.0),
            ("hold", 10.0),
            ("rel", 180.0),
        ],
    ),
    (
        "comp.preset.pump",
        &[
            ("style", 7.0),
            ("thr", -30.0),
            ("ratio", 20.0),
            ("att", 0.5),
            ("rel", 240.0),
            ("auto_gain", 1.0),
        ],
    ),
    (
        "comp.preset.bass",
        &[
            ("style", 2.0),
            ("thr", -22.0),
            ("ratio", 4.0),
            ("knee", 10.0),
            ("att", 15.0),
            ("rel", 200.0),
            ("auto_gain", 1.0),
        ],
    ),
];

/// How fast the history scrolls.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Speed {
    /// A column a frame: about eight seconds across.
    Fast,
    /// Four frames a column: about half a minute across.
    Slow,
    /// Everything since the editor opened, squeezed to fit: the whole take.
    Whole,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    Threshold,
    Ratio,
    Knee,
    Range,
}

/// One column of the history.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
struct Column {
    /// Input and output peak, dB; gain reduction, dB (positive).
    inp: f32,
    out: f32,
    gr: f32,
    /// Where the transport was, in ticks, if it was playing.
    tick: Option<i64>,
}

impl Column {
    /// The louder and the deeper of two columns: how columns fold together.
    fn fold(self, o: Column) -> Column {
        Column {
            inp: self.inp.max(o.inp),
            out: self.out.max(o.out),
            gr: self.gr.max(o.gr),
            tick: self.tick.or(o.tick),
        }
    }
}

/// The screenshot's stand-in transport: every fed column advances it by `step` ticks.
#[derive(Copy, Clone, Debug)]
struct DemoClock {
    tick: f64,
    step: f64,
}

/// What the editor remembers between frames.
pub struct CompEditor {
    pub open: Option<(StripId, EffectId)>,
    history: VecDeque<Column>,
    /// Frames folded so far into the column being built (Slow and Whole).
    pending: Option<(Column, u32)>,
    /// Frames per column in Whole; doubles each time the history fills.
    whole_frames: u32,
    pub speed: Speed,
    /// Bars and beats behind the history.
    pub grid: bool,
    /// Stop scrolling (a click on the history, or the transport stopped and silent).
    pub frozen: bool,
    /// Columns scrolled back from the newest (the wheel over the history).
    scroll: usize,
    /// The dB span of the displays and meters.
    pub range_db: f32,
    /// The engine drawer.
    pub engine_open: bool,
    drag: Option<Grab>,
    /// The operating point on the transfer curve, from the last frame's meter.
    now: (f32, f32),
    /// The meters' reading this frame (the history may be frozen; the meters never are),
    /// and their held peaks: input, reduction, output.
    live: Column,
    hold: [f32; 3],
    /// The key EQ's selected band.
    key_band: Option<usize>,
    key_drag: Option<usize>,
    analyser: SpectrumAnalyser,
    scratch: Vec<f32>,
    /// Momentary loudness blocks of the output since the last reset, for the integrated
    /// reading (BS.1770's gating runs over these).
    pub blocks: Vec<f32>,
    last_hops: f32,
    /// The response graph for the settings it was drawn for.
    response: Option<(CompSettings, StepResponse)>,
    demo: Option<DemoClock>,
    /// Where the last frame drew the displays, for tests that drive them.
    geometry: CompGeometry,
    /// The height the rows under the displays took in the last frame.
    below: f32,
    /// The UI clock, and when the wheel last moved something (a burst is one undo step).
    clock: f64,
    wheel_at: Option<f64>,
}

/// Where the editor drew its displays in the last frame: tests aim their gestures with it
/// instead of mirroring the layout.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CompGeometry {
    pub history: Option<Rect>,
    pub transfer: Option<Rect>,
    pub key_eq: Option<Rect>,
    /// The dB span of the history and the transfer curve.
    pub range_db: f32,
}

impl CompGeometry {
    /// A point on the transfer display: input level across, output level up, dB.
    pub fn transfer_point(&self, input_db: f32, output_db: f32) -> Option<Pos2> {
        let r = self.transfer?;
        let lo = -self.range_db;
        Some(pos2(
            r.left() + (input_db - lo) / -lo * r.width(),
            r.bottom() - (output_db - lo) / -lo * r.height(),
        ))
    }

    /// A point on the key EQ's display: frequency across, gain up.
    pub fn key_eq_point(&self, hz: f32, gain_db: f32) -> Option<Pos2> {
        let r = self.key_eq?;
        let span = (KEY_HI_HZ / KEY_LO_HZ).ln();
        Some(pos2(
            r.left() + (hz / KEY_LO_HZ).ln() / span * r.width(),
            r.center().y - gain_db / KEY_DB * r.height() * 0.45,
        ))
    }
}

impl std::fmt::Debug for CompEditor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompEditor")
            .field("open", &self.open)
            .field("speed", &self.speed)
            .field("columns", &self.history.len())
            .finish_non_exhaustive()
    }
}

impl Default for CompEditor {
    fn default() -> Self {
        CompEditor {
            open: None,
            history: VecDeque::with_capacity(HISTORY_MAX),
            pending: None,
            whole_frames: 1,
            speed: Speed::Fast,
            grid: true,
            frozen: false,
            scroll: 0,
            range_db: 48.0,
            engine_open: false,
            drag: None,
            now: (-120.0, -120.0),
            live: Column {
                inp: -120.0,
                out: -120.0,
                gr: 0.0,
                tick: None,
            },
            hold: [0.0; 3],
            key_band: None,
            key_drag: None,
            analyser: SpectrumAnalyser::new(4096),
            scratch: Vec::new(),
            blocks: Vec::new(),
            last_hops: 0.0,
            response: None,
            demo: None,
            geometry: CompGeometry::default(),
            below: 0.0,
            clock: 0.0,
            wheel_at: None,
        }
    }
}

fn db(lin: f32) -> f32 {
    if lin > 1e-6 {
        20.0 * lin.log10()
    } else {
        -120.0
    }
}

/// `4.0:1`, `∞:1`.
pub fn ratio_text(r: f32) -> String {
    if r >= RATIO_INF - 0.05 {
        "\u{221e}:1".to_string()
    } else {
        format!("{r:.1}:1")
    }
}

/// A ratio pulled to the nearest click stop when it is within a hair of it on the knob's
/// (logarithmic) travel.
pub fn snap_ratio(r: f32) -> f32 {
    let pos = |x: f32| x.max(1.0).ln() / RATIO_INF.ln();
    RATIO_STOPS
        .iter()
        .copied()
        .find(|s| (pos(*s) - pos(r)).abs() < RATIO_SNAP)
        .unwrap_or(r)
}

/// A dB reading with a sign, or a dash for "nothing measured".
fn lufs_text(v: f32) -> String {
    if v <= loudness::SILENCE + 1.0 || !v.is_finite() {
        "\u{2014}".to_string()
    } else {
        format!("{v:.1}")
    }
}

impl CompEditor {
    /// Add one column of history directly — what the engine's meter does each frame, for
    /// screenshots and tests with no engine. Levels linear, reduction in dB.
    pub fn feed(&mut self, input: f32, output: f32, gr_db: f32) {
        let tick = self.demo.as_mut().map(|d| {
            d.tick += d.step;
            d.tick as i64
        });
        let col = Column {
            inp: db(input),
            out: db(output),
            gr: gr_db,
            tick,
        };
        self.meter(col);
        self.push(col);
        if input > 0.0 {
            self.now = (db(input), db(input) - gr_db);
        }
    }

    /// One frame's column, folded as the speed says.
    fn push(&mut self, col: Column) {
        let frames = match self.speed {
            Speed::Fast => 1,
            Speed::Slow => SLOW_FRAMES,
            Speed::Whole => self.whole_frames,
        };
        let (acc, n) = match self.pending.take() {
            Some((acc, n)) => (acc.fold(col), n + 1),
            None => (col, 1),
        };
        if n < frames {
            self.pending = Some((acc, n));
            return;
        }
        if self.history.len() == HISTORY_MAX {
            if self.speed == Speed::Whole {
                // The whole take no longer fits: fold every pair of columns into one and
                // take twice as many frames per column from now on.
                let old: Vec<Column> = self.history.drain(..).collect();
                for pair in old.chunks(2) {
                    let c = pair
                        .iter()
                        .copied()
                        .reduce(Column::fold)
                        .unwrap_or_default();
                    self.history.push_back(c);
                }
                self.whole_frames = self.whole_frames.saturating_mul(2);
            } else {
                self.history.pop_front();
            }
        }
        self.history.push_back(acc);
    }

    /// This frame's meter reading: the meters and their held peaks follow it.
    fn meter(&mut self, col: Column) {
        self.live = col;
        for (h, v) in self.hold.iter_mut().zip([col.inp, col.gr, col.out]) {
            *h = h.max(v);
        }
    }

    /// Start the history over, at `speed`.
    pub fn set_speed(&mut self, speed: Speed) {
        if self.speed != speed {
            self.speed = speed;
            self.pending = None;
            self.scroll = 0;
            if speed == Speed::Whole {
                self.history.clear();
                self.whole_frames = 1;
            }
        }
    }

    /// The columns on screen: the last `VISIBLE` (less the scroll), or all of them in
    /// Whole.
    fn visible(&self) -> (usize, usize) {
        let len = self.history.len();
        if self.speed == Speed::Whole {
            return (0, len);
        }
        let end = len.saturating_sub(self.scroll);
        (end.saturating_sub(VISIBLE), end)
    }

    /// The integrated loudness of the output since the last reset, LUFS.
    pub fn integrated(&self) -> f32 {
        loudness::integrated(&self.blocks)
    }

    fn reset_loudness(&mut self) {
        self.blocks.clear();
    }

    /// Where the last frame drew the displays.
    pub fn geometry(&self) -> CompGeometry {
        self.geometry
    }

    /// Columns of history kept, and frames per column in the Whole speed.
    pub fn columns(&self) -> (usize, u32) {
        (self.history.len(), self.whole_frames)
    }
}

impl CantusApp {
    pub fn open_comp_editor(&mut self, strip: StripId, effect: EffectId) {
        let ed = &mut self.comp_editor;
        ed.open = Some((strip, effect));
        ed.history.clear();
        ed.pending = None;
        ed.whole_frames = 1;
        ed.scroll = 0;
        ed.hold = [-120.0, 0.0, -120.0];
        ed.live = Column {
            inp: -120.0,
            out: -120.0,
            gr: 0.0,
            tick: None,
        };
        ed.blocks.clear();
        ed.last_hops = 0.0;
        ed.analyser = SpectrumAnalyser::new(4096);
        ed.response = None;
        ed.key_band = None;
        self.engine.set_analyser(Some(effect));
    }

    pub fn close_comp_editor(&mut self) {
        if self.comp_editor.open.take().is_some() {
            self.engine.set_analyser(None);
        }
        // A demo's stand-in telemetry must not outlive it.
        if self.comp_editor.demo.take().is_some() {
            self.fx.injected = None;
        }
    }

    /// Open whichever editor an effect has.
    pub fn open_effect_editor(&mut self, strip: StripId, effect: EffectId) {
        let kind = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.kind);
        match kind {
            Some(EffectKind::Eq) => {
                self.close_comp_editor();
                self.close_fx_editor();
                self.open_eq_editor(strip, effect);
            }
            Some(EffectKind::Compressor) => {
                self.close_eq_editor();
                self.close_fx_editor();
                self.open_comp_editor(strip, effect);
            }
            Some(kind) if kind.has_editor() => {
                self.close_eq_editor();
                self.close_comp_editor();
                self.open_fx_editor(strip, effect, kind);
            }
            _ => {}
        }
    }

    fn set_comp(&mut self, strip: StripId, effect: EffectId, key: &str, value: f32, gesture: bool) {
        let cmd = Command::SetEffectParam {
            strip,
            effect,
            key: key.to_string(),
            value,
        };
        if gesture {
            self.exec_merging(cmd, MergeKey::of(&("comp", effect.0, key)));
        } else {
            self.exec(cmd);
        }
    }

    /// Set several parameters as one command; a gesture (`merge`) folds frame after frame
    /// into one undo step.
    fn set_comp_many(
        &mut self,
        strip: StripId,
        effect: EffectId,
        values: &[(String, f32)],
        label_key: &str,
        merge: Option<MergeKey>,
    ) {
        let commands: Vec<Command> = values
            .iter()
            .map(|(k, v)| Command::SetEffectParam {
                strip,
                effect,
                key: k.clone(),
                value: *v,
            })
            .collect();
        if commands.is_empty() {
            return;
        }
        let cmd = Command::Batch {
            label_key: label_key.into(),
            commands,
        };
        match merge {
            Some(key) => self.exec_merging(cmd, key),
            None => self.exec(cmd),
        };
    }

    /// Forget a key-EQ band: every field it stored goes, as one command.
    pub fn remove_key_band(&mut self, strip: StripId, effect: EffectId, band: usize) {
        let Some(spec) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.clone())
        else {
            return;
        };
        let commands: Vec<Command> = KEY_BAND_FIELDS
            .iter()
            .map(|f| key_band_key(band, f.0))
            .filter(|k| spec.params.contains_key(k))
            .map(|key| Command::ClearEffectParam { strip, effect, key })
            .collect();
        if !commands.is_empty() {
            self.exec(Command::Batch {
                label_key: "cmd.comp_key_band".into(),
                commands,
            });
        }
        if self.comp_editor.key_band == Some(band) {
            self.comp_editor.key_band = None;
        }
    }

    /// Put a Comprimo on `strip`, keyed from `key`, set to duck — fast attack, a short
    /// hold, a release of an eighth note at the project's tempo, 12 dB of range so the
    /// ducked strip dips rather than vanishes — as one step of undo. FL users do this
    /// constantly; here it is one menu item on the strip (`research/fabfilter/pro-c.md`,
    /// P1-12). Returns the new effect, or `None` when the key would close a loop.
    pub fn duck_strip_from(&mut self, strip: StripId, key: StripId) -> Option<EffectId> {
        let bpm = self.project().tempo_bpm as f32;
        let eighth_ms = (30_000.0 / bpm.max(1.0)).clamp(40.0, 600.0);
        let mut slot = self.bus.new_effect(EffectKind::Compressor);
        for (k, v) in [
            ("thr", -30.0),
            ("ratio", 10.0),
            ("knee", 6.0),
            ("range", 12.0),
            ("att", 1.0),
            ("hold", 10.0),
            ("rel", eighth_ms),
        ] {
            slot.spec.set(k, v);
        }
        let effect = slot.id;
        let index = self.project().strip(strip)?.effects.len();
        let ok = self.exec(Command::Batch {
            label_key: "cmd.comp_duck".into(),
            commands: vec![
                Command::InsertEffect {
                    strip,
                    index,
                    slot: Box::new(slot),
                },
                Command::SetEffectSidechain {
                    strip,
                    effect,
                    source: Some(key),
                },
            ],
        });
        ok.then_some(effect)
    }

    /// The gain reduction a strip's compressors are applying now, dB, if the engine
    /// reports it: today the one whose editor is open (its telemetry). `None` when the
    /// strip has no compressor, `Some(None)` when it has one the engine does not report.
    pub fn comp_gr_on_strip(&self, strip: StripId) -> Option<Option<f32>> {
        let s = self.project().strip(strip)?;
        let comps: Vec<EffectId> = s
            .effects
            .iter()
            .filter(|e| e.spec.kind == EffectKind::Compressor && !e.bypass)
            .map(|e| e.id)
            .collect();
        if comps.is_empty() {
            return None;
        }
        Some(match self.comp_editor.open {
            Some((st, fx)) if st == strip && comps.contains(&fx) => {
                let t = self.fx_telemetry();
                (t[tele::SAMPLE_RATE] > 0.0).then_some(t[tele::GR])
            }
            // Every other strip: the engine's per-strip reduction (the deepest of its
            // working compressors), in the same order as the desk's meters.
            _ => {
                let index = self.project().strips.iter().position(|s| s.id == strip)?;
                self.engine.meters().strip_gr.get(index).copied()
            }
        })
    }
}

/// The engine drawer's five keys and a detector's numbers for them.
fn engine_values(d: &StyleSpec) -> [(&'static str, f32); 5] {
    [
        ("det_rms", d.rms_ms.max(0.1)),
        ("det_fb", d.feedback),
        ("det_prog", d.program_release),
        ("det_dual", d.dual),
        ("det_curve", d.curve),
    ]
}

impl CantusApp {
    /// Pick a style: its own detector, leaving any custom engine — one step of undo.
    pub fn set_comp_style(&mut self, strip: StripId, effect: EffectId, style: Style) {
        self.set_comp_many(
            strip,
            effect,
            &[
                ("style".into(), style.index() as f32),
                ("custom".into(), 0.0),
            ],
            "cmd.comp_style",
            None,
        );
    }

    /// Set one of the engine drawer's numbers. The first move away from a style starts
    /// from the style's own numbers and switches the engine to custom, in the same step
    /// of undo; a gesture folds frame after frame into it.
    pub fn set_comp_engine(
        &mut self,
        strip: StripId,
        effect: EffectId,
        key: &str,
        value: f32,
        gesture: bool,
    ) {
        let Some(spec) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.clone())
        else {
            return;
        };
        let s = cantus_engine::comp_settings(&spec);
        let mut values: Vec<(String, f32)> = Vec::new();
        if s.engine.is_none() {
            values.push(("custom".into(), 1.0));
            for (k, x) in engine_values(&s.detector()) {
                values.push((k.into(), x));
            }
        }
        values.push((key.into(), value));
        let merge = gesture.then(|| MergeKey::of(&("comp-engine", effect.0, key.to_string())));
        self.set_comp_many(strip, effect, &values, "cmd.comp_engine", merge);
    }
}

/// What a style's detector is, in words — the answer to "what does Opto mean?".
pub fn style_words(style: Style) -> String {
    detector_words(&style.spec())
}

/// What a detector is, in words: detector, feedback, release.
pub fn detector_words(s: &StyleSpec) -> String {
    let detector = if s.is_peak() {
        t("comp.peak")
    } else {
        tf("comp.rms", &[("ms", &format!("{:.0}", s.rms_ms))])
    };
    let feedback = if s.feedback > 0.0 {
        tf(
            "comp.feedback",
            &[("pct", &format!("{:.0}", s.feedback * 100.0))],
        )
    } else {
        t("comp.feedforward")
    };
    let mut release = vec![if s.program_release > 0.0 {
        t("comp.release_program")
    } else {
        t("comp.release_set")
    }];
    if s.dual > 0.0 {
        release.push(tf(
            "comp.release_dual",
            &[("pct", &format!("{:.0}", s.dual * 100.0))],
        ));
    }
    if s.curve > 0.0 {
        release.push(tf(
            "comp.release_linear",
            &[("pct", &format!("{:.0}", s.curve * 100.0))],
        ));
    }
    format!("{detector} \u{b7} {feedback} \u{b7} {}", release.join(", "))
}

/// The style's name as shown: its own, or "Custom (from Opto)".
pub fn style_name(s: &CompSettings) -> String {
    let base = t(&format!("comp.style.{}", s.style.index()));
    if s.engine.is_some() {
        tf("comp.custom_from", &[("style", &base)])
    } else {
        base
    }
}

/// The telemetry, read by name.
#[derive(Copy, Clone)]
struct Tele([f32; 64]);

impl Tele {
    /// Whether an engine (or a demo) is reporting at all.
    fn live(&self) -> bool {
        self.0[tele::SAMPLE_RATE] > 0.0
    }
    fn get(&self, i: usize) -> f32 {
        self.0[i]
    }
}

pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some((strip, effect)) = app.comp_editor.open else {
        return;
    };
    let Some(slot) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .cloned()
    else {
        app.close_comp_editor();
        return;
    };
    if app.comp_editor.demo.is_some() && app.fx.injected.is_none() {
        demo_feed(app);
    }
    app.comp_editor.clock = ctx.input(|i| i.time);
    pump(app);
    ctx.request_repaint();

    let s = cantus_engine::comp_settings(&slot.spec);
    let tel = Tele(app.fx_telemetry());
    let strip_name = app.capture_label(cantus_engine::CaptureTarget::Strip(strip));
    let mut open = true;
    egui::Window::new(format!("{} \u{2014} {strip_name}", t("effect.compressor")))
        .id(egui::Id::new(("comp-editor", effect.0)))
        .open(&mut open)
        .default_size([1180.0, 860.0])
        .min_size([760.0, 560.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| body(app, ui, strip, effect, &slot, &s, &tel));
    if !open {
        app.close_comp_editor();
    }
}

/// Once a frame: the engine's meter into the history, the analyser tap into the key's
/// spectrum, new loudness blocks into the integrated reading.
fn pump(app: &mut CantusApp) {
    // A real engine reporting ends a demo (`--editor comp` with a device): its numbers win.
    if app.comp_editor.demo.is_some() && app.engine.meters().fx[tele::SAMPLE_RATE] > 0.0 {
        app.comp_editor.demo = None;
        app.fx.injected = None;
    }
    // A wheel burst is one step of undo: seal it half a second after the last notch.
    let now = app.comp_editor.clock;
    if app
        .comp_editor
        .wheel_at
        .is_some_and(|t| now - t > WHEEL_GESTURE_S)
    {
        app.comp_editor.wheel_at = None;
        app.end_gesture();
    }
    let [inp, out, gr] = app.engine.take_comp_meter();
    let playing = app.engine.is_playing();
    let tick = playing.then(|| app.engine.position().raw());
    let tel = Tele(app.fx_telemetry());
    let demo = app.comp_editor.demo.is_some();
    let ed = &mut app.comp_editor;
    let col = Column {
        inp: db(inp),
        out: db(out),
        gr,
        tick,
    };
    if !demo {
        ed.meter(col);
        // Frozen by hand, or stopped and silent: the picture of the last take stays.
        let silent = inp < 1e-5 && gr <= 0.0;
        if !ed.frozen && !(silent && !playing) {
            ed.push(col);
        }
        if inp > 0.0 {
            ed.now = (db(inp), db(inp) - gr);
        }
    }
    // The held peaks fall at 12 dB/s (about 0.2 dB a frame); reduction at the same rate.
    ed.hold[0] = (ed.hold[0] - 0.2).max(-120.0);
    ed.hold[1] = (ed.hold[1] - 0.2).max(0.0);
    ed.hold[2] = (ed.hold[2] - 0.2).max(-120.0);
    let mut pairs = std::mem::take(&mut ed.scratch);
    pairs.clear();
    app.engine.drain_analyser(&mut pairs);
    if !pairs.is_empty() {
        app.comp_editor.analyser.push(&pairs);
        app.comp_editor.analyser.compute();
    }
    app.comp_editor.scratch = pairs;
    // The loudness blocks: the telemetry carries the last eight with a step counter, so
    // a slow frame loses none. Integrated loudness counts what played.
    let ed = &mut app.comp_editor;
    if tel.live() {
        let hops = tel.get(tele::HOPS);
        if hops < ed.last_hops {
            ed.last_hops = 0.0; // the engine was reset
        }
        let new = ((hops - ed.last_hops) as usize).min(cantus_dsp::comprimo::BLOCKS_REPORTED);
        if (playing || demo) && ed.last_hops > 0.0 {
            for i in (0..new).rev() {
                ed.blocks.push(tel.get(tele::BLOCKS + i));
            }
        }
        ed.last_hops = hops;
    }
}

/// The height of the knob rows and of the key row under the displays.
const CONTROLS_H: f32 = space::GRID * 28.0;
const KEY_H: f32 = space::GRID * 36.0;
/// The meters and numbers beside the transfer curve.
const METERS_W: f32 = space::GRID * 54.0;

fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &CompSettings,
    tel: &Tele,
) {
    // The family's frame: the strip, presets, A/B, and the bypass.
    crate::views::fx::header(app, ui, strip, effect, slot);
    ui.add_space(space::S1);
    style_row(app, ui, strip, effect, s);
    if app.comp_editor.engine_open {
        ui.add_space(space::S1);
        engine_row(app, ui, strip, effect, s);
    }
    ui.add_space(space::S1);
    toolbar(app, ui, strip, effect, s, tel);
    ui.add_space(space::S1);
    // The rows under the displays keep their height; the displays take what is left.
    // The rows are measured as drawn (last frame), so a row that grows never pushes the
    // window taller frame after frame.
    let fixed = app
        .comp_editor
        .below
        .max(CONTROLS_H + KEY_H + space::S2 * 3.0);
    let h = (ui.available_height() - fixed).max(space::GRID * 40.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::S2;
        let w = (ui.available_width() - h - METERS_W - space::S2 * 2.0).max(space::GRID * 40.0);
        history_display(app, ui, strip, effect, s, vec2(w, h));
        transfer_display(app, ui, strip, effect, s, tel, h);
        meters_panel(app, ui, s, tel, vec2(METERS_W, h));
    });
    let top = ui.cursor().top();
    ui.add_space(space::S2);
    controls(app, ui, strip, effect, &slot.spec, s);
    ui.add_space(space::S2);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = space::S2;
        key_section(app, ui, strip, effect, slot, s);
        let rest = ui.available_width();
        let response_w = (rest * 0.36).clamp(space::GRID * 56.0, space::GRID * 80.0);
        let eq_w = (rest - response_w - space::S2).max(space::GRID * 40.0);
        key_eq_panel(app, ui, strip, effect, slot, s, vec2(eq_w, KEY_H));
        response_graph(app, ui, s, vec2(response_w, KEY_H));
    });
    app.comp_editor.below = ui.cursor().top() - top + space::S2;
}

/// Text on a small plate of the panel colour, so it reads over a busy display.
fn plate_text(p: &egui::Painter, at: Pos2, align: Align2, text: String, color: Color32) {
    let pal = theme::p();
    let galley = p.layout_no_wrap(text, design::paint::ruler(), color);
    let rect = align.anchor_size(at, galley.size());
    p.rect_filled(
        rect.expand(space::S1 * 0.5),
        pal.corner_radius,
        pal.panel.gamma_multiply(0.92),
    );
    p.galley(rect.min, galley, color);
}

/// A small caption in the dim text colour.
fn caption(ui: &mut egui::Ui, key: &str) {
    ui.label(
        RichText::new(t(key))
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
}

/// The styles as a segmented row; the one in force is filled. The detector it means is
/// spelled out beside it, and the engine drawer opens from the right.
fn style_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &CompSettings,
) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        caption(ui, "fx.comp.style");
        for style in Style::ALL {
            let on = s.style == style && s.engine.is_none();
            let label = t(&format!("comp.style.{}", style.index()));
            let text = if s.style == style && s.engine.is_some() {
                // The base of a custom engine keeps a mark, so "from where" is visible.
                RichText::new(format!("{label}*")).italics()
            } else {
                RichText::new(label)
            };
            if ui
                .selectable_label(on, text)
                .on_hover_text(style_words(style))
                .clicked()
            {
                app.set_comp_style(strip, effect, style);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let open = app.comp_editor.engine_open;
            if ui
                .selectable_label(open, t("comp.engine"))
                .on_hover_text(t("comp.engine_tip"))
                .clicked()
            {
                app.comp_editor.engine_open = !open;
            }
            let words = detector_words(&s.detector());
            let text = if s.engine.is_some() {
                RichText::new(format!("{} \u{b7} {words}", style_name(s))).color(pal.gold)
            } else {
                RichText::new(words).color(pal.text_dim)
            };
            ui.add(egui::Label::new(text.text_style(design::caption())).truncate());
        });
    });
}

/// The engine drawer: the five numbers a style is, as knobs. Turning one makes the style
/// "Custom (from …)", starting from the style's own numbers — one undo step per gesture.
fn engine_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &CompSettings,
) {
    let pal = theme::p();
    let current = engine_values(&s.detector());
    egui::Frame::NONE
        .fill(pal.panel_raised)
        .stroke(Stroke::new(1.0, pal.grid_strong))
        .corner_radius(pal.corner_radius)
        .inner_margin(egui::Margin::same(space::S2 as i8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.set_width(space::GRID * 56.0);
                    ui.label(RichText::new(style_name(s)).text_style(design::strong()));
                    ui.label(
                        RichText::new(t(if s.engine.is_some() {
                            "comp.engine_custom"
                        } else {
                            "comp.engine_style"
                        }))
                        .text_style(design::caption())
                        .color(pal.text_dim),
                    );
                    if s.engine.is_some()
                        && ui
                            .button(tf(
                                "comp.engine_back",
                                &[("style", &t(&format!("comp.style.{}", s.style.index())))],
                            ))
                            .clicked()
                    {
                        app.set_comp(strip, effect, "custom", 0.0, false);
                    }
                });
                for (key, value) in current {
                    let Some(info) = EffectKind::Compressor.param(key) else {
                        continue;
                    };
                    let mut v = value;
                    let knob = Knob::new(&mut v, info.min..=info.max, info.default)
                        .diameter(space::GRID * 10.0)
                        .log(info.logarithmic)
                        .unit(info.unit)
                        .label(t(info.label_key));
                    let knob = match key {
                        "det_rms" => knob.format(|x| {
                            if x <= 0.1 {
                                t("comp.peak_short")
                            } else {
                                crate::widgets::knob::readout(x, "ms")
                            }
                        }),
                        "det_prog" => knob.format(|x| format!("\u{d7}{x:.1}")),
                        "det_curve" => knob.format(|x| {
                            if x <= 0.005 {
                                t("comp.curve_exp")
                            } else if x >= 0.995 {
                                t("comp.curve_lin")
                            } else {
                                format!("{:.0}% {}", x * 100.0, t("comp.curve_lin"))
                            }
                        }),
                        _ => knob,
                    };
                    let r = ui.add(knob).on_hover_text(t(&format!("comp.{key}_tip")));
                    if r.changed() {
                        app.set_comp_engine(strip, effect, key, v, true);
                    }
                    if r.drag_stopped() || r.double_clicked() {
                        app.end_gesture();
                    }
                }
            });
        });
}

/// Over the displays: how the history moves and what scale it shows, and — at the right —
/// what you hear, the equal-loudness compare, and the latency the lookahead costs.
fn toolbar(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &CompSettings,
    tel: &Tele,
) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        caption(ui, "comp.history");
        for (speed, key) in [
            (Speed::Fast, "comp.speed.fast"),
            (Speed::Slow, "comp.speed.slow"),
            (Speed::Whole, "comp.speed.whole"),
        ] {
            if ui
                .selectable_label(app.comp_editor.speed == speed, t(key))
                .on_hover_text(t(&format!("{key}_tip")))
                .clicked()
            {
                app.comp_editor.set_speed(speed);
            }
        }
        let ed = &mut app.comp_editor;
        if ui
            .selectable_label(ed.grid, t("comp.grid"))
            .on_hover_text(t("comp.grid_tip"))
            .clicked()
        {
            ed.grid = !ed.grid;
        }
        if ui
            .selectable_label(ed.frozen, t("comp.freeze"))
            .on_hover_text(t("comp.freeze_tip"))
            .clicked()
        {
            ed.frozen = !ed.frozen;
        }
        if ed.scroll > 0 && ui.button(t("comp.live")).clicked() {
            ed.scroll = 0;
        }
        ui.separator();
        caption(ui, "comp.scale");
        let mut range = ed.range_db;
        egui::ComboBox::from_id_salt(("comp-scale", effect.0))
            .selected_text(format!("{range:.0} dB"))
            .width(space::GRID * 18.0)
            .show_ui(ui, |ui| {
                for r in RANGES {
                    ui.selectable_value(&mut range, r, format!("{r:.0} dB"));
                }
            });
        ed.range_db = range;

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // The latency the lookahead costs, always in view: the research's most
            // repeated complaint is latency that appears silently.
            let sr = if tel.live() {
                tel.get(tele::SAMPLE_RATE)
            } else {
                DISPLAY_SR
            };
            let samples =
                s.lookahead_samples(sr) + cantus_dsp::comprimo::oversample::latency(s.oversample);
            let ms = samples as f32 / sr * 1_000.0;
            let badge = if samples == 0 {
                t("comp.latency_none")
            } else {
                tf(
                    "comp.latency",
                    &[("ms", &format!("{ms:.1}")), ("n", &samples.to_string())],
                )
            };
            ui.label(
                RichText::new(badge)
                    .text_style(design::mono_small())
                    .color(if samples == 0 { pal.text_dim } else { pal.gold }),
            )
            .on_hover_text(t("comp.latency_tip"));
            let os = cantus_dsp::comprimo::oversample::FACTORS
                .iter()
                .position(|f| *f == s.oversample)
                .unwrap_or(0);
            let mut pick = os;
            egui::ComboBox::from_id_salt(("comp-os", effect.0))
                .selected_text(t(&format!("comp.os.{os}")))
                .width(space::GRID * 12.0)
                .show_ui(ui, |ui| {
                    for i in 0..3 {
                        ui.selectable_value(&mut pick, i, t(&format!("comp.os.{i}")));
                    }
                })
                .response
                .on_hover_text(t("comp.os_tip"));
            if pick != os {
                app.set_comp(strip, effect, "os", pick as f32, false);
            }
            caption(ui, "fx.comp.os");
            ui.separator();
            let comparing = s.listen == Listen::Compare;
            let label = if comparing && tel.live() {
                tf(
                    "comp.compare_on",
                    &[("db", &format!("{:+.1}", tel.get(tele::COMPARE)))],
                )
            } else {
                t("comp.compare")
            };
            if ui
                .selectable_label(comparing, label)
                .on_hover_text(t("comp.compare_tip"))
                .clicked()
            {
                let to = if comparing {
                    Listen::Normal
                } else {
                    Listen::Compare
                };
                app.set_comp(strip, effect, "listen", to.index() as f32, false);
            }
            ui.separator();
            for listen in [Listen::Delta, Listen::Key, Listen::Wet, Listen::Normal] {
                let key = format!("comp.listen.{}", listen.index());
                if ui
                    .selectable_label(s.listen == listen, t(&key))
                    .on_hover_text(t(&format!("{key}_tip")))
                    .clicked()
                {
                    app.set_comp(strip, effect, "listen", listen.index() as f32, false);
                }
            }
            caption(ui, "comp.listen");
        });
    });
}

/// The level history: input as a dim fill, output as a line, gain reduction hanging from
/// the top in the danger colour, all on one dB scale; bars and beats behind; the
/// threshold as a dashed line you drag. Click to freeze, wheel to scroll back, hover to
/// read any moment.
fn history_display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &CompSettings,
    size: egui::Vec2,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let range = app.comp_editor.range_db;
    app.comp_editor.geometry.history = Some(rect);
    app.comp_editor.geometry.range_db = range;
    let y = |d: f32| rect.top() + (-d / range).clamp(0.0, 1.0) * rect.height();
    let d_of = |py: f32| -(py - rect.top()) / rect.height() * range;
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let step = if range <= 36.0 { 6.0 } else { 12.0 };
    let mut g = -step;
    while g > -range {
        p.hline(rect.x_range(), y(g), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(rect.left() + 3.0, y(g) - 1.0),
            Align2::LEFT_BOTTOM,
            format!("{g:.0}"),
            design::paint::ruler(),
            pal.text_dim,
        );
        g -= step;
    }
    let ed = &app.comp_editor;
    let (a, b) = ed.visible();
    let cols = b - a;
    let slots = if ed.speed == Speed::Whole {
        cols.max(2)
    } else {
        VISIBLE
    };
    let dx = rect.width() / slots as f32;
    let x_of = |i: usize| rect.right() - dx * (cols - i) as f32;
    let col = |i: usize| ed.history[a + i];
    // Bars and beats from the transport, behind everything.
    if ed.grid {
        let ts = app.project().time_signature;
        let (bar_len, beat_len) = (ts.ticks_per_bar().0, ts.ticks_per_beat().0);
        for i in 1..cols {
            let (Some(p0), Some(p1)) = (col(i - 1).tick, col(i).tick) else {
                continue;
            };
            let x = x_of(i);
            if p1.div_euclid(bar_len) != p0.div_euclid(bar_len) {
                p.vline(x, rect.y_range(), Stroke::new(1.0, pal.grid_bar));
                p.text(
                    pos2(x + 2.0, rect.bottom() - 2.0),
                    Align2::LEFT_BOTTOM,
                    format!("{}", p1.div_euclid(bar_len) + 1),
                    design::paint::ruler(),
                    pal.text_dim,
                );
            } else if p1.div_euclid(beat_len) != p0.div_euclid(beat_len) {
                p.vline(x, rect.y_range(), Stroke::new(1.0, pal.grid));
            }
        }
    }
    let fill_alpha = if pal.dark { 0.35 } else { 0.2 };
    let dim = pal.text_dim.gamma_multiply(fill_alpha);
    let red = pal.danger.gamma_multiply(fill_alpha + 0.1);
    let mut fill = egui::Mesh::default();
    let mut gr_mesh = egui::Mesh::default();
    let mut out_line = Vec::with_capacity(cols);
    let mut gr_line = Vec::with_capacity(cols);
    for i in 0..cols {
        let c = col(i);
        let x = x_of(i);
        let base = fill.vertices.len() as u32;
        fill.colored_vertex(pos2(x, y(c.inp)), dim);
        fill.colored_vertex(pos2(x, rect.bottom()), dim);
        let gy = y(-c.gr);
        gr_mesh.colored_vertex(pos2(x, rect.top()), red);
        gr_mesh.colored_vertex(pos2(x, gy), red);
        if i > 0 {
            fill.add_triangle(base - 2, base - 1, base);
            fill.add_triangle(base - 1, base, base + 1);
            gr_mesh.add_triangle(base - 2, base - 1, base);
            gr_mesh.add_triangle(base - 1, base, base + 1);
        }
        out_line.push(pos2(x, y(c.out)));
        gr_line.push(pos2(x, gy));
    }
    p.add(fill);
    p.add(gr_mesh);
    if out_line.len() > 1 {
        p.add(egui::Shape::line(out_line, Stroke::new(1.5, pal.text)));
        p.add(egui::Shape::line(gr_line, Stroke::new(1.5, pal.danger)));
    }
    // The threshold: a dashed line, brighter while held.
    let ty = y(s.threshold_db);
    let held = app.comp_editor.drag == Some(Grab::Threshold);
    let near = resp.hover_pos().is_some_and(|pt| (pt.y - ty).abs() < 6.0);
    p.add(egui::Shape::dashed_line(
        &[pos2(rect.left(), ty), pos2(rect.right(), ty)],
        Stroke::new(if held || near { 2.0 } else { 1.0 }, pal.gold),
        6.0,
        4.0,
    ));
    plate_text(
        &p,
        pos2(rect.right() - space::S2, ty - 3.0),
        Align2::RIGHT_BOTTOM,
        format!("{:+.1} dB", s.threshold_db),
        pal.gold,
    );
    let ed = &app.comp_editor;
    let last_gr = ed.history.back().map(|h| h.gr).unwrap_or(0.0);
    let max_gr = (a..b).map(|i| ed.history[i].gr).fold(0.0f32, f32::max);
    plate_text(
        &p,
        pos2(rect.right() - space::S2, rect.bottom() - space::S2),
        Align2::RIGHT_BOTTOM,
        format!(
            "{}   {}",
            tf("comp.gr_now", &[("db", &format!("{last_gr:.1}"))]),
            tf("comp.gr_max", &[("db", &format!("{max_gr:.1}"))])
        ),
        pal.danger,
    );
    // What the history is doing, when it is not simply running.
    let state = if ed.frozen {
        Some(t("comp.frozen"))
    } else if ed.scroll > 0 {
        Some(tf("comp.scrolled", &[("n", &ed.scroll.to_string())]))
    } else if ed.speed == Speed::Whole {
        Some(tf(
            "comp.whole_state",
            &[("n", &ed.whole_frames.to_string())],
        ))
    } else {
        None
    };
    if let Some(state) = state {
        p.text(
            pos2(rect.left() + space::S6, rect.top() + space::S2),
            Align2::LEFT_TOP,
            state,
            design::paint::caption(),
            pal.gold,
        );
    }
    if cols == 0 {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            t("comp.history_empty"),
            design::paint::empty_state(),
            pal.text_dim,
        );
    }
    // Hover: read any moment.
    if let Some(pt) = resp.hover_pos() {
        if !near && cols > 0 && app.comp_editor.drag.is_none() {
            let i = (((pt.x - x_of(0)) / dx).round().max(0.0) as usize).min(cols - 1);
            let c = col(i);
            p.vline(x_of(i), rect.y_range(), Stroke::new(1.0, pal.grid_bar));
            let mut text = tf(
                "comp.readout",
                &[
                    ("inp", &format!("{:.1}", c.inp.max(-99.9))),
                    ("out", &format!("{:.1}", c.out.max(-99.9))),
                    ("gr", &format!("{:.1}", c.gr)),
                ],
            );
            if let Some(tick) = c.tick {
                let (bar, beat, _) = app.project().time_signature.bbt(cantus_core::Ticks(tick));
                text.push_str(&format!("  \u{b7}  {bar}.{beat}"));
            }
            let at = pos2(
                (pt.x + space::S3).min(rect.right() - space::GRID * 60.0),
                rect.top() + space::S6,
            );
            let galley = p.layout_no_wrap(text, design::paint::ruler(), pal.text);
            let plate = Rect::from_min_size(at, galley.size()).expand(space::S1);
            p.rect_filled(plate, pal.corner_radius, pal.panel_raised);
            p.rect_stroke(
                plate,
                pal.corner_radius,
                Stroke::new(1.0, pal.grid_strong),
                StrokeKind::Inside,
            );
            p.galley(at, galley, pal.text);
        }
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    // The wheel scrolls back through what was kept (not in Whole, which shows it all).
    if resp.hovered() && app.comp_editor.speed != Speed::Whole {
        let wheel = ui.input(|i| i.smooth_scroll_delta.y + i.smooth_scroll_delta.x);
        if wheel != 0.0 {
            let ed = &mut app.comp_editor;
            let max = ed.history.len().saturating_sub(VISIBLE);
            let by = (wheel / dx).round() as i64;
            ed.scroll = (ed.scroll as i64 + by).clamp(0, max as i64) as usize;
        }
    }
    if resp.clicked() && !near {
        app.comp_editor.frozen = !app.comp_editor.frozen;
    }
    if resp.double_clicked() {
        app.comp_editor.frozen = false;
        app.comp_editor.scroll = 0;
    }
    // What a drag grabs is decided where the press began: by the time egui calls it a
    // drag, the pointer has already moved.
    let origin = ui.input(|i| i.pointer.press_origin());
    if resp.drag_started() && origin.is_some_and(|o| (o.y - ty).abs() < 6.0) {
        app.comp_editor.drag = Some(Grab::Threshold);
    }
    if app.comp_editor.drag == Some(Grab::Threshold) {
        if let Some(pt) = resp.interact_pointer_pos() {
            if resp.dragged() {
                app.set_comp(strip, effect, "thr", d_of(pt.y).clamp(-80.0, 0.0), true);
            }
        }
    }
    if resp.drag_stopped() {
        app.comp_editor.drag = None;
        app.end_gesture();
    }
    if near {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
}

/// The transfer curve: input level across, output level up, on the history's scale.
/// Drag the threshold point, the slope above it (the ratio, with click stops; Shift for
/// free), the knee marks, or the dashed range floor; the wheel widens the knee. A
/// feedback detector's curve is drawn as it really settles, with the set curve dashed
/// behind it.
fn transfer_display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &CompSettings,
    tel: &Tele,
    side: f32,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side), Sense::click_and_drag());
    app.comp_editor.geometry.transfer = Some(rect);
    let lo = -app.comp_editor.range_db;
    let x = |d: f32| rect.left() + ((d - lo) / -lo).clamp(0.0, 1.0) * rect.width();
    let y = |d: f32| rect.bottom() - ((d - lo) / -lo).clamp(0.0, 1.0) * rect.height();
    let dx_of = |px: f32| lo + (px - rect.left()) / rect.width() * -lo;
    let dy_of = |py: f32| lo + (rect.bottom() - py) / rect.height() * -lo;
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let step = if -lo <= 36.0 { 6.0 } else { 12.0 };
    let mut g = -step;
    while g > lo {
        p.hline(rect.x_range(), y(g), Stroke::new(1.0, pal.grid));
        p.vline(x(g), rect.y_range(), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(x(g) + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            format!("{g:.0}"),
            design::paint::ruler(),
            pal.text_dim,
        );
        p.text(
            pos2(rect.left() + 2.0, y(g) - 1.0),
            Align2::LEFT_BOTTOM,
            format!("{g:.0}"),
            design::paint::ruler(),
            pal.text_dim,
        );
        g -= step;
    }
    // 1:1, for reference.
    p.line_segment(
        [pos2(x(lo), y(lo)), pos2(x(0.0), y(0.0))],
        Stroke::new(1.0, pal.grid_bar),
    );
    let mix = s.mix;
    let out_of = |d: f32| d - mix * effective_gain_reduction_db(d, s);
    let fb = s.detector().feedback;
    let n = 120;
    let at = |i: usize| lo + -lo * i as f32 / n as f32;
    if fb > 0.0 || mix != 1.0 {
        // The curve as set, behind the one that really happens.
        let set: Vec<Pos2> = (0..=n)
            .map(|i| {
                pos2(
                    x(at(i)),
                    y(at(i) - cantus_dsp::comprimo::gain_reduction_db(at(i), s)),
                )
            })
            .collect();
        p.add(egui::Shape::dashed_line(
            &set,
            Stroke::new(1.0, pal.gold),
            4.0,
            3.0,
        ));
    }
    // The range floor: the curve never falls further below 1:1 than this.
    let floor_shown = s.range_db < -lo;
    if floor_shown {
        let r = s.range_db * mix.max(0.01);
        let held = app.comp_editor.drag == Some(Grab::Range);
        p.add(egui::Shape::dashed_line(
            &[pos2(x(lo + r), y(lo)), pos2(x(0.0), y(-r))],
            Stroke::new(if held { 2.0 } else { 1.0 }, pal.danger),
            3.0,
            3.0,
        ));
    }
    let curve: Vec<Pos2> = (0..=n).map(|i| pos2(x(at(i)), y(out_of(at(i))))).collect();
    p.add(egui::Shape::line(curve, Stroke::new(2.5, pal.gold)));
    // The knee marks and the threshold point.
    let t_pt = pos2(x(s.threshold_db), y(out_of(s.threshold_db)));
    let k_lo = s.threshold_db - s.knee_db * 0.5;
    let k_hi = s.threshold_db + s.knee_db * 0.5;
    let k_pts = [
        pos2(x(k_lo), y(out_of(k_lo))),
        pos2(x(k_hi), y(out_of(k_hi))),
    ];
    for k in k_pts {
        p.circle_stroke(k, 4.0, Stroke::new(1.5, pal.gold));
    }
    p.circle_filled(t_pt, 6.0, pal.gold);
    // Where the key is now, on the input axis, and the operating point.
    if tel.live() {
        let key = tel.get(tele::KEY_LEVEL);
        if key > lo {
            let kx = x(key);
            p.line_segment(
                [pos2(kx, rect.bottom()), pos2(kx, rect.bottom() - space::S3)],
                Stroke::new(3.0, pal.selection),
            );
        }
    }
    let (ni, no) = app.comp_editor.now;
    if ni > lo {
        p.circle_filled(pos2(x(ni), y(no)), 4.0, pal.danger);
    }
    let mut lines = vec![format!(
        "{}  {}  \u{b7}  {} {:.1} dB",
        t("fx.comp.ratio"),
        ratio_text(s.ratio),
        t("fx.comp.knee"),
        s.knee_db
    )];
    if fb > 0.0 && s.ratio > 1.0 {
        let over = 12.0;
        let g = effective_gain_reduction_db(s.threshold_db + over, s);
        let eff = if over - g > 0.01 {
            over / (over - g)
        } else {
            RATIO_INF
        };
        lines.push(tf("comp.effective", &[("ratio", &ratio_text(eff))]));
    }
    for (i, line) in lines.into_iter().enumerate() {
        plate_text(
            &p,
            pos2(
                rect.left() + space::S2,
                rect.top() + space::S2 + i as f32 * 14.0,
            ),
            Align2::LEFT_TOP,
            line,
            pal.text,
        );
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    let shift = ui.input(|i| i.modifiers.shift);
    if resp.drag_started() {
        if let Some(pt) = ui.input(|i| i.pointer.press_origin()) {
            let near = |q: Pos2| pt.distance(q) < 9.0;
            let over_floor = floor_shown && {
                let (di, dout) = (dx_of(pt.x), dy_of(pt.y));
                ((di - dout) - s.range_db * mix).abs() < -lo * 9.0 / rect.width()
            };
            app.comp_editor.drag = if near(t_pt) {
                Some(Grab::Threshold)
            } else if near(k_pts[0]) || near(k_pts[1]) {
                Some(Grab::Knee)
            } else if over_floor {
                Some(Grab::Range)
            } else if dx_of(pt.x) > s.threshold_db {
                Some(Grab::Ratio)
            } else {
                None
            };
        }
    }
    if let (Some(g), Some(pt)) = (app.comp_editor.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            match g {
                Grab::Threshold => {
                    app.set_comp(strip, effect, "thr", dx_of(pt.x).clamp(-80.0, 0.0), true)
                }
                Grab::Knee => {
                    let k = ((dx_of(pt.x) - s.threshold_db).abs() * 2.0).clamp(0.0, 72.0);
                    app.set_comp(strip, effect, "knee", k, true);
                }
                Grab::Range => {
                    let r = ((dx_of(pt.x) - dy_of(pt.y)) / mix.max(0.01)).clamp(0.0, 60.0);
                    app.set_comp(strip, effect, "range", r, true);
                }
                Grab::Ratio => {
                    // The pointer is a point the curve should pass through: its input
                    // level over the threshold, and its output over the threshold.
                    let over_in = (dx_of(pt.x) - s.threshold_db).max(0.5);
                    let over_out = (dy_of(pt.y) - s.threshold_db).clamp(0.0, over_in);
                    let ratio = if over_out < 0.01 {
                        RATIO_INF
                    } else {
                        (over_in / over_out).clamp(1.0, RATIO_INF)
                    };
                    let ratio = if shift { ratio } else { snap_ratio(ratio) };
                    app.set_comp(strip, effect, "ratio", ratio, true);
                }
            }
        }
    }
    if resp.drag_stopped() {
        app.comp_editor.drag = None;
        app.end_gesture();
    }
    // The wheel over the curve: the knee, one step of undo per burst.
    if resp.hovered() {
        let wheel = ui.input(|i| i.smooth_scroll_delta.y);
        if wheel != 0.0 {
            let fine = if shift { 0.2 } else { 1.0 };
            let k = (s.knee_db + wheel * 0.1 * fine).clamp(0.0, 72.0);
            app.set_comp(strip, effect, "knee", k, true);
            app.comp_editor.wheel_at = Some(app.comp_editor.clock);
        }
    }
    if resp.double_clicked() {
        app.set_comp_many(
            strip,
            effect,
            &[("ratio".into(), 4.0), ("knee".into(), 6.0)],
            "cmd.comp_reset_curve",
            None,
        );
    }
    resp.on_hover_text(t("comp.transfer_tip"));
}

/// Input, reduction and output meters with their held peaks, the output's loudness drawn
/// on its meter, and the numbers: loudness momentary / short-term / integrated, ten
/// seconds of reduction, the crest factor in and out, the auto gain in force.
fn meters_panel(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    s: &CompSettings,
    tel: &Tele,
    size: egui::Vec2,
) {
    let pal = theme::p();
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter_at(rect.expand(1.0));
    let range = app.comp_editor.range_db;
    let ed = &app.comp_editor;
    let last = ed.live;
    let bar_w = space::GRID * 3.0;
    let col_w = space::GRID * 9.0;
    let top = rect.top() + space::S6;
    let bottom = rect.bottom() - space::S4;
    let y = |d: f32| top + (-d / range).clamp(0.0, 1.0) * (bottom - top);
    let meters = [
        ("comp.meter_in", last.inp, ed.hold[0], false),
        ("comp.meter_gr", last.gr, ed.hold[1], true),
        ("comp.meter_out", last.out, ed.hold[2], false),
    ];
    for (i, (label, v, hold, reduction)) in meters.into_iter().enumerate() {
        let mid = rect.left() + (i as f32 + 0.5) * col_w;
        let well = Rect::from_min_max(
            pos2(mid - bar_w * 0.5, top),
            pos2(mid + bar_w * 0.5, bottom),
        );
        p.rect_filled(well, 2.0, theme::recess(&pal));
        let (bar, color) = if reduction {
            (
                Rect::from_min_max(well.min, pos2(well.right(), y(-v))),
                pal.danger,
            )
        } else {
            (
                Rect::from_min_max(pos2(well.left(), y(v)), well.max),
                theme::meter_color(10f32.powf(v / 20.0)),
            )
        };
        p.rect_filled(bar.shrink(1.0), 1.0, color);
        let hy = if reduction { y(-hold) } else { y(hold) };
        p.hline(well.x_range(), hy, Stroke::new(1.5, color));
        p.rect_stroke(
            well,
            2.0,
            Stroke::new(1.0, pal.grid_strong),
            StrokeKind::Inside,
        );
        p.text(
            pos2(well.center().x, rect.top()),
            Align2::CENTER_TOP,
            t(label),
            design::paint::caption(),
            pal.text_dim,
        );
        let held = if reduction {
            format!("{:.1}", -hold.max(0.0))
        } else if hold <= -99.0 {
            "-\u{221e}".to_string()
        } else {
            format!("{hold:.1}")
        };
        p.text(
            pos2(well.center().x, bottom + 2.0),
            Align2::CENTER_TOP,
            held,
            design::paint::ruler(),
            if !reduction && hold >= 0.0 {
                pal.danger
            } else {
                pal.text
            },
        );
        // The output's short-term loudness, on its own meter: a tick in the ok colour.
        if i == 2 && tel.live() {
            let lu = tel.get(tele::OUT_S);
            if lu > -range {
                p.line_segment(
                    [
                        pos2(well.left() - 3.0, y(lu)),
                        pos2(well.right() + 3.0, y(lu)),
                    ],
                    Stroke::new(2.0, pal.ok),
                );
            }
        }
    }
    // The numbers, in a column to the right of the meters.
    let nx = rect.left() + 3.0 * col_w + space::S2;
    let right = rect.right();
    let mut row = rect.top();
    let line_h = 14.0;
    let heading = |p: &egui::Painter, key: &str, row: &mut f32| {
        *row += 4.0;
        p.text(
            pos2(nx, *row),
            Align2::LEFT_TOP,
            t(key),
            design::paint::caption(),
            pal.text_dim,
        );
        *row += line_h;
    };
    let pair = |p: &egui::Painter, label: String, value: String, color: Color32, row: &mut f32| {
        p.text(
            pos2(nx, *row),
            Align2::LEFT_TOP,
            label,
            design::paint::ruler(),
            pal.text_dim,
        );
        p.text(
            pos2(right, *row),
            Align2::RIGHT_TOP,
            value,
            design::paint::ruler(),
            color,
        );
        *row += line_h;
    };
    let live = tel.live();
    let v = |i: usize| if live { tel.get(i) } else { loudness::SILENCE };
    heading(&p, "comp.loudness", &mut row);
    pair(
        &p,
        t("comp.lufs_m"),
        lufs_text(v(tele::OUT_M)),
        pal.text,
        &mut row,
    );
    pair(
        &p,
        t("comp.lufs_s"),
        lufs_text(v(tele::OUT_S)),
        pal.ok,
        &mut row,
    );
    let integrated = ed.integrated();
    pair(
        &p,
        t("comp.lufs_i"),
        lufs_text(integrated),
        pal.text,
        &mut row,
    );
    let reset_at = row;
    row += line_h + 2.0;
    heading(&p, "comp.gr_stats", &mut row);
    let (avg, peak) = if live {
        (tel.get(tele::GR_AVG), tel.get(tele::GR_PEAK))
    } else {
        (0.0, 0.0)
    };
    pair(
        &p,
        t("comp.gr_avg"),
        format!("{avg:.1}"),
        pal.danger,
        &mut row,
    );
    pair(
        &p,
        t("comp.gr_peak"),
        format!("{peak:.1}"),
        pal.danger,
        &mut row,
    );
    heading(&p, "comp.crest", &mut row);
    let (ci, co) = if live {
        (tel.get(tele::CREST_IN), tel.get(tele::CREST_OUT))
    } else {
        (0.0, 0.0)
    };
    pair(
        &p,
        t("comp.crest_io"),
        format!("{ci:.1}\u{2192}{co:.1}"),
        pal.text,
        &mut row,
    );
    if s.auto_gain {
        heading(&p, "fx.comp.auto_gain", &mut row);
        let g = if live { tel.get(tele::AUTO_GAIN) } else { 0.0 };
        pair(&p, String::new(), format!("{g:+.1} dB"), pal.gold, &mut row);
    }
    let reset = Rect::from_min_size(pos2(nx, reset_at), vec2(right - nx, line_h + 2.0));
    if ui
        .put(
            reset,
            egui::Button::new(RichText::new(t("comp.lufs_reset")).text_style(design::caption()))
                .small(),
        )
        .on_hover_text(t("comp.lufs_reset_tip"))
        .clicked()
    {
        app.comp_editor.reset_loudness();
    }
}

/// Knob size in the editor: 40 px, the grid's M-to-L step.
const KNOB_D: f32 = space::GRID * 10.0;

/// A knob bound to one of Comprimo's parameters, with its readout in the parameter's own
/// words (∞:1, Off, %), the ratio's click stops, and one undo step per gesture.
fn comp_knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    spec: &EffectSpec,
    key: &str,
) -> egui::Response {
    let Some(info) = EffectKind::Compressor.param(key) else {
        return ui.label(key);
    };
    let mut v = spec.get(key);
    let knob = Knob::new(&mut v, info.min..=info.max, info.default)
        .diameter(KNOB_D)
        .log(info.logarithmic)
        .stepped(info.stepped)
        .unit(info.unit)
        .label(t(info.label_key));
    let knob = match key {
        "ratio" => knob.format(ratio_text),
        "key_hpf" => knob.format(|x| {
            if x <= 20.5 {
                t("comp.off")
            } else {
                crate::widgets::knob::readout(x, "Hz")
            }
        }),
        "dry" => knob.format(|x| {
            if x <= cantus_dsp::comprimo::DRY_OFF_DB + 0.05 {
                t("comp.off")
            } else {
                crate::widgets::knob::readout(x, "dB")
            }
        }),
        "look" => knob.format(|x| {
            if x <= 0.005 {
                t("comp.off")
            } else {
                crate::widgets::knob::readout(x, "ms")
            }
        }),
        _ => knob,
    };
    let r = ui.add(knob).on_hover_text(t(&format!("comp.tip.{key}")));
    if r.changed() {
        let shift = ui.input(|i| i.modifiers.shift);
        let v = if key == "ratio" && !shift {
            snap_ratio(v)
        } else {
            v
        };
        app.set_comp(strip, effect, key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    if key == "ratio" {
        // The click stops, as ticks around the knob: where the ratio will land.
        let pal = theme::p();
        let c = pos2(r.rect.center().x, r.rect.top() + KNOB_D * 0.5 + 1.0);
        let radius = KNOB_D * 0.5;
        for stop in RATIO_STOPS {
            let pos = stop.ln() / RATIO_INF.ln();
            let a = 0.75 * std::f32::consts::PI + 1.5 * std::f32::consts::PI * pos;
            let (sn, cs) = a.sin_cos();
            ui.painter().line_segment(
                [
                    pos2(c.x + (radius + 1.0) * cs, c.y + (radius + 1.0) * sn),
                    pos2(c.x + (radius + 4.0) * cs, c.y + (radius + 4.0) * sn),
                ],
                Stroke::new(1.0, pal.grid_bar),
            );
        }
    }
    r
}

/// The word AUTO under a knob, lit when on — the benchmark's cue, kept.
fn auto_chip(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    key: &str,
    on: bool,
) {
    let text = RichText::new(t("comp.auto"))
        .text_style(design::caption())
        .strong();
    if ui
        .selectable_label(on, text)
        .on_hover_text(t(&format!("comp.tip.{key}")))
        .clicked()
    {
        app.set_comp(strip, effect, key, if on { 0.0 } else { 1.0 }, false);
    }
}

/// Note values a release snaps to, in quarter notes: the name and the length.
const NOTES: [(&str, f32); 9] = [
    ("1/64", 1.0 / 16.0),
    ("1/32", 1.0 / 8.0),
    ("1/16", 0.25),
    ("1/16.", 0.375),
    ("1/8", 0.5),
    ("1/8.", 0.75),
    ("1/4", 1.0),
    ("1/2", 2.0),
    ("1/1", 4.0),
];

/// The release as a note value at the project's tempo: the nearest one is named under the
/// knob, and the menu snaps to any — a snap, not a mode, so the release stays in ms.
fn note_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    rel_ms: f32,
) {
    let quarter_ms = 60_000.0 / (app.project().tempo_bpm as f32).max(1.0);
    let near = NOTES
        .iter()
        .find(|(_, q)| ((q * quarter_ms) / rel_ms - 1.0).abs() < 0.03)
        .map(|(n, _)| n.to_string());
    let label =
        RichText::new(near.clone().unwrap_or_else(|| t("comp.note"))).text_style(design::caption());
    ui.menu_button(label, |ui| {
        caption(ui, "comp.note_tip");
        for (name, q) in NOTES {
            let ms = q * quarter_ms;
            if !(5.0..=5_000.0).contains(&ms) {
                continue;
            }
            let text = format!("{name}   {}", crate::widgets::knob::readout(ms, "ms"));
            if ui
                .selectable_label(near.as_deref() == Some(name), text)
                .clicked()
            {
                app.set_comp(strip, effect, "rel", ms, false);
                ui.close();
            }
        }
    })
    .response
    .on_hover_text(t("comp.note_tip"));
}

/// A captioned group of controls.
fn group(ui: &mut egui::Ui, key: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.vertical(|ui| {
        caption(ui, key);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = space::S1;
            add(ui);
        });
    });
}

/// The knobs, in three groups: dynamics, time, output.
fn controls(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    spec: &EffectSpec,
    s: &CompSettings,
) {
    ui.allocate_ui(vec2(ui.available_width(), CONTROLS_H), |ui| {
        ui.horizontal_top(|ui| {
            group(ui, "comp.group.dynamics", |ui| {
                for key in ["thr", "ratio", "knee", "range"] {
                    comp_knob(app, ui, strip, effect, spec, key);
                }
            });
            ui.separator();
            group(ui, "comp.group.time", |ui| {
                comp_knob(app, ui, strip, effect, spec, "att");
                ui.vertical(|ui| {
                    comp_knob(app, ui, strip, effect, spec, "rel");
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = space::S1;
                        auto_chip(app, ui, strip, effect, "auto_rel", s.auto_release);
                        note_menu(app, ui, strip, effect, s.release_ms);
                    });
                });
                for key in ["hold", "look"] {
                    comp_knob(app, ui, strip, effect, spec, key);
                }
            });
            ui.separator();
            group(ui, "comp.group.output", |ui| {
                ui.vertical(|ui| {
                    comp_knob(app, ui, strip, effect, spec, "makeup");
                    auto_chip(app, ui, strip, effect, "auto_gain", s.auto_gain);
                });
                for key in ["dry", "mix", "in_gain", "out_gain"] {
                    comp_knob(app, ui, strip, effect, spec, key);
                }
            });
        });
    });
}

/// The key: where it comes from, its level, its high-pass, the stereo link, and the EQ's
/// switch.
fn key_section(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &CompSettings,
) {
    ui.allocate_ui(vec2(space::GRID * 62.0, KEY_H), |ui| {
        ui.vertical(|ui| {
            ui.set_width(space::GRID * 62.0);
            caption(ui, "comp.key");
            let current = slot.sidechain;
            let label = match current.and_then(|k| app.project().strip_number(k)) {
                Some(n) => app.capture_label(cantus_engine::CaptureTarget::Strip(
                    app.project().strips[n - 1].id,
                )),
                None => t("comp.key_own"),
            };
            egui::ComboBox::from_id_salt(("comp-key", effect.0))
                .selected_text(label)
                .width(space::GRID * 58.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(current.is_none(), t("comp.key_own"))
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
                .on_hover_text(t("comp.key_tip"));
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = space::S1;
                for key in ["key_gain", "key_hpf", "link"] {
                    comp_knob(app, ui, strip, effect, &slot.spec, key);
                }
            });
            let mut eq_in = s.key_eq;
            if ui
                .toggle_value(&mut eq_in, t("comp.key_eq"))
                .on_hover_text(t("comp.key_eq_tip"))
                .changed()
            {
                app.set_comp(
                    strip,
                    effect,
                    "key_eq",
                    if eq_in { 1.0 } else { 0.0 },
                    false,
                );
            }
        });
    });
}

/// The key EQ: its display, and under it the selected band's numbers as fields you can
/// type into — so every band is reachable from the keyboard, and its state is on screen.
#[allow(clippy::too_many_arguments)]
fn key_eq_panel(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &CompSettings,
    size: egui::Vec2,
) {
    ui.allocate_ui(size, |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = space::S1;
            let display = vec2(size.x, size.y - space::ROW - space::S1);
            key_eq_display(app, ui, strip, effect, slot, s, display);
            key_band_row(app, ui, strip, effect, slot);
        });
    });
}

/// The selected key band: shape, frequency, gain, Q or slope, on/off, remove.
fn key_band_row(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let bands = key_bands(&slot.spec);
    ui.horizontal(|ui| {
        ui.set_height(space::ROW);
        let Some(b) = app.comp_editor.key_band.filter(|b| bands[*b].used) else {
            caption(ui, "comp.key_band_none");
            return;
        };
        let v = bands[b];
        let key = |f: &str| key_band_key(b, f);
        let pal = theme::p();
        ui.label(
            RichText::new(tf("comp.key_band", &[("n", &(b + 1).to_string())]))
                .text_style(design::strong())
                .color(crate::views::eq_editor::band_color(b)),
        );
        let mut shape = v.shape.index();
        egui::ComboBox::from_id_salt(("comp-key-shape", effect.0, b))
            .selected_text(t(&format!("eq.shape.{shape}")))
            .width(space::GRID * 22.0)
            .show_ui(ui, |ui| {
                for sh in EqShape::ALL {
                    ui.selectable_value(
                        &mut shape,
                        sh.index(),
                        t(&format!("eq.shape.{}", sh.index())),
                    );
                }
            });
        if shape != v.shape.index() {
            app.set_comp(strip, effect, &key("shape"), shape as f32, false);
        }
        let mut field = |ui: &mut egui::Ui,
                         f: &str,
                         value: f32,
                         range: std::ops::RangeInclusive<f32>,
                         suffix: &str,
                         speed: f64,
                         log: bool| {
            let mut x = value;
            let r = ui.add(
                egui::DragValue::new(&mut x)
                    .range(range)
                    .speed(speed)
                    .suffix(suffix)
                    .max_decimals(if log { 0 } else { 2 }),
            );
            if r.changed() {
                app.set_comp_many(
                    strip,
                    effect,
                    &[(key(f), x)],
                    "cmd.comp_key_band",
                    Some(MergeKey::of(&(
                        "comp-key-field",
                        effect.0,
                        b,
                        f.to_string(),
                    ))),
                );
            }
            if r.drag_stopped() || r.lost_focus() {
                app.end_gesture();
            }
        };
        field(
            ui,
            "freq",
            v.freq,
            20.0..=20_000.0,
            " Hz",
            (v.freq as f64 * 0.01).max(0.1),
            true,
        );
        if v.shape.has_gain() {
            field(ui, "gain", v.gain, -30.0..=30.0, " dB", 0.1, false);
        }
        if v.shape.has_slope() {
            field(ui, "slope", v.slope, 6.0..=96.0, " dB/oct", 1.0, true);
        } else {
            field(ui, "q", v.q, 0.1..=18.0, " Q", 0.01, false);
        }
        let mut on = v.on;
        if ui
            .toggle_value(&mut on, t("comp.band_on"))
            .on_hover_text(t("eq.bypass_band"))
            .changed()
        {
            app.set_comp(strip, effect, &key("on"), if on { 1.0 } else { 0.0 }, false);
        }
        if ui
            .button(RichText::new(t("eq.remove_band")).color(pal.danger))
            .clicked()
        {
            app.remove_key_band(strip, effect, b);
        }
    });
}

/// One key-EQ band as the editor reads it from the spec.
#[derive(Copy, Clone, Debug)]
struct KeyBandView {
    used: bool,
    on: bool,
    shape: EqShape,
    freq: f32,
    gain: f32,
    q: f32,
    slope: f32,
}

fn key_bands(spec: &EffectSpec) -> [KeyBandView; KEY_BANDS] {
    std::array::from_fn(|b| {
        let g = |f: &str| spec.get(&key_band_key(b, f));
        KeyBandView {
            used: g("used") >= 0.5,
            on: g("on") >= 0.5,
            shape: EqShape::from_index(g("shape").round().max(0.0) as usize),
            freq: g("freq"),
            gain: g("gain"),
            q: g("q"),
            slope: (g("slope") / 6.0).round().clamp(1.0, 16.0) * 6.0,
        }
    })
}

/// The key EQ's curve at `hz`: every band in use and on, as the detector hears it.
fn key_curve_db(bands: &[KeyBandView], hz: f32) -> f32 {
    let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
    bands
        .iter()
        .filter(|b| b.used && b.on)
        .map(|b| {
            let n = design_band(b.shape, DISPLAY_SR, b.freq, b.gain, b.q, b.slope, &mut c);
            c[..n]
                .iter()
                .map(|x| x.magnitude_db(DISPLAY_SR, hz))
                .sum::<f32>()
        })
        .sum()
}

/// `① Low cut · 120 Hz · 24 dB/oct`.
fn key_band_words(b: usize, v: &KeyBandView) -> String {
    let mut parts = vec![
        tf("comp.key_band", &[("n", &(b + 1).to_string())]),
        t(&format!("eq.shape.{}", v.shape.index())),
        crate::widgets::knob::readout(v.freq, "Hz"),
    ];
    if v.shape.has_gain() {
        parts.push(format!("{:+.1} dB", v.gain));
    }
    if v.shape.has_slope() {
        parts.push(format!("{:.0} dB/oct", v.slope));
    } else {
        parts.push(format!("Q {:.2}", v.q));
    }
    if !v.on {
        parts.push(t("comp.bypassed"));
    }
    parts.join(" \u{b7} ")
}

/// The key's EQ: Aequalis's shapes and design on the detector, drawn over the key's
/// spectrum. Double-click to add a band (a cut at the edges, a bell elsewhere), drag a dot,
/// the wheel for Q (slope on a cut), Alt+click to bypass, right-click for its menu,
/// Delete to remove it.
fn key_eq_display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &CompSettings,
    size: egui::Vec2,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    app.comp_editor.geometry.key_eq = Some(rect);
    let span = (KEY_HI_HZ / KEY_LO_HZ).ln();
    let x_of = |hz: f32| rect.left() + (hz / KEY_LO_HZ).ln() / span * rect.width();
    let hz_of =
        |px: f32| KEY_LO_HZ * (span * ((px - rect.left()) / rect.width()).clamp(0.0, 1.0)).exp();
    let y_of = |d: f32| rect.center().y - (d / KEY_DB).clamp(-1.0, 1.0) * rect.height() * 0.45;
    let db_of = |py: f32| (rect.center().y - py) / (rect.height() * 0.45) * KEY_DB;
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    for hz in [
        50.0f32, 100.0, 200.0, 500.0, 1_000.0, 2_000.0, 5_000.0, 10_000.0,
    ] {
        p.vline(x_of(hz), rect.y_range(), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(x_of(hz) + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            crate::widgets::knob::readout(hz, "Hz")
                .replace(" Hz", "")
                .replace(".0", ""),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    for d in [-12.0f32, 12.0] {
        p.hline(rect.x_range(), y_of(d), Stroke::new(1.0, pal.grid));
    }
    p.hline(rect.x_range(), y_of(0.0), Stroke::new(1.0, pal.grid_bar));
    let bands = key_bands(&slot.spec);
    // The key's spectrum: its own input when the key is this strip; the key itself (the
    // output) while you listen to it. A key from another strip is not tapped otherwise.
    let sr = DISPLAY_SR;
    let an = &app.comp_editor.analyser;
    let internal = slot.sidechain.is_none();
    let source = if s.listen == Listen::Key {
        Some(1)
    } else if internal {
        Some(0)
    } else {
        None
    };
    let spec_y = |d: f32| rect.bottom() - ((d + 90.0) / 90.0).clamp(0.0, 1.0) * rect.height();
    if let (Some(which), true) = (source, an.has_data()) {
        let pts: Vec<Pos2> = (0..KEY_POINTS)
            .map(|i| {
                let hz = KEY_LO_HZ * (span * i as f32 / (KEY_POINTS - 1) as f32).exp();
                let mut d = an.level_db(which, sr, hz, 1.0 / 6.0);
                // Tilted 4.5 dB/octave about 1 kHz, as the EQ's analyser: music reads
                // level.
                d += 4.5 * (hz / 1_000.0).log2();
                pos2(x_of(hz), spec_y(d))
            })
            .collect();
        let fill = pal
            .text_dim
            .gamma_multiply(if pal.dark { 0.3 } else { 0.15 });
        let mut mesh = egui::Mesh::default();
        for (i, pt) in pts.iter().enumerate() {
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(*pt, fill);
            mesh.colored_vertex(pos2(pt.x, rect.bottom()), fill);
            if i > 0 {
                mesh.add_triangle(base - 2, base - 1, base);
                mesh.add_triangle(base - 1, base, base + 1);
            }
        }
        p.add(mesh);
        if which == 0 && s.key_eq {
            // What the detector hears: the input through the EQ.
            let heard: Vec<Pos2> = pts
                .iter()
                .map(|pt| {
                    let hz = hz_of(pt.x);
                    let d = (rect.bottom() - pt.y) / rect.height() * 90.0 - 90.0;
                    pos2(pt.x, spec_y(d + key_curve_db(&bands, hz)))
                })
                .collect();
            p.add(egui::Shape::line(heard, Stroke::new(1.0, pal.text_dim)));
        }
    }
    // The curve.
    let curve: Vec<Pos2> = (0..KEY_POINTS)
        .map(|i| {
            let hz = KEY_LO_HZ * (span * i as f32 / (KEY_POINTS - 1) as f32).exp();
            pos2(x_of(hz), y_of(key_curve_db(&bands, hz)))
        })
        .collect();
    if s.key_eq {
        p.add(egui::Shape::line(curve, Stroke::new(2.0, pal.gold)));
    } else {
        p.add(egui::Shape::dashed_line(
            &curve,
            Stroke::new(1.0, pal.gold),
            5.0,
            4.0,
        ));
    }
    // The dots, numbered: a band keeps its number for life.
    let dot = |v: &KeyBandView| {
        pos2(
            x_of(v.freq.clamp(KEY_LO_HZ, KEY_HI_HZ)),
            y_of(if v.shape.has_gain() { v.gain } else { 0.0 }),
        )
    };
    let hit = |pt: Pos2| {
        (0..KEY_BANDS)
            .filter(|b| bands[*b].used)
            .map(|b| (b, dot(&bands[b]).distance(pt)))
            .filter(|(_, d)| *d < 10.0)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(b, _)| b)
    };
    let hovered = resp.hover_pos().and_then(hit);
    let pressed_on = ui.input(|i| i.pointer.press_origin()).and_then(hit);
    for (b, v) in bands.iter().enumerate() {
        if !v.used {
            continue;
        }
        let c = crate::views::eq_editor::band_color(b);
        let at = dot(v);
        let selected = app.comp_editor.key_band == Some(b);
        let r = if selected || hovered == Some(b) {
            8.0
        } else {
            7.0
        };
        if v.on {
            p.circle_filled(at, r, c);
        } else {
            p.circle_stroke(at, r, Stroke::new(1.5, c));
        }
        if selected {
            p.circle_stroke(at, r + 2.0, Stroke::new(1.5, pal.text));
        }
        p.text(
            at,
            Align2::CENTER_CENTER,
            format!("{}", b + 1),
            design::paint::ruler(),
            if v.on { theme::on_color(&pal, c) } else { c },
        );
    }
    // Words: the state, and the selected band's numbers.
    let head = if s.key_eq {
        t("comp.key_eq_title")
    } else {
        format!("{} \u{b7} {}", t("comp.key_eq_title"), t("comp.bypassed"))
    };
    p.text(
        rect.left_top() + vec2(space::S2, space::S1),
        Align2::LEFT_TOP,
        head,
        design::paint::caption(),
        if s.key_eq { pal.text } else { pal.text_dim },
    );
    if !bands.iter().any(|b| b.used) {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            t("comp.key_eq_empty"),
            design::paint::caption(),
            pal.text_dim,
        );
    }
    if source.is_none() {
        p.text(
            rect.right_top() + vec2(-space::S2, space::S1),
            Align2::RIGHT_TOP,
            t("comp.key_spectrum_hint"),
            design::paint::caption(),
            pal.text_dim,
        );
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
    key_eq_gestures(
        app,
        ui,
        &resp,
        strip,
        effect,
        &bands,
        (hovered, pressed_on),
        (hz_of, db_of),
    );
}

/// The key EQ's gestures: select, Alt+click to bypass, double-click to add, drag, wheel,
/// Delete, and the band's menu.
#[allow(clippy::too_many_arguments)]
fn key_eq_gestures<H, D>(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    resp: &egui::Response,
    strip: StripId,
    effect: EffectId,
    bands: &[KeyBandView; KEY_BANDS],
    (hovered, pressed_on): (Option<usize>, Option<usize>),
    (hz_of, db_of): (H, D),
) where
    H: Fn(f32) -> f32,
    D: Fn(f32) -> f32,
{
    let key = |b: usize, f: &str| key_band_key(b, f);
    let (alt, shift) = ui.input(|i| (i.modifiers.alt, i.modifiers.shift));
    if resp.clicked() {
        match hovered {
            Some(b) if alt => {
                let on = if bands[b].on { 0.0 } else { 1.0 };
                app.set_comp(strip, effect, &key(b, "on"), on, false);
            }
            Some(b) => app.comp_editor.key_band = Some(b),
            None => app.comp_editor.key_band = None,
        }
    }
    if resp.secondary_clicked() {
        if let Some(b) = hovered {
            app.comp_editor.key_band = Some(b);
        }
    }
    if resp.double_clicked() && hovered.is_none() {
        let free = (0..KEY_BANDS).find(|b| !bands[*b].used);
        if let (Some(b), Some(pt)) = (free, resp.interact_pointer_pos()) {
            let frac = (pt.x - resp.rect.left()) / resp.rect.width();
            let shape = if frac < 0.12 {
                EqShape::LowCut
            } else if frac > 0.88 {
                EqShape::HighCut
            } else {
                EqShape::Bell
            };
            let mut values = vec![
                (key(b, "used"), 1.0),
                (key(b, "on"), 1.0),
                (key(b, "shape"), shape.index() as f32),
                (key(b, "freq"), hz_of(pt.x).clamp(KEY_LO_HZ, KEY_HI_HZ)),
            ];
            if shape.has_gain() {
                values.push((key(b, "gain"), db_of(pt.y).clamp(-30.0, 30.0)));
            } else {
                values.push((key(b, "slope"), 24.0));
            }
            app.set_comp_many(strip, effect, &values, "cmd.comp_key_band", None);
            app.comp_editor.key_band = Some(b);
        }
    }
    if resp.drag_started() {
        app.comp_editor.key_drag = pressed_on;
        if pressed_on.is_some() {
            app.comp_editor.key_band = pressed_on;
        }
    }
    if let (Some(b), Some(pt)) = (app.comp_editor.key_drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let mut values = vec![(key(b, "freq"), hz_of(pt.x).clamp(KEY_LO_HZ, KEY_HI_HZ))];
            if bands[b].shape.has_gain() {
                values.push((key(b, "gain"), db_of(pt.y).clamp(-30.0, 30.0)));
            }
            app.set_comp_many(
                strip,
                effect,
                &values,
                "cmd.comp_key_band",
                Some(MergeKey::of(&("comp-key", effect.0, b))),
            );
        }
    }
    if resp.drag_stopped() {
        app.comp_editor.key_drag = None;
        app.end_gesture();
    }
    // The wheel: Q, or the slope of a cut in 6 dB steps.
    if let Some(b) = hovered
        .or(app.comp_editor.key_band)
        .filter(|b| bands[*b].used)
    {
        let wheel = if resp.hovered() {
            ui.input(|i| i.smooth_scroll_delta.y)
        } else {
            0.0
        };
        if wheel != 0.0 {
            let v = bands[b];
            let merge = Some(MergeKey::of(&("comp-key-wheel", effect.0, b)));
            if v.shape.has_slope() {
                let slope = (v.slope + 6.0 * wheel.signum()).clamp(6.0, 96.0);
                app.set_comp_many(
                    strip,
                    effect,
                    &[(key(b, "slope"), slope)],
                    "cmd.comp_key_band",
                    merge,
                );
            } else {
                let fine = if shift { 0.2 } else { 1.0 };
                let q = (v.q * (wheel * 0.004 * fine).exp()).clamp(0.1, 18.0);
                app.set_comp_many(
                    strip,
                    effect,
                    &[(key(b, "q"), q)],
                    "cmd.comp_key_band",
                    merge,
                );
            }
            app.comp_editor.wheel_at = Some(app.comp_editor.clock);
        }
    }
    let delete =
        ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace));
    if delete && resp.hovered() {
        if let Some(b) = app.comp_editor.key_band {
            app.remove_key_band(strip, effect, b);
        }
    }
    resp.context_menu(|ui| {
        let Some(b) = app.comp_editor.key_band.filter(|b| bands[*b].used) else {
            ui.label(t("comp.key_eq_empty"));
            return;
        };
        ui.label(RichText::new(key_band_words(b, &bands[b])).text_style(design::caption()));
        ui.separator();
        for shape in EqShape::ALL {
            if ui
                .selectable_label(
                    bands[b].shape == shape,
                    t(&format!("eq.shape.{}", shape.index())),
                )
                .clicked()
            {
                app.set_comp(strip, effect, &key(b, "shape"), shape.index() as f32, false);
                ui.close();
            }
        }
        ui.separator();
        if ui
            .selectable_label(!bands[b].on, t("eq.bypass_band"))
            .clicked()
        {
            let on = if bands[b].on { 0.0 } else { 1.0 };
            app.set_comp(strip, effect, &key(b, "on"), on, false);
            ui.close();
        }
        if ui.button(t("eq.remove_band")).clicked() {
            app.remove_key_band(strip, effect, b);
            ui.close();
        }
    });
}

/// How this compressor answers a burst: the reduction over time for a step 12 dB over
/// the threshold held 400 ms, then 20 dB under it — run through the real DSP with the
/// settings as they are, with the attack and release it measures. It is what "attack"
/// means in this style, drawn.
fn response_graph(app: &mut CantusApp, ui: &mut egui::Ui, s: &CompSettings, size: egui::Vec2) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let stale = app
        .comp_editor
        .response
        .as_ref()
        .is_none_or(|(c, _)| c != s);
    if stale {
        app.comp_editor.response = Some((*s, step_response(s, RESPONSE_SR, RESPONSE_POINTS)));
    }
    let Some((_, r)) = &app.comp_editor.response else {
        return;
    };
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let plot = Rect::from_min_max(
        rect.min + vec2(space::S2, space::S8 + space::S2),
        rect.max - vec2(space::S2, space::S4),
    );
    let x = |ms: f32| plot.left() + (ms / r.length_ms).clamp(0.0, 1.0) * plot.width();
    let scale = (r.settled_db * 1.3).max(6.0);
    let y = |gr: f32| plot.top() + (gr / scale).clamp(0.0, 1.0) * plot.height();
    // The burst, as a band behind the curve.
    let burst = Rect::from_x_y_ranges(x(r.rise_ms)..=x(r.fall_ms), plot.y_range());
    p.rect_filled(
        burst,
        0.0,
        pal.text_dim
            .gamma_multiply(if pal.dark { 0.18 } else { 0.1 }),
    );
    for ms in [0.0f32, 250.0, 500.0, 750.0, 1_000.0] {
        p.vline(x(ms), plot.y_range(), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(x(ms) + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            format!("{ms:.0}"),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    p.hline(
        plot.x_range(),
        y(r.settled_db),
        Stroke::new(1.0, pal.grid_bar),
    );
    let dt = r.length_ms / r.gr_db.len() as f32;
    let pts: Vec<Pos2> = r
        .gr_db
        .iter()
        .enumerate()
        .map(|(i, g)| pos2(x(i as f32 * dt), y(*g)))
        .collect();
    let mut mesh = egui::Mesh::default();
    let red = pal
        .danger
        .gamma_multiply(if pal.dark { 0.35 } else { 0.25 });
    for (i, pt) in pts.iter().enumerate() {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(pt.x, plot.top()), red);
        mesh.colored_vertex(*pt, red);
        if i > 0 {
            mesh.add_triangle(base - 2, base - 1, base);
            mesh.add_triangle(base - 1, base, base + 1);
        }
    }
    p.add(mesh);
    p.add(egui::Shape::line(pts, Stroke::new(1.5, pal.danger)));
    // The measured points: 63 % in, 63 % out.
    if let Some(a) = r.attack_ms {
        let at = pos2(x(r.step_ms + a), y(r.settled_db * 0.632));
        p.circle_stroke(at, 3.5, Stroke::new(1.5, pal.text));
    }
    if let Some(rel) = r.release_ms {
        let at = pos2(x(r.drop_ms + rel), y(r.settled_db * 0.368));
        p.circle_stroke(at, 3.5, Stroke::new(1.5, pal.text));
    }
    p.text(
        rect.left_top() + vec2(space::S2, space::S1),
        Align2::LEFT_TOP,
        t("comp.response"),
        design::paint::caption(),
        pal.text,
    );
    let measured = match (r.attack_ms, r.release_ms) {
        (Some(a), Some(rel)) => tf(
            "comp.response_times",
            &[
                ("att", &crate::widgets::knob::readout(a, "ms")),
                ("rel", &crate::widgets::knob::readout(rel, "ms")),
                ("gr", &format!("{:.1}", r.settled_db)),
            ],
        ),
        (Some(a), None) => tf(
            "comp.response_held",
            &[("att", &crate::widgets::knob::readout(a, "ms"))],
        ),
        _ => t("comp.response_none"),
    };
    p.text(
        rect.left_top() + vec2(space::S2, space::S1 + 13.0),
        Align2::LEFT_TOP,
        measured,
        design::paint::ruler(),
        pal.danger,
    );
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
    resp.on_hover_text(t("comp.response_tip"));
}

/// A small, read-only transfer curve for the inspector's effect card, with the numbers
/// that matter at rest: threshold, ratio, style, and the auto gain and lookahead if on.
pub fn mini_curve(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let pal = theme::p();
    let s = cantus_engine::comp_settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let lo = -60.0f32;
    let sq = Rect::from_min_size(rect.min, vec2(h, h));
    let x = |d: f32| sq.left() + ((d - lo) / -lo) * sq.width();
    let y = |d: f32| sq.bottom() - ((d - lo) / -lo).clamp(0.0, 1.0) * sq.height();
    p.line_segment(
        [pos2(x(lo), y(lo)), pos2(x(0.0), y(0.0))],
        Stroke::new(1.0, pal.grid_bar),
    );
    let pts: Vec<Pos2> = (0..=60)
        .map(|i| {
            let d = lo + -lo * i as f32 / 60.0;
            pos2(x(d), y(d - s.mix * effective_gain_reduction_db(d, &s)))
        })
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.5, pal.gold)));
    let mut words = format!(
        "{:+.0} dB \u{b7} {} \u{b7} {}",
        s.threshold_db,
        ratio_text(s.ratio),
        style_name(&s)
    );
    if s.auto_gain {
        words.push_str(&format!(" \u{b7} {}", t("comp.auto")));
    }
    if s.lookahead_ms > 0.0 {
        words.push_str(&format!(" \u{b7} {:.1} ms", s.lookahead_ms));
    }
    p.text(
        pos2(sq.right() + space::S2, rect.center().y),
        Align2::LEFT_CENTER,
        words,
        design::paint::ruler(),
        pal.text,
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
    resp.on_hover_text(t("comp.open_tip"))
}

/// The strip menu's "Duck this strip from…": a submenu of the strips that can key it
/// (never one that would close a loop); choosing one adds a Comprimo keyed from it, set
/// to duck, in one step of undo, and opens its editor.
pub fn duck_menu(app: &mut CantusApp, ui: &mut egui::Ui, strip: StripId) {
    ui.menu_button(t("comp.duck.menu"), |ui| {
        caption(ui, "comp.duck.hint");
        let options: Vec<StripId> = app
            .project()
            .strips
            .iter()
            .map(|x| x.id)
            .filter(|id| *id != strip && !app.project().routing_would_cycle(*id, Some(strip)))
            .collect();
        for id in options {
            let name = app.capture_label(cantus_engine::CaptureTarget::Strip(id));
            if ui.button(name).clicked() {
                if let Some(effect) = app.duck_strip_from(strip, id) {
                    app.open_effect_editor(strip, effect);
                }
                ui.close();
            }
        }
    });
}

/// A strip's gain-reduction lane on the desk, beside its level meter: shown on a strip
/// that carries a Comprimo, hanging from the top in the danger colour over 0–24 dB (ticks
/// every 6 dB), so a strip that is working reads at a glance and the faders stay in line.
pub fn strip_gr_meter(app: &CantusApp, ui: &mut egui::Ui, strip: StripId, height: f32) {
    let Some(gr) = app.comp_gr_on_strip(strip) else {
        return;
    };
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(vec2(space::GRID * 1.5, height), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 1.0, theme::recess(&pal));
    if let Some(g) = gr {
        let h = (g / 24.0).clamp(0.0, 1.0) * rect.height();
        if h > 0.5 {
            p.rect_filled(
                Rect::from_min_size(rect.min, vec2(rect.width(), h)).shrink(0.5),
                0.5,
                pal.danger,
            );
        }
    }
    for db in [6.0f32, 12.0, 18.0] {
        let y = rect.top() + db / 24.0 * rect.height();
        p.hline(rect.x_range(), y, Stroke::new(1.0, pal.grid_bar));
    }
    p.rect_stroke(
        rect,
        1.0,
        Stroke::new(1.0, pal.danger.gamma_multiply(0.6)),
        StrokeKind::Inside,
    );
    let tip = match gr {
        Some(g) => tf("comp.strip_gr", &[("db", &format!("{g:.1}"))]),
        None => t("comp.strip_gr_idle"),
    };
    resp.on_hover_text(tip);
}

/// `--editor comp` and the screenshots: a compressor doing a job — a Punch style gently
/// customised, lookahead, auto gain, and a key that ignores the low end — with the engine
/// drawer open, so every part of the editor has something to show.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let values: Vec<(String, f32)> = [
        ("style", 1.0),
        ("thr", -26.0),
        ("ratio", 4.0),
        ("knee", 9.0),
        ("att", 6.0),
        ("rel", 140.0),
        ("hold", 4.0),
        ("look", 2.0),
        ("auto_gain", 1.0),
        ("custom", 1.0),
        ("det_rms", 0.1),
        ("det_fb", 0.4),
        ("det_prog", 0.0),
        ("det_dual", 0.35),
        ("det_curve", 0.0),
        ("sc0.used", 1.0),
        ("sc0.shape", EqShape::LowCut.index() as f32),
        ("sc0.freq", 90.0),
        ("sc0.slope", 24.0),
        ("sc1.used", 1.0),
        ("sc1.freq", 2_500.0),
        ("sc1.gain", 4.0),
        ("sc1.q", 1.2),
    ]
    .iter()
    .map(|(k, v)| (k.to_string(), *v))
    .collect();
    app.set_comp_many(strip, effect, &values, "cmd.fx_preset", None);
    let ed = &mut app.comp_editor;
    ed.engine_open = true;
    // A stand-in transport: a beat every 30 frames, as the screenshot's drum loop.
    ed.demo = Some(DemoClock {
        tick: 0.0,
        step: cantus_core::PPQ as f64 / 30.0,
    });
}

/// What a playing engine would report, for screenshots and tests with no engine: the
/// real DSP run over a drum loop with the effect's settings — its telemetry, its
/// loudness blocks, the analyser's pairs, and (when nothing has been fed yet) the history.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect)) = app.comp_editor.open else {
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
    let mut c = cantus_dsp::Comprimo::new(DISPLAY_SR);
    c.set(&cantus_engine::comp_settings(&spec));
    let frame = 800;
    let mut seed = 5u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let fill_history = app.comp_editor.history.is_empty();
    let mut pairs = Vec::with_capacity(8_192 * 2);
    let mut last_hops = 0.0;
    let frames = 600;
    for f in 0..frames {
        let mut l = Vec::with_capacity(frame);
        for i in 0..frame {
            // A kick every 30 frames, a snare between, hats, and a bass line under it.
            let n = f * frame + i;
            let beat = (n % (30 * frame)) as f32 / DISPLAY_SR;
            let kick = (-beat * 18.0).exp() * (std::f32::consts::TAU * 55.0 * beat).sin();
            let off = ((n + 15 * frame) % (30 * frame)) as f32 / DISPLAY_SR;
            let snare = if (n / (30 * frame)) % 2 == 1 {
                (-off * 25.0).exp() * noise() * 0.6
            } else {
                0.0
            };
            let hat =
                (-(((n % (15 * frame / 2)) as f32) / DISPLAY_SR) * 60.0).exp() * noise() * 0.15;
            let bass = 0.12 * (std::f32::consts::TAU * 82.0 * n as f32 / DISPLAY_SR).sin();
            l.push(0.8 * kick + snare + hat + bass);
        }
        let dry = l.clone();
        let mut r = l.clone();
        c.process(&mut l, &mut r, None);
        let m = c.meter();
        if fill_history {
            app.comp_editor.feed(m.input, m.output, m.gr_db);
        }
        let tel = c.telemetry();
        let hops = tel[tele::HOPS];
        let new = ((hops - last_hops) as usize).min(cantus_dsp::comprimo::BLOCKS_REPORTED);
        for i in (0..new).rev() {
            app.comp_editor.blocks.push(tel[tele::BLOCKS + i]);
        }
        last_hops = hops;
        if f >= frames - 11 {
            for (a, b) in dry.iter().zip(&l) {
                pairs.push(*a);
                pairs.push(*b);
            }
        }
    }
    app.comp_editor.analyser.push(&pairs);
    app.comp_editor.analyser.compute();
    // The bell on the key is selected, so its numbers are on screen.
    if app.comp_editor.key_band.is_none() {
        app.comp_editor.key_band = Some(1);
    }
    let m = c.meter();
    app.comp_editor.now = (db(m.input), db(m.input) - m.gr_db);
    app.fx.injected = Some(c.telemetry());
}
