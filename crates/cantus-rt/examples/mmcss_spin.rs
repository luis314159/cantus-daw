//! Do promoted *spinning* workers stall? (The sched builder's round-21 finding: pool threads in
//! MMCSS "Pro Audio" HIGH had 2.5–17.6 s blocks; with MMCSS off, at most 155 ms.)
//!
//! The pool's shape without the engine: a **paced callback** (one block per period) arms each
//! block; **K workers** each run a slice of it, then wait for the next block — **spinning**
//! the whole time (the pool's 1.5-period spin budget makes it continuous at steady state), or
//! spinning a **bounded** time and then parking until the callback unparks them. The block
//! ends when every worker's slice is done, so a stalled worker stalls the block. **L
//! normal-priority busy threads** stand in for the compilers. Each thread records the longest
//! gap between two iterations of its own loop: the time it was not running.
//!
//! ```text
//! cargo run -p cantus-rt --example mmcss_spin --release -- [--seconds 10] [--workers 4]
//!     [--load 20] [--work-us 300] [--cb-work-us 300] [--only <config>]
//! ```

#[cfg(not(windows))]
fn main() {
    println!("Windows only: MMCSS is what is measured.");
}

#[cfg(windows)]
fn main() {
    imp::main();
}

#[cfg(windows)]
mod imp {
    use cantus_rt::thread::{promote_callback, promote_worker, AudioHost, Waiting, WorkerRole};
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::*};
    use std::sync::Arc;
    use std::time::{Duration, Instant};
    use windows_sys::Win32::System::Threading::{
        AvSetMmThreadCharacteristicsW, AvSetMmThreadPriority, GetCurrentThread, SetThreadPriority,
        AVRT_PRIORITY_CRITICAL, THREAD_PRIORITY_TIME_CRITICAL,
    };

    const PERIOD: Duration = Duration::from_micros(2_667);

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Prio {
        Normal,
        /// MMCSS "Pro Audio" HIGH through `promote_worker(Live)` / `promote_callback`.
        ProAudioHigh,
        /// MMCSS "Pro Audio" CRITICAL.
        ProAudioCritical,
        /// `SetThreadPriority(THREAD_PRIORITY_TIME_CRITICAL)`, no MMCSS.
        TimeCritical,
    }

    #[derive(Clone, Copy, Debug)]
    struct Config {
        name: &'static str,
        callback: Prio,
        workers: Prio,
        /// Spin this fraction of a period after a slice, then park; `None` = spin forever.
        bounded: Option<f64>,
        k: usize,
        /// The callback does not wait for the workers (they only share the machine with it).
        detached: bool,
    }

    fn promote(p: Prio, callback: bool) {
        match p {
            Prio::Normal => {}
            Prio::TimeCritical if callback => {
                // SAFETY: pseudo-handle of this thread.
                unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) };
            }
            Prio::ProAudioHigh if callback => {
                promote_callback(AudioHost::Wasapi, PERIOD);
            }
            // Declared bounded so `promote_worker` grants MMCSS — the spinning configs then
            // break that promise on purpose: that is the case being measured.
            Prio::ProAudioHigh => std::mem::forget(promote_worker(
                WorkerRole::Live,
                PERIOD,
                Waiting::Bounded { busy_share: 0.5 },
            )),
            Prio::ProAudioCritical => {
                let name: Vec<u16> = "Pro Audio\0".encode_utf16().collect();
                let mut i = 0u32;
                // SAFETY: NUL-terminated name, live index; the handle lives with the thread.
                unsafe {
                    let h = AvSetMmThreadCharacteristicsW(name.as_ptr(), &mut i);
                    AvSetMmThreadPriority(h, AVRT_PRIORITY_CRITICAL);
                }
            }
            Prio::TimeCritical => {
                // SAFETY: pseudo-handle of this thread.
                unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_TIME_CRITICAL) };
            }
        }
    }

    /// Spins on the clock for `us` (real work: the core is busy); returns the longest gap
    /// between two clock reads inside it, µs (time the thread was not running).
    fn work(us: u64) -> u64 {
        let t = Instant::now();
        let mut last = t;
        let mut gap = 0u64;
        let mut x = 1.0f64;
        loop {
            let now = Instant::now();
            gap = gap.max(now.duration_since(last).as_micros() as u64);
            last = now;
            if now.duration_since(t).as_micros() >= us as u128 {
                break;
            }
            for _ in 0..64 {
                x = std::hint::black_box(x * 1.000_001 + 1e-9);
            }
        }
        std::hint::black_box(x);
        gap
    }

    struct Shared {
        epoch: AtomicU64,
        done: AtomicUsize,
        /// Workers promoted and running: the first block waits for all of them.
        ready: AtomicUsize,
        stop: AtomicBool,
        /// Each worker's longest gap between two loop iterations, µs.
        gap_us: Vec<AtomicU64>,
    }

    struct Out {
        late: Vec<f64>,
        block: Vec<f64>,
        cb_gap_max: f64,
        worker_gap_max: f64,
        misses: usize,
    }

    fn run(c: Config, seconds: f64, load: usize, work_us: u64, cb_work_us: u64) -> Out {
        let stop_load = Arc::new(AtomicBool::new(false));
        let loaders: Vec<_> = (0..load)
            .map(|_| {
                let s = stop_load.clone();
                std::thread::spawn(move || {
                    while !s.load(Relaxed) {
                        work(1000);
                    }
                })
            })
            .collect();
        let sh = Arc::new(Shared {
            epoch: AtomicU64::new(0),
            done: AtomicUsize::new(0),
            ready: AtomicUsize::new(0),
            stop: AtomicBool::new(false),
            gap_us: (0..c.k).map(|_| AtomicU64::new(0)).collect(),
        });
        let workers: Vec<_> = (0..c.k)
            .map(|w| {
                let sh = sh.clone();
                std::thread::spawn(move || {
                    promote(c.workers, false);
                    sh.ready.fetch_add(1, AcqRel);
                    let mut seen = 0u64;
                    let mut last = Instant::now();
                    let mut idle_since = Instant::now();
                    let mut gap = 0u64;
                    while !sh.stop.load(Relaxed) {
                        let now = Instant::now();
                        gap = gap.max(now.duration_since(last).as_micros() as u64);
                        last = now;
                        let e = sh.epoch.load(Acquire);
                        if e != seen {
                            seen = e;
                            gap = gap.max(work(work_us));
                            sh.done.fetch_add(1, AcqRel);
                            idle_since = Instant::now();
                            last = idle_since;
                            continue;
                        }
                        if let Some(f) = c.bounded {
                            if idle_since.elapsed() > PERIOD.mul_f64(f) {
                                // Park until the next block (the callback unparks us).
                                std::thread::park_timeout(PERIOD * 4);
                                idle_since = Instant::now();
                                last = idle_since;
                                continue;
                            }
                        }
                        std::hint::spin_loop();
                    }
                    sh.gap_us[w].store(gap, Relaxed);
                })
            })
            .collect();
        let handles: Vec<std::thread::Thread> =
            workers.iter().map(|h| h.thread().clone()).collect();
        let sh2 = sh.clone();
        let out = std::thread::spawn(move || {
            promote(c.callback, true);
            while sh2.ready.load(Acquire) < c.k {
                std::thread::sleep(Duration::from_millis(1));
            }
            // A second of blocks before measuring: thread start-up is not what is measured.
            let warm = (1.0 / PERIOD.as_secs_f64()) as usize;
            let blocks = (seconds / PERIOD.as_secs_f64()) as usize + warm;
            let (mut late, mut block) = (Vec::with_capacity(blocks), Vec::with_capacity(blocks));
            let mut misses = 0;
            let mut cb_gap = 0f64;
            let mut next = Instant::now() + PERIOD;
            for b in 0..blocks {
                if b == warm {
                    late.clear();
                    block.clear();
                    misses = 0;
                    cb_gap = 0.0;
                }
                let now = Instant::now();
                if next > now {
                    std::thread::sleep(next - now);
                }
                let woke = Instant::now();
                late.push(woke.saturating_duration_since(next).as_secs_f64() * 1e6);
                sh2.done.store(0, Relaxed);
                sh2.epoch.fetch_add(1, Release);
                if c.bounded.is_some() {
                    for t in &handles {
                        t.unpark();
                    }
                }
                cb_gap = cb_gap.max(work(cb_work_us) as f64);
                let mut last = Instant::now();
                while !c.detached && sh2.done.load(Acquire) < c.k {
                    let n = Instant::now();
                    cb_gap = cb_gap.max(n.duration_since(last).as_secs_f64() * 1e6);
                    last = n;
                    std::hint::spin_loop();
                }
                let end = Instant::now();
                block.push(end.duration_since(woke).as_secs_f64() * 1e6);
                if end > next + PERIOD {
                    misses += 1;
                    next = end;
                }
                next += PERIOD;
            }
            (late, block, misses, cb_gap)
        })
        .join()
        .unwrap();
        sh.stop.store(true, Relaxed);
        for t in &workers {
            t.thread().unpark();
        }
        for w in workers {
            w.join().unwrap();
        }
        stop_load.store(true, Relaxed);
        for l in loaders {
            l.join().unwrap();
        }
        let worker_gap_max = sh.gap_us.iter().map(|g| g.load(Relaxed)).max().unwrap_or(0) as f64;
        Out {
            late: out.0,
            block: out.1,
            misses: out.2,
            cb_gap_max: out.3,
            worker_gap_max,
        }
    }

    fn q(v: &[f64], p: f64) -> f64 {
        let mut s = v.to_vec();
        s.sort_by(f64::total_cmp);
        s[((s.len() - 1) as f64 * p).round() as usize]
    }

    pub fn main() {
        let mut seconds = 10.0;
        let mut k = 4;
        let mut load = std::thread::available_parallelism().map_or(8, |n| n.get());
        let (mut work_us, mut cb_work_us) = (300u64, 300u64);
        let mut only: Option<String> = None;
        let mut repeat = 1usize;
        let a: Vec<String> = std::env::args().skip(1).collect();
        for w in a.windows(2) {
            match w[0].as_str() {
                "--seconds" => seconds = w[1].parse().unwrap_or(seconds),
                "--workers" => k = w[1].parse().unwrap_or(k),
                "--load" => load = w[1].parse().unwrap_or(load),
                "--work-us" => work_us = w[1].parse().unwrap_or(work_us),
                "--cb-work-us" => cb_work_us = w[1].parse().unwrap_or(cb_work_us),
                "--only" => only = Some(w[1].clone()),
                "--repeat" => repeat = w[1].parse().unwrap_or(repeat),
                _ => {}
            }
        }
        use Prio::*;
        let all = [
            Config {
                name: "callback-only-normal",
                callback: Normal,
                workers: Normal,
                bounded: None,
                k: 0,
                detached: false,
            },
            Config {
                name: "callback-only-mmcss",
                callback: ProAudioHigh,
                workers: Normal,
                bounded: None,
                k: 0,
                detached: false,
            },
            Config {
                name: "callback-only-time-critical",
                callback: TimeCritical,
                workers: Normal,
                bounded: None,
                k: 0,
                detached: false,
            },
            Config {
                name: "spin-normal",
                callback: ProAudioHigh,
                workers: Normal,
                bounded: None,
                k,
                detached: false,
            },
            Config {
                name: "spin-mmcss-high",
                callback: ProAudioHigh,
                workers: ProAudioHigh,
                bounded: None,
                k,
                detached: false,
            },
            Config {
                name: "spin-mmcss-critical",
                callback: ProAudioHigh,
                workers: ProAudioCritical,
                bounded: None,
                k,
                detached: false,
            },
            Config {
                name: "spin-time-critical",
                callback: ProAudioHigh,
                workers: TimeCritical,
                bounded: None,
                k,
                detached: false,
            },
            Config {
                name: "bounded-mmcss-high-0.50",
                callback: ProAudioHigh,
                workers: ProAudioHigh,
                bounded: Some(0.5),
                k,
                detached: false,
            },
            Config {
                name: "bounded-mmcss-high-0.70",
                callback: ProAudioHigh,
                workers: ProAudioHigh,
                bounded: Some(0.70),
                k,
                detached: false,
            },
            Config {
                name: "bounded-mmcss-high-0.85",
                callback: ProAudioHigh,
                workers: ProAudioHigh,
                bounded: Some(0.85),
                k,
                detached: false,
            },
            Config {
                name: "bounded-mmcss-high-1.00",
                callback: ProAudioHigh,
                workers: ProAudioHigh,
                bounded: Some(1.0),
                k,
                detached: false,
            },
            Config {
                name: "detached-spin-mmcss-high",
                callback: ProAudioHigh,
                workers: ProAudioHigh,
                bounded: None,
                k,
                detached: true,
            },
            Config {
                name: "detached-spin-normal",
                callback: ProAudioHigh,
                workers: Normal,
                bounded: None,
                k,
                detached: true,
            },
            Config {
                name: "bounded-time-critical",
                callback: ProAudioHigh,
                workers: TimeCritical,
                bounded: Some(0.5),
                k,
                detached: false,
            },
        ];
        println!(
            "period {} µs, callback work {cb_work_us} µs + {k} workers × {work_us} µs per block, load {load} threads, {seconds} s each",
            PERIOD.as_micros()
        );
        println!(
            "{:<24} {:>9} {:>9} {:>9} | {:>9} {:>9} {:>10} | {:>11} {:>12} | {:>6}",
            "config",
            "late p99",
            "late max",
            "",
            "blk p50",
            "blk p99",
            "blk max",
            "cb gap max",
            "wkr gap max",
            "misses"
        );
        for c in (0..repeat).flat_map(|_| all) {
            if only.as_deref().is_some_and(|o| !c.name.contains(o)) {
                continue;
            }
            let o = run(c, seconds, load, work_us, cb_work_us);
            println!(
                "{:<24} {:>9.0} {:>9.0} {:>9} | {:>9.0} {:>9.0} {:>10.0} | {:>11.0} {:>12.0} | {:>4}/{}",
                c.name,
                q(&o.late, 0.99),
                q(&o.late, 1.0),
                "",
                q(&o.block, 0.5),
                q(&o.block, 0.99),
                q(&o.block, 1.0),
                o.cb_gap_max,
                o.worker_gap_max,
                o.misses,
                o.block.len()
            );
        }
    }
}
