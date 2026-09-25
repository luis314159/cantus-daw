//! The Aequalis editor: the EQ's own window, where the display *is* the control.
//!
//! What the research on the benchmark EQ said users value, and what this does about it
//! (`research/fabfilter/pro-q.md`, "How we beat it"; `docs/plugins/eq.md` has the matrix):
//!
//! - **The display is the instrument.** Double-click to make a band — its shape follows
//!   where you clicked (cuts at the edges, shelves near them, a notch at the floor, a bell
//!   anywhere else) — or press on the curve and pull. Drag a dot (every selected dot moves
//!   with it, relatively), drag a rectangle to select several, the wheel for Q (slope on a
//!   cut; `Ctrl` for gain, `Alt` for the dynamic range), `Shift` for fine, `Alt` to lock the
//!   axis the drag started on, `Alt`+click to bypass, double-click a dot to type exact
//!   values (`2k`, `A4`, `C#2+13`, `2x`), right-click for the band's menu.
//! - **The axes are controls too.** Drag the frequency axis up or down to zoom around the
//!   pointer, sideways to pan, double-click to reset; the dB range widens itself when a
//!   band is dragged past it; a piano replaces the axis on request, and then a drag moves in
//!   semitones.
//! - **State is visible at rest.** The processing mode and its latency, the analyser's
//!   sources and freeze, the key strip, the auditioned band: all visible chips, not hover
//!   panels. The selected band's numbers are on a plate beside its dot and in the panel
//!   under the display.
//! - **Dynamics in real units** (threshold, range, ratio, attack and release in dB and ms),
//!   in both directions, from the band or from the key strip, with the last seconds of the
//!   band's movement drawn against its threshold — which you drag.
//! - **The host shows what a plug-in cannot**: the key strip's spectrum over this one, where
//!   they collide (hatched), and an EQ match to it in one click, fitted into editable bands.
//! - **Keyboard**: 1–9 and 0 select bands, Tab steps, arrows nudge (semitones and dB),
//!   `[`/`]` change Q (or the slope), Delete removes, B bypasses, S auditions, D makes a band
//!   dynamic, Enter types values, Esc backs out.
//! - **Every gesture is one step of the project's undo**, the multi-band drag included.
//!
//! The curve is drawn from the same design the audio thread runs (`cantus_dsp::aequalis`),
//! in the mode it runs; the spectra come from the engine's analyser tap on this effect's
//! input and output, and the key's from the unit's telemetry.

#[path = "eq_editor_display.rs"]
mod display;
#[path = "eq_editor_panel.rs"]
mod panel;
#[path = "eq_editor_parse.rs"]
pub mod parse;

use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use crate::widgets::Knob;
use cantus_core::{
    eq_key, Command, EffectId, EffectSlot, MergeKey, StripId, EQ_BANDS, EQ_BAND_FIELDS,
};
use cantus_dsp::aequalis::fit::{Fit, FitOptions};
use cantus_dsp::aequalis::{self, latency_of, Mode, Resolution};
use cantus_dsp::{BandSettings, EqShape, Placement, SpectrumAnalyser};
use egui::{pos2, vec2, Align2, Color32, Pos2, RichText, Sense, Stroke, StrokeKind};
use std::collections::{BTreeSet, VecDeque};

/// Points across the display at which curves and spectra are evaluated.
const POINTS: usize = 320;
/// Columns of dynamics history kept: one a frame, about six seconds.
const HISTORY: usize = 360;
/// The dB ranges the display offers; a drag past the range steps up through them.
pub const RANGES: [f32; 5] = [3.0, 6.0, 12.0, 18.0, 30.0];
/// The frequency view the display opens on, and how far it can zoom out.
pub const VIEW: (f32, f32) = (20.0, 20_000.0);
const VIEW_LIMITS: (f32, f32) = (10.0, 24_000.0);

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
}

/// What the editor remembers between frames.
#[derive(Debug)]
pub struct EqEditor {
    /// The EQ being edited, if the window is open.
    pub open: Option<(StripId, EffectId)>,
    /// The selected bands.
    pub selection: BTreeSet<usize>,
    /// The band whose controls the panel shows: the last one clicked.
    pub primary: Option<usize>,
    /// The display's gain range, ± dB.
    pub range_db: f32,
    /// The display's frequency view, Hz.
    pub view: (f32, f32),
    /// A piano instead of the frequency axis; drags move in semitones.
    pub piano: bool,
    pub show: AnalyserView,
    analyser: SpectrumAnalyser,
    scratch: Vec<f32>,
    /// The drawn spectra per display point, dB on the analyser scale (tilted).
    pre: Vec<f32>,
    post: Vec<f32>,
    key: Vec<f32>,
    last: std::time::Instant,
    drag: Option<display::Drag>,
    rubber: Option<(Pos2, Pos2)>,
    axis: Option<display::AxisDrag>,
    pub typing: Option<Typing>,
    /// Per frame: each band's dynamic gain change and its detector's level, dB.
    history: VecDeque<([f32; EQ_BANDS], [f32; EQ_BANDS])>,
    /// Numbers each gesture's merge key, so a gesture is one undo step and the next is
    /// another.
    gesture: u64,
    nudge_at: Option<std::time::Instant>,
    pub matching: Option<MatchState>,
    /// Telemetry injected by demos and tests, read instead of the engine's.
    pub injected: Option<[f32; 64]>,
    /// Each band's curve across the display, kept while nothing it depends on changes.
    curves: Vec<Option<display::CachedCurve>>,
    /// When the last wheel turn or key nudge landed: a burst within 500 ms is one step.
    burst_at: Option<std::time::Instant>,
    /// Where the plot was drawn last frame, and the view it was drawn with — so tests and
    /// tools can aim at what is on screen instead of mirroring the layout.
    pub plot: Option<egui::Rect>,
    plot_view: (f32, f32),
}

impl Default for EqEditor {
    fn default() -> Self {
        EqEditor {
            open: None,
            selection: BTreeSet::new(),
            primary: None,
            range_db: 18.0,
            view: VIEW,
            piano: false,
            show: AnalyserView::default(),
            analyser: SpectrumAnalyser::new(4096),
            scratch: Vec::new(),
            pre: vec![-200.0; POINTS],
            post: vec![-200.0; POINTS],
            key: vec![-200.0; POINTS],
            last: std::time::Instant::now(),
            drag: None,
            rubber: None,
            axis: None,
            typing: None,
            history: VecDeque::with_capacity(HISTORY),
            gesture: 0,
            nudge_at: None,
            matching: None,
            injected: None,
            curves: vec![None; EQ_BANDS + 2],
            burst_at: None,
            plot: None,
            plot_view: VIEW,
        }
    }
}

/// A chip: framed whether it is on or off, filled when on — so a switch that is off still
/// reads as a switch, and its state is a fill, not a hover (`research/fabfilter/family.md`
/// 7.2).
pub(crate) fn chip(ui: &mut egui::Ui, on: bool, text: impl Into<String>) -> egui::Response {
    ui.add(egui::Button::selectable(on, text.into()).frame_when_inactive(true))
}

/// A chip bound to a switch.
pub(crate) fn toggle(
    ui: &mut egui::Ui,
    value: &mut bool,
    text: impl Into<String>,
) -> egui::Response {
    let mut r = chip(ui, *value, text);
    if r.clicked() {
        *value = !*value;
        r.mark_changed();
    }
    r
}

/// A colour per band number, from the track palette: a band keeps its colour for life.
pub fn band_color(band: usize) -> Color32 {
    theme::track_color(cantus_core::Color::PALETTE[band % cantus_core::Color::PALETTE.len()])
}

/// `1.2 kHz`, `85 Hz`.
fn hz_label(hz: f32) -> String {
    crate::widgets::knob::readout(hz, "Hz")
}

impl EqEditor {
    /// Feed the analyser (input, output) pairs directly — what the engine's tap does, for
    /// screenshots and tests with no engine.
    pub fn feed(&mut self, pairs: &[f32]) {
        self.analyser.push(pairs);
    }

    /// Add one frame of dynamics history directly (demos and tests).
    pub fn feed_history(&mut self, deltas: [f32; EQ_BANDS], levels: [f32; EQ_BANDS]) {
        if self.history.len() == HISTORY {
            self.history.pop_front();
        }
        self.history.push_back((deltas, levels));
    }

    /// Where band `band`'s dot was drawn last frame.
    pub fn dot_pos(&self, spec: &cantus_core::EffectSpec, band: usize) -> Option<Pos2> {
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

    /// Select exactly `bands`; the first becomes the one the panel shows.
    pub fn select(&mut self, bands: &[usize]) {
        self.selection = bands.iter().copied().collect();
        self.primary = bands.first().copied();
    }

    fn next_gesture(&mut self, effect: EffectId) -> MergeKey {
        self.gesture += 1;
        MergeKey::of(&("eq-gesture", effect.0, self.gesture))
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
}

/// The fields a new band of `shape` is created with.
fn new_band_fields(shape: EqShape, freq: f32, gain: f32) -> Vec<(&'static str, f32)> {
    let mut fields = vec![
        ("used", 1.0),
        ("shape", shape.index() as f32),
        ("freq", freq),
    ];
    if shape.has_gain() {
        fields.push(("gain", gain));
    }
    if matches!(
        shape,
        EqShape::LowShelf | EqShape::HighShelf | EqShape::TiltShelf
    ) {
        fields.push(("q", 0.71));
    }
    if shape.has_slope() {
        fields.push(("q", 0.71));
        fields.push(("slope", 24.0));
    }
    fields
}

impl CantusApp {
    /// Open an EQ's editor and point the analyser at it.
    pub fn open_eq_editor(&mut self, strip: StripId, effect: EffectId) {
        self.comp_editor.open = None;
        let ed = &mut self.eq_editor;
        ed.open = Some((strip, effect));
        ed.typing = None;
        ed.drag = None;
        ed.matching = None;
        ed.history.clear();
        ed.pre.fill(-200.0);
        ed.post.fill(-200.0);
        ed.key.fill(-200.0);
        // Open on the first band there is, so its numbers are on screen at once.
        let first = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .and_then(|e| (0..EQ_BANDS).find(|b| e.spec.get(&eq_key(*b, "used")) >= 0.5));
        self.eq_editor.select(first.as_slice());
        self.engine.set_analyser(Some(effect));
    }

    /// Close the editor. An audition in progress ends with it — a solo left on in a saved
    /// project would be a trap.
    pub fn close_eq_editor(&mut self) {
        if let Some((strip, effect)) = self.eq_editor.open.take() {
            let soloed = self
                .project()
                .strip(strip)
                .and_then(|s| s.effect(effect))
                .is_some_and(|e| e.spec.get("solo") >= 1.0);
            if soloed {
                self.set_eq_solo(strip, effect, None);
            }
            self.engine.set_analyser(None);
        }
    }

    /// Move the analyser's samples into the editor. Called once a frame.
    pub(crate) fn pump_analyser(&mut self) {
        if self.fx.open.is_some() {
            let mut pairs = std::mem::take(&mut self.eq_editor.scratch);
            pairs.clear();
            self.engine.drain_analyser(&mut pairs);
            self.pump_fx_pairs(&pairs);
            self.eq_editor.scratch = pairs;
            return;
        }
        if self.eq_editor.open.is_none() {
            return;
        }
        let ed = &mut self.eq_editor;
        ed.scratch.clear();
        self.engine.drain_analyser(&mut ed.scratch);
        if !ed.scratch.is_empty() {
            ed.analyser.push(&ed.scratch);
        }
    }

    /// The open EQ's telemetry: injected (demos, tests) or the engine's.
    fn eq_telemetry(&self) -> [f32; 64] {
        self.eq_editor
            .injected
            .unwrap_or_else(|| self.engine.meters().fx)
    }

    /// The sample rate the display designs at: the engine's.
    fn eq_sample_rate(&self) -> f32 {
        let sr = self.engine.sample_rate as f32;
        if sr > 1_000.0 {
            sr
        } else {
            48_000.0
        }
    }

    /// Run commands as one step; with a gesture key, merged into the gesture's step.
    fn eq_exec(&mut self, label: &str, commands: Vec<Command>, gesture: Option<MergeKey>) {
        if commands.is_empty() {
            return;
        }
        let cmd = Command::Batch {
            label_key: label.into(),
            commands,
        };
        match gesture {
            Some(key) => {
                self.exec_merging(cmd, key);
            }
            None => {
                self.exec(cmd);
            }
        }
    }

    fn band_cmds(
        strip: StripId,
        effect: EffectId,
        band: usize,
        fields: &[(&str, f32)],
    ) -> Vec<Command> {
        fields
            .iter()
            .map(|(f, v)| Command::SetEffectParam {
                strip,
                effect,
                key: eq_key(band, f),
                value: *v,
            })
            .collect()
    }

    /// Set fields of one band as one command; a gesture merges frame to frame.
    fn set_band(
        &mut self,
        strip: StripId,
        effect: EffectId,
        band: usize,
        fields: &[(&str, f32)],
        gesture: bool,
    ) {
        let key = gesture.then(|| {
            let names: Vec<&str> = fields.iter().map(|(f, _)| *f).collect();
            MergeKey::of(&("eq", effect.0, band, names))
        });
        self.eq_exec(
            "cmd.eq_band",
            Self::band_cmds(strip, effect, band, fields),
            key,
        );
    }

    /// Set one global parameter of the EQ (mode, output, …).
    fn set_eq_global(&mut self, strip: StripId, effect: EffectId, key: &str, value: f32) {
        self.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        });
    }

    /// The first band number not in use.
    fn free_band(&self, strip: StripId, effect: EffectId) -> Option<usize> {
        let spec = &self.project().strip(strip)?.effect(effect)?.spec;
        (0..EQ_BANDS).find(|b| spec.get(&eq_key(*b, "used")) < 0.5)
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
        self.add_eq_band_with(strip, effect, shape, freq, gain, &[], None)
    }

    /// Make a band with extra fields, as one step (or as the start of a gesture).
    #[allow(clippy::too_many_arguments)]
    fn add_eq_band_with(
        &mut self,
        strip: StripId,
        effect: EffectId,
        shape: EqShape,
        freq: f32,
        gain: f32,
        extra: &[(&'static str, f32)],
        gesture: Option<MergeKey>,
    ) -> Option<usize> {
        let band = self.free_band(strip, effect)?;
        let mut fields = new_band_fields(shape, freq, gain);
        fields.extend_from_slice(extra);
        self.eq_exec(
            "cmd.eq_band",
            Self::band_cmds(strip, effect, band, &fields),
            gesture,
        );
        self.eq_editor.select(&[band]);
        Some(band)
    }

    /// Remove a band: every field it stored goes, so the number is free and the file
    /// forgets it. One undo step.
    pub fn remove_eq_band(&mut self, strip: StripId, effect: EffectId, band: usize) {
        self.remove_eq_bands(strip, effect, &[band]);
    }

    /// Remove several bands as one undo step.
    pub fn remove_eq_bands(&mut self, strip: StripId, effect: EffectId, bands: &[usize]) {
        let solo = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.get("solo").round() as usize)
            .unwrap_or(0);
        let mut commands: Vec<Command> = Vec::new();
        for band in bands {
            commands.extend(EQ_BAND_FIELDS.iter().map(|f| Command::ClearEffectParam {
                strip,
                effect,
                key: eq_key(*band, f.0),
            }));
            // Removing the auditioned band ends the audition.
            if solo == band + 1 {
                commands.push(Command::ClearEffectParam {
                    strip,
                    effect,
                    key: "solo".into(),
                });
            }
        }
        self.eq_exec("cmd.eq_band_remove", commands, None);
        let ed = &mut self.eq_editor;
        for b in bands {
            ed.selection.remove(b);
        }
        if ed.primary.is_some_and(|p| bands.contains(&p)) {
            ed.primary = ed.selection.iter().next().copied();
        }
    }

    /// Audition one band (the region it acts on, alone), or stop auditioning.
    pub fn set_eq_solo(&mut self, strip: StripId, effect: EffectId, band: Option<usize>) {
        self.set_eq_global(strip, effect, "solo", band.map_or(0.0, |b| b as f32 + 1.0));
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
        let spec = self.project().strip(strip)?.effect(effect)?.spec.clone();
        let copy = self.free_band(strip, effect)?;
        let (a, b) = if mid_side {
            (Placement::Mid, Placement::Side)
        } else {
            (Placement::Left, Placement::Right)
        };
        let mut commands = Self::band_cmds(strip, effect, band, &[("place", a.index() as f32)]);
        for (key, ..) in EQ_BAND_FIELDS {
            let k = eq_key(band, key);
            if let Some(v) = spec.params.get(&k) {
                let v = if *key == "place" {
                    b.index() as f32
                } else {
                    *v
                };
                commands.push(Command::SetEffectParam {
                    strip,
                    effect,
                    key: eq_key(copy, key),
                    value: v,
                });
            }
        }
        if !spec.params.contains_key(&eq_key(band, "place")) {
            commands.push(Command::SetEffectParam {
                strip,
                effect,
                key: eq_key(copy, "place"),
                value: b.index() as f32,
            });
        }
        self.eq_exec("cmd.eq_split", commands, None);
        self.eq_editor.select(&[band, copy]);
        Some(copy)
    }

    /// Put a fit's bands on the EQ, at free numbers, as one undo step.
    pub fn apply_eq_fit(&mut self, strip: StripId, effect: EffectId, fit: &Fit) -> Vec<usize> {
        let Some(spec) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.clone())
        else {
            return Vec::new();
        };
        let mut free = (0..EQ_BANDS).filter(|b| spec.get(&eq_key(*b, "used")) < 0.5);
        let mut commands = Vec::new();
        let mut made = Vec::new();
        for b in &fit.bands {
            let Some(n) = free.next() else { break };
            let mut fields = new_band_fields(b.shape, b.freq, b.gain_db);
            fields.retain(|(k, _)| *k != "q");
            fields.push(("q", b.q));
            commands.extend(Self::band_cmds(strip, effect, n, &fields));
            made.push(n);
        }
        self.eq_exec("cmd.eq_match", commands, None);
        self.eq_editor.select(&made);
        made
    }

    /// Move the selected bands by `semitones` and `db`, as one step (or merged into a run
    /// of key presses).
    fn eq_nudge_selection(&mut self, strip: StripId, effect: EffectId, semitones: f32, db: f32) {
        let Some(spec) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.clone())
        else {
            return;
        };
        let now = std::time::Instant::now();
        let merge = self
            .eq_editor
            .nudge_at
            .is_some_and(|t| now.duration_since(t).as_millis() < 600);
        self.eq_editor.nudge_at = Some(now);
        if !merge {
            self.end_gesture();
            self.eq_editor.gesture += 1;
        }
        let key = MergeKey::of(&("eq-nudge", effect.0, self.eq_editor.gesture));
        let mut commands = Vec::new();
        let mut widest = 0.0f32;
        for b in self.eq_editor.selection.clone() {
            let f = spec.get(&eq_key(b, "freq"));
            let shape = EqShape::from_index(spec.get(&eq_key(b, "shape")) as usize);
            let mut fields = vec![(
                "freq",
                (f * 2f32.powf(semitones / 12.0)).clamp(10.0, 30_000.0),
            )];
            if shape.has_gain() {
                let g = (spec.get(&eq_key(b, "gain")) + db).clamp(-30.0, 30.0);
                widest = widest.max(g.abs());
                fields.push(("gain", g));
            }
            commands.extend(Self::band_cmds(strip, effect, b, &fields));
        }
        self.eq_editor.range_db = self.eq_editor.range_for(widest);
        self.eq_exec("cmd.eq_band", commands, Some(key));
    }

    /// Multiply the selected bands' Q by `factor` (cuts: step the slope instead).
    fn eq_scale_selection_q(&mut self, strip: StripId, effect: EffectId, factor: f32) {
        let Some(spec) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.clone())
        else {
            return;
        };
        let mut commands = Vec::new();
        for b in self.eq_editor.selection.clone() {
            let shape = EqShape::from_index(spec.get(&eq_key(b, "shape")) as usize);
            if shape.has_slope() {
                let s = spec.get(&eq_key(b, "slope"));
                let s = (s + if factor > 1.0 { 6.0 } else { -6.0 }).clamp(6.0, 96.0);
                commands.extend(Self::band_cmds(strip, effect, b, &[("slope", s)]));
            } else {
                let q = (spec.get(&eq_key(b, "q")) * factor).clamp(0.025, 40.0);
                commands.extend(Self::band_cmds(strip, effect, b, &[("q", q)]));
            }
        }
        self.eq_exec("cmd.eq_band", commands, None);
    }

    /// Flip a switch-like field (`on`, `dyn`) on every selected band, all to the same new
    /// value (the opposite of the primary band's).
    fn eq_toggle_selection(&mut self, strip: StripId, effect: EffectId, field: &str) {
        let Some(spec) = self
            .project()
            .strip(strip)
            .and_then(|s| s.effect(effect))
            .map(|e| e.spec.clone())
        else {
            return;
        };
        let Some(first) = self
            .eq_editor
            .primary
            .or(self.eq_editor.selection.first().copied())
        else {
            return;
        };
        let to = if spec.get(&eq_key(first, field)) >= 0.5 {
            0.0
        } else {
            1.0
        };
        let mut commands = Vec::new();
        for b in self.eq_editor.selection.clone() {
            commands.extend(Self::band_cmds(strip, effect, b, &[(field, to)]));
        }
        self.eq_exec("cmd.eq_band", commands, None);
    }
}

/// The editor window, drawn every frame while one is open.
pub fn window(app: &mut CantusApp, ctx: &egui::Context) {
    let Some((strip, effect)) = app.eq_editor.open else {
        return;
    };
    let Some(slot) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .cloned()
    else {
        app.close_eq_editor();
        return;
    };
    // Drop selections of bands that no longer exist (undo, presets, A/B).
    let used = |b: &usize| slot.spec.get(&eq_key(*b, "used")) >= 0.5;
    app.eq_editor.selection.retain(used);
    if app.eq_editor.primary.is_some_and(|p| !used(&p)) {
        app.eq_editor.primary = app.eq_editor.selection.iter().next().copied();
    }
    if app
        .eq_editor
        .typing
        .as_ref()
        .is_some_and(|t| !used(&t.band))
    {
        app.eq_editor.typing = None;
    }
    // One column of dynamics history a frame.
    let tel = app.eq_telemetry();
    if app.eq_editor.injected.is_none() {
        let deltas = app.engine.meters().eq_deltas;
        let mut d = [0.0; EQ_BANDS];
        d.copy_from_slice(&deltas[..EQ_BANDS]);
        let mut levels = [-200.0; EQ_BANDS];
        levels.copy_from_slice(
            &tel[aequalis::telemetry::LEVEL..aequalis::telemetry::LEVEL + EQ_BANDS],
        );
        app.eq_editor.feed_history(d, levels);
    }
    let strip_name = app.capture_label(cantus_engine::CaptureTarget::Strip(strip));
    let mut open = true;
    egui::Window::new(format!("{} \u{2014} {strip_name}", t("effect.eq")))
        .id(egui::Id::new(("eq-editor", effect.0)))
        .open(&mut open)
        .default_size([1120.0, 720.0])
        .min_size([640.0, 480.0])
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            header(app, ui, strip, effect, &slot);
            ui.add_space(space::S1);
            toolbar(app, ui, strip, effect, &slot, &tel);
            ui.add_space(space::S1);
            let bands = cantus_engine::eq_bands(&slot.spec);
            let panel_h = space::GRID * 36.0;
            let display_h = (ui.available_height() - panel_h).max(space::GRID * 44.0);
            display::display(app, ui, strip, effect, &slot, &bands, &tel, display_h);
            ui.add_space(space::S2);
            if app.eq_editor.matching.is_some() {
                panel::match_panel(app, ui, strip, effect, &slot, &tel);
            } else {
                panel::band_panel(app, ui, strip, effect, &slot, &bands, &tel);
            }
        });
    ctx.request_repaint();
    if !open {
        app.close_eq_editor();
    }
}

/// Top bar: which strip, presets, A/B, the band count, the output.
fn header(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let pal = theme::p();
    ui.horizontal(|ui| {
        let (chip, _) = ui.allocate_exact_size(vec2(space::S3, space::S3), Sense::hover());
        let color = app
            .project()
            .tracks_on_strip(strip)
            .next()
            .map(|t| theme::track_color(t.color))
            .unwrap_or(pal.gold);
        ui.painter().rect_filled(chip, pal.corner_radius, color);
        let number = app.project().strip_number(strip).unwrap_or(0);
        ui.label(
            RichText::new(tf("inspector.strip_n", &[("n", &number)])).text_style(design::strong()),
        );
        ui.separator();
        crate::views::fx::presets_menu(app, ui, strip, effect, slot);
        crate::views::fx::ab_bar(app, ui, strip, effect, slot);
        ui.separator();
        let used = (0..EQ_BANDS)
            .filter(|b| slot.spec.get(&eq_key(*b, "used")) >= 0.5)
            .count();
        ui.label(
            RichText::new(tf("eq.bands", &[("n", &used), ("max", &EQ_BANDS)]))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let mut on = !slot.bypass;
            if toggle(ui, &mut on, t("eq.active"))
                .on_hover_text(t("effect.bypass"))
                .changed()
            {
                app.exec(Command::SetEffectBypass {
                    strip,
                    effect,
                    bypass: !on,
                });
            }
            for key in ["out", "scale"] {
                let info = slot.spec.kind.param(key).copied();
                let Some(info) = info else { continue };
                let mut v = slot.spec.get(key);
                let knob = Knob::new(&mut v, info.min..=info.max, info.default)
                    .diameter(space::GRID * 6.0)
                    .unit(info.unit)
                    .label(t(info.label_key));
                let r = ui.add(knob).on_hover_text(t(&format!("eq.{key}_tip")));
                if r.changed() {
                    app.exec_merging(
                        Command::SetEffectParam {
                            strip,
                            effect,
                            key: key.into(),
                            value: v,
                        },
                        MergeKey::of(&("eq-global", effect.0, key)),
                    );
                }
                if r.drag_stopped() || r.double_clicked() {
                    app.end_gesture();
                }
            }
            let mut auto = slot.spec.get("auto") >= 0.5;
            if toggle(ui, &mut auto, t("fx.eq.auto"))
                .on_hover_text(t("eq.auto_tip"))
                .changed()
            {
                app.set_eq_global(strip, effect, "auto", if auto { 1.0 } else { 0.0 });
            }
        });
    });
}

/// The strip of chips under the header: processing mode and latency, the analyser's
/// sources and options, the key strip, EQ match, and the display's own options. Every
/// state is visible here at rest.
fn toolbar(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    tel: &[f32; 64],
) {
    let pal = theme::p();
    let sr = app.eq_sample_rate();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = space::S1;
        // Processing mode, as a segmented control, and the latency it costs.
        let mode = Mode::from_index(slot.spec.get("mode").round() as usize);
        for m in Mode::ALL {
            if chip(ui, mode == m, t(&format!("eq.mode.{}", m.index())))
                .on_hover_text(t(&format!("eq.mode.{}_tip", m.index())))
                .clicked()
                && mode != m
            {
                app.set_eq_global(strip, effect, "mode", m.index() as f32);
            }
        }
        let res = Resolution::from_index(slot.spec.get("lp_res").round() as usize);
        if mode == Mode::LinearPhase {
            egui::ComboBox::from_id_salt(("eq-res", effect.0))
                .selected_text(t(&format!("eq.res.{}", res.index())))
                .width(space::GRID * 22.0)
                .show_ui(ui, |ui| {
                    for r in Resolution::ALL {
                        let lat = latency_of(Mode::LinearPhase, r, sr);
                        let text = format!(
                            "{} \u{b7} {:.0} ms",
                            t(&format!("eq.res.{}", r.index())),
                            lat as f32 / sr * 1_000.0
                        );
                        if ui.selectable_label(res == r, text).clicked() {
                            app.set_eq_global(strip, effect, "lp_res", r.index() as f32);
                        }
                    }
                })
                .response
                .on_hover_text(t("eq.res_tip"));
        }
        latency_badge(ui, latency_of(mode, res, sr), sr);
        ui.separator();
        // The analyser: sources, then what it does.
        let ed = &mut app.eq_editor;
        ui.label(
            RichText::new(t("eq.analyser"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        let key_on = tel[aequalis::telemetry::KEY_ON] >= 0.5 || slot.sidechain.is_some();
        toggle(ui, &mut ed.show.pre, t("eq.an.pre")).on_hover_text(t("eq.an.pre_tip"));
        toggle(ui, &mut ed.show.post, t("eq.an.post")).on_hover_text(t("eq.an.post_tip"));
        ui.add_enabled_ui(key_on, |ui| {
            toggle(ui, &mut ed.show.key, t("eq.an.key"))
                .on_hover_text(t("eq.an.key_tip"))
                .on_disabled_hover_text(t("eq.an.no_key"));
        });
        toggle(ui, &mut ed.show.freeze, t("eq.an.freeze")).on_hover_text(t("eq.an.freeze_tip"));
        toggle(ui, &mut ed.show.grab, t("eq.an.grab")).on_hover_text(t("eq.an.grab_tip"));
        ui.add_enabled_ui(key_on, |ui| {
            toggle(ui, &mut ed.show.collisions, t("eq.an.collisions"))
                .on_hover_text(t("eq.an.collisions_tip"))
                .on_disabled_hover_text(t("eq.an.no_key"));
        });
        analyser_menu(app, ui);
        ui.separator();
        key_picker(app, ui, strip, effect, slot);
        let matching = app.eq_editor.matching.is_some();
        if chip(ui, matching, t("eq.match"))
            .on_hover_text(t("eq.match_tip"))
            .clicked()
        {
            app.eq_editor.matching = if matching {
                None
            } else {
                Some(MatchState::default())
            };
        }
        ui.separator();
        let ed = &mut app.eq_editor;
        toggle(ui, &mut ed.piano, t("eq.piano")).on_hover_text(t("eq.piano_tip"));
        egui::ComboBox::from_id_salt(("eq-range", effect.0))
            .selected_text(format!("\u{b1}{:.0} dB", ed.range_db))
            .width(space::GRID * 18.0)
            .show_ui(ui, |ui| {
                for r in RANGES {
                    ui.selectable_value(&mut ed.range_db, r, format!("\u{b1}{r:.0} dB"));
                }
            })
            .response
            .on_hover_text(t("eq.range_tip"));
    });
}

/// "Latency 53.3 ms · 2560 smp", or "No latency" — always on screen, never a surprise.
fn latency_badge(ui: &mut egui::Ui, samples: usize, sr: f32) {
    let pal = theme::p();
    let (text, color) = if samples == 0 {
        (t("eq.latency_none"), pal.text_dim)
    } else {
        (
            tf(
                "eq.latency",
                &[
                    ("ms", &format!("{:.1}", samples as f32 / sr * 1_000.0)),
                    ("n", &samples),
                ],
            ),
            pal.text,
        )
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text, design::paint::ruler(), color);
    let size = galley.size() + vec2(space::S3, space::S1 * 1.5);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let p = ui.painter();
    p.rect(
        rect,
        pal.corner_radius,
        if samples == 0 {
            Color32::TRANSPARENT
        } else {
            theme::recess(&pal)
        },
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
    p.galley(rect.center() - galley.size() * 0.5, galley, color);
    resp.on_hover_text(t("eq.latency_tip"));
}

fn analyser_menu(app: &mut CantusApp, ui: &mut egui::Ui) {
    let ed = &mut app.eq_editor;
    let s = ed.show;
    let summary = format!(
        "{} \u{b7} {} \u{b7} {:.0} dB \u{b7} {:.1} dB/oct",
        t(&format!("eq.an.res.{}", s.resolution)),
        t(&format!("eq.an.speed.{}", s.speed)),
        s.range_db,
        s.tilt
    );
    ui.menu_button(summary, |ui| {
        ui.label(
            RichText::new(t("eq.an.resolution"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        for r in 0..4 {
            ui.radio_value(
                &mut ed.show.resolution,
                r,
                format!("{} \u{b7} {}", t(&format!("eq.an.res.{r}")), 1024 << r),
            );
        }
        ui.separator();
        ui.label(
            RichText::new(t("eq.an.speed"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        for sp in 0..5 {
            ui.radio_value(&mut ed.show.speed, sp, t(&format!("eq.an.speed.{sp}")));
        }
        ui.separator();
        ui.label(
            RichText::new(t("eq.an.range"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        for r in [60.0, 90.0, 120.0] {
            ui.radio_value(&mut ed.show.range_db, r, format!("{r:.0} dB"));
        }
        ui.separator();
        ui.label(
            RichText::new(t("eq.an.tilt"))
                .text_style(design::caption())
                .color(theme::p().text_dim),
        );
        for tl in [0.0, 3.0, 4.5, 6.0] {
            ui.radio_value(&mut ed.show.tilt, tl, format!("{tl:.1} dB/oct"));
        }
    })
    .response
    .on_hover_text(t("eq.an.settings_tip"));
    if ed.analyser.size() != ed.show.fft_size() {
        ed.analyser = SpectrumAnalyser::new(ed.show.fft_size());
    }
}

/// The key strip: whose output the key-listening bands, the external spectrum, the
/// collisions and EQ match hear. Chosen here, from the desk, with no send to set up.
fn key_picker(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
) {
    let pal = theme::p();
    ui.label(
        RichText::new(t("eq.key"))
            .text_style(design::caption())
            .color(pal.text_dim),
    );
    let current = slot.sidechain;
    let label = match current {
        Some(k) => app.capture_label(cantus_engine::CaptureTarget::Strip(k)),
        None => t("eq.key_none"),
    };
    egui::ComboBox::from_id_salt(("eq-key", effect.0))
        .selected_text(label)
        .width(space::GRID * 36.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(current.is_none(), t("eq.key_none"))
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
        .on_hover_text(t("eq.key_tip"));
}

/// A small, read-only picture of an EQ's curve, for the inspector's effect card: the curve
/// as the mode realises it, the dots, and the mode's latency when it has one.
pub fn mini_curve(ui: &mut egui::Ui, spec: &cantus_core::EffectSpec) -> egui::Response {
    let pal = theme::p();
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, space::GRID * 14.0), Sense::click());
    let geo = display::Geo::new(rect, 18.0, VIEW);
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    p.hline(
        rect.x_range(),
        geo.y(0.0),
        Stroke::new(1.0, pal.grid_strong),
    );
    let bands = cantus_engine::eq_bands(spec);
    let mode = Mode::from_index(spec.get("mode").round() as usize);
    let sr = 48_000.0;
    let n = POINTS / 2;
    let hz: Vec<f32> = (0..n).map(|k| geo.hz_at(k, n)).collect();
    let mut total = vec![0.0f32; n];
    let mut one = vec![0.0f32; n];
    for b in bands.iter().filter(|b| b.active) {
        aequalis::band_curve_db(b, mode, sr, &hz, &mut one);
        for (t, o) in total.iter_mut().zip(&one) {
            *t += o;
        }
    }
    let pts: Vec<Pos2> = hz
        .iter()
        .zip(&total)
        .map(|(f, db)| pos2(geo.x(*f), geo.y(*db)))
        .collect();
    p.add(egui::Shape::line(pts, Stroke::new(1.5, pal.gold)));
    for (i, b) in bands.iter().enumerate().filter(|(_, b)| b.active) {
        p.circle_filled(geo.dot(b), 3.0, band_color(i));
    }
    if mode != Mode::ZeroLatency {
        let res = Resolution::from_index(spec.get("lp_res").round() as usize);
        let lat = latency_of(mode, res, sr);
        p.text(
            pos2(rect.right() - space::S2, rect.top() + space::S1),
            Align2::RIGHT_TOP,
            format!(
                "{} \u{b7} {:.1} ms",
                t(&format!("eq.mode.{}", mode.index())),
                lat as f32 / sr * 1_000.0
            ),
            design::paint::ruler(),
            pal.text_dim,
        );
    }
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
    resp.on_hover_text(t("eq.open_tip"))
}

// ---- demo --------------------------------------------------------------------------------

/// `--editor eq` and the screenshots: on top of the four bands the app's demo makes, a
/// dynamic de-esser band, a side-only shelf, a key strip, and a two-band selection.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    app.add_eq_band_with(
        strip,
        effect,
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
    app.add_eq_band_with(
        strip,
        effect,
        EqShape::HighShelf,
        14_000.0,
        -2.5,
        &[("place", 4.0)],
        None,
    );
    let other = app
        .project()
        .tracks
        .iter()
        .filter_map(|t| t.strip)
        .find(|s| *s != strip);
    if let Some(key) = other {
        app.exec(Command::SetEffectSidechain {
            strip,
            effect,
            source: Some(key),
        });
    }
    app.eq_editor.show.collisions = true;
    app.set_eq_global(strip, effect, "mode", Mode::NaturalPhase.index() as f32);
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
    let Some(slot) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .cloned()
    else {
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
    // The dynamic band in the panel, with the bell at 2.8 kHz selected beside it.
    let dynamic = (0..EQ_BANDS).find(|b| slot.spec.get(&eq_key(*b, "dyn")) >= 0.5);
    if let Some(d) = dynamic {
        app.eq_editor.select(&[d, 2]);
    }
}
