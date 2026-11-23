//! MTS-ESP's safe API against a fake `LIBMTS` (ADR 0156, "what the tests hold"): register,
//! publish, read, unregister; a second master refused, and the reinitialise ODDSound's header
//! offers when the library shares its state between processes; the block's read and publish
//! allocate nothing; a library that is not `LIBMTS` refused by name. The fake
//! (`tests/fake_mts/fake_mts.rs`) exports the same C symbols and is built here with `rustc`
//! — nothing of ODDSound's is used or shipped. r42:tuning-face

use cantus_rt::mts::{LoadError, MasterState, Mts, Refusal, KEYS};
use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Counts this thread's allocations while `COUNTING` is on (the audio thread's rule).
struct Counting;
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! { static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }

// SAFETY: forwards to the system allocator, counting on the way.
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

/// The fake library, built once per run into the test's own scratch folder.
fn fake() -> &'static PathBuf {
    static LIB: OnceLock<PathBuf> = OnceLock::new();
    LIB.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fake-mts");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join(if cfg!(windows) {
            "fake_mts.dll"
        } else if cfg!(target_os = "macos") {
            "libfake_mts.dylib"
        } else {
            "libfake_mts.so"
        });
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fake_mts/fake_mts.rs");
        let rustc = rustc();
        let out = std::process::Command::new(rustc)
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

/// The compiler of the toolchain running these tests: `RUSTC` when set, else the `rustc`
/// beside the `cargo` that runs them (cargo sets `CARGO` for a test), else the one on PATH.
fn rustc() -> std::ffi::OsString {
    if let Some(r) = std::env::var_os("RUSTC") {
        return r;
    }
    if let Some(cargo) = std::env::var_os("CARGO") {
        let beside = PathBuf::from(cargo).with_file_name(if cfg!(windows) { "rustc.exe" } else { "rustc" });
        if beside.is_file() {
            return beside.into_os_string();
        }
    }
    "rustc".into()
}

/// One library state per process: the tests take turns, each starting with no master.
fn turn() -> MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    let g = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let m = Mts::open(fake()).unwrap();
    m.lever("FAKE_MTS_SetOtherMaster", false);
    m.lever("FAKE_MTS_SetIPC", false);
    g
}

fn equal_hz(k: usize) -> f64 {
    440.0 * 2f64.powf((k as f64 - 69.0) / 12.0)
}

#[test]
fn a_client_reads_nothing_without_a_master_and_the_masters_table_with_one() {
    let _t = turn();
    let client = Mts::open(fake()).unwrap();
    client.register_client();
    assert_eq!(client.clients(), Some(1));
    let mut out = [0.0; KEYS];
    assert!(!client.has_master());
    assert!(!client.read(&mut out), "no master: nothing read");
    assert!(out.iter().all(|v| *v == 0.0), "out left alone");

    // Another program (a second handle on the same library) becomes the master.
    let master = Mts::open(fake()).unwrap();
    assert_eq!(master.master_state(), MasterState::Free);
    master.become_master(false).unwrap();
    assert!(master.is_master());
    let mut hz = [0.0; KEYS];
    for (k, v) in hz.iter_mut().enumerate() {
        *v = equal_hz(k) * 415.0 / 440.0;
    }
    master.publish(&hz);
    master.set_scale_name("Werckmeister III in Eb");
    let mut unmapped = [false; KEYS];
    unmapped[61] = true;
    master.filter(&unmapped);

    assert!(client.has_master());
    assert!(client.read(&mut out));
    assert_eq!(out, hz);
    assert_eq!(client.scale_name().as_deref(), Some("Werckmeister III in Eb"));
    assert!(client.should_filter(61) && !client.should_filter(60));
    // The master does not follow itself.
    let mut own = [0.0; KEYS];
    assert!(!master.read(&mut own));

    // The master goes: its clients fall back to their own tuning.
    drop(master);
    assert!(!client.has_master());
    assert!(!client.read(&mut out));
    client.deregister_client();
    assert_eq!(client.clients(), Some(0));
}

#[test]
fn a_second_master_is_refused_and_reinitialise_is_offered_only_across_processes() {
    let _t = turn();
    let ours = Mts::open(fake()).unwrap();
    // Another program holds the role.
    assert!(ours.lever("FAKE_MTS_SetOtherMaster", true));
    assert_eq!(
        ours.master_state(),
        MasterState::Another {
            can_reinitialize: false
        }
    );
    assert_eq!(
        ours.become_master(true),
        Err(Refusal::Another {
            can_reinitialize: false
        }),
        "no shared state: nothing to reinitialise"
    );
    assert!(!ours.is_master());
    // Shared between processes: a crashed master's flag may be stale; reinitialise is offered,
    // and taken only when asked.
    ours.lever("FAKE_MTS_SetIPC", true);
    assert_eq!(
        ours.become_master(false),
        Err(Refusal::Another {
            can_reinitialize: true
        })
    );
    ours.become_master(true).unwrap();
    assert_eq!(ours.master_state(), MasterState::Ours);
    // Publishing a table nobody else holds; nothing published once resigned.
    let hz: [f64; KEYS] = std::array::from_fn(equal_hz);
    ours.publish(&hz);
    ours.resign_master();
    assert_eq!(ours.master_state(), MasterState::Free);
    let other = Mts::open(fake()).unwrap();
    let mut changed = hz;
    changed[69] = 415.0;
    ours.publish(&changed); // not the master: nothing
    other.become_master(false).unwrap();
    let client = Mts::open(fake()).unwrap();
    let mut out = [0.0; KEYS];
    assert!(client.read(&mut out));
    assert_eq!(out[69], 440.0, "a resigned master published nothing");
}

#[test]
fn the_blocks_read_and_publish_allocate_nothing() {
    let _t = turn();
    let master = Mts::open(fake()).unwrap();
    master.become_master(false).unwrap();
    let client = Mts::open(fake()).unwrap();
    client.register_client();
    let hz: [f64; KEYS] = std::array::from_fn(equal_hz);
    let mut out = [0.0; KEYS];
    ALLOCS.store(0, Ordering::SeqCst);
    COUNTING.with(|c| c.set(true));
    for _ in 0..1000 {
        master.publish(&hz);
        let _ = client.has_master();
        let _ = client.read(&mut out);
        let _ = client.should_filter(60);
    }
    COUNTING.with(|c| c.set(false));
    assert_eq!(ALLOCS.load(Ordering::SeqCst), 0);
    assert_eq!(out, hz);
}

#[test]
fn a_library_that_is_not_libmts_is_refused_by_name() {
    let _t = turn();
    // This very test binary's neighbour: a library without MTS's functions. On every system
    // the fake's own folder holds only it, so build a second library with no exports.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("not-mts");
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("empty.rs");
    std::fs::write(&src, "#[no_mangle] pub extern \"C\" fn not_mts() {}\n").unwrap();
    let file = dir.join(if cfg!(windows) {
        "not_mts.dll"
    } else if cfg!(target_os = "macos") {
        "libnot_mts.dylib"
    } else {
        "libnot_mts.so"
    });
    let rustc = rustc();
    let ok = std::process::Command::new(rustc)
        .args(["--edition", "2021", "--crate-type", "cdylib", "--crate-name", "not_mts", "-o"])
        .arg(&file)
        .arg(&src)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    match Mts::open(&file) {
        Err(LoadError::Symbols(missing)) => assert!(missing.contains(&"MTS_GetTuningTable")),
        other => panic!("{other:?}"),
    }
}
