//! r39:el-arrange — the canvas's grid and what goes over the clips (`des-arrange`,
//! *Canvas*): 4-bar groups by value (16-bar groups below 8 px a bar); bars, beats and
//! sixteenths by zoom; a section's start a step stronger; each row's foot; the selected
//! track's lane and an armed one tinted; folders' lanes summarised; below the last track the
//! grid veiled and the drop hint; after the song's end a veil and a line. Over the clips:
//! the time range, the rubber band, the playhead.

use super::super::slate::{say, Ink, W};
use super::super::{ticks_to_beats, ArrangementView, TimelineGeometry};
use crate::app::CantusApp;
use crate::kit::{Brush, Fill, Style, Tok};
use egui::{pos2, Align2, Rect};

/// How much of the selection tint the selected track's lane takes: a lane is the largest
/// fill there is (r39:lapis2).
const LANE_TINT: f32 = 0.35;

const TOKENS: &[Tok] = &[
    Tok::Panel,
    Tok::Ink,
    Tok::Grid,
    Tok::GridStrong,
    Tok::GridBar,
];

/// The ground of the lanes, the lines, the rows' feet and tints, the folders' summaries.
pub fn grid(b: &mut Brush, app: &CantusApp, geo: &TimelineGeometry, rect: Rect, s: &Style) {
    b.register(rect, "lanes", TOKENS, &[]);
    let ink = Ink::of(s);
    let k = s.k;
    let rect = b.snap_rect(rect);
    b.rect(rect, Fill::Solid(ink.base));
    let p = app.project();
    let map = crate::views::bars::map(p);
    let first = geo.scroll_beats.max(0.0);
    let last = geo.beats(rect.right());
    let ts = p.time_signature;
    let per_bar = ts.numerator as f32 * 4.0 / ts.denominator as f32;
    let bar_px = per_bar * geo.px_per_beat;
    let group: i64 = if bar_px < 8.0 * k { 16 } else { 4 };
    // The groups by value.
    crate::views::bars::each_bar(&map, first, last, |n, start, len| {
        if (n - 1).div_euclid(group) % 2 == 1 {
            let x0 = geo.x(start).max(rect.left());
            let x1 = geo.x(start + len).min(rect.right());
            if x1 > x0 {
                let r = Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.bottom()));
                b.rect(b.snap_rect(r), Fill::Solid(ink.lane2));
            }
        }
    });
    // Tints of rows: the selected track's lane, an armed one.
    for (i, r) in geo.rows.iter().enumerate() {
        if r.height <= 0.0 || r.top > rect.bottom() || r.top + r.height < rect.top() {
            continue;
        }
        let Some(t) = p.tracks.get(i) else {
            continue;
        };
        let band = Rect::from_min_max(
            pos2(rect.left(), r.top),
            pos2(rect.right(), r.top + r.height),
        );
        if app.selected_track == Some(t.id) {
            let c = ink.hue(crate::theme::track_color(t.color));
            b.rect(b.snap_rect(band), Fill::Solid(Ink::a(c, 0.05)));
            // r39:lapis2 — and the selection's tint, faint, where the theme has one.
            if ink.sel_tint.a() > 0 {
                let k = ink.sel_tint.a() as f32 / 255.0 * LANE_TINT;
                let c = crate::kit::color::opaque(ink.sel_tint);
                b.rect(b.snap_rect(band), Fill::Solid(Ink::a(c, k)));
            }
        } else if t.armed {
            b.rect(b.snap_rect(band), Fill::Solid(Ink::a(ink.rec, 0.04)));
        }
    }
    // The line ladder: sixteenths from 40 px a beat, beats from 6, bars from 8 px a bar.
    if geo.px_per_beat >= 40.0 * k {
        let six = Ink::to(ink.base, ink.grid_beat, 0.55);
        let step = 0.25;
        let mut beat = (first / step).floor() * step;
        while beat <= last {
            if (beat * 4.0).round() as i64 % 4 != 0 {
                b.vline(geo.x(beat), rect.top(), rect.bottom(), b.px(), six);
            }
            beat += step;
        }
    }
    crate::views::bars::ladder(&map, first, last, geo.px_per_beat, 6.0 * k, |l| {
        let x = geo.x(l.beat);
        if l.bar {
            if bar_px >= 8.0 * k {
                b.vline(x, rect.top(), rect.bottom(), b.px(), ink.grid_bar);
            }
        } else {
            b.vline(x, rect.top(), rect.bottom(), b.px(), ink.grid_beat);
        }
    });
    // A section's start, a step stronger.
    let strong = Ink::to(ink.grid_bar, ink.ink4, 0.6);
    for sec in &p.sections {
        let x = geo.x(ticks_to_beats(sec.start));
        if x >= rect.left() && x <= rect.right() {
            b.vline(x, rect.top(), rect.bottom(), b.px(), strong);
        }
    }
    // Each row's foot.
    let foot = Ink::to(ink.base, ink.gutter, 0.55);
    for r in &geo.rows {
        if r.height <= 0.0 {
            continue;
        }
        let y = r.top + r.height;
        if y >= rect.top() && r.top <= rect.bottom() {
            b.hline(rect.left(), rect.right(), y - 0.5 * b.px(), b.px(), foot);
        }
    }
    // Folders' lanes: the group's span as one quiet body, each member's clips as bars.
    for fr in &geo.folders {
        let band = Rect::from_min_max(
            pos2(rect.left(), fr.top),
            pos2(rect.right(), fr.top + fr.height),
        );
        if band.bottom() < rect.top() || band.top() > rect.bottom() {
            continue;
        }
        b.hline(
            rect.left(),
            rect.right(),
            band.bottom() - 0.5 * b.px(),
            b.px(),
            foot,
        );
        let Some(fo) = p.folder(fr.id) else {
            continue;
        };
        let hue = ink.hue(crate::theme::track_color(fo.color));
        let (lo, hi) = p
            .folder_members(fr.id)
            .flat_map(|t| t.clips.iter())
            .fold((i64::MAX, i64::MIN), |(a, z), c| {
                (a.min(c.start.0), z.max(c.end().0))
            });
        if lo >= hi {
            continue;
        }
        let x0 = geo.x(ticks_to_beats(cantus_core::Ticks(lo)));
        let x1 = geo.x(ticks_to_beats(cantus_core::Ticks(hi)));
        let body = Rect::from_min_max(
            pos2(x0, band.top() + 2.0 * k),
            pos2(x1, band.bottom() - 2.0 * k),
        );
        b.rrect(body, s.r(3.0), Fill::Solid(Ink::a(hue, 0.10)));
        b.hline(
            x0 + 2.0 * k,
            x1 - 2.0 * k,
            body.top() + 0.5 * k,
            b.px(),
            Ink::a(hue, 0.30),
        );
        let bar_h = if fo.open { 5.0 * k } else { 8.0 * k };
        let n = p.folder_members(fr.id).count().max(1) as f32;
        let gap = 1.0 * k;
        let avail = body.height() - 4.0 * k;
        let h = (bar_h).min((avail - gap * (n - 1.0)) / n).max(1.0);
        let total = h * n + gap * (n - 1.0);
        let mut y = body.center().y - total * 0.5;
        for t in p.folder_members(fr.id) {
            let c = Ink::a(ink.hue(crate::theme::track_color(t.color)), 0.55);
            for clip in &t.clips {
                let cx0 = geo.x(ticks_to_beats(clip.start)).max(rect.left());
                let cx1 = geo.x(ticks_to_beats(clip.end())).min(rect.right());
                if cx1 > cx0 {
                    let r = Rect::from_min_max(pos2(cx0, y), pos2(cx1 - b.px(), y + h));
                    b.rect(r, Fill::Solid(c));
                }
            }
            y += h + gap;
        }
    }
    // After the song's end: a veil and a line.
    let end = geo.x(ticks_to_beats(p.content_end()));
    if p.content_end().0 > 0 && end < rect.right() {
        let r = Rect::from_min_max(pos2(end.max(rect.left()), rect.top()), rect.max);
        b.rect(b.snap_rect(r), Fill::Solid(Ink::a(ink.gutter, 0.35)));
        if end >= rect.left() {
            b.vline(end, rect.top(), rect.bottom(), b.px(), ink.ink4);
        }
    }
    // Below the last track: the grid veiled, and the hint from 90 px of room.
    let below = geo.bottom() + s.s(super::super::head::ADD_ROW);
    if below < rect.bottom() {
        let r = Rect::from_min_max(pos2(rect.left(), below.max(rect.top())), rect.max);
        b.rect(b.snap_rect(r), Fill::Solid(Ink::a(ink.base, 0.45)));
        if r.height() >= 90.0 * k && !p.tracks.is_empty() {
            let w = &app.arrangement.words.drop;
            let _ = say(
                b,
                r,
                w,
                W::Regular,
                12.0,
                ink.ink3,
                Align2::CENTER_CENTER,
                s,
            );
        }
    }
}

/// Over the clips: the time range, the rubber band, the playhead's line.
pub fn overlay(
    b: &mut Brush,
    app: &CantusApp,
    v: &ArrangementView,
    geo: &TimelineGeometry,
    rect: Rect,
    s: &Style,
) {
    let ink = Ink::of(s);
    if let Some((from, to)) = app.time_selection {
        let x0 = geo.x(ticks_to_beats(from));
        let x1 = geo.x(ticks_to_beats(to));
        let column = Rect::from_min_max(
            pos2(x0, rect.top()),
            pos2(x1, rect.bottom().min(geo.bottom().max(rect.top()))),
        );
        let a = if ink.dark { 0.07 } else { 0.06 };
        b.rect(b.snap_rect(column), Fill::Solid(Ink::a(ink.ink1, a)));
        for x in [x0, x1] {
            b.vline(x, column.top(), column.bottom(), b.px(), ink.ink3);
        }
    }
    if let Some((a, c)) = v.marquee_rect() {
        let r = Rect::from_two_pos(a, c);
        b.rect(r, Fill::Solid(Ink::a(ink.ink1, 0.07)));
        b.rrect_stroke(r, 0.0, b.px(), ink.ink3);
    }
    // A clip moved or resized: the edge it lands on, through every row (the snap line).
    if let Some(d) = v.drag.as_ref() {
        use super::super::DragMode;
        let at = match d.mode {
            DragMode::Move | DragMode::TrimStart => Some(d.anchor_start + d.delta),
            DragMode::TrimEnd | DragMode::Loop => Some(d.anchor_start + d.anchor_length + d.delta),
            DragMode::Slip => None,
        };
        if let Some(t) = at.filter(|_| d.delta != cantus_core::Ticks::ZERO) {
            let x = geo.x(ticks_to_beats(t));
            b.vline(x, rect.top(), rect.bottom(), b.px(), ink.accent);
        }
    }
    let ph = geo.x(ticks_to_beats(app.engine.position()));
    if ph >= rect.left() && ph <= rect.right() {
        b.vline(ph, rect.top(), rect.bottom(), b.px(), ink.accent);
    }
}
