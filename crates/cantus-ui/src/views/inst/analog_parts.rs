//! Kit parts bound to an instrument's numbers and placed by rect (ADR 0024 §4, §8): what
//! Calor's and Gravis's editors are built from.
//!
//! - [`card`]: a section card in its function hue, its title in a box that stays clear of
//!   a control the title bar may hold ([`bar`]), its display ground bleeding to the edges.
//! - [`Item`], [`layout`] and [`section`]: a card's controls — cells, segmented choices,
//!   LED toggles, columns of them, and slots the caller fills — measured before the card
//!   is painted, so a card is as tall as what it holds, and wrapped into lines when the
//!   card is narrow.
//! - The bound controls: [`cell_at`] (the kit Cell: value pill, knob, label; modulation
//!   rings; a dropped source connects; a drag is one undo step), [`choice_at`] (a
//!   segmented control over a stepped key), [`toggle_at`] (an LED button over an on/off
//!   key), [`stepper_at`] (a menu field with ‹ › over a long list), [`push_at`] (a cap
//!   with a word, for actions) and [`words`] (a text in a box).
//!
//! Sizes are logical points at 100 %, multiplied by the shell's scale `k`.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::band::{self, CardLook, CardRects};
use crate::kit::cell;
use crate::kit::hint::{self, Caps};
use crate::kit::knob::KnobSize;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::Param;
use crate::kit::{self, cap, Brush, State, Style};
use crate::views::inst::{source_family, write_readout};
use cantus_core::inst::modulation::connections;
use cantus_core::{NativeSpec, TrackId};
use egui::{Align2, Color32, Pos2, Rect, Response, Sense, Vec2};
use std::fmt::Write as _;

/// The gap between controls in a card, and between their lines.
pub const GAP: f32 = 6.0;
/// A segmented control's and a menu field's height.
pub const ROW: f32 = 24.0;
/// An LED button's and a push button's height.
pub const LED: f32 = 20.0;
/// A caption over a segmented control.
pub const CAPTION: f32 = 13.0;
/// A card's title bar.
pub const TITLE: f32 = band::TITLE;
/// A card's padding around its body.
pub const PAD: f32 = 8.0;

// ---- readouts ------------------------------------------------------------------------------

/// How a cell prints its value.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Fmt {
    /// The house readout for the parameter's unit.
    Unit,
    /// A signed whole number and a unit: `+1 oct`, `-7 st`.
    Signed(&'static str),
    /// A whole number after a prefix: `8`, `#12`.
    Whole(&'static str),
    /// Degrees: `90°`.
    Degrees,
    /// A pan position: `L40`, `C`, `R25`.
    Pan,
    /// A MIDI note's name: `C2`.
    Note,
    /// A number with one decimal and a unit: `2.2 mm`, `+3.0 dB/oct`.
    One(&'static str),
    /// A whole number and a unit: `864 mm`, `1 oct`.
    Int(&'static str),
    /// A slow rate in Hz with the decimals it needs: `0.20 Hz`, `12.5 Hz`, `350 Hz`.
    Hz,
    /// ±cents of Gravis's outer unison voices, for this many voices.
    Unison(u8),
}

impl Fmt {
    /// Write `v` (in `unit`, the parameter's) into `b`.
    pub fn write(self, v: f32, unit: &str, b: &mut String) {
        let _ = match self {
            Fmt::Unit => {
                write_readout(v, unit, b);
                Ok(())
            }
            Fmt::Signed("") => write!(b, "{v:+.0}"),
            Fmt::Signed(u) => write!(b, "{v:+.0} {u}"),
            Fmt::Whole(p) => write!(b, "{p}{v:.0}"),
            Fmt::Degrees => write!(b, "{v:.0}\u{b0}"),
            Fmt::Pan if v.abs() < 0.005 => write!(b, "C"),
            Fmt::Pan if v < 0.0 => write!(b, "L{:.0}", -v * 100.0),
            Fmt::Pan => write!(b, "R{:.0}", v * 100.0),
            Fmt::Note => write!(
                b,
                "{}",
                cantus_core::pitch_name(v.round().clamp(0.0, 127.0) as u8)
            ),
            Fmt::One(u) => write!(b, "{v:.1} {u}"),
            Fmt::Int(u) => write!(b, "{v:.0} {u}"),
            Fmt::Hz if v < 10.0 => write!(b, "{v:.2} Hz"),
            Fmt::Hz if v < 100.0 => write!(b, "{v:.1} Hz"),
            Fmt::Hz => {
                write_readout(v, "Hz", b);
                Ok(())
            }
            Fmt::Unison(n) => write!(
                b,
                "\u{b1}{:.0} ct",
                cantus_dsp::gravis::synth::outer_cents(n as usize, v as f64)
            ),
        };
    }
}

// ---- words ---------------------------------------------------------------------------------

/// Paint a text in a box through `b`: measured, fitted by its policy, never outside.
pub fn words(b: &mut Brush, rect: Rect, text: &Text<'_>, s: &Style) -> Option<text::Painted> {
    text::paint(b, rect, text, s.tk())
}

/// A micro caption (a small control's name), in caps, left-aligned, abbreviated.
pub fn caption_text<'a>(src: Src<'a>, s: &Style) -> Text<'a> {
    Text {
        src,
        role: Role::Micro,
        caps: true,
        align: Align2::LEFT_CENTER,
        policy: Policy::Abbreviate,
        color: s.t.text_dim,
    }
}

// ---- cards ---------------------------------------------------------------------------------

/// Paint a card in `rect` in `s`'s hue: the title (boxed, clear of `reserve` points at the
/// right of the title bar, where [`bar`] puts a control), the display ground when
/// `display` is set (this tall at 100 %).
pub fn card(
    ui: &egui::Ui,
    rect: Rect,
    s: &Style,
    title: Src<'_>,
    display: Option<f32>,
    reserve: f32,
) -> CardRects {
    kit::with_brush(&ui.painter_at(rect.expand(2.0 * s.k)), |b| {
        let look = CardLook {
            title: Src::Str(""),
            switchable: false,
            selector: None,
            menu: false,
            display,
        };
        let cr = band::card(b, rect, s, State::LIT, &look);
        let tb = cr.title;
        let right = tb.right()
            - s.s(8.0)
            - if reserve > 0.0 {
                s.s(reserve + 8.0)
            } else {
                0.0
            };
        let r = Rect::from_min_max(
            Pos2::new(tb.left() + s.s(17.0), tb.top()),
            Pos2::new(right.max(tb.left() + s.s(17.0)), tb.bottom()),
        );
        let t = Text {
            src: title,
            role: Role::Section,
            caps: true,
            align: Align2::LEFT_CENTER,
            policy: Policy::Elide(Elide::End),
            color: s.t.label,
        };
        let _ = words(b, r, &t, s);
        cr
    })
}

/// The place for a control `w` points wide at the right of a card's title bar.
pub fn bar(cr: &CardRects, s: &Style, w: f32) -> Rect {
    let tb = cr.title;
    Rect::from_min_max(
        Pos2::new(tb.right() - s.s(6.0 + w), tb.top() + s.s(2.0)),
        Pos2::new(tb.right() - s.s(6.0), tb.bottom() - s.s(2.0)),
    )
}

// ---- the bound controls --------------------------------------------------------------------

/// A parameter's cell with its top-left corner at `at`: the kit Cell, bound. Range, taper
/// and label from the parameter (the label abbreviates through `<label_key>.short`), the
/// value printed by `fmt`, a drag is one undo step, a modulation source dropped on it
/// connects it, and what modulates it is drawn as rings.
#[allow(clippy::too_many_arguments)]
pub fn cell_at(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    s: &Style,
    size: KnobSize,
    fmt: Fmt,
    at: Pos2,
) -> Response {
    let rect = Rect::from_min_size(at, cell::size(size) * s.k);
    bound(app, ui, track, spec, key, s, (size, fmt), rect, true)
}

#[allow(clippy::too_many_arguments)]
fn bound(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    s: &Style,
    (size, fmt): (KnobSize, Fmt),
    rect: Rect,
    as_cell: bool,
) -> Response {
    let Some(info) = spec.kind.param(key) else {
        return ui.allocate_rect(rect, Sense::hover());
    };
    let mut v = spec.get(key);
    let rings: Vec<(f32, Color32)> = connections(spec)
        .filter(|c| c.dst == key && c.on)
        .map(|c| (c.amount, s.t.hue(source_family(c.src))))
        .collect();
    let name = t(info.label_key);
    let unit = info.unit;
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect)
            .id_salt(("bound", key)),
    );
    let mut p = Param::new(&mut v, info.min..=info.max, info.default)
        .size(size)
        .log(info.logarithmic)
        .name(&name)
        .style(*s)
        .rings(&rings);
    if as_cell {
        p = p.cell(Src::Key(info.label_key));
    }
    let r = p.format(move |v, b| fmt.write(v, unit, b)).show(&mut child);
    if r.changed() {
        let v = if info.stepped { v.round() } else { v };
        app.set_inst_value(track, key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    if egui::DragAndDrop::has_payload_of_type::<&'static str>(ui.ctx()) && r.contains_pointer() {
        kit::with_brush(&ui.painter_at(r.rect.expand(2.0 * s.k)), |b| {
            b.rrect_stroke(r.rect, s.r(6.0), s.s(1.5), s.hue);
        });
    }
    if let Some(src) = r.dnd_release_payload::<&'static str>() {
        app.connect_modulation(track, *src, key);
    }
    r
}

/// The labels of a choice as the kit's sources, without allocating (eight at most).
fn sources<'a>(labels: &[&'a str]) -> ([Src<'a>; 8], usize) {
    let mut out = [Src::Str(""); 8];
    let n = labels.len().min(8);
    for (o, l) in out.iter_mut().zip(labels) {
        *o = Src::Key(l);
    }
    (out, n)
}

/// A segmented control over a stepped key in `rect`: `labels` are i18n keys (each
/// abbreviates through `<key>.short`), the first standing for the value `first`. One click
/// is one undo step.
#[allow(clippy::too_many_arguments)]
pub fn choice_at(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    labels: &[&str],
    first: f32,
    rect: Rect,
    s: &Style,
) -> Response {
    let (srcs, n) = sources(labels);
    let now = ((spec.get(key).round() - first).max(0.0) as usize).min(n.saturating_sub(1));
    let name = param_name(spec, key);
    let value = labels.get(now).map(|l| t(l)).unwrap_or_default();
    let (resp, chosen) = crate::kit::widgets::Segmented::new(now, &srcs[..n])
        .id(ui.id().with(("choice", key)))
        .name(&name)
        .value(&value)
        .caps(Caps::PARAM)
        .style(*s)
        .show_at(ui, rect);
    if let Some(i) = chosen {
        app.set_inst_value(track, key, first + i as f32, false);
    }
    resp
}

/// A parameter's name, for the hint bar.
fn param_name(spec: &NativeSpec, key: &str) -> String {
    spec.kind
        .param(key)
        .map(|i| t(i.label_key))
        .unwrap_or_default()
}

/// An LED button over an on/off key in `rect`; one click is one undo step.
#[allow(clippy::too_many_arguments)]
pub fn toggle_at(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    label: &str,
    rect: Rect,
    s: &Style,
) -> Response {
    let on = spec.get(key) >= 0.5;
    let name = t(label);
    let resp = crate::kit::widgets::LedButton::new(on, Src::Key(label))
        .id(ui.id().with(("toggle", key)))
        .name(&name)
        .style(*s)
        .show_at(ui, rect);
    if resp.clicked() {
        app.set_inst_value(track, key, if on { 0.0 } else { 1.0 }, false);
    }
    resp
}

/// A menu field with ‹ › over a stepped key with many values (an LFO's shapes): the arrows
/// step through `labels`, a click on the field steps forward. One undo step each.
#[allow(clippy::too_many_arguments)]
pub fn stepper_at(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    labels: &[&str],
    rect: Rect,
    s: &Style,
) -> Response {
    let n = labels.len().max(1) as i32;
    let now = (spec.get(key).round() as i32).clamp(0, n - 1);
    let name = param_name(spec, key);
    let value = t(labels[now as usize]);
    let (resp, c) = crate::kit::widgets::MenuField::new(&value)
        .steppers(true)
        .id(ui.id().with(("stepper", key)))
        .name(&name)
        .caps(Caps {
            menu: true,
            ..Caps::PARAM
        })
        .style(*s)
        .show_at(ui, rect);
    if resp.clicked() {
        let next = (now + if c.prev { -1 } else { 1 }).rem_euclid(n);
        app.set_inst_value(track, key, next as f32, false);
    }
    resp
}

/// A push button (a cap with a word) for an action: re-roll, calibrate, restore. `tip` is
/// what the help line says it does.
pub fn push_at(ui: &mut egui::Ui, rect: Rect, s: &Style, label: &str, tip: &str) -> Response {
    let resp = ui.interact(rect, ui.id().with(("push", label)), Sense::click());
    if resp.hovered() {
        hint::set_text(ui.ctx(), &t(label), &t(tip), Caps::NONE);
    }
    let st = State::of(&resp, false);
    kit::with_brush(&ui.painter_at(rect.expand(3.0 * s.k)), |b| {
        let base = if s.t.dark {
            crate::kit::color::shade(s.t.deck_title, 0.05)
        } else {
            s.t.deck
        };
        let look = cap::CapLook {
            base: Some(base),
            ..Default::default()
        };
        cap::paint(b, rect, s, st, &look);
        let t = Text {
            src: Src::Key(label),
            role: Role::Label,
            caps: true,
            align: Align2::CENTER_CENTER,
            policy: Policy::Abbreviate,
            color: if st.hovered { s.t.text } else { s.t.label },
        };
        let _ = words(b, rect.shrink2(Vec2::new(s.s(6.0), 0.0)), &t, s);
    });
    resp
}

// ---- a card's controls, measured then placed -----------------------------------------------

/// One thing in a card's body.
#[derive(Copy, Clone, Debug)]
pub enum Item<'a> {
    /// A parameter's cell.
    Cell(&'a str, KnobSize, Fmt),
    /// A segmented control over a stepped key, `w` points wide; with `caption`, the
    /// parameter's name over it.
    Choice {
        key: &'a str,
        labels: &'a [&'a str],
        first: f32,
        w: f32,
        caption: bool,
    },
    /// An LED button over an on/off key: (key, label key, width).
    Toggle(&'a str, &'a str, f32),
    /// A menu field with ‹ › over a long list: (key, labels, width), with its caption.
    Stepper(&'a str, &'a [&'a str], f32),
    /// Items stacked in a column.
    Column(&'a [Item<'a>]),
    /// A place the caller fills (a text, buttons, a small display): width, height.
    Slot(f32, f32),
    /// Start a new line.
    Break,
}

impl Item<'_> {
    /// The item's size at `s`'s scale.
    pub fn size(&self, s: &Style) -> Vec2 {
        let k = s.k;
        match self {
            Item::Cell(_, size, _) => cell::size(*size) * k,
            Item::Choice { w, caption, .. } => {
                Vec2::new(*w, ROW + if *caption { CAPTION } else { 0.0 }) * k
            }
            Item::Toggle(_, _, w) => Vec2::new(*w, LED) * k,
            Item::Stepper(_, _, w) => Vec2::new(*w, ROW + CAPTION) * k,
            Item::Column(items) => {
                let mut size = Vec2::ZERO;
                for (i, it) in items.iter().enumerate() {
                    let z = it.size(s);
                    size.x = size.x.max(z.x);
                    size.y += z.y + if i > 0 { GAP * k } else { 0.0 };
                }
                size
            }
            Item::Slot(w, h) => Vec2::new(*w, *h) * k,
            Item::Break => Vec2::ZERO,
        }
    }
}

/// Lay `items` out in lines from `origin`, `width` wide (wrapping when an item does not
/// fit), each item centred in its line's height. Writes a rect per item into `out` and
/// returns the height the lines take.
pub fn layout(items: &[Item<'_>], origin: Pos2, width: f32, s: &Style, out: &mut Vec<Rect>) -> f32 {
    out.clear();
    out.resize(items.len(), Rect::NOTHING);
    let gap = GAP * s.k;
    let right = origin.x + width;
    // The lines: (first item, one past the last, height).
    let mut lines: Vec<(usize, usize, f32)> = Vec::new();
    let (mut start, mut x, mut h) = (0usize, origin.x, 0.0f32);
    for (i, it) in items.iter().enumerate() {
        if matches!(it, Item::Break) {
            lines.push((start, i, h));
            (start, x, h) = (i + 1, origin.x, 0.0);
            continue;
        }
        let z = it.size(s);
        if x > origin.x && x + z.x > right + 0.5 {
            lines.push((start, i, h));
            (start, x, h) = (i, origin.x, 0.0);
        }
        x += z.x + gap;
        h = h.max(z.y);
    }
    lines.push((start, items.len(), h));
    let mut y = origin.y;
    let mut first = true;
    for (a, b, lh) in lines {
        if a >= b || lh <= 0.0 {
            continue;
        }
        if !first {
            y += gap;
        }
        first = false;
        let mut x = origin.x;
        for (i, it) in items.iter().enumerate().take(b).skip(a) {
            if matches!(it, Item::Break) {
                continue;
            }
            let z = it.size(s);
            out[i] = Rect::from_min_size(Pos2::new(x, y + (lh - z.y) * 0.5), z);
            x += z.x + gap;
        }
        y += lh;
    }
    y - origin.y
}

/// The height `items` take in a body `width` wide.
pub fn lines_height(items: &[Item<'_>], width: f32, s: &Style) -> f32 {
    let mut out = Vec::with_capacity(items.len());
    layout(items, Pos2::ZERO, width, s, &mut out)
}

/// The height of a card `width` wide holding `items` under a display `display` tall
/// (points at 100 %).
pub fn card_height(items: &[Item<'_>], width: f32, display: Option<f32>, s: &Style) -> f32 {
    let k = s.k;
    let head = TITLE * k + display.map_or(0.0, |d| (d + 1.0) * k);
    let body = if items.is_empty() {
        0.0
    } else {
        lines_height(items, width - 2.0 * PAD * k, s) + 2.0 * PAD * k
    };
    head + body
}

/// Draw `items` in the rects [`layout`] gave them; the rects of the slots, in order, go
/// into `slots` for the caller to fill.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    items: &[Item<'_>],
    rects: &[Rect],
    slots: &mut Vec<Rect>,
) {
    for (it, r) in items.iter().zip(rects) {
        draw_one(app, ui, track, spec, s, it, *r, slots);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_one(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    it: &Item<'_>,
    r: Rect,
    slots: &mut Vec<Rect>,
) {
    let k = s.k;
    let captioned = |ui: &egui::Ui, key: &str, r: Rect| {
        let label = spec.kind.param(key).map_or(key, |i| i.label_key);
        let cr = Rect::from_min_size(r.min, Vec2::new(r.width(), CAPTION * k));
        kit::with_brush(&ui.painter_at(cr), |b| {
            let _ = words(b, cr, &caption_text(Src::Key(label), s), s);
        });
        Rect::from_min_max(Pos2::new(r.left(), r.top() + CAPTION * k), r.max)
    };
    match *it {
        Item::Cell(key, size, fmt) => {
            cell_at(app, ui, track, spec, key, s, size, fmt, r.min);
        }
        Item::Choice {
            key,
            labels,
            first,
            caption,
            ..
        } => {
            let seg = if caption { captioned(&*ui, key, r) } else { r };
            choice_at(app, ui, track, spec, key, labels, first, seg, s);
        }
        Item::Toggle(key, label, _) => {
            toggle_at(app, ui, track, spec, key, label, r, s);
        }
        Item::Stepper(key, labels, _) => {
            let field = captioned(&*ui, key, r);
            stepper_at(app, ui, track, spec, key, labels, field, s);
        }
        Item::Column(items) => {
            let mut y = r.top();
            for child in items {
                let z = child.size(s);
                let cr = Rect::from_min_size(Pos2::new(r.left(), y), z);
                draw_one(app, ui, track, spec, s, child, cr, slots);
                y += z.y + GAP * k;
            }
        }
        Item::Slot(..) => slots.push(r),
        Item::Break => {}
    }
}

/// A whole card: painted in `rect` (its hue is `s`'s), `items` laid out in its body under
/// the display from its top, drawn and bound. Returns the card's rects
/// and the slots' rects, or `None` for a card outside the visible area (skipped).
#[allow(clippy::too_many_arguments)]
pub fn section(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
    title: Src<'_>,
    display: Option<f32>,
    reserve: f32,
    items: &[Item<'_>],
) -> Option<(CardRects, Vec<Rect>)> {
    if !ui.clip_rect().intersects(rect) {
        return None;
    }
    let cr = card(ui, rect, s, title, display, reserve);
    let k = s.k;
    let top = cr.display.map_or(cr.title.bottom(), |d| d.bottom());
    let body = Rect::from_min_max(
        Pos2::new(rect.left() + PAD * k, top + PAD * k),
        Pos2::new(rect.right() - PAD * k, rect.bottom() - PAD * k),
    );
    let mut rects = Vec::with_capacity(items.len());
    // Top-aligned, so the cards of a row line up line by line.
    layout(items, body.min, body.width(), s, &mut rects);
    let mut slots = Vec::new();
    draw(app, ui, track, spec, s, items, &rects, &mut slots);
    Some((cr, slots))
}

/// A row of cards `h` tall across `area` from `y`, split by `fracs` with the kit's 4-pt
/// gutters; returns their rects.
pub fn row(area: Rect, y: f32, h: f32, fracs: &[f32], s: &Style) -> Vec<Rect> {
    let gutter = gutter(s);
    let w = area.width() - gutter * fracs.len().saturating_sub(1) as f32;
    let mut x = area.left();
    fracs
        .iter()
        .map(|f| {
            let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w * f, h));
            x += w * f + gutter;
            r
        })
        .collect()
}

/// The kit's gutter between cards.
pub fn gutter(s: &Style) -> f32 {
    s.d(4.0)
}

/// The width a card needs to hold `items` with no wrapping: its widest line and its
/// padding.
pub fn natural_width(items: &[Item<'_>], s: &Style) -> f32 {
    let gap = GAP * s.k;
    let (mut widest, mut line, mut first) = (0.0f32, 0.0f32, true);
    for it in items {
        if matches!(it, Item::Break) {
            widest = widest.max(line);
            (line, first) = (0.0, true);
            continue;
        }
        line += it.size(s).x + if first { 0.0 } else { gap };
        first = false;
    }
    widest.max(line) + 2.0 * PAD * s.k
}

/// Fractions of a row `width` wide for cards holding `lists`, chosen so the row is as
/// short as it can be: each card starts at its natural width (the slack, or the
/// shortfall, spread in proportion), then the tallest card takes the width it needs to
/// lose a line from the others, as long as none of them grows as tall as it was.
pub fn balanced(lists: &[&[Item<'_>]], width: f32, s: &Style) -> Vec<f32> {
    let n = lists.len();
    let avail = (width - gutter(s) * n.saturating_sub(1) as f32).max(1.0);
    let natural: Vec<f32> = lists.iter().map(|l| natural_width(l, s)).collect();
    let floor: Vec<f32> = lists
        .iter()
        .map(|l| l.iter().map(|it| it.size(s).x).fold(0.0, f32::max) + 2.0 * PAD * s.k)
        .collect();
    let sum: f32 = natural.iter().sum::<f32>().max(1.0);
    let mut w: Vec<f32> = natural.iter().map(|x| x * avail / sum).collect();
    let h = |i: usize, x: f32| card_height(lists[i], x, None, s);
    // The narrowest width in `lo..=hi` at which card `i` is shorter than `limit`.
    let narrowest = |i: usize, lo: f32, hi: f32, limit: f32| {
        let (mut lo, mut hi) = (lo, hi);
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            if h(i, mid) < limit {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    };
    for _ in 0..12 {
        let hs: Vec<f32> = (0..n).map(|i| h(i, w[i])).collect();
        let Some(t) = (0..n).max_by(|a, b| hs[*a].total_cmp(&hs[*b])) else {
            break;
        };
        let top = hs[t] - 0.5;
        if h(t, avail) >= top {
            break;
        }
        let want = narrowest(t, w[t], avail, top);
        let need = want - w[t];
        let give: Vec<f32> = (0..n)
            .map(|i| {
                if i == t {
                    0.0
                } else {
                    let lo = floor[i].min(w[i]);
                    (w[i] - narrowest(i, lo, w[i], top)).max(0.0)
                }
            })
            .collect();
        let spare: f32 = give.iter().sum();
        if spare < need || need <= 0.0 {
            break;
        }
        for i in 0..n {
            w[i] -= give[i] * need / spare;
        }
        w[t] = want;
    }
    w.iter().map(|x| x / avail).collect()
}

/// The widths of `n` cards across `width` split by `fracs`, less the gutters.
pub fn widths(width: f32, fracs: &[f32], s: &Style) -> Vec<f32> {
    let w = width - gutter(s) * fracs.len().saturating_sub(1) as f32;
    fracs.iter().map(|f| w * f).collect()
}

/// A card of a row, for [`row_height`] and [`cards`].
pub struct Card<'a> {
    /// The title's i18n key.
    pub title: &'a str,
    pub f: crate::kit::Function,
    /// A display under the title bar, this tall at 100 %.
    pub display: Option<f32>,
    /// Points kept free at the right of the title bar, for a control there.
    pub reserve: f32,
    pub items: &'a [Item<'a>],
    /// Its share of the row's width.
    pub frac: f32,
}

/// The height of a row of `cards` across `width`: its tallest card's.
pub fn row_height(cards: &[Card<'_>], width: f32, s: &Style) -> f32 {
    let fracs: Vec<f32> = cards.iter().map(|c| c.frac).collect();
    widths(width, &fracs, s)
        .iter()
        .zip(cards)
        .map(|(w, c)| card_height(c.items, *w, c.display, s))
        .fold(0.0, f32::max)
}

/// Draw a row of `cards` `h` tall across `area` from `y`; returns what each drew (its
/// rects and its slots), `None` for a card out of sight.
#[allow(clippy::too_many_arguments)]
pub fn cards(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    area: Rect,
    y: f32,
    h: f32,
    cards: &[Card<'_>],
) -> Vec<Option<(CardRects, Vec<Rect>)>> {
    let fracs: Vec<f32> = cards.iter().map(|c| c.frac).collect();
    let rects = row(area, y, h, &fracs, s);
    cards
        .iter()
        .zip(rects)
        .map(|(c, r)| {
            let st = s.function(c.f);
            section(
                app,
                ui,
                track,
                spec,
                &st,
                r,
                Src::Key(c.title),
                c.display,
                c.reserve,
                c.items,
            )
        })
        .collect()
}
