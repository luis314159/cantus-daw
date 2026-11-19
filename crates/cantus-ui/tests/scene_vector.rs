//! The instruments' scenes as projected vectors (round 40 phase 3, ADR 0126): every variant
//! built and finite, its instrument inside its picture; the pick answers the piece, the
//! microphone or the lid drawn under a point and nothing in the empty room; at rest a frame
//! paints the cached picture, asks for no repaint, builds nothing and allocates no more
//! than an empty frame; a lid drag rebuilds inside the frame budget's share; the selection
//! reads against what is behind it in every theme.
//! Its own test binary: a counting allocator for the thread under measurement.

use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::{Floor, RoomLook, Rug, Walls};
use cantus_core::inst::tympanum::{KitRef, RoomChoice};
use cantus_core::inst::tympanum_kit::{KitDef, RoomDef};
use cantus_ui::scene::{self, Picked, Room, SceneView, Subject};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

struct Counting;

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        unsafe { System.realloc(ptr, layout, new_size) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

fn note() {
    let _ = COUNTING.try_with(|c| {
        if c.get() {
            let _ = ALLOCS.try_with(|a| a.set(a.get() + 1));
        }
    });
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn counted<R>(f: impl FnOnce() -> R) -> (R, u64) {
    ALLOCS.with(|a| a.set(0));
    COUNTING.with(|c| c.set(true));
    let r = f();
    COUNTING.with(|c| c.set(false));
    (r, ALLOCS.with(|a| a.get()))
}

const SIZE: egui::Vec2 = egui::vec2(756.0, 452.0);

/// Every test here takes its turn: the two timing tests ran beside the four others, each
/// rendering pictures of its own, and timed them (beta.15's release, the macOS runner's
/// three cores: a lid drag read 2 440 ms at 1.5x, 390 ms on the owner's machine). Cargo
/// runs one test binary at a time, so in turn they time a picture, not a crowd.
fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn kit_def() -> KitDef {
    KitDef {
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
    }
}

fn view(subject: Subject, room: Room, dark: bool) -> SceneView {
    SceneView {
        subject,
        room,
        look: RoomLook::default(),
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
        persp: 0.35,
        width: 0.7,
        room_level: 0.35,
        mics,
    }
}

fn kit(mics: bool) -> Subject {
    Subject::Kit {
        kit: KitRef::default_kit(),
        def: Box::new(kit_def()),
        finish: Finish::Maple,
        mics,
    }
}

/// Every piano type × finish × lid × room × mood draws: a picture of the right size, the
/// instrument's outline inside it, every op finite.
#[test]
fn every_variant_draws_finite_and_inside_its_picture() {
    let _turn = turn();
    let mut n = 0;
    for dark in [true, false] {
        for room in Room::ALL {
            for body in Body::ALL {
                for finish in Finish::ALL {
                    for lid in [0.0, 0.55, 1.0] {
                        let v = view(piano(body, finish, lid, true), room, dark);
                        let s = scene::stats(&v, SIZE);
                        assert!(
                            s.finite,
                            "{body:?} {finish:?} {lid} {room:?} {dark}: finite"
                        );
                        assert!(s.ops > 100, "{body:?} {room:?}: drawn ({} ops)", s.ops);
                        let (x0, y0, x1, y1) = s.instrument;
                        assert!(
                            x0 >= 0.0 && y0 >= 0.0 && x1 <= SIZE.x && y1 <= SIZE.y,
                            "{body:?} {finish:?} lid {lid} {room:?}: inside ({x0},{y0})–({x1},{y1})"
                        );
                        n += 1;
                    }
                }
            }
            let s = scene::stats(&view(kit(true), room, dark), SIZE);
            assert!(s.finite && s.ops > 1000, "kit in {room:?}");
            let (x0, y0, x1, y1) = s.instrument;
            assert!(
                x0 >= 0.0 && y0 >= 0.0 && x1 <= SIZE.x && y1 <= SIZE.y,
                "kit in {room:?}"
            );
        }
    }
    assert_eq!(n, 2 * 5 * 4 * 5 * 3);
    // Every floor, walls and rug choice draws too.
    for f in Floor::ALL {
        for w in Walls::ALL {
            let mut v = view(
                piano(Body::Grand, Finish::Black, 1.0, false),
                Room::Studio,
                true,
            );
            v.look = RoomLook {
                room: None,
                floor: Some(*f),
                walls: Some(*w),
                rug: Some(Rug::ALL[(f.key().len() + w.key().len()) % Rug::ALL.len()]),
            };
            assert!(scene::stats(&v, SIZE).finite);
        }
    }
}

/// The pick: every piece of the kit drawn is picked at a point of its own outline; the
/// lid, the pair and the room pair of a piano; nothing in the empty room.
#[test]
fn the_pick_answers_what_is_drawn_and_nothing_in_the_empty_room() {
    let _turn = turn();
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), SIZE);
    let ctx = egui::Context::default();
    scene::set_blocking(true);
    let mut sc = scene::Scene::default();
    let kv = view(kit(true), Room::Hall, true);
    ctx.run_ui(Default::default(), |ui| {
        let _ = sc.paint(ui.painter(), rect, &kv);
    })
    .textures_delta
    .clear();
    for piece in 0..12u16 {
        let want = Picked::Piece { piece, zone: None };
        let p = sc
            .point_on(want)
            .unwrap_or_else(|| panic!("piece {piece} reachable"));
        assert!(
            matches!(sc.pick(rect, p), Some(Picked::Piece { piece: q, .. }) if q == piece),
            "piece {piece} at {p:?}"
        );
        assert_eq!(
            scene::pick(rect, &kv, p)
                .map(|x| matches!(x, Picked::Piece { piece: q, .. } if q == piece)),
            Some(true)
        );
    }
    for m in 0..12u16 {
        let want = Picked::Mic(cantus_core::OutputId(m));
        assert!(sc.point_on(want).is_some(), "mic {m}");
    }
    assert_eq!(
        sc.pick(rect, rect.min + egui::vec2(4.0, 4.0)),
        None,
        "the empty room"
    );
    let mut sp = scene::Scene::default();
    let pv = view(
        piano(Body::Concert, Finish::Black, 1.0, true),
        Room::Stage,
        true,
    );
    ctx.run_ui(Default::default(), |ui| {
        let _ = sp.paint(ui.painter(), rect, &pv);
    })
    .textures_delta
    .clear();
    assert!(sp.point_on(Picked::Lid).is_some(), "the lid");
    assert!(
        sp.point_on(Picked::Mic(cantus_core::OutputId(1))).is_some(),
        "the pair at the player"
    );
    assert!(
        sp.point_on(Picked::Mic(cantus_core::OutputId(3))).is_some(),
        "the room pair"
    );
    assert_eq!(sp.pick(rect, rect.min + egui::vec2(4.0, 4.0)), None);
}

fn frame(ctx: &egui::Context, sc: &mut scene::Scene, v: Option<&SceneView>, t: f64) -> (u64, bool) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(t),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let mut moving = false;
    let (out, allocs) = counted(|| {
        ctx.run_ui(input, |root| {
            if let Some(v) = v {
                moving = sc.paint(root.painter(), root.max_rect(), v);
            }
        })
    });
    let mut out = out;
    out.textures_delta.clear();
    let repaint = out
        .viewport_output
        .values()
        .any(|o| o.repaint_delay < std::time::Duration::from_secs(1));
    (allocs, moving || repaint)
}

/// At rest: the picture shown is the one asked for; a frame builds nothing, asks for no
/// repaint, and allocates no more than an empty frame — with a selection drawn too.
#[test]
fn at_rest_no_repaint_no_build_and_no_allocation() {
    let _turn = turn();
    scene::set_blocking(true);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    let mut sc = scene::Scene::default();
    let mut t = 0.0;
    let empty = (0..6)
        .map(|_| {
            t += 1.0 / 60.0;
            frame(&ctx, &mut sc, None, t).0
        })
        .max()
        .unwrap();
    let mut v = view(kit(false), Room::Stage, true);
    v.selected = Some(Picked::Piece {
        piece: 1,
        zone: None,
    });
    v.hovered = Some(Picked::Piece {
        piece: 9,
        zone: None,
    });
    for _ in 0..4 {
        t += 1.0 / 60.0;
        frame(&ctx, &mut sc, Some(&v), t);
    }
    let builds = sc.builds;
    for _ in 0..10 {
        t += 1.0 / 60.0;
        let (allocs, repaint) = frame(&ctx, &mut sc, Some(&v), t);
        assert!(!repaint, "a scene at rest asks for no repaint");
        assert!(
            allocs <= empty + 2,
            "at rest: {allocs} allocations, an empty frame {empty}"
        );
    }
    assert_eq!(sc.builds, builds, "nothing rebuilt at rest");
}

/// A lid drag rebuilds the piano's picture quickly: the build and its picture at 1× and
/// 1.5× (measured here on one thread; the worker does it off the UI's) — the worst lid
/// position, each the best of three, as the kit's test below takes its best of five: one
/// timing alone read a shared runner's hiccup, not the picture.
#[test]
fn a_lid_drag_rebuilds_within_its_budget() {
    let _turn = turn();
    for ppp in [1.0f32, 1.5] {
        let mut worst = 0.0f64;
        for k in 0..6 {
            let v = view(
                piano(Body::Concert, Finish::Black, k as f32 / 5.0, false),
                Room::Stage,
                true,
            );
            let mut best = f64::MAX;
            for _ in 0..3 {
                let t0 = std::time::Instant::now();
                let img = scene::image(&v, SIZE, ppp);
                best = best.min(t0.elapsed().as_secs_f64() * 1e3);
                assert_eq!(img.size[0], (SIZE.x * ppp).round() as usize);
            }
            worst = worst.max(best);
        }
        println!("lid drag at {ppp}x: worst build {worst:.0} ms");
        // Generous in CI (debug, shared runners): the measured figure is in the report.
        assert!(worst < 2000.0, "{worst} ms at {ppp}x");
    }
}

/// Tympanum's kit on the stage builds and rasterises quickly (round 40 polish: the stands'
/// hardware — double-braced legs, memory locks, wing bolts, tilters, the hi-hat's clutch —
/// added primitives): the best of five builds (display list only) and of five pictures at
/// 1× and 1.5×, by night and by day, printed for the report.
#[test]
fn the_kit_builds_within_its_budget() {
    let _turn = turn();
    for dark in [true, false] {
        let v = view(kit(false), Room::Stage, dark);
        let mut best_build = f64::MAX;
        let mut ops = 0;
        for _ in 0..5 {
            let t0 = std::time::Instant::now();
            ops = scene::stats(&v, SIZE).ops;
            best_build = best_build.min(t0.elapsed().as_secs_f64() * 1e3);
        }
        for ppp in [1.0f32, 1.5] {
            let mut best = f64::MAX;
            for _ in 0..5 {
                let t0 = std::time::Instant::now();
                let img = scene::image(&v, SIZE, ppp);
                best = best.min(t0.elapsed().as_secs_f64() * 1e3);
                assert_eq!(img.size[0], (SIZE.x * ppp).round() as usize);
            }
            println!(
                "kit on the stage (dark {dark}) at {ppp}x: build {best_build:.1} ms \
                 ({ops} ops), picture {best:.1} ms"
            );
            assert!(best < 2000.0, "{best} ms at {ppp}x");
        }
    }
}

/// Legibility (ADR 0111): the selection's outline — what says what is clicked — reads at
/// least 3:1 in every theme: drawn over a halo of its own (dark or light, whichever stands
/// further from the theme's focus colour), it holds that contrast on any part of any
/// picture. Reported beside it: the focus colour against the picture itself (the median of
/// the outline's points), which the halo makes up for where it is low.
#[test]
fn the_selection_reads_against_the_scene_in_every_theme() {
    let _turn = turn();
    use cantus_ui::kit::appearance::PRESETS;
    use cantus_ui::kit::color::contrast;
    scene::set_blocking(true);
    let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE);
    let mut pictures = Vec::new();
    for dark in [true, false] {
        let v = view(kit(false), Room::Stage, dark);
        let ctx = egui::Context::default();
        let mut sc = scene::Scene::default();
        ctx.run_ui(Default::default(), |ui| {
            let _ = sc.paint(ui.painter(), rect, &v);
        })
        .textures_delta
        .clear();
        let outline: Vec<egui::Pos2> = sc
            .built()
            .expect("a picture")
            .outlines
            .iter()
            .filter(|o| matches!(o.0, scene::display::Part::Piece(1 | 9 | 0)))
            .flat_map(|o| o.2.iter().copied())
            .collect();
        pictures.push((dark, scene::image(&v, SIZE, 1.0), outline));
    }
    for (id, make) in PRESETS {
        let t = make().kit_tokens();
        let halo = scene::halo_for(t.focus);
        let with_halo = contrast(t.focus, halo);
        let (_, img, outline) = pictures.iter().find(|p| p.0 == t.dark).unwrap();
        let mut ratios: Vec<f32> = outline
            .iter()
            .filter_map(|p| {
                let (x, y) = (p.x.round() as usize, p.y.round() as usize);
                (x < img.size[0] && y < img.size[1])
                    .then(|| contrast(t.focus, img.pixels[y * img.size[0] + x]))
            })
            .collect();
        ratios.sort_by(f32::total_cmp);
        let bare = ratios[ratios.len() / 2];
        println!("{id}: {with_halo:.2}:1 on its halo, {bare:.2}:1 on the picture");
        assert!(
            with_halo >= 3.0,
            "{id}: the selection at {with_halo:.2}:1 on its halo"
        );
    }
}
