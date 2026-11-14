//! Fragor end to end (ADR 0131), on the boot path: an app made by `CantusApp::assemble`
//! with the real engine handle; Fragor inserted on a playing track's strip the way the
//! picker inserts it (its starting preset: the green overdrive on the board); the song
//! rendered offline through the engine's compile — the path export, freeze and stems take —
//! and the overdrive heard in the master, clipping (12 dB more in is far less out); a knob turned through the editor's host (one
//! command), a pedal moved to another slot (one step) and undone to the bit.

use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::views::fx::host::FxHost as _;
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

fn rms_db(x: &[f32]) -> f64 {
    let e: f64 = x.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / x.len().max(1) as f64;
    10.0 * e.max(1e-30).log10()
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

#[test]
fn fragor_on_a_playing_strip_distorts_the_master_and_undoes() {
    let (mut app, _proc) = assembled();
    let mut p = Project::new("Fragor");
    let mut track = p.make_track("Lead");
    let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    for (i, pitch) in [52u8, 55, 57, 59, 52, 55, 57, 59].iter().enumerate() {
        clip.notes
            .push(p.make_note(Ticks::beats(i as i64), Ticks::beats(1), *pitch, 112));
    }
    track.clips.push(clip);
    p.tracks.push(track);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let dry = rendered(&mut app);
    assert!(dry.iter().any(|v| v.abs() > 1e-3), "the track plays");

    // Inserted as the picker inserts it: the starting preset puts the overdrive on.
    app.add_effect(strip, EffectKind::Pedal);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(spec.get("s1"), 1.0, "the green overdrive on the board");
    let driven = rendered(&mut app);
    assert!(!same(&dry, &driven), "heard in the master");
    // A circuit that clips: 12 dB more into the pedal is far less than 12 dB more out of it
    // (a linear path, the master's included, would pass all twelve).
    assert!(
        app.project().master.effects.is_empty(),
        "nothing else on the path bends"
    );
    let mut host = cantus_ui::views::fx::host::DawHost::new(&mut app, strip, effect, "fragor");
    host.set("in", 12.0, None);
    drop(host);
    let pushed = rendered(&mut app);
    let rise = rms_db(&pushed) - rms_db(&driven);
    assert!(
        rise < 8.0,
        "the overdrive compresses: +12 dB in, {rise:+.1} dB out"
    );
    app.undo();
    assert!(
        same(&driven, &rendered(&mut app)),
        "the input undone to the bit"
    );

    // A knob turned through the editor's host, as the drag does: one command.
    let mut host = cantus_ui::views::fx::host::DawHost::new(&mut app, strip, effect, "fragor");
    host.set("od.drive", 1.0, None);
    drop(host);
    assert_eq!(
        app.project()
            .effect(strip, effect)
            .unwrap()
            .spec
            .get("od.drive"),
        1.0
    );
    let harder = rendered(&mut app);
    assert!(!same(&driven, &harder));

    // A second pedal added, then the two swapped by a drag: one step each.
    let mut host = cantus_ui::views::fx::host::DawHost::new(&mut app, strip, effect, "fragor");
    host.set_many(&[("s2", 4.0), ("ge.on", 1.0)], "fragor.cmd.add");
    drop(host);
    let chained = rendered(&mut app);
    let mut host = cantus_ui::views::fx::host::DawHost::new(&mut app, strip, effect, "fragor");
    host.set_many(&[("s1", 4.0), ("s2", 1.0)], "fragor.cmd.move");
    drop(host);
    let swapped = rendered(&mut app);
    assert!(!same(&chained, &swapped), "the order is heard");
    app.undo();
    let spec = &app.project().effect(strip, effect).unwrap().spec;
    assert_eq!(
        (spec.get("s1"), spec.get("s2")),
        (1.0, 4.0),
        "the move undone in one step"
    );
    assert!(same(&chained, &rendered(&mut app)), "and the sound with it");
}
