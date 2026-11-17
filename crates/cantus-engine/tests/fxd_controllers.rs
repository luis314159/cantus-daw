//! The X-Y pad and the formula controller on the audio thread (fxd, ADRs 0147, 0148):
//! unlinked, at rest or at zero they touch no sample; moving — a pad on a path with
//! inertia, its Y through a bent range, a formula whose knob b a clip automates and whose
//! knob a an LFO drives — they are heard, block-size independent at tolerance 0,
//! the real-time processor equals the offline render, a synced formula and a path from
//! bar 5 are bars 5… of a full render, inertia glides where its absence jumps, and a
//! processor forked at a checkpoint (listen) carries the puck on to the bit.

use cantus_core::modulator::*;
use cantus_core::*;
use cantus_dsp::nulltest::{assert_null, compare};
use cantus_engine::*;
use std::sync::Arc;

const SR: f64 = 48_000.0;

/// A Firmus sine dyad on every beat for 32 beats (each released before the next, so a
/// render from any beat plays what a full one plays there), on a strip with a wide EQ bell.
fn base() -> Project {
    let mut p = Project::new("fxd");
    p.tempo_bpm = 120.0;
    let mut lead = p.make_track("Lead");
    let f = lead.instrument.firmus_mut().unwrap();
    f.osc1_wave = Waveform::Sine;
    f.amp_attack_s = 0.0;
    f.amp_release_s = 0.02;
    let mut clip = p.make_clip("beats", Ticks::ZERO, Ticks::beats(32));
    for b in 0..32 {
        for k in [69u8, 76] {
            let n = p.make_note(Ticks::beats(b), Ticks::QUARTER - Ticks(PPQ / 4), k, 100);
            clip.notes.push(n);
        }
    }
    lead.clips.push(clip);
    let strip = lead.strip.unwrap();
    p.tracks.push(lead);
    let mut eq = p.make_effect(EffectKind::Eq);
    for (k, v) in [
        ("b0.used", 1.0),
        ("b0.freq", 1_000.0),
        ("b0.q", 0.5),
        ("b0.gain", 0.0),
    ] {
        eq.spec.set(k, v).unwrap();
    }
    p.strip_mut(strip).unwrap().effects.push(eq);
    p
}

fn strip(p: &Project) -> StripId {
    p.tracks[0].strip.unwrap()
}

fn eq_gain(p: &Project) -> AutoTarget {
    AutoTarget::Effect {
        effect: p.strip(strip(p)).unwrap().effects[0].id,
        key: "b0.gain".into(),
    }
}

fn add(p: &mut Project, kind: ModKind, routes: Vec<(AutoTarget, f32)>) -> ModulatorId {
    let mut m = Modulator::new(ModulatorId(p.alloc_id()), kind);
    m.routes = routes
        .into_iter()
        .map(|(target, depth)| ModRoute { target, depth })
        .collect();
    let id = m.id;
    p.master.modulators.push(m);
    id
}

fn edit(p: &mut Project, id: ModulatorId, f: impl FnOnce(&mut ModSource)) {
    let (host, i, _) = p.modulator(id).unwrap();
    f(&mut p.modulators_mut(host).unwrap()[i].source);
    assert!(p.modulator(id).unwrap().2.is_valid());
}

fn snap(p: &Project) -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(p, 1))
}

fn opts(block: usize, start: Ticks, end: Ticks) -> RenderOptions {
    RenderOptions {
        start,
        end: Some(end),
        tail_seconds: 0.0,
        block_frames: block,
        ..Default::default()
    }
}

fn samples(p: &Project, block: usize) -> Vec<f32> {
    render(snap(p), &opts(block, Ticks::ZERO, Ticks::beats(16)))
        .unwrap()
        .samples
}

/// Every controller moving at once.
fn moving() -> Project {
    let mut p = base();
    let s = strip(&p);
    let gain = eq_gain(&p);
    let pad = add(
        &mut p,
        ModKind::XyPad,
        vec![(gain.clone(), 0.6), (AutoTarget::StripPan(s), 1.0)],
    );
    edit(&mut p, pad, |src| {
        let ModSource::XyPad(x) = src else {
            unreachable!()
        };
        x.inertia_ms = 180.0;
        x.path = Some(XyPath {
            points: vec![[0.1, 0.2], [0.9, 0.3], [0.7, 0.9], [0.2, 0.6], [0.5, 0.1]],
            beats: 3.0,
            smooth: true,
        });
        x.play_path = true;
        x.links = vec![RouteLink {
            target: AutoTarget::StripPan(s),
            output: 1,
            range: Some(RouteRange {
                from: -0.8,
                to: 0.4,
                curve: 0.5,
            }),
        }];
    });
    let f = add(
        &mut p,
        ModKind::Formula,
        vec![(AutoTarget::StripGain(s), -0.4)],
    );
    edit(&mut p, f, |src| {
        let ModSource::Formula(f) = src else {
            unreachable!()
        };
        f.text = "a*Sin(SongTime*Pi*(Int(b*10)))+c".into();
        f.inputs = [0.5, 0.3, 0.5];
    });
    // Knob b automated: 0.2 to 0.9 over beats 2…10.
    let knob = p.make_automation_clip(
        "b",
        AutoTarget::ModInput {
            modulator: f,
            input: 1,
        },
        Ticks::beats(2),
        Ticks::beats(8),
        vec![
            cantus_core::automation::AutoPoint::new(Ticks::ZERO, 0.2),
            cantus_core::automation::AutoPoint::new(Ticks::beats(8), 0.9),
        ],
    );
    p.tracks[0].clips.push(knob);
    // An LFO on knob a (one level of nesting).
    add(
        &mut p,
        ModKind::Lfo,
        vec![(
            AutoTarget::ModInput {
                modulator: f,
                input: 0,
            },
            0.4,
        )],
    );
    p
}

#[test]
fn unlinked_resting_and_zero_controllers_touch_no_sample() {
    let plain = samples(&base(), 512);
    assert!(cantus_dsp::nulltest::peak(&plain) > 0.05);
    let mut p = base();
    let s = strip(&p);
    // Unlinked: no route at all.
    add(&mut p, ModKind::XyPad, vec![]);
    add(&mut p, ModKind::Formula, vec![]);
    assert!(Snapshot::compile(&p, 1).modulators.is_empty());
    // At rest: the pad centred, bipolar, inertia on (the puck sits on its target).
    let g = eq_gain(&p);
    let pad = add(
        &mut p,
        ModKind::XyPad,
        vec![(AutoTarget::StripGain(s), 0.9), (g, 0.5)],
    );
    edit(&mut p, pad, |src| {
        let ModSource::XyPad(x) = src else {
            unreachable!()
        };
        x.inertia_ms = 300.0;
        x.links = vec![RouteLink {
            target: AutoTarget::StripGain(s),
            output: 1,
            range: Some(RouteRange {
                from: -0.3,
                to: 0.3,
                curve: 0.0,
            }),
        }];
    });
    // At zero: a formula whose value is 0, and one whose knobs are.
    let f0 = add(
        &mut p,
        ModKind::Formula,
        vec![(AutoTarget::StripPan(s), 0.7)],
    );
    edit(&mut p, f0, |src| {
        let ModSource::Formula(f) = src else {
            unreachable!()
        };
        f.text = "0".into();
    });
    let lead = p.tracks[0].id;
    let f1 = add(
        &mut p,
        ModKind::Formula,
        vec![(AutoTarget::TrackGain(lead), 0.7)],
    );
    edit(&mut p, f1, |src| {
        let ModSource::Formula(f) = src else {
            unreachable!()
        };
        f.text = "a*Sin(SongTime*Pi*b)+c".into();
        f.inputs = [0.0, 0.7, 0.0];
    });
    assert_eq!(Snapshot::compile(&p, 1).modulators.len(), 3);
    for block in [64usize, 333] {
        assert_null(&plain, &samples(&p, block), 0.0, "controllers at rest");
    }
}

#[test]
fn moving_controllers_are_heard_and_block_size_independent() {
    let p = moving();
    let reference = samples(&p, 512);
    let plain = samples(&base(), 512);
    assert!(
        !compare(&plain, &reference).passes(1e-3),
        "the controllers must be heard"
    );
    for block in [1usize, 7, 16, 37, 64, 333, 1024, 8192] {
        assert_null(
            &reference,
            &samples(&p, block),
            0.0,
            &format!("block {block}"),
        );
    }
}

#[test]
fn realtime_processing_matches_the_offline_render() {
    let s = snap(&moving());
    let offline = render(s.clone(), &opts(256, Ticks::ZERO, Ticks::beats(16))).unwrap();
    let live = live_render(&s, offline.frames());
    assert_null(&offline.samples, &live, 0.0, "realtime vs offline");
}

fn live_render(s: &Arc<Snapshot>, total: usize) -> Vec<f32> {
    let (mut tx, rx_engine) = rtrb::RingBuffer::<EngineMessage>::new(64);
    let (tx_engine, mut rx) = rtrb::RingBuffer::<EngineEvent>::new(4096);
    let mut p = Processor::without_midi(SR, rx_engine, tx_engine);
    let mut live = (**s).clone();
    live.loop_region = None;
    live.metronome = false;
    let live = Arc::new(live);
    for slot in live.effect_slots() {
        tx.push(EngineMessage::AddEffect(Box::new(EffectUnit::new(
            slot, SR as f32,
        ))))
        .ok();
    }
    tx.push(EngineMessage::Automation(
        cantus_engine::automation::Runtime::build(&live),
    ))
    .ok();
    tx.push(EngineMessage::SetSnapshot(live)).ok();
    tx.push(EngineMessage::Play).ok();
    let mut out = Vec::with_capacity(total * 2);
    // Irregular blocks, as a driver gives them.
    let sizes = [128usize, 37, 512, 1, 256, 999, 64];
    let mut k = 0;
    while out.len() < total * 2 {
        let n = sizes[k % sizes.len()].min(total - out.len() / 2);
        k += 1;
        let mut buf = vec![0.0f32; n * 2];
        p.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        while rx.pop().is_ok() {}
    }
    out
}

/// A synced formula and a path without inertia are functions of the song position: from
/// bar 5 they are bars 5… of the full render, to the bit.
#[test]
fn from_bar_5_a_formula_and_a_path_play_what_a_full_render_plays_there() {
    let mut p = base();
    let s = strip(&p);
    let f = add(
        &mut p,
        ModKind::Formula,
        vec![(AutoTarget::StripGain(s), 0.3)],
    );
    edit(&mut p, f, |src| {
        let ModSource::Formula(f) = src else {
            unreachable!()
        };
        f.text = "Round(0.5*Sin(SongTime*Pi*2*b)+0.5)*a + Frac(SongTime*0.75)*c".into();
        f.inputs = [0.6, 0.35, 0.4];
    });
    let pad = add(&mut p, ModKind::XyPad, vec![(AutoTarget::StripPan(s), 0.8)]);
    edit(&mut p, pad, |src| {
        let ModSource::XyPad(x) = src else {
            unreachable!()
        };
        x.path = Some(XyPath {
            points: vec![[0.0, 0.5], [1.0, 0.5], [0.3, 0.5]],
            beats: 1.5,
            smooth: false,
        });
        x.play_path = true;
    });
    let range = |start: Ticks| {
        render(snap(&p), &opts(300, start, Ticks::beats(24)))
            .unwrap()
            .samples
    };
    let full = range(Ticks::ZERO);
    let late = range(Ticks::beats(16));
    let offset = (16.0 * 0.5 * SR) as usize * 2;
    // The movement itself, as the ratio of the modulated render to the plain one (gain and
    // pan multiply the dry signal), must agree sample for sample.
    let mut q = base();
    q.master.modulators.clear();
    let plain_full = render(snap(&q), &opts(300, Ticks::ZERO, Ticks::beats(24)))
        .unwrap()
        .samples;
    let plain_late = render(snap(&q), &opts(300, Ticks::beats(16), Ticks::beats(24)))
        .unwrap()
        .samples;
    let ratio = |m: &[f32], d: &[f32]| -> Vec<f32> {
        m.iter()
            .zip(d)
            .map(|(a, b)| if b.abs() > 1e-3 { a / b } else { 0.0 })
            .collect()
    };
    let a = ratio(&full[offset..], &plain_full[offset..]);
    let b = ratio(&late, &plain_late);
    let n = a.len().min(b.len());
    assert!(compare(&a[..n], &b[..n]).passes(1e-5), "from bar 5");
}

/// Inertia glides: the pad's X jumped from 0 to 1 by automation, with 0 ms the strip's
/// gain lands within a grid step; with 200 ms it is under half way after 50 ms and nearly
/// there after a second.
#[test]
fn inertia_glides_where_its_absence_jumps() {
    let level = |inertia: f32| {
        let mut p = base();
        let s = strip(&p);
        let pad = add(
            &mut p,
            ModKind::XyPad,
            vec![(AutoTarget::StripGain(s), -0.5)],
        );
        edit(&mut p, pad, |src| {
            let ModSource::XyPad(x) = src else {
                unreachable!()
            };
            x.inertia_ms = inertia;
            x.unipolar = true;
        });
        let jump = p.make_automation_clip(
            "x",
            AutoTarget::ModInput {
                modulator: pad,
                input: 0,
            },
            Ticks::ZERO,
            Ticks::beats(8),
            vec![
                cantus_core::automation::AutoPoint::new(Ticks::ZERO, 0.0),
                cantus_core::automation::AutoPoint::new(Ticks::beats(2), 0.0),
                cantus_core::automation::AutoPoint::new(Ticks::beats(2), 1.0),
                cantus_core::automation::AutoPoint::new(Ticks::beats(8), 1.0),
            ],
        );
        p.tracks[0].clips.push(jump);
        let out = render(snap(&p), &opts(256, Ticks::ZERO, Ticks::beats(8)))
            .unwrap()
            .samples;
        let plain = samples(&base(), 256);
        // The gain applied, from the RMS of a 10 ms window against the plain render's.
        move |t: f64| {
            let at = (t * SR) as usize * 2;
            let w = (0.01 * SR) as usize * 2;
            let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
            rms(&out[at..at + w]) / rms(&plain[at..at + w])
        }
    };
    let (snap0, glide) = (level(0.0), level(200.0));
    // Windows inside notes (each beat sounds for its first three quarters).
    let before = snap0(0.6);
    let after = snap0(1.2);
    assert!(
        after < before * 0.7,
        "the gain went down: {before} → {after}"
    );
    // The jump is at 1 s (beat 2 at 120 BPM).
    assert!(
        (snap0(1.02) - after).abs() < 0.02 * before,
        "no inertia: there at once"
    );
    let g = glide(1.05);
    assert!(
        (g - before).abs() < 0.5 * (before - after),
        "200 ms: under half way at 50 ms ({g})"
    );
    assert!(
        (glide(2.6) - after).abs() < 0.03 * before,
        "and there after 1.6 s"
    );
}

/// A processor forked at a checkpoint (listen, ADR 0107) carries the puck and every
/// controller's state: a section heard from its checkpoint is the whole render's.
#[test]
fn a_fork_carries_the_puck_on() {
    use cantus_engine::listen::{listen, Checkpoints, ListenOptions};
    let p = moving();
    let s = Snapshot::compile(&p, 1);
    let opts = |start| ListenOptions {
        start,
        end: Some(Ticks::beats(16)),
        tail_seconds: 0.0,
        keep_audio: true,
        sections: vec![Ticks::beats(8)],
        ..Default::default()
    };
    let mut cps = Checkpoints::default();
    let whole = listen(
        &s,
        &opts(Ticks::ZERO),
        Vec::new(),
        Vec::new(),
        &mut cps,
        &mut |_| true,
    )
    .expect("listens");
    assert_eq!(cps.ticks(), vec![Ticks::beats(8)]);
    let from = listen(
        &s,
        &opts(Ticks::beats(8)),
        Vec::new(),
        Vec::new(),
        &mut cps,
        &mut |_| true,
    )
    .expect("listens");
    assert_eq!(
        from.resumed,
        cantus_engine::listen::Resumed::Checkpoint(Ticks::beats(8))
    );
    let at = (8.0 * 0.5 * SR) as usize * 2;
    assert_null(&whole.audio[at..], &from.audio, 0.0, "from the checkpoint");
}
