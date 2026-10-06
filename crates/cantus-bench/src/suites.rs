//! The processor at scale, the offline render, and opening a project.

use crate::report::{Better, HotSpot, Metric};
use crate::rig::{Rig, SAMPLE_RATE};
use crate::stats::{median, quantile, Summary};
use crate::{sessions, Plan};
use cantus_core::{Snapshot, Ticks};
use cantus_engine::{render, EffectUnit, RenderOptions};
use std::sync::Arc;
use std::time::Instant;

/// Session sizes, in tracks. 100 is built and played, but the engine plays only its first
/// `MAX_TRACKS` (64): it is there so the ceiling shows in the report.
pub const SIZES: [usize; 4] = [10, 32, 64, 100];

/// The scaled sessions at 128 frames (and the largest at 64): block time, its 99th
/// percentile, and the load against the budget; the hot spots of the 64-track session.
pub fn engine(plan: &Plan, spots: &mut Vec<HotSpot>) -> Vec<Metric> {
    let mut out = Vec::new();
    let mut cases: Vec<(usize, usize)> = SIZES.iter().map(|n| (*n, 128)).collect();
    cases.push((64, 64));
    for (n, frames) in cases {
        let p = sessions::scaled(n);
        let start = p.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.start);
        let blocks = (plan.session_seconds * SAMPLE_RATE / frames as f64).ceil() as usize;
        let (mut med, mut p99, mut load) = (Vec::new(), Vec::new(), Vec::new());
        let (mut cpu, mut share) = (Vec::new(), Vec::new());
        let mut voices = 0;
        for run in 0..plan.runs {
            let mut rig = Rig::new(&p, start, frames);
            let r = rig.run(SAMPLE_RATE as usize / frames, blocks);
            let m = median(&r.block_us);
            med.push(m);
            p99.push(quantile(&r.block_us, 0.99));
            load.push(m / r.budget_us() * 100.0);
            cpu.extend(r.cpu_us_per_block());
            share.extend(r.cpu_share_pct());
            voices = r.voices;
            if run == 0 && n == 64 && frames == 128 {
                spots.extend(rig.hot_spots(&format!("s{n}.b{frames}"), 15));
            }
        }
        let id = format!("engine.s{n}.b{frames}");
        let note = if n > sessions::ENGINE_TRACKS {
            format!(
                "{voices} voices; the engine plays {} of {n} tracks (MAX_TRACKS)",
                sessions::ENGINE_TRACKS
            )
        } else {
            format!("{voices} voices at the end of the run")
        };
        let gate = n <= sessions::ENGINE_TRACKS;
        let m = Metric::new(&id, "µs/block", Better::Lower, Summary::of(&med)).note(note);
        out.push(if gate { m } else { m.ungated() });
        out.push(
            Metric::new(
                format!("{id}.p99"),
                "µs/block",
                Better::Lower,
                Summary::of(&p99),
            )
            .ungated(),
        );
        if !cpu.is_empty() {
            let m = Metric::new(
                format!("{id}.cpu"),
                "µs/block",
                Better::Lower,
                Summary::of(&cpu),
            )
            .note("thread CPU time: the work without the waits for a core");
            out.push(if gate { m } else { m.ungated() });
            out.push(
                Metric::new(
                    format!("{id}.share"),
                    "% of a core",
                    Better::Higher,
                    Summary::of(&share),
                )
                .ungated()
                .note("100 % on an idle machine; less is the neighbours"),
            );
        }
        out.push(
            Metric::new(
                format!("{id}.load"),
                "% of budget",
                Better::Lower,
                Summary::of(&load),
            )
            .ungated(),
        );
    }
    out
}

/// The profiler's own cost: the 64-track session with it on and off.
pub fn profiler_overhead(plan: &Plan) -> Vec<Metric> {
    let p = sessions::scaled(64);
    let start = p.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.start);
    let blocks = (plan.session_seconds * SAMPLE_RATE / 128.0).ceil() as usize;
    let mut pct = Vec::new();
    for _ in 0..plan.runs {
        let mut on = Rig::new(&p, start, 128);
        let mut off = Rig::new(&p, start, 128);
        off.processor.set_profiling(false);
        let a = median(&on.run(375, blocks).block_us);
        let b = median(&off.run(375, blocks).block_us);
        pct.push((a - b) / b * 100.0);
    }
    vec![Metric::new(
        "engine.profiler_overhead.s64",
        "% of block",
        Better::Lower,
        Summary::of(&pct),
    )
    .ungated()]
}

/// Offline render speed, in multiples of real time: each demo song whole (the export
/// path, `cantus_engine::render`, 512-frame blocks), and the scaled sessions' loop.
pub fn render_speed(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut cases: Vec<(String, cantus_core::Project, Ticks, Option<Ticks>)> = Vec::new();
    let demo_ids: &[&str] = if plan.quick {
        &["house"]
    } else {
        &["house", "ballad", "funk", "lofi", "ambient", "trap"]
    };
    for id in demo_ids {
        let p = (cantus_ui::demos::find(id).expect("demo").build)();
        cases.push((id.to_string(), p, Ticks::ZERO, None));
    }
    for n in [10usize, 64] {
        let p = sessions::scaled(n);
        let l = p.loop_region.clone().expect("a loop");
        cases.push((format!("s{n}"), p, l.start, Some(l.end)));
    }
    for (name, p, start, end) in cases {
        let snap = Arc::new(Snapshot::compile(&p, 1));
        let opts = RenderOptions {
            start,
            end,
            tail_seconds: 1.0,
            ..Default::default()
        };
        let (mut xrt, mut xrt_cpu) = (Vec::new(), Vec::new());
        for _ in 0..plan.runs {
            let (r, wall, cpu) = crate::cpu::timed(|| render(snap.clone(), &opts));
            let r = r.expect("renders");
            std::hint::black_box(r.peak);
            xrt.push(r.duration_seconds() / wall);
            xrt_cpu.extend(cpu.map(|c| r.duration_seconds() / c.max(1e-9)));
        }
        out.push(Metric::new(
            format!("render.{name}.xrt"),
            "× real time",
            Better::Higher,
            Summary::of(&xrt),
        ));
        if !xrt_cpu.is_empty() {
            out.push(
                Metric::new(
                    format!("render.{name}.xrt_cpu"),
                    "× real time",
                    Better::Higher,
                    Summary::of(&xrt_cpu),
                )
                .note("audio seconds per CPU second of the rendering thread"),
            );
        }
    }
    out
}

/// Opening a project: read and parse the file (with its integrity check), compile the
/// snapshot, and build every effect's and instrument's unit — what stands between a
/// double-click and the first note.
pub fn open(plan: &Plan) -> Vec<Metric> {
    let mut out = Vec::new();
    let dir = std::env::temp_dir().join(format!("cantus-bench-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    for n in [10usize, 64] {
        let p = sessions::scaled(n);
        let path = dir.join(format!("s{n}.cantus"));
        cantus_project::save(&p, &path, "bench").expect("saves");
        let (mut load, mut compile, mut units, mut total) = (vec![], vec![], vec![], vec![]);
        for _ in 0..plan.runs.max(3) {
            let t0 = Instant::now();
            let loaded = cantus_project::load(&path, true).expect("opens");
            let t1 = Instant::now();
            let snap = Snapshot::compile(&loaded.project, 1);
            let t2 = Instant::now();
            let inst = cantus_engine::inst::units_for(&snap, SAMPLE_RATE as f32, |_, _| false);
            let fx: Vec<EffectUnit> = snap
                .effect_slots()
                .map(|s| EffectUnit::new(s, SAMPLE_RATE as f32))
                .collect();
            let t3 = Instant::now();
            std::hint::black_box((inst.len(), fx.len()));
            let ms = |a: Instant, b: Instant| (b - a).as_secs_f64() * 1e3;
            load.push(ms(t0, t1));
            compile.push(ms(t1, t2));
            units.push(ms(t2, t3));
            total.push(ms(t0, t3));
        }
        for (part, v) in [
            ("read", load),
            ("compile", compile),
            ("units", units),
            ("total", total),
        ] {
            let m = Metric::new(
                format!("open.s{n}.{part}"),
                "ms",
                Better::Lower,
                Summary::of(&v),
            );
            out.push(if part == "total" { m } else { m.ungated() });
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
    out
}
