//! The hint every kit widget writes when hovered or dragged (ADR 0024 §9): what the
//! control is called, its value, and what it can do. The toolbar's hint bar and a
//! device's help line read it.
//!
//! The slot lives in the egui context, so nothing needs a handle on the application:
//! a widget calls [`set`], the bar calls [`current`]. Writing reuses the slot's
//! buffers, so a control being dragged does not allocate every frame.

use egui::{Context, Id};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex};

/// What a control can do, shown as icons at the hint bar's right (FL's capability
/// strip).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Caps {
    /// Can be automated or modulated.
    pub automatable: bool,
    /// Has a right-click menu.
    pub menu: bool,
    /// Something needs attention (a plug-in adding latency, a missing file).
    pub warning: bool,
    /// Can be typed into.
    pub typed: bool,
}

impl Caps {
    pub const NONE: Caps = Caps {
        automatable: false,
        menu: false,
        warning: false,
        typed: false,
    };
    /// A parameter: automatable, with a menu, typed entry.
    pub const PARAM: Caps = Caps {
        automatable: true,
        menu: true,
        warning: false,
        typed: true,
    };
}

/// The hovered control's name, value and capabilities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hint {
    pub name: String,
    /// Empty when the control has no value to show.
    pub value: String,
    pub caps: Caps,
    /// The control is being dragged: the value is live.
    pub live: bool,
}

#[derive(Default)]
struct Slot {
    hint: Hint,
    pass: u64,
}

fn slot(ctx: &Context) -> Arc<Mutex<Slot>> {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_insert_with(Id::new("cantus-kit-hint"), || {
            Arc::new(Mutex::new(Slot::default()))
        })
        .clone()
    })
}

/// Write the hint for the control under the pointer: its `name`, and a value written
/// into the slot's own buffer by `value` (so formatting a number does not allocate).
pub fn set(
    ctx: &Context,
    name: &str,
    caps: Caps,
    live: bool,
    value: impl FnOnce(&mut String) -> std::fmt::Result,
) {
    let s = slot(ctx);
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.pass = ctx.cumulative_pass_nr();
    let h = &mut s.hint;
    h.name.clear();
    h.name.push_str(name);
    h.value.clear();
    let _ = value(&mut h.value);
    h.caps = caps;
    h.live = live;
}

/// [`set`] with a value that is already text.
pub fn set_text(ctx: &Context, name: &str, value: &str, caps: Caps) {
    set(ctx, name, caps, false, |b| b.write_str(value));
}

/// The hint written this frame or the last, if any control is hovered.
pub fn current(ctx: &Context) -> Option<Hint> {
    let s = slot(ctx);
    let s = s.lock().unwrap_or_else(|e| e.into_inner());
    let now = ctx.cumulative_pass_nr();
    (now.saturating_sub(s.pass) <= 1 && !s.hint.name.is_empty()).then(|| s.hint.clone())
}

/// Read the current hint without cloning it.
pub fn read<R>(ctx: &Context, f: impl FnOnce(Option<&Hint>) -> R) -> R {
    let s = slot(ctx);
    let s = s.lock().unwrap_or_else(|e| e.into_inner());
    let now = ctx.cumulative_pass_nr();
    let fresh = now.saturating_sub(s.pass) <= 1 && !s.hint.name.is_empty();
    f(fresh.then_some(&s.hint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hint_is_read_back_and_expires() {
        let ctx = Context::default();
        let mut out = ctx.run_ui(Default::default(), |ui| {
            set(ui.ctx(), "Cutoff", Caps::PARAM, true, |b| {
                write!(b, "{:.1} kHz", 2.4)
            });
        });
        out.textures_delta.clear();
        let h = current(&ctx).expect("a hint was written");
        assert_eq!(h.name, "Cutoff");
        assert_eq!(h.value, "2.4 kHz");
        assert!(h.caps.automatable && h.live);
        for _ in 0..3 {
            ctx.run_ui(Default::default(), |_| {})
                .textures_delta
                .clear();
        }
        assert!(current(&ctx).is_none(), "a hint outlives the hover");
    }
}
