//! Folding the workspace's areas (round 39, ADR 0110): every docked area **collapses** to
//! its tab bar and expands back where it was, **takes the whole workspace** and gives it
//! back, and the keyboard **moves between areas** in reading order.
//!
//! - **Collapse** (the tab's menu, *Collapse area*, Ctrl+Alt+C): the area's side of the split
//!   that holds it shrinks to its tab bar — a bar's height in a column, a rail of the same
//!   width in a row — and the split's old position is kept, so *Expand* (or a click on the
//!   folded area) puts it back exactly. Unlike hiding (F5…F9), the area keeps its place, its
//!   tab and its neighbours. A drag of the folded split's line unfolds it where it is let go.
//!   The folds are kept with the layout ([`crate::layout::Layout::folds`]).
//! - **Maximise** (the tab's menu, a double-click on a tab, Ctrl+Shift+Enter): the area's
//!   leaf takes the whole main surface; the same again — or showing or hiding any area —
//!   gives every other area back as it was. Floating windows stay where they float. A layout
//!   saved meanwhile is the shape under the maximised area.
//! - **Next area / previous area** (Ctrl+F6 / Ctrl+Shift+F6, the platform's keys for it):
//!   the focus goes to the next area on screen, left to right and top to bottom; the focused
//!   tab is drawn lit, and the keys that act on "the area being worked in" follow it.
//!
//! The dock keeps its own minimum (`egui_dock`'s `separator.extra`, 96 pt): while an area
//! is folded the workspace lowers it to the fold's size, or the fold would be undone every
//! frame ([`Workspace::separator_floor`]).

use super::workspace::{Pane, Workspace};
use egui::Rect;
use egui_dock::{DockState, Node, NodeIndex, NodePath, SurfaceIndex, Tree};
use serde::{Deserialize, Serialize};

/// A folded area: which, and its split's position before it folded.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fold {
    pub pane: Pane,
    /// The split's fraction before the fold: where *Expand* puts it back.
    pub before: f32,
    /// The fraction the fold set last frame: a split moved from it was dragged, and unfolds.
    #[serde(skip)]
    pub set: Option<f32>,
}

/// The area maximised, and the main surface it was taken from.
pub struct Maximised {
    pub pane: Pane,
    pub tree: Tree<Pane>,
}

/// The size of a folded area across its split, in points at 100 %: the tab bar's height.
pub fn fold_size() -> f32 {
    (crate::kit::chrome::TAB * crate::kit::tokens::current().density).round()
}

/// The split holding `node` and whether `node` is its left (top) child.
fn parent_of(node: NodeIndex) -> Option<(NodeIndex, bool)> {
    let i = node.0;
    if i == 0 {
        return None;
    }
    Some((NodeIndex((i - 1) / 2), i % 2 == 1))
}

/// The fraction that leaves `size` points to the `left` (top) child or to the other one,
/// of a split laid out in `rect`.
fn fold_fraction(rect: Rect, horizontal: bool, left: bool, size: f32) -> Option<f32> {
    let range = if horizontal {
        rect.width()
    } else {
        rect.height()
    };
    if !(range.is_finite() && range > size * 2.0) {
        return None;
    }
    let f = (size / range).clamp(0.0, 1.0);
    Some(if left { f } else { 1.0 - f })
}

impl Workspace {
    /// Whether `pane` is folded to its tab bar.
    pub fn is_folded(&self, pane: Pane) -> bool {
        self.folds.iter().any(|f| f.pane == pane)
    }

    /// The main surface's leaf holding `pane` and its split, if it has one: a lone area
    /// fills the workspace and has nothing to fold against.
    fn fold_place(&self, pane: Pane) -> Option<(NodeIndex, NodeIndex, bool)> {
        let path = self.find(pane).filter(|p| p.surface.is_main())?;
        let (split, left) = parent_of(path.node)?;
        Some((path.node, split, left))
    }

    /// Whether `pane` can fold: docked on the main surface beside another area.
    pub fn can_fold(&self, pane: Pane) -> bool {
        self.maximised.is_none() && self.fold_place(pane).is_some()
    }

    /// Collapse `pane` to its tab bar, keeping its split's position to expand back to.
    pub fn fold(&mut self, pane: Pane) {
        if self.is_folded(pane) || !self.can_fold(pane) {
            return;
        }
        let Some((_, split, _)) = self.fold_place(pane) else {
            return;
        };
        let before = match &self.dock.main_surface()[split] {
            Node::Horizontal(s) | Node::Vertical(s) => s.fraction,
            _ => return,
        };
        self.folds.push(Fold {
            pane,
            before,
            set: None,
        });
        self.keep_folds();
    }

    /// Expand a folded `pane` back to where its split was.
    pub fn unfold(&mut self, pane: Pane) {
        let Some(i) = self.folds.iter().position(|f| f.pane == pane) else {
            return;
        };
        let f = self.folds.remove(i);
        if let Some((_, split, _)) = self.fold_place(pane) {
            if let Node::Horizontal(s) | Node::Vertical(s) =
                &mut self.dock.main_surface_mut()[split]
            {
                s.fraction = f.before.clamp(0.05, 0.95);
            }
        }
    }

    /// Collapse or expand `pane`.
    pub fn toggle_fold(&mut self, pane: Pane) {
        if self.is_folded(pane) {
            self.unfold(pane);
        } else {
            self.fold(pane);
        }
    }

    /// Before the dock is laid out: each fold's split held at the fold's size in points
    /// (the window may have been resized), and a fold whose split was dragged let go.
    pub(super) fn keep_folds(&mut self) {
        let size = fold_size() + 1.0;
        let mut i = 0;
        while i < self.folds.len() {
            let pane = self.folds[i].pane;
            let Some((_, split, left)) = self.fold_place(pane) else {
                // Gone from the main surface (hidden, floated): nothing to hold.
                self.folds.remove(i);
                continue;
            };
            let tree = self.dock.main_surface_mut();
            let (rect, now, horizontal) = match &tree[split] {
                Node::Horizontal(s) => (s.rect, s.fraction, true),
                Node::Vertical(s) => (s.rect, s.fraction, false),
                _ => {
                    self.folds.remove(i);
                    continue;
                }
            };
            if let Some(set) = self.folds[i].set {
                if (now - set).abs() > 1e-4 {
                    // Dragged away from the fold: the area is open where it was let go.
                    self.folds.remove(i);
                    continue;
                }
            }
            if let Some(f) = fold_fraction(rect, horizontal, left, size) {
                if let Node::Horizontal(s) | Node::Vertical(s) = &mut tree[split] {
                    s.fraction = f;
                }
                self.folds[i].set = Some(f);
            }
            i += 1;
        }
    }

    /// The dock's least size for an area, in points: the fold's while one is folded.
    pub fn separator_floor(&self, normal: f32) -> f32 {
        if self.folds.is_empty() {
            normal
        } else {
            normal.min(fold_size())
        }
    }

    /// The folds a layout keeps.
    pub fn folds(&self) -> &[Fold] {
        &self.folds
    }

    /// A layout's folds, held from the next frame.
    pub fn set_folds(&mut self, folds: &[Fold]) {
        self.folds = folds
            .iter()
            .filter(|f| !matches!(f.pane, Pane::Device(_)))
            .map(|f| Fold { set: None, ..*f })
            .collect();
    }

    // ---- maximise ----------------------------------------------------------------------

    /// The area maximised, if one is.
    pub fn maximised(&self) -> Option<Pane> {
        self.maximised.as_ref().map(|m| m.pane)
    }

    /// `pane`'s leaf takes the whole main surface; the same again gives the rest back.
    pub fn toggle_maximise(&mut self, pane: Pane) {
        if self.maximised.is_some() {
            self.restore_maximised();
            return;
        }
        let Some(path) = self.find(pane).filter(|p| p.surface.is_main()) else {
            return;
        };
        let Ok(leaf) = self.dock.leaf(path.node_path()) else {
            return;
        };
        if self.dock.main_surface().len() <= 1 {
            return;
        }
        let (tabs, active) = (leaf.tabs.clone(), leaf.active);
        let mut alone = Tree::new(tabs);
        if let Node::Leaf(l) = &mut alone[NodeIndex::root()] {
            l.active = active;
        }
        let tree = std::mem::replace(self.dock.main_surface_mut(), alone);
        self.maximised = Some(Maximised { pane, tree });
        self.dock.set_focused_node_and_surface(NodePath {
            surface: SurfaceIndex::main(),
            node: NodeIndex::root(),
        });
    }

    /// Give back the main surface a maximised area took (its tab in front kept).
    pub fn restore_maximised(&mut self) {
        let Some(m) = self.maximised.take() else {
            return;
        };
        let front = self.dock.main_surface().root_node().and_then(|n| match n {
            Node::Leaf(l) => l.tabs.get(l.active.0).copied(),
            _ => None,
        });
        *self.dock.main_surface_mut() = m.tree;
        if let Some(p) = front {
            if let Some(path) = self.find(p) {
                let _ = self.dock.set_active_tab(path);
                self.dock.set_focused_node_and_surface(path.node_path());
            }
        }
    }

    /// The dock as a layout keeps it: the shape under a maximised area.
    pub fn dock_for_layout(&self) -> DockState<Pane> {
        let mut d = self.dock.clone();
        if let Some(m) = &self.maximised {
            *d.main_surface_mut() = m.tree.clone();
        }
        d
    }

    // ---- the focus between areas -------------------------------------------------------

    /// The areas showing on the main surface, in reading order (left to right, then top to
    /// bottom, by their rects' top-left corners), each its leaf's tab in front.
    pub fn areas_in_order(&self) -> Vec<(Pane, Rect)> {
        let mut v: Vec<(Pane, Rect)> = self
            .dock
            .main_surface()
            .iter()
            .filter_map(|n| match n {
                Node::Leaf(l) => {
                    let r = l.rect;
                    let p = l.tabs.get(l.active.0).copied()?;
                    (r.is_finite() && r.is_positive()).then_some((p, r))
                }
                _ => None,
            })
            .collect();
        // Rows first: areas whose tops are within a tab bar of each other read as one row.
        let row = fold_size().max(8.0);
        v.sort_by(|a, b| {
            let (ra, rb) = ((a.1.top() / row).round(), (b.1.top() / row).round());
            ra.total_cmp(&rb).then(a.1.left().total_cmp(&b.1.left()))
        });
        v
    }

    /// The area `step` places after (or before, negative) the focused one, in reading
    /// order, wrapping round; it is focused and returned.
    pub fn focus_step(&mut self, step: i32) -> Option<Pane> {
        let areas = self.areas_in_order();
        if areas.is_empty() {
            return None;
        }
        let now = self
            .dock
            .focused_leaf()
            .and_then(|p| self.dock.leaf(p).ok())
            .and_then(|l| l.tabs.get(l.active.0).copied());
        let at = now
            .and_then(|p| areas.iter().position(|a| a.0 == p))
            .map_or(if step >= 0 { -1 } else { 0 }, |i| i as i32);
        let n = areas.len() as i32;
        let next = areas[((at + step).rem_euclid(n)) as usize].0;
        if let Some(path) = self.find(next) {
            self.dock.set_focused_node_and_surface(path.node_path());
        }
        Some(next)
    }

    /// Open `pane`'s tab menu under its tab (Shift+F10 on a focused area).
    pub fn open_tab_menu(&mut self, pane: Pane) {
        let Some(leaf) = self
            .find(pane)
            .and_then(|p| self.dock.leaf(p.node_path()).ok())
            .map(|l| l.rect)
            .filter(|r| r.is_finite())
        else {
            return;
        };
        self.menu = Some(super::workspace::TabMenu {
            pane,
            at: leaf.min + egui::vec2(12.0, fold_size()),
            fresh: true,
        });
    }

    /// The area being worked in: the focused leaf's tab in front.
    pub fn focused_area(&self) -> Option<Pane> {
        let p = self.dock.focused_leaf()?;
        let l = self.dock.leaf(p).ok()?;
        l.tabs.get(l.active.0).copied()
    }
}

/// A folded area's body: its glyph on the panel, the whole of it a key that expands it.
/// Returns whether it was clicked.
pub fn rail(ui: &mut egui::Ui, pane: Pane) -> bool {
    use crate::kit::{glyph, hint, with_brush, Fill, Style};
    let rect = ui.max_rect();
    if !rect.is_positive() {
        return false;
    }
    let resp = ui.interact(
        rect,
        ui.id().with(("cantus-fold-rail", pane)),
        egui::Sense::click(),
    );
    let s = Style::current();
    let t = s.t;
    with_brush(ui.painter(), |b| {
        b.rect(rect, Fill::Solid(t.panel));
        let side = (12.0 * s.tk())
            .min(rect.width() - 4.0)
            .min(rect.height() - 4.0);
        if side > 4.0 {
            let g = Rect::from_center_size(
                egui::pos2(rect.center().x, rect.top() + side * 0.5 + 6.0 * s.k),
                egui::Vec2::splat(side),
            );
            let ink = if resp.hovered() { t.text } else { t.text_dim };
            glyph::paint(b, g, pane.glyph(), ink);
        }
    });
    if resp.hovered() {
        let name = crate::i18n::t(pane.title_key());
        let tip = crate::i18n::tf("ux.panels.folded_hint", &[("area", &name)]);
        hint::set_text(ui.ctx(), &tip, "", hint::Caps::NONE);
    }
    resp.clicked()
}

// ---- the docks' row under a maximised area (r39:lo-arrange) -------------------------------

/// The height of the row the hidden areas fold to while one is maximised, at 100 %.
pub const DOCKS_ROW: f32 = 28.0;

/// What the docks' row was asked for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RowPick {
    /// An area's tab: everything comes back, that area in front.
    Pane(Pane),
    /// "Show the docks": everything comes back as it was.
    Restore,
}

impl Workspace {
    /// The areas a maximised one hid (des-arrange's maximised arrangement: the docks fold
    /// to one row of their tabs), in the layout's order; none while nothing is maximised.
    pub fn hidden_by_maximise(&self) -> Vec<Pane> {
        let Some(m) = &self.maximised else {
            return Vec::new();
        };
        m.tree
            .tabs()
            .copied()
            .filter(|p| *p != m.pane && !matches!(p, Pane::Device(_)))
            .collect()
    }
}

/// The keymap's action that shows `pane`, for its key in the row.
fn pane_action(pane: Pane) -> Option<crate::keymap::Action> {
    use crate::keymap::Action;
    Some(match pane {
        Pane::Arrangement => Action::ViewArrangement,
        Pane::ChannelRack => Action::ViewChannelRack,
        Pane::PianoRoll => Action::ViewPianoRoll,
        Pane::Mixer => Action::ViewMixer,
        Pane::Inspector => Action::ToggleInspector,
        Pane::Browser => Action::ToggleBrowser,
        Pane::Routing => Action::ViewRouting,
        Pane::Controllers => Action::ViewControllers,
        Pane::History => Action::ViewHistory,
        Pane::Score => Action::ViewScore,
        Pane::Session => Action::ToggleSessionView,
        _ => return None,
    })
}

/// The docks' row (des-arrange `max-*`): one 28-px row at the workspace's foot holding the
/// hidden areas' tabs, each its name (Medium 11) and its key (ink 3) — "Mixer F9 · Piano
/// roll F7" — and "Show the docks ⌃" at its right. Returns what was clicked.
pub fn docks_row(
    ui: &mut egui::Ui,
    rect: Rect,
    panes: &[Pane],
    keymap: &crate::keymap::Keymap,
) -> Option<RowPick> {
    use crate::kit::text::{self, Elide, Policy, Role, Text};
    use crate::kit::{hint, with_brush, Fill, Style};
    let s = Style::current();
    let room = super::room::Room::of(&s.t);
    let k = s.k;
    with_brush(ui.painter(), |b| {
        b.register(rect, "docks_row", &[crate::kit::Tok::Toolbar], &[]);
        let r = b.snap_rect(rect);
        b.rect(r, Fill::Solid(room.surface));
        b.hline(
            r.left(),
            r.right(),
            r.top() + b.px() * 0.5,
            b.px(),
            room.edge,
        );
    });
    let mut picked = None;
    let k11 = s.tk() * 11.0 / Role::Control.size().max(1.0);
    let k10 = s.tk() * 10.0 / Role::Label.size().max(1.0);
    // At the right: "Show the docks ⌃".
    let word = crate::i18n::t("lo_arrange.docks.show");
    let ww = text::measure(ui.ctx(), &Text::new(&word, Role::Label, room.ink2), s.tk()).x;
    let show = Rect::from_min_max(
        egui::pos2(rect.right() - ww - 30.0 * k, rect.top()),
        egui::pos2(rect.right() - 8.0 * k, rect.bottom()),
    );
    let resp = ui.interact(show, ui.id().with("docks-row-show"), egui::Sense::click());
    let ink = if resp.hovered() { room.ink1 } else { room.ink2 };
    with_brush(ui.painter(), |b| {
        let t = Text::new(&word, Role::Label, ink)
            .caps(false)
            .align(egui::Align2::LEFT_CENTER);
        let tr = Rect::from_min_max(egui::pos2(show.left() + 4.0 * k, show.top()), show.max);
        let _ = text::paint(b, tr, &t, s.tk());
        let c = egui::pos2(show.right() - 7.0 * k, show.center().y);
        let h = 2.5 * k;
        b.polyline(
            &[
                egui::pos2(c.x - 2.0 * h, c.y + h),
                egui::pos2(c.x, c.y - h),
                egui::pos2(c.x + 2.0 * h, c.y + h),
            ],
            1.3 * k,
            ink,
        );
    });
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            &crate::i18n::t("lo_arrange.docks.show_tip"),
            "",
            hint::Caps::NONE,
        );
    }
    if resp.clicked() {
        picked = Some(RowPick::Restore);
    }
    // From the left: the hidden areas' tabs.
    let mut x = rect.left() + 12.0 * k;
    for pane in panes {
        let name = crate::i18n::t(pane.title_key());
        let key = pane_action(*pane)
            .and_then(|a| keymap.bindings_for(a).first().map(|b| b.display()))
            .unwrap_or_default();
        let nt = Text::new(&name, Role::Control, room.ink2).caps(false);
        let kt = Text::new(&key, Role::Label, room.ink3).caps(false);
        let nw = text::measure(ui.ctx(), &nt, k11).x;
        let kw = if key.is_empty() {
            0.0
        } else {
            text::measure(ui.ctx(), &kt, k10).x + 6.0 * k
        };
        let w = nw + kw + 12.0 * k;
        if x + w > show.left() - 8.0 * k {
            break;
        }
        let tab = Rect::from_min_max(
            egui::pos2(x - 6.0 * k, rect.top() + 4.0 * k),
            egui::pos2(x + w - 6.0 * k, rect.bottom() - 4.0 * k),
        );
        let resp = ui.interact(
            tab,
            ui.id().with(("docks-row", *pane)),
            egui::Sense::click(),
        );
        with_brush(ui.painter(), |b| {
            if resp.hovered() {
                b.rrect(b.snap_rect(tab), s.r(3.0), Fill::Solid(room.well_hi));
            }
            let ink = if resp.hovered() { room.ink1 } else { room.ink2 };
            let nt = Text::new(&name, Role::Control, ink)
                .caps(false)
                .align(egui::Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let nr = Rect::from_min_max(
                egui::pos2(x, rect.top()),
                egui::pos2(x + nw + 1.0, rect.bottom()),
            );
            let _ = text::paint(b, nr, &nt, k11);
            if !key.is_empty() {
                let kt = Text::new(&key, Role::Label, room.ink3)
                    .caps(false)
                    .align(egui::Align2::LEFT_CENTER);
                let kr = Rect::from_min_max(
                    egui::pos2(nr.right() + 6.0 * k, rect.top()),
                    egui::pos2(nr.right() + kw + 2.0, rect.bottom()),
                );
                let _ = text::paint(b, kr, &kt, k10);
            }
        });
        if resp.hovered() {
            let tip = crate::i18n::tf("lo_arrange.docks.tab_tip", &[("area", &name)]);
            hint::set_text(ui.ctx(), &tip, "", hint::Caps::NONE);
        }
        if resp.clicked() {
            picked = Some(RowPick::Pane(*pane));
        }
        x += w + 12.0 * k;
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::workspace::Pane;

    #[test]
    fn the_parent_of_a_node_is_its_split_and_its_side() {
        assert_eq!(parent_of(NodeIndex(0)), None);
        assert_eq!(parent_of(NodeIndex(1)), Some((NodeIndex(0), true)));
        assert_eq!(parent_of(NodeIndex(2)), Some((NodeIndex(0), false)));
        assert_eq!(parent_of(NodeIndex(5)), Some((NodeIndex(2), true)));
    }

    #[test]
    fn a_fold_leaves_its_size_to_its_side() {
        let r = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 500.0));
        assert_eq!(fold_fraction(r, true, true, 25.0), Some(0.025));
        assert_eq!(fold_fraction(r, false, false, 25.0), Some(0.95));
        assert_eq!(fold_fraction(r, false, true, 300.0), None, "no room");
    }

    #[test]
    fn a_lone_area_does_not_fold_and_a_maximised_one_gives_everything_back() {
        let mut ws = Workspace::default();
        let before = ws.dock.main_surface().len();
        ws.toggle_maximise(Pane::Mixer);
        assert_eq!(ws.maximised(), Some(Pane::Mixer));
        assert_eq!(ws.dock.main_surface().len(), 1);
        assert!(!ws.can_fold(Pane::Mixer), "alone in the workspace");
        // The layout keeps the shape under it.
        assert_eq!(ws.dock_for_layout().main_surface().len(), before);
        ws.toggle_maximise(Pane::Mixer);
        assert_eq!(ws.maximised(), None);
        assert_eq!(ws.dock.main_surface().len(), before);
        for p in Pane::ZONES {
            assert!(ws.contains(p), "{p:?} is back");
        }
    }

    #[test]
    fn folding_keeps_the_split_to_expand_back_to() {
        let mut ws = Workspace::default();
        let (_, split, _) = ws.fold_place(Pane::Inspector).unwrap();
        let frac = |ws: &Workspace| match &ws.dock.main_surface()[split] {
            Node::Horizontal(s) | Node::Vertical(s) => s.fraction,
            _ => f32::NAN,
        };
        let was = frac(&ws);
        ws.fold(Pane::Inspector);
        assert!(ws.is_folded(Pane::Inspector));
        ws.unfold(Pane::Inspector);
        assert!(!ws.is_folded(Pane::Inspector));
        assert_eq!(frac(&ws), was);
    }
}
