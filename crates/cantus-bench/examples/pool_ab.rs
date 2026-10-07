//! A paired A/B driver for the pool (round 22): one scaled session, back to back, the
//! block times' median and 99th percentile on one JSON line. The same file builds against
//! the round-21 tree (it uses only `Rig` and `sessions::scaled`), so the two binaries can
//! run alternately on the same cores. The pool's workers come from `CANTUS_WORKERS`.
//!
//! ```text
//! cargo run --release -p cantus-bench --example pool_ab -- <tracks> <frames> <seconds>
//! ```

use cantus_bench::rig::{Rig, SAMPLE_RATE};
use cantus_bench::sessions;
use cantus_core::Ticks;

fn main() {
    let args: Vec<usize> = std::env::args()
        .skip(1)
        .filter_map(|a| a.parse().ok())
        .collect();
    let tracks = args.first().copied().unwrap_or(64);
    let frames = args.get(1).copied().unwrap_or(128);
    let seconds = args.get(2).copied().unwrap_or(4);
    let p = sessions::scaled(tracks);
    let start = p.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.start);
    let mut rig = Rig::new(&p, start, frames);
    let blocks = seconds * SAMPLE_RATE as usize / frames;
    let r = rig.run(SAMPLE_RATE as usize / frames, blocks);
    let mut v = r.block_us.clone();
    v.sort_by(|a, b| a.total_cmp(b));
    let q = |x: f64| v[((v.len() as f64 * x) as usize).min(v.len() - 1)];
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "{{\"tracks\":{tracks},\"frames\":{frames},\"blocks\":{},\"p50\":{:.1},\"p99\":{:.1},\"mean\":{:.1},\"max\":{:.1},\"voices\":{}}}",
        v.len(),
        q(0.5),
        q(0.99),
        mean,
        v[v.len() - 1],
        r.voices
    );
}
