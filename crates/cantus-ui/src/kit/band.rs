//! `SectionBand` — a 20-pt band carrying a section's title, with an optional power LED
//! and dividers between sections (FL's LOUDNESS | ENVELOPE | NOISE GATE). `Card` — a
//! device section: a 24-pt title bar (LED, TITLE, type selector, ⋯) over a body of
//! cells, with a display that bleeds to the card's edges.

use super::brush::{Brush, Fill};
use super::color::{shade, with_alpha};
use super::glyph::{self, Glyph};
use super::text::{self, Elide, Policy, Role, Src, Text};
use super::{State, Style, Tok};

/// The tokens a section band paints with, beside its hue (for inspect mode).
pub const SECTION_TOKENS: &[Tok] = &[Tok::Panel, Tok::Label, Tok::KnobTrack];
/// The tokens a card paints with, beside its hue.
pub const CARD_TOKENS: &[Tok] = &[
    Tok::Deck,
    Tok::DeckTitle,
    Tok::Label,
    Tok::Text,
    Tok::TextDim,
    Tok::KnobTrack,
    Tok::Ink,
    Tok::Edge,
    Tok::Light,
    Tok::Dark,
];
use egui::{Align2, Color32, Pos2, Rect, Vec2};

/// A band's height at 100 %.
pub const BAND: f32 = 20.0;
/// A card's title bar at 100 %.
pub const TITLE: f32 = 24.0;

/// A square-ish section lamp (10 × 10, radius 2): filled in the hue when on, hollow off.
pub fn lamp(b: &mut Brush, c: Pos2, s: &Style, on: bool, hue: Color32, alpha: f32) {
    let r = Rect::from_center_size(c, Vec2::splat(s.s(10.0)));
    let r = b.snap_rect(r);
    let f = |c: Color32| c.gamma_multiply(alpha);
    if on {
        super::light::glow(b, r.center(), s.s(11.0), hue, 0.7 * alpha);
        b.rrect(r, s.r(2.0), Fill::Solid(f(shade(hue, -0.25))));
        b.rrect(
            r.shrink(b.px()),
            s.r(1.5),
            Fill::V(f(shade(hue, 0.10)), f(hue)),
        );
    } else {
        b.rrect_stroke(r.shrink(s.s(0.75)), s.r(2.0), s.s(1.5), f(s.t.knob_track));
    }
}

/// Paint a section band in `rect`. `power`: an LED at the left, lit when `Some(true)`.
pub fn section(
    b: &mut Brush,
    rect: Rect,
    s: &Style,
    st: State,
    title: Src<'_>,
    power: Option<bool>,
    centred: bool,
) {
    b.register(rect, "section_band", SECTION_TOKENS, &[s.hue_tok]);
    let rect = b.snap_rect(rect);
    let a = st.alpha();
    let f = |c: Color32| c.gamma_multiply(a);
    let base = shade(s.t.panel, if s.t.dark { -0.08 } else { -0.05 });
    b.rect(rect, Fill::Solid(f(base)));
    b.hline(
        rect.left(),
        rect.right(),
        rect.top() + b.px() * 0.5,
        b.px(),
        f(shade(base, 0.05)),
    );
    let mut x = rect.left() + s.s(8.0);
    if let Some(on) = power {
        lamp(b, Pos2::new(x + s.s(5.0), rect.center().y), s, on, s.hue, a);
        x += s.s(16.0);
    }
    let tr = Rect::from_min_max(
        Pos2::new(x, rect.top()),
        Pos2::new(rect.right() - s.s(8.0), rect.bottom()),
    );
    let t = Text {
        src: title,
        role: Role::Section,
        caps: true,
        align: if centred {
            Align2::CENTER_CENTER
        } else {
            Align2::LEFT_CENTER
        },
        policy: Policy::Elide(Elide::End),
        color: f(s.t.label),
    };
    let _ = text::paint(b, tr, &t, s.tk());
}

/// A 1-px divider between two bands or sections.
pub fn divider(b: &mut Brush, x: f32, y0: f32, y1: f32, s: &Style) {
    let x = b.snap_mid(x);
    b.vline(
        x,
        y0,
        y1,
        b.px(),
        shade(s.t.panel, if s.t.dark { -0.16 } else { -0.12 }),
    );
}

/// What a card's title bar holds.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CardLook<'a> {
    pub title: Src<'a>,
    /// The section can be switched off: an LED at the left (lit = on). Otherwise a
    /// 3-pt tab in the hue marks it.
    pub switchable: bool,
    /// A type selector shown as ‹ name ›.
    pub selector: Option<&'a str>,
    /// A ⋯ menu at the right.
    pub menu: bool,
    /// A display under the title bar, this tall (points at 100 %), bleeding to the edges.
    pub display: Option<f32>,
}

/// Where a card's parts are.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct CardRects {
    pub title: Rect,
    pub led: Option<Rect>,
    pub selector: Option<Rect>,
    pub menu: Option<Rect>,
    pub display: Option<Rect>,
    pub body: Rect,
}

pub fn rects(rect: Rect, s: &Style, look: &CardLook<'_>) -> CardRects {
    let title = Rect::from_min_size(rect.min, Vec2::new(rect.width(), s.s(TITLE)));
    let led = look.switchable.then(|| {
        Rect::from_center_size(
            Pos2::new(rect.left() + s.s(13.0), title.center().y),
            Vec2::splat(s.s(12.0)),
        )
    });
    let menu = look.menu.then(|| {
        Rect::from_center_size(
            Pos2::new(rect.right() - s.s(14.0), title.center().y),
            Vec2::new(s.s(18.0), s.s(16.0)),
        )
    });
    let selector = look.selector.map(|_| {
        let right = menu.map_or(rect.right() - s.s(8.0), |m| m.left() - s.s(4.0));
        Rect::from_min_max(
            Pos2::new(rect.center().x, title.top()),
            Pos2::new(right, title.bottom()),
        )
    });
    let display = look.display.map(|h| {
        Rect::from_min_size(
            Pos2::new(rect.left(), title.bottom() + s.s(1.0)),
            Vec2::new(rect.width(), s.s(h)),
        )
    });
    let top = display.map_or(title.bottom(), |d| d.bottom());
    let body = Rect::from_min_max(
        Pos2::new(rect.left() + s.s(8.0), top + s.s(8.0)),
        Pos2::new(rect.right() - s.s(8.0), rect.bottom() - s.s(8.0)),
    );
    CardRects {
        title,
        led,
        selector,
        menu,
        display,
        body,
    }
}

/// Paint a card's frame, title bar and display ground. `st.lit`: the section is on;
/// `st.focused`: the selected card (a border in the hue). The caller paints the body
/// (at 45 % when the section is off) and the display's content.
pub fn card(b: &mut Brush, rect: Rect, s: &Style, st: State, look: &CardLook<'_>) -> CardRects {
    b.register(rect, "card", CARD_TOKENS, &[s.hue_tok]);
    let rect = b.snap_rect(rect);
    let rad = s.r(6.0);
    let dp = s.t.depth;
    let a = if st.disabled { 0.45 } else { 1.0 };
    let f = |c: Color32| c.gamma_multiply(a);
    let t = &s.t;
    if !t.dark && dp > 0.0 {
        b.rrect_soft(
            rect.translate(Vec2::new(0.0, s.s(1.0))),
            rad,
            Fill::Solid(with_alpha(t.dark_shade, 0.10 * dp)),
            s.s(3.0),
        );
    }
    b.rrect(rect, rad, Fill::Solid(f(t.deck)));
    let r = rects(rect, s, look);
    // The title band: the card's top, rounded above, square below.
    let tb = b.snap_rect(r.title);
    b.rrect(
        tb,
        rad,
        Fill::V(
            f(shade(t.deck_title, 0.015 * dp)),
            f(shade(t.deck_title, -0.015 * dp)),
        ),
    );
    b.rect(
        Rect::from_min_max(Pos2::new(tb.left(), tb.center().y), tb.max),
        Fill::V(f(t.deck_title), f(shade(t.deck_title, -0.015 * dp))),
    );
    let hl = if t.dark {
        with_alpha(t.light, 0.06 * dp)
    } else {
        with_alpha(t.light, 0.7 * dp.min(1.0))
    };
    b.hline(
        tb.left() + rad,
        tb.right() - rad,
        tb.top() + b.px() * 0.5,
        b.px(),
        hl,
    );
    b.hline(
        tb.left(),
        tb.right(),
        tb.bottom() - b.px() * 0.5,
        b.px(),
        f(shade(t.deck_title, -0.08)),
    );
    let mut x = tb.left() + s.s(8.0);
    match r.led {
        Some(led) => {
            lamp(b, led.center(), s, st.lit, s.hue, a);
            x = led.right() + s.s(4.0);
        }
        None => {
            let tab = Rect::from_min_size(
                Pos2::new(tb.left() + s.s(8.0), tb.center().y - s.s(6.0)),
                Vec2::new(s.s(3.0), s.s(12.0)),
            );
            b.rrect(b.snap_rect(tab), s.r(1.0), Fill::Solid(f(s.hue)));
            x += s.s(9.0);
        }
    }
    let title_right = r
        .selector
        .map_or(r.menu.map_or(tb.right() - s.s(8.0), |m| m.left()), |sr| {
            sr.left() - s.s(6.0)
        });
    let title_color = if st.lit || !look.switchable {
        t.label
    } else {
        t.text_dim
    };
    let tt = Text {
        src: look.title,
        role: Role::Section,
        caps: true,
        align: Align2::LEFT_CENTER,
        policy: Policy::Elide(Elide::End),
        color: f(title_color),
    };
    let _ = text::paint(
        b,
        Rect::from_min_max(Pos2::new(x, tb.top()), Pos2::new(title_right, tb.bottom())),
        &tt,
        s.k,
    );
    if let (Some(sr), Some(name)) = (r.selector, look.selector) {
        let cw = s.s(10.0);
        glyph::paint(
            b,
            Rect::from_min_size(
                Pos2::new(sr.left(), sr.center().y - cw * 0.5),
                Vec2::splat(cw),
            ),
            Glyph::Prev,
            f(t.text_dim),
        );
        glyph::paint(
            b,
            Rect::from_min_size(
                Pos2::new(sr.right() - cw, sr.center().y - cw * 0.5),
                Vec2::splat(cw),
            ),
            Glyph::Next,
            f(t.text_dim),
        );
        let inner = sr.shrink2(Vec2::new(cw + s.s(2.0), 0.0));
        let _ = text::paint(
            b,
            inner,
            &Text::new(name, Role::Control, f(t.text)).policy(Policy::Elide(Elide::End)),
            s.k,
        );
    }
    if let Some(m) = r.menu {
        glyph::paint(b, m.shrink(s.s(2.0)), Glyph::More, f(t.text_dim));
    }
    if let Some(d) = r.display {
        let d = b.snap_rect(d);
        b.rect(
            d,
            Fill::V(
                f(t.ink),
                f(shade(t.ink, if t.dark { -0.02 } else { -0.03 })),
            ),
        );
        let edge = f(with_alpha(t.edge, 0.55));
        b.hline(d.left(), d.right(), d.top() + b.px() * 0.5, b.px(), edge);
        b.hline(d.left(), d.right(), d.bottom() - b.px() * 0.5, b.px(), edge);
    }
    if st.focused {
        b.rrect_stroke(rect.shrink(s.s(0.75)), rad, s.s(1.5), s.hue);
    }
    r
}
