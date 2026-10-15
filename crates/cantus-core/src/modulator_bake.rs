//! *Bake to automation* (ADR 0057): a modulator's effect on one parameter turned into an
//! automation clip's curve — what the parameter's position is, base and this modulator's
//! offset together, over a stretch of the song — so the route can go and the sound stays.
//!
//! **What is sampled.** The position is read every 1/96 of a beat: every synced division
//! (8/1 … 1/32, dotted and triplet) starts on that grid, so a step sequence and a held
//! random source change exactly on a sample, and their jumps are kept as jumps (a *hold*
//! point and the next value at the step's own tick). The rest is a polyline thinned until
//! no point is further than 1/500 of the travel from the line through its neighbours.
//!
//! **Where the outputs come from.** A source that is a function of time is evaluated here,
//! with the engine's own functions (`eval`); one that follows audio cannot be known ahead
//! and is handed in as the engine traced it during an offline render.

use super::eval::{output, Gates, ModTime};
use super::ModSource;
use crate::automation::compile::lane_for;
use crate::automation::range::{base_value, range_of};
use crate::automation::{AutoPoint, AutoTarget, Shape};
use crate::model::{ModulatorId, Project};
use crate::time::{Ticks, PPQ};

/// Samples a beat holds: every synced division's boundary falls on one.
pub const PER_BEAT: i64 = 96;
/// The thinning tolerance, in travel (0…1).
pub const TOLERANCE: f32 = 0.002;

/// A modulator's output (× its amount) at a tick, as the engine traced it.
pub type Trace = [(f64, f32)];

/// The curve that plays `target` over `[from, to)` with modulator `id`'s route into it
/// baked in, in ticks from `from` (an automation clip's content), or `None` when there is
/// no such route or the target names nothing. `base` is the target's own position where no
/// clip automates it, when the caller knows it better than the project (a hosted plug-in's
/// value, which only its unit can normalise); `play_from` is where playback is taken to
/// start (a free LFO runs from there); `trace`, the outputs of a source that follows audio.
#[allow(clippy::too_many_arguments)]
pub fn bake(
    project: &Project,
    id: ModulatorId,
    target: &AutoTarget,
    from: Ticks,
    to: Ticks,
    play_from: Ticks,
    base: Option<f32>,
    trace: Option<&Trace>,
) -> Option<Vec<AutoPoint>> {
    let (host, _, m) = project.modulator(id)?;
    let route = m.route(target)?;
    let range = range_of(project, target)?;
    if to <= from {
        return None;
    }
    let lane = lane_for(project, target);
    let own = base.or_else(|| {
        base_value(project, target)
            .filter(|v| v.is_finite())
            .map(|v| range.pos(v))
    })?;
    let amount_lane = lane_for(project, &AutoTarget::ModAmount(id));
    let bpm = project.tempo_bpm;
    let gates = notes_gates(project, host, &m.source);
    let play = play_from.0 as f64 / PPQ as f64;
    let step = (PPQ / PER_BEAT).max(1);
    let mut ticks = Vec::new();
    let mut t = from.0;
    while t < to.0 {
        ticks.push(t);
        t += step;
    }
    ticks.push(to.0);
    let values: Vec<f32> = ticks
        .iter()
        .map(|&tick| {
            let at = tick as f64;
            let base_pos = lane.as_ref().and_then(|l| l.pos_at(at)).unwrap_or(own);
            let out = match trace {
                Some(tr) => traced(tr, at),
                None => {
                    let time = ModTime::at_beats(at / PPQ as f64, play, bpm);
                    let amount = amount_lane
                        .as_ref()
                        .and_then(|l| l.pos_at(at))
                        .unwrap_or(m.amount)
                        .clamp(0.0, 1.0);
                    output(&m.source, &time, gates.as_ref(), None) * amount
                }
            };
            (base_pos + route.depth * out).clamp(0.0, 1.0)
        })
        .collect();
    Some(thin(&ticks, &values, from.0))
}

/// The traced output at `tick`: the value of the last control point at or before it.
fn traced(trace: &Trace, tick: f64) -> f32 {
    let i = trace.partition_point(|(t, _)| *t <= tick);
    match i.checked_sub(1) {
        Some(i) => trace[i].1,
        None => trace.first().map_or(0.0, |x| x.1),
    }
}

/// The note gates a source hears, compiled from the project as the snapshot does.
fn notes_gates(project: &Project, host: super::ModHost, src: &ModSource) -> Option<Gates> {
    let track = match src {
        ModSource::Lfo(l) if l.retrigger == super::Retrigger::Note => {
            l.notes.or_else(|| project.host_notes_track(host))
        }
        ModSource::Envelope(e) => match e.trigger {
            super::EnvTrigger::Notes { track } => track.or_else(|| project.host_notes_track(host)),
            super::EnvTrigger::Audio { .. } => None,
        },
        _ => None,
    }?;
    let snap = crate::snapshot::Snapshot::compile(project, 0);
    let t = snap.track(track)?;
    let gates = Gates::of_events(&t.events);
    Some(match src {
        ModSource::Envelope(e) => gates.with_levels(e, 60.0 / project.tempo_bpm.max(1e-9)),
        _ => gates,
    })
}

/// The samples as points in content ticks (from `origin`): jumps kept as a hold and the
/// next value, the rest thinned to [`TOLERANCE`].
pub fn thin(ticks: &[i64], values: &[f32], origin: i64) -> Vec<AutoPoint> {
    let n = values.len();
    // A jump between two samples: a change far steeper than its neighbours'.
    let d = |i: usize| (values[i + 1] - values[i]).abs();
    let jump = |i: usize| {
        if i + 1 >= n {
            return false;
        }
        let here = d(i);
        let before = if i > 0 { d(i - 1) } else { 0.0 };
        let after = if i + 2 < n { d(i + 1) } else { 0.0 };
        here > 0.01 && here > 4.0 * before.max(after) + 1e-4
    };
    let mut out = Vec::new();
    let mut run_start = 0;
    let mut i = 0;
    while i < n {
        if i + 1 == n || jump(i) {
            // A run of smooth samples [run_start, i], thinned; a jump after it holds.
            let kept = douglas_peucker(ticks, values, run_start, i);
            for k in kept {
                let mut p = AutoPoint::new(Ticks(ticks[k] - origin), values[k]);
                if k == i && i + 1 < n {
                    p.shape = Shape::Hold;
                }
                out.push(p);
            }
            run_start = i + 1;
        }
        i += 1;
    }
    out
}

/// The indices Ramer–Douglas–Peucker keeps of `[a, b]` (both ends always).
fn douglas_peucker(ticks: &[i64], values: &[f32], a: usize, b: usize) -> Vec<usize> {
    if b <= a {
        return vec![a];
    }
    let mut keep = vec![false; b - a + 1];
    keep[0] = true;
    keep[b - a] = true;
    let mut stack = vec![(a, b)];
    while let Some((lo, hi)) = stack.pop() {
        if hi <= lo + 1 {
            continue;
        }
        let (t0, v0) = (ticks[lo] as f64, f64::from(values[lo]));
        let (t1, v1) = (ticks[hi] as f64, f64::from(values[hi]));
        let mut worst = (0.0f64, lo);
        for k in lo + 1..hi {
            let u = (ticks[k] as f64 - t0) / (t1 - t0).max(1.0);
            let line = v0 + (v1 - v0) * u;
            let e = (f64::from(values[k]) - line).abs();
            if e > worst.0 {
                worst = (e, k);
            }
        }
        if worst.0 > f64::from(TOLERANCE) {
            keep[worst.1 - a] = true;
            stack.push((lo, worst.1));
            stack.push((worst.1, hi));
        }
    }
    keep.iter()
        .enumerate()
        .filter(|(_, k)| **k)
        .map(|(i, _)| a + i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::Automation;
    use crate::modulator::*;

    fn with_route(kind: ModKind, depth: f32) -> (Project, ModulatorId, AutoTarget) {
        let mut p = Project::new("bake");
        p.tempo_bpm = 120.0;
        let t = p.make_track("t");
        let s = t.strip.unwrap();
        p.tracks.push(t);
        let target = AutoTarget::StripPan(s);
        let mut m = Modulator::new(ModulatorId(p.alloc_id()), kind);
        m.routes.push(ModRoute {
            target: target.clone(),
            depth,
        });
        let id = m.id;
        p.tracks[0].modulators.push(m);
        (p, id, target)
    }

    /// The baked curve, played as a clip from `from`, reads what the base and the
    /// modulator together give at every sample — within the thinning tolerance.
    fn check_against_source(p: &Project, id: ModulatorId, target: &AutoTarget, beats: i64) {
        let (from, to) = (Ticks::beats(1), Ticks::beats(1 + beats));
        let pts = bake(p, id, target, from, to, Ticks::ZERO, None, None).unwrap();
        let curve = Automation::new(target.clone(), pts.clone());
        assert!(curve.is_valid());
        let m = p.modulator(id).unwrap().2;
        let base = range_of(p, target)
            .unwrap()
            .pos(base_value(p, target).unwrap());
        let mut worst = 0f32;
        for k in 0..(beats * PER_BEAT) {
            let tick = from.0 + k * (PPQ / PER_BEAT);
            let time = ModTime::at_beats(tick as f64 / PPQ as f64, 0.0, p.tempo_bpm);
            let want =
                (base + m.routes[0].depth * output(&m.source, &time, None, None)).clamp(0.0, 1.0);
            let got = curve.value_at((tick - from.0) as f64).unwrap();
            worst = worst.max((got - want).abs());
        }
        assert!(worst <= TOLERANCE * 1.01, "worst {worst}");
        assert!(
            pts.len() < (beats * PER_BEAT) as usize / 2,
            "{} points",
            pts.len()
        );
    }

    #[test]
    fn a_baked_lfo_reads_what_the_lfo_played() {
        let (p, id, target) = with_route(ModKind::Lfo, 0.4);
        check_against_source(&p, id, &target, 4);
    }

    #[test]
    fn baked_steps_and_held_randoms_jump_on_their_own_ticks() {
        for kind in [ModKind::Step, ModKind::Random] {
            let (p, id, target) = with_route(kind, -0.5);
            check_against_source(&p, id, &target, 4);
            let pts = bake(
                &p,
                id,
                &target,
                Ticks::ZERO,
                Ticks::beats(2),
                Ticks::ZERO,
                None,
                None,
            )
            .unwrap();
            // Every jump is a hold point and a point at the step's own tick.
            let holds = pts.iter().filter(|q| q.shape == Shape::Hold).count();
            assert!(holds >= 4, "{kind:?}: {holds} holds");
        }
    }

    #[test]
    fn a_trace_bakes_a_follower_as_the_engine_heard_it() {
        let (p, id, target) = with_route(ModKind::Follower, 1.0);
        let trace: Vec<(f64, f32)> = (0..400)
            .map(|k| (k as f64 * 24.0, if k % 100 < 50 { 0.3 } else { 0.0 }))
            .collect();
        let pts = bake(
            &p,
            id,
            &target,
            Ticks::ZERO,
            Ticks::beats(10),
            Ticks::ZERO,
            None,
            Some(&trace),
        )
        .unwrap();
        let c = Automation::new(target.clone(), pts);
        assert!((c.value_at(100.0).unwrap() - 0.8).abs() < 1e-3);
        assert!((c.value_at(1300.0).unwrap() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn nothing_to_bake_is_none() {
        let (p, id, target) = with_route(ModKind::Lfo, 0.4);
        let none = ModulatorId(123_456);
        assert!(bake(
            &p,
            none,
            &target,
            Ticks::ZERO,
            Ticks::beats(1),
            Ticks::ZERO,
            None,
            None
        )
        .is_none());
        assert!(bake(
            &p,
            id,
            &AutoTarget::MasterGain,
            Ticks::ZERO,
            Ticks::beats(1),
            Ticks::ZERO,
            None,
            None
        )
        .is_none());
        assert!(bake(
            &p,
            id,
            &target,
            Ticks::beats(1),
            Ticks::ZERO,
            Ticks::ZERO,
            None,
            None
        )
        .is_none());
    }
}
