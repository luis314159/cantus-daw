//! The first run, channel names, routing and the chip's lamp (round 20). The bar
//! (BACKLOG §6b): a musician who never opened Cantus records a guitar through a new
//! interface in under a minute without opening Settings — here, three clicks from start
//! to an armed track on the input that moved.

mod support;

use cantus_core::TrackInput;
use cantus_ui::audio_setup::{self as setup, demo, Hit};
use cantus_ui::settings::{AudioSettings, Settings};
use cantus_ui::CantusApp;
use egui::{Pos2, Rect};

fn studio() -> (CantusApp, tempfile::TempDir) {
    let mut a = support::app(cantus_ui::welcome_project());
    let dir = tempfile::tempdir().unwrap();
    a.settings_path = dir.path().join("settings.json");
    a.layouts_path = dir.path().join("layouts.json");
    demo::setup(&mut a);
    (a, dir)
}

fn run(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1280.0, 720.0))),
        events,
        ..Default::default()
    };
    ctx.run_ui(input, |ui| a.frame(ui)).textures_delta.clear();
}

fn click(a: &mut CantusApp, ctx: &egui::Context, h: Hit) {
    run(a, ctx, vec![]);
    let at = a
        .audio_setup
        .geo
        .rect(&h)
        .unwrap_or_else(|| panic!("{h:?} was not drawn; drawn: {:?}", a.audio_setup.geo.hits))
        .center();
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

#[test]
fn a_first_time_musician_is_armed_on_the_guitar_in_three_clicks() {
    let (mut a, _dir) = studio();
    let ctx = support::ctx();
    a.attach(&ctx);
    // A fresh install: nothing set, the setup opens at start.
    a.settings.audio = AudioSettings::default();
    a.settings.audio_setup = Default::default();
    a.audio_setup.demo.as_mut().unwrap().device = demo::REALTEK_OUT.into();
    setup::arm_first_run(&mut a);
    assert!(a.audio_setup.first_run.is_some());
    let tracks = a.project().tracks.len();
    // 1. The UMC's card.
    click(&mut a, &ctx, Hit::Card(0));
    assert_eq!(a.settings.audio.device.as_deref(), Some(demo::UMC_OUT));
    assert_eq!(
        a.settings.audio.buffer_frames,
        Some(if cantus_engine::devices::asio_built() {
            128
        } else {
            256
        }),
        "the suggestion"
    );
    // 2. The test tone.
    a.audio_setup.demo.as_mut().unwrap().device = demo::UMC_OUT.into();
    click(&mut a, &ctx, Hit::FirstTone);
    assert_eq!(a.audio_setup.tone, Some(0));
    // The guitar plays into input 1: the meter moves and the input is found.
    run(&mut a, &ctx, vec![]);
    assert_eq!(a.audio_setup.first_run.as_ref().unwrap().heard, Some(0));
    // 3. Record on it.
    click(&mut a, &ctx, Hit::FirstDone);
    assert!(a.audio_setup.first_run.is_none(), "closed");
    assert_eq!(a.audio_setup.tone, None, "the tone stops with it");
    assert!(
        a.settings.audio_setup.setup_done,
        "it does not open by itself again"
    );
    assert!(!a.show_settings, "Settings never opened");
    assert_eq!(a.project().tracks.len(), tracks + 1);
    let t = a.project().track(a.selected_track.unwrap()).unwrap();
    assert!(t.armed, "armed: the next thing is pressing record");
    assert_eq!(
        t.input.as_ref().map(|i| (i.first, i.stereo)),
        Some((0, false))
    );
    // One undo takes the track away.
    a.undo();
    assert_eq!(a.project().tracks.len(), tracks);
    // Not opened by itself for someone who already set audio up.
    let (mut b, _d) = studio();
    setup::arm_first_run(&mut b);
    assert!(b.audio_setup.first_run.is_none());
    // Nor in an app built for tests (only the real start arms it).
    assert!(support::app(cantus_ui::welcome_project())
        .audio_setup
        .first_run
        .is_none());
}

#[test]
fn channel_names_replace_numbers_in_every_input_menu() {
    let (mut a, _dir) = studio();
    let label = |i: TrackInput| {
        let mut s = String::new();
        cantus_ui::record_session::input_label(&i, &mut s);
        s
    };
    a.settings.audio_setup.profile_mut(demo::UMC).inputs = vec!["Guitar".into(), "Vocal".into()];
    setup::apply_routing(&mut a);
    assert_eq!(label(TrackInput::mono(0)), "Guitar");
    assert_eq!(label(TrackInput::stereo(0)), "Guitar / Vocal");
    a.settings.audio_setup.profile_mut(demo::UMC).inputs = vec!["Keys L".into(), "Keys R".into()];
    setup::apply_routing(&mut a);
    assert_eq!(label(TrackInput::stereo(0)), "Keys");
    a.settings.audio_setup.profile_mut(demo::UMC).inputs.clear();
    setup::apply_routing(&mut a);
    assert_eq!(label(TrackInput::mono(1)), "In 2", "unnamed: the number");
}

#[test]
fn the_routing_page_sends_a_headphone_cue_and_moves_the_main_pair() {
    let (mut a, _dir) = studio();
    let ctx = support::ctx();
    a.attach(&ctx);
    a.audio_setup.routing = true;
    click(&mut a, &ctx, Hit::Cue(Some(2)));
    let p = a.settings.audio_setup.profile(demo::UMC).unwrap();
    assert_eq!(p.cue, Some(2), "outputs 3–4 carry the cue");
    click(&mut a, &ctx, Hit::Main(2));
    assert_eq!(a.settings.audio_setup.profile(demo::UMC).unwrap().main, 2);
    click(&mut a, &ctx, Hit::Cue(None));
    assert_eq!(a.settings.audio_setup.profile(demo::UMC).unwrap().cue, None);
    // Stored: it comes back with the interface.
    let back = Settings::load(&a.settings_path);
    assert_eq!(back.audio_setup.profile(demo::UMC).unwrap().main, 2);
    // Every view draws in both languages and both themes without a missing string.
    for loc in cantus_ui::i18n::Locale::ALL {
        cantus_ui::i18n::default_thread_locale(loc);
        a.audio_setup.popover = true;
        a.audio_setup.first_run = Some(Default::default());
        let text = support::paint(&support::ctx(), egui::vec2(1280.0, 720.0), 3, |ui| {
            a.frame(ui)
        });
        assert!(!text.contains("devices."), "{loc:?}: a key shows: {text}");
        a.audio_setup.first_run = None;
    }
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
}

#[test]
fn the_chip_is_amber_after_a_dropout_and_red_without_a_device() {
    let (mut a, _dir) = studio();
    use cantus_ui::views::audio_chip::{health, Health};
    setup::tick(&mut a);
    assert_eq!(health(&a), Health::Good);
    a.audio_setup.demo.as_mut().unwrap().dropouts = 1;
    setup::tick(&mut a);
    assert_eq!(health(&a), Health::Warn);
    a.audio_setup.demo.as_mut().unwrap().lost = true;
    setup::tick(&mut a);
    assert_eq!(health(&a), Health::Lost);
    // A dummy engine (no device at all) is red too.
    let b = support::app(cantus_ui::welcome_project());
    assert_eq!(health(&b), Health::Lost);
}
