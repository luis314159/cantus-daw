//! Saltus end to end (fx side round, wave 3, ADR 0135 provisional), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine handle; a line playing on a track;
//! Saltus inserted on its strip as the picker inserts it — on its starting preset, the poly
//! twelve-string: the octave is heard in the master, and the master comes late by the poly
//! engine's latency, the dry aligned to it; then the engine switched to mono as the
//! display's switch does — the latency falls to 2.7 ms; the interval moved as the editor
//! moves it — a fifth instead of the octave, heard; undone, the octave again, to the bit;
//! and at its neutral defaults, the dry master late by the mono latency, to the bit. Its own
//! test binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fx3:saltus

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
        tail_seconds: 0.2,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0f32, |m, v| m.max(v.abs()))
}

/// The largest difference between `a` and `b` moved `frames` later (interleaved stereo).
fn off_by(a: &[f32], b: &[f32], frames: usize) -> f32 {
    let s = 2 * frames;
    a[..a.len() - s]
        .iter()
        .zip(&b[s..])
        .fold(0f32, |m, (x, y)| m.max((x - y).abs()))
}

fn latency(app: &CantusApp, strip: StripId) -> usize {
    let slot = app.project().strip(strip).unwrap().effects[0].clone();
    EffectUnit::new(&slot, 48_000.0).latency()
}

#[test]
fn saltus_on_a_strip_starts_on_the_twelve_string_switches_engine_and_undoes() {
    let (mut app, _proc) = assembled();
    let mut p = Project::new("Saltus");
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
    assert!(peak(&dry) > 0.01, "the line sounds");

    // Inserted as the picker inserts it: the twelve-string, poly, heard.
    app.add_effect(strip, EffectKind::PitchShift);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(spec.get("engine"), 1.0, "the poly twelve-string");
    let poly = latency(&app, strip);
    assert_eq!(poly, 4_095);
    let twelve = rendered(&mut app);
    assert!(
        off_by(&dry, &twelve, poly) > 0.01,
        "the octave is heard over the dry"
    );

    // The display's engine switch: mono, 2.7 ms.
    let set = |app: &mut CantusApp, key: &str, value: f32| {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        })
    };
    set(&mut app, "engine", 0.0);
    assert_eq!(latency(&app, strip), 128, "mono");
    let mono_octave = rendered(&mut app);

    // The interval moved as the editor moves it: a fifth, heard; undone, the octave again.
    set(&mut app, "st1", 7.0);
    let fifth = rendered(&mut app);
    assert!(off_by(&mono_octave, &fifth, 0) > 0.01, "the fifth is heard");
    app.undo();
    assert_eq!(rendered(&mut app), mono_octave, "undone, to the bit");

    // Its neutral defaults: the dry master, late by the mono latency, to the bit.
    for key in ["st1", "lvl1", "dry", "out"] {
        let d = EffectKind::PitchShift.param(key).unwrap().default;
        set(&mut app, key, d);
    }
    let null = rendered(&mut app);
    assert_eq!(off_by(&dry, &null, 128), 0.0, "the null, to the bit");
    assert!(
        null[..256].iter().all(|x| *x == 0.0),
        "nothing before the latency"
    );
}
