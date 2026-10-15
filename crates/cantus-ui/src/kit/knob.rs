//! `Knob` — S 28 / M 36 / L 56 / XL 80 pt rings, plus a flat `Mini` (20 pt) for mixer
//! strips and rack rows (FL's pan knob). A 270° track; the value arc drawn **from the
//! default** in the function hue with round ends; a bevelled body that is a cached
//! layer (rasterised once per pixel size and theme — `rendering-performance.md` §5.1
//! measured it 4× cheaper than tessellating 256 bodies); a pointer; modulation rings
//! outside.

use super::brush::{Brush, Fill};
use super::cache;
use super::color::{shade, with_alpha};
use super::{State, Style, Tok};

/// The tokens a ring knob paints with in a dark theme (for inspect mode); its arc
/// takes the section hue.
pub const TOKENS: &[Tok] = &[
    Tok::KnobTrack,
    Tok::KnobBody,
    Tok::KnobRim,
    Tok::KnobPointer,
    Tok::TextDim,
    Tok::Light,
    Tok::Dark,
    Tok::Focus,
];
/// In a light theme: no rim.
pub const LIGHT_TOKENS: &[Tok] = &[
    Tok::KnobTrack,
    Tok::KnobBody,
    Tok::KnobPointer,
    Tok::TextDim,
    Tok::Light,
    Tok::Dark,
    Tok::Focus,
];
/// The tokens the flat mixer knob paints with.
pub const MINI_TOKENS: &[Tok] = &[Tok::Strip, Tok::Text, Tok::Light, Tok::Focus];
use egui::{Color32, Pos2, Rect, Vec2};
use std::f32::consts::PI;

/// Where the travel starts (bottom left) and how far it sweeps.
pub const START: f32 = 0.75 * PI;
pub const SWEEP: f32 = 1.5 * PI;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum KnobSize {
    /// 20 pt, flat: mixer strips, rack rows.
    Mini,
    S,
    M,
    L,
    /// One per device at most, for its primary parameter.
    XL,
}

impl KnobSize {
    pub const ALL: [KnobSize; 5] = [
        KnobSize::Mini,
        KnobSize::S,
        KnobSize::M,
        KnobSize::L,
        KnobSize::XL,
    ];

    /// The ring's outer diameter at 100 %.
    pub fn diameter(self) -> f32 {
        match self {
            KnobSize::Mini => 20.0,
            KnobSize::S => 28.0,
            KnobSize::M => 36.0,
            KnobSize::L => 56.0,
            KnobSize::XL => 80.0,
        }
    }

    fn track(self) -> f32 {
        match self {
            KnobSize::Mini => 1.5,
            KnobSize::S => 2.5,
            KnobSize::M => 3.0,
            KnobSize::L => 4.0,
            KnobSize::XL => 5.0,
        }
    }

    fn gap(self) -> f32 {
        match self {
            KnobSize::Mini => 1.5,
            KnobSize::S | KnobSize::M => 2.0,
            KnobSize::L => 3.0,
            KnobSize::XL => 4.0,
        }
    }

    fn pointer(self) -> f32 {
        match self {
            KnobSize::Mini | KnobSize::S => 1.5,
            KnobSize::M => 2.0,
            KnobSize::L => 2.5,
            KnobSize::XL => 3.0,
        }
    }
}

/// What a knob shows. Positions are 0…1 along the travel.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct KnobLook<'a> {
    pub size: KnobSize,
    pub value: f32,
    pub default: f32,
    /// Modulation arriving: (depth −1…1 of the travel, source colour), stacked outwards.
    pub rings: &'a [(f32, Color32)],
    /// Where modulation has the parameter now (0…1, ADR 0057): a dot on the track.
    pub live: Option<f32>,
}

impl KnobLook<'_> {
    pub fn new(size: KnobSize, value: f32, default: f32) -> KnobLook<'static> {
        KnobLook {
            size,
            value,
            default,
            rings: &[],
            live: None,
        }
    }
}

fn angle(p: f32) -> f32 {
    START + SWEEP * p.clamp(0.0, 1.0)
}

/// Paint a knob centred on `c`. The part covers a square of the ring's diameter (plus
/// 3 pt per modulation ring).
pub fn paint(b: &mut Brush, c: Pos2, s: &Style, st: State, look: &KnobLook<'_>) {
    // The centre on the pixel grid, so the cached body's texels land on pixels.
    let c = Pos2::new(b.snap(c.x), b.snap(c.y));
    let size = look.size;
    let r = s.s(size.diameter()) * 0.5;
    let area = Rect::from_center_size(c, Vec2::splat(2.0 * r));
    if size == KnobSize::Mini {
        b.register(area, "knob_mini", MINI_TOKENS, &[s.hue_tok]);
    } else {
        let toks = if s.t.dark { TOKENS } else { LIGHT_TOKENS };
        b.register(area, "knob", toks, &[s.hue_tok]);
    }
    let tw = s.s(size.track());
    let tr = r - tw * 0.5;
    let br = r - tw - s.s(size.gap());
    let a = st.alpha();
    let f = |c: Color32| c.gamma_multiply(a);
    let dark = s.t.dark;
    let hue = if st.active {
        shade(s.hue, if dark { 0.08 } else { -0.08 })
    } else {
        s.hue
    };
    let (v, d) = (look.value.clamp(0.0, 1.0), look.default.clamp(0.0, 1.0));

    if size == KnobSize::Mini {
        // FL's mixer knob: a flat face, a darker outline, a light pointer; the value
        // shows as a thin arc only when it is off its default.
        let dp = s.t.depth;
        let face = shade(s.t.strip, 0.035);
        b.circle(c, r, Fill::Solid(f(shade(s.t.strip, -(0.05 + 0.07 * dp)))));
        b.circle(
            c,
            r - b.px(),
            Fill::V(f(shade(face, 0.02 * dp)), f(shade(face, -0.02 * dp))),
        );
        if st.hovered {
            b.circle(c, r - b.px(), Fill::Solid(with_alpha(s.t.light, 0.05 * a)));
        }
        if !st.disabled && (v - d).abs() > 0.002 {
            let (a0, a1) = (angle(d.min(v)), angle(d.max(v)));
            b.arc(c, r + s.s(2.0), a0, a1, s.s(1.5), f(hue), true);
        }
        let dir = Vec2::angled(angle(v));
        b.segment(
            c + dir * (r * 0.25),
            c + dir * (r * 0.78),
            s.s(1.5),
            f(s.t.text),
            true,
        );
        if let Some(l) = look.live.filter(|_| !st.disabled) {
            let dir = Vec2::angled(angle(l));
            b.circle(c + dir * (r + s.s(2.0)), s.s(1.8), Fill::Solid(f(s.t.hue_mod)));
        }
        if st.focused {
            b.circle_stroke(c, r + s.s(3.5), s.s(1.5), s.t.focus);
        }
        return;
    }

    // Modulation rings, outside the track.
    for (i, (depth, col)) in look.rings.iter().enumerate() {
        let rr = r + s.s(3.0) * (i as f32 + 1.0);
        let (p0, p1) = (v, (v + depth).clamp(0.0, 1.0));
        let (a0, a1) = (angle(p0.min(p1)), angle(p0.max(p1)));
        b.arc(c, rr, a0, a1, s.s(2.0), f(with_alpha(*col, 0.9)), false);
    }

    // The track, and the notch at a default that is not an end.
    b.arc(c, tr, angle(0.0), angle(1.0), tw, f(s.t.knob_track), true);
    if d > 0.01 && d < 0.99 {
        let dir = Vec2::angled(angle(d));
        b.segment(
            c + dir * (r + s.s(1.0)),
            c + dir * (r + s.s(3.0)),
            s.s(1.5),
            f(s.t.text_dim),
            false,
        );
    }
    // The value arc, from the default, round-ended.
    if !st.disabled && (v - d).abs() > 0.002 {
        b.arc(c, tr, angle(d.min(v)), angle(d.max(v)), tw, hue, true);
    }

    // The body: a cached layer.
    body(b, c, br, s, a);
    if st.hovered && !st.disabled {
        let tint = if dark {
            with_alpha(s.t.light, 0.07)
        } else {
            with_alpha(s.t.light, 0.10)
        };
        b.circle(c, br, Fill::Solid(tint));
    }

    // The pointer.
    let dir = Vec2::angled(angle(v));
    let pw = s.s(size.pointer());
    b.segment(
        c + dir * (br * 0.36),
        c + dir * (br * 0.86),
        pw,
        f(s.t.knob_pointer),
        true,
    );
    // Where modulation has it now (ADR 0057): a dot riding the track.
    if let Some(l) = look.live.filter(|_| !st.disabled) {
        let dir = Vec2::angled(angle(l));
        let dot = c + dir * tr;
        b.circle(dot, tw * 0.75 + b.px(), Fill::Solid(f(s.t.panel)));
        b.circle(dot, tw * 0.75, Fill::Solid(f(s.t.hue_mod)));
    }
    if st.focused {
        b.circle_stroke(c, r + s.s(3.0), s.s(1.5), s.t.focus);
    }
}

/// The body layer's side in physical pixels (even, so it centres on a pixel corner),
/// and its margin for the shadow.
fn body_px(br: f32, ppp: f32) -> (usize, f32) {
    let d = br * 2.0 * ppp;
    let margin = (d * 0.16).ceil().max(2.0);
    let side = (d + 2.0 * margin).ceil() as usize;
    (side + side % 2, margin)
}

static CACHED_BODIES: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Draw knob bodies from the layer cache (the default) or as vector geometry — the
/// switch exists so the bench can measure what the cache saves.
pub fn set_cached_bodies(on: bool) {
    CACHED_BODIES.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The body as plain geometry: the same layers the cached texture holds, tessellated —
/// used when the atlas has no room, and by the bench.
pub fn body_vector(b: &mut Brush, c: Pos2, br: f32, s: &Style, alpha: f32) {
    let t = &s.t;
    let d = t.depth;
    let f = |c: Color32| c.gamma_multiply(alpha);
    let px = b.px();
    b.circle(
        c + Vec2::new(0.0, br * 0.1),
        br * 1.12,
        Fill::Solid(f(with_alpha(t.dark_shade, 0.25 * d))),
    );
    b.circle(c, br, Fill::Solid(f(shade(t.knob_body, -0.14))));
    b.circle(
        c,
        br - px,
        Fill::V(
            f(shade(t.knob_body, 0.16 * d)),
            f(shade(t.knob_body, -0.07 * d)),
        ),
    );
    b.circle(
        c,
        br * 0.8,
        Fill::V(
            f(shade(t.knob_body, -0.02 * d)),
            f(shade(t.knob_body, 0.06 * d)),
        ),
    );
    b.arc(
        c,
        br - 1.5 * px,
        PI * 1.12,
        PI * 1.88,
        px,
        f(with_alpha(t.light, 0.2 * d)),
        false,
    );
    b.arc(
        c,
        br * 0.8 - 0.5 * px,
        PI * 1.15,
        PI * 1.85,
        px,
        f(with_alpha(t.light, 0.1 * d)),
        false,
    );
    b.arc(
        c,
        br * 0.8 + 0.5 * px,
        PI * 0.15,
        PI * 0.85,
        px,
        f(with_alpha(t.dark_shade, 0.35 * d)),
        false,
    );
    if t.dark {
        b.circle_stroke(c, br - 0.5 * px, px, f(with_alpha(t.knob_rim, 0.3)));
    }
}

/// Paint a knob body of radius `br` at `c` from the layer cache.
pub fn body(b: &mut Brush, c: Pos2, br: f32, s: &Style, alpha: f32) {
    if !CACHED_BODIES.load(std::sync::atomic::Ordering::Relaxed) {
        body_vector(b, c, br, s, alpha);
        return;
    }
    let ppp = b.ppp();
    let (side, _) = body_px(br, ppp);
    let t = &s.t;
    let key = cache::key((
        "knob-body",
        side,
        (br * ppp).to_bits(),
        cache::ckey(t.knob_body),
        cache::ckey(t.knob_rim),
        cache::ckey(t.light),
        cache::ckey(t.dark_shade),
        t.dark,
        t.depth.to_bits(),
    ));
    let look = BodyLook {
        body: t.knob_body,
        rim: t.knob_rim,
        light: t.light,
        shade: t.dark_shade,
        dark: t.dark,
        depth: t.depth,
    };
    let uv = b.layer(key, side, side, |pix| raster_body(pix, br * ppp, &look));
    let Some(uv) = uv else {
        // No room in the atlas: draw the body as plain geometry.
        body_vector(b, c, br, s, alpha);
        return;
    };
    let half = side as f32 / ppp * 0.5;
    let rect = Rect::from_min_max(c - Vec2::splat(half), c + Vec2::splat(half));
    b.layer_quad(rect, uv, alpha);
}

/// The colours and relief a body is rasterised with.
struct BodyLook {
    body: Color32,
    rim: Color32,
    light: Color32,
    shade: Color32,
    dark: bool,
    depth: f32,
}

fn ts(c: Color32, a: f32) -> tiny_skia::Color {
    let [r, g, b, _] = super::color::opaque(c).to_array();
    tiny_skia::Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// Rasterise a knob body of radius `r` pixels, centred in `pix`.
fn raster_body(pix: &mut tiny_skia::Pixmap, r: f32, look: &BodyLook) {
    let BodyLook {
        body,
        rim,
        light,
        shade: sh,
        dark,
        depth: d,
    } = *look;
    use tiny_skia::{
        FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Point, RadialGradient,
        SpreadMode, Stroke, Transform,
    };
    let w = pix.width() as f32;
    let c = w * 0.5;
    let id = Transform::identity();
    let circle = |rad: f32| PathBuilder::from_circle(c, c, rad.max(0.5));

    // A soft shadow under the body, a little below it.
    if let (Some(sh), Some(path)) = (
        RadialGradient::new(
            Point::from_xy(c, c + r * 0.10),
            0.0,
            Point::from_xy(c, c + r * 0.10),
            r * 1.22,
            vec![
                GradientStop::new(0.0, ts(sh, d * if dark { 0.55 } else { 0.30 })),
                GradientStop::new(0.78, ts(sh, d * if dark { 0.40 } else { 0.18 })),
                GradientStop::new(1.0, ts(sh, 0.0)),
            ],
            SpreadMode::Pad,
            id,
        ),
        circle(r * 1.22),
    ) {
        let p = Paint {
            shader: sh,
            anti_alias: true,
            ..Default::default()
        };
        pix.fill_path(&path, &p, FillRule::Winding, id, None);
    }

    let lin = |a: Color32, b: Color32, rad: f32| {
        LinearGradient::new(
            Point::from_xy(c - rad * 0.55, c - rad * 0.8),
            Point::from_xy(c + rad * 0.55, c + rad * 0.8),
            vec![
                GradientStop::new(0.0, ts(a, 1.0)),
                GradientStop::new(1.0, ts(b, 1.0)),
            ],
            SpreadMode::Pad,
            id,
        )
    };
    let fill = |pix: &mut tiny_skia::Pixmap, rad: f32, a: Color32, b: Color32| {
        if let (Some(sh), Some(path)) = (lin(a, b, rad), circle(rad)) {
            let p = Paint {
                shader: sh,
                anti_alias: true,
                ..Default::default()
            };
            pix.fill_path(&path, &p, FillRule::Winding, id, None);
        }
    };
    // The outline, the bevelled skirt (lit top-left), then the face inside it, lit the
    // other way so the step between them reads as an edge.
    fill(pix, r, shade(body, -0.14), shade(body, -0.14));
    fill(pix, r - 1.0, shade(body, 0.16 * d), shade(body, -0.07 * d));
    let face = r * 0.80;
    fill(pix, face, shade(body, -0.02 * d), shade(body, 0.06 * d));

    let stroke = |pix: &mut tiny_skia::Pixmap,
                  rad: f32,
                  color: tiny_skia::Color,
                  width: f32,
                  a0: f32,
                  a1: f32| {
        let mut pb = PathBuilder::new();
        let n = 32;
        for i in 0..=n {
            let t = a0 + (a1 - a0) * i as f32 / n as f32;
            let (x, y) = (c + rad * t.cos(), c + rad * t.sin());
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        if let Some(path) = pb.finish() {
            let mut p = Paint::default();
            p.set_color(color);
            p.anti_alias = true;
            let st = Stroke {
                width,
                line_cap: tiny_skia::LineCap::Round,
                ..Default::default()
            };
            pix.stroke_path(&path, &p, &st, id, None);
        }
    };
    // Light along the top of the skirt and of the face; the rim on dark themes.
    stroke(pix, r - 1.5, ts(light, 0.20 * d), 1.0, PI * 1.12, PI * 1.88);
    stroke(
        pix,
        face - 0.5,
        ts(light, 0.10 * d),
        1.0,
        PI * 1.15,
        PI * 1.85,
    );
    stroke(pix, face + 0.5, ts(sh, 0.35 * d), 1.0, PI * 0.15, PI * 0.85);
    if dark {
        stroke(pix, r - 0.5, ts(rim, 0.30), 1.0, 0.0, 2.0 * PI);
    }
}
