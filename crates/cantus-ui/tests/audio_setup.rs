//! Audio set up directly (round 20, BACKLOG §6b): the chip and its popover switch the
//! device in place, each interface's settings are remembered, a lost device keeps the
//! app working and comes back by itself, problems come with their fix, channels have
//! names, and a musician who never opened Cantus is armed on the guitar's input in three
//! clicks. Gestures go through the rects the views publish (`AudioSetup::geo`); nothing
//! here opens the machine's sound card (the setup is `offline`, the engine a dummy).

mod support;

use cantus_core::TrackInput;
use cantus_engine::hotplug::{DeviceList, DeviceWatch};
use cantus_ui::audio_setup::{self as setup, demo, interfaces, problems, Hit, Notice};
use cantus_ui::settings::{AudioSettings, Settings};
use cantus_ui::CantusApp;
use egui::{Pos2, Rect};

/// An app in the demo studio, its files in a temporary folder.
fn studio() -> (CantusApp, tempfile::TempDir) {
    let mut a = support::app(cantus_ui::welcome_project());
    let dir = tempfile::tempdir().unwrap();
    a.settings_path = dir.path().join("settings.json");
    a.layouts_path = dir.path().join("layouts.json");
    demo::setup(&mut a);
    (a, dir)
}

const SCREEN: egui::Vec2 = egui::vec2(1280.0, 720.0);

fn run(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
}

fn click_at(a: &mut CantusApp, ctx: &egui::Context, at: Pos2) {
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    run(a, ctx, vec![egui::Event::PointerMoved(at)]);
    run(a, ctx, vec![button(true)]);
    run(a, ctx, vec![button(false)]);
}

/// Draw a frame, find where `h` was drawn, click its middle.
fn click(a: &mut CantusApp, ctx: &egui::Context, h: Hit) {
    run(a, ctx, vec![]);
    let r = a
        .audio_setup
        .geo
        .rect(&h)
        .unwrap_or_else(|| panic!("{h:?} was not drawn; drawn: {:?}", a.audio_setup.geo.hits));
    click_at(a, ctx, r.center());
}

#[test]
fn endpoints_group_into_interfaces_with_their_asio_driver() {
    let s = demo::scan(true);
    let keys: Vec<&str> = s.interfaces.iter().map(|i| i.key.as_str()).collect();
    assert_eq!(keys[0], demo::UMC, "interfaces first: {keys:?}");
    let umc = s.interface(demo::UMC).unwrap();
    assert_eq!(umc.kind, interfaces::Kind::Interface);
    assert_eq!(umc.outputs, vec![demo::UMC_OUT.to_string()]);
    assert_eq!(umc.inputs, vec![demo::UMC_IN.to_string()]);
    assert_eq!(umc.asio.as_deref(), Some("UMC ASIO Driver"));
    let rt = s.interface("Realtek(R) Audio").unwrap();
    assert_eq!(rt.kind, interfaces::Kind::BuiltIn);
    assert_eq!(rt.asio.as_deref(), Some("Realtek ASIO"));
    assert_eq!(s.interface("WH-1000XM4 Stereo").unwrap().kind, interfaces::Kind::Headphones);
    assert_eq!(
        s.interface("NVIDIA High Definition Audio").unwrap().kind,
        interfaces::Kind::Display
    );
    assert_eq!(interfaces::adapter_of("Line (2- Focusrite USB Audio)"), "Focusrite USB Audio");
    assert_eq!(interfaces::adapter_of("Speakers (Realtek(R) Audio)"), "Realtek(R) Audio");
    assert_eq!(interfaces::adapter_of("ASIO4ALL v2"), "ASIO4ALL v2");
    assert!(interfaces::asio_matches("Focusrite USB ASIO", "Focusrite USB Audio"));
    assert!(!interfaces::asio_matches("UMC ASIO Driver", "Realtek(R) Audio"));
    // The suggestion: ASIO for an interface when the build has it, never for the laptop.
    let with = interfaces::suggest(umc, true);
    assert_eq!(with.driver.as_deref(), Some("ASIO"));
    assert_eq!(with.device.as_deref(), Some("UMC ASIO Driver"));
    assert_eq!(with.buffer_frames, Some(128));
    let without = interfaces::suggest(umc, false);
    assert_eq!(without.driver, None);
    assert_eq!(without.device.as_deref(), Some(demo::UMC_OUT));
    assert_eq!(without.input_device.as_deref(), Some(demo::UMC_IN));
    assert_eq!(interfaces::suggest(rt, true).driver, None, "Realtek's ASIO stalls");
}

#[test]
fn each_interface_remembers_its_settings_through_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    let mut s = Settings::default();
    let umc = AudioSettings {
        driver: Some("ASIO".into()),
        device: Some("UMC ASIO Driver".into()),
        buffer_frames: Some(64),
        sample_rate: Some(96_000),
        record_offset_frames: 31,
        ..Default::default()
    };
    s.audio_setup.remember(demo::UMC, &umc);
    let p = s.audio_setup.profile_mut(demo::UMC);
    p.inputs = vec!["Guitar".into(), "Mic".into()];
    p.cue = Some(2);
    p.cue_db = -6.0;
    s.audio_setup.remember("Realtek(R) Audio", &AudioSettings::default());
    s.audio_setup.setup_done = true;
    s.save(&path).unwrap();
    let back = Settings::load(&path);
    assert_eq!(back, s);
    assert_eq!(back.audio_setup.recall(demo::UMC), Some(&umc));
    assert_eq!(back.audio_setup.profile(demo::UMC).unwrap().input_name(0), Some("Guitar"));
    assert_eq!(back.audio_setup.last.as_deref(), Some("Realtek(R) Audio"));
    // A file from before round 20: nothing remembered, the setup not done.
    std::fs::write(&path, r#"{"audio":{"device":"Speakers"}}"#).unwrap();
    let old = Settings::load(&path);
    assert!(!old.audio_setup.setup_done);
    assert!(old.audio_setup.profiles.is_empty());
}

#[test]
fn the_chip_opens_a_popover_that_switches_the_device_in_place() {
    let (mut a, _dir) = studio();
    let ctx = support::ctx();
    a.attach(&ctx);
    click(&mut a, &ctx, Hit::Chip);
    assert!(a.audio_setup.popover, "a click on the chip opens it");
    // A buffer: applied at once, remembered for the UMC.
    click(&mut a, &ctx, Hit::Buffer(256));
    assert_eq!(a.settings.audio.buffer_frames, Some(256));
    assert_eq!(a.audio_setup.applied.len(), 1, "applied once, in place");
    assert_eq!(
        a.settings.audio_setup.recall(demo::UMC).unwrap().buffer_frames,
        Some(256)
    );
    assert!(a.audio_setup.popover, "it stays open for the next choice");
    // A rate.
    click(&mut a, &ctx, Hit::Rate(Some(44_100)));
    assert_eq!(a.settings.audio.sample_rate, Some(44_100));
    // Another interface, from the list under the field: the laptop's own audio.
    click(&mut a, &ctx, Hit::InterfaceField);
    let row = a
        .audio_setup
        .scan
        .as_ref()
        .unwrap()
        .interfaces
        .iter()
        .position(|i| i.key == "Realtek(R) Audio")
        .unwrap();
    click(&mut a, &ctx, Hit::InterfaceRow(row));
    assert_eq!(a.settings.audio.device.as_deref(), Some(demo::REALTEK_OUT));
    assert_eq!(
        a.settings.audio.input_device.as_deref(),
        Some(demo::REALTEK_IN),
        "the laptop's own microphone, not the UMC's input"
    );
    // Back to the UMC: what was set on it comes back.
    a.audio_setup.demo.as_mut().unwrap().device = demo::REALTEK_OUT.into();
    click(&mut a, &ctx, Hit::InterfaceField);
    click(&mut a, &ctx, Hit::InterfaceRow(0));
    assert_eq!(a.settings.audio.device.as_deref(), Some(demo::UMC_OUT));
    assert_eq!(a.settings.audio.buffer_frames, Some(256));
    assert_eq!(a.settings.audio.sample_rate, Some(44_100));
    // Escape closes it.
    run(&mut a, &ctx, vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }]);
    assert!(!a.audio_setup.popover);
}

#[test]
fn each_buffer_shows_its_round_trip_before_it_is_chosen() {
    let live = demo::live_on_umc();
    // 330 frames measured at 128: 74 of overhead; 64 → 202 frames, 1024 → 2122.
    let ms = |b| setup::estimate_ms(&live, b);
    assert!((ms(64) - 202.0 / 48.0).abs() < 1e-9);
    assert!((ms(128) - 330.0 / 48.0).abs() < 1e-9, "the running size reads as measured");
    assert!((ms(1024) - 2122.0 / 48.0).abs() < 1e-9);
    assert!(ms(64) < ms(128) && ms(128) < ms(256));
}
