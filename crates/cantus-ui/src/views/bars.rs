//! The timeline's bars through the tempo map (round 29, ADR 0060): the one helper every
//! ruler, grid, bar/beat/tick display and bar or beat snap in the interface counts by.
//!
//! The timeline's x axis is beat-linear, so a tempo change moves nothing on screen; only
//! the bar lines move, when the metre changes. With one metre every answer here is the one
//! the old arithmetic gave (`TimeSignature` from tick 0), so a project without metre
//! changes draws and snaps exactly as before.

use cantus_core::tempo::{MetreChange, TempoPoint};
use cantus_core::{Project, TempoMap, Ticks, TimeSignature, PPQ};
use std::cell::RefCell;
use std::sync::Arc;

/// What the map was compiled from: a cache hit compares these (small lists) instead of
/// compiling the map again every frame.
#[derive(PartialEq)]
struct Key {
    bpm: u64,
    sig: TimeSignature,
    points: Vec<TempoPoint>,
    metres: Vec<MetreChange>,
}

thread_local! {
    static CACHE: RefCell<Option<(Key, Arc<TempoMap>)>> = const { RefCell::new(None) };
}

/// The project's tempo map, compiled once per change of its tempo or metre.
pub fn map(project: &Project) -> Arc<TempoMap> {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        let same = c.as_ref().is_some_and(|(k, _)| {
            k.bpm == project.tempo_bpm.to_bits()
                && k.sig == project.time_signature
                && k.points == project.tempo_points
                && k.metres == project.metre_changes
        });
        if !same {
            let key = Key {
                bpm: project.tempo_bpm.to_bits(),
                sig: project.time_signature,
                points: project.tempo_points.clone(),
                metres: project.metre_changes.clone(),
            };
            *c = Some((key, Arc::new(project.tempo_map())));
        }
        c.as_ref().map(|(_, m)| m.clone()).expect("just set")
    })
}

/// The grid that means *a bar* (the grid menu's "Bar"): with metre changes, the bar of the
/// metre in force where it snaps.
pub const BAR_GRID: Ticks = Ticks(PPQ * 4);

/// `tick` snapped to the nearest line of `grid`, counted through the metre: a bar grid
/// snaps to the bar lines of the metre in force (with one metre, its bar from tick 0); a
/// finer grid counts from the start of the bar that holds `tick`.
pub fn snap(project: &Project, tick: Ticks, grid: Ticks) -> Ticks {
    let m = map(project);
    if grid == BAR_GRID {
        return if m.is_one_metre() {
            tick.snap(project.time_signature.ticks_per_bar())
        } else {
            m.snap_bar(tick)
        };
    }
    m.snap(tick, grid.max(Ticks(1)))
}

/// A clip's content tick `c` snapped to `grid`: with one metre counted from the content's
/// start (as the piano roll always did); with metre changes through the bars of the
/// timeline it lies on (`c + to_timeline`).
pub fn snap_content(project: &Project, c: Ticks, grid: Ticks, to_timeline: Ticks) -> Ticks {
    Snapper::of(project).content(c, grid, to_timeline)
}

/// [`snap_content`], down.
pub fn snap_content_floor(project: &Project, c: Ticks, grid: Ticks, to_timeline: Ticks) -> Ticks {
    Snapper::of(project).content_floor(c, grid, to_timeline)
}

/// The snapping of a project, held apart from it (a gesture's closures keep it while
/// they change the project).
#[derive(Clone, Debug)]
pub struct Snapper {
    map: Arc<TempoMap>,
    /// The song's first bar (one metre: the bar grid's length).
    bar: Ticks,
}

impl Snapper {
    pub fn of(project: &Project) -> Snapper {
        Snapper {
            map: map(project),
            bar: project.time_signature.ticks_per_bar(),
        }
    }

    /// See [`snap`].
    pub fn tick(&self, tick: Ticks, grid: Ticks) -> Ticks {
        if grid == BAR_GRID {
            return if self.map.is_one_metre() {
                tick.snap(self.bar)
            } else {
                self.map.snap_bar(tick)
            };
        }
        self.map.snap(tick, grid.max(Ticks(1)))
    }

    /// See [`snap_content`].
    pub fn content(&self, c: Ticks, grid: Ticks, to_timeline: Ticks) -> Ticks {
        if self.map.is_one_metre() {
            let g = if grid == BAR_GRID {
                self.bar
            } else {
                grid.max(Ticks(1))
            };
            return c.snap(g);
        }
        self.tick(c + to_timeline, grid) - to_timeline
    }

    /// See [`snap_content_floor`].
    pub fn content_floor(&self, c: Ticks, grid: Ticks, to_timeline: Ticks) -> Ticks {
        if self.map.is_one_metre() {
            let g = if grid == BAR_GRID {
                self.bar
            } else {
                grid.max(Ticks(1))
            };
            return c.snap_floor(g);
        }
        let t = c + to_timeline;
        let floor = if grid == BAR_GRID {
            self.map.bar_start(t)
        } else {
            self.map.snap_floor(t, grid.max(Ticks(1)))
        };
        floor - to_timeline
    }
}

/// `tick` snapped down to a line of `grid` (see [`snap`]).
pub fn snap_floor(project: &Project, tick: Ticks, grid: Ticks) -> Ticks {
    let m = map(project);
    if grid == BAR_GRID {
        return if m.is_one_metre() {
            tick.snap_floor(project.time_signature.ticks_per_bar())
        } else {
            m.bar_start(tick)
        };
    }
    m.snap_floor(tick, grid.max(Ticks(1)))
}

/// The length of the bar that holds `tick` (the grid a bar's worth of anything snaps to).
pub fn bar_len(project: &Project, tick: Ticks) -> Ticks {
    let m = map(project);
    m.bar_end(tick) - m.bar_start(tick)
}

/// The start of the bar that holds `tick`.
pub fn bar_start(project: &Project, tick: Ticks) -> Ticks {
    map(project).bar_start(tick)
}

/// Bar/beat/tick (1-based) of `tick`, counted through every metre change.
pub fn bbt(project: &Project, tick: Ticks) -> (i64, i64, i64) {
    map(project).bbt(tick)
}

/// The tick `n` bars after `tick` (before it, negative), keeping its place in the bar.
pub fn add_bars(project: &Project, tick: Ticks, n: i64) -> Ticks {
    map(project).add_bars(tick, n)
}

/// The tick `bars` whole bars after `tick`'s bar start (a default punch, a pattern's bar).
pub fn bars_from(project: &Project, tick: Ticks, bars: i64) -> Ticks {
    let m = map(project);
    let (bar, _, _) = m.bbt(tick);
    m.bar_tick(bar + bars)
}

/// Bars from bar 1 to `tick`, fractional (a bar cut short counts as one): the length of a
/// span in bars is the difference of two of these.
pub fn bar_position(project: &Project, tick: Ticks) -> f64 {
    let m = map(project);
    let (bar, _, _) = m.bbt(tick);
    let start = m.bar_start(tick);
    let len = (m.bar_end(tick) - start).0.max(1) as f64;
    (bar - 1) as f64 + (tick - start).0 as f64 / len
}

/// The metre a bar/beat/tick readout is sized for: of the song's metres, the one whose
/// beat needs the most tick digits (with one metre, the song's).
pub fn widest_metre(project: &Project) -> TimeSignature {
    project
        .metre_changes
        .iter()
        .map(|m| m.signature)
        .chain(std::iter::once(project.time_signature))
        .max_by_key(|s| (s.ticks_per_beat().0.to_string().len(), s.numerator))
        .unwrap_or(project.time_signature)
}

/// The tempo at `tick` (a ramp's value inside one).
pub fn bpm_at(project: &Project, tick: Ticks) -> f64 {
    map(project).bpm_at(tick.0 as f64)
}

/// Seconds between two ticks, through the map.
pub fn seconds_between(project: &Project, from: Ticks, to: Ticks) -> f64 {
    map(project).seconds_between(from, to)
}

/// The step between ladder lines, in beats: a beat, doubled until lines are at least
/// `min_px` apart; past a bar, whole bars doubling (the timeline's old rule).
pub fn beat_step(px_per_beat: f32, beats_per_bar: f32, min_px: f32) -> f32 {
    let mut step = 1.0f32;
    while step * px_per_beat < min_px {
        step *= 2.0;
        if step >= beats_per_bar && step * px_per_beat < min_px {
            step = beats_per_bar;
            while step * px_per_beat < min_px {
                step *= 2.0;
            }
            break;
        }
    }
    step
}

/// The bar lines (and, with `beats`, the beat lines) from `from` (inclusive) to `to`
/// (exclusive), each with whether it starts a bar. With one metre exactly the multiples of
/// the bar and the beat from tick 0.
pub fn lines(map: &TempoMap, from: Ticks, to: Ticks, beats: bool, mut f: impl FnMut(Ticks, bool)) {
    if beats {
        for (t, bar) in map.beat_lines(from, to) {
            f(t, bar);
        }
    } else {
        for t in map.bar_lines(from, to) {
            f(t, true);
        }
    }
}

/// Whether `tick` is a bar line.
pub fn is_bar_line(map: &TempoMap, tick: Ticks) -> bool {
    map.bar_start(tick) == tick
}

/// A line of a timeline's ladder: where (in beats), and whether it starts a bar.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Line {
    pub beat: f32,
    pub bar: bool,
}

/// The lines between beats `first` and `last` at `px_per_beat`, thinned (doubling) until
/// they are `min_px` apart — the ruler's and the lanes' ladder. With one metre exactly the
/// old ladder; with several each bar draws its own lines from its own start.
pub fn ladder(
    map: &TempoMap,
    first: f32,
    last: f32,
    px_per_beat: f32,
    min_px: f32,
    mut f: impl FnMut(Line),
) {
    if map.is_one_metre() {
        let ts = map.signature_at(Ticks::ZERO);
        let per_bar = ts.numerator as f32 * 4.0 / ts.denominator as f32;
        let step = beat_step(px_per_beat, per_bar, min_px);
        let mut beat = (first / step).floor() * step;
        while beat <= last {
            f(Line {
                beat,
                bar: (beat / per_bar).fract().abs() < 1e-4,
            });
            beat += step;
        }
        return;
    }
    let from = Ticks((first.max(0.0) * PPQ as f32) as i64);
    let from = map.bar_start(from);
    let to = Ticks((last * PPQ as f32).ceil() as i64 + 1);
    // Bars thinned like the beats when even a bar is too narrow.
    let mut every = 1i64;
    for bar in map.bar_lines(from, to) {
        let (n, _, _) = map.bbt(bar);
        let len = (map.bar_end(bar) - bar).0 as f32 / PPQ as f32;
        let per_bar = map.signature_at(bar).ticks_per_bar().0 as f32 / PPQ as f32;
        let start = bar.0 as f32 / PPQ as f32;
        while len * px_per_beat * (every as f32) < min_px && every < 64 {
            every *= 2;
        }
        if (n - 1).rem_euclid(every) != 0 {
            continue;
        }
        f(Line {
            beat: start,
            bar: true,
        });
        let step = beat_step(px_per_beat, per_bar, min_px);
        if step >= per_bar {
            continue;
        }
        let mut beat = start + step;
        while beat < start + len - 1e-4 && beat <= last {
            f(Line { beat, bar: false });
            beat += step;
        }
    }
}

/// The bars between beats `first` and `last`: each one's 1-based number, its start and
/// its length in beats — for numbers and alternating shades.
pub fn each_bar(map: &TempoMap, first: f32, last: f32, mut f: impl FnMut(i64, f32, f32)) {
    let from = map.bar_start(Ticks((first.max(0.0) * PPQ as f32) as i64));
    let to = Ticks((last * PPQ as f32).ceil() as i64 + 1);
    for bar in map.bar_lines(from, to) {
        let (n, _, _) = map.bbt(bar);
        let len = (map.bar_end(bar) - bar).0 as f32 / PPQ as f32;
        f(n, bar.0 as f32 / PPQ as f32, len);
    }
}

/// A clip's length as bars, beats and ticks from its start (review 2): with one metre the
/// song's metre counts it; with metre changes, whole bars from `start` through them, the
/// rest in the metre where the last whole bar ends.
pub fn length_bbt(project: &Project, start: Ticks, length: Ticks) -> (i64, i64, i64) {
    let m = map(project);
    if m.is_one_metre() {
        return project.time_signature.bbt(length);
    }
    let end = start + length.max(Ticks::ZERO);
    let mut bars = 0;
    let mut at = start;
    loop {
        let next = m.add_bars(at, 1);
        if next > end || next <= at {
            break;
        }
        bars += 1;
        at = next;
    }
    let beat = m.beat_len_at(at).0.max(1);
    let rest = (end - at).0;
    (bars + 1, rest / beat + 1, rest % beat)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(metres: &[(i64, u8, u8)]) -> Project {
        let mut p = Project::new("bars");
        p.metre_changes = metres
            .iter()
            .map(|(t, n, d)| MetreChange {
                tick: Ticks(*t),
                signature: TimeSignature::new(*n, *d),
            })
            .collect();
        if let Some(m) = p.metre_changes.first() {
            p.time_signature = m.signature;
        }
        p
    }

    #[test]
    fn one_metre_snaps_as_before() {
        let mut p = Project::new("bars");
        p.time_signature = TimeSignature::new(3, 4);
        for t in [-100, 0, 1500, 2881, 9999] {
            let t = Ticks(t);
            assert_eq!(snap(&p, t, Ticks(240)), t.snap(Ticks(240)));
            assert_eq!(snap(&p, t, BAR_GRID), t.snap(Ticks(2880)));
            assert_eq!(snap_floor(&p, t, BAR_GRID), t.snap_floor(Ticks(2880)));
            assert_eq!(bbt(&p, t), p.time_signature.bbt(t));
        }
    }

    #[test]
    fn snaps_and_counts_through_metre_changes() {
        // 4/4, a bar of 7/8 at bar 3, 3/4 from bar 4.
        let p = project(&[(0, 4, 4), (7680, 7, 8), (11_040, 3, 4)]);
        assert_eq!(snap(&p, Ticks(12_100), Ticks(960)), Ticks(12_000));
        assert_eq!(snap(&p, Ticks(10_500), BAR_GRID), Ticks(11_040));
        assert_eq!(snap_floor(&p, Ticks(10_500), BAR_GRID), Ticks(7680));
        assert_eq!(bar_len(&p, Ticks(8000)), Ticks(3360));
        assert_eq!(bbt(&p, Ticks(11_040)), (4, 1, 0));
        assert_eq!(bars_from(&p, Ticks(8000), 2), p.tempo_map().bar_tick(5));
        let mut bars = Vec::new();
        ladder(&map(&p), 0.0, 16.0, 40.0, 7.0, |l| {
            if l.bar {
                bars.push(l.beat);
            }
        });
        assert_eq!(bars, vec![0.0, 4.0, 8.0, 11.5, 14.5]);
        let mut numbers = Vec::new();
        each_bar(&map(&p), 0.0, 16.0, |n, _, len| numbers.push((n, len)));
        assert_eq!(numbers[2], (3, 3.5));
    }
}
