//! The export dialog through the application (ADR 0033): it opens with the last choices
//! and the song's title, what is chosen is remembered in the settings file (and read back
//! by the next start), the export runs off the interface thread, a stop leaves no file,
//! stems come out in the chosen format, and the dialog renders in both themes and both
//! languages.

use cantus_engine::EngineHandle;
use cantus_export::{Format, Mp3Mode, Preset, Scope};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::settings::Settings;
use cantus_ui::views::{export, export_dialog};
use cantus_ui::{starter_project, CantusApp};

static LANGUAGE: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn language() -> std::sync::MutexGuard<'static, ()> {
    let guard = LANGUAGE.lock().unwrap_or_else(|e| e.into_inner());
    i18n::set_thread_locale(Locale::En);
    guard
}

/// An app whose settings file is in `dir`, so the tests never touch the user's.
fn app(dir: &std::path::Path) -> CantusApp {
    let mut a = CantusApp::from_parts(Some(starter_project()), EngineHandle::dummy(), false);
    a.settings_path = dir.join("settings.json");
    a
}

fn files_in(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n != "settings.json")
        .collect();
    v.sort();
    v
}

#[test]
fn the_dialog_opens_with_the_last_choices_and_the_songs_title() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    a.settings.export.format = Format::Ogg;
    a.settings.export.tags.artist = "Luis".into();
    export::open(&mut a, None);
    let d = a.export.dialog.as_ref().unwrap();
    assert_eq!(d.format, Format::Ogg);
    assert_eq!(d.tags.artist, "Luis");
    assert_eq!(d.tags.title, a.project().name);
    // The File menu's stems entry opens it on stems.
    export::close(&mut a);
    export::open(&mut a, Some(Scope::Stems));
    assert_eq!(a.export.dialog.as_ref().unwrap().scope, Scope::Stems);
    // A selection that is not there falls back to the song.
    export::close(&mut a);
    a.settings.export.scope = Scope::Selection;
    a.time_selection = None;
    export::open(&mut a, None);
    assert_eq!(a.export.dialog.as_ref().unwrap().scope, Scope::Song);
}

#[test]
fn the_choices_persist_in_the_settings_file() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    export::open(&mut a, None);
    {
        let d = a.export.dialog.as_mut().unwrap();
        *d = Preset::Podcast.apply(d);
        d.tags.artist = "Coro".into();
        d.tags.year = "2026".into();
    }
    export::close(&mut a);
    assert!(a.export.dialog.is_none());
    let saved = Settings::load(&a.settings_path);
    assert_eq!(saved.export.preset(), Some(Preset::Podcast));
    assert_eq!(saved.export.tags.artist, "Coro");
    assert_eq!(
        saved.export.tags.title, "",
        "the title is the song's, not remembered"
    );
    // The next start reads them back.
    let mut b = app(dir.path());
    b.settings = Settings::load(&b.settings_path);
    export::open(&mut b, None);
    let d = b.export.dialog.as_ref().unwrap();
    assert_eq!(d.preset(), Some(Preset::Podcast));
    assert_eq!(d.mp3, Mp3Mode::Cbr(128));
    assert_eq!(d.tags.year, "2026");
}

#[test]
fn an_export_runs_off_the_interface_thread_and_says_so() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    export::open(&mut a, None);
    a.export.dialog.as_mut().unwrap().format = Format::Mp3;
    a.export.dialog.as_mut().unwrap().tail_seconds = 0.5;
    let path = dir.path().join("song.mp3");
    assert!(export::start(&mut a, &path));
    assert!(a.export.busy(), "the call returns while the export runs");
    assert!(a.export.progress().is_some());
    export::wait(&mut a);
    assert!(!a.export.busy());
    assert!(!a.status.error, "{}", a.status.text);
    assert!(a.status.text.contains("song.mp3"), "{}", a.status.text);
    assert_eq!(files_in(dir.path()), vec!["song.mp3".to_string()]);
    // The folder is remembered for next time.
    assert_eq!(a.settings.export.folder.as_deref(), Some(dir.path()));
}

#[test]
fn export_to_takes_the_format_from_the_extension() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    let path = dir.path().join("song.flac");
    a.export_to(path.clone());
    while a.export_in_progress() {
        a.poll_export();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!a.status.error, "{}", a.status.text);
    let r = claxon_head(&path);
    assert_eq!(&r, b"fLaC");
}

fn claxon_head(p: &std::path::Path) -> [u8; 4] {
    let b = std::fs::read(p).unwrap();
    [b[0], b[1], b[2], b[3]]
}

#[test]
fn a_stopped_export_leaves_no_file() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    export::open(&mut a, None);
    a.export.dialog.as_mut().unwrap().tail_seconds = 15.0;
    let path = dir.path().join("song.ogg");
    assert!(export::start(&mut a, &path));
    export::stop(&mut a);
    export::wait(&mut a);
    assert!(
        files_in(dir.path()).is_empty(),
        "{:?}",
        files_in(dir.path())
    );
    assert!(a.status.text.contains("stopped"), "{}", a.status.text);
}

#[test]
fn stems_come_out_in_the_chosen_format() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let mut a = app(dir.path());
    export::open(&mut a, Some(Scope::Stems));
    {
        let d = a.export.dialog.as_mut().unwrap();
        d.format = Format::Flac;
        d.tail_seconds = 0.5;
    }
    let out = dir.path().join("stems");
    assert!(export::start(&mut a, &out));
    export::wait(&mut a);
    assert!(!a.status.error, "{}", a.status.text);
    let track = a.project().tracks[0].name.clone();
    assert_eq!(files_in(&out), vec![format!("01 {track}.flac")]);
}

#[test]
fn the_dialog_renders_in_both_themes_and_languages() {
    let _l = language();
    let dir = tempfile::tempdir().unwrap();
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
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
            for preset in Preset::ALL {
                export::open(&mut a, None);
                let d = a.export.dialog.as_mut().unwrap();
                *d = preset.apply(d);
                for _ in 0..2 {
                    let mut out = ctx.run_ui(input.clone(), |ui| {
                        export_dialog::show(&mut a, ui.ctx());
                    });
                    out.textures_delta.clear();
                }
                assert!(a.export.dialog.is_some(), "nothing was clicked: still open");
            }
        }
    }
    i18n::set_thread_locale(Locale::En);
}

#[test]
fn every_key_of_the_dialog_has_words_in_both_languages() {
    let _l = language();
    let src = include_str!("../locales/areas/export_audio.en.txt");
    let keys: Vec<&str> = src
        .lines()
        .filter_map(|l| l.split_once(" = ").map(|(k, _)| k.trim()))
        .filter(|k| !k.starts_with('#'))
        .collect();
    assert!(keys.len() > 50);
    for locale in Locale::ALL {
        i18n::set_thread_locale(locale);
        for key in &keys {
            assert_ne!(i18n::t(key), *key, "{locale:?} has no words for {key}");
        }
    }
    i18n::set_thread_locale(Locale::En);
}
