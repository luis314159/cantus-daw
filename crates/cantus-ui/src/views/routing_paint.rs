//! The matrix's painters: the visible cells into one brush, the frozen row and column
//! headers, the band between strips and tracks, and the corner.
//!
//! **What a cell shows** (design §5, the kinds told apart by shape, not only colour):
//! an empty cell is a sunken well; a refused one is hatched (a loop) or faded (no room);
//! the strip's own cell is a diagonal with no well; an output is a filled dot in the
//! source's colour; a send is a ring with its level as an arc — the ring in the sends'
//! hue when post-fader and in the pre-fader hue when pre (`kit::tap::function`, with a
//! mark in the ring's open foot), faded
//! when switched off, with the doubled-signal mark when the destination hears the row
//! twice; an output and a send to the same strip, the dot inside the ring; a key, the
//! key mark and a corner in the dynamics hue. A track row is a radio: its strip's dot, a
//! hollow ring elsewhere, and a diamond in the source hue where an insert hears its notes.

use super::{Col, Geometry, Gesture, Grid, Refused, RoutingView, Row, NUM_W};
use crate::kit::color::{on_fill, safe_track, shade, with_alpha, FillSize};
use crate::kit::knob::{START, SWEEP};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cable, focus_ring, tap, Brush, Fill, Function, Style, Tok};
use egui::{pos2, Align2, Color32, Rect};

/// The tokens the body paints with (for inspect mode).
const BODY_TOKENS: &[Tok] = &[
    Tok::Ink,
    Tok::Well,
    Tok::Grid,
    Tok::GridBar,
    Tok::GridStrong,
    Tok::KnobTrack,
    Tok::TextDim,
    Tok::Panel,
    Tok::Highlight,
    Tok::Focus,
];
const HEAD_TOKENS: &[Tok] = &[
    Tok::Panel,
    Tok::HeaderCell,
    Tok::Text,
    Tok::TextDim,
    Tok::GridBar,
    Tok::Selection,
];

/// A row's or column's colour on a chip, clamped to the safe band.
fn chip(c: Option<cantus_core::Color>, s: &Style) -> Option<Color32> {
    c.map(|c| safe_track(crate::theme::track_color(c), s.t.dark, FillSize::Small))
}

/// The cells, the dock dividers, the band, the crosshair and the focus ring.
pub fn body(
    b: &mut Brush,
    v: &RoutingView,
    geo: &Geometry,
    hover: Option<(usize, usize)>,
    ring: Option<(usize, usize)>,
    s: &Style,
) {
    let g = &v.grid;
    b.register(geo.body, "routing_matrix", BODY_TOKENS, &[]);
    // The panel around the grid, the deep ground under it.
    b.rect(b.snap_rect(geo.body), Fill::Solid(s.t.panel));
    let content = geo.content();
    let grid = Rect::from_min_size(
        pos2(geo.col_left(0), geo.row_top(0)),
        egui::vec2(content.x, content.y),
    )
    .intersect(geo.body);
    b.rect(b.snap_rect(grid), Fill::Solid(s.t.ink));
    let (rows, cols) = geo.visible();
    let px = b.px();
    let copy = match v.gesture {
        Some(Gesture::Copy { col, start, end }) => Some((col, start.min(end), start.max(end))),
        _ => None,
    };
    for r in rows.clone() {
        let color = chip(g.row_heads[r].color, s).unwrap_or(s.t.text_dim);
        for c in cols.clone() {
            let rect = geo.cell_rect(r, c);
            let cross = hover.is_some_and(|(hr, hc)| hr == r || hc == c);
            let preview = copy.is_some_and(|(cc, a, z)| cc == c && r >= a && r <= z);
            cell(b, g, r, c, rect, color, cross, preview, s);
        }
    }
    // Dock dividers: where the dock changes, and after the master.
    for c in cols.clone() {
        if g.divider_before(c) {
            let x = b.snap_mid(geo.col_left(c));
            b.vline(x, geo.body.top(), geo.body.bottom(), px * 2.0, s.t.grid_bar);
        }
    }
    if geo.band > 0.0 {
        let band = geo.band_rect();
        let band = Rect::from_min_max(pos2(geo.body.left(), band.top()), band.max);
        b.rect(b.snap_rect(band), Fill::Solid(s.t.panel));
        b.hline(
            band.left(),
            band.right(),
            b.snap_mid(band.top()),
            px,
            s.t.grid_bar,
        );
        b.hline(
            band.left(),
            band.right(),
            b.snap_mid(band.bottom()),
            px,
            s.t.grid_bar,
        );
    }
    if let Some((r, c)) = ring {
        let rect = geo.cell_rect(r, c).shrink(px);
        focus_ring(b, rect, s.r(2.0), s);
    }
}

/// One cell.
#[allow(clippy::too_many_arguments)]
fn cell(
    b: &mut Brush,
    g: &Grid,
    r: usize,
    c: usize,
    rect: Rect,
    color: Color32,
    cross: bool,
    preview: bool,
    s: &Style,
) {
    let x = g.cell(r, c);
    let px = b.px();
    let inner = b.snap_rect(rect.shrink(px));
    let radius = s.r(2.0);
    let center = rect.center();
    let size = rect.width();
    if x.itself {
        b.segment(
            inner.left_bottom(),
            inner.right_top(),
            px,
            s.t.grid_strong,
            false,
        );
        return;
    }
    let mut well = if cross {
        shade(s.t.well, if s.t.dark { 0.05 } else { -0.04 })
    } else {
        s.t.well
    };
    if x.refused == Some(Refused::NoRoom) {
        well = with_alpha(well, 0.45);
    }
    b.rrect(inner, radius, Fill::Solid(well));
    if x.refused == Some(Refused::Loop) {
        // Hatched: refused, with the reason in the hint bar.
        let hatch = with_alpha(s.t.text_dim, 0.3);
        let w = inner.width();
        for k in [0.25f32, 0.5, 0.75] {
            let d = w * k * 2.0;
            let a = pos2(inner.left() + (d - w).max(0.0), inner.bottom() - d.min(w));
            let z = pos2(inner.left() + d.min(w), inner.bottom() - (d - w).max(0.0));
            b.segment(a, z, px, hatch, false);
        }
    }
    let is_track = matches!(g.rows[r], Row::Track(_));
    if is_track {
        if x.output {
            b.dot(center, size * 0.26, color);
        } else {
            b.circle_stroke(center, size * 0.2, px, with_alpha(s.t.grid_strong, 0.9));
        }
        if x.notes > 0 {
            note_mark(b, inner, s);
        }
    } else {
        if let Some(m) = x.send {
            let alpha = if m.enabled { 1.0 } else { 0.35 };
            let rr = size * 0.34;
            let w = (size * 0.11).max(px * 1.5);
            let track = match m.tap {
                cantus_core::SendTap::Post => s.t.knob_track,
                cantus_core::SendTap::Pre => s.t.hue(tap::function(m.tap)),
            };
            b.arc(
                center,
                rr,
                START,
                START + SWEEP,
                w,
                with_alpha(track, alpha),
                true,
            );
            let level = m.level.clamp(0.0, 1.0);
            if level > 0.0 {
                b.arc(
                    center,
                    rr,
                    START,
                    START + SWEEP * level,
                    w,
                    with_alpha(color, alpha),
                    true,
                );
            }
            if m.tap == cantus_core::SendTap::Pre {
                // Pre-fader: a mark in the pre-fader hue in the ring's open foot, so the
                // tap reads at a glance and not only by the ring's tint.
                let foot = pos2(center.x, center.y + rr);
                b.dot(foot, (size * 0.12).max(px * 2.0), with_alpha(track, alpha));
            }
            if m.doubled {
                let at = pos2(inner.right() - size * 0.18, inner.top() + size * 0.18);
                cable::doubled_mark(b, s, at, size * 0.4);
            }
        }
        if x.output {
            let r0 = if x.send.is_some() { 0.13 } else { 0.26 };
            b.dot(center, size * r0, color);
        }
        if x.keys > 0 {
            let dyn_hue = s.t.hue(Function::Dynamics);
            let k = size * 0.32;
            b.polygon(
                &[
                    inner.right_bottom(),
                    pos2(inner.right() - k, inner.bottom()),
                    pos2(inner.right(), inner.bottom() - k),
                ],
                dyn_hue,
            );
            if x.send.is_none() && !x.output {
                cable::key_mark(b, s, center, size * 0.62, dyn_hue);
            }
        }
    }
    if preview {
        b.rrect_stroke(inner, radius, px * 1.5, s.t.highlight);
    }
}

/// A small diamond in the source hue at a cell's lower left: an insert of the column
/// hears the row's notes.
fn note_mark(b: &mut Brush, inner: Rect, s: &Style) {
    let k = inner.width() * 0.16;
    let c = pos2(inner.left() + k * 1.4, inner.bottom() - k * 1.4);
    b.polygon(
        &[
            pos2(c.x, c.y - k),
            pos2(c.x + k, c.y),
            pos2(c.x, c.y + k),
            pos2(c.x - k, c.y),
        ],
        s.t.hue(Function::Source),
    );
}

/// The row headers: per row its number cell in its colour and its name; the band's
/// caption between the strips and the tracks.
pub fn row_heads(
    b: &mut Brush,
    g: &Grid,
    geo: &Geometry,
    hover: Option<(usize, usize)>,
    focus: Option<(usize, usize)>,
    s: &Style,
) {
    b.register(geo.row_head, "routing_rows", HEAD_TOKENS, &[]);
    b.rect(b.snap_rect(geo.row_head), Fill::Solid(s.t.panel));
    let px = b.px();
    let (rows, _) = geo.visible();
    let num_w = s.s(NUM_W);
    for r in rows {
        let top = geo.row_top(r);
        let row = Rect::from_min_max(
            pos2(geo.row_head.left(), top),
            pos2(geo.row_head.right(), top + geo.cell),
        );
        let lit = hover.is_some_and(|(h, _)| h == r) || focus.is_some_and(|(f, _)| f == r);
        let head = &g.row_heads[r];
        let num = b.snap_rect(Rect::from_min_max(
            pos2(row.left() + s.s(4.0), row.top() + px),
            pos2(row.left() + s.s(4.0) + num_w, row.bottom() - px),
        ));
        let fill = chip(head.color, s).unwrap_or(s.t.header_cell);
        b.rrect(num, s.r(2.0), Fill::Solid(fill));
        let mut digits = [0u8; 8];
        let t = Text::new(number(head.number, &mut digits), Role::Label, on_fill(fill))
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, num, &t, s.tk());
        let name = Rect::from_min_max(pos2(num.right() + s.s(6.0), row.top()), row.max);
        let ink = if lit { s.t.text } else { s.t.text_dim };
        let t = Text::new(&head.name, Role::Body, ink)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::Middle));
        let _ = text::paint(b, name.shrink2(egui::vec2(s.s(2.0), 0.0)), &t, s.tk());
        if lit {
            b.rect(
                b.snap_rect(Rect::from_min_max(
                    pos2(row.right() - s.s(3.0), row.top()),
                    row.max,
                )),
                Fill::Solid(s.t.selection),
            );
        }
    }
    if geo.band > 0.0 {
        let band = geo.band_rect();
        let band = Rect::from_min_max(band.min, pos2(geo.row_head.right(), band.bottom()));
        b.rect(b.snap_rect(band), Fill::Solid(shade(s.t.panel, -0.04)));
        b.hline(
            band.left(),
            band.right(),
            b.snap_mid(band.top()),
            px,
            s.t.grid_bar,
        );
        b.hline(
            band.left(),
            band.right(),
            b.snap_mid(band.bottom()),
            px,
            s.t.grid_bar,
        );
        let t = Text::key("matrix.tracks", Role::Section, s.t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, band.shrink2(egui::vec2(s.s(8.0), 0.0)), &t, s.tk());
    }
    b.vline(
        b.snap_mid(geo.row_head.right() - px * 0.5),
        geo.row_head.top(),
        geo.row_head.bottom(),
        px,
        s.t.grid_bar,
    );
}

/// The column headers: per column its name on its side and its number chip under it
/// (the master's is "M"), dividers where the dock changes.
pub fn col_heads(
    b: &mut Brush,
    g: &Grid,
    geo: &Geometry,
    hover: Option<(usize, usize)>,
    focus: Option<(usize, usize)>,
    s: &Style,
) {
    b.register(geo.col_head, "routing_cols", HEAD_TOKENS, &[]);
    b.rect(b.snap_rect(geo.col_head), Fill::Solid(s.t.panel));
    let px = b.px();
    let (_, cols) = geo.visible();
    let chip_h = s.s(16.0);
    for c in cols {
        let left = geo.col_left(c);
        let col = Rect::from_min_max(
            pos2(left, geo.col_head.top()),
            pos2(left + geo.cell, geo.col_head.bottom()),
        );
        let lit = hover.is_some_and(|(_, h)| h == c) || focus.is_some_and(|(_, f)| f == c);
        let head = &g.col_heads[c];
        let num = b.snap_rect(Rect::from_min_max(
            pos2(col.left() + px, col.bottom() - chip_h - s.s(2.0)),
            pos2(col.right() - px, col.bottom() - s.s(2.0)),
        ));
        let fill = match g.cols[c] {
            Col::Master => s.t.header_cell,
            Col::Strip(_) => chip(head.color, s).unwrap_or(s.t.header_cell),
        };
        b.rrect(num, s.r(2.0), Fill::Solid(fill));
        let mut digits = [0u8; 8];
        let label = match g.cols[c] {
            Col::Master => crate::views::clips_chrome::word("matrix.master_short"),
            Col::Strip(_) => number(head.number, &mut digits),
        };
        let t = Text::new(label, Role::Micro, on_fill(fill)).policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, num, &t, s.tk());
        let name = Rect::from_min_max(
            pos2(col.left(), col.top() + s.s(4.0)),
            pos2(col.right(), num.top() - s.s(4.0)),
        );
        let ink = if lit { s.t.text } else { s.t.text_dim };
        let t = Text::new(&head.name, Role::Label, ink)
            .align(Align2::CENTER_BOTTOM)
            .policy(Policy::Rotate90(Elide::Middle));
        let _ = text::paint(b, name, &t, s.tk());
        if lit {
            b.rect(
                b.snap_rect(Rect::from_min_max(
                    pos2(col.left(), col.bottom() - s.s(2.0)),
                    col.max,
                )),
                Fill::Solid(s.t.selection),
            );
        }
        if g.divider_before(c) {
            let x = b.snap_mid(left);
            b.vline(
                x,
                geo.col_head.top(),
                geo.col_head.bottom(),
                px * 2.0,
                s.t.grid_bar,
            );
        }
    }
    b.hline(
        geo.col_head.left(),
        geo.col_head.right(),
        b.snap_mid(geo.col_head.bottom() - px * 0.5),
        px,
        s.t.grid_bar,
    );
}

/// The corner over the row headers: which way the routes run ("From ↓ · To →").
pub fn corner(b: &mut Brush, rect: Rect, s: &Style) {
    b.register(
        rect,
        "routing_corner",
        &[Tok::Panel, Tok::TextDim, Tok::GridBar],
        &[],
    );
    b.rect(b.snap_rect(rect), Fill::Solid(s.t.panel));
    let pad = s.s(8.0);
    let to = Rect::from_min_max(
        pos2(rect.left() + pad, rect.top() + pad),
        pos2(rect.right() - pad, rect.top() + pad + s.s(16.0)),
    );
    let from = Rect::from_min_max(
        pos2(rect.left() + pad, rect.bottom() - pad - s.s(16.0)),
        pos2(rect.right() - pad, rect.bottom() - pad),
    );
    let t = Text::key("matrix.to", Role::Section, s.t.text_dim)
        .align(Align2::RIGHT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, to, &t, s.tk());
    let t = Text::key("matrix.from", Role::Section, s.t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, from, &t, s.tk());
    let px = b.px();
    b.hline(
        rect.left(),
        rect.right(),
        b.snap_mid(rect.bottom() - px * 0.5),
        px,
        s.t.grid_bar,
    );
    b.vline(
        b.snap_mid(rect.right() - px * 0.5),
        rect.top(),
        rect.bottom(),
        px,
        s.t.grid_bar,
    );
}

/// `n` in decimal, into `buf`, without allocating.
fn number(n: usize, buf: &mut [u8; 8]) -> &str {
    let mut i = buf.len();
    let mut n = n;
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 || i == 0 {
            break;
        }
    }
    std::str::from_utf8(&buf[i..]).unwrap_or("")
}
