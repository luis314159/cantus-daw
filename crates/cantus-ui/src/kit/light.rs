//! The one lighting recipe (ADR 0024 §4, `fl-studio.md` §1.4, §12.4). Light comes from
//! above. Nothing else in the kit makes depth.
//!
//! - **Raised** (caps, keys, tiles, buttons): a 1-px outline at base −20 % L; a face
//!   running from +6 % L at the top to −6 % L at the bottom; a 1-px top edge at +15 % L
//!   inside; a 2-px drop below at base −25 % L, 60 % alpha, fading.
//! - **Sunken** (wells, fields, meter wells): filled darker toward the bottom (−4 % L),
//!   a 1-px border at −8 % L, and a 1-px inner top line at −12 % L — the shadow the lip
//!   casts.
//! - **Lit** parts are the same recipe with the role colour as the base.
//! - **Depth** (the theme's form, 0…1.5) scales every step of the recipe: at 0 a part is
//!   flat with a quiet outline, at 1 it is FL's relief.

use super::brush::{Brush, Fill};
use super::color::{shade, with_alpha};
use egui::{Color32, Pos2, Rect};

/// The colours of the raised recipe for one base.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Raised {
    pub outline: Color32,
    pub top: Color32,
    pub bottom: Color32,
    pub edge: Color32,
    pub drop: Color32,
    pub groove: Color32,
}

impl Raised {
    /// The recipe at full depth.
    pub fn of(base: Color32) -> Raised {
        Raised::at(base, 1.0)
    }

    /// The recipe at `depth` (0 flat … 1 FL's relief).
    pub fn at(base: Color32, depth: f32) -> Raised {
        let d = depth.max(0.0);
        Raised {
            outline: shade(base, -(0.08 + 0.12 * d)),
            top: shade(base, 0.06 * d),
            bottom: shade(base, -0.06 * d),
            edge: shade(base, 0.15 * d),
            drop: with_alpha(shade(base, -0.25), (0.6 * d).min(1.0)),
            groove: shade(base, -(0.20 + 0.15 * d)),
        }
    }
}

/// The colours of the sunken recipe for one base.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sunken {
    pub fill_top: Color32,
    pub fill_bottom: Color32,
    pub border: Color32,
    pub inner_top: Color32,
}

impl Sunken {
    /// The recipe at full depth.
    pub fn of(base: Color32) -> Sunken {
        Sunken::at(base, 1.0)
    }

    /// The recipe at `depth`.
    pub fn at(base: Color32, depth: f32) -> Sunken {
        let d = depth.max(0.0);
        Sunken {
            fill_top: base,
            fill_bottom: shade(base, -0.04 * d),
            border: shade(base, -(0.04 + 0.04 * d)),
            inner_top: shade(base, -(0.04 + 0.08 * d)),
        }
    }
}

/// Options on the raised recipe.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RaisedOpts {
    /// A 1-px groove this far down the face (0…1): step keys at 0.3, fader caps at 0.5.
    pub groove: Option<f32>,
    /// Paint the drop below.
    pub drop: bool,
    /// Scale of the drop and edges (the style's `k`).
    pub k: f32,
    /// Multiply everything by this alpha (disabled parts).
    pub alpha: f32,
    /// How much of the recipe (the theme's depth).
    pub depth: f32,
}

impl Default for RaisedOpts {
    fn default() -> Self {
        RaisedOpts {
            groove: None,
            drop: true,
            k: 1.0,
            alpha: 1.0,
            depth: 1.0,
        }
    }
}

fn fade(c: Color32, a: f32) -> Color32 {
    if a >= 1.0 {
        c
    } else {
        c.gamma_multiply(a)
    }
}

/// The raised recipe on `rect` (snapped to the pixel grid first) with corner `r`.
pub fn raised(b: &mut Brush, rect: Rect, r: f32, base: Color32, o: RaisedOpts) {
    raised_with(b, rect, r, &Raised::at(base, o.depth), o);
}

/// [`raised`] with precomputed colours — for a view painting many parts of one base.
pub fn raised_with(b: &mut Brush, rect: Rect, r: f32, c: &Raised, o: RaisedOpts) {
    let a = o.alpha;
    raised_face(b, rect, r, c, Fill::V(fade(c.top, a), fade(c.bottom, a)), o);
}

/// The raised recipe with its face filled by `face` instead of the plain ramp — the
/// fader cap's face, lit above its groove and hollow below it, is the same recipe with
/// more stops.
pub fn raised_face(b: &mut Brush, rect: Rect, r: f32, c: &Raised, face: Fill<'_>, o: RaisedOpts) {
    let rect = b.snap_rect(rect);
    let px = b.px();
    let a = o.alpha;
    if o.drop && o.depth > 0.0 {
        let d = (2.0 * o.k).max(px);
        b.rrect_soft(
            rect.translate(egui::vec2(0.0, d * 0.5)),
            r + d * 0.5,
            Fill::Solid(fade(c.drop, a)),
            d * 1.5,
        );
    }
    b.rrect(rect, r, Fill::Solid(fade(c.outline, a)));
    let inner = rect.shrink(px);
    b.rrect(inner, (r - px).max(0.0), face);
    // The top edge: a 1-px line of light just inside the outline, along the flat part.
    let inset = (r - px).max(0.0);
    b.hline(
        inner.left() + inset,
        inner.right() - inset,
        inner.top() + px * 0.5,
        px,
        fade(c.edge, a),
    );
    if let Some(t) = o.groove {
        let y = b.snap_mid(inner.top() + inner.height() * t);
        b.hline(
            inner.left() + px,
            inner.right() - px,
            y,
            px,
            fade(c.groove, a),
        );
    }
}

/// The sunken recipe on `rect` with corner `r`, at `depth`.
pub fn sunken(b: &mut Brush, rect: Rect, r: f32, base: Color32, alpha: f32, depth: f32) {
    sunken_with(b, rect, r, &Sunken::at(base, depth), alpha);
}

/// [`sunken`] with precomputed colours.
pub fn sunken_with(b: &mut Brush, rect: Rect, r: f32, c: &Sunken, alpha: f32) {
    let rect = b.snap_rect(rect);
    let px = b.px();
    b.rrect(rect, r, Fill::Solid(fade(c.border, alpha)));
    let inner = rect.shrink(px);
    b.rrect(
        inner,
        (r - px).max(0.0),
        Fill::V(fade(c.fill_top, alpha), fade(c.fill_bottom, alpha)),
    );
    let inset = (r - px).max(0.0);
    b.hline(
        inner.left() + inset * 0.5,
        inner.right() - inset * 0.5,
        inner.top() + px * 0.5,
        px,
        fade(c.inner_top, alpha),
    );
}

/// A lit lens: the soft glow a lit LED or lamp throws around itself, as a cached layer
/// (a radial falloff rasterised once per colour and size).
pub fn glow(b: &mut Brush, center: Pos2, radius: f32, color: Color32, strength: f32) {
    let ppp = b.ppp();
    let px = (radius * 2.0 * ppp).ceil().max(2.0) as usize;
    let key = super::cache::key(("glow", super::cache::ckey(color), px));
    let uv = b.layer(key, px, px, |pix| {
        let [r, g, bl, _] = color.to_array();
        let col = |a: f32| {
            tiny_skia::Color::from_rgba8(r, g, bl, (a * 255.0).round().clamp(0.0, 255.0) as u8)
        };
        let half = px as f32 * 0.5;
        let Some(shader) = tiny_skia::RadialGradient::new(
            tiny_skia::Point::from_xy(half, half),
            0.0,
            tiny_skia::Point::from_xy(half, half),
            half,
            vec![
                tiny_skia::GradientStop::new(0.0, col(0.55)),
                tiny_skia::GradientStop::new(0.45, col(0.22)),
                tiny_skia::GradientStop::new(1.0, col(0.0)),
            ],
            tiny_skia::SpreadMode::Pad,
            tiny_skia::Transform::identity(),
        ) else {
            return;
        };
        let paint = tiny_skia::Paint {
            shader,
            anti_alias: true,
            ..Default::default()
        };
        if let Some(rect) = tiny_skia::Rect::from_xywh(0.0, 0.0, px as f32, px as f32) {
            pix.fill_rect(rect, &paint, tiny_skia::Transform::identity(), None);
        }
    });
    if let Some(uv) = uv {
        let side = px as f32 / ppp;
        let rect = b.snap_rect(Rect::from_center_size(center, egui::Vec2::splat(side)));
        b.layer_quad(rect, uv, strength);
    }
}

/// The theme's sheen over a bar (ADR 0046 §8): `t.sheen` at the bar's top edge, fading to
/// clear two thirds of the way down — the website's lit buttons and panel heads. A flat
/// theme (clear sheen) paints nothing, not even a vertex.
pub fn sheen(b: &mut Brush, rect: Rect, t: &super::Tokens) {
    if t.sheen.a() == 0 || rect.height() <= 0.0 {
        return;
    }
    let stops = [
        (0.0, t.sheen),
        (0.3, t.sheen.gamma_multiply(0.45)),
        (0.7, Color32::TRANSPARENT),
        (1.0, Color32::TRANSPARENT),
    ];
    b.rect(b.snap_rect(rect), Fill::Stops(&stops));
}

/// The theme's glows in a large empty ground (ADR 0046 §8), as on the website's hero: the
/// warm one centred near the top right, the cool one at the bottom left, each an ellipse
/// of rings of geometry, clipped to `area`. Paint it after the ground's
/// fill and before what sits on it. A flat theme paints nothing.
pub fn ambient(b: &mut Brush, area: Rect, t: &super::Tokens) {
    if area.width() <= 0.0 || area.height() <= 0.0 {
        return;
    }
    let (w, h) = (area.width(), area.height());
    if t.glow.a() > 0 {
        let c = area.min + egui::vec2(0.82 * w, 0.08 * h);
        glow_field(b, area, c, egui::vec2(0.62 * w, 1.1 * h), t.glow);
    }
    if t.glow_cool.a() > 0 {
        let c = area.min + egui::vec2(0.06 * w, 1.0 * h);
        glow_field(b, area, c, egui::vec2(0.48 * w, 0.95 * h), t.glow_cool);
    }
}

/// One glow: an ellipse of radii `radii` round `center`, in `color` at its alpha, the
/// part inside `clip` only — rings of geometry whose colours follow a smooth bell (no
/// texture, so the GPU and the CPU renderer draw it alike, without a seam or a step).
fn glow_field(b: &mut Brush, clip: Rect, center: Pos2, radii: egui::Vec2, color: Color32) {
    const RINGS: usize = 12;
    let stops: [(f32, Color32); RINGS + 1] = std::array::from_fn(|k| {
        let t = k as f32 / RINGS as f32;
        let s = 1.0 - t;
        // Smoothstep: flat at the centre, no ring at the rim.
        (t, color.gamma_multiply(s * s * (3.0 - 2.0 * s)))
    });
    b.radial(clip, center, radii, &stops, 12);
}
