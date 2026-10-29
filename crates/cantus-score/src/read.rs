//! Reading: the notes rounded *for reading* — never moved in the project — on a grid that
//! each passage chooses for itself.
//!
//! The bar is cut into slots (a quarter, or a dotted beat in compound metres). Each slot
//! weighs the grids it could be read on — straight sixteenths or thirty-seconds, triplets,
//! sextuplets, quintuplets — by how far its onsets would move plus a price for the more
//! complex reading, and keeps the cheapest; ends snap to the grid of the slot they end in.
//! Two triplet slots that a note crosses become one triplet over a half note (quarter
//! triplets). A slot whose reading puts something off the straight grid is a tuplet
//! window; the voices decide later whether they need it.

use crate::metre::Metre;
use cantus_core::notation::ReadingGrid;

/// A note of the part as the project holds it, on the timeline.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SourceNote {
    pub id: u64,
    pub start: i64,
    pub len: i64,
    pub pitch: u8,
    pub velocity: u8,
}

/// A note as read: its start and end on the reading grid.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ReadNote {
    pub id: u64,
    pub start: i64,
    pub end: i64,
    pub pitch: u8,
    pub velocity: u8,
}

/// A bar on the timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct BarSpan {
    pub start: i64,
    pub metre: Metre,
}

/// A stretch read as a tuplet: `actual` units of `unit` ticks in the time of `normal`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Window {
    pub start: i64,
    pub len: i64,
    pub unit: i64,
    pub actual: u8,
    pub normal: u8,
}

impl Window {
    pub fn end(&self) -> i64 {
        self.start + self.len
    }
}

/// The finest straight position the reading ever keeps (a thirty-second note).
pub const STRAIGHT_FINEST: i64 = cantus_core::time::PPQ / 8;

#[derive(Copy, Clone, Debug, PartialEq)]
struct Grid {
    unit: i64,
    /// `(actual, normal)` for a tuplet; `None` straight.
    tuplet: Option<(u8, u8)>,
    /// Ticks added per onset for reading it this way.
    price: i64,
}

#[derive(Clone, Debug)]
struct Slot {
    start: i64,
    len: i64,
    grid: Grid,
}

impl Slot {
    fn snap(&self, t: i64) -> i64 {
        let u = self.grid.unit.max(1);
        let k = ((t - self.start) as f64 / u as f64).round() as i64;
        self.start + k.clamp(0, self.len / u) * u
    }
}

fn candidates(len: i64, ternary: bool, grid: ReadingGrid) -> Vec<Grid> {
    let q = cantus_core::time::PPQ;
    let straight = |unit: i64, price: i64| Grid {
        unit,
        tuplet: None,
        price,
    };
    let tup = |n: i64, a: u8, b: u8, price: i64| Grid {
        unit: len / n,
        tuplet: Some((a, b)),
        price,
    };
    let divisible = |n: i64| len % n == 0 && len / n > 0;
    match grid {
        ReadingGrid::Eighth => vec![straight(q / 2, 0)],
        ReadingGrid::Sixteenth => vec![straight(q / 4, 0)],
        ReadingGrid::ThirtySecond => vec![straight(q / 8, 0)],
        ReadingGrid::EighthTriplet if !ternary && divisible(3) => vec![tup(3, 3, 2, 0)],
        ReadingGrid::SixteenthTriplet if !ternary && divisible(6) => vec![tup(6, 6, 4, 0)],
        ReadingGrid::EighthTriplet | ReadingGrid::SixteenthTriplet => vec![straight(q / 4, 0)],
        ReadingGrid::Auto => {
            let mut c = vec![straight(q / 4, 0), straight(q / 8, 30)];
            if !ternary {
                if divisible(3) {
                    c.push(tup(3, 3, 2, 25));
                }
                if divisible(6) {
                    c.push(tup(6, 6, 4, 45));
                }
                if divisible(5) {
                    c.push(tup(5, 5, 4, 70));
                }
            }
            c
        }
    }
}

/// The slots of the bars: a quarter each (a dotted beat in compound metres; a beat
/// shorter than a quarter, or one a quarter does not divide, is its own).
fn slots(bars: &[BarSpan], grid: ReadingGrid) -> Vec<Slot> {
    let q = cantus_core::time::PPQ;
    let mut out = Vec::new();
    for b in bars {
        for &(bs, bl) in &b.metre.beats {
            let ternary = b.metre.beat_is_ternary(bl);
            let pieces: Vec<(i64, i64)> = if !ternary && bl > q && bl % q == 0 {
                (0..bl / q).map(|i| (bs + i * q, q)).collect()
            } else {
                vec![(bs, bl)]
            };
            for (s, l) in pieces {
                let g = candidates(l, ternary, grid)[0];
                out.push(Slot {
                    start: b.start + s,
                    len: l,
                    grid: g,
                });
            }
        }
    }
    out
}

/// Read `notes` on `bars` (which cover them) with `grid`: the notes rounded for reading,
/// and the tuplet windows the reading found.
pub fn read(
    notes: &[SourceNote],
    bars: &[BarSpan],
    grid: ReadingGrid,
) -> (Vec<ReadNote>, Vec<Window>) {
    let mut slots = slots(bars, grid);
    if slots.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let slot_of = |slots: &[Slot], t: i64| -> usize {
        slots
            .partition_point(|s| s.start <= t)
            .saturating_sub(1)
            .min(slots.len() - 1)
    };
    // Each onset belongs to the slot it is nearest the start of: up to a twelfth of a slot
    // early counts as the next one's downbeat.
    let mut onsets: Vec<Vec<i64>> = vec![Vec::new(); slots.len()];
    for n in notes {
        let i = slot_of(&slots, n.start);
        let s = &slots[i];
        let j = if n.start + s.len / 12 >= s.start + s.len && i + 1 < slots.len() {
            i + 1
        } else {
            i
        };
        onsets[j].push(n.start);
    }
    for (i, s) in slots.iter_mut().enumerate() {
        let ternary = bars
            .iter()
            .rev()
            .find(|b| b.start <= s.start)
            .map(|b| {
                b.metre
                    .beat_is_ternary(b.metre.beat_at(s.start - b.start).1)
            })
            .unwrap_or(false);
        let mut best: Option<(i64, Grid)> = None;
        for g in candidates(s.len, ternary, grid) {
            let probe = Slot {
                start: s.start,
                len: s.len,
                grid: g,
            };
            let mut cost = 0;
            for &t in &onsets[i] {
                let snapped = probe.snap(t);
                cost += (t - snapped).abs();
                if snapped != s.start {
                    cost += g.price;
                }
            }
            if best.is_none_or(|(c, _)| cost < c) {
                best = Some((cost, g));
            }
        }
        if let Some((_, g)) = best {
            s.grid = g;
        }
    }
    // Snap: onsets with the slot they were counted in, ends with the slot they end in.
    let mut out: Vec<ReadNote> = Vec::with_capacity(notes.len());
    for n in notes {
        let i = slot_of(&slots, n.start);
        let si = if n.start + slots[i].len / 12 >= slots[i].start + slots[i].len
            && i + 1 < slots.len()
        {
            i + 1
        } else {
            i
        };
        let mut start = slots[si].snap(n.start);
        // The last slot's end is the range's: an onset never rounds onto it, where no bar
        // would show it (review, finding 8) — it keeps the slot's last point instead.
        if si + 1 == slots.len() {
            let last = &slots[si];
            start = start.min(last.start + last.len - last.grid.unit);
        }
        let e = n.start + n.len.max(1);
        let ei = slot_of(&slots, e);
        let mut end = slots[ei].snap(e);
        if ei > 0 && e - slots[ei].start < slots[ei].grid.unit / 2 {
            // Just past a slot's start: the previous slot's grid may hold it closer.
            let prev = slots[ei - 1].snap(e);
            if (prev - e).abs() < (end - e).abs() {
                end = prev;
            }
        }
        if end <= start {
            end = start + slots[si].grid.unit;
        }
        out.push(ReadNote {
            id: n.id,
            start,
            end,
            pitch: n.pitch,
            velocity: n.velocity,
        });
    }
    // One pitch never overlaps itself: the earlier note ends where the later starts, and
    // two on the same tick keep the longer.
    out.sort_by_key(|n| (n.pitch, n.start, std::cmp::Reverse(n.end), n.id));
    let mut kept: Vec<ReadNote> = Vec::with_capacity(out.len());
    for n in out {
        if let Some(last) = kept.last_mut() {
            if last.pitch == n.pitch {
                if last.start == n.start {
                    continue;
                }
                if last.end > n.start {
                    last.end = n.start;
                }
            }
        }
        kept.push(n);
    }
    kept.sort_by_key(|n| (n.start, n.pitch, n.id));

    // Windows: a tuplet slot with something inside off the straight grid.
    let off_straight = |s: &Slot, t: i64| {
        t > s.start && t < s.start + s.len && (t - s.start) % STRAIGHT_FINEST != 0
    };
    let mut windows: Vec<Window> = Vec::new();
    for s in &slots {
        let Some((a, b)) = s.grid.tuplet else {
            continue;
        };
        let used = kept
            .iter()
            .any(|n| off_straight(s, n.start) || off_straight(s, n.end));
        if used {
            windows.push(Window {
                start: s.start,
                len: s.len,
                unit: s.grid.unit,
                actual: a,
                normal: b,
            });
        }
    }
    // Two triplet quarters a note crosses become one triplet over the half note, when
    // the pair starts on a strong beat of its bar.
    let mut merged: Vec<Window> = Vec::with_capacity(windows.len());
    let mut i = 0;
    while i < windows.len() {
        let w = windows[i];
        if let Some(&v) = windows.get(i + 1) {
            let pair = w.end() == v.start
                && w.len == v.len
                && (w.actual, w.normal) == (3, 2)
                && (v.actual, v.normal) == (3, 2)
                && w.unit == v.unit;
            // A note over the pair's middle that starts or ends off the straight grid.
            let off = |t: i64| (t - w.start) % STRAIGHT_FINEST != 0;
            let crossed = kept.iter().any(|n| {
                n.start >= w.start
                    && n.start < v.start
                    && n.end > v.start
                    && n.end <= v.end()
                    && (off(n.start) || off(n.end))
            });
            let bar = bars.iter().rev().find(|b| b.start <= w.start);
            let strong = bar.is_some_and(|b| {
                let off = w.start - b.start;
                off % (2 * w.len) == 0 && off + 2 * w.len <= b.metre.len
            });
            if pair && crossed && strong {
                merged.push(Window {
                    start: w.start,
                    len: w.len * 2,
                    unit: w.unit,
                    actual: 3,
                    normal: 2,
                });
                i += 2;
                continue;
            }
        }
        merged.push(w);
        i += 1;
    }
    (kept, merged)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantus_core::time::TimeSignature;

    fn bars(n: usize, sig: TimeSignature) -> Vec<BarSpan> {
        let len = sig.ticks_per_bar().0;
        (0..n)
            .map(|i| BarSpan {
                start: i as i64 * len,
                metre: Metre::new(sig, len),
            })
            .collect()
    }

    fn note(start: i64, len: i64, pitch: u8) -> SourceNote {
        SourceNote {
            id: start as u64,
            start,
            len,
            pitch,
            velocity: 90,
        }
    }

    #[test]
    fn played_sixteenths_read_straight() {
        let b = bars(1, TimeSignature::new(4, 4));
        let notes: Vec<SourceNote> = [0i64, 248, 470, 731, 965]
            .iter()
            .map(|&t| note(t, 200, 60))
            .collect();
        let (r, w) = read(&notes, &b, ReadingGrid::Auto);
        let starts: Vec<i64> = r.iter().map(|n| n.start).collect();
        assert_eq!(starts, vec![0, 240, 480, 720, 960]);
        assert!(w.is_empty());
    }

    #[test]
    fn played_triplets_read_as_triplets() {
        let b = bars(1, TimeSignature::new(4, 4));
        let notes: Vec<SourceNote> = [5i64, 330, 628, 960]
            .iter()
            .map(|&t| note(t, 300, 64))
            .collect();
        let (r, w) = read(&notes, &b, ReadingGrid::Auto);
        let starts: Vec<i64> = r.iter().map(|n| n.start).collect();
        assert_eq!(starts, vec![0, 320, 640, 960]);
        assert_eq!(w.len(), 1);
        assert_eq!(
            (w[0].start, w[0].len, w[0].actual, w[0].normal),
            (0, 960, 3, 2)
        );
    }

    #[test]
    fn quarter_triplets_make_one_window_over_the_half() {
        let b = bars(1, TimeSignature::new(4, 4));
        let notes: Vec<SourceNote> = [0i64, 640, 1280]
            .iter()
            .map(|&t| note(t, 640, 67))
            .collect();
        let (_, w) = read(&notes, &b, ReadingGrid::Auto);
        assert_eq!(w.len(), 1, "{w:?}");
        assert_eq!((w[0].start, w[0].len), (0, 1920));
    }
}
