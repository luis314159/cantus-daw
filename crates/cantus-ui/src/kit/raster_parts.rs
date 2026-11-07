//! Small objects rasterised once into the layer atlas (round 37, ADR 0103): a lamp's glass
//! in its socket and a knob's well. Drawn as geometry they were the costliest part of a
//! desk (a gradient disc tessellates into rows and a feathered rim); as layers each is one
//! textured quad, the way knob bodies have been since round 21.

use super::color::{mix, opaque, shade};
use egui::Color32;
use tiny_skia::{
    FillRule, GradientStop, LinearGradient, Paint, PathBuilder, Pixmap, Point, Shader, SpreadMode,
    Stroke, Transform,
};

/// A token colour as tiny-skia's, at `a` opacity (its own alpha ignored).
pub fn ts(c: Color32, a: f32) -> tiny_skia::Color {
    let [r, g, b, _] = opaque(c).to_array();
    tiny_skia::Color::from_rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// A token colour whose alpha is its strength, as tiny-skia's, scaled by `k`.
pub fn tsa(c: Color32, k: f32) -> tiny_skia::Color {
    ts(c, c.a() as f32 / 255.0 * k)
}

fn disc(pix: &mut Pixmap, cx: f32, cy: f32, r: f32, shader: Option<Shader>) {
    if let (Some(path), Some(shader)) = (PathBuilder::from_circle(cx, cy, r.max(0.5)), shader) {
        let p = Paint {
            shader,
            anti_alias: true,
            ..Default::default()
        };
        pix.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }
}

fn vertical(
    cy: f32,
    r: f32,
    top: tiny_skia::Color,
    bottom: tiny_skia::Color,
) -> Option<Shader<'static>> {
    LinearGradient::new(
        Point::from_xy(0.0, cy - r),
        Point::from_xy(0.0, cy + r),
        vec![GradientStop::new(0.0, top), GradientStop::new(1.0, bottom)],
        SpreadMode::Pad,
        Transform::identity(),
    )
}

/// An arc's stroke, 1 px wide, round-capped.
pub fn arc(pix: &mut Pixmap, cx: f32, cy: f32, r: f32, col: tiny_skia::Color, a0: f32, a1: f32) {
    let mut pb = PathBuilder::new();
    let n = 32;
    for i in 0..=n {
        let t = a0 + (a1 - a0) * i as f32 / n as f32;
        let (x, y) = (cx + r * t.cos(), cy + r * t.sin());
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
        pix.stroke_path(&path, &p, &st, Transform::identity(), None);
    }
}

/// What a lamp is made of.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct LampLook {
    pub lit: bool,
    pub color: Color32,
    pub ground: Color32,
    pub light: Color32,
    pub lip: Color32,
    pub edge: Color32,
    pub specular: Color32,
}

/// Rasterise a lamp of radius `r` pixels with a lens of radius `lens` pixels, centred in
/// `pix`: the socket's lit lower lip, the socket darker at its top, the lens (lit from
/// inside: a hot top, the colour, a darker foot; or dark glass tinted by its colour), the
/// lens's edge, the specular on the glass.
pub fn lamp(pix: &mut Pixmap, r: f32, lens: f32, l: &LampLook) {
    let c = pix.width() as f32 * 0.5;
    disc(
        pix,
        c,
        c + 1.0,
        r,
        Some(Shader::SolidColor(tsa(l.lip, 1.0))),
    );
    disc(
        pix,
        c,
        c,
        r,
        vertical(
            c,
            r,
            ts(shade(l.ground, -0.26), 1.0),
            ts(shade(l.ground, -0.10), 1.0),
        ),
    );
    let (top, bottom) = if l.lit {
        (mix(l.color, l.light, 0.62), shade(l.color, 0.04))
    } else {
        let off = mix(shade(l.ground, -0.14), l.color, 0.24);
        (shade(off, -0.06), shade(off, 0.06))
    };
    disc(
        pix,
        c,
        c,
        lens,
        vertical(c, lens, ts(top, 1.0), ts(bottom, 1.0)),
    );
    arc(
        pix,
        c,
        c,
        lens - 0.5,
        tsa(l.edge, 0.8),
        0.0,
        std::f32::consts::TAU,
    );
    let k = if l.lit { 0.9 } else { 0.55 };
    let (sx, sy) = (c - lens * 0.30, c - lens * 0.36);
    disc(
        pix,
        sx,
        sy,
        (lens * 0.30).max(0.8),
        Some(Shader::SolidColor(tsa(l.specular, k))),
    );
}

/// What a knob's well is made of.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct WellLook {
    pub base: Color32,
    pub shadow: Color32,
    pub lip: Color32,
    pub depth: f32,
}

/// Rasterise a knob's well of radius `r` pixels, centred in `pix`: the lit lower lip, the
/// floor darker at the top where the lip shades it.
pub fn well(pix: &mut Pixmap, r: f32, w: &WellLook) {
    use std::f32::consts::PI;
    let c = pix.width() as f32 * 0.5;
    arc(pix, c, c, r - 0.5, tsa(w.lip, 1.0), PI * 0.12, PI * 0.88);
    let top = super::color::over(w.shadow.gamma_multiply(0.6), shade(w.base, -0.02 * w.depth));
    disc(
        pix,
        c,
        c,
        r - 1.0,
        vertical(c, r, ts(top, 1.0), ts(shade(w.base, 0.03 * w.depth), 1.0)),
    );
}
