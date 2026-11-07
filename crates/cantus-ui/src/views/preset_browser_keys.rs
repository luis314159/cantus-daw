//! The preset browser's keys (round 37, ADR 0101; the review of its first delivery): it
//! takes **only its own** — ↑ ↓ choose (the audition follows after a short pause, so holding
//! a key does not pile up renders), Space hears again or stops, Enter loads, Esc closes, and
//! the first printable key starts a search — and every other key goes on to the
//! application's shortcuts (Ctrl+Z, save, the transport…).

use crate::app::CantusApp;
use egui::{Event, Id, Key, Modifiers};
use std::time::{Duration, Instant};

/// How long the selection must rest before ↑ / ↓ hear it.
pub const SETTLE: Duration = Duration::from_millis(150);

/// The search field's id.
pub fn search_id() -> Id {
    Id::new("preset-browser-search")
}

/// What the keys asked for this frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyAsk {
    None,
    Hear(String),
    Load(String),
    Close,
}

/// Take the browser's keys out of this frame's input, before the shortcuts read it: called
/// by `CantusApp::global_shortcuts` while the browser is open and no text field holds the
/// keyboard (the search field, once typing, keeps its own keys; ↑ ↓ still move the list,
/// in [`while_typing`]).
pub fn take(app: &mut CantusApp, ctx: &egui::Context) -> KeyAsk {
    let none = Modifiers::NONE;
    let (up, down, enter, space, esc, typed) = ctx.input_mut(|i| {
        let up = i.consume_key(none, Key::ArrowUp);
        let down = i.consume_key(none, Key::ArrowDown);
        let enter = i.consume_key(none, Key::Enter);
        let space = i.consume_key(none, Key::Space);
        let esc = i.consume_key(none, Key::Escape);
        // A printable key with no command modifier starts a search (Space is the browser's).
        let mut typed = String::new();
        if !i.modifiers.command && !i.modifiers.ctrl && !i.modifiers.alt {
            i.events.retain(|e| match e {
                Event::Text(t) if t != " " => {
                    typed.push_str(t);
                    false
                }
                Event::Text(t) if t == " " && space => false,
                _ => true,
            });
        }
        (up, down, enter, space, esc, typed)
    });
    if !typed.is_empty() {
        app.presets.query.text.push_str(&typed);
        app.presets.scroll = 0.0;
        app.presets.focus_search = true;
    }
    if esc {
        return KeyAsk::Close;
    }
    if up || down {
        step(app, if down { 1 } else { -1 });
        return KeyAsk::None;
    }
    match app.presets.selected.clone() {
        Some(id) if enter => KeyAsk::Load(id),
        Some(id) if space => KeyAsk::Hear(id),
        _ => KeyAsk::None,
    }
}

/// ↑ ↓ while the search field is typed into: they move the list (the field has no use
/// for them).
pub fn while_typing(app: &mut CantusApp, ctx: &egui::Context) {
    let (up, down) = ctx.input_mut(|i| {
        (
            i.consume_key(Modifiers::NONE, Key::ArrowUp),
            i.consume_key(Modifiers::NONE, Key::ArrowDown),
        )
    });
    if up || down {
        step(app, if down { 1 } else { -1 });
    }
}

/// Choose the previous or next preset; it is heard once the selection rests.
fn step(app: &mut CantusApp, by: i32) {
    if let Some(id) = app.presets.step(by) {
        app.presets.reveal = true;
        app.presets.pending = Some((id, Instant::now()));
    }
}

/// The preset chosen by the keys whose pause is over (the window hears it).
pub fn settled(app: &mut CantusApp, now: Instant) -> Option<String> {
    match &app.presets.pending {
        Some((_, at)) if now.duration_since(*at) >= SETTLE => {
            app.presets.pending.take().map(|(id, _)| id)
        }
        _ => None,
    }
}

/// Do what the keys asked.
pub fn act(app: &mut CantusApp, ask: KeyAsk) {
    match ask {
        KeyAsk::Hear(id) => app.audition_preset_id(&id),
        KeyAsk::Load(id) => {
            app.apply_preset_id(&id);
        }
        KeyAsk::Close => app.close_preset_browser(),
        KeyAsk::None => {}
    }
}
