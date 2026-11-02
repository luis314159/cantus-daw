//! Drawing a Lottie frame with tiny-skia (ADR 0089 §2): layers bottom to top, parents,
//! precomps, masks and track mattes, shapes with Lottie's rule that a style paints every
//! outline before it in its group (and its groups'), trims, repeaters, gradients, dashes.

use super::super::raster::{self, Filter, Region, Tint};
use super::geom::{self, Geo};
use super::model::{Content, Doc, Glyph, Gradient, Layer, Shape, Stroke as LStroke, Text};
use crate::render::hue_rotate;
use cantus_dsp::math;
use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Mask, MaskType,
    Paint, PathBuilder, Pixmap, Point, RadialGradient, Rect, SpreadMode, Stroke, StrokeDash,
    Transform,
};

/// Outlines painted in one frame at most (repeaters inside repeaters cannot run away).
pub const MAX_PAINTS: usize = 50_000;
/// Canvas-sized buffers (mattes, masks, precomp clips) one frame may make.
pub const MAX_OFFSCREEN: usize = 64;
/// What one such buffer counts against [`MAX_PAINTS`].
pub const OFFSCREEN_PAINTS: usize = 100;

/// Where a frame goes: pixels, or the box of what would be drawn.
pub enum Target<'a> {
    Pixels(&'a mut Pixmap),
    Bounds(&'a mut [f32; 4]),
}

/// A style, evaluated at the frame.
#[derive(Clone)]
enum Style {
    Fill {
        paint: Paint<'static>,
        rule: FillRule,
    },
    Stroke {
        paint: Paint<'static>,
        stroke: Stroke,
    },
}

/// A style and the outlines it paints, in its own group's space (`t`).
struct Draw {
    style: Style,
    geo: Vec<Geo>,
    t: Transform,
    opacity: f32,
}

pub struct Frame<'a> {
    pub doc: &'a Doc,
    pub hue: f32,
    pub filter: Filter,
    paints: usize,
    /// Canvas-sized buffers (a matte's layer, a mask, a precomp's clip) made this frame.
    offscreen: usize,
}

impl<'a> Frame<'a> {
    pub fn new(doc: &'a Doc, hue: f32, filter: Filter) -> Frame<'a> {
        Frame {
            doc,
            hue,
            filter,
            paints: 0,
            offscreen: 0,
        }
    }

    /// Room for one more canvas-sized buffer this frame (each also counts as many paints):
    /// past [`MAX_OFFSCREEN`] the matte, mask or clip is left out, never the frame's memory.
    fn take_offscreen(&mut self) -> bool {
        if self.offscreen >= MAX_OFFSCREEN {
            return false;
        }
        self.offscreen += 1;
        self.paints += OFFSCREEN_PAINTS;
        true
    }

    /// Buffers made in the last frame drawn with this.
    pub fn offscreen_used(&self) -> usize {
        self.offscreen
    }

    fn color(&self, c: &[f32], alpha: f32) -> Color {
        let g = |i: usize| c.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let (r, gg, b) = hue_rotate(g(0), g(1), g(2), self.hue);
        Color::from_rgba(r, gg, b, alpha.clamp(0.0, 1.0)).unwrap_or(Color::BLACK)
    }

    /// Draw `layers` (a composition) at its frame `f`, `base` mapping it onto the target.
    #[allow(clippy::too_many_arguments)]
    pub fn layers(
        &mut self,
        layers: &[Layer],
        f: f32,
        base: Transform,
        opacity: f32,
        mask: Option<&Mask>,
        target: &mut Target<'_>,
        depth: usize,
    ) {
        if depth > 32 {
            return;
        }
        for i in (0..layers.len()).rev() {
            let l = &layers[i];
            if l.is_matte || l.hidden || f < l.ip || f >= l.op {
                continue;
            }
            // Its matte: the layer above it (or the one `tp` names).
            let matte_src = (l.matte != 0)
                .then(|| match l.matte_parent {
                    Some(tp) => layers.iter().position(|x| x.index == Some(tp)),
                    None => i.checked_sub(1),
                })
                .flatten();
            let mut own_mask = None;
            // A buffer only for a layer that has a matte (review 2), and only when drawing.
            let wants = matte_src.is_some() && matches!(target, Target::Pixels(_));
            if let (Some(mi), Target::Pixels(pm), true) =
                (matte_src, &*target, wants && self.take_offscreen())
            {
                let (w, h) = (pm.width(), pm.height());
                if let Some(mut off) = Pixmap::new(w, h) {
                    let src = &layers[mi];
                    {
                        let mut t = Target::Pixels(&mut off);
                        self.layer(layers, mi, src, f, base, 1.0, None, &mut t, depth);
                    }
                    let kind = if l.matte >= 3 {
                        MaskType::Luminance
                    } else {
                        MaskType::Alpha
                    };
                    let mut m = Mask::from_pixmap(off.as_ref(), kind);
                    if l.matte == 2 || l.matte == 4 {
                        m.invert();
                    }
                    own_mask = Some(m);
                }
            }
            let combined = combine(mask, own_mask);
            let m = combined.as_ref().or(mask);
            self.layer(layers, i, l, f, base, opacity, m, target, depth);
        }
    }

    /// The transform of layer `i` at the composition's frame `f`, through its parents.
    fn world(&self, layers: &[Layer], i: usize, f: f32, hops: usize) -> Transform {
        let l = &layers[i];
        let lt = (f - l.st) / l.stretch;
        let own = geom::matrix(&l.ks, lt);
        match l
            .parent
            .and_then(|p| layers.iter().position(|x| x.index == Some(p)))
        {
            Some(pi) if pi != i && hops < 64 => self.world(layers, pi, f, hops + 1).pre_concat(own),
            _ => own,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn layer(
        &mut self,
        layers: &[Layer],
        i: usize,
        l: &Layer,
        f: f32,
        base: Transform,
        opacity: f32,
        mask: Option<&Mask>,
        target: &mut Target<'_>,
        depth: usize,
    ) {
        let lt = (f - l.st) / l.stretch;
        let t = base.pre_concat(self.world(layers, i, f, 0));
        let alpha = opacity * (l.ks.opacity.at1(lt, 100.0) / 100.0).clamp(0.0, 1.0);
        if alpha <= 0.0 {
            return;
        }
        // Its masks, in its own space.
        let masks = if l.masks.is_empty() {
            None
        } else if let (Target::Pixels(pm), true) = (
            &*target,
            matches!(target, Target::Pixels(_)) && self.take_offscreen(),
        ) {
            layer_masks(l, lt, t, pm.width(), pm.height())
        } else {
            None
        };
        let combined = combine(mask, masks);
        let mask = combined.as_ref().or(mask);
        let blend = blend_mode(l.blend);
        match &l.content {
            Content::Shapes(items) => self.shapes(items, lt, t, alpha, mask, blend, target),
            Content::Text(text) => self.text(text, t, alpha, mask, blend, target),
            Content::Precomp { asset, w, h } => {
                let child_f = match &l.time_remap {
                    Some(tm) => tm.at1(lt, 0.0) * self.doc.fr,
                    None => lt,
                };
                // A precomposition is clipped to its own box.
                let clip = match &*target {
                    Target::Pixels(pm) if *w > 0.0 && *h > 0.0 && self.take_offscreen() => {
                        let mut m = Mask::new(pm.width(), pm.height());
                        if let (Some(m), Some(r)) = (m.as_mut(), Rect::from_xywh(0.0, 0.0, *w, *h))
                        {
                            m.fill_path(&PathBuilder::from_rect(r), FillRule::Winding, true, t);
                        }
                        m
                    }
                    _ => None,
                };
                let combined = combine(mask, clip);
                let mask = combined.as_ref().or(mask);
                if let Some(children) = self.doc.precomps.get(*asset) {
                    self.layers(children, child_f, t, alpha, mask, target, depth + 1);
                }
            }
            Content::Image(k) => self.image(*k, t, alpha, target),
            Content::Solid { color, w, h } => {
                if let Some(r) = Rect::from_xywh(0.0, 0.0, *w, *h) {
                    let geo = vec![Geo {
                        contours: vec![geom::rect(
                            [r.x() + r.width() * 0.5, r.y() + r.height() * 0.5],
                            [r.width(), r.height()],
                            0.0,
                        )],
                        t,
                    }];
                    let mut paint = Paint::default();
                    paint.set_color(self.color(color, 1.0));
                    paint.anti_alias = true;
                    paint.blend_mode = blend;
                    let d = Draw {
                        style: Style::Fill {
                            paint,
                            rule: FillRule::Winding,
                        },
                        geo,
                        t,
                        opacity: alpha,
                    };
                    self.paint(&d, mask, target);
                }
            }
            Content::Null => {}
        }
    }

    fn image(&mut self, k: usize, t: Transform, alpha: f32, target: &mut Target<'_>) {
        let Some(Some(img)) = self.doc.images.get(k) else {
            return;
        };
        match target {
            Target::Bounds(b) => {
                let r = Rect::from_xywh(0.0, 0.0, img.width() as f32, img.height() as f32);
                if let Some(r) = r.and_then(|r| r.transform(t)) {
                    grow(b, r);
                }
            }
            Target::Pixels(pm) => {
                raster::image(
                    pm,
                    img,
                    Region::of(img),
                    t,
                    self.filter,
                    Tint::opacity(alpha).hue(self.hue),
                );
            }
        }
    }
}

impl Frame<'_> {
    #[allow(clippy::too_many_arguments)]
    fn shapes(
        &mut self,
        items: &[Shape],
        f: f32,
        t: Transform,
        alpha: f32,
        mask: Option<&Mask>,
        blend: BlendMode,
        target: &mut Target<'_>,
    ) {
        let mut geo = Vec::new();
        let mut draws = Vec::new();
        self.collect(items, f, t, alpha, blend, &mut geo, &mut draws);
        // The first style in a group is on top: paint from the last.
        for d in draws.iter().rev() {
            self.paint(d, mask, target);
        }
    }

    /// Walk a group: outlines gather; each style takes every outline gathered so far.
    #[allow(clippy::too_many_arguments)]
    fn collect(
        &mut self,
        items: &[Shape],
        f: f32,
        t: Transform,
        alpha: f32,
        blend: BlendMode,
        geo_out: &mut Vec<Geo>,
        draws_out: &mut Vec<Draw>,
    ) {
        let mut geo: Vec<Geo> = Vec::new();
        let mut draws: Vec<Draw> = Vec::new();
        for item in items {
            if self.paints > MAX_PAINTS {
                break;
            }
            match item {
                Shape::Group(children, tr) => {
                    let gt = t.pre_concat(geom::matrix(tr, f));
                    let ga = alpha * (tr.opacity.at1(f, 100.0) / 100.0).clamp(0.0, 1.0);
                    let (mut g, mut d) = (Vec::new(), Vec::new());
                    self.collect(children, f, gt, ga, blend, &mut g, &mut d);
                    geo.extend(g);
                    draws.extend(d);
                }
                Shape::Path(p) => geo.push(Geo {
                    contours: vec![p.at(f)],
                    t,
                }),
                Shape::Rect { p, s, r } => geo.push(Geo {
                    contours: vec![geom::rect(
                        p.at2(f, [0.0; 2]),
                        s.at2(f, [0.0; 2]),
                        r.at1(f, 0.0),
                    )],
                    t,
                }),
                Shape::Ellipse { p, s } => geo.push(Geo {
                    contours: vec![geom::ellipse(p.at2(f, [0.0; 2]), s.at2(f, [0.0; 2]))],
                    t,
                }),
                Shape::Star {
                    star,
                    p,
                    points,
                    rotation,
                    outer,
                    inner,
                } => geo.push(Geo {
                    contours: vec![geom::star(
                        *star,
                        p.at2(f, [0.0; 2]),
                        points.at1(f, 5.0),
                        rotation.at1(f, 0.0),
                        outer.at1(f, 0.0),
                        inner.at1(f, 0.0),
                    )],
                    t,
                }),
                Shape::Fill {
                    color,
                    opacity,
                    even_odd,
                } => {
                    let c = color.at(f);
                    let paint = self.solid(&c, blend);
                    draws.push(Draw {
                        style: Style::Fill {
                            paint,
                            rule: rule(*even_odd),
                        },
                        geo: geo.clone(),
                        t,
                        opacity: alpha * pct(opacity.at1(f, 100.0)),
                    });
                }
                Shape::Stroke {
                    color,
                    opacity,
                    stroke,
                } => {
                    let c = color.at(f);
                    let paint = self.solid(&c, blend);
                    draws.push(Draw {
                        style: Style::Stroke {
                            paint,
                            stroke: stroke_at(stroke, f),
                        },
                        geo: geo.clone(),
                        t,
                        opacity: alpha * pct(opacity.at1(f, 100.0)),
                    });
                }
                Shape::GradientFill {
                    g,
                    opacity,
                    even_odd,
                } => {
                    if let Some(paint) = self.gradient(g, f, blend) {
                        draws.push(Draw {
                            style: Style::Fill {
                                paint,
                                rule: rule(*even_odd),
                            },
                            geo: geo.clone(),
                            t,
                            opacity: alpha * pct(opacity.at1(f, 100.0)),
                        });
                    }
                }
                Shape::GradientStroke { g, opacity, stroke } => {
                    if let Some(paint) = self.gradient(g, f, blend) {
                        draws.push(Draw {
                            style: Style::Stroke {
                                paint,
                                stroke: stroke_at(stroke, f),
                            },
                            geo: geo.clone(),
                            t,
                            opacity: alpha * pct(opacity.at1(f, 100.0)),
                        });
                    }
                }
                Shape::Trim {
                    start,
                    end,
                    offset,
                    individually,
                } => {
                    let s = start.at1(f, 0.0) / 100.0;
                    let e = end.at1(f, 100.0) / 100.0;
                    let o = offset.at1(f, 0.0) / 360.0;
                    trim_all(&mut geo, s, e, o, *individually);
                }
                Shape::RoundCorners(r) => {
                    let r = r.at1(f, 0.0);
                    for g in &mut geo {
                        for c in &mut g.contours {
                            *c = geom::round_corners(c, r);
                        }
                    }
                }
                Shape::Repeater {
                    copies,
                    offset,
                    above,
                    tr,
                } => {
                    let n = (copies.at1(f, 1.0).round().max(0.0) as usize).min(1000);
                    self.paints += n * (draws.len() + 1);
                    if self.paints > MAX_PAINTS {
                        break;
                    }
                    let off = offset.at1(f, 0.0);
                    let inv = t.invert().unwrap_or_default();
                    let (so, eo) = (
                        pct(tr.start_opacity.at1(f, 100.0)),
                        pct(tr.end_opacity.at1(f, 100.0)),
                    );
                    let mut new_geo = Vec::new();
                    let mut new_draws = Vec::new();
                    let order: Vec<usize> = if *above {
                        (0..n).rev().collect()
                    } else {
                        (0..n).collect()
                    };
                    for k in order {
                        let rk = t
                            .pre_concat(geom::repeat_matrix(tr, f, off + k as f32))
                            .pre_concat(inv);
                        let op = if n > 1 {
                            so + (eo - so) * k as f32 / (n - 1) as f32
                        } else {
                            so
                        };
                        let moved = |g: &Geo| Geo {
                            contours: g.contours.clone(),
                            t: rk.pre_concat(g.t),
                        };
                        new_geo.extend(geo.iter().map(moved));
                        for d in &draws {
                            new_draws.push(Draw {
                                style: d.style.clone(),
                                geo: d.geo.iter().map(moved).collect(),
                                t: rk.pre_concat(d.t),
                                opacity: d.opacity * op,
                            });
                        }
                    }
                    geo = new_geo;
                    draws = new_draws;
                }
            }
        }
        geo_out.extend(geo);
        draws_out.extend(draws);
    }

    fn solid(&self, c: &[f32], blend: BlendMode) -> Paint<'static> {
        let mut paint = Paint::default();
        paint.set_color(self.color(c, c.get(3).copied().unwrap_or(1.0)));
        paint.anti_alias = true;
        paint.blend_mode = blend;
        paint
    }

    fn gradient(&self, g: &Gradient, f: f32, blend: BlendMode) -> Option<Paint<'static>> {
        let v = g.colors.at(f);
        let n = g.stops.max(1);
        if v.len() < n * 4 {
            return None;
        }
        let alpha_stops: Vec<(f32, f32)> =
            v[n * 4..].chunks_exact(2).map(|c| (c[0], c[1])).collect();
        let alpha_at = |x: f32| -> f32 {
            if alpha_stops.is_empty() {
                return 1.0;
            }
            let k = alpha_stops.partition_point(|s| s.0 < x);
            if k == 0 {
                return alpha_stops[0].1;
            }
            if k >= alpha_stops.len() {
                return alpha_stops[alpha_stops.len() - 1].1;
            }
            let (a, b) = (alpha_stops[k - 1], alpha_stops[k]);
            let w = if b.0 > a.0 {
                (x - a.0) / (b.0 - a.0)
            } else {
                0.0
            };
            a.1 + (b.1 - a.1) * w
        };
        let stops: Vec<GradientStop> = v[..n * 4]
            .chunks_exact(4)
            .map(|c| GradientStop::new(c[0].clamp(0.0, 1.0), self.color(&c[1..4], alpha_at(c[0]))))
            .collect();
        let [sx, sy] = g.start.at2(f, [0.0; 2]);
        let [ex, ey] = g.end.at2(f, [0.0; 2]);
        let shader = if g.kind == 2 {
            let (dx, dy) = (ex - sx, ey - sy);
            let r = (dx * dx + dy * dy).sqrt().max(1e-3);
            let h = (g.highlight.at1(f, 0.0) / 100.0).clamp(-0.99, 0.99);
            let ang = math::atan2(dy, dx) + g.angle.at1(f, 0.0) * std::f32::consts::PI / 180.0;
            let (s, c) = math::sin_cos(ang);
            let focal = Point::from_xy(sx + h * r * c, sy + h * r * s);
            RadialGradient::new(
                focal,
                0.0,
                Point::from_xy(sx, sy),
                r,
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            )
        } else {
            LinearGradient::new(
                Point::from_xy(sx, sy),
                Point::from_xy(ex, ey),
                stops,
                SpreadMode::Pad,
                Transform::identity(),
            )
        }?;
        Some(Paint {
            shader,
            anti_alias: true,
            blend_mode: blend,
            ..Paint::default()
        })
    }

    fn paint(&mut self, d: &Draw, mask: Option<&Mask>, target: &mut Target<'_>) {
        if d.opacity <= 0.0 || d.geo.is_empty() {
            return;
        }
        self.paints += d.geo.len();
        if self.paints > MAX_PAINTS {
            return;
        }
        // Every outline brought into the style's space, as one path.
        let Some(inv) = d.t.invert() else {
            return;
        };
        let mut pb = PathBuilder::new();
        for g in &d.geo {
            geom::to_path(&g.contours, inv.pre_concat(g.t), &mut pb);
        }
        let Some(path) = pb.finish() else {
            return;
        };
        match target {
            Target::Bounds(b) => {
                let mut r = path.bounds();
                if let Style::Stroke { stroke, .. } = &d.style {
                    let w = stroke.width * 0.5;
                    r = Rect::from_ltrb(r.left() - w, r.top() - w, r.right() + w, r.bottom() + w)
                        .unwrap_or(r);
                }
                if let Some(r) = r.transform(d.t) {
                    grow(b, r);
                }
            }
            Target::Pixels(pm) => match &d.style {
                Style::Fill { paint, rule } => {
                    let mut p = paint.clone();
                    p.shader.apply_opacity(d.opacity);
                    pm.fill_path(&path, &p, *rule, d.t, mask);
                }
                Style::Stroke { paint, stroke } => {
                    let mut p = paint.clone();
                    p.shader.apply_opacity(d.opacity);
                    pm.stroke_path(&path, &p, stroke, d.t, mask);
                }
            },
        }
    }

    fn text(
        &mut self,
        text: &Text,
        t: Transform,
        alpha: f32,
        mask: Option<&Mask>,
        blend: BlendMode,
        target: &mut Target<'_>,
    ) {
        if self.doc.glyphs.is_empty() {
            return;
        }
        let scale = text.size / 100.0;
        let lh = if text.line_height > 0.0 {
            text.line_height
        } else {
            text.size * 1.2
        };
        let doc = self.doc;
        let mut geo = Vec::new();
        for (row, line) in text.text.split(['\r', '\n']).enumerate() {
            let glyphs: Vec<Option<&Glyph>> = line
                .chars()
                .map(|c| {
                    let s = c.to_string();
                    doc.glyphs
                        .iter()
                        .find(|g| g.ch == s && g.font == text.font)
                        .or_else(|| doc.glyphs.iter().find(|g| g.ch == s))
                })
                .collect();
            let advance = |g: &Option<&Glyph>| {
                g.map(|g| g.advance * scale).unwrap_or(text.size * 0.3)
                    + text.tracking * text.size / 1000.0
            };
            let width: f32 = glyphs.iter().map(advance).sum();
            let mut x = match text.justify {
                1 => -width,
                2 => -width * 0.5,
                _ => 0.0,
            };
            let y = row as f32 * lh;
            for g in &glyphs {
                if let Some(g) = g {
                    let gt = t.pre_translate(x, y).pre_scale(scale, scale);
                    let mut unused = Vec::new();
                    self.collect(&g.shapes, 0.0, gt, alpha, blend, &mut geo, &mut unused);
                }
                x += advance(g);
            }
        }
        let paint = self.solid(&text.fill, blend);
        let d = Draw {
            style: Style::Fill {
                paint,
                rule: FillRule::Winding,
            },
            geo,
            t,
            opacity: alpha,
        };
        self.paint(&d, mask, target);
    }
}

fn pct(v: f32) -> f32 {
    (v / 100.0).clamp(0.0, 1.0)
}

fn rule(even_odd: bool) -> FillRule {
    if even_odd {
        FillRule::EvenOdd
    } else {
        FillRule::Winding
    }
}

/// Trim every outline gathered: each one on its own ("simultaneously"), or all of them as
/// one path end to end ("individually").
fn trim_all(geo: &mut [Geo], s: f32, e: f32, o: f32, individually: bool) {
    if !individually {
        for g in geo.iter_mut() {
            g.contours = g
                .contours
                .iter()
                .flat_map(|c| geom::trim(std::slice::from_ref(c), s, e, o))
                .collect();
        }
        return;
    }
    let lengths: Vec<f32> = geo.iter().map(|g| geom::length(&g.contours)).collect();
    let total: f32 = lengths.iter().sum();
    if total <= 0.0 {
        return;
    }
    let (mut s, mut e) = (s.min(e), s.max(e));
    if e - s >= 1.0 - 1e-6 {
        return;
    }
    s += o.rem_euclid(1.0);
    e += o.rem_euclid(1.0);
    let mut at = 0.0f32;
    for (g, len) in geo.iter_mut().zip(lengths) {
        let (a, b) = (at / total, (at + len) / total);
        at += len;
        if len <= 0.0 {
            continue;
        }
        let local = |x: f32| ((x - a) / (b - a)).clamp(0.0, 1.0);
        let mut parts = geom::trim(&g.contours, local(s), local(e), 0.0);
        if e > 1.0 {
            parts.extend(geom::trim(&g.contours, local(0.0), local(e - 1.0), 0.0));
        }
        g.contours = parts;
    }
}

fn stroke_at(s: &LStroke, f: f32) -> Stroke {
    let mut out = Stroke {
        width: s.width.at1(f, 1.0).max(0.0),
        miter_limit: s.miter,
        line_cap: match s.cap {
            1 => LineCap::Butt,
            3 => LineCap::Square,
            _ => LineCap::Round,
        },
        line_join: match s.join {
            1 => LineJoin::Miter,
            3 => LineJoin::Bevel,
            _ => LineJoin::Round,
        },
        dash: None,
    };
    if !s.dashes.is_empty() {
        let mut v: Vec<f32> = s.dashes.iter().map(|d| d.at1(f, 0.0).max(0.0)).collect();
        if v.len() % 2 == 1 {
            let again = v.clone();
            v.extend(again);
        }
        let offset = s.dash_offset.as_ref().map(|o| o.at1(f, 0.0)).unwrap_or(0.0);
        out.dash = StrokeDash::new(v, offset);
    }
    out
}

/// Two masks as one (their product), or whichever there is.
fn combine(a: Option<&Mask>, b: Option<Mask>) -> Option<Mask> {
    match (a, b) {
        (None, b) => b,
        (Some(_), None) => None,
        (Some(a), Some(mut b)) => {
            for (x, y) in b.data_mut().iter_mut().zip(a.data()) {
                *x = ((u16::from(*x) * u16::from(*y) + 127) / 255) as u8;
            }
            Some(b)
        }
    }
}

fn layer_masks(l: &Layer, lt: f32, t: Transform, w: u32, h: u32) -> Option<Mask> {
    let mut acc: Option<Vec<u8>> = None;
    for m in &l.masks {
        if m.mode == 'n' {
            continue;
        }
        let mut one = Mask::new(w, h)?;
        let mut pb = PathBuilder::new();
        geom::to_path(&[m.path.at(lt)], Transform::identity(), &mut pb);
        if let Some(p) = pb.finish() {
            one.fill_path(&p, FillRule::Winding, true, t);
        }
        if m.inverted {
            one.invert();
        }
        let o = (m.opacity.at1(lt, 100.0) / 100.0).clamp(0.0, 1.0);
        let full = || vec![255u8; (w * h) as usize];
        let cur = acc.take();
        let mut out = match (m.mode, cur) {
            ('s', None) | ('i', None) => full(),
            (_, Some(c)) => c,
            (_, None) => vec![0u8; (w * h) as usize],
        };
        for (d, s) in out.iter_mut().zip(one.data()) {
            let s = f32::from(*s) / 255.0 * o;
            let a = f32::from(*d) / 255.0;
            let r = match m.mode {
                's' => a * (1.0 - s),
                'i' => a * s,
                'l' => a.min(s),
                'd' => (a - s).abs(),
                _ => a + s - a * s,
            };
            *d = (r * 255.0 + 0.5) as u8;
        }
        acc = Some(out);
    }
    let mut mask = Mask::new(w, h)?;
    mask.data_mut().copy_from_slice(&acc?);
    Some(mask)
}

fn blend_mode(bm: u8) -> BlendMode {
    match bm {
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Darken,
        5 => BlendMode::Lighten,
        6 => BlendMode::ColorDodge,
        7 => BlendMode::ColorBurn,
        8 => BlendMode::HardLight,
        9 => BlendMode::SoftLight,
        10 => BlendMode::Difference,
        11 => BlendMode::Exclusion,
        12 => BlendMode::Hue,
        13 => BlendMode::Saturation,
        14 => BlendMode::Color,
        15 => BlendMode::Luminosity,
        16 => BlendMode::Plus,
        _ => BlendMode::SourceOver,
    }
}

fn grow(b: &mut [f32; 4], r: Rect) {
    b[0] = b[0].min(r.left());
    b[1] = b[1].min(r.top());
    b[2] = b[2].max(r.right());
    b[3] = b[3].max(r.bottom());
}
