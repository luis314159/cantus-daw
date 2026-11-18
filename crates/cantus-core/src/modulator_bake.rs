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
//!
//! **A controller** (fxd: an X-Y pad, a formula) is read as the engine plays its route:
//! its inputs from their lanes at each sample (automation on `AutoTarget::ModInput`, else
//! the source's own), its formula compiled once, the route's own axis and range
//! (`route_value`). A pad with inertia, or a controller another modulator moves the inputs
//! of, is state the engine steps: it is traced (`needs_trace`), the trace holding the
//! route's value (the engine's `trace_route`).

use super::eval::{controller_outputs, output, Gates, ModTime};
use super::{route_value, ModSource, Modulator, RouteRange};
use crate::automation::compile::{lane_for, AutoLane};
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
    // Through the tempo map (ADR 0060): at one tempo the old arithmetic.
    let map = project.tempo_map();
    let reader = Reader::new(project, host, m, target);
    // One level of nesting, as the engine plays it: every modulator moving this one's
    // amount, with its own gates and its own amount (automation in, unmodulated).
    let amount_target = AutoTarget::ModAmount(id);
    let nested: Vec<_> = project
        .routes_into(&amount_target)
        .filter(|(n, r)| n.id != id && !n.muted && r.depth != 0.0 && r.depth.is_finite())
        .filter_map(|(n, r)| {
            let (n_host, _, _) = project.modulator(n.id)?;
            Some((
                n,
                r.depth.clamp(-1.0, 1.0),
                Reader::new(project, n_host, n, &amount_target),
                lane_for(project, &AutoTarget::ModAmount(n.id)),
            ))
        })
        .collect();
    let play = play_from.0 as f64 / PPQ as f64;
    let step = (PPQ / PER_BEAT).max(1);
    let mut ticks = Vec::new();
    let mut t = from.0;
    while t < to.0 {
        ticks.push(t);
        t += step;
    }
    ticks.push(to.0);
    let value = |tick: i64| {
        let at = tick as f64;
        let base_pos = lane.as_ref().and_then(|l| l.pos_at(at)).unwrap_or(own);
        let out = match trace {
            Some(tr) => traced(tr, at),
            None => {
                let time = ModTime::at_beats_in(at / PPQ as f64, play, &map);
                let mut amount = amount_lane
                    .as_ref()
                    .and_then(|l| l.pos_at(at))
                    .unwrap_or(m.amount);
                for (n, depth, n_reader, n_lane) in &nested {
                    let n_amount = n_lane
                        .as_ref()
                        .and_then(|l| l.pos_at(at))
                        .unwrap_or(n.amount);
                    amount += depth * n_reader.value(&time, at) * n_amount;
                }
                reader.value(&time, at) * amount.clamp(0.0, 1.0)
            }
        };
        (base_pos + route.depth * out).clamp(0.0, 1.0)
    };
    // Each sample, and where the curve jumps onto it (a step, a held random, a square's
    // edge, or any of them moving the amount): its value a tick before too, so the curve
    // runs on to the jump and holds there — exact however steep the rest is.
    let mut at = Vec::with_capacity(ticks.len() + 16);
    let mut values = Vec::with_capacity(ticks.len() + 16);
    let mut jumps = Vec::with_capacity(ticks.len() + 16);
    for (k, &tick) in ticks.iter().enumerate() {
        let v = value(tick);
        if k > 0 && tick - 1 > ticks[k - 1] {
            let (prev, left): (f32, f32) = (values[values.len() - 1], value(tick - 1));
            let span = (tick - 1 - ticks[k - 1]) as f32;
            let slope = (left - prev).abs() / span;
            let jump = (v - left).abs();
            if jump > 0.003 && jump > 4.0 * slope + 1e-4 {
                at.push(tick - 1);
                values.push(left);
                jumps.push(true);
            }
        }
        at.push(tick);
        values.push(v);
        jumps.push(false);
    }
    Some(thin_marked(&at, &values, &jumps, from.0))
}

/// Whether baking modulator `id` needs the engine's trace of an offline render: it is
/// state the engine steps (it follows audio, a pad's inertia, a controller whose inputs
/// another modulator moves), or something that is moves its amount.
pub fn needs_trace(project: &Project, id: ModulatorId) -> bool {
    let amount_target = AutoTarget::ModAmount(id);
    project.modulator(id).is_some_and(|(_, _, m)| {
        stepped(project, m)
            || project
                .routes_into(&amount_target)
                .any(|(n, _)| n.id != id && !n.muted && stepped(project, n))
    })
}

/// Whether `m`'s output is only known by stepping it as the engine does: what follows
/// audio, a pad with inertia (fxd), a controller whose inputs another modulator drives.
fn stepped(project: &Project, m: &Modulator) -> bool {
    m.source.stateful()
        || (0..m.source.kind().inputs()).any(|input| {
            project
                .routes_into(&AutoTarget::ModInput {
                    modulator: m.id,
                    input,
                })
                .any(|(n, r)| n.id != m.id && !n.muted && r.depth != 0.0)
        })
}

/// One modulator as a bake reads its route into one target, made once per bake: the
/// gates it hears, and for a controller its input lanes, its compiled formula and the
/// route's axis and range.
struct Reader<'a> {
    src: &'a ModSource,
    gates: Option<Gates>,
    controller: bool,
    inputs: [f32; 3],
    input_lanes: [Option<AutoLane>; 3],
    program: Option<super::formula::Program>,
    link: (u8, Option<RouteRange>),
    bipolar: bool,
}

impl<'a> Reader<'a> {
    fn new(project: &Project, host: super::ModHost, m: &'a Modulator, target: &AutoTarget) -> Self {
        let mut inputs = [0.0f32; 3];
        let mut input_lanes = [None, None, None];
        for k in 0..m.source.kind().inputs() {
            inputs[usize::from(k)] = m.source.input(k).unwrap_or(0.0).clamp(0.0, 1.0);
            input_lanes[usize::from(k)] = lane_for(
                project,
                &AutoTarget::ModInput {
                    modulator: m.id,
                    input: k,
                },
            );
        }
        Reader {
            src: &m.source,
            gates: notes_gates(project, host, &m.source),
            controller: super::ModKind::CONTROLLERS.contains(&m.source.kind()),
            inputs,
            input_lanes,
            program: match &m.source {
                ModSource::Formula(f) => super::formula::Program::compile(&f.text).ok(),
                _ => None,
            },
            link: m.source.link(target),
            bipolar: m.source.bipolar(),
        }
    }

    /// What the route takes from the source at `time` (tick `at`), before depth and
    /// amount: a controller's inputs from their lanes there, its outputs through the
    /// route's axis and range, as the engine's `mod_sum` takes them.
    fn value(&self, time: &ModTime, at: f64) -> f32 {
        if !self.controller {
            return output(self.src, time, self.gates.as_ref(), None);
        }
        let mut ins = self.inputs;
        for (v, lane) in ins.iter_mut().zip(&self.input_lanes) {
            if let Some(p) = lane.as_ref().and_then(|l| l.pos_at(at)) {
                *v = p.clamp(0.0, 1.0);
            }
        }
        let outs = controller_outputs(self.src, self.program.as_ref(), time, ins);
        route_value(self.link.0, self.link.1.as_ref(), outs, self.bipolar)
    }
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
        ModSource::Envelope(e) => gates.with_levels_in(e, &project.tempo_map()),
        _ => gates,
    })
}

/// The samples as points in content ticks (from `origin`): jumps kept as a hold and the
/// next value, the rest thinned to [`TOLERANCE`].
pub fn thin(ticks: &[i64], values: &[f32], origin: i64) -> Vec<AutoPoint> {
    let n = values.len();
    // A jump between two samples: a change far steeper than its neighbours'.
    let d = |i: usize| (values[i + 1] - values[i]).abs();
    let jumps: Vec<bool> = (0..n)
        .map(|i| {
            if i + 1 >= n {
                return false;
            }
            let here = d(i);
            let before = if i > 0 { d(i - 1) } else { 0.0 };
            let after = if i + 2 < n { d(i + 1) } else { 0.0 };
            here > 0.01 && here > 4.0 * before.max(after) + 1e-4
        })
        .collect();
    thin_marked(ticks, values, &jumps, origin)
}

/// [`thin`] with the jumps known: `jumps[i]` when the curve jumps from sample `i` to the
/// next.
pub fn thin_marked(ticks: &[i64], values: &[f32], jumps: &[bool], origin: i64) -> Vec<AutoPoint> {
    let n = values.len();
    let jump = |i: usize| i + 1 < n && jumps.get(i).copied().unwrap_or(false);
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

    /// fxd: the controller `id`'s source, edited.
    fn edit_source(p: &mut Project, id: ModulatorId, f: impl FnOnce(&mut ModSource)) {
        let (host, i, _) = p.modulator(id).unwrap();
        f(&mut p.modulators_mut(host).unwrap()[i].source);
    }

    /// fxd: a straight automation clip on `target` from 0 to 1 over the first `beats`.
    fn ramp(p: &mut Project, target: AutoTarget, beats: i64) {
        let pts = vec![
            AutoPoint::new(Ticks::ZERO, 0.0),
            AutoPoint::new(Ticks::beats(beats), 1.0),
        ];
        let c = p.make_automation_clip("ramp", target, Ticks::ZERO, Ticks::beats(beats), pts);
        p.tracks[0].clips.push(c);
    }

    /// The bake over the first `beats` against `want(beat)` at every sample.
    fn check_against(
        p: &Project,
        id: ModulatorId,
        target: &AutoTarget,
        beats: i64,
        want: impl Fn(f64) -> f32,
    ) {
        let to = Ticks::beats(beats);
        let pts = bake(p, id, target, Ticks::ZERO, to, Ticks::ZERO, None, None).unwrap();
        let curve = Automation::new(target.clone(), pts);
        assert!(curve.is_valid());
        let mut worst = 0f32;
        for k in 0..(beats * PER_BEAT) {
            let tick = k * (PPQ / PER_BEAT);
            let got = curve.value_at(tick as f64).unwrap();
            worst = worst.max((got - want(tick as f64 / PPQ as f64)).abs());
        }
        assert!(worst <= TOLERANCE * 1.01, "worst {worst}");
    }

    #[test]
    fn a_pad_bakes_the_axis_and_the_range_its_route_reads_with_its_input_lanes() {
        let (mut p, id, target) = with_route(ModKind::XyPad, 0.8);
        let range = RouteRange {
            from: 0.0,
            to: 0.6,
            curve: 0.4,
        };
        let t = target.clone();
        edit_source(&mut p, id, |s| {
            let ModSource::XyPad(x) = s else {
                unreachable!()
            };
            x.x = 0.9;
            x.y = 0.3;
            x.links.push(RouteLink {
                target: t,
                output: 1,
                range: Some(range),
            });
        });
        // Y moved by automation from 0 to 1 over four beats (X, unread, stays at 0.9).
        ramp(
            &mut p,
            AutoTarget::ModInput {
                modulator: id,
                input: 1,
            },
            4,
        );
        assert!(!needs_trace(&p, id));
        check_against(&p, id, &target, 4, |beat| {
            let y = (beat / 4.0) as f32;
            0.5 + 0.8 * (0.6 * crate::automation::Shape::Curve.at(0.4, y))
        });
    }

    #[test]
    fn a_formula_bakes_with_its_automated_knob_and_its_compiled_program() {
        let (mut p, id, target) = with_route(ModKind::Formula, 0.5);
        edit_source(&mut p, id, |s| {
            let ModSource::Formula(f) = s else {
                unreachable!()
            };
            f.text = "0.5+0.5*Sin(SongTime*Pi*b)".into();
            f.inputs = [0.0, 0.25, 0.0];
        });
        ramp(
            &mut p,
            AutoTarget::ModInput {
                modulator: id,
                input: 1,
            },
            4,
        );
        assert!(!needs_trace(&p, id));
        check_against(&p, id, &target, 4, |beat| {
            let b = beat / 4.0;
            let v = 0.5 + 0.5 * cantus_dsp::math::sin(beat * std::f64::consts::PI * b);
            0.5 + 0.5 * v as f32
        });
        // Bipolar, the same knob: twice the value less one.
        edit_source(&mut p, id, |s| {
            if let ModSource::Formula(f) = s {
                f.bipolar = true;
            }
        });
        check_against(&p, id, &target, 4, |beat| {
            let b = beat / 4.0;
            let v = 0.5 + 0.5 * cantus_dsp::math::sin(beat * std::f64::consts::PI * b);
            0.5 + 0.5 * (2.0 * v as f32 - 1.0)
        });
    }

    #[test]
    fn a_pad_with_inertia_or_a_driven_controller_is_traced() {
        let (mut p, id, _) = with_route(ModKind::XyPad, 0.8);
        assert!(!needs_trace(&p, id));
        edit_source(&mut p, id, |s| {
            if let ModSource::XyPad(x) = s {
                x.inertia_ms = 200.0;
            }
        });
        assert!(needs_trace(&p, id));
        // An LFO whose amount the gliding pad moves.
        let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
        lfo.routes.push(ModRoute {
            target: AutoTarget::StripWidth(p.tracks[0].strip.unwrap()),
            depth: 0.3,
        });
        let lfo_id = lfo.id;
        p.tracks[0].modulators.push(lfo);
        assert!(!needs_trace(&p, lfo_id));
        let (host, i, _) = p.modulator(id).unwrap();
        p.modulators_mut(host).unwrap()[i].routes.push(ModRoute {
            target: AutoTarget::ModAmount(lfo_id),
            depth: -0.5,
        });
        assert!(needs_trace(&p, lfo_id));
        // A formula is a function of time, unless a modulator moves a knob.
        let (mut p, f, _) = with_route(ModKind::Formula, 0.5);
        assert!(!needs_trace(&p, f));
        let mut lfo = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo);
        lfo.routes.push(ModRoute {
            target: AutoTarget::ModInput {
                modulator: f,
                input: 2,
            },
            depth: 0.3,
        });
        p.tracks[0].modulators.push(lfo);
        assert!(needs_trace(&p, f));
    }
}
