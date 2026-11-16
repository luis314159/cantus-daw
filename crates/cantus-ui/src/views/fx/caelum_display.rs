//! Caelum's display: the sky. Dark in every theme (the unit's glass look).
//!
//! - **The tank** (the left, most of the width): the last eight seconds right to left, the
//!   now at the right edge. The tail is a band of light over the input's register, as
//!   thick and as bright as the tank's energy; the **shimmer** climbs above it in layers a
//!   voice's interval apart — each fainter by the shimmer's share, and each further up the
//!   older the tail, so the octaves are seen rising as the sound ages. Infinite holds a
//!   line of light at the top; freeze a held horizon. The input's peak is a tick at the
//!   bottom of the now; the ducking's reduction a bar beside it.
//! - **The voices** (top right): a ladder of semitones, −36 … +36, the unison at its middle,
//!   each voice a rung with its interval written whole.
//! - **The decay by octave** (bottom right): the decay the tank achieves at the eight
//!   octaves, 63 Hz … 8 kHz, on a time scale of 0.3 … 60 s (∞ at the top while infinite).

use super::super::canvas::{self, Ink};
use super::super::host::FxHost;
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_engine::fx::caelum::t;
use egui::{pos2, vec2, Align2, Rect};

/// Readings kept: eight seconds at thirty a second.
pub const KEPT: usize = 240;

/// One block's numbers as the display reads them.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Reading {
    /// Any telemetry at all (an engine playing, or the demo).
    pub seen: bool,
    pub energy_db: f32,
    pub shimmer: f32,
    pub input: f32,
    pub duck_db: f32,
    pub freeze: f32,
    pub bloom: f32,
    pub predelay_ms: f32,
    pub line_ms: f32,
    pub achieved: [f32; 8],
}

impl Reading {
    pub fn of(tel: &[f32; 64]) -> Reading {
        let mut achieved = [0.0; 8];
        achieved.copy_from_slice(&tel[t::ACHIEVED..t::ACHIEVED + 8]);
        Reading {
            seen: tel[t::SAMPLE_RATE] > 0.0,
            energy_db: tel[t::ENERGY],
            shimmer: tel[t::SHIMMER].clamp(0.0, 1.0),
            input: tel[t::INPUT],
            duck_db: tel[t::DUCK],
            freeze: tel[t::FREEZE],
            bloom: tel[t::BLOOM],
            predelay_ms: tel[t::PREDELAY],
            line_ms: tel[t::LINE],
            achieved,
        }
    }
}

/// The display's memory: the readings of the last eight seconds, newest last.
#[derive(Debug, Default)]
pub struct View {
    history: Vec<Reading>,
    /// Seconds since the last reading was kept.
    since: f32,
    /// The rise's phase (the shimmer's layers drift upward), 0 … 1.
    pub phase: f32,
}

impl View {
    /// Keep a reading every thirtieth of a second.
    pub fn update(&mut self, r: &Reading, dt: f32) {
        if !r.seen {
            return;
        }
        self.since += dt;
        self.phase = (self.phase + dt * 0.08).fract();
        if self.since >= 1.0 / 30.0 || self.history.is_empty() {
            self.since = 0.0;
            if self.history.len() == KEPT {
                self.history.remove(0);
            }
            self.history.push(*r);
        }
    }

    /// Fill the history at once (the screenshots' demo).
    pub fn seed(&mut self, readings: &[Reading]) {
        let from = readings.len().saturating_sub(KEPT);
        self.history = readings[from..].to_vec();
        self.phase = 0.37;
    }

    pub fn len(&self) -> usize {
        self.history.len()
    }
}

/// The parts of the display.
struct Parts {
    sky: Rect,
    ladder: Rect,
    decay: Rect,
}

fn parts(r: Rect, k: f32) -> Parts {
    let inner = r.shrink(10.0 * k);
    let side = (inner.width() * 0.27).clamp(220.0 * k, 340.0 * k);
    let sky = Rect::from_min_max(inner.min, pos2(inner.max.x - side - 12.0 * k, inner.max.y));
    let right = Rect::from_min_max(pos2(sky.max.x + 12.0 * k, inner.min.y), inner.max);
    let split = right.min.y + right.height() * 0.46;
    Parts {
        sky,
        ladder: Rect::from_min_max(right.min, pos2(right.max.x, split - 6.0 * k)),
        decay: Rect::from_min_max(pos2(right.min.x, split + 6.0 * k), right.max),
    }
}

/// Energy in dB as a share of the sky's light, 0 … 1 (−90 dB nothing, −12 dB all).
fn light(db: f32) -> f32 {
    ((db + 90.0) / 78.0).clamp(0.0, 1.0)
}

pub fn show(
    host: &mut dyn FxHost,
    view: &mut View,
    ui: &mut egui::Ui,
    s: &Style,
    r: Rect,
    now: &Reading,
) {
    let spec = host.spec().clone();
    let k = s.k;
    let ink = Ink::of(s);
    let p = parts(r, k);
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, None);
        sky(b, s, &ink, p.sky, view, now, &spec);
        ladder(b, s, &ink, p.ladder, &spec);
        decay(b, s, &ink, p.decay, now, &spec);
        canvas::edge_b(b, r, &ink);
    });
}

/// A frame and its title, the plain way every pane of this display is drawn.
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

#[allow(clippy::too_many_arguments)]
fn sky(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    view: &View,
    now: &Reading,
    spec: &cantus_core::EffectSpec,
) {
    let k = s.k;
    let area = pane(b, ink, r, "caelum.d.sky", k);
    // The horizon: the ground of the sky a little lighter toward the bottom.
    let glow = ink.voice(ink.hue, false);
    let spark = ink.voice(ink.hue, true);
    let rows = 28;
    for i in 0..rows {
        let y0 = area.min.y + area.height() * i as f32 / rows as f32;
        let y1 = area.min.y + area.height() * (i + 1) as f32 / rows as f32;
        let u = i as f32 / rows as f32;
        b.rect(
            Rect::from_min_max(pos2(area.min.x, y0), pos2(area.max.x, y1)),
            Fill::Solid(with_alpha(glow, 0.05 * u * u)),
        );
    }
    // A faint star field: fixed points, brighter where the tank holds energy.
    let lit = light(now.energy_db);
    for i in 0..90u32 {
        let h = i.wrapping_mul(2_654_435_761);
        let x = area.min.x + area.width() * ((h & 0xffff) as f32 / 65_535.0);
        let y = area.min.y + area.height() * 0.85 * (((h >> 16) & 0xffff) as f32 / 65_535.0);
        let a = 0.10 + 0.25 * lit * (((h >> 7) & 7) as f32 / 7.0);
        b.dot(pos2(x, y), 0.8 * k, with_alpha(ink.text, a));
    }
    // Pitch rows: an octave apart from the input's register (the bottom third) upward.
    let base = area.max.y - area.height() * 0.22;
    let octave = area.height() * 0.17;
    for o in 0..5 {
        let y = base - o as f32 * octave;
        canvas::dashed(
            b,
            pos2(area.min.x, y),
            pos2(area.max.x, y),
            1.0 * k,
            with_alpha(ink.grid, 0.7),
            3.0 * k,
            5.0 * k,
        );
    }
    let pa = spec.get("pa");
    let pb = spec.get("pb");
    let blvl = spec.get("pblvl");
    let n = view.history.len();
    let col_w = area.width() / (KEPT - 1) as f32;
    // A sound climbs one interval about every pass of the tank's mean loop times the
    // shimmer's share — drawn slower, so the steps read: one layer a second at full share.
    let shim = spec.get("shim").clamp(0.0, 1.0);
    let climb_s = 1.0 / shim.max(0.05);
    // Each band: its centre line over the history, its half-thickness, its colour; drawn
    // as a soft halo and a brighter core between two lines (no seams between columns).
    let band = |b: &mut Brush, y: &dyn Fn(usize, &Reading) -> (f32, f32), col| {
        let mut top = Vec::with_capacity(n);
        let mut bottom = Vec::with_capacity(n);
        let mut core_t = Vec::with_capacity(n);
        let mut core_b = Vec::with_capacity(n);
        for (j, rd) in view.history.iter().enumerate() {
            let x = area.max.x - (n - 1 - j) as f32 * col_w;
            let (yc, h) = y(n - 1 - j, rd);
            top.push(pos2(x, yc - h));
            bottom.push(pos2(x, yc + h));
            core_t.push(pos2(x, yc - h * 0.45));
            core_b.push(pos2(x, yc + h * 0.45));
        }
        if n > 1 {
            b.between(&top, &bottom, with_alpha(col, 0.22));
            b.between(&core_t, &core_b, with_alpha(col, 0.55));
        }
    };
    // The tail itself, a band over the input's register as thick as its energy.
    band(
        b,
        &|_, rd| {
            let e = light(rd.energy_db);
            (base, if e > 0.0 { (3.0 + 24.0 * e) * k } else { 0.0 })
        },
        glow,
    );
    // The shimmer's layers: a voice's interval apart, each lit once the sound has aged
    // long enough to climb to it, fainter by the share at every step.
    for (iv, lvl, col) in [(pa, 1.0 - 0.5 * blvl, glow), (pb, blvl, spark)] {
        if lvl <= 0.01 || iv.abs() < 0.5 || shim <= 0.0 {
            continue;
        }
        let step = octave * iv / 12.0;
        for layer in 1..=4 {
            let yl = base - step * layer as f32;
            if yl < area.min.y + 8.0 * k || yl > area.max.y {
                continue;
            }
            band(
                b,
                &|age, rd| {
                    let secs = age as f32 / 30.0;
                    let lit = ((secs - (layer as f32 - 1.0) * climb_s) / climb_s).clamp(0.0, 1.0);
                    let e = light(rd.energy_db) * lvl * shim.powi(layer - 1) * lit;
                    (yl, if e > 0.01 { (2.0 + 20.0 * e) * k } else { 0.0 })
                },
                col,
            );
        }
    }
    let _ = view.phase;
    // Infinite: a line of light across the top; freeze: a held horizon.
    let inf = spec.get("inf") >= 0.5;
    if inf {
        b.hline(
            area.min.x,
            area.max.x,
            area.min.y + 2.0 * k,
            1.4 * k,
            with_alpha(spark, 0.85),
        );
        canvas::put_key(
            b,
            pos2(area.max.x - 4.0 * k, area.min.y + 6.0 * k),
            Align2::RIGHT_TOP,
            "caelum.d.infinite",
            Role::Micro,
            spark,
            k,
        );
    }
    if now.freeze > 0.01 || spec.get("freeze") >= 0.5 {
        let y = base;
        b.hline(
            area.min.x,
            area.max.x,
            y,
            2.0 * k,
            with_alpha(ink.text, 0.7 * now.freeze.max(0.5)),
        );
        canvas::put_key(
            b,
            pos2(area.min.x + 4.0 * k, y - 6.0 * k),
            Align2::LEFT_BOTTOM,
            "caelum.d.frozen",
            Role::Micro,
            ink.text,
            k,
        );
    }
    // The now: the input's peak a tick, the ducking a bar, the time scale below.
    let x = area.max.x;
    let peak = (1.0 + cantus_dsp::math::log10(now.input.max(1e-6)) / 4.0).clamp(0.0, 1.0);
    b.vline(
        x,
        area.max.y - 18.0 * k * peak - 2.0 * k,
        area.max.y,
        2.0 * k,
        ink.text,
    );
    if spec.get("duck") > 0.0 {
        let d = (now.duck_db / spec.get("duck").max(1.0)).clamp(0.0, 1.0);
        let bar = Rect::from_min_max(
            pos2(x - 10.0 * k, area.min.y + 24.0 * k),
            pos2(x - 6.0 * k, area.min.y + 24.0 * k + 60.0 * k * d),
        );
        b.rect(bar, Fill::Solid(with_alpha(ink.danger, 0.85)));
        canvas::putf(
            b,
            pos2(x - 14.0 * k, area.min.y + 24.0 * k),
            Align2::RIGHT_TOP,
            Role::Micro,
            ink.danger,
            k,
            Some(ink.plate),
            format_args!(
                "{} \u{2212}{:.1} dB",
                super::super::frame::tr("caelum.d.ducking"),
                now.duck_db
            ),
        );
    }
    let dim = ink.dim;
    canvas::put_key(
        b,
        pos2(area.min.x, area.max.y + 2.0 * k),
        Align2::LEFT_TOP,
        "caelum.d.ago",
        Role::Micro,
        dim,
        k,
    );
    canvas::put_key(
        b,
        pos2(area.max.x, area.max.y + 2.0 * k),
        Align2::RIGHT_TOP,
        "caelum.d.now",
        Role::Micro,
        dim,
        k,
    );
    // The space in words, top left under the title.
    let decay = if inf {
        super::super::frame::tr("caelum.d.infinite").to_string()
    } else {
        format!("{:.1} s", spec.get("decay"))
    };
    canvas::putf(
        b,
        pos2(area.min.x + 4.0 * k, area.min.y + 4.0 * k),
        Align2::LEFT_TOP,
        Role::Micro,
        ink.text,
        k,
        Some(ink.plate),
        format_args!(
            "{} {decay} \u{b7} {} {:.0} ms \u{b7} {} {:.0} ms",
            super::super::frame::tr("caelum.l.decay"),
            super::super::frame::tr("caelum.l.pre"),
            now.predelay_ms,
            super::super::frame::tr("caelum.d.lines"),
            now.line_ms,
        ),
    );
    let _ = mix;
}

fn ladder(b: &mut Brush, s: &Style, ink: &Ink, r: Rect, spec: &cantus_core::EffectSpec) {
    let k = s.k;
    let area = pane(b, ink, r, "caelum.d.voices", k);
    let x = area.min.x + 26.0 * k;
    let y_of = |st: f32| area.center().y - (st / 36.0) * area.height() * 0.46;
    b.vline(x, y_of(36.0), y_of(-36.0), 1.0 * k, ink.grid_strong);
    for st in (-36..=36).step_by(12) {
        let y = y_of(st as f32);
        b.hline(x - 6.0 * k, x + 6.0 * k, y, 1.0 * k, ink.grid_strong);
        canvas::putf(
            b,
            pos2(x - 9.0 * k, y),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.dim,
            k,
            None,
            format_args!("{st:+}"),
        );
    }
    let glow = ink.voice(ink.hue, false);
    let spark = ink.voice(ink.hue, true);
    let shim = spec.get("shim");
    for (key, st, lvl, col) in [
        ("caelum.l.first", spec.get("pa"), 1.0, glow),
        ("caelum.l.second", spec.get("pb"), spec.get("pblvl"), spark),
    ] {
        let first = key == "caelum.l.first";
        let y = y_of(st);
        let a = if shim > 0.0 { 0.35 + 0.65 * lvl } else { 0.3 };
        b.segment(
            pos2(x, y),
            pos2(area.max.x - 4.0 * k, y),
            2.0 * k,
            with_alpha(col, a),
            true,
        );
        b.circle(pos2(x, y), 4.0 * k, Fill::Solid(with_alpha(col, a)));
        canvas::putf(
            b,
            if first {
                pos2(x + 12.0 * k, y - 3.0 * k)
            } else {
                pos2(x + 12.0 * k, y + 3.0 * k)
            },
            if first {
                Align2::LEFT_BOTTOM
            } else {
                Align2::LEFT_TOP
            },
            Role::Micro,
            with_alpha(ink.text, a.max(0.6)),
            k,
            None,
            format_args!("{} {st:+.0} st", super::super::frame::tr(key)),
        );
    }
}

fn decay(
    b: &mut Brush,
    s: &Style,
    ink: &Ink,
    r: Rect,
    now: &Reading,
    spec: &cantus_core::EffectSpec,
) {
    let k = s.k;
    let area = pane(b, ink, r, "caelum.d.decay", k);
    let plot = Rect::from_min_max(
        area.min + vec2(28.0 * k, 0.0),
        area.max - vec2(0.0, 14.0 * k),
    );
    let lo = cantus_dsp::math::log10(0.3f32);
    let hi = cantus_dsp::math::log10(60.0f32);
    let y_of = |s: f32| {
        let u = (cantus_dsp::math::log10(s.clamp(0.3, 60.0)) - lo) / (hi - lo);
        plot.max.y - u * plot.height()
    };
    for (sec, label) in [(1.0f32, "1 s"), (10.0, "10 s"), (60.0, "60 s")] {
        let y = y_of(sec);
        b.hline(
            plot.min.x,
            plot.max.x,
            y,
            1.0 * k,
            with_alpha(ink.grid, 0.8),
        );
        canvas::put(
            b,
            pos2(plot.min.x - 4.0 * k, y),
            Align2::RIGHT_CENTER,
            &Text::new(label, Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    let names = [
        "63 Hz", "125 Hz", "250 Hz", "500 Hz", "1 kHz", "2 kHz", "4 kHz", "8 kHz",
    ];
    let w = plot.width() / 8.0;
    let glow = ink.voice(ink.hue, false);
    let inf = spec.get("inf") >= 0.5;
    let mut pts = Vec::with_capacity(8);
    for (i, a) in now.achieved.iter().enumerate() {
        let x = plot.min.x + w * (i as f32 + 0.5);
        let v = if a.is_finite() { *a } else { 60.0 };
        let y = if now.seen {
            y_of(v)
        } else {
            y_of(spec.get("decay"))
        };
        pts.push(pos2(x, y));
        b.rect(
            Rect::from_min_max(pos2(x - w * 0.3, y), pos2(x + w * 0.3, plot.max.y)),
            Fill::Solid(with_alpha(glow, 0.18)),
        );
        canvas::put(
            b,
            pos2(x, plot.max.y + 2.0 * k),
            Align2::CENTER_TOP,
            &Text::new(names[i], Role::Micro, ink.dim).caps(false),
            k,
            None,
        );
    }
    canvas::trace(b, &pts, 1.6 * k, glow, ink);
    for p in &pts {
        b.circle(*p, 2.5 * k, Fill::Solid(glow));
    }
    if inf {
        canvas::put_key(
            b,
            pos2(plot.max.x, plot.min.y),
            Align2::RIGHT_TOP,
            "caelum.d.infinite",
            Role::Micro,
            ink.voice(ink.hue, true),
            k,
        );
    }
}
