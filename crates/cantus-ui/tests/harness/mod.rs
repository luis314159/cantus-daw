//! What the soak and the crash tests share (round 22): a "live" engine — a manual engine
//! whose processor runs on its own thread at the pace of an audio callback, fed by a fake
//! input — and the application's frame, painted headless.
#![allow(dead_code)]

use cantus_engine::{EngineHandle, Processor};
use cantus_ui::CantusApp;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const RATE: u32 = 48_000;

/// Input frame `i` of the fake input: `i + 1` scaled to stay exact in f32, so any frame
/// read back from a take says which input frame it was (as `rec_e2e` does).
pub fn ramp(i: u64) -> f32 {
    ((i % 1_048_575) + 1) as f32 / 1_048_576.0
}

pub fn which(v: f32) -> Option<u64> {
    (v != 0.0).then(|| (v * 1_048_576.0).round() as u64 - 1)
}

/// What the callback thread measured.
#[derive(Default)]
pub struct CallbackStats {
    pub blocks: AtomicU64,
    /// Blocks whose `process` took longer than the block lasts.
    pub overruns: AtomicU64,
    /// The slowest block, in microseconds.
    pub max_us: AtomicU64,
    /// Input frames pushed so far.
    pub input_frames: AtomicU64,
    /// Microseconds since the thread started at its last block (a stall shows here).
    pub last_block_at_us: AtomicU64,
}

/// A manual engine's processor running on a thread like a callback.
pub struct LiveEngine {
    pub stats: Arc<CallbackStats>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub started: Instant,
}

impl LiveEngine {
    /// The handle for the app, and the processor running at `block` frames a callback
    /// with a mono ramp coming in.
    pub fn start(block: usize) -> (EngineHandle, LiveEngine) {
        let (handle, proc) = EngineHandle::manual(f64::from(RATE));
        let live = LiveEngine::run(proc, block);
        (handle, live)
    }

    fn run(mut proc: Processor, block: usize) -> LiveEngine {
        let (mut ring, consumer) = rtrb::RingBuffer::<f32>::new(RATE as usize);
        proc.attach_input(consumer, 1);
        let stats = Arc::new(CallbackStats::default());
        let stop = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let (s, flag) = (stats.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("soak-callback".into())
            .spawn(move || {
                let period = Duration::from_secs_f64(block as f64 / f64::from(RATE));
                let mut out = vec![0f32; block * 2];
                let mut next = Instant::now();
                let mut pushed = 0u64;
                while !flag.load(Ordering::Relaxed) {
                    for _ in 0..block {
                        if ring.push(ramp(pushed)).is_err() {
                            break;
                        }
                        pushed += 1;
                    }
                    s.input_frames.store(pushed, Ordering::Relaxed);
                    let t0 = Instant::now();
                    proc.process(&mut out, 2);
                    let us = t0.elapsed().as_micros() as u64;
                    s.blocks.fetch_add(1, Ordering::Relaxed);
                    if us as f64 > period.as_secs_f64() * 1e6 {
                        s.overruns.fetch_add(1, Ordering::Relaxed);
                    }
                    s.max_us.fetch_max(us, Ordering::Relaxed);
                    s.last_block_at_us
                        .store(started.elapsed().as_micros() as u64, Ordering::Relaxed);
                    next += period;
                    let now = Instant::now();
                    if next > now {
                        std::thread::sleep(next - now);
                    } else if now - next > period * 8 {
                        // Fell far behind (the machine was busy): start the clock again.
                        next = now;
                    }
                }
            })
            .expect("spawn the callback thread");
        LiveEngine {
            stats,
            stop,
            thread: Some(thread),
            started,
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for LiveEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

/// A context set up as the real one is (fonts, type scale), in English.
pub fn ctx() -> egui::Context {
    cantus_ui::i18n::default_thread_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    ctx
}

/// One whole frame of the application at `size`, headless; returns its wall time.
pub fn frame(app: &mut CantusApp, ctx: &egui::Context, size: egui::Vec2, time: f64) -> Duration {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        time: Some(time),
        ..Default::default()
    };
    let t0 = Instant::now();
    let mut out = ctx.run_ui(input, |ui| app.frame(ui));
    let dt = t0.elapsed();
    // Drop the frame's textures as the other headless tests do (nothing uploads them).
    out.textures_delta.clear();
    dt
}

/// Point every file the application writes at `dir`, not the user's folder.
pub fn isolate(app: &mut CantusApp, dir: &std::path::Path) {
    app.settings_path = dir.join("settings.json");
    app.layouts_path = dir.join("layouts.json");
    app.keymap_path = dir.join("keymap.toml");
    app.appearance_path = dir.join("appearance.json");
    app.autosave_dir = dir.join("autosave");
}
