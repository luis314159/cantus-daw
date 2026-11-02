//! The light visualiser drawn (ADR 0091 §4), like a broadcast overlay: the master's
//! spectrum as rounded bars in the theme's accent with a soft glow (or its level as one
//! bar), a glass chip with the song's name and tempo, and one with the bar and its beats —
//! the beat lit and pulsing as it lands. Over a transparent window every part sits on a
//! soft glass plate so it reads over any picture; over a chroma key nothing is
//! half-transparent (a key cannot cut a soft edge cleanly): plates are opaque, glows off.

use super::companion::{StageSettings, VisMode, VisSnap};
use super::vis::BANDS;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Rect};

/// Where the visualiser's parts go in a window of `r` (points): the band at the bottom,
/// the song's chip at the top left, the bar's at the top right.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Layout {
    pub band: Option<Rect>,
    pub song: Option<Rect>,
    pub bar: Option<Rect>,
}

pub fn layout(r: Rect, st: &StageSettings, k: f32) -> Layout {
    let m = 12.0 * k;
    let band = (st.vis != VisMode::Off).then(|| {
        let h = (r.height() * 0.16).clamp(40.0 * k, 120.0 * k);
        let h = if st.vis == VisMode::Level {
            h.min(22.0 * k)
        } else {
            h
        };
        Rect::from_min_max(
            pos2(r.left() + m, r.bottom() - m - h),
            pos2(r.right() - m, r.bottom() - m),
        )
    });
    let chip_h = 40.0 * k;
    let bar_w = 108.0 * k;
    let bar = st.bar_beat.then(|| {
        Rect::from_min_size(
            pos2(r.right() - m - bar_w, r.top() + m),
            vec2(bar_w, chip_h),
        )
    });
    let song = st.song.then(|| {
        let right = bar.map_or(r.right() - m, |b| b.left() - 8.0 * k);
        let w = (right - r.left() - m).clamp(0.0, 260.0 * k);
        Rect::from_min_size(pos2(r.left() + m, r.top() + m), vec2(w, chip_h))
    });
    Layout {
        band,
        song: song.filter(|s| s.width() >= 60.0 * k),
        bar,
    }
}

/// A glass plate: a soft shadow, the panel's colour see-through, an accent edge.
fn plate(b: &mut Brush, r: Rect, s: &Style, solid: bool) {
    let k = s.k;
    let radius = 10.0 * k;
    if !solid {
        b.rrect_soft(
            r.translate(vec2(0.0, 2.0 * k)),
            radius,
            Fill::Solid(with_alpha(s.t.shadow, 0.45)),
            10.0 * k,
        );
    }
    let a = if solid { 1.0 } else { 0.62 };
    b.rrect(
        r,
        radius,
        Fill::V(
            with_alpha(shade(s.t.panel, 0.04), a),
            with_alpha(shade(s.t.panel, -0.03), a),
        ),
    );
    b.rrect_stroke(
        r,
        radius,
        b.px(),
        with_alpha(
            mix(s.t.edge, s.t.highlight, 0.35),
            if solid { 1.0 } else { 0.7 },
        ),
    );
}

/// Paint the visualiser's parts as `l` places them.
pub fn paint(b: &mut Brush, l: &Layout, v: &VisSnap, st: &StageSettings, s: &Style) {
    // Opaque plates, no glows over a chroma key: a key cuts no soft edge cleanly.
    let solid = st.backdrop.chroma().is_some();
    if let Some(r) = l.band {
        match st.vis {
            VisMode::Spectrum => spectrum(b, r, &v.bands, s, solid),
            VisMode::Level => level(b, r, v.level, s, solid),
            VisMode::Off => {}
        }
    }
    if let Some(r) = l.song {
        song(b, r, v, s, solid);
    }
    if let Some(r) = l.bar {
        bar(b, r, v, s, solid);
    }
}

fn spectrum(b: &mut Brush, r: Rect, bands: &[f32; BANDS], s: &Style, solid: bool) {
    let k = s.k;
    plate(b, r, s, solid);
    let inner = r.shrink2(vec2(10.0 * k, 8.0 * k));
    let gap = (2.5 * k).max(b.px());
    let w = ((inner.width() - gap * (BANDS - 1) as f32) / BANDS as f32).max(b.px());
    let lo = s.t.highlight;
    let hi = s.t.brand_bright;
    for (i, v) in bands.iter().enumerate() {
        let x = inner.left() + i as f32 * (w + gap);
        // A floor of one dot, so a quiet band still shows where it is.
        let h = (v * inner.height()).max(w.min(3.0 * k));
        let bar = Rect::from_min_max(pos2(x, inner.bottom() - h), pos2(x + w, inner.bottom()));
        let top = mix(lo, hi, *v);
        if !solid && *v > 0.05 {
            b.rrect_soft(
                bar.expand(1.5 * k),
                w * 0.5,
                Fill::Solid(with_alpha(top, 0.22 * v)),
                6.0 * k,
            );
        }
        b.rrect(bar, w * 0.5, Fill::V(top, shade(lo, -0.12)));
    }
}

fn level(b: &mut Brush, r: Rect, v: f32, s: &Style, solid: bool) {
    let k = s.k;
    plate(b, r, s, solid);
    let inner = r.shrink2(vec2(8.0 * k, 6.0 * k));
    let radius = inner.height() * 0.5;
    b.rrect(inner, radius, Fill::Solid(s.t.meter_well));
    let w = inner.width() * v.clamp(0.0, 1.0);
    if w <= 0.5 {
        return;
    }
    let stops = [
        (0.0, s.t.meter_low),
        (0.75, s.t.meter_high),
        (1.0, s.t.meter_clip),
    ];
    let cy = inner.center().y;
    if w <= inner.height() {
        b.circle(
            pos2(inner.left() + w * 0.5, cy),
            w * 0.5,
            Fill::Solid(stops[0].1),
        );
        return;
    }
    // Round ends, and the gradient along the bar between them as thin slices.
    let (x0, x1) = (inner.left() + radius, inner.left() + w - radius);
    b.circle(pos2(x0, cy), radius, Fill::Solid(stops[0].1));
    let end = along(&stops, (x1 - inner.left()) / inner.width());
    b.circle(pos2(x1, cy), radius, Fill::Solid(end));
    let n = 24;
    for i in 0..n {
        let a = x0 + (x1 - x0) * i as f32 / n as f32;
        let z = x0 + (x1 - x0) * (i + 1) as f32 / n as f32;
        let c = along(&stops, (z - inner.left()) / inner.width());
        b.rect(
            Rect::from_min_max(pos2(a, inner.top()), pos2(z + 0.5, inner.bottom())),
            Fill::Solid(c),
        );
    }
}

/// The colour `t` of the way along `stops` (ascending).
fn along(stops: &[(f32, egui::Color32)], t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    for w in stops.windows(2) {
        let ((t0, c0), (t1, c1)) = (w[0], w[1]);
        if t <= t1 {
            return mix(c0, c1, ((t - t0) / (t1 - t0).max(1e-6)).clamp(0.0, 1.0));
        }
    }
    stops.last().map_or(egui::Color32::TRANSPARENT, |s| s.1)
}

fn song(b: &mut Brush, r: Rect, v: &VisSnap, s: &Style, solid: bool) {
    let k = s.k;
    plate(b, r, s, solid);
    let inner = r.shrink2(vec2(12.0 * k, 4.0 * k));
    let untitled;
    let name: &str = if v.song.is_empty() {
        untitled = t("stage.vis.untitled");
        &untitled
    } else {
        &v.song
    };
    let top = Rect::from_min_size(inner.min, vec2(inner.width(), inner.height() * 0.56));
    let bottom = Rect::from_min_max(pos2(inner.left(), top.bottom()), inner.max);
    let tt = Text::new(name, Role::Body, s.t.text)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, top, &tt, s.tk());
    // Whole tempos without a decimal (120 BPM), the rest with one (92.5 BPM).
    let bpm = if (v.bpm - v.bpm.round()).abs() < 0.05 {
        format!("{:.0}", v.bpm)
    } else {
        format!("{:.1}", v.bpm)
    };
    let tempo = tf("stage.vis.bpm", &[("bpm", &bpm)]);
    let bt = Text::new(&tempo, Role::Label, s.t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, bottom, &bt, s.tk());
}

fn bar(b: &mut Brush, r: Rect, v: &VisSnap, s: &Style, solid: bool) {
    let k = s.k;
    plate(b, r, s, solid);
    let inner = r.shrink2(vec2(10.0 * k, 4.0 * k));
    let top = Rect::from_min_size(inner.min, vec2(inner.width(), inner.height() * 0.5));
    let label = tf("stage.vis.bar", &[("bar", &v.bar)]);
    let lt = Text::new(&label, Role::Label, s.t.text)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, top, &lt, s.tk());
    // One dot a beat; the beat that lands lit and pulsing.
    let n = v.beats_per_bar.clamp(1, 16) as usize;
    let row = Rect::from_min_max(pos2(inner.left(), top.bottom()), inner.max);
    let step = (row.width() / n as f32).min(14.0 * k);
    let rad = (step * 0.3).min(4.0 * k);
    for i in 0..n {
        let c = pos2(row.left() + step * (i as f32 + 0.5), row.center().y);
        let on = v.playing && i as u32 == v.beat;
        if on {
            let pulse = (1.0 - v.beat_phase).powi(2);
            let accent = if i == 0 {
                s.t.brand_bright
            } else {
                s.t.highlight
            };
            if !solid {
                b.circle(
                    c,
                    rad * (1.6 + 1.2 * pulse),
                    Fill::Solid(with_alpha(accent, 0.25 * pulse)),
                );
            }
            b.circle(c, rad * (1.0 + 0.25 * pulse), Fill::Solid(accent));
        } else {
            b.circle(c, rad * 0.8, Fill::Solid(with_alpha(s.t.text_dim, 0.55)));
        }
    }
}
