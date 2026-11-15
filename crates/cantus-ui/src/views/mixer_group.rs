//! Bands on Cantus's desk (round 40 phase 3): an instrument's strips and a user's strip group
//! drawn as one system (design item 4 and the owner's `design-p3/groups` sheets, approved
//! 2026-11-12; ADR 0123 and ADR 0016 amended).
//!
//! - An **instrument's family**: its own strip and the strips its outputs play on (a
//!   Tympanum's mics, a split Pulsus's pads, Chorda's perspectives), and any strip the user
//!   **added** to it (`MixerStrip::attached`, schema 70: a bus fed only by the family, drawn
//!   at the band's end, marked as added). A solid bar in the track's colour, the instrument's
//!   glyph, *Drums · Tympanum · 3 mics*. Folded (the view's choice), its outputs are one strip.
//! - A **user's group** (ADR 0016): a thin line along the top in the group's colour, its name,
//!   its count at the band's end. Folded (`collapsed`, a command), it is one strip that names
//!   its members, shows their meters side by side and offers *Unfold* — never hidden.
//! - **One system, one rule** (the owner's amendment): a strip is in at most one family; a
//!   user group either sits **inside one family** (every member from it — a *sub-group*,
//!   drawn as a head under the family's band, as Tympanum's own mixer draws its families) or
//!   **outside families**, taking a whole family as one member. *Each mic to its own strip*
//!   makes a sub-group per output family; *All to Main* takes them away.
//! - Top-level bands stand apart from their neighbours by a 12-pt gap (the right dock's own
//!   device); a group band's right edge is dragged to take in, or give back, the strip beside
//!   it (one undo step; Esc cancels).
//!
//! Worked out with the desk (`mixer_desk`, once per revision): [`Families::build`] finds the
//! bands; [`lay`] lays a dock's items (gaps, folded members at their folded strip);
//! `mixer_group_paint` draws the bands and the folded strips.

#[path = "mixer_group_paint.rs"]
mod paint;

pub(super) use paint::{bands, folded, gap};

use super::desk::{Item, Place};
use crate::app::CantusApp;
use crate::i18n::tf;
use cantus_core::{Dock, GroupId, Project, StripId, TrackId};

/// A band's row, at 100 % (the desk's units); a sub-group's head row.
pub const BAND: f32 = 20.0;
pub const SUB: f32 = 17.0;

/// What a band is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// An instrument's family, by its track.
    Family(TrackId),
    /// A user's group: outside families (`sub` false), or inside one (`sub` true).
    Group { id: GroupId, sub: bool },
}

/// One band on the desk.
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub kind: Kind,
    /// Its members' project indices, in desk order.
    pub members: Vec<usize>,
    /// What folding gathers into one strip: a family's outputs' strips and the strips added
    /// to it (its own strip stays); a group's every member.
    pub folds: Vec<usize>,
    /// A family's strips added by the user (drawn at its end, marked).
    pub added: Vec<usize>,
    pub folded: bool,
    /// Its row: 0 the top, then down (a family inside a user group, a sub-group in a family).
    pub row: u8,
}

impl Band {
    /// The strip a folded band is drawn as: the first of what it folds.
    pub fn rep(&self) -> Option<usize> {
        self.folds.first().copied()
    }
}

/// The desk's bands, and which families are folded (the view's, kept across rebuilds; a
/// user group's fold is the song's `collapsed`).
#[derive(Clone, Debug, Default)]
pub struct Families {
    pub list: Vec<Band>,
    pub folds: std::collections::BTreeSet<u64>,
    /// How many rows the bands take.
    pub rows: u8,
    /// Hidden strip → the folded strip that stands for it.
    hide: std::collections::BTreeMap<usize, usize>,
    /// Strips added to a family that no longer come from it: (strip, track).
    pub left: Vec<(StripId, TrackId)>,
}

/// Who a strip belongs to: the track whose own strip it is, or whose output plays on it.
pub fn owner(p: &Project, id: StripId) -> Option<(TrackId, bool)> {
    p.tracks.iter().find_map(|t| {
        let outs = t.instrument.output_strips();
        if outs.contains(&id) {
            return Some((t.id, false));
        }
        (t.strip == Some(id) && !outs.is_empty()).then_some((t.id, true))
    })
}

/// The strips of `track`'s family: its own strip and its outputs' strips.
pub fn family_strips(p: &Project, track: TrackId) -> Vec<StripId> {
    let Some(t) = p.track(track) else {
        return Vec::new();
    };
    let mut v: Vec<StripId> = t.strip.into_iter().collect();
    v.extend(t.instrument.output_strips());
    v
}

/// Whether strip `id` is fed only by `track`'s family (outputs into it, sends to it): what
/// lets it be added to the family's band (schema 70).
pub fn fed_only_by(p: &Project, id: StripId, track: TrackId) -> bool {
    let fam = family_strips(p, track);
    if fam.contains(&id) || fam.len() < 2 {
        return false;
    }
    if p.tracks
        .iter()
        .any(|t| t.strip == Some(id) || t.instrument.feeds_strip(id))
    {
        return false;
    }
    let mut any = false;
    for s in &p.strips {
        if s.output == Some(id) || s.sends.iter().any(|x| x.to == id) {
            if !fam.contains(&s.id) {
                return false;
            }
            any = true;
        }
    }
    any
}

impl Families {
    /// Find the bands of `p`'s desk.
    pub fn build(&mut self, p: &Project) {
        self.list.clear();
        self.hide.clear();
        self.left.clear();
        // Families: consecutive strips of one dock owned by one instrument with at least one
        // output on a strip of its own, and the strips added to it that sit with them.
        for dock in [Dock::Left, Dock::Middle, Dock::Right] {
            let mut cur: Option<Band> = None;
            for (i, s) in p.strips.iter().enumerate() {
                if s.dock != dock || !s.on_desk() {
                    continue;
                }
                let own = owner(p, s.id);
                let added = s.attached.filter(|t| fed_only_by(p, s.id, *t));
                if let (Some(t), None) = (s.attached, added) {
                    if p.track(t).is_some() {
                        self.left.push((s.id, t));
                    }
                }
                match own.map(|o| o.0).or(added) {
                    Some(t) => {
                        if cur.as_ref().is_some_and(|f| f.kind != Kind::Family(t)) {
                            self.push(cur.take());
                        }
                        let f = cur.get_or_insert_with(|| Band {
                            kind: Kind::Family(t),
                            members: Vec::new(),
                            folds: Vec::new(),
                            added: Vec::new(),
                            folded: self.folds.contains(&t.0),
                            row: 0,
                        });
                        f.members.push(i);
                        match own {
                            None => {
                                f.added.push(i);
                                f.folds.push(i);
                            }
                            Some((_, false)) => f.folds.push(i),
                            Some((_, true)) => {}
                        }
                    }
                    None => self.push(cur.take()),
                }
            }
            self.push(cur.take());
        }
        // User groups: inside a family when every member is of it, else outside.
        for g in &p.strip_groups {
            let members: Vec<usize> = p
                .strips
                .iter()
                .enumerate()
                .filter(|(_, s)| s.group == Some(g.id) && s.on_desk())
                .map(|(i, _)| i)
                .collect();
            if members.is_empty() {
                continue;
            }
            let sub = self.list.iter().any(|f| {
                matches!(f.kind, Kind::Family(_)) && members.iter().all(|m| f.members.contains(m))
            });
            self.list.push(Band {
                kind: Kind::Group { id: g.id, sub },
                folds: members.clone(),
                members,
                added: Vec::new(),
                folded: g.collapsed,
                row: 0,
            });
        }
        self.rows_and_folds();
    }

    fn push(&mut self, f: Option<Band>) {
        // A family is a band when an output (or an added strip) sits with it.
        if let Some(f) = f.filter(|f| !f.folds.is_empty()) {
            self.list.push(f);
        }
    }

    /// Each band's row (outer groups over families over sub-groups, only the levels the desk
    /// has), and which strips folding hides behind which folded strip (an outer fold first).
    fn rows_and_folds(&mut self) {
        let level = |b: &Band, list: &[Band]| -> u8 {
            match b.kind {
                Kind::Group { sub: true, .. } => 2,
                Kind::Group { sub: false, .. } => 0,
                Kind::Family(_) => {
                    let inside = list.iter().any(|g| {
                        matches!(g.kind, Kind::Group { sub: false, .. })
                            && b.members.iter().all(|m| g.members.contains(m))
                    });
                    u8::from(inside)
                }
            }
        };
        let levels: Vec<u8> = self.list.iter().map(|b| level(b, &self.list)).collect();
        let mut present = levels.clone();
        present.sort_unstable();
        present.dedup();
        for (b, l) in self.list.iter_mut().zip(&levels) {
            b.row = present.iter().position(|x| x == l).unwrap_or(0) as u8;
        }
        self.rows = present.len() as u8;
        let mut order: Vec<usize> = (0..self.list.len()).collect();
        order.sort_by_key(|i| self.list[*i].row);
        for i in order {
            let b = &self.list[i];
            if !b.folded || b.folds.is_empty() {
                continue;
            }
            let rep = b.folds[0];
            if self.hide.contains_key(&rep) {
                continue;
            }
            for m in b.folds.iter().skip(1) {
                self.hide.entry(*m).or_insert(rep);
            }
        }
    }

    /// The folded strip that stands for strip `index`, when folding hides it.
    pub fn hidden_by(&self, index: usize) -> Option<usize> {
        self.hide.get(&index).copied()
    }

    /// Whether strip `index` is left off the desk.
    pub fn hidden(&self, index: usize) -> bool {
        self.hide.contains_key(&index)
    }

    /// The folded band strip `index` stands for, when it is one (the outermost).
    pub fn folded_at(&self, index: usize) -> Option<&Band> {
        self.list
            .iter()
            .filter(|b| b.folded && b.rep() == Some(index))
            .min_by_key(|b| b.row)
    }

    /// The top-level band strip `index` is in (the gaps go round these).
    pub fn top_of(&self, index: usize) -> Option<&Band> {
        self.list
            .iter()
            .filter(|b| {
                b.members.contains(&index) && !matches!(b.kind, Kind::Group { sub: true, .. })
            })
            .min_by_key(|b| b.row)
    }
}

/// The room the bands take above the strips.
pub fn band_h(d: &super::desk::Desk) -> f32 {
    let subs = d
        .fam
        .list
        .iter()
        .any(|b| matches!(b.kind, Kind::Group { sub: true, .. }));
    match d.fam.rows {
        0 => 0.0,
        n if subs => BAND * (n - 1) as f32 + SUB,
        n => BAND * n as f32,
    }
}

/// Lay one dock's items: a 12-pt gap before and after each top-level band (a user group's
/// first gap is its `Separator`, which carries the group), a folded band as its one strip,
/// the strips it folds placed at it.
pub fn lay(fam: &Families, p: &Project, dock: Dock, items: &mut Vec<Item>, place: &mut [Place]) {
    let (mut strips, mut seps) = (0u16, 0u16);
    let mut cur: Option<Kind> = None;
    for (i, s) in p.strips.iter().enumerate() {
        if s.dock != dock || !s.on_desk() {
            continue;
        }
        let top = fam.top_of(i).map(|b| b.kind);
        if top != cur {
            if cur.is_some() {
                items.push(Item::Gap);
                seps += 1;
            }
            match top {
                Some(Kind::Group { id, .. }) => {
                    items.push(Item::Separator(id));
                    seps += 1;
                }
                Some(Kind::Family(_)) if cur.is_none() && !items.is_empty() => {
                    items.push(Item::Gap);
                    seps += 1;
                }
                _ => {}
            }
            cur = top;
        }
        if let Some(rep) = fam.hidden_by(i) {
            place[i] = Place {
                folded: true,
                ..place[rep]
            };
            continue;
        }
        items.push(Item::Strip(s.id, i));
        place[i] = Place {
            dock,
            strips,
            seps,
            folded: false,
        };
        strips += 1;
    }
    if cur.is_some() {
        items.push(Item::Gap);
    }
}

/// A member's name everywhere but the desk: *Drums · Snare Top* (the instrument's track,
/// then the strip's own name); any other strip, its own name.
pub fn full_name(p: &Project, id: StripId) -> String {
    let Some(s) = p.strip(id) else {
        return String::new();
    };
    match owner(p, id) {
        Some((t, false)) => {
            let inst = p.track(t).map(|x| x.name.clone()).unwrap_or_default();
            tf(
                "inst_mixer.full_name",
                &[("inst", &inst), ("name", &s.name)],
            )
        }
        _ => s.name.clone(),
    }
}

/// The instrument's name in its band: its product name (*Tympanum*, *Chorda*, *Pulsus*).
pub fn instrument_word(p: &Project, t: TrackId) -> String {
    p.track(t)
        .map(|tr| crate::swap::instrument_name(&tr.instrument))
        .unwrap_or_default()
}

/// The strips a group made of `strips` holds under the one-band rule: when they all come
/// from one family, those (a sub-group); otherwise every family any of them is in, whole.
pub fn whole_families(p: &Project, strips: &[StripId]) -> Vec<StripId> {
    let fam_of = |s: StripId| {
        owner(p, s).map(|o| o.0).or_else(|| {
            p.strip(s)
                .and_then(|x| x.attached)
                .filter(|t| fed_only_by(p, s, *t))
        })
    };
    let homes: Vec<Option<TrackId>> = strips.iter().map(|s| fam_of(*s)).collect();
    if let Some(Some(h)) = homes.first() {
        if homes.iter().all(|x| *x == Some(*h)) {
            return strips.to_vec();
        }
    }
    let mut out: Vec<StripId> = Vec::new();
    for (s, home) in strips.iter().zip(&homes) {
        let add: Vec<StripId> = match home {
            Some(t) => {
                let mut v = family_strips(p, *t);
                v.extend(
                    p.strips
                        .iter()
                        .filter(|x| x.attached == Some(*t))
                        .map(|x| x.id),
                );
                v
            }
            None => vec![*s],
        };
        for a in add {
            if !out.contains(&a) {
                out.push(a);
            }
        }
    }
    out
}

/// Fold or unfold `track`'s family on the app's desk (the band's click does it; tests and
/// shots set it).
pub fn set_folded(app: &mut CantusApp, track: TrackId, folded: bool) {
    if app.mixer.desk.fam.folds.contains(&track.0) != folded {
        toggle(&mut app.mixer, track);
    }
}

/// Whether `track`'s family is folded on the app's desk.
pub fn is_folded(app: &CantusApp, track: TrackId) -> bool {
    app.mixer.desk.fam.folds.contains(&track.0)
}

/// Fold or unfold `track`'s family: the desk is worked out again.
pub(super) fn toggle(v: &mut super::MixerView, track: TrackId) {
    let mut folds = std::mem::take(&mut v.desk.fam.folds);
    if !folds.remove(&track.0) {
        folds.insert(track.0);
    }
    v.desk = super::desk::Desk::default();
    v.desk.fam.folds = folds;
}
