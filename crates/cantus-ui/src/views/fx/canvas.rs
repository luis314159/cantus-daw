//! What the effect editors' displays paint with (ADR 0024 §4, `plugin-instrument-uis.md`
//! §7.10): the colours of a display from the theme's tokens — its ground, grid, the
//! device's hue for the main curve — and words that are **measured before they are
//! painted**, in a role, on a plate when they sit over data, so a label never runs into
//! a curve or another label unseen.
//!
//! Text goes through the kit's cache (`kit::text::fit`): a label that did not change
//! costs a hash and a lookup, and numbers are formatted into one reused buffer.

use crate::kit::color::{shade, with_alpha};
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::{Brush, Fill, Style, Tok};
use egui::{Align2, Color32, Painter, Pos2, Rect, Stroke, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;

/// The colours of a display.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ink {
    /// The display's ground (`surface.ink`).
    pub ground: Color32,
    pub grid: Color32,
    pub grid_strong: Color32,
    /// The display's edge.
    pub edge: Color32,
    pub text: Color32,
    pub dim: Color32,
    /// The device's (or the card's) function hue: the main curve, handles, fills.
    pub hue: Color32,
    /// Gain reduction, clipping, a warning.
    pub danger: Color32,
    /// Something open or passing (a gate's open phase, a verdict that passes).
    pub ok: Color32,
    /// Behind a label that sits over data.
    pub plate: Color32,
    /// A secondary line (the input under the output).
    pub faint: Color32,
    pub dark: bool,
}

impl Ink {
    pub fn of(s: &Style) -> Ink {
        let t = &s.t;
        Ink {
            ground: t.ink,
            grid: t.grid,
            grid_strong: if t.dark {
                shade(t.grid, 0.07)
            } else {
                t.grid_strong
            },
            edge: with_alpha(t.edge, 0.55),
            text: t.text,
            dim: t.text_dim,
            hue: s.hue,
            danger: t.danger,
            ok: t.meter_low,
            plate: with_alpha(t.ink, 0.86),
            faint: with_alpha(t.text_dim, if t.dark { 0.30 } else { 0.40 }),
            dark: t.dark,
        }
    }

    /// A fill under a curve in the hue: ≤ 35 % in a dark theme, ≤ 20 % in a light one.
    pub fn fill(&self, share: f32) -> Color32 {
        with_alpha(self.hue, share * if self.dark { 0.35 } else { 0.22 })
    }

    /// A colour at `alpha`.
    pub fn a(c: Color32, alpha: f32) -> Color32 {
        with_alpha(c, alpha)
    }
}

/// The ground of a display: `ink`, a hair lighter at the top, a 1-px edge.
pub fn ground(p: &Painter, rect: Rect, ink: &Ink) {
    let top = if ink.dark {
        shade(ink.ground, 0.025)
    } else {
        ink.ground
    };
    let mut mesh = egui::Mesh::default();
    let i = 0u32;
    mesh.colored_vertex(rect.left_top(), top);
    mesh.colored_vertex(rect.right_top(), top);
    mesh.colored_vertex(rect.right_bottom(), ink.ground);
    mesh.colored_vertex(rect.left_bottom(), ink.ground);
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
    p.add(mesh);
}

/// The display's edge, painted last so the data never covers it.
pub fn edge(p: &Painter, rect: Rect, ink: &Ink) {
    p.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0 / p.ctx().pixels_per_point(), ink.edge),
        egui::StrokeKind::Inside,
    );
}

thread_local! {
    static FMT: RefCell<String> = RefCell::new(String::with_capacity(64));
}

fn snap(p: &Painter, v: f32) -> f32 {
    let ppp = p.ctx().pixels_per_point();
    (v * ppp).round() / ppp
}

/// The galley of `text` fitted to `width`.
fn galley(
    p: &Painter,
    text: &Text<'_>,
    k: f32,
    width: f32,
) -> Option<std::sync::Arc<egui::Galley>> {
    let ctx = p.ctx();
    crate::kit::cache::with(ctx, |c| ktext::fit(ctx, c, text, k, width)).map(|(g, _)| g)
}

/// A word on a display, in `role`, anchored at `at` by `align`, no wider than `max_w`
/// (then elided). Returns where it landed.
#[allow(clippy::too_many_arguments)]
pub fn label_in(
    p: &Painter,
    at: Pos2,
    align: Align2,
    src: Src<'_>,
    role: Role,
    color: Color32,
    k: f32,
    max_w: f32,
    plate: Option<Color32>,
) -> Rect {
    let text = Text {
        src,
        role,
        caps: role == Role::Micro || role == Role::Section,
        align,
        policy: Policy::Elide(Elide::End),
        color,
    };
    let Some(g) = galley(p, &text, k, max_w) else {
        return Rect::NOTHING;
    };
    let size = g.size();
    let r = align.anchor_size(at, size);
    let pos = Pos2::new(snap(p, r.min.x), snap(p, r.min.y));
    let r = Rect::from_min_size(pos, size);
    if let Some(c) = plate {
        p.rect_filled(r.expand2(Vec2::new(3.0 * k, 1.0 * k)), 2.0 * k, c);
    }
    p.galley(pos, g, color);
    r
}

/// A word on a display (no plate), up to 400 pt wide.
pub fn label(
    p: &Painter,
    at: Pos2,
    align: Align2,
    text: &str,
    role: Role,
    color: Color32,
    k: f32,
) -> Rect {
    label_in(
        p,
        at,
        align,
        Src::Str(text),
        role,
        color,
        k,
        400.0 * k,
        None,
    )
}

/// A translated word on a display.
pub fn label_key(
    p: &Painter,
    at: Pos2,
    align: Align2,
    key: &str,
    role: Role,
    color: Color32,
    k: f32,
) -> Rect {
    label_in(p, at, align, Src::Key(key), role, color, k, 400.0 * k, None)
}

/// A word on a plate: for labels that sit over curves and fills.
pub fn plate(
    p: &Painter,
    at: Pos2,
    align: Align2,
    text: &str,
    color: Color32,
    ink: &Ink,
    k: f32,
) -> Rect {
    label_in(
        p,
        at,
        align,
        Src::Str(text),
        Role::Value,
        color,
        k,
        400.0 * k,
        Some(ink.plate),
    )
}

/// A formatted word on a display, written into a reused buffer: `labelf(p, at, align,
/// Role::Micro, colour, k, format_args!("{db:.0}"))`.
#[allow(clippy::too_many_arguments)]
pub fn labelf(
    p: &Painter,
    at: Pos2,
    align: Align2,
    role: Role,
    color: Color32,
    k: f32,
    plate: Option<Color32>,
    args: std::fmt::Arguments<'_>,
) -> Rect {
    FMT.with(|b| match b.try_borrow_mut() {
        Ok(mut buf) => {
            buf.clear();
            let _ = buf.write_fmt(args);
            label_in(
                p,
                at,
                align,
                Src::Str(&buf),
                role,
                color,
                k,
                400.0 * k,
                plate,
            )
        }
        Err(_) => {
            let s = std::fmt::format(args);
            label_in(p, at, align, Src::Str(&s), role, color, k, 400.0 * k, plate)
        }
    })
}

/// Labels placed left to right that must not touch: each is skipped when it would land
/// on the one before it.
#[derive(Default)]
pub struct Ruler {
    last: f32,
}

impl Ruler {
    pub fn new() -> Ruler {
        Ruler {
            last: f32::NEG_INFINITY,
        }
    }

    /// Whether a label spanning `x0..x1` fits after the last one placed, with a gap.
    pub fn fits(&self, x0: f32, gap: f32) -> bool {
        x0 >= self.last + gap
    }

    pub fn placed(&mut self, r: Rect) {
        if r.is_positive() {
            self.last = r.right();
        }
    }
}

// ---- through the kit's brush ----------------------------------------------------------
//
// A display painted through one `Brush` is one mesh (pooled: no allocation once warm)
// plus its words, which the brush paints after the geometry — so a label painted last
// is never under a curve.

/// The tokens a display paints with, beside its hue (for inspect mode).
pub const DISPLAY_TOKENS: &[Tok] = &[
    Tok::Ink,
    Tok::Grid,
    Tok::GridStrong,
    Tok::Edge,
    Tok::Text,
    Tok::TextDim,
    Tok::Danger,
    Tok::MeterLow,
];

/// The display's ground and its name for inspect mode.
pub fn ground_b(b: &mut Brush, rect: Rect, ink: &Ink, hue: Option<Tok>) {
    b.register(rect, "display", DISPLAY_TOKENS, &[hue]);
    let top = if ink.dark {
        shade(ink.ground, 0.025)
    } else {
        ink.ground
    };
    b.rect(b.snap_rect(rect), Fill::V(top, ink.ground));
}

/// A 1-px edge around a display.
pub fn edge_b(b: &mut Brush, rect: Rect, ink: &Ink) {
    let r = b.snap_rect(rect);
    let px = b.px();
    b.hline(r.left(), r.right(), r.top() + px * 0.5, px, ink.edge);
    b.hline(r.left(), r.right(), r.bottom() - px * 0.5, px, ink.edge);
    b.vline(r.left() + px * 0.5, r.top(), r.bottom(), px, ink.edge);
    b.vline(r.right() - px * 0.5, r.top(), r.bottom(), px, ink.edge);
}

/// A dashed line from `a` to `c`.
pub fn dashed(b: &mut Brush, a: Pos2, c: Pos2, w: f32, color: Color32, dash: f32, gap: f32) {
    let d = c - a;
    let len = d.length();
    if len <= 0.0 {
        return;
    }
    let dir = d / len;
    let mut t = 0.0;
    while t < len {
        let e = (t + dash).min(len);
        b.segment(a + dir * t, a + dir * e, w, color, false);
        t = e + gap;
    }
}

/// Diagonal hatching inside `r` (up to the right), `step` apart.
pub fn hatch(b: &mut Brush, r: Rect, step: f32, w: f32, color: Color32) {
    let h = r.height();
    let mut x = r.left() - h;
    while x < r.right() {
        // The line from (x, bottom) to (x + h, top), clipped to the rect's sides.
        let (mut x0, mut y0) = (x, r.bottom());
        let (mut x1, mut y1) = (x + h, r.top());
        if x0 < r.left() {
            y0 -= r.left() - x0;
            x0 = r.left();
        }
        if x1 > r.right() {
            y1 += x1 - r.right();
            x1 = r.right();
        }
        if x1 > x0 {
            b.segment(Pos2::new(x0, y0), Pos2::new(x1, y1), w, color, false);
        }
        x += step;
    }
}

/// A word anchored at `at` by `align`, on a plate when given one. Returns where it went.
pub fn put(
    b: &mut Brush,
    at: Pos2,
    align: Align2,
    text: &Text<'_>,
    k: f32,
    plate: Option<Color32>,
) -> Rect {
    let size = ktext::measure(b.ctx(), text, k);
    let r = align.anchor_size(at, size);
    if let Some(c) = plate {
        b.rrect(
            b.snap_rect(r.expand2(Vec2::new(3.0 * k, 1.0 * k))),
            2.0 * k,
            Fill::Solid(c),
        );
    }
    let t = Text {
        policy: Policy::Elide(Elide::End),
        align: Align2::CENTER_CENTER,
        ..*text
    };
    match ktext::paint(b, r.expand(1.0), &t, k) {
        Some(p) => p.rect,
        None => Rect::NOTHING,
    }
}

/// A formatted word, written into a reused buffer: `putf(b, at, align, Role::Micro,
/// colour, k, None, format_args!("{db:.0}"))`.
#[allow(clippy::too_many_arguments)]
pub fn putf(
    b: &mut Brush,
    at: Pos2,
    align: Align2,
    role: Role,
    color: Color32,
    k: f32,
    plate: Option<Color32>,
    args: std::fmt::Arguments<'_>,
) -> Rect {
    FMT.with(|cell| match cell.try_borrow_mut() {
        Ok(mut buf) => {
            buf.clear();
            let _ = buf.write_fmt(args);
            let t = Text::new(&buf, role, color).caps(false);
            put(b, at, align, &t, k, plate)
        }
        Err(_) => {
            let s = std::fmt::format(args);
            let t = Text::new(&s, role, color).caps(false);
            put(b, at, align, &t, k, plate)
        }
    })
}

/// A translated word at `at`.
pub fn put_key(
    b: &mut Brush,
    at: Pos2,
    align: Align2,
    key: &str,
    role: Role,
    color: Color32,
    k: f32,
) -> Rect {
    let t = Text::key(key, role, color);
    put(b, at, align, &t, k, None)
}

/// A formatted line in a box, left-aligned and elided at the end when it does not fit.
pub fn line_in(
    b: &mut Brush,
    rect: Rect,
    role: Role,
    color: Color32,
    k: f32,
    args: std::fmt::Arguments<'_>,
) {
    FMT.with(|cell| match cell.try_borrow_mut() {
        Ok(mut buf) => {
            buf.clear();
            let _ = buf.write_fmt(args);
            let t = Text::new(&buf, role, color)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = ktext::paint(b, rect, &t, k);
        }
        Err(_) => {
            let s = std::fmt::format(args);
            let t = Text::new(&s, role, color)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = ktext::paint(b, rect, &t, k);
        }
    })
}

/// A value with its name, on one plate: the name small in capitals (`OPEN`), the value in
/// the value face (`-30.0 dB`) — FL's and Pro-Q's way with a readout over a display. (A
/// value text puts its unit a hair apart instead of after a space, so a word and a number
/// are two texts, never one.)
#[allow(clippy::too_many_arguments)]
pub fn tag(
    b: &mut Brush,
    at: Pos2,
    align: Align2,
    word: &str,
    color: Color32,
    k: f32,
    plate: Option<Color32>,
    value: std::fmt::Arguments<'_>,
) -> Rect {
    FMT.with(|cell| {
        let mut owned;
        let mut guard = cell.try_borrow_mut();
        let buf: &mut String = match guard.as_mut() {
            Ok(b) => b,
            Err(_) => {
                owned = String::new();
                &mut owned
            }
        };
        buf.clear();
        let _ = buf.write_fmt(value);
        let w = Text::new(word, Role::Micro, color).caps(true);
        let v = Text::new(buf, Role::Value, color);
        let ctx = b.ctx().clone();
        let ws = ktext::measure(&ctx, &w, k);
        let vs = ktext::measure(&ctx, &v, k);
        let gap = if word.is_empty() { 0.0 } else { 4.0 * k };
        let size = Vec2::new(ws.x + gap + vs.x, ws.y.max(vs.y));
        let r = align.anchor_size(at, size);
        if let Some(c) = plate {
            b.rrect(
                b.snap_rect(r.expand2(Vec2::new(3.0 * k, 1.0 * k))),
                2.0 * k,
                Fill::Solid(c),
            );
        }
        let wr = Rect::from_min_size(Pos2::new(r.left(), r.top()), Vec2::new(ws.x + 1.0, size.y));
        let vr = Rect::from_min_max(
            Pos2::new(wr.right() - 1.0 + gap, r.top()),
            r.max + Vec2::new(1.0, 0.0),
        );
        if !word.is_empty() {
            let _ = ktext::paint(
                b,
                wr,
                &w.align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                k,
            );
        }
        let _ = ktext::paint(
            b,
            vr,
            &v.align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            k,
        );
        r
    })
}
