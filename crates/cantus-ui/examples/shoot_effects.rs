//! The effect editors in their device frame, without a screen (ADR 0024 §8, §14).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_effects -- [out-dir] [kind…] [--quick] [--bench]
//! ```
//!
//! - **Shots**: every editor alone (`eq comp limiter space multiband deesser gate plugin`),
//!   in Slate and Paper, in English and Spanish, at 100 % and 150 % (pixels per point) —
//!   and, for the no-overlap check, at the window scales 75 % and 200 %. `--quick`: Slate
//!   and Paper in English at 100 % only. PNGs go to `target/shots/effects` by default.
//! - **Bench** (`--bench`, run it with `--release`): per editor, 200 frames at 100 %
//!   after 30 of warm-up; the median CPU time to build a frame and to tessellate it, and the
//!   allocations a frame makes in steady state, counted by the global allocator below.
//!
//! The engine is the dummy one, so each editor is fed what a playing engine would, by its
//! own `demo_feed`.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct Counting;

static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
/// `CANTUS_ALLOC_TRACE`: print a backtrace for each allocation of one frame.
static TRACE: AtomicBool = AtomicBool::new(false);

thread_local! {
    static IN_TRACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn trace() {
    if TRACE.load(Ordering::Relaxed) && !IN_TRACE.with(|t| t.replace(true)) {
        let bt = std::backtrace::Backtrace::force_capture().to_string();
        let frames: Vec<&str> = bt
            .lines()
            .filter(|l| l.contains("cantus_ui::") && !l.contains("shoot_effects"))
            .take(3)
            .collect();
        eprintln!("ALLOC {}", frames.join(" <- ").replace("  ", ""));
        IN_TRACE.with(|t| t.set(false));
    }
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            trace();
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
            trace();
        }
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The test CLAP plug-in's library, built from source the first time (as `shoot` does).
fn test_plugin() -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let target = root.join("target").join("test-plugins");
    let file = target.join("debug").join(if cfg!(windows) {
        "cantus_test_clap.dll"
    } else if cfg!(target_os = "macos") {
        "libcantus_test_clap.dylib"
    } else {
        "libcantus_test_clap.so"
    });
    if !file.exists() {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let _ = std::process::Command::new(cargo)
            .args(["build", "-p", "cantus-test-clap", "--target-dir"])
            .arg(&target)
            .current_dir(&root)
            .status();
    }
    file
}

/// An app with `kind`'s editor open and fed, in the language set on this thread.
fn app_with(kind: &str) -> CantusApp {
    // Aequalis's other states: `eq-plain` (a band with no dynamics chosen), `eq-match`.
    if let Some(state) = kind.strip_prefix("eq-") {
        let mut app = app_with("eq");
        match state {
            "plain" => app.eq_editor.select(&[2]),
            "match" => app.eq_editor.matching = Some(Default::default()),
            _ => {}
        }
        return app;
    }
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    if kind == "plugin" {
        views::plugins::demo_setup(&mut app, &test_plugin(), true);
    } else {
        app.demo_editor(kind);
        let k = match kind {
            "eq" => Some(cantus_core::EffectKind::Eq),
            "comp" => Some(cantus_core::EffectKind::Compressor),
            _ => app.fx.open.map(|o| o.2),
        };
        if let Some(k) = k {
            views::fx::demo_feed(&mut app, k);
        }
    }
    // Every shot of a kind starts at the window's corner.
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

fn draw(app: &mut CantusApp, ctx: &egui::Context) {
    views::eq_editor::window(app, ctx);
    views::comp_editor::window(app, ctx);
    views::fx::window(app, ctx);
}

fn kind_of(name: &str) -> cantus_core::EffectKind {
    use cantus_core::EffectKind as K;
    match name {
        "eq" | "eq-plain" | "eq-match" => K::Eq,
        "comp" => K::Compressor,
        "limiter" => K::Limiter,
        "space" => K::Space,
        "multiband" => K::Multiband,
        "deesser" => K::DeEsser,
        "gate" => K::Gate,
        _ => K::External,
    }
}

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::slate()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(|a, b| a.total_cmp(b));
    v[v.len() / 2]
}

fn bench(kinds: &[String]) {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    println!(
        "{:<10} {:>9} {:>9} {:>9} {:>9} {:>12} {:>9}",
        "editor", "build ms", "tess ms", "total ms", "p10 ms", "allocs/frame", "vertices"
    );
    for kind in kinds {
        let ctx = context(true);
        let mut app = app_with(kind);
        let size = views::fx::frame::designed_size(kind_of(kind)) + egui::vec2(40.0, 40.0);
        let input = |i: usize| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let (mut build, mut tess) = (Vec::new(), Vec::new());
        let mut allocs = 0usize;
        let mut verts = 0usize;
        let (warm, frames) = (30, 200);
        for i in 0..warm + frames {
            let counting = i >= warm;
            // Count what the editor allocates, not egui's own pass around it.
            ALLOCS.store(0, Ordering::Relaxed);
            let t0 = std::time::Instant::now();
            let mut out = ctx.run_ui(input(i), |ui| {
                COUNTING.store(counting, Ordering::Relaxed);
                draw(&mut app, ui.ctx());
                COUNTING.store(false, Ordering::Relaxed);
            });
            let t1 = std::time::Instant::now();
            allocs += ALLOCS.load(Ordering::Relaxed);
            if counting && i == warm && std::env::var("CANTUS_ALLOC_TRACE").is_ok() {
                TRACE.store(true, Ordering::Relaxed);
                COUNTING.store(true, Ordering::Relaxed);
                ctx.run_ui(input(i), |ui| draw(&mut app, ui.ctx()))
                    .textures_delta
                    .clear();
                COUNTING.store(false, Ordering::Relaxed);
                TRACE.store(false, Ordering::Relaxed);
            }
            out.textures_delta.clear();
            let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
            let t2 = std::time::Instant::now();
            verts = prims
                .iter()
                .map(|p| match &p.primitive {
                    egui::epaint::Primitive::Mesh(m) => m.vertices.len(),
                    _ => 0,
                })
                .sum();
            drop(prims);
            if counting {
                build.push((t1 - t0).as_secs_f64() * 1e3);
                tess.push((t2 - t1).as_secs_f64() * 1e3);
            }
        }
        // The median, and the 10th percentile of whole frames: on a busy machine other
        // processes take the core now and then, and the low end shows what a frame costs.
        let mut totals: Vec<f64> = build.iter().zip(&tess).map(|(a, b)| a + b).collect();
        totals.sort_by(|a, b| a.total_cmp(b));
        let p10 = totals[totals.len() / 10];
        let (b, t) = (median(&mut build), median(&mut tess));
        println!(
            "{kind:<10} {b:>9.3} {t:>9.3} {:>9.3} {p10:>9.3} {:>12.1} {verts:>9}",
            b + t,
            allocs as f64 / frames as f64
        );
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let quick = args.iter().any(|a| a == "--quick");
    let benching = args.iter().any(|a| a == "--bench");
    args.retain(|a| !a.starts_with("--"));
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let dir = if args.first().is_some_and(|a| a.contains(['/', '\\'])) {
        PathBuf::from(args.remove(0))
    } else {
        root.join("target").join("shots").join("effects")
    };
    let all = [
        "eq",
        "comp",
        "limiter",
        "space",
        "multiband",
        "deesser",
        "gate",
        "plugin",
    ];
    let kinds: Vec<String> = if args.is_empty() {
        all.iter().map(|s| s.to_string()).collect()
    } else {
        args
    };
    if benching {
        bench(&kinds);
        return;
    }
    std::fs::create_dir_all(&dir).expect("output dir");
    use cantus_ui::i18n::Locale;
    // (theme, language, pixels per point, window scale)
    let mut shots: Vec<(bool, Locale, f32, f32)> = Vec::new();
    for dark in [true, false] {
        for lang in [Locale::En, Locale::Es] {
            if quick && lang == Locale::Es {
                continue;
            }
            for ppp in [1.0f32, 1.5] {
                if quick && ppp > 1.0 {
                    continue;
                }
                shots.push((dark, lang, ppp, 1.0));
            }
            if !quick {
                shots.push((dark, lang, 1.0, 0.75));
                shots.push((dark, lang, 1.0, 2.0));
            }
        }
    }
    for kind in &kinds {
        for &(dark, lang, ppp, k) in &shots {
            cantus_ui::i18n::set_locale(lang);
            let ctx = context(dark);
            let mut app = app_with(kind);
            let fx = kind_of(kind);
            views::fx::frame::shell_of(fx).set_scale(&ctx, k);
            let mut size = views::fx::frame::designed_size(fx) * k;
            if kind == "plugin" {
                // The generic editor fits its parameters: learn its height first, in a
                // context of its own (the shot's context must see every texture upload).
                let probe = context(dark);
                views::fx::frame::shell_of(fx).set_scale(&probe, k);
                for _ in 0..2 {
                    let input = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    };
                    probe
                        .run_ui(input, |ui| draw(&mut app, ui.ctx()))
                        .textures_delta
                        .clear();
                }
                if let Some(h) = views::plugins::wanted_height(&app) {
                    size.y = h * k;
                }
            }
            let name = format!(
                "{kind}-{}-{}-{}{}.png",
                if dark { "slate" } else { "paper" },
                lang.code(),
                (ppp * 100.0) as u32,
                if k == 1.0 {
                    String::new()
                } else {
                    format!("-k{}", (k * 100.0) as u32)
                }
            );
            let path = dir.join(name);
            shoot(&ctx, size, ppp, 6, Some(&path), |root| {
                let ctx = root.ctx().clone();
                draw(&mut app, &ctx);
            });
            println!("{}", path.display());
        }
    }
}
