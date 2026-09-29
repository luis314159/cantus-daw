//! The workspace's chrome, skinned (ADR 0024 §5, §10): `Tab`, `Splitter` (a 1-px line
//! that lights on hover, a grip in the middle) and `Scrollbar` (always visible, 6 pt;
//! its thumb has grips at both ends because dragging an end zooms, as in FL).

use super::brush::{Brush, Fill};
use super::color::{shade, with_alpha};
use super::light::{self, RaisedOpts};
use super::text::{self, Elide, Policy, Role, Src, Text};
use super::{focus_ring, State, Style, Tok};

/// The tokens a tab paints with (for inspect mode).
pub const TAB_TOKENS: &[Tok] = &[
    Tok::PanelRaised,
    Tok::Panel,
    Tok::Focus,
    Tok::Text,
    Tok::TextDim,
];
/// The tokens a splitter paints with.
pub const SPLITTER_TOKENS: &[Tok] = &[Tok::Panel, Tok::Focus, Tok::Text, Tok::TextDim];
/// The tokens a scrollbar paints with in a dark theme.
pub const SCROLLBAR_TOKENS: &[Tok] = &[Tok::Panel, Tok::PanelLight];
/// In a light theme (its thumb is a darker panel).
pub const SCROLLBAR_LIGHT_TOKENS: &[Tok] = &[Tok::Panel];
use egui::{Align2, Pos2, Rect, Vec2};

/// A tab's height at 100 %.
pub const TAB: f32 = 24.0;
/// A scrollbar's thickness at 100 %.
pub const SCROLLBAR: f32 = 6.0;
/// A splitter's hit area at 100 %.
pub const SPLITTER: f32 = 4.0;

/// Paint a tab; `st.lit` is the active one.
pub fn tab(b: &mut Brush, rect: Rect, s: &Style, st: State, label: Src<'_>) {
    b.register(rect, "tab", TAB_TOKENS, &[]);
    let rect = b.snap_rect(rect);
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let t = &s.t;
    let r = s.r(3.0);
    if st.lit {
        let base = t.panel_raised;
        b.rrect(rect, r, Fill::V(f(shade(base, 0.02)), f(base)));
        b.rect(
            Rect::from_min_max(Pos2::new(rect.left(), rect.center().y), rect.max),
            Fill::V(f(shade(base, 0.01)), f(base)),
        );
        b.hline(
            rect.left() + r,
            rect.right() - r,
            rect.top() + b.px() * 0.5,
            b.px(),
            f(shade(base, 0.10)),
        );
        let u = s.s(2.0);
        b.rect(
            b.snap_rect(Rect::from_min_max(
                Pos2::new(rect.left(), rect.bottom() - u),
                rect.max,
            )),
            Fill::Solid(f(t.focus)),
        );
    } else if st.hovered {
        b.rrect(
            rect,
            r,
            Fill::Solid(f(shade(t.panel, if t.dark { 0.03 } else { -0.03 }))),
        );
    }
    let color = if st.lit || st.hovered {
        t.text
    } else {
        t.text_dim
    };
    let tt = Text {
        src: label,
        role: Role::Control,
        caps: false,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: f(color),
    };
    let _ = text::paint(b, rect.shrink2(Vec2::new(s.s(8.0), 0.0)), &tt, s.tk());
    if st.focused {
        focus_ring(b, rect, r, s);
    }
}

/// Paint a splitter across `rect` (its long side is the line). Hover lights the line;
/// dragging thickens it. A three-dot grip marks its middle.
pub fn splitter(b: &mut Brush, rect: Rect, s: &Style, st: State) {
    b.register(rect, "splitter", SPLITTER_TOKENS, &[]);
    let vertical = rect.height() > rect.width();
    let t = &s.t;
    let rest = shade(t.panel, if t.dark { -0.14 } else { -0.12 });
    let (color, w) = if st.active {
        (t.focus, 2.0 * b.px())
    } else if st.hovered {
        (t.focus, b.px())
    } else {
        (rest, b.px())
    };
    let c = rect.center();
    if vertical {
        b.vline(c.x, rect.top(), rect.bottom(), w, color);
    } else {
        b.hline(rect.left(), rect.right(), c.y, w, color);
    }
    let dot = if st.hovered || st.active {
        t.text
    } else {
        t.text_dim
    };
    for i in -1..=1 {
        let o = i as f32 * s.s(4.0);
        let p = if vertical {
            Pos2::new(c.x, c.y + o)
        } else {
            Pos2::new(c.x + o, c.y)
        };
        b.circle(p, s.s(1.1), Fill::Solid(with_alpha(dot, 0.8)));
    }
}

/// Where a scrollbar's thumb is, for a view showing `start … start + len` (fractions of
/// the content) in a bar occupying `rect`.
pub fn thumb(rect: Rect, s: &Style, start: f32, len: f32) -> Rect {
    let vertical = rect.height() > rect.width();
    let (full, min) = if vertical {
        (rect.height(), s.s(16.0))
    } else {
        (rect.width(), s.s(16.0))
    };
    let len = len.clamp(0.0, 1.0);
    let size = (full * len).max(min).min(full);
    let at = (full - size) * (start / (1.0 - len).max(1e-6)).clamp(0.0, 1.0);
    if vertical {
        Rect::from_min_size(
            Pos2::new(rect.left(), rect.top() + at),
            Vec2::new(rect.width(), size),
        )
    } else {
        Rect::from_min_size(
            Pos2::new(rect.left() + at, rect.top()),
            Vec2::new(size, rect.height()),
        )
    }
}

/// Paint a scrollbar: a sunken track, a raised thumb with grips at both ends.
pub fn scrollbar(b: &mut Brush, rect: Rect, s: &Style, st: State, start: f32, len: f32) {
    let toks = if s.t.dark {
        SCROLLBAR_TOKENS
    } else {
        SCROLLBAR_LIGHT_TOKENS
    };
    b.register(rect, "scrollbar", toks, &[]);
    let t = &s.t;
    let r = rect.width().min(rect.height()) * 0.5;
    light::sunken(
        b,
        rect,
        r,
        shade(t.panel, if t.dark { -0.07 } else { -0.06 }),
        st.alpha(),
        s.t.depth,
    );
    let th = thumb(rect, s, start, len).shrink(b.px());
    let base = if t.dark {
        t.panel_light
    } else {
        shade(t.panel, -0.16)
    };
    let base = if st.active {
        shade(base, 0.10)
    } else if st.hovered {
        shade(base, 0.06)
    } else {
        base
    };
    light::raised(
        b,
        th,
        (th.width().min(th.height()) * 0.5).max(0.0),
        base,
        RaisedOpts {
            drop: false,
            k: s.k,
            alpha: st.alpha(),
            depth: s.t.depth,
            ..Default::default()
        },
    );
    if st.hovered || st.active {
        let vertical = rect.height() > rect.width();
        let grip = shade(base, -0.22);
        for end in [0.0f32, 1.0] {
            for i in 0..2 {
                let o = s.s(3.0 + 2.0 * i as f32);
                if vertical {
                    let y = if end == 0.0 {
                        th.top() + o
                    } else {
                        th.bottom() - o
                    };
                    b.hline(
                        th.left() + s.s(1.5),
                        th.right() - s.s(1.5),
                        b.snap_mid(y),
                        b.px(),
                        grip,
                    );
                } else {
                    let x = if end == 0.0 {
                        th.left() + o
                    } else {
                        th.right() - o
                    };
                    b.vline(
                        b.snap_mid(x),
                        th.top() + s.s(1.5),
                        th.bottom() - s.s(1.5),
                        b.px(),
                        grip,
                    );
                }
            }
        }
    }
}
