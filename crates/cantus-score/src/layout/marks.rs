//! The marks over a staff's notes: ties (Gould: away from the stem, a chord's upper half
//! up and lower half down, between the heads, ends thin and middle thick — Bravura's
//! 0.1 and 0.22 spaces), tuplet numbers and brackets on the stem side, octave lines,
//! dynamics under the staff, and tempo marks over the system — each placed against the
//! skyline so it clears what is already drawn.

use super::chord::y_of;
use super::columns::BarCols;
use super::draw::{dynamic_glyph, SystemPlan};
use super::notes::{EvKey, EventPlace, HeadPlace};
use super::sky::Skyline;
use crate::display::{Anchor, Item};
use crate::glyphs::{defaults, Glyph};
use crate::model::{Kind, Score, Staff, Stem};
use crate::text::{Face, TextMeasure};
use std::collections::HashMap;

/// A tie's curve from `x0` to `x1` at `y`, bowing up or down.
fn tie(x0: f32, x1: f32, y: f32, up: bool, owner: Option<u64>) -> Item {
    let len = (x1 - x0).max(0.5);
    let h = (0.12 * len + 0.25).clamp(0.35, 1.0);
    let s = if up { -1.0 } else { 1.0 };
    let c = h * 4.0 / 3.0;
    Item::Curve {
        p: [
            [x0, y],
            [x0 + len * 0.22, y + s * c],
            [x1 - len * 0.22, y + s * c],
            [x1, y],
        ],
        end: defaults::TIE_ENDPOINT_THICKNESS,
        mid: defaults::TIE_MIDPOINT_THICKNESS,
        up,
        owner,
    }
}

/// Which way the tie of a head bows: a lone head's away from its stem; in a chord the
/// upper half up, the lower half down, the middle one away from the stem.
fn bows_up(h: &HeadPlace) -> bool {
    if h.count <= 1 {
        return !h.up_stem;
    }
    let half = h.count / 2;
    if h.rank < half {
        true
    } else if h.rank >= h.count - half {
        false
    } else {
        !h.up_stem
    }
}

/// Ties: from each tied head to its continuation in the system, or to the system's end
/// (and, for a continuation whose tie began in the system before, in from the preamble).
pub fn ties(
    score: &Score,
    staff: &Staff,
    plan: &SystemPlan,
    heads: &HashMap<(usize, usize, usize, i32), HeadPlace>,
    out: &mut Vec<Item>,
    sky: &mut Skyline,
) {
    let _ = score;
    for b in plan.first_bar..=plan.last_bar {
        for (vi, v) in staff.bars[b].voices.iter().enumerate() {
            for (ei, e) in v.events.iter().enumerate() {
                let Kind::Chord(hs) = &e.kind else { continue };
                let next: (usize, usize, usize) = if ei + 1 < v.events.len() {
                    (b, vi, ei + 1)
                } else {
                    (b + 1, vi, 0)
                };
                for h in hs {
                    let Some(src) = heads.get(&(b, vi, ei, h.pos)) else {
                        continue;
                    };
                    let up = bows_up(src);
                    let dy = if up { -0.3 } else { 0.3 };
                    if h.tied_from && b == plan.first_bar && ei == 0 {
                        // Its tie began in the system before: in from the preamble.
                        let x0 = (src.x0 - 2.5).max(plan.preamble_end + 0.3);
                        let it = tie(x0, src.x0 - 0.15, src.y + dy, up, Some(h.id));
                        sky.add(it.bounds());
                        out.push(it);
                    }
                    if !h.tie {
                        continue;
                    }
                    let start = src.after_dots.unwrap_or(src.x1) + 0.12;
                    let it = match heads.get(&(next.0, next.1, next.2, h.pos)) {
                        Some(dst) if next.0 <= plan.last_bar => {
                            tie(start, dst.x0 - 0.12, src.y + dy, up, Some(h.id))
                        }
                        _ if next.0 > plan.last_bar => {
                            tie(start, plan.x1 - 0.3, src.y + dy, up, Some(h.id))
                        }
                        _ => continue,
                    };
                    sky.add(it.bounds());
                    out.push(it);
                }
            }
        }
    }
}

/// Tuplets: the number over (or under) a beamed group's beam; a bracket with the number
/// in its gap when the group is not one beam.
pub fn tuplets(
    staff: &Staff,
    plan: &SystemPlan,
    places: &HashMap<EvKey, EventPlace>,
    out: &mut Vec<Item>,
    sky: &mut Skyline,
) {
    for b in plan.first_bar..=plan.last_bar {
        for (vi, v) in staff.bars[b].voices.iter().enumerate() {
            for t in &v.tuplets {
                let ps: Vec<EventPlace> = (t.first..=t.last)
                    .filter_map(|k| places.get(&(b, vi, k)).copied())
                    .collect();
                if ps.is_empty() {
                    continue;
                }
                let digits: Vec<Glyph> = t
                    .actual
                    .to_string()
                    .chars()
                    .map(|c| Glyph::tuplet_digit(c.to_digit(10).unwrap_or(3)))
                    .collect();
                let w: f32 = digits.iter().map(|g| g.advance()).sum();
                let first_note = ps.iter().find(|p| !p.rest);
                let above = first_note.is_none_or(|p| p.stem == Stem::Up);
                let x0 = ps[0].stem_x.filter(|_| !t.bracket).unwrap_or(ps[0].x0);
                let x1 = ps[ps.len() - 1]
                    .stem_x
                    .filter(|_| !t.bracket)
                    .unwrap_or(ps[ps.len() - 1].x1);
                let cx = (x0 + x1) / 2.0;
                let baseline = if !t.bracket {
                    let tips: Vec<f32> = ps.iter().filter_map(|p| p.tip).collect();
                    if above {
                        tips.iter()
                            .copied()
                            .fold(f32::MAX, f32::min)
                            .min(sky.top_in(cx - w / 2.0, cx + w / 2.0) + 0.1)
                            - 0.35
                    } else {
                        tips.iter()
                            .copied()
                            .fold(f32::MIN, f32::max)
                            .max(sky.bottom_in(cx - w / 2.0, cx + w / 2.0) - 0.1)
                            + 0.35
                            + 1.5
                    }
                } else {
                    let (a, z) = (x0 - 0.1, x1 + 0.1);
                    let th = defaults::TUPLET_BRACKET_THICKNESS;
                    let line_y = if above {
                        (sky.top_in(a, z) - 0.9).min(-0.9)
                    } else {
                        (sky.bottom_in(a, z) + 0.9).max(4.9)
                    };
                    let hook = if above { 0.6 } else { -0.6 };
                    let gap = (w / 2.0 + 0.3, cx);
                    for (p, q) in [(a, gap.1 - gap.0), (gap.1 + gap.0, z)] {
                        out.push(Item::Line {
                            x0: p,
                            y0: line_y,
                            x1: q,
                            y1: line_y,
                            w: th,
                            owner: None,
                        });
                    }
                    for x in [a, z] {
                        out.push(Item::Line {
                            x0: x,
                            y0: line_y,
                            x1: x,
                            y1: line_y + hook,
                            w: th,
                            owner: None,
                        });
                    }
                    line_y + 0.75
                };
                let mut x = cx - w / 2.0;
                for g in digits {
                    let it = Item::Glyph {
                        glyph: g,
                        x,
                        y: baseline,
                        scale: 1.0,
                        owner: None,
                    };
                    sky.add(it.bounds());
                    out.push(it);
                    x += g.advance();
                }
            }
        }
    }
}

/// Octave lines: "8va" (or "8vb") at the first note, a dashed line to the last, a hook
/// at its end.
pub fn ottavas(
    staff: &Staff,
    plan: &SystemPlan,
    places: &HashMap<EvKey, EventPlace>,
    out: &mut Vec<Item>,
    sky: &mut Skyline,
) {
    for o in &staff.ottavas {
        if o.to.0 < plan.first_bar || o.from.0 > plan.last_bar {
            continue;
        }
        let starts_here = o.from.0 >= plan.first_bar;
        let ends_here = o.to.0 <= plan.last_bar;
        let x0 = if starts_here {
            places.get(&o.from).map_or(plan.preamble_end, |p| p.x0)
        } else {
            plan.preamble_end
        };
        let x1 = if ends_here {
            places.get(&o.to).map_or(plan.x1, |p| p.x1)
        } else {
            plan.x1 - 0.5
        };
        let up = o.shift > 0;
        let g = if up {
            Glyph::OttavaAlta
        } else {
            Glyph::OttavaBassaVb
        };
        let gw = g.width();
        let baseline = if up {
            (sky.top_in(x0, x1) - 0.6).min(-1.5)
        } else {
            (sky.bottom_in(x0, x1) + 2.2).max(6.6)
        };
        let mid = baseline - 0.9;
        out.push(Item::Glyph {
            glyph: g,
            x: x0,
            y: baseline,
            scale: 1.0,
            owner: None,
        });
        let th = defaults::OCTAVE_LINE_THICKNESS;
        if x1 > x0 + gw + 0.6 {
            out.push(Item::Dashed {
                x0: x0 + gw + 0.3,
                y0: mid,
                x1,
                w: th,
                dash: 0.6,
                gap: 0.45,
            });
        }
        if ends_here {
            let hook = if up { 0.9 } else { -0.9 };
            out.push(Item::Line {
                x0: x1,
                y0: mid,
                x1,
                y1: mid + hook,
                w: th,
                owner: None,
            });
        }
        sky.add([x0, baseline - 2.0, x1, baseline + 0.2]);
    }
}

/// Dynamics under the staff, clear of the notes above them.
pub fn dynamics(list: &[(f32, crate::model::Dynamic)], out: &mut Vec<Item>, sky: &mut Skyline) {
    for &(x, d) in list {
        let g = dynamic_glyph(d);
        let b = g.bounds();
        let x = x - 0.2;
        let baseline = (sky.bottom_in(x + b[0] - 0.3, x + b[2] + 0.3) + 0.5 - b[1]).max(6.4);
        let it = Item::Glyph {
            glyph: g,
            x,
            y: baseline,
            scale: 1.0,
            owner: None,
        };
        sky.add(it.bounds());
        out.push(it);
    }
}

/// Tempo marks over the top staff: *accel.* or *rit.* when a ramp starts, then ♩ = 120
/// (a dotted quarter in compound metres), bold, at the bar's first note.
#[allow(clippy::needless_range_loop)] // `b` names the bar in the score and the plan both.
pub fn tempo(
    score: &Score,
    cols: &[BarCols],
    plan: &SystemPlan,
    text: &TextMeasure,
    out: &mut Vec<Item>,
    sky: &mut Skyline,
) {
    const SIZE: f32 = 2.0;
    const NOTE: f32 = 0.62;
    for b in plan.first_bar..=plan.last_bar {
        let Some(t) = score.bars[b].tempo else {
            continue;
        };
        let local = b - plan.first_bar;
        let x = if b == plan.first_bar {
            plan.x0 + 0.75
        } else {
            plan.col_x[local]
                .first()
                .copied()
                .unwrap_or(plan.bar_x[local] + 1.0)
                + cols[b].columns.first().map_or(0.0, |c| c.left)
        };
        let words = t.ramp.map(|up| if up { "accel." } else { "rit." });
        let pm = if t.per_minute.fract() == 0.0 {
            format!("{}", t.per_minute as i64)
        } else {
            format!("{:.1}", t.per_minute)
        };
        let eq = format!(" = {pm}");
        let ww = words.map_or(0.0, |w| text.width(w, SIZE, Face::Italic) + 0.8);
        let note_w =
            Glyph::MetNoteQuarterUp.advance() * NOTE + if t.unit.dots > 0 { 0.5 } else { 0.0 };
        let total = ww + note_w + text.width(&eq, SIZE, Face::Bold);
        let baseline = (sky.top_in(x, x + total) - 1.0).min(-2.4);
        let mut at = x;
        if let Some(w) = words {
            out.push(Item::Text {
                text: w.to_string(),
                x: at,
                y: baseline,
                size: SIZE,
                face: Face::Italic,
                anchor: Anchor::Left,
            });
            at += ww;
        }
        out.push(Item::Glyph {
            glyph: Glyph::MetNoteQuarterUp,
            x: at,
            y: baseline,
            scale: NOTE,
            owner: None,
        });
        at += Glyph::MetNoteQuarterUp.advance() * NOTE;
        if t.unit.dots > 0 {
            out.push(Item::Glyph {
                glyph: Glyph::MetAugmentationDot,
                x: at + 0.15,
                y: baseline,
                scale: NOTE,
                owner: None,
            });
            at += 0.5;
        }
        out.push(Item::Text {
            text: eq,
            x: at,
            y: baseline,
            size: SIZE,
            face: Face::Bold,
            anchor: Anchor::Left,
        });
        sky.add([x, baseline - SIZE, x + total, baseline + 0.4]);
        let _ = y_of(0);
    }
}
