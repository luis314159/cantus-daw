//! The engine's `ExternalTuning` on MTS-ESP (ADR 0156), against the fake `LIBMTS` the
//! `cantus-rt` tests build (`crates/cantus-rt/tests/fake_mts/fake_mts.rs`, compiled here
//! with `rustc`): the master's table read as semitones from 12-TET at A440 and its unmapped
//! keys as `NAN`; Cantus's table published in Hz, an unmapped key given its lower neighbour's
//! frequency (ODDSound's advice); the conversions exact both ways; without the library every
//! call nothing; the block's read allocates nothing. r42:tuning-face

use cantus_core::tuning::TuningTable;
use cantus_engine::mts::{hz_of, table_of, MtsLink};
use cantus_engine::tuning::ExternalTuning;
use cantus_rt::mts::KEYS;
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! { static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

// SAFETY: forwards to the system allocator, counting this thread's allocations on the way.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNTING.with(|c| c.get()) {
            ALLOCS.fetch_add(1, Ordering::SeqCst);
        }
        // SAFETY: the caller's layout, forwarded.
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: forwarded.
        unsafe { System.dealloc(p, l) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn rustc() -> std::ffi::OsString {
    if let Some(r) = std::env::var_os("RUSTC") {
        return r;
    }
    if let Some(cargo) = std::env::var_os("CARGO") {
        let beside = PathBuf::from(cargo).with_file_name(if cfg!(windows) {
            "rustc.exe"
        } else {
            "rustc"
        });
        if beside.is_file() {
            return beside.into_os_string();
        }
    }
    "rustc".into()
}

/// The fake library, built once per run.
fn fake() -> &'static PathBuf {
    static LIB: OnceLock<PathBuf> = OnceLock::new();
    LIB.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fake-mts-engine");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(if cfg!(windows) {
            "fake_mts.dll"
        } else if cfg!(target_os = "macos") {
            "libfake_mts.dylib"
        } else {
            "libfake_mts.so"
        });
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../cantus-rt/tests/fake_mts/fake_mts.rs");
        let out = std::process::Command::new(rustc())
            .args(["--edition", "2021", "--crate-type", "cdylib"])
            .args(["--crate-name", "fake_mts", "-C", "opt-level=1", "-o"])
            .arg(&file)
            .arg(&src)
            .output()
            .expect("rustc runs");
        assert!(
            out.status.success(),
            "the fake LIBMTS did not build:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        file
    })
}

fn turn() -> MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

/// Werckmeister III's deviations from equal (cents, §5.2), at A415, as a table.
fn werck_415() -> TuningTable {
    let dev = [
        0.0, -9.775, -7.820, -5.865, -9.775, -1.955, -11.730, -3.910, -7.820, -11.730, -3.910,
        -7.820,
    ];
    let shift = 12.0 * cantus_dsp::math::log2(415.0f64 / 440.0);
    TuningTable(std::array::from_fn(|k| {
        ((dev[k % 12] - dev[9]) / 100.0 + shift) as f32
    }))
}

#[test]
fn hz_and_semitones_convert_both_ways_exactly_enough() {
    let t = werck_415();
    let hz = hz_of(&t);
    assert!((hz[69] - 415.0).abs() < 1e-3, "A4 at the diapason: {}", hz[69]);
    let mut back = TuningTable::EQUAL;
    table_of(&hz, &mut back);
    for k in 0..KEYS {
        // f32 table, f64 Hz: within a thousandth of a cent.
        assert!((back.0[k] - t.0[k]).abs() < 1e-5, "key {k}");
    }
    let mut equal = TuningTable::EQUAL;
    table_of(&hz_of(&TuningTable::EQUAL), &mut equal);
    assert!(equal.0.iter().all(|v| v.abs() < 1e-6));
}

#[test]
fn an_unmapped_key_is_published_at_its_lower_neighbours_frequency() {
    let mut t = TuningTable::EQUAL;
    t.0[61] = f32::NAN;
    t.0[0] = f32::NAN;
    let hz = hz_of(&t);
    assert_eq!(hz[61], hz[60]);
    assert_eq!(hz[0], hz[1], "none below: the next higher");
}

#[test]
fn without_the_library_every_call_is_nothing() {
    let link = MtsLink::absent();
    assert!(link.library().is_none() && link.absence().is_some());
    let mut out = werck_415();
    assert!(!link.read(&mut out));
    assert_eq!(out, werck_415(), "left alone");
    link.publish(&out);
    assert!(!link.is_master());
}

#[test]
fn cantus_follows_a_master_and_leads_its_clients() {
    let _t = turn();
    let follower = MtsLink::open(fake());
    let leader = MtsLink::open(fake());
    assert!(follower.library().is_some(), "{:?}", follower.absence());
    let mut out = TuningTable::EQUAL;
    assert!(!follower.read(&mut out), "no master yet");
    let mut table = werck_415();
    table.0[61] = f32::NAN;
    leader
        .become_master(false, &table, "Werckmeister III in Eb \u{b7} A415")
        .unwrap();
    assert!(leader.is_master() && follower.follows());
    assert!(follower.read(&mut out));
    for k in 0..KEYS {
        if k == 61 {
            assert!(out.0[k].is_nan(), "the master's unmapped key sounds nothing");
        } else {
            assert!((out.0[k] - table.0[k]).abs() < 1e-5, "key {k}");
        }
    }
    assert_eq!(
        follower.master_name().as_deref(),
        Some("Werckmeister III in Eb \u{b7} A415")
    );
    // A second master is refused while one leads.
    assert!(follower.become_master(false, &table, "x").is_err());
    // The block's read and publish: no allocation.
    ALLOCS.store(0, Ordering::SeqCst);
    COUNTING.with(|c| c.set(true));
    for _ in 0..500 {
        leader.publish(&table);
        let _ = follower.read(&mut out);
    }
    COUNTING.with(|c| c.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0);
    leader.resign_master();
    assert!(!follower.read(&mut out));
}
