//! Menus in a recessed family (round 39, `des-controls`' menu sheet, owner-approved): a
//! raised sheet (6-pt corners, one soft shadow, a 1-px edge), rows of 24 pt, a 26-pt
//! column at the left for the check or the radio dot so labels align either way, the
//! shortcut at the right in ink 3, a submenu's chevron in its place; a section is a quiet
//! heading in ink 3, title case — never a capital bar; the hover an inset pill of the
//! accent at 24 % (Paper 13 %); a disabled row stays, in ink 4, so its key is still learnt.
//!
//! Several lists in one popup are one menu: inside a popup the popup's frame is the sheet
//! ([`super::automate::frame`]) and a list paints its rows only.

use super::brush::{Brush, Fill};
use super::color::{mix, with_alpha};
use super::menu::{row_in, Check, Item};
use super::text::{self, Elide, Policy, Role, Src, Text};
use super::{Style, Tok};
use egui::{Align2, Color32, Pos2, Rect, Vec2};

/// The tokens a Slate menu paints with (for inspect mode).
pub const TOKENS: &[Tok] = &[
    Tok::Menu,
    Tok::MenuText,
    Tok::Placeholder,
    Tok::Disabled,
    Tok::Focus,
    Tok::GridBar,
    Tok::Dark,
];

/// A row, a heading and a separator, and the sheet's padding at top and foot (points).
pub const ROW: f32 = 24.0;
pub const HEAD: f32 = 24.0;
pub const SEP: f32 = 9.0;
pub const PAD: f32 = 5.0;
/// The check column.
pub const GUTTER: f32 = 26.0;

/// The sheet's edge: a step of ink over the face (Night), the line (Paper).
pub fn edge(s: &Style) -> Color32 {
    if s.t.dark {
        mix(s.t.menu, s.t.text, 0.09)
    } else {
        s.t.grid_bar
    }
}

/// The sheet's corner radius.
pub fn radius(s: &Style) -> f32 {
    s.s(6.0)
}

/// The hover's pill.
pub fn hover_fill(s: &Style) -> Color32 {
    mix(s.t.menu, s.t.focus, if s.t.dark { 0.24 } else { 0.13 })
}

/// The sheet alone: one soft shadow (0 6 18 at 50 %, Paper 18 %), the face, its edge.
pub fn sheet(b: &mut Brush, rect: Rect, s: &Style) {
    let r = radius(s);
    b.rrect_soft(
        rect.translate(Vec2::new(0.0, s.s(6.0))),
        r + s.s(3.0),
        Fill::Solid(with_alpha(
            s.t.dark_shade,
            if s.t.dark { 0.50 } else { 0.18 },
        )),
        s.s(9.0),
    );
    b.rrect(rect, r, Fill::Solid(edge(s)));
    b.rrect(
        rect.shrink(b.px()),
        (r - b.px()).max(0.0),
        Fill::Solid(s.t.menu),
    );
}

/// Paint a menu in `rect`; `sheet` draws its own sheet (off inside a popup whose frame is
/// the sheet).
pub fn paint(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    items: &[Item<'_>],
    hovered: Option<usize>,
    with_sheet: bool,
) {
    b.register(rect, "menu", TOKENS, &[]);
    let rect = b.snap_rect(rect);
    if with_sheet {
        sheet(b, rect, s);
    }
    let t = &s.t;
    let px = b.px();
    for (i, it) in items.iter().enumerate() {
        let rr = b.snap_rect(row_in(rect, items, s, i));
        match *it {
            Item::Separator => {
                b.hline(
                    rr.left() + s.s(6.0),
                    rr.right() - s.s(6.0),
                    b.snap_mid(rr.center().y),
                    px,
                    mix(t.menu, t.disabled, if t.dark { 0.5 } else { 0.45 }),
                );
            }
            Item::Band(label) => {
                let tt = Text {
                    src: label,
                    role: Role::Section,
                    caps: false,
                    align: Align2::LEFT_BOTTOM,
                    policy: Policy::Elide(Elide::End),
                    color: t.placeholder,
                };
                let lr = Rect::from_min_max(
                    Pos2::new(rr.left() + s.s(8.0), rr.top()),
                    Pos2::new(rr.right() - s.s(8.0), rr.bottom() - s.s(4.0)),
                );
                let _ = text::paint(b, lr, &tt, s.tk() * 10.5 / 11.0);
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
                    b.rrect(rr, s.s(4.0), Fill::Solid(hover_fill(s)));
                }
                let ink = if enabled { t.menu_text } else { t.disabled };
                let quiet = if enabled { t.placeholder } else { t.disabled };
                let cx = rr.left() + s.s(10.0);
                let cy = rr.center().y;
                match check {
                    Check::On => {
                        let w = s.s(4.5);
                        b.polyline(
                            &[
                                Pos2::new(cx - w, cy),
                                Pos2::new(cx - w * 0.3, cy + w * 0.7),
                                Pos2::new(cx + w, cy - w * 0.75),
                            ],
                            s.s(1.6),
                            if enabled { t.focus } else { t.disabled },
                        );
                    }
                    Check::Radio(true) => {
                        b.circle(
                            Pos2::new(cx, cy),
                            s.s(3.0),
                            Fill::Solid(if enabled { t.focus } else { t.disabled }),
                        );
                    }
                    Check::Radio(false) => {
                        b.circle_stroke(
                            Pos2::new(cx, cy),
                            s.s(3.5),
                            px,
                            mix(t.menu, t.placeholder, 0.6),
                        );
                    }
                    Check::Off | Check::None => {}
                }
                let mut right = rr.right() - s.s(8.0);
                if submenu {
                    let (x, h) = (rr.right() - s.s(10.0), s.s(3.5));
                    b.polyline(
                        &[
                            Pos2::new(x - s.s(2.0), cy - h),
                            Pos2::new(x + s.s(2.0), cy),
                            Pos2::new(x - s.s(2.0), cy + h),
                        ],
                        s.s(1.25),
                        if hot { t.menu_text } else { t.placeholder },
                    );
                    right = x - s.s(8.0);
                } else if let Some(sc) = shortcut {
                    let st = Text::new(sc, Role::Label, quiet).align(Align2::RIGHT_CENTER);
                    let w = text::measure(b.ctx(), &st, s.tk()).x;
                    let sr = Rect::from_min_max(
                        Pos2::new(right - w - 1.0, rr.top()),
                        Pos2::new(right, rr.bottom()),
                    );
                    let _ = text::paint(b, sr, &st, s.tk());
                    right = sr.left() - s.s(12.0);
                }
                let lr = Rect::from_min_max(
                    Pos2::new(rr.left() + s.s(GUTTER - 4.0), rr.top()),
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

/// The width a Slate menu of `items` needs for every row to read whole.
pub fn width(ctx: &egui::Context, items: &[Item<'_>], s: &Style) -> f32 {
    let measure = |src: Src<'_>, role: Role, k: f32| {
        let t = Text {
            src,
            role,
            caps: false,
            align: Align2::LEFT_CENTER,
            policy: Policy::Elide(Elide::End),
            color: s.t.menu_text,
        };
        text::measure(ctx, &t, k).x
    };
    items
        .iter()
        .map(|it| match *it {
            Item::Separator => 0.0,
            Item::Band(label) => {
                s.s(4.0 + 8.0 + 8.0 + 4.0) + measure(label, Role::Section, s.tk() * 10.5 / 11.0)
            }
            Item::Action {
                label,
                shortcut,
                submenu,
                ..
            } => {
                let tail = if submenu {
                    s.s(18.0)
                } else {
                    shortcut.map_or(0.0, |sc| {
                        s.s(12.0) + measure(Src::Str(sc), Role::Label, s.tk()) + 1.0
                    })
                };
                s.s(4.0 + GUTTER - 4.0) + measure(label, Role::Body, s.tk()) + tail + s.s(8.0 + 4.0)
            }
        })
        .fold(0.0, f32::max)
        + s.s(2.0)
}
