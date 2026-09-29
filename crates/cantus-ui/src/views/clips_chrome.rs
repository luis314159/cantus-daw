//! The chrome the three editing views share — the arrangement, the piano roll and the
//! channel rack: their toolbar, their scroll bars, and how the wheel and the middle
//! button move them (ADR 0024 §4, §10).
//!
//! - **The wheel scrolls both ways.** egui turns Shift+wheel into a *horizontal* delta
//!   before any view sees it (`input_state/wheel_state.rs`), so a view reads `x` for
//!   time and `y` for rows and never asks about Shift. Asking — reading `y` when Shift was
//!   held — is what left Shift+wheel dead in all three views (`cantus-audit.md` #4).
//! - **Ctrl+wheel zooms time** at the pointer; **Ctrl+Alt+wheel zooms the rows**.
//! - **Middle-drag pans** in two dimensions, as in FL.
//! - **The scroll bars are always visible** and each is also a zoom: dragging either end
//!   of the thumb changes how much is shown (FL's playlist bar); double-clicking the bar
//!   shows everything.
//! - **The toolbar is kit parts only**: `ToolbarKey`s with the house icons, menu fields,
//!   LED buttons and readouts, laid out from both ends of one strip.

use crate::icons::{self, Icon};
use crate::kit::hint::{self, Caps};
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::{self, cap, chrome, glyph, led, well, Brush, Fill, State, Style, Tok};
use egui::{Align2, Color32, CursorIcon, Id, PointerButton, Pos2, Rect, Response, Sense, Ui, Vec2};

/// The toolbar's height at 100 %.
pub const TOOLBAR_H: f32 = 34.0;
/// A toolbar key's side.
pub const KEY: f32 = 26.0;
/// How thick the scroll bars are: FL's editors use a bar wide enough to grab an end of.
pub const BAR: f32 = 12.0;

/// The tokens the toolbar's ground paints with (for inspect mode).
const GROUND_TOKENS: &[Tok] = &[Tok::Toolbar, Tok::Light, Tok::Dark];

// ---- the wheel and the middle button ------------------------------------------------

/// What the wheel asked a view for this frame.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Wheel {
    /// How far the content should move, in points (egui's sign: positive moves it right
    /// or down). Shift+wheel arrives here as `x`.
    pub scroll: Vec2,
    /// Zoom of the time axis, multiplicative (1 is none): Ctrl+wheel.
    pub zoom_x: f32,
    /// Zoom of the rows: Ctrl+Alt+wheel.
    pub zoom_y: f32,
    /// Alt is held: the plain wheel belongs to the view's own gesture (velocity).
    pub alt: bool,
}

/// Read the wheel. Call it only for a view the pointer is over.
pub fn wheel(ui: &Ui) -> Wheel {
    ui.input(|i| {
        let z = i.zoom_delta_2d();
        let alt = i.modifiers.alt;
        Wheel {
            scroll: i.smooth_scroll_delta,
            zoom_x: if alt { 1.0 } else { z.x },
            zoom_y: if alt { z.y } else { 1.0 },
            alt,
        }
    })
}

/// Middle-drag over `resp`: how far the content should move, in points.
pub fn pan(ui: &Ui, resp: &Response) -> Vec2 {
    if resp.dragged_by(PointerButton::Middle) {
        ui.ctx().set_cursor_icon(CursorIcon::Grabbing);
        resp.drag_delta()
    } else {
        Vec2::ZERO
    }
}

// ---- the scroll bars ----------------------------------------------------------------

/// One axis of a view: what is shown of what there is, in the view's own units (beats,
/// rows, steps or points).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Span {
    /// The first unit shown.
    pub start: f32,
    /// How many units are shown.
    pub len: f32,
    /// How many there are; never less than `start + len`.
    pub total: f32,
}

impl Span {
    pub fn new(start: f32, len: f32, total: f32) -> Span {
        let len = len.max(1e-3);
        Span {
            start,
            len,
            total: total.max(start + len).max(len),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Grab {
    Thumb,
    Low,
    High,
}

/// A scroll bar in `rect` (vertical when taller than wide) that is also a zoom when
/// `zoom` gives the least and most units it may show. Returns the span it was moved to.
pub fn bar(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    span: Span,
    zoom: Option<(f32, f32)>,
    name_key: &'static str,
) -> Option<Span> {
    let s = Style::current();
    let vertical = rect.height() > rect.width();
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    let full = if vertical {
        rect.height()
    } else {
        rect.width()
    }
    .max(1.0);
    let frac = |v: f32| v / span.total.max(1e-6);
    let thumb = chrome::thumb(rect, &s, frac(span.start), frac(span.len));
    let along = |p: Pos2| if vertical { p.y } else { p.x };
    let (lo, hi) = if vertical {
        (thumb.top(), thumb.bottom())
    } else {
        (thumb.left(), thumb.right())
    };
    // The ends are the zoom handles: a fixed grab zone, never more than a third of the
    // thumb so a short thumb can still be dragged whole.
    let grip = s.s(7.0).min((hi - lo) / 3.0);
    let grab_at = |p: Pos2| -> Option<Grab> {
        let a = along(p);
        if a < lo || a > hi {
            return None;
        }
        Some(if zoom.is_some() && a < lo + grip {
            Grab::Low
        } else if zoom.is_some() && a > hi - grip {
            Grab::High
        } else {
            Grab::Thumb
        })
    };
    let mut out = None;
    let key = id.with("grab");
    if resp.drag_started_by(PointerButton::Primary) {
        if let Some(p) = ui.input(|i| i.pointer.press_origin()) {
            if let Some(g) = grab_at(p) {
                ui.data_mut(|d| d.insert_temp(key, (g, span.start, span.len)));
            }
        }
    }
    if resp.dragged_by(PointerButton::Primary) {
        let grabbed = ui.data(|d| d.get_temp::<(Grab, f32, f32)>(key));
        let origin = ui.input(|i| i.pointer.press_origin());
        if let (Some((g, start0, len0)), Some(o), Some(p)) =
            (grabbed, origin, resp.interact_pointer_pos())
        {
            let units = (along(p) - along(o)) / full * span.total;
            let end0 = start0 + len0;
            let (min, max) = zoom.unwrap_or((len0, len0));
            let moved = match g {
                Grab::Thumb => Span {
                    start: (start0 + units).clamp(0.0, (span.total - len0).max(0.0)),
                    len: len0,
                    total: span.total,
                },
                Grab::Low => {
                    let len = (end0 - (start0 + units).max(0.0)).clamp(min, max);
                    Span {
                        start: (end0 - len).max(0.0),
                        len,
                        total: span.total,
                    }
                }
                Grab::High => Span {
                    start: start0,
                    len: (len0 + units).clamp(min, max),
                    total: span.total,
                },
            };
            out = Some(moved);
        }
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<(Grab, f32, f32)>(key));
    }
    if resp.clicked() {
        // A click beside the thumb pages there: the view centres on the click.
        if let Some(p) = resp.interact_pointer_pos() {
            if grab_at(p).is_none() {
                let at = (along(p) - if vertical { rect.top() } else { rect.left() }) / full
                    * span.total;
                out = Some(Span {
                    start: (at - span.len * 0.5).clamp(0.0, (span.total - span.len).max(0.0)),
                    ..span
                });
            }
        }
    }
    if resp.double_clicked() {
        if let Some((min, max)) = zoom {
            out = Some(Span {
                start: 0.0,
                len: span.total.clamp(min, max),
                total: span.total,
            });
        }
    }
    let hovered_end = resp
        .hover_pos()
        .and_then(grab_at)
        .is_some_and(|g| g != Grab::Thumb);
    let zooming = hovered_end
        || ui
            .data(|d| d.get_temp::<(Grab, f32, f32)>(key))
            .is_some_and(|g| g.0 != Grab::Thumb);
    if zooming && (resp.hovered() || resp.dragged()) {
        ui.ctx().set_cursor_icon(if vertical {
            CursorIcon::ResizeVertical
        } else {
            CursorIcon::ResizeHorizontal
        });
    }
    if resp.hovered() {
        hint::set_text(ui.ctx(), word(name_key), "", Caps::NONE);
    }
    let shown = out.unwrap_or(span);
    let st = State::of(&resp, false);
    kit::with_brush(ui.painter(), |b| {
        chrome::scrollbar(b, rect, &s, st, frac(shown.start), frac(shown.len))
    });
    out
}

// ---- small things the painters share --------------------------------------------------

/// A translated word that lives as long as the program, for the names a view hands to a
/// widget every frame (a knob's hint): looking one up then costs no allocation. Each word
/// is leaked once per language it is read in — a handful of short strings.
pub fn word(key: &'static str) -> &'static str {
    use std::cell::RefCell;
    type Words = Vec<(&'static str, crate::i18n::Locale, &'static str)>;
    thread_local! {
        static WORDS: RefCell<Words> = const { RefCell::new(Vec::new()) };
    }
    let now = crate::i18n::locale();
    WORDS.with(|w| {
        let mut w = w.borrow_mut();
        if let Some((_, _, s)) = w.iter().find(|(k, l, _)| *k == key && *l == now) {
            return *s;
        }
        let s: &'static str = Box::leak(crate::i18n::t(key).into_boxed_str());
        w.push((key, now, s));
        s
    })
}

/// An integer written into a buffer on the stack: bar numbers and track numbers are
/// painted every frame, and `to_string` would allocate each one.
#[derive(Default, Debug)]
pub struct Digits([u8; 24]);

impl Digits {
    pub fn fmt(&mut self, n: i64) -> &str {
        let neg = n < 0;
        let mut v = n.unsigned_abs();
        let mut i = self.0.len();
        loop {
            i -= 1;
            self.0[i] = b'0' + (v % 10) as u8;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        if neg {
            i -= 1;
            self.0[i] = b'-';
        }
        std::str::from_utf8(&self.0[i..]).unwrap_or("")
    }
}

/// A track colour as the editors draw it (ADR 0024 §2): FL's pastel band for small
/// fills, the same hue darker for large ones, and text on each tinted toward the colour
/// (FL's rule 5), held at 4.5:1.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Tint {
    /// Colour cells, chips, notes.
    pub small: Color32,
    /// Headers, clip bars.
    pub large: Color32,
    pub on_small: Color32,
    pub on_large: Color32,
}

/// The tints of the colours a view has drawn, so the colour arithmetic (a contrast
/// search per colour) runs once per colour and theme rather than once per clip per frame.
#[derive(Debug, Default)]
pub struct Tints {
    generation: u64,
    dark: bool,
    known: Vec<(Color32, Tint)>,
}

impl Tints {
    pub fn of(&mut self, c: Color32, s: &Style) -> Tint {
        use crate::kit::color::{on_fill, safe_track, FillSize};
        if self.generation != s.t.generation || self.dark != s.t.dark {
            self.known.clear();
            self.generation = s.t.generation;
            self.dark = s.t.dark;
        }
        if let Some((_, t)) = self.known.iter().find(|(k, _)| *k == c) {
            return *t;
        }
        let small = safe_track(c, s.t.dark, FillSize::Small);
        let mut large = safe_track(c, s.t.dark, FillSize::Large);
        // FL's large fills on the dark theme sit at L 36–40 %, deep enough that every
        // one of them carries its name in a pale tint of itself; at the band's top a
        // yellow would flip to dark text and the headers would stop reading as one set.
        if s.t.dark {
            let mut h = crate::kit::color::Hsl::of(large);
            h.l = h.l.min(0.38);
            large = h.rgb();
        }
        let t = Tint {
            small,
            large,
            on_small: on_fill(small),
            on_large: tinted_text(large, s.t.dark),
        };
        if self.known.len() >= 64 {
            self.known.clear();
        }
        self.known.push((c, t));
        t
    }
}

/// Text on a large coloured fill: the fill's own hue, pale on a dark theme and deep on a
/// light one — decided by the theme rather than by the fill's luminance, so a row of
/// headers is one set — then pushed to 4.5:1.
fn tinted_text(fill: Color32, dark: bool) -> Color32 {
    use crate::kit::color::{ensure_contrast, Hsl};
    let h = Hsl::of(fill);
    let tint = Hsl {
        h: h.h,
        s: (h.s * 1.6).clamp(0.25, 0.8),
        l: if dark { 0.90 } else { 0.16 },
    }
    .rgb();
    ensure_contrast(tint, fill, 4.5)
}

/// Fill the corner where two bars meet.
pub fn corner(b: &mut Brush, rect: Rect, s: &Style) {
    b.register(rect, "corner", &[Tok::Panel], &[]);
    b.rect(b.snap_rect(rect), Fill::Solid(s.t.panel));
}

// ---- the toolbar ----------------------------------------------------------------------

/// A view's toolbar: one strip, laid out from both ends towards the middle, so the left
/// group (menu, title, tools) and the right group (readouts) never overlap — whatever does
/// not fit in what is left is not drawn rather than drawn over its neighbour.
pub struct Toolbar {
    pub rect: Rect,
    left: f32,
    right: f32,
    pub s: Style,
}

impl Toolbar {
    /// Take the strip at the top of `ui` and paint its ground.
    pub fn new(ui: &mut Ui) -> Toolbar {
        let s = Style::current();
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(w, s.s(TOOLBAR_H)), Sense::hover());
        kit::with_brush(ui.painter(), |b| {
            b.register(rect, "view_toolbar", GROUND_TOKENS, &[]);
            let r = b.snap_rect(rect);
            let t = &s.t;
            let base = t.toolbar;
            b.rect(
                r,
                Fill::V(
                    kit::color::shade(base, 0.02 * t.depth),
                    kit::color::shade(base, -0.02 * t.depth),
                ),
            );
            b.hline(
                r.left(),
                r.right(),
                r.top() + b.px() * 0.5,
                b.px(),
                kit::color::shade(base, 0.08 * t.depth),
            );
            b.hline(
                r.left(),
                r.right(),
                r.bottom() - b.px() * 0.5,
                b.px(),
                kit::color::shade(base, -0.22),
            );
        });
        Toolbar {
            rect,
            left: rect.left() + s.s(6.0),
            right: rect.right() - s.s(6.0),
            s,
        }
    }

    /// Room left between the two groups.
    pub fn room(&self) -> f32 {
        (self.right - self.left).max(0.0)
    }

    /// A rect `w` wide and `h` tall at the left end, or `None` when it no longer fits.
    pub fn take_left(&mut self, w: f32, h: f32) -> Option<Rect> {
        let w = self.s.s(w);
        if self.left + w > self.right {
            return None;
        }
        let r = Rect::from_min_size(
            Pos2::new(self.left, self.rect.center().y - self.s.s(h) * 0.5),
            Vec2::new(w, self.s.s(h)),
        );
        self.left += w + self.s.s(4.0);
        Some(r)
    }

    /// The same from the right end.
    pub fn take_right(&mut self, w: f32, h: f32) -> Option<Rect> {
        let w = self.s.s(w);
        if self.right - w < self.left {
            return None;
        }
        let r = Rect::from_min_size(
            Pos2::new(self.right - w, self.rect.center().y - self.s.s(h) * 0.5),
            Vec2::new(w, self.s.s(h)),
        );
        self.right -= w + self.s.s(4.0);
        Some(r)
    }

    /// A 1-px divider between groups on the left.
    pub fn divider(&mut self, ui: &Ui) {
        if let Some(r) = self.take_left(6.0, 20.0) {
            let s = self.s;
            kit::with_brush(ui.painter(), |b| {
                kit::band::divider(b, r.center().x, r.top(), r.bottom(), &s)
            });
        }
    }

    /// FL's ▸ at a window's top-left: the view's menu.
    pub fn menu(&mut self, ui: &mut Ui, id: Id, name_key: &'static str) -> Option<Response> {
        let r = self.take_left(18.0, 22.0)?;
        let resp = ui.interact(r, id, Sense::click());
        let ink = if resp.hovered() {
            self.s.t.text
        } else {
            self.s.t.text_dim
        };
        let s = self.s;
        kit::with_brush(ui.painter(), |b| {
            b.register(r, "menu_arrow", &[Tok::Text, Tok::TextDim], &[]);
            glyph::paint(b, r.shrink(s.s(3.0)), glyph::Glyph::Menu, ink)
        });
        if resp.hovered() {
            hint::set_text(ui.ctx(), word(name_key), "", Caps::NONE);
        }
        Some(resp)
    }

    /// The view's title: `title` in the title role, then `context` dimmed ("Motif · Keys").
    pub fn title(&mut self, ui: &Ui, title: Src<'_>, context: Option<&str>) {
        let s = self.s;
        let t = Text {
            src: title,
            role: Role::Title,
            caps: false,
            align: Align2::LEFT_CENTER,
            policy: Policy::Elide(Elide::End),
            color: s.t.text,
        };
        let w = text::measure(ui.ctx(), &t, s.tk()).x / s.k + 2.0;
        let ctx_w = context
            .map(|c| {
                let ct = Text::new(c, Role::Title, s.t.text_dim);
                text::measure(ui.ctx(), &ct, s.tk()).x / s.k + 10.0
            })
            .unwrap_or(0.0);
        // The title gives way to the tools: it takes what it needs, never more than a
        // third of the strip.
        let most = (self.room() / s.k / 3.0).max(40.0);
        let Some(r) = self.take_left((w + ctx_w).min(most), 22.0) else {
            return;
        };
        kit::with_brush(ui.painter(), |b| {
            b.register(r, "view_title", &[Tok::Text, Tok::TextDim], &[]);
            let painted = text::paint(b, r, &t, s.tk());
            if let (Some(c), Some(p)) = (context, painted) {
                let rest = Rect::from_min_max(Pos2::new(p.rect.right() + s.s(8.0), r.top()), r.max);
                if rest.width() > s.s(12.0) {
                    let ct = Text::new(c, Role::Title, s.t.text_dim)
                        .align(Align2::LEFT_CENTER)
                        .policy(Policy::Elide(Elide::Middle));
                    let _ = text::paint(b, rest, &ct, s.tk());
                }
            }
        });
    }

    /// A toolbar key with a house icon; `lit` is state (a tool chosen, a clip muted), never
    /// a press. `tip` builds the hint and the tooltip, and is only called while hovered.
    pub fn key(
        &mut self,
        ui: &mut Ui,
        id: Id,
        icon: Icon,
        lit: bool,
        enabled: bool,
        tip: impl FnOnce() -> String,
    ) -> Option<Response> {
        let r = self.take_left(KEY, KEY)?;
        Some(key_at(ui, r, id, icon, lit, enabled, tip, &self.s))
    }

    /// A key on the right end.
    pub fn key_right(
        &mut self,
        ui: &mut Ui,
        id: Id,
        icon: Icon,
        lit: bool,
        enabled: bool,
        tip: impl FnOnce() -> String,
    ) -> Option<Response> {
        let r = self.take_right(KEY, KEY)?;
        Some(key_at(ui, r, id, icon, lit, enabled, tip, &self.s))
    }

    /// A key that shows a colour — a clip's — instead of an icon: a picture of a colour
    /// says *which*, where a glyph could only say "colour". `None` draws the diagonal
    /// that means "the track's".
    pub fn swatch(
        &mut self,
        ui: &mut Ui,
        id: Id,
        color: Option<Color32>,
        tip: impl FnOnce() -> String,
    ) -> Option<Response> {
        let r = self.take_left(KEY, KEY)?;
        let s = self.s;
        let resp = sensed(ui, r, id, true);
        let st = State::of(&resp, false);
        kit::with_brush(ui.painter(), |b| {
            cap::toolbar_key(b, r, &s, st, cap::KeyFace::Text(""));
            let sw = r.shrink(s.s(7.0));
            match color {
                Some(c) => {
                    b.register(sw, "swatch", &[Tok::Edge], &[]);
                    b.rrect(sw, s.r(2.0), Fill::Solid(s.t.edge));
                    b.rrect(sw.shrink(b.px()), s.r(2.0), Fill::Solid(c));
                }
                None => {
                    b.register(sw, "swatch", &[Tok::Edge, Tok::TextDim], &[]);
                    b.rrect_stroke(sw, s.r(2.0), b.px(), s.t.edge);
                    let w = b.px() * 1.5;
                    b.segment(sw.left_bottom(), sw.right_top(), w, s.t.text_dim, false);
                }
            }
        });
        Some(tipped(ui, resp, tip))
    }

    /// A menu field (a well with the value and ▾) `w` wide, with a caption in its own
    /// cell before it — the left cell of a label | value pair, never a loose word.
    pub fn field(
        &mut self,
        ui: &mut Ui,
        id: Id,
        caption: Option<&str>,
        value: &str,
        w: f32,
        tip: impl FnOnce() -> String,
    ) -> Option<Response> {
        let s = self.s;
        let cap_w = caption
            .map(|c| {
                let t = Text::new(c, Role::Label, s.t.text_dim);
                text::measure(ui.ctx(), &t, s.tk()).x / s.k + 2.0
            })
            .unwrap_or(0.0);
        let need = text::measure(ui.ctx(), &Text::new(value, Role::Control, s.t.text), s.tk()).x
            / s.k
            + 30.0;
        let w = w.max(need);
        if self.room() < s.s(w + cap_w + 8.0) {
            return None;
        }
        if let (Some(c), Some(r)) = (caption, self.take_left(cap_w, 22.0)) {
            kit::with_brush(ui.painter(), |b| {
                b.register(r, "caption", &[Tok::TextDim], &[]);
                let t = Text::new(c, Role::Label, s.t.text_dim).align(Align2::RIGHT_CENTER);
                let _ = text::paint(b, r, &t, s.tk());
            });
        }
        let r = self.take_left(w, 22.0)?;
        let resp = sensed(ui, r, id, true);
        let st = State::of(&resp, false);
        kit::with_brush(ui.painter(), |b| {
            well::menu_field(b, r, &s, st, value, false)
        });
        Some(tipped(ui, resp, tip))
    }

    /// An on/off as an LED button with a caps word (Snap).
    pub fn led(
        &mut self,
        ui: &mut Ui,
        id: Id,
        on: bool,
        label_key: &str,
        w: f32,
        tip: impl FnOnce() -> String,
    ) -> Option<Response> {
        let r = self.take_left(w, 22.0)?;
        let s = self.s.hue_of(Tok::Led);
        let resp = sensed(ui, r, id, true);
        let st = State::of(&resp, on);
        kit::with_brush(ui.painter(), |b| {
            led::button(b, r, &s, st, Src::Key(label_key))
        });
        Some(tipped(ui, resp, tip))
    }

    /// A key with a short caps word on it (QUANTIZE), `w` wide.
    pub fn word(
        &mut self,
        ui: &mut Ui,
        id: Id,
        word: &str,
        w: f32,
        tip: impl FnOnce() -> String,
    ) -> Option<Response> {
        let r = self.take_left(w, 22.0)?;
        let s = self.s;
        let resp = sensed(ui, r, id, true);
        let st = State::of(&resp, false);
        kit::with_brush(ui.painter(), |b| {
            cap::toolbar_key(b, r, &s, st, cap::KeyFace::Text(word))
        });
        Some(tipped(ui, resp, tip))
    }

    /// A readout on the right end: a value in a well, numbers right-aligned.
    pub fn readout_right(&mut self, ui: &Ui, value: &str, w: f32, numeric: bool) {
        let s = self.s;
        // As wide as the value needs (Spanish runs long), never narrower than asked.
        let role = if numeric { Role::Value } else { Role::Body };
        let need =
            text::measure(ui.ctx(), &Text::new(value, role, s.t.text), s.tk()).x / s.k + 14.0;
        if let Some(r) = self.take_right(w.max(need), 22.0) {
            kit::with_brush(ui.painter(), |b| {
                well::readout(b, r, &s, State::REST, value, numeric)
            });
        }
    }
}

/// Sense a rect, greyed when not `enabled`.
fn sensed(ui: &mut Ui, r: Rect, id: Id, enabled: bool) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let mut resp = ui.interact(r, id, sense);
    if !enabled {
        resp.flags.remove(egui::response::Flags::ENABLED);
    }
    resp
}

/// Write the hint and the tooltip, building the text only while hovered.
fn tipped(ui: &Ui, resp: Response, tip: impl FnOnce() -> String) -> Response {
    if resp.hovered() {
        let text = tip();
        hint::set_text(ui.ctx(), &text, "", Caps::NONE);
        resp.on_hover_text(text)
    } else {
        resp
    }
}

#[allow(clippy::too_many_arguments)]
fn key_at(
    ui: &mut Ui,
    r: Rect,
    id: Id,
    icon: Icon,
    lit: bool,
    enabled: bool,
    tip: impl FnOnce() -> String,
    s: &Style,
) -> Response {
    let resp = sensed(ui, r, id, enabled);
    let st = State::of(&resp, lit);
    kit::with_brush(ui.painter(), |b| {
        cap::toolbar_key(b, r, s, st, cap::KeyFace::Text(""))
    });
    // The house icons are drawn over the key; they cut their gaps in the colour behind
    // them, which is the key's own face.
    let (ink, behind) = if lit {
        (s.t.highlight_glyph, s.t.highlight_band)
    } else if st.hovered {
        (s.t.text, s.t.toolbar)
    } else {
        (s.t.text_dim, s.t.toolbar)
    };
    let ink = ink.gamma_multiply(st.alpha());
    icons::paint_on(ui.painter(), r.shrink(s.s(6.0)), icon, ink, behind);
    tipped(ui, resp, tip)
}

/// The frame of a popup that holds kit menus and rows of their own (swatches, icons):
/// the menu's face, so everything in it sits on one sheet.
pub fn menu_frame() -> egui::Frame {
    let s = Style::current();
    egui::Frame::NONE
        .fill(s.t.menu)
        .corner_radius(egui::CornerRadius::same(s.r(3.0).round() as u8))
        .inner_margin(egui::Margin::same(3))
}
