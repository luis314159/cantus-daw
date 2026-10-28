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
//!   card shows the five numbers behind it — peak or RMS window, feedback, program
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
//! **The look** (ADR 0024 §8): the device frame's header and footer (presets, A/B, undo,
//! bypass, the latency and oversampling chips, the output meter), and a body of cards in
//! the dynamics hue — the style, the three displays, the knobs as cells (the threshold is
//! the one XL knob: it is where the compressor starts working, the line on the history
//! and the point on the curve), and the key in the tone hue.
//!
//! **Any host** (ADR 0020): [`editor`] draws on an [`FxHost`]; [`framed`] is Cantus's
//! (the project, every drag one undo step). What only Cantus has — the engine's
//! per-frame meter, the analyser tap it drains, a demo's stand-in clock — goes through
//! `host.daw()`; in a plug-in window the history reads the unit's telemetry and the tap
//! the plug-in keeps.

use crate::app::CantusApp;
use crate::i18n::{t, tf, Locale};
use crate::kit::cell::{self, CellLook, Control};
use crate::kit::hint::{self, Caps};
use crate::kit::knob::{KnobLook, START, SWEEP};
use crate::kit::menu::Item;
use crate::kit::shell::Grid;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{band, with_brush, Function, Style};
use crate::views::fx::controls::{self, Size};
use crate::views::fx::frame;
use crate::views::fx::host::{DawHost, FxHost, KeySource, KeyTarget};
use crate::views::fx::FactoryPreset;
use cantus_core::fx::compressor::{KEY_BANDS, KEY_BAND_FIELDS};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::comprimo::{
    loudness, tele, CompSettings, Listen, StepResponse, Style as CompStyle, StyleSpec, RATIO_INF,
};
use cantus_dsp::{EqShape, SpectrumAnalyser};
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;

mod desk;
mod history;
mod key;
mod transfer;

pub use desk::{demo_feed, demo_setup, duck_menu, mini_curve, strip_gr_meter};

/// Columns of history kept: at a frame a column, a little over a minute.
const HISTORY_MAX: usize = 4096;
/// Columns across the history in the Fast and Slow speeds.
const VISIBLE: usize = 480;
/// Frames folded into one column at the Slow speed.
const SLOW_FRAMES: u32 = 4;
/// The scales the displays and meters can show, dB.
pub const RANGES: [f32; 5] = [24.0, 36.0, 48.0, 60.0, 72.0];
/// The scales as the field prints them.
const RANGE_TEXT: [&str; 5] = ["24 dB", "36 dB", "48 dB", "60 dB", "72 dB"];
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
/// The analyser's length, pairs.
const ANALYSER_N: usize = 4096;
/// Gesture numbers for [`FxHost::apply`]: the engine card, and a key band (plus its
/// number).
const ENGINE_GESTURE: u64 = 0xC0_0001;
const BAND_GESTURE: u64 = 0xC0_0100;

// The body's rows at 100 % (ADR 0024 §8: cards on the device ground, 4-pt gutters).
/// The style card: the styles and what the chosen one is.
const STYLE_H: f32 = 84.0;
/// The top row with the engine card open beside the styles.
const ENGINE_H: f32 = 126.0;
/// The key's row: its source and knobs, its EQ, the response graph.
const KEY_ROW: f32 = 150.0;
/// The meters card beside the transfer curve.
const METERS_W: f32 = 212.0;
/// The engine card's column of words before its cells.
const ENGINE_COLUMN: f32 = 120.0;
/// The key card's column (its source) before its cells.
const KEY_COLUMN: f32 = 150.0;
/// The strip at the top of the history display that holds its controls.
const HISTORY_HEAD: f32 = 30.0;
/// The strip at the top of the transfer display with the curve's numbers.
const TRANSFER_HEAD: f32 = 20.0;

/// The styles' names.
const STYLE_KEYS: [&str; 9] = [
    "comp.style.0",
    "comp.style.1",
    "comp.style.2",
    "comp.style.3",
    "comp.style.4",
    "comp.style.5",
    "comp.style.6",
    "comp.style.7",
    "comp.style.8",
];
/// What to hear, in the order the segmented control shows it: (the `listen` value, its
/// name).
const LISTEN_ORDER: [(usize, &str); 5] = [
    (0, "comp.listen.0"),
    (3, "comp.listen.3"),
    (1, "comp.listen.1"),
    (2, "comp.listen.2"),
    (4, "comp.compare"),
];
const OS_KEYS: [&str; 3] = ["comp.os.0", "comp.os.1", "comp.os.2"];
/// Aequalis's shape names, by index.
const SHAPE_KEYS: [&str; 9] = [
    "eq.shape.0",
    "eq.shape.1",
    "eq.shape.2",
    "eq.shape.3",
    "eq.shape.4",
    "eq.shape.5",
    "eq.shape.6",
    "eq.shape.7",
    "eq.shape.8",
];

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

impl Speed {
    const ALL: [Speed; 3] = [Speed::Fast, Speed::Slow, Speed::Whole];
    const KEYS: [&'static str; 3] = ["comp.speed.fast", "comp.speed.slow", "comp.speed.whole"];

    fn index(self) -> usize {
        Speed::ALL.iter().position(|s| *s == self).unwrap_or(0)
    }
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
    const SILENT: Column = Column {
        inp: -120.0,
        out: -120.0,
        gr: 0.0,
        tick: None,
    };

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

/// A string kept until what it says changes.
type Cached<K> = Option<(K, String)>;

/// `slot`'s string, made again by `make` when `key` is not the one it was made for.
fn cached<K: PartialEq>(slot: &mut Cached<K>, key: K, make: impl FnOnce() -> String) -> &str {
    if slot.as_ref().is_none_or(|(k, _)| *k != key) {
        *slot = Some((key, make()));
    }
    slot.as_ref().map_or("", |(_, s)| s.as_str())
}

/// The editor's words that are not fixed, formatted when what they say changes.
#[derive(Debug, Default)]
struct Words {
    /// Per (host revision, language): the style line, the style's name as shown
    /// ("Custom (from Punch)"), the base's segment label ("Punch*"), "Back to Punch".
    style: Option<((u64, Locale), [String; 4])>,
    scrolled: Cached<(usize, Locale)>,
    whole: Cached<(u32, Locale)>,
    effective: Cached<(u64, Locale)>,
    /// The response graph's measured line, made with the response.
    response: Cached<Locale>,
    band: Cached<(usize, Locale)>,
}
/// What the editor remembers between frames (Cantus keeps it in `app.comp_editor`; a
/// plug-in window keeps its own).
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
    /// The engine card.
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
    /// The key's spectrum, made when the first pairs arrive.
    analyser: Option<SpectrumAnalyser>,
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
    /// The UI clock, and when the wheel last moved something (a burst is one undo step).
    clock: f64,
    wheel_at: Option<f64>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chip_latency: frame::ChipText,
    chip_os: frame::ChipText,
    words: Words,
    /// The key's source as the host said it, per host revision.
    key: Option<(u64, Option<KeySource>)>,
    /// Points of the curves being drawn, reused.
    pts: Vec<Pos2>,
    /// The key EQ's curve at its display points, per host revision.
    key_curve: Option<(u64, [f32; KEY_POINTS])>,
    /// The key band's menu, open at a point (from its ⋯ or a right-click on a dot).
    band_menu: Option<(Pos2, Rect)>,
    /// The ratio as dragged on its knob, before the click stops pull it.
    ratio_drag: Option<f32>,
    /// Ticks in a bar and in a beat, for the history's grid.
    beats: (i64, i64),
    /// The song's tempo map while it has metre changes (Cantus; review 3): the history's
    /// bars and beats through it. `None`: one bar length.
    pub(super) map: Option<std::sync::Arc<cantus_core::TempoMap>>,
}

/// The editor's state on any host: [`editor`] draws it.
pub type State = CompEditor;

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
    /// Nothing allocates here: the app swaps the editor out and back every frame.
    fn default() -> Self {
        CompEditor {
            open: None,
            history: VecDeque::new(),
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
            live: Column::SILENT,
            hold: [0.0; 3],
            key_band: None,
            key_drag: None,
            analyser: None,
            scratch: Vec::new(),
            blocks: Vec::new(),
            last_hops: 0.0,
            response: None,
            demo: None,
            geometry: CompGeometry::default(),
            clock: 0.0,
            wheel_at: None,
            chrome: frame::Chrome::default(),
            chip_latency: frame::ChipText::default(),
            chip_os: frame::ChipText::default(),
            words: Words::default(),
            key: None,
            pts: Vec::new(),
            key_curve: None,
            band_menu: None,
            ratio_drag: None,
            beats: (4 * cantus_core::PPQ, cantus_core::PPQ),
            map: None,
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
    let mut s = String::new();
    ratio_pill(r, &mut s);
    s
}

/// `4.0:1`, `∞:1`, written into `out`.
fn ratio_pill(r: f32, out: &mut String) {
    if r >= RATIO_INF - 0.05 {
        out.push_str("\u{221e}:1");
    } else {
        let _ = write!(out, "{r:.1}:1");
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

/// A loudness reading, or a dash for "nothing measured".
struct Lufs(f32);

impl std::fmt::Display for Lufs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 <= loudness::SILENCE + 1.0 || !self.0.is_finite() {
            f.write_str("\u{2014}")
        } else {
            write!(f, "{:.1}", self.0)
        }
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
        if self.history.len() >= HISTORY_MAX {
            if self.speed == Speed::Whole {
                // The whole take no longer fits: fold every pair of columns into one and
                // take twice as many frames per column from now on — in place.
                let n = self.history.len();
                for i in 0..n / 2 {
                    let c = self.history[2 * i].fold(self.history[2 * i + 1]);
                    self.history[i] = c;
                }
                if n % 2 == 1 {
                    self.history[n / 2] = self.history[n - 1];
                }
                self.history.truncate(n.div_ceil(2));
                self.whole_frames = self.whole_frames.saturating_mul(2);
            } else {
                self.history.pop_front();
            }
        }
        if self.history.capacity() == 0 {
            self.history.reserve_exact(HISTORY_MAX);
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

    /// Start over for a newly opened effect: the history, the meters, the loudness, the
    /// key's spectrum.
    fn restart(&mut self) {
        self.history.clear();
        self.pending = None;
        self.whole_frames = 1;
        self.scroll = 0;
        self.hold = [-120.0, 0.0, -120.0];
        self.live = Column::SILENT;
        self.blocks.clear();
        self.last_hops = 0.0;
        self.analyser = None;
        self.response = None;
        self.key_band = None;
        self.band_menu = None;
        self.key = None;
        self.key_curve = None;
    }
}
impl CantusApp {
    pub fn open_comp_editor(&mut self, strip: StripId, effect: EffectId) {
        let ed = &mut self.comp_editor;
        ed.open = Some((strip, effect));
        ed.restart();
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

    /// Open whichever editor an effect has — or, when it is open already (shown, under
    /// another window, or in the tray), bring it back and to the front as it is (round 27,
    /// ADR 0051): the mixer's *Open the editor*, the inspector's slot.
    pub fn open_effect_editor(&mut self, strip: StripId, effect: EffectId) {
        // A hosted plug-in opens in its view: its own window or Cantus's (ADR 0073).
        if self.project().effect(strip, effect).map(|e| e.spec.kind) == Some(EffectKind::External) {
            self.open_hosted(crate::views::plugin_notice::Target::Insert(effect));
            return;
        }
        // A hosted effect's own window, open — perhaps minimised to the taskbar or under
        // other windows: back and to the front too (`open_window` only brings it forward).
        if self
            .plugins
            .hosted(effect)
            .is_some_and(|h| h.window_open() && !h.window_floating())
        {
            let owner = self.plugins.owner_window;
            let _ = self.plugins.open_window(effect, "", owner);
        }
        let at = Some((strip, effect));
        let open = self.eq_editor.open == at
            || self.comp_editor.open == at
            || self.fx.open.map(|(s, e, _)| (s, e)) == at;
        if open {
            if let Some(ctx) = &self.theme_ctx {
                let key = crate::views::fx::device_id(effect).value();
                crate::views::workspace_device::present(ctx, key);
            }
            return;
        }
        let kind = self.project().effect(strip, effect).map(|e| e.spec.kind);
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

    /// Forget a key-EQ band: every field it stored goes, as one command.
    pub fn remove_key_band(&mut self, strip: StripId, effect: EffectId, band: usize) {
        remove_band(&mut DawHost::new(self, strip, effect, "comp"), band);
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
        let slot = self.ducking_compressor();
        let effect = slot.id;
        // One batch, built where every routing batch is (round 25, ADR 0041).
        let cmd = cantus_core::routing::plan::key_with(
            self.project(),
            key,
            strip,
            slot,
            "cmd.comp_duck",
        )?;
        self.exec(cmd).then_some(effect)
    }

    /// A Comprimo set to duck (see [`Self::duck_strip_from`]), with a fresh id.
    pub fn ducking_compressor(&mut self) -> cantus_core::EffectSlot {
        let bpm = crate::views::bars::bpm_at(self.project(), self.engine.position()) as f32;
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
        slot
    }

    /// The gain reduction a strip's compressors are applying now, dB, if the engine
    /// reports it: today the one whose editor is open (its telemetry). `None` when the
    /// strip has no compressor, `Some(None)` when it has one the engine does not report.
    pub fn comp_gr_on_strip(&self, strip: StripId) -> Option<Option<f32>> {
        let effects = self.project().effects_of(strip)?;
        let working = |id: EffectId| {
            effects
                .iter()
                .any(|e| e.id == id && e.spec.kind == EffectKind::Compressor && !e.bypass)
        };
        if !effects
            .iter()
            .any(|e| e.spec.kind == EffectKind::Compressor && !e.bypass)
        {
            return None;
        }
        Some(match self.comp_editor.open {
            Some((st, fx)) if st == strip && working(fx) => {
                let t = self.fx_telemetry();
                (t[tele::SAMPLE_RATE] > 0.0).then_some(t[tele::GR])
            }
            // Every other strip: the engine's per-strip reduction (the deepest of its
            // working compressors), in the same order as the desk's meters.
            // The master's chain: its own reduction (ADR 0026).
            _ if strip.is_master() => Some(self.engine.meters().master_gr),
            _ => {
                let index = self.project().strips.iter().position(|s| s.id == strip)?;
                self.engine.meters().strip_gr.get(index).copied()
            }
        })
    }

    /// Pick a style: its own detector, leaving any custom engine — one step of undo.
    pub fn set_comp_style(&mut self, strip: StripId, effect: EffectId, style: CompStyle) {
        set_style(&mut DawHost::new(self, strip, effect, "comp"), style);
    }

    /// Set one of the engine's numbers. The first move away from a style starts from the
    /// style's own numbers and switches the engine to custom, in the same step of undo; a
    /// gesture folds frame after frame into it.
    pub fn set_comp_engine(
        &mut self,
        strip: StripId,
        effect: EffectId,
        key: &str,
        value: f32,
        gesture: bool,
    ) {
        set_engine(
            &mut DawHost::new(self, strip, effect, "comp"),
            key,
            value,
            gesture,
        );
    }
}

// ---- what the editor does, on any host ----------------------------------------------------

/// The key of field `f` of key band `b` (`sc2.freq`), as the parameter table keeps it:
/// nothing is formatted.
fn band_key(b: usize, f: usize) -> &'static str {
    let infos = cantus_core::fx::compressor::params();
    let fields = KEY_BAND_FIELDS.len();
    let base = infos.len() - KEY_BANDS * fields;
    infos[base + b.min(KEY_BANDS - 1) * fields + f.min(fields - 1)].key
}

/// The fields of a key band, by their place in `KEY_BAND_FIELDS`.
const F_USED: usize = 0;
const F_ON: usize = 1;
const F_SHAPE: usize = 2;
const F_FREQ: usize = 3;
const F_GAIN: usize = 4;
const F_Q: usize = 5;
const F_SLOPE: usize = 6;

/// Pick a style: its own detector, leaving any custom engine — one step.
pub fn set_style(host: &mut dyn FxHost, style: CompStyle) {
    host.set_many(
        &[("style", style.index() as f32), ("custom", 0.0)],
        "cmd.comp_style",
    );
}

/// The engine's five keys and a detector's numbers for them.
fn engine_values(d: &StyleSpec) -> [(&'static str, f32); 5] {
    [
        ("det_rms", d.rms_ms.max(0.1)),
        ("det_fb", d.feedback),
        ("det_prog", d.program_release),
        ("det_dual", d.dual),
        ("det_curve", d.curve),
    ]
}

/// Set one of the engine's numbers: the first move away from a style starts from the
/// style's own numbers and makes the engine custom, in the same step; a gesture folds
/// frame after frame into it.
pub fn set_engine(host: &mut dyn FxHost, key: &str, value: f32, gesture: bool) {
    let cs = cantus_engine::comp_settings(host.spec());
    let g = gesture.then_some(ENGINE_GESTURE);
    if cs.engine.is_none() {
        let v = engine_values(&cs.detector());
        let edits = [
            ("custom", Some(1.0)),
            (v[0].0, Some(v[0].1)),
            (v[1].0, Some(v[1].1)),
            (v[2].0, Some(v[2].1)),
            (v[3].0, Some(v[3].1)),
            (v[4].0, Some(v[4].1)),
            (key, Some(value)),
        ];
        host.apply(&edits, "cmd.comp_engine", g);
    } else {
        host.apply(&[(key, Some(value))], "cmd.comp_engine", g);
    }
}

/// Forget a key-EQ band: every field it stored goes back to nothing, as one step.
pub fn remove_band(host: &mut dyn FxHost, b: usize) {
    let mut edits: [(&str, Option<f32>); 7] = [("", None); 7];
    let mut n = 0;
    for f in 0..KEY_BAND_FIELDS.len().min(7) {
        let k = band_key(b, f);
        if host.spec().params.contains_key(k) {
            edits[n] = (k, None);
            n += 1;
        }
    }
    if n > 0 {
        host.apply(&edits[..n], "cmd.comp_key_band", None);
    }
}

/// What a style's detector is, in words — the answer to "what does Opto mean?".
pub fn style_words(style: CompStyle) -> String {
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
    let base = t(STYLE_KEYS[s.style.index().min(8)]);
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
// ---- the window ---------------------------------------------------------------------------

/// The open Comprimo's window, drawn every frame while one is open.
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some((strip, effect)) = app.comp_editor.open else {
        return;
    };
    if app.project().effect(strip, effect).is_none() {
        app.close_comp_editor();
        return;
    }
    if app.comp_editor.demo.is_some() && app.fx.injected.is_none() {
        demo_feed(app);
    }
    let mut pos = app.fx.pos;
    let open = crate::views::fx::open_window(
        ctx,
        egui::Id::new(("fx-editor", effect.0)),
        &mut pos,
        EffectKind::Compressor,
        |ui| framed(app, ui, strip, effect),
    );
    app.fx.pos = pos;
    if !open {
        app.close_comp_editor();
    }
}

/// Comprimo's editor in Cantus's own window: the host is the project (every drag one undo
/// step, the key any strip of the desk).
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.comp_editor);
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "comp"), &mut st, ui);
    app.comp_editor = st;
    out
}

/// Comprimo's editor on any host (ADR 0020): Cantus's window, or a plug-in window inside
/// another DAW — the device frame and the compressor inside it.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    st.clock = ui.input(|i| i.time);
    // A wheel burst is one step of undo: sealed half a second after the last notch.
    if st.wheel_at.is_some_and(|t| st.clock - t > WHEEL_GESTURE_S) {
        st.wheel_at = None;
        host.end_gesture();
    }
    pump(host, st);
    let tel = Tele(host.telemetry());
    let cs = cantus_engine::comp_settings(host.spec());
    let sr = if tel.live() {
        tel.get(tele::SAMPLE_RATE)
    } else {
        host.sample_rate()
    };
    // The latency the lookahead and the oversampling cost, always in view: the research's
    // most repeated complaint is latency that appears silently.
    let latency =
        cs.lookahead_samples(sr) + cantus_dsp::comprimo::oversample::latency(cs.oversample);
    let mut lat = std::mem::take(&mut st.chip_latency);
    let mut os = std::mem::take(&mut st.chip_os);
    let chips = [
        lat.latency(latency as f32, sr),
        os.oversampling(cs.oversample as u32),
    ];
    let meter = frame::Levels {
        level: [st.live.out; 2],
        hold: [st.hold[2]; 2],
    };
    let look = frame::Look {
        kind: EffectKind::Compressor,
        chips: &chips,
        meter: Some(meter),
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &cs)
    });
    st.chrome = chrome;
    st.chip_latency = lat;
    st.chip_os = os;
    out
}

/// Once a frame: the meter into the history, the analyser's pairs into the key's
/// spectrum, new loudness blocks into the integrated reading.
fn pump(host: &mut dyn FxHost, st: &mut State) {
    let tel = Tele(host.telemetry());
    let playing_at = host.transport().playing_at;
    let playing = playing_at.is_some();
    st.scratch.clear();
    let [inp, out, gr] = match host.daw() {
        Some((app, _, _)) => {
            // A real engine reporting ends a demo (`--editor comp` with a device).
            if st.demo.is_some() && app.engine.meters().fx[tele::SAMPLE_RATE] > 0.0 {
                st.demo = None;
                app.fx.injected = None;
            }
            // The bar and the beat at the playhead (ADR 0060).
            let map = crate::views::bars::map(app.project());
            let ts = map.signature_at(app.engine.position());
            st.beats = (ts.ticks_per_bar().0.max(1), ts.ticks_per_beat().0.max(1));
            st.map = (!map.is_one_metre()).then_some(map);
            app.engine.drain_analyser(&mut st.scratch);
            // The largest of every block since the last frame.
            app.engine.take_comp_meter()
        }
        None => {
            // A plug-in window: the unit's last block, and the tap the plug-in keeps.
            let pairs = host.pairs();
            let take = pairs.len().min(ANALYSER_N);
            for (a, b) in pairs.iter().skip(pairs.len() - take) {
                st.scratch.push(*a);
                st.scratch.push(*b);
            }
            let bar = host.transport().ticks_per_bar.max(1);
            st.beats = (bar, cantus_core::PPQ);
            if tel.live() {
                [
                    tel.get(tele::IN_PEAK),
                    tel.get(tele::OUT_PEAK),
                    tel.get(tele::GR),
                ]
            } else {
                [0.0; 3]
            }
        }
    };
    let demo = st.demo.is_some();
    if !demo {
        let col = Column {
            inp: db(inp),
            out: db(out),
            gr,
            tick: playing_at,
        };
        st.meter(col);
        // Frozen by hand, or stopped and silent: the picture of the last take stays.
        let silent = inp < 1e-5 && gr <= 0.0;
        if !st.frozen && !(silent && !playing) {
            st.push(col);
        }
        if inp > 0.0 {
            st.now = (db(inp), db(inp) - gr);
        }
    }
    // The held peaks fall at 12 dB/s (about 0.2 dB a frame); reduction at the same rate.
    st.hold[0] = (st.hold[0] - 0.2).max(-120.0);
    st.hold[1] = (st.hold[1] - 0.2).max(0.0);
    st.hold[2] = (st.hold[2] - 0.2).max(-120.0);
    if !st.scratch.is_empty() {
        let an = st
            .analyser
            .get_or_insert_with(|| SpectrumAnalyser::new(ANALYSER_N));
        an.push(&st.scratch);
        an.compute();
    }
    // The loudness blocks: the telemetry carries the last eight with a step counter, so
    // a slow frame loses none. Integrated loudness counts what played.
    if tel.live() {
        let hops = tel.get(tele::HOPS);
        if hops < st.last_hops {
            st.last_hops = 0.0; // the engine was reset
        }
        let new = ((hops - st.last_hops) as usize).min(cantus_dsp::comprimo::BLOCKS_REPORTED);
        if (playing || demo) && st.last_hops > 0.0 {
            for i in (0..new).rev() {
                st.blocks.push(tel.get(tele::BLOCKS + i));
            }
        }
        st.last_hops = hops;
    }
}

/// The widths of a row's cards, as the grid's fractions: `N` rects `h` tall.
fn row<const N: usize>(g: &mut Grid, h: f32, widths: [f32; N]) -> [Rect; N] {
    let sum: f32 = widths.iter().sum::<f32>().max(1.0);
    let fr: [f32; N] = std::array::from_fn(|i| widths[i] / sum);
    let mut it = g.row(h, &fr);
    std::array::from_fn(|_| it.next().unwrap_or(Rect::NOTHING))
}

/// The compressor inside the frame: the style (and the engine), the three displays, the
/// knobs, the key.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &Tele,
    cs: &CompSettings,
) {
    // Every card, row and column below shares one of two styles made once per look.
    let parent = ui.style().clone();
    let sp = styles(&parent, s);
    ui.set_style(sp[0].clone());
    body_cards(host, st, ui, s, body, tel, cs);
    ui.set_style(parent);
}

/// [`body_ui`]'s cards, on the shared style.
fn body_cards(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &Tele,
    cs: &CompSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let w = area.width() - 2.0 * gap;
    let row_a = if st.engine_open { ENGINE_H } else { STYLE_H } * k;
    let row_c = (controls::card_height(Size::XL) + 6.0) * k;
    let row_d = KEY_ROW * k;
    let row_b = (area.height() - row_a - row_c - row_d - 3.0 * gap).max(120.0 * k);

    // Row A: the styles and what the chosen one is; the engine beside them when open.
    if st.engine_open {
        let engine_w = (ENGINE_COLUMN + 4.0 + controls::cells_width(&[Size::M; 5]) + 16.0) * k;
        let [a, b] = row(&mut g, row_a, [area.width() - engine_w - gap, engine_w]);
        style_card(host, st, ui, s, a, cs);
        engine_card(host, st, ui, s, b, cs);
    } else {
        let a = g.full(row_a);
        style_card(host, st, ui, s, a, cs);
    }

    // Row B: the history, the transfer curve (square), the meters and the numbers.
    let title = (band::TITLE + 1.0) * k;
    let side = (row_b - title - TRANSFER_HEAD * k).clamp(80.0 * k, w * 0.4);
    let meters_w = METERS_W * k;
    let [hr, tr, mr] = row(
        &mut g,
        row_b,
        [(w - side - meters_w).max(80.0 * k), side, meters_w],
    );
    let disp = |r: Rect| (r.height() / k - band::TITLE - 1.0).max(20.0);
    controls::card_with(
        ui,
        s,
        hr,
        Src::Key("comp.history"),
        None,
        Some(disp(hr)),
        |ui, cr| {
            if let Some(d) = cr.display {
                history::show(host, st, ui, s, cs, d);
            }
        },
    );
    controls::card_with(
        ui,
        s,
        tr,
        Src::Key("comp.card.transfer"),
        None,
        Some(disp(tr)),
        |ui, cr| {
            if let Some(d) = cr.display {
                transfer::show(host, st, ui, s, cs, tel, d);
            }
        },
    );
    controls::card_with(
        ui,
        s,
        mr,
        Src::Key("comp.card.meters"),
        None,
        Some(disp(mr)),
        |ui, cr| {
            if let Some(d) = cr.display {
                transfer::meters(st, ui, s, cs, tel, d);
            }
        },
    );

    // Row C: the knobs — dynamics (the threshold, XL), time, output.
    let wd = controls::cells_width(&[Size::XL, Size::M, Size::M, Size::M]) + 16.0;
    let wt = controls::cells_width(&[Size::M; 4]) + 16.0;
    let wo = controls::cells_width(&[Size::M; 5]) + 16.0;
    let spare = (w / k - wd - wt - wo).max(0.0);
    let [dr, tir, or] = row(
        &mut g,
        row_c,
        [wd + spare * 0.25, wt + spare * 0.25, wo + spare * 0.5],
    );
    dynamics_card(host, st, ui, s, dr);
    time_card(host, ui, s, tir, cs);
    output_card(host, ui, s, or, cs);

    // Row D: the key (tone hue): its source and knobs, its EQ; the response to a burst.
    let wk = KEY_COLUMN + 4.0 + controls::cells_width(&[Size::M; 3]) + 16.0;
    let wr = (w / k - wk - 500.0).clamp(200.0, 320.0);
    let [kr, er, rr] = row(&mut g, row_d, [wk, w / k - wk - wr, wr]);
    key::key_card(host, st, ui, s, kr);
    key::key_eq_card(host, st, ui, s, cs, er);
    key::response_card(st, ui, s, cs, rr);
}
// ---- the style and the engine -------------------------------------------------------------

/// Refresh the style's words when the settings or the language changed.
fn style_words_for(st: &mut State, rev: u64, cs: &CompSettings) {
    let key = (rev, crate::i18n::locale());
    if st.words.style.as_ref().is_some_and(|(k, _)| *k == key) {
        return;
    }
    let base = t(STYLE_KEYS[cs.style.index().min(8)]);
    let name = style_name(cs);
    let det = detector_words(&cs.detector());
    let line = if cs.engine.is_some() {
        format!("{name} \u{b7} {det}")
    } else {
        det
    };
    let star = format!("{base}*");
    let back = tf("comp.engine_back", &[("style", &base)]);
    st.words.style = Some((key, [line, name, star, back]));
}

/// A line of words in a box, elided at its end; hovering it puts the whole line on the
/// help line.
fn words_line(ui: &mut egui::Ui, s: &Style, text: &str, color: egui::Color32, name: &str, w: f32) {
    let (rect, resp) = ui.allocate_exact_size(vec2(w * s.k, s.d(18.0)), Sense::hover());
    if resp.hovered() {
        hint::set_text(ui.ctx(), name, text, Caps::NONE);
    }
    if ui.is_rect_visible(rect) {
        with_brush(ui.painter(), |b| {
            let t = Text::new(text, Role::Body, color)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = ktext::paint(b, rect, &t, s.tk());
        });
    }
}

/// The styles, as one segmented control (a custom engine's base keeps a mark, so "from
/// where" is visible), the engine's switch, and what the detector in force is, in words.
fn style_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    cs: &CompSettings,
) {
    style_words_for(st, host.revision(), cs);
    let idx = cs.style.index().min(8);
    let custom = cs.engine.is_some();
    let engine_open = st.engine_open;
    let mut open_engine = false;
    let mut pick = None;
    let Some((_, [line, _, star, _])) = st.words.style.as_ref() else {
        return;
    };
    controls::card(ui, s, rect, Src::Key("fx.comp.style"), None, |ui, cr| {
        let w = cr.body.width() / s.k;
        let top = ((cr.body.height() / s.k - 44.0) * 0.5).max(0.0);
        col(ui, s, w, false, |ui| {
            ui.add_space(top * s.k);
            ui.horizontal(|ui| {
                gap_x(ui, s);
                let led_w = if engine_open { 0.0 } else { 104.0 };
                let seg_w = if engine_open { w } else { w - led_w - 4.0 };
                let labels: [Src<'_>; 9] = std::array::from_fn(|i| {
                    if custom && i == idx {
                        Src::Str(star.as_str())
                    } else {
                        Src::Key(STYLE_KEYS[i])
                    }
                });
                // A custom engine lights no style: clicking its base goes back to it.
                let selected = if custom { labels.len() } else { idx };
                // The style is a parameter: its menu, its mark, the learn mode (round 29,
                // review 2 of ADR 0062, #44).
                pick = controls::segmented_key(
                    ui,
                    s,
                    host,
                    "style",
                    selected,
                    &labels,
                    frame::tr("fx.comp.style"),
                    seg_w,
                );
                if !engine_open
                    && controls::led(
                        ui,
                        s,
                        false,
                        Src::Key("comp.engine"),
                        frame::tr("comp.engine_tip"),
                        led_w,
                    )
                    .clicked()
                {
                    open_engine = true;
                }
            });
            let color = if custom { s.hue } else { s.t.text_dim };
            words_line(ui, s, line, color, frame::tr("fx.comp.style"), w);
        });
    });
    if let Some(i) = pick {
        set_style(host, CompStyle::ALL[i.min(8)]);
    }
    if open_engine {
        st.engine_open = true;
    }
}

fn rms_pill(x: f32, b: &mut String) {
    if x <= 0.1 {
        b.push_str(frame::tr("comp.peak_short"));
    } else {
        controls::write_readout(x, "ms", b);
    }
}

fn prog_pill(x: f32, b: &mut String) {
    let _ = write!(b, "\u{d7}{x:.1}");
}

fn curve_pill(x: f32, b: &mut String) {
    if x <= 0.005 {
        b.push_str(frame::tr("comp.curve_exp"));
    } else if x >= 0.995 {
        b.push_str(frame::tr("comp.curve_lin"));
    } else {
        let _ = write!(b, "{:.0}%", x * 100.0);
    }
}

/// The engine: the five numbers a style is, as cells. Turning one makes the style "Custom
/// (from …)", starting from the style's own numbers — one undo step per gesture. Its lamp
/// closes the card.
fn engine_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    cs: &CompSettings,
) {
    let custom = cs.engine.is_some();
    let Some((_, [_, name, _, back])) = st.words.style.as_ref() else {
        return;
    };
    let mut go_back = false;
    let (_, lamp) = controls::card(ui, s, rect, Src::Key("comp.engine"), Some(true), |ui, _| {
        let explain = frame::tr(if custom {
            "comp.engine_custom"
        } else {
            "comp.engine_style"
        });
        col(ui, s, ENGINE_COLUMN, false, |ui| {
            let spare = (ui.available_height() / s.k - if custom { 44.0 } else { 18.0 }) * 0.5;
            ui.add_space(spare.max(0.0) * s.k);
            let color = if custom { s.hue } else { s.t.text };
            words_line(ui, s, name, color, explain, ENGINE_COLUMN);
            if custom
                && controls::key(ui, s, Src::Str(back), explain, ENGINE_COLUMN, false).clicked()
            {
                go_back = true;
            }
        });
        for (key, value) in engine_values(&cs.detector()) {
            let Some(info) = EffectKind::Compressor.param(key) else {
                continue;
            };
            let fmt: Pill<'_> = match key {
                "det_rms" => Some(&rms_pill),
                "det_prog" => Some(&prog_pill),
                "det_curve" => Some(&curve_pill),
                _ => None,
            };
            let mut c = controls::Cell::param(info, value).size(Size::M);
            if let Some(f) = fmt {
                c = c.format(f);
            }
            let out = c.show_in(ui, s, host, key);
            if let Some(v) = out.changed {
                set_engine(host, key, v, true);
            }
            if out.done {
                host.end_gesture();
            }
        }
    });
    if go_back {
        host.set("custom", 0.0, None);
    }
    if lamp {
        st.engine_open = false;
    }
}
// ---- the knobs ----------------------------------------------------------------------------

fn db1_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.1} dB");
}

fn range_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.0} dB");
}

fn att_pill(v: f32, b: &mut String) {
    if v < 1.0 {
        let _ = write!(b, "{v:.2} ms");
    } else {
        controls::write_readout(v, "ms", b);
    }
}

fn off_or(v: f32, off: bool, unit: &str, b: &mut String) {
    if off {
        b.push_str(frame::tr("comp.off"));
    } else {
        controls::write_readout(v, unit, b);
    }
}

fn look_pill(v: f32, b: &mut String) {
    off_or(v, v <= 0.005, "ms", b);
}

fn dry_pill(v: f32, b: &mut String) {
    off_or(v, v <= cantus_dsp::comprimo::DRY_OFF_DB + 0.05, "dB", b);
}

fn hpf_pill(v: f32, b: &mut String) {
    off_or(v, v <= 20.5, "Hz", b);
}

/// The height a column of rows takes at 100 %, and the space that centres it in `ui`.
fn centre(ui: &mut egui::Ui, s: &Style, content: f32) {
    let spare = (ui.available_height() / s.k - content) * 0.5;
    if spare > 0.0 {
        ui.add_space(spare * s.k);
    }
}

/// The height of an M cell at 100 %.
const CELL_M: f32 = 86.0;

/// A row exactly `h` points tall (at 100 %), its items centred on it: a caption beside a
/// switch lines up with the switch, and no row grows by egui's minimum row height.
fn hrow<R>(ui: &mut egui::Ui, s: &Style, h: f32, f: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(
        vec2(w, s.d(h)),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.set_min_height(s.d(h));
            gap_x(ui, s);
            f(ui)
        },
    )
    .inner
}

thread_local! {
    /// The body's two styles — the parent's with the kit's 4 × 4-pt gaps (a card's body,
    /// a row) and 4 × 5 (a column) — kept until the look changes, so no `Ui` clones its
    /// style every frame (a clone of egui's style is a dozen allocations).
    static SPACED: RefCell<Option<Spaced>> = const { RefCell::new(None) };
}

/// The parent style and scale they were made for, and the two styles.
type Spaced = (Arc<egui::Style>, u32, [Arc<egui::Style>; 2]);

/// A cell's value formatter.
type Pill<'a> = Option<&'a dyn Fn(f32, &mut String)>;

/// The two styles for `parent` at the device's scale (made when either changed).
fn styles(parent: &Arc<egui::Style>, s: &Style) -> [Arc<egui::Style>; 2] {
    SPACED.with(|c| {
        let mut c = c.borrow_mut();
        let fresh = c.as_ref().is_some_and(|(p, k, _)| {
            *k == s.k.to_bits() && (Arc::ptr_eq(p, parent) || **p == **parent)
        });
        if !fresh {
            let make = |y: f32| {
                let mut st = (**parent).clone();
                st.spacing.item_spacing = vec2(s.d(4.0), s.d(y));
                Arc::new(st)
            };
            *c = Some((parent.clone(), s.k.to_bits(), [make(4.0), make(5.0)]));
        }
        match c.as_ref() {
            Some((_, _, sp)) => sp.clone(),
            None => [parent.clone(), parent.clone()],
        }
    })
}

/// The shared style: 0 — 4 × 4-pt gaps, 1 — 4 × 5.
fn spaced(ui: &egui::Ui, i: usize) -> Arc<egui::Style> {
    SPACED.with(|c| match c.borrow().as_ref() {
        Some((_, _, sp)) => sp[i.min(1)].clone(),
        None => ui.style().clone(),
    })
}

/// A column of switches inside a card's body (as `controls::column`, on the shared
/// style): 5-pt gaps, or 4 when `tight`.
fn col<R>(
    ui: &mut egui::Ui,
    s: &Style,
    width: f32,
    tight: bool,
    f: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let style = spaced(ui, if tight { 0 } else { 1 });
    ui.allocate_ui_with_layout(
        vec2(width * s.k, ui.available_height()),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            ui.set_style(style);
            f(ui)
        },
    )
    .inner
}

/// The kit's 4-pt gap between the items of a row, set only when it is not already.
pub(super) fn gap_x(ui: &mut egui::Ui, s: &Style) {
    if ui.spacing().item_spacing.x != s.d(4.0) {
        ui.spacing_mut().item_spacing.x = s.d(4.0);
    }
}

/// The ratio, with its click stops: the knob lands on 1.5, 2, 3, 4, 6, 8, 10, 20 and ∞
/// when near them (Shift: free), and the stops are ticks around it.
fn ratio_cell(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style) {
    let spec = host.spec();
    let Some(info) = spec.kind.param("ratio") else {
        return;
    };
    // While dragged, the knob follows the pointer and the value follows the stops.
    let shown = st.ratio_drag.unwrap_or(spec.get("ratio"));
    let out = controls::Cell::param(info, shown)
        .size(Size::M)
        .format(&ratio_pill)
        .show_in(ui, s, host, "ratio");
    if let Some(v) = out.changed {
        let free = ui.input(|i| i.modifiers.shift);
        st.ratio_drag = out.resp.dragged().then_some(v);
        host.set("ratio", if free { v } else { snap_ratio(v) }, Some("ratio"));
    }
    if out.done {
        st.ratio_drag = None;
        host.end_gesture();
    }
    if !ui.is_rect_visible(out.resp.rect) {
        return;
    }
    let look = CellLook {
        control: Control::Knob(KnobLook::new(Size::M, 0.0, 0.0)),
        value: "",
        label: Src::Str(""),
    };
    let c = cell::rects(out.resp.rect, s, &look).control.center();
    let r = s.s(Size::M.diameter()) * 0.5;
    with_brush(ui.painter(), |b| {
        let px = b.px();
        let color = s.t.knob_track;
        for stop in RATIO_STOPS {
            let a = START + SWEEP * (stop.ln() / RATIO_INF.ln());
            let d = vec2(a.cos(), a.sin());
            b.segment(
                c + d * (r + s.s(1.5)),
                c + d * (r + s.s(4.0)),
                px,
                color,
                false,
            );
        }
    });
}

/// When it starts and how much: the threshold (the device's primary parameter, the one XL
/// knob), the ratio with its stops, the knee, the range — and the oversampling, which runs
/// the gain stage and the detector faster.
fn dynamics_card(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("comp.group.dynamics"),
        None,
        |ui, _| {
            controls::param(ui, s, host, "thr", Size::XL);
            let w = controls::cells_width(&[Size::M; 3]);
            col(ui, s, w, false, |ui| {
                centre(ui, s, 86.0 + 5.0 + controls::ROW);
                hrow(ui, s, CELL_M, |ui| {
                    ratio_cell(host, st, ui, s);
                    controls::param_with(ui, s, host, "knee", Size::M, Some(&db1_pill));
                    controls::param_with(ui, s, host, "range", Size::M, Some(&range_pill));
                });
                hrow(ui, s, controls::ROW, |ui| {
                    let cap = 92.0;
                    controls::caption(ui, s, Src::Key("fx.comp.os"), cap);
                    let labels = OS_KEYS.map(Src::Key);
                    controls::segmented_param(ui, s, host, "os", &labels, w - cap - 4.0);
                });
            });
        },
    );
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

/// The release as a note value at the tempo: the nearest one is named in the field, and
/// its list snaps to any — a snap, not a mode, so the release stays in ms.
fn note_field(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, w: f32, rel_ms: f32) {
    let bpm = host.transport().ticks_per_second / cantus_core::PPQ as f64 * 60.0;
    let quarter_ms = 60_000.0 / (bpm as f32).max(1.0);
    let near = NOTES
        .iter()
        .find(|(_, q)| ((q * quarter_ms) / rel_ms - 1.0).abs() < 0.03)
        .map(|(n, _)| *n);
    let id = ui.id().with("comp-note");
    let value = near.unwrap_or(frame::tr("comp.note"));
    let Some(at) = controls::field(ui, s, id, value, frame::tr("comp.note_tip"), w) else {
        return;
    };
    // The rows exist only while the list is open.
    let rows: Vec<(String, f32, bool)> = NOTES
        .iter()
        .map(|(name, q)| (*name, q * quarter_ms))
        .filter(|(_, ms)| (5.0..=5_000.0).contains(ms))
        .map(|(name, ms)| {
            let mut text = format!("{name}   ");
            controls::write_readout(ms, "ms", &mut text);
            (text, ms, near == Some(name))
        })
        .collect();
    let items: Vec<Item<'_>> = rows
        .iter()
        .map(|(text, _, on)| frame::radio(Src::Str(text), *on))
        .collect();
    if let Some(i) = controls::list(ui, s, id, at, w.max(170.0), &items) {
        if let Some((_, ms, _)) = rows.get(i) {
            host.set("rel", *ms, None);
        }
    }
}

/// How fast it takes hold and lets go: attack, release (auto, and snapped to a note),
/// hold, lookahead.
fn time_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, cs: &CompSettings) {
    controls::card(ui, s, rect, Src::Key("comp.group.time"), None, |ui, cr| {
        let w = (cr.body.width() / s.k).min(controls::cells_width(&[Size::M; 4]) + 40.0);
        col(ui, s, w, false, |ui| {
            centre(ui, s, 86.0 + 5.0 + controls::ROW);
            hrow(ui, s, CELL_M, |ui| {
                controls::param_with(ui, s, host, "att", Size::M, Some(&att_pill));
                controls::param(ui, s, host, "rel", Size::M);
                controls::param(ui, s, host, "hold", Size::M);
                controls::param_with(ui, s, host, "look", Size::M, Some(&look_pill));
            });
            hrow(ui, s, controls::ROW, |ui| {
                let half = (controls::cells_width(&[Size::M; 4]) - 4.0) * 0.5;
                controls::led_param(ui, s, host, "auto_rel", Src::Key("fx.comp.auto_rel"), half);
                note_field(host, ui, s, half, cs.release_ms);
            });
        });
    });
}

/// The output, in the output hue: make-up (and auto gain), the dry signal, the mix, the
/// gains in and out — and what to hear.
fn output_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect, cs: &CompSettings) {
    let so = s.function(Function::Output);
    let s = &so;
    controls::card(
        ui,
        s,
        rect,
        Src::Key("comp.group.output"),
        None,
        |ui, cr| {
            let w = cr.body.width() / s.k;
            col(ui, s, w, true, |ui| {
                centre(ui, s, 86.0 + 4.0 + 20.0 + 4.0 + controls::ROW);
                hrow(ui, s, CELL_M, |ui| {
                    controls::param(ui, s, host, "makeup", Size::M);
                    controls::param_with(ui, s, host, "dry", Size::M, Some(&dry_pill));
                    controls::param(ui, s, host, "mix", Size::M);
                    controls::param(ui, s, host, "in_gain", Size::M);
                    controls::param(ui, s, host, "out_gain", Size::M);
                });
                hrow(ui, s, 20.0, |ui| {
                    controls::led_param(
                        ui,
                        s,
                        host,
                        "auto_gain",
                        Src::Key("fx.comp.auto_gain"),
                        controls::cells_width(&[Size::M; 2]),
                    );
                });
                hrow(ui, s, controls::ROW, |ui| {
                    let cap = 40.0;
                    controls::caption(ui, s, Src::Key("comp.listen"), cap);
                    let now = cs.listen.index();
                    let sel = LISTEN_ORDER
                        .iter()
                        .position(|(v, _)| *v == now)
                        .unwrap_or(0);
                    let labels = LISTEN_ORDER.map(|(_, k)| Src::Key(k));
                    if let Some(i) = controls::segmented_key(
                        ui,
                        s,
                        host,
                        "listen",
                        sel,
                        &labels,
                        frame::tr("fx.comp.listen"),
                        w - cap - 4.0,
                    ) {
                        host.set("listen", LISTEN_ORDER[i].0 as f32, None);
                    }
                });
            });
        },
    );
}

/// Where the detector's key comes from: any strip of the desk (Cantus), or the plug-in's
/// sidechain input, switched on (another DAW routes it).
fn key_source(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, w: f32) {
    let rev = host.revision();
    if st.key.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.key = Some((rev, host.key_source()));
    }
    let mut target = None;
    match st.key.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips {
            current,
            label,
            options,
        }) => {
            let current = *current;
            let id = ui.id().with("comp-key");
            let value = if current.is_none() {
                frame::tr("comp.key_own")
            } else {
                label.as_str()
            };
            if let Some(at) = controls::field(ui, s, id, value, frame::tr("comp.key_tip"), w) {
                let mut items = vec![frame::radio(Src::Key("comp.key_own"), current.is_none())];
                items.extend(
                    options
                        .iter()
                        .map(|(sid, text)| frame::radio(Src::Str(text), current == Some(*sid))),
                );
                if let Some(i) = controls::list(ui, s, id, at, w.max(180.0), &items) {
                    let to = if i == 0 {
                        None
                    } else {
                        options.get(i - 1).map(|o| o.0)
                    };
                    target = Some(KeyTarget::Strip(to));
                }
            }
        }
        Some(KeySource::Switch(on)) => {
            let labels = [
                Src::Key("comp.key_own"),
                Src::Key("export.sidechain.external"),
            ];
            if let Some(i) =
                controls::segmented(ui, s, *on as usize, &labels, frame::tr("comp.key_tip"), w)
            {
                target = Some(KeyTarget::Switch(i == 1));
            }
        }
        None => {}
    }
    if let Some(t) = target {
        host.set_key_source(t);
    }
}
