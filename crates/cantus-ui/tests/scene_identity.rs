//! r42:arcus-look — Arcus's cello and bass joined the scene engine (ADR 0126) as its third
//! and fourth subjects, additively: Chorda's pianos and Tympanum's kit are drawn to the
//! pixel as they were before, in both pictures (Drawn and Photographic) and both moods.
//! The hashes below were measured on the tree before Arcus's picture (`r42/arcus`
//! 58b7a0006) with this same file; `CANTUS_SCENE_HASHES=1` prints them again.
//!
//! The pictures' floating point goes through the platform's own `sin`/`cos`, so the hashes
//! are Windows's (the owner's machine, where they were measured); elsewhere the test only
//! checks that each picture is drawn.

use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::RoomLook;
use cantus_ui::scene::{self, Room, SceneBackend, SceneView, Subject};

fn view(subject: Subject, room: Room, dark: bool) -> SceneView {
    SceneView {
        subject,
        room,
        look: RoomLook::default(),
        dark,
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: egui::Color32::TRANSPARENT,
        ink: egui::Color32::TRANSPARENT,
    }
}

fn piano(body: Body, finish: Finish) -> Subject {
    Subject::Piano {
        look: Look::new(body, finish),
        lid: 1.0,
        persp: 0.35,
        width: 0.7,
        room_level: 0.35,
        mics: true,
    }
}

fn kit() -> Subject {
    use cantus_core::inst::tympanum::{KitRef, RoomChoice};
    use cantus_core::inst::tympanum_kit::{KitDef, RoomDef};
    // As `scene_vector.rs` draws it: the designer's studio kit, its pieces and mics the
    // scene's own.
    let def = KitDef {
        kit: KitRef::default_kit(),
        name: "Studio".into(),
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
    };
    Subject::Kit {
        kit: def.kit.clone(),
        def: Box::new(def),
        finish: Finish::Maple,
        mics: true,
    }
}

fn fnv(img: &egui::ColorImage) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for c in &img.pixels {
        for b in c.to_array() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h ^ ((img.size[0] as u64) << 32 | img.size[1] as u64)
}

/// The pictures measured: (name, view).
fn cases() -> Vec<(&'static str, SceneView)> {
    vec![
        (
            "concert-studio-night",
            view(piano(Body::Concert, Finish::Black), Room::Studio, true),
        ),
        (
            "upright-stage-paper",
            view(piano(Body::Upright, Finish::Walnut), Room::Stage, false),
        ),
        (
            "grand-hall-night",
            view(piano(Body::Grand, Finish::White), Room::Hall, true),
        ),
        ("kit-stage-night", view(kit(), Room::Stage, true)),
        ("kit-studio-paper", view(kit(), Room::Studio, false)),
    ]
}

const BEFORE: &[(&str, u64)] = &[
    ("drawn-concert-studio-night", 0xaea6071687d73038),
    ("drawn-upright-stage-paper", 0x7ea3bd47be24001d),
    ("drawn-grand-hall-night", 0x6dd831bb15f008a4),
    ("drawn-kit-stage-night", 0xd2c50e3c32061592),
    ("drawn-kit-studio-paper", 0x008a4837c6ec4995),
    ("photo-concert-studio-night", 0x45ba006fdfb46f4e),
    ("photo-upright-stage-paper", 0xdd344f2f81bfed7a),
    ("photo-grand-hall-night", 0x67fb18adb8fd2d9e),
    ("photo-kit-stage-night", 0x094b076c6f057961),
    ("photo-kit-studio-paper", 0x6ffe22ce93427edc),
];

#[test]
fn chordas_and_tympanums_pictures_are_the_pixels_they_were() {
    cantus_ui::views::inst::chorda::scene::hybrid::register();
    let size = egui::vec2(360.0, 200.0);
    let mut got = Vec::new();
    for (be, tag) in [
        (SceneBackend::Vector, "drawn"),
        (SceneBackend::PerPixel, "photo"),
    ] {
        scene::backend::set_default(be);
        for (name, v) in cases() {
            let img = scene::image(&v, size, 1.0);
            assert!(img.size[0] > 8 && img.size[1] > 8, "{name}: a picture");
            got.push((format!("{tag}-{name}"), fnv(&img)));
        }
    }
    scene::backend::set_default(SceneBackend::Vector);
    if std::env::var("CANTUS_SCENE_HASHES").is_ok() {
        for (n, h) in &got {
            println!("    (\"{n}\", {h:#018x}),");
        }
    }
    if cfg!(windows) && !BEFORE.is_empty() {
        for (n, h) in &got {
            let want = BEFORE.iter().find(|b| b.0 == n).map(|b| b.1);
            assert_eq!(want, Some(*h), "{n}: the picture changed");
        }
    }
}
