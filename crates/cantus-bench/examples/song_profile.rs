//! Where a song's blocks go, node by node (serial unless `CANTUS_WORKERS` says otherwise):
//! the heaviest nodes in µs per block, the block's median and 99th percentile. A measuring
//! aid for the pool (round 22): the critical path of a song is its heaviest chain.
//!
//! ```text
//! cargo run --release -p cantus-bench --example song_profile -- <song> <frames> <seconds>
//! ```

use cantus_bench::rig::{Rig, SAMPLE_RATE};
use cantus_core::{LoopRegion, Ticks};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let song = args.first().cloned().unwrap_or_else(|| "startup".into());
    let frames: usize = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(256);
    let seconds: usize = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(10);
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let mut p = if song == "startup" {
        cantus_ui::demos::startup_project()
    } else {
        (cantus_ui::demos::find(&song).expect("a demo").build)()
    };
    if p.loop_region.is_none() || !p.loop_enabled {
        p.loop_region = Some(LoopRegion {
            start: Ticks::ZERO,
            end: p.content_end().max(Ticks::beats(4)),
        });
        p.loop_enabled = true;
    }
    let start = p.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.start);
    let mut rig = Rig::new(&p, start, frames);
    let blocks = seconds * SAMPLE_RATE as usize / frames;
    let r = rig.run(SAMPLE_RATE as usize / frames, blocks);
    let mut v = r.block_us.clone();
    v.sort_by(|a, b| a.total_cmp(b));
    println!(
        "{song} at {frames}: p50 {:.0} µs, p99 {:.0} µs, max {:.0} µs over {} blocks",
        v[v.len() / 2],
        v[v.len() * 99 / 100],
        v[v.len() - 1],
        v.len()
    );
    for h in rig.hot_spots(&song, 20) {
        println!(
            "  {:>8.1} µs/block  {:>5.1} %  {}",
            h.us_per_block, h.share_pct, h.node
        );
    }
}
