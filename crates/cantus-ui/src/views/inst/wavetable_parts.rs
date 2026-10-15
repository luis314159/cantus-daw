//! Kit parts bound to an instrument's numbers, placed at exact rects and drawn at the
//! shell's scale (ADR 0024 §4, §8) — what Unda's and Modulus's editors are built from:
//! cells (value pill, knob, label) with the instrument's own readouts, menu fields whose
//! list opens as a kit menu, steppers, segmented switches, LED buttons, cards with a
//! power lamp and a ‹ type › selector, and boxed text for displays.
//!
//! The kit's own widgets take the installed style at 100 %; these take the shell's
//! [`Style`], so a 200 % window draws its switches at 200 % too. Every gesture is one
//! command (a drag merges into one undo step), and every part writes its hint.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::band::{self, CardLook, CardRects};
use crate::kit::cell;
use crate::kit::hint::{self, Caps};
use crate::kit::knob::KnobSize;
use crate::kit::menu::{self, Check, Item};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::Param;
use crate::kit::{well, with_brush, Brush, State, Style};
use crate::views::inst::{source_family, write_readout};
use cantus_core::inst::modulation::connections;
use cantus_core::{NativeSpec, TrackId};
use egui::{Align2, Color32, Id, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::fmt::Write as _;

/// A readout written into a reused buffer.
pub type Fmt<'a> = &'a dyn Fn(f32, &mut String);

/// One cell to lay out: its parameter, an optional label of its own and readout.
#[derive(Copy, Clone)]
pub struct C<'a> {
    pub key: &'a str,
    pub label: Option<Src<'a>>,
    pub fmt: Option<Fmt<'a>>,
}

/// A cell for `key` with the parameter's label and the house readout.
pub fn c(key: &str) -> C<'_> {
    C {
        key,
        label: None,
        fmt: None,
    }
}

impl<'a> C<'a> {
    pub fn label(mut self, l: Src<'a>) -> Self {
        self.label = Some(l);
        self
    }
    pub fn fmt(mut self, f: Fmt<'a>) -> Self {
        self.fmt = Some(f);
        self
    }
}

/// The width and height of a cell of `size` at the style's scale.
pub fn cell_size(size: KnobSize, s: &Style) -> Vec2 {
    cell::size(size) * s.k
}

/// The gap between cells in a row.
pub fn gap(s: &Style) -> f32 {
    s.d(4.0)
}

/// How wide a row of `n` cells of `size` is.
pub fn row_width(n: usize, size: KnobSize, s: &Style) -> f32 {
    let w = cell_size(size, s).x;
    n as f32 * w + n.saturating_sub(1) as f32 * gap(s)
}

/// The modulation reaching `key`, as the kit's rings.
fn rings_for(spec: &NativeSpec, key: &str, s: &Style) -> Vec<(f32, Color32)> {
    connections(spec)
        .filter(|c| c.dst == key && c.on)
        .map(|c| (c.amount, s.t.hue(source_family(c.src))))
        .collect()
}

/// The readout a parameter gets unless its cell names another: the house one, with whole
/// numbers for counts, signed octaves and semitones, and degrees.
pub fn house(v: f32, unit: &str, stepped: bool, b: &mut String) {
    let _ = match unit {
        "oct" => write!(b, "{:+.0} oct", v),
        "st" if stepped => write!(b, "{:+.0} st", v),
        "°" => write!(b, "{v:.0}°"),
        "" if stepped => write!(b, "{:.0}", v),
        "dB" if v.abs() >= 10.0 => write!(b, "{v:+.0} dB"),
        _ => {
            write_readout(v, unit, b);
            Ok(())
        }
    };
}

/// The house readout shortened for a small (S) cell's pill, 40 pt wide: whole decibels,
/// semitones and cents, `3.2k` for kilohertz, `12 ms`.
pub fn house_s(v: f32, unit: &str, stepped: bool, b: &mut String) {
    let _ = match unit {
        "dB" | "st" | "ct" => write!(b, "{v:+.0} {unit}"),
        "Hz" if v >= 10_000.0 => write!(b, "{:.0}k", v / 1_000.0),
        "Hz" if v >= 1_000.0 => write!(b, "{:.1}k", v / 1_000.0),
        "Hz" if v < 10.0 => write!(b, "{v:.1} Hz"),
        "Hz" => write!(b, "{v:.0} Hz"),
        "ms" if v >= 1_000.0 => write!(b, "{:.1} s", v / 1_000.0),
        "ms" if v >= 10.0 => write!(b, "{v:.0} ms"),
        "s" if v < 0.01 => write!(b, "{:.1} ms", v * 1_000.0),
        "s" if v < 1.0 => write!(b, "{:.0} ms", v * 1_000.0),
        "s" => write!(b, "{v:.1} s"),
        _ => {
            house(v, unit, stepped, b);
            Ok(())
        }
    };
}

/// A cell (value pill, knob, label) for one number, its top-left at `at`: a drag is one
/// undo step (a stepped number keeps its fraction while dragged), a dropped modulation
/// source connects it, what modulates it is drawn as rings.
#[allow(clippy::too_many_arguments)]
pub fn cell_at(
    app: &mut CantusApp,
    ui: &mut Ui,
    track: TrackId,
    spec: &NativeSpec,
    c: C<'_>,
    s: &Style,
    size: KnobSize,
    at: Pos2,
) -> Response {
    param_at(app, ui, track, spec, c, s, size, at, true)
}

/// As [`cell_at`], a bare knob (inside a display or a table row), centred in a square
/// of the knob's diameter + 6 pt whose top-left is `at`.
#[allow(clippy::too_many_arguments)]
pub fn knob_at(
    app: &mut CantusApp,
    ui: &mut Ui,
    track: TrackId,
    spec: &NativeSpec,
    c: C<'_>,
    s: &Style,
    size: KnobSize,
    at: Pos2,
) -> Response {
    param_at(app, ui, track, spec, c, s, size, at, false)
}

#[allow(clippy::too_many_arguments)]
fn param_at(
    app: &mut CantusApp,
    ui: &mut Ui,
    track: TrackId,
    spec: &NativeSpec,
    c: C<'_>,
    s: &Style,
    size: KnobSize,
    at: Pos2,
    as_cell: bool,
) -> Response {
    let key = c.key;
    let Some(info) = spec.kind.param(key) else {
        return ui.interact(
            Rect::from_min_size(at, Vec2::ZERO),
            ui.id().with(("missing", key)),
            Sense::hover(),
        );
    };
    let dim = if as_cell {
        cell_size(size, s)
    } else {
        Vec2::splat((size.diameter() + 6.0) * s.k)
    };
    let rect = Rect::from_min_size(at, dim);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .id_salt(("cell", key, as_cell, size))
            .layout(egui::Layout::left_to_right(egui::Align::Min)),
    );
    // A stepped number keeps its unrounded value while dragged, so slow drags still move.
    let shadow_id = child.id().with("shadow");
    let shadow: Option<f32> = info
        .stepped
        .then(|| child.data(|d| d.get_temp(shadow_id)))
        .flatten();
    let mut v = shadow.unwrap_or_else(|| spec.get(key));
    let mut rings = rings_for(spec, key, s);
    let mut mod_rings = [(0.0, egui::Color32::TRANSPARENT); crate::views::modulator::MAX_RINGS];
    let w = crate::views::inst::frame::wire(
        app,
        track,
        key,
        &mut v,
        (info.min, info.max, info.logarithmic),
        s,
        &mut mod_rings,
    );
    rings.extend_from_slice(&mod_rings[..w.n]);
    let name = t(info.label_key);
    let unit = info.unit;
    let stepped = info.stepped;
    let label = c.label.unwrap_or(Src::Key(info.label_key));
    let own = c.fmt;
    let small = matches!(size, KnobSize::S | KnobSize::Mini);
    let mut p = Param::new(&mut v, info.min..=info.max, info.default)
        .size(size)
        .log(info.logarithmic)
        .name(&name)
        .style(*s)
        .rings(&rings)
        .offers(w.on, w.automated, w.offer);
    if as_cell {
        p = p.cell(label);
    }
    let (r, choice) = p
        .format(move |v, b| match own {
            Some(f) => f(v, b),
            None if small => house_s(if stepped { v.round() } else { v }, unit, stepped, b),
            None => house(if stepped { v.round() } else { v }, unit, stepped, b),
        })
        .show_full(&mut child);
    crate::views::inst::frame::wired(app, &w, choice, r.rect);
    if r.changed() {
        let nv = if stepped { v.round() } else { v };
        if stepped && r.dragged() {
            child.data_mut(|d| d.insert_temp(shadow_id, v));
        }
        if nv != spec.get(key) {
            app.set_inst_value(track, key, nv, true);
        }
    }
    if stepped && !r.dragged() && shadow.is_some() {
        child.data_mut(|d| d.remove::<f32>(shadow_id));
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    if egui::DragAndDrop::has_payload_of_type::<&'static str>(ui.ctx()) && r.contains_pointer() {
        with_brush(ui.painter(), |b| {
            b.rrect_stroke(r.rect, s.r(6.0), s.s(1.5), s.hue);
        });
    }
    if let Some(src) = r.dnd_release_payload::<&'static str>() {
        app.connect_modulation(track, *src, key);
    }
    r
}

/// A row of cells from `rect`'s left, vertically centred; returns the x after the last.
#[allow(clippy::too_many_arguments)]
pub fn cells(
    app: &mut CantusApp,
    ui: &mut Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    size: KnobSize,
    rect: Rect,
    list: &[C<'_>],
) -> f32 {
    let sz = cell_size(size, s);
    let mut x = rect.left();
    let y = rect.center().y - sz.y * 0.5;
    for c in list {
        cell_at(app, ui, track, spec, *c, s, size, Pos2::new(x, y));
        x += sz.x + gap(s);
    }
    x - gap(s)
}

/// A row of cells spread evenly across `rect` (the gaps grow, the cells keep their size).
#[allow(clippy::too_many_arguments)]
pub fn cells_spread(
    app: &mut CantusApp,
    ui: &mut Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    size: KnobSize,
    rect: Rect,
    list: &[C<'_>],
) {
    let sz = cell_size(size, s);
    let n = list.len().max(1) as f32;
    let spare = (rect.width() - n * sz.x).max(0.0);
    let g = if list.len() > 1 {
        (spare / (n - 1.0)).min(sz.x * 0.5)
    } else {
        0.0
    };
    let used = n * sz.x + (n - 1.0) * g;
    let mut x = rect.left() + ((rect.width() - used) * 0.5).max(0.0);
    let y = rect.center().y - sz.y * 0.5;
    for c in list {
        cell_at(app, ui, track, spec, *c, s, size, Pos2::new(x, y));
        x += sz.x + g;
    }
}

// ---- cards ---------------------------------------------------------------------------------

/// A card: its frame, title bar and display ground. `power` is the section's on/off (the
/// lamp lights when on); the caller paints the body and the display's content.
pub fn card(
    ui: &Ui,
    rect: Rect,
    s: &Style,
    title: Src<'_>,
    power: Option<bool>,
    selector: Option<&str>,
    display: Option<f32>,
) -> CardRects {
    let look = CardLook {
        title,
        switchable: power.is_some(),
        selector,
        menu: false,
        display,
    };
    let st = State {
        lit: power.unwrap_or(true),
        ..State::REST
    };
    with_brush(&ui.painter_at(rect.expand(s.s(4.0))), |b| {
        band::card(b, rect, s, st, &look)
    })
}

/// The card's power lamp as a switch for a 0/1 number: one undo step per click.
#[allow(clippy::too_many_arguments)]
pub fn power(
    app: &mut CantusApp,
    ui: &Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    cr: &CardRects,
) {
    let Some(led) = cr.led else {
        return;
    };
    let hit = Rect::from_min_max(
        Pos2::new(cr.title.left(), cr.title.top()),
        Pos2::new(led.right() + (led.width() * 0.5), cr.title.bottom()),
    );
    let r = ui.interact(
        hit,
        ui.id().with(("power", key, cr.title.width() as i32)),
        Sense::click(),
    );
    if r.hovered() {
        hint::set_text(ui.ctx(), &t("unda.power_tip"), "", Caps::NONE);
    }
    if r.clicked() {
        let on = spec.get(key) >= 0.5;
        app.set_inst_value(track, key, if on { 0.0 } else { 1.0 }, false);
    }
}

/// What a card's ‹ name › selector reports.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Pick {
    pub prev: bool,
    pub next: bool,
    pub open: bool,
}

/// The card's selector as a control: ‹ steps back, › forward, the name opens the list.
pub fn selector(ui: &Ui, id: Id, cr: &CardRects, name: &str) -> Pick {
    let Some(r) = cr.selector else {
        return Pick::default();
    };
    let resp = ui.interact(r, id, Sense::click());
    if resp.hovered() {
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
    let mut p = Pick::default();
    if resp.clicked() {
        if let Some(at) = resp.interact_pointer_pos() {
            let third = r.width() / 4.0;
            p.prev = at.x < r.left() + third * 0.6;
            p.next = at.x > r.right() - third * 0.6;
            p.open = !p.prev && !p.next;
        }
    }
    p
}

/// A step through `n` choices from `now`, wrapping: `Some(new)` when it moved.
pub fn step(now: usize, n: usize, p: Pick) -> Option<usize> {
    if n == 0 {
        return None;
    }
    if p.prev {
        Some((now + n - 1) % n)
    } else if p.next {
        Some((now + 1) % n)
    } else {
        None
    }
}

// ---- menus, fields, switches ---------------------------------------------------------------

/// A kit menu under `anchor` while its id is open: `Some(i)` when row `i` is chosen. A
/// click anywhere else closes it.
pub fn popup(ui: &Ui, id: Id, anchor: Rect, s: &Style, items: &[Item<'_>]) -> Option<usize> {
    let open: bool = ui.data(|d| d.get_temp(id)).unwrap_or(false);
    if !open {
        return None;
    }
    let k = s.d(1.0);
    let width = anchor.width().max(s.s(170.0));
    let h = menu::height(items, k);
    let area = egui::Area::new(id.with("area"))
        .order(egui::Order::Foreground)
        .fixed_pos(anchor.left_bottom() + Vec2::new(0.0, s.s(2.0)))
        .show(ui.ctx(), |ui| {
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, h), Sense::click());
            let hovered = resp.hover_pos().and_then(|p| {
                (0..items.len())
                    .find(|&i| items[i].is_action() && menu::row(rect, items, k, i).contains(p))
            });
            with_brush(ui.painter(), |b| menu::paint(b, rect, s, items, hovered));
            (resp, hovered)
        });
    let (resp, hovered) = area.inner;
    let chosen = if resp.clicked() { hovered } else { None };
    if chosen.is_some() || resp.clicked_elsewhere() {
        ui.data_mut(|d| d.insert_temp(id, false));
    }
    chosen
}

/// Open (or close) the menu `id`.
pub fn toggle_popup(ui: &Ui, id: Id) {
    let open: bool = ui.data(|d| d.get_temp(id)).unwrap_or(false);
    ui.data_mut(|d| d.insert_temp(id, !open));
}

/// Radio rows for `labels`, `selected` marked.
pub fn radio_items<'a>(labels: &'a [String], selected: usize) -> Vec<Item<'a>> {
    labels
        .iter()
        .enumerate()
        .map(|(i, l)| Item::Action {
            label: Src::Str(l),
            check: Check::Radio(i == selected),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect()
}

/// A menu field in `rect` with ‹ › steppers: the value is `labels[selected]`, the steppers
/// walk the list (wrapping), the field opens it as a kit menu. `Some(i)` when chosen.
pub fn choice(
    ui: &Ui,
    id: Id,
    rect: Rect,
    s: &Style,
    labels: &[String],
    selected: usize,
    name: &str,
) -> Option<usize> {
    let open: bool = ui.data(|d| d.get_temp(id.with("menu"))).unwrap_or(false);
    let value = labels.get(selected).map(String::as_str).unwrap_or("");
    let (_, c) = crate::kit::widgets::MenuField::new(value)
        .steppers(true)
        .open(open)
        .id(id)
        .name(name)
        .style(*s)
        .show_at(ui, rect);
    let n = labels.len().max(1);
    let out = if c.prev {
        Some((selected + n - 1) % n)
    } else if c.next {
        Some((selected + 1) % n)
    } else {
        if c.open {
            toggle_popup(ui, id.with("menu"));
        }
        None
    };
    let fr = well::field_rects(rect, s, true);
    let items = radio_items(labels, selected);
    popup(ui, id.with("menu"), fr.field, s, &items).or(out)
}

/// [`choice`] bound to a stepped number whose values are named by `labels`: one undo step
/// per choice.
#[allow(clippy::too_many_arguments)]
pub fn choice_key(
    app: &mut CantusApp,
    ui: &Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    rect: Rect,
    s: &Style,
    labels: &[String],
) {
    let now = spec.get(key).round().max(0.0) as usize;
    let name = spec
        .kind
        .param(key)
        .map(|i| t(i.label_key))
        .unwrap_or_default();
    if let Some(i) = choice(
        ui,
        ui.id().with(("choice", key)),
        rect,
        s,
        labels,
        now,
        &name,
    ) {
        if i != now {
            app.set_inst_value(track, key, i as f32, false);
        }
    }
}

/// [`choice_key`] without the ‹ › steppers, for a narrow field: the list opens on a click.
#[allow(clippy::too_many_arguments)]
pub fn choice_key_plain(
    app: &mut CantusApp,
    ui: &Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    rect: Rect,
    s: &Style,
    labels: &[String],
) {
    let now = spec.get(key).round().max(0.0) as usize;
    let name = spec
        .kind
        .param(key)
        .map(|i| t(i.label_key))
        .unwrap_or_default();
    let id = ui.id().with(("choice", key));
    let open: bool = ui.data(|d| d.get_temp(id.with("menu"))).unwrap_or(false);
    let value = labels.get(now).map(String::as_str).unwrap_or("");
    let (_, c) = crate::kit::widgets::MenuField::new(value)
        .open(open)
        .id(id)
        .name(&name)
        .style(*s)
        .show_at(ui, rect);
    if c.open {
        toggle_popup(ui, id.with("menu"));
    }
    let items = radio_items(labels, now);
    if let Some(i) = popup(ui, id.with("menu"), rect, s, &items) {
        if i != now {
            app.set_inst_value(track, key, i as f32, false);
        }
    }
}

/// A whole number in a well with ‹ › beside it (octaves, semitones): the steppers, the
/// wheel or a vertical drag (8 pt a step) move it; a double-click resets it. One undo
/// step per gesture.
#[allow(clippy::too_many_arguments)]
pub fn stepper_key(
    app: &mut CantusApp,
    ui: &Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    rect: Rect,
    s: &Style,
    fmt: Fmt<'_>,
) {
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let id = ui.id().with(("stepper", key));
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let fr = well::field_rects(rect, s, true);
    let v = spec.get(key);
    let mut nv = v;
    if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            if fr.prev.is_some_and(|r| r.contains(p)) {
                nv -= 1.0;
            } else if fr.next.is_some_and(|r| r.contains(p)) {
                nv += 1.0;
            }
        }
    }
    let mut merge = false;
    if resp.dragged() {
        let steps = ui.data_mut(|d| {
            let a = d.get_temp_mut_or_default::<f32>(id.with("acc"));
            *a -= resp.drag_delta().y;
            let whole = (*a / (8.0 * s.k)).trunc();
            *a -= whole * 8.0 * s.k;
            whole
        });
        nv += steps;
        merge = true;
    }
    if resp.double_clicked() {
        nv = info.default;
    }
    let nv = nv.clamp(info.min, info.max).round();
    if nv != v {
        app.set_inst_value(track, key, nv, merge);
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    let st = State::of(&resp, false);
    let mut buf = String::new();
    fmt(nv, &mut buf);
    if st.hovered {
        hint::set_text(ui.ctx(), &t(info.label_key), &buf, Caps::PARAM);
    }
    with_brush(ui.painter(), |b| {
        well::readout(b, fr.field, s, st, &buf, true);
        let dim = s.t.text_dim;
        for (side, g) in [
            (fr.prev, crate::kit::glyph::Glyph::Prev),
            (fr.next, crate::kit::glyph::Glyph::Next),
        ] {
            if let Some(r) = side {
                crate::kit::glyph::paint(b, r.shrink(s.s(2.0)), g, dim);
            }
        }
    });
}

/// A segmented switch in `rect`: `Some(i)` when segment `i` is clicked.
pub fn seg(
    ui: &Ui,
    id: Id,
    rect: Rect,
    s: &Style,
    labels: &[Src<'_>],
    selected: usize,
    name: &str,
) -> Option<usize> {
    crate::kit::widgets::Segmented::new(selected, labels)
        .id(id)
        .name(name)
        .style(*s)
        .show_at(ui, rect)
        .1
}

/// [`seg`] bound to a stepped number.
#[allow(clippy::too_many_arguments)]
pub fn seg_key(
    app: &mut CantusApp,
    ui: &Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    rect: Rect,
    s: &Style,
    labels: &[Src<'_>],
) {
    let now = spec.get(key).round().max(0.0) as usize;
    let name = spec
        .kind
        .param(key)
        .map(|i| t(i.label_key))
        .unwrap_or_default();
    if let Some(i) = seg(ui, ui.id().with(("seg", key)), rect, s, labels, now, &name) {
        app.set_inst_value(track, key, i as f32, false);
    }
}

/// An LED button in `rect`: `true` when clicked.
pub fn led_button(
    ui: &Ui,
    id: Id,
    rect: Rect,
    s: &Style,
    on: bool,
    label: Src<'_>,
    name: &str,
) -> bool {
    crate::kit::widgets::LedButton::new(on, label)
        .id(id)
        .name(name)
        .style(*s)
        .show_at(ui, rect)
        .clicked()
}

/// [`led_button`] bound to a 0/1 number, labelled by the parameter.
#[allow(clippy::too_many_arguments)]
pub fn led_key(
    app: &mut CantusApp,
    ui: &Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    rect: Rect,
    s: &Style,
) {
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    let on = spec.get(key) >= 0.5;
    let name = t(info.label_key);
    if led_button(
        ui,
        ui.id().with(("led", key)),
        rect,
        s,
        on,
        Src::Key(info.label_key),
        &name,
    ) {
        app.set_inst_value(track, key, if on { 0.0 } else { 1.0 }, false);
    }
}

/// A button: an LED button whose lamp never lights (an action, not a state).
pub fn action(
    ui: &Ui,
    id: Id,
    rect: Rect,
    s: &Style,
    label: Src<'_>,
    name: &str,
    enabled: bool,
) -> bool {
    crate::kit::widgets::LedButton::new(false, label)
        .id(id)
        .name(name)
        .caps(Caps::NONE)
        .enabled(enabled)
        .style(*s)
        .show_at(ui, rect)
        .clicked()
}

// ---- text in boxes -------------------------------------------------------------------------

/// Paint `s` in `rect` in `role`, aligned, elided at the end when it must be.
pub fn say(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    txt: &str,
    role: Role,
    color: Color32,
    align: Align2,
) {
    let _ = text::paint(
        b,
        rect,
        &Text::new(txt, role, color)
            .caps(false)
            .align(align)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
}

/// A label of a label | value row: right-aligned, in caps, abbreviated when it must be.
pub fn row_label(b: &mut Brush, rect: Rect, s: &Style, key: &str) {
    let _ = text::paint(
        b,
        rect,
        &Text::key(key, Role::Label, s.t.label)
            .caps(true)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Abbreviate),
        s.tk(),
    );
}

/// A small readout chip inside a display (its corner): the text on a darker plate.
pub fn chip(b: &mut Brush, rect: Rect, s: &Style, txt: &str, color: Color32, align: Align2) {
    let plate = crate::kit::color::with_alpha(s.t.ink, 0.72);
    let ctx = b.ctx().clone();
    let size = text::measure(&ctx, &Text::new(txt, Role::Micro, color), s.tk());
    let w = (size.x + s.s(8.0)).min(rect.width());
    let r = align.align_size_within_rect(Vec2::new(w, rect.height()), rect);
    b.rrect(r, s.r(3.0), crate::kit::Fill::Solid(plate));
    let _ = text::paint(
        b,
        r.shrink2(Vec2::new(s.s(4.0), 0.0)),
        &Text::new(txt, Role::Micro, color)
            .caps(false)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
}

/// Hint for a display: its name and gestures in the help line while hovered.
pub fn display_hint(ui: &Ui, resp: &Response, name_key: &str, value: &str) {
    if resp.hovered() || resp.dragged() {
        hint::set_text(ui.ctx(), &t(name_key), value, Caps::PARAM);
    }
}

// ---- layout --------------------------------------------------------------------------------

/// Split `rect` into columns by `fracs` (summing to 1) with 4-pt gutters.
pub fn columns(rect: Rect, s: &Style, fracs: &[f32]) -> Vec<Rect> {
    let g = s.d(4.0);
    let w = rect.width() - g * fracs.len().saturating_sub(1) as f32;
    let mut x = rect.left();
    fracs
        .iter()
        .map(|f| {
            let r = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w * f, rect.height()));
            x += w * f + g;
            r
        })
        .collect()
}

/// Rows down a rect: each call takes `h` points (already scaled) and a gutter.
pub struct Rows {
    pub rect: Rect,
    pub y: f32,
    pub gutter: f32,
}

impl Rows {
    pub fn new(rect: Rect, s: &Style) -> Rows {
        Rows {
            rect,
            y: rect.top(),
            gutter: s.d(4.0),
        }
    }

    pub fn take(&mut self, h: f32) -> Rect {
        let r = Rect::from_min_size(
            Pos2::new(self.rect.left(), self.y),
            Vec2::new(self.rect.width(), h),
        );
        self.y += h + self.gutter;
        r
    }

    /// What is left below.
    pub fn rest(&self) -> Rect {
        Rect::from_min_max(Pos2::new(self.rect.left(), self.y), self.rect.max)
    }
}

/// A card's body below its display (or its title bar).
pub fn below_display(cr: &CardRects, s: &Style) -> Rect {
    Rect::from_min_max(
        Pos2::new(
            cr.body.left(),
            cr.display.map_or(cr.body.top(), |d| d.bottom() + s.s(8.0)),
        ),
        cr.body.max,
    )
}

/// The selected card's border, in its hue.
pub fn outline(ui: &Ui, rect: Rect, s: &Style) {
    with_brush(&ui.painter_at(rect.expand(s.s(2.0))), |b| {
        let r = b.snap_rect(rect);
        b.rrect_stroke(r.shrink(s.s(0.75)), s.r(6.0), s.s(1.5), s.hue);
    });
}

/// A menu field that does not apply here: dimmed, not hidden, and not clickable.
pub fn field_off(ui: &Ui, rect: Rect, s: &Style, value: &str, steppers: bool) {
    let _ = crate::kit::widgets::MenuField::new(value)
        .steppers(steppers)
        .enabled(false)
        .style(*s)
        .show_at(ui, rect);
}

/// The area between two curves of the same length (a modulation's reach in a display), as
/// one unfeathered mesh so the quads show no seams. Painted straight away, under whatever
/// the display's brush paints next.
pub fn band(painter: &egui::Painter, top: &[Pos2], bottom: &[Pos2], color: Color32) {
    let n = top.len().min(bottom.len());
    if n < 2 {
        return;
    }
    let mut mesh = egui::Mesh::default();
    for i in 0..n {
        mesh.colored_vertex(top[i], color);
        mesh.colored_vertex(bottom[i], color);
    }
    for i in 0..n as u32 - 1 {
        let a = 2 * i;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    painter.add(egui::Shape::mesh(mesh));
}
