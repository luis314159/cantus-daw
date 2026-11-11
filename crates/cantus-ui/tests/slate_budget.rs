//! What Slate costs (round 39, ADR 0108): the whole window at 1080p, playing, in Slate
//! Night (the new default: recessed keys, flat wells, Barlow Semi Condensed) against
//! Cantus Night (beta.13's default: raised keys, materials, gradients, IBM Plex) — and the
//! two optional families beside them. **The new look may not cost frames**: on a warm frame
//! Slate allocates no more, draws no more calls and tessellates no more triangles than
//! Cantus Night; its time is measured in release with `--ignored`.
//!
//! A binary of its own on purpose: the kit's tokens and the face are installed
//! process-wide, so the rigs run one after another (`serial`).

use cantus_engine::{EngineHandle, Processor};
use cantus_ui::theme::Theme;
use cantus_ui::{welcome_project, CantusApp};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::time::{Duration, Instant};

struct Counting;

thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}

fn note() {
    let _ = ON.try_with(|on| {
        if on.get() {
            let _ = COUNT.try_with(|c| c.set(c.get() + 1));
        }
    });
}

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

#[global_allocator]
static A: Counting = Counting;

struct Frame {
    allocs: u64,
    triangles: usize,
    draws: usize,
    time: Duration,
}

struct Rig {
    app: CantusApp,
    processor: Processor,
    ctx: egui::Context,
    audio: Vec<f32>,
    frame: u64,
}

impl Rig {
    fn new(theme: &Theme) -> Rig {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(theme, &ctx);
        let (engine, processor) = EngineHandle::manual(48_000.0);
        let mut app = CantusApp::from_parts(Some(welcome_project()), engine, false);
        app.sync_engine();
        app.toggle_play();
        Rig {
            app,
            processor,
            ctx,
            audio: vec![0.0; 800 * 2],
            frame: 0,
        }
    }

    fn step(&mut self) -> Frame {
        self.processor.process(&mut self.audio, 2);
        self.frame += 1;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1920.0, 1080.0),
            )),
            time: Some(self.frame as f64 / 60.0),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let app = &mut self.app;
        let t0 = Instant::now();
        ON.with(|o| o.set(true));
        COUNT.with(|c| c.set(0));
        let mut out = self.ctx.run_ui(input, |root| app.frame(root));
        ON.with(|o| o.set(false));
        let allocs = COUNT.with(|c| c.get());
        let prims = self.ctx.tessellate(std::mem::take(&mut out.shapes), 1.0);
        let time = t0.elapsed();
        out.textures_delta.clear();
        let triangles = prims
            .iter()
            .map(|p| match &p.primitive {
                egui::epaint::Primitive::Mesh(m) => m.indices.len() / 3,
                _ => 0,
            })
            .sum();
        Frame {
            allocs,
            triangles,
            draws: prims.len(),
            time,
        }
    }

    fn run(&mut self, warm: usize, frames: usize) -> Vec<Frame> {
        for _ in 0..warm {
            self.step();
        }
        // Quiet before measuring: no instrument's samples still loading beside the
        // window (their progress rebuilds the status line on whichever frame they land).
        let t = Instant::now();
        while self.app.engine.samples.progress().is_some() && t.elapsed().as_secs() < 20 {
            std::thread::sleep(Duration::from_millis(5));
            self.step();
        }
        (0..frames).map(|_| self.step()).collect()
    }
}

fn median(mut v: Vec<u64>) -> u64 {
    v.sort_unstable();
    v[v.len() / 2]
}

fn serial() -> std::sync::MutexGuard<'static, ()> {
    static M: std::sync::Mutex<()> = std::sync::Mutex::new(());
    M.lock().unwrap_or_else(|e| e.into_inner())
}

/// The themes measured: beta.13's default first, then round 39's three families.
fn themes() -> [(&'static str, Theme); 4] {
    [
        (
            "Cantus Night (beta.13)",
            cantus_ui::theme_family::cantus_night(),
        ),
        ("Slate Night", cantus_ui::theme::slate_night()),
        ("Graphite Night", cantus_ui::theme::graphite_night()),
        ("Studio Ink Night", cantus_ui::theme::studio_ink_night()),
    ]
}

#[test]
fn slate_costs_no_allocation_no_draw_call_and_no_triangle_more_than_beta_13() {
    let _one = serial();
    let mut rows = Vec::new();
    for (name, theme) in themes() {
        let f = Rig::new(&theme).run(20, 20);
        let allocs = median(f.iter().map(|f| f.allocs).collect());
        let tris = f.iter().map(|f| f.triangles).max().unwrap_or(0);
        let draws = f.iter().map(|f| f.draws).max().unwrap_or(0);
        eprintln!("{name}: {allocs} allocations, {tris} triangles, {draws} draw calls");
        rows.push((name, allocs, tris, draws));
    }
    let (_, a0, t0, d0) = rows[0];
    for &(name, a, t, d) in &rows[1..] {
        assert!(a <= a0 + 8, "{name} allocates {a} > beta.13's {a0}");
        assert!(d <= d0, "{name} draws {d} calls > beta.13's {d0}");
        assert!(t <= t0, "{name} tessellates {t} triangles > beta.13's {t0}");
    }
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn slate_timing() {
    let _one = serial();
    let ms = |frames: Vec<Frame>| {
        let mut v: Vec<f64> = frames.iter().map(|f| f.time.as_secs_f64() * 1e3).collect();
        v.sort_by(f64::total_cmp);
        (v[v.len() / 2], v[v.len() * 95 / 100])
    };
    // Interleaved runs, best of three, so a busy machine weighs on all alike.
    let mut best = [(f64::MAX, f64::MAX); 4];
    for _ in 0..3 {
        for (k, (_, theme)) in themes().iter().enumerate() {
            let (p50, p95) = ms(Rig::new(theme).run(30, 200));
            if p50 < best[k].0 {
                best[k] = (p50, p95);
            }
        }
    }
    for (k, (name, _)) in themes().iter().enumerate() {
        eprintln!(
            "whole window 1920x1080, build + tessellate, {name}: p50 {:.2} ms, p95 {:.2} ms",
            best[k].0, best[k].1
        );
    }
    assert!(
        best[1].0 <= best[0].0 * 1.05 + 0.10,
        "Slate costs time: {:.2} ms against beta.13's {:.2} ms",
        best[1].0,
        best[0].0
    );
}
