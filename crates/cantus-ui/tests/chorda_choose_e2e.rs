//! Choosing Chorda never blocks a frame (round 39, fixes; BACKLOG §8): model 2's body —
//! two radiation responses, 16 ms to synthesise at a rate not yet made — is made on the
//! designer's worker beside the keyboard, never on the interface's thread when the unit is
//! made there, and the piano plays once they are made. Through `CantusApp::assemble`, the
//! instrument picker and the real engine wiring (a processor driven block by block).
//!
//! A binary of its own: nothing else has made a body at its rate first.

use cantus_core::InstrumentKind;
use cantus_dsp::chorda::body;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;

#[test]
fn choosing_chorda_makes_nothing_heavy_on_the_interface_thread() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    std::fs::write(
        data.path().join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    // A rate nothing else in this binary uses.
    let fs = 44_100.0;
    let (engine, mut proc_) = EngineHandle::manual(fs);
    let song = cantus_core::Project::new("piano");
    let mut app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    assert!(!body::is_warm(fs), "no body made at {fs} Hz yet");
    let mut t = 0.0;
    let mut out = vec![0f32; 800 * 2];
    let mut frame = |app: &mut CantusApp, proc_: &mut cantus_engine::Processor| {
        proc_.process(&mut out, 2);
        t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            time: Some(t),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| app.frame(ui)).textures_delta.clear();
        out.iter().fold(0.0f32, |m, x| m.max(x.abs()))
    };
    frame(&mut app, &mut proc_);
    // The picker opens (the designer starts on the keyboard and the body), Chorda is chosen
    // at once, and the frames go on: the unit is made on this thread, its body never.
    let purpose = PickPurpose::AddInstrument { index: None };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Instrument(InstrumentKind::Chorda));
    app.picker = None;
    let track = app.project().tracks.last().unwrap().id;
    assert!(app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .is_some_and(|n| n.kind == InstrumentKind::Chorda));
    for _ in 0..3 {
        frame(&mut app, &mut proc_);
    }
    assert_eq!(
        body::made_on_this_thread(),
        0,
        "the body's responses were synthesised on the interface's thread"
    );
    // Once the designer has made them (and the keyboard), the piano plays.
    let since = std::time::Instant::now();
    while !cantus_engine::inst::chorda::lookup(cantus_engine::inst::chorda::key_of(
        Default::default(),
        fs as f32,
    ))
    .is_some()
    {
        assert!(since.elapsed().as_secs() < 120, "the keyboard is designed");
        frame(&mut app, &mut proc_);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(body::is_warm(fs), "the designer made the body");
    app.engine.note_on(track, 60, 100);
    let mut peak = 0.0f32;
    for _ in 0..30 {
        peak = peak.max(frame(&mut app, &mut proc_));
    }
    assert!(peak > 1e-4, "the piano plays: {peak}");
    assert_eq!(body::made_on_this_thread(), 0);
}
