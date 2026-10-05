//! The test instrument (`crates/cantus-test-plugins/clap-synth`, ADR 0028): built from
//! source on first use, a folder with a copy of it, a host that knows it, and the rig's
//! instrument helpers.

use super::{scanner, Rig};
use cantus_core::*;
use cantus_host::{PluginHost, PluginInfo};
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::OnceLock;
use std::time::Duration;

pub const SYNTH_ID: &str = "org.cantus.test.synth";
pub const S_GAIN: u32 = 0;
pub const S_LATENCY: u32 = 1;
pub const S_TEMPO: u32 = 2;
pub const S_PLAYING: u32 = 3;
pub const S_GESTURE: u32 = 4;

/// The test synth's shared library, built once per test process.
pub fn synth_dll() -> PathBuf {
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
                .args(["build", "-p", "cantus-test-clap-synth", "--target-dir"])
                .arg(&target)
                .current_dir(&workspace)
                .status()
                .expect("run cargo to build the test synth");
            assert!(status.success(), "the test synth did not build");
            let file = if cfg!(windows) {
                "cantus_test_clap_synth.dll"
            } else if cfg!(target_os = "macos") {
                "libcantus_test_clap_synth.dylib"
            } else {
                "libcantus_test_clap_synth.so"
            };
            target.join("debug").join(file)
        })
        .clone()
}

/// The folder every test of a process shares, with one copy of the synth.
pub fn synth_folder() -> &'static Path {
    static DIR: OnceLock<tempfile::TempDir> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = tempfile::tempdir().expect("temp dir");
        std::fs::copy(synth_dll(), dir.path().join("cantus-test-synth.clap"))
            .expect("copy the synth");
        dir
    })
    .path()
}

/// A host that knows the test synth.
pub fn host_with_synth() -> PluginHost {
    let mut host = PluginHost::new(None, super::SR);
    host.scanner = scanner(Duration::from_secs(60));
    host.db.folders.push(synth_folder().to_path_buf());
    let report = host.scan_now(false);
    assert_eq!(report.plugins, 1, "{report:?} {:?}", host.db);
    host
}

pub fn synth_info(host: &PluginHost) -> PluginInfo {
    host.db
        .find(cantus_host::Format::Clap, SYNTH_ID)
        .expect("the test synth is in the database")
        .clone()
}

/// One track: a C major chord on beat 2 (one beat long), then E4 alone on beat 4, at
/// 120 BPM — so the first sound is exactly at sample 24 000.
pub fn chord_project() -> Project {
    let mut p = Project::new("instrument");
    let mut keys = p.make_track("Keys");
    let mut clip = p.make_clip("Chord", Ticks::ZERO, Ticks::beats(8));
    for pitch in [60u8, 64, 67] {
        let n = p.make_note(Ticks::beats(1), Ticks::QUARTER, pitch, 100);
        clip.notes.push(n);
    }
    let n = p.make_note(Ticks::beats(3), Ticks::QUARTER, 64, 80);
    clip.notes.push(n);
    keys.clips.push(clip);
    p.tracks.push(keys);
    p
}

impl Rig {
    /// Put the test synth on `track` (one command), as the picker does.
    pub fn set_synth(&mut self, track: TrackId) {
        let info = synth_info(&self.host);
        let spec = self
            .host
            .make_instrument(track, &info)
            .expect("the synth loads");
        self.exec(Command::SetInstrument {
            track,
            spec: Box::new(spec),
        });
    }

    pub fn set_inst_param(&mut self, track: TrackId, id: u32, value: f64) {
        self.exec(Command::SetInstrumentPluginParam {
            track,
            id,
            value: Some(value),
        });
    }

    pub fn instrument(&self, track: TrackId) -> &ExternalPlugin {
        self.bus
            .project()
            .track(track)
            .and_then(|t| t.instrument.external())
            .expect("a hosted instrument")
    }
}

/// Bit-level differences between two renders.
pub fn differing(a: &[f32], b: &[f32]) -> usize {
    a.iter()
        .zip(b)
        .filter(|(x, y)| x.to_bits() != y.to_bits())
        .count()
        + a.len().abs_diff(b.len())
}
