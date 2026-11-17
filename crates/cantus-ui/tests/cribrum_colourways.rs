//! Cribrum's colourways (ADR 0130 §Colourways, extending ADR 0074 §5): the user's choice
//! per unit kind kept in `settings.json`, applied by the boot path (`CantusApp::assemble`)
//! and surviving a restart; each colourway keeps the themes' contrast promises and paints the
//! editor; a choice repaints every open window of the kind at once. Its own test binary: it
//! points `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::kit::skin::{self, Unit};
use cantus_ui::CantusApp;

fn assemble() -> CantusApp {
    let config = cantus_ui::app::config_dir();
    let files = Files::read(&config, Some(cantus_ui::i18n::Locale::En));
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

const UNIT: Unit = Unit::Effect(EffectKind::AutoFilter);

#[test]
fn the_colourway_is_the_users_kept_through_a_restart_and_worn_at_once() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    std::fs::create_dir_all(cantus_ui::app::config_dir()).unwrap();

    // A new installation: Cribrum wears its face, Nocturne.
    let mut app = assemble();
    assert!(app.settings.unit_colourways.is_empty());
    assert_eq!(skin::colourway_of(UNIT).map(|w| w.id), Some("nocturne"));
    assert_eq!(skin::of(UNIT).key, "fx.cribrum");

    // Brass chosen: worn at once, by every Cribrum (the sheet is the kind's), and kept.
    app.set_unit_colourway("cribrum", "brass");
    assert_eq!(skin::of(UNIT).key, "fx.cribrum.brass");
    assert_eq!(
        app.settings
            .unit_colourways
            .get("cribrum")
            .map(String::as_str),
        Some("brass")
    );
    // Another unit is untouched.
    assert_eq!(skin::of(Unit::Effect(EffectKind::Eq)).key, "fx.eq");

    // A restart: the thread forgets, the boot path reads the settings again.
    skin::set_colourways(std::iter::empty());
    assert_eq!(skin::of(UNIT).key, "fx.cribrum");
    let app = assemble();
    assert_eq!(
        app.settings
            .unit_colourways
            .get("cribrum")
            .map(String::as_str),
        Some("brass"),
        "kept in settings.json"
    );
    assert_eq!(
        skin::of(UNIT).key,
        "fx.cribrum.brass",
        "applied by assemble"
    );

    // An id a later version no longer has: the face, never a missing sheet.
    skin::set_colourway("cribrum", "gone");
    assert_eq!(skin::of(UNIT).key, "fx.cribrum");
    skin::set_colourways(std::iter::empty());
}

#[test]
fn every_colourway_keeps_the_contrast_promises_and_paints_the_editor() {
    let ways = skin::colourways(UNIT);
    assert_eq!(ways.len(), 3);
    assert_eq!(ways[0].sheet.key, "fx.cribrum", "the face is the first");
    let mut keys = std::collections::BTreeSet::new();
    for w in ways {
        assert!(keys.insert(w.sheet.key), "{}: a key of its own", w.id);
        let fails = skin::contrast_failures(w.sheet);
        assert!(fails.is_empty(), "{}: {fails:?}", w.id);
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            assert_ne!(cantus_ui::i18n::t(w.name_key), w.name_key, "{} named", w.id);
        }
        // The editor painted in it: the window's frame wears the colourway's sheet.
        skin::set_colourway("cribrum", w.id);
        assert_eq!(skin::of(UNIT).key, w.sheet.key);
        let mut a = CantusApp::from_parts(
            Some(cantus_ui::starter_project()),
            EngineHandle::dummy(),
            false,
        );
        a.demo_editor("cribrum");
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            cantus_ui::views::fx::window(&mut a, ui.ctx())
        });
        out.textures_delta.clear();
        assert!(!out.shapes.is_empty(), "{}: the editor painted", w.id);
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    skin::set_colourways(std::iter::empty());
}
