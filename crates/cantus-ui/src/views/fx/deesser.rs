//! Sibilus's editor: the de-esser's window, where the display is the control. Owned by
//! Sibilus; nothing else edits this file.
//!
//! The benchmark (`research/fabfilter/pro-ds.md`) shows *where* it de-esses but not *why*,
//! and nothing on its display can be touched. Here three lanes share one time axis — the
//! audio's own, lookahead taken out, measured by the DSP in 11 ms columns so no "s" falls
//! between two frames:
//!
//! - **Signal**: the input, mirrored. Where it was de-essed the column turns green: the
//!   solid part is what is left, the lighter hatched part what was taken away.
//! - **Cut**: the reduction in dB, hanging from the top, on the Range's scale.
//! - **Detector**: the band's level, with the **threshold as a line you drag**; the part
//!   over it is filled, and in Single vocal the classifier's confidence runs along the
//!   bottom — level over the line *and* sibilant is what makes it cut. At the right edge, a
//!   histogram of where the sibilants peaked: what **Learn** sets the threshold from.
//!
//! Over the lanes, the guard rails against over-de-essing, as readouts: events, events in
//! the last 10 s, the average and the deepest cut, and a warning past 6 dB. Under them,
//! the **detection band is two edges dragged on the spectrum** of the input (drag the
//! middle to move both), with the cut the processing makes drawn over it live; beside it,
//! the switches and the cells, every one keyboard-reachable. The wheel scrolls the
//! history back; a double-click returns to live. Every drag and turn is one undo step.
//!
//! The editor runs on any [`FxHost`] (ADR 0020): in Cantus the key is any strip of the
//! desk; in a plug-in window the host routes the sidechain and the editor switches to it.

use super::canvas::{self, Ink};
use super::controls::{self, Cell, Size};
use super::frame;
use super::host::{DawHost, FxHost, KeySource, KeyTarget};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::space;
use crate::i18n::{t, tf};
use crate::kit::hint::{self, Caps};
use crate::kit::shell::Grid;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{band, with_brush, Brush, Fill, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::sibilus::{
    self as ds, Column, DeessSettings, Detection, Listen, Processing, Sibilus,
};
use cantus_dsp::SpectrumAnalyser;
use cantus_engine::fx::deesser as tele;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Write as _;

/// Columns of history kept: about 30 s at 48 kHz.
pub const HISTORY_COLS: usize = 2_800;
/// Points per column on screen, at 100 %.
const PX: f32 = 2.0;
/// The detector lane's scale, dB.
const DET_LO_DB: f32 = -60.0;
/// The spectrum's axis.
const SPEC_LO_HZ: f32 = 200.0;
const SPEC_HI_HZ: f32 = 20_000.0;
const SPEC_FLOOR_DB: f32 = -96.0;
/// Music falls about this fast; with the tilt a sibilant stands out as it sounds.
const SPEC_TILT: f32 = 4.5;
/// How fast the spectrum falls back, dB per frame.
const SPEC_FALL: f32 = 1.5;
const POINTS: usize = 200;
/// The sample rate assumed until the engine reports one.
const DEFAULT_SR: f32 = 48_000.0;
/// Above this average cut per event the editor warns: past it consonants start to lisp.
pub const HEAVY_DB: f32 = 6.0;

/// Factory presets: a label key and the parameters it sets. Anything not named goes back
/// to its default (Single vocal, wide band, 5 ms lookahead, L/R linked).
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "ds.preset.male",
        &[
            ("thr", -34.0),
            ("range", 6.0),
            ("lo", 4_000.0),
            ("hi", 9_000.0),
            ("rel", 50.0),
        ],
    ),
    (
        "ds.preset.female",
        &[
            ("thr", -34.0),
            ("range", 6.0),
            ("lo", 5_500.0),
            ("hi", 12_000.0),
            ("rel", 40.0),
        ],
    ),
    (
        "ds.preset.voiceover",
        &[
            ("thr", -40.0),
            ("range", 8.0),
            ("proc", 1.0),
            ("lo", 4_500.0),
            ("hi", 11_000.0),
            ("rel", 60.0),
            ("look", 8.0),
        ],
    ),
    (
        "ds.preset.backing",
        &[
            ("thr", -36.0),
            ("range", 8.0),
            ("stereo", 3.0),
            ("lo", 5_000.0),
            ("hi", 12_000.0),
        ],
    ),
    (
        "ds.preset.cymbals",
        &[
            ("mode", 1.0),
            ("proc", 2.0),
            ("thr", -26.0),
            ("range", 4.0),
            ("lo", 6_000.0),
            ("hi", 16_000.0),
            ("att", 2.0),
            ("rel", 120.0),
            ("link", 0.5),
        ],
    ),
    (
        "ds.preset.bus",
        &[
            ("mode", 1.0),
            ("proc", 1.0),
            ("thr", -20.0),
            ("range", 3.0),
            ("lo", 8_000.0),
            ("hi", 20_000.0),
            ("att", 0.5),
            ("rel", 80.0),
        ],
    ),
];

/// The choices' words and what each does, by index.
const MODE_KEYS: [&str; 2] = ["ds.mode.0", "ds.mode.1"];
const MODE_TIPS: [&str; 2] = ["ds.mode.0_tip", "ds.mode.1_tip"];
const PROC_KEYS: [&str; 3] = ["ds.proc.0", "ds.proc.1", "ds.proc.2"];
const PROC_TIPS: [&str; 3] = ["ds.proc.0_tip", "ds.proc.1_tip", "ds.proc.2_tip"];
const STEREO_KEYS: [&str; 4] = ["ds.stereo.0", "ds.stereo.1", "ds.stereo.2", "ds.stereo.3"];
const STEREO_TIPS: [&str; 4] = [
    "ds.stereo.0_tip",
    "ds.stereo.1_tip",
    "ds.stereo.2_tip",
    "ds.stereo.3_tip",
];
const LISTEN_KEYS: [&str; 3] = ["ds.listen.0", "ds.listen.1", "ds.listen.2"];
const LISTEN_TIPS: [&str; 3] = ["ds.listen.0_tip", "ds.listen.1_tip", "ds.listen.2_tip"];

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    Threshold,
    Lo,
    Hi,
    /// The whole band, from where it was and where the pointer took it.
    Band {
        lo: f32,
        hi: f32,
        at_hz: f32,
    },
}

/// Where the displays are, published for tests: drags aim at these.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geometry {
    pub signal: Rect,
    pub cut: Rect,
    pub detector: Rect,
    pub spectrum: Rect,
}

impl Default for Geometry {
    fn default() -> Self {
        Geometry {
            signal: Rect::NOTHING,
            cut: Rect::NOTHING,
            detector: Rect::NOTHING,
            spectrum: Rect::NOTHING,
        }
    }
}

impl Geometry {
    /// The detector lane's height of a level, dB.
    pub fn detector_y(&self, db: f32) -> f32 {
        let f = ((db - DET_LO_DB) / -DET_LO_DB).clamp(0.0, 1.0);
        self.detector.bottom() - f * self.detector.height()
    }

    pub fn detector_db(&self, y: f32) -> f32 {
        DET_LO_DB + (self.detector.bottom() - y) / self.detector.height().max(1.0) * -DET_LO_DB
    }

    pub fn spectrum_x(&self, hz: f32) -> f32 {
        let f = (hz.max(1.0) / SPEC_LO_HZ).ln() / (SPEC_HI_HZ / SPEC_LO_HZ).ln();
        self.spectrum.left() + f * self.spectrum.width()
    }

    pub fn spectrum_hz(&self, x: f32) -> f32 {
        let f = (x - self.spectrum.left()) / self.spectrum.width().max(1.0);
        SPEC_LO_HZ * (SPEC_HI_HZ / SPEC_LO_HZ).powf(f)
    }
}

fn point_hz(i: usize) -> f32 {
    SPEC_LO_HZ * (SPEC_HI_HZ / SPEC_LO_HZ).powf(i as f32 / (POINTS - 1) as f32)
}

/// Words the editor formats only when what they say changes.
#[derive(Debug, Default)]
struct Words {
    /// The key's source as the host said it, per host revision, and the field's words.
    key: Option<(u64, Option<KeySource>)>,
    key_for: Option<(u64, crate::i18n::Locale)>,
    key_text: String,
    /// Learn's key: (learning, sibilants heard, language).
    learn: Option<(bool, usize, crate::i18n::Locale)>,
    learn_text: String,
    /// The hover readout: which column, and its words.
    hover: Option<(usize, usize, crate::i18n::Locale)>,
    hover_text: String,
}

/// What the editor remembers between frames.
#[derive(Debug)]
pub struct State {
    /// The effect this state belongs to; another one starts afresh.
    effect: Option<EffectId>,
    /// The columns seen, oldest first.
    history: VecDeque<Column>,
    last_seq: Option<u64>,
    sr: f32,
    chunk: usize,
    latency: usize,
    events: ds::EventStats,
    /// Columns scrolled back from the newest; 0 is live.
    scroll: usize,
    drag: Option<Grab>,
    analyser: SpectrumAnalyser,
    spec_in: Vec<f32>,
    spec_out: Vec<f32>,
    /// The split's amplitude at each display point, for the settings it was made for.
    cut_for: Option<(Processing, u32, u32)>,
    cut_amp: Vec<f32>,
    /// Learning a threshold: the columns heard since it started.
    pub learning: bool,
    learn_cols: Vec<Column>,
    /// The threshold Learn last set, drawn beside the line while it stands.
    pub learned: Option<f32>,
    pub geo: Geometry,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    words: Words,
    /// Scratch kept between frames: the analysed pairs, a line's points.
    scratch: Vec<f32>,
    pts: Vec<Pos2>,
    /// What the tap held when it was last analysed (length, newest, oldest): the analysis
    /// runs only when the tap brought something new.
    analysed: Option<Pair3>,
}

type Pair3 = (usize, Option<(u32, u32)>, Option<(u32, u32)>);

impl Default for State {
    fn default() -> Self {
        State {
            effect: None,
            history: VecDeque::new(),
            last_seq: None,
            sr: DEFAULT_SR,
            chunk: ds::chunk_for(DEFAULT_SR),
            latency: 0,
            events: ds::EventStats::default(),
            scroll: 0,
            drag: None,
            analyser: SpectrumAnalyser::new(4096),
            spec_in: vec![SPEC_FLOOR_DB; POINTS],
            spec_out: vec![SPEC_FLOOR_DB; POINTS],
            cut_for: None,
            cut_amp: vec![1.0; POINTS],
            learning: false,
            learn_cols: Vec::new(),
            learned: None,
            geo: Geometry::default(),
            chrome: frame::Chrome::default(),
            chips: frame::ChipText::default(),
            words: Words::default(),
            scratch: Vec::new(),
            pts: Vec::new(),
            analysed: None,
        }
    }
}

impl State {
    /// Add one column of history — what the engine's telemetry does each frame; for
    /// demos and tests with no engine.
    pub fn push(&mut self, c: Column) {
        if self.history.len() == HISTORY_COLS {
            self.history.pop_front();
        }
        self.history.push_back(c);
        if self.scroll > 0 {
            // Scrolled back: the view stays where it is while the history grows.
            self.scroll = (self.scroll + 1).min(self.history.len().saturating_sub(1));
        }
        if self.learning && self.learn_cols.len() < HISTORY_COLS * 2 {
            self.learn_cols.push(c);
        }
    }

    pub fn history(&self) -> &VecDeque<Column> {
        &self.history
    }

    /// Seconds per column.
    fn col_s(&self) -> f32 {
        self.chunk as f32 / self.sr.max(1.0)
    }
}

/// Where sibilant events peaked, dB: runs of columns the classifier took for sibilance
/// (Single vocal) or that were cut by more than 1 dB (Allround), each run's loudest
/// detector level.
pub fn event_peaks(cols: &[Column], voice: bool) -> Vec<f32> {
    let mut peaks = Vec::new();
    runs(cols.iter(), voice, |p| peaks.push(p));
    peaks
}

/// Each event's peak in `cols`, to `f` — as [`event_peaks`], without a list.
fn runs<'a>(cols: impl Iterator<Item = &'a Column>, voice: bool, mut f: impl FnMut(f32)) {
    let mut run: Option<f32> = None;
    for c in cols {
        let inside = if voice {
            c.conf >= 0.5
        } else {
            c.gr_db > ds::EVENT_ON_DB
        };
        match (inside, run) {
            (true, Some(p)) => run = Some(p.max(c.det_db)),
            (true, None) => run = Some(c.det_db),
            (false, Some(p)) => {
                f(p);
                run = None;
            }
            (false, None) => {}
        }
    }
    if let Some(p) = run {
        f(p);
    }
}

/// **Learn**: the threshold from a passage. The median sibilant gets half the Range, the
/// loudest all of it, the quiet ones nothing — `median(peaks) − range / 2`. In Allround,
/// where there is no classifier, an "event" is a run of columns in the loudest quarter of
/// the band's level. `None` below three events: too little to learn from.
pub fn learn_threshold(cols: &[Column], voice: bool, range_db: f32) -> Option<f32> {
    let peaks = if voice {
        event_peaks(cols, true)
    } else {
        let mut levels: Vec<f32> = cols
            .iter()
            .filter(|c| c.input > 0.0)
            .map(|c| c.det_db)
            .collect();
        if levels.len() < 8 {
            return None;
        }
        levels.sort_by(f32::total_cmp);
        let p75 = levels[levels.len() * 3 / 4];
        let marked: Vec<Column> = cols
            .iter()
            .map(|c| Column {
                conf: if c.det_db >= p75 && c.input > 0.0 {
                    1.0
                } else {
                    0.0
                },
                ..*c
            })
            .collect();
        event_peaks(&marked, true)
    };
    if peaks.len() < 3 {
        return None;
    }
    let mut p = peaks;
    p.sort_by(f32::total_cmp);
    let median = if p.len() % 2 == 1 {
        p[p.len() / 2]
    } else {
        0.5 * (p[p.len() / 2 - 1] + p[p.len() / 2])
    };
    Some((median - range_db.max(0.0) * 0.5).clamp(DET_LO_DB, 0.0))
}

/// Start listening for a threshold, or stop and set it: one step of the host's undo.
pub fn learn(host: &mut dyn FxHost, st: &mut State) {
    if !st.learning {
        st.learning = true;
        st.learn_cols.clear();
        return;
    }
    st.learning = false;
    let cols = std::mem::take(&mut st.learn_cols);
    let s = tele::settings(host.spec());
    match learn_threshold(&cols, s.detection == Detection::Voice, s.range_db) {
        Some(thr) => {
            host.set("thr", thr, None);
            st.learned = Some(thr);
            host.status(tf("ds.learned_to", &[("db", &format!("{thr:.1}"))]));
        }
        None => host.status(t("ds.learn_none")),
    }
}

thread_local! {
    /// The state [`framed`] leaves in the app while the editor borrows its own: made once
    /// and swapped in and out, so a frame builds no new state (its analyser allocates).
    static SPARE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Run `f` on the app with the de-esser's state taken out of it (the host borrows the
/// app; the editor its state).
fn with_state<R>(app: &mut CantusApp, f: impl FnOnce(&mut CantusApp, &mut State) -> R) -> R {
    let spare = SPARE.with(|c| c.borrow_mut().take()).unwrap_or_default();
    let mut st = std::mem::replace(&mut app.fx.deesser, spare);
    let r = f(app, &mut st);
    let spare = std::mem::replace(&mut app.fx.deesser, st);
    SPARE.with(|c| *c.borrow_mut() = Some(spare));
    r
}

impl CantusApp {
    /// Start listening for a threshold, or stop and set it: one undo step.
    pub fn deesser_learn(&mut self, strip: StripId, effect: EffectId) {
        with_state(self, |app, st| {
            learn(&mut DawHost::new(app, strip, effect, "deesser"), st)
        });
    }
}

/// Bring in the columns the engine measured since the last frame.
fn pump(st: &mut State, tel: &cantus_engine::Telemetry) {
    let h = tele::header(tel);
    if h.chunk == 0 || h.sample_rate <= 0.0 {
        return;
    }
    st.sr = h.sample_rate;
    st.chunk = h.chunk;
    st.latency = h.latency;
    st.events = h.events;
    let held = h.seq.saturating_sub(ds::HISTORY as u64);
    let from = match st.last_seq {
        Some(last) if h.seq >= last => last,
        // The first look, or the effect was reset.
        _ => held,
    };
    // A stretch the editor did not see (it was closed, or a frame took too long) is
    // drawn empty, so the time axis stays true.
    let missing = held.saturating_sub(from).min(HISTORY_COLS as u64);
    for _ in 0..missing {
        st.push(Column {
            det_db: -120.0,
            ..Column::default()
        });
    }
    for index in from.max(held)..h.seq {
        if let Some(c) = tele::column(tel, h.seq, index) {
            st.push(c);
        }
    }
    st.last_seq = Some(h.seq);
}

/// The spectrum of what the analyser tap brought, and the split's shape for the settings.
fn update_spectrum(st: &mut State, pairs: &VecDeque<(f32, f32)>, s: &DeessSettings) {
    let n = st.analyser.size();
    let sig = (
        pairs.len(),
        pairs.back().map(|p| (p.0.to_bits(), p.1.to_bits())),
        pairs.front().map(|p| (p.0.to_bits(), p.1.to_bits())),
    );
    // Only when the tap brought something new: the analysis is the costliest thing the
    // editor does, and an editor at rest must cost next to nothing.
    if pairs.len() >= n && st.analysed != Some(sig) {
        st.analysed = Some(sig);
        st.scratch.clear();
        for (a, b) in pairs.iter().skip(pairs.len() - n) {
            st.scratch.push(*a);
            st.scratch.push(*b);
        }
        st.analyser.push(&st.scratch);
        st.analyser.compute();
        for i in 0..POINTS {
            let hz = point_hz(i);
            let tilt = SPEC_TILT * (hz / 1_000.0).log2();
            let a = st.analyser.level_db(0, st.sr, hz, 1.0 / 6.0) + tilt;
            let b = st.analyser.level_db(1, st.sr, hz, 1.0 / 6.0) + tilt;
            st.spec_in[i] = a.max(st.spec_in[i] - SPEC_FALL);
            st.spec_out[i] = b.max(st.spec_out[i] - SPEC_FALL);
        }
    }
    let key = (s.processing, s.lo_hz.to_bits(), s.hi_hz.to_bits());
    if st.cut_for != Some(key) {
        let mut d = Sibilus::new(st.sr);
        d.set(&DeessSettings {
            listen: Listen::Normal,
            ..*s
        });
        for (i, a) in st.cut_amp.iter_mut().enumerate() {
            *a = d.split_amplitude(point_hz(i));
        }
        st.cut_for = Some(key);
    }
}

/// Runs of columns cut by more than an event's worth that have ended — the events the
/// readouts count, so a run still going is not counted twice.
fn closed_events<'a>(cols: impl Iterator<Item = &'a Column>) -> usize {
    let mut n = 0;
    let mut inside = false;
    for c in cols {
        let now = c.gr_db > ds::EVENT_ON_DB;
        if inside && !now {
            n += 1;
        }
        inside = now;
    }
    n
}

/// `4.5 kHz`, `350 Hz`.
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

/// An axis mark: `500 Hz`, `1 kHz`, `20 kHz`.
struct HzMark(f32);

impl std::fmt::Display for HzMark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{:.0} kHz", self.0 / 1_000.0)
        } else {
            write!(f, "{:.0} Hz", self.0)
        }
    }
}

/// A level with a real minus sign, one decimal: `−38.0`, `+0.0`.
struct Signed(f32);

impl std::fmt::Display for Signed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 < -0.049 {
            write!(f, "\u{2212}{:.1}", -self.0)
        } else {
            write!(f, "+{:.1}", self.0.abs())
        }
    }
}

// ---- the frame ------------------------------------------------------------------------

/// The right of the lanes kept for their scales, at 100 %.
const GUTTER: f32 = 40.0;
/// The cells: the threshold (the primary parameter, larger), then the rest.
const DS_CELLS: [Size; 9] = [
    Size::L,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
    Size::M,
];

/// Sibilus in Cantus's own window: the host is the project (every drag one undo step, the
/// key any strip of the desk).
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    with_state(app, |app, st| {
        if st.effect != Some(effect) {
            *st = State {
                effect: Some(effect),
                ..State::default()
            };
        }
        st.chrome.reset_for(effect.0);
        editor(&mut DawHost::new(app, strip, effect, "deesser"), st, ui)
    })
}

/// Sibilus's editor on any host (ADR 0020): the device frame and the de-esser inside it.
/// Everything it needs from the host goes through [`FxHost`].
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    pump(st, &tel);
    let dss = tele::settings(host.spec());
    update_spectrum(st, host.pairs(), &dss);
    let sr = if st.sr > 1_000.0 {
        st.sr
    } else {
        host.sample_rate()
    };
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(ds::latency_for(&dss, sr) as f32, sr)];
    let meter = st.history.back().map(|c| {
        let db = if c.output > 1e-6 {
            20.0 * c.output.log10()
        } else {
            -160.0
        };
        frame::Levels {
            level: [db; 2],
            hold: [db; 2],
        }
    });
    let look = frame::Look {
        kind: EffectKind::DeEsser,
        chips: &chip,
        meter,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &dss)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// The de-esser inside the frame: the lanes on top; the spectrum, the switches and the
/// cells under them.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    dss: &DeessSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let sw_h = (band::TITLE + 8.0 + 3.0 * controls::ROW + 10.0 + 8.0) * k;
    let cells_h = controls::card_height(Size::L) * k;
    let low_h = sw_h + gap + cells_h;
    let hist_h = (area.height() - low_h - gap).max(160.0 * k);
    let hist_r = g.full(hist_h);
    let low = g.rest();
    let cw = (controls::cells_width(&DS_CELLS) + 16.0) * k;
    let right = Rect::from_min_max(pos2(low.right() - cw, low.top()), low.max);
    let spec_r = Rect::from_min_max(low.min, pos2(right.left() - gap, low.bottom()));
    let half = (right.width() - gap) * 0.5;
    let det_r = Rect::from_min_size(right.min, vec2(half, sw_h));
    let key_r = Rect::from_min_size(pos2(right.right() - half, right.top()), vec2(half, sw_h));
    let cells_r = Rect::from_min_max(pos2(right.left(), det_r.bottom() + gap), right.max);
    let disp = |r: Rect| (r.height() / k - band::TITLE - 1.0).max(20.0);
    let (_, _) = controls::card_with(
        ui,
        s,
        hist_r,
        Src::Key("ds.card.history"),
        None,
        Some(disp(hist_r)),
        |ui, cr| {
            if let Some(d) = cr.display {
                history(host, st, ui, s, dss, d);
            }
        },
    );
    let (_, _) = controls::card_with(
        ui,
        s,
        spec_r,
        Src::Key("ds.card.band"),
        None,
        Some(disp(spec_r)),
        |ui, cr| {
            if let Some(d) = cr.display {
                spectrum(host, st, ui, s, dss, d);
            }
        },
    );
    detection_card(host, ui, s, det_r);
    key_card(host, st, ui, s, key_r, dss);
    cells_card(host, ui, s, cells_r, dss);
}

/// The help line for what the pointer is on: its name and what it does.
fn tip(ui: &egui::Ui, hovered: bool, name: &str, what: &str) {
    if hovered {
        hint::set_text(ui.ctx(), name, what, Caps::NONE);
    }
}

thread_local! {
    /// A value formatted for a readout, reused.
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

/// A caps word and a number on plates, anchored at `at` by `align` (the pair reads left
/// to right): "THRESHOLD −38.0 dB". Returns the rect of both, or nothing when it would
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
    let vs = with_fmt(args, |v| {
        let t = Text::new(v, Role::Value, color).caps(false);
        ktext::measure(b.ctx(), &t, k)
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

/// Hatching for what was taken away: short diagonals across `[y0, y1]` in a column at
/// `x`, `pitch` apart and lined up from column to column.
#[allow(clippy::too_many_arguments)]
fn hatch_col(b: &mut Brush, x: f32, y0: f32, y1: f32, col_w: f32, pitch: f32, w: f32, c: Color32) {
    if y1 - y0 < 2.0 {
        return;
    }
    let mut y = y0 - (y0 + x).rem_euclid(pitch) + pitch;
    while y < y1 {
        let top = (y - col_w).max(y0);
        b.segment(pos2(x, y), pos2(x + (y - top), top), w, c, false);
        y += pitch;
    }
}

// ---- the history ------------------------------------------------------------------------

/// The colours of the history: green is what de-essing did.
struct Greens {
    dim: Color32,
    left: Color32,
    taken: Color32,
    hatch: Color32,
    bar: Color32,
}

impl Greens {
    fn of(ink: &Ink) -> Greens {
        let d = ink.dark;
        Greens {
            dim: Ink::a(ink.dim, if d { 0.35 } else { 0.26 }),
            left: Ink::a(ink.ok, if d { 0.8 } else { 0.62 }),
            taken: Ink::a(ink.ok, if d { 0.3 } else { 0.22 }),
            hatch: Ink::a(ink.ok, 0.75),
            bar: Ink::a(ink.ok, if d { 0.9 } else { 0.75 }),
        }
    }
}

/// The lanes: a head with the readouts that guard against over-de-essing, then the
/// signal, the cut and the detector, their scales in a gutter at the right.
fn history(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    dss: &DeessSettings,
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let gr = Greens::of(&ink);
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), 24.0 * k));
    let lanes = Rect::from_min_max(pos2(rect.left(), head.bottom()), rect.max);
    let gap = 4.0 * k;
    let wave = Rect::from_min_max(
        lanes.min,
        pos2(lanes.right(), lanes.top() + (lanes.height() * 0.44).round()),
    );
    let cut = Rect::from_min_max(
        pos2(lanes.left(), wave.bottom() + gap),
        pos2(
            lanes.right(),
            wave.bottom() + gap + (lanes.height() * 0.14).round(),
        ),
    );
    let det = Rect::from_min_max(pos2(lanes.left(), cut.bottom() + gap), lanes.max);
    st.geo.signal = wave;
    st.geo.cut = cut;
    st.geo.detector = det;
    let geo = st.geo;
    let resp = ui.interact(lanes, ui.id().with("ds-history"), Sense::click_and_drag());
    let voice = dss.detection == Detection::Voice;
    let right = lanes.right() - GUTTER * k;
    let px = PX * k;

    // Wheel: look back; double-click: live.
    if resp.hovered() {
        let d = ui.input(|i| i.smooth_scroll_delta);
        let step = (d.y + d.x) / px;
        if step != 0.0 {
            let max = st.history.len().saturating_sub(1);
            st.scroll = (st.scroll as f32 + step).round().clamp(0.0, max as f32) as usize;
        }
    }
    if resp.double_clicked() {
        st.scroll = 0;
    }

    let cols = ((right - lanes.left()) / px).floor() as usize;
    let end = st.history.len().saturating_sub(st.scroll);
    let start = end.saturating_sub(cols);
    // The centre of column `i`.
    let x_of = |i: usize| right - (end - i) as f32 * px + px * 0.5;
    let ty = geo.detector_y(dss.threshold_db);
    let held = st.drag == Some(Grab::Threshold);
    let hover = resp.hover_pos();
    let near = hover.is_some_and(|pt| det.contains(pt) && (pt.y - ty).abs() < 8.0 * k);
    // The hovered column, and its words when it is a new one.
    let hovered = hover.and_then(|pt| {
        let back = ((right - pt.x) / px).floor();
        (!held && pt.x < right && back >= 0.0 && (back as usize) < end - start)
            .then(|| (pt, end - 1 - back as usize))
    });
    if let Some((_, i)) = hovered {
        let lang = crate::i18n::locale();
        let key = (i, st.last_seq.unwrap_or(0) as usize, lang);
        if st.words.hover != Some(key) {
            let c = st.history[i];
            let db = |lin: f32| {
                if lin > 1e-6 {
                    format!("{:.1}", 20.0 * lin.log10())
                } else {
                    "\u{2212}\u{221e}".to_string()
                }
            };
            let mut text = tf(
                "ds.hover",
                &[
                    (
                        "ago",
                        &format!("{:.2}", (end - 1 - i + st.scroll) as f32 * st.col_s()),
                    ),
                    ("in", &db(c.input)),
                    ("det", &format!("{:.1}", c.det_db.max(-120.0))),
                    ("cut", &format!("{:.1}", c.gr_db)),
                ],
            );
            if voice {
                text.push_str(&tf(
                    "ds.hover_conf",
                    &[("conf", &format!("{:.0}", c.conf * 100.0))],
                ));
            }
            if c.gr_db >= ds::EVENT_ON_DB && c.hz > 0.0 {
                text.push_str(&tf("ds.hover_hz", &[("hz", &Hz(c.hz))]));
            }
            // En spaces: the readout keeps one face (a plain space starts a unit).
            st.words.hover_text = text.replace(' ', "\u{2002}");
            st.words.hover = Some(key);
        }
    }
    let e = st.events;
    let recent = (10.0 / st.col_s()) as usize;
    let rate = closed_events(st.history.range(st.history.len().saturating_sub(recent)..));
    let paused = (st.scroll > 0).then(|| st.scroll as f32 * st.col_s());
    let now_gr = st.history.back().map(|c| c.gr_db).unwrap_or(0.0);
    let learned = st
        .learned
        .is_some_and(|v| (v - dss.threshold_db).abs() < 0.05);

    let mut pts = std::mem::take(&mut st.pts);
    let hist = &st.history;
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let bpx = b.px();
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(head.bottom()),
            bpx,
            ink.grid,
        );
        for y in [wave.bottom() + gap * 0.5, cut.bottom() + gap * 0.5] {
            b.hline(
                rect.left(),
                rect.right(),
                b.snap_mid(y),
                bpx,
                ink.grid_strong,
            );
        }
        b.vline(
            b.snap_mid(right),
            lanes.top(),
            lanes.bottom(),
            bpx,
            ink.grid,
        );

        // ---- Signal: the input mirrored, green where de-essed ----
        let centre = wave.center().y;
        let half = wave.height() * 0.5 - 2.0 * k;
        // The square root of the amplitude: a vowel keeps its shape (a dB scale flattens
        // everything loud into a slab) and a quiet "s" is still visible (a linear one
        // loses it).
        let amp = |lin: f32| lin.clamp(0.0, 1.0).sqrt() * half;
        let active = |c: &Column| c.gr_db >= 0.5;
        b.hline(wave.left(), right, b.snap_mid(centre), bpx, ink.grid_strong);
        // ---- Cut: the reduction hanging from the top, on the Range's scale ----
        let scale = (dss.range_db.max(6.0) / 6.0).ceil() * 6.0;
        let top = cut.top() + 1.0 * k;
        let depth = |g: f32| top + (g / scale).clamp(0.0, 1.0) * (cut.height() - 2.0 * k);
        // ---- Detector: the band level, the threshold you drag, why it cut ----
        for db in [-12.0f32, -24.0, -36.0, -48.0] {
            b.hline(
                det.left(),
                right,
                b.snap_mid(geo.detector_y(db)),
                bpx,
                ink.grid,
            );
        }
        let over = |conf: f32| Ink::a(ink.hue, if voice { 0.1 + 0.5 * conf } else { 0.45 });
        let rib = 12.0 * k;
        let mut peak_gr = 0.0f32;
        let mut prev: Option<(f32, &Column)> = None;
        pts.clear();
        for (j, c) in hist.range(start..end).enumerate() {
            let x = x_of(start + j);
            peak_gr = peak_gr.max(c.gr_db);
            pts.push(pos2(x, geo.detector_y(c.det_db)));
            let Some((x0, p)) = prev else {
                prev = Some((x, c));
                continue;
            };
            let (h0, h1) = (amp(p.input), amp(c.input));
            let outer = if active(c) { gr.taken } else { gr.dim };
            b.area(
                &[pos2(x0, centre - h0), pos2(x, centre - h1)],
                centre,
                outer,
            );
            b.area(
                &[pos2(x0, centre + h0), pos2(x, centre + h1)],
                centre,
                outer,
            );
            if active(c) {
                let i0 = if active(p) {
                    amp(p.output).min(h0)
                } else {
                    0.0
                };
                let i1 = amp(c.output).min(h1);
                b.area(
                    &[pos2(x0, centre - i0), pos2(x, centre - i1)],
                    centre,
                    gr.left,
                );
                b.area(
                    &[pos2(x0, centre + i0), pos2(x, centre + i1)],
                    centre,
                    gr.left,
                );
                let xl = x - px * 0.5;
                let pitch = 6.0 * k;
                hatch_col(b, xl, centre - h1, centre - i1, px, pitch, bpx, gr.hatch);
                hatch_col(b, xl, centre + i1, centre + h1, px, pitch, bpx, gr.hatch);
            }
            b.area(
                &[pos2(x0, depth(p.gr_db)), pos2(x, depth(c.gr_db))],
                top,
                gr.bar,
            );
            let (d0, d1) = (
                geo.detector_y(p.det_db).min(ty),
                geo.detector_y(c.det_db).min(ty),
            );
            b.area(&[pos2(x0, d0), pos2(x, d1)], ty, over(c.conf));
            if voice {
                // The classifier: as tall as it is sure, along the bottom.
                let base = det.bottom() - 1.0 * k;
                b.area(
                    &[
                        pos2(x0, base - p.conf.clamp(0.0, 1.0) * rib),
                        pos2(x, base - c.conf.clamp(0.0, 1.0) * rib),
                    ],
                    base,
                    Ink::a(ink.hue, 0.8),
                );
            }
            prev = Some((x, c));
        }
        if pts.len() > 1 {
            b.polyline(&pts, 1.25 * k, ink.text);
        }
        let range_y = depth(dss.range_db);
        canvas::dashed(
            b,
            pos2(cut.left(), range_y),
            pos2(right, range_y),
            bpx,
            ink.ok,
            4.0 * k,
            4.0 * k,
        );
        // Where the sibilants peaked: a histogram against the scale, 3 dB a bin.
        let mut bins = [0usize; 20];
        runs(hist.range(start..end), voice, |pk| {
            let bin = ((pk - DET_LO_DB) / 3.0).floor();
            if (0.0..20.0).contains(&bin) {
                bins[bin as usize] += 1;
            }
        });
        let most = bins.iter().copied().max().unwrap_or(1).max(1) as f32;
        let hist_w = 32.0 * k;
        for (bin, n) in bins.iter().enumerate() {
            if *n == 0 {
                continue;
            }
            let lo = DET_LO_DB + bin as f32 * 3.0;
            let r = Rect::from_min_max(
                pos2(
                    right - *n as f32 / most * hist_w,
                    geo.detector_y(lo + 3.0) + 1.0,
                ),
                pos2(right, geo.detector_y(lo) - 1.0),
            );
            b.rrect(r, 1.0 * k, Fill::Solid(Ink::a(ink.ok, 0.45)));
            b.rrect_stroke(r, 1.0 * k, bpx, ink.ok);
        }
        // The threshold: a dashed line, heavier while held or near.
        canvas::dashed(
            b,
            pos2(det.left(), ty),
            pos2(right, ty),
            if held || near { 2.0 } else { 1.25 } * k,
            ink.hue,
            6.0 * k,
            4.0 * k,
        );
        if let Some((pt, _)) = hovered {
            b.vline(
                b.snap_mid(pt.x),
                lanes.top(),
                lanes.bottom(),
                bpx,
                ink.grid_strong,
            );
        }

        // ---- the words, last ----
        let plate = Some(ink.plate);
        let micro = |b: &mut Brush, at: Pos2, align: Align2, args: std::fmt::Arguments<'_>| {
            canvas::putf(b, at, align, Role::Micro, ink.dim, tk, None, args)
        };
        let gx = lanes.right() - 4.0 * k;
        for (db, align) in [(0.0f32, Align2::RIGHT_TOP), (-12.0, Align2::RIGHT_CENTER)] {
            let y = centre - amp(10f32.powf(db / 20.0));
            b.hline(right, right + 3.0 * k, b.snap_mid(y), bpx, ink.dim);
            if db == 0.0 {
                micro(b, pos2(gx, y), align, format_args!("0"));
            } else {
                micro(b, pos2(gx, y), align, format_args!("\u{2212}{:.0}", -db));
            }
        }
        micro(b, pos2(gx, top), Align2::RIGHT_TOP, format_args!("0"));
        micro(
            b,
            pos2(gx, cut.bottom() - 1.0 * k),
            Align2::RIGHT_BOTTOM,
            format_args!("\u{2212}{scale:.0}"),
        );
        for db in [-12.0f32, -24.0, -36.0, -48.0] {
            micro(
                b,
                pos2(gx, geo.detector_y(db)),
                Align2::RIGHT_CENTER,
                format_args!("\u{2212}{:.0}", -db),
            );
        }
        let lane_label = |b: &mut Brush, at: Pos2, align: Align2, key: &str| {
            canvas::put(
                b,
                at,
                align,
                &Text::key(key, Role::Micro, ink.dim),
                tk,
                plate,
            )
        };
        lane_label(
            b,
            wave.left_top() + vec2(6.0, 4.0) * k,
            Align2::LEFT_TOP,
            "ds.lane.signal",
        );
        legend(
            b,
            pos2(wave.left() + 6.0 * k, wave.bottom() - 4.0 * k),
            s,
            &ink,
            &gr,
        );
        let cl = lane_label(
            b,
            cut.left_top() + vec2(6.0, 3.0) * k,
            Align2::LEFT_TOP,
            "ds.lane.cut",
        );
        let r = tagged(
            b,
            pos2(cl.right() + 10.0 * k, cl.center().y),
            Align2::LEFT_CENTER,
            frame::tr("ds.tag.now"),
            ink.text,
            tk,
            plate,
            f32::NEG_INFINITY,
            format_args!("{now_gr:.1} dB"),
        );
        tagged(
            b,
            pos2(r.right() + 10.0 * k, cl.center().y),
            Align2::LEFT_CENTER,
            frame::tr("ds.tag.peak"),
            ink.text,
            tk,
            plate,
            f32::NEG_INFINITY,
            format_args!("{peak_gr:.1} dB"),
        );
        let dl = lane_label(
            b,
            det.left_top() + vec2(6.0, 3.0) * k,
            Align2::LEFT_TOP,
            "ds.lane.detector",
        );
        if voice {
            lane_label(
                b,
                pos2(det.left() + 6.0 * k, det.bottom() - rib - 3.0 * k),
                Align2::LEFT_BOTTOM,
                "ds.lane.sibilance",
            );
        }
        // The threshold's tag: over its line, or under it when the lane's name is there.
        let word = frame::tr("fx.ds.thr");
        let paint_thr = |b: &mut Brush, at: Pos2, align: Align2| {
            let r = tagged(
                b,
                at,
                align,
                word,
                ink.hue,
                tk,
                plate,
                f32::NEG_INFINITY,
                format_args!("{} dB", Signed(dss.threshold_db)),
            );
            if learned {
                canvas::put(
                    b,
                    pos2(r.right() + 6.0 * k, r.center().y),
                    Align2::LEFT_CENTER,
                    &Text::key("ds.learned", Role::Micro, ink.hue),
                    tk,
                    plate,
                );
            }
        };
        let above = ty - 3.0 * k - 16.0 * k > dl.bottom() + 2.0 * k;
        if above {
            paint_thr(
                b,
                pos2(det.left() + 6.0 * k, ty - 3.0 * k),
                Align2::LEFT_BOTTOM,
            );
        } else {
            paint_thr(
                b,
                pos2(dl.right() + 10.0 * k, ty + 3.0 * k),
                Align2::LEFT_TOP,
            );
        }

        // ---- the head: the guard rails, and live or paused ----
        let hy = head.center().y;
        let live_r = match paused {
            Some(back) => tagged(
                b,
                pos2(head.right() - 8.0 * k, hy),
                Align2::RIGHT_CENTER,
                frame::tr("ds.tag.paused"),
                ink.hue,
                tk,
                None,
                f32::NEG_INFINITY,
                format_args!("\u{2212}{back:.1} s"),
            ),
            None => canvas::put(
                b,
                pos2(head.right() - 8.0 * k, hy),
                Align2::RIGHT_CENTER,
                &Text::key("ds.tag.live", Role::Micro, ink.dim),
                tk,
                None,
            ),
        };
        let mut x = head.left() + 8.0 * k;
        if e.count == 0 {
            let r = Rect::from_min_max(
                pos2(x, head.top()),
                pos2(live_r.left() - 12.0 * k, head.bottom()),
            );
            let t = Text::key("ds.stats_none", Role::Body, ink.dim)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = ktext::paint(b, r, &t, tk);
        } else {
            let heavy = e.mean_gr_db > HEAVY_DB;
            let next = |b: &mut Brush,
                        x: &mut f32,
                        word: &str,
                        color: Color32,
                        args: std::fmt::Arguments<'_>| {
                let r = tagged(
                    b,
                    pos2(*x, hy),
                    Align2::LEFT_CENTER,
                    word,
                    color,
                    tk,
                    None,
                    f32::NEG_INFINITY,
                    args,
                );
                *x = r.right() + 14.0 * k;
            };
            next(
                b,
                &mut x,
                frame::tr("ds.tag.events"),
                ink.text,
                format_args!("{}", e.count),
            );
            next(
                b,
                &mut x,
                frame::tr("ds.tag.recent"),
                ink.text,
                format_args!("{rate}"),
            );
            next(
                b,
                &mut x,
                frame::tr("ds.tag.avg"),
                if heavy { ink.danger } else { ink.text },
                format_args!("{:.1} dB", e.mean_gr_db),
            );
            next(
                b,
                &mut x,
                frame::tr("ds.tag.max"),
                ink.text,
                format_args!("{:.1} dB", e.max_gr_db),
            );
            if heavy {
                // A warning shape as well as the colour, and what it means.
                let c = pos2(x + 6.0 * k, hy);
                let r = 5.5 * k;
                b.polyline(
                    &[
                        pos2(c.x, c.y - r),
                        pos2(c.x + r, c.y + r * 0.8),
                        pos2(c.x - r, c.y + r * 0.8),
                        pos2(c.x, c.y - r),
                    ],
                    1.5 * k,
                    ink.danger,
                );
                let lr = Rect::from_min_max(
                    pos2(x + 16.0 * k, head.top()),
                    pos2(live_r.left() - 12.0 * k, head.bottom()),
                );
                let t = Text::key("ds.heavy_short", Role::Body, ink.danger)
                    .caps(false)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End));
                let _ = ktext::paint(b, lr, &t, tk);
            }
        }

        // Hover: what happened in that column, in real units — last, over everything.
        if let Some((pt, _)) = hovered {
            let t = Text::new(&st.words.hover_text, Role::Value, ink.text).caps(false);
            let w = ktext::measure(b.ctx(), &t, tk).x;
            let x = if pt.x + 12.0 * k + w < right {
                pt.x + 8.0 * k
            } else {
                (pt.x - 8.0 * k - w).max(lanes.left() + 4.0 * k)
            };
            canvas::put(
                b,
                pos2(x, wave.top() + 22.0 * k),
                Align2::LEFT_TOP,
                &t,
                tk,
                Some(super::dynfx::on_display(&ink, s.t.deck_title)),
            );
        }
    });
    st.pts = pts;

    // The threshold is dragged where the events are. What is grabbed is decided where the
    // button went down, not where the pointer is once the drag is recognised.
    let pressed_near = ui
        .input(|i| i.pointer.press_origin())
        .is_some_and(|pt| det.contains(pt) && (pt.y - ty).abs() < 8.0 * k);
    if resp.drag_started() && pressed_near {
        st.drag = Some(Grab::Threshold);
    }
    if st.drag == Some(Grab::Threshold) && resp.dragged() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let v = geo.detector_db(pt.y).clamp(DET_LO_DB, 0.0);
            host.set("thr", v, Some("thr"));
        }
    }
    if resp.drag_stopped() && st.drag == Some(Grab::Threshold) {
        st.drag = None;
        host.end_gesture();
    }
    if near || held {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("ds.card.history"),
            frame::tr(if paused.is_some() {
                "ds.paused_tip"
            } else {
                "ds.history_tip"
            }),
            Caps::NONE,
        );
    }
}

/// The signal lane's legend: a swatch of what is left and one of what was removed.
fn legend(b: &mut Brush, at: Pos2, s: &Style, ink: &Ink, gr: &Greens) {
    let k = s.k;
    let tk = s.tk();
    let left = Text::key("ds.legend.left", Role::Micro, ink.text);
    let removed = Text::key("ds.legend.removed", Role::Micro, ink.text);
    let (w1, w2) = (
        ktext::measure(b.ctx(), &left, tk),
        ktext::measure(b.ctx(), &removed, tk),
    );
    let sw = 10.0 * k;
    let w = sw + 4.0 * k + w1.x + 10.0 * k + sw + 4.0 * k + w2.x + 6.0 * k;
    let h = w1.y.max(sw) + 4.0 * k;
    let r = Rect::from_min_size(pos2(at.x, at.y - h), vec2(w, h));
    b.rrect(b.snap_rect(r), 2.0 * k, Fill::Solid(ink.plate));
    let cy = r.center().y;
    let mut x = r.left() + 3.0 * k;
    let s1 = Rect::from_center_size(pos2(x + sw * 0.5, cy), vec2(sw, sw));
    b.rect(b.snap_rect(s1), Fill::Solid(gr.left));
    x += sw + 4.0 * k;
    let _ = ktext::paint(
        b,
        Rect::from_min_size(pos2(x, r.top()), vec2(w1.x + 1.0, r.height())),
        &left.align(Align2::LEFT_CENTER),
        tk,
    );
    x += w1.x + 10.0 * k;
    let s2 = Rect::from_center_size(pos2(x + sw * 0.5, cy), vec2(sw, sw));
    b.rect(b.snap_rect(s2), Fill::Solid(gr.taken));
    let px = b.px();
    canvas::hatch(b, s2, 4.0 * k, px, gr.hatch);
    x += sw + 4.0 * k;
    let _ = ktext::paint(
        b,
        Rect::from_min_size(pos2(x, r.top()), vec2(w2.x + 1.0, r.height())),
        &removed.align(Align2::LEFT_CENTER),
        tk,
    );
}

// ---- the spectrum -----------------------------------------------------------------------

/// Set the band's edges as one gesture: both keys in one gesture, so a drag is one undo
/// step even when it moves both.
fn set_band(host: &mut dyn FxHost, lo: Option<f32>, hi: Option<f32>) {
    for (key, v) in [("lo", lo), ("hi", hi)] {
        if let Some(v) = v {
            host.set(key, v, Some("ds-band"));
        }
    }
}

/// The detection band on the spectrum: drag its edges, or its middle to move it.
fn spectrum(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    dss: &DeessSettings,
    rect: Rect,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(rect, ui.id().with("ds-spectrum"), Sense::click_and_drag());
    st.geo.spectrum = rect;
    let geo = st.geo;
    let (lo, hi) = ds::band_edges(dss.lo_hz, dss.hi_hz, st.sr);
    let y_spec = |db: f32| {
        let f = ((db - SPEC_FLOOR_DB) / -SPEC_FLOOR_DB).clamp(0.0, 1.0);
        rect.bottom() - f * rect.height()
    };
    // The cut the processing makes: at the full Range, dashed; live, solid. On its own
    // scale, half again the Range, so a 6 dB cut is a dip you can see.
    let cut_top = rect.top() + 32.0 * k;
    let cut_h = rect.height() * 0.4;
    let cut_scale = (dss.range_db * 1.5).max(9.0);
    let y_cut = |db: f32| cut_top + (-db / cut_scale).clamp(0.0, 1.0) * cut_h;
    let response = |g_db: f32, i: usize| {
        let g = 10f32.powf(-g_db / 20.0);
        let a = st.cut_amp[i];
        20.0 * (1.0 + (g - 1.0) * a).abs().max(1e-4).log10()
    };
    let now = st.history.back().map(|c| c.gr_db).unwrap_or(0.0);
    let drag = st.drag;
    let hover = resp.hover_pos();
    let grab_w = 7.0 * k;
    let near_x = |x: f32| hover.is_some_and(|pt| (pt.x - x).abs() < grab_w);
    let (xl, xh) = (geo.spectrum_x(lo), geo.spectrum_x(hi));
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        let bpx = b.px();
        // The band, behind everything.
        b.rect(
            Rect::from_x_y_ranges(xl..=xh, rect.y_range()),
            Fill::Solid(Ink::a(ink.hue, 0.10)),
        );
        for hz in [500.0f32, 1_000.0, 2_000.0, 5_000.0, 10_000.0] {
            b.vline(
                b.snap_mid(geo.spectrum_x(hz)),
                rect.top(),
                rect.bottom(),
                bpx,
                ink.grid,
            );
        }
        // The input as a fill, glowing where the band is; the output as a line.
        let dim = Ink::a(ink.dim, if ink.dark { 0.22 } else { 0.18 });
        let glow = Ink::a(ink.hue, if ink.dark { 0.55 } else { 0.4 });
        for i in 1..POINTS {
            let (h0, h1) = (point_hz(i - 1), point_hz(i));
            let c = if h1 > lo && h0 < hi { glow } else { dim };
            b.area(
                &[
                    pos2(geo.spectrum_x(h0), y_spec(st.spec_in[i - 1])),
                    pos2(geo.spectrum_x(h1), y_spec(st.spec_in[i])),
                ],
                rect.bottom(),
                c,
            );
        }
        let mut line = [Pos2::ZERO; POINTS];
        for (i, p) in line.iter_mut().enumerate() {
            *p = pos2(geo.spectrum_x(point_hz(i)), y_spec(st.spec_out[i]));
        }
        b.polyline(&line, 1.25 * k, Ink::a(ink.dim, 0.9));
        for (db, strong) in [(0.0, true), (-dss.range_db, false)] {
            b.hline(
                rect.left(),
                rect.right(),
                b.snap_mid(y_cut(db)),
                bpx,
                Ink::a(ink.ok, if strong { 0.35 } else { 0.2 }),
            );
        }
        // The cut's lines: lime on slate, a darker lime where the ground is pale.
        let ok_l = crate::kit::color::ensure_contrast(ink.ok, ink.ground, 3.0);
        for (i, p) in line.iter_mut().enumerate() {
            *p = pos2(
                geo.spectrum_x(point_hz(i)),
                y_cut(response(dss.range_db, i)),
            );
        }
        for w in line.windows(2).step_by(2) {
            b.segment(w[0], w[1], 1.0 * k, Ink::a(ok_l, 0.8), false);
        }
        for (i, p) in line.iter_mut().enumerate() {
            *p = pos2(geo.spectrum_x(point_hz(i)), y_cut(response(now, i)));
        }
        b.polyline(&line, 2.0 * k, ok_l);
        // The last event's centre.
        let centre = (st.events.count > 0 && st.events.last_hz > SPEC_LO_HZ)
            .then(|| geo.spectrum_x(st.events.last_hz));
        let ey = rect.bottom() - 18.0 * k;
        if let Some(x) = centre {
            b.polygon(
                &[
                    pos2(x, ey - 7.0 * k),
                    pos2(x - 5.0 * k, ey),
                    pos2(x + 5.0 * k, ey),
                ],
                ink.ok,
            );
        }
        // The edges and their grips.
        for (x, grab) in [(xl, Grab::Lo), (xh, Grab::Hi)] {
            let hot = drag == Some(grab) || (drag.is_none() && near_x(x));
            b.vline(
                b.snap_mid(x),
                rect.top(),
                rect.bottom(),
                if hot { 2.5 } else { 1.5 } * k,
                ink.hue,
            );
            let grip = Rect::from_center_size(pos2(x, rect.top() + 10.0 * k), vec2(8.0, 16.0) * k);
            b.rrect(
                grip,
                2.0 * k,
                Fill::Solid(if hot { ink.text } else { ink.hue }),
            );
        }
        if let Some(pt) = hover {
            b.vline(
                b.snap_mid(pt.x),
                rect.top(),
                rect.bottom(),
                bpx,
                ink.grid_strong,
            );
        }

        // ---- the words, last ----
        let plate = Some(ink.plate);
        let mut ruler = canvas::Ruler::new();
        for hz in [500.0f32, 1_000.0, 2_000.0, 5_000.0, 10_000.0] {
            let at = pos2(geo.spectrum_x(hz) + 3.0 * k, rect.bottom() - 2.0 * k);
            if !ruler.fits(at.x, 6.0 * k) {
                continue;
            }
            let r = canvas::putf(
                b,
                at,
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                // On a plate: the band's edges may cross it.
                plate,
                format_args!("{}", HzMark(hz)),
            );
            ruler.placed(r);
        }
        // The last mark sits on the right edge: its label goes inside.
        if ruler.fits(rect.right() - 40.0 * k, 6.0 * k) {
            canvas::putf(
                b,
                pos2(rect.right() - 3.0 * k, rect.bottom() - 2.0 * k),
                Align2::RIGHT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{}", HzMark(SPEC_HI_HZ)),
            );
        }
        // The cut's scale, and what it is now.
        // Lime on paper needs a darker lime to be read.
        let ok_c = crate::kit::color::ensure_contrast(ink.ok, ink.ground, 4.5);
        canvas::putf(
            b,
            pos2(rect.right() - 3.0 * k, y_cut(0.0) - 1.0 * k),
            Align2::RIGHT_BOTTOM,
            Role::Micro,
            ok_c,
            tk,
            plate,
            format_args!("0 dB"),
        );
        canvas::putf(
            b,
            pos2(rect.right() - 3.0 * k, y_cut(-dss.range_db) - 1.0 * k),
            Align2::RIGHT_BOTTOM,
            Role::Micro,
            ok_c,
            tk,
            plate,
            format_args!("\u{2212}{:.0} dB", dss.range_db),
        );
        let r = tagged(
            b,
            pos2(rect.left() + 6.0 * k, cut_top + 4.0 * k),
            Align2::LEFT_TOP,
            frame::tr("ds.tag.cut"),
            ok_c,
            tk,
            plate,
            f32::NEG_INFINITY,
            format_args!("{now:.1} dB"),
        );
        tagged(
            b,
            pos2(r.left(), r.bottom() + 4.0 * k),
            Align2::LEFT_TOP,
            frame::tr("fx.ds.range"),
            ok_c,
            tk,
            plate,
            f32::NEG_INFINITY,
            format_args!("{:.1} dB", dss.range_db),
        );
        if let Some(x) = centre {
            tagged(
                b,
                pos2(x, ey - 9.0 * k),
                Align2::CENTER_BOTTOM,
                frame::tr("ds.tag.centre"),
                ok_c,
                tk,
                plate,
                f32::NEG_INFINITY,
                format_args!("{}", Hz(st.events.last_hz)),
            );
        }
        // The edges' frequencies at the top, beside their grips; a second row when the
        // band is too narrow for both.
        let lo_r = canvas::putf(
            b,
            pos2(xl + grab_w, rect.top() + 3.0 * k),
            Align2::LEFT_TOP,
            Role::Value,
            ink.hue,
            tk,
            plate,
            format_args!("{}", Hz(lo)),
        );
        let hi_t = with_fmt(format_args!("{}", Hz(hi)), |v| {
            ktext::measure(b.ctx(), &Text::new(v, Role::Value, ink.hue).caps(false), tk).x
        });
        let hi_y = if xh - grab_w - hi_t < lo_r.right() + 6.0 * k {
            lo_r.bottom() + 4.0 * k
        } else {
            rect.top() + 3.0 * k
        };
        canvas::putf(
            b,
            pos2(xh - grab_w, hi_y),
            Align2::RIGHT_TOP,
            Role::Value,
            ink.hue,
            tk,
            plate,
            format_args!("{}", Hz(hi)),
        );
        if let Some(pt) = hover {
            // The frequency under the pointer, beside the pointer.
            canvas::putf(
                b,
                pos2(pt.x + 4.0 * k, rect.bottom() - 30.0 * k),
                Align2::LEFT_BOTTOM,
                Role::Value,
                ink.text,
                tk,
                Some(super::dynfx::on_display(&ink, s.t.deck_title)),
                format_args!("{}", Hz(geo.spectrum_hz(pt.x))),
            );
        }
    });

    // Drag: an edge, or the middle.
    if resp.drag_started() {
        // Where the button went down decides what is held.
        if let Some(pt) = ui.input(|i| i.pointer.press_origin()) {
            st.drag = if (pt.x - xl).abs() < grab_w {
                Some(Grab::Lo)
            } else if (pt.x - xh).abs() < grab_w {
                Some(Grab::Hi)
            } else if pt.x > xl && pt.x < xh {
                Some(Grab::Band {
                    lo,
                    hi,
                    at_hz: geo.spectrum_hz(pt.x),
                })
            } else {
                None
            };
        }
    }
    let sixth = 2f32.powf(1.0 / 6.0);
    if let (Some(g), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let hz = geo.spectrum_hz(pt.x);
            match g {
                Grab::Lo => set_band(host, Some(hz.clamp(ds::MIN_HZ, hi / sixth)), None),
                Grab::Hi => set_band(host, None, Some(hz.clamp(lo * sixth, ds::MAX_HZ))),
                Grab::Band { lo, hi, at_hz } => {
                    let r = (hz / at_hz).clamp(ds::MIN_HZ / lo, ds::MAX_HZ / hi);
                    set_band(host, Some(lo * r), Some(hi * r));
                }
                Grab::Threshold => {}
            }
        }
    }
    if resp.drag_stopped() && st.drag.is_some() {
        st.drag = None;
        host.end_gesture();
    }
    if resp.double_clicked() {
        set_band(host, Some(5_000.0), Some(12_000.0));
        host.end_gesture();
    }
    if near_x(xl) || near_x(xh) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("ds.card.band"),
            frame::tr("ds.band_tip"),
            Caps::NONE,
        );
    }
}

// ---- the cards --------------------------------------------------------------------------

/// A segmented control on a parameter, its help line saying what the chosen word does.
fn choice(
    ui: &mut egui::Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &'static str,
    labels: &[&'static str],
    tips: &[&'static str],
    w: f32,
) {
    let srcs: [Src<'_>; 4] =
        std::array::from_fn(|i| Src::Key(labels.get(i).copied().unwrap_or("")));
    let r = ui.scope(|ui| controls::segmented_param(ui, s, host, key, &srcs[..labels.len()], w));
    let now = (host.spec().get(key).round().max(0.0) as usize).min(tips.len() - 1);
    let name = host
        .spec()
        .kind
        .param(key)
        .map_or("", |i| frame::tr(i.label_key));
    tip(
        ui,
        r.response.contains_pointer(),
        name,
        frame::tr(tips[now]),
    );
}

/// How it detects and what it turns down: the mode, the processing, the stereo.
fn detection_card(host: &mut dyn FxHost, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    let _ = controls::card(
        ui,
        s,
        rect,
        Src::Key("ds.card.detection"),
        None,
        |ui, cr| {
            let w = cr.body.width() / s.k;
            controls::column(ui, s, w, |ui| {
                choice(ui, s, host, "mode", &MODE_KEYS, &MODE_TIPS, w);
                choice(ui, s, host, "proc", &PROC_KEYS, &PROC_TIPS, w);
                choice(ui, s, host, "stereo", &STEREO_KEYS, &STEREO_TIPS, w);
            });
        },
    );
}

/// What the detector hears (the key), what you hear, and Learn.
fn key_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    dss: &DeessSettings,
) {
    let lang = crate::i18n::locale();
    let rev = host.revision();
    if st.words.key.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.words.key = Some((rev, host.key_source()));
    }
    if st.words.key_for != Some((rev, lang)) {
        st.words.key_text = match st.words.key.as_ref().and_then(|(_, k)| k.as_ref()) {
            Some(KeySource::Strips {
                current: Some(_),
                label,
                ..
            }) => label.clone(),
            _ => t("ds.key_own"),
        };
        st.words.key_for = Some((rev, lang));
    }
    // Learn's words: what it will do, or how many sibilants it has heard.
    let voice = dss.detection == Detection::Voice;
    let heard = if st.learning {
        let mut n = 0;
        runs(st.learn_cols.iter(), voice, |_| n += 1);
        n
    } else {
        0
    };
    if st.words.learn != Some((st.learning, heard, lang)) {
        st.words.learn_text = if st.learning {
            tf("ds.learning", &[("n", &heard)])
        } else {
            t("ds.learn")
        };
        st.words.learn = Some((st.learning, heard, lang));
    }
    let _ = controls::card(ui, s, rect, Src::Key("ds.card.key"), None, |ui, cr| {
        let w = cr.body.width() / s.k;
        controls::column(ui, s, w, |ui| {
            key_source(host, st, ui, s, w);
            choice(ui, s, host, "listen", &LISTEN_KEYS, &LISTEN_TIPS, w);
            // A key, not a choice: it starts listening, and a second press sets the line.
            let r = controls::key(
                ui,
                s,
                Src::Str(&st.words.learn_text),
                frame::tr("ds.learn"),
                w,
                st.learning,
            );
            if r.clicked() {
                learn(host, st);
            }
            tip(
                ui,
                r.hovered(),
                frame::tr("ds.learn"),
                frame::tr("ds.learn_tip"),
            );
        });
    });
}

/// Where the detector's key comes from: any strip of the desk (Cantus), or the plug-in's
/// sidechain input, switched on (another DAW routes it).
fn key_source(host: &mut dyn FxHost, st: &State, ui: &mut egui::Ui, s: &Style, w: f32) {
    let words = &st.words;
    match words.key.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips {
            current, options, ..
        }) => {
            let current = *current;
            let id = ui.id().with("ds-key");
            let r =
                ui.scope(|ui| controls::field(ui, s, id, &words.key_text, frame::tr("ds.key"), w));
            if let Some(at) = r.inner {
                let mut items = vec![frame::radio(Src::Key("ds.key_own"), current.is_none())];
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
                frame::tr("ds.key"),
                frame::tr("ds.key_tip"),
            );
        }
        Some(KeySource::Switch(on)) => {
            let labels = [
                Src::Key("ds.key_own"),
                Src::Key("export.sidechain.external"),
            ];
            let r = ui.scope(|ui| {
                controls::segmented(ui, s, *on as usize, &labels, frame::tr("ds.key"), w)
            });
            if let Some(i) = r.inner {
                host.set_key_source(KeyTarget::Switch(i == 1));
            }
            tip(
                ui,
                r.response.contains_pointer(),
                frame::tr("ds.key"),
                frame::tr("ds.key_tip"),
            );
        }
        None => {}
    }
}

fn db_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{} dB", Signed(v));
}

fn range_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.1} dB");
}

fn look_pill(v: f32, b: &mut String) {
    if v < 0.5 {
        b.push_str(frame::tr("ds.off"));
    } else {
        let _ = write!(b, "{v:.0} ms");
    }
}

/// How a cell writes its value into its pill, when not in the parameter's own unit.
type Pill<'a> = Option<&'a dyn Fn(f32, &mut String)>;

/// One cell on a parameter; the band's edges keep their order, as the display does.
fn ds_cell(
    ui: &mut egui::Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &'static str,
    size: Size,
    format: Pill<'_>,
) {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let mut c = Cell::param(info, spec.get(key)).size(size);
    if let Some(f) = format {
        c = c.format(f);
    }
    let out = c.show_in(ui, s, host, key);
    if let Some(mut v) = out.changed {
        if matches!(key, "lo" | "hi") {
            let other = host.spec().get(if key == "lo" { "hi" } else { "lo" });
            let sixth = 2f32.powf(1.0 / 6.0);
            v = if key == "lo" {
                v.min(other / sixth)
            } else {
                v.max(other * sixth)
            };
        }
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
}

/// The cells: the threshold (the primary parameter, larger) and the range; the band's
/// edges; attack, release and lookahead; the stereo link; the classifier's sensitivity.
fn cells_card(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    dss: &DeessSettings,
) {
    let voice = dss.detection == Detection::Voice;
    let _ = controls::card(ui, s, rect, Src::Key("ds.card.cells"), None, |ui, _| {
        ds_cell(ui, s, host, "thr", DS_CELLS[0], Some(&db_pill));
        ds_cell(ui, s, host, "range", Size::M, Some(&range_pill));
        ds_cell(ui, s, host, "lo", Size::M, None);
        ds_cell(ui, s, host, "hi", Size::M, None);
        ds_cell(ui, s, host, "att", Size::M, None);
        ds_cell(ui, s, host, "rel", Size::M, None);
        ds_cell(ui, s, host, "look", Size::M, Some(&look_pill));
        ds_cell(ui, s, host, "link", Size::M, None);
        ui.add_enabled_ui(voice, |ui| {
            ds_cell(ui, s, host, "sens", Size::M, None);
        });
    });
}

// ---- the inspector's picture ----------------------------------------------------------

/// The inspector's picture: the band on a frequency axis and the settings in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::DeEsser));
    let ink = Ink::of(&st);
    let s = tele::settings(spec);
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), space::GRID * 14.0),
        Sense::click(),
    );
    let axis = Rect::from_min_max(
        pos2(rect.left() + space::S2, rect.bottom() - space::S6),
        pos2(rect.right() - space::S2, rect.bottom() - space::S2),
    );
    let x = |hz: f32| axis.left() + (hz / 1_000.0).ln() / (20.0f32).ln() * axis.width();
    let (lo, hi) = ds::band_edges(s.lo_hz, s.hi_hz, DEFAULT_SR);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        let px = b.px();
        b.hline(
            axis.left(),
            axis.right(),
            b.snap_mid(axis.bottom()),
            px,
            ink.grid_strong,
        );
        let band = Rect::from_min_max(pos2(x(lo), axis.top()), pos2(x(hi), axis.bottom()));
        b.rrect(band, 1.0, Fill::Solid(Ink::a(ink.hue, 0.35)));
        b.rrect_stroke(band, 1.0, px, ink.hue);
        for hz in [1_000.0f32, 5_000.0, 10_000.0, 20_000.0] {
            b.vline(
                b.snap_mid(x(hz)),
                axis.bottom() - 3.0,
                axis.bottom(),
                px,
                ink.dim,
            );
        }
        let words = Rect::from_min_max(
            pos2(rect.left() + space::S2, rect.top() + space::S1),
            pos2(rect.right() - space::S2, axis.top() - space::S1),
        );
        let (l1, l2) = (
            Rect::from_min_max(words.min, pos2(words.right(), words.center().y)),
            Rect::from_min_max(pos2(words.left(), words.center().y), words.max),
        );
        canvas::line_in(
            b,
            l1,
            Role::Label,
            ink.text,
            st.tk(),
            format_args!(
                "{:+.0} dB \u{b7} {:.0} dB \u{b7} {} \u{b7} {}",
                s.threshold_db,
                s.range_db,
                frame::tr(MODE_KEYS[s.detection.index().min(1)]),
                frame::tr(PROC_KEYS[s.processing.index().min(2)]),
            ),
        );
        canvas::line_in(
            b,
            l2,
            Role::Label,
            ink.hue,
            st.tk(),
            format_args!("{}\u{2013}{}", Hz(lo), Hz(hi)),
        );
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("ds.open_tip"), "", Caps::NONE);
    }
    resp
}

// ---- demo -----------------------------------------------------------------------------

/// The settings `--editor deesser` and the screenshot tool open it with.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    for (key, v) in [
        ("thr", -38.0),
        ("range", 6.0),
        ("proc", 2.0),
        ("lo", 4_500.0),
        ("hi", 11_000.0),
        ("look", 5.0),
    ] {
        app.set_fx_param(strip, effect, key, v, false);
    }
}

/// A few seconds of something like speech: voiced syllables with formants, and "s", "sh"
/// and "t" between them — the material the demo runs the real DSP over.
pub fn demo_voice(sr: f32) -> Vec<f32> {
    use cantus_dsp::aequalis::{design, Coefs, EqShape, MAX_SECTIONS};
    #[derive(Clone, Copy)]
    enum Seg {
        Vowel(f32, f32, f32),
        Hiss(f32, f32, f32, f32),
        Gap(f32),
    }
    use Seg::*;
    // (seconds, …): vowel (len, f0, level), hiss (len, lo, hi, level), gap (len).
    let script = [
        Gap(0.15),
        Vowel(0.24, 180.0, 0.5),
        Hiss(0.14, 5_000.0, 11_000.0, 0.28),
        Vowel(0.30, 200.0, 0.55),
        Gap(0.08),
        Hiss(0.03, 2_000.0, 12_000.0, 0.3),
        Vowel(0.22, 170.0, 0.45),
        Hiss(0.16, 2_500.0, 6_000.0, 0.2),
        Vowel(0.20, 190.0, 0.5),
        Hiss(0.18, 5_500.0, 12_000.0, 0.35),
        Gap(0.12),
        Vowel(0.34, 210.0, 0.6),
        Hiss(0.10, 5_000.0, 10_000.0, 0.16),
        Vowel(0.26, 160.0, 0.5),
        Gap(0.16),
        Vowel(0.20, 220.0, 0.55),
        Hiss(0.20, 4_800.0, 11_000.0, 0.4),
        Vowel(0.30, 185.0, 0.5),
        Gap(0.10),
        Hiss(0.12, 5_200.0, 11_500.0, 0.25),
        Vowel(0.28, 175.0, 0.5),
        Hiss(0.03, 2_000.0, 12_000.0, 0.25),
        Vowel(0.22, 195.0, 0.45),
        Gap(0.14),
        Vowel(0.26, 205.0, 0.55),
        Hiss(0.15, 5_000.0, 11_000.0, 0.32),
    ];
    let mut seed = 0x5eed_u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let tdf = |c: &Coefs, z: &mut [f32; 2], x: f32| {
        let y = c.b0 * x + z[0];
        z[0] = c.b1 * x - c.a1 * y + z[1];
        z[1] = c.b2 * x - c.a2 * y;
        y
    };
    let mut out = Vec::new();
    for seg in script {
        match seg {
            Gap(len) => out.extend(std::iter::repeat_n(0.0, (len * sr) as usize)),
            Vowel(len, f0, level) => {
                let n = (len * sr) as usize;
                let formants = [
                    (700.0, 90.0, 1.0),
                    (1_200.0, 110.0, 0.5),
                    (2_700.0, 160.0, 0.3),
                    (3_600.0, 250.0, 0.15),
                ];
                let harmonics: Vec<(f32, f32)> = (1..)
                    .map(|h| h as f32 * f0)
                    .take_while(|f| *f < 12_000.0)
                    .map(|f| {
                        let w: f32 = formants
                            .iter()
                            .map(|(fc, bw, g)| g / (1.0 + ((f - fc) / bw).powi(2)))
                            .sum::<f32>()
                            + 0.004;
                        (f, w)
                    })
                    .collect();
                let norm: f32 = harmonics.iter().map(|(_, w)| w).sum();
                for i in 0..n {
                    let tt = i as f32 / sr;
                    // A syllable swells and fades, with a little movement inside it.
                    let shape = (std::f32::consts::PI * tt / len).sin().max(0.0).powf(0.5);
                    let wobble = 1.0 + 0.18 * (std::f32::consts::TAU * 5.5 * tt + f0).sin();
                    let env = shape * wobble * (tt / 0.02).min(1.0);
                    let vib = 1.0 + 0.01 * (std::f32::consts::TAU * 5.0 * tt).sin();
                    let x: f32 = harmonics
                        .iter()
                        .map(|(f, w)| w * (std::f32::consts::TAU * f * vib * tt).sin())
                        .sum();
                    out.push(level * env * x / norm * 2.5 + 0.002 * noise());
                }
            }
            Hiss(len, lo, hi, level) => {
                let n = (len * sr) as usize;
                let mut c = [Coefs::IDENTITY; MAX_SECTIONS];
                let mut chain: Vec<(Coefs, [f32; 2])> = Vec::new();
                for (shape, hz) in [(EqShape::LowCut, lo), (EqShape::HighCut, hi)] {
                    let k = design(
                        shape,
                        sr,
                        hz,
                        0.0,
                        std::f32::consts::FRAC_1_SQRT_2,
                        24.0,
                        &mut c,
                    );
                    chain.extend(c[..k].iter().map(|co| (*co, [0.0; 2])));
                }
                for i in 0..n {
                    let tt = i as f32 / sr;
                    let env = (tt / 0.012).min(1.0) * ((len - tt) / 0.02).clamp(0.0, 1.0);
                    let mut x = noise();
                    for (co, z) in chain.iter_mut() {
                        x = tdf(co, z, x);
                    }
                    out.push(level * env * x * 2.0);
                }
            }
        }
    }
    out
}

/// What a playing engine would feed the editor, for screenshots with no engine: the real
/// DSP run over [`demo_voice`] with the editor's settings.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(spec) = app.project().effect(strip, effect).map(|e| e.spec.clone()) else {
        return;
    };
    let sr = DEFAULT_SR;
    let voice = demo_voice(sr);
    let mut d = Sibilus::new(sr);
    d.set(&tele::settings(&spec));
    let st = &mut app.fx.deesser;
    *st = State {
        effect: Some(effect),
        ..State::default()
    };
    let chunk = d.chunk();
    let mut pairs = Vec::with_capacity(2 * super::PAIRS);
    let keep_from = voice.len().saturating_sub(super::PAIRS);
    let mut seen = 0;
    for (k, block) in voice.chunks(chunk).enumerate() {
        let (mut l, mut r) = (block.to_vec(), block.to_vec());
        d.process(&mut l, &mut r, None);
        for (i, (a, b)) in block.iter().zip(&l).enumerate() {
            if k * chunk + i >= keep_from {
                pairs.push(*a);
                pairs.push(*b);
            }
        }
        while seen < d.seq() {
            if let Some(c) = d.column(seen) {
                st.push(c);
            }
            seen += 1;
        }
    }
    st.last_seq = Some(d.seq());
    let tel = tele::encode(&d);
    app.fx.injected = Some(tel);
    app.fx.feed(&pairs);
}
