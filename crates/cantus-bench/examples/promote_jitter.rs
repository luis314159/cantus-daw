//! What promoting the callback thread buys (ADR 0031 R1): the bench's scaled session played
//! **paced** — one block per device period, as a driver's callback runs — on a thread left at
//! normal priority (cpal's WASAPI thread today) and on one promoted by
//! `cantus_rt::promote_callback` (MMCSS "Pro Audio" at HIGH), with the machine idle and with
//! every logical processor busy with normal-priority work (a build, a browser).
//!
//! ```text
//! cargo run -p cantus-bench --example promote_jitter --release -- [--tracks 6] [--frames 128]
//!     [--seconds 10] [--rounds 2] [--load <threads>]
//! ```
//!
//! Per block it records the **wake lateness** (how long after the period's start the thread
//! got the CPU) and the **block time** (`Processor::process`); a block that ends after the
//! next period starts is a **miss** — a dropout on a real device.

use cantus_bench::rig::{Rig, SAMPLE_RATE};
use cantus_bench::sessions;
use cantus_core::Ticks;
use cantus_rt::thread::{promote_callback, AudioHost};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Args {
    tracks: usize,
    frames: usize,
    seconds: f64,
    rounds: usize,
    load: usize,
}

fn args() -> Args {
    let mut a = Args {
        tracks: 6,
        frames: 128,
        seconds: 10.0,
        rounds: 2,
        load: std::thread::available_parallelism().map_or(4, |n| n.get()),
    };
    let v: Vec<String> = std::env::args().skip(1).collect();
    for w in v.windows(2) {
        match w[0].as_str() {
            "--tracks" => a.tracks = w[1].parse().unwrap_or(a.tracks),
            "--frames" => a.frames = w[1].parse().unwrap_or(a.frames),
            "--seconds" => a.seconds = w[1].parse().unwrap_or(a.seconds),
            "--rounds" => a.rounds = w[1].parse().unwrap_or(a.rounds),
            "--load" => a.load = w[1].parse().unwrap_or(a.load),
            _ => {}
        }
    }
    a
}

#[derive(Default)]
struct Run {
    late_us: Vec<f64>,
    block_us: Vec<f64>,
    misses: usize,
    promotion: String,
}

fn q(v: &[f64], p: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    s[((s.len() - 1) as f64 * p).round() as usize]
}

/// Busy normal-priority threads until `stop`.
fn load(n: usize, stop: &Arc<AtomicBool>) -> Vec<std::thread::JoinHandle<()>> {
    (0..n)
        .map(|i| {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut x = 1.0f64 + i as f64;
                while !stop.load(Ordering::Relaxed) {
                    for _ in 0..10_000 {
                        x = std::hint::black_box(x * 1.000_000_1 + 0.5).sqrt() + 1.0;
                    }
                }
                std::hint::black_box(x);
            })
        })
        .collect()
}

fn play(a: &Args, promote: bool, busy: usize) -> Run {
    let stop = Arc::new(AtomicBool::new(false));
    let loaders = load(busy, &stop);
    let (tracks, frames, seconds) = (a.tracks, a.frames, a.seconds);
    let run = std::thread::spawn(move || {
        let p = sessions::scaled(tracks);
        let start = p.loop_region.as_ref().map_or(Ticks::ZERO, |l| l.start);
        let mut rig = Rig::new(&p, start, frames);
        let period = Duration::from_secs_f64(frames as f64 / SAMPLE_RATE);
        let mut r = Run::default();
        if promote {
            // As the graph builder will: on the first block after the stream starts.
            r.promotion = promote_callback(AudioHost::Wasapi, period).to_string();
        } else {
            r.promotion = "normal priority".into();
        }
        for _ in 0..(SAMPLE_RATE as usize / frames) {
            rig.block();
        }
        let blocks = (seconds * SAMPLE_RATE / frames as f64) as usize;
        let mut next = Instant::now() + period;
        for _ in 0..blocks {
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            }
            let woke = Instant::now();
            let late = woke.saturating_duration_since(next);
            let us = rig.block();
            let done = Instant::now();
            r.late_us.push(late.as_secs_f64() * 1e6);
            r.block_us.push(us);
            if done > next + period {
                r.misses += 1;
                // A driver would have moved on: the next period starts from now.
                next = done;
            }
            next += period;
        }
        r
    })
    .join()
    .unwrap();
    stop.store(true, Ordering::Relaxed);
    for l in loaders {
        l.join().unwrap();
    }
    run
}

fn main() {
    let a = args();
    let budget = a.frames as f64 / SAMPLE_RATE * 1e6;
    println!(
        "scaled({}) at {} frames ({budget:.0} µs period), {} s per run, {} rounds, load {} threads",
        a.tracks, a.frames, a.seconds, a.rounds, a.load
    );
    let configs = [(false, 0), (true, 0), (false, a.load), (true, a.load)];
    let mut all: Vec<Vec<Run>> = configs.iter().map(|_| Vec::new()).collect();
    for round in 0..a.rounds {
        for (i, &(promote, busy)) in configs.iter().enumerate() {
            let r = play(&a, promote, busy);
            eprintln!(
                "round {round} promote={promote} load={busy}: {} misses ({})",
                r.misses, r.promotion
            );
            all[i].push(r);
        }
    }
    println!(
        "{:<26} {:>9} {:>9} {:>10} {:>10} | {:>9} {:>9} {:>10} {:>9} | {:>7}",
        "config",
        "late p50",
        "late p99",
        "late p99.9",
        "late max",
        "blk p50",
        "blk p99",
        "blk p99.9",
        "blk max",
        "misses"
    );
    let mut json = Vec::new();
    for (i, &(promote, busy)) in configs.iter().enumerate() {
        let late: Vec<f64> = all[i]
            .iter()
            .flat_map(|r| r.late_us.iter().copied())
            .collect();
        let blk: Vec<f64> = all[i]
            .iter()
            .flat_map(|r| r.block_us.iter().copied())
            .collect();
        let misses: usize = all[i].iter().map(|r| r.misses).sum();
        let name = format!(
            "{} / {}",
            if promote { "promoted" } else { "normal" },
            if busy > 0 { "loaded" } else { "idle" }
        );
        let row = [
            q(&late, 0.5),
            q(&late, 0.99),
            q(&late, 0.999),
            q(&late, 1.0),
            q(&blk, 0.5),
            q(&blk, 0.99),
            q(&blk, 0.999),
            q(&blk, 1.0),
        ];
        println!(
            "{name:<26} {:>9.1} {:>9.1} {:>10.1} {:>10.1} | {:>9.1} {:>9.1} {:>10.1} {:>9.1} | {:>4}/{}",
            row[0], row[1], row[2], row[3], row[4], row[5], row[6], row[7], misses, blk.len()
        );
        json.push(format!(
            "{{\"config\":\"{name}\",\"late_us\":{{\"p50\":{:.1},\"p99\":{:.1},\"p999\":{:.1},\"max\":{:.1}}},\"block_us\":{{\"p50\":{:.1},\"p99\":{:.1},\"p999\":{:.1},\"max\":{:.1}}},\"misses\":{misses},\"blocks\":{}}}",
            row[0], row[1], row[2], row[3], row[4], row[5], row[6], row[7], blk.len()
        ));
    }
    println!("[{}]", json.join(",\n "));
}
