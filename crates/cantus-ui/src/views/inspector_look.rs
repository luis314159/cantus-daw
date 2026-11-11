//! r39:lo-mixer — the inspector's rows as des-mixer's `inspector-*` sheets draw them, so
//! every section reads in one hand (round 39's close-out; the head, the section titles, the
//! insert rows, the EQ and the sends were el-mixer's first pass):
//!
//! - a **field** (Output, Input, Key into, From, the VCA, the recording input): 24 high,
//!   r 4, the well with a 1-px edge; a glyph, the value Regular 12 ink 1, its detail
//!   Regular 10 ink 3, a chevron;
//! - a **value** (Volume, Pan, Width, a parameter, a mix): no well — a 3-px bar with its
//!   handle (from the middle for a bipolar value) and the value Regular 11 ink 1 at the
//!   right;
//! - a **switch** (Safe from snapshots, Hear the key, Low latency): 26 × 14, a pill;
//! - a **text** (a figure, a name): no well — Regular 12 ink 1, numbers Regular 11 at the
//!   right.
//!
//! Painting only; the rows' sensing and acting stay where they were (`inspector_sheet`).

use crate::kit::glyph::{self, Glyph};
use crate::kit::glyph_mixer::{Ink, MGlyph};
use crate::kit::{Brush, Fill, State, Style};
use crate::views::mixer::look::{self, Pal, W};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// What leads a field's value.
#[derive(Copy, Clone)]
pub enum Lead {
    None,
    /// A glyph of the kit's family.
    Glyph(Glyph),
    /// A glyph of set 3 (the mixer's).
    Mixer(MGlyph),
    /// A 6-pt square of a strip's colour (the key's source).
    Swatch(Color32),
}

/// The bar of a value row: at most 96 wide, the rest the value's.
pub const BAR: f32 = 96.0;

/// Paint a field in `rect`: the well (hovered a step lighter, open in the accent's ring),
/// `lead`, `value`, `detail` after it, and a chevron when it opens a list.
#[allow(clippy::too_many_arguments)]
pub fn field(
    b: &mut Brush,
    s: &Style,
    rect: Rect,
    st: State,
    lead: Lead,
    value: &str,
    detail: &str,
    chevron: bool,
) {
    let p = Pal::of(s);
    let a = st.alpha();
    let r = b.snap_rect(rect);
    let px = b.px();
    let ground = if st.hovered || st.lit {
        p.well_hi
    } else {
        p.well
    };
    let edge = if st.lit {
        p.accent
    } else {
        crate::kit::color::mix(p.edge, p.line, 0.5)
    };
    b.rrect(r, 4.0, Fill::Solid(edge.gamma_multiply(a)));
    b.rrect(r.shrink(px), 3.0, Fill::Solid(ground.gamma_multiply(a)));
    let cy = r.center().y;
    let base = cy + 4.0;
    let mut x = r.left() + 8.0;
    let ink = |c: Color32| c.gamma_multiply(a);
    match lead {
        Lead::None => {}
        Lead::Glyph(g) => {
            let gr = Rect::from_center_size(pos2(x + 6.0, cy), vec2(12.0, 12.0));
            glyph::paint(b, gr, g, ink(p.ink2));
            x += 18.0;
        }
        Lead::Mixer(g) => {
            look::glyph(
                b,
                s,
                pos2(x + 6.0, cy),
                12.0,
                g,
                Ink::Solid(ink(p.ink2)),
                ground,
            );
            x += 18.0;
        }
        Lead::Swatch(c) => {
            let sq = Rect::from_center_size(pos2(x + 3.0, cy), vec2(6.0, 6.0));
            b.rrect(b.snap_rect(sq), 1.0, Fill::Solid(ink(c)));
            x += 12.0;
        }
    }
    let right = if chevron {
        let c = pos2(r.right() - 10.0, cy);
        crate::kit::well::chevron(
            b,
            c,
            7.0,
            4.0,
            st.lit,
            1.2,
            ink(if st.hovered { p.ink1 } else { p.ink3 }),
        );
        r.right() - 20.0
    } else {
        r.right() - 8.0
    };
    let room = (right - x).max(4.0);
    let vw = look::say(
        b,
        s,
        value,
        x,
        base,
        Align2::LEFT_BOTTOM,
        room,
        12.0,
        W::Regular,
        ink(if st.disabled { p.ink3 } else { p.ink1 }),
    );
    let dx = x + vw + 5.0;
    if !detail.is_empty() && dx < right - 12.0 {
        look::say(
            b,
            s,
            detail,
            dx,
            base,
            Align2::LEFT_BOTTOM,
            right - dx,
            10.0,
            W::Regular,
            ink(p.ink3),
        );
    }
}

/// Where a value row's bar lies in its value column `rect`.
pub fn bar_rect(rect: Rect) -> Rect {
    let w = BAR.min((rect.width() - 56.0).max(24.0));
    Rect::from_min_size(pos2(rect.left(), rect.center().y - 1.5), vec2(w, 3.0))
}

/// Paint a value: the bar at `pos` (0…1; from the middle when `bipolar`, a tick there),
/// its handle, `text` at the right.
pub fn value(b: &mut Brush, s: &Style, rect: Rect, st: State, pos: f32, bipolar: bool, text: &str) {
    let p = Pal::of(s);
    let a = st.alpha();
    let bar = bar_rect(rect);
    let pos = pos.clamp(0.0, 1.0);
    let hx = bar.left() + bar.width() * pos;
    let cy = bar.center().y;
    b.rrect(
        b.snap_rect(bar),
        1.5,
        Fill::Solid(p.bar_track.gamma_multiply(a)),
    );
    let lit = if st.disabled {
        p.ink4
    } else if st.hovered || st.active {
        p.ink1
    } else {
        p.ink2
    };
    let from = if bipolar {
        bar.left() + bar.width() * 0.5
    } else {
        bar.left()
    };
    let (x0, x1) = (from.min(hx), from.max(hx));
    if x1 - x0 > 0.5 {
        b.rrect(
            b.snap_rect(Rect::from_min_max(
                pos2(x0, bar.top()),
                pos2(x1, bar.bottom()),
            )),
            1.5,
            Fill::Solid(lit.gamma_multiply(a)),
        );
    }
    if bipolar {
        // The centre's tick.
        b.vline(
            b.snap(from) + b.px() * 0.5,
            cy - 4.0,
            cy + 4.0,
            b.px(),
            p.ink4.gamma_multiply(a),
        );
    }
    handle(b, &p, pos2(hx, cy), st.hovered || st.active, a);
    look::say(
        b,
        s,
        text,
        rect.right(),
        cy + 4.0,
        Align2::RIGHT_BOTTOM,
        (rect.right() - bar.right() - 8.0).max(8.0),
        11.0,
        W::Regular,
        (if st.disabled { p.ink3 } else { p.ink1 }).gamma_multiply(a),
    );
}

/// A bar's handle: r 4, `raised` with a 1-px edge (a step lighter under the hand).
pub fn handle(b: &mut Brush, p: &Pal, c: Pos2, hot: bool, a: f32) {
    let face = if hot {
        crate::kit::color::mix(p.raised, p.ink1, 0.12)
    } else {
        p.raised
    };
    b.circle(c, 4.0, Fill::Solid(face.gamma_multiply(a)));
    b.circle_stroke(c, 4.0, b.px(), p.edge.gamma_multiply(a));
}

/// Paint a switch 26 × 14 whose right edge is at `right`, centred on `cy`; returns its
/// rect. On: the accent's pill, the knob at the right; off: the well, the knob at the left
/// in ink 3.
pub fn switch(b: &mut Brush, s: &Style, right: f32, cy: f32, on: bool, st: State) -> Rect {
    let p = Pal::of(s);
    let a = st.alpha();
    let r = b.snap_rect(Rect::from_min_size(
        pos2(right - 26.0, cy - 7.0),
        vec2(26.0, 14.0),
    ));
    let px = b.px();
    if on {
        b.rrect(r, 7.0, Fill::Solid(p.accent.gamma_multiply(a)));
        b.circle(
            pos2(r.right() - 7.0, r.center().y),
            5.0,
            Fill::Solid(p.on_ink.gamma_multiply(a)),
        );
    } else {
        b.rrect(r, 7.0, Fill::Solid(p.edge.gamma_multiply(a)));
        b.rrect(
            r.shrink(px),
            7.0 - px,
            Fill::Solid((if st.hovered { p.well_hi } else { p.well }).gamma_multiply(a)),
        );
        b.circle(
            pos2(r.left() + 7.0, r.center().y),
            4.5,
            Fill::Solid((if st.hovered { p.ink2 } else { p.ink3 }).gamma_multiply(a)),
        );
    }
    r
}

/// Paint a plain text value in `rect`: a name at the left in Regular 12, a figure at the
/// right in Regular 11; ink 1 (ink 3 when it does not apply).
pub fn text(b: &mut Brush, s: &Style, rect: Rect, value: &str, numeric: bool, dim: bool) {
    let p = Pal::of(s);
    let ink = if dim { p.ink3 } else { p.ink1 };
    let base = rect.center().y + 4.0;
    if numeric {
        look::say(
            b,
            s,
            value,
            rect.right(),
            base,
            Align2::RIGHT_BOTTOM,
            rect.width(),
            11.0,
            W::Regular,
            ink,
        );
    } else {
        look::say(
            b,
            s,
            value,
            rect.left(),
            base,
            Align2::LEFT_BOTTOM,
            rect.width(),
            12.0,
            W::Regular,
            ink,
        );
    }
}

/// How wide a [`key`] holding `word` is (`lead`: with its lamp or glyph).
pub fn key_width(ctx: &egui::Context, s: &Style, word: &str, lead: bool) -> f32 {
    (look::width_of(ctx, s, word, 11.0, W::Medium) + if lead { 40.0 } else { 26.0 }).ceil()
}

/// des-mixer's key (*Freeze strip*, a row's switch): the well (r 4, a step lighter under the
/// hand), a lamp (`Some(on)`: the accent's dot with its halo, or a 1-px ring in ink 4) or a
/// glyph of set 3, then the word in Medium 11 (ink 1 when on, ink 2 off).
pub fn key(
    b: &mut Brush,
    s: &Style,
    rect: Rect,
    st: State,
    lamp: Option<bool>,
    glyph: Option<MGlyph>,
    word: &str,
) {
    let p = Pal::of(s);
    let a = st.alpha();
    let r = b.snap_rect(rect);
    let px = b.px();
    let ground = if st.hovered || st.active {
        p.well_hi
    } else {
        p.well
    };
    b.rrect(
        r,
        4.0,
        Fill::Solid(crate::kit::color::mix(p.edge, p.line, 0.5).gamma_multiply(a)),
    );
    b.rrect(r.shrink(px), 3.0, Fill::Solid(ground.gamma_multiply(a)));
    let cy = r.center().y;
    let mut x = r.left() + 10.0;
    let on = lamp.unwrap_or(st.lit);
    if let Some(lit) = lamp {
        let c = pos2(x + 2.0, cy);
        if lit {
            b.circle(
                c,
                4.5,
                Fill::Solid(crate::kit::color::with_alpha(p.accent, 0.2 * a)),
            );
            b.circle(c, 3.0, Fill::Solid(p.accent.gamma_multiply(a)));
        } else {
            b.circle_stroke(c, 2.5, px, p.ink4.gamma_multiply(a));
        }
        x += 12.0;
    } else if let Some(g) = glyph {
        let ink = if on {
            Ink::Solid(p.accent.gamma_multiply(a))
        } else {
            Ink::Solid(p.ink2.gamma_multiply(a))
        };
        look::glyph(b, s, pos2(x + 5.0, cy), 14.0, g, ink, ground);
        x += 16.0;
    }
    let ink = if st.disabled {
        p.ink4
    } else if on || st.hovered {
        p.ink1
    } else {
        p.ink2
    };
    look::say(
        b,
        s,
        word,
        x,
        cy + 4.0,
        Align2::LEFT_BOTTOM,
        (r.right() - 6.0 - x).max(4.0),
        11.0,
        W::Medium,
        ink.gamma_multiply(a),
    );
}
