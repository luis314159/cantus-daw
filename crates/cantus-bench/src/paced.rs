//! `cantus-bench paced`: the real processor at the device period (ADR 0031 R13 (4), round
//! 22's target) — each block started at its period's start as a device callback would be,
//! the thread promoted as the WASAPI callback is (MMCSS "Pro Audio"), the pool's workers
//! promoted and placed as the application's are. A **miss** is a block that ended after
//! its period did: a dropout on a device. Back-to-back blocks hide what a live callback
//! sees — caches cooling, cores clocking down, workers parking between blocks — so the
//! live targets are measured here.

use crate::rig::{Rig, SAMPLE_RATE};
use crate::stats::{median, quantile};
use cantus_core::{LoopRegion, Project, Ticks};
use cantus_engine::Workers;
use std::time::{Duration, Instant};

/// What one paced run measured.
#[derive(Clone, Debug)]
pub struct Paced {
    pub song: String,
    pub frames: usize,
    pub blocks: usize,
    /// Blocks that ended after their period ended (a device would have dropped out).
    pub misses: usize,
    /// Blocks whose own time was over the period (the engine's `overruns`).
    pub overruns: u64,
    /// How late each block started after its period began, and how long it took, µs.
    pub late_p99_us: f64,
    pub block_p50_us: f64,
    pub block_p99_us: f64,
    pub block_max_us: f64,
    /// The pool's workers that joined, the wakes and the blocks it ran in parallel.
    pub workers: u32,
    pub wakes: u64,
    pub parallel_share: f64,
    pub conflicts: u64,
    /// The last overrun's culprit, in the project's words (the profiler's blame).
    pub culprit: String,
}

/// The songs a paced run plays: the startup project and every demo.
pub fn songs() -> Vec<(String, Project)> {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut v = vec![(
        "startup".to_string(),
        cantus_ui::demos::startup_project(),
    )];
    for d in cantus_ui::demos::ALL.iter() {
        v.push((d.id.to_string(), (d.build)()));
    }
    v
}

/// Play `project` for `seconds` at `frames` per block, paced at the device period, with
/// `workers`. The song loops over its whole content when it has no loop of its own.
pub fn run(
    name: &str,
    project: &Project,
    frames: usize,
    seconds: f64,
    workers: Workers,
    promote_callback: bool,
) -> Paced {
    let mut p = project.clone();
    if p.loop_region.is_none() || !p.loop_enabled {
        let end = p.content_end().max(Ticks::beats(4));
        p.loop_region = Some(LoopRegion {
            start: Ticks::ZERO,
            end,
        });
        p.loop_enabled = true;
    }
    let start = p.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.start);
    let mut rig = Rig::with_workers(&p, start, frames, workers);
    let period = Duration::from_secs_f64(frames as f64 / SAMPLE_RATE);
    // The process out of EcoQoS, as the application starts (R1).
    let _ = cantus_rt::thread::opt_out_of_power_throttling();
    // The callback's own promotion (R1), as on WASAPI.
    if promote_callback {
        let _ = cantus_rt::promote_callback(cantus_rt::AudioHost::Wasapi, period);
    }
    // A second of warm-up, paced too: the workers start on the first block's period.
    let warm = (SAMPLE_RATE / frames as f64) as usize;
    let blocks = (seconds * SAMPLE_RATE / frames as f64).ceil() as usize;
    let mut late = Vec::with_capacity(blocks);
    let mut took = Vec::with_capacity(blocks);
    let mut misses = 0usize;
    let mut parallel = 0usize;
    let mut due = Instant::now();
    let mut stats0 = None;
    for k in 0..warm + blocks {
        if k == warm {
            due = Instant::now();
            stats0 = Some(*rig.processor.stats());
        }
        let now = Instant::now();
        if due > now {
            std::thread::sleep(due - now);
        }
        let s = Instant::now();
        rig.block();
        let e = Instant::now();
        if k >= warm {
            late.push(s.saturating_duration_since(due).as_secs_f64() * 1e6);
            took.push((e - s).as_secs_f64() * 1e6);
            if e > due + period {
                misses += 1;
            }
            if rig.processor.stats().parallel {
                parallel += 1;
            }
        }
        // The next period — or, after a miss, the first period that has not begun yet:
        // a device drops what it missed rather than queueing it.
        due += period;
        let now = Instant::now();
        while due + period <= now {
            due += period;
        }
    }
    let s1 = *rig.processor.stats();
    let s0 = stats0.unwrap_or(s1);
    Paced {
        song: name.to_string(),
        frames,
        blocks,
        misses,
        overruns: s1.overruns - s0.overruns,
        late_p99_us: quantile(&late, 0.99),
        block_p50_us: median(&took),
        block_p99_us: quantile(&took, 0.99),
        block_max_us: took.iter().copied().fold(0.0, f64::max),
        workers: s1.workers,
        wakes: s1.wakes - s0.wakes,
        parallel_share: parallel as f64 / blocks.max(1) as f64,
        conflicts: s1.schedule_conflicts,
        culprit: s1
            .last_dropout
            .map(|d| {
                format!(
                    "{} ({:.1} of {:.1} ms)",
                    cantus_ui::perf_status::culprit_text(project, d.culprit),
                    d.culprit_us / 1e3,
                    d.block_us / 1e3
                )
            })
            .unwrap_or_default(),
    }
}

/// A table row.
pub fn row(r: &Paced) -> String {
    format!(
        "| {} | {} | {} | **{}** | {} | {:.0} | {:.0} | {:.0} | {:.0} | {} | {} | {:.0} % | {} | {} |",
        r.song,
        r.frames,
        r.blocks,
        r.misses,
        r.overruns,
        r.late_p99_us,
        r.block_p50_us,
        r.block_p99_us,
        r.block_max_us,
        r.workers,
        r.wakes,
        r.parallel_share * 100.0,
        r.conflicts,
        r.culprit
    )
}

pub const HEADER: &str = "| Song | Frames | Blocks | Misses | Overruns | Late p99 µs | Block p50 µs | Block p99 µs | Block max µs | Workers | Wakes | Parallel | Conflicts | Last overrun |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|";
