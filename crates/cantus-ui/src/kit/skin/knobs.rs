//! A unit's knobs (round 32, ADR 0074): every [`KnobShape`] the kit draws — a value ring
//! on a flat disc (FabFilter's), a fluted skirt under a cap, a chicken head over a printed
//! scale, soft rubber with a dot, a machined cap with knurling, a numbered dial.
//!
//! The kit's own parts stay on every shape: the modulation rings outside, the value arc
//! drawn from the default in the unit's hue, the default's notch, the live dot of
//! modulation and the focus ring — a shape changes how a knob looks, never what it says.
//! Static bodies are rasterised once per size, colours and theme into the layer atlas
//! (keyed by all three, so one unit's colours never show in another's window); what moves
//! (the pointer, the arc) is drawn each frame.

use super::super::brush::{Brush, Fill};
use super::super::cache;
use super::super::color::{mix, shade, with_alpha};
use super::super::knob::{angle, paint_live, paint_track, value_hue, Geometry, KnobLook, KnobSize};

use super::super::{State, Style, Tok};
use super::KnobShape;
use egui::{Color32, Pos2, Rect, Vec2};
use std::f32::consts::{PI, TAU};

impl KnobShape {
    /// Every shape the kit draws (a unit's own painter aside), the kit's first.
    pub const ALL: [KnobShape; 7] = [
        KnobShape::Plain,
        KnobShape::Arc,
        KnobShape::Skirted,
        KnobShape::ChickenHead,
        KnobShape::Soft,
        KnobShape::Cap,
        KnobShape::Dial,
    ];

    /// The name the gallery and the shots give it.
    pub fn name(self) -> &'static str {
        match self {
            KnobShape::Plain => "plain",
            KnobShape::Arc => "arc",
            KnobShape::Skirted => "skirted",
            KnobShape::ChickenHead => "chicken_head",
            KnobShape::Soft => "soft",
            KnobShape::Cap => "cap",
            KnobShape::Dial => "dial",
            KnobShape::Custom(_) => "custom",
        }
    }
}

/// The tokens a shape paints with (for inspect mode), beside the hue.
pub fn tokens(shape: KnobShape, dark: bool) -> &'static [Tok] {
    match (shape, dark) {
        (KnobShape::Arc, _) => &[
            Tok::KnobTrack,
            Tok::KnobBody,
            Tok::KnobPointer,
            Tok::TextDim,
            Tok::Light,
            Tok::Focus,
        ],
        (KnobShape::ChickenHead, _) => &[
            Tok::KnobTrack,
            Tok::KnobBody,
            Tok::KnobPointer,
            Tok::TextDim,
            Tok::Light,
            Tok::Dark,
            Tok::Focus,
        ],
        // The dial's body is the kit's, with its rim on a dark theme.
        (KnobShape::Dial, true) => &[
            Tok::KnobTrack,
            Tok::KnobBody,
            Tok::KnobRim,
            Tok::KnobPointer,
            Tok::Text,
            Tok::TextDim,
            Tok::Light,
            Tok::Dark,
            Tok::Focus,
        ],
        (KnobShape::Dial, false) => &[
            Tok::KnobTrack,
            Tok::KnobBody,
            Tok::KnobPointer,
            Tok::Text,
            Tok::TextDim,
            Tok::Light,
            Tok::Dark,
            Tok::Focus,
        ],
        // The soft knob's dot is the hue; the cap's line is cut into its metal.
        (KnobShape::Soft | KnobShape::Cap, _) => &[
            Tok::KnobTrack,
            Tok::KnobBody,
            Tok::TextDim,
            Tok::Light,
            Tok::Dark,
            Tok::Focus,
        ],
        _ => &[
            Tok::KnobTrack,
            Tok::KnobBody,
            Tok::KnobPointer,
            Tok::TextDim,
            Tok::Light,
            Tok::Dark,
            Tok::Focus,
        ],
    }
}

/// Paint a knob of `shape` centred on `c` (the rings and the focus are the caller's).
pub fn paint(
    b: &mut Brush,
    c: Pos2,
    s: &Style,
    st: State,
    look: &KnobLook<'_>,
    shape: KnobShape,
    g: &Geometry,
) {
    let a = st.alpha();
    let hue = value_hue(s, st);
    let v = look.value.clamp(0.0, 1.0);
    let dir = Vec2::angled(angle(v));
    let f = |col: Color32| col.gamma_multiply(a);
    let t = &s.t;
    let hover = |b: &mut Brush, radius: f32| {
        if st.hovered && !st.disabled {
            let tint = with_alpha(t.light, if t.dark { 0.07 } else { 0.10 });
            b.circle(c, radius, Fill::Solid(tint));
        }
    };
    match shape {
        KnobShape::Plain | KnobShape::Custom(_) => {}
        KnobShape::Arc => {
            // A thin track, the value arc full width over a soft halo; a flat disc.
            let (tr, tw) = (g.track_r, g.track_w);
            b.arc(
                c,
                tr,
                angle(0.0),
                angle(1.0),
                tw * 0.55,
                f(t.knob_track),
                true,
            );
            let d = look.default.clamp(0.0, 1.0);
            if !st.disabled && (v - d).abs() > 0.002 {
                let (a0, a1) = (angle(d.min(v)), angle(d.max(v)));
                b.arc(c, tr, a0, a1, tw * 2.2, with_alpha(hue, 0.16 * a), false);
                b.arc(c, tr, a0, a1, tw, hue, true);
            }
            if d > 0.01 && d < 0.99 {
                let nd = Vec2::angled(angle(d));
                b.segment(
                    c + nd * (g.r + s.s(1.0)),
                    c + nd * (g.r + s.s(3.0)),
                    s.s(1.5),
                    f(t.text_dim),
                    false,
                );
            }
            let disc = g.body_r + s.s(1.0);
            b.circle(c, disc, Fill::Solid(f(shade(t.knob_body, -0.05))));
            b.arc(
                c,
                disc - b.px() * 0.5,
                PI * 1.1,
                PI * 1.9,
                b.px(),
                f(with_alpha(t.light, 0.16 * t.depth.min(1.0))),
                false,
            );
            hover(b, disc);
            b.segment(
                c + dir * (disc * 0.5),
                c + dir * (disc * 0.9),
                g.pointer_w,
                f(t.knob_pointer),
                true,
            );
            paint_live(b, c, s, st, look, tr, tw);
        }
        KnobShape::Skirted => {
            paint_track(b, c, s, st, look, (g.track_r, g.track_w), g.r, hue);
            body(b, c, g.body_r, s, a, shape);
            hover(b, g.body_r);
            // The line on the skirt, outside the cap.
            b.segment(
                c + dir * (g.body_r * 0.64),
                c + dir * (g.body_r * 0.97),
                g.pointer_w,
                f(t.knob_pointer),
                true,
            );
            paint_live(b, c, s, st, look, g.track_r, g.track_w);
        }
        KnobShape::ChickenHead => chicken_head(b, c, s, st, look, g, hue),
        KnobShape::Soft => {
            paint_track(b, c, s, st, look, (g.track_r, g.track_w), g.r, hue);
            body(b, c, g.body_r, s, a, shape);
            hover(b, g.body_r);
            // The dot, in the unit's hue.
            let at = c + dir * (g.body_r * 0.66);
            b.circle(
                at,
                g.pointer_w * 1.05 + b.px(),
                Fill::Solid(f(shade(t.knob_body, -0.18))),
            );
            b.circle(at, g.pointer_w * 1.05, Fill::Solid(f(hue)));
            paint_live(b, c, s, st, look, g.track_r, g.track_w);
        }
        KnobShape::Cap => {
            paint_track(b, c, s, st, look, (g.track_r, g.track_w), g.r, hue);
            body(b, c, g.body_r, s, a, shape);
            hover(b, g.body_r);
            // An engraved line: its shade, and the light of its lower lip.
            let (p0, p1) = (c + dir * (g.body_r * 0.18), c + dir * (g.body_r * 0.76));
            let lip = Vec2::new(0.0, b.px());
            b.segment(
                p0 + lip,
                p1 + lip,
                g.pointer_w,
                f(with_alpha(t.light, 0.35)),
                true,
            );
            b.segment(p0, p1, g.pointer_w, f(shade(t.knob_body, -0.32)), true);
            paint_live(b, c, s, st, look, g.track_r, g.track_w);
        }
        KnobShape::Dial => dial(b, c, s, st, look, g, hue),
    }
}

/// A chicken-head pointer knob over a printed scale: ticks at the travel's tenths, the
/// value arc thin inside them, the head pointing at the value with a white line inlaid.
fn chicken_head(
    b: &mut Brush,
    c: Pos2,
    s: &Style,
    st: State,
    look: &KnobLook<'_>,
    g: &Geometry,
    hue: Color32,
) {
    let t = &s.t;
    let a = st.alpha();
    let f = |col: Color32| col.gamma_multiply(a);
    let v = look.value.clamp(0.0, 1.0);
    for i in 0..=10 {
        let d = Vec2::angled(angle(i as f32 / 10.0));
        let len = if i % 5 == 0 { 0.62 } else { 0.4 } * g.track_w * 1.6;
        b.segment(
            c + d * (g.r - len),
            c + d * g.r,
            s.s(1.0),
            f(t.text_dim),
            false,
        );
    }
    let inner = g.r - g.track_w * 1.2;
    paint_track(b, c, s, st, look, (inner, g.track_w * 0.5), g.r, hue);
    let r = g.body_r * 0.98;
    let dir = Vec2::angled(angle(v));
    let side = dir.rot90();
    // The head: a long taper to the value, a short one behind; then the hub.
    let head = |w: f32, lift: Vec2| -> [Pos2; 8] {
        [
            c + lift + dir * (r * 0.98) + side * (r * 0.09 * w),
            c + lift + dir * (r * 0.30) + side * (r * 0.30 * w),
            c + lift - dir * (r * 0.40) + side * (r * 0.30 * w),
            c + lift - dir * (r * 0.62) + side * (r * 0.14 * w),
            c + lift - dir * (r * 0.62) - side * (r * 0.14 * w),
            c + lift - dir * (r * 0.40) - side * (r * 0.30 * w),
            c + lift + dir * (r * 0.30) - side * (r * 0.30 * w),
            c + lift + dir * (r * 0.98) - side * (r * 0.09 * w),
        ]
    };
    let drop = Vec2::new(0.0, s.s(1.2));
    let shadow = head(1.0, drop);
    b.polygon(
        &shadow,
        f(with_alpha(t.dark_shade, 0.35 * t.depth.min(1.0))),
    );
    let outline = head(1.0, Vec2::ZERO);
    b.polygon(&outline, f(shade(t.knob_body, -0.10)));
    let face = head(0.82, Vec2::new(0.0, -b.px()));
    b.polygon(&face, f(shade(t.knob_body, 0.05 * t.depth)));
    b.circle(
        c,
        r * 0.36,
        Fill::V(
            f(shade(t.knob_body, 0.10 * t.depth)),
            f(shade(t.knob_body, -0.04)),
        ),
    );
    if st.hovered && !st.disabled {
        b.polygon(
            &outline,
            with_alpha(t.light, if t.dark { 0.07 } else { 0.10 }),
        );
    }
    b.segment(
        c + dir * (r * 0.30),
        c + dir * (r * 0.88),
        g.pointer_w,
        f(t.knob_pointer),
        true,
    );
    paint_live(b, c, s, st, look, inner, g.track_w);
}

/// A dial: the travel's scale printed round a smaller knob — the numbers 0…10 on the
/// large sizes, ticks on the small — and the value arc close round the body.
fn dial(
    b: &mut Brush,
    c: Pos2,
    s: &Style,
    st: State,
    look: &KnobLook<'_>,
    g: &Geometry,
    hue: Color32,
) {
    let t = &s.t;
    let a = st.alpha();
    let f = |col: Color32| col.gamma_multiply(a);
    let numbers = matches!(look.size, KnobSize::L | KnobSize::XL);
    let ring = if numbers {
        g.r - s.s(5.5)
    } else {
        g.r - s.s(2.0)
    };
    // A graduated scale, eleven marks, the even ones long on a large dial. No numbers: a
    // dial does not know its parameter's range, and "0–10" round a 0–200 % width read as
    // a second, wrong value (the review of delivery 2) — the value pill says the value.
    for i in 0..=10 {
        let d = Vec2::angled(angle(i as f32 / 10.0));
        let len = if numbers && i % 2 == 0 {
            s.s(5.0)
        } else if numbers || i % 5 != 0 {
            s.s(2.0)
        } else {
            s.s(3.0)
        };
        let col = if numbers && i % 2 == 0 {
            t.text
        } else {
            t.text_dim
        };
        b.segment(
            c + d * (ring - len * 0.5),
            c + d * (ring + len * 0.5),
            s.s(1.0),
            f(col),
            false,
        );
    }
    let inner = ring - if numbers { s.s(6.5) } else { s.s(3.5) };
    let tw = g.track_w * 0.7;
    paint_track(b, c, s, st, look, (inner, tw), inner + tw, hue);
    let br = inner - tw * 0.5 - s.s(1.5);
    super::super::knob::body(b, c, br, s, a);
    if st.hovered && !st.disabled {
        let tint = with_alpha(t.light, if t.dark { 0.07 } else { 0.10 });
        b.circle(c, br, Fill::Solid(tint));
    }
    let dir = Vec2::angled(angle(look.value.clamp(0.0, 1.0)));
    b.segment(
        c + dir * (br * 0.3),
        c + dir * (br * 0.9),
        g.pointer_w,
        f(t.knob_pointer),
        true,
    );
    paint_live(b, c, s, st, look, inner, tw);
}

// ---- bodies, rasterised once -------------------------------------------------------------

/// The colours a body is rasterised with.
#[derive(Copy, Clone)]
struct Colours {
    body: Color32,
    light: Color32,
    shade: Color32,
    depth: f32,
}

fn ts(c: Color32, a: f32) -> tiny_skia::Color {
    let [r, g, b, _] = super::super::color::opaque(c).to_array();
    tiny_skia::Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Paint the body of `shape`, radius `br`, at `c` from the layer cache (as geometry when
/// the atlas has no room).
fn body(b: &mut Brush, c: Pos2, br: f32, s: &Style, alpha: f32, shape: KnobShape) {
    let c = Pos2::new(b.snap(c.x), b.snap(c.y));
    let ppp = b.ppp();
    let d = br * 2.0 * ppp;
    let margin = (d * 0.18).ceil().max(2.0);
    let side = (d + 2.0 * margin).ceil() as usize;
    let side = side + side % 2;
    let t = &s.t;
    let col = Colours {
        body: t.knob_body,
        light: t.light,
        shade: t.dark_shade,
        depth: t.depth,
    };
    let key = cache::key((
        "knob-shape",
        shape.name(),
        side,
        (br * ppp).to_bits(),
        cache::ckey(col.body),
        cache::ckey(col.light),
        cache::ckey(col.shade),
        t.dark,
        t.depth.to_bits(),
    ));
    let r_px = br * ppp;
    let uv = b.layer(key, side, side, |pix| raster(pix, r_px, shape, &col));
    match uv {
        Some(uv) => {
            let half = side as f32 / ppp * 0.5;
            let rect = Rect::from_min_max(c - Vec2::splat(half), c + Vec2::splat(half));
            b.layer_quad(rect, uv, alpha);
        }
        None => {
            b.circle(
                c,
                br,
                Fill::V(
                    shade(col.body, 0.08).gamma_multiply(alpha),
                    shade(col.body, -0.08).gamma_multiply(alpha),
                ),
            );
        }
    }
}

/// Rasterise a body of radius `r` pixels, centred in `pix`.
fn raster(pix: &mut tiny_skia::Pixmap, r: f32, shape: KnobShape, col: &Colours) {
    use tiny_skia::{
        FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Point, RadialGradient,
        SpreadMode, Stroke, Transform,
    };
    let w = pix.width() as f32;
    let c = w * 0.5;
    let id = Transform::identity();
    let d = col.depth.clamp(0.0, 1.5);
    let circle = |rad: f32| PathBuilder::from_circle(c, c, rad.max(0.5));
    let fill_shader =
        |pix: &mut tiny_skia::Pixmap, path: &tiny_skia::Path, shader: tiny_skia::Shader| {
            let p = Paint {
                shader,
                anti_alias: true,
                ..Default::default()
            };
            pix.fill_path(path, &p, FillRule::Winding, id, None);
        };
    let lin = |a: Color32, b: Color32, rad: f32| {
        LinearGradient::new(
            Point::from_xy(c - rad * 0.5, c - rad * 0.85),
            Point::from_xy(c + rad * 0.5, c + rad * 0.85),
            vec![
                GradientStop::new(0.0, ts(a, 1.0)),
                GradientStop::new(1.0, ts(b, 1.0)),
            ],
            SpreadMode::Pad,
            id,
        )
    };
    let stroke = |pix: &mut tiny_skia::Pixmap,
                  path: &tiny_skia::Path,
                  color: tiny_skia::Color,
                  width: f32| {
        let mut p = Paint::default();
        p.set_color(color);
        p.anti_alias = true;
        let st = Stroke {
            width,
            ..Default::default()
        };
        pix.stroke_path(path, &p, &st, id, None);
    };
    // The shadow under every body.
    if let (Some(sh), Some(path)) = (
        RadialGradient::new(
            Point::from_xy(c, c + r * 0.12),
            0.0,
            Point::from_xy(c, c + r * 0.12),
            r * 1.2,
            vec![
                GradientStop::new(0.0, ts(col.shade, 0.45 * d)),
                GradientStop::new(0.8, ts(col.shade, 0.30 * d)),
                GradientStop::new(1.0, ts(col.shade, 0.0)),
            ],
            SpreadMode::Pad,
            id,
        ),
        circle(r * 1.2),
    ) {
        fill_shader(pix, &path, sh);
    }
    let body = col.body;
    match shape {
        KnobShape::Skirted => {
            // The skirt: fluted, its flutes catching the light on their upper sides.
            let n = 28;
            let mut pb = PathBuilder::new();
            for i in 0..=n * 4 {
                let th = TAU * i as f32 / (n * 4) as f32;
                let flute = 1.0 - 0.035 * (0.5 + 0.5 * (n as f32 * th).cos());
                let (x, y) = (c + r * flute * th.cos(), c + r * flute * th.sin());
                if i == 0 {
                    pb.move_to(x, y);
                } else {
                    pb.line_to(x, y);
                }
            }
            pb.close();
            if let (Some(path), Some(sh)) = (
                pb.finish(),
                lin(shade(body, 0.06 * d), shade(body, -0.16 * d), r),
            ) {
                fill_shader(pix, &path, sh);
                stroke(pix, &path, ts(shade(body, -0.22), 1.0), 1.0);
            }
            for i in 0..n {
                let th = TAU * (i as f32 + 0.5) / n as f32;
                let up = (-th.sin()).max(0.0);
                let mut pb = PathBuilder::new();
                pb.move_to(c + r * 0.66 * th.cos(), c + r * 0.66 * th.sin());
                pb.line_to(c + r * 0.95 * th.cos(), c + r * 0.95 * th.sin());
                if let Some(path) = pb.finish() {
                    let a = 0.10 + 0.25 * up;
                    stroke(pix, &path, ts(col.light, a * d), 1.0);
                }
            }
            // The cap: a dome, its shadow on the skirt, a lit rim.
            if let Some(path) = circle(r * 0.62) {
                if let Some(sh) = RadialGradient::new(
                    Point::from_xy(c, c + r * 0.05),
                    0.0,
                    Point::from_xy(c, c + r * 0.05),
                    r * 0.70,
                    vec![
                        GradientStop::new(0.85, ts(col.shade, 0.35 * d)),
                        GradientStop::new(1.0, ts(col.shade, 0.0)),
                    ],
                    SpreadMode::Pad,
                    id,
                ) {
                    if let Some(p) = circle(r * 0.70) {
                        fill_shader(pix, &p, sh);
                    }
                }
                if let Some(sh) = RadialGradient::new(
                    Point::from_xy(c - r * 0.18, c - r * 0.22),
                    0.0,
                    Point::from_xy(c, c),
                    r * 0.62,
                    vec![
                        GradientStop::new(0.0, ts(shade(body, 0.16 * d), 1.0)),
                        GradientStop::new(1.0, ts(shade(body, -0.06 * d), 1.0)),
                    ],
                    SpreadMode::Pad,
                    id,
                ) {
                    fill_shader(pix, &path, sh);
                }
                stroke(pix, &path, ts(shade(body, -0.20), 1.0), 1.0);
            }
        }
        KnobShape::Soft => {
            // Soft-touch rubber: no bevel, a broad soft light, a darker rim.
            if let Some(path) = circle(r) {
                if let Some(sh) = RadialGradient::new(
                    Point::from_xy(c - r * 0.2, c - r * 0.3),
                    0.0,
                    Point::from_xy(c, c),
                    r,
                    vec![
                        GradientStop::new(0.0, ts(shade(body, 0.07 * d), 1.0)),
                        GradientStop::new(0.85, ts(shade(body, -0.03 * d), 1.0)),
                        GradientStop::new(1.0, ts(shade(body, -0.12 * d), 1.0)),
                    ],
                    SpreadMode::Pad,
                    id,
                ) {
                    fill_shader(pix, &path, sh);
                }
            }
        }
        KnobShape::Cap => {
            // Machined metal: a knurled rim, a face turned in fine rings, two bright
            // wedges where the light catches the turning.
            let metal = mix(body, col.light, 0.18);
            if let (Some(path), Some(sh)) = (
                circle(r),
                lin(shade(metal, 0.10 * d), shade(metal, -0.16 * d), r),
            ) {
                fill_shader(pix, &path, sh);
            }
            let teeth = ((TAU * r) / 2.6).round().max(24.0) as usize;
            for i in 0..teeth {
                let th = TAU * i as f32 / teeth as f32;
                let mut pb = PathBuilder::new();
                pb.move_to(c + r * 0.86 * th.cos(), c + r * 0.86 * th.sin());
                pb.line_to(c + (r - 0.5) * th.cos(), c + (r - 0.5) * th.sin());
                if let Some(path) = pb.finish() {
                    let lit = if i % 2 == 0 {
                        ts(col.light, 0.22 * d)
                    } else {
                        ts(col.shade, 0.30 * d)
                    };
                    stroke(pix, &path, lit, 0.9);
                }
            }
            let face = r * 0.84;
            if let (Some(path), Some(sh)) = (
                circle(face),
                lin(shade(metal, 0.04 * d), shade(metal, -0.04 * d), face),
            ) {
                fill_shader(pix, &path, sh);
                stroke(pix, &path, ts(col.shade, 0.45 * d), 1.0);
            }
            let mut rr = face - 2.0;
            let mut k = 0;
            while rr > 2.0 {
                if let Some(path) = circle(rr) {
                    let a = if k % 2 == 0 {
                        ts(col.light, 0.05 * d)
                    } else {
                        ts(col.shade, 0.05 * d)
                    };
                    stroke(pix, &path, a, 1.0);
                }
                rr -= 1.5;
                k += 1;
            }
            for (a0, a1) in [(PI * 1.15, PI * 1.45), (PI * 0.15, PI * 0.45)] {
                let mut pb = PathBuilder::new();
                pb.move_to(c, c);
                let steps = 12;
                for i in 0..=steps {
                    let th = a0 + (a1 - a0) * i as f32 / steps as f32;
                    pb.line_to(c + face * th.cos(), c + face * th.sin());
                }
                pb.close();
                if let Some(path) = pb.finish() {
                    let mut p = Paint::default();
                    p.set_color(ts(col.light, 0.10 * d));
                    p.anti_alias = true;
                    pix.fill_path(&path, &p, FillRule::Winding, id, None);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shape_has_a_name_of_its_own() {
        let mut names: Vec<&str> = KnobShape::ALL.iter().map(|s| s.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), KnobShape::ALL.len());
    }
}
