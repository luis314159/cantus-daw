//! What Cantus holds in memory, end to end (round 38, the scorecard's *private memory*
//! row): the bytes live on the heap after each step of the boot path — the processor, the
//! app `CantusApp::assemble` puts together on an empty song and on the demo it opens with,
//! its first frames — counted by this binary's own allocator.
//! Its own test binary: it counts every allocation and points `CANTUS_DATA_DIR` at a
//! temporary folder.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering::Relaxed};

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

/// One test at a time: the count is the whole binary's, so a test running beside another
/// would count the other's allocations too.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// While on: every allocation's size counted, by size (a histogram of what was allocated).
static SIZES_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const SIZE_SLOTS: usize = 4096;
static SIZES: [(
    std::sync::atomic::AtomicUsize,
    std::sync::atomic::AtomicUsize,
); SIZE_SLOTS] = [const {
    (
        std::sync::atomic::AtomicUsize::new(0),
        std::sync::atomic::AtomicUsize::new(0),
    )
}; SIZE_SLOTS];

fn count_size(size: usize) {
    if !SIZES_ON.load(Relaxed) {
        return;
    }
    // Open addressing on the size: (size, count).
    let mut i = (size.wrapping_mul(0x9E37_79B9)) % SIZE_SLOTS;
    for _ in 0..SIZE_SLOTS {
        let (k, n) = &SIZES[i];
        match k.compare_exchange(0, size, Relaxed, Relaxed) {
            Ok(_) => {
                n.fetch_add(1, Relaxed);
                return;
            }
            Err(v) if v == size => {
                n.fetch_add(1, Relaxed);
                return;
            }
            Err(_) => i = (i + 1) % SIZE_SLOTS,
        }
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        count_size(l.size());
        LIVE.fetch_add(l.size() as isize, Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        count_size(l.size());
        LIVE.fetch_add(l.size() as isize, Relaxed);
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        LIVE.fetch_sub(l.size() as isize, Relaxed);
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        LIVE.fetch_add(new as isize - l.size() as isize, Relaxed);
        unsafe { System.realloc(p, l, new) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn mb() -> f64 {
    LIVE.load(Relaxed) as f64 / 1_048_576.0
}

fn data() -> std::path::PathBuf {
    let d = tempfile::tempdir().unwrap().keep();
    std::env::set_var("CANTUS_DATA_DIR", &d);
    std::fs::write(
        d.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    d
}

fn frames(app: &mut CantusApp, ctx: &egui::Context, n: usize) {
    for i in 0..n {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1920.0, 1080.0),
            )),
            time: Some(i as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| app.frame(ui)).textures_delta.clear();
    }
}

/// What one of each per-slot part holds, live (printed with `--nocapture`).
#[test]
fn what_the_parts_hold() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let at = |what: &str, f: &dyn Fn() -> Box<dyn std::any::Any>| {
        let before = LIVE.load(Relaxed);
        let kept = f();
        let held = LIVE.load(Relaxed) - before;
        eprintln!("{:9.1} KB  {what}", held as f64 / 1024.0);
        drop(kept);
    };
    at("Firmus", &|| Box::new(cantus_dsp::Firmus::new(48_000.0)));
    at("Pulsus", &|| Box::new(cantus_dsp::Pulsus::new(48_000.0)));
    SIZES_ON.store(true, Relaxed);
    at("processor (manual)", &|| {
        Box::new(EngineHandle::manual(48_000.0))
    });
    SIZES_ON.store(false, Relaxed);
    let mut by: Vec<(usize, usize)> = SIZES
        .iter()
        .map(|(k, n)| (k.load(Relaxed), n.load(Relaxed)))
        .filter(|(k, _)| *k > 0)
        .collect();
    by.sort_by_key(|(k, n)| std::cmp::Reverse(k * n));
    for (k, n) in by.iter().take(25) {
        eprintln!("  {:9.1} KB  {n:6} x {k} B", (k * n) as f64 / 1024.0);
    }
}

/// The steps of a start, and what each leaves live (printed with `--nocapture`).
#[test]
fn what_a_start_holds() {
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = data();
    let mut steps: Vec<(&str, f64)> = vec![("at the test's start", mb())];
    let files = Files::read(&dir, Some(cantus_ui::i18n::Locale::En));
    steps.push(("the files read", mb()));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    steps.push(("fonts and style", mb()));
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    steps.push(("the processor", mb()));
    let mut app = CantusApp::assemble(
        &ctx,
        files,
        Some(cantus_ui::templates::Starter::Empty.build()),
        engine,
        None,
        None,
        None,
        false,
    );
    steps.push(("assembled on an empty song", mb()));
    frames(&mut app, &ctx, 3);
    steps.push(("three frames", mb()));
    app.replace_project(cantus_ui::demos::startup_project());
    frames(&mut app, &ctx, 3);
    steps.push(("the demo opened, three frames", mb()));
    let mut last = 0.0;
    for (what, m) in &steps {
        eprintln!("{m:8.1} MB  (+{:7.1})  {what}", m - last);
        last = *m;
    }
    // Bounds from round 38's measure (the processor 96 MB, an empty song and its first
    // frames +11, the demo +39), about a tenth above it: more memory held by a start fails
    // here, not first in the scorecard. Lower them as ADR 0102's engine-memory work lands.
    let step = |what: &str| {
        let i = steps.iter().position(|(w, _)| *w == what).expect(what);
        steps[i].1 - steps[i - 1].1
    };
    let processor = step("the processor");
    let empty = step("assembled on an empty song") + step("three frames");
    let demo = step("the demo opened, three frames");
    assert!(
        processor <= 106.0,
        "the processor holds {processor:.1} MB (96 measured)"
    );
    assert!(
        empty <= 16.0,
        "an empty song and its frames hold {empty:.1} MB (11 measured)"
    );
    assert!(
        demo <= 45.0,
        "the demo holds {demo:.1} MB more (39 measured)"
    );
    drop(proc_);
}
