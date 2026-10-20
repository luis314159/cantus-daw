//! The effect editors' controls, drawn by the kit at the device's scale and in its
//! section's hue (ADR 0024 §4): the parameter **cell** (value pill, knob, label — one
//! object), the **LED button** for every on/off, the **segmented** control for every
//! choice among a few words, the **menu field** for longer lists, an **action key** for
//! verbs (Learn, Match), and the **card** that holds them.
//!
//! The kit's own widgets read the installed style at 100 %; these take the device's
//! [`Style`] (its scale `k` and the card's hue), and the cell keeps the house gestures of
//! `widgets::knob`: 200 px of drag for the whole travel, Shift for fine, double-click for
//! the default, arrows when focused, **right-click to type** a value into the pill (what
//! the readout prints reads back: `2k`, `-6`, `1.5 s`), and a value moves only on a
//! gesture — drawing a cell never issues a command.
//!
//! Values are formatted into one reused buffer, so a control that is not moving costs no
//! allocation.

use super::frame::{popup_list, tr};
use super::host::FxHost;
use crate::kit::band::{self, CardLook, CardRects};
use crate::kit::cell::{self, CellLook, Control};
use crate::kit::hint::{self, Caps};
use crate::kit::knob::{KnobLook, KnobSize};
use crate::kit::light::{self, RaisedOpts};
use crate::kit::menu::Item;
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::{focus_ring, well, with_brush, State, Style};
use crate::widgets::knob::{pos_of, value_at, Taper};
use egui::{Align2, EventFilter, Key, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::cell::RefCell;

pub use crate::kit::knob::KnobSize as Size;

/// Pixels of drag for the whole travel; Shift is a fifth of the rate.
const TRAVEL_PX: f32 = 200.0;
const FINE: f32 = 0.2;

thread_local! {
    static BUF: RefCell<String> = RefCell::new(String::with_capacity(32));
}

/// Run `f` with a value formatted into the shared buffer.
fn with_text<R>(fmt: impl FnOnce(&mut String), f: impl FnOnce(&str) -> R) -> R {
    BUF.with(|b| match b.try_borrow_mut() {
        Ok(mut buf) => {
            buf.clear();
            fmt(&mut buf);
            f(&buf)
        }
        Err(_) => {
            let mut s = String::new();
            fmt(&mut s);
            f(&s)
        }
    })
}

/// A value as `cantus_core::fx::readout::readout` prints it, written into `out` (no
/// allocation once `out` has grown; a formatter takes it straight, allocating nothing):
/// `1.2 kHz`, `350 Hz`, `-3.5 dB`, `12.0 ms`, `25%`.
pub fn write_readout(v: f32, unit: &str, out: &mut impl std::fmt::Write) {
    let _ = match unit {
        "Hz" if v.abs() >= 1_000.0 => write!(out, "{:.1} kHz", v / 1_000.0),
        "Hz" if v.abs() < 10.0 => write!(out, "{v:.2} Hz"),
        "Hz" => write!(out, "{v:.0} Hz"),
        "ms" if v >= 1_000.0 => write!(out, "{:.2} s", v / 1_000.0),
        "ms" if v >= 100.0 => write!(out, "{v:.0} ms"),
        "ms" => write!(out, "{v:.1} ms"),
        "s" if v < 1.0 => write!(out, "{:.0} ms", v * 1_000.0),
        "s" => write!(out, "{v:.2} s"),
        "dB" => write!(out, "{v:+.1} dB"),
        "st" => write!(out, "{v:+.1} st"),
        "ct" => write!(out, "{v:+.0} ct"),
        "%" => write!(out, "{:.0}%", v * 100.0),
        "\u{b0}" => write!(out, "{v:+.1}\u{b0}"),
        "" if v.abs() >= 100.0 => write!(out, "{v:.0}"),
        "" => write!(out, "{v:.2}"),
        u => write!(out, "{v:.2} {u}"),
    };
}

/// What writes a pill's text for a value (a ratio as `4.0:1`, ∞).
pub type Pill<'a> = &'a dyn Fn(f32, &mut String);

/// A cell on any value. Build it, then [`Cell::show`].
pub struct Cell<'a> {
    value: f32,
    min: f32,
    max: f32,
    default: f32,
    taper: Taper,
    stepped: bool,
    size: KnobSize,
    label: Src<'a>,
    name: &'a str,
    unit: &'a str,
    format: Option<Pill<'a>>,
    wheel: bool,
    rings: &'a [(f32, egui::Color32)],
    /// The parameter's menu (ADR 0027): right-click offers automate, type, reset.
    offer: Option<crate::kit::automate::Offer>,
}

/// What a cell did this frame.
#[derive(Clone, Debug)]
pub struct CellOut {
    pub resp: Response,
    /// The value it moved to, when a gesture moved it.
    pub changed: Option<f32>,
    /// The gesture ended (a drag let go, a reset, a typed value): the end of an undo step.
    pub done: bool,
    /// Its menu asked to automate it (create the clip, or open it).
    pub automate: bool,
    /// What its modulation did (ADR 0057): *Modulate…*, a depth drag, a handle dropped.
    pub modulation: Option<crate::kit::automate::Choice>,
}

impl<'a> Cell<'a> {
    pub fn new(value: f32, range: std::ops::RangeInclusive<f32>, default: f32) -> Cell<'a> {
        let (min, max) = (*range.start(), *range.end());
        Cell {
            value,
            min,
            max,
            default: default.clamp(min, max),
            taper: Taper::Linear,
            stepped: false,
            size: KnobSize::M,
            label: Src::Str(""),
            name: "",
            unit: "",
            format: None,
            wheel: false,
            rings: &[],
            offer: None,
        }
    }

    /// A cell on one of an effect's parameters: its range, taper, steps, unit and label.
    pub fn param(info: &'static cantus_core::EffectParamInfo, value: f32) -> Cell<'static> {
        Cell {
            taper: if info.logarithmic {
                Taper::Log
            } else {
                Taper::Linear
            },
            stepped: info.stepped,
            label: Src::Key(info.label_key),
            name: tr(info.label_key),
            unit: info.unit,
            ..Cell::new(value, info.min..=info.max, info.default)
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
    pub fn size(mut self, s: KnobSize) -> Self {
        self.size = s;
        self
    }
    /// The label under the knob: an i18n key, so it can abbreviate (`<key>.short`).
    pub fn label(mut self, l: Src<'a>) -> Self {
        self.label = l;
        self
    }
    /// The full name, for the help line.
    pub fn name(mut self, n: &'a str) -> Self {
        self.name = n;
        self
    }
    pub fn unit(mut self, u: &'a str) -> Self {
        self.unit = u;
        self
    }
    /// Write the pill's text (instead of the unit's readout).
    pub fn format(mut self, f: &'a dyn Fn(f32, &mut String)) -> Self {
        self.format = Some(f);
        self
    }
    pub fn wheel(mut self, on: bool) -> Self {
        self.wheel = on;
        self
    }
    pub fn rings(mut self, r: &'a [(f32, egui::Color32)]) -> Self {
        self.rings = r;
        self
    }
    /// The parameter's menu on a right-click instead of typing straight away (typing is
    /// one of its entries), and the automated mark when `automated`.
    pub fn automation(mut self, automated: bool) -> Self {
        self.offer = Some(crate::kit::automate::Offer {
            automated,
            typing: true,
            modulation: self.offer.and_then(|o| o.modulation),
        });
        self
    }
    /// It can be modulated (ADR 0057): *Modulate…* in its menu, a modulator's handle
    /// dropped on it, Alt+drag (or a mapping modulator's drag) for the depth, the live dot.
    pub fn modulation(mut self, m: crate::kit::automate::ModOffer) -> Self {
        let mut o = self.offer.unwrap_or(crate::kit::automate::Offer {
            typing: true,
            ..Default::default()
        });
        o.modulation = Some(m);
        self.offer = Some(o);
        self
    }

    fn write(&self, v: f32, out: &mut String) {
        match self.format {
            Some(f) => f(v, out),
            None => write_readout(v, self.unit, out),
        }
    }

    /// [`Cell::show`] with what Cantus adds to every parameter (ADR 0027, 0057): its
    /// automation menu and mark, its modulation — *Modulate…* in the menu, a modulator's
    /// handle dropped on it, the Alt-drag depth, the rings and the live dot — each answered
    /// through `host`. Every editor's cell on a parameter goes through here (review 1), so
    /// none can miss it; the caller applies `changed` and `done`.
    pub fn show_in(mut self, ui: &mut Ui, s: &Style, host: &mut dyn FxHost, key: &str) -> CellOut {
        use crate::views::modulator::MAX_RINGS;
        if host.can_automate() {
            self = self.automation(host.automated(key));
        }
        let mut rings = [(0.0, egui::Color32::TRANSPARENT); MAX_RINGS];
        let base = pos_of(
            self.value.clamp(self.min, self.max),
            self.min,
            self.max,
            self.taper,
        );
        let modulation = host.modulation(key, base, s, &mut rings);
        let n = modulation.map_or(0, |(_, n)| n);
        if let Some((m, _)) = modulation {
            self = self.modulation(m);
        }
        // The cell's own rings first, then its modulators'.
        let own = self.rings.len().min(MAX_RINGS);
        let mut all = [(0.0, egui::Color32::TRANSPARENT); 2 * MAX_RINGS];
        all[..own].copy_from_slice(&self.rings[..own]);
        all[own..own + n].copy_from_slice(&rings[..n]);
        let out = self.rings(&all[..own + n]).show(ui, s);
        if let Some(choice) = out.modulation {
            host.modulate(key, choice, out.resp.rect);
        }
        if out.automate {
            host.automate(key);
        }
        out
    }

    pub fn show(self, ui: &mut Ui, s: &Style) -> CellOut {
        crate::kit::automate::probe::note(
            if self.name.is_empty() {
                match self.label {
                    Src::Key(k) => k,
                    Src::Str(t) => t,
                }
            } else {
                self.name
            },
            self.offer.is_some_and(|o| o.modulation.is_some()),
        );
        crate::kit::automate::probe::note_learn(
            if self.name.is_empty() {
                match self.label {
                    Src::Key(k) => k,
                    Src::Str(t) => t,
                }
            } else {
                self.name
            },
            self.offer.and_then(|o| o.learn()).is_some(),
        );
        let size = cell::size(self.size) * s.k;
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let (min, max, taper) = (self.min, self.max, self.taper);
        let start = self.value.clamp(min, max);
        let p0 = pos_of(start, min, max, taper);
        let mut p = p0;
        let fine = if ui.input(|i| i.modifiers.shift) {
            FINE
        } else {
            1.0
        };
        // A depth drag (ADR 0057) moves the modulation, not the value.
        let depth_drag = crate::kit::automate::depth_drag(ui, self.offer);
        let mod_choice = crate::kit::automate::modulation_choice(ui, &resp, self.offer);
        if resp.dragged() && !depth_drag {
            let d = resp.drag_delta();
            p += (d.x - d.y) * fine / TRAVEL_PX;
        }
        if self.wheel && resp.hovered() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            p += dy * fine / (TRAVEL_PX * 2.0);
        }
        let mut keyed = false;
        if resp.has_focus() {
            ui.memory_mut(|m| {
                m.set_focus_lock_filter(
                    resp.id,
                    EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                )
            });
            crate::widgets::claim_arrow_keys(ui.ctx(), resp.id);
            let steps = ui.input(|i| {
                (i.num_presses(Key::ArrowUp) + i.num_presses(Key::ArrowRight)) as i32
                    - (i.num_presses(Key::ArrowDown) + i.num_presses(Key::ArrowLeft)) as i32
            });
            if steps != 0 {
                p += steps as f32 * 0.01 * fine;
                keyed = true;
            }
        }
        // Only a gesture moves the value: going through the position and back is exact only
        // to a rounding step, and a cell that rewrote its value every frame would issue a
        // command every frame.
        let mut v = if p != p0 {
            value_at(p.clamp(0.0, 1.0), min, max, taper)
        } else {
            start
        };
        let mut done = (resp.drag_stopped() && !depth_drag) || keyed;
        if resp.double_clicked() {
            v = self.default;
            done = true;
        }
        // Typing into the pill: right-click opens it, Enter sets, Esc leaves.
        let pill = cell::rects(
            rect,
            s,
            &CellLook {
                control: Control::Knob(KnobLook::new(self.size, 0.0, 0.0)),
                value: "",
                label: self.label,
            },
        )
        .pill;
        // The parameter's menu, where there is one; typing opens from it.
        let choice = self
            .offer
            .and_then(|o| crate::kit::automate::menu(ui, &resp, o));
        let open = match self.offer {
            Some(_) => choice == Some(crate::kit::automate::Choice::Type),
            None => resp.secondary_clicked(),
        };
        if choice == Some(crate::kit::automate::Choice::Reset) {
            v = self.default;
            done = true;
        }
        if let Some(x) = typing_with(
            ui,
            &resp,
            pill,
            s,
            self.unit,
            |b| self.write(start, b),
            open,
        ) {
            v = x;
            done = true;
        }
        if self.stepped {
            v = v.round();
        }
        let v = v.clamp(min, max);
        let changed = (v != start).then_some(v);
        let mut resp = resp;
        if changed.is_some() {
            resp.mark_changed();
        }
        let st = State::of(&resp, false);
        if st.hovered {
            hint::set(ui.ctx(), self.name, Caps::PARAM, resp.dragged(), |b| {
                self.write(v, b);
                Ok(())
            });
        }
        if ui.is_rect_visible(rect) {
            let look_value = pos_of(v, min, max, taper);
            let look_default = pos_of(self.default, min, max, taper);
            let painter = ui.painter_at(rect.expand(4.0 * s.k));
            with_text(
                |b| self.write(v, b),
                |text| {
                    with_brush(&painter, |b| {
                        let l = CellLook {
                            control: Control::Knob(KnobLook {
                                size: self.size,
                                value: look_value,
                                default: look_default,
                                rings: self.rings,
                                live: self.offer.and_then(|o| o.modulation).and_then(|m| m.live),
                            }),
                            value: text,
                            label: self.label,
                        };
                        cell::paint(b, rect, s, st, &l);
                        if self.offer.is_some_and(|o| o.automated) {
                            crate::kit::automate::mark(b, rect, s);
                        }
                        let m = self.offer.and_then(|o| o.modulation);
                        if m.is_some_and(|m| m.depth.is_some() && !m.mapping) {
                            crate::kit::automate::mod_mark(b, rect, s);
                        }
                        if m.and_then(|m| m.learn).is_some_and(|l| l.mapped) {
                            crate::kit::automate::learn_mark(b, rect, s);
                        }
                        if m.is_some()
                            && crate::kit::automate::handle_in_flight(ui.ctx())
                            && resp.contains_pointer()
                        {
                            crate::kit::automate::drop_target(b, rect, s);
                        }
                    })
                },
            );
        }
        CellOut {
            resp,
            changed,
            done,
            automate: choice == Some(crate::kit::automate::Choice::Automate),
            modulation: match choice {
                Some(
                    c @ (crate::kit::automate::Choice::Modulate
                    | crate::kit::automate::Choice::Learn
                    | crate::kit::automate::Choice::Forget),
                ) => Some(c),
                _ => mod_choice,
            },
        }
    }
}

/// The text being typed into a pill, kept by the pill's id.
#[derive(Clone, Debug, Default)]
struct Typed(String);

/// Right-click on `resp` opens a text box over `pill`, prefilled by `current`. Returns the
/// value typed when Enter is pressed and it reads as a number.
pub(crate) fn typing(
    ui: &mut Ui,
    resp: &Response,
    pill: Rect,
    s: &Style,
    unit: &str,
    current: impl FnOnce(&mut String),
) -> Option<f32> {
    typing_with(ui, resp, pill, s, unit, current, resp.secondary_clicked())
}

/// [`typing`], opened when `open` says (a menu's *Type a value...*) rather than on a
/// right-click.
pub(crate) fn typing_with(
    ui: &mut Ui,
    resp: &Response,
    pill: Rect,
    s: &Style,
    unit: &str,
    current: impl FnOnce(&mut String),
    open: bool,
) -> Option<f32> {
    let id = resp.id.with("typing");
    if open {
        let mut text = String::new();
        current(&mut text);
        ui.data_mut(|d| d.insert_temp(id, Typed(text)));
    }
    let mut typed = ui.data(|d| d.get_temp::<Typed>(id))?;
    let r = pill.expand2(Vec2::new(6.0 * s.k, 2.0 * s.k));
    // The kit's field (round 26): the value selected as it opens; Enter or a click
    // elsewhere sets what reads as a number, Esc leaves. Over the cell, whose face is
    // painted after this (third review), at the editor's scale.
    let mut event = None;
    let mut edited = false;
    egui::Area::new(id.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(r.min)
        .show(ui.ctx(), |ui| {
            let (r, _) = ui.allocate_exact_size(r.size(), Sense::hover());
            let out = crate::kit::field::Field::new(id)
                .role(crate::design::role::Role::Value)
                .centered()
                .scale(s.k)
                .opens_when_shown()
                .show(ui, r, &mut typed.0);
            (event, edited) = (out.event, out.edited);
        });
    match event {
        Some(crate::kit::field::Event::Commit) => {
            ui.data_mut(|d| d.remove::<Typed>(id));
            // The readout untouched sets nothing (it would round the value and end a
            // host undo step).
            edited
                .then(|| cantus_core::fx::readout::parse_readout(&typed.0, unit))
                .flatten()
        }
        Some(crate::kit::field::Event::Cancel) => {
            ui.data_mut(|d| d.remove::<Typed>(id));
            None
        }
        None => {
            ui.data_mut(|d| d.insert_temp(id, typed));
            None
        }
    }
}

/// A cell bound to parameter `key` of the effect: a drag is one step of the host's undo.
pub fn param(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &'static str,
    size: KnobSize,
) -> Response {
    param_with(ui, s, host, key, size, None)
}

/// [`param`] with a pill of its own words (a ratio as `4.0:1`, ∞).
pub fn param_with(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &'static str,
    size: KnobSize,
    format: Option<Pill<'_>>,
) -> Response {
    let spec = host.spec();
    let Some(info) = spec.kind.param(key) else {
        return ui.allocate_response(Vec2::ZERO, Sense::hover());
    };
    let mut c = Cell::param(info, spec.get(key)).size(size);
    if let Some(f) = format {
        c = c.format(f);
    }
    // In Cantus, the parameter's menu and its automated mark (ADR 0027), and its
    // modulation (ADR 0057).
    let out = c.show_in(ui, s, host, key);
    if let Some(v) = out.changed {
        host.set(key, v, Some(key));
    }
    if out.done {
        host.end_gesture();
    }
    out.resp
}

// ---- switches ---------------------------------------------------------------------------

/// The standard height of a switch row at 100 %.
pub const ROW: f32 = 22.0;

/// An on/off: an LED and a caps word (the kit's `LedButton` at the device's style).
/// Returns the response; clicked = flip it.
pub fn led(ui: &mut Ui, s: &Style, on: bool, label: Src<'_>, name: &str, width: f32) -> Response {
    crate::kit::widgets::LedButton::new(on, label)
        .name(name)
        .width(width)
        .style(*s)
        .show(ui)
}

/// An LED bound to a 0/1 parameter of the effect.
pub fn led_param(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &str,
    label: Src<'_>,
    width: f32,
) -> Response {
    let on = host.spec().get(key) >= 0.5;
    let name = host.spec().kind.param(key).map_or("", |i| tr(i.label_key));
    let r = led(ui, s, on, label, name, width);
    let learnt = stepped_offer(ui, s, host, key, name, &r);
    if r.clicked() && !learnt {
        host.set(key, if on { 0.0 } else { 1.0 }, None);
    }
    r
}

/// A stepped parameter's control (an LED, a segmented choice) as a parameter (round 29,
/// review 1 of ADR 0062): its menu — automate, modulate, *Learn MIDI controller* — its
/// mapped mark, and the learn mode's outline and click. `true` when the click learnt.
/// Every control that sets a stepped parameter by itself calls it with its response
/// (review 2, #44): a row of value buttons, each.
pub fn stepped_offer(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &str,
    name: &str,
    resp: &Response,
) -> bool {
    use crate::kit::automate as a;
    // Nothing of learn to show and no menu asked for: no offer built (review 3, #51).
    if host.learn_idle(key)
        && !resp.secondary_clicked()
        && !egui::Popup::is_any_open(ui.ctx())
        && !a::probe::noting_learn()
    {
        return false;
    }
    let Some(info) = host.spec().kind.param(key) else {
        return false;
    };
    let v = host.spec().get(key);
    let span = info.max - info.min;
    let pos = if span > 0.0 {
        ((v - info.min) / span).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let mut rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let Some((m, _)) = host.modulation(key, pos, s, &mut rings) else {
        a::probe::note_learn(name, false);
        return false;
    };
    a::probe::note_learn(name, m.learn.is_some());
    let offer = a::Offer {
        automated: host.automated(key),
        typing: false,
        modulation: Some(m),
    };
    if let Some(l) = m.learn {
        if l.mode {
            if a::learn_spot(ui, resp.rect, resp.id, l) {
                host.modulate(key, a::Choice::Learn, resp.rect);
            }
            return l.mode;
        }
        if l.mapped {
            a::learn_mark_over(ui, resp.rect, ui.clip_rect());
        }
    }
    match a::menu(ui, resp, offer) {
        Some(a::Choice::Automate) => host.automate(key),
        Some(a::Choice::Reset) => host.set(key, info.default, None),
        Some(c) => host.modulate(key, c, resp.rect),
        None => {}
    }
    false
}

/// [`segmented`] that is the stepped parameter `key`'s control, the caller setting the
/// value the segment means (a slope of 12, 24 or 48 dB): its menu, its mapped mark, the
/// learn mode (round 29, review 2 of ADR 0062, #44). No choice while the learn mode has it.
#[allow(clippy::too_many_arguments)]
pub fn segmented_key(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &str,
    selected: usize,
    labels: &[Src<'_>],
    name: &str,
    width: f32,
) -> Option<usize> {
    let (resp, r) = crate::kit::widgets::Segmented::new(selected, labels)
        .name(name)
        .size(width, ROW)
        .style(*s)
        .show(ui);
    let learnt = stepped_offer(ui, s, host, key, name, &resp);
    r.filter(|_| !learnt)
}

/// [`field`] that is the stepped parameter `key`'s control (its list built by the
/// caller): its menu, its mapped mark, the learn mode (review 2, #44). Never open while the
/// learn mode has it.
#[allow(clippy::too_many_arguments)]
pub fn field_key(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &str,
    id: egui::Id,
    value: &str,
    name: &str,
    width: f32,
) -> Option<Anchor> {
    let (at, resp) = field_resp(ui, s, id, value, name, width);
    let learnt = stepped_offer(ui, s, host, key, name, &resp);
    if learnt {
        ui.data_mut(|d| d.insert_temp(id.with("open"), false));
        return None;
    }
    at
}

/// A choice among a few words (the kit's `Segmented` at the device's style). Returns
/// the segment clicked, if it is a new one.
pub fn segmented(
    ui: &mut Ui,
    s: &Style,
    selected: usize,
    labels: &[Src<'_>],
    name: &str,
    width: f32,
) -> Option<usize> {
    crate::kit::widgets::Segmented::new(selected, labels)
        .name(name)
        .size(width, ROW)
        .style(*s)
        .show(ui)
        .1
}

/// A segmented control bound to a stepped parameter: segment `i` sets `i` (plus `base`).
pub fn segmented_param(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &'static str,
    labels: &[Src<'_>],
    width: f32,
) -> Option<usize> {
    let spec = host.spec();
    let now = spec.get(key).round().max(0.0) as usize;
    let name = spec.kind.param(key).map_or("", |i| tr(i.label_key));
    let (resp, r) = crate::kit::widgets::Segmented::new(now, labels)
        .name(name)
        .size(width, ROW)
        .style(*s)
        .show(ui);
    let learnt = stepped_offer(ui, s, host, key, name, &resp);
    let r = r.filter(|_| !learnt);
    if let Some(i) = r {
        host.set(key, i as f32, None);
    }
    r
}

/// A menu field: a well with `value` and ▾. Returns where its list opens while it is
/// open (the caller then builds the rows — only then — and shows them with [`list`]).
pub fn field(
    ui: &mut Ui,
    s: &Style,
    id: egui::Id,
    value: &str,
    name: &str,
    width: f32,
) -> Option<Anchor> {
    field_resp(ui, s, id, value, name, width).0
}

/// [`field`], with the field's response.
fn field_resp(
    ui: &mut Ui,
    s: &Style,
    id: egui::Id,
    value: &str,
    name: &str,
    width: f32,
) -> (Option<Anchor>, Response) {
    // The kit's field at the device's style; the open state is kept here, by `id`.
    let open_id = id.with("open");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let (resp, clicks) = crate::kit::widgets::MenuField::new(value)
        .open(open)
        .name(name)
        .size(width, ROW)
        .style(*s)
        .show(ui);
    if clicks.open {
        open = !open;
        ui.data_mut(|d| d.insert_temp(open_id, open));
    }
    let rect = resp.rect;
    (
        open.then(|| Anchor {
            at: rect.left_bottom(),
            field: rect,
        }),
        resp,
    )
}

/// Where an open field's list goes: under the field, a click on which closes it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Anchor {
    pub at: Pos2,
    pub field: Rect,
}

/// The rows of an open [`field`], at `at`, `width` wide. Returns the row chosen; the
/// field closes on a choice or a click elsewhere.
pub fn list(
    ui: &mut Ui,
    s: &Style,
    id: egui::Id,
    at: Anchor,
    width: f32,
    items: &[Item<'_>],
) -> Option<usize> {
    let mut close = false;
    let chosen = popup_list(
        ui.ctx(),
        id.with("list"),
        at.at,
        at.field,
        s,
        items,
        width * s.k,
        &mut close,
    );
    if chosen.is_some() || close {
        ui.data_mut(|d| d.insert_temp(id.with("open"), false));
    }
    chosen
}

/// A verb: a raised key with its word (Learn, Match, To clip). `lit` while its action is
/// under way.
pub fn key(ui: &mut Ui, s: &Style, label: Src<'_>, name: &str, width: f32, lit: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width * s.k, s.d(ROW)), Sense::click());
    let st = State::of(&resp, lit);
    if st.hovered {
        hint::set_text(ui.ctx(), name, "", Caps::NONE);
    }
    if ui.is_rect_visible(rect) {
        with_brush(ui.painter(), |b| paint_key(b, rect, s, st, label));
    }
    resp
}

/// The action key's painter: the raised recipe on the card's title colour — lit, on the
/// section's hue — with the word centred.
pub fn paint_key(b: &mut crate::kit::Brush, rect: Rect, s: &Style, st: State, label: Src<'_>) {
    b.register(rect, "action_key", KEY_TOKENS, &[s.hue_tok]);
    let t = &s.t;
    let r = s.s(t.radius) + s.r(1.0);
    let base = if st.lit {
        crate::kit::color::mix(t.deck_title, s.hue, 0.55)
    } else if t.dark {
        crate::kit::color::shade(t.deck_title, 0.05)
    } else {
        t.deck
    };
    let base = if st.active {
        crate::kit::color::shade(base, -0.03)
    } else if st.hovered {
        crate::kit::color::shade(base, 0.04)
    } else {
        base
    };
    light::raised(
        b,
        rect,
        r,
        base,
        RaisedOpts {
            drop: !st.active,
            k: s.k,
            alpha: st.alpha(),
            depth: t.depth,
            ..Default::default()
        },
    );
    let ink = crate::kit::color::ensure_contrast(if st.lit { t.text } else { t.label }, base, 4.5)
        .gamma_multiply(st.alpha());
    let text = Text {
        src: label,
        role: Role::Control,
        caps: false,
        align: Align2::CENTER_CENTER,
        policy: Policy::Abbreviate,
        color: ink,
    };
    let _ = ktext::paint(b, rect.shrink2(Vec2::new(s.s(5.0), 0.0)), &text, s.tk());
    if st.focused {
        focus_ring(b, rect, r, s);
    }
}

/// The tokens an action key paints with, beside its hue.
pub const KEY_TOKENS: &[crate::kit::Tok] = &[
    crate::kit::Tok::DeckTitle,
    crate::kit::Tok::Deck,
    crate::kit::Tok::Text,
    crate::kit::Tok::Label,
    crate::kit::Tok::Light,
    crate::kit::Tok::Focus,
];

/// A value in a well: numbers right-aligned in mono, names left.
pub fn readout(
    ui: &mut Ui,
    s: &Style,
    value: &str,
    numeric: bool,
    name: &str,
    width: f32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(width * s.k, s.d(ROW)), Sense::hover());
    let st = State::of(&resp, false);
    if st.hovered && !name.is_empty() {
        hint::set_text(ui.ctx(), name, value, Caps::NONE);
    }
    if ui.is_rect_visible(rect) {
        with_brush(ui.painter(), |b| {
            well::readout(b, rect, s, st, value, numeric)
        });
    }
    resp
}

/// A caption in a box: a legend printed once (a group's name above its switches).
pub fn caption(ui: &mut Ui, s: &Style, label: Src<'_>, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width * s.k, s.d(14.0)), Sense::hover());
    if ui.is_rect_visible(rect) {
        with_brush(ui.painter(), |b| {
            let t = Text {
                src: label,
                role: Role::Micro,
                caps: true,
                align: Align2::LEFT_CENTER,
                policy: Policy::Abbreviate,
                color: s.t.text_dim,
            };
            let _ = ktext::paint(b, rect, &t, s.tk());
        });
    }
}

/// A line of words in a box, elided at the end: a message, a legend.
pub fn line(ui: &mut Ui, s: &Style, text: &str, color: egui::Color32, width: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width * s.k, s.d(18.0)), Sense::hover());
    if ui.is_rect_visible(rect) {
        with_brush(ui.painter(), |b| {
            let t = Text::new(text, Role::Body, color)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(ktext::Elide::End));
            let _ = ktext::paint(b, rect, &t, s.tk());
        });
    }
}

/// "12 parameters": a count in a readout, formatted when the count changes.
pub fn count(ui: &mut Ui, s: &Style, key: &'static str, n: usize, width: f32) {
    thread_local! {
        static LAST: RefCell<Option<(&'static str, usize, crate::i18n::Locale, String)>> =
            const { RefCell::new(None) };
    }
    let lang = crate::i18n::locale();
    LAST.with(|l| {
        let mut l = l.borrow_mut();
        if l.as_ref()
            .is_none_or(|(k, m, g, _)| *k != key || *m != n || *g != lang)
        {
            *l = Some((key, n, lang, crate::i18n::tf(key, &[("n", &n)])));
        }
        if let Some((_, _, _, text)) = l.as_ref() {
            readout(ui, s, text, false, "", width);
        }
    });
}

/// A text box in a well (a filter, a name): the kit's well around egui's text editing,
/// with a hint when empty.
pub fn text_field(
    ui: &mut Ui,
    s: &Style,
    id: egui::Id,
    text: &mut String,
    hint: &str,
    width: f32,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width * s.k, s.d(ROW)), Sense::hover());
    // The kit's field (round 26), standing: a click opens a session.
    let r = crate::kit::field::Field::new(id)
        .hint(hint)
        .scale(s.k)
        .show(ui, rect, text)
        .resp;
    if r.hovered() {
        hint::set_text(
            ui.ctx(),
            hint,
            "",
            Caps {
                typed: true,
                ..Caps::NONE
            },
        );
    }
    r
}

// ---- cards ------------------------------------------------------------------------------

/// A card's title bar and frame in `rect`, then `body` inside it (8 pt in, the cells
/// centred on a row). `on`: the section's LED when it can be switched (`None`: a hue tab).
/// Returns the card's parts and whether the LED was clicked.
pub fn card<R>(
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    title: Src<'_>,
    on: Option<bool>,
    body: impl FnOnce(&mut Ui, &CardRects) -> R,
) -> (R, bool) {
    card_with(ui, s, rect, title, on, None, body)
}

/// [`card`], with its lamp's response: a band's On is its parameter's control (round 29,
/// review 2 of ADR 0062, #44; review 3, #51: handed back, never kept in egui's memory).
pub fn card_lamp<R>(
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    title: Src<'_>,
    on: Option<bool>,
    body: impl FnOnce(&mut Ui, &CardRects) -> R,
) -> (R, bool, Option<Response>) {
    card_full(ui, s, rect, title, on, None, body)
}

/// The lamp of a [`card_lamp`] as the stepped parameter `key`'s control: its menu, its
/// mapped mark, the learn mode. `true` while the learn mode has it: the caller then does
/// not flip it.
pub fn card_lamp_offer(
    ui: &mut Ui,
    s: &Style,
    host: &mut dyn FxHost,
    key: &str,
    lamp: Option<&Response>,
) -> bool {
    let Some(r) = lamp else {
        return false;
    };
    let name = host.spec().kind.param(key).map_or("", |i| tr(i.label_key));
    stepped_offer(ui, s, host, key, name, r)
}

/// [`card`] with a display under the title bar, `display` points tall at 100 %.
pub fn card_with<R>(
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    title: Src<'_>,
    on: Option<bool>,
    display: Option<f32>,
    body: impl FnOnce(&mut Ui, &CardRects) -> R,
) -> (R, bool) {
    let (r, clicked, _) = card_full(ui, s, rect, title, on, display, body);
    (r, clicked)
}

#[allow(clippy::too_many_arguments)]
fn card_full<R>(
    ui: &mut Ui,
    s: &Style,
    rect: Rect,
    title: Src<'_>,
    on: Option<bool>,
    display: Option<f32>,
    body: impl FnOnce(&mut Ui, &CardRects) -> R,
) -> (R, bool, Option<Response>) {
    let look = CardLook {
        title,
        switchable: on.is_some(),
        selector: None,
        menu: false,
        display,
    };
    let st = State {
        lit: on.unwrap_or(true),
        ..State::REST
    };
    let rects = with_brush(&ui.painter_at(rect.expand(4.0)), |b| {
        band::card(b, rect, s, st, &look)
    });
    let mut clicked = false;
    let mut lamp = None;
    if let Some(led) = rects.led {
        let r = ui.interact(
            led.expand(4.0 * s.k),
            ui.id()
                .with(("card-led", rect.min.x as i32, rect.min.y as i32)),
            Sense::click(),
        );
        clicked = r.clicked();
        if r.hovered() {
            hint::set_text(ui.ctx(), tr("fx.frame.section_on"), "", Caps::NONE);
        }
        lamp = Some(r);
    }
    let inner = rects.body;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(inner)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    // Set only when it differs: a style clone is a dozen allocations a frame.
    let gap = Vec2::new(s.d(4.0), s.d(4.0));
    if child.spacing().item_spacing != gap {
        child.spacing_mut().item_spacing = gap;
    }
    if on == Some(false) {
        child.disable();
    }
    let r = body(&mut child, &rects);
    (r, clicked, lamp)
}

/// A column of switches inside a card's body: `f` adds them top to bottom.
pub fn column<R>(ui: &mut Ui, s: &Style, width: f32, f: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_ui_with_layout(
        Vec2::new(width * s.k, ui.available_height()),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            let gap = Vec2::new(s.d(4.0), s.d(5.0));
            if ui.spacing().item_spacing != gap {
                ui.spacing_mut().item_spacing = gap;
            }
            f(ui)
        },
    )
    .inner
}

/// The width a row of cells of `sizes` takes at 100 %, with the 4-pt gaps.
pub fn cells_width(sizes: &[KnobSize]) -> f32 {
    let w: f32 = sizes.iter().map(|z| cell::size(*z).x).sum();
    w + 4.0 * sizes.len().saturating_sub(1) as f32
}

/// The height of a card holding one row of cells of `size`, at 100 %.
pub fn card_height(size: KnobSize) -> f32 {
    band::TITLE + 8.0 + cell::size(size).y + 8.0
}

/// Where the pointer is in a rect, or `None`.
pub fn hover_in(ui: &Ui, rect: Rect) -> Option<Pos2> {
    ui.input(|i| i.pointer.hover_pos())
        .filter(|p| rect.contains(*p))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Final review, fields 2: a typed value opened and left untouched — Enter, or a click
    /// elsewhere — sets nothing: no rounding to the readout, no host undo step.
    #[test]
    fn an_untouched_typed_value_sets_nothing() {
        use egui::{Event, Modifiers, PointerButton};
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        let s = Style::current();
        let mut frame = 0u64;
        let mut run = |events: Vec<Event>| {
            frame += 1;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                time: Some(frame as f64 / 60.0),
                events,
                ..Default::default()
            };
            let mut got = None;
            ctx.run_ui(input, |ui| {
                let o = Cell::new(1_234.567, 20.0..=20_000.0, 1_000.0)
                    .unit("Hz")
                    .show(ui, &s);
                got = Some((o.resp.rect, o.changed, o.done));
            })
            .textures_delta
            .clear();
            got.unwrap()
        };
        let click = |at, button, pressed| Event::PointerButton {
            pos: at,
            button,
            pressed,
            modifiers: Modifiers::NONE,
        };
        for close in ["enter", "away"] {
            let at = run(Vec::new()).0.center();
            run(vec![Event::PointerMoved(at)]);
            for pressed in [true, false] {
                run(vec![click(at, PointerButton::Secondary, pressed)]);
            }
            run(vec![Event::PointerGone]);
            run(Vec::new());
            assert!(crate::kit::field::active(&ctx), "{close}: the field opened");
            let mut seen = Vec::new();
            if close == "enter" {
                seen.push(run(vec![Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }]));
            } else {
                let away = Pos2::new(490.0, 390.0);
                seen.push(run(vec![Event::PointerMoved(away)]));
                for pressed in [true, false] {
                    seen.push(run(vec![click(away, PointerButton::Primary, pressed)]));
                }
            }
            seen.push(run(Vec::new()));
            assert!(
                !crate::kit::field::active(&ctx),
                "{close}: the field closed"
            );
            for (_, changed, done) in seen {
                assert_eq!(changed, None, "{close}: the untouched value was set");
                assert!(!done, "{close}: an undo step was ended");
            }
        }
    }

    /// Third review (looking at the pictures): a typed value opens *over* its cell — the
    /// cell's own face, painted after the field, hid it — with its words at the editor's
    /// scale, and Enter sets what was typed.
    #[test]
    fn a_typed_value_opens_over_its_cell_at_the_editors_scale() {
        use egui::{Event, Modifiers, PointerButton};
        let mut rows = Vec::new();
        for k in [1.0f32, 2.0] {
            let ctx = egui::Context::default();
            crate::design::install_fonts(&ctx);
            crate::design::install_style(&ctx);
            let s = Style::current().scaled(k);
            let mut v = 125.0f32;
            let mut frame = 0u64;
            let mut run = |v: &mut f32, events: Vec<Event>| {
                let mut cell = Rect::NOTHING;
                frame += 1;
                let input = egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(500.0, 400.0))),
                    time: Some(frame as f64 / 60.0),
                    events,
                    ..Default::default()
                };
                let mut out = ctx.run_ui(input, |ui| {
                    let o = Cell::new(*v, 0.0..=500.0, 0.0).unit("ms").show(ui, &s);
                    cell = o.resp.rect;
                    if let Some(x) = o.changed {
                        *v = x;
                    }
                });
                out.textures_delta.clear();
                (out.shapes, cell)
            };
            let cell = run(&mut v, Vec::new()).1;
            let at = cell.center();
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
            run(&mut v, vec![Event::PointerGone]);
            run(&mut v, Vec::new());
            assert!(crate::kit::field::active(&ctx), "k {k}: the field opened");
            run(&mut v, vec![Event::Text("80".into())]);
            let shapes = run(&mut v, Vec::new()).0;
            // The last words painted over the cell are the field's.
            let last = shapes
                .iter()
                .rev()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) => Some(t),
                    _ => None,
                })
                .filter(|t| !t.galley.text().is_empty())
                .find(|t| cell.intersects(t.galley.rect.translate(t.pos.to_vec2())))
                .expect("words on the cell");
            assert_eq!(
                last.galley.text(),
                "80",
                "k {k}: the field is on top of its cell"
            );
            rows.push(last.galley.rect.height());
            run(
                &mut v,
                vec![Event::Key {
                    key: Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }],
            );
            run(&mut v, Vec::new());
            assert!((v - 80.0).abs() < 1e-4, "k {k}: Enter set {v}");
        }
        assert!(
            (rows[1] / rows[0] - 2.0).abs() < 0.15,
            "the words at the editor's scale: {rows:?}"
        );
    }

    #[test]
    fn the_readout_is_the_cores_to_the_letter() {
        let mut buf = String::new();
        for unit in [
            "Hz", "ms", "s", "dB", "st", "ct", "%", "\u{b0}", "", ":1", "LUFS",
        ] {
            for v in [
                -48.0f32, -3.5, 0.0, 0.004, 0.25, 0.71, 1.0, 12.0, 99.9, 150.0, 999.0, 1_500.0,
                12_000.0,
            ] {
                buf.clear();
                write_readout(v, unit, &mut buf);
                assert_eq!(
                    buf,
                    cantus_core::fx::readout::readout(v, unit),
                    "{v} {unit}"
                );
            }
        }
    }
}
