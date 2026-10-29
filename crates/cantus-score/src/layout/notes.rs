//! One staff of one system: lines, preamble, every event, and the marks over them.

use super::beam::{self, Stemmed};
use super::chord::{self, y_of};
use super::columns::{BarCols, StaffRef};
use super::draw::{draw_time, preamble, StaffDrawn, SystemPlan};
use super::marks;
use super::sky::Skyline;
use crate::display::Item;
use crate::glyphs::{defaults, Glyph};
use crate::model::{Kind, Score, Stem};
use crate::text::TextMeasure;
use std::collections::HashMap;

/// Where a notehead was drawn: what a tie needs.
#[derive(Copy, Clone, Debug)]
pub struct HeadPlace {
    pub x0: f32,
    pub x1: f32,
    pub y: f32,
    pub up_stem: bool,
    pub rank: usize,
    pub count: usize,
    pub after_dots: Option<f32>,
}

/// Where an event was drawn: its ink's x range, its stem's x and tip, its stem.
#[derive(Copy, Clone, Debug)]
pub struct EventPlace {
    pub x0: f32,
    pub x1: f32,
    pub stem_x: Option<f32>,
    pub tip: Option<f32>,
    pub stem: Stem,
    pub rest: bool,
}

/// The key of an event: (bar, voice, event).
pub type EvKey = (usize, usize, usize);

/// Draw staff `si` of the system `plan`.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_range_loop)] // `b` names the bar in the score, the plan and the staff.
pub fn draw_staff(
    score: &Score,
    staves: &[StaffRef],
    si: usize,
    cols: &[BarCols],
    plan: &SystemPlan,
    text: &TextMeasure,
    top: bool,
) -> StaffDrawn {
    let r = staves[si];
    let part = &score.parts[r.part];
    let staff = &part.staves[r.staff];
    let mut items: Vec<Item> = Vec::new();
    let line_end = plan.x1;
    for l in 0..5 {
        items.push(Item::Line {
            x0: plan.x0,
            y0: l as f32,
            x1: line_end,
            y1: l as f32,
            w: defaults::STAFF_LINE_THICKNESS,
            owner: None,
        });
    }
    let first_sig = (plan.first || score.bars[plan.first_bar].show_sig)
        .then_some(score.bars[plan.first_bar].sig);
    preamble(staff.clef, part.key, first_sig, plan.x0, Some(&mut items));
    if let Some(sig) = plan.courtesy {
        draw_time(sig, plan.bar_end[plan.bar_end.len() - 1] + 0.75, &mut items);
    }
    let mut heads: HashMap<(usize, usize, usize, i32), HeadPlace> = HashMap::new();
    let mut places: HashMap<EvKey, EventPlace> = HashMap::new();
    let mut dynamics: Vec<(f32, crate::model::Dynamic)> = Vec::new();
    for b in plan.first_bar..=plan.last_bar {
        let local = b - plan.first_bar;
        let bar = &score.bars[b];
        if b != plan.first_bar && bar.show_sig {
            draw_time(bar.sig, plan.bar_x[local] + 0.75, &mut items);
        }
        // Whole-bar rests, centred between where the bar's content starts and its end.
        for &(st, vi) in &cols[b].whole_rests {
            if st != si {
                continue;
            }
            let two = staff.bars[b].voices.len() > 1;
            let g = Glyph::RestWhole;
            let start = if b == plan.first_bar {
                plan.preamble_end
            } else if bar.show_sig {
                plan.bar_x[local] + 0.75 + super::draw::time_width(bar.sig)
            } else {
                plan.bar_x[local]
            };
            let x = (start + plan.bar_end[local]) / 2.0 - g.width() / 2.0;
            items.push(glyph(g, x, chord::rest_y(g, vi, two), None));
            places.insert(
                (b, vi, 0),
                EventPlace {
                    x0: x,
                    x1: x + g.width(),
                    stem_x: None,
                    tip: None,
                    stem: Stem::Up,
                    rest: true,
                },
            );
        }
        let mut groups: HashMap<(usize, usize), Vec<(usize, Stemmed)>> = HashMap::new();
        for (ci, col) in cols[b].columns.iter().enumerate() {
            let x = plan.col_x[local][ci];
            for entry in col.entries.iter().filter(|e| e.staff == si) {
                let e = &staff.bars[b].voices[entry.voice].events[entry.event];
                let ex = x + entry.dx;
                let sh = &entry.shape;
                let key = (b, entry.voice, entry.event);
                if let Some((g, y)) = sh.rest {
                    items.push(glyph(g, ex, y, None));
                    for d in &sh.dots {
                        items.push(glyph(Glyph::AugmentationDot, ex + d.0, d.1, None));
                    }
                    places.insert(
                        key,
                        EventPlace {
                            x0: ex,
                            x1: ex + g.width(),
                            stem_x: None,
                            tip: None,
                            stem: e.stem,
                            rest: true,
                        },
                    );
                    continue;
                }
                let Kind::Chord(hs) = &e.kind else { continue };
                let owner = hs.first().map(|h| h.id);
                for h in &sh.heads {
                    items.push(glyph(h.glyph, ex + h.dx, y_of(h.pos), Some(h.id)));
                }
                ledger_lines(sh, ex, &mut items);
                for a in &sh.accidentals {
                    let y = y_of(a.pos);
                    if a.courtesy {
                        let l = Glyph::AccidentalParensLeft;
                        items.push(glyph(l, ex + a.dx, y, Some(a.id)));
                        let ax = ex + a.dx + l.width();
                        items.push(glyph(a.glyph, ax, y, Some(a.id)));
                        items.push(glyph(
                            Glyph::AccidentalParensRight,
                            ax + a.glyph.width(),
                            y,
                            Some(a.id),
                        ));
                    } else {
                        items.push(glyph(a.glyph, ex + a.dx, y, Some(a.id)));
                    }
                }
                for d in &sh.dots {
                    items.push(glyph(Glyph::AugmentationDot, ex + d.0, d.1, owner));
                }
                let lo = hs.iter().map(|h| h.pos).min().unwrap_or(0);
                let hi = hs.iter().map(|h| h.pos).max().unwrap_or(0);
                let mut place = EventPlace {
                    x0: ex + sh.heads.iter().map(|h| h.dx).fold(0.0, f32::min),
                    x1: ex + sh.right,
                    stem_x: sh.stem_x.map(|s| ex + s),
                    tip: None,
                    stem: e.stem,
                    rest: false,
                };
                if let Some(sx) = sh.stem_x {
                    let s = Stemmed {
                        x: ex + sx,
                        lo,
                        hi,
                        stem: e.stem,
                        dur: e.dur,
                        beams: e.beams.clone(),
                        head: chord::notehead(e.dur),
                        owner,
                    };
                    match e.group {
                        Some(g) => groups
                            .entry((entry.voice, g))
                            .or_default()
                            .push((entry.event, s)),
                        None => {
                            place.tip = Some(beam::lone(&s, &mut items));
                            if let Some(f) = chord::flag(e.dur, e.stem) {
                                if e.stem == Stem::Up {
                                    place.x1 = place.x1.max(ex + sx + f.width());
                                }
                            }
                        }
                    }
                }
                places.insert(key, place);
                let dots_right = sh
                    .dots
                    .iter()
                    .map(|d| ex + d.0 + Glyph::AugmentationDot.width())
                    .reduce(f32::max);
                let mut by_pos: Vec<i32> = hs.iter().map(|h| h.pos).collect();
                by_pos.sort_unstable_by(|a, b| b.cmp(a));
                for h in &sh.heads {
                    let rank = by_pos.iter().position(|p| *p == h.pos).unwrap_or(0);
                    heads.insert(
                        (b, entry.voice, entry.event, h.pos),
                        HeadPlace {
                            x0: ex + h.dx,
                            x1: ex + h.dx + h.glyph.width(),
                            y: y_of(h.pos),
                            up_stem: e.stem == Stem::Up,
                            rank,
                            count: by_pos.len(),
                            after_dots: dots_right,
                        },
                    );
                }
                if let Some(d) = e.dynamic {
                    dynamics.push((ex, d));
                }
            }
        }
        let mut keys: Vec<(usize, usize)> = groups.keys().copied().collect();
        keys.sort_unstable();
        for k in keys {
            let mut g = groups.remove(&k).unwrap_or_default();
            g.sort_by_key(|(ei, _)| *ei);
            let stems: Vec<Stemmed> = g.iter().map(|(_, s)| s.clone()).collect();
            if stems.len() < 2 {
                for (ei, s) in &g {
                    let tip = beam::lone(s, &mut items);
                    if let Some(p) = places.get_mut(&(b, k.0, *ei)) {
                        p.tip = Some(tip);
                    }
                }
                continue;
            }
            let tips = beam::group(&stems, &mut items);
            for ((ei, _), tip) in g.iter().zip(tips) {
                if let Some(p) = places.get_mut(&(b, k.0, *ei)) {
                    p.tip = Some(tip);
                }
            }
        }
    }
    let mut sky = Skyline::new(plan.x0, plan.x1 + 6.0);
    for it in &items {
        sky.add(it.bounds());
    }
    marks::ties(score, staff, plan, &heads, &mut items, &mut sky);
    marks::tuplets(staff, plan, &places, &mut items, &mut sky);
    marks::ottavas(staff, plan, &places, &mut items, &mut sky);
    marks::dynamics(&dynamics, &mut items, &mut sky);
    if top {
        marks::tempo(score, cols, plan, text, &mut items, &mut sky);
    }
    StaffDrawn { items, sky }
}

fn glyph(g: Glyph, x: f32, y: f32, owner: Option<u64>) -> Item {
    Item::Glyph {
        glyph: g,
        x,
        y,
        scale: 1.0,
        owner,
    }
}

/// Ledger lines through and beyond the heads off the staff, each reaching past the heads
/// it serves by Bravura's extension.
fn ledger_lines(sh: &chord::Shape, ex: f32, out: &mut Vec<Item>) {
    let ext = defaults::LEGER_LINE_EXTENSION;
    let w = defaults::LEGER_LINE_THICKNESS;
    let hi = sh.heads.iter().map(|h| h.pos).max().unwrap_or(0);
    let lo = sh.heads.iter().map(|h| h.pos).min().unwrap_or(0);
    let mut lines: Vec<i32> = Vec::new();
    let mut p = 6;
    while p <= hi {
        lines.push(p);
        p += 2;
    }
    let mut p = -6;
    while p >= lo {
        lines.push(p);
        p -= 2;
    }
    for l in lines {
        let served = sh
            .heads
            .iter()
            .filter(|h| if l > 0 { h.pos >= l } else { h.pos <= l });
        let (a, b) = served.fold((f32::MAX, f32::MIN), |(a, b), h| {
            (a.min(h.dx), b.max(h.dx + h.glyph.width()))
        });
        if a > b {
            continue;
        }
        out.push(Item::Line {
            x0: ex + a - ext,
            y0: y_of(l),
            x1: ex + b + ext,
            y1: y_of(l),
            w,
            owner: None,
        });
    }
}
