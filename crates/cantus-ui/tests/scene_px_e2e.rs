//! r40:scene-px — *Settings ▸ Appearance ▸ Instrument pictures* end to end: the app
//! `CantusApp::assemble` puts together from a `settings.json` that says *Photographic*
//! registers the photographic renderer and draws every scene with it — the picture the
//! scene module hands out is, to the bit, the photographic renderer's of the same view —
//! and *Drawn* gives the projected-vector picture back. A binary of its own: the setting
//! and the registration are process-wide.

use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::RoomLook;
use cantus_core::inst::tympanum::{KitRef, RoomChoice};
use cantus_core::inst::tympanum_kit::{KitDef, RoomDef};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::scene::{self, backend, Room, SceneBackend, SceneView, Subject};
use cantus_ui::views::inst::chorda::scene::hybrid;
use cantus_ui::CantusApp;

fn chorda_on_the_stage() -> SceneView {
    SceneView {
        subject: Subject::Piano {
            look: Look::new(Body::Grand, Finish::Black),
            lid: 1.0,
            persp: 0.35,
            width: 0.7,
            room_level: 0.35,
            mics: false,
        },
        room: Room::Stage,
        look: RoomLook::default(),
        dark: true,
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: egui::Color32::from_rgb(92, 175, 233),
        ink: egui::Color32::from_rgb(227, 232, 237),
    }
}

/// Tympanum's kit on the stage (the scene module's own pieces when the definition has none).
fn tympanum_on_the_stage() -> SceneView {
    SceneView {
        subject: Subject::Kit {
            kit: KitRef::default_kit(),
            def: Box::new(KitDef {
                kit: KitRef::default_kit(),
                name: "Studio Maple".into(),
                modelled: true,
                pieces: Vec::new(),
                mics: Vec::new(),
                bleed: Default::default(),
                room: RoomDef {
                    width: 600.0,
                    depth: 800.0,
                    height: 340.0,
                    back: 270.0,
                },
                default_room: RoomChoice::Own,
                licence: None,
                credit: None,
                finish: Finish::Maple,
            }),
            finish: Finish::Maple,
            mics: false,
        },
        ..chorda_on_the_stage()
    }
}

#[test]
fn photographic_pictures_come_from_the_per_pixel_renderer_through_assemble() {
    let data = tempfile::tempdir().unwrap();
    std::env::set_var("CANTUS_DATA_DIR", data.path());
    std::fs::write(
        data.path().join("settings.json"),
        r#"{ "instrument_pictures": "PerPixel" }"#,
    )
    .unwrap();
    let files = Files::read(data.path(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let song = cantus_ui::demos::startup_project();
    let app = CantusApp::assemble(
        &ctx,
        files,
        Some(song),
        EngineHandle::dummy(),
        None,
        None,
        None,
        false,
    );
    assert_eq!(
        app.settings.instrument_pictures,
        SceneBackend::PerPixel,
        "the setting was read"
    );
    assert!(
        backend::pixel().is_some(),
        "assemble registered the photographic renderer"
    );
    assert_eq!(backend::current(), SceneBackend::PerPixel);

    let v = chorda_on_the_stage();
    let size = egui::vec2(300.0, 180.0);
    let photo = scene::image(&v, size, 1.0);
    let (direct, _) = hybrid::render(&v, size, 1.0, hybrid::FULL, hybrid::threads());
    assert_eq!(photo.size, direct.size);
    assert!(
        photo.pixels == direct.pixels,
        "the scene's picture is the photographic renderer's, to the bit"
    );

    // Tympanum's kit: the same renderer.
    let k = tympanum_on_the_stage();
    let kit_photo = scene::image(&k, size, 1.0);
    let (kit_direct, _) = hybrid::render(&k, size, 1.0, hybrid::FULL, hybrid::threads());
    assert!(
        kit_photo.pixels == kit_direct.pixels,
        "Tympanum's picture is the photographic renderer's, to the bit"
    );

    // *Drawn*, as the settings page sets it: the projected vectors again, for both.
    backend::set_default(SceneBackend::Vector);
    assert_eq!(backend::current(), SceneBackend::Vector);
    let drawn = scene::image(&v, size, 1.0);
    assert!(drawn.pixels != photo.pixels, "Drawn is not the photograph");
    let kit_drawn = scene::image(&k, size, 1.0);
    assert!(
        kit_drawn.pixels != kit_photo.pixels,
        "Tympanum drawn is not the photograph"
    );
    backend::set_default(SceneBackend::PerPixel);
    let again = scene::image(&v, size, 1.0);
    assert!(
        again.pixels == photo.pixels,
        "and Photographic comes back the same"
    );
    assert!(scene::image(&k, size, 1.0).pixels == kit_photo.pixels);
}
