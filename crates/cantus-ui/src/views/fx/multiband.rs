//! Multiplex's editor. Owned by Multiplex; nothing else edits this file.
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
//! - **Each band's gain change is drawn live and as history**: the gold curve is the
//!   device's response right now (from the same filters the audio runs), a ghost holds the
//!   deepest of the last seconds, and under it every band has a lane of its gain over
//!   time. Input and output meters sit beside the display — the benchmark has no input
//!   meter.
//! - **Real units, two sides**: attack and release in milliseconds, ratios as numbers
//!   (`4.0:1` squeezes, `1:2.0` stretches), a lower threshold for upward compression or
//!   expansion in the same band, and a transfer curve for the selected band whose
//!   thresholds and ratios are dragged directly.
//! - **The band panel is docked under the display** and never covers the analyser; the
//!   selected band's numbers are always on screen.
//! - **The key is chosen here**, from the desk's strips, with no send to set up, and each
//!   band says whether it listens to it; each band's detector can hear its own range or a
//!   free one.
//!
//! Every drag and knob turn is one step of the project's undo.

use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::Knob;
use cantus_core::fx::multiband::{mb_key, MB_BANDS, MB_BAND_FIELDS};
use cantus_core::{Command, EffectId, EffectSlot, EffectSpec, StripId};
use cantus_dsp::multiplex::{
    gain_db as curve_gain_db, response_part_db, transfer_db, BandSettings, Layout, Listen, Mode,
    Placement, Settings, Slope,
};
use cantus_dsp::SpectrumAnalyser;
use cantus_engine::fx::multiband::{
    settings, T_GAIN, T_INPUT, T_LATENCY, T_LEVEL, T_OUTPUT, T_RATE, T_SPLIT,
};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind};
use std::collections::VecDeque;

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
const DOT_R: f32 = 7.0;

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
    analyser: SpectrumAnalyser,
    scratch: Vec<f32>,
    pre: Vec<f32>,
    post: Vec<f32>,
    last: Option<std::time::Instant>,
    /// Per frame: each band's dynamic gain, dB, by band number.
    history: VecDeque<[f32; MB_BANDS]>,
    /// Meter levels as drawn (dB, with the house fall): in L, in R, out L, out R.
    meters: [f32; 4],
    drag: Option<Drag>,
    /// The display's geometry in the last frame, for hit-testing and for tests.
    pub geo: Option<Geo>,
}

impl Default for State {
    fn default() -> Self {
        State {
            selected: None,
            analyser: SpectrumAnalyser::new(4096),
            scratch: Vec::new(),
            pre: vec![-120.0; POINTS],
            post: vec![-120.0; POINTS],
            last: None,
            history: VecDeque::with_capacity(HISTORY),
            meters: [-120.0; 4],
            drag: None,
            geo: None,
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
    // Blue, green, purple, red first: far apart in hue, and none of them the gold of
    // the response curve.
    const ORDER: [usize; 8] = [1, 3, 2, 4, 7, 6, 0, 5];
    theme::track_color(palette[ORDER[band % 8] % palette.len()])
}

fn hz_label(hz: f32) -> String {
    crate::widgets::knob::readout(hz, "Hz")
}

/// `4.0:1` squeezes, `1:2.0` stretches, `∞:1` limits.
pub fn ratio_label(r: f32) -> String {
    if r >= 99.0 {
        "\u{221e}:1".to_string()
    } else if r >= 1.0 {
        format!("{r:.1}:1")
    } else {
        format!("1:{:.1}", 1.0 / r.max(0.01))
    }
}

fn note_label(hz: f32) -> String {
    let midi = 69.0 + 12.0 * (hz / 440.0).log2();
    let n = midi.round();
    let cents = ((midi - n) * 100.0).round() as i32;
    let name = cantus_core::pitch_name(n.clamp(0.0, 127.0) as u8);
    if cents == 0 {
        name
    } else {
        format!("{name} {cents:+}")
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

// ---- commands ---------------------------------------------------------------------

impl CantusApp {
    fn mb_set(&mut self, strip: StripId, effect: EffectId, key: &str, v: f32, gesture: bool) {
        self.set_fx_param(strip, effect, key, v, gesture);
    }

    /// Split the band under `hz` there. The new band takes the part above `hz` and starts
    /// as a copy of the band it split, so the split alone changes nothing but the detector.
    /// On an empty Multiplex this makes the first two bands. One undo step. Returns the new
    /// band's number.
    pub fn mb_split_at(&mut self, strip: StripId, effect: EffectId, hz: f32) -> Option<usize> {
        let spec = self.project().strip(strip)?.effect(effect)?.spec.clone();
        let s = settings(&spec);
        let lay = Layout::of(&s.bands);
        let hz = hz.clamp(LO_HZ * 2f32.powf(MIN_OCT), HI_HZ);
        let set = |b: usize, f: &str, v: f32| Command::SetEffectParam {
            strip,
            effect,
            key: mb_key(b, f),
            value: v,
        };
        let mut commands = Vec::new();
        let new = if lay.n == 0 {
            commands.push(set(0, "used", 1.0));
            commands.push(set(0, "from", LO_HZ));
            commands.push(set(1, "used", 1.0));
            commands.push(set(1, "from", hz));
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
            commands.push(set(new, "used", 1.0));
            commands.push(set(new, "from", hz));
            for (field, ..) in MB_BAND_FIELDS
                .iter()
                .filter(|f| !matches!(f.0, "used" | "from" | "solo" | "mute"))
            {
                let key = mb_key(old, field);
                if let Some(v) = spec.params.get(&key) {
                    commands.push(set(new, field, *v));
                }
            }
            new
        };
        self.exec(Command::Batch {
            label_key: "cmd.mb_band_add".into(),
            commands,
        });
        self.fx.multiband.selected = Some(new);
        Some(new)
    }

    /// Remove a band: every field it stored goes, so the number is free, and its range
    /// goes to its neighbour. One undo step.
    pub fn mb_remove_band(&mut self, strip: StripId, effect: EffectId, band: usize) {
        let commands: Vec<Command> = MB_BAND_FIELDS
            .iter()
            .map(|f| Command::ClearEffectParam {
                strip,
                effect,
                key: mb_key(band, f.0),
            })
            .collect();
        self.exec(Command::Batch {
            label_key: "cmd.mb_band_remove".into(),
            commands,
        });
        if self.fx.multiband.selected == Some(band) {
            self.fx.multiband.selected = None;
        }
    }

    /// Start from `n` evenly musical bands (2, 3 or 4). One undo step.
    pub fn mb_quick_bands(&mut self, strip: StripId, effect: EffectId, n: usize) {
        let froms: &[f32] = match n {
            2 => &[LO_HZ, 250.0],
            3 => &[LO_HZ, 200.0, 2_500.0],
            _ => &[LO_HZ, 120.0, 1_000.0, 6_000.0],
        };
        let mut commands = Vec::new();
        for (b, from) in froms.iter().enumerate() {
            for (f, v) in [("used", 1.0), ("from", *from)] {
                commands.push(Command::SetEffectParam {
                    strip,
                    effect,
                    key: mb_key(b, f),
                    value: v,
                });
            }
        }
        self.exec(Command::Batch {
            label_key: "cmd.mb_band_add".into(),
            commands,
        });
        self.fx.multiband.selected = Some(0);
    }
}

// ---- the window's body ----------------------------------------------------------

pub fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let s = settings(&slot.spec);
    let lay = Layout::of(&s.bands);
    let tel = app.fx_telemetry();
    // The selection follows the bands: never a band that is gone.
    let ed = &mut app.fx.multiband;
    if ed.selected.is_none_or(|b| !s.bands[b].used) {
        ed.selected = (lay.n > 0).then(|| lay.order[0]);
    }
    // One column of history per frame.
    let mut gains = [0.0f32; MB_BANDS];
    gains.copy_from_slice(&tel[T_GAIN..T_GAIN + MB_BANDS]);
    ed.feed_history(gains);
    // The spectrum, from the analyser tap's most recent pairs.
    let sr = if tel[T_RATE] > 0.0 {
        tel[T_RATE]
    } else {
        48_000.0
    };
    let now = std::time::Instant::now();
    let dt = ed
        .last
        .map(|l| now.duration_since(l).as_secs_f32())
        .unwrap_or(0.0)
        .min(0.25);
    ed.last = Some(now);
    let n_pairs = app.fx.pairs.len().min(ed.analyser.size());
    if n_pairs == ed.analyser.size() {
        ed.scratch.clear();
        let skip = app.fx.pairs.len() - n_pairs;
        for (a, b) in app.fx.pairs.iter().skip(skip) {
            ed.scratch.push(*a);
            ed.scratch.push(*b);
        }
        ed.analyser.push(&ed.scratch);
        ed.analyser.compute();
        let fall = SPECTRUM_FALL * dt;
        for i in 0..POINTS {
            let hz = point_hz(i);
            let a = ed.analyser.level_db(0, sr, hz, 1.0 / 6.0);
            let b = ed.analyser.level_db(1, sr, hz, 1.0 / 6.0);
            ed.pre[i] = a.max(ed.pre[i] - fall);
            ed.post[i] = b.max(ed.post[i] - fall);
        }
    }
    // Meters with the house ballistics: instant rise, a steady fall.
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
        ed.meters[i] = db.max(ed.meters[i] - METER_FALL * dt);
    }

    toolbar(app, ui, strip, effect, slot, &s, &tel);
    ui.add_space(space::S1);
    let panel_h = space::GRID * 38.0;
    let h = (ui.available_height() - panel_h).max(space::GRID * 64.0);
    display(app, ui, strip, effect, &s, &lay, &tel, sr, h);
    ui.add_space(space::S1);
    band_panel(app, ui, strip, effect, slot, &s, &lay, &tel);
}

fn point_hz(i: usize) -> f32 {
    LO_HZ * (HI_HZ / LO_HZ).powf(i as f32 / (POINTS - 1) as f32)
}

fn caption(ui: &mut egui::Ui, text: String) {
    ui.label(
        RichText::new(text)
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
}

/// One row: the mode, the slope, what to hear, the sidechain, and the latency.
fn toolbar(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &Settings,
    tel: &cantus_engine::Telemetry,
) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        caption(ui, t("fx.mb.mode"));
        for (i, mode) in Mode::ALL.into_iter().enumerate() {
            if ui
                .selectable_label(s.mode == mode, t(&format!("mb.mode.{i}")))
                .on_hover_text(t(&format!("mb.mode.{i}_tip")))
                .clicked()
            {
                app.mb_set(strip, effect, "mode", i as f32, false);
            }
        }
        ui.separator();
        caption(ui, t("fx.mb.slope"));
        for slope in Slope::ALL {
            if ui
                .selectable_label(s.slope == slope, format!("{:.0}", slope.db()))
                .on_hover_text(t("mb.slope_tip"))
                .clicked()
            {
                app.mb_set(strip, effect, "slope", slope.db(), false);
            }
        }
        ui.separator();
        caption(ui, t("fx.mb.listen"));
        for (i, listen) in [Listen::Output, Listen::Delta, Listen::Key]
            .into_iter()
            .enumerate()
        {
            if ui
                .selectable_label(s.listen == listen, t(&format!("mb.listen.{i}")))
                .on_hover_text(t(&format!("mb.listen.{i}_tip")))
                .clicked()
            {
                app.mb_set(strip, effect, "listen", i as f32, false);
            }
        }
        ui.separator();
        caption(ui, t("mb.key"));
        let current = slot.sidechain;
        let label = match current {
            Some(k) => app.capture_label(cantus_engine::CaptureTarget::Strip(k)),
            None => t("mb.key_none"),
        };
        egui::ComboBox::from_id_salt(("mb-key", effect.0))
            .selected_text(label)
            .width(space::GRID * 34.0)
            .show_ui(ui, |ui| {
                if ui
                    .selectable_label(current.is_none(), t("mb.key_none"))
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
            .on_hover_text(if current.is_some() {
                t("mb.key_tip")
            } else {
                format!("{}\n{}", t("mb.key_tip"), t("mb.key_hint"))
            });
        if current.is_some() {
            let keyed = s.bands.iter().filter(|b| b.used && b.key_ext).count();
            caption(ui, tf("mb.key_bands", &[("n", &keyed)]));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let latency = tel[T_LATENCY];
            let sr = if tel[T_RATE] > 0.0 {
                tel[T_RATE]
            } else {
                48_000.0
            };
            // The latency, shown whether or not it is zero: a delay that appears silently
            // is the complaint the research heard most.
            let text = if latency > 0.0 {
                tf(
                    "mb.latency",
                    &[
                        ("ms", &format!("{:.1}", latency / sr * 1_000.0)),
                        ("n", &format!("{latency:.0}")),
                    ],
                )
            } else {
                t("mb.latency_none")
            };
            ui.label(
                RichText::new(text)
                    .text_style(design::caption())
                    .color(if latency > 0.0 {
                        pal.gold
                    } else {
                        pal.text_dim
                    }),
            )
            .on_hover_text(t("mb.latency_tip"));
            if tel[T_SPLIT] > 0.0 && s.mode == Mode::Dynamic {
                ui.label(
                    RichText::new(t("mb.via_split"))
                        .text_style(design::caption())
                        .color(pal.gold),
                )
                .on_hover_text(t("mb.via_split_tip"));
            }
        });
    });
}

// ---- the display ------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &Settings,
    lay: &Layout,
    tel: &cantus_engine::Telemetry,
    sr: f32,
    height: f32,
) {
    let pal = theme::p();
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    // Carve the rect: the meters (and the output) on the right; level, gain and the
    // history lanes stacked on the left, sharing one frequency axis.
    let meters_w = space::GRID * 14.0;
    let lane_h = space::GRID * 4.0;
    // Tall enough for the output knob beside the lanes, whatever the band count.
    let lanes_h = (lane_h * lay.n.max(2) as f32)
        .max(space::GRID * 13.0)
        .min(height * 0.26);
    let gap = space::S2;
    // The frequency axis has a strip of its own under the gain pane, clear of both grids.
    let axis_h = space::S3;
    let rest = height - lanes_h - gap - axis_h;
    let gain_h = (rest * 0.34).round();
    // A gutter on the left carries each pane's scale, so no label sits on a band.
    let gutter = space::GRID * 7.0;
    let main = Rect::from_min_max(
        pos2(rect.left() + gutter, rect.top()),
        pos2(rect.right() - meters_w - space::S2, rect.bottom()),
    );
    let level = Rect::from_min_size(main.min, vec2(main.width(), rest - gain_h));
    let gain = Rect::from_min_size(
        pos2(main.left(), level.bottom() + gap),
        vec2(main.width(), gain_h),
    );
    let lanes = Rect::from_min_max(
        pos2(rect.left(), gain.bottom() + axis_h),
        pos2(main.right(), rect.bottom()),
    );
    let meters = Rect::from_min_max(
        pos2(main.right() + space::S2, rect.top()),
        pos2(rect.right(), gain.bottom()),
    );
    let geo = Geo {
        level,
        gain,
        lanes,
        meters,
    };
    app.fx.multiband.geo = Some(geo);
    let p = ui.painter_at(rect);
    let selected = app.fx.multiband.selected;
    // Hit-test where the press began: by the time egui calls it a drag the pointer has
    // already moved a few pixels off what it grabbed.
    let pointer = if resp.drag_started() {
        ui.input(|i| i.pointer.press_origin()).or(resp.hover_pos())
    } else {
        resp.hover_pos()
    };

    for r in [level, gain, lanes] {
        p.rect_filled(r, pal.corner_radius, theme::recess(&pal));
    }

    // ---- band columns: the band's colour, stronger when selected ----
    for pos in 0..lay.n {
        let b = lay.order[pos];
        let band = &s.bands[b];
        let Some((x0, x1)) = geo.span(lay, b) else {
            continue;
        };
        let a = if selected == Some(b) { 0.15 } else { 0.06 };
        let a = if !band.on || band.mute { a * 0.4 } else { a };
        for r in [level, gain] {
            let col = Rect::from_min_max(pos2(x0, r.top()), pos2(x1, r.bottom()));
            p.rect_filled(col, 0.0, band_color(b).gamma_multiply(a));
        }
    }

    // ---- grids: decades across both panes, dB lines on each pane's own scale ----
    for decade in [10.0f32, 100.0, 1_000.0, 10_000.0] {
        for m in [1.0, 2.0, 5.0] {
            let hz = decade * m;
            if !(LO_HZ..=HI_HZ).contains(&hz) {
                continue;
            }
            let strong = m == 1.0;
            let stroke = Stroke::new(1.0, if strong { pal.grid_strong } else { pal.grid });
            for r in [level, gain] {
                p.vline(geo.x(hz), r.y_range(), stroke);
            }
            if strong || m == 5.0 {
                p.text(
                    pos2(geo.x(hz), gain.bottom() + 1.0),
                    Align2::CENTER_TOP,
                    hz_label(hz),
                    design::paint::ruler(),
                    pal.text_dim,
                );
            }
        }
    }
    let mut db = -12.0f32;
    while db > LEVEL_LO {
        let y = geo.y_level(db);
        p.hline(level.x_range(), y, Stroke::new(1.0, pal.grid));
        p.text(
            pos2(level.left() - 4.0, y),
            Align2::RIGHT_CENTER,
            format!("{db:.0}"),
            design::paint::ruler(),
            pal.text_dim,
        );
        db -= 12.0;
    }
    for g in [-12.0f32, -6.0, 0.0, 6.0, 12.0] {
        let y = geo.y_gain(g);
        p.hline(
            gain.x_range(),
            y,
            Stroke::new(1.0, if g == 0.0 { pal.grid_bar } else { pal.grid }),
        );
        if g.abs() != 6.0 {
            p.text(
                pos2(gain.left() - 4.0, y),
                Align2::RIGHT_CENTER,
                if g == 0.0 {
                    "0".to_string()
                } else {
                    format!("{g:+.0}")
                },
                design::paint::ruler(),
                pal.text_dim,
            );
        }
    }
    p.text(
        pos2(level.left() - 4.0, level.top() + 1.0),
        Align2::RIGHT_TOP,
        t("mb.axis_level"),
        design::paint::caption(),
        pal.text_dim,
    );

    // ---- the spectrum: the input as a faint fill, the output as a line ----
    let ed = &app.fx.multiband;
    if ed.pre.iter().any(|v| *v > -119.0) {
        let mut mesh = egui::Mesh::default();
        let fill = pal.text_dim.gamma_multiply(0.10);
        for i in 0..POINTS {
            let x = geo.x(point_hz(i));
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(pos2(x, geo.y_level(ed.pre[i])), fill);
            mesh.colored_vertex(pos2(x, level.bottom()), fill);
            if i > 0 {
                mesh.add_triangle(base - 2, base - 1, base);
                mesh.add_triangle(base - 1, base, base + 1);
            }
        }
        p.add(mesh);
        let line: Vec<Pos2> = (0..POINTS)
            .map(|i| pos2(geo.x(point_hz(i)), geo.y_level(ed.post[i])))
            .collect();
        p.add(egui::Shape::line(
            line,
            Stroke::new(1.25, pal.text_dim.gamma_multiply(0.85)),
        ));
    }

    // ---- per band, in the level pane: its detector level as a bar, its thresholds as
    // lines to drag, its number and state at the top ----
    let mut hovered: Option<(usize, Grab)> = None;
    let mut key_row = 0.0f32;
    for pos in 0..lay.n {
        let b = lay.order[pos];
        let band = &s.bands[b];
        let Some((x0, x1)) = geo.span(lay, b) else {
            continue;
        };
        let color = band_color(b);
        let (x0i, x1i) = (x0 + 2.0, x1 - 2.0);
        let wide = x1 - x0 > 64.0 || selected == Some(b);
        let level_db = tel[T_LEVEL + b];
        let thr_y = geo.y_level(band.thr_db);
        let lo_y = geo.y_level(band.thr_lo_db);
        let upper_on = band.ratio != 1.0 && band.range_db > 0.0;
        let lower_on = band.ratio_lo != 1.0 && band.range_lo_db > 0.0;
        // The level: a bar at the column's left edge, lit where it is past a threshold
        // that is working — so "is it acting?" reads at a glance.
        let bar = Rect::from_min_max(
            pos2(x0i + 2.0, level.top() + 22.0),
            pos2(x0i + 8.0, level.bottom()),
        );
        p.rect_filled(bar, 1.0, pal.grid.gamma_multiply(0.6));
        if level_db > LEVEL_LO {
            let ly = geo.y_level(level_db).max(bar.top());
            p.rect_filled(
                Rect::from_min_max(pos2(bar.left(), ly), bar.max),
                1.0,
                color.gamma_multiply(0.4),
            );
            // Lit — full colour and a little wider — where a working section acts.
            let lit = |a: f32, b: f32| {
                Rect::from_min_max(pos2(bar.left() - 1.0, a), pos2(bar.right() + 1.0, b))
            };
            if upper_on && level_db > band.thr_db {
                p.rect_filled(lit(ly, thr_y), 1.0, color);
            }
            if lower_on && level_db < band.thr_lo_db {
                p.rect_filled(lit(lo_y, ly), 1.0, color);
            }
        }
        // Thresholds: the upper a solid line with a notch at its right end, the lower a
        // dashed one with the notch the other way up. A section that is off (1:1) is
        // faint, and the lower one is only drawn when it is on or the band is selected.
        let line_x0 = bar.right() + 3.0;
        let near = |y: f32| {
            pointer.is_some_and(|pt| {
                pt.x >= x0 && pt.x <= x1 && (pt.y - y).abs() <= 5.0 && level.contains(pt)
            })
        };
        let drag = app.fx.multiband.drag;
        let held = |lower: bool| matches!(drag, Some(Drag::Threshold { band, lower: l }) if band == b && l == lower);
        let show_lo = lower_on || selected == Some(b);
        let hot_hi = held(false) || (!held(true) && near(thr_y));
        let hot_lo = show_lo && !hot_hi && (held(true) || near(lo_y));
        if hot_hi {
            hovered = Some((b, Grab::Threshold(false)));
        } else if hot_lo {
            hovered = Some((b, Grab::Threshold(true)));
        }
        let stroke = |on: bool, hot: bool| -> Stroke {
            let w = if hot { 2.5 } else { 1.5 };
            Stroke::new(w, if on { color } else { color.gamma_multiply(0.4) })
        };
        let notch = |y: f32, down: bool, on: bool| {
            let nx = x1i - 7.0;
            let tip = if down { y + 7.0 } else { y - 7.0 };
            egui::Shape::convex_polygon(
                vec![pos2(nx - 5.0, y), pos2(nx + 5.0, y), pos2(nx, tip)],
                if on { color } else { color.gamma_multiply(0.4) },
                Stroke::NONE,
            )
        };
        p.line_segment(
            [pos2(line_x0, thr_y), pos2(x1i, thr_y)],
            stroke(upper_on, hot_hi),
        );
        p.add(notch(thr_y, true, upper_on));
        if wide && (upper_on || selected == Some(b)) {
            p.text(
                pos2(line_x0 + 2.0, thr_y - 1.0),
                Align2::LEFT_BOTTOM,
                format!("{:.0} dB \u{b7} {}", band.thr_db, ratio_label(band.ratio)),
                design::paint::ruler(),
                if upper_on { pal.text } else { pal.text_dim },
            );
        }
        if show_lo {
            p.add(egui::Shape::dashed_line(
                &[pos2(line_x0, lo_y), pos2(x1i, lo_y)],
                stroke(lower_on, hot_lo),
                5.0,
                3.0,
            ));
            p.add(notch(lo_y, false, lower_on));
            if wide {
                p.text(
                    pos2(line_x0 + 2.0, lo_y + 1.0),
                    Align2::LEFT_TOP,
                    format!(
                        "{:.0} dB \u{b7} {}",
                        band.thr_lo_db,
                        ratio_label(band.ratio_lo)
                    ),
                    design::paint::ruler(),
                    if lower_on { pal.text } else { pal.text_dim },
                );
            }
        }
        // The band's number, and what state it is in, at the top of its column.
        if x1 - x0 > 20.0 {
            let chip = pos2((x0 + x1) * 0.5, level.top() + 10.0);
            let mut flags = Vec::new();
            if !band.on {
                flags.push(t("mb.flag_off"));
            }
            if band.solo {
                flags.push(t("mb.flag_solo"));
            }
            if band.mute {
                flags.push(t("mb.flag_mute"));
            }
            match band.placement {
                Placement::Mid => flags.push(t("mb.flag_mid")),
                Placement::Side => flags.push(t("mb.flag_side")),
                Placement::Stereo => {}
            }
            if band.key_ext {
                flags.push(t("mb.flag_key"));
            }
            let chip = if !flags.is_empty() && x1 - x0 > 56.0 {
                let text = flags.join(" \u{b7} ");
                let galley = p.layout_no_wrap(text, design::paint::ruler(), pal.text);
                let w = 16.0 + galley.size().x;
                let c = pos2(chip.x - w * 0.5 + 7.0, chip.y);
                p.galley(
                    pos2(c.x + 10.0, c.y - galley.size().y * 0.5),
                    galley,
                    pal.text,
                );
                c
            } else {
                chip
            };
            p.circle_filled(chip, 7.0, color);
            p.text(
                chip,
                Align2::CENTER_CENTER,
                (b + 1).to_string(),
                design::paint::ruler(),
                theme::on_color(&pal, color),
            );
        }
        // A free key range: a bar along the bottom with a handle at each end, dragged
        // like everything else here. The detector hears what lies between them.
        if band.key_free {
            let ky = level.bottom() - 10.0 - key_row * 9.0;
            key_row += 1.0;
            let (kx0, kx1) = (geo.x(band.key_lo_hz), geo.x(band.key_hi_hz));
            p.line_segment(
                [pos2(kx0, ky), pos2(kx1, ky)],
                Stroke::new(3.0, color.gamma_multiply(0.8)),
            );
            for (x, lo) in [(kx0, true), (kx1, false)] {
                let hot = drag == Some(Drag::Key { band: b, lo })
                    || (hovered.is_none()
                        && pointer.is_some_and(|pt| pt.distance(pos2(x, ky)) <= 7.0));
                if hot {
                    hovered = Some((b, Grab::Key(lo)));
                }
                let d = if lo { 6.0 } else { -6.0 };
                p.add(egui::Shape::convex_polygon(
                    vec![pos2(x, ky - 6.0), pos2(x, ky + 6.0), pos2(x + d, ky)],
                    if hot { pal.gold } else { color },
                    Stroke::NONE,
                ));
            }
            p.text(
                pos2(kx0 - 4.0, ky),
                Align2::RIGHT_CENTER,
                t("mb.flag_key"),
                design::paint::ruler(),
                color,
            );
        }
    }

    // ---- per band, in the gain pane: the lens of what it can reach, its static gain as
    // a dot to drag, and what it is doing now as a tick with its value ----
    for pos in 0..lay.n {
        let b = lay.order[pos];
        let band = &s.bands[b];
        let Some((x0, x1)) = geo.span(lay, b) else {
            continue;
        };
        let color = band_color(b);
        let (x0i, x1i) = (x0 + 2.0, x1 - 2.0);
        let (cut, boost) = reach(band);
        let base = band.gain_db;
        if band.on && (cut > 0.0 || boost > 0.0) {
            let top = geo.y_gain(base + boost);
            let bottom = geo.y_gain(base - cut);
            p.rect_filled(
                Rect::from_min_max(pos2(x0i, top), pos2(x1i, bottom)),
                2.0,
                color.gamma_multiply(0.14),
            );
            // The lens's ends are brackets, so it cannot be taken for a threshold.
            for y in [top, bottom] {
                for (a, d) in [(x0i, 6.0), (x1i, -6.0)] {
                    p.line_segment([pos2(a, y), pos2(a + d, y)], Stroke::new(1.0, color));
                }
            }
        }
        let dyn_db = tel[T_GAIN + b];
        if band.on && dyn_db.abs() > 0.05 {
            let y = geo.y_gain(base + dyn_db);
            p.line_segment(
                [pos2(x0i + 4.0, y), pos2(x1i - 4.0, y)],
                Stroke::new(2.0, color),
            );
            if x1 - x0 > 44.0 {
                p.text(
                    pos2(x1i - 3.0, y + if dyn_db < 0.0 { 2.0 } else { -2.0 }),
                    if dyn_db < 0.0 {
                        Align2::RIGHT_TOP
                    } else {
                        Align2::RIGHT_BOTTOM
                    },
                    format!("{dyn_db:+.1}"),
                    design::paint::ruler(),
                    pal.text,
                );
            }
        }
        let dot = pos2(geo.x(centre_hz(lay, b)), geo.y_gain(base));
        let dot_near = pointer.is_some_and(|pt| pt.distance(dot) <= DOT_R + 3.0);
        let held = app
            .fx
            .multiband
            .drag
            .is_some_and(|d| matches!(d, Drag::Gain { band, .. } if band == b));
        if (dot_near && hovered.is_none()) || held {
            hovered = Some((b, Grab::Gain));
        }
        if band.on {
            p.circle_filled(dot, DOT_R, color);
        } else {
            p.circle_stroke(dot, DOT_R, Stroke::new(1.5, color));
        }
        if selected == Some(b) || dot_near || held {
            p.circle_stroke(dot, DOT_R + 2.5, Stroke::new(1.5, pal.text));
        }
        p.text(
            dot,
            Align2::CENTER_CENTER,
            (b + 1).to_string(),
            design::paint::ruler(),
            theme::on_color(&pal, color),
        );
    }

    // ---- the response now, and the ghost of the deepest of the last seconds ----
    if lay.n > 0 {
        // A bypassed band is at 0 dB, whatever its gain says.
        let statics: [f32; MB_BANDS] = std::array::from_fn(|b| {
            if s.bands[b].on {
                s.bands[b].gain_db
            } else {
                0.0
            }
        });
        // Solo and mute are part of what is heard: a band that is silent is drawn at the
        // floor, as the output has it.
        let any_solo = s.bands.iter().any(|b| b.used && b.solo);
        let silent = |b: usize| s.bands[b].mute || (any_solo && !s.bands[b].solo);
        let now: [f32; MB_BANDS] = std::array::from_fn(|b| {
            if silent(b) {
                -60.0
            } else {
                statics[b] + tel[T_GAIN + b]
            }
        });
        let hist = &app.fx.multiband.history;
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
        let curve = |db: &[f32; MB_BANDS], side: bool| -> Vec<Pos2> {
            (0..POINTS)
                .map(|i| {
                    let hz = point_hz(i);
                    pos2(geo.x(hz), geo.y_gain(response_part_db(s, db, sr, hz, side)))
                })
                .collect()
        };
        if ghost != now {
            p.add(egui::Shape::dashed_line(
                &curve(&ghost, false),
                Stroke::new(1.0, pal.gold.gamma_multiply(0.75)),
                4.0,
                3.0,
            ));
        }
        // With a mid- or side-only band, the mid and the side are two curves: the side
        // thinner, each named at its right end.
        let split_image = (0..lay.n).any(|p| s.bands[lay.order[p]].placement != Placement::Stereo);
        let mid = curve(&now, false);
        if split_image {
            let side = curve(&now, true);
            // Name the two where they are furthest apart, each on its own side, so the
            // names never sit on each other or on a band's readout at the edge.
            // (Short of the right edge, where the names would run off the pane.)
            let room = gain.right() - space::GRID * 20.0;
            let at = (0..side.len().min(mid.len()))
                .filter(|i| side[*i].x < room)
                .max_by(|a, b| {
                    let d = |i: usize| (side[i].y - mid[i].y).abs();
                    d(*a).total_cmp(&d(*b))
                })
                .unwrap_or(0);
            let (sp, mp) = (side[at], mid[at]);
            p.add(egui::Shape::line(
                side,
                Stroke::new(1.5, pal.gold.gamma_multiply(0.8)),
            ));
            let side_above = sp.y <= mp.y;
            for (pt, key, above) in [
                (sp, "mb.flag_side", side_above),
                (mp, "mb.flag_mid", !side_above),
            ] {
                let (dy, align) = if above {
                    (-2.0, Align2::LEFT_BOTTOM)
                } else {
                    (2.0, Align2::LEFT_TOP)
                };
                p.text(
                    pt + vec2(4.0, dy),
                    align,
                    t(key),
                    design::paint::ruler(),
                    pal.gold,
                );
            }
        }
        p.add(egui::Shape::line(mid, Stroke::new(2.5, pal.gold)));
    }

    // ---- crossovers: a line through both panes, a handle with the frequency where the
    // panes meet ----
    let mut hovered_xo = None;
    for pos in 1..lay.n {
        let b = lay.order[pos];
        let hz = lay.crossovers[pos - 1];
        let x = geo.x(hz);
        let label = hz_label(hz);
        let galley = p.layout_no_wrap(label, design::paint::ruler(), pal.text);
        let handle = Rect::from_center_size(
            pos2(x, (level.bottom() + gain.top()) * 0.5),
            vec2(galley.size().x + space::S2, galley.size().y + 4.0),
        );
        let held = app.fx.multiband.drag == Some(Drag::Crossover { band: b });
        let near = pointer.is_some_and(|pt| {
            handle.expand(2.0).contains(pt)
                || ((pt.x - x).abs() <= 5.0 && (level.contains(pt) || gain.contains(pt)))
        });
        let hot = held || (near && hovered.is_none());
        if hot {
            hovered_xo = Some(b);
        }
        let stroke = Stroke::new(
            if hot { 2.0 } else { 1.0 },
            if hot { pal.gold } else { pal.text_dim },
        );
        for r in [level, gain] {
            p.add(egui::Shape::dashed_line(
                &[pos2(x, r.top()), pos2(x, r.bottom())],
                stroke,
                4.0,
                3.0,
            ));
        }
        p.rect_filled(
            handle,
            pal.corner_radius,
            if hot { pal.gold } else { pal.panel_raised },
        );
        p.rect_stroke(
            handle,
            pal.corner_radius,
            Stroke::new(1.0, if hot { pal.gold } else { pal.grid_strong }),
            StrokeKind::Inside,
        );
        p.galley(
            handle.center() - galley.size() * 0.5,
            galley,
            if hot {
                theme::on_color(&pal, pal.gold)
            } else {
                pal.text
            },
        );
    }

    lanes_paint(app, &p, &geo, s, lay);
    meters_paint(app, &p, &geo);

    // ---- the cursor readout, or the invitation when there is nothing ----
    if lay.n == 0 {
        // On a card of its own, so the spectrum behind does not run through the words.
        let card = Rect::from_center_size(
            level.center() - vec2(0.0, 2.0),
            vec2(space::GRID * 104.0, space::GRID * 18.0),
        );
        p.rect_filled(card, pal.corner_radius, pal.panel_raised);
        p.rect_stroke(
            card,
            pal.corner_radius,
            Stroke::new(1.0, pal.grid_strong),
            StrokeKind::Inside,
        );
        p.text(
            level.center() - vec2(0.0, 18.0),
            Align2::CENTER_CENTER,
            t("mb.empty"),
            design::paint::empty_state(),
            pal.text,
        );
    } else if let Some(pt) = pointer.filter(|pt| level.contains(*pt) || gain.contains(*pt)) {
        let hz = geo.hz(pt.x);
        let value = if level.contains(pt) {
            format!("{:.1} dBFS", geo.level_at(pt.y))
        } else {
            format!("{:+.1} dB", geo.gain_at(pt.y))
        };
        p.text(
            pos2(level.right() - 4.0, level.top() + 22.0),
            Align2::RIGHT_TOP,
            format!(
                "{}  \u{b7}  {}  \u{b7}  {value}",
                hz_label(hz),
                note_label(hz)
            ),
            design::paint::ruler(),
            pal.text,
        );
    }
    for r in [level, gain, lanes] {
        p.rect_stroke(
            r,
            pal.corner_radius,
            Stroke::new(1.0, pal.grid_strong),
            StrokeKind::Inside,
        );
    }

    // The empty state's quick starts.
    if lay.n == 0 {
        let row = Rect::from_center_size(
            level.center() + vec2(0.0, 16.0),
            vec2(space::GRID * 60.0, space::ROW),
        );
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(row)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        for n in [2usize, 3, 4] {
            if child.button(tf("mb.quick", &[("n", &n)])).clicked() {
                app.mb_quick_bands(strip, effect, n);
            }
        }
    }

    // The output, under the meters.
    let out_rect = Rect::from_min_max(
        pos2(meters.left(), lanes.top()),
        pos2(rect.right(), rect.bottom()),
    );
    let mut out = s.output_db;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(out_rect)
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    let r = child.add(
        Knob::new(&mut out, -24.0..=24.0, 0.0)
            .diameter(space::GRID * 6.0)
            .unit("dB")
            .label(t("fx.mb.out")),
    );
    if r.changed() {
        app.mb_set(strip, effect, "out", out, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }

    gestures(
        app, ui, &resp, strip, effect, s, lay, &geo, hovered, hovered_xo,
    );
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    Threshold(bool),
    Gain,
    Key(bool),
}

/// Every drag, click and key on the display.
#[allow(clippy::too_many_arguments)]
fn gestures(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    resp: &egui::Response,
    strip: StripId,
    effect: EffectId,
    s: &Settings,
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
    if hovered_xo.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    } else if matches!(
        hovered,
        Some((_, Grab::Threshold(_))) | Some((_, Grab::Gain))
    ) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    } else if matches!(hovered, Some((_, Grab::Key(_)))) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if resp.drag_started() {
        let origin = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos())
            .unwrap_or_default();
        let drag = if let Some(band) = hovered_xo {
            Some(Drag::Crossover { band })
        } else {
            match hovered {
                Some((band, Grab::Threshold(lower))) => Some(Drag::Threshold { band, lower }),
                Some((band, Grab::Gain)) => Some(Drag::Gain {
                    band,
                    origin_db: s.bands[band].gain_db,
                    origin_y: origin.y,
                }),
                Some((band, Grab::Key(lo))) => Some(Drag::Key { band, lo }),
                None => None,
            }
        };
        if let Some(d) = drag {
            let band = match d {
                Drag::Crossover { band }
                | Drag::Threshold { band, .. }
                | Drag::Gain { band, .. }
                | Drag::Key { band, .. } => band,
            };
            if !matches!(d, Drag::Crossover { .. }) {
                app.fx.multiband.selected = Some(band);
            }
        }
        app.fx.multiband.drag = drag;
    }
    if let (Some(d), Some(pt)) = (app.fx.multiband.drag, resp.interact_pointer_pos()) {
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
                    let hi = if pos < lay.n - 1 {
                        lay.crossovers[pos]
                    } else {
                        HI_HZ * 2f32.powf(MIN_OCT)
                    } / 2f32.powf(MIN_OCT);
                    let hz = geo.hz(pt.x).clamp(lo.min(hi), hi.max(lo));
                    app.mb_set(strip, effect, &mb_key(band, "from"), hz, true);
                }
                Drag::Threshold { band, lower } => {
                    let b = &s.bands[band];
                    let db = geo.level_at(pt.y);
                    if lower {
                        let v = db.clamp(-90.0, b.thr_db);
                        app.mb_set(strip, effect, &mb_key(band, "thr_lo"), v, true);
                    } else {
                        let v = db.clamp(b.thr_lo_db.max(-80.0), 0.0);
                        app.mb_set(strip, effect, &mb_key(band, "thr"), v, true);
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
                    app.mb_set(strip, effect, &mb_key(band, "gain"), v, true);
                }
                Drag::Key { band, lo } => {
                    // A sixth of an octave apart at least.
                    let b = &s.bands[band];
                    let hz = geo.hz(pt.x);
                    let (field, v) = if lo {
                        (
                            "key_lo",
                            hz.clamp(LO_HZ, b.key_hi_hz / 2f32.powf(1.0 / 6.0)),
                        )
                    } else {
                        (
                            "key_hi",
                            hz.clamp(b.key_lo_hz * 2f32.powf(1.0 / 6.0), HI_HZ),
                        )
                    };
                    app.mb_set(strip, effect, &mb_key(band, field), v, true);
                }
            }
        }
    }
    if resp.drag_stopped() {
        app.fx.multiband.drag = None;
        app.end_gesture();
    }
    let pointer = resp.interact_pointer_pos().or(resp.hover_pos());
    if resp.clicked() {
        if let Some(b) = pointer.and_then(band_at) {
            app.fx.multiband.selected = Some(b);
        }
    }
    if resp.double_clicked() && hovered.is_none() && hovered_xo.is_none() {
        if let Some(pt) = pointer.filter(|pt| geo.level.contains(*pt) || geo.gain.contains(*pt)) {
            if app.mb_split_at(strip, effect, geo.hz(pt.x)).is_none() {
                app.set_status(t("mb.no_room"));
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
        let sel = app.fx.multiband.selected.and_then(|b| lay.position(b));
        if (prev || next) && lay.n > 0 {
            let p = match sel {
                Some(p) if next => (p + 1).min(lay.n - 1),
                Some(p) => p.saturating_sub(1),
                None => 0,
            };
            app.fx.multiband.selected = Some(lay.order[p]);
        }
        if let Some(b) = app.fx.multiband.selected.filter(|b| s.bands[*b].used) {
            if del {
                app.mb_remove_band(strip, effect, b);
            } else if up || down {
                let step = if fine { 0.1 } else { 1.0 } * if up { 1.0 } else { -1.0 };
                let v = (s.bands[b].thr_db + step).clamp(-80.0, 0.0);
                app.mb_set(strip, effect, &mb_key(b, "thr"), v, false);
            }
        }
    }
    // Right-click: the band's menu.
    let menu_band = pointer.and_then(band_at).or(app.fx.multiband.selected);
    resp.context_menu(|ui| {
        let Some(b) = menu_band.filter(|b| s.bands[*b].used) else {
            ui.label(t("mb.empty"));
            return;
        };
        band_menu(
            app,
            ui,
            strip,
            effect,
            b,
            &s.bands[b],
            pointer.map(|pt| geo.hz(pt.x)),
        );
    });
}

fn band_menu(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    b: usize,
    band: &BandSettings,
    at_hz: Option<f32>,
) {
    ui.label(
        RichText::new(tf("mb.band_n", &[("n", &(b + 1))]))
            .text_style(design::strong())
            .color(band_color(b)),
    );
    let toggle = |app: &mut CantusApp, ui: &mut egui::Ui, field: &str, on: bool, label: String| {
        if ui.selectable_label(on, label).clicked() {
            app.mb_set(
                strip,
                effect,
                &mb_key(b, field),
                if on { 0.0 } else { 1.0 },
                false,
            );
            ui.close();
        }
    };
    toggle(app, ui, "on", !band.on, t("mb.bypass_band"));
    toggle(app, ui, "solo", band.solo, t("fx.mb.solo"));
    toggle(app, ui, "mute", band.mute, t("fx.mb.mute"));
    ui.menu_button(t("fx.mb.place"), |ui| {
        for (i, pl) in Placement::ALL.iter().enumerate() {
            if ui
                .selectable_label(band.placement == *pl, t(&format!("mb.place.{i}")))
                .clicked()
            {
                app.mb_set(strip, effect, &mb_key(b, "place"), i as f32, false);
                ui.close();
            }
        }
    });
    ui.separator();
    if let Some(hz) = at_hz {
        if ui
            .button(tf("mb.split_here", &[("hz", &hz_label(hz))]))
            .clicked()
        {
            if app.mb_split_at(strip, effect, hz).is_none() {
                app.set_status(t("mb.no_room"));
            }
            ui.close();
        }
    }
    if ui.button(t("mb.remove_band")).clicked() {
        app.mb_remove_band(strip, effect, b);
        ui.close();
    }
}

/// One lane per band, lowest first: its gain change over the last seconds, cuts hanging
/// down and boosts rising from the lane's middle, with the value now at the right.
fn lanes_paint(app: &CantusApp, p: &egui::Painter, geo: &Geo, s: &Settings, lay: &Layout) {
    let pal = theme::p();
    let r = geo.lanes;
    if lay.n == 0 {
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            t("mb.lanes_empty"),
            design::paint::caption(),
            pal.text_dim,
        );
        return;
    }
    let hist = &app.fx.multiband.history;
    let lane_h = r.height() / lay.n as f32;
    let label_w = space::GRID * 7.0;
    let value_w = space::GRID * 13.0;
    let x0 = r.left() + label_w;
    let x1 = r.right() - value_w;
    let dx = (x1 - x0) / HISTORY as f32;
    let start = x1 - dx * hist.len() as f32;
    for pos in 0..lay.n {
        let b = lay.order[pos];
        let color = band_color(b);
        let top = r.top() + lane_h * pos as f32;
        let mid = top + lane_h * 0.5;
        let half = lane_h * 0.5 - 1.0;
        if pos > 0 {
            p.hline(r.x_range(), top, Stroke::new(1.0, pal.grid));
        }
        p.hline(x0..=x1, mid, Stroke::new(1.0, pal.grid_strong));
        p.circle_filled(
            pos2(r.left() + 9.0, mid),
            (half - 1.0).clamp(2.0, 5.0),
            color,
        );
        p.text(
            pos2(r.left() + 17.0, mid),
            Align2::LEFT_CENTER,
            (b + 1).to_string(),
            design::paint::ruler(),
            pal.text_dim,
        );
        // Cuts down, boosts up; 12 dB fills half a lane.
        let mut mesh = egui::Mesh::default();
        let fill = color.gamma_multiply(0.75);
        for (i, h) in hist.iter().enumerate() {
            let v = h[b];
            let x = start + dx * i as f32;
            let y = mid - (v / 12.0).clamp(-1.0, 1.0) * half;
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(pos2(x, mid), fill);
            mesh.colored_vertex(pos2(x, y), fill);
            if i > 0 {
                mesh.add_triangle(base - 2, base - 1, base);
                mesh.add_triangle(base - 1, base, base + 1);
            }
        }
        p.add(mesh);
        let now = hist.back().map(|h| h[b]).unwrap_or(0.0);
        let band = &s.bands[b];
        let text = if !band.on {
            t("mb.flag_off")
        } else {
            format!("{now:+.1} dB")
        };
        p.text(
            pos2(r.right() - 4.0, mid),
            Align2::RIGHT_CENTER,
            text,
            design::paint::ruler(),
            if now.abs() > 0.05 {
                pal.text
            } else {
                pal.text_dim
            },
        );
    }
}

/// Input and output, left and right: the name, the peak in dB, then the bars.
fn meters_paint(app: &CantusApp, p: &egui::Painter, geo: &Geo) {
    let pal = theme::p();
    let r = geo.meters;
    let m = app.fx.multiband.meters;
    let top = r.top() + 28.0;
    let bottom = r.bottom();
    let y = |db: f32| {
        let f = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
        bottom - f * (bottom - top)
    };
    let pair_gap = space::S2;
    let bar_w = (r.width() - pair_gap - 2.0) / 4.0;
    for (i, db) in m.iter().enumerate() {
        let x = r.left() + i as f32 * bar_w + if i >= 2 { pair_gap } else { 0.0 };
        let bar = Rect::from_min_max(pos2(x, top), pos2(x + bar_w - 1.0, bottom));
        p.rect_filled(bar, 1.0, theme::recess(&pal));
        if *db > -60.0 {
            let lin = 10f32.powf(db / 20.0);
            p.rect_filled(
                Rect::from_min_max(pos2(bar.left(), y(*db)), bar.max),
                1.0,
                theme::meter_color(lin),
            );
        }
        if i % 2 == 0 {
            let peak = m[i].max(m[i + 1]);
            let cx = x + bar_w;
            p.text(
                pos2(cx, r.top()),
                Align2::CENTER_TOP,
                t(if i == 0 {
                    "mb.meter_in"
                } else {
                    "mb.meter_out"
                }),
                design::paint::caption(),
                pal.text_dim,
            );
            p.text(
                pos2(cx, r.top() + 13.0),
                Align2::CENTER_TOP,
                if peak > -60.0 {
                    format!("{peak:.1}")
                } else {
                    "-\u{221e}".into()
                },
                design::paint::ruler(),
                if peak >= 0.0 { pal.danger } else { pal.text },
            );
        }
    }
    for db in [-6.0f32, -12.0, -24.0, -48.0] {
        p.hline(
            r.left()..=r.right(),
            y(db),
            Stroke::new(1.0, pal.grid.gamma_multiply(0.8)),
        );
    }
}

// ---- the band panel -----------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn band_panel(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &Settings,
    lay: &Layout,
    tel: &cantus_engine::Telemetry,
) {
    let pal = theme::p();
    let Some(b) = app.fx.multiband.selected.filter(|b| s.bands[*b].used) else {
        // The same card, empty, so the window does not jump when the first band arrives.
        design::Elevation::Card.frame(&pal).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(space::GRID * 29.0);
            caption(ui, t("mb.hint"));
        });
        return;
    };
    let band = s.bands[b];
    let spec = &slot.spec;
    design::Elevation::Card.frame(&pal).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (chip, _) = ui.allocate_exact_size(vec2(space::S3, space::S3), Sense::hover());
            ui.painter()
                .circle_filled(chip.center(), chip.width() * 0.5, band_color(b));
            let (lo, hi) = lay.edges(b).unwrap_or((None, None));
            ui.label(
                RichText::new(tf("mb.band_n", &[("n", &(b + 1))])).text_style(design::strong()),
            );
            caption(
                ui,
                format!(
                    "{} \u{2013} {}",
                    hz_label(lo.unwrap_or(LO_HZ)),
                    hz_label(hi.unwrap_or(HI_HZ))
                ),
            );
            ui.separator();
            for (field, on, label) in [
                ("on", band.on, t("fx.mb.on")),
                ("solo", band.solo, t("fx.mb.solo")),
                ("mute", band.mute, t("fx.mb.mute")),
            ] {
                if ui
                    .selectable_label(on, label)
                    .on_hover_text(t(&format!("mb.{field}_tip")))
                    .clicked()
                {
                    app.mb_set(
                        strip,
                        effect,
                        &mb_key(b, field),
                        if on { 0.0 } else { 1.0 },
                        false,
                    );
                }
            }
            ui.separator();
            egui::ComboBox::from_id_salt(("mb-place", effect.0, b))
                .selected_text(t(&format!("mb.place.{}", band.placement.index())))
                .width(space::GRID * 18.0)
                .show_ui(ui, |ui| {
                    for (i, pl) in Placement::ALL.iter().enumerate() {
                        if ui
                            .selectable_label(band.placement == *pl, t(&format!("mb.place.{i}")))
                            .clicked()
                        {
                            app.mb_set(strip, effect, &mb_key(b, "place"), i as f32, false);
                        }
                    }
                })
                .response
                .on_hover_text(t("mb.place_tip"));
            // Which key, and which part of it the detector hears.
            let keyed = slot.sidechain.is_some();
            let key_text = if band.key_ext {
                t("mb.key_ext")
            } else {
                t("mb.key_own")
            };
            egui::ComboBox::from_id_salt(("mb-bkey", effect.0, b))
                .selected_text(key_text)
                .width(space::GRID * 22.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(!band.key_ext, t("mb.key_own"))
                        .clicked()
                    {
                        app.mb_set(strip, effect, &mb_key(b, "key"), 0.0, false);
                    }
                    if ui
                        .add_enabled(
                            keyed,
                            egui::Button::selectable(band.key_ext, t("mb.key_ext")),
                        )
                        .on_disabled_hover_text(t("mb.key_hint"))
                        .clicked()
                    {
                        app.mb_set(strip, effect, &mb_key(b, "key"), 1.0, false);
                    }
                })
                .response
                .on_hover_text(t("mb.key_band_tip"));
            let mut free = band.key_free;
            if ui
                .checkbox(&mut free, t("fx.mb.key_free"))
                .on_hover_text(t("mb.key_free_tip"))
                .changed()
            {
                app.mb_set(
                    strip,
                    effect,
                    &mb_key(b, "key_free"),
                    if free { 1.0 } else { 0.0 },
                    false,
                );
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t("mb.remove_band")).clicked() {
                    app.mb_remove_band(strip, effect, b);
                }
                caption(ui, tf("mb.bands", &[("n", &lay.n), ("max", &MB_BANDS)]));
            });
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = space::S1;
            let knobs: &[&str] = if band.key_free {
                &[
                    "thr", "ratio", "range", "thr_lo", "ratio_lo", "range_lo", "knee", "att",
                    "rel", "la", "gain", "key_lo", "key_hi",
                ]
            } else {
                &[
                    "thr", "ratio", "range", "thr_lo", "ratio_lo", "range_lo", "knee", "att",
                    "rel", "la", "gain",
                ]
            };
            let curve_side = space::GRID * 30.0;
            // Each knob is its diameter plus 20 px wide; the gaps and separators come out
            // of the room first.
            let separators = if band.key_free { 3.0 } else { 2.0 };
            let room = ui.available_width() - curve_side - space::S4 - separators * 12.0;
            let per = room / knobs.len() as f32 - space::S1;
            let d = (per - 20.0).clamp(space::GRID * 6.0, space::GRID * 10.0);
            for (i, field) in knobs.iter().enumerate() {
                if i == 3 || i == 6 || i == 11 {
                    ui.separator();
                }
                band_knob(app, ui, strip, effect, spec, b, field, d);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                transfer_curve(app, ui, strip, effect, b, &band, tel, curve_side);
            });
        });
    });
}

#[allow(clippy::too_many_arguments)]
fn band_knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    spec: &EffectSpec,
    b: usize,
    field: &str,
    diameter: f32,
) {
    let Some((_, label, min, max, default, unit, log, stepped)) =
        MB_BAND_FIELDS.iter().find(|f| f.0 == field).copied()
    else {
        return;
    };
    let key = mb_key(b, field);
    let mut v = spec.get(&key);
    let knob = Knob::new(&mut v, min..=max, default)
        .diameter(diameter)
        .log(log)
        .stepped(stepped)
        .unit(unit)
        .label(t(label));
    let knob = match field {
        "ratio" | "ratio_lo" => knob.format(ratio_label),
        "la" => knob.format(|x| {
            if x <= 0.0 {
                t("mb.off")
            } else {
                crate::widgets::knob::readout(x, "ms")
            }
        }),
        _ => knob,
    };
    let r = ui.add(knob).on_hover_text(t(&format!("mb.tip.{field}")));
    if r.changed() {
        app.mb_set(strip, effect, &key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
}

/// The selected band's two-sided transfer curve: input level across, output up. Drag the
/// upper or the lower threshold point, or the curve beyond either to set its ratio.
#[allow(clippy::too_many_arguments)]
fn transfer_curve(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    b: usize,
    band: &BandSettings,
    tel: &cantus_engine::Telemetry,
    side: f32,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(vec2(side, side * 0.64), Sense::click_and_drag());
    let lo = LEVEL_LO;
    let x = |d: f32| rect.left() + ((d - lo) / -lo).clamp(0.0, 1.0) * rect.width();
    let y = |d: f32| rect.bottom() - ((d - lo) / -lo).clamp(0.0, 1.0) * rect.height();
    let dx_of = |px: f32| lo + (px - rect.left()) / rect.width() * -lo;
    let dy_of = |py: f32| lo + (rect.bottom() - py) / rect.height() * -lo;
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    for g in [-12.0f32, -24.0, -36.0, -48.0, -60.0] {
        p.hline(rect.x_range(), y(g), Stroke::new(1.0, pal.grid));
        p.vline(x(g), rect.y_range(), Stroke::new(1.0, pal.grid));
    }
    p.line_segment(
        [pos2(x(lo), y(lo)), pos2(x(0.0), y(0.0))],
        Stroke::new(1.0, pal.grid_bar),
    );
    let color = band_color(b);
    let curve: Vec<Pos2> = (0..=96)
        .map(|i| {
            let d = lo + -lo * i as f32 / 96.0;
            pos2(x(d), y(transfer_db(d, band)))
        })
        .collect();
    p.add(egui::Shape::line(curve, Stroke::new(2.0, color)));
    let hi_pt = pos2(x(band.thr_db), y(transfer_db(band.thr_db, band)));
    let lo_pt = pos2(x(band.thr_lo_db), y(transfer_db(band.thr_lo_db, band)));
    p.circle_filled(hi_pt, 5.0, color);
    p.circle_stroke(lo_pt, 5.0, Stroke::new(2.0, color));
    let level = tel[T_LEVEL + b];
    if level > lo {
        p.circle_filled(
            pos2(x(level), y(level + curve_gain_db(level, band))),
            3.5,
            pal.danger,
        );
    }
    p.text(
        pos2(rect.left() + 4.0, rect.top() + 3.0),
        Align2::LEFT_TOP,
        format!(
            "{}  {}",
            ratio_label(band.ratio),
            ratio_label(band.ratio_lo)
        ),
        design::paint::ruler(),
        pal.text,
    );
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    // Which handle, for the whole drag.
    let id = egui::Id::new(("mb-transfer", effect.0, b));
    if resp.drag_started() {
        if let Some(pt) = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos())
        {
            let grab: u8 = if pt.distance(hi_pt) < 9.0 {
                1
            } else if pt.distance(lo_pt) < 9.0 {
                2
            } else if dx_of(pt.x) > band.thr_db {
                3
            } else if dx_of(pt.x) < band.thr_lo_db {
                4
            } else {
                0
            };
            ui.ctx().data_mut(|d| d.insert_temp(id, grab));
        }
    }
    let grab: u8 = ui.ctx().data(|d| d.get_temp(id)).unwrap_or(0);
    if let (true, Some(pt)) = (resp.dragged(), resp.interact_pointer_pos()) {
        let key = |f: &str| mb_key(b, f);
        match grab {
            1 => {
                let v = dx_of(pt.x).clamp(band.thr_lo_db.max(-80.0), 0.0);
                app.mb_set(strip, effect, &key("thr"), v, true);
            }
            2 => {
                let v = dx_of(pt.x).clamp(-90.0, band.thr_db);
                app.mb_set(strip, effect, &key("thr_lo"), v, true);
            }
            3 => {
                // The pointer is a point the curve should pass through above the threshold.
                let over_in = (dx_of(pt.x) - band.thr_db).max(0.5);
                let over_out = dy_of(pt.y) - band.thr_db;
                let r = if over_out <= 0.01 {
                    100.0
                } else {
                    (over_in / over_out).clamp(0.1, 100.0)
                };
                app.mb_set(strip, effect, &key("ratio"), r, true);
            }
            4 => {
                let under_in = (band.thr_lo_db - dx_of(pt.x)).max(0.5);
                let under_out = band.thr_lo_db - dy_of(pt.y);
                let r = if under_out <= 0.01 {
                    100.0
                } else {
                    (under_in / under_out).clamp(0.1, 100.0)
                };
                app.mb_set(strip, effect, &key("ratio_lo"), r, true);
            }
            _ => {}
        }
    }
    if resp.drag_stopped() {
        ui.ctx().data_mut(|d| d.remove::<u8>(id));
        app.end_gesture();
    }
    resp.on_hover_text(t("mb.transfer_tip"));
}

// ---- the inspector's picture ------------------------------------------------------------

pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), space::GRID * 14.0),
        Sense::click(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let s = settings(spec);
    let lay = Layout::of(&s.bands);
    let geo = Geo {
        level: rect,
        gain: rect,
        lanes: rect,
        meters: rect,
    };
    for pos in 0..lay.n {
        let b = lay.order[pos];
        if let Some((x0, x1)) = geo.span(&lay, b) {
            let color = band_color(b);
            p.rect_filled(
                Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.bottom())),
                0.0,
                color.gamma_multiply(if s.bands[b].can_act() { 0.22 } else { 0.07 }),
            );
            let (cut, boost) = reach(&s.bands[b]);
            let yc = geo.y_gain(s.bands[b].gain_db);
            if s.bands[b].on && (cut > 0.0 || boost > 0.0) {
                p.rect_filled(
                    Rect::from_min_max(
                        pos2(x0 + 2.0, geo.y_gain(s.bands[b].gain_db + boost)),
                        pos2(x1 - 2.0, geo.y_gain(s.bands[b].gain_db - cut)),
                    ),
                    1.0,
                    color.gamma_multiply(0.3),
                );
            }
            p.line_segment(
                [pos2(x0 + 2.0, yc), pos2(x1 - 2.0, yc)],
                Stroke::new(1.5, color),
            );
        }
        if pos > 0 {
            let x = geo.x(lay.crossovers[pos - 1]);
            p.vline(x, rect.y_range(), Stroke::new(1.0, pal.text_dim));
        }
    }
    p.text(
        pos2(rect.left() + space::S2, rect.top() + space::S1),
        Align2::LEFT_TOP,
        if lay.n == 0 {
            t("mb.preview_empty")
        } else {
            tf(
                "mb.preview",
                &[
                    ("n", &lay.n),
                    ("mode", &t(&format!("mb.mode.{}", s.mode as usize))),
                ],
            )
        },
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
    resp.on_hover_text(t("mb.open_tip"))
}

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
    let Some(spec) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .map(|e| e.spec.clone())
    else {
        return;
    };
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
