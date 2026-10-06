//! macOS: `sysctl hw.perflevel*` (Apple silicon: perflevel0 = performance cores, perflevel1 =
//! efficiency; each cluster shares an L2, the last level a core sees), or `hw.physicalcpu` on
//! an Intel Mac. macOS has no thread affinity: the ids are indices, and Apple's workgroups do
//! the placing.

use super::{LogicalCpu, Source, Topology};

fn sysctl_u64(name: &str) -> Option<u64> {
    let c = std::ffi::CString::new(name).ok()?;
    let mut v: u64 = 0;
    let mut len = std::mem::size_of::<u64>();
    // SAFETY: `c` is NUL-terminated; `v` and `len` are live out-pointers, `len` holding the
    // buffer's size (the integer sysctls written into it are 4 or 8 bytes, little-endian).
    let r = unsafe {
        libc::sysctlbyname(
            c.as_ptr(),
            (&mut v as *mut u64).cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    match (r, len) {
        (0, 4) => Some(v & 0xFFFF_FFFF),
        (0, 8) => Some(v),
        _ => None,
    }
}

pub fn detect() -> Option<Topology> {
    detect_from(Source::Sysctl)
}

pub fn detect_from(source: Source) -> Option<Topology> {
    if source != Source::Sysctl {
        return None;
    }
    let levels = sysctl_u64("hw.nperflevels").unwrap_or(1).max(1);
    let mut cpus = Vec::new();
    let mut caches = Vec::new();
    let mut core = 0u32;
    for level in 0..levels {
        let key = |k: &str| {
            if levels > 1 || sysctl_u64("hw.perflevel0.physicalcpu").is_some() {
                sysctl_u64(&format!("hw.perflevel{level}.{k}"))
            } else {
                sysctl_u64(&format!("hw.{k}"))
            }
        };
        let physical = key("physicalcpu")? as u32;
        let logical = key("logicalcpu")
            .unwrap_or(physical as u64)
            .max(physical as u64) as u32;
        let per_core = (logical / physical.max(1)).max(1);
        // Cores per shared last-level cache: L3 if there is one (Intel), else the cluster's L2.
        let (llc_level, llc_bytes, sharers) = match key("l3cachesize") {
            Some(b) if b > 0 => (3, b, key("cpusperl3").unwrap_or(logical as u64)),
            _ => (
                2,
                key("l2cachesize").unwrap_or(0),
                key("cpusperl2").unwrap_or(logical as u64),
            ),
        };
        let cores_per_llc = (sharers as u32 / per_core).max(1);
        let class = (levels - 1 - level) as u8;
        for c in 0..physical {
            let llc = (level as u32) << 16 | c / cores_per_llc;
            if !caches.iter().any(|d: &(u32, u8, u64)| d.0 == llc) {
                caches.push((llc, llc_level, llc_bytes));
            }
            for _ in 0..per_core {
                let id = cpus.len() as u32;
                cpus.push(LogicalCpu {
                    id,
                    group: 0,
                    number: id,
                    core,
                    efficiency_class: class,
                    llc,
                    parked: false,
                    reserved: false,
                });
            }
            core += 1;
        }
    }
    let mut t = Topology::from_cpus(Source::Sysctl, &cpus);
    for (k, level, bytes) in caches {
        t.set_cache_info(k, level, bytes);
    }
    Some(t)
}
