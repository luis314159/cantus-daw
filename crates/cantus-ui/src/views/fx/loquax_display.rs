//! Loquax's display: the bands lit by a voice. Dark in every theme (the unit's phosphor).
//!
//! - **The bank** (most of the width): one column per band on a log frequency axis from the
//!   lowest band to the highest, each a ladder of fine cells lit from the bottom by what
//!   the band says now (the vocoded output, `cantus_engine::fx::loquax::T_BANDS`), the
//!   highest cell lit a little longer (the peak); the band's own level (`b0` … `b63`) a
//!   short bar across its column; the formant shift drawn as the synthesis bands' ticks
//!   moved against the analysis bands' along the top.
//! - **Who speaks, who sings** (the right): the voice (this track, or the sidechain when
//!   swapped) and the carrier (the synthesiser, the sidechain or both), each with its level
//!   now; the synthesiser's voices sounding or the robot's note; the consonant light (the
//!   detector hearing a sibilant) and the hold.

use super::super::canvas::{self, Ink};
use super::super::frame::tr;
use super::super::host::FxHost;
use crate::kit::color::with_alpha;
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::EffectSpec;
use cantus_dsp::loquax::bank::layout;
use cantus_dsp::loquax::t;
use cantus_engine::fx::loquax::{SHOWN, T_BANDS};
use egui::{pos2, vec2, Align2, Pos2, Rect};

/// One block's numbers as the display reads them.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Reading {
    pub seen: bool,
    pub bands: usize,
    pub voices: f32,
    pub gate: f32,
    pub modulator_db: f32,
    pub carrier_db: f32,
    pub robot: bool,
    pub hold: bool,
    /// The bands' output levels in their groups, dBFS.
    pub out_db: [f32; SHOWN],
}

impl Default for Reading {
    fn default() -> Self {
        Reading {
            seen: false,
            bands: 0,
            voices: 0.0,
            gate: 0.0,
            modulator_db: -200.0,
            carrier_db: -200.0,
            robot: false,
            hold: false,
            out_db: [-200.0; SHOWN],
        }
    }
}

impl Reading {
    pub fn of(tel: &[f32; 64]) -> Reading {
        let mut out_db = [-200.0; SHOWN];
        out_db.copy_from_slice(&tel[T_BANDS..T_BANDS + SHOWN]);
        Reading {
            seen: tel[0] > 0.0,
            bands: tel[t::BANDS].max(0.0) as usize,
            voices: tel[t::VOICES],
            gate: tel[t::GATE],
            modulator_db: tel[t::MOD_DB],
            carrier_db: tel[t::CARRIER_DB],
            robot: tel[t::ROBOT] >= 0.5,
            hold: tel[t::HOLD] >= 0.5,
            out_db,
        }
    }
}

/// The display's memory: each group's lit height, falling slowly (the phosphor), and its
/// peak, held a moment.
#[derive(Debug, Default)]
pub struct View {
    lit: [f32; SHOWN],
    peak: [f32; SHOWN],
    hold: [f32; SHOWN],
}

/// A level in dBFS as a share of a column, 0 … 1 (−72 dB nothing, 0 dB all).
fn share(db: f32) -> f32 {
    ((db + 72.0) / 72.0).clamp(0.0, 1.0)
}

impl View {
    pub fn update(&mut self, r: &Reading, dt: f32) {
        for g in 0..SHOWN {
            let v = share(r.out_db[g]);
            self.lit[g] = if v > self.lit[g] {
                v
            } else {
                (self.lit[g] - dt * 1.4).max(v)
            };
            if v >= self.peak[g] {
                self.peak[g] = v;
                self.hold[g] = 0.6;
            } else {
                self.hold[g] -= dt;
                if self.hold[g] <= 0.0 {
                    self.peak[g] = (self.peak[g] - dt * 0.8).max(v);
                }
            }
        }
    }

    /// The readings as they stand (the screenshots' demo).
    pub fn seed(&mut self, r: &Reading) {
        for g in 0..SHOWN {
            self.lit[g] = share(r.out_db[g]);
            self.peak[g] = (self.lit[g] + 0.06).min(1.0);
            self.hold[g] = 0.6;
        }
    }
}

pub fn show(
    host: &mut dyn FxHost,
    view: &View,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    now: &Reading,
) {
    let spec = host.spec().clone();
    let k = s.k;
    let ink = Ink::of(s);
    let inner = r.shrink(10.0 * k);
    let side = (inner.width() * 0.26).clamp(230.0 * k, 330.0 * k);
    let bank_r = Rect::from_min_max(inner.min, pos2(inner.max.x - side - 12.0 * k, inner.max.y));
    let who_r = Rect::from_min_max(pos2(bank_r.max.x + 12.0 * k, inner.min.y), inner.max);
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, None);
        bank(b, s, &ink, bank_r, view, now, &spec);
        who(b, s, &ink, who_r, now, &spec);
        canvas::edge_b(b, r, &ink);
    });
}

fn pane(b: &mut Brush, ink: &Ink, r: Rect, title: &str, k: f32) -> Rect {
    b.rrect_stroke(r, 4.0 * k, 1.0 * k, with_alpha(ink.grid_strong, 0.8));
    let t = Text::key(title, Role::Micro, ink.dim);
    canvas::put(
        b,
        pos2(r.min.x + 8.0 * k, r.min.y + 6.0 * k),
        Align2::LEFT_TOP,
        &t,
        k,
        None,
    );
    Rect::from_min_max(
        pos2(r.min.x + 8.0 * k, r.min.y + 22.0 * k),
        r.max - vec2(8.0, 8.0) * k,
    )
}

fn bank(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    view: &View,
    now: &Reading,
    spec: &EffectSpec,
) {
    let k = s.k;
    let area = pane(b, ink, r, "loquax.d.bank", k);
    let n = spec.get("bands").round().clamp(8.0, 64.0) as usize;
    let (lo, hi, width) = (spec.get("lo"), spec.get("hi"), spec.get("width"));
    // The frequency axis: the bank's range with a margin, log.
    let (f0, f1) = (lo / 1.25, hi * 1.25);
    let span = cantus_dsp::math::log2(f1 / f0);
    let plot = Rect::from_min_max(
        area.min + vec2(0.0, 18.0 * k),
        area.max - vec2(0.0, 16.0 * k),
    );
    let x_of = |hz: f32| plot.min.x + plot.width() * cantus_dsp::math::log2(hz / f0) / span;
    // Hz marks below.
    for (hz, label) in [
        (100.0f32, "100 Hz"),
        (300.0, "300 Hz"),
        (1_000.0, "1 kHz"),
        (3_000.0, "3 kHz"),
        (10_000.0, "10 kHz"),
    ] {
        if hz < f0 || hz > f1 {
            continue;
        }
        let x = x_of(hz);
        b.vline(
            x,
            plot.min.y,
            plot.max.y,
            1.0 * k,
            with_alpha(ink.grid, 0.6),
        );
        let t = Text::new(label, Role::Micro, ink.dim).caps(false);
        canvas::put(
            b,
            pos2(x, plot.max.y + 2.0 * k),
            Align2::CENTER_TOP,
            &t,
            k,
            None,
        );
    }
    let glow = ink.voice(ink.hue, false);
    let second = ink.voice(ink.hue, true);
    let per = n.div_ceil(SHOWN).max(1);
    let cells = 24;
    let cell_h = plot.height() / cells as f32;
    for i in 0..n {
        let (c, _, _) = layout(i, n, lo, hi, width);
        let x = x_of(c);
        let next = if i + 1 < n {
            x_of(layout(i + 1, n, lo, hi, width).0)
        } else {
            x + 8.0 * k
        };
        let prev = if i > 0 {
            x_of(layout(i - 1, n, lo, hi, width).0)
        } else {
            x - 8.0 * k
        };
        let w = ((next - prev) * 0.5 * 0.72).clamp(2.0 * k, 16.0 * k);
        let g = (i / per).min(SHOWN - 1);
        let lit = if now.seen { view.lit[g] } else { 0.0 };
        let peak = if now.seen { view.peak[g] } else { 0.0 };
        for j in 0..cells {
            let y1 = plot.max.y - j as f32 * cell_h;
            let cell = Rect::from_min_max(
                pos2(x - w * 0.5, y1 - cell_h + 1.2 * k),
                pos2(x + w * 0.5, y1),
            );
            let u = (j as f32 + 0.5) / cells as f32;
            let on = u <= lit;
            let col = if on {
                with_alpha(glow, 0.35 + 0.65 * u)
            } else {
                with_alpha(ink.grid, 0.35)
            };
            b.rect(cell, Fill::Solid(col));
        }
        if peak > 0.02 {
            let y = plot.max.y - peak * plot.height();
            b.hline(
                x - w * 0.5,
                x + w * 0.5,
                y,
                1.4 * k,
                with_alpha(ink.text, 0.85),
            );
        }
        // The band's own level, −24 … +12 dB: a bar across its column.
        let db = spec.get(cantus_core::fx::loquax::BAND_KEYS[i]);
        if db != 0.0 {
            let u = ((db + 24.0) / 36.0).clamp(0.0, 1.0);
            let y = plot.max.y - u * plot.height();
            b.hline(x - w * 0.6, x + w * 0.6, y, 2.0 * k, second);
        }
    }
    // The formant: the synthesis bands' ticks moved against the analysis bands'.
    let formant = spec.get("formant");
    let ratio = cantus_dsp::math::exp2(formant / 12.0);
    let top = area.min.y + 10.0 * k;
    for i in 0..n {
        let (c, _, _) = layout(i, n, lo, hi, width);
        let xa = x_of(c);
        b.vline(
            xa,
            top - 3.0 * k,
            top + 3.0 * k,
            1.0 * k,
            with_alpha(ink.dim, 0.8),
        );
        if formant != 0.0 {
            let xs = x_of((c * ratio).clamp(f0, f1));
            b.vline(xs, top + 3.0 * k, top + 9.0 * k, 1.0 * k, second);
        }
    }
    if formant != 0.0 {
        canvas::putf(
            b,
            pos2(area.max.x, area.min.y - 2.0 * k),
            Align2::RIGHT_BOTTOM,
            Role::Micro,
            second,
            k,
            Some(ink.plate),
            format_args!("{} {formant:+.1} st", tr("loquax.l.formant")),
        );
    }
    canvas::putf(
        b,
        pos2(area.min.x + 4.0 * k, area.min.y + 20.0 * k),
        Align2::LEFT_TOP,
        Role::Micro,
        ink.text,
        k,
        Some(ink.plate),
        format_args!("{} {n}", tr("loquax.l.bands")),
    );
}

/// A level meter: a fine horizontal bar with its word and value.
#[allow(clippy::too_many_arguments)]
fn meter(
    b: &mut Brush,
    ink: &Ink,
    at: Pos2,
    w: f32,
    key: &'static str,
    db: f32,
    col: egui::Color32,
    k: f32,
) {
    let t = Text::key(key, Role::Micro, ink.dim);
    canvas::put(b, at, Align2::LEFT_TOP, &t, k, None);
    let bar = Rect::from_min_size(at + vec2(0.0, 16.0 * k), vec2(w, 5.0 * k));
    b.rect(bar, Fill::Solid(with_alpha(ink.grid, 0.6)));
    let u = share(db);
    b.rect(
        Rect::from_min_size(bar.min, vec2(w * u, bar.height())),
        Fill::Solid(col),
    );
    if db > -199.0 {
        canvas::putf(
            b,
            pos2(at.x + w, at.y),
            Align2::RIGHT_TOP,
            Role::Micro,
            ink.text,
            k,
            None,
            format_args!("{db:.0} dB"),
        );
    }
}

fn who(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, now: &Reading, spec: &EffectSpec) {
    let k = s.k;
    let area = pane(b, ink, r, "loquax.d.who", k);
    let glow = ink.voice(ink.hue, false);
    let second = ink.voice(ink.hue, true);
    let swap = spec.get("swap") >= 0.5;
    let src = spec.get("src").round() as i32;
    let voice_key = if swap {
        "loquax.d.voice.key"
    } else {
        "loquax.d.voice.track"
    };
    let carrier_key = match (src, swap) {
        (_, true) => "loquax.d.carrier.track",
        (0, _) => "loquax.d.carrier.key",
        (2, _) => "loquax.d.carrier.both",
        _ => "loquax.d.carrier.synth",
    };
    let w = area.width();
    let mut y = area.min.y;
    // The voice.
    canvas::put(
        b,
        pos2(area.min.x, y),
        Align2::LEFT_TOP,
        &Text::key("loquax.d.voice", Role::Label, glow),
        k,
        None,
    );
    y += 18.0 * k;
    canvas::put(
        b,
        pos2(area.min.x, y),
        Align2::LEFT_TOP,
        &Text::key(voice_key, Role::Micro, ink.text).caps(false),
        k,
        None,
    );
    y += 18.0 * k;
    meter(
        b,
        ink,
        pos2(area.min.x, y),
        w,
        "loquax.d.level",
        now.modulator_db,
        glow,
        k,
    );
    y += 34.0 * k;
    // The arrow from the voice into the carrier.
    let cx = area.min.x + 18.0 * k;
    b.segment(pos2(cx, y), pos2(cx, y + 18.0 * k), 1.4 * k, ink.dim, true);
    b.polygon(
        &[
            pos2(cx - 4.0 * k, y + 13.0 * k),
            pos2(cx + 4.0 * k, y + 13.0 * k),
            pos2(cx, y + 19.0 * k),
        ],
        ink.dim,
    );
    canvas::put(
        b,
        pos2(cx + 10.0 * k, y + 9.0 * k),
        Align2::LEFT_CENTER,
        &Text::key("loquax.d.shapes", Role::Micro, ink.dim).caps(false),
        k,
        None,
    );
    y += 26.0 * k;
    // The carrier.
    canvas::put(
        b,
        pos2(area.min.x, y),
        Align2::LEFT_TOP,
        &Text::key("loquax.d.carrier", Role::Label, second),
        k,
        None,
    );
    y += 18.0 * k;
    canvas::put(
        b,
        pos2(area.min.x, y),
        Align2::LEFT_TOP,
        &Text::key(carrier_key, Role::Micro, ink.text).caps(false),
        k,
        None,
    );
    y += 18.0 * k;
    meter(
        b,
        ink,
        pos2(area.min.x, y),
        w,
        "loquax.d.level",
        now.carrier_db,
        second,
        k,
    );
    y += 34.0 * k;
    // The synthesiser's voices (or the robot's note), the consonant light and the hold.
    if spec.get("robot") >= 0.5 {
        let note = spec.get("rnote").round() as i32;
        let names = [
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
        ];
        let name = names[note.rem_euclid(12) as usize];
        canvas::putf(
            b,
            pos2(area.min.x, y),
            Align2::LEFT_TOP,
            Role::Micro,
            if now.robot { second } else { ink.text },
            k,
            None,
            format_args!("{} {name}{}", tr("loquax.d.robot"), note / 12 - 1),
        );
    } else {
        canvas::putf(
            b,
            pos2(area.min.x, y),
            Align2::LEFT_TOP,
            Role::Micro,
            ink.text,
            k,
            None,
            format_args!("{} {:.0}", tr("loquax.d.voices"), now.voices),
        );
    }
    y += 22.0 * k;
    let lamp = |b: &mut Brush, x: f32, on: bool, key: &'static str| {
        b.circle(
            pos2(x + 5.0 * k, y + 6.0 * k),
            4.0 * k,
            Fill::Solid(if on {
                glow
            } else {
                with_alpha(ink.grid_strong, 0.8)
            }),
        );
        canvas::put(
            b,
            pos2(x + 14.0 * k, y + 6.0 * k),
            Align2::LEFT_CENTER,
            &Text::key(key, Role::Micro, if on { ink.text } else { ink.dim }).caps(false),
            k,
            None,
        );
    };
    lamp(b, area.min.x, now.gate > 0.5, "loquax.d.consonant");
    lamp(
        b,
        area.min.x + w * 0.55,
        now.hold || spec.get("hold") >= 0.5,
        "loquax.d.hold",
    );
    // The bank in words: its bands and slope, its range, its followers.
    y += 30.0 * k;
    b.hline(
        area.min.x,
        area.max.x,
        y - 8.0 * k,
        1.0 * k,
        with_alpha(ink.grid_strong, 0.7),
    );
    let order = [4, 6, 8][(spec.get("order").round().clamp(0.0, 2.0)) as usize];
    let lines: [(&'static str, String); 4] = [
        (
            "loquax.l.bands",
            format!(
                "{:.0} \u{b7} {} {order}",
                spec.get("bands"),
                tr("loquax.d.order")
            ),
        ),
        (
            "loquax.d.range",
            format!(
                "{} \u{2013} {}",
                cantus_core::fx::readout::readout(spec.get("lo"), "Hz"),
                cantus_core::fx::readout::readout(spec.get("hi"), "Hz")
            ),
        ),
        (
            "loquax.card.follow",
            format!("{:.1} ms \u{b7} {:.0} ms", spec.get("att"), spec.get("rel")),
        ),
        (
            "loquax.l.formant",
            format!(
                "{:+.1} st \u{b7} {} {:+.1} dB",
                spec.get("formant"),
                tr("loquax.d.tilt"),
                spec.get("tilt")
            ),
        ),
    ];
    for (key, value) in lines {
        canvas::put(
            b,
            pos2(area.min.x, y),
            Align2::LEFT_TOP,
            &Text::key(key, Role::Micro, ink.dim),
            k,
            None,
        );
        canvas::put(
            b,
            pos2(area.max.x, y),
            Align2::RIGHT_TOP,
            &Text::new(&value, Role::Micro, ink.text).caps(false),
            k,
            None,
        );
        y += 18.0 * k;
    }
}
