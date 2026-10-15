//! What a modulator looks like in its panel (ADR 0057): its movement drawn from the very
//! functions the engine plays (`cantus_core::modulator::eval`), so the picture is the
//! sound — an LFO's cycle, the steps, a random source's draws, an envelope's shape, a
//! follower's level — with where it is now as a dot, and its live output as a meter.

use crate::kit::color::with_alpha;
use crate::kit::{well, Brush, Fill, State, Style};
use cantus_core::modulator::eval::{output, ModTime};
use cantus_core::modulator::{ModRate, ModSource};
use egui::{pos2, Pos2, Rect};

/// Points drawn across a preview.
const POINTS: usize = 96;

/// The beats one preview spans: a cycle, the whole sequence, a few draws.
pub fn preview_beats(src: &ModSource) -> f64 {
    use cantus_dsp::modfx_clock::division_beats;
    match src {
        ModSource::Lfo(l) => match l.rate {
            ModRate::Sync { division } => division_beats(f32::from(division)),
            ModRate::Hz { hz } => 2.0 * f64::from(hz).recip().min(8.0),
        },
        ModSource::Step(s) => division_beats(f32::from(s.division)) * s.steps.len() as f64,
        ModSource::Random(r) => match r.rate {
            ModRate::Sync { division } => 4.0 * division_beats(f32::from(division)),
            ModRate::Hz { hz } => 8.0 * f64::from(hz).recip().min(4.0),
        },
        ModSource::Envelope(e) => {
            // Attack, decay, a held stretch as long again, the release: at 120 BPM.
            let ms = e.attack_ms + e.decay_ms + e.release_ms;
            f64::from(ms.max(50.0)) * 2.0 / 500.0
        }
        ModSource::Follower(_) => 1.0,
    }
}

/// The source's value at a share `u` (0…1) of the preview, −1…1 or 0…1.
fn sample(src: &ModSource, u: f64) -> f32 {
    let beats = u * preview_beats(src);
    match src {
        ModSource::Envelope(e) => {
            // A note held through attack and decay and as long again, then let go.
            let spb = 0.5;
            let held = f64::from(e.attack_ms + e.decay_ms) * 2e-3;
            let g = cantus_core::modulator::eval::Gate {
                on: 0.0,
                off: held.max(0.05),
            };
            cantus_core::modulator::eval::gate_level(e, 0.0, g, beats * spb)
        }
        _ => output(src, &ModTime::at_beats(beats, 0.0, 120.0), None, None),
    }
}

/// The movement of `src` across `rect` (a well), in `hue`: bipolar around the middle, or
/// from the bottom; `now` is where it is (its output, −1…1 or 0…1) drawn as a level mark.
pub fn preview(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    st: State,
    src: &ModSource,
    now: Option<f32>,
    hue: egui::Color32,
) {
    well::paint(b, rect, s, st, None);
    let inner = rect.shrink2(egui::vec2(s.s(4.0), s.s(3.0)));
    let bipolar = src.bipolar();
    let y_of = |v: f32| {
        let p = if bipolar { 0.5 + 0.5 * v } else { v };
        inner.bottom() - inner.height() * p.clamp(0.0, 1.0)
    };
    if bipolar {
        b.hline(
            inner.left(),
            inner.right(),
            y_of(0.0),
            b.px(),
            with_alpha(s.t.text_dim, 0.25),
        );
    }
    let a = st.alpha();
    if let ModSource::Follower(_) = src {
        // A follower has no shape ahead: its level, live.
        let v = now.unwrap_or(0.0).clamp(0.0, 1.0);
        let bar = Rect::from_min_max(pos2(inner.left(), y_of(v)), inner.right_bottom());
        b.rect(bar, Fill::Solid(with_alpha(hue, 0.55 * a)));
        return;
    }
    let mut pts: [Pos2; POINTS] = [Pos2::ZERO; POINTS];
    for (i, p) in pts.iter_mut().enumerate() {
        let u = i as f64 / (POINTS - 1) as f64;
        *p = pos2(
            inner.left() + inner.width() * u as f32,
            y_of(sample(src, u)),
        );
    }
    b.polyline(&pts, s.s(1.5), hue.gamma_multiply(a));
    if let Some(v) = now {
        // Where it is now: a short mark at the right edge, at its level.
        let y = y_of(v);
        b.segment(
            pos2(inner.right() - s.s(5.0), y),
            pos2(inner.right() + s.s(2.0), y),
            s.s(2.0),
            s.t.text.gamma_multiply(a),
            true,
        );
    }
}

/// A modulator's live output as a meter in `rect`: from the middle for a bipolar source,
/// from the left otherwise.
pub fn meter(b: &mut Brush, rect: Rect, s: &Style, v: f32, bipolar: bool, hue: egui::Color32) {
    b.rrect(rect, s.r(2.0), Fill::Solid(s.t.well));
    let x_of = |p: f32| rect.left() + rect.width() * p.clamp(0.0, 1.0);
    let (x0, x1) = if bipolar {
        let (m, x) = (x_of(0.5), x_of(0.5 + 0.5 * v));
        (m.min(x), m.max(x))
    } else {
        (rect.left(), x_of(v))
    };
    if x1 - x0 > 0.25 * b.px() {
        b.rect(
            Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.bottom())),
            Fill::Solid(hue),
        );
    }
}

/// The step editor's bars in `rect`: each step from the middle (bipolar) or the bottom,
/// the step playing now lit.
pub fn steps(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    values: &[f32],
    unipolar: bool,
    playing: Option<usize>,
    hue: egui::Color32,
) {
    well::paint(b, rect, s, State::REST, None);
    let inner = rect.shrink2(egui::vec2(s.s(3.0), s.s(3.0)));
    let n = values.len().max(1);
    let w = inner.width() / n as f32;
    let y_of = |v: f32| {
        let p = if unipolar {
            0.5 * (v + 1.0)
        } else {
            0.5 + 0.5 * v
        };
        inner.bottom() - inner.height() * p.clamp(0.0, 1.0)
    };
    let base = if unipolar { inner.bottom() } else { y_of(0.0) };
    for (i, v) in values.iter().enumerate() {
        let x0 = inner.left() + w * i as f32 + s.s(1.0);
        let x1 = inner.left() + w * (i + 1) as f32 - s.s(1.0);
        let y = y_of(*v);
        let lit = playing == Some(i);
        let col = if lit { hue } else { with_alpha(hue, 0.55) };
        b.rect(
            Rect::from_min_max(
                pos2(x0, y.min(base)),
                pos2(x1.max(x0 + b.px()), y.max(base)),
            ),
            Fill::Solid(col),
        );
    }
    if !unipolar {
        b.hline(
            inner.left(),
            inner.right(),
            base,
            b.px(),
            with_alpha(s.t.text_dim, 0.35),
        );
    }
}

/// The step under `x` in a step editor at `rect` of `n` steps, and the value at `y`.
/// Steps hold −1…1 whatever their polarity: the editor's bottom is −1, its top 1.
pub fn step_at(rect: Rect, s: &Style, n: usize, p: Pos2) -> (usize, f32) {
    let inner = rect.shrink2(egui::vec2(s.s(3.0), s.s(3.0)));
    let n = n.max(1);
    let i = (((p.x - inner.left()) / inner.width()) * n as f32).floor();
    let i = (i.max(0.0) as usize).min(n - 1);
    let pos = ((inner.bottom() - p.y) / inner.height().max(1.0)).clamp(0.0, 1.0);
    let v = 2.0 * pos - 1.0;
    (i, (v * 100.0).round() / 100.0)
}
