//! `Menu` — FL 2026's menus: light rows of 22 pt on the dark app, section bands, a check
//! column at the left, chevrons for submenus at the right, shortcuts right-aligned. The
//! widget ([`widgets::menu_list`](super::widgets::menu_list)) reports a right-click on a
//! checkable item as a toggle that keeps the menu open.

use super::brush::{Brush, Fill};
use super::color::{shade, with_alpha};
use super::glyph::{self, Glyph};
use super::text::{self, Elide, Policy, Role, Src, Text};
use super::{Style, Tok};

/// The tokens a menu paints with (for inspect mode).
pub const TOKENS: &[Tok] = &[
    Tok::Menu,
    Tok::MenuBand,
    Tok::MenuHover,
    Tok::MenuText,
    Tok::MenuTextHover,
    Tok::Dark,
];
use egui::{Align2, Pos2, Rect, Vec2};

pub const ROW: f32 = 22.0;
pub const BAND: f32 = 18.0;
pub const SEPARATOR: f32 = 7.0;

/// A check-column mark.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Check {
    None,
    Off,
    On,
    Radio(bool),
}

/// One row of a menu.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Item<'a> {
    Action {
        label: Src<'a>,
        check: Check,
        shortcut: Option<&'a str>,
        submenu: bool,
        enabled: bool,
    },
    Band(Src<'a>),
    Separator,
}

impl<'a> Item<'a> {
    pub fn action(label: Src<'a>) -> Item<'a> {
        Item::Action {
            label,
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: true,
        }
    }

    fn height(&self) -> f32 {
        match self {
            Item::Action { .. } => ROW,
            Item::Band(_) => BAND,
            Item::Separator => SEPARATOR,
        }
    }

    /// Whether a click can act on it.
    pub fn is_action(&self) -> bool {
        matches!(self, Item::Action { enabled: true, .. })
    }
}

/// The height of a menu of `items` at scale `k`, frame included.
pub fn height(items: &[Item<'_>], k: f32) -> f32 {
    items.iter().map(|i| i.height()).sum::<f32>() * k + 8.0 * k
}

/// The rect of row `i` in a menu occupying `rect`.
pub fn row(rect: Rect, items: &[Item<'_>], k: f32, i: usize) -> Rect {
    let y = rect.top() + 4.0 * k + items[..i].iter().map(|it| it.height()).sum::<f32>() * k;
    Rect::from_min_size(
        Pos2::new(rect.left() + 4.0 * k, y),
        Vec2::new(rect.width() - 8.0 * k, items[i].height() * k),
    )
}

/// Paint a menu in `rect` with `hovered` highlighted.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, items: &[Item<'_>], hovered: Option<usize>) {
    b.register(rect, "menu", TOKENS, &[]);
    let t = &s.t;
    let rect = b.snap_rect(rect);
    let r = s.r(3.0);
    // Floating: a soft shadow, a hairline frame, the light face.
    b.rrect_soft(
        rect.translate(Vec2::new(0.0, s.s(3.0))),
        r + s.s(4.0),
        Fill::Solid(with_alpha(
            t.dark_shade,
            s.t.depth.min(1.0) * if t.dark { 0.40 } else { 0.16 },
        )),
        s.s(12.0),
    );
    b.rrect(rect, r, Fill::Solid(shade(t.menu, -0.22)));
    b.rrect(
        rect.shrink(b.px()),
        (r - b.px()).max(0.0),
        Fill::V(shade(t.menu, 0.015), shade(t.menu, -0.015)),
    );
    let dim = with_alpha(t.menu_text, 0.72);
    for (i, it) in items.iter().enumerate() {
        let rr = b.snap_rect(row(rect, items, s.d(1.0), i));
        match *it {
            Item::Separator => {
                b.hline(
                    rr.left() + s.s(6.0),
                    rr.right() - s.s(6.0),
                    b.snap_mid(rr.center().y),
                    b.px(),
                    shade(t.menu, -0.16),
                );
            }
            Item::Band(label) => {
                b.rect(rr, Fill::Solid(t.menu_band));
                let tt = Text {
                    src: label,
                    role: Role::Section,
                    caps: true,
                    align: Align2::LEFT_CENTER,
                    policy: Policy::Elide(Elide::End),
                    color: t.menu_text,
                };
                let _ = text::paint(b, rr.shrink2(Vec2::new(s.s(8.0), 0.0)), &tt, s.tk());
            }
            Item::Action {
                label,
                check,
                shortcut,
                submenu,
                enabled,
            } => {
                let hot = enabled && hovered == Some(i);
                if hot {
                    b.rrect(
                        rr,
                        s.s(2.0),
                        Fill::V(shade(t.menu_hover, 0.03), shade(t.menu_hover, -0.02)),
                    );
                }
                let ink = if hot { t.menu_text_hover } else { t.menu_text };
                let ink = if enabled { ink } else { with_alpha(ink, 0.45) };
                let col = Rect::from_min_size(rr.min, Vec2::new(s.s(22.0), rr.height()));
                match check {
                    Check::On => glyph::paint(b, col.shrink(s.s(5.0)), Glyph::Check, ink),
                    Check::Off => {}
                    Check::Radio(on) => {
                        b.circle_stroke(col.center(), s.s(4.0), s.s(1.2), ink);
                        if on {
                            b.circle(col.center(), s.s(2.2), Fill::Solid(ink));
                        }
                    }
                    Check::None => {}
                }
                let mut right = rr.right() - s.s(8.0);
                if submenu {
                    let g = Rect::from_center_size(
                        Pos2::new(right - s.s(4.0), rr.center().y),
                        Vec2::splat(s.s(10.0)),
                    );
                    glyph::paint(b, g, Glyph::Menu, ink);
                    right = g.left() - s.s(4.0);
                }
                if let Some(sc) = shortcut {
                    let w = text::measure(b.ctx(), &Text::new(sc, Role::Body, ink), s.tk()).x;
                    let sr = Rect::from_min_max(
                        Pos2::new(right - w, rr.top()),
                        Pos2::new(right, rr.bottom()),
                    );
                    let _ = text::paint(
                        b,
                        sr,
                        &Text::new(sc, Role::Body, if hot { ink } else { dim })
                            .align(Align2::RIGHT_CENTER),
                        // At the size it was measured at.
                        s.tk(),
                    );
                    right = sr.left() - s.s(12.0);
                }
                let lr = Rect::from_min_max(
                    Pos2::new(col.right(), rr.top()),
                    Pos2::new(right, rr.bottom()),
                );
                let tt = Text {
                    src: label,
                    role: Role::Body,
                    caps: false,
                    align: Align2::LEFT_CENTER,
                    policy: Policy::Elide(Elide::End),
                    color: ink,
                };
                let _ = text::paint(b, lr, &tt, s.tk());
            }
        }
    }
}
