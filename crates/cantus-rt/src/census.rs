//! A heap census for **measurement builds only** (round 40, ADR 0120 §1): a global allocator
//! that wraps the system's and keeps, besides the live bytes, the call stack of every block
//! of 64 KB or more and of a byte-weighted sample of the smaller ones (one in every
//! [`SAMPLE`] bytes a thread allocates, tcmalloc's method), and writes what is live, by call
//! stack, to the file `CANTUS_HEAP_CENSUS` names every ten seconds.
//!
//! It answers what the map from outside (`tools/scorecard/memmap.ps1 -Detail`) cannot: which
//! of the process heap's bytes are Cantus's own and which a driver's (the graphics and audio
//! drivers allocate from the same heap). The distributed build never has it (`cantus`'s
//! `census` feature, off by default); the audio thread never allocates, so it adds nothing
//! there.
//!
//! Stacks are taken with `RtlCaptureStackBackTrace` (no lock, no allocation) and named only
//! when the report is written (dbghelp, on the reporting thread).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;

/// One sampled block per this many bytes a thread allocates (each smaller block stands for
/// this many bytes when it is sampled).
pub const SAMPLE: usize = 128 << 10;
/// Blocks of this size or more are always recorded, at their own size.
pub const BIG: usize = 64 << 10;
const FRAMES: usize = 28;

/// The census allocator: `#[global_allocator] static A: cantus_rt::census::Census = Census;`
#[derive(Debug)]
pub struct Census;

static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);
static BLOCKS: AtomicIsize = AtomicIsize::new(0);

thread_local! {
    /// Inside the census's own bookkeeping (its allocations are counted, never recorded).
    static INSIDE: Cell<bool> = const { Cell::new(false) };
    /// Bytes this thread may still allocate before its next sample.
    static TILL: Cell<isize> = const { Cell::new(SAMPLE as isize) };
}

struct Rec {
    size: usize,
    weight: usize,
    frames: [usize; FRAMES],
    n: usize,
}

static TABLE: Mutex<Option<HashMap<usize, Rec>>> = Mutex::new(None);
/// A filter of the addresses ever recorded, so a free looks at the table only when its
/// address may be there (bits are never cleared; a collision costs one look).
static SEEN: [AtomicU64; 1024] = [const { AtomicU64::new(0) }; 1024];

fn bit(p: usize) -> (usize, u64) {
    let h = (p >> 4).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 48;
    ((h >> 6) & 1023, 1u64 << (h & 63))
}

fn record(p: usize, size: usize) {
    let weight = if size >= BIG {
        size
    } else {
        let left = TILL.with(|t| {
            let v = t.get() - size as isize;
            t.set(v);
            v
        });
        if left > 0 {
            return;
        }
        TILL.with(|t| t.set(left.rem_euclid(SAMPLE as isize) + 1));
        SAMPLE
    };
    if INSIDE.with(|i| i.replace(true)) {
        return;
    }
    let mut frames = [0usize; FRAMES];
    // SAFETY: the buffer holds FRAMES pointers; the hash pointer may be null.
    let n = unsafe {
        windows_sys::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace(
            2,
            FRAMES as u32,
            frames.as_mut_ptr() as *mut *mut c_void,
            std::ptr::null_mut(),
        )
    } as usize;
    let (w, b) = bit(p);
    SEEN[w].fetch_or(b, Relaxed);
    if let Ok(mut t) = TABLE.lock() {
        t.get_or_insert_with(HashMap::new).insert(
            p,
            Rec {
                size,
                weight,
                frames,
                n,
            },
        );
    }
    INSIDE.with(|i| i.set(false));
}

fn forget(p: usize) {
    let (w, b) = bit(p);
    if SEEN[w].load(Relaxed) & b == 0 {
        return;
    }
    if INSIDE.with(|i| i.replace(true)) {
        return;
    }
    if let Ok(mut t) = TABLE.lock() {
        if let Some(t) = t.as_mut() {
            t.remove(&p);
        }
    }
    INSIDE.with(|i| i.set(false));
}

fn grew(by: isize) {
    let now = LIVE.fetch_add(by, Relaxed) + by;
    PEAK.fetch_max(now, Relaxed);
}

// SAFETY: every call is forwarded to `System` with its own arguments; the bookkeeping never
// touches the block.
unsafe impl GlobalAlloc for Census {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, forwarded.
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            grew(l.size() as isize);
            BLOCKS.fetch_add(1, Relaxed);
            record(p as usize, l.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        // SAFETY: the caller's contract, forwarded.
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            grew(l.size() as isize);
            BLOCKS.fetch_add(1, Relaxed);
            record(p as usize, l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        forget(p as usize);
        grew(-(l.size() as isize));
        BLOCKS.fetch_sub(1, Relaxed);
        // SAFETY: the caller's contract, forwarded.
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        // SAFETY: the caller's contract, forwarded.
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
            forget(p as usize);
            grew(new as isize - l.size() as isize);
            record(q as usize, new);
        }
        q
    }
}

/// The live bytes the census counts.
pub fn live() -> isize {
    LIVE.load(Relaxed)
}

/// Starts the reporter when `CANTUS_HEAP_CENSUS` names a file: every ten seconds it writes
/// what is live, grouped by call stack and by the first Cantus (or other crate's) function
/// on it. Returns whether it started.
pub fn report_to_env() -> bool {
    let Some(path) = std::env::var_os("CANTUS_HEAP_CENSUS") else {
        return false;
    };
    std::thread::Builder::new()
        .name("cantus-heap-census".into())
        .spawn(move || {
            INSIDE.with(|i| i.set(true));
            let mut names = Names::new();
            loop {
                std::thread::sleep(std::time::Duration::from_secs(10));
                let text = report(&mut names);
                let _ = std::fs::write(&path, text);
            }
        })
        .is_ok()
}

struct Names {
    known: HashMap<usize, String>,
}

impl Names {
    fn new() -> Self {
        use windows_sys::Win32::System::Diagnostics::Debug::*;
        // SAFETY: plain calls on this process's pseudo-handle.
        unsafe {
            SymSetOptions(SYMOPT_UNDNAME | SYMOPT_DEFERRED_LOADS | SYMOPT_LOAD_LINES);
            SymInitializeW(
                windows_sys::Win32::System::Threading::GetCurrentProcess(),
                std::ptr::null(),
                1,
            );
        }
        Self {
            known: HashMap::new(),
        }
    }

    fn name(&mut self, ip: usize) -> String {
        if let Some(n) = self.known.get(&ip) {
            return n.clone();
        }
        use windows_sys::Win32::System::Diagnostics::Debug::*;
        #[repr(C)]
        struct Sym {
            info: SYMBOL_INFOW,
            more: [u16; 512],
        }
        // SAFETY: SYMBOL_INFOW is plain data; zeroed is a valid start for SymFromAddrW.
        let mut s: Sym = unsafe { std::mem::zeroed() };
        s.info.SizeOfStruct = std::mem::size_of::<SYMBOL_INFOW>() as u32;
        s.info.MaxNameLen = 512;
        let mut off = 0u64;
        let process = unsafe { windows_sys::Win32::System::Threading::GetCurrentProcess() };
        // SAFETY: the buffer holds MaxNameLen characters after the struct.
        let ok = unsafe { SymFromAddrW(process, ip as u64, &mut off, &mut s.info) } != 0;
        let n = if ok {
            let len = (s.info.NameLen as usize).min(512);
            // SAFETY: Name is the first of `len` characters SymFromAddrW wrote.
            let chars = unsafe { std::slice::from_raw_parts(s.info.Name.as_ptr(), len) };
            String::from_utf16_lossy(chars)
        } else {
            format!("{ip:#x}")
        };
        self.known.insert(ip, n.clone());
        n
    }
}

/// Frames of the allocator, the standard library and the collections: not owners.
fn plumbing(name: &str) -> bool {
    let n = name.trim_start_matches('<');
    [
        "alloc::",
        "core::",
        "std::",
        "hashbrown::",
        "cantus_rt::census",
        "__rust",
        "__rdl",
        "__rg",
        "RtlCapture",
        "smallvec::",
        "indexmap::",
    ]
    .iter()
    .any(|p| n.starts_with(p))
}

fn report(names: &mut Names) -> String {
    let recs: Vec<(usize, usize, Vec<usize>)> = match TABLE.lock() {
        Ok(t) => t
            .as_ref()
            .map(|t| {
                t.values()
                    .map(|r| (r.size, r.weight, r.frames[..r.n].to_vec()))
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => return String::from("the census's table is poisoned\n"),
    };
    let mut by_stack: HashMap<Vec<String>, (usize, usize, usize)> = HashMap::new();
    let mut by_owner: HashMap<String, usize> = HashMap::new();
    let mut seen = 0usize;
    for (size, weight, frames) in &recs {
        seen += weight;
        let named: Vec<String> = frames
            .iter()
            .map(|ip| names.name(*ip))
            .filter(|n| !plumbing(n))
            .take(8)
            .collect();
        let owner = named
            .iter()
            .find(|n| n.trim_start_matches('<').starts_with("cantus"))
            .or(named.first())
            .map(|n| owner_of(n))
            .unwrap_or_else(|| String::from("(no frame)"));
        *by_owner.entry(owner).or_default() += weight;
        let e = by_stack.entry(named).or_default();
        e.0 += weight;
        e.1 += 1;
        e.2 = e.2.max(*size);
    }
    let mb = |b: usize| b as f64 / 1_048_576.0;
    let mut out = format!(
        "live {:.2} MB in {} blocks (peak {:.2} MB); recorded {:.2} MB (blocks of {} KB or more at their size, the rest sampled one per {} KB)\n\nby owner (MB):\n",
        LIVE.load(Relaxed) as f64 / 1_048_576.0,
        BLOCKS.load(Relaxed),
        PEAK.load(Relaxed) as f64 / 1_048_576.0,
        mb(seen),
        BIG >> 10,
        SAMPLE >> 10
    );
    let mut owners: Vec<_> = by_owner.into_iter().collect();
    owners.sort_by_key(|(_, w)| std::cmp::Reverse(*w));
    for (o, w) in owners.iter().take(80) {
        out += &format!("  {:8.2}  {o}\n", mb(*w));
    }
    out += "\nby call stack (MB, records, largest block):\n";
    let mut stacks: Vec<_> = by_stack.into_iter().collect();
    stacks.sort_by_key(|(_, (w, _, _))| std::cmp::Reverse(*w));
    for (frames, (w, n, big)) in stacks.iter().take(80) {
        out += &format!("{:8.2}  ×{n}  max {} KB\n", mb(*w), big >> 10);
        for f in frames {
            out += &format!("            {f}\n");
        }
    }
    out
}

/// A function's owner: its crate and first module (`cantus_ui::kit`), generics dropped.
fn owner_of(name: &str) -> String {
    let n = name.trim_start_matches('<');
    let n = n.split(['<', ' ', '(']).next().unwrap_or(n);
    let parts: Vec<&str> = n.split("::").collect();
    parts[..parts.len().clamp(1, 3)].join("::")
}
