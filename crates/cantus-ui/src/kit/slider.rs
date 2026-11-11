//! Sliders and value bars (round 39, `des-controls`' fader sheet, owner-approved).
//!
//! - **Slider**: the fader on its side — a 3-pt groove in the well's edge, the fill is the
//!   value in the knob arc's colours (from the centre when bipolar, with a 1 × 9 centre
//!   mark), a thumb 9 × 15 drawn like a fader cap with an upright index; a range has two.
//! - **Value bar**, the dense form for inspector rows and editors: a well 20 pt high whose
//!   fill is the value (`well→ink3 30 %`; held, the accent at 30 %; automated, the clip's
//!   colour at 30 %), the value's edge a 1-pt line, the name at the left (Regular 11) and
//!   the number at the right (Regular 12, its unit Regular 10 in ink 3); modulated, a 3-pt
//!   band of the modulator's colour along its foot and a line at the live value.

use super::automate::Drivers;
use super::brush::{Brush, Fill};
use super::color::{mix, opaque, with_alpha};
use super::drive_sign::{self, Painted};
use super::text::{self, Policy, Role, Src, Text};
use super::{focus_ring, knob_slate, State, Style, Tok};
use egui::{Align2, Pos2, Rect, Vec2};

/// The tokens a slider and a value bar paint with (for inspect mode).
pub const TOKENS: &[Tok] = &[
    Tok::Well,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Text,
    Tok::TextDim,
    Tok::Placeholder,
    Tok::Focus,
    Tok::Auto,
    Tok::Modulated,
    Tok::FaderCapHi,
    Tok::FaderCapMid,
    Tok::FaderCapLo,
];

/// What a slider shows: its value (0…1), a second thumb for a range, and whether it is
/// bipolar (its fill from the centre).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SliderLook {
    pub value: f32,
    pub upper: Option<f32>,
    pub bipolar: bool,
}

/// The thumb's size at 100 %.
pub const THUMB: Vec2 = Vec2::new(9.0, 15.0);

/// The thumb's rect for `v` (0…1) in a slider occupying `rect`.
pub fn thumb_rect(rect: Rect, s: &Style, v: f32) -> Rect {
    let w = THUMB.x * s.k;
    let x = rect.left() + w * 0.5 + (rect.width() - w) * v.clamp(0.0, 1.0);
    Rect::from_center_size(Pos2::new(x, rect.center().y), THUMB * s.k)
}

/// Paint a slider in `rect`, driven by `d`.
pub fn slider(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &SliderLook, d: Drivers) {
    b.register(rect, "slider", TOKENS, &[]);
    let t = &s.t;
    let a = if st.disabled { 0.55 } else { 1.0 };
    let px = b.px();
    let gh = (3.0 * s.k / px).round().max(1.0) * px;
    let cy = b.snap(rect.center().y - gh * 0.5);
    let x0 = rect.left() + THUMB.x * s.k * 0.5;
    let x1 = rect.right() - THUMB.x * s.k * 0.5;
    let groove = Rect::from_min_max(Pos2::new(x0, cy), Pos2::new(x1, cy + gh));
    b.rrect(
        groove,
        gh * 0.5,
        Fill::Solid(opaque(t.inner_shadow).gamma_multiply(a)),
    );
    let at = |v: f32| x0 + (x1 - x0) * v.clamp(0.0, 1.0);
    let (from, to) = match look.upper {
        Some(u) => (look.value, u),
        None if look.bipolar => (0.5, look.value),
        None => (0.0, look.value),
    };
    let fill = knob_slate::arc_color(s, st, d);
    let (l, r) = (at(from.min(to)), at(from.max(to)));
    if r - l > px {
        b.rrect(
            Rect::from_min_max(Pos2::new(l, cy), Pos2::new(r, cy + gh)),
            gh * 0.5,
            Fill::Solid(fill.gamma_multiply(a)),
        );
    }
    if look.bipolar {
        let mark = Rect::from_center_size(
            Pos2::new(b.snap_mid(at(0.5)), rect.center().y),
            Vec2::new(px.max(s.k), 9.0 * s.k),
        );
        b.rect(mark, Fill::Solid(t.placeholder.gamma_multiply(a)));
    }
    let thumbs = [Some(look.value), look.upper];
    for v in thumbs.into_iter().flatten() {
        let th = b.snap_rect(thumb_rect(rect, s, v));
        super::fader_slate::paint_cap(b, th, s, st, d, false);
        let ink = if st.active {
            t.focus
        } else if d.clip {
            t.auto
        } else {
            t.text
        };
        let iw = (s.k / px).round().max(1.0) * px;
        let x = b.snap(th.center().x - iw * 0.5);
        b.rect(
            Rect::from_min_max(
                Pos2::new(x, th.top() + 3.0 * s.k),
                Pos2::new(x + iw, th.bottom() - 3.0 * s.k),
            ),
            Fill::Solid(ink.gamma_multiply(a)),
        );
    }
    if st.focused {
        focus_ring(b, rect, s.s(3.0), s);
    }
    if !d.none() {
        let h = s.s(6.0);
        drive_sign::row(
            b,
            Pos2::new(rect.left() - s.s(4.0) - h * 0.9, rect.center().y),
            h,
            d,
            s,
            None,
        );
    }
    super::automate::note_painted(Painted::Value {
        x: rect.left(),
        y: rect.center().y,
        h: s.s(6.0),
        drew: d,
    });
}

/// What a value bar shows.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BarLook<'a> {
    pub name: Src<'a>,
    /// The formatted value ("2.40 kHz": the unit after the last space is set apart).
    pub value: &'a str,
    /// The value along the bar (0…1).
    pub pos: f32,
    /// Its fill from the middle (a pan).
    pub bipolar: bool,
    /// A modulator's swing (0…1 either side of `pos`) and where it has it now.
    pub swing: Option<(f32, f32)>,
}

/// Paint a value bar in `rect` (20 pt high at 100 %), driven by `d`.
pub fn value_bar(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &BarLook<'_>, d: Drivers) {
    b.register(rect, "value_bar", TOKENS, &[]);
    let t = &s.t;
    let rect = b.snap_rect(rect);
    let px = b.px();
    let r = s.s(3.0);
    let a = if st.disabled { 0.55 } else { 1.0 };
    super::cap::well_body(b, rect, r, s, t.well, a);
    let inner = rect.shrink(px);
    let at = |v: f32| inner.left() + inner.width() * v.clamp(0.0, 1.0);
    let from: f32 = if look.bipolar { 0.5 } else { 0.0 };
    let (l, rr) = (at(from.min(look.pos)), at(from.max(look.pos)));
    let fill = if st.active {
        with_alpha(t.focus, if t.dark { 0.30 } else { 0.22 })
    } else if d.clip {
        with_alpha(t.auto, 0.30)
    } else {
        with_alpha(t.placeholder, if t.dark { 0.30 } else { 0.26 })
    };
    if rr - l > 0.0 {
        b.rect(
            Rect::from_min_max(Pos2::new(l, inner.top()), Pos2::new(rr, inner.bottom())),
            Fill::Solid(fill.gamma_multiply(a)),
        );
    }
    let edge_ink = if st.disabled {
        t.disabled
    } else if st.active {
        t.focus
    } else if d.clip {
        t.auto
    } else if st.hovered {
        t.text
    } else {
        t.text_dim
    };
    let ew = (s.k / px).round().max(1.0) * px;
    let ex = b.snap(at(look.pos) - ew * 0.5);
    b.rect(
        Rect::from_min_max(
            Pos2::new(ex, inner.top()),
            Pos2::new(ex + ew, inner.bottom()),
        ),
        Fill::Solid(edge_ink),
    );
    if let Some((swing, live)) = look.swing.filter(|_| !st.disabled) {
        let band = s.s(3.0);
        let (bl, br) = (at(look.pos - swing), at(look.pos + swing));
        b.rect(
            Rect::from_min_max(
                Pos2::new(bl, inner.bottom() - band),
                Pos2::new(br, inner.bottom()),
            ),
            Fill::Solid(with_alpha(t.modulated, 0.55)),
        );
        let lx = b.snap(at(live));
        b.rect(
            Rect::from_min_max(
                Pos2::new(lx, inner.top()),
                Pos2::new(lx + px, inner.bottom()),
            ),
            Fill::Solid(t.modulated),
        );
    }
    let pad = s.s(8.0);
    let name_ink = if st.disabled { t.disabled } else { t.text };
    let name = Text {
        src: look.name,
        role: Role::Label,
        caps: false,
        align: Align2::LEFT_CENTER,
        policy: Policy::Abbreviate,
        color: name_ink,
    };
    let _ = text::paint(b, inner.shrink2(Vec2::new(pad, 0.0)), &name, s.tk());
    let num_ink = if st.disabled {
        t.disabled
    } else if st.active {
        t.focus
    } else if d.clip {
        t.auto
    } else {
        t.text
    };
    // The number and its unit a size down after it, in one run (`text`'s value layout).
    let nt = Text::new(look.value, Role::Value, num_ink).align(Align2::RIGHT_CENTER);
    let nw = text::measure(b.ctx(), &nt, s.tk()).x + 1.0;
    let right = inner.right() - pad;
    let nr = Rect::from_min_max(
        Pos2::new(right - nw, inner.top()),
        Pos2::new(right, inner.bottom()),
    );
    let unit = (!st.disabled).then_some(t.placeholder);
    let _ = text::with_unit_ink(unit, || text::paint(b, nr, &nt, s.tk()));
    let sh = s.s(6.0);
    drive_sign::row(
        b,
        Pos2::new(nr.left() - s.s(4.0) - sh * 0.9, inner.center().y),
        sh,
        Drivers {
            clip: d.clip,
            modulated: d.modulated,
            mapped: d.mapped,
            vca: d.vca,
        },
        s,
        None,
    );
    if st.focused {
        let g = s.s(2.0);
        b.rrect_stroke(rect.expand(g), r + g, px, t.focus);
    }
    let _ = mix;
    super::automate::note_painted(Painted::Value {
        x: nr.left(),
        y: inner.center().y,
        h: sh,
        drew: d,
    });
}
