//! The X-Y pad and the formula controller in the model (fxd, ADRs 0147, 0148): their
//! commands and inverses, what is refused, the puck's inertia, paths, route ranges and
//! what the snapshot hands the engine.

use super::eval::{controller_outputs, ModTime};
use super::xy::Puck;
use super::*;
use crate::{EffectKind, Snapshot};

fn project() -> (Project, crate::EffectId) {
    let mut p = Project::new("fxd");
    let t = p.make_track("Lead");
    let sid = t.strip.unwrap();
    p.tracks.push(t);
    let fx = p.make_effect(EffectKind::Saturator);
    let fid = fx.id;
    p.strip_mut(sid).unwrap().effects.push(fx);
    (p, fid)
}

fn drive(fid: crate::EffectId) -> AutoTarget {
    AutoTarget::Effect {
        effect: fid,
        key: "drive".into(),
    }
}

fn add(p: &mut Project, kind: ModKind, routes: Vec<ModRoute>) -> ModulatorId {
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), kind);
    m.routes = routes;
    let id = m.id;
    Command::InsertModulator {
        host: ModHost::Master,
        index: 0,
        modulator: Box::new(m),
    }
    .apply(p)
    .unwrap();
    id
}

fn src(p: &Project, id: ModulatorId) -> ModSource {
    p.modulator(id).unwrap().2.source.clone()
}

#[test]
fn an_input_is_set_alone_and_undone_exactly() {
    let (mut p, fid) = project();
    let id = add(
        &mut p,
        ModKind::XyPad,
        vec![ModRoute {
            target: drive(fid),
            depth: 1.0,
        }],
    );
    let before = p.clone();
    let undo = Command::SetModulatorInput {
        id,
        input: 1,
        value: 0.8,
    }
    .apply(&mut p)
    .unwrap();
    assert_eq!(src(&p, id).input(1), Some(0.8));
    assert_eq!(src(&p, id).input(0), Some(0.5));
    undo.apply(&mut p).unwrap();
    assert_eq!(p, before);
    // Past its inputs, out of range, not a number, a modulator that is not there.
    for (input, value, who) in [(2, 0.5, id), (0, 1.5, id), (0, f32::NAN, id), (0, 0.5, ModulatorId(9_999))] {
        assert!(Command::SetModulatorInput { id: who, input, value }.apply(&mut p).is_err());
    }
    assert_eq!(p, before);
    // A drag's frames merge per input.
    let a = Command::SetModulatorInput { id, input: 0, value: 0.1 };
    let b = Command::SetModulatorInput { id, input: 0, value: 0.2 };
    let c = Command::SetModulatorInput { id, input: 1, value: 0.2 };
    assert!(b.supersedes(&a));
    assert!(!c.supersedes(&a));
    assert_eq!(a.label_key(), "cmd.fxd_set_input");
}

#[test]
fn inputs_are_targets_a_controller_and_recording_reach() {
    let (mut p, fid) = project();
    let id = add(
        &mut p,
        ModKind::Formula,
        vec![ModRoute {
            target: drive(fid),
            depth: 0.5,
        }],
    );
    let target = AutoTarget::ModInput { modulator: id, input: 2 };
    assert_eq!(crate::automation::range::range_of(&p, &target), Some(crate::automation::range::AMOUNT));
    assert_eq!(crate::automation::range::base_value(&p, &target), Some(0.5));
    let cmd = target.command(&p, 0.25).unwrap();
    assert_eq!(AutoTarget::value_of_command(&cmd), Some((target.clone(), 0.25)));
    cmd.apply(&mut p).unwrap();
    assert_eq!(src(&p, id).input(2), Some(0.25));
    // An input the kind does not have names nothing.
    let none = AutoTarget::ModInput { modulator: id, input: 3 };
    assert_eq!(crate::automation::range::range_of(&p, &none), None);
    assert!(none.command(&p, 0.5).is_none());
}

#[test]
fn a_formula_that_does_not_compile_is_refused() {
    let (mut p, _) = project();
    let id = add(&mut p, ModKind::Formula, vec![]);
    let before = p.clone();
    let mut m = p.modulator(id).unwrap().2.clone();
    let ModSource::Formula(f) = &mut m.source else { unreachable!() };
    f.text = "a*Sin(".into();
    assert!(Command::SetModulator { modulator: Box::new(m.clone()) }.apply(&mut p).is_err());
    let ModSource::Formula(f) = &mut m.source else { unreachable!() };
    f.text = "a*Sin(SongTime*Pi*b)+c".into();
    let undo = Command::SetModulator { modulator: Box::new(m) }.apply(&mut p).unwrap();
    undo.apply(&mut p).unwrap();
    assert_eq!(p, before);
}

#[test]
fn links_are_validated() {
    let (mut p, fid) = project();
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), ModKind::XyPad);
    let link = |output, range| RouteLink { target: drive(fid), output, range };
    let set = |m: &mut Modulator, links| {
        let ModSource::XyPad(x) = &mut m.source else { unreachable!() };
        x.links = links;
    };
    set(&mut m, vec![link(1, Some(RouteRange { from: 0.0, to: 1.0, curve: 0.3 }))]);
    assert!(m.is_valid());
    set(&mut m, vec![link(2, None)]);
    assert!(!m.is_valid(), "a pad has two axes");
    set(&mut m, vec![link(0, Some(RouteRange { from: 0.0, to: 2.0, curve: 0.0 }))]);
    assert!(!m.is_valid());
    set(&mut m, vec![link(0, None), link(1, None)]);
    assert!(!m.is_valid(), "one link per target");
    // A route into the pad's own input is refused, like its own amount.
    set(&mut m, vec![]);
    m.routes.push(ModRoute {
        target: AutoTarget::ModInput { modulator: m.id, input: 0 },
        depth: 0.5,
    });
    assert!(!m.is_valid());
}

#[test]
fn route_ranges_are_the_output_where_unbent_and_full() {
    let full_bi = RouteRange { from: -1.0, to: 1.0, curve: 0.0 };
    let full_uni = RouteRange { from: 0.0, to: 1.0, curve: 0.0 };
    for v in [-1.0f32, -0.5, 0.0, 0.25, 1.0] {
        assert!((full_bi.apply(v, true) - v).abs() < 1e-6);
    }
    for v in [0.0f32, 0.3, 1.0] {
        assert_eq!(full_uni.apply(v, false), v);
    }
    // A symmetric range at the centre adds exactly nothing.
    for r in [0.1f32, 0.37, 1.0] {
        let g = RouteRange { from: -r, to: r, curve: 0.0 };
        assert_eq!(g.apply(0.0, true), 0.0);
    }
    // FL's way: from 20 % to 80 % of the travel, bent.
    let g = RouteRange { from: 0.2, to: 0.8, curve: 0.5 };
    assert_eq!(g.apply(0.0, false), 0.2);
    assert!((g.apply(1.0, false) - 0.8).abs() < 1e-6);
    assert!(g.apply(0.5, false) != 0.5);
    assert_eq!(route_value(1, None, [0.1, 0.7], true), 0.7);
}

#[test]
fn the_puck_follows_critically_damped_and_rests_to_the_bit() {
    let mut puck = Puck::default();
    puck.step([0.3, 0.6], 200.0, 0.01);
    assert_eq!(puck.pos, [0.3, 0.6], "placed on its first target");
    // Resting on the target: the same bits for ever.
    for _ in 0..1000 {
        puck.step([0.3, 0.6], 200.0, 1.0 / 3000.0);
    }
    assert_eq!(puck.pos, [0.3, 0.6]);
    // A jump: no overshoot, monotone, a closed form that splits exactly.
    let (mut whole, mut parts) = (puck, puck);
    whole.step([0.9, 0.1], 200.0, 0.2);
    for _ in 0..10 {
        parts.step([0.9, 0.1], 200.0, 0.02);
    }
    for d in 0..2 {
        assert!((whole.pos[d] - parts.pos[d]).abs() < 1e-6, "{whole:?} {parts:?}");
    }
    let mut last = puck.pos[0];
    let mut p = puck;
    for _ in 0..400 {
        p.step([0.9, 0.1], 200.0, 0.005);
        assert!(p.pos[0] >= last && p.pos[0] <= 0.9);
        last = p.pos[0];
    }
    assert!((p.pos[0] - 0.9).abs() < 1e-3);
    // No inertia: the puck is the target.
    let mut q = Puck::default();
    q.step([0.2, 0.2], 0.0, 0.01);
    q.step([0.7, 0.4], 0.0, 0.01);
    assert_eq!(q.pos, [0.7, 0.4]);
}

#[test]
fn a_path_loops_on_the_song_position() {
    let path = XyPath {
        points: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        beats: 4.0,
        smooth: false,
    };
    assert!(path.is_valid());
    assert_eq!(path.at(0.0), [0.0, 0.0]);
    assert_eq!(path.at(1.5), [1.0, 0.5]);
    assert_eq!(path.at(3.5), [0.0, 0.5], "the last point joins the first");
    assert_eq!(path.at(5.5), path.at(1.5));
    assert_eq!(path.at(-2.5), path.at(1.5));
    let smooth = XyPath { smooth: true, ..path.clone() };
    for k in 0..64 {
        let p = smooth.at(f64::from(k) / 16.0);
        assert!(p.iter().all(|v| (0.0..=1.0).contains(v)));
    }
    assert_eq!(smooth.at(1.0), [1.0, 0.0], "through its points");
    // A recorded gesture, resampled evenly.
    let samples: Vec<(f64, f32, f32)> =
        (0..100).map(|i| (f64::from(i) * 0.04, i as f32 / 99.0, 0.5)).collect();
    let g = XyPath::from_gesture(&samples, 0.0, 4.0, 16).unwrap();
    assert_eq!(g.points.len(), 16);
    assert!(g.is_valid());
    assert!(g.points.windows(2).all(|w| w[1][0] >= w[0][0]));
    // The pad's target: the path only while the song plays and the path is on.
    let mut x = XySource { path: Some(path), ..XySource::default() };
    assert_eq!(x.target(1.5, true, [0.2, 0.3]), [0.2, 0.3]);
    x.play_path = true;
    assert_eq!(x.target(1.5, true, [0.2, 0.3]), [1.0, 0.5]);
    assert_eq!(x.target(1.5, false, [0.2, 0.3]), [0.2, 0.3]);
}

#[test]
fn the_pad_at_rest_and_the_formula_at_zero_output_nothing() {
    let t = ModTime::at_beats(3.25, 0.0, 120.0);
    let pad = ModKind::XyPad.source();
    assert_eq!(controller_outputs(&pad, None, &t, [0.5, 0.5, 0.0]), [0.0, 0.0]);
    let f = ModSource::Formula(FormulaSource {
        text: "a*Sin(SongTime*Pi*b)+c".into(),
        inputs: [0.0; 3],
        bipolar: false,
        links: vec![],
        look: 0,
    });
    let prog = formula::Program::compile("a*Sin(SongTime*Pi*b)+c").unwrap();
    assert_eq!(controller_outputs(&f, Some(&prog), &t, [0.0; 3]), [0.0, 0.0]);
    assert_eq!(controller_outputs(&f, None, &t, [0.5; 3]), [0.0, 0.0], "never compiled here");
    // The default formula is FL's first example.
    let d = ModKind::Formula.source();
    let ModSource::Formula(df) = &d else { unreachable!() };
    assert_eq!(df.text, "0.5*Sin(SongTime*Pi)+0.5");
    assert!((super::eval::output(&d, &ModTime::at_beats(0.5, 0.0, 120.0), None, None) - 1.0).abs() < 1e-6);
}

#[test]
fn the_snapshot_carries_links_inputs_and_the_program() {
    let (mut p, fid) = project();
    let gain = AutoTarget::MasterGain;
    let id = add(
        &mut p,
        ModKind::XyPad,
        vec![
            ModRoute { target: drive(fid), depth: 1.0 },
            ModRoute { target: gain.clone(), depth: 0.5 },
        ],
    );
    let mut m = p.modulator(id).unwrap().2.clone();
    let ModSource::XyPad(x) = &mut m.source else { unreachable!() };
    x.x = 0.25;
    x.links = vec![RouteLink {
        target: gain.clone(),
        output: 1,
        range: Some(RouteRange { from: 0.0, to: 0.5, curve: 0.0 }),
    }];
    Command::SetModulator { modulator: Box::new(m) }.apply(&mut p).unwrap();
    let s = Snapshot::compile(&p, 1);
    let plan = &s.modulators[0];
    assert_eq!(plan.inputs, [0.25, 0.5, 0.0]);
    assert_eq!(plan.input_lanes, [None, None, None]);
    assert!(plan.program.is_none());
    let at = |t: &AutoTarget| s.automation.iter().position(|l| &l.target == t).unwrap();
    for (k, &(li, _)) in plan.routes.iter().enumerate() {
        let want = if li == at(&gain) {
            (1, Some(RouteRange { from: 0.0, to: 0.5, curve: 0.0 }))
        } else {
            (0, None)
        };
        assert_eq!(plan.links[k], want);
    }
    // A formula's program is compiled into the plan.
    let fid2 = add(&mut p, ModKind::Formula, vec![ModRoute { target: drive(fid), depth: -0.3 }]);
    let s = Snapshot::compile(&p, 2);
    let f = s.modulators.iter().find(|m| m.id == fid2).unwrap();
    assert!(f.program.is_some());
    assert_eq!(f.inputs, [0.5, 0.5, 0.5]);
    // Unrouted, a controller compiles to nothing.
    let mut q = Project::new("none");
    add(&mut q, ModKind::XyPad, vec![]);
    add(&mut q, ModKind::Formula, vec![]);
    assert!(Snapshot::compile(&q, 1).modulators.is_empty());
}

#[test]
fn controllers_survive_a_save() {
    let (mut p, fid) = project();
    let id = add(&mut p, ModKind::XyPad, vec![ModRoute { target: drive(fid), depth: 1.0 }]);
    let mut m = p.modulator(id).unwrap().2.clone();
    let ModSource::XyPad(x) = &mut m.source else { unreachable!() };
    x.inertia_ms = 350.0;
    x.play_path = true;
    x.path = Some(XyPath { points: vec![[0.1, 0.2], [0.9, 0.8]], beats: 2.0, smooth: true });
    let json = serde_json::to_string(&m).unwrap();
    let back: Modulator = serde_json::from_str(&json).unwrap();
    assert_eq!(back, m);
    // An old route, an old pad: nothing new is written for what is at its default.
    let plain = serde_json::to_string(&Modulator::new(ModulatorId(5), ModKind::XyPad)).unwrap();
    assert!(!plain.contains("links") && !plain.contains("inertia") && !plain.contains("path"));
}
