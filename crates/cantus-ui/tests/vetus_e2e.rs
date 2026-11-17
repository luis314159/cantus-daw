//! Vetus end to end (fx side round, ADR 0130), on the boot path: an app made by
//! `CantusApp::assemble` with the real engine handle; a clean sine chord playing on a track;
//! Vetus inserted on its strip as the picker inserts it (its starting preset, a worn
//! cassette); the song rendered offline through the engine's compile — the path export,
//! freeze and stems take — and the tape heard in the master: hiss between the notes'
//! harmonics, the top dulled. Then the bits switched on at 3 as the editor switches them:
//! the master gets the quantiser's grit; undone, the render is the cassette's again, to the
//! bit. Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
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

/// Two bars at 120 BPM, no tail.
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

/// The energy of the first difference (a gentle high-pass) of the left channel: hiss and
/// grit raise it.
fn edge(x: &[f32]) -> f64 {
    let l: Vec<f32> = x.chunks_exact(2).map(|f| f[0]).collect();
    l.windows(2)
        .map(|w| f64::from(w[1] - w[0]).powi(2))
        .sum::<f64>()
        / l.len() as f64
}

#[test]
fn vetus_on_a_strip_ages_the_master_in_an_offline_render_and_undoes() {
    let (mut app, _proc) = assembled();
    let mut p = Project::new("Vetus");
    p.tempo_bpm = 120.0;
    let mut track = p.make_track("Keys");
    let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(8));
    for pitch in [48u8, 55, 60, 64] {
        clip.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(8), pitch, 100));
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);
    assert!(dry.iter().any(|v| v.abs() > 0.01), "the chord sounds");

    // Inserted as the picker inserts it: the worn cassette.
    app.add_effect(strip, EffectKind::LoFi);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(spec.get("wob.on"), 1.0, "the starting preset is heard");
    assert_eq!(spec.get("noise.on"), 1.0);
    let tape = rendered(&mut app);
    assert!(tape.iter().all(|v| v.is_finite()));
    assert_ne!(tape, dry, "the tape is heard in the master");

    // The bits switched on at 3, as the editor switches them: the grit.
    for (key, value) in [("bits.on", 1.0), ("bits.depth", 3.0)] {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        });
    }
    let gritty = rendered(&mut app);
    assert!(
        edge(&gritty) > 1.5 * edge(&tape),
        "the quantiser's steps: {:e} against {:e}",
        edge(&gritty),
        edge(&tape)
    );

    // Undone, the cassette again, to the bit.
    app.undo();
    app.undo();
    assert_eq!(rendered(&mut app), tape, "undone: the cassette again");
}
