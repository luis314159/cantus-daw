//! The Session pane (round 33, ADR 0077): it says no bare word and draws no stock widget
//! (the kit's lint over its files), and it holds the frame budget — a 16 × 32 session with
//! clips playing, queued and recording costs a bounded number of allocations and draw
//! calls a frame, and an idle grid asks for no repaint.
//!
//! Timing (`#[ignore]`; `cargo test --release -p cantus-ui --test session_view --
//! --ignored --nocapture`): the median and 95th percentile of build + tessellate.

use cantus_engine::EngineHandle;
use cantus_ui::views::session;
use cantus_ui::CantusApp;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ON.try_with(|on| {
            if on.get() {
                let _ = COUNT.try_with(|c| c.set(c.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const BANNED: &[&str] = &[
    "selectable_label",
    "selectable_value",
    "toggle_value",
    "ui.button(",
    "small_button",
    "egui::Button",
    "ComboBox",
    "menu_button",
    "radio_value",
    "checkbox(",
    "RichText",
    "ui.label(",
    "painter.text(",
    "FontId",
    "Color32::from_rgb",
    "Color32::from_gray",
    "Color32::WHITE",
    "Color32::BLACK",
    "on_hover_text",
];

#[test]
fn the_session_files_say_no_bare_word_and_draw_no_stock_widget() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<PathBuf> = std::fs::read_dir(src.join("views"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("session")
        })
        .collect();
    files.push(src.join("controllers_grid.rs"));
    assert!(files.len() >= 14, "{files:?}");
    let mut found = Vec::new();
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") || f.ends_with("session_tests.rs") {
                continue;
            }
            for bad in BANNED {
                if line.contains(bad) {
                    found.push(format!("{}:{}: {bad}", f.display(), n + 1));
                }
            }
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

struct Rig {
    ctx: egui::Context,
    app: CantusApp,
}

fn rig(tracks: usize, scenes: usize) -> Rig {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme_family::cantus_night(), &ctx);
    let mut app = CantusApp::from_parts(
        Some(session::demo::project(tracks, scenes)),
        EngineHandle::dummy(),
        false,
    );
    session::demo::showcase(&mut app);
    Rig { ctx, app }
}

/// One frame of the pane alone: (allocations, draw calls, triangles, repaint asked).
fn frame(r: &mut Rig) -> (u64, usize, usize, bool) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 900.0),
        )),
        ..Default::default()
    };
    COUNT.with(|c| c.set(0));
    ON.with(|on| on.set(true));
    let app = &mut r.app;
    let mut out = r.ctx.run_ui(input, |ui| {
        // The whole of the session's frame: what the engine said, the blink's repaint, the
        // grids — then the pane.
        session::frame(app, ui.ctx());
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| session::show(app, ui));
    });
    ON.with(|on| on.set(false));
    let allocs = COUNT.with(Cell::get);
    out.textures_delta.clear();
    let repaint = out
        .viewport_output
        .values()
        .any(|v| v.repaint_delay < Duration::from_secs(1));
    let clipped = r.ctx.tessellate(out.shapes, out.pixels_per_point);
    let tris = clipped
        .iter()
        .map(|c| match &c.primitive {
            egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
            _ => 0,
        })
        .sum();
    (allocs, clipped.len(), tris, repaint)
}

#[test]
fn a_full_session_stays_in_budget() {
    let mut r = rig(16, 32);
    for _ in 0..4 {
        frame(&mut r);
    }
    let (allocs, calls, tris, _) = frame(&mut r);
    println!("session 16x32: {allocs} allocations, {calls} draw calls, {tris} triangles");
    // The cells are one brush each band: a handful of meshes, never one per cell.
    assert!(calls <= 24, "{calls} draw calls");
    assert!(tris <= 60_000, "{tris} triangles");
    assert!(allocs <= 150, "{allocs} allocations");
}

#[test]
fn an_idle_grid_asks_for_no_repaint() {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::paper(), &ctx);
    let app = CantusApp::from_parts(
        Some(session::demo::project(6, 6)),
        EngineHandle::dummy(),
        false,
    );
    let mut r = Rig { ctx, app };
    for _ in 0..4 {
        frame(&mut r);
    }
    let (_, _, _, repaint) = frame(&mut r);
    assert!(!repaint, "nothing plays or blinks: no repaint");
}

/// The pane in the light theme, scenes none: drawn headless. Here, not among the lib
/// tests — a palette installed is the process's, and they read it in parallel.
#[test]
fn the_pane_draws_headless_in_the_light_theme_with_no_scene() {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(&cantus_ui::theme::paper(), &ctx);
    let mut app = CantusApp::from_parts(
        Some(session::demo::project(3, 0)),
        EngineHandle::dummy(),
        false,
    );
    session::demo::showcase(&mut app);
    for _ in 0..3 {
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 700.0),
                )),
                ..Default::default()
            },
            |ui| session::show(&mut app, ui),
        );
        out.textures_delta.clear();
    }
    assert!(app.session_view.geo.is_some());
}

#[test]
#[ignore]
fn session_timing() {
    let mut r = rig(16, 32);
    for _ in 0..10 {
        frame(&mut r);
    }
    let mut times = Vec::new();
    for _ in 0..200 {
        let t = Instant::now();
        frame(&mut r);
        times.push(t.elapsed());
    }
    times.sort();
    let med = times[times.len() / 2];
    let p95 = times[times.len() * 95 / 100];
    println!("session 16x32: median {med:?}, p95 {p95:?}");
    assert!(p95 < Duration::from_millis(16), "{p95:?}");
}
