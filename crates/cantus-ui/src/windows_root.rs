//! The windows from the main window's pass (ADR 0151): the screens read, a window whose
//! screen went parked and one whose screen came back opened, the windows placed and shown,
//! the layout captured and applied with them.

use crate::app::CantusApp;
use crate::layout::Layout;
use crate::screens::{self, Place, PxRect, ScreenKey};
use crate::views::workspace::Workspace;
use crate::views::workspace_ticker as ticker;
use crate::windows::{Dock, Seen, PARKED_LOOK};
use egui::{Context, ViewportId};

impl CantusApp {
    /// The windows' part of the main window's pass.
    pub(crate) fn windows_frame(&mut self, ctx: &Context) {
        if let Some(lay) = self.windows.pending.take() {
            self.apply_layout_value(&lay);
        }
        self.note_main(ctx);
        let now = ctx.input(|i| i.time);
        let parked = !self.windows.parked.is_empty();
        if self.windows.stale || (parked && now - self.windows.read_at >= PARKED_LOOK.as_secs_f64())
        {
            self.read_screens(ctx, now);
        }
        if !self.windows.parked.is_empty() {
            ticker::wake_after(ctx, PARKED_LOOK);
        }
        self.close_windows(ctx);
        self.place_windows();
        self.show_windows(ctx);
    }

    /// What the system says of the main window: its place in physical pixels and the
    /// follow rule's scale for its screen (the windows' scales are measured against it).
    fn note_main(&mut self, ctx: &Context) {
        let zoom = ctx.zoom_factor();
        let (seen, monitor) = ctx.input(|i| {
            let v = i.viewport();
            let native = v.native_pixels_per_point.unwrap_or(1.0).max(0.25);
            let px = |r: egui::Rect| PxRect {
                x: (r.min.x * zoom * native).round() as i32,
                y: (r.min.y * zoom * native).round() as i32,
                w: (r.width() * zoom * native).round() as i32,
                h: (r.height() * zoom * native).round() as i32,
            };
            let seen = Seen {
                outer: v.outer_rect.map(px),
                native,
                minimized: v.minimized.unwrap_or(false),
                focused: v.focused.unwrap_or(false),
                maximized: v.maximized.unwrap_or(false),
                fullscreen: v.fullscreen.unwrap_or(false),
            };
            let monitor = v
                .monitor_size
                .map(|m| crate::screen_scale::monitor_px(m.x, zoom, native));
            (seen, monitor)
        });
        if let Some(px) = monitor {
            self.windows.main_rule = crate::screen_scale::for_screen(px, seen.native);
        }
        if self.windows.main.native != seen.native {
            self.windows.stale = true;
        }
        self.windows.main = seen;
    }

    /// Read the screens (winit's list through the main window's handle, or a test's), and
    /// act on a change of the set.
    pub(crate) fn read_screens(&mut self, ctx: &Context, now: f64) {
        self.windows.stale = false;
        self.windows.read_at = now;
        let list = match &self.windows.fed_screens {
            Some(f) => f.clone(),
            None => system_screens(),
        };
        if list.is_empty() {
            return;
        }
        let fp = screens::fingerprint(&list);
        if fp == self.windows.fingerprint {
            self.windows.screens = list;
            return;
        }
        self.screens_changed(ctx, fp, list);
    }

    /// The set of screens changed (or was read for the first time): the old desk's layout
    /// remembered, the new desk's recalled when it has one; else the windows whose screen
    /// went are parked and those whose screen came back are opened again.
    fn screens_changed(&mut self, ctx: &Context, fp: String, list: Vec<ScreenKey>) {
        let old = std::mem::replace(&mut self.windows.fingerprint, fp.clone());
        let first = old.is_empty();
        if !first {
            let lay = self.capture_layout();
            self.layouts.remember(&old, lay);
        }
        self.windows.screens = list;
        if first {
            self.restore_main_place();
        }
        let made_here = self.layouts.current.screens == fp;
        if !made_here && (!first || !self.layouts.current.screens.is_empty()) {
            if let Some(lay) = self.layouts.recall(&fp).cloned() {
                self.apply_layout_value(&lay);
                ctx.request_repaint();
                return;
            }
        }
        // Windows whose screen went: their areas into the main window.
        let mut gone = Vec::new();
        let mut i = 0;
        while i < self.windows.list.len() {
            let w = &self.windows.list[i];
            let settled = w.place.as_ref().map(|p| screens::settle(p, &self.windows.screens));
            match settled {
                Some(None) => {
                    gone.push(self.window_name(i));
                    let shape = self.window_shape(i);
                    let screen = shape.place.as_ref().map(|p| p.screen.clone());
                    self.windows.list.remove(i);
                    self.park_shape(shape);
                    if let Some(s) = screen {
                        self.windows.missing = Some(s);
                    }
                }
                Some(Some(p)) => {
                    let moved = self.windows.list[i].place.as_ref() != Some(&p);
                    self.windows.list[i].place = Some(p);
                    if moved {
                        self.windows.list[i].shown = false;
                    }
                    i += 1;
                }
                None => i += 1,
            }
        }
        // Windows whose screen came back.
        let mut back = None;
        for shape in std::mem::take(&mut self.windows.parked) {
            let returns = shape
                .place
                .as_ref()
                .is_some_and(|p| screens::settle(p, &self.windows.screens).is_some());
            if returns {
                back = shape.place.as_ref().map(|p| p.screen.clone());
                let dock = shape
                    .dock_state()
                    .map(Workspace::with_dock)
                    .unwrap_or_default();
                if self.open_window(&shape, dock).is_none() {
                    self.windows.parked.push(shape);
                }
            } else {
                self.windows.parked.push(shape);
            }
        }
        if let (false, Some(s)) = (gone.is_empty(), self.windows.missing.clone()) {
            let name = crate::windows_docks::screen_label(&s);
            let areas = gone.join(", ");
            self.set_status(crate::i18n::tf(
                "screens.notice.missing",
                &[("name", &name), ("areas", &areas)],
            ));
        } else if let Some(s) = back {
            let name = crate::windows_docks::screen_label(&s);
            self.set_status(crate::i18n::tf("screens.notice.returned", &[("name", &name)]));
        }
        ctx.request_repaint();
    }

    /// The layout as it is now: the main window's dock and place, every window (open and
    /// waiting for its screen), the set of screens.
    pub fn capture_layout(&self) -> Layout {
        let mut lay = self
            .layouts
            .current
            .capture(self.dock(Dock::Main), self.central.into());
        lay.windows = (0..self.windows.list.len())
            .filter(|i| !self.windows.list[*i].closing)
            .map(|i| self.window_shape(i))
            .chain(self.windows.parked.iter().cloned())
            .collect();
        lay.main_place = self.main_place();
        lay.screens = self.windows.fingerprint.clone();
        lay
    }

    /// At the first reading of the screens: the main window back on the screen it was on
    /// when the layout was kept, when that screen is here and the window opened elsewhere
    /// (once; the system's own place otherwise).
    fn restore_main_place(&mut self) {
        let Some(place) = self.layouts.current.main_place.clone() else {
            return;
        };
        let Some(settled) = screens::settle(&place, &self.windows.screens) else {
            return;
        };
        let now = self
            .windows
            .main
            .outer
            .and_then(|r| screens::screen_of(&r, &self.windows.screens));
        let there = screens::find_screen(&settled.screen, &self.windows.screens);
        if now.is_some() && now == there {
            return;
        }
        let _ = place_through_winit(ViewportId::ROOT, Some(&settled));
    }

    /// The main window's place, when its screen is known.
    pub fn main_place(&self) -> Option<Place> {
        let r = self.windows.main.outer?;
        let i = screens::screen_of(&r, &self.windows.screens)?;
        Some(Place {
            screen: self.windows.screens[i].clone(),
            rect: r,
            maximised: self.windows.main.maximized,
            full_screen: self.windows.main.fullscreen,
        })
    }

    /// Make `lay` the shape of every window: the windows there are closed, the main
    /// window's dock takes the layout's, its windows open (or wait for their screen).
    pub fn apply_layout_value(&mut self, lay: &Layout) {
        // From a window's pass (the palette there): at the main window's next pass.
        if self.windows.current.is_some() {
            self.windows.pending = Some(lay.clone());
            return;
        }
        // The windows there are now close; their areas are the layout's to place.
        while let Some(w) = self.windows.list.pop() {
            drop(w);
        }
        self.windows.parked.clear();
        self.windows.current = None;
        self.central = lay.central.into();
        self.workspace.load(lay);
        for shape in &lay.windows {
            let Some(dock) = shape.dock_state() else {
                continue;
            };
            let _ = self.open_window(shape, Workspace::with_dock(dock));
        }
        self.windows.host = ViewportId::ROOT;
    }

    /// Windows whose close button was pressed (their areas back where they belong in the
    /// main window) or that emptied: gone.
    fn close_windows(&mut self, ctx: &Context) {
        let mut i = 0;
        while i < self.windows.list.len() {
            if !self.windows.list[i].closing {
                i += 1;
                continue;
            }
            let w = self.windows.list.remove(i);
            for p in w.workspace.panes() {
                if self.dock_of(p).is_none() {
                    self.workspace.reveal(p);
                }
            }
            if self.windows.host == w.id() {
                self.windows.host = ViewportId::ROOT;
            }
            ctx.request_repaint();
        }
    }

    /// Put each window not yet shown on its place — hidden until then — through its winit
    /// handle; without one (tests, a window not made yet) it shows where the system puts it.
    fn place_windows(&mut self) {
        for w in &mut self.windows.list {
            if w.shown {
                if let Some(r) = w.seen.outer {
                    // Where it is now is its place (the system moves it, the user drags it).
                    if let Some(i) = screens::screen_of(&r, &self.windows.screens) {
                        w.place = Some(Place {
                            screen: self.windows.screens[i].clone(),
                            rect: r,
                            maximised: w.seen.maximized,
                            full_screen: w.seen.fullscreen,
                        });
                    }
                }
                continue;
            }
            match place_through_winit(w.id(), w.place.as_ref()) {
                Placed::Done => w.shown = true,
                Placed::NotYet if w.passes > 2 || self.windows.fed_screens.is_some() => {
                    w.shown = true;
                }
                Placed::NotYet => {}
            }
        }
    }

    /// Every window's viewport, this pass (egui keeps the ones shown each pass).
    fn show_windows(&mut self, ctx: &Context) {
        let song = self.project().name.clone();
        let keys: Vec<u32> = self.windows.list.iter().map(|w| w.key).collect();
        for key in keys {
            let Some(i) = self.windows.index(key) else {
                continue;
            };
            let name = self.window_name(i);
            let title = crate::i18n::tf("screens.window.title", &[("song", &song), ("area", &name)]);
            let w = &self.windows.list[i];
            let vb = egui::ViewportBuilder::default()
                .with_title(title.clone())
                .with_app_id("cantus")
                .with_min_inner_size(screens::MIN_WINDOW)
                .with_visible(w.shown || self.windows.fed_screens.is_some())
                .with_window_level(if w.on_top {
                    egui::WindowLevel::AlwaysOnTop
                } else {
                    egui::WindowLevel::Normal
                });
            if ctx.embed_viewports() {
                // Tests and pictures without OS windows: an egui window inside this one.
                egui::Window::new(title)
                    .id(egui::Id::new(("cantus-window-embedded", key)))
                    .default_size([640.0, 420.0])
                    .show(ctx, |ui| self.window_pass_in(ui, key, true));
            } else {
                ctx.show_viewport_deferred(w.id(), vb, crate::windows_pass::callback(key));
            }
        }
    }
}

/// What placing a window did.
enum Placed {
    Done,
    NotYet,
}

/// Place window `id` on `place` through its winit handle (physical pixels), unmaximised
/// first so the size given is its restored size, then maximised or full screen as it was.
fn place_through_winit(id: ViewportId, place: Option<&Place>) -> Placed {
    let Some(win) = eframe::window_of(id) else {
        return Placed::NotYet;
    };
    if let Some(p) = place {
        use winit::dpi::{PhysicalPosition, PhysicalSize};
        win.set_maximized(false);
        win.set_outer_position(PhysicalPosition::new(p.rect.x, p.rect.y));
        let _ = win.request_inner_size(PhysicalSize::new(
            p.rect.w.max(1) as u32,
            p.rect.h.max(1) as u32,
        ));
        if p.full_screen {
            win.set_fullscreen(Some(winit::window::Fullscreen::Borderless(
                win.current_monitor(),
            )));
        } else if p.maximised {
            win.set_maximized(true);
        }
    }
    Placed::Done
}

/// The screens as winit lists them, through the main window's handle (none before it is
/// made, and none without eframe: the tests feed theirs).
fn system_screens() -> Vec<ScreenKey> {
    let Some(win) = eframe::window_of(ViewportId::ROOT) else {
        return Vec::new();
    };
    win.available_monitors()
        .map(|m| {
            let size = m.size();
            let pos = m.position();
            ScreenKey::new(
                &m.name().unwrap_or_default(),
                [size.width, size.height],
                [pos.x, pos.y],
                m.scale_factor() as f32,
            )
        })
        .collect()
}
