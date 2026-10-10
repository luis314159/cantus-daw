//! Voices on several threads (ADR 0038), measured in pairs: each song at the given block
//! size on the application's pool (`Workers::auto()`), the same processor alternating
//! **runs with the voice split on and off** — A B, B A, A B… — so both sides see the same
//! machine within seconds, and **the same music**: pair *k* seeks both sides (and the
//! serial engine, `W = 0`, for scale) to the same point, *k*/`pairs` of the way through
//! the song, warms each up for half a second, then times it. Prints each side's median
//! block and 99th percentile (median of runs), the median of the per-pair ratios on/off,
//! and the median block's share of the budget.
//!
//! ```text
//! cargo run --release -p cantus-bench --example voice_ab -- [frames] [seconds per run] [pairs] [songs]
//! cargo run --release -p cantus-bench --example voice_ab -- 256 2 6 startup,ballad,trap
//! ```

use cantus_bench::rig::{Rig, SAMPLE_RATE};
use cantus_bench::stats::{median, quantile};
use cantus_core::{LoopRegion, Ticks};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let frames: usize = args.first().and_then(|a| a.parse().ok()).unwrap_or(256);
    let seconds: f64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(2.0);
    let pairs: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(6);
    let only: Option<Vec<String>> = args
        .get(3)
        .map(|s| s.split(',').map(|x| x.trim().to_string()).collect());
    let budget = frames as f64 / SAMPLE_RATE * 1e6;
    let blocks = (seconds * SAMPLE_RATE / frames as f64).ceil() as usize;
    println!(
        "voice_ab: {frames} frames ({:.0} µs budget), {seconds} s per run, {pairs} pairs, {} workers",
        budget,
        cantus_engine::Workers::auto().count
    );
    println!("| Song | Serial p50 | Off p50 | On p50 | On/off p50 | Off p99 | On p99 | On/off p99 | Off load | On load | Forks/s | Helped/s |");
    println!("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for (name, mut p) in cantus_bench::paced::songs() {
        if only.as_ref().is_some_and(|o| !o.contains(&name)) {
            continue;
        }
        if p.loop_region.is_none() || !p.loop_enabled {
            let end = p.content_end().max(Ticks::beats(4));
            p.loop_region = Some(LoopRegion {
                start: Ticks::ZERO,
                end,
            });
            p.loop_enabled = true;
        }
        let start = p.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.start);
        let warm = SAMPLE_RATE as usize / frames;
        let (lo, hi) = p
            .loop_region
            .as_ref()
            .map_or((0, 0), |l| (l.start.0, l.end.0));
        let mut serial_rig = Rig::with_workers(&p, start, frames, cantus_engine::Workers::SERIAL);
        serial_rig.run(warm, 0);
        let mut rig = Rig::with_workers(&p, start, frames, cantus_engine::Workers::auto());
        rig.run(warm, 0);
        let (mut on50, mut off50, mut on99, mut off99) = (vec![], vec![], vec![], vec![]);
        let (mut r50, mut r99, mut ser50) = (vec![], vec![], vec![]);
        let (mut forks, mut helped, mut on_blocks) = (0u64, 0u64, 0usize);
        for k in 0..pairs {
            // The same stretch of the song for every side of the pair.
            let at = Ticks(lo + (hi - lo) * k as i64 / pairs as i64);
            let timed = |rig: &mut Rig| {
                rig.send(cantus_engine::EngineMessage::Seek(at));
                rig.run(warm / 2, 0);
                rig.run(0, blocks)
            };
            let mut side = |on: bool, rig: &mut Rig| {
                rig.processor.set_voice_split(on);
                let s0 = *rig.processor.stats();
                let r = timed(rig);
                let s1 = *rig.processor.stats();
                if on {
                    forks += s1.voice_forks - s0.voice_forks;
                    helped += s1.voice_jobs_helped - s0.voice_jobs_helped;
                    on_blocks += blocks;
                }
                (median(&r.block_us), quantile(&r.block_us, 0.99))
            };
            let order = if k % 2 == 0 {
                [true, false]
            } else {
                [false, true]
            };
            let mut got = [(0.0, 0.0); 2];
            for on in order {
                got[usize::from(on)] = side(on, &mut rig);
            }
            ser50.push(median(&timed(&mut serial_rig).block_us));
            let (off, on) = (got[0], got[1]);
            off50.push(off.0);
            off99.push(off.1);
            on50.push(on.0);
            on99.push(on.1);
            r50.push(on.0 / off.0);
            r99.push(on.1 / off.1);
        }
        let serial = median(&ser50);
        let secs = (on_blocks * frames) as f64 / SAMPLE_RATE;
        println!(
            "| {name} | {serial:.0} | {:.0} | {:.0} | {:.2} | {:.0} | {:.0} | {:.2} | {:.0} % | {:.0} % | {:.0} | {:.0} |",
            median(&off50),
            median(&on50),
            median(&r50),
            median(&off99),
            median(&on99),
            median(&r99),
            median(&off50) / budget * 100.0,
            median(&on50) / budget * 100.0,
            forks as f64 / secs.max(1e-9),
            helped as f64 / secs.max(1e-9),
        );
    }
}
