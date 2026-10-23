//! The lock-step exchange between two real processes (ADR 0063): this test binary starts
//! a copy of itself as the workers' process (`worker_side`, which does nothing unless the
//! environment names a region), and the two meet in named shared memory — two exchanges
//! served by two workers sharing one lobby.

use cantus_rt::ipc::{self, Caller, Completer, Exchange, Lobby, LobbyBell, Outcome};
use cantus_rt::shm::{unique_name, SharedMemory};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const REGION: usize = 4096;
const ENV: &str = "CANTUS_IPC_TEST_REGION";
const MODE: &str = "CANTUS_IPC_TEST_MODE";
/// Exchange `i` at `i * 1024`, its payload at `i * 1024 + 512`; the lobby at 3 072.
const LOBBY: usize = 3072;

fn exchange(mem: &SharedMemory, i: usize) -> *const Exchange {
    // SAFETY: inside the 4 096-byte, page-aligned region, 128-byte aligned.
    unsafe { mem.as_ptr().add(i * 1024) as *const Exchange }
}

fn lobby(mem: &SharedMemory) -> *const Lobby {
    // SAFETY: as above.
    unsafe { mem.as_ptr().add(LOBBY) as *const Lobby }
}

fn payload(mem: &SharedMemory, i: usize) -> &AtomicU64 {
    // SAFETY: an aligned AtomicU64 inside the live region.
    unsafe { &*(mem.as_ptr().add(i * 1024 + 512) as *const AtomicU64) }
}

fn spawn_worker(name: &str, mode: &str) -> Child {
    Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", "worker_side", "--nocapture", "--test-threads=1"])
        .env(ENV, name)
        .env(MODE, mode)
        .spawn()
        .expect("start the workers' copy")
}

/// The workers' copy: two threads serving both exchanges; each job doubles the number and
/// adds one. `abort-after-3` dies on the fourth job; `slow-first` takes 60 ms over its
/// first. `u64::MAX` asks it to stop.
#[test]
fn worker_side() {
    let Ok(name) = std::env::var(ENV) else {
        return; // an ordinary test run: nothing to serve
    };
    let mode = std::env::var(MODE).unwrap_or_default();
    let mem = Arc::new(SharedMemory::open(&name, REGION).expect("open the region"));
    let stop = Arc::new(AtomicBool::new(false));
    let served = Arc::new(AtomicU64::new(0));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let (mem, stop, served, name, mode) = (
                mem.clone(),
                stop.clone(),
                served.clone(),
                name.clone(),
                mode.clone(),
            );
            std::thread::spawn(move || {
                // SAFETY: the region outlives the thread (the Arc).
                let bell = unsafe { LobbyBell::open(&name, lobby(&mem)) }.expect("lobby bell");
                let done: Vec<Completer> = (0..2)
                    // SAFETY: as above.
                    .map(|i| unsafe { Completer::open(&format!("{name}-{i}"), exchange(&mem, i)) })
                    .collect::<Result<_, _>>()
                    .expect("caller bells");
                ipc::serve(
                    &bell,
                    Duration::from_micros(200),
                    Some(Duration::from_millis(50)),
                    &stop,
                    || (0..2).find_map(|i| done[i].exchange().take().map(|n| (i, n))),
                    || (0..2).any(|i| done[i].exchange().pending()),
                    |i, n| {
                        let k = served.fetch_add(1, Ordering::SeqCst) + 1;
                        if mode == "abort-after-3" && k == 4 {
                            std::process::abort();
                        }
                        if mode == "slow-first" && k == 1 {
                            std::thread::sleep(Duration::from_millis(60));
                        }
                        let p = payload(&mem, i);
                        let x = p.load(Ordering::Relaxed);
                        if x == u64::MAX {
                            stop.store(true, Ordering::SeqCst);
                        } else {
                            p.store(x * 2 + 1, Ordering::Relaxed);
                        }
                        done[i].complete(n);
                    },
                );
            })
        })
        .collect();
    for t in threads {
        let _ = t.join();
    }
}

struct Pair {
    name: String,
    mem: SharedMemory,
    /// Kept alive: the lobby's bell exists while one side holds it.
    _bell: LobbyBell,
    callers: Vec<Caller>,
    child: Child,
}

fn pair(mode: &str) -> Pair {
    let name = unique_name("cantus-ipc-test");
    let mem = SharedMemory::create(&name, REGION).expect("make the region");
    // SAFETY: `mem` lives in the same `Pair` as the bells, dropped after them.
    let bell = unsafe { LobbyBell::create(&name, lobby(&mem)) }.expect("the lobby bell");
    let callers = (0..2)
        .map(|i| {
            // SAFETY: as above.
            unsafe {
                Caller::create(
                    &format!("{name}-{i}"),
                    exchange(&mem, i),
                    &name,
                    lobby(&mem),
                )
            }
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("make the bells");
    let child = spawn_worker(&name, mode);
    Pair {
        name,
        mem,
        _bell: bell,
        callers,
        child,
    }
}

fn call(p: &Pair, i: usize, x: u64, deadline: Duration) -> Outcome {
    payload(&p.mem, i).store(x, Ordering::Relaxed);
    let n = p.callers[i].submit();
    p.callers[i].wait(
        n,
        Duration::from_micros(50),
        Some(Instant::now() + deadline),
    )
}

fn stop(p: &mut Pair) {
    assert_eq!(call(p, 0, u64::MAX, Duration::from_secs(10)), Outcome::Done);
    assert!(p.child.wait().expect("the workers end").success());
}

#[test]
fn round_trips_between_two_processes_carry_their_data() {
    let mut p = pair("");
    let mut times = Vec::new();
    for x in 0..2_000u64 {
        let i = (x % 2) as usize;
        let t = Instant::now();
        assert_eq!(
            call(&p, i, x, Duration::from_secs(10)),
            Outcome::Done,
            "job {x}"
        );
        times.push(t.elapsed());
        assert_eq!(payload(&p.mem, i).load(Ordering::Relaxed), x * 2 + 1);
    }
    times.sort();
    eprintln!(
        "round trip between processes: p50 {:?}, p99 {:?}",
        times[times.len() / 2],
        times[times.len() * 99 / 100]
    );
    stop(&mut p);
}

#[test]
fn two_callers_at_once_are_served_together() {
    let mut p = pair("");
    for round in 0..500u64 {
        // Both published before either waits: two workers, two jobs.
        payload(&p.mem, 0).store(round, Ordering::Relaxed);
        payload(&p.mem, 1).store(round + 7, Ordering::Relaxed);
        let a = p.callers[0].submit();
        let b = p.callers[1].submit();
        let d = Some(Instant::now() + Duration::from_secs(10));
        assert_eq!(p.callers[0].wait(a, Duration::ZERO, d), Outcome::Done);
        assert_eq!(p.callers[1].wait(b, Duration::ZERO, d), Outcome::Done);
        assert_eq!(payload(&p.mem, 0).load(Ordering::Relaxed), round * 2 + 1);
        assert_eq!(
            payload(&p.mem, 1).load(Ordering::Relaxed),
            (round + 7) * 2 + 1
        );
    }
    stop(&mut p);
}

#[test]
fn a_worker_process_that_dies_closes_the_exchange_and_the_caller_is_woken() {
    let mut p = pair("abort-after-3");
    for x in 0..3 {
        assert_eq!(call(&p, 0, x, Duration::from_secs(10)), Outcome::Done);
    }
    // The watchdog: waits for the process and closes the exchange.
    let closer = p.callers[0]
        .closer(&format!("{}-0", p.name))
        .expect("a closer");
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
    payload(&p.mem, 0).store(9, Ordering::Relaxed);
    let n = p.callers[0].submit();
    let out = p.callers[0].wait(n, Duration::from_micros(50), None);
    assert_eq!(out, Outcome::Closed);
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    watch.join().unwrap();
    let _ = p.child.wait();
}

#[test]
fn a_late_job_is_left_to_finish_and_seen_finished_later() {
    let mut p = pair("slow-first");
    let t = Instant::now();
    assert_eq!(call(&p, 0, 5, Duration::from_millis(5)), Outcome::Late);
    assert!(
        t.elapsed() < Duration::from_millis(50),
        "the deadline held: {:?}",
        t.elapsed()
    );
    // SAFETY: the exchange is in our live region.
    let (job, _) = unsafe { &*exchange(&p.mem, 0) }.numbers();
    let start = Instant::now();
    while !p.callers[0].finished(job) {
        assert!(start.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(payload(&p.mem, 0).load(Ordering::Relaxed), 11);
    assert_eq!(call(&p, 0, 1, Duration::from_secs(10)), Outcome::Done);
    assert_eq!(payload(&p.mem, 0).load(Ordering::Relaxed), 3);
    stop(&mut p);
}

/// The deadline is the bound the ADR states (review #12): a late job's wait ends within
/// 2 ms of it — not on the next tick of a coarse system timer (15.6 ms on Windows by
/// default). No worker runs, so every wait is late; the median of twenty is judged, so one
/// preempted wait on a busy machine does not decide it.
#[test]
fn a_late_wait_ends_within_two_milliseconds_of_its_deadline() {
    let name = unique_name("cantus-ipc-deadline");
    let mem = SharedMemory::create(&name, REGION).expect("make the region");
    // SAFETY: `mem` outlives the bell and the caller, dropped after them below.
    let bell = unsafe { LobbyBell::create(&name, lobby(&mem)) }.expect("the lobby bell");
    // SAFETY: as above.
    let caller =
        unsafe { Caller::create(&format!("{name}-0"), exchange(&mem, 0), &name, lobby(&mem)) }
            .expect("the caller");
    let deadline = Duration::from_millis(5);
    // The thread that waits with a deadline in Cantus is a real-time one (the device's
    // callback, a promoted pool worker); this one is promoted as a pool worker is. macOS
    // gives an ordinary thread's timers leeway — coalesced, they fired 1.3–3.8 ms late on
    // GitHub's runner (round 30) — and a time-constraint thread's none.
    let promotion = cantus_rt::thread::promote_worker(
        cantus_rt::thread::WorkerRole::Live,
        deadline,
        cantus_rt::thread::Waiting::Bounded { busy_share: 0.5 },
    );
    eprintln!(
        "deadline waits on a thread: {} (a Mac VM: {})",
        promotion.promotion,
        cantus_rt::thread::in_a_virtual_mac()
    );
    let mut took: Vec<Duration> = (0..20)
        .map(|_| {
            let n = caller.submit();
            let t = Instant::now();
            assert_eq!(
                caller.wait(n, Duration::from_micros(50), Some(t + deadline)),
                Outcome::Late
            );
            let e = t.elapsed();
            // The next job takes the late one's place (nobody serves them).
            // SAFETY: the exchange is in our live region.
            let x = unsafe { &*exchange(&mem, 0) };
            while x.take().is_some() {}
            e
        })
        .collect();
    took.sort();
    let median = took[took.len() / 2];
    // In a macOS virtual machine (GitHub's runner) the guest's timers fire 0.3–4.3 ms late
    // even on a time-constraint thread, whatever wait is used (round 30: os_sync's, with a
    // median of 7.6 ms for 5): the bound there is what a VM gives. On a Mac itself the 2 ms
    // bound holds (docs/mac-session.md measures it).
    let slack = if cantus_rt::thread::in_a_virtual_mac() {
        Duration::from_millis(5)
    } else {
        Duration::from_millis(2)
    };
    assert!(
        median < deadline + slack,
        "a 5 ms deadline waited {median:?} (all: {took:?})"
    );
    // The caller and the bell go before the region they point into (declared after it).
    let _ = (&caller, &bell);
}

/// Once the other side has mapped a region, its creator removes the name (review #43): on
/// Unix nothing is left in `/dev/shm` if Cantus is killed; both mappings keep sharing.
#[test]
fn an_unlinked_region_still_shares_its_pages_and_cannot_be_opened_again() {
    let name = unique_name("cantus-shm-unlink");
    let made = SharedMemory::create(&name, REGION).expect("make the region");
    let opened = SharedMemory::open(&name, REGION).expect("open it");
    made.unlink_name();
    made.unlink_name();
    payload(&made, 0).store(42, Ordering::Relaxed);
    assert_eq!(payload(&opened, 0).load(Ordering::Relaxed), 42);
    #[cfg(unix)]
    assert!(
        SharedMemory::open(&name, REGION).is_err(),
        "the name is gone"
    );
}
