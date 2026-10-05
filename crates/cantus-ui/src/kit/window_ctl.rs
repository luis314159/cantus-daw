//! A device window's own buttons (round 20): **swap**, **minimise**, **maximise / restore**
//! and **close**, at the right end of the shell's header — where every operating system
//! and FL's plug-in windows put them, so a musician finds them without being told.
//!
//! The owner's words: closing and minimising an instrument meant unfolding a menu first.
//! Here every action is a button of its own, drawn with the kit: flat at rest (they must
//! not compete with the device), lit on hover, sunk while pressed; the close key turns the
//! danger colour under the pointer, as a window's does. *Swap* is a labelled cap — a word
//! and a glyph — because an icon alone for "put another device here" is a riddle.
//!
//! Pure painters and a layout, like the rest of the kit; [`super::shell::Shell::show`]
//! senses the rects and turns clicks into [`super::shell::Event`]s.

use super::brush::{Brush, Fill};
use super::cap::{self, CapLook};
use super::color::{on_fill, shade};
use super::glyph::{self, Glyph};
use super::text::{self, Elide, Policy, Role, Text};
use super::{State, Style, Tok};
use egui::{Align2, Pos2, Rect, Vec2};

/// The tokens the buttons paint with (for inspect mode).
pub const TOKENS: &[Tok] = &[Tok::Panel, Tok::Text, Tok::TextDim, Tok::Danger, Tok::Cap];

/// A window button's width at 100 %: the full height of the header, a little wider.
pub const BUTTON_W: f32 = 30.0;
/// The swap cap's width at 100 %, and its height.
pub const SWAP_W: f32 = 74.0;
pub const SWAP_H: f32 = 20.0;

/// Which buttons a window shows. A device in the exported plug-ins' window has none: the
/// host owns that window (ADR 0020).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct WindowCtl {
    /// The i18n key of the swap cap's tooltip ("Swap for another instrument…"); `None`
    /// hides the cap.
    pub swap: Option<&'static str>,
    pub minimise: bool,
    /// `Some(maximised)` shows the maximise key, or the restore key when maximised.
    pub maximised: Option<bool>,
    pub close: bool,
}

impl WindowCtl {
    /// Every button, the window not maximised.
    pub const ALL: WindowCtl = WindowCtl {
        swap: Some("swap.tip.effect"),
        minimise: true,
        maximised: Some(false),
        close: true,
    };
}

/// Where each button is; `None` for the ones not shown. `left` is the cluster's left edge.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct WindowRects {
    pub swap: Option<Rect>,
    pub minimise: Option<Rect>,
    pub maximise: Option<Rect>,
    pub close: Option<Rect>,
    /// The hairline between the swap cap and the window keys.
    pub divider: Option<f32>,
    pub left: f32,
}

/// The button under the pointer, and whether it is held down.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum WinHot {
    #[default]
    None,
    Swap,
    Minimise,
    Maximise,
    Close,
}

/// Lay the buttons out at the right end of `header` (the header's full rect). `compact`
/// draws the swap cap as its glyph alone, for a header short of room.
pub fn layout(header: Rect, s: &Style, c: &WindowCtl, compact: bool) -> WindowRects {
    let k = s.k;
    let mut x = header.right();
    let mut key = |on: bool| {
        on.then(|| {
            let r = Rect::from_min_max(
                Pos2::new(x - BUTTON_W * k, header.top()),
                Pos2::new(x, header.bottom()),
            );
            x -= BUTTON_W * k;
            r
        })
    };
    let close = key(c.close);
    let maximise = key(c.maximised.is_some());
    let minimise = key(c.minimise);
    let mut divider = None;
    let swap = c.swap.map(|_| {
        let gap = 8.0 * k;
        divider = Some(x - gap * 0.5);
        let w = if compact { SWAP_H + 4.0 } else { SWAP_W } * k;
        let r = Rect::from_center_size(
            Pos2::new(x - gap - w * 0.5, header.center().y),
            Vec2::new(w, SWAP_H * k),
        );
        x = r.left();
        r
    });
    WindowRects {
        swap,
        minimise,
        maximise,
        close,
        divider: divider.filter(|_| close.is_some() || maximise.is_some() || minimise.is_some()),
        left: x,
    }
}

/// Paint the buttons laid out in `r`; `hot` is under the pointer, `down` is held.
pub fn paint(
    b: &mut Brush,
    header: Rect,
    s: &Style,
    c: &WindowCtl,
    r: &WindowRects,
    hot: WinHot,
    down: bool,
) {
    b.register(
        Rect::from_min_max(Pos2::new(r.left, header.top()), header.max),
        "window_ctl",
        TOKENS,
        &[],
    );
    let t = &s.t;
    let k = s.k;
    if let Some(x) = r.divider {
        b.vline(
            b.snap_mid(x),
            header.top() + 6.0 * k,
            header.bottom() - 6.0 * k,
            b.px(),
            shade(t.panel, -0.12),
        );
    }
    if let Some(sr) = r.swap {
        let st = State {
            hovered: hot == WinHot::Swap,
            active: hot == WinHot::Swap && down,
            ..State::REST
        };
        let look = CapLook {
            base: Some(shade(t.panel, if t.dark { 0.07 } else { 0.05 })),
            radius: Some(3.0),
            ..CapLook::default()
        };
        cap::paint(b, b.snap_rect(sr), s, st, &look);
        let ink = if st.hovered { t.text } else { t.text_dim };
        // The glyph alone when the cap is compact, else the glyph and the word.
        let labelled = sr.width() > (SWAP_H + 12.0) * k;
        let gc = if labelled {
            Pos2::new(sr.left() + 12.0 * k, sr.center().y)
        } else {
            sr.center()
        };
        let g = Rect::from_center_size(gc, Vec2::splat(13.0 * k));
        glyph::paint(b, g, Glyph::Replace, ink);
        if labelled {
            let tr = Rect::from_min_max(
                Pos2::new(g.right() + 4.0 * k, sr.top()),
                Pos2::new(sr.right() - 6.0 * k, sr.bottom()),
            );
            let label = Text::key("swap.button", Role::Control, ink)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(Elide::End));
            let _ = text::paint(b, tr, &label, s.tk());
        }
    }
    let keys = [
        (r.minimise, WinHot::Minimise, Glyph::Minimise),
        (
            r.maximise,
            WinHot::Maximise,
            if c.maximised == Some(true) {
                Glyph::Restore
            } else {
                Glyph::Maximise
            },
        ),
        (r.close, WinHot::Close, Glyph::Close),
    ];
    for (rect, h, g) in keys {
        let Some(rect) = rect else { continue };
        let lit = hot == h;
        let fill = match (lit, h == WinHot::Close) {
            (true, true) => Some(if down { shade(t.danger, -0.06) } else { t.danger }),
            (true, false) => Some(shade(
                t.panel,
                match (t.dark, down) {
                    (true, false) => 0.09,
                    (true, true) => 0.04,
                    (false, false) => -0.07,
                    (false, true) => -0.11,
                },
            )),
            _ => None,
        };
        if let Some(f) = fill {
            b.rect(b.snap_rect(rect), Fill::Solid(f));
        }
        let ink = match fill {
            Some(f) if h == WinHot::Close => on_fill(f),
            Some(_) => t.text,
            None => t.text_dim,
        };
        let side = if h == WinHot::Close { 12.0 } else { 11.0 } * k;
        glyph::paint(b, Rect::from_center_size(rect.center(), Vec2::splat(side)), g, ink);
    }
}

/// The i18n key of a button's tooltip.
pub fn tip(c: &WindowCtl, h: WinHot) -> &'static str {
    match h {
        WinHot::Swap => c.swap.unwrap_or("swap.tip.effect"),
        WinHot::Minimise => "swap.tip.minimise",
        WinHot::Maximise if c.maximised == Some(true) => "swap.tip.restore",
        WinHot::Maximise => "swap.tip.maximise",
        WinHot::Close => "swap.tip.close",
        WinHot::None => "",
    }
}
