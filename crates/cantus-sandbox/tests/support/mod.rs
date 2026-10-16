//! What the isolation tests share: the test plug-ins built from source (the CLAP gain and
//! synth, the VST3 gain and pulse), this build's `cantus-sandbox`, backends that load into
//! it, and a rig that runs a project through the whole host and engine.
#![allow(dead_code)]

use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_host::bridge::{BridgedBackend, Sandboxes};
use cantus_host::db::{stamp, DbEntry, ScanStatus};
use cantus_host::{
    Audio, AudioSetup, AudioThread, Format, NoteEvent, ParamChange, ParamOut,
    PluginHost, PluginInfo,
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
        let dir = tempfile::tempdir().expect("temp dir");
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
        let (lib, arch, ext) = if cfg!(windows) {
            ("cantus_test_vst3.dll", "x86_64-win", "vst3")
        } else {
            ("libcantus_test_vst3.so", "x86_64-linux", "so")
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
        std::fs::copy(&beside, dir.join(format!("CantusIsolationTest.{ext}")))
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

pub fn bridged(pool: &Arc<Sandboxes>, format: Format, group: &str, offline: bool) -> BridgedBackend {
    BridgedBackend {
        format,
        group: group.to_string(),
        pool: pool.clone(),
        offline,
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

    pub fn settle(&mut self) {
        for _ in 0..64 {
            self.block(256);
        }
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
        self.exec(Command::InsertEffect {
            strip,
            index,
            slot: Box::new(slot),
        });
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
