//! Shapes and the family's way of painting a part (ADR 0090): an outline of **varying
//! weight** (thin where the light falls, heavier at the bottom right, where it would cast a
//! shadow), **cel shading** from the family's key light (upper left) — a shade band round
//! the far side, the base, a light patch — and a **rim** of light on the far edge of the
//! big round parts: the look of today's 2D animated characters, from vectors, crisp at
//! any scale.
//!
//! **Speed.** Every band is an *opaque solid* fill: tiny-skia fills those several times
//! faster than a gradient or a translucent colour (measured: 0.045 ms against 0.3 ms for a
//! body-sized ellipse on a 600-pixel stage), which is what holds a character within its
//! 1.5 ms. Gradients are kept for the soft contact shadow alone.
//!
//! **Allocation.** A shape is a function `(sink, grow)` that adds its sub-paths to a sink
//! (`grow` widens it by that much: its outline is the shape grown by the ink). It is drawn
//! through a [`Pen`], which maps every point by the canvas's transform into a path builder
//! taken from a per-thread pool, so tiny-skia fills it as it is (with a transform it would
//! clone the path first); the cel bands transform that same path in place; lines are
//! ribbons, never strokes. A frame allocates only what tiny-skia allocates inside a fill
//! (`tests/alloc.rs` counts it).
//!
//! Compound parts are one path of several sub-paths (a body and a head, three toes): their
//! outline is drawn first and their fill over it, so where sub-paths overlap no line shows
//! and the whole reads as one silhouette.

use super::{lerp, P};
use crate::render::Canvas;
use std::cell::RefCell;
use tiny_skia::{Color, FillRule, Paint, Path, PathBuilder, Transform};

/// A part's colours (straight RGBA; turned by the pose's hue when painted).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Look {
    pub base: [u8; 4],
    pub shade: [u8; 4],
    pub light: [u8; 4],
    /// The rim light's colour; alpha 0 for none.
    pub rim: [u8; 4],
}

/// An outline: its colour (never turned by the hue) and its weight in the character's units.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ink {
    pub color: [u8; 4],
    pub width: f32,
}

impl Ink {
    pub fn thin(self, k: f32) -> Ink {
        Ink {
            width: self.width * k,
            ..self
        }
    }
}

/// Mix two straight colours.
pub fn mix(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let f = |x: u8, y: u8| {
        lerp(f32::from(x), f32::from(y), t)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3])]
}

/// The same colour at `alpha` (0…1) of its own alpha.
pub fn alpha(c: [u8; 4], alpha: f32) -> [u8; 4] {
    [
        c[0],
        c[1],
        c[2],
        (f32::from(c[3]) * alpha.clamp(0.0, 1.0)).round() as u8,
    ]
}

// ---- Where shapes go ----

/// What takes path commands: a path builder, or a [`Pen`] that maps them first.
pub trait Sink {
    fn move_to(&mut self, x: f32, y: f32);
    fn line_to(&mut self, x: f32, y: f32);
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32);
    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32);
    fn close(&mut self);
}

impl Sink for PathBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        PathBuilder::move_to(self, x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        PathBuilder::line_to(self, x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        PathBuilder::quad_to(self, x1, y1, x, y);
    }
    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        PathBuilder::cubic_to(self, x1, y1, x2, y2, x, y);
    }
    fn close(&mut self) {
        PathBuilder::close(self);
    }
}

/// A sink that maps every point by a transform before passing it on: several parts with
/// their own placements (two wings) drawn as one shape, one fill per layer.
pub struct Mapped<'a> {
    pub inner: &'a mut dyn Sink,
    pub t: Transform,
}

impl Mapped<'_> {
    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        let t = &self.t;
        (t.sx * x + t.kx * y + t.tx, t.ky * x + t.sy * y + t.ty)
    }
}

impl Sink for Mapped<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.map(x, y);
        self.inner.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.map(x, y);
        self.inner.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = self.map(x1, y1);
        let (x, y) = self.map(x, y);
        self.inner.quad_to(a, b, x, y);
    }
    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.map(x1, y1);
        let (c, d) = self.map(x2, y2);
        let (x, y) = self.map(x, y);
        self.inner.cubic_to(a, b, c, d, x, y);
    }
    fn close(&mut self) {
        self.inner.close();
    }
}

/// A shape: adds its sub-paths to the sink, grown outward by the second argument.
pub type ShapeFn<'a> = &'a dyn Fn(&mut dyn Sink, f32);

thread_local! {
    /// Path builders (their buffers) kept between frames on each drawing thread.
    static POOL: RefCell<Vec<PathBuilder>> = const { RefCell::new(Vec::new()) };
}

/// Path commands in a character's units, mapped by a transform (the canvas's) into a
/// builder from the per-thread pool.
pub struct Pen {
    pb: PathBuilder,
    t: Transform,
}

impl Pen {
    pub fn new(t: Transform) -> Pen {
        let pb = POOL.with(|p| p.borrow_mut().pop()).unwrap_or_default();
        Pen { pb, t }
    }

    /// A pen drawing with the canvas's own transform.
    pub fn on(c: &Canvas) -> Pen {
        Pen::new(c.transform)
    }

    fn map(&self, x: f32, y: f32) -> (f32, f32) {
        let t = &self.t;
        (t.sx * x + t.kx * y + t.tx, t.ky * x + t.sy * y + t.ty)
    }

    /// The path drawn (in pixels), going back to the pool when dropped.
    pub fn finish(self) -> Shape {
        // An empty shape (a layer with nothing in it this frame) keeps its builder: finish
        // would drop it, and the next pen would allocate anew.
        if self.pb.is_empty() {
            POOL.with(|p| p.borrow_mut().push(self.pb));
            return Shape(None);
        }
        Shape(self.pb.finish())
    }
}

impl Sink for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.map(x, y);
        self.pb.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.map(x, y);
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, b) = self.map(x1, y1);
        let (x, y) = self.map(x, y);
        self.pb.quad_to(a, b, x, y);
    }
    fn cubic_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (a, b) = self.map(x1, y1);
        let (c, d) = self.map(x2, y2);
        let (x, y) = self.map(x, y);
        self.pb.cubic_to(a, b, c, d, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// A finished path in pixels; its buffers go back to the pool when it is dropped.
pub struct Shape(Option<Path>);

impl Shape {
    pub fn path(&self) -> Option<&Path> {
        self.0.as_ref()
    }

    /// Transform it in place (no copy).
    pub fn transform(&mut self, t: Transform) {
        self.0 = self.0.take().and_then(|p| p.transform(t));
    }

    /// Scale it by `k` about the point at (`fx`, `fy`) of its bounds, in place.
    pub fn shrink_toward(&mut self, k: f32, fx: f32, fy: f32) {
        let Some(r) = self.0.as_ref().map(Path::bounds) else {
            return;
        };
        let a = (r.left() + fx * r.width(), r.top() + fy * r.height());
        self.transform(
            Transform::from_translate(a.0, a.1)
                .pre_scale(k, k)
                .pre_translate(-a.0, -a.1),
        );
    }
}

impl Drop for Shape {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            let pb = p.clear();
            POOL.with(|pool| pool.borrow_mut().push(pb));
        }
    }
}

/// `shape` grown by `grow`, drawn through a pen with transform `t`.
pub fn build(shape: ShapeFn, grow: f32, t: Transform) -> Shape {
    let mut pen = Pen::new(t);
    shape(&mut pen, grow);
    pen.finish()
}

// ---- Shapes ----
//
// Every closed shape here winds the same way (positive area, clockwise on screen): a part
// made of several overlapping sub-paths is filled by the non-zero rule, where sub-paths
// wound against each other would cancel and leave holes.

const KAPPA: f32 = 0.552_284_7;

/// Twice the signed area of a polygon (positive: the rig's winding).
fn area2(pts: &[P]) -> f32 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            a.0 * b.1 - b.0 * a.1
        })
        .sum()
}

/// An ellipse turned by `deg` about its centre.
pub fn ellipse_into(pb: &mut dyn Sink, c: P, rx: f32, ry: f32, deg: f32) {
    let k = KAPPA;
    let p = |x: f32, y: f32| super::rotate((c.0 + x, c.1 + y), c, deg);
    let (a, b) = (rx.max(1e-4), ry.max(1e-4));
    let start = p(a, 0.0);
    pb.move_to(start.0, start.1);
    let quads = [
        ((a, k * b), (k * a, b), (0.0, b)),
        ((-k * a, b), (-a, k * b), (-a, 0.0)),
        ((-a, -k * b), (-k * a, -b), (0.0, -b)),
        ((k * a, -b), (a, -k * b), (a, 0.0)),
    ];
    for (c1, c2, e) in quads {
        let (c1, c2, e) = (p(c1.0, c1.1), p(c2.0, c2.1), p(e.0, e.1));
        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, e.0, e.1);
    }
    pb.close();
}

/// A closed polygon (wound the rig's way whichever way `pts` go).
pub fn polygon_into(pb: &mut dyn Sink, pts: &[P]) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    let back = area2(pts) < 0.0;
    let at = |i: usize| if back { pts[(n - i) % n] } else { pts[i] };
    pb.move_to(at(0).0, at(0).1);
    for i in 1..n {
        pb.line_to(at(i).0, at(i).1);
    }
    pb.close();
}

/// A rectangle centred on `c`, `w` × `h`, turned by `deg`, grown by `g`.
pub fn rect_into(pb: &mut dyn Sink, c: P, w: f32, h: f32, deg: f32, g: f32) {
    let (hw, hh) = (w * 0.5 + g, h * 0.5 + g);
    let pts = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)]
        .map(|(x, y)| super::rotate((c.0 + x, c.1 + y), c, deg));
    polygon_into(pb, &pts);
}

/// Catmull-Rom tangents as cubic control points between `p1` and `p2`.
fn cr(p0: P, p1: P, p2: P, p3: P) -> (P, P) {
    (
        (p1.0 + (p2.0 - p0.0) / 6.0, p1.1 + (p2.1 - p0.1) / 6.0),
        (p2.0 - (p3.0 - p1.0) / 6.0, p2.1 - (p3.1 - p1.1) / 6.0),
    )
}

/// A smooth closed shape through `pts` (Catmull-Rom).
pub fn blob_into(pb: &mut dyn Sink, pts: &[P]) {
    let n = pts.len();
    if n < 3 {
        return;
    }
    // Walked backwards when wound the other way.
    let back = area2(pts) < 0.0;
    let at = |i: usize| {
        if back {
            pts[(n - i % n) % n]
        } else {
            pts[i % n]
        }
    };
    let first = at(0);
    pb.move_to(first.0, first.1);
    for i in 0..n {
        let (c1, c2) = cr(at(i + n - 1), at(i), at(i + 1), at(i + 2));
        let p2 = at(i + 1);
        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, p2.0, p2.1);
    }
    pb.close();
}

/// A smooth closed shape through `pts` (at most 24), grown by `g` (each point pushed out
/// from their centroid: right for the family's round shapes).
pub fn blob_grown_into(pb: &mut dyn Sink, pts: &[P], g: f32) {
    if g == 0.0 {
        return blob_into(pb, pts);
    }
    let k = pts.len().min(24);
    let n = k.max(1) as f32;
    let c = (
        pts[..k].iter().map(|p| p.0).sum::<f32>() / n,
        pts[..k].iter().map(|p| p.1).sum::<f32>() / n,
    );
    let mut grown = [(0.0, 0.0); 24];
    for (o, p) in grown.iter_mut().zip(pts[..k].iter()) {
        let (dx, dy) = (p.0 - c.0, p.1 - c.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        *o = (p.0 + dx / len * g, p.1 + dy / len * g);
    }
    blob_into(pb, &grown[..k]);
}

/// A smooth open line through `pts` (Catmull-Rom, the ends doubled), continuing the
/// current contour (the first point is a `line_to` unless `start`).
pub fn spline_into(pb: &mut dyn Sink, pts: &[P], start: bool) {
    let n = pts.len();
    if n == 0 {
        return;
    }
    if start {
        pb.move_to(pts[0].0, pts[0].1);
    } else {
        pb.line_to(pts[0].0, pts[0].1);
    }
    for i in 0..n.saturating_sub(1) {
        let (c1, c2) = cr(
            pts[i.saturating_sub(1)],
            pts[i],
            pts[i + 1],
            pts[(i + 2).min(n - 1)],
        );
        pb.cubic_to(c1.0, c1.1, c2.0, c2.1, pts[i + 1].0, pts[i + 1].1);
    }
}

/// The point at `t` on the quadratic curve a → ctrl → b, and its unit normal.
fn quad_at(a: P, ctrl: P, b: P, t: f32) -> (P, P) {
    let u = 1.0 - t;
    let p = (
        u * u * a.0 + 2.0 * u * t * ctrl.0 + t * t * b.0,
        u * u * a.1 + 2.0 * u * t * ctrl.1 + t * t * b.1,
    );
    let mut d = (
        2.0 * u * (ctrl.0 - a.0) + 2.0 * t * (b.0 - ctrl.0),
        2.0 * u * (ctrl.1 - a.1) + 2.0 * t * (b.1 - ctrl.1),
    );
    if d.0 * d.0 + d.1 * d.1 < 1e-12 {
        d = (b.0 - a.0, b.1 - a.1);
    }
    let len = (d.0 * d.0 + d.1 * d.1).sqrt().max(1e-6);
    (p, (-d.1 / len, d.0 / len))
}

/// Half a round end: from `from` (on the circle of radius `r` about `centre`) round the
/// point `out` (a unit direction) to the opposite side.
fn cap(pb: &mut dyn Sink, from: P, centre: P, out: P, r: f32) {
    let r = r.max(1e-5);
    let k = KAPPA * r;
    let nf = ((from.0 - centre.0) / r, (from.1 - centre.1) / r);
    let tip = (centre.0 + out.0 * r, centre.1 + out.1 * r);
    let to = (centre.0 - nf.0 * r, centre.1 - nf.1 * r);
    pb.cubic_to(
        from.0 + out.0 * k,
        from.1 + out.1 * k,
        tip.0 + nf.0 * k,
        tip.1 + nf.1 * k,
        tip.0,
        tip.1,
    );
    pb.cubic_to(
        tip.0 - nf.0 * k,
        tip.1 - nf.1 * k,
        to.0 + out.0 * k,
        to.1 + out.1 * k,
        to.0,
        to.1,
    );
}

/// A tube through `mid` (at most 16 points) with unit normals `nrm` and half-widths
/// `half`, round at both ends, wound the rig's way.
fn tube_into(pb: &mut dyn Sink, mid: &[P], nrm: &[P], half: &[f32]) {
    let n = mid.len().min(16);
    if n < 2 {
        return;
    }
    let mut right = [(0.0, 0.0); 16];
    let mut left = [(0.0, 0.0); 16];
    for i in 0..n {
        right[i] = (mid[i].0 - nrm[i].0 * half[i], mid[i].1 - nrm[i].1 * half[i]);
        left[n - 1 - i] = (mid[i].0 + nrm[i].0 * half[i], mid[i].1 + nrm[i].1 * half[i]);
    }
    let tangent = |v: P| (v.1, -v.0);
    spline_into(pb, &right[..n], true);
    cap(
        pb,
        right[n - 1],
        mid[n - 1],
        tangent(nrm[n - 1]),
        half[n - 1],
    );
    spline_into(pb, &left[..n], false);
    let t0 = tangent(nrm[0]);
    cap(pb, left[n - 1], mid[0], (-t0.0, -t0.1), half[0]);
    pb.close();
}

/// A limb: a tube along the curve a → ctrl → b, `w0` wide at `a` and `w1` at `b`, round
/// at both ends (a bent arm, a leg, a feather, a tail).
pub fn limb_into(pb: &mut dyn Sink, a: P, ctrl: P, b: P, w0: f32, w1: f32) {
    const N: usize = 5;
    let mut mid = [(0.0, 0.0); N];
    let mut nrm = [(0.0, 0.0); N];
    let mut half = [0.0; N];
    for i in 0..N {
        let t = i as f32 / (N - 1) as f32;
        let (p, n) = quad_at(a, ctrl, b, t);
        mid[i] = p;
        nrm[i] = n;
        half[i] = lerp(w0, w1, t) * 0.5;
    }
    tube_into(pb, &mid, &nrm, &half);
}

/// A ribbon along the points `pts` (at most 16; a smooth line), `width` wide, round at
/// both ends: a line drawn as one fill.
pub fn ribbon_into(pb: &mut dyn Sink, pts: &[P], width: f32) {
    let n = pts.len().min(16);
    if n < 2 {
        return;
    }
    let mut nrm = [(0.0, 0.0); 16];
    for (i, o) in nrm.iter_mut().enumerate().take(n) {
        let a = pts[i.saturating_sub(1)];
        let b = pts[(i + 1).min(n - 1)];
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        *o = (-dy / len, dx / len);
    }
    tube_into(pb, &pts[..n], &nrm[..n], &[width * 0.5; 16][..n]);
}

// ---- Painting ----

thread_local! {
    /// Fills made on this thread (what `tests/alloc.rs` sets the allocations against).
    static FILLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The fills this thread has made so far.
#[doc(hidden)]
pub fn fills_made() -> usize {
    FILLS.with(std::cell::Cell::get)
}

thread_local! {
    /// Whether this thread is inside tiny-skia's `fill_path` now: what `tests/alloc.rs`
    /// tells tiny-skia's own allocations from the rig's by.
    static IN_FILL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether this thread is inside a fill now (for `tests/alloc.rs`'s counting allocator;
/// a const thread-local: reading it never allocates).
#[doc(hidden)]
pub fn in_fill() -> bool {
    IN_FILL.with(std::cell::Cell::get)
}

fn fill_rule(c: &mut Canvas, s: &Shape, color: Color, rule: FillRule) {
    let Some(p) = s.path() else {
        return;
    };
    FILLS.with(|f| f.set(f.get() + 1));
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    IN_FILL.with(|f| f.set(true));
    c.pixmap
        .fill_path(p, &paint, rule, Transform::identity(), None);
    IN_FILL.with(|f| f.set(false));
}

fn fill_px(c: &mut Canvas, s: &Shape, color: Color) {
    fill_rule(c, s, color, FillRule::Winding);
}

/// The outline of a shape: the shape grown by the ink and nudged toward the bottom right,
/// so the line is thin where the light falls and heavier underneath. One fill.
pub fn outline(c: &mut Canvas, shape: ShapeFn, ink: &Ink) {
    if ink.width <= 0.0 {
        return;
    }
    let w = ink.width;
    let t = c.transform.pre_translate(0.08 * w, 0.18 * w);
    let s = build(shape, 0.6 * w, t);
    let col = c.fixed(ink.color);
    fill_px(c, &s, col);
}

/// A part: its outline and its cel-shaded body (`bands` 3 with a light patch, 2 without,
/// 1 flat).
pub fn part(c: &mut Canvas, shape: ShapeFn, look: &Look, ink: &Ink, bands: u8) {
    outline(c, shape, ink);
    self::bands(c, shape, look, bands);
}

/// The cel bands of a shape (no outline).
pub fn bands(c: &mut Canvas, shape: ShapeFn, look: &Look, bands: u8) {
    let mut s = build(shape, 0.0, c.transform);
    bands_of(c, &mut s, look, bands);
}

/// The cel bands of a drawn path: 1 (the base), 2 (the shade, then the base drawn smaller
/// toward the light, leaving a band of shade widest at the bottom right) or 3 (and a light
/// patch nearer the light). The same path, transformed in place between the fills.
pub fn bands_of(c: &mut Canvas, s: &mut Shape, look: &Look, bands: u8) {
    if bands <= 1 {
        let col = c.color(look.base);
        fill_px(c, s, col);
        return;
    }
    let shade = c.color(look.shade);
    fill_px(c, s, shade);
    s.shrink_toward(0.84, 0.32, 0.26);
    let base = c.color(look.base);
    fill_px(c, s, base);
    if bands >= 3 {
        s.shrink_toward(0.46 / 0.84, 0.26, 0.2);
        let light = c.color(mix(look.base, look.light, 0.7));
        fill_px(c, s, light);
    }
}

/// A flat fill, turned by the hue.
pub fn flat(c: &mut Canvas, shape: ShapeFn, rgba: [u8; 4]) {
    let s = build(shape, 0.0, c.transform);
    let col = c.color(rgba);
    fill_px(c, &s, col);
}

/// A flat fill, never turned (eyes, ink, glass).
pub fn fixed(c: &mut Canvas, shape: ShapeFn, rgba: [u8; 4]) {
    let s = build(shape, 0.0, c.transform);
    let col = c.fixed(rgba);
    fill_px(c, &s, col);
}

/// A flat fill by the even-odd rule (a ring: an outer shape less an inner one), never
/// turned by the hue.
pub fn fixed_even_odd(c: &mut Canvas, shape: ShapeFn, rgba: [u8; 4]) {
    let s = build(shape, 0.0, c.transform);
    let col = c.fixed(rgba);
    fill_rule(c, &s, col, FillRule::EvenOdd);
}

/// A ribbon line in `rgba` (turned by the hue when `hue`).
pub fn ribbon(c: &mut Canvas, pts: &[P], width: f32, rgba: [u8; 4], hue: bool) {
    let mut pen = Pen::on(c);
    ribbon_into(&mut pen, pts, width);
    let s = pen.finish();
    let col = if hue { c.color(rgba) } else { c.fixed(rgba) };
    fill_px(c, &s, col);
}

/// The rim light of a round part: an arc of light just inside the far (bottom-right)
/// edge of the ellipse at `centre`, from `from` to `to` degrees (0 = right, 90 = down).
#[allow(clippy::too_many_arguments)]
pub fn rim_arc(
    c: &mut Canvas,
    centre: P,
    rx: f32,
    ry: f32,
    from: f32,
    to: f32,
    look: &Look,
    ink_width: f32,
) {
    if look.rim[3] == 0 {
        return;
    }
    let inset = ink_width * 1.1;
    let pts: [P; 7] = std::array::from_fn(|k| {
        let a = lerp(from, to, k as f32 / 6.0);
        let (s, co) = (super::sin_deg(a), super::cos_deg(a));
        (centre.0 + (rx - inset) * co, centre.1 + (ry - inset) * s)
    });
    ribbon(c, &pts, ink_width * 0.6, look.rim, true);
}

/// A soft round glow (`rgba` at its centre, fading to nothing at the edge): the contact
/// shadow. Three nested translucent ovals, softest outside — no gradient (a gradient
/// costs several opaque fills and allocates its stops every frame).
pub fn glow(c: &mut Canvas, centre: P, rx: f32, ry: f32, rgba: [u8; 4], hue: bool) {
    for (k, a) in [(1.0, 0.34), (0.72, 0.4), (0.44, 0.5)] {
        let s = build(
            &|p, _| ellipse_into(p, centre, rx * k, ry * k, 0.0),
            0.0,
            c.transform,
        );
        let col = alpha(rgba, a);
        let col = if hue { c.color(col) } else { c.fixed(col) };
        fill_px(c, &s, col);
    }
}

/// A colour as it is drawn on a canvas with no hue (for tests that measure fills).
pub fn as_color(rgba: [u8; 4]) -> Color {
    Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3])
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_skia::Pixmap;

    fn filled_at(build: impl Fn(&mut dyn Sink), x: u32, y: u32) -> bool {
        let mut pm = Pixmap::new(100, 100).expect("a pixmap");
        let mut c = Canvas {
            pixmap: &mut pm,
            transform: Transform::from_scale(100.0, 100.0),
            px_per_unit: 100.0,
            hue: 0.0,
            shadow: true,
        };
        flat(&mut c, &|s, _| build(s), [255, 0, 0, 255]);
        pm.pixel(x, y).is_some_and(|p| p.alpha() == 255)
    }

    #[test]
    fn overlapping_sub_paths_add_up() {
        // An ellipse with a limb, a blob (either way round) and a ribbon over its middle:
        // the overlap is filled, never a hole.
        assert!(filled_at(
            |pb| {
                ellipse_into(pb, (0.5, 0.5), 0.3, 0.3, 0.0);
                limb_into(pb, (0.2, 0.5), (0.5, 0.45), (0.8, 0.5), 0.2, 0.2);
            },
            50,
            50
        ));
        for pts in [
            [(0.3, 0.3), (0.7, 0.3), (0.7, 0.7), (0.3, 0.7)],
            [(0.3, 0.7), (0.7, 0.7), (0.7, 0.3), (0.3, 0.3)],
        ] {
            assert!(filled_at(
                |pb| {
                    ellipse_into(pb, (0.5, 0.5), 0.3, 0.3, 0.0);
                    blob_into(pb, &pts);
                },
                50,
                50
            ));
            assert!(filled_at(
                |pb| {
                    ellipse_into(pb, (0.5, 0.5), 0.3, 0.3, 0.0);
                    polygon_into(pb, &pts);
                },
                50,
                50
            ));
        }
        assert!(filled_at(
            |pb| {
                ellipse_into(pb, (0.5, 0.5), 0.3, 0.3, 30.0);
                ribbon_into(pb, &[(0.2, 0.6), (0.5, 0.5), (0.8, 0.6)], 0.15);
            },
            50,
            50
        ));
        // A ribbon is round at its ends.
        assert!(filled_at(
            |pb| ribbon_into(pb, &[(0.2, 0.5), (0.8, 0.5)], 0.2),
            15,
            50
        ));
    }

    #[test]
    fn a_pen_draws_where_the_canvas_transform_says() {
        // The same ellipse drawn through a pen (mapped, then filled as it is) and through
        // tiny-skia with the transform: the same pixels.
        let t = Transform::from_row(80.0, 10.0, -12.0, 70.0, 20.0, 15.0);
        let mut a = Pixmap::new(120, 120).expect("a pixmap");
        let mut b = Pixmap::new(120, 120).expect("a pixmap");
        {
            let mut c = Canvas {
                pixmap: &mut a,
                transform: t,
                px_per_unit: 75.0,
                hue: 0.0,
                shadow: true,
            };
            fixed(
                &mut c,
                &|s, _| ellipse_into(s, (0.5, 0.5), 0.3, 0.2, 20.0),
                [9, 9, 9, 255],
            );
        }
        let mut pb = PathBuilder::new();
        ellipse_into(&mut pb, (0.5, 0.5), 0.3, 0.2, 20.0);
        let path = pb.finish().expect("a path");
        let mut paint = Paint::default();
        paint.set_color_rgba8(9, 9, 9, 255);
        b.fill_path(&path, &paint, FillRule::Winding, t, None);
        let differ = a
            .pixels()
            .iter()
            .zip(b.pixels())
            .filter(|(x, y)| x.alpha().abs_diff(y.alpha()) > 2)
            .count();
        assert!(differ < 10, "{differ} pixels differ");
    }
}
