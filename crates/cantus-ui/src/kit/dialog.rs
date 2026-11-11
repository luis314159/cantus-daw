//! Buttons and the dialog frame (round 39, `des-controls`' dialogs sheet, owner-approved).
//!
//! **Buttons** — four kinds, six states each (rest, hover, pressed, focused, disabled,
//! busy): the *primary* in the accent with its label in `primary_ink`, the *secondary* a
//! lifted neutral (Paper: white framed in the well's edge), the *destructive* in
//! `destructive` with white words — never the default — and the *quiet* words that take a
//! well only under the hand. 28 pt in a dialog, 24 compact, corners 4, a shadow 1 pt down,
//! labels Medium 12, at least 84 (64) wide. One primary per dialog, at the right end.
//!
//! **The frame** — the window darkens under the `scrim`; the sheet is `raised` (the menus'
//! face), corners 8, a 1-px edge, one deep soft shadow; the title Medium 15 at 20 pt in, a
//! close × 24 pt from the right; the footer 56 pt a step toward the bar's colour under a
//! 1-px line, its buttons at the right 10 pt apart.

use super::brush::{Brush, Fill};
use super::color::{mix, opaque, with_alpha};
use super::text::{self, Elide, Policy, Role, Src, Text};
use super::{State, Style, Tok};
use egui::{Align2, Pos2, Rect, Vec2};

/// What a button does.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// The one action the dialog is for (Enter presses it).
    Primary,
    /// Every other action, Cancel among them.
    Secondary,
    /// An action that deletes; its words say what. Never the default.
    Destructive,
    /// Words until the pointer comes (Reset, Details).
    Quiet,
}

impl Kind {
    pub const ALL: [Kind; 4] = [
        Kind::Primary,
        Kind::Secondary,
        Kind::Destructive,
        Kind::Quiet,
    ];
}

/// The tokens a button paints with (for inspect mode).
pub const BUTTON_TOKENS: &[Tok] = &[
    Tok::Focus,
    Tok::PrimaryInk,
    Tok::Destructive,
    Tok::Menu,
    Tok::Text,
    Tok::TextDim,
    Tok::Well,
    Tok::Toolbar,
    Tok::InnerShadow,
    Tok::Disabled,
    Tok::Dark,
];

/// The tokens the frame paints with.
pub const FRAME_TOKENS: &[Tok] = &[
    Tok::Scrim,
    Tok::Menu,
    Tok::Toolbar,
    Tok::Text,
    Tok::TextDim,
    Tok::GridBar,
    Tok::InnerShadow,
    Tok::Dark,
];

/// A dialog button's height (compact: 24) at 100 %.
pub const HEIGHT: f32 = 28.0;
pub const COMPACT: f32 = 24.0;
/// The footer's height and the title's inset at 100 %.
pub const FOOTER: f32 = 56.0;
pub const INSET: f32 = 20.0;

/// The width a button needs for its words (`busy` makes room for the turning arc).
pub fn width(ctx: &egui::Context, s: &Style, label: Src<'_>, compact: bool, busy: bool) -> f32 {
    let t = Text {
        src: label,
        role: Role::Control,
        caps: false,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: s.t.text,
    };
    let words = text::measure(ctx, &t, s.tk()).x;
    let (pad, min) = if compact { (24.0, 64.0) } else { (32.0, 84.0) };
    (words + s.s(pad) + 2.0).max(s.s(min)) + if busy { s.s(18.0) } else { 0.0 }
}

/// Paint a button of `kind` in `rect`; `busy` turns an arc at `phase` (0…1) before its
/// words.
pub fn button(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    st: State,
    kind: Kind,
    label: Src<'_>,
    busy: Option<f32>,
) {
    b.register(rect, "button", BUTTON_TOKENS, &[]);
    let t = &s.t;
    let night = t.dark;
    let rect = b.snap_rect(rect);
    let r = s.s(4.0);
    let px = b.px();
    let compact = rect.height() < s.s(HEIGHT - 1.0);
    let shadow = |b: &mut Brush, a: f32| {
        let c = if night {
            with_alpha(t.dark_shade, a)
        } else {
            with_alpha(opaque(t.drop), a * 0.5)
        };
        b.rrect(rect.translate(Vec2::new(0.0, px)), r, Fill::Solid(c));
    };
    let ink = if st.disabled {
        let fill = mix(t.toolbar, t.well, 0.6);
        if kind != Kind::Quiet {
            b.rrect(rect, r, Fill::Solid(fill));
        }
        t.disabled
    } else {
        match kind {
            Kind::Primary | Kind::Destructive => {
                let base = if kind == Kind::Primary {
                    t.focus
                } else {
                    t.destructive
                };
                let fill = if busy.is_some() {
                    mix(base, t.toolbar, 0.25)
                } else if st.active {
                    mix(base, t.dark_shade, 0.18)
                } else if st.hovered {
                    mix(base, t.light, 0.12)
                } else {
                    base
                };
                shadow(b, 0.30);
                b.rrect(rect, r, Fill::Solid(fill));
                if !st.active {
                    b.hline(
                        rect.left() + 2.0 * px,
                        rect.right() - 2.0 * px,
                        rect.top() + px * 0.5,
                        px,
                        with_alpha(t.light, 0.22),
                    );
                }
                if kind == Kind::Primary {
                    t.primary_ink
                } else {
                    t.light
                }
            }
            Kind::Secondary => {
                let fill = if st.active {
                    t.well
                } else if night {
                    mix(t.menu, t.text, if st.hovered { 0.14 } else { 0.08 })
                } else if st.hovered {
                    mix(t.light, t.well, 0.06)
                } else {
                    t.light
                };
                shadow(b, 0.35);
                if night {
                    b.rrect(rect, r, Fill::Solid(fill));
                    if !st.active {
                        b.hline(
                            rect.left() + 2.0 * px,
                            rect.right() - 2.0 * px,
                            rect.top() + px * 0.5,
                            px,
                            with_alpha(t.light, 0.10),
                        );
                    }
                } else {
                    b.rrect(rect, r, Fill::Solid(opaque(t.inner_shadow)));
                    b.rrect(rect.shrink(px), (r - px).max(0.0), Fill::Solid(fill));
                }
                t.text
            }
            Kind::Quiet => {
                if st.hovered || st.active {
                    let fill = if st.active {
                        t.well
                    } else {
                        super::cap::well_fill(
                            s,
                            State {
                                hovered: true,
                                ..State::REST
                            },
                        )
                    };
                    b.rrect(rect, r, Fill::Solid(fill));
                    t.text
                } else {
                    t.text_dim
                }
            }
        }
    };
    if st.focused && !st.disabled {
        let g = s.s(2.0);
        b.rrect_stroke(
            rect.expand(g * 0.5 + s.s(1.0)),
            r + g,
            g,
            with_alpha(t.focus, 0.6),
        );
    }
    let k = if compact {
        s.tk() * 11.5 / 12.0
    } else {
        s.tk()
    };
    // The inset the sheet asks for, less where the caller's rect is narrower (a word must
    // stay whole).
    let wants = {
        let probe = Text {
            src: label,
            role: Role::Control,
            caps: false,
            align: Align2::CENTER_CENTER,
            policy: Policy::Elide(Elide::End),
            color: ink,
        };
        text::measure(b.ctx(), &probe, k).x
    };
    let pad = s
        .s(if compact { 12.0 } else { 16.0 })
        .min(((rect.width() - wants) * 0.5 - 1.0).max(1.0));
    let mut words = rect.shrink2(Vec2::new(pad, 0.0));
    if let Some(phase) = busy {
        let c = Pos2::new(rect.left() + s.s(14.0), rect.center().y);
        let rr = s.s(5.0);
        let w = s.s(1.5);
        b.circle_stroke(c, rr, w, with_alpha(ink, 0.3));
        let a0 = phase * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
        b.arc(c, rr, a0, a0 + std::f32::consts::FRAC_PI_2, w, ink, true);
        words.min.x += s.s(18.0);
    }
    let tt = Text {
        src: label,
        role: Role::Control,
        caps: false,
        align: Align2::CENTER_CENTER,
        policy: Policy::Elide(Elide::End),
        color: ink,
    };
    let _ = text::paint(b, words, &tt, k);
}

/// The veil a dialog lays over the window behind it.
pub fn scrim(b: &mut Brush, window: Rect, s: &Style) {
    b.rect(window, Fill::Solid(s.t.scrim));
}

/// Where a frame's parts are.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FrameRects {
    pub title: Rect,
    pub close: Rect,
    pub body: Rect,
    pub footer: Rect,
}

/// The parts of a dialog frame in `rect` (`footer`: whether it has one).
pub fn rects(rect: Rect, s: &Style, footer: bool) -> FrameRects {
    let inset = s.s(INSET);
    let head = s.s(46.0);
    let foot = if footer { s.s(FOOTER) } else { 0.0 };
    let close = Rect::from_center_size(
        Pos2::new(rect.right() - s.s(24.0), rect.top() + s.s(26.0)),
        Vec2::splat(s.s(24.0)),
    );
    FrameRects {
        title: Rect::from_min_max(
            Pos2::new(rect.left() + inset, rect.top()),
            Pos2::new(close.left() - s.s(8.0), rect.top() + head),
        ),
        close,
        body: Rect::from_min_max(
            Pos2::new(rect.left() + inset, rect.top() + head),
            Pos2::new(rect.right() - inset, rect.bottom() - foot),
        ),
        footer: Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - foot), rect.max),
    }
}

/// Paint a dialog's frame in `rect`: the sheet, its title, the close ×, the footer's band
/// (when `footer`). `close_hot`: the pointer is on the ×.
pub fn frame(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    title: Src<'_>,
    footer: bool,
    close_hot: bool,
) -> FrameRects {
    b.register(rect, "dialog", FRAME_TOKENS, &[]);
    let t = &s.t;
    let rect = b.snap_rect(rect);
    let r = s.s(8.0);
    let px = b.px();
    let parts = rects(rect, s, footer);
    b.rrect_soft(
        rect.translate(Vec2::new(0.0, s.s(14.0))),
        r + s.s(8.0),
        Fill::Solid(with_alpha(t.dark_shade, if t.dark { 0.55 } else { 0.20 })),
        s.s(22.0),
    );
    let edge = if t.dark {
        mix(t.menu, t.text, 0.10)
    } else {
        t.grid_bar
    };
    b.rrect(rect, r, Fill::Solid(edge));
    b.rrect(rect.shrink(px), (r - px).max(0.0), Fill::Solid(t.menu));
    if footer {
        let f = parts.footer;
        let band = mix(t.menu, t.toolbar, 0.65);
        b.rect(
            Rect::from_min_max(f.min + Vec2::new(px, 0.0), f.max - Vec2::new(px, r)),
            Fill::Solid(band),
        );
        b.rrect(
            Rect::from_min_max(
                Pos2::new(f.left() + px, f.bottom() - 2.0 * r),
                f.max - Vec2::splat(px),
            ),
            (r - px).max(0.0),
            Fill::Solid(band),
        );
        b.hline(
            f.left() + px,
            f.right() - px,
            f.top() + px * 0.5,
            px,
            mix(t.menu, opaque(t.inner_shadow), 0.4),
        );
    }
    let tt = Text {
        src: title,
        role: Role::Title,
        caps: false,
        align: Align2::LEFT_CENTER,
        policy: Policy::Elide(Elide::End),
        color: t.text,
    };
    let title_rect = Rect::from_min_max(
        parts.title.min + Vec2::new(0.0, s.s(14.0)),
        Pos2::new(parts.title.right(), rect.top() + s.s(38.0)),
    );
    let _ = text::paint(b, title_rect, &tt, s.tk() * 15.0 / 13.0);
    // The close ×: quiet until hovered.
    let c = parts.close.center();
    if close_hot {
        b.rrect(
            parts.close,
            s.s(4.0),
            Fill::Solid(mix(t.menu, t.text, 0.10)),
        );
    }
    let k = s.s(4.0);
    let w = s.s(1.5);
    let ink = if close_hot { t.text } else { t.text_dim };
    b.segment(c - Vec2::splat(k), c + Vec2::splat(k), w, ink, true);
    b.segment(c + Vec2::new(-k, k), c + Vec2::new(k, -k), w, ink, true);
    parts
}

/// A keycap ("Esc", "Ctrl"): an outline 15 pt high, corners 3, SemiBold 9.5 in ink 2; its
/// width, which the caller lays the next words after.
pub fn keycap(b: &mut Brush, at: Pos2, s: &Style, key: &str) -> f32 {
    let t = &s.t;
    let k = s.tk() * 9.5 / 11.0;
    let tt = Text::new(key, Role::Section, t.text_dim).align(Align2::CENTER_CENTER);
    let w = text::measure(b.ctx(), &tt, k).x + s.s(8.0);
    let rect = b.snap_rect(Rect::from_min_size(
        Pos2::new(at.x, at.y - s.s(7.5)),
        Vec2::new(w, s.s(15.0)),
    ));
    b.rrect_stroke(
        rect.shrink(b.px() * 0.5),
        s.s(3.0),
        b.px(),
        mix(t.menu, t.placeholder, 0.5),
    );
    let _ = text::paint(b, rect, &tt, k);
    w
}

// ---- the dialog as a window (round 39, `el-controls`) -------------------------------------
//
// Every dialog view opens its window through [`window`], says its title with [`title`] and
// ends with its buttons through [`footer`] and [`button_ui`]: in a recessed family the frame,
// the scrim, the title row and the footer band are the sheet's; in Classic the window keeps
// egui's own title bar and frame, as before.

/// The frame a dialog's window wears in a recessed family: the sheet (`menu`), corners 8, a
/// 1-px edge, one deep shadow (0 14 44); 20 pt in, 14 at the foot (the footer's band fills
/// it to the edges).
pub fn window_frame(s: &Style) -> egui::Frame {
    let t = &s.t;
    let edge = if t.dark {
        mix(t.menu, t.text, 0.10)
    } else {
        t.grid_bar
    };
    egui::Frame::NONE
        .fill(t.menu)
        .stroke(egui::Stroke::new(1.0, edge))
        .corner_radius(egui::CornerRadius::same(8))
        .inner_margin(egui::Margin {
            left: 20,
            right: 20,
            top: 0,
            bottom: 14,
        })
        .shadow(egui::Shadow {
            offset: [0, 14],
            blur: 44,
            spread: 0,
            color: with_alpha(t.dark_shade, if t.dark { 0.55 } else { 0.20 }),
        })
}

/// A dialog's window titled `title`: in a recessed family the kit's frame, no egui title
/// bar (the content starts with [`title`]), in front of the window's other floating parts.
pub fn window(title: impl Into<egui::WidgetText>) -> egui::Window<'static> {
    let s = Style::current();
    let w = egui::Window::new(title);
    if !s.t.system.recessed() {
        return w;
    }
    w.title_bar(false)
        .frame(window_frame(&s))
        .order(egui::Order::Foreground)
}

/// Lay the scrim over the whole window, under the dialogs (a layer of its own behind the
/// windows in front).
pub fn scrim_layer(ctx: &egui::Context) {
    let s = Style::current();
    let screen = ctx.content_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("cantus-dialog-scrim"),
    ));
    painter.rect_filled(screen, 0.0, s.t.scrim);
}

/// A dialog's title row, first in its window: the title Medium 15 at the frame's inset and a
/// close key at its right; true when it was clicked (the caller cancels). In Classic the
/// window's own title bar says it and this draws nothing. Lays the scrim too.
pub fn title(ui: &mut egui::Ui, words: &str) -> bool {
    title_row(ui, words, true)
}

/// [`title`] without a close key: a dialog that must be answered (the recovery's).
pub fn heading(ui: &mut egui::Ui, words: &str) -> bool {
    title_row(ui, words, false)
}

fn title_row(ui: &mut egui::Ui, words: &str, closable: bool) -> bool {
    let s = Style::current();
    if !s.t.system.recessed() {
        return false;
    }
    scrim_layer(ui.ctx());
    let tt = Text::new(words, Role::Title, s.t.text).align(Align2::LEFT_CENTER);
    let k = s.tk() * 15.0 / 13.0;
    let wide = text::measure(ui.ctx(), &tt, k).x + s.s(8.0 + 24.0 + 8.0);
    let w = ui.available_width().max(wide);
    let (row, _) = ui.allocate_exact_size(Vec2::new(w, s.s(46.0)), egui::Sense::hover());
    let close = Rect::from_center_size(
        Pos2::new(row.right() - s.s(8.0), row.top() + s.s(26.0)),
        Vec2::splat(s.s(24.0)),
    );
    let resp = ui.interact(
        close,
        ui.id().with("cantus-dialog-close"),
        if closable {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let resp = if closable {
        crate::views::hint::said_with(resp, || crate::i18n::t("kit.dialog.close"))
    } else {
        resp
    };
    let hot = closable && resp.hovered();
    super::with_brush(ui.painter(), |b| {
        b.register(row, "dialog-title", FRAME_TOKENS, &[]);
        let tr = Rect::from_min_max(
            Pos2::new(row.left(), row.top() + s.s(14.0)),
            Pos2::new(close.left() - s.s(8.0), row.top() + s.s(38.0)),
        );
        let _ = text::paint(b, tr, &tt, k);
        if !closable {
            return;
        }
        if hot {
            b.rrect(close, s.s(4.0), Fill::Solid(mix(s.t.menu, s.t.text, 0.10)));
        }
        let c = close.center();
        let (a, w) = (s.s(4.0), s.s(1.5));
        let ink = if hot { s.t.text } else { s.t.text_dim };
        b.segment(c - Vec2::splat(a), c + Vec2::splat(a), w, ink, true);
        b.segment(c + Vec2::new(-a, a), c + Vec2::new(a, -a), w, ink, true);
    });
    closable && resp.clicked()
}

/// A compact dialog button (24 pt, at least 64 wide), its response: rows of a list
/// (a shortcut's key, *Add*), where a dialog's 28-pt button would be too large.
pub fn compact_ui(ui: &mut egui::Ui, label: &str, kind: Kind) -> egui::Response {
    let s = Style::current();
    let w = width(ui.ctx(), &s, Src::Str(label), true, false);
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(COMPACT)), egui::Sense::click());
    let resp = crate::views::hint::said_with(resp, || label.to_string());
    let st = State {
        focused: resp.has_focus(),
        ..State::of(&resp, false)
    };
    super::with_brush(ui.painter(), |b| {
        button(b, r, &s, st, kind, Src::Str(label), None)
    });
    resp
}

/// A dialog's button: as wide as its words (at least 84 pt), 28 high; true when clicked.
pub fn button_ui(ui: &mut egui::Ui, label: &str, kind: Kind, enabled: bool) -> bool {
    let s = Style::current();
    let w = width(ui.ctx(), &s, Src::Str(label), false, false);
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (r, resp) = ui.allocate_exact_size(Vec2::new(w, s.s(HEIGHT)), sense);
    let resp = crate::views::hint::said_with(resp, || label.to_string());
    let st = State {
        disabled: !enabled,
        focused: resp.has_focus(),
        ..State::of(&resp, false)
    };
    super::with_brush(ui.painter(), |b| {
        button(b, r, &s, st, kind, Src::Str(label), None)
    });
    enabled && resp.clicked()
}

/// A dialog's footer: in a recessed family a band a step toward the bar's colour under a
/// 1-px line, to the window's edges, with the buttons `add` lays right to left (the first
/// added at the right end: the primary) and "Esc cancels" at its left where there is room.
pub fn footer<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let s = Style::current();
    if !s.t.system.recessed() {
        ui.add_space(8.0);
        return ui
            .allocate_ui_with_layout(
                Vec2::new(ui.available_width(), s.s(26.0)),
                egui::Layout::right_to_left(egui::Align::Center),
                add,
            )
            .inner;
    }
    ui.add_space(s.s(28.0));
    let painter = ui.ctx().layer_painter(ui.layer_id());
    let band = painter.add(egui::Shape::Noop);
    let line = painter.add(egui::Shape::Noop);
    let inner = ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), s.s(HEIGHT)),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing.x = s.s(10.0);
            add(ui)
        },
    );
    let row = inner.response.rect;
    let used = ui.min_rect();
    let t = &s.t;
    let full = Rect::from_min_max(
        Pos2::new(used.left() - 20.0, row.top() - s.s(14.0)),
        Pos2::new(used.right().max(row.right()) + 20.0, row.bottom() + 14.0),
    );
    let fill = mix(t.menu, t.toolbar, 0.65);
    painter.set(
        band,
        egui::epaint::RectShape::filled(
            full.shrink2(Vec2::new(1.0, 0.0)),
            egui::CornerRadius {
                nw: 0,
                ne: 0,
                sw: 7,
                se: 7,
            },
            fill,
        ),
    );
    painter.set(
        line,
        egui::Shape::hline(
            full.x_range(),
            full.top() + 0.5,
            egui::Stroke::new(1.0, mix(t.menu, opaque(t.inner_shadow), 0.4)),
        ),
    );
    // "Esc cancels" at its left, where the buttons leave room.
    let words = crate::i18n::t("kit.dialog.esc_cancels");
    let room = row.left() - used.left();
    if room > s.s(120.0) {
        super::with_brush(ui.painter(), |b| {
            let at = Pos2::new(used.left(), row.center().y);
            let tt = Text::new(&words, Role::Label, t.placeholder).align(Align2::LEFT_CENTER);
            let kw = keycap(b, at, &s, "Esc");
            let r = Rect::from_min_max(
                Pos2::new(at.x + kw + s.s(6.0), row.top()),
                Pos2::new(row.left() - s.s(8.0), row.bottom()),
            );
            if text::measure(b.ctx(), &tt, s.tk()).x <= r.width() {
                let _ = text::paint(b, r, &tt, s.tk());
            }
        });
    }
    inner.inner
}

/// A floating dialog's face in `rect` (the views that lay their dialog out themselves, in an
/// `Area`): the kit's sheet in a recessed family, the workspace's raised panel in Classic.
pub fn sheet(ui: &egui::Ui, rect: Rect, s: &Style) {
    if s.t.system.recessed() {
        scrim_layer(ui.ctx());
    }
    face(ui, rect, s);
}

/// [`sheet`] without the scrim — for a dialog that lays its own veil (one that also takes
/// the clicks behind it: Settings, the first run, export). The one dialog face of a
/// recessed family (integration of round 39: `el-topbar`'s frame and this one became one;
/// `workspace_settings::frame` paints this).
pub fn face(ui: &egui::Ui, rect: Rect, s: &Style) {
    if !s.t.system.recessed() {
        crate::views::workspace_settings::frame_classic(ui, rect, s);
        return;
    }
    super::with_brush(ui.painter(), |b| {
        b.register(rect, "dialog", FRAME_TOKENS, &[]);
        let t = &s.t;
        let rect = b.snap_rect(rect);
        let r = s.s(8.0);
        b.rrect_soft(
            rect.translate(Vec2::new(0.0, s.s(14.0))),
            r + s.s(8.0),
            Fill::Solid(with_alpha(t.dark_shade, if t.dark { 0.55 } else { 0.20 })),
            s.s(22.0),
        );
        let edge = if t.dark {
            mix(t.menu, t.text, 0.10)
        } else {
            t.grid_bar
        };
        b.rrect(rect, r, Fill::Solid(edge));
        b.rrect(
            rect.shrink(b.px()),
            (r - b.px()).max(0.0),
            Fill::Solid(t.menu),
        );
    });
}

/// Such a dialog's title row in `rect` (the title, the close key); true when the close key
/// was clicked. Classic keeps the workspace's row with its glyph.
pub fn head(ui: &mut egui::Ui, rect: Rect, s: &Style, g: super::glyph::Glyph, key: &str) -> bool {
    if !s.t.system.recessed() {
        return crate::views::workspace_settings::title_classic(ui, rect, s, g, key);
    }
    let close = Rect::from_center_size(
        Pos2::new(rect.right() - s.s(24.0), rect.center().y),
        Vec2::splat(s.s(24.0)),
    );
    let resp = ui.interact(
        close,
        ui.id().with(("cantus-dialog-head", key)),
        egui::Sense::click(),
    );
    let resp = crate::views::hint::said_with(resp, || crate::i18n::t("kit.dialog.close"));
    let hot = resp.hovered();
    super::with_brush(ui.painter(), |b| {
        b.register(rect, "dialog-title", FRAME_TOKENS, &[]);
        let tr = Rect::from_min_max(
            Pos2::new(rect.left() + s.s(INSET), rect.top()),
            Pos2::new(close.left() - s.s(8.0), rect.bottom()),
        );
        let tt = Text::key(key, Role::Title, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, tr, &tt, s.tk() * 15.0 / 13.0);
        if hot {
            b.rrect(close, s.s(4.0), Fill::Solid(mix(s.t.menu, s.t.text, 0.10)));
        }
        let c = close.center();
        let (a, w) = (s.s(4.0), s.s(1.5));
        let ink = if hot { s.t.text } else { s.t.text_dim };
        b.segment(c - Vec2::splat(a), c + Vec2::splat(a), w, ink, true);
        b.segment(c + Vec2::new(-a, a), c + Vec2::new(a, -a), w, ink, true);
    });
    resp.clicked()
}
