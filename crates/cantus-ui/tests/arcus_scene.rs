//! r42:arcus-look — Arcus's picture on the scene engine (ADR 0126): the cello and the
//! double bass standing in every room in both moods draw finite and inside their picture,
//! the pick finds the instrument and nothing in the empty room; through the app
//! (`CantusApp`, the editor's own picture with the live string, finger and bow), a frame
//! at rest builds nothing, asks for no repaint and allocates next to nothing, and what a
//! frame costs stands beside Chorda's picture's (a ratio, timed beside it: RELEASING L3).

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, TrackId};
use cantus_ui::scene::{self, bowed::Bowed, Room, SceneView, Subject};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct Counting;

thread_local! {
    static ALLOCS: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note();
        System.alloc(layout)
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note();
        System.alloc_zeroed(layout)
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note();
        System.realloc(ptr, layout, new_size)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn note() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}

fn counted<R>(f: impl FnOnce() -> R) -> (R, u64) {
    let before = ALLOCS.with(|c| c.get());
    let r = f();
    (r, ALLOCS.with(|c| c.get()) - before)
}

/// One binary's timing tests take turns (RELEASING L3).
fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn view(b: Bowed, room: Room, dark: bool) -> SceneView {
    SceneView {
        subject: Subject::Bowed {
            instrument: b,
            holder: Default::default(), // r42:arcus-upper
        },
        room,
        look: Default::default(),
        dark,
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: egui::Color32::TRANSPARENT,
        ink: egui::Color32::TRANSPARENT,
    }
}

const SIZE: egui::Vec2 = egui::vec2(1040.0, 680.0);

#[test]
fn every_bowed_scene_draws_finite_and_inside_its_picture() {
    let size = egui::vec2(590.0, 420.0);
    for b in [Bowed::Cello, Bowed::Bass, Bowed::Violin, Bowed::Viola] {
        // r42:arcus-upper
        for room in Room::ALL {
            for dark in [true, false] {
                let st = scene::stats(&view(b, room, dark), size);
                assert!(st.finite, "{b:?} in {room:?}: every point finite");
                assert!(st.ops > 2000, "{b:?}: a real instrument ({} ops)", st.ops);
                let (x0, y0, x1, y1) = st.instrument;
                assert!(
                    x0 >= 0.0 && y0 >= 0.0 && x1 <= size.x && y1 <= size.y,
                    "{b:?} in {room:?}: inside the picture ({x0} {y0} {x1} {y1})"
                );
                if b.dims().held {
                    // r42:arcus-upper — held at the playing angle: wider than tall, filling
                    // the picture's width.
                    assert!(x1 - x0 > 0.6 * size.x && x1 - x0 > 1.2 * (y1 - y0), "{b:?}");
                } else {
                    // Standing: taller than wide, filling the picture's height above the
                    // caption's band.
                    assert!(y1 - y0 > 0.75 * size.y && y1 - y0 > 1.4 * (x1 - x0));
                }
                assert!(y1 <= size.y - 50.0, "{b:?}: the caption's band is free");
            }
        }
    }
}

#[test]
fn the_pick_finds_the_instrument_and_nothing_in_the_empty_room() {
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(590.0, 420.0));
    for b in [Bowed::Cello, Bowed::Bass, Bowed::Violin, Bowed::Viola] {
        // r42:arcus-upper
        let v = view(b, Room::Studio, true);
        let pts = scene::instrument_points(rect, &v);
        assert!(
            pts.len() > 200,
            "{b:?}: the instrument covers its picture ({})",
            pts.len()
        );
        let mid = pts.iter().fold(egui::Vec2::ZERO, |a, p| a + p.to_vec2()) / pts.len() as f32;
        assert!(
            (mid.x - rect.center().x).abs() < rect.width() * 0.12,
            "{b:?}: in the middle"
        );
        assert_eq!(scene::pick(rect, &v, rect.min + egui::vec2(6.0, 6.0)), None);
    }
}

fn app_with(kind: InstrumentKind, inst: f32) -> (cantus_ui::CantusApp, TrackId) {
    let mut p = Project::new("Pictures");
    let mut tr = p.make_track("Strings");
    let mut spec = NativeSpec::new(kind);
    if kind == InstrumentKind::BowedString {
        spec.set("inst", inst);
    }
    tr.instrument = InstrumentSpec::Native(spec);
    let track = tr.id;
    p.tracks.push(tr);
    let (engine, _proc) = cantus_engine::EngineHandle::manual(48_000.0);
    std::mem::forget(_proc);
    (
        cantus_ui::CantusApp::from_parts(Some(p), engine, false),
        track,
    )
}

/// One frame of `kind`'s picture: its allocations, whether it asks for another frame, its
/// time.
fn frame(
    ctx: &egui::Context,
    app: &mut cantus_ui::CantusApp,
    track: TrackId,
    kind: InstrumentKind,
    t: f64,
) -> (u64, bool, f64) {
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(590.0, 420.0));
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(t),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    let (out, allocs) = counted(|| {
        ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show(root, |ui| match kind {
                InstrumentKind::Chorda => {
                    cantus_ui::views::inst::chorda::scene_card_for_tests(app, ui, track, rect)
                }
                _ => cantus_ui::views::inst::arcus::picture_for_tests(app, ui, track, rect),
            });
        })
    });
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    let mut out = out;
    out.textures_delta.clear();
    let repaint = out
        .viewport_output
        .values()
        .any(|o| o.repaint_delay < std::time::Duration::from_secs(1));
    (allocs, repaint, ms)
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// At rest (no note): the picture shown is the one asked for — nothing rebuilt, no repaint
/// asked for, and next to nothing allocated beyond an empty frame.
#[test]
fn at_rest_the_picture_builds_nothing_and_asks_for_nothing() {
    let _turn = turn();
    scene::set_blocking(true);
    // r42:arcus-upper — the violin too, and each holder (a figure's hands settle, then
    // ask for nothing).
    use cantus_ui::scene::bowed_hold::{Hold, Holder};
    let cases = [0.0, 1.0, 2.0]
        .into_iter()
        .flat_map(|i| Hold::ALL.into_iter().map(move |h| (i, h)));
    for (inst, hold) in cases {
        let ctx = context();
        let (mut app, track) = app_with(InstrumentKind::BowedString, inst);
        app.settings.arcus_holder = Holder::of(hold);
        let mut t = 0.0;
        for _ in 0..4 {
            t += 1.0 / 60.0;
            frame(&ctx, &mut app, track, InstrumentKind::BowedString, t);
        }
        let empty = {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
                time: Some(t),
                ..Default::default()
            };
            (0..4)
                .map(|_| {
                    let (mut out, n) = counted(|| {
                        ctx.run_ui(input.clone(), |root| {
                            egui::CentralPanel::default().show(root, |_| {});
                        })
                    });
                    out.textures_delta.clear();
                    n
                })
                .max()
                .unwrap_or(0)
        };
        let builds = app
            .inst
            .arcus_scenes
            .values()
            .map(|s| s.0.builds)
            .sum::<u64>();
        for _ in 0..8 {
            t += 1.0 / 60.0;
            let (allocs, repaint, _) = frame(&ctx, &mut app, track, InstrumentKind::BowedString, t);
            eprintln!("at rest: {allocs} allocations (an empty frame {empty})");
            assert!(
                !repaint,
                "at rest ({inst}, {hold:?}) the picture asks for no repaint"
            );
            assert!(
                allocs <= empty + 6,
                "at rest ({inst}, {hold:?}): {allocs} allocations, an empty frame {empty}"
            );
        }
        let now = app
            .inst
            .arcus_scenes
            .values()
            .map(|s| s.0.builds)
            .sum::<u64>();
        assert_eq!(now, builds, "nothing rebuilt at rest");
    }
}

/// A note bowed: the live picture (the lit string, the finger, the bow sliding) costs a
/// frame of the same order as Chorda's picture at rest — timed beside it, best of five.
#[test]
fn the_live_picture_costs_what_chordas_does() {
    let _turn = turn();
    scene::set_blocking(true);
    let ctx = context();
    let (mut arcus, at) = app_with(InstrumentKind::BowedString, 0.0);
    cantus_ui::views::inst::arcus::demo_feed(&mut arcus);
    let (mut chorda, ct) = app_with(InstrumentKind::Chorda, 0.0);
    let mut t = 0.0;
    for _ in 0..6 {
        t += 1.0 / 60.0;
        frame(&ctx, &mut arcus, at, InstrumentKind::BowedString, t);
        frame(&ctx, &mut chorda, ct, InstrumentKind::Chorda, t);
    }
    let mut ratios = Vec::new();
    for _ in 0..5 {
        let (mut a, mut c) = (0.0, 0.0);
        for _ in 0..30 {
            t += 1.0 / 60.0;
            a += frame(&ctx, &mut arcus, at, InstrumentKind::BowedString, t).2;
            c += frame(&ctx, &mut chorda, ct, InstrumentKind::Chorda, t).2;
        }
        ratios.push(a / c.max(1e-6));
    }
    ratios.sort_by(f64::total_cmp);
    let best = ratios[0];
    eprintln!("Arcus live / Chorda at rest, a frame: {ratios:.2?}");
    let one = |app: &mut cantus_ui::CantusApp, tr: TrackId, k: InstrumentKind, t: &mut f64| {
        (0..30)
            .map(|_| {
                *t += 1.0 / 60.0;
                frame(&ctx, app, tr, k, *t).2
            })
            .fold(f64::MAX, f64::min)
    };
    let (a, c) = (
        one(&mut arcus, at, InstrumentKind::BowedString, &mut t),
        one(&mut chorda, ct, InstrumentKind::Chorda, &mut t),
    );
    eprintln!("a frame, best of 30: Arcus live {a:.3} ms, Chorda at rest {c:.3} ms");
    assert!(
        best < 6.0,
        "a live Arcus frame costs {best:.2}× a Chorda frame at rest"
    );
    let builds = arcus
        .inst
        .arcus_scenes
        .values()
        .map(|s| s.0.builds)
        .sum::<u64>();
    assert_eq!(
        builds, 1,
        "playing rebuilds no picture: only what it plays is drawn"
    );
}
