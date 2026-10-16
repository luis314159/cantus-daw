//! The metre side of the tempo map: bar lines, beats, bar/beat/tick and snapping, counted
//! through every metre change (ADR 0060 §3). With one metre each answer is the one
//! `TimeSignature` gave before round 29, to the tick.

use super::{Bar, TempoMap};
use crate::time::{Ticks, TimeSignature};

impl TempoMap {
    /// The metre segment holding `tick` (the first for anything before tick 0).
    #[inline]
    fn bar_seg(&self, tick: i64) -> usize {
        self.bars
            .partition_point(|b| b.tick <= tick)
            .saturating_sub(1)
    }

    /// Where the metre segment `i` ends (the next change), if it does.
    #[inline]
    fn bar_seg_end(&self, i: usize) -> Option<i64> {
        self.bars.get(i + 1).map(|b| b.tick)
    }

    /// The time signature in force at `tick`.
    pub fn signature_at(&self, tick: Ticks) -> TimeSignature {
        self.bars[self.bar_seg(tick.0)].sig
    }

    /// The length of the bar that holds `tick` as its metre counts it (a bar cut short by
    /// an off-line metre change is shorter: [`TempoMap::bar_end`] says where it ends).
    pub fn bar_len_at(&self, tick: Ticks) -> Ticks {
        self.signature_at(tick).ticks_per_bar()
    }

    /// The length of a beat (one `1/denominator` note) at `tick`.
    pub fn beat_len_at(&self, tick: Ticks) -> Ticks {
        self.signature_at(tick).ticks_per_beat()
    }

    /// The start of the bar that holds `tick`.
    pub fn bar_start(&self, tick: Ticks) -> Ticks {
        let b = &self.bars[self.bar_seg(tick.0)];
        let len = b.len();
        Ticks(b.tick + (tick.0 - b.tick).div_euclid(len) * len)
    }

    /// The start of the bar after the one that holds `tick`.
    pub fn bar_end(&self, tick: Ticks) -> Ticks {
        let i = self.bar_seg(tick.0);
        let next = self.bar_start(tick).0 + self.bars[i].len();
        Ticks(self.bar_seg_end(i).map_or(next, |e| next.min(e)))
    }

    /// Bar/beat/tick, 1-based, counted through every metre change.
    pub fn bbt(&self, tick: Ticks) -> (i64, i64, i64) {
        let b = &self.bars[self.bar_seg(tick.0)];
        let len = b.len();
        let beat = b.sig.ticks_per_beat().0;
        let rel = tick.0 - b.tick;
        let in_bar = rel.rem_euclid(len);
        (
            b.bar + rel.div_euclid(len) + 1,
            in_bar / beat + 1,
            in_bar % beat,
        )
    }

    /// The tick where 1-based bar `bar` starts (bar 1 at tick 0; bars before it count back
    /// in the first metre).
    pub fn bar_tick(&self, bar: i64) -> Ticks {
        let idx = bar - 1;
        let i = self
            .bars
            .partition_point(|b| b.bar <= idx)
            .saturating_sub(1);
        let b = &self.bars[i];
        Ticks(b.tick + (idx - b.bar) * b.len())
    }

    /// The tick of bar/beat/tick (1-based bar and beat), the inverse of
    /// [`TempoMap::bbt`].
    pub fn tick_of_bbt(&self, bar: i64, beat: i64, tick: i64) -> Ticks {
        let start = self.bar_tick(bar);
        start + self.beat_len_at(start) * (beat - 1) + Ticks(tick)
    }

    /// The bar lines from `from` (inclusive) to `to` (exclusive), in order.
    pub fn bar_lines(&self, from: Ticks, to: Ticks) -> impl Iterator<Item = Ticks> + '_ {
        let i = self.bar_seg(from.0);
        let start = self.bar_start(from);
        let first = if start == from {
            from
        } else {
            self.bar_end(from)
        };
        BarLines {
            map: self,
            seg: self.bar_seg(first.0).max(i),
            next: first,
            to,
        }
    }

    /// The beat lines (every `1/denominator` of the metre in force) from `from`
    /// (inclusive) to `to` (exclusive), each with whether it starts a bar.
    pub fn beat_lines(&self, from: Ticks, to: Ticks) -> impl Iterator<Item = (Ticks, bool)> + '_ {
        let mut next = self.beat_at_or_after(from);
        std::iter::from_fn(move || {
            if next >= to {
                return None;
            }
            let here = next;
            next = self.beat_after(here);
            Some((here, self.bar_start(here) == here))
        })
    }

    /// The first beat line at or after `tick`.
    pub fn beat_at_or_after(&self, tick: Ticks) -> Ticks {
        let i = self.bar_seg(tick.0);
        let b = &self.bars[i];
        let beat = b.sig.ticks_per_beat().0;
        // Beats count from the bar's start (a bar cut short ends on the change).
        let bar = self.bar_start(tick).0;
        let n = (tick.0 - bar + beat - 1).div_euclid(beat);
        let at = bar + n * beat;
        Ticks(self.bar_seg_end(i).map_or(at, |e| at.min(e)))
    }

    /// The beat line after the one at `tick` (or the first after `tick`).
    pub fn beat_after(&self, tick: Ticks) -> Ticks {
        self.beat_at_or_after(tick + Ticks(1))
    }

    /// The first beat at or after `tick` (fractional) and whether it starts a bar: what the
    /// click plays next. Beat lines are whole ticks.
    pub fn next_beat(&self, tick: f64) -> (f64, bool) {
        let t = Ticks(tick.ceil() as i64);
        let at = self.beat_at_or_after(t);
        (at.0 as f64, self.bar_start(at) == at)
    }

    /// `tick` snapped to the nearest multiple of `grid` — counted from tick 0 with one
    /// metre (exactly [`Ticks::snap`]), from the start of the bar that holds it with
    /// several, so a grid stays on the bars after a bar of 7/8; a grid of a bar or more
    /// snaps to the nearest bar line.
    pub fn snap(&self, tick: Ticks, grid: Ticks) -> Ticks {
        if self.is_one_metre() || grid.0 <= 0 {
            return if grid.0 > 0 { tick.snap(grid) } else { tick };
        }
        let start = self.bar_start(tick);
        let end = self.bar_end(tick);
        if grid >= end - start {
            return if tick - start < end - tick {
                start
            } else {
                end
            };
        }
        (start + (tick - start).snap(grid)).min(end)
    }

    /// `tick` snapped down to a multiple of `grid` (see [`TempoMap::snap`]).
    pub fn snap_floor(&self, tick: Ticks, grid: Ticks) -> Ticks {
        if self.is_one_metre() || grid.0 <= 0 {
            return if grid.0 > 0 {
                tick.snap_floor(grid)
            } else {
                tick
            };
        }
        let start = self.bar_start(tick);
        let end = self.bar_end(tick);
        if grid >= end - start {
            return start;
        }
        start + (tick - start).snap_floor(grid)
    }

    /// `tick` snapped to the nearest bar line.
    pub fn snap_bar(&self, tick: Ticks) -> Ticks {
        let start = self.bar_start(tick);
        let end = self.bar_end(tick);
        if tick - start < end - tick {
            start
        } else {
            end
        }
    }

    /// The bar line `n` bars after (before, negative) the start of the bar holding `tick`,
    /// plus `tick`'s offset into its bar (clamped to the target bar) — *nudge by a bar*.
    pub fn add_bars(&self, tick: Ticks, n: i64) -> Ticks {
        let (bar, _, _) = self.bbt(tick);
        let into = tick - self.bar_start(tick);
        let target = self.bar_tick(bar + n);
        let len = self.bar_end(target) - target;
        target + into.min(len)
    }
}

/// The bar lines of a span (see [`TempoMap::bar_lines`]).
#[derive(Debug)]
pub struct BarLines<'a> {
    map: &'a TempoMap,
    seg: usize,
    next: Ticks,
    to: Ticks,
}

impl Iterator for BarLines<'_> {
    type Item = Ticks;

    fn next(&mut self) -> Option<Ticks> {
        if self.next >= self.to {
            return None;
        }
        let here = self.next;
        let b: &Bar = &self.map.bars[self.seg];
        let mut next = here.0 + b.len();
        if let Some(e) = self.map.bar_seg_end(self.seg) {
            if next >= e {
                next = e;
                self.seg += 1;
            }
        }
        self.next = Ticks(next);
        Some(here)
    }
}
