//! A software renderer for egui frames, so the interface can be *looked at* without a
//! screen: tessellate what a frame painted, fill the triangles on the CPU, write a PNG.
//!
//! It exists for the design review the round-14 plan asks for — "screenshots compared per
//! view before and after, so *better* is shown rather than claimed" — and for machines
//! whose display is locked or absent. It is not the application's renderer and is not
//! fast; it is exact enough to judge layout, colour and type.
//!
//! Blending follows egui's own convention: colours are premultiplied, in gamma space, and
//! a textured vertex multiplies the texel (the font atlas is white with coverage in alpha).

use egui::epaint::{ClippedPrimitive, Primitive};
use egui::{Color32, ImageData, TextureId};
use std::collections::HashMap;
use std::path::Path;

/// The textures a frame's meshes sample, kept across frames as egui sends only deltas.
#[derive(Default)]
pub struct Textures {
    images: HashMap<TextureId, (usize, usize, Vec<Color32>)>,
}

impl Textures {
    /// Apply what a frame's output says changed.
    pub fn update(&mut self, delta: &egui::TexturesDelta) {
        for (id, deltas) in delta
            .set
            .iter()
            .flat_map(|(id, ds)| ds.iter().map(move |d| (id, d)))
        {
            let d = deltas;
            let ImageData::Color(img) = &d.image;
            let (w, h) = (img.size[0], img.size[1]);
            match d.pos {
                None => {
                    self.images.insert(*id, (w, h, img.pixels.clone()));
                }
                Some([x0, y0]) => {
                    if let Some((tw, _, px)) = self.images.get_mut(id) {
                        for y in 0..h {
                            for x in 0..w {
                                px[(y0 + y) * *tw + x0 + x] = img.pixels[y * w + x];
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.images.remove(id);
        }
    }

    fn sample(&self, id: TextureId, u: f32, v: f32) -> [f32; 4] {
        let Some((w, h, px)) = self.images.get(&id) else {
            return [1.0; 4];
        };
        let x = ((u * *w as f32) as isize).clamp(0, *w as isize - 1) as usize;
        let y = ((v * *h as f32) as isize).clamp(0, *h as isize - 1) as usize;
        let c = px[y * *w + x];
        [
            c.r() as f32 / 255.0,
            c.g() as f32 / 255.0,
            c.b() as f32 / 255.0,
            c.a() as f32 / 255.0,
        ]
    }
}

/// An RGBA image being painted, premultiplied, 0…1.
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn new(width: usize, height: usize, background: Color32) -> Self {
        let bg = [
            background.r() as f32 / 255.0,
            background.g() as f32 / 255.0,
            background.b() as f32 / 255.0,
            1.0,
        ];
        Canvas {
            width,
            height,
            px: vec![bg; width * height],
        }
    }

    /// The pixel at (x, y) as 8-bit RGBA.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let p = self.px[y * self.width + x];
        [
            (p[0].clamp(0.0, 1.0) * 255.0).round() as u8,
            (p[1].clamp(0.0, 1.0) * 255.0).round() as u8,
            (p[2].clamp(0.0, 1.0) * 255.0).round() as u8,
            255,
        ]
    }

    /// Paint tessellated primitives at `pixels_per_point`.
    pub fn paint(&mut self, prims: &[ClippedPrimitive], textures: &Textures, ppp: f32) {
        for cp in prims {
            let Primitive::Mesh(mesh) = &cp.primitive else {
                continue;
            };
            let clip = cp.clip_rect;
            let (cx0, cy0) = ((clip.min.x * ppp).max(0.0), (clip.min.y * ppp).max(0.0));
            let (cx1, cy1) = (
                (clip.max.x * ppp).min(self.width as f32),
                (clip.max.y * ppp).min(self.height as f32),
            );
            for tri in mesh.indices.chunks_exact(3) {
                let v = [
                    &mesh.vertices[tri[0] as usize],
                    &mesh.vertices[tri[1] as usize],
                    &mesh.vertices[tri[2] as usize],
                ];
                let p: Vec<(f32, f32)> = v.iter().map(|v| (v.pos.x * ppp, v.pos.y * ppp)).collect();
                let area =
                    (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
                if area.abs() < 1e-9 {
                    continue;
                }
                let minx = p
                    .iter()
                    .map(|q| q.0)
                    .fold(f32::MAX, f32::min)
                    .max(cx0)
                    .floor() as i64;
                let maxx = p
                    .iter()
                    .map(|q| q.0)
                    .fold(f32::MIN, f32::max)
                    .min(cx1)
                    .ceil() as i64;
                let miny = p
                    .iter()
                    .map(|q| q.1)
                    .fold(f32::MAX, f32::min)
                    .max(cy0)
                    .floor() as i64;
                let maxy = p
                    .iter()
                    .map(|q| q.1)
                    .fold(f32::MIN, f32::max)
                    .min(cy1)
                    .ceil() as i64;
                let col: Vec<[f32; 4]> = v
                    .iter()
                    .map(|v| {
                        [
                            v.color.r() as f32 / 255.0,
                            v.color.g() as f32 / 255.0,
                            v.color.b() as f32 / 255.0,
                            v.color.a() as f32 / 255.0,
                        ]
                    })
                    .collect();
                // The top-left fill rule, as a GPU applies it: a pixel whose centre lies
                // exactly on an edge belongs to the triangle only if that edge is a top
                // or a left one. Without it, a pixel on the edge two triangles share is
                // blended twice, which shows as a seam wherever a mesh is translucent.
                let top_left = |i: usize| {
                    let (a, b) = (p[(i + 1) % 3], p[(i + 2) % 3]);
                    let (dx, dy) = if area > 0.0 {
                        (b.0 - a.0, b.1 - a.1)
                    } else {
                        (a.0 - b.0, a.1 - b.1)
                    };
                    (dy == 0.0 && dx > 0.0) || dy < 0.0
                };
                let tl = [top_left(0), top_left(1), top_left(2)];
                const ON_EDGE: f32 = 1e-6;
                for y in miny.max(0)..maxy.min(self.height as i64) {
                    for x in minx.max(0)..maxx.min(self.width as i64) {
                        let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                        if px < cx0 || px >= cx1 || py < cy0 || py >= cy1 {
                            continue;
                        }
                        let w0 =
                            ((p[1].0 - px) * (p[2].1 - py) - (p[2].0 - px) * (p[1].1 - py)) / area;
                        let w1 =
                            ((p[2].0 - px) * (p[0].1 - py) - (p[0].0 - px) * (p[2].1 - py)) / area;
                        let w2 =
                            ((p[0].0 - px) * (p[1].1 - py) - (p[1].0 - px) * (p[0].1 - py)) / area;
                        let inside = |w: f32, i: usize| w > ON_EDGE || (w >= -ON_EDGE && tl[i]);
                        if !(inside(w0, 0) && inside(w1, 1) && inside(w2, 2)) {
                            continue;
                        }
                        let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                        let vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                        let t = textures.sample(mesh.texture_id, u, vv);
                        let mut s = [0f32; 4];
                        for k in 0..4 {
                            s[k] = (w0 * col[0][k] + w1 * col[1][k] + w2 * col[2][k]) * t[k];
                        }
                        let d = &mut self.px[y as usize * self.width + x as usize];
                        for k in 0..3 {
                            d[k] = s[k] + d[k] * (1.0 - s[3]);
                        }
                    }
                }
            }
        }
    }

    /// Write the canvas as an 8-bit RGBA PNG (stored, uncompressed: no dependency, and a
    /// screenshot for a review is read once).
    pub fn write_png(&self, path: &Path) -> std::io::Result<()> {
        let mut raw = Vec::with_capacity((self.width * 4 + 1) * self.height);
        for y in 0..self.height {
            raw.push(0u8);
            for x in 0..self.width {
                raw.extend_from_slice(&self.pixel(x, y));
            }
        }
        let mut z = vec![0x78, 0x01];
        let mut chunks = raw.chunks(65_535).peekable();
        while let Some(c) = chunks.next() {
            z.push(if chunks.peek().is_none() { 1 } else { 0 });
            let len = c.len() as u16;
            z.extend_from_slice(&len.to_le_bytes());
            z.extend_from_slice(&(!len).to_le_bytes());
            z.extend_from_slice(c);
        }
        let (mut a, mut b) = (1u32, 0u32);
        for byte in &raw {
            a = (a + *byte as u32) % 65_521;
            b = (b + a) % 65_521;
        }
        z.extend_from_slice(&((b << 16) | a).to_be_bytes());
        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&(self.width as u32).to_be_bytes());
        ihdr.extend_from_slice(&(self.height as u32).to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        std::fs::write(path, out)
    }
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c ^= *b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut body = kind.to_vec();
    body.extend_from_slice(data);
    out.extend_from_slice(&body);
    out.extend_from_slice(&crc32(&body).to_be_bytes());
}

/// Run `frames` frames of `ui` at `size` points and render the last one to `path`.
/// Returns the canvas so a caller can also check pixels.
pub fn shoot(
    ctx: &egui::Context,
    size: egui::Vec2,
    ppp: f32,
    frames: usize,
    path: Option<&Path>,
    mut ui: impl FnMut(&mut egui::Ui),
) -> Canvas {
    ctx.set_pixels_per_point(ppp);
    let mut textures = Textures::default();
    let mut last = None;
    for frame in 0..frames.max(1) {
        // Time moves a quarter second a frame, so a window's opening fade and any other
        // animation has finished by the frame that is drawn.
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(frame as f64 * 0.25),
            predicted_dt: 0.25,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |root| ui(root));
        textures.update(&out.textures_delta);
        out.textures_delta.clear();
        last = Some(out);
    }
    let out = last.expect("at least one frame");
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    let bg = ctx.global_style().visuals.panel_fill;
    let mut canvas = Canvas::new(
        (size.x * ppp).round() as usize,
        (size.y * ppp).round() as usize,
        bg,
    );
    canvas.paint(&prims, &textures, out.pixels_per_point);
    if let Some(p) = path {
        let _ = canvas.write_png(p);
    }
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_painted_rectangle_lands_where_it_was_painted_in_its_colour() {
        let ctx = egui::Context::default();
        let canvas = shoot(&ctx, egui::vec2(100.0, 60.0), 1.0, 2, None, |root| {
            egui::CentralPanel::default().show(root, |ui| {
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(egui::pos2(20.0, 10.0), egui::pos2(60.0, 40.0)),
                    0.0,
                    Color32::from_rgb(200, 30, 40),
                );
            });
        });
        assert_eq!(canvas.pixel(40, 25), [200, 30, 40, 255]);
        assert_ne!(canvas.pixel(80, 50), [200, 30, 40, 255]);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.png");
        canvas.write_png(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
    }
}
