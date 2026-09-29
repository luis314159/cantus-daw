//! "Pick an element": the theme's eyedropper.
//!
//! While inspect mode is on, every kit painter registers what it drew — its rect, its
//! part name and the tokens its colours come from — into a per-frame list. Clicking
//! any spot of the interface then selects the topmost element there and lists the
//! tokens it uses, ready to recolour. When the mode is off, registering is one branch
//! on a flag the brush already holds: nothing is recorded, nothing allocated.
//!
//! The list lives with the kit's per-context cache. A brush collects its items while it
//! paints and hands them over when it flushes; [`hit`] answers from the last complete
//! frame.

use super::brush::{Brush, Fill};
use super::color::with_alpha;
use super::text::{self, Elide, Policy, Role, Text};
use super::tokens::Tok;
use super::{light, Style};
use egui::{Align2, Context, Pos2, Rect, Vec2};

/// One element as painted: where, what, and which tokens colour it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Item {
    pub rect: Rect,
    /// The part's name, as the gallery calls it ("knob", "tile", "card"…).
    pub part: &'static str,
    /// The tokens the part always paints with.
    pub tokens: &'static [Tok],
    /// The tokens this use of it adds: the section hue, a lamp's colour and ground.
    pub extra: [Option<Tok>; 3],
}

impl Item {
    /// Every token of the element, without repeats.
    pub fn all_tokens(&self) -> impl Iterator<Item = Tok> + '_ {
        let extra = self.extra;
        self.tokens
            .iter()
            .copied()
            .chain(extra.into_iter().enumerate().filter_map(move |(i, t)| {
                t.filter(|t| !self.tokens.contains(t) && !extra[..i].contains(&Some(*t)))
            }))
    }
}

/// The per-context registry: the frame being painted and the last complete one.
#[derive(Default)]
pub struct Registry {
    pass: u64,
    current: Vec<Item>,
    last: Vec<Item>,
}

impl Registry {
    fn roll(&mut self, pass: u64) {
        if pass != self.pass {
            std::mem::swap(&mut self.current, &mut self.last);
            self.current.clear();
            self.pass = pass;
        }
    }

    /// Take a brush's items for the pass `pass`.
    pub fn extend(&mut self, pass: u64, items: impl Iterator<Item = Item>) {
        self.roll(pass);
        self.current.extend(items);
    }

    /// The items of the last complete frame, as seen during pass `pass`.
    fn complete(&self, pass: u64) -> &[Item] {
        if pass > self.pass {
            &self.current
        } else {
            &self.last
        }
    }
}

/// Turn inspect mode on or off for `ctx`.
pub fn set_enabled(ctx: &Context, on: bool) {
    super::cache::with_raw(ctx, |c| {
        c.inspect = if on {
            Some(c.inspect.take().unwrap_or_default())
        } else {
            None
        };
    });
}

/// Whether inspect mode is on.
pub fn enabled(ctx: &Context) -> bool {
    super::cache::with_raw(ctx, |c| c.inspect.is_some())
}

/// The elements of the last complete frame, in painting order (topmost last).
pub fn items(ctx: &Context) -> Vec<Item> {
    let pass = ctx.cumulative_pass_nr();
    super::cache::with_raw(ctx, |c| {
        c.inspect
            .as_ref()
            .map(|r| r.complete(pass).to_vec())
            .unwrap_or_default()
    })
}

/// The topmost element at `pos` in the last complete frame.
pub fn hit(ctx: &Context, pos: Pos2) -> Option<Item> {
    let pass = ctx.cumulative_pass_nr();
    super::cache::with_raw(ctx, |c| {
        c.inspect.as_ref().and_then(|r| {
            r.complete(pass)
                .iter()
                .rev()
                .find(|i| i.rect.contains(pos))
                .copied()
        })
    })
}

/// Paint a picked element: its outline, and a card listing its tokens (a swatch, the
/// human name, the stable key) placed at `at`, clamped inside `bounds`.
pub fn paint_pick(b: &mut Brush, s: &Style, item: &Item, at: Pos2, bounds: Rect) {
    let t = &s.t;
    let k = s.k;
    let r = b.snap_rect(item.rect.expand(2.0 * k));
    b.rrect(r, 3.0 * k, Fill::Solid(with_alpha(t.focus, 0.14)));
    b.rrect_stroke(r, 3.0 * k, 2.0 * k, t.focus);
    let n = item.all_tokens().count();
    let row = 22.0 * k;
    let size = Vec2::new(250.0 * k, 30.0 * k + row * n as f32 + 6.0 * k);
    let mut card = Rect::from_min_size(at, size);
    card = card.translate(Vec2::new(
        (bounds.right() - card.right()).min(0.0),
        (bounds.bottom() - card.bottom()).min(0.0),
    ));
    let card = b.snap_rect(card);
    b.rrect_soft(
        card.translate(Vec2::new(0.0, 3.0 * k)),
        6.0 * k,
        Fill::Solid(with_alpha(t.dark_shade, 0.35)),
        12.0 * k,
    );
    light::raised(
        b,
        card,
        5.0 * k,
        t.menu,
        light::RaisedOpts {
            drop: false,
            k,
            depth: t.depth,
            ..Default::default()
        },
    );
    let title = Rect::from_min_size(
        card.min + Vec2::new(10.0 * k, 4.0 * k),
        Vec2::new(card.width() - 20.0 * k, 22.0 * k),
    );
    let tt = Text::new(item.part, Role::Section, t.menu_text)
        .caps(true)
        .align(Align2::LEFT_CENTER);
    let _ = text::paint(b, title, &tt, s.tk());
    for (i, tok) in item.all_tokens().enumerate() {
        let y = card.top() + 28.0 * k + row * i as f32;
        let sw = Rect::from_min_size(
            Pos2::new(card.left() + 10.0 * k, y + 3.0 * k),
            Vec2::splat(16.0 * k),
        );
        b.rrect(
            b.snap_rect(sw),
            3.0 * k,
            Fill::Solid(with_alpha(t.dark_shade, 0.5)),
        );
        b.rrect(
            b.snap_rect(sw).shrink(b.px()),
            2.5 * k,
            Fill::Solid(t.get(tok)),
        );
        let name = crate::i18n::t(&tok.name_key());
        let nr = Rect::from_min_max(
            Pos2::new(sw.right() + 8.0 * k, y),
            Pos2::new(card.right() - 96.0 * k, y + row),
        );
        let nt = Text::new(&name, Role::Body, t.menu_text)
            .align(Align2::LEFT_CENTER)
            .policy(Policy::Elide(Elide::End));
        let _ = text::paint(b, nr, &nt, s.tk());
        let kr = Rect::from_min_max(
            Pos2::new(card.right() - 94.0 * k, y),
            Pos2::new(card.right() - 8.0 * k, y + row),
        );
        let kt = Text::new(tok.key(), Role::Micro, with_alpha(t.menu_text, 0.7))
            .caps(false)
            .align(Align2::RIGHT_CENTER)
            .policy(Policy::Elide(Elide::Middle));
        let _ = text::paint(b, kr, &kt, s.tk());
    }
}
