//! Spatium's tail view: the reverb on a time axis — the benchmark has none.
//!
//! Drawn: the pre-delay, the early reflections as the engine will make them (one stem per
//! reflection, energy per 5 ms), the late field's build-up and its decay at 1 kHz (solid)
//! and at 125 Hz and 8 kHz (dashed), and the **measured** envelope of the last event that
//! went through the reverb (filled). Handles: the pre-delay, the end of the reflections'
//! window (their size), their level, and the decay (the −30 dB point of the 1 kHz line).
//! Their words go last, on plates, and step aside for one another (audit #22).
//!
//! The measurement runs here, on the UI thread, from the host's analyser pairs: the dry
//! part is subtracted back out (the mix law is known), an event is found in the input,
//! and when its tail has fallen 35 dB (or four seconds have passed) the tail is measured
//! the way the DSP tests measure it — octave bands, Schroeder's backward integration, T20.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::FxHost;
use super::super::multiband::{choice, dashed_poly, with_fmt};
use super::{Secs, State, View, G_TAIL};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style as Look};
use cantus_dsp::spatium::measure::{band_decay, decay as measure_decay, envelope_db};
use cantus_dsp::spatium::model::{
    diffusion, er_level_db, er_pattern, late_gap_ms, late_level_db, room_at, Style, ER_OFF_DB,
    ER_SIZE_MAX, ER_TAPS,
};
use cantus_dsp::spatium::OCTAVES;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};
use std::collections::VecDeque;

/// Seconds of audio kept for measuring.
const KEEP_S: f32 = 10.0;
/// The envelope's bins, ms.
const BIN_MS: f32 = 5.0;
/// Input below this (dBFS, per 10 ms) is silence.
const QUIET_DB: f32 = -55.0;
/// The view's floor, dB.
const FLOOR_DB: f32 = -66.0;

/// What the last event's tail measured.
#[derive(Clone, Debug)]
pub struct Measured {
    /// The reverb's envelope from the event's start, dB against its own peak, per bin.
    pub envelope_db: Vec<f32>,
    /// How long the input played, ms.
    pub input_ms: f32,
    /// Decay per octave ([`OCTAVES`]) and broadband, seconds (T20), with the energy after
    /// the end of the capture accounted for (`measure::band_decay`).
    pub t60: [Option<f32>; OCTAVES.len()],
    pub broadband: Option<f32>,
    /// Per octave: the capture held the decay (`Decay::complete`). Where it did
    /// not, the tail was cut and the number is extrapolated — the table says so (ADR 0022).
    pub complete: [bool; OCTAVES.len()],
    /// The measured waterfall: [`WF_ROWS`] bands every [`WF_HOP`] s from the event's
    /// start, dB against the loudest cell; `wf_frames` columns.
    pub waterfall: Vec<f32>,
    pub wf_frames: usize,
}

/// The effect's recent audio, the reverb alone, and the events found in it.
#[derive(Debug, Default)]
pub struct Capture {
    input: VecDeque<f32>,
    wet: VecDeque<f32>,
    sample_rate: f32,
    /// Absolute index of the first sample kept.
    start: u64,
    acc: f64,
    acc_n: usize,
    active: bool,
    quiet_blocks: usize,
    onset: Option<u64>,
    end: Option<u64>,
    measured_for: Option<u64>,
    pub measured: Option<Measured>,
}

impl Capture {
    fn now(&self) -> u64 {
        self.start + self.input.len() as u64
    }

    /// Add (input, output) pairs of the effect. `slot_mix` is the slot's own dry/wet;
    /// `dry` the reverb's dry gain: with both known, the output minus the dry part is the
    /// reverb alone.
    pub fn push(
        &mut self,
        pairs: &[(f32, f32)],
        sample_rate: f32,
        slot_mix: f32,
        dry: f32,
        _wet: f32,
    ) {
        if sample_rate != self.sample_rate {
            *self = Capture {
                sample_rate,
                ..Capture::default()
            };
        }
        let k = slot_mix * dry + (1.0 - slot_mix);
        let block = ((sample_rate * 0.01) as usize).max(1);
        let keep = (KEEP_S * sample_rate) as usize;
        for (i, o) in pairs {
            let w = if slot_mix > 1e-3 {
                (o - i * k) / slot_mix
            } else {
                0.0
            };
            self.input.push_back(*i);
            self.wet.push_back(w);
            if self.input.len() > keep {
                self.input.pop_front();
                self.wet.pop_front();
                self.start += 1;
            }
            self.acc += (*i as f64) * (*i as f64);
            self.acc_n += 1;
            if self.acc_n == block {
                let db = 10.0 * (self.acc / block as f64).max(1e-20).log10() as f32;
                self.block(db, self.now() - block as u64);
                self.acc = 0.0;
                self.acc_n = 0;
            }
        }
    }

    /// One 10 ms block of input: find where events start and where they stop.
    fn block(&mut self, db: f32, at: u64) {
        let active = db > QUIET_DB;
        if active {
            // A new event after 200 ms of silence; a shorter gap continues the last one.
            if self.onset.is_none() || (!self.active && self.quiet_blocks >= 20) {
                self.onset = Some(at);
            }
            self.end = None;
            self.quiet_blocks = 0;
        } else {
            if self.active {
                self.end = Some(at);
            }
            self.quiet_blocks += 1;
        }
        self.active = active;
    }

    /// Measure the last event's tail once it has fallen far enough (or long enough).
    pub fn analyse(&mut self) {
        let (Some(onset), Some(end)) = (self.onset, self.end) else {
            return;
        };
        if self.measured_for == Some(onset) || onset < self.start || end < onset {
            return;
        }
        let fs = self.sample_rate;
        let now = self.now();
        let tail = (now - end) as usize;
        if tail < (0.25 * fs) as usize {
            return;
        }
        let e0 = (end - self.start) as usize;
        let win = (0.02 * fs) as usize;
        let rms = |from: usize| -> f32 {
            let s = self.wet.range(from..(from + win).min(self.wet.len()));
            let n = s.len().max(1) as f32;
            (s.map(|x| x * x).sum::<f32>() / n).sqrt()
        };
        let first = rms(e0);
        let last = rms(self.wet.len().saturating_sub(win));
        let fallen = first > 1e-9 && 20.0 * (last / first).max(1e-9).log10() < -35.0;
        if !(fallen || tail as f32 >= 4.0 * fs) {
            return;
        }
        let o0 = (onset - self.start) as usize;
        let seg: Vec<f32> = self.wet.range(o0..).copied().collect();
        let decay: Vec<f32> = self.wet.range(e0..).copied().collect();
        // A reverb's tail is noise: 5 ms bins jump by several dB. Average the power over
        // 20 ms (four bins, centred) so the line shows the decay, not the grain.
        let raw = envelope_db(&seg, fs, BIN_MS);
        let pow: Vec<f32> = raw.iter().map(|d| 10f32.powf(d / 10.0)).collect();
        let mut env: Vec<f32> = (0..pow.len())
            .map(|i| {
                let lo = i.saturating_sub(2);
                let hi = (i + 2).min(pow.len());
                let p = pow[lo..hi].iter().sum::<f32>() / (hi - lo) as f32;
                10.0 * p.max(1e-20).log10()
            })
            .collect();
        let peak = env.iter().copied().fold(f32::MIN, f32::max);
        env.iter_mut().for_each(|d| *d -= peak);
        let mut bands = [None; OCTAVES.len()];
        let mut complete = [false; OCTAVES.len()];
        for ((b, c), hz) in bands.iter_mut().zip(complete.iter_mut()).zip(OCTAVES) {
            let d = band_decay(&decay, fs, hz);
            *b = d.t60;
            *c = d.complete();
        }
        let (wf_frames, waterfall) = waterfall(&seg, fs, 12.0);
        self.measured = Some(Measured {
            envelope_db: env,
            input_ms: (end - onset) as f32 / fs * 1_000.0,
            t60: bands,
            broadband: measure_decay(&decay, fs).t60,
            complete,
            waterfall,
            wf_frames,
        });
        self.measured_for = Some(onset);
    }
}

/// A handle on the time axis.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Grab {
    Predelay,
    ErSize,
    ErLevel,
    Decay,
}

#[derive(Copy, Clone)]
struct Geo {
    rect: Rect,
    t_max: f32,
}

impl Geo {
    fn x(&self, t: f32) -> f32 {
        self.rect.left() + (t / self.t_max).clamp(0.0, 1.0) * self.rect.width()
    }
    fn t(&self, x: f32) -> f32 {
        (x - self.rect.left()) / self.rect.width().max(1.0) * self.t_max
    }
    fn y(&self, db: f32) -> f32 {
        self.rect.top() + (db / FLOOR_DB).clamp(0.0, 1.0) * self.rect.height()
    }
    fn db(&self, y: f32) -> f32 {
        (y - self.rect.top()) / self.rect.height().max(1.0) * FLOOR_DB
    }
}

/// A span of time that reads well on an axis: 0.25, 0.5, 1, 2, 3, 5, 8, 12 s.
fn nice_span(t: f32) -> f32 {
    [0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 8.0, 12.0]
        .into_iter()
        .find(|s| *s >= t)
        .unwrap_or(12.0)
}

/// The model's picture of an impulse through the settings: energy per bin, dB against its
/// own peak, and where things are. The same functions the engine uses place every piece.
struct Model {
    /// Seconds from the input: the pre-delay, the reflections' window end, the late onset.
    pre: f32,
    er_end: f32,
    late_on: f32,
    /// The late field's level at its onset, dB (same scale as `bins`).
    late_db: f32,
    /// The early reflections: time (s) and level (dB), both sides; `stems` of them.
    stem: [(f32, f32); 2 * ER_TAPS],
    stems: usize,
    /// The loudest reflection's level, dB (the level handle sits there).
    er_top: Option<f32>,
    /// The late field's decay at 125 Hz, 1 kHz and 8 kHz, seconds.
    decays: [f32; 3],
}

fn model(view: &View) -> Model {
    let s = view.s;
    let fs = view.sample_rate;
    let room = room_at(s.space);
    let curve = s.target_curve();
    let pre = s.predelay_ms * 0.001;
    let decays = [curve.t60(125.0), curve.t60(1_000.0), curve.t60(8_000.0)];
    // The late field's energy at its onset for a unit impulse: the calibrated steady-state
    // power spread over the decay (see `cantus_dsp::spatium::late_gain`).
    let late_lin = 10f32.powf(late_level_db(s.distance) / 20.0);
    let total = late_lin * late_lin * (decays[1] / 2.0).sqrt();
    let p0 = total * 13.8 / (decays[1] * fs);
    let mut late_db = 10.0 * p0.max(1e-20).log10();
    let late_on = pre + late_gap_ms(&room, s.distance) * 0.001;
    let pattern = er_pattern(
        &room,
        s.style,
        s.er_size.clamp(0.25, ER_SIZE_MAX),
        s.character,
    );
    let mut stem = [(0.0f32, 0.0f32); 2 * ER_TAPS];
    let mut stems = 0;
    let mut er_top = None;
    if let (Some(db), false) = (
        er_level_db(s.style, s.distance, s.er_db),
        s.style == Style::Spring,
    ) {
        let lin = 10f32.powf(db / 20.0);
        let bin = BIN_MS * 0.001 * fs;
        for side in 0..2 {
            for k in 0..pattern.count.min(ER_TAPS) {
                let g = pattern.gain[side][k] * lin * 0.5_f32.sqrt();
                let level = 10.0 * (g * g / bin).max(1e-20).log10();
                stem[stems] = (pre + pattern.t_ms[side][k] * 0.001, level);
                stems += 1;
            }
        }
        er_top = stem[..stems].iter().map(|s| s.1).reduce(f32::max);
    }
    // Normalise to the loudest thing drawn.
    let top = er_top.unwrap_or(f32::MIN).max(late_db);
    for s in &mut stem[..stems] {
        s.1 -= top;
    }
    late_db -= top;
    Model {
        pre,
        er_end: pre + pattern.window_ms * 0.001,
        late_on,
        late_db,
        stem,
        stems,
        er_top: er_top.map(|t| t - top),
        decays,
    }
}

/// The head, where the view's switch sits, at 100 %.
const HEAD: f32 = 28.0;

const VIEW_LABELS: [Src<'static>; 2] = [
    Src::Key("space.tail.envelope"),
    Src::Key("space.tail.spectrum"),
];
const VIEW_TIPS: [&str; 2] = ["space.tail.envelope_tip", "space.tail.spectrum_tip"];

/// Labels that must not land on one another: each is moved aside when it would.
struct Placer {
    rects: [Rect; 12],
    n: usize,
}

impl Placer {
    fn new() -> Placer {
        Placer {
            rects: [Rect::NOTHING; 12],
            n: 0,
        }
    }

    /// Where a label of `size` anchored at `at` by `align` can go inside `bounds`: there,
    /// or moved down or up by its height until it touches nothing placed before.
    fn place(&mut self, at: Pos2, align: Align2, size: egui::Vec2, bounds: Rect, gap: f32) -> Rect {
        let first = align.anchor_size(at, size);
        let step = size.y + gap;
        let mut best = first;
        for dy in [0.0, step, -step, 2.0 * step, -2.0 * step, 3.0 * step] {
            let mut r = first.translate(vec2(0.0, dy));
            // Kept inside the view.
            let dx = (bounds.left() - r.left()).max(0.0) + (bounds.right() - r.right()).min(0.0);
            let dy2 = (bounds.top() - r.top()).max(0.0) + (bounds.bottom() - r.bottom()).min(0.0);
            r = r.translate(vec2(dx, dy2));
            if !self.rects[..self.n]
                .iter()
                .any(|p| p.expand(gap).intersects(r))
            {
                best = r;
                break;
            }
        }
        if self.n < self.rects.len() {
            self.rects[self.n] = best;
            self.n += 1;
        }
        best
    }
}

/// A word on the view, placed where it touches no other, on a plate.
#[allow(clippy::too_many_arguments)]
fn label(
    b: &mut Brush,
    placer: &mut Placer,
    bounds: Rect,
    at: Pos2,
    align: Align2,
    color: Color32,
    ink: &Ink,
    k: f32,
    args: std::fmt::Arguments<'_>,
) {
    with_fmt(args, |text| {
        let t = Text::new(text, Role::Label, color).caps(false);
        let size = crate::kit::text::measure(b.ctx(), &t, k);
        let r = placer.place(at, align, size, bounds.shrink(2.0 * k), 3.0 * k);
        b.rrect(
            b.snap_rect(r.expand2(vec2(3.0 * k, 1.0 * k))),
            2.0 * k,
            Fill::Solid(ink.plate),
        );
        let _ = crate::kit::text::paint(b, r.expand(1.0), &t, k);
    });
}

/// The time ticks' step for a span.
fn tick_step(t_max: f32) -> f32 {
    match t_max {
        x if x <= 0.5 => 0.05,
        x if x <= 1.0 => 0.1,
        x if x <= 3.0 => 0.25,
        x if x <= 5.0 => 0.5,
        _ => 1.0,
    }
}

pub fn display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Look,
    view: &View,
    rect: Rect,
) {
    let k = s.k;
    let m = model(view);
    let t_max = nice_span(m.late_on + 0.75 * m.decays[1]);
    let head = Rect::from_min_size(rect.min, vec2(rect.width(), HEAD * k));
    let plot = Rect::from_min_max(pos2(rect.left(), head.bottom()), rect.max);
    let geo = Geo { rect: plot, t_max };
    let resp = ui.interact(plot, ui.id().with("space-tail"), Sense::click_and_drag());
    let pointer = resp.hover_pos();
    let handles = handles(view, &geo, &m);
    let spectrum = st.tail_spectrum;
    // The measured envelope's points, into a buffer kept between frames.
    st.tail_pts.clear();
    if let Some(ms) = st.capture.measured.as_ref().filter(|_| !spectrum) {
        for (i, d) in ms.envelope_db.iter().enumerate() {
            let tt = i as f32 * BIN_MS * 0.001;
            if tt > t_max {
                break;
            }
            st.tail_pts.push(pos2(geo.x(tt), geo.y(*d)));
        }
    }
    with_brush(&ui.painter_at(rect), |b| {
        let ink = Ink::of(s);
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(head.bottom()),
            b.px(),
            ink.edge,
        );
        if spectrum {
            spectrum_view(b, &geo, s, view, &m, st.capture.measured.as_ref());
        } else {
            envelope_view(b, &geo, s, view, &m, st, &handles, pointer);
        }
        // The one number the view adds to the knobs: the decay the last tail really had.
        if let Some(bb) = st.capture.measured.as_ref().and_then(|x| x.broadband) {
            canvas::putf(
                b,
                pos2(head.right() - 8.0 * k, head.center().y),
                Align2::RIGHT_CENTER,
                Role::Label,
                ink.dim,
                s.tk(),
                None,
                format_args!("{} {}", frame::tr("space.tail.measured_word"), Secs(bb)),
            );
        }
    });
    // The switch, in the head.
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(head.shrink2(vec2(6.0 * k, 3.0 * k)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    // On the display: its style (fills and polarity), the theme's in the plain look.
    if let Some(i) = choice(
        &mut child,
        &crate::kit::skin::display_style(s),
        spectrum as usize,
        &VIEW_LABELS,
        &VIEW_TIPS,
        frame::tr("space.tail"),
        176.0,
    ) {
        st.tail_spectrum = i == 1;
    }
    if !spectrum {
        tail_gestures(host, st, ui, k, view, &geo, &m, &handles, &resp);
    }
    if resp.hovered() && st.time_drag.is_none() {
        hint::set_text(
            ui.ctx(),
            frame::tr("space.tail"),
            frame::tr(if spectrum {
                "space.tail.spectrum_tip"
            } else {
                "space.tail.envelope_tip"
            }),
            Caps::NONE,
        );
    }
}

/// The four handles on the time axis: what each is, where it sits.
fn handles(view: &View, geo: &Geo, m: &Model) -> ([(Grab, Pos2); 4], usize) {
    // The time handles sit low, where the tail is quiet, clear of the reflections.
    let top = geo.rect.bottom() - geo.rect.height().min(260.0) * 0.26;
    let er_on = m.er_top.is_some() && view.s.er_db > ER_OFF_DB;
    let t30 = m.late_on + m.decays[1] * 0.5;
    let mut out = [(Grab::Predelay, Pos2::ZERO); 4];
    let mut n = 0;
    out[n] = (Grab::Predelay, pos2(geo.x(m.pre), top));
    n += 1;
    if er_on {
        out[n] = (
            Grab::ErSize,
            pos2(geo.x(m.er_end), top + geo.rect.height() * 0.08),
        );
        n += 1;
        let first = m.stem[..m.stems]
            .iter()
            .map(|s| s.0)
            .fold(f32::MAX, f32::min);
        out[n] = (
            Grab::ErLevel,
            pos2(
                geo.x(first),
                geo.y(m.er_top.unwrap_or(0.0)).max(geo.rect.top() + 12.0),
            ),
        );
        n += 1;
    }
    out[n] = (Grab::Decay, pos2(geo.x(t30), geo.y(m.late_db - 30.0)));
    n += 1;
    (out, n)
}

/// The envelope: the grid, the last tail measured, the pre-delay, the reflections, the
/// late field's lines, the handles — and last, every word.
#[allow(clippy::too_many_arguments)]
fn envelope_view(
    b: &mut Brush,
    geo: &Geo,
    s: &Look,
    view: &View,
    m: &Model,
    st: &State,
    handles: &([(Grab, Pos2); 4], usize),
    pointer: Option<Pos2>,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let rect = geo.rect;
    let px = b.px();
    let t_max = geo.t_max;
    // Grid: time ticks, dB every 12.
    let step = tick_step(t_max);
    let mut tt = step;
    while tt < t_max - 1e-4 {
        b.vline(
            b.snap_mid(geo.x(tt)),
            rect.top(),
            rect.bottom(),
            px,
            ink.grid,
        );
        tt += step;
    }
    for db in [-12.0f32, -24.0, -36.0, -48.0, -60.0] {
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(geo.y(db)),
            px,
            ink.grid,
        );
    }
    // The measured envelope of the last event, filled from the floor.
    if st.tail_pts.len() > 1 {
        b.area(
            &st.tail_pts,
            rect.bottom(),
            Ink::a(ink.dim, if ink.dark { 0.30 } else { 0.22 }),
        );
        b.polyline(&st.tail_pts, px.max(k), ink.dim);
    }
    if let Some(ms) = &st.capture.measured {
        // Where the input stopped.
        let x = b.snap_mid(geo.x(ms.input_ms * 0.001));
        b.vline(x, rect.top(), rect.bottom(), px, Ink::a(ink.dim, 0.6));
    }
    // The pre-delay, shaded.
    b.rect(
        Rect::from_min_max(rect.min, pos2(geo.x(m.pre), rect.bottom())),
        Fill::Solid(Ink::a(ink.grid_strong, 0.5)),
    );
    // One hairline per reflection with a dot on top: dense patterns stay a comb.
    // The early reflections in the look's second voice (Spatium's cold cyan); plain, the hue.
    let er = Ink::a(ink.voice(ink.hue, true), if ink.dark { 0.6 } else { 0.7 });
    for (t, db) in &m.stem[..m.stems] {
        let top = pos2(geo.x(*t), geo.y(*db));
        b.segment(pos2(top.x, rect.bottom()), top, px.max(k), er, false);
        b.circle(top, 1.75 * k, Fill::Solid(ink.hue));
    }
    // The late field: its build-up (diffusion smears the onset), then its decay per band.
    let diff = diffusion(
        &room_at(view.s.space),
        view.s.style,
        view.s.distance,
        view.s.character,
    );
    let rise = 0.004 + 0.03 * diff;
    let late_line = |decay: f32, pts: &mut [Pos2; 121]| {
        for (i, p) in pts.iter_mut().enumerate() {
            let tt = m.late_on + (t_max - m.late_on).max(0.0) * i as f32 / 120.0;
            let dt = tt - m.late_on;
            let build = 10.0 * (1.0 - (-dt / rise).exp()).max(1e-6).log10();
            *p = pos2(geo.x(tt), geo.y(m.late_db + build - 60.0 * dt / decay));
        }
    };
    let mut pts = [Pos2::ZERO; 121];
    for (i, d) in m.decays.iter().enumerate() {
        late_line(*d, &mut pts);
        if i == 1 {
            b.polyline(&pts, 2.5 * k, ink.hue);
        } else {
            dashed_poly(b, &pts, k, ink.hue, 5.0 * k, 4.0 * k);
        }
    }
    // The pre-delay and the window's end are lines across the view, not only points.
    let line = Ink::a(ink.hue, 0.7);
    b.vline(
        b.snap_mid(geo.x(m.pre)),
        rect.top(),
        rect.bottom(),
        px,
        line,
    );
    let er_on = m.er_top.is_some() && view.s.er_db > ER_OFF_DB;
    if er_on {
        canvas::dashed(
            b,
            pos2(geo.x(m.er_end), rect.top()),
            pos2(geo.x(m.er_end), rect.bottom()),
            px,
            line,
            4.0 * k,
            3.0 * k,
        );
    }
    // The handles: diamonds, a shape no other mark on the view has.
    let (list, n) = handles;
    let held = st.time_drag;
    for (grab, c) in &list[..*n] {
        let hot = held == Some(*grab) || pointer.is_some_and(|pt| pt.distance(*c) < 10.0 * k);
        let r = if hot { 6.5 } else { 5.0 } * k;
        let d = |r: f32| {
            [
                *c + vec2(0.0, -r),
                *c + vec2(r, 0.0),
                *c + vec2(0.0, r),
                *c + vec2(-r, 0.0),
            ]
        };
        b.polygon(&d(r + k), crate::kit::color::on_fill(ink.hue));
        b.polygon(&d(r), ink.hue);
    }
    if let Some(pt) = pointer {
        b.vline(
            b.snap_mid(pt.x),
            rect.top(),
            rect.bottom(),
            px,
            Ink::a(ink.dim, 0.5),
        );
    }

    // ---- the words, last ----
    let mut ruler = canvas::Ruler::new();
    let mut tt = step;
    while tt < t_max - 1e-4 {
        let x = geo.x(tt) + 2.0 * k;
        if ruler.fits(x, 6.0 * k) {
            let r = canvas::putf(
                b,
                pos2(x, rect.bottom() - 2.0 * k),
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{}", Secs(tt)),
            );
            ruler.placed(r);
        }
        tt += step;
    }
    for db in [-12.0f32, -24.0, -36.0, -48.0, -60.0] {
        canvas::putf(
            b,
            pos2(rect.right() - 3.0 * k, geo.y(db) - 1.0),
            Align2::RIGHT_BOTTOM,
            Role::Micro,
            ink.dim,
            tk,
            None,
            format_args!("{db:.0}"),
        );
    }
    let mut placer = Placer::new();
    // The frequency ticks along the bottom are placed first, so nothing lands on them.
    placer.place(
        pos2(rect.left(), rect.bottom()),
        Align2::LEFT_BOTTOM,
        vec2(rect.width(), 14.0 * k),
        rect,
        0.0,
    );
    let words = rect.shrink(2.0 * k);
    for (grab, c) in &list[..*n] {
        let right = c.x < rect.right() - 90.0 * k;
        let (at, align) = if right {
            (*c + vec2(9.0 * k, 0.0), Align2::LEFT_CENTER)
        } else {
            (*c - vec2(9.0 * k, 0.0), Align2::RIGHT_CENTER)
        };
        let color = ink.text;
        match grab {
            Grab::Predelay => label(
                b,
                &mut placer,
                words,
                at,
                align,
                color,
                &ink,
                tk,
                format_args!(
                    "{} {}",
                    frame::tr("space.tail.pre_word"),
                    Secs(view.s.predelay_ms * 0.001)
                ),
            ),
            Grab::ErSize => label(
                b,
                &mut placer,
                words,
                at,
                align,
                color,
                &ink,
                tk,
                format_args!(
                    "{} {}",
                    frame::tr("space.tail.early_word"),
                    Secs(m.er_end - m.pre)
                ),
            ),
            Grab::ErLevel => label(
                b,
                &mut placer,
                words,
                at,
                align,
                color,
                &ink,
                tk,
                format_args!("{:+.1} dB", view.s.er_db),
            ),
            Grab::Decay => label(
                b,
                &mut placer,
                words,
                at,
                align,
                color,
                &ink,
                tk,
                format_args!("{}", Secs(m.decays[1])),
            ),
        }
    }
    // The bands' names where their lines cross −40 dB (or at the view's end).
    for (i, d) in m.decays.iter().enumerate() {
        if i == 1 {
            continue;
        }
        let at = (m.late_on + d * (m.late_db + 40.0).max(1.0) / 60.0).min(t_max * 0.92);
        let db = m.late_db - 60.0 * (at - m.late_on) / d;
        let x = geo.x(at);
        let right = x > rect.right() - 60.0 * k;
        let p = pos2(
            x + if right { -3.0 * k } else { 3.0 * k },
            geo.y(db.max(-58.0)) - 2.0 * k,
        );
        let align = if right {
            Align2::RIGHT_BOTTOM
        } else {
            Align2::LEFT_BOTTOM
        };
        if i == 0 {
            label(
                b,
                &mut placer,
                words,
                p,
                align,
                ink.text,
                &ink,
                tk,
                format_args!("125 Hz"),
            );
        } else {
            label(
                b,
                &mut placer,
                words,
                p,
                align,
                ink.text,
                &ink,
                tk,
                format_args!("8 kHz"),
            );
        }
    }
    if let Some(pt) = pointer {
        canvas::putf(
            b,
            pos2(rect.right() - 8.0 * k, rect.top() + 4.0 * k),
            Align2::RIGHT_TOP,
            Role::Label,
            ink.text,
            tk,
            Some(ink.plate),
            format_args!(
                "{}  \u{b7}  {:.1} dB",
                Secs(geo.t(pt.x).max(0.0)),
                geo.db(pt.y)
            ),
        );
    }
}

/// The handles' drags. Every drag is one step; dragging the pre-delay frees it from the
/// tempo.
#[allow(clippy::too_many_arguments)]
fn tail_gestures(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    k: f32,
    view: &View,
    geo: &Geo,
    m: &Model,
    handles: &([(Grab, Pos2); 4], usize),
    resp: &egui::Response,
) {
    let (list, n) = handles;
    if resp.drag_started() {
        if let Some(pt) = ui
            .input(|i| i.pointer.press_origin())
            .or(resp.interact_pointer_pos())
        {
            st.time_drag = list[..*n]
                .iter()
                .map(|(g, c)| (*g, c.distance(pt)))
                .filter(|(_, d)| *d < 12.0 * k)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(g, _)| g);
        }
    }
    if let (Some(grab), Some(pt)) = (st.time_drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let tt = geo.t(pt.x).max(0.0);
            let room = room_at(view.s.space);
            match grab {
                Grab::Predelay => host.apply(
                    &[
                        ("predelay", Some((tt * 1_000.0).clamp(0.0, 500.0))),
                        ("sync", Some(0.0)),
                    ],
                    "cmd.space_edit",
                    Some(G_TAIL),
                ),
                Grab::ErSize => host.apply(
                    &[(
                        "er_size",
                        Some(((tt - m.pre) * 1_000.0 / room.er_end_ms).clamp(0.25, ER_SIZE_MAX)),
                    )],
                    "cmd.space_edit",
                    Some(G_TAIL),
                ),
                Grab::ErLevel => {
                    let top = m.er_top.unwrap_or(0.0);
                    host.apply(
                        &[(
                            "er",
                            Some((view.s.er_db + geo.db(pt.y) - top).clamp(ER_OFF_DB + 0.5, 12.0)),
                        )],
                        "cmd.space_edit",
                        Some(G_TAIL),
                    )
                }
                Grab::Decay => {
                    let want = (2.0 * (tt - m.late_on)).max(0.05);
                    host.apply(
                        &[(
                            "decay",
                            Some((view.s.decay_s * want / m.decays[1]).clamp(0.1, 30.0)),
                        )],
                        "cmd.space_edit",
                        Some(G_TAIL),
                    )
                }
            }
        }
    }
    if resp.drag_stopped() {
        st.time_drag = None;
        host.end_gesture();
    }
    let near = resp
        .hover_pos()
        .is_some_and(|pt| list[..*n].iter().any(|(_, c)| c.distance(pt) < 10.0 * k));
    if near || st.time_drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        hint::set_text(
            ui.ctx(),
            frame::tr("space.tail"),
            frame::tr("space.tail.handles_tip"),
            Caps::NONE,
        );
    }
}

/// The tail as a spectrogram: time across, frequency up, the measured level as the
/// strength of the device's hue (one hue, so it reads in both themes and in grey). Over
/// it, where the curve says each frequency should be 30 dB down (thin) and 60 dB down
/// (dashed), counted from the late field's onset: if the engine does what it is told, the
/// colour fades out along the dashed line.
fn spectrum_view(
    b: &mut Brush,
    geo: &Geo,
    s: &Look,
    view: &View,
    m: &Model,
    ms: Option<&Measured>,
) {
    let k = s.k;
    let tk = s.tk();
    let ink = Ink::of(s);
    let rect = geo.rect;
    let y_of = |row: f32| rect.bottom() - row / WF_ROWS as f32 * rect.height();
    let y_hz = |hz: f32| y_of((hz / WF_LO).ln() / (WF_HI / WF_LO).ln() * WF_ROWS as f32);
    if let Some(ms) = ms {
        let cols = ms.wf_frames;
        // At most one column per pixel: neighbouring frames merge to their loudest.
        let per = ((cols as f32 * WF_HOP / geo.t_max) / rect.width())
            .ceil()
            .max(1.0) as usize;
        let mut f = 0;
        while f < cols {
            let t0 = f as f32 * WF_HOP;
            if t0 > geo.t_max {
                break;
            }
            let x0 = geo.x(t0);
            let x1 = geo.x((f + per) as f32 * WF_HOP);
            for row in 0..WF_ROWS {
                let db = (f..(f + per).min(cols))
                    .map(|c| ms.waterfall[c * WF_ROWS + row])
                    .fold(f32::MIN, f32::max);
                let a = ((db + 60.0) / 60.0).clamp(0.0, 1.0).powf(1.4);
                if a <= 0.01 {
                    continue;
                }
                let r = Rect::from_min_max(
                    pos2(x0, y_of(row as f32 + 1.0)),
                    pos2(x1.max(x0 + 1.0), y_of(row as f32)),
                );
                b.rect(
                    r,
                    Fill::Solid(Ink::a(ink.hue, a * if ink.dark { 0.95 } else { 0.85 })),
                );
            }
            f += per;
        }
    }
    // Frequency lines.
    for hz in [100.0f32, 1_000.0, 10_000.0] {
        b.hline(
            rect.left(),
            rect.right(),
            b.snap_mid(y_hz(hz)),
            b.px(),
            ink.grid_strong,
        );
    }
    // Where the curve says the tail is 30 and 60 dB down, per frequency.
    let curve = view.s.target_curve();
    let mut pts = [Pos2::ZERO; 61];
    for (drop, dashed) in [(0.5f32, false), (1.0, true)] {
        for (i, p) in pts.iter_mut().enumerate() {
            let row = i as f32 / 60.0 * WF_ROWS as f32;
            let hz = wf_row_hz(row);
            *p = pos2(geo.x(m.late_on + curve.t60(hz) * drop), y_of(row));
        }
        if dashed {
            dashed_poly(b, &pts, 1.5 * k, ink.text, 6.0 * k, 4.0 * k);
        } else {
            b.polyline(&pts, k, ink.text);
        }
    }
    // The words, last.
    for hz in [100.0f32, 1_000.0, 10_000.0] {
        canvas::putf(
            b,
            pos2(rect.left() + 3.0 * k, y_hz(hz) - 1.0),
            Align2::LEFT_BOTTOM,
            Role::Micro,
            ink.text,
            tk,
            Some(ink.plate),
            format_args!("{}", super::Hz(hz)),
        );
    }
    canvas::put_key(
        b,
        pos2(rect.right() - 8.0 * k, rect.top() + 4.0 * k),
        Align2::RIGHT_TOP,
        "space.tail.spectrum_legend",
        Role::Label,
        ink.text,
        tk,
    );
    let step = if geo.t_max <= 1.0 {
        0.1
    } else if geo.t_max <= 3.0 {
        0.5
    } else {
        1.0
    };
    let mut ruler = canvas::Ruler::new();
    let mut tt = step;
    while tt < geo.t_max - 1e-4 {
        let x = geo.x(tt) + 2.0 * k;
        if ruler.fits(x, 6.0 * k) {
            let r = canvas::putf(
                b,
                pos2(x, rect.bottom() - 2.0 * k),
                Align2::LEFT_BOTTOM,
                Role::Micro,
                ink.dim,
                tk,
                None,
                format_args!("{}", Secs(tt)),
            );
            ruler.placed(r);
        }
        tt += step;
    }
    if ms.is_none() {
        canvas::put_key(
            b,
            rect.center(),
            Align2::CENTER_CENTER,
            "space.tail.spectrum_empty",
            Role::Body,
            ink.dim,
            tk,
        );
    }
}
/// Rows of the measured waterfall: log-spaced bands from 50 Hz to 16 kHz.
pub const WF_ROWS: usize = 40;
const WF_LO: f32 = 50.0;
const WF_HI: f32 = 16_000.0;
/// The waterfall's hop, seconds.
pub const WF_HOP: f32 = 0.01;

/// A band's frequency edges in the waterfall.
pub fn wf_row_hz(row: f32) -> f32 {
    WF_LO * (WF_HI / WF_LO).powf(row / WF_ROWS as f32)
}

/// The measured waterfall: the reverb's level per band (rows) every 10 ms (columns), dB
/// against the loudest cell. A spectrogram of the real tail, from the event's start.
pub fn waterfall(seg: &[f32], sample_rate: f32, max_s: f32) -> (usize, Vec<f32>) {
    let n = 2048usize;
    let hop = (WF_HOP * sample_rate) as usize;
    let frames = ((seg.len().saturating_sub(n)) / hop.max(1)).min((max_s / WF_HOP) as usize);
    let window: Vec<f32> = (0..n)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos())
        .collect();
    let bin_hz = sample_rate / n as f32;
    let mut out = vec![-200.0f32; frames * WF_ROWS];
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    for f in 0..frames {
        let s = &seg[f * hop..f * hop + n];
        for i in 0..n {
            re[i] = s[i] * window[i];
            im[i] = 0.0;
        }
        cantus_dsp::spectrum::fft(&mut re, &mut im);
        for row in 0..WF_ROWS {
            let lo = ((wf_row_hz(row as f32) / bin_hz) as usize).max(1);
            let hi = ((wf_row_hz(row as f32 + 1.0) / bin_hz) as usize).clamp(lo + 1, n / 2);
            let p: f32 =
                (lo..hi).map(|k| re[k] * re[k] + im[k] * im[k]).sum::<f32>() / (hi - lo) as f32;
            out[f * WF_ROWS + row] = 10.0 * p.max(1e-30).log10();
        }
    }
    let peak = out.iter().copied().fold(f32::MIN, f32::max);
    out.iter_mut().for_each(|d| *d -= peak);
    (frames, out)
}
