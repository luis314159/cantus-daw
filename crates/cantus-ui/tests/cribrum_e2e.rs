//! Cribrum end to end (fx side round, ADR 0130 provisional), on the boot path: an app made
//! by `CantusApp::assemble` with the real engine handle; a bright synth chord playing on a
//! track; Cribrum inserted on its strip as the picker inserts it (its starting preset, a
//! resonant ladder swept once a bar); the song rendered offline through the engine's compile
//! — the path export, freeze and stems take — and the sweep heard in the master: the
//! highs come and go with the bar. Then a control changed as the editor changes it (the
//! cutoff down, the sweep off): the master darkens; undone, the render is the sweep's again,
//! to the bit. Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder.

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

/// The energy of the highs (the first difference, a gentle high-pass) of the left channel in
/// each eighth of a bar.
fn highs_by_eighth(x: &[f32]) -> Vec<f64> {
    let eighth = 12_000usize; // frames at 120 BPM, 48 kHz
    x.chunks_exact(2)
        .map(|f| f[0])
        .collect::<Vec<f32>>()
        .windows(2)
        .map(|w| f64::from(w[1] - w[0]).powi(2))
        .collect::<Vec<f64>>()
        .chunks(eighth)
        .map(|c| c.iter().sum::<f64>() / c.len() as f64)
        .collect()
}

#[test]
fn cribrum_on_a_strip_sweeps_the_master_in_an_offline_render_and_undoes() {
    let (mut app, _proc) = assembled();
    // One track holding a bright chord for two bars.
    let mut p = Project::new("Cribrum");
    p.tempo_bpm = 120.0;
    let mut track = p.make_track("Synth");
    let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(8));
    for pitch in [48u8, 55, 60, 64] {
        clip.notes
            .push(p.make_note(Ticks::ZERO, Ticks::beats(8), pitch, 110));
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);
    assert!(dry.iter().any(|v| v.abs() > 0.01), "the chord sounds");

    // Inserted as the picker inserts it: on its starting preset, fully wet.
    app.add_effect(strip, EffectKind::AutoFilter);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(spec.get("mix"), 1.0, "the starting preset is heard");
    assert!(spec.get("lfo.cut") > 0.0, "and it moves");
    let swept = rendered(&mut app);
    assert!(swept.iter().all(|v| v.is_finite()));
    assert_ne!(swept, dry, "the filter is heard in the master");
    // The sweep: once a bar, the highs come and go — the brightest eighth of the second bar
    // far above its darkest.
    let h = highs_by_eighth(&swept);
    let bar2 = &h[8..16];
    let (lo, hi) = bar2
        .iter()
        .fold((f64::MAX, 0f64), |(a, b), v| (a.min(*v), b.max(*v)));
    assert!(hi > 6.0 * lo, "the sweep moves the highs: {lo:e} … {hi:e}");

    // A control changed as the editor changes it: the cutoff down to 200 Hz, the sweep off.
    for (key, value) in [("a.cut", 200.0), ("lfo.cut", 0.0)] {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        });
    }
    let dark = rendered(&mut app);
    let (hs, hd) = (
        highs_by_eighth(&swept)[8..].iter().sum::<f64>(),
        highs_by_eighth(&dark)[8..].iter().sum::<f64>(),
    );
    assert!(hd * 10.0 < hs, "the master darkens: {hd:e} against {hs:e}");

    // Undone, the sweep is back, to the bit.
    app.undo();
    app.undo();
    assert_eq!(rendered(&mut app), swept, "undone: the sweep again");
}
