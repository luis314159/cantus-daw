//! Named window layouts: which areas are where, how big, and which are showing.
//!
//! A DAW is used in phases — arranging, editing notes, mixing — and each wants a different
//! shape of window. Rather than making the user drag the same splitters back and forth,
//! Cantus lets them name a shape and return to it.
//!
//! Since ADR 0024 §5 the window is a dock ([`crate::views::workspace`]): every area is a tab
//! that can be dragged, split, stacked or floated. A layout keeps
//!
//! - **the dock's tree**, as `egui_dock` serialises it (device editors left out: those are
//!   opened by the project, not by a layout);
//! - **the inspector's width and the arrangement's height in points**, imposed on the frame
//!   after the layout is applied, so a layout means the same thing in a bigger window;
//! - the older fields — which panels show, which editor is in front — kept up to date,
//!   so a layout file stays readable by the release before the dock.
//!
//! **Migration.** A layout written before the dock has no tree. It becomes the default
//! dock with its hidden panels taken out and its central view in front, and its sizes are
//! imposed as before ([`Layout::dock_state`]).
//!
//! eframe is built without its `persistence` feature, so the layouts file is what carries
//! the window's shape across restarts: the app captures the current layout on exit and
//! applies it on startup.

use crate::views::workspace::{default_dock, Pane, Workspace, INSPECTOR_W};
use egui::Rect;
use egui_dock::{DockState, Node};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// The panels the older layouts described, by name.
pub const INSPECTOR: &str = "inspector";
pub const ARRANGEMENT: &str = "arrangement";

/// Which editor is in front in the lower zone.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Central {
    PianoRoll,
    ChannelRack,
    Mixer,
}

impl Central {
    pub fn pane(self) -> Pane {
        match self {
            Central::PianoRoll => Pane::PianoRoll,
            Central::ChannelRack => Pane::ChannelRack,
            Central::Mixer => Pane::Mixer,
        }
    }
}

/// One named arrangement of the window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub name: String,
    /// Width of the inspector, in points.
    pub inspector: f32,
    /// Height of the arrangement, in points; 0 keeps the dock's own share.
    pub arrangement: f32,
    pub show_inspector: bool,
    pub show_arrangement: bool,
    pub central: Central,
    /// The dock's tree. `None` in a layout from before the dock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dock: Option<serde_json::Value>,
}

impl Layout {
    /// The layout a fresh install opens with, and what *Reset layout* returns to.
    pub fn default_layout() -> Layout {
        Layout {
            // The key i18n string is resolved at display time, so the stored name stays
            // stable when the user switches language.
            name: DEFAULT_NAME.to_string(),
            inspector: INSPECTOR_W,
            arrangement: 0.0,
            show_inspector: true,
            show_arrangement: true,
            central: Central::Mixer,
            dock: dock_to_value(&default_dock()),
        }
    }

    /// The dock this layout describes — its own tree, or, for a layout from before the
    /// dock (or a tree that no longer reads), the default dock shaped by its old fields.
    pub fn dock_state(&self) -> DockState<Pane> {
        if let Some(d) = self
            .dock
            .as_ref()
            .and_then(|v| serde_json::from_value::<DockState<Pane>>(v.clone()).ok())
            .filter(|d| d.iter_all_tabs().next().is_some())
        {
            return d;
        }
        self.migrate()
    }

    /// The default dock with this old layout's panels hidden and its central view in front.
    pub fn migrate(&self) -> DockState<Pane> {
        let mut ws = Workspace::with_dock(default_dock());
        if !self.show_inspector {
            ws.hide(Pane::Inspector);
        }
        if !self.show_arrangement {
            ws.hide(Pane::Arrangement);
        }
        ws.reveal(self.central.pane());
        ws.dock
    }

    /// This layout with the workspace's shape folded in: its tree, the sizes laid out
    /// last frame, which areas show and which editor is in front.
    pub fn capture(&self, ws: &Workspace, central: Central) -> Layout {
        let (inspector, arrangement) = ws.measured_sizes();
        Layout {
            inspector: inspector.unwrap_or(self.inspector),
            arrangement: arrangement.unwrap_or(self.arrangement),
            show_inspector: ws.contains(Pane::Inspector),
            show_arrangement: ws.contains(Pane::Arrangement),
            central,
            dock: dock_to_value(&ws.dock),
            ..self.clone()
        }
    }

    /// This layout's size for a panel, or `None` for a panel it does not describe.
    pub fn size_of(&self, panel: &str) -> Option<f32> {
        match panel {
            INSPECTOR => Some(self.inspector),
            ARRANGEMENT => Some(self.arrangement),
            _ => None,
        }
    }

    pub fn shows(&self, panel: &str) -> bool {
        match panel {
            INSPECTOR => self.show_inspector,
            ARRANGEMENT => self.show_arrangement,
            _ => true,
        }
    }

    /// The label to show: built-in layouts are translated, user-named ones are not.
    pub fn display_name(&self) -> String {
        if self.name == DEFAULT_NAME {
            crate::i18n::t("layout.default")
        } else {
            self.name.clone()
        }
    }
}

/// The dock as a layout file keeps it: device editors left out, and the rects egui_dock
/// fills in every frame zeroed (an unlaid-out rect is infinite, which JSON cannot hold).
pub fn dock_to_value(d: &DockState<Pane>) -> Option<serde_json::Value> {
    let mut d = d.filter_tabs(|p| !matches!(p, Pane::Device(_)));
    // Filtering drops the windows that held only devices, and with them the index a
    // focus may have pointed at: focus the main surface's first leaf instead.
    let first = d
        .main_surface()
        .iter()
        .position(|n| n.is_leaf())
        .map(egui_dock::NodeIndex);
    if let Some(node) = first {
        d.set_focused_node_and_surface(egui_dock::NodePath {
            surface: egui_dock::SurfaceIndex::main(),
            node,
        });
    }
    for (_, node) in d.iter_all_nodes_mut() {
        match node {
            Node::Leaf(l) => {
                l.rect = Rect::ZERO;
                l.viewport = Rect::ZERO;
                l.scroll = 0.0;
            }
            Node::Horizontal(s) | Node::Vertical(s) => s.rect = Rect::ZERO,
            Node::Empty => {}
        }
    }
    serde_json::to_value(&d).ok()
}

/// The stored name of the built-in layout. Never shown raw — see [`Layout::display_name`].
pub const DEFAULT_NAME: &str = "@default";

/// Every layout the user has, and which one is in use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layouts {
    pub current: Layout,
    #[serde(default)]
    pub saved: Vec<Layout>,
    /// Frames left in which the current layout is to be imposed on the workspace. One is
    /// enough; the field exists so `apply` can be called from a menu and take effect on the
    /// next frame, whenever that is.
    #[serde(skip)]
    forcing: u8,
}

impl Default for Layouts {
    fn default() -> Self {
        Layouts {
            current: Layout::default_layout(),
            saved: Vec::new(),
            forcing: 0,
        }
    }
}

impl Layouts {
    /// Save the current shape under `name`, replacing a layout of the same name.
    pub fn save_as(&mut self, name: &str, ws: &Workspace, central: Central) {
        let mut layout = self.current.capture(ws, central);
        layout.name = name.to_string();
        match self.saved.iter_mut().find(|l| l.name == name) {
            Some(existing) => *existing = layout.clone(),
            None => self.saved.push(layout.clone()),
        }
        self.current = layout;
    }

    /// Switch to a saved layout by name. Returns false if there is no such layout.
    pub fn apply_named(&mut self, name: &str) -> bool {
        let Some(layout) = self.saved.iter().find(|l| l.name == name).cloned() else {
            return false;
        };
        self.current = layout;
        self.force();
        true
    }

    /// Impose the current layout on the next frame.
    pub fn force(&mut self) {
        self.forcing = 1;
    }

    /// Whether the current layout is to be imposed this frame.
    pub fn is_forcing(&self) -> bool {
        self.forcing > 0
    }

    /// The size the current layout imposes on `panel` this frame, if it is being applied.
    /// `None` means "leave it to the dock and the user".
    pub fn forced(&self, panel: &str) -> Option<f32> {
        (self.forcing > 0)
            .then(|| self.current.size_of(panel))
            .flatten()
    }

    /// Call once per frame, after the layout has been imposed.
    pub fn end_frame(&mut self) {
        self.forcing = self.forcing.saturating_sub(1);
    }

    pub fn delete(&mut self, name: &str) {
        self.saved.retain(|l| l.name != name);
        if self.current.name == name {
            self.current.name = DEFAULT_NAME.to_string();
        }
    }

    /// Back to the built-in shape, keeping the saved layouts.
    pub fn reset(&mut self) {
        self.current = Layout::default_layout();
        self.force();
    }

    /// Fold the workspace's shape into `current`, so what gets written to disk is what the
    /// user is actually looking at.
    pub fn sync(&mut self, ws: &Workspace, central: Central) {
        self.current = self.current.capture(ws, central);
    }

    pub fn load(path: &Path) -> Layouts {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| e.to_string())
    }
}

/// Where the layouts file lives, next to the theme.
pub fn layouts_path(dir: &Path) -> PathBuf {
    dir.join("layouts.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A layouts file as the release before the dock wrote it.
    const OLD: &str = r#"{
      "current": {"name": "Mixing", "inspector": 320.0, "arrangement": 200.0,
                  "show_inspector": false, "show_arrangement": true, "central": "mixer"},
      "saved": [{"name": "Mixing", "inspector": 320.0, "arrangement": 200.0,
                 "show_inspector": false, "show_arrangement": true, "central": "mixer"},
                {"name": "Notes", "inspector": 300.0, "arrangement": 180.0,
                 "show_inspector": true, "show_arrangement": false, "central": "piano_roll"}]
    }"#;

    #[test]
    fn an_old_layouts_file_loads_and_migrates_to_the_dock() {
        let dir = tempfile::tempdir().unwrap();
        let path = layouts_path(dir.path());
        std::fs::write(&path, OLD).unwrap();
        let l = Layouts::load(&path);
        assert_eq!(l.current.name, "Mixing");
        assert!(l.current.dock.is_none(), "an old file has no tree");
        let d = l.current.dock_state();
        let ws = Workspace::with_dock(d);
        assert!(
            !ws.contains(Pane::Inspector),
            "its hidden inspector stays hidden"
        );
        assert!(ws.contains(Pane::Arrangement));
        assert!(ws.is_visible(Pane::Mixer), "its central view is in front");
        let notes = Workspace::with_dock(l.saved[1].dock_state());
        assert!(!notes.contains(Pane::Arrangement));
        assert!(notes.is_visible(Pane::PianoRoll));
        assert!(notes.contains(Pane::Inspector));
    }

    #[test]
    fn layouts_are_named_saved_switched_and_reset() {
        let mut layouts = Layouts::default();
        let mut ws = Workspace::default();
        ws.hide(Pane::Inspector);
        layouts.save_as("Mixing", &ws, Central::Mixer);
        assert_eq!(layouts.saved.len(), 1);
        assert_eq!(layouts.current.name, "Mixing");
        assert!(!layouts.saved[0].show_inspector);
        // Saving the same name again replaces rather than duplicates.
        layouts.save_as("Mixing", &ws, Central::Mixer);
        assert_eq!(layouts.saved.len(), 1);

        assert!(!layouts.apply_named("Nope"));
        assert!(layouts.apply_named("Mixing"));
        assert!(layouts.is_forcing(), "switching layouts must impose it");
        let back = Workspace::with_dock(layouts.current.dock_state());
        assert!(!back.contains(Pane::Inspector));
        layouts.end_frame();
        assert!(!layouts.is_forcing());

        layouts.reset();
        assert_eq!(layouts.current.name, DEFAULT_NAME);
        assert_eq!(layouts.saved.len(), 1, "reset keeps the saved layouts");
        assert!(layouts.is_forcing(), "reset must impose the default");
        let d = Workspace::with_dock(layouts.current.dock_state());
        for p in [Pane::Arrangement, Pane::Inspector, Pane::Mixer] {
            assert!(d.contains(p), "the default dock holds {p:?}");
        }

        layouts.delete("Mixing");
        assert!(layouts.saved.is_empty());
    }

    #[test]
    fn deleting_the_layout_in_use_falls_back_to_the_default_name() {
        let mut layouts = Layouts::default();
        layouts.save_as("Mixing", &Workspace::default(), Central::Mixer);
        layouts.delete("Mixing");
        assert_eq!(layouts.current.name, DEFAULT_NAME);
    }

    #[test]
    fn layouts_round_trip_through_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = layouts_path(dir.path());
        // A missing file is not an error: it is a fresh install.
        assert_eq!(Layouts::load(&path), Layouts::default());

        let mut layouts = Layouts::default();
        let mut ws = Workspace::default();
        ws.toggle_float(Pane::Mixer);
        ws.hide(Pane::Arrangement);
        layouts.sync(&ws, Central::Mixer);
        layouts.saved.push(Layout {
            name: "Tracking".into(),
            ..Layout::default_layout()
        });
        layouts.save(&path).unwrap();
        let back = Layouts::load(&path);
        assert_eq!(back, layouts);
        let ws = Workspace::with_dock(back.current.dock_state());
        assert!(
            ws.is_floating(Pane::Mixer),
            "a floating area stays floating"
        );
        assert!(!ws.contains(Pane::Arrangement));

        // A corrupt file must not stop the app from opening.
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Layouts::load(&path), Layouts::default());
    }

    #[test]
    fn device_tabs_are_not_saved() {
        let mut ws = Workspace::default();
        ws.reveal(Pane::Device(42));
        assert!(ws.contains(Pane::Device(42)));
        let v = dock_to_value(&ws.dock).unwrap();
        let back: DockState<Pane> = serde_json::from_value(v).unwrap();
        assert!(back.find_tab(&Pane::Device(42)).is_none());
    }

    #[test]
    fn the_built_in_layout_shows_a_translated_name() {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let d = Layout::default_layout();
        assert_ne!(d.display_name(), DEFAULT_NAME);
        let custom = Layout {
            name: "Mixing".into(),
            ..d
        };
        assert_eq!(custom.display_name(), "Mixing");
    }
}
