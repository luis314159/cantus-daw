//! `Tile` — an 18-pt list row (an FX slot, a browser row), FL's inspector tile: ▸ menu,
//! the name, a mix ring (its white arc is the wet amount), an LED. An empty tile keeps the
//! geometry and shows an italic placeholder ("Slot 5") on a slightly lighter fill, so
//! nothing jumps when a slot is filled.

use super::brush::{Brush, Fill};
use super::color::{shade, with_alpha};
use super::glyph::{self, Glyph};
use super::led;
use super::text::{self, Elide, Policy, Role, Src, Text};
use super::{State, Style, Tok};

/// The tokens a tile paints with (for inspect mode); its LED registers itself.
pub const TOKENS: &[Tok] = &[
    Tok::PanelRaised,
    Tok::Selection,
    Tok::Text,
    Tok::Placeholder,
    Tok::Focus,
];
use egui::{Align2, Pos2, Rect, Vec2};

/// The row height at 100 %.
pub const HEIGHT: f32 = 18.0;

/// What a tile shows.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TileLook<'a> {
    /// The loaded item's name; `None` is an empty slot.
    pub name: Option<Src<'a>>,
    /// What an empty slot is called.
    pub placeholder: Src<'a>,
    /// The wet amount (0…1), drawn as the mix ring; `None` hides the ring.
    pub mix: Option<f32>,
    /// The enable LED.
    pub on: bool,
}

/// Where a tile's parts are, for the widget to sense.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct TileRects {
    pub menu: Rect,
    pub name: Rect,
    pub mix: Rect,
    pub led: Rect,
}

pub fn rects(rect: Rect, s: &Style) -> TileRects {
    let h = rect.height();
    let menu = Rect::from_min_size(rect.min, Vec2::new(s.s(14.0), h));
    let led = Rect::from_center_size(
        Pos2::new(rect.right() - s.s(10.0), rect.center().y),
        Vec2::splat(s.s(11.0)),
    );
    let mix = Rect::from_center_size(
        Pos2::new(led.left() - s.s(12.0), rect.center().y),
        Vec2::splat(s.s(14.0)),
    );
    let name = Rect::from_min_max(
        Pos2::new(menu.right(), rect.top()),
        Pos2::new(mix.left() - s.s(4.0), rect.bottom()),
    );
    TileRects {
        menu,
        name,
        mix,
        led,
    }
}

/// Paint a tile in `rect`; `st.lit` marks the selected row.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &TileLook<'_>) {
    b.register(rect, "tile", TOKENS, &[]);
    let rect = b.snap_rect(rect);
    let a = st.alpha();
    let f = |c: egui::Color32| c.gamma_multiply(a);
    let dark = s.t.dark;
    let mut fill = s.t.panel_raised;
    if look.name.is_none() {
        fill = shade(fill, if dark { 0.03 } else { -0.015 });
    }
    if st.hovered {
        fill = shade(fill, if dark { 0.03 } else { -0.025 });
    }
    let border = shade(s.t.panel_raised, if dark { -0.10 } else { -0.12 });
    b.rect(
        rect,
        Fill::Solid(f(if st.lit { s.t.selection } else { border })),
    );
    b.rect(rect.shrink(b.px()), Fill::Solid(f(fill)));
    let r = rects(rect, s);
    let ink = s.t.text;
    glyph::paint(
        b,
        r.menu.shrink2(Vec2::new(s.s(3.0), s.s(4.0))),
        Glyph::Menu,
        f(if st.hovered {
            ink
        } else {
            with_alpha(ink, 0.75)
        }),
    );
    let t = match look.name {
        Some(src) => Text {
            src,
            role: Role::Body,
            caps: false,
            align: Align2::LEFT_CENTER,
            policy: Policy::Elide(Elide::Middle),
            color: f(ink),
        },
        None => Text {
            src: look.placeholder,
            role: Role::Placeholder,
            caps: false,
            align: Align2::LEFT_CENTER,
            policy: Policy::Elide(Elide::End),
            color: f(s.t.placeholder),
        },
    };
    let _ = text::paint(b, r.name, &t, s.tk());
    if let Some(mix) = look.mix {
        let c = r.mix.center();
        let rad = r.mix.width() * 0.5;
        b.circle(
            c,
            rad,
            Fill::Solid(f(shade(fill, -(0.10 + 0.12 * s.t.depth)))),
        );
        b.circle(
            c,
            rad - s.s(2.0),
            Fill::V(
                f(shade(fill, -0.08)),
                f(shade(fill, -(0.08 + 0.08 * s.t.depth))),
            ),
        );
        let ring = if look.name.is_some() {
            ink
        } else {
            with_alpha(ink, 0.35)
        };
        let m = mix.clamp(0.0, 1.0);
        if m > 0.001 {
            let a0 = -std::f32::consts::FRAC_PI_2;
            b.arc(
                c,
                rad - s.s(1.0),
                a0,
                a0 + m * std::f32::consts::TAU,
                s.s(1.6),
                f(ring),
                false,
            );
        }
    }
    led::paint(
        b,
        r.led.center(),
        s.s(9.0),
        look.on,
        s,
        Tok::Led,
        Tok::PanelRaised,
        a,
    );
    if st.focused {
        super::focus_ring(b, rect, 0.0, s);
    }
}
