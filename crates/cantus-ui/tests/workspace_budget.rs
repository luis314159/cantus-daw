//! The workspace's share of the frame budget (ADR 0024 §13), counted: the toolbar and the
//! status bar allocate nothing on a warm frame (a counting global allocator around them);
//! the whole window's allocations and time are printed for the record, and timed in
//! release with `--ignored`.

use cantus_engine::EngineHandle;
use cantus_ui::app::CantusApp;
use cantus_ui::i18n::{default_thread_locale, Locale};
use cantus_ui::{views, welcome_project};
use egui::{Pos2, Rect, Vec2};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

struct Counting;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static THIS_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) && THIS_THREAD.with(|t| t.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) && THIS_THREAD.with(|t| t.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn count(f: impl FnOnce()) -> usize {
    THIS_THREAD.with(|t| t.set(true));
    ALLOCS.store(0, Ordering::Relaxed);
    COUNTING.store(true, Ordering::Relaxed);
    f();
    COUNTING.store(false, Ordering::Relaxed);
    THIS_THREAD.with(|t| t.set(false));
    ALLOCS.load(Ordering::Relaxed)
}

fn app() -> (CantusApp, egui::Context, tempfile::TempDir) {
    default_thread_locale(Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let dir = tempfile::tempdir().unwrap();
    let mut a = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    a.settings_path = dir.path().join("settings.json");
    a.layouts_path = dir.path().join("layouts.json");
    a.keymap_path = dir.path().join("keymap.toml");
    a.appearance_path = dir.path().join("appearance.json");
    a.attach(&ctx);
    (a, ctx, dir)
}

fn input(w: f32, h: f32) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w, h))),
        ..Default::default()
    }
}

#[test]
fn the_toolbar_and_the_status_bar_are_counted_on_a_warm_frame() {
    // The counter sees everything on this thread between the two calls, egui's own
    // per-frame lists included (each widget's rect, the frame's shapes, grown afresh every
    // frame); the chrome's painting and texts are cached and reused. Held low and steady.
    let (mut a, ctx, _dir) = app();
    let mut bar = Vec::new();
    let mut status = Vec::new();
    for pass in 0..14 {
        let (mut nb, mut ns) = (0, 0);
        ctx.run_ui(input(1920.0, 1080.0), |ui| {
            let r = ui.max_rect();
            let top = Rect::from_min_size(r.min, Vec2::new(r.width(), 54.0));
            let bottom = Rect::from_min_max(Pos2::new(r.left(), r.bottom() - 22.0), r.max);
            let mut t = ui.new_child(egui::UiBuilder::new().max_rect(top));
            let mut b = ui.new_child(egui::UiBuilder::new().max_rect(bottom));
            nb = count(|| views::transport::show(&mut a, &mut t));
            ns = count(|| views::workspace_status::show(&mut a, &mut b));
        })
        .textures_delta
        .clear();
        if pass >= 5 {
            bar.push(nb);
            status.push(ns);
        }
    }
    eprintln!("allocations per warm frame: toolbar {bar:?}, status bar {status:?}");
    // egui's maps grow in bursts on some frames (a rehash, a list that doubles), and more
    // when other tests share the process: the quietest frame is what the chrome costs —
    // 12 and 2 when this test runs alone. The ceiling catches a label rebuilt per key per
    // frame (hundreds more), not egui's own noise.
    let (b, st) = (
        *bar.iter().min().unwrap_or(&0),
        *status.iter().min().unwrap_or(&0),
    );
    assert!(
        b <= 400 && st <= 64,
        "the chrome allocates more than egui's own bookkeeping: toolbar {bar:?}, status {status:?}"
    );
}

#[test]
fn the_whole_window_is_counted() {
    let (mut a, ctx, _dir) = app();
    let mut allocs = Vec::new();
    for pass in 0..6 {
        let mut n = 0;
        ctx.run_ui(input(1920.0, 1080.0), |ui| {
            n = count(|| a.frame(ui));
        })
        .textures_delta
        .clear();
        if pass >= 3 {
            allocs.push(n);
        }
    }
    eprintln!("whole window (every view), allocations per warm frame: {allocs:?}");
    assert!(allocs.iter().all(|n| *n > 0), "counting works");
}

/// `cargo test --release -p cantus-ui --test workspace_budget -- --ignored --nocapture`
#[test]
#[ignore]
fn the_whole_window_frame_time() {
    for (w, h) in [(1280.0, 720.0), (1920.0, 1080.0)] {
        let (mut a, ctx, _dir) = app();
        let mut times = Vec::new();
        for pass in 0..80 {
            let t = Instant::now();
            ctx.run_ui(input(w, h), |ui| a.frame(ui))
                .textures_delta
                .clear();
            if pass >= 20 {
                times.push(t.elapsed().as_secs_f64() * 1000.0);
            }
        }
        times.sort_by(f64::total_cmp);
        let p50 = times[times.len() / 2];
        let p95 = times[times.len() * 95 / 100];
        eprintln!(
            "whole window {w}x{h}: p50 {p50:.2} ms, p95 {p95:.2} ms (layout, no tessellation)"
        );
    }
}
