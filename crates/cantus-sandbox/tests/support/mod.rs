//! What the isolation tests share: the test plug-ins built from source (the CLAP gain and
//! synth, the VST3 gain and pulse), this build's `cantus-sandbox`, backends that load into
//! it, and a rig that runs a project through the whole host and engine.
#![allow(dead_code)]

use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_host::bridge::{BridgedBackend, Sandboxes};
use cantus_host::db::{stamp, DbEntry, ScanStatus};
use cantus_host::{
    Audio, AudioSetup, AudioThread, Format, HostError, Loaded, MainThread, NoteEvent, Notice,
    ParamChange, ParamOut, PluginHost, PluginInfo, StatePoll,
};
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

pub const SR: f64 = 48_000.0;
pub const MAX_BLOCK: usize = 8192;

pub const GAIN_ID: &str = "org.cantus.test.gain";
pub const SYNTH_ID: &str = "org.cantus.test.synth";
pub const VST3_GAIN_ID: &str = "43414E545553544741494E50524F4331";
pub const VST3_PULSE_ID: &str = "43414E5455535450554C5345494E5331";

/// clap-gain's parameters.
pub const P_GAIN: u32 = 0;
pub const P_LATENCY: u32 = 1;
pub const P_KEY: u32 = 2;
pub const P_FAULT: u32 = 4;
pub const P_PROCESS: u32 = 5;
/// clap-synth's.
pub const S_GAIN: u32 = 0;
pub const S_FAULT: u32 = 5;
pub const S_PROCESS: u32 = 6;
/// The VST3 gain's and pulse's.
pub const V_GAIN: u32 = 0;
pub const V_FAULT: u32 = 2;
pub const V_PROCESS: u32 = 3;
pub const PULSE_FAULT: u32 = 0;
pub const PULSE_PROCESS: u32 = 1;

/// The fault values (`clap-gain/src/fault.rs`).
pub const ABORT_PROCESS: f64 = 1.0;
pub const HANG_PROCESS: f64 = 2.0;
pub const ABORT_SAVE: f64 = 3.0;
pub const ABORT_LOAD: f64 = 4.0;
pub const ABORT_ACTIVATE: f64 = 5.0;
pub const ABORT_EDITOR: f64 = 6.0;
pub const HANG_SAVE: f64 = 7.0;
pub const HANG_LOAD: f64 = 8.0;
pub const HANG_EDITOR: f64 = 9.0;
pub const PUMP_SAVE: f64 = 10.0;
pub const SLOW_ONCE: f64 = 11.0;
pub const PUMP_WINDOW: f64 = 12.0;
pub const FAIL_SAVE_ONCE: f64 = 13.0;
pub const SEGV_PROCESS: f64 = 14.0;
pub const PUMP_WINDOW_LONG: f64 = 15.0;
pub const SEGV_EDITOR: f64 = 16.0;
pub const SLOW_SAVE: f64 = 17.0;
pub const SLOW_SECOND: f64 = 18.0;
pub const SEGV_DESTROY: f64 = 19.0;
pub const SEGV_THREAD_TIMER: f64 = 20.0;
pub const HANG_THREAD_TIMER: f64 = 21.0;
pub const RESCAN_SLOW: f64 = 22.0;
pub const RAISE_HANDLED: f64 = 23.0;
pub const HANG_HIDDEN_WINDOW: f64 = 24.0;
/// The gain's state's tag becomes `heard`, the host told nothing.
pub const TAG_QUIETLY: f64 = 25.0;
/// The Fault parameter's top (`fault::LAST` in the test plug-ins): what a normalised
/// automation value is scaled by.
pub const FAULT_LAST: f64 = 25.0;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the workspace root")
        .to_path_buf()
}

/// Tests that load into helper processes run one at a time: each is a real process tree.
pub fn serial() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/// A CLAP test plug-in's library (`package`, `stem`), built once per test process into
/// `target/test-plugins`.
fn built(package: &str, stem: &str) -> PathBuf {
    let target = workspace().join("target").join("test-plugins");
    let file = if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    };
    let path = target.join("debug").join(&file);
    if std::env::var_os("CANTUS_TEST_PLUGINS_PREBUILT").is_some() && path.is_file() {
        return path;
    }
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Process::new(cargo)
        .args(["build", "-p", package, "--target-dir"])
        .arg(&target)
        .current_dir(workspace())
        .status()
        .expect("run cargo to build a test plug-in");
    assert!(status.success(), "{package} did not build");
    path
}

/// A folder with one copy of each CLAP test plug-in.
pub fn clap_folder() -> &'static Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        // A test binary never drops its statics: copies other runs left behind an hour ago
        // and more are cleared first, so the temporary folder stays small.
        let tmp = std::env::temp_dir();
        if let Ok(entries) = std::fs::read_dir(&tmp) {
            for e in entries.flatten() {
                let old = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > std::time::Duration::from_secs(3_600));
                if old
                    && e.file_name()
                        .to_string_lossy()
                        .starts_with("cantus-test-clap-")
                {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
        let dir = tempfile::Builder::new()
            .prefix("cantus-test-clap-")
            .tempdir()
            .expect("temp dir");
        std::fs::copy(
            built("cantus-test-clap", "cantus_test_clap"),
            dir.path().join("cantus-test-gain.clap"),
        )
        .expect("copy the gain");
        std::fs::copy(
            built("cantus-test-clap-synth", "cantus_test_clap_synth"),
            dir.path().join("cantus-test-synth.clap"),
        )
        .expect("copy the synth");
        dir
    })
    .path()
}

/// The VST3 test plug-ins in a bundle of their own.
pub fn vst3_bundle() -> &'static Path {
    static BUNDLE: OnceLock<PathBuf> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        // macOS: `Contents/MacOS/CantusIsolationTest`, no extension (ADR 0066).
        let (lib, arch, ext) = if cfg!(windows) {
            ("cantus_test_vst3.dll", "x86_64-win", ".vst3")
        } else if cfg!(target_os = "macos") {
            ("libcantus_test_vst3.dylib", "MacOS", "")
        } else {
            ("libcantus_test_vst3.so", "x86_64-linux", ".so")
        };
        let beside = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|deps| deps.join(lib)))
            .filter(|p| p.is_file())
            .expect("cantus-test-vst3 is built beside the tests (a dev-dependency)");
        let bundle = workspace()
            .join("target")
            .join("cantus-test-plugins")
            .join("isolation")
            .join("CantusIsolationTest.vst3");
        let dir = bundle.join("Contents").join(arch);
        std::fs::create_dir_all(&dir).expect("bundle folders");
        std::fs::copy(&beside, dir.join(format!("CantusIsolationTest{ext}")))
            .expect("copy the VST3 test plug-in");
        bundle
    })
}

/// What a scan says of the test plug-ins (read in this process: they are well behaved).
pub fn infos() -> &'static [PluginInfo] {
    static INFOS: OnceLock<Vec<PluginInfo>> = OnceLock::new();
    INFOS.get_or_init(|| {
        let mut out = Vec::new();
        let clap = cantus_host::backend_for(Format::Clap).unwrap();
        for file in ["cantus-test-gain.clap", "cantus-test-synth.clap"] {
            out.extend(clap.scan(&clap_folder().join(file)).expect("scan a CLAP"));
        }
        let vst3 = cantus_host::backend_for(Format::Vst3).unwrap();
        out.extend(vst3.scan(vst3_bundle()).expect("scan the VST3 bundle"));
        out
    })
}

pub fn info(id: &str) -> PluginInfo {
    infos()
        .iter()
        .find(|i| i.id == id)
        .unwrap_or_else(|| panic!("{id} among {:?}", infos()))
        .clone()
}

/// This build's helper.
pub fn sandbox_exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_cantus-sandbox"))
}

pub fn pool() -> Arc<Sandboxes> {
    Sandboxes::new(sandbox_exe())
}

pub fn bridged(
    pool: &Arc<Sandboxes>,
    format: Format,
    group: &str,
    offline: bool,
) -> BridgedBackend {
    BridgedBackend {
        format,
        group: group.to_string(),
        pool: pool.clone(),
        offline,
        cancel: None,
    }
}

/// A load answered later, come in: waits (a test may) until its helper has loaded it —
/// the audio half made then replaces the first — or says why it did not.
pub fn ready(mut loaded: Loaded) -> Result<Loaded, String> {
    let t0 = std::time::Instant::now();
    loop {
        for n in loaded.main.idle() {
            match n {
                Notice::Loaded => {
                    if let Some(a) = loaded.main.take_audio() {
                        loaded.audio = a;
                    }
                    return Ok(loaded);
                }
                Notice::LoadFailed(why) => return Err(why),
                Notice::Crashed { reason, .. } => return Err(reason),
                _ => {}
            }
        }
        if loaded.main.isolated_in().is_none() {
            return Ok(loaded);
        }
        assert!(
            t0.elapsed() < std::time::Duration::from_secs(60),
            "it loads in time"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

/// [`ready`], which must load.
pub fn loaded(loaded: Loaded) -> Loaded {
    ready(loaded).expect("the plug-in loads")
}

/// Its state as it is now, asked for and waited for as a save does (its answer comes
/// later): in, or — not to be had in time — what it said last, and why. `idle` is not
/// called, so the notices stay for the test.
pub fn fresh_state(main: &mut dyn MainThread) -> StatePoll {
    main.ask_state_now();
    let t0 = std::time::Instant::now();
    while main.state_waiting() {
        assert!(
            t0.elapsed() < std::time::Duration::from_secs(60),
            "the state comes in time"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    main.poll_state()
}

/// [`fresh_state`], which must come as it is now.
pub fn state_now(main: &mut dyn MainThread) -> Vec<u8> {
    match fresh_state(main) {
        StatePoll::Fresh(s) => s,
        other => panic!("its state as it is now, not {other:?}"),
    }
}

/// Opens its editor as the host does, and waits for a window answered later (its helper
/// busy): `Ok` once it is open.
pub fn open_window(
    main: &mut dyn MainThread,
    parent: raw_window_handle::RawWindowHandle,
) -> Result<(), String> {
    match main.open_editor(parent) {
        Ok(_) => return Ok(()),
        Err(HostError::Busy(_)) => {}
        Err(e) => return Err(e.to_string()),
    }
    let t0 = std::time::Instant::now();
    loop {
        for n in main.idle() {
            match n {
                Notice::EditorOpened => return Ok(()),
                Notice::Crashed { reason, .. } => return Err(reason),
                _ => {}
            }
        }
        if t0.elapsed() > std::time::Duration::from_secs(30) {
            return Err("its window never opened".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

pub fn setup() -> AudioSetup {
    AudioSetup {
        sample_rate: SR,
        max_block: MAX_BLOCK,
    }
}

/// Deterministic noise.
pub fn noise(n: usize, seed: u32) -> Vec<f32> {
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

/// One call of the audio half; the changes and notes at their offsets.
pub fn call(
    audio: &mut dyn AudioThread,
    l: &mut [f32],
    r: &mut [f32],
    params: &[ParamChange],
    notes: &[NoteEvent],
) -> ParamOut {
    let mut out = ParamOut::new();
    audio.process(
        Audio {
            left: l,
            right: r,
            key: None,
            transport: None,
        },
        params,
        notes,
        &mut out,
    );
    out
}

/// Runs `l`/`r` through `audio` in blocks of `block`, with `params` at the first block.
pub fn run(
    audio: &mut dyn AudioThread,
    l: &mut [f32],
    r: &mut [f32],
    block: usize,
    params: &[ParamChange],
) {
    let mut at = 0;
    while at < l.len() {
        let end = (at + block).min(l.len());
        let p: &[ParamChange] = if at == 0 { params } else { &[] };
        call(audio, &mut l[at..end], &mut r[at..end], p, &[]);
        at = end;
    }
}

pub fn same_bits(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits())
}

pub fn differing(a: &[f32], b: &[f32]) -> usize {
    a.iter()
        .zip(b)
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count()
        + a.len().abs_diff(b.len())
}

/// A fault set while processing: one change at offset 0.
pub fn fault(id: u32, value: f64) -> [ParamChange; 1] {
    [ParamChange {
        offset: 0,
        id,
        value,
    }]
}

/// A plug-in host that knows the test plug-ins (their database entries written as a scan
/// would), with this build's helper.
pub fn host() -> PluginHost {
    let mut host = PluginHost::new(None, SR);
    for (path, format) in [
        (clap_folder().join("cantus-test-gain.clap"), Format::Clap),
        (clap_folder().join("cantus-test-synth.clap"), Format::Clap),
        (vst3_bundle().to_path_buf(), Format::Vst3),
    ] {
        let (modified, size) = stamp(&path).unwrap_or((0, 0));
        let plugins = infos()
            .iter()
            .filter(|i| i.path == path || i.path.starts_with(&path))
            .cloned()
            .collect();
        host.db.put(DbEntry {
            path,
            format,
            modified,
            size,
            status: ScanStatus::Ok,
            plugins,
        });
    }
    host.set_sandboxes(pool());
    host
}

/// The project, the host, the engine, and a processor the test drives as the callback.
pub struct Rig {
    pub bus: CommandBus,
    pub host: PluginHost,
    pub engine: EngineHandle,
    pub processor: Processor,
    synced: u64,
    pub notices: Vec<cantus_host::HostNotice>,
}

impl Rig {
    pub fn new(project: Project, host: PluginHost) -> Rig {
        let (engine, processor) = EngineHandle::manual(SR);
        let mut rig = Rig {
            bus: CommandBus::new(project),
            host,
            engine,
            processor,
            synced: u64::MAX,
            notices: Vec::new(),
        };
        rig.sync();
        rig.until_loaded();
        rig
    }

    pub fn sync(&mut self) {
        let rev = self.bus.revision();
        if rev == self.synced {
            return;
        }
        self.synced = rev;
        let n = self.host.sync(self.bus.project(), &mut self.engine);
        self.notices.extend(n);
        let snapshot = self.engine.compile(self.bus.project(), rev);
        self.engine.set_snapshot(snapshot);
    }

    /// The project changed outside a command (a reload): sync anyway.
    pub fn resync(&mut self) {
        self.synced = u64::MAX;
        self.sync();
    }

    pub fn exec(&mut self, cmd: Command) {
        self.bus
            .execute(cmd, Origin::User)
            .expect("command applies");
        self.sync();
    }

    /// One audio block, then what the interface does each frame.
    pub fn block(&mut self, frames: usize) -> Vec<f32> {
        let mut buf = vec![0f32; frames * 2];
        self.processor.process(&mut buf, 2);
        self.frame();
        buf
    }

    /// What the interface does each frame.
    pub fn frame(&mut self) {
        self.engine.poll();
        let n = self.host.idle(&mut self.bus, &mut self.engine);
        self.notices.extend(n);
        self.sync();
    }

    /// Frames until every isolated plug-in has loaded (its load is answered later).
    /// Blocks and frames until the hosted plug-in `id` reports `latency`, then settles. A
    /// latency set through a parameter arrives with the answer to a later call, and the
    /// host compensates it when told (`LatencyChanged`); a slow machine can tell it after
    /// `settle` (the macOS runner, round 30).
    pub fn until_latency(&mut self, id: EffectId, latency: usize) {
        let t0 = std::time::Instant::now();
        while self.host.hosted(id).map(|h| h.latency()) != Some(latency) {
            assert!(
                t0.elapsed() < std::time::Duration::from_secs(10),
                "the latency {latency} arrives"
            );
            self.block(256);
            self.frame();
        }
        self.settle();
    }

    pub fn until_loaded(&mut self) {
        let t0 = std::time::Instant::now();
        self.frame();
        while self.host.loading() {
            assert!(
                t0.elapsed() < std::time::Duration::from_secs(60),
                "the plug-ins load in time"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
            self.frame();
        }
    }

    /// Frames until the render's plug-ins are ready (`stored`: from the project's states),
    /// as an export waits for them.
    pub fn prepare_render(&mut self, stored: bool) {
        let t0 = std::time::Instant::now();
        while !self.host.render_ready(
            cantus_host::RenderFor::Export,
            self.bus.project(),
            SR,
            stored,
        ) {
            assert!(
                t0.elapsed() < std::time::Duration::from_secs(60),
                "the render's plug-ins load in time"
            );
            std::thread::sleep(std::time::Duration::from_millis(2));
            self.frame();
        }
    }

    /// Blocks and frames, then frames until every plug-in loaded (or moved) is in.
    pub fn settle(&mut self) {
        for _ in 0..64 {
            self.block(256);
        }
        self.until_loaded();
    }

    pub fn play(&mut self, frames: usize, block: usize) -> Vec<f32> {
        self.engine.seek(Ticks::ZERO);
        self.engine.play();
        let mut out = Vec::with_capacity(frames * 2);
        while out.len() / 2 < frames {
            let n = block.min(frames - out.len() / 2);
            out.extend(self.block(n));
        }
        self.engine.stop();
        out
    }

    /// Inserts `id` on `strip` as the insert menu does, running as `isolation`.
    pub fn insert(&mut self, strip: StripId, id: &str, isolation: PluginIsolation) -> EffectId {
        let info = info(id);
        let mut slot = self
            .host
            .make_slot_isolated(&mut self.bus, strip, &info, Some(isolation))
            .expect("the plug-in loads");
        if let Some(p) = slot.spec.external.as_deref_mut() {
            p.isolation = Some(isolation);
        }
        let effect = slot.id;
        let index = self
            .bus
            .project()
            .strip(strip)
            .map_or(0, |s| s.effects.len());
        self.bus
            .execute_merging(
                Command::InsertEffect {
                    strip,
                    index,
                    slot: Box::new(slot),
                },
                Origin::User,
                PluginHost::load_key(effect),
            )
            .expect("command applies");
        self.sync();
        self.until_loaded();
        effect
    }

    pub fn set_param(&mut self, strip: StripId, effect: EffectId, id: u32, value: f64) {
        self.exec(Command::SetPluginParam {
            strip,
            effect,
            id,
            value: Some(value),
        });
    }

    pub fn plugin(&self, effect: EffectId) -> &ExternalPlugin {
        self.bus
            .project()
            .all_effects()
            .find(|(_, e)| e.id == effect)
            .and_then(|(_, e)| e.spec.external.as_deref())
            .expect("a hosted slot")
    }
}

/// A riff on one track, eight beats at 120 BPM, on the first strip.
pub fn demo_project() -> Project {
    let mut p = Project::new("isolation");
    let mut lead = p.make_track("Lead");
    let mut clip = p.make_clip("Riff", Ticks::ZERO, Ticks::beats(8));
    for (i, pitch) in [60u8, 63, 67, 70, 72, 70, 67, 63].iter().enumerate() {
        let n = p.make_note(
            Ticks::QUARTER * i as i64,
            Ticks::QUARTER - Ticks(40),
            *pitch,
            90,
        );
        clip.notes.push(n);
    }
    lead.clips.push(clip);
    p.tracks.push(lead);
    p
}

/// The strip track `i` plays into.
pub fn strip_of(p: &Project, track: usize) -> StripId {
    p.tracks[track].strip.expect("track feeds a strip")
}

pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0f32, |m, s| m.max(s.abs()))
}
