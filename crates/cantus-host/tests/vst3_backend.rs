//! The VST3 backend against plug-ins built from source in this repository
//! (`crates/cantus-test-plugins/vst3`): scanning, loading, audio, parameters, gestures,
//! latency, state, block-size independence, notes, the editor, and a hundred loads.
//!
//! The plug-in is a dev-dependency, so the cargo that builds these tests builds it too, as a
//! DLL next to the test binaries; the tests copy it into a `.vst3` bundle. (A test binary
//! run by hand without it builds it with a nested cargo.) Tests take a lock, because they
//! share one module and read its global counters.

#![cfg(all(feature = "vst3", any(target_os = "windows", target_os = "linux")))]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};

use cantus_host::vst3::Vst3Backend;
use cantus_host::{
    Audio, AudioSetup, AudioThread, Backend, Format, Loaded, NoteEvent, NoteExpression, NoteKind,
    Notice, ParamChange, ParamEvent, ParamOut, PluginInfo, PluginKind, TransportInfo,
};

// ---------------------------------------------------------------------------------------
// Allocation counting on the audio thread (ADR 0005)

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
}

// SAFETY: forwards to the system allocator; only counts.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        System.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if COUNTING.with(Cell::get) {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Allocations and frees made by this thread while `f` runs.
fn allocations_in(f: impl FnOnce()) -> usize {
    let before = ALLOCATIONS.load(Ordering::SeqCst);
    COUNTING.with(|c| c.set(true));
    f();
    COUNTING.with(|c| c.set(false));
    ALLOCATIONS.load(Ordering::SeqCst) - before
}

// ---------------------------------------------------------------------------------------
// The test plug-in

const GAIN_ID: &str = "43414E545553544741494E50524F4331";
const PULSE_ID: &str = "43414E5455535450554C5345494E5331";
const GAIN: u32 = 0;
const LATENCY: u32 = 1;
const RATE: f64 = 48_000.0;
const MAX_BLOCK: usize = 4096;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The test plug-in laid out as a bundle (once per test run):
/// `CantusTest.vst3/Contents/<arch>/CantusTest.vst3`.
fn bundle() -> &'static Path {
    static BUNDLE: OnceLock<PathBuf> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        let (built, arch, ext) = if cfg!(target_os = "windows") {
            ("cantus_test_vst3.dll", "x86_64-win", "vst3")
        } else {
            ("libcantus_test_vst3.so", "x86_64-linux", "so")
        };
        let target = workspace().join("target").join("cantus-test-plugins");
        let beside_tests = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|deps| deps.join(built)))
            .filter(|p| p.is_file());
        let source = beside_tests.unwrap_or_else(|| build_by_hand(&target).join(built));
        let bundle = target.join("bundles").join("CantusTest.vst3");
        let dir = bundle.join("Contents").join(arch);
        std::fs::create_dir_all(&dir).expect("bundle folders");
        std::fs::copy(&source, dir.join(format!("CantusTest.{ext}")))
            .expect("copy the plug-in into its bundle");
        bundle
    })
}

/// Build the plug-in with a nested cargo into its own target folder; returns the folder
/// holding the binary.
fn build_by_hand(target: &Path) -> PathBuf {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let built = Command::new(cargo)
        .current_dir(workspace())
        .args(["build", "--quiet", "-p", "cantus-test-vst3", "--target-dir"])
        .arg(target)
        .output()
        .expect("run cargo to build the test plug-in");
    assert!(
        built.status.success(),
        "building the test plug-in failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    target.join("debug")
}

fn binary() -> PathBuf {
    cantus_host::vst3::module::binary_path(bundle()).expect("the bundle has a binary")
}

/// One test at a time: they share the module and its counters.
fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/// The plug-in's own `cantus_test_*` exports, through a second handle on the same module
/// (which also keeps it mapped, so its counters survive the host unloading it).
struct Probe {
    lib: libloading::Library,
}

impl Probe {
    fn new() -> Probe {
        // SAFETY: our own test plug-in; loading it only runs Rust's initialisers.
        let lib = unsafe { libloading::Library::new(binary()) }.expect("open the test plug-in");
        Probe { lib }
    }

    fn call<T>(&self, name: &[u8]) -> libloading::Symbol<'_, T> {
        // SAFETY: the names and signatures are the plug-in's (`src/lib.rs`).
        unsafe { self.lib.get(name) }.expect("test export")
    }

    fn live_objects(&self) -> i32 {
        let f = self.call::<unsafe extern "C" fn() -> i32>(b"cantus_test_live_objects\0");
        // SAFETY: as above.
        unsafe { f() }
    }

    fn module_entries(&self) -> i32 {
        let f = self.call::<unsafe extern "C" fn() -> i32>(b"cantus_test_module_entries\0");
        // SAFETY: as above.
        unsafe { f() }
    }

    fn perform_edit(&self, id: u32, normalized: f64) -> i32 {
        let f = self.call::<unsafe extern "C" fn(u32, f64) -> i32>(b"cantus_test_perform_edit\0");
        // SAFETY: as above.
        unsafe { f(id, normalized) }
    }

    fn zoom(&self, set: f64) -> f64 {
        let f = self.call::<unsafe extern "C" fn(f64) -> f64>(b"cantus_test_zoom\0");
        // SAFETY: as above.
        unsafe { f(set) }
    }

    // The editor's tests run where editors open (Windows so far).
    #[cfg_attr(not(windows), allow(dead_code))]
    fn resize_editor(&self, w: i32, h: i32) -> i32 {
        let f = self.call::<unsafe extern "C" fn(i32, i32) -> i32>(b"cantus_test_resize_editor\0");
        // SAFETY: as above.
        unsafe { f(w, h) }
    }

    /// (parent window, width, height) of the newest editor.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn editor(&self) -> (usize, i32, i32) {
        let f =
            self.call::<unsafe extern "C" fn(*mut i32, *mut i32) -> usize>(b"cantus_test_editor\0");
        let (mut w, mut h) = (0, 0);
        // SAFETY: as above.
        let parent = unsafe { f(&mut w, &mut h) };
        (parent, w, h)
    }

    /// (peak, silence flags) of the sidechain in the gain's last block.
    fn key(&self) -> (f64, u64) {
        let f = self.call::<unsafe extern "C" fn(*mut u64) -> f64>(b"cantus_test_key\0");
        let mut silence = 0;
        // SAFETY: as above.
        let peak = unsafe { f(&mut silence) };
        (peak, silence)
    }

    fn messages(&self) -> i32 {
        let f = self.call::<unsafe extern "C" fn() -> i32>(b"cantus_test_messages\0");
        // SAFETY: as above.
        unsafe { f() }
    }

    /// (state flags, [tempo, music, bar, cycle start, cycle end, num, den, samples]).
    fn last_context(&self) -> (u32, [f64; 8]) {
        let f = self.call::<unsafe extern "C" fn(*mut f64) -> u32>(b"cantus_test_last_context\0");
        let mut v = [0.0; 8];
        // SAFETY: as above; the export writes eight values.
        let state = unsafe { f(v.as_mut_ptr()) };
        (state, v)
    }
}

fn setup() -> AudioSetup {
    AudioSetup {
        sample_rate: RATE,
        max_block: MAX_BLOCK,
    }
}

fn info_for(id: &str) -> PluginInfo {
    Vst3Backend::new()
        .scan(bundle())
        .expect("scan")
        .into_iter()
        .find(|i| i.id == id)
        .expect("the class is in the bundle")
}

fn load(id: &str) -> Loaded {
    Vst3Backend::new()
        .load(&info_for(id), setup())
        .expect("load")
}

/// Deterministic noise in −1…1.
fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut x = seed.wrapping_mul(2_654_435_761).max(1);
    (0..n)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            (x as f32 / u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

/// Run `left`/`right` through `audio` in blocks of `block`, handing each block the
/// changes whose absolute position falls in it. Returns every event the plug-in reported.
fn run(
    audio: &mut dyn AudioThread,
    left: &mut [f32],
    right: &mut [f32],
    block: usize,
    changes: &[(usize, u32, f64)],
) -> Vec<ParamEvent> {
    let mut out = ParamOut::new();
    let mut events = Vec::new();
    let mut params = Vec::with_capacity(changes.len());
    let mut start = 0;
    while start < left.len() {
        let end = (start + block).min(left.len());
        params.clear();
        for &(at, id, value) in changes {
            if (start..end).contains(&at) {
                params.push(ParamChange {
                    offset: (at - start) as u32,
                    id,
                    value,
                });
            }
        }
        out.clear();
        audio.process(
            Audio {
                left: &mut left[start..end],
                right: &mut right[start..end],
                key: None,
                transport: None,
            },
            &params,
            &[],
            &mut out,
        );
        events.extend_from_slice(out.events());
        start = end;
    }
    events
}

fn db(gain_db: f64) -> f32 {
    10f64.powf(gain_db / 20.0) as f32
}

fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

// ---------------------------------------------------------------------------------------
// Discovery and scanning

#[test]
fn the_scan_finds_both_classes_with_their_full_description() {
    let _g = serial();
    let backend = Vst3Backend::new();
    assert_eq!(backend.format(), Format::Vst3);
    assert!(backend.is_candidate(bundle()), "a bundle folder");
    assert!(
        !backend.is_candidate(&binary()),
        "not the binary inside a bundle"
    );
    assert!(!backend.is_candidate(Path::new("x.clap")));

    let infos = backend.scan(bundle()).expect("scan");
    assert_eq!(
        infos.len(),
        2,
        "the controller class is not a plug-in: {infos:#?}"
    );
    let gain = infos.iter().find(|i| i.id == GAIN_ID).expect("gain");
    assert_eq!(
        gain,
        &PluginInfo {
            format: Format::Vst3,
            id: GAIN_ID.into(),
            name: "Cantus Test Gain".into(),
            vendor: "Cantus Tests".into(),
            version: "1.0.0".into(),
            path: bundle().to_path_buf(),
            kind: PluginKind::Effect,
            main_inputs: 2,
            main_outputs: 2,
            sidechain: true,
            editor: true,
            features: vec!["Fx".into(), "Dynamics".into()],
        }
    );
    let pulse = infos.iter().find(|i| i.id == PULSE_ID).expect("pulse");
    assert_eq!(pulse.name, "Cantus Test Pulse");
    assert_eq!(pulse.kind, PluginKind::Instrument);
    assert_eq!((pulse.main_inputs, pulse.main_outputs), (0, 2));
    assert!(!pulse.sidechain);
    assert!(!pulse.editor, "the pulse makes no view");
}

#[test]
fn search_paths_are_the_standard_folders() {
    let paths = Vst3Backend::new().search_paths();
    if cfg!(target_os = "windows") {
        let common = PathBuf::from(std::env::var_os("COMMONPROGRAMFILES").unwrap()).join("VST3");
        let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap())
            .join("Programs")
            .join("Common")
            .join("VST3");
        assert_eq!(
            paths,
            vec![local, common],
            "per user first, then system-wide"
        );
    } else {
        assert!(paths.iter().any(|p| p.ends_with(".vst3")));
    }
}

#[test]
fn moduleinfo_json_describes_a_bundle_without_loading_it() {
    let _g = serial();
    // A bundle with the file and no binary at all: anything found came from the file.
    let dir = workspace()
        .join("target")
        .join("cantus-test-plugins")
        .join("bundles")
        .join("Described.vst3");
    let resources = dir.join("Contents").join("Resources");
    std::fs::create_dir_all(&resources).unwrap();
    std::fs::write(
        resources.join("moduleinfo.json"),
        format!(
            r#"{{
                // as moduleinfotool writes it
                "Name": "Described",
                "Version": "2.0.0",
                "Factory Info": {{ "Vendor": "Cantus Tests", "Flags": {{ "Unicode": true, }}, }},
                "Classes": [
                    {{
                        "CID": "{GAIN_ID}",
                        "Category": "Audio Module Class",
                        "Name": "Cantus Test Gain",
                        "Sub Categories": [ "Fx", "Dynamics" ],
                    }},
                ],
            }}"#
        ),
    )
    .unwrap();
    let backend = Vst3Backend::new();
    let infos = backend.scan(&dir).expect("described without loading");
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].id, GAIN_ID);
    assert_eq!(
        infos[0].version, "2.0.0",
        "the module's version when the class has none"
    );
    assert_eq!(infos[0].kind, PluginKind::Effect);
    assert!(
        backend.scan_loading(&dir).is_err(),
        "there is nothing to load"
    );
    assert!(
        !infos[0].sidechain,
        "the file does not say; the default is none"
    );

    // The same file next to a real binary: loading makes the description exact.
    let with_binary = dir.with_file_name("WithInfo.vst3");
    let arch = with_binary
        .join("Contents")
        .join(binary().parent().unwrap().file_name().unwrap());
    std::fs::create_dir_all(&arch).unwrap();
    std::fs::create_dir_all(with_binary.join("Contents").join("Resources")).unwrap();
    let ext = binary().extension().unwrap().to_owned();
    let copy = arch.join("WithInfo").with_extension(ext);
    std::fs::copy(binary(), &copy).unwrap();
    std::fs::copy(
        resources.join("moduleinfo.json"),
        with_binary
            .join("Contents")
            .join("Resources")
            .join("moduleinfo.json"),
    )
    .unwrap();
    let described = backend.scan(&with_binary).expect("described");
    assert!(!described[0].sidechain);
    let loaded = backend.load(&described[0], setup()).expect("load");
    let exact = loaded.main.info();
    assert!(exact.sidechain, "the plug-in's aux input, found at load");
    assert_eq!((exact.main_inputs, exact.main_outputs), (2, 2));
    assert_eq!(exact.name, "Cantus Test Gain");
}

#[test]
fn the_sidechain_reaches_the_aux_bus() {
    let _g = serial();
    let probe = Probe::new();
    let Loaded { main, mut audio } = load(GAIN_ID);
    let (mut l, mut r) = (vec![0.0f32; 64], vec![0.0f32; 64]);
    let (kl, kr) = (vec![0.5f32; 64], vec![-0.25f32; 64]);
    let mut out = ParamOut::new();
    audio.process(
        Audio {
            left: &mut l,
            right: &mut r,
            key: Some((&kl, &kr)),
            transport: None,
        },
        &[],
        &[],
        &mut out,
    );
    assert_eq!(probe.key(), (0.5, 0), "the key, not flagged silent");
    audio.process(
        Audio {
            left: &mut l,
            right: &mut r,
            key: None,
            transport: None,
        },
        &[],
        &[],
        &mut out,
    );
    assert_eq!(probe.key(), (0.0, 0b11), "silence, and said to be");
    audio.stop();
    drop((main, audio));
}

#[test]
fn a_class_the_module_does_not_have_is_not_found() {
    let _g = serial();
    let mut info = info_for(GAIN_ID);
    info.id = "00000000000000000000000000000001".into();
    assert!(matches!(
        Vst3Backend::new().load(&info, setup()),
        Err(cantus_host::HostError::NotFound(_))
    ));
}

// ---------------------------------------------------------------------------------------
// Audio

#[test]
fn at_0_db_the_audio_is_bit_exact() {
    let _g = serial();
    let Loaded { main, mut audio } = load(GAIN_ID);
    assert_eq!(main.info().kind, PluginKind::Effect);
    assert!(main.info().sidechain, "the aux input is the sidechain");
    let (l0, r0) = (noise(10_000, 1), noise(10_000, 2));
    let (mut l, mut r) = (l0.clone(), r0.clone());
    let events = run(&mut *audio, &mut l, &mut r, 512, &[]);
    assert!(events.is_empty());
    assert!(same_bits(&l, &l0) && same_bits(&r, &r0));
    audio.stop();
}

#[test]
fn a_parameter_change_lands_on_its_sample() {
    let _g = serial();
    let Loaded {
        mut main,
        mut audio,
    } = load(GAIN_ID);
    let params = main.params();
    // Gain and Latency, then the hidden Fault and Process of round 29's isolation tests.
    assert_eq!(params.len(), 4);
    assert!(params[2..].iter().all(|p| p.hidden), "{params:?}");
    assert!(params[3].read_only, "Process is read-only");
    let gain = &params[0];
    assert_eq!((gain.id, gain.name.as_str()), (GAIN, "Gain"));
    assert_eq!((gain.min, gain.max, gain.default), (-48.0, 48.0, 0.0));
    assert!(!gain.stepped && gain.automatable && !gain.hidden && !gain.read_only);
    let latency = &params[1];
    assert_eq!(
        (latency.min, latency.max, latency.default),
        (0.0, 4096.0, 0.0)
    );
    assert!(latency.stepped);
    assert_eq!(main.param_text(GAIN, -6.0).as_deref(), Some("-6.0 dB"));
    assert_eq!(
        main.param_text(LATENCY, 256.0).as_deref(),
        Some("256 samples")
    );

    let (mut l, mut r) = (vec![1.0f32; 512], vec![0.5f32; 512]);
    run(&mut *audio, &mut l, &mut r, 512, &[(100, GAIN, -6.0)]);
    assert!(l[..100].iter().all(|&x| x == 1.0), "before the change");
    assert!(
        l[100..].iter().all(|&x| x == db(-6.0)),
        "from the change on"
    );
    assert!(r[100..].iter().all(|&x| x == 0.5 * db(-6.0)));

    // The controller follows what the processor was told.
    main.idle();
    assert_eq!(main.param_value(GAIN), Some(-6.0));
    audio.stop();
}

#[test]
fn the_reported_latency_is_the_plug_ins() {
    let _g = serial();
    let Loaded {
        mut main,
        mut audio,
    } = load(GAIN_ID);
    assert_eq!(main.latency(), 0);
    let (l0, r0) = (noise(4096, 3), noise(4096, 4));
    let (mut l, mut r) = (l0.clone(), r0.clone());
    run(&mut *audio, &mut l, &mut r, 256, &[(0, LATENCY, 256.0)]);
    // The controller hears of the change on the next idle and restarts the component
    // with kLatencyChanged; the host reads the new latency and says so.
    let notices = main.idle();
    assert!(
        notices.contains(&Notice::LatencyChanged(256)),
        "{notices:?}"
    );
    assert_eq!(main.latency(), 256);
    assert!(l[..256].iter().all(|&x| x == 0.0), "the delay starts empty");
    assert!(
        same_bits(&l[256..], &l0[..4096 - 256]),
        "exactly 256 samples late"
    );
    assert!(same_bits(&r[256..], &r0[..4096 - 256]));
    audio.stop();
}

#[test]
fn block_sizes_from_1_to_4096_give_identical_audio() {
    let _g = serial();
    let len = 12_000;
    let changes = [
        (0, LATENCY, 100.0),
        (1234, GAIN, -6.0),
        (5000, GAIN, 3.0),
        (5001, GAIN, -12.5),
        (9999, GAIN, 0.0),
    ];
    let render = |block: usize| {
        let Loaded { main, mut audio } = load(GAIN_ID);
        let (mut l, mut r) = (noise(len, 5), noise(len, 6));
        run(&mut *audio, &mut l, &mut r, block, &changes);
        audio.stop();
        drop((audio, main));
        (l, r)
    };
    let (l_ref, r_ref) = render(MAX_BLOCK);
    assert!(l_ref[..100].iter().all(|&x| x == 0.0));
    assert!(l_ref[100..].iter().any(|&x| x != 0.0));
    for block in [1, 2, 3, 7, 64, 100, 1000, 4095] {
        let (l, r) = render(block);
        assert!(same_bits(&l, &l_ref), "left differs at block size {block}");
        assert!(same_bits(&r, &r_ref), "right differs at block size {block}");
    }
}

#[test]
fn processing_allocates_nothing() {
    let _g = serial();
    let Loaded { main, mut audio } = load(GAIN_ID);
    let probe = Probe::new();
    let (mut l, mut r) = (noise(4096, 7), noise(4096, 8));
    run(&mut *audio, &mut l, &mut r, 512, &[]); // first block: setProcessing(true)
    probe.perform_edit(GAIN, 0.25); // a gesture waiting in the ring
    let params = [
        ParamChange {
            offset: 3,
            id: GAIN,
            value: -3.0,
        },
        ParamChange {
            offset: 9,
            id: LATENCY,
            value: 12.0,
        },
    ];
    let mut out = ParamOut::new();
    let n = allocations_in(|| {
        for _ in 0..8 {
            out.clear();
            audio.process(
                Audio {
                    left: &mut l[..512],
                    right: &mut r[..512],
                    key: Some((&[0.0; 512][..], &[0.0; 512][..])),
                    transport: None,
                },
                &params,
                &[],
                &mut out,
            );
        }
        audio.reset();
        audio.stop();
    });
    assert_eq!(n, 0, "the host side of process allocated or freed");
    drop((audio, main));
}

// ---------------------------------------------------------------------------------------
// The controller, state, lifetime

#[test]
fn an_edit_in_the_plug_ins_editor_reaches_the_host_as_a_gesture() {
    let _g = serial();
    let Loaded {
        mut main,
        mut audio,
    } = load(GAIN_ID);
    let probe = Probe::new();
    // The editor drags Gain to normalized 0.25 (−24 dB): beginEdit, performEdit, endEdit.
    assert_eq!(
        probe.perform_edit(GAIN, 0.25),
        1,
        "the controller has our handler"
    );
    let (mut l, mut r) = (vec![1.0f32; 64], vec![1.0f32; 64]);
    let events = run(&mut *audio, &mut l, &mut r, 64, &[]);
    assert_eq!(
        events,
        vec![
            ParamEvent::Begin(GAIN),
            ParamEvent::Value(GAIN, -24.0),
            ParamEvent::End(GAIN),
        ],
        "in order, with the plain value"
    );
    assert!(
        l.iter().all(|&x| x == db(-24.0)),
        "the processor got the value from the block's first sample"
    );
    assert!(main.idle().is_empty(), "an edit is not a notice");
    assert_eq!(main.param_value(GAIN), Some(-24.0));
    audio.stop();
}

#[test]
fn the_processor_talks_to_the_controller_through_the_hosts_messages() {
    let _g = serial();
    let probe = Probe::new();
    let before = probe.messages();
    let loaded = load(GAIN_ID);
    assert_eq!(
        probe.messages(),
        before + 1,
        "setActive(true) sent one IMessage the host allocated"
    );
    drop(loaded);
}

#[test]
fn state_round_trips_through_a_vstpreset() {
    let _g = serial();
    let probe = Probe::new();
    let saved = {
        let Loaded {
            mut main,
            mut audio,
        } = load(GAIN_ID);
        let (mut l, mut r) = (vec![0.0f32; 64], vec![0.0f32; 64]);
        run(
            &mut *audio,
            &mut l,
            &mut r,
            64,
            &[(0, GAIN, -12.0), (0, LATENCY, 64.0)],
        );
        audio.stop();
        main.idle();
        assert_eq!(probe.zoom(2.5), 2.5, "the controller's own state");
        let saved = main.save_state().expect("save");
        assert_eq!(&saved[..4], b"VST3", "a .vstpreset");
        assert_eq!(&saved[8..40], GAIN_ID.as_bytes());
        saved
    };

    let Loaded {
        mut main,
        mut audio,
    } = load(GAIN_ID);
    assert_eq!(probe.zoom(-1.0), 1.0, "a fresh controller");
    assert_eq!(main.param_value(GAIN), Some(0.0));
    main.load_state(&saved).expect("load");
    assert_eq!(
        main.param_value(GAIN),
        Some(-12.0),
        "setComponentState reached the controller"
    );
    assert_eq!(main.param_value(LATENCY), Some(64.0));
    assert_eq!(probe.zoom(-1.0), 2.5, "the controller state came back");
    assert_eq!(main.latency(), 64);
    assert!(main.idle().contains(&Notice::LatencyChanged(64)));
    assert_eq!(
        main.save_state().expect("save again"),
        saved,
        "byte for byte"
    );

    let (mut l, mut r) = (vec![1.0f32; 128], vec![1.0f32; 128]);
    run(&mut *audio, &mut l, &mut r, 128, &[]);
    assert!(l[..64].iter().all(|&x| x == 0.0) && l[64..].iter().all(|&x| x == db(-12.0)));

    let mut other = saved.clone();
    other[8] = b'F';
    assert!(
        main.load_state(&other).is_err(),
        "another class's state is refused"
    );
    assert!(main.load_state(b"not a preset").is_err());
    audio.stop();
}

#[test]
fn a_hundred_loads_leave_nothing_behind() {
    let _g = serial();
    let probe = Probe::new();
    let live = probe.live_objects();
    let entries = probe.module_entries();
    let info = info_for(GAIN_ID);
    let backend = Vst3Backend::new();
    for i in 0..100 {
        let Loaded {
            mut main,
            mut audio,
        } = backend.load(&info, setup()).expect("load");
        let (mut l, mut r) = (noise(256, i), noise(256, i + 1));
        run(&mut *audio, &mut l, &mut r, 256, &[(7, GAIN, -1.0)]);
        audio.stop();
        main.idle();
        main.save_state().expect("save");
        // Either half may go first.
        if i % 2 == 0 {
            drop(main);
            drop(audio);
        } else {
            drop(audio);
            drop(main);
        }
    }
    assert_eq!(
        probe.live_objects(),
        live,
        "every object the plug-in made was released"
    );
    assert_eq!(
        probe.module_entries(),
        entries,
        "every InitDll had its ExitDll"
    );
}

// ---------------------------------------------------------------------------------------
// Notes and transport (the single-component instrument)

#[test]
fn notes_and_expressions_land_on_their_samples() {
    let _g = serial();
    let probe = Probe::new();
    let Loaded { main, mut audio } = load(PULSE_ID);
    assert_eq!(main.info().kind, PluginKind::Instrument);
    // Only the hidden Fault and Process of round 29's isolation tests.
    assert!(
        main.params().iter().all(|p| p.hidden),
        "{:?}",
        main.params()
    );
    let note = |offset, kind| NoteEvent {
        offset,
        note_id: -1,
        channel: 0,
        key: 60,
        kind,
    };
    let notes = [
        note(5, NoteKind::On { velocity: 0.75 }),
        note(
            12,
            NoteKind::Expression {
                expression: NoteExpression::Pan,
                value: 0.25,
            },
        ),
        note(
            13,
            NoteKind::Expression {
                expression: NoteExpression::Pressure,
                value: 0.5,
            },
        ),
        note(20, NoteKind::Off { velocity: 0.5 }),
    ];
    let transport = TransportInfo {
        playing: true,
        recording: false,
        tempo_bpm: 128.0,
        tempo_inc: 0.0,
        position_beats: 6.5,
        position_samples: 146_250,
        // 6.5 beats at 128 BPM: 146 250 samples at 48 kHz.
        position_seconds: 3.046_875,
        project_samples: 146_250,
        bar_start_beats: 6.0,
        bar_number: 2,
        time_sig_num: 3,
        time_sig_den: 4,
        loop_beats: Some((0.0, 12.0)),
        loop_seconds: Some((0.0, 5.625)),
    };
    let (mut l, mut r) = (vec![9.0f32; 32], vec![9.0f32; 32]);
    let mut out = ParamOut::new();
    audio.process(
        Audio {
            left: &mut l,
            right: &mut r,
            key: None,
            transport: Some(&transport),
        },
        &[],
        &notes,
        &mut out,
    );
    let mut want_l = vec![0.0f32; 32];
    want_l[5] = 0.75;
    want_l[20] = -0.5;
    let mut want_r = vec![0.0f32; 32];
    want_r[12] = 0.25;
    want_r[13] = 0.5;
    assert_eq!(l, want_l);
    assert_eq!(r, want_r);

    let (state, v) = probe.last_context();
    // ProcessContext flags: playing 2, cycle active 4, time-music 512, tempo 1024,
    // bar 2048, cycle valid 4096, time signature 8192, continuous time 131072.
    let want = 2 | 4 | 512 | 1024 | 2048 | 4096 | 8192 | 131_072;
    assert_eq!(state, want, "{state:#b}");
    assert_eq!(v, [128.0, 6.5, 6.0, 0.0, 12.0, 3.0, 4.0, 146_250.0]);
    // Under a tempo map (round 29, ADR 0060; review 1): the song's real samples (the
    // engine's rounding of its seconds, review 2) become `projectTimeSamples`, whatever the
    // beats would be at the tempo now.
    let ramp = TransportInfo {
        tempo_bpm: 131.5,
        tempo_inc: 1e-4,
        position_beats: 14.0,
        position_seconds: 7.25,
        project_samples: 348_000,
        bar_start_beats: 12.0,
        bar_number: 4,
        ..transport
    };
    let (mut l, mut r) = (vec![0.0f32; 32], vec![0.0f32; 32]);
    audio.process(
        Audio {
            left: &mut l,
            right: &mut r,
            key: None,
            transport: Some(&ramp),
        },
        &[],
        &[],
        &mut out,
    );
    let (_, v) = probe.last_context();
    assert_eq!(v[0], 131.5, "the tempo");
    assert_eq!(v[1], 14.0, "the beats");
    assert_eq!(v[7], 348_000.0, "7.25 s at 48 kHz");
    audio.stop();
}

// ---------------------------------------------------------------------------------------
// The editor

#[cfg(target_os = "windows")]
#[test]
fn the_editor_attaches_resizes_and_detaches() {
    use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
    use std::num::NonZeroIsize;

    let _g = serial();
    let probe = Probe::new();
    let Loaded { mut main, audio } = load(GAIN_ID);
    assert!(main.has_editor());
    // The test view never touches its parent, so any non-zero handle stands for one.
    let parent = RawWindowHandle::Win32(Win32WindowHandle::new(NonZeroIsize::new(0x5150).unwrap()));
    let size = main.open_editor(parent).expect("open");
    assert_eq!((size.width, size.height), (400, 300));
    assert_eq!(
        probe.editor(),
        (0x5150, 400, 300),
        "attached to the parent, as HWND"
    );

    assert_eq!(probe.resize_editor(640, 480), 1, "the view has our frame");
    let notices = main.idle();
    assert!(
        notices.contains(&Notice::EditorResized(cantus_host::EditorSize {
            width: 640,
            height: 480
        })),
        "{notices:?}"
    );
    assert_eq!(probe.editor(), (0x5150, 640, 480), "onSize followed");

    main.close_editor();
    assert_eq!(probe.editor().0, 0, "removed");
    drop((main, audio));
}

// ---------------------------------------------------------------------------------------
// The instrument adapter (ADR 0028) over a VST3 instrument

#[test]
fn a_vst3_instrument_plays_through_the_instrument_adapter() {
    use cantus_engine::inst::{InstrumentUnit, NoteOn};
    let _g = serial();
    let Loaded { main, audio } = load(PULSE_ID);
    let mut unit = cantus_host::HostedInstrument::new(
        audio,
        &main.params(),
        |id| main.param_value(id),
        main.latency(),
        RATE,
        Default::default(),
    );
    let (mut l, mut r) = (vec![0.0f32; 16], vec![0.0f32; 16]);
    assert!(
        !unit.process(&mut l, &mut r),
        "nothing played, nothing written"
    );
    // A note given between two calls is the first sample of the next one.
    unit.note_on(NoteOn {
        key: 60,
        velocity: 0.75,
        id: 7,
        channel: 0,
    });
    l.fill(0.0);
    r.fill(0.0);
    assert!(unit.process(&mut l, &mut r));
    assert_eq!(l[0], 0.75);
    assert!(l[1..].iter().all(|x| *x == 0.0));
    assert_eq!(unit.active_voices(), 1);
    unit.note_off(60, 7, 0.5);
    l.fill(0.0);
    r.fill(0.0);
    unit.process(&mut l, &mut r);
    assert_eq!(l[0], -0.5, "its release velocity");
    assert_eq!(unit.active_voices(), 0);
    // A stop releases what still sounds.
    unit.note_on(NoteOn {
        key: 64,
        velocity: 0.5,
        id: 8,
        channel: 0,
    });
    unit.all_notes_off();
    assert_eq!(unit.active_voices(), 0);
    l.fill(0.0);
    r.fill(0.0);
    unit.process(&mut l, &mut r);
    assert_eq!(l[0], 0.5, "the note-on, then its note-off, in one instant");
    unit.stop();
}
