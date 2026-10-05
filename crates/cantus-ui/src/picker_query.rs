//! What the gallery shows (ADR 0029 §2): the catalogue filtered by what the picker was
//! opened for, the shelf chosen at the left and the words typed, then sorted — and, in
//! the curated order with nothing typed, grouped under each shelf's heading the way
//! Komplete Kontrol and Bitwig group a browser. Pure functions over the catalogue.

use crate::picker_catalog::{fold, score, Category, Device, Role, Source};
use crate::picker_prefs::{Prefs, Sort};

/// The shelf chosen in the left column.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Shelf {
    #[default]
    All,
    Favourites,
    Recent,
    Category(Category),
    Source(Source),
}

impl Shelf {
    pub fn key(self) -> &'static str {
        match self {
            Shelf::All => "picker.shelf.all",
            Shelf::Favourites => "picker.shelf.favourites",
            Shelf::Recent => "picker.shelf.recent",
            Shelf::Category(c) => c.key(),
            Shelf::Source(s) => s.key(),
        }
    }

    pub fn holds(self, d: &Device, prefs: &Prefs) -> bool {
        match self {
            Shelf::All => true,
            Shelf::Favourites => prefs.is_favourite(&d.key),
            Shelf::Recent => prefs.recency(&d.key).is_some(),
            Shelf::Category(c) => d.in_category(c),
            Shelf::Source(s) => d.source == s,
        }
    }
}

/// One question the gallery answers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// Instruments or effects; `None` (the browsing panel) shows both.
    pub role: Option<Role>,
    pub shelf: Shelf,
    pub text: String,
    pub sort: Sort,
}

/// A run of the result under one heading.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    /// `None`: no heading (a flat result).
    pub title: Option<Category>,
    /// Indices into the catalogue, in order.
    pub items: Vec<usize>,
}

/// The devices `q` asks for, as indices into `devices`, in display order.
pub fn run(devices: &[Device], q: &Query, prefs: &Prefs) -> Vec<usize> {
    let text = fold(q.text.trim());
    let mut hits: Vec<(usize, u32)> = devices
        .iter()
        .enumerate()
        .filter(|(_, d)| q.role.is_none_or(|r| d.role == r))
        .filter(|(_, d)| q.shelf.holds(d, prefs))
        .filter_map(|(i, d)| {
            if text.is_empty() {
                Some((i, 0))
            } else {
                score(d, &text).map(|s| (i, s))
            }
        })
        .collect();
    let name = |i: usize| fold(&devices[i].name);
    // Typing ranks by how well a device matches; the chosen order breaks ties.
    hits.sort_by(|&(a, sa), &(b, sb)| {
        sb.cmp(&sa).then_with(|| {
            let (da, db) = (&devices[a], &devices[b]);
            match q.sort {
                Sort::Curated => (da.role != Role::Instrument, da.order, name(a))
                    .cmp(&(db.role != Role::Instrument, db.order, name(b))),
                Sort::Name => name(a).cmp(&name(b)),
                Sort::Vendor => (fold(&da.vendor), name(a)).cmp(&(fold(&db.vendor), name(b))),
                Sort::Recent => (
                    prefs.recency(&da.key).unwrap_or(usize::MAX),
                    name(a),
                )
                    .cmp(&(prefs.recency(&db.key).unwrap_or(usize::MAX), name(b))),
            }
        })
    });
    // In the recent shelf, the order is the order of use whatever the sort says.
    if q.shelf == Shelf::Recent && text.is_empty() {
        hits.sort_by_key(|&(i, _)| prefs.recency(&devices[i].key).unwrap_or(usize::MAX));
    }
    hits.into_iter().map(|(i, _)| i).collect()
}

/// The result laid out under headings: grouped by primary shelf when the curated order
/// shows everything with nothing typed, one flat run otherwise.
pub fn groups(devices: &[Device], q: &Query, prefs: &Prefs) -> Vec<Group> {
    let items = run(devices, q, prefs);
    let grouped = q.sort == Sort::Curated
        && q.text.trim().is_empty()
        && matches!(q.shelf, Shelf::All | Shelf::Source(_));
    if !grouped {
        return vec![Group { title: None, items }];
    }
    let order: Vec<Category> = Category::INSTRUMENTS
        .iter()
        .chain(Category::EFFECTS.iter())
        .copied()
        .collect();
    let mut out: Vec<Group> = Vec::new();
    for c in order {
        // Natives first in their designed order, then hosted by name: `run` already did.
        let mine: Vec<usize> = items
            .iter()
            .copied()
            .filter(|&i| devices[i].primary() == c)
            .collect();
        if !mine.is_empty() {
            out.push(Group {
                title: Some(c),
                items: mine,
            });
        }
    }
    out
}

/// How many devices each shelf of the left column holds for `role`.
pub fn count(devices: &[Device], role: Option<Role>, shelf: Shelf, prefs: &Prefs) -> usize {
    devices
        .iter()
        .filter(|d| role.is_none_or(|r| d.role == r))
        .filter(|d| shelf.holds(d, prefs))
        .count()
}

/// The shelves of the left column for `role`, in order: the user's two, the categories,
/// then where devices come from. Empty categories are left out.
pub fn shelves(devices: &[Device], role: Option<Role>) -> Vec<Shelf> {
    let mut out = vec![Shelf::All, Shelf::Favourites, Shelf::Recent];
    let cats: Vec<Category> = match role {
        Some(Role::Instrument) => Category::INSTRUMENTS.to_vec(),
        Some(Role::Effect) => Category::EFFECTS.to_vec(),
        None => Category::INSTRUMENTS
            .iter()
            .chain(Category::EFFECTS.iter())
            .copied()
            .collect(),
    };
    let fits = |d: &&Device| role.is_none_or(|r| d.role == r);
    for c in cats {
        if devices.iter().filter(fits).any(|d| d.in_category(c)) {
            out.push(Shelf::Category(c));
        }
    }
    for s in [Source::Native, Source::Clap, Source::Vst3] {
        if devices.iter().filter(fits).any(|d| d.source == s) {
            out.push(Shelf::Source(s));
        }
    }
    out
}
