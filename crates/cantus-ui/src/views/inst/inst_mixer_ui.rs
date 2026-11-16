//! The small parts an instrument's own views draw in Slate's hand (round 40 phase 3, the
//! approved `design-p3` sheets): a card, a card's head (its title and a quiet note), a line
//! of words in a box, a button with an optional glyph, a list row, a pill, the mic glyph
//! the browser set has none of. Shared by Tympanum's Kit and Mixer tabs and the instrument
//! mixer (Chorda's too). Colours from the theme's tokens through the mixer's palette
//! ([`crate::views::mixer::look::Pal`]): no literal colour, no stock widget.

use crate::kit::color::mix;
use crate::kit::glyph_browser as gb;
use crate::kit::{with_brush, Brush, Fill, State, Style};
use crate::views::mixer::look::{self, Pal, W};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Response, Sense, Ui};

/// The gutter between cards, at 100 %.
pub const GAP: f32 = 8.0;

pub(crate) fn pal(s: &Style) -> Pal {
    Pal::of(s)
}

/// A card: Slate's raised surface, 6-pt corners, a hairline of light along its top.
pub fn card(b: &mut Brush, s: &Style, r: Rect) {
    let p = Pal::of(s);
    let r = b.snap_rect(r);
    let rad = s.s(6.0);
    b.rrect(r, rad, Fill::Solid(p.edge));
    b.rrect(
        Rect::from_min_max(r.min, pos2(r.right(), r.bottom() - b.px())),
        rad,
        Fill::Solid(p.surface),
    );
}

/// The ground a picture or a list sits on inside a card: the well.
pub fn sunk(b: &mut Brush, s: &Style, r: Rect) {
    let p = Pal::of(s);
    b.rrect(b.snap_rect(r), s.s(4.0), Fill::Solid(p.well));
}

/// Words in `r`, anchored by `align` (left, centre or right; vertically centred), `size`
/// points (at 100 %) in weight `w`. Never cut: what does not fit is elided at its end, and
/// the views lay their boxes out so nothing is.
#[allow(clippy::too_many_arguments)]
pub(crate) fn words(
    b: &mut Brush,
    s: &Style,
    text: &str,
    r: Rect,
    align: Align2,
    size: f32,
    w: W,
    color: Color32,
) -> f32 {
    let x = match align.x() {
        egui::Align::Min => r.left(),
        egui::Align::Center => r.center().x,
        egui::Align::Max => r.right(),
    };
    let base = r.center().y + size * s.k * 0.36;
    look::say(b, s, text, x, base, align, r.width(), size, w, color)
}

/// How wide `text` is set at `size` in `w`, at this style's scale.
pub(crate) fn width(ui: &Ui, s: &Style, text: &str, size: f32, w: W) -> f32 {
    look::width_of(ui.ctx(), s, text, size, w)
}

/// A card's head: the title (SemiBold 13, ink 1) at the left of `r`, a note (Regular 11,
/// ink 3) at its right.
pub fn head(b: &mut Brush, s: &Style, r: Rect, title: &str, note: &str) {
    let p = Pal::of(s);
    let tw = words(
        b,
        s,
        title,
        r,
        Align2::LEFT_CENTER,
        13.0,
        W::SemiBold,
        p.ink1,
    );
    if !note.is_empty() {
        let room = Rect::from_min_max(pos2(r.left() + tw + s.s(8.0), r.top()), r.max);
        self::note(
            b,
            s,
            note,
            room,
            Align2::RIGHT_CENTER,
            11.0,
            W::Regular,
            p.ink3,
        );
    }
}

/// What a button shows at its left.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Lead {
    None,
    Glyph(gb::Glyph),
    /// The microphone this module draws ([`mic_glyph`]).
    Mic,
}

/// A button: Slate's raised key (`raised` over a 1-px edge, a step lighter under the
/// hand), its glyph and label centred. `lit`: the accent's tint, its ink the accent's.
#[allow(clippy::too_many_arguments)]
pub fn button(
    ui: &Ui,
    s: &Style,
    rect: Rect,
    id: egui::Id,
    label: &str,
    lead: Lead,
    lit: bool,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.interact(rect, id, sense);
    let st = State::of(&resp, lit);
    with_brush(ui.painter(), |b| {
        paint_button(b, s, rect, st, label, lead, enabled)
    });
    resp
}

/// [`button`]'s drawing.
pub fn paint_button(
    b: &mut Brush,
    s: &Style,
    rect: Rect,
    st: State,
    label: &str,
    lead: Lead,
    enabled: bool,
) {
    let p = Pal::of(s);
    let r = b.snap_rect(rect);
    let rad = s.s(4.0);
    let face = if st.lit {
        lit_face(s)
    } else if st.hovered && enabled {
        mix(p.raised, p.ink1, 0.07)
    } else {
        p.raised
    };
    b.rrect(r, rad, Fill::Solid(p.edge));
    b.rrect(r.shrink(b.px()), rad - b.px(), Fill::Solid(face));
    let ink = if !enabled {
        p.ink3
    } else if st.lit {
        lit_ink(s)
    } else {
        p.ink1
    };
    let gw = match lead {
        Lead::None => 0.0,
        _ => s.s(14.0),
    };
    let tw = look::width_of(b.ctx(), s, label, 12.0, W::Medium);
    let total = gw + if gw > 0.0 { s.s(6.0) } else { 0.0 } + tw;
    let x0 = (r.center().x - total * 0.5).max(r.left() + s.s(6.0));
    if gw > 0.0 {
        let g = Rect::from_center_size(pos2(x0 + gw * 0.5, r.center().y), vec2(gw, gw));
        match lead {
            Lead::Glyph(gl) => {
                let _ = gb::paint(b, g, gl, ink);
            }
            Lead::Mic => mic_glyph(b, s, g.center(), gw, ink),
            Lead::None => {}
        }
    }
    let tx = x0 + total - tw;
    let room = Rect::from_min_max(pos2(tx, r.top()), pos2(r.right() - s.s(4.0), r.bottom()));
    words(b, s, label, room, Align2::LEFT_CENTER, 12.0, W::Medium, ink);
}

/// A lit button's face: the raised key tinted with the accent.
pub fn lit_face(s: &Style) -> Color32 {
    let p = Pal::of(s);
    mix(p.raised, p.accent, if p.dark { 0.22 } else { 0.16 })
}

/// A lit button's words: the accent toward ink 1 — or ink 1 itself where that mix would
/// read under 4.5:1 on the lit face (round 40 polish: 4.37:1 in Ultraviolet).
pub fn lit_ink(s: &Style) -> Color32 {
    let p = Pal::of(s);
    let ink = mix(p.accent, p.ink1, if p.dark { 0.35 } else { 0.45 });
    if crate::kit::color::contrast(ink, lit_face(s)) < 4.5 {
        p.ink1
    } else {
        ink
    }
}

/// A microphone drawn by hand, `size` across, centred at `c`: a capsule with its grille
/// lines on a short stem and a yoke.
pub fn mic_glyph(b: &mut Brush, s: &Style, c: Pos2, size: f32, ink: Color32) {
    let _ = s;
    let u = size / 14.0;
    let lw = (1.2 * u).max(b.px());
    let cap = Rect::from_center_size(pos2(c.x, c.y - 2.0 * u), vec2(5.0 * u, 8.0 * u));
    b.rrect_stroke(cap, 2.5 * u, lw, ink);
    b.segment(
        pos2(cap.left() + 1.4 * u, cap.center().y),
        pos2(cap.right() - 1.4 * u, cap.center().y),
        lw * 0.8,
        ink,
        false,
    );
    // The yoke: an open cup under the capsule.
    let y = c.y + 1.0 * u;
    b.segment(
        pos2(c.x - 4.0 * u, y - 2.0 * u),
        pos2(c.x - 4.0 * u, y),
        lw,
        ink,
        false,
    );
    b.segment(
        pos2(c.x + 4.0 * u, y - 2.0 * u),
        pos2(c.x + 4.0 * u, y),
        lw,
        ink,
        false,
    );
    b.segment(
        pos2(c.x - 4.0 * u, y),
        pos2(c.x + 4.0 * u, y + 0.01),
        lw,
        ink,
        false,
    );
    b.segment(pos2(c.x, y), pos2(c.x, y + 3.5 * u), lw, ink, false);
    b.segment(
        pos2(c.x - 2.5 * u, y + 3.5 * u),
        pos2(c.x + 2.5 * u, y + 3.5 * u),
        lw,
        ink,
        false,
    );
}

/// A tag over a picture: a raised pill, its name SemiBold 12 and a detail Regular 11 in
/// ink 2; `lit` rings it in the accent (the selected piece's), else a quiet edge. Returns
/// its rect, placed with its left at `at.x` and its middle at `at.y`, kept inside `bounds`.
#[allow(clippy::too_many_arguments)]
pub fn tag(
    b: &mut Brush,
    ui: &Ui,
    s: &Style,
    at: Pos2,
    bounds: Rect,
    name: &str,
    detail: &str,
    lit: bool,
) -> Rect {
    let p = Pal::of(s);
    let pad = s.s(8.0);
    let nw = width(ui, s, name, 12.0, W::SemiBold);
    let dw = if detail.is_empty() {
        0.0
    } else {
        s.s(6.0) + width(ui, s, detail, 11.0, W::Regular)
    };
    let size = vec2(pad * 2.0 + nw + dw, s.s(22.0));
    let mut r = Rect::from_min_size(pos2(at.x, at.y - size.y * 0.5), size);
    let dx = (bounds.right() - s.s(4.0) - r.right()).min(0.0)
        + (bounds.left() + s.s(4.0) - r.left()).max(0.0);
    let dy = (bounds.bottom() - s.s(4.0) - r.bottom()).min(0.0)
        + (bounds.top() + s.s(4.0) - r.top()).max(0.0);
    r = r.translate(vec2(dx, dy));
    let r = b.snap_rect(r);
    let rad = s.s(5.0);
    b.rrect(r, rad, Fill::Solid(if lit { p.accent } else { p.edge }));
    let inner = r.shrink(if lit { s.s(1.5) } else { b.px() });
    b.rrect(
        inner,
        rad - s.s(1.0),
        Fill::Solid(mix(p.raised, p.base, 0.25)),
    );
    let nr = Rect::from_min_max(
        pos2(r.left() + pad, r.top()),
        pos2(r.left() + pad + nw + 1.0, r.bottom()),
    );
    words(
        b,
        s,
        name,
        nr,
        Align2::LEFT_CENTER,
        12.0,
        W::SemiBold,
        p.ink1,
    );
    if dw > 0.0 {
        let dr = Rect::from_min_max(
            pos2(nr.right() + s.s(5.0), r.top()),
            pos2(r.right() - pad + 1.0, r.bottom()),
        );
        words(
            b,
            s,
            detail,
            dr,
            Align2::LEFT_CENTER,
            11.0,
            W::Regular,
            p.ink2,
        );
    }
    r
}

/// A small badge ("Modelled"): a quiet pill, Medium 10.
pub fn badge(b: &mut Brush, ui: &Ui, s: &Style, left: f32, cy: f32, text: &str) -> Rect {
    let p = Pal::of(s);
    let w = width(ui, s, text, 10.0, W::Medium) + s.s(12.0);
    let r = b.snap_rect(Rect::from_min_size(
        pos2(left, cy - s.s(8.0)),
        vec2(w, s.s(16.0)),
    ));
    b.rrect(r, s.s(3.0), Fill::Solid(mix(p.raised, p.ink1, 0.10)));
    words(
        b,
        s,
        text,
        r,
        Align2::CENTER_CENTER,
        10.0,
        W::Medium,
        p.ink2,
    );
    r
}

/// A plate laid over a picture: the base at 88 %, 6-pt corners, a hairline edge — what the
/// tag, the room card and the buttons over the scene sit on.
pub fn veil(b: &mut Brush, s: &Style, r: Rect) {
    let p = Pal::of(s);
    let r = b.snap_rect(r);
    b.rrect(
        r,
        s.s(6.0),
        Fill::Solid(crate::kit::color::with_alpha(p.edge, 0.9)),
    );
    b.rrect(
        r.shrink(b.px()),
        s.s(6.0) - b.px(),
        Fill::Solid(crate::kit::color::with_alpha(p.surface, 0.92)),
    );
}

/// The help line's words while `resp` is hovered (ADR 0109).
pub fn hint(resp: Response, words: impl FnOnce() -> String) -> Response {
    crate::views::hint::said_with(resp, words)
}

/// A short paragraph in `r` from its top (Regular 11 in the label's face), broken at spaces;
/// returns the height it took.
pub fn prose(
    b: &mut Brush,
    ui: &Ui,
    s: &Style,
    r: Rect,
    text: &str,
    color: Color32,
    align: Align2,
) -> f32 {
    use crate::kit::prose;
    let role = crate::kit::text::Role::Label;
    let k = s.tk();
    let lines = prose::wrap(ui.ctx(), text, role, k, r.width());
    let _ = prose::paint(b, r, &lines, role, k, color, align);
    lines.height()
}

/// Words that are only a note: painted when they fit `r` whole, else left out (a note is
/// never cut; what it says is on the help line too). Returns whether they were painted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn note(
    b: &mut Brush,
    s: &Style,
    text: &str,
    r: Rect,
    align: Align2,
    size: f32,
    w: W,
    color: Color32,
) -> bool {
    if look::width_of(b.ctx(), s, text, size, w) > r.width() {
        return false;
    }
    words(b, s, text, r, align, size, w, color);
    true
}
