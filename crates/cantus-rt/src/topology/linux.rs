//! Linux: `/sys/devices/system/cpu`. Cores from `topology/core_cpus_list` (or the older
//! `thread_siblings_list`), classes from `cpu_capacity` (Arm big.LITTLE) or Intel's hybrid PMU
//! lists (`/sys/devices/cpu_atom/cpus` = E-cores), the last-level cache from the highest
//! `cache/index*` level's `shared_cpu_list`.

use super::parse::{cache_size, capacity_classes, cpu_list};
use super::{LogicalCpu, Source, Topology};
use std::fs::read_to_string;

const CPU: &str = "/sys/devices/system/cpu";

pub fn detect() -> Option<Topology> {
    detect_from(Source::Sysfs)
}

pub fn detect_from(source: Source) -> Option<Topology> {
    if source != Source::Sysfs {
        return None;
    }
    let online = cpu_list(&read_to_string(format!("{CPU}/online")).ok()?);
    if online.is_empty() {
        return None;
    }
    let read = |cpu: u32, file: &str| read_to_string(format!("{CPU}/cpu{cpu}/{file}")).ok();
    let atoms = read_to_string("/sys/devices/cpu_atom/cpus")
        .map(|t| cpu_list(&t))
        .unwrap_or_default();
    let capacities: Vec<u32> = online
        .iter()
        .map(|&c| {
            read(c, "cpu_capacity")
                .and_then(|t| t.trim().parse().ok())
                .unwrap_or(1024)
        })
        .collect();
    let classes = capacity_classes(&capacities);
    let mut llc_info: Vec<(u32, u8, u64)> = Vec::new();
    let mut cpus = Vec::with_capacity(online.len());
    for (i, &c) in online.iter().enumerate() {
        let siblings = read(c, "topology/core_cpus_list")
            .or_else(|| read(c, "topology/thread_siblings_list"))
            .map(|t| cpu_list(&t))
            .unwrap_or_default();
        let (llc, level, bytes) = last_level_cache(c).unwrap_or((0, 0, 0));
        if !llc_info.iter().any(|d| d.0 == llc) {
            llc_info.push((llc, level, bytes));
        }
        let efficiency_class = if !atoms.is_empty() {
            u8::from(!atoms.contains(&c))
        } else {
            classes[i]
        };
        cpus.push(LogicalCpu {
            id: c,
            group: 0,
            number: c,
            core: siblings.first().copied().unwrap_or(c),
            efficiency_class,
            llc,
            parked: false,
            reserved: false,
        });
    }
    let mut t = Topology::from_cpus(Source::Sysfs, &cpus);
    for (key, level, bytes) in llc_info {
        t.set_cache_info(key, level, bytes);
    }
    Some(t)
}

/// The highest-level data or unified cache of `cpu`: (domain key = its first sharer, level,
/// bytes).
fn last_level_cache(cpu: u32) -> Option<(u32, u8, u64)> {
    let mut best: Option<(u32, u8, u64)> = None;
    for index in 0..16 {
        let dir = format!("{CPU}/cpu{cpu}/cache/index{index}");
        let Ok(level) = read_to_string(format!("{dir}/level")) else {
            break;
        };
        let ty = read_to_string(format!("{dir}/type")).unwrap_or_default();
        if ty.trim() == "Instruction" {
            continue;
        }
        let Ok(level) = level.trim().parse::<u8>() else {
            continue;
        };
        if best.is_some_and(|b| b.1 >= level) {
            continue;
        }
        let sharers = read_to_string(format!("{dir}/shared_cpu_list"))
            .map(|t| cpu_list(&t))
            .unwrap_or_default();
        let bytes = read_to_string(format!("{dir}/size"))
            .ok()
            .and_then(|t| cache_size(&t))
            .unwrap_or(0);
        best = Some((sharers.first().copied().unwrap_or(cpu), level, bytes));
    }
    best
}
