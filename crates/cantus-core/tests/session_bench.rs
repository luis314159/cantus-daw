//! D3 (the integrator's review): what a projection costs on a large project — 128 tracks,
//! a 64-bar arrangement on each, a session column of four one-bar loops with follow
//! actions on every track (a chain to the horizon), audio on 16 of them — measured as the
//! conductor pays it on every change: the plan, `project_play` and `Snapshot::compile`.
//! The numbers go to stderr (`--nocapture`); the bound only catches a regression by an
//! order of magnitude.

use cantus_core::session::play::{open_ended, plan, project_play, Input, Timed};
use cantus_core::session::*;
use cantus_core::*;
use std::time::Instant;

const BAR: Ticks = Ticks::beats(4);

fn big() -> Project {
    let mut p = Project::new("big");
    let sid = AudioSourceId(p.alloc_id());
    p.audio_pool.push(cantus_core::audio::AudioSource {
        id: sid,
        name: "loop".into(),
        path: "Audio/loop.wav".into(),
        sample_rate: 48_000,
        channels: 2,
        frames: 48_000 * 600,
        sha256: None,
        recorded: false,
    });
    let scenes: Vec<SceneId> = (0..4).map(|_| SceneId(p.alloc_id())).collect();
    for s in &scenes {
        p.session.scenes.push(Scene::new(*s, ""));
    }
    for ti in 0..128 {
        let mut t = p.make_track(format!("T{ti}"));
        for k in 0..16 {
            let audio = ti % 8 == 0;
            let mut c = if audio {
                p.make_audio_clip("a", sid, BAR * (k * 4), BAR * 4)
            } else {
                p.make_clip("n", BAR * (k * 4), BAR * 4)
            };
            if !audio {
                for i in 0..64 {
                    let n = p.make_note(Ticks(i * 240), Ticks(200), 40 + (i % 40) as u8, 90);
                    c.notes.push(n);
                }
            }
            t.clips.push(c);
        }
        for s in &scenes {
            let audio = ti % 8 == 0;
            let mut c = if audio {
                p.make_audio_clip("la", sid, Ticks::ZERO, BAR)
            } else {
                p.make_clip("l", Ticks::ZERO, BAR)
            };
            c.content_length = Some(BAR);
            if !audio {
                for i in 0..16 {
                    let n = p.make_note(Ticks(i * 240), Ticks(200), 50 + i as u8, 100);
                    c.notes.push(n);
                }
            }
            let mut sc = SessionClip::new(c);
            sc.follow = FollowActions {
                on: true,
                a: FollowAction::Next,
                b: FollowAction::Other,
                chance_a: 70,
                after: FollowAfter::Linked,
                count: 1,
            };
            t.slots.push(Slot {
                scene: *s,
                clip: Some(sc),
                stop: true,
            });
        }
        p.tracks.push(t);
    }
    p
}

#[test]
fn a_projection_of_a_large_project_costs_this_much() {
    let p = big();
    let scene = p.session.scenes[0].id;
    let inputs = [Timed::at(
        Ticks(10),
        Input::LaunchScene {
            scene,
            velocity: 1.0,
        },
    )];
    let mut plan_ms = Vec::new();
    let mut proj_ms = Vec::new();
    let mut compile_ms = Vec::new();
    for _ in 0..12 {
        let t0 = Instant::now();
        let pl = plan(&p, &inputs, BAR * 32);
        let t1 = Instant::now();
        let perf = open_ended(&p, &pl.perf, BAR * 32, 64);
        let q = project_play(&p, &perf);
        let t2 = Instant::now();
        let snap = Snapshot::compile(&q, 1);
        let t3 = Instant::now();
        assert!(!snap.tracks[1].events.is_empty());
        plan_ms.push((t1 - t0).as_secs_f64() * 1000.0);
        proj_ms.push((t2 - t1).as_secs_f64() * 1000.0);
        compile_ms.push((t3 - t2).as_secs_f64() * 1000.0);
    }
    let worst = |v: &[f64]| v.iter().copied().fold(0.0, f64::max);
    let med = |v: &mut Vec<f64>| {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    };
    let (wp, wj, wc) = (worst(&plan_ms), worst(&proj_ms), worst(&compile_ms));
    eprintln!(
        "128 tracks: plan {:.1} ms (worst {wp:.1}), project_play {:.1} ms (worst {wj:.1}), compile {:.1} ms (worst {wc:.1}); total worst {:.1} ms",
        med(&mut plan_ms),
        med(&mut proj_ms),
        med(&mut compile_ms),
        wp + wj + wc
    );
    assert!(wp + wj + wc < 5_000.0);
    // One track launched while the scene plays: planned on from what was planned, its
    // projection compiled reusing every track it did not change.
    let first = plan(&p, &inputs, BAR * 32);
    let q0 = project_play(&p, &open_ended(&p, &first.perf, BAR * 32, 64));
    let s0 = Snapshot::compile(&q0, 1);
    let slot = SlotRef::new(p.tracks[5].id, p.session.scenes[2].id);
    let mut more = inputs.to_vec();
    more.push(Timed::at(
        BAR * 2 + Ticks(5),
        Input::Launch {
            slot,
            velocity: 1.0,
        },
    ));
    let mut one = Vec::new();
    for _ in 0..12 {
        let t0 = Instant::now();
        let pl =
            cantus_core::session::play::plan_from(&p, &more, BAR * 32, Some((&first, BAR * 2)));
        let q = project_play(&p, &open_ended(&p, &pl.perf, BAR * 32, 64));
        let s = Snapshot::compile_reusing(&q, 2, Some((&q0, &s0)));
        one.push(t0.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(s, Snapshot::compile(&q, 2), "reusing compiles the same");
    }
    let w1 = worst(&one);
    eprintln!("128 tracks, one track launched: plan + project_play + compile_reusing {:.1} ms (worst {w1:.1})", med(&mut one));
}
