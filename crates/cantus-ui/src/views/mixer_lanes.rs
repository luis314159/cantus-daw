//! Where the band's cables hang, where their end marks sit, where each is grabbed and
//! which cable is under the pointer (round 26, ADR 0043 §2 as amended by three reviews):
//! pure geometry over the frame's drawn cables, no egui, no allocation once the scratch
//! lists have grown.
//!
//! **In this order.**
//!
//! 1. *The selection's lanes* (`Drawn::laned`: lit by the selection or a selected
//!    cable). Shortest first, each takes the lane below every laned cable its span
//!    overlaps, so a cable nested in another hangs above it and an output and a send
//!    between the same strips are two lines; the lanes share the band's depth, at least
//!    [`MIN_PITCH`] apart (the longest yield, faint, where the band holds no more). A lone
//!    laned cable keeps its span's depth. Nothing here depends on the pointer.
//! 2. *The selection's marks.* A cable's end mark rides up its destination's corner — at
//!    half its depth, or lower on the corner (never past its bottom) where that keeps it
//!    off the port's ring and clear of the marks already placed by a glyph's width
//!    ([`glyph_r`]: a dot, a chevron, a key). The selected cable first, then the
//!    selection's, shorter first; a mark with no such place is left out (the selected
//!    cable's never is).
//! 3. *The hover's cables* (lit, not laned), shortest first, each at the depth nearest
//!    the one its span asks for that keeps [`SEP_RUN`] from every lit run it overlaps,
//!    nests where the band has the depth (below the lit cables it contains, above those
//!    containing it), and **has a place for its mark** among those already placed at its
//!    port. A hover cable with no such depth yields: drawn in its colour but thin and
//!    without a mark (`Drawn::yielded`), so a hovered bus still shows everything feeding
//!    it, as many with marks as the band holds apart.
//! 4. *The faint cables' marks*, where they fit.
//! 5. *Grab areas*: every lit cable's shown mark (not at an edge chip) gets a square that
//!    holds the mark, keeps clear of every other shown mark's glyph and of every other
//!    area, and stays off every port's ring (a press on a port always starts a new cable)
//!    — the selected cable's, and its tail's, excepted from the port only. A lit mark
//!    that cannot have one is left out.
//!
//! **The hit test.** The nearest cable within reach; the layer it is drawn in (faint, the
//! hover's, the selection's) only breaks a tie (cables within [`TIE`] of each other),
//! then the shorter: a cable is always the one picked on its own run.

use super::cables::Drawn;
use crate::kit::cable;
use cantus_core::routing::RouteKind;
use egui::{pos2, Pos2, Rect, Vec2};

/// How near the pointer must come for a cable to be hovered.
pub(super) const HIT: f32 = 4.0;
/// Two cables this near the pointer are a tie, which the layer drawn on top breaks.
pub(super) const TIE: f32 = 0.05;
/// The least distance between the flat runs of two lit cables whose spans overlap, for
/// the hover's cables.
pub(super) const SEP_RUN: f32 = 1.5;
/// The least pitch of the selection's lanes.
pub(super) const MIN_PITCH: f32 = 0.6;
/// Clear space between two marks' glyphs.
pub(super) const GLYPH_CLEAR: f32 = 1.0;
/// A port's hit area: 14 pt across, from 7 pt above its centre to its ring's foot.
pub(super) const PORT_HALF: f32 = 7.0;
pub(super) const PORT_RING: f32 = 3.5;
/// A grab area's half size: at most, and the least worth having.
pub(super) const SENSOR_MAX: f32 = 5.0;
pub(super) const SENSOR_MIN: f32 = 1.5;
/// The room the selected cable's tail keeps around it: its least grab area's corner.
const TAIL_R: f32 = SENSOR_MIN * std::f32::consts::SQRT_2 + 0.1;

/// A port's hit area, centred on `c`.
pub(super) fn port_rect(c: Pos2) -> Rect {
    Rect::from_min_max(
        pos2(c.x - PORT_HALF, c.y - PORT_HALF),
        pos2(c.x + PORT_HALF, c.y + PORT_RING),
    )
}

/// How far a mark's glyph reaches from its centre, by kind (kit/cable.rs `end_mark`): an
/// output's dot, a send's chevron, a key's glyph.
pub(super) fn glyph_r(d: &Drawn) -> f32 {
    match d.route.kind {
        RouteKind::Output => 2.8,
        RouteKind::Send => 4.4,
        RouteKind::Key(_) => 6.0,
    }
}

/// The scratch lists, kept between frames (their capacity).
#[derive(Debug, Default)]
pub(super) struct Scratch {
    order: Vec<u32>,
    lane: Vec<u8>,
    excl: Vec<(f32, f32)>,
    /// The marks placed so far: the drawn cable's index.
    shown: Vec<u32>,
    /// The selected cable's tail this frame: no mark comes near it (its grab area).
    tail: Option<Pos2>,
    /// The ports this frame (their centres): no grab area lies on one.
    pub(super) ports: Vec<Pos2>,
    /// Each grab area this frame: the drawn cable's index, whether it is the selected
    /// cable's tail, and its area.
    pub(super) sensors: Vec<(u32, bool, Rect)>,
}

/// The layer a cable is painted (and hit) in: 0 faint, 1 lit by the hover alone, 2 the
/// selection's.
pub(super) fn layer(d: &Drawn) -> u8 {
    match (d.lit, d.laned) {
        (_, true) => 2,
        (true, false) => 1,
        (false, false) => 0,
    }
}

fn span(d: &Drawn) -> (f32, f32) {
    (d.from.x.min(d.to.x), d.from.x.max(d.to.x))
}

/// Whether two cables' flat runs would lie over each other (their spans overlap).
pub(super) fn overlap(a: &Drawn, b: &Drawn) -> bool {
    let ((alo, ahi), (blo, bhi)) = (span(a), span(b));
    alo < bhi - 0.5 && ahi > blo + 0.5
}

/// Whether `a`'s span holds `b`'s and is longer.
pub(super) fn contains(a: &Drawn, b: &Drawn) -> bool {
    let ((alo, ahi), (blo, bhi)) = (span(a), span(b));
    alo <= blo + 0.5 && ahi >= bhi - 0.5 && (ahi - alo) > (bhi - blo) + 0.5
}

/// The point of `d`'s destination corner at height `y` (between its port and its run).
fn on_corner(d: &Drawn, y: f32) -> Pos2 {
    let dir = if d.to.x >= d.from.x { 1.0 } else { -1.0 };
    let span = (d.to.x - d.from.x).abs();
    let depth = d.depth.max(0.0);
    let r = depth.min(span * 0.5);
    let v1 = (d.from.y.max(d.to.y) + depth - d.to.y).max(1e-3);
    let sin = ((y - d.to.y) / v1).clamp(0.0, 1.0);
    let cos = (1.0 - sin * sin).max(0.0).sqrt();
    let c1 = d.to.x - dir * r;
    pos2(c1 + dir * r * cos, d.to.y + v1 * sin)
}

/// Where `d`'s mark can sit with the marks in `shown` (indices into `drawn`) already
/// placed: half its depth up the corner if that is clear, else the nearest clear height
/// lower on it, off the port's ring.
fn mark_place(drawn: &[Drawn], shown: &[u32], tail: Option<Pos2>, d: &Drawn) -> Option<Pos2> {
    let r = glyph_r(d);
    let top = d.to.y + PORT_RING + SENSOR_MIN;
    let nominal = d.to.y + d.depth * 0.5;
    let bottom = d.to.y + d.depth;
    let clear = |p: Pos2| {
        tail.is_none_or(|t| t.distance(p) >= r + TAIL_R + GLYPH_CLEAR)
            && shown.iter().all(|j| {
                let o = &drawn[*j as usize];
                o.mark_at.distance(p) >= r + glyph_r(o) + GLYPH_CLEAR
            })
    };
    let start = nominal.max(top);
    if start > bottom + 1e-3 {
        return None;
    }
    let mut y = start;
    while y <= bottom + 1e-3 {
        let p = on_corner(d, y);
        if clear(p) {
            return Some(p);
        }
        y += 0.5;
    }
    None
}

/// Every pass of the placement (module docs), in a band whose cables hang between the
/// floor and `room` points below the ports. `selected` is the selected cable's index.
pub(super) fn place(drawn: &mut [Drawn], s: &mut Scratch, room: f32, selected: Option<usize>) {
    let floor = cable::MIN_DEPTH.min(room);
    for d in drawn.iter_mut() {
        d.mark = false;
        d.yielded = false;
        d.relaxed = false;
    }
    s.shown.clear();
    selection_lanes(drawn, s, floor, room);
    s.tail = selected
        .filter(|i| drawn[*i].lit && !drawn[*i].chips[0])
        .map(|i| cable::path(drawn[i].from, drawn[i].to, drawn[i].depth).tail);
    place_marks(drawn, s, selected, |d, i| Some(i) == selected || d.laned);
    hover_depths(drawn, s, floor, room);
    place_marks(drawn, s, None, |d, _| !d.lit && !d.yielded);
    sensors(drawn, s, selected);
}

fn by_span(drawn: &[Drawn], order: &mut [u32]) {
    order.sort_unstable_by(|a, b| {
        let (da, db) = (&drawn[*a as usize], &drawn[*b as usize]);
        let (sa, sb) = (span(da), span(db));
        (sa.1 - sa.0)
            .total_cmp(&(sb.1 - sb.0))
            .then(da.edge.cmp(&db.edge))
    });
}

/// The marks of the cables `which` picks, the selected one first, then shorter first.
fn place_marks(
    drawn: &mut [Drawn],
    s: &mut Scratch,
    selected: Option<usize>,
    which: impl Fn(&Drawn, usize) -> bool,
) {
    s.order.clear();
    s.order.extend(
        drawn
            .iter()
            .enumerate()
            .filter(|(i, d)| !d.mark && which(d, *i))
            .map(|(i, _)| i as u32),
    );
    by_span(drawn, &mut s.order);
    if let Some(k) = selected.and_then(|i| s.order.iter().position(|j| *j as usize == i)) {
        let first = s.order.remove(k);
        s.order.insert(0, first);
    }
    for k in 0..s.order.len() {
        let i = s.order[k] as usize;
        let place = mark_place(drawn, &s.shown, s.tail, &drawn[i]);
        let place = if Some(i) == selected {
            // The selected cable always shows its mark.
            place.or(Some(on_corner(
                &drawn[i],
                drawn[i].to.y + drawn[i].depth * 0.5,
            )))
        } else {
            place
        };
        if let Some(p) = place {
            drawn[i].mark_at = p;
            drawn[i].mark = true;
            s.shown.push(i as u32);
        }
    }
}

fn selection_lanes(drawn: &mut [Drawn], s: &mut Scratch, floor: f32, room: f32) {
    s.order.clear();
    s.order.extend(
        drawn
            .iter()
            .enumerate()
            .filter(|(_, d)| d.laned)
            .map(|(i, _)| i as u32),
    );
    if s.order.len() < 2 {
        return;
    }
    by_span(drawn, &mut s.order);
    s.lane.clear();
    s.lane.resize(drawn.len(), 0);
    let mut most = 1u8;
    for k in 0..s.order.len() {
        let d = drawn[s.order[k] as usize];
        let mut lane = 1u8;
        for j in 0..k {
            let o = drawn[s.order[j] as usize];
            if overlap(&d, &o) {
                lane = lane.max(s.lane[s.order[j] as usize].saturating_add(1));
            }
        }
        s.lane[s.order[k] as usize] = lane;
        most = most.max(lane);
    }
    if most < 2 {
        return;
    }
    // The lanes the band holds at MIN_PITCH apart; the highest beyond them yield.
    let fit = ((room - floor) / MIN_PITCH).floor().max(0.0) as usize + 1;
    let used = most.min(fit.min(255) as u8);
    for &i in &s.order {
        let lane = s.lane[i as usize];
        let d = &mut drawn[i as usize];
        if lane > used {
            d.lit = false;
            d.laned = false;
            continue;
        }
        d.depth = if used < 2 {
            floor
        } else {
            floor + (room - floor) * f32::from(lane - 1) / f32::from(used - 1)
        };
    }
}

fn hover_depths(drawn: &mut [Drawn], s: &mut Scratch, floor: f32, room: f32) {
    s.order.clear();
    s.order.extend(
        drawn
            .iter()
            .enumerate()
            .filter(|(_, d)| d.lit && !d.laned)
            .map(|(i, _)| i as u32),
    );
    if s.order.is_empty() {
        return;
    }
    by_span(drawn, &mut s.order);
    for k in 0..s.order.len() {
        let i = s.order[k] as usize;
        let h = drawn[i];
        // A short send or key may hang above the floor (its kind's share of it): so may
        // its hover placement.
        let base = floor.min(h.depth);
        let (mut lo, mut hi) = (base, room);
        s.excl.clear();
        let mut against = |o: &Drawn, excl: &mut Vec<(f32, f32)>| {
            if !overlap(&h, o) {
                return;
            }
            excl.push((o.depth - SEP_RUN, o.depth + SEP_RUN));
            // Nest where the band has the depth for it, pair by pair.
            if contains(&h, o) && o.depth + SEP_RUN <= room {
                lo = lo.max(o.depth + SEP_RUN);
            } else if contains(o, &h) && o.depth - SEP_RUN >= base {
                hi = hi.min(o.depth - SEP_RUN);
            }
        };
        for o in drawn.iter().filter(|o| o.laned) {
            against(o, &mut s.excl);
        }
        for &j in &s.order[..k] {
            let o = drawn[j as usize];
            if o.lit {
                against(&o, &mut s.excl);
            }
        }
        // The depth nearest its span's that is clear of the runs and leaves its mark a
        // place at its port; nested if it can be, else only separated, else it yields.
        let mut found = None;
        for (a, b, relaxed) in [(lo, hi, false), (base, room, true)] {
            found = nearest_with_mark(drawn, s, i, a, b, h.depth);
            if found.is_some() {
                drawn[i].relaxed = relaxed;
                break;
            }
        }
        match found {
            Some((depth, mark)) => {
                let d = &mut drawn[i];
                d.depth = depth;
                d.mark_at = mark;
                d.mark = true;
                s.shown.push(i as u32);
            }
            None => {
                let d = &mut drawn[i];
                d.lit = false;
                d.yielded = true;
            }
        }
    }
}

/// The depth in `[lo, hi]` nearest `want`, outside every interval of `s.excl`, at which
/// cable `i`'s mark has a place among the marks shown — with that place.
fn nearest_with_mark(
    drawn: &[Drawn],
    s: &Scratch,
    i: usize,
    lo: f32,
    hi: f32,
    want: f32,
) -> Option<(f32, Pos2)> {
    if lo > hi + 1e-4 {
        return None;
    }
    let free = |d: f32| {
        d >= lo - 1e-4
            && d <= hi + 1e-4
            && s.excl.iter().all(|(a, b)| d <= *a + 1e-4 || d >= *b - 1e-4)
    };
    let start = want.clamp(lo, hi);
    let try_at = |d: f32| -> Option<(f32, Pos2)> {
        if !free(d) {
            return None;
        }
        let mut c = drawn[i];
        c.depth = d;
        mark_place(drawn, &s.shown, s.tail, &c).map(|p| (d, p))
    };
    // Outward from the depth its span asks for, a point at a time, both ways; the
    // boundaries of the clear stretches too (a run's edge is often the nearest place).
    if let Some(f) = try_at(start) {
        return Some(f);
    }
    let reach = (hi - lo).max(0.0);
    let mut step = 1.0;
    while step <= reach + 1.0 {
        for d in [start - step, start + step] {
            if d >= lo - 1e-4 && d <= hi + 1e-4 {
                if let Some(f) = try_at(d.clamp(lo, hi)) {
                    return Some(f);
                }
            }
        }
        step += 1.0;
    }
    for &(a, b) in &s.excl {
        for d in [a, b] {
            if let Some(f) = try_at(d) {
                return Some(f);
            }
        }
    }
    None
}

/// Every lit cable's grab area for its shown mark (and the selected cable's tail): each
/// holds its mark, keeps clear of every other shown mark's glyph and every other area,
/// and stays off the ports (the selected cable's excepted). A lit mark that cannot have
/// one is left out.
fn sensors(drawn: &mut [Drawn], s: &mut Scratch, selected: Option<usize>) {
    s.sensors.clear();
    // The points that want an area: each lit shown mark not at a chip, then the selected
    // cable's tail.
    let tail = selected.zip(s.tail);
    let wants = |d: &Drawn| d.lit && d.mark && !d.chips[1];
    for pass in 0..2 {
        s.sensors.clear();
        let n = drawn.len();
        for i in 0..n + 1 {
            let (at, is_tail, idx) = if i < n {
                if !wants(&drawn[i]) {
                    continue;
                }
                (drawn[i].mark_at, false, i)
            } else if let Some((t, p)) = tail {
                (p, true, t)
            } else {
                continue;
            };
            let exempt = Some(idx) == selected;
            // Clear of every other shown mark's glyph (the square's corner too) and half
            // the way to every other point wanting an area.
            let mut half = SENSOR_MAX;
            for (j, o) in drawn.iter().enumerate() {
                if !o.mark || (j == idx && !is_tail) {
                    continue;
                }
                let gap = o.mark_at.distance(at) - glyph_r(o);
                half = half.min(gap / std::f32::consts::SQRT_2);
                if wants(o) {
                    let q = o.mark_at;
                    let linf = (q.x - at.x).abs().max((q.y - at.y).abs());
                    half = half.min(linf * 0.5 - 0.01);
                }
            }
            if let Some((_, p)) = tail.filter(|_| !is_tail) {
                let linf = (p.x - at.x).abs().max((p.y - at.y).abs());
                half = half.min(linf * 0.5 - 0.01);
            }
            // Off every port's hit area.
            if !exempt {
                for c in &s.ports {
                    let pr = port_rect(*c);
                    let dx = (pr.min.x - at.x).max(at.x - pr.max.x);
                    let dy = (pr.min.y - at.y).max(at.y - pr.max.y);
                    let outside = dx.max(dy);
                    half = half.min(outside - 0.01);
                }
            }
            if half < SENSOR_MIN - 0.02 && !(exempt && half > 0.25) {
                if pass == 0 && !is_tail && Some(idx) != selected {
                    // No area for it: the mark is left out (and the others may grow).
                    drawn[idx].mark = false;
                }
                continue;
            }
            s.sensors.push((
                idx as u32,
                is_tail,
                Rect::from_center_size(at, Vec2::splat(half * 2.0)),
            ));
        }
    }
}

/// The cable under `p`: the nearest within reach. Only a tie — cables within
/// [`TIE`] of each other there, lying on one another — goes to the layer drawn on top,
/// then to the shorter, the more particular: a layer never beats a nearer cable.
pub(super) fn hot(drawn: &[Drawn], p: Pos2) -> Option<usize> {
    let mut best: Option<(usize, f32, u8, f32)> = None;
    for (k, d) in drawn.iter().enumerate() {
        let dist = cable::hit(d.from, d.to, d.depth, p);
        if dist >= HIT {
            continue;
        }
        let l = layer(d);
        let (lo, hi) = span(d);
        let len = hi - lo;
        let better = best.is_none_or(|(_, bd, bl, blen)| {
            if (dist - bd).abs() > TIE {
                dist < bd
            } else {
                l > bl || (l == bl && len < blen)
            }
        });
        if better {
            best = Some((k, dist, l, len));
        }
    }
    best.map(|(k, _, _, _)| k)
}

#[cfg(test)]
mod tests {
    //! The placement's promises, held over random desks: strip counts (4–64) and pitches,
    //! outputs, sends and keys (shared ports, nested and doubled spans), buses fed by 8–32
    //! strips, selections (and a selected cable), a hovered strip, *All* and *Selected*,
    //! every band height from its least to its most.
    use super::*;
    use cantus_core::routing::Route;
    use cantus_core::{EffectId, StripId};

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
    }

    const PORT_Y: f32 = 100.0;

    #[derive(Clone, Copy, PartialEq)]
    enum Kind {
        Output,
        Send,
        Key,
    }

    struct Desk {
        /// Each port's x (strip i), the master's last.
        ports: Vec<f32>,
        edges: Vec<(usize, usize, Kind)>,
        selected: Vec<usize>,
        /// A bus fed by many strips (hovered, when there is one).
        bus: Option<usize>,
        all: bool,
        band: f32,
    }

    fn desk(seed: u64) -> Desk {
        let mut r = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let busy = r.below(3) == 0;
        let n = if busy {
            24 + r.below(41) as usize
        } else {
            4 + r.below(37) as usize
        };
        let pitch = [24.0, 48.0, 76.0][r.below(3) as usize];
        let mut ports: Vec<f32> = (0..n).map(|i| 200.0 + i as f32 * pitch).collect();
        ports.push(120.0);
        let master = n;
        // Routing never loops: every route goes up a random order of the strips (a bus
        // may sit left or right of what feeds it).
        let mut rank: Vec<u64> = (0..n).map(|_| r.next()).collect();
        let bus = busy.then(|| r.below(n as u64) as usize);
        if let Some(b) = bus {
            rank[b] = u64::MAX - 1;
        }
        rank.push(u64::MAX);
        let up = |a: usize, b: usize| rank[a] < rank[b];
        let mut edges: Vec<(usize, usize, Kind)> = Vec::new();
        if let Some(b) = bus {
            // 8 to 32 inputs: outputs and sends into the bus.
            let inputs = (8 + r.below(25) as usize).min(n - 1);
            for i in (0..n).filter(|i| *i != b).take(inputs) {
                let kind = if r.below(2) == 0 {
                    Kind::Output
                } else {
                    Kind::Send
                };
                edges.push((i, b, kind));
            }
        }
        for i in 0..n {
            if !edges.iter().any(|(f, _, k)| *f == i && *k == Kind::Output) {
                let out = if r.below(2) == 0 {
                    master
                } else {
                    r.below(n as u64) as usize
                };
                if out != i && up(i, out) {
                    edges.push((i, out, Kind::Output));
                }
            }
            for _ in 0..r.below(4) {
                let to = r.below(n as u64) as usize;
                let kind = if r.below(5) == 0 {
                    Kind::Key
                } else {
                    Kind::Send
                };
                if to != i && up(i, to) && !edges.iter().any(|(f, t, _)| *f == i && *t == to) {
                    edges.push((i, to, kind));
                }
            }
        }
        let mut selected = Vec::new();
        for _ in 0..1 + r.below(3) {
            let s = r.below(n as u64) as usize;
            if !selected.contains(&s) && Some(s) != bus {
                selected.push(s);
            }
        }
        Desk {
            ports,
            edges,
            selected,
            bus,
            all: r.below(2) == 0,
            band: 20.0 + r.below(45) as f32,
        }
    }

    fn room(band: f32) -> f32 {
        (band - 13.5).max(4.0)
    }

    /// What `mixer_cables::collect` draws for this desk, `hovered` lit.
    fn drawn(d: &Desk, hovered: Option<usize>) -> Vec<Drawn> {
        let master = d.ports.len() - 1;
        let room = room(d.band);
        let mut v = Vec::new();
        for (k, &(f, t, kind)) in d.edges.iter().enumerate() {
            let to_master = t == master;
            let touches = d.selected.contains(&f) || (!to_master && d.selected.contains(&t));
            let hov = hovered.is_some_and(|h| h == f || (!to_master && h == t));
            let lit = touches || hov;
            let show = if to_master { lit } else { d.all || lit };
            if !show {
                continue;
            }
            let (from, to) = (pos2(d.ports[f], PORT_Y), pos2(d.ports[t], PORT_Y));
            let share = match kind {
                Kind::Output => 1.0,
                Kind::Send => 0.78,
                Kind::Key => 0.6,
            };
            let (fs, ts) = (StripId(f as u64 + 1), StripId(t as u64 + 1));
            let route = match kind {
                Kind::Output => Route::output(fs, ts),
                Kind::Send => Route::send(fs, ts),
                Kind::Key => Route::key(fs, ts, EffectId(k as u64)),
            };
            v.push(Drawn {
                route,
                from,
                to,
                depth: cable::depth_for(from, to, room) * share,
                lit,
                laned: touches,
                chips: [false, false],
                mark: false,
                mark_at: Pos2::ZERO,
                yielded: false,
                relaxed: false,
                edge: k as u32,
            });
        }
        v
    }

    fn placed(
        d: &Desk,
        hovered: Option<usize>,
        selected: Option<u32>,
        s: &mut Scratch,
    ) -> Vec<Drawn> {
        let mut v = drawn(d, hovered);
        s.ports.clear();
        s.ports.extend(d.ports.iter().map(|x| pos2(*x, PORT_Y)));
        let sel = selected.and_then(|e| v.iter().position(|c| c.edge == e && c.laned));
        place(&mut v, s, room(d.band), sel);
        v
    }

    /// Points along a cable, its run and its legs.
    fn samples(c: &Drawn) -> impl Iterator<Item = Pos2> {
        let pts = cable::path(c.from, c.to, c.depth).pts;
        (0..pts.len() - 1).flat_map(move |i| {
            (0..8).map(move |k| pts[i] + (pts[i + 1] - pts[i]) * (k as f32 / 8.0))
        })
    }

    /// How far each mark's glyph reaches, read off kit/cable.rs `end_mark` on its own (not
    /// the placement's table): an output's dot 2.6, a send's chevron 3.6 forward and its
    /// wings 0.6·3.6 back and 3.6 aside (≈ 4.2), a key's glyph 12 pt square (half 6).
    fn drawn_r(c: &Drawn) -> f32 {
        match c.route.kind {
            cantus_core::routing::RouteKind::Output => 2.6,
            cantus_core::routing::RouteKind::Send => 4.2,
            cantus_core::routing::RouteKind::Key(_) => 6.0,
        }
    }

    /// The distance from a rect to a point (0 inside).
    fn rect_dist(r: Rect, p: Pos2) -> f32 {
        let dx = (r.min.x - p.x).max(p.x - r.max.x).max(0.0);
        let dy = (r.min.y - p.y).max(p.y - r.max.y).max(0.0);
        (dx * dx + dy * dy).sqrt()
    }

    #[derive(Default)]
    struct Tally {
        /// (desks with a bus, the others) × (bands under 40 pt, from 40 up).
        hover: [[usize; 2]; 2],
        relaxed: [[usize; 2]; 2],
        yielded: [[usize; 2]; 2],
        laned: usize,
        laned_hidden: usize,
    }

    fn check(seed: u64, s: &mut Scratch, t: &mut Tally) {
        let d = desk(seed);
        let n = d.ports.len() - 1;
        let hovered = d.bus.or(Some((seed as usize * 7) % n));
        // A selected cable, on half the desks: one of the selection's.
        let rest0 = placed(&d, None, None, s);
        let selected = seed
            .is_multiple_of(2)
            .then(|| rest0.iter().find(|c| c.laned).map(|c| c.edge))
            .flatten();
        let rest = placed(&d, None, selected, s);
        let v = placed(&d, hovered, selected, s);
        let room = room(d.band);
        let floor = cable::MIN_DEPTH.min(room);
        let why = |what: &str| {
            format!(
                "seed {seed} (band {}, all {}, bus {:?}): {what}",
                d.band, d.all, d.bus
            )
        };
        // (a) The selection's lanes do not move when a strip is hovered.
        for a in rest.iter().filter(|c| c.laned) {
            let b = v.iter().find(|c| c.edge == a.edge).expect("still drawn");
            assert!(b.laned && b.depth == a.depth, "{}", why("a lane moved"));
        }
        let lit: Vec<&Drawn> = v.iter().filter(|c| c.lit).collect();
        let unplaced = drawn(&d, hovered);
        let own = |e: u32| unplaced.iter().find(|c| c.edge == e).unwrap().depth;
        for (i, a) in lit.iter().enumerate() {
            assert!(
                a.depth >= floor.min(own(a.edge)) - 1e-3 && a.depth <= room + 1e-3,
                "{}",
                why("out of the band")
            );
            for b in &lit[i + 1..] {
                // (e) No two lit cables share a flat run.
                if overlap(a, b) {
                    assert!(
                        (a.depth - b.depth).abs() >= 0.5,
                        "{}",
                        why(&format!("{:?} and {:?} share a run", a.route, b.route))
                    );
                }
                // (f) Nested cables do not cross where they need not.
                for (outer, inner) in [(a, b), (b, a)] {
                    if contains(outer, inner) && outer.depth <= inner.depth {
                        let (feasible, hover) = if !outer.laned {
                            (inner.depth + SEP_RUN <= room, outer)
                        } else if !inner.laned {
                            (outer.depth - SEP_RUN >= floor.min(own(inner.edge)), inner)
                        } else {
                            (true, inner)
                        };
                        assert!(
                            !feasible || (!hover.laned && hover.relaxed),
                            "{}",
                            why(&format!(
                                "{:?} hangs above {:?}, which it holds",
                                outer.route, inner.route
                            ))
                        );
                    }
                }
            }
        }
        // (d) A hover cable alone where it hangs keeps its span's depth (unless that is
        // too shallow for its mark to clear the port).
        for h in v.iter().filter(|c| c.lit && !c.laned) {
            let alone = !lit
                .iter()
                .any(|o| o.edge != h.edge && (overlap(h, o) || (o.to.x - h.to.x).abs() < 30.0));
            let want = own(h.edge);
            if alone && want >= PORT_RING + SENSOR_MIN + 0.01 {
                assert_eq!(h.depth, want, "{}", why("a lone hover cable moved"));
            }
        }
        // (c) Shown marks never meet, glyph to glyph.
        let shown: Vec<&Drawn> = v.iter().filter(|c| c.mark).collect();
        for (i, a) in shown.iter().enumerate() {
            for b in &shown[i + 1..] {
                let need = drawn_r(a) + drawn_r(b);
                assert!(
                    a.mark_at.distance(b.mark_at) >= need,
                    "{}",
                    why(&format!("marks of {:?} and {:?} meet", a.route, b.route))
                );
            }
        }
        // Every non-yielded hover cable shows its mark, with its grab area (third
        // review, 2); the selected cable too, and its tail has one.
        let area = |i: usize, tail: bool| {
            s.sensors
                .iter()
                .find(|(j, t, _)| *j as usize == i && *t == tail)
                .map(|(_, _, r)| *r)
        };
        for (i, c) in v.iter().enumerate() {
            if c.lit && !c.laned {
                assert!(
                    c.mark,
                    "{}",
                    why(&format!("hover {:?} shows no mark", c.route))
                );
                assert!(
                    area(i, false).is_some(),
                    "{}",
                    why("a hover mark with no area")
                );
            }
        }
        let sel = selected.and_then(|e| v.iter().position(|c| c.edge == e));
        if let Some(i) = sel {
            assert!(v[i].mark, "{}", why("the selected cable shows no mark"));
            assert!(
                area(i, false).is_some(),
                "{}",
                why("the selected mark has no area")
            );
            assert!(
                area(i, true).is_some(),
                "{}",
                why("the selected tail has no area")
            );
        }
        // Each area holds its own mark, keeps off every other shown glyph and every
        // other area, and off the ports (the selected cable's excepted).
        for (k, &(i, tail, r)) in s.sensors.iter().enumerate() {
            let c = &v[i as usize];
            let own_at = if tail { r.center() } else { c.mark_at };
            assert!(r.contains(own_at), "{}", why("an area off its own mark"));
            for (j, o) in v.iter().enumerate() {
                if o.mark && !(j == i as usize && !tail) {
                    assert!(
                        rect_dist(r, o.mark_at) >= drawn_r(o) - 1e-3,
                        "{}",
                        why(&format!("{:?}'s area over {:?}'s mark", c.route, o.route))
                    );
                }
            }
            for &(_, _, q) in &s.sensors[k + 1..] {
                assert!(!r.intersects(q), "{}", why("grab areas meet"));
            }
            if Some(i as usize) != sel {
                for x in &d.ports {
                    let port = port_rect(pos2(*x, PORT_Y));
                    assert!(!r.intersects(port), "{}", why("a grab area over a port"));
                }
            }
        }
        // (b) Every lit cable is the one picked somewhere on itself; a faint one too,
        // unless other cables lie exactly on the whole of it.
        for (k, c) in v.iter().enumerate() {
            let covered = !c.lit
                && samples(c).all(|p| {
                    v.iter()
                        .enumerate()
                        .any(|(j, o)| j != k && cable::hit(o.from, o.to, o.depth, p) <= TIE)
                });
            if covered {
                continue;
            }
            assert!(
                samples(c).any(|p| hot(&v, p) == Some(k)),
                "{}",
                why(&format!(
                    "{:?} (lit {}) cannot be pointed at",
                    c.route, c.lit
                ))
            );
        }
        // The tallies.
        let (b, tall) = (usize::from(d.bus.is_some()), usize::from(d.band >= 40.0));
        t.hover[b][tall] += unplaced.iter().filter(|c| c.lit && !c.laned).count();
        t.relaxed[b][tall] += v.iter().filter(|c| c.lit && c.relaxed).count();
        t.yielded[b][tall] += v.iter().filter(|c| c.yielded).count();
        t.laned += v.iter().filter(|c| c.laned && !c.chips[1]).count();
        t.laned_hidden += v
            .iter()
            .filter(|c| c.laned && !c.chips[1] && !c.mark)
            .count();
    }

    #[test]
    fn the_placement_keeps_every_promise_on_random_desks() {
        let mut s = Scratch::default();
        let mut t = Tally::default();
        for seed in 1..=2000u64 {
            check(seed, &mut s, &mut t);
        }
        eprintln!(
            "hover cables [no bus, bus] × [band < 40, ≥ 40]: {:?}; relaxed {:?}; yielded {:?}",
            t.hover, t.relaxed, t.yielded
        );
        eprintln!(
            "the selection's marks left out: {} of {}",
            t.laned_hidden, t.laned
        );
        // The selection's own marks: the selected cable's never hides; the rest may, where
        // the band holds no room for them apart — bounded here.
        assert!(
            t.laned_hidden * 5 <= t.laned,
            "{} of {} of the selection's marks left out",
            t.laned_hidden,
            t.laned
        );
    }
}
