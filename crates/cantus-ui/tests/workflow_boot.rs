//! The workflow on the boot path (round 31, ADR 0072): the desktop app goes `run` →
//! `boot::Boot` → `CantusApp::assemble`, never `CantusApp::new`, so the templates' folder,
//! the recent projects and the palette's uses are wired there. Its own test binary: it
//! points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

#[test]
fn the_boot_path_wires_the_templates_the_recent_projects_and_the_palette() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let config = cantus_ui::app::config_dir();
    // What a session before kept: a recent project and a use of the palette.
    let mut recent = cantus_ui::recent::Recent::load(cantus_ui::recent::path(&config));
    let song = data.path().join("song.cantus");
    recent.note(&song);
    let mut uses = cantus_ui::palette_index::Uses::default();
    uses.used("action:undo");
    uses.save(&cantus_ui::palette_index::uses_path(&config))
        .unwrap();
    let files = Files::read(&config, Some(cantus_ui::i18n::Locale::En));
    let app = CantusApp::assemble(
        &egui::Context::default(),
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert_eq!(
        app.workflow.store.root.as_deref(),
        Some(cantus_ui::app::data_dir().join("templates").as_path())
    );
    assert_eq!(app.workflow.recent.projects.len(), 1);
    assert!(app.workflow.recent.projects[0].ends_with("song.cantus"));
    assert_eq!(
        app.workflow.uses.of("action:undo").map(|u| u.count),
        Some(1)
    );
    // A template saved from the assembled app lands in the data folder.
    let path = app
        .workflow
        .store
        .save_project(&cantus_ui::starter_project(), None, "Mine")
        .unwrap();
    assert!(path.starts_with(cantus_ui::app::data_dir().join("templates")));
    app.workflow.store.delete(&path).unwrap();
    // The owner's decision (2026-10-26): the start page opens as Cantus starts — on this
    // path, the shipped one — when no project was given to open.
    assert!(app.workflow.start.opened_at_launch(), "opens at start");
    start_page_at_launch(&config);
}

fn assemble(files: Files, project: Option<cantus_core::Project>) -> CantusApp {
    CantusApp::assemble(
        &egui::Context::default(),
        files,
        project,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    )
}

/// Not over a project given to open (the command line, a file from Explorer, the update's
/// relaunch with one), not with *Show at start* off; a file opened from the system later
/// (Finder, the Dock, a drop) or work restored from a crash closes it; the recovery offer
/// and the audio first-run setup come first (it waits behind them).
fn start_page_at_launch(config: &std::path::Path) {
    // A project given to open.
    let file = config.join("given.cantus");
    cantus_project::save(&cantus_ui::starter_project(), &file, "test").unwrap();
    let mut files = Files::read(config, Some(cantus_ui::i18n::Locale::En));
    files.project_path = Some(file.clone());
    let app = assemble(files, Some(cantus_ui::starter_project()));
    assert!(
        !app.workflow.start.is_open(),
        "not over a project given to open"
    );
    // The switch off.
    let mut files = Files::read(config, Some(cantus_ui::i18n::Locale::En));
    files.settings.start_page_at_start = false;
    let app = assemble(files, None);
    assert!(!app.workflow.start.is_open(), "Show at start off");
    // A file from the system once it is open: the file opens, the page closes.
    let files = Files::read(config, Some(cantus_ui::i18n::Locale::En));
    let mut app = assemble(files, None);
    assert!(app.workflow.start.is_open());
    app.request_open_path(file.clone());
    assert!(!app.workflow.start.is_open(), "a file opened closes it");
    // The audio first-run setup comes first: the page waits, still open behind it.
    let files = Files::read(config, Some(cantus_ui::i18n::Locale::En));
    let mut app = assemble(files, None);
    cantus_ui::audio_setup::open_first_run(&mut app);
    assert!(cantus_ui::workflow::start_page_waits(&app));
    assert!(app.workflow.start.is_open());
}
