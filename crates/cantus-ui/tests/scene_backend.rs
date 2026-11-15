//! The scene's backend (round 40 phase 3, ADR 0126 amended; the owner's *Instrument
//! pictures: Drawn · Photographic*): Drawn is the default and a settings file without the
//! choice reads it; Photographic with no per-pixel renderer registered still draws
//! vectors; once one registers, the scene hands it the model — the subject as described,
//! its 3-D primitives, the fitted camera — and paints its pixels; a control gliding asks
//! for a quick bake, and the picture at rest for the whole one; the pick answers from the
//! same geometry in either backend; a tag's anchor stands at the piece.
//! Its own test binary: the backend is a process-wide setting.

use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::RoomLook;
use cantus_ui::scene::{self, Model, Picked, PixelBackend, Room, SceneBackend, SceneView, Subject};
use std::sync::{Arc, Mutex};

const SIZE: egui::Vec2 = egui::vec2(756.0, 452.0);
const MARK: egui::Color32 = egui::Color32::from_rgb(201, 7, 133);

fn view(lid: f32) -> SceneView {
    SceneView {
        subject: Subject::Piano {
            look: Look::new(Body::Concert, Finish::Black),
            lid,
            persp: 0.35,
            width: 0.7,
            room_level: 0.35,
            mics: true,
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

/// A stand-in per-pixel renderer: a flat picture of the right size; what it was asked.
#[derive(Default)]
struct Fake {
    asked: Mutex<Vec<(bool, usize, bool)>>,
}

impl PixelBackend for Fake {
    fn render(&self, m: &Model, ppp: f32, quick: bool) -> egui::ColorImage {
        let has_subject = matches!(m.subject, Some(Subject::Piano { .. }));
        self.asked
            .lock()
            .unwrap()
            .push((quick, m.prims.len(), has_subject));
        let w = (m.size.x * ppp).round() as usize;
        let h = (m.size.y * ppp).round() as usize;
        egui::ColorImage::new([w, h], vec![MARK; w * h])
    }
}

fn frame(ctx: &egui::Context, sc: &mut scene::Scene, v: &SceneView, t: f64) -> bool {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(t),
        ..Default::default()
    };
    let mut moving = false;
    ctx.run_ui(input, |root| {
        moving = sc.paint(root.painter(), root.max_rect(), v);
    })
    .textures_delta
    .clear();
    moving
}

#[test]
fn drawn_by_default_and_photographic_hands_the_model_to_the_registered_renderer() {
    // The setting: Drawn on a fresh install and in a file written before it existed.
    assert_eq!(
        cantus_ui::settings::Settings::default().instrument_pictures,
        scene::backend::DEFAULT_PICTURES
    );
    let old: cantus_ui::settings::Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.instrument_pictures, scene::backend::DEFAULT_PICTURES);
    // Drawn today (the owner may flip `DEFAULT_PICTURES` when Photographic is light enough).
    assert_eq!(scene::backend::DEFAULT_PICTURES, SceneBackend::Vector);
    assert_eq!(scene::backend::current(), SceneBackend::Vector);

    // Photographic chosen, nothing registered: vectors still (never a blank picture).
    scene::backend::set_default(SceneBackend::PerPixel);
    assert_eq!(scene::backend::current(), SceneBackend::Vector);
    let drawn = scene::image(&view(1.0), SIZE, 1.0);
    assert_ne!(drawn.pixels[drawn.size[0] * 200 + 378], MARK);

    // Registered: its picture, from the model with the subject and the primitives.
    let fake = Arc::new(Fake::default());
    assert!(scene::backend::register_pixel(fake.clone()));
    assert_eq!(scene::backend::current(), SceneBackend::PerPixel);
    let img = scene::image(&view(1.0), SIZE, 1.5);
    assert_eq!(img.size, [1134, 678]);
    assert!(img.pixels.iter().all(|p| *p == MARK));
    let (quick, prims, subject) = fake.asked.lock().unwrap()[0];
    assert!(!quick && prims > 100 && subject, "{prims} primitives");

    // The pick answers from the same geometry: the lid where the vectors put it.
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE);
    scene::set_blocking(true);
    let ctx = egui::Context::default();
    let mut sc = scene::Scene::default();
    frame(&ctx, &mut sc, &view(0.0), 0.0);
    let at = sc.point_on(Picked::Lid).expect("the lid on screen");
    assert_eq!(scene::pick(rect, &view(0.0), at), Some(Picked::Lid));

    // A lid turned glides: its frames ask for quick bakes; at rest, one whole one.
    fake.asked.lock().unwrap().clear();
    let mut t = 0.0;
    let mut glided = false;
    for _ in 0..40 {
        t += 0.05;
        glided |= frame(&ctx, &mut sc, &view(1.0), t);
    }
    assert!(glided, "the lid glides to where it is set");
    let asked = fake.asked.lock().unwrap().clone();
    assert!(asked.iter().any(|a| a.0), "a quick bake while it glides");
    assert!(!asked.last().unwrap().0, "the whole picture once it rests");
    let n = asked.len();
    for _ in 0..5 {
        t += 0.05;
        frame(&ctx, &mut sc, &view(1.0), t);
    }
    assert_eq!(fake.asked.lock().unwrap().len(), n, "nothing baked at rest");
    scene::backend::set_default(SceneBackend::Vector);
    assert_eq!(scene::backend::current(), SceneBackend::Vector);
}

#[test]
fn a_tags_anchor_stands_at_the_piece_it_names() {
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), SIZE);
    let v = view(1.0);
    let a = scene::anchor(rect, &v, Picked::Lid).expect("an anchor");
    assert!(rect.contains(a), "the lid's anchor at {a:?}");
    // Just below the anchor is the lid itself.
    assert_eq!(
        scene::pick(rect, &v, a + egui::vec2(0.0, 6.0)),
        Some(Picked::Lid)
    );
    // A microphone's anchor is its head, which picks it.
    let mic = scene::anchor(rect, &v, Picked::Mic(cantus_core::OutputId(1)))
        .expect("the player's pair drawn");
    assert_eq!(
        scene::pick(rect, &v, mic),
        Some(Picked::Mic(cantus_core::OutputId(1)))
    );
}
