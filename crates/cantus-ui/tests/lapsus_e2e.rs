//! Lapsus end to end (fx side round, ADR 0132 provisional), on the boot path: an app made
//! by `CantusApp::assemble` with the real engine handle; a riff playing on a track; Lapsus
//! inserted on its strip as the picker inserts it (its starting preset, half time every
//! half bar); the song rendered offline through the engine's compile — the path export,
//! freeze and stems take — and the half time heard in the master. Then the pattern changed
//! as the editor changes it (the playing time pattern drawn flat): the master is the dry
//! song again, to the bit; undone, the half time is back, to the bit. A pad of the volume
//! bank (a trance gate) cuts the master in sixteenths. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::fx::lapsus::{self as model, Lane};
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

#[test]
fn lapsus_on_a_strip_shapes_the_master_in_an_offline_render_and_undoes() {
    let (mut app, _proc) = assembled();
    // One track playing a riff of sixteenths for two bars.
    let mut p = Project::new("Lapsus");
    p.tempo_bpm = 120.0;
    let mut track = p.make_track("Synth");
    let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    for i in 0..32 {
        let pitch = [60u8, 63, 67, 70][i % 4];
        clip.notes.push(p.make_note(
            Ticks::SIXTEENTH * i as i64,
            Ticks::SIXTEENTH - Ticks(20),
            pitch,
            100,
        ));
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);
    assert!(dry.iter().any(|v| v.abs() > 0.01), "the riff sounds");

    // Inserted as the picker inserts it: on its starting preset.
    app.add_effect(strip, EffectKind::TimeShaper);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(spec.get("tslot"), 7.0, "half time ½ is the starting preset");
    assert!(
        spec.get("out") > 0.0,
        "levelled as the library levels its presets"
    );
    // Its output at unity, as the editor's knob sets it, so the dry song compares bit for bit.
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: "out".into(),
        value: 0.0,
    });
    let half = rendered(&mut app);
    assert!(half.iter().all(|v| v.is_finite()));
    assert_ne!(half, dry, "half time is heard in the master");
    // Each half bar's first sample is the dry's (the cycle starts live, to the bit), its
    // middle is not (the first beat read at half speed).
    let bar = 96_000usize;
    for at in [0, bar / 2, bar, bar + bar / 2] {
        assert_eq!(
            half[2 * at],
            dry[2 * at],
            "the cycle starts on the music at {at}"
        );
    }
    let mid = &half[2 * (bar / 4)..2 * (bar / 2 - 1_000)];
    let dmid = &dry[2 * (bar / 4)..2 * (bar / 2 - 1_000)];
    assert!(mid.iter().zip(dmid).any(|(a, b)| (a - b).abs() > 0.01));

    // The playing time pattern drawn flat, as the editor draws: the dry song, to the bit.
    app.exec(Command::SetLapsusPattern {
        strip,
        effect,
        lane: Lane::Time,
        slot: 7,
        pattern: Some(Box::new(model::Pattern {
            len: 1,
            points: vec![model::Point::new(0.0, 0.0)],
            name: None,
        })),
    });
    assert_eq!(rendered(&mut app), dry, "a flat time curve is a wire");
    app.undo();
    assert_eq!(
        rendered(&mut app),
        half,
        "undone: half time again, to the bit"
    );

    // A pad of the volume bank: the trance gate cuts the master in sixteenths.
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: "vslot".into(),
        value: 5.0,
    });
    let gated = rendered(&mut app);
    let sixteenth = 6_000usize;
    // The trance gate's second sixteenth is off: silent there (after its glide).
    let off = &gated[2 * (sixteenth + 600)..2 * (2 * sixteenth - 10)];
    assert!(off.iter().all(|v| v.abs() < 1e-4), "the gate is shut");
    app.undo();
    assert_eq!(rendered(&mut app), half);
}
