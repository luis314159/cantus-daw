//! The matrix's model: which rows and columns it shows and what each cell holds, derived
//! from the project **once per revision** (and per filter), never per frame.
//!
//! Rows are the sources — every strip in desk order (left dock, middle, right), then every
//! track; columns are the destinations — the master first, then the strips in desk order,
//! with a divider where the dock changes. A strip × strip cell holds the row's output (if
//! it goes there), its send there (level, switch, tap, and whether the destination hears
//! the row twice), how many of the column's inserts are keyed from the row, and — for an
//! empty cell — whether a send there is refused (a loop, or no room). A track × strip
//! cell holds whether the track plays into that strip (one per row, a radio) and how many
//! of its inserts hear the track's notes (the owner's answer 8).
//!
//! **One answer to "can this route be made?"** is `Project::route_check`. Asking it for
//! every cell would walk the graph once per cell (256 strips: 65 000 walks), so the loop
//! refusal is worked out here from one reachability set per strip — and a test holds that
//! the two agree on every cell of random graphs. The hint asks `route_check` itself for the
//! one cell under the pointer, which also names the strip that closes a loop.

use crate::routing_actions::strip_name;
use cantus_core::{routing, Dock, Project, SendTap, StripId, TrackId, MAX_SENDS};
use std::collections::HashMap;

/// A row of the matrix: a source.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Row {
    Strip(StripId),
    Track(TrackId),
}

/// A column of the matrix: a destination.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Col {
    Master,
    Strip(StripId),
}

impl Col {
    /// The destination as the routing calls name it ([`StripId::MASTER`] for the master).
    pub fn id(self) -> StripId {
        match self {
            Col::Master => StripId::MASTER,
            Col::Strip(s) => s,
        }
    }
}

/// A send as its cell shows it.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SendMark {
    pub level: f32,
    pub enabled: bool,
    pub tap: SendTap,
    /// The row's output already reaches the column: it hears the row twice (answer 5).
    pub doubled: bool,
}

/// Why an empty strip cell takes no send.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// The column already reaches the row: a send there would close a loop.
    Loop,
    /// The row has all its sends.
    NoRoom,
}

/// What one cell holds.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Cell {
    /// A strip row: its output goes here. A track row: the track plays into this column.
    pub output: bool,
    pub send: Option<SendMark>,
    /// The column's inserts keyed from the row.
    pub keys: u8,
    /// A track row: the column's inserts that hear the track's notes.
    pub notes: u8,
    /// An empty strip cell a send cannot be made into.
    pub refused: Option<Refused>,
    /// The row and the column are the same strip.
    pub itself: bool,
}

impl Cell {
    /// Holds a route (an output, a send or a key — or, on a track row, the track's strip).
    pub fn routed(&self) -> bool {
        self.output || self.send.is_some() || self.keys > 0 || self.notes > 0
    }
}

/// A row's or a column's header: its number (from 1; 0 for the master) and its name, and
/// the colour it wears (a strip's own, its first track's or its group's).
#[derive(Clone, Debug, PartialEq)]
pub struct Head {
    pub number: usize,
    pub name: String,
    pub color: Option<cantus_core::Color>,
}

/// The matrix for one project revision and filter.
#[derive(Debug, Default)]
pub struct Grid {
    key: Option<(u64, bool)>,
    pub rows: Vec<Row>,
    pub row_heads: Vec<Head>,
    /// The first `strip_rows` rows are strips; the rest are tracks.
    pub strip_rows: usize,
    pub cols: Vec<Col>,
    pub col_heads: Vec<Head>,
    /// Per column, its dock (`None`: the master).
    pub col_dock: Vec<Option<Dock>>,
    /// Row-major, `rows.len() × cols.len()`.
    pub cells: Vec<Cell>,
    /// Strips in the project, and how many the filter hides.
    pub strips: usize,
    pub hidden: usize,
    /// Routes drawn other than the implied master outputs (outputs to strips, sends, keys).
    pub routes: usize,
}

impl Grid {
    /// Bring the grid up to date with the project; `true` when it was rebuilt.
    pub fn refresh(&mut self, p: &Project, revision: u64, in_use: bool) -> bool {
        if self.key == Some((revision, in_use)) {
            return false;
        }
        self.build(p, in_use);
        self.key = Some((revision, in_use));
        true
    }

    /// Rebuild whatever was there (the next [`Grid::refresh`] rebuilds too).
    pub fn invalidate(&mut self) {
        self.key = None;
    }

    pub fn cell(&self, r: usize, c: usize) -> &Cell {
        &self.cells[r * self.cols.len() + c]
    }

    pub fn row_of(&self, row: Row) -> Option<usize> {
        self.rows.iter().position(|r| *r == row)
    }

    pub fn col_of(&self, col: Col) -> Option<usize> {
        self.cols.iter().position(|c| *c == col)
    }

    /// Whether a divider stands before column `c` (the dock changes there; the master
    /// column has one after it).
    pub fn divider_before(&self, c: usize) -> bool {
        c > 0 && self.col_dock.get(c) != self.col_dock.get(c - 1)
    }

    pub fn build(&mut self, p: &Project, in_use: bool) {
        let n = p.strips.len();
        let index: HashMap<StripId, usize> = p
            .strips
            .iter()
            .enumerate()
            .map(|(i, s)| (s.id, i))
            .collect();
        // The processing order's edges — outputs, sends and keys — for the loop test.
        let mut next: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut keys_something = vec![false; n];
        for (i, s) in p.strips.iter().enumerate() {
            if let Some(o) = s.output.and_then(|o| index.get(&o)) {
                next[i].push(*o);
            }
            for x in &s.sends {
                if let Some(t) = index.get(&x.to) {
                    next[i].push(*t);
                }
            }
            for e in &s.effects {
                if let Some(k) = e.sidechain.and_then(|k| index.get(&k)) {
                    next[*k].push(i);
                    keys_something[*k] = true;
                }
            }
        }
        for e in &p.master.effects {
            if let Some(k) = e.sidechain.and_then(|k| index.get(&k)) {
                keys_something[*k] = true;
            }
        }
        let words = n.div_ceil(64).max(1);
        let mut reach = vec![0u64; n * words];
        let mut stack = Vec::new();
        for i in 0..n {
            let set = &mut reach[i * words..(i + 1) * words];
            stack.clear();
            stack.extend_from_slice(&next[i]);
            while let Some(j) = stack.pop() {
                if set[j / 64] & (1 << (j % 64)) == 0 {
                    set[j / 64] |= 1 << (j % 64);
                    stack.extend_from_slice(&next[j]);
                }
            }
        }
        let reaches =
            |from: usize, to: usize| reach[from * words + to / 64] & (1 << (to % 64)) != 0;
        let fed: Vec<bool> = p.strips.iter().map(|s| routing::is_fed(p, s.id)).collect();
        let used = |i: usize| {
            let s = &p.strips[i];
            s.output.is_some() || !s.sends.is_empty() || keys_something[i] || fed[i]
        };
        let order: Vec<usize> = Dock::ALL
            .iter()
            .flat_map(|d| (0..n).filter(move |i| p.strips[*i].dock == *d))
            .collect();
        // *In use*: a row for every strip that routes somewhere other than the master or
        // is fed; a column for every strip something feeds (a destination in use).
        let shown: Vec<usize> = order
            .iter()
            .copied()
            .filter(|i| !in_use || used(*i))
            .collect();
        let shown_cols: Vec<usize> = order
            .iter()
            .copied()
            .filter(|i| !in_use || fed[*i])
            .collect();
        self.strips = n;
        self.hidden = n - shown.len().max(shown_cols.len());
        let head_of = |i: usize| {
            let s = &p.strips[i];
            let first = p.tracks.iter().find(|t| t.strip == Some(s.id));
            Head {
                number: i + 1,
                name: strip_name(p, s.id),
                color: s
                    .color
                    .or_else(|| first.map(|t| t.color))
                    .or_else(|| s.group.and_then(|g| p.strip_group(g)).map(|g| g.color)),
            }
        };
        self.rows.clear();
        self.row_heads.clear();
        self.cols.clear();
        self.col_heads.clear();
        self.col_dock.clear();
        for &i in &shown {
            self.rows.push(Row::Strip(p.strips[i].id));
            self.row_heads.push(head_of(i));
        }
        self.strip_rows = self.rows.len();
        for (ti, t) in p.tracks.iter().enumerate() {
            self.rows.push(Row::Track(t.id));
            self.row_heads.push(Head {
                number: ti + 1,
                name: t.name.clone(),
                color: Some(t.color),
            });
        }
        self.cols.push(Col::Master);
        self.col_heads.push(Head {
            number: 0,
            name: strip_name(p, StripId::MASTER),
            color: None,
        });
        self.col_dock.push(None);
        for &i in &shown_cols {
            self.cols.push(Col::Strip(p.strips[i].id));
            self.col_heads.push(head_of(i));
            self.col_dock.push(Some(p.strips[i].dock));
        }
        self.routes = 0;
        self.cells.clear();
        let col_strips: Vec<Option<usize>> = self
            .cols
            .iter()
            .map(|c| match c {
                Col::Master => None,
                Col::Strip(id) => index.get(id).copied(),
            })
            .collect();
        for &i in &shown {
            let src = &p.strips[i];
            for (c, col) in self.cols.iter().enumerate() {
                let mut cell = Cell::default();
                match (*col, col_strips[c]) {
                    (Col::Master, _) => {
                        cell.output = src.output.is_none();
                        cell.keys = count(p.master.effects.iter(), |e| e.sidechain == Some(src.id));
                    }
                    (Col::Strip(to), Some(j)) => {
                        let dst = &p.strips[j];
                        cell.itself = i == j;
                        cell.output = src.output == Some(to);
                        cell.send = src.send_to(to).map(|x| SendMark {
                            level: x.level,
                            enabled: x.enabled,
                            tap: x.tap,
                            doubled: routing::doubled(p, src.id, to),
                        });
                        cell.keys = count(dst.effects.iter(), |e| e.sidechain == Some(src.id));
                        if !cell.itself && cell.send.is_none() {
                            cell.refused = if reaches(j, i) {
                                Some(Refused::Loop)
                            } else if src.sends.len() >= MAX_SENDS {
                                Some(Refused::NoRoom)
                            } else {
                                None
                            };
                        }
                        self.routes += usize::from(cell.output)
                            + usize::from(cell.send.is_some())
                            + cell.keys as usize;
                    }
                    (Col::Strip(_), None) => {}
                }
                self.cells.push(cell);
            }
        }
        for t in &p.tracks {
            for (c, col) in self.cols.iter().enumerate() {
                let mut cell = Cell::default();
                let effects = match col_strips[c] {
                    None => &p.master.effects,
                    Some(j) => &p.strips[j].effects,
                };
                cell.output = match col {
                    Col::Master => t.strip.is_none(),
                    Col::Strip(id) => t.strip == Some(*id),
                };
                cell.notes = count(effects.iter(), |e| e.midi_from == Some(t.id));
                self.cells.push(cell);
            }
        }
    }
}

fn count<T>(items: impl Iterator<Item = T>, f: impl Fn(&T) -> bool) -> u8 {
    items.filter(|x| f(x)).count().min(u8::MAX as usize) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::routing::{edges, EdgeKind, RouteKind, RouteRefusal};
    use cantus_core::{EffectKind, Send};

    /// A small deterministic generator (the tests need no crate for it).
    struct Lcg(u64);
    impl Lcg {
        fn below(&mut self, n: usize) -> usize {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 33) as usize) % n.max(1)
        }
    }

    /// A random graph built only from routes `route_check` allows: outputs, sends (some
    /// pre-fader, some off, a strip with all sixteen) and keys.
    fn random_project(seed: u64) -> Project {
        let mut rng = Lcg(seed);
        let mut p = Project::default();
        for _ in 0..rng.below(6) {
            let s = p.make_strip("x");
            p.strips.push(s);
        }
        let ids: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
        for _ in 0..ids.len() * 3 {
            let (a, b) = (ids[rng.below(ids.len())], ids[rng.below(ids.len())]);
            match rng.below(3) {
                0 if p.route_check(a, b, RouteKind::Output).is_ok() => {
                    p.strip_mut(a).unwrap().output = Some(b);
                }
                1 if p.route_check(a, b, RouteKind::Send).is_ok() => {
                    let tap = if rng.below(3) == 0 {
                        SendTap::Pre
                    } else {
                        SendTap::Post
                    };
                    p.strip_mut(a).unwrap().sends.push(Send {
                        level: rng.below(100) as f32 / 100.0,
                        enabled: rng.below(5) != 0,
                        tap,
                        ..Send::new(b)
                    });
                }
                2 if p.route_check(a, b, RouteKind::Key(None)).is_ok() => {
                    let mut slot = p.make_effect(EffectKind::Compressor);
                    slot.sidechain = Some(a);
                    p.strip_mut(b).unwrap().effects.push(slot);
                }
                _ => {}
            }
        }
        // One strip with every send it may have, so "no room" shows somewhere.
        if seed.is_multiple_of(3) {
            let full = ids[0];
            for &to in &ids[1..] {
                if p.strip(full).unwrap().sends.len() >= MAX_SENDS {
                    break;
                }
                if p.route_check(full, to, RouteKind::Send).is_ok() {
                    p.strip_mut(full).unwrap().sends.push(Send::new(to));
                }
            }
            while p.strip(full).unwrap().sends.len() < MAX_SENDS {
                let s = p.make_strip("y");
                let id = s.id;
                p.strips.push(s);
                p.strip_mut(full).unwrap().sends.push(Send::new(id));
            }
        }
        p
    }

    /// The cell's refusal, worked out from reachability, is `route_check`'s answer for a
    /// send, on every strip cell of random graphs: the one answer, kept.
    #[test]
    fn refused_cells_agree_with_route_check() {
        let mut kinds = [0usize; 4];
        for seed in 0..40 {
            let p = random_project(seed);
            let mut g = Grid::default();
            g.build(&p, false);
            for r in 0..g.strip_rows {
                let Row::Strip(from) = g.rows[r] else {
                    unreachable!()
                };
                for (c, col) in g.cols.iter().enumerate() {
                    let cell = g.cell(r, c);
                    let Col::Strip(to) = *col else { continue };
                    let at = format!("seed {seed}, cell {r},{c}");
                    match p.route_check(from, to, RouteKind::Send) {
                        Err(RouteRefusal::ToItself) => assert!(cell.itself, "{at}"),
                        Err(RouteRefusal::Loop { .. }) => {
                            kinds[0] += 1;
                            assert_eq!(cell.refused, Some(Refused::Loop), "{at}")
                        }
                        Err(RouteRefusal::NoRoom) => {
                            kinds[1] += 1;
                            assert_eq!(cell.refused, Some(Refused::NoRoom), "{at}")
                        }
                        Err(RouteRefusal::AlreadyThere) => {
                            kinds[2] += 1;
                            assert!(cell.send.is_some(), "{at}")
                        }
                        Ok(()) => {
                            kinds[3] += 1;
                            assert_eq!(cell.refused, None, "{at}");
                            assert!(cell.send.is_none() && !cell.itself, "{at}");
                        }
                        Err(e) => panic!("{at}: unexpected {e:?}"),
                    }
                }
            }
        }
        assert!(kinds.iter().all(|k| *k > 0), "every case seen: {kinds:?}");
    }

    /// Every edge of the graph is in its cell, and nothing else is: outputs (the master's
    /// in the master column), sends with their level, switch and tap, keys counted.
    #[test]
    fn cells_mirror_the_edges() {
        for seed in 0..40 {
            let p = random_project(seed);
            let mut g = Grid::default();
            g.build(&p, false);
            let mut seen = 0usize;
            for e in edges(&p) {
                let r = g.row_of(Row::Strip(e.from)).expect("every strip has a row");
                let col = if e.to.is_master() {
                    Col::Master
                } else {
                    Col::Strip(e.to)
                };
                let c = g.col_of(col).expect("every destination has a column");
                let cell = g.cell(r, c);
                match e.kind {
                    EdgeKind::Output => assert!(cell.output, "seed {seed}: an output"),
                    EdgeKind::Send {
                        level,
                        enabled,
                        tap,
                    } => {
                        let m = cell.send.expect("a send");
                        assert_eq!((m.level, m.enabled, m.tap), (level, enabled, tap));
                    }
                    EdgeKind::Key { .. } => assert!(cell.keys > 0, "seed {seed}: a key"),
                }
                if !e.to.is_master() {
                    seen += 1;
                }
            }
            assert_eq!(seen, g.routes, "seed {seed}: routes counted once each");
            let drawn: usize = (0..g.strip_rows)
                .flat_map(|r| (1..g.cols.len()).map(move |c| (r, c)))
                .map(|(r, c)| {
                    let x = g.cell(r, c);
                    usize::from(x.output) + usize::from(x.send.is_some()) + x.keys as usize
                })
                .sum();
            assert_eq!(
                drawn, g.routes,
                "seed {seed}: no cell shows a route not there"
            );
        }
    }

    /// *In use* keeps the strips that route or are fed as rows, the fed ones as columns,
    /// and the master always; the tracks are always there, one strip each (a radio), with
    /// the notes an insert hears from them.
    #[test]
    fn in_use_hides_untouched_strips_and_tracks_are_a_radio() {
        let mut p = Project::default();
        let ids: Vec<StripId> = p.strips.iter().map(|s| s.id).collect();
        let mut t = p.make_track("Kick");
        t.strip = Some(ids[0]);
        let kick = t.id;
        p.tracks.push(t);
        p.strip_mut(ids[0]).unwrap().sends.push(Send::new(ids[5]));
        let mut gate = p.make_effect(EffectKind::Gate);
        gate.midi_from = Some(kick);
        p.strip_mut(ids[5]).unwrap().effects.push(gate);
        let mut g = Grid::default();
        g.build(&p, true);
        assert_eq!(
            g.rows,
            vec![Row::Strip(ids[0]), Row::Strip(ids[5]), Row::Track(kick)]
        );
        assert_eq!(
            g.cols,
            vec![Col::Master, Col::Strip(ids[0]), Col::Strip(ids[5])]
        );
        assert_eq!(g.hidden, p.strips.len() - 2);
        let r = g.row_of(Row::Track(kick)).unwrap();
        let lit: Vec<usize> = (0..g.cols.len()).filter(|c| g.cell(r, *c).output).collect();
        assert_eq!(lit, vec![1], "the track plays into its strip only");
        assert_eq!(g.cell(r, 2).notes, 1, "the gate hears the kick's notes");
        g.build(&p, false);
        assert_eq!(g.strip_rows, p.strips.len());
        assert_eq!(g.cols.len(), p.strips.len() + 1);
        // Desk order: left dock, middle, right.
        let rank = |d: &Option<Dock>| {
            d.map_or(0, |d| {
                1 + Dock::ALL.iter().position(|x| *x == d).unwrap_or(0)
            })
        };
        assert!(g.col_dock.windows(2).all(|w| rank(&w[0]) <= rank(&w[1])));
        assert!(g.divider_before(1), "the master stands apart");
    }
}
