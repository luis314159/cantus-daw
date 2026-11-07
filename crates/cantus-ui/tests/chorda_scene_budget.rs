//! Chorda's scene against the frame budget (round 37, ADR 0104): in steady state a frame
//! paints the cached mesh and allocates nothing beyond what an empty frame does; a glide
//! (a control turned) rebuilds into the buffers it has, allocating nothing once warm; the
//! scene stays under its triangle bound in every factory preset, and a rebuild is quick.
//! Its own test binary: a counting allocator for the thread under measurement.

use cantus_core::InstrumentKind;
use cantus_ui::views::inst::chorda::{scene, FACTORY_PRESETS};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::Instant;

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

const SIZE: egui::Vec2 = egui::vec2(1040.0, 600.0);

/// One frame: the scene for `inputs` (or nothing) in a full-window panel, tessellated.
/// Returns the allocations of building the frame and the triangles it drew.
fn frame(
    ctx: &egui::Context,
    s: &mut scene::Scene,
    inputs: Option<scene::Inputs>,
    t: f64,
) -> (u64, usize) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(t),
        predicted_dt: 1.0 / 60.0,
        ..Default::default()
    };
    let (out, allocs) = counted(|| {
        ctx.run_ui(input, |root| {
            if let Some(i) = inputs {
                let rect = root.max_rect();
                let painter = root.painter().clone();
                let _ = s.paint(&painter, rect, i, t);
            }
        })
    });
    let mut out = out;
    out.textures_delta.clear();
    let shapes = std::mem::take(&mut out.shapes);
    let prims = ctx.tessellate(shapes, out.pixels_per_point);
    let tris = prims
        .iter()
        .map(|p| match &p.primitive {
            egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
            _ => 0,
        })
        .sum();
    (allocs, tris)
}

#[test]
fn the_scene_costs_no_allocation_in_steady_state_nor_in_a_glide() {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    let mut s = scene::Scene::default();
    let base = scene::Inputs::of(&cantus_core::NativeSpec::new(InstrumentKind::Chorda), true);
    // The control: an empty frame's own allocations (egui's).
    let mut t = 0.0;
    for _ in 0..5 {
        t += 1.0 / 60.0;
        frame(&ctx, &mut s, None, t);
    }
    let empty = (0..5)
        .map(|_| {
            t += 1.0 / 60.0;
            frame(&ctx, &mut s, None, t).0
        })
        .max()
        .unwrap();
    // Until the full bake has landed (the worker's; seconds in a debug build).
    let start = Instant::now();
    loop {
        t += 1.0 / 60.0;
        frame(&ctx, &mut s, Some(base), t);
        if s.ready() || start.elapsed().as_secs() > 300 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(s.ready(), "the bake came");
    for _ in 0..3 {
        t += 1.0 / 60.0;
        frame(&ctx, &mut s, Some(base), t);
    }
    let builds = s.builds;
    let (steady, tris) = (0..10)
        .map(|_| {
            t += 1.0 / 60.0;
            frame(&ctx, &mut s, Some(base), t)
        })
        .fold((0, 0), |a, b| (a.0.max(b.0), b.1));
    assert_eq!(s.builds, builds, "steady state rebuilds nothing");
    assert!(
        steady <= empty + 1,
        "the scene allocates {steady} a frame against an empty frame's {empty}"
    );
    eprintln!("scene: {tris} triangles; {steady} allocations a frame (empty: {empty})");
    // A glide: the lid comes down and goes back up, twice — warm after the first.
    let mut shut = base;
    shut.lid = 0.0;
    for target in [shut, base] {
        for _ in 0..40 {
            t += 1.0 / 60.0;
            frame(&ctx, &mut s, Some(target), t);
        }
    }
    let mut worst = 0;
    for target in [shut, base] {
        for _ in 0..40 {
            t += 1.0 / 60.0;
            let up = s.uploads;
            let a = frame(&ctx, &mut s, Some(target), t).0;
            // A frame a finished bake lands on uploads its picture (once per bake, not
            // per frame): the glide's own frames are the ones measured.
            if s.uploads == up {
                worst = worst.max(a);
            }
        }
    }
    assert!(
        worst <= empty + 1,
        "a gliding frame allocates {worst} against an empty frame's {empty}"
    );
}

#[test]
fn every_presets_scene_stays_in_its_triangle_bound() {
    let mut most = 0;
    for p in FACTORY_PRESETS {
        let spec = cantus_ui::views::inst::preset_spec(InstrumentKind::Chorda, p);
        let i = scene::Inputs::of(&spec, true);
        let n = scene::triangles(&i, SIZE);
        assert!(n > 2_000, "{}: drawn", p.label_key);
        most = most.max(n);
    }
    eprintln!("the most triangles in a scene: {most}");
    assert!(most <= 400_000, "{most} triangles");
}

/// The bake's time (release only: `cargo test --release -- --ignored`), for the ADR.
#[test]
#[ignore]
fn measure_the_bake() {
    let spec = cantus_core::NativeSpec::new(InstrumentKind::Chorda);
    let i = scene::Inputs::of(&spec, true);
    for (q, name) in [(scene::FULL, "full"), (scene::PREVIEW, "preview")] {
        let ppp = if q.full { 1.0 } else { 0.5 };
        let t0 = Instant::now();
        let img = scene::render(&i, SIZE, ppp, q, 7, 16_384);
        eprintln!(
            "{name}: {}×{} in {:.0} ms",
            img.size[0],
            img.size[1],
            t0.elapsed().as_secs_f64() * 1e3
        );
    }
}

/// Every preset's scene bakes (a small preview, a debug build: overflow checks on), in
/// both lights.
#[test]
fn every_presets_scene_bakes() {
    // A small picture in both lights, and the Play view's own shape (a cable's end near
    // the lens projected far off a wide, short image once overflowed the line bounds).
    for (dark, size) in [
        (true, egui::vec2(160.0, 90.0)),
        (false, egui::vec2(160.0, 90.0)),
        (true, egui::vec2(690.0, 241.0)),
    ] {
        for p in FACTORY_PRESETS {
            let spec = cantus_ui::views::inst::preset_spec(InstrumentKind::Chorda, p);
            let i = scene::Inputs::of(&spec, dark);
            let img = scene::render(&i, size, 1.0, scene::FULL, 4, 16_384);
            assert_eq!(
                img.size,
                [size.x as usize, size.y as usize],
                "{}",
                p.label_key
            );
        }
    }
}

/// The worker bakes a look changed back and forth (the e2e's undo), and comes back.
#[test]
fn the_worker_comes_back_after_looks_change() {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    let mut s = scene::Scene::default();
    let felt = FACTORY_PRESETS
        .iter()
        .find(|p| p.label_key == "chorda.preset.felt")
        .unwrap();
    let spec = cantus_ui::views::inst::preset_spec(InstrumentKind::Chorda, felt);
    let up = scene::Inputs::of(&spec, true);
    let mut parlour = up;
    parlour.look.body = cantus_core::inst::chorda_look::Body::Parlour;
    let rect = egui::Rect::from_min_size(egui::pos2(20.0, 160.0), egui::vec2(690.0, 241.0));
    let mut t = 0.0;
    for target in [up, parlour, up] {
        let start = Instant::now();
        loop {
            t += 0.25;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 500.0),
                )),
                time: Some(t),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |root| {
                let painter = root.painter().clone();
                let _ = s.paint(&painter, rect, target, t);
            });
            out.textures_delta.clear();
            if s.ready() || start.elapsed().as_secs() > 120 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(s.ready(), "{target:?}: {s:?}");
    }
}

// ---- the review of the merged scene (round 37) ---------------------------------------

/// One frame of `s` painting `target` in `rect`; whether the frame asked for another.
fn paint_once(
    ctx: &egui::Context,
    s: &mut scene::Scene,
    rect: egui::Rect,
    target: scene::Inputs,
    t: f64,
) -> bool {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
        time: Some(t),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |root| {
        let painter = root.painter().clone();
        let _ = s.paint(&painter, rect, target, t);
    });
    out.textures_delta.clear();
    ctx.has_requested_repaint()
}

/// Run frames only when the app asks for one (as a window does), until `done` or 60 s.
fn reactive(
    ctx: &egui::Context,
    s: &mut scene::Scene,
    rect: egui::Rect,
    target: scene::Inputs,
    t: &mut f64,
    done: impl Fn(&scene::Scene) -> bool,
) {
    let start = Instant::now();
    let mut asked = true;
    while !done(s) && start.elapsed().as_secs() < 60 {
        if asked || ctx.has_requested_repaint() {
            *t += 1.0 / 60.0;
            asked = paint_once(ctx, s, rect, target, *t);
        } else {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

fn concert(dark: bool) -> scene::Inputs {
    scene::Inputs::of(&cantus_core::NativeSpec::new(InstrumentKind::Chorda), dark)
}

/// Review #1: a look changed and changed back while its bake is on the way: the late bake
/// is never shown over the look now there; and the worker wakes the UI when it lands (the
/// driver runs only the frames asked for).
#[test]
fn a_bake_for_a_look_undone_is_never_shown() {
    let ctx = egui::Context::default();
    let mut s = scene::Scene::default();
    let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(320.0, 180.0));
    let black = concert(true);
    let mut white = black;
    white.look.finish = cantus_core::inst::chorda_look::Finish::White;
    let mut t = 0.0;
    reactive(&ctx, &mut s, rect, black, &mut t, |s| s.ready());
    assert!(s.ready(), "{s:?}");
    scene::TEST_BAKE_DELAY_MS.store(400, std::sync::atomic::Ordering::Relaxed);
    t += 0.016;
    paint_once(&ctx, &mut s, rect, white, t);
    t += 0.016;
    paint_once(&ctx, &mut s, rect, black, t);
    reactive(&ctx, &mut s, rect, black, &mut t, |s| s.ready());
    scene::TEST_BAKE_DELAY_MS.store(0, std::sync::atomic::Ordering::Relaxed);
    assert!(
        s.ready(),
        "the late bake landed and the UI was woken: {s:?}"
    );
    let (shown, _) = s.showing().expect("a picture");
    assert_eq!(
        shown.look, black.look,
        "the picture shows the project's look"
    );
}

/// Review #2: a new size is baked without another event, and the scene is not ready while
/// the old size is stretched over the new.
#[test]
fn a_new_size_is_baked_without_waiting_for_an_event() {
    let ctx = egui::Context::default();
    let mut s = scene::Scene::default();
    let a = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(320.0, 180.0));
    let b = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 160.0));
    let i = concert(true);
    let mut t = 0.0;
    reactive(&ctx, &mut s, a, i, &mut t, |s| s.ready());
    assert!(s.ready());
    // One frame at the new size (the event that resized the window), then only the frames
    // asked for.
    t += 0.016;
    paint_once(&ctx, &mut s, b, i, t);
    assert!(!s.ready(), "not ready while the old size is shown: {s:?}");
    reactive(&ctx, &mut s, b, i, &mut t, |s| s.ready());
    assert!(s.ready(), "{s:?}");
    assert_eq!(s.showing().unwrap().1, [400, 160]);
}

/// Review #4: a bake never passes the GPU's largest texture side, nor the samples' cap.
#[test]
fn a_bake_is_bounded_by_the_texture_side_and_the_samples_cap() {
    let (ppp, ss) = scene::bake_shape(egui::vec2(800.0, 450.0), 1.0, scene::FULL, 16_384);
    assert_eq!((ppp, ss), (1.0, 2), "a normal picture: two samples a side");
    // A wall of 4K screens at 200 %.
    let size = egui::vec2(7_680.0, 2_160.0);
    let (ppp, ss) = scene::bake_shape(size, 2.0, scene::FULL, 16_384);
    let px = size.x * ppp * size.y * ppp * (ss * ss) as f32;
    assert!(px <= scene::MAX_SAMPLES as f32 * 1.001, "{px} samples");
    assert_eq!(ss, 1);
    let (ppp, _) = scene::bake_shape(egui::vec2(4_000.0, 300.0), 1.0, scene::FULL, 2_048);
    assert!(
        4_000.0 * ppp <= 2_048.0 + 0.5,
        "the widest side fits the texture"
    );
}

/// Review #5: the whole scene card (the pickers, the caption, the hint) at rest allocates
/// next to nothing beyond an empty panel.
#[test]
fn the_scene_card_at_rest_allocates_next_to_nothing() {
    use cantus_core::{InstrumentSpec, NativeSpec, Project};
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let mut p = Project::new("Card");
    let mut tr = p.make_track("Piano");
    tr.instrument = InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Chorda));
    let track = tr.id;
    p.tracks.push(tr);
    let (engine, _proc) = cantus_engine::EngineHandle::manual(48_000.0);
    let mut app = cantus_ui::CantusApp::from_parts(Some(p), engine, false);
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), egui::vec2(700.0, 420.0));
    let mut t = 0.0;
    let run = |app: &mut cantus_ui::CantusApp, card: bool, t: f64| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE)),
            time: Some(t),
            ..Default::default()
        };
        let (out, allocs) = counted(|| {
            ctx.run_ui(input, |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    if card {
                        cantus_ui::views::inst::chorda::scene_card_for_tests(app, ui, track, rect);
                    }
                });
            })
        });
        let mut out = out;
        out.textures_delta.clear();
        allocs
    };
    // Warm, until the bake has landed.
    let start = Instant::now();
    while start.elapsed().as_secs() < 60 {
        t += 1.0 / 60.0;
        run(&mut app, true, t);
        if app.inst.chorda.scene_of(track).is_some_and(|s| s.ready()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(app.inst.chorda.scene_of(track).is_some_and(|s| s.ready()));
    for _ in 0..5 {
        t += 1.0 / 60.0;
        run(&mut app, true, t);
    }
    let card = (0..10)
        .map(|_| {
            t += 1.0 / 60.0;
            run(&mut app, true, t)
        })
        .max()
        .unwrap();
    let empty = (0..10)
        .map(|_| {
            t += 1.0 / 60.0;
            run(&mut app, false, t)
        })
        .max()
        .unwrap();
    eprintln!("the scene card at rest: {card} allocations a frame (an empty panel: {empty})");
    assert!(card <= empty + 6, "{card} against {empty}");
}
