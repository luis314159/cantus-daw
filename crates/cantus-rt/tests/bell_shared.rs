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
    let mut timed_out = false;
    while start.elapsed() < Duration::from_millis(30) {
        if !bell.wait(epoch, Some(Duration::from_millis(30) - start.elapsed())) {
            timed_out = true;
            break;
        }
    }
    let took = start.elapsed();
    assert!(timed_out || took >= Duration::from_millis(30));
    assert!(took >= Duration::from_millis(29), "{took:?}");
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
