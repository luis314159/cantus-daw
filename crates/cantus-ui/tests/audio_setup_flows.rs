//! Audio setup flows (round 20): plug-and-play, problems with their fix, the first run,
//! channel names and routing, the chip's lamp. As in `audio_setup.rs`, gestures go
//! through the published rects and nothing opens the machine's sound card.

mod support;

use cantus_engine::hotplug::{DeviceList, DeviceWatch};
use cantus_ui::audio_setup::{self as setup, demo, problems, Hit, Notice};

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

fn devices(with_umc: bool) -> DeviceList {
    let s = demo::scan(with_umc);
    DeviceList {
        outputs: s.outputs.clone(),
        inputs: s.inputs.iter().map(|i| i.name.clone()).collect(),
        default_output: s.outputs.first().cloned(),
        default_input: None,
    }
}

fn has(a: &CantusApp, n: &Notice) -> bool {
    a.audio_setup.notices.iter().any(|(x, _)| x == n)
}

#[test]
fn a_lost_interface_keeps_the_session_working_and_comes_back_by_itself() {
    let (mut a, _dir) = studio();
    let ctx = support::ctx();
    a.attach(&ctx);
    setup::plug::devices_changed(&mut a, devices(true));
    assert!(
        a.audio_setup.notices.is_empty(),
        "the first list is the baseline"
    );
    a.audio_setup.scan = Some(demo::scan(true));
    setup::plug::devices_changed(&mut a, devices(false));
    assert_eq!(a.audio_setup.plug.lost.as_deref(), Some(demo::UMC));
    assert!(has(&a, &Notice::Lost(demo::UMC.into())));
    use cantus_ui::views::audio_chip::{health, Health};
    assert_eq!(health(&a), Health::Lost, "the chip turns red");
    // The session keeps working: frames draw, edits and undo work.
    let tracks = a.project().tracks.len();
    a.add_audio_track();
    run(&mut a, &ctx, vec![]);
    assert_eq!(a.project().tracks.len(), tracks + 1);
    a.undo();
    assert_eq!(a.project().tracks.len(), tracks);
    let lost = a.audio_setup.plug.lost.clone();
    let found = problems::diagnose(&setup::live(&mut a, false), lost.as_deref(), None, 0);
    assert!(matches!(found[0].0, problems::Problem::Lost { .. }));
    assert!(
        found[0].0.text().contains(demo::UMC),
        "said by name: {}",
        found[0].0.text()
    );
    assert_eq!(found[0].1, vec![problems::Fix::UseDefault]);
    // Plugged back in: opened again with what it had, without asking.
    let before = a.audio_setup.applied.len();
    setup::plug::devices_changed(&mut a, devices(true));
    assert_eq!(a.audio_setup.applied.len(), before + 1);
    assert_eq!(a.settings.audio.device.as_deref(), Some(demo::UMC_OUT));
    assert_eq!(a.audio_setup.plug.lost, None);
    assert!(has(&a, &Notice::Reconnected(demo::UMC.into())));
    assert!(!a
        .audio_setup
        .notices
        .iter()
        .any(|(n, _)| matches!(n, Notice::Lost(_))));
}

#[test]
fn a_new_interface_is_offered_and_one_click_switches_to_it() {
    let (mut a, _dir) = studio();
    let ctx = support::ctx();
    a.attach(&ctx);
    // Working on the laptop; then the UMC is plugged in — seen by the watcher thread.
    a.settings.audio.device = Some(demo::REALTEK_OUT.into());
    a.settings.audio.input_device = Some(demo::REALTEK_IN.into());
    a.audio_setup.demo.as_mut().unwrap().device = demo::REALTEK_OUT.into();
    a.audio_setup.scan = Some(demo::scan(false));
    let plugged = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let p = plugged.clone();
    a.audio_setup.plug.watch_with(DeviceWatch::with_source(
        std::time::Duration::from_millis(5),
        move || devices(p.load(std::sync::atomic::Ordering::Relaxed)),
    ));
    let wait = |a: &mut CantusApp, ctx: &egui::Context, until: &dyn Fn(&CantusApp) -> bool| {
        for _ in 0..400 {
            run(a, ctx, vec![]);
            if until(a) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("timed out");
    };
    wait(&mut a, &ctx, &|a| a.audio_setup.plug.last.is_some());
    assert!(a.audio_setup.notices.is_empty());
    plugged.store(true, std::sync::atomic::Ordering::Relaxed);
    wait(&mut a, &ctx, &|a| !a.audio_setup.notices.is_empty());
    assert_eq!(
        a.audio_setup.notices[0].0,
        Notice::Connected(demo::UMC.into())
    );
    // The card's first button switches; the remembered UMC settings come back.
    a.audio_setup.scan = Some(demo::scan(true));
    click(&mut a, &ctx, Hit::Notice(0, 0));
    assert_eq!(a.settings.audio.device.as_deref(), Some(demo::UMC_OUT));
    assert_eq!(a.settings.audio.buffer_frames, Some(128));
    assert!(a.audio_setup.notices.is_empty(), "answered");
}

#[test]
fn problems_are_said_in_words_and_their_fix_is_one_click() {
    let (mut a, _dir) = studio();
    let ctx = support::ctx();
    a.attach(&ctx);
    // Two interfaces at two rates: the input cannot follow the output.
    let live = a.audio_setup.demo.as_mut().unwrap();
    live.input = Some(demo::REALTEK_IN.into());
    live.input_error = Some(format!(
        "{}: the input runs at 44100 Hz and cannot run at 48000 Hz",
        demo::REALTEK_IN
    ));
    let own = setup::own_input(&a);
    let found = problems::diagnose(&setup::live(&mut a, false), None, own.as_deref(), 0);
    let (p, fixes) = &found[0];
    assert_eq!(
        *p,
        problems::Problem::RateMismatch {
            input: 44_100,
            output: 48_000
        }
    );
    assert!(
        p.text().contains("44.1") && p.text().contains("48"),
        "{}",
        p.text()
    );
    assert_eq!(fixes[0], problems::Fix::RunAt(44_100));
    assert_eq!(fixes[1], problems::Fix::UseInput(demo::UMC_IN.into()));
    // The fixes through the popover's buttons.
    a.audio_setup.popover = true;
    click(&mut a, &ctx, Hit::Fix(0, 1));
    assert_eq!(a.settings.audio.input_device.as_deref(), Some(demo::UMC_IN));
    click(&mut a, &ctx, Hit::Fix(0, 0));
    assert_eq!(a.settings.audio.sample_rate, Some(44_100));
    // Another application holds the device; a stalled ASIO driver; dropouts.
    let mut l = demo::live_on_umc();
    l.open_error = Some(format!("stream error {}", cantus_engine::devices::BUSY_TAG));
    let f = problems::diagnose(&l, None, None, 0);
    assert!(matches!(f[0].0, problems::Problem::Busy { .. }));
    let mut l = demo::live_on_umc();
    l.driver = "ASIO".into();
    l.stalled = true;
    let f = problems::diagnose(&l, None, None, 0);
    assert_eq!(f[0].0, problems::Problem::Stalled { asio: true });
    assert_eq!(f[0].1, vec![problems::Fix::RestartAudio]);
    assert!(f[0].0.is_silent());
    let f = problems::diagnose(&demo::live_on_umc(), None, None, 5);
    assert_eq!(
        f[0].0,
        problems::Problem::Dropouts {
            count: 5,
            buffer: 128
        }
    );
    assert_eq!(f[0].1, vec![problems::Fix::RaiseBuffer(256)]);
    setup::fix(&mut a, &f[0].1[0]);
    assert_eq!(a.settings.audio.buffer_frames, Some(256));
    assert!(problems::diagnose(&demo::live_on_umc(), None, None, 2).is_empty());
    // Every sentence exists in both languages.
    for loc in cantus_ui::i18n::Locale::ALL {
        cantus_ui::i18n::default_thread_locale(loc);
        for p in [
            problems::Problem::Lost { name: "X".into() },
            problems::Problem::Stalled { asio: true },
            problems::Problem::Stalled { asio: false },
            problems::Problem::OpenFailed { error: "e".into() },
            problems::Problem::Busy { name: "X".into() },
            problems::Problem::RateMismatch {
                input: 44_100,
                output: 48_000,
            },
            problems::Problem::InputFailed { error: "e".into() },
            problems::Problem::Dropouts {
                count: 3,
                buffer: 64,
            },
        ] {
            assert!(!p.text().starts_with("devices."), "{loc:?}: {}", p.text());
        }
    }
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
}
