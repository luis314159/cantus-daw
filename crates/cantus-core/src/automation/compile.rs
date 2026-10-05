//! Every automation clip in a project, compiled into one function of time per target —
//! what the snapshot hands the engine and what the knobs read.
//!
//! **Which clip plays.** All unmuted automation clips with the same target, on any track,
//! make one lane. Where clips overlap, the one that **started last** plays (a tie goes to
//! the later one in track and clip order), so a short clip dropped on a long one takes
//! over for its length and the long one comes back after it. Between clips the value
//! **holds** where the last clip left it, as FL's do; before the first clip the
//! parameter is what the project holds (its knob), so a project renders the same from any
//! start point.

use super::range::{base_value, range_of, AutoRange};
use super::{AutoTarget, Automation, Shape};
use crate::model::{Clip, Project};
use crate::time::Ticks;
use std::collections::BTreeMap;

/// One piece of a lane: from `at` until the next piece starts, the curve between two
/// points placed on the timeline at `t0` and `t1` (which may lie outside the piece, when
/// a clip's window cuts a curve in two).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct AutoSeg {
    pub at: Ticks,
    pub t0: Ticks,
    pub t1: Ticks,
    pub v0: f32,
    pub v1: f32,
    pub shape: Shape,
    pub tension: f32,
}

impl AutoSeg {
    fn hold(at: Ticks, v: f32) -> Self {
        AutoSeg {
            at,
            t0: at,
            t1: at,
            v0: v,
            v1: v,
            shape: Shape::Hold,
            tension: 0.0,
        }
    }

    /// The curve's position (0…1) at a timeline tick inside the piece.
    #[inline]
    pub fn pos(&self, tick: f64) -> f32 {
        let span = (self.t1.0 - self.t0.0) as f64;
        if span <= 0.0 {
            return self.v1;
        }
        let u = ((tick - self.t0.0 as f64) / span) as f32;
        self.v0 + (self.v1 - self.v0) * self.shape.at(self.tension, u)
    }

    /// Whether the piece holds one value from start to end.
    #[inline]
    pub fn is_flat(&self) -> bool {
        self.v0 == self.v1 || self.shape == Shape::Hold || self.t1 <= self.t0
    }
}

/// One target's automation over the whole timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct AutoLane {
    pub target: AutoTarget,
    pub range: AutoRange,
    /// The value, in the parameter's units, before the first clip.
    pub base: f32,
    /// Sorted by `at`; the last piece lasts forever.
    pub segs: Vec<AutoSeg>,
}

impl AutoLane {
    /// The piece playing at `tick`, or `None` before the first clip.
    #[inline]
    pub fn seg_at(&self, tick: f64) -> Option<usize> {
        let i = self.segs.partition_point(|s| s.at.0 as f64 <= tick);
        i.checked_sub(1)
    }

    /// The curve's position (0…1) at `tick`, or `None` before the first clip.
    #[inline]
    pub fn pos_at(&self, tick: f64) -> Option<f32> {
        self.seg_at(tick).map(|i| self.segs[i].pos(tick))
    }

    /// The parameter's value at `tick`, in its own units.
    #[inline]
    pub fn value_at(&self, tick: f64) -> f32 {
        match self.pos_at(tick) {
            Some(p) => self.range.value(p),
            None => self.base,
        }
    }

    /// Where the next piece starts after `tick`, if any.
    #[inline]
    pub fn next_after(&self, tick: f64) -> Option<Ticks> {
        let i = self.segs.partition_point(|s| s.at.0 as f64 <= tick);
        self.segs.get(i).map(|s| s.at)
    }

    /// Whether the value is changing at `tick` (inside a piece that moves).
    #[inline]
    pub fn moving_at(&self, tick: f64) -> bool {
        self.seg_at(tick).is_some_and(|i| !self.segs[i].is_flat())
    }
}

/// Compile every automated target of `project` into a lane, in target order. A target
/// that names nothing (an effect since removed) is left out: its clips stay in the
/// project and play again when undo brings the effect back.
pub fn compile(project: &Project) -> Vec<AutoLane> {
    let mut by_target: BTreeMap<&AutoTarget, Vec<(&Clip, &Automation)>> = BTreeMap::new();
    for t in &project.tracks {
        for c in &t.clips {
            let Some(a) = c.automation.as_ref() else {
                continue;
            };
            if c.muted || a.points.is_empty() || c.length <= Ticks::ZERO {
                continue;
            }
            by_target.entry(&a.target).or_default().push((c, a));
        }
    }
    let mut out = Vec::with_capacity(by_target.len());
    for (target, clips) in by_target {
        let (Some(range), Some(base)) = (range_of(project, target), base_value(project, target))
        else {
            continue;
        };
        out.push(AutoLane {
            target: target.clone(),
            range,
            base,
            segs: lane_segs(&clips),
        });
    }
    out
}

/// The lane for one target, compiled on its own (what a knob that only needs its own
/// value asks for); `None` when nothing automates it.
pub fn lane_for(project: &Project, target: &AutoTarget) -> Option<AutoLane> {
    let clips: Vec<(&Clip, &Automation)> = project
        .tracks
        .iter()
        .flat_map(|t| t.clips.iter())
        .filter_map(|c| c.automation.as_ref().map(|a| (c, a)))
        .filter(|(c, a)| {
            &a.target == target && !c.muted && !a.points.is_empty() && c.length > Ticks::ZERO
        })
        .collect();
    if clips.is_empty() {
        return None;
    }
    Some(AutoLane {
        target: target.clone(),
        range: range_of(project, target)?,
        base: base_value(project, target)?,
        segs: lane_segs(&clips),
    })
}

/// Resolve overlaps (the latest start wins) and lay every winning stretch's curve on the
/// timeline, holding between clips. `clips` is in track and clip order.
fn lane_segs(clips: &[(&Clip, &Automation)]) -> Vec<AutoSeg> {
    // Every boundary, then the winner of each elementary stretch between two of them.
    let mut cuts: Vec<Ticks> = clips.iter().flat_map(|(c, _)| [c.start, c.end()]).collect();
    cuts.sort();
    cuts.dedup();
    let mut stretches: Vec<(usize, Ticks, Ticks)> = Vec::new();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let winner = clips
            .iter()
            .enumerate()
            .filter(|(_, (c, _))| c.start <= a && c.end() >= b)
            .max_by_key(|(i, (c, _))| (c.start, *i))
            .map(|(i, _)| i);
        let Some(k) = winner else { continue };
        match stretches.last_mut() {
            Some(last) if last.0 == k && last.2 == a => last.2 = b,
            _ => stretches.push((k, a, b)),
        }
    }
    let mut segs = Vec::new();
    for (k, a, b) in stretches {
        let (clip, auto) = clips[k];
        emit(clip, auto, a, b, &mut segs);
    }
    segs
}

/// The curve of `clip` over its timeline stretch `[a, b)`, then a hold at `b` of the
/// value it reached there.
fn emit(clip: &Clip, auto: &Automation, a: Ticks, b: Ticks, out: &mut Vec<AutoSeg>) {
    let pts = &auto.points;
    // Each repetition of the content (one, for a clip that does not loop) inside [a, b):
    // timeline `t` shows content `t - shift`.
    let period = clip.period().filter(|p| *p < clip.length);
    let mut end_value = None;
    let mut rep_start = clip.start;
    loop {
        let rep_end = match period {
            Some(p) => (rep_start + p).min(clip.end()),
            None => clip.end(),
        };
        let (ta, tb) = (rep_start.max(a), rep_end.min(b));
        if ta < tb {
            let shift = rep_start - clip.offset;
            let (ca, cb) = (ta - shift, tb - shift);
            // The piece of curve under `ca`, then one per point inside the stretch.
            let i0 = pts.partition_point(|p| p.tick <= ca);
            if i0 == 0 {
                out.push(AutoSeg::hold(ta, pts[0].value));
            } else {
                out.push(piece(pts, i0 - 1, ta, shift));
            }
            for j in i0..pts.len() {
                if pts[j].tick >= cb {
                    break;
                }
                out.push(piece(pts, j, pts[j].tick + shift, shift));
            }
            // The value the curve reached where the stretch ends (for a repetition that
            // ends there, the content's end, before it would start again).
            end_value = auto.value_at(cb.0 as f64);
        }
        match period {
            Some(p) if rep_end < clip.end() && rep_end < b => rep_start += p,
            _ => break,
        }
    }
    if let Some(v) = end_value {
        out.push(AutoSeg::hold(b, v));
    }
}

/// The piece starting at timeline `at` that follows the curve from point `j` to the next.
fn piece(pts: &[super::AutoPoint], j: usize, at: Ticks, shift: Ticks) -> AutoSeg {
    let p = pts[j];
    match pts.get(j + 1) {
        Some(q) => AutoSeg {
            at,
            t0: p.tick + shift,
            t1: q.tick + shift,
            v0: p.value,
            v1: q.value,
            shape: p.shape,
            tension: p.tension,
        },
        None => AutoSeg::hold(at, p.value),
    }
}
