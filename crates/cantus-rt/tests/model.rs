//! The pure types — topology model, placement, padding, prefault, the denormal guard's
//! bookkeeping — on synthetic machines. No OS calls: these run under Miri
//! (`cargo +nightly miri test -p cantus-rt --test model`).

use cantus_rt::memory::prefault;
use cantus_rt::pad::{padded_len, round_up, CachePadded, CACHE_PAD};
use cantus_rt::topology::parse::{cache_size, capacity_classes, cpu_list};
use cantus_rt::topology::{CoreClass, LogicalCpu, Source, Topology};

fn cpu(id: u32, core: u32, class: u8, llc: u32) -> LogicalCpu {
    LogicalCpu {
        id,
        group: 0,
        number: id,
        core,
        efficiency_class: class,
        llc,
        parked: false,
        reserved: false,
    }
}

/// An i7-12700H as Windows' CPU sets describe it: 6 P-cores with two threads (class 1), then
/// 8 E-cores with one (class 0), one L3. CPU set ids start at 256.
fn alder_lake_h() -> Vec<LogicalCpu> {
    let mut v = Vec::new();
    for c in 0..6 {
        v.push(cpu(256 + 2 * c, 2 * c, 1, 0));
        v.push(cpu(257 + 2 * c, 2 * c, 1, 0));
    }
    for e in 0..8 {
        v.push(cpu(268 + e, 12 + e, 0, 0));
    }
    v
}

/// A dual-CCD desktop part: 16 uniform cores with SMT, two L3 domains of 8.
fn dual_ccd() -> Vec<LogicalCpu> {
    (0..32).map(|i| cpu(i, i / 2, 0, (i / 16) * 0x10)).collect()
}

#[test]
fn a_hybrid_laptop_has_six_p_and_eight_e_cores() {
    let t = Topology::from_cpus(Source::CpuSets, &alder_lake_h());
    assert!(t.is_hybrid());
    assert_eq!(t.logical, 20);
    assert_eq!(t.cores.len(), 14);
    assert_eq!(t.performance_cores().count(), 6);
    assert_eq!(t.efficiency_cores().count(), 8);
    assert!(t.performance_cores().all(|c| c.cpus.len() == 2));
    assert_eq!(t.caches.len(), 1);
    // §2.1: six P-cores minus the callback's and the OS's.
    assert_eq!(t.default_live_workers(), 4);
}

#[test]
fn live_workers_skip_the_first_p_core_and_never_take_an_e_core() {
    let t = Topology::from_cpus(Source::CpuSets, &alder_lake_h());
    let p = t.placement(4);
    // One logical processor per core: the first thread of P-cores 1..=4.
    assert_eq!(p.live, vec![258, 260, 262, 264]);
    // Ahead (R3): the P-cores left (0 and 5), then the E-cores.
    assert_eq!(&p.ahead[..2], &[256, 266]);
    assert_eq!(p.ahead.len(), 2 + 8);
    // Background: E-cores first.
    assert_eq!(p.background[0], 268);
    assert_eq!(p.background.len(), 14);
}

#[test]
fn the_live_pool_stays_in_one_cache_domain() {
    let t = Topology::from_cpus(Source::Sysfs, &dual_ccd());
    assert!(!t.is_hybrid());
    assert_eq!(t.caches.len(), 2);
    let d = t.live_domain().unwrap();
    assert_eq!(d.cores.len(), 8);
    assert_eq!(t.default_live_workers(), 6);
    let p = t.placement(14);
    // Only seven fit after skipping the first core of the domain.
    assert_eq!(p.live.len(), 7);
    assert!(p.live.iter().all(|&id| id < 16));
}

#[test]
fn parked_and_reserved_cores_are_not_placed() {
    let mut cpus = alder_lake_h();
    for c in cpus.iter_mut().filter(|c| c.core == 2) {
        c.parked = true;
    }
    cpus.iter_mut().find(|c| c.core == 4).unwrap().reserved = true;
    let t = Topology::from_cpus(Source::CpuSets, &cpus);
    // Core 4 still has one usable thread; core 2 has none.
    assert_eq!(t.cores.iter().filter(|c| !c.available).count(), 1);
    assert_eq!(t.default_live_workers(), 3);
    // Core 2 (ids 258/259) is skipped; core 4 is placed on its unreserved thread, 261.
    assert_eq!(t.placement(8).live, vec![261, 262, 264, 266]);
}

#[test]
fn a_uniform_cpu_counts_every_core_as_performance() {
    let cpus: Vec<_> = (0..4).map(|i| cpu(i, i, 3, 0)).collect();
    let t = Topology::from_cpus(Source::Fallback, &cpus);
    assert!(t.cores.iter().all(|c| c.class == CoreClass::Performance));
    assert_eq!(t.default_live_workers(), 2);
    // Never fewer than one, never more than fourteen.
    let one = Topology::from_cpus(Source::Fallback, &[cpu(0, 0, 0, 0)]);
    assert_eq!(one.default_live_workers(), 1);
    let big: Vec<_> = (0..64).map(|i| cpu(i, i, 0, 0)).collect();
    assert_eq!(
        Topology::from_cpus(Source::Fallback, &big).default_live_workers(),
        14
    );
}

#[test]
fn cache_info_lands_on_its_domain() {
    let mut t = Topology::from_cpus(Source::Sysfs, &dual_ccd());
    t.set_cache_info(0x10, 3, 32 << 20);
    assert_eq!((t.caches[1].level, t.caches[1].bytes), (3, 32 << 20));
    assert_eq!(t.caches[0].level, 0);
}

#[test]
fn padding_is_128_bytes() {
    assert_eq!(CACHE_PAD, 128);
    let cells: Vec<CachePadded<u32>> = (0..4).map(CachePadded::new).collect();
    let a = &*cells[0] as *const u32 as usize;
    let b = &*cells[1] as *const u32 as usize;
    assert_eq!(b - a, 128);
    assert_eq!(a % 128, 0);
    let mut c = CachePadded::new([1u64; 3]);
    c[2] = 7;
    assert_eq!(c.into_inner(), [1, 1, 7]);
    assert_eq!(round_up(1), 128);
    assert_eq!(round_up(128), 128);
    assert_eq!(round_up(129), 256);
    // 100 f32s (400 bytes) take 512 bytes: 128 floats.
    assert_eq!(padded_len::<f32>(100), 128);
    assert_eq!(padded_len::<()>(5), 5);
}

#[test]
fn prefault_touches_every_page_and_changes_nothing() {
    let mut v: Vec<f32> = (0..10_000).map(|i| i as f32 * 0.5).collect();
    let before = v.clone();
    let writes = prefault(&mut v);
    assert_eq!(v, before);
    // 40 000 bytes: at least one write per page of this system (4 KiB on x86, 16 KiB on
    // Apple silicon).
    let page = cantus_rt::memory::page_size();
    assert!(
        writes >= 40_000 / page,
        "{writes} writes, {page}-byte pages"
    );
    assert_eq!(prefault::<f32>(&mut []), 0);
    assert_eq!(prefault(&mut [(); 3]), 0);
}

#[test]
fn guards_nest_and_reassert_is_inert_outside_one() {
    let before = cantus_rt::denormal::control_register();
    cantus_rt::fpenv::reassert();
    assert_eq!(cantus_rt::denormal::control_register(), before);
    {
        let g = cantus_rt::DenormalGuard::new();
        let _h = cantus_rt::DenormalGuard::new();
        g.reassert();
    }
    assert_eq!(cantus_rt::denormal::control_register(), before);
}

#[test]
fn driver_names_map_to_hosts() {
    use cantus_rt::AudioHost;
    assert_eq!(AudioHost::from_driver_name("ASIO"), AudioHost::Asio);
    assert_eq!(AudioHost::from_driver_name("WASAPI"), AudioHost::Wasapi);
    assert_eq!(
        AudioHost::from_driver_name("CoreAudio"),
        AudioHost::CoreAudio
    );
    assert_eq!(AudioHost::from_driver_name("jack"), AudioHost::Jack);
    assert_eq!(AudioHost::from_driver_name("ALSA"), AudioHost::Alsa);
    assert_eq!(AudioHost::from_driver_name("PipeWire"), AudioHost::PipeWire);
    assert!(AudioHost::PipeWire.driver_owns_priority());
    assert!(!AudioHost::Alsa.driver_owns_priority());
    assert_eq!(AudioHost::from_driver_name("PulseAudio"), AudioHost::Other);
    assert!(AudioHost::from_driver_name("ASIO").driver_owns_priority());
    assert!(!AudioHost::from_driver_name("WASAPI").driver_owns_priority());
}

#[test]
fn parse_cpu_lists() {
    assert_eq!(cpu_list("0-3,8,10-11\n"), vec![0, 1, 2, 3, 8, 10, 11]);
    assert_eq!(cpu_list("5"), vec![5]);
    assert_eq!(cpu_list(""), Vec::<u32>::new());
    assert_eq!(cpu_list("3,1-2,2"), vec![1, 2, 3]);
    assert_eq!(cpu_list("x,4-2,7"), vec![7]);
}

#[test]
fn parse_cache_sizes() {
    assert_eq!(cache_size("48K"), Some(48 * 1024));
    assert_eq!(cache_size("24576K\n"), Some(24 << 20));
    assert_eq!(cache_size("2M"), Some(2 << 20));
    assert_eq!(cache_size("1024"), Some(1024));
    assert_eq!(cache_size("K"), None);
    assert_eq!(cache_size(""), None);
}

#[test]
fn parse_capacities_rank_into_classes() {
    assert_eq!(
        capacity_classes(&[1024, 1024, 446, 446, 871]),
        vec![2, 2, 0, 0, 1]
    );
    assert_eq!(capacity_classes(&[1024, 1024]), vec![0, 0]);
}
