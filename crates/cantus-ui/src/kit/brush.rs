//! The brush: every shape the kit paints goes through here, into **one mesh** per flush,
//! in painter's order, sampling one texture — the kit's layer atlas, whose white corner
//! serves flat geometry and whose other cells hold cached layers (knob bodies, glows,
//! logos). Text is deferred and emitted after the geometry of the same flush, so a view
//! that paints through one brush costs two draw calls, whatever it holds.
//!
//! The tessellator is analytic and small: rounded rectangles (and circles, which are
//! rounded squares) are built as concentric outlines, filled row by row so a gradient
//! can have as many stops as the lighting recipe wants, and feathered by one physical
//! pixel **centred on the edge** — so an edge that lies on the pixel grid is crisp, the
//! way epaint's own shapes are. Nothing allocates once the buffers have grown: meshes
//! handed to epaint come back to a pool once epaint has tessellated the frame.

use egui::epaint::{Mesh, TextShape, Vertex};
use egui::{Color32, Context, Galley, Painter, Pos2, Rect, Shape, TextureId, Vec2};
use std::collections::VecDeque;
use std::sync::Arc;

/// Empty a mesh and keep its buffers. (epaint 0.36's `Mesh::clear` frees the vertex
/// buffer despite its documentation, which would make every frame allocate.)
fn reset(m: &mut Mesh) {
    m.indices.clear();
    m.vertices.clear();
    m.texture_id = TextureId::default();
}

/// How a shape is filled: flat, a vertical two-colour ramp, or vertical stops
/// (`t` 0…1 from the shape's top to its bottom).
#[derive(Copy, Clone, Debug)]
pub enum Fill<'a> {
    Solid(Color32),
    V(Color32, Color32),
    Stops(&'a [(f32, Color32)]),
}

impl Fill<'_> {
    fn at(&self, t: f32) -> Color32 {
        match *self {
            Fill::Solid(c) => c,
            Fill::V(a, b) => a.lerp_to_gamma(b, t.clamp(0.0, 1.0)),
            Fill::Stops(s) => {
                let t = t.clamp(0.0, 1.0);
                let Some(first) = s.first() else {
                    return Color32::TRANSPARENT;
                };
                if t <= first.0 {
                    return first.1;
                }
                for w in s.windows(2) {
                    let ((t0, c0), (t1, c1)) = (w[0], w[1]);
                    if t <= t1 {
                        let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
                        return c0.lerp_to_gamma(c1, f);
                    }
                }
                s[s.len() - 1].1
            }
        }
    }

    fn stops(&self) -> &[(f32, Color32)] {
        match self {
            Fill::Stops(s) => s,
            _ => &[],
        }
    }
}

struct Deferred {
    pos: Pos2,
    galley: Arc<Galley>,
    color: Color32,
    angle: f32,
}

/// See the module docs. Get one with [`with_brush`](super::with_brush) or own one per
/// view; paint; [`flush`](Brush::flush) into a painter.
pub struct Brush {
    /// The geometry being painted: a plain mesh, so a vertex push is a `Vec` push.
    mesh: Mesh,
    /// Meshes handed to epaint; once epaint has dropped its clone, a later flush of a
    /// size that fits refills one, so a warm brush never allocates.
    pool: VecDeque<Arc<Mesh>>,
    cache: Option<Arc<std::sync::Mutex<super::cache::KitCache>>>,
    /// The last cached layer looked up, valid until the next `begin`.
    memo: (u64, Rect),
    texts: Vec<Deferred>,
    ring: Vec<Pos2>,
    foreign: Vec<(TextureId, Rect)>,
    ppp: f32,
    tex: TextureId,
    white: Pos2,
    ctx: Option<Context>,
    /// Inspect mode (read once per `begin`): register what is painted.
    inspecting: bool,
    items: Vec<super::inspect::Item>,
    pass: u64,
    /// Meshes handed to epaint since the brush was made (for the frame-budget counters).
    pub flushes: u64,
}

impl Default for Brush {
    fn default() -> Self {
        Brush {
            mesh: Mesh::default(),
            pool: VecDeque::new(),
            cache: None,
            memo: (0, Rect::NOTHING),
            texts: Vec::new(),
            ring: Vec::new(),
            foreign: Vec::new(),
            ppp: 1.0,
            tex: TextureId::default(),
            white: egui::epaint::WHITE_UV,
            ctx: None,
            inspecting: false,
            items: Vec::new(),
            pass: 0,
            flushes: 0,
        }
    }
}

impl Brush {
    /// Start painting for `ctx`: its pixels per point, its layer atlas. Anything not yet
    /// flushed is kept.
    pub fn begin(&mut self, ctx: &Context) {
        let cache = super::cache::handle(ctx);
        let (tex, white) = {
            let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
            c.begin_pass(ctx);
            self.inspecting = c.inspect.is_some();
            c.atlas(ctx)
        };
        self.pass = ctx.cumulative_pass_nr();
        self.cache = Some(cache);
        self.begin_with(ctx, ctx.pixels_per_point(), tex, white);
    }

    /// Run `f` with the kit cache of the context this brush paints for — without going
    /// through the context's data map, which is what makes it cheap enough per part.
    pub fn with_cache<R>(
        &mut self,
        f: impl FnOnce(&Context, &mut super::cache::KitCache) -> R,
    ) -> R {
        let ctx = self.ctx().clone();
        if self.cache.is_none() {
            self.cache = Some(super::cache::handle(&ctx));
        }
        let cache = self.cache.as_ref().expect("set above");
        let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
        f(&ctx, &mut c)
    }

    /// The uv of the cached layer `key`, drawing it with `draw` if it is new: the
    /// brush remembers the last one, so a row of identical knobs asks the cache once.
    pub fn layer(
        &mut self,
        key: u64,
        w: usize,
        h: usize,
        draw: impl FnOnce(&mut tiny_skia::Pixmap),
    ) -> Option<Rect> {
        if self.memo.0 == key && self.memo.1 != Rect::NOTHING {
            return Some(self.memo.1);
        }
        let uv = self.with_cache(|ctx, c| c.layer(ctx, key, w, h, draw))?;
        self.memo = (key, uv);
        Some(uv)
    }

    /// [`begin`](Self::begin) with an explicit scale and atlas (tests, benches).
    pub fn begin_with(&mut self, ctx: &Context, ppp: f32, tex: TextureId, white: Pos2) {
        if tex != self.tex && !self.mesh.is_empty() {
            // A different atlas: the geometry so far belongs to the old one.
            reset(&mut self.mesh);
        }
        self.memo = (0, Rect::NOTHING);
        self.ppp = ppp.max(0.25);
        self.tex = tex;
        self.white = white;
        if self.ctx.as_ref() != Some(ctx) {
            self.ctx = Some(ctx.clone());
        }
    }

    /// The context this brush paints for.
    ///
    /// # Panics
    /// Before the first [`begin`](Self::begin).
    pub fn ctx(&self) -> &Context {
        self.ctx.as_ref().expect("Brush::begin was not called")
    }

    /// Physical pixels per point.
    pub fn ppp(&self) -> f32 {
        self.ppp
    }

    /// One physical pixel, in points: the feather, and a hairline's width.
    pub fn px(&self) -> f32 {
        1.0 / self.ppp
    }

    /// Snap a coordinate to the nearest pixel boundary (edges of fills).
    pub fn snap(&self, v: f32) -> f32 {
        (v * self.ppp).round() / self.ppp
    }

    /// Snap a coordinate to the nearest pixel centre (the axis of a 1-px line).
    pub fn snap_mid(&self, v: f32) -> f32 {
        ((v * self.ppp).floor() + 0.5) / self.ppp
    }

    /// A rect with every edge on a pixel boundary.
    pub fn snap_rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(
            Pos2::new(self.snap(r.min.x), self.snap(r.min.y)),
            Pos2::new(self.snap(r.max.x), self.snap(r.max.y)),
        )
    }

    fn m(&mut self) -> &mut Mesh {
        &mut self.mesh
    }

    /// Vertices written since the last flush.
    pub fn vertex_count(&self) -> usize {
        self.mesh.vertices.len()
    }

    #[inline]
    fn v(&mut self, pos: Pos2, color: Color32) -> u32 {
        let i = self.mesh.vertices.len() as u32;
        self.mesh.vertices.push(Vertex {
            pos,
            uv: self.white,
            color,
        });
        i
    }

    #[inline]
    fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.mesh.indices.extend_from_slice(&[a, b, c]);
    }

    /// Hand the geometry to `painter` as one mesh, then the deferred text on top.
    pub fn flush(&mut self, painter: &Painter) {
        if !self.mesh.is_empty() {
            self.mesh.texture_id = self.tex;
            // The geometry is copied into the pooled mesh epaint has finished with (it
            // drops its clone after tessellating the frame) that fits it best, and the
            // painting buffer stays: a brush that paints flushes of mixed sizes — a
            // toolbar key, then a whole timeline — keeps one buffer as large as its
            // largest flush, and each size finds its own pooled mesh again next frame.
            // (Swapping buffers instead handed each flush the previous one's buffer, and
            // a dense view regrew a megabyte a frame until every buffer was the largest.)
            // The pool only grows while it has nothing free that fits.
            let arc = match self.best_fit() {
                Some(i) => {
                    let mut a = self.pool.remove(i).expect("found above");
                    if let Some(m) = Arc::get_mut(&mut a) {
                        reset(m);
                        m.vertices.extend_from_slice(&self.mesh.vertices);
                        m.indices.extend_from_slice(&self.mesh.indices);
                        m.texture_id = self.mesh.texture_id;
                    }
                    a
                }
                None => Arc::new(Mesh {
                    indices: self.mesh.indices.clone(),
                    vertices: self.mesh.vertices.clone(),
                    texture_id: self.mesh.texture_id,
                }),
            };
            reset(&mut self.mesh);
            painter.add(Shape::Mesh(arc.clone()));
            self.pool.push_back(arc);
            self.flushes += 1;
        }
        if self.inspecting && !self.items.is_empty() {
            let pass = self.pass;
            let items = &mut self.items;
            if let Some(cache) = &self.cache {
                let mut c = cache.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(reg) = c.inspect.as_mut() {
                    reg.extend(pass, items.drain(..));
                }
            }
            self.items.clear();
        }
        for (tex, rect) in self.foreign.drain(..) {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(tex, rect, uv, Color32::WHITE);
        }
        for t in self.texts.drain(..) {
            let mut s = TextShape::new(t.pos, t.galley, t.color);
            if t.angle != 0.0 {
                s = s.with_angle(t.angle);
            }
            painter.add(s);
        }
    }

    /// The pooled mesh epaint no longer holds that fits the geometry being flushed with
    /// the least room to spare; failing that, the largest free one (it grows once).
    fn best_fit(&mut self) -> Option<usize> {
        let (nv, ni) = (self.mesh.vertices.len(), self.mesh.indices.len());
        let mut fit: Option<(usize, usize)> = None;
        let mut largest: Option<(usize, usize)> = None;
        for (i, a) in self.pool.iter_mut().enumerate() {
            let Some(m) = Arc::get_mut(a) else {
                continue;
            };
            let cap = m.vertices.capacity();
            if cap >= nv && m.indices.capacity() >= ni && fit.is_none_or(|(_, c)| cap < c) {
                fit = Some((i, cap));
            }
            if largest.is_none_or(|(_, c)| cap > c) {
                largest = Some((i, cap));
            }
        }
        fit.or(largest).map(|(i, _)| i)
    }

    /// Say what was just painted in `rect` and which tokens coloured it, for inspect
    /// mode ([`inspect`](super::inspect)). One branch when the mode is off.
    #[inline]
    /// `extra` names the tokens that vary by use: the section hue (`s.hue_tok`), a
    /// lamp's colour and ground.
    pub fn register(
        &mut self,
        rect: Rect,
        part: &'static str,
        tokens: &'static [super::Tok],
        extra: &[Option<super::Tok>],
    ) {
        if self.inspecting {
            let mut e = [None; 3];
            for (d, x) in e.iter_mut().zip(extra) {
                *d = *x;
            }
            self.items.push(super::inspect::Item {
                rect,
                part,
                tokens,
                extra: e,
                skin: super::skin::worn().key,
            });
        }
    }

    /// Drop everything painted since the last flush, keeping the buffers (benches, and
    /// tests that measure the painters alone).
    pub fn discard(&mut self) {
        reset(&mut self.mesh);
        self.texts.clear();
        self.foreign.clear();
        self.ring.clear();
    }

    /// Queue a quad of a texture of its own (a logo); it is painted after this flush's
    /// geometry, before its text.
    pub fn foreign(&mut self, tex: TextureId, rect: Rect) {
        self.foreign.push((tex, rect));
    }

    /// Queue a laid-out galley; it is painted after this flush's geometry.
    pub fn text(&mut self, pos: Pos2, galley: Arc<Galley>, color: Color32, angle: f32) {
        self.texts.push(Deferred {
            pos,
            galley,
            color,
            angle,
        });
    }

    /// A quad from the atlas (a cached layer) at `alpha`.
    pub fn layer_quad(&mut self, rect: Rect, uv: Rect, alpha: f32) {
        let a = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        self.image(rect, uv, Color32::from_white_alpha(a));
    }

    /// A quad from the atlas (a cached layer), multiplied by `tint`.
    pub fn image(&mut self, rect: Rect, uv: Rect, tint: Color32) {
        let m = self.m();
        let i = m.vertices.len() as u32;
        for (p, t) in [
            (rect.left_top(), uv.left_top()),
            (rect.right_top(), uv.right_top()),
            (rect.right_bottom(), uv.right_bottom()),
            (rect.left_bottom(), uv.left_bottom()),
        ] {
            m.vertices.push(Vertex {
                pos: p,
                uv: t,
                color: tint,
            });
        }
        m.indices
            .extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
    }

    /// An axis-aligned rect with no feather — for rects already on the pixel grid,
    /// where a feather would only soften the edge.
    pub fn rect(&mut self, r: Rect, fill: Fill<'_>) {
        let (c0, c1) = (fill.at(0.0), fill.at(1.0));
        let stops = fill.stops();
        if stops.len() > 2 {
            let mut prev = (r.top(), c0);
            for &(t, c) in stops.iter().chain(std::iter::once(&(1.0, c1))) {
                let y = r.top() + r.height() * t.clamp(0.0, 1.0);
                if y > prev.0 {
                    self.quad(prev.0, y, r.left(), r.right(), prev.1, c);
                }
                prev = (y, c);
            }
        } else {
            self.quad(r.top(), r.bottom(), r.left(), r.right(), c0, c1);
        }
    }

    /// A radial gradient as plain geometry: an ellipse of `radii` round `center`, coloured
    /// by `stops` (`t` 0 at the centre … 1 at the rim, ascending, at least two), drawn
    /// inside `clip` only. Rings of quads, each a single colour along the ring, so the
    /// GPU and the CPU renderer interpolate it alike and no seam shows where triangles
    /// meet; quads across the clip's edge are clipped exactly (a glow at a corner of its
    /// view). `quarter_steps` sets the outline's smoothness: points per quarter turn.
    pub fn radial(
        &mut self,
        clip: Rect,
        center: Pos2,
        radii: Vec2,
        stops: &[(f32, Color32)],
        quarter_steps: usize,
    ) {
        let full = Rect::from_center_size(center, radii * 2.0);
        if stops.len() < 2 || radii.min_elem() <= 0.0 || !full.intersects(clip) {
            return;
        }
        let q = quarter(quarter_steps.clamp(2, 24));
        let n = q.len() - 1;
        // The unit circle's point `j` of `4n`, counter-clockwise from +x.
        let unit = |j: usize| -> Vec2 {
            let (s, c) = q[j % n];
            match (j / n) % 4 {
                0 => Vec2::new(c, s),
                1 => Vec2::new(-s, c),
                2 => Vec2::new(-c, -s),
                _ => Vec2::new(s, -c),
            }
        };
        let at = |t: f32, j: usize| center + unit(j) * radii * t;
        for w in stops.windows(2) {
            let ((t0, c0), (t1, c1)) = (w[0], w[1]);
            if t1 <= t0 || (c0.a() == 0 && c1.a() == 0) {
                continue;
            }
            for j in 0..4 * n {
                let quad = [at(t0, j), at(t0, j + 1), at(t1, j + 1), at(t1, j)];
                let cols = [c0, c0, c1, c1];
                self.clipped_quad(clip, quad, cols);
            }
        }
    }

    /// A convex quad with a colour per corner, the part inside `clip` only.
    fn clipped_quad(&mut self, clip: Rect, quad: [Pos2; 4], cols: [Color32; 4]) {
        if quad.iter().all(|p| clip.contains(*p)) {
            let i: [u32; 4] = std::array::from_fn(|k| self.v(quad[k], cols[k]));
            self.tri(i[0], i[1], i[2]);
            self.tri(i[0], i[2], i[3]);
            return;
        }
        let (min, max) = quad.iter().fold(
            (Pos2::new(f32::MAX, f32::MAX), Pos2::new(f32::MIN, f32::MIN)),
            |(lo, hi), p| (lo.min(*p), hi.max(*p)),
        );
        if !Rect::from_min_max(min, max).intersects(clip) {
            return;
        }
        // Sutherland–Hodgman against the four sides, on the stack: a quad clipped by a
        // rectangle has at most eight corners.
        let mut a: ([(Pos2, Color32); 8], usize) = ([(Pos2::ZERO, Color32::TRANSPARENT); 8], 4);
        for k in 0..4 {
            a.0[k] = (quad[k], cols[k]);
        }
        let sides: [(u8, f32); 4] = [
            (0, clip.left()),
            (1, clip.right()),
            (2, clip.top()),
            (3, clip.bottom()),
        ];
        for (side, edge) in sides {
            let inside = |p: Pos2| match side {
                0 => p.x >= edge,
                1 => p.x <= edge,
                2 => p.y >= edge,
                _ => p.y <= edge,
            };
            let cross = |p: Pos2, r: Pos2| {
                if side < 2 {
                    (edge - p.x) / (r.x - p.x)
                } else {
                    (edge - p.y) / (r.y - p.y)
                }
            };
            let mut out: ([(Pos2, Color32); 8], usize) =
                ([(Pos2::ZERO, Color32::TRANSPARENT); 8], 0);
            let mut push = |v: (Pos2, Color32)| {
                if out.1 < 8 {
                    out.0[out.1] = v;
                    out.1 += 1;
                }
            };
            for k in 0..a.1 {
                let (p, pc) = a.0[k];
                let (r, rc) = a.0[(k + 1) % a.1];
                match (inside(p), inside(r)) {
                    (true, true) => push((r, rc)),
                    (true, false) => {
                        let f = cross(p, r);
                        push((p + (r - p) * f, pc.lerp_to_gamma(rc, f)));
                    }
                    (false, true) => {
                        let f = cross(p, r);
                        push((p + (r - p) * f, pc.lerp_to_gamma(rc, f)));
                        push((r, rc));
                    }
                    (false, false) => {}
                }
            }
            a = out;
            if a.1 < 3 {
                return;
            }
        }
        let first = self.v(a.0[0].0, a.0[0].1);
        let mut prev = self.v(a.0[1].0, a.0[1].1);
        for k in 2..a.1 {
            let next = self.v(a.0[k].0, a.0[k].1);
            self.tri(first, prev, next);
            prev = next;
        }
    }

    fn quad(&mut self, y0: f32, y1: f32, x0: f32, x1: f32, top: Color32, bottom: Color32) {
        let a = self.v(Pos2::new(x0, y0), top);
        let b = self.v(Pos2::new(x1, y0), top);
        let c = self.v(Pos2::new(x1, y1), bottom);
        let d = self.v(Pos2::new(x0, y1), bottom);
        self.tri(a, b, c);
        self.tri(a, c, d);
    }

    /// A horizontal line `w` physical-pixel-snapped points thick, on the pixel grid.
    pub fn hline(&mut self, x0: f32, x1: f32, y: f32, w: f32, color: Color32) {
        let w = (w * self.ppp).round().max(1.0) / self.ppp;
        let top = self.snap(y - w * 0.5);
        let r = Rect::from_min_max(
            Pos2::new(self.snap(x0), top),
            Pos2::new(self.snap(x1), top + w),
        );
        self.rect(r, Fill::Solid(color));
    }

    /// A vertical line, as [`hline`](Self::hline).
    pub fn vline(&mut self, x: f32, y0: f32, y1: f32, w: f32, color: Color32) {
        let w = (w * self.ppp).round().max(1.0) / self.ppp;
        let left = self.snap(x - w * 0.5);
        let r = Rect::from_min_max(
            Pos2::new(left, self.snap(y0)),
            Pos2::new(left + w, self.snap(y1)),
        );
        self.rect(r, Fill::Solid(color));
    }

    /// Points per corner quarter-circle for a corner of `r` points.
    fn corner_steps(&self, r: f32) -> usize {
        ((r * self.ppp / 1.5).ceil() as usize).clamp(1, 24)
    }
}

use std::f32::consts::FRAC_PI_2;

/// (sin, cos) of `j / n · 90°` for `j = 0…n`, for every `n` a corner can use: the
/// rounded outlines read them instead of calling `sin_cos` per point.
fn quarter(n: usize) -> &'static [(f32, f32)] {
    static TABLES: std::sync::OnceLock<Vec<Vec<(f32, f32)>>> = std::sync::OnceLock::new();
    let t = TABLES.get_or_init(|| {
        (0..=24usize)
            .map(|n| {
                let nf = n.max(1) as f32;
                (0..=n)
                    .map(|j| (j as f32 / nf * FRAC_PI_2).sin_cos())
                    .collect()
            })
            .collect()
    });
    &t[n.min(24)]
}

impl Brush {
    /// Append the closed outline of `rect` grown by `d` (corner radius `r + d`, never
    /// below zero), `n` steps per corner, clockwise from the left end of the top-left
    /// corner. Every outline with the same `n` has the same number of points, so two
    /// of them can be stitched into a ring.
    fn push_outline(&mut self, rect: Rect, r: f32, d: f32, n: usize) {
        let rr = (r + d).max(0.0);
        let (x0, y0, x1, y1) = (
            rect.min.x - d,
            rect.min.y - d,
            rect.max.x + d,
            rect.max.y + d,
        );
        let (cl, cr, ct, cb) = (x0 + rr, x1 - rr, y0 + rr, y1 - rr);
        let q = quarter(n);
        for &(s, c) in q.iter().rev() {
            self.ring.push(Pos2::new(cl - rr * s, ct - rr * c));
        }
        for &(s, c) in q {
            self.ring.push(Pos2::new(cr + rr * s, ct - rr * c));
        }
        for &(s, c) in q.iter().rev() {
            self.ring.push(Pos2::new(cr + rr * s, cb + rr * c));
        }
        for &(s, c) in q {
            self.ring.push(Pos2::new(cl - rr * s, cb + rr * c));
        }
    }

    /// Stitch outlines already in `self.ring` (each `m` points, `k` of them) into
    /// closed strips, colouring each outline's vertices with `color(outline, point)`.
    fn stitch(&mut self, k: usize, m: usize, closed: bool, color: impl Fn(usize, Pos2) -> Color32) {
        let base = self.mesh.vertices.len() as u32;
        for o in 0..k {
            for i in 0..m {
                let p = self.ring[o * m + i];
                let c = color(o, p);
                self.v(p, c);
            }
        }
        let seg = if closed { m } else { m.saturating_sub(1) };
        for o in 0..k.saturating_sub(1) {
            let a = base + (o * m) as u32;
            let b = base + ((o + 1) * m) as u32;
            for i in 0..seg {
                let j = ((i + 1) % m) as u32;
                let i = i as u32;
                self.tri(a + i, a + j, b + j);
                self.tri(a + i, b + j, b + i);
            }
        }
        self.ring.clear();
    }

    /// A rounded rect, feathered by one physical pixel centred on its edge.
    pub fn rrect(&mut self, rect: Rect, r: f32, fill: Fill<'_>) {
        let f = self.px();
        self.rrect_soft(rect, r, fill, f);
    }

    /// A rounded rect whose edge fades over `feather` points (centred on the edge): the
    /// anti-aliased edge when `feather` is one pixel, a soft shadow when it is more.
    pub fn rrect_soft(&mut self, rect: Rect, r: f32, fill: Fill<'_>, feather: f32) {
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            return;
        }
        let r = r.clamp(0.0, rect.width().min(rect.height()) * 0.5);
        let h = feather * 0.5;
        let n = self.corner_steps(r + h);
        let (top, height) = (rect.top(), rect.height().max(1e-6));
        let at = |y: f32| fill.at((y - top) / height);

        // The interior, row by row, on the outline pulled in by half the feather.
        let inner = rect.shrink(h);
        if inner.width() > 0.0 && inner.height() > 0.0 {
            let rr = (r - h).max(0.0);
            let (cl, cr) = (inner.left() + rr, inner.right() - rr);
            let (ct, cb) = (inner.top() + rr, inner.bottom() - rr);
            let mut prev: Option<(u32, u32)> = None;
            let mut row = |b: &mut Brush, xl: f32, xr: f32, y: f32| {
                let c = at(y);
                let l = b.v(Pos2::new(xl, y), c);
                let rgt = b.v(Pos2::new(xr, y), c);
                if let Some((pl, pr)) = prev {
                    b.tri(pl, pr, rgt);
                    b.tri(pl, rgt, l);
                }
                prev = Some((l, rgt));
            };
            let q = quarter(n);
            for &(s, c) in q {
                row(self, cl - rr * s, cr + rr * s, ct - rr * c);
            }
            for &(t, _) in fill.stops() {
                let y = top + height * t;
                if y > ct && y < cb {
                    row(self, inner.left(), inner.right(), y);
                }
            }
            for &(s, c) in q.iter().rev() {
                row(self, cl - rr * s, cr + rr * s, cb + rr * c);
            }
        }

        // The feather: the pulled-in outline in the fill, the pushed-out one clear.
        let m = 4 * (n + 1);
        self.push_outline(rect, r, -h, n);
        self.push_outline(rect, r, h, n);
        self.stitch(2, m, true, |o, p| {
            if o == 0 {
                at(p.y)
            } else {
                Color32::TRANSPARENT
            }
        });
    }

    /// The offsets and colours of a stroke `w` wide with a one-pixel feather.
    fn stroke_profile(&self, w: f32, color: Color32) -> ([f32; 4], [Color32; 4], usize) {
        let f = self.px();
        if w > f {
            let (a, b) = (w * 0.5 + f * 0.5, w * 0.5 - f * 0.5);
            (
                [-a, -b, b, a],
                [Color32::TRANSPARENT, color, color, Color32::TRANSPARENT],
                4,
            )
        } else {
            let thin = color.gamma_multiply((w / f).clamp(0.0, 1.0));
            (
                [-f, 0.0, f, 0.0],
                [Color32::TRANSPARENT, thin, Color32::TRANSPARENT, color],
                3,
            )
        }
    }

    /// A rounded rect's outline, `w` wide, centred on `rect`'s edge.
    pub fn rrect_stroke(&mut self, rect: Rect, r: f32, w: f32, color: Color32) {
        let r = r.clamp(0.0, rect.width().min(rect.height()) * 0.5);
        let (offs, cols, k) = self.stroke_profile(w, color);
        let n = self.corner_steps(r + w);
        for d in &offs[..k] {
            self.push_outline(rect, r, *d, n);
        }
        self.stitch(k, 4 * (n + 1), true, |o, _| cols[o]);
    }

    /// A disc.
    pub fn circle(&mut self, c: Pos2, r: f32, fill: Fill<'_>) {
        match fill {
            Fill::Solid(color) => self.dot(c, r, color),
            _ => self.rrect(Rect::from_center_size(c, Vec2::splat(2.0 * r)), r, fill),
        }
    }

    /// A flat disc: a feathered polygon with about one side per 2.5 physical pixels of
    /// its circumference (round caps, LED cores, specks).
    pub fn dot(&mut self, c: Pos2, r: f32, color: Color32) {
        if r <= 0.0 {
            return;
        }
        let n = ((std::f32::consts::TAU * r * self.ppp / 2.5).ceil() as usize).clamp(6, 96);
        let h = self.px() * 0.5;
        let base = self.mesh.vertices.len() as u32;
        let step = std::f32::consts::TAU / n as f32;
        let (sin, cos) = step.sin_cos();
        let (mut dx, mut dy) = (1.0f32, 0.0f32);
        for _ in 0..n {
            let (ri, ro) = ((r - h).max(0.0), r + h);
            self.v(Pos2::new(c.x + dx * ri, c.y + dy * ri), color);
            self.v(
                Pos2::new(c.x + dx * ro, c.y + dy * ro),
                Color32::TRANSPARENT,
            );
            let nx = dx * cos - dy * sin;
            dy = dx * sin + dy * cos;
            dx = nx;
        }
        for i in 0..n as u32 {
            let j = (i + 1) % n as u32;
            let (a, ao, b2, bo) = (
                base + 2 * i,
                base + 2 * i + 1,
                base + 2 * j,
                base + 2 * j + 1,
            );
            self.tri(a, ao, bo);
            self.tri(a, bo, b2);
            if i > 0 && i + 1 < n as u32 {
                self.tri(base, a, b2);
            }
        }
    }

    /// A ring `w` wide centred on radius `r`.
    pub fn circle_stroke(&mut self, c: Pos2, r: f32, w: f32, color: Color32) {
        self.rrect_stroke(Rect::from_center_size(c, Vec2::splat(2.0 * r)), r, w, color);
    }

    /// An arc from angle `a0` to `a1` (radians, screen coordinates: 0 points right,
    /// π/2 down), `w` wide, centred on radius `r`; `caps` rounds its ends.
    #[allow(clippy::too_many_arguments)]
    pub fn arc(&mut self, c: Pos2, r: f32, a0: f32, a1: f32, w: f32, color: Color32, caps: bool) {
        let span = (a1 - a0).abs();
        if span < 1e-4 {
            return;
        }
        // One segment per ~3 physical pixels of arc (rendering-performance.md §5.1).
        let n = ((span * r * self.ppp / 3.0).ceil() as usize).clamp(2, 256);
        let (offs, cols, k) = self.stroke_profile(w, color);
        // The unit directions, once, by rotating a vector: one `sin_cos` for the arc,
        // not one per point per ring.
        let (sin, cos) = ((a1 - a0) / n as f32).sin_cos();
        let start = self.ring.len();
        let mut dir = Vec2::angled(a0);
        for _ in 0..=n {
            self.ring.push(Pos2::new(dir.x, dir.y));
            dir = Vec2::new(dir.x * cos - dir.y * sin, dir.x * sin + dir.y * cos);
        }
        for d in &offs[..k] {
            let rad = r + d;
            for i in 0..=n {
                let u = self.ring[start + i];
                self.ring.push(Pos2::new(c.x + u.x * rad, c.y + u.y * rad));
            }
        }
        self.ring.drain(start..start + n + 1);
        self.stitch(k, n + 1, false, |o, _| cols[o]);
        if caps {
            for a in [a0, a1] {
                self.dot(c + r * Vec2::angled(a), w * 0.5, color);
            }
        }
    }

    /// A straight line from `a` to `b`, `w` wide, feathered along its sides; `caps`
    /// rounds its ends.
    pub fn segment(&mut self, a: Pos2, b: Pos2, w: f32, color: Color32, caps: bool) {
        let d = b - a;
        let len = d.length();
        if len < 1e-5 {
            return;
        }
        let n = Vec2::new(-d.y, d.x) / len;
        let (offs, cols, k) = self.stroke_profile(w, color);
        for o in &offs[..k] {
            self.ring.push(a + n * *o);
            self.ring.push(b + n * *o);
        }
        self.stitch(k, 2, false, |o, _| cols[o]);
        if caps {
            self.dot(a, w * 0.5, color);
            self.dot(b, w * 0.5, color);
        }
    }

    /// An open polyline (curves in displays), `w` wide, mitred joins.
    pub fn polyline(&mut self, pts: &[Pos2], w: f32, color: Color32) {
        let m = pts.len();
        if m < 2 {
            return;
        }
        let (offs, cols, k) = self.stroke_profile(w, color);
        for o in &offs[..k] {
            for i in 0..m {
                let n = miter(pts, i, false);
                self.ring.push(pts[i] + n * *o);
            }
        }
        self.stitch(k, m, false, |o, _| cols[o]);
    }

    /// An open polyline `w` wide in `color` with a glow round it: `glow` at the core's
    /// edge, fading to nothing `reach` points out — one strip of six rings (a CRT's beam,
    /// a VFD's light) where layering two wider lines under the core took twelve.
    pub fn glow_polyline(
        &mut self,
        pts: &[Pos2],
        w: f32,
        color: Color32,
        glow: Color32,
        reach: f32,
    ) {
        let m = pts.len();
        if m < 2 {
            return;
        }
        let f = self.px();
        let core = (w * 0.5 - f * 0.5).max(0.0);
        let edge = w * 0.5 + f * 0.5;
        let out = edge + reach.max(f);
        let core_c = if w > f {
            color
        } else {
            color.gamma_multiply((w / f).clamp(0.0, 1.0))
        };
        let offs = [-out, -edge, -core, core, edge, out];
        let cols = [
            Color32::TRANSPARENT,
            glow,
            core_c,
            core_c,
            glow,
            Color32::TRANSPARENT,
        ];
        for o in &offs {
            for i in 0..m {
                let n = miter(pts, i, false);
                self.ring.push(pts[i] + n * *o);
            }
        }
        self.stitch(6, m, false, |o, _| cols[o]);
    }

    /// A dashed open polyline: pieces `dash` long with `gap` between them, carried across
    /// the polyline's joins (a key cable, a switched-off send, an FM graph's feedback).
    /// Nothing is allocated: each piece is a [`Brush::segment`].
    pub fn dashed(&mut self, pts: &[Pos2], w: f32, color: Color32, dash: f32, gap: f32) {
        let (dash, gap) = (dash.max(0.5), gap.max(0.5));
        let mut on = true;
        let mut left = dash;
        for seg in pts.windows(2) {
            let (mut a, z) = (seg[0], seg[1]);
            let mut len = a.distance(z);
            if len < 1e-5 {
                continue;
            }
            let dir = (z - a) / len;
            while len > 1e-5 {
                let step = len.min(left);
                let p = a + dir * step;
                if on {
                    self.segment(a, p, w, color, false);
                }
                a = p;
                len -= step;
                left -= step;
                if left <= 1e-5 {
                    on = !on;
                    left = if on { dash } else { gap };
                }
            }
        }
    }

    /// The area between a polyline and the horizontal line at `base` (a curve's fill in
    /// a display), as one quad per segment. Not feathered: its edges are the curve's
    /// stroke and the display's axis.
    pub fn area(&mut self, pts: &[Pos2], base: f32, color: Color32) {
        for w in pts.windows(2) {
            let (a, b) = (w[0], w[1]);
            let i0 = self.v(a, color);
            let i1 = self.v(b, color);
            let i2 = self.v(Pos2::new(b.x, base), color);
            let i3 = self.v(Pos2::new(a.x, base), color);
            self.tri(i0, i1, i2);
            self.tri(i0, i2, i3);
        }
    }

    /// The area between two polylines sampled at the same points (a dynamic EQ band's
    /// reach, the band between two decay curves), as one quad per segment. Not feathered, as [`Brush::area`].
    pub fn between(&mut self, top: &[Pos2], bottom: &[Pos2], color: Color32) {
        let n = top.len().min(bottom.len());
        for i in 1..n {
            let i0 = self.v(top[i - 1], color);
            let i1 = self.v(top[i], color);
            let i2 = self.v(bottom[i], color);
            let i3 = self.v(bottom[i - 1], color);
            self.tri(i0, i1, i2);
            self.tri(i0, i2, i3);
        }
    }

    /// A filled convex polygon (glyphs: chevrons, arrows), feathered.
    pub fn polygon(&mut self, pts: &[Pos2], color: Color32) {
        let m = pts.len();
        if m < 3 {
            return;
        }
        let h = self.px() * 0.5;
        // Outward normals whichever way the points wind.
        let area: f32 = (0..m)
            .map(|i| {
                let (p, q) = (pts[i], pts[(i + 1) % m]);
                p.x * q.y - q.x * p.y
            })
            .sum();
        let sign = if area < 0.0 { 1.0 } else { -1.0 };
        for off in [-h, h] {
            for i in 0..m {
                let n = miter(pts, i, true) * sign;
                self.ring.push(pts[i] + n * off);
            }
        }
        let base = self.mesh.vertices.len() as u32;
        self.stitch(2, m, true, |o, _| {
            if o == 0 {
                color
            } else {
                Color32::TRANSPARENT
            }
        });
        for i in 1..(m as u32 - 1) {
            self.tri(base, base + i, base + i + 1);
        }
    }
}

/// The mitred normal at point `i` of a polyline (unit normals averaged, lengthened so
/// the offset line stays parallel to both segments).
fn miter(pts: &[Pos2], i: usize, closed: bool) -> Vec2 {
    let m = pts.len();
    let norm = |a: Pos2, b: Pos2| {
        let d = (b - a).normalized();
        Vec2::new(-d.y, d.x)
    };
    let prev = if i > 0 {
        Some(norm(pts[i - 1], pts[i]))
    } else if closed {
        Some(norm(pts[m - 1], pts[0]))
    } else {
        None
    };
    let next = if i + 1 < m {
        Some(norm(pts[i], pts[i + 1]))
    } else if closed {
        Some(norm(pts[m - 1], pts[0]))
    } else {
        None
    };
    match (prev, next) {
        (Some(a), Some(b)) => {
            let n = (a + b) * 0.5;
            let l2 = n.length_sq();
            if l2 < 0.1 {
                a
            } else {
                n / l2
            }
        }
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => Vec2::ZERO,
    }
}
