//! The modulators' model, commands, evaluation and compilation (ADR 0057).

use super::eval::{output, Follower, Gate, Gates, LiveEnv, ModTime};
use super::*;
use crate::snapshot::NoteEvent;
use crate::{EffectKind, NoteId, Snapshot, Ticks};

fn project_with_strip_fx() -> (Project, TrackId, StripId, crate::EffectId) {
    let mut p = Project::new("m");
    let t = p.make_track("Lead");
    let (tid, sid) = (t.id, t.strip.unwrap());
    p.tracks.push(t);
    let fx = p.make_effect(EffectKind::Saturator);
    let fid = fx.id;
    p.strip_mut(sid).unwrap().effects.push(fx);
    (p, tid, sid, fid)
}

fn lfo(p: &mut Project) -> Modulator {
    Modulator::new(ModulatorId(p.alloc_id()), ModKind::Lfo)
}

fn drive(fid: crate::EffectId) -> AutoTarget {
    AutoTarget::Effect {
        effect: fid,
        key: "drive".into(),
    }
}

fn t(beats: f64) -> ModTime<'static> {
    ModTime::at_beats(beats, 0.0, 120.0)
}

#[test]
fn validity_refuses_what_a_command_must_not_store() {
    let mut p = Project::new("v");
    let mut m = lfo(&mut p);
    assert!(m.is_valid());
    m.routes.push(ModRoute {
        target: AutoTarget::MasterGain,
        depth: 0.5,
    });
    assert!(m.is_valid());
    let mut twice = m.clone();
    twice.routes.push(twice.routes[0].clone());
    assert!(!twice.is_valid(), "one route per target");
    let mut own = m.clone();
    own.routes.push(ModRoute {
        target: AutoTarget::ModAmount(own.id),
        depth: 0.5,
    });
    assert!(!own.is_valid(), "no route into its own amount");
    let mut deep = m.clone();
    deep.routes[0].depth = 1.5;
    assert!(!deep.is_valid());
    deep.routes[0].depth = f32::NAN;
    assert!(!deep.is_valid());
    let mut amount = m.clone();
    amount.amount = f32::INFINITY;
    assert!(!amount.is_valid());
    let mut steps = Modulator::new(m.id, ModKind::Step);
    if let ModSource::Step(s) = &mut steps.source {
        s.steps.clear();
    }
    assert!(!steps.is_valid(), "a step sequence needs a step");
    let mut rate = m.clone();
    rate.source = ModSource::Lfo(LfoSource {
        rate: ModRate::Hz { hz: 0.0 },
        ..match ModKind::Lfo.source() {
            ModSource::Lfo(l) => l,
            _ => unreachable!(),
        }
    });
    assert!(!rate.is_valid());
    for k in ModKind::ALL {
        assert!(Modulator::new(m.id, k).is_valid(), "{k:?} defaults");
    }
}

#[test]
fn insert_remove_and_set_have_exact_inverses() {
    let (mut p, tid, sid, fid) = project_with_strip_fx();
    let before = p.clone();
    let mut m = lfo(&mut p);
    let after_alloc = p.clone();
    m.routes.push(ModRoute {
        target: drive(fid),
        depth: 0.3,
    });
    let undo_insert = Command::InsertModulator {
        host: ModHost::Track(tid),
        index: 0,
        modulator: Box::new(m.clone()),
    }
    .apply(&mut p)
    .unwrap();
    assert_eq!(p.track(tid).unwrap().modulators, vec![m.clone()]);
    assert!(p.is_modulated(&drive(fid)));
    // A second one, on the strip, edited.
    let mut env = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Envelope);
    env.routes.push(ModRoute {
        target: AutoTarget::StripGain(sid),
        depth: -0.5,
    });
    Command::InsertModulator {
        host: ModHost::Strip(sid),
        index: 0,
        modulator: Box::new(env.clone()),
    }
    .apply(&mut p)
    .unwrap();
    let mid = p.clone();
    let mut edited = m.clone();
    edited.routes[0].depth = -0.8;
    edited.name = "Wobble".into();
    let undo_set = Command::SetModulator {
        modulator: Box::new(edited.clone()),
    }
    .apply(&mut p)
    .unwrap();
    assert_eq!(p.modulator(m.id).unwrap().2, &edited);
    undo_set.apply(&mut p).unwrap();
    assert_eq!(p, mid);
    let undo_remove = Command::RemoveModulator { id: env.id }
        .apply(&mut p)
        .unwrap();
    assert!(p.modulator(env.id).is_none());
    undo_remove.apply(&mut p).unwrap();
    assert_eq!(p, mid);
    Command::RemoveModulator { id: env.id }
        .apply(&mut p)
        .unwrap();
    undo_insert.apply(&mut p).unwrap();
    // Only the ids allocated remain different.
    let mut back = p.clone();
    back.next_id = after_alloc.next_id;
    assert_eq!(back, after_alloc);
    assert_eq!(before.tracks, p.tracks);
}

#[test]
fn commands_refuse_duplicates_bad_values_and_missing_hosts() {
    let (mut p, tid, _, _) = project_with_strip_fx();
    let m = lfo(&mut p);
    let ins = |m: &Modulator, host| Command::InsertModulator {
        host,
        index: 0,
        modulator: Box::new(m.clone()),
    };
    ins(&m, ModHost::Track(tid)).apply(&mut p).unwrap();
    assert!(matches!(
        ins(&m, ModHost::Master).apply(&mut p),
        Err(CommandError::DuplicateId(_))
    ));
    let other = lfo(&mut p);
    assert!(ins(&other, ModHost::Track(TrackId(999_999)))
        .apply(&mut p)
        .is_err());
    let mut bad = other.clone();
    bad.amount = 2.0;
    assert!(ins(&bad, ModHost::Master).apply(&mut p).is_err());
    assert!(Command::SetModulator {
        modulator: Box::new(other.clone())
    }
    .apply(&mut p)
    .is_err());
    // A host holds at most MAX_PER_HOST.
    for _ in 0..MAX_PER_HOST {
        let m = lfo(&mut p);
        let _ = ins(&m, ModHost::Master).apply(&mut p);
    }
    assert_eq!(p.master.modulators.len(), MAX_PER_HOST);
    let one_more = lfo(&mut p);
    assert!(ins(&one_more, ModHost::Master).apply(&mut p).is_err());
    // A track arriving with a modulator whose id is taken is refused.
    let mut t = p.make_track("dup");
    t.modulators.push(m.clone());
    assert!(matches!(
        Command::InsertTrack {
            index: 0,
            track: Box::new(t)
        }
        .apply(&mut p),
        Err(CommandError::DuplicateId(_))
    ));
}

#[test]
fn a_synced_lfo_is_a_function_of_the_song_position() {
    let src = ModKind::Lfo.source(); // a sine, one cycle a quarter note
    let at = |b: f64| output(&src, &t(b), None, None);
    assert!(at(0.0).abs() < 1e-6);
    assert!((at(0.25) - 1.0).abs() < 1e-5, "{}", at(0.25));
    assert!((at(0.75) + 1.0).abs() < 1e-5);
    for b in [0.1, 1.3, 7.77] {
        assert!((at(b) - at(b + 1.0)).abs() < 1e-5, "periodic at {b}");
        // Where playback started does not move it.
        let late = ModTime::at_beats(b, 0.6, 120.0);
        assert_eq!(at(b), output(&src, &late, None, None));
    }
}

#[test]
fn a_free_lfo_runs_from_the_start_of_playback() {
    let ModSource::Lfo(mut l) = ModKind::Lfo.source() else {
        unreachable!()
    };
    l.rate = ModRate::Hz { hz: 2.0 };
    l.unipolar = true;
    let src = ModSource::Lfo(l);
    // 2 Hz: a quarter cycle is 0.125 s, a beat at 120 BPM is 0.5 s.
    let from = |play: f64, b: f64| output(&src, &ModTime::at_beats(b, play, 120.0), None, None);
    assert!((from(0.0, 0.0) - 0.5).abs() < 1e-6, "unipolar centre");
    assert!((from(0.0, 0.25) - 1.0).abs() < 1e-5);
    assert!((from(4.0, 4.25) - 1.0).abs() < 1e-5, "restarted at play");
    assert!((from(4.0, 4.0) - from(0.0, 0.0)).abs() < 1e-6);
}

#[test]
fn a_note_retriggered_lfo_starts_its_cycle_at_each_note() {
    let ModSource::Lfo(mut l) = ModKind::Lfo.source() else {
        unreachable!()
    };
    l.retrigger = Retrigger::Note;
    let src = ModSource::Lfo(l);
    let gates = Gates {
        gates: vec![
            Gate { on: 1.1, off: 1.5 },
            Gate {
                on: 3.37,
                off: f64::INFINITY,
            },
        ],
        levels: None,
    };
    let at = |b: f64| output(&src, &t(b), Some(&gates), None);
    assert!(at(1.1).abs() < 1e-6 && at(3.37).abs() < 1e-6);
    assert!((at(1.35) - 1.0).abs() < 1e-5);
    assert!((at(3.62) - 1.0).abs() < 1e-5);
    // A live note later than the sequenced one wins.
    assert!(output(&src, &t(3.9), Some(&gates), Some(3.9)).abs() < 1e-6);
}

#[test]
fn steps_hold_each_value_and_glide_into_the_next() {
    let ModSource::Step(mut s) = ModKind::Step.source() else {
        unreachable!()
    };
    s.steps = vec![1.0, -1.0, 0.5];
    s.division = DIV_SIXTEENTH; // a quarter beat each
    let hold = ModSource::Step(s.clone());
    let at = |src: &ModSource, b: f64| output(src, &t(b), None, None);
    assert_eq!(at(&hold, 0.1), 1.0);
    assert_eq!(at(&hold, 0.3), -1.0);
    assert_eq!(at(&hold, 0.6), 0.5);
    assert_eq!(at(&hold, 0.8), 1.0, "it cycles");
    s.glide = 0.5;
    let glide = ModSource::Step(s);
    assert_eq!(at(&glide, 0.1), 1.0, "the first half holds");
    let mid = at(&glide, 0.1875); // three quarters of the step: half the glide
    assert!((mid - 0.0).abs() < 1e-5, "{mid}");
    assert!(at(&glide, 0.2499) < -0.99);
}

#[test]
fn random_values_are_seeded_held_or_smooth() {
    let ModSource::Random(mut r) = ModKind::Random.source() else {
        unreachable!()
    };
    let held = ModSource::Random(r.clone());
    let at = |src: &ModSource, b: f64| output(src, &t(b), None, None);
    // An eighth each: constant inside one, a new value in the next, the same every time.
    assert_eq!(at(&held, 0.05), at(&held, 0.45));
    assert_ne!(at(&held, 0.45), at(&held, 0.55));
    assert_eq!(at(&held, 3.3), at(&held, 3.3));
    r.seed = 2;
    let other = ModSource::Random(r.clone());
    assert_ne!(at(&held, 0.3), at(&other, 0.3));
    r.smooth = true;
    let smooth = ModSource::Random(r);
    // Continuous: no step bigger than the slope allows across a cycle boundary.
    let mut prev = at(&smooth, 0.0);
    for i in 1..400 {
        let v = at(&smooth, i as f64 * 0.005);
        assert!((v - prev).abs() < 0.1, "jump at {i}");
        assert!((-1.0..=1.0).contains(&v));
        prev = v;
    }
}

fn ev(tick: i64, on: bool, note: u64) -> NoteEvent {
    NoteEvent {
        tick: Ticks(tick),
        pitch: 60,
        velocity: if on { 100 } else { 0 },
        note: NoteId(note),
        ordinal: 0,
    }
}

#[test]
fn gates_stay_open_while_any_note_is_held_and_reopen_on_each_note() {
    let q = crate::PPQ;
    let g = Gates::of_events(&[
        ev(0, true, 1),
        ev(q / 2, true, 2),
        ev(q, false, 1),
        ev(2 * q, false, 2),
        ev(3 * q, true, 3),
        ev(4 * q, false, 3),
    ]);
    assert_eq!(
        g.gates,
        vec![
            Gate {
                on: 0.0,
                off: f64::INFINITY
            },
            Gate { on: 0.5, off: 2.0 },
            Gate { on: 3.0, off: 4.0 },
        ]
    );
}

/// The closed form (what a render from any bar reads) and the stepped envelope (what live
/// notes drive) are two implementations of one ADSR: stepped on a fine grid through the
/// same gates, they agree.
#[test]
fn the_closed_form_envelope_agrees_with_the_stepped_one() {
    let env = EnvSource {
        attack_ms: 40.0,
        decay_ms: 120.0,
        sustain: 0.4,
        release_ms: 200.0,
        trigger: EnvTrigger::Notes { track: None },
    };
    let gates = Gates {
        gates: vec![
            Gate { on: 0.0, off: 0.5 },
            // A retrigger during the release, held long, then released.
            Gate { on: 0.6, off: 1.8 },
        ],
        levels: None,
    };
    let spb = 0.5;
    let dt = 1.0 / 48_000.0;
    let mut live = LiveEnv::default();
    let mut worst = 0f32;
    let mut beat = 0.0;
    let mut gi = 0;
    while beat < 3.0 {
        // Gate changes land on the step where they are due, as the engine's grid does.
        if gi < gates.gates.len() && beat >= gates.gates[gi].on {
            live.set_gate(true, &env, true);
            gi += 1;
        }
        if gi > 0 && beat >= gates.gates[gi - 1].off && live.gate {
            live.set_gate(false, &env, false);
        }
        let closed = gates.level(&env, beat, spb);
        worst = worst.max((closed - live.level).abs());
        live.step(&env, dt);
        beat += dt / spb;
    }
    assert!(worst < 2e-3, "closed form and stepped differ by {worst}");
    assert_eq!(gates.level(&env, -1.0, spb), 0.0);
    // Held: the sustain.
    assert!((gates.level(&env, 1.5, spb) - 0.4).abs() < 1e-6);
    // Long after the release: silence.
    assert_eq!(gates.level(&env, 2.9, spb), 0.0);
}

#[test]
fn a_follower_rises_with_its_attack_falls_with_its_release_and_shrugs_off_nan() {
    let mut f = Follower::default();
    let dt = 16.0 / 48_000.0;
    for _ in 0..3000 {
        f.step(0.5, 5.0, 100.0, dt);
    }
    assert!((f.level - 0.5).abs() < 1e-4);
    assert!((f.output(6.0206) - 1.0).abs() < 1e-3);
    f.step(f32::NAN, 5.0, 100.0, dt);
    assert!(f.level.is_finite());
    let before = f.level;
    // 100 ms of silence takes 63 % of the way down.
    for _ in 0..300 {
        f.step(0.0, 5.0, 100.0, dt);
    }
    assert!(
        (f.level - before * cantus_dsp::math::exp(-1.0)).abs() < 2e-3,
        "{}",
        f.level
    );
}

#[test]
fn compiling_leaves_out_what_drives_nothing() {
    let (mut p, tid, sid, fid) = project_with_strip_fx();
    let plain = Snapshot::compile(&p, 1);
    let mut m = lfo(&mut p);
    m.routes.push(ModRoute {
        target: drive(fid),
        depth: 0.0,
    });
    m.routes.push(ModRoute {
        target: AutoTarget::Effect {
            effect: crate::EffectId(424_242),
            key: "drive".into(),
        },
        depth: 0.5,
    });
    p.track_mut(tid).unwrap().modulators.push(m.clone());
    let mut muted = lfo(&mut p);
    muted.muted = true;
    muted.routes.push(ModRoute {
        target: AutoTarget::StripGain(sid),
        depth: 0.5,
    });
    p.master.modulators.push(muted);
    let s = Snapshot::compile(&p, 1);
    assert!(s.modulators.is_empty());
    assert_eq!(s.automation, plain.automation);
}

#[test]
fn a_modulated_target_gets_a_lane_and_routes_find_it() {
    let (mut p, tid, sid, fid) = project_with_strip_fx();
    // An automation clip on the strip's pan, so lanes from clips and from routes mix.
    let clip = p.make_automation_clip(
        "pan",
        AutoTarget::StripPan(sid),
        Ticks::ZERO,
        Ticks::beats(4),
        vec![crate::AutoPoint::new(Ticks::ZERO, 0.2)],
    );
    p.track_mut(tid).unwrap().clips.push(clip);
    let mut m = lfo(&mut p);
    m.routes = vec![
        ModRoute {
            target: drive(fid),
            depth: 0.25,
        },
        ModRoute {
            target: AutoTarget::StripPan(sid),
            depth: -0.5,
        },
        ModRoute {
            target: AutoTarget::MasterGain,
            depth: 0.1,
        },
    ];
    let mut nest = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Random);
    nest.routes.push(ModRoute {
        target: AutoTarget::ModAmount(m.id),
        depth: -0.5,
    });
    p.track_mut(tid).unwrap().modulators.push(m.clone());
    p.master.modulators.push(nest.clone());
    let s = Snapshot::compile(&p, 1);
    assert!(s.automation.windows(2).all(|w| w[0].target < w[1].target));
    assert_eq!(s.modulators.len(), 2);
    let plan = s.modulators.iter().find(|x| x.id == m.id).unwrap();
    let targets: Vec<&AutoTarget> = plan
        .routes
        .iter()
        .map(|(i, _)| &s.automation[*i].target)
        .collect();
    assert_eq!(
        targets,
        vec![
            &drive(fid),
            &AutoTarget::StripPan(sid),
            &AutoTarget::MasterGain
        ]
    );
    assert_eq!(plan.routes[1].1, -0.5);
    // The pan lane keeps its clip; the drive lane has no pieces and the project's value.
    let pan = &s.automation[plan.routes[1].0];
    assert!(!pan.segs.is_empty());
    let d = &s.automation[plan.routes[0].0];
    assert!(d.segs.is_empty());
    assert_eq!(d.value_at(100.0), d.base);
    // The nested route: the first modulator's amount lane.
    let amount = plan.amount_lane.expect("its amount is modulated");
    assert_eq!(s.automation[amount].target, AutoTarget::ModAmount(m.id));
    assert_eq!(s.automation[amount].base, 1.0);
}

#[test]
fn a_modulator_round_trips_through_json() {
    let mut p = Project::new("j");
    for k in ModKind::ALL {
        let mut m = Modulator::new(ModulatorId(p.alloc_id()), k);
        m.routes.push(ModRoute {
            target: AutoTarget::Plugin {
                effect: crate::EffectId(7),
                id: 12,
            },
            depth: -0.75,
        });
        m.amount = 0.5;
        let text = serde_json::to_string(&m).unwrap();
        let back: Modulator = serde_json::from_str(&text).unwrap();
        assert_eq!(back, m);
    }
}

/// A depth dragged is many `SetModulator`s merged into one undo step that holds the last
/// modulator whole — never a growing batch.
#[test]
fn a_dragged_modulator_merges_into_one_superseding_step() {
    let (mut p, tid, _, fid) = project_with_strip_fx();
    let mut m = lfo(&mut p);
    m.routes.push(ModRoute {
        target: drive(fid),
        depth: 0.1,
    });
    p.track_mut(tid).unwrap().modulators.push(m.clone());
    let before = p.clone();
    let mut bus = crate::CommandBus::new(p);
    let key = crate::MergeKey::of(&("depth", m.id.0));
    for k in 1..=30 {
        let mut e = m.clone();
        e.routes[0].depth = 0.1 + k as f32 * 0.01;
        bus.execute_merging(
            Command::SetModulator {
                modulator: Box::new(e),
            },
            crate::Origin::User,
            key,
        )
        .unwrap();
    }
    assert_eq!(bus.history().len(), 1);
    assert!(matches!(
        bus.history()[0].command,
        Command::SetModulator { .. }
    ));
    bus.undo();
    assert_eq!(bus.project(), &before);
}

#[test]
fn a_slow_attack_under_dense_notes_climbs_whole_to_the_top() {
    // Review 1: the closed form walked back only 8 gates, so an attack many gates long
    // stalled near 9 gates' worth and fell back at every note-on.
    let env = EnvSource {
        attack_ms: 2000.0,
        decay_ms: 0.0,
        sustain: 1.0,
        release_ms: 200.0,
        trigger: EnvTrigger::Notes { track: None },
    };
    let spb = 0.5;
    let mut gates = Gates::default();
    for k in 0..64 {
        gates.gates.push(Gate {
            on: f64::from(k) * 0.25,
            off: f64::INFINITY,
        });
    }
    for g in [gates.clone(), gates.with_levels(&env, spb)] {
        let mut last = 0f32;
        let mut beat = 0.0;
        while beat < 8.0 {
            let v = g.level(&env, beat, spb);
            assert!(v >= last - 1e-6, "fell from {last} to {v} at beat {beat}");
            last = v;
            beat += 0.01;
        }
        // 2 s of attack is 4 beats: half way at 2, the top from 4 on.
        assert!((g.level(&env, 2.0, spb) - 0.5).abs() < 1e-4);
        assert!((g.level(&env, 6.0, spb) - 1.0).abs() < 1e-6);
    }
}

#[test]
fn a_follower_whose_strip_is_gone_hears_silence_not_the_master() {
    // Review 1: a removed listened strip fell back to the whole mix.
    use super::compile::ListenAt;
    let (mut p, tid, sid, fid) = project_with_strip_fx();
    let other = p.make_strip("Kick");
    let kick = other.id;
    p.strips.push(other);
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), ModKind::Follower);
    if let ModSource::Follower(f) = &mut m.source {
        f.listen = Listen::Strip(kick);
    }
    m.routes.push(ModRoute {
        target: drive(fid),
        depth: -0.5,
    });
    p.track_mut(tid).unwrap().modulators.push(m);
    let listen = |p: &Project| Snapshot::compile(p, 1).modulators[0].listen;
    assert!(matches!(listen(&p), Some(ListenAt::Strip(_))));
    p.strips.retain(|s| s.id != kick);
    assert_eq!(listen(&p), Some(ListenAt::Silent));
    // Listening to its own host's strip still hears that strip.
    let (host, _, _) = p
        .all_modulators()
        .next()
        .map(|(h, m)| (h, 0, m.id))
        .unwrap();
    p.modulators_mut(host).unwrap()[0].source = ModKind::Follower.source();
    assert_eq!(
        listen(&p),
        Some(ListenAt::Strip(p.strip_index(sid).unwrap()))
    );
}

#[test]
fn new_random_sources_do_not_move_in_step() {
    // Review 1: every new Random started with seed 1, so two moved identically.
    let a = Modulator::new(ModulatorId(41), ModKind::Random);
    let b = Modulator::new(ModulatorId(42), ModKind::Random);
    let seq = |m: &Modulator| {
        (0..16)
            .map(|k| output(&m.source, &t(f64::from(k) * 0.5), None, None))
            .collect::<Vec<_>>()
    };
    assert_ne!(seq(&a), seq(&b));
    // Still a repeatable function of the saved seed, inside the seed field's range.
    for m in [&a, &b] {
        let ModSource::Random(r) = &m.source else {
            unreachable!()
        };
        assert!((1..=999).contains(&r.seed), "seed {}", r.seed);
    }
    assert_eq!(
        seq(&a),
        seq(&Modulator::new(ModulatorId(41), ModKind::Random))
    );
}

/// Review 1 (finding 20): under a tempo map that moves, an envelope from notes counts its
/// gates in song seconds through the map, and a retriggered LFO in Hz counts its seconds
/// since the note-on through it — at one tempo, both exactly as before.
#[test]
fn modulators_read_time_through_a_moving_tempo_map() {
    use crate::tempo::{TempoMap, TempoPoint};
    let env = EnvSource {
        attack_ms: 400.0,
        decay_ms: 300.0,
        sustain: 0.5,
        release_ms: 2000.0,
        trigger: EnvTrigger::Notes { track: None },
    };
    let gates = Gates {
        gates: vec![Gate { on: 1.0, off: 5.0 }, Gate { on: 6.0, off: 7.0 }],
        levels: None,
    };
    // 120 BPM, then a ramp to 60 from beat 2 to beat 6.
    let pt = |b: i64, bpm: f64, ramp: bool| TempoPoint {
        tick: crate::Ticks::beats(b),
        bpm,
        ramp,
    };
    let map = TempoMap::new(
        120.0,
        crate::TimeSignature::default(),
        &[pt(0, 120.0, false), pt(2, 120.0, true), pt(6, 60.0, false)],
        &[],
    );
    let ppq = crate::PPQ as f64;
    let compiled = gates.with_levels_in(&env, &map);
    // Review 2 (finding 25): the release (2 s) outlasts the gap between the gates (beats
    // 5 to 6, ~0.93 s through the ramp), so the second gate starts from the first's
    // release level — computed here by hand in the map's seconds, not read from the code.
    let s = |beat: f64| map.seconds_at(beat * ppq);
    let first = Gate {
        on: 0.0,
        off: s(5.0) - s(1.0),
    };
    let by_hand = super::eval::gate_level(&env, 0.0, first, s(6.0) - s(1.0));
    let levels = compiled.levels.as_ref().unwrap();
    assert_eq!(levels.from[0], 0.0);
    assert!(by_hand > 0.05, "the release is still sounding: {by_hand}");
    assert!(
        (levels.from[1] - by_hand).abs() < 1e-6,
        "the second gate starts from {} (by hand {by_hand})",
        levels.from[1]
    );
    // At one tempo's seconds (0.5 s a beat) it would start elsewhere.
    let constant = super::eval::gate_level(
        &env,
        0.0,
        Gate { on: 0.0, off: 2.0 },
        2.5,
    );
    assert!((constant - by_hand).abs() > 1e-3);
    for beat in [1.2, 2.5, 4.0, 5.5, 6.3, 6.9, 7.4] {
        let t = ModTime::at_beats_in(beat, 0.0, &map);
        // By hand: the gate in seconds, from the map.
        let i = gates.at(beat).unwrap();
        let g = gates.gates[i];
        let on = map.seconds_at(g.on * ppq);
        let off = map.seconds_at(g.off * ppq);
        let from = compiled.levels.as_ref().unwrap().from[i];
        let want = super::eval::gate_level(
            &env,
            from,
            Gate {
                on: 0.0,
                off: off - on,
            },
            map.seconds_at(beat * ppq) - on,
        );
        assert_eq!(compiled.level_at(&env, &t), want, "beat {beat}");
    }
    // A retrigger 1.5 beats back, at a rate of 1 Hz: the seconds through the map.
    let t = ModTime::at_beats_in(5.0, 0.0, &map);
    let secs = map.seconds_between_f64(3.5 * ppq, 5.0 * ppq);
    assert!((t.seconds_since(3.5) - secs).abs() < 1e-12);
    // One tempo: the old arithmetic, to the bit.
    let c = TempoMap::constant(120.0, crate::TimeSignature::default());
    let t = ModTime::at_beats_in(5.0, 1.0, &c);
    assert_eq!(t, ModTime::at_beats(5.0, 1.0, 120.0));
    assert_eq!(gates.with_levels_in(&env, &c), gates.with_levels(&env, 0.5));
}
