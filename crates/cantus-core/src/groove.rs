//! Groove templates (round 31, ADR 0069): Ableton's groove pool, the MPC's swing.
//!
//! A [`Groove`] is a feel over a period — for each step of a grid, how late (or early) a
//! note there is played, how hard and how long, against a straight performance. A clip
//! takes one **non-destructively** ([`ClipGroove`]): the snapshot plays its notes moved,
//! accented and lengthened by the clip's amounts, and the clip's notes stay as written
//! until *Commit* writes the groove into them. The pool holds the built-in swings (8ths
//! and 16ths, 50–75 %, never stored) and the grooves the project extracted from its own
//! clips — a MIDI clip's notes, or an audio clip's transients.
//!
//! **Where the grid is.** A groove reads a note's position on the timeline (where it is
//! heard), so swing lines up with the bars whatever the clip's offset, and a looped clip's
//! every repetition gets the same feel. Each note takes the point of its nearest grid step
//! and keeps its own distance from it: a groove adds a feel, it does not quantise.

use crate::model::{Clip, Note};
use crate::time::Ticks;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A pooled groove's id (`Project::alloc_id`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GrooveId(pub u64);

impl fmt::Display for GrooveId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gr{}", self.0)
    }
}

/// One step of a groove.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroovePoint {
    /// Where the step is in the period.
    pub at: Ticks,
    /// How much later than the step a note there is played (negative: earlier), ticks.
    pub shift: f32,
    /// Its velocity against the groove's average (1 = the same).
    #[serde(default = "one")]
    pub velocity: f32,
    /// Its length against the groove's average (1 = the same).
    #[serde(default = "one")]
    pub length: f32,
}

fn one() -> f32 {
    1.0
}

/// A feel over a period.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Groove {
    pub id: GrooveId,
    pub name: String,
    /// How long before it repeats (a bar, two beats …).
    pub period: Ticks,
    /// Its steps, sorted by `at`, inside the period.
    pub points: Vec<GroovePoint>,
}

/// The grid a built-in swing swings.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SwingGrid {
    Eighths,
    Sixteenths,
}

impl SwingGrid {
    /// The step it swings, ticks.
    pub fn step(self) -> Ticks {
        match self {
            SwingGrid::Eighths => Ticks::EIGHTH,
            SwingGrid::Sixteenths => Ticks::SIXTEENTH,
        }
    }
}

/// The built-in swings' amounts (MPC 50 … 75 %).
pub const SWING_PERCENTS: [u8; 7] = [50, 54, 58, 62, 66, 71, 75];

/// Where a clip's groove comes from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GrooveSource {
    /// A built-in swing: the second of each pair of steps played at `percent` of the
    /// pair (50 straight, 66 a triplet feel, 75 a dotted one).
    Swing { grid: SwingGrid, percent: u8 },
    /// A groove of the project's pool.
    Pool { id: GrooveId },
}

impl GrooveSource {
    /// Every built-in swing, in the pool's order.
    pub fn builtins() -> impl Iterator<Item = GrooveSource> {
        [SwingGrid::Eighths, SwingGrid::Sixteenths]
            .into_iter()
            .flat_map(|grid| {
                SWING_PERCENTS
                    .into_iter()
                    .map(move |percent| GrooveSource::Swing { grid, percent })
            })
    }

    /// Whether it names a swing in range or a groove the pool holds.
    pub fn exists(&self, pool: &[Groove]) -> bool {
        match self {
            GrooveSource::Swing { percent, .. } => (50..=75).contains(percent),
            GrooveSource::Pool { id } => pool.iter().any(|g| g.id == *id),
        }
    }
}

/// A clip's groove and how much of it is applied.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipGroove {
    pub source: GrooveSource,
    /// How much of the groove's timing (0…1; past 1 exaggerates, to 1.5).
    #[serde(default = "one")]
    pub timing: f32,
    /// How much of its accents (0…1).
    #[serde(default)]
    pub velocity: f32,
    /// How much of its lengths (0…1).
    #[serde(default)]
    pub length: f32,
}

impl ClipGroove {
    pub fn new(source: GrooveSource) -> ClipGroove {
        ClipGroove {
            source,
            timing: 1.0,
            velocity: 0.0,
            length: 0.0,
        }
    }

    /// Whether its amounts are in range.
    pub fn is_valid(&self) -> bool {
        (0.0..=1.5).contains(&self.timing)
            && (0.0..=1.0).contains(&self.velocity)
            && (0.0..=1.0).contains(&self.length)
    }
}

impl Groove {
    /// The built-in swing `percent` of `grid`, as a groove (id 0: never pooled).
    pub fn swing(grid: SwingGrid, percent: u8) -> Groove {
        let step = grid.step();
        let pair = (step.0 * 2) as f32;
        let shift = (percent.clamp(50, 75) as f32 / 100.0 - 0.5) * pair;
        Groove {
            id: GrooveId(0),
            name: format!(
                "Swing {} {percent}%",
                match grid {
                    SwingGrid::Eighths => "8",
                    SwingGrid::Sixteenths => "16",
                }
            ),
            period: Ticks(step.0 * 2),
            points: vec![
                GroovePoint {
                    at: Ticks::ZERO,
                    shift: 0.0,
                    velocity: 1.0,
                    length: 1.0,
                },
                GroovePoint {
                    at: step,
                    shift,
                    velocity: 1.0,
                    length: 1.0,
                },
            ],
        }
    }

    /// Whether it is well formed: a period, steps sorted inside it, finite numbers.
    pub fn is_valid(&self) -> bool {
        self.period > Ticks::ZERO
            && !self.points.is_empty()
            && self.points.len() <= 512
            && self.points.windows(2).all(|w| w[0].at < w[1].at)
            && self.points.iter().all(|p| {
                p.at >= Ticks::ZERO
                    && p.at < self.period
                    && p.shift.is_finite()
                    && p.shift.abs() <= self.period.0 as f32
                    && p.velocity.is_finite()
                    && (0.0..=4.0).contains(&p.velocity)
                    && p.length.is_finite()
                    && (0.0..=4.0).contains(&p.length)
            })
    }

    /// The step nearest `tick` (circularly in the period).
    fn point_at(&self, tick: Ticks) -> &GroovePoint {
        let period = self.period.0.max(1);
        let phase = tick.0.rem_euclid(period);
        let dist = |p: &GroovePoint| {
            let d = (phase - p.at.0).rem_euclid(period);
            d.min(period - d)
        };
        // Ties go to the earlier step, so a note halfway is read the same every time.
        let mut best = &self.points[0];
        let mut bd = dist(best);
        for p in &self.points[1..] {
            let d = dist(p);
            if d < bd {
                best = p;
                bd = d;
            }
        }
        best
    }

    /// A note heard at `tick` with `velocity` and `length`, played with `amounts` of this
    /// groove: where, how hard and how long. Integer ticks, so the snapshot stays exact.
    pub fn apply(
        &self,
        amounts: &ClipGroove,
        tick: Ticks,
        velocity: u8,
        length: Ticks,
    ) -> (Ticks, u8, Ticks) {
        if self.points.is_empty() || self.period <= Ticks::ZERO {
            return (tick, velocity, length);
        }
        let p = self.point_at(tick);
        let t = tick.0 + (amounts.timing * p.shift).round() as i64;
        let v = if amounts.velocity == 0.0 {
            velocity
        } else {
            let g = 1.0 + amounts.velocity * (p.velocity - 1.0);
            (velocity as f32 * g).round().clamp(1.0, 127.0) as u8
        };
        let l = if amounts.length == 0.0 {
            length
        } else {
            let g = 1.0 + amounts.length * (p.length - 1.0);
            Ticks(((length.0 as f32 * g).round() as i64).max(1))
        };
        (Ticks(t), v, l)
    }

    /// A groove taken from `hits` — each `(tick on the timeline, strength, length)` —
    /// over `period` on a grid of `step`: for each step, the average distance of the hits
    /// nearest it, and their strength and length against the whole's average. A step no
    /// hit is near keeps the straight time. `None` when nothing was heard.
    pub fn extract(
        id: GrooveId,
        name: String,
        hits: &[(Ticks, f32, Ticks)],
        period: Ticks,
        step: Ticks,
    ) -> Option<Groove> {
        if hits.is_empty() || period <= Ticks::ZERO || step <= Ticks::ZERO {
            return None;
        }
        let steps = (period.0 / step.0).clamp(1, 512) as usize;
        let mut sum = vec![(0.0f64, 0.0f64, 0.0f64, 0u32); steps];
        let (mut vel_all, mut len_all) = (0.0f64, 0.0f64);
        for &(t, v, l) in hits {
            let phase = t.0.rem_euclid(period.0);
            let k = ((phase as f64 / step.0 as f64).round() as i64).rem_euclid(steps as i64);
            let grid = k * step.0;
            // The signed distance from the step, the short way round the period.
            let mut d = phase - grid;
            if d > period.0 / 2 {
                d -= period.0;
            } else if d < -period.0 / 2 {
                d += period.0;
            }
            let s = &mut sum[k as usize];
            s.0 += d as f64;
            s.1 += v as f64;
            s.2 += l.0 as f64;
            s.3 += 1;
            vel_all += v as f64;
            len_all += l.0 as f64;
        }
        let n = hits.len() as f64;
        let (vel_avg, len_avg) = (vel_all / n, len_all / n);
        let points = sum
            .iter()
            .enumerate()
            .map(|(k, s)| {
                let c = s.3.max(1) as f64;
                let ratio = |x: f64, avg: f64| {
                    if s.3 == 0 || avg <= 0.0 {
                        1.0
                    } else {
                        ((x / c) / avg).clamp(0.0, 4.0) as f32
                    }
                };
                GroovePoint {
                    at: Ticks(k as i64 * step.0),
                    shift: (s.0 / c) as f32,
                    velocity: ratio(s.1, vel_avg),
                    length: ratio(s.2, len_avg),
                }
            })
            .collect();
        Some(Groove {
            id,
            name,
            period: Ticks(steps as i64 * step.0),
            points,
        })
    }

    /// The groove of a note clip's notes, heard where the clip plays them first.
    pub fn from_clip(id: GrooveId, clip: &Clip, period: Ticks, step: Ticks) -> Option<Groove> {
        let mut at = Vec::new();
        let mut hits = Vec::new();
        for n in clip.notes.iter().filter(|n| !n.muted) {
            clip.timeline_positions(n.start, &mut at);
            if let Some(t) = at.first() {
                hits.push((*t, n.velocity as f32, n.length));
            }
        }
        Groove::extract(id, clip.name.clone(), &hits, period, step)
    }
}

/// The groove a source names: a built-in swing made, or the pool's.
pub fn resolve(pool: &[Groove], source: &GrooveSource) -> Option<Groove> {
    match source {
        GrooveSource::Swing { grid, percent } => Some(Groove::swing(*grid, *percent)),
        GrooveSource::Pool { id } => pool.iter().find(|g| g.id == *id).cloned(),
    }
}

/// Whether *Commit* can write `clip`'s groove into its notes exactly: always, unless the
/// clip loops over a span that is not a whole number of the groove's periods — then each
/// repetition is played with another feel, and one set of notes cannot hold them all.
pub fn commit_is_exact(clip: &Clip, pool: &[Groove]) -> bool {
    let (Some(cg), Some(period)) = (clip.groove, clip.period()) else {
        return true;
    };
    match resolve(pool, &cg.source) {
        Some(g) if g.period > Ticks::ZERO => period.0 % g.period.0 == 0,
        _ => true,
    }
}

/// `clip`'s notes with its groove written into them (*Commit*): each note moved, accented
/// and lengthened as it is heard, at its first place on the timeline. The clip's own notes
/// when it has no groove.
pub fn committed(clip: &Clip, pool: &[Groove]) -> Vec<Note> {
    let Some(cg) = clip.groove else {
        return clip.notes.clone();
    };
    let Some(g) = resolve(pool, &cg.source) else {
        return clip.notes.clone();
    };
    let mut at = Vec::new();
    clip.notes
        .iter()
        .map(|n| {
            clip.timeline_positions(n.start, &mut at);
            let Some(&heard) = at.first() else {
                return n.clone();
            };
            let (t, v, l) = g.apply(&cg, heard, n.velocity, n.length);
            Note {
                start: committed_start(clip, n.start, t - heard),
                velocity: v,
                length: l,
                ..n.clone()
            }
        })
        .collect()
}

/// A note's content start moved by `by`, never out of the window's left edge it was
/// inside, nor before the content's start.
fn committed_start(clip: &Clip, start: Ticks, by: Ticks) -> Ticks {
    let moved = start + by;
    let floor = if start >= clip.offset {
        clip.offset
    } else {
        start.min(Ticks::ZERO)
    };
    moved.max(floor.max(Ticks::ZERO).min(start))
}

/// Where a note heard at `on` in `clip` plays with the clip's groove, and how: what the
/// snapshot compiles. A grooved note never starts before the clip does.
pub fn heard(
    groove: Option<(&Groove, &ClipGroove)>,
    clip: &Clip,
    on: Ticks,
    velocity: u8,
    length: Ticks,
) -> (Ticks, u8, Ticks) {
    match groove {
        None => (on, velocity, length),
        Some((g, cg)) => {
            let (t, v, l) = g.apply(cg, on, velocity, length);
            (t.max(clip.start.min(on)), v, l)
        }
    }
}

/// Whether some clip of `project` plays with pooled groove `id` (what `RemoveGroove`
/// refuses).
pub fn in_use(project: &crate::model::Project, id: GrooveId) -> bool {
    project.tracks.iter().flat_map(|t| t.clips.iter()).any(|c| {
        c.groove
            .is_some_and(|g| g.source == GrooveSource::Pool { id })
    })
}

/// `base`, or `base 2`, `base 3` … — the first name no groove of `pool` has.
pub fn unique_name(pool: &[Groove], base: &str) -> String {
    let taken = |n: &str| pool.iter().any(|g| g.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|k| format!("{base} {k}"))
        .find(|n| !taken(n))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amounts(timing: f32, velocity: f32, length: f32) -> ClipGroove {
        ClipGroove {
            source: GrooveSource::Swing {
                grid: SwingGrid::Sixteenths,
                percent: 66,
            },
            timing,
            velocity,
            length,
        }
    }

    #[test]
    fn a_swing_moves_every_second_step_and_nothing_else() {
        let g = Groove::swing(SwingGrid::Sixteenths, 66);
        assert!(g.is_valid());
        let a = amounts(1.0, 0.0, 0.0);
        // On the beat: untouched. The second 16th: 66 % of the pair (480) = 316.8, so
        // 76.8 ticks late, rounded to 77.
        assert_eq!(
            g.apply(&a, Ticks(960), 100, Ticks(120)),
            (Ticks(960), 100, Ticks(120))
        );
        assert_eq!(g.apply(&a, Ticks(240), 100, Ticks(120)).0, Ticks(317));
        // A note a little off the step keeps its own distance from it.
        assert_eq!(g.apply(&a, Ticks(250), 100, Ticks(120)).0, Ticks(327));
        // Half the timing amount: half the swing.
        assert_eq!(
            g.apply(&amounts(0.5, 0.0, 0.0), Ticks(240), 100, Ticks(120))
                .0,
            Ticks(278)
        );
        // 50 % is straight.
        let straight = Groove::swing(SwingGrid::Eighths, 50);
        assert_eq!(
            straight.apply(&a, Ticks(480), 90, Ticks(10)),
            (Ticks(480), 90, Ticks(10))
        );
        // 8ths at 75 %: the off-beat 8th lands on the third 16th of the beat.
        let e = Groove::swing(SwingGrid::Eighths, 75);
        assert_eq!(e.apply(&a, Ticks(480), 90, Ticks(10)).0, Ticks(720));
    }

    #[test]
    fn a_groove_extracted_from_a_played_part_gives_it_back() {
        // A 16th-note part played late on the off-beats and softer on them.
        let hits: Vec<(Ticks, f32, Ticks)> = (0..32)
            .map(|k| {
                let late = if k % 2 == 1 { 60 } else { 0 };
                let vel = if k % 2 == 1 { 60.0 } else { 120.0 };
                (Ticks(k * 240 + late), vel, Ticks(200))
            })
            .collect();
        let g =
            Groove::extract(GrooveId(7), "Played".into(), &hits, Ticks(1920), Ticks(240)).unwrap();
        assert!(g.is_valid());
        assert_eq!(g.points.len(), 8);
        assert_eq!(g.points[1].shift, 60.0);
        assert_eq!(g.points[0].shift, 0.0);
        assert!((g.points[1].velocity - 60.0 / 90.0).abs() < 1e-6);
        assert!((g.points[0].velocity - 120.0 / 90.0).abs() < 1e-6);
        // Applied fully to a straight part at 90: the played one, to the tick and velocity.
        let full = amounts(1.0, 1.0, 1.0);
        assert_eq!(
            g.apply(&full, Ticks(240 * 5), 90, Ticks(200)),
            (Ticks(1260), 60, Ticks(200))
        );
        assert_eq!(
            g.apply(&full, Ticks(240 * 4), 90, Ticks(200)),
            (Ticks(960), 120, Ticks(200))
        );
        // At no amount: nothing changes.
        let none = amounts(0.0, 0.0, 0.0);
        assert_eq!(
            g.apply(&none, Ticks(1250), 33, Ticks(7)),
            (Ticks(1250), 33, Ticks(7))
        );
        assert!(
            Groove::extract(GrooveId(1), String::new(), &[], Ticks(1920), Ticks(240)).is_none()
        );
    }

    #[test]
    fn a_hit_late_across_the_period_counts_as_early_for_the_first_step() {
        // A downbeat played 30 ticks early is heard at the end of the previous period.
        let hits = [
            (Ticks(1920 - 30), 100.0, Ticks(100)),
            (Ticks(480), 100.0, Ticks(100)),
        ];
        let g =
            Groove::extract(GrooveId(1), String::new(), &hits, Ticks(1920), Ticks(480)).unwrap();
        assert_eq!(g.points[0].shift, -30.0);
        assert_eq!(g.points[1].shift, 0.0);
    }

    #[test]
    fn the_built_ins_are_the_mpc_swings_and_exist() {
        let all: Vec<GrooveSource> = GrooveSource::builtins().collect();
        assert_eq!(all.len(), 14);
        assert!(all.iter().all(|s| s.exists(&[])));
        assert!(!GrooveSource::Pool { id: GrooveId(3) }.exists(&[]));
        assert!(!GrooveSource::Swing {
            grid: SwingGrid::Eighths,
            percent: 80
        }
        .exists(&[]));
    }

    #[test]
    fn invalid_grooves_are_refused() {
        let mut g = Groove::swing(SwingGrid::Eighths, 60);
        g.points.swap(0, 1);
        assert!(!g.is_valid());
        let mut g = Groove::swing(SwingGrid::Eighths, 60);
        g.points[1].shift = f32::NAN;
        assert!(!g.is_valid());
        let mut g = Groove::swing(SwingGrid::Eighths, 60);
        g.period = Ticks::ZERO;
        assert!(!g.is_valid());
    }
}
