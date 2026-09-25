//! A rotary knob: the house control for every parameter that is not a fader or a bar.
//!
//! egui's slider is a rail with a number beside it, which is how a debug panel looks. A
//! parameter in an instrument or an effect is read at a glance — *roughly where is it,
//! and is it off its default* — and a knob answers both in 36 px: the arc says how far,
//! and an arc that starts from the **default** rather than the minimum says which way it
//! has been moved. That is the same rule the bipolar bar follows (`widgets::bar`), in a
//! shape that packs into a grid.
//!
//! ## Interaction (every one of these is also how the fader and the bar behave)
//!
//! - **Drag** up or right to increase; the whole travel is 200 px, so a knob is not
//!   twitchier than a fader. **Shift** while dragging is fine control, a fifth of the rate.
//! - **Double-click** resets to the default.
//! - **Arrow keys** step by 1 % when focused (0.1 % with Shift), and claim the arrows so
//!   the note nudge does not fire at the same time (`widgets::claim_arrow_keys`).
//! - **Right-click** opens an exact entry box: a number typed is a number set.
//! - **Wheel** moves it only when the caller says so, like the fader: inside a vertical
//!   scroll area the wheel belongs to the scroll.
//!
//! The mapping is linear or logarithmic (frequencies, times, Q) and can snap to integers.
//! Position and value are exact inverses, so the pointer never drifts off the knob.

use crate::design;
use crate::theme;
use egui::{
    pos2, vec2, Align2, EventFilter, Key, Response, Sense, Shape, Stroke, Ui, Widget, WidgetInfo,
};
use std::f32::consts::PI;

/// Pixels of drag for the whole range.
const TRAVEL_PX: f32 = 200.0;
/// Shift: a fifth of the rate.
const FINE: f32 = 0.2;
/// The arc runs 270°, with the gap at the bottom, as on hardware.
const START: f32 = 0.75 * PI;
const SWEEP: f32 = 1.5 * PI;

/// How a knob maps its travel onto its range.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Taper {
    Linear,
    /// Equal ratios per equal travel. Needs a positive range.
    Log,
}

/// Position (0…1) of `v` on a knob over `min..=max`.
pub fn pos_of(v: f32, min: f32, max: f32, taper: Taper) -> f32 {
    let v = v.clamp(min, max);
    match taper {
        Taper::Log if min > 0.0 && max > min => (v / min).ln() / (max / min).ln(),
        _ => (v - min) / (max - min).max(1e-9),
    }
    .clamp(0.0, 1.0)
}

/// Value at position `p` (0…1). The exact inverse of [`pos_of`].
pub fn value_at(p: f32, min: f32, max: f32, taper: Taper) -> f32 {
    let p = p.clamp(0.0, 1.0);
    match taper {
        Taper::Log if min > 0.0 && max > min => min * (max / min).powf(p),
        _ => min + (max - min) * p,
    }
}

pub struct Knob<'a> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    default: f32,
    taper: Taper,
    stepped: bool,
    diameter: f32,
    wheel: bool,
    label: String,
    /// Turns the value into its readout. `None`: the number with `unit`.
    format: Option<Box<dyn Fn(f32) -> String + 'a>>,
    unit: String,
}

impl<'a> Knob<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>, default: f32) -> Self {
        let (min, max) = (*range.start(), *range.end());
        Knob {
            value,
            min,
            max,
            default: default.clamp(min, max),
            taper: Taper::Linear,
            stepped: false,
            diameter: design::space::GRID * 9.0,
            wheel: false,
            label: String::new(),
            format: None,
            unit: String::new(),
        }
    }

    pub fn log(mut self, on: bool) -> Self {
        self.taper = if on { Taper::Log } else { Taper::Linear };
        self
    }

    pub fn stepped(mut self, on: bool) -> Self {
        self.stepped = on;
        self
    }

    pub fn diameter(mut self, d: f32) -> Self {
        self.diameter = d;
        self
    }

    pub fn wheel(mut self, on: bool) -> Self {
        self.wheel = on;
        self
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = unit.into();
        self
    }

    pub fn format(mut self, f: impl Fn(f32) -> String + 'a) -> Self {
        self.format = Some(Box::new(f));
        self
    }
}

/// The readout of a number: no more digits than a musician reads. `1.2 kHz`, `350 Hz`,
/// `0.71`, `-3.5 dB`, `12 ms`. Written once in `cantus_core::fx::readout`, so the exported
/// plug-ins (ADR 0020) print what the knobs print.
pub fn readout(v: f32, unit: &str) -> String {
    cantus_core::fx::readout::readout(v, unit)
}

impl Widget for Knob<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let Knob {
            value,
            min,
            max,
            default,
            taper,
            stepped,
            diameter,
            wheel,
            label,
            format,
            unit,
        } = self;
        let text_h = 12.0;
        let width = (diameter + 20.0).max(diameter);
        let size = vec2(width, diameter + 2.0 * text_h + 4.0);
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let start = value.clamp(min, max);
        let p0 = pos_of(start, min, max, taper);
        let mut p = p0;
        let fine = if ui.input(|i| i.modifiers.shift) {
            FINE
        } else {
            1.0
        };
        if response.dragged() {
            let d = response.drag_delta();
            p += (d.x - d.y) * fine / TRAVEL_PX;
        }
        if wheel && response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                p += scroll * fine / (TRAVEL_PX * 2.0);
            }
        }
        if response.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    response.id,
                    EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                );
            });
            super::claim_arrow_keys(ui.ctx(), response.id);
            let steps = ui.input(|i| {
                (i.num_presses(Key::ArrowUp) + i.num_presses(Key::ArrowRight)) as i32
                    - (i.num_presses(Key::ArrowDown) + i.num_presses(Key::ArrowLeft)) as i32
            });
            if steps != 0 {
                p += steps as f32 * 0.01 * fine;
            }
        }
        // Only a gesture moves the value. Going through the position and back is exact
        // only to a rounding step, and a knob that rewrote its value on every frame would
        // issue a command on every frame.
        let mut v = if p != p0 {
            value_at(p, min, max, taper)
        } else {
            start
        };
        if response.double_clicked() {
            v = default;
        }
        // Exact entry on right-click: a popup with a number box. What is typed is what
        // is set, clamped; the knob follows.
        let mut typed = None;
        egui::Popup::context_menu(&response).show(|ui| {
            let mut x = start;
            let speed = ((max - min) / 400.0).max(1e-4) as f64;
            let r = ui.add(
                egui::DragValue::new(&mut x)
                    .range(min..=max)
                    .speed(speed)
                    .suffix(if unit.is_empty() {
                        String::new()
                    } else {
                        format!(" {unit}")
                    }),
            );
            r.request_focus();
            if r.changed() {
                typed = Some(x);
            }
        });
        if let Some(x) = typed {
            v = x;
        }
        if stepped {
            v = v.round();
        }
        let v = v.clamp(min, max);
        if v != start {
            *value = v;
            response.mark_changed();
        }

        if ui.is_rect_visible(rect) {
            let pal = theme::p();
            let painter = ui.painter();
            let c = pos2(rect.center().x, rect.top() + diameter * 0.5 + 1.0);
            let r = diameter * 0.5;
            let angle = |pos: f32| START + SWEEP * pos;
            let arc = |from: f32, to: f32, radius: f32| -> Vec<egui::Pos2> {
                let n = ((to - from).abs() / SWEEP * 48.0).ceil().max(2.0) as usize;
                (0..=n)
                    .map(|i| {
                        let a = from + (to - from) * i as f32 / n as f32;
                        pos2(c.x + radius * a.cos(), c.y + radius * a.sin())
                    })
                    .collect()
            };
            let track_r = r - 2.0;
            // The track: the whole 270°, recessed.
            painter.add(Shape::line(
                arc(angle(0.0), angle(1.0), track_r),
                Stroke::new(3.0, theme::recess(&pal)),
            ));
            // The value arc runs from the default, so an untouched knob shows nothing and
            // a moved one shows which way and how far.
            let pv = pos_of(v, min, max, taper);
            let pd = pos_of(default, min, max, taper);
            let active = response.dragged() || response.has_focus();
            if (pv - pd).abs() > 0.002 {
                painter.add(Shape::line(
                    arc(angle(pd.min(pv)), angle(pd.max(pv)), track_r),
                    Stroke::new(3.0, if active { pal.gold_bright } else { pal.gold }),
                ));
            }
            // The body: a raised disc with a read line to the value.
            let body_r = r - 6.0;
            painter.circle_filled(c, body_r, pal.panel_raised);
            painter.circle_stroke(
                c,
                body_r,
                Stroke::new(
                    if active { 1.5 } else { 1.0 },
                    if active {
                        pal.gold
                    } else if response.hovered() {
                        pal.text_dim
                    } else {
                        pal.grid_bar
                    },
                ),
            );
            let a = angle(pv);
            painter.line_segment(
                [
                    pos2(c.x + body_r * 0.25 * a.cos(), c.y + body_r * 0.25 * a.sin()),
                    pos2(c.x + body_r * 0.9 * a.cos(), c.y + body_r * 0.9 * a.sin()),
                ],
                Stroke::new(2.0, pal.text),
            );
            // Label, then the value — monospaced, brighter while it is being changed.
            let label_y = rect.top() + diameter + 2.0;
            painter.text(
                pos2(rect.center().x, label_y),
                Align2::CENTER_TOP,
                &label,
                design::paint::caption(),
                pal.text_dim,
            );
            let shown = match &format {
                Some(f) => f(v),
                None => readout(v, &unit),
            };
            painter.text(
                pos2(rect.center().x, label_y + text_h),
                Align2::CENTER_TOP,
                shown,
                design::paint::ruler(),
                if active || response.hovered() {
                    pal.text
                } else {
                    pal.text_dim
                },
            );
        }
        response.widget_info(|| WidgetInfo::slider(ui.is_enabled(), v as f64, &label));
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_and_value_are_exact_inverses() {
        for taper in [Taper::Linear, Taper::Log] {
            let (min, max) = (20.0, 20_000.0);
            for i in 0..=1000 {
                let p = i as f32 / 1000.0;
                let back = pos_of(value_at(p, min, max, taper), min, max, taper);
                assert!((back - p).abs() < 1e-4, "{taper:?} at {p}: {back}");
            }
        }
        // A log knob puts 632 Hz (the geometric middle) at the middle of its travel.
        let mid = pos_of(632.46, 20.0, 20_000.0, Taper::Log);
        assert!((mid - 0.5).abs() < 1e-3);
    }

    #[test]
    fn readouts_are_what_a_musician_reads() {
        assert_eq!(readout(1_500.0, "Hz"), "1.5 kHz");
        assert_eq!(readout(350.0, "Hz"), "350 Hz");
        assert_eq!(readout(-3.5, "dB"), "-3.5 dB");
        assert_eq!(readout(0.25, "%"), "25%");
        assert_eq!(readout(12.0, "ms"), "12.0 ms");
        assert_eq!(readout(0.25, "s"), "250 ms");
    }

    #[test]
    fn drawing_a_knob_does_not_move_it() {
        let ctx = egui::Context::default();
        crate::design::install_style(&ctx);
        let mut v = 0.37f32;
        ctx.run_ui(egui::RawInput::default(), |ui| {
            ui.add(Knob::new(&mut v, 0.0..=1.0, 0.5).label("Mix"));
        })
        .textures_delta
        .clear();
        assert_eq!(v, 0.37);
        // A log knob at a value that does not survive position → value exactly must
        // still report no change: an untouched knob issues no command.
        let mut hz = 1_234.567f32;
        let mut changed = false;
        for _ in 0..3 {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                changed |= ui
                    .add(Knob::new(&mut hz, 20.0..=20_000.0, 1_000.0).log(true))
                    .changed();
            })
            .textures_delta
            .clear();
        }
        assert_eq!(hz, 1_234.567);
        assert!(!changed, "an untouched knob reported a change");
    }
}
