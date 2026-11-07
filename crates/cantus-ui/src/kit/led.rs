//! `Led` — an 11-pt lamp of glass in a socket (round 37, ADR 0103): the socket cut into
//! the panel, its lip catching the light; the lens lit from inside when on (a hot core,
//! the colour, a darker rim) and dark tinted glass when off; the specular on the glass
//! either way; lit, a tight halo (a cached glow), not a haze. `LedButton` — an LED
//! and a caps word in one 20-pt button: every on/off in a device.

use super::brush::{Brush, Fill};
use super::color::{mix, shade, with_alpha};
use super::light::{self, Material, RaisedOpts};
use super::text::{self, Policy, Role, Src, Text};
use super::{focus_ring, State, Style, Tok};

/// The tokens an LED button paints with in a dark theme, beside its section hue (for
/// inspect mode).
pub const BUTTON_TOKENS: &[Tok] = &[
    Tok::DeckTitle,
    Tok::Text,
    Tok::Label,
    Tok::Light,
    Tok::Focus,
    Tok::Bevel,
    Tok::BevelShade,
    Tok::Drop,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
];
/// In a light theme (the button is the card's face).
pub const BUTTON_LIGHT_TOKENS: &[Tok] = &[
    Tok::Deck,
    Tok::Text,
    Tok::Label,
    Tok::Light,
    Tok::Focus,
    Tok::Bevel,
    Tok::BevelShade,
    Tok::Drop,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
];
/// The tokens a lamp paints with, beside its colour and its ground: the light in its
/// lit core, its socket's lip, its lens's edge, the specular on the glass.
pub const TOKENS: &[Tok] = &[Tok::Light, Tok::Lip, Tok::BevelShade, Tok::Specular];
use egui::{Align2, Color32, Pos2, Rect, Vec2};

/// Paint a lamp of `diameter` points at `c`, in the token `color` when `lit`. `ground`
/// is the surface it sits on, which its unlit core is mixed toward.
#[allow(clippy::too_many_arguments)]
pub fn paint(
    b: &mut Brush,
    c: Pos2,
    diameter: f32,
    lit: bool,
    s: &Style,
    color: Tok,
    ground: Tok,
    alpha: f32,
) {
    let area = Rect::from_center_size(c, Vec2::splat(diameter));
    b.register(area, "led", TOKENS, &[Some(color), Some(ground)]);
    lamp(
        b,
        c,
        diameter,
        lit,
        s,
        s.t.get(color),
        s.t.get(ground),
        alpha,
    );
}

#[allow(clippy::too_many_arguments)]
fn lamp(
    b: &mut Brush,
    c: Pos2,
    diameter: f32,
    lit: bool,
    s: &Style,
    color: Color32,
    ground: Color32,
    alpha: f32,
) {
    let c = Pos2::new(b.snap_mid(c.x), b.snap_mid(c.y));
    let r = diameter * 0.5;
    let px = b.px();
    let m = Material::of(&s.t);
    // Lit, a tight halo round the lens (light on the panel, not a haze).
    if lit {
        // A light theme's panel needs more of it to show the lamp is on.
        let (k, reach) = if s.t.dark { (0.6, 1.9) } else { (0.75, 1.6) }; // r37:finish (Paper: a tighter glow, not a smear)
        light::glow(b, c, r * reach, color, k * alpha);
    }
    let lens = (r - (1.2 * s.k).max(px)).max(px);
    let ppp = b.ppp();
    // One cached layer per colour, size and state (round 37): the socket, the lens, its
    // edge and the specular, rasterised once.
    let side = ((2.0 * r * ppp + 3.0).ceil() as usize) | 1;
    let look = super::raster_parts::LampLook {
        lit,
        color,
        ground,
        light: s.t.light,
        lip: m.lip,
        edge: m.bevel_shade,
        specular: m.specular,
    };
    let key = super::cache::key((
        "lamp",
        side,
        (r * ppp).to_bits(),
        (lens * ppp).to_bits(),
        lit,
        [color, ground, s.t.light, m.lip, m.bevel_shade, m.specular].map(super::cache::ckey),
    ));
    let uv = b.layer(key, side, side, |pix| {
        super::raster_parts::lamp(pix, r * ppp, lens * ppp, &look)
    });
    if let Some(uv) = uv {
        let half = side as f32 / ppp * 0.5;
        let rect = Rect::from_min_max(c - Vec2::splat(half), c + Vec2::splat(half));
        b.layer_quad(rect, uv, alpha);
        return;
    }
    lamp_vector(b, c, r, lens, lit, s, color, ground, alpha, &m);
}

/// The lamp as geometry, where the atlas has no room.
#[allow(clippy::too_many_arguments)]
fn lamp_vector(
    b: &mut Brush,
    c: Pos2,
    r: f32,
    lens: f32,
    lit: bool,
    s: &Style,
    color: Color32,
    ground: Color32,
    alpha: f32,
    m: &Material,
) {
    let px = b.px();
    let f = |c: Color32| c.gamma_multiply(alpha);
    // The bezel: a socket cut into the panel — its lower lip catching the light, its
    // hollow darker at the top.
    if m.lip.a() > 0 {
        b.dot(c + Vec2::new(0.0, px), r, f(m.lip));
    }
    b.circle(
        c,
        r,
        Fill::V(f(shade(ground, -0.26)), f(shade(ground, -0.10))),
    );
    // The lens: glass, lit from inside when on (a hot top, the colour, a darker foot), a
    // dark glass tinted by its colour when off. A vertical ramp: the light is above.
    if lit {
        b.circle(
            c,
            lens,
            Fill::V(f(mix(color, s.t.light, 0.62)), f(shade(color, 0.04))),
        );
    } else {
        let off = mix(shade(ground, -0.14), color, 0.24);
        b.circle(c, lens, Fill::V(f(shade(off, -0.06)), f(shade(off, 0.06))));
    }
    // The lens's edge in the socket, and the specular on the glass.
    b.circle_stroke(c, lens, px, f(with_alpha(m.bevel_shade, 0.8)));
    let k = if lit { 0.9 } else { 0.55 };
    b.dot(
        c + Vec2::new(-lens * 0.30, -lens * 0.36),
        (lens * 0.30).max(px * 0.8),
        f(m.specular.gamma_multiply(k)),
    );
}

/// Paint an LED button: a small cap with a lamp at its left and a caps word.
pub fn button(b: &mut Brush, rect: Rect, s: &Style, st: State, label: Src<'_>) {
    use super::skin::{switches, SwitchShape};
    let shape = s.skin.switch;
    let toks = match shape {
        SwitchShape::Plain => {
            if s.t.dark {
                BUTTON_TOKENS
            } else {
                BUTTON_LIGHT_TOKENS
            }
        }
        _ => switches::button_tokens(shape, s.t.dark),
    };
    b.register(rect, "led_button", toks, &[s.hue_tok]);
    let r = s.s(s.t.radius) + s.r(1.0);
    let base = if s.t.dark {
        shade(s.t.deck_title, 0.05)
    } else {
        s.t.deck
    };
    let pressed = if st.active {
        shade(base, -0.03)
    } else if st.hovered {
        shade(base, 0.04)
    } else {
        base
    };
    // The lamp shape lights the whole cap; every other shape is a cap with its switch.
    let lamp_face = if matches!(shape, SwitchShape::Lamp) {
        Some(switches::lamp_face(b, rect, r, s, st, base))
    } else if matches!(shape, SwitchShape::Plain) {
        // An object (round 37, ADR 0103): raised at rest, pushed in while held.
        light::object(
            b,
            rect,
            r,
            pressed,
            &Material::of(&s.t),
            light::ObjectOpts {
                pressed: st.active,
                k: s.k,
                alpha: st.alpha(),
                ..Default::default()
            },
        );
        None
    } else {
        light::raised(
            b,
            rect,
            r,
            pressed,
            RaisedOpts {
                drop: !st.active,
                k: s.k,
                alpha: st.alpha(),
                depth: s.t.depth,
                ..Default::default()
            },
        );
        None
    };
    let w = s.s(switches::slot_w(shape));
    let slot = Rect::from_center_size(
        Pos2::new(rect.left() + s.s(6.0) + w * 0.5, rect.center().y),
        Vec2::new(w, (rect.height() - s.s(6.0)).max(s.s(8.0))),
    );
    match shape {
        SwitchShape::Plain => lamp(b, slot.center(), w, st.lit, s, s.hue, pressed, st.alpha()),
        SwitchShape::Lamp => {}
        SwitchShape::Custom(paint) => paint(b, slot, s, st),
        other => switches::paint(b, slot, s, st, other, s.hue),
    }
    let ink = match lamp_face {
        Some(face) if st.lit => super::color::on_fill(face),
        _ if st.lit => s.t.text,
        _ => s.t.label,
    }
    .gamma_multiply(st.alpha());
    let left = if lamp_face.is_some() {
        rect.left() + s.s(6.0)
    } else {
        slot.right() + s.s(5.0)
    };
    let tr = Rect::from_min_max(
        Pos2::new(left, rect.top()),
        Pos2::new(rect.right() - s.s(6.0), rect.bottom()),
    );
    let t = Text {
        src: label,
        role: Role::Label,
        caps: true,
        // A lamp button's word sits in its middle, as on a drum machine's pad.
        align: if lamp_face.is_some() {
            Align2::CENTER_CENTER
        } else {
            Align2::LEFT_CENTER
        },
        policy: Policy::Abbreviate,
        color: ink,
    };
    let _ = text::paint(b, tr, &t, s.tk());
    if st.focused {
        focus_ring(b, rect, r, s);
    }
}
