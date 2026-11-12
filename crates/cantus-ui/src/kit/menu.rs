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

    /// Its height in `s` at 100 % (a recessed family's rows are `des-controls`' 24 pt, its
    /// headings 24 and its separators 9).
    pub fn height_in(&self, s: &Style) -> f32 {
        self.height_of(s.t.system.recessed())
    }

    fn height_of(&self, recessed: bool) -> f32 {
        if recessed {
            match self {
                Item::Action { .. } => super::menu_slate::ROW,
                Item::Band(_) => super::menu_slate::HEAD,
                Item::Separator => super::menu_slate::SEP,
            }
        } else {
            match self {
                Item::Action { .. } => ROW,
                Item::Band(_) => BAND,
                Item::Separator => SEPARATOR,
            }
        }
    }

    /// Its height at 100 % in the installed theme's system.
    fn height(&self) -> f32 {
        self.height_of(installed_recessed())
    }

    /// Whether a click can act on it.
    pub fn is_action(&self) -> bool {
        matches!(self, Item::Action { enabled: true, .. })
    }
}

/// Whether the installed theme builds its keys recessed (Slate and its siblings).
fn installed_recessed() -> bool {
    super::tokens::current_system().recessed()
}

/// The sheet's padding at its top and foot in a system.
fn pad(recessed: bool) -> f32 {
    if recessed {
        super::menu_slate::PAD
    } else {
        4.0
    }
}

/// The height of a menu of `items` at scale `k`, frame included (the installed theme's
/// rows: a view measures with it and paints with the installed style).
pub fn height(items: &[Item<'_>], k: f32) -> f32 {
    let r = installed_recessed();
    items.iter().map(|i| i.height_of(r)).sum::<f32>() * k + 2.0 * pad(r) * k
}

/// The height of a menu of `items` in `s` (its density in), frame included.
pub fn height_in(items: &[Item<'_>], s: &Style) -> f32 {
    let (r, k) = (s.t.system.recessed(), s.d(1.0));
    items.iter().map(|i| i.height_of(r)).sum::<f32>() * k + 2.0 * pad(r) * k
}

/// The rect of row `i` in a menu of `items` occupying `rect`, in `s`.
pub fn row_in(rect: Rect, items: &[Item<'_>], s: &Style, i: usize) -> Rect {
    let (r, k) = (s.t.system.recessed(), s.d(1.0));
    let y = rect.top() + pad(r) * k + items[..i].iter().map(|it| it.height_of(r)).sum::<f32>() * k;
    Rect::from_min_size(
        Pos2::new(rect.left() + 4.0 * k, y),
        Vec2::new(rect.width() - 8.0 * k, items[i].height_of(r) * k),
    )
}

/// The rect of row `i` in a menu occupying `rect`.
pub fn row(rect: Rect, items: &[Item<'_>], k: f32, i: usize) -> Rect {
    let r = installed_recessed();
    let y = rect.top() + pad(r) * k + items[..i].iter().map(|it| it.height_of(r)).sum::<f32>() * k;
    Rect::from_min_size(
        Pos2::new(rect.left() + 4.0 * k, y),
        Vec2::new(rect.width() - 8.0 * k, items[i].height() * k),
    )
}

/// Paint a menu in `rect` with `hovered` highlighted.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, items: &[Item<'_>], hovered: Option<usize>) {
    if s.t.system.recessed() {
        super::menu_slate::paint(b, rect, s, items, hovered, true);
        return;
    }
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
        let rr = b.snap_rect(row_in(rect, items, s, i));
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

/// The width a menu of `items` needs for every row to read whole (round 39, ADR 0109): the
/// check column, the words, the key and the chevron with their gaps, the frame's insets.
pub fn width(ctx: &egui::Context, items: &[Item<'_>], s: &Style) -> f32 {
    if s.t.system.recessed() {
        return super::menu_slate::width(ctx, items, s);
    }
    let measure = |src: Src<'_>, role: Role, caps: bool| {
        let t = Text {
            src,
            role,
            caps,
            align: Align2::LEFT_CENTER,
            policy: Policy::Elide(Elide::End),
            color: s.t.menu_text,
        };
        text::measure(ctx, &t, s.tk()).x
    };
    let inset = s.s(8.0);
    items
        .iter()
        .map(|it| match *it {
            Item::Separator => 0.0,
            Item::Band(label) => inset + s.s(16.0) + measure(label, Role::Section, true),
            Item::Action {
                label,
                shortcut,
                submenu,
                ..
            } => {
                let key = shortcut.map_or(0.0, |sc| {
                    s.s(12.0) + measure(Src::Str(sc), Role::Body, false)
                });
                let chevron = if submenu { s.s(18.0) } else { 0.0 };
                inset + s.s(22.0) + measure(label, Role::Body, false) + key + chevron + s.s(8.0)
            }
        })
        .fold(0.0, f32::max)
        + s.s(2.0)
}

/// A popup's contents kept inside the window (round 39, `des-controls`' overflow): a menu
/// taller than the window scrolls (the wheel, or the keys walking it) instead of running
/// off its foot; egui's popup then keeps the whole sheet on the screen, flipping it above
/// its anchor where there is no room under it.
pub fn fit<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let room = (ui.ctx().content_rect().height() - 2.0 * FIT_MARGIN).max(80.0);
    // `min_scrolled_height` too: a popup's first frame (egui's sizing pass) measures it in
    // `default_area_size` (400 pt tall), and the scroll area took those 400 as its room —
    // a menu taller than that scrolled in a window with room for all of it (the View menu,
    // 2026-11-11). With both, it is as tall as its content up to the window's room.
    egui::ScrollArea::vertical()
        .id_salt("cantus-menu-fit")
        .max_height(room)
        .min_scrolled_height(room)
        .auto_shrink([true, true])
        .show(ui, add)
        .inner
}

/// What [`fit`] leaves between a menu and the window's edges, in points.
pub const FIT_MARGIN: f32 = 8.0;
