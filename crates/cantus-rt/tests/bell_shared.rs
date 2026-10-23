//! The cross-process bell and the region names on every system (ADR 0063, ADR 0066): the
//! wait each system uses, a ring through one mapping waking a sleeper on another mapping of
//! the same pages (what two processes do), an exact timed wait, and names within macOS's
//! 31-byte limit for POSIX shared memory.

use cantus_rt::ipc::{self, Bell};
use cantus_rt::shm::{unique_name, SharedMemory, MAX_NAME};
use std::sync::atomic::AtomicU32;
use std::time::{Duration, Instant};

const REGION: usize = 4096;

fn word(mem: &SharedMemory) -> *const AtomicU32 {
    mem.as_ptr() as *const AtomicU32
}

/// No system sleeps in a polling loop: Linux has its futex, Windows its event, macOS an
/// address wait (`os_sync_wait_on_address` on 14.4+, `__ulock_wait` before).
#[test]
fn every_system_has_a_real_wait() {
    let kind = ipc::wait_kind();
    if cfg!(target_os = "macos") {
        assert!(matches!(kind, "os_sync" | "ulock"), "{kind}");
    } else if cfg!(target_os = "linux") {
        assert_eq!(kind, "futex");
    } else if cfg!(windows) {
        assert_eq!(kind, "event");
    }
}

/// Two mappings of one region stand for two processes: their addresses differ, their pages
/// are the same. A ring through one wakes a sleeper on the other, promptly.
#[test]
fn a_ring_through_one_mapping_wakes_a_sleeper_on_another() {
    let name = unique_name("cantus-bell-test");
    let made = SharedMemory::create(&name, REGION).expect("make the region");
    let opened = SharedMemory::open(&name, REGION).expect("open it");
    assert_ne!(made.as_ptr(), opened.as_ptr());
    let bell_name = format!("{name}-b");
    // SAFETY: both words are the first four bytes of regions that outlive the bells.
    let ringer = unsafe { Bell::create(&bell_name, word(&made)) }.expect("bell");
    // SAFETY: as above.
    let sleeper = unsafe { Bell::open(&bell_name, word(&opened)) }.expect("bell");
    std::thread::scope(|s| {
        let epoch = sleeper.epoch();
        let waiter = s.spawn(move || {
            let start = Instant::now();
            let mut woke = false;
            // Spurious returns are allowed: loop until the word moves, as the callers do.
            while start.elapsed() < Duration::from_secs(5) {
                if sleeper.epoch() != epoch {
                    woke = true;
                    break;
                }
                sleeper.wait(epoch, Some(Duration::from_secs(5)));
            }
            (woke, start.elapsed())
        });
        std::thread::sleep(Duration::from_millis(50));
        let rung = Instant::now();
        ringer.ring();
        let (woke, _) = waiter.join().expect("the sleeper");
        assert!(woke, "the ring reached the other mapping");
        let late = rung.elapsed();
        assert!(
            late < Duration::from_millis(500),
            "woken {late:?} after the ring"
        );
    });
}

/// A timed wait that nobody rings ends at its timeout, not long after (the deadline of a
/// caller waiting for a plug-in's block).
#[test]
fn a_timed_wait_ends_at_its_timeout() {
    let name = unique_name("cantus-bell-timeout");
    let made = SharedMemory::create(&name, REGION).expect("make the region");
    // SAFETY: the word is the region's first four bytes; the region outlives the bell.
    let bell = unsafe { Bell::create(&format!("{name}-b"), word(&made)) }.expect("bell");
    let epoch = bell.epoch();
    let start = Instant::now();
    // As the exchange's callers do (`Exchange::wait` recomputes what is left of its
    // deadline): a system's timed wait may end a little before its timeout (Windows: 28.6 ms
    // of 30 on GitHub's runner once), so the deadline, not one wait, is what ends it.
    let mut timed_out = false;
    while start.elapsed() < Duration::from_millis(30) {
        if !bell.wait(epoch, Some(Duration::from_millis(30) - start.elapsed())) {
            timed_out = true;
        }
    }
    let took = start.elapsed();
    assert!(timed_out, "nobody rang, yet every wait said rung");
    assert!(took >= Duration::from_millis(30), "{took:?}");
    assert!(took < Duration::from_millis(500), "{took:?}");
}

/// Generated names fit every system's limit (31 bytes with the `/` on macOS), stay unique,
/// and work; a longer one is refused up front rather than by the system's `ENAMETOOLONG`.
#[test]
fn region_names_fit_the_system_limit() {
    let long = "cantus-a-prefix-much-longer-than-macos-allows";
    let names: Vec<String> = (0..64).map(|_| unique_name(long)).collect();
    for n in &names {
        assert!(n.len() <= MAX_NAME, "{n} is {} bytes", n.len());
    }
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "unique");
    let region = SharedMemory::create(&names[0], REGION).expect("a generated name works");
    assert_eq!(region.len(), REGION);
    let too_long = "x".repeat(MAX_NAME + 1);
    let e = SharedMemory::create(&too_long, REGION).expect_err("too long");
    assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
    if cfg!(target_vendor = "apple") {
        assert_eq!(MAX_NAME, 30);
        assert!(unique_name("cantus-plugin").starts_with("cantus-"));
    } else {
        assert!(unique_name("cantus-plugin").starts_with("cantus-plugin-"));
    }
}

const WAKE_ENV: &str = "CANTUS_BELL_TEST_REGION";

/// The other process of [`a_ring_wakes_another_process_promptly`]: sleeps on the bell at
/// byte 0 with no timeout, and after each wake-up answers by moving the word at byte 64.
#[test]
fn bell_other_process() {
    let Ok(name) = std::env::var(WAKE_ENV) else {
        return; // an ordinary run: nothing to do
    };
    let mem = SharedMemory::open(&name, REGION).expect("open the region");
    // SAFETY: the region outlives both bells; both words are aligned atomics in it.
    let bell = unsafe { Bell::open(&format!("{name}-b"), word(&mem)) }.expect("bell");
    // SAFETY: byte 64 of the 4 096-byte, page-aligned region: an aligned u32 for its life.
    let answer = unsafe { &*(mem.as_ptr().add(64) as *const AtomicU32) };
    for _ in 0..20 {
        let epoch = bell.epoch();
        answer.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        while bell.epoch() == epoch {
            bell.wait(epoch, None);
        }
    }
    answer.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// Across processes (the plug-in sandbox's case): a ring wakes a sleeper in another
/// process within a millisecond or so — not when its park times out. Measured twenty
/// times; the worst must stay far under a block's deadline.
#[test]
fn a_ring_wakes_another_process_promptly() {
    use std::sync::atomic::Ordering::SeqCst;
    let name = unique_name("cantus-bell-x");
    let mem = SharedMemory::create(&name, REGION).expect("make the region");
    // SAFETY: as in the other tests.
    let bell = unsafe { Bell::create(&format!("{name}-b"), word(&mem)) }.expect("bell");
    // SAFETY: byte 64 of the 4 096-byte, page-aligned region: an aligned u32 for its life.
    let answer = unsafe { &*(mem.as_ptr().add(64) as *const AtomicU32) };
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "bell_other_process",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(WAKE_ENV, &name)
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("start the other process");
    let wait_for = |n: u32| {
        let t = Instant::now();
        while answer.load(SeqCst) < n {
            assert!(t.elapsed() < Duration::from_secs(10), "no answer {n}");
            std::hint::spin_loop();
        }
        t.elapsed()
    };
    wait_for(1);
    let mut times = Vec::new();
    for n in 2..=21 {
        // Let it fall asleep, then ring and time the answer.
        std::thread::sleep(Duration::from_millis(5));
        let t = Instant::now();
        bell.ring();
        let _ = wait_for(n);
        times.push(t.elapsed());
    }
    let _ = child.wait();
    times.sort();
    let (median, worst) = (times[times.len() / 2], times[times.len() - 1]);
    eprintln!(
        "{} wake-ups across processes: median {median:?}, worst {worst:?}",
        ipc::wait_kind()
    );
    assert!(median < Duration::from_millis(2), "{times:?}");
    assert!(worst < Duration::from_millis(50), "{times:?}");
}
