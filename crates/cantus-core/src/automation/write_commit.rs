//! What a finished write pass does to the project (ADR 0068), and what the engine hears
//! while it runs.
//!
//! **The commit.** Each target's runs are laid in pass order ([`overlay`]: the last pass
//! wins inside a loop), and each stretch becomes an automation clip over exactly the time
//! it covers: what played there before is cleared (`edit::cover`, the bake's rule), the
//! curve is the samples thinned to 1/500 of the control's travel (the bake's thinning, every
//! move a ramp: a hand's frames come at their own times, so nothing is guessed a jump),
//! and the clips around it play on — a touch that glided back meets the old curve where it
//! was. The project's own value of every target the hand moved is put back to what it was
//! when the pass began: the hand's moves live in the clip, and what plays before the first
//! clip is what played before. A track that wrote in **Write** comes out of the pass in
//! **Touch** (Pro Tools' *after a write pass, switch to touch*: a second pass never
//! overwrites by accident). One `Command::Batch`.
//!
//! **While it runs.** [`live_lanes`] changes the snapshot's lanes for what is being
//! written: a held control's lane drives nothing (the hand is heard through the project's
//! value), a latched one holds its value, a released touch glides back along the old curve.

use super::compile::{lane_for, AutoLane, AutoSeg};
use super::edit::cover;
use super::range::{base_value, range_of};
use super::write::{overlay, AutoMode, Live, Writer};
use super::{AutoPoint, AutoTarget, Automation, Shape};
use crate::command::Command;
use crate::model::{Project, TrackId};
use crate::time::Ticks;

/// How the commit finds a track and a name for each new clip.
pub struct Commit<'a> {
    /// The project when the pass began (what the knobs are put back to).
    pub before: &'a Project,
    /// The track a clip of the target goes under (`automation::host_track` in the
    /// application: the parameter's own track, else the selected one).
    pub host: &'a dyn Fn(&AutoTarget) -> Option<TrackId>,
    /// A new clip's name ("Lead · Cutoff").
    pub name: &'a dyn Fn(&AutoTarget) -> String,
    /// Fresh ids.
    pub ids: &'a mut dyn FnMut() -> u64,
    /// The tracks that wrote in Write mode, switched to Touch by the commit.
    pub wrote: &'a [TrackId],
    /// Whether the master wrote in Write mode.
    pub master_wrote: bool,
}

impl std::fmt::Debug for Commit<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Commit")
            .field("wrote", &self.wrote)
            .finish()
    }
}

/// The commands a pass becomes, and how many clips it made. Empty when it wrote nothing.
pub fn commit(now: &Project, writer: &Writer, c: &mut Commit<'_>) -> (Vec<Command>, usize) {
    let mut sim = now.clone();
    let mut out: Vec<Command> = Vec::new();
    let mut made = 0;
    let mut opened: Vec<TrackId> = Vec::new();
    let push = |sim: &mut Project, out: &mut Vec<Command>, cmds: Vec<Command>| {
        for cmd in cmds {
            if cmd.clone().apply(sim).is_ok() {
                out.push(cmd);
            }
        }
    };
    for (target, runs) in writer.written() {
        let Some(track) = (c.host)(target) else {
            continue;
        };
        for run in overlay(runs) {
            let (Some(first), Some(last)) = (run.first(), run.last()) else {
                continue;
            };
            let from = Ticks(first.0.max(0));
            let to = Ticks(last.0.max(from.0 + 1));
            let samples: Vec<(i64, f32)> = run.iter().map(|(t, v)| ((*t).max(0), *v)).collect();
            let mut points = thin_run(&samples, from);
            // It holds its last value to the clip's end.
            if let Some(p) = points.last().copied() {
                if p.tick < to - from {
                    points.push(AutoPoint::new(to - from, p.value));
                }
            }
            let mut cmds = cover(&sim, target, from, to);
            let mut clip = sim.make_clip((c.name)(target), from, to - from);
            clip.id = crate::model::ClipId((c.ids)());
            clip.automation = Some(Automation::new(target.clone(), points));
            cmds.push(Command::InsertClip {
                track,
                clip: Box::new(clip),
            });
            push(&mut sim, &mut out, cmds);
            made += 1;
        }
        if !opened.contains(&track) && sim.track(track).is_some_and(|t| !t.automation_open) {
            opened.push(track);
            push(
                &mut sim,
                &mut out,
                vec![Command::SetTrackAutomationOpen {
                    id: track,
                    open: true,
                }],
            );
        }
        // The project's own value back where it was: the hand lives in the clip now.
        if let Some(cmd) = restore(c.before, &sim, target) {
            push(&mut sim, &mut out, vec![cmd]);
        }
    }
    if made == 0 {
        return (Vec::new(), 0);
    }
    for &t in c.wrote {
        if sim
            .track(t)
            .is_some_and(|tr| tr.auto_mode == AutoMode::Write)
        {
            push(
                &mut sim,
                &mut out,
                vec![Command::SetTrackAutoMode {
                    track: t,
                    mode: AutoMode::Touch,
                }],
            );
        }
    }
    if c.master_wrote && sim.master.auto_mode == AutoMode::Write {
        push(
            &mut sim,
            &mut out,
            vec![Command::SetMasterAutoMode {
                mode: AutoMode::Touch,
            }],
        );
    }
    (out, made)
}

/// The command that puts `target`'s own value in `now` back to what `before` held, or
/// `None` when it is the same (or names nothing any more).
pub fn restore(before: &Project, now: &Project, target: &AutoTarget) -> Option<Command> {
    match target {
        // A hosted parameter: the value the project stored for it, or none.
        AutoTarget::Plugin { effect, id } => {
            let stored = |p: &Project| {
                let h = p.effect_host(*effect)?;
                let x = p.effect(h, *effect)?.spec.external.as_deref()?;
                Some(x.params.get(id).copied())
            };
            let (was, is) = (stored(before)?, stored(now)?);
            (was.map(f64::to_bits) != is.map(f64::to_bits)).then(|| Command::SetPluginParam {
                strip: now.effect_host(*effect).expect("checked"),
                effect: *effect,
                id: *id,
                value: was,
            })
        }
        AutoTarget::PluginInstrument { track, id } => {
            let stored = |p: &Project| {
                let x = p.track(*track)?.instrument.external()?;
                Some(x.params.get(id).copied())
            };
            let (was, is) = (stored(before)?, stored(now)?);
            (was.map(f64::to_bits) != is.map(f64::to_bits)).then_some(
                Command::SetInstrumentPluginParam {
                    track: *track,
                    id: *id,
                    value: was,
                },
            )
        }
        _ => {
            let (was, is) = (base_value(before, target)?, base_value(now, target)?);
            if was.to_bits() == is.to_bits() {
                return None;
            }
            target.command(now, f64::from(was))
        }
    }
}

/// Where `target` was heard in `project` at `tick` (0…1 of its travel): its curve there, or
/// its own value before any clip. A hosted parameter with no curve has no position the
/// project knows: `fallback`.
pub fn heard_pos(project: &Project, target: &AutoTarget, tick: i64, fallback: f32) -> f32 {
    if let Some(lane) = lane_for(project, target) {
        if let Some(p) = lane.pos_at(tick as f64) {
            return p;
        }
    }
    if target.is_plugin() {
        return fallback;
    }
    match (range_of(project, target), base_value(project, target)) {
        (Some(r), Some(v)) if v.is_finite() => r.pos(v),
        _ => fallback,
    }
}

/// The snapshot's lanes changed for what `writer` is writing (see the module's docs). A
/// target held by a hand that no lane drives needs nothing; one gliding back that no lane
/// drove gets a lane for the glide, its value before the pass held after it.
pub fn live_lanes(
    lanes: &mut Vec<AutoLane>,
    writer: &Writer,
    now: &Project,
    before: &Project,
    at: i64,
) {
    for target in writer.targets.keys() {
        let live = writer.live_at(target, at);
        if live == Live::Curve {
            continue;
        }
        let i = lanes.iter().position(|l| &l.target == target);
        match (live, i) {
            (Live::Hand, Some(i)) => lanes[i].segs.clear(),
            (Live::Hold(v), Some(i)) => lanes[i].segs = vec![hold(Ticks::ZERO, v)],
            (Live::Glide { a, b, from, to }, Some(i)) => {
                let segs = std::mem::take(&mut lanes[i].segs);
                lanes[i].segs = glide_into(&segs, a, b, from, to);
            }
            (Live::Glide { a, b, from, to }, None) => {
                let (Some(range), Some(base)) = (range_of(now, target), base_value(now, target))
                else {
                    continue;
                };
                // A hosted parameter glides back to where the writer sends it (its stored value,
                // `to`): heard as written.
                let old = if target.is_plugin() {
                    to
                } else {
                    base_value(before, target).map_or(to, |v| range.pos(v))
                };
                let mut segs = glide_into(&[], a, b, from, to);
                segs.push(hold(Ticks(b), old));
                lanes.push(AutoLane {
                    target: target.clone(),
                    range,
                    base,
                    segs,
                });
            }
            _ => {}
        }
    }
    lanes.sort_by(|a, b| a.target.cmp(&b.target));
}

fn hold(at: Ticks, v: f32) -> AutoSeg {
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

/// `segs` with `[a, b)` replaced by a straight glide from `from` to `to`, the old curve
/// going on from `b`.
fn glide_into(segs: &[AutoSeg], a: i64, b: i64, from: f32, to: f32) -> Vec<AutoSeg> {
    let (ta, tb) = (Ticks(a), Ticks(b.max(a)));
    let mut out: Vec<AutoSeg> = segs.iter().copied().filter(|s| s.at < ta).collect();
    out.push(AutoSeg {
        at: ta,
        t0: ta,
        t1: tb,
        v0: from,
        v1: to,
        shape: Shape::Curve,
        tension: 0.0,
    });
    // The old piece under `b`, from `b` on.
    let k = segs.partition_point(|s| s.at <= tb);
    if k > 0 {
        let mut s = segs[k - 1];
        s.at = tb;
        if s.t1 <= tb {
            // Past its own curve: it holds where it ended.
            s = hold(tb, s.v1);
        }
        out.push(s);
    } else {
        // Before the first clip: the value the glide went back to (the project's own).
        out.push(hold(tb, to));
    }
    out.extend(segs.iter().copied().filter(|s| s.at > tb));
    out
}

/// A run's samples as a curve's points from `origin`, thinned to the bake's tolerance with
/// every change a ramp between its samples (a released touch's glide is one).
fn thin_run(samples: &[(i64, f32)], origin: Ticks) -> Vec<AutoPoint> {
    let ticks: Vec<i64> = samples.iter().map(|s| s.0).collect();
    let values: Vec<f32> = samples.iter().map(|s| s.1).collect();
    let jumps = vec![false; samples.len()];
    crate::modulator::bake::thin_marked(&ticks, &values, &jumps, origin.0)
}
