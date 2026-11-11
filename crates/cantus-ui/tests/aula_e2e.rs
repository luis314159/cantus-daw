//! Aula end to end (round 39, ADR 0112), on the boot path: an app made by
//! `CantusApp::assemble` with the real engine handle; Aula inserted on a track's strip the way
//! the picker inserts it (its starting preset); a response read from a WAV file the way the
//! editor's *Load file* and a drop set it (one command, one undo step); the song rendered
//! offline through the engine's compile — the path export, freeze and stems take — and the
//! file's tail heard after the notes, in the file's own colour (its right side quieter).
//! Then undone: back to Cantus's own hall, and a render that differs. Its own test binary: it
//! points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

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

/// A two-second stereo response at 44.1 kHz, 24-bit: sparse echoes decaying, the right side
/// at a third of the left.
fn write_ir(path: &Path) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let full = (1 << 23) - 1;
    for i in 0..88_200 {
        let g = cantus_dsp::math::exp(-3.0 * i as f32 / 44_100.0);
        let v = if i % 211 == 0 { g * 0.8 } else { 0.0 };
        w.write_sample((v * full as f32) as i32).unwrap();
        w.write_sample((v / 3.0 * full as f32) as i32).unwrap();
    }
    w.finalize().unwrap();
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

/// Energy of the left and the right from `from` seconds on.
fn tail(x: &[f32], from: f32) -> (f64, f64) {
    let at = (from * 48_000.0) as usize * 2;
    let (mut l, mut r) = (0.0f64, 0.0f64);
    for f in x[at.min(x.len())..].chunks_exact(2) {
        l += (f[0] as f64).powi(2);
        r += (f[1] as f64).powi(2);
    }
    (l, r)
}

#[test]
fn aula_on_a_strip_plays_a_files_tail_in_an_offline_render_and_undoes() {
    let (mut app, _proc) = assembled();
    // One track playing one short note at the start, nothing after.
    let mut p = Project::new("Aula");
    let mut track = p.make_track("Keys");
    let mut clip = p.make_clip("Hit", Ticks::ZERO, Ticks::beats(8));
    clip.notes
        .push(p.make_note(Ticks::ZERO, Ticks::SIXTEENTH, 64, 110));
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);

    // Inserted as the picker inserts it: on its starting preset (a quarter wet).
    app.add_effect(strip, EffectKind::Aula);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = &app.project().effect(strip, effect).unwrap().spec;
    assert_eq!(spec.get("mix"), 0.25, "the starting preset");
    assert_eq!(spec.get("ir"), 7.0);
    // Full wet, so the response alone is measured.
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: "mix".into(),
        value: 1.0,
    });
    let own = rendered(&mut app);

    // The response from a file, as Load file sets it.
    let dir = tempfile::tempdir().unwrap();
    let file: PathBuf = dir.path().join("Echo room.wav");
    write_ir(&file);
    cantus_ui::views::fx::aula::set_file(
        &mut app,
        strip,
        effect,
        Some(file.to_string_lossy().to_string()),
    );
    let ir = app.project().effect(strip, effect).unwrap().spec.ir.clone();
    assert_eq!(ir.as_deref().map(|f| f.name()), Some("Echo room"));
    let wet = rendered(&mut app);
    assert!(wet.iter().all(|v| v.is_finite()));

    // The note is over by half a second; the file's echoes go on for two.
    let (dl, dr) = tail(&dry, 0.6);
    let (wl, wr) = tail(&wet, 0.6);
    assert!(
        wl > 100.0 * dl.max(1e-12),
        "the tail is heard: {wl:e} against {dl:e} dry"
    );
    assert!(
        wl > 4.0 * wr && wr > 0.0,
        "its right side is quieter, as the file: {wl:e} {wr:e}"
    );
    assert!(dr <= dl.max(1e-12) * 2.0);
    assert_ne!(wet, own, "the file is not Cantus's own hall");
    assert!(matches!(
        cantus_engine::fx::aula_store::status(&file.to_string_lossy()),
        cantus_engine::fx::aula_store::IrStatus::Loaded {
            rate: 44_100,
            channels: 2,
            ..
        }
    ));

    // One undo step: back to the hall, to its bits.
    app.undo();
    assert!(app
        .project()
        .effect(strip, effect)
        .unwrap()
        .spec
        .ir
        .is_none());
    assert_eq!(
        rendered(&mut app),
        own,
        "undone, the hall renders as before"
    );
}
