//! Lenis's focus on the screen (design sheet v1): the two limits and the six bands answer
//! the pointer. A gate (its line or its tab) drags left and right; a band's node drags in
//! frequency (left and right) and amount (up and down) as one step, the wheel over it sets
//! its width, a double-click switches it on or off, a click selects it (the *Focus band*
//! card then shows it). Each grip's menu offers automation and MIDI learn on what it moves
//! (the kit's `stepped_offer`); the help line says what it does.

use super::super::controls;
use super::super::frame::tr;
use super::super::host::FxHost;
use super::screen::{hz_words, Grip, Plot};
use crate::kit::hint::{self, Caps};
use crate::kit::Style;
use cantus_core::fx::lenis::band_key;
use cantus_dsp::lenis::{LenisSettings, FOCUS_BANDS};
use egui::{Pos2, Sense};

/// What the screen remembers between frames: the grip held, the drag's undo step.
#[derive(Debug, Default)]
pub struct Hold {
    pub grip: Option<Grip>,
    gesture: u64,
}

/// The grip under `p`: a node first (the nearest within reach), then a gate's tab or line.
pub fn grip_at(plot: &Plot, set: &LenisSettings, p: Pos2, k: f32, tabs: [f32; 2]) -> Option<Grip> {
    let mut best: Option<(f32, usize)> = None;
    for n in 0..FOCUS_BANDS {
        let d = plot.node(set, n).distance(p);
        if d <= 11.0 * k && best.is_none_or(|(bd, _)| d < bd) {
            best = Some((d, n));
        }
    }
    if let Some((_, n)) = best {
        return Some(Grip::Band(n));
    }
    if !plot.area.expand(4.0 * k).contains(p) {
        return None;
    }
    for (hi, w) in [(false, tabs[0]), (true, tabs[1])] {
        let grip = if hi { Grip::Hi } else { Grip::Lo };
        if plot.tab(set, hi, w, k).expand(2.0 * k).contains(p) {
            return Some(grip);
        }
        let x = plot.x(if hi { set.high_hz } else { set.low_hz });
        if (p.x - x).abs() <= 5.0 * k {
            return Some(grip);
        }
    }
    None
}

/// The screen's grips answer the pointer; returns the grip under it (or held).
#[allow(clippy::too_many_arguments)]
pub fn edit(
    host: &mut dyn FxHost,
    ui: &mut egui::Ui,
    s: &Style,
    plot: &Plot,
    set: &LenisSettings,
    hold: &mut Hold,
    sel: &mut usize,
    tabs: [f32; 2],
) -> Option<Grip> {
    let k = s.k;
    let id = ui.id().with("lenis-screen");
    let resp = ui.interact(plot.area.expand(10.0 * k), id, Sense::click_and_drag());
    let pointer = resp.interact_pointer_pos().or_else(|| resp.hover_pos());
    let under = pointer.and_then(|p| grip_at(plot, set, p, k, tabs));
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin()).or(pointer);
        hold.grip = origin.and_then(|p| grip_at(plot, set, p, k, tabs));
        hold.gesture = hold.gesture.wrapping_add(1) | (id.value() << 8);
        if let Some(Grip::Band(n)) = hold.grip {
            *sel = n;
        }
    }
    if resp.clicked() {
        if let Some(Grip::Band(n)) = under {
            *sel = n;
        }
    }
    // The menu of what is under the pointer: the gate's limit, the band's amount.
    let menu_key = match under.or(hold.grip) {
        Some(Grip::Lo) => Some(("lo", tr("fx.lenis.lo"))),
        Some(Grip::Hi) => Some(("hi", tr("fx.lenis.hi"))),
        Some(Grip::Band(n)) => Some((band_key(n, "amt"), tr("fx.lenis.bamt"))),
        None => None,
    };
    let learnt =
        menu_key.is_some_and(|(key, name)| controls::stepped_offer(ui, s, host, key, name, &resp));
    if learnt {
        return under;
    }
    if let (Some(g), Some(p), true) = (hold.grip, pointer, resp.dragged()) {
        let hz = plot.hz(p.x);
        match g {
            Grip::Lo => {
                let v = hz.clamp(20.0, set.high_hz / 1.25);
                host.set("lo", v, Some("lo"));
            }
            Grip::Hi => {
                let v = hz.clamp(set.low_hz * 1.25, 20_000.0);
                host.set("hi", v, Some("hi"));
            }
            Grip::Band(n) => {
                let amt = (plot.w_at(p.y) - 1.0).clamp(-1.0, 1.0);
                let mut edits = [
                    (band_key(n, "hz"), Some(hz.clamp(20.0, 20_000.0))),
                    (band_key(n, "amt"), Some(amt)),
                    (band_key(n, "on"), None),
                ];
                if !set.bands[n].on {
                    edits[2].1 = Some(1.0);
                }
                host.apply(&edits, "cmd.lenis_band", Some(hold.gesture));
            }
        }
    }
    if resp.drag_stopped() && hold.grip.take().is_some() {
        host.end_gesture();
    }
    if resp.double_clicked() {
        if let Some(Grip::Band(n)) = under {
            let on = set.bands[n].on;
            host.set(band_key(n, "on"), if on { 0.0 } else { 1.0 }, None);
        }
    }
    // The wheel over a band sets its width (an eighth of an octave a notch).
    if let Some(Grip::Band(n)) = under {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 {
            let w = set.bands[n].width_oct;
            let v = (w * cantus_dsp::math::powf(2.0, dy / 240.0)).clamp(0.1, 4.0);
            host.set(band_key(n, "w"), v, Some(band_key(n, "w")));
        }
    }
    let shown = hold.grip.or(under);
    if let Some(g) = shown {
        hint_for(ui.ctx(), set, g, resp.dragged());
    }
    shown
}

/// The help line for a grip: its name and value, then what it does.
fn hint_for(ctx: &egui::Context, set: &LenisSettings, g: Grip, live: bool) {
    use std::fmt::Write as _;
    let mut words = String::new();
    match g {
        Grip::Lo | Grip::Hi => {
            let hz = if g == Grip::Hi {
                set.high_hz
            } else {
                set.low_hz
            };
            hz_words(hz, &mut words);
            let name = tr(if g == Grip::Hi {
                "fx.lenis.hi"
            } else {
                "fx.lenis.lo"
            });
            hint::set(ctx, name, Caps::PARAM, live, |b| {
                write!(b, "{words} \u{b7} {}", tr("lenis.tip.gate"))
            });
        }
        Grip::Band(n) => {
            let b0 = set.bands[n];
            hz_words(b0.hz, &mut words);
            let name = crate::i18n::tf("lenis.d.band", &[("n", &(n + 1))]);
            hint::set(ctx, &name, Caps::PARAM, live, |b| {
                write!(
                    b,
                    "{words} \u{b7} {:+.0} % \u{b7} {:.2} {} \u{b7} {}",
                    b0.amount * 100.0,
                    b0.width_oct,
                    tr("lenis.u.octaves"),
                    tr("lenis.tip.node")
                )
            });
        }
    }
}
