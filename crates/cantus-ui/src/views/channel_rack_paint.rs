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
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use crate::views::clips_chrome::{Digits, Tint};
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
    const CAPS_ABOVE_W: f32 = 11.0;
    const CAPS_ABOVE_H: f32 = 14.0;
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

/// The keycap's rect inside a step's cell: FL's 12 on a 16 pitch, and never taller than
/// a key is sensible.
pub(super) fn cap_rect(cell: Rect, lod: StepLod) -> Rect {
    if !lod.caps {
        let gap = if cell.width() >= 8.0 { 1.5 } else { 0.0 };
        return cell.shrink(gap);
    }
    // Never wider than a key is tall-ish: a fitted pattern on a wide screen gives each
    // step 60 points, and a keycap stretched to fill them stops reading as a key.
    let w = (cell.width() * 0.76)
        .min(cell.height() * 0.74)
        .round()
        .max(3.0);
    let h = (cell.height() - 6.0).min(w * 1.8).max(3.0);
    Rect::from_center_size(cell.center(), vec2(w, h))
}

/// One step key. `even` is the second group of each pair (FL's maroon); `strength` is how
/// hard a lit step hits (0…1, from its velocity).
#[allow(clippy::too_many_arguments)]
pub(super) fn step_key(
    b: &mut Brush,
    r: Rect,
    lit: bool,
    even: bool,
    strength: f32,
    muted: bool,
    lod: StepLod,
    s: &Style,
) {
    let t = &s.t;
    let rest = if even { t.step_even } else { t.step_odd };
    let base = if lit {
        let on = if even {
            t.step_even_lit
        } else {
            t.step_odd_lit
        };
        let on = mix(rest, on, 0.45 + 0.55 * strength.clamp(0.0, 1.0));
        if muted {
            mix(on, rest, 0.6)
        } else {
            on
        }
    } else {
        rest
    };
    if !lod.caps {
        b.rect(r, Fill::V(shade(base, 0.03), shade(base, -0.04)));
        return;
    }
    light::raised(
        b,
        r,
        s.r(2.5),
        base,
        RaisedOpts {
            // The slit: FL's keycap line a quarter of the way down.
            groove: Some(0.24),
            drop: true,
            k: s.k * 0.8,
            depth: t.depth,
            ..Default::default()
        },
    );
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
    for step in first..last {
        if step % per_bar == 0 && step > 0 {
            b.vline(
                x_of(step),
                grid.top(),
                rows_bottom,
                b.px(),
                shade(t.panel_light, -0.12),
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
    let group = |step: usize| (step / per_beat) % 2 == 1;
    for (ri, row) in geo.rows.iter().enumerate() {
        if row.top + row.height < grid.top() || row.top > grid.bottom() {
            continue;
        }
        let Some(track) = app.project().tracks.get(row.track) else {
            continue;
        };
        let Some(lane) = row.lane else {
            heading_roll(b, app, geo, ri, grid, pattern, axes, at, s);
            continue;
        };
        if lod.cells {
            for step in first..last {
                let r = cap_rect(geo.cell(ri, step), lod);
                step_key(b, r, false, group(step), 0.0, false, lod, s);
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
                let r = cap_rect(geo.cell(ri, step), lod);
                let strength = n.velocity as f32 / 127.0;
                step_key(
                    b,
                    r,
                    true,
                    group(step),
                    strength,
                    n.muted || clip.muted,
                    lod,
                    s,
                );
            }
        }
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
        if step % label_every != 0 {
            continue;
        }
        let x = x_of(step);
        let (bar, _, _) = ts.bbt(pattern.tick(step));
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

/// Where the parts of a channel row go.
#[derive(Copy, Clone, Debug)]
pub(super) struct RowRects {
    pub led: Pos2,
    pub pan: Rect,
    pub vol: Rect,
    pub route: Option<Rect>,
    pub button: Rect,
    pub selector: Rect,
}

/// The knobs' square, at 100 %.
pub(super) const KNOB: f32 = 24.0;

pub(super) fn row_rects(strip: Rect, kind: RowKind, s: &Style) -> RowRects {
    let k = s.k;
    let indent = if kind == RowKind::Pad { 14.0 * k } else { 0.0 };
    let cy = strip.center().y;
    let x0 = strip.left() + 6.0 * k + indent;
    let led = pos2(x0 + 6.0 * k, cy);
    let knob =
        |x: f32| Rect::from_center_size(pos2(x + KNOB * 0.5 * k, cy), vec2(KNOB * k, KNOB * k));
    let pan = knob(led.x + 10.0 * k);
    let vol = knob(pan.right() + 2.0 * k);
    let h = (strip.height() - 6.0 * k).clamp(12.0 * k, 24.0 * k);
    let route = (kind == RowKind::Track)
        .then(|| Rect::from_min_size(pos2(vol.right() + 5.0 * k, cy - h * 0.5), vec2(28.0 * k, h)));
    let bx = route.map_or(vol.right() + 5.0 * k, |r| r.right() + 5.0 * k);
    let selector = Rect::from_min_size(
        pos2(strip.right() - 9.0 * k, cy - h * 0.5),
        vec2(4.0 * k, h),
    );
    let button = Rect::from_min_max(
        pos2(bx, cy - h * 0.5),
        pos2(selector.left() - 5.0 * k, cy + h * 0.5),
    );
    RowRects {
        led,
        pan,
        vol,
        route,
        button,
        selector,
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
    pub tint: Tint,
    pub button_hovered: bool,
}

/// A row's static parts: the lamp, the route well, the channel button with its name,
/// and the selector. The button is neutral at rest and takes the channel's colour when it
/// is the selected one (FL).
pub(super) fn row(b: &mut Brush, r: &RowRects, look: &RowLook<'_>, s: &Style) {
    let t = &s.t;
    b.register(
        r.button
            .union(Rect::from_center_size(r.led, vec2(12.0, 12.0))),
        "rack_row",
        ROW_TOKENS,
        &[],
    );
    crate::kit::led::paint(
        b,
        r.led,
        11.0 * s.k,
        look.on,
        s,
        Tok::Led,
        Tok::PanelLight,
        1.0,
    );
    if let (Some(rr), Some(route)) = (r.route, look.route) {
        crate::kit::well::readout(b, rr, s, crate::kit::State::REST, route, true);
    }
    let face = if look.selected {
        look.tint.large
    } else {
        t.panel_raised
    };
    let face = if look.button_hovered {
        shade(face, 0.04)
    } else {
        face
    };
    light::raised(
        b,
        r.button,
        s.r(2.0),
        face,
        RaisedOpts {
            k: s.k,
            depth: t.depth,
            ..Default::default()
        },
    );
    let ink = if look.selected {
        look.tint.on_large
    } else {
        t.text
    };
    let name = Text::new(look.name, Role::Body, ink)
        .align(Align2::CENTER_CENTER)
        .policy(Policy::Elide(Elide::Middle));
    let _ = text::paint(b, r.button.shrink2(vec2(6.0 * s.k, 0.0)), &name, s.tk());
    let sel = if look.selected {
        t.selection
    } else {
        shade(t.panel_light, -0.18)
    };
    b.rrect(r.selector, s.r(1.5), Fill::Solid(sel));
}
