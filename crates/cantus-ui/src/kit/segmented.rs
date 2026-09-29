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
pub const DARK_TOKENS: &[Tok] = &[Tok::Well, Tok::Text, Tok::TextDim, Tok::Focus];
/// In a light theme: the selected segment is knob-dark, like the knobs.
pub const LIGHT_TOKENS: &[Tok] = &[
    Tok::Well,
    Tok::KnobBody,
    Tok::KnobPointer,
    Tok::Text,
    Tok::Label,
    Tok::Focus,
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
    let toks = if s.t.dark { DARK_TOKENS } else { LIGHT_TOKENS };
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
    light::sunken(b, rect, r, base, a, s.t.depth);
    let px = b.px();
    let inner = rect.shrink(px);
    let sel_fill = if s.t.dark {
        shade(base, 0.13)
    } else {
        s.t.knob_body
    };
    let sel_text = if s.t.dark { s.t.text } else { s.t.knob_pointer };
    for (i, label) in labels.iter().enumerate() {
        let seg = b.snap_rect(segment(inner, n, i));
        if i == selected {
            let lit = light::Raised::at(sel_fill, s.t.depth);
            b.rrect(seg, (r - px).max(0.0), Fill::V(f(lit.top), f(lit.bottom)));
            b.hline(
                seg.left() + r,
                seg.right() - r,
                seg.top() + px * 0.5,
                px,
                f(with_alpha(lit.edge, 0.6)),
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
        let color = if i == selected {
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
