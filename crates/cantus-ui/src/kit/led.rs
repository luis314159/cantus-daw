//! `Led` — an 11-pt lamp built like FL's: a faint outer ring, a dark ring, a bright rim,
//! the core; lit, it throws a cached glow. Off keeps the geometry. `LedButton` — an LED
//! and a caps word in one 20-pt button: every on/off in a device.

use super::brush::{Brush, Fill};
use super::color::{mix, shade, with_alpha};
use super::light::{self, RaisedOpts};
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
];
/// In a light theme (the button is the card's face).
pub const BUTTON_LIGHT_TOKENS: &[Tok] = &[Tok::Deck, Tok::Text, Tok::Label, Tok::Light, Tok::Focus];
/// The tokens a lamp paints with, beside its colour and its ground.
pub const TOKENS: &[Tok] = &[Tok::Light];
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
    let f = |c: Color32| c.gamma_multiply(alpha);
    // Outer ring: a lens set into the panel.
    b.circle(c, r, Fill::Solid(f(with_alpha(shade(ground, 0.16), 0.55))));
    if lit {
        light::glow(b, c, r * 2.6, color, 0.9 * alpha);
        b.circle(c, r - px, Fill::Solid(f(shade(color, -0.30))));
        b.circle(c, r - 2.0 * px, Fill::Solid(f(shade(color, 0.08))));
        b.circle(
            c,
            (r - 3.0 * px).max(px),
            Fill::V(f(shade(color, 0.12)), f(color)),
        );
        // The specular dot, up and to the left.
        b.circle(
            c + Vec2::new(-r * 0.28, -r * 0.3),
            (r * 0.22).max(px * 0.8),
            Fill::Solid(f(with_alpha(s.t.light, 0.55 * s.t.depth.min(1.0)))),
        );
    } else {
        let off = mix(shade(ground, -0.10), color, 0.18);
        b.circle(c, r - px, Fill::Solid(f(shade(ground, -0.16))));
        b.circle(
            c,
            (r - 2.0 * px).max(px),
            Fill::V(f(shade(off, 0.03)), f(shade(off, -0.03))),
        );
    }
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
