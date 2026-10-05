//! The Aequalis editor: the EQ in its device frame (ADR 0024 §8), where the display *is*
//! the control.
//!
//! What the research on the benchmark EQ said users value, and what this does about it
//! (`research/fabfilter/pro-q.md`, "How we beat it"; `docs/plugins/eq.md` has the matrix):
//!
//! - **The display is the instrument**, and takes most of the window. Double-click to make
//!   a band — its shape follows where you clicked (cuts at the edges, shelves near them, a
//!   notch at the floor, a bell anywhere else) — or press on the curve and pull. Drag a dot
//!   (every selected dot moves with it, relatively), drag a rectangle to select several,
//!   the wheel for Q (slope on a cut; `Ctrl` for gain, `Alt` for the dynamic range),
//!   `Shift` for fine, `Alt` to lock the axis the drag started on, `Alt`+click to bypass,
//!   double-click a dot to type exact values (`2k`, `A4`, `C#2+13`, `2x`), right-click for
//!   the band's menu.
//! - **The axes are controls too.** Drag the frequency axis up or down to zoom around the
//!   pointer, sideways to pan, double-click to reset; the dB range widens itself when a
//!   band is dragged past it; a piano replaces the axis on request, and then a drag moves in
//!   semitones.
//! - **State is visible at rest.** The processing mode, the key strip, EQ match, the piano
//!   and the range sit inside the display, top-left; the latency is the footer's chip; the
//!   selected band's numbers are on a plate beside its dot and in its card under the
//!   display, with its dynamics, the output and the analyser in cards beside it.
//! - **Dynamics in real units** (threshold, range, ratio, attack and release in dB and ms),
//!   in both directions, from the band or from the key strip, with the last seconds of the
//!   band's movement drawn against its threshold — which you drag.
//! - **The host shows what a plug-in cannot**: the key strip's spectrum over this one, where
//!   they collide (hatched), and an EQ match to it in one click, fitted into editable bands.
//! - **Keyboard**: 1–9 and 0 select bands, Tab steps, arrows nudge (semitones and dB),
//!   `[`/`]` change Q (or the slope), Delete removes, B bypasses, S auditions, D makes a band
//!   dynamic, Enter types values, Esc backs out.
//! - **Every gesture is one step of the host's undo**, the multi-band drag included.
//!
//! Everything goes through the editor's [`FxHost`], so the same editor runs in Cantus (the
//! project's undo, the desk's strips as keys, the engine's meters) and in the exported
//! plug-in's window. The curve is drawn from the same design the audio thread runs
//! (`cantus_dsp::aequalis`), in the mode it runs; the spectra come from the analyser tap on
//! this effect's input and output, and the key's from the unit's telemetry.

#[path = "eq_editor_display.rs"]
mod display;
#[path = "eq_editor_panel.rs"]
mod panel;
#[path = "eq_editor_parse.rs"]
pub mod parse;

use crate::app::CantusApp;
use crate::i18n::{t, tf, Locale};
use crate::kit::text::Src;
use crate::kit::{Function, Style};
use crate::views::fx::canvas::{self, Ink};
use crate::views::fx::frame;
use crate::views::fx::host::{DawHost, FxHost, KeySource};
use cantus_core::fx::eq::EQ_BAND_FIELDS;
use cantus_core::{EffectId, EffectKind, EffectParamInfo, EffectSpec, StripId, EQ_BANDS};
use cantus_dsp::aequalis::fit::{Fit, FitOptions};
use cantus_dsp::aequalis::{self, latency_of, Mode, Resolution};
use cantus_dsp::{BandSettings, EqShape, Placement, SpectrumAnalyser};
use egui::{pos2, vec2, Color32, Pos2, Rect, Sense};
use std::collections::{BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::time::Instant;

/// Points across the display at which curves and spectra are evaluated.
const POINTS: usize = 256;
/// Columns of dynamics history kept: one a frame, about six seconds.
const HISTORY: usize = 360;
/// The dB ranges the display offers; a drag past the range steps up through them.
pub const RANGES: [f32; 5] = [3.0, 6.0, 12.0, 18.0, 30.0];
/// The frequency view the display opens on, and how far it can zoom out.
pub const VIEW: (f32, f32) = (20.0, 20_000.0);
const VIEW_LIMITS: (f32, f32) = (10.0, 24_000.0);

/// The row of cards under the display, at 100 %: tall enough for a column of four
/// switches beside the cells, short enough that the display keeps over 70 % of the body.
const ROW_H: f32 = 140.0;
/// The band card's width at 100 %: three cells and a column of switches.
const BAND_W: f32 = 340.0;
/// The output card's: two cells and a column.
const OUTPUT_W: f32 = 216.0;
/// The analyser card at its narrowest: its switches in two columns.
const ANALYSER_W_S: f32 = 212.0;
/// The dynamics card at its narrowest: five small cells and a little history.
const DYNAMICS_W_MIN: f32 = 370.0;

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
const PLACE_KEYS: [&str; 5] = [
    "eq.place.0",
    "eq.place.1",
    "eq.place.2",
    "eq.place.3",
    "eq.place.4",
];
const MODE_KEYS: [&str; 3] = ["eq.mode.0", "eq.mode.1", "eq.mode.2"];
const RES_KEYS: [&str; 4] = ["eq.res.0", "eq.res.1", "eq.res.2", "eq.res.3"];
const AN_RES_KEYS: [&str; 4] = ["eq.an.res.0", "eq.an.res.1", "eq.an.res.2", "eq.an.res.3"];
const SPEED_KEYS: [&str; 5] = [
    "eq.an.speed.0",
    "eq.an.speed.1",
    "eq.an.speed.2",
    "eq.an.speed.3",
    "eq.an.speed.4",
];
/// Band numbers as the dots print them.
const DIGITS: [&str; EQ_BANDS] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17",
    "18", "19", "20", "21", "22", "23", "24",
];

/// The parameter key of `field` of `band` (`b3.gain`), as the parameter table holds it:
/// a `&'static str`, so reading or writing a band never builds a string.
pub(crate) fn bkey(band: usize, field: &str) -> &'static str {
    binfo(band, field).key
}

/// The parameter description of `field` of `band`.
pub(crate) fn binfo(band: usize, field: &str) -> &'static EffectParamInfo {
    let fi = EQ_BAND_FIELDS
        .iter()
        .position(|f| f.0 == field)
        .unwrap_or(0);
    let all = cantus_core::fx::eq::params();
    &all[(band.min(EQ_BANDS - 1) * EQ_BAND_FIELDS.len() + fi).min(all.len() - 1)]
}

/// Whether band `band` exists (it may still be bypassed).
fn used(spec: &EffectSpec, band: usize) -> bool {
    spec.get(bkey(band, "used")) >= 0.5
}

/// The first band number not in use.
fn free_band(spec: &EffectSpec) -> Option<usize> {
    (0..EQ_BANDS).find(|b| !used(spec, *b))
}

/// Factory presets: a label key and the parameters it sets (`views::fx::presets_menu`).
/// Anything a preset does not name goes back to its default, so each is a complete EQ.
pub const FACTORY_PRESETS: &[crate::views::fx::FactoryPreset] = &[
    (
        "eq.preset.vocal",
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 90.0),
            ("b0.q", 0.71),
            ("b0.slope", 24.0),
            ("b1.used", 1.0),
            ("b1.freq", 280.0),
            ("b1.gain", -2.5),
            ("b1.q", 1.2),
            ("b2.used", 1.0),
            ("b2.freq", 3_200.0),
            ("b2.gain", 2.5),
            ("b2.q", 0.9),
            ("b3.used", 1.0),
            ("b3.shape", 2.0),
            ("b3.freq", 11_000.0),
            ("b3.gain", 2.0),
            ("b3.q", 0.71),
            ("b4.used", 1.0),
            ("b4.freq", 6_500.0),
            ("b4.gain", 0.001),
            ("b4.q", 3.0),
            ("b4.dyn", 1.0),
            ("b4.thr", -30.0),
            ("b4.range", -6.0),
            ("b4.att", 2.0),
            ("b4.rel", 80.0),
        ],
    ),
    (
        "eq.preset.kick",
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 30.0),
            ("b0.q", 0.71),
            ("b0.slope", 24.0),
            ("b1.used", 1.0),
            ("b1.freq", 62.0),
            ("b1.gain", 3.0),
            ("b1.q", 1.4),
            ("b2.used", 1.0),
            ("b2.freq", 350.0),
            ("b2.gain", -5.0),
            ("b2.q", 1.6),
            ("b2.gq", 1.0),
            ("b3.used", 1.0),
            ("b3.freq", 4_000.0),
            ("b3.gain", 3.5),
            ("b3.q", 1.0),
        ],
    ),
    (
        "eq.preset.demud",
        &[
            ("b0.used", 1.0),
            ("b0.freq", 300.0),
            ("b0.gain", 0.001),
            ("b0.q", 1.0),
            ("b0.dyn", 1.0),
            ("b0.thr", -28.0),
            ("b0.range", -6.0),
            ("b0.ratio", 3.0),
            ("b0.att", 15.0),
            ("b0.rel", 150.0),
        ],
    ),
    (
        "eq.preset.harsh",
        &[
            ("b0.used", 1.0),
            ("b0.freq", 3_400.0),
            ("b0.gain", 0.001),
            ("b0.q", 2.2),
            ("b0.dyn", 1.0),
            ("b0.thr", -30.0),
            ("b0.range", -5.0),
            ("b0.ratio", 4.0),
            ("b0.att", 3.0),
            ("b0.rel", 90.0),
        ],
    ),
    (
        "eq.preset.air",
        &[
            ("mode", 2.0),
            ("lp_res", 1.0),
            ("b0.used", 1.0),
            ("b0.shape", 2.0),
            ("b0.freq", 12_000.0),
            ("b0.gain", 3.0),
            ("b0.q", 0.6),
        ],
    ),
    (
        "eq.preset.telephone",
        &[
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 400.0),
            ("b0.q", 0.9),
            ("b0.slope", 24.0),
            ("b1.used", 1.0),
            ("b1.shape", 4.0),
            ("b1.freq", 3_200.0),
            ("b1.q", 0.9),
            ("b1.slope", 24.0),
            ("b2.used", 1.0),
            ("b2.freq", 1_500.0),
            ("b2.gain", 4.0),
            ("b2.q", 1.0),
        ],
    ),
    (
        "eq.preset.mono_bass",
        &[
            ("mode", 1.0),
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 120.0),
            ("b0.q", 0.71),
            ("b0.slope", 24.0),
            ("b0.place", 4.0),
            ("b1.used", 1.0),
            ("b1.shape", 2.0),
            ("b1.freq", 8_000.0),
            ("b1.gain", 2.0),
            ("b1.q", 0.71),
            ("b1.place", 4.0),
        ],
    ),
    (
        "eq.preset.master",
        &[
            ("mode", 2.0),
            ("lp_res", 2.0),
            ("b0.used", 1.0),
            ("b0.shape", 3.0),
            ("b0.freq", 25.0),
            ("b0.q", 0.71),
            ("b0.slope", 18.0),
            ("b1.used", 1.0),
            ("b1.shape", 7.0),
            ("b1.freq", 1_000.0),
            ("b1.gain", 1.0),
            ("b1.q", 0.5),
        ],
    ),
];

/// The analyser's settings. Kept by the editor, not by the project or its presets: the
/// analyser is how you look, not what the EQ does.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AnalyserView {
    /// The input, drawn as a fill.
    pub pre: bool,
    /// The output, drawn as a line.
    pub post: bool,
    /// The key strip's output (the sidechain), drawn dashed.
    pub key: bool,
    /// Hold the maximum instead of falling.
    pub freeze: bool,
    /// Label the spectrum's peaks, and let them be grabbed into bands.
    pub grab: bool,
    /// Hatch where the output and the key strip collide.
    pub collisions: bool,
    /// FFT size: 1024 · 2^n.
    pub resolution: usize,
    /// How fast the spectrum falls: 0 very fast … 4 very slow.
    pub speed: usize,
    /// The analyser's scale, dB below full scale.
    pub range_db: f32,
    /// dB per octave about 1 kHz.
    pub tilt: f32,
}

impl Default for AnalyserView {
    fn default() -> Self {
        AnalyserView {
            pre: true,
            post: true,
            key: true,
            freeze: false,
            grab: false,
            collisions: false,
            resolution: 2,
            speed: 2,
            range_db: 90.0,
            tilt: 4.5,
        }
    }
}

impl AnalyserView {
    pub const FALL: [f32; 5] = [120.0, 72.0, 36.0, 18.0, 9.0];
    pub const RANGES: [f32; 3] = [60.0, 90.0, 120.0];
    pub const TILTS: [f32; 4] = [0.0, 3.0, 4.5, 6.0];

    pub fn fft_size(&self) -> usize {
        1024 << self.resolution.min(3)
    }
}

/// Exact entry for a band: three boxes, Tab between them, Enter to set, Esc to leave.
#[derive(Clone, Debug)]
pub struct Typing {
    pub band: usize,
    pub fields: [String; 3],
    pub focus: usize,
    /// Focus the first box on the next frame.
    pub fresh: bool,
}

/// EQ match in progress: the input's and the key's long-term spectra, accumulated while
/// listening, and the options of the fit.
#[derive(Clone, Debug)]
pub struct MatchState {
    pub listening: bool,
    /// Summed power per third octave (`aequalis::key_band_hz`): input, key.
    pub input: [f64; aequalis::KEY_BANDS],
    pub reference: [f64; aequalis::KEY_BANDS],
    pub frames: usize,
    /// How much of the difference to apply, 0…1.
    pub amount: f32,
    pub budget: usize,
    pub max_boost: f32,
    pub max_cut: f32,
    /// The last fit, and what it was fitted from.
    pub fit: Option<Fit>,
    fitted_at: usize,
}

impl Default for MatchState {
    fn default() -> Self {
        MatchState {
            listening: true,
            input: [0.0; aequalis::KEY_BANDS],
            reference: [0.0; aequalis::KEY_BANDS],
            frames: 0,
            amount: 1.0,
            budget: 6,
            max_boost: 9.0,
            max_cut: 12.0,
            fit: None,
            fitted_at: usize::MAX,
        }
    }
}

impl MatchState {
    /// The difference the fit aims at — reference minus input, level-matched (its mean
    /// over 100 Hz–10 kHz is 0 dB, so a louder reference does not become a boost
    /// everywhere), smoothed over a third of an octave either side and scaled by the
    /// amount. `None` until both sides have been heard.
    pub fn target(&self) -> Option<Vec<(f32, f32)>> {
        if self.frames == 0 {
            return None;
        }
        let db = |p: f64| 10.0 * (p / self.frames as f64).max(1e-12).log10();
        let raw: Vec<(f32, f32)> = (0..aequalis::KEY_BANDS)
            .filter(|k| self.input[*k] > 0.0 && self.reference[*k] > 0.0)
            .map(|k| {
                (
                    aequalis::key_band_hz(k),
                    (db(self.reference[k]) - db(self.input[k])) as f32,
                )
            })
            .filter(|(_, d)| d.is_finite())
            .collect();
        if raw.len() < 4 {
            return None;
        }
        let mid: Vec<f32> = raw
            .iter()
            .filter(|(f, _)| (100.0..=10_000.0).contains(f))
            .map(|(_, d)| *d)
            .collect();
        let mean = if mid.is_empty() {
            0.0
        } else {
            mid.iter().sum::<f32>() / mid.len() as f32
        };
        let smooth: Vec<(f32, f32)> = (0..raw.len())
            .map(|i| {
                let lo = i.saturating_sub(1);
                let hi = (i + 1).min(raw.len() - 1);
                let s: f32 = raw[lo..=hi].iter().map(|(_, d)| d).sum();
                (raw[i].0, (s / (hi - lo + 1) as f32 - mean) * self.amount)
            })
            .collect();
        Some(smooth)
    }

    pub fn options(&self) -> FitOptions {
        FitOptions {
            max_bands: self.budget,
            max_boost_db: self.max_boost,
            max_cut_db: self.max_cut,
            ..FitOptions::default()
        }
    }

    /// Fit again when the options changed, and every half second of listening.
    fn refit(&mut self, sr: f32) {
        let stamp = self.frames / 30 * 1_000
            + self.budget * 7
            + (self.amount * 100.0) as usize * 131
            + (self.max_boost * 10.0) as usize * 17
            + (self.max_cut * 10.0) as usize * 19;
        if stamp != self.fitted_at {
            self.fitted_at = stamp;
            self.fit = self
                .target()
                .map(|target| aequalis::fit::fit(&target, &self.options(), sr));
        }
    }
}

/// A line of words formatted once per change of what it says (and of the language), so
/// an editor at rest formats nothing.
#[derive(Debug)]
struct Said<K> {
    key: Option<(K, Locale)>,
    text: String,
}

impl<K> Default for Said<K> {
    fn default() -> Self {
        Said {
            key: None,
            text: String::new(),
        }
    }
}

impl<K: PartialEq + Copy> Said<K> {
    fn get(&mut self, key: K, make: impl FnOnce(&mut String)) -> &str {
        let lang = crate::i18n::locale();
        if self.key != Some((key, lang)) {
            self.key = Some((key, lang));
            self.text.clear();
            make(&mut self.text);
        }
        &self.text
    }
}

/// The footer's chips.
#[derive(Debug, Default)]
struct Chips {
    /// (latency samples, sample rate).
    latency: Said<(usize, u32)>,
    /// Bands in use.
    bands: Said<usize>,
}

/// The words the cards and the display's controls print, kept until they change.
#[derive(Debug, Default)]
struct Words {
    /// "Band 5", "Band 5 +2": (band, others selected).
    title: Said<(usize, usize)>,
    /// "High · Medium · 90 dB · 4.5 dB/oct".
    analyser: Said<(usize, usize, u32, u32)>,
    /// "±18 dB".
    range: Said<u32>,
    /// The auto gain in force, "−8.7 dB" (tenths).
    auto: Said<i32>,
    /// "12.3 s heard" (tenths of a second).
    heard: Said<usize>,
    /// The fit's verdict: (bands, RMS and worst error in tenths of a dB).
    result: Said<(usize, i32, i32)>,
    /// "Match to Strip 2 · Kick", per host revision.
    match_title: Said<u64>,
}

/// What the editor remembers between frames. Cantus keeps one in `CantusApp::eq_editor`;
/// the exported plug-in's window keeps its own (`views::fx::Editor`). Making one allocates
/// nothing — the analyser and the caches grow on first use — so Cantus can take it out
/// of the app for the length of a frame.
#[derive(Debug)]
pub struct EqEditor {
    /// The EQ being edited, if the window is open (Cantus).
    pub open: Option<(StripId, EffectId)>,
    /// The bands as the host's parameters make them, and the host revision they were read
    /// at: read once per change, not by every part of the editor every frame.
    bands_at: Option<(u64, [cantus_dsp::BandSettings; aequalis::MAX_BANDS])>,
    /// The selected bands.
    pub selection: BTreeSet<usize>,
    /// The band whose card is shown: the last one clicked.
    pub primary: Option<usize>,
    /// The display's gain range, ± dB.
    pub range_db: f32,
    /// The display's frequency view, Hz.
    pub view: (f32, f32),
    /// A piano instead of the frequency axis; drags move in semitones.
    pub piano: bool,
    pub show: AnalyserView,
    analyser: Option<SpectrumAnalyser>,
    /// Interleaved pairs drained from the host, reused.
    scratch: Vec<f32>,
    /// The drawn spectra per display point, dB on the analyser scale (tilted).
    pre: [f32; POINTS],
    post: [f32; POINTS],
    key: [f32; POINTS],
    last: Option<Instant>,
    drag: Option<display::Drag>,
    rubber: Option<(Pos2, Pos2)>,
    axis: Option<display::AxisDrag>,
    pub typing: Option<Typing>,
    /// Per frame: each band's dynamic gain change and its detector's level, dB.
    history: VecDeque<([f32; EQ_BANDS], [f32; EQ_BANDS])>,
    /// Numbers each gesture, so a gesture is one undo step (the host merges what carries
    /// the same number) and the next is another.
    gesture: u64,
    nudge_at: Option<Instant>,
    pub matching: Option<MatchState>,
    /// Telemetry injected by demos and tests, read instead of the host's.
    pub injected: Option<[f32; 64]>,
    /// Each band's curves across the display, kept while nothing they depend on changes.
    curves: display::Curves,
    /// Scratch for what the display computes each frame.
    buf: display::Buf,
    /// When the last wheel turn landed: a burst within 500 ms is one step.
    burst_at: Option<Instant>,
    /// Where the plot was drawn last frame, and the view it was drawn with — so tests and
    /// tools can aim at what is on screen instead of mirroring the layout.
    pub plot: Option<Rect>,
    plot_view: (f32, f32),
    /// The open right-click menu.
    menu: Option<display::Menu>,
    /// The threshold being dragged on the dynamics history.
    hist_drag: bool,
    /// The key's source as the host said it, per host revision.
    key_source: Option<(u64, Option<KeySource>)>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: Chips,
    words: Words,
}

impl Default for EqEditor {
    fn default() -> Self {
        EqEditor {
            open: None,
            bands_at: None,
            selection: BTreeSet::new(),
            primary: None,
            range_db: 18.0,
            view: VIEW,
            piano: false,
            show: AnalyserView::default(),
            analyser: None,
            scratch: Vec::new(),
            pre: [-200.0; POINTS],
            post: [-200.0; POINTS],
            key: [-200.0; POINTS],
            last: None,
            drag: None,
            rubber: None,
            axis: None,
            typing: None,
            history: VecDeque::new(),
            gesture: 0,
            nudge_at: None,
            matching: None,
            injected: None,
            curves: display::Curves::default(),
            buf: display::Buf::default(),
            burst_at: None,
            plot: None,
            plot_view: VIEW,
            menu: None,
            hist_drag: false,
            key_source: None,
            chrome: frame::Chrome::default(),
            chips: Chips::default(),
            words: Words::default(),
        }
    }
}

/// A colour per band number, from the track palette: a band keeps its colour for life.
pub fn band_color(band: usize) -> Color32 {
    crate::theme::track_color(cantus_core::Color::PALETTE[band % cantus_core::Color::PALETTE.len()])
}

impl EqEditor {
    /// The analyser, made at its size the first time it is needed.
    fn analyser(&mut self) -> &mut SpectrumAnalyser {
        let n = self.show.fft_size();
        if self.analyser.as_ref().is_none_or(|a| a.size() != n) {
            self.analyser = Some(SpectrumAnalyser::new(n));
        }
        self.analyser
            .get_or_insert_with(|| SpectrumAnalyser::new(n))
    }

    fn has_data(&self) -> bool {
        self.analyser.as_ref().is_some_and(|a| a.has_data())
    }

    /// Feed the analyser (input, output) pairs directly — what the engine's tap does, for
    /// screenshots and tests with no engine.
    pub fn feed(&mut self, pairs: &[f32]) {
        self.analyser().push(pairs);
        self.buf.fresh = true;
    }

    /// Add one frame of dynamics history directly (demos and tests).
    pub fn feed_history(&mut self, deltas: [f32; EQ_BANDS], levels: [f32; EQ_BANDS]) {
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back((deltas, levels));
    }

    /// Where band `band`'s dot was drawn last frame.
    pub fn dot_pos(&self, spec: &EffectSpec, band: usize) -> Option<Pos2> {
        let rect = self.plot?;
        let bands = cantus_engine::eq_bands(spec);
        Some(display::Geo::new(rect, self.range_db, self.plot_view).dot(bands.get(band)?))
    }

    /// The point of the plot at `hz` and `db` (on the EQ's scale), as drawn last frame.
    pub fn plot_pos(&self, hz: f32, db: f32) -> Option<Pos2> {
        let rect = self.plot?;
        let g = display::Geo::new(rect, self.range_db, self.plot_view);
        Some(pos2(g.x(hz), g.y(db)))
    }

    /// Select exactly `bands`; the first becomes the one the card shows.
    pub fn select(&mut self, bands: &[usize]) {
        self.selection.clear();
        self.selection.extend(bands.iter().copied());
        self.primary = bands.first().copied();
    }

    /// A new gesture's number.
    fn next_gesture(&mut self) -> u64 {
        self.gesture += 1;
        self.gesture
    }

    /// The range that fits `gain`: the current one, or the next step up.
    fn range_for(&self, gain: f32) -> f32 {
        if gain.abs() <= self.range_db {
            return self.range_db;
        }
        RANGES
            .iter()
            .copied()
            .find(|r| *r >= gain.abs())
            .unwrap_or(30.0)
    }

    /// Forget selections, a typing box and a menu for bands that no longer exist (undo,
    /// presets, A/B).
    fn tidy(&mut self, spec: &EffectSpec) {
        self.selection.retain(|b| used(spec, *b));
        if self.primary.is_some_and(|p| !used(spec, p)) {
            self.primary = self.selection.iter().next().copied();
        }
        if self.typing.as_ref().is_some_and(|t| !used(spec, t.band)) {
            self.typing = None;
        }
    }

    /// What the host brought since the last frame: the analyser's pairs and one column of
    /// dynamics history.
    fn take_audio(&mut self, host: &mut dyn FxHost, tel: &[f32; 64]) {
        let mut pairs = std::mem::take(&mut self.scratch);
        pairs.clear();
        if host.drain_analyser(&mut pairs) {
            if !pairs.is_empty() {
                self.analyser().push(&pairs);
                self.buf.fresh = true;
            }
        } else {
            // A host that keeps the tap's recent pairs: the analyser's window is the last
            // of them, whatever came before — read again only when they moved on.
            let n = self.show.fft_size();
            let recent = host.pairs();
            let tail = recent
                .back()
                .map_or((0, 0, 0), |p| (recent.len(), p.0.to_bits(), p.1.to_bits()));
            if tail != self.buf.tail {
                self.buf.tail = tail;
                let skip = recent.len().saturating_sub(n);
                for (a, b) in recent.iter().skip(skip) {
                    pairs.push(*a);
                    pairs.push(*b);
                }
                if !pairs.is_empty() {
                    self.analyser().push(&pairs);
                    self.buf.fresh = true;
                }
            }
        }
        self.scratch = pairs;
        if self.injected.is_none() {
            if let Some(d) = host.eq_deltas() {
                let mut levels = [-200.0; EQ_BANDS];
                levels.copy_from_slice(
                    &tel[aequalis::telemetry::LEVEL..aequalis::telemetry::LEVEL + EQ_BANDS],
                );
                self.feed_history(d, levels);
            }
        }
    }
}

// ---- editing bands through the host ------------------------------------------------------
//
// Every change is one call of `FxHost::apply`: one undo step in Cantus, one gesture per
// parameter in another DAW. A gesture's number folds a drag (or a burst of wheel turns or
// key presses) into one step.

/// The fields a new band of `shape` is created with.
fn new_band_fields(shape: EqShape, freq: f32, gain: f32) -> ([(&'static str, f32); 6], usize) {
    let mut f = [("", 0.0); 6];
    let mut n = 0;
    let mut push = |k: &'static str, v: f32| {
        f[n] = (k, v);
        n += 1;
    };
    push("used", 1.0);
    push("shape", shape.index() as f32);
    push("freq", freq);
    if shape.has_gain() {
        push("gain", gain);
    }
    if matches!(
        shape,
        EqShape::LowShelf | EqShape::HighShelf | EqShape::TiltShelf
    ) || shape.has_slope()
    {
        push("q", 0.71);
    }
    if shape.has_slope() {
        push("slope", 24.0);
    }
    (f, n)
}

/// Set fields of one band as one step (or folded into gesture `gesture`).
fn set_band(host: &mut dyn FxHost, band: usize, fields: &[(&str, f32)], gesture: Option<u64>) {
    let edits: Vec<(&str, Option<f32>)> = fields
        .iter()
        .map(|(f, v)| (bkey(band, f), Some(*v)))
        .collect();
    host.apply(&edits, "cmd.eq_band", gesture);
}

/// Make a band at the first free number, with `extra` fields, and select it.
#[allow(clippy::too_many_arguments)]
fn add_band(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    shape: EqShape,
    freq: f32,
    gain: f32,
    extra: &[(&'static str, f32)],
    gesture: Option<u64>,
) -> Option<usize> {
    let band = free_band(host.spec())?;
    let (fields, n) = new_band_fields(shape, freq, gain);
    let mut edits: Vec<(&str, Option<f32>)> = Vec::with_capacity(n + extra.len());
    for (f, v) in fields[..n].iter().chain(extra) {
        let key = bkey(band, f);
        match edits.iter_mut().find(|e| e.0 == key) {
            Some(e) => e.1 = Some(*v),
            None => edits.push((key, Some(*v))),
        }
    }
    host.apply(&edits, "cmd.eq_band", gesture);
    st.select(&[band]);
    Some(band)
}

/// Remove bands: every field they stored goes, so their numbers are free and the file
/// forgets them. Removing the auditioned band ends the audition, in the same step.
fn remove_bands(host: &mut dyn FxHost, st: &mut EqEditor, bands: &[usize]) {
    let solo = host.spec().get("solo").round() as usize;
    let mut edits: Vec<(&str, Option<f32>)> = Vec::new();
    for &band in bands {
        edits.extend(EQ_BAND_FIELDS.iter().map(|f| (bkey(band, f.0), None)));
        if solo == band + 1 {
            edits.push(("solo", None));
        }
    }
    host.apply(&edits, "cmd.eq_band_remove", None);
    for b in bands {
        st.selection.remove(b);
    }
    if st.primary.is_some_and(|p| bands.contains(&p)) {
        st.primary = st.selection.iter().next().copied();
    }
}

/// Audition one band (the region it acts on, alone), or stop auditioning.
fn set_solo(host: &mut dyn FxHost, band: Option<usize>) {
    host.set("solo", band.map_or(0.0, |b| b as f32 + 1.0), None);
}

/// Split a band into a pair: the band itself on L (or M), a copy on R (or S). One step.
/// Returns the copy's number.
fn split(host: &mut dyn FxHost, st: &mut EqEditor, band: usize, mid_side: bool) -> Option<usize> {
    let spec = host.spec();
    let copy = free_band(spec)?;
    let (a, b) = if mid_side {
        (Placement::Mid, Placement::Side)
    } else {
        (Placement::Left, Placement::Right)
    };
    let mut edits: Vec<(&str, Option<f32>)> = vec![(bkey(band, "place"), Some(a.index() as f32))];
    for (field, ..) in EQ_BAND_FIELDS {
        if let Some(v) = spec.params.get(bkey(band, field)) {
            let v = if *field == "place" {
                b.index() as f32
            } else {
                *v
            };
            edits.push((bkey(copy, field), Some(v)));
        }
    }
    if !spec.params.contains_key(bkey(band, "place")) {
        edits.push((bkey(copy, "place"), Some(b.index() as f32)));
    }
    host.apply(&edits, "cmd.eq_split", None);
    st.select(&[band, copy]);
    Some(copy)
}

/// Put a fit's bands on the EQ, at free numbers, as one step.
fn apply_fit(host: &mut dyn FxHost, st: &mut EqEditor, fit: &Fit) -> Vec<usize> {
    let spec = host.spec();
    let mut free = (0..EQ_BANDS).filter(|b| !used(spec, *b));
    let mut edits: Vec<(&str, Option<f32>)> = Vec::new();
    let mut made = Vec::new();
    for b in &fit.bands {
        let Some(n) = free.next() else { break };
        let (fields, count) = new_band_fields(b.shape, b.freq, b.gain_db);
        for (f, v) in fields[..count].iter().filter(|(f, _)| *f != "q") {
            edits.push((bkey(n, f), Some(*v)));
        }
        edits.push((bkey(n, "q"), Some(b.q)));
        made.push(n);
    }
    host.apply(&edits, "cmd.eq_match", None);
    st.select(&made);
    made
}

/// Move the selected bands by `semitones` and `db`, as one step — or folded into a run of
/// key presses less than 600 ms apart.
fn nudge(host: &mut dyn FxHost, st: &mut EqEditor, semitones: f32, db: f32) {
    let now = Instant::now();
    let merge = st
        .nudge_at
        .is_some_and(|t| now.duration_since(t).as_millis() < 600);
    st.nudge_at = Some(now);
    if !merge {
        host.end_gesture();
        st.next_gesture();
    }
    let spec = host.spec();
    let mut edits: Vec<(&str, Option<f32>)> = Vec::new();
    let mut widest = 0.0f32;
    for &b in &st.selection {
        let f = spec.get(bkey(b, "freq"));
        let shape = EqShape::from_index(spec.get(bkey(b, "shape")) as usize);
        edits.push((
            bkey(b, "freq"),
            Some((f * 2f32.powf(semitones / 12.0)).clamp(10.0, 30_000.0)),
        ));
        if shape.has_gain() {
            let g = (spec.get(bkey(b, "gain")) + db).clamp(-30.0, 30.0);
            widest = widest.max(g.abs());
            edits.push((bkey(b, "gain"), Some(g)));
        }
    }
    st.range_db = st.range_for(widest);
    host.apply(&edits, "cmd.eq_band", Some(st.gesture));
}

/// Multiply the selected bands' Q by `factor` (cuts: step the slope instead).
fn scale_q(host: &mut dyn FxHost, st: &EqEditor, factor: f32) {
    let spec = host.spec();
    let mut edits: Vec<(&str, Option<f32>)> = Vec::new();
    for &b in &st.selection {
        let shape = EqShape::from_index(spec.get(bkey(b, "shape")) as usize);
        if shape.has_slope() {
            let s = spec.get(bkey(b, "slope"));
            let s = (s + if factor > 1.0 { 6.0 } else { -6.0 }).clamp(6.0, 96.0);
            edits.push((bkey(b, "slope"), Some(s)));
        } else {
            let q = (spec.get(bkey(b, "q")) * factor).clamp(0.025, 40.0);
            edits.push((bkey(b, "q"), Some(q)));
        }
    }
    host.apply(&edits, "cmd.eq_band", None);
}

/// Flip a switch-like field (`on`, `dyn`) on every selected band, all to the same new
/// value (the opposite of the primary band's).
fn toggle_selection(host: &mut dyn FxHost, st: &EqEditor, field: &str) {
    let Some(first) = st.primary.or(st.selection.first().copied()) else {
        return;
    };
    let to = if host.spec().get(bkey(first, field)) >= 0.5 {
        0.0
    } else {
        1.0
    };
    let edits: Vec<(&str, Option<f32>)> = st
        .selection
        .iter()
        .map(|b| (bkey(*b, field), Some(to)))
        .collect();
    host.apply(&edits, "cmd.eq_band", None);
}

/// Every selected band to the next placement (stereo → L → R → M → S).
fn cycle_placement(host: &mut dyn FxHost, st: &EqEditor) {
    let Some(first) = st.primary.or(st.selection.first().copied()) else {
        return;
    };
    let now = host.spec().get(bkey(first, "place")).round().max(0.0) as usize;
    let next = (now + 1) % Placement::ALL.len();
    let edits: Vec<(&str, Option<f32>)> = st
        .selection
        .iter()
        .map(|b| (bkey(*b, "place"), Some(next as f32)))
        .collect();
    host.apply(&edits, "cmd.eq_band", None);
}

impl CantusApp {
    /// Open an EQ's editor and point the analyser at it.
    pub fn open_eq_editor(&mut self, strip: StripId, effect: EffectId) {
        self.comp_editor.open = None;
        let ed = &mut self.eq_editor;
        ed.open = Some((strip, effect));
        ed.typing = None;
        ed.drag = None;
        ed.menu = None;
        ed.matching = None;
        ed.history.clear();
        ed.pre.fill(-200.0);
        ed.post.fill(-200.0);
        ed.key.fill(-200.0);
        // Open on the first band there is, so its numbers are on screen at once.
        let first = self
            .project()
            .effect(strip, effect)
            .and_then(|e| (0..EQ_BANDS).find(|b| used(&e.spec, *b)));
        self.eq_editor.select(first.as_slice());
        self.engine.set_analyser(Some(effect));
    }

    /// Close the editor. An audition in progress ends with it — a solo left on in a saved
    /// project would be a trap.
    pub fn close_eq_editor(&mut self) {
        if let Some((strip, effect)) = self.eq_editor.open.take() {
            let soloed = self
                .project()
                .effect(strip, effect)
                .is_some_and(|e| e.spec.get("solo") >= 1.0);
            if soloed {
                self.set_eq_solo(strip, effect, None);
            }
            self.engine.set_analyser(None);
        }
    }

    /// Move the analyser's samples into the open plug-in editor. Called once a frame.
    /// (The EQ's own analyser drains through its host, in [`editor`].)
    pub(crate) fn pump_analyser(&mut self) {
        if self.fx.open.is_some() {
            let mut pairs = std::mem::take(&mut self.eq_editor.scratch);
            pairs.clear();
            self.engine.drain_analyser(&mut pairs);
            self.pump_fx_pairs(&pairs);
            self.eq_editor.scratch = pairs;
        }
    }

    /// Run `f` on the EQ's editor state with the project as its host.
    fn with_eq<R>(
        &mut self,
        strip: StripId,
        effect: EffectId,
        f: impl FnOnce(&mut dyn FxHost, &mut EqEditor) -> R,
    ) -> R {
        let mut st = std::mem::take(&mut self.eq_editor);
        let r = f(&mut DawHost::new(self, strip, effect, "eq"), &mut st);
        self.eq_editor = st;
        r
    }

    /// Make a band at the first free number. Returns it.
    pub fn add_eq_band(
        &mut self,
        strip: StripId,
        effect: EffectId,
        shape: EqShape,
        freq: f32,
        gain: f32,
    ) -> Option<usize> {
        self.with_eq(strip, effect, |h, st| {
            add_band(h, st, shape, freq, gain, &[], None)
        })
    }

    /// Remove a band: every field it stored goes, so the number is free and the file
    /// forgets it. One undo step.
    pub fn remove_eq_band(&mut self, strip: StripId, effect: EffectId, band: usize) {
        self.remove_eq_bands(strip, effect, &[band]);
    }

    /// Remove several bands as one undo step.
    pub fn remove_eq_bands(&mut self, strip: StripId, effect: EffectId, bands: &[usize]) {
        self.with_eq(strip, effect, |h, st| remove_bands(h, st, bands));
    }

    /// Audition one band (the region it acts on, alone), or stop auditioning.
    pub fn set_eq_solo(&mut self, strip: StripId, effect: EffectId, band: Option<usize>) {
        self.with_eq(strip, effect, |h, _| set_solo(h, band));
    }

    /// Split a band into a pair: the band itself on L (or M), a copy on R (or S). One undo
    /// step. Returns the copy's number.
    pub fn split_eq_band(
        &mut self,
        strip: StripId,
        effect: EffectId,
        band: usize,
        mid_side: bool,
    ) -> Option<usize> {
        self.with_eq(strip, effect, |h, st| split(h, st, band, mid_side))
    }

    /// Put a fit's bands on the EQ, at free numbers, as one undo step.
    pub fn apply_eq_fit(&mut self, strip: StripId, effect: EffectId, fit: &Fit) -> Vec<usize> {
        self.with_eq(strip, effect, |h, st| apply_fit(h, st, fit))
    }
}

// ---- the window --------------------------------------------------------------------------

/// The editor window, drawn every frame while one is open: the device frame, where every
/// editor goes (`views::fx::open_window`).
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some((strip, effect)) = app.eq_editor.open else {
        return;
    };
    if app.project().effect(strip, effect).is_none() {
        app.close_eq_editor();
        return;
    }
    let mut pos = app.fx.pos;
    let open = crate::views::fx::open_window(
        ctx,
        egui::Id::new(("fx-editor", effect.0)),
        &mut pos,
        EffectKind::Eq,
        |ui| framed(app, ui, strip, effect),
    );
    app.fx.pos = pos;
    if !open {
        app.close_eq_editor();
    }
}

/// Aequalis in Cantus's own window: the host is the project (every gesture one undo step,
/// the key any strip of the desk, the engine's meters).
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.eq_editor);
    st.chrome.reset_for(effect.0);
    let out = editor(&mut DawHost::new(app, strip, effect, "eq"), &mut st, ui);
    app.eq_editor = st;
    out
}

/// Aequalis on any host (ADR 0020): the device frame, the display and the cards. Everything
/// it needs goes through [`FxHost`].
pub fn editor(host: &mut dyn FxHost, st: &mut EqEditor, ui: &mut egui::Ui) -> frame::Out {
    st.tidy(host.spec());
    let tel = st.injected.unwrap_or_else(|| host.telemetry());
    st.take_audio(host, &tel);
    let sr = host.sample_rate();
    let spec = host.spec();
    let mode = Mode::from_index(spec.get("mode").round() as usize);
    let res = Resolution::from_index(spec.get("lp_res").round() as usize);
    let latency = latency_of(mode, res, sr);
    let count = (0..EQ_BANDS).filter(|b| used(spec, *b)).count();
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [
        chips.latency.get((latency, sr.round() as u32), |s| {
            if latency == 0 {
                s.push_str(&t("eq.latency_none"));
            } else {
                s.push_str(&tf(
                    "eq.latency",
                    &[
                        ("ms", &format!("{:.1}", latency as f32 / sr * 1_000.0)),
                        ("n", &latency),
                    ],
                ));
            }
        }),
        chips.bands.get(count, |s| {
            s.push_str(&tf("eq.bands", &[("n", &count), ("max", &EQ_BANDS)]))
        }),
    ];
    let more = [
        frame::action(Src::Key("eq.select_all")),
        frame::action(Src::Key("eq.reset_view")),
    ];
    let look = frame::Look {
        kind: EffectKind::Eq,
        chips: &chip,
        meter: None,
        more: &more,
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, sr)
    });
    st.chrome = chrome;
    st.chips = chips;
    match out.more {
        Some(0) => {
            let spec = host.spec();
            st.selection = (0..EQ_BANDS).filter(|b| used(spec, *b)).collect();
            st.primary = st.selection.iter().next().copied();
        }
        Some(1) => st.view = VIEW,
        _ => {}
    }
    out
}

/// The body: the display over most of it, then one row of cards — the selected band's
/// (with its dynamics beside it when it has them, or the analyser's), and the output's at
/// the right. While EQ match is open its card takes the band cards' place.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut EqEditor,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    sr: f32,
) {
    let k = s.k;
    let mut g = crate::kit::shell::Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row_h = ROW_H * k;
    let disp = g.full((area.height() - row_h - gap).max(120.0 * k));
    let mut words = std::mem::take(&mut st.words);
    display::show(host, st, ui, s, disp, tel, sr, &mut words);
    let row = g.full(row_h);
    // The row, left to right: the band (or EQ match in its place), its dynamics when it
    // has them, the analyser, the output. The dynamics and match cards take what is left;
    // the analyser folds into two columns beside them, and, on a window too narrow for
    // four cards, gives way to the dynamics.
    let dynamic = st.matching.is_none()
        && st
            .primary
            .filter(|p| used(host.spec(), *p))
            .is_some_and(|p| {
                let spec = host.spec();
                spec.get(bkey(p, "dyn")) >= 0.5
                    && EqShape::from_index(spec.get(bkey(p, "shape")).round() as usize).has_gain()
            });
    let out_w = OUTPUT_W * k;
    let band_w = BAND_W * k;
    let an_min = ANALYSER_W_S * k;
    let fixed = if dynamic {
        band_w + out_w + an_min + DYNAMICS_W_MIN * k + 3.0 * gap
    } else {
        0.0
    };
    let show_analyser = !dynamic || fixed <= row.width() + 0.5;
    let span = move |x0: f32, x1: f32| {
        Rect::from_min_max(pos2(x0, row.top()), pos2(x1.max(x0), row.bottom()))
    };
    let right = Rect::from_min_max(pos2(row.right() - out_w, row.top()), row.max);
    if st.matching.is_some() {
        let an = Rect::from_min_max(
            pos2(right.left() - gap - an_min, row.top()),
            pos2(right.left() - gap, row.bottom()),
        );
        let m = Rect::from_min_max(row.min, pos2(an.left() - gap, row.bottom()));
        panel::match_card(host, st, &mut words, ui, s, m, tel, sr);
        panel::analyser_card(st, &mut words, ui, s, an, tel);
    } else {
        let left = span(row.left(), row.left() + band_w);
        panel::band_card(host, st, &mut words, ui, s, left);
        let end = right.left() - gap;
        match st.primary {
            Some(p) if dynamic => {
                let an_x = if show_analyser {
                    end - an_min
                } else {
                    end + gap
                };
                let d = span(left.right() + gap, an_x - gap);
                panel::dynamics_card(host, st, ui, s, d, p);
                if show_analyser {
                    panel::analyser_card(st, &mut words, ui, s, span(an_x, end), tel);
                }
            }
            _ => {
                let an = span(left.right() + gap, end);
                panel::analyser_card(st, &mut words, ui, s, an, tel);
            }
        }
    }
    panel::output_card(host, st, &mut words, ui, s, right, tel);
    st.words = words;
}

/// A small, read-only picture of an EQ's curve, for the inspector's effect card: the curve
/// as the mode realises it, the dots, and the mode's latency when it has one.
pub fn mini_curve(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    const N: usize = POINTS / 2;
    let s = Style::current().function(Function::Tone);
    let ink = Ink::of(&s);
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(width, crate::design::space::GRID * 14.0),
        Sense::click(),
    );
    let geo = display::Geo::new(rect, 18.0, VIEW);
    let bands = cantus_engine::eq_bands(spec);
    let mode = Mode::from_index(spec.get("mode").round() as usize);
    let sr = 48_000.0;
    let mut hz = [0.0f32; N];
    for (i, h) in hz.iter_mut().enumerate() {
        *h = geo.hz_at(i, N);
    }
    let mut total = [0.0f32; N];
    let mut one = [0.0f32; N];
    for b in bands.iter().filter(|b| b.active) {
        aequalis::band_curve_db(b, mode, sr, &hz, &mut one);
        for (t, o) in total.iter_mut().zip(&one) {
            *t += o;
        }
    }
    let mut pts = [Pos2::ZERO; N];
    for (i, p) in pts.iter_mut().enumerate() {
        *p = pos2(geo.x(hz[i]), geo.y(total[i]));
    }
    crate::kit::with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(geo.y(0.0)),
            b.px(),
            ink.grid_strong,
        );
        b.area(&pts, geo.y(0.0), ink.fill(0.6));
        b.polyline(&pts, 1.5, ink.hue);
        for (i, band) in bands.iter().enumerate() {
            if band.active && used(spec, i) {
                b.dot(geo.dot(band), 3.0, band_color(i));
            }
        }
        if mode != Mode::ZeroLatency {
            let res = Resolution::from_index(spec.get("lp_res").round() as usize);
            let lat = latency_of(mode, res, sr);
            canvas::putf(
                b,
                pos2(rect.right() - 6.0, rect.top() + 4.0),
                egui::Align2::RIGHT_TOP,
                crate::kit::text::Role::Micro,
                ink.dim,
                1.0,
                Some(ink.plate),
                format_args!(
                    "{} \u{b7} {:.1} ms",
                    frame::tr(MODE_KEYS[mode.index().min(2)]),
                    lat as f32 / sr * 1_000.0
                ),
            );
        }
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            frame::tr("eq.open_tip"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    resp
}

// ---- demo --------------------------------------------------------------------------------

/// `--editor eq` and the screenshots: on top of the four bands the app's demo makes, a
/// dynamic de-esser band, a side-only shelf, a key strip, and a two-band selection.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    app.with_eq(strip, effect, |h, st| {
        add_band(
            h,
            st,
            EqShape::Bell,
            6_200.0,
            0.001,
            &[
                ("q", 3.0),
                ("dyn", 1.0),
                ("thr", -34.0),
                ("range", -7.0),
                ("att", 2.0),
                ("rel", 90.0),
            ],
            None,
        );
    });
    app.with_eq(strip, effect, |h, st| {
        add_band(
            h,
            st,
            EqShape::HighShelf,
            14_000.0,
            -2.5,
            &[("place", 4.0)],
            None,
        );
    });
    let other = app
        .project()
        .tracks
        .iter()
        .filter_map(|t| t.strip)
        .find(|s| *s != strip);
    if let Some(key) = other {
        app.exec(cantus_core::Command::SetEffectSidechain {
            strip,
            effect,
            source: Some(key),
        });
    }
    app.eq_editor.show.collisions = true;
    app.set_fx_param(
        strip,
        effect,
        "mode",
        Mode::NaturalPhase.index() as f32,
        false,
    );
}

fn demo_noise(seed: &mut u32) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    (*seed as f32 / u32::MAX as f32) * 2.0 - 1.0
}

/// For screenshots and tests with no engine: run the real EQ over a mix-like signal and a
/// key strip's, and feed the editor what a playing engine would — the analyser's pairs,
/// the telemetry (the key's spectrum, the detectors) and a few seconds of dynamics history.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect)) = app.eq_editor.open else {
        return;
    };
    let Some(slot) = app.project().effect(strip, effect).cloned() else {
        return;
    };
    let sr = 48_000.0f32;
    let mut eq = cantus_dsp::Aequalis::new(sr);
    eq.set_options(aequalis::Options {
        mode: Mode::from_index(slot.spec.get("mode").round() as usize),
        ..aequalis::Options::default()
    });
    eq.set(&cantus_engine::eq_bands(&slot.spec), slot.spec.get("out"));
    let mut seed = 7u32;
    let mut pink = [0.0f32; 3];
    let block = 800;
    let frames = 300;
    let mut pairs = Vec::with_capacity(2 * block);
    for frame in 0..frames {
        let mut l = Vec::with_capacity(block);
        let mut k = Vec::with_capacity(block);
        for i in 0..block {
            let n = frame * block + i;
            let tt = n as f32 / sr;
            // A "vocal": a 220 Hz voice with harmonics and sibilant bursts every half
            // second; the "key": a bass line and its kick.
            let burst = if (n / 24_000) % 2 == 1 && (n % 24_000) < 6_000 {
                1.0
            } else {
                0.15
            };
            let w = demo_noise(&mut seed);
            pink[0] = 0.997 * pink[0] + 0.03 * w;
            pink[1] = 0.96 * pink[1] + 0.12 * w;
            pink[2] = 0.5 * pink[2] + 0.4 * w;
            let voice = (1..8)
                .map(|h| (std::f32::consts::TAU * 220.0 * h as f32 * tt).sin() * 0.25 / h as f32)
                .sum::<f32>();
            let hiss = burst * 0.25 * (w - pink[2]);
            l.push(voice + 0.3 * (pink[0] + pink[1]) + hiss);
            let bass = 0.45 * (std::f32::consts::TAU * 55.0 * tt).sin()
                + 0.2 * (std::f32::consts::TAU * 110.0 * tt).sin();
            // A guitar chord where the voice lives: the collision the view should show.
            let chord = [196.0f32, 247.0, 294.0]
                .iter()
                .flat_map(|f| (1..6).map(move |h| (f * h as f32, 0.09 / h as f32)))
                .map(|(f, a)| a * (std::f32::consts::TAU * f * tt).sin())
                .sum::<f32>();
            k.push(bass + chord + 0.15 * pink[0] + 0.08 * pink[1]);
        }
        let pre = l.clone();
        let mut r = l.clone();
        eq.process_keyed(&mut l, &mut r, Some((&k, &k)));
        let deltas = eq.dynamic_deltas();
        let mut d = [0.0; EQ_BANDS];
        d.copy_from_slice(&deltas[..EQ_BANDS]);
        app.eq_editor.feed_history(d, eq.detector_levels());
        if frame >= frames - 12 {
            pairs.clear();
            for (a, b) in pre.iter().zip(&l) {
                pairs.push(*a);
                pairs.push(*b);
            }
            app.eq_editor.feed(&pairs);
        }
    }
    app.eq_editor.injected = Some(eq.telemetry());
    // The dynamic band in its card, with the bell at 2.8 kHz selected beside it.
    let dynamic = (0..EQ_BANDS).find(|b| slot.spec.get(bkey(*b, "dyn")) >= 0.5);
    if let Some(d) = dynamic {
        app.eq_editor.select(&[d, 2]);
    }
}

/// The bands the host's parameters make, read once per host revision (a host that keeps
/// no revision, 0, is read every time).
pub(crate) fn bands_of(
    st: &mut EqEditor,
    host: &dyn FxHost,
) -> [cantus_dsp::BandSettings; aequalis::MAX_BANDS] {
    let rev = host.revision();
    match st.bands_at {
        Some((r, b)) if r == rev && rev != 0 => b,
        _ => {
            let b = cantus_engine::eq_bands(host.spec());
            st.bands_at = Some((rev, b));
            b
        }
    }
}
