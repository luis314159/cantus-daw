//! The beta's performance floor (`docs/BETA.md` blocker 2): the startup song and every demo
//! at 256 frames through the application's engine — the pool the program starts
//! (`Workers::auto()`, promoted and placed), the heavy instruments splitting their voices
//! over it (ADR 0038) — **over the whole song**: each run times `windows` stretches spread
//! evenly from the first bar to the last (each run's set offset from the others'), every
//! window entered by a seek one bar early (at most 1.5 s) so the notes that sound in it have
//! started. A song is gated by its heaviest section, so `demo.<song>.b256.load` is the
//! **worst window's** median block as a share of the 5.33 ms budget — the beta's bar is
//! under 70 % — and `.load99` the 99th percentile of every timed block. (Back to back is a
//! lower bound on a live block; the paced runner, `cantus-bench paced`, counts the misses a
//! device would hear.) A note held from before its window's pre-roll is not heard there: the
//! engine does not chase notes on a seek.

use crate::report::{Better, Metric};
use crate::rig::{Rig, SAMPLE_RATE};
use crate::stats::{median, quantile, Summary};
use crate::Plan;
use cantus_core::{LoopRegion, Ticks};
use cantus_engine::EngineMessage;

/// The block the beta's floor is set at.
pub const FRAMES: usize = 256;

/// How a run covers a song: `windows` stretches of `seconds` each.
struct Sweep {
    windows: usize,
    seconds: f64,
    runs: usize,
}

/// One run over a song: every timed block, and the worst window (its median, its bar).
struct Pass {
    blocks: Vec<f64>,
    worst: f64,
    worst_bar: i64,
}

fn sweep(rig: &mut Rig, lo: i64, hi: i64, bpm: f64, s: &Sweep, run: usize) -> Pass {
    let bar = Ticks::beats(4).0;
    let bars = ((hi - lo) / bar).max(1);
    let timed = (s.seconds * SAMPLE_RATE / FRAMES as f64).ceil() as usize;
    let mut pass = Pass {
        blocks: Vec::new(),
        worst: 0.0,
        worst_bar: 0,
    };
    for i in 0..s.windows {
        // Run k's windows sit (k + ½)/runs of a window's spacing in, so the runs together
        // cover the song more densely than one does.
        let phase = (run as f64 + 0.5) / s.runs.max(1) as f64;
        let b = (((i as f64 + phase) / s.windows as f64) * bars as f64) as i64;
        let at = lo + b.min(bars - 1) * bar;
        let pre_ticks = Ticks::from_seconds(1.5, bpm).0.min(bar);
        let pre = (at - pre_ticks).max(lo);
        rig.send(EngineMessage::Seek(Ticks(pre)));
        let pre_blocks = Ticks(at - pre).to_seconds(bpm) * SAMPLE_RATE / FRAMES as f64;
        let r = rig.run(pre_blocks.ceil() as usize + 1, timed);
        let m = median(&r.block_us);
        if m > pass.worst {
            pass.worst = m;
            pass.worst_bar = b + 1;
        }
        pass.blocks.extend_from_slice(&r.block_us);
    }
    pass
}

pub fn suite(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    let s = if plan.quick {
        Sweep {
            windows: 6,
            seconds: 0.5,
            runs: 1,
        }
    } else {
        Sweep {
            windows: 16,
            seconds: 1.0,
            runs: plan.runs,
        }
    };
    let budget = FRAMES as f64 / SAMPLE_RATE * 1e6;
    for (name, mut p) in crate::paced::songs() {
        if p.loop_region.is_none() || !p.loop_enabled {
            let end = p.content_end().max(Ticks::beats(4));
            p.loop_region = Some(LoopRegion {
                start: Ticks::ZERO,
                end,
            });
            p.loop_enabled = true;
        }
        let (lo, hi) = p
            .loop_region
            .as_ref()
            .map_or((0, Ticks::beats(4).0), |l| (l.start.0, l.end.0));
        let (mut med, mut p99, mut load, mut load99) = (vec![], vec![], vec![], vec![]);
        let mut worst_bars = Vec::new();
        let mut rig = Rig::with_workers(&p, Ticks(lo), FRAMES, cantus_engine::Workers::auto());
        rig.run(SAMPLE_RATE as usize / FRAMES, 0);
        let s0 = *rig.processor.stats();
        for run in 0..s.runs {
            let pass = sweep(&mut rig, lo, hi, p.tempo_bpm, &s, run);
            let (m, q) = (median(&pass.blocks), quantile(&pass.blocks, 0.99));
            med.push(m);
            p99.push(q);
            load.push(pass.worst / budget * 100.0);
            load99.push(q / budget * 100.0);
            worst_bars.push(pass.worst_bar);
        }
        let s1 = rig.processor.stats();
        let bars = (hi - lo) / Ticks::beats(4).0;
        let note = format!(
            "worst window at bar {} of {bars}; {} workers, {} voice forks ({} voices on helpers)",
            worst_bars
                .iter()
                .map(|b| b.to_string())
                .collect::<Vec<_>>()
                .join("/"),
            s1.workers,
            s1.voice_forks - s0.voice_forks,
            s1.voice_jobs_helped - s0.voice_jobs_helped
        );
        let id = format!("demo.{name}.b{FRAMES}");
        out.push(Metric::new(&id, "µs/block", Better::Lower, Summary::of(&med)).note(note));
        out.push(
            Metric::new(
                format!("{id}.p99"),
                "µs/block",
                Better::Lower,
                Summary::of(&p99),
            )
            .ungated(),
        );
        out.push(
            Metric::new(
                format!("{id}.load"),
                "% of budget",
                Better::Lower,
                Summary::of(&load),
            )
            .ungated()
            .note("the worst window's median; the beta's bar: under 70 %"),
        );
        out.push(
            Metric::new(
                format!("{id}.load99"),
                "% of budget",
                Better::Lower,
                Summary::of(&load99),
            )
            .ungated(),
        );
    }
    out
}
