//! How the Slate cast paints (round 40, ADR 0122): the design rig's SVG pieces as the rig's
//! opaque fills (`docs/round40/design/dancer/src/svg.py`).
//!
//! - **Lines** are straight bars with round ends and round joins ([`bars_into`], one fill:
//!   a capsule per segment, all wound the rig's way) or smooth ribbons
//!   (`rig::draw::ribbon_into`); never strokes.
//! - **The cut**: a piece in front is separated from the one behind by a hairline of the
//!   room's colour — the piece grown by the cut and filled in the room first (the rig's
//!   outline-as-a-fill, without the nudge to the lower right).
//! - **Rounded rectangles** ([`rrect_into`]): a key, a cabinet, a sneaker, a visor slot.
//! - Every fill opaque except the contact shadow (one translucent oval).

use crate::render::{Canvas, StageInk};
use crate::rig::draw::{self as d, ellipse_into, Sink};
use crate::rig::P;
use tiny_skia::Color;

const KAPPA: f32 = 0.552_284_7;

/// A quarter-and-quarter round end: from `from` (= centre + n·r) round the tip
/// (centre + u·r) to centre − n·r.
fn round_end(s: &mut dyn Sink, centre: P, u: P, n: P, r: f32) {
    let k = KAPPA * r;
    let from = (centre.0 + n.0 * r, centre.1 + n.1 * r);
    let tip = (centre.0 + u.0 * r, centre.1 + u.1 * r);
    let to = (centre.0 - n.0 * r, centre.1 - n.1 * r);
    s.cubic_to(
        from.0 + u.0 * k,
        from.1 + u.1 * k,
        tip.0 + n.0 * k,
        tip.1 + n.1 * k,
        tip.0,
        tip.1,
    );
    s.cubic_to(
        tip.0 - n.0 * k,
        tip.1 - n.1 * k,
        to.0 + u.0 * k,
        to.1 + u.1 * k,
        to.0,
        to.1,
    );
}

/// A bar from `a` to `b`, `w0` wide at `a` and `w1` at `b`, round at both ends (a capsule
/// that may taper), grown by `g`; wound the rig's way (positive area, as `ellipse_into`).
pub fn capsule_into(s: &mut dyn Sink, a: P, b: P, w0: f32, w1: f32, g: f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    let (r0, r1) = (w0 * 0.5 + g, w1 * 0.5 + g);
    if len < 1e-6 {
        ellipse_into(s, a, r0.max(r1), r0.max(r1), 0.0);
        return;
    }
    let u = (dx / len, dy / len);
    let n = (u.1, -u.0);
    s.move_to(a.0 + n.0 * r0, a.1 + n.1 * r0);
    s.line_to(b.0 + n.0 * r1, b.1 + n.1 * r1);
    round_end(s, b, u, n, r1);
    s.line_to(a.0 - n.0 * r0, a.1 - n.1 * r0);
    round_end(s, a, (-u.0, -u.1), (-n.0, -n.1), r0);
    s.close();
}

/// Straight bars through `pts`, `width` wide, round at the ends and at every join (the
/// design's round-capped polyline), grown by `g`: one capsule a segment, one fill.
pub fn bars_into(s: &mut dyn Sink, pts: &[P], width: f32, g: f32) {
    if pts.len() == 1 {
        let r = width * 0.5 + g;
        ellipse_into(s, pts[0], r, r, 0.0);
    }
    for w in pts.windows(2) {
        capsule_into(s, w[0], w[1], width, width, g);
    }
}

/// A rounded rectangle centred on `c`, `w` × `h`, corner radius `r`, turned by `deg`,
/// grown by `g` (its corners grow with it).
#[allow(clippy::too_many_arguments)]
pub fn rrect_into(s: &mut dyn Sink, c: P, w: f32, h: f32, r: f32, deg: f32, g: f32) {
    let (w, h) = (w + 2.0 * g, h + 2.0 * g);
    let r = (r + g).min(w * 0.5).min(h * 0.5).max(0.0);
    let (x0, y0) = (-w * 0.5, -h * 0.5);
    let k = KAPPA * r;
    let (sn, cs) = cantus_dsp::math::sin_cos(deg * (std::f32::consts::PI / 180.0));
    let m = |x: f32, y: f32| (c.0 + x * cs - y * sn, c.1 + x * sn + y * cs);
    let mv = |s: &mut dyn Sink, x: f32, y: f32| {
        let p = m(x, y);
        s.move_to(p.0, p.1);
    };
    let ln = |s: &mut dyn Sink, x: f32, y: f32| {
        let p = m(x, y);
        s.line_to(p.0, p.1);
    };
    let cu = |s: &mut dyn Sink, a: P, b: P, e: P| {
        let (a, b, e) = (m(a.0, a.1), m(b.0, b.1), m(e.0, e.1));
        s.cubic_to(a.0, a.1, b.0, b.1, e.0, e.1);
    };
    let (x1, y1) = (x0 + w, y0 + h);
    mv(s, x0 + r, y0);
    ln(s, x1 - r, y0);
    cu(s, (x1 - r + k, y0), (x1, y0 + r - k), (x1, y0 + r));
    ln(s, x1, y1 - r);
    cu(s, (x1, y1 - r + k), (x1 - r + k, y1), (x1 - r, y1));
    ln(s, x0 + r, y1);
    cu(s, (x0 + r - k, y1), (x0, y1 - r + k), (x0, y1 - r));
    ln(s, x0, y0 + r);
    cu(s, (x0, y0 + r - k), (x0 + r - k, y0), (x0 + r, y0));
    s.close();
}

/// A circle, grown by `g`.
pub fn circle_into(s: &mut dyn Sink, c: P, r: f32, g: f32) {
    ellipse_into(s, c, r + g, r + g, 0.0);
}

/// A painter for one Slate character: the canvas, the theme's light and the cut.
pub struct Brush<'a, 'b> {
    pub c: &'a mut Canvas<'b>,
    pub ink: StageInk,
    /// The room's colour the cuts are drawn in.
    pub room: Color,
    /// How wide a cut is (the character's units).
    pub cut: f32,
}

impl<'a, 'b> Brush<'a, 'b> {
    pub fn new(c: &'a mut Canvas<'b>, cut: f32) -> Brush<'a, 'b> {
        let ink = c.ink;
        // The room as it is, never moved off a key: over a chroma key the cut is the key.
        let [r, g, b, a] = ink.room;
        let room = Color::from_rgba8(r, g, b, a);
        Brush { c, ink, room, cut }
    }

    /// A colour as it is (never turned by the hue).
    pub fn fixed(&self, rgba: [u8; 4]) -> Color {
        self.c.fixed(rgba)
    }

    /// A colour turned by the pose's hue (the gold accents).
    pub fn hued(&self, rgba: [u8; 4]) -> Color {
        self.c.color(rgba)
    }

    /// A shape (a function of its growth), its cut first when `cut`.
    pub fn piece(&mut self, shape: &dyn Fn(&mut dyn Sink, f32), color: Color, cut: bool) {
        if cut && self.cut > 0.0 {
            let (g, room) = (self.cut, self.room);
            d::paint(self.c, &|s, _| shape(s, g), room);
        }
        d::paint(self.c, &|s, _| shape(s, 0.0), color);
    }

    /// A shape filled by the even-odd rule (nested rings), no cut.
    pub fn piece_even_odd(&mut self, shape: &dyn Fn(&mut dyn Sink), color: Color) {
        d::paint_even_odd(self.c, &|s, _| shape(s), color);
    }

    /// Straight bars through `pts` (round ends and joins).
    pub fn bars(&mut self, pts: &[P], width: f32, color: Color, cut: bool) {
        self.piece(&|s, g| bars_into(s, pts, width, g), color, cut);
    }

    /// A smooth ribbon through `pts` (at most 16), round at both ends.
    pub fn curve(&mut self, pts: &[P], width: f32, color: Color, cut: bool) {
        self.piece(&|s, g| d::ribbon_into(s, pts, width + 2.0 * g), color, cut);
    }

    pub fn circle(&mut self, c: P, r: f32, color: Color, cut: bool) {
        self.piece(&|s, g| circle_into(s, c, r, g), color, cut);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rrect(&mut self, c: P, w: f32, h: f32, r: f32, deg: f32, color: Color, cut: bool) {
        self.piece(&|s, g| rrect_into(s, c, w, h, r, deg, g), color, cut);
    }

    /// A tapered bar `a` → `b` (`w0` → `w1` wide), round at both ends.
    pub fn tube(&mut self, a: P, b: P, w0: f32, w1: f32, color: Color, cut: bool) {
        self.piece(&|s, g| capsule_into(s, a, b, w0, w1, g), color, cut);
    }

    /// An ellipse ring (`width` its band), opaque: the beat ring, the bass's shockwave.
    pub fn ring(&mut self, c: P, rx: f32, ry: f32, width: f32, color: Color) {
        if width <= 0.0 {
            return;
        }
        let h = width * 0.5;
        d::paint_even_odd(
            self.c,
            &|s, _| {
                // A band of one width all round (as a stroke), not the ellipse scaled.
                ellipse_into(s, c, rx + h, ry + h, 0.0);
                ellipse_into(s, c, (rx - h).max(1e-4), (ry - h).max(1e-4), 0.0);
            },
            color,
        );
    }

    /// A small translucent oval (`opacity` of `rgba`): a puff of dust.
    pub fn blot(&mut self, c: P, rx: f32, ry: f32, rgba: [u8; 4], opacity: f32) {
        let col = self.c.fixed(d::alpha(rgba, opacity));
        d::paint(self.c, &|s, _| ellipse_into(s, c, rx, ry, 0.0), col);
    }

    /// The contact shadow: one translucent oval in the theme's shadow colour.
    pub fn shadow(&mut self, c: P, rx: f32, ry: f32, opacity: f32) {
        if !self.c.shadow {
            return;
        }
        let col = self.c.fixed(d::alpha(self.ink.shadow, opacity));
        d::paint(self.c, &|s, _| ellipse_into(s, c, rx, ry, 0.0), col);
    }
}

/// A convex polygon (at most 16 points) grown by `g` with round corners: the polygon and a
/// bar of width 2g round its edges (as a stroke with round joins), one shape.
pub fn poly_round_into(s: &mut dyn Sink, pts: &[P], g: f32) {
    d::polygon_into(s, pts);
    if g <= 0.0 {
        return;
    }
    let n = pts.len().min(16);
    for i in 0..n {
        capsule_into(s, pts[i], pts[(i + 1) % n], 2.0 * g, 2.0 * g, 0.0);
    }
}

/// `subject` clipped to the convex polygon `clip` (Sutherland–Hodgman, both at most 16
/// points, the clip wound either way): the part of a piece's light that stays inside it
/// (a two-value cel without a mask). Writes into `out`, returns how many points.
pub fn clip_convex(subject: &[P], clip: &[P], out: &mut [P; 32]) -> usize {
    let mut a = [(0.0f32, 0.0f32); 32];
    let mut na = subject.len().min(32);
    a[..na].copy_from_slice(&subject[..na]);
    let m = clip.len().min(16);
    // The clip's winding: which side of an edge is inside.
    let area: f32 = (0..m)
        .map(|i| {
            let (p, q) = (clip[i], clip[(i + 1) % m]);
            p.0 * q.1 - q.0 * p.1
        })
        .sum();
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };
    for i in 0..m {
        let (e0, e1) = (clip[i], clip[(i + 1) % m]);
        let side = |p: P| sign * ((e1.0 - e0.0) * (p.1 - e0.1) - (e1.1 - e0.1) * (p.0 - e0.0));
        let mut b = [(0.0f32, 0.0f32); 32];
        let mut nb = 0;
        for k in 0..na {
            let (p, q) = (a[k], a[(k + 1) % na]);
            let (sp, sq) = (side(p), side(q));
            if sp >= 0.0 && nb < 32 {
                b[nb] = p;
                nb += 1;
            }
            if (sp >= 0.0) != (sq >= 0.0) && nb < 32 {
                let t = sp / (sp - sq);
                b[nb] = (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
                nb += 1;
            }
        }
        a = b;
        na = nb;
        if na == 0 {
            break;
        }
    }
    out[..na].copy_from_slice(&a[..na]);
    na
}

/// A two-value cel (the design's `cel`: the shade, then the base moved toward the light by
/// `off` and kept inside the piece) for a closed smooth shape through `pts` (at most 24):
/// the base is the shape moved by `off` and drawn smaller about its middle by as much, so
/// it touches the piece's edge on the lit side and leaves a band of shade on the other —
/// no clip mask.
pub fn cel_blob(br: &mut Brush, pts: &[P], base: Color, shade: Color, off: P) {
    let n = pts.len().min(24);
    if n < 3 {
        return;
    }
    br.piece(&|s, _| d::blob_into(s, &pts[..n]), shade, false);
    let c = pts[..n].iter().fold((0.0, 0.0), |a, p| {
        (a.0 + p.0 / n as f32, a.1 + p.1 / n as f32)
    });
    let r = pts[..n]
        .iter()
        .map(|p| ((p.0 - c.0) * (p.0 - c.0) + (p.1 - c.1) * (p.1 - c.1)).sqrt())
        .sum::<f32>()
        / n as f32;
    let k = (1.0 - (off.0 * off.0 + off.1 * off.1).sqrt() / r.max(1e-4)).clamp(0.5, 1.0);
    let mut lit = [(0.0, 0.0); 24];
    for (o, p) in lit.iter_mut().zip(&pts[..n]) {
        *o = (c.0 + off.0 + k * (p.0 - c.0), c.1 + off.1 + k * (p.1 - c.1));
    }
    br.piece(&|s, _| d::blob_into(s, &lit[..n]), base, false);
}

/// A two-value cel for a tube through three points (`w` its widths): the shade, then the
/// base — the tube moved by half of `off` and narrowed by the part of `off` across each
/// segment: exactly the strip the moved tube shares with the piece.
pub fn cel_tube(br: &mut Brush, pts: [P; 3], w: [f32; 3], base: Color, shade: Color, off: P) {
    br.piece(
        &|s, _| {
            capsule_into(s, pts[0], pts[1], w[0], w[1], 0.0);
            capsule_into(s, pts[1], pts[2], w[1], w[2], 0.0);
        },
        shade,
        false,
    );
    let half = (off.0 * 0.5, off.1 * 0.5);
    let across = |a: P, b: P| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let ln = (dx * dx + dy * dy).sqrt().max(1e-6);
        ((-dy * off.0 + dx * off.1) / ln).abs()
    };
    let (c0, c1) = (across(pts[0], pts[1]), across(pts[1], pts[2]));
    let m = |p: P| (p.0 + half.0, p.1 + half.1);
    br.piece(
        &|s, _| {
            capsule_into(
                s,
                m(pts[0]),
                m(pts[1]),
                (w[0] - c0).max(0.0),
                (w[1] - c0).max(0.0),
                0.0,
            );
            capsule_into(
                s,
                m(pts[1]),
                m(pts[2]),
                (w[1] - c1).max(0.0),
                (w[2] - c1).max(0.0),
                0.0,
            );
        },
        base,
        false,
    );
}

/// A two-value cel for a rounded rectangle (with its cut when `cut`): the shade, then the
/// base — what the rectangle moved by `off` shares with it, a rectangle smaller by `off`'s
/// share along each of its sides.
#[allow(clippy::too_many_arguments)]
pub fn cel_rrect(
    br: &mut Brush,
    c: P,
    w: f32,
    h: f32,
    r: f32,
    deg: f32,
    base: Color,
    shade: Color,
    off: P,
    cut: bool,
) {
    br.rrect(c, w, h, r, deg, shade, cut);
    // `off` in the rectangle's own frame.
    let (s, co) = cantus_dsp::math::sin_cos(-deg * (std::f32::consts::PI / 180.0));
    let lo = (off.0 * co - off.1 * s, off.0 * s + off.1 * co);
    let (w2, h2) = ((w - lo.0.abs()).max(0.0), (h - lo.1.abs()).max(0.0));
    let r2 = r.min(w2 * 0.5).min(h2 * 0.5);
    br.rrect(
        (c.0 + off.0 * 0.5, c.1 + off.1 * 0.5),
        w2,
        h2,
        r2,
        deg,
        base,
        false,
    );
}
