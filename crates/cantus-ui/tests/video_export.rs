//! The video export through the application (ADR 0088): the dialog renders in both themes
//! and languages for every format, its choices are remembered, an export runs off the
//! interface thread with the song's render and lands as a file, a stop leaves nothing,
//! what cannot start says why, the agent tool is in the catalogue, and every key has words
//! in both languages.

use cantus_engine::EngineHandle;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::video_export::{self as vx, Back, Format, Range, Shape};
use cantus_ui::{starter_project, CantusApp};

static LANGUAGE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn language() -> std::sync::MutexGuard<'static, ()> {
    let guard = LANGUAGE.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_thread_locale(Locale::En);
    guard
}

fn app(dir: &std::path::Path) -> CantusApp {
    let mut a = CantusApp::from_parts(Some(starter_project()), EngineHandle::dummy(), false);
    a.settings_path = dir.join("settings.json");
    a
}

fn input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    }
}

#[test]
fn the_dialog_renders_in_both_themes_and_languages_for_every_format() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    for dark in [false, true] {
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let theme = if dark {
            cantus_ui::theme::ink_and_gold()
        } else {
            cantus_ui::theme::default_theme()
        };
        cantus_ui::theme::apply(&theme, &ctx);
        let mut a = app(dir.path());
        for locale in Locale::ALL {
            i18n::set_thread_locale(locale);
            for (format, shape, back) in [
                (Format::Mp4, Shape::Vertical, Back::Stage),
                (Format::Webm, Shape::Square, Back::Green),
                (Format::Mov, Shape::Landscape, Back::Transparent),
                (Format::Png, Shape::Custom, Back::Black),
            ] {
                vx::open(&mut a);
                let d = a.video.dialog.as_mut().unwrap();
                d.format = format;
                d.shape = shape;
                d.background = back;
                for _ in 0..2 {
                    let mut out = ctx.run_ui(input(), |ui| vx::frame(&mut a, ui.ctx()));
                    out.textures_delta.clear();
                }
                assert!(a.video.dialog.is_some(), "nothing was clicked: still open");
                let preview = cantus_ui::views::video_export::geo(&ctx, "preview").unwrap();
                let (w, h) = a.video.dialog.as_ref().unwrap().size();
                let shown = preview.width() / preview.height();
                assert!(
                    (shown - w as f32 / h as f32).abs() < 0.02,
                    "the preview keeps the shape"
                );
                assert!(cantus_ui::views::video_export::geo(&ctx, "button.export").is_some());
            }
        }
    }
    i18n::set_thread_locale(Locale::En);
}

#[test]
fn a_video_export_runs_off_the_interface_thread_and_lands_as_a_file() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    vx::open(&mut a);
    let mut d = a.video.dialog.clone().unwrap();
    d.format = Format::Mov;
    d.shape = Shape::Custom;
    d.custom = (720, 1280);
    d.background = Back::Transparent;
    let path = dir.path().join("dance");
    vx::start(&mut a, &path, &d, false).unwrap();
    assert!(a.video.busy(), "the call returns while the export runs");
    vx::wait(&mut a);
    assert!(!a.status.error, "{}", a.status.text);
    let file = dir.path().join("dance.mov");
    assert!(file.is_file(), "the extension is added");
    assert!(a.status.text.contains("dance.mov"), "{}", a.status.text);
    assert!(matches!(a.video.last, Some(Ok(_))));
    // Remembered, with its folder.
    assert_eq!(a.settings.video.format, Format::Mov);
    assert_eq!(a.settings.video.folder.as_deref(), Some(dir.path()));
    let bytes = std::fs::read(&file).unwrap();
    assert!(cantus_video::bmff::top(&bytes, b"moov").is_some());
}

#[test]
fn a_stop_leaves_nothing_and_a_missing_range_says_why() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let mut d = a.settings.video.clone();
    d.format = Format::Png;
    let path = dir.path().join("frames");
    vx::start(&mut a, &path, &d, false).unwrap();
    vx::stop(&mut a);
    vx::wait(&mut a);
    let left: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .filter(|n| n != "settings.json")
        .collect();
    assert!(left.is_empty(), "{left:?}");
    a.time_selection = None;
    d.range = Range::Selection;
    let e = vx::start(&mut a, &path, &d, false).unwrap_err();
    assert_eq!(e, i18n::t("export_audio.no_selection"));
}

#[test]
fn agents_have_export_video() {
    let tool = cantus_ui::ai_tools::find("export_video").expect("registered");
    assert_eq!((tool.input_schema)()["required"][0], "file");
    assert!(cantus_ui::ai_tools::find("get_video_export").is_some());
}

#[test]
fn every_key_of_the_video_area_has_words_in_both_languages() {
    let _l = language();
    let src = include_str!("../locales/areas/video.en.txt");
    let keys: Vec<&str> = src
        .lines()
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim()))
        .filter(|k| !k.starts_with('#'))
        .collect();
    assert!(keys.len() > 80);
    for locale in Locale::ALL {
        i18n::set_thread_locale(locale);
        for key in &keys {
            assert_ne!(i18n::t(key), *key, "{locale:?} has no words for {key}");
        }
    }
    i18n::set_thread_locale(Locale::En);
}

fn mov_settings(a: &CantusApp) -> cantus_ui::video_export::VideoSettings {
    let mut d = a.settings.video.clone();
    d.format = Format::Mov;
    d.shape = Shape::Custom;
    d.custom = (720, 1280);
    d
}

/// One render at a time through the plug-ins' render instances (review 1 #1): while a
/// video exports, the app says an export is in progress — freezing, bakes and the frame
/// ticker go by it — and an audio export does not start.
#[test]
fn a_video_export_holds_the_render_as_an_audio_export_does() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let d = mov_settings(&a);
    vx::start(&mut a, &dir.path().join("v"), &d, false).unwrap();
    assert!(a.export_in_progress(), "the video export counts as one");
    assert!(
        !cantus_ui::views::export::start(&mut a, &dir.path().join("song.wav")),
        "no audio export alongside"
    );
    vx::wait(&mut a);
    assert!(!a.export_in_progress());
    assert!(!dir.path().join("song.wav").exists());
}

/// A plug-in lost during the render fails the export, as the audio export says it
/// (review 1 #2).
#[test]
fn a_plug_in_lost_in_the_render_is_said_not_finished_silently() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let d = mov_settings(&a);
    vx::start(&mut a, &dir.path().join("lost"), &d, false).unwrap();
    a.plugins
        .lose_in_render_for_tests("Hall", "stopped", cantus_host::RenderLoss::Stopped);
    vx::wait(&mut a);
    assert!(matches!(a.video.last, Some(Err(_))), "{:?}", a.video.last);
    assert!(a.status.error, "{}", a.status.text);
    assert!(a.status.text.contains("Hall"), "{}", a.status.text);
}

/// The encoders are found on a worker: the dialog's first ask answers "not yet"
/// (review 1 #4), and the answer comes.
#[test]
fn the_encoders_are_found_off_the_interface_thread() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let mut d = a.settings.video.clone();
    d.format = Format::Mp4;
    d.system_aac = !d.system_aac; // codecs no earlier test asked for
    assert!(
        vx::plan(&mut a, &d).is_none(),
        "the first ask does not block"
    );
    let t0 = std::time::Instant::now();
    let p = loop {
        if let Some(p) = vx::plan(&mut a, &d) {
            break p;
        }
        assert!(t0.elapsed().as_secs() < 30, "found in time");
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    assert_eq!(Some(p), vx::plan(&mut a, &d), "then kept");
}

/// The title reads over every background offered, in both themes (review 1 #6).
#[test]
fn the_title_reads_over_every_background_in_every_theme() {
    use cantus_ui::video_export::job;
    for theme in [
        cantus_ui::theme::default_theme(),
        cantus_ui::theme::ink_and_gold(),
    ] {
        let ctx = egui::Context::default();
        cantus_ui::theme::apply(&theme, &ctx);
        let c = cantus_ui::kit::Style::current().t.panel;
        let stage = [c.r(), c.g(), c.b(), 255];
        for format in [Format::Mp4, Format::Mov] {
            for back in Back::ALL {
                let d = cantus_ui::video_export::VideoSettings {
                    format,
                    background: back,
                    ..Default::default()
                };
                let bg = job::background_for(&d, stage);
                let (ink, edge) = job::title_colours(bg);
                assert!(cantus_video::title::contrast(ink, edge) >= 4.5);
                if let cantus_dance::render::Background::Solid(b) = bg {
                    let c = cantus_video::title::contrast(ink, b);
                    assert!(c >= 4.5, "{back:?} over {b:?}: {c:.2}");
                }
            }
        }
    }
}

/// The estimate goes by this machine's measured pace once an export ran, said rounded
/// (review 1 #10); a name the save dialog returned without the extension asks before the
/// file with it is replaced (review 1 #8).
#[test]
fn the_pace_is_measured_and_the_estimate_is_honest() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let d = mov_settings(&a);
    assert!(!a.settings.video.pace.contains_key("mov"));
    vx::start(&mut a, &dir.path().join("paced"), &d, false).unwrap();
    vx::wait(&mut a);
    let ms = a.settings.video.pace.get("mov").copied().expect("measured");
    assert!(ms > 0.0 && ms.is_finite(), "{ms}");
    assert_eq!(vx::honest(4.0), "a few seconds");
    assert_eq!(vx::honest(22.0), "about 20 s");
    assert_eq!(vx::honest(95.0), "about 2 min");
    let picked = dir.path().join("paced");
    let target = vx::with_extension(&picked, &d);
    assert!(vx::must_ask(&picked, &target), "paced.mov is there: asked");
    assert!(
        !vx::must_ask(&target, &target),
        "the dialog asked about this one"
    );
}

/// WebM, MOV and PNG never look for the MP4's encoders; an MP4's are found on a worker
/// even when an export (or an agent) waits for them (review 2 #1).
#[test]
fn only_an_mp4_asks_for_encoders_and_never_on_this_thread() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let d = mov_settings(&a);
    vx::start(&mut a, &dir.path().join("no-plan"), &d, false).unwrap();
    vx::wait(&mut a);
    // Counted by this window: another test's worker may still be finishing (it ran
    // `ffmpeg -encoders` for seconds on Linux) and the process's count would see it.
    assert_eq!(a.video.asked, 0, "a MOV asks for no encoders");
    let mut mp4 = a.settings.video.clone();
    mp4.format = Format::Mp4;
    mp4.system_aac = !mp4.system_aac;
    let before = vx::PLANS.lock().unwrap().0;
    let _ = vx::plan_now(&mut a, &mp4);
    assert_eq!(a.video.asked, 1);
    let seen = vx::PLANS.lock().unwrap().clone();
    assert!(seen.0 > before, "computed, and waited for");
    assert_eq!(
        seen.1, "cantus-video-codecs",
        "found on the worker, waited for here"
    );
}

/// Closing the window stops a video being made and leaves no work in progress behind
/// (review 2 #2).
#[test]
fn closing_stops_a_video_and_removes_its_part() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let mut d = mov_settings(&a);
    d.custom = (2160, 3840); // long enough to be under way when the window closes
    vx::start(&mut a, &dir.path().join("closing"), &d, false).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    a.close_session();
    assert!(!a.video.busy());
    let left: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("cantus-part"))
        .collect();
    assert!(left.is_empty(), "{left:?}");
}

/// While a video exports, an audio export (the window's or an agent's) is refused in words
/// naming it, before anything is made (review 2 #3).
#[test]
fn an_audio_export_during_a_video_is_refused_in_words() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let mut d = mov_settings(&a);
    d.custom = (2160, 3840);
    vx::start(&mut a, &dir.path().join("long"), &d, false).unwrap();
    assert!(!cantus_ui::views::export::start(
        &mut a,
        &dir.path().join("song.wav")
    ));
    assert!(a.status.error);
    assert!(a.status.text.contains("video export"), "{}", a.status.text);
    assert_eq!(vx::busy_words(&a).as_deref(), Some(a.status.text.as_str()));
    vx::stop(&mut a);
    vx::wait(&mut a);
    assert_eq!(vx::busy_words(&a), None);
}
