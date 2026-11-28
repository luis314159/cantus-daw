//! An effect's editor reads its own unit's telemetry only (2026-11-27), on the boot path:
//! an app made by `CantusApp::assemble` with the real engine handle, the first demo
//! playing, the processor run by hand so the test decides when the engine reports.
//!
//! The engine keeps the last analysed unit's readings until the next analysed unit reports.
//! The soak (`soak_smoke`, under load) opened Multiplex's editor right after another
//! effect's, before the audio thread had run a block: the editor took the other kind's
//! slot 22 for its sample rate (1.52 Hz), and `warp`'s clamp in its curve panicked —
//! `min > max`. The same frame in the app, with the device stalled or the callback late,
//! was a crash. `Meters::fx_of` now says whose the readings are, and an editor whose unit
//! they are not draws at rest until its own report comes. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

mod harness;

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

const SIZE: egui::Vec2 = egui::Vec2::new(1600.0, 900.0);

/// Blocks and frames, one each, until `done` holds (at most `n`); whether it did.
fn run_until(
    app: &mut CantusApp,
    proc_: &mut Processor,
    ctx: &egui::Context,
    n: usize,
    mut done: impl FnMut(&CantusApp) -> bool,
) -> bool {
    let mut out = vec![0f32; 512];
    for i in 0..n {
        proc_.process(&mut out, 2);
        harness::frame(app, ctx, SIZE, i as f64 / 60.0);
        if done(app) {
            return true;
        }
    }
    false
}

fn last_effect(app: &CantusApp, strip: StripId) -> EffectId {
    app.project().effects_of(strip).unwrap().last().unwrap().id
}

#[test]
fn an_editor_opened_after_another_never_reads_the_last_units_telemetry() {
    let ctx = harness::ctx();
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let (engine, mut proc_) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    harness::isolate(&mut app, &data().join("isolated"));
    cantus_ui::views::demos::open(&mut app, 0);
    app.toggle_play();
    let strip = app
        .project()
        .strips
        .iter()
        .find(|s| !s.id.is_master())
        .map(|s| s.id)
        .expect("a strip");
    app.add_effect(strip, EffectKind::Multiband);
    let multiband = last_effect(&app, strip);
    let quiet = [0.0f32; cantus_engine::TELEMETRY_LEN];
    let mut raced = Vec::new();
    for kind in [
        EffectKind::Compressor,
        EffectKind::Limiter,
        EffectKind::Maximizer,
        EffectKind::Space,
        EffectKind::Reverb,
        EffectKind::Delay,
        EffectKind::Gate,
        EffectKind::Saturator,
        EffectKind::Stereo,
    ] {
        app.add_effect(strip, kind);
        let other = last_effect(&app, strip);
        // The other effect's editor, until the engine has reported on it.
        app.fx.open = Some((strip, other, kind));
        let reported = run_until(&mut app, &mut proc_, &ctx, 200, |a| {
            a.engine.meters().fx_of == Some((other, kind))
        });
        if !reported {
            continue; // a kind with no telemetry leaves the last readings as they were
        }
        assert_eq!(
            app.fx_telemetry_of(other, kind),
            app.engine.meters().fx,
            "{kind:?}: its own readings"
        );
        // Multiplex's editor at once, no block run: the readings are still the other's.
        app.fx.open = Some((strip, multiband, EffectKind::Multiband));
        harness::frame(&mut app, &ctx, SIZE, 10.0);
        harness::frame(&mut app, &ctx, SIZE, 10.1);
        assert_eq!(app.engine.meters().fx_of, Some((other, kind)));
        assert_eq!(
            app.fx_telemetry_of(multiband, EffectKind::Multiband),
            quiet,
            "Multiplex shown {kind:?}'s readings"
        );
        raced.push(kind);
        // Its own report comes: its numbers, its rate.
        assert!(run_until(&mut app, &mut proc_, &ctx, 200, |a| {
            a.engine.meters().fx_of == Some((multiband, EffectKind::Multiband))
        }));
        let t = app.fx_telemetry_of(multiband, EffectKind::Multiband);
        assert_eq!(t, app.engine.meters().fx);
        assert_eq!(t[cantus_engine::fx::multiband::T_RATE], 48_000.0);
    }
    eprintln!("raced against Multiplex's editor: {raced:?}");
    assert!(raced.len() >= 3, "only {raced:?} reported telemetry");

    // Multiplex replaced by a Limiter (the soak's swap; the slot comes with its own id) and
    // the new unit's editor opened at once: Multiplex's readings are not its.
    app.fx.open = Some((strip, multiband, EffectKind::Multiband));
    assert!(run_until(&mut app, &mut proc_, &ctx, 200, |a| {
        a.engine.meters().fx_of == Some((multiband, EffectKind::Multiband))
    }));
    let slot = app.bus.new_effect(EffectKind::Limiter);
    let limiter = slot.id;
    app.exec(Command::ReplaceEffect {
        strip,
        effect: multiband,
        slot: Box::new(slot),
    });
    app.fx.open = Some((strip, limiter, EffectKind::Limiter));
    harness::frame(&mut app, &ctx, SIZE, 20.0);
    assert_eq!(
        app.engine.meters().fx_of,
        Some((multiband, EffectKind::Multiband))
    );
    assert_eq!(app.fx_telemetry_of(limiter, EffectKind::Limiter), quiet);
    assert!(run_until(&mut app, &mut proc_, &ctx, 200, |a| {
        a.engine.meters().fx_of == Some((limiter, EffectKind::Limiter))
    }));
    assert_eq!(
        app.fx_telemetry_of(limiter, EffectKind::Limiter),
        app.engine.meters().fx
    );
    // And the kind is part of whose they are: the same id read as another kind is at rest.
    assert_eq!(app.fx_telemetry_of(limiter, EffectKind::Multiband), quiet);
}
