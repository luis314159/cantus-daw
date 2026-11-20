//! Fragor's circuits as pedals of their own, end to end (round 41, ADR 0152), on the boot
//! path: an app made by `CantusApp::assemble` with the real engine handle; Tabula inserted
//! on a playing track's strip as the picker inserts it; its board emptied and filled from
//! the board's own shelf path (`tabula_edit::add`, each pedal's starting preset) with the
//! green overdrive, a phaser and the triangle muff — two drives in series with another
//! pedal between; the song rendered offline through the engine's compile (the path export,
//! freeze and stems take). The chain is heard, and heard exactly as the three units inserted
//! in a row with each drive as Fragor with only that slot filled; a footswitch on the
//! second drive is heard; the song saved, opened again and rendered to the bit; and undo
//! takes the pedals back off in a step each.

use cantus_core::fx::fragor_pedal::{as_fragor, switch};
use cantus_core::fx::tabula::PedalId;
use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::views::fx::host::DawHost;
use cantus_ui::views::fx::tabula_edit;
use cantus_ui::CantusApp;
use std::path::Path as FsPath;

fn data() -> &'static FsPath {
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

fn rendered(app: &mut CantusApp) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    render(
        snap,
        &RenderOptions {
            end: Some(Ticks::beats(8)),
            tail_seconds: 0.0,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

fn song(app: &mut CantusApp) -> StripId {
    let mut p = Project::new("Pedals");
    let mut track = p.make_track("Guitar");
    let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    for (i, pitch) in [40u8, 43, 45, 47, 40, 43, 45, 47].iter().enumerate() {
        clip.notes
            .push(p.make_note(Ticks::beats(i as i64), Ticks::beats(1), *pitch, 112));
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    app.project().tracks[0].strip.expect("a strip")
}

fn pedals_of(app: &CantusApp, strip: StripId, effect: EffectId) -> Vec<(PedalId, EffectSpec)> {
    app.project()
        .effect(strip, effect)
        .unwrap()
        .spec
        .pedals()
        .iter()
        .map(|p| (p.id, p.spec.clone()))
        .collect()
}

#[test]
fn two_drives_on_a_board_sound_as_fragor_reopen_and_undo() {
    let (mut app, _proc) = assembled();
    let strip = song(&mut app);
    let dry = rendered(&mut app);
    assert!(dry.iter().any(|v| v.abs() > 1e-3), "the track plays");

    // Tabula as the picker inserts it, emptied, then the three from its shelf's path.
    app.add_effect(strip, EffectKind::Board);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let rig: Vec<PedalId> = pedals_of(&app, strip, effect).iter().map(|p| p.0).collect();
    // One host per gesture, as the window makes one each frame.
    for id in rig {
        tabula_edit::remove(&mut DawHost::new(&mut app, strip, effect, "tabula"), id);
    }
    let chain = [
        EffectKind::FragorOverdrive,
        EffectKind::Phaser,
        EffectKind::FragorMuff,
    ];
    let ids: Vec<PedalId> = chain
        .iter()
        .map(|k| {
            tabula_edit::add(&mut DawHost::new(&mut app, strip, effect, "tabula"), *k)
                .expect("a pedal added")
        })
        .collect();
    let pedals = pedals_of(&app, strip, effect);
    assert_eq!(
        pedals.iter().map(|p| p.1.kind).collect::<Vec<_>>(),
        chain,
        "two drives with a phaser between, in order"
    );
    for (_, s) in [&pedals[0], &pedals[2]] {
        let c = s.kind.fragor_circuit().unwrap();
        assert_eq!(s.get(switch(c)), 1.0, "{:?} starts with its circuit in", s.kind);
    }
    let board = rendered(&mut app);
    assert!(!same(&dry, &board), "the chain is heard in the master");

    // The chain exactly as the three units inserted in a row, each drive as Fragor with
    // only its slot filled.
    let saved_project = app.project().clone();
    {
        let mut p = saved_project.clone();
        let fresh: Vec<EffectId> = pedals.iter().map(|_| EffectId(p.alloc_id())).collect();
        let s = p.strips.iter_mut().find(|s| s.id == strip).unwrap();
        let template = s.effects[0].clone();
        s.effects.clear();
        for (i, (_, spec)) in pedals.iter().enumerate() {
            let mut slot = template.clone();
            slot.id = fresh[i];
            slot.spec = if spec.kind.fragor_circuit().is_some() {
                as_fragor(spec)
            } else {
                spec.clone()
            };
            s.effects.push(slot);
        }
        let (mut other, _p2) = assembled();
        other.replace_project(p);
        assert!(
            same(&board, &rendered(&mut other)),
            "the board is its pedals in a row, each drive Fragor with one slot"
        );
    }

    // The second drive's footswitch: heard, and back.
    let mut host = DawHost::new(&mut app, strip, effect, "tabula");
    tabula_edit::footswitch(&mut host, ids[2], false);
    drop(host);
    assert!(!same(&board, &rendered(&mut app)), "the muff's footswitch heard");
    app.undo();
    assert!(same(&board, &rendered(&mut app)), "and back, to the bit");

    // Saved and opened again: the same board, the same sound to the bit.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pedals.cantus");
    app.save_to_path(&path);
    assert!(path.exists());
    app.new_project();
    app.open(&path);
    assert_eq!(*app.project(), saved_project, "the board reopened as it was");
    assert!(same(&board, &rendered(&mut app)), "reopened, to the bit");

    // Undo in the session it was made: each pedal off in one step.
    let (mut b, _p3) = assembled();
    let strip = song(&mut b);
    b.add_effect(strip, EffectKind::Board);
    let effect = b.project().strip(strip).unwrap().effects[0].id;
    let before = rendered(&mut b);
    let mut host = DawHost::new(&mut b, strip, effect, "tabula");
    tabula_edit::add(&mut host, EffectKind::FragorFuzz).expect("the fuzz added");
    drop(host);
    assert!(!same(&before, &rendered(&mut b)), "the fuzz heard on the rig");
    b.undo();
    assert!(same(&before, &rendered(&mut b)), "undone to the bit");
}
