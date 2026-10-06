//! Thin widgets over the painters: each allocates its rect, senses input, turns it into
//! a [`State`], writes the [hint](super::hint) while hovered or dragged, and calls its
//! painter through the thread's brush. Behaviour follows the existing house controls
//! (`widgets::knob`): 200 px of drag for the whole travel, Shift for fine control,
//! double-click to reset, arrows when focused, the wheel only when the caller asks.
//!
//! Values are formatted into one reused buffer, so a control that is not moving costs
//! no allocation.

use super::cell::{self, CellLook, Control};
use super::fader::{self, FaderLook};
use super::hint::{self, Caps};
use super::knob::{self, KnobLook, KnobSize};
use super::menu::{self, Item};
use super::meter::{self, Levels};
use super::text::Src;
use super::tile::{self, TileLook};
use super::{cap, chrome, led, segmented, well, with_brush, State, Style};
use crate::widgets::knob::{pos_of, value_at, Taper};
use egui::{EventFilter, Key, Rect, Response, Sense, Ui, Vec2};
use std::cell::RefCell;
use std::fmt::Write as _;

const TRAVEL_PX: f32 = 200.0;
const FINE: f32 = 0.2;

thread_local! {
    static FMT: RefCell<String> = RefCell::new(String::with_capacity(32));
}

/// Run `f` with the value formatted into the shared buffer.
fn formatted<R>(fmt: &dyn Fn(f32, &mut String), v: f32, f: impl FnOnce(&str) -> R) -> R {
    FMT.with(|b| match b.try_borrow_mut() {
        Ok(mut buf) => {
            buf.clear();
            fmt(v, &mut buf);
            f(&buf)
        }
        Err(_) => {
            let mut s = String::new();
            fmt(v, &mut s);
            f(&s)
        }
    })
}

/// The default readout: two decimals.
fn two_decimals(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.2}");
}

/// A continuous parameter: a knob, or a cell around one. `F` writes the readout.
pub struct Param<'a, F = fn(f32, &mut String)> {
    value: &'a mut f32,
    min: f32,
    max: f32,
    default: f32,
    taper: Taper,
    size: KnobSize,
    name: &'a str,
    label: Option<Src<'a>>,
    format: F,
    wheel: bool,
    style: Option<Style>,
    rings: &'a [(f32, egui::Color32)],
    /// The parameter's own menu on a right-click (ADR 0027), and whether a clip drives it.
    offer: Option<super::automate::Offer>,
}

impl<'a> Param<'a> {
    pub fn new(value: &'a mut f32, range: std::ops::RangeInclusive<f32>, default: f32) -> Self {
        let (min, max) = (*range.start(), *range.end());
        Param {
            value,
            min,
            max,
            default: default.clamp(min, max),
            taper: Taper::Linear,
            size: KnobSize::M,
            name: "",
            label: None,
            format: two_decimals,
            wheel: false,
            style: None,
            rings: &[],
            offer: None,
        }
    }

    /// Write the value's readout into the (reused) buffer.
    pub fn format<G: Fn(f32, &mut String)>(self, f: G) -> Param<'a, G> {
        Param {
            value: self.value,
            min: self.min,
            max: self.max,
            default: self.default,
            taper: self.taper,
            size: self.size,
            name: self.name,
            label: self.label,
            format: f,
            wheel: self.wheel,
            style: self.style,
            rings: self.rings,
            offer: self.offer,
        }
    }
}

impl<'a, F: Fn(f32, &mut String)> Param<'a, F> {
    pub fn log(mut self, on: bool) -> Self {
        self.taper = if on { Taper::Log } else { Taper::Linear };
        self
    }
    pub fn size(mut self, s: KnobSize) -> Self {
        self.size = s;
        self
    }
    /// The full name, for the hint bar and the help line.
    pub fn name(mut self, n: &'a str) -> Self {
        self.name = n;
        self
    }
    /// Show as a cell with this label (an i18n key, so it can abbreviate).
    pub fn cell(mut self, label: Src<'a>) -> Self {
        self.label = Some(label);
        self
    }
    pub fn wheel(mut self, on: bool) -> Self {
        self.wheel = on;
        self
    }
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }
    pub fn rings(mut self, r: &'a [(f32, egui::Color32)]) -> Self {
        self.rings = r;
        self
    }
    /// Offer the parameter's menu (automate, reset) on a right-click; `automated` marks
    /// the control as driven by a clip (ADR 0027).
    pub fn automation(mut self, automated: bool) -> Self {
        self.offer = Some(super::automate::Offer {
            automated,
            typing: false,
        });
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        self.show_full(ui).0
    }

    /// [`Param::show`], with what was chosen in the parameter's menu. A reset is done
    /// here (the value goes to its default and the response says it changed).
    pub fn show_full(self, ui: &mut Ui) -> (Response, Option<super::automate::Choice>) {
        let s = self.style.unwrap_or_else(Style::current);
        let size = if self.label.is_some() {
            cell::size(self.size) * s.k
        } else {
            Vec2::splat((self.size.diameter() + 6.0) * s.k)
        };
        let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let (min, max, taper) = (self.min, self.max, self.taper);
        let start = self.value.clamp(min, max);
        let mut p = pos_of(start, min, max, taper);
        let fine = if ui.input(|i| i.modifiers.shift) {
            FINE
        } else {
            1.0
        };
        if resp.dragged() {
            let d = resp.drag_delta();
            p += (d.x - d.y) * fine / TRAVEL_PX;
        }
        if self.wheel && resp.hovered() {
            let dy = ui.input(|i| i.smooth_scroll_delta.y);
            p += dy * fine / (TRAVEL_PX * 2.0);
        }
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
            let step = if fine < 1.0 { 0.001 } else { 0.01 };
            ui.input(|i| {
                if i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowRight) {
                    p += step;
                }
                if i.key_pressed(Key::ArrowDown) || i.key_pressed(Key::ArrowLeft) {
                    p -= step;
                }
            });
        }
        // Only a gesture moves the value: re-deriving it through a log taper when nothing
        // moved would store its round-trip error (2400 Hz → 2399.9995) on every frame.
        let p0 = pos_of(start, min, max, taper);
        let mut v = if (p - p0).abs() > 1e-6 {
            value_at(p.clamp(0.0, 1.0), min, max, taper)
        } else {
            start
        };
        if resp.double_clicked() {
            v = self.default;
        }
        // A value that only went through the taper and back (log ranges do not round-trip
        // exactly) has not changed: only a move of more than a millionth of the range does,
        // or every idle frame would write a command.
        if (v - start).abs() > (max - min).abs() * 1e-6 {
            *self.value = v;
            resp.mark_changed();
        }
        let st = State::of(&resp, false);
        let look = KnobLook {
            size: self.size,
            value: pos_of(*self.value, min, max, taper),
            default: pos_of(self.default, min, max, taper),
            rings: self.rings,
        };
        let fmt = &self.format;
        if st.hovered {
            hint::set(ui.ctx(), self.name, Caps::PARAM, resp.dragged(), |b| {
                fmt(*self.value, b);
                Ok(())
            });
        }
        let painter = ui.painter_at(rect.expand(8.0 * s.k));
        formatted(fmt, *self.value, |text| {
            with_brush(&painter, |b| match self.label {
                Some(label) => {
                    let l = CellLook {
                        control: Control::Knob(look),
                        value: text,
                        label,
                    };
                    cell::paint(b, rect, &s, st, &l);
                }
                None => knob::paint(b, rect.center(), &s, st, &look),
            });
            if self.offer.is_some_and(|o| o.automated) {
                with_brush(&painter, |b| super::automate::mark(b, rect, &s));
            }
        });
        let mut choice = None;
        if let Some(offer) = self.offer {
            choice = super::automate::menu(ui, &resp, offer);
            if choice == Some(super::automate::Choice::Reset) {
                *self.value = self.default;
                resp.mark_changed();
            }
        }
        (resp, choice)
    }
}

fn allocate(ui: &mut Ui, size: Vec2, sense: Sense) -> (Rect, Response) {
    ui.allocate_exact_size(size, sense)
}

/// A vertical fader of `height` points at 100 %; `selected` lights it. `pos` is 0…1.
pub fn fader(
    ui: &mut Ui,
    pos: &mut f32,
    height: f32,
    selected: bool,
    name: &str,
    ticks: &[f32],
) -> Response {
    let s = Style::current();
    let (rect, mut resp) = allocate(ui, Vec2::new(24.0, height) * s.k, Sense::click_and_drag());
    let before = *pos;
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            FINE
        } else {
            1.0
        };
        let travel = (rect.height() - fader::CAP.y * s.k).max(1.0);
        *pos = (*pos - resp.drag_delta().y * fine / travel).clamp(0.0, 1.0);
    } else if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            *pos = fader::pos_at(rect, &s, p.y);
        }
    }
    if (*pos - before).abs() > f32::EPSILON {
        resp.mark_changed();
    }
    let st = State::of(&resp, selected);
    if st.hovered {
        hint::set(ui.ctx(), name, Caps::PARAM, resp.dragged(), |b| {
            write!(b, "{:.0} %", *pos * 100.0)
        });
    }
    let look = FaderLook {
        pos: *pos,
        ticks,
        unity: None,
    };
    with_brush(ui.painter(), |b| fader::paint(b, rect, &s, st, &look));
    resp
}

/// An on/off as an LED button. Returns the response; `on` flips on click.
pub fn led_button(ui: &mut Ui, on: &mut bool, label: Src<'_>, name: &str, width: f32) -> Response {
    let s = Style::current();
    let (rect, mut resp) = allocate(ui, Vec2::new(width * s.k, s.d(20.0)), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let st = State::of(&resp, *on);
    if st.hovered {
        hint::set_text(ui.ctx(), name, "", Caps::PARAM);
    }
    with_brush(ui.painter(), |b| led::button(b, rect, &s, st, label));
    resp
}

/// A choice among a few words. `selected` changes on click.
pub fn segmented(
    ui: &mut Ui,
    selected: &mut usize,
    labels: &[Src<'_>],
    name: &str,
    width: f32,
) -> Response {
    let s = Style::current();
    let (rect, mut resp) = allocate(ui, Vec2::new(width * s.k, s.d(24.0)), Sense::click());
    let hovered = resp
        .hover_pos()
        .and_then(|p| segmented::hit(rect, labels.len(), p.x));
    if resp.clicked() {
        if let Some(i) = hovered {
            if i != *selected {
                *selected = i;
                resp.mark_changed();
            }
        }
    }
    let st = State::of(&resp, false);
    if st.hovered {
        hint::set_text(ui.ctx(), name, "", Caps::NONE);
    }
    with_brush(ui.painter(), |b| {
        segmented::paint(b, rect, &s, st, labels, *selected, hovered)
    });
    resp
}

/// What a menu field reports.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldClicks {
    /// The field itself: open its list.
    pub open: bool,
    pub prev: bool,
    pub next: bool,
}

/// A menu field showing `value`, with ‹ › when `steppers`. `open` lights its outline.
pub fn menu_field(
    ui: &mut Ui,
    value: &str,
    steppers: bool,
    open: bool,
    name: &str,
    width: f32,
) -> (Response, FieldClicks) {
    let s = Style::current();
    let (rect, resp) = allocate(ui, Vec2::new(width * s.k, s.d(24.0)), Sense::click());
    let r = well::field_rects(rect, &s, steppers);
    let mut c = FieldClicks::default();
    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            c.prev = r.prev.is_some_and(|x| x.contains(p));
            c.next = r.next.is_some_and(|x| x.contains(p));
            c.open = !c.prev && !c.next;
        }
    }
    let st = State::of(&resp, open);
    if st.hovered {
        hint::set_text(
            ui.ctx(),
            name,
            value,
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    with_brush(ui.painter(), |b| {
        well::menu_field(b, rect, &s, st, value, steppers)
    });
    (resp, c)
}

/// A 28 × 28 toolbar key; `on` flips on click and lights it.
pub fn toolbar_key(ui: &mut Ui, on: &mut bool, face: cap::KeyFace<'_>, name: &str) -> Response {
    let s = Style::current();
    let (rect, mut resp) = allocate(ui, Vec2::splat(28.0 * s.k), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let st = State::of(&resp, *on);
    if st.hovered {
        hint::set_text(ui.ctx(), name, "", Caps::NONE);
    }
    with_brush(ui.painter(), |b| cap::toolbar_key(b, rect, &s, st, face));
    resp
}

/// What a tile reports.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct TileClicks {
    pub menu: bool,
    pub name: bool,
    pub led: bool,
    /// The wheel over it: reorder (−1 up, +1 down), as in FL.
    pub reorder: i8,
}

/// A slot row of `width`.
pub fn tile(
    ui: &mut Ui,
    look: &TileLook<'_>,
    selected: bool,
    name: &str,
    width: f32,
) -> (Response, TileClicks) {
    let s = Style::current();
    let (rect, resp) = allocate(ui, Vec2::new(width, s.d(tile::HEIGHT)), Sense::click());
    let r = tile::rects(rect, &s);
    let mut c = TileClicks::default();
    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            c.menu = r.menu.contains(p);
            c.led = r.led.expand(3.0).contains(p);
            c.name = !c.menu && !c.led;
        }
    }
    if resp.hovered() {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        c.reorder = if dy > 0.0 {
            -1
        } else if dy < 0.0 {
            1
        } else {
            0
        };
        hint::set_text(
            ui.ctx(),
            name,
            "",
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    let st = State::of(&resp, selected);
    with_brush(ui.painter(), |b| tile::paint(b, rect, &s, st, look));
    (resp, c)
}

/// A workspace tab.
pub fn tab(ui: &mut Ui, active: bool, label: Src<'_>, width: f32) -> Response {
    let s = Style::current();
    let (rect, resp) = allocate(ui, Vec2::new(width, s.d(chrome::TAB)), Sense::click());
    let st = State::of(&resp, active);
    with_brush(ui.painter(), |b| chrome::tab(b, rect, &s, st, label));
    resp
}

/// A splitter across `rect` (the caller places it). Drag moves `size` by the pointer's
/// motion along its axis; right-click resets it to `default`; double-click toggles
/// between the last two sizes in `memory`.
pub fn splitter(
    ui: &mut Ui,
    rect: Rect,
    id: egui::Id,
    size: &mut f32,
    default: f32,
    range: std::ops::RangeInclusive<f32>,
) -> Response {
    let s = Style::current();
    let vertical = rect.height() > rect.width();
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    if resp.dragged() {
        let d = resp.drag_delta();
        *size = (*size + if vertical { d.x } else { d.y }).clamp(*range.start(), *range.end());
    }
    if resp.secondary_clicked() {
        *size = default;
    }
    if resp.double_clicked() {
        let last = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(default);
        ui.data_mut(|d| d.insert_temp(id, *size));
        *size = last;
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(if vertical {
            egui::CursorIcon::ResizeHorizontal
        } else {
            egui::CursorIcon::ResizeVertical
        });
    }
    let st = State::of(&resp, false);
    with_brush(ui.painter(), |b| chrome::splitter(b, rect, &s, st));
    resp
}

/// A scrollbar in `rect` for a view showing `len` of the content from `start` (both
/// fractions). Dragging the thumb moves `start`.
pub fn scrollbar(ui: &mut Ui, rect: Rect, id: egui::Id, start: &mut f32, len: f32) -> Response {
    let s = Style::current();
    let vertical = rect.height() > rect.width();
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    if resp.dragged() {
        let full = if vertical {
            rect.height()
        } else {
            rect.width()
        };
        let d = resp.drag_delta();
        let moved = if vertical { d.y } else { d.x } / full.max(1.0);
        *start = (*start + moved).clamp(0.0, (1.0 - len).max(0.0));
    }
    let st = State::of(&resp, false);
    with_brush(ui.painter(), |b| {
        chrome::scrollbar(b, rect, &s, st, *start, len)
    });
    resp
}

/// A meter of `size`; hovering it puts the level under the pointer in the hint bar.
pub fn meter(ui: &mut Ui, size: Vec2, levels: &Levels, name: &str) -> Response {
    let s = Style::current();
    let (rect, resp) = allocate(ui, size * s.k, Sense::hover());
    if let Some(p) = resp.hover_pos() {
        let pos = (rect.bottom() - p.y) / rect.height().max(1.0);
        hint::set(ui.ctx(), name, Caps::NONE, false, |b| {
            write!(b, "{:.1} dB", meter::db_at(pos))
        });
    }
    with_brush(ui.painter(), |b| meter::paint(b, rect, &s, levels));
    resp
}

/// What a menu reports.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuClicks {
    /// An item was chosen: close the menu.
    pub clicked: Option<usize>,
    /// A checkable item was right-clicked: toggle it and keep the menu open (FL).
    pub toggled: Option<usize>,
}

/// A menu's rows, `width` wide.
pub fn menu_list(ui: &mut Ui, items: &[Item<'_>], width: f32) -> (Response, MenuClicks) {
    let s = Style::current();
    let h = menu::height(items, s.d(1.0));
    let (rect, resp) = allocate(ui, Vec2::new(width, h), Sense::click());
    let hovered = resp.hover_pos().and_then(|p| {
        (0..items.len())
            .find(|&i| items[i].is_action() && menu::row(rect, items, s.d(1.0), i).contains(p))
    });
    let mut c = MenuClicks::default();
    if resp.clicked() {
        c.clicked = hovered;
    }
    if resp.secondary_clicked() {
        c.toggled = hovered.filter(|&i| {
            matches!(
                items[i],
                Item::Action {
                    check: menu::Check::On | menu::Check::Off,
                    ..
                }
            )
        });
    }
    with_brush(ui.painter(), |b| menu::paint(b, rect, &s, items, hovered));
    (resp, c)
}

/// A value in a well (a readout, a field's face).
pub fn readout(ui: &mut Ui, value: &str, numeric: bool, name: &str, width: f32) -> Response {
    let s = Style::current();
    let (rect, resp) = allocate(ui, Vec2::new(width * s.k, s.d(22.0)), Sense::click());
    let st = State::of(&resp, false);
    if st.hovered {
        hint::set_text(
            ui.ctx(),
            name,
            value,
            Caps {
                typed: true,
                ..Caps::NONE
            },
        );
    }
    with_brush(ui.painter(), |b| {
        well::readout(b, rect, &s, st, value, numeric)
    });
    resp
}

// ---- builders with a style of their own ----------------------------------------------
//
// The functions above paint in the installed style at 100 %. Inside a device window the
// controls take the window's scale `k` and their card's function hue: these builders
// carry a `.style(s)`, as `Param` does.

/// An on/off as an LED button, with a style: `LedButton::new(on, label).style(s).show(ui)`,
/// or `.show_at(ui, rect)` in a rect a card's layout gave it.
pub struct LedButton<'a> {
    on: bool,
    label: Src<'a>,
    name: &'a str,
    width: f32,
    style: Option<Style>,
    id: Option<egui::Id>,
    caps: Caps,
    momentary: bool,
    enabled: bool,
}

impl<'a> LedButton<'a> {
    pub fn new(on: bool, label: Src<'a>) -> Self {
        LedButton {
            on,
            label,
            name: "",
            width: 88.0,
            style: None,
            id: None,
            caps: Caps::PARAM,
            momentary: false,
            enabled: true,
        }
    }
    /// The full name, for the hint bar and the help line.
    pub fn name(mut self, n: &'a str) -> Self {
        self.name = n;
        self
    }
    /// Its width at 100 %.
    pub fn width(mut self, w: f32) -> Self {
        self.width = w;
        self
    }
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }
    /// Its identity for [`show_at`](Self::show_at) (the same id every frame).
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }
    /// What the hint bar says it can do (a parameter's by default).
    pub fn caps(mut self, caps: Caps) -> Self {
        self.caps = caps;
        self
    }
    /// An action, not a state: the lamp lights only while it is held.
    pub fn momentary(mut self, on: bool) -> Self {
        self.momentary = on;
        self
    }
    /// Off: dimmed, and a click does nothing.
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    fn sense(&self) -> Sense {
        if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        }
    }
    /// Clicked: flip it (or do it, when momentary).
    pub fn show(self, ui: &mut Ui) -> Response {
        let s = self.style.unwrap_or_else(Style::current);
        let (rect, resp) = allocate(ui, Vec2::new(self.width * s.k, s.d(20.0)), self.sense());
        self.paint(ui, rect, resp, &s)
    }
    /// The same button filling `rect`.
    pub fn show_at(self, ui: &Ui, rect: Rect) -> Response {
        let s = self.style.unwrap_or_else(Style::current);
        let id = self.id.unwrap_or_else(|| {
            ui.id()
                .with(("led", self.name, rect.min.x as i32, rect.min.y as i32))
        });
        let resp = ui.interact(rect, id, self.sense());
        self.paint(ui, rect, resp, &s)
    }
    fn paint(self, ui: &Ui, rect: Rect, resp: Response, s: &Style) -> Response {
        let lit = if self.momentary {
            resp.is_pointer_button_down_on()
        } else {
            self.on
        };
        let mut st = State::of(&resp, lit);
        st.disabled = !self.enabled;
        if st.hovered {
            hint::set_text(ui.ctx(), self.name, "", self.caps);
        }
        if ui.is_rect_visible(rect) {
            with_brush(ui.painter(), |b| led::button(b, rect, s, st, self.label));
        }
        resp
    }
}

/// A choice among a few words, with a style: `Segmented::new(sel, &labels).style(s).show(ui)`,
/// or `.show_at(ui, rect)`.
pub struct Segmented<'a> {
    selected: usize,
    labels: &'a [Src<'a>],
    name: &'a str,
    width: f32,
    height: f32,
    style: Option<Style>,
    id: Option<egui::Id>,
    caps: Caps,
    value: &'a str,
    enabled: bool,
}

impl<'a> Segmented<'a> {
    pub fn new(selected: usize, labels: &'a [Src<'a>]) -> Self {
        Segmented {
            selected,
            labels,
            name: "",
            width: 160.0,
            height: 24.0,
            style: None,
            id: None,
            caps: Caps::NONE,
            value: "",
            enabled: true,
        }
    }
    pub fn name(mut self, n: &'a str) -> Self {
        self.name = n;
        self
    }
    /// Width and height at 100 %.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }
    /// Its identity for [`show_at`](Self::show_at).
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }
    /// What the hint bar says it can do (nothing by default; a parameter's when bound).
    pub fn caps(mut self, caps: Caps) -> Self {
        self.caps = caps;
        self
    }
    /// The value the hint bar shows beside the name.
    pub fn value(mut self, v: &'a str) -> Self {
        self.value = v;
        self
    }
    /// Off: dimmed, and a click chooses nothing.
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    fn sense(&self) -> Sense {
        if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        }
    }
    /// The segment clicked, when it is a new one.
    pub fn show(self, ui: &mut Ui) -> (Response, Option<usize>) {
        let s = self.style.unwrap_or_else(Style::current);
        let (rect, resp) = allocate(
            ui,
            Vec2::new(self.width * s.k, s.d(self.height)),
            self.sense(),
        );
        self.paint(ui, rect, resp, &s)
    }
    /// The same control filling `rect`.
    pub fn show_at(self, ui: &Ui, rect: Rect) -> (Response, Option<usize>) {
        let s = self.style.unwrap_or_else(Style::current);
        let id = self.id.unwrap_or_else(|| {
            ui.id()
                .with(("seg", self.name, rect.min.x as i32, rect.min.y as i32))
        });
        let resp = ui.interact(rect, id, self.sense());
        self.paint(ui, rect, resp, &s)
    }
    fn paint(self, ui: &Ui, rect: Rect, resp: Response, s: &Style) -> (Response, Option<usize>) {
        let n = self.labels.len();
        let hovered = if self.enabled {
            resp.hover_pos().and_then(|p| segmented::hit(rect, n, p.x))
        } else {
            None
        };
        let chosen = if self.enabled && resp.clicked() {
            hovered.filter(|i| *i != self.selected)
        } else {
            None
        };
        let mut st = State::of(&resp, false);
        st.disabled = !self.enabled;
        if st.hovered {
            hint::set_text(ui.ctx(), self.name, self.value, self.caps);
        }
        if ui.is_rect_visible(rect) {
            with_brush(ui.painter(), |b| {
                segmented::paint(b, rect, s, st, self.labels, self.selected, hovered)
            });
        }
        (resp, chosen)
    }
}

/// A menu field, with a style: `MenuField::new(value).open(o).style(s).show(ui)`, or
/// `.show_at(ui, rect)`.
pub struct MenuField<'a> {
    value: &'a str,
    steppers: bool,
    open: bool,
    name: &'a str,
    width: f32,
    height: f32,
    style: Option<Style>,
    id: Option<egui::Id>,
    caps: Caps,
    enabled: bool,
}

impl<'a> MenuField<'a> {
    pub fn new(value: &'a str) -> Self {
        MenuField {
            value,
            steppers: false,
            open: false,
            name: "",
            width: 160.0,
            height: 24.0,
            style: None,
            id: None,
            caps: Caps {
                menu: true,
                ..Caps::NONE
            },
            enabled: true,
        }
    }
    pub fn steppers(mut self, on: bool) -> Self {
        self.steppers = on;
        self
    }
    /// Its list is open: the outline lights.
    pub fn open(mut self, on: bool) -> Self {
        self.open = on;
        self
    }
    pub fn name(mut self, n: &'a str) -> Self {
        self.name = n;
        self
    }
    /// Width and height at 100 %.
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
    pub fn style(mut self, s: Style) -> Self {
        self.style = Some(s);
        self
    }
    /// Its identity for [`show_at`](Self::show_at).
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }
    /// What the hint bar says it can do (a menu by default).
    pub fn caps(mut self, caps: Caps) -> Self {
        self.caps = caps;
        self
    }
    /// Off (does not apply here): dimmed, not hidden, and not clickable.
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
    fn sense(&self) -> Sense {
        if self.enabled {
            Sense::click()
        } else {
            Sense::hover()
        }
    }
    pub fn show(self, ui: &mut Ui) -> (Response, FieldClicks) {
        let s = self.style.unwrap_or_else(Style::current);
        let (rect, resp) = allocate(
            ui,
            Vec2::new(self.width * s.k, s.d(self.height)),
            self.sense(),
        );
        self.paint(ui, rect, resp, &s)
    }
    /// The same field filling `rect`.
    pub fn show_at(self, ui: &Ui, rect: Rect) -> (Response, FieldClicks) {
        let s = self.style.unwrap_or_else(Style::current);
        let id = self.id.unwrap_or_else(|| {
            ui.id()
                .with(("field", self.name, rect.min.x as i32, rect.min.y as i32))
        });
        let resp = ui.interact(rect, id, self.sense());
        self.paint(ui, rect, resp, &s)
    }
    fn paint(self, ui: &Ui, rect: Rect, resp: Response, s: &Style) -> (Response, FieldClicks) {
        let r = well::field_rects(rect, s, self.steppers);
        let mut c = FieldClicks::default();
        if self.enabled && resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                c.prev = r.prev.is_some_and(|x| x.contains(p));
                c.next = r.next.is_some_and(|x| x.contains(p));
                c.open = !c.prev && !c.next;
            }
        }
        let st = if self.enabled {
            State::of(&resp, self.open)
        } else {
            State::DISABLED
        };
        if self.enabled && st.hovered {
            hint::set_text(ui.ctx(), self.name, self.value, self.caps);
        }
        if ui.is_rect_visible(rect) {
            with_brush(ui.painter(), |b| {
                well::menu_field(b, rect, s, st, self.value, self.steppers)
            });
        }
        (resp, c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An idle knob on a log range — whose value does not survive the taper's round trip
    /// exactly — reports no change, so it writes no command.
    #[test]
    fn an_idle_frame_changes_nothing() {
        let ctx = egui::Context::default();
        let mut hz = 2_400.0f32;
        let mut changed = false;
        for _ in 0..3 {
            ctx.run_ui(egui::RawInput::default(), |ui| {
                changed |= Param::new(&mut hz, 20.0..=20_000.0, 1_000.0)
                    .log(true)
                    .show(ui)
                    .changed();
            })
            .textures_delta
            .clear();
        }
        assert!(!changed, "an idle knob reported a change");
        assert_eq!(hz, 2_400.0);
    }
}
