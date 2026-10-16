//! The inspector's row grammar (ADR 0024 §7, Bitwig's inspector, `bitwig-studio.md` §4.1):
//! **label | value** rows of 24 pt, the label right-aligned in a fixed column, every value
//! in a `Well` (with a 2-pt level bar at its left edge when the value is normalised),
//! rows grouped under 20-pt section bands. What does not apply is dimmed, never hidden.
//!
//! Rows are allocated through egui (so the sheet scrolls and senses) and painted through
//! the inspector's one brush. Text that is not a translation key goes through [`Fmt`]:
//! a value is formatted when it changes, not every frame.

use super::{Act, MenuId, MenuOpen};
use crate::kit::band;
use crate::kit::color::{shade, with_alpha};
use crate::kit::glyph::{self, Glyph};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{cache, led, well, Brush, Fill, Function, State, Style, Tok};
use crate::widgets::knob::{pos_of, value_at, Taper};
use egui::{pos2, Align2, Id, Pos2, Rect, Response, Sense, Ui, Vec2};
use std::collections::HashMap;
use std::fmt::Write as _;

/// A row's height at 100 %.
pub const ROW: f32 = 24.0;
/// The well inside a row: the row less 2 pt above and below.
pub const WELL_INSET: f32 = 2.0;
/// The gap between the label column and the value.
pub const GAP: f32 = 8.0;
/// The side margin of rows inside the panel.
pub const MARGIN: f32 = 8.0;
/// Pointer travel for a whole value range, as the house knobs (`widgets::knob`).
const TRAVEL_PX: f32 = 200.0;
const FINE: f32 = 0.2;

/// Texts made from values or names, kept until what they were made from changes. A
/// lookup is a hash; a value that is not moving is never formatted again.
#[derive(Default)]
pub struct Fmt {
    map: HashMap<u64, (u64, String)>,
}

impl Fmt {
    /// The text under `key`, rewritten by `write` only when `stamp` differs from the one
    /// it was written with.
    pub fn get(&mut self, key: u64, stamp: u64, write: impl FnOnce(&mut String)) -> &str {
        let e = self
            .map
            .entry(key)
            .or_insert_with(|| (stamp.wrapping_add(1), String::new()));
        if e.0 != stamp {
            e.1.clear();
            write(&mut e.1);
            e.0 = stamp;
        }
        &e.1
    }

    /// The translation of `key` in the current locale, looked up once per locale.
    pub fn tr(&mut self, key: &str) -> &str {
        let stamp = crate::i18n::locale() as u64;
        self.get(cache::key(("tr", key)), stamp, |b| {
            b.push_str(&crate::i18n::t(key))
        })
    }

    /// Forget everything (a new subject): the strings go, the table keeps its room.
    pub fn clear(&mut self) {
        if self.map.len() > 512 {
            self.map.clear();
        }
    }
}

/// A value's stamp: its bits.
pub fn bits(v: f32) -> u64 {
    v.to_bits() as u64
}

// ---- formatting -----------------------------------------------------------------------

/// "+1.5 dB", "−2.4 dB", "−∞ dB" at the bottom of a fader's travel.
pub fn db(v: f32, b: &mut String) {
    if !v.is_finite() || v <= cantus_core::MIN_GAIN_DB {
        b.push_str("\u{2212}\u{221e} dB");
        return;
    }
    let r = (v * 10.0).round() / 10.0;
    if r < 0.0 {
        let _ = write!(b, "\u{2212}{:.1} dB", -r);
    } else {
        let _ = write!(b, "+{r:.1} dB");
    }
}

/// A linear level (a send's) as decibels.
pub fn level_db(v: f32, b: &mut String) {
    if v <= 1e-5 {
        b.push_str("\u{2212}\u{221e} dB");
    } else {
        db(20.0 * v.log10(), b);
    }
}

/// `C`, `L25`, `R100`.
pub fn pan(v: f32, b: &mut String) {
    if v.abs() < 0.005 {
        b.push('C');
    } else if v < 0.0 {
        let _ = write!(b, "L{:.0}", -v * 100.0);
    } else {
        let _ = write!(b, "R{:.0}", v * 100.0);
    }
}

/// "100 %".
pub fn percent(v: f32, b: &mut String) {
    let _ = write!(b, "{:.0} %", v * 100.0);
}

/// "90 Hz", "1.20 kHz", "12.0 kHz".
pub fn hz(v: f32, b: &mut String) {
    if v >= 10_000.0 {
        let _ = write!(b, "{:.1} kHz", v / 1_000.0);
    } else if v >= 1_000.0 {
        let _ = write!(b, "{:.2} kHz", v / 1_000.0);
    } else {
        let _ = write!(b, "{v:.0} Hz");
    }
}

/// A Q or a plain ratio: "0.71".
pub fn plain(v: f32, b: &mut String) {
    let _ = write!(b, "{v:.2}");
}

/// A parameter with a unit, the house way (`cantus_core::fx::readout`).
pub fn unit(v: f32, unit: &str, b: &mut String) {
    b.push_str(&cantus_core::fx::readout::readout(v, unit));
}

// ---- values -----------------------------------------------------------------------------

/// How a dragged value moves and how its level bar reads.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Range {
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub taper: Taper,
    pub stepped: bool,
    /// The level bar grows from the middle (pan, gain around unity).
    pub bipolar: bool,
}

impl Range {
    pub const fn lin(min: f32, max: f32, default: f32) -> Range {
        Range {
            min,
            max,
            default,
            taper: Taper::Linear,
            stepped: false,
            bipolar: false,
        }
    }

    pub const fn log(min: f32, max: f32, default: f32) -> Range {
        Range {
            taper: Taper::Log,
            ..Range::lin(min, max, default)
        }
    }

    pub const fn bipolar(mut self) -> Range {
        self.bipolar = true;
        self
    }

    pub const fn stepped(mut self, on: bool) -> Range {
        self.stepped = on;
        self
    }

    pub fn pos(&self, v: f32) -> f32 {
        pos_of(v, self.min, self.max, self.taper)
    }
}

/// A gain in dB placed on the fader's own travel, so a well and the fader agree.
pub const GAIN: Range = Range {
    min: cantus_core::MIN_GAIN_DB,
    max: cantus_core::MAX_GAIN_DB,
    default: 0.0,
    taper: Taper::Linear,
    stepped: false,
    bipolar: false,
};

/// Where a gain sits on the fader's travel (0…1).
pub fn gain_pos(db: f32) -> f32 {
    crate::widgets::fader::pos_of(db)
}

/// What a drag on a value did this frame.
#[derive(Clone, Debug)]
pub struct Drag {
    pub resp: Response,
    /// The new value, when it moved.
    pub value: Option<f32>,
    /// The gesture ended: close the undo step.
    pub ended: bool,
}

/// Sense a value in `rect`: vertical drag over 200 px for the whole travel (Shift for
/// fine), double-click to reset. Gains ride the fader's travel.
pub fn drag(ui: &mut Ui, id: Id, rect: Rect, v: f32, r: &Range, gain: bool) -> Drag {
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let p0 = if gain { gain_pos(v) } else { r.pos(v) };
    let mut p = p0;
    if resp.dragged() {
        let fine = if ui.input(|i| i.modifiers.shift) {
            FINE
        } else {
            1.0
        };
        let d = resp.drag_delta();
        p += (d.x - d.y) * fine / TRAVEL_PX;
    }
    let mut out = if gain {
        crate::widgets::fader::db_at(p.clamp(0.0, 1.0))
    } else {
        value_at(p.clamp(0.0, 1.0), r.min, r.max, r.taper)
    };
    if r.stepped {
        out = out.round();
    }
    if resp.double_clicked() {
        out = r.default;
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let moved = (out - v).abs() > f32::EPSILON && (resp.dragged() || resp.double_clicked());
    let ended = resp.drag_stopped() || resp.double_clicked();
    Drag {
        resp,
        value: moved.then_some(out),
        ended,
    }
}

// ---- painting -----------------------------------------------------------------------------

/// The 2-pt level bar at a well's left edge: the value's share of its travel, from the
/// bottom — or from the middle for a bipolar value.
pub fn level_bar(b: &mut Brush, well: Rect, s: &Style, pos: f32, bipolar: bool, alpha: f32) {
    let px = b.px();
    let x0 = b.snap(well.left() + 2.0 * px);
    let lane = Rect::from_min_max(
        pos2(x0, well.top() + 2.0 * px),
        pos2(x0 + b.snap(s.s(2.0)).max(px), well.bottom() - 2.0 * px),
    );
    b.rect(lane, Fill::Solid(with_alpha(s.hue, 0.18 * alpha)));
    let y_of = |p: f32| lane.bottom() - lane.height() * p.clamp(0.0, 1.0);
    let (a, z) = if bipolar {
        let (m, y) = (y_of(0.5), y_of(pos));
        (m.min(y), m.max(y))
    } else {
        (y_of(pos), lane.bottom())
    };
    if z - a > 0.25 * px {
        b.rect(
            Rect::from_min_max(pos2(lane.left(), a), pos2(lane.right(), z)),
            Fill::Solid(s.hue.gamma_multiply(alpha)),
        );
    }
}

/// A well holding `value`; `bar` adds the level bar (position, bipolar).
pub fn value_well(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    st: State,
    value: &str,
    numeric: bool,
    bar: Option<(f32, bool)>,
) {
    well::readout(b, rect, s, st, value, numeric);
    if let Some((p, bipolar)) = bar {
        level_bar(b, rect, s, p, bipolar, st.alpha());
    }
}

/// An italic placeholder in a well: an empty field says what would go there.
pub fn placeholder_well(b: &mut Brush, rect: Rect, s: &Style, st: State, key: &str) {
    well::paint(b, rect, s, st, None);
    b.register(rect, "placeholder", &[Tok::Placeholder], &[]);
    let t = Text::key(
        key,
        Role::Placeholder,
        s.t.placeholder.gamma_multiply(st.alpha()),
    )
    .align(Align2::LEFT_CENTER)
    .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, rect.shrink2(Vec2::new(s.s(7.0), 0.0)), &t, s.tk());
}

/// A small word on a chip (a plug-in's format, a warning), sized to fit; returns its
/// width so the caller can place the next thing.
pub fn chip(b: &mut Brush, right: Pos2, s: &Style, word: Src<'_>, tok: Tok, alpha: f32) -> f32 {
    let t = Text {
        src: word,
        ..Text::new("", Role::Micro, s.t.get(tok).gamma_multiply(alpha))
    }
    .caps(false);
    let w = text::measure(b.ctx(), &t, s.tk()).x + s.s(8.0);
    let h = s.s(13.0);
    let r = Rect::from_min_max(
        pos2(right.x - w, right.y - h * 0.5),
        pos2(right.x, right.y + h * 0.5),
    );
    b.register(r, "chip", &[Tok::Well], &[Some(tok)]);
    b.rrect(
        b.snap_rect(r),
        s.r(2.0),
        Fill::Solid(with_alpha(s.t.well, 0.85 * alpha)),
    );
    b.rrect_stroke(
        b.snap_rect(r).shrink(b.px() * 0.5),
        s.r(2.0),
        b.px(),
        with_alpha(s.t.get(tok), 0.55 * alpha),
    );
    let _ = text::paint(b, r, &t, s.tk());
    w
}

/// A 20-pt square key with a glyph on the raised recipe; `lit` in the section's hue.
pub fn glyph_key(b: &mut Brush, rect: Rect, s: &Style, st: State, g: Glyph) {
    let base = if st.lit {
        s.hue
    } else if s.t.dark {
        shade(s.t.deck_title, 0.05)
    } else {
        s.t.deck
    };
    b.register(
        rect,
        "glyph_key",
        &[Tok::DeckTitle, Tok::Deck, Tok::Text],
        &[s.hue_tok],
    );
    crate::kit::light::raised(
        b,
        rect,
        s.s(s.t.radius),
        if st.hovered { shade(base, 0.04) } else { base },
        crate::kit::light::RaisedOpts {
            drop: !st.active,
            k: s.k,
            alpha: st.alpha(),
            depth: s.t.depth,
            ..Default::default()
        },
    );
    let ink = if st.lit {
        crate::kit::color::on_fill(s.hue)
    } else if st.hovered {
        s.t.text
    } else {
        s.t.text_dim
    };
    glyph::paint(b, rect.shrink(s.s(5.0)), g, ink.gamma_multiply(st.alpha()));
}

// ---- the sheet ------------------------------------------------------------------------------

/// A dim readout at a band's right ("4 / 10"), formatted when its stamp changes.
pub struct Count<'c> {
    pub key: u64,
    pub stamp: u64,
    pub write: &'c dyn Fn(&mut String),
}

/// Where one label | value row landed.
#[derive(Copy, Clone, Debug)]
pub struct Row {
    pub full: Rect,
    pub label: Rect,
    /// The value's well (the row less its insets).
    pub value: Rect,
}

/// What a section band reported.
#[derive(Copy, Clone, Debug, Default)]
pub struct BandClicks {
    pub power: bool,
    pub plus: bool,
    pub plus_rect: Option<Rect>,
}

/// The column the inspector lays its rows into, and what the rows need to paint and to
/// act: the view's one brush, the text cache, the actions to run after painting.
pub struct Sheet<'a> {
    pub ui: &'a mut Ui,
    pub b: &'a mut Brush,
    pub fmt: &'a mut Fmt,
    pub acts: &'a mut Vec<Act>,
    pub menu: &'a mut Option<MenuOpen>,
    pub s: Style,
    /// The label column's width.
    pub col: f32,
}

impl<'a> Sheet<'a> {
    pub fn width(&self) -> f32 {
        self.ui.available_width()
    }

    /// A band across the whole panel, titled by `key`, in `f`'s hue; `power` adds its LED,
    /// `plus` a + key at its right, `count` a dim readout beside it.
    pub fn band(
        &mut self,
        key: &str,
        f: Function,
        power: Option<bool>,
        plus: bool,
        count: Option<Count<'_>>,
    ) -> BandClicks {
        self.ui.add_space(self.s.d(4.0));
        let w = self.width();
        let (rect, _) = self
            .ui
            .allocate_exact_size(Vec2::new(w, self.s.s(band::BAND)), Sense::hover());
        let s = self.s.function(f);
        band::section(self.b, rect, &s, State::REST, Src::Key(key), power, false);
        let mut out = BandClicks::default();
        if power.is_some() {
            let lr = Rect::from_center_size(
                pos2(rect.left() + s.s(13.0), rect.center().y),
                Vec2::splat(s.s(16.0)),
            );
            let r = self
                .ui
                .interact(lr, Id::new(("insp-band-power", key)), Sense::click());
            out.power = r.clicked();
            if r.hovered() {
                let name = self.fmt.tr(key);
                hint::set_text(self.ui.ctx(), name, "", Caps::NONE);
            }
        }
        let mut right = rect.right() - s.s(6.0);
        if plus {
            let pr = Rect::from_center_size(
                pos2(right - s.s(8.0), rect.center().y),
                Vec2::splat(s.s(16.0)),
            );
            let r = self
                .ui
                .interact(pr, Id::new(("insp-band-plus", key)), Sense::click());
            let st = State::of(&r, false);
            glyph_key(self.b, pr, &s, st, Glyph::Plus);
            out.plus = r.clicked();
            out.plus_rect = Some(pr);
            right = pr.left() - s.s(6.0);
        }
        if let Some(c) = count {
            let c = self.fmt.get(c.key, c.stamp, c.write);
            let t = Text::new(c, Role::Value, s.t.text_dim)
                .align(Align2::RIGHT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let r = Rect::from_min_max(
                pos2(right - s.s(56.0), rect.top()),
                pos2(right, rect.bottom()),
            );
            let _ = text::paint(self.b, r, &t, s.tk());
        }
        self.ui.add_space(self.s.d(4.0));
        out
    }

    /// Allocate one label | value row.
    pub fn row(&mut self) -> Row {
        let w = self.width();
        let (full, _) = self
            .ui
            .allocate_exact_size(Vec2::new(w, self.s.s(ROW)), Sense::hover());
        let m = self.s.s(MARGIN);
        let label = Rect::from_min_max(
            pos2(full.left() + m, full.top()),
            pos2(full.left() + m + self.col, full.bottom()),
        );
        let inset = self.s.s(WELL_INSET);
        let value = Rect::from_min_max(
            pos2(label.right() + self.s.s(GAP), full.top() + inset),
            pos2(full.right() - m, full.bottom() - inset),
        );
        Row { full, label, value }
    }

    /// A row's label: right-aligned, abbreviated when it does not fit, dimmed when the row
    /// does not apply.
    pub fn label(&mut self, r: Rect, key: &str, dim: bool) {
        let a = if dim { 0.45 } else { 1.0 };
        let t = Text::key(key, Role::Body, self.s.t.text_dim.gamma_multiply(a))
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Abbreviate);
        let _ = text::paint(self.b, r, &t, self.s.tk());
    }

    /// A row with a plain value: `text` in a well.
    pub fn text_row(&mut self, key: &str, text: &str, numeric: bool, dim: bool) -> Row {
        let row = self.row();
        self.label(row.label, key, dim);
        let st = State {
            disabled: dim,
            ..State::REST
        };
        well::readout(self.b, row.value, &self.s, st, text, numeric);
        row
    }

    /// A row whose value opens a list: a menu field. Opens (or closes) `menu` on click.
    pub fn menu_row(&mut self, key: &str, value: &str, menu: MenuId, dim: bool) -> Row {
        let row = self.row();
        self.label(row.label, key, dim);
        self.menu_field(row.value, value, menu, dim);
        row
    }

    /// A menu field in `rect` that toggles `menu`.
    pub fn menu_field(&mut self, rect: Rect, value: &str, menu: MenuId, dim: bool) -> Response {
        let id = Id::new(("insp-menu", menu));
        let r = self.ui.interact(rect, id, Sense::click());
        let open = self.menu.as_ref().is_some_and(|m| m.id == menu);
        if r.clicked() && !dim {
            *self.menu = if open {
                None
            } else {
                Some(MenuOpen {
                    id: menu,
                    anchor: rect,
                })
            };
        }
        let st = State {
            disabled: dim,
            ..State::of(&r, open)
        };
        if r.hovered() {
            hint::set_text(
                self.ui.ctx(),
                value,
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
        }
        well::menu_field(self.b, rect, &self.s, st, value, false);
        r
    }

    /// A field that opens the device picker for `purpose` (round 20): the gallery is the
    /// list. A right-click opens `menu`, the quick list, instead.
    pub fn pick_field(
        &mut self,
        rect: Rect,
        value: &str,
        purpose: crate::picker::PickPurpose,
        menu: MenuId,
        tip_key: &str,
    ) -> Response {
        let r = self
            .ui
            .interact(rect, Id::new(("insp-pick", menu)), Sense::click());
        let open = self.menu.as_ref().is_some_and(|m| m.id == menu);
        if r.clicked() {
            *self.menu = None;
            self.acts.push(Act::Pick(purpose));
        } else if r.secondary_clicked() {
            *self.menu = (!open).then_some(MenuOpen {
                id: menu,
                anchor: rect,
            });
        }
        if r.hovered() {
            let tip = crate::i18n::t(tip_key);
            hint::set_text(
                self.ui.ctx(),
                value,
                "",
                Caps {
                    menu: true,
                    ..Caps::NONE
                },
            );
            let _ = r.clone().on_hover_text(tip);
        }
        well::menu_field(self.b, rect, &self.s, State::of(&r, open), value, false);
        r
    }

    /// A row holding an on/off: an LED button across the value column. Returns a click.
    pub fn led_row(
        &mut self,
        key: &str,
        button_key: &str,
        on: bool,
        f: Function,
        dim: bool,
    ) -> bool {
        self.led_row_of((), key, button_key, on, f, dim)
    }

    /// [`Self::led_row`] for a row a panel draws once per item — a modulator's switches —
    /// its id salted by `owner` so two items' rows never share one (fix4: two modulators'
    /// *Sync* and *Restart* switches clashed).
    pub fn led_row_of(
        &mut self,
        owner: impl std::hash::Hash + std::fmt::Debug,
        key: &str,
        button_key: &str,
        on: bool,
        f: Function,
        dim: bool,
    ) -> bool {
        let row = self.row();
        self.label(row.label, key, dim);
        let w = row.value.width().min(self.s.s(110.0));
        let br = Rect::from_min_size(row.value.min, Vec2::new(w, row.value.height()));
        let r = self.ui.interact(
            br,
            Id::new(("insp-led", key, button_key, owner)),
            Sense::click(),
        );
        let st = State {
            disabled: dim,
            ..State::of(&r, on)
        };
        if r.hovered() {
            let name = self.fmt.tr(key);
            hint::set_text(self.ui.ctx(), name, "", Caps::NONE);
        }
        led::button(self.b, br, &self.s.function(f), st, Src::Key(button_key));
        r.clicked() && !dim
    }

    /// A value the pointer drags: its row's well with a level bar, formatted by `write`
    /// under `key` when it changes. Returns the drag.
    #[allow(clippy::too_many_arguments)]
    pub fn value(
        &mut self,
        rect: Rect,
        id: Id,
        v: f32,
        range: &Range,
        gain: bool,
        name: &str,
        write: impl FnOnce(f32, &mut String),
        f: Function,
        dim: bool,
    ) -> Drag {
        let d = drag(self.ui, id, rect, v, range, gain);
        let shown = d.value.unwrap_or(v);
        let text = self
            .fmt
            .get(cache::key(("insp-v", id)), bits(shown), |b| write(shown, b));
        let st = State {
            disabled: dim,
            ..State::of(&d.resp, false)
        };
        let s = self.s.function(f);
        let pos = if gain {
            gain_pos(shown)
        } else {
            range.pos(shown)
        };
        well::readout(self.b, rect, &s, st, text, true);
        level_bar(self.b, rect, &s, pos, range.bipolar, st.alpha());
        if d.resp.hovered() || d.resp.dragged() {
            hint::set(self.ui.ctx(), name, Caps::PARAM, d.resp.dragged(), |b| {
                b.write_str(text)
            });
        }
        d
    }

    /// A whole label | value row around [`Self::value`]; pushes the command `make` builds
    /// (merged under `merge`) when the value moves, and closes the gesture when it ends.
    #[allow(clippy::too_many_arguments)]
    pub fn param_row(
        &mut self,
        key: &'static str,
        id: Id,
        v: f32,
        range: &Range,
        gain: bool,
        write: impl FnOnce(f32, &mut String),
        f: Function,
        dim: bool,
        act: impl FnOnce(f32) -> Act,
    ) -> Row {
        let row = self.row();
        self.label(row.label, key, dim);
        let name = self.fmt.tr(key).to_owned_if_needed();
        let d = self.value(row.value, id, v, range, gain, &name, write, f, dim);
        if let Some(nv) = d.value {
            if !dim {
                self.acts.push(act(nv));
            }
        }
        if d.ended {
            self.acts.push(Act::EndGesture);
        }
        row
    }
}

/// A translated name borrowed from the cache for as long as the row needs it.
trait OwnedIfNeeded {
    fn to_owned_if_needed(&self) -> NameBuf;
}

/// Copy a short name onto the stack (see [`NameBuf`]).
pub fn name(s: &str) -> NameBuf {
    s.to_owned_if_needed()
}

/// A short name kept on the stack: hint names are one line, so 64 bytes hold them without
/// touching the heap; a longer one is cut at a character boundary.
pub struct NameBuf {
    buf: [u8; 64],
    len: usize,
}

impl std::ops::Deref for NameBuf {
    type Target = str;
    fn deref(&self) -> &str {
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl OwnedIfNeeded for str {
    fn to_owned_if_needed(&self) -> NameBuf {
        let mut end = self.len().min(64);
        while !self.is_char_boundary(end) {
            end -= 1;
        }
        let mut buf = [0u8; 64];
        buf[..end].copy_from_slice(&self.as_bytes()[..end]);
        NameBuf { buf, len: end }
    }
}
