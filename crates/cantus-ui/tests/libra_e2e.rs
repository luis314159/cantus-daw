//! Libra end to end (fx side round, ADR 0133 provisional), on the boot path: an app made by
//! `CantusApp::assemble` with the real engine handle; a chord playing on a track; Libra
//! inserted on its strip as the picker inserts it (a wire: the master is the dry one, to the
//! bit); then its gain and its operation changed as the editor changes them — −6 dB and the
//! side alone: a mono chord's side is silence, the master goes quiet; undone, the dry master
//! again, to the bit. Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

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

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0f32, |m, v| m.max(v.abs()))
}

#[test]
fn libra_on_a_strip_is_a_wire_then_corrects_the_master_and_undoes() {
    let (mut app, _proc) = assembled();
    let mut p = Project::new("Libra");
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
    assert!(peak(&dry) > 0.01, "the chord sounds");

    app.add_effect(strip, EffectKind::Utility);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    assert_eq!(rendered(&mut app), dry, "inserted, Libra is a wire");

    // Down 6 dB, as the editor sets it: half.
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: "out".into(),
        value: -6.020_6,
    });
    let half = rendered(&mut app);
    let ratio = peak(&half) / peak(&dry);
    assert!((ratio - 0.5).abs() < 0.01, "−6 dB is half: {ratio}");

    // The side alone, as the channel keys set it: the master's own side is what is left.
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: "chan".into(),
        value: 6.0,
    });
    let side = rendered(&mut app);
    assert!(
        peak(&side) < 0.6 * peak(&half),
        "the side alone is less than the whole"
    );

    app.undo();
    app.undo();
    assert_eq!(
        rendered(&mut app),
        dry,
        "undone: the wire again, to the bit"
    );
}
