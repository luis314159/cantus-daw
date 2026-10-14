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
    /// Modulation arriving at this control: (depth as a fraction of the travel, −1…1,
    /// colour of its source), drawn as bands outside the track (ADR 0023).
    rings: Vec<(f32, egui::Color32)>,
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
            rings: Vec::new(),
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

    /// Show a modulation range: `depth` of the travel (−1…1) from the value, in `color`.
    /// Several rings stack outwards.
    pub fn ring(mut self, depth: f32, color: egui::Color32) -> Self {
        self.rings.push((depth, color));
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
            rings,
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
        // Exact entry on right-click: the kit's field under the knob (round 26), the value
        // selected. What is typed is what is set (read as the readouts print it, a unit or
        // none), clamped; the knob follows. Esc leaves it as it was.
        let mut typed = None;
        let tid = response.id.with("typed");
        if response.secondary_clicked() {
            let text = cantus_core::fx::readout::readout(start, &unit);
            ui.data_mut(|d| d.insert_temp(tid, TypedValue(text)));
        }
        if let Some(mut tv) = ui.data(|d| d.get_temp::<TypedValue>(tid)) {
            let ks = crate::kit::Style::current();
            let size = egui::vec2(96.0 * ks.k, ks.d(22.0));
            let at = response.rect.center_bottom() + egui::vec2(-size.x * 0.5, 2.0);
            let mut event = None;
            let mut edited = false;
            egui::Area::new(tid)
                .order(egui::Order::Foreground)
                .fixed_pos(at)
                .constrain(true)
                .show(ui.ctx(), |ui| {
                    let (r, _) = ui.allocate_exact_size(size, Sense::hover());
                    let out = crate::kit::field::Field::new(tid)
                        .role(crate::design::role::Role::Value)
                        .centered()
                        .opens_when_shown()
                        .show(ui, r, &mut tv.0);
                    (event, edited) = (out.event, out.edited);
                });
            match event {
                Some(crate::kit::field::Event::Commit) => {
                    // The readout untouched sets nothing (it would round the value).
                    if edited {
                        typed = cantus_core::fx::readout::parse_readout(&tv.0, &unit);
                    }
                    ui.data_mut(|d| d.remove::<TypedValue>(tid));
                }
                Some(crate::kit::field::Event::Cancel) => {
                    ui.data_mut(|d| d.remove::<TypedValue>(tid));
                }
                None => {
                    ui.data_mut(|d| d.insert_temp(tid, tv));
                }
            }
        }
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
            // Modulation: a band per source, from the value to where the source can take
            // it, outside the track so the value arc stays readable.
            for (i, (depth, color)) in rings.iter().enumerate() {
                let to = (pv + depth).clamp(0.0, 1.0);
                if (to - pv).abs() > 0.002 {
                    painter.add(Shape::line(
                        arc(
                            angle(pv.min(to)),
                            angle(pv.max(to)),
                            r + 1.0 + 3.0 * i as f32,
                        ),
                        Stroke::new(2.0, *color),
                    ));
                }
            }
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
                    // The edge at rest is text_dim: the boundary of a control must reach 3:1
                    // against its surface (research/instruments/family.md §6.5, V1), and
                    // grid_bar measured 1.7–2.0:1 in the shipped themes.
                    if active {
                        pal.gold
                    } else if response.hovered() {
                        pal.text
                    } else {
                        pal.text_dim
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

    /// Final review, fields 1: an exact entry opened and left untouched — Enter, or a
    /// click elsewhere — sets nothing: no rounding to the readout's precision, no change
    /// reported (no undo step).
    #[test]
    fn an_untouched_exact_entry_sets_nothing() {
        use egui::{Event, Modifiers, PointerButton, Pos2, Rect};
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        let mut v = 1_234.567f32;
        let mut frame = 0u64;
        let mut changed = false;
        let mut run = |v: &mut f32, changed: &mut bool, events: Vec<Event>| {
            frame += 1;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                time: Some(frame as f64 / 60.0),
                events,
                ..Default::default()
            };
            let mut r = None;
            ctx.run_ui(input, |ui| {
                let resp = ui.add(
                    Knob::new(v, 20.0..=20_000.0, 1_000.0)
                        .log(true)
                        .unit("Hz")
                        .label("Freq"),
                );
                *changed |= resp.changed();
                r = Some(resp.rect);
            })
            .textures_delta
            .clear();
            r.unwrap()
        };
        let click = |at, button, pressed| Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        };
        for close in ["enter", "away"] {
            let at = run(&mut v, &mut changed, Vec::new()).center();
            run(&mut v, &mut changed, vec![Event::PointerMoved(at)]);
            for pressed in [true, false] {
                run(
                    &mut v,
                    &mut changed,
                    vec![click(at, PointerButton::Secondary, pressed)],
                );
            }
            run(&mut v, &mut changed, Vec::new());
            run(&mut v, &mut changed, Vec::new());
            assert!(crate::kit::field::active(&ctx), "{close}: the field opened");
            if close == "enter" {
                run(
                    &mut v,
                    &mut changed,
                    vec![Event::Key {
                        key: Key::Enter,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: Modifiers::NONE,
                    }],
                );
            } else {
                let away = egui::pos2(390.0, 290.0);
                run(&mut v, &mut changed, vec![Event::PointerMoved(away)]);
                for pressed in [true, false] {
                    run(
                        &mut v,
                        &mut changed,
                        vec![click(away, PointerButton::Primary, pressed)],
                    );
                }
            }
            run(&mut v, &mut changed, Vec::new());
            run(&mut v, &mut changed, Vec::new());
            assert!(
                !crate::kit::field::active(&ctx),
                "{close}: the field closed"
            );
            assert_eq!(v, 1_234.567, "{close}: the value as it was");
            assert!(!changed, "{close}: a change was reported");
        }
    }

    /// Third review, finding 11: the exact entry is the kit's field (no egui DragValue):
    /// a right-click opens it with the value selected, what is typed replaces it, Enter
    /// sets it (clamped), Esc leaves the knob as it was.
    #[test]
    fn a_right_click_types_an_exact_value_in_the_kits_field() {
        use egui::{Event, Modifiers, PointerButton, Pos2, Rect};
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        let mut v = 0.37f32;
        let mut frame = 0u64;
        let mut run = |v: &mut f32, events: Vec<Event>| {
            frame += 1;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0))),
                time: Some(frame as f64 / 60.0),
                events,
                ..Default::default()
            };
            let mut r = None;
            ctx.run_ui(input, |ui| {
                r = Some(ui.add(Knob::new(v, 0.0..=1.0, 0.5).label("Mix")).rect);
            })
            .textures_delta
            .clear();
            r.unwrap()
        };
        let key = |k| Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        for (typed, end, want) in [
            ("0.8", Key::Enter, 0.8f32),
            ("2", Key::Enter, 1.0),
            ("0.1", Key::Escape, 1.0),
        ] {
            let rect = run(&mut v, Vec::new());
            let at = rect.center();
            run(&mut v, vec![Event::PointerMoved(at)]);
            for pressed in [true, false] {
                run(
                    &mut v,
                    vec![Event::PointerButton {
                        pos: at,
                        button: PointerButton::Secondary,
                        pressed,
                        modifiers: Modifiers::NONE,
                    }],
                );
            }
            run(&mut v, Vec::new());
            run(&mut v, Vec::new());
            assert!(crate::kit::field::active(&ctx), "the field opened");
            run(&mut v, vec![Event::Text(typed.into())]);
            run(&mut v, vec![key(end)]);
            run(&mut v, Vec::new());
            assert!((v - want).abs() < 1e-5, "{typed:?} then {end:?}: {v}");
        }
    }
}

/// The words typed into a knob's exact entry, kept by the knob while it is open.
#[derive(Clone, Debug, Default)]
struct TypedValue(String);
