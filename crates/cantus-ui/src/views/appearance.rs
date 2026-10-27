//! The appearance page (ADR 0024 §2, the user's "as configurable as possible"):
//! everything about the look, applied live and undone cleanly.
//!
//! - **Look**: the mode — Dark, Light, or Follow the system — and the base theme of each
//!   mode; FL's theme knobs (Hue, Saturation, Brightness, Contrast, Text); the form (Depth,
//!   Roundness, Density, Text size); the role pickers (selection, highlight, LEDs, meters,
//!   text…).
//! - **Colours**: all 61 named tokens, searchable, grouped, each with its reset; reset per
//!   group; and "Pick an element": click anything in the interface to list the tokens it is
//!   painted with, ready to recolour.
//! - **Themes and logo**: save, duplicate, delete, share (export) and import themes; A/B
//!   compare; the logo (1–10) and its tint (Original / Theme).
//!
//! Every change goes through [`change`]: it applies at once, pushes one undo step per
//! gesture (a knob dragged is one step, not a hundred), and is written to
//! `appearance.json` when the gesture ends. The page edits the mode that is showing.

use super::appearance_panels as panels;
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::kit::appearance::{Appearance, Mode};
use crate::kit::glyph::Glyph;
use crate::kit::inspect::{self, Item};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{with_brush, Style};
use egui::{Align2, Context, Id, LayerId, Order, Pos2, Rect, Sense, Vec2};

/// The page's tabs.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Look,
    Colours,
    Themes,
}

/// The page's own state.
#[derive(Default)]
pub struct Page {
    pub tab: Tab,
    /// "Pick an element" is on: the next click selects what is under the pointer.
    pub picking: bool,
    /// The element picked last: the colours list shows its tokens.
    pub picked: Option<Item>,
    /// The system's dark mode as last seen (for Follow the system).
    pub system_dark: bool,
    pub search: String,
    /// The name being typed for "Save as".
    pub name: String,
    /// The token whose colour picker is open, and where.
    pub editing: Option<(crate::kit::Tok, Pos2)>,
    /// A base theme menu is open for this mode.
    pub base_menu: Option<(Mode, Pos2)>,
    /// A message from the last theme operation (import, export), and whether it failed.
    pub message: Option<(String, bool)>,
    undo: Vec<Appearance>,
    redo: Vec<Appearance>,
    /// A change is in progress (a knob held): it is one undo step.
    gesture: bool,
    /// Changed since the file was last written.
    dirty: bool,
    /// The logo the splash's cache holds (and the theme generation, for a tinted one).
    pub remembered: Option<(crate::kit::logo::LogoChoice, u64)>,
    /// The icon's colour picker is open, here (round 25).
    pub icon_picker: Option<Pos2>,
    /// The shipped themes' tokens, worked out once for their previews.
    pub previews: Vec<(String, crate::kit::Tokens)>,
}

/// How many steps of the look the page can undo.
const UNDO: usize = 64;

/// Change the look: applied now, one undo step per gesture, written when it ends.
pub fn change(app: &mut CantusApp, f: impl FnOnce(&mut Appearance)) {
    let before = app.appearance.clone();
    f(&mut app.appearance);
    if app.appearance == before {
        return;
    }
    let page = &mut app.appearance_page;
    if !page.gesture {
        page.undo.push(before);
        if page.undo.len() > UNDO {
            page.undo.remove(0);
        }
        page.redo.clear();
        page.gesture = true;
    }
    page.dirty = true;
    app.apply_appearance();
}

/// Undo the last change of the look.
pub fn undo(app: &mut CantusApp) {
    if let Some(prev) = app.appearance_page.undo.pop() {
        let now = std::mem::replace(&mut app.appearance, prev);
        app.appearance_page.redo.push(now);
        app.appearance_page.dirty = true;
        app.apply_appearance();
    }
}

/// Redo what was undone.
pub fn redo(app: &mut CantusApp) {
    if let Some(next) = app.appearance_page.redo.pop() {
        let now = std::mem::replace(&mut app.appearance, next);
        app.appearance_page.undo.push(now);
        app.appearance_page.dirty = true;
        app.apply_appearance();
    }
}

impl Page {
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

/// The mode the page edits: the one showing.
pub fn editing_mode(app: &CantusApp) -> Mode {
    app.appearance.effective(app.appearance_page.system_dark)
}

/// The appearance page, as a floating window over the workspace, drawn in the kit.
pub fn window(app: &mut CantusApp, ctx: &Context) {
    let mut open = app.show_theme_editor;
    let s = Style::current();
    let screen = ctx.content_rect();
    let size = Vec2::new(600.0, 680.0).min(screen.size() - Vec2::new(40.0, 100.0));
    egui::Window::new("cantus-appearance")
        .id(Id::new("cantus-appearance"))
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .resizable(true)
        .default_size(size)
        .min_size(Vec2::new(440.0, 360.0).min(size))
        .default_pos(Pos2::new(
            screen.right() - size.x - 24.0,
            screen.top() + 60.0,
        ))
        .constrain(true)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            // The window claims what it paints: its contents sit in child UIs, which
            // leave the window's own area empty, and egui hands the wheel only to the layer
            // whose area is under the pointer (the owner, 2026-10-22: Settings would not
            // scroll to its Plug-ins).
            ui.expand_to_include_rect(rect);
            with_brush(ui.painter(), |b| {
                // A floating surface: a soft shadow, the raised panel.
                b.rrect_soft(
                    rect.translate(Vec2::new(0.0, 4.0)),
                    s.r(6.0) + 4.0,
                    crate::kit::Fill::Solid(crate::kit::color::with_alpha(s.t.shadow, 0.9)),
                    16.0,
                );
                light::raised(
                    b,
                    rect,
                    s.r(6.0),
                    s.t.panel,
                    RaisedOpts {
                        drop: false,
                        k: s.k,
                        depth: s.t.depth,
                        ..Default::default()
                    },
                );
            });
            let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0 * s.k));
            if header_ui(app, ui, header) {
                open = false;
            }
            let tabs = Rect::from_min_size(
                Pos2::new(rect.left() + 12.0 * s.k, header.bottom() + 8.0 * s.k),
                Vec2::new(rect.width() - 24.0 * s.k, 24.0 * s.k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(tabs));
            let mut sel = app.appearance_page.tab as usize;
            let labels = [
                Src::Key("workspace.appearance.tab.look"),
                Src::Key("workspace.appearance.tab.colours"),
                Src::Key("workspace.appearance.tab.themes"),
            ];
            if crate::kit::widgets::segmented(&mut child, &mut sel, &labels, "", tabs.width() / s.k)
                .changed()
            {
                app.appearance_page.tab = [Tab::Look, Tab::Colours, Tab::Themes][sel];
            }
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 12.0 * s.k, tabs.bottom() + 8.0 * s.k),
                Pos2::new(rect.right() - 6.0 * s.k, rect.bottom() - 10.0 * s.k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .id_salt("cantus-appearance-body")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - 14.0 * s.k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * s.k, 6.0 * s.k);
                    match app.appearance_page.tab {
                        Tab::Look => panels::look(app, ui),
                        Tab::Colours => panels::colours(app, ui),
                        Tab::Themes => panels::themes(app, ui),
                    }
                });
        });
    panels::popups(app, ctx);
    super::appearance_look::popups(app, ctx);
    // The end of a gesture: one undo step is closed, and the file is written.
    if !ctx.input(|i| i.pointer.any_down()) {
        app.appearance_page.gesture = false;
        if app.appearance_page.dirty {
            app.appearance_page.dirty = false;
            app.store_appearance();
        }
    }
    // Ctrl+Z over the page undoes the look, not the project.
    app.show_theme_editor = open;
}

/// The page's title bar: its name, A/B, undo and redo, the eyedropper, close. Returns
/// whether close was clicked.
fn header_ui(app: &mut CantusApp, ui: &mut egui::Ui, rect: Rect) -> bool {
    let s = Style::current();
    let k = s.k;
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(rect);
        b.hline(
            r.left() + 1.0,
            r.right() - 1.0,
            r.bottom() - b.px() * 0.5,
            b.px(),
            crate::kit::color::shade(s.t.panel, -0.16),
        );
        let g = Rect::from_center_size(
            Pos2::new(r.left() + 18.0 * k, r.center().y),
            Vec2::splat(14.0 * k),
        );
        crate::kit::glyph::paint(b, g, Glyph::Gear, s.t.text_dim);
        let tr = Rect::from_min_max(
            Pos2::new(g.right() + 8.0 * k, r.top()),
            Pos2::new(r.center().x, r.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &Text::key("settings.appearance", Role::Title, s.t.text)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End)),
            s.tk(),
        );
    });
    let keys = Rect::from_min_max(
        Pos2::new(rect.center().x, rect.top() + 4.0 * k),
        Pos2::new(rect.right() - 8.0 * k, rect.bottom() - 4.0 * k),
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(keys)
            .layout(egui::Layout::right_to_left(egui::Align::Center)),
    );
    let mut close = false;
    child.spacing_mut().item_spacing.x = 4.0 * k;
    if w::glyph_key(
        &mut child,
        Glyph::Close,
        false,
        22.0,
        &crate::i18n::t("workspace.close"),
    )
    .clicked()
    {
        close = true;
    }
    let picking = app.appearance_page.picking;
    if w::glyph_key(
        &mut child,
        Glyph::Pick,
        picking,
        22.0,
        &crate::i18n::t("theme.inspect.tip"),
    )
    .clicked()
    {
        app.toggle_pick();
    }
    let can_redo = app.appearance_page.can_redo();
    let can_undo = app.appearance_page.can_undo();
    if w::glyph_key(
        &mut child,
        Glyph::Redo,
        false,
        22.0,
        &crate::i18n::t("workspace.appearance.redo"),
    )
    .clicked()
        && can_redo
    {
        redo(app);
    }
    if w::glyph_key(
        &mut child,
        Glyph::Undo,
        false,
        22.0,
        &crate::i18n::t("workspace.appearance.undo"),
    )
    .clicked()
        && can_undo
    {
        undo(app);
    }
    // A|B: comparing shows which of the two is on screen; a click swaps.
    let comparing = app.appearance.compare.as_ref().map(|c| c.showing_b);
    match comparing {
        Some(b_showing) => {
            let label = if b_showing {
                "shell.ab.b"
            } else {
                "shell.ab.a"
            };
            if w::button(&mut child, Src::Key("workspace.appearance.keep"), 0.0, true).clicked() {
                change(app, |a| a.end_compare());
            }
            if w::button(&mut child, Src::Key(label), 30.0, true)
                .on_hover_text(crate::i18n::t("shell.ab.tip"))
                .clicked()
            {
                change(app, |a| a.toggle_compare());
            }
        }
        None => {
            if w::button(&mut child, Src::Key("theme.compare"), 0.0, true).clicked() {
                let mode = editing_mode(app);
                change(app, |a| a.start_compare(mode));
            }
        }
    }
    close
}

/// "Pick an element": while it is on, a click anywhere selects the element under the
/// pointer and opens its colours; right-click or Esc leaves.
pub fn pick(app: &mut CantusApp, ctx: &Context) {
    if !app.appearance_page.picking {
        return;
    }
    let screen = ctx.content_rect();
    let resp = egui::Area::new(Id::new("cantus-pick"))
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .show(ctx, |ui| ui.allocate_rect(screen, Sense::click()))
        .inner;
    ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
    let hover = ctx.pointer_hover_pos();
    if let Some(p) = hover {
        if let Some(item) = inspect::hit(ctx, p) {
            let painter =
                ctx.layer_painter(LayerId::new(Order::Tooltip, Id::new("cantus-pick-show")));
            let s = Style::current();
            with_brush(&painter, |b| {
                inspect::paint_pick(b, &s, &item, p + Vec2::new(16.0, 16.0), screen);
            });
        }
    }
    if resp.clicked() {
        let picked = hover.and_then(|p| inspect::hit(ctx, p));
        app.toggle_pick();
        if let Some(item) = picked {
            app.appearance_page.picked = Some(item);
            app.appearance_page.tab = Tab::Colours;
            app.show_theme_editor = true;
        }
    } else if resp.secondary_clicked() {
        app.toggle_pick();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Tok;

    fn app() -> CantusApp {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let mut a = CantusApp::from_parts(
            Some(crate::starter_project()),
            cantus_engine::EngineHandle::dummy(),
            false,
        );
        let dir = std::env::temp_dir().join(format!("cantus-appearance-{}", std::process::id()));
        a.appearance_path = super::super::appearance_file::path(&dir);
        a
    }

    #[test]
    fn a_change_applies_and_undoes_in_one_step_per_gesture() {
        let mut a = app();
        let before = a.appearance.clone();
        let mode = editing_mode(&a);
        let c = egui::Color32::from_rgb(10, 200, 120);
        // One gesture of three moves: one undo step.
        for v in [c, egui::Color32::from_rgb(11, 201, 121), c] {
            change(&mut a, |ap| ap.set_token(mode, Tok::Selection, v));
        }
        assert!(a.appearance_page.can_undo());
        a.appearance_page.gesture = false;
        undo(&mut a);
        assert_eq!(
            a.appearance, before,
            "one undo takes the whole gesture back"
        );
        assert!(!a.appearance_page.can_undo());
        redo(&mut a);
        assert_eq!(
            a.appearance.slot(mode).overrides.get(Tok::Selection.key()),
            Some(&crate::theme::to_hex(c))
        );
    }
}
