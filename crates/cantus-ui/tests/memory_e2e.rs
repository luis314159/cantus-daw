//! What Cantus holds in memory, end to end (round 38, the scorecard's *private memory*
//! row): the bytes live on the heap after each step of the boot path — the processor, the
//! app `CantusApp::assemble` puts together on an empty song and on the demo it opens with,
//! its first frames — counted by this binary's own allocator.
//! Its own test binary: it counts every allocation and points `CANTUS_DATA_DIR` at a
//! temporary folder.

use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::boot::Shell;
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
    at("note-effect chain", &|| {
        Box::new(cantus_engine::notefx::Chain::new())
    });
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
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    // Another test of this binary may have parsed Spanish already (it switches to it).
    let spanish_before = cantus_ui::i18n::parsed(cantus_ui::i18n::Locale::Es);
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
    // r40:memory (ADR 0120 §2, §5) — an empty song computes no Exemplar sound and parses
    // one language: the built-ins wait for the picker (or a song with an Exemplar), the
    // Spanish table for Spanish.
    assert!(
        !cantus_dsp::exemplar::builtin::built(),
        "a start on an empty song computed Exemplar's built-in sounds"
    );
    assert_eq!(
        cantus_ui::i18n::parsed(cantus_ui::i18n::Locale::Es),
        spanish_before,
        "an English start parsed the Spanish strings"
    );
    // The picker asks for them (on a thread); computed here, they are a step of their own.
    CantusApp::warm_builtins();
    let _ = cantus_dsp::exemplar::builtin::builtins();
    steps.push(("Exemplar's built-in sounds (the picker asks)", mb()));
    app.replace_project(cantus_ui::demos::startup_project());
    frames(&mut app, &ctx, 3);
    steps.push(("the demo opened, three frames", mb()));
    let mut last = 0.0;
    for (what, m) in &steps {
        eprintln!("{m:8.1} MB  (+{:7.1})  {what}", m - last);
        last = *m;
    }
    // Bounds about a tenth above what was measured: more memory held by a start fails
    // here, not first in the scorecard. Round 39 (ADR 0107): the processor 8.0 MB, 59.7 MB
    // in all where round 38 held 145.5. Round 40 (ADR 0120): an empty song and its frames
    // 7.8 → 5.2 MB (one language parsed, its strings borrowed), Exemplar's built-ins a step
    // the picker causes, not the start; 56.9 MB in all.
    let step = |what: &str| {
        let i = steps.iter().position(|(w, _)| *w == what).expect(what);
        steps[i].1 - steps[i - 1].1
    };
    let processor = step("the processor");
    let builtins = step("Exemplar's built-in sounds (the picker asks)");
    let empty = step("assembled on an empty song") + step("three frames");
    let demo = step("the demo opened, three frames");
    let total = steps.last().map_or(0.0, |s| s.1);
    assert!(
        processor <= 9.0,
        "the processor holds {processor:.1} MB (8.0 measured; 96 before ADR 0107)"
    );
    assert!(
        builtins <= 6.6,
        "Exemplar's built-in sounds hold {builtins:.1} MB (6.0 measured)"
    );
    assert!(
        empty <= 5.8,
        "an empty song and its frames hold {empty:.1} MB (5.2 measured; 7.8 before ADR 0120)"
    );
    assert!(
        demo <= 41.5,
        "the demo holds {demo:.1} MB more (37.7 measured)"
    );
    assert!(
        total <= 62.6,
        "a start on the demo holds {total:.1} MB (56.9 measured; 145.5 before ADR 0107)"
    );
    drop(proc_);
}

/// Every string a pass painted (the shapes egui was handed), for what a window shows.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn walk(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_owned()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, &mut out);
    }
    out
}

/// r40:memory (ADR 0120 §4, §5) — the window as the boot runs it (`boot::Shell`, around an
/// app `assemble` made): egui is told of a texture side no wider than
/// `boot::TEXTURE_SIDE` whatever the driver reports, so its font atlas is no wider; a
/// session parses the language it speaks, and switching language shows the other one's
/// strings in the very next frame.
#[test]
fn the_shell_caps_the_font_atlas_and_a_language_switch_shows_at_once() {
    use cantus_ui::i18n::{parsed, t, Locale};
    let _one = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let dir = data();
    cantus_ui::i18n::set_thread_locale(Locale::En);
    let spanish_before = parsed(Locale::Es);
    let files = Files::read(&dir, Some(Locale::En));
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, _proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(
        &ctx,
        files,
        Some(cantus_ui::templates::Starter::Empty.build()),
        engine,
        None,
        None,
        None,
        false,
    );
    let mut shell = cantus_ui::boot::Shell::Running(Box::new(app));
    let pass = |shell: &mut cantus_ui::boot::Shell, side: Option<usize>| {
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::Vec2::new(1600.0, 900.0),
            )),
            // What the OpenGL driver of the scorecard's machine reports.
            max_texture_side: side,
            ..Default::default()
        };
        eframe::App::raw_input_hook(shell, &ctx, &mut input);
        let mut out = ctx.run_ui(input, |root| {
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(shell, root, &mut frame);
        });
        out.textures_delta.clear();
        texts(&out.shapes)
    };
    let english = pass(&mut shell, Some(16_384));
    for _ in 0..2 {
        pass(&mut shell, None);
    }
    let [w, _] = ctx.fonts(|f| f.font_image_size());
    assert!(
        w <= cantus_ui::boot::TEXTURE_SIDE,
        "egui's font atlas is {w} texels wide (the cap is {})",
        cantus_ui::boot::TEXTURE_SIDE
    );
    assert!(parsed(Locale::En), "an English session parsed no English");
    assert_eq!(
        parsed(Locale::Es),
        spanish_before,
        "an English session parsed Spanish"
    );
    let (file_en, file_es) = (
        t("menu.file"),
        cantus_ui::i18n::t_in(Locale::Es, "menu.file"),
    );
    assert_ne!(file_en, file_es);
    assert!(
        english.contains(&file_en),
        "the English window shows no {file_en:?}"
    );
    // The user's way: Settings' language list calls this.
    let Shell::Running(app) = &mut shell else {
        panic!("running")
    };
    app.set_locale(Locale::Es);
    let spanish = pass(&mut shell, None);
    // Back to English the same way, for the tests after this one.
    if let Shell::Running(app) = &mut shell {
        app.set_locale(Locale::En);
    }
    assert!(
        spanish.contains(&file_es),
        "the frame after switching to Spanish shows no {file_es:?}"
    );
    assert!(!spanish.contains(&file_en), "it still shows {file_en:?}");
}
