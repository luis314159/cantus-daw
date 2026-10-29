//! Painting an engraved score (ADR 0075): the display list `cantus-score` makes, drawn
//! with egui's painter in the kit's colours — Bravura's glyphs set on their baselines, the
//! staff's lines and stems as square-ended strokes, beams as filled parallelograms, ties
//! as tapered crescents with a feathered edge. The PDF reads the same list.

use cantus_score::display::{Anchor, DisplayList, Item};
use cantus_score::glyphs::EM_IN_SPACES;
use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{pos2, Color32, Painter, Pos2, Rect, Shape, Stroke};

/// The colours a score is drawn in.
#[derive(Copy, Clone, Debug)]
pub struct ScoreInk {
    pub ink: Color32,
    /// A selected note's.
    pub selected: Color32,
}

/// Where the display list lands on the screen: its origin, and points per staff space.
#[derive(Copy, Clone, Debug)]
pub struct Placement {
    pub origin: Pos2,
    pub space: f32,
}

impl Placement {
    pub fn at(&self, x: f32, y: f32) -> Pos2 {
        pos2(
            self.origin.x + x * self.space,
            self.origin.y + y * self.space,
        )
    }

    /// The display-list point under a screen point.
    pub fn back(&self, p: Pos2) -> (f32, f32) {
        (
            (p.x - self.origin.x) / self.space,
            (p.y - self.origin.y) / self.space,
        )
    }
}

fn glyph_shape(painter: &Painter, c: char, size: f32, at: Pos2, color: Color32) -> Option<Shape> {
    let font = crate::design::music_font(size);
    let galley = painter.layout_no_wrap(c.to_string(), font, color);
    let row = galley.rows.first()?;
    let g = row.row.glyphs.first()?;
    let off = row.pos.to_vec2() + g.pos.to_vec2();
    Some(Shape::galley(at - off, galley, color))
}

fn text_shape(
    painter: &Painter,
    text: &str,
    face: cantus_score::text::Face,
    size: f32,
    at: Pos2,
    anchor: Anchor,
    color: Color32,
) -> Option<Shape> {
    let font = crate::design::score_text_font(face, size);
    let galley = painter.layout_no_wrap(text.to_string(), font, color);
    let row = galley.rows.first()?;
    let base = row.pos.y + row.row.glyphs.first().map_or(0.0, |g| g.pos.y);
    let w = galley.size().x;
    let x = match anchor {
        Anchor::Left => at.x,
        Anchor::Centre => at.x - w / 2.0,
        Anchor::Right => at.x - w,
    };
    Some(Shape::galley(pos2(x, at.y - base), galley, color))
}

/// A tie or slur: its middle line sampled, thickened toward its bow, with a one-pixel
/// feather on both edges (epaint fills only convex paths).
fn curve_mesh(
    p: [[f32; 2]; 4],
    end: f32,
    mid: f32,
    up: bool,
    pl: &Placement,
    color: Color32,
    ppp: f32,
) -> Mesh {
    const N: usize = 28;
    let pt = |t: f32| -> (f32, f32) {
        let u = 1.0 - t;
        let a = u * u * u;
        let b = 3.0 * u * u * t;
        let c = 3.0 * u * t * t;
        let d = t * t * t;
        (
            a * p[0][0] + b * p[1][0] + c * p[2][0] + d * p[3][0],
            a * p[0][1] + b * p[1][1] + c * p[2][1] + d * p[3][1],
        )
    };
    let feather = 1.0 / ppp.max(0.5);
    let clear = Color32::TRANSPARENT;
    let mut mesh = Mesh::default();
    let side = if up { -1.0 } else { 1.0 };
    for i in 0..=N {
        let t = i as f32 / N as f32;
        let (x, y) = pt(t);
        let (x2, y2) = pt((t + 0.01).min(1.0));
        let (x1, y1) = pt((t - 0.01).max(0.0));
        let (dx, dy) = (x2 - x1, y2 - y1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        // The normal pointing to the bow's outside.
        let (mut nx, mut ny) = (-dy / len, dx / len);
        if ny * side < 0.0 {
            nx = -nx;
            ny = -ny;
        }
        let w = end + (mid - end) * (4.0 * t * (1.0 - t));
        let inner = pl.at(x - nx * w / 2.0, y - ny * w / 2.0);
        let outer = pl.at(x + nx * w / 2.0, y + ny * w / 2.0);
        let f = egui::vec2(nx, ny) * feather;
        for (q, c) in [
            (inner - f, clear),
            (inner, color),
            (outer, color),
            (outer + f, clear),
        ] {
            mesh.vertices.push(Vertex {
                pos: q,
                uv: WHITE_UV,
                color: c,
            });
        }
        if i > 0 {
            let a = ((i - 1) * 4) as u32;
            let b = (i * 4) as u32;
            for k in 0..3u32 {
                mesh.indices.extend_from_slice(&[
                    a + k,
                    a + k + 1,
                    b + k,
                    a + k + 1,
                    b + k + 1,
                    b + k,
                ]);
            }
        }
    }
    mesh
}

/// Paint `list` with `pl`, notes `selected` says in the selection colour, skipping what
/// falls outside `clip`.
pub fn paint(
    painter: &Painter,
    list: &DisplayList,
    pl: &Placement,
    ink: &ScoreInk,
    selected: &dyn Fn(u64) -> bool,
    clip: Rect,
) {
    // The music font is the app's (its start-up registers it); a context that has not got
    // it yet (a tool, a test) takes it now and draws from the next pass.
    let ctx = painter.ctx();
    if !ctx.fonts(|f| {
        f.definitions()
            .families
            .contains_key(&crate::design::family_music())
    }) {
        crate::font_fallback::enable_music();
        crate::design::install_fonts(ctx);
        ctx.request_repaint();
        return;
    }
    let ppp = painter.ctx().pixels_per_point();
    let color_of = |owner: Option<u64>| match owner {
        Some(id) if selected(id) => ink.selected,
        _ => ink.ink,
    };
    let em = EM_IN_SPACES * pl.space;
    let mut shapes: Vec<Shape> = Vec::with_capacity(list.items.len());
    for it in &list.items {
        let b = it.bounds();
        let r = Rect::from_min_max(pl.at(b[0], b[1]), pl.at(b[2], b[3])).expand(pl.space * 2.0);
        if !r.intersects(clip) {
            continue;
        }
        match it {
            Item::Glyph {
                glyph,
                x,
                y,
                scale,
                owner,
            } => {
                if let Some(s) = glyph_shape(
                    painter,
                    glyph.codepoint(),
                    em * scale,
                    pl.at(*x, *y),
                    color_of(*owner),
                ) {
                    shapes.push(s);
                }
            }
            Item::Line {
                x0,
                y0,
                x1,
                y1,
                w,
                owner,
            } => {
                let (a, b) = (pl.at(*x0, *y0), pl.at(*x1, *y1));
                let width = (w * pl.space).max(0.6 / ppp.max(0.5));
                if (a.x - b.x).abs() < 1e-3 || (a.y - b.y).abs() < 1e-3 {
                    // Axis-aligned: a rectangle, square at both ends.
                    let rect = if (a.x - b.x).abs() < 1e-3 {
                        Rect::from_min_max(
                            pos2(a.x - width / 2.0, a.y.min(b.y)),
                            pos2(a.x + width / 2.0, a.y.max(b.y)),
                        )
                    } else {
                        Rect::from_min_max(
                            pos2(a.x.min(b.x), a.y - width / 2.0),
                            pos2(a.x.max(b.x), a.y + width / 2.0),
                        )
                    };
                    shapes.push(Shape::rect_filled(rect, 0.0, color_of(*owner)));
                } else {
                    shapes.push(Shape::line_segment(
                        [a, b],
                        Stroke::new(width, color_of(*owner)),
                    ));
                }
            }
            Item::Dashed {
                x0,
                y0,
                x1,
                w,
                dash,
                gap,
            } => {
                let mut x = *x0;
                while x < *x1 {
                    let e = (x + dash).min(*x1);
                    shapes.push(Shape::line_segment(
                        [pl.at(x, *y0), pl.at(e, *y0)],
                        Stroke::new(w * pl.space, ink.ink),
                    ));
                    x = e + gap;
                }
            }
            Item::Beam {
                x0,
                y0,
                x1,
                y1,
                h,
                owner,
            } => {
                let pts = vec![
                    pl.at(*x0, *y0),
                    pl.at(*x1, *y1),
                    pl.at(*x1, y1 + h),
                    pl.at(*x0, y0 + h),
                ];
                shapes.push(Shape::convex_polygon(pts, color_of(*owner), Stroke::NONE));
            }
            Item::Curve {
                p,
                end,
                mid,
                up,
                owner,
            } => {
                shapes.push(Shape::mesh(curve_mesh(
                    *p,
                    *end,
                    *mid,
                    *up,
                    pl,
                    color_of(*owner),
                    ppp,
                )));
            }
            Item::Text {
                text,
                x,
                y,
                size,
                face,
                anchor,
            } => {
                if let Some(s) = text_shape(
                    painter,
                    text,
                    *face,
                    size * pl.space,
                    pl.at(*x, *y),
                    *anchor,
                    ink.ink,
                ) {
                    shapes.push(s);
                }
            }
            Item::Brace { x, y0, y1 } => {
                // Stretched taller far more than wider: filled from its outline, the
                // outline stroked a hair to smooth its edge.
                let tris = cantus_score::outline::brace(*x, *y0, *y1);
                let mut mesh = Mesh::default();
                for p in tris {
                    mesh.indices.push(mesh.vertices.len() as u32);
                    mesh.vertices.push(Vertex {
                        pos: pl.at(p[0], p[1]),
                        uv: WHITE_UV,
                        color: ink.ink,
                    });
                }
                shapes.push(Shape::mesh(mesh));
                let h = y1 - y0;
                let (sx, sy) = (
                    cantus_score::outline::brace_width_scale(h),
                    h / EM_IN_SPACES,
                );
                for c in cantus_score::outline::contours(cantus_score::Glyph::Brace) {
                    let pts: Vec<Pos2> = c
                        .iter()
                        .map(|q| pl.at(x + q[0] * sx, y1 + q[1] * sy))
                        .collect();
                    shapes.push(Shape::closed_line(
                        pts,
                        Stroke::new(0.6 / ppp.max(0.5), ink.ink),
                    ));
                }
            }
        }
    }
    painter.extend(shapes);
}
