//! `Segmented` — equal segments in a well, 1-px dividers, the selected one lit: filled,
//! its word in full-strength text, and a 2-pt underline in the section's hue. FL's
//! LIMIT | COMP, Play | Edit | Matrix, A | B.

use super::brush::{Brush, Fill};
use super::color::{shade, with_alpha};
use super::light;
use super::text::{self, Policy, Role, Src, Text};
use super::{focus_ring, State, Style, Tok};

/// The tokens a segmented control paints with in a dark theme, beside its section hue
/// (for inspect mode): the selected segment is a lighter well.
pub const DARK_TOKENS: &[Tok] = &[
    Tok::Well,
    Tok::Text,
    Tok::TextDim,
    Tok::Focus,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Bevel,
];
/// In a light theme: the selected segment is a raised light tile, its word in the ink
/// (r37:finish — judge 8: a knob-dark pill was a dark island on Paper).
pub const LIGHT_TOKENS: &[Tok] = &[
    Tok::Well,
    Tok::PanelRaised,
    Tok::Text,
    Tok::Label,
    Tok::Focus,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Bevel,
];
use egui::{Align2, Pos2, Rect};

/// The rect of segment `i` of `n` in `rect`.
pub fn segment(rect: Rect, n: usize, i: usize) -> Rect {
    let n = n.max(1) as f32;
    let w = rect.width() / n;
    Rect::from_min_max(
        Pos2::new(rect.left() + w * i as f32, rect.top()),
        Pos2::new(rect.left() + w * (i as f32 + 1.0), rect.bottom()),
    )
}

/// The segment under `x`, if any.
pub fn hit(rect: Rect, n: usize, x: f32) -> Option<usize> {
    if n == 0 || x < rect.left() || x >= rect.right() {
        return None;
    }
    Some((((x - rect.left()) / rect.width()) * n as f32) as usize).map(|i| i.min(n - 1))
}

/// Paint the control in `rect`: `labels[selected]` lit, `hovered` a step lighter.
pub fn paint(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    st: State,
    labels: &[Src<'_>],
    selected: usize,
    hovered: Option<usize>,
) {
    let n = labels.len();
    if n == 0 {
        return;
    }
    if s.t.system.recessed() && !matches!(s.skin.switch, super::skin::SwitchShape::Lamp) {
        paint_slate(b, rect, s, st, labels, selected, hovered);
        return;
    }
    let toks = match (
        matches!(s.skin.switch, super::skin::SwitchShape::Lamp),
        s.t.dark,
    ) {
        (true, true) => &[
            Tok::Well,
            Tok::Text,
            Tok::TextDim,
            Tok::Light,
            Tok::Focus,
            Tok::InnerShadow,
            Tok::Lip,
        ][..],
        (true, false) => &[
            Tok::Well,
            Tok::Text,
            Tok::Label,
            Tok::Light,
            Tok::Focus,
            Tok::InnerShadow,
            Tok::Lip,
        ][..],
        (false, true) => DARK_TOKENS,
        (false, false) => LIGHT_TOKENS,
    };
    b.register(rect, "segmented", toks, &[s.hue_tok]);
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let r = s.s(s.t.radius) + s.r(1.0);
    let rect = b.snap_rect(rect);
    let base = if s.t.dark {
        s.t.well
    } else {
        shade(s.t.well, 0.06)
    };
    let m = light::Material::of(&s.t);
    light::recess(b, rect, r, base, &m, a, s.k);
    let px = b.px();
    let inner = rect.shrink(px);
    let sel_fill = if s.t.dark {
        shade(base, 0.13)
    } else {
        s.t.panel_raised
    };
    let sel_text = s.t.text;
    for (i, label) in labels.iter().enumerate() {
        let seg = b.snap_rect(segment(inner, n, i));
        let lamp = matches!(s.skin.switch, super::skin::SwitchShape::Lamp);
        if i == selected && lamp {
            // A unit whose switches are lamps (round 32, ADR 0074): the chosen segment
            // glows whole in its hue.
            b.rrect(
                seg,
                (r - px).max(0.0),
                Fill::V(f(shade(s.hue, 0.06)), f(shade(s.hue, -0.06))),
            );
            b.hline(
                seg.left() + r,
                seg.right() - r,
                seg.top() + px * 0.5,
                px,
                f(with_alpha(s.t.light, 0.35)),
            );
        } else if i == selected {
            // The chosen segment stands up in the well: a small object, its bevel lit.
            let lit = light::Raised::at(sel_fill, s.t.depth);
            b.rrect(seg, (r - px).max(0.0), Fill::V(f(lit.top), f(lit.bottom)));
            b.hline(
                seg.left() + r,
                seg.right() - r,
                seg.top() + px * 0.5,
                px,
                f(m.bevel),
            );
            // The underline, in the hue.
            let u = s.s(2.0);
            b.rect(
                b.snap_rect(Rect::from_min_max(
                    Pos2::new(seg.left() + s.s(3.0), seg.bottom() - u),
                    Pos2::new(seg.right() - s.s(3.0), seg.bottom()),
                )),
                Fill::Solid(f(s.hue)),
            );
        } else if hovered == Some(i) {
            b.rrect(
                seg,
                (r - px).max(0.0),
                Fill::Solid(f(shade(base, if s.t.dark { 0.04 } else { -0.03 }))),
            );
        }
        if i > 0 && i != selected && i != selected + 1 {
            b.vline(
                seg.left(),
                seg.top() + s.s(4.0),
                seg.bottom() - s.s(4.0),
                px,
                f(shade(base, -0.12)),
            );
        }
        let color = if i == selected && lamp {
            super::color::on_fill(s.hue)
        } else if i == selected {
            sel_text
        } else if hovered == Some(i) {
            s.t.text
        } else if s.t.dark {
            s.t.text_dim
        } else {
            s.t.label
        };
        let t = Text {
            src: *label,
            role: Role::Control,
            caps: false,
            align: Align2::CENTER_CENTER,
            policy: Policy::Abbreviate,
            color: f(color),
        };
        let _ = text::paint(b, seg.shrink2(egui::vec2(s.s(4.0), 0.0)), &t, s.tk());
    }
    if st.focused {
        focus_ring(b, rect, r, s);
    }
}

/// The tokens a Slate segmented control paints with (for inspect mode).
pub const SLATE_TOKENS: &[Tok] = &[
    Tok::Well,
    Tok::InnerShadow,
    Tok::Lip,
    Tok::Menu,
    Tok::Text,
    Tok::TextDim,
    Tok::Placeholder,
    Tok::Focus,
];

/// A segmented control in a recessed family (round 39, `des-controls`' switches sheet): a
/// well; the chosen segment lifts, inset 2 pt — Night a raised pill with a lit top, Paper a
/// white one framed in the well's edge — its word Medium in ink 1; under the hand a step
/// lifted and ink 2; the others ink 3; 1-px dividers only between unchosen neighbours.
/// Never coloured: a choice among equals stays neutral.
fn paint_slate(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    st: State,
    labels: &[Src<'_>],
    selected: usize,
    hovered: Option<usize>,
) {
    b.register(rect, "segmented", SLATE_TOKENS, &[]);
    let n = labels.len();
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let t = &s.t;
    let r = s.s(t.radius);
    let rect = b.snap_rect(rect);
    super::cap::well_body(b, rect, r, s, t.well, a);
    let px = b.px();
    let inner = rect.shrink(px);
    let inset = s.s(2.0);
    let edge = super::color::opaque(t.inner_shadow);
    for (i, label) in labels.iter().enumerate() {
        let seg = b.snap_rect(segment(inner, n, i));
        let pill = seg.shrink(inset - px);
        if i == selected {
            if t.dark {
                b.rrect(pill, (r - inset).max(px), Fill::Solid(f(t.menu)));
                b.hline(
                    pill.left() + s.s(2.0),
                    pill.right() - s.s(2.0),
                    pill.top() + px * 0.5,
                    px,
                    f(super::color::mix(t.menu, t.text, 0.16)),
                );
            } else {
                b.rrect(pill, (r - inset).max(px), Fill::Solid(f(edge)));
                b.rrect(
                    pill.shrink(px),
                    (r - inset - px).max(px),
                    Fill::Solid(f(t.light)),
                );
            }
        } else if hovered == Some(i) && !st.disabled {
            b.rrect(
                pill,
                (r - inset).max(px),
                Fill::Solid(f(super::cap::well_fill(
                    s,
                    State {
                        hovered: true,
                        ..State::REST
                    },
                ))),
            );
        }
        if i > 0 && i != selected && i != selected + 1 {
            b.vline(
                seg.left(),
                seg.top() + s.s(5.0),
                seg.bottom() - s.s(5.0),
                px,
                f(super::color::mix(t.well, t.placeholder, 0.35)),
            );
        }
        // One weight for every segment, so a word that fits chosen fits unchosen (the
        // segments' widths are laid out for it); the choice is said by the pill and the ink.
        let role = Role::Control;
        let color = if st.disabled {
            t.disabled
        } else if i == selected {
            t.text
        } else if hovered == Some(i) {
            t.text_dim
        } else {
            t.placeholder
        };
        let tt = Text {
            src: *label,
            role,
            caps: false,
            align: Align2::CENTER_CENTER,
            policy: Policy::Abbreviate,
            color: f(color),
        };
        let _ = text::paint(b, seg.shrink2(egui::vec2(s.s(4.0), 0.0)), &tt, s.tk());
    }
    if st.focused {
        focus_ring(b, rect, r, s);
    }
}
