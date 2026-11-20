//! r41:ids — another project in the place of the open one, end to end on the boot path
//! (BACKLOG §1, ADR 0149). Ids start again from 1 in every project, so the open project's
//! units must never serve the new one's inserts of the same id: opened after Neon Tide,
//! whose id 35 was a strip's compressor, Aurora's master EQ (also 35) was taken as
//! delivered, never sent, and skipped — it showed no input and changed nothing (beta.15's
//! known issue). An app made by `CantusApp::assemble` with the real engine handle and its
//! processor, and:
//!
//! - **every demo after every other** (both orders, the 42 pairs), opened as the demo picker
//!   opens it: the engine runs, chain by chain, exactly the units the new demo opened alone
//!   runs — its own ids, in their places, of their kinds — and every insert of it;
//! - **the same through *Replace project*** (an import, a recovery, the AI's door), its undo
//!   and its redo — the paths that keep the old project in the history;
//! - **nothing of the old song is heard**: a delay full of the open song's echoes, and the
//!   next project the same song without its notes — same ids, same places, same kinds — is
//!   silent to the bit, as it is opened alone.
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::Path;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn assembled() -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    (app, proc_)
}

/// `frames` frames of the live engine, in blocks of 512, the interface syncing between
/// them as its frames do.
fn play(app: &mut CantusApp, proc_: &mut Processor, frames: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(frames * 2);
    let mut done = 0;
    while done < frames {
        let n = 512.min(frames - done);
        let mut buf = vec![0f32; n * 2];
        proc_.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        done += n;
        app.sync_engine();
    }
    out
}

/// The interface syncs, the engine takes what it was sent, and the interface hears back
/// (the units handed back), twice: what a few frames do.
fn settle(app: &mut CantusApp, proc_: &mut Processor) {
    for _ in 0..2 {
        app.sync_engine();
        proc_.settle();
        let mut buf = vec![0f32; 2 * 64];
        proc_.process(&mut buf, 2);
    }
    app.sync_engine();
    proc_.settle();
}

type Held = Vec<(StripId, EffectId, EffectKind)>;

/// What the engine runs, in a stable order.
fn held(proc_: &Processor) -> Held {
    let mut h = proc_.held_effects();
    h.sort_by_key(|(s, e, _)| (s.0, e.0));
    h
}

/// Every insert of the project, in the same order.
fn inserts(p: &Project) -> Held {
    let mut h: Held = p
        .all_effects()
        .map(|(s, e)| (s, e.id, e.spec.kind))
        .collect();
    h.sort_by_key(|(s, e, _)| (s.0, e.0));
    h
}

/// What the engine runs for `p` opened alone, in an app of its own.
fn alone(p: &Project) -> Held {
    let (mut app, mut proc_) = assembled();
    app.replace_project(p.clone());
    settle(&mut app, &mut proc_);
    held(&proc_)
}

fn demos() -> Vec<(&'static str, Project)> {
    cantus_ui::demos::ALL
        .iter()
        .map(|d| (d.id, (d.build)()))
        .collect()
}

/// How many inserts of `b` share an id with an insert of `a` in another place or of
/// another kind, and how many in the same place and kind: the collisions a pair tries.
fn collisions(a: &Project, b: &Project) -> (usize, usize) {
    let (ia, ib) = (inserts(a), inserts(b));
    let mut other = 0;
    let mut same = 0;
    for (s, e, k) in &ib {
        if let Some((s0, _, k0)) = ia.iter().find(|(_, e0, _)| e0 == e) {
            if s0 == s && k0 == k {
                same += 1;
            } else {
                other += 1;
            }
        }
    }
    (other, same)
}

#[test]
fn every_demo_after_every_other_runs_its_own_inserts() {
    let demos = demos();
    let own: Vec<Held> = demos.iter().map(|(_, p)| alone(p)).collect();
    for ((id, p), h) in demos.iter().zip(&own) {
        assert_eq!(*h, inserts(p), "{id} opened alone runs every insert it has");
    }
    let (mut app, mut proc_) = assembled();
    let mut tried = (0, 0);
    for (i, (a_id, a)) in demos.iter().enumerate() {
        for (j, (b_id, b)) in demos.iter().enumerate() {
            if i == j {
                continue;
            }
            let c = collisions(a, b);
            tried = (tried.0 + c.0, tried.1 + c.1);
            app.replace_project(a.clone());
            settle(&mut app, &mut proc_);
            app.toggle_play();
            play(&mut app, &mut proc_, 4_800);
            app.toggle_play();
            assert_eq!(held(&proc_), own[i], "{a_id} (after the last pair)");
            let dropped = proc_.stats().dropped_events;
            app.replace_project(b.clone());
            settle(&mut app, &mut proc_);
            assert_eq!(
                held(&proc_),
                own[j],
                "{b_id} opened after {a_id}: its own inserts, in their places, of their kinds"
            );
            assert_eq!(
                proc_.stats().dropped_events,
                dropped,
                "{b_id} after {a_id}: no insert skipped for want of its unit"
            );
        }
    }
    eprintln!(
        "ids shared in another place or kind: {}; in the same place and kind: {}",
        tried.0, tried.1
    );
    assert!(
        tried.0 > 0,
        "the demos collide in place or kind (Aurora after Neon Tide)"
    );
}

#[test]
fn replace_project_its_undo_and_its_redo_run_each_projects_inserts() {
    let demos = demos();
    // The pair with the most ids shared in another place or kind.
    let mut best = (0, 0, 1);
    for i in 0..demos.len() {
        for j in 0..demos.len() {
            let c = collisions(&demos[i].1, &demos[j].1).0;
            if i != j && c > best.0 {
                best = (c, i, j);
            }
        }
    }
    let ((a_id, a), (b_id, b)) = (&demos[best.1], &demos[best.2]);
    eprintln!("{b_id} in the place of {a_id}: {} ids shared", best.0);
    let (own_a, own_b) = (alone(a), alone(b));
    let (mut app, mut proc_) = assembled();
    app.replace_project(a.clone());
    settle(&mut app, &mut proc_);
    assert_eq!(held(&proc_), own_a);
    assert!(app.exec(Command::ReplaceProject {
        project: Box::new(b.clone()),
    }));
    settle(&mut app, &mut proc_);
    assert_eq!(held(&proc_), own_b, "replaced");
    app.undo();
    settle(&mut app, &mut proc_);
    assert_eq!(held(&proc_), own_a, "undone");
    app.redo();
    settle(&mut app, &mut proc_);
    assert_eq!(held(&proc_), own_b, "redone");
}

/// A lead on every beat into a delay with a long feedback on its strip.
fn echoes() -> Project {
    let mut p = Project::new("Echoes");
    p.tempo_bpm = 120.0;
    let mut track = p.make_track("Lead");
    let mut clip = p.make_clip("Lead", Ticks::ZERO, Ticks::beats(8));
    for b in 0..4 {
        let n = p.make_note(Ticks::beats(b), Ticks::QUARTER, 64, 110);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    p.tracks.push(track);
    p
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}

#[test]
fn nothing_of_the_old_song_is_heard_in_the_next() {
    let (mut app, mut proc_) = assembled();
    app.replace_project(echoes());
    let strip = app.project().tracks[0].strip.expect("a strip");
    app.add_effect(strip, EffectKind::Delay);
    let fx = app
        .project()
        .strip(strip)
        .unwrap()
        .effects
        .last()
        .unwrap()
        .id;
    assert!(app.exec(Command::SetEffectParam {
        strip,
        effect: fx,
        key: "feedback".into(),
        value: 0.95,
    }));
    settle(&mut app, &mut proc_);
    app.seek(Ticks::ZERO);
    app.toggle_play();
    let heard = play(&mut app, &mut proc_, 96_000);
    app.toggle_play();
    assert!(
        peak(&heard[48_000 * 2..]) > 1e-3,
        "the song sounds, its echoes in the delay"
    );

    // The same song without its notes: the same ids, in the same places, of the same kinds.
    let mut quiet = app.project().clone();
    for t in &mut quiet.tracks {
        for c in &mut t.clips {
            c.notes.clear();
        }
    }
    app.replace_project(quiet.clone());
    settle(&mut app, &mut proc_);
    app.toggle_play();
    let after = play(&mut app, &mut proc_, 48_000);
    app.toggle_play();

    let (mut lone, mut lone_proc) = assembled();
    lone.replace_project(quiet);
    settle(&mut lone, &mut lone_proc);
    lone.toggle_play();
    let alone = play(&mut lone, &mut lone_proc, 48_000);
    lone.toggle_play();
    eprintln!(
        "after the echoes: peak {:e}; opened alone: peak {:e}",
        peak(&after),
        peak(&alone)
    );
    assert_eq!(after.len(), alone.len());
    for (i, (a, b)) in after.iter().zip(&alone).enumerate() {
        assert_eq!(
            a.to_bits(),
            b.to_bits(),
            "sample {i}: the next song as it is opened alone (an echo of the last one heard)"
        );
    }
}
