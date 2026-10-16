//! The lock-step exchange between two real processes (ADR 0063): this test binary starts
//! a copy of itself as the worker (`worker_side`, which does nothing unless the
//! environment names a region), and the two meet in named shared memory.

use cantus_rt::ipc::{Caller, Exchange, Next, Outcome, Worker};
use cantus_rt::shm::{unique_name, SharedMemory};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const REGION: usize = 4096;
const ENV: &str = "CANTUS_IPC_TEST_REGION";
const MODE: &str = "CANTUS_IPC_TEST_MODE";

/// The payload beside the exchange: one number in, one out.
fn payload(mem: &SharedMemory) -> &AtomicU64 {
    // SAFETY: the region is 4 096 bytes, page-aligned; the exchange takes the first 256
    // and an AtomicU64 at 512 is aligned and inside it.
    unsafe { &*(mem.as_ptr().add(512) as *const AtomicU64) }
}

fn exchange(mem: &SharedMemory) -> *const Exchange {
    mem.as_ptr() as *const Exchange
}

fn spawn_worker(name: &str, mode: &str) -> Child {
    Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", "worker_side", "--nocapture", "--test-threads=1"])
        .env(ENV, name)
        .env(MODE, mode)
        .spawn()
        .expect("start the worker copy")
}

/// The worker copy: doubles the number and adds one, job after job. `abort-after-3` dies
/// on the fourth job; `slow-first` takes 60 ms over its first.
#[test]
fn worker_side() {
    let Ok(name) = std::env::var(ENV) else {
        return; // an ordinary test run: nothing to serve
    };
    let mode = std::env::var(MODE).unwrap_or_default();
    let mem = SharedMemory::open(&name, REGION).expect("open the region");
    // SAFETY: the region outlives the worker (both live to the end of this function).
    let w = unsafe { Worker::open(&name, exchange(&mem)) }.expect("open the bells");
    let mut last = 0;
    let mut served = 0;
    loop {
        match w.next(last, Duration::from_micros(200), Some(Duration::from_secs(10))) {
            Next::Idle => return, // the caller went away
            Next::Job(n) => {
                served += 1;
                if mode == "abort-after-3" && served == 4 {
                    std::process::abort();
                }
                if mode == "slow-first" && served == 1 {
                    std::thread::sleep(Duration::from_millis(60));
                }
                let p = payload(&mem);
                let x = p.load(Ordering::Relaxed);
                if x == u64::MAX {
                    w.complete(n);
                    return; // told to stop
                }
                p.store(x * 2 + 1, Ordering::Relaxed);
                w.complete(n);
                last = n;
            }
        }
    }
}

struct Pair {
    name: String,
    mem: SharedMemory,
    caller: Caller,
    child: Child,
}

fn pair(mode: &str) -> Pair {
    let name = unique_name("cantus-ipc-test");
    let mem = SharedMemory::create(&name, REGION).expect("make the region");
    // SAFETY: `mem` lives in the same `Pair` as the caller, dropped after it.
    let caller = unsafe { Caller::create(&name, exchange(&mem)) }.expect("make the bells");
    let child = spawn_worker(&name, mode);
    Pair {
        name,
        mem,
        caller,
        child,
    }
}

fn call(p: &Pair, x: u64, deadline: Duration) -> Outcome {
    payload(&p.mem).store(x, Ordering::Relaxed);
    let n = p.caller.submit();
    p.caller
        .wait(n, Duration::from_micros(50), Some(Instant::now() + deadline))
}

#[test]
fn round_trips_between_two_processes_carry_their_data() {
    let mut p = pair("");
    let mut times = Vec::new();
    for x in 0..2_000u64 {
        let t = Instant::now();
        assert_eq!(call(&p, x, Duration::from_secs(10)), Outcome::Done, "job {x}");
        times.push(t.elapsed());
        assert_eq!(payload(&p.mem).load(Ordering::Relaxed), x * 2 + 1);
    }
    times.sort();
    eprintln!(
        "round trip between processes: p50 {:?}, p99 {:?}",
        times[times.len() / 2],
        times[times.len() * 99 / 100]
    );
    assert_eq!(call(&p, u64::MAX, Duration::from_secs(10)), Outcome::Done);
    assert!(p.child.wait().expect("the worker ends").success());
}

#[test]
fn a_worker_that_dies_closes_the_exchange_and_the_caller_is_woken() {
    let mut p = pair("abort-after-3");
    for x in 0..3 {
        assert_eq!(call(&p, x, Duration::from_secs(10)), Outcome::Done);
    }
    // The watchdog: waits for the process and closes the exchange.
    let closer = p.caller.closer(&p.name).expect("a closer");
    let mut child = std::mem::replace(
        &mut p.child,
        Command::new(std::env::current_exe().unwrap())
            .arg("--list")
            .spawn()
            .unwrap(),
    );
    let watch = std::thread::spawn(move || {
        let _ = child.wait();
        closer.close();
    });
    let t = Instant::now();
    // No deadline: only the watchdog's close can end this wait.
    payload(&p.mem).store(9, Ordering::Relaxed);
    let n = p.caller.submit();
    let out = p.caller.wait(n, Duration::from_micros(50), None);
    assert_eq!(out, Outcome::Closed);
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    watch.join().unwrap();
    let _ = p.child.wait();
}

#[test]
fn a_late_job_is_left_to_finish_and_seen_finished_later() {
    let mut p = pair("slow-first");
    let t = Instant::now();
    assert_eq!(call(&p, 5, Duration::from_millis(5)), Outcome::Late);
    assert!(
        t.elapsed() < Duration::from_millis(50),
        "the deadline held: {:?}",
        t.elapsed()
    );
    let (job, _) = p.caller_numbers();
    let start = Instant::now();
    while !p.caller.finished(job) {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(payload(&p.mem).load(Ordering::Relaxed), 11);
    assert_eq!(call(&p, 1, Duration::from_secs(10)), Outcome::Done);
    assert_eq!(payload(&p.mem).load(Ordering::Relaxed), 3);
    assert_eq!(call(&p, u64::MAX, Duration::from_secs(10)), Outcome::Done);
    let _ = p.child.wait();
}

impl Pair {
    fn caller_numbers(&self) -> (u32, u32) {
        // SAFETY: the exchange is at the start of our live region.
        unsafe { &*exchange(&self.mem) }.numbers()
    }
}
