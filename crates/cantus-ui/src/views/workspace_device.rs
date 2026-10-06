//! Device windows that **dock by default and float by choice** (ADR 0024 §5).
//!
//! An effect's or an instrument's editor used to open as an `egui::Window` floating over
//! the desk it belongs to. [`DeviceWindow`] is a drop-in for that builder — the same
//! `.id() .open() .default_size() .min_size() .resizable() .collapsible() .show(ctx, …)`
//! chain — whose editor shows up as a **tab of the workspace** instead: docked beside the
//! arrangement the first time, then wherever the user drags it, split, stacked, or torn
//! out as a floating window. Switching an editor over is one line:
//!
//! ```ignore
//! // before
//! egui::Window::new(title).id(id).open(&mut open).default_size([980.0, 620.0])
//! // after
//! crate::views::workspace_device::DeviceWindow::new(title).id(id).open(&mut open).default_size([980.0, 620.0])
//! ```
//!
//! How it works, in immediate mode: the editor's own `window(app, ctx)` keeps running at
//! the end of the frame, as today. Each [`DeviceWindow::show`] registers the device (its
//! key and title) in a registry kept in the egui context. The workspace, which lays the
//! dock out earlier in the frame, gives every registered device a tab and records the
//! rect, layer and clip of the tab's body while the tab is showing; `show` then builds a
//! `Ui` on exactly that rect and runs the editor's body there. A new editor appears on
//! the frame after it first asks (the workspace asks for that frame at once); a closed
//! tab makes `show` report `open = false`, so the editor closes the way it did when its
//! window's ✕ was clicked. An editor whose tab is behind another tab is not drawn at all.
//!
//! Nothing here needs the application: the registry lives in the context, so an editor
//! can switch without touching `app.rs`.
//!
//! **Round 20: the window's own buttons.** The registry also keeps what the shell's
//! buttons ask of a device ([`crate::kit::window_ctl`]): **minimised** — not drawn, a chip
//! in the device tray brings it back ([`minimise`], [`restore`]); **maximised** — filling
//! the workspace, and back ([`toggle_maximise`]); and the effects' editors, which float
//! as areas of their own rather than tabs ([`Entry::area`]), so one tray, one Esc and one
//! set of buttons serve every device. It remembers where each device floated, so a device
//! closed and opened again comes back where it was ([`Registry::places`]), and which
//! device was clicked last, so **Esc** closes it ([`focused`]).

use crate::kit::Function;
use egui::{Context, Id, LayerId, Pos2, Rect, Ui, UiBuilder, Vec2};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A device editor's key in the registry and its tab's identity: the `Id` the editor gave
/// its window, as a number.
pub type DeviceKey = u64;

/// One registered editor.
#[derive(Clone, Debug)]
pub struct Entry {
    pub key: DeviceKey,
    pub title: String,
    pub default_size: Vec2,
    pub min_size: Vec2,
    /// The pass the editor last asked to be shown in.
    pub seen: u64,
    /// Where its tab's body is, and in which pass that was recorded.
    pub body: Option<Body>,
    /// The user closed its tab: the next `show` reports `open = false`.
    pub closed: bool,
    /// Hosted as a floating area of its own (an effect's editor), not as a tab of the dock.
    pub area: bool,
    /// Minimised to the device tray: not drawn; its chip brings it back.
    pub minimised: bool,
    /// Filling the workspace.
    pub maximised: bool,
    /// Where it was before it was maximised: a floating rect, or `None` for a tab that
    /// was docked.
    pub restore: Option<Rect>,
    /// A maximise or restore the workspace has to carry out on a tab (it owns the dock).
    pub pending: Option<Pending>,
    /// The device hue: its chip's mark in the tray.
    pub hue: Function,
    /// Where it was drawn last, whole and its header (published geometry for tests and
    /// for the focus).
    pub shown: Option<(Rect, Rect)>,
    /// Where its window buttons were drawn last (published geometry).
    pub buttons: Option<crate::kit::window_ctl::WindowRects>,
}

/// A change of place the workspace carries out on a device's tab.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pending {
    Maximise,
    Restore,
}

/// A docked tab's body, recorded by the workspace while laying the dock out.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Body {
    pub rect: Rect,
    pub clip: Rect,
    pub layer: LayerId,
    pub pass: u64,
}

/// Every editor that asked to be shown recently.
#[derive(Default)]
pub struct Registry {
    pub entries: Vec<Entry>,
    /// The workspace's area last frame: where a maximised device goes.
    pub area: Option<Rect>,
    /// The device clicked last, while it stays on screen: Esc closes it.
    pub focused: Option<DeviceKey>,
    /// Where each device floated last, for the session: it opens there again. A device
    /// docked by the user is forgotten here, and docks again.
    pub places: HashMap<DeviceKey, Rect>,
    /// Floating windows to put at a place again for a few frames: egui keeps a window's
    /// old size for the frame it moves, and a window that was maximised is pushed back
    /// against the workspace's corner by it.
    pub settle: Vec<(DeviceKey, Pos2, u8)>,
}

impl Registry {
    pub fn get(&self, key: DeviceKey) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    pub fn get_mut(&mut self, key: DeviceKey) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.key == key)
    }

    /// The entry for `key`, made fresh when there is none.
    fn entry(&mut self, key: DeviceKey, pass: u64, area: bool) -> (&mut Entry, bool) {
        match self.entries.iter().position(|e| e.key == key) {
            Some(i) => (&mut self.entries[i], false),
            None => {
                self.entries.push(Entry::new(key, pass, area));
                (self.entries.last_mut().expect("just pushed"), true)
            }
        }
    }
}

impl Entry {
    fn new(key: DeviceKey, pass: u64, area: bool) -> Entry {
        Entry {
            key,
            title: String::new(),
            default_size: Vec2::new(880.0, 560.0),
            min_size: Vec2::new(480.0, 320.0),
            seen: pass,
            body: None,
            closed: false,
            area,
            minimised: false,
            maximised: false,
            restore: None,
            pending: None,
            hue: Function::Output,
            shown: None,
            buttons: None,
        }
    }
}

/// The registry of `ctx`.
pub fn registry(ctx: &Context) -> Arc<Mutex<Registry>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("cantus-devices"), || {
            Arc::new(Mutex::new(Registry::default()))
        })
        .clone()
    })
}

/// Run `f` on the registry of `ctx`.
pub fn with_registry<R>(ctx: &Context, f: impl FnOnce(&mut Registry) -> R) -> R {
    let reg = registry(ctx);
    let mut g = reg.lock().unwrap_or_else(|e| e.into_inner());
    f(&mut g)
}

/// The workspace records a docked device tab's body (called from its tab's `ui`).
pub fn record_body(ctx: &Context, key: DeviceKey, ui: &Ui) {
    let pass = ctx.cumulative_pass_nr();
    let body = Body {
        rect: ui.max_rect(),
        clip: ui.clip_rect(),
        layer: ui.layer_id(),
        pass,
    };
    with_registry(ctx, |r| {
        if let Some(e) = r.get_mut(key) {
            e.body = Some(body);
        }
    });
}

/// The user closed a device's tab.
pub fn mark_closed(ctx: &Context, key: DeviceKey) {
    with_registry(ctx, |r| {
        if let Some(e) = r.get_mut(key) {
            e.closed = true;
            e.body = None;
        }
    });
}

/// The title of a registered device, for its tab.
pub fn title(ctx: &Context, key: DeviceKey, out: &mut String) -> bool {
    with_registry(ctx, |r| match r.get(key) {
        Some(e) => {
            out.clear();
            out.push_str(&e.title);
            true
        }
        None => false,
    })
}

/// The editors that asked to be shown in the last two passes, with no tab closed on them.
pub fn wanted(ctx: &Context, out: &mut Vec<DeviceKey>) {
    let pass = ctx.cumulative_pass_nr();
    out.clear();
    with_registry(ctx, |r| {
        // An editor that stopped asking was closed by the application: forget it.
        r.entries.retain(|e| pass.saturating_sub(e.seen) <= 2);
        if r.focused.is_some_and(|k| r.get(k).is_none()) {
            r.focused = None;
        }
        out.extend(
            r.entries
                .iter()
                .filter(|e| !e.closed && !e.area && !e.minimised)
                .map(|e| e.key),
        );
    });
}

// ---- the window's buttons (round 20) ------------------------------------------------------

/// Minimise a device to the tray: it stops being drawn (its tab leaves the dock, its area
/// is not shown) and stays open; its chip brings it back.
pub fn minimise(ctx: &Context, key: DeviceKey) {
    with_registry(ctx, |r| {
        if let Some(e) = r.get_mut(key) {
            e.minimised = true;
            e.body = None;
        }
        if r.focused == Some(key) {
            r.focused = None;
        }
    });
    ctx.request_repaint();
}

/// Bring a minimised device back where it was, and make it the one Esc closes.
pub fn restore(ctx: &Context, key: DeviceKey) {
    with_registry(ctx, |r| {
        if let Some(e) = r.get_mut(key) {
            e.minimised = false;
        }
        r.focused = Some(key);
    });
    ctx.request_repaint();
}

/// Maximise a device to the workspace, or restore it to where it was. An area does it
/// itself; a tab is moved by the workspace on the next frame.
pub fn toggle_maximise(ctx: &Context, key: DeviceKey) {
    with_registry(ctx, |r| {
        if let Some(e) = r.get_mut(key) {
            if e.area {
                if e.maximised {
                    e.maximised = false;
                } else {
                    e.restore = e.shown.map(|(whole, _)| whole);
                    e.maximised = true;
                }
            } else {
                e.pending = Some(if e.maximised {
                    Pending::Restore
                } else {
                    Pending::Maximise
                });
            }
        }
    });
    ctx.request_repaint();
}

/// Whether the device is maximised (its key shows *restore*).
pub fn is_maximised(ctx: &Context, key: DeviceKey) -> bool {
    with_registry(ctx, |r| r.get(key).is_some_and(|e| e.maximised))
}

/// Whether the device is minimised to the tray.
pub fn is_minimised(ctx: &Context, key: DeviceKey) -> bool {
    with_registry(ctx, |r| r.get(key).is_some_and(|e| e.minimised))
}

/// A device was drawn this frame at `whole`, its header at `header`, its window buttons at
/// `buttons`: published for tests, and a press inside makes it the device Esc closes.
pub fn note_shown(
    ctx: &Context,
    key: DeviceKey,
    whole: Rect,
    header: Rect,
    buttons: Option<crate::kit::window_ctl::WindowRects>,
) {
    let pressed = ctx.input(|i| {
        i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| whole.contains(p))
    });
    with_registry(ctx, |r| {
        if let Some(e) = r.get_mut(key) {
            e.shown = Some((whole, header));
            e.buttons = buttons;
        }
        if pressed {
            r.focused = Some(key);
        }
    });
}

/// Name a registered device (its chip in the tray), when the name changed.
pub fn set_title(ctx: &Context, key: DeviceKey, title: &str) {
    with_registry(ctx, |r| {
        if let Some(e) = r.get_mut(key) {
            if e.title != title {
                e.title.clear();
                e.title.push_str(title);
            }
        }
    });
}

/// A device's size at its scale and the least it can be drawn in, as it registered them.
pub fn sizes(ctx: &Context, key: DeviceKey) -> Option<(Vec2, Vec2)> {
    with_registry(ctx, |r| r.get(key).map(|e| (e.default_size, e.min_size)))
}

/// Where a device's window buttons were drawn last.
pub fn buttons(ctx: &Context, key: DeviceKey) -> Option<crate::kit::window_ctl::WindowRects> {
    with_registry(ctx, |r| r.get(key).and_then(|e| e.buttons))
}

/// Where a device was drawn last: (whole, header).
pub fn shown(ctx: &Context, key: DeviceKey) -> Option<(Rect, Rect)> {
    with_registry(ctx, |r| r.get(key).and_then(|e| e.shown))
}

/// The device Esc closes: the one clicked last, if it is still open and on screen.
pub fn focused(ctx: &Context) -> Option<DeviceKey> {
    with_registry(ctx, |r| {
        r.focused
            .filter(|k| r.get(*k).is_some_and(|e| !e.closed && !e.minimised))
    })
}

/// The workspace's frame starts: remember its area, and a press that lands on no device
/// takes the focus away from them all.
pub fn begin_frame(ctx: &Context, area: Rect) {
    let press = ctx.input(|i| {
        i.pointer
            .any_pressed()
            .then(|| i.pointer.interact_pos())
            .flatten()
    });
    with_registry(ctx, |r| {
        r.area = Some(area);
        if let Some(p) = press {
            let on_device = r
                .entries
                .iter()
                .any(|e| !e.minimised && e.shown.is_some_and(|(w, _)| w.contains(p)));
            if !on_device {
                r.focused = None;
            }
        }
    });
}

/// The minimised devices, for the tray: (key, title, hue), in the order they opened.
pub fn minimised(ctx: &Context, out: &mut Vec<(DeviceKey, String, Function)>) {
    out.clear();
    with_registry(ctx, |r| {
        out.extend(
            r.entries
                .iter()
                .filter(|e| e.minimised && !e.closed)
                .map(|e| (e.key, e.title.clone(), e.hue)),
        );
    });
}

/// What an area-hosted device (an effect's editor) is to do this frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AreaState {
    /// Closed from outside (Esc, the tray): close the editor.
    pub closed: bool,
    pub minimised: bool,
    /// Maximised: draw it over the workspace's area.
    pub maximised: Option<Rect>,
    /// Where it floated last (this session), if anywhere.
    pub place: Option<Pos2>,
}

/// An effect's editor registers itself as an area-hosted device every frame it is open.
/// `title` is asked for only when the entry has none.
pub fn register_area(
    ctx: &Context,
    key: DeviceKey,
    hue: Function,
    title: impl FnOnce() -> String,
) -> AreaState {
    let pass = ctx.cumulative_pass_nr();
    let st = with_registry(ctx, |r| {
        let area = r.area;
        let place = r.places.get(&key).map(|p| p.min);
        let (e, _) = r.entry(key, pass, true);
        e.seen = pass;
        e.area = true;
        e.hue = hue;
        if e.title.is_empty() {
            e.title = title();
        }
        AreaState {
            closed: e.closed,
            minimised: e.minimised,
            maximised: area.filter(|a| e.maximised && a.is_positive()),
            place,
        }
    });
    if st.closed {
        with_registry(ctx, |r| r.entries.retain(|e| e.key != key));
    }
    st
}

/// Remember where a device floats (an area's rect, a floating tab's window).
pub fn remember_place(ctx: &Context, key: DeviceKey, rect: Rect) {
    with_registry(ctx, |r| {
        r.places.insert(key, rect);
    });
}

/// Forget where a device floated: it was docked.
pub fn forget_place(ctx: &Context, key: DeviceKey) {
    with_registry(ctx, |r| {
        r.places.remove(&key);
    });
}

/// A device took another's place (a swap): it floats where that one floated.
pub fn inherit_place(ctx: &Context, from: DeviceKey, to: DeviceKey) {
    with_registry(ctx, |r| {
        if let Some(p) = r.places.get(&from).copied() {
            r.places.insert(to, p);
        }
    });
}

/// Put `key`'s floating window at `at` for the next few frames.
pub fn settle(ctx: &Context, key: DeviceKey, at: Pos2) {
    with_registry(ctx, |r| {
        r.settle.retain(|s| s.0 != key);
        r.settle.push((key, at, 3));
    });
}

/// The settles still to apply this frame (each counted down once).
pub fn take_settles(ctx: &Context, out: &mut Vec<(DeviceKey, Pos2)>) {
    out.clear();
    with_registry(ctx, |r| {
        for s in &mut r.settle {
            out.push((s.0, s.1));
            s.2 = s.2.saturating_sub(1);
        }
        r.settle.retain(|s| s.2 > 0);
    });
}

/// Where a device floated last, if it did.
pub fn place_of(ctx: &Context, key: DeviceKey) -> Option<Rect> {
    with_registry(ctx, |r| r.places.get(&key).copied())
}

/// The tabs with a maximise or restore to carry out (taken: done once).
pub fn take_pending(ctx: &Context, out: &mut Vec<(DeviceKey, Pending, Option<Rect>)>) {
    out.clear();
    with_registry(ctx, |r| {
        for e in &mut r.entries {
            if let Some(p) = e.pending.take() {
                out.push((e.key, p, e.restore));
            }
        }
    });
}

/// The workspace carried out a maximise (with where to restore to) or a restore.
pub fn set_maximised(ctx: &Context, key: DeviceKey, on: bool, restore: Option<Rect>) {
    with_registry(ctx, |r| {
        if let Some(e) = r.get_mut(key) {
            e.maximised = on;
            e.restore = restore;
        }
    });
}

/// Alt+K (`Action::ToggleDeviceKeyboard`, user-editable in the keymap): ask the instrument
/// whose editor is in front to show or hide its keyboard card. Called by the application.
pub fn request_keyboard_toggle(ctx: &Context) {
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(Id::new("cantus-device-keyboard"), pass));
}

/// For the device shell: whether the keyboard card's shortcut was pressed this frame (or
/// the last, since the keys are read before the editors are drawn). True once per press.
pub fn keyboard_toggle_requested(ctx: &Context) -> bool {
    let id = Id::new("cantus-device-keyboard");
    let pass = ctx.cumulative_pass_nr();
    let at = ctx.data(|d| d.get_temp::<u64>(id));
    match at {
        Some(p) if pass.saturating_sub(p) <= 1 => {
            ctx.data_mut(|d| d.remove::<u64>(id));
            true
        }
        _ => false,
    }
}

/// A device editor's window: a tab of the workspace, docked by default (see the module
/// docs). Mirrors `egui::Window`'s builder, so switching is one line.
#[must_use = "call `.show(ctx, |ui| …)`"]
pub struct DeviceWindow<'open> {
    title: String,
    id: Option<Id>,
    open: Option<&'open mut bool>,
    default_size: Vec2,
    min_size: Vec2,
    hue: Option<Function>,
}

impl<'open> DeviceWindow<'open> {
    pub fn new(title: impl Into<String>) -> Self {
        DeviceWindow {
            title: title.into(),
            id: None,
            open: None,
            default_size: Vec2::new(880.0, 560.0),
            min_size: Vec2::new(480.0, 320.0),
            hue: None,
        }
    }

    /// The device hue: its chip's mark in the tray when it is minimised.
    pub fn hue(mut self, hue: Function) -> Self {
        self.hue = Some(hue);
        self
    }

    /// The editor's identity: the same id always means the same tab.
    pub fn id(mut self, id: Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Set to `false` when the user closes the tab.
    pub fn open(mut self, open: &'open mut bool) -> Self {
        self.open = Some(open);
        self
    }

    /// The size it floats at when torn out of the dock.
    pub fn default_size(mut self, size: impl Into<Vec2>) -> Self {
        self.default_size = size.into();
        self
    }

    pub fn min_size(mut self, size: impl Into<Vec2>) -> Self {
        self.min_size = size.into();
        self
    }

    /// Kept for the drop-in: a docked tab is always resizable by its splitters.
    pub fn resizable(self, _on: bool) -> Self {
        self
    }

    /// Kept for the drop-in: a tab does not collapse.
    pub fn collapsible(self, _on: bool) -> Self {
        self
    }

    /// Kept for the drop-in: the tab is the title bar.
    pub fn title_bar(self, _on: bool) -> Self {
        self
    }

    /// The key this window registers under.
    pub fn key(&self) -> DeviceKey {
        self.id
            .unwrap_or_else(|| Id::new(("cantus-device", self.title.as_str())))
            .value()
    }

    /// Register the editor and, when its tab is showing, run `add_contents` in the tab's
    /// body. Returns what the body returned, or `None` when it was not drawn this frame.
    pub fn show<R>(self, ctx: &Context, add_contents: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        let key = self.key();
        let pass = ctx.cumulative_pass_nr();
        let mut fresh = false;
        let state = with_registry(ctx, |r| {
            let (e, new) = r.entry(key, pass, false);
            fresh = new;
            if let Some(hue) = self.hue {
                e.hue = hue;
            }
            if e.title != self.title {
                e.title.clear();
                e.title.push_str(&self.title);
            }
            e.seen = pass;
            e.default_size = self.default_size;
            e.min_size = self.min_size;
            if e.closed {
                None
            } else {
                Some(e.body.filter(|b| b.pass == pass))
            }
        });
        let Some(body) = state else {
            // The user closed the tab: close the editor, and forget it.
            with_registry(ctx, |r| r.entries.retain(|e| e.key != key));
            if let Some(open) = self.open {
                *open = false;
            }
            return None;
        };
        if fresh {
            // The workspace gives it a tab on the next frame: draw that frame now.
            ctx.request_repaint();
        }
        let body = body?;
        let mut ui = Ui::new(
            ctx.clone(),
            Id::new(("cantus-device-body", key)),
            UiBuilder::new().layer_id(body.layer).max_rect(body.rect),
        );
        ui.set_clip_rect(body.clip.intersect(body.rect));
        Some(add_contents(&mut ui))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ctx: &Context, mut f: impl FnMut(&mut Ui)) {
        ctx.run_ui(Default::default(), |ui| f(ui))
            .textures_delta
            .clear();
    }

    #[test]
    fn a_device_registers_draws_in_its_tab_and_closes_with_it() {
        let ctx = Context::default();
        let id = Id::new(("eq-editor", 7));
        let key = id.value();
        let mut open = true;
        let mut drew = false;
        // First frame: it registers; there is no tab yet, so nothing is drawn.
        run(&ctx, |ui| {
            let r = DeviceWindow::new("EQ")
                .id(id)
                .open(&mut open)
                .show(ui.ctx(), |_| ());
            assert!(r.is_none());
        });
        let mut keys = Vec::new();
        wanted(&ctx, &mut keys);
        assert_eq!(keys, vec![key]);
        // Second frame: the workspace records the tab's body, then the editor draws in it.
        run(&ctx, |ui| {
            record_body(ui.ctx(), key, ui);
            DeviceWindow::new("EQ")
                .id(id)
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    drew = true;
                    assert!(ui.max_rect().width() > 0.0);
                });
        });
        assert!(drew && open);
        let mut t = String::new();
        assert!(title(&ctx, key, &mut t));
        assert_eq!(t, "EQ");
        // The user closes the tab: the editor hears `open = false`.
        mark_closed(&ctx, key);
        run(&ctx, |ui| {
            DeviceWindow::new("EQ")
                .id(id)
                .open(&mut open)
                .show(ui.ctx(), |_| ());
        });
        assert!(!open);
        wanted(&ctx, &mut keys);
        assert!(keys.is_empty());
    }
}
