//! Spatium's tail view: the reverb on a time axis — the benchmark has none.
//!
//! Drawn: the pre-delay, the early reflections as the engine will make them (one stem per
//! reflection, energy per 5 ms), the late field's build-up and its decay at 1 kHz (solid)
//! and at 125 Hz and 8 kHz (dashed), and the **measured** envelope of the last event that
//! went through the reverb (filled). Handles: the pre-delay, the end of the reflections'
//! window (their size), their level, and the decay (the −30 dB point of the 1 kHz line).
//!
//! The measurement runs here, on the UI thread, from the engine's analyser pairs: the dry
//! part is subtracted back out (the mix law is known), an event is found in the input,
//! and when its tail has fallen 35 dB (or four seconds have passed) the tail is measured
//! the way the DSP tests measure it — octave bands, Schroeder's backward integration, T20.

use super::{seconds, View};
use crate::app::CantusApp;
use crate::design::{self, space};
use crate::i18n::{t, tf};
use crate::theme;
use cantus_dsp::spatium::measure::{band_decay, decay as measure_decay, envelope_db};
use cantus_dsp::spatium::model::{
    diffusion, er_level_db, er_pattern, late_gap_ms, late_level_db, room_at, Style, ER_OFF_DB,
    ER_SIZE_MAX,
};
use cantus_dsp::spatium::OCTAVES;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense, Stroke, StrokeKind};
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
    /// The early reflections: time (s) and level (dB), both sides.
    stems: Vec<(f32, f32)>,
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
    let mut stems = Vec::new();
    let mut er_top = None;
    if let (Some(db), false) = (
        er_level_db(s.style, s.distance, s.er_db),
        s.style == Style::Spring,
    ) {
        let lin = 10f32.powf(db / 20.0);
        let bin = BIN_MS * 0.001 * fs;
        for side in 0..2 {
            for k in 0..pattern.count {
                let g = pattern.gain[side][k] * lin * 0.5_f32.sqrt();
                let level = 10.0 * (g * g / bin).max(1e-20).log10();
                stems.push((pre + pattern.t_ms[side][k] * 0.001, level));
            }
        }
        er_top = stems.iter().map(|s| s.1).reduce(f32::max);
    }
    // Normalise to the loudest thing drawn.
    let top = er_top.unwrap_or(f32::MIN).max(late_db);
    for s in &mut stems {
        s.1 -= top;
    }
    late_db -= top;
    Model {
        pre,
        er_end: pre + pattern.window_ms * 0.001,
        late_on,
        late_db,
        stems,
        er_top: er_top.map(|t| t - top),
        decays,
    }
}

pub fn display(app: &mut CantusApp, ui: &mut egui::Ui, view: &View, size: egui::Vec2) {
    let pal = theme::p();
    let m = model(view);
    let t_max = nice_span(m.late_on + 0.75 * m.decays[1]);
    // A title row the size of the curve display's header, so the two displays line up.
    let head = ui.spacing().interact_size.y;
    let measured = app.fx.space.capture.measured.clone();
    // The one number the view adds to the knobs: the decay the last tail really had.
    let summary = match measured.as_ref().and_then(|m| m.broadband) {
        Some(b) => tf("space.tail.measured", &[("s", &seconds(b))]),
        None => String::new(),
    };
    let mut spectrum = app.fx.space.tail_spectrum;
    ui.allocate_ui_with_layout(
        vec2(size.x, head),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(
                egui::RichText::new(t("space.tail"))
                    .text_style(design::strong())
                    .color(pal.text),
            );
            if ui
                .selectable_label(!spectrum, t("space.tail.envelope"))
                .on_hover_text(t("space.tail.envelope_tip"))
                .clicked()
            {
                spectrum = false;
            }
            if ui
                .selectable_label(spectrum, t("space.tail.spectrum"))
                .on_hover_text(t("space.tail.spectrum_tip"))
                .clicked()
            {
                spectrum = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(summary)
                        .text_style(design::mono_small())
                        .color(pal.text_dim),
                );
            });
        },
    );
    app.fx.space.tail_spectrum = spectrum;
    let h = size.y - head - ui.spacing().item_spacing.y;
    let (rect, resp) = ui.allocate_exact_size(vec2(size.x, h), Sense::click_and_drag());
    let geo = Geo { rect, t_max };
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    if spectrum {
        spectrum_view(&p, &geo, view, &m, measured.as_ref());
        p.rect_stroke(
            rect,
            pal.corner_radius,
            Stroke::new(1.0, pal.grid_strong),
            StrokeKind::Inside,
        );
        return;
    }

    // Grid: time ticks, dB every 12.
    let step = match t_max {
        x if x <= 0.5 => 0.05,
        x if x <= 1.0 => 0.1,
        x if x <= 3.0 => 0.25,
        x if x <= 5.0 => 0.5,
        _ => 1.0,
    };
    let mut tt = step;
    while tt < t_max - 1e-4 {
        p.vline(geo.x(tt), rect.y_range(), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(geo.x(tt) + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            seconds(tt),
            design::paint::ruler(),
            pal.text_dim,
        );
        tt += step;
    }
    for db in [-12.0f32, -24.0, -36.0, -48.0, -60.0] {
        p.hline(rect.x_range(), geo.y(db), Stroke::new(1.0, pal.grid));
        p.text(
            pos2(rect.right() - 3.0, geo.y(db) - 1.0),
            Align2::RIGHT_BOTTOM,
            format!("{db:.0}"),
            design::paint::ruler(),
            pal.text_dim,
        );
    }

    // The measured envelope of the last event, filled from the floor.
    if let Some(ms) = &measured {
        let mut mesh = egui::Mesh::default();
        let fill = pal
            .text_dim
            .gamma_multiply(if pal.dark { 0.35 } else { 0.25 });
        let mut line = Vec::new();
        for (i, d) in ms.envelope_db.iter().enumerate() {
            let tt = i as f32 * BIN_MS * 0.001;
            if tt > t_max {
                break;
            }
            let pt = pos2(geo.x(tt), geo.y(*d));
            line.push(pt);
            let base = mesh.vertices.len() as u32;
            mesh.colored_vertex(pt, fill);
            mesh.colored_vertex(pos2(pt.x, rect.bottom()), fill);
            if i > 0 {
                mesh.add_triangle(base - 2, base - 1, base);
                mesh.add_triangle(base - 1, base, base + 1);
            }
        }
        p.add(mesh);
        p.add(egui::Shape::line(line, Stroke::new(1.0, pal.text_dim)));
        // Where the input stopped.
        let x = geo.x(ms.input_ms * 0.001);
        p.vline(
            x,
            rect.y_range(),
            Stroke::new(1.0, pal.text_dim.gamma_multiply(0.6)),
        );
    }

    // The pre-delay, shaded; the reflections' window, bracketed.
    let pre_rect = Rect::from_min_max(rect.min, pos2(geo.x(m.pre), rect.bottom()));
    p.rect_filled(pre_rect, 0.0, pal.grid.gamma_multiply(0.6));
    // One hairline per reflection with a dot on top: dense patterns stay a comb, not a
    // block.
    let er_color = pal.gold.gamma_multiply(if pal.dark { 0.6 } else { 0.7 });
    for (tt, db) in &m.stems {
        let top = pos2(geo.x(*tt), geo.y(*db));
        p.line_segment(
            [pos2(top.x, rect.bottom()), top],
            Stroke::new(1.0, er_color),
        );
        p.circle_filled(top, 1.75, pal.gold);
    }
    // The late field: its build-up (diffusion smears the onset), then its decay per band.
    let diff = diffusion(
        &room_at(view.s.space),
        view.s.style,
        view.s.distance,
        view.s.character,
    );
    let rise = 0.004 + 0.03 * diff;
    let late_line = |decay: f32| -> Vec<Pos2> {
        (0..=120)
            .map(|i| {
                let tt = m.late_on + (t_max - m.late_on).max(0.0) * i as f32 / 120.0;
                let dt = tt - m.late_on;
                let build = 10.0 * (1.0 - (-dt / rise).exp()).max(1e-6).log10();
                pos2(geo.x(tt), geo.y(m.late_db + build - 60.0 * dt / decay))
            })
            .collect()
    };
    for (i, d) in m.decays.iter().enumerate() {
        let pts = late_line(*d);
        if i == 1 {
            p.add(egui::Shape::line(pts, Stroke::new(2.5, pal.gold)));
        } else {
            p.add(egui::Shape::dashed_line(
                &pts,
                Stroke::new(1.0, pal.gold),
                5.0,
                4.0,
            ));
            // The band's name where its line crosses −40 dB (or at the view's end), clear
            // of the time ticks along the bottom.
            let at = (m.late_on + d * (m.late_db + 40.0).max(1.0) / 60.0).min(t_max * 0.92);
            let db = m.late_db - 60.0 * (at - m.late_on) / d;
            let right = geo.x(at) > rect.right() - space::GRID * 14.0;
            p.text(
                pos2(
                    geo.x(at) + if right { -3.0 } else { 3.0 },
                    geo.y(db.max(FLOOR_DB + 8.0)) - 2.0,
                ),
                if right {
                    Align2::RIGHT_BOTTOM
                } else {
                    Align2::LEFT_BOTTOM
                },
                if i == 0 { "125 Hz" } else { "8 kHz" },
                design::paint::ruler(),
                pal.text,
            );
        }
    }
    tail_handles(app, ui, view, &geo, &m, &resp);
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
}

/// The four handles on the time axis, their labels, and their drags. Every drag is one
/// undo step; dragging the pre-delay frees it from the tempo.
fn tail_handles(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    view: &View,
    geo: &Geo,
    m: &Model,
    resp: &egui::Response,
) {
    let pal = theme::p();
    let p = ui.painter_at(geo.rect);
    // The time handles sit low, where the tail is quiet, clear of the reflections.
    let top = geo.rect.bottom() - space::GRID * 16.0;
    let er_on = m.er_top.is_some() && view.s.er_db > ER_OFF_DB;
    let t30 = m.late_on + m.decays[1] * 0.5;
    let mut handles: Vec<(Grab, Pos2, String)> = vec![(
        Grab::Predelay,
        pos2(geo.x(m.pre), top),
        tf(
            "space.tail.pre",
            &[(
                "ms",
                &crate::widgets::knob::readout(view.s.predelay_ms, "ms"),
            )],
        ),
    )];
    if er_on {
        handles.push((
            Grab::ErSize,
            pos2(geo.x(m.er_end), top + space::GRID * 5.0),
            tf(
                "space.tail.early",
                &[(
                    "ms",
                    &crate::widgets::knob::readout((m.er_end - m.pre) * 1_000.0, "ms"),
                )],
            ),
        ));
        let first = m.stems.iter().map(|s| s.0).fold(f32::MAX, f32::min);
        handles.push((
            Grab::ErLevel,
            pos2(
                geo.x(first),
                geo.y(m.er_top.unwrap_or(0.0))
                    .max(geo.rect.top() + space::S3),
            ),
            format!("{:+.1} dB", view.s.er_db),
        ));
    }
    handles.push((
        Grab::Decay,
        pos2(geo.x(t30), geo.y(m.late_db - 30.0)),
        seconds(m.decays[1]),
    ));
    let pointer = resp.hover_pos();
    let near = |c: Pos2| pointer.is_some_and(|pt| pt.distance(c) < 10.0);
    let held = app.fx.space.time_drag;
    // The pre-delay and the window's end are lines across the view, not only points.
    p.vline(
        geo.x(m.pre),
        geo.rect.y_range(),
        Stroke::new(1.0, pal.gold.gamma_multiply(0.7)),
    );
    if er_on {
        p.add(egui::Shape::dashed_line(
            &[
                pos2(geo.x(m.er_end), geo.rect.top()),
                pos2(geo.x(m.er_end), geo.rect.bottom()),
            ],
            Stroke::new(1.0, pal.gold.gamma_multiply(0.7)),
            4.0,
            3.0,
        ));
    }
    for (grab, c, label) in &handles {
        let hot = held == Some(*grab) || near(*c);
        let r = if hot { 6.5 } else { 5.0 };
        // A diamond: a shape no other mark on the view has.
        let pts = vec![
            *c + vec2(0.0, -r),
            *c + vec2(r, 0.0),
            *c + vec2(0.0, r),
            *c + vec2(-r, 0.0),
        ];
        p.add(egui::Shape::convex_polygon(
            pts,
            pal.gold,
            Stroke::new(1.0, theme::on_color(&pal, pal.gold)),
        ));
        let right = c.x < geo.rect.right() - space::GRID * 18.0;
        p.text(
            *c + vec2(if right { 9.0 } else { -9.0 }, 0.0),
            if right {
                Align2::LEFT_CENTER
            } else {
                Align2::RIGHT_CENTER
            },
            label,
            design::paint::ruler(),
            pal.text,
        );
    }
    if let Some(pt) = pointer {
        p.text(
            pos2(geo.rect.right() - space::S2, geo.rect.bottom() - space::S4),
            Align2::RIGHT_BOTTOM,
            format!(
                "{}  \u{b7}  {:.1} dB",
                seconds(geo.t(pt.x).max(0.0)),
                geo.db(pt.y)
            ),
            design::paint::ruler(),
            pal.text_dim,
        );
    }

    // ---- gestures ------------------------------------------------------------------
    if resp.drag_started() {
        if let Some(pt) = resp.interact_pointer_pos() {
            app.fx.space.time_drag = handles
                .iter()
                .map(|(g, c, _)| (*g, c.distance(pt)))
                .filter(|(_, d)| *d < 12.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(g, _)| g);
        }
    }
    if let (Some(grab), Some(pt)) = (app.fx.space.time_drag, resp.interact_pointer_pos()) {
        if resp.dragged() {
            let tt = geo.t(pt.x).max(0.0);
            let room = room_at(view.s.space);
            let fields: Vec<(String, f32)> = match grab {
                Grab::Predelay => vec![
                    ("predelay".into(), (tt * 1_000.0).clamp(0.0, 500.0)),
                    ("sync".into(), 0.0),
                ],
                Grab::ErSize => vec![(
                    "er_size".into(),
                    ((tt - m.pre) * 1_000.0 / room.er_end_ms).clamp(0.25, ER_SIZE_MAX),
                )],
                Grab::ErLevel => {
                    let top = m.er_top.unwrap_or(0.0);
                    vec![(
                        "er".into(),
                        (view.s.er_db + geo.db(pt.y) - top).clamp(ER_OFF_DB + 0.5, 12.0),
                    )]
                }
                Grab::Decay => {
                    let want = (2.0 * (tt - m.late_on)).max(0.05);
                    vec![(
                        "decay".into(),
                        (view.s.decay_s * want / m.decays[1]).clamp(0.1, 30.0),
                    )]
                }
            };
            app.set_space_params(view.strip, view.effect, &fields, true);
        }
    }
    if resp.drag_stopped() {
        app.fx.space.time_drag = None;
        app.end_gesture();
    }
    if handles.iter().any(|(_, c, _)| near(*c)) || app.fx.space.time_drag.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
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

/// The tail as a spectrogram: time across, frequency up, the measured level as the
/// strength of the device colour (one hue, so it reads in both themes and in grey). Over
/// it, where the curve says each frequency should be 30 dB down (thin) and 60 dB down
/// (dashed), counted from the late field's onset: if the engine does what it is told, the
/// colour fades out along the dashed line.
fn spectrum_view(p: &egui::Painter, geo: &Geo, view: &View, m: &Model, ms: Option<&Measured>) {
    let pal = theme::p();
    let rect = geo.rect;
    let y_of = |row: f32| rect.bottom() - row / WF_ROWS as f32 * rect.height();
    let y_hz = |hz: f32| y_of((hz / WF_LO).ln() / (WF_HI / WF_LO).ln() * WF_ROWS as f32);
    if let Some(ms) = ms {
        let cols = ms.wf_frames;
        // At most one column per pixel: neighbouring frames merge to their loudest.
        let per = ((cols as f32 * WF_HOP / geo.t_max) / rect.width())
            .ceil()
            .max(1.0) as usize;
        let mut mesh = egui::Mesh::default();
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
                    .map(|k| ms.waterfall[k * WF_ROWS + row])
                    .fold(f32::MIN, f32::max);
                let a = ((db + 60.0) / 60.0).clamp(0.0, 1.0).powf(1.4);
                if a <= 0.01 {
                    continue;
                }
                let c = pal
                    .gold
                    .gamma_multiply(a * if pal.dark { 0.95 } else { 0.85 });
                let r = Rect::from_min_max(
                    pos2(x0, y_of(row as f32 + 1.0)),
                    pos2(x1.max(x0 + 1.0), y_of(row as f32)),
                );
                mesh.add_colored_rect(r, c);
            }
            f += per;
        }
        p.add(mesh);
    } else {
        p.text(
            rect.center(),
            Align2::CENTER_CENTER,
            t("space.tail.spectrum_empty"),
            design::paint::empty_state(),
            pal.text_dim,
        );
    }
    // Frequency lines and labels.
    for hz in [100.0f32, 1_000.0, 10_000.0] {
        let y = y_hz(hz);
        p.hline(rect.x_range(), y, Stroke::new(1.0, pal.grid_strong));
        p.text(
            pos2(rect.left() + 3.0, y - 1.0),
            Align2::LEFT_BOTTOM,
            crate::widgets::knob::readout(hz, "Hz"),
            design::paint::ruler(),
            pal.text,
        );
    }
    // Where the curve says the tail is 30 and 60 dB down, per frequency.
    let curve = view.s.target_curve();
    for (drop, dashed) in [(0.5f32, false), (1.0, true)] {
        let pts: Vec<Pos2> = (0..=60)
            .map(|i| {
                let row = i as f32 / 60.0 * WF_ROWS as f32;
                let hz = wf_row_hz(row);
                pos2(geo.x(m.late_on + curve.t60(hz) * drop), y_of(row))
            })
            .collect();
        let stroke = Stroke::new(if dashed { 1.5 } else { 1.0 }, pal.text);
        if dashed {
            p.add(egui::Shape::dashed_line(&pts, stroke, 6.0, 4.0));
        } else {
            p.add(egui::Shape::line(pts, stroke));
        }
    }
    p.text(
        pos2(rect.right() - space::S2, rect.top() + space::S2),
        Align2::RIGHT_TOP,
        t("space.tail.spectrum_legend"),
        design::paint::ruler(),
        pal.text,
    );
    // Time ticks.
    let step = if geo.t_max <= 1.0 {
        0.1
    } else if geo.t_max <= 3.0 {
        0.5
    } else {
        1.0
    };
    let mut tt = step;
    while tt < geo.t_max - 1e-4 {
        p.text(
            pos2(geo.x(tt) + 2.0, rect.bottom() - 2.0),
            Align2::LEFT_BOTTOM,
            seconds(tt),
            design::paint::ruler(),
            pal.text_dim,
        );
        tt += step;
    }
}
