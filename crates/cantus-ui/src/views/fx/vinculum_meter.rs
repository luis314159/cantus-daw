//! Vinculum's needle meters (design sheet v1): a moving-coil meter behind the faceplate's
//! window, its card lit from behind by two lamps in the colourway's light, the scale
//! printed on the card (the minor and major marks, the numbers, the red zone), the black
//! needle and its shadow on the card, the moulded hood over the pivot, a peak marker on the
//! arc, the glass. The display hands it where the needle stands; nothing here moves by
//! itself. Original to Cantus.

use super::super::canvas;
use super::art::{self, c};
use crate::kit::color::{mix, with_alpha};
use crate::kit::skin::racks as pal;
use crate::kit::text::{Role, Text};
use crate::kit::{Brush, Fill, Style};
use cantus_dsp::math::{cos, sin};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The needle's swing either side of upright, radians.
const SWING: f32 = 0.84;

/// One mark of a scale: where it is (0 … 1, left to right), its number if it carries one.
#[derive(Copy, Clone, Debug)]
pub struct Mark {
    pub at: f32,
    pub label: Option<&'static str>,
}

/// A meter's scale: its marks, where its red zone runs (positions), its title's key and
/// its unit.
pub struct Scale {
    pub marks: &'static [Mark],
    pub red: (f32, f32),
    pub title: &'static str,
    pub unit: &'static str,
}

/// Where the dial's arc is in a card.
#[derive(Copy, Clone, Debug)]
struct Dial {
    pivot: Pos2,
    rad: f32,
}

impl Dial {
    /// The dial in `card`: as large as the card allows with the numbers at the scale's
    /// ends still on it.
    fn of(card: Rect, k: f32) -> Dial {
        let rad = (card.height() * 0.92).min((card.width() * 0.5 - 30.0 * k) / sin(SWING));
        Dial {
            pivot: pos2(card.center().x, card.top() + 0.24 * card.height() + rad),
            rad,
        }
    }

    fn dir(&self, p: f32) -> egui::Vec2 {
        let a = (p.clamp(-0.03, 1.03) * 2.0 - 1.0) * SWING;
        vec2(sin(a), -cos(a))
    }

    fn at(&self, p: f32, out: f32) -> Pos2 {
        self.pivot + self.dir(p) * (self.rad + out)
    }
}

/// The meter in the window `r`: its card lit `lamp`, `scale` printed on it, the needle at
/// `needle` and the peak marker at `peak` (positions; `None` hides the marker).
#[allow(clippy::too_many_arguments)]
pub fn meter(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    lamp: Color32,
    scale: &Scale,
    needle: f32,
    peak: Option<f32>,
    lit: f32,
) {
    let k = s.k;
    let card = r;
    // The card: paper lit from behind, warmer where the lamps are, its corners in shadow.
    let paper = mix(c(pal::CARD), lamp, 0.22 + 0.10 * lit);
    let dim = mix(paper, c(pal::SHADOW), 0.42 - 0.22 * lit);
    b.rect(
        card,
        Fill::V(dim, mix(paper, c(pal::SHADOW), 0.25 - 0.15 * lit)),
    );
    for fx in [0.28f32, 0.72] {
        b.radial(
            card,
            pos2(
                card.left() + card.width() * fx,
                card.bottom() - card.height() * 0.12,
            ),
            vec2(card.width() * 0.46, card.height() * 0.78),
            &[
                (0.0, with_alpha(mix(paper, lamp, 0.25), 0.85 * lit)),
                (0.55, with_alpha(paper, 0.45 * lit)),
                (1.0, with_alpha(paper, 0.0)),
            ],
            12,
        );
    }
    let d = Dial::of(card, k);
    let ink = c(pal::CARD_INK);
    let red = c(pal::CARD_RED);
    // The scale's arc, its red zone a band over it, the finer second arc inside it.
    let arc = |p: f32| {
        let a = (p * 2.0 - 1.0) * SWING;
        a - std::f32::consts::FRAC_PI_2
    };
    b.arc(d.pivot, d.rad, arc(0.0), arc(1.0), 1.0 * k, ink, false);
    b.arc(
        d.pivot,
        d.rad + 2.6 * k,
        arc(scale.red.0),
        arc(scale.red.1),
        4.0 * k,
        with_alpha(red, 0.92),
        false,
    );
    b.arc(
        d.pivot,
        d.rad - 5.0 * k,
        arc(0.0),
        arc(1.0),
        0.5 * k,
        with_alpha(ink, 0.6),
        false,
    );
    for m in scale.marks {
        let in_red = m.at >= scale.red.0 - 1e-3 && m.at <= scale.red.1 + 1e-3;
        let col = if in_red { red } else { ink };
        if m.label.is_some() {
            b.segment(
                d.at(m.at, -5.0 * k),
                d.at(m.at, 8.0 * k),
                1.2 * k,
                col,
                false,
            );
        } else {
            b.segment(
                d.at(m.at, -2.0 * k),
                d.at(m.at, 4.5 * k),
                0.8 * k,
                col,
                false,
            );
        }
        if let Some(word) = m.label {
            canvas::put(
                b,
                d.at(m.at, 17.0 * k),
                Align2::CENTER_CENTER,
                &Text::new(word, Role::Micro, col).caps(false),
                k,
                None,
            );
        }
    }
    // The peak marker: a small red pointer just outside the arc.
    if let Some(pk) = peak {
        let tip = d.at(pk, 1.5 * k);
        let dir = d.dir(pk);
        let side = vec2(-dir.y, dir.x) * 3.0 * k;
        b.polygon(
            &[tip, tip + dir * 7.0 * k + side, tip + dir * 7.0 * k - side],
            red,
        );
    }
    // The needle: from under the hood to past the arc; its shadow cast on the card.
    let hood_top = card.bottom() - card.height() * 0.16;
    let dir = d.dir(needle);
    let start = {
        // Where the needle comes out from under the hood.
        let t = (d.pivot.y - hood_top) / (-dir.y).max(0.2);
        d.pivot + dir * t
    };
    let end = d.pivot + dir * (d.rad + 9.0 * k);
    let sh = vec2(2.2, 3.0) * k;
    b.segment(
        start + sh,
        end + sh,
        1.6 * k,
        with_alpha(c(pal::SHADOW), 0.22),
        false,
    );
    b.segment(
        start,
        d.pivot + dir * (d.rad * 0.78),
        1.6 * k,
        c(pal::NEEDLE),
        false,
    );
    b.segment(
        d.pivot + dir * (d.rad * 0.78),
        end,
        0.9 * k,
        c(pal::NEEDLE),
        false,
    );
    // The hood over the pivot: black moulding with a lit lip, its curved top.
    let n = 24;
    let mut top = [Pos2::ZERO; 26];
    for (i, q) in top.iter_mut().enumerate().take(n + 1) {
        let t = i as f32 / n as f32;
        let x = card.left() + card.width() * t;
        let bulge = 1.0 - (2.0 * t - 1.0) * (2.0 * t - 1.0);
        *q = pos2(x, hood_top + card.height() * 0.05 * (1.0 - bulge));
    }
    for i in 0..n {
        let a = top[i];
        let z = top[i + 1];
        b.polygon(
            &[a, z, pos2(z.x, card.bottom()), pos2(a.x, card.bottom())],
            mix(c(pal::HOUSING), c(pal::HOUSING_LIGHT), 0.15),
        );
    }
    b.polyline(&top[..=n], 1.0 * k, with_alpha(c(pal::HOUSING_LIGHT), 0.9));
    // The title and the unit printed on the hood in the card's pale ink, clear of the
    // needle's sweep.
    let hy = 0.5 * (hood_top + card.height() * 0.025 + card.bottom());
    let pale = with_alpha(c(pal::CARD), 0.88);
    let title = Text::key(scale.title, Role::Micro, pale);
    let unit = Text::new(scale.unit, Role::Micro, with_alpha(c(pal::CARD), 0.6)).caps(false);
    let tw = crate::kit::text::measure(b.ctx(), &title, k).x;
    let uw = crate::kit::text::measure(b.ctx(), &unit, k).x;
    let x0 = card.center().x - 0.5 * (tw + 6.0 * k + uw);
    canvas::put(b, pos2(x0, hy), Align2::LEFT_CENTER, &title, k, None);
    canvas::put(
        b,
        pos2(x0 + tw + 6.0 * k, hy),
        Align2::LEFT_CENTER,
        &unit,
        k,
        None,
    );
    art::glass(b, card, k);
}

/// The gain reduction's scale: 0 dB at the right, 20 dB at the left, the first decibels
/// spread (what a mix bus works in), on a power law.
pub fn gr_pos(gr_db: f32) -> f32 {
    1.0 - cantus_dsp::math::powf((gr_db / 20.0).clamp(0.0, 1.0), 0.6)
}

/// The output's scale, dBFS: −40 at the left, 0 at the right, on the square root of the
/// amplitude (a meter's movement).
pub fn out_pos(db: f32) -> f32 {
    let a = cantus_dsp::math::db_to_gain(db.clamp(-60.0, 6.0));
    ((a.sqrt() - 0.1) / 0.9).clamp(-0.02, 1.04)
}
