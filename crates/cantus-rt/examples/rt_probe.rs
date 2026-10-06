//! What this machine gives the engine: the CPU topology and which real-time promotions the
//! OS grants. Run by hand on each platform (ADR 0031 plan, Phase 0):
//!
//! ```text
//! cargo run -p cantus-rt --example rt_probe --release
//! ```

use cantus_rt::memory::{lock, page_size, prefault, prefault_stack};
use cantus_rt::thread::{
    current_priority, promote_callback, promote_worker, set_current_thread_cpus, AudioHost,
    WorkerRole,
};
use cantus_rt::topology::{CoreClass, Source, Topology};
use std::time::{Duration, Instant};

const PERIOD: Duration = Duration::from_micros(2_667); // 128 frames at 48 kHz

fn main() {
    println!(
        "== cantus-rt probe ({} {})",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    let t = Topology::detect();
    print_topology("topology", &t);
    if t.source == Source::CpuSets {
        if let Some(g) = Topology::detect_from(Source::ProcessorInformation) {
            print_topology("cross-check (GetLogicalProcessorInformationEx)", &g);
        }
    }
    let live = t.default_live_workers();
    let place = t.placement(live);
    println!("default live workers: {live}");
    println!(
        "placement: live {:?}\n           ahead {:?}\n           background {:?}",
        place.live, place.ahead, place.background
    );

    println!("\n== process");
    let fma_before = cantus_rt::fpenv::crt_fma3_enabled();
    let init = cantus_rt::init_process();
    println!(
        "CRT FMA3 path before: {fma_before:?}; after: {:?}",
        cantus_rt::fpenv::crt_fma3_enabled()
    );
    println!("init_process: {init}");

    println!("\n== threads");
    for host in [AudioHost::Wasapi, AudioHost::Asio, AudioHost::Alsa] {
        let (before, p, after) = std::thread::spawn(move || {
            let before = current_priority();
            let p = promote_callback(host, PERIOD);
            (before, p, current_priority())
        })
        .join()
        .unwrap();
        println!("callback on {host:?}: {p}  [priority {before:?} -> {after:?}]");
    }
    for role in [WorkerRole::Live, WorkerRole::Ahead, WorkerRole::Background] {
        let cpus = match role {
            WorkerRole::Live => place.live.first(),
            WorkerRole::Ahead => place.ahead.first(),
            WorkerRole::Background => place.background.first(),
        }
        .copied();
        let line = std::thread::spawn(move || {
            let before = current_priority();
            let w = promote_worker(role, PERIOD);
            let after = current_priority();
            let pin = cpus.map(|c| (c, set_current_thread_cpus(&[c])));
            let text = format!(
                "worker {role:?}: {}  [priority {before:?} -> {after:?}]",
                w.promotion
            );
            drop(w);
            let reverted = current_priority();
            match pin {
                Some((c, Ok(()))) => format!("{text}; placed on cpu {c}; reverted to {reverted:?}"),
                Some((c, Err(e))) => {
                    format!("{text}; placing on cpu {c}: {e}; reverted to {reverted:?}")
                }
                None => format!("{text}; reverted to {reverted:?}"),
            }
        })
        .join()
        .unwrap();
        println!("{line}");
    }

    println!("\n== memory (page {} bytes)", page_size());
    let n = 16 << 20;
    let t0 = Instant::now();
    let mut cold = vec![0u8; n];
    let alloc = t0.elapsed();
    let t1 = Instant::now();
    let touched = prefault(&mut cold);
    let first = t1.elapsed();
    let t2 = Instant::now();
    prefault(&mut cold);
    let again = t2.elapsed();
    println!(
        "16 MiB: allocate {:.2} ms, first touch {:.2} ms ({touched} pages), touched again {:.2} ms",
        ms(alloc),
        ms(first),
        ms(again)
    );
    prefault_stack();
    println!("lock: {}", lock(&cold));

    println!("\n== floating point");
    let before = cantus_rt::denormal::control_register();
    {
        let _g = cantus_rt::DenormalGuard::new();
        println!(
            "control register {before:#x} -> {:#x} in a guard (flushing: {})",
            cantus_rt::denormal::control_register(),
            cantus_rt::denormal::flushing()
        );
    }
    println!(
        "after the guard {:#x}",
        cantus_rt::denormal::control_register()
    );
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

fn print_topology(title: &str, t: &Topology) {
    let p = t.performance_cores().count();
    let e = t.efficiency_cores().count();
    println!(
        "{title}: {:?}, {} logical, {} physical cores ({p} P + {e} E){}",
        t.source,
        t.logical,
        t.cores.len(),
        if t.is_hybrid() {
            ", hybrid"
        } else {
            ", uniform"
        }
    );
    for c in &t.cores {
        let class = if c.class == CoreClass::Performance {
            "P"
        } else {
            "E"
        };
        println!(
            "  core {:#07x} {class} (class {}) llc {:#x} cpus {:?}{}",
            c.key,
            c.efficiency_class,
            c.llc,
            c.cpus,
            if c.available {
                ""
            } else {
                " [parked/reserved]"
            }
        );
    }
    for d in &t.caches {
        println!(
            "  last-level cache {:#x}: L{} {} KiB, {} cores",
            d.key,
            d.level,
            d.bytes / 1024,
            d.cores.len()
        );
    }
}
