//! r40:scene-px — the photographic renderer (the *Photographic* instrument pictures): every
//! variant of the designer's scenes renders (each piano body, finish and lid, each room and
//! theme, the kit with and without its microphones), within the bake's bounds; the bits
//! never depend on how many threads bake; the bake's time and peak at the card's size
//! (release, ignored by default: `cargo test --release -p cantus-ui --test scene_px -- --ignored`).

use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::{Floor, RoomLook, Rug, Walls};
use cantus_core::inst::tympanum::{KitRef, RoomChoice};
use cantus_core::inst::tympanum_kit::{KitDef, RoomDef};
use cantus_ui::scene::{Room, SceneView, Subject};
use cantus_ui::views::inst::chorda::scene::hybrid::{self, Quality};

fn empty_kit() -> KitDef {
    KitDef {
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
        finish: cantus_core::inst::chorda_look::Finish::Maple,
    }
}

fn view(subject: Subject, room: Room, dark: bool, look: RoomLook) -> SceneView {
    SceneView {
        subject,
        room,
        look,
        dark,
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: egui::Color32::from_rgb(92, 175, 233),
        ink: egui::Color32::from_rgb(227, 232, 237),
    }
}

fn piano(body: Body, finish: Finish, lid: f32, mics: bool) -> Subject {
    Subject::Piano {
        look: Look::new(body, finish),
        lid,
        persp: 0.5,
        width: 0.6,
        room_level: 0.4,
        mics,
    }
}

fn kit(finish: Finish, mics: bool) -> Subject {
    Subject::Kit {
        kit: KitRef::default_kit(),
        def: Box::new(empty_kit()),
        finish,
        mics,
    }
}

/// Small and quick: one sample, no occlusion or mirror.
const QUICK: Quality = Quality {
    ss: 1,
    full: false,
    shadow: 512,
    trace: true,
};

/// What a picture holds: its luminance's spread, and how far its middle is from its corners.
fn spread(img: &egui::ColorImage) -> (f32, f32) {
    let [w, h] = img.size;
    let lum =
        |c: egui::Color32| 0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32;
    let all: Vec<f32> = img.pixels.iter().map(|&c| lum(c)).collect();
    let mean = all.iter().sum::<f32>() / all.len() as f32;
    let var = all.iter().map(|l| (l - mean).powi(2)).sum::<f32>() / all.len() as f32;
    let mid: f32 = (h / 3..2 * h / 3)
        .flat_map(|y| (w / 3..2 * w / 3).map(move |x| (x, y)))
        .map(|(x, y)| all[y * w + x])
        .sum::<f32>()
        / ((h / 3) * (w / 3)) as f32;
    let corner: f32 = (0..h / 8)
        .flat_map(|y| (0..w / 8).map(move |x| (x, y)))
        .map(|(x, y)| all[y * w + x])
        .sum::<f32>()
        / ((h / 8) * (w / 8)) as f32;
    (var.sqrt(), (mid - corner).abs())
}

fn check(name: &str, v: &SceneView, q: Quality) {
    let size = egui::vec2(240.0, 144.0);
    let (img, st) = hybrid::render(v, size, 1.0, q, 2);
    assert_eq!(img.size, [240, 144], "{name}: the picture's size");
    let (sd, _) = spread(&img);
    assert!(sd > 4.0, "{name}: a picture, not a flat fill (spread {sd})");
    assert!(
        st.tris > 1000 && st.peak > 0,
        "{name}: a scene was built and baked ({} triangles)",
        st.tris
    );
}

#[test]
fn every_piano_body_finish_and_lid_renders() {
    let finishes = [
        Finish::Black,
        Finish::White,
        Finish::Walnut,
        Finish::Rosewood,
        Finish::Maple,
    ];
    let bodies = [Body::Concert, Body::Grand, Body::Parlour, Body::Upright];
    for (i, body) in bodies.into_iter().enumerate() {
        for (j, finish) in finishes.into_iter().enumerate() {
            let lid = [0.0, 0.55, 1.0][(i + j) % 3];
            let mics = (i + j) % 2 == 0;
            let v = view(
                piano(body, finish, lid, mics),
                Room::Stage,
                (i + j) % 2 == 1,
                RoomLook::default(),
            );
            check(&format!("{body:?} {finish:?} lid {lid}"), &v, QUICK);
        }
    }
}

#[test]
fn every_room_and_theme_renders_both_instruments() {
    for room in Room::ALL {
        for dark in [true, false] {
            let v = view(
                piano(Body::Grand, Finish::Black, 1.0, true),
                room,
                dark,
                RoomLook::default(),
            );
            check(&format!("piano {room:?} dark {dark}"), &v, QUICK);
            let v = view(kit(Finish::Maple, !dark), room, dark, RoomLook::default());
            check(&format!("kit {room:?} dark {dark}"), &v, QUICK);
        }
    }
}

#[test]
fn every_floor_wall_and_rug_renders_with_every_effect() {
    let floors = [
        Floor::LightOak,
        Floor::HoneyOak,
        Floor::DarkWalnut,
        Floor::StageWalnut,
        Floor::EbonyStage,
        Floor::Concrete,
        Floor::Terrazzo,
        Floor::Flagstone,
    ];
    let walls = [
        Walls::WarmPlaster,
        Walls::WarmWood,
        Walls::WalnutPanels,
        Walls::Plaster,
        Walls::Charcoal,
        Walls::BlueFabric,
        Walls::GreenFabric,
        Walls::Brick,
        Walls::HallWood,
        Walls::Limestone,
        Walls::StageDark,
    ];
    let rugs = [Rug::Red, Rug::Blue, Rug::Grey, Rug::Bare];
    for (i, w) in walls.into_iter().enumerate() {
        let look = RoomLook {
            room: None,
            floor: Some(floors[i % floors.len()]),
            walls: Some(w),
            rug: Some(rugs[i % rugs.len()]),
        };
        let full = Quality {
            ss: 1,
            full: true,
            shadow: 512,
            trace: true,
        };
        let v = view(
            piano(Body::Parlour, Finish::Walnut, 0.5, false),
            Room::Studio,
            i % 2 == 0,
            look,
        );
        check(&format!("{look:?}"), &v, full);
    }
}

#[test]
fn the_bits_never_depend_on_the_threads() {
    let v = view(
        piano(Body::Concert, Finish::Black, 1.0, true),
        Room::Stage,
        true,
        RoomLook::default(),
    );
    let q = Quality {
        ss: 2,
        full: true,
        shadow: 512,
        trace: true,
    };
    let size = egui::vec2(200.0, 120.0);
    let (a, _) = hybrid::render(&v, size, 1.0, q, 1);
    let (b, _) = hybrid::render(&v, size, 1.0, q, 5);
    assert!(
        a.pixels == b.pixels,
        "one thread and five bake the same picture"
    );
}

#[test]
fn a_bake_stays_inside_its_bounds() {
    use hybrid::{shape, MAX_SAMPLES, MAX_SIDE};
    // A multi-4K window's card at 2 px/pt: one sample a side, then smaller.
    let (w, h, q) = shape(egui::vec2(3800.0, 2200.0), 2.0, hybrid::FULL);
    assert!(
        w * h * q.ss * q.ss <= MAX_SAMPLES,
        "{w}x{h} at {} samples a side",
        q.ss
    );
    assert!(w <= MAX_SIDE && h <= MAX_SIDE);
    assert!(
        (w as f32 / h as f32 - 3800.0 / 2200.0).abs() < 0.01,
        "the picture keeps its shape"
    );
    // The card at 1.5 px/pt keeps two samples a side.
    let (w, h, q) = shape(egui::vec2(756.0, 452.0), 1.5, hybrid::FULL);
    assert_eq!((w, h, q.ss), (1134, 678, 2));
    // A tiny one is never empty.
    let (w, h, _) = shape(egui::vec2(1.0, 1.0), 1.0, hybrid::FULL);
    assert!(w >= 8 && h >= 8);
}

/// The card's bake at 1.5 px/pt: its time and its peak (printed for the round's report).
#[test]
#[ignore]
fn the_cards_bake_time_and_peak() {
    let size = egui::vec2(756.0, 452.0);
    for (name, v) in [
        (
            "chorda stage night",
            view(
                piano(Body::Concert, Finish::Black, 1.0, false),
                Room::Stage,
                true,
                RoomLook::default(),
            ),
        ),
        (
            "tympanum hall night",
            view(
                kit(Finish::Maple, true),
                Room::Hall,
                true,
                RoomLook::default(),
            ),
        ),
    ] {
        let mut best = f64::MAX;
        let mut peak = 0;
        for _ in 0..3 {
            let (_, st) = hybrid::render(&v, size, 1.5, hybrid::FULL, hybrid::threads());
            best = best.min(st.ms);
            peak = st.peak;
        }
        println!(
            "{name}: best of 3 {best:.0} ms, peak {:.0} MB",
            peak as f64 / 1e6
        );
        assert!(best < 2500.0, "{name}: {best:.0} ms");
        assert!(peak < 160_000_000, "{name}: {peak} bytes");
    }
}

/// The bake runs on one pool kept between bakes, its threads below normal priority (never
/// taking the processor from the audio or the interface), and the pool can go when idle.
#[test]
fn the_bake_pool_is_shared_low_priority_and_lets_go() {
    let v = view(
        piano(Body::Grand, Finish::Black, 1.0, false),
        Room::Stage,
        true,
        RoomLook::default(),
    );
    let _ = hybrid::render(&v, egui::vec2(120.0, 72.0), 1.0, QUICK, hybrid::threads());
    assert!(hybrid::pool::alive(), "the pool stays after a bake");
    let pool = hybrid::pool::pool().expect("a pool");
    assert_eq!(pool.current_num_threads(), hybrid::pool::workers());
    assert!((1..=6).contains(&hybrid::pool::workers()));
    let p = pool.install(cantus_rt::thread::current_priority);
    #[cfg(windows)]
    assert_eq!(p, Some(-1), "THREAD_PRIORITY_BELOW_NORMAL");
    let _ = p;
    drop(pool);
    // Let go when no bake holds it (another test's bake may: then it stays, as it should).
    hybrid::pool::release();
}
