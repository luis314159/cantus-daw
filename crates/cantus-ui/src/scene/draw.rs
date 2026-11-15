//! The display list drawn (the design's SVG backend, rasterised): every op filled or
//! stroked with `tiny-skia` at the screen's pixels — antialiased paths, linear and radial
//! gradients, the lid's clip — layer by layer; the soft layers (the far walls, the floor's
//! reflection with its fading mask, the shadows' penumbra, core and contacts) drawn apart
//! and blurred (three box passes, the SVG `feGaussianBlur` recipe), then laid over the
//! picture. Bands of rows are drawn in parallel. Pure: the same list and scale give the
//! same pixels.

use super::display::{Cap, Layer, List, Op, Paint, Shape};
use crate::kit::skin::scene::C3;
use rayon::prelude::*;
use tiny_skia::{
    FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Mask, PathBuilder, Pixmap,
    PixmapPaint, Point, RadialGradient, Rect, Shader, SpreadMode, Stroke, Transform,
};

/// The blurs' base (the design's `svg(blur=3.0)`), in points.
const BLUR: f32 = 3.0;

fn color(c: C3, a: f32) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba(
        c[0].clamp(0.0, 1.0),
        c[1].clamp(0.0, 1.0),
        c[2].clamp(0.0, 1.0),
        a.clamp(0.0, 1.0),
    )
    .unwrap_or(tiny_skia::Color::TRANSPARENT)
}

fn stops(s: &[(f32, C3, f32)], op: f32) -> Vec<GradientStop> {
    s.iter()
        .map(|&(o, c, a)| GradientStop::new(o.clamp(0.0, 1.0), color(c, a * op)))
        .collect()
}

fn shader(p: &Paint, op: f32) -> Shader<'static> {
    match p {
        Paint::Solid(c) => Shader::SolidColor(color(*c, op)),
        Paint::Lin { a, b, stops: s } => {
            let first = s
                .first()
                .map_or(color([0.0; 3], 0.0), |f| color(f.1, f.2 * op));
            LinearGradient::new(
                Point::from_xy(a.x, a.y),
                Point::from_xy(b.x, b.y),
                stops(s, op),
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap_or(Shader::SolidColor(first))
        }
        Paint::Rad {
            c,
            rx,
            ry,
            stops: s,
        } => {
            let first = s
                .first()
                .map_or(color([0.0; 3], 0.0), |f| color(f.1, f.2 * op));
            RadialGradient::new(
                Point::from_xy(0.0, 0.0),
                0.0,
                Point::from_xy(0.0, 0.0),
                1.0,
                stops(s, op),
                SpreadMode::Pad,
                Transform::from_row(rx.max(0.1), 0.0, 0.0, ry.max(0.1), c.x, c.y),
            )
            .unwrap_or(Shader::SolidColor(first))
        }
    }
}

fn path(pts: &[egui::Pos2], closed: bool) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let mut it = pts.iter().filter(|p| p.x.is_finite() && p.y.is_finite());
    let f = it.next()?;
    pb.move_to(f.x, f.y);
    for p in it {
        pb.line_to(p.x, p.y);
    }
    if closed {
        pb.close();
    }
    pb.finish()
}

fn stroke(w: f32, cap: Cap) -> Stroke {
    Stroke {
        width: w.max(0.01),
        line_cap: match cap {
            Cap::Round => LineCap::Round,
            Cap::Butt => LineCap::Butt,
        },
        line_join: LineJoin::Round,
        ..Stroke::default()
    }
}

/// Draw one op into `pix` through `tf` (points → this pixmap's pixels).
fn draw_op(pix: &mut Pixmap, o: &Op, tf: Transform) {
    let mut paint = tiny_skia::Paint {
        shader: shader(&o.paint, o.op),
        anti_alias: true,
        ..tiny_skia::Paint::default()
    };
    match &o.shape {
        Shape::Poly { pts, seam, clip } => {
            let Some(p) = path(pts, true) else {
                return;
            };
            let mask = clip.as_ref().and_then(|c| {
                let cp = path(c, true)?;
                let mut m = Mask::new(pix.width(), pix.height())?;
                m.fill_path(&cp, FillRule::Winding, true, tf);
                Some(m)
            });
            if clip.is_some() && mask.is_none() {
                return;
            }
            pix.fill_path(&p, &paint, FillRule::Winding, tf, mask.as_ref());
            if *seam > 0.0 {
                pix.stroke_path(&p, &paint, &stroke(*seam, Cap::Round), tf, mask.as_ref());
            }
        }
        Shape::Line { a, b, w, cap } => {
            if let Some(p) = path(&[*a, *b], false) {
                pix.stroke_path(&p, &paint, &stroke(*w, *cap), tf, None);
            }
        }
        Shape::Polyline { pts, w, closed } => {
            if let Some(p) = path(pts, *closed) {
                pix.stroke_path(&p, &paint, &stroke(*w, Cap::Round), tf, None);
            }
        }
        Shape::Ellipse { c, rx, ry } => {
            if let Some(r) =
                Rect::from_xywh(c.x - rx, c.y - ry, 2.0 * rx.max(0.01), 2.0 * ry.max(0.01))
            {
                if let Some(p) = PathBuilder::from_oval(r) {
                    paint.anti_alias = true;
                    pix.fill_path(&p, &paint, FillRule::Winding, tf, None);
                }
            }
        }
    }
}

/// How many bands of rows a picture `h` pixels tall is drawn in.
fn bands(h: u32) -> u32 {
    (h / 96).clamp(1, 16)
}

/// Draw `ops` (in order) over `pix`, `ppp` pixels a point; bands of rows in parallel.
pub fn draw_ops(pix: &mut Pixmap, ops: &[&Op], ppp: f32) {
    if ops.is_empty() {
        return;
    }
    let (w, h) = (pix.width(), pix.height());
    let n = bands(h);
    let bh = h.div_ceil(n);
    let row = w as usize * 4;
    let bounds: Vec<(f32, f32)> = ops
        .iter()
        .map(|o| {
            let b = o.bounds();
            (b.1 * ppp - 2.0, b.3 * ppp + 2.0)
        })
        .collect();
    let data = pix.data_mut();
    data.par_chunks_mut(row * bh as usize)
        .enumerate()
        .for_each(|(i, chunk)| {
            let y0 = i as u32 * bh;
            let rows = (chunk.len() / row) as u32;
            let Some(size) = tiny_skia::IntSize::from_wh(w, rows) else {
                return;
            };
            let Some(mut band) = Pixmap::from_vec(chunk.to_vec(), size) else {
                return;
            };
            let tf = Transform::from_row(ppp, 0.0, 0.0, ppp, 0.0, -(y0 as f32));
            let (top, bot) = (y0 as f32, (y0 + rows) as f32);
            for (o, &(a, b)) in ops.iter().zip(&bounds) {
                if b < top || a > bot {
                    continue;
                }
                draw_op(&mut band, o, tf);
            }
            chunk.copy_from_slice(band.data());
        });
}

/// A box blur of radius `r` along each line of `n` pixels (stride `step` bytes between
/// pixels, `channels` of the four), in place; outside the line is transparent.
fn box_line(line: &mut [u8], n: usize, r: usize, channels: &[usize], tmp: &mut Vec<u32>) {
    let d = (2 * r + 1) as u32;
    for &ch in channels {
        tmp.clear();
        tmp.extend((0..n).map(|i| line[i * 4 + ch] as u32));
        let mut sum: u32 = tmp.iter().take(r.min(n)).sum();
        for i in 0..n {
            if i + r < n {
                sum += tmp[i + r];
            }
            line[i * 4 + ch] = ((sum + d / 2) / d).min(255) as u8;
            if i >= r {
                sum -= tmp[i - r];
            }
        }
    }
}

fn blur_rows(data: &mut [u8], w: usize, radii: &[usize], channels: &[usize]) {
    data.par_chunks_mut(w * 4)
        .for_each_init(Vec::new, |tmp, line| {
            for &r in radii {
                box_line(line, w, r, channels, tmp);
            }
        });
}

fn transpose(src: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    out.par_chunks_mut(h * 4).enumerate().for_each(|(x, col)| {
        for y in 0..h {
            col[y * 4..y * 4 + 4].copy_from_slice(&src[(y * w + x) * 4..(y * w + x) * 4 + 4]);
        }
    });
    out
}

/// The box radii three passes take to approach a Gaussian of `sigma` pixels.
fn radii(sigma: f32) -> Vec<usize> {
    if sigma < 0.3 {
        return Vec::new();
    }
    if sigma < 2.0 {
        // Small: two passes of radius 1 are about σ 1.15; one is about σ 0.8.
        return if sigma < 1.0 { vec![1] } else { vec![1, 1] };
    }
    let d = (sigma * 3.0 * (2.0 * std::f32::consts::PI).sqrt() / 4.0 + 0.5).floor() as usize;
    // SVG's three boxes of width d (an even d's offset pair and its d + 1 centred alike at
    // this resolution): each a centred box of radius d / 2.
    vec![d / 2; 3]
}

/// Blur `pix` by `sigma` pixels (only `channels`; a shadow's colour is black: its alpha).
pub fn blur(pix: &mut Pixmap, sigma: f32, channels: &[usize]) {
    let rs = radii(sigma);
    if rs.is_empty() {
        return;
    }
    let (w, h) = (pix.width() as usize, pix.height() as usize);
    blur_rows(pix.data_mut(), w, &rs, channels);
    let mut t = transpose(pix.data(), w, h);
    blur_rows(&mut t, h, &rs, channels);
    let back = transpose(&t, h, w);
    pix.data_mut().copy_from_slice(&back);
}

fn layer(list: &List, l: Layer) -> Vec<&Op> {
    list.layer_ops(l)
}

/// A soft layer: drawn apart, blurred, laid over `pix` at `opacity` (through `mask`).
fn soft(
    pix: &mut Pixmap,
    ops: &[&Op],
    ppp: f32,
    sigma: f32,
    alpha_only: bool,
    opacity: f32,
    mask: Option<&Mask>,
) {
    if ops.is_empty() {
        return;
    }
    let Some(mut s) = Pixmap::new(pix.width(), pix.height()) else {
        return;
    };
    draw_ops(&mut s, ops, ppp);
    blur(
        &mut s,
        sigma * ppp,
        if alpha_only { &[3] } else { &[0, 1, 2, 3] },
    );
    let paint = PixmapPaint {
        opacity: opacity.clamp(0.0, 1.0),
        ..PixmapPaint::default()
    };
    pix.draw_pixmap(0, 0, s.as_ref(), &paint, Transform::identity(), mask);
}

/// The floor's mirror, drawn once: crisp near where things stand on the floor and softer
/// as it runs away from them (a polished floor's reflection), fading out by the fade's end.
fn floor_mirror(pix: &mut Pixmap, ops: &[&Op], ppp: f32, gloss: f32, fade: Option<(f32, f32)>) {
    let (w, h) = (pix.width(), pix.height());
    let Some(mut sharp) = Pixmap::new(w, h) else {
        return;
    };
    draw_ops(&mut sharp, ops, ppp);
    let mut soft_copy = sharp.clone();
    blur(&mut sharp, BLUR * 0.12 * ppp, &[0, 1, 2, 3]);
    blur(&mut soft_copy, BLUR * 0.7 * ppp, &[0, 1, 2, 3]);
    let paint = PixmapPaint {
        opacity: gloss.clamp(0.0, 1.0),
        ..PixmapPaint::default()
    };
    let Some((y0, y1)) = fade else {
        pix.draw_pixmap(0, 0, sharp.as_ref(), &paint, Transform::identity(), None);
        return;
    };
    // Near: whole by the contact line, gone a third of the way down the fade; far: the
    // rest of the fade.
    let yn = y0 + (y1 - y0) * 0.32;
    let ramp = |y: f32, a: f32, b: f32| (1.0 - (y - a) / (b - a).max(1.0)).clamp(0.0, 1.0);
    let near = row_mask(w, h, |y| ramp(y / ppp, y0, yn));
    let far = row_mask(w, h, |y| {
        let y = y / ppp;
        (ramp(y, y0, y1) - ramp(y, y0, yn)).max(0.0)
    });
    pix.draw_pixmap(
        0,
        0,
        soft_copy.as_ref(),
        &paint,
        Transform::identity(),
        far.as_ref(),
    );
    pix.draw_pixmap(
        0,
        0,
        sharp.as_ref(),
        &paint,
        Transform::identity(),
        near.as_ref(),
    );
}

/// A mask whose every row is `f(y)` (0 … 1, `y` in pixels).
fn row_mask(w: u32, h: u32, f: impl Fn(f32) -> f32) -> Option<Mask> {
    let mut m = Mask::new(w, h)?;
    let row = w as usize;
    for (y, line) in m.data_mut().chunks_mut(row).enumerate() {
        let a = (f(y as f32 + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
        line.fill(a);
    }
    Some(m)
}

/// The whole picture `list` of a scene `size` points large, at `ppp` pixels a point, over
/// `ground` (the mood's sky).
pub fn render(list: &List, size: egui::Vec2, ppp: f32, ground: C3, gloss: f32) -> Option<Pixmap> {
    let w = ((size.x * ppp).round() as u32).max(1);
    let h = ((size.y * ppp).round() as u32).max(1);
    let mut pix = Pixmap::new(w, h)?;
    pix.fill(color(ground, 1.0));
    // The far walls drawn sharp under their soft copy: the blur fades a layer toward
    // transparent at the picture's edges, and the sky must never show through there.
    let far = layer(list, Layer::Far);
    draw_ops(&mut pix, &far, ppp);
    soft(&mut pix, &far, ppp, BLUR * 0.28, false, 1.0, None);
    draw_ops(&mut pix, &layer(list, Layer::Haze), ppp);
    draw_ops(&mut pix, &layer(list, Layer::Back), ppp);
    let mirror = layer(list, Layer::Mirror);
    if !mirror.is_empty() {
        floor_mirror(&mut pix, &mirror, ppp, gloss, list.mirror_fade);
    }
    draw_ops(&mut pix, &layer(list, Layer::Rug), ppp);
    soft(
        &mut pix,
        &layer(list, Layer::ShadowWide),
        ppp,
        BLUR * 5.0,
        true,
        1.0,
        None,
    );
    soft(
        &mut pix,
        &layer(list, Layer::Shadow),
        ppp,
        BLUR * 1.6,
        true,
        1.0,
        None,
    );
    soft(
        &mut pix,
        &layer(list, Layer::Contact),
        ppp,
        BLUR * 0.5,
        true,
        1.0,
        None,
    );
    draw_ops(&mut pix, &layer(list, Layer::Objects), ppp);
    draw_ops(&mut pix, &layer(list, Layer::Front), ppp);
    Some(pix)
}
