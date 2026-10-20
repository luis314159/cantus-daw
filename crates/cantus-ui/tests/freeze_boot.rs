//! Freezing on the boot path (round 29, ADR 0064): the desktop app goes `run` →
//! `boot::Boot` → `CantusApp::assemble`, never `CantusApp::new`, so the frozen renders'
//! folder and low-latency monitoring are wired there. Its own test binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder, and nothing else in the process reads it.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

/// The data folder every test here shares (set once: the environment is the process's).
fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

#[test]
fn the_boot_path_points_renders_at_the_data_folder_and_tells_the_engine_low_latency() {
    let data = data();
    let mut files = Files::read(data, Some(cantus_ui::i18n::Locale::En));
    assert!(!files.settings.low_latency, "off on a new installation");
    assert_eq!(
        files.settings.low_latency_ms,
        cantus_ui::settings::DEFAULT_LOW_LATENCY_MS
    );
    files.settings.low_latency = true;
    let ctx = egui::Context::default();
    let (engine, mut proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    assert_eq!(
        app.engine.frozen.app_dir(),
        Some(data.join("cache").join("freeze").as_path())
    );
    let mut buf = vec![0f32; 128 * 2];
    proc.process(&mut buf, 2);
    assert!(
        proc.stats().low_latency.on,
        "the setting reached the engine"
    );
}

/// Round 29, review 2: a frozen project opened at start finds its renders at its first
/// compile — its file is known then — so the plug-ins they replace are never loaded.
#[test]
fn a_frozen_project_opened_at_start_finds_its_renders_at_once() {
    use cantus_core::freeze::FreezeTarget;
    use cantus_core::*;
    let _ = data();
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("song.cantus");
    // Frozen and saved, its render beside it.
    let mut p = cantus_ui::starter_project();
    p.tracks.clear();
    let mut t = p.make_track("Synth");
    let mut c = p.make_clip("Synth", Ticks::ZERO, Ticks::beats(4));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(2), 60, 100));
    t.clips.push(c);
    p.tracks.push(t);
    let strip = p.tracks[0].strip.unwrap();
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(p), engine, false);
    app.path = Some(file.clone());
    let ctx = egui::Context::default();
    app.freeze_target(FreezeTarget::Strip(strip));
    for _ in 0..2000 {
        cantus_ui::freeze::tick(&mut app, &ctx);
        if !app.freeze.busy() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(app.project().frozen(FreezeTarget::Strip(strip)).is_some());
    app.start_save(&file);
    app.poll_save(true);
    assert!(folder.path().join("Freeze").is_dir());
    // Opened at start.
    let project = cantus_project::load(&file, true).unwrap().project;
    let mut files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    files.project_path = Some(file.clone());
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let opened = CantusApp::assemble(&ctx, files, Some(project), engine, None, None, None, false);
    assert_eq!(
        opened.engine.frozen.open_count(),
        1,
        "its render found by the first compile"
    );
}
