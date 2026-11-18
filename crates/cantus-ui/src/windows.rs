//! **Windows of their own** (ADR 0151): any area of the workspace in an OS window with its own
//! dock, on whichever screen; the windows remembered per set of screens.
//!
//! - Each window is an egui **deferred viewport** with its own [`Workspace`]. Its callback
//!   captures only the window's key and reaches the application through the UI thread's
//!   cell ([`lend`], [`with_lent`]): `boot::Shell` lends it for the window's pass (eframe
//!   runs every pass on that thread, one after another). During a window's pass the window's
//!   dock is swapped into `app.workspace` and the main window's waits in the window's slot
//!   ([`Windows::current`]), so every view works unchanged.
//! - The **main window's pass is the clock**: the frame's once-a-tick work runs there, and
//!   the ticker asks each window for its next frame only while an area it shows moves
//!   ([`moves`]). Input repaints its own window; a click or a key in one window repaints the
//!   others once (the selection, the project). At rest nothing draws.
//! - **Places** are in the desktop's physical pixels (`crate::screens`). A window is made
//!   hidden, placed through its winit handle (`eframe::window_of`, a CANTUS PATCH) and shown
//!   once placed; one whose screen is gone docks its areas back into the main window and
//!   returns with the screen (§8).
//! - **Scale**: each window draws at its own screen's interface scale (the follow rule of
//!   ADR 0111 §7 for its screen, or the user's own for that window), given by its input
//!   before its pass ([`adjust_input`]); egui's zoom itself never changes per pass.

use crate::screens::{self, Place, PxRect, ScreenKey};
use crate::views::workspace::{Pane, Workspace};
use egui::ViewportId;
use serde::{Deserialize, Serialize};

/// The most windows besides the main one.
pub const MAX: usize = 8;
/// While a window waits for its screen to come back, the screens are read again this often
/// (one pass of the main window; nothing else wakes for it).
pub const PARKED_LOOK: std::time::Duration = std::time::Duration::from_secs(3);

/// Where a tab is sent.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    /// A new window — on screen `screen` (an index into the screens read last), or on the
    /// screen the main window is not on when there is one.
    NewWindow { screen: Option<usize> },
    /// A new window at this place on the desktop (a tab torn out of a window).
    NewWindowAt(PxRect),
    /// The main window's dock.
    Main,
    /// The window with this key.
    Window(u32),
}

/// A window as a layout keeps it (`layouts.json`, the user's — never the project's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowShape {
    pub key: u32,
    /// The user's name for it; empty: named after its areas.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Its dock's tree, as `layout::dock_to_value` writes it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dock: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<Place>,
    /// Its bar with the transport (§4).
    #[serde(default = "yes")]
    pub bar: bool,
    #[serde(default)]
    pub on_top: bool,
    /// The user's own interface scale for it; `None` follows its screen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
}

fn yes() -> bool {
    true
}

impl WindowShape {
    /// Its dock, or `None` when the tree does not read (a file from elsewhere) or is empty.
    pub fn dock_state(&self) -> Option<egui_dock::DockState<Pane>> {
        self.dock
            .as_ref()
            .and_then(|v| serde_json::from_value::<egui_dock::DockState<Pane>>(v.clone()).ok())
            .filter(|d| d.iter_all_tabs().next().is_some())
    }

    /// The areas its dock holds.
    pub fn panes(&self) -> Vec<Pane> {
        self.dock_state()
            .map(|d| d.iter_all_tabs().map(|(_, p)| *p).collect())
            .unwrap_or_default()
    }
}

/// Which dock holds an area: the main window's, or window `i`'s (`Windows::list`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Dock {
    Main,
    Window(usize),
}

/// What the system said of a window at its last pass.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Seen {
    /// Its outer rectangle in physical pixels, when the system tells it.
    pub outer: Option<PxRect>,
    /// The system's scale where it is.
    pub native: f32,
    pub minimized: bool,
    pub focused: bool,
    pub maximized: bool,
    pub fullscreen: bool,
}

/// A window of its own, while it is open.
pub struct OsWindow {
    pub key: u32,
    pub name: String,
    /// Its dock — or, during its own pass, the main window's (swapped, see the module).
    pub workspace: Workspace,
    /// Where it is (or is to be put).
    pub place: Option<Place>,
    pub bar: bool,
    pub on_top: bool,
    /// The user's own interface scale for it.
    pub scale: Option<f32>,
    /// The interface scale it draws at now (its screen's, or `scale`).
    pub zoom: f32,
    /// The factor its input was given last pass: its zoom over egui's.
    pub factor: f32,
    pub seen: Seen,
    /// Passes it has drawn.
    pub passes: u64,
    /// Placed on its screen and shown.
    pub shown: bool,
    /// Its close button was pressed: its areas go back to the main window.
    pub closing: bool,
    pub watch: crate::window_watch::Watch,
}

impl OsWindow {
    pub fn id(&self) -> ViewportId {
        id_of(self.key)
    }

    pub fn shape(&self, main_dock: Option<&Workspace>) -> WindowShape {
        let ws = main_dock.unwrap_or(&self.workspace);
        WindowShape {
            key: self.key,
            name: self.name.clone(),
            dock: crate::layout::dock_to_value(&ws.dock),
            place: self.place.clone(),
            bar: self.bar,
            on_top: self.on_top,
            scale: self.scale,
        }
    }
}

/// The pass's number, told apart per window (ADR 0151): egui counts passes per viewport,
/// so the main window's pass 100 and a window's pass 100 would be "the same pass" to the
/// application's once-a-pass guards (the keys, Esc, a text field's focus). The main window
/// keeps egui's count; a window's carries its viewport in the high bits.
pub fn pass_nr(ctx: &egui::Context) -> u64 {
    let n = ctx.cumulative_pass_nr();
    let vp = ctx.viewport_id();
    if vp == ViewportId::ROOT {
        n
    } else {
        n ^ (vp.0.value() & 0xFFFF_FF00_0000_0000).max(1 << 40)
    }
}

/// The viewport of the window with key `key`.
pub fn id_of(key: u32) -> ViewportId {
    ViewportId::from_hash_of(("cantus-window", key))
}

/// Every window of its own, and what the screens are.
pub struct Windows {
    pub list: Vec<OsWindow>,
    next_key: u32,
    /// The window whose pass is running (its dock is in `app.workspace`, the main window's in
    /// its slot); `None` in the main window's pass.
    pub current: Option<usize>,
    /// The window the user last clicked or typed in: dialogs open over it, a device editor
    /// opened now opens in it.
    pub host: ViewportId,
    /// The screens as read last (physical pixels), and their fingerprint.
    pub screens: Vec<ScreenKey>,
    pub fingerprint: String,
    /// Screens fed by a test (`None` in the program: read from winit).
    pub fed_screens: Option<Vec<ScreenKey>>,
    /// Read the screens again at the next pass of the main window.
    pub stale: bool,
    /// Windows waiting for their screen to come back, as they were.
    pub parked: Vec<WindowShape>,
    /// Where a window that closed by having its last area hidden was, by area: the area's
    /// key brings the window back (F5–F9 on an area that lived in a window).
    pub homes: Vec<(Pane, WindowShape)>,
    /// When the screens were last read (the main window's clock, seconds).
    pub(crate) read_at: f64,
    /// What the system said of the main window at its last pass.
    pub main: Seen,
    /// The follow rule's interface scale for the main window's screen (ADR 0111 §7): a
    /// window's scale is measured against it.
    pub main_rule: f32,
    /// The screen last found missing (for the status line's words).
    pub missing: Option<ScreenKey>,
    /// A layout asked for during a window's pass, applied at the main window's next pass.
    pub pending: Option<crate::layout::Layout>,
    /// The window whose menu is open, and where (its bar's ⋯ key).
    pub menu: Option<(u32, egui::Pos2)>,
    /// View ▸ *Screens and windows…* is open (§9).
    pub map_open: bool,
}

impl Default for Windows {
    fn default() -> Self {
        Windows {
            list: Vec::with_capacity(MAX),
            next_key: 1,
            current: None,
            host: ViewportId::ROOT,
            screens: Vec::new(),
            fingerprint: String::new(),
            fed_screens: None,
            stale: true,
            parked: Vec::new(),
            homes: Vec::new(),
            read_at: f64::NEG_INFINITY,
            main: Seen::default(),
            main_rule: 1.0,
            missing: None,
            pending: None,
            menu: None,
            map_open: false,
        }
    }
}

impl Windows {
    /// The index of the window with key `key`.
    pub fn index(&self, key: u32) -> Option<usize> {
        self.list.iter().position(|w| w.key == key)
    }

    /// The index of the window drawn in viewport `id`.
    pub fn index_of(&self, id: ViewportId) -> Option<usize> {
        self.list.iter().position(|w| w.id() == id)
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    fn take_key(&mut self, wanted: Option<u32>) -> u32 {
        let taken = |k: u32, list: &[OsWindow]| list.iter().any(|w| w.key == k);
        if let Some(k) = wanted.filter(|k| *k > 0 && !taken(*k, &self.list)) {
            self.next_key = self.next_key.max(k + 1);
            return k;
        }
        while taken(self.next_key, &self.list) {
            self.next_key += 1;
        }
        self.next_key += 1;
        self.next_key - 1
    }

    /// A window made from `shape` (its dock given, or an empty one).
    pub fn open(&mut self, shape: &WindowShape, dock: Workspace) -> Option<usize> {
        if self.list.len() >= MAX {
            return None;
        }
        let key = self.take_key(Some(shape.key));
        self.list.push(OsWindow {
            key,
            name: shape.name.clone(),
            workspace: dock,
            place: shape.place.clone(),
            bar: shape.bar,
            on_top: shape.on_top,
            scale: shape.scale,
            zoom: 1.0,
            factor: 1.0,
            seen: Seen::default(),
            passes: 0,
            shown: false,
            closing: false,
            watch: crate::window_watch::Watch::for_window(),
        });
        Some(self.list.len() - 1)
    }

    /// The screen the main window is on (by its last place), if the screens are known.
    pub fn main_screen(&self, main: Option<&PxRect>) -> Option<usize> {
        main.and_then(|r| screens::screen_of(r, &self.screens))
            .or(if self.screens.is_empty() {
                None
            } else {
                Some(0)
            })
    }
}
