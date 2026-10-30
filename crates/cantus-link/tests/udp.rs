//! Link over real sockets on this machine (ADR 0078 §5): handles in one process, and two
//! **separate processes** (this test binary run again as a child), on the loopback and a
//! port of the tests' own — never Link's group, never the network.
//!
//! Each child prints, every 100 ms of the system clock, the session's beat *at that exact
//! moment* (`Session::beat_at`), so the parent compares the two processes' beats at the
//! same instants, whatever the moment each line was printed.

use cantus_link::{Config, Link};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The system clock in µs: one clock both processes read.
fn wall() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as i64)
}

fn wait_until(limit: Duration, mut f: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if f() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn handles_in_one_process_find_each_other() {
    let _ = env_logger::builder().is_test(true).try_init();
    let cfg = Config::local(20_931);
    let a = Link::new(120.0, wall, cfg);
    let b = Link::new(97.0, wall, cfg);
    a.enable(true).expect("sockets");
    std::thread::sleep(Duration::from_millis(30));
    b.enable(true).expect("sockets");
    assert!(
        wait_until(Duration::from_secs(5), || a.session().peers == 1
            && b.session().peers == 1
            && (a.session().tempo - b.session().tempo).abs() < 1e-3),
        "a {:?} b {:?}",
        a.session(),
        b.session()
    );
    // Settled: the same beat at the same instant.
    std::thread::sleep(Duration::from_millis(500));
    let t = wall();
    let (sa, sb) = (a.session(), b.session());
    let err = (sa.beat_at(t) - sb.beat_at(t)).abs() * 60_000.0 / sa.tempo;
    println!("in-process: {err:.4} ms");
    assert!(err <= 1.0, "{err} ms");
    // A tempo change on one side reaches the other.
    b.set_tempo(141.0, wall());
    assert!(wait_until(Duration::from_secs(2), || (a.session().tempo
        - 141.0)
        .abs()
        < 1e-3));
    // Leaving: the other counts no peer at once (bye-bye).
    b.enable(false).unwrap();
    assert!(wait_until(Duration::from_secs(2), || a.session().peers == 0));
}

/// The child: a Link handle at the tempo in `CANTUS_LINK_CHILD`, printing its beats.
#[test]
fn child_peer() {
    let Ok(bpm) = std::env::var("CANTUS_LINK_CHILD") else {
        return;
    };
    let bpm: f64 = bpm.parse().unwrap();
    let link = Link::new(bpm, wall, Config::local(20_933));
    link.enable(true).expect("sockets");
    let end = Instant::now() + Duration::from_secs(6);
    let step = 100_000i64;
    let mut k = wall() / step + 1;
    while Instant::now() < end {
        let at = k * step;
        let now = wall();
        if now < at {
            std::thread::sleep(Duration::from_micros((at - now) as u64));
        }
        let s = link.session();
        println!("BEAT {k} {:.9} {:.6} {}", s.beat_at(at), s.tempo, s.peers);
        k += 1;
    }
}

fn spawn(bpm: &str) -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "child_peer", "--nocapture", "--test-threads=1"])
        .env("CANTUS_LINK_CHILD", bpm)
        .stdout(Stdio::piped())
        .spawn()
        .expect("the child")
}

fn beats(child: std::process::Child) -> Vec<(i64, f64, f64, usize)> {
    let out = child.wait_with_output().unwrap();
    BufReader::new(out.stdout.as_slice())
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| {
            let p: Vec<&str> = l.split_whitespace().collect();
            (p.first() == Some(&"BEAT")).then(|| {
                (
                    p[1].parse().unwrap(),
                    p[2].parse().unwrap(),
                    p[3].parse().unwrap(),
                    p[4].parse().unwrap(),
                )
            })
        })
        .collect()
}

#[test]
fn two_cantus_processes_converge() {
    if std::env::var("CANTUS_LINK_CHILD").is_ok() {
        return;
    }
    let a = spawn("120");
    std::thread::sleep(Duration::from_millis(200));
    let b = spawn("88");
    let (a, b) = (beats(a), beats(b));
    // The instants both printed once settled (the last three seconds).
    let mut worst = 0.0f64;
    let mut n = 0;
    for (k, ba, ta, pa) in &a {
        if let Some((_, bb, tb, pb)) = b.iter().find(|x| x.0 == *k) {
            // The same tempo, to the wire's whole microseconds per beat.
            if *pa == 1 && *pb == 1 && (ta - tb).abs() < 1e-3 {
                n += 1;
                if n > 10 {
                    worst = worst.max((ba - bb).abs() * 60_000.0 / ta);
                }
            }
        }
    }
    println!("two processes: {n} common instants, worst phase error {worst:.4} ms");
    assert!(n > 20, "the processes never shared a session ({n})");
    assert!(worst <= 1.0, "phase error {worst} ms");
}

/// The first scan finds nothing (no network yet); every later one the loopback.
fn late_loopback(_only_loopback: bool) -> Vec<std::net::Ipv4Addr> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SCANS: AtomicU32 = AtomicU32::new(0);
    if SCANS.fetch_add(1, Ordering::Relaxed) == 0 {
        Vec::new()
    } else {
        vec![std::net::Ipv4Addr::LOCALHOST]
    }
}

/// Review r33 #6: turned on with no interface to speak on, a handle stays on and takes part
/// from the scan that finds one (it failed to enable — or, with the loopback only, stayed
/// there for good). Loopback only, as every test.
#[test]
fn an_interface_found_by_a_later_scan_takes_part() {
    let mut late = Config::local(20_937);
    late.scan = late_loopback;
    late.rescan = Duration::from_millis(200);
    let a = Link::new(120.0, wall, late);
    let b = Link::new(120.0, wall, Config::local(20_937));
    a.enable(true).expect("on, though nothing to speak on yet");
    // Round 2, D6: the handle says it speaks on nothing (the top bar shows it).
    assert_eq!(a.interfaces_open(), 0);
    b.enable(true).expect("sockets");
    assert!(
        wait_until(Duration::from_secs(5), || a.session().peers == 1
            && b.session().peers == 1),
        "a {:?} b {:?}",
        a.session().peers,
        b.session().peers
    );
    assert_eq!(a.interfaces_open(), 1);
}
