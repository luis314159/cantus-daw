//! External gear and control surfaces on the interface (round 36, ADR 0094, 0095), headless:
//! a virtual Mackie Control drives the mixer through ordinary commands, a surface's port is
//! taken whole, a unit is added in one undo step, a ping is refused with nothing open, and
//! the agents' tools are in the registry.

use cantus_core::external_emulation::VirtualMcu;
use cantus_core::external_surface::to14;
use cantus_core::external_unit::UnitKind;
use cantus_engine::midi::Heard;
use cantus_engine::{EngineHandle, MidiEvent};
use cantus_ui::surface::service::{self, Unit};
use cantus_ui::surface::SurfaceChoice;
use cantus_ui::CantusApp;

fn app() -> CantusApp {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = CantusApp::from_parts(
        Some(cantus_ui::starter_project()),
        EngineHandle::dummy(),
        false,
    );
    app.settings_path = tmp.path().join("settings.json");
    // The folder stays for the test's life (the settings file is written there).
    std::mem::forget(tmp);
    app
}

fn virtual_mcu(app: &mut CantusApp) {
    app.settings.surfaces = vec![SurfaceChoice {
        kind: "mcu".into(),
        virtual_unit: true,
        ..Default::default()
    }];
    service::tick(app);
}

fn unit(app: &mut CantusApp) -> &mut VirtualMcu {
    match app.surfaces.units[0].as_mut() {
        Some(Unit::Mcu(u)) => u,
        _ => panic!("a virtual Mackie Control"),
    }
}

fn hand(app: &mut CantusApp, msgs: Vec<Vec<u8>>) {
    for m in msgs {
        service::feed(app, 0, &m);
    }
}

#[test]
fn a_virtual_mackie_shows_the_mixer_and_drives_it_through_commands() {
    let mut app = app();
    app.exec(cantus_core::Command::SetStripGain {
        id: app.project().strips[1].id,
        gain_db: -6.0,
    });
    virtual_mcu(&mut app);
    // It shows the mixer: strip 2's fader at -6 dB, in the console scale.
    assert_eq!(
        unit(&mut app).faders[1],
        cantus_core::external_surface::fader_value(-6.0)
    );
    assert!(
        unit(&mut app).lcd.iter().any(|c| *c != b' '),
        "names on the scribble strips"
    );
    // A hand moves fader 2 to 0 dB and lets go: one gain, one undo step.
    let before = app.project().clone();
    let undo_before = app.bus.undo_label_key().map(str::to_string);
    let m = unit(&mut app).move_fader(1, to14(0.8), true);
    hand(&mut app, m);
    assert_eq!(app.project().strips[1].gain_db, 0.0);
    app.bus.undo().expect("one undo step");
    assert_eq!(app.project().strips[1].gain_db, before.strips[1].gain_db);
    assert_eq!(
        app.bus.undo_label_key().map(str::to_string),
        undo_before,
        "the whole move was a single step"
    );
    // Mute, solo and Cycle through the buttons.
    let p = unit(&mut app).press(0x10 + 2, true);
    hand(&mut app, p);
    assert!(app.project().strips[2].muted);
    let p = unit(&mut app).press(0x08 + 3, true);
    hand(&mut app, p);
    assert!(app.project().strips[3].soloed);
    let was = app.project().loop_enabled;
    let p = unit(&mut app).press(0x56, true);
    hand(&mut app, p);
    assert_eq!(app.project().loop_enabled, !was);
    // The mixer moves; the unit follows on the next frame (its LED).
    service::tick(&mut app);
    assert_eq!(unit(&mut app).leds[0x10 + 2], 0x7F);
}

#[test]
fn a_surfaces_port_is_taken_whole_and_the_rest_are_left_alone() {
    let mut app = app();
    app.midi_port_names = vec!["Console".into(), "Keyboard".into()];
    app.settings.surfaces = vec![SurfaceChoice {
        kind: "mcu".into(),
        input: "Console".into(),
        ..Default::default()
    }];
    service::tick(&mut app);
    assert_eq!(service::taken_ports(&app), vec![0]);
    let at = std::time::Instant::now();
    let play = MidiEvent {
        status: 0x90,
        data1: 0x56, // cycle
        data2: 0x7F,
    };
    let was = app.project().loop_enabled;
    assert!(service::heard(
        &mut app,
        &Heard {
            port: 0,
            event: play,
            at
        }
    ));
    assert_eq!(app.project().loop_enabled, !was, "the surface's button ran");
    assert!(
        !service::heard(
            &mut app,
            &Heard {
                port: 1,
                event: play,
                at
            }
        ),
        "another port's notes are the generic map's and the engine's"
    );
    // A surface switched off takes nothing.
    app.settings.surfaces[0].enabled = false;
    service::tick(&mut app);
    assert!(service::taken_ports(&app).is_empty());
}

#[test]
fn a_unit_is_added_with_its_input_in_one_undo_step_and_a_ping_needs_a_device() {
    let mut app = app();
    let track = app.project().tracks[0].id;
    let before = app.project().clone();
    assert!(cantus_ui::external_service::add_unit(
        &mut app,
        track,
        UnitKind::Instrument
    ));
    let u = app.project().external_units[0].clone();
    assert_eq!(u.track, track);
    let input = app.project().track(track).unwrap().input.clone().unwrap();
    assert_eq!(input.first, u.ret.first);
    assert_eq!(input.monitor, cantus_core::audio::MonitorMode::On);
    // No real audio device in a headless app (a dummy with no inputs): it is said, never
    // pinged.
    let why = cantus_ui::external_service::start_ping(&mut app, u.id).unwrap_err();
    assert!(
        matches!(why, "external.state.no_device" | "external.state.channels"),
        "{why}"
    );
    assert!(!app.external.is_pinging());
    // One undo step takes both back.
    app.bus.undo().unwrap();
    let mut back = app.project().clone();
    back.next_id = before.next_id;
    assert_eq!(back.external_units, before.external_units);
    assert_eq!(
        back.track(track).unwrap().input,
        before.track(track).unwrap().input
    );
    // A second unit of the same kind is refused.
    assert!(cantus_ui::external_service::add_unit(
        &mut app,
        track,
        UnitKind::Instrument
    ));
    assert!(!cantus_ui::external_service::add_unit(
        &mut app,
        track,
        UnitKind::Instrument
    ));
}

#[test]
fn the_agents_tools_are_in_the_registry() {
    let names: Vec<&str> = cantus_ui::ai_tools::registry()
        .iter()
        .map(|t| t.name)
        .collect();
    for n in [
        "add_external_unit",
        "ping_external_unit",
        "list_external_units",
        "list_surfaces",
    ] {
        assert!(names.contains(&n), "{n}");
    }
}

#[test]
fn removing_a_unit_gives_the_track_the_input_it_had() {
    let mut app = app();
    let track = app.project().tracks[0].id;
    let before = cantus_core::audio::TrackInput::mono(3);
    app.exec(cantus_core::Command::SetTrackInput {
        id: track,
        input: Some(before.clone()),
    });
    assert!(cantus_ui::external_service::add_unit(
        &mut app,
        track,
        UnitKind::Instrument
    ));
    let id = app.project().external_units[0].id;
    assert_ne!(
        app.project().track(track).unwrap().input,
        Some(before.clone())
    );
    assert!(cantus_ui::external_service::remove_unit(&mut app, id));
    assert!(app.project().external_units.is_empty());
    assert_eq!(app.project().track(track).unwrap().input, Some(before));
    // One undo step brings the unit and its input back.
    app.bus.undo().unwrap();
    assert_eq!(app.project().external_units.len(), 1);
}

#[test]
fn export_and_bounce_say_why_they_cannot_render_outboard_gear() {
    let mut app = app();
    let track = app.project().tracks[0].id;
    assert!(cantus_ui::external_service::export_refusal(&app).is_none());
    assert!(cantus_ui::external_service::add_unit(
        &mut app,
        track,
        UnitKind::Instrument
    ));
    let why = cantus_ui::external_service::export_refusal(&app).expect("refused");
    assert!(why.contains(&app.project().track(track).unwrap().name));
    assert!(!cantus_ui::views::export::start_with(
        &mut app,
        &std::env::temp_dir().join("cantus-external-refused.wav"),
        None
    ));
    // Switched off, the gear is no part of the song: the refusal goes.
    let mut u = app.project().external_units[0].clone();
    u.enabled = false;
    app.exec(cantus_core::Command::SetExternalUnit { unit: Box::new(u) });
    assert!(cantus_ui::external_service::export_refusal(&app).is_none());
}

#[test]
fn a_mackie_connection_query_from_its_port_is_answered_through_the_service() {
    let mut app = app();
    app.midi_port_names = vec!["MCU".into()];
    app.settings.surfaces = vec![SurfaceChoice {
        kind: "mcu".into(),
        input: "MCU".into(),
        ..Default::default()
    }];
    service::tick(&mut app);
    // TouchMCU's Host Connection Query: F0 00 00 66 14 01, serial, challenge, F7.
    let query = [
        0xF0, 0x00, 0x00, 0x66, 0x14, 0x01, b'S', b'E', b'R', b'I', b'A', b'L', b'7', 0x01, 0x02,
        0x03, 0x04, 0xF7,
    ];
    service::sysex(&mut app, 0, &query);
    assert!(app
        .surfaces
        .log
        .iter()
        .any(|l| l.starts_with("out F0 00 00 66 14 02 53 45 52 49 41 4C 37")));
}

#[test]
fn every_user_facing_offline_render_refuses_outboard_gear_and_listening_says_it() {
    let mut app = app();
    let track = app.project().tracks[0].id;
    let name = app.project().track(track).unwrap().name.clone();
    assert!(cantus_ui::external_service::outboard_tracks(app.project()).is_empty());
    assert!(cantus_ui::external_service::add_unit(
        &mut app,
        track,
        UnitKind::Instrument
    ));
    // The video export.
    let d = app.settings.video.clone();
    let err = cantus_ui::video_export::start(
        &mut app,
        &std::env::temp_dir().join("cantus-external-refused.mp4"),
        &d,
        false,
    )
    .unwrap_err();
    assert!(err.contains(&name), "{err}");
    // The modulator bake's words.
    let bake = cantus_ui::external_service::render_refusal(&app, "external.cannot_bake").unwrap();
    assert!(bake.contains(&name));
    // An AI listen renders without the gear and names what it left out.
    assert_eq!(
        cantus_ui::external_service::outboard_tracks(app.project()),
        vec![name]
    );
}

/// Two faders ridden at once, their messages interleaved, one let go before the other: one
/// undo step for the whole ride (first touch to last release), both strips back with it.
#[test]
fn two_faders_ridden_at_once_are_one_undo_step() {
    let mut app = app();
    virtual_mcu(&mut app);
    let (a, b) = (
        app.project().strips[0].clone(),
        app.project().strips[1].clone(),
    );
    let before = app.bus.undo_label_key().map(str::to_string);
    let touch = |ch: u8, on: bool| vec![0x90, 0x68 + ch, if on { 0x7F } else { 0 }];
    let fader = |ch: u8, pos: f32| cantus_core::external_mcu::fader(ch, to14(pos));
    hand(&mut app, vec![touch(0, true), touch(1, true)]);
    for k in 0..10 {
        let p = 0.3 + k as f32 * 0.03;
        hand(&mut app, vec![fader(0, p), fader(1, 1.0 - p)]);
    }
    hand(&mut app, vec![touch(0, false)]);
    hand(&mut app, vec![fader(1, 0.2)]);
    hand(&mut app, vec![touch(1, false)]);
    assert_ne!(app.project().strips[0].gain_db, a.gain_db);
    assert_ne!(app.project().strips[1].gain_db, b.gain_db);
    app.bus.undo().expect("one step");
    assert_eq!(app.project().strips[0].gain_db, a.gain_db);
    assert_eq!(app.project().strips[1].gain_db, b.gain_db);
    assert_eq!(
        app.bus.undo_label_key().map(str::to_string),
        before,
        "the whole ride was one step"
    );
    // The next touch is a step of its own.
    hand(
        &mut app,
        vec![touch(0, true), fader(0, 0.9), touch(0, false)],
    );
    app.bus.undo().unwrap();
    assert_eq!(app.project().strips[0].gain_db, a.gain_db);
}

/// In the recording mode a fader the lock refuses does not move (and says nothing each
/// message); another the lock allows keeps moving in the same gesture.
#[test]
fn a_locked_target_is_skipped_quietly_and_the_others_keep_moving() {
    let mut app = app();
    virtual_mcu(&mut app);
    let (a, b) = (
        app.project().strips[0].clone(),
        app.project().strips[1].clone(),
    );
    let mut live = std::collections::BTreeSet::new();
    live.insert(b.id);
    app.bus
        .set_record_lock(Some(cantus_core::record_mode::EditLock {
            mix_live: false,
            master_live: true,
            live_tracks: Default::default(),
            live_strips: live,
        }));
    let probe = cantus_core::Command::SetStripGain {
        id: a.id,
        gain_db: -1.0,
    };
    assert!(app.bus.locked_against(&probe), "the lock refuses strip 1");
    let errors_before = (app.status.error, app.status.text.clone());
    let touch = |ch: u8, on: bool| vec![0x90, 0x68 + ch, if on { 0x7F } else { 0 }];
    let fader = |ch: u8, pos: f32| cantus_core::external_mcu::fader(ch, to14(pos));
    hand(&mut app, vec![touch(0, true), touch(1, true)]);
    for k in 0..5 {
        hand(
            &mut app,
            vec![
                fader(0, 0.2 + k as f32 * 0.1),
                fader(1, 0.3 + k as f32 * 0.1),
            ],
        );
    }
    hand(&mut app, vec![touch(0, false), touch(1, false)]);
    assert_eq!(
        app.project().strips[0].gain_db,
        a.gain_db,
        "locked: unmoved"
    );
    assert_ne!(app.project().strips[1].gain_db, b.gain_db, "live: moved");
    assert_eq!(
        (app.status.error, app.status.text.clone()),
        errors_before,
        "no error spam"
    );
}
