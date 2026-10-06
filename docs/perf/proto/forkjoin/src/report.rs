//! The measurements and their printout.

use super::*;

fn pct(v: &mut [f64], p: f64) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p).round() as usize]
}

struct Stats {
    med: f64,
    p99: f64,
    p999: f64,
    max: f64,
}

fn time_blocks(pool: &Pool, blocks: usize, gap: Option<Duration>) -> Stats {
    let mut scratch = [0f32; FRAMES];
    let mut t = Vec::with_capacity(blocks);
    for b in 0..blocks + 200 {
        if let Some(g) = gap {
            // A live callback has idle gaps between blocks; emulate them with a busy wait
            // (sleeping would measure the OS timer, not the pool).
            let until = Instant::now() + g;
            while Instant::now() < until {
                std::hint::spin_loop();
            }
        }
        let t0 = Instant::now();
        pool.sh.run_block(&pool.threads, &mut scratch);
        if b >= 200 {
            t.push(t0.elapsed().as_secs_f64() * 1e6);
        }
    }
    let mut v = t.clone();
    Stats {
        med: pct(&mut v, 0.5),
        p99: pct(&mut v, 0.99),
        p999: pct(&mut v, 0.999),
        max: pct(&mut v, 1.0),
    }
}

fn bit_identity(defs: &[NodeDef], workers: usize) -> bool {
    let serial = Pool::new(defs.to_vec(), 0, Idle::Spin);
    let par = Pool::new(defs.to_vec(), workers, Idle::Spin);
    let mut s1 = [0f32; FRAMES];
    let mut s2 = [0f32; FRAMES];
    let mut same = true;
    for _ in 0..500 {
        serial.sh.run_block(&serial.threads, &mut s1);
        par.sh.run_block(&par.threads, &mut s2);
        let a = serial.sh.master_out();
        let b = par.sh.master_out();
        same &= a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits());
    }
    serial.stop();
    par.stop();
    same
}

fn wake_latency(workers: usize, idle: Idle) -> (f64, f64) {
    // A graph with nothing to do: what is left is seeing the block.
    let pool = Pool::new(graph(1, 1, 0, 0, 0), workers, idle);
    let mut scratch = [0f32; FRAMES];
    let mut worst = Vec::new();
    for _ in 0..400 {
        let until = Instant::now() + Duration::from_micros(1300);
        while Instant::now() < until {
            std::hint::spin_loop();
        }
        let t0 = pool.sh.base.elapsed().as_nanos() as u64;
        pool.sh.run_block(&pool.threads, &mut scratch);
        // Wait until every worker has seen it (measurement only).
        let deadline = Instant::now() + Duration::from_millis(50);
        while pool.sh.seen_at.iter().any(|s| s.load(Relaxed) < t0) && Instant::now() < deadline {
            std::hint::spin_loop();
        }
        let last = pool.sh.seen_at.iter().map(|s| s.load(Relaxed)).max().unwrap_or(t0);
        worst.push(last.saturating_sub(t0) as f64 / 1000.0);
    }
    pool.stop();
    let mut v = worst.clone();
    (pct(&mut v, 0.5), pct(&mut v, 0.99))
}

fn ring_copy(n: usize) -> (f64, f64) {
    let src: Vec<f32> = (0..n).map(|i| i as f32 * 0.25).collect();
    let cells: Vec<AtomicU32> = (0..n).map(|_| AtomicU32::new(0)).collect();
    let mut dst = vec![0f32; n];
    let reps = (4_000_000 / n).max(10);
    let t0 = Instant::now();
    for r in 0..reps {
        for (c, x) in cells.iter().zip(&src) {
            c.store((x + r as f32).to_bits(), Relaxed);
        }
        for (d, c) in dst.iter_mut().zip(&cells) {
            *d = f32::from_bits(c.load(Relaxed));
        }
    }
    let atomic = t0.elapsed().as_secs_f64() * 1e9 / (reps * n) as f64;
    std::hint::black_box(&dst);
    let mut mid = vec![0f32; n];
    let t0 = Instant::now();
    for r in 0..reps {
        for (m, x) in mid.iter_mut().zip(&src) {
            *m = x + r as f32;
        }
        dst.copy_from_slice(&mid);
        std::hint::black_box(&dst);
    }
    let plain = t0.elapsed().as_secs_f64() * 1e9 / (reps * n) as f64;
    (atomic, plain)
}

fn maxw() -> usize {
    std::env::var("FJ_MAXW").ok().and_then(|v| v.parse().ok()).unwrap_or(64)
}

fn bare_cost() {
    println!("## 6. Scheduling alone: 256 tracks -> 128 buses -> master, nodes that do nothing");
    println!("workers | median us per block | ns per node | p99 us");
    let mut g = graph(256, 128, 0, 0, 0);
    for d in &mut g {
        d.bare = true;
    }
    let n = g.len() as f64;
    for w in [0usize, 1, 2, 3, 4].into_iter().filter(|w| *w <= maxw()) {
        let pool = Pool::new(g.clone(), w, Idle::Spin);
        let s = time_blocks(&pool, 5000, None);
        println!("{w:7} | {:19.2} | {:11.1} | {:6.1}", s.med, s.med * 1000.0 / n, s.p99);
        pool.stop();
    }
}

pub fn run() {
    if std::env::var("FJ_ONLY").as_deref() == Ok("6") {
        bare_cost();
        return;
    }
    let cpus = thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    println!("forkjoin-proto: {cpus} logical CPUs, {FRAMES}-frame blocks");
    println!("budget at 48 kHz: 128 frames = 2667 us, 64 frames = 1333 us\n");

    println!("## 1. Dispatch overhead: 100 tracks, 16 buses, master, no DSP (gather only)");
    println!("workers | median us | p99 | p99.9 | max");
    let light = graph(100, 16, 0, 0, 0);
    for w in [0usize, 1, 3, 5, 7, 11].into_iter().filter(|w| *w <= maxw()) {
        let pool = Pool::new(light.clone(), w, Idle::Spin);
        let s = time_blocks(&pool, 3000, None);
        println!("{w:7} | {:9.1} | {:5.1} | {:5.1} | {:6.1}", s.med, s.p99, s.p999, s.max);
        pool.stop();
    }

    println!("\n## 2. Heavy: 100 tracks x 24 passes, 16 buses x 24, master x 48, back to back");
    println!("workers | median us | p99 | p99.9 | max | speed-up (median)");
    let heavy = graph(100, 16, 24, 24, 48);
    let mut serial = 0.0;
    for w in [0usize, 1, 2, 3, 4, 5, 7, 11, 13].into_iter().filter(|w| *w <= maxw()) {
        let pool = Pool::new(heavy.clone(), w, Idle::Spin);
        let s = time_blocks(&pool, 2000, None);
        if w == 0 {
            serial = s.med;
        }
        println!(
            "{w:7} | {:9.1} | {:5.1} | {:5.1} | {:6.1} | {:.2}x",
            s.med, s.p99, s.p999, s.max, serial / s.med
        );
        pool.stop();
    }

    println!("\n## 2b. Heavy, with a 1 ms idle gap before each block (as a live callback)");
    println!("workers | idle | median us | p99 | p99.9 | max");
    for (w, idle) in [(3usize, Idle::Spin), (3, Idle::Park), (5, Idle::Spin), (5, Idle::Park), (7, Idle::Spin), (7, Idle::Park)].into_iter().filter(|x| x.0 <= maxw()) {
        let pool = Pool::new(heavy.clone(), w, idle);
        let s = time_blocks(&pool, 1500, Some(Duration::from_micros(1000)));
        let name = if idle == Idle::Spin { "spin" } else { "park" };
        println!("{w:7} | {name} | {:9.1} | {:5.1} | {:5.1} | {:6.1}", s.med, s.p99, s.p999, s.max);
        pool.stop();
    }

    println!("\n## 3. Parallel equals serial, to the bit (500 blocks of the heavy graph)");
    for w in [1usize, 3, 7, 13].into_iter() {
        println!("workers {w:2}: {}", if bit_identity(&heavy, w) { "identical" } else { "DIFFERENT" });
    }

    println!("\n## 4. Time for the last worker to see a new block (us), after a 1.3 ms gap");
    println!("workers | idle | median | p99");
    for (w, idle) in [(3usize, Idle::Spin), (3, Idle::Park), (5, Idle::Spin), (5, Idle::Park), (11, Idle::Spin), (11, Idle::Park)].into_iter().filter(|x| x.0 <= maxw()) {
        let (m, p) = wake_latency(w, idle);
        let name = if idle == Idle::Spin { "spin" } else { "park" };
        println!("{w:7} | {name} | {m:6.1} | {p:6.1}");
    }

    println!("\n## 5. Copy through relaxed AtomicU32 cells vs a plain copy (ns per sample)");
    println!("frames | atomic write+read | plain write+copy");
    for n in [128usize, 1024, 8192] {
        let (a, p) = ring_copy(n);
        println!("{n:6} | {a:17.3} | {p:16.3}");
    }
}
