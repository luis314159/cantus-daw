//! Multiplex's editor: the multiband dynamics, **display first** (ADR 0024 §8). Owned by
//! Multiplex; nothing else edits this file.
//!
//! What the research on the benchmark found its users asking for
//! (`research/fabfilter/pro-mb.md` §7 and §9), and what this does about it:
//!
//! - **The threshold is on the display, and it is a line you drag** — in the band's own
//!   column, on the level scale, against the band's detector level drawn live in the same
//!   column. The part of the level over the threshold is lit. The lower threshold is a
//!   dashed line in the same place. The benchmark's threshold exists only on a knob, and
//!   its gain dots are mistaken for thresholds; here the gain is a **dot** in a separate
//!   gain strip, the threshold a **line**, the range a **lens** — three shapes that cannot
//!   be confused.
//! - **Crossovers are dragged on the spectrum**, with their frequency written on the
//!   handle; a double-click splits the band under the pointer, and the new band starts as
//!   a copy of the one it came from, so a split alone changes nothing you hear.
//! - **Each band's gain change is drawn live and as history**: the curve in the device's
//!   hue is the response right now (from the same filters the audio runs), a ghost holds
//!   the deepest of the last seconds, and under it every band has a lane of its gain over
//!   time. Input and output meters sit beside the display — the benchmark has no input
//!   meter.
//! - **Real units, two sides**: attack and release in milliseconds, ratios as numbers
//!   (`4.0:1` squeezes, `1:2.0` stretches), a lower threshold for upward compression or
//!   expansion in the same band, and a transfer curve for the selected band whose
//!   thresholds and ratios are dragged directly.
//! - **The selected band's card is docked under the display** and never covers the
//!   analyser; its numbers are always on screen, its colour marks the card.
//! - **The key is chosen here** — any strip of the desk in Cantus, the plug-in's sidechain
//!   input elsewhere — and each band says whether it listens to it; each band's detector
//!   can hear its own range or a free one.
//!
//! The display takes most of the window; the mode, slope, what to hear and the key sit
//! inside it, top-left, and what is under the pointer top-right. The editor runs on any
//! [`FxHost`] (ADR 0020): Cantus's window, or the exported plug-in's. Every drag and knob
//! turn is one step of the host's undo.

use super::canvas::{self, Ink};
use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost, KeySource, KeyTarget};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::space;
use crate::i18n::{t, tf, Locale};
use crate::kit::band;
use crate::kit::color::{on_fill, safe_track, FillSize};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::Item;
use crate::kit::meter::{self, Levels};
use crate::kit::shell::Grid;
use crate::kit::text::{Role, Src, Text};
use crate::kit::{segmented, with_brush, Brush, Fill, Function, State as Look, Style};
use cantus_core::fx::multiband::{band_key, field_index, MB_BANDS, MB_BAND_FIELDS};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::multiplex::{
    gain_db as curve_gain_db, response_part_db, transfer_db, BandSettings, Layout, Mode, Placement,
    Settings, Slope,
};
use cantus_dsp::SpectrumAnalyser;
use cantus_engine::fx::multiband::{
    settings, T_GAIN, T_INPUT, T_LATENCY, T_LEVEL, T_OUTPUT, T_RATE, T_SPLIT,
};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense, Vec2};
use std::collections::VecDeque;
use std::fmt::{self, Display, Write as _};

const LO_HZ: f32 = 20.0;
const HI_HZ: f32 = 20_000.0;
/// Points across the display at which curves and spectra are evaluated.
const POINTS: usize = 200;
/// The level scale (spectrum, thresholds, detector levels), dBFS.
const LEVEL_LO: f32 = -72.0;
/// The gain scale, ± dB.
const GAIN_RANGE: f32 = 15.0;
/// How fast the drawn spectrum falls, dB per second.
const SPECTRUM_FALL: f32 = 36.0;
/// Frames of history kept: at one a frame, about five seconds.
const HISTORY: usize = 300;
/// Frames the ghost of the deepest gain change holds: about three seconds.
const GHOST: usize = 180;
/// Meter fall, dB per second (the house ballistics).
const METER_FALL: f32 = 24.0;
/// The narrowest band a drag or a split will make, in octaves.
const MIN_OCT: f32 = 1.0 / 3.0;
/// A gain dot's radius at 100 %.
const DOT_R: f32 = 7.0;

/// The band card's height at 100 %: its title, a row of switches, a row of M cells.
const BAND_CARD: f32 = band::TITLE + 8.0 + controls::ROW + 6.0 + 86.0 + 8.0;
/// The display's head, where its selectors sit, at 100 %.
const HEAD: f32 = 30.0;
/// The widths of the transfer and output cards at 100 %.
const TRANSFER_W: f32 = 150.0;
const OUTPUT_W: f32 = 80.0;

/// A band's cells, in their groups: the upper section, the lower one, time and gain.
const CELLS: [&str; 11] = [
    "thr", "ratio", "range", "thr_lo", "ratio_lo", "range_lo", "knee", "att", "rel", "la", "gain",
];
/// …and, with a free key range, where it runs.
const KEY_CELLS: [&str; 2] = ["key_lo", "key_hi"];
/// Where a new group of cells starts (a divider before it).
const GROUPS: [usize; 3] = [3, 6, 11];

const MODE_LABELS: [Src<'static>; 3] = [
    Src::Key("mb.mode.0"),
    Src::Key("mb.mode.1"),
    Src::Key("mb.mode.2"),
];
const MODE_TIPS: [&str; 3] = ["mb.mode.0_tip", "mb.mode.1_tip", "mb.mode.2_tip"];
const SLOPE_LABELS: [Src<'static>; 4] = [
    Src::Str("6"),
    Src::Str("12"),
    Src::Str("24"),
    Src::Str("48"),
];
const SLOPE_TIPS: [&str; 4] = [
    "mb.slope_tip",
    "mb.slope_tip",
    "mb.slope_tip",
    "mb.slope_tip",
];
const LISTEN_LABELS: [Src<'static>; 3] = [
    Src::Key("mb.listen.0"),
    Src::Key("mb.listen.1"),
    Src::Key("mb.listen.2"),
];
const LISTEN_TIPS: [&str; 3] = ["mb.listen.0_tip", "mb.listen.1_tip", "mb.listen.2_tip"];
const PLACE_LABELS: [Src<'static>; 3] = [
    Src::Key("mb.place.0"),
    Src::Key("mb.place.1"),
    Src::Key("mb.place.2"),
];
const PLACE_TIPS: [&str; 3] = ["mb.place_tip", "mb.place_tip", "mb.place_tip"];
const KEY_LABELS: [Src<'static>; 2] = [Src::Key("mb.key_own"), Src::Key("mb.key_ext")];
const KEY_TIPS: [&str; 2] = ["mb.key_band_tip", "mb.key_band_tip"];

/// Factory presets: a label key and the parameters it sets.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "mb.preset.glue",
        &[
            ("b0.used", 1.0),
            ("b0.from", 20.0),
            ("b0.thr", -26.0),
            ("b0.ratio", 1.8),
            ("b0.range", 6.0),
            ("b0.knee", 12.0),
            ("b0.att", 30.0),
            ("b0.rel", 250.0),
            ("b1.used", 1.0),
            ("b1.from", 120.0),
            ("b1.thr", -28.0),
            ("b1.ratio", 1.5),
            ("b1.range", 6.0),
            ("b1.knee", 12.0),
            ("b1.att", 25.0),
            ("b1.rel", 200.0),
            ("b2.used", 1.0),
            ("b2.from", 2_000.0),
            ("b2.thr", -30.0),
            ("b2.ratio", 1.5),
            ("b2.range", 6.0),
            ("b2.knee", 12.0),
            ("b2.att", 15.0),
            ("b2.rel", 150.0),
            ("b3.used", 1.0),
            ("b3.from", 8_000.0),
            ("b3.thr", -34.0),
            ("b3.ratio", 1.5),
            ("b3.range", 4.0),
            ("b3.knee", 12.0),
            ("b3.att", 10.0),
            ("b3.rel", 120.0),
        ],
    ),
    (
        "mb.preset.deharsh",
        &[
            ("b0.used", 1.0),
            ("b0.from", 20.0),
            ("b0.ratio", 1.0),
            ("b1.used", 1.0),
            ("b1.from", 2_500.0),
            ("b1.thr", -32.0),
            ("b1.ratio", 3.0),
            ("b1.range", 8.0),
            ("b1.knee", 6.0),
            ("b1.att", 2.0),
            ("b1.rel", 80.0),
            ("b2.used", 1.0),
            ("b2.from", 6_500.0),
            ("b2.ratio", 1.0),
        ],
    ),
    (
        "mb.preset.low_end",
        &[
            ("mode", 1.0),
            ("b0.used", 1.0),
            ("b0.from", 20.0),
            ("b0.thr", -24.0),
            ("b0.ratio", 4.0),
            ("b0.range", 8.0),
            ("b0.att", 12.0),
            ("b0.rel", 140.0),
            ("b0.la", 2.0),
            ("b1.used", 1.0),
            ("b1.from", 90.0),
            ("b1.thr", -26.0),
            ("b1.ratio", 3.0),
            ("b1.range", 6.0),
            ("b1.att", 8.0),
            ("b1.rel", 120.0),
            ("b1.la", 2.0),
            ("b2.used", 1.0),
            ("b2.from", 280.0),
            ("b2.ratio", 1.0),
        ],
    ),
    (
        "mb.preset.up_down",
        &[
            ("b0.used", 1.0),
            ("b0.from", 20.0),
            ("b0.thr", -22.0),
            ("b0.ratio", 4.0),
            ("b0.thr_lo", -42.0),
            ("b0.ratio_lo", 3.0),
            ("b0.range_lo", 12.0),
            ("b0.att", 3.0),
            ("b0.rel", 120.0),
            ("b1.used", 1.0),
            ("b1.from", 160.0),
            ("b1.thr", -24.0),
            ("b1.ratio", 4.0),
            ("b1.thr_lo", -44.0),
            ("b1.ratio_lo", 3.0),
            ("b1.range_lo", 12.0),
            ("b1.att", 2.0),
            ("b1.rel", 100.0),
            ("b2.used", 1.0),
            ("b2.from", 2_500.0),
            ("b2.thr", -26.0),
            ("b2.ratio", 4.0),
            ("b2.thr_lo", -46.0),
            ("b2.ratio_lo", 3.0),
            ("b2.range_lo", 12.0),
            ("b2.att", 1.0),
            ("b2.rel", 80.0),
        ],
    ),
    (
        "mb.preset.vocal",
        &[
            ("b0.used", 1.0),
            ("b0.from", 20.0),
            ("b0.ratio", 1.0),
            ("b0.gain", -2.0),
            ("b1.used", 1.0),
            ("b1.from", 200.0),
            ("b1.thr", -24.0),
            ("b1.ratio", 2.0),
            ("b1.range", 8.0),
            ("b1.att", 8.0),
            ("b1.rel", 120.0),
            ("b2.used", 1.0),
            ("b2.from", 4_500.0),
            ("b2.thr", -32.0),
            ("b2.ratio", 5.0),
            ("b2.range", 10.0),
            ("b2.att", 0.5),
            ("b2.rel", 60.0),
            ("b2.la", 1.0),
            ("b3.used", 1.0),
            ("b3.from", 10_000.0),
            ("b3.ratio", 1.0),
            ("b3.thr_lo", -48.0),
            ("b3.ratio_lo", 2.0),
            ("b3.range_lo", 6.0),
        ],
    ),
    (
        "mb.preset.punch",
        &[
            ("b0.used", 1.0),
            ("b0.from", 20.0),
            ("b0.thr", -20.0),
            ("b0.ratio", 0.7),
            ("b0.range", 6.0),
            ("b0.att", 5.0),
            ("b0.rel", 90.0),
            ("b1.used", 1.0),
            ("b1.from", 180.0),
            ("b1.ratio", 1.0),
            ("b2.used", 1.0),
            ("b2.from", 3_000.0),
            ("b2.thr", -26.0),
            ("b2.ratio", 0.75),
            ("b2.range", 4.0),
            ("b2.att", 2.0),
            ("b2.rel", 60.0),
        ],
    ),
];

/// What the editor remembers between frames.
#[derive(Debug)]
pub struct State {
    pub selected: Option<usize>,
    /// Made when first needed, so a state that is only moved around allocates nothing.
    analyser: Option<SpectrumAnalyser>,
    scratch: Vec<f32>,
    /// The spectrum as drawn, per point (with its fall): input, output. Sized when first
    /// needed.
    pre: Vec<f32>,
    post: Vec<f32>,
    last: Option<std::time::Instant>,
    /// Per frame: each band's dynamic gain, dB, by band number.
    history: VecDeque<[f32; MB_BANDS]>,
    /// Meter levels as drawn (dB, with the house fall): in L, in R, out L, out R.
    meters: [f32; 4],
    drag: Option<Drag>,
    /// What a drag on the transfer curve holds.
    transfer: Option<TGrab>,
    /// The display's geometry in the last frame, for hit-testing and for tests.
    pub geo: Option<Geo>,
    /// The effect the history and the meters belong to.
    effect: Option<EffectId>,
    /// The device frame: the preset, its lists.
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    words: Words,
    /// The key's source as the host said it, per host revision.
    key: Option<(u64, Option<KeySource>)>,
    /// The band's menu, opened with a right-click on the display.
    menu: Option<Menu>,
    /// The curves, evaluated into buffers kept between frames.
    curve: [Pos2; POINTS],
    side: [Pos2; POINTS],
    ghost: [Pos2; POINTS],
    /// What the tap held when it was last analysed (length, newest, oldest).
    analysed: Option<Pair3>,
}

/// A cheap signature of the tap's contents.
type Pair3 = (usize, Option<(u32, u32)>, Option<(u32, u32)>);

impl Default for State {
    fn default() -> Self {
        State {
            selected: None,
            analyser: None,
            scratch: Vec::new(),
            pre: Vec::new(),
            post: Vec::new(),
            last: None,
            history: VecDeque::new(),
            meters: [-120.0; 4],
            drag: None,
            transfer: None,
            geo: None,
            effect: None,
            chrome: frame::Chrome::default(),
            chips: frame::ChipText::default(),
            words: Words::default(),
            key: None,
            menu: None,
            curve: [Pos2::ZERO; POINTS],
            side: [Pos2::ZERO; POINTS],
            ghost: [Pos2::ZERO; POINTS],
            analysed: None,
        }
    }
}

impl State {
    /// Add one frame of history directly — what the engine's telemetry does each frame,
    /// for screenshots and tests with no engine.
    pub fn feed_history(&mut self, gains_db: [f32; MB_BANDS]) {
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(gains_db);
    }

    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Another effect's editor: its own history, meters and spectrum.
    fn start_for(&mut self, effect: EffectId) {
        if self.effect != Some(effect) {
            self.effect = Some(effect);
            self.history.clear();
            self.meters = [-120.0; 4];
            self.pre.iter_mut().for_each(|v| *v = -120.0);
            self.post.iter_mut().for_each(|v| *v = -120.0);
            self.drag = None;
            self.transfer = None;
            self.menu = None;
            self.key = None;
            self.geo = None;
        }
        self.chrome.reset_for(effect.0);
    }
}

/// The words the editor formats, kept until the language or what they count changes.
#[derive(Debug, Default)]
struct Words {
    lang: Option<Locale>,
    /// "Band 3", per band number.
    band: Vec<String>,
    /// "2 bands", "3 bands", "4 bands": the empty display's quick starts.
    quick: Vec<String>,
    /// "4 of 8 bands", for a count.
    count: Option<(usize, String)>,
    /// "2 bands listen to it", for a count.
    keyed: Option<(usize, String)>,
}

impl Words {
    fn refresh(&mut self) {
        let lang = crate::i18n::locale();
        if self.lang == Some(lang) {
            return;
        }
        self.lang = Some(lang);
        self.band = (0..MB_BANDS)
            .map(|b| tf("mb.band_n", &[("n", &(b + 1))]))
            .collect();
        self.quick = [2usize, 3, 4]
            .iter()
            .map(|n| tf("mb.quick", &[("n", n)]))
            .collect();
        self.count = None;
        self.keyed = None;
    }

    /// Format the counts the display prints, when they changed.
    fn prepare(&mut self, bands: usize, keyed: usize) {
        self.refresh();
        if self.count.as_ref().is_none_or(|c| c.0 != bands) {
            self.count = Some((bands, tf("mb.bands", &[("n", &bands), ("max", &MB_BANDS)])));
        }
        if self.keyed.as_ref().is_none_or(|c| c.0 != keyed) {
            self.keyed = Some((keyed, tf("mb.key_bands", &[("n", &keyed)])));
        }
    }
}

/// The band menu: which band, where it opened, the frequency under the pointer then, and
/// its "Split at …" row.
#[derive(Clone, Debug)]
struct Menu {
    band: usize,
    at: Pos2,
    hz: Option<f32>,
    split: String,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Drag {
    /// The crossover under band `band` (its `from`).
    Crossover {
        band: usize,
    },
    Threshold {
        band: usize,
        lower: bool,
    },
    Gain {
        band: usize,
        origin_db: f32,
        origin_y: f32,
    },
    /// One end of a band's free key range.
    Key {
        band: usize,
        lo: bool,
    },
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    Threshold(bool),
    Gain,
    Key(bool),
}

/// What a drag on the transfer curve holds.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum TGrab {
    Upper,
    Lower,
    UpperRatio,
    LowerRatio,
}

/// The display's geometry: the level pane, the gain pane, the history lanes and the meters.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geo {
    pub level: Rect,
    pub gain: Rect,
    pub lanes: Rect,
    pub meters: Rect,
}

impl Geo {
    pub fn x(&self, hz: f32) -> f32 {
        let f = (hz.max(1.0) / LO_HZ).ln() / (HI_HZ / LO_HZ).ln();
        self.level.left() + f * self.level.width()
    }
    pub fn hz(&self, x: f32) -> f32 {
        let f = (x - self.level.left()) / self.level.width().max(1.0);
        LO_HZ * (HI_HZ / LO_HZ).powf(f)
    }
    pub fn y_level(&self, db: f32) -> f32 {
        let f = ((db - LEVEL_LO) / -LEVEL_LO).clamp(0.0, 1.0);
        self.level.bottom() - f * self.level.height()
    }
    pub fn level_at(&self, y: f32) -> f32 {
        LEVEL_LO + (self.level.bottom() - y) / self.level.height().max(1.0) * -LEVEL_LO
    }
    pub fn y_gain(&self, db: f32) -> f32 {
        self.gain.center().y - (db / GAIN_RANGE).clamp(-1.05, 1.05) * self.gain.height() * 0.5
    }
    pub fn gain_at(&self, y: f32) -> f32 {
        (self.gain.center().y - y) / (self.gain.height() * 0.5).max(1.0) * GAIN_RANGE
    }
    /// The x span of a band on the display.
    pub fn span(&self, lay: &Layout, band: usize) -> Option<(f32, f32)> {
        let (lo, hi) = lay.edges(band)?;
        Some((
            lo.map(|f| self.x(f)).unwrap_or(self.level.left()),
            hi.map(|f| self.x(f)).unwrap_or(self.level.right()),
        ))
    }
}

/// A colour per band number, from the track palette: a band keeps its colour for life.
pub fn band_color(band: usize) -> Color32 {
    let palette = cantus_core::Color::PALETTE;
    // Blue, green, purple, red first: far apart in hue, and none of them the hue of
    // the response curve.
    const ORDER: [usize; 8] = [1, 3, 2, 4, 7, 6, 0, 5];
    crate::theme::track_color(palette[ORDER[band % 8] % palette.len()])
}

/// A band's colour as the display draws it: clamped to the theme's safe band, so it
/// reads on the display's ground in every theme.
fn band_ink(band: usize, s: &Style) -> Color32 {
    safe_track(band_color(band), s.t.dark, FillSize::Small)
}

/// The static key of one field of one band (`b3.thr`): no formatting.
fn key(band: usize, field: &str) -> &'static str {
    band_key(band, field_index(field).unwrap_or(0))
}

/// `1.2 kHz`, `350 Hz`, as the knobs print them.
struct Hz(f32);

impl Display for Hz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 >= 1_000.0 {
            write!(f, "{:.1} kHz", self.0 / 1_000.0)
        } else {
            write!(f, "{:.0} Hz", self.0)
        }
    }
}

/// `4.0:1` squeezes, `1:2.0` stretches, `∞:1` limits.
struct Ratio(f32);

impl Display for Ratio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let r = self.0;
        if r >= 99.0 {
            f.write_str("\u{221e}:1")
        } else if r >= 1.0 {
            write!(f, "{r:.1}:1")
        } else {
            write!(f, "1:{:.1}", 1.0 / r.max(0.01))
        }
    }
}

/// `4.0:1` squeezes, `1:2.0` stretches, `∞:1` limits.
pub fn ratio_label(r: f32) -> String {
    Ratio(r).to_string()
}

/// The nearest note to a frequency, and how far off it is: `A4`, `E7 +12`.
struct Note(f32);

impl Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const NAMES: [&str; 12] = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let midi = 69.0 + 12.0 * (self.0.max(1.0) / 440.0).log2();
        let n = midi.round().clamp(0.0, 127.0);
        let cents = ((midi - n) * 100.0).round() as i32;
        let n = n as i32;
        write!(f, "{}{}", NAMES[(n % 12) as usize], n / 12 - 1)?;
        if cents != 0 {
            write!(f, " {cents:+}")?;
        }
        Ok(())
    }
}

/// A band's states as short words: `off · S · M · mid · key`.
struct Flags<'a>(&'a BandSettings);

impl Flags<'_> {
    fn items(&self) -> [Option<&'static str>; 5] {
        let b = self.0;
        [
            (!b.on).then_some("mb.flag_off"),
            b.solo.then_some("mb.flag_solo"),
            b.mute.then_some("mb.flag_mute"),
            match b.placement {
                Placement::Mid => Some("mb.flag_mid"),
                Placement::Side => Some("mb.flag_side"),
                Placement::Stereo => None,
            },
            b.key_ext.then_some("mb.flag_key"),
        ]
    }

    fn any(&self) -> bool {
        self.items().iter().any(Option::is_some)
    }
}

impl Display for Flags<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for k in self.items().into_iter().flatten() {
            if !first {
                f.write_str(" \u{b7} ")?;
            }
            first = false;
            f.write_str(frame::tr(k))?;
        }
        Ok(())
    }
}

/// The band's geometric centre on the display, Hz.
fn centre_hz(lay: &Layout, band: usize) -> f32 {
    let (lo, hi) = lay.edges(band).unwrap_or((None, None));
    let lo = lo.unwrap_or(LO_HZ).max(LO_HZ);
    let hi = hi.unwrap_or(HI_HZ).min(HI_HZ);
    (lo * hi).sqrt()
}

/// How far a band can cut and boost, dB: its potential range, drawn as a lens.
fn reach(b: &BandSettings) -> (f32, f32) {
    let mut cut = 0.0;
    let mut boost = 0.0;
    if b.ratio > 1.0 {
        cut += b.range_db;
    } else if b.ratio < 1.0 {
        boost += b.range_db;
    }
    if b.ratio_lo > 1.0 {
        boost += b.range_lo_db;
    } else if b.ratio_lo < 1.0 {
        cut += b.range_lo_db;
    }
    (cut, boost)
}

fn point_hz(i: usize) -> f32 {
    LO_HZ * (HI_HZ / LO_HZ).powf(i as f32 / (POINTS - 1) as f32)
}

/// A polyline drawn dashed: `dash` on, `gap` off, along its length.
pub(crate) fn dashed_poly(
    b: &mut Brush,
    pts: &[Pos2],
    w: f32,
    color: Color32,
    dash: f32,
    gap: f32,
) {
    let mut on = true;
    let mut left = dash;
    for win in pts.windows(2) {
        let (mut a, c) = (win[0], win[1]);
        let mut len = (c - a).length();
        if len <= 0.0 {
            continue;
        }
        let dir = (c - a) / len;
        while len > 1e-4 {
            let step = left.min(len);
            let e = a + dir * step;
            if on {
                b.segment(a, e, w, color, false);
            }
            a = e;
            len -= step;
            left -= step;
            if left <= 1e-4 {
                on = !on;
                left = if on { dash } else { gap };
            }
        }
    }
}

/// A choice among a few words, each with what it does on the help line.
#[allow(clippy::too_many_arguments)]
pub(crate) fn choice(
    ui: &mut egui::Ui,
    s: &Style,
    selected: usize,
    labels: &[Src<'static>],
    tips: &[&'static str],
    name: &str,
    width: f32,
) -> Option<usize> {
    choice_resp(ui, s, selected, labels, tips, name, width).0
}

/// [`choice`] that is the stepped parameter `key`'s control (the caller sets the value the
/// segment means): its menu, its mapped mark, the learn mode (round 29, review 2 of ADR
/// 0062, #44). No choice while the learn mode has it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn choice_key(
    ui: &mut egui::Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &str,
    selected: usize,
    labels: &[Src<'static>],
    tips: &[&'static str],
    name: &str,
    width: f32,
) -> Option<usize> {
    let (out, resp) = choice_resp(ui, s, selected, labels, tips, name, width);
    let learnt = controls::stepped_offer(ui, s, host, key, name, &resp);
    out.filter(|_| !learnt)
}

#[allow(clippy::too_many_arguments)]
fn choice_resp(
    ui: &mut egui::Ui,
    s: &Style,
    selected: usize,
    labels: &[Src<'static>],
    tips: &[&'static str],
    name: &str,
    width: f32,
) -> (Option<usize>, egui::Response) {
    let (rect, resp) =
        ui.allocate_exact_size(vec2(width * s.k, s.d(controls::ROW)), Sense::click());
    let hovered = resp
        .hover_pos()
        .and_then(|p| segmented::hit(rect, labels.len(), p.x));
    let out = if resp.clicked() {
        hovered.filter(|i| *i != selected)
    } else {
        None
    };
    let st = Look::of(&resp, false);
    if let Some(i) = hovered {
        // The segment's word (the control's name for a number) and what it does.
        let tip = tips.get(i).map_or("", |k| frame::tr(k));
        match labels[i] {
            Src::Key(k) => hint::set_text(ui.ctx(), frame::tr(k), tip, Caps::NONE),
            Src::Str(_) => hint::set_text(ui.ctx(), name, tip, Caps::NONE),
        }
    }
    if ui.is_rect_visible(rect) {
        with_brush(ui.painter(), |b| {
            segmented::paint(b, rect, s, st, labels, selected, hovered)
        });
    }
    (out, resp)
}

// ---- commands ---------------------------------------------------------------------

/// Split the band under `hz` there. The new band takes the part above `hz` and starts as
/// a copy of the band it split, so the split alone changes nothing but the detector. On
/// an empty Multiplex this makes the first two bands. One step of the host's undo.
/// Returns the new band's number.
pub fn split_at(host: &mut dyn FxHost, st: &mut State, hz: f32) -> Option<usize> {
    let spec = host.spec();
    let s = settings(spec);
    let lay = Layout::of(&s.bands);
    let hz = hz.clamp(LO_HZ * 2f32.powf(MIN_OCT), HI_HZ);
    let mut edits: Vec<(&str, Option<f32>)> = Vec::with_capacity(MB_BAND_FIELDS.len() + 4);
    let new = if lay.n == 0 {
        edits.extend([
            (key(0, "used"), Some(1.0)),
            (key(0, "from"), Some(LO_HZ)),
            (key(1, "used"), Some(1.0)),
            (key(1, "from"), Some(hz)),
        ]);
        1
    } else {
        let new = (0..MB_BANDS).find(|b| !s.bands[*b].used)?;
        // The band under `hz`, and room either side of the split.
        let pos = (0..lay.n).find(|p| {
            let (lo, hi) = lay.edges(lay.order[*p]).unwrap_or((None, None));
            lo.is_none_or(|lo| hz >= lo) && hi.is_none_or(|hi| hz < hi)
        })?;
        let old = lay.order[pos];
        let (lo, hi) = lay.edges(old).unwrap_or((None, None));
        let room = |a: f32, b: f32| (b / a).log2() >= MIN_OCT;
        if lo.is_some_and(|lo| !room(lo, hz)) || hi.is_some_and(|hi| !room(hz, hi)) {
            return None;
        }
        edits.push((key(new, "used"), Some(1.0)));
        edits.push((key(new, "from"), Some(hz)));
        for (fi, field) in MB_BAND_FIELDS.iter().enumerate() {
            if matches!(field.0, "used" | "from" | "solo" | "mute") {
                continue;
            }
            if let Some(v) = spec.params.get(band_key(old, fi)) {
                edits.push((band_key(new, fi), Some(*v)));
            }
        }
        new
    };
    host.apply(&edits, "cmd.mb_band_add", None);
    st.selected = Some(new);
    Some(new)
}

/// Remove a band: every field it stored goes, so the number is free, and its range goes
/// to its neighbour. One step.
pub fn remove_band(host: &mut dyn FxHost, st: &mut State, band: usize) {
    let edits: Vec<(&str, Option<f32>)> = (0..MB_BAND_FIELDS.len())
        .map(|fi| (band_key(band, fi), None))
        .collect();
    host.apply(&edits, "cmd.mb_band_remove", None);
    if st.selected == Some(band) {
        st.selected = None;
    }
}

/// Start from `n` evenly musical bands (2, 3 or 4). One step.
pub fn quick_bands(host: &mut dyn FxHost, st: &mut State, n: usize) {
    let froms: &[f32] = match n {
        2 => &[LO_HZ, 250.0],
        3 => &[LO_HZ, 200.0, 2_500.0],
        _ => &[LO_HZ, 120.0, 1_000.0, 6_000.0],
    };
    let edits: Vec<(&str, Option<f32>)> = froms
        .iter()
        .enumerate()
        .flat_map(|(b, from)| [(key(b, "used"), Some(1.0)), (key(b, "from"), Some(*from))])
        .collect();
    host.apply(&edits, "cmd.mb_band_add", None);
    st.selected = Some(0);
}

/// Flip one of a band's switches (`on`, `solo`, `mute`, `key_free`): one step.
fn flip(host: &mut dyn FxHost, band: usize, field: &str, on: bool) {
    host.set(key(band, field), if on { 0.0 } else { 1.0 }, None);
}

impl CantusApp {
    /// [`split_at`] on a strip's Multiplex, from anywhere in Cantus.
    pub fn mb_split_at(&mut self, strip: StripId, effect: EffectId, hz: f32) -> Option<usize> {
        self.project().effect(strip, effect)?;
        let mut st = std::mem::take(&mut self.fx.multiband);
        let r = split_at(
            &mut DawHost::new(self, strip, effect, "multiband"),
            &mut st,
            hz,
        );
        self.fx.multiband = st;
        r
    }

    /// [`remove_band`] on a strip's Multiplex.
    pub fn mb_remove_band(&mut self, strip: StripId, effect: EffectId, band: usize) {
        let mut st = std::mem::take(&mut self.fx.multiband);
        remove_band(
            &mut DawHost::new(self, strip, effect, "multiband"),
            &mut st,
            band,
        );
        self.fx.multiband = st;
    }

    /// [`quick_bands`] on a strip's Multiplex.
    pub fn mb_quick_bands(&mut self, strip: StripId, effect: EffectId, n: usize) {
        let mut st = std::mem::take(&mut self.fx.multiband);
        quick_bands(
            &mut DawHost::new(self, strip, effect, "multiband"),
            &mut st,
            n,
        );
        self.fx.multiband = st;
    }
}

// ---- the editor -----------------------------------------------------------------------

/// Multiplex's editor in Cantus's own window: the host is the project (every drag one
/// undo step, the key any strip of the desk).
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.multiband);
    st.start_for(effect);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "multiband"),
        &mut st,
        ui,
    );
    app.fx.multiband = st;
    out
}

/// Multiplex's editor on any host (ADR 0020): the device frame and the bands inside it.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let set = settings(host.spec());
    let lay = Layout::of(&set.bands);
    let tel = host.telemetry();
    // The selection follows the bands: never a band that is gone.
    if st.selected.is_none_or(|b| !set.bands[b].used) {
        st.selected = (lay.n > 0).then(|| lay.order[0]);
    }
    let sr = if tel[T_RATE] > 0.0 {
        tel[T_RATE]
    } else {
        host.sample_rate()
    };
    ingest(host, st, &tel, sr);
    st.words.refresh();
    let mut chips = std::mem::take(&mut st.chips);
    // The latency, shown whether or not it is zero: a delay that appears silently is the
    // complaint the research heard most.
    let chip = [chips.latency(tel[T_LATENCY], sr)];
    let out_db = [st.meters[2], st.meters[3]];
    let look = frame::Look {
        kind: EffectKind::Multiband,
        chips: &chip,
        meter: Some(Levels {
            level: out_db,
            hold: out_db,
        }),
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &set, &lay, &tel, sr)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// What came in since the last frame: a column of gain history, the spectrum (with its
/// fall) from the analyser tap's latest pairs, and the meters (instant rise, steady fall).
fn ingest(host: &dyn FxHost, st: &mut State, tel: &cantus_engine::Telemetry, sr: f32) {
    let mut gains = [0.0f32; MB_BANDS];
    gains.copy_from_slice(&tel[T_GAIN..T_GAIN + MB_BANDS]);
    st.feed_history(gains);
    let now = std::time::Instant::now();
    let dt = st
        .last
        .map(|l| now.duration_since(l).as_secs_f32())
        .unwrap_or(0.0)
        .min(0.25);
    st.last = Some(now);
    if st.pre.len() != POINTS {
        st.pre.resize(POINTS, -120.0);
        st.post.resize(POINTS, -120.0);
    }
    let an = st
        .analyser
        .get_or_insert_with(|| SpectrumAnalyser::new(4096));
    let pairs = host.pairs();
    let n = pairs.len().min(an.size());
    // Only when the tap brought something new: the analysis is the costliest thing the
    // editor does, and an editor at rest must cost next to nothing. (An unchanged tap
    // would give the same levels, which the drawn spectrum already holds.)
    let sig = (
        pairs.len(),
        pairs.back().map(|p| (p.0.to_bits(), p.1.to_bits())),
        pairs.front().map(|p| (p.0.to_bits(), p.1.to_bits())),
    );
    if n == an.size() && st.analysed != Some(sig) {
        st.analysed = Some(sig);
        st.scratch.clear();
        for (a, b) in pairs.iter().skip(pairs.len() - n) {
            st.scratch.push(*a);
            st.scratch.push(*b);
        }
        an.push(&st.scratch);
        an.compute();
        let fall = SPECTRUM_FALL * dt;
        for i in 0..POINTS {
            let hz = point_hz(i);
            let a = an.level_db(0, sr, hz, 1.0 / 6.0);
            let b = an.level_db(1, sr, hz, 1.0 / 6.0);
            st.pre[i] = a.max(st.pre[i] - fall);
            st.post[i] = b.max(st.post[i] - fall);
        }
    }
    for (i, v) in [
        tel[T_INPUT],
        tel[T_INPUT + 1],
        tel[T_OUTPUT],
        tel[T_OUTPUT + 1],
    ]
    .into_iter()
    .enumerate()
    {
        let db = if v > 1e-6 { 20.0 * v.log10() } else { -120.0 };
        st.meters[i] = db.max(st.meters[i] - METER_FALL * dt);
    }
}

/// The body: the display over most of it, and under it the selected band's card, the
/// band's transfer curve and the output.
#[allow(clippy::too_many_arguments)]
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    set: &Settings,
    lay: &Layout,
    tel: &cantus_engine::Telemetry,
    sr: f32,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let row_h = BAND_CARD * k;
    let disp = g.full((area.height() - row_h - s.d(4.0)).max(160.0 * k));
    display(host, st, ui, s, disp, set, lay, tel, sr);
    let total = area.width() - 2.0 * s.d(4.0);
    let side = (TRANSFER_W + OUTPUT_W) * k;
    let fr = [
        (total - side) / total,
        TRANSFER_W * k / total,
        OUTPUT_W * k / total,
    ];
    let mut row = g.row(row_h, &fr);
    let r: [Rect; 3] = std::array::from_fn(|_| row.next().unwrap_or(Rect::NOTHING));
    band_card(host, st, ui, s, r[0], set);
    transfer_card(host, st, ui, s, r[1], set, tel);
    let so = s.function(Function::Output);
    controls::card(ui, &so, r[2], Src::Key("fx.mb.out"), None, |ui, _| {
        let resp = controls::param(ui, &so, host, "out", Size::M);
        if resp.hovered() && !resp.dragged() {
            hint::set_text(
                ui.ctx(),
                frame::tr("fx.mb.out"),
                frame::tr("mb.tip.out"),
                Caps::PARAM,
            );
        }
    });
}

// ---- the display ------------------------------------------------------------------

thread_local! {
    static BUF: std::cell::RefCell<String> = std::cell::RefCell::new(String::with_capacity(64));
}

/// Run `f` with `args` written into a reused buffer.
pub(crate) fn with_fmt<R>(args: fmt::Arguments<'_>, f: impl FnOnce(&str) -> R) -> R {
    BUF.with(|cell| match cell.try_borrow_mut() {
        Ok(mut buf) => {
            buf.clear();
            let _ = buf.write_fmt(args);
            f(&buf)
        }
        Err(_) => f(&fmt::format(args)),
    })
}

/// A word on the display, anchored at `at`, no wider than `max_w` (then elided), on a
/// plate when given one. Returns where it went.
#[allow(clippy::too_many_arguments)]
fn put_in(
    b: &mut Brush,
    at: Pos2,
    align: Align2,
    role: Role,
    color: Color32,
    k: f32,
    max_w: f32,
    plate: Option<Color32>,
    args: fmt::Arguments<'_>,
) -> Rect {
    with_fmt(args, |text| {
        let t = Text::new(text, role, color)
            .caps(false)
            .align(Align2::CENTER_CENTER)
            .policy(crate::kit::text::Policy::Elide(
                crate::kit::text::Elide::End,
            ));
        let size = crate::kit::text::measure(b.ctx(), &t, k);
        let size = vec2(size.x.min(max_w.max(0.0)), size.y);
        if size.x < 8.0 * k {
            return Rect::NOTHING;
        }
        let r = align.anchor_size(at, size);
        if let Some(c) = plate {
            b.rrect(
                b.snap_rect(r.expand2(vec2(3.0 * k, 1.0 * k))),
                2.0 * k,
                Fill::Solid(c),
            );
        }
        match crate::kit::text::paint(b, r.expand(1.0), &t, k) {
            Some(p) => p.rect,
            None => Rect::NOTHING,
        }
    })
}

/// The panes of the display, carved from its rect.
fn carve(rect: Rect, s: &Style, bands: usize) -> (Rect, Geo) {
    let k = s.k;
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD * k));
    let inner = Rect::from_min_max(pos2(rect.left(), head.bottom()), rect.max);
    let meters_w = 60.0 * k;
    let gutter = 30.0 * k;
    let pad = 8.0 * k;
    let lanes_h = (14.0 * k * bands.max(2) as f32)
        .max(44.0 * k)
        .min(inner.height() * 0.26);
    let gap = 6.0 * k;
    let axis_h = 16.0 * k;
    let rest = inner.height() - lanes_h - axis_h - 4.0 * k;
    let gain_h = (rest * 0.34).round();
    let main = Rect::from_min_max(
        pos2(rect.left() + gutter, inner.top()),
        pos2(rect.right() - meters_w - pad, inner.bottom()),
    );
    let level = Rect::from_min_size(main.min, vec2(main.width(), rest - gap - gain_h));
    let gain = Rect::from_min_size(
        pos2(main.left(), level.bottom() + gap),
        vec2(main.width(), gain_h),
    );
    let lanes = Rect::from_min_max(
        pos2(rect.left() + 4.0 * k, gain.bottom() + axis_h),
        pos2(rect.right() - 4.0 * k, inner.bottom() - 4.0 * k),
    );
    let meters = Rect::from_min_max(
        pos2(main.right() + pad, inner.top() + 4.0 * k),
        pos2(rect.right() - pad, gain.bottom()),
    );
    (
        head,
        Geo {
            level,
            gain,
            lanes,
            meters,
        },
    )
}

/// The width the head's selectors take at 100 %, from its left edge.
const HEAD_CONTROLS: f32 = 8.0 + 186.0 + 8.0 + 120.0 + 8.0 + 186.0 + 8.0 + 150.0;

/// Evaluate the response now, its ghost, and (with a mid- or side-only band) the side's
/// curve, into the state's buffers.
fn curves(
    st: &mut State,
    set: &Settings,
    lay: &Layout,
    tel: &cantus_engine::Telemetry,
    sr: f32,
    geo: &Geo,
) -> (bool, bool) {
    if lay.n == 0 {
        return (false, false);
    }
    // A bypassed band is at 0 dB, whatever its gain says.
    let statics: [f32; MB_BANDS] = std::array::from_fn(|b| {
        if set.bands[b].on {
            set.bands[b].gain_db
        } else {
            0.0
        }
    });
    // Solo and mute are part of what is heard: a band that is silent is drawn at the
    // floor, as the output has it.
    let any_solo = set.bands.iter().any(|b| b.used && b.solo);
    let silent = |b: usize| set.bands[b].mute || (any_solo && !set.bands[b].solo);
    let now: [f32; MB_BANDS] = std::array::from_fn(|b| {
        if silent(b) {
            -60.0
        } else {
            statics[b] + tel[T_GAIN + b]
        }
    });
    let hist = &st.history;
    let ghost: [f32; MB_BANDS] = std::array::from_fn(|b| {
        let deepest = hist
            .iter()
            .rev()
            .take(GHOST)
            .map(|h| h[b])
            .fold(0.0f32, |m, v| if v.abs() > m.abs() { v } else { m });
        if silent(b) {
            -60.0
        } else {
            statics[b] + deepest
        }
    });
    let eval = |db: &[f32; MB_BANDS], side: bool, out: &mut [Pos2; POINTS]| {
        for (i, p) in out.iter_mut().enumerate() {
            let hz = point_hz(i);
            *p = pos2(
                geo.x(hz),
                geo.y_gain(response_part_db(set, db, sr, hz, side)),
            );
        }
    };
    eval(&now, false, &mut st.curve);
    let has_ghost = ghost != now;
    if has_ghost {
        eval(&ghost, false, &mut st.ghost);
    }
    let split = (0..lay.n).any(|p| set.bands[lay.order[p]].placement != Placement::Stereo);
    if split {
        eval(&now, true, &mut st.side);
    }
    (has_ghost, split)
}

#[allow(clippy::too_many_arguments)]
fn display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    set: &Settings,
    lay: &Layout,
    tel: &cantus_engine::Telemetry,
    sr: f32,
) {
    let (head, geo) = carve(rect, s, lay.n);
    st.geo = Some(geo);
    let panes = Rect::from_min_max(pos2(rect.left(), head.bottom()), rect.max);
    let resp = ui.interact(panes, ui.id().with("mb-display"), Sense::click_and_drag());
    // Hit-test where the press began: by the time egui calls it a drag the pointer has
    // already moved a few pixels off what it grabbed.
    let pointer = if resp.drag_started() {
        ui.input(|i| i.pointer.press_origin()).or(resp.hover_pos())
    } else {
        resp.hover_pos()
    };
    let (has_ghost, split) = curves(st, set, lay, tel, sr, &geo);
    let keyed = set.bands.iter().filter(|b| b.used && b.key_ext).count();
    st.words.prepare(lay.n, keyed);
    let (hovered, hovered_xo, tag) = paint_display(
        st, ui, s, rect, head, &geo, set, lay, tel, pointer, has_ghost, split,
    );
    if ui.rect_contains_pointer(tag) {
        hint::set_text(
            ui.ctx(),
            frame::tr("mb.via_split"),
            frame::tr("mb.via_split_tip"),
            Caps::NONE,
        );
    }

    // The empty state's quick starts, over the invitation's plate.
    if lay.n == 0 {
        let k = s.k;
        let row = Rect::from_center_size(
            geo.level.center() + vec2(0.0, 18.0 * k),
            vec2(3.0 * 96.0 * k + 16.0 * k, controls::ROW * k),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        child.spacing_mut().item_spacing.x = 8.0 * k;
        let mut chosen = None;
        for (i, n) in [2usize, 3, 4].into_iter().enumerate() {
            let label = st.words.quick.get(i).map_or("", String::as_str);
            if controls::key(
                &mut child,
                s,
                Src::Str(label),
                frame::tr("mb.quick_tip"),
                96.0,
                false,
            )
            .clicked()
            {
                chosen = Some(n);
            }
        }
        if let Some(n) = chosen {
            quick_bands(host, st, n);
        }
    }

    head_controls(host, st, ui, s, head, set);
    gestures(host, st, ui, s, &resp, set, lay, &geo, hovered, hovered_xo);
    band_menu(host, st, ui, s);
}

/// The selectors inside the display's head: the mode, the slope, what to hear, the key.
fn head_controls(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    head: Rect,
    set: &Settings,
) {
    let k = s.k;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(head.shrink2(vec2(8.0 * k, 3.0 * k)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing.x = 8.0 * k;
    let ui = &mut child;
    if let Some(i) = choice_key(
        ui,
        s,
        host,
        "mode",
        set.mode as usize,
        &MODE_LABELS,
        &MODE_TIPS,
        frame::tr("fx.mb.mode"),
        186.0,
    ) {
        host.set("mode", i as f32, None);
    }
    let slope = Slope::ALL.iter().position(|x| *x == set.slope).unwrap_or(2);
    if let Some(i) = choice_key(
        ui,
        s,
        host,
        "slope",
        slope,
        &SLOPE_LABELS,
        &SLOPE_TIPS,
        frame::tr("fx.mb.slope"),
        120.0,
    ) {
        host.set("slope", Slope::ALL[i].db(), None);
    }
    if let Some(i) = choice_key(
        ui,
        s,
        host,
        "listen",
        set.listen as usize,
        &LISTEN_LABELS,
        &LISTEN_TIPS,
        frame::tr("fx.mb.listen"),
        186.0,
    ) {
        host.set("listen", i as f32, None);
    }
    key_source(host, st, ui, s, 150.0);
}

/// Where the bands' key comes from: any strip of the desk (Cantus), or the plug-in's
/// sidechain input, switched on (another DAW routes it).
fn key_source(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style, w: f32) {
    let rev = host.revision();
    if st.key.as_ref().is_none_or(|(r, _)| *r != rev) {
        st.key = Some((rev, host.key_source()));
    }
    let name = frame::tr("mb.key");
    match st.key.as_ref().and_then(|(_, k)| k.as_ref()) {
        Some(KeySource::Strips {
            current,
            label,
            options,
        }) => {
            let current = *current;
            let id = ui.id().with("mb-key");
            let value = if current.is_some() {
                label.as_str()
            } else {
                frame::tr("mb.key_off")
            };
            let at = controls::field(ui, s, id, value, name, w);
            if let Some(at) = at {
                let mut items = vec![frame::radio(Src::Key("mb.key_off"), current.is_none())];
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
                Src::Key("mb.key_off"),
                Src::Key("export.sidechain.external"),
            ];
            let tips = ["mb.key_tip", "mb.key_tip"];
            if let Some(i) = choice(ui, s, on as usize, &labels, &tips, name, w) {
                host.set_key_source(KeyTarget::Switch(i == 1));
            }
        }
        None => {}
    }
}

/// A threshold's notch at its line's right end: down for the upper, up for the lower.
fn notch(b: &mut Brush, x: f32, y: f32, down: bool, color: Color32, k: f32) {
    let tip = if down { y + 7.0 * k } else { y - 7.0 * k };
    b.polygon(
        &[pos2(x - 5.0 * k, y), pos2(x + 5.0 * k, y), pos2(x, tip)],
        color,
    );
}

/// The display's geometry, in one mesh, then its words. Returns what the pointer is on:
/// a band's handle, or a crossover.
#[allow(clippy::too_many_arguments)]
fn paint_display(
    st: &State,
    ui: &egui::Ui,
    s: &Style,
    rect: Rect,
    head: Rect,
    geo: &Geo,
    set: &Settings,
    lay: &Layout,
    tel: &cantus_engine::Telemetry,
    pointer: Option<Pos2>,
    has_ghost: bool,
    split: bool,
) -> (Option<(usize, Grab)>, Option<usize>, Rect) {
    let k = s.k;
    let ink = Ink::of(s);
    let (level, gain, lanes) = (geo.level, geo.gain, geo.lanes);
    let selected = st.selected;
    let drag = st.drag;
    let mut hovered: Option<(usize, Grab)> = None;
    let mut hovered_xo = None;
    // The crossovers' handles, for their words.
    let mut handles = [(Rect::NOTHING, false); MB_BANDS];
    let tag = with_brush(&ui.painter_at(rect), |b| {
        let px = b.px();
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(head.bottom()),
            px,
            ink.edge,
        );

        // ---- band columns: the band's colour, stronger when selected ----
        for pos in 0..lay.n {
            let bnd = lay.order[pos];
            let band = &set.bands[bnd];
            let Some((x0, x1)) = geo.span(lay, bnd) else {
                continue;
            };
            let a = if selected == Some(bnd) { 0.15 } else { 0.06 };
            let a = if !band.on || band.mute { a * 0.4 } else { a };
            let c = Ink::a(band_ink(bnd, s), a);
            for r in [level, gain] {
                b.rect(
                    Rect::from_min_max(pos2(x0, r.top()), pos2(x1, r.bottom())),
                    Fill::Solid(c),
                );
            }
        }

        // ---- grids: decades across both panes, dB lines on each pane's own scale ----
        for decade in [10.0f32, 100.0, 1_000.0, 10_000.0] {
            for m in [1.0, 2.0, 5.0] {
                let hz = decade * m;
                if !(LO_HZ..=HI_HZ).contains(&hz) {
                    continue;
                }
                let c = if m == 1.0 { ink.grid_strong } else { ink.grid };
                let x = b.snap_mid(geo.x(hz));
                for r in [level, gain] {
                    b.vline(x, r.top(), r.bottom(), px, c);
                }
            }
        }
        let mut db = -12.0f32;
        while db > LEVEL_LO {
            b.hline(
                level.left(),
                level.right(),
                b.snap_mid(geo.y_level(db)),
                px,
                ink.grid,
            );
            db -= 12.0;
        }
        for g in [-12.0f32, -6.0, 0.0, 6.0, 12.0] {
            let c = if g == 0.0 { ink.grid_strong } else { ink.grid };
            b.hline(gain.left(), gain.right(), b.snap_mid(geo.y_gain(g)), px, c);
        }

        // ---- the spectrum: the input as a faint fill, the output as a line ----
        if st.pre.len() == POINTS && st.pre.iter().any(|v| *v > -119.0) {
            let mut pts = [Pos2::ZERO; POINTS];
            for (i, p) in pts.iter_mut().enumerate() {
                *p = pos2(geo.x(point_hz(i)), geo.y_level(st.pre[i]));
            }
            b.area(
                &pts,
                level.bottom(),
                Ink::a(ink.dim, if ink.dark { 0.14 } else { 0.12 }),
            );
            for (i, p) in pts.iter_mut().enumerate() {
                *p = pos2(geo.x(point_hz(i)), geo.y_level(st.post[i]));
            }
            b.polyline(&pts, 1.25 * k, Ink::a(ink.dim, 0.85));
        }

        // ---- per band, in the level pane: its detector level as a bar, its thresholds
        // as lines to drag, its number at the top, its free key range ----
        let mut key_row = 0.0f32;
        for pos in 0..lay.n {
            let bnd = lay.order[pos];
            let band = &set.bands[bnd];
            let Some((x0, x1)) = geo.span(lay, bnd) else {
                continue;
            };
            let color = band_ink(bnd, s);
            let faint = Ink::a(color, 0.4);
            let (x0i, x1i) = (x0 + 2.0 * k, x1 - 2.0 * k);
            let level_db = tel[T_LEVEL + bnd];
            let thr_y = geo.y_level(band.thr_db);
            let lo_y = geo.y_level(band.thr_lo_db);
            let upper_on = band.ratio != 1.0 && band.range_db > 0.0;
            let lower_on = band.ratio_lo != 1.0 && band.range_lo_db > 0.0;
            // The level: a bar at the column's left edge, lit where it is past a threshold
            // that is working — so "is it acting?" reads at a glance.
            let bar = Rect::from_min_max(
                pos2(x0i + 2.0 * k, level.top() + 22.0 * k),
                pos2(x0i + 8.0 * k, level.bottom()),
            );
            b.rect(b.snap_rect(bar), Fill::Solid(Ink::a(ink.grid_strong, 0.6)));
            if level_db > LEVEL_LO {
                let ly = geo.y_level(level_db).max(bar.top());
                b.rect(
                    Rect::from_min_max(pos2(bar.left(), ly), bar.max),
                    Fill::Solid(Ink::a(color, 0.4)),
                );
                let lit = |a: f32, z: f32| {
                    Rect::from_min_max(pos2(bar.left() - k, a), pos2(bar.right() + k, z))
                };
                if upper_on && level_db > band.thr_db {
                    b.rect(lit(ly, thr_y), Fill::Solid(color));
                }
                if lower_on && level_db < band.thr_lo_db {
                    b.rect(lit(lo_y, ly), Fill::Solid(color));
                }
            }
            // Thresholds: the upper a solid line with a notch at its right end, the lower
            // a dashed one with the notch the other way up. A section that is off (1:1)
            // is faint, and the lower one is only drawn when it is on or the band is
            // selected.
            let line_x0 = bar.right() + 3.0 * k;
            let near = |y: f32| {
                pointer.is_some_and(|pt| {
                    pt.x >= x0 && pt.x <= x1 && (pt.y - y).abs() <= 5.0 * k && level.contains(pt)
                })
            };
            let held = |lower: bool| matches!(drag, Some(Drag::Threshold { band, lower: l }) if band == bnd && l == lower);
            let show_lo = lower_on || selected == Some(bnd);
            let hot_hi = held(false) || (!held(true) && near(thr_y));
            let hot_lo = show_lo && !hot_hi && (held(true) || near(lo_y));
            if hot_hi {
                hovered = Some((bnd, Grab::Threshold(false)));
            } else if hot_lo {
                hovered = Some((bnd, Grab::Threshold(true)));
            }
            let w = |hot: bool| if hot { 2.5 * k } else { 1.5 * k };
            let hi_c = if upper_on { color } else { faint };
            b.segment(
                pos2(line_x0, thr_y),
                pos2(x1i, thr_y),
                w(hot_hi),
                hi_c,
                false,
            );
            notch(b, x1i - 7.0 * k, thr_y, true, hi_c, k);
            if show_lo {
                let lo_c = if lower_on { color } else { faint };
                canvas::dashed(
                    b,
                    pos2(line_x0, lo_y),
                    pos2(x1i, lo_y),
                    w(hot_lo),
                    lo_c,
                    5.0 * k,
                    3.0 * k,
                );
                notch(b, x1i - 7.0 * k, lo_y, false, lo_c, k);
            }
            // The band's number at the top of its column.
            if x1 - x0 > 20.0 * k {
                b.circle(chip_at(x0, x1, level, band, k), 7.0 * k, Fill::Solid(color));
            }
            // A free key range: a bar along the bottom with a handle at each end, dragged
            // like everything else here. The detector hears what lies between them.
            if band.key_free {
                let ky = level.bottom() - 10.0 * k - key_row * 9.0 * k;
                key_row += 1.0;
                let (kx0, kx1) = (geo.x(band.key_lo_hz), geo.x(band.key_hi_hz));
                b.segment(
                    pos2(kx0, ky),
                    pos2(kx1, ky),
                    3.0 * k,
                    Ink::a(color, 0.8),
                    false,
                );
                for (x, lo) in [(kx0, true), (kx1, false)] {
                    let hot = drag == Some(Drag::Key { band: bnd, lo })
                        || (hovered.is_none()
                            && pointer.is_some_and(|pt| pt.distance(pos2(x, ky)) <= 7.0 * k));
                    if hot {
                        hovered = Some((bnd, Grab::Key(lo)));
                    }
                    let d = if lo { 6.0 * k } else { -6.0 * k };
                    b.polygon(
                        &[
                            pos2(x, ky - 6.0 * k),
                            pos2(x, ky + 6.0 * k),
                            pos2(x + d, ky),
                        ],
                        if hot { ink.hue } else { color },
                    );
                }
            }
        }

        // ---- per band, in the gain pane: the lens of what it can reach, its static gain
        // as a dot to drag, and what it is doing now as a tick ----
        for pos in 0..lay.n {
            let bnd = lay.order[pos];
            let band = &set.bands[bnd];
            let Some((x0, x1)) = geo.span(lay, bnd) else {
                continue;
            };
            let color = band_ink(bnd, s);
            let (x0i, x1i) = (x0 + 2.0 * k, x1 - 2.0 * k);
            let (cut, boost) = reach(band);
            let base = band.gain_db;
            if band.on && (cut > 0.0 || boost > 0.0) {
                let top = geo.y_gain(base + boost);
                let bottom = geo.y_gain(base - cut);
                b.rect(
                    Rect::from_min_max(pos2(x0i, top), pos2(x1i, bottom)),
                    Fill::Solid(Ink::a(color, 0.14)),
                );
                // The lens's ends are brackets, so it cannot be taken for a threshold.
                for y in [top, bottom] {
                    for (a, d) in [(x0i, 6.0 * k), (x1i, -6.0 * k)] {
                        b.segment(pos2(a, y), pos2(a + d, y), px, color, false);
                    }
                }
            }
            let dyn_db = tel[T_GAIN + bnd];
            if band.on && dyn_db.abs() > 0.05 {
                let y = geo.y_gain(base + dyn_db);
                b.segment(
                    pos2(x0i + 4.0 * k, y),
                    pos2(x1i - 4.0 * k, y),
                    2.0 * k,
                    color,
                    false,
                );
            }
            let dot = pos2(geo.x(centre_hz(lay, bnd)), geo.y_gain(base));
            let dot_near = pointer.is_some_and(|pt| pt.distance(dot) <= (DOT_R + 3.0) * k);
            let held = matches!(drag, Some(Drag::Gain { band, .. }) if band == bnd);
            if (dot_near && hovered.is_none()) || held {
                hovered = Some((bnd, Grab::Gain));
            }
            if band.on {
                b.circle(dot, DOT_R * k, Fill::Solid(color));
            } else {
                b.circle_stroke(dot, DOT_R * k, 1.5 * k, color);
            }
            if selected == Some(bnd) || dot_near || held {
                b.circle_stroke(dot, (DOT_R + 2.5) * k, 1.5 * k, ink.text);
            }
        }

        // ---- the response now, its ghost, and the side's curve ----
        if lay.n > 0 {
            if has_ghost {
                dashed_poly(
                    b,
                    &st.ghost,
                    1.0 * k,
                    Ink::a(ink.hue, 0.75),
                    4.0 * k,
                    3.0 * k,
                );
            }
            if split {
                b.polyline(&st.side, 1.5 * k, Ink::a(ink.hue, 0.8));
            }
            b.polyline(&st.curve, 2.5 * k, ink.hue);
        }

        // ---- crossovers: a line through both panes, a handle with the frequency where
        // the panes meet ----
        for (pos, slot) in handles.iter_mut().enumerate().take(lay.n).skip(1) {
            let bnd = lay.order[pos];
            let hz = lay.crossovers[pos - 1];
            let x = geo.x(hz);
            let size = with_fmt(format_args!("{}", Hz(hz)), |text| {
                crate::kit::text::measure(b.ctx(), &Text::new(text, Role::Value, ink.text), s.tk())
            });
            let handle = Rect::from_center_size(
                pos2(x, (level.bottom() + gain.top()) * 0.5),
                vec2(size.x + 10.0 * k, size.y + 4.0 * k),
            );
            let held = drag == Some(Drag::Crossover { band: bnd });
            let near = pointer.is_some_and(|pt| {
                handle.expand(2.0 * k).contains(pt)
                    || ((pt.x - x).abs() <= 5.0 * k && (level.contains(pt) || gain.contains(pt)))
            });
            let hot = held || (near && hovered.is_none());
            if hot {
                hovered_xo = Some(bnd);
            }
            let c = if hot { ink.hue } else { ink.dim };
            for r in [level, gain] {
                canvas::dashed(
                    b,
                    pos2(x, r.top()),
                    pos2(x, r.bottom()),
                    if hot { 2.0 * k } else { px },
                    c,
                    4.0 * k,
                    3.0 * k,
                );
            }
            let hr = b.snap_rect(handle);
            b.rrect(
                hr,
                3.0 * k,
                Fill::Solid(if hot { ink.hue } else { s.t.deck_title }),
            );
            b.rrect_stroke(hr, 3.0 * k, px, if hot { ink.hue } else { ink.grid_strong });
            *slot = (hr, hot);
        }

        // ---- the lanes: each band's gain over the last seconds ----
        let hist = &st.history;
        if lay.n > 0 {
            let lane_h = lanes.height() / lay.n as f32;
            let (x0, x1) = (lanes.left() + 28.0 * k, lanes.right() - 56.0 * k);
            let dx = (x1 - x0) / HISTORY as f32;
            let start = x1 - dx * hist.len() as f32;
            let n = hist.len().min(HISTORY);
            let mut pts = [Pos2::ZERO; HISTORY];
            for pos in 0..lay.n {
                let bnd = lay.order[pos];
                let color = band_ink(bnd, s);
                let top = lanes.top() + lane_h * pos as f32;
                let mid = top + lane_h * 0.5;
                let half = lane_h * 0.5 - 1.0;
                if pos > 0 {
                    b.hline(lanes.left(), lanes.right(), b.snap_mid(top), px, ink.grid);
                }
                b.hline(x0, x1, b.snap_mid(mid), px, ink.grid_strong);
                b.circle(
                    pos2(lanes.left() + 9.0 * k, mid),
                    (half - 1.0).clamp(2.0 * k, 5.0 * k),
                    Fill::Solid(color),
                );
                // Cuts down, boosts up; 12 dB fills half a lane.
                for (i, h) in hist.iter().enumerate().take(n) {
                    pts[i] = pos2(
                        start + dx * i as f32,
                        mid - (h[bnd] / 12.0).clamp(-1.0, 1.0) * half,
                    );
                }
                b.area(&pts[..n], mid, Ink::a(color, 0.75));
            }
        }

        // ---- the meters: in and out, left and right ----
        let mut batch = meter::MeterBatch::new(s);
        for i in 0..2 {
            let r = meter_rect(geo, i, k);
            let lv = Levels {
                level: [st.meters[2 * i], st.meters[2 * i + 1]],
                hold: [st.meters[2 * i], st.meters[2 * i + 1]],
            };
            batch.paint(b, r, &lv);
        }

        // ---- the empty state: its words on a card of their own ----
        if lay.n == 0 {
            let card = empty_card(level, k);
            b.rrect(b.snap_rect(card), s.r(6.0), Fill::Solid(s.t.deck));
            b.rrect_stroke(card, s.r(6.0), px, ink.edge);
        }
        for r in [level, gain, lanes] {
            canvas::edge_b(b, r, &ink);
        }

        display_words(
            b, st, s, &ink, head, geo, set, lay, tel, pointer, split, &handles,
        )
    });
    (hovered, hovered_xo, tag)
}

/// Where a band's number sits at the top of its column: centred, or moved left to make
/// room for its states beside it.
fn chip_at(x0: f32, x1: f32, level: Rect, band: &BandSettings, k: f32) -> Pos2 {
    let c = pos2((x0 + x1) * 0.5, level.top() + 10.0 * k);
    if Flags(band).any() && x1 - x0 > 56.0 * k {
        pos2((c.x - 24.0 * k).max(x0 + 10.0 * k), c.y)
    } else {
        c
    }
}

/// Meter `i` (0 in, 1 out) in the meters' column, under its name and peak.
fn meter_rect(geo: &Geo, i: usize, k: f32) -> Rect {
    let m = geo.meters;
    let w = ((m.width() - 8.0 * k) * 0.5).min(20.0 * k);
    let cx = m.left() + m.width() * (0.25 + 0.5 * i as f32);
    Rect::from_min_max(
        pos2(cx - w * 0.5, m.top() + 30.0 * k),
        pos2(cx + w * 0.5, m.bottom()),
    )
}

/// The invitation's card on an empty display.
fn empty_card(level: Rect, k: f32) -> Rect {
    Rect::from_center_size(
        level.center(),
        vec2((420.0 * k).min(level.width() - 16.0 * k), 84.0 * k),
    )
}

/// A tick on a gain scale: `0`, `+12`, `-12`.
struct Tick(f32);

impl Display for Tick {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0.0 {
            f.write_str("0")
        } else {
            write!(f, "{:+.0}", self.0)
        }
    }
}

/// A meter's peak: `-7.3`, or `-∞`.
struct Peak(f32);

impl Display for Peak {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 > -60.0 {
            write!(f, "{:.1}", self.0)
        } else {
            f.write_str("-\u{221e}")
        }
    }
}

/// The display's words, after all of its geometry, so nothing is painted over them:
/// the scales, the bands' numbers, states and thresholds, the crossovers' frequencies,
/// the lanes' values, the meters' peaks, and in the head what is under the pointer.
/// Returns where the "heard through the split" tag went, for its hint.
#[allow(clippy::too_many_arguments)]
fn display_words(
    b: &mut Brush,
    st: &State,
    s: &Style,
    ink: &Ink,
    head: Rect,
    geo: &Geo,
    set: &Settings,
    lay: &Layout,
    tel: &cantus_engine::Telemetry,
    pointer: Option<Pos2>,
    split: bool,
    handles: &[(Rect, bool); MB_BANDS],
) -> Rect {
    let k = s.k;
    let tk = s.tk();
    let (level, gain, lanes) = (geo.level, geo.gain, geo.lanes);
    let plate = Some(ink.plate);
    let micro = |b: &mut Brush, at: Pos2, align: Align2, args: fmt::Arguments<'_>| {
        canvas::putf(b, at, align, Role::Micro, ink.dim, tk, None, args)
    };

    // ---- the scales: each pane's in the gutter, the frequencies under the gain pane ----
    let top = canvas::put_key(
        b,
        pos2(level.left() - 4.0 * k, level.top()),
        Align2::RIGHT_TOP,
        "mb.axis_level",
        Role::Micro,
        ink.dim,
        tk,
    );
    let mut db = -12.0f32;
    while db > LEVEL_LO {
        let y = geo.y_level(db);
        if y - 6.0 * k > top.bottom() {
            micro(
                b,
                pos2(level.left() - 4.0 * k, y),
                Align2::RIGHT_CENTER,
                format_args!("{db:.0}"),
            );
        }
        db -= 12.0;
    }
    for g in [-12.0f32, 0.0, 12.0] {
        micro(
            b,
            pos2(gain.left() - 4.0 * k, geo.y_gain(g)),
            Align2::RIGHT_CENTER,
            format_args!("{}", Tick(g)),
        );
    }
    let mut ruler = canvas::Ruler::new();
    for decade in [10.0f32, 100.0, 1_000.0, 10_000.0] {
        for m in [1.0, 5.0] {
            let hz = decade * m;
            if !(LO_HZ..=HI_HZ).contains(&hz) {
                continue;
            }
            let x = geo.x(hz) + 3.0 * k;
            if !ruler.fits(x, 8.0 * k) {
                continue;
            }
            let r = micro(
                b,
                pos2(x, gain.bottom() + 2.0 * k),
                Align2::LEFT_TOP,
                format_args!("{}", Hz(hz)),
            );
            ruler.placed(r);
        }
    }

    // ---- per band: its number and states at the top, its thresholds, its key range, its
    // gain now, its dot ----
    let selected = st.selected;
    let mut key_row = 0.0f32;
    for pos in 0..lay.n {
        let bnd = lay.order[pos];
        let band = &set.bands[bnd];
        let Some((x0, x1)) = geo.span(lay, bnd) else {
            continue;
        };
        let color = band_ink(bnd, s);
        if x1 - x0 > 20.0 * k {
            let c = chip_at(x0, x1, level, band, k);
            canvas::putf(
                b,
                c,
                Align2::CENTER_CENTER,
                Role::Micro,
                on_fill(color),
                tk,
                None,
                format_args!("{}", bnd + 1),
            );
            if Flags(band).any() && x1 - x0 > 56.0 * k {
                put_in(
                    b,
                    pos2(c.x + 11.0 * k, c.y),
                    Align2::LEFT_CENTER,
                    Role::Micro,
                    ink.text,
                    tk,
                    x1 - c.x - 16.0 * k,
                    plate,
                    format_args!("{}", Flags(band)),
                );
            }
        }
        let wide = x1 - x0 > 64.0 * k || selected == Some(bnd);
        let upper_on = band.ratio != 1.0 && band.range_db > 0.0;
        let lower_on = band.ratio_lo != 1.0 && band.range_lo_db > 0.0;
        let line_x0 = x0 + 15.0 * k;
        let room = x1 - line_x0 - 20.0 * k;
        if wide && (upper_on || selected == Some(bnd)) {
            put_in(
                b,
                pos2(line_x0 + 3.0 * k, geo.y_level(band.thr_db) - 3.0 * k),
                Align2::LEFT_BOTTOM,
                Role::Label,
                if upper_on { ink.text } else { ink.dim },
                tk,
                room,
                plate,
                format_args!("{:.0} dB \u{b7} {}", band.thr_db, Ratio(band.ratio)),
            );
        }
        if wide && (lower_on || selected == Some(bnd)) {
            put_in(
                b,
                pos2(line_x0 + 3.0 * k, geo.y_level(band.thr_lo_db) + 3.0 * k),
                Align2::LEFT_TOP,
                Role::Label,
                if lower_on { ink.text } else { ink.dim },
                tk,
                room,
                plate,
                format_args!("{:.0} dB \u{b7} {}", band.thr_lo_db, Ratio(band.ratio_lo)),
            );
        }
        if band.key_free {
            let ky = level.bottom() - 10.0 * k - key_row * 9.0 * k;
            key_row += 1.0;
            put_in(
                b,
                pos2(geo.x(band.key_lo_hz) - 4.0 * k, ky),
                Align2::RIGHT_CENTER,
                Role::Micro,
                color,
                tk,
                60.0 * k,
                plate,
                format_args!("{}", frame::tr("mb.flag_key")),
            );
        }
        let dyn_db = tel[T_GAIN + bnd];
        if band.on && dyn_db.abs() > 0.05 && x1 - x0 > 44.0 * k {
            let y = geo.y_gain(band.gain_db + dyn_db);
            let (dy, align) = if dyn_db < 0.0 {
                (2.0 * k, Align2::RIGHT_TOP)
            } else {
                (-2.0 * k, Align2::RIGHT_BOTTOM)
            };
            put_in(
                b,
                pos2(x1 - 5.0 * k, y + dy),
                align,
                Role::Micro,
                ink.text,
                tk,
                x1 - x0 - 10.0 * k,
                plate,
                format_args!("{dyn_db:+.1}"),
            );
        }
        let dot = pos2(geo.x(centre_hz(lay, bnd)), geo.y_gain(band.gain_db));
        canvas::putf(
            b,
            dot,
            Align2::CENTER_CENTER,
            Role::Micro,
            if band.on { on_fill(color) } else { ink.text },
            tk,
            None,
            format_args!("{}", bnd + 1),
        );
    }

    // ---- the mid and the side, named where they are furthest apart ----
    if split && lay.n > 0 {
        let room = gain.right() - 80.0 * k;
        let d = |i: usize| (st.side[i].y - st.curve[i].y).abs();
        let at = (0..POINTS)
            .filter(|i| st.side[*i].x < room)
            .max_by(|a, b| d(*a).total_cmp(&d(*b)))
            .unwrap_or(0);
        let (sp, mp) = (st.side[at], st.curve[at]);
        let side_above = sp.y <= mp.y;
        for (pt, key, above) in [
            (sp, "mb.flag_side", side_above),
            (mp, "mb.flag_mid", !side_above),
        ] {
            let (dy, align) = if above {
                (-3.0 * k, Align2::LEFT_BOTTOM)
            } else {
                (3.0 * k, Align2::LEFT_TOP)
            };
            put_in(
                b,
                pt + vec2(4.0 * k, dy),
                align,
                Role::Micro,
                ink.hue,
                tk,
                80.0 * k,
                plate,
                format_args!("{}", frame::tr(key)),
            );
        }
    }

    // ---- the crossovers' frequencies, on their handles ----
    for (&(hr, hot), hz) in handles[1..lay.n.max(1)].iter().zip(lay.crossovers.iter()) {
        let c = if hot { on_fill(ink.hue) } else { ink.text };
        canvas::putf(
            b,
            hr.center(),
            Align2::CENTER_CENTER,
            Role::Value,
            c,
            tk,
            None,
            format_args!("{}", Hz(*hz)),
        );
    }

    // ---- the lanes: each band's number, and its gain now ----
    if lay.n == 0 {
        canvas::put_key(
            b,
            lanes.center(),
            Align2::CENTER_CENTER,
            "mb.lanes_empty",
            Role::Body,
            ink.dim,
            tk,
        );
    } else {
        let lane_h = lanes.height() / lay.n as f32;
        for pos in 0..lay.n {
            let bnd = lay.order[pos];
            let mid = lanes.top() + lane_h * (pos as f32 + 0.5);
            micro(
                b,
                pos2(lanes.left() + 17.0 * k, mid),
                Align2::LEFT_CENTER,
                format_args!("{}", bnd + 1),
            );
            let now = st.history.back().map(|h| h[bnd]).unwrap_or(0.0);
            let c = if now.abs() > 0.05 { ink.text } else { ink.dim };
            let at = pos2(lanes.right() - 4.0 * k, mid);
            if set.bands[bnd].on {
                canvas::putf(
                    b,
                    at,
                    Align2::RIGHT_CENTER,
                    Role::Micro,
                    c,
                    tk,
                    None,
                    format_args!("{now:+.1} dB"),
                );
            } else {
                canvas::putf(
                    b,
                    at,
                    Align2::RIGHT_CENTER,
                    Role::Micro,
                    c,
                    tk,
                    None,
                    format_args!("{}", frame::tr("mb.flag_off")),
                );
            }
        }
    }

    // ---- the meters' names and peaks ----
    for (i, key) in ["mb.meter_in", "mb.meter_out"].into_iter().enumerate() {
        let r = meter_rect(geo, i, k);
        let x = r.center().x;
        canvas::put_key(
            b,
            pos2(x, geo.meters.top()),
            Align2::CENTER_TOP,
            key,
            Role::Micro,
            ink.dim,
            tk,
        );
        let peak = st.meters[2 * i].max(st.meters[2 * i + 1]);
        let c = if peak >= 0.0 { ink.danger } else { ink.text };
        canvas::putf(
            b,
            pos2(x, geo.meters.top() + 13.0 * k),
            Align2::CENTER_TOP,
            Role::Micro,
            c,
            tk,
            None,
            format_args!("{}", Peak(peak)),
        );
    }

    // ---- the invitation, when there are no bands ----
    if lay.n == 0 {
        let card = empty_card(level, k);
        put_in(
            b,
            pos2(card.center().x, card.top() + 22.0 * k),
            Align2::CENTER_CENTER,
            Role::Body,
            ink.text,
            tk,
            card.width() - 16.0 * k,
            None,
            format_args!("{}", frame::tr("mb.empty")),
        );
    }

    // ---- the head, at the right: what is under the pointer, or how many bands; then
    // what the output is heard through, and who listens to the key ----
    let right = head.right() - 10.0 * k;
    let limit = head.left() + HEAD_CONTROLS * k + 12.0 * k;
    let cy = head.center().y;
    let r = match pointer.filter(|pt| level.contains(*pt) || gain.contains(*pt)) {
        Some(pt) if level.contains(pt) => {
            let hz = geo.hz(pt.x);
            put_in(
                b,
                pos2(right, cy),
                Align2::RIGHT_CENTER,
                Role::Label,
                ink.text,
                tk,
                right - limit,
                None,
                format_args!(
                    "{}  \u{b7}  {}  \u{b7}  {:.1} dBFS",
                    Hz(hz),
                    Note(hz),
                    geo.level_at(pt.y)
                ),
            )
        }
        Some(pt) => {
            let hz = geo.hz(pt.x);
            put_in(
                b,
                pos2(right, cy),
                Align2::RIGHT_CENTER,
                Role::Label,
                ink.text,
                tk,
                right - limit,
                None,
                format_args!(
                    "{}  \u{b7}  {}  \u{b7}  {:+.1} dB",
                    Hz(hz),
                    Note(hz),
                    geo.gain_at(pt.y)
                ),
            )
        }
        None => {
            let count = st.words.count.as_ref().map_or("", |c| c.1.as_str());
            put_in(
                b,
                pos2(right, cy),
                Align2::RIGHT_CENTER,
                Role::Label,
                ink.dim,
                tk,
                right - limit,
                None,
                format_args!("{count}"),
            )
        }
    };
    let mut x = if r.is_positive() {
        r.left() - 14.0 * k
    } else {
        right
    };
    let mut tag = Rect::NOTHING;
    if tel[T_SPLIT] > 0.0 && set.mode == Mode::Dynamic {
        tag = put_in(
            b,
            pos2(x, cy),
            Align2::RIGHT_CENTER,
            Role::Micro,
            s.t.highlight,
            tk,
            x - limit,
            plate,
            format_args!("{}", frame::tr("mb.via_split")),
        );
        if tag.is_positive() {
            x = tag.left() - 14.0 * k;
        }
    }
    if let Some((_, text)) = st.words.keyed.as_ref().filter(|_| key_set(st)) {
        put_in(
            b,
            pos2(x, cy),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.dim,
            tk,
            x - limit,
            None,
            format_args!("{text}"),
        );
    }
    tag
}

/// Whether the bands have a key to listen to: a strip chosen, or the sidechain on.
fn key_set(st: &State) -> bool {
    matches!(
        st.key.as_ref().and_then(|(_, k)| k.as_ref()),
        Some(KeySource::Strips {
            current: Some(_),
            ..
        }) | Some(KeySource::Switch(true))
    )
}

/// Every drag, click and key on the display.
#[allow(clippy::too_many_arguments)]
fn gestures(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    resp: &egui::Response,
    set: &Settings,
    lay: &Layout,
    geo: &Geo,
    hovered: Option<(usize, Grab)>,
    hovered_xo: Option<usize>,
) {
    let fine = ui.input(|i| i.modifiers.shift);
    let band_at = |pt: Pos2| -> Option<usize> {
        (0..lay.n).map(|p| lay.order[p]).find(|b| {
            geo.span(lay, *b)
                .is_some_and(|(x0, x1)| pt.x >= x0 && pt.x < x1)
        })
    };
    if hovered_xo.is_some() || matches!(hovered, Some((_, Grab::Key(_)))) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    } else if matches!(
        hovered,
        Some((_, Grab::Threshold(_))) | Some((_, Grab::Gain))
    ) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.drag_started() {
        let origin = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos())
            .unwrap_or_default();
        let drag = match (hovered_xo, hovered) {
            (Some(band), _) => Some(Drag::Crossover { band }),
            (None, Some((band, Grab::Threshold(lower)))) => Some(Drag::Threshold { band, lower }),
            (None, Some((band, Grab::Gain))) => Some(Drag::Gain {
                band,
                origin_db: set.bands[band].gain_db,
                origin_y: origin.y,
            }),
            (None, Some((band, Grab::Key(lo)))) => Some(Drag::Key { band, lo }),
            (None, None) => None,
        };
        if let Some(
            Drag::Threshold { band, .. } | Drag::Gain { band, .. } | Drag::Key { band, .. },
        ) = drag
        {
            st.selected = Some(band);
        }
        st.drag = drag;
    }
    if let (Some(d), Some(pt)) = (st.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            match d {
                Drag::Crossover { band } => {
                    // Between its neighbours, a third of an octave from each.
                    let pos = lay.position(band).unwrap_or(0);
                    let lo = if pos >= 2 {
                        lay.crossovers[pos - 2]
                    } else {
                        LO_HZ
                    } * 2f32.powf(MIN_OCT);
                    let hi = if pos + 1 < lay.n {
                        lay.crossovers[pos]
                    } else {
                        HI_HZ * 2f32.powf(MIN_OCT)
                    } / 2f32.powf(MIN_OCT);
                    let hz = geo.hz(pt.x).clamp(lo.min(hi), hi.max(lo));
                    let k = key(band, "from");
                    host.set(k, hz, Some(k));
                }
                Drag::Threshold { band, lower } => {
                    let b = &set.bands[band];
                    let db = geo.level_at(pt.y);
                    if lower {
                        let k = key(band, "thr_lo");
                        host.set(k, db.clamp(-90.0, b.thr_db), Some(k));
                    } else {
                        let k = key(band, "thr");
                        host.set(k, db.clamp(b.thr_lo_db.max(-80.0), 0.0), Some(k));
                    }
                }
                Drag::Gain {
                    band,
                    origin_db,
                    origin_y,
                } => {
                    let scale = if fine { 0.2 } else { 1.0 };
                    let dy = (pt.y - origin_y) * scale;
                    let v = geo.gain_at(geo.y_gain(origin_db) + dy).clamp(-24.0, 24.0);
                    let k = key(band, "gain");
                    host.set(k, v, Some(k));
                }
                Drag::Key { band, lo } => {
                    // A sixth of an octave apart at least.
                    let b = &set.bands[band];
                    let hz = geo.hz(pt.x);
                    let (k, v) = if lo {
                        (
                            key(band, "key_lo"),
                            hz.clamp(LO_HZ, b.key_hi_hz / 2f32.powf(1.0 / 6.0)),
                        )
                    } else {
                        (
                            key(band, "key_hi"),
                            hz.clamp(b.key_lo_hz * 2f32.powf(1.0 / 6.0), HI_HZ),
                        )
                    };
                    host.set(k, v, Some(k));
                }
            }
        }
    }
    if resp.drag_stopped() {
        st.drag = None;
        host.end_gesture();
    }
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    if resp.clicked() {
        if let Some(b) = pointer.and_then(band_at) {
            st.selected = Some(b);
        }
    }
    if resp.double_clicked() && hovered.is_none() && hovered_xo.is_none() {
        if let Some(pt) = pointer.filter(|pt| geo.level.contains(*pt) || geo.gain.contains(*pt)) {
            if split_at(host, st, geo.hz(pt.x)).is_none() {
                host.status(t("mb.no_room"));
            }
        }
    }
    // Keys while the pointer is over the display: [ and ] step through the bands, Delete
    // removes the selected one, up and down move its threshold.
    if resp.hovered() {
        let (prev, next, del, up, down) = ui.input(|i| {
            (
                i.key_pressed(egui::Key::OpenBracket),
                i.key_pressed(egui::Key::CloseBracket),
                i.key_pressed(egui::Key::Delete),
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
            )
        });
        let sel = st.selected.and_then(|b| lay.position(b));
        if (prev || next) && lay.n > 0 {
            let p = match sel {
                Some(p) if next => (p + 1).min(lay.n - 1),
                Some(p) => p.saturating_sub(1),
                None => 0,
            };
            st.selected = Some(lay.order[p]);
        }
        if let Some(b) = st.selected.filter(|b| set.bands[*b].used) {
            if del {
                remove_band(host, st, b);
            } else if up || down {
                let step = if fine { 0.1 } else { 1.0 } * if up { 1.0 } else { -1.0 };
                let v = (set.bands[b].thr_db + step).clamp(-80.0, 0.0);
                host.set(key(b, "thr"), v, None);
            }
        }
        // The help line: the band under the pointer, and what the display's gestures do.
        let name = pointer
            .and_then(band_at)
            .and_then(|b| st.words.band.get(b))
            .map_or(frame::tr("mb.display"), String::as_str);
        hint::set_text(
            ui.ctx(),
            name,
            frame::tr("mb.display_tip"),
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    // Right-click: the band's menu.
    if resp.secondary_clicked() {
        let band = pointer
            .and_then(band_at)
            .or(st.selected)
            .filter(|b| set.bands[*b].used);
        if let Some(band) = band {
            let hz = pointer
                .filter(|pt| geo.level.contains(*pt) || geo.gain.contains(*pt))
                .map(|pt| geo.hz(pt.x));
            st.menu = Some(Menu {
                band,
                at: pointer.unwrap_or(resp.rect.center()),
                hz,
                split: hz.map_or_else(String::new, |h| {
                    tf("mb.split_here", &[("hz", &Hz(h).to_string())])
                }),
            });
        }
    }
    let _ = s;
}

/// What a row of the band's menu does.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Act {
    On,
    Solo,
    Mute,
    Place(usize),
    Split,
    Remove,
    None,
}

/// The band's menu, while open: its switches, where it sits in the image, split it at
/// the pointer, remove it.
fn band_menu(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui, s: &Style) {
    let Some(m) = st.menu.as_ref() else {
        return;
    };
    let set = settings(host.spec());
    let band = set.bands[m.band.min(MB_BANDS - 1)];
    if !band.used {
        st.menu = None;
        return;
    }
    let (b, at, hz) = (m.band, m.at, m.hz);
    let mut items: Vec<Item<'_>> = Vec::with_capacity(12);
    let mut acts: Vec<Act> = Vec::with_capacity(12);
    items.push(Item::Band(Src::Str(
        st.words.band.get(b).map_or("", String::as_str),
    )));
    acts.push(Act::None);
    items.push(frame::check(Src::Key("mb.bypass_band"), !band.on));
    acts.push(Act::On);
    items.push(frame::check(Src::Key("fx.mb.solo"), band.solo));
    acts.push(Act::Solo);
    items.push(frame::check(Src::Key("fx.mb.mute"), band.mute));
    acts.push(Act::Mute);
    items.push(Item::Band(Src::Key("fx.mb.place")));
    acts.push(Act::None);
    for (i, pl) in Placement::ALL.iter().enumerate() {
        items.push(frame::radio(PLACE_LABELS[i], band.placement == *pl));
        acts.push(Act::Place(i));
    }
    items.push(Item::Separator);
    acts.push(Act::None);
    if hz.is_some() {
        items.push(frame::action(Src::Str(&m.split)));
        acts.push(Act::Split);
    }
    items.push(frame::action(Src::Key("mb.remove_band")));
    acts.push(Act::Remove);
    let mut close = false;
    let opener = Rect::from_center_size(at, Vec2::splat(2.0));
    let chosen = frame::popup_list(
        ui.ctx(),
        ui.id().with("mb-band-menu"),
        at,
        opener,
        s,
        &items,
        210.0 * s.k,
        &mut close,
    );
    drop(items);
    if let Some(i) = chosen {
        close = true;
        match acts.get(i).copied().unwrap_or(Act::None) {
            Act::On => flip(host, b, "on", band.on),
            Act::Solo => flip(host, b, "solo", band.solo),
            Act::Mute => flip(host, b, "mute", band.mute),
            Act::Place(p) => host.set(key(b, "place"), p as f32, None),
            Act::Split => {
                if split_at(host, st, hz.unwrap_or(1_000.0)).is_none() {
                    host.status(t("mb.no_room"));
                }
            }
            Act::Remove => remove_band(host, st, b),
            Act::None => {}
        }
    }
    if close {
        st.menu = None;
    }
}

// ---- the band's card ------------------------------------------------------------------

/// What a band cell does, for the help line.
fn tip_of(field: &str) -> &'static str {
    match field {
        "thr" => "mb.tip.thr",
        "ratio" => "mb.tip.ratio",
        "range" => "mb.tip.range",
        "thr_lo" => "mb.tip.thr_lo",
        "ratio_lo" => "mb.tip.ratio_lo",
        "range_lo" => "mb.tip.range_lo",
        "knee" => "mb.tip.knee",
        "att" => "mb.tip.att",
        "rel" => "mb.tip.rel",
        "la" => "mb.tip.la",
        "gain" => "mb.tip.gain",
        "key_lo" => "mb.tip.key_lo",
        _ => "mb.tip.key_hi",
    }
}

fn ratio_pill(v: f32, b: &mut String) {
    let _ = write!(b, "{}", Ratio(v));
}

fn la_pill(v: f32, b: &mut String) {
    if v <= 0.0 {
        b.push_str(frame::tr("mb.off"));
    } else {
        controls::write_readout(v, "ms", b);
    }
}

/// A pill's writer: the value into the text the pill shows.
type Pill<'a> = &'a dyn Fn(f32, &mut String);

/// A level in a small cell's pill: the number alone (`-34.0`, `+6.0`) — the cell's name
/// says it is in dB, and a unit would push the number out of a 48-pt pill.
fn db_pill_s(v: f32, b: &mut String) {
    let _ = write!(b, "{v:+.1}");
}

/// A time in a small cell's pill: `1.5 ms`, `70 ms`, `1.2 s`.
fn ms_pill_s(v: f32, b: &mut String) {
    let _ = if v >= 1_000.0 {
        write!(b, "{:.1} s", v / 1_000.0)
    } else if v >= 10.0 {
        write!(b, "{v:.0} ms")
    } else {
        write!(b, "{v:.1} ms")
    };
}

/// A frequency in a small cell's pill: `350 Hz`, `4.5k`, `12.0k` (typed back as it reads).
fn hz_pill_s(v: f32, b: &mut String) {
    let _ = if v >= 1_000.0 {
        write!(b, "{:.1}k", v / 1_000.0)
    } else {
        write!(b, "{v:.0} Hz")
    };
}

fn la_pill_s(v: f32, b: &mut String) {
    if v <= 0.0 {
        b.push_str(frame::tr("mb.off"));
    } else {
        ms_pill_s(v, b);
    }
}

/// One of a band's cells: a drag is one step; hovered, the help line says what it does.
fn band_cell(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    band: usize,
    field: &str,
    size: Size,
) {
    let small = size == Size::S;
    let format: Option<Pill<'_>> = match field {
        "ratio" | "ratio_lo" => Some(&ratio_pill),
        "la" if small => Some(&la_pill_s),
        "la" => Some(&la_pill),
        "thr" | "range" | "thr_lo" | "range_lo" | "knee" | "gain" if small => Some(&db_pill_s),
        "att" | "rel" if small => Some(&ms_pill_s),
        "key_lo" | "key_hi" if small => Some(&hz_pill_s),
        _ => None,
    };
    let k = key(band, field);
    let resp = controls::param_with(ui, s, host, k, size, format);
    if resp.hovered() && !resp.dragged() {
        let name = host
            .spec()
            .kind
            .param(k)
            .map_or("", |i| frame::tr(i.label_key));
        hint::set_text(ui.ctx(), name, frame::tr(tip_of(field)), Caps::PARAM);
    }
}

/// An LED bound to one of a band's switches, with what it does on the help line.
#[allow(clippy::too_many_arguments)]
fn band_led(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    band: usize,
    field: &str,
    on: bool,
    label: &'static str,
    tip: &'static str,
    width: f32,
) {
    let r = controls::led(ui, s, on, Src::Key(label), frame::tr(label), width);
    if r.hovered() {
        hint::set_text(ui.ctx(), frame::tr(label), frame::tr(tip), Caps::NONE);
    }
    // The band's switch is a parameter: its menu, its mark, the learn mode (review 2 of
    // ADR 0062, #44).
    let learnt = controls::stepped_offer(ui, s, host, &key(band, field), frame::tr(label), &r);
    if r.clicked() && !learnt {
        flip(host, band, field, on);
    }
}

/// The selected band: its switches (the card's lamp is its On), where it sits in the image,
/// what its detector hears, and its cells — the band's colour marks the card, the cells
/// take the device's hue.
#[allow(clippy::too_many_arguments)]
fn band_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    set: &Settings,
) {
    let k = s.k;
    let Some(b) = st.selected.filter(|b| set.bands[*b].used) else {
        // The same card, empty, so the window does not jump when the first band comes.
        controls::card(ui, s, rect, Src::Key("mb.band"), None, |ui, cr| {
            with_brush(ui.painter(), |bb| {
                canvas::line_in(
                    bb,
                    Rect::from_min_size(cr.body.min, vec2(cr.body.width(), controls::ROW * k)),
                    Role::Body,
                    s.t.text_dim,
                    s.tk(),
                    format_args!("{}", frame::tr("mb.hint")),
                );
            });
        });
        return;
    };
    let band = set.bands[b];
    let keyed = key_set(st);
    let marked = s.with_hue(band_ink(b, s));
    let title = st.words.band.get(b).map_or("", String::as_str);
    let mut remove = false;
    let (_, lamp) = controls::card(
        ui,
        &marked,
        rect,
        Src::Str(title),
        Some(band.on),
        |ui, cr| {
            let body = cr.body;
            // The switches.
            let row = Rect::from_min_size(body.min, vec2(body.width(), controls::ROW * k));
            let mut sw = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(row)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            sw.spacing_mut().item_spacing.x = 4.0 * k;
            band_led(
                host,
                &mut sw,
                s,
                b,
                "solo",
                band.solo,
                "fx.mb.solo",
                "mb.solo_tip",
                64.0,
            );
            band_led(
                host,
                &mut sw,
                s,
                b,
                "mute",
                band.mute,
                "fx.mb.mute",
                "mb.mute_tip",
                80.0,
            );
            sw.add_space(8.0 * k);
            let place = band.placement.index();
            if let Some(i) = choice_key(
                &mut sw,
                s,
                host,
                &key(b, "place"),
                place,
                &PLACE_LABELS,
                &PLACE_TIPS,
                frame::tr("fx.mb.place"),
                186.0,
            ) {
                host.set(key(b, "place"), i as f32, None);
            }
            sw.add_space(8.0 * k);
            if let Some(i) = choice_key(
                &mut sw,
                s,
                host,
                &key(b, "key"),
                band.key_ext as usize,
                &KEY_LABELS,
                &KEY_TIPS,
                frame::tr("fx.mb.key"),
                176.0,
            ) {
                if i == 1 && !keyed {
                    host.status(t("mb.key_hint"));
                } else {
                    host.set(key(b, "key"), i as f32, None);
                }
            }
            band_led(
                host,
                &mut sw,
                s,
                b,
                "key_free",
                band.key_free,
                "fx.mb.key_free",
                "mb.key_free_tip",
                104.0,
            );
            let left = sw.available_width() / k;
            if left >= 96.0 {
                sw.add_space(sw.available_width() - 110.0 * k);
                if controls::key(
                    &mut sw,
                    s,
                    Src::Key("mb.remove_band"),
                    frame::tr("mb.remove_band"),
                    110.0,
                    false,
                )
                .clicked()
                {
                    remove = true;
                }
            }
            // The cells, in their groups.
            let cells = Rect::from_min_max(pos2(body.left(), row.bottom() + 6.0 * k), body.max);
            let mut cu = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(cells)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            cu.spacing_mut().item_spacing.x = 4.0 * k;
            let n = if band.key_free {
                CELLS.len() + KEY_CELLS.len()
            } else {
                CELLS.len()
            };
            let groups = if band.key_free { 3 } else { 2 };
            let need = |z: Size| controls::cells_width(&[z; 13][..n]) + groups as f32 * 12.0;
            let size = if need(Size::M) * k <= cells.width() {
                Size::M
            } else {
                Size::S
            };
            for (i, field) in CELLS.iter().chain(KEY_CELLS.iter()).take(n).enumerate() {
                if GROUPS.contains(&i) {
                    let (r, _) =
                        cu.allocate_exact_size(vec2(8.0 * k, cells.height()), Sense::hover());
                    with_brush(cu.painter(), |bb| {
                        band::divider(
                            bb,
                            r.center().x,
                            r.top() + 10.0 * k,
                            r.bottom() - 10.0 * k,
                            s,
                        )
                    });
                }
                band_cell(host, &mut cu, s, b, field, size);
            }
        },
    );
    let learnt = controls::card_lamp_offer(ui, s, host, &key(b, "on"));
    if lamp && !learnt {
        flip(host, b, "on", band.on);
    }
    if remove {
        remove_band(host, st, b);
    }
}

// ---- the transfer curve ---------------------------------------------------------------

/// The selected band's transfer curve, in a card of its own.
#[allow(clippy::too_many_arguments)]
fn transfer_card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    set: &Settings,
    tel: &cantus_engine::Telemetry,
) {
    let disp = (rect.height() / s.k - band::TITLE - 1.0).max(20.0);
    let b = st.selected.filter(|b| set.bands[*b].used);
    let (_, _) = controls::card_with(
        ui,
        s,
        rect,
        Src::Key("mb.card.transfer"),
        None,
        Some(disp),
        |ui, cr| {
            if let (Some(d), Some(b)) = (cr.display, b) {
                transfer_curve(host, st, ui, s, d, b, &set.bands[b], tel);
            }
        },
    );
}

/// The band's two-sided transfer curve: input level across, output up. Drag the upper or
/// the lower threshold point, or the curve beyond either to set its ratio.
#[allow(clippy::too_many_arguments)]
fn transfer_curve(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    b: usize,
    band: &BandSettings,
    tel: &cantus_engine::Telemetry,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let resp = ui.interact(
        rect,
        ui.id().with(("mb-transfer", b)),
        Sense::click_and_drag(),
    );
    let plot = rect.shrink(3.0 * k);
    let lo = LEVEL_LO;
    let x = |d: f32| plot.left() + ((d - lo) / -lo).clamp(0.0, 1.0) * plot.width();
    let y = |d: f32| plot.bottom() - ((d - lo) / -lo).clamp(0.0, 1.0) * plot.height();
    let dx_of = |px: f32| lo + (px - plot.left()) / plot.width().max(1.0) * -lo;
    let dy_of = |py: f32| lo + (plot.bottom() - py) / plot.height().max(1.0) * -lo;
    let color = band_ink(b, s);
    let hi_pt = pos2(x(band.thr_db), y(transfer_db(band.thr_db, band)));
    let lo_pt = pos2(x(band.thr_lo_db), y(transfer_db(band.thr_lo_db, band)));
    with_brush(&ui.painter_at(rect), |bb| {
        let px = bb.px();
        for g in [-12.0f32, -24.0, -36.0, -48.0, -60.0] {
            bb.hline(plot.left(), plot.right(), bb.snap_mid(y(g)), px, ink.grid);
            bb.vline(bb.snap_mid(x(g)), plot.top(), plot.bottom(), px, ink.grid);
        }
        bb.segment(
            pos2(x(lo), y(lo)),
            pos2(x(0.0), y(0.0)),
            px,
            ink.grid_strong,
            false,
        );
        let mut pts = [Pos2::ZERO; 97];
        for (i, p) in pts.iter_mut().enumerate() {
            let d = lo + -lo * i as f32 / 96.0;
            *p = pos2(x(d), y(transfer_db(d, band)));
        }
        bb.polyline(&pts, 2.0 * k, color);
        bb.circle(hi_pt, 5.0 * k, Fill::Solid(color));
        bb.circle_stroke(lo_pt, 5.0 * k, 2.0 * k, color);
        let level = tel[T_LEVEL + b];
        if level > lo {
            bb.circle(
                pos2(x(level), y(level + curve_gain_db(level, band))),
                3.5 * k,
                Fill::Solid(ink.danger),
            );
        }
        canvas::putf(
            bb,
            pos2(plot.left() + 3.0 * k, plot.top() + 2.0 * k),
            Align2::LEFT_TOP,
            Role::Micro,
            ink.text,
            tk,
            Some(ink.plate),
            format_args!("{}  {}", Ratio(band.ratio), Ratio(band.ratio_lo)),
        );
    });

    // Which handle, for the whole drag.
    if resp.drag_started() {
        if let Some(pt) = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos())
        {
            st.transfer = if pt.distance(hi_pt) < 9.0 * k {
                Some(TGrab::Upper)
            } else if pt.distance(lo_pt) < 9.0 * k {
                Some(TGrab::Lower)
            } else if dx_of(pt.x) > band.thr_db {
                Some(TGrab::UpperRatio)
            } else if dx_of(pt.x) < band.thr_lo_db {
                Some(TGrab::LowerRatio)
            } else {
                None
            };
        }
    }
    if let (true, Some(pt), Some(g)) = (resp.dragged(), resp.interact_pointer_pos(), st.transfer) {
        match g {
            TGrab::Upper => {
                let k = key(b, "thr");
                host.set(
                    k,
                    dx_of(pt.x).clamp(band.thr_lo_db.max(-80.0), 0.0),
                    Some(k),
                );
            }
            TGrab::Lower => {
                let k = key(b, "thr_lo");
                host.set(k, dx_of(pt.x).clamp(-90.0, band.thr_db), Some(k));
            }
            TGrab::UpperRatio => {
                // The pointer is a point the curve should pass through above the threshold.
                let over_in = (dx_of(pt.x) - band.thr_db).max(0.5);
                let over_out = dy_of(pt.y) - band.thr_db;
                let r = if over_out <= 0.01 {
                    100.0
                } else {
                    (over_in / over_out).clamp(0.1, 100.0)
                };
                let k = key(b, "ratio");
                host.set(k, r, Some(k));
            }
            TGrab::LowerRatio => {
                let under_in = (band.thr_lo_db - dx_of(pt.x)).max(0.5);
                let under_out = band.thr_lo_db - dy_of(pt.y);
                let r = if under_out <= 0.01 {
                    100.0
                } else {
                    (under_in / under_out).clamp(0.1, 100.0)
                };
                let k = key(b, "ratio_lo");
                host.set(k, r, Some(k));
            }
        }
    }
    if resp.drag_stopped() {
        st.transfer = None;
        host.end_gesture();
    }
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("mb.card.transfer"),
            frame::tr("mb.transfer_tip"),
            Caps::NONE,
        );
    }
}

// ---- the inspector's picture ------------------------------------------------------------

thread_local! {
    /// The picture's words, kept until the count, the mode or the language changes.
    static PREVIEW: std::cell::RefCell<Option<(usize, usize, Locale, String)>> =
        const { std::cell::RefCell::new(None) };
}

/// The inspector's picture: the bands across the spectrum, what each can reach, the
/// crossovers, and the count and mode in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Multiband));
    let ink = Ink::of(&st);
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), space::GRID * 14.0),
        Sense::click(),
    );
    let s = settings(spec);
    let lay = Layout::of(&s.bands);
    let geo = Geo {
        level: rect,
        gain: rect,
        lanes: rect,
        meters: rect,
    };
    let lang = crate::i18n::locale();
    let mode = s.mode as usize;
    PREVIEW.with(|cell| {
        let mut cache = cell.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|c| c.0 != lay.n || c.1 != mode || c.2 != lang)
        {
            let text = if lay.n == 0 {
                t("mb.preview_empty")
            } else {
                tf(
                    "mb.preview",
                    &[("n", &lay.n), ("mode", &t(MODE_KEYS[mode.min(2)]))],
                )
            };
            *cache = Some((lay.n, mode, lang, text));
        }
        let words = cache.as_ref().map_or("", |c| c.3.as_str());
        with_brush(&ui.painter_at(rect), |b| {
            canvas::ground_b(b, rect, &ink, st.hue_tok);
            let px = b.px();
            for pos in 0..lay.n {
                let bnd = lay.order[pos];
                let band = &s.bands[bnd];
                let color = band_ink(bnd, &st);
                if let Some((x0, x1)) = geo.span(&lay, bnd) {
                    let a = if band.can_act() { 0.22 } else { 0.07 };
                    b.rect(
                        Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.bottom())),
                        Fill::Solid(Ink::a(color, a)),
                    );
                    let (cut, boost) = reach(band);
                    let yc = geo.y_gain(band.gain_db);
                    if band.on && (cut > 0.0 || boost > 0.0) {
                        b.rect(
                            Rect::from_min_max(
                                pos2(x0 + 2.0, geo.y_gain(band.gain_db + boost)),
                                pos2(x1 - 2.0, geo.y_gain(band.gain_db - cut)),
                            ),
                            Fill::Solid(Ink::a(color, 0.3)),
                        );
                    }
                    b.segment(pos2(x0 + 2.0, yc), pos2(x1 - 2.0, yc), 1.5, color, false);
                }
                if pos > 0 {
                    let x = b.snap_mid(geo.x(lay.crossovers[pos - 1]));
                    b.vline(x, rect.top(), rect.bottom(), px, ink.dim);
                }
            }
            canvas::line_in(
                b,
                Rect::from_min_size(
                    rect.min + vec2(space::S2, space::S1),
                    vec2(rect.width() - 2.0 * space::S2, 16.0),
                ),
                Role::Label,
                ink.text,
                st.tk(),
                format_args!("{words}"),
            );
            let edge = if resp.hovered() { ink.hue } else { ink.edge };
            canvas::edge_b(b, rect, &Ink { edge, ..ink });
        });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("mb.open_tip"), "", Caps::NONE);
    }
    resp
}

const MODE_KEYS: [&str; 3] = ["mb.mode.0", "mb.mode.1", "mb.mode.2"];

// ---- demo ------------------------------------------------------------------------

/// The settings `--editor multiband` and the screenshot tool open it with: four bands
/// doing four different things — low end held, the body glued, harshness caught with a
/// lookahead, the air lifted from below — so every part of the display has something to
/// show.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    let params: &[(&str, f32)] = &[
        ("b0.used", 1.0),
        ("b0.from", 20.0),
        ("b0.thr", -24.0),
        ("b0.ratio", 3.0),
        ("b0.range", 9.0),
        ("b0.att", 12.0),
        ("b0.rel", 140.0),
        ("b1.used", 1.0),
        ("b1.from", 150.0),
        ("b1.thr", -22.0),
        ("b1.ratio", 2.0),
        ("b1.range", 6.0),
        ("b1.att", 20.0),
        ("b1.rel", 200.0),
        ("b1.gain", 1.5),
        ("b2.used", 1.0),
        ("b2.from", 2_600.0),
        ("b2.thr", -34.0),
        ("b2.ratio", 4.0),
        ("b2.range", 10.0),
        ("b2.att", 1.0),
        ("b2.rel", 70.0),
        ("b2.la", 2.0),
        ("b2.key_free", 1.0),
        ("b2.key_lo", 4_500.0),
        ("b2.key_hi", 12_000.0),
        ("b3.used", 1.0),
        ("b3.from", 9_000.0),
        ("b3.ratio", 1.0),
        ("b3.thr_lo", -46.0),
        ("b3.ratio_lo", 2.5),
        ("b3.range_lo", 8.0),
        ("b3.place", 2.0),
    ];
    let map = params.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    app.apply_fx_params(strip, effect, &map, "cmd.fx_preset");
    app.fx.multiband.start_for(effect);
    app.fx.multiband.selected = Some(2);
}

/// What a playing engine would feed the editor, for screenshots with no engine: a small
/// mix (a kick, a bass line, a chord, bright hats and a sibilant hiss) run through the real
/// Multiplex, a frame at a time, so the history, the meters and the spectrum are what the
/// device would really show.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(spec) = app.project().effect(strip, effect).map(|e| e.spec.clone()) else {
        return;
    };
    app.fx.multiband.start_for(effect);
    let sr = 48_000.0f32;
    let mut dsp = cantus_dsp::multiplex::Multiplex::new(sr);
    dsp.set(&settings(&spec));
    let mut seed = 11u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let frame = 800usize;
    let frames = HISTORY;
    let beat = (sr * 60.0 / 120.0) as usize;
    let mut hp = 0.0f32;
    let mut pairs = Vec::with_capacity(frame * 2 * 24);
    for f in 0..frames {
        let mut l = vec![0.0f32; frame];
        for (i, v) in l.iter_mut().enumerate() {
            let n = f * frame + i;
            let tt = n as f32 / sr;
            let in_beat = n % beat;
            let kick_env = (-(in_beat as f32) / (0.09 * sr)).exp();
            let kick =
                0.9 * kick_env * (std::f32::consts::TAU * (48.0 + 90.0 * kick_env) * tt).sin();
            let bass = 0.22 * (std::f32::consts::TAU * 55.0 * tt).sin().signum() * 0.6;
            let chord = 0.08
                * [220.0f32, 277.2, 329.6, 440.0]
                    .iter()
                    .map(|hz| (std::f32::consts::TAU * hz * tt).sin())
                    .sum::<f32>();
            let w = noise();
            hp = 0.6 * hp + 0.4 * w;
            let hat_env = (-((n % (beat / 2)) as f32) / (0.03 * sr)).exp();
            let hats = 0.35 * hat_env * (w - hp);
            let ess = if (n / (beat * 2)) % 2 == 1 {
                0.12 * (w - hp) * ((std::f32::consts::TAU * 6.5 * tt).sin() * 0.5 + 0.5)
            } else {
                0.0
            };
            *v = kick + bass + chord + hats + ess;
        }
        let src = l.clone();
        let mut r = l.clone();
        dsp.process(&mut l, &mut r, None);
        let t = cantus_engine::fx::multiband::telemetry_of(&dsp);
        let mut g = [0.0f32; MB_BANDS];
        g.copy_from_slice(&t[T_GAIN..T_GAIN + MB_BANDS]);
        app.fx.multiband.feed_history(g);
        app.fx.injected = Some(t);
        if f + 24 >= frames {
            for (a, (bl, br)) in src.iter().zip(l.iter().zip(&r)) {
                pairs.push(*a);
                pairs.push((bl + br) * 0.5);
            }
        }
    }
    app.fx.feed(&pairs);
}
