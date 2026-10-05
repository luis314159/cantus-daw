//! Culmen's editor: the mastering maximizer's own window (ADR 0026), where what you see
//! is what you drag.
//!
//! FL's Maximus shows its bands' curves and a level history; Ozone's Maximizer learns a
//! gain and prints loudness; Pro-L shows the reduction over the waveform; Pro-MB lets you
//! drag the bands. None does all of it in one place, and each hides something
//! (`research/fabfilter/pro-l.md` §6, `pro-mb.md` §7). So here:
//!
//! - **Gain over time** is one picture of every stage: the output's peaks over the
//!   input's, the brickwall's reduction from the top, and **a lane per band** showing what
//!   that band cut or lifted, at the same moment. The ceiling is a line you drag.
//! - **The bands are drawn on the spectrum** they share: drag a split, click a band to
//!   edit it, double-click it for solo. The full band sits beside them.
//! - **The curve is the control**: the selected band's two-sided curve, its threshold
//!   points and both slopes dragged directly, the band's level now as a dot on it.
//! - **Loudness is always on screen** — momentary, short-term, integrated, range, true
//!   peak, PLR, and the input's — against a target chosen from the usual deliveries, with
//!   the verdict in words and a shape.
//! - **Learn drive listens and corrects** until the output reads the target (closed loop,
//!   not Ozone's single guess), as one undo step.
//!
//! Every drag and knob turn is one step of the project's undo.

use super::controls::{self, Size};
use super::frame;
use super::host::{DawHost, FxHost};
use super::FactoryPreset;
use crate::app::CantusApp;
use crate::i18n::tf;
use crate::kit::band;
use crate::kit::shell::Grid;
use crate::kit::text::Src;
use crate::kit::Style;
use cantus_core::fx::maximizer::{band_key, field_index, FULL};
use cantus_core::{EffectId, EffectKind, StripId};
use cantus_dsp::culmen::telemetry::{self, Column};
use cantus_dsp::culmen::MaxSettings;
use cantus_dsp::terminus::loudness::{self, ColumnLoudness};
use egui::Rect;
use std::collections::VecDeque;

#[path = "maximizer_bands.rs"]
mod bands;
#[path = "maximizer_cards.rs"]
mod cards;
#[path = "maximizer_display.rs"]
mod display;
#[path = "maximizer_learn.rs"]
pub mod learn;
#[path = "maximizer_meters.rs"]
mod meters;

pub use learn::{Learn, Outcome};

/// Columns of history kept: at 10 ms each, ten seconds.
pub const HISTORY: usize = 1_000;

/// The static key of one field of one band (`mid.thr`): no formatting.
pub fn key(band: usize, field: &str) -> &'static str {
    band_key(band, field_index(field).unwrap_or(0))
}

/// The bands' names on screen, by index (low, mid, high, full).
pub const BAND_KEYS: [&str; 4] = ["max.band.lo", "max.band.mid", "max.band.hi", "max.band.all"];
/// The bands' group names (a card's title).
pub const GROUP_KEYS: [&str; 4] = [
    "max.group.lo",
    "max.group.mid",
    "max.group.hi",
    "max.group.all",
];

// ---- delivery targets -------------------------------------------------------------------

/// A delivery target: its loudness and ceiling, how close counts, and its rule.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Target {
    pub key: &'static str,
    pub rule: &'static str,
    pub lufs: f32,
    pub ceiling: f32,
    pub tolerance: f32,
}

/// The usual jobs: streaming (Spotify, YouTube, Tidal normalise to −14), Apple Music
/// (−16), a loud CD or download, club use, EBU R128 broadcast.
pub const TARGETS: [Target; 5] = [
    Target {
        key: "max.target.stream",
        rule: "max.target.stream_rule",
        lufs: -14.0,
        ceiling: -1.0,
        tolerance: 1.0,
    },
    Target {
        key: "max.target.apple",
        rule: "max.target.apple_rule",
        lufs: -16.0,
        ceiling: -1.0,
        tolerance: 1.0,
    },
    Target {
        key: "max.target.cd",
        rule: "max.target.cd_rule",
        lufs: -9.0,
        ceiling: -0.3,
        tolerance: 1.0,
    },
    Target {
        key: "max.target.club",
        rule: "max.target.club_rule",
        lufs: -7.0,
        ceiling: -0.1,
        tolerance: 1.0,
    },
    Target {
        key: "max.target.ebu",
        rule: "max.target.ebu_rule",
        lufs: -23.0,
        ceiling: -1.0,
        tolerance: 0.5,
    },
];

/// The target a (loudness, ceiling) pair is, if any.
pub fn target_of(lufs: f32, ceiling: f32) -> Option<usize> {
    TARGETS
        .iter()
        .position(|t| (t.lufs - lufs).abs() < 0.05 && (t.ceiling - ceiling).abs() < 0.05)
}

// ---- presets --------------------------------------------------------------------------

/// Factory presets: a label key and the parameters it sets; what a preset does not name
/// goes back to its default.
pub const FACTORY_PRESETS: &[FactoryPreset] = &[
    (
        "preset.max.transparent",
        &[
            ("gain", 3.0),
            ("all.thr", -18.0),
            ("all.ratio", 1.5),
            ("all.knee", 12.0),
            ("all.att", 30.0),
            ("all.rel", 300.0),
        ],
    ),
    (
        "preset.max.streaming",
        &[
            ("gain", 6.0),
            ("target", -14.0),
            ("ceiling", -1.0),
            ("lo.thr", -20.0),
            ("lo.ratio", 2.0),
            ("lo.att", 30.0),
            ("lo.rel", 200.0),
            ("all.thr", -16.0),
            ("all.ratio", 1.4),
            ("all.knee", 10.0),
        ],
    ),
    (
        "preset.max.loud",
        &[
            ("gain", 10.0),
            ("target", -8.0),
            ("ceiling", -0.3),
            ("style", 2.0),
            ("clip", 0.4),
            ("link_t", 0.6),
            ("mid.thr", -22.0),
            ("mid.ratio", 1.5),
            ("all.thr", -20.0),
            ("all.ratio", 2.0),
            ("all.thr_lo", -40.0),
            ("all.ratio_lo", 1.5),
        ],
    ),
    (
        "preset.max.club",
        &[
            ("gain", 12.0),
            ("target", -7.0),
            ("ceiling", -0.1),
            ("style", 1.0),
            ("clip", 0.5),
            ("lo.thr", -16.0),
            ("lo.ratio", 2.5),
            ("lo.place", 1.0),
            ("hi.thr", -24.0),
            ("hi.ratio", 1.5),
        ],
    ),
    (
        "preset.max.cd",
        &[
            ("gain", 8.0),
            ("target", -9.0),
            ("ceiling", -0.3),
            ("all.thr", -18.0),
            ("all.ratio", 1.5),
            ("dither", 5.0),
            ("shape", 2.0),
        ],
    ),
    (
        "preset.max.lowend",
        &[
            ("gain", 4.0),
            ("x1", 120.0),
            ("lo.thr", -24.0),
            ("lo.ratio", 3.0),
            ("lo.att", 20.0),
            ("lo.rel", 120.0),
            ("lo.place", 1.0),
        ],
    ),
    (
        "preset.max.upward",
        &[
            ("gain", 4.0),
            ("mid.thr_lo", -40.0),
            ("mid.ratio_lo", 1.6),
            ("all.thr_lo", -35.0),
            ("all.ratio_lo", 2.0),
            ("all.range_lo", 8.0),
        ],
    ),
    (
        "preset.max.ms",
        &[
            ("gain", 4.0),
            ("lo.thr", -40.0),
            ("lo.ratio", 4.0),
            ("lo.place", 2.0),
            ("hi.thr", -30.0),
            ("hi.ratio", 0.8),
            ("hi.range", 3.0),
            ("hi.place", 2.0),
        ],
    ),
    (
        "preset.max.broadcast",
        &[
            ("target", -23.0),
            ("ceiling", -1.0),
            ("style", 4.0),
            ("all.thr", -24.0),
            ("all.ratio", 2.0),
        ],
    ),
    (
        "preset.max.maximus",
        &[
            ("gain", 6.0),
            ("style", 1.0),
            ("xmode", 1.0),
            ("lo.thr", -18.0),
            ("lo.ratio", 2.5),
            ("mid.thr", -16.0),
            ("mid.ratio", 2.0),
            ("hi.thr", -20.0),
            ("hi.ratio", 2.5),
            ("all.thr", -12.0),
            ("all.ratio", 1.5),
        ],
    ),
];

// ---- the editor's memory ----------------------------------------------------------------

/// What the editor remembers between frames.
#[derive(Debug)]
pub struct State {
    /// The effect the history belongs to.
    pub effect: Option<EffectId>,
    /// Ten-millisecond columns, oldest first.
    pub history: VecDeque<Column>,
    last_seq: Option<f32>,
    /// The band the curve and the band card show: 0 low … 3 full.
    pub selected: usize,
    /// The output's loudness and the input's, since the last reset.
    pub loud: ColumnLoudness,
    pub loud_in: ColumnLoudness,
    /// The largest output true peak since the last reset, dBTP.
    pub max_tp: f32,
    /// Learning the drive, when under way, and what the last session came to.
    pub learn: Option<Learn>,
    pub learned: Option<Outcome>,
    /// What is being dragged.
    pub(crate) drag: Option<bands::Grab>,
    /// Where the last frame drew its displays — published so tests can aim a drag.
    pub geometry: Geometry,
    pub chrome: frame::Chrome,
    chips: frame::ChipText,
    /// Words formatted when what they say changes.
    words: meters::Words,
}

impl Default for State {
    fn default() -> Self {
        State {
            effect: None,
            history: VecDeque::with_capacity(HISTORY),
            last_seq: None,
            selected: FULL,
            loud: ColumnLoudness::new(),
            loud_in: ColumnLoudness::new(),
            max_tp: f32::NEG_INFINITY,
            learn: None,
            learned: None,
            drag: None,
            geometry: Geometry::default(),
            chrome: frame::Chrome::default(),
            chips: frame::ChipText::default(),
            words: meters::Words::default(),
        }
    }
}

/// The displays as the last frame laid them out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geometry {
    /// The level plot of the history: [`display::LOW_DB`] at the bottom, 0 dB at the top.
    pub history: Rect,
    /// The band strip: 20 Hz to 20 kHz across, and the full band's column at its right.
    pub strip: Rect,
    pub full: Rect,
    /// The curve: [`bands::CURVE_LO`] … 0 dB on both axes.
    pub curve: Rect,
}

impl Default for Geometry {
    fn default() -> Self {
        Geometry {
            history: Rect::NOTHING,
            strip: Rect::NOTHING,
            full: Rect::NOTHING,
            curve: Rect::NOTHING,
        }
    }
}
impl Geometry {
    /// The height of a level on the history.
    pub fn history_y(&self, db: f32) -> f32 {
        display::y_of(db, self.history)
    }

    /// Where a frequency sits on the band strip.
    pub fn strip_x(&self, hz: f32) -> f32 {
        bands::x_of(hz, self.strip)
    }

    /// Where a level pair lands on the curve.
    pub fn curve_pos(&self, input_db: f32, output_db: f32) -> egui::Pos2 {
        bands::curve_pos(self.curve, input_db, output_db)
    }
}

impl State {
    /// Take the columns the telemetry brought since the last frame: into the history, the
    /// loudness, and the learning.
    pub fn ingest(&mut self, t: &[f32; 64]) {
        let new = match self.last_seq {
            Some(last) => telemetry::since(t[telemetry::SEQ], last),
            None => t[telemetry::SEQ].max(0.0) as u32,
        }
        .min(telemetry::COLUMNS as u32);
        // A counter that went back is a new stream (the engine was reset).
        self.last_seq = Some(t[telemetry::SEQ]);
        let seq = t[telemetry::SEQ].max(0.0) as u32;
        for j in (0..new).rev() {
            let index = (seq + telemetry::WRAP - 1 - j) % telemetry::WRAP;
            let c = Column::read(t, index);
            if self.history.len() == HISTORY {
                self.history.pop_front();
            }
            self.history.push_back(c);
            self.loud.push(c.energy_out as f64);
            self.loud_in.push(c.energy_in as f64);
            self.max_tp = self.max_tp.max(c.output_db);
            if let Some(l) = self.learn.as_mut() {
                l.column(c.energy_out as f64);
            }
        }
    }

    /// Start the loudness over.
    pub fn reset_loudness(&mut self) {
        self.loud.reset();
        self.loud_in.reset();
        self.max_tp = f32::NEG_INFINITY;
    }

    /// The integrated output loudness, if measured.
    pub fn integrated(&self) -> Option<f64> {
        self.loud.integrated()
    }
}

/// `lufs` of an energy, re-exported for the learning and the meters.
pub fn lufs(energy: f64) -> f64 {
    loudness::lufs(energy)
}

// ---- the window -------------------------------------------------------------------------

/// Culmen's editor in Cantus's own window: the host is the project.
pub fn framed(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
) -> frame::Out {
    let mut st = std::mem::take(&mut app.fx.maximizer);
    if st.effect != Some(effect) {
        st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
    st.chrome.reset_for(effect.0);
    let out = editor(
        &mut DawHost::new(app, strip, effect, "maximizer"),
        &mut st,
        ui,
    );
    app.fx.maximizer = st;
    out
}

/// Culmen's editor on any host (ADR 0020): the device frame and the maximizer inside it.
pub fn editor(host: &mut dyn FxHost, st: &mut State, ui: &mut egui::Ui) -> frame::Out {
    let tel = host.telemetry();
    st.ingest(&tel);
    learn::step(host, st);
    let ms = cantus_engine::fx::maximizer::settings(host.spec());
    let sr = if tel[telemetry::RATE] > 0.0 {
        tel[telemetry::RATE]
    } else {
        host.sample_rate()
    };
    let latency = if tel[telemetry::LATENCY] > 0.0 {
        tel[telemetry::LATENCY]
    } else {
        cantus_dsp::terminus::latency_of(&ms.brickwall(), sr) as f32
    };
    let mut chips = std::mem::take(&mut st.chips);
    let chip = [chips.latency(latency, sr)];
    let meter = st.history.back().map(|c| frame::Levels {
        level: [c.output_db; 2],
        hold: [st.max_tp; 2],
    });
    let look = frame::Look {
        kind: EffectKind::Maximizer,
        chips: &chip,
        meter,
        more: &[],
    };
    let mut chrome = std::mem::take(&mut st.chrome);
    let out = frame::show(ui, host, &mut chrome, &look, |ui, s, body, host| {
        body_ui(host, st, ui, s, body, &tel, &ms)
    });
    st.chrome = chrome;
    st.chips = chips;
    out
}

/// A row of the grid as an array.
fn row_of<const N: usize>(g: &mut Grid, h: f32, fracs: &[f32; N]) -> [Rect; N] {
    let mut it = g.row(h, fracs);
    std::array::from_fn(|_| it.next().unwrap_or(Rect::NOTHING))
}

/// Culmen inside the frame: gain over time and the loudness on top; the bands, the curve
/// and the output in the middle; the selected band's cells and the brickwall below.
fn body_ui(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    body: Rect,
    tel: &[f32; 64],
    ms: &MaxSettings,
) {
    let k = s.k;
    let mut g = Grid::new(body, s);
    let area = g.rest();
    let gap = s.d(4.0);
    let row3 = controls::card_height(Size::L) * k;
    let row2 = (band::TITLE + 172.0) * k;
    let row1 = (area.height() - row2 - row3 - 2.0 * gap).max(180.0 * k);
    let w = area.width();
    let disp = |r: Rect| (r.height() / k - band::TITLE - 1.0).max(20.0);

    // Row 1: gain over time, and the loudness beside it.
    let lw = 300.0 * k;
    let [hist_r, loud_r] = row_of(&mut g, row1, &[(w - lw - gap) / (w - gap), lw / (w - gap)]);
    let _ = controls::card_with(
        ui,
        s,
        hist_r,
        Src::Key("max.card.history"),
        None,
        Some(disp(hist_r)),
        |ui, cr| {
            if let Some(d) = cr.display {
                display::history(host, st, ui, s, ms, tel, d);
            }
        },
    );
    let so = s.function(crate::kit::Function::Output);
    let _ = controls::card(
        ui,
        &so,
        loud_r,
        Src::Key("max.card.loudness"),
        None,
        |ui, cr| meters::loudness(host, st, ui, &so, cr.body, ms),
    );

    // Row 2: the bands on the spectrum, the selected band's curve, the output.
    let cw = 300.0 * k;
    let ow = (2.0 * cards::COLUMN_W + 4.0 + 16.0) * k;
    let bw = w - cw - ow - 2.0 * gap;
    let ws = w - 2.0 * gap;
    let [bands_r, curve_r, out_r] = row_of(&mut g, row2, &[bw / ws, cw / ws, ow / ws]);
    let _ = controls::card_with(
        ui,
        s,
        bands_r,
        Src::Key("max.card.bands"),
        None,
        Some(disp(bands_r) - controls::ROW - 8.0),
        |ui, cr| {
            if let Some(d) = cr.display {
                bands::strip(host, st, ui, s, ms, tel, d);
            }
            bands::strip_controls(host, ui, s, cr.body.width() / k);
        },
    );
    let sel = st.selected.min(FULL);
    let (_, clicked) = controls::card_with(
        ui,
        s,
        curve_r,
        Src::Key(GROUP_KEYS[sel]),
        Some(ms.bands[sel].on),
        Some(disp(curve_r)),
        |ui, cr| {
            if let Some(d) = cr.display {
                bands::curve(host, st, ui, s, ms, tel, d);
            }
        },
    );
    if clicked {
        host.set(
            key(sel, "on"),
            if ms.bands[sel].on { 0.0 } else { 1.0 },
            None,
        );
    }
    cards::output_card(host, ui, &so, out_r);

    // Row 3: the selected band's cells; the brickwall's.
    let bw3 = cards::band_card_width() * k;
    let ww = w - bw3 - gap;
    let [band_r, wall_r] = row_of(&mut g, row3, &[bw3 / (w - gap), ww / (w - gap)]);
    cards::band_card(host, ui, s, band_r, sel, ms);
    cards::wall_card(host, ui, s, wall_r);
}

/// The words of a learning that is under way or finished, for the loudness card.
pub fn learn_words(st: &State) -> Option<String> {
    if let Some(l) = &st.learn {
        return Some(match l.measured() {
            Some(v) => tf(
                "max.learning",
                &[("n", &(l.steps() + 1)), ("lufs", &format!("{v:.1} LUFS"))],
            ),
            None => crate::i18n::t("max.learn.waiting"),
        });
    }
    st.learned.as_ref().map(|o| o.words())
}

#[path = "maximizer_demo.rs"]
mod demo;
pub use demo::{demo_feed, demo_setup, demo_signal, preview};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_preset_names_real_parameters_in_range() {
        for (name, values) in FACTORY_PRESETS {
            assert!(name.starts_with("preset.max."));
            for (k, v) in *values {
                let info = EffectKind::Maximizer
                    .param(k)
                    .unwrap_or_else(|| panic!("{name}: {k} is not a parameter"));
                assert!(info.min <= *v && *v <= info.max, "{name}: {k} = {v}");
            }
        }
    }

    #[test]
    fn targets_are_found_back_from_their_numbers() {
        for (i, t) in TARGETS.iter().enumerate() {
            assert_eq!(target_of(t.lufs, t.ceiling), Some(i));
        }
        assert_eq!(target_of(-12.0, -1.0), None);
        let _ = cantus_core::EffectSpec::new(EffectKind::Maximizer);
    }
}
