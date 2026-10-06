//! The device shell (ADR 0024 §8): one frame for Cantus's effects, its instruments, the
//! generic editor of hosted plug-ins and the exported plug-ins' window (`FxHost`).
//!
//! - **Header, 28 pt**, which *is* the title bar: ▸ device menu and glyph · the name in
//!   the device hue · the context "(Insert 3 · Keys)" dimmed · a **centred preset well**
//!   (☆, the name — italic with a gold dot when modified — ▾, and ‹ › outside) · A|B ·
//!   undo/redo for this device · (instruments) Play | Edit | Matrix · the bypass lamp ·
//!   ? · ⋯ · ⤢. Dragging its background moves a floating window.
//! - **Body**: an optional display on top, then **cards** in a grid with 4-pt gutters on
//!   the device ground ([`Grid`]).
//! - **Footer, 24 pt**: the help line (fed by the kit's hints), chips, an output meter,
//!   a resize grip.
//! - **Scale `k`**: 75–200 %; the grip resizes freely and snaps to the nearest step on
//!   release.
//! - **Instrument bands** as ready-made cards: macros (up to 8 L knobs, 88 pt,
//!   collapsible), modulation chips (collapsible) sharing that row, and a 56-pt keyboard
//!   docked at the bottom when shown (K) — 148 pt with everything open, 21.8 % of the
//!   680-pt instrument frame.
//!
//! It is data-driven and knows nothing of the application: the caller hands in what the
//! header shows and gets back [`Event`]s (the egui way of callbacks), and fills the body.
//! Painters here are pure, like the rest of the kit; [`Shell::show`] is the widget.

use super::band::{self, CardLook, CardRects};
use super::brush::{Brush, Fill};
use super::color::{shade, with_alpha};
use super::glyph::{self, Glyph};
use super::knob::{self, KnobLook, KnobSize};
use super::light::{self, RaisedOpts};
use super::meter::{self, Levels};
use super::segmented;
use super::text::{self, Elide, Policy, Role, Src, Text};
use super::window_ctl::{self, WinHot, WindowCtl, WindowRects};
use super::{Function, State, Style, Tok};

/// The tokens the header paints with, beside the device hue (for inspect mode).
pub const HEADER_TOKENS: &[Tok] = &[
    Tok::Panel,
    Tok::Text,
    Tok::TextDim,
    Tok::Well,
    Tok::Brand,
    Tok::Placeholder,
];
/// The tokens the footer paints with.
pub const FOOTER_TOKENS: &[Tok] = &[
    Tok::Panel,
    Tok::TextDim,
    Tok::MeterWell,
    Tok::MeterLow,
    Tok::MeterHigh,
    Tok::MeterClip,
];
/// The device ground between the cards.
pub const GROUND_TOKENS: &[Tok] = &[Tok::DeviceGround];
/// The keyboard card.
pub const KEYBOARD_TOKENS: &[Tok] = &[Tok::Deck, Tok::KeyWhite, Tok::KeyBlack];
use egui::{Align2, Pos2, Rect, Vec2};

/// The header's height at 100 %.
pub const HEADER: f32 = 28.0;
/// The footer's height at 100 %.
pub const FOOTER: f32 = 24.0;
/// The window scales a device offers (ADR 0024 §8).
pub const SCALES: [f32; 7] = [0.75, 0.9, 1.0, 1.25, 1.5, 1.75, 2.0];
/// The preset well's width at 100 %.
pub const PRESET_W: f32 = 240.0;
/// Designed sizes at 100 % (`plugin-instrument-uis.md` §7.11).
pub const EFFECT: Vec2 = Vec2::new(880.0, 560.0);
pub const DISPLAY_EFFECT: Vec2 = Vec2::new(1040.0, 640.0);
pub const INSTRUMENT: Vec2 = Vec2::new(1040.0, 680.0);

/// The scale step nearest to `k`.
pub fn snap_scale(k: f32) -> f32 {
    SCALES
        .iter()
        .copied()
        .min_by(|a, b| (a - k).abs().total_cmp(&(b - k).abs()))
        .unwrap_or(1.0)
}

/// A/B compare slot.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ab {
    A,
    B,
}

/// The preset well.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Preset<'a> {
    pub name: &'a str,
    pub modified: bool,
    pub favourite: bool,
}

/// What the header shows. Every optional part is left out when `None`.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Header<'a> {
    pub name: &'a str,
    /// "(Insert 3 · Keys)", dimmed.
    pub context: &'a str,
    /// The device hue: the hue of its main function (Aequalis tone, Comprimo dynamics…).
    pub hue: Function,
    pub preset: Option<Preset<'a>>,
    pub ab: Option<Ab>,
    /// (can undo, can redo), the host history filtered to this device.
    pub undo: Option<(bool, bool)>,
    /// Play | Edit | Matrix (instruments): the labels and the selected one.
    pub views: Option<(&'a [Src<'a>], usize)>,
    /// Bypass: `Some(true)` when the device is bypassed; the lamp is lit when it runs.
    pub bypassed: Option<bool>,
    pub help: bool,
    /// The window's own buttons (round 20): swap, minimise, maximise / restore, close.
    /// `None` where the host owns the window (the exported plug-ins').
    pub window: Option<WindowCtl>,
}

/// What the footer shows.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Footer<'a> {
    /// Pre-formatted chips: "Latency 2.0 ms", "Voices 5/8", "CPU 3.1 %".
    pub chips: &'a [&'a str],
    /// The output meter.
    pub meter: Option<Levels>,
}

/// Something the user did to the shell.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Event {
    Menu,
    PresetPrev,
    PresetNext,
    PresetBrowse,
    Favourite,
    Ab(Ab),
    Undo,
    Redo,
    View(usize),
    Bypass,
    Help,
    More,
    /// The size menu, or a grip drag that ended: the new scale.
    Scale(f32),
    SizeMenu,
    /// The header's background was dragged: move the window by this much.
    Move(Vec2),
    /// The window's buttons (round 20): swap the device for another, minimise it to the
    /// device tray, maximise or restore it, close it.
    Swap,
    Minimise,
    Maximise,
    Close,
}

/// Where each part of the header is; `None` for the parts it does not show.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct HeaderRects {
    pub menu: Rect,
    pub name: Rect,
    pub preset: Option<Rect>,
    pub preset_prev: Option<Rect>,
    pub preset_next: Option<Rect>,
    pub star: Option<Rect>,
    pub ab: Option<Rect>,
    pub undo: Option<Rect>,
    pub redo: Option<Rect>,
    pub views: Option<Rect>,
    pub bypass: Option<Rect>,
    pub help: Option<Rect>,
    pub more: Rect,
    pub size: Rect,
    /// The window's own buttons, when the header shows them.
    pub window: Option<WindowRects>,
}

/// A hovered header part, for highlighting.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Hot {
    #[default]
    None,
    Menu,
    Preset,
    PresetPrev,
    PresetNext,
    Star,
    Ab,
    Undo,
    Redo,
    Views,
    Bypass,
    Help,
    More,
    Size,
    /// One of the window's buttons, and whether it is held down.
    Window(WinHot, bool),
}

fn sq(c: Pos2, side: f32) -> Rect {
    Rect::from_center_size(c, Vec2::splat(side))
}

/// Lay the header out in `rect`: the window's buttons at the right end, tools packed
/// from there, the preset well centred in what is left between the name and the tools.
/// When the name would be squeezed, the swap cap gives up its word first.
pub fn header_rects(rect: Rect, s: &Style, h: &Header<'_>) -> HeaderRects {
    let r = header_rects_with(rect, s, h, false);
    if h.window.is_some_and(|c| c.swap.is_some()) && r.name.width() < 110.0 * s.k {
        return header_rects_with(rect, s, h, true);
    }
    r
}

fn header_rects_with(rect: Rect, s: &Style, h: &Header<'_>, compact: bool) -> HeaderRects {
    let cy = rect.center().y;
    let k = s.k;
    let icon = 22.0 * k;
    let mut r = HeaderRects {
        menu: sq(Pos2::new(rect.left() + 14.0 * k, cy), 18.0 * k),
        name: Rect::NOTHING,
        preset: None,
        preset_prev: None,
        preset_next: None,
        star: None,
        ab: None,
        undo: None,
        redo: None,
        views: None,
        bypass: None,
        help: None,
        more: Rect::NOTHING,
        size: Rect::NOTHING,
        window: None,
    };
    let mut x = rect.right() - 6.0 * k;
    if let Some(c) = &h.window {
        let w = window_ctl::layout(rect, s, c, compact);
        x = w.left - 6.0 * k;
        r.window = Some(w);
    }
    let mut take = |w: f32| {
        let at = Rect::from_min_max(Pos2::new(x - w, rect.top()), Pos2::new(x, rect.bottom()));
        x -= w;
        at
    };
    r.size = sq(take(icon).center(), icon);
    r.more = sq(take(icon).center(), icon);
    if h.help {
        r.help = Some(sq(take(icon).center(), icon));
    }
    if h.bypassed.is_some() {
        r.bypass = Some(sq(take(icon + 4.0 * k).center(), icon));
    }
    if let Some((labels, _)) = h.views {
        let _ = take(6.0 * k);
        let w = 58.0 * k * labels.len() as f32;
        let at = take(w);
        r.views = Some(Rect::from_center_size(at.center(), Vec2::new(w, 22.0 * k)));
    }
    if h.undo.is_some() {
        let _ = take(6.0 * k);
        r.redo = Some(sq(take(icon).center(), icon));
        r.undo = Some(sq(take(icon).center(), icon));
    }
    if h.ab.is_some() {
        let _ = take(6.0 * k);
        let at = take(52.0 * k);
        r.ab = Some(Rect::from_center_size(
            at.center(),
            Vec2::new(48.0 * k, 22.0 * k),
        ));
    }
    let tools_left = x - 8.0 * k;
    let name_left = r.menu.right() + 22.0 * k;
    r.name = Rect::from_min_max(
        Pos2::new(name_left, rect.top()),
        Pos2::new(tools_left, rect.bottom()),
    );
    if h.preset.is_some() {
        let w = (PRESET_W * k).min((tools_left - name_left - 150.0 * k).max(120.0 * k));
        let step = 14.0 * k;
        // Centred on the window, pushed right when the name needs the room.
        let cx = rect.center().x.max(name_left + 150.0 * k + step + w * 0.5);
        let cx = cx.min(tools_left - step - w * 0.5);
        let well = Rect::from_center_size(Pos2::new(cx, cy), Vec2::new(w, 22.0 * k));
        r.preset_prev = Some(sq(Pos2::new(well.left() - step * 0.5 - 2.0 * k, cy), step));
        r.preset_next = Some(sq(Pos2::new(well.right() + step * 0.5 + 2.0 * k, cy), step));
        r.star = Some(sq(Pos2::new(well.left() + 11.0 * k, cy), 14.0 * k));
        r.name = Rect::from_min_max(
            r.name.min,
            Pos2::new(well.left() - step - 8.0 * k, rect.bottom()),
        );
        r.preset = Some(well);
    }
    r
}

/// Paint the header in `rect`; `hot` is the part under the pointer.
pub fn paint_header(b: &mut Brush, rect: Rect, s: &Style, h: &Header<'_>, hot: Hot) -> HeaderRects {
    b.register(rect, "shell_header", HEADER_TOKENS, &[Some(h.hue.tok())]);
    let t = &s.t;
    let rect = b.snap_rect(rect);
    let hue = t.hue(h.hue);
    let k = s.k;
    // The bar: the panel colour, lit from above, a hairline below.
    b.rect(rect, Fill::V(shade(t.panel, 0.025), shade(t.panel, -0.015)));
    b.hline(
        rect.left(),
        rect.right(),
        rect.top() + b.px() * 0.5,
        b.px(),
        shade(t.panel, 0.07),
    );
    b.hline(
        rect.left(),
        rect.right(),
        rect.bottom() - b.px() * 0.5,
        b.px(),
        shade(t.panel, -0.14),
    );
    let r = header_rects(rect, s, h);
    let ink = |on: bool| if on { t.text } else { t.text_dim };

    glyph::paint(
        b,
        r.menu.shrink(3.0 * k),
        Glyph::Menu,
        ink(hot == Hot::Menu),
    );
    let mark = sq(
        Pos2::new(r.menu.right() + 10.0 * k, rect.center().y),
        16.0 * k,
    );
    glyph::paint(b, mark, Glyph::Device, hue);

    // The name in the device hue, then the dimmed context, measured so they never touch.
    let name = Text::new(h.name, Role::Title, hue)
        .caps(true)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let name = Text {
        role: Role::Section,
        ..name
    };
    let nw = text::measure(b.ctx(), &name, s.tk()).x.min(r.name.width());
    let _ = text::paint(
        b,
        Rect::from_min_size(r.name.min, Vec2::new(nw + 1.0, r.name.height())),
        &name,
        k,
    );
    if !h.context.is_empty() {
        let cr = Rect::from_min_max(
            Pos2::new(r.name.left() + nw + 6.0 * k, r.name.top()),
            r.name.max,
        );
        let ctx_t = Text::new(h.context, Role::Title, t.text_dim)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::Middle));
        let _ = text::paint(b, cr, &ctx_t, s.tk());
    }

    if let (Some(p), Some(well)) = (h.preset, r.preset) {
        let st = State {
            hovered: hot == Hot::Preset,
            ..State::REST
        };
        super::well::paint(b, well, s, st, None);
        if let Some(star) = r.star {
            let g = if p.favourite {
                Glyph::StarFilled
            } else {
                Glyph::Star
            };
            let c = if p.favourite {
                t.brand
            } else {
                ink(hot == Hot::Star)
            };
            glyph::paint(b, star, g, c);
        }
        let down = sq(
            Pos2::new(well.right() - 11.0 * k, well.center().y),
            10.0 * k,
        );
        glyph::paint(b, down, Glyph::Down, ink(hot == Hot::Preset));
        let mut tr = Rect::from_min_max(
            Pos2::new(well.left() + 22.0 * k, well.top()),
            Pos2::new(down.left() - 6.0 * k, well.bottom()),
        );
        let role = if p.modified {
            Role::Placeholder
        } else {
            Role::Body
        };
        let pt = Text::new(p.name, role, t.text)
            .align(Align2::CENTER_CENTER)
            .policy(Policy::Elide(Elide::End));
        if p.modified {
            // The gold dot of a modified preset, after the (italic) name.
            let w = text::measure(b.ctx(), &pt, s.tk())
                .x
                .min(tr.width() - 10.0 * k);
            let dot = Pos2::new(tr.center().x + w * 0.5 + 6.0 * k, well.center().y);
            b.circle(dot, 2.6 * k, Fill::Solid(t.brand));
            tr.max.x -= 10.0 * k;
        }
        let _ = text::paint(b, tr, &pt, s.tk());
        for (g, rr, hh) in [
            (Glyph::Prev, r.preset_prev, Hot::PresetPrev),
            (Glyph::Next, r.preset_next, Hot::PresetNext),
        ] {
            if let Some(rr) = rr {
                glyph::paint(b, rr.shrink(1.0 * k), g, ink(hot == hh));
            }
        }
    }
    if let (Some(ab), Some(rr)) = (h.ab, r.ab) {
        let labels = [Src::Key("shell.ab.a"), Src::Key("shell.ab.b")];
        let st = State {
            hovered: hot == Hot::Ab,
            ..State::REST
        };
        segmented::paint(
            b,
            rr,
            &s.hue_of(Tok::Brand),
            st,
            &labels,
            (ab == Ab::B) as usize,
            None,
        );
    }
    if let (Some((can_undo, can_redo)), Some(u), Some(rd)) = (h.undo, r.undo, r.redo) {
        let c = |on: bool, hov: bool| {
            if !on {
                with_alpha(t.text_dim, 0.4)
            } else {
                ink(hov)
            }
        };
        glyph::paint(
            b,
            u.shrink(4.0 * k),
            Glyph::Undo,
            c(can_undo, hot == Hot::Undo),
        );
        glyph::paint(
            b,
            rd.shrink(4.0 * k),
            Glyph::Redo,
            c(can_redo, hot == Hot::Redo),
        );
    }
    if let (Some((labels, sel)), Some(vr)) = (h.views, r.views) {
        let st = State {
            hovered: hot == Hot::Views,
            ..State::REST
        };
        segmented::paint(b, vr, &s.function(h.hue), st, labels, sel, None);
    }
    if let (Some(bypassed), Some(br)) = (h.bypassed, r.bypass) {
        let running = !bypassed;
        if running {
            light::glow(b, br.center(), 12.0 * k, hue, 0.55);
        }
        let c = if running {
            hue
        } else {
            with_alpha(t.text_dim, 0.7)
        };
        glyph::paint(b, br.shrink(3.0 * k), Glyph::Power, c);
    }
    if let Some(hr) = r.help {
        glyph::paint(b, hr.shrink(4.0 * k), Glyph::Help, ink(hot == Hot::Help));
    }
    glyph::paint(
        b,
        r.more.shrink(4.0 * k),
        Glyph::More,
        ink(hot == Hot::More),
    );
    glyph::paint(
        b,
        r.size.shrink(5.0 * k),
        Glyph::Resize,
        ink(hot == Hot::Size),
    );
    if let (Some(c), Some(w)) = (&h.window, &r.window) {
        let (wh, down) = match hot {
            Hot::Window(wh, down) => (wh, down),
            _ => (WinHot::None, false),
        };
        window_ctl::paint(b, rect, s, c, w, wh, down);
    }
    r
}

/// Where the footer's parts are.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FooterRects {
    pub help: Rect,
    pub meter: Option<Rect>,
    pub grip: Rect,
}

/// Paint the footer: `help` is the hovered control's hint as one line ("Cutoff · 2.4
/// kHz"), or the idle line; chips and the meter at the right; the grip in the corner.
pub fn paint_footer(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    f: &Footer<'_>,
    help: &str,
) -> FooterRects {
    b.register(rect, "shell_footer", FOOTER_TOKENS, &[]);
    let t = &s.t;
    let k = s.k;
    let rect = b.snap_rect(rect);
    b.rect(rect, Fill::V(shade(t.panel, 0.01), shade(t.panel, -0.02)));
    b.hline(
        rect.left(),
        rect.right(),
        rect.top() + b.px() * 0.5,
        b.px(),
        shade(t.panel, -0.14),
    );
    let grip = sq(
        Pos2::new(rect.right() - 9.0 * k, rect.center().y + 2.0 * k),
        12.0 * k,
    );
    glyph::paint(b, grip, Glyph::Grip, t.text_dim);
    let mut x = grip.left() - 8.0 * k;
    let mut meter_rect = None;
    if let Some(levels) = f.meter {
        // A horizontal output meter: the vertical meter's colours, laid on its side.
        let mr = Rect::from_min_max(
            Pos2::new(x - 64.0 * k, rect.center().y - 4.0 * k),
            Pos2::new(x, rect.center().y + 4.0 * k),
        );
        horizontal_meter(b, mr, s, &levels);
        meter_rect = Some(mr);
        let out = Rect::from_min_max(
            Pos2::new(mr.left() - 30.0 * k, rect.top()),
            Pos2::new(mr.left() - 4.0 * k, rect.bottom()),
        );
        let _ = text::paint(
            b,
            out,
            &Text::key("shell.out", Role::Micro, t.text_dim).align(Align2::RIGHT_CENTER),
            k,
        );
        x = out.left() - 10.0 * k;
    }
    for chip in f.chips.iter().rev() {
        let w = text::measure(b.ctx(), &Text::new(chip, Role::Micro, t.text), s.tk()).x + 12.0 * k;
        let cr = Rect::from_min_max(
            Pos2::new(x - w, rect.center().y - 8.0 * k),
            Pos2::new(x, rect.center().y + 8.0 * k),
        );
        b.rrect(
            b.snap_rect(cr),
            3.0 * k,
            Fill::Solid(shade(t.panel, if t.dark { -0.06 } else { -0.05 })),
        );
        let _ = text::paint(
            b,
            cr,
            &Text::new(chip, Role::Micro, t.text_dim).caps(false),
            k,
        );
        x = cr.left() - 6.0 * k;
    }
    let hr = Rect::from_min_max(
        Pos2::new(rect.left() + 10.0 * k, rect.top()),
        Pos2::new(x - 8.0 * k, rect.bottom()),
    );
    let ht = Text::new(help, Role::Body, t.text_dim)
        .align(Align2::LEFT_CENTER)
        .policy(Policy::Elide(Elide::End));
    let _ = text::paint(b, hr, &ht, s.tk());
    FooterRects {
        help: hr,
        meter: meter_rect,
        grip,
    }
}

/// A two-lane meter laid horizontally (the footer's output meter).
pub fn horizontal_meter(b: &mut Brush, rect: Rect, s: &Style, levels: &Levels) {
    let t = &s.t;
    let rect = b.snap_rect(rect);
    light::sunken(b, rect, s.r(1.0), t.meter_well, 1.0, t.depth);
    let inner = rect.shrink(b.px() * 1.5);
    let lane_h = ((inner.height() - b.px()) * 0.5).max(b.px());
    for (i, lv) in levels.level.iter().enumerate() {
        let y0 = inner.top() + i as f32 * (lane_h + b.px());
        let w = inner.width() * meter::pos_of_db(*lv);
        let x6 = inner.left() + inner.width() * meter::pos_of_db(-6.0);
        let x0 = inner.left() + inner.width() * meter::pos_of_db(0.0);
        let right = inner.left() + w;
        let lane = |a: f32, z: f32| Rect::from_min_max(Pos2::new(a, y0), Pos2::new(z, y0 + lane_h));
        if right > inner.left() {
            b.rect(
                b.snap_rect(lane(inner.left(), right.min(x6))),
                Fill::Solid(t.meter_low),
            );
        }
        if right > x6 {
            b.rect(
                b.snap_rect(lane(x6, right.min(x0))),
                Fill::Solid(t.meter_high),
            );
        }
        if right > x0 {
            b.rect(b.snap_rect(lane(x0, right)), Fill::Solid(t.meter_clip));
        }
    }
}

/// Lays cards out on the device ground: 8-pt padding, 4-pt gutters.
#[derive(Copy, Clone, Debug)]
pub struct Grid {
    area: Rect,
    gutter: f32,
    y: f32,
}

impl Grid {
    pub fn new(body: Rect, s: &Style) -> Grid {
        let area = body.shrink(s.d(8.0));
        Grid {
            area,
            gutter: s.d(4.0),
            y: area.top(),
        }
    }

    /// A full-width band `h` points tall (a display, a card).
    pub fn full(&mut self, h: f32) -> Rect {
        let r = Rect::from_min_size(
            Pos2::new(self.area.left(), self.y),
            Vec2::new(self.area.width(), h),
        );
        self.y += h + self.gutter;
        r
    }

    /// A row `h` tall split by `fracs` (which sum to 1), with gutters between.
    pub fn row<'f>(&mut self, h: f32, fracs: &'f [f32]) -> impl Iterator<Item = Rect> + 'f {
        let (left, top, gutter) = (self.area.left(), self.y, self.gutter);
        let w = self.area.width() - gutter * (fracs.len().saturating_sub(1)) as f32;
        self.y += h + gutter;
        let mut x = left;
        fracs.iter().map(move |f| {
            let r = Rect::from_min_size(Pos2::new(x, top), Vec2::new(w * f, h));
            x += w * f + gutter;
            r
        })
    }

    /// What is left below the rows so far.
    pub fn rest(&self) -> Rect {
        Rect::from_min_max(Pos2::new(self.area.left(), self.y), self.area.max)
    }

    /// Everything from `h` points above the bottom: for bands docked at the bottom.
    pub fn bottom(&mut self, h: f32) -> Rect {
        let r = Rect::from_min_max(
            Pos2::new(self.area.left(), self.area.bottom() - h),
            self.area.max,
        );
        self.area.max.y -= h + self.gutter;
        r
    }
}

/// The device ground behind the cards.
pub fn paint_ground(b: &mut Brush, rect: Rect, s: &Style) {
    b.register(rect, "device_ground", GROUND_TOKENS, &[]);
    let t = &s.t;
    b.rect(
        b.snap_rect(rect),
        Fill::V(t.device_ground, shade(t.device_ground, -0.015)),
    );
}

// ---- instrument bands ----------------------------------------------------------------

/// The macro card's height, open, at 100 %.
pub const MACROS: f32 = 88.0;
/// The keyboard card's height at 100 %.
pub const KEYBOARD: f32 = 56.0;
/// A collapsed band: its title bar.
pub const COLLAPSED: f32 = 24.0;

/// Which bands an instrument shows, and how.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Bands {
    pub macros_open: bool,
    pub mods_open: bool,
    /// The keyboard (K); hidden by default except in Play view.
    pub keyboard: bool,
}

/// The height the bands take at 100 %: macros and modulation share one row (the chips
/// sit in a card beside the macros), the keyboard docks below.
pub fn bands_height(b: &Bands) -> f32 {
    let row = if b.macros_open || b.mods_open {
        MACROS
    } else {
        COLLAPSED
    };
    row + if b.keyboard { 4.0 + KEYBOARD } else { 0.0 }
}

/// Where the bands go at the bottom of the body.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct BandRects {
    pub macros: Rect,
    pub mods: Rect,
    pub keyboard: Option<Rect>,
}

/// Take the bands off the bottom of `grid`.
pub fn layout_bands(grid: &mut Grid, s: &Style, b: &Bands) -> BandRects {
    let keyboard = b.keyboard.then(|| grid.bottom(KEYBOARD * s.k));
    let row_h = if b.macros_open || b.mods_open {
        MACROS
    } else {
        COLLAPSED
    } * s.k;
    let row = grid.bottom(row_h);
    let split = row.left() + (row.width() - 4.0 * s.k) * 0.68;
    BandRects {
        macros: Rect::from_min_max(row.min, Pos2::new(split, row.bottom())),
        mods: Rect::from_min_max(Pos2::new(split + 4.0 * s.k, row.top()), row.max),
        keyboard,
    }
}

/// One macro: its name, position (0…1) and readout.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Macro<'a> {
    pub label: Src<'a>,
    pub value: f32,
    pub text: &'a str,
}

/// The macro card: up to eight L knobs, each with its name and value beside it.
pub fn macro_card(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    macros: &[Macro<'_>],
    open: bool,
) -> CardRects {
    let s = &s.function(Function::Mod);
    let look = CardLook {
        title: Src::Key("shell.macros"),
        switchable: false,
        selector: None,
        menu: true,
        display: None,
    };
    let cr = band::card(b, rect, s, State::LIT, &look);
    if !open || macros.is_empty() {
        return cr;
    }
    let k = s.k;
    let body = Rect::from_min_max(
        Pos2::new(rect.left() + 6.0 * k, cr.title.bottom() + 2.0 * k),
        Pos2::new(rect.right() - 6.0 * k, rect.bottom() - 2.0 * k),
    );
    let n = macros.len().min(8);
    let w = body.width() / n as f32;
    for (i, m) in macros.iter().take(n).enumerate() {
        let cell = Rect::from_min_size(
            Pos2::new(body.left() + w * i as f32, body.top()),
            Vec2::new(w, body.height()),
        );
        let d = KnobSize::L.diameter() * k;
        let c = Pos2::new(cell.left() + d * 0.5 + 4.0 * k, cell.center().y);
        knob::paint(
            b,
            c,
            s,
            State::REST,
            &KnobLook::new(KnobSize::L, m.value, 0.0),
        );
        let tx = c.x + d * 0.5 + 6.0 * k;
        let lr = Rect::from_min_max(
            Pos2::new(tx, c.y - 16.0 * k),
            Pos2::new(cell.right() - 4.0 * k, c.y),
        );
        let vr = Rect::from_min_max(
            Pos2::new(tx, c.y),
            Pos2::new(cell.right() - 4.0 * k, c.y + 16.0 * k),
        );
        let lt = Text {
            src: m.label,
            role: Role::Label,
            caps: true,
            align: Align2::LEFT_CENTER,
            policy: Policy::Abbreviate,
            color: s.t.label,
        };
        let _ = text::paint(b, lr, &lt, s.tk());
        let vt = Text::new(m.text, Role::Value, s.t.text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, vr, &vt, s.tk());
    }
    cr
}

/// A modulation source chip: the family hue as a 3-pt bar, the tag, a count.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Chip<'a> {
    pub tag: &'a str,
    pub family: Function,
    /// How many targets it drives.
    pub count: u8,
}

/// The modulation card: source chips in rows.
pub fn mod_card(b: &mut Brush, rect: Rect, s: &Style, chips: &[Chip<'_>], open: bool) -> CardRects {
    let s = &s.function(Function::Mod);
    let look = CardLook {
        title: Src::Key("shell.modulation"),
        switchable: false,
        selector: None,
        menu: true,
        display: None,
    };
    let cr = band::card(b, rect, s, State::LIT, &look);
    if !open {
        return cr;
    }
    let k = s.k;
    let t = &s.t;
    let (cw, ch) = (58.0 * k, 20.0 * k);
    let mut at = Pos2::new(rect.left() + 8.0 * k, cr.title.bottom() + 8.0 * k);
    for c in chips {
        if at.x + cw > rect.right() - 8.0 * k {
            at = Pos2::new(rect.left() + 8.0 * k, at.y + ch + 6.0 * k);
        }
        if at.y + ch > rect.bottom() - 4.0 * k {
            break;
        }
        let r = b.snap_rect(Rect::from_min_size(at, Vec2::new(cw, ch)));
        light::raised(
            b,
            r,
            3.0 * k,
            t.deck_title,
            RaisedOpts {
                k,
                depth: s.t.depth,
                ..Default::default()
            },
        );
        b.rect(
            b.snap_rect(Rect::from_min_size(
                r.min + Vec2::splat(b.px()),
                Vec2::new(3.0 * k, ch - 2.0 * b.px()),
            )),
            Fill::Solid(t.hue(c.family)),
        );
        let tr = Rect::from_min_max(
            Pos2::new(r.left() + 7.0 * k, r.top()),
            Pos2::new(r.right() - 14.0 * k, r.bottom()),
        );
        let _ = text::paint(
            b,
            tr,
            &Text::new(c.tag, Role::Label, t.text)
                .caps(true)
                .align(Align2::LEFT_CENTER),
            k,
        );
        if c.count > 0 {
            let dot = Pos2::new(r.right() - 8.0 * k, r.center().y);
            b.circle(
                dot,
                5.0 * k,
                Fill::Solid(shade(t.deck_title, if t.dark { -0.08 } else { -0.1 })),
            );
            let digits = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
            let d = digits[(c.count as usize).min(9)];
            let _ = text::paint(
                b,
                sq(dot, 10.0 * k),
                &Text::new(d, Role::Micro, t.text_dim),
                k,
            );
        }
        at.x += cw + 6.0 * k;
    }
    cr
}

/// The keyboard card: `lo`…`hi` (MIDI notes, whole octaves), `held` lit in the hue.
pub fn keyboard_card(b: &mut Brush, rect: Rect, s: &Style, lo: u8, hi: u8, held: &[u8]) {
    b.register(rect, "keyboard", KEYBOARD_TOKENS, &[s.hue_tok]);
    let t = &s.t;
    let k = s.k;
    let rect = b.snap_rect(rect);
    b.rrect(rect, 6.0 * k, Fill::Solid(t.deck));
    let keys = rect.shrink(4.0 * k);
    let is_black = |n: u8| matches!(n % 12, 1 | 3 | 6 | 8 | 10);
    let whites = (lo..=hi).filter(|n| !is_black(*n)).count().max(1);
    let ww = keys.width() / whites as f32;
    let (white, black) = (t.key_white, t.key_black);
    let mut x = keys.left();
    for n in lo..=hi {
        if is_black(n) {
            continue;
        }
        let r = b.snap_rect(Rect::from_min_max(
            Pos2::new(x, keys.top()),
            Pos2::new(x + ww, keys.bottom()),
        ));
        let lit = held.contains(&n);
        let base = if lit { s.hue } else { white };
        b.rect(r, Fill::Solid(shade(base, -0.28)));
        let face = Rect::from_min_max(r.min, Pos2::new(r.right() - b.px(), r.bottom() - b.px()));
        b.rrect(
            face,
            2.0 * k,
            Fill::V(shade(base, -0.06), shade(base, 0.03)),
        );
        x += ww;
    }
    let mut x = keys.left();
    for n in lo..=hi {
        if !is_black(n) {
            x += ww;
            continue;
        }
        let bw = ww * 0.62;
        let r = Rect::from_min_size(
            Pos2::new(x - bw * 0.5, keys.top()),
            Vec2::new(bw, keys.height() * 0.6),
        );
        let base = if held.contains(&n) { s.hue } else { black };
        light::raised(
            b,
            r,
            1.5 * k,
            base,
            RaisedOpts {
                k,
                depth: s.t.depth,
                ..Default::default()
            },
        );
    }
}

// ---- the widget ----------------------------------------------------------------------

/// What [`Shell::show`] reports.
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    /// What the user did this frame, in order.
    pub events: Vec<Event>,
    /// The scale the shell was drawn at.
    pub k: f32,
    pub header: Rect,
    pub body: Rect,
    pub footer: Rect,
    /// While the grip is dragged: the scale it would snap to on release.
    pub pending_scale: Option<f32>,
    /// Where the window's own buttons were drawn (round 20), when the header has them.
    pub window: Option<WindowRects>,
}

/// The shell as a widget: `Shell::new(id).show(ui, &header, &footer, |ui, style, body| …)`.
/// It fills `ui`'s whole rect (the caller sizes the window: the designed size × `k`).
pub struct Shell {
    pub id: egui::Id,
}

thread_local! {
    static HELP: std::cell::RefCell<String> = std::cell::RefCell::new(String::with_capacity(96));
}

impl Shell {
    pub fn new(id: impl std::hash::Hash + std::fmt::Debug) -> Shell {
        Shell {
            id: egui::Id::new(("cantus-shell", id)),
        }
    }

    /// The scale this shell shows at (kept per id; 100 % until set).
    pub fn scale(&self, ctx: &egui::Context) -> f32 {
        ctx.data(|d| d.get_temp::<f32>(self.id)).unwrap_or(1.0)
    }

    /// Set the scale (snapped to a step): from the ⤢ menu, or the stored choice.
    pub fn set_scale(&self, ctx: &egui::Context, k: f32) {
        ctx.data_mut(|d| d.insert_temp(self.id, snap_scale(k)));
    }

    pub fn show(
        &self,
        ui: &mut egui::Ui,
        header: &Header<'_>,
        footer: &Footer<'_>,
        body: impl FnOnce(&mut egui::Ui, &Style, Rect),
    ) -> Output {
        use egui::Sense;
        let ctx = ui.ctx().clone();
        let k = self.scale(&ctx);
        let base = Style::current();
        let s = base.scaled(k).function(header.hue);
        let rect = ui.max_rect();
        // Claim the whole rect, so the window's area is as large as the shell: without it
        // the area measures nothing, the pointer is never over it, and the wheel never
        // reaches a scroll area inside (an instrument's Edit view did not scroll).
        ui.expand_to_include_rect(rect);
        let hr = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEADER * k));
        let fr = Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - FOOTER * k), rect.max);
        let br = Rect::from_min_max(
            Pos2::new(rect.left(), hr.bottom()),
            Pos2::new(rect.right(), fr.top()),
        );
        let mut events = Vec::new();

        // The header: its background moves the window; its parts are buttons.
        let bg = ui.interact(hr, self.id.with("header"), Sense::click_and_drag());
        if bg.dragged() {
            events.push(Event::Move(bg.drag_delta()));
        }
        let r = header_rects(hr, &s, header);
        let parts: [(Option<Rect>, Hot, Event, &str); 12] = [
            (Some(r.menu), Hot::Menu, Event::Menu, "shell.menu"),
            (
                r.preset,
                Hot::Preset,
                Event::PresetBrowse,
                "shell.preset.browse",
            ),
            (
                r.preset_prev,
                Hot::PresetPrev,
                Event::PresetPrev,
                "shell.preset.prev",
            ),
            (
                r.preset_next,
                Hot::PresetNext,
                Event::PresetNext,
                "shell.preset.next",
            ),
            (
                r.star,
                Hot::Star,
                Event::Favourite,
                "shell.preset.favourite",
            ),
            (r.ab, Hot::Ab, Event::Ab(Ab::A), "shell.ab.tip"),
            (r.undo, Hot::Undo, Event::Undo, "shell.undo"),
            (r.redo, Hot::Redo, Event::Redo, "shell.redo"),
            (r.views, Hot::Views, Event::View(0), "shell.view.play"),
            (r.bypass, Hot::Bypass, Event::Bypass, "shell.bypass"),
            (r.help, Hot::Help, Event::Help, "shell.help"),
            (Some(r.more), Hot::More, Event::More, "shell.more"),
        ];
        let mut hot = Hot::None;
        for (i, (rr, h, ev, key)) in parts.iter().enumerate() {
            let Some(rr) = rr else { continue };
            let resp = ui.interact(*rr, self.id.with(("part", i)), Sense::click());
            if resp.hovered() {
                hot = *h;
                super::hint::set_text(&ctx, &crate::i18n::t(key), "", super::hint::Caps::NONE);
            }
            if resp.clicked() {
                let ev = match (*ev, resp.interact_pointer_pos()) {
                    (Event::Ab(_), Some(p)) => {
                        Event::Ab(if p.x > rr.center().x { Ab::B } else { Ab::A })
                    }
                    (Event::View(_), Some(p)) => {
                        let n = header.views.map_or(1, |(l, _)| l.len());
                        Event::View(segmented::hit(*rr, n, p.x).unwrap_or(0))
                    }
                    (e, _) => e,
                };
                events.push(ev);
            }
        }
        let size = ui.interact(r.size, self.id.with("size"), Sense::click());
        if size.hovered() {
            hot = Hot::Size;
            super::hint::set_text(
                &ctx,
                &crate::i18n::t("shell.size"),
                "",
                super::hint::Caps {
                    menu: true,
                    ..super::hint::Caps::NONE
                },
            );
        }
        if size.clicked() {
            events.push(Event::SizeMenu);
        }
        // The window's own buttons (round 20): each a button with a tooltip.
        if let (Some(c), Some(w)) = (&header.window, &r.window) {
            let keys = [
                (w.swap, WinHot::Swap, Event::Swap),
                (w.minimise, WinHot::Minimise, Event::Minimise),
                (w.maximise, WinHot::Maximise, Event::Maximise),
                (w.close, WinHot::Close, Event::Close),
            ];
            for (rr, wh, ev) in keys {
                let Some(rr) = rr else { continue };
                let resp = ui.interact(rr, self.id.with(("window", wh as u8)), Sense::click());
                if resp.hovered() || resp.is_pointer_button_down_on() {
                    hot = Hot::Window(wh, resp.is_pointer_button_down_on());
                    let tip = crate::i18n::t(window_ctl::tip(c, wh));
                    super::hint::set_text(&ctx, &tip, "", super::hint::Caps::NONE);
                    resp.clone().on_hover_text(tip);
                }
                if resp.clicked() {
                    events.push(ev);
                }
            }
        }
        let painter = ui.painter_at(rect);
        super::with_brush(&painter, |b| {
            paint_header(b, hr, &s, header, hot);
            paint_ground(b, br, &s);
        });

        // The body is the caller's.
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(br)
                .id_salt(self.id.with("body")),
        );
        body(&mut child, &s, br);

        // The grip: resize freely, snap on release.
        let grip = sq(
            Pos2::new(fr.right() - 9.0 * k, fr.center().y + 2.0 * k),
            16.0 * k,
        );
        let g = ui.interact(grip, self.id.with("grip"), Sense::drag());
        let pending_key = self.id.with("pending");
        let mut pending = ctx.data(|d| d.get_temp::<f32>(pending_key));
        if g.drag_started() {
            pending = Some(k);
        }
        if g.dragged() {
            let d = g.drag_delta();
            let grow = 1.0 + (d.x / rect.width().max(1.0) + d.y / rect.height().max(1.0)) * 0.5;
            pending =
                Some((pending.unwrap_or(k) * grow).clamp(SCALES[0], SCALES[SCALES.len() - 1]));
            ctx.set_cursor_icon(egui::CursorIcon::ResizeNwSe);
        }
        if g.drag_stopped() {
            if let Some(p) = pending.take() {
                let snapped = snap_scale(p);
                self.set_scale(&ctx, snapped);
                events.push(Event::Scale(snapped));
            }
        }
        ctx.data_mut(|d| match pending {
            Some(p) => {
                d.insert_temp(pending_key, p);
            }
            None => d.remove::<f32>(pending_key),
        });
        if g.hovered() && !g.dragged() {
            super::hint::set_text(
                &ctx,
                &crate::i18n::t("shell.grip"),
                "",
                super::hint::Caps::NONE,
            );
        }

        // The footer, with the help line of whatever the pointer is on.
        let pending_scale = pending.map(snap_scale);
        HELP.with(|h| {
            let mut h = h.borrow_mut();
            h.clear();
            super::hint::read(&ctx, |hint| match hint {
                Some(hint) => {
                    h.push_str(&hint.name);
                    if !hint.value.is_empty() {
                        h.push_str(" · ");
                        h.push_str(&hint.value);
                    }
                }
                None => h.push_str(&crate::i18n::t("shell.help.idle")),
            });
            super::with_brush(&painter, |b| {
                paint_footer(b, fr, &s, footer, &h);
                if let Some(p) = pending_scale {
                    scale_pill(b, grip.left_top(), &s, p);
                }
            });
        });
        Output {
            events,
            k,
            header: hr,
            body: br,
            footer: fr,
            pending_scale,
            window: r.window,
        }
    }
}

/// The pill that shows the scale a grip drag would snap to.
fn scale_pill(b: &mut Brush, at: Pos2, s: &Style, k: f32) {
    let t = &s.t;
    let labels = ["75 %", "90 %", "100 %", "125 %", "150 %", "175 %", "200 %"];
    let i = SCALES
        .iter()
        .position(|v| (*v - k).abs() < 1e-3)
        .unwrap_or(2);
    let r = Rect::from_min_size(at - Vec2::new(60.0, 26.0), Vec2::new(52.0, 20.0));
    b.rrect(b.snap_rect(r), 10.0, Fill::Solid(t.value_pill));
    let _ = text::paint(
        b,
        r,
        &Text::new(labels[i], Role::Value, t.value_pill_text),
        1.0,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bands_stay_within_the_budget() {
        let all = Bands {
            macros_open: true,
            mods_open: true,
            keyboard: true,
        };
        let share = bands_height(&all) / INSTRUMENT.y;
        assert!(
            share <= 0.22,
            "the bands take {:.1} % of the frame",
            share * 100.0
        );
        let none = Bands {
            macros_open: false,
            mods_open: false,
            keyboard: false,
        };
        assert_eq!(bands_height(&none), COLLAPSED);
    }

    #[test]
    fn scales_snap_to_the_steps() {
        assert_eq!(snap_scale(1.1), 1.0);
        assert_eq!(snap_scale(1.2), 1.25);
        assert_eq!(snap_scale(3.0), 2.0);
        assert_eq!(snap_scale(0.1), 0.75);
    }

    #[test]
    fn the_header_parts_never_overlap() {
        let s = Style::new(super::super::Tokens::slate());
        let views = [Src::Str("Play"), Src::Str("Edit"), Src::Str("Matrix")];
        let h = Header {
            name: "Calor",
            context: "(Insert 3 · Keys)",
            hue: Function::Source,
            preset: Some(Preset {
                name: "Glass Pluck",
                modified: true,
                favourite: false,
            }),
            ab: Some(Ab::A),
            undo: Some((true, false)),
            views: Some((&views, 0)),
            bypassed: Some(false),
            help: true,
            window: Some(WindowCtl::ALL),
        };
        for w in [820.0, 880.0, 1040.0] {
            let r = header_rects(
                Rect::from_min_size(Pos2::ZERO, Vec2::new(w, HEADER)),
                &s,
                &h,
            );
            let parts: Vec<Rect> = [
                Some(r.menu),
                r.preset_prev,
                r.preset,
                r.preset_next,
                r.ab,
                r.undo,
                r.redo,
                r.views,
                r.bypass,
                r.help,
                Some(r.more),
                Some(r.size),
            ]
            .into_iter()
            .chain(
                r.window
                    .iter()
                    .flat_map(|w| [w.swap, w.minimise, w.maximise, w.close]),
            )
            .flatten()
            .collect();
            for (i, a) in parts.iter().enumerate() {
                for bb in &parts[i + 1..] {
                    assert!(
                        !a.intersects(*bb) || a.intersect(*bb).area() < 0.5,
                        "{a:?} overlaps {bb:?} at {w}"
                    );
                }
            }
            assert!(
                r.name.right() <= r.preset_prev.unwrap().left(),
                "the name runs into the presets"
            );
        }
    }
}
