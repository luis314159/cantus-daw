//! External instruments on the boot path (round 36 review, ADR 0094): the desktop app goes
//! `run` → `boot::Boot` → `CantusApp::assemble`, never `CantusApp::new`. `assemble` starts the
//! MIDI-out thread (`sync_service::wire`); each frame the interface registers every enabled
//! instrument's route with that very thread — without it every stamped note is dropped —
//! and a ping registers its unit's route while it runs, even for a unit that is off. Its own
//! test binary, one test: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::external_unit::{route_for, Now, Present, UnitKind};
use cantus_core::Command;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;

#[test]
fn the_boot_path_registers_the_units_routes_with_the_midi_out_thread() {
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
    assert_eq!(
        app.sync_service().out_unit_routes(),
        Some(vec![]),
        "assemble started the MIDI-out thread, with no route yet"
    );
    app.external.present_override = Some(Present {
        device: Some("Interface".into()),
        in_channels: 8,
        out_channels: 8,
        midi_outputs: vec!["Hardware Synth".into()],
    });
    app.external.now_override = Some(Now {
        device: "Interface".into(),
        rtt: 600,
        buffer: 256,
        rate: 48_000,
    });
    let mut p = app.project().clone();
    let t = p.make_track("Synth");
    let track = t.id;
    app.bus.reserve_ids(p.next_id);
    assert!(app.exec(Command::InsertTrack {
        index: 0,
        track: Box::new(t),
    }));
    assert!(cantus_ui::external_service::add_unit(
        &mut app,
        track,
        UnitKind::Instrument
    ));
    let u = app.project().external_units[0].clone();
    let route = route_for(u.id, u.midi.as_ref().unwrap());
    // A frame: the route reaches the running thread.
    app.controller_input();
    assert_eq!(
        app.sync_service().out_unit_routes(),
        Some(vec![(route, "Hardware Synth".to_string(), 0)])
    );
    // Switched off: its route leaves the table.
    let mut off = u.clone();
    off.enabled = false;
    app.exec(Command::SetExternalUnit {
        unit: Box::new(off),
    });
    app.controller_input();
    assert_eq!(app.sync_service().out_unit_routes(), Some(vec![]));
    // Pinged while off: its own route is registered for the ping (never another's, never
    // route 1).
    cantus_ui::external_service::start_ping(&mut app, u.id).expect("the ping starts");
    app.controller_input();
    assert_eq!(
        app.sync_service().out_unit_routes(),
        Some(vec![(route, "Hardware Synth".to_string(), 0)])
    );
    // An audio restart gives up the ping, and says so.
    cantus_ui::external_service::engine_replaced(&mut app);
    assert!(!app.external.is_pinging());
}
