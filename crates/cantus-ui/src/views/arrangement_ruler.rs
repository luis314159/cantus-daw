//! r39:el-arrange — the ruler, the sections lane and the canvas's grid in Slate
//! (`des-arrange/README.md`, *Ruler and top lanes*, *Canvas*): bar numbers from 26 px a bar
//! (then every 2, 4, 8, 16), beats ticked from 6 px a beat and named from 40, the loop as a
//! band in the ruler's top 5 px, the time selection, the playhead's cap (hollow where play
//! started); under it the sections as named spans with their length, and the markers; the
//! grid in 4-bar groups by value, bars, beats and sixteenths by zoom, a section's start a
//! step stronger, the selected track's lane tinted, after the song's end a veil.

use super::slate::{say, Ink, W};
use super::{ticks_to_beats, TimelineGeometry, LOOP_LANE_H};
use crate::app::CantusApp;
use crate::kit::{Brush, Fill, Style, Tok};
use cantus_core::Ticks;
use egui::{pos2, vec2, Align2, Color32, Rect};

/// The ruler's height (logical px, punch lane apart), and the sections lane's.
pub const RULER_H: f32 = 24.0;
pub const SECTIONS_H: f32 = 20.0;

/// What the ruler keeps between frames: where play started, and a text buffer.
#[derive(Debug, Default)]
pub struct State {
    /// The playhead while stopped: where the next play starts (the hollow cap).
    pub play_from: Ticks,
    buf: String,
}

const TOKENS: &[Tok] = &[
    Tok::Toolbar,
    Tok::Text,
    Tok::TextDim,
    Tok::Placeholder,
    Tok::Highlight,
    Tok::Readout,
    Tok::ReadoutSoft,
    Tok::LoopEdge,
];

/// Every how many bars a number is written at `bar_px` per bar.
pub fn label_every(bar_px: f32, k: f32) -> i64 {
    let mut every = 1i64;
    while bar_px * (every as f32) < 26.0 * k && every < 1024 {
        every *= 2;
    }
    every
}

/// The ruler: numbers, ticks, the loop band, the selection, the playhead's cap. `numbers`
/// is the band under the loop (and punch) lanes.
pub(super) fn paint(
    b: &mut Brush,
    app: &CantusApp,
    st: &mut State,
    geo: &TimelineGeometry,
    rect: Rect,
    numbers: Rect,
    s: &Style,
) {
    b.register(rect, "ruler", TOKENS, &[]);
    let ink = Ink::of(s);
    let k = s.k;
    let rect = b.snap_rect(rect);
    b.rect(rect, Fill::Solid(ink.surface));
    let p = app.project();
    let map = crate::views::bars::map(p);
    let first = geo.scroll_beats.max(0.0);
    let last = geo.beats(rect.right());
    let ts = p.time_signature;
    let per_bar = ts.numerator as f32 * 4.0 / ts.denominator as f32;
    let bar_px = per_bar * geo.px_per_beat;
    let every = label_every(bar_px, k);
    let playing = app.engine.is_playing();
    let pos = app.engine.position();
    if !playing {
        st.play_from = pos;
    }
    let play_bar = map.bbt(pos).0;
    // The loop's bars under the numbers, while it is on.
    let lp = p.loop_region.as_ref();
    if let (Some(l), true) = (lp, p.loop_enabled) {
        let x0 = geo.x(ticks_to_beats(l.start)).max(rect.left());
        let x1 = geo.x(ticks_to_beats(l.end)).min(rect.right());
        if x1 > x0 {
            let r = Rect::from_min_max(pos2(x0, numbers.top()), pos2(x1, numbers.bottom()));
            b.rect(b.snap_rect(r), Fill::Solid(Ink::a(ink.accent, 0.06)));
        }
    }
    // Beat and sixteenth ticks, and bars' ticks.
    let beat_px = geo.px_per_beat;
    let bottom = numbers.bottom();
    let beats_named = beat_px >= 40.0 * k;
    crate::views::bars::ladder(&map, first, last, beat_px, 6.0 * k, |l| {
        let x = geo.x(l.beat);
        if l.bar {
            return;
        }
        b.vline(x, bottom - 4.0 * k, bottom, b.px(), ink.ink4);
    });
    if beats_named {
        let step = 0.25;
        let mut beat = (first / step).floor() * step;
        while beat <= last {
            if (beat.fract() * 4.0).round() as i64 % 4 != 0 {
                let x = geo.x(beat);
                b.vline(x, bottom - 2.0 * k, bottom, b.px(), ink.ink4);
            }
            beat += step;
        }
    }
    // Bars: a labelled one a line from y+8 and its number; the rest a 7-px tick.
    let mut digits = crate::views::clips_chrome::Digits::default();
    let chips = super::tempo::chips(app);
    crate::views::bars::each_bar(&map, first, last, |n, start, len| {
        let x = geo.x(start);
        let labelled = (n - 1).rem_euclid(every) == 0;
        if labelled {
            b.vline(x, numbers.top(), bottom, b.px(), ink.ink3);
        } else {
            b.vline(x, bottom - 7.0 * k, bottom, b.px(), ink.ink4);
        }
        // A metre change's chip after the number, from 36 px a bar.
        let mut after = 0.0;
        let tick = crate::views::beats_to_ticks(start);
        if len * beat_px >= 36.0 * k {
            if let Some((_, sig)) = chips.iter().find(|(t, _)| *t == tick) {
                let r =
                    super::tempo::chip_rect(b.ctx(), x + 3.0 * k, numbers.top(), bottom, *sig, s);
                after = super::tempo::paint_chip(b, r, *sig, false, s) + 2.0 * k;
            }
        }
        if labelled {
            // r39:lapis2 — the bar numbers are readouts (the logo's gold in Slate Lapis).
            let c = if n == play_bar {
                s.t.readout_or(ink.ink1, false)
            } else {
                s.t.readout_or(ink.ink2, true)
            };
            let r = Rect::from_min_max(
                pos2(x + 5.0 * k + after, numbers.top()),
                pos2(x + len * beat_px * every as f32 - 2.0 * k, bottom),
            );
            let _ = say(
                b,
                r,
                digits.fmt(n),
                W::Medium,
                11.0,
                c,
                Align2::LEFT_CENTER,
                s,
            );
        }
        // Beats named "41.2" when close.
        if beats_named {
            let beats = len.round() as i64;
            for j in 1..beats {
                let bx = geo.x(start + j as f32);
                if bx > rect.right() || bx < rect.left() - 40.0 * k {
                    continue;
                }
                st.buf.clear();
                use std::fmt::Write as _;
                let _ = write!(st.buf, "{n}.{}", j + 1);
                let r = Rect::from_min_max(
                    pos2(bx + 4.0 * k, numbers.top()),
                    pos2(bx + 40.0 * k, bottom),
                );
                let _ = say(
                    b,
                    r,
                    &st.buf,
                    W::Regular,
                    9.0,
                    ink.ink3,
                    Align2::LEFT_CENTER,
                    s,
                );
            }
        }
    });
    // The transport's marks (the loop band's ends, the playhead's cap) in the playhead's
    // colour in a recessed family: the accent in Slate (the same token there), the logo's
    // gold in Slate Lapis (r39:lapis).
    let mark = if s.t.system.recessed() {
        s.t.playhead
    } else {
        ink.accent
    };
    // The loop band: the ruler's top 5 px.
    if let Some(l) = lp {
        let x0 = geo.x(ticks_to_beats(l.start));
        let x1 = geo.x(ticks_to_beats(l.end));
        let on = p.loop_enabled;
        let band = Rect::from_min_max(pos2(x0, rect.top()), pos2(x1, rect.top() + 5.0 * k));
        let (fill, end) = if on {
            (Ink::a(ink.accent, if ink.dark { 0.32 } else { 0.24 }), mark)
        } else {
            (Ink::a(ink.ink4, 0.45), ink.ink3)
        };
        b.rrect(band, 1.5 * k, Fill::Solid(fill));
        // r39:lapis2 — the region's edges along its whole length, where the theme gives
        // them a colour (the logo's gold in Slate Lapis; Slate draws none).
        let edge = s.t.loop_edge;
        if edge.a() > 0 {
            let e = if on {
                edge
            } else {
                Ink::a(crate::kit::color::opaque(edge), 0.4)
            };
            let px = b.px();
            b.hline(x0, x1, band.top() + 0.5 * px, px, e);
            b.hline(x0, x1, band.bottom() - 0.5 * px, px, e);
        }
        for x in [x0, x1] {
            let e = Rect::from_center_size(pos2(x, rect.top() + 3.5 * k), vec2(2.0 * k, 7.0 * k));
            b.rect(b.snap_rect(e), Fill::Solid(end));
        }
    }
    // The time selection.
    if let Some((from, to)) = app.time_selection {
        let x0 = geo.x(ticks_to_beats(from));
        let x1 = geo.x(ticks_to_beats(to));
        let r = Rect::from_min_max(pos2(x0, rect.top() + 7.0 * k), pos2(x1, bottom));
        b.rect(b.snap_rect(r), Fill::Solid(Ink::a(ink.ink1, 0.08)));
        for x in [x0, x1] {
            b.vline(x, r.top(), r.bottom(), b.px(), ink.ink3);
        }
    }
    // The playhead's cap at the ruler's foot; where play started, hollow.
    if playing {
        cap(
            b,
            geo.x(ticks_to_beats(st.play_from)),
            bottom,
            &ink,
            (true, mark),
            k,
        );
    }
    cap(
        b,
        geo.x(ticks_to_beats(pos)),
        bottom,
        &ink,
        (false, mark),
        k,
    );
}

/// The playhead's cap: a 9 × 7.5 pentagon pointing down at `y`.
fn cap(b: &mut Brush, x: f32, y: f32, ink: &Ink, (hollow, mark): (bool, Color32), k: f32) {
    let pts = [
        pos2(x - 4.5 * k, y - 7.5 * k),
        pos2(x + 4.5 * k, y - 7.5 * k),
        pos2(x + 4.5 * k, y - 3.0 * k),
        pos2(x, y),
        pos2(x - 4.5 * k, y - 3.0 * k),
    ];
    if hollow {
        b.polygon(&pts, ink.ink2);
        let inner = pts.map(|p| {
            pos2(
                x + (p.x - x) * 0.72,
                y - 7.5 * k + (p.y - (y - 7.5 * k)) * 0.72 + 0.8 * k,
            )
        });
        b.polygon(&inner, ink.surface);
    } else {
        b.polygon(&pts, mark);
    }
}

/// The loop lane's height (the press band at the ruler's top).
pub fn loop_lane() -> f32 {
    LOOP_LANE_H
}

#[path = "arrangement_grid.rs"]
mod grid;
pub(super) use grid::{grid, overlay};
#[path = "arrangement_sections.rs"]
pub mod sections;
