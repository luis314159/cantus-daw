//! Rota's motif on Tabula's board (ADR 0134's faces, ADR 0136): the wheel — a horn rotor seen
//! from above, its bell sweeping a ring of speed lines, the sound thrown out in arcs that
//! crowd on one side and spread on the other (the Doppler shift a rotating horn makes).
//! Original to Cantus; the same each frame. // fx3:rota

use crate::kit::color::with_alpha;
use crate::kit::{Brush, Fill};
use egui::{pos2, vec2, Color32, Pos2, Rect};
use std::f32::consts::{PI, TAU};

/// Paint the wheel over the art area `r` (`u` the enclosure's scale).
pub fn wheel(b: &mut Brush, u: f32, r: Rect, print: Color32, accent: Color32) {
    let fine = 0.8 * u;
    let rad = (r.height() * 0.30).min(r.width() * 0.22);
    let c = pos2(r.center().x - r.width() * 0.12, r.center().y);
    // The sound's wavefronts: arcs to the right, close together ahead of the bell, wider
    // behind it.
    for i in 0..6 {
        let t = i as f32 / 5.0;
        let rr = rad * (1.25 + 1.15 * t);
        let off = vec2(rad * 0.35 * t, 0.0);
        b.arc(
            c + off,
            rr,
            -PI * 0.28,
            PI * 0.28,
            fine,
            with_alpha(accent, 0.75 - 0.1 * i as f32),
            false,
        );
    }
    // The rotor's sweep: a ring of dashes.
    let n = 24;
    for i in 0..n {
        if i % 2 == 0 {
            let a0 = TAU * i as f32 / n as f32;
            let a1 = TAU * (i + 1) as f32 / n as f32;
            b.arc(c, rad * 1.02, a0, a1, fine, with_alpha(print, 0.5), false);
        }
    }
    // The arm with its bell (and the dummy's weight), the hub.
    let a = -PI * 0.12;
    let dir = vec2(a.cos(), a.sin());
    let nrm = vec2(-dir.y, dir.x);
    b.segment(c - dir * rad * 0.8, c + dir * rad * 0.5, fine * 1.6, print, true);
    let tip = c + dir * rad * 0.95;
    let bell: [Pos2; 4] = [
        c + dir * rad * 0.45 + nrm * 1.2 * u,
        tip + nrm * rad * 0.28,
        tip - nrm * rad * 0.28,
        c + dir * rad * 0.45 - nrm * 1.2 * u,
    ];
    b.polygon(&bell, with_alpha(accent, 0.9));
    b.circle(c - dir * rad * 0.8, 2.6 * u, Fill::Solid(with_alpha(print, 0.7)));
    b.circle(c, 3.0 * u, Fill::Solid(print));
    b.circle_stroke(c, 5.0 * u, fine, with_alpha(print, 0.6));
}
