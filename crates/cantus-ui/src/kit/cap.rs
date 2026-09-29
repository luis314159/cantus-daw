//! `Cap` — the raised recipe as a part: fader caps, step keys, channel buttons, and
//! `ToolbarKey`, the 28 × 28 key whose active state is FL's orange highlight gradient.

use super::brush::{Brush, Fill};
use super::color::shade;
use super::glyph::{self, Glyph};
use super::light::{self, Raised, RaisedOpts};
use super::text::{self, Role, Text};
use super::{focus_ring, State, Style, Tok};
use egui::{Color32, Rect};

/// The tokens a cap paints with (for inspect mode).
pub const TOKENS: &[Tok] = &[Tok::Cap, Tok::Selection, Tok::Focus];
/// The tokens a toolbar key paints with.
pub const KEY_TOKENS: &[Tok] = &[
    Tok::Toolbar,
    Tok::Highlight,
    Tok::HighlightBand,
    Tok::HighlightGlyph,
    Tok::Text,
    Tok::TextDim,
    Tok::Focus,
];

/// What a cap is made of.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct CapLook {
    /// The face colour at rest; `None` is the theme's `cap`.
    pub base: Option<Color32>,
    /// The colour it takes when lit (a selected fader is lime, an armed one orange);
    /// `None` is the theme's selection colour.
    pub lit: Option<Color32>,
    /// A groove across the face, this far down (0…1).
    pub groove: Option<f32>,
    /// Corner radius in points at 100 %; `None` is the theme's.
    pub radius: Option<f32>,
}

/// The face colour a cap shows in `st`.
pub fn face(s: &Style, st: State, look: &CapLook) -> Color32 {
    let base = if st.lit {
        look.lit.unwrap_or(s.t.selection)
    } else {
        look.base.unwrap_or(s.t.cap)
    };
    if st.active {
        shade(base, -0.03)
    } else if st.hovered {
        shade(base, 0.04)
    } else {
        base
    }
}

/// Paint a cap filling `rect`.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &CapLook) {
    b.register(rect, "cap", TOKENS, &[]);
    let r = look.radius.map_or(s.s(s.t.radius), |r| s.r(r));
    let base = face(s, st, look);
    light::raised(
        b,
        rect,
        r,
        base,
        RaisedOpts {
            groove: look.groove,
            // A pressed cap sits down: no drop under it.
            drop: !st.active,
            k: s.k,
            alpha: st.alpha(),
            depth: s.t.depth,
        },
    );
    if st.focused {
        focus_ring(b, rect, r, s);
    }
}

/// What a toolbar key shows.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum KeyFace<'a> {
    Glyph(Glyph),
    /// A short word (a mode letter, "REC").
    Text(&'a str),
}

/// The highlight gradient FL uses for an active key: the highlight at the edges, its
/// pale band through the middle (`fl-studio.md` §1.2), deeper with the theme's depth.
pub fn highlight_stops(s: &Style) -> [(f32, Color32); 4] {
    let (h, band, d) = (s.t.highlight, s.t.highlight_band, s.t.depth);
    let band = h.lerp_to_gamma(band, d.clamp(0.0, 1.0));
    [
        (0.0, shade(h, 0.02 * d)),
        (0.42, band),
        (0.58, band),
        (1.0, shade(h, -0.04 * d)),
    ]
}

/// Paint a toolbar key: a dark cap at rest, the highlight gradient when active (lit).
pub fn toolbar_key(b: &mut Brush, rect: Rect, s: &Style, st: State, face: KeyFace<'_>) {
    b.register(rect, "toolbar_key", KEY_TOKENS, &[]);
    let r = s.s(s.t.radius);
    let a = st.alpha();
    let ink = if st.lit {
        let stops = highlight_stops(s);
        let rect = b.snap_rect(rect);
        let px = b.px();
        let c = Raised::at(s.t.highlight, s.t.depth);
        b.rrect(rect, r, Fill::Solid(c.outline.gamma_multiply(a)));
        b.rrect(rect.shrink(px), (r - px).max(0.0), Fill::Stops(&stops));
        s.t.highlight_glyph
    } else {
        let base = if s.t.dark {
            shade(s.t.toolbar, -0.05)
        } else {
            shade(s.t.toolbar, 0.06)
        };
        let base = if st.active {
            shade(base, -0.03)
        } else if st.hovered {
            shade(base, 0.05)
        } else {
            base
        };
        light::raised(
            b,
            rect,
            r,
            base,
            RaisedOpts {
                drop: !st.active,
                k: s.k,
                alpha: a,
                depth: s.t.depth,
                ..Default::default()
            },
        );
        if st.hovered {
            s.t.text
        } else {
            s.t.text_dim
        }
    };
    let ink = ink.gamma_multiply(a);
    let inner = rect.shrink(s.s(6.0));
    match face {
        KeyFace::Glyph(g) => glyph::paint(b, inner, g, ink),
        KeyFace::Text(t) => {
            let _ = text::paint(
                b,
                rect.shrink(s.s(2.0)),
                &Text::new(t, Role::Micro, ink),
                s.tk(),
            );
        }
    }
    if st.focused {
        focus_ring(b, rect, r, s);
    }
}
