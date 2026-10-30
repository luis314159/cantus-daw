//! Sync on the boot path (round 33, ADR 0078–0079): the shipped app goes `run` → `boot` →
//! `CantusApp::assemble`, never `CantusApp::new`; `assemble` starts the sync service on the
//! engine's seam, and an audio restart hands it the new engine's. Its own test binary: it
//! points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_engine::transport::sync::SourceKind;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::views::transport::sync_service::{self as svc, SyncSource};
use cantus_ui::CantusApp;
use std::time::{Duration, Instant};

#[test]
fn assemble_starts_sync_on_the_engines_seam() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    let dir = tempfile::tempdir().unwrap();
    let files = Files::read(dir.path(), Some(cantus_ui::i18n::Locale::En));
    let (engine, _processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::assemble(
        &egui::Context::default(),
        files,
        None,
        engine,
        None,
        None,
        None,
        false,
    );
    assert!(app.sync_service().running(), "the service's thread runs");
    let shared = app
        .sync_service()
        .shared()
        .cloned()
        .expect("the engine's seam");
    // A source chosen (a port's clock; no Link: no socket in a test): the service
    // publishes it to the engine within a few milliseconds.
    app.settings.sync.source = SyncSource::Clock;
    svc::apply(&mut app);
    let t = Instant::now();
    loop {
        if shared.timeline.read().map(|tl| tl.kind) == Some(SourceKind::Clock) {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(2), "never published");
        std::thread::sleep(Duration::from_millis(5));
    }
    app.settings.sync.source = SyncSource::None;
    svc::apply(&mut app);
    // The restart path hands the service the new engine's seam.
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/app.rs")).unwrap();
    assert!(
        src.contains("sync_service::wire(&mut app)"),
        "assemble wires sync"
    );
    assert!(
        src.contains("sync_service::engine_replaced(self)"),
        "restart_audio hands sync the new engine"
    );
    assert!(
        src.contains("sync_service::apply(self)"),
        "a rescan reapplies the ports"
    );
}

/// The palette's *Toggle Ableton Link* (keymap 14) reaches sync's switch. Read from the
/// source, not run: turning Link on in a test would open the network (the owner's firewall
/// rule: tests stay on the loopback).
#[test]
fn the_toggle_link_action_turns_link_on_and_off() {
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/views/session_keys.rs"
    ))
    .unwrap();
    assert!(
        src.contains(
            "Action::ToggleLink => crate::views::transport::sync_service::toggle_link(app)"
        ),
        "ToggleLink calls sync_service::toggle_link"
    );
}
