//! The owner's three frictions of round 30, end to end: Settings easy to find (the Options
//! menu, its pages, the picker's way to Settings ▸ Plug-ins), the count-in on play, and
//! plug-ins looked for at start (on the boot path, `CantusApp::assemble`). Its own test
//! binary: it points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::Ticks;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::keymap::{Action, Preset};
use cantus_ui::macos::menu::{self, Entry, Pick};
use cantus_ui::picker::PickPurpose;
use cantus_ui::picker_catalog::Source;
use cantus_ui::picker_query::Shelf;
use cantus_ui::views::picker_keys;
use cantus_ui::views::plugins;
use cantus_ui::views::workspace_settings::{self, SettingsPage};
use cantus_ui::CantusApp;

fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn app() -> CantusApp {
    let _ = data();
    let mut app = CantusApp::from_parts(
        Some(cantus_ui::welcome_project()),
        EngineHandle::dummy(),
        false,
    );
    let tmp = data().join("ui");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    // Scans here read only an empty folder of the test's, never the machine's plug-ins.
    app.plugins.scanner.standard_folders = false;
    app
}

// ---- (a) Settings easy to find ---------------------------------------------------------

#[test]
fn the_options_menu_opens_settings_on_its_shortcut_and_each_page() {
    // This thread's words, whatever another test sets for the process.
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let map = Preset::Cantus.keymap();
    let options = menu::options(&map);
    let picks: Vec<Pick> = options
        .entries
        .iter()
        .filter_map(|e| match e {
            Entry::Item { pick, .. } => Some(*pick),
            _ => None,
        })
        .collect();
    // Settings itself, on the keymap's own Ctrl+, (the macOS builder's action), which the
    // window's Options menu shows beside it.
    assert_eq!(picks[0], Pick::Action(Action::OpenSettings));
    let ctrl_comma = cantus_ui::keymap::Binding::cmd(egui::Key::Comma);
    assert!(map.bindings_for(Action::OpenSettings).contains(&ctrl_comma));
    assert!(picks.contains(&Pick::Settings(SettingsPage::Plugins)));
    assert!(picks.contains(&Pick::Settings(SettingsPage::Audio)));
    assert!(picks.contains(&Pick::ScanPlugins));
    // The Mac's bar carries the same menu, beside View.
    let bar = menu::model(&map);
    assert!(bar.iter().any(|m| m == &options));

    // Each page item opens Settings there; Settings alone keeps the page last chosen.
    let mut app = app();
    let ctx = egui::Context::default();
    menu::run(&mut app, &ctx, Pick::Settings(SettingsPage::Plugins));
    assert!(app.show_settings);
    assert_eq!(app.settings_page.page, SettingsPage::Plugins);
    app.show_settings = false;
    app.run_action(Action::OpenSettings);
    assert!(app.show_settings);
    assert_eq!(app.settings_page.page, SettingsPage::Plugins);
    workspace_settings::open(&mut app, SettingsPage::Audio);
    assert_eq!(app.settings_page.page, SettingsPage::Audio);
}

#[test]
fn every_settings_page_has_its_word_in_both_languages() {
    use cantus_ui::i18n::{t, Locale};
    for locale in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_locale(locale);
        for p in SettingsPage::ALL {
            let word = t(p.key());
            assert!(!word.contains('.'), "{locale:?}: {word}");
        }
    }
    cantus_ui::i18n::set_locale(Locale::En);
}

#[test]
fn an_empty_plug_in_shelf_leads_to_a_scan_and_to_settings() {
    // With no plug-ins at all, the CLAP and VST3 shelves are still listed.
    let shelves = cantus_ui::picker_query::shelves(&[], None);
    assert!(shelves.contains(&Shelf::Source(Source::Clap)));
    assert!(shelves.contains(&Shelf::Source(Source::Vst3)));
    let mut app = app();
    app.open_picker(PickPurpose::AddInstrument { index: None });
    assert!(app.picker.is_some());
    picker_keys::act(&mut app, picker_keys::Action::ScanPlugins);
    assert!(app.plugins.scanning(), "Scan plug-ins starts a scan");
    picker_keys::act(&mut app, picker_keys::Action::PluginSettings);
    assert!(app.picker.is_none(), "the picker steps aside");
    assert!(app.show_settings);
    assert_eq!(app.settings_page.page, SettingsPage::Plugins);
}

// ---- (b) the count-in ------------------------------------------------------------------

fn at_bar_three(app: &mut CantusApp) -> Ticks {
    let bar3 = cantus_ui::views::bars::add_bars(app.project(), Ticks::ZERO, 2);
    app.seek(bar3);
    bar3
}

/// Play from stop: the dummy engine reports nothing, so it is told it plays.
fn play(app: &mut CantusApp) {
    assert!(!app.engine.is_playing());
    app.toggle_play();
    let at = app.engine.position();
    app.engine.set_heard_for_tests(at, true);
}

fn stop(app: &mut CantusApp) {
    assert!(app.engine.is_playing());
    app.toggle_play();
    let at = app.engine.position();
    app.engine.set_heard_for_tests(at, false);
}

#[test]
fn play_counts_in_only_when_asked_and_says_once_that_the_count_in_waits_for_record() {
    // This thread's words, whatever another test sets for the process.
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut app = app();
    let mut rs = app.project().recording.clone();
    rs.count_in_bars = 1;
    app.set_record_settings(rs);
    // Off (the default): Play starts where the playhead is, and the status line says why
    // no count-in sounded — once.
    assert!(!app.settings.count_in_on_play);
    let bar3 = at_bar_three(&mut app);
    play(&mut app);
    assert_eq!(app.engine.position(), bar3);
    assert!(!app.play_count_in());
    let said = app.status.text.clone();
    assert!(said.contains("Also on play"), "{said}");
    stop(&mut app);
    app.set_status(String::new());
    at_bar_three(&mut app);
    play(&mut app);
    assert!(!app.status.text.contains("Also on play"), "once");
    stop(&mut app);

    // On: Play from stop starts one bar early; stopping ends the count-in.
    app.settings.count_in_on_play = true;
    let bar3 = at_bar_three(&mut app);
    play(&mut app);
    let bar2 = cantus_ui::views::bars::add_bars(app.project(), bar3, -1);
    assert_eq!(app.engine.position(), bar2, "one bar of count-in");
    assert!(app.play_count_in());
    stop(&mut app);
    assert!(!app.play_count_in(), "stopped: the count-in is over");
}

// ---- (c) plug-ins looked for at start --------------------------------------------------

#[test]
fn the_boot_path_looks_for_new_plug_ins_in_the_background_unless_turned_off() {
    let data = data();
    let boot = |on: bool, db: Option<cantus_host::PluginDb>| {
        let mut files = Files::read(data, Some(cantus_ui::i18n::Locale::En));
        assert!(files.settings.plugin_scan_at_start, "on by default");
        files.settings.plugin_scan_at_start = on;
        let ctx = egui::Context::default();
        let app = CantusApp::assemble(
            &ctx,
            files,
            None,
            EngineHandle::dummy(),
            None,
            db,
            None,
            false,
        );
        (app.plugins.scanning(), app.scan_quiet)
    };
    assert_eq!(
        boot(true, Some(Default::default())),
        (true, true),
        "a real start scans, quietly unless it finds something"
    );
    assert_eq!(boot(false, Some(Default::default())), (false, false));
    assert_eq!(
        boot(true, None),
        (false, false),
        "tests and tools hand no database"
    );
    assert!(
        !plugins::scans_at_start(true, true, true),
        "not in the stress mode"
    );
}

#[test]
fn a_scan_names_what_it_found_new() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(
        plugins::found_text(&names(&["Surge XT"])),
        "New plug-in: Surge XT"
    );
    assert_eq!(
        plugins::found_text(&names(&["Odin 2", "Surge XT", "ValhallaSupermassive"])),
        "3 new plug-ins: Odin 2, Surge XT, ValhallaSupermassive"
    );
    assert_eq!(
        plugins::found_text(&names(&["A", "B", "C", "D", "E"])),
        "5 new plug-ins: A, B, C and 2 more"
    );
}

/// Every Settings page reads whole in English and Spanish at a laptop's size: long notes
/// and warnings wrap to the page, nothing is cut with "…" (round 30, the integrator's
/// review of the Spanish Plug-ins page).
#[test]
fn every_settings_page_reads_whole_in_both_languages() {
    use cantus_ui::i18n::Locale;
    fn walk(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) if t.galley.elided => out.push(t.galley.text().to_owned()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    for locale in [Locale::En, Locale::Es] {
        cantus_ui::i18n::set_locale(locale);
        let mut a = app();
        // An interface open, as the owner's: the Audio page's longest lines.
        cantus_ui::views::settings_audio::demo_setup(&mut a);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1366.0, 900.0),
            )),
            ..Default::default()
        };
        for page in SettingsPage::ALL {
            workspace_settings::open(&mut a, page);
            let mut cut = Vec::new();
            for frame in 0..3 {
                let mut out = ctx.run_ui(input.clone(), |ui| a.dialogs(ui.ctx()));
                out.textures_delta.clear();
                if frame == 2 {
                    for c in &out.shapes {
                        walk(&c.shape, &mut cut);
                    }
                }
            }
            assert!(cut.is_empty(), "{locale:?} {page:?} cut: {cut:?}");
        }
    }
    cantus_ui::i18n::set_locale(Locale::En);
}
