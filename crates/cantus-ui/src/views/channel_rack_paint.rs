//! Painting the channel rack with the kit (ADR 0024): FL's rows — the enable lamp, the
//! route number in a sunken well, the channel button, the selector — and its signature,
//! the **step keycap**: a raised cap with a slit across its top, grey and maroon in
//! alternating groups, lit pale blue and pink.
//!
//! Everything here reads and draws. The rows × steps grid is one mesh (ADR 0011), with a
//! level of detail: full keycaps while there are few and they are big enough to see,
//! flat keys below that, and only the lit steps once cells are specks.

use super::{Pattern, RackGeometry, RowKind, ANY_PITCH};
use crate::app::CantusApp;
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::light;
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use crate::views::clips_chrome::Digits;
use crate::views::piano_roll::Axis;
use cantus_core::Ticks;
use egui::{pos2, vec2, Align2, Pos2, Rect};

const GRID_TOKENS: &[Tok] = &[
    Tok::PanelLight,
    Tok::StepOdd,
    Tok::StepEven,
    Tok::StepOddLit,
    Tok::StepEvenLit,
    Tok::Playhead,
    Tok::GridBar,
];
const HEADER_TOKENS: &[Tok] = &[Tok::Panel, Tok::Text, Tok::TextDim, Tok::Playhead];
const ROW_TOKENS: &[Tok] = &[
    Tok::PanelLight,
    Tok::Led,
    Tok::Well,
    Tok::Text,
    Tok::PanelRaised,
    Tok::Selection,
];

/// How much detail a step is worth at its size on screen.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) struct StepLod {
    /// Draw the unlit steps at all.
    pub cells: bool,
    /// Full keycaps: the raised recipe with its slit. Only while there are few enough
    /// that their triangles do not matter — which is also when they are big enough to see.
    pub caps: bool,
}

impl StepLod {
    const CELLS_ABOVE: f32 = 4.0;
    pub(super) const CAPS_ABOVE_W: f32 = 11.0;
    pub(super) const CAPS_ABOVE_H: f32 = 14.0;
    /// Keycaps stop at this many cells on screen, whatever their size.
    const CAPS_MAX_CELLS: usize = 1_200;

    pub fn for_size(w: f32, h: f32, cells_on_screen: usize) -> StepLod {
        StepLod {
            cells: w >= Self::CELLS_ABOVE && h >= Self::CELLS_ABOVE,
            caps: w >= Self::CAPS_ABOVE_W
                && h >= Self::CAPS_ABOVE_H
                && cells_on_screen <= Self::CAPS_MAX_CELLS,
        }
    }
}

/// The step's well inside its cell (round 39, des-views): 18 x 22 on a pitch of 20 at a
/// 30-px row — a point of ground either side, the row's height less 8 — and never
/// shorter than a sliver.
pub(super) fn cap_rect(cell: Rect, lod: StepLod) -> Rect {
    let gx = if cell.width() >= 8.0 { 1.0 } else { 0.5 };
    let gy = if lod.caps {
        ((cell.height() - 22.0) * 0.5).max(2.0)
    } else {
        (cell.height() * 0.12).max(1.0)
    };
    cell.shrink2(egui::vec2(gx, gy))
}

/// One step (round 39, des-views): a well with a 1-px edge, r 3, the second and fourth
/// beat of each bar a step lighter; lit, the step tinted with the channel's hue and
/// filled from the floor to its velocity (45 % + 55 % x velocity) with a lit top.
/// `even` is the alternate beat; `strength` the velocity (0…1); `hue` the channel's.
#[allow(clippy::too_many_arguments)]
pub(super) fn step_key(
    b: &mut Brush,
    r: Rect,
    lit: bool,
    even: bool,
    strength: f32,
    muted: bool,
    hue: egui::Color32,
    selected: bool,
    lod: StepLod,
    s: &Style,
) {
    let room = crate::views::room::Room::of(&s.t);
    let well = if even {
        if room.dark {
            mix(room.well, room.surface, 0.75)
        } else {
            mix(room.well, room.edge, 0.35)
        }
    } else {
        room.well
    };
    let px = b.px();
    let r = b.snap_rect(r);
    let (room_a, fill_a) = match (muted, selected) {
        (true, _) => (0.18, 0.30),
        (false, true) => (0.42, 0.86),
        (false, false) => (0.42, 0.80),
    };
    if !lod.caps {
        // Small: a flat well, or the hue at the step's level.
        let c = if lit {
            mix(well, hue, fill_a * (0.6 + 0.4 * strength.clamp(0.0, 1.0)))
        } else {
            well
        };
        b.rect(r, Fill::Solid(c));
        return;
    }
    let rad = s.r(3.0);
    b.rrect(r, rad, Fill::Solid(room.edge));
    let inner = r.shrink(px);
    let ground = if lit { mix(well, hue, room_a) } else { well };
    b.rrect(inner, (rad - px).max(0.0), Fill::Solid(ground));
    if lit {
        let level = (0.45 + 0.55 * strength.clamp(0.0, 1.0)) * inner.height();
        let top = b.snap(inner.bottom() - level);
        let f = Rect::from_min_max(pos2(inner.left(), top), inner.max);
        let c = mix(well, hue, fill_a);
        b.rrect(f, (rad - px).max(0.0), Fill::Solid(c));
        let lip = if room.dark {
            mix(c, room.white, 0.35)
        } else {
            mix(c, room.white, 0.25)
        };
        b.hline(f.left() + px, f.right() - px, top + 0.5 * px, px, lip);
    }
}

/// The grid's ground, the playhead's column, and every step on screen.
#[allow(clippy::too_many_arguments)]
pub(super) fn grid(
    b: &mut Brush,
    app: &CantusApp,
    geo: &RackGeometry,
    grid: Rect,
    pattern: &Pattern,
    axes: &[Axis],
    playing: Option<usize>,
    at: &mut Vec<Ticks>,
    s: &Style,
) {
    let t = &s.t;
    let grid = b.snap_rect(grid);
    b.register(grid, "rack_grid", GRID_TOKENS, &[]);
    b.rect(grid, Fill::Solid(t.panel_light));
    let first = geo.scroll_steps.floor().max(0.0) as usize;
    let visible = (grid.width() / geo.step_w).ceil() as usize + 2;
    let last = (first + visible).min(pattern.count);
    let ts = app.project().time_signature;
    let per_beat = (ts.ticks_per_beat().0 / pattern.step.0).max(1) as usize;
    let per_bar = (ts.ticks_per_bar().0 / pattern.step.0).max(1) as usize;
    let x_of = |step: usize| geo.grid_left + (step as f32 - geo.scroll_steps) * geo.step_w;
    // The rows end where the channels do: below them the rack is empty ground.
    let rows_bottom = geo
        .rows
        .last()
        .map_or(grid.top(), |r| r.top + r.height)
        .clamp(grid.top(), grid.bottom());
    // A bar line between bars; the groups of keys say where the beats are.
    // Through the metre changes (ADR 0060): a bar line where a bar starts, the beat groups
    // by the beats of the bar each step is in.
    let map = crate::views::bars::map(app.project());
    let several = !map.is_one_metre();
    let bar_at = |step: usize| {
        if several {
            crate::views::bars::is_bar_line(&map, pattern.tick(step))
        } else {
            step.is_multiple_of(per_bar)
        }
    };
    let row_h0 = geo.rows.first().map(|r| r.height).unwrap_or(16.0);
    // r39:el-views — big steps part their beats and bars by the gaps round their wells
    // (`gapped`), not by lines.
    let lined = !(geo.step_w >= StepLod::CAPS_ABOVE_W && row_h0 >= StepLod::CAPS_ABOVE_H);
    for step in (first..last).filter(|_| lined) {
        if bar_at(step) && step > 0 {
            b.vline(
                x_of(step),
                grid.top(),
                rows_bottom,
                b.px(),
                shade(t.panel_light, -0.12),
            );
        } else if step > 0 && step.is_multiple_of(per_beat) {
            // r37:finish — every beat's group of steps parted by a faint line (the judge:
            // the 4-step grouping not seen).
            b.vline(
                x_of(step),
                grid.top(),
                rows_bottom,
                b.px(),
                shade(t.panel_light, if t.dark { -0.06 } else { -0.08 }),
            );
        }
    }
    if let Some(step) = playing.filter(|p| *p >= first && *p < last) {
        let col = Rect::from_min_size(
            pos2(x_of(step), grid.top()),
            vec2(geo.step_w, rows_bottom - grid.top()),
        );
        b.rect(b.snap_rect(col), Fill::Solid(with_alpha(t.playhead, 0.18)));
    }
    let row_h = geo.rows.first().map(|r| r.height).unwrap_or(16.0);
    let on_screen = geo
        .rows
        .iter()
        .filter(|r| r.lane.is_some() && r.top + r.height > grid.top() && r.top < grid.bottom())
        .count();
    let lod = StepLod::for_size(geo.step_w, row_h, on_screen * last.saturating_sub(first));
    // The gap at a step's sides: wider where a beat starts, wider still at a bar.
    let side = |step: usize| {
        if !lod.caps {
            0.0
        } else if bar_at(step) {
            2.5
        } else if step.is_multiple_of(per_beat) {
            1.0
        } else {
            0.0
        }
    };
    let gapped = |r: Rect, step: usize| {
        Rect::from_min_max(
            pos2(r.left() + side(step), r.top()),
            pos2(r.right() - side(step + 1), r.bottom()),
        )
    };
    let group = |step: usize| {
        if several {
            map.bbt(pattern.tick(step)).1 % 2 == 0
        } else {
            (step / per_beat) % 2 == 1
        }
    };
    app.rack.audio_columns.set(0); // r39:el-views — what the audio rows drew this frame
    for (ri, row) in geo.rows.iter().enumerate() {
        if row.top + row.height < grid.top() || row.top > grid.bottom() {
            continue;
        }
        let Some(track) = app.project().tracks.get(row.track) else {
            continue;
        };
        // r39:lo-rest — the row's band across the grid as in the channel column: the
        // surface, a 1-px gap of the base under it (des-views), tinted for the chosen one.
        {
            let room = crate::views::room::Room::of(t);
            let band = b.snap_rect(Rect::from_min_max(
                pos2(grid.left(), row.top),
                pos2(grid.right(), row.top + row.height),
            ));
            let axis = axes.get(row.track);
            let chosen = app.selected_track == Some(track.id)
                && match row.kind {
                    RowKind::Track => row.lane.is_some() || axis.is_some_and(|a| a.len() != 1),
                    RowKind::Pad => {
                        let pitch = axis.map(|a| a.pitch(row.lane.unwrap_or(0)));
                        pitch.and_then(|p| track.instrument.pulsus()?.index_of_pitch(p))
                            == Some(app.rack.selected_pad)
                    }
                };
            let ground = if chosen {
                mix(
                    room.surface,
                    crate::theme::track_color(track.color),
                    if room.dark { 0.10 } else { 0.12 },
                )
            } else {
                room.surface
            };
            b.rect(band, Fill::Solid(room.base));
            b.rect(
                Rect::from_min_max(band.min, pos2(band.right(), band.bottom() - b.px())),
                Fill::Solid(ground),
            );
        }
        let Some(lane) = row.lane else {
            heading_roll(b, app, geo, ri, grid, pattern, axes, at, s);
            continue;
        };
        // r39:el-views — the channel's own hue fills its steps; the chosen one's fuller.
        let hue = crate::theme::track_color(track.color);
        let chosen = app.selected_track == Some(track.id);
        // r39:el-views — a melodic channel playing more than steps shows its pattern as
        // a small roll in one well over the page (des-views).
        if lane == ANY_PITCH && lod.caps {
            // r39:el-views — an audio channel: its sound over the page, a column a pixel
            // (des-views).
            // (`clip_on` takes note clips only: an audio clip has no steps.)
            let end = pattern.start + pattern.window();
            let audio = track.clips.iter().find(|c| {
                c.audio.is_some() && c.lane == 0 && c.start < end && c.end() > pattern.start
            });
            if let Some(clip) = audio {
                let page = Rect::from_min_max(
                    geo.cell(ri, first).min,
                    geo.cell(ri, last.saturating_sub(1).max(first)).max,
                );
                audio_row(b, app, page, (ri, first), geo, clip, pattern, hue, s);
                continue;
            }
            if let Some(clip) = pattern.clip_on(track).filter(|c| is_melodic(c, pattern)) {
                let page = Rect::from_min_max(
                    geo.cell(ri, first).min,
                    geo.cell(ri, last.saturating_sub(1).max(first)).max,
                );
                mini_roll(
                    b,
                    page,
                    ri,
                    geo,
                    clip,
                    pattern,
                    (first, last),
                    per_beat,
                    per_bar,
                    hue,
                    chosen,
                    at,
                    s,
                );
                continue;
            }
        }
        if lod.cells {
            for step in first..last {
                let r = gapped(cap_rect(geo.cell(ri, step), lod), step);
                step_key(b, r, false, group(step), 0.0, false, hue, false, lod, s);
            }
        }
        let Some(clip) = pattern.clip_on(track) else {
            continue;
        };
        let pitch = (lane != ANY_PITCH).then(|| axes.get(row.track).map_or(0, |a| a.pitch(lane)));
        for n in &clip.notes {
            if pitch.is_some_and(|p| p != n.pitch) {
                continue;
            }
            clip.timeline_positions(n.start, at);
            for on in at.iter().copied() {
                let Some(step) = pattern.index_of(on) else {
                    continue;
                };
                if step < first || step >= last {
                    continue;
                }
                let r = gapped(cap_rect(geo.cell(ri, step), lod), step);
                let strength = n.velocity as f32 / 127.0;
                step_key(
                    b,
                    r,
                    true,
                    group(step),
                    strength,
                    n.muted || clip.muted,
                    hue,
                    chosen,
                    lod,
                    s,
                );
            }
        }
    }
}

/// Whether a channel's clip plays more than steps: more than one pitch, or notes longer
/// than a step.
fn is_melodic(clip: &cantus_core::Clip, pattern: &Pattern) -> bool {
    let first = clip.notes.first().map(|n| n.pitch);
    clip.notes
        .iter()
        .any(|n| Some(n.pitch) != first || n.length > pattern.step * 2)
}

/// A melodic channel's pattern as a small roll (des-views): one well over the page, beat
/// lines, its notes 3 high ranked by pitch in the channel's hue.
#[allow(clippy::too_many_arguments)]
fn mini_roll(
    b: &mut Brush,
    page: Rect,
    ri: usize,
    geo: &RackGeometry,
    clip: &cantus_core::Clip,
    pattern: &Pattern,
    (first, last): (usize, usize),
    per_beat: usize,
    per_bar: usize,
    hue: egui::Color32,
    chosen: bool,
    at: &mut Vec<Ticks>,
    s: &Style,
) {
    let room = crate::views::room::Room::of(&s.t);
    let well = b.snap_rect(page.shrink2(vec2(1.0, ((page.height() - 22.0) * 0.5).max(2.0))));
    b.rrect(well, s.r(3.0), Fill::Solid(room.edge));
    let inner = well.shrink(b.px());
    b.rrect(inner, s.r(3.0), Fill::Solid(room.well));
    for step in first..last {
        if step.is_multiple_of(per_beat) && step > first {
            let a = if step.is_multiple_of(per_bar) {
                0.60
            } else {
                0.30
            };
            let x = geo.cell(ri, step).left();
            b.vline(
                x,
                inner.top(),
                inner.bottom(),
                b.px(),
                mix(room.well, room.ink4, a),
            );
        }
    }
    let mask = clip
        .notes
        .iter()
        .fold(0u128, |m, n| m | 1u128 << (n.pitch & 127));
    let used = mask.count_ones().max(1) as f32;
    let span = inner.height() - 6.0;
    let row = (span / used).min(5.0);
    let top = inner.center().y - row * used * 0.5;
    let ink = mix(room.well, hue, if chosen { 0.95 } else { 0.78 });
    let px_step = geo.step_w;
    for n in &clip.notes {
        let rank = mask
            .checked_shr(u32::from(n.pitch & 127) + 1)
            .unwrap_or(0)
            .count_ones() as f32;
        let y = top + rank * row + row * 0.5;
        clip.timeline_positions(n.start, at);
        for on in at.iter().copied() {
            let Some(step) = pattern.index_of(on) else {
                continue;
            };
            if step < first || step >= last {
                continue;
            }
            let x0 = geo.cell(ri, step).left()
                + (on - pattern.tick(step)).0 as f32 / pattern.step.0 as f32 * px_step;
            let x1 = (x0 + n.length.0 as f32 / pattern.step.0 as f32 * px_step - 1.0)
                .min(inner.right())
                .max(x0 + 2.0);
            let r = Rect::from_min_max(pos2(x0, y - 1.5), pos2(x1, y + 1.5));
            b.rrect(b.snap_rect(r), 1.0, Fill::Solid(ink));
        }
    }
}

/// An audio channel's pattern (des-views): one well over the page and the sound's peaks in
/// it, a column a physical pixel, in the channel's hue at 72 %.
#[allow(clippy::too_many_arguments)]
fn audio_row(
    b: &mut Brush,
    app: &CantusApp,
    page: Rect,
    (ri, first): (usize, usize),
    geo: &RackGeometry,
    clip: &cantus_core::Clip,
    pattern: &Pattern,
    hue: egui::Color32,
    s: &Style,
) {
    let room = crate::views::room::Room::of(&s.t);
    let well = b.snap_rect(page.shrink2(vec2(1.0, ((page.height() - 22.0) * 0.5).max(2.0))));
    b.rrect(well, s.r(3.0), Fill::Solid(room.edge));
    let inner = well.shrink(b.px());
    b.rrect(inner, s.r(3.0), Fill::Solid(room.well));
    let Some(region) = clip.audio.as_ref() else {
        return;
    };
    let Some(src) = app.project().audio_source(region.source) else {
        return;
    };
    let crate::waveform::Lookup::Ready(peaks) =
        app.peaks.lookup(src, app.audio_base(), Some(b.ctx()))
    else {
        return;
    };
    let map = crate::views::bars::map(app.project());
    let rate = f64::from(peaks.rate.max(1));
    let start_s = map.seconds_at(clip.start.0 as f64);
    let offset_s = map.seconds_at((clip.start + clip.offset).0 as f64) - start_s;
    let x0 = geo.cell(ri, first).left();
    let tick0 = pattern.tick(first).0 as f64;
    let per_px = pattern.step.0 as f64 / f64::from(geo.step_w.max(0.1));
    let frame = |x: f32| {
        let tick = tick0 + f64::from(x - x0) * per_px;
        (map.seconds_at(tick) - start_s + offset_s) * rate + f64::from(region.slip_frames)
    };
    let ink = mix(room.well, hue, 0.72);
    let (px, cy) = (b.px(), inner.center().y);
    let half = (inner.height() * 0.5 - 2.0).max(1.0);
    let mut x = inner.left();
    while x < inner.right() {
        let p = peaks.peak_in(frame(x), frame(x + px)).min(1.0);
        if p > 0.0 {
            b.vline(x + px * 0.5, cy - p * half, cy + p * half, px, ink);
            app.rack.audio_columns.set(app.rack.audio_columns.get() + 1);
        }
        x += px;
    }
}

/// A kit's heading row across the grid: FL's mini piano roll — every pad's hits as ticks
/// in a sunken strip, so the kit's rhythm reads before it is unfolded.
#[allow(clippy::too_many_arguments)]
fn heading_roll(
    b: &mut Brush,
    app: &CantusApp,
    geo: &RackGeometry,
    ri: usize,
    grid: Rect,
    pattern: &Pattern,
    axes: &[Axis],
    at: &mut Vec<Ticks>,
    s: &Style,
) {
    let row = geo.rows[ri];
    let Some(track) = app.project().tracks.get(row.track) else {
        return;
    };
    let t = &s.t;
    let left = geo.grid_left - geo.scroll_steps * geo.step_w;
    let right = (left + geo.step_w * pattern.count as f32).min(grid.right());
    let strip = Rect::from_min_max(
        pos2(left.max(grid.left()) + 2.0, row.top + 3.0),
        pos2(right - 2.0, row.top + row.height - 3.0),
    );
    if strip.width() < 4.0 || strip.height() < 4.0 {
        return;
    }
    light::sunken(
        b,
        strip,
        s.r(2.0),
        mix(t.ink, t.row_black, 0.4),
        1.0,
        t.depth,
    );
    let Some(clip) = pattern.clip_on(track) else {
        return;
    };
    let Some(axis) = axes.get(row.track) else {
        return;
    };
    let rows = axis.len().max(1) as f32;
    let tint = crate::theme::track_color(clip.color.unwrap_or(track.color));
    let ink = if t.dark {
        shade(tint, 0.18)
    } else {
        shade(tint, -0.12)
    };
    let inner = strip.shrink(2.0);
    let h = (inner.height() / rows).clamp(1.0, 3.0);
    for n in &clip.notes {
        let Some(lane) = axis.lane_of(n.pitch) else {
            continue;
        };
        clip.timeline_positions(n.start, at);
        for on in at.iter().copied() {
            let Some(step) = pattern.index_of(on) else {
                continue;
            };
            let x = geo.grid_left + (step as f32 - geo.scroll_steps) * geo.step_w;
            if x + geo.step_w < inner.left() || x > inner.right() {
                continue;
            }
            let y = inner.bottom() - (lane as f32 + 0.5) / rows * inner.height();
            b.rect(
                Rect::from_min_max(
                    pos2(x.max(inner.left()), y - h * 0.5),
                    pos2((x + geo.step_w * 0.8).min(inner.right()), y + h * 0.5),
                ),
                Fill::Solid(ink),
            );
        }
    }
}

/// The header over the grid: bar numbers, thinned by doubling, and the playhead's mark.
pub(super) fn header(
    b: &mut Brush,
    app: &CantusApp,
    geo: &RackGeometry,
    header: Rect,
    pattern: &Pattern,
    playing: Option<usize>,
    s: &Style,
) {
    let t = &s.t;
    let header = b.snap_rect(header);
    b.register(header, "rack_header", HEADER_TOKENS, &[]);
    b.rect(header, Fill::V(shade(t.panel, 0.02), shade(t.panel, -0.02)));
    b.hline(
        header.left(),
        header.right(),
        header.bottom() - 0.5 * b.px(),
        b.px(),
        shade(t.panel, -0.2),
    );
    let ts = app.project().time_signature;
    let map = crate::views::bars::map(app.project());
    let per_bar = (ts.ticks_per_bar().0 / pattern.step.0).max(1) as usize;
    let mut every = 1usize;
    while geo.step_w * ((per_bar * every) as f32) < 40.0 * s.k {
        every *= 2;
    }
    let label_every = per_bar * every;
    let first = geo.scroll_steps.floor().max(0.0) as usize;
    let visible = (header.width() / geo.step_w).ceil() as usize + 2;
    let last = (first + visible).min(pattern.count);
    let mut digits = Digits::default();
    let x_of = |step: usize| geo.grid_left + (step as f32 - geo.scroll_steps) * geo.step_w;
    for step in (first - first % label_every)..last {
        // Through the metre changes (ADR 0060): every bar line, thinned by its number.
        let (bar, _, _) = map.bbt(pattern.tick(step));
        let on = if map.is_one_metre() {
            step % label_every == 0
        } else {
            crate::views::bars::is_bar_line(&map, pattern.tick(step))
                && (bar - 1).rem_euclid(every as i64) == 0
        };
        if !on {
            continue;
        }
        let x = x_of(step);
        let color = if (bar - 1) % 4 == 0 {
            t.text
        } else {
            t.text_dim
        };
        let tx = Text::new(digits.fmt(bar), Role::Label, color)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let r = Rect::from_min_max(
            pos2(x + 3.0 * s.k, header.top()),
            pos2(x + geo.step_w * label_every as f32 - 2.0, header.bottom()),
        );
        let _ = text::paint(b, r, &tx, s.tk());
    }
    if let Some(step) = playing.filter(|p| *p >= first && *p < last) {
        let x = x_of(step);
        b.rect(
            b.snap_rect(Rect::from_min_max(
                pos2(x, header.bottom() - 3.0),
                pos2(x + geo.step_w, header.bottom()),
            )),
            Fill::Solid(t.playhead),
        );
    }
}

/// Where the parts of a channel row go (des-views' measures, logical px from the column's
/// left: the lamp at 16, the knobs at 36 and 58, the mixer track's well 24 × 18 at 72, the
/// name plate 150 × 20 at 104 — a kit's pads stepped in 14, with the tree line at 110 —
/// and the target mark 3 × 14 at 262). A column narrower than the design's keeps the
/// controls and shortens the plate.
#[derive(Copy, Clone, Debug)]
pub(super) struct RowRects {
    /// The row's whole band in the channel column.
    pub strip: Rect,
    pub led: Pos2,
    pub pan: Rect,
    pub vol: Rect,
    pub route: Option<Rect>,
    /// The name plate.
    pub button: Rect,
    /// The target mark (FL's selector): a click there selects the channel.
    pub selector: Rect,
    /// A kit's tree line, at its x (pads only).
    pub tree_x: Option<f32>,
    /// A kit's chevron, at the plate's left (a kit's own row only).
    pub fold: Option<Rect>,
}

/// The knobs' square, at 100 %.
pub(super) const KNOB: f32 = 22.0;
/// The plate's left and its width, the pads' indent and the target mark's x (100 %).
pub(super) const PLATE_X: f32 = 104.0;
pub(super) const PLATE_W: f32 = 150.0;
pub(super) const PAD_INDENT: f32 = 14.0;
pub(super) const TARGET_X: f32 = 262.0;

/// The rects of a row: `kit` says it is a kit's own row (its plate carries a chevron).
pub(super) fn row_rects(strip: Rect, kind: RowKind, kit: bool, s: &Style) -> RowRects {
    let k = s.k;
    let at = |x: f32| strip.left() + x * k;
    let cy = strip.center().y;
    let led = pos2(at(16.0), cy);
    let knob = |x: f32| Rect::from_center_size(pos2(at(x), cy), vec2(KNOB * k, KNOB * k));
    let (pan, vol) = (knob(36.0), knob(58.0));
    let route = (kind == RowKind::Track).then(|| {
        let h = (18.0 * k).min(strip.height() - 2.0 * k);
        Rect::from_min_size(pos2(at(72.0), cy - h * 0.5), vec2(24.0 * k, h))
    });
    // The target mark at 262, or 6 from the column's right where it is narrower.
    let tx = at(TARGET_X).min(strip.right() - 6.0 * k);
    let th = (14.0 * k).min(strip.height() - 6.0 * k);
    let selector = Rect::from_min_size(pos2(tx, cy - th * 0.5), vec2(3.0 * k, th));
    let indent = if kind == RowKind::Pad {
        PAD_INDENT
    } else {
        0.0
    };
    let ph = (20.0 * k).min(strip.height() - 4.0 * k);
    let left = at(PLATE_X + indent);
    let right = at(PLATE_X + PLATE_W).min(tx - 8.0 * k).max(left + 24.0 * k);
    let button = Rect::from_min_max(pos2(left, cy - ph * 0.5), pos2(right, cy + ph * 0.5));
    RowRects {
        strip,
        led,
        pan,
        vol,
        route,
        button,
        selector,
        tree_x: (kind == RowKind::Pad).then(|| at(110.0)),
        fold: kit.then(|| Rect::from_min_size(button.min, vec2(18.0 * k, button.height()))),
    }
}

/// What a row shows beyond its track.
#[derive(Copy, Clone, Debug)]
pub(super) struct RowLook<'a> {
    pub name: &'a str,
    pub route: Option<&'a str>,
    /// Sounding: the enable lamp is lit.
    pub on: bool,
    pub selected: bool,
    /// The channel's own colour (the stripe, the selected row and plate).
    pub hue: egui::Color32,
    pub button_hovered: bool,
    /// A kit's own row: how many pads it has and whether they are shown.
    pub kit: Option<(usize, bool)>,
    /// A kit's pad: whether it is the kit's last (the tree line ends at it).
    pub last_pad: bool,
}

/// A row's static parts: the stripe, the lamp, the mixer track's well, the name plate with
/// its name and the target mark (des-views). The plate is neutral and lifted, with its
/// foot; the selected channel's is tinted with its colour and its name set Medium.
pub(super) fn row(b: &mut Brush, r: &RowRects, look: &RowLook<'_>, s: &Style) {
    let t = &s.t;
    let k = s.k;
    let room = crate::views::room::Room::of(t);
    b.register(r.strip, "rack_row", ROW_TOKENS, &[]);
    // The row: the surface, a 1-px gap of the base under it, and the channel's stripe.
    let strip = b.snap_rect(r.strip);
    let band = Rect::from_min_max(strip.min, pos2(strip.right(), strip.bottom() - b.px()));
    let ground = if look.selected {
        // r39:lapis2 — through the theme's selection tint (none in Slate).
        crate::kit::color::selected_fill(
            mix(room.surface, look.hue, if room.dark { 0.10 } else { 0.12 }),
            t.selection_tint,
            1.0,
        )
    } else {
        room.surface
    };
    b.rect(strip, Fill::Solid(room.base));
    b.rect(band, Fill::Solid(ground));
    let stripe = Rect::from_min_size(band.min, vec2(3.0 * k, band.height()));
    let hue = if look.selected {
        look.hue
    } else {
        mix(room.surface, look.hue, 0.72)
    };
    b.rect(b.snap_rect(stripe), Fill::Solid(hue));
    crate::kit::led::paint(
        b,
        r.led,
        11.0 * k,
        look.on,
        s,
        Tok::Led,
        Tok::PanelLight,
        1.0,
    );
    if let (Some(rr), Some(route)) = (r.route, look.route) {
        crate::kit::well::readout(b, rr, s, crate::kit::State::REST, route, true);
    }
    // A kit's tree: a hairline down from the kit's plate, a tick into each pad's.
    if let Some(x) = r.tree_x {
        let x = b.snap(x);
        let bottom = if look.last_pad {
            r.button.center().y
        } else {
            strip.bottom()
        };
        b.vline(x, strip.top(), bottom, b.px(), room.ink4);
        b.hline(
            x,
            r.button.left() - 2.0 * k,
            b.snap(r.button.center().y),
            b.px(),
            room.ink4,
        );
    }
    // The plate: `raised` (Night) or `raised` toward the surface (Paper), a 1-px foot.
    let plate = if room.dark {
        room.raised
    } else {
        mix(room.raised, room.surface, 0.20)
    };
    let plate = if look.selected {
        mix(plate, look.hue, if room.dark { 0.22 } else { 0.20 })
    } else if look.button_hovered {
        mix(plate, room.ink1, 0.05)
    } else {
        plate
    };
    let p = b.snap_rect(r.button);
    b.rrect(p, s.r(3.0), Fill::Solid(plate));
    b.rect(
        Rect::from_min_max(
            pos2(p.left() + s.r(3.0), p.bottom() - b.px()),
            pos2(p.right() - s.r(3.0), p.bottom()),
        ),
        Fill::Solid(mix(plate, room.edge, 0.5)),
    );
    // The name: Medium for the selected channel and a kit, ink 3 for a silent pad.
    let mut left = p.left() + 9.0 * k;
    let mut right = p.right() - 8.0 * k;
    if let Some((pads, open)) = look.kit {
        let c = pos2(p.left() + 10.0 * k, p.center().y);
        crate::views::arrangement::head::chevron(b, c, open, room.ink2, k);
        left = p.left() + 20.0 * k;
        let mut d = Digits::default();
        let count = crate::i18n::tf("rack.kit_pads", &[("n", &d.fmt(pads as i64))]);
        let small = s.tk() * 10.0 / 11.0;
        let tx = Text::new(&count, Role::Label, room.ink3)
            .caps(false)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let w = text::measure(b.ctx(), &tx, small).x;
        if p.width() > w + 90.0 * k {
            let _ = text::paint(b, p.shrink2(vec2(8.0 * k, 0.0)), &tx, small);
            right -= w + 6.0 * k;
        }
    }
    let ink = if look.route.is_none() && !look.on {
        room.ink3
    } else {
        crate::kit::color::ensure_contrast(room.ink1, plate, 4.5)
    };
    let role = if look.selected || look.kit.is_some() {
        Role::Control
    } else {
        Role::Body
    };
    let name = Text::new(look.name, role, ink)
        .caps(false)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::Middle));
    let _ = text::paint(
        b,
        Rect::from_min_max(pos2(left, p.top()), pos2(right, p.bottom())),
        &name,
        s.tk(),
    );
    // The target mark: the accent on the selected channel, ink 4 at 35 % otherwise.
    let mark = if look.selected {
        room.accent
    } else {
        with_alpha(room.ink4, 0.35)
    };
    b.rrect(b.snap_rect(r.selector), 1.5 * k, Fill::Solid(mark));
}

/// The foot's height under the last row, at 100 %: the drop zone and its margins.
pub(super) const FOOT_H: f32 = 72.0;

/// Where the foot's parts go under the last row (`below`): "Add a channel" in a 150 × 24
/// well under the channel column, the drop zone (56 high) across the grid.
pub(super) fn foot_rects(channels: Rect, grid: Rect, below: f32, s: &Style) -> (Rect, Rect) {
    let k = s.k;
    let add = Rect::from_min_size(
        pos2(channels.left() + 12.0 * k, below + 8.0 * k),
        vec2((150.0 * k).min(channels.width() - 20.0 * k), 24.0 * k),
    );
    let zone = Rect::from_min_max(
        pos2(grid.left() + 2.0 * k, below + 8.0 * k),
        pos2(grid.right() - 8.0 * k, below + 64.0 * k),
    );
    (add, zone)
}

/// "Add a channel": a well with a plus and the words in ink 2 (lifted under the pointer).
pub(super) fn add_well(b: &mut Brush, r: Rect, hovered: bool, s: &Style) {
    let k = s.k;
    let room = crate::views::room::Room::of(&s.t);
    b.register(r, "rack_add", ROW_TOKENS, &[]);
    let r = b.snap_rect(r);
    let fill = if hovered { room.well_hi } else { room.well };
    b.rrect(r, s.r(3.0), Fill::Solid(room.edge));
    b.rrect(r.shrink(b.px()), s.r(3.0), Fill::Solid(fill));
    let ink = if hovered { room.ink1 } else { room.ink2 };
    let g = Rect::from_center_size(
        pos2(r.left() + 14.0 * k, r.center().y),
        vec2(10.0, 10.0) * k,
    );
    crate::kit::glyph::paint(b, g, crate::kit::glyph::Glyph::Plus, ink);
    let tx = Text::key("rack.add_channel", Role::Body, ink)
        .caps(false)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(
        b,
        Rect::from_min_max(pos2(g.right() + 8.0 * k, r.top()), r.max),
        &tx,
        s.tk(),
    );
}

/// The drop zone: a dashed outline (4 on, 3 off) in ink 4 at 90 %, r 6, and its two lines
/// in ink 3 and ink 4; lit with the accent while a sound is held over it.
pub(super) fn drop_zone(b: &mut Brush, r: Rect, lit: bool, s: &Style) {
    let k = s.k;
    let room = crate::views::room::Room::of(&s.t);
    b.register(r, "rack_drop", ROW_TOKENS, &[]);
    let r = b.snap_rect(r);
    let ring = if lit {
        b.rrect(r, 6.0 * k, Fill::Solid(with_alpha(room.accent, 0.10)));
        room.accent
    } else {
        with_alpha(room.ink4, 0.90)
    };
    // The outline as one dashed path round the rounded rect.
    let rad = 6.0 * k;
    let q = r.shrink(b.px() * 0.5);
    // Four quarter circles of nine points and the start again: no allocation.
    let mut pts = [Pos2::ZERO; 37];
    let pi = std::f32::consts::PI;
    let corners = [
        (pos2(q.right() - rad, q.top() + rad), -0.5 * pi),
        (pos2(q.right() - rad, q.bottom() - rad), 0.0),
        (pos2(q.left() + rad, q.bottom() - rad), 0.5 * pi),
        (pos2(q.left() + rad, q.top() + rad), pi),
    ];
    for (j, (c, a0)) in corners.into_iter().enumerate() {
        for i in 0..9 {
            let a = a0 + i as f32 / 8.0 * 0.5 * pi;
            pts[j * 9 + i] = c + vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a)) * rad;
        }
    }
    pts[36] = pts[0];
    b.dashed(&pts, b.px(), ring, 4.0 * k, 3.0 * k);
    let (title, sub) = if lit {
        (room.accent, room.ink3)
    } else {
        (room.ink3, room.ink4)
    };
    let big = Text::key("rack.drop_title", Role::Body, title)
        .caps(false)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let at = Rect::from_center_size(
        pos2(r.center().x, r.center().y - 8.0 * k),
        vec2(r.width() - 16.0 * k, 16.0 * k),
    );
    let _ = text::paint(b, at, &big, s.tk());
    let small = Text::key("rack.drop_sub", Role::Label, sub)
        .caps(false)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::End));
    let at = Rect::from_center_size(
        pos2(r.center().x, r.center().y + 9.0 * k),
        vec2(r.width() - 16.0 * k, 14.0 * k),
    );
    let _ = text::paint(b, at, &small, s.tk());
}
