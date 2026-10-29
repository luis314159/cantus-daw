//! Systems and pages: where the systems break (the best spacing over the whole piece, not
//! a greedy fill), each justified to its width; staves stacked by their skylines; bar
//! lines, the brace, names and bar numbers; pages filled and spread.

use super::columns::{self, BarCols, StaffRef};
use super::draw::{self, StaffDrawn, SystemPlan};
use super::notes;
use super::{Engraved, Format, LayoutOptions, Page, SystemMap};
use crate::display::{Anchor, Item};
use crate::glyphs::defaults;
use crate::model::Score;
use crate::text::Face;

const NAME_SIZE: f32 = 2.2;
const TITLE_SIZE: f32 = 4.2;
const COMPOSER_SIZE: f32 = 2.2;
const BAR_NUMBER_SIZE: f32 = 1.5;
const BRACE_GAP: f32 = 0.35;
/// A last system emptier than this costs the break that leaves it (two thirds full).
const LAST_FULL: f32 = 0.65;
/// A last system at least this full is justified like the others (Dorico's default, half
/// the width; MuseScore's is a third); a shorter one keeps its natural spacing.
const JUSTIFY_LAST: f32 = 0.5;

fn staves_of(score: &Score) -> Vec<StaffRef> {
    let mut v = Vec::new();
    for (p, part) in score.parts.iter().enumerate() {
        for s in 0..part.staves.len() {
            v.push(StaffRef { part: p, staff: s });
        }
    }
    v
}

/// The fixed widths of a system from bar `i` to `j - 1`: the preamble, each bar's lead
/// and mid-system time signatures, and a courtesy time signature at its end.
struct Widths {
    preamble: f32,
    ideal: Vec<f32>,
    min: Vec<f32>,
    extra: Vec<f32>,
}

fn widths(score: &Score, staves: &[StaffRef], cols: &[BarCols]) -> Widths {
    let preamble = |sig: bool| {
        staves
            .iter()
            .map(|r| {
                let st = &score.parts[r.part].staves[r.staff];
                let key = score.parts[r.part].key;
                draw::preamble(st.clef, key, sig.then_some(score.bars[0].sig), 0.0, None)
            })
            .fold(0.0, f32::max)
    };
    let mut ideal = Vec::with_capacity(cols.len());
    let mut min = Vec::with_capacity(cols.len());
    let mut extra = Vec::with_capacity(cols.len());
    for (i, c) in cols.iter().enumerate() {
        let (a, b) = c.widths();
        ideal.push(a);
        min.push(b);
        let bar = &score.bars[i];
        extra.push(if bar.show_sig && i > 0 {
            0.75 + draw::time_width(bar.sig)
        } else {
            0.0
        });
    }
    Widths {
        preamble: preamble(true).max(preamble(false)),
        ideal,
        min,
        extra,
    }
}

/// Where the systems break: `[start, end)` per system, chosen to space every system as
/// near its ideal as the width allows.
fn breaks(score: &Score, w: &Widths, width: f32, indent: f32) -> Vec<(usize, usize)> {
    let n = w.ideal.len();
    if n == 0 {
        return Vec::new();
    }
    let courtesy = |j: usize| {
        if j < n && score.bars[j].show_sig {
            0.75 + draw::time_width(score.bars[j].sig) + 0.3
        } else {
            0.0
        }
    };
    let mut best = vec![f32::MAX; n + 1];
    let mut from = vec![0usize; n + 1];
    best[0] = 0.0;
    for j in 1..=n {
        for i in (0..j).rev() {
            if j - i > 32 {
                break;
            }
            if best[i] == f32::MAX {
                continue;
            }
            let ind = if i == 0 { indent } else { 0.0 };
            let fixed =
                ind + w.preamble + courtesy(j) + (i + 1..j).map(|k| w.extra[k]).sum::<f32>();
            let natural: f32 = fixed + w.ideal[i..j].iter().sum::<f32>();
            let tight: f32 = fixed + w.min[i..j].iter().sum::<f32>();
            if tight > width && j - i > 1 {
                break;
            }
            let ratio = width / natural.max(1.0);
            let cost = if j == n {
                // The last system may stay short (Gould), but not a scrap: under
                // two thirds full it costs more the emptier it is.
                if ratio >= 1.0 {
                    let fill = 1.0 / ratio;
                    if fill < LAST_FULL {
                        (LAST_FULL - fill).powi(2) * 200.0
                    } else {
                        0.0
                    }
                } else {
                    (1.0 / ratio - 1.0).powi(2) * 300.0
                }
            } else if ratio >= 1.0 {
                (ratio - 1.0).powi(2) * 100.0
            } else {
                (1.0 / ratio - 1.0).powi(2) * 300.0
            };
            let total = best[i] + cost + 1.0;
            if total < best[j] {
                best[j] = total;
                from[j] = i;
            }
        }
        if best[j] == f32::MAX {
            from[j] = j - 1;
            best[j] = best[j - 1].min(f32::MAX / 2.0) + 1000.0;
        }
    }
    let mut out = Vec::new();
    let mut j = n;
    while j > 0 {
        let i = from[j];
        out.push((i, j));
        j = i;
    }
    out.reverse();
    out
}

/// Lay out the system `[a, b)` at `x0` with `width` (or its natural width when `None`).
#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_range_loop)] // `k` names the bar in the columns and the widths.
fn plan(
    score: &Score,
    cols: &[BarCols],
    w: &Widths,
    a: usize,
    b: usize,
    x0: f32,
    width: Option<f32>,
    first: bool,
    staves: &[StaffRef],
) -> SystemPlan {
    let n = score.bars.len();
    let courtesy = (b < n && score.bars[b].show_sig).then(|| score.bars[b].sig);
    let courtesy_w = courtesy.map_or(0.0, |s| 0.75 + draw::time_width(s) + 0.3);
    let sig = (first || score.bars[a].show_sig).then_some(score.bars[a].sig);
    let pre_end = staves
        .iter()
        .map(|r| {
            let st = &score.parts[r.part].staves[r.staff];
            draw::preamble(st.clef, score.parts[r.part].key, sig, x0, None)
        })
        .fold(x0, f32::max);
    let fixed: f32 = (pre_end - x0)
        + courtesy_w
        + (a..b)
            .map(|k| cols[k].lead + if k > a { w.extra[k] } else { 0.0 })
            .sum::<f32>();
    let flexible = |s: f32| -> f32 {
        (a..b)
            .map(|k| {
                if cols[k].columns.is_empty() {
                    columns::WHOLE_REST_BAR * s.max(0.7) - cols[k].lead
                } else {
                    cols[k].spaces(s).iter().sum::<f32>()
                }
            })
            .sum()
    };
    let s = match width {
        None => 1.0,
        Some(target) => {
            let goal = target - fixed;
            let (mut lo, mut hi) = (0.05f32, 40.0f32);
            for _ in 0..50 {
                let mid = (lo + hi) / 2.0;
                if flexible(mid) < goal {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            (lo + hi) / 2.0
        }
    };
    let mut bar_x = Vec::with_capacity(b - a);
    let mut bar_end = Vec::with_capacity(b - a);
    let mut col_x = Vec::with_capacity(b - a);
    let mut x = pre_end;
    for k in a..b {
        // The bar line before bar k (or the preamble's end for the first).
        bar_x.push(x);
        if k > a {
            x += w.extra[k];
        }
        x += cols[k].lead;
        let mut xs = Vec::with_capacity(cols[k].columns.len());
        if cols[k].columns.is_empty() {
            x += columns::WHOLE_REST_BAR * s.max(0.7) - cols[k].lead;
        } else {
            for sp in cols[k].spaces(s) {
                xs.push(x);
                x += sp;
            }
        }
        col_x.push(xs);
        bar_end.push(x);
    }
    SystemPlan {
        first_bar: a,
        last_bar: b - 1,
        x0,
        x1: x + courtesy_w,
        first,
        bar_x,
        bar_end,
        col_x,
        preamble_end: pre_end,
        courtesy,
    }
}

/// A system drawn: its items in system y (the first staff's top line at 0), its reach
/// above and below, the staves' tops, and its map (with system y).
struct Drawn {
    items: Vec<Item>,
    above: f32,
    below: f32,
    first_sky: super::sky::Skyline,
    last_sky: super::sky::Skyline,
    map: SystemMap,
}

#[allow(clippy::too_many_arguments)]
fn draw_system(
    score: &Score,
    staves: &[StaffRef],
    cols: &[BarCols],
    p: &SystemPlan,
    opts: &LayoutOptions,
    names: bool,
    number: bool,
) -> Drawn {
    let drawn: Vec<StaffDrawn> = (0..staves.len())
        .map(|si| notes::draw_staff(score, staves, si, cols, p, &opts.text, si == 0))
        .collect();
    // Stack the staves: within a part (a grand staff) closer, between parts wider.
    let mut tops = vec![0.0f32];
    for si in 1..staves.len() {
        let same = staves[si].part == staves[si - 1].part;
        let (pad, least) = if same {
            (1.0, 4.0 + 5.5)
        } else {
            (1.5, 4.0 + 7.0)
        };
        let d = drawn[si - 1]
            .sky
            .distance_to(&drawn[si].sky, pad)
            .max(least);
        tops.push(tops[si - 1] + d);
    }
    let mut items: Vec<Item> = Vec::new();
    for (si, d) in drawn.iter().enumerate() {
        let dy = tops[si];
        for it in &d.items {
            items.push(shift(it, dy));
        }
    }
    let bottom_of = |si: usize| tops[si] + 4.0;
    // Bar lines through each part's staves; the last bar of the score ends thin–thick.
    let thin = defaults::THIN_BARLINE_THICKNESS;
    let thick = defaults::THICK_BARLINE_THICKNESS;
    let last_of_score = score.bars.len() - 1;
    let parts: Vec<(usize, usize)> = {
        let mut v: Vec<(usize, usize)> = Vec::new();
        for (si, r) in staves.iter().enumerate() {
            match v.last_mut() {
                Some(l) if staves[l.0].part == r.part => l.1 = si,
                _ => v.push((si, si)),
            }
        }
        v
    };
    for (local, &end) in p.bar_end.iter().enumerate() {
        let b = p.first_bar + local;
        for &(s0, s1) in &parts {
            let (y0, y1) = (tops[s0], bottom_of(s1));
            if b == last_of_score {
                items.push(Item::Line {
                    x0: end - thick / 2.0,
                    y0,
                    x1: end - thick / 2.0,
                    y1,
                    w: thick,
                    owner: None,
                });
                let tx = end - thick - defaults::THIN_THICK_BARLINE_SEPARATION - thin / 2.0;
                items.push(Item::Line {
                    x0: tx,
                    y0,
                    x1: tx,
                    y1,
                    w: thin,
                    owner: None,
                });
            } else {
                items.push(Item::Line {
                    x0: end - thin / 2.0,
                    y0,
                    x1: end - thin / 2.0,
                    y1,
                    w: thin,
                    owner: None,
                });
            }
        }
    }
    // The system's opening line when it holds more than one staff, and a brace per grand
    // staff.
    if staves.len() > 1 {
        items.push(Item::Line {
            x0: p.x0 + thin / 2.0,
            y0: 0.0,
            x1: p.x0 + thin / 2.0,
            y1: bottom_of(staves.len() - 1),
            w: thin,
            owner: None,
        });
    }
    let brace_w =
        |h: f32| crate::glyphs::Glyph::Brace.width() * crate::outline::brace_width_scale(h);
    let mut name_x = p.x0 - 1.0;
    for &(s0, s1) in &parts {
        if s1 > s0 {
            let (y0, y1) = (tops[s0], bottom_of(s1));
            let w = brace_w(y1 - y0);
            items.push(Item::Brace {
                x: p.x0 - BRACE_GAP - w,
                y0,
                y1,
            });
            name_x = name_x.min(p.x0 - BRACE_GAP - w - 1.0);
        }
    }
    if names {
        for &(s0, s1) in &parts {
            let name = &score.parts[staves[s0].part].name;
            if name.is_empty() {
                continue;
            }
            let cy = (tops[s0] + bottom_of(s1)) / 2.0;
            items.push(Item::Text {
                text: name.clone(),
                x: name_x,
                y: cy + opts.text.cap_height(NAME_SIZE) / 2.0,
                size: NAME_SIZE,
                face: Face::Regular,
                anchor: Anchor::Right,
            });
        }
    }
    if number {
        let n = score.bars[p.first_bar].number;
        let y = drawn[0].sky.top_in(p.x0, p.x0 + 3.0).min(0.0) - 0.8;
        items.push(Item::Text {
            text: n.to_string(),
            x: p.x0,
            y,
            size: BAR_NUMBER_SIZE,
            face: Face::Regular,
            anchor: Anchor::Left,
        });
    }
    let (above, _) = drawn[0].sky.extents();
    let (_, below) = drawn[drawn.len() - 1].sky.extents();
    let mut ticks: Vec<(i64, f32)> = Vec::new();
    for (local, b) in (p.first_bar..=p.last_bar).enumerate() {
        let bar = &score.bars[b];
        let first_x = p.col_x[local]
            .first()
            .copied()
            .unwrap_or(p.bar_x[local] + 1.0);
        ticks.push((bar.start, first_x.min(p.bar_end[local])));
        for (ci, c) in cols[b].columns.iter().enumerate() {
            let t = bar.start + c.t;
            if ticks.last().is_none_or(|l| l.0 < t) {
                ticks.push((t, p.col_x[local][ci]));
            }
        }
        ticks.push((bar.start + bar.len, p.bar_end[local]));
    }
    ticks.dedup_by(|b, a| b.0 == a.0);
    let last = staves.len() - 1;
    Drawn {
        items,
        above: above + if number { 1.0 } else { 0.0 },
        below,
        first_sky: drawn[0].sky.clone(),
        last_sky: drawn[last].sky.clone(),
        map: SystemMap {
            top: 0.0,
            bottom: bottom_of(last),
            x0: p.x0,
            x1: p.x1,
            first_bar: p.first_bar,
            last_bar: p.last_bar,
            ticks,
            staff_tops: tops,
        },
    }
}

fn shift(it: &Item, dy: f32) -> Item {
    let mut it = it.clone();
    match &mut it {
        Item::Glyph { y, .. } => *y += dy,
        Item::Line { y0, y1, .. } | Item::Beam { y0, y1, .. } => {
            *y0 += dy;
            *y1 += dy;
        }
        Item::Dashed { y0, .. } => *y0 += dy,
        Item::Curve { p, .. } => {
            for q in p.iter_mut() {
                q[1] += dy;
            }
        }
        Item::Text { y, .. } => *y += dy,
        Item::Brace { y0, y1, .. } => {
            *y0 += dy;
            *y1 += dy;
        }
    }
    it
}

fn place(d: &Drawn, dy: f32, out: &mut Page) {
    for it in &d.items {
        out.list.items.push(shift(it, dy));
    }
    let mut m = d.map.clone();
    m.top += dy;
    m.bottom += dy;
    for t in m.staff_tops.iter_mut() {
        *t += dy;
    }
    out.systems.push(m);
}

pub fn engrave(score: &Score, opts: &LayoutOptions) -> Engraved {
    let staves = staves_of(score);
    if staves.is_empty() || score.bars.is_empty() {
        return Engraved::default();
    }
    let cols = columns::measure(score, &staves);
    let w = widths(score, &staves, &cols);
    let grand = score.parts.iter().any(|p| p.staves.len() > 1);
    let name_w = score
        .parts
        .iter()
        .map(|p| opts.text.width(&p.name, NAME_SIZE, Face::Regular))
        .fold(0.0, f32::max);
    let brace = if grand { 1.6 } else { 0.0 };
    let indent = if name_w > 0.0 {
        name_w + 1.5 + brace
    } else {
        brace + 0.5
    };
    let (page_w, page_h, margin) = match opts.format {
        Format::Pages(paper) => paper.spaces(),
        Format::Wrap { width } => (width, f32::MAX, 1.0),
        Format::Continuous => (f32::MAX, f32::MAX, 1.0),
    };
    let content_w = page_w - 2.0 * margin;
    let systems: Vec<(usize, usize)> = match opts.format {
        Format::Continuous => vec![(0, score.bars.len())],
        _ => breaks(score, &w, content_w, indent),
    };
    let last = systems.len() - 1;
    let mut drawn: Vec<Drawn> = Vec::with_capacity(systems.len());
    for (k, &(a, b)) in systems.iter().enumerate() {
        let first = k == 0;
        let x0 = margin + if first { indent } else { brace };
        let width = match opts.format {
            Format::Continuous => None,
            _ => {
                let target = content_w - (x0 - margin);
                // The last system is justified only when it is nearly full.
                let natural = w.preamble + (a..b).map(|i| w.ideal[i] + w.extra[i]).sum::<f32>();
                if k == last && natural < JUSTIFY_LAST * target {
                    None
                } else {
                    Some(target)
                }
            }
        };
        let p = plan(score, &cols, &w, a, b, x0, width, first, &staves);
        drawn.push(draw_system(score, &staves, &cols, &p, opts, first, !first));
    }
    // Stack the systems, onto pages when there are pages.
    let mut pages: Vec<Page> = Vec::new();
    let mut page = Page::default();
    let header_h = if opts.header && matches!(opts.format, Format::Pages(_)) {
        TITLE_SIZE + COMPOSER_SIZE + 4.0
    } else {
        0.0
    };
    let mut y = margin + header_h;
    let mut prev: Option<(f32, usize)> = None;
    let gap_least = 4.0 + 8.0;
    for (k, d) in drawn.iter().enumerate() {
        let top = match prev {
            None => y + d.above.max(2.5),
            Some((ptop, pk)) => {
                ptop + drawn[pk]
                    .last_sky
                    .distance_to(&d.first_sky, 2.5)
                    .max(gap_least)
                    + (drawn[pk].map.bottom - 4.0)
            }
        };
        if let Format::Pages(_) = opts.format {
            if prev.is_some() && top + 4.0 + d.below > page_h - margin {
                pages.push(std::mem::take(&mut page));
                y = margin;
                let top = y + d.above.max(2.5);
                place(d, top, &mut page);
                prev = Some((top, k));
                continue;
            }
        }
        place(d, top, &mut page);
        prev = Some((top, k));
    }
    pages.push(page);
    // Size, header, page numbers, vertical justification of full pages.
    let n_pages = pages.len();
    for (pi, pg) in pages.iter_mut().enumerate() {
        let bottom = pg.systems.last().map(|s| s.bottom).unwrap_or(0.0)
            + drawn.last().map_or(4.0, |d| d.below)
            + 2.0;
        match opts.format {
            Format::Pages(_) => {
                pg.list.width = page_w;
                pg.list.height = page_h;
                justify_page(pg, page_h - margin, pi + 1 < n_pages);
                if opts.header && pi == 0 {
                    pg.list.items.push(Item::Text {
                        text: score.title.clone(),
                        x: page_w / 2.0,
                        y: margin + TITLE_SIZE * 0.8,
                        size: TITLE_SIZE,
                        face: Face::Bold,
                        anchor: Anchor::Centre,
                    });
                    if !score.composer.is_empty() {
                        pg.list.items.push(Item::Text {
                            text: score.composer.clone(),
                            x: page_w - margin,
                            y: margin + TITLE_SIZE + COMPOSER_SIZE + 0.8,
                            size: COMPOSER_SIZE,
                            face: Face::Regular,
                            anchor: Anchor::Right,
                        });
                    }
                }
                if opts.header && pi > 0 {
                    pg.list.items.push(Item::Text {
                        text: (pi + 1).to_string(),
                        x: page_w / 2.0,
                        y: page_h - margin / 2.0,
                        size: 1.8,
                        face: Face::Regular,
                        anchor: Anchor::Centre,
                    });
                }
            }
            _ => {
                pg.list.width = pg.systems.iter().map(|s| s.x1).fold(0.0, f32::max) + margin;
                pg.list.height = bottom + margin;
            }
        }
    }
    Engraved { pages }
}

/// Spread a full page's systems so its last staff ends at the bottom margin (Gould: a
/// page of music fills its page), when it is more than three quarters full.
fn justify_page(pg: &mut Page, bottom: f32, full: bool) {
    let n = pg.systems.len();
    if n < 2 || !full {
        return;
    }
    let last = pg.systems[n - 1].bottom;
    let first = pg.systems[0].top;
    let spare = bottom - 4.0 - last;
    if spare <= 0.0 || (last - first) / (bottom - first) < 0.75 {
        return;
    }
    let step = spare / (n - 1) as f32;
    // Each system moves down by its index × step: items by the system they belong to.
    let bounds: Vec<(f32, f32)> = pg
        .systems
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let lo = if i == 0 {
                f32::MIN
            } else {
                (pg.systems[i - 1].bottom + s.top) / 2.0
            };
            let hi = if i + 1 == n {
                f32::MAX
            } else {
                (s.bottom + pg.systems[i + 1].top) / 2.0
            };
            (lo, hi)
        })
        .collect();
    for it in pg.list.items.iter_mut() {
        let b = it.bounds();
        let cy = (b[1] + b[3]) / 2.0;
        if let Some(i) = bounds.iter().position(|(lo, hi)| cy >= *lo && cy < *hi) {
            *it = shift(it, i as f32 * step);
        }
    }
    for (i, s) in pg.systems.iter_mut().enumerate() {
        let d = i as f32 * step;
        s.top += d;
        s.bottom += d;
        for t in s.staff_tops.iter_mut() {
            *t += d;
        }
    }
}
