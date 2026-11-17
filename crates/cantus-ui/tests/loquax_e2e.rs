//! Loquax end to end (fx side round, wave A, ADR 0139 provisional), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine handle; a voice (a line on its track),
//! a keyboard whose track is muted (its notes still sent, ADR 0022) and a pad. Loquax is
//! inserted on the voice's strip as the picker inserts it — on its starting preset, Robot:
//! the voice heard vocoded on the drone with no notes and no key. **The synthesiser played by
//! MIDI notes**: the drone off, the carrier the synthesiser, silent with no notes, heard with
//! the keyboard's track as the slot's notes. **The sidechain carrier**: the carrier the
//! sidechain, silent with no key, heard with the pad's strip as the slot's key. An edit (the
//! formants moved) heard and undone to the bit; the object the voice speaks into (a look)
//! changes no sample; and at its neutral defaults, the dry master to the bit. Its own test
//! binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fxa:loquax

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

/// The largest difference between two renders.
fn diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).fold(0f32, |m, (x, y)| m.max((x - y).abs()))
}

/// A track playing `notes` (pitch, start beat, beats).
fn line(p: &mut Project, name: &str, notes: &[(u8, i64, i64)]) -> Track {
    let mut track = p.make_track(name);
    let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(4));
    for &(pitch, at, len) in notes {
        let n = p.make_note(Ticks::beats(at), Ticks::beats(len), pitch, 100);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    track
}

#[test]
fn loquax_speaks_through_the_drone_the_notes_and_the_sidechain_and_undoes() {
    let (mut app, _proc) = assembled();
    let mut p = Project::new("Loquax");
    p.tempo_bpm = 120.0;
    let voice = line(&mut p, "Voice", &[(57, 0, 1), (60, 1, 1), (64, 2, 1), (62, 3, 1)]);
    let mut keys = line(&mut p, "Keys", &[(48, 0, 4), (52, 0, 4), (55, 0, 4)]);
    keys.muted = true;
    let pad = line(&mut p, "Pad", &[(45, 0, 4), (52, 0, 4)]);
    p.tracks.extend([voice, keys, pad]);
    app.replace_project(p);
    let keys = app.project().tracks[1].id;
    let strip = app.project().tracks[0].strip.expect("a strip");
    let pad = app.project().tracks[2].strip.expect("a strip");
    let dry = rendered(&mut app);
    assert!(peak(&dry) > 0.01, "the voice and the pad sound");

    // Inserted as the picker inserts it: Robot, the voice vocoded on the drone, no latency.
    app.add_effect(strip, EffectKind::Vocoder);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(
        (spec.get("mix"), spec.get("robot"), spec.get("bands")),
        (1.0, 1.0, 16.0),
        "Robot"
    );
    let slot = app.project().strip(strip).unwrap().effects[0].clone();
    assert_eq!(EffectUnit::new(&slot, 48_000.0).latency(), 0);
    let robot = rendered(&mut app);
    assert!(diff(&dry, &robot) > 0.01, "the robot is heard");

    let set = |app: &mut CantusApp, key: &str, value: f32| {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        })
    };
    // The synthesiser as the carrier: with no notes the voice has nothing to speak through
    // (the master is the pad alone, as with the voice taken away); the keyboard's notes
    // played into it, the vocoded voice is heard.
    set(&mut app, "robot", 0.0);
    set(&mut app, "src", 1.0);
    let no_notes = rendered(&mut app);
    app.exec(Command::SetEffectMidiSource {
        strip,
        effect,
        source: Some(keys),
    });
    let by_notes = rendered(&mut app);
    assert!(
        diff(&by_notes, &no_notes) > 0.01,
        "the keyboard's notes play the carrier: {}",
        diff(&by_notes, &no_notes)
    );

    // An edit heard, undone to the bit.
    set(&mut app, "formant", 5.0);
    let moved = rendered(&mut app);
    assert!(diff(&by_notes, &moved) > 0.001, "the formants moved are heard");
    app.undo();
    assert_eq!(rendered(&mut app), by_notes, "undone, to the bit");

    // The object the voice speaks into is a look: the talk-box's tube changes no sample.
    set(&mut app, "obj", 1.0);
    assert_eq!(rendered(&mut app), by_notes, "the object is never heard");
    set(&mut app, "obj", 0.0);

    // The sidechain as the carrier: no key, nothing to speak through; the pad's strip as
    // the slot's key, the voice speaks through the pad.
    app.exec(Command::SetEffectMidiSource {
        strip,
        effect,
        source: None,
    });
    set(&mut app, "src", 0.0);
    let no_key = rendered(&mut app);
    app.exec(Command::SetEffectSidechain {
        strip,
        effect,
        source: Some(pad),
    });
    let keyed = rendered(&mut app);
    assert!(
        diff(&keyed, &no_key) > 0.01,
        "the pad carries the voice: {}",
        diff(&keyed, &no_key)
    );

    // Its neutral defaults: the dry master, to the bit (the key still routed).
    let defaults: Vec<Command> = EffectKind::Vocoder
        .params()
        .iter()
        .map(|i| Command::SetEffectParam {
            strip,
            effect,
            key: i.key.into(),
            value: i.default,
        })
        .collect();
    app.exec(Command::Batch {
        label_key: "cmd.fx_preset".into(),
        commands: defaults,
    });
    assert_eq!(rendered(&mut app), dry, "the null, to the bit");
}
