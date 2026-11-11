//! How the top bar paints (round 39, `dir-a`'s phase B sheets): recessed wells 24 high on
//! the bar's centre line, keys joined in one well and divided by its own dark edge, the
//! state of a key as a tint inside its well, and words at the sheet's exact sizes.
//!
//! The inks: ink 1 the theme's text, ink 2 its secondary text, ink 3 its placeholder held at
//! 4.5:1 on the bar and in a well, ink 4 (unavailable, idle marks) a quarter of the way from
//! the bar to ink 2. The accent is the highlight; play is the LED green, record the danger
//! red, busy the meter's amber.

use crate::kit::color::{ensure_contrast_all, mix, opaque};
use crate::kit::glyph::{self, Glyph};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{cap, icons, Brush, Fill, State, Style};
use egui::{Align2, Color32, Pos2, Rect, Vec2};

/// The bar's height at 100 %.
pub const BAR: f32 = 44.0;
/// A key's height at 100 %, and the corner of its well.
pub const KEY_H: f32 = 24.0;
pub const RADIUS: f32 = 4.0;

/// Ink 1: what is read first.
pub fn ink1(s: &Style) -> Color32 {
    s.t.text
}

/// Ink 2: menus, names, values at rest.
pub fn ink2(s: &Style) -> Color32 {
    s.t.text_dim
}

/// Ink 3: units, ticks, captions — 4.5:1 on the bar and in its wells.
pub fn ink3(s: &Style) -> Color32 {
    ensure_contrast_all(s.t.placeholder, &[s.t.toolbar, s.t.well], 4.5)
}

/// Ink 4: an unavailable glyph, an idle mark — never words someone must read.
pub fn ink4(s: &Style) -> Color32 {
    mix(s.t.toolbar, s.t.text_dim, 0.28)
}

/// The amber of a busy reading.
pub fn busy(s: &Style) -> Color32 {
    s.t.meter_high
}

/// The key row in `r`: 24 high on its centre line, as wide as `r`.
pub fn row(r: Rect, s: &Style) -> Rect {
    Rect::from_center_size(r.center(), Vec2::new(r.width(), s.s(KEY_H).min(r.height())))
}

/// A slice of the key row `row`: `w` wide from `x` (points from the row's left).
pub fn cell(row: Rect, x: f32, w: f32) -> Rect {
    Rect::from_min_size(
        Pos2::new(row.left() + x, row.top()),
        Vec2::new(w, row.height()),
    )
}

/// A well filling `r`: its floor `fill` (the theme's well when `None`).
pub fn well(b: &mut Brush, r: Rect, s: &Style, fill: Option<Color32>) {
    cap::well_body(b, r, s.r(RADIUS), s, fill.unwrap_or(s.t.well), 1.0);
}

/// Keys joined in one well, `cells` of it divided by the well's dark edge inset 4.
pub fn joined(b: &mut Brush, cells: &[Rect], s: &Style) {
    let (Some(first), Some(last)) = (cells.first(), cells.last()) else {
        return;
    };
    let all = Rect::from_min_max(first.min, last.max);
    well(b, all, s, None);
    let edge = opaque(s.t.inner_shadow);
    for c in &cells[1..] {
        b.vline(
            b.snap_mid(c.left() - b.px() * 0.5),
            c.top() + s.s(4.0),
            c.bottom() - s.s(4.0),
            b.px(),
            edge,
        );
    }
}

/// A key's state inside its well: hover lifts its floor, pressed darkens it to the edge,
/// on tints it with `hue` by `amount`; inset 2, corner 3.
pub fn tint(b: &mut Brush, c: Rect, s: &Style, st: State, hue: Color32, amount: f32) {
    let inner = c.shrink(s.s(2.0));
    let fill = if st.active {
        opaque(s.t.inner_shadow)
    } else if st.lit {
        mix(s.t.well, hue, amount)
    } else if st.hovered {
        mix(s.t.well, s.t.text, if s.t.dark { 0.05 } else { 0.06 })
    } else {
        return;
    };
    b.rrect(b.snap_rect(inner), s.r(3.0), Fill::Solid(fill));
}

/// A glyph in key `c` from the icon family, drawn for the key's height (`box_h` the
/// drawing's box, 24 for the transport, 20 for a window's key), in the inks of `st`; the
/// kit's own glyph where the family has none.
pub fn icon(b: &mut Brush, c: Rect, s: &Style, g: Glyph, st: State, on: Color32, box_h: f32) {
    let fill = if st.lit {
        mix(s.t.well, on, 0.2)
    } else {
        s.t.well
    };
    let inks = icons::inks(s, st, on, fill);
    let r = Rect::from_center_size(c.center(), Vec2::splat(s.s(box_h)));
    if icons::paint(b, r, g, &inks, st.alpha()) {
        return;
    }
    let side = s.s(box_h) * 0.5;
    let gr = Rect::from_center_size(c.center(), Vec2::splat(side));
    glyph::paint(b, gr, g, inks.main.gamma_multiply(st.alpha()));
}

/// A small chevron ▾ centred at `c`, `half` wide each side.
pub fn chevron(b: &mut Brush, c: Pos2, half: f32, ink: Color32, s: &Style) {
    let h = half * 0.55;
    let pts = [
        Pos2::new(c.x - half, c.y - h),
        Pos2::new(c.x, c.y + h),
        Pos2::new(c.x + half, c.y - h),
    ];
    b.polyline(&pts, s.s(1.2), ink);
}

/// The scale a role is set at to read `px` points at 100 % (the sheet's sizes from the
/// kit's roles: 15 from the body's 12, 10 from the control's…).
pub fn at_px(s: &Style, role: Role, px: f32) -> f32 {
    s.tk() * px / role.size().max(1.0)
}

/// Words `w` in `r`, `px` points in `role`'s face, aligned, never past `r`.
#[allow(clippy::too_many_arguments)]
pub fn words(
    b: &mut Brush,
    r: Rect,
    w: &str,
    role: Role,
    px: f32,
    ink: Color32,
    align: Align2,
    s: &Style,
) {
    let _ = text::paint(
        b,
        r,
        &Text::new(w, role, ink)
            .align(align)
            .policy(Policy::Elide(Elide::End)),
        at_px(s, role, px),
    );
}

/// [`words`] from an i18n key.
#[allow(clippy::too_many_arguments)]
pub fn words_key(
    b: &mut Brush,
    r: Rect,
    key: &str,
    role: Role,
    px: f32,
    ink: Color32,
    align: Align2,
    s: &Style,
) {
    let _ = text::paint(
        b,
        r,
        &Text::key(key, role, ink)
            .align(align)
            .policy(Policy::Elide(Elide::End)),
        at_px(s, role, px),
    );
}

/// The width of `w` set `px` points in `role`'s face.
pub fn measure(ctx: &egui::Context, w: &str, role: Role, px: f32, s: &Style) -> f32 {
    text::measure(ctx, &Text::new(w, role, s.t.text), at_px(s, role, px)).x
}

/// [`measure`] of an i18n key.
pub fn measure_key(ctx: &egui::Context, key: &str, role: Role, px: f32, s: &Style) -> f32 {
    text::measure(ctx, &Text::key(key, role, s.t.text), at_px(s, role, px)).x
}

/// A count in a small pill (the dropouts, the count-in's beats, More's folded groups):
/// `fill` behind, `ink` the number.
pub fn pill(b: &mut Brush, r: Rect, n: &str, fill: Color32, ink: Color32, s: &Style) {
    b.rrect(b.snap_rect(r), r.height() * 0.5, Fill::Solid(fill));
    words(b, r, n, Role::Micro, 8.5, ink, Align2::CENTER_CENTER, s);
}
