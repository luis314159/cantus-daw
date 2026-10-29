//! A unit's switches (round 32, ADR 0074): every [`SwitchShape`] the kit draws — a
//! bat-handle toggle, a rocker, a sliding pill, a lamp button that glows whole. A shape
//! takes the lamp's place in an LED button (the cap and its word stay the kit's), and the
//! card's power lamp; the lamp shape lights the whole button, and the segmented control's
//! chosen segment. A unit's own painter (`SwitchShape::Custom`) is called in the lamp's
//! place too.

use super::super::brush::{Brush, Fill};
use super::super::color::{mix, shade, with_alpha};
use super::super::light;
use super::super::{State, Style, Tok};
use super::SwitchShape;
use egui::{Color32, Pos2, Rect, Vec2};

impl SwitchShape {
    /// Every shape the kit draws (a unit's own painter aside), the kit's first.
    pub const ALL: [SwitchShape; 5] = [
        SwitchShape::Plain,
        SwitchShape::Bat,
        SwitchShape::Rocker,
        SwitchShape::Pill,
        SwitchShape::Lamp,
    ];

    /// The name the gallery and the shots give it.
    pub fn name(self) -> &'static str {
        match self {
            SwitchShape::Plain => "plain",
            SwitchShape::Bat => "bat",
            SwitchShape::Rocker => "rocker",
            SwitchShape::Pill => "pill",
            SwitchShape::Lamp => "lamp",
            SwitchShape::Custom(_) => "custom",
        }
    }
}

/// The width of the switch's place at the left of an LED button, at 100 %.
pub fn slot_w(shape: SwitchShape) -> f32 {
    match shape {
        SwitchShape::Plain | SwitchShape::Lamp => 8.0,
        SwitchShape::Pill => 17.0,
        _ => 11.0,
    }
}

/// The tokens a switch of `shape` paints with on its own (a card's power switch).
pub fn tokens(shape: SwitchShape) -> &'static [Tok] {
    match shape {
        SwitchShape::Pill => &[Tok::Well, Tok::KnobBody, Tok::Light, Tok::Dark],
        SwitchShape::Bat => &[Tok::KnobBody, Tok::Light, Tok::Dark],
        SwitchShape::Rocker => &[Tok::KnobBody, Tok::Dark],
        _ => &[Tok::Light],
    }
}

/// The tokens an LED button of `shape` paints with (for inspect mode), beside its hue.
pub fn button_tokens(shape: SwitchShape, dark: bool) -> &'static [Tok] {
    match (shape, dark) {
        (SwitchShape::Rocker, true) => &[
            Tok::DeckTitle,
            Tok::Text,
            Tok::Label,
            Tok::KnobBody,
            Tok::Dark,
            Tok::Focus,
        ],
        (SwitchShape::Rocker, false) => &[
            Tok::Deck,
            Tok::Text,
            Tok::Label,
            Tok::KnobBody,
            Tok::Dark,
            Tok::Focus,
        ],
        // A lamp's word is a tint of its light when lit, the label when not.
        (SwitchShape::Lamp, true) => &[Tok::DeckTitle, Tok::Label, Tok::Light, Tok::Focus],
        (SwitchShape::Lamp, false) => &[Tok::Deck, Tok::Label, Tok::Light, Tok::Focus],
        (SwitchShape::Bat, true) => &[
            Tok::DeckTitle,
            Tok::Text,
            Tok::Label,
            Tok::KnobBody,
            Tok::Light,
            Tok::Dark,
            Tok::Focus,
        ],
        (SwitchShape::Bat, false) => &[
            Tok::Deck,
            Tok::Text,
            Tok::Label,
            Tok::KnobBody,
            Tok::Light,
            Tok::Dark,
            Tok::Focus,
        ],
        (SwitchShape::Pill, true) => &[
            Tok::DeckTitle,
            Tok::Text,
            Tok::Label,
            Tok::Well,
            Tok::KnobBody,
            Tok::Light,
            Tok::Focus,
        ],
        (SwitchShape::Pill, false) => &[
            Tok::Deck,
            Tok::Text,
            Tok::Label,
            Tok::Well,
            Tok::KnobBody,
            Tok::Light,
            Tok::Focus,
        ],
        (_, true) => &[
            Tok::DeckTitle,
            Tok::Text,
            Tok::Label,
            Tok::Light,
            Tok::Focus,
        ],
        (_, false) => &[Tok::Deck, Tok::Text, Tok::Label, Tok::Light, Tok::Focus],
    }
}

/// Paint a switch of `shape` filling `rect` (the lamp's place), on when `st.lit`, in
/// `hue`. The lamp shape and the kit's are the caller's ([`super::super::led`]).
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, st: State, shape: SwitchShape, hue: Color32) {
    let t = &s.t;
    let a = st.alpha();
    let f = |c: Color32| c.gamma_multiply(a);
    let d = t.depth.clamp(0.0, 1.5);
    let px = b.px();
    let on = st.lit;
    let c = Pos2::new(b.snap_mid(rect.center().x), b.snap_mid(rect.center().y));
    let unit = rect.width().min(rect.height());
    match shape {
        SwitchShape::Plain | SwitchShape::Lamp | SwitchShape::Custom(_) => {}
        SwitchShape::Bat => {
            // A hex nut, and the bat handle leaning up (on) or down (off) from it.
            let metal = mix(t.knob_body, t.light, 0.30);
            let nut = unit * 0.36;
            b.circle(
                c,
                nut,
                Fill::V(f(shade(metal, 0.10 * d)), f(shade(metal, -0.20 * d))),
            );
            b.circle_stroke(c, nut, px, f(with_alpha(t.dark_shade, 0.6)));
            b.circle(c, nut * 0.55, Fill::Solid(f(shade(metal, -0.30))));
            let dir = if on { -1.0 } else { 1.0 };
            let tip = c + Vec2::new(0.0, dir * unit * 0.62);
            let wide = unit * 0.13;
            let shadow = Vec2::new(px, px * 1.5);
            b.segment(
                c + shadow,
                tip + shadow,
                wide * 2.0,
                f(with_alpha(t.dark_shade, 0.35 * d)),
                true,
            );
            b.segment(c, tip, wide * 2.0, f(shade(metal, -0.08)), true);
            b.segment(
                c + Vec2::new(-wide * 0.35, 0.0),
                tip + Vec2::new(-wide * 0.35, 0.0),
                wide * 0.6,
                f(with_alpha(t.light, 0.35 * d.min(1.0))),
                true,
            );
            // The ball at its end, in the unit's hue when on.
            let ball = if on { hue } else { shade(metal, -0.04) };
            b.circle(
                tip,
                wide * 1.35,
                Fill::V(f(shade(ball, 0.12)), f(shade(ball, -0.12))),
            );
        }
        SwitchShape::Rocker => {
            // A rocker, its upper half pressed in when on — with a lit strip there.
            let body = Rect::from_center_size(c, Vec2::new(unit * 0.70, unit * 1.0));
            let body = b.snap_rect(body);
            let r = s.r(2.0);
            b.rrect(body, r, Fill::Solid(f(shade(t.knob_body, -0.25))));
            let inner = body.shrink(px);
            let mid = b.snap(inner.center().y);
            let (up, down) = (
                Rect::from_min_max(inner.min, Pos2::new(inner.right(), mid)),
                Rect::from_min_max(Pos2::new(inner.left(), mid), inner.max),
            );
            let raised = |lit_top: bool| {
                if lit_top {
                    Fill::V(f(shade(t.knob_body, 0.12 * d)), f(shade(t.knob_body, 0.02)))
                } else {
                    Fill::V(
                        f(shade(t.knob_body, -0.10 * d)),
                        f(shade(t.knob_body, -0.02)),
                    )
                }
            };
            b.rrect(up, (r - px).max(0.0), raised(!on));
            b.rrect(down, (r - px).max(0.0), raised(on));
            b.hline(
                inner.left(),
                inner.right(),
                mid,
                px,
                f(with_alpha(t.dark_shade, 0.5)),
            );
            let strip = Rect::from_center_size(
                Pos2::new(up.center().x, up.center().y),
                Vec2::new(up.width() * 0.5, (up.height() * 0.28).max(px)),
            );
            if on {
                light::glow(b, strip.center(), unit * 0.7, hue, 0.6 * a);
                b.rrect(b.snap_rect(strip), px, Fill::Solid(f(hue)));
            } else {
                b.rrect(
                    b.snap_rect(strip),
                    px,
                    Fill::Solid(f(shade(t.knob_body, -0.18))),
                );
            }
        }
        SwitchShape::Pill => {
            // A track, filled in the hue when on, and a knob that slides along it.
            let track = Rect::from_center_size(c, Vec2::new(rect.width(), unit * 0.62));
            let track = b.snap_rect(track);
            let r = track.height() * 0.5;
            if on {
                b.rrect(track, r, Fill::V(f(shade(hue, -0.10)), f(shade(hue, 0.04))));
            } else {
                light::sunken(b, track, r, t.well, a, d);
            }
            let kr = r - px * 1.5;
            let kx = if on {
                track.right() - r
            } else {
                track.left() + r
            };
            let kc = Pos2::new(kx, track.center().y);
            b.circle(
                kc + Vec2::new(0.0, px),
                kr,
                Fill::Solid(f(with_alpha(t.dark_shade, 0.35 * d))),
            );
            b.circle(
                kc,
                kr,
                Fill::V(
                    f(shade(t.knob_body, 0.14 * d)),
                    f(shade(t.knob_body, -0.06 * d)),
                ),
            );
            b.circle_stroke(kc, kr, px, f(with_alpha(t.light, 0.12)));
        }
    }
}

/// The lamp shape's face: the whole cap lit in the hue when on (a drum machine's
/// button), a dark lens of the hue when off. Returns the face colour, for the word on it.
pub fn lamp_face(
    b: &mut Brush,
    rect: Rect,
    r: f32,
    s: &Style,
    st: State,
    base: Color32,
) -> Color32 {
    let t = &s.t;
    let a = st.alpha();
    let face = if st.lit {
        mix(shade(s.hue, 0.04), t.light, 0.10)
    } else {
        mix(base, s.hue, 0.14)
    };
    let face = if st.active {
        shade(face, -0.04)
    } else if st.hovered {
        shade(face, 0.04)
    } else {
        face
    };
    if st.lit {
        // A lamp's light spills round it in a dark room; on a light theme it barely does.
        let (spill, strength) = if t.dark { (0.75, 0.45) } else { (0.6, 0.14) };
        light::glow(
            b,
            rect.center(),
            rect.width().max(rect.height()) * spill,
            s.hue,
            strength * a,
        );
    }
    light::raised(
        b,
        rect,
        r,
        face,
        light::RaisedOpts {
            drop: !st.active,
            k: s.k,
            alpha: a,
            depth: t.depth,
            ..Default::default()
        },
    );
    if st.lit {
        // The light inside the lens, brightest at its middle.
        let inner = b.snap_rect(rect).shrink(2.0 * b.px());
        b.rrect(
            inner,
            (r - 2.0 * b.px()).max(0.0),
            Fill::Stops(&[
                (0.0, with_alpha(t.light, 0.0)),
                (0.5, with_alpha(t.light, 0.18 * a)),
                (1.0, with_alpha(t.light, 0.0)),
            ]),
        );
    }
    face
}
