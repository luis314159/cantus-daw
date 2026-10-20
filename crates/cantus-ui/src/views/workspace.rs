//! The workspace (ADR 0024 §5): the window's areas as tabs that dock, split, stack and
//! float — arrangement, piano roll, channel rack, mixer, inspector and every device
//! editor — hosted by `egui_dock` and skinned with the kit.
//!
//! - **The default layout**: the arrangement in the centre, the lower zone holding the
//!   mixer, the piano roll and the channel rack as tabs, the inspector on the right.
//! - **Show and hide**: F5 arrangement, F6 rack, F7 piano roll, F9 mixer (FL's keys) and I
//!   the inspector toggle their area ([`Workspace::toggle`]); the toolbar has a key for
//!   each, and the status bar's corners hold the lower zone's and the inspector's.
//! - **Every splitter**: drag to resize (a 1-px line lights on hover, with a grip);
//!   **right-click resets** it; **double-click toggles** between its last two positions.
//!   The inspector keeps its width in points when the window is resized.
//! - **Floating windows snap** to each other and to the workspace's edges within 8 pt while
//!   dragged; **Alt** disables snapping (FL). F12 closes the floating device windows.
//! - **Device windows dock by default and float by choice** ([`super::workspace_device`]).
//! - The tree is serialised with the named layouts (`crate::layout`); a layout from before
//!   the dock is migrated to it.
//!
//! The dock is taken out of the application for the frame (`std::mem::take`), so each tab
//! can call its view with `&mut CantusApp`, exactly as the views were called before.

use super::workspace_device::{self as device, DeviceKey};
use crate::app::CentralView;
use crate::kit::glyph::Glyph;
use egui::{Context, Id, Pos2, Rect, Vec2};
use egui_dock::{DockState, Node, NodeIndex, NodePath, SurfaceIndex, TabIndex, TabPath};
use serde::{Deserialize, Serialize};

/// The inspector's width by default, in points (ADR 0024 §7).
pub const INSPECTOR_W: f32 = 280.0;
/// The sample browser's width when it opens, in points.
pub const BROWSER_W: f32 = 260.0;
/// The share of the height the arrangement takes above the lower zone by default.
pub const ARRANGEMENT_SHARE: f32 = 0.46;
/// How close a floating window's edge must come to another edge to snap to it, in points.
pub const SNAP: f32 = 8.0;

/// One area of the workspace: what a tab shows.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    Arrangement,
    PianoRoll,
    ChannelRack,
    Mixer,
    Inspector,
    /// The devices panel: every instrument and effect to browse and drag (round 20,
    /// `views::picker_panel`), docked at the left like FL's browser.
    Devices,
    /// The sample browser (ADR 0030): FL's browser, at the left.
    Browser,
    /// The clip browser (round 26, ADR 0037): link groups and clips as tiles to drag onto
    /// the arrangement, FL's playlist picker panel; at the left, Alt+F5.
    Clips,
    /// The Routing tab: the routing graph as a matrix (round 25, `views::routing`), in the
    /// lower zone beside the mixer; Alt+F9.
    Routing,
    /// The controller mappings (round 29, ADR 0062, `views::controllers`): every MIDI
    /// mapping, global and the project's, in the lower zone beside the mixer; Ctrl+Shift+K.
    Controllers,
    /// A device editor, by the key its window registered under. Never saved: devices
    /// are opened by the project, not by the layout.
    Device(DeviceKey),
}

impl Pane {
    /// The areas a layout is made of, in the toolbar's order.
    pub const ZONES: [Pane; 5] = [
        Pane::Arrangement,
        Pane::ChannelRack,
        Pane::PianoRoll,
        Pane::Mixer,
        Pane::Inspector,
    ];

    /// The i18n key of its name.
    pub fn title_key(self) -> &'static str {
        match self {
            Pane::Arrangement => "workspace.pane.arrangement",
            Pane::PianoRoll => "workspace.pane.piano_roll",
            Pane::ChannelRack => "workspace.pane.channel_rack",
            Pane::Mixer => "workspace.pane.mixer",
            Pane::Inspector => "workspace.pane.inspector",
            Pane::Devices => "picker.panel.title",
            Pane::Browser => "samples.title",
            Pane::Clips => "links.browser.title",
            Pane::Routing => "matrix.pane",
            Pane::Controllers => "ctl.pane",
            Pane::Device(_) => "workspace.pane.device",
        }
    }

    /// The i18n key of what its toolbar key does.
    pub fn toggle_key(self) -> &'static str {
        match self {
            Pane::Arrangement => "workspace.toggle.arrangement",
            Pane::PianoRoll => "workspace.toggle.piano_roll",
            Pane::ChannelRack => "workspace.toggle.channel_rack",
            Pane::Mixer => "workspace.toggle.mixer",
            Pane::Inspector => "workspace.toggle.inspector",
            Pane::Devices => "picker.menu.view_devices",
            Pane::Browser => "samples.toggle",
            Pane::Clips => "links.browser.toggle",
            Pane::Routing => "matrix.toggle",
            Pane::Controllers => "ctl.toggle",
            Pane::Device(_) => "workspace.pane.device",
        }
    }

    pub fn glyph(self) -> Glyph {
        match self {
            Pane::Arrangement => Glyph::Arrangement,
            Pane::PianoRoll => Glyph::PianoRoll,
            Pane::ChannelRack => Glyph::Rack,
            Pane::Mixer => Glyph::Mixer,
            Pane::Inspector => Glyph::Inspector,
            Pane::Devices => Glyph::Browser,
            Pane::Browser => Glyph::Browser,
            Pane::Clips => Glyph::Link,
            Pane::Routing => Glyph::Send,
            Pane::Controllers => Glyph::Controller,
            Pane::Device(_) => Glyph::Device,
        }
    }

    /// Whether it belongs in the lower zone: the three editors that used to take turns in
    /// the centre of the window, and the Routing tab beside the mixer.
    pub fn is_lower(self) -> bool {
        matches!(
            self,
            Pane::PianoRoll | Pane::ChannelRack | Pane::Mixer | Pane::Routing | Pane::Controllers
        )
    }

    /// The legacy central view this pane is, if it is one.
    pub fn central(self) -> Option<CentralView> {
        match self {
            Pane::PianoRoll => Some(CentralView::PianoRoll),
            Pane::ChannelRack => Some(CentralView::ChannelRack),
            Pane::Mixer => Some(CentralView::Mixer),
            _ => None,
        }
    }

    pub fn of_central(c: CentralView) -> Pane {
        match c {
            CentralView::PianoRoll => Pane::PianoRoll,
            CentralView::ChannelRack => Pane::ChannelRack,
            CentralView::Mixer => Pane::Mixer,
        }
    }
}

/// The dock a fresh install opens with, and what *Reset layout* returns to: the
/// arrangement in the centre, the lower zone (mixer, piano roll, rack) under it, the
/// inspector on the right.
pub fn default_dock() -> DockState<Pane> {
    let mut d = DockState::new(vec![Pane::Arrangement]);
    let s = d.main_surface_mut();
    let [centre, _] = s.split_right(NodeIndex::root(), 0.78, vec![Pane::Inspector]);
    s.split_below(
        centre,
        ARRANGEMENT_SHARE,
        vec![Pane::Mixer, Pane::PianoRoll, Pane::ChannelRack],
    );
    d
}

/// Per splitter: the other of its last two positions, and where a drag started.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(super) struct SplitMemo {
    pub(super) surface: usize,
    pub(super) node: usize,
    pub(super) other: Option<f32>,
    pub(super) drag_from: Option<f32>,
}

/// A tab's right-click menu, while open.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) struct TabMenu {
    pub pane: Pane,
    pub at: Pos2,
}

/// The workspace: the dock and what the splitters, the snapping and the toggles remember.
pub struct Workspace {
    pub dock: DockState<Pane>,
    pub(super) splits: Vec<SplitMemo>,
    /// The split fractions at the start of the frame, per (surface, node).
    pub(super) fractions: Vec<(usize, usize, f32)>,
    /// The floating window being snapped while it is dragged.
    pub(super) snapping: Option<SurfaceIndex>,
    /// The dock's area last frame.
    pub(super) area: Rect,
    /// Sizes a layout imposes on the next frame: inspector width, arrangement height.
    pub(super) pending: Option<(f32, f32)>,
    /// The inspector's width in points, kept when the window is resized.
    pub(super) inspector_w: f32,
    /// A width was just imposed: measure it next frame and correct what the dock's own
    /// margins took (a split sets a fraction, not points).
    pub(super) check_inspector: bool,
    /// Whether the inspector was collapsed to its rail last frame.
    pub(super) inspector_collapsed: bool,
    /// What the lower-zone toggle hid, to bring it back as it was.
    lower_hidden: Vec<Pane>,
    lower_active: Option<Pane>,
    /// The central view the application asked for last (`app.central`).
    pub(super) synced_central: Option<CentralView>,
    /// Scratch, reused every frame.
    pub(super) active: Vec<Pane>,
    pub(super) focused: Option<Pane>,
    pub(super) bars: Vec<Rect>,
    pub(super) wanted: Vec<DeviceKey>,
    pub(super) menu: Option<TabMenu>,
    pub(super) titles: super::workspace_skin::Titles,
}

impl Default for Workspace {
    /// The default dock, with the inspector at its width in points on the first frame.
    fn default() -> Self {
        let mut ws = Workspace::with_dock(default_dock());
        ws.impose_sizes(INSPECTOR_W, 0.0);
        ws
    }
}

impl Workspace {
    pub fn with_dock(dock: DockState<Pane>) -> Workspace {
        Workspace {
            dock,
            splits: Vec::new(),
            fractions: Vec::new(),
            snapping: None,
            area: Rect::NOTHING,
            pending: None,
            inspector_w: INSPECTOR_W,
            check_inspector: false,
            inspector_collapsed: false,
            lower_hidden: Vec::new(),
            lower_active: None,
            synced_central: None,
            active: Vec::with_capacity(16),
            focused: None,
            bars: Vec::with_capacity(8),
            wanted: Vec::with_capacity(4),
            menu: None,
            titles: Default::default(),
        }
    }

    /// Take a layout's shape: its dock (migrated if it is from before the dock) and, on the
    /// next frame, its sizes.
    pub fn load(&mut self, layout: &crate::layout::Layout) {
        // Device editors stay where they are: a layout does not open or close them.
        let devices: Vec<Pane> = self
            .dock
            .iter_all_tabs()
            .map(|(_, p)| *p)
            .filter(|p| matches!(p, Pane::Device(_)))
            .collect();
        self.dock = layout.dock_state();
        for d in devices {
            self.place(d);
        }
        self.impose_sizes(layout.inspector, layout.arrangement);
        self.splits.clear();
        self.synced_central = None;
        self.menu = None;
    }

    /// Where `pane` is, if it is in the dock.
    pub fn find(&self, pane: Pane) -> Option<TabPath> {
        self.dock.find_tab(&pane)
    }

    /// Whether `pane` is in the dock at all (it may be behind another tab).
    pub fn contains(&self, pane: Pane) -> bool {
        self.find(pane).is_some()
    }

    /// Whether `pane` is on screen: in the dock and the active tab of its leaf.
    pub fn is_visible(&self, pane: Pane) -> bool {
        self.find(pane).is_some_and(|p| {
            self.dock
                .leaf(p.node_path())
                .is_ok_and(|leaf| leaf.active == p.tab)
        })
    }

    /// F5 / F6 / F7 / F9 / I: hide an area that is showing; bring one that is behind
    /// another tab to the front; put one that is not there back where it belongs.
    pub fn toggle(&mut self, pane: Pane) {
        if self.is_visible(pane) {
            self.hide(pane);
        } else {
            self.reveal(pane);
        }
    }

    /// Take `pane` out of the dock.
    pub fn hide(&mut self, pane: Pane) {
        if let Some(path) = self.find(pane) {
            self.dock.remove_tab(path);
        }
    }

    /// Make `pane` the tab showing in its leaf, putting it back in the dock if it is not
    /// there.
    pub fn reveal(&mut self, pane: Pane) {
        match self.find(pane) {
            Some(path) => {
                let _ = self.dock.set_active_tab(path);
                self.dock.set_focused_node_and_surface(path.node_path());
            }
            None => self.place(pane),
        }
    }

    /// The first leaf on the main surface holding a tab that satisfies `f`.
    fn main_leaf(&self, f: impl Fn(Pane) -> bool) -> Option<NodeIndex> {
        self.dock
            .main_surface()
            .iter()
            .enumerate()
            .find(|(_, n)| n.iter_tabs().any(|t| f(*t)))
            .map(|(i, _)| NodeIndex(i))
    }

    /// Put `pane` back where it belongs.
    fn place(&mut self, pane: Pane) {
        let width = if self.area.is_positive() {
            self.area.width()
        } else {
            1280.0
        };
        let inspector_share = (1.0 - self.inspector_w / width).clamp(0.5, 0.95);
        let lower = self.main_leaf(Pane::is_lower);
        let arrangement = self.main_leaf(|p| p == Pane::Arrangement);
        let inspector = self.main_leaf(|p| p == Pane::Inspector);
        let s = self.dock.main_surface_mut();
        if s.is_empty() {
            *s = egui_dock::Tree::new(vec![pane]);
            return;
        }
        let append = |s: &mut egui_dock::Tree<Pane>, node: NodeIndex, pane: Pane| {
            if let Node::Leaf(leaf) = &mut s[node] {
                leaf.tabs.push(pane);
                leaf.active = TabIndex(leaf.tabs.len() - 1);
            }
            s.set_focused_node(node);
        };
        match pane {
            p if p.is_lower() => match (lower, arrangement) {
                (Some(node), _) => append(s, node, p),
                (None, Some(node)) => {
                    s.split_below(node, ARRANGEMENT_SHARE, vec![p]);
                }
                (None, None) => match inspector {
                    Some(node) => {
                        s.split_left(node, inspector_share, vec![p]);
                    }
                    None => s.push_to_first_leaf(p),
                },
            },
            Pane::Arrangement => match (lower, inspector) {
                (Some(node), _) => {
                    s.split_above(node, ARRANGEMENT_SHARE, vec![Pane::Arrangement]);
                }
                (None, Some(node)) => {
                    s.split_left(node, inspector_share, vec![Pane::Arrangement]);
                }
                (None, None) => s.push_to_first_leaf(Pane::Arrangement),
            },
            Pane::Inspector => {
                s.split_right(NodeIndex::root(), inspector_share, vec![Pane::Inspector]);
            }
            // At the left edge, a browser's width (FL's, Ableton's).
            Pane::Devices => {
                // `split_left` gives the new (left) node `fraction` of the width.
                let share = (260.0 / width).clamp(0.12, 0.3);
                s.split_left(NodeIndex::root(), share, vec![Pane::Devices]);
            }
            Pane::Browser => {
                // At the left of everything, as wide as FL's browser. `split_left` gives
                // the new (left) node `fraction` of the width (egui_dock's SplitNode keeps
                // the left child's share, whatever its docs say).
                let share = (BROWSER_W / width).clamp(0.12, 0.3);
                s.split_left(NodeIndex::root(), share, vec![Pane::Browser]);
            }
            // Beside the arrangement, where FL keeps the playlist's picker panel: at the left
            // of the arrangement's area when it is open (its share of that area), else at the
            // left of everything.
            Pane::Clips => {
                let w = super::clip_browser::WIDTH;
                match arrangement {
                    Some(node) => {
                        // A fifth of the arrangement's area: about FL's picker panel beside a
                        // playlist, whatever the window measured when the pane was placed.
                        let share = (w / (width * inspector_share)).clamp(0.2, 0.3);
                        s.split_left(node, share, vec![Pane::Clips]);
                    }
                    None => {
                        let share = (w / width).clamp(0.1, 0.3);
                        s.split_left(NodeIndex::root(), share, vec![Pane::Clips]);
                    }
                }
            }
            Pane::Device(_) => match arrangement.or(lower) {
                Some(node) => append(s, node, pane),
                None => s.push_to_first_leaf(pane),
            },
            _ => s.push_to_first_leaf(pane),
        }
    }

    /// The status bar's lower-zone toggle: hide the whole lower zone, or bring back what it
    /// held.
    pub fn toggle_lower(&mut self) {
        let showing: Vec<Pane> = self
            .dock
            .iter_all_tabs()
            .map(|(_, p)| *p)
            .filter(|p| p.is_lower())
            .collect();
        if showing.is_empty() {
            let panes = if self.lower_hidden.is_empty() {
                vec![Pane::Mixer, Pane::PianoRoll, Pane::ChannelRack]
            } else {
                std::mem::take(&mut self.lower_hidden)
            };
            for p in panes {
                self.place(p);
            }
            if let Some(a) = self.lower_active.take() {
                self.reveal(a);
            }
        } else {
            self.lower_active = showing.iter().copied().find(|p| self.is_visible(*p));
            for p in &showing {
                self.hide(*p);
            }
            self.lower_hidden = showing;
        }
    }

    /// Whether any lower-zone editor is in the dock.
    pub fn lower_shown(&self) -> bool {
        self.dock.iter_all_tabs().any(|(_, p)| p.is_lower())
    }

    /// F12: close every floating device window.
    pub fn close_floating_devices(&mut self, ctx: &Context) {
        // One at a time: taking a tab out can move the others' nodes.
        loop {
            let next = self
                .dock
                .iter_all_tabs()
                .filter(|(path, _)| !path.surface.is_main())
                .find_map(|(path, p)| match p {
                    Pane::Device(k) => Some((path, *k)),
                    _ => None,
                });
            let Some((path, key)) = next else {
                break;
            };
            device::mark_closed(ctx, key);
            self.dock.remove_tab(path);
        }
    }

    /// Esc over a floating device window closes it. Returns whether one was closed.
    pub fn close_hovered_floating_device(&mut self, ctx: &Context) -> bool {
        let Some(pos) = ctx.pointer_hover_pos() else {
            return false;
        };
        let hit = self
            .dock
            .iter_all_tabs()
            .filter(|(path, _)| !path.surface.is_main())
            .find_map(|(path, p)| match p {
                Pane::Device(k) => {
                    let leaf = self.dock.leaf(path.node_path()).ok()?;
                    leaf.rect.contains(pos).then_some((path, *k))
                }
                _ => None,
            });
        match hit {
            Some((path, key)) => {
                device::mark_closed(ctx, key);
                self.dock.remove_tab(path);
                true
            }
            None => false,
        }
    }

    /// Float a docked pane as a window, or dock a floating one back where it belongs.
    pub fn toggle_float(&mut self, pane: Pane) {
        let Some(path) = self.find(pane) else {
            return;
        };
        if path.surface.is_main() {
            let size = self
                .dock
                .leaf(path.node_path())
                .map(|l| l.rect.size())
                .ok()
                .filter(|s| s.x > 1.0 && s.y > 1.0)
                .unwrap_or(Vec2::new(640.0, 420.0));
            let at = self.area.center() - size * 0.5;
            let at = if self.area.is_positive() {
                at
            } else {
                Pos2::new(120.0, 120.0)
            };
            self.dock.detach_tab(path, Rect::from_min_size(at, size));
        } else {
            self.dock.remove_tab(path);
            self.place(pane);
        }
    }

    /// Whether `pane` floats in a window of its own.
    pub fn is_floating(&self, pane: Pane) -> bool {
        self.find(pane).is_some_and(|p| !p.surface.is_main())
    }

    /// Impose a layout's sizes on the next frame: the inspector's width and the
    /// arrangement's height, in points.
    pub fn impose_sizes(&mut self, inspector: f32, arrangement: f32) {
        self.pending = Some((inspector, arrangement));
        if inspector > 0.0 {
            self.inspector_w = inspector;
        }
    }

    /// The inspector's width and the arrangement's height as laid out last frame, if they
    /// are showing.
    pub fn measured_sizes(&self) -> (Option<f32>, Option<f32>) {
        let size = |pane: Pane| {
            self.find(pane)
                .filter(|p| p.surface.is_main())
                .and_then(|p| self.dock.leaf(p.node_path()).ok())
                .map(|l| l.rect.size())
                .filter(|s| s.x.is_finite() && s.x > 0.0 && s.y.is_finite() && s.y > 0.0)
        };
        (
            size(Pane::Inspector).map(|s| s.x),
            size(Pane::Arrangement).map(|s| s.y),
        )
    }

    pub(super) fn memo(&mut self, path: NodePath) -> &mut SplitMemo {
        let (surface, node) = (path.surface.0, path.node.0);
        match self
            .splits
            .iter()
            .position(|m| m.surface == surface && m.node == node)
        {
            Some(i) => &mut self.splits[i],
            None => {
                self.splits.push(SplitMemo {
                    surface,
                    node,
                    ..Default::default()
                });
                self.splits.last_mut().expect("just pushed")
            }
        }
    }
}

// The frame: before the dock, the dock, after it.
pub use super::workspace_frame::show;

/// A tab's id in egui: stable across frames and layouts.
pub(crate) fn pane_id(p: Pane) -> Id {
    Id::new(("cantus-pane", p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_dock_holds_every_area_with_the_mixer_in_front() {
        let ws = Workspace::default();
        for p in Pane::ZONES {
            assert!(ws.contains(p), "{p:?} is in the default dock");
        }
        assert!(ws.is_visible(Pane::Arrangement));
        assert!(ws.is_visible(Pane::Mixer));
        assert!(ws.is_visible(Pane::Inspector));
        assert!(!ws.is_visible(Pane::PianoRoll), "behind the mixer's tab");
    }

    #[test]
    fn a_key_hides_what_shows_fronts_what_hides_and_restores_what_is_gone() {
        let mut ws = Workspace::default();
        // Behind a tab: brought to the front, not hidden.
        ws.toggle(Pane::PianoRoll);
        assert!(ws.is_visible(Pane::PianoRoll));
        assert!(!ws.is_visible(Pane::Mixer));
        // Showing: hidden.
        ws.toggle(Pane::PianoRoll);
        assert!(!ws.contains(Pane::PianoRoll));
        // Gone: back into the lower zone, beside the others, in front.
        ws.toggle(Pane::PianoRoll);
        assert!(ws.is_visible(Pane::PianoRoll));
        let lower = ws.find(Pane::Mixer).unwrap().node_path();
        assert_eq!(ws.find(Pane::PianoRoll).unwrap().node_path(), lower);
        // The arrangement comes back above the lower zone, the inspector at the right.
        ws.hide(Pane::Arrangement);
        ws.hide(Pane::Inspector);
        ws.reveal(Pane::Arrangement);
        ws.reveal(Pane::Inspector);
        assert!(ws.is_visible(Pane::Arrangement) && ws.is_visible(Pane::Inspector));
        match ws.dock.main_surface().root_node() {
            Some(Node::Horizontal(_)) => {}
            other => panic!("the inspector splits the root at the right: {other:?}"),
        }
    }

    #[test]
    fn everything_can_go_and_come_back() {
        let mut ws = Workspace::default();
        for p in Pane::ZONES {
            ws.hide(p);
        }
        assert!(ws.dock.iter_all_tabs().next().is_none());
        ws.reveal(Pane::Mixer);
        ws.reveal(Pane::Arrangement);
        assert!(ws.is_visible(Pane::Mixer) && ws.is_visible(Pane::Arrangement));
    }

    #[test]
    fn the_lower_zone_toggle_hides_all_three_and_brings_them_back_as_they_were() {
        let mut ws = Workspace::default();
        ws.reveal(Pane::ChannelRack);
        ws.toggle_lower();
        assert!(!ws.lower_shown());
        ws.toggle_lower();
        for p in [Pane::Mixer, Pane::PianoRoll, Pane::ChannelRack] {
            assert!(ws.contains(p));
        }
        assert!(
            ws.is_visible(Pane::ChannelRack),
            "the one in front stays in front"
        );
    }

    #[test]
    fn an_area_floats_and_docks_back() {
        let mut ws = Workspace::default();
        ws.toggle_float(Pane::Mixer);
        assert!(ws.is_floating(Pane::Mixer));
        ws.toggle_float(Pane::Mixer);
        assert!(!ws.is_floating(Pane::Mixer));
        assert!(ws.is_visible(Pane::Mixer));
    }

    #[test]
    fn a_device_docks_beside_the_arrangement() {
        let mut ws = Workspace::default();
        ws.reveal(Pane::Device(7));
        let at = ws.find(Pane::Device(7)).unwrap();
        assert!(at.surface.is_main(), "docked, not floating");
        assert_eq!(
            at.node_path(),
            ws.find(Pane::Arrangement).unwrap().node_path()
        );
        assert!(ws.is_visible(Pane::Device(7)));
    }
}
