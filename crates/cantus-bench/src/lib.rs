//! # cantus-bench
//!
//! Cantus's benchmark suite (round 20). The goal the project states is performance better
//! than every DAW on the market; this crate is what makes that measurable before anyone
//! optimises. The method is `docs/perf/benchmark.md`; today's numbers are
//! `docs/perf/baseline.md`.
//!
//! - [`kernels`]: each effect at its defaults and heavy, each instrument at 1/8/32 voices.
//! - [`suites`]: the processor on the demo songs scaled to 10/32/64/100 tracks, the
//!   profiler's own cost, offline render speed in multiples of real time, project open.
//! - [`ui`]: frame times of the whole window on big sessions.
//! - [`startup`]: the real executable, from spawn to its second frame.
//! - [`dawbench`]: DAWbench-style DSP and VI capacity, defined so the same sessions can
//!   be rebuilt in other DAWs.
//! - [`compare`]: the perf gate against a stored baseline (`cargo run -p xtask -- perf`).
//!
//! A small harness of our own rather than criterion: the gate needs medians *and* spread
//! per metric in one JSON file per machine class, the same sessions timed block by block
//! as the audio callback sees them, and a quick mode under two minutes. Criterion's
//! per-function statistics and HTML reports answer a different question.

pub mod compare;
pub mod cpu;
pub mod dawbench;
pub mod kernels;
pub mod paced;
pub mod report;
pub mod rig;
pub mod sessions;
pub mod startup;
pub mod stats;
pub mod suites;
pub mod ui;

/// How hard a run measures.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Plan {
    /// The gate's suite: fewer runs, shorter, nothing that opens a window or a device.
    pub quick: bool,
    /// Runs per metric; each metric is the median of its runs.
    pub runs: usize,
    /// Seconds of audio per kernel run.
    pub kernel_seconds: f64,
    /// Seconds of audio per session run.
    pub session_seconds: f64,
}

impl Plan {
    pub const QUICK: Plan = Plan {
        quick: true,
        runs: 3,
        kernel_seconds: 0.5,
        session_seconds: 1.0,
    };
    pub const FULL: Plan = Plan {
        quick: false,
        runs: 5,
        kernel_seconds: 2.0,
        session_seconds: 4.0,
    };
}

/// The groups of the suite, by the name the command line takes.
pub const GROUPS: [&str; 8] = [
    "fx", "inst", "engine", "render", "open", "ui", "dawbench", "startup",
];

/// Run the groups in `only` (all of them when empty; the quick plan never runs
/// `dawbench` or `startup`) into a report.
pub fn run(plan: &Plan, only: &[String]) -> report::Report {
    let mut r = report::Report::new(if plan.quick { "quick" } else { "full" });
    let wants = |g: &str| {
        if only.is_empty() {
            !(plan.quick && (g == "dawbench" || g == "startup"))
        } else {
            only.iter().any(|o| o == g)
        }
    };
    let mut step = |g: &str, f: &mut dyn FnMut(&mut report::Report) -> Vec<report::Metric>| {
        if wants(g) {
            let t = std::time::Instant::now();
            eprintln!("bench: {g} …");
            let m = f(&mut r);
            eprintln!(
                "bench: {g}: {} metrics in {:.1} s",
                m.len(),
                t.elapsed().as_secs_f64()
            );
            r.metrics.extend(m);
        }
    };
    step("fx", &mut |_| kernels::effects(plan));
    step("inst", &mut |_| kernels::instrument_metrics(plan));
    step("engine", &mut |r| {
        let mut v = suites::engine(plan, &mut r.hot_spots);
        v.extend(suites::profiler_overhead(plan));
        v
    });
    step("render", &mut |_| suites::render_speed(plan));
    step("open", &mut |_| suites::open(plan));
    step("ui", &mut |_| ui::frames(plan));
    step("dawbench", &mut |_| dawbench::suite(plan));
    step("startup", &mut |_| startup::suite(plan));
    r
}
