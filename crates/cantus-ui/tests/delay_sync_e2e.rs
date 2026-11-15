//! Delay's synced time end to end (round 40, ADR 0119), on the boot path: an app made by
//! `CantusApp::assemble` with the real engine handle; Delay inserted the way the picker
//! inserts it (its starting preset); the sync turned on the way its window's switch turns it
//! (`views::fx::delay::set_sync`, through the window's host: one command, one undo step);
//! the song rendered offline through the engine's compile — the path export, freeze and
//! stems take — across a tempo change: the echo is the note value at each tempo, the free
//! time is not; and undone, the song renders as before, to the bit. Its own test binary: it
//! points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::tempo::TempoPoint;
use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::views::fx::host::DawHost;
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
            end: Some(Ticks::beats(10)),
            tail_seconds: 0.5,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

/// The first frame from `from` seconds on where the left side passes `level`.
fn onset(x: &[f32], from: f64, level: f32) -> Option<f64> {
    let a = (from * 48_000.0) as usize;
    (a..x.len() / 2)
        .find(|i| x[i * 2].abs() > level)
        .map(|i| i as f64 / 48_000.0)
}

fn set(app: &mut CantusApp, strip: StripId, effect: EffectId, key: &str, value: f32) {
    app.exec(Command::SetEffectParam {
        strip,
        effect,
        key: key.into(),
        value,
    });
}

#[test]
fn the_switch_makes_the_echo_follow_a_tempo_change_in_a_render_and_undoes() {
    let (mut app, _proc) = assembled();
    // One short note at the start and one at beat 6; 120 BPM, then 90 from beat 4.
    let mut p = Project::new("Delay sync");
    p.tempo_bpm = 120.0;
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: 120.0,
            ramp: false,
        },
        TempoPoint {
            tick: Ticks::beats(4),
            bpm: 90.0,
            ramp: false,
        },
    ];
    let mut track = p.make_track("Keys");
    let mut clip = p.make_clip("Hits", Ticks::ZERO, Ticks::beats(8));
    for b in [0, 6] {
        clip.notes
            .push(p.make_note(Ticks::beats(b), Ticks::SIXTEENTH, 72, 110));
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);

    app.add_effect(strip, EffectKind::Delay);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    // Only the echoes, one each, at 500 ms (a quarter at 120 BPM) on both sides.
    for (k, v) in [
        ("mix", 1.0),
        ("feedback", 0.0),
        ("ping_pong", 0.0),
        ("tone_hz", 20_000.0),
        ("time_ms", 500.0),
    ] {
        set(&mut app, strip, effect, k, v);
    }
    let free = rendered(&mut app);
    let steps = app.bus.history().len();

    // The window's switch, at the song's tempo where the playhead is (120 BPM).
    cantus_ui::views::fx::delay::set_sync(
        &mut DawHost::new(&mut app, strip, effect, "delay"),
        true,
        120.0,
    );
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(
        (spec.get("sync"), spec.get("note")),
        (1.0, 12.0),
        "a quarter"
    );
    assert_eq!(app.bus.history().len(), steps + 1, "one step");
    let synced = rendered(&mut app);

    let peak = dry.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    let level = peak * 0.05;
    let note = onset(&dry, 3.0, level).expect("the second note");
    assert!(
        (note - (2.0 + 4.0 / 3.0)).abs() < 0.01,
        "beat 6 at {note} s"
    );
    let first = onset(&synced, 0.0, level).unwrap();
    assert!(
        (first - 0.5).abs() < 0.01,
        "a quarter at 120 BPM: {first} s"
    );
    let echo = onset(&synced, 3.0, level).unwrap() - note;
    assert!(
        (echo - 2.0 / 3.0).abs() < 0.005,
        "a quarter at 90 BPM: {echo} s"
    );
    let free_echo = onset(&free, 3.0, level).unwrap() - note;
    assert!(
        (free_echo - 0.5).abs() < 0.005,
        "the free time stays: {free_echo} s"
    );

    // Off again, the time it was playing at 90 BPM kept in milliseconds; then both undone.
    cantus_ui::views::fx::delay::set_sync(
        &mut DawHost::new(&mut app, strip, effect, "delay"),
        false,
        90.0,
    );
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!((spec.get("sync"), spec.get("time_ms")), (0.0, 666.7));
    app.undo();
    app.undo();
    assert_eq!(
        app.project()
            .effect(strip, effect)
            .unwrap()
            .spec
            .get("sync"),
        0.0
    );
    assert_eq!(
        rendered(&mut app),
        free,
        "undone, the free time renders as before"
    );
}
