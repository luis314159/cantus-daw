//! Rota end to end (fx side round, wave 3, ADR 0136 provisional), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine handle; notes playing on a track;
//! Rota inserted on its strip as the picker inserts it — on its starting preset, Chorale:
//! the rotary heard in the master, with no latency; the switch thrown to fast as the
//! display throws it, heard, and undone to the bit; **the track's notes throwing the
//! switch** (its notes as Rota's source): each note toggles slow and fast, so the master
//! differs from the same notes with the notes switch off; and at its neutral defaults, the
//! dry master to the bit. Its own test binary: it points `CANTUS_DATA_DIR` at a temporary
//! folder. // fx3:rota

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

fn diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0f32, |m, (x, y)| m.max((x - y).abs()))
}

#[test]
fn rota_on_a_strip_starts_on_chorale_switches_by_hand_and_by_notes_and_undoes() {
    let (mut app, _proc) = assembled();
    let mut p = Project::new("Rota");
    p.tempo_bpm = 120.0;
    let mut track = p.make_track("Organ");
    let mut clip = p.make_clip("Chords", Ticks::ZERO, Ticks::beats(4));
    for (i, pitch) in [60u8, 64, 67, 72].into_iter().enumerate() {
        let n = p.make_note(Ticks::beats(i as i64), Ticks::beats(1), pitch, 100);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let track = app.project().tracks[0].id;
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);
    assert!(peak(&dry) > 0.01, "the notes sound");

    // Inserted as the picker inserts it: Chorale, heard, no latency.
    app.add_effect(strip, EffectKind::Rotary);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(
        (spec.get("mix"), spec.get("speed")),
        (1.0, 1.0),
        "Chorale: slow"
    );
    let slot = app.project().strip(strip).unwrap().effects[0].clone();
    assert_eq!(EffectUnit::new(&slot, 48_000.0).latency(), 0);
    let chorale = rendered(&mut app);
    assert!(diff(&dry, &chorale) > 0.01, "the rotary is heard");

    let set = |app: &mut CantusApp, key: &str, value: f32| {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        })
    };
    // The switch thrown to fast, heard; undone, Chorale again to the bit.
    set(&mut app, "speed", 2.0);
    let fast = rendered(&mut app);
    assert!(diff(&chorale, &fast) > 0.01, "fast is heard");
    app.undo();
    assert_eq!(rendered(&mut app), chorale, "undone, to the bit");

    // The track's own notes throw the switch: with them as Rota's source each note toggles
    // the speed; with the notes switch off the same source changes nothing.
    app.exec(Command::SetEffectMidiSource {
        strip,
        effect,
        source: Some(track),
    });
    let by_notes = rendered(&mut app);
    set(&mut app, "notes", 0.0);
    let held = rendered(&mut app);
    assert_eq!(
        held, chorale,
        "the notes switch off: the notes change nothing"
    );
    assert!(diff(&by_notes, &held) > 0.01, "the notes threw the switch");

    // Its neutral defaults: the dry master, to the bit.
    for key in ["mix", "out", "notes"] {
        let d = EffectKind::Rotary.param(key).unwrap().default;
        set(&mut app, key, d);
    }
    assert_eq!(rendered(&mut app), dry, "the null, to the bit");
}
