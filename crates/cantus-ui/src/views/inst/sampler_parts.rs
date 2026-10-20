//! Kit controls placed at a device window's own scale and hue (ADR 0024 §4, §8), shared by
//! Exemplar and Pulsus II.
//!
//! The kit's thin widgets size themselves with `Style::current()` — 100 % and the tone
//! hue — so inside a shell drawn at 75 % or 200 %, or in a card of another function, they
//! would come out the wrong size and colour. These place the kit's own painters in a rect
//! the caller laid out, with the shell's [`Style`]: the same parts, the same states, the
//! same hints, at the window's scale.

use crate::kit::band::{self, CardLook, CardRects};
use crate::kit::hint::{self, Caps};
use crate::kit::knob::KnobSize;
use crate::kit::menu::{self, Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{FieldClicks, Param};
use crate::kit::{cell, well, with_brush, State, Style};
use crate::widgets::knob::{pos_of, value_at, Taper};
use egui::{Align2, Color32, Pos2, Rect, Response, Sense, Ui, UiBuilder, Vec2};
// The keyed controls (round 29, review 2 of ADR 0062).
use crate::app::CantusApp;
use cantus_core::{NativeSpec, TrackId};

/// A row control's height (LED button, segmented, menu field, readout) at 100 %.
pub const ROW: f32 = 22.0;
/// The gap between controls in a card, at 100 %.
pub const GAP: f32 = 4.0;
/// A card's title bar plus its body's padding, at 100 %: what a card adds to its contents.
pub const CARD_CHROME: f32 = band::TITLE + 16.0;

/// A child `Ui` whose rect is `rect`: where a kit widget that allocates its own size goes.
pub fn at(ui: &mut Ui, rect: Rect) -> Ui {
    ui.new_child(UiBuilder::new().max_rect(rect))
}

/// The size of a cell of `size` at this style's scale.
pub fn cell_size(s: &Style, size: KnobSize) -> Vec2 {
    cell::size(size) * s.k
}

/// A card of cells `n` wide, `rows` high, with `size` knobs: its width and height.
pub fn card_for(s: &Style, size: KnobSize, n: usize, rows: usize) -> Vec2 {
    let c = cell::size(size);
    let n = n.max(1) as f32;
    let rows = rows.max(1) as f32;
    Vec2::new(
        16.0 + c.x * n + GAP * (n - 1.0),
        CARD_CHROME + c.y * rows + GAP * (rows - 1.0),
    ) * s.k
}

/// Rects for `n` cells of `size`, left to right from the top left of `body`, `per_row` to
/// a row.
pub fn cells(body: Rect, s: &Style, size: KnobSize, n: usize, per_row: usize) -> Vec<Rect> {
    let c = cell_size(s, size);
    let gap = GAP * s.k;
    let per_row = per_row.max(1);
    (0..n)
        .map(|i| {
            let (col, row) = ((i % per_row) as f32, (i / per_row) as f32);
            Rect::from_min_size(
                Pos2::new(
                    body.left() + col * (c.x + gap),
                    body.top() + row * (c.y + gap),
                ),
                c,
            )
        })
        .collect()
}

/// Split `total` among parts that need `needs` each (and a gutter between them): each gets
/// its need, and what is left is shared in proportion.
pub fn fracs(needs: &[f32], total: f32, gutter: f32) -> Vec<f32> {
    let n = needs.len().max(1) as f32;
    let room = (total - gutter * (n - 1.0)).max(1.0);
    let sum: f32 = needs.iter().sum::<f32>().max(1.0);
    let spare = (room - sum).max(0.0);
    needs
        .iter()
        .map(|w| {
            let w = if sum > room {
                w * room / sum
            } else {
                w + spare * w / sum
            };
            w / room
        })
        .collect()
}

/// Paint a card in `rect` and return where its parts are. `on` lights its LED (or keeps
/// a card without one at full strength).
pub fn card(ui: &Ui, rect: Rect, s: &Style, look: &CardLook<'_>, on: bool) -> CardRects {
    let st = State {
        lit: on,
        ..State::REST
    };
    with_brush(ui.painter(), |b| band::card(b, rect, s, st, look))
}

/// Clicks on a card's type selector (‹ name ›): -1 for ‹, +1 for ›, 0 otherwise.
pub fn selector_step(ui: &mut Ui, rects: &CardRects, s: &Style, name: &str) -> i32 {
    selector_step_resp(ui, rects, s, name).0
}

/// [`selector_step`] over the stepped parameter `key`: its menu, its mapped mark, the
/// learn mode (round 29, review 2 of ADR 0062, #44). No step while the learn mode has it.
#[allow(clippy::too_many_arguments)]
pub fn selector_step_key(
    app: &mut CantusApp,
    ui: &mut Ui,
    rects: &CardRects,
    s: &Style,
    name: &str,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
) -> i32 {
    let (step, resp) = selector_step_resp(ui, rects, s, name);
    match resp {
        Some(r) if crate::controllers::stepped_inst(app, ui, track, spec, key, &r) => 0,
        _ => step,
    }
}

fn selector_step_resp(
    ui: &mut Ui,
    rects: &CardRects,
    s: &Style,
    name: &str,
) -> (i32, Option<Response>) {
    let Some(sr) = rects.selector else {
        return (0, None);
    };
    let resp = ui.allocate_rect(sr, Sense::click());
    if resp.hovered() {
        hint::set_text(ui.ctx(), name, "", Caps::PARAM);
        with_brush(ui.painter(), |b| {
            b.rrect(
                b.snap_rect(sr.shrink2(Vec2::new(0.0, 3.0 * s.k))),
                s.r(3.0),
                crate::kit::Fill::Solid(crate::kit::color::with_alpha(s.hue, 0.12)),
            )
        });
    }
    if !resp.clicked() {
        return (0, Some(resp));
    }
    let step = match resp.interact_pointer_pos() {
        Some(p) if p.x < sr.center().x => -1,
        _ => 1,
    };
    (step, Some(resp))
}

/// [`card_led`] that is the stepped parameter `key`'s control (a group's mute): its menu,
/// its mapped mark, the learn mode (round 29, review 2 of ADR 0062, #44). Not clicked
/// while the learn mode has it.
#[allow(clippy::too_many_arguments)]
pub fn card_led_key(
    app: &mut CantusApp,
    ui: &mut Ui,
    rects: &CardRects,
    name: &str,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
) -> bool {
    let Some(r) = rects.led else {
        return false;
    };
    let resp = ui.allocate_rect(r.expand(3.0), Sense::click());
    if resp.hovered() {
        hint::set_text(ui.ctx(), name, "", Caps::PARAM);
    }
    let learning = crate::controllers::stepped_inst(app, ui, track, spec, key, &resp);
    resp.clicked() && !learning
}

/// [`seg`] over the stepped parameter `key`: its menu, its mapped mark, the learn mode
/// (review 2, #44). No choice while the learn mode has it.
#[allow(clippy::too_many_arguments)]
pub fn seg_key(
    app: &mut CantusApp,
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    selected: usize,
    labels: &[Src<'_>],
    name: &str,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
) -> Option<usize> {
    ui.expand_to_include_rect(rect);
    let (resp, chosen) = crate::kit::widgets::Segmented::new(selected, labels)
        .name(name)
        .style(*s)
        .show_at(ui, rect);
    app.controllers
        .inst_words
        .note(spec.kind, key, || crate::controllers_words::srcs(labels));
    let learning = crate::controllers::stepped_inst(app, ui, track, spec, key, &resp);
    chosen.filter(|_| !learning)
}

/// A card's LED (when it is switchable): whether it was clicked.
pub fn card_led(ui: &mut Ui, rects: &CardRects, name: &str) -> bool {
    let Some(r) = rects.led else {
        return false;
    };
    let resp = ui.allocate_rect(r.expand(3.0), Sense::click());
    if resp.hovered() {
        hint::set_text(ui.ctx(), name, "", Caps::PARAM);
    }
    resp.clicked()
}

/// A number as a kit cell in `rect` (or a bare knob when `label` is `None`): returns the
/// response and the value to store. A stepped number keeps the fraction a drag has
/// travelled under `id` while it is dragged, so a slow drag still reaches the next step.
#[allow(clippy::too_many_arguments)]
pub fn num_cell(
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    id: egui::Id,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    log: bool,
    stepped: bool,
    label: Option<Src<'_>>,
    name: &str,
    size: KnobSize,
    rings: &[(f32, Color32)],
    fmt: impl Fn(f32, &mut String),
) -> (Response, f32) {
    let (r, v, _) = num_cell_offered(
        ui, rect, s, id, value, range, default, log, stepped, label, name, size, rings, None, fmt,
    );
    (r, v)
}

/// [`num_cell`] on a parameter: its automation mark and menu and its modulation
/// (`offer`: automated, the modulators' offer), and what its menu asked.
#[allow(clippy::too_many_arguments)]
pub fn num_cell_offered(
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    id: egui::Id,
    value: f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    log: bool,
    stepped: bool,
    label: Option<Src<'_>>,
    name: &str,
    size: KnobSize,
    rings: &[(f32, Color32)],
    offer: Option<(bool, crate::kit::automate::ModOffer)>,
    fmt: impl Fn(f32, &mut String),
) -> (Response, f32, Option<crate::kit::automate::Choice>) {
    let held = if stepped {
        ui.data(|d| d.get_temp::<f32>(id))
    } else {
        None
    };
    let mut v = held.unwrap_or(value);
    let mut child = at(ui, rect);
    let mut p = Param::new(&mut v, range, default)
        .size(size)
        .log(log)
        .name(name)
        .style(*s)
        .rings(rings);
    if let Some(l) = label {
        p = p.cell(l);
    }
    match offer {
        Some((automated, m)) => p = p.automation(automated).modulation(m),
        None => p = p.plain(),
    }
    let (r, choice) = p.format(&fmt).show_full(&mut child);
    if stepped {
        if r.dragged() {
            ui.data_mut(|d| d.insert_temp(id, v));
        } else if held.is_some() {
            ui.data_mut(|d| d.remove::<f32>(id));
        }
    }
    (r, if stepped { v.round() } else { v }, choice)
}

/// An on/off as an LED button in `rect`. Toggle on click (check `clicked()`).
pub fn led(ui: &mut Ui, rect: Rect, s: &Style, on: bool, label: Src<'_>, name: &str) -> Response {
    ui.expand_to_include_rect(rect);
    crate::kit::widgets::LedButton::new(on, label)
        .name(name)
        .style(*s)
        .show_at(ui, rect)
}

/// An action (Find best loop, Remove zone): an LED button whose lamp lights while pressed.
pub fn press(ui: &mut Ui, rect: Rect, s: &Style, label: Src<'_>, name: &str) -> Response {
    ui.expand_to_include_rect(rect);
    crate::kit::widgets::LedButton::new(false, label)
        .name(name)
        .caps(Caps::NONE)
        .momentary(true)
        .style(*s)
        .show_at(ui, rect)
}

/// A choice among a few words: the index chosen this frame, if it changed.
pub fn seg(
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    selected: usize,
    labels: &[Src<'_>],
    name: &str,
) -> Option<usize> {
    ui.expand_to_include_rect(rect);
    crate::kit::widgets::Segmented::new(selected, labels)
        .name(name)
        .style(*s)
        .show_at(ui, rect)
        .1
}

/// A menu field showing `value`, with ‹ › when `steppers`; `open` lights its outline.
pub fn field(
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    value: &str,
    steppers: bool,
    open: bool,
    name: &str,
) -> (Response, FieldClicks) {
    ui.expand_to_include_rect(rect);
    crate::kit::widgets::MenuField::new(value)
        .steppers(steppers)
        .open(open)
        .name(name)
        .style(*s)
        .show_at(ui, rect)
}

/// A value in a well; `lit` outlines it in the hue (the selected row of a table).
pub fn readout(
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    value: &str,
    numeric: bool,
    lit: bool,
    name: &str,
) -> Response {
    let resp = ui.allocate_rect(rect, Sense::click());
    let st = State::of(&resp, lit);
    if st.hovered {
        hint::set_text(ui.ctx(), name, value, Caps::NONE);
    }
    with_brush(ui.painter(), |b| {
        well::readout(b, rect, s, st, value, numeric)
    });
    resp
}

/// A number in a well that is dragged like a knob (200 px for the whole travel, Shift for
/// fine, double-click resets): a table's cells, a sample's frames. `fmt` writes it.
#[allow(clippy::too_many_arguments)]
pub fn drag_num(
    ui: &mut Ui,
    rect: Rect,
    s: &Style,
    v: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    default: f32,
    lit: bool,
    name: &str,
    fmt: impl Fn(f32, &mut String),
) -> Response {
    let (min, max) = (*range.start(), *range.end());
    let mut resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let start = v.clamp(min, max);
    let mut p = pos_of(start, min, max, Taper::Linear);
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            0.2
        } else {
            1.0
        };
        let d = resp.drag_delta();
        p += (d.x - d.y) * fine / 200.0;
    }
    let mut nv = value_at(p.clamp(0.0, 1.0), min, max, Taper::Linear);
    if resp.double_clicked() {
        nv = default.clamp(min, max);
    }
    if (nv - start).abs() > f32::EPSILON {
        *v = nv;
        resp.mark_changed();
    }
    let st = State::of(&resp, lit);
    let mut buf = String::with_capacity(16);
    fmt(*v, &mut buf);
    if st.hovered {
        hint::set_text(ui.ctx(), name, &buf, Caps::PARAM);
    }
    if st.hovered || st.active {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    with_brush(ui.painter(), |b| well::readout(b, rect, s, st, &buf, true));
    resp
}

/// A list dropped below `anchor` while it is open (the state is kept under `id`): the
/// index of the item chosen, if one was. `toggle` opens or closes it this frame.
pub fn dropdown(
    ui: &mut Ui,
    id: egui::Id,
    anchor: Rect,
    s: &Style,
    items: &[Item<'_>],
    toggle: bool,
) -> Option<usize> {
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if toggle {
        open = !open;
    }
    if !open {
        ui.data_mut(|d| d.remove::<bool>(id));
        return None;
    }
    let d = s.d(1.0);
    let w = anchor.width().max(180.0 * s.k);
    let h = menu::height(items, d);
    let mut chosen = None;
    let area = egui::Area::new(id.with("list"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.left_bottom() + Vec2::new(0.0, 2.0 * s.k))
        .show(ui.ctx(), |ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
            let hovered = resp.hover_pos().and_then(|p| {
                (0..items.len())
                    .find(|&i| items[i].is_action() && menu::row(rect, items, d, i).contains(p))
            });
            with_brush(ui.painter(), |b| menu::paint(b, rect, s, items, hovered));
            if resp.clicked() {
                chosen = hovered;
            }
            resp
        });
    let close = chosen.is_some() || (!toggle && area.inner.clicked_elsewhere());
    ui.data_mut(|dd| {
        if close {
            dd.remove::<bool>(id);
        } else {
            dd.insert_temp(id, true);
        }
    });
    chosen
}

/// Menu rows for a list of names, the current one checked.
pub fn radio_items<'a>(labels: &'a [Src<'a>], current: Option<usize>) -> Vec<Item<'a>> {
    labels
        .iter()
        .enumerate()
        .map(|(i, l)| Item::Action {
            label: *l,
            check: match current {
                Some(c) => Check::Radio(c == i),
                None => Check::None,
            },
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect()
}

/// A word in a box: measured, fitted by `policy`, never outside `rect`.
#[allow(clippy::too_many_arguments)]
pub fn words(
    ui: &Ui,
    rect: Rect,
    s: &Style,
    src: Src<'_>,
    role: Role,
    color: Color32,
    align: Align2,
    policy: Policy,
) {
    let t = Text {
        src,
        role,
        // Words of the interface in caps where the role says so; data (a file, a span,
        // a name) as it is.
        caps: role.caps() && matches!(src, Src::Key(_)),
        align,
        policy,
        color,
    };
    with_brush(ui.painter(), |b| {
        let _ = text::paint(b, rect, &t, s.tk());
    });
}

/// The left cell of a label | value row: a caps label, right-aligned, abbreviated.
pub fn row_label(ui: &Ui, rect: Rect, s: &Style, key: &str) {
    let t = Text::key(key, Role::Label, s.t.label)
        .caps(true)
        .align(Align2::RIGHT_CENTER)
        .policy(Policy::Abbreviate);
    with_brush(ui.painter(), |b| {
        let _ = text::paint(b, rect, &t, s.tk());
    });
}

/// A name, elided in the middle as names are.
pub fn name_text<'a>(s: &'a str, role: Role, color: Color32) -> Text<'a> {
    Text::new(s, role, color)
        .caps(false)
        .policy(Policy::Elide(Elide::Middle))
}

/// A value in its unit, as a musician reads it, into `b` (no allocation): percentages
/// stored as 0…100, cents and semitones signed, stepped numbers whole; the rest as the
/// house readout writes them.
pub fn write_value(v: f32, unit: &str, stepped: bool, b: &mut String) {
    use std::fmt::Write as _;
    let _ = match unit {
        "%" => write!(b, "{v:.0} %"),
        "ct" => write!(b, "{v:+.0} ct"),
        "st" if stepped => write!(b, "{v:+.0} st"),
        "st" => write!(b, "{v:+.1} st"),
        "dB/s" => write!(b, "{v:.0} dB/s"),
        "°" => write!(b, "{v:.0}°"),
        "" if stepped => write!(b, "{v:.0}"),
        _ => {
            crate::views::inst::write_readout(v, unit, b);
            Ok(())
        }
    };
}

/// A pan stored as −100…100: `C`, `L40`, `R25`.
pub fn write_pan(v: f32, b: &mut String) {
    use std::fmt::Write as _;
    let _ = if v.abs() < 0.5 {
        write!(b, "C")
    } else if v < 0.0 {
        write!(b, "L{:.0}", -v)
    } else {
        write!(b, "R{v:.0}")
    };
}
