//! r42:arcus-hands — the realistic figures' live arms drawn with a depth per sample (the
//! coordinator on sheets hands-1 and hands-2: black notches, stair-steps and dark slivers
//! between and along the fingers, three sheets running — structural, not stray triangles).
//! Painted in depth order triangle by triangle, curled fingers that pass in front of each
//! other were drawn by their middles' depth: one finger's far side over the next one's near
//! side, cracks between patches, the fingerboard showing through; and a triangle's inside
//! edges were a one-bit fill. Here each arm's triangles (and High's patches) are filled into
//! a small buffer over the arm's own screen rectangle, several samples a pixel (the bake's:
//! three a side at 100 %, two at 200 %), each sample keeping the nearest surface; the
//! samples are averaged into the pixel (the arm's outline antialiased by its coverage) and
//! the picture is drawn as one texture. The buffers are kept (a thread's scratch); at rest,
//! when nothing moved, the last picture is drawn again without filling or uploading.

use crate::scene::geo::V3;
use egui::{Color32, Pos2, Rect};

/// r42:arcus-hands, pass 3 — a triangle's corners in the room and the test each sample's
/// place there must pass to be filled (`Raster::tri_kept`).
type Kept<'a> = ([V3; 3], &'a dyn Fn(V3) -> bool);
/// r42:arcus-hands, pass 5 — a triangle's corners at rest and the skin's fine grain there (a
/// luminance factor per sample: `Raster::tri_grain`).
type Grain<'a> = ([V3; 3], &'a dyn Fn(V3) -> f32);

/// The samples over one arm's rectangle (physical pixels from `x0`, `y0`).
#[derive(Default)]
pub struct Raster {
    pub x0: i32,
    pub y0: i32,
    pub w: usize,
    pub h: usize,
    pub ss: usize,
    pub ppp: f32,
    depth: Vec<f32>,
    col: Vec<[f32; 4]>,
    out: Vec<u8>,
}

impl Raster {
    /// Start over `rect` (points) at `ppp` pixels a point, `ss` samples a pixel side.
    pub fn begin(&mut self, rect: Rect, ppp: f32, ss: usize) {
        self.ppp = ppp;
        self.ss = ss.max(1);
        self.x0 = (rect.min.x * ppp).floor() as i32;
        self.y0 = (rect.min.y * ppp).floor() as i32;
        let x1 = (rect.max.x * ppp).ceil() as i32;
        let y1 = (rect.max.y * ppp).ceil() as i32;
        self.w = (x1 - self.x0).max(1) as usize;
        self.h = (y1 - self.y0).max(1) as usize;
        let n = self.w * self.h * self.ss * self.ss;
        self.depth.clear();
        self.depth.resize(n, f32::MAX);
        self.col.clear();
        self.col.resize(n, [0.0; 4]);
    }

    /// The rectangle (points) the picture covers.
    pub fn rect(&self) -> Rect {
        let k = 1.0 / self.ppp;
        Rect::from_min_size(
            Pos2::new(self.x0 as f32 * k, self.y0 as f32 * k),
            egui::vec2(self.w as f32 * k, self.h as f32 * k),
        )
    }

    /// A triangle (points), its corners' depths and colours: each sample it covers takes
    /// it where it is nearer than what the sample holds (`blend`: laid over instead, where
    /// it is no deeper than the surface there by `slack` — a crease on its skin).
    pub fn tri(&mut self, p: [Pos2; 3], z: [f32; 3], c: [Color32; 3], blend: Option<f32>) {
        self.fill(p, z, c, blend, None, None);
    }

    /// r42:arcus-hands, pass 5 — [`tri`](Self::tri) (or [`tri_kept`](Self::tri_kept) with
    /// `kept`) with each sample's colour scaled by `grain` at its place at rest (`rest`, the
    /// corners'): the skin's pores and fine lines, the knuckles' wrinkles, finer than the
    /// patches' points.
    pub fn tri_grain(
        &mut self,
        p: [Pos2; 3],
        z: [f32; 3],
        c: [Color32; 3],
        kept: Option<Kept<'_>>,
        rest: [V3; 3],
        grain: &dyn Fn(V3) -> f32,
    ) {
        self.fill(p, z, c, None, kept, Some((rest, grain)));
    }

    /// r42:arcus-hands, pass 3 — [`tri`](Self::tri) where only the samples whose place in
    /// the room (`room`: the corners', taken across the triangle as on screen) `keep` holds
    /// are filled: a hand patch across the neck's edge cut along the edge to the sample, not
    /// along its small triangles (the stair-stepped chips at the hand's edge at 400 %).
    pub fn tri_kept(
        &mut self,
        p: [Pos2; 3],
        z: [f32; 3],
        c: [Color32; 3],
        room: [V3; 3],
        keep: &dyn Fn(V3) -> bool,
    ) {
        self.fill(p, z, c, None, Some((room, keep)), None);
    }

    fn fill(
        &mut self,
        p: [Pos2; 3],
        z: [f32; 3],
        c: [Color32; 3],
        blend: Option<f32>,
        kept: Option<Kept<'_>>,
        grain: Option<Grain<'_>>,
    ) {
        let ss = self.ss as f32;
        let q = p.map(|p| {
            (
                (p.x * self.ppp - self.x0 as f32) * ss,
                (p.y * self.ppp - self.y0 as f32) * ss,
            )
        });
        let area = (q[1].0 - q[0].0) * (q[2].1 - q[0].1) - (q[2].0 - q[0].0) * (q[1].1 - q[0].1);
        if area.abs() < 1e-9 {
            return;
        }
        let (sw, sh) = (self.w * self.ss, self.h * self.ss);
        let minx = q
            .iter()
            .map(|v| v.0)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as usize;
        let maxx = (q
            .iter()
            .map(|v| v.0)
            .fold(f32::MIN, f32::max)
            .ceil()
            .max(0.0) as usize)
            .min(sw);
        let miny = q
            .iter()
            .map(|v| v.1)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as usize;
        let maxy = (q
            .iter()
            .map(|v| v.1)
            .fold(f32::MIN, f32::max)
            .ceil()
            .max(0.0) as usize)
            .min(sh);
        if minx >= maxx || miny >= maxy {
            return;
        }
        let cf = c.map(|c| [c.r() as f32, c.g() as f32, c.b() as f32, c.a() as f32]);
        let inv = 1.0 / area;
        for y in miny..maxy {
            let py = y as f32 + 0.5;
            for x in minx..maxx {
                let px = x as f32 + 0.5;
                let w0 = ((q[1].0 - px) * (q[2].1 - py) - (q[2].0 - px) * (q[1].1 - py)) * inv;
                let w1 = ((q[2].0 - px) * (q[0].1 - py) - (q[0].0 - px) * (q[2].1 - py)) * inv;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-5 || w1 < -1e-5 || w2 < -1e-5 {
                    continue;
                }
                if let Some((room, keep)) = &kept {
                    if !keep(room[0] * w0 + room[1] * w1 + room[2] * w2) {
                        continue;
                    }
                }
                let d = z[0] * w0 + z[1] * w1 + z[2] * w2;
                let i = y * sw + x;
                let mut rgba =
                    [0, 1, 2, 3].map(|ch| cf[0][ch] * w0 + cf[1][ch] * w1 + cf[2][ch] * w2);
                if let Some((rest, g)) = &grain {
                    if d < self.depth[i] {
                        let k = g(rest[0] * w0 + rest[1] * w1 + rest[2] * w2);
                        for v in rgba.iter_mut().take(3) {
                            *v = (*v * k).min(255.0);
                        }
                    }
                }
                match blend {
                    None => {
                        if d < self.depth[i] {
                            self.depth[i] = d;
                            self.col[i] = rgba;
                        }
                    }
                    Some(slack) => {
                        if d <= self.depth[i] + slack && self.col[i][3] > 0.0 {
                            let a = rgba[3] / 255.0;
                            let o = &mut self.col[i];
                            for ch in 0..3 {
                                o[ch] = rgba[ch] + o[ch] * (1.0 - a);
                            }
                        }
                    }
                }
            }
        }
    }

    /// The samples averaged into the pixels (premultiplied RGBA bytes).
    pub fn resolve(&mut self) -> egui::ColorImage {
        let ss = self.ss;
        let n = (ss * ss) as f32;
        let sw = self.w * ss;
        self.out.clear();
        self.out.reserve(self.w * self.h * 4);
        for y in 0..self.h {
            for x in 0..self.w {
                let mut acc = [0.0f32; 4];
                for j in 0..ss {
                    let row = (y * ss + j) * sw + x * ss;
                    for s in &self.col[row..row + ss] {
                        for ch in 0..4 {
                            acc[ch] += s[ch];
                        }
                    }
                }
                for a in acc {
                    self.out.push((a / n).round().clamp(0.0, 255.0) as u8);
                }
            }
        }
        egui::ColorImage::from_rgba_premultiplied([self.w, self.h], &self.out)
    }
}

thread_local! {
    /// Each arm pass's texture (the left arm's behind the neck, in front of it; the bow
    /// arm's), kept and refilled.
    static TEX: std::cell::RefCell<[Option<(egui::Context, egui::TextureHandle)>; 4]> =
        const { std::cell::RefCell::new([None, None, None, None]) };
}

/// Whether slot `slot` holds no picture for `ctx` (none yet, or another context's).
pub fn stale(ctx: &egui::Context, slot: usize) -> bool {
    TEX.with(|t| {
        t.borrow()[slot.min(3)]
            .as_ref()
            .is_none_or(|(c, _)| c != ctx)
    })
}

/// `r`'s picture uploaded into slot `slot`'s texture (made the first time), or the slot's
/// last one when `fresh` is false; its id.
pub fn upload(
    ctx: &egui::Context,
    slot: usize,
    r: &mut Raster,
    fresh: bool,
) -> Option<egui::TextureId> {
    TEX.with(|t| {
        let mut t = t.borrow_mut();
        let s = &mut t[slot.min(3)];
        let theirs = s.as_ref().is_some_and(|(c, _)| c != ctx);
        if fresh || s.is_none() || theirs {
            let img = r.resolve();
            match s {
                Some((c, h)) if c == ctx => h.set(img, egui::TextureOptions::NEAREST),
                _ => {
                    *s = Some((
                        ctx.clone(),
                        ctx.load_texture(
                            format!("arcus-arm-{slot}"),
                            img,
                            egui::TextureOptions::NEAREST,
                        ),
                    ))
                }
            }
        }
        s.as_ref().map(|(_, h)| h.id())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_nearer_triangle_wins_each_sample_and_edges_are_covered_in_part() {
        let mut r = Raster::default();
        r.begin(Rect::from_min_max(Pos2::ZERO, Pos2::new(4.0, 4.0)), 1.0, 2);
        let red = Color32::from_rgb(255, 0, 0);
        let blue = Color32::from_rgb(0, 0, 255);
        let p = [
            Pos2::new(-1.0, -1.0),
            Pos2::new(9.0, -1.0),
            Pos2::new(-1.0, 9.0),
        ];
        r.tri(p, [2.0; 3], [red; 3], None);
        r.tri(p, [1.0; 3], [blue; 3], None);
        r.tri(p, [3.0; 3], [red; 3], None);
        let img = r.resolve();
        assert_eq!(img.pixels[0], blue);
        // A pixel the diagonal crosses is part covered.
        let mut r = Raster::default();
        r.begin(Rect::from_min_max(Pos2::ZERO, Pos2::new(4.0, 4.0)), 1.0, 2);
        let p = [
            Pos2::new(0.0, 0.0),
            Pos2::new(4.0, 0.0),
            Pos2::new(0.0, 4.0),
        ];
        r.tri(p, [1.0; 3], [red; 3], None);
        let edge = r.resolve().pixels[2 * 4 + 1];
        assert!(edge.a() > 0 && edge.a() < 255, "{edge:?}");
    }
}
