//! `Well` — the sunken recipe as a part: fields, dropdowns, readouts, number boxes — and
//! `MenuField`, a well holding a value and ▾, with optional ‹ › steppers outside.

use super::brush::Brush;
use super::color::shade;
use super::glyph::{self, Glyph};
use super::light;
use super::text::{self, Elide, Policy, Role, Text};
use super::{focus_ring, State, Style, Tok};

/// The tokens a well paints with (for inspect mode); its hue is the section's.
pub const TOKENS: &[Tok] = &[Tok::Well, Tok::Focus];
/// A readout: a well with a value in it.
pub const READOUT_TOKENS: &[Tok] = &[Tok::Well, Tok::Text, Tok::Focus];
/// A menu field.
pub const FIELD_TOKENS: &[Tok] = &[Tok::Well, Tok::Text, Tok::TextDim, Tok::Focus];
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
    light::sunken(b, rect, r, base(s, st, base_color), st.alpha(), s.t.depth);
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
    let _ = text::paint(b, inner, &t, s.tk());
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
