//! DAWbench-style capacity tests (`docs/perf/benchmark.md` §3–§4): how many instances
//! of a reference compressor (DSP) and how many voices of a reference synth (VI) one core
//! sustains at a given buffer size.
//!
//! The track count is searched by bisection: a session *passes* when its median block
//! stays under half the block's budget. These sessions do the same work every block (a
//! held note, a repeating chord), so on a quiet machine their 99th percentile sits close
//! to the median, and the factor of two is the margin a live callback needs for the
//! scheduler's wake-up, the driver and the cold caches a back-to-back run does not see.
//! The median, not the 99th percentile, because on a shared machine the tail of a
//! back-to-back run is the neighbours' preemptions, not the session. The live figure,
//! taken with a device, is the one to compare with other DAWs (§5); this is its offline
//! estimate.

use crate::report::{Better, Metric};
use crate::rig::{Rig, SAMPLE_RATE};
use crate::stats::{median, Summary};
use crate::{sessions, Plan};
use cantus_core::{InstrumentKind, Project, Ticks};

/// A session of `n` tracks.
type Build<'a> = &'a dyn Fn(usize) -> Project;

/// Compressors per track in the DSP session.
pub const DSP_PER_TRACK: usize = 8;
/// Voices per track in the VI session (its chords).
pub const VI_VOICES: usize = 8;
/// A session passes while its median block stays under this share of the budget.
pub const PASS_LOAD: f64 = 0.5;

/// Whether `p` passes at `frames`, and its median load.
fn passes(p: &Project, frames: usize, seconds: f64) -> (bool, f64) {
    let blocks = (seconds * SAMPLE_RATE / frames as f64).ceil() as usize;
    let mut rig = Rig::new(p, Ticks::ZERO, frames);
    let run = rig.run((SAMPLE_RATE as usize / frames) / 2, blocks);
    let load = median(&run.block_us) / run.budget_us();
    (load <= PASS_LOAD, load)
}

/// The most tracks of `build(n)` that pass at `frames` (0 to `ENGINE_TRACKS`), and
/// whether the engine's ceiling was what stopped the search.
pub fn capacity(build: &dyn Fn(usize) -> Project, frames: usize, seconds: f64) -> (usize, bool) {
    let max = sessions::ENGINE_TRACKS;
    if passes(&build(max), frames, seconds).0 {
        return (max, true);
    }
    let (mut lo, mut hi) = (0usize, max);
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if passes(&build(mid), frames, seconds).0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo, false)
}

/// The DSP and VI capacities at 64 and 128 frames, each searched `plan.runs` times.
pub fn suite(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    let seconds = if plan.quick { 2.0 } else { 4.0 };
    let runs = plan.runs.min(3);
    let dsp = |n: usize| sessions::dsp(n, DSP_PER_TRACK);
    let vi = |n: usize| sessions::vi(n, InstrumentKind::Analog);
    let tests: [(&str, Build, usize, &str); 2] = [
        ("dsp", &dsp, DSP_PER_TRACK, "Comprimo instances"),
        ("vi", &vi, VI_VOICES, "Calor voices"),
    ];
    for (name, build, per, unit) in tests {
        for frames in [64usize, 128] {
            let mut capped = false;
            let v: Vec<f64> = (0..runs)
                .map(|_| {
                    let (n, c) = capacity(build, frames, seconds);
                    capped |= c;
                    (n * per) as f64
                })
                .collect();
            let mut m = Metric::new(
                format!("dawbench.{name}.b{frames}"),
                unit,
                Better::Higher,
                Summary::of(&v),
            )
            .ungated();
            if capped {
                m = m.note(format!(
                    "at least this many: the engine's {} tracks all passed",
                    sessions::ENGINE_TRACKS
                ));
            }
            out.push(m);
        }
    }
    out
}
