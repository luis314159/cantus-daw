//! Widgets of our own, for the controls egui's defaults cannot draw well enough.
//!
//! egui's built-in widgets carry the whole application — buttons, combos, text edits —
//! and nothing here replaces them wholesale. This module exists for the handful of
//! controls where the default *cannot* express what the control means:
//!
//! - [`fader`]: egui paints a slider's groove and its cap from the same
//!   `inactive.bg_fill`, so at rest a fader is a rail with a slightly-outlined lump on
//!   it. A console fader is a recessed slot with a raised cap that has a read line and a
//!   dB scale beside it, and none of those are reachable through `Slider`'s builder.
//! - [`bar`]: a slider fills from its left end, so a pan at dead centre looks like a pan
//!   at 3 %. A bipolar control has to fill from its detent outwards, because its meaning
//!   is a direction and an amount, not a quantity.
//!
//! ## Widgets that own the arrow keys
//!
//! A focused fader moves with the arrow keys, and so does the note selection — and
//! [`CantusApp::global_shortcuts`](crate::app::CantusApp::global_shortcuts) runs before
//! any panel is laid out, so it cannot see that a widget is about to take the key.
//! egui's [`set_focus_lock_filter`](egui::Memory::set_focus_lock_filter) stops focus from
//! *moving*, but it does not stop our own dispatch from firing, and there is no public
//! way to read the filter back. So a widget that wants the arrows calls
//! [`claim_arrow_keys`] while focused, and the dispatch asks [`arrow_keys_claimed`]
//! first. Without it, nudging a fader would nudge the selected notes as well.

pub mod bar;
pub mod fader;

pub use bar::Bar;
pub use fader::Fader;

use egui::{Context, Id};

fn arrow_owner_id() -> Id {
    Id::new("cantus-arrow-owner")
}

/// Say that the focused widget `id` moves with the arrow keys this frame.
pub fn claim_arrow_keys(ctx: &Context, id: Id) {
    ctx.data_mut(|d| d.insert_temp(arrow_owner_id(), id));
}

/// Whether the widget that currently holds keyboard focus has claimed the arrow keys.
///
/// The claim is written while the widget is drawn and read at the top of the next frame,
/// which is correct: focus persists between frames, so the claim is still the truth about
/// the widget the keys are meant for.
pub fn arrow_keys_claimed(ctx: &Context) -> bool {
    let Some(focused) = ctx.memory(|m| m.focused()) else {
        return false;
    };
    ctx.data(|d| d.get_temp::<Id>(arrow_owner_id())) == Some(focused)
}
