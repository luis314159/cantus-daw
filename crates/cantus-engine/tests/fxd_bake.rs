//! The X-Y pad and the formula controller, step 3 (fxd, ADRs 0144, 0145, 0057 §5):
//! *Bake to automation* of a controller's route renders what the route played — a pad
//! without inertia on its Y through a bent range with Y automated, a formula whose knob b
//! a clip automates through its own range (both computed ahead), a pad with inertia on a
//! path and a formula whose knob an LFO drives (both from the engine's `trace_route`) —
//! within the curve's thinning; and the telemetry tells the face where a gliding puck is
//! and where it heads.

use cantus_core::automation::AutoPoint;
use cantus_core::modulator::bake::{bake, needs_trace};
use cantus_core::modulator::*;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

const SR: f64 = 48_000.0;

/// A Firmus sine dyad on every beat for 16 beats, on the lead's strip.
fn base() -> Project {
    let mut p = Project::new("fxd bake");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let f = lead.instrument.firmus_mut().unwrap();
    f.osc1_wave = Waveform::Sine;
    f.amp_attack_s = 0.0;
    f.amp_release_s = 0.02;
    let mut clip = p.make_clip("beats", Ticks::ZERO, Ticks::beats(16));
    for b in 0..16 {
        for k in [69u8, 76] {
            let n = p.make_note(Ticks::beats(b), Ticks::QUARTER - Ticks(PPQ / 4), k, 100);
            clip.notes.push(n);
        }
    }
    lead.clips.push(clip);
    p.tracks.push(lead);
    p
}

fn pan(p: &Project) -> AutoTarget {
    AutoTarget::StripPan(p.tracks[0].strip.unwrap())
}

fn add(p: &mut Project, kind: ModKind, target: AutoTarget, depth: f32) -> ModulatorId {
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), kind);
    m.routes.push(ModRoute { target, depth });
    let id = m.id;
    p.master.modulators.push(m);
    id
}

fn edit(p: &mut Project, id: ModulatorId, f: impl FnOnce(&mut ModSource)) {
    let (host, i, _) = p.modulator(id).unwrap();
    f(&mut p.modulators_mut(host).unwrap()[i].source);
    assert!(p.modulator(id).unwrap().2.is_valid());
}

/// An automation clip on input `input` of `id` through `(beat, value)` points.
fn automate(p: &mut Project, id: ModulatorId, input: u8, pts: &[(i64, f32)]) {
    let pts = pts
        .iter()
        .map(|&(b, v)| AutoPoint::new(Ticks::beats(b), v))
        .collect();
    let c = p.make_automation_clip(
        "input",
        AutoTarget::ModInput {
            modulator: id,
            input,
        },
        Ticks::ZERO,
        Ticks::beats(16),
        pts,
    );
    p.tracks[0].clips.push(c);
}

fn snap(p: &Project) -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(p, 1))
}

fn opts() -> RenderOptions {
    RenderOptions {
        tail_seconds: 0.0,
        end: Some(Ticks::beats(16)),
        block_frames: 512,
        ..Default::default()
    }
}

/// Bake `id`'s route into the pan over the 16 beats (traced when the bake needs it), take
/// the route away, play the clip: the render is the modulated one within the thinning
/// (pan within 0.002 of its travel moves a channel's gain well under 1 %), and the
/// route moved the sound (it is not a null compared with a null).
fn bakes_what_it_played(p: &Project, id: ModulatorId, traced: bool) {
    let target = pan(p);
    assert_eq!(needs_trace(p, id), traced);
    let (from, to) = (Ticks::ZERO, Ticks::beats(16));
    let s = snap(p);
    let modulated = render(s.clone(), &opts()).unwrap().samples;
    // What the engine played on that route, at every control point.
    let played =
        cantus_engine::modulator::trace_route(s, &opts(), Vec::new(), Vec::new(), id, &target);
    let trace = traced.then_some(played.as_slice());
    let pts = bake(p, id, &target, from, to, from, None, trace).unwrap();
    // The curve is the route as the engine played it within the thinning — computed ahead
    // (inputs from their lanes, the route's axis and range) or from the trace — wherever a
    // control point falls on one of the bake's samples (every 80 ticks: 2 000 samples, a
    // multiple of the 16-sample grid and of the 1/96 beat).
    let depth = p.modulator(id).unwrap().2.route(&target).unwrap().depth;
    let curve = Automation::new(target.clone(), pts.clone());
    let on_both = |tick: f64| (tick - tick.round()).abs() < 1e-6 && tick.round() as i64 % 80 == 0;
    let mut seen = 0;
    let off = played
        .iter()
        .filter(|(tick, _)| on_both(*tick))
        .map(|&(tick, v)| {
            seen += 1;
            let want = (0.5 + depth * v).clamp(0.0, 1.0);
            (curve.value_at(tick.round()).unwrap() - want).abs()
        })
        .fold(0f32, f32::max);
    assert!(seen > 150, "{seen} points compared");
    assert!(
        off <= cantus_core::modulator::bake::TOLERANCE * 1.01,
        "the curve is {off} off what played"
    );
    let mut baked = p.clone();
    let (host, i, _) = baked.modulator(id).unwrap();
    let m = &mut baked.modulators_mut(host).unwrap()[i];
    m.routes.clear();
    m.tidy_links();
    let c = baked.make_automation_clip("baked", target.clone(), from, to - from, pts);
    baked.tracks[0].clips.push(c);
    let out = render(snap(&baked), &opts()).unwrap().samples;
    let peak = modulated.iter().fold(0f32, |m, x| m.max(x.abs()));
    let worst = modulated
        .iter()
        .zip(&out)
        .fold(0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(worst < 0.01 * peak, "{worst} of {peak}");
    let mut plain = p.clone();
    plain.master.modulators.clear();
    let still = render(snap(&plain), &opts()).unwrap().samples;
    let moved = modulated
        .iter()
        .zip(&still)
        .fold(0f32, |m, (a, b)| m.max((a - b).abs()));
    assert!(
        moved > 0.2 * peak,
        "the route moved the pan: {moved} of {peak}"
    );
}

#[test]
fn a_pad_on_its_y_through_a_bent_range_bakes_what_it_played() {
    let mut p = base();
    let target = pan(&p);
    let pad = add(&mut p, ModKind::XyPad, target.clone(), 0.9);
    edit(&mut p, pad, |src| {
        let ModSource::XyPad(x) = src else {
            unreachable!()
        };
        x.x = 0.9;
        x.links.push(RouteLink {
            target,
            output: 1,
            range: Some(RouteRange {
                from: -0.7,
                to: 0.5,
                curve: 0.4,
            }),
        });
    });
    // Y swept up and down by automation; X (unread) still.
    automate(&mut p, pad, 1, &[(0, 0.0), (5, 1.0), (9, 0.2), (16, 0.8)]);
    bakes_what_it_played(&p, pad, false);
}

#[test]
fn a_formula_with_an_automated_knob_bakes_what_it_played() {
    let mut p = base();
    let target = pan(&p);
    let f = add(&mut p, ModKind::Formula, target.clone(), 0.8);
    edit(&mut p, f, |src| {
        let ModSource::Formula(f) = src else {
            unreachable!()
        };
        f.text = "0.5+0.5*Sin(SongTime*Pi*(0.25+b))".into();
        f.inputs = [0.5, 0.0, 0.5];
        f.bipolar = true;
        f.links.push(RouteLink {
            target,
            output: 0,
            range: Some(RouteRange {
                from: -0.5,
                to: 0.5,
                curve: -0.3,
            }),
        });
    });
    automate(&mut p, f, 1, &[(0, 0.0), (16, 1.0)]);
    bakes_what_it_played(&p, f, false);
}

#[test]
fn a_pad_with_inertia_bakes_from_its_trace_what_it_played() {
    let mut p = base();
    let target = pan(&p);
    let pad = add(&mut p, ModKind::XyPad, target.clone(), 0.8);
    edit(&mut p, pad, |src| {
        let ModSource::XyPad(x) = src else {
            unreachable!()
        };
        x.inertia_ms = 150.0;
        x.path = Some(XyPath {
            points: vec![[0.1, 0.9], [0.9, 0.2], [0.6, 0.7], [0.2, 0.1]],
            beats: 4.0,
            smooth: false,
        });
        x.play_path = true;
        x.links.push(RouteLink {
            target,
            output: 1,
            range: Some(RouteRange {
                from: -0.8,
                to: 0.6,
                curve: 0.3,
            }),
        });
    });
    bakes_what_it_played(&p, pad, true);
}

#[test]
fn a_formula_whose_knob_an_lfo_drives_bakes_from_its_trace() {
    let mut p = base();
    let target = pan(&p);
    let f = add(&mut p, ModKind::Formula, target, 0.7);
    edit(&mut p, f, |src| {
        let ModSource::Formula(f) = src else {
            unreachable!()
        };
        f.text = "a".into();
        f.inputs = [0.5, 0.0, 0.0];
        f.bipolar = true;
    });
    add(
        &mut p,
        ModKind::Lfo,
        AutoTarget::ModInput {
            modulator: f,
            input: 0,
        },
        0.5,
    );
    bakes_what_it_played(&p, f, true);
}

/// The face's telemetry: a pad with 400 ms of inertia whose X jumps from 0 to 1 at 1 s
/// reports its puck on its way, between the two, and where it heads at the new X; later
/// nearly there. A formula is no pad.
#[test]
fn the_telemetry_tells_where_a_gliding_puck_is_and_where_it_heads() {
    let mut p = base();
    let target = pan(&p);
    let pad = add(&mut p, ModKind::XyPad, target.clone(), 0.5);
    edit(&mut p, pad, |src| {
        let ModSource::XyPad(x) = src else {
            unreachable!()
        };
        x.inertia_ms = 400.0;
        x.y = 0.25;
    });
    automate(&mut p, pad, 0, &[(0, 0.0), (2, 0.0), (2, 1.0), (16, 1.0)]);
    let width = AutoTarget::StripWidth(p.tracks[0].strip.unwrap());
    let formula = add(&mut p, ModKind::Formula, width, 0.5);
    let s = snap(&p);
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut proc = Processor::without_midi(SR, rx_engine, tx_engine);
    let mut reports = proc.take_reports().expect("the reading end");
    tx.push(EngineMessage::Automation(
        cantus_engine::automation::Runtime::build(&s),
    ))
    .ok();
    tx.push(EngineMessage::SetSnapshot(s)).ok();
    tx.push(EngineMessage::Play).ok();
    let mut done = 0usize;
    let mut last = None;
    let mut play_to = |secs: f64, last: &mut Option<_>| {
        while (done as f64) < secs * SR {
            let mut buf = vec![0.0f32; 256 * 2];
            proc.process(&mut buf, 2);
            done += 256;
            while rx.pop().is_ok() {}
            reports.drain(|r| {
                if let Some(m) = r.modulation {
                    *last = Some(m);
                }
            });
        }
    };
    play_to(0.5, &mut last);
    let t = last.expect("reported");
    let (puck, heads) = t.pad(pad).expect("a pad");
    assert_eq!(puck, [0.0, 0.25], "at rest where it was placed");
    assert_eq!(heads, [0.0, 0.25]);
    assert!(t.pad(formula).is_none(), "a formula is no pad");
    assert!(t.value(formula).is_some());
    play_to(1.1, &mut last);
    let (puck, heads) = last.unwrap().pad(pad).unwrap();
    assert!(puck[0] > 0.0 && puck[0] < 0.5, "on its way: {puck:?}");
    assert_eq!(puck[1], 0.25);
    assert_eq!(heads, [1.0, 0.25], "heads for the new X");
    play_to(3.2, &mut last);
    let (puck, heads) = last.unwrap().pad(pad).unwrap();
    assert!(puck[0] > 0.9 && puck[0] <= 1.0, "nearly there: {puck:?}");
    assert_eq!(heads, [1.0, 0.25]);
}
