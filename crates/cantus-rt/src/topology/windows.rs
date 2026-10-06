//! Windows: CPU sets (`GetSystemCpuSetInformation`) for cores, classes and cache domains —
//! the ids `SetThreadSelectedCpuSets` takes — with `GetLogicalProcessorInformationEx` for the
//! last-level cache's level and size, and as a second, independent source.
//!
//! The records are variable-length C structs; they are read here as bytes at the offsets of
//! `windows-sys`'s own definitions (`offset_of!`), so the only `unsafe` is the two calls.

use super::{LogicalCpu, Source, Topology};
use std::mem::{offset_of, size_of};
use windows_sys::Win32::System::SystemInformation::{
    CacheData, CacheUnified, GetLogicalProcessorInformationEx, GetSystemCpuSetInformation,
    RelationCache, RelationProcessorCore, CACHE_RELATIONSHIP, GROUP_AFFINITY,
    LOGICAL_PROCESSOR_RELATIONSHIP, PROCESSOR_RELATIONSHIP, SYSTEM_CPU_SET_INFORMATION,
    SYSTEM_CPU_SET_INFORMATION_0_0, SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX,
};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

pub fn detect() -> Option<Topology> {
    detect_from(Source::CpuSets).or_else(|| detect_from(Source::ProcessorInformation))
}

pub fn detect_from(source: Source) -> Option<Topology> {
    match source {
        Source::CpuSets => {
            let cpus = cpu_sets()?;
            let mut t = Topology::from_cpus(Source::CpuSets, &cpus);
            for (level, bytes, members) in last_level_caches()? {
                // The domain a cache belongs to is the CPU sets' domain of any of its members.
                if let Some(c) = cpus.iter().find(|c| members.contains(&(c.group, c.number))) {
                    t.set_cache_info(c.llc, level, bytes);
                }
            }
            Some(t)
        }
        Source::ProcessorInformation => glpi_topology(),
        _ => None,
    }
}

/// Little-endian fields of a record, bounds-checked.
struct Rd<'a>(&'a [u8]);

impl Rd<'_> {
    fn bytes<const N: usize>(&self, at: usize) -> Option<[u8; N]> {
        self.0.get(at..at + N)?.try_into().ok()
    }
    fn u8(&self, at: usize) -> Option<u8> {
        self.0.get(at).copied()
    }
    fn u16(&self, at: usize) -> Option<u16> {
        self.bytes(at).map(u16::from_le_bytes)
    }
    fn u32(&self, at: usize) -> Option<u32> {
        self.bytes(at).map(u32::from_le_bytes)
    }
    fn u64(&self, at: usize) -> Option<u64> {
        self.bytes(at).map(u64::from_le_bytes)
    }
}

/// Splits a buffer of variable-length records whose `u32` size is at `size_at`.
fn records(buf: &[u8], size_at: usize) -> impl Iterator<Item = &[u8]> {
    let mut at = 0usize;
    std::iter::from_fn(move || {
        let size = Rd(buf.get(at..)?).u32(size_at)? as usize;
        if size == 0 || at + size > buf.len() {
            return None;
        }
        let rec = &buf[at..at + size];
        at += size;
        Some(rec)
    })
}

fn cpu_sets() -> Option<Vec<LogicalCpu>> {
    // SAFETY: a pseudo-handle for this process; nothing to close.
    let process = unsafe { GetCurrentProcess() };
    let mut len = 0u32;
    // SAFETY: a null buffer of length 0 asks only for the size, written to the live `len`.
    unsafe { GetSystemCpuSetInformation(std::ptr::null_mut(), 0, &mut len, process, 0) };
    // `u64`s keep the buffer aligned for the structs (the OS writes them aligned).
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` writable bytes, aligned to 8, alive for the call.
    let ok =
        unsafe { GetSystemCpuSetInformation(buf.as_mut_ptr().cast(), len, &mut len, process, 0) };
    if ok == 0 || len == 0 {
        return None;
    }
    let bytes: Vec<u8> = buf
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .take(len as usize)
        .collect();
    let base = offset_of!(SYSTEM_CPU_SET_INFORMATION, Anonymous);
    let f = |o: usize| base + o;
    type C = SYSTEM_CPU_SET_INFORMATION_0_0;
    let mut out = Vec::new();
    for rec in records(&bytes, offset_of!(SYSTEM_CPU_SET_INFORMATION, Size)) {
        let r = Rd(rec);
        // CpuSetInformation is type 0, the only type defined.
        if r.u32(offset_of!(SYSTEM_CPU_SET_INFORMATION, Type))? != 0 {
            continue;
        }
        let group = r.u16(f(offset_of!(C, Group)))?;
        let flags = r.u8(f(offset_of!(C, Anonymous1)))?;
        out.push(LogicalCpu {
            id: r.u32(f(offset_of!(C, Id)))?,
            group,
            number: r.u8(f(offset_of!(C, LogicalProcessorIndex)))? as u32,
            core: (group as u32) << 16 | r.u8(f(offset_of!(C, CoreIndex)))? as u32,
            efficiency_class: r.u8(f(offset_of!(C, EfficiencyClass)))?,
            llc: (group as u32) << 16 | r.u8(f(offset_of!(C, LastLevelCacheIndex)))? as u32,
            // Bit 0 Parked, 1 Allocated, 2 AllocatedToTargetProcess.
            parked: flags & 1 != 0,
            reserved: flags & 2 != 0 && flags & 4 == 0,
        });
    }
    (!out.is_empty()).then_some(out)
}

/// `GetLogicalProcessorInformationEx(relation)`'s raw records.
fn glpi(relation: LOGICAL_PROCESSOR_RELATIONSHIP) -> Option<Vec<u8>> {
    let mut len = 0u32;
    // SAFETY: a null buffer asks only for the size.
    unsafe { GetLogicalProcessorInformationEx(relation, std::ptr::null_mut(), &mut len) };
    let mut buf = vec![0u64; (len as usize).div_ceil(8)];
    // SAFETY: `buf` holds at least `len` writable bytes, aligned to 8, alive for the call.
    let ok =
        unsafe { GetLogicalProcessorInformationEx(relation, buf.as_mut_ptr().cast(), &mut len) };
    (ok != 0 && len != 0).then(|| {
        buf.iter()
            .flat_map(|w| w.to_le_bytes())
            .take(len as usize)
            .collect()
    })
}

/// The `(group, number)` of every logical processor in `count` group masks starting at `at`.
fn members(r: &Rd, at: usize, count: usize) -> Vec<(u16, u32)> {
    let mut out = Vec::new();
    for g in 0..count.max(1) {
        let m = at + g * size_of::<GROUP_AFFINITY>();
        let (Some(mask), Some(group)) = (r.u64(m), r.u16(m + offset_of!(GROUP_AFFINITY, Group)))
        else {
            break;
        };
        out.extend((0..64).filter(|b| mask >> b & 1 != 0).map(|b| (group, b)));
    }
    out
}

/// Every last-level cache: level, bytes, members.
fn last_level_caches() -> Option<Vec<(u8, u64, Vec<(u16, u32)>)>> {
    let buf = glpi(RelationCache)?;
    let h = offset_of!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, Anonymous);
    let size_at = offset_of!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, Size);
    let mut all = Vec::new();
    for rec in records(&buf, size_at) {
        let r = Rd(rec);
        let ty = r.u32(h + offset_of!(CACHE_RELATIONSHIP, Type))? as i32;
        if ty != CacheUnified && ty != CacheData {
            continue;
        }
        let level = r.u8(h + offset_of!(CACHE_RELATIONSHIP, Level))?;
        let bytes = r.u32(h + offset_of!(CACHE_RELATIONSHIP, CacheSize))? as u64;
        // GroupCount is 0 before Windows 11 / Server 2022: one mask then.
        let count = r.u16(h + offset_of!(CACHE_RELATIONSHIP, GroupCount))? as usize;
        let m = members(&r, h + offset_of!(CACHE_RELATIONSHIP, Anonymous), count);
        all.push((level, bytes, m));
    }
    let top = all.iter().map(|c| c.0).max()?;
    all.retain(|c| c.0 == top);
    Some(all)
}

/// The topology from `GetLogicalProcessorInformationEx` alone: cores and their efficiency
/// class from `RelationProcessorCore`, domains from the last-level `RelationCache`. Its ids are
/// `group · 64 + number` (no CPU set ids), so it serves to cross-check, not to place.
fn glpi_topology() -> Option<Topology> {
    let buf = glpi(RelationProcessorCore)?;
    let h = offset_of!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, Anonymous);
    let size_at = offset_of!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION_EX, Size);
    let caches = last_level_caches().unwrap_or_default();
    let domain_of = |gn: (u16, u32)| caches.iter().position(|c| c.2.contains(&gn));
    let mut cpus = Vec::new();
    for rec in records(&buf, size_at) {
        let r = Rd(rec);
        let class = r.u8(h + offset_of!(PROCESSOR_RELATIONSHIP, EfficiencyClass))?;
        let count = r.u16(h + offset_of!(PROCESSOR_RELATIONSHIP, GroupCount))? as usize;
        let m = members(&r, h + offset_of!(PROCESSOR_RELATIONSHIP, GroupMask), count);
        let Some(&(g0, n0)) = m.first() else { continue };
        for &(group, number) in &m {
            cpus.push(LogicalCpu {
                id: group as u32 * 64 + number,
                group,
                number,
                core: (g0 as u32) << 16 | n0,
                efficiency_class: class,
                llc: domain_of((group, number)).map_or(0, |d| d as u32),
                parked: false,
                reserved: false,
            });
        }
    }
    if cpus.is_empty() {
        return None;
    }
    let mut t = Topology::from_cpus(Source::ProcessorInformation, &cpus);
    for (i, c) in caches.iter().enumerate() {
        t.set_cache_info(i as u32, c.0, c.1);
    }
    Some(t)
}
