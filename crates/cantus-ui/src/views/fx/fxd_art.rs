//! fxd — what the two controllers' faces share (ADRs 0147, 0148, design sheet v1): the
//! machined frame a display sits in (chamfered rails, a bevel, hex-socket screws, an
//! engraved scale), the glass over it, a card row packed by its words (`fxa_cells::breaks`:
//! no lone card, the window grows rather than the display shrinking), and the shell the
//! window wears. Vectors at the style's scale; colours from the face's skin and the theme's
//! tokens (no literal colour); nothing animates by itself.

use super::controls::{self, Size};
use super::fxa_cells::{self, Item};
use crate::kit::color::{mix, with_alpha};
use crate::kit::text::{self as ktext, Role, Src, Text};
use crate::kit::{Brush, Fill, Style};
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect};

/// The face's lit colour on its dark display (the accent's dark-theme value: the displays
/// are dark in both themes).
pub fn lit(s: &Style) -> Color32 {
    s.skin.accent_in(true).unwrap_or(s.hue)
}

/// The second voice on the display.
pub fn lit2(s: &Style) -> Color32 {
    s.skin.accent2_in(true).unwrap_or(s.hue)
}

/// The display's ground: near black, a breath of the accent.
pub fn ground(s: &Style) -> Color32 {
    mix(s.t.dark, lit(s), 0.04)
}

/// The metal of the frame: the panel's own surface, lifted and dropped for the bevel.
pub struct Metal {
    pub top: Color32,
    pub face: Color32,
    pub foot: Color32,
    pub edge: Color32,
    pub engrave: Color32,
}

pub fn metal(s: &Style) -> Metal {
    let base = mix(s.t.panel_raised, lit(s), 0.06);
    Metal {
        top: mix(base, s.t.light, 0.35),
        face: base,
        foot: mix(base, s.t.dark, 0.35),
        edge: mix(base, s.t.dark, 0.6),
        engrave: mix(base, s.t.dark, 0.7),
    }
}

/// A hex-socket screw at `p`.
pub fn screw(b: &mut Brush, s: &Style, p: Pos2, rad: f32, turn: f32) {
    let m = metal(s);
    b.circle(p + vec2(0.0, rad * 0.25), rad * 1.12, Fill::Solid(with_alpha(s.t.dark, 0.35)));
    b.circle(p, rad, Fill::V(m.top, m.foot));
    b.circle_stroke(p, rad, rad * 0.12, m.edge);
    let hex: Vec<Pos2> = (0..6)
        .map(|i| {
            let a = turn * 1.047 + i as f32 * std::f32::consts::FRAC_PI_3;
            p + vec2(cantus_dsp::math::cos(a), cantus_dsp::math::sin(a)) * rad * 0.42
        })
        .collect();
    b.polygon(&hex, m.engrave);
}

/// The machined frame around `r` (`w` its rail width), the screws at its corners; returns
/// the opening the display fills.
pub fn frame(b: &mut Brush, s: &Style, r: Rect, w: f32) -> Rect {
    let m = metal(s);
    let k = s.k;
    // The shadow under the frame, then the rails: a chamfered outer edge, a flat, a bevel
    // falling into the opening.
    b.rrect_soft(r.translate(vec2(0.0, 3.0 * k)), 7.0 * k, Fill::Solid(with_alpha(s.t.dark, 0.45)), 8.0 * k);
    b.rrect(r, 7.0 * k, Fill::V(m.top, m.foot));
    b.rrect(r.shrink(1.2 * k), 6.0 * k, Fill::V(m.face, mix(m.face, m.foot, 0.5)));
    // Brushing along the rails: fine lines, a hash apart.
    let mut y = r.top() + 2.0 * k;
    let mut i = 0u32;
    while y < r.bottom() - 2.0 * k {
        let a = 0.04 + 0.05 * hash(i);
        b.hline(r.left() + 2.0 * k, r.right() - 2.0 * k, y, 0.5 * k, with_alpha(m.top, a));
        y += 1.6 * k;
        i += 1;
    }
    let open = r.shrink(w);
    let bevel = open.expand(3.0 * k);
    b.rrect(bevel, 4.0 * k, Fill::V(m.foot, m.top));
    b.rrect_stroke(r.shrink(0.6 * k), 7.0 * k, 0.8 * k, with_alpha(m.top, 0.7));
    b.rrect_stroke(bevel, 4.0 * k, 0.8 * k, m.edge);
    let inset = w * 0.5;
    for (i, c) in [r.left_top(), r.right_top(), r.left_bottom(), r.right_bottom()]
        .into_iter()
        .enumerate()
    {
        let dx = if c.x < r.center().x { inset } else { -inset };
        let dy = if c.y < r.center().y { inset } else { -inset };
        screw(b, s, c + vec2(dx, dy), 3.4 * k, hash(i as u32 + 7));
    }
    open
}

/// The glass over a display: a soft sheen across its upper part and a fine inner edge.
pub fn glass(b: &mut Brush, s: &Style, r: Rect) {
    let k = s.k;
    let sheen = Rect::from_min_max(r.min, pos2(r.max.x, r.min.y + r.height() * 0.42));
    b.rect(
        sheen,
        Fill::V(with_alpha(s.t.light, 0.07), with_alpha(s.t.light, 0.0)),
    );
    b.rrect_stroke(r, 2.0 * k, 0.8 * k, with_alpha(s.t.light, 0.10));
}

/// A number in 0 … 1 for `i`: the same every frame.
pub fn hash(i: u32) -> f32 {
    let mut x = i.wrapping_mul(0x9E37_79B9) ^ 0x85EB_CA6B;
    x ^= x >> 15;
    x = x.wrapping_mul(0x2C1B_3C6D);
    x ^= x >> 12;
    (x & 0xFFFF) as f32 / 65_535.0
}

/// A label painted in `rect` (a display's legend, a rail's engraving).
pub fn label(b: &mut Brush, s: &Style, rect: Rect, src: Src<'_>, role: Role, color: Color32, align: Align2) {
    let t = Text {
        src,
        role,
        caps: true,
        align,
        policy: ktext::Policy::Abbreviate,
        color,
    };
    let _ = ktext::paint(b, rect, &t, s.tk());
}

/// A plain value (numbers, a formula) painted in `rect`, as it is.
pub fn value(b: &mut Brush, s: &Style, rect: Rect, text: &str, role: Role, color: Color32, align: Align2) {
    let t = Text::new(text, role, color).caps(false).align(align);
    let _ = ktext::paint(b, rect, &t, s.tk());
}

/// One card of a face: its title and its items, each drawn by `draw`.
pub struct Card<'a> {
    pub title: &'static str,
    pub items: &'a [Item],
}

/// A card's width at 100 %.
pub fn card_width(ui: &egui::Ui, c: &Card<'_>) -> f32 {
    fxa_cells::card_width(ui, c.items)
}

/// The rows the cards take across `avail` (100 %), packed by their words.
pub fn rows(ui: &egui::Ui, cards: &[Card<'_>], avail: f32) -> Vec<(usize, usize)> {
    let w: Vec<f32> = cards.iter().map(|c| card_width(ui, c)).collect();
    fxa_cells::breaks(&w, GAP, avail)
}

/// The gap between cards, 100 %.
pub const GAP: f32 = 8.0;

/// A card's height, 100 %.
pub fn card_h() -> f32 {
    controls::card_height(Size::M)
}

/// What a control of a card answered.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Moved {
    /// Item `i` of card `c` moved to a value (a knob) or a choice (a switch).
    To { card: usize, item: usize, value: f32 },
    /// The gesture on it ended.
    Done { card: usize, item: usize },
}

/// Lay `cards` in rows across `area` (scaled), each item drawn from `values` (a knob's value
/// with its range, default and readout; a switch's choice).
pub fn lay_cards(
    ui: &mut egui::Ui,
    s: &Style,
    area: Rect,
    cards: &[Card<'_>],
    values: &dyn Fn(usize, usize) -> Value,
) -> Vec<Moved> {
    let mut out = Vec::new();
    let k = s.k;
    let rows = rows(ui, cards, area.width() / k);
    let h = card_h() * k;
    for (r, &(i, j)) in rows.iter().enumerate() {
        let widths: Vec<f32> = cards[i..j].iter().map(|c| card_width(ui, c) * k).collect();
        let total: f32 = widths.iter().sum::<f32>() + GAP * k * (j - i - 1) as f32;
        // Each row fills the width: the slack shared among its cards.
        let extra = ((area.width() - total) / (j - i) as f32).max(0.0);
        let mut x = area.left();
        let y = area.top() + r as f32 * (h + GAP * k);
        for (n, c) in (i..j).enumerate() {
            let rect = Rect::from_min_size(pos2(x, y), vec2(widths[n] + extra, h));
            x += widths[n] + extra + GAP * k;
            let card = &cards[c];
            controls::card(ui, s, rect, Src::Key(card.title), None, |ui, _| {
                for (it, item) in card.items.iter().enumerate() {
                    let w = fxa_cells::width(ui, item);
                    match (*item, values(c, it)) {
                        (Item::Knob(_, lab), Value::Knob { v, lo, hi, def, fmt }) => {
                            let cell = controls::Cell::new(v, lo..=hi, def)
                                .size(Size::M)
                                .label(Src::Key(lab))
                                .width(w)
                                .format(fmt);
                            let o = cell.show(ui, s);
                            if let Some(v) = o.changed {
                                out.push(Moved::To { card: c, item: it, value: v });
                            }
                            if o.done {
                                out.push(Moved::Done { card: c, item: it });
                            }
                        }
                        (Item::Switch(_, lab, choices), Value::Choice(sel)) => {
                            controls::column(ui, s, w, |ui| {
                                controls::caption(ui, s, Src::Key(lab), w);
                                let labels: Vec<Src<'_>> =
                                    choices.iter().map(|c| Src::Key(c)).collect();
                                if let Some(v) = controls::segmented(ui, s, sel, &labels, lab, w) {
                                    out.push(Moved::To { card: c, item: it, value: v as f32 });
                                    out.push(Moved::Done { card: c, item: it });
                                }
                            });
                        }
                        _ => {}
                    }
                }
            });
        }
    }
    out
}

/// The height `cards` take across `avail` (100 %), gaps in.
pub fn cards_height(ui: &egui::Ui, cards: &[Card<'_>], avail: f32) -> f32 {
    let n = rows(ui, cards, avail).len().max(1) as f32;
    n * card_h() + (n - 1.0) * GAP
}

/// What an item of a card shows.
pub enum Value<'a> {
    Knob {
        v: f32,
        lo: f32,
        hi: f32,
        def: f32,
        fmt: &'a dyn Fn(f32, &mut String),
    },
    Choice(usize),
}

/// A panel's sunk well (a list, the object's bay): the theme's well, a fine edge.
pub fn well(b: &mut Brush, s: &Style, r: Rect) {
    b.rrect(r, 5.0 * s.k, Fill::V(mix(s.t.panel, s.t.dark, 0.25), s.t.panel));
    b.rrect_stroke(r, 5.0 * s.k, 0.7 * s.k, with_alpha(s.t.dark, 0.5));
}
