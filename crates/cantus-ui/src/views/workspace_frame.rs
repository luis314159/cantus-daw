//! One frame of the workspace: what happens before the dock is laid out (sync the central
//! view, give new device editors their tab, impose a layout's sizes), the dock itself
//! with every view in its tab, and what happens after (splitters' right-click and
//! double-click, snapping floating windows, the tab menu).

use super::workspace::{pane_id, Pane, TabMenu, Workspace, ARRANGEMENT_SHARE, INSPECTOR_W, SNAP};
use super::workspace_device as device;
use super::workspace_skin::{self as skin, Titles};
use crate::app::{CantusApp, EditFocus};
use crate::kit::hint::{self, Caps};
use crate::kit::menu::Item;
use crate::kit::text::Src;
use crate::kit::Style;
use egui::emath::TSTransform;
use egui::{
    Context, Id, LayerId, Order, PointerButton, Pos2, Rect, Response, Ui, Vec2, WidgetText,
};
use egui_dock::tab_viewer::OnCloseResponse;
use egui_dock::{DockArea, Node, NodeIndex, NodePath, SurfaceIndex, TabViewer, Tree};

/// Something the workspace asks the application to do once the dock is back in it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum After {
    ResetLayout,
}

/// Lay the workspace out in `ui`: every area as a tab of the dock.
pub fn show(app: &mut CantusApp, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let mut ws = std::mem::take(&mut app.workspace);
    let area = ui.max_rect();
    ws.before(app, &ctx, area);
    let s = Style::current();
    let style = skin::dock_style(&s, ui.style(), ctx.pixels_per_point());
    let tab_bar = style.tab_bar.height;
    {
        let Workspace {
            dock,
            active,
            focused,
            bars,
            titles,
            menu,
            ..
        } = &mut ws;
        let mut viewer = Viewer {
            app,
            ctx: ctx.clone(),
            active,
            focused: *focused,
            bars,
            titles,
            menu,
            buf: String::new(),
            tk: s.tk(),
        };
        DockArea::new(dock)
            .id(Id::new("cantus-dock"))
            .style(style)
            .window_bounds(area)
            .show_close_buttons(true)
            .show_add_buttons(false)
            .tab_context_menus(false)
            .draggable_tabs(true)
            .show_tab_name_on_hover(false)
            .show_leaf_close_all_buttons(false)
            .show_leaf_collapse_buttons(false)
            .show_inside(ui, &mut viewer);
    }
    let then = ws.after(app, ui, area, tab_bar);
    app.workspace = ws;
    if then == Some(After::ResetLayout) {
        app.reset_layout();
    }
}

struct Viewer<'a> {
    app: &'a mut CantusApp,
    ctx: Context,
    active: &'a [Pane],
    focused: Option<Pane>,
    bars: &'a [Rect],
    titles: &'a mut Titles,
    menu: &'a mut Option<TabMenu>,
    buf: String,
    tk: f32,
}

impl TabViewer for Viewer<'_> {
    type Tab = Pane;

    fn id(&mut self, tab: &mut Pane) -> Id {
        pane_id(*tab)
    }

    fn title(&mut self, tab: &mut Pane) -> WidgetText {
        self.titles.get(&self.ctx, *tab, self.tk)
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut Pane) {
        let app = &mut *self.app;
        // A click in the mixer makes it the thing the arrows and M / S act on (§10). The
        // arrangement and the editors say so themselves when they are clicked.
        if *tab == Pane::Mixer
            && ui.input(|i| i.pointer.primary_pressed())
            && ui.rect_contains_pointer(ui.max_rect())
        {
            app.focus = EditFocus::Mixer;
        }
        match *tab {
            Pane::Arrangement => super::arrangement::show(app, ui),
            Pane::PianoRoll => super::piano_roll::show(app, ui),
            Pane::ChannelRack => super::channel_rack::show(app, ui),
            Pane::Mixer => super::mixer::show(app, ui),
            Pane::Inspector => super::inspector::show(app, ui),
            Pane::Device(key) => device::record_body(ui.ctx(), key, ui),
        }
    }

    fn on_tab_button(&mut self, tab: &mut Pane, response: &Response) {
        // The tab being dragged is drawn by egui_dock on its own layer, following the
        // pointer; its place in the bar stays empty.
        if self
            .ctx
            .dragged_id()
            .is_some_and(|d| response.id == d.with("dragged"))
        {
            return;
        }
        // The tab bar it sits in: a tab that overflows its bar is clipped by it, never
        // painted over the next area.
        let r = response.rect;
        let Some(clip) = self
            .bars
            .iter()
            .filter(|b| (b.top() - r.top()).abs() < 3.0 && b.left() <= r.left() + 1.0)
            .max_by(|a, b| a.left().total_cmp(&b.left()))
            .map(|b| b.intersect(r))
            .filter(|c| c.width() > 1.0)
        else {
            return;
        };
        self.titles.text(&self.ctx, *tab, &mut self.buf);
        let lit = self.active.contains(tab);
        let focused = self.focused == Some(*tab);
        skin::paint_tab(
            &self.ctx, response, clip, *tab, &self.buf, lit, focused, true,
        );
        if response.hovered() {
            hint::set_text(
                &self.ctx,
                &self.buf,
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        if response.secondary_clicked() {
            let at = response
                .interact_pointer_pos()
                .unwrap_or(response.rect.left_bottom());
            *self.menu = Some(TabMenu { pane: *tab, at });
        }
    }

    fn on_close(&mut self, tab: &mut Pane) -> OnCloseResponse {
        if let Pane::Device(key) = *tab {
            device::mark_closed(&self.ctx, key);
        }
        OnCloseResponse::Close
    }

    fn scroll_bars(&self, _tab: &Pane) -> [bool; 2] {
        // Every view lays itself out and scrolls itself, as it did in its panel.
        [false, false]
    }
}

/// The rect of every node of `tree` laid out in `area`, by index (as egui_dock does it,
/// before the separators take their pixel).
fn node_rects(tree: &Tree<Pane>, area: Rect, out: &mut Vec<Rect>) {
    out.clear();
    out.resize(tree.len(), Rect::NOTHING);
    if out.is_empty() {
        return;
    }
    out[0] = area;
    for (i, node) in tree.iter().enumerate() {
        let r = out[i];
        if !r.is_finite() {
            continue;
        }
        let (a, b) = match node {
            Node::Horizontal(s) => {
                let x = r.left() + r.width() * s.fraction;
                (
                    Rect::from_min_max(r.min, Pos2::new(x, r.bottom())),
                    Rect::from_min_max(Pos2::new(x, r.top()), r.max),
                )
            }
            Node::Vertical(s) => {
                let y = r.top() + r.height() * s.fraction;
                (
                    Rect::from_min_max(r.min, Pos2::new(r.right(), y)),
                    Rect::from_min_max(Pos2::new(r.left(), y), r.max),
                )
            }
            _ => continue,
        };
        if let Some(o) = out.get_mut(2 * i + 1) {
            *o = a;
        }
        if let Some(o) = out.get_mut(2 * i + 2) {
            *o = b;
        }
    }
}

/// Whether node `i` of `tree` is a leaf holding exactly `pane` and nothing else.
fn leaf_is(tree: &Tree<Pane>, i: usize, pane: Pane) -> bool {
    matches!(tree.iter().nth(i), Some(Node::Leaf(l)) if l.tabs.len() == 1 && l.tabs[0] == pane)
}

/// Whether node `i` of `tree` is a leaf holding `f`.
fn leaf_has(tree: &Tree<Pane>, i: usize, f: impl Fn(Pane) -> bool) -> bool {
    matches!(tree.iter().nth(i), Some(Node::Leaf(l)) if l.tabs.iter().any(|t| f(*t)))
}

impl Workspace {
    fn before(&mut self, app: &mut CantusApp, ctx: &Context, area: Rect) {
        // The central view: `app.central` is what menus, keys and tests set; the dock
        // follows it when it changes.
        let pane = Pane::of_central(app.central);
        match self.synced_central {
            Some(c) if c == app.central => {}
            Some(_) => self.reveal(pane),
            // The first frame: bring it forward if the layout has it, but never put back
            // an area the layout hides.
            None => {
                if self.contains(pane) {
                    self.reveal(pane);
                }
            }
        }
        self.synced_central = Some(app.central);

        // Device editors: a tab for each new one, none for those that closed.
        let mut wanted = std::mem::take(&mut self.wanted);
        device::wanted(ctx, &mut wanted);
        let stale: Vec<Pane> = self
            .dock
            .iter_all_tabs()
            .filter_map(|(_, p)| match p {
                Pane::Device(k) if !wanted.contains(k) => Some(*p),
                _ => None,
            })
            .collect();
        for p in stale {
            self.hide(p);
        }
        for key in &wanted {
            if !self.contains(Pane::Device(*key)) {
                self.reveal(Pane::Device(*key));
                ctx.request_repaint();
            }
        }
        self.wanted = wanted;

        // Sizes: a layout being applied, or the window resized (the inspector keeps its
        // width in points).
        let resized = self.area.is_positive() && (area.width() - self.area.width()).abs() > 0.5;
        if let Some((inspector, arrangement)) = self.pending.take() {
            self.apply_sizes(area, Some(inspector), Some(arrangement));
            self.check_inspector = true;
        } else if resized {
            let w = self.inspector_w;
            self.apply_sizes(area, Some(w), None);
            self.check_inspector = true;
        }
        self.area = area;

        // The inspector collapsing to its 24-pt rail, or opening again, moves the dock's
        // splitter (ADR 0024 §7). Its width otherwise follows the dock: the inspector
        // takes the width it is given, so only the collapse is followed here, and a
        // layout being applied or the splitter being dragged is never fought.
        if self.is_visible(Pane::Inspector) && self.area.is_positive() {
            let collapsed = app.inspector.collapsed;
            if collapsed != self.inspector_collapsed {
                let want = super::inspector::desired_width(app);
                self.inspector_w = want;
                self.apply_sizes(area, Some(want), None);
                self.check_inspector = true;
                self.inspector_collapsed = collapsed;
            }
        }

        // What the tabs need to know while they are painted.
        self.active.clear();
        self.bars.clear();
        let bar = (crate::kit::chrome::TAB * crate::kit::tokens::current().density).round();
        for (_, leaf) in self.dock.iter_leaves() {
            if let Some(t) = leaf.tabs.get(leaf.active.0) {
                self.active.push(*t);
            }
            if leaf.rect.is_finite() {
                self.bars.push(Rect::from_min_size(
                    leaf.rect.min,
                    Vec2::new(leaf.rect.width(), bar),
                ));
            }
        }
        self.focused = self.dock.focused_leaf().and_then(|p| {
            let leaf = self.dock.leaf(p).ok()?;
            leaf.tabs.get(leaf.active.0).copied()
        });
        let fractions: Vec<(usize, usize, f32)> = self
            .dock
            .iter_all_nodes()
            .filter_map(|(p, n)| match n {
                Node::Horizontal(s) | Node::Vertical(s) => {
                    Some((p.surface.0, p.node.0, s.fraction))
                }
                _ => None,
            })
            .collect();
        self.fractions = fractions;
    }

    /// Set the main surface's splits so the inspector is `inspector` points wide and the
    /// arrangement `arrangement` points high, in a dock laid out in `area`.
    fn apply_sizes(&mut self, area: Rect, inspector: Option<f32>, arrangement: Option<f32>) {
        if !area.is_positive() {
            return;
        }
        let mut rects = Vec::new();
        {
            let tree = self.dock.main_surface();
            node_rects(tree, area, &mut rects);
        }
        let tree = self.dock.main_surface_mut();
        let n = tree.len();
        for i in 0..n {
            let (l, r) = (2 * i + 1, 2 * i + 2);
            let rect = rects[i];
            if !rect.is_positive() {
                continue;
            }
            let right_is_inspector = leaf_is(tree, r, Pane::Inspector);
            let top_is_arrangement = leaf_has(tree, l, |p| p == Pane::Arrangement);
            match &mut tree[NodeIndex(i)] {
                Node::Horizontal(s) if right_is_inspector => {
                    if let Some(w) = inspector.filter(|w| *w > 0.0) {
                        // The inspector never takes more than half the window.
                        s.fraction = (1.0 - w / rect.width()).clamp(0.5, 0.95);
                    }
                }
                Node::Vertical(s) if top_is_arrangement => {
                    if let Some(h) = arrangement.filter(|h| *h > 0.0) {
                        s.fraction = (h / rect.height()).clamp(0.1, 0.9);
                    }
                }
                _ => {}
            }
        }
    }

    /// The inspector as laid out differs from the width it was given by what the dock's
    /// separators and margins took: move its split by the difference, once.
    fn correct_inspector(&mut self) {
        let Some(got) = self.measured_sizes().0 else {
            return;
        };
        let want = self.inspector_w;
        if (got - want).abs() < 0.5 {
            return;
        }
        let tree = self.dock.main_surface_mut();
        for i in 0..tree.len() {
            if !leaf_is(tree, 2 * i + 2, Pane::Inspector) {
                continue;
            }
            if let Node::Horizontal(s) = &mut tree[NodeIndex(i)] {
                if s.rect.is_positive() {
                    s.fraction = (s.fraction - (want - got) / s.rect.width()).clamp(0.5, 0.95);
                }
            }
        }
    }

    /// The position a splitter returns to on right-click.
    fn default_fraction(&self, path: NodePath, rect: Rect) -> f32 {
        let tree = &self.dock[path.surface];
        let (l, r) = (2 * path.node.0 + 1, 2 * path.node.0 + 2);
        match tree.iter().nth(path.node.0) {
            Some(Node::Horizontal(_)) if leaf_is(tree, r, Pane::Inspector) => {
                (1.0 - INSPECTOR_W / rect.width().max(1.0)).clamp(0.5, 0.95)
            }
            Some(Node::Vertical(_))
                if leaf_has(tree, l, |p| p == Pane::Arrangement)
                    && leaf_has(tree, r, Pane::is_lower) =>
            {
                ARRANGEMENT_SHARE
            }
            _ => 0.5,
        }
    }

    fn after(&mut self, app: &mut CantusApp, ui: &Ui, area: Rect, tab_bar: f32) -> Option<After> {
        let ctx = ui.ctx().clone();
        // The dock → the central view: the lower zone's tab showing is `app.central`.
        let shown = self
            .dock
            .iter_leaves()
            .filter_map(|(_, l)| l.tabs.get(l.active.0).copied())
            .find(|p| p.is_lower())
            .and_then(Pane::central);
        if let Some(c) = shown {
            if c != app.central {
                app.central = c;
                self.synced_central = Some(c);
            }
        }
        if std::mem::take(&mut self.check_inspector) {
            self.correct_inspector();
        }
        self.splitters(&ctx, ui);
        self.snap(&ctx, area, tab_bar);
        self.tab_menu(&ctx)
    }

    /// Right-click resets a splitter; double-click swaps it with its last position; the
    /// kit's grip lights on the one the pointer is on.
    fn splitters(&mut self, ctx: &Context, ui: &Ui) {
        let (down, released, secondary, double, hover) = ctx.input(|i| {
            (
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.pointer.secondary_clicked(),
                i.pointer.button_double_clicked(PointerButton::Primary),
                i.pointer.interact_pos().or(i.pointer.hover_pos()),
            )
        });
        let reach = crate::kit::chrome::SPLITTER * 0.5 + 2.0;
        let splits: Vec<(NodePath, bool, Rect, f32)> = self
            .dock
            .iter_all_nodes()
            .filter_map(|(p, n)| match n {
                Node::Horizontal(s) => Some((p, true, s.rect, s.fraction)),
                Node::Vertical(s) => Some((p, false, s.rect, s.fraction)),
                _ => None,
            })
            .collect();
        for (path, horizontal, rect, now) in splits {
            if !rect.is_positive() {
                continue;
            }
            let before = self
                .fractions
                .iter()
                .find(|b| b.0 == path.surface.0 && b.1 == path.node.0)
                .map_or(now, |b| b.2);
            let line = if horizontal {
                let x = rect.left() + rect.width() * now;
                Rect::from_min_max(
                    Pos2::new(x - reach, rect.top()),
                    Pos2::new(x + reach, rect.bottom()),
                )
            } else {
                let y = rect.top() + rect.height() * now;
                Rect::from_min_max(
                    Pos2::new(rect.left(), y - reach),
                    Pos2::new(rect.right(), y + reach),
                )
            };
            let on = hover.is_some_and(|p| line.contains(p));
            let default = self.default_fraction(path, rect);
            let memo = self.memo(path);
            if (now - before).abs() > 1e-6 && down {
                memo.drag_from.get_or_insert(before);
            }
            let dragging = memo.drag_from.is_some() && down;
            if released {
                if let Some(from) = memo.drag_from.take() {
                    if (from - now).abs() > 1e-4 {
                        memo.other = Some(from);
                    }
                }
            }
            let mut set = None;
            if double && on {
                set = Some(memo.other.unwrap_or(default));
                memo.other = Some(before);
            } else if secondary && on {
                memo.other = Some(before);
                set = Some(default);
            }
            let inspector_split = horizontal
                && path.surface.is_main()
                && leaf_is(
                    &self.dock[path.surface],
                    2 * path.node.0 + 2,
                    Pane::Inspector,
                );
            if inspector_split && (dragging || set.is_some()) {
                let f = set.unwrap_or(now);
                self.inspector_w = (rect.width() * (1.0 - f)).max(1.0);
            }
            if let Some(f) = set {
                if let Some(Node::Horizontal(s) | Node::Vertical(s)) =
                    self.dock[path.surface].iter_mut().nth(path.node.0)
                {
                    s.fraction = f.clamp(0.05, 0.95);
                }
            }
            if (on || dragging) && path.surface.is_main() {
                let thin = if horizontal {
                    Rect::from_center_size(
                        line.center(),
                        Vec2::new(crate::kit::chrome::SPLITTER, line.height()),
                    )
                } else {
                    Rect::from_center_size(
                        line.center(),
                        Vec2::new(line.width(), crate::kit::chrome::SPLITTER),
                    )
                };
                skin::paint_splitter(ui.painter(), thin, dragging);
                ctx.set_cursor_icon(if horizontal {
                    egui::CursorIcon::ResizeHorizontal
                } else {
                    egui::CursorIcon::ResizeVertical
                });
            }
        }
    }

    /// Floating windows snap to each other and to the workspace's edges while dragged;
    /// Alt disables it. The drag itself is egui's (drift-free); the snapped position is
    /// shown by shifting the window's shapes, and kept when the drag ends.
    fn snap(&mut self, ctx: &Context, area: Rect, _tab_bar: f32) {
        let windows: Vec<(SurfaceIndex, Id, Rect)> = self
            .dock
            .iter_surfaces_indexed()
            .filter(|(i, s)| !i.is_main() && !s.is_empty())
            .filter_map(|(i, _)| {
                let id = Id::new(format!("window {i:?}"));
                egui::AreaState::load(ctx, id).map(|a| (i, id, a.rect()))
            })
            .collect();
        if windows.is_empty() {
            self.snapping = None;
            return;
        }
        let alt = ctx.input(|i| i.modifiers.alt);
        let dragged = ctx.dragged_id();
        for &(surface, id, rect) in &windows {
            let dragging = dragged == Some(id.with("move"));
            if !dragging && self.snapping != Some(surface) {
                continue;
            }
            let target = if alt {
                rect
            } else {
                snapped(
                    rect,
                    area,
                    windows.iter().filter(|w| w.0 != surface).map(|w| w.2),
                )
            };
            let off = target.min - rect.min;
            if off != Vec2::ZERO {
                ctx.transform_layer_shapes(
                    LayerId::new(Order::Middle, id),
                    TSTransform::from_translation(off),
                );
            }
            if dragging {
                self.snapping = Some(surface);
            } else {
                // Released: keep the snapped place.
                if let Some(w) = self.dock.get_window_state_mut(surface) {
                    w.set_position(target.min);
                }
                self.snapping = None;
                ctx.request_repaint();
            }
        }
    }

    /// The tab's right-click menu, in the kit's look.
    fn tab_menu(&mut self, ctx: &Context) -> Option<After> {
        let menu = self.menu?;
        let floating = self.is_floating(menu.pane);
        let items = [
            Item::action(Src::Key(if floating {
                "workspace.menu.dock"
            } else {
                "workspace.menu.float"
            })),
            Item::action(Src::Key("workspace.menu.hide")),
            Item::Separator,
            Item::action(Src::Key("layout.reset")),
        ];
        let s = Style::current();
        let width = 200.0 * s.k;
        let mut chosen = None;
        let area = egui::Area::new(Id::new("cantus-tab-menu"))
            .order(Order::Foreground)
            .fixed_pos(menu.at)
            .constrain(true)
            .show(ctx, |ui| {
                let (_, c) = crate::kit::widgets::menu_list(ui, &items, width);
                chosen = c.clicked;
            });
        let outside = ctx.input(|i| i.pointer.any_click())
            && !ctx
                .pointer_interact_pos()
                .is_some_and(|p| area.response.rect.contains(p));
        let escape = ctx.input(|i| i.key_pressed(egui::Key::Escape));
        let mut then = None;
        match chosen {
            Some(0) => self.toggle_float(menu.pane),
            Some(1) => {
                if let Pane::Device(k) = menu.pane {
                    device::mark_closed(ctx, k);
                }
                self.hide(menu.pane);
            }
            Some(3) => then = Some(After::ResetLayout),
            _ => {}
        }
        if chosen.is_some() || outside || escape {
            self.menu = None;
        }
        then
    }
}

/// `rect` moved so its edges meet the nearest edge of `bounds` or of `others` within
/// [`SNAP`] points, on each axis on its own.
pub fn snapped(rect: Rect, bounds: Rect, others: impl Iterator<Item = Rect>) -> Rect {
    let mut best_x: Option<f32> = None;
    let mut best_y: Option<f32> = None;
    let consider = |best: &mut Option<f32>, d: f32| {
        if d.abs() <= SNAP && best.is_none_or(|b| d.abs() < b.abs()) {
            *best = Some(d);
        }
    };
    let mut edges = |o: Rect, inside: bool| {
        // Inside the workspace: meet its edges from within. Another window: sit beside it
        // (my left on its right) or line up with it (my left on its left).
        for x in [o.left(), o.right()] {
            consider(&mut best_x, x - rect.left());
            consider(&mut best_x, x - rect.right());
        }
        for y in [o.top(), o.bottom()] {
            consider(&mut best_y, y - rect.top());
            consider(&mut best_y, y - rect.bottom());
        }
        let _ = inside;
    };
    edges(bounds, true);
    for o in others {
        edges(o, false);
    }
    rect.translate(Vec2::new(best_x.unwrap_or(0.0), best_y.unwrap_or(0.0)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_snaps_within_eight_points_and_not_beyond() {
        let bounds = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 700.0));
        let w = Rect::from_min_size(Pos2::new(5.0, 300.0), Vec2::new(200.0, 100.0));
        assert_eq!(snapped(w, bounds, std::iter::empty()).left(), 0.0);
        let far = w.translate(Vec2::new(20.0, 0.0));
        assert_eq!(snapped(far, bounds, std::iter::empty()), far);
        // Beside another window: my left edge on its right edge.
        let other = Rect::from_min_size(Pos2::new(300.0, 100.0), Vec2::new(100.0, 100.0));
        let w = Rect::from_min_size(Pos2::new(406.0, 150.0), Vec2::new(120.0, 80.0));
        let s = snapped(w, bounds, [other].into_iter());
        assert_eq!(s.left(), 400.0);
        assert_eq!(s.top(), 150.0, "the other axis stays where it was");
    }

    #[test]
    fn node_rects_follow_the_fractions() {
        let d = super::super::workspace::default_dock();
        let mut out = Vec::new();
        let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 600.0));
        node_rects(d.main_surface(), area, &mut out);
        assert_eq!(out[0], area);
        assert!(
            (out[2].width() - 220.0).abs() < 0.5,
            "the inspector's share"
        );
        assert!((out[3].height() - 600.0 * ARRANGEMENT_SHARE).abs() < 0.5);
    }
}
