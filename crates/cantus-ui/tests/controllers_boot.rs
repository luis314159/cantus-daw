//! MIDI controllers on the boot path (round 29, ADR 0062): the desktop app goes `run` →
//! `boot::Boot` → `CantusApp::assemble`, never `CantusApp::new`. The global controller map
//! is read in the Files stage (from beside the keymap) and kept by `assemble` with its
//! file; the MIDI tap that wakes the window is the one the boot opened the ports with (or,
//! without ports, one `assemble` makes) and the one rescans reopen them with. Its own test
//! binary, one test: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::controller::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

#[test]
fn the_boot_path_reads_the_global_map_and_wires_a_waking_tap() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    reads_the_global_map();
    opens_the_ports_with_the_tap_assemble_keeps();
    a_lossy_global_file_is_kept_and_said();
    an_unreadable_global_file_is_never_saved_over();
}

/// A dummy app on the boot path from `dir`'s files.
fn boot(dir: &std::path::Path) -> CantusApp {
    let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
    CantusApp::assemble(
        &egui::Context::default(),
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    )
}

/// Review 4 of ADR 0062, #42: a global file that loses a mapping is kept beside, the start
/// says where, and the session saves the map over the file (what it read is safe).
fn a_lossy_global_file_is_kept_and_said() {
    let dir = tempfile::tempdir().unwrap();
    let path = cantus_ui::controllers::global_path(dir.path());
    let good = serde_json::to_value(Mapping::new(
        MappingId(1),
        MidiSource {
            port: PortRef::any(),
            channel: None,
            control: Control::Cc(41),
        },
        MappingTarget::Action("play-pause".into()),
        MapMode::Trigger,
    ))
    .unwrap();
    let text = serde_json::json!({
        "version": 1,
        "mappings": [good, {"id": 2, "target": {"FromTheFuture": 3}}],
    })
    .to_string();
    std::fs::write(&path, &text).unwrap();
    let app = boot(dir.path());
    assert_eq!(
        app.controllers.global.mappings.len(),
        1,
        "what could be read"
    );
    assert_eq!(app.controllers.global_path.as_deref(), Some(path.as_path()));
    assert!(app.status.error, "said");
    let kept = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .find(|n| n.ends_with(".bad"))
        .expect("kept beside");
    assert!(
        app.status.text.contains(&kept),
        "names {kept}: {}",
        app.status.text
    );
}

/// #42, #28: a global file that can be neither read nor kept (a folder where it should
/// be): never saved over, and every global edit says it is not kept.
fn an_unreadable_global_file_is_never_saved_over() {
    let dir = tempfile::tempdir().unwrap();
    let path = cantus_ui::controllers::global_path(dir.path());
    std::fs::create_dir_all(&path).unwrap();
    let mut app = boot(dir.path());
    assert_eq!(app.controllers.global_path, None, "never saved over");
    app.set_status("something else said since".into());
    app.controllers.global.mappings.push(Mapping::new(
        MappingId(5),
        MidiSource {
            port: PortRef::any(),
            channel: None,
            control: Control::Cc(42),
        },
        MappingTarget::Action("stop".into()),
        MapMode::Trigger,
    ));
    app.store_global_map();
    assert!(path.is_dir(), "nothing written over it");
    assert!(app.status.error, "the edit says it is not kept");
    assert!(
        app.status.text.contains(&path.display().to_string()),
        "{}",
        app.status.text
    );
}

fn reads_the_global_map() {
    let dir = tempfile::tempdir().unwrap();
    let path = cantus_ui::controllers::global_path(dir.path());
    let mut map = ControllerMap::default();
    map.mappings.push(Mapping::new(
        MappingId(1),
        MidiSource {
            port: PortRef::any(),
            channel: None,
            control: Control::Cc(41),
        },
        MappingTarget::Action("play-pause".into()),
        MapMode::Trigger,
    ));
    cantus_ui::controllers::save_global(&path, &map).unwrap();
    let files = Files::read(dir.path(), Some(cantus_ui::i18n::Locale::En));
    assert_eq!(files.controllers, map, "read in the Files stage");
    assert_eq!(files.controllers_path, path);
    let ctx = egui::Context::default();
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        None,
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert_eq!(app.controllers.global, map);
    assert_eq!(app.controllers.global_path.as_deref(), Some(path.as_path()));
    assert!(
        app.controllers.tap.wakes(),
        "a mapped knob wakes the window"
    );
    // A mapping learnt now is written where the next start reads it.
    app.controllers.global.mappings.clear();
    app.store_global_map();
    assert!(cantus_ui::controllers::load_global(&path).is_empty());
}

fn opens_the_ports_with_the_tap_assemble_keeps() {
    // The boot's Stage::Audio opens the ports with a waking tap (`boot.rs`); `assemble`
    // keeps that same tap, so the filter it publishes is the one the callbacks read.
    let ctx = egui::Context::default();
    let tap = cantus_ui::controllers::waking_tap(&ctx);
    let (p, _c) = rtrb::RingBuffer::<cantus_engine::MidiEvent>::new(4);
    let inputs =
        cantus_engine::MidiInputs::open_with(cantus_engine::MidiSender::new(p), tap.clone());
    let dir = tempfile::tempdir().unwrap();
    let files = Files::read(dir.path(), Some(cantus_ui::i18n::Locale::En));
    let app = CantusApp::assemble(
        &ctx,
        files,
        None,
        EngineHandle::dummy(),
        Some(inputs),
        None,
        None,
        false,
    );
    assert!(app.controllers.tap.same(&tap));
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/boot.rs")).unwrap();
    assert!(
        src.contains("MidiInputs::open_with(sender, tap)"),
        "the boot opens the ports with the controller tap"
    );
    // restart_audio and rescan_midi reopen the ports with the same tap.
    let app_src =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/app.rs")).unwrap();
    assert_eq!(app_src.matches("self.controllers.tap.clone()").count(), 2);
}
