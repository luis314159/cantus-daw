//! Fork–join inside a task (`cantus_rt::fork`, ADR 0038): every job runs exactly once,
//! wherever it runs; the forker returns only after all of them (and its borrows end with
//! its frame); nobody present means the forker runs them in order; a panicking job neither
//! hangs nor frees what a helper is using; and forking allocates nothing.

use cantus_rt::fork::{Forks, Job};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::*};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// `helpers` threads present in the pool, helping until `stop`.
fn helpers(
    forks: &Arc<Forks>,
    n: usize,
    stop: &Arc<AtomicBool>,
) -> Vec<std::thread::JoinHandle<()>> {
    (0..n)
        .map(|_| {
            let (f, s) = (forks.clone(), stop.clone());
            std::thread::spawn(move || {
                f.presence(true);
                while !s.load(Relaxed) {
                    if !f.help() {
                        std::hint::spin_loop();
                    }
                }
                f.presence(false);
            })
        })
        .collect()
}

fn wait_present(forks: &Forks, n: usize) {
    let t0 = Instant::now();
    while forks.present() < n {
        assert!(
            t0.elapsed() < Duration::from_secs(10),
            "helpers never arrived"
        );
        std::thread::yield_now();
    }
}

/// Fork `counts.len()` jobs, each adding its index into its own counter.
fn fork_counting(forks: &Forks, counts: &mut [u64], spin: u32) {
    let mut work: Vec<_> = counts
        .iter_mut()
        .enumerate()
        .map(|(i, c)| {
            move || {
                for _ in 0..spin {
                    std::hint::spin_loop();
                }
                *c += i as u64 + 1;
            }
        })
        .collect();
    let mut jobs: Vec<Job<'_>> = work
        .iter_mut()
        .map(|w| Some(w as &mut (dyn FnMut() + Send)))
        .collect();
    forks.run(&mut jobs);
    assert!(jobs.iter().all(|j| j.is_none()), "a job was left");
}

#[test]
fn every_job_runs_exactly_once_with_helpers() {
    let forks = Arc::new(Forks::new());
    let stop = Arc::new(AtomicBool::new(false));
    let hs = helpers(&forks, 3, &stop);
    forks.presence(true);
    wait_present(&forks, 4);
    for round in 0..2_000u64 {
        let n = 2 + (round % 40) as usize;
        let mut counts = vec![0u64; n];
        fork_counting(&forks, &mut counts, (round % 7) as u32 * 50);
        for (i, c) in counts.iter().enumerate() {
            assert_eq!(
                *c,
                i as u64 + 1,
                "round {round}: job {i} ran {c} times' worth"
            );
        }
    }
    forks.presence(false);
    stop.store(true, Relaxed);
    for h in hs {
        h.join().unwrap();
    }
    let (helped, forked) = forks.counts();
    assert_eq!(forked, 2_000, "every fork published");
    assert!(helped > 0, "the helpers never took a job");
}

#[test]
fn nobody_present_means_the_forker_runs_them_in_order() {
    let forks = Forks::new();
    let order = std::sync::Mutex::new(Vec::new());
    let mut work: Vec<_> = (0..10)
        .map(|i| {
            let order = &order;
            move || order.lock().unwrap().push(i)
        })
        .collect();
    let mut jobs: Vec<Job<'_>> = work
        .iter_mut()
        .map(|w| Some(w as &mut (dyn FnMut() + Send)))
        .collect();
    forks.run(&mut jobs);
    assert_eq!(*order.lock().unwrap(), (0..10).collect::<Vec<_>>());
    assert_eq!(forks.counts(), (0, 0), "nothing was published");
}

#[test]
fn several_forkers_at_once_each_get_their_own_jobs_back() {
    let forks = Arc::new(Forks::new());
    let stop = Arc::new(AtomicBool::new(false));
    let hs = helpers(&forks, 2, &stop);
    let forkers: Vec<_> = (0..cantus_rt::fork::SLOTS + 2)
        .map(|k| {
            let f = forks.clone();
            std::thread::spawn(move || {
                f.presence(true);
                for round in 0..500 {
                    let mut counts = vec![0u64; 3 + (round + k) % 13];
                    fork_counting(&f, &mut counts, 20);
                    for (i, c) in counts.iter().enumerate() {
                        assert_eq!(*c, i as u64 + 1, "forker {k}, round {round}, job {i}");
                    }
                }
                f.presence(false);
            })
        })
        .collect();
    for f in forkers {
        f.join().unwrap();
    }
    stop.store(true, Relaxed);
    for h in hs {
        h.join().unwrap();
    }
}

#[test]
fn a_panicking_job_neither_hangs_the_fork_nor_frees_what_others_use() {
    let forks = Arc::new(Forks::new());
    let stop = Arc::new(AtomicBool::new(false));
    let hs = helpers(&forks, 2, &stop);
    forks.presence(true);
    wait_present(&forks, 3);
    let finished = AtomicUsize::new(0);
    let entered = AtomicBool::new(false);
    let forker = std::thread::current().id();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // On the forker a job waits until a helper is inside one, then panics; on a helper
        // it says it entered and works for 5 ms — so a helper is mid-job, for certain, when
        // the forker unwinds, and `run` must wait for it.
        let mut work: Vec<_> = (0..64)
            .map(|_| {
                let (finished, entered) = (&finished, &entered);
                move || {
                    if std::thread::current().id() == forker {
                        let t0 = Instant::now();
                        while !entered.load(SeqCst) && t0.elapsed() < Duration::from_secs(10) {
                            std::hint::spin_loop();
                        }
                        panic!("a job fails on the forker");
                    }
                    entered.store(true, SeqCst);
                    let t0 = Instant::now();
                    while t0.elapsed() < Duration::from_millis(5) {
                        std::hint::spin_loop();
                    }
                    finished.fetch_add(1, SeqCst);
                }
            })
            .collect();
        let mut jobs: Vec<Job<'_>> = work
            .iter_mut()
            .map(|w| Some(w as &mut (dyn FnMut() + Send)))
            .collect();
        forks.run(&mut jobs);
    }));
    assert!(r.is_err(), "the panic came back to the forker");
    assert!(entered.load(SeqCst), "no helper ever entered a job");
    // The fork is over: whatever a helper took finished before `run` unwound.
    let after = finished.load(SeqCst);
    assert!(
        after > 0,
        "run unwound while a helper was still inside its job"
    );
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        finished.load(SeqCst),
        after,
        "a job ran after its fork ended"
    );
    // And the slot is free again.
    let mut counts = vec![0u64; 8];
    fork_counting(&forks, &mut counts, 0);
    assert!(counts.iter().enumerate().all(|(i, c)| *c == i as u64 + 1));
    forks.presence(false);
    stop.store(true, Relaxed);
    for h in hs {
        h.join().unwrap();
    }
}

/// With the `fuzz` feature: threads stalled at random inside the protocol's windows
/// ([`ForkSite`]) — a helper registered but not yet re-reading the state, a forker between
/// opening and claiming or between closing and waiting, a thread holding a claimed job —
/// still run every job exactly once, with several forkers sharing the slots.
#[cfg(feature = "fuzz")]
#[test]
fn stalls_inside_the_protocol_lose_or_repeat_no_job() {
    use cantus_rt::fork::ForkSite;
    use std::sync::atomic::AtomicU64;
    let forks = Arc::new(Forks::new());
    let hits: Arc<[AtomicU64; 4]> = Arc::new(Default::default());
    let tick = Arc::new(AtomicU64::new(0x9E37_79B9));
    {
        let (hits, tick) = (hits.clone(), tick.clone());
        assert!(forks.set_hook(Arc::new(move |site: ForkSite| {
            let k = site as usize;
            hits[k].fetch_add(1, Relaxed);
            // One hit in eight stalls, for up to ~20 µs of spinning.
            let x = tick.fetch_add(0x9E37_79B9_7F4A_7C15, Relaxed);
            let x = (x ^ (x >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            if x >> 61 == 0 {
                for _ in 0..(x >> 40) % 4_000 {
                    std::hint::spin_loop();
                }
            }
        })));
        assert!(!forks.set_hook(Arc::new(|_| {})), "a hook is set once");
    }
    let stop = Arc::new(AtomicBool::new(false));
    let hs = helpers(&forks, 3, &stop);
    let forkers: Vec<_> = (0..cantus_rt::fork::SLOTS + 1)
        .map(|k| {
            let f = forks.clone();
            std::thread::spawn(move || {
                f.presence(true);
                for round in 0..400 {
                    let mut counts = vec![0u64; 2 + (round + k) % 17];
                    fork_counting(&f, &mut counts, (round % 5) as u32 * 30);
                    for (i, c) in counts.iter().enumerate() {
                        assert_eq!(*c, i as u64 + 1, "forker {k}, round {round}, job {i}");
                    }
                }
                f.presence(false);
            })
        })
        .collect();
    for f in forkers {
        f.join().unwrap();
    }
    stop.store(true, Relaxed);
    for h in hs {
        h.join().unwrap();
    }
    let hits: Vec<u64> = hits.iter().map(|h| h.load(Relaxed)).collect();
    eprintln!("hook hits (opened, registered, claimed, closing): {hits:?}");
    assert!(
        hits.iter().all(|h| *h > 0),
        "a site was never reached: {hits:?}"
    );
}
