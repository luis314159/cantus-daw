//! The fader in a recessed family (round 39, `des-controls`' fader sheet, owner-approved):
//! a 2-pt groove, a scale on its right with the 0 dB detent either side, and a cap lit from
//! above — 20 × 18 pt, the strip's brightest neutral (`fcap_*`), a 1-pt rim, a lit top,
//! two grips pressed in and an engraved index at the exact level. Held, its rim and index
//! take the accent; driven by a clip, the clip's colour and the lane sign beside it.
//! Frozen or disabled, the whole fader at 55 %.
//!
//! The travel is the kit's ([`super::fader::cap_rect`], round 37's 20-pt cap height), so
//! every view's drag maps as before; the cap is drawn centred where that cap was.

use super::automate::Drivers;
use super::brush::{Brush, Fill};
use super::color::{mix, opaque, with_alpha};
use super::drive_sign::{self, Painted};
use super::fader::{cap_rect, FaderLook, CAP};
use super::{State, Style, Tok};
use egui::{Color32, Pos2, Rect, Vec2};

/// The tokens a Slate fader paints with (for inspect mode).
pub const TOKENS: &[Tok] = &[
    Tok::InnerShadow,
    Tok::Well,
    Tok::Placeholder,
    Tok::Toolbar,
    Tok::FaderCapHi,
    Tok::FaderCapMid,
    Tok::FaderCapLo,
    Tok::Text,
    Tok::Focus,
    Tok::Auto,
    Tok::Disabled,
];

/// The Slate cap's size at 100 % (`des-controls`: 20 × 18, r 2).
pub const SLATE_CAP: Vec2 = Vec2::new(20.0, 18.0);

/// The cap's rect for `pos` in a fader occupying `rect`: centred where the kit's cap is.
pub fn slate_cap_rect(rect: Rect, s: &Style, pos: f32) -> Rect {
    let c = cap_rect(rect, s, pos).center();
    let w = (SLATE_CAP.x * s.k).min(rect.width());
    Rect::from_center_size(c, Vec2::new(w, SLATE_CAP.y * s.k))
}

/// Paint a Slate fader in `rect`, driven by `d`.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &FaderLook<'_>, d: Drivers) {
    b.register(rect, "fader", TOKENS, &[]);
    let t = &s.t;
    let night = t.dark;
    let a = if st.disabled { 0.55 } else { 1.0 };
    let f = |c: Color32| c.gamma_multiply(a);
    let px = b.px();
    let k = s.k;
    let cx = b.snap(rect.center().x);
    let top = rect.top() + CAP.y * k * 0.5;
    let bottom = rect.bottom() - CAP.y * k * 0.5;
    let edge = opaque(t.inner_shadow);
    // The groove: 2 pt, the travel ± 2, round.
    let gw = (2.0 * k / px).round().max(1.0) * px;
    let groove = Rect::from_min_max(
        Pos2::new(cx - gw * 0.5, top - 2.0 * k),
        Pos2::new(cx + gw * 0.5, bottom + 2.0 * k),
    );
    let gcol = if night { edge } else { mix(edge, t.well, 0.3) };
    b.rrect(groove, gw * 0.5, Fill::Solid(f(gcol)));
    // The scale on the right (3 pt, the 0 dB 5), the detent's tick on the left too.
    let tick = mix(t.toolbar, t.placeholder, 0.55);
    for &p in look.ticks {
        let y = b.snap_mid(bottom - (bottom - top) * p);
        let unity = look.unity.is_some_and(|u| (u - p).abs() < 1e-3);
        let len = if unity { 5.0 * k } else { 3.0 * k };
        let col = if unity { t.placeholder } else { tick };
        let x = groove.right() + 2.0 * k;
        b.hline(x, x + len, y, px, f(col));
        if unity {
            b.hline(
                groove.left() - 2.0 * k - len,
                groove.left() - 2.0 * k,
                y,
                px,
                f(col),
            );
        }
    }
    if let Some(u) = look
        .unity
        .filter(|u| !look.ticks.iter().any(|p| (p - u).abs() < 1e-3))
    {
        let y = b.snap_mid(bottom - (bottom - top) * u);
        let len = 5.0 * k;
        b.hline(
            groove.left() - 2.0 * k - len,
            groove.left() - 2.0 * k,
            y,
            px,
            f(t.placeholder),
        );
        b.hline(
            groove.right() + 2.0 * k,
            groove.right() + 2.0 * k + len,
            y,
            px,
            f(t.placeholder),
        );
    }
    // A VCA's level: a dashed ghost cap where the VCA puts it, the V sign at its left.
    if let Some(g) = super::automate::current_vca_ghost() {
        let ghost = b.snap_rect(slate_cap_rect(rect, s, g));
        super::automate::paint_vca_ghost(b, ghost, s, f(t.placeholder));
    }
    let cap = b.snap_rect(slate_cap_rect(rect, s, look.pos));
    paint_cap(b, cap, s, st, d, true);
    // The VCA's V stands by its ghost when there is one.
    let shown = Drivers {
        vca: d.vca && super::automate::current_vca_ghost().is_none(),
        ..d
    };
    drive_sign::fader_signs(b, cap, shown, s);
    super::automate::note_painted(Painted::Fader { cap, drew: d });
}

/// The cap in `cap` (`index`: draw the engraved level line; a slider's thumb draws its own,
/// upright).
pub fn paint_cap(b: &mut Brush, cap: Rect, s: &Style, st: State, d: Drivers, index: bool) {
    // A cap is static for its size, its theme and its state: one cached layer (ADR 0108 §2),
    // a strip of 64 faders one quad each.
    if cached_cap(b, cap, s, st, d, index) {
        return;
    }
    paint_cap_vector(b, cap, s, st, d, index);
}

/// The colours of a cap in `st`, driven by `d`: (shadow, rim, veil, lit top, grip, index's
/// dark line, index's ink).
#[allow(clippy::type_complexity)]
fn cap_colors(
    s: &Style,
    st: State,
    d: Drivers,
) -> (
    Color32,
    Color32,
    Option<Color32>,
    Color32,
    Color32,
    Option<Color32>,
    Color32,
) {
    let t = &s.t;
    let night = t.dark;
    let edge = opaque(t.inner_shadow);
    let shadow = if night {
        with_alpha(t.dark_shade, 0.45)
    } else {
        with_alpha(opaque(t.drop), 0.20)
    };
    let rim = if st.active {
        t.focus
    } else if d.clip {
        t.auto
    } else if night {
        edge
    } else {
        paper_rim(t, edge)
    };
    let veil = (st.hovered && !st.disabled).then(|| {
        if night {
            with_alpha(t.text, 0.07)
        } else {
            with_alpha(t.light, 0.6)
        }
    });
    let lit = if night {
        mix(t.fcap_hi, t.light, 0.35)
    } else {
        t.light
    };
    let grip = if night {
        mix(t.fcap_lo, edge, 0.55)
    } else {
        mix(t.fcap_lo, t.disabled, 0.7)
    };
    let engrave = night.then(|| mix(t.fcap_mid, edge, 0.6));
    let ink = if st.active {
        t.focus
    } else if d.clip {
        t.auto
    } else if night {
        mix(t.text, t.light, 0.6)
    } else {
        t.text
    };
    (shadow, rim, veil, lit, grip, engrave, ink)
}

fn ts(c: Color32, a: f32) -> tiny_skia::Color {
    let [r, g, b, al] = c.to_srgba_unmultiplied();
    tiny_skia::Color::from_rgba8(r, g, b, (al as f32 * a).round() as u8)
}

fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    let mut pb = tiny_skia::PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// The cap from the layer atlas; false when the atlas has no room.
fn cached_cap(b: &mut Brush, cap: Rect, s: &Style, st: State, d: Drivers, index: bool) -> bool {
    let t = &s.t;
    let ppp = b.ppp();
    let a: f32 = if st.disabled { 0.55 } else { 1.0 };
    let (shadow, rim, veil, lit, grip, engrave, ink) = cap_colors(s, st, d);
    let (w, h) = ((cap.width() * ppp).round(), (cap.height() * ppp).round());
    if w < 4.0 || h < 4.0 {
        return false;
    }
    let (wi, hi) = (w as usize, h as usize + 2);
    let k = s.k * ppp;
    let r = (s.r(2.0) * ppp).round();
    let key = super::cache::key((
        "slate-fader-cap",
        wi,
        hi,
        r.to_bits(),
        k.to_bits(),
        [
            shadow, rim, lit, grip, ink, t.fcap_hi, t.fcap_mid, t.fcap_lo,
        ]
        .map(super::cache::ckey),
        [veil, engrave].map(|c| c.map(super::cache::ckey)),
        index,
        a.to_bits(),
    ));
    let uv = b.layer(key, wi, hi, |pix| {
        use tiny_skia::{
            FillRule, GradientStop, LinearGradient, Paint, Point, Shader, SpreadMode, Transform,
        };
        let id = Transform::identity();
        let fill =
            |pix: &mut tiny_skia::Pixmap, x: f32, y: f32, ww: f32, hh: f32, rr: f32, sh: Shader| {
                if let Some(path) = rrect_path(x, y, ww, hh, rr) {
                    let p = Paint {
                        shader: sh,
                        anti_alias: true,
                        ..Default::default()
                    };
                    pix.fill_path(&path, &p, FillRule::Winding, id, None);
                }
            };
        let solid = |c: Color32| Shader::SolidColor(ts(c, a));
        fill(pix, 0.0, 1.0, w, h, r, solid(shadow));
        fill(pix, 0.0, 0.0, w, h, r, solid(rim));
        let grad = LinearGradient::new(
            Point::from_xy(0.0, 1.0),
            Point::from_xy(0.0, h - 1.0),
            vec![
                GradientStop::new(0.0, ts(t.fcap_hi, a)),
                GradientStop::new(0.5, ts(t.fcap_mid, a)),
                GradientStop::new(1.0, ts(t.fcap_lo, a)),
            ],
            SpreadMode::Pad,
            id,
        );
        if let Some(g) = grad {
            fill(pix, 1.0, 1.0, w - 2.0, h - 2.0, (r - 1.0).max(1.0), g);
        }
        if let Some(v) = veil {
            fill(
                pix,
                1.0,
                1.0,
                w - 2.0,
                h - 2.0,
                (r - 1.0).max(1.0),
                solid(v),
            );
        }
        let rect = |pix: &mut tiny_skia::Pixmap, x: f32, y: f32, ww: f32, hh: f32, c: Color32| {
            if let Some(rr) = tiny_skia::Rect::from_xywh(x, y, ww, hh) {
                let mut p = Paint::default();
                p.set_color(ts(c, a));
                pix.fill_rect(rr, &p, id, None);
            }
        };
        rect(pix, 2.0, 1.0, w - 4.0, 1.0, lit);
        if index {
            let cy = (h * 0.5).floor();
            let inset = (5.0 * k).round();
            let g = (4.0 * k).round();
            for y in [cy - g, cy + g] {
                rect(pix, inset, y, w - 2.0 * inset, 1.0, grip);
            }
            let iw = k.round().max(1.0);
            let iy = (cy - iw * 0.5).round();
            let x0 = (2.0 * k).round();
            if let Some(e) = engrave {
                rect(pix, x0, iy - 1.0, w - 2.0 * x0, 1.0, e);
            }
            rect(pix, x0, iy, w - 2.0 * x0, iw, ink);
        }
    });
    let Some(uv) = uv else {
        return false;
    };
    let min = Pos2::new(b.snap(cap.left()), b.snap(cap.top()));
    let quad = Rect::from_min_size(min, Vec2::new(wi as f32 / ppp, hi as f32 / ppp));
    b.layer_quad(quad, uv, 1.0);
    true
}

/// The cap as vectors (the atlas full, the bench).
fn paint_cap_vector(b: &mut Brush, cap: Rect, s: &Style, st: State, d: Drivers, index: bool) {
    let t = &s.t;
    let night = t.dark;
    let a = if st.disabled { 0.55 } else { 1.0 };
    let f = |c: Color32| c.gamma_multiply(a);
    let px = b.px();
    let k = s.k;
    let r = s.r(2.0);
    let edge = opaque(t.inner_shadow);
    // It stands on the strip: its own shape a pixel down.
    let shadow = if night {
        with_alpha(t.dark_shade, 0.45)
    } else {
        with_alpha(opaque(t.drop), 0.20)
    };
    b.rrect(cap.translate(Vec2::new(0.0, px)), r, Fill::Solid(f(shadow)));
    let rim = if st.active {
        t.focus
    } else if d.clip {
        t.auto
    } else if night {
        edge
    } else {
        paper_rim(t, edge)
    };
    b.rrect(cap, r, Fill::Solid(f(rim)));
    let face = cap.shrink(px);
    let rf = (r - px).max(px);
    b.rrect(
        face,
        rf,
        Fill::Stops(&[
            (0.0, f(t.fcap_hi)),
            (0.5, f(t.fcap_mid)),
            (1.0, f(t.fcap_lo)),
        ]),
    );
    if st.hovered && !st.disabled {
        let veil = if night {
            with_alpha(t.text, 0.07)
        } else {
            with_alpha(t.light, 0.6)
        };
        b.rrect(face, rf, Fill::Solid(veil));
    }
    // The lit top.
    let lit = if night {
        mix(t.fcap_hi, t.light, 0.35)
    } else {
        t.light
    };
    b.hline(
        cap.left() + 2.0 * px,
        cap.right() - 2.0 * px,
        cap.top() + 1.5 * px,
        px,
        f(lit),
    );
    if !index {
        return;
    }
    // Two grips pressed in either side of the index.
    let grip = if night {
        mix(t.fcap_lo, edge, 0.55)
    } else {
        mix(t.fcap_lo, t.disabled, 0.7)
    };
    let cy = cap.center().y;
    let inset = 5.0 * k;
    for dy in [-4.0 * k, 4.0 * k] {
        let y = b.snap_mid(cy + dy);
        b.hline(cap.left() + inset, cap.right() - inset, y, px, f(grip));
    }
    // The index: the exact level, a light line on a dark one (engraved).
    let iw = (k / px).round().max(1.0) * px;
    let ink = if st.active {
        t.focus
    } else if d.clip {
        t.auto
    } else if night {
        mix(t.text, t.light, 0.6)
    } else {
        t.text
    };
    let iy = b.snap(cy - iw * 0.5);
    let (x0, x1) = (cap.left() + 2.0 * k, cap.right() - 2.0 * k);
    if night {
        b.rect(
            Rect::from_min_max(Pos2::new(x0, iy - px), Pos2::new(x1, iy)),
            Fill::Solid(f(mix(t.fcap_mid, edge, 0.6))),
        );
    }
    b.rect(
        Rect::from_min_max(Pos2::new(x0, iy), Pos2::new(x1, iy + iw)),
        Fill::Solid(f(ink)),
    );
}

/// A light theme's fader-cap rim (round 39, *themes*): the well's edge a step toward the
/// placeholder ink, as designed, taken down until the white cap's outline reads at 3:1 on
/// its strip, selected or not (WCAG 1.4.11; Slate Paper's stood at 1.8:1).
pub(crate) fn paper_rim(t: &super::Tokens, edge: egui::Color32) -> egui::Color32 {
    super::color::ensure_contrast_all(
        mix(edge, t.placeholder, 0.25),
        &[t.strip, t.strip_selected],
        3.0,
    )
}
