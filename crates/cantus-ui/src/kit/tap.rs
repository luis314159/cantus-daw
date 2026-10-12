//! The `POST | PRE` switch every send shows (round 25, the owner's answer 4): on the
//! strip's own send rows (Extra large), on a send cable's pill, in the inspector's send
//! rows and in the route menu. One painter, so a send's tap reads the same everywhere.
//!
//! It is the kit's segmented control with one segment per [`SendTap`] (in
//! [`SendTap::ALL`]'s order, so a tap point added later gets its segment here without a
//! change), the selected one lit and underlined in the tap's own hue — the sends' hue
//! (*Space*) for post-fader, the modulation hue for pre-fader — so a pre-fader send is
//! told apart at a glance, not only by its word. Hovering says what the tap means and
//! what a click does. Callers issue the change with `CantusApp::set_send_tap`.

use super::hint::{self, Caps};
use super::segmented;
use super::text::Src;
use super::{Brush, Function, State, Style};
use cantus_core::SendTap;
use egui::{Id, Key, Rect, Sense, Ui};

/// The function hue a tap is lit in: post-fader in the sends' hue, pre-fader in the
/// modulation hue. The desk's `PRE` pill and the matrix's pre-fader cell use it too.
pub fn function(tap: SendTap) -> Function {
    match tap {
        SendTap::Post => Function::Space,
        SendTap::Pre => Function::Mod,
    }
}

/// The i18n keys of a tap's name and of what it means (the hint and the tooltip).
pub fn hint_keys(tap: SendTap) -> (&'static str, &'static str) {
    match tap {
        SendTap::Post => ("matrix.tap.post.name", "matrix.tap.post.hint"),
        SendTap::Pre => ("matrix.tap.pre.name", "matrix.tap.pre.hint"),
    }
}

fn index(tap: SendTap) -> usize {
    SendTap::ALL.iter().position(|t| *t == tap).unwrap_or(0)
}

/// Paint the switch for `tap` in `rect` (`hovered`: the segment under the pointer).
pub fn paint(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    st: State,
    tap: SendTap,
    hovered: Option<usize>,
) {
    let labels = SendTap::ALL.map(|t| Src::Key(t.label_key()));
    let s = s.function(function(tap));
    segmented::paint(b, rect, &s, st, &labels, index(tap), hovered);
}

/// Paint the switch for `tap` in `rect` and sense it as `id`; returns the tap the user
/// picked this frame, if any: a click on another segment picks it; with the switch
/// focused, ← and → step through the taps.
pub fn switch(
    ui: &Ui,
    b: &mut Brush,
    s: &Style,
    rect: Rect,
    id: Id,
    tap: SendTap,
) -> Option<SendTap> {
    let resp = ui.interact(rect, id, Sense::click());
    let n = SendTap::ALL.len();
    let hovered = resp.hover_pos().and_then(|p| segmented::hit(rect, n, p.x));
    let mut picked = None;
    if resp.clicked_by(egui::PointerButton::Primary) {
        picked = hovered.map(|i| SendTap::ALL[i]).filter(|t| *t != tap);
    }
    if resp.has_focus() {
        let (left, right) = ui.input(|i| {
            (
                i.key_pressed(Key::ArrowLeft),
                i.key_pressed(Key::ArrowRight),
            )
        });
        let at = index(tap);
        if left && at > 0 {
            picked = Some(SendTap::ALL[at - 1]);
        } else if right && at + 1 < n {
            picked = Some(SendTap::ALL[at + 1]);
        }
    }
    let st = State::of(&resp, false);
    if st.hovered {
        // What the segment under the pointer means: the hovered one, or the current.
        let shown = hovered.map_or(tap, |i| SendTap::ALL[i]);
        let (name, value) = hint_keys(shown);
        hint::set_text(
            ui.ctx(),
            &crate::i18n::t(name),
            &crate::i18n::t(value),
            Caps::NONE,
        );
    }
    paint(b, rect, s, st, tap, hovered);
    picked
}

/// The width of a tap [`chip`] showing `tap` at `k` (text scale): its word plus padding,
/// measured (never cut).
pub fn chip_width(ctx: &egui::Context, s: &Style, tap: SendTap, k: f32) -> f32 {
    let tx =
        super::text::Text::key(tap.label_key(), super::text::Role::Micro, s.t.text).caps(false);
    super::text::measure(ctx, &tx, k).x + s.s(7.0)
}

/// A send's tap as **one chip sized to its word** (integration: where a two-segment
/// switch would be too narrow to name both taps — the strip's own send rows, the selected
/// cable's pill): `PRE` lit in the modulation hue, `POST` dim in an outline. A click picks
/// the next tap of [`SendTap::ALL`] (so a tap point added later joins the cycle); with the
/// chip focused, ← and → step. Sensed only where it shows (`clip`: the dock it lies in).
/// Returns the tap picked this frame, if any. `k` is the text scale.
#[allow(clippy::too_many_arguments)]
pub fn chip(
    ui: &Ui,
    b: &mut Brush,
    s: &Style,
    rect: Rect,
    clip: Rect,
    id: Id,
    tap: SendTap,
    k: f32,
) -> Option<SendTap> {
    let sensed = rect.intersect(clip);
    let resp = (sensed.width() > 0.5 && sensed.height() > 0.5)
        .then(|| ui.interact(sensed, id, Sense::click()));
    let n = SendTap::ALL.len();
    let at = index(tap);
    let mut picked = None;
    if let Some(resp) = &resp {
        if resp.clicked_by(egui::PointerButton::Primary) {
            picked = Some(SendTap::ALL[(at + 1) % n]).filter(|t| *t != tap);
        }
        if resp.has_focus() {
            let (left, right) = ui.input(|i| {
                (
                    i.key_pressed(Key::ArrowLeft),
                    i.key_pressed(Key::ArrowRight),
                )
            });
            if left && at > 0 {
                picked = Some(SendTap::ALL[at - 1]);
            } else if right && at + 1 < n {
                picked = Some(SendTap::ALL[at + 1]);
            }
        }
    }
    let st = resp.as_ref().map_or(State::REST, |r| State::of(r, false));
    if st.hovered {
        let (name, value) = hint_keys(tap);
        hint::set_text(
            ui.ctx(),
            &crate::i18n::t(name),
            &crate::i18n::t(value),
            Caps::NONE,
        );
    }
    paint_chip(b, s, rect, st, tap, k);
    picked
}

/// Paint a tap chip (see [`chip`]).
pub fn paint_chip(b: &mut Brush, s: &Style, rect: Rect, st: State, tap: SendTap, k: f32) {
    use super::color::{on_fill, with_alpha};
    use super::text::{self, Role, Text};
    use super::Fill;
    let t = &s.t;
    let r = b.snap_rect(rect);
    let radius = s.r(3.0).min(r.height() * 0.5);
    let ink = if tap.is_post() {
        // Post-fader, the usual: quiet, an outline and dim words.
        let edge = with_alpha(t.text_dim, if st.hovered { 0.9 } else { 0.5 });
        if st.hovered {
            b.rrect(r, radius, Fill::Solid(with_alpha(t.text, 0.08)));
        }
        b.rrect_stroke(r.shrink(b.px() * 0.5), radius, b.px(), edge);
        if st.hovered {
            t.text
        } else {
            t.text_dim
        }
    } else {
        // Pre-fader: lit in its own hue, so it is told apart at a glance.
        let hue = t.hue(function(tap));
        let fill = if st.hovered {
            super::color::shade(hue, 0.08)
        } else {
            hue
        };
        b.rrect(r, radius, Fill::Solid(fill));
        on_fill(fill)
    };
    if st.focused {
        b.rrect_stroke(r.expand(b.px()), radius, b.px(), t.focus);
    }
    let tx = Text::key(tap.label_key(), Role::Micro, ink)
        .caps(false)
        .align(egui::Align2::CENTER_CENTER);
    let _ = text::paint(b, r, &tx, k);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tap_has_a_segment_a_hue_and_words() {
        crate::i18n::default_thread_locale(crate::i18n::Locale::En);
        for (i, tap) in SendTap::ALL.into_iter().enumerate() {
            assert_eq!(index(tap), i);
            let (name, value) = hint_keys(tap);
            assert_ne!(crate::i18n::t(name), name, "{name} has no words");
            assert_ne!(crate::i18n::t(value), value, "{value} has no words");
        }
        assert_ne!(function(SendTap::Post), function(SendTap::Pre));
    }
}
