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
    /// fx:colourways — a colourway's swatch (surface, accent, second voice), ringed when
    /// chosen: a radio row that shows what it picks.
    Swatch([egui::Color32; 3], bool),
}

/// fx:colourways — a swatch in the check column: three stripes, a ring when chosen.
pub fn swatch(
    b: &mut super::Brush,
    col: Rect,
    c: [egui::Color32; 3],
    on: bool,
    ink: egui::Color32,
    s: &super::Style,
) {
    let r = Rect::from_center_size(col.center(), Vec2::new(s.s(14.0), s.s(10.0)));
    let w = r.width() / 3.0;
    for (i, colour) in c.iter().enumerate() {
        let x = r.left() + w * i as f32;
        b.rect(
            Rect::from_min_max(Pos2::new(x, r.top()), Pos2::new(x + w, r.bottom())),
            super::Fill::Solid(*colour),
        );
    }
    b.rrect_stroke(
        r.expand(if on { s.s(1.5) } else { s.s(0.5) }),
        s.s(2.0),
        if on { s.s(1.2) } else { s.s(0.6) },
        ink,
    );
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
                    Check::Swatch(c, on) => swatch(b, col, c, on, ink, s), // fx:colourways
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

/// Where a list `h` tall opens from `opener` (the control that opened it), its top-left at
/// `anchor` (usually under the opener), in the window's `screen`: the place and the height
/// the list has there (less than `h` when it must scroll). Under its opener when it fits
/// there, above it when it fits there instead; else it scrolls, under its opener while
/// eight `row`s find room there, else above it (review 2 of round 40, R2-B6). It never
/// covers its opener (beta.14: a list raised over its field hid what it was a list of;
/// it was raised until the window held it whole). Every popup list
/// anchored on a control places itself here (GitHub issue #1: the instruments' preset
/// list opened at the window's right edge, away from its well).
pub fn place(screen: Rect, anchor: Pos2, opener: Rect, h: f32, row: f32) -> (Pos2, f32) {
    let m = FIT_MARGIN;
    let below = screen.bottom() - m - anchor.y;
    let above = opener.top() - (screen.top() + m);
    if h <= below {
        (anchor, h)
    } else if h <= above {
        (Pos2::new(anchor.x, opener.top() - h), h)
    } else if below >= above.min(row * 8.0) {
        (anchor, below.max(row * 2.0))
    } else {
        (Pos2::new(anchor.x, opener.top() - above), above)
    }
}

/// Where a field's list `h` tall (its sheet's edges counted), `w` wide, opens from `field`
/// in the window's `screen`: its top-left and the height it has there (less than `h` when
/// it must scroll). A dropdown never covers the field that opened it (beta.14, the owner:
/// the audio popover's *Input device* list rose over the *Driver* row, its rows on the
/// rows beneath): under the field when it fits there, above it when it fits there
/// instead, else on the side with more room, scrolling (under it while eight `row`s find
/// room there). Its left edge is the field's, moved left only as far as the window's
/// right edge asks.
pub fn place_beside(screen: Rect, field: Rect, w: f32, h: f32, row: f32) -> (Pos2, f32) {
    let m = FIT_MARGIN;
    let gap = DROP_GAP;
    let below = screen.bottom() - m - (field.bottom() + gap);
    let above = (field.top() - gap) - (screen.top() + m);
    let x = field
        .left()
        .min(screen.right() - m - w)
        .max(screen.left() + m);
    let under = |room: f32| (Pos2::new(x, field.bottom() + gap), room);
    let over = |room: f32| (Pos2::new(x, field.top() - gap - room), room);
    if h <= below {
        under(h)
    } else if h <= above {
        over(h)
    } else if below >= above.min(row * 8.0) {
        under(below.max(row * 2.0))
    } else {
        over(above)
    }
}

/// The gap between a field and its open list, in points.
pub const DROP_GAP: f32 = 2.0;

/// What [`dropdown`] reports: the rows' clicks, the sheet's rect (the whole of what is the
/// list's: a press on its edge or its scroll bar is not elsewhere) and the rows' rect.
#[derive(Copy, Clone, Debug)]
pub struct Dropped {
    pub clicks: super::widgets::MenuClicks,
    pub sheet: Rect,
    pub rows: Rect,
}

/// A field's open list (every combo of the app: Settings' lists, the audio popover's, the
/// inspector's, the units', the dialogs'): `items` on the kit's popup sheet — opaque, its
/// frame and shadow ([`super::automate::frame`]) — placed by [`place_beside`] from where
/// `field` is drawn **this frame** (a list kept at the place its field had when it was
/// clicked was left behind when the page moved), scrolling when the window is shorter
/// than the list, in an area `id` on `order`. `extra` paints over the rows (the cores
/// list's engines).
pub fn dropdown(
    ctx: &egui::Context,
    id: egui::Id,
    order: egui::Order,
    field: Rect,
    items: &[Item<'_>],
    width: f32,
    extra: impl FnOnce(&mut egui::Ui, Rect),
) -> Dropped {
    let s = Style::current();
    let width = width.max(field.width()).max(self::width(ctx, items, &s));
    let h = height_in(items, &s);
    let sheet = super::automate::frame();
    let edge = sheet.total_margin().sum() + Vec2::splat(2.0);
    let screen = ctx.content_rect();
    let (at, room) = place_beside(screen, field, width + edge.x, h + edge.y, s.d(ROW));
    let room = (room - edge.y).max(s.d(ROW) * 2.0);
    let mut rows = Rect::NOTHING;
    let area = egui::Area::new(id)
        .order(order)
        .fixed_pos(at)
        .constrain(false)
        .show(ctx, |ui| {
            sheet
                .show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt(id.with("scroll"))
                        .max_height(room)
                        .min_scrolled_height(room.min(h))
                        .show(ui, |ui| {
                            let (resp, c) = super::widgets::menu_list(ui, items, width);
                            rows = resp.rect;
                            extra(ui, resp.rect);
                            c
                        })
                        .inner
                })
                .inner
        });
    Dropped {
        clicks: area.inner,
        sheet: area.response.rect,
        rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect {
        min: Pos2::new(0.0, 0.0),
        max: Pos2::new(800.0, 600.0),
    };

    fn field(y: f32) -> Rect {
        Rect::from_min_size(Pos2::new(100.0, y), Vec2::new(300.0, 24.0))
    }

    #[test]
    fn a_dropdown_opens_under_its_field_above_it_or_scrolling_never_over_it() {
        // Room under it: under it, whole.
        let f = field(100.0);
        let (at, h) = place_beside(SCREEN, f, 320.0, 200.0, 22.0);
        assert_eq!((at, h), (Pos2::new(100.0, f.bottom() + DROP_GAP), 200.0));
        // No room under it, room above: above it, whole.
        let f = field(500.0);
        let (at, h) = place_beside(SCREEN, f, 320.0, 200.0, 22.0);
        assert_eq!(h, 200.0);
        assert_eq!(at.y + h, f.top() - DROP_GAP);
        // Room on neither side: it scrolls on the larger, and never covers the field.
        for y in [150.0, 280.0, 420.0] {
            let f = field(y);
            let (at, h) = place_beside(SCREEN, f, 320.0, 900.0, 22.0);
            let list = Rect::from_min_size(at, Vec2::new(320.0, h));
            assert!(h < 900.0, "it scrolls");
            assert!(!list.intersects(f), "{list:?} covers {f:?}");
            assert!(SCREEN.contains_rect(list), "{list:?} runs off the window");
        }
        // At the window's right edge: moved left just enough.
        let f = Rect::from_min_size(Pos2::new(700.0, 100.0), Vec2::new(90.0, 24.0));
        let (at, _) = place_beside(SCREEN, f, 320.0, 100.0, 22.0);
        assert_eq!(at.x, 800.0 - FIT_MARGIN - 320.0);
    }

    #[test]
    fn an_anchored_list_never_rises_over_its_opener() {
        // Taller than the room on either side of its opener, short enough for the window:
        // round 40 raised it over the opener; it scrolls beside it now.
        let opener = field(300.0);
        let (at, h) = place(SCREEN, opener.left_bottom(), opener, 500.0, 22.0);
        let list = Rect::from_min_size(at, Vec2::new(300.0, h));
        assert!(
            !list.intersects(opener.shrink(0.5)),
            "{list:?} over {opener:?}"
        );
    }
}
