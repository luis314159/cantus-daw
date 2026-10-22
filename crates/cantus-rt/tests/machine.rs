//! Tests that ask this machine's OS. They hold on any machine; on a hybrid CPU (the Windows 11
//! laptop of round 21: an i7-12700H) they also check that both sources find the P- and E-cores.

#![cfg(not(miri))]

use cantus_rt::memory::{lock, prefault};
use cantus_rt::thread::Grant;
#[cfg(windows)]
use cantus_rt::thread::MmcssPriority;
use cantus_rt::thread::{
    callback_promotion, current_priority, opt_out_of_power_throttling, promote_callback,
    promote_worker, realtime_share, set_current_thread_cpus, AudioHost, PowerThrottling, Promotion,
    Waiting, WorkerRole,
};
#[cfg(any(windows, target_os = "linux"))]
use cantus_rt::topology::Source;
use cantus_rt::topology::{CoreClass, Topology};
use std::time::Duration;

const PERIOD: Duration = Duration::from_micros(2_667);

/// Intel's CPUID hybrid flag (leaf 7, EDX bit 15): the hardware's own word, independent of the
/// OS tables the topology reads.
#[cfg(target_arch = "x86_64")]
fn cpuid_says_hybrid() -> bool {
    #[allow(unused_unsafe)]
    // SAFETY: CPUID exists on every x86-64 CPU; leaf 7 subleaf 0 is defined when the max leaf
    // (leaf 0's EAX) is at least 7, checked first.
    unsafe {
        use std::arch::x86_64::__cpuid_count;
        __cpuid_count(0, 0).eax >= 7 && (__cpuid_count(7, 0).edx >> 15) & 1 == 1
    }
}

#[cfg(not(target_arch = "x86_64"))]
fn cpuid_says_hybrid() -> bool {
    false
}

#[test]
fn the_topology_is_whole() {
    let t = Topology::detect();
    println!(
        "{:?}: {} logical, {} cores, {} P, {} E, {} cache domains",
        t.source,
        t.logical,
        t.cores.len(),
        t.performance_cores().count(),
        t.efficiency_cores().count(),
        t.caches.len()
    );
    assert!(t.logical >= 1 && !t.cores.is_empty());
    assert_eq!(
        t.cores.iter().map(|c| c.cpus.len()).sum::<usize>(),
        t.logical
    );
    assert!(t.performance_cores().count() >= 1);
    // Every core in exactly one cache domain.
    let mut seen: Vec<usize> = t
        .caches
        .iter()
        .flat_map(|d| d.cores.iter().copied())
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, (0..t.cores.len()).collect::<Vec<_>>());
    // As many logical processors as std sees (one processor group: under 64).
    if t.logical < 64 {
        let std_n = std::thread::available_parallelism().unwrap().get();
        assert_eq!(t.logical, std_n);
    }
    #[cfg(windows)]
    assert_eq!(t.source, Source::CpuSets);
    #[cfg(target_os = "linux")]
    assert_eq!(t.source, Source::Sysfs);
    #[cfg(windows)]
    assert!(
        t.caches.iter().all(|d| d.level >= 2 && d.bytes > 0),
        "{:?}",
        t.caches
    );
}

#[test]
fn a_hybrid_cpu_shows_both_classes() {
    let t = Topology::detect();
    if cpuid_says_hybrid() {
        assert!(
            t.is_hybrid(),
            "CPUID says hybrid; the topology found one class: {t:?}"
        );
        assert!(t.efficiency_cores().count() >= 1);
        // On Intel's hybrid parts the P-cores are the hyper-threaded ones, when any are.
        let ht = |c: &&cantus_rt::topology::Core| c.cpus.len() > 1;
        assert!(
            t.efficiency_cores().filter(ht).count() <= t.performance_cores().filter(ht).count()
        );
        // The live pool never lands on an E-core.
        let place = t.placement(t.default_live_workers());
        for id in &place.live {
            let core = t.cores.iter().find(|c| c.cpus.contains(id)).unwrap();
            assert_eq!(core.class, CoreClass::Performance);
        }
    } else {
        println!(
            "CPUID does not say hybrid: {} classes found",
            if t.is_hybrid() { 2 } else { 1 }
        );
    }
}

/// Windows has two independent tables; they must agree on every core and its class.
#[cfg(windows)]
#[test]
fn cpu_sets_and_processor_information_agree() {
    let a = Topology::detect_from(Source::CpuSets).expect("CPU sets");
    let b = Topology::detect_from(Source::ProcessorInformation)
        .expect("GetLogicalProcessorInformationEx");
    assert_eq!(a.logical, b.logical);
    assert_eq!(a.cores.len(), b.cores.len());
    assert_eq!(a.caches.len(), b.caches.len());
    assert_eq!(a.performance_cores().count(), b.performance_cores().count());
    assert_eq!(a.efficiency_cores().count(), b.efficiency_cores().count());
    let shape = |t: &Topology| {
        let mut v: Vec<(u8, usize)> = t
            .cores
            .iter()
            .map(|c| (c.efficiency_class, c.cpus.len()))
            .collect();
        v.sort_unstable();
        v
    };
    assert_eq!(shape(&a), shape(&b));
    let size = |t: &Topology| {
        let mut v: Vec<(u8, u64, usize)> = t
            .caches
            .iter()
            .map(|d| (d.level, d.bytes, d.cores.len()))
            .collect();
        v.sort_unstable();
        v
    };
    assert_eq!(size(&a), size(&b));
}

#[test]
fn the_callback_is_promoted_once_or_says_why() {
    std::thread::spawn(|| {
        let before = current_priority();
        let p = promote_callback(AudioHost::Wasapi, PERIOD);
        println!(
            "callback: {p} [priority {before:?} -> {:?}]",
            current_priority()
        );
        match p {
            Promotion::Granted(_) => assert!(p.is_realtime()),
            Promotion::Refused(r) => assert!(!r.step.is_empty() && !p.to_string().is_empty()),
            Promotion::LeftAsIs(why) => assert!(!why.is_empty()),
        }
        #[cfg(windows)]
        assert_eq!(
            p,
            Promotion::Granted(Grant::Mmcss {
                task: "Pro Audio",
                priority: MmcssPriority::High,
                task_index: match p {
                    Promotion::Granted(Grant::Mmcss { task_index, .. }) => task_index,
                    _ => 0,
                },
            }),
            "MMCSS refused on Windows: {p}"
        );
        // The second call is the first answer, from the thread-local.
        assert_eq!(promote_callback(AudioHost::Wasapi, PERIOD), p);
        assert_eq!(callback_promotion(), Some(p));
    })
    .join()
    .unwrap();
    // A fresh thread has no answer yet.
    std::thread::spawn(|| assert_eq!(callback_promotion(), None))
        .join()
        .unwrap();
}

#[test]
fn a_driver_owned_callback_is_left_alone() {
    std::thread::spawn(|| {
        let before = current_priority();
        for host in [AudioHost::Asio, AudioHost::CoreAudio, AudioHost::Jack] {
            assert!(host.driver_owns_priority());
        }
        let p = promote_callback(AudioHost::Asio, PERIOD);
        assert!(matches!(p, Promotion::LeftAsIs(_)), "{p}");
        assert!(p.is_realtime());
        assert_eq!(current_priority(), before);
    })
    .join()
    .unwrap();
}

/// Linux (ADR 0065): a JACK or PipeWire callback thread their library left at normal
/// priority — this test's thread is one — is promoted after all, or says why not; it is
/// never reported as real-time while it is not.
#[cfg(target_os = "linux")]
#[test]
fn a_linux_driver_thread_left_at_normal_priority_is_promoted() {
    for host in [AudioHost::Jack, AudioHost::PipeWire] {
        std::thread::spawn(move || {
            let p = promote_callback(host, PERIOD);
            println!("{host:?} callback: {p}");
            assert!(!matches!(p, Promotion::LeftAsIs(_)), "{p}");
            match p {
                Promotion::Granted(_) => assert!(p.is_realtime()),
                Promotion::Refused(r) => assert!(!r.step.is_empty() && !p.is_realtime()),
                Promotion::LeftAsIs(_) => unreachable!(),
            }
        })
        .join()
        .unwrap();
    }
}

/// The rule round 21 measured (docs/round21/rt.md §5): a spinning worker never enters a
/// throttled real-time class; a bounded one does. Both revert on drop.
#[test]
fn workers_are_promoted_safely_for_how_they_wait_and_reverted_on_drop() {
    let bounded = Waiting::Bounded { busy_share: 0.5 };
    println!("real-time share {}", realtime_share());
    assert!(bounded.realtime_safe());
    assert!(!Waiting::Spins.realtime_safe());
    assert!(!Waiting::Bounded { busy_share: 0.9 }.realtime_safe());
    #[cfg(windows)]
    assert!(
        (0.0..=0.9).contains(&realtime_share()),
        "SystemResponsiveness gives {}",
        realtime_share()
    );
    for role in [WorkerRole::Live, WorkerRole::Ahead, WorkerRole::Background] {
        for waiting in [
            Waiting::Spins,
            bounded,
            Waiting::Bounded { busy_share: 0.95 },
        ] {
            std::thread::spawn(move || {
                let before = current_priority();
                let w = promote_worker(role, PERIOD, waiting);
                println!(
                    "{role:?} {waiting:?}: {} [{before:?} -> {:?}]",
                    w.promotion,
                    current_priority()
                );
                #[cfg(windows)]
                match (role, waiting.realtime_safe()) {
                    (_, true) => assert!(
                        matches!(w.promotion, Promotion::Granted(Grant::Mmcss { .. })),
                        "{}",
                        w.promotion
                    ),
                    (WorkerRole::Background, false) => {
                        assert!(matches!(w.promotion, Promotion::LeftAsIs(_)))
                    }
                    (_, false) => assert!(
                        matches!(
                            w.promotion,
                            Promotion::Granted(Grant::ThreadPriority { .. })
                        ),
                        "a spinning thread must not be in MMCSS: {}",
                        w.promotion
                    ),
                }
                #[cfg(not(windows))]
                if !waiting.realtime_safe() && role != WorkerRole::Background {
                    assert!(!matches!(
                        w.promotion,
                        Promotion::Granted(Grant::TimeConstraint { .. })
                    ));
                }
                drop(w);
                assert_eq!(current_priority(), before);
            })
            .join()
            .unwrap();
        }
    }
}

#[test]
fn a_thread_can_be_placed_on_a_live_core() {
    let t = Topology::detect();
    let place = t.placement(t.default_live_workers());
    std::thread::spawn(move || {
        let Some(&id) = place.live.first().or(place.background.first()) else {
            return;
        };
        let r = set_current_thread_cpus(&[id]);
        #[cfg(any(windows, target_os = "linux"))]
        assert!(r.is_ok(), "{:?}", r);
        println!("placed on {id}: {r:?}");
        let _ = set_current_thread_cpus(&[]);
    })
    .join()
    .unwrap();
}

#[test]
fn the_process_leaves_ecoqos() {
    let p = opt_out_of_power_throttling();
    println!("{p}");
    #[cfg(windows)]
    assert!(matches!(p, PowerThrottling::OptedOut { .. }), "{p}");
    #[cfg(not(windows))]
    assert_eq!(p, PowerThrottling::NotApplicable);
}

#[test]
fn a_prefaulted_slab_is_locked_or_says_why() {
    let mut slab = vec![0f32; 1 << 18]; // 1 MiB
    prefault(&mut slab);
    let l = lock(&slab);
    println!("{l}");
    #[cfg(windows)]
    assert!(l.is_locked(), "{l}");
    if let cantus_rt::memory::Lock::Refused(r) = &l {
        assert!(!r.step.is_empty());
    }
    drop(l);
    slab[12345] = 1.0;
    assert_eq!(slab[12345], 1.0);
}

/// The device page's report (ADR 0065): the callback's answer, and each live worker's —
/// gone when its promotion is dropped.
#[test]
fn the_priority_report_follows_the_threads() {
    use cantus_rt::thread::{
        priority_report, promote_worker, worker_promotion_of, Waiting, WorkerRole,
    };
    std::thread::spawn(|| {
        let p = promote_callback(AudioHost::Alsa, PERIOD);
        // Another test's callback may have come since: the report holds one of them.
        assert!(priority_report().callback.is_some(), "{p}");
    })
    .join()
    .unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
    let worker = std::thread::spawn(move || {
        let w = promote_worker(
            WorkerRole::Live,
            PERIOD,
            Waiting::Bounded { busy_share: 0.5 },
        );
        tx.send((std::thread::current().id(), w.promotion)).unwrap();
        done_rx.recv().unwrap();
        drop(w);
    });
    let (id, promotion) = rx.recv().unwrap();
    assert_eq!(worker_promotion_of(id), Some(promotion));
    assert!(priority_report().workers.contains(&promotion));
    done_tx.send(()).unwrap();
    worker.join().unwrap();
    assert_eq!(
        worker_promotion_of(id),
        None,
        "a worker gone is off the report"
    );
}
