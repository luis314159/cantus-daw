//! A glyph's outline as polygons, and triangles to fill them: for the one glyph the
//! engraver stretches unevenly (a brace grows taller far more than it grows wider, as
//! Gould and every engraving program draw it), which a font renderer cannot do.

use crate::glyphs::{Glyph, BRAVURA};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::MetadataProvider;

struct Pen {
    contours: Vec<Vec<[f32; 2]>>,
    at: [f32; 2],
}

const STEPS: usize = 12;

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.contours.push(vec![[x, -y]]);
        self.at = [x, -y];
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.push([x, -y]);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (p0, p1, p2) = (self.at, [cx, -cy], [x, -y]);
        for i in 1..=STEPS {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            self.push([
                u * u * p0[0] + 2.0 * u * t * p1[0] + t * t * p2[0],
                u * u * p0[1] + 2.0 * u * t * p1[1] + t * t * p2[1],
            ]);
        }
    }
    fn curve_to(&mut self, c0x: f32, c0y: f32, c1x: f32, c1y: f32, x: f32, y: f32) {
        let (p0, p1, p2, p3) = (self.at, [c0x, -c0y], [c1x, -c1y], [x, -y]);
        for i in 1..=STEPS {
            let t = i as f32 / STEPS as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            self.push([
                a * p0[0] + b * p1[0] + c * p2[0] + d * p3[0],
                a * p0[1] + b * p1[1] + c * p2[1] + d * p3[1],
            ]);
        }
    }
    fn close(&mut self) {}
}

impl Pen {
    fn push(&mut self, p: [f32; 2]) {
        if let Some(c) = self.contours.last_mut() {
            c.push(p);
        }
        self.at = p;
    }
}

/// A glyph's contours in staff spaces (y down, origin at the glyph's), curves flattened.
pub fn contours(glyph: Glyph) -> Vec<Vec<[f32; 2]>> {
    char_contours(BRAVURA, glyph.codepoint(), crate::glyphs::EM_IN_SPACES)
}

/// A character's contours in `font` at `em` units per em (y down), curves flattened.
pub fn char_contours(font: &[u8], c: char, em: f32) -> Vec<Vec<[f32; 2]>> {
    let Ok(font) = skrifa::FontRef::new(font) else {
        return Vec::new();
    };
    let Some(gid) = font.charmap().map(c) else {
        return Vec::new();
    };
    let outlines = font.outline_glyphs();
    let Some(g) = outlines.get(gid) else {
        return Vec::new();
    };
    let mut pen = Pen {
        contours: Vec::new(),
        at: [0.0, 0.0],
    };
    let settings = DrawSettings::unhinted(Size::new(em), LocationRef::default());
    if g.draw(settings, &mut pen).is_err() {
        return Vec::new();
    }
    for c in pen.contours.iter_mut() {
        if c.len() > 1 && c.first() == c.last() {
            c.pop();
        }
    }
    pen.contours.retain(|c| c.len() >= 3);
    pen.contours
}

fn area(p: &[[f32; 2]]) -> f32 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let (x0, y0) = (p[i][0], p[i][1]);
        let (x1, y1) = (p[(i + 1) % p.len()][0], p[(i + 1) % p.len()][1]);
        a += x0 * y1 - x1 * y0;
    }
    a / 2.0
}

fn inside(p: [f32; 2], a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> bool {
    let s = |p: [f32; 2], q: [f32; 2], r: [f32; 2]| {
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0])
    };
    let (d1, d2, d3) = (s(a, b, p), s(b, c, p), s(c, a, p));
    let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(neg && pos)
}

/// Triangles filling one simple polygon (ear clipping), three points each.
pub fn triangulate(poly: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut idx: Vec<usize> = (0..poly.len()).collect();
    if area(poly) < 0.0 {
        idx.reverse();
    }
    let mut out = Vec::with_capacity(poly.len().saturating_sub(2) * 3);
    let cross = |a: [f32; 2], b: [f32; 2], c: [f32; 2]| {
        (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    };
    let mut guard = 0;
    while idx.len() > 3 && guard < 100_000 {
        guard += 1;
        let n = idx.len();
        let mut clipped = false;
        for i in 0..n {
            let (ia, ib, ic) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (poly[ia], poly[ib], poly[ic]);
            if cross(a, b, c) <= 0.0 {
                continue;
            }
            let ear = idx
                .iter()
                .all(|&k| k == ia || k == ib || k == ic || !inside(poly[k], a, b, c));
            if ear {
                out.extend_from_slice(&[a, b, c]);
                idx.remove(i);
                clipped = true;
                break;
            }
        }
        if !clipped {
            // Degenerate (collinear runs): drop a point and go on.
            idx.remove(0);
        }
    }
    if idx.len() == 3 {
        out.extend_from_slice(&[poly[idx[0]], poly[idx[1]], poly[idx[2]]]);
    }
    out
}

/// A brace from `y0` to `y1` with its left edge at `x`: its triangles, the glyph stretched
/// to the height and widened only as the square root of that.
pub fn brace(x: f32, y0: f32, y1: f32) -> Vec<[f32; 2]> {
    let h = (y1 - y0).max(0.1);
    let sy = h / crate::glyphs::EM_IN_SPACES;
    let sx = brace_width_scale(h);
    let mut out = Vec::new();
    for c in contours(Glyph::Brace) {
        for p in triangulate(&c) {
            out.push([x + p[0] * sx, y1 + p[1] * sy]);
        }
    }
    out
}

/// How much wider than the glyph a brace `h` spaces tall is drawn.
pub fn brace_width_scale(h: f32) -> f32 {
    (h / crate::glyphs::EM_IN_SPACES).sqrt() * 1.6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_brace_has_an_outline_and_fills_it() {
        let c = contours(Glyph::Brace);
        assert!(!c.is_empty());
        let total: f32 = c.iter().map(|p| area(p).abs()).sum();
        let tris = brace(0.0, 0.0, 4.0);
        assert!(tris.len() >= 9 && tris.len().is_multiple_of(3));
        let filled: f32 = tris.chunks(3).map(|t| area(t).abs()).sum::<f32>();
        let scale = brace_width_scale(4.0);
        assert!(
            (filled - total * scale).abs() < 0.05 * total * scale,
            "{filled} vs {}",
            total * scale
        );
    }
}
