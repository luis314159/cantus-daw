//! The room card over an instrument's picture (round 40 phase 3, ADR 0126; the integrator):
//! *Room* and its rooms on one row; the room's name and what it is on the next; then the
//! floor, the walls and the rug, each with its label, on one row or two — every part inside
//! the card and the card inside the picture. Tympanum's Kit view and Chorda's Play view
//! draw this one card (each with its own rooms and words).
//!
//! The caller makes the words ([`Card`]) and measures them once ([`sizes`]: a view that
//! keeps them allocates nothing at rest); the card answers what was chosen ([`Shown`]).

use crate::i18n::t;
use crate::kit::text::Src;
use crate::kit::{with_brush, Style};
use crate::views::inst::inst_mixer::ui::{self as iu, words};
use crate::views::mixer::look::{Pal, W};
use cantus_core::inst::room_look::{self, Floor, RoomKind, Rug, Walls};
use cantus_core::NativeSpec;
use egui::{pos2, vec2, Align2, Rect, Sense};

/// One of the room's looks: the floor, the walls or the rug (the scene's words, schema 69)
/// — the picture only, never heard.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LookPart {
    Floor,
    Walls,
    Rug,
}

impl LookPart {
    pub const ALL: [LookPart; 3] = [LookPart::Floor, LookPart::Walls, LookPart::Rug];

    /// The text key it is kept in (`look.floor`, `look.walls`, `look.rug`).
    pub fn key(self) -> &'static str {
        match self {
            LookPart::Floor => room_look::FLOOR_KEY,
            LookPart::Walls => room_look::WALLS_KEY,
            LookPart::Rug => room_look::RUG_KEY,
        }
    }

    /// Its label's string key.
    pub fn label(self) -> &'static str {
        match self {
            LookPart::Floor => "scene.floor",
            LookPart::Walls => "scene.walls",
            LookPart::Rug => "scene.rug",
        }
    }

    /// Its words, in the scene's order: (stored word, its name's string key).
    pub fn choices(self) -> Vec<(&'static str, &'static str)> {
        match self {
            LookPart::Floor => Floor::ALL
                .iter()
                .map(|x| (x.key(), x.label_key()))
                .collect(),
            LookPart::Walls => Walls::ALL
                .iter()
                .map(|x| (x.key(), x.label_key()))
                .collect(),
            LookPart::Rug => Rug::ALL.iter().map(|x| (x.key(), x.label_key())).collect(),
        }
    }

    /// The word shown in `room` (the scene's room): the stored one, else the room's own —
    /// (stored word, its name's string key).
    pub fn now(self, spec: &NativeSpec, room: crate::scene::Room) -> (&'static str, &'static str) {
        let look = room_look::of(spec);
        let d = crate::scene::rooms::def(room);
        match self {
            LookPart::Floor => {
                let x = look.floor.unwrap_or(d.floor);
                (x.key(), x.label_key())
            }
            LookPart::Walls => {
                let x = look.walls.unwrap_or(d.walls);
                (x.key(), x.label_key())
            }
            LookPart::Rug => {
                let x = look.rug.unwrap_or(d.rug);
                (x.key(), x.label_key())
            }
        }
    }
}

/// What the card says — every word made by the caller.
pub struct Card<'a> {
    /// *Room*, before the rooms.
    pub label: &'a str,
    pub rooms: &'a [RoomKind],
    /// Each room's word, in `rooms`' order (at most five).
    pub names: &'a [String],
    pub now: RoomKind,
    /// What the room shown is (its second line, after its name): left out when it does
    /// not fit whole.
    pub meta: &'a str,
    /// Floor, Walls, Rug.
    pub look_labels: [&'a str; 3],
    /// The word each look shows now.
    pub looks: [&'a str; 3],
    /// What the rooms row and a look's field say on the help line.
    pub room_tip: &'a str,
    pub look_tip: &'a str,
}

/// The card's measures at one scale: the label, the rooms row, each look's label and field
/// (a field as wide as its longest word: no choice is ever cut).
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Sizes {
    pub k: f32,
    label: f32,
    segs: f32,
    parts: [(f32, f32); 3],
}

/// Measure `card` (allocates: a view that keeps the result measures again only when its
/// words or its scale change).
pub fn sizes(ui: &egui::Ui, s: &Style, card: &Card) -> Sizes {
    let segs = card
        .names
        .iter()
        .map(|n| iu::width(ui, s, n, 12.0, W::Medium) + s.s(24.0))
        .sum::<f32>();
    let parts = std::array::from_fn(|i| {
        let part = LookPart::ALL[i];
        let field = part
            .choices()
            .iter()
            .map(|(_, k)| iu::width(ui, s, &t(k), 12.0, W::Regular))
            .fold(0.0f32, f32::max)
            // The field's insets (its word 8 pt in, its chevron's 20) are not scaled.
            + s.s(30.0).max(31.0);
        (
            iu::width(ui, s, card.look_labels[i], 12.0, W::Regular),
            field,
        )
    });
    Sizes {
        k: s.k,
        label: iu::width(ui, s, card.label, 12.0, W::Regular),
        segs,
        parts,
    }
}

const PAD: f32 = 12.0;
const GAP: f32 = 6.0;

/// The card's width when `room` points are free, and whether its looks sit on one row
/// (else Floor · Walls over the Rug).
pub fn layout(sz: &Sizes, s: &Style, room: f32) -> (f32, bool) {
    let (pad, gap) = (s.s(PAD), s.s(GAP));
    let segs = (s.s(28.0) + sz.label + s.s(12.0) + sz.segs).max(s.s(300.0));
    let one = pad * 2.0 + sz.parts.iter().map(|(l, f)| l + gap + f).sum::<f32>() + 2.0 * s.s(12.0);
    let p = &sz.parts;
    let two = pad * 2.0 + p[0].0 + p[0].1 + p[1].0 + p[1].1 + 2.0 * gap + s.s(12.0);
    if segs.max(one) <= room {
        (segs.max(one), true)
    } else {
        (segs.max(two).min(room.max(segs)), false)
    }
}

/// The card's height for its layout.
pub fn height(s: &Style, fits: bool) -> f32 {
    s.s(if fits { 104.0 } else { 134.0 })
}

/// What the card did this frame: where it stands, its fields, and a room or a look chosen.
#[derive(Copy, Clone, Debug)]
pub struct Shown {
    pub card: Rect,
    pub fields: [Rect; 3],
    pub room: Option<RoomKind>,
    pub look: Option<(LookPart, &'static str)>,
}

/// The card at the picture's bottom right. `open` is the look whose list is open, `opened`
/// the pass it opened on (its opening click is not a click outside it); both the caller's.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &egui::Ui,
    s: &Style,
    id: egui::Id,
    pic: Rect,
    card: &Card,
    sz: &Sizes,
    (w, fits): (f32, bool),
    open: &mut Option<LookPart>,
    opened: &mut u64,
) -> Shown {
    let p = Pal::of(s);
    let (pad, gap) = (s.s(PAD), s.s(GAP));
    let w = w.min(pic.width() - 2.0 * pad);
    let h = height(s, fits);
    let rect = Rect::from_min_size(
        pos2(pic.right() - pad - w, pic.bottom() - pad - h),
        vec2(w, h),
    );
    let mut shown = Shown {
        card: rect,
        fields: [Rect::NOTHING; 3],
        room: None,
        look: None,
    };
    with_brush(ui.painter(), |b| iu::veil(b, s, rect));
    // Row 1: Room and its rooms.
    let r1 = Rect::from_min_size(
        pos2(rect.left() + pad, rect.top() + s.s(10.0)),
        vec2(w - 2.0 * pad, s.s(26.0)),
    );
    with_brush(ui.painter(), |b| {
        words(
            b,
            s,
            card.label,
            r1,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink3,
        );
    });
    let n = card.rooms.len().min(card.names.len()).min(5);
    let labels: [Src<'_>; 5] =
        std::array::from_fn(|i| Src::Str(card.names.get(i).map_or("", String::as_str)));
    let sel = card.rooms.iter().position(|k| *k == card.now).unwrap_or(0);
    let seg = Rect::from_min_max(pos2(r1.left() + sz.label + s.s(12.0), r1.top()), r1.max);
    let (resp, chosen) = crate::kit::widgets::Segmented::new(sel, &labels[..n])
        .id(id.with("rooms"))
        .name(card.label)
        .style(*s)
        .show_at(ui, seg);
    let _ = iu::hint(resp, || card.room_tip.to_string());
    if let Some(i) = chosen.filter(|i| *i != sel) {
        shown.room = card.rooms.get(i).copied();
    }
    // Row 2: the room, and what it is.
    let r2 = Rect::from_min_size(
        pos2(r1.left(), r1.bottom() + s.s(6.0)),
        vec2(r1.width(), s.s(22.0)),
    );
    with_brush(ui.painter(), |b| {
        let name = card.names.get(sel).map_or("", String::as_str);
        let nw = words(
            b,
            s,
            name,
            r2,
            Align2::LEFT_CENTER,
            12.0,
            W::SemiBold,
            p.ink1,
        );
        let mr = Rect::from_min_max(pos2(r2.left() + nw + s.s(8.0), r2.top()), r2.max);
        iu::note(
            b,
            s,
            card.meta,
            mr,
            Align2::LEFT_CENTER,
            11.0,
            W::Regular,
            p.ink2,
        );
    });
    // Rows 3 (and 4): the looks.
    let mut x = r1.left();
    let mut y = r2.bottom() + s.s(8.0);
    for (i, (lw, fw)) in sz.parts.iter().enumerate() {
        if !fits && i == 2 {
            x = r1.left();
            y += s.s(30.0);
        }
        let lr = Rect::from_min_size(pos2(x, y), vec2(lw + 1.0, s.s(26.0)));
        let fr = Rect::from_min_size(pos2(lr.right() + gap, y), vec2(*fw, s.s(26.0)));
        x = fr.right() + s.s(12.0);
        shown.fields[i] = fr;
        let part = LookPart::ALL[i];
        if let Some(word) = field(ui, s, id, card, i, lr, fr, open, opened) {
            shown.look = Some((part, word));
        }
    }
    probe::note(pic, &shown);
    shown
}

/// One look's label and field, and its list when open: the word chosen.
#[allow(clippy::too_many_arguments)]
fn field(
    ui: &egui::Ui,
    s: &Style,
    id: egui::Id,
    card: &Card,
    i: usize,
    lr: Rect,
    fr: Rect,
    open: &mut Option<LookPart>,
    opened: &mut u64,
) -> Option<&'static str> {
    let p = Pal::of(s);
    let part = LookPart::ALL[i];
    let resp = ui.interact(fr, id.with(part.key()), Sense::click());
    let resp = iu::hint(resp, || card.look_tip.to_string());
    if resp.clicked() {
        *open = if *open == Some(part) {
            None
        } else {
            Some(part)
        };
        *opened = ui.ctx().cumulative_pass_nr();
    }
    let is_open = *open == Some(part);
    with_brush(ui.painter(), |b| {
        words(
            b,
            s,
            card.look_labels[i],
            lr,
            Align2::LEFT_CENTER,
            12.0,
            W::Regular,
            p.ink3,
        );
        let st = crate::kit::State::of(&resp, is_open);
        let lead = crate::views::inspector::look::Lead::None;
        crate::views::inspector::look::field(b, s, fr, st, lead, card.looks[i], "", true);
    });
    if !is_open {
        return None;
    }
    let choices = part.choices();
    let names: Vec<String> = choices.iter().map(|(_, l)| t(l)).collect();
    let items: Vec<crate::kit::menu::Item<'_>> = names
        .iter()
        .zip(&choices)
        .map(|(l, (_, k))| crate::kit::menu::Item::Action {
            label: Src::Str(l),
            check: crate::kit::menu::Check::Radio(t(k) == card.looks[i]),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    let mut close = false;
    let chosen = crate::views::fx::frame::popup_list(
        ui.ctx(),
        id.with(("list", part.key())),
        pos2(fr.left(), fr.bottom() + s.s(2.0)),
        fr,
        s,
        &items,
        fr.width().max(s.s(160.0)),
        &mut close,
    );
    if let Some(j) = chosen {
        *open = None;
        return Some(choices[j].0);
    }
    if close && ui.ctx().cumulative_pass_nr() > *opened {
        *open = None;
    }
    None
}

/// For the tests: every room card drawn while noting, with the picture it stood in.
pub mod probe {
    use egui::Rect;
    use std::cell::RefCell;

    /// One card drawn: the picture, the card and its three fields.
    #[derive(Copy, Clone, Debug)]
    pub struct Seen {
        pub picture: Rect,
        pub card: Rect,
        pub fields: [Rect; 3],
    }

    thread_local! {
        static SEEN: RefCell<Option<Vec<Seen>>> = const { RefCell::new(None) };
    }

    /// Start noting the cards drawn on this thread.
    pub fn start() {
        SEEN.with(|s| *s.borrow_mut() = Some(Vec::new()));
    }

    /// The cards drawn since [`start`] (and stop noting).
    pub fn take() -> Vec<Seen> {
        SEEN.with(|s| s.borrow_mut().take()).unwrap_or_default()
    }

    pub(super) fn note(picture: Rect, shown: &super::Shown) {
        SEEN.with(|s| {
            if let Some(v) = s.borrow_mut().as_mut() {
                v.push(Seen {
                    picture,
                    card: shown.card,
                    fields: shown.fields,
                });
            }
        });
    }
}
