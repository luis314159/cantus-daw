//! The user's own starting presets end to end (ADR 0138, provisional), on the boot path: a
//! `settings.json` naming Saltus's start as the mono *Fifth up, live* is read by
//! `CantusApp::assemble`; Saltus inserted as the picker inserts it, as Tabula's *Add pedal*
//! adds it and as an agent's `add_effect` adds it all start there, and it is heard (the
//! render moves); a board's start chosen from its own settings becomes every new board's;
//! *Reset to factory starting preset* brings back the poly twelve-string; the neutral switch
//! keeps its rule; and the choice is in the settings file for the next start. Its own test
//! binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fx3:start

use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_mcp::client::TestClient;
use cantus_mcp::Answer;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use serde_json::json;
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

fn mono_fifth() -> EffectSpec {
    let mut s = EffectSpec::new(EffectKind::PitchShift);
    for (k, v) in [("st1", 7.0), ("lvl1", 0.5), ("dry", 1.0), ("out", -1.0)] {
        s.set(k, v);
    }
    s
}

fn assembled() -> (CantusApp, Processor, egui::Context) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    (app, proc_, ctx)
}

fn rendered(app: &mut CantusApp) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = RenderOptions {
        end: Some(Ticks::beats(4)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn last_effect(app: &CantusApp, strip: StripId) -> EffectSlot {
    app.project()
        .strip(strip)
        .unwrap()
        .effects
        .last()
        .unwrap()
        .clone()
}

#[test]
fn every_insert_starts_on_the_persons_choice_and_the_choice_is_kept() {
    // The settings as an earlier session left them.
    let settings = json!({
        "starting": [{ "label": "Fifth up, live", "spec": serde_json::to_value(mono_fifth()).unwrap() }]
    });
    std::fs::write(data().join("settings.json"), settings.to_string()).unwrap();
    let (mut app, _proc, ctx) = assembled();
    let mut p = Project::new("Start");
    p.tempo_bpm = 120.0;
    let mut track = p.make_track("Lead");
    let mut clip = p.make_clip("Line", Ticks::ZERO, Ticks::beats(4));
    for (i, pitch) in [57u8, 60, 64, 62].into_iter().enumerate() {
        let n = p.make_note(Ticks::beats(i as i64), Ticks::beats(1), pitch, 100);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);

    // The picker's path.
    app.add_effect(strip, EffectKind::PitchShift);
    assert_eq!(last_effect(&app, strip).spec, mono_fifth(), "the picker");
    assert_ne!(rendered(&mut app), dry, "the fifth is heard");

    // Tabula's Add pedal: a board (its factory rig) and Saltus added to it.
    app.add_effect(strip, EffectKind::Board);
    let board = last_effect(&app, strip).id;
    assert_eq!(
        last_effect(&app, strip).spec,
        cantus_core::fx::tabula::guitar_rig()
    );
    let id = {
        let mut host = cantus_ui::views::fx::host::DawHost::new(&mut app, strip, board, "tabula");
        cantus_ui::views::fx::tabula_edit::add(&mut host, EffectKind::PitchShift).unwrap()
    };
    let spec = last_effect(&app, strip).spec;
    let pedal = cantus_core::fx::tabula::board(&spec)
        .pedal(id)
        .unwrap()
        .clone();
    assert_eq!(pedal.spec, mono_fifth(), "Add pedal");

    // An agent's add_effect.
    cantus_ui::ai_mcp::wire_with(&mut app, &ctx, cantus_ui::ai_mcp::catalog::app());
    let mut c = TestClient::legacy(app.ai.server().unwrap().clone(), "2025-11-25");
    let r = c.call_tool(
        "add_effect",
        json!({"strip_id": strip.0, "kind": "saltus"}),
        &mut || {
            cantus_ui::ai_mcp::pump(&mut app);
            if let Some(agents) = app.ai.agents_mut() {
                let ids: Vec<u64> = agents.asks().map(|x| x.id).collect();
                for id in ids {
                    agents.answer(id, Answer::Allow);
                }
            }
        },
    );
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(last_effect(&app, strip).spec, mono_fifth(), "an agent");

    // A board's start chosen from its own settings (its preset menu's *Use as starting
    // preset*), then every new board's.
    let mut mine = cantus_core::fx::tabula::guitar_rig();
    mine.set("mix", 0.4);
    app.use_as_starting(mine.clone(), "My rig".into());
    app.add_effect(strip, EffectKind::Board);
    assert_eq!(last_effect(&app, strip).spec, mine);

    // Kept for the next start.
    let kept = cantus_ui::settings::Settings::load(&data().join("settings.json"));
    assert_eq!(kept.starting.len(), 2);

    // Reset to factory: the poly twelve-string again.
    app.reset_starting(EffectKind::PitchShift);
    app.add_effect(strip, EffectKind::PitchShift);
    assert_eq!(
        last_effect(&app, strip).spec,
        cantus_core::starting::starting_spec(EffectKind::PitchShift)
    );
    assert_eq!(last_effect(&app, strip).spec.get("engine"), 1.0);

    // The neutral switch keeps its rule: no starting preset, the person's or the factory's.
    app.set_insert_neutral(true);
    app.add_effect(strip, EffectKind::Board);
    assert_eq!(
        last_effect(&app, strip).spec,
        EffectSpec::new(EffectKind::Board)
    );
}
