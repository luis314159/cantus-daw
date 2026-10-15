//! The kit parts Organum's and Lamina's editors share (ADR 0024 §4, §8), each bound to one
//! parameter and drawn at the shell's scale in the section's function hue:
//!
//! - [`cell`]: the kit's cell (value pill, knob, label), with the
//!   readouts these two instruments need that the house one does not print (rpm, cm,
//!   degrees, millimetres, a scanner's hertz with decimals, a voice count).
//! - [`switch`]: a segmented control over a stepped parameter (Stop · Slow · Fast).
//! - [`lamp`]: an LED button over an on/off parameter.
//! - [`card`]: a card with a title that leaves room for a control in its title bar.
//! - [`label`]: every word in a box, measured, never outside it.
//! - [`Rows`]: cards laid out in rows with 4-pt gutters inside the area the shell hands us.
//!
//! Widgets place themselves in a rect (a child `Ui` there), so the layout is computed first
//! and the controls land where the cards were painted.

use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::band::{self, CardLook, CardRects};
use crate::kit::cell;
use crate::kit::hint::Caps;
use crate::kit::knob::KnobSize;
use crate::kit::text::{self as ktext, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::Param;
use crate::kit::{with_brush, Brush, Function, State, Style};
use crate::views::inst::{source_family, write_readout};
use cantus_core::inst::modulation::connections;
use cantus_core::{InstrumentKind, NativeSpec, TrackId};
use egui::{Align2, Pos2, Rect, Sense, Vec2};
use std::fmt::Write as _;

/// Write `v` the way a player reads it, in the parameter's unit, without allocating.
pub fn readout(key: &str, unit: &str, v: f32, b: &mut String) {
    let _ = match unit {
        "rpm" => write!(b, "{v:.0} rpm"),
        "cm" => write!(b, "{v:.0} cm"),
        "\u{b0}" => write!(b, "{v:.0}\u{b0}"),
        "mm" => write!(b, "{v:.2} mm"),
        "voices" => write!(b, "{v:.0}"),
        "Hz" if v < 100.0 => write!(b, "{v:.2} Hz"),
        "dB" if key == "dynamics" => write!(b, "{v:.0} dB"),
        "%" if key == "vel.curve" => write!(b, "{:+.0} %", v * 100.0),
        _ => {
            write_readout(v, unit, b);
            Ok(())
        }
    };
}

/// One parameter as a kit cell: range, taper, unit and label from its description (the
/// label abbreviates through `<label_key>.short`), one undo step per drag, a modulation
/// source dropped on it connects it, what modulates it drawn as rings.
pub fn cell(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    s: &Style,
    size: KnobSize,
) -> egui::Response {
    param(app, ui, track, spec, key, s, size, true)
}

#[allow(clippy::too_many_arguments)]
fn param(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    s: &Style,
    size: KnobSize,
    as_cell: bool,
) -> egui::Response {
    let Some(info) = spec.kind.param(key) else {
        return ui.allocate_response(Vec2::ZERO, Sense::hover());
    };
    let mut v = spec.get(key);
    let mut rings: Vec<(f32, egui::Color32)> = connections(spec)
        .filter(|c| c.dst == key && c.on)
        .map(|c| (c.amount, s.t.hue(source_family(c.src))))
        .collect();
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
    let (pkey, unit) = (info.key, info.unit);
    let mut p = Param::new(&mut v, info.min..=info.max, info.default)
        .size(size)
        .log(info.logarithmic)
        .name(&name)
        .style(*s)
        .rings(&rings)
        .automation(w.automated)
        .modulation(w.offer);
    if as_cell {
        p = p.cell(Src::Key(info.label_key));
    }
    let (r, choice) = p
        .format(move |v, b| readout(pkey, unit, v, b))
        .show_full(ui);
    crate::views::inst::frame::wired(app, &w, choice, r.rect);
    if r.changed() {
        let v = if info.stepped { v.round() } else { v };
        app.set_inst_value(track, key, v, true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    if egui::DragAndDrop::has_payload_of_type::<&'static str>(ui.ctx()) && r.contains_pointer() {
        with_brush(ui.painter(), |b| {
            b.rrect_stroke(r.rect, s.r(4.0), s.s(1.5), s.hue)
        });
    }
    if let Some(src) = r.dnd_release_payload::<&'static str>() {
        app.connect_modulation(track, *src, key);
    }
    r
}

/// Run `add` in a child `Ui` covering `rect`, its ids salted by `salt`.
pub fn at<R>(
    ui: &mut egui::Ui,
    rect: Rect,
    salt: impl std::hash::Hash + std::fmt::Debug,
    add: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).id_salt(salt));
    add(&mut child)
}

/// A row of cells across `rect`, vertically centred, spread over its width (gaps between
/// 2 and 14 points), centred when they do not need it all.
#[allow(clippy::too_many_arguments)]
pub fn cells(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    track: TrackId,
    spec: &NativeSpec,
    keys: &[&str],
    s: &Style,
    size: KnobSize,
) {
    let n = keys.len();
    if n == 0 {
        return;
    }
    let sz = cell::size(size) * s.k;
    let spare = rect.width() - sz.x * n as f32;
    let gap = if n > 1 {
        (spare / (n - 1) as f32).clamp(s.s(2.0), s.s(14.0))
    } else {
        0.0
    };
    let total = sz.x * n as f32 + gap * n.saturating_sub(1) as f32;
    let x0 = rect.left() + ((rect.width() - total) * 0.5).max(0.0);
    let y0 = rect.center().y - sz.y * 0.5;
    for (i, key) in keys.iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x0 + i as f32 * (sz.x + gap), y0), sz);
        at(ui, r, ("cell", *key), |ui| {
            cell(app, ui, track, spec, key, s, size);
        });
        record(app, spec, key, r);
    }
}

/// Keep where a control landed, for the tests that aim at it.
fn record(app: &mut CantusApp, spec: &NativeSpec, key: &str, r: Rect) {
    let Some(info) = spec.kind.param(key) else {
        return;
    };
    match spec.kind {
        InstrumentKind::Organ => {
            app.inst.organ.placed.insert(info.key, r);
        }
        InstrumentKind::ElectricPiano => {
            app.inst.epiano.placed.insert(info.key, r);
        }
        InstrumentKind::Chorda => {
            app.inst.chorda.placed.insert(info.key, r);
        }
        _ => {}
    }
}

/// A segmented control over the stepped parameter `key`, filling `rect`: segment `i` sets
/// it to `i`. A click is one undo step.
#[allow(clippy::too_many_arguments)]
pub fn switch(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    labels: &[Src<'_>],
    s: &Style,
) -> egui::Response {
    let n = labels.len();
    record(app, spec, key, rect);
    let now = (spec.get(key).round().max(0.0) as usize).min(n.saturating_sub(1));
    let name = spec
        .kind
        .param(key)
        .map(|i| t(i.label_key))
        .unwrap_or_default();
    let (resp, chosen) = crate::kit::widgets::Segmented::new(now, labels)
        .id(egui::Id::new(("keys-switch", track.0, key)))
        .name(&name)
        .caps(Caps::PARAM)
        .style(*s)
        .show_at(ui, rect);
    if let Some(i) = chosen {
        app.set_inst_value(track, key, i as f32, false);
    }
    resp
}

/// An LED button over the on/off parameter `key` (on at 0.5 and above), filling `rect`.
#[allow(clippy::too_many_arguments)]
pub fn lamp(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    rect: Rect,
    track: TrackId,
    spec: &NativeSpec,
    key: &str,
    label: Src<'_>,
    s: &Style,
) -> egui::Response {
    record(app, spec, key, rect);
    let on = spec.get(key) >= 0.5;
    let name = spec
        .kind
        .param(key)
        .map(|i| t(i.label_key))
        .unwrap_or_default();
    let resp = crate::kit::widgets::LedButton::new(on, label)
        .id(egui::Id::new(("keys-lamp", track.0, key)))
        .name(&name)
        .style(*s)
        .show_at(ui, rect);
    if resp.clicked() {
        app.set_inst_value(track, key, if on { 0.0 } else { 1.0 }, false);
    }
    resp
}

/// What a card of these editors shows.
#[derive(Copy, Clone, Debug)]
pub struct Card<'a> {
    /// The title's i18n key.
    pub title: &'a str,
    pub f: Function,
    /// A power LED in the title bar, lit when `Some(true)`.
    pub power: Option<bool>,
    /// A display under the title bar, this tall at 100 %.
    pub display: Option<f32>,
    /// Points (at 100 %) kept free at the right of the title bar for a control.
    pub tail: f32,
}

impl<'a> Card<'a> {
    pub fn new(title: &'a str, f: Function) -> Self {
        Card {
            title,
            f,
            power: None,
            display: None,
            tail: 0.0,
        }
    }
    pub fn display(mut self, h: f32) -> Self {
        self.display = Some(h);
        self
    }
    pub fn power(mut self, on: bool) -> Self {
        self.power = Some(on);
        self
    }
    pub fn tail(mut self, w: f32) -> Self {
        self.tail = w;
        self
    }
}

/// Paint a card; its title stops short of the `tail` kept for a control.
pub fn card(b: &mut Brush, rect: Rect, s: &Style, c: &Card<'_>) -> CardRects {
    let s = s.function(c.f);
    let look = CardLook {
        title: if c.tail > 0.0 {
            Src::Str("")
        } else {
            Src::Key(c.title)
        },
        switchable: c.power.is_some(),
        selector: None,
        menu: false,
        display: c.display,
    };
    let st = State {
        lit: c.power.unwrap_or(true),
        ..State::REST
    };
    let r = band::card(b, rect, &s, st, &look);
    if c.tail > 0.0 {
        let x = r
            .led
            .map_or(r.title.left() + s.s(17.0), |l| l.right() + s.s(4.0));
        let right = tail_rect(&r, &s, c.tail).left() - s.s(6.0);
        let color = if c.power == Some(false) {
            s.t.text_dim
        } else {
            s.t.label
        };
        let tt = Text::key(c.title, Role::Section, color)
            .caps(true)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = ktext::paint(
            b,
            Rect::from_min_max(
                Pos2::new(x, r.title.top()),
                Pos2::new(right.max(x), r.title.bottom()),
            ),
            &tt,
            s.tk(),
        );
    }
    r
}

/// The rect at the right of a card's title bar kept for a control `w` points wide.
pub fn tail_rect(r: &CardRects, s: &Style, w: f32) -> Rect {
    let tb = r.title;
    Rect::from_min_max(
        Pos2::new(tb.right() - s.s(6.0) - s.s(w), tb.top() + s.s(3.0)),
        Pos2::new(tb.right() - s.s(6.0), tb.bottom() - s.s(3.0)),
    )
}

/// Paint a card in its own brush and return where its parts are.
pub fn paint_card(ui: &egui::Ui, rect: Rect, s: &Style, c: &Card<'_>) -> CardRects {
    with_brush(&ui.painter_at(rect.expand(s.s(4.0))), |b| {
        card(b, rect, s, c)
    })
}

/// Text in a box: `role`, left or centred, elided at the end if it must be.
pub fn label(b: &mut Brush, rect: Rect, text: &str, role: Role, color: egui::Color32, s: &Style) {
    let _ = ktext::paint(
        b,
        rect,
        &Text::new(text, role, color)
            .caps(false)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End)),
        s.tk(),
    );
}

/// Cards in rows: a cursor down the area the shell hands us, 4-pt gutters.
#[derive(Copy, Clone, Debug)]
pub struct Rows {
    area: Rect,
    gutter: f32,
    y: f32,
}

impl Rows {
    pub fn new(area: Rect, s: &Style) -> Rows {
        Rows {
            area,
            gutter: s.d(4.0),
            y: area.top(),
        }
    }

    /// A row `h` tall split by `fracs` (which sum to 1), gutters between.
    pub fn row(&mut self, h: f32, fracs: &[f32]) -> Vec<Rect> {
        let w = self.area.width() - self.gutter * fracs.len().saturating_sub(1) as f32;
        let mut x = self.area.left();
        let out = fracs
            .iter()
            .map(|f| {
                let r = Rect::from_min_size(Pos2::new(x, self.y), Vec2::new(w * f, h));
                x += w * f + self.gutter;
                r
            })
            .collect();
        self.y += h + self.gutter;
        out
    }
}

/// Split `rect` into a column of `n` equal rects `h` tall each, `gap` apart, from the top.
pub fn stack(rect: Rect, h: f32, gap: f32, n: usize) -> Vec<Rect> {
    (0..n)
        .map(|i| {
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.top() + i as f32 * (h + gap)),
                Vec2::new(rect.width(), h),
            )
        })
        .collect()
}

/// Split `rect` horizontally by `fracs` with `gap` between.
pub fn split(rect: Rect, gap: f32, fracs: &[f32]) -> Vec<Rect> {
    let w = rect.width() - gap * fracs.len().saturating_sub(1) as f32;
    let mut x = rect.left();
    fracs
        .iter()
        .map(|f| {
            let r = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(w * f, rect.height()));
            x += w * f + gap;
            r
        })
        .collect()
}
