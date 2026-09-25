//! What the host's tests share: the test plug-in (`crates/cantus-test-plugins/clap-gain`),
//! built from source on first use; folders of copies of it; and a rig that runs a project
//! through the whole host — `PluginHost`, the engine handle, a processor this test drives
//! in place of the audio callback, and the command bus.
#![allow(dead_code)]

use cantus_core::*;
use cantus_engine::{EngineHandle, Processor};
use cantus_host::{PluginHost, PluginInfo, Scanner};
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::OnceLock;
use std::time::Duration;

pub const PLUGIN_ID: &str = "org.cantus.test.gain";
pub const P_GAIN: u32 = 0;
pub const P_LATENCY: u32 = 1;
pub const P_KEY: u32 = 2;
pub const P_GESTURE: u32 = 3;
pub const SR: f64 = 48_000.0;

/// The test plug-in's shared library, built once per test process, into its own target
/// folder so the build does not wait on the one running the tests.
pub fn test_plugin_dll() -> PathBuf {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT
        .get_or_init(|| {
            let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("the workspace root")
                .to_path_buf();
            let target = workspace.join("target").join("test-plugins");
            let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
            let status = Process::new(cargo)
                .args(["build", "-p", "cantus-test-clap", "--target-dir"])
                .arg(&target)
                .current_dir(&workspace)
                .status()
                .expect("run cargo to build the test plug-in");
            assert!(status.success(), "the test plug-in did not build");
            let file = if cfg!(windows) {
                "cantus_test_clap.dll"
            } else if cfg!(target_os = "macos") {
                "libcantus_test_clap.dylib"
            } else {
                "libcantus_test_clap.so"
            };
            target.join("debug").join(file)
        })
        .clone()
}

/// A fresh folder with copies of the test plug-in under `names`.
pub fn plugin_folder(names: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    for name in names {
        std::fs::copy(test_plugin_dll(), dir.path().join(name)).expect("copy the plug-in");
    }
    dir
}

/// A scanner that runs this build's `cantus-scan` and looks only where it is told.
pub fn scanner(timeout: Duration) -> Scanner {
    Scanner {
        exe: PathBuf::from(env!("CARGO_BIN_EXE_cantus-scan")),
        timeout,
        standard_folders: false,
    }
}

/// A host that knows the test plug-in in `folder`.
pub fn host_with_plugin(folder: &Path) -> PluginHost {
    let mut host = PluginHost::new(None, SR);
    host.scanner = scanner(Duration::from_secs(60));
    host.db.folders.push(folder.to_path_buf());
    let report = host.scan_now(false);
    assert_eq!(report.plugins, 1, "{report:?} {:?}", host.db);
    host
}

pub fn plugin_info(host: &PluginHost) -> PluginInfo {
    host.db
        .find(cantus_host::Format::Clap, PLUGIN_ID)
        .expect("the test plug-in is in the database")
        .clone()
}

/// The folder every test of a process shares, with one copy of the plug-in.
pub fn shared_folder() -> &'static Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| plugin_folder(&["cantus-test.clap"]))
        .path()
}

/// A riff on one track, eight beats at 120 BPM, on the first strip.
pub fn demo_project() -> Project {
    let mut p = Project::new("host");
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

/// The largest absolute sample.
pub fn peak(samples: &[f32]) -> f32 {
    samples.iter().fold(0.0, |m, x| m.max(x.abs()))
}

pub fn strip_of(p: &Project, track: usize) -> StripId {
    p.tracks[track].strip.expect("track feeds a strip")
}

/// The whole host around a project: the bus, the plug-in host, the engine handle and the
/// processor, which the test drives as the audio callback would.
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

    /// What the application does when the project changed: plug-ins first, then the
    /// snapshot.
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

    pub fn exec(&mut self, cmd: Command) {
        self.bus
            .execute(cmd, Origin::User)
            .expect("command applies");
        self.sync();
    }

    /// One audio block, then what the UI does each frame.
    pub fn block(&mut self, frames: usize) -> Vec<f32> {
        let mut buf = vec![0f32; frames * 2];
        self.processor.process(&mut buf, 2);
        self.engine.poll();
        let n = self.host.idle(&mut self.bus, &mut self.engine);
        self.notices.extend(n);
        self.sync();
        buf
    }

    /// Blocks with the transport stopped, until the host has settled (restarts done).
    pub fn settle(&mut self) {
        for _ in 0..64 {
            self.block(256);
        }
    }

    /// Play from the start for `frames`, at `block` frames per call.
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

    /// Insert the test plug-in on `strip`; returns the slot's id.
    pub fn insert_plugin(&mut self, strip: StripId) -> EffectId {
        let info = plugin_info(&self.host);
        let slot = self
            .host
            .make_slot(&mut self.bus, strip, &info)
            .expect("the plug-in loads");
        let id = slot.id;
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
        id
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
            .strips
            .iter()
            .find_map(|s| s.effect(effect))
            .and_then(|e| e.spec.external.as_deref())
            .expect("a hosted slot")
    }
}

/// Render a project with no plug-in host at all (the engine's own offline render), for
/// comparison.
pub fn render_plain(project: &Project, frames: usize) -> Vec<f32> {
    let (engine, processor) = EngineHandle::manual(SR);
    let mut rig = Rig {
        bus: CommandBus::new(project.clone()),
        host: PluginHost::new(None, SR),
        engine,
        processor,
        synced: u64::MAX,
        notices: Vec::new(),
    };
    rig.sync();
    rig.settle();
    rig.play(frames, 256)
}
