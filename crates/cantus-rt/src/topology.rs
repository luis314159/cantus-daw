//! The CPU as the scheduler needs to see it: physical cores, their performance class, and
//! the last-level-cache domains they share (ADR 0031 §2.1, §2.6, R3, R4).
//!
//! - **Physical cores, not hyper-threads**: DSP is floating-point bound and a sibling shares
//!   the core's FP units; one logical processor per core is used.
//! - **P and E classes**: Windows' CPU sets say `EfficiencyClass` (higher = faster), Linux's
//!   sysfs `cpu_capacity` or `/sys/devices/cpu_atom`, macOS `hw.perflevel*`. A CPU whose
//!   cores all share one class is *uniform*: every core counts as a performance core.
//! - **Last-level-cache domains**: the live pool stays inside one (DAWbench 2025: a
//!   single-CCD part beat dual-CCD parts at 64 samples). A 12th-gen laptop has one L3; a
//!   Ryzen 9 has one per CCD.
//!
//! [`Topology::detect`] never fails: without an OS answer it falls back to
//! `available_parallelism` as that many uniform cores, and says so in [`Topology::source`].

#[cfg(target_os = "linux")]
#[path = "topology/linux.rs"]
mod os;
#[cfg(target_os = "macos")]
#[path = "topology/macos.rs"]
mod os;
#[cfg(all(windows, not(miri)))]
#[path = "topology/windows.rs"]
mod os;

pub mod parse;

/// One logical processor, as the OS reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogicalCpu {
    /// What the placement call takes: the CPU set id on Windows, the CPU number on Linux;
    /// an index with no OS meaning on macOS (which has no affinity).
    pub id: u32,
    /// The processor group (Windows; 0 elsewhere) and the number inside it.
    pub group: u16,
    pub number: u32,
    /// Which physical core: equal for SMT siblings, unique across the machine.
    pub core: u32,
    /// Higher = faster; equal on every core of a uniform CPU.
    pub efficiency_class: u8,
    /// Which last-level-cache domain.
    pub llc: u32,
    /// Parked by the power manager right now, or reserved for another process.
    pub parked: bool,
    pub reserved: bool,
}

/// Performance or efficiency core.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CoreClass {
    Performance,
    Efficiency,
}

/// A physical core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Core {
    pub key: u32,
    pub class: CoreClass,
    pub efficiency_class: u8,
    pub llc: u32,
    /// Its logical processors' ids, in OS order; the first is the one a pool thread uses.
    pub cpus: Vec<u32>,
    /// Usable now: not every logical processor parked or reserved.
    pub available: bool,
}

/// A last-level-cache domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CacheDomain {
    pub key: u32,
    /// Cache level (3 for an L3) and size in bytes; 0 when the OS did not say.
    pub level: u8,
    pub bytes: u64,
    /// Indices into [`Topology::cores`].
    pub cores: Vec<usize>,
}

/// Where the topology came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Windows `GetSystemCpuSetInformation` (sizes from `GetLogicalProcessorInformationEx`).
    CpuSets,
    /// Windows `GetLogicalProcessorInformationEx` alone.
    ProcessorInformation,
    /// Linux `/sys/devices/system/cpu`.
    Sysfs,
    /// macOS `sysctl hw.perflevel*`.
    Sysctl,
    /// `std::thread::available_parallelism`, as uniform cores.
    Fallback,
}

/// The machine's cores and caches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Topology {
    pub source: Source,
    pub cores: Vec<Core>,
    pub caches: Vec<CacheDomain>,
    pub logical: usize,
}

/// Which cores each kind of pool thread goes on (ADR 0031 §2.1 with R3–R5), as one logical
/// processor id per physical core, in the order threads should take them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Placement {
    /// Live workers: performance cores inside one cache domain, skipping the first (the one
    /// the OS steers interrupts and the UI to); never E-cores, where Thread Director would
    /// move a spinner anyway (R5).
    pub live: Vec<u32>,
    /// Ahead workers while playing (R3): the performance cores the live pool leaves, then
    /// efficiency cores.
    pub ahead: Vec<u32>,
    /// Background freeze and export: efficiency cores first, then performance cores.
    pub background: Vec<u32>,
}

impl Topology {
    /// The machine's topology, from the best source this OS offers.
    pub fn detect() -> Topology {
        #[cfg(any(target_os = "linux", target_os = "macos", all(windows, not(miri))))]
        if let Some(t) = os::detect() {
            return t;
        }
        Topology::fallback()
    }

    /// The topology from one named source, `None` if it is not available here. For the
    /// tests that check two sources agree.
    pub fn detect_from(source: Source) -> Option<Topology> {
        match source {
            Source::Fallback => Some(Topology::fallback()),
            #[cfg(any(target_os = "linux", target_os = "macos", all(windows, not(miri))))]
            s => os::detect_from(s),
            #[cfg(not(any(target_os = "linux", target_os = "macos", all(windows, not(miri)))))]
            _ => None,
        }
    }

    /// `available_parallelism` uniform cores, one logical processor each.
    pub fn fallback() -> Topology {
        let n = std::thread::available_parallelism().map_or(1, |n| n.get()) as u32;
        let cpus: Vec<LogicalCpu> = (0..n)
            .map(|i| LogicalCpu {
                id: i,
                group: 0,
                number: i,
                core: i,
                efficiency_class: 0,
                llc: 0,
                parked: false,
                reserved: false,
            })
            .collect();
        Topology::from_cpus(Source::Fallback, &cpus)
    }

    /// Builds cores, classes and cache domains from a list of logical processors. Pure: the
    /// OS readers and the tests all come through here.
    pub fn from_cpus(source: Source, cpus: &[LogicalCpu]) -> Topology {
        let top = cpus.iter().map(|c| c.efficiency_class).max().unwrap_or(0);
        let mut cores: Vec<Core> = Vec::new();
        for c in cpus {
            let usable = !c.parked && !c.reserved;
            match cores.iter_mut().find(|k| k.key == c.core) {
                Some(k) => {
                    k.cpus.push(c.id);
                    k.available |= usable;
                }
                None => cores.push(Core {
                    key: c.core,
                    class: if c.efficiency_class == top {
                        CoreClass::Performance
                    } else {
                        CoreClass::Efficiency
                    },
                    efficiency_class: c.efficiency_class,
                    llc: c.llc,
                    cpus: vec![c.id],
                    available: usable,
                }),
            }
        }
        cores.sort_by_key(|k| k.key);
        // A core's usable logical processors first (stable: OS order otherwise), so `cpus[0]`
        // is always the one to place a thread on.
        let usable = |id: &u32| cpus.iter().any(|c| c.id == *id && !c.parked && !c.reserved);
        for k in &mut cores {
            k.cpus.sort_by_key(|id| !usable(id));
        }
        let mut caches: Vec<CacheDomain> = Vec::new();
        for (i, k) in cores.iter().enumerate() {
            match caches.iter_mut().find(|d| d.key == k.llc) {
                Some(d) => d.cores.push(i),
                None => caches.push(CacheDomain {
                    key: k.llc,
                    level: 0,
                    bytes: 0,
                    cores: vec![i],
                }),
            }
        }
        caches.sort_by_key(|d| d.key);
        Topology {
            source,
            cores,
            caches,
            logical: cpus.len(),
        }
    }

    /// Records a cache domain's level and size, where the OS says them separately.
    pub fn set_cache_info(&mut self, key: u32, level: u8, bytes: u64) {
        if let Some(d) = self.caches.iter_mut().find(|d| d.key == key) {
            d.level = level;
            d.bytes = bytes;
        }
    }

    /// True when the CPU has more than one core class.
    pub fn is_hybrid(&self) -> bool {
        self.cores.iter().any(|c| c.class == CoreClass::Efficiency)
    }

    pub fn performance_cores(&self) -> impl Iterator<Item = &Core> {
        self.cores.iter().filter(|c| c.class == CoreClass::Performance)
    }

    pub fn efficiency_cores(&self) -> impl Iterator<Item = &Core> {
        self.cores.iter().filter(|c| c.class == CoreClass::Efficiency)
    }

    /// The cache domain the live pool uses: the one with the most available performance
    /// cores (the first such on a tie).
    pub fn live_domain(&self) -> Option<&CacheDomain> {
        let p = |d: &&CacheDomain| {
            d.cores
                .iter()
                .filter(|&&i| self.cores[i].class == CoreClass::Performance && self.cores[i].available)
                .count()
        };
        let best = self.caches.iter().map(|d| p(&d)).max()?;
        self.caches.iter().find(|d| p(d) == best)
    }

    /// ADR 0031 §2.1's default number of live workers: the live domain's performance cores
    /// minus two (one for the callback, one left to the OS, the UI and the driver), at least
    /// 1, at most 14. On macOS the device workgroup's `max_parallel_threads − 1` replaces it
    /// (R4).
    pub fn default_live_workers(&self) -> usize {
        let p = self.live_domain().map_or(0, |d| {
            d.cores
                .iter()
                .filter(|&&i| self.cores[i].class == CoreClass::Performance && self.cores[i].available)
                .count()
        });
        p.saturating_sub(2).clamp(1, 14)
    }

    /// Where `live` live workers go, and the ahead and background orders around them.
    pub fn placement(&self, live: usize) -> Placement {
        let first = |c: &Core| c.cpus[0];
        let usable = |c: &&Core| c.available;
        let domain: Vec<usize> = self.live_domain().map(|d| d.cores.clone()).unwrap_or_default();
        let live_cores: Vec<&Core> = domain
            .iter()
            .map(|&i| &self.cores[i])
            .filter(|c| c.class == CoreClass::Performance && c.available)
            .skip(1)
            .take(live)
            .collect();
        let taken = |c: &&Core| live_cores.iter().any(|l| l.key == c.key);
        let p: Vec<&Core> = self.performance_cores().filter(usable).collect();
        let e: Vec<&Core> = self.efficiency_cores().filter(usable).collect();
        let spare_p = p.iter().filter(|c| !taken(c));
        Placement {
            live: live_cores.iter().map(|c| first(c)).collect(),
            ahead: spare_p.chain(e.iter()).map(|c| first(c)).collect(),
            background: e.iter().chain(p.iter()).map(|c| first(c)).collect(),
        }
    }
}
