//! Arca end to end (fx side round, wave 3, ADR 0137 provisional), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine handle; a line playing on a track;
//! Arca inserted on its strip as the picker inserts it — on its starting preset, the clean
//! combo: the amp and its cabinet heard in the master, late by the oversampler's latency;
//! the gain turned as the plate turns it, heard, and undone to the bit; **the user's impulse
//! response** loaded as the cabinet's file and chosen (a delayed click: the master comes
//! that much later); and at its neutral defaults, the dry master to the bit. Its own test
//! binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fx3:arca

use cantus_core::*;
use cantus_engine::{render, EffectUnit, EngineHandle, Processor, RenderOptions};
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
    let opts = RenderOptions {
        end: Some(Ticks::beats(4)),
        tail_seconds: 0.3,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0f32, |m, v| m.max(v.abs()))
}

/// The first frame of interleaved stereo louder than a thousandth of the peak.
fn onset(x: &[f32]) -> usize {
    let floor = peak(x) * 1e-3;
    x.iter().position(|v| v.abs() > floor).unwrap_or(x.len()) / 2
}

fn latency(app: &CantusApp, strip: StripId) -> usize {
    let slot = app.project().strip(strip).unwrap().effects[0].clone();
    EffectUnit::new(&slot, 48_000.0).latency()
}

/// A mono impulse response: one click `at` samples in, at 48 kHz.
fn click_file(at: usize) -> String {
    let path = data().join("click.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..4_800 {
        w.write_sample(if i == at { 0.5f32 } else { 0.0 }).unwrap();
    }
    w.finalize().unwrap();
    path.to_string_lossy().to_string()
}

#[test]
fn arca_on_a_strip_starts_on_the_clean_combo_turns_loads_a_response_and_undoes() {
    let (mut app, _proc) = assembled();
    let mut p = Project::new("Arca");
    p.tempo_bpm = 120.0;
    let mut track = p.make_track("Guitar");
    let mut clip = p.make_clip("Line", Ticks::ZERO, Ticks::beats(4));
    for (i, pitch) in [52u8, 55, 59, 57].into_iter().enumerate() {
        let n = p.make_note(Ticks::beats(i as i64), Ticks::beats(1), pitch, 100);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);
    assert!(peak(&dry) > 0.01, "the line sounds");

    // Inserted as the picker inserts it: the clean combo, heard, late by the oversampler.
    app.add_effect(strip, EffectKind::Amp);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(
        (spec.get("model"), spec.get("cab")),
        (1.0, 1.0),
        "the clean combo"
    );
    let lat = latency(&app, strip);
    assert_eq!(lat, cantus_engine::fx::arca::settings(&spec).0.latency());
    assert!(lat > 0);
    let combo = rendered(&mut app);
    assert!(peak(&combo) > 0.01 && combo != dry, "the amp is heard");

    let set = |app: &mut CantusApp, key: &str, value: f32| {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        })
    };
    // The gain turned, heard; undone, the combo again to the bit.
    set(&mut app, "gain", 0.9);
    let driven = rendered(&mut app);
    assert!(driven != combo, "the gain is heard");
    app.undo();
    assert_eq!(rendered(&mut app), combo, "undone, to the bit");

    // The user's impulse response: a click 480 samples in, loaded and chosen, no amp — the
    // master is the dry line 10 ms later.
    cantus_ui::views::fx::aula::set_file(&mut app, strip, effect, Some(click_file(480)));
    set(&mut app, "cab", 6.0);
    set(&mut app, "model", 0.0);
    assert_eq!(latency(&app, strip), 0, "the cabinet alone has no latency");
    let through = rendered(&mut app);
    assert!(peak(&through) > 0.001, "the file plays");
    let (a, b) = (onset(&dry), onset(&through));
    assert!(
        (b as i64 - a as i64 - 480).abs() <= 2,
        "the click's delay: dry at {a}, through the file at {b}"
    );

    // Its neutral defaults: the dry master, to the bit.
    for key in ["model", "cab", "out", "gain"] {
        let d = EffectKind::Amp.param(key).unwrap().default;
        set(&mut app, key, d);
    }
    assert_eq!(rendered(&mut app), dry, "the null, to the bit");
}
