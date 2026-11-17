//! Tabula's artwork (ADR 0131): the illustrations a unit wears when it stands on the board
//! as a pedal — the motifs `pedal_art` draws for Fragor's five circuits, extended by the
//! families of the whole Cantus range: a sine for modulation, repeats fading for echoes,
//! rings for spaces, a needle for dynamics, a curve for tone, steps for time and rhythm,
//! a spiral for rotation. Original to Cantus, vectors in fine strokes, the same each frame.

use super::pedal_art::Motif;
use crate::kit::color::with_alpha;
use crate::kit::{Brush, Fill};
use egui::{pos2, vec2, Color32, Pos2, Rect};
use std::f32::consts::{PI, TAU};

/// Paint one of Tabula's motifs over the art area `r` (`u` the enclosure's scale).
pub fn motif(b: &mut Brush, u: f32, r: Rect, m: Motif, print: Color32, accent: Color32) {
    let fine = 0.8 * u;
    let c0 = r.center();
    let mut pts = [Pos2::ZERO; 97];
    let w = r.width();
    let hgt = r.height();
    match m {
        Motif::Sine => {
            // Three sines in phase steps over a fine horizon: a modulation's sweep.
            b.hline(
                r.left(),
                r.right(),
                c0.y,
                fine * 0.7,
                with_alpha(print, 0.35),
            );
            for (j, (amp, a)) in [(0.40f32, 0.85f32), (0.28, 0.55), (0.16, 0.35)]
                .iter()
                .enumerate()
            {
                let n = pts.len() - 1;
                for (i, p) in pts.iter_mut().enumerate() {
                    let x = i as f32 / n as f32;
                    let ph = TAU * (1.5 * x + j as f32 * 0.12);
                    *p = pos2(r.left() + w * x, c0.y - ph.sin() * hgt * amp);
                }
                b.polyline(&pts, fine * (1.6 - 0.35 * j as f32), with_alpha(accent, *a));
            }
        }
        Motif::Echoes => {
            // A struck note and its repeats, each lower and softer: a delay's taps.
            let base = r.bottom() - hgt * 0.12;
            b.hline(r.left(), r.right(), base, fine, with_alpha(print, 0.45));
            for i in 0..7 {
                let t = i as f32 / 6.0;
                let x = r.left() + w * (0.08 + 0.84 * t);
                let hh = hgt * 0.78 * (1.0 - t * 0.82);
                let a = 0.95 - 0.11 * i as f32;
                b.segment(
                    pos2(x, base),
                    pos2(x, base - hh),
                    2.2 * u,
                    with_alpha(accent, a),
                    true,
                );
                b.circle(
                    pos2(x, base - hh),
                    2.4 * u,
                    Fill::Solid(with_alpha(accent, a)),
                );
                if i > 0 {
                    b.arc(
                        pos2(x - w * 0.07, base - hh * 0.5),
                        w * 0.07,
                        PI,
                        TAU,
                        fine * 0.7,
                        with_alpha(print, 0.25),
                        false,
                    );
                }
            }
        }
        Motif::Rings => {
            // A drop in still water: rings spreading, the far ones fainter.
            let o = pos2(c0.x, c0.y + hgt * 0.08);
            for i in 0..7 {
                let rr = (6.0 + 9.0 * i as f32) * u;
                if rr > w * 0.6 {
                    break;
                }
                let a = 0.9 - 0.12 * i as f32;
                b.arc(
                    o,
                    rr,
                    PI * 1.05,
                    PI * 1.95,
                    fine * 1.2,
                    with_alpha(accent, a),
                    false,
                );
                b.arc(
                    o,
                    rr,
                    PI * 0.15,
                    PI * 0.85,
                    fine,
                    with_alpha(print, a * 0.5),
                    false,
                );
            }
            b.circle(o, 2.6 * u, Fill::Solid(accent));
        }
        Motif::Needle => {
            // A meter's arc with its scale and a needle leaning into the red.
            let o = pos2(c0.x, r.bottom() - hgt * 0.05);
            let rad = (w * 0.46).min(hgt * 0.9);
            b.arc(
                o,
                rad,
                PI * 1.15,
                PI * 1.85,
                fine * 1.4,
                with_alpha(print, 0.7),
                false,
            );
            b.arc(
                o,
                rad,
                PI * 1.68,
                PI * 1.85,
                3.0 * u,
                with_alpha(accent, 0.9),
                false,
            );
            for i in 0..=10 {
                let a = PI * (1.15 + 0.07 * i as f32);
                let d = vec2(a.cos(), a.sin());
                let l = if i % 5 == 0 { 9.0 } else { 5.0 } * u;
                b.segment(
                    o + d * rad,
                    o + d * (rad - l),
                    fine,
                    with_alpha(print, 0.7),
                    false,
                );
            }
            let a = PI * 1.62;
            let d = vec2(a.cos(), a.sin());
            b.segment(o, o + d * (rad - 3.0 * u), 1.4 * u, accent, true);
            b.circle(o, 4.0 * u, Fill::Solid(with_alpha(print, 0.8)));
        }
        Motif::Curve => {
            // A tone curve over a grid: a lift in the lows, a bell, a gentle roll-off.
            for i in 1..4 {
                let y = r.top() + hgt * i as f32 / 4.0;
                b.hline(r.left(), r.right(), y, fine * 0.6, with_alpha(print, 0.22));
            }
            for i in 1..6 {
                let x = r.left() + w * i as f32 / 6.0;
                b.vline(x, r.top(), r.bottom(), fine * 0.6, with_alpha(print, 0.16));
            }
            let n = pts.len() - 1;
            for (i, p) in pts.iter_mut().enumerate() {
                let x = i as f32 / n as f32;
                let bell = (-((x - 0.55) / 0.12).powi(2)).exp();
                let low = 0.25 / (1.0 + (x / 0.18).powi(4));
                let roll = -0.3 / (1.0 + ((1.0 - x) / 0.12).powi(4));
                let y = low + 0.5 * bell + roll;
                *p = pos2(r.left() + w * x, c0.y - y * hgt * 0.45);
            }
            b.polyline(&pts, fine * 1.8, accent);
        }
        Motif::Steps => {
            // A row of steps of different heights: a pattern, a rhythm.
            let n = 8;
            let gap = 2.0 * u;
            let bw = (w - gap * (n - 1) as f32) / n as f32;
            let heights = [0.9f32, 0.35, 0.6, 0.2, 0.75, 0.45, 1.0, 0.3];
            for (i, f) in heights.iter().enumerate() {
                let x = r.left() + i as f32 * (bw + gap);
                let top = r.bottom() - hgt * 0.9 * f;
                let cell = Rect::from_min_max(pos2(x, top), pos2(x + bw, r.bottom()));
                b.rect(cell, Fill::Solid(with_alpha(accent, 0.25 + 0.6 * f)));
                b.hline(x, x + bw, top, fine * 1.4, with_alpha(print, 0.8));
            }
        }
        Motif::Spiral => {
            // A spiral turning out from the middle, and a horn's mouth on its rim.
            let n = pts.len() - 1;
            let rad = w.min(hgt) * 0.48;
            for (i, p) in pts.iter_mut().enumerate() {
                let t = i as f32 / n as f32;
                let a = TAU * 2.6 * t;
                *p = c0 + vec2(a.cos(), a.sin()) * rad * t;
            }
            b.polyline(&pts, fine * 1.5, accent);
            b.circle_stroke(c0, rad, fine, with_alpha(print, 0.4));
            b.circle(c0, 2.6 * u, Fill::Solid(print));
        }
        _ => {}
    }
}

/// The border Tabula's motifs share: a fine inset frame and a tick at each corner.
pub fn border(b: &mut Brush, u: f32, r: Rect, print: Color32, accent: Color32) {
    let fine = 0.7 * u;
    let inset = r.shrink(8.0 * u);
    b.rrect_stroke(inset, 5.0 * u, fine, with_alpha(print, 0.32));
    for (cx, cy, sx, sy) in [
        (inset.left(), inset.top(), 1.0f32, 1.0f32),
        (inset.right(), inset.top(), -1.0, 1.0),
        (inset.left(), inset.bottom(), 1.0, -1.0),
        (inset.right(), inset.bottom(), -1.0, -1.0),
    ] {
        let o = pos2(cx + sx * 4.0 * u, cy + sy * 4.0 * u);
        b.segment(
            o,
            o + vec2(sx * 9.0 * u, 0.0),
            fine * 1.4,
            with_alpha(accent, 0.7),
            true,
        );
        b.segment(
            o,
            o + vec2(0.0, sy * 9.0 * u),
            fine * 1.4,
            with_alpha(accent, 0.7),
            true,
        );
    }
}
