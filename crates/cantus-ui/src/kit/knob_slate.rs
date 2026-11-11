//! The knob in a recessed family (round 39, `des-controls`' knob sheet, owner-approved):
//! a 270° groove (the scale) with the value arc on it, a gap, a cap lit from above with one
//! pointer. The groove's open quarter at the bottom is where a driven value shows its sign.
//!
//! | kit size | where | D | groove | gap | cap Ø | pointer | sign |
//! |---|---|---|---|---|---|---|---|
//! | Mini | strips, inline rows (the sheet's S) | 20 | 2 | 2 | 12 | 1.5 | dot 4 |
//! | S | narrow rows | 28 | 2 | 3 | 18 | 2 | 6 |
//! | M | panels, inspector, effects | 32 | 2 | 3 | 22 | 2 | 6 |
//! | L | an editor's main controls | 52 | 3 | 4 | 38 | 2.5 | 8 |
//! | XL | one per device | 76 | 4 | 5 | 58 | 3 | 10 |
//!
//! (Sizes in points at 100 %; the kit's allocation per size is unchanged, the drawing sits
//! centred in it.) The cap — its shadow, its rim, the radial light and the crescent — is
//! rasterised once per size and theme into the layer atlas (ADR 0108 §2, as the round-37
//! bodies were); the groove, the arcs and the pointer are vectors.

use super::automate::Drivers;
use super::brush::{Brush, Fill};
use super::color::{mix, opaque, with_alpha};
use super::drive_sign::{self, Painted};
use super::knob::{angle, KnobLook, KnobSize};
use super::{cache, State, Style, Tok};
use egui::{Color32, Pos2, Rect, Vec2};

/// The tokens a Slate knob paints with (for inspect mode).
pub const TOKENS: &[Tok] = &[
    Tok::Well,
    Tok::InnerShadow,
    Tok::Text,
    Tok::TextDim,
    Tok::Placeholder,
    Tok::Disabled,
    Tok::Focus,
    Tok::KnobCapHi,
    Tok::KnobCapMid,
    Tok::KnobCapLo,
    Tok::Drop,
    Tok::Auto,
    Tok::Modulated,
];

/// A knob's measures in points at 100 % (`des-controls`' `KNOB` table).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Geo {
    pub d: f32,
    pub ring: f32,
    pub gap: f32,
    pub ptr: f32,
    /// Where the pointer starts, a fraction of the cap's radius.
    pub p0: f32,
    /// The sign's height (the small knob's dot's diameter).
    pub sign: f32,
    /// The focus ring's distance outside the knob.
    pub focus: f32,
}

pub fn geo(size: KnobSize) -> Geo {
    let g = |d, ring, gap, ptr, p0, sign, focus| Geo {
        d,
        ring,
        gap,
        ptr,
        p0,
        sign,
        focus,
    };
    match size {
        KnobSize::Mini => g(20.0, 2.0, 2.0, 1.5, 0.20, 4.0, 2.0),
        KnobSize::S => g(28.0, 2.0, 3.0, 2.0, 0.28, 6.0, 2.0),
        KnobSize::M => g(32.0, 2.0, 3.0, 2.0, 0.28, 6.0, 2.0),
        KnobSize::L => g(52.0, 3.0, 4.0, 2.5, 0.34, 8.0, 3.0),
        KnobSize::XL => g(76.0, 4.0, 5.0, 3.0, 0.34, 10.0, 3.0),
    }
}

/// A knob whose default sits in the middle of its travel is bipolar: its arc runs from the
/// top.
pub fn bipolar(look: &KnobLook<'_>) -> bool {
    (look.default - 0.5).abs() < 0.01
}

/// The value arc's colour in `st`, driven by `d`.
pub fn arc_color(s: &Style, st: State, d: Drivers) -> Color32 {
    let t = &s.t;
    if st.disabled {
        t.disabled
    } else if st.active {
        t.focus
    } else if d.clip {
        t.auto
    } else if st.focused {
        t.text
    } else if st.hovered {
        mix(t.text_dim, t.text, 0.6)
    } else {
        mix(t.text_dim, t.placeholder, 0.25)
    }
}

/// The groove: the well (Night), a well a step toward its edge (Paper).
pub fn groove_color(s: &Style) -> Color32 {
    if s.t.dark {
        s.t.well
    } else {
        mix(s.t.well, opaque(s.t.inner_shadow), 0.55)
    }
}

/// Paint a Slate knob centred on `c`, driven by `d`.
pub fn paint(b: &mut Brush, c: Pos2, s: &Style, st: State, look: &KnobLook<'_>, d: Drivers) {
    let t = &s.t;
    let g = geo(look.size);
    let k = s.k;
    let ring = (g.ring * k / b.px()).round().max(1.0) * b.px();
    let r_out = g.d * k * 0.5;
    let groove_r = r_out - ring * 0.5;
    let cap_r = r_out - ring - g.gap * k;
    let mini = look.size == KnobSize::Mini;
    b.register(
        Rect::from_center_size(c, Vec2::splat(g.d * k)),
        "knob",
        TOKENS,
        &[],
    );
    let v = look.value.clamp(0.0, 1.0);
    let bip = bipolar(look);
    let from = if bip { 0.5 } else { 0.0 };
    // The groove, the bipolar notch and the cap: one cached layer (static for a size, a
    // theme and a state), the value's arc and pointer vectors over it.
    let steps = super::knob::take_stepping();
    let groove = Groove {
        r: groove_r,
        ring,
        notch: bip,
    };
    if let Some(n) = steps {
        // Stepped (`des-controls`): a dot per detent on the groove's line, the chosen one
        // larger — ink 2 at rest, ink 1 under the hand, the accent held; then the cap.
        let chosen = (v * (n - 1) as f32).round() as u8;
        for i in 0..n {
            let at = c + Vec2::angled(angle(i as f32 / (n - 1) as f32)) * groove_r;
            let on = i == chosen;
            let col = if st.disabled {
                t.disabled
            } else if on && st.active {
                t.focus
            } else if on && st.hovered {
                t.text
            } else if on {
                t.text_dim
            } else {
                mix(t.toolbar, t.placeholder, 0.7)
            };
            let r = ring * if on { 0.75 } else { 0.5 };
            b.circle(at, r.max(b.px() * 0.75), Fill::Solid(col));
        }
        cap(b, c, cap_r, s, st, None);
    } else {
        cap(b, c, cap_r, s, st, Some(groove));
    }
    // A modulator's swing: a band a little wider than the groove, in its colour at 38 %.
    if !st.disabled {
        for (depth, _) in look.rings {
            let (a, z) = (v.min(v + depth), v.max(v + depth));
            let (a, z) = (a.clamp(0.0, 1.0), z.clamp(0.0, 1.0));
            if z - a > 0.002 {
                b.arc(
                    c,
                    groove_r,
                    angle(a),
                    angle(z),
                    ring + k,
                    with_alpha(t.modulated, 0.38),
                    false,
                );
            }
        }
    }
    let arc = arc_color(s, st, d);
    if steps.is_none() && (v - from).abs() > 0.002 {
        b.arc(
            c,
            groove_r,
            angle(from.min(v)),
            angle(from.max(v)),
            ring,
            arc,
            true,
        );
    }
    // Automated and held by the hand: a violet tick across the groove where the clip had it.
    if let Some(h) = super::automate::current_held_clip().filter(|_| d.clip && st.active) {
        let dir = Vec2::angled(angle(h.clamp(0.0, 1.0)));
        let w = if s.k * b.ppp() > 1.25 { 2.0 } else { 1.5 } * s.k;
        b.segment(
            c + dir * (groove_r - ring),
            c + dir * (groove_r + ring),
            w,
            t.auto,
            true,
        );
    }
    // Where the modulator has it now: a dot in its colour, knocked out of the panel.
    if let Some(l) = look.live.filter(|_| !st.disabled) {
        let at = c + Vec2::angled(angle(l)) * groove_r;
        b.circle(at, ring * 0.5 + k, Fill::Solid(t.panel));
        b.circle(at, ring * 0.5 + 0.5 * k, Fill::Solid(t.modulated));
    }
    // The pointer: from near the centre to just inside the rim.
    let ptr = g.ptr * k;
    let pc = if st.disabled {
        t.disabled
    } else if d.clip && !st.active {
        t.auto
    } else {
        t.text
    };
    let dir = Vec2::angled(angle(v));
    b.segment(
        c + dir * (cap_r * g.p0),
        c + dir * (cap_r - ptr * 0.5 - 1.5 * k),
        ptr,
        pc,
        true,
    );
    if st.focused {
        b.circle_stroke(
            c,
            r_out + g.focus * k + 0.5 * b.px(),
            b.px(),
            with_alpha(t.focus, 0.85),
        );
    }
    drive_sign::knob_signs(b, c, (groove_r, ring, cap_r), g.sign * k, mini, d, s);
    super::automate::note_painted(Painted::Knob {
        c,
        groove_r,
        ring,
        cap_r,
        ptr,
        p0: g.p0,
        sign_h: g.sign * k,
        value: v,
        from,
        mini,
        drew: d,
    });
}

/// A knob's groove, drawn with its cap: its radius and width, and the bipolar notch.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Groove {
    pub r: f32,
    pub ring: f32,
    pub notch: bool,
}

/// The colours a cap is rasterised with.
#[derive(Copy, Clone)]
struct CapLook {
    groove: Color32,
    notch: Color32,
    /// The veil under the hand.
    veil: Option<Color32>,
    drop: Color32,
    rim: Color32,
    hi: Color32,
    mid: Color32,
    lo: Color32,
    crescent: Color32,
    /// The flat fill of a disabled cap (no light, no shadow).
    flat: Option<Color32>,
}

/// The cap of radius `r` at `c`: cached per pixel size and colours, hovered under a veil.
pub fn cap(b: &mut Brush, c: Pos2, r: f32, s: &Style, st: State, groove: Option<Groove>) {
    let t = &s.t;
    let night = t.dark;
    let look = CapLook {
        groove: groove_color(s).gamma_multiply(if st.disabled { 0.5 } else { 1.0 }),
        notch: if st.disabled {
            t.disabled
        } else {
            t.placeholder
        },
        veil: (st.hovered && !st.disabled).then(|| {
            if night {
                with_alpha(t.text, 0.07)
            } else {
                with_alpha(t.light, 0.5)
            }
        }),
        drop: t.drop,
        rim: if st.disabled {
            if night {
                opaque(t.inner_shadow)
            } else {
                t.grid_bar
            }
        } else if night {
            opaque(t.inner_shadow)
        } else {
            mix(opaque(t.inner_shadow), t.placeholder, 0.15)
        },
        hi: t.cap_hi,
        mid: t.cap_mid,
        lo: t.cap_lo,
        crescent: if night {
            with_alpha(mix(t.cap_hi, t.light, 0.18), 0.75)
        } else {
            t.light
        },
        flat: st.disabled.then(|| mix(t.toolbar, t.well, 0.6)),
    };
    let ppp = b.ppp();
    let rp = r * ppp;
    // The groove in device pixels: its radius, width and the notch's (width, height).
    let gp = groove.map(|g| GroovePx {
        r: g.r * ppp,
        ring: (g.ring * ppp).round().max(1.0),
        notch: g
            .notch
            .then(|| ((s.k * ppp).round().max(1.0), (2.0 * s.k * ppp).round())),
    });
    let outer = gp.map_or(rp, |g| {
        g.r + g.ring * 0.5 + g.notch.map_or(0.0, |n| n.1 + 1.0)
    });
    let margin = (2.0 * s.k * ppp).ceil() + 2.0;
    let side = (2.0 * outer + 2.0 * margin).ceil() as usize;
    let side = side + side % 2;
    let key = cache::key((
        "slate-knob-body",
        side,
        rp.to_bits(),
        [
            look.drop,
            look.rim,
            look.hi,
            look.mid,
            look.lo,
            look.crescent,
        ]
        .map(cache::ckey),
        [Some(look.groove), Some(look.notch), look.flat, look.veil].map(|c| c.map(cache::ckey)),
        gp.map(|g| {
            (
                g.r.to_bits(),
                g.ring.to_bits(),
                g.notch.map(|n| (n.0.to_bits(), n.1.to_bits())),
            )
        }),
        (s.k * ppp).to_bits(),
    ));
    let down = (s.k * ppp).round().max(1.0);
    let uv = b.layer(key, side, side, |pix| raster_cap(pix, rp, down, &look, gp));
    match uv {
        Some(uv) => {
            let half = side as f32 / ppp * 0.5;
            let rect = Rect::from_min_max(c - Vec2::splat(half), c + Vec2::splat(half));
            b.layer_quad(rect, uv, 1.0);
        }
        None => {
            // No room in the atlas: the same layers as vectors.
            let px = b.px();
            if let Some(g) = groove {
                b.arc(c, g.r, angle(0.0), angle(1.0), g.ring, look.groove, true);
                if g.notch {
                    let (w, h) = (s.k.max(px), 2.0 * s.k);
                    let top = c.y - g.r - g.ring * 0.5 - h;
                    b.rect(
                        Rect::from_min_size(Pos2::new(b.snap(c.x - w * 0.5), top), Vec2::new(w, h)),
                        Fill::Solid(look.notch),
                    );
                }
            }
            if let Some(f) = look.flat {
                b.circle(c, r, Fill::Solid(look.rim));
                b.circle(c, r - px, Fill::Solid(f));
            } else {
                b.circle(c + Vec2::new(0.0, down * px), r, Fill::Solid(look.drop));
                b.circle(c, r, Fill::Solid(look.rim));
                b.circle(c, r - px, Fill::V(look.hi, look.lo));
            }
            if let Some(v) = look.veil {
                b.circle(c, r - px, Fill::Solid(v));
            }
        }
    }
}

/// A groove in device pixels.
#[derive(Copy, Clone)]
struct GroovePx {
    r: f32,
    ring: f32,
    /// The bipolar notch's width and height.
    notch: Option<(f32, f32)>,
}

fn ts(c: Color32) -> tiny_skia::Color {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    tiny_skia::Color::from_rgba8(r, g, b, a)
}

/// Rasterise a cap of radius `r` pixels, centred in `pix`: its shadow `down` pixels below,
/// a 1-px rim, the radial light from the upper left, the lit crescent along its top.
fn raster_cap(
    pix: &mut tiny_skia::Pixmap,
    r: f32,
    down: f32,
    look: &CapLook,
    groove: Option<GroovePx>,
) {
    use tiny_skia::{
        FillRule, GradientStop, Paint, PathBuilder, Point, RadialGradient, Shader, SpreadMode,
        Stroke, Transform,
    };
    let c = pix.width() as f32 * 0.5;
    let id = Transform::identity();
    let fill = |pix: &mut tiny_skia::Pixmap, cy: f32, rad: f32, shader: Shader| {
        if let Some(path) = PathBuilder::from_circle(c, cy, rad.max(0.5)) {
            let p = Paint {
                shader,
                anti_alias: true,
                ..Default::default()
            };
            pix.fill_path(&path, &p, FillRule::Winding, id, None);
        }
    };
    if let Some(g) = groove {
        // The 270° groove, round-ended, from the lower left over the top to the lower right.
        let mut pb = PathBuilder::new();
        let n = 72;
        for i in 0..=n {
            let a = (135.0 + 270.0 * i as f32 / n as f32).to_radians();
            let (x, y) = (c + g.r * a.cos(), c + g.r * a.sin());
            if i == 0 {
                pb.move_to(x, y);
            } else {
                pb.line_to(x, y);
            }
        }
        if let Some(path) = pb.finish() {
            let mut p = Paint::default();
            p.set_color(ts(look.groove));
            p.anti_alias = true;
            let st = Stroke {
                width: g.ring,
                line_cap: tiny_skia::LineCap::Round,
                ..Default::default()
            };
            pix.stroke_path(&path, &p, &st, id, None);
        }
        if let Some((w, h)) = g.notch {
            let x = (c - w * 0.5).round();
            let top = c - g.r - g.ring * 0.5 - h;
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, top, w, h) {
                let mut p = Paint::default();
                p.set_color(ts(look.notch));
                pix.fill_rect(rect, &p, id, None);
            }
        }
    }
    if let Some(flat) = look.flat {
        fill(pix, c, r, Shader::SolidColor(ts(look.rim)));
        fill(pix, c, r - 1.0, Shader::SolidColor(ts(flat)));
        return;
    }
    fill(pix, c + down, r, Shader::SolidColor(ts(look.drop)));
    fill(pix, c, r, Shader::SolidColor(ts(look.rim)));
    // The light: SVG's radial gradient at (40 %, 28 %) of the face's box, r 78 %.
    let f = r - 1.0;
    let (gx, gy) = (c - f + 0.8 * f, c - f + 0.56 * f);
    let shader = RadialGradient::new(
        Point::from_xy(gx, gy),
        0.0,
        Point::from_xy(gx, gy),
        1.56 * f,
        vec![
            GradientStop::new(0.0, ts(look.hi)),
            GradientStop::new(0.55, ts(look.mid)),
            GradientStop::new(1.0, ts(look.lo)),
        ],
        SpreadMode::Pad,
        id,
    );
    if let Some(sh) = shader {
        fill(pix, c, f, sh);
    }
    // The crescent: ±58° about the top, 1 px, 1.5 px inside the rim.
    let rad = r - 1.5;
    let mut pb = PathBuilder::new();
    let n = 24;
    for i in 0..=n {
        let a = (-58.0 + 116.0 * i as f32 / n as f32).to_radians();
        let (x, y) = (c + rad * a.sin(), c - rad * a.cos());
        if i == 0 {
            pb.move_to(x, y);
        } else {
            pb.line_to(x, y);
        }
    }
    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color(ts(look.crescent));
        p.anti_alias = true;
        let st = Stroke {
            width: 1.0,
            line_cap: tiny_skia::LineCap::Round,
            ..Default::default()
        };
        pix.stroke_path(&path, &p, &st, id, None);
    }
    if let Some(v) = look.veil {
        fill(pix, c, r - 1.0, Shader::SolidColor(ts(v)));
    }
}

#[cfg(test)]
mod tests {
    use crate::kit::color::contrast;

    /// The tokens `des-controls` added are Slate's as designed (Paper's driven colours moved
    /// the least to 4.5:1, its meter middle to 3:1 in its white groove), and every new
    /// promise holds in both lights: driven values read as numbers on a card, a bar and a
    /// panel; the buttons' words on their fills.
    #[test]
    fn the_control_tokens_are_the_designs_and_keep_their_promises() {
        let n = crate::theme_slate::slate_night_tokens();
        let hex = |c: egui::Color32| format!("{:02x}{:02x}{:02x}", c.r(), c.g(), c.b());
        assert_eq!(hex(n.auto), "b48df4");
        assert_eq!(hex(n.modulated), "ee86c8");
        assert_eq!(hex(n.destructive), "c8443f");
        assert_eq!(hex(n.primary_ink), "0d1822");
        assert_eq!(
            [hex(n.fcap_hi), hex(n.fcap_mid), hex(n.fcap_lo)],
            ["8a95a1", "66707b", "525b65"]
        );
        assert_eq!(
            [hex(n.cap_hi), hex(n.cap_mid), hex(n.cap_lo)],
            ["4a545e", "353e47", "272e35"]
        );
        assert_eq!(hex(n.disabled), "47515b");
        assert_eq!(n.scrim.a(), 158);
        for t in [n, crate::theme_slate::slate_paper_tokens()] {
            for bg in [t.deck, t.toolbar, t.panel] {
                assert!(contrast(t.auto, bg) >= 4.5, "auto {}", contrast(t.auto, bg));
                assert!(contrast(t.modulated, bg) >= 4.5);
            }
            assert!(contrast(t.primary_ink, t.focus) >= 4.5);
            assert!(contrast(t.light, t.destructive) >= 4.5);
            assert!(contrast(t.meter_mid, t.meter_well) >= 3.0);
            let fails: Vec<_> = t
                .pairings()
                .into_iter()
                .filter(|(_, fg, bg, need)| contrast(*fg, *bg) + 1e-3 < *need)
                .map(|(w, ..)| w)
                .collect();
            assert!(fails.is_empty(), "{fails:?}");
        }
    }
}
