//! **Esc closes what opened last** (round 39, ADR 0110).
//!
//! Every surface that Esc closes — a menu, the palette, the device picker, a dialog, the
//! Settings window — says each frame it is open ([`open`]) and asks, before it reads the key,
//! whether this Esc is its own ([`take`]). The Esc belongs to the surface that opened last
//! among those open; it is taken out of the input then, so nothing older (nor the keymap's
//! *Clear selection*) sees it. A surface that never registers keeps its old behaviour; a
//! registered one under a newer surface no longer closes with it.
//!
//! The stack is per context and costs a short vector: a surface is remembered from the first
//! frame it said it was open to the first frame it did not.

use egui::{Context, Id, Key, Modifiers};
use std::sync::{Arc, Mutex};

/// A surface Esc closes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Layer {
    /// A workspace tab's right-click menu.
    TabMenu,
    /// The command palette.
    Palette,
    /// The device picker.
    Picker,
    /// The preset browser.
    Presets,
    /// The Settings window.
    Settings,
    /// The keyboard shortcuts.
    Shortcuts,
    /// The appearance editor.
    Appearance,
    /// The layout's name dialog.
    LayoutName,
    /// The About window.
    About,
    /// A surface named by the caller (an id of its own).
    Other(Id),
}

#[derive(Default)]
struct Stack {
    pass: u64,
    /// Open at the last complete pass, with the pass each opened in.
    open: Vec<(Layer, u64)>,
    /// Said open during this pass.
    now: Vec<(Layer, u64)>,
    /// The pass whose Esc was taken, and by whom.
    taken: Option<(u64, Layer)>,
}

impl Stack {
    fn roll(&mut self, pass: u64) {
        if pass != self.pass {
            if pass == self.pass + 1 {
                // Swapped, not taken: the two lists keep their room (no allocation a frame).
                std::mem::swap(&mut self.open, &mut self.now);
            } else {
                // Passes went by with nothing open: what was open then is not now.
                self.open.clear();
            }
            self.now.clear();
            self.pass = pass;
        }
    }

    fn top(&self) -> Option<Layer> {
        self.open
            .iter()
            .chain(self.now.iter())
            .max_by_key(|(_, at)| *at)
            .map(|(l, _)| *l)
    }
}

fn stack(ctx: &Context) -> Arc<Mutex<Stack>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("cantus-esc-stack"), || {
            Arc::new(Mutex::new(Stack::default()))
        })
        .clone()
    })
}

/// `layer` is open this frame.
pub fn open(ctx: &Context, layer: Layer) {
    let pass = ctx.cumulative_pass_nr();
    let s = stack(ctx);
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.roll(pass);
    if s.now.iter().any(|(l, _)| *l == layer) {
        return;
    }
    let since = s
        .open
        .iter()
        .find(|(l, _)| *l == layer)
        .map_or(pass, |(_, at)| *at);
    s.now.push((layer, since));
}

/// Whether this frame's Esc is `layer`'s to close it: Esc pressed with no modifier, and
/// `layer` the newest surface open (or the only one asking). The key is taken out of the
/// input when it is.
pub fn take(ctx: &Context, layer: Layer) -> bool {
    let pressed = ctx.input(|i| i.key_pressed(Key::Escape) && !i.modifiers.any());
    let pass = ctx.cumulative_pass_nr();
    let s = stack(ctx);
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.roll(pass);
    if let Some((p, by)) = s.taken {
        if p == pass {
            return by == layer;
        }
    }
    if !pressed {
        return false;
    }
    let mine = match s.top() {
        Some(top) => top == layer,
        // Nothing has said it is open yet: the first to ask.
        None => true,
    };
    if mine {
        s.taken = Some((pass, layer));
        drop(s);
        ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    }
    mine
}

/// Whether any registered surface is open (at the last complete frame or this one): the
/// keymap's Esc (*Clear selection*) waits while one is.
pub fn any_open(ctx: &Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    let s = stack(ctx);
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.roll(pass);
    !s.open.is_empty() || !s.now.is_empty()
}

/// The newest surface open, for tests and the help line.
pub fn top(ctx: &Context) -> Option<Layer> {
    let pass = ctx.cumulative_pass_nr();
    let s = stack(ctx);
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.roll(pass);
    s.top()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &Context, esc: bool, mut f: impl FnMut(&Context)) {
        let mut input = egui::RawInput::default();
        if esc {
            input.events.push(egui::Event::Key {
                key: Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
        }
        ctx.run_ui(input, |ui| f(ui.ctx())).textures_delta.clear();
    }

    #[test]
    fn the_newest_surface_takes_the_esc_and_the_older_waits() {
        let ctx = Context::default();
        let a = Layer::Settings;
        let b = Layer::Palette;
        let mut a_open = true;
        frame(&ctx, false, |c| open(c, a));
        let mut b_open = true;
        frame(&ctx, false, |c| {
            open(c, a);
            open(c, b);
        });
        // One Esc: the palette (newer) closes, Settings stays — whatever order they ask in.
        frame(&ctx, true, |c| {
            if a_open && take(c, a) {
                a_open = false;
            }
            if b_open && take(c, b) {
                b_open = false;
            }
            if a_open {
                open(c, a);
            }
            if b_open {
                open(c, b);
            }
        });
        assert!(a_open && !b_open);
        frame(&ctx, true, |c| {
            if a_open && take(c, a) {
                a_open = false;
            }
        });
        assert!(!a_open);
        frame(&ctx, false, |_| {});
        frame(&ctx, false, |_| {});
        assert!(!any_open(&ctx), "closed surfaces do not stay open");
    }
}
