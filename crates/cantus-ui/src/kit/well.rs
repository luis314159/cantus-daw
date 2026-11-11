//! `Well` — a hollow cut into the surface (round 37, ADR 0103): fields, dropdowns, number
//! boxes; its lower lip catches the light, its upper lip shades its floor. A readout is a
//! recessed display — the same well under the theme's glass. `MenuField`, a well holding a
//! value and ▾, with optional ‹ › steppers outside.

use super::brush::Brush;
use super::color::shade;
use super::glyph::{self, Glyph};
use super::light::{self, Material};
use super::text::{self, Elide, Policy, Role, Text};
use super::{focus_ring, State, Style, Tok};

/// The tokens a well paints with (for inspect mode); its hue is the section's.
pub const TOKENS: &[Tok] = &[Tok::Well, Tok::Focus, Tok::InnerShadow, Tok::Lip];
/// A readout: a well with a value in it, under glass.
pub const READOUT_TOKENS: &[Tok] = &[
    Tok::Well,
    Tok::Text,
    Tok::Focus,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Specular,
];
/// A menu field.
pub const FIELD_TOKENS: &[Tok] = &[
    Tok::Well,
    Tok::Text,
    Tok::TextDim,
    Tok::Focus,
    Tok::InnerShadow,
    Tok::Lip,
];
use egui::{Align2, Color32, Rect, Vec2};

/// The well's fill in `st`.
fn base(s: &Style, st: State, base: Option<Color32>) -> Color32 {
    let c = base.unwrap_or(s.t.well);
    if st.hovered && !st.disabled {
        shade(c, if s.t.dark { 0.025 } else { -0.02 })
    } else {
        c
    }
}

/// Paint a well filling `rect`. `base` overrides the theme's `well`.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, st: State, base_color: Option<Color32>) {
    b.register(rect, "well", TOKENS, &[s.hue_tok]);
    well(b, rect, s, st, base_color);
}

fn well(b: &mut Brush, rect: Rect, s: &Style, st: State, base_color: Option<Color32>) {
    let r = s.s(s.t.radius);
    if s.t.system.recessed() {
        // Slate and its siblings (round 39): the key's own well — a 1-px dark edge, a 1-px
        // light lip, a flat floor; no inner shade, no gloss. Typed into (`des-controls`'
        // fields): the well goes to its edge (Paper: white) inside a 2-pt ring of the accent
        // at 55 %, 2 pt out.
        if st.lit {
            let fill = if s.t.dark {
                super::color::opaque(s.t.inner_shadow)
            } else {
                s.t.light
            };
            super::cap::well_body(b, rect, r, s, fill, st.alpha());
            let ring = s.s(2.0);
            b.rrect_stroke(
                b.snap_rect(rect).expand(s.s(2.0) + ring * 0.5),
                r + s.s(2.0) + ring * 0.5,
                ring,
                super::color::with_alpha(s.t.focus, 0.55),
            );
        } else {
            super::cap::well_body(b, rect, r, s, base(s, st, base_color), st.alpha());
        }
        if st.focused && !st.lit {
            focus_ring(b, rect, r, s);
        }
        return;
    } else {
        let m = Material::of(&s.t);
        light::recess(b, rect, r, base(s, st, base_color), &m, st.alpha(), s.k);
    }
    if st.lit {
        // An open field, or one being typed into: its outline in the section's hue.
        b.rrect_stroke(b.snap_rect(rect).shrink(b.px() * 0.5), r, b.px(), s.hue);
    }
    if st.focused {
        focus_ring(b, rect, r, s);
    }
}

/// A well holding a value: numbers right-aligned in mono, names left in the body face.
pub fn readout(b: &mut Brush, rect: Rect, s: &Style, st: State, value: &str, numeric: bool) {
    b.register(rect, "readout", READOUT_TOKENS, &[s.hue_tok]);
    well(b, rect, s, st, None);
    if !s.t.system.recessed() {
        // Glass over a readout is the raised family's (round 37); Slate's wells are flat.
        let inner = b.snap_rect(rect).shrink(b.px());
        let r = (s.s(s.t.radius) - b.px()).max(0.0);
        light::glass(b, inner, r, &Material::of(&s.t), st.alpha());
    }
    let pad = s.s(6.0);
    let inner = rect.shrink2(Vec2::new(pad, 0.0));
    let (role, align) = if numeric {
        (Role::Value, Align2::RIGHT_CENTER)
    } else {
        (Role::Body, Align2::LEFT_CENTER)
    };
    let color = if st.active { s.hue } else { s.t.text }.gamma_multiply(st.alpha());
    let t = Text::new(value, role, color)
        .align(align)
        .policy(Policy::Elide(if numeric {
            Elide::End
        } else {
            Elide::Middle
        }));
    let _ = text::paint_face(b, inner, &t, s.tk(), s.skin.lettering.value());
}

/// Where a menu field's parts landed, for the widget to sense.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FieldRects {
    pub field: Rect,
    pub prev: Option<Rect>,
    pub next: Option<Rect>,
}

/// The layout of a menu field in `rect`: steppers outside the well when asked for.
pub fn field_rects(rect: Rect, s: &Style, steppers: bool) -> FieldRects {
    if !steppers {
        return FieldRects {
            field: rect,
            prev: None,
            next: None,
        };
    }
    let w = s.s(14.0);
    FieldRects {
        field: Rect::from_min_max(rect.min + Vec2::new(w, 0.0), rect.max - Vec2::new(w, 0.0)),
        prev: Some(Rect::from_min_size(rect.min, Vec2::new(w, rect.height()))),
        next: Some(Rect::from_min_size(
            egui::pos2(rect.max.x - w, rect.min.y),
            Vec2::new(w, rect.height()),
        )),
    }
}

/// Paint a menu field: a well with `value` and ▾ (lit = open), ‹ › outside when
/// `steppers`.
pub fn menu_field(b: &mut Brush, rect: Rect, s: &Style, st: State, value: &str, steppers: bool) {
    if s.t.system.recessed() {
        menu_field_slate(b, rect, s, st, value, steppers);
        return;
    }
    b.register(rect, "menu_field", FIELD_TOKENS, &[s.hue_tok]);
    let r = field_rects(rect, s, steppers);
    well(b, r.field, s, st, None);
    let a = st.alpha();
    let ink = s.t.text.gamma_multiply(a);
    let dim = s.t.text_dim.gamma_multiply(a);
    let chev = s.s(10.0);
    let arrow = Rect::from_center_size(
        egui::pos2(r.field.right() - s.s(6.0) - chev * 0.5, r.field.center().y),
        Vec2::splat(chev),
    );
    glyph::paint(b, arrow, Glyph::Down, if st.hovered { ink } else { dim });
    let inner = Rect::from_min_max(
        egui::pos2(r.field.left() + s.s(7.0), r.field.top()),
        egui::pos2(arrow.left() - s.s(3.0), r.field.bottom()),
    );
    let t = Text::new(value, Role::Control, ink).align(Align2::LEFT_CENTER);
    let _ = text::paint(b, inner, &t, s.tk());
    for (side, g) in [(r.prev, Glyph::Prev), (r.next, Glyph::Next)] {
        if let Some(sr) = side {
            glyph::paint(b, sr.shrink(s.s(2.0)), g, dim);
        }
    }
}

/// A chevron `w` wide and `h` tall centred on `c`, pointing down (or up), `sw` thick.
pub fn chevron(b: &mut Brush, c: egui::Pos2, w: f32, h: f32, up: bool, sw: f32, ink: Color32) {
    let (dy, x) = (if up { -h * 0.5 } else { h * 0.5 }, w * 0.5);
    b.polyline(
        &[
            egui::pos2(c.x - x, c.y - dy),
            egui::pos2(c.x, c.y + dy),
            egui::pos2(c.x + x, c.y - dy),
        ],
        sw,
        ink,
    );
}

/// A drop-down in a recessed family (round 39, `des-controls`' fields sheet): the value at
/// the left in Regular 12, a chevron 7 × 4 in an 18-pt zone at the right — ink 2, ink 1
/// under the hand and open; open, it points up and the field wears the focus ring.
fn menu_field_slate(b: &mut Brush, rect: Rect, s: &Style, st: State, value: &str, steppers: bool) {
    b.register(rect, "menu_field", FIELD_TOKENS, &[]);
    let r = field_rects(rect, s, steppers);
    let rr = s.s(s.t.radius);
    let a = st.alpha();
    let fill = if st.hovered && !st.disabled {
        super::cap::well_fill(
            s,
            State {
                hovered: true,
                ..State::REST
            },
        )
    } else {
        s.t.well
    };
    super::cap::well_body(b, r.field, rr, s, fill, a);
    let (ink, quiet) = if st.disabled {
        (s.t.disabled, s.t.disabled)
    } else {
        (
            s.t.text,
            if st.hovered || st.lit {
                s.t.text
            } else {
                s.t.text_dim
            },
        )
    };
    let zone = s.s(18.0);
    let cc = egui::pos2(r.field.right() - zone * 0.5, r.field.center().y);
    chevron(b, cc, s.s(7.0), s.s(4.0), st.lit, s.s(1.25), quiet);
    let inner = Rect::from_min_max(
        egui::pos2(r.field.left() + s.s(8.0), r.field.top()),
        egui::pos2(r.field.right() - zone, r.field.bottom()),
    );
    let t = Text::new(value, Role::Body, ink).align(Align2::LEFT_CENTER);
    let _ = text::paint(b, inner, &t, s.tk());
    for (side, up) in [(r.prev, false), (r.next, true)] {
        if let Some(sr) = side {
            // The steppers: a chevron each way, outside the well.
            let c = sr.center();
            let (x, h) = (s.s(2.0), s.s(3.5));
            let pts = if up {
                [
                    egui::pos2(c.x - x, c.y - h),
                    egui::pos2(c.x + x, c.y),
                    egui::pos2(c.x - x, c.y + h),
                ]
            } else {
                [
                    egui::pos2(c.x + x, c.y - h),
                    egui::pos2(c.x - x, c.y),
                    egui::pos2(c.x + x, c.y + h),
                ]
            };
            b.polyline(&pts, s.s(1.25), quiet);
        }
    }
    if st.lit || st.focused {
        focus_ring(b, r.field, rr, s);
    }
}
