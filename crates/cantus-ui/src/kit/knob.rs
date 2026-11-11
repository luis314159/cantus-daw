//! `Knob` — S 28 / M 36 / L 56 / XL 80 pt, plus `Mini` (20 pt) for mixer strips and rack
//! rows. Drawn as an object (round 37, ADR 0103): a cylinder standing in a round **well**
//! cut into the card; the 270° track runs in the well with the value arc drawn **from the
//! default** in the function hue, round-ended, always there; the body is a cached layer
//! (rasterised once per pixel size and theme — `rendering-performance.md` §5.1 measured it
//! 4× cheaper than tessellating 256 bodies): its shadow in the well, a skirt lit from
//! above, a domed cap with a bevel and the theme's specular; a pointer with its shadow;
//! modulation rings outside. The mini knob is the same object, smaller.

use super::brush::{Brush, Fill};
use super::cache;
use super::color::{shade, with_alpha};
use super::light::Material;
use super::skin::KnobShape;
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
    Tok::Well,
    Tok::Bevel,
    Tok::BevelShade,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
    Tok::Drop,
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
    Tok::Well,
    Tok::Bevel,
    Tok::BevelShade,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
    Tok::Drop,
];
/// The tokens the mini knob paints with in a dark theme: the ring knob's, without the
/// default's notch.
pub const MINI_TOKENS: &[Tok] = &[
    Tok::KnobBody,
    Tok::KnobRim,
    Tok::KnobPointer,
    Tok::Light,
    Tok::Dark,
    Tok::Focus,
    Tok::Well,
    Tok::Bevel,
    Tok::BevelShade,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
    Tok::Drop,
];
/// The mini knob in a light theme: a matte cap of the fader caps' colour, its pointer in
/// the text's.
pub const MINI_LIGHT_TOKENS: &[Tok] = &[
    Tok::Cap,
    Tok::Text,
    Tok::Light,
    Tok::Dark,
    Tok::Focus,
    Tok::Well,
    Tok::Bevel,
    Tok::BevelShade,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
    Tok::Drop,
];
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

/// The angle of position `p` (0…1) along the travel.
pub fn angle(p: f32) -> f32 {
    START + SWEEP * p.clamp(0.0, 1.0)
}

/// A ring knob's measures at a style's scale: the outer radius, the track's width and
/// radius, the body's radius, the pointer's width — what a skin's knob shapes
/// (`skin::knobs`) and a unit's own painter build on.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geometry {
    pub r: f32,
    pub track_w: f32,
    pub track_r: f32,
    pub body_r: f32,
    pub pointer_w: f32,
}

impl Geometry {
    pub fn of(size: KnobSize, s: &Style) -> Geometry {
        let r = s.s(size.diameter()) * 0.5;
        let tw = s.s(size.track());
        Geometry {
            r,
            track_w: tw,
            track_r: r - tw * 0.5,
            body_r: r - tw - s.s(size.gap()),
            pointer_w: s.s(size.pointer()),
        }
    }
}

/// The value's hue as the knob shows it: a step deeper while dragged.
pub fn value_hue(s: &Style, st: State) -> Color32 {
    if st.active {
        shade(s.hue, if s.t.dark { 0.08 } else { -0.08 })
    } else {
        s.hue
    }
}

/// The modulation rings outside the track, stacked outwards.
pub fn paint_rings(b: &mut Brush, c: Pos2, s: &Style, st: State, look: &KnobLook<'_>, r: f32) {
    let a = st.alpha();
    let v = look.value.clamp(0.0, 1.0);
    for (i, (depth, col)) in look.rings.iter().enumerate() {
        let rr = r + s.s(3.0) * (i as f32 + 1.0);
        let (p0, p1) = (v, (v + depth).clamp(0.0, 1.0));
        let (a0, a1) = (angle(p0.min(p1)), angle(p0.max(p1)));
        b.arc(
            c,
            rr,
            a0,
            a1,
            s.s(2.0),
            with_alpha(*col, 0.9).gamma_multiply(a),
            false,
        );
    }
}

/// The track at radius `tr`, `tw` wide; the notch at a default that is not an end; the
/// value arc from the default in `hue`.
#[allow(clippy::too_many_arguments)]
pub fn paint_track(
    b: &mut Brush,
    c: Pos2,
    s: &Style,
    st: State,
    look: &KnobLook<'_>,
    (tr, tw): (f32, f32),
    r: f32,
    hue: Color32,
) {
    let a = st.alpha();
    let f = |c: Color32| c.gamma_multiply(a);
    let (v, d) = (look.value.clamp(0.0, 1.0), look.default.clamp(0.0, 1.0));
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
    if !st.disabled && (v - d).abs() > 0.002 {
        b.arc(c, tr, angle(d.min(v)), angle(d.max(v)), tw, hue, true);
    }
}

/// Where modulation has the parameter now (ADR 0057): a dot riding radius `tr`.
pub fn paint_live(
    b: &mut Brush,
    c: Pos2,
    s: &Style,
    st: State,
    look: &KnobLook<'_>,
    tr: f32,
    tw: f32,
) {
    let a = st.alpha();
    if let Some(l) = look.live.filter(|_| !st.disabled) {
        let dir = Vec2::angled(angle(l));
        let dot = c + dir * tr;
        b.circle(
            dot,
            tw * 0.75 + b.px(),
            Fill::Solid(s.t.panel.gamma_multiply(a)),
        );
        b.circle(dot, tw * 0.75, Fill::Solid(s.t.hue_mod.gamma_multiply(a)));
    }
}

/// Paint a knob centred on `c`. The part covers a square of the ring's diameter (plus
/// 3 pt per modulation ring).
pub fn paint(b: &mut Brush, c: Pos2, s: &Style, st: State, look: &KnobLook<'_>) {
    // The centre on the pixel grid, so the cached body's texels land on pixels.
    let c = Pos2::new(b.snap(c.x), b.snap(c.y));
    let size = look.size;
    let r = s.s(size.diameter()) * 0.5;
    let area = Rect::from_center_size(c, Vec2::splat(2.0 * r));
    let shaped = size != KnobSize::Mini && !matches!(s.skin.knob, KnobShape::Plain);
    if !shaped && s.t.system.recessed() {
        // Slate and its siblings (round 39, `des-controls`): a groove, a cap lit from
        // above, the driven value's colour and sign.
        super::knob_slate::paint(b, c, s, st, look, super::automate::current_driving());
        return;
    }
    let _ = take_stepping(); // the classic knob has no detents: forget them
    let dark = s.t.dark;
    if size == KnobSize::Mini {
        let toks = if dark { MINI_TOKENS } else { MINI_LIGHT_TOKENS };
        b.register(area, "knob_mini", toks, &[s.hue_tok]);
    } else if shaped {
        let toks = super::skin::knobs::tokens(s.skin.knob, dark);
        b.register(area, "knob", toks, &[s.hue_tok]);
    } else {
        let toks = if dark { TOKENS } else { LIGHT_TOKENS };
        b.register(area, "knob", toks, &[s.hue_tok]);
    }
    if shaped {
        // A unit's knob (round 32, ADR 0074): the modulation rings and the focus ring are
        // the kit's on every shape; the rest is the shape's.
        let g = Geometry::of(size, s);
        paint_rings(b, c, s, st, look, g.r);
        match s.skin.knob {
            KnobShape::Custom(paint) => paint(b, c, s, st, look),
            shape => super::skin::knobs::paint(b, c, s, st, look, shape, &g),
        }
        if st.focused {
            b.circle_stroke(c, g.r + s.s(3.0), s.s(1.5), s.t.focus);
        }
        return;
    }
    let a = st.alpha();
    let f = |c: Color32| c.gamma_multiply(a);
    let hue = value_hue(s, st);
    let (v, d) = (look.value.clamp(0.0, 1.0), look.default.clamp(0.0, 1.0));
    let mini = size == KnobSize::Mini;
    // The mini knob's track runs just inside its well; a ring knob's where it always has.
    let (tw, tr, br) = if mini {
        let tw = s.s(1.5);
        (tw, r - s.s(0.25) - tw * 0.5, r - s.s(2.5))
    } else {
        let tw = s.s(size.track());
        (tw, r - tw * 0.5, r - tw - s.s(size.gap()))
    };

    if !mini {
        paint_rings(b, c, s, st, look, r);
    }
    // The well, the track in it, the notch at a default that is not an end.
    paint_well(b, c, r + s.s(if mini { 0.5 } else { 1.0 }), s, a);
    // A ring knob's track is the theme's; the mini knob's is the dark ring of its well.
    if !mini {
        b.arc(c, tr, angle(0.0), angle(1.0), tw, f(s.t.knob_track), true);
    }
    if !mini && d > 0.01 && d < 0.99 {
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
    // The body: a cached layer. A light theme's mini knob is a matte cap.
    let (made, ink) = if mini && !dark {
        (s.t.cap, s.t.text)
    } else {
        (s.t.knob_body, s.t.knob_pointer)
    };
    body_of(b, c, br, s, a, made);
    if mini && !dark {
        // r37:finish — a light theme's matte cap with its rim and a lit edge: an object,
        // not a flat disc (the judge, Paper).
        b.circle_stroke(c, br - 0.5 * b.px(), b.px() * 1.2, f(shade(made, -0.38)));
        b.circle_stroke(c, br - 2.0 * b.px(), b.px(), f(with_alpha(s.t.light, 0.55)));
    }
    if st.hovered && !st.disabled {
        let tint = with_alpha(s.t.light, if dark { 0.07 } else { 0.10 });
        b.circle(c, br, Fill::Solid(tint));
    }
    let pw = if mini { s.s(1.5) } else { s.s(size.pointer()) };
    pointer(b, c, br, v, pw, s, a, if mini { 0.12 } else { 0.32 }, ink);
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

/// The well a knob stands in, radius `r` at `c`: a round hollow cut into the card, its
/// floor darker at the top where the lip shades it, its lower lip catching the light.
pub fn paint_well(b: &mut Brush, c: Pos2, r: f32, s: &Style, alpha: f32) {
    let m = Material::of(&s.t);
    let look = super::raster_parts::WellLook {
        base: s.t.well,
        shadow: m.inner_shadow,
        lip: m.lip,
        depth: m.depth,
    };
    let ppp = b.ppp();
    // A cached layer (round 37): drawn as geometry, a gradient disc and its lip were the
    // costliest thing on a strip.
    let side = (2.0 * r * ppp + 2.0).ceil() as usize;
    let side = side + side % 2;
    let key = cache::key((
        "knob-well",
        side,
        (r * ppp).to_bits(),
        [look.base, look.shadow, look.lip].map(cache::ckey),
        look.depth.to_bits(),
    ));
    let uv = b.layer(key, side, side, |pix| {
        super::raster_parts::well(pix, r * ppp, &look)
    });
    if let Some(uv) = uv {
        let half = side as f32 / ppp * 0.5;
        let rect = Rect::from_min_max(c - Vec2::splat(half), c + Vec2::splat(half));
        b.layer_quad(rect, uv, alpha);
        return;
    }
    let f = |c: Color32| c.gamma_multiply(alpha);
    let px = b.px();
    let base = s.t.well;
    if m.lip.a() > 0 {
        b.arc(c, r - px * 0.5, PI * 0.12, PI * 0.88, px, f(m.lip), false);
    }
    b.circle(
        c,
        r - px,
        Fill::V(
            f(super::color::over(
                m.inner_shadow.gamma_multiply(0.6),
                shade(base, -0.02 * m.depth),
            )),
            f(shade(base, 0.03 * m.depth)),
        ),
    );
}

/// The pointer on a body of radius `br`: a light line from `from` of the radius to near
/// the edge, with the shadow it casts on the cap just under it.
#[allow(clippy::too_many_arguments)]
fn pointer(
    b: &mut Brush,
    c: Pos2,
    br: f32,
    v: f32,
    w: f32,
    s: &Style,
    alpha: f32,
    from: f32,
    ink: Color32,
) {
    let f = |c: Color32| c.gamma_multiply(alpha);
    let dir = Vec2::angled(angle(v));
    let (p0, p1) = (c + dir * (br * from), c + dir * (br * 0.84));
    let drop = Vec2::new(0.0, (0.75 * s.k).max(b.px()));
    b.segment(
        p0 + drop,
        p1 + drop,
        w,
        f(with_alpha(s.t.dark_shade, 0.45 * s.t.depth.min(1.0))),
        true,
    );
    b.segment(p0, p1, w, f(ink), true);
}

/// The body layer's side in physical pixels (even, so it centres on a pixel corner),
/// and its margin for the shadow.
fn body_px(br: f32, ppp: f32) -> (usize, f32) {
    let d = br * 2.0 * ppp;
    let margin = (d * 0.22).ceil().max(2.0);
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
pub fn body_vector(b: &mut Brush, c: Pos2, br: f32, s: &Style, alpha: f32, made: Color32) {
    let t = &s.t;
    let m = Material::of(t);
    let d = m.depth;
    let f = |c: Color32| c.gamma_multiply(alpha);
    let px = b.px();
    let body = made;
    b.circle(
        c + Vec2::new(0.0, br * 0.12),
        br * 1.15,
        Fill::Solid(f(m.drop.gamma_multiply(0.6))),
    );
    b.circle(c, br, Fill::Solid(f(shade(body, -0.30))));
    b.circle(
        c,
        br - px,
        Fill::V(f(shade(body, 0.14 * d)), f(shade(body, -0.16 * d))),
    );
    let face = br * 0.8;
    b.circle(
        c,
        face,
        Fill::V(f(shade(body, 0.10 * d)), f(shade(body, -0.06 * d))),
    );
    b.arc(
        c,
        face - 0.5 * px,
        PI * 1.08,
        PI * 1.92,
        px,
        f(m.bevel),
        false,
    );
    b.arc(
        c,
        face + 0.5 * px,
        PI * 0.12,
        PI * 0.88,
        px,
        f(m.bevel_shade),
        false,
    );
    b.circle(
        c + Vec2::new(-face * 0.32, -face * 0.40),
        face * 0.28,
        Fill::Solid(f(m.specular.gamma_multiply(0.30))),
    );
    if t.dark {
        b.circle_stroke(c, br - 0.5 * px, px, f(with_alpha(t.knob_rim, 0.3)));
    }
}

/// Paint a knob body of radius `br` at `c` from the layer cache.
pub fn body(b: &mut Brush, c: Pos2, br: f32, s: &Style, alpha: f32) {
    body_of(b, c, br, s, alpha, s.t.knob_body);
}

/// [`body`] made of `made` (a light theme's mini knob is a matte cap).
pub fn body_of(b: &mut Brush, c: Pos2, br: f32, s: &Style, alpha: f32, made: Color32) {
    if !CACHED_BODIES.load(std::sync::atomic::Ordering::Relaxed) {
        body_vector(b, c, br, s, alpha, made);
        return;
    }
    let ppp = b.ppp();
    let (side, _) = body_px(br, ppp);
    let mut look = BodyLook::of(&s.t);
    look.body = made;
    let key = cache::key((
        "knob-body-2",
        side,
        (br * ppp).to_bits(),
        [
            look.body,
            look.rim,
            look.bevel,
            look.bevel_shade,
            look.specular,
            look.drop,
        ]
        .map(cache::ckey),
        look.dark,
        look.depth.to_bits(),
    ));
    let uv = b.layer(key, side, side, |pix| raster_body(pix, br * ppp, &look));
    let Some(uv) = uv else {
        // No room in the atlas: draw the body as plain geometry.
        body_vector(b, c, br, s, alpha, made);
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
    bevel: Color32,
    bevel_shade: Color32,
    specular: Color32,
    drop: Color32,
    dark: bool,
    depth: f32,
}

impl BodyLook {
    fn of(t: &super::Tokens) -> BodyLook {
        let m = Material::of(t);
        BodyLook {
            body: t.knob_body,
            rim: t.knob_rim,
            bevel: m.bevel,
            bevel_shade: m.bevel_shade,
            specular: m.specular,
            drop: m.drop,
            dark: t.dark,
            depth: m.depth,
        }
    }
}

fn ts(c: Color32, a: f32) -> tiny_skia::Color {
    let [r, g, b, _] = super::color::opaque(c).to_array();
    tiny_skia::Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// A token colour whose alpha is its strength, as tiny-skia's, scaled by `k`.
fn tsa(c: Color32, k: f32) -> tiny_skia::Color {
    ts(c, c.a() as f32 / 255.0 * k)
}

/// Rasterise a knob body of radius `r` pixels, centred in `pix`: a cylinder standing in
/// its well — the shadow it casts, its skirt lit from above, a domed cap with a bevel
/// round its edge, and the specular the light leaves on the cap (bright on metal, faint
/// on a matte theme).
fn raster_body(pix: &mut tiny_skia::Pixmap, r: f32, look: &BodyLook) {
    use tiny_skia::{
        FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Point, RadialGradient, Shader,
        SpreadMode, Stroke, Transform,
    };
    let d = look.depth;
    let body = look.body;
    let w = pix.width() as f32;
    let c = w * 0.5;
    let id = Transform::identity();
    let circle = |cx: f32, cy: f32, rad: f32| PathBuilder::from_circle(cx, cy, rad.max(0.5));
    let fill = |pix: &mut tiny_skia::Pixmap, path: Option<tiny_skia::Path>, sh: Option<Shader>| {
        if let (Some(path), Some(shader)) = (path, sh) {
            let p = Paint {
                shader,
                anti_alias: true,
                ..Default::default()
            };
            pix.fill_path(&path, &p, FillRule::Winding, id, None);
        }
    };
    let radial = |cx: f32, cy: f32, rad: f32, stops: Vec<GradientStop>| {
        RadialGradient::new(
            Point::from_xy(cx, cy),
            0.0,
            Point::from_xy(cx, cy),
            rad,
            stops,
            SpreadMode::Pad,
            id,
        )
    };
    let linear = |top: Color32, bottom: Color32, rad: f32| {
        LinearGradient::new(
            Point::from_xy(c - rad * 0.35, c - rad),
            Point::from_xy(c + rad * 0.35, c + rad),
            vec![
                GradientStop::new(0.0, ts(top, 1.0)),
                GradientStop::new(1.0, ts(bottom, 1.0)),
            ],
            SpreadMode::Pad,
            id,
        )
    };

    // The shadow it casts into its well, a little below it.
    let dy = r * 0.12;
    fill(
        pix,
        circle(c, c + dy, r * 1.2),
        radial(
            c,
            c + dy,
            r * 1.2,
            vec![
                GradientStop::new(0.0, tsa(look.drop, 1.0)),
                GradientStop::new(0.8, tsa(look.drop, 0.75)),
                GradientStop::new(1.0, tsa(look.drop, 0.0)),
            ],
        ),
    );
    // The skirt: the cylinder's side, lit from above, its outline darker.
    fill(
        pix,
        circle(c, c, r),
        Some(tiny_skia::Shader::SolidColor(ts(shade(body, -0.30), 1.0))),
    );
    fill(
        pix,
        circle(c, c, r - 1.0),
        linear(shade(body, 0.14 * d), shade(body, -0.16 * d), r),
    );
    // The cap: a dome lit from the upper left.
    let face = r * 0.80;
    fill(
        pix,
        circle(c, c, face),
        radial(
            c - face * 0.30,
            c - face * 0.45,
            face * 1.55,
            vec![
                GradientStop::new(0.0, ts(shade(body, 0.18 * d), 1.0)),
                GradientStop::new(0.55, ts(shade(body, 0.04 * d), 1.0)),
                GradientStop::new(1.0, ts(shade(body, -0.08 * d), 1.0)),
            ],
        ),
    );
    // Round the cap's edge: light along the top, shade along the bottom.
    let arc = |pix: &mut tiny_skia::Pixmap, rad: f32, col: tiny_skia::Color, a0: f32, a1: f32| {
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
            p.set_color(col);
            p.anti_alias = true;
            let st = Stroke {
                width: 1.0,
                line_cap: tiny_skia::LineCap::Round,
                ..Default::default()
            };
            pix.stroke_path(&path, &p, &st, id, None);
        }
    };
    arc(pix, face - 0.5, tsa(look.bevel, 1.6), PI * 1.08, PI * 1.92);
    arc(
        pix,
        face + 0.5,
        tsa(look.bevel_shade, 1.0),
        PI * 0.12,
        PI * 0.88,
    );
    arc(pix, r - 1.5, tsa(look.bevel, 1.0), PI * 1.15, PI * 1.85);
    // The specular the light leaves on the cap, up and to the left.
    let (sx, sy) = (c - face * 0.32, c - face * 0.40);
    fill(
        pix,
        circle(sx, sy, face * 0.50),
        radial(
            sx,
            sy,
            face * 0.50,
            vec![
                GradientStop::new(0.0, tsa(look.specular, 0.75)),
                GradientStop::new(0.45, tsa(look.specular, 0.26)),
                GradientStop::new(1.0, tsa(look.specular, 0.0)),
            ],
        ),
    );
    if look.dark {
        arc(pix, r - 0.5, ts(look.rim, 0.30), 0.0, 2.0 * PI);
    }
}

// r39:el-controls — a stepped knob (`des-controls`): detents in place of the groove.
thread_local! {
    static STEPS: std::cell::Cell<Option<u8>> = const { std::cell::Cell::new(None) };
}

/// The next knob painted on this thread has `n` detents (2…24; `None`: a continuous knob):
/// dots in place of its groove, the chosen one lit. The knob painted takes it.
pub fn stepping(n: Option<u8>) {
    STEPS.with(|c| c.set(n.filter(|n| (2..=24).contains(n))));
}

/// The detents of the knob being painted (and forget them).
pub fn take_stepping() -> Option<u8> {
    STEPS.with(|c| c.take())
}
