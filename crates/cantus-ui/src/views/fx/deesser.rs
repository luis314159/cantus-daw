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
//! Under the lanes, the **detection band is two edges dragged on the spectrum** of the
//! input (drag the middle to move both), with the cut the processing makes drawn over it
//! live; beside it, the knobs, every one keyboard-reachable. The wheel scrolls the history
//! back; a double-click returns to live. Every drag and knob turn is one undo step.

use super::FactoryPreset;
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::Knob;
use cantus_core::{Command, EffectId, EffectSlot, EffectSpec, MergeKey, StripId};
use cantus_dsp::sibilus::{
    self as ds, Column, DeessSettings, Detection, Listen, Processing, Sibilus,
};
use cantus_dsp::SpectrumAnalyser;
use cantus_engine::fx::deesser as tele;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, RichText, Sense, Shape, Stroke, StrokeKind};
use std::collections::VecDeque;

/// Columns of history kept: about 30 s at 48 kHz.
pub const HISTORY_COLS: usize = 2_800;
/// Pixels per column on screen.
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
#[derive(Copy, Clone, Debug)]
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
}

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
                peaks.push(p);
                run = None;
            }
            (false, None) => {}
        }
    }
    if let Some(p) = run {
        peaks.push(p);
    }
    peaks
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

impl CantusApp {
    /// Start listening for a threshold, or stop and set it: one undo step.
    pub fn deesser_learn(&mut self, strip: StripId, effect: EffectId) {
        let st = &mut self.fx.deesser;
        if !st.learning {
            st.learning = true;
            st.learn_cols.clear();
            return;
        }
        st.learning = false;
        let cols = std::mem::take(&mut st.learn_cols);
        let Some(spec) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.clone())
        else {
            return;
        };
        let s = tele::settings(&spec);
        match learn_threshold(&cols, s.detection == Detection::Voice, s.range_db) {
            Some(thr) => {
                self.set_fx_param(strip, effect, "thr", thr, false);
                self.fx.deesser.learned = Some(thr);
                self.set_status(tf("ds.learned_to", &[("db", &format!("{thr:.1}"))]));
            }
            None => self.set_status(t("ds.learn_none")),
        }
    }
}

/// Bring in the columns the engine measured since the last frame.
fn pump(app: &mut CantusApp, effect: EffectId) {
    let tel = app.fx_telemetry();
    let h = tele::header(&tel);
    let st = &mut app.fx.deesser;
    if st.effect != Some(effect) {
        *st = State {
            effect: Some(effect),
            ..State::default()
        };
    }
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
        if let Some(c) = tele::column(&tel, h.seq, index) {
            st.push(c);
        }
    }
    st.last_seq = Some(h.seq);
}

/// The spectrum of what the analyser tap brought, and the split's shape for the settings.
fn update_spectrum(app: &mut CantusApp, s: &DeessSettings) {
    let fx = &mut app.fx;
    let st = &mut fx.deesser;
    let n = st.analyser.size();
    if fx.pairs.len() >= n {
        let mut buf = Vec::with_capacity(2 * n);
        for (a, b) in fx.pairs.iter().skip(fx.pairs.len() - n) {
            buf.push(*a);
            buf.push(*b);
        }
        st.analyser.push(&buf);
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
        st.cut_amp = (0..POINTS)
            .map(|i| d.split_amplitude(point_hz(i)))
            .collect();
        st.cut_for = Some(key);
    }
}

pub fn body(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let s = tele::settings(&slot.spec);
    pump(app, effect);
    update_spectrum(app, &s);
    controls(app, ui, strip, effect, slot, &s);
    ui.add_space(space::S1);
    status_row(app, ui, &s);
    ui.add_space(space::S1);
    let bottom_h = space::GRID * 44.0;
    // Everything below asks for exactly what is left, spacing included, so a resizable
    // window does not creep taller frame by frame.
    let spacing = ui.spacing().item_spacing.y;
    let hist_h = (ui.available_height() - bottom_h - space::S2 - 3.0 * spacing)
        .floor()
        .max(space::GRID * 40.0);
    let width = ui.available_width();
    history(app, ui, strip, effect, &s, vec2(width, hist_h));
    ui.add_space(space::S2);
    ui.horizontal_top(|ui| {
        let knobs_w = space::GRID * 84.0;
        let spec_w = (ui.available_width() - knobs_w - space::S3).max(space::GRID * 60.0);
        spectrum(app, ui, strip, effect, &s, vec2(spec_w, bottom_h));
        ui.add_space(space::S2);
        knobs(app, ui, strip, effect, &slot.spec);
    });
}

/// A row of choices; returns the one clicked.
fn segmented(ui: &mut egui::Ui, current: usize, keys: &[&str]) -> Option<usize> {
    let mut out = None;
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 1.0;
        for (i, key) in keys.iter().enumerate() {
            if ui
                .selectable_label(i == current, t(key))
                .on_hover_text(t(&format!("{key}_tip")))
                .clicked()
            {
                out = Some(i);
            }
        }
    });
    out
}

fn caption(ui: &mut egui::Ui, key: &str) {
    ui.label(
        RichText::new(t(key))
            .text_style(design::caption())
            .color(theme::p().text_dim),
    );
}

/// Mode, processing, stereo and listening: every state visible at rest.
fn controls(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    s: &DeessSettings,
) {
    ui.horizontal_wrapped(|ui| {
        caption(ui, "fx.ds.mode");
        if let Some(i) = segmented(ui, s.detection.index(), &["ds.mode.0", "ds.mode.1"]) {
            app.set_fx_param(strip, effect, "mode", i as f32, false);
        }
        ui.separator();
        caption(ui, "fx.ds.proc");
        if let Some(i) = segmented(
            ui,
            s.processing.index(),
            &["ds.proc.0", "ds.proc.1", "ds.proc.2"],
        ) {
            app.set_fx_param(strip, effect, "proc", i as f32, false);
        }
        ui.separator();
        caption(ui, "fx.ds.stereo");
        if let Some(i) = segmented(
            ui,
            s.stereo.index(),
            &["ds.stereo.0", "ds.stereo.1", "ds.stereo.2", "ds.stereo.3"],
        ) {
            app.set_fx_param(strip, effect, "stereo", i as f32, false);
        }
    });
    ui.horizontal_wrapped(|ui| {
        caption(ui, "ds.key");
        key_picker(app, ui, strip, effect, slot);
        ui.separator();
        caption(ui, "fx.ds.listen");
        if let Some(i) = segmented(
            ui,
            s.listen.index(),
            &["ds.listen.0", "ds.listen.1", "ds.listen.2"],
        ) {
            app.set_fx_param(strip, effect, "listen", i as f32, false);
        }
        ui.separator();
        let learning = app.fx.deesser.learning;
        let label = if learning {
            let heard =
                event_peaks(&app.fx.deesser.learn_cols, s.detection == Detection::Voice).len();
            tf("ds.learning", &[("n", &heard)])
        } else {
            t("ds.learn")
        };
        // A button, not a choice: it starts listening, and a second press sets the line.
        let text = RichText::new(format!("\u{25ce} {label}"));
        if ui
            .add(egui::Button::new(text).selected(learning))
            .on_hover_text(t("ds.learn_tip"))
            .clicked()
        {
            app.deesser_learn(strip, effect);
        }
    });
}

/// The key: this strip's own input, or any other strip after its fader.
fn key_picker(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let current = slot.sidechain;
    let label = match current {
        Some(k) if app.project().strip(k).is_some() => {
            app.capture_label(cantus_engine::CaptureTarget::Strip(k))
        }
        _ => t("ds.key_own"),
    };
    egui::ComboBox::from_id_salt(("ds-key", effect.0))
        .selected_text(label)
        .width(space::GRID * 40.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_none(), t("ds.key_own"))
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
                .filter(|id| *id != strip && !app.project().routing_would_cycle(*id, Some(strip)))
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
        .on_hover_text(t("ds.key_tip"));
}

/// Events, how hard it works, and the latency — the guard rails against over-de-essing.
fn status_row(app: &CantusApp, ui: &mut egui::Ui, s: &DeessSettings) {
    let pal = theme::p();
    let st = &app.fx.deesser;
    let e = st.events;
    // Events in the last 10 s of history.
    let recent = (10.0 / st.col_s()) as usize;
    let tail: Vec<Column> = st
        .history
        .iter()
        .skip(st.history.len().saturating_sub(recent))
        .copied()
        .collect();
    let rate = closed_events(&tail);
    let sr = st.sr;
    let paused = (st.scroll > 0).then(|| st.scroll as f32 * st.col_s());
    ui.horizontal(|ui| {
        let text = if e.count == 0 {
            t("ds.stats_none")
        } else {
            tf(
                "ds.stats",
                &[
                    ("n", &e.count),
                    ("rate", &rate),
                    ("avg", &format!("{:.1}", e.mean_gr_db)),
                    ("max", &format!("{:.1}", e.max_gr_db)),
                ],
            )
        };
        ui.label(
            RichText::new(text)
                .text_style(design::mono_small())
                .color(pal.text_dim),
        );
        if e.count > 0 && e.mean_gr_db > HEAVY_DB {
            ui.label(
                RichText::new(format!(
                    "\u{26a0} {}",
                    tf("ds.heavy", &[("avg", &format!("{:.1}", e.mean_gr_db))])
                ))
                .text_style(design::caption())
                .color(pal.danger),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let lat = ds::latency_for(s, sr);
            let text = if lat == 0 {
                t("ds.latency_zero")
            } else {
                tf(
                    "ds.latency",
                    &[("ms", &format!("{:.1}", lat as f32 * 1_000.0 / sr))],
                )
            };
            ui.label(
                RichText::new(text)
                    .text_style(design::mono_small())
                    .color(pal.text),
            )
            .on_hover_text(t("ds.latency_tip"));
            ui.separator();
            // Live or paused, said at rest.
            let (text, color) = match paused {
                Some(back) => (tf("ds.paused", &[("s", &format!("{back:.1}"))]), pal.gold),
                None => (t("ds.live"), pal.text_dim),
            };
            ui.label(
                RichText::new(text)
                    .text_style(design::caption())
                    .color(color),
            );
        });
    });
}

/// Pixels right of the lanes for their scales, so no number is printed over the data.
const GUTTER: f32 = 40.0;

/// A line of text on a plate, legible over whatever is drawn under it.
fn plate(
    p: &egui::Painter,
    pos: Pos2,
    align: Align2,
    text: String,
    font: egui::FontId,
    color: Color32,
) -> Rect {
    let pal = theme::p();
    let galley = p.layout_no_wrap(text, font, color);
    let r = align.anchor_size(pos, galley.size() + vec2(6.0, 2.0));
    p.rect_filled(r, 2.0, pal.panel_raised.gamma_multiply(0.88));
    p.galley(r.min + vec2(3.0, 1.0), galley, color);
    r
}

/// A band between two edges, one point per column, coloured per column: the smooth
/// outline of the history rather than a staircase. `(x, top, bottom, colour)`.
fn band_strip(p: &egui::Painter, pts: &[(f32, f32, f32, Color32)]) {
    let mut mesh = egui::Mesh::default();
    for (k, &(x, y0, y1, c)) in pts.iter().enumerate() {
        let base = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(x, y0), c);
        mesh.colored_vertex(pos2(x, y1), c);
        if k > 0 {
            mesh.add_triangle(base - 2, base - 1, base);
            mesh.add_triangle(base - 1, base, base + 1);
        }
    }
    p.add(mesh);
}

/// Hatching for what was taken away: short diagonals across `[y0, y1]` in a column at
/// `x`, on a 6 px pitch that lines up from column to column.
fn hatch(shapes: &mut Vec<Shape>, x: f32, y0: f32, y1: f32, stroke: Stroke) {
    if y1 - y0 < 2.0 {
        return;
    }
    let pitch = 6.0;
    let mut y = y0 - (y0 + x).rem_euclid(pitch) + pitch;
    while y < y1 {
        let top = (y - PX).max(y0);
        shapes.push(Shape::line_segment(
            [pos2(x, y), pos2(x + (y - top), top)],
            stroke,
        ));
        y += pitch;
    }
}

/// Runs of columns cut by more than an event's worth that have ended — the events the
/// status row counts, so a run still going is not counted twice.
fn closed_events(cols: &[Column]) -> usize {
    let n = event_peaks(cols, false).len();
    if cols.last().is_some_and(|c| c.gr_db > ds::EVENT_ON_DB) {
        n.saturating_sub(1)
    } else {
        n
    }
}

/// The colours of the history, per theme: green is what de-essing did.
struct Ink {
    dim: Color32,
    left: Color32,
    taken: Color32,
    hatch: Stroke,
    bar: Color32,
}

impl Ink {
    fn new(pal: &theme::Palette) -> Ink {
        Ink {
            dim: pal
                .text_dim
                .gamma_multiply(if pal.dark { 0.35 } else { 0.26 }),
            left: pal.ok.gamma_multiply(if pal.dark { 0.8 } else { 0.62 }),
            taken: pal.ok.gamma_multiply(if pal.dark { 0.3 } else { 0.22 }),
            hatch: Stroke::new(1.0, pal.ok.gamma_multiply(0.75)),
            bar: pal.ok.gamma_multiply(if pal.dark { 0.9 } else { 0.75 }),
        }
    }
}

/// The signal lane's legend: a swatch of what is left and one of what was removed.
fn legend(p: &egui::Painter, at: Pos2, ink: &Ink) {
    let pal = theme::p();
    let font = design::paint::caption();
    let left = t("ds.legend.left");
    let removed = t("ds.legend.removed");
    let g1 = p.layout_no_wrap(left, font.clone(), pal.text);
    let g2 = p.layout_no_wrap(removed, font, pal.text);
    let sw = 10.0;
    let w = sw + 4.0 + g1.size().x + 10.0 + sw + 4.0 + g2.size().x + 6.0;
    let h = g1.size().y.max(sw) + 4.0;
    let r = Rect::from_min_size(pos2(at.x, at.y - h), vec2(w, h));
    p.rect_filled(r, 2.0, pal.panel_raised.gamma_multiply(0.88));
    let mut x = r.left() + 3.0;
    let cy = r.center().y;
    let s1 = Rect::from_center_size(pos2(x + sw * 0.5, cy), vec2(sw, sw));
    p.rect_filled(s1, 1.0, ink.left);
    x += sw + 4.0;
    let w1 = g1.size().x;
    p.galley(pos2(x, cy - g1.size().y * 0.5), g1, pal.text);
    x += w1 + 10.0;
    let s2 = Rect::from_center_size(pos2(x + sw * 0.5, cy), vec2(sw, sw));
    p.rect_filled(s2, 1.0, ink.taken);
    let mut shapes = Vec::new();
    hatch(&mut shapes, s2.left(), s2.top(), s2.bottom(), ink.hatch);
    hatch(
        &mut shapes,
        s2.left() + PX * 2.0,
        s2.top(),
        s2.bottom(),
        ink.hatch,
    );
    hatch(
        &mut shapes,
        s2.left() + PX * 4.0,
        s2.top(),
        s2.bottom(),
        ink.hatch,
    );
    p.extend(shapes);
    x += sw + 4.0;
    p.galley(pos2(x, cy - g2.size().y * 0.5), g2, pal.text);
}

/// The three lanes.
fn history(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &DeessSettings,
    size: egui::Vec2,
) {
    let pal = theme::p();
    let ink = Ink::new(&pal);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let gap = space::S1;
    let wave = Rect::from_min_max(
        rect.min,
        pos2(rect.right(), rect.top() + (rect.height() * 0.44).round()),
    );
    let cut = Rect::from_min_max(
        pos2(rect.left(), wave.bottom() + gap),
        pos2(
            rect.right(),
            wave.bottom() + gap + (rect.height() * 0.14).round(),
        ),
    );
    let det = Rect::from_min_max(pos2(rect.left(), cut.bottom() + gap), rect.max);
    app.fx.deesser.geo.signal = wave;
    app.fx.deesser.geo.cut = cut;
    app.fx.deesser.geo.detector = det;
    let geo = app.fx.deesser.geo;
    let voice = s.detection == Detection::Voice;
    let right = rect.right() - GUTTER;

    let p = ui.painter_at(rect);
    for lane in [wave, cut, det] {
        p.rect_filled(lane, pal.corner_radius, theme::recess(&pal));
    }

    // Wheel: look back; double-click: live.
    if resp.hovered() {
        let d = ui.input(|i| i.smooth_scroll_delta);
        let step = (d.y + d.x) / PX;
        if step != 0.0 {
            let st = &mut app.fx.deesser;
            let max = st.history.len().saturating_sub(1);
            st.scroll = (st.scroll as f32 + step).round().clamp(0.0, max as f32) as usize;
        }
    }
    if resp.double_clicked() {
        app.fx.deesser.scroll = 0;
    }

    let st = &app.fx.deesser;
    let cols = ((right - rect.left()) / PX).floor() as usize;
    let end = st.history.len().saturating_sub(st.scroll);
    let start = end.saturating_sub(cols);
    // The centre of column `i`.
    let x_of = |i: usize| right - (end - i) as f32 * PX + PX * 0.5;
    let visible: Vec<Column> = st.history.range(start..end).copied().collect();
    let xs: Vec<f32> = (start..end).map(x_of).collect();

    // ---- Signal: the input mirrored, green where de-essed ----
    let centre = wave.center().y;
    let half = wave.height() * 0.5 - 2.0;
    // The square root of the amplitude: a vowel keeps its shape (a dB scale flattens
    // everything loud into a slab) and a quiet "s" is still visible (a linear one loses it).
    let amp = |lin: f32| lin.clamp(0.0, 1.0).sqrt() * half;
    p.hline(
        wave.left()..=right,
        centre,
        Stroke::new(1.0, pal.grid_strong),
    );
    let active = |c: &Column| c.gr_db >= 0.5;
    let outer: Vec<_> = visible
        .iter()
        .zip(&xs)
        .map(|(c, x)| {
            let h = amp(c.input);
            let col = if active(c) { ink.taken } else { ink.dim };
            (*x, centre - h, centre + h, col)
        })
        .collect();
    band_strip(&p, &outer);
    let inner: Vec<_> = visible
        .iter()
        .zip(&xs)
        .map(|(c, x)| {
            if active(c) {
                let h = amp(c.output).min(amp(c.input));
                (*x, centre - h, centre + h, ink.left)
            } else {
                (*x, centre, centre, Color32::TRANSPARENT)
            }
        })
        .collect();
    band_strip(&p, &inner);
    let mut hatches = Vec::new();
    for (c, x) in visible.iter().zip(&xs) {
        if active(c) {
            let (hi, lo) = (amp(c.input), amp(c.output).min(amp(c.input)));
            let x = x - PX * 0.5;
            hatch(&mut hatches, x, centre - hi, centre - lo, ink.hatch);
            hatch(&mut hatches, x, centre + lo, centre + hi, ink.hatch);
        }
    }
    p.extend(hatches);
    for (db, label) in [(0.0f32, "0"), (-12.0, "\u{2212}12")] {
        let y = centre - amp(10f32.powf(db / 20.0));
        p.hline(right..=right + 3.0, y, Stroke::new(1.0, pal.text_dim));
        p.text(
            pos2(rect.right() - 3.0, y),
            if db == 0.0 {
                Align2::RIGHT_TOP
            } else {
                Align2::RIGHT_CENTER
            },
            label,
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    plate(
        &p,
        wave.left_top() + vec2(space::S1, space::S1),
        Align2::LEFT_TOP,
        t("ds.lane.signal"),
        design::paint::caption(),
        pal.text_dim,
    );
    legend(
        &p,
        pos2(wave.left() + space::S1, wave.bottom() - space::S1),
        &ink,
    );

    // ---- Cut: the reduction hanging from the top, on the Range's scale ----
    let scale = (s.range_db.max(6.0) / 6.0).ceil() * 6.0;
    let depth = |gr: f32| cut.top() + 1.0 + (gr / scale).clamp(0.0, 1.0) * (cut.height() - 2.0);
    let bars: Vec<_> = visible
        .iter()
        .zip(&xs)
        .map(|(c, x)| (*x, cut.top() + 1.0, depth(c.gr_db), ink.bar))
        .collect();
    band_strip(&p, &bars);
    let range_y = depth(s.range_db);
    p.add(Shape::dashed_line(
        &[pos2(cut.left(), range_y), pos2(right, range_y)],
        Stroke::new(1.0, pal.ok),
        4.0,
        4.0,
    ));
    for (y, label) in [
        (cut.top() + 1.0, "0".to_string()),
        (cut.bottom() - 1.0, format!("\u{2212}{scale:.0}")),
    ] {
        p.text(
            pos2(rect.right() - 3.0, y),
            if y < cut.center().y {
                Align2::RIGHT_TOP
            } else {
                Align2::RIGHT_BOTTOM
            },
            label,
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    let now_gr = st.history.back().map(|c| c.gr_db).unwrap_or(0.0);
    let peak_gr = visible.iter().map(|c| c.gr_db).fold(0.0f32, f32::max);
    let r = plate(
        &p,
        cut.left_top() + vec2(space::S1, space::S1 * 0.5),
        Align2::LEFT_TOP,
        t("ds.lane.cut"),
        design::paint::caption(),
        pal.text_dim,
    );
    plate(
        &p,
        pos2(r.right() + space::S1, r.top()),
        Align2::LEFT_TOP,
        tf(
            "ds.cut_now",
            &[
                ("db", &format!("{now_gr:.1}")),
                ("peak", &format!("{peak_gr:.1}")),
            ],
        ),
        design::paint::ruler(),
        pal.text,
    );

    // ---- Detector: the band level, the threshold you drag, why it cut ----
    for db in [-12.0f32, -24.0, -36.0, -48.0] {
        let y = geo.detector_y(db);
        p.hline(det.left()..=right, y, Stroke::new(1.0, pal.grid));
        p.text(
            pos2(rect.right() - 3.0, y),
            Align2::RIGHT_CENTER,
            format!("\u{2212}{:.0}", -db),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    let ty = geo.detector_y(s.threshold_db);
    let fill = |conf: f32| {
        pal.gold
            .gamma_multiply(if voice { 0.1 + 0.5 * conf } else { 0.45 })
    };
    let over: Vec<_> = visible
        .iter()
        .zip(&xs)
        .map(|(c, x)| {
            let y = geo.detector_y(c.det_db).min(ty);
            (*x, y, ty, fill(c.conf))
        })
        .collect();
    band_strip(&p, &over);
    if voice {
        // The classifier: as tall as it is sure, along the bottom.
        let rib = space::S3;
        let conf: Vec<_> = visible
            .iter()
            .zip(&xs)
            .map(|(c, x)| {
                (
                    *x,
                    det.bottom() - 1.0 - c.conf.clamp(0.0, 1.0) * rib,
                    det.bottom() - 1.0,
                    pal.gold.gamma_multiply(0.8),
                )
            })
            .collect();
        band_strip(&p, &conf);
    }
    let trace: Vec<Pos2> = visible
        .iter()
        .zip(&xs)
        .map(|(c, x)| pos2(*x, geo.detector_y(c.det_db)))
        .collect();
    if trace.len() > 1 {
        p.add(Shape::line(trace, Stroke::new(1.25, pal.text)));
    }
    // Where the sibilants peaked: a histogram against the scale, 3 dB a bin.
    let peaks = event_peaks(&visible, voice);
    if !peaks.is_empty() {
        let mut bins = [0usize; 20];
        for pk in &peaks {
            let b = ((pk - DET_LO_DB) / 3.0).floor();
            if (0.0..20.0).contains(&b) {
                bins[b as usize] += 1;
            }
        }
        let most = bins.iter().copied().max().unwrap_or(1).max(1) as f32;
        let hist_w = space::GRID * 8.0;
        for (b, n) in bins.iter().enumerate() {
            if *n == 0 {
                continue;
            }
            let lo = DET_LO_DB + b as f32 * 3.0;
            let r = Rect::from_min_max(
                pos2(
                    right - *n as f32 / most * hist_w,
                    geo.detector_y(lo + 3.0) + 1.0,
                ),
                pos2(right, geo.detector_y(lo) - 1.0),
            );
            p.rect_filled(r, 1.0, pal.ok.gamma_multiply(0.45));
            p.rect_stroke(r, 1.0, Stroke::new(1.0, pal.ok), StrokeKind::Inside);
        }
    }
    // The threshold: a dashed line, brighter while held or near.
    let held = app.fx.deesser.drag == Some(Grab::Threshold);
    let near = resp
        .hover_pos()
        .is_some_and(|pt| det.contains(pt) && (pt.y - ty).abs() < 8.0);
    p.add(Shape::dashed_line(
        &[pos2(det.left(), ty), pos2(right, ty)],
        Stroke::new(if held || near { 2.0 } else { 1.25 }, pal.gold),
        6.0,
        4.0,
    ));
    let mut label = tf(
        "ds.threshold_line",
        &[("db", &format!("{:.1}", s.threshold_db))],
    );
    if st
        .learned
        .is_some_and(|v| (v - s.threshold_db).abs() < 0.05)
    {
        label = format!("{label}  \u{b7}  {}", t("ds.learned"));
    }
    plate(
        &p,
        pos2(det.left() + space::S1, ty - 2.0),
        Align2::LEFT_BOTTOM,
        label,
        design::paint::ruler(),
        pal.gold,
    );
    plate(
        &p,
        det.left_top() + vec2(space::S1, space::S1 * 0.5),
        Align2::LEFT_TOP,
        t("ds.lane.detector"),
        design::paint::caption(),
        pal.text_dim,
    );
    if voice {
        plate(
            &p,
            pos2(det.left() + space::S1, det.bottom() - space::S3 - 2.0),
            Align2::LEFT_BOTTOM,
            t("ds.lane.sibilance"),
            design::paint::caption(),
            pal.text_dim,
        );
    }

    // Hover: what happened in that column, in real units.
    let col_s = st.col_s();
    if let Some(pt) = resp.hover_pos() {
        let back = ((right - pt.x) / PX).floor();
        if !held && pt.x < right && back >= 0.0 && (back as usize) < end - start {
            let i = end - 1 - back as usize;
            let c = st.history[i];
            p.vline(pt.x, rect.y_range(), Stroke::new(1.0, pal.grid_bar));
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
                        &format!("{:.2}", (end - 1 - i + st.scroll) as f32 * col_s),
                    ),
                    ("in", &db(c.input)),
                    ("det", &format!("{:.1}", c.det_db.max(-120.0))),
                    ("cut", &format!("{:.1}", c.gr_db)),
                ],
            );
            if voice {
                text.push(' ');
                text.push_str(&tf(
                    "ds.hover_conf",
                    &[("conf", &format!("{:.0}", c.conf * 100.0))],
                ));
            }
            if c.gr_db >= ds::EVENT_ON_DB && c.hz > 0.0 {
                text.push(' ');
                text.push_str(&tf("ds.hover_hz", &[("hz", &hz_text(c.hz))]));
            }
            let (x, align) = if pt.x + 360.0 < right {
                (pt.x + space::S2, Align2::LEFT_TOP)
            } else {
                (pt.x - space::S2, Align2::RIGHT_TOP)
            };
            plate(
                &p,
                pos2(x, wave.top() + space::S6),
                align,
                text,
                design::paint::ruler(),
                pal.text,
            );
        }
    }
    for lane in [wave, cut, det] {
        p.rect_stroke(
            lane,
            pal.corner_radius,
            Stroke::new(1.0, pal.grid_strong),
            StrokeKind::Inside,
        );
    }

    // The threshold is dragged where the events are. What is grabbed is decided where the
    // button went down, not where the pointer is once the drag is recognised.
    let pressed_near = ui
        .input(|i| i.pointer.press_origin())
        .is_some_and(|pt| det.contains(pt) && (pt.y - ty).abs() < 8.0);
    if resp.drag_started() && pressed_near {
        app.fx.deesser.drag = Some(Grab::Threshold);
    }
    if app.fx.deesser.drag == Some(Grab::Threshold) && resp.dragged() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let v = geo.detector_db(pt.y).clamp(DET_LO_DB, 0.0);
            app.set_fx_param(strip, effect, "thr", v, true);
        }
    }
    if resp.drag_stopped() && app.fx.deesser.drag == Some(Grab::Threshold) {
        app.fx.deesser.drag = None;
        app.end_gesture();
    }
    if near || held {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
}

fn hz_text(hz: f32) -> String {
    if hz >= 1_000.0 {
        format!("{:.1} kHz", hz / 1_000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

/// Set the band's edges as one gesture: both keys under one merge key, so a drag is one
/// undo step even when it moves both.
fn set_band(
    app: &mut CantusApp,
    strip: StripId,
    effect: EffectId,
    lo: Option<f32>,
    hi: Option<f32>,
) {
    for (key, v) in [("lo", lo), ("hi", hi)] {
        if let Some(v) = v {
            app.exec_merging(
                Command::SetEffectParam {
                    strip,
                    effect,
                    key: key.into(),
                    value: v,
                },
                MergeKey::of(&("ds-band", effect.0)),
            );
        }
    }
}

/// The detection band on the spectrum: drag its edges, or its middle to move it.
fn spectrum(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    s: &DeessSettings,
    size: egui::Vec2,
) {
    let pal = theme::p();
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    app.fx.deesser.geo.spectrum = rect;
    let geo = app.fx.deesser.geo;
    let st = &app.fx.deesser;
    let (lo, hi) = ds::band_edges(s.lo_hz, s.hi_hz, st.sr);
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let y_spec = |db: f32| {
        let f = ((db - SPEC_FLOOR_DB) / -SPEC_FLOOR_DB).clamp(0.0, 1.0);
        rect.bottom() - f * rect.height()
    };
    // The band, behind everything.
    let band = Rect::from_min_max(
        pos2(geo.spectrum_x(lo), rect.top()),
        pos2(geo.spectrum_x(hi), rect.bottom()),
    );
    p.rect_filled(band, 0.0, pal.gold.gamma_multiply(0.10));
    for hz in [500.0f32, 1_000.0, 2_000.0, 5_000.0, 10_000.0, 20_000.0] {
        let x = geo.spectrum_x(hz);
        p.vline(x, rect.y_range(), Stroke::new(1.0, pal.grid));
        // The last mark sits on the right edge: its label goes inside.
        let (at, align) = if hz >= SPEC_HI_HZ {
            (x - 3.0, Align2::RIGHT_BOTTOM)
        } else {
            (x + 3.0, Align2::LEFT_BOTTOM)
        };
        p.text(
            pos2(at, rect.bottom() - 2.0),
            align,
            hz_text(hz).replace(".0 ", " "),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
    // The input as a fill, glowing where the band is; the output as a line.
    let mut outside = egui::Mesh::default();
    let mut inside = egui::Mesh::default();
    let dim = pal
        .text_dim
        .gamma_multiply(if pal.dark { 0.22 } else { 0.18 });
    let glow = pal.gold.gamma_multiply(if pal.dark { 0.55 } else { 0.4 });
    for i in 1..POINTS {
        let (h0, h1) = (point_hz(i - 1), point_hz(i));
        let (x0, x1) = (geo.spectrum_x(h0), geo.spectrum_x(h1));
        let (y0, y1) = (y_spec(st.spec_in[i - 1]), y_spec(st.spec_in[i]));
        let (m, c) = if h1 > lo && h0 < hi {
            (&mut inside, glow)
        } else {
            (&mut outside, dim)
        };
        let base = m.vertices.len() as u32;
        m.colored_vertex(pos2(x0, y0), c);
        m.colored_vertex(pos2(x0, rect.bottom()), c);
        m.colored_vertex(pos2(x1, y1), c);
        m.colored_vertex(pos2(x1, rect.bottom()), c);
        m.add_triangle(base, base + 1, base + 2);
        m.add_triangle(base + 1, base + 2, base + 3);
    }
    p.add(outside);
    p.add(inside);
    let line: Vec<Pos2> = (0..POINTS)
        .map(|i| pos2(geo.spectrum_x(point_hz(i)), y_spec(st.spec_out[i])))
        .collect();
    p.add(Shape::line(
        line,
        Stroke::new(1.25, pal.text_dim.gamma_multiply(0.9)),
    ));

    // The cut the processing makes: at the full Range, dashed; live, solid. On its own
    // scale, half again the Range, so a 6 dB cut is a dip you can see.
    let cut_top = rect.top() + space::S6 + space::S1;
    let cut_h = rect.height() * 0.4;
    let cut_scale = (s.range_db * 1.5).max(9.0);
    let y_cut = |db: f32| cut_top + (-db / cut_scale).clamp(0.0, 1.0) * cut_h;
    for (db, strong) in [(0.0, true), (-s.range_db, false)] {
        let y = y_cut(db);
        p.hline(
            rect.x_range(),
            y,
            Stroke::new(1.0, pal.ok.gamma_multiply(if strong { 0.35 } else { 0.2 })),
        );
        p.text(
            pos2(rect.right() - 3.0, y - 1.0),
            Align2::RIGHT_BOTTOM,
            if strong {
                "0 dB".to_string()
            } else {
                format!("\u{2212}{:.0} dB", s.range_db)
            },
            design::paint::ruler(),
            pal.ok,
        );
    }
    let response = |gr: f32, i: usize| {
        let g = 10f32.powf(-gr / 20.0);
        let a = st.cut_amp[i];
        20.0 * (1.0 + (g - 1.0) * a).abs().max(1e-4).log10()
    };
    let full: Vec<Pos2> = (0..POINTS)
        .map(|i| pos2(geo.spectrum_x(point_hz(i)), y_cut(response(s.range_db, i))))
        .collect();
    p.add(Shape::dashed_line(
        &full,
        Stroke::new(1.0, pal.ok.gamma_multiply(0.8)),
        4.0,
        3.0,
    ));
    let now = st.history.back().map(|c| c.gr_db).unwrap_or(0.0);
    let live: Vec<Pos2> = (0..POINTS)
        .map(|i| pos2(geo.spectrum_x(point_hz(i)), y_cut(response(now, i))))
        .collect();
    p.add(Shape::line(live, Stroke::new(2.0, pal.ok)));
    plate(
        &p,
        pos2(rect.left() + space::S1, cut_top + 3.0),
        Align2::LEFT_TOP,
        tf(
            "ds.cut_curve",
            &[
                ("db", &format!("{now:.1}")),
                ("range", &format!("{:.1}", s.range_db)),
            ],
        ),
        design::paint::ruler(),
        pal.ok,
    );

    // The last event's centre.
    if st.events.count > 0 && st.events.last_hz > SPEC_LO_HZ {
        let x = geo.spectrum_x(st.events.last_hz);
        let y = rect.bottom() - space::S4;
        p.add(Shape::convex_polygon(
            vec![pos2(x, y - 7.0), pos2(x - 5.0, y), pos2(x + 5.0, y)],
            pal.ok,
            Stroke::NONE,
        ));
        plate(
            &p,
            pos2(x, y - 9.0),
            Align2::CENTER_BOTTOM,
            tf("ds.centre", &[("hz", &hz_text(st.events.last_hz))]),
            design::paint::ruler(),
            pal.ok,
        );
    }

    // The edges and their grips.
    let drag = app.fx.deesser.drag;
    let hover = resp.hover_pos();
    let near_x = |x: f32| hover.is_some_and(|pt| (pt.x - x).abs() < 7.0);
    for (hz, grab, align) in [
        (lo, Grab::Lo, Align2::LEFT_TOP),
        (hi, Grab::Hi, Align2::RIGHT_TOP),
    ] {
        let x = geo.spectrum_x(hz);
        let active = drag == Some(grab) || (drag.is_none() && near_x(x));
        p.vline(
            x,
            rect.y_range(),
            Stroke::new(if active { 2.5 } else { 1.5 }, pal.gold),
        );
        let grip = Rect::from_center_size(pos2(x, rect.top() + 10.0), vec2(8.0, 16.0));
        p.rect_filled(grip, 2.0, if active { pal.gold_bright } else { pal.gold });
        let dx = if align == Align2::LEFT_TOP { 7.0 } else { -7.0 };
        p.text(
            pos2(x + dx, rect.top() + 3.0),
            align,
            hz_text(hz),
            design::paint::ruler(),
            pal.gold,
        );
    }
    if let Some(pt) = hover {
        // The frequency under the pointer, beside the pointer.
        p.vline(pt.x, rect.y_range(), Stroke::new(1.0, pal.grid_bar));
        plate(
            &p,
            pos2(pt.x + 4.0, rect.bottom() - space::S6),
            Align2::LEFT_BOTTOM,
            hz_text(geo.spectrum_hz(pt.x)),
            design::paint::ruler(),
            pal.text,
        );
    }
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );

    // Drag: an edge, or the middle.
    if resp.drag_started() {
        // Where the button went down decides what is held.
        if let Some(pt) = ui.input(|i| i.pointer.press_origin()) {
            let (xl, xh) = (geo.spectrum_x(lo), geo.spectrum_x(hi));
            app.fx.deesser.drag = if (pt.x - xl).abs() < 7.0 {
                Some(Grab::Lo)
            } else if (pt.x - xh).abs() < 7.0 {
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
    if let (Some(g), Some(pt)) = (app.fx.deesser.drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let hz = geo.spectrum_hz(pt.x);
            match g {
                Grab::Lo => set_band(
                    app,
                    strip,
                    effect,
                    Some(hz.clamp(ds::MIN_HZ, hi / sixth)),
                    None,
                ),
                Grab::Hi => set_band(
                    app,
                    strip,
                    effect,
                    None,
                    Some(hz.clamp(lo * sixth, ds::MAX_HZ)),
                ),
                Grab::Band { lo, hi, at_hz } => {
                    let r = (hz / at_hz).clamp(ds::MIN_HZ / lo, ds::MAX_HZ / hi);
                    set_band(app, strip, effect, Some(lo * r), Some(hi * r));
                }
                Grab::Threshold => {}
            }
        }
    }
    if resp.drag_stopped() && app.fx.deesser.drag.is_some() {
        app.fx.deesser.drag = None;
        app.end_gesture();
    }
    if resp.double_clicked() {
        set_band(app, strip, effect, Some(5_000.0), Some(12_000.0));
        app.end_gesture();
    }
    if hover.is_some_and(|pt| {
        (pt.x - geo.spectrum_x(lo)).abs() < 7.0 || (pt.x - geo.spectrum_x(hi)).abs() < 7.0
    }) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
}

/// One knob bound to a parameter, with the de-esser's readouts.
fn knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    spec: &EffectSpec,
    key: &str,
    diameter: f32,
) {
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let mut v = spec.get(key);
    let k = Knob::new(&mut v, info.min..=info.max, info.default)
        .diameter(diameter)
        .log(info.logarithmic)
        .stepped(info.stepped)
        .unit(info.unit)
        .label(t(info.label_key));
    let k = match key {
        "look" => k.format(|x| {
            if x < 0.5 {
                t("ds.off")
            } else {
                format!("{x:.0} ms")
            }
        }),
        _ => k,
    };
    let r = ui.add(k);
    if r.changed() {
        if matches!(key, "lo" | "hi") {
            // Keep the pair ordered, as the display does.
            let other = spec.get(if key == "lo" { "hi" } else { "lo" });
            let sixth = 2f32.powf(1.0 / 6.0);
            v = if key == "lo" {
                v.min(other / sixth)
            } else {
                v.max(other * sixth)
            };
        }
        app.set_fx_param(strip, effect, key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
}

fn knobs(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    spec: &EffectSpec,
) {
    let voice = spec.get("mode") < 0.5;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(space::S1, space::S1);
        ui.horizontal(|ui| {
            for key in ["thr", "range"] {
                knob(app, ui, strip, effect, spec, key, space::GRID * 14.0);
            }
            ui.add_space(space::S2);
            for key in ["lo", "hi"] {
                knob(app, ui, strip, effect, spec, key, space::GRID * 9.0);
            }
        });
        ui.horizontal(|ui| {
            for key in ["att", "rel", "look", "link"] {
                knob(app, ui, strip, effect, spec, key, space::GRID * 9.0);
            }
            ui.add_enabled_ui(voice, |ui| {
                knob(app, ui, strip, effect, spec, "sens", space::GRID * 9.0);
            });
        });
    });
}

/// The inspector's picture: the band on a frequency axis and the settings in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let pal = theme::p();
    let s = tele::settings(spec);
    let (rect, resp) = ui.allocate_exact_size(
        vec2(ui.available_width(), space::GRID * 14.0),
        Sense::click(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let axis = Rect::from_min_max(
        pos2(rect.left() + space::S2, rect.bottom() - space::S6),
        pos2(rect.right() - space::S2, rect.bottom() - space::S2),
    );
    let x = |hz: f32| axis.left() + (hz / 1_000.0).ln() / (20.0f32).ln() * axis.width();
    let (lo, hi) = ds::band_edges(s.lo_hz, s.hi_hz, DEFAULT_SR);
    p.hline(
        axis.x_range(),
        axis.bottom(),
        Stroke::new(1.0, pal.grid_strong),
    );
    let band = Rect::from_min_max(pos2(x(lo), axis.top()), pos2(x(hi), axis.bottom()));
    p.rect_filled(band, 1.0, pal.gold.gamma_multiply(0.35));
    p.rect_stroke(band, 1.0, Stroke::new(1.0, pal.gold), StrokeKind::Inside);
    for hz in [1_000.0f32, 5_000.0, 10_000.0, 20_000.0] {
        p.vline(
            x(hz),
            axis.bottom() - 3.0..=axis.bottom(),
            Stroke::new(1.0, pal.text_dim),
        );
    }
    let words = format!(
        "{:+.0} dB \u{b7} {:.0} dB \u{b7} {} \u{b7} {}",
        s.threshold_db,
        s.range_db,
        t(&format!("ds.mode.{}", s.detection.index())),
        t(&format!("ds.proc.{}", s.processing.index())),
    );
    p.text(
        pos2(rect.left() + space::S2, rect.top() + space::S2),
        Align2::LEFT_TOP,
        words,
        design::paint::ruler(),
        pal.text,
    );
    p.text(
        pos2(rect.right() - space::S2, rect.top() + space::S2),
        Align2::RIGHT_TOP,
        format!("{}\u{2013}{}", hz_text(lo), hz_text(hi)),
        design::paint::ruler(),
        pal.gold,
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
    resp.on_hover_text(t("ds.open_tip"))
}

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
    let Some(spec) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .map(|e| e.spec.clone())
    else {
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
