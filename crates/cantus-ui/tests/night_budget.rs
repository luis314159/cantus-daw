//! What Cantus Night's gradients cost (ADR 0046 §8): the whole window at 1080p, playing,
//! in Cantus Night as shipped against the same theme with every gradient token cleared.
//! The sheen and the glows must add no allocation and no draw call on a warm frame, and
//! only a few thousand triangles; their time is measured in release with `--ignored`.
//!
//! A binary of its own on purpose: the kit's tokens are installed process-wide, and the
//! other budget tests run in Slate.

use cantus_engine::{EngineHandle, Processor};
use cantus_ui::kit::tokens::{self, Tokens};
use cantus_ui::{welcome_project, CantusApp};
use egui::Color32;
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
    fn new(t: Tokens) -> Rig {
        cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme_family::cantus_night(), &ctx);
        tokens::install(t);
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

fn flat(mut t: Tokens) -> Tokens {
    t.sheen = Color32::TRANSPARENT;
    t.glow = Color32::TRANSPARENT;
    t.glow_cool = Color32::TRANSPARENT;
    t
}

fn median(mut v: Vec<u64>) -> u64 {
    v.sort_unstable();
    v[v.len() / 2]
}

#[test]
fn cantus_night_s_gradients_cost_no_allocation_and_no_draw_call() {
    let night = cantus_ui::theme_family::cantus_night_tokens();
    let lit = Rig::new(night).run(20, 20);
    let plain = Rig::new(flat(night)).run(20, 20);
    let allocs = |f: &[Frame]| median(f.iter().map(|f| f.allocs).collect());
    let worst = |f: &[Frame], g: fn(&Frame) -> usize| f.iter().map(g).max().unwrap_or(0);
    let (a_lit, a_plain) = (allocs(&lit), allocs(&plain));
    let (t_lit, t_plain) = (worst(&lit, |f| f.triangles), worst(&plain, |f| f.triangles));
    let (d_lit, d_plain) = (worst(&lit, |f| f.draws), worst(&plain, |f| f.draws));
    eprintln!(
        "whole window, Cantus Night: {a_lit} allocations, {t_lit} triangles, {d_lit} draw \
         calls; flat: {a_plain}, {t_plain}, {d_plain}"
    );
    assert!(
        a_lit <= a_plain,
        "the gradients allocate: {a_lit} > {a_plain}"
    );
    assert_eq!(
        d_lit, d_plain,
        "the gradients paint into the views' own meshes"
    );
    assert!(
        t_lit <= t_plain + 6_000,
        "the gradients took {} triangles",
        t_lit.saturating_sub(t_plain)
    );
}

#[test]
#[ignore = "timing: run in release with --ignored"]
fn cantus_night_s_gradients_timing() {
    let night = cantus_ui::theme_family::cantus_night_tokens();
    let ms = |frames: Vec<Frame>| {
        let mut v: Vec<f64> = frames.iter().map(|f| f.time.as_secs_f64() * 1e3).collect();
        v.sort_by(f64::total_cmp);
        (v[v.len() / 2], v[v.len() * 95 / 100])
    };
    // Interleaved runs, best of three, so a busy machine weighs on both alike.
    let mut best = [(f64::MAX, f64::MAX); 2];
    for _ in 0..3 {
        for (k, t) in [night, flat(night)].into_iter().enumerate() {
            let (p50, p95) = ms(Rig::new(t).run(30, 200));
            if p50 < best[k].0 {
                best[k] = (p50, p95);
            }
        }
    }
    eprintln!(
        "whole window build + tessellate: Cantus Night p50 {:.2} ms p95 {:.2} ms; flat p50 \
         {:.2} ms p95 {:.2} ms",
        best[0].0, best[0].1, best[1].0, best[1].1
    );
    assert!(
        best[0].0 <= best[1].0 * 1.10 + 0.10,
        "the gradients cost time"
    );
}
