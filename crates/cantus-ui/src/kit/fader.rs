//! `Fader` — a 4-pt groove with ticks every 6 dB and FL's cap: 16 × 25 pt, lit above its
//! groove line and hollow below it, the lightest object on a strip. Lime (the selection
//! colour) when its strip is selected, with the groove below the cap filled lime too.

use super::brush::{Brush, Fill};
use super::color::{shade, with_alpha};
use super::light::{self, Raised, RaisedOpts};
use super::{State, Style, Tok};

/// The tokens a fader paints with in a dark theme (for inspect mode).
pub const TOKENS: &[Tok] = &[Tok::Cap, Tok::Selection, Tok::Well, Tok::Text, Tok::Focus];
/// In a light theme (its ticks are the secondary text).
pub const LIGHT_TOKENS: &[Tok] = &[
    Tok::Cap,
    Tok::Selection,
    Tok::Well,
    Tok::TextDim,
    Tok::Focus,
];
use egui::{Pos2, Rect, Vec2};

/// The cap's size at 100 %.
pub const CAP: Vec2 = Vec2::new(16.0, 25.0);

/// What a fader shows. Positions are 0 (bottom) … 1 (top).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FaderLook<'a> {
    pub pos: f32,
    /// Tick positions (every 6 dB, usually), drawn beside the groove.
    pub ticks: &'a [f32],
    /// The unity mark, drawn a little longer.
    pub unity: Option<f32>,
}

/// The cap's rect for `pos` in a fader occupying `rect`.
pub fn cap_rect(rect: Rect, s: &Style, pos: f32) -> Rect {
    let cap = CAP * s.k;
    let travel = (rect.height() - cap.y).max(0.0);
    let cy = rect.bottom() - cap.y * 0.5 - travel * pos.clamp(0.0, 1.0);
    Rect::from_center_size(Pos2::new(rect.center().x, cy), cap)
}

/// The position (0…1) a pointer at `y` asks for, for a fader in `rect`.
pub fn pos_at(rect: Rect, s: &Style, y: f32) -> f32 {
    let cap = CAP.y * s.k;
    let travel = (rect.height() - cap).max(1.0);
    ((rect.bottom() - cap * 0.5 - y) / travel).clamp(0.0, 1.0)
}

/// Paint a vertical fader in `rect`; `st.lit` is "the strip is selected".
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &FaderLook<'_>) {
    let toks = if s.t.dark { TOKENS } else { LIGHT_TOKENS };
    b.register(rect, "fader", toks, &[]);
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let cap = cap_rect(rect, s, look.pos);
    let cx = b.snap(rect.center().x);
    let gw = s.s(4.0);
    let top = rect.top() + CAP.y * s.k * 0.5;
    let bottom = rect.bottom() - CAP.y * s.k * 0.5;
    let groove = Rect::from_min_max(
        Pos2::new(cx - gw * 0.5, top - gw),
        Pos2::new(cx + gw * 0.5, bottom + gw),
    );

    // Ticks either side of the groove.
    let tick = with_alpha(if s.t.dark { s.t.text } else { s.t.text_dim }, 0.30);
    for &t in look.ticks {
        let y = bottom - (bottom - top) * t;
        let long = look.unity.is_some_and(|u| (u - t).abs() < 1e-3);
        let len = s.s(if long { 5.0 } else { 3.0 });
        let y = b.snap_mid(y);
        b.hline(
            groove.left() - s.s(2.0) - len,
            groove.left() - s.s(2.0),
            y,
            b.px(),
            f(tick),
        );
        b.hline(
            groove.right() + s.s(2.0),
            groove.right() + s.s(2.0) + len,
            y,
            b.px(),
            f(tick),
        );
    }

    let groove_base = shade(s.t.well, if s.t.dark { -0.10 } else { -0.18 });
    light::sunken(b, groove, gw * 0.5, groove_base, a, s.t.depth);
    if st.lit {
        // The groove below the cap lights up in the selection colour.
        let lit =
            Rect::from_min_max(Pos2::new(groove.left(), cap.center().y), groove.max).shrink(b.px());
        b.rrect(lit, gw * 0.5, Fill::Solid(f(shade(s.t.selection, -0.08))));
    }

    // The cap.
    let base = if st.lit { s.t.selection } else { s.t.cap };
    let base = if st.active {
        shade(base, -0.03)
    } else if st.hovered {
        shade(base, 0.03)
    } else {
        base
    };
    // FL's cap: lit above its groove, hollow below; the theme's depth scales the relief.
    let d = s.t.depth;
    let stops = [
        (0.0, f(shade(base, -0.10 * d))),
        (0.43, f(shade(base, 0.14 * d))),
        (0.48, f(shade(base, 0.08 * d))),
        (0.52, f(shade(base, -0.42 * d))),
        (1.0, f(shade(base, -0.08 * d))),
    ];
    let c = Raised::at(base, d);
    light::raised_face(
        b,
        cap,
        s.r(2.0),
        &c,
        Fill::Stops(&stops),
        RaisedOpts {
            groove: Some(0.5),
            drop: !st.active,
            k: s.k,
            alpha: a,
            depth: d,
        },
    );
    if st.focused {
        super::focus_ring(b, cap, s.r(2.0), s);
    }
}
