//! **A window's bar** (ADR 0148 §4): at the top of a window of its own — its menu (⋯) and
//! name at the left, then the top bar's own groups (the transport, the position, the tempo,
//! the master meter: the same components, `transport::draw_group`), and *Back into the
//! main window* at the right. Its menu: the bar, always on top, full screen, back, close.

use crate::app::CantusApp;
use crate::kit::glyph::{self, Glyph};
use crate::kit::menu::{Check, Item};
use crate::kit::text::{self as ktext, Role, Src, Text};
use crate::kit::{with_brush, Fill, Style};
use crate::views::transport::layout::Group;
use egui::{pos2, vec2, Align2, Rect, Sense, Ui};

/// The groups a window's bar shows.
pub const GROUPS: [Group; 4] = [
    Group::Transport,
    Group::Position,
    Group::Tempo,
    Group::Meter,
];

/// A key of the bar: a well with a glyph, lit while hovered or `lit`.
fn key(ui: &mut Ui, r: Rect, g: Glyph, lit: bool, id: egui::Id) -> egui::Response {
    let s = Style::current();
    let resp = ui.interact(r, id, Sense::click());
    let on = lit || resp.hovered();
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(r);
        let fill = if on { s.t.selection } else { s.t.well };
        b.rrect(r, 4.0 * s.k, Fill::Solid(fill));
        let side = (r.height() * 0.5).round();
        glyph::paint(
            b,
            Rect::from_center_size(r.center(), vec2(side, side)),
            g,
            s.t.text,
        );
    });
    resp
}

/// Draw window `i`'s bar in `ui` (its pass; its dock is `app.workspace`).
pub fn show(app: &mut CantusApp, ui: &mut Ui, i: usize) {
    let s = Style::current();
    let k = s.k;
    let rect = ui.max_rect();
    ui.allocate_rect(rect, Sense::hover());
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(rect);
        b.rect(r, Fill::Solid(s.t.toolbar));
        b.hline(
            r.left(),
            r.right(),
            r.bottom() - b.px() * 0.5,
            b.px(),
            s.t.ink,
        );
    });
    let window_key = app.windows.list[i].key;
    let pad = 8.0 * k;
    let side = (rect.height() - 12.0 * k).round();
    let menu_r = Rect::from_min_size(
        pos2(rect.left() + pad, rect.center().y - side * 0.5),
        vec2(side, side),
    );
    let open = app.windows.menu.is_some_and(|(w, _)| w == window_key);
    let id = egui::Id::new(("cantus-window-menu-key", window_key));
    if key(ui, menu_r, Glyph::More, open, id).clicked() {
        app.windows.menu = if open {
            None
        } else {
            Some((window_key, menu_r.left_bottom() + vec2(0.0, 6.0 * k)))
        };
    }
    // Its first area's glyph and the window's name.
    let first = app.workspace.panes().first().copied();
    let gl = Rect::from_center_size(
        pos2(menu_r.right() + 16.0 * k, rect.center().y),
        vec2(16.0 * k, 16.0 * k),
    );
    if let Some(p) = first {
        with_brush(ui.painter(), |b| {
            glyph::paint(b, gl, p.glyph(), s.t.highlight)
        });
    }
    let name = app.window_name(i);
    let name_w = 140.0 * k;
    let name_r = Rect::from_min_max(
        pos2(gl.right() + 6.0 * k, rect.top()),
        pos2(gl.right() + 6.0 * k + name_w, rect.bottom()),
    );
    with_brush(ui.painter(), |b| {
        let text = Text::new(&name, Role::Title, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(ktext::Policy::Elide(ktext::Elide::End));
        let _ = ktext::paint(b, name_r, &text, s.tk());
    });
    // Back into the main window, at the right.
    let back = Rect::from_min_size(
        pos2(rect.right() - pad - side, rect.center().y - side * 0.5),
        vec2(side, side),
    );
    let bid = egui::Id::new(("cantus-window-back", window_key));
    let resp = key(ui, back, Glyph::Restore, false, bid);
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &crate::i18n::t("screens.window.back"),
            "",
            crate::kit::hint::Caps::NONE,
        );
    }
    if resp.clicked() {
        back_into_main(app, ui.ctx());
    }
    // The top bar's own groups, as far as there is room.
    let ctx = ui.ctx().clone();
    let mut x = name_r.right() + 12.0 * k;
    let groups = app.windows.list[i].groups.clone();
    for g in groups {
        let w = crate::views::transport::group_width(g, app, &ctx);
        if w <= 0.0 || x + w > back.left() - 8.0 * k {
            break;
        }
        let r = Rect::from_min_max(pos2(x, rect.top()), pos2(x + w, rect.bottom()));
        crate::views::transport::draw_group(g, app, ui, r);
        x += w + 10.0 * k;
    }
}

/// Every area of the window drawn now back into the main window (it closes).
fn back_into_main(app: &mut CantusApp, ctx: &egui::Context) {
    for p in app.workspace.panes() {
        app.move_pane(ctx, p, crate::windows::Target::Main);
    }
}

/// What a row of the window's menu does.
#[derive(Copy, Clone, PartialEq, Eq)]
enum Row {
    Bar,
    Groups,
    Rename,
    OnTop,
    FullScreen,
    Back,
    Close,
}

/// Window `i`'s menu, when its ⋯ key opened it (its pass).
pub fn menu(app: &mut CantusApp, ctx: &egui::Context, i: usize) {
    let window_key = app.windows.list[i].key;
    let Some((wk, at)) = app.windows.menu else {
        return;
    };
    if wk != window_key {
        return;
    }
    let w = &app.windows.list[i];
    let check = |on: bool| if on { Check::On } else { Check::Off };
    let rows: [(Item<'_>, Row); 8] = [
        (
            with(
                Item::action(Src::Key("screens.window.bar")),
                check(w.bar),
                None,
            ),
            Row::Bar,
        ),
        (
            Item::action(Src::Key("screens.window.bar_groups")),
            Row::Groups,
        ),
        (Item::action(Src::Key("screens.window.rename")), Row::Rename),
        (
            with(
                Item::action(Src::Key("screens.window.on_top")),
                check(w.on_top),
                None,
            ),
            Row::OnTop,
        ),
        (
            with(
                Item::action(Src::Key("screens.window.full_screen")),
                Check::None,
                Some("F11"),
            ),
            Row::FullScreen,
        ),
        (Item::Separator, Row::Bar),
        (Item::action(Src::Key("screens.window.back")), Row::Back),
        (Item::action(Src::Key("screens.window.close")), Row::Close),
    ];
    let items: Vec<Item<'_>> = rows.iter().map(|r| r.0).collect();
    let s = Style::current();
    let width = crate::kit::menu::width(ctx, &items, &s);
    let mut chosen = None;
    let area = egui::Area::new(egui::Id::new(("cantus-window-menu", window_key)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            let (_, c) = crate::kit::widgets::menu_list(ui, &items, width);
            chosen = c.clicked;
        });
    let outside = ctx.input(|inp| inp.pointer.any_pressed())
        && !ctx
            .pointer_interact_pos()
            .is_some_and(|p| area.response.rect.contains(p) || p.y < at.y);
    let escape = ctx.input(|inp| inp.key_pressed(egui::Key::Escape));
    if let Some(r) = chosen.map(|c| rows[c].1) {
        let w = &mut app.windows.list[i];
        match r {
            Row::Bar => w.bar = !w.bar,
            Row::Groups => {
                w.bar = true;
                app.windows.groups_menu = Some((window_key, at));
            }
            Row::Rename => {
                let name = app.window_name(i);
                app.windows.renaming = Some((window_key, name));
            }
            Row::OnTop => w.on_top = !w.on_top,
            Row::FullScreen => {
                let full = ctx.input(|inp| inp.viewport().fullscreen.unwrap_or(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
            }
            Row::Back => back_into_main(app, ctx),
            Row::Close => back_into_main(app, ctx),
        }
    }
    if chosen.is_some() || outside || escape {
        app.windows.menu = None;
    }
}

fn with<'a>(mut i: Item<'a>, c: Check, key: Option<&'a str>) -> Item<'a> {
    if let Item::Action {
        check, shortcut, ..
    } = &mut i
    {
        *check = c;
        *shortcut = key;
    }
    i
}

/// The groups a window's bar can show, in the top bar's order.
pub const OFFERED: [Group; 8] = [
    Group::Transport,
    Group::Position,
    Group::Tempo,
    Group::Metronome,
    Group::Snap,
    Group::Undo,
    Group::Meter,
    Group::Performance,
];

/// *Choose what the bar shows…*: the groups, each checked when the bar shows it; a click
/// switches one and keeps the list open (Esc or a click outside closes it). The window's
/// setting, kept with the layout; no undo, since it is not the project.
pub fn groups_menu(app: &mut CantusApp, ctx: &egui::Context, i: usize) {
    let window_key = app.windows.list[i].key;
    let Some((wk, at)) = app.windows.groups_menu else {
        return;
    };
    if wk != window_key {
        return;
    }
    let shown = app.windows.list[i].groups.clone();
    let items: Vec<Item<'_>> = OFFERED
        .iter()
        .map(|g| {
            let on = if shown.contains(g) {
                Check::On
            } else {
                Check::Off
            };
            with(Item::action(Src::Key(g.name_key())), on, None)
        })
        .collect();
    let s = Style::current();
    let width = crate::kit::menu::width(ctx, &items, &s);
    let mut chosen = None;
    let area = egui::Area::new(egui::Id::new(("cantus-window-groups", window_key)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            let (_, c) = crate::kit::widgets::menu_list(ui, &items, width);
            chosen = c.clicked.or(c.toggled);
        });
    if let Some(c) = chosen {
        let g = OFFERED[c];
        let w = &mut app.windows.list[i];
        if w.groups.contains(&g) {
            w.groups.retain(|x| *x != g);
        } else {
            // In the top bar's order.
            w.groups.push(g);
            w.groups
                .sort_by_key(|x| OFFERED.iter().position(|o| o == x));
        }
        return;
    }
    let outside = ctx.input(|inp| inp.pointer.any_pressed())
        && !ctx
            .pointer_interact_pos()
            .is_some_and(|p| area.response.rect.contains(p));
    let escape = ctx.input(|inp| inp.key_pressed(egui::Key::Escape));
    if outside || escape {
        app.windows.groups_menu = None;
    }
}

/// *Rename the window…*: the kit's dialog over the window with the name's field (Enter
/// keeps it, Esc leaves; an empty name gives the window its areas' name back). The window's
/// setting, kept with the layout; no undo, since it is not the project.
pub fn rename_dialog(app: &mut CantusApp, ctx: &egui::Context, i: usize) {
    let window_key = app.windows.list[i].key;
    let Some((wk, mut name)) = app.windows.renaming.take() else {
        return;
    };
    if wk != window_key {
        app.windows.renaming = Some((wk, name));
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = vec2(360.0 * k, 150.0 * k).min(screen.size() - vec2(16.0, 16.0));
    let rect = Rect::from_center_size(screen.center(), size);
    let (mut save, mut close) = (false, false);
    egui::Area::new(egui::Id::new(("cantus-window-rename", window_key)))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            crate::views::audio_parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), 32.0 * k));
            let key = "screens.window.rename_title";
            if crate::kit::dialog::head(ui, head, &s, Glyph::Inspector, key) {
                close = true;
            }
            let x0 = rect.left() + 20.0 * k;
            let x1 = rect.right() - 20.0 * k;
            let well = Rect::from_min_max(
                pos2(x0, head.bottom() + 20.0 * k),
                pos2(x1, head.bottom() + 20.0 * k + s.d(24.0)),
            );
            let hint = crate::i18n::t("screens.window.name_hint");
            let out = crate::kit::field::Field::new(egui::Id::new(("window-name", window_key)))
                .hint(&hint)
                .opens_when_shown()
                .show(ui, well, &mut name);
            match out.event {
                Some(crate::kit::field::Event::Commit) => save = out.enter,
                Some(crate::kit::field::Event::Cancel) => close = true,
                None => {}
            }
            let buttons = Rect::from_min_max(
                pos2(x0, rect.bottom() - 44.0 * k),
                pos2(x1, rect.bottom() - 14.0 * k),
            );
            let mut row = ui.new_child(
                egui::UiBuilder::new()
                    .max_rect(buttons)
                    .layout(egui::Layout::right_to_left(egui::Align::Center)),
            );
            let label = crate::i18n::t("layout.save");
            if crate::views::audio_parts::button(&mut row, &label, true, true, 90.0).clicked() {
                save = true;
            }
            row.add_space(8.0 * k);
            let label = crate::i18n::t("layout.cancel");
            if crate::views::audio_parts::button(&mut row, &label, false, true, 90.0).clicked() {
                close = true;
            }
        });
    if save {
        app.windows.list[i].name = name.trim().to_string();
    } else if !close {
        app.windows.renaming = Some((wk, name));
    }
}
