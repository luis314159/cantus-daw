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

use egui::{Context, Id, LayerId, Rect, Ui, UiBuilder, Vec2};
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
}

impl Registry {
    pub fn get(&self, key: DeviceKey) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    pub fn get_mut(&mut self, key: DeviceKey) -> Option<&mut Entry> {
        self.entries.iter_mut().find(|e| e.key == key)
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
        out.extend(r.entries.iter().filter(|e| !e.closed).map(|e| e.key));
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
}

impl<'open> DeviceWindow<'open> {
    pub fn new(title: impl Into<String>) -> Self {
        DeviceWindow {
            title: title.into(),
            id: None,
            open: None,
            default_size: Vec2::new(880.0, 560.0),
            min_size: Vec2::new(480.0, 320.0),
        }
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
            let e = match r.get_mut(key) {
                Some(e) => e,
                None => {
                    fresh = true;
                    r.entries.push(Entry {
                        key,
                        title: String::new(),
                        default_size: self.default_size,
                        min_size: self.min_size,
                        seen: pass,
                        body: None,
                        closed: false,
                    });
                    r.entries.last_mut().expect("just pushed")
                }
            };
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
