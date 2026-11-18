//! The docks of every window (ADR 0148 §§1–3): which one holds an area, moving an area
//! between windows, a window that empties closes and remembers where it was.

use crate::app::CantusApp;
use crate::screens::{self, ScreenKey};
use crate::views::workspace::{Pane, Workspace};
use crate::windows::{Dock, Target, WindowShape, MAX};
use egui::{Context, ViewportId};

impl CantusApp {
    /// The main window's dock, wherever it waits during this pass.
    pub fn main_dock(&mut self) -> &mut Workspace {
        match self.windows.current {
            Some(i) => &mut self.windows.list[i].workspace,
            None => &mut self.workspace,
        }
    }

    /// The dock `d`, wherever it is during this pass.
    pub fn dock(&self, d: Dock) -> &Workspace {
        match (d, self.windows.current) {
            (Dock::Main, Some(i)) => &self.windows.list[i].workspace,
            (Dock::Main, None) => &self.workspace,
            (Dock::Window(i), Some(c)) if i == c => &self.workspace,
            (Dock::Window(i), _) => &self.windows.list[i].workspace,
        }
    }

    pub fn dock_mut(&mut self, d: Dock) -> &mut Workspace {
        match (d, self.windows.current) {
            (Dock::Main, _) => self.main_dock(),
            (Dock::Window(i), Some(c)) if i == c => &mut self.workspace,
            (Dock::Window(i), _) => &mut self.windows.list[i].workspace,
        }
    }

    /// The dock drawn in this pass.
    pub fn here(&self) -> Dock {
        self.windows.current.map_or(Dock::Main, Dock::Window)
    }

    /// Every dock, the main window's first.
    pub fn docks(&self) -> impl Iterator<Item = (Dock, &Workspace)> {
        std::iter::once(Dock::Main)
            .chain((0..self.windows.list.len()).map(Dock::Window))
            .map(move |d| (d, self.dock(d)))
    }

    /// The dock holding `pane`, if any does.
    pub fn dock_of(&self, pane: Pane) -> Option<Dock> {
        self.docks().find(|(_, w)| w.contains(pane)).map(|(d, _)| d)
    }

    /// Whether some window's dock holds `pane` (it may be behind another tab).
    pub fn pane_shown(&self, pane: Pane) -> bool {
        self.dock_of(pane).is_some()
    }

    /// Whether `pane` is the tab showing in its leaf, in whichever window.
    pub fn pane_visible(&self, pane: Pane) -> bool {
        self.dock_of(pane)
            .is_some_and(|d| self.dock(d).is_visible(pane))
    }

    /// The viewport a dock is drawn in.
    pub fn viewport_of(&self, d: Dock) -> ViewportId {
        match d {
            Dock::Main => ViewportId::ROOT,
            Dock::Window(i) => self.windows.list[i].id(),
        }
    }

    /// Bring `pane` to the front where it is — its window raised — or put it back where it
    /// belongs: the window it last lived in, else the main window.
    pub fn reveal_pane(&mut self, pane: Pane) {
        match self.dock_of(pane) {
            Some(d) => {
                self.dock_mut(d).reveal(pane);
                self.raise(d);
            }
            None => {
                if !self.reopen_home(pane) {
                    self.main_dock().reveal(pane);
                }
            }
        }
    }

    /// Take `pane` out of whichever dock holds it. A window left with nothing closes, and
    /// remembers where it was: the area's key brings it back there.
    pub fn hide_pane(&mut self, pane: Pane) {
        let Some(d) = self.dock_of(pane) else {
            return;
        };
        if let Dock::Window(i) = d {
            let shape = self.window_shape(i);
            self.windows.homes.retain(|(p, _)| *p != pane);
            self.windows.homes.push((pane, shape));
            if self.windows.homes.len() > 32 {
                self.windows.homes.remove(0);
            }
        }
        self.dock_mut(d).hide(pane);
        self.close_emptied();
    }

    /// Show or hide `pane` wherever it lives (F5, F6, F7, F9, I): hidden from a window of
    /// its own, it comes back to that window.
    pub fn toggle_pane_anywhere(&mut self, pane: Pane) {
        match self.dock_of(pane) {
            Some(Dock::Window(i)) => {
                if self.dock(Dock::Window(i)).is_visible(pane) {
                    self.hide_pane(pane);
                } else {
                    self.dock_mut(Dock::Window(i)).reveal(pane);
                    self.raise(Dock::Window(i));
                }
            }
            Some(Dock::Main) => self.main_dock().toggle(pane),
            None => {
                if !self.reopen_home(pane) {
                    self.main_dock().toggle(pane);
                }
            }
        }
    }

    /// Raise the window drawing dock `d` (a key acted on an area in another window).
    pub(crate) fn raise(&self, d: Dock) {
        let Some(ctx) = self.theme_ctx.as_ref() else {
            return;
        };
        let id = self.viewport_of(d);
        if id != ctx.viewport_id() {
            ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Focus);
        }
    }

    /// The shape of window `i` now (its dock wherever it waits).
    pub fn window_shape(&self, i: usize) -> WindowShape {
        self.windows.list[i].shape(Some(self.dock(Dock::Window(i))))
    }

    /// The window `pane` last lived in, opened again with it. `false` when it had none, or
    /// no window can open.
    fn reopen_home(&mut self, pane: Pane) -> bool {
        let Some(at) = self.windows.homes.iter().position(|(p, _)| *p == pane) else {
            return false;
        };
        let (_, shape) = self.windows.homes.remove(at);
        let ws = Workspace::with_dock(egui_dock::DockState::new(vec![pane]));
        let shape = WindowShape {
            dock: None,
            ..shape
        };
        self.open_window(&shape, ws).is_some()
    }

    /// Open a window from `shape` with dock `ws` — its areas taken out of every other dock
    /// first, so an area is never in two — placed on the screens there are, or parked (its
    /// areas left in the main window) when its screen is gone.
    pub fn open_window(&mut self, shape: &WindowShape, mut ws: Workspace) -> Option<usize> {
        let panes = ws.panes();
        if panes.is_empty() || self.windows.list.len() >= MAX {
            return None;
        }
        let mut shape = shape.clone();
        if let Some(place) = &shape.place {
            if !self.windows.screens.is_empty() {
                match screens::settle(place, &self.windows.screens) {
                    Some(p) => shape.place = Some(p),
                    None => {
                        shape.dock = crate::layout::dock_to_value(&ws.dock);
                        self.park_shape(shape);
                        return None;
                    }
                }
            }
        }
        for p in &panes {
            if let Some(d) = self.dock_of(*p) {
                self.dock_mut(d).hide(*p);
            }
        }
        ws.adds_devices = false;
        let at = self.windows.open(&shape, ws)?;
        let key = self.windows.list.get(at)?.key;
        self.close_emptied();
        if let Some(ctx) = &self.theme_ctx {
            ctx.request_repaint_of(ViewportId::ROOT);
        }
        self.windows.index(key)
    }

    /// A window whose screen is gone: its areas into the main window, its shape kept to
    /// come back with the screen.
    pub(crate) fn park_shape(&mut self, shape: WindowShape) {
        for p in shape.panes() {
            if self.dock_of(p).is_none() {
                self.main_dock().reveal(p);
            }
        }
        self.windows.parked.retain(|w| w.key != shape.key);
        self.windows.parked.push(shape);
    }

    /// Mark the windows left with no area to close (they are gone at the main window's
    /// next pass).
    pub(crate) fn close_emptied(&mut self) {
        for i in 0..self.windows.list.len() {
            if self.dock(Dock::Window(i)).is_empty() {
                self.windows.list[i].closing = true;
            }
        }
    }

    /// Send `pane` to `to` (a tab's menu, a dragged tab, the View menu).
    pub fn move_pane(&mut self, ctx: &Context, pane: Pane, to: Target) {
        let Some(from) = self.dock_of(pane) else {
            return;
        };
        match to {
            Target::NewWindow { screen } => {
                if self.windows.list.len() >= MAX {
                    self.set_status(crate::i18n::t("screens.full"));
                    return;
                }
                let screen = screen.or_else(|| {
                    let main = self.windows.main.outer;
                    let m = self.windows.main_screen(main.as_ref());
                    screens::other_screen(&self.windows.screens, m).or(m)
                });
                let place = screen.and_then(|s| screens::fresh_place(&self.windows.screens, s));
                let shape = WindowShape {
                    key: 0,
                    name: String::new(),
                    dock: None,
                    place,
                    bar: true,
                    groups: Vec::new(),
                    on_top: false,
                    scale: None,
                };
                let ws = Workspace::with_dock(egui_dock::DockState::new(vec![pane]));
                if let Some(i) = self.open_window(&shape, ws) {
                    self.windows.host = self.windows.list[i].id();
                }
            }
            Target::NewWindowAt(rect) => {
                if self.windows.list.len() >= MAX {
                    self.set_status(crate::i18n::t("screens.full"));
                    return;
                }
                let place = screens::screen_of(&rect, &self.windows.screens).map(|i| {
                    crate::screens::Place {
                        screen: self.windows.screens[i].clone(),
                        rect,
                        maximised: false,
                        full_screen: false,
                    }
                });
                let shape = WindowShape {
                    key: 0,
                    name: String::new(),
                    dock: None,
                    place,
                    bar: true,
                    groups: Vec::new(),
                    on_top: false,
                    scale: None,
                };
                let ws = Workspace::with_dock(egui_dock::DockState::new(vec![pane]));
                if let Some(i) = self.open_window(&shape, ws) {
                    self.windows.host = self.windows.list[i].id();
                }
            }
            Target::Main => {
                if from != Dock::Main {
                    self.dock_mut(from).hide(pane);
                    self.main_dock().reveal(pane);
                }
            }
            Target::Window(key) => {
                let Some(i) = self.windows.index(key) else {
                    return;
                };
                if from != Dock::Window(i) {
                    self.dock_mut(from).hide(pane);
                    self.dock_mut(Dock::Window(i)).reveal(pane);
                }
            }
        }
        self.close_emptied();
        // Every window draws its new shape once.
        ctx.request_repaint_of(ViewportId::ROOT);
        for w in &self.windows.list {
            ctx.request_repaint_of(w.id());
        }
    }

    /// A tab let go at `at` (points of the window drawn now) outside its window: into the
    /// window of Cantus's under that point, else a window of its own there, half its
    /// screen's size, centred on the point.
    pub fn tear_out(&mut self, ctx: &Context, pane: Pane, at: egui::Pos2) {
        let seen = match self.windows.current {
            Some(i) => self.windows.list[i].seen,
            None => self.windows.main,
        };
        let ppp = ctx.pixels_per_point();
        let origin = seen.outer.map_or((0, 0), |r| (r.x, r.y));
        let p = (
            origin.0 + (at.x * ppp).round() as i32,
            origin.1 + (at.y * ppp).round() as i32,
        );
        // Over another of Cantus's windows: into its dock.
        let here = ctx.viewport_id();
        for w in &self.windows.list {
            if w.id() != here && w.seen.outer.is_some_and(|r| r.contains(p)) {
                let key = w.key;
                return self.move_pane(ctx, pane, Target::Window(key));
            }
        }
        if here != ViewportId::ROOT && self.windows.main.outer.is_some_and(|r| r.contains(p)) {
            return self.move_pane(ctx, pane, Target::Main);
        }
        let s = screens::screen_of(
            &crate::screens::PxRect {
                x: p.0,
                y: p.1,
                w: 1,
                h: 1,
            },
            &self.windows.screens,
        )
        .and_then(|i| self.windows.screens.get(i).cloned());
        let (w, h) = s.map_or((960, 640), |s| (s.size[0] as i32 / 2, s.size[1] as i32 / 2));
        let rect = crate::screens::PxRect {
            x: p.0 - w / 2,
            y: p.1 - 16,
            w,
            h,
        };
        self.move_pane(ctx, pane, Target::NewWindowAt(rect));
    }

    /// The window a plug-in editor opened now belongs to (ADR 0148 §10): the window of its
    /// own the user is working in — its editor opens on that window's screen, a little in
    /// from its corner — else the main window.
    pub fn editor_owner(&mut self) -> Option<raw_window_handle::RawWindowHandle> {
        let host = self.windows.host;
        if let Some(i) = self.windows.index_of(host) {
            let w = &self.windows.list[i];
            if let Some(raw) = w.native {
                let at = w.seen.outer.map(|r| {
                    let inset = (48.0 * w.seen.native.max(1.0)) as i32;
                    (r.x + inset, r.y + inset)
                });
                cantus_host::window::place_next(at);
                return Some(raw);
            }
        }
        cantus_host::window::place_next(None);
        self.plugins.owner_window
    }

    /// Before window `i` goes: its plug-in editors given to the main window, or Windows
    /// would destroy them with it (ADR 0148 §10).
    pub(crate) fn release_editors(&mut self, i: usize) {
        let Some(raw) = self.windows.list.get(i).and_then(|w| w.native) else {
            return;
        };
        let id = crate::windows::handle_id(&raw);
        let main = self.plugins.owner_window;
        let n = self.plugins.reown_windows(id, main);
        if n > 0 {
            log::info!(
                "{n} plug-in editor window(s) given to the main window before their window closed"
            );
        }
        self.windows.released.push(id);
        if self.windows.released.len() > 64 {
            self.windows.released.remove(0);
        }
    }

    /// Where a tab of the dock `here` can be sent: the main window, every other window by
    /// name, a new window on each screen (named as the system names it).
    pub fn move_targets(&self, here: Dock) -> Vec<(String, Target)> {
        use crate::i18n::{t, tf};
        let mut out = Vec::new();
        if here != Dock::Main {
            out.push((t("screens.menu.to_main"), Target::Main));
        }
        for (i, w) in self.windows.list.iter().enumerate() {
            if here != Dock::Window(i) && !w.closing {
                let name = self.window_name(i);
                let row = tf("screens.menu.to_window", &[("name", &name)]);
                out.push((row, Target::Window(w.key)));
            }
        }
        if self.windows.screens.len() > 1 && self.windows.list.len() < MAX {
            for (n, s) in self.windows.screens.iter().enumerate() {
                let name = screen_label(s);
                let row = tf(
                    "screens.menu.to_screen",
                    &[("n", &(n + 1)), ("name", &name)],
                );
                out.push((row, Target::NewWindow { screen: Some(n) }));
            }
        }
        out
    }

    /// A window's name: the user's, else its areas' in front.
    pub fn window_name(&self, i: usize) -> String {
        let w = &self.windows.list[i];
        if !w.name.is_empty() {
            return w.name.clone();
        }
        let ws = self.dock(Dock::Window(i));
        let mut names: Vec<String> = ws
            .dock
            .iter_leaves()
            .filter_map(|(_, leaf)| leaf.tabs.get(leaf.active.0))
            .map(|p| crate::i18n::t(p.title_key()))
            .collect();
        if names.is_empty() {
            names = ws
                .panes()
                .iter()
                .map(|p| crate::i18n::t(p.title_key()))
                .collect();
        }
        names.join(" · ")
    }
}

/// How a screen is named in a menu: the system's name without Windows' `\\.\` prefix, else
/// its size.
pub fn screen_label(s: &ScreenKey) -> String {
    let n = s.name.trim_start_matches("\\\\.\\");
    if n.is_empty() {
        format!("{} × {}", s.size[0], s.size[1])
    } else {
        n.to_string()
    }
}
