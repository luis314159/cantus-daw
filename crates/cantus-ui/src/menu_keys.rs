//! **Every kit menu answers the keyboard** (round 39, ADR 0110): ↓ and ↑ move a highlight
//! over its rows that act (wrapping round, Home and End to the ends), Enter or Space runs
//! the highlighted row, and the pointer and the keys share the one highlight — so a menu
//! opened from the keyboard (Shift+F10, the platform's context-menu key) is used from the
//! keyboard to the end, as Windows', macOS's and FL Studio's are.
//!
//! A menu that shows over the window (a popup, an area in front) **holds the keys** it walks
//! with while it is open: the keymap does not also nudge notes on ↓ or launch a slot on
//! Enter ([`holds_keys`], asked by `CantusApp::global_shortcuts`), and Esc closes it when it
//! opened last (`crate::esc`). A popup drawn as several lists walks as one. A list drawn
//! inside a panel (not in front) never takes the keys.

use crate::kit::menu::Item;
use egui::{Context, Id, Key, Modifiers, Order, Ui};

/// The pass a menu last held the keys in.
fn held_id() -> Id {
    Id::new("cantus-menu-keys-held")
}

/// Whether a menu held the keys in this pass or the one before: the keymap waits.
pub fn holds_keys(ctx: &Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<u64>(held_id()))
        .is_some_and(|p| p + 1 >= pass)
}

/// The rows that act, in order.
fn actionable(items: &[Item<'_>]) -> Vec<usize> {
    (0..items.len()).filter(|&i| items[i].is_action()).collect()
}

/// One menu's walk: a popup may draw its rows as several kit lists (the track's menu is
/// seven: recording, automation, freeze…), walked as one. Kept per layer.
#[derive(Clone, Debug, Default)]
struct Walk {
    /// The pass it was walked in last (`None`: never).
    pass: Option<u64>,
    /// The rows that act, per list, in the order the lists were drawn last pass and this.
    before: Vec<usize>,
    now: Vec<usize>,
    /// The lit row, counted over every list of the layer.
    at: Option<usize>,
    /// Enter or Space this pass: the lit row runs.
    run: bool,
}

fn walk_id(ui: &Ui) -> Id {
    Id::new(("cantus-menu-keys-walk", ui.layer_id()))
}

/// The highlight of the menu list drawn in `ui` this frame and whether the keys ran it: the
/// row the pointer is on wins; otherwise ↓ ↑ Home End move it — over every list the popup
/// draws, as one menu — and Enter or Space runs it.
pub fn nav(ui: &Ui, items: &[Item<'_>], pointer: Option<usize>) -> (Option<usize>, bool) {
    let ctx = ui.ctx();
    let rows = actionable(items);
    if rows.is_empty() || ui.layer_id().order < Order::Foreground {
        return (pointer, false);
    }
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(held_id(), pass));
    let id = walk_id(ui);
    let mut w: Walk = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    if w.pass != Some(pass) {
        // The first list of the pass reads the keys, for the whole menu as drawn last pass.
        if w.pass.map(|p| p + 1) != Some(pass) {
            w.at = None;
            w.before.clear();
        } else {
            w.before = std::mem::take(&mut w.now);
        }
        w.now.clear();
        w.pass = Some(pass);
        let none = Modifiers::NONE;
        let (down, up, home, end, run) = ctx.input_mut(|i| {
            (
                i.consume_key(none, Key::ArrowDown),
                i.consume_key(none, Key::ArrowUp),
                i.consume_key(none, Key::Home),
                i.consume_key(none, Key::End),
                i.consume_key(none, Key::Enter) | i.consume_key(none, Key::Space),
            )
        });
        // The first frame a menu shows, only this list is known.
        let total = w.before.iter().sum::<usize>().max(rows.len());
        if down {
            w.at = Some(w.at.map_or(0, |a| (a + 1) % total));
        }
        if up {
            w.at = Some(w.at.map_or(total - 1, |a| (a + total - 1) % total));
        }
        if home {
            w.at = Some(0);
        }
        if end {
            w.at = Some(total - 1);
        }
        w.run = run && w.at.is_some();
        // Esc closes a popup menu when it is what opened last (`crate::esc`); a menu drawn
        // in an area of its own (a tab's, the desk's) closes on its own Esc.
        // Only an egui popup or menu: an area of the window's own (a tab's menu, the desk's)
        // reads its own Esc.
        let popup = ui.stack().iter().any(|f| {
            matches!(
                f.info.kind,
                Some(egui::UiKind::Popup) | Some(egui::UiKind::Menu)
            )
        });
        if popup {
            let layer = crate::esc::Layer::Other(id);
            crate::esc::open(ctx, layer);
            if crate::esc::take(ctx, layer) {
                ui.close();
            }
        }
    }
    let offset: usize = w.now.iter().sum();
    w.now.push(rows.len());
    if let Some(p) = pointer.and_then(|p| rows.iter().position(|r| *r == p)) {
        w.at = Some(offset + p);
    }
    let mine =
        w.at.filter(|a| (offset..offset + rows.len()).contains(a))
            .map(|a| rows[a - offset]);
    let ran = w.run && mine.is_some();
    let last = (w.at, w.before.iter().sum::<usize>().max(w.now.iter().sum()));
    ctx.data_mut(|d| {
        d.insert_temp(id, w);
        d.insert_temp(last_id(), last);
    });
    (mine.or(pointer), ran)
}

fn last_id() -> Id {
    Id::new("cantus-menu-keys-last")
}

/// The last menu that held the keys: which of its rows that act is lit (counted over all
/// its lists), and how many there are — for the tests that walk menus with the keys.
pub fn last(ctx: &Context) -> Option<(Option<usize>, usize)> {
    ctx.data(|d| d.get_temp(last_id()))
}

/// The context menu the keyboard asked for (Shift+F10 outside the mixer): the one a
/// right-click on that thing opens.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Wanted {
    Clip(cantus_core::TrackId, cantus_core::ClipId),
    Track(cantus_core::TrackId),
}

fn wanted_id() -> Id {
    Id::new("cantus-menu-keys-wanted")
}

/// Ask for `w`'s context menu: its view opens it when it draws next.
pub fn want(ctx: &Context, w: Wanted) {
    ctx.data_mut(|d| d.insert_temp(wanted_id(), w));
    ctx.request_repaint();
}

/// The clip whose menu was asked for, taken.
pub fn take_clip(ctx: &Context) -> Option<(cantus_core::TrackId, cantus_core::ClipId)> {
    ctx.data_mut(|d| match d.get_temp::<Wanted>(wanted_id()) {
        Some(Wanted::Clip(t, c)) => {
            d.remove::<Wanted>(wanted_id());
            Some((t, c))
        }
        _ => None,
    })
}

/// Whether `track`'s header menu was asked for (taken if so).
pub fn take_track(ctx: &Context, track: cantus_core::TrackId) -> bool {
    ctx.data_mut(|d| match d.get_temp::<Wanted>(wanted_id()) {
        Some(Wanted::Track(t)) if t == track => {
            d.remove::<Wanted>(wanted_id());
            true
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::text::Src;

    fn frame(ctx: &Context, keys: &[Key], items: &[Item<'_>]) -> (Option<usize>, bool) {
        let mut input = egui::RawInput::default();
        for k in keys {
            input.events.push(egui::Event::Key {
                key: *k,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            });
        }
        let mut out = (None, false);
        ctx.run_ui(input, |ui| {
            egui::Area::new(Id::new("menu"))
                .order(Order::Foreground)
                .show(ui.ctx(), |ui| out = nav(ui, items, None));
        })
        .textures_delta
        .clear();
        out
    }

    #[test]
    fn the_arrows_walk_the_rows_that_act_and_enter_runs_one() {
        let ctx = Context::default();
        let items = [
            Item::Band(Src::Str("A")),
            Item::action(Src::Str("one")),
            Item::Separator,
            Item::action(Src::Str("two")),
            Item::action(Src::Str("three")),
        ];
        assert_eq!(frame(&ctx, &[Key::ArrowDown], &items), (Some(1), false));
        assert_eq!(frame(&ctx, &[Key::ArrowDown], &items), (Some(3), false));
        assert_eq!(frame(&ctx, &[Key::End], &items), (Some(4), false));
        assert_eq!(
            frame(&ctx, &[Key::ArrowDown], &items),
            (Some(1), false),
            "wraps"
        );
        assert_eq!(frame(&ctx, &[Key::ArrowUp], &items), (Some(4), false));
        assert_eq!(frame(&ctx, &[Key::Enter], &items), (Some(4), true));
        assert!(holds_keys(&ctx));
    }
}
