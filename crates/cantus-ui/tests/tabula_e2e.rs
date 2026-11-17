//! Tabula end to end (ADR 0131), on the boot path: an app made by `CantusApp::assemble`
//! with the real engine handle; Tabula inserted on a playing track's strip the way the
//! picker inserts it (its starting preset: the guitar rig); the song rendered offline
//! through the engine's compile — the path export, freeze and stems take — and the rig heard
//! in the master; a board of one pedal heard exactly as that unit inserted alone; a pedal
//! moved by the board's own edit (one step of undo, undone to the bit); a pedal added from
//! the picker's path, a footswitch, a factory board loaded as a preset.

use cantus_core::fx::tabula::{Path, PedalId};
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
    let mut p = Project::new("Tabula");
    let mut track = p.make_track("Guitar");
    let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    for (i, pitch) in [52u8, 55, 57, 59, 52, 55, 57, 59].iter().enumerate() {
        clip.notes
            .push(p.make_note(Ticks::beats(i as i64), Ticks::beats(1), *pitch, 112));
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    app.project().tracks[0].strip.expect("a strip")
}

#[test]
fn tabula_on_a_playing_strip_is_heard_moves_in_one_step_and_undoes() {
    let (mut app, _proc) = assembled();
    let strip = song(&mut app);
    let dry = rendered(&mut app);
    assert!(dry.iter().any(|v| v.abs() > 1e-3), "the track plays");

    // Inserted as the picker inserts it: the guitar rig on the board.
    app.add_effect(strip, EffectKind::Board);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let kinds: Vec<EffectKind> = app
        .project()
        .effect(strip, effect)
        .unwrap()
        .spec
        .pedals()
        .iter()
        .map(|p| p.spec.kind)
        .collect();
    assert_eq!(
        kinds,
        [
            EffectKind::Pedal,
            EffectKind::Chorus,
            EffectKind::Delay,
            EffectKind::Space
        ],
        "the guitar rig"
    );
    let rig = rendered(&mut app);
    assert!(!same(&dry, &rig), "the rig heard in the master");

    // A pedal moved by the board's own edit (a drag lands as this): one step of undo.
    let mut host = DawHost::new(&mut app, strip, effect, "tabula");
    tabula_edit::move_to(&mut host, PedalId(3), 0, Path::Pre);
    drop(host);
    assert_eq!(
        app.project().effect(strip, effect).unwrap().spec.pedals()[0].id,
        PedalId(3)
    );
    let moved = rendered(&mut app);
    assert!(!same(&rig, &moved), "the order is heard");
    app.undo();
    assert_eq!(
        app.project().effect(strip, effect).unwrap().spec.pedals()[0].id,
        PedalId(0),
        "the move undone in one step"
    );
    assert!(
        same(&rig, &rendered(&mut app)),
        "and the sound with it, to the bit"
    );

    // A footswitch: the pedal out of the sound, and back.
    let mut host = DawHost::new(&mut app, strip, effect, "tabula");
    tabula_edit::footswitch(&mut host, PedalId(0), false);
    drop(host);
    let off = rendered(&mut app);
    assert!(!same(&rig, &off), "the overdrive switched off is heard");
    app.undo();
    assert!(same(&rig, &rendered(&mut app)));

    // A pedal added as the picker adds it: a fresh id, sounding at once.
    let mut host = DawHost::new(&mut app, strip, effect, "tabula");
    let id = tabula_edit::add(&mut host, EffectKind::Tremolo).expect("added");
    drop(host);
    assert_eq!(id, PedalId(4));
    assert!(!same(&rig, &rendered(&mut app)), "the tremolo heard");
    app.undo();
    assert!(same(&rig, &rendered(&mut app)));

    // A factory board loaded as a preset: the whole board, one step.
    let boards = cantus_ui::views::fx::factory_presets(EffectKind::Board);
    let (_, values) = boards
        .iter()
        .find(|(k, _)| *k == "preset.tabula.vocal_doubler")
        .expect("the doubler");
    let map: std::collections::BTreeMap<String, f32> =
        values.iter().map(|(k, v)| (k.to_string(), *v)).collect();
    app.apply_fx_params(strip, effect, &map, "cmd.fx_preset");
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert!(
        cantus_core::fx::tabula::board(&spec).has_split(),
        "the doubler's split"
    );
    assert!(!same(&rig, &rendered(&mut app)));
    app.undo();
    assert!(same(&rig, &rendered(&mut app)), "the rig back, to the bit");
}

/// A board of one pedal in the app is that unit inserted alone, to the bit.
#[test]
fn a_board_of_one_in_the_app_is_the_unit() {
    let (mut app, _proc) = assembled();
    let strip = song(&mut app);
    app.add_effect(strip, EffectKind::Chorus);
    let alone = rendered(&mut app);
    let unit = app.project().strip(strip).unwrap().effects[0].clone();
    app.undo();
    app.add_effect(strip, EffectKind::Board);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let mut slot = app.project().effect(strip, effect).unwrap().clone();
    slot.spec = EffectSpec::new(EffectKind::Board);
    slot.spec.board = Some(Box::new(cantus_core::fx::tabula::Board {
        pedals: vec![cantus_core::fx::tabula::Pedal::new(
            PedalId(0),
            unit.spec.clone(),
        )],
        macros: Vec::new(),
    }));
    app.exec(Command::ReplaceEffect {
        strip,
        effect,
        slot: Box::new(slot),
    });
    assert!(
        same(&alone, &rendered(&mut app)),
        "a board of one is its unit"
    );
}
