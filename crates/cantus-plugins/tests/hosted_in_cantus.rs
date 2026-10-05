//! The round trip of ADRs 0019 and 0020: our own effects, built as a CLAP plug-in, loaded
//! by Cantus's own CLAP host exactly as a third party's plug-in is — scanned from a file,
//! loaded, given its state — sound the same as the native insert the engine runs, to the
//! bit. It proves both ends at once: the export wraps the DSP without changing it, and the
//! host drives a plug-in without changing what it hears or says.

mod common;

use cantus_host::{Audio, AudioSetup, Backend, Notice, ParamOut};
use common::*;
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::OnceLock;

const BLOCK: usize = 512;

/// The plug-in's shared library, built once per test process into its own target folder
/// (so the build does not wait on the one running the tests), without the editor.
fn plugin_dll() -> PathBuf {
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
                .args(["build", "-p", "cantus-plugins", "--no-default-features"])
                .args(["--features", "all", "--target-dir"])
                .arg(&target)
                .current_dir(&workspace)
                .status()
                .expect("run cargo to build the plug-ins");
            assert!(status.success(), "the plug-ins did not build");
            let file = if cfg!(windows) {
                "cantus_plugins.dll"
            } else if cfg!(target_os = "macos") {
                "libcantus_plugins.dylib"
            } else {
                "libcantus_plugins.so"
            };
            target.join("debug").join(file)
        })
        .clone()
}

#[test]
fn every_exported_plugin_hosted_by_cantus_sounds_as_the_native_insert() {
    let dir = tempfile::tempdir().expect("temp dir");
    let clap = dir.path().join("Cantus.clap");
    std::fs::copy(plugin_dll(), &clap).expect("copy the plug-in");

    let backend = cantus_host::clap::ClapBackend;
    assert!(backend.is_candidate(&clap));
    let infos = backend
        .scan(&clap)
        .expect("Cantus's host reads our plug-in file");
    assert_eq!(infos.len(), kinds().len(), "one descriptor per effect");

    let (in_l, in_r) = signal(48_000, 7);
    for info in &infos {
        let kind = cantus_plugins::desc::by_clap_id(&info.id)
            .unwrap_or_else(|| panic!("{} is not one of ours", info.id))
            .kind;
        let spec = spec_with(kind, &settings(kind));

        // Hosted: through the file, the scan, the host's load, and the state a project
        // would give it.
        let loaded = backend
            .load(
                info,
                AudioSetup {
                    sample_rate: SR as f64,
                    max_block: BLOCK,
                },
            )
            .unwrap_or_else(|e| panic!("{}: {e}", info.name));
        let (mut main, mut audio) = (loaded.main, loaded.audio);
        let state = serde_json::to_vec(&spec).unwrap();
        main.load_state(&state)
            .unwrap_or_else(|e| panic!("{}: {e}", info.name));
        // A state with another latency (Fervor's oversampling, say) makes the plug-in ask
        // for a restart from its first call, as CLAP wants (the latency changes only in
        // `activate`), and the session restarts it. Here: one block to let it ask, the
        // restart a session makes, and the measured run from a fresh start.
        let mut out = ParamOut::new();
        let (mut pl, mut pr) = (in_l[..BLOCK].to_vec(), in_r[..BLOCK].to_vec());
        let probe = Audio {
            left: &mut pl,
            right: &mut pr,
            key: None,
            transport: None,
        };
        audio.process(probe, &[], &[], &mut out);
        let asked = main
            .idle()
            .iter()
            .any(|n| matches!(n, Notice::RestartNeeded | Notice::LatencyChanged(_)));
        let fresh = engine_unit(&cantus_core::EffectSpec::new(kind)).latency();
        if engine_unit(&spec).latency() != fresh {
            assert!(asked, "{}: a new latency asks for a restart", info.name);
        }
        drop(audio);
        audio = main
            .restart()
            .unwrap_or_else(|e| panic!("{}: {e}", info.name))
            .expect("a new audio half");
        let (mut hl, mut hr) = (in_l.clone(), in_r.clone());
        for (l, r) in hl.chunks_mut(BLOCK).zip(hr.chunks_mut(BLOCK)) {
            audio.process(
                Audio {
                    left: l,
                    right: r,
                    key: None,
                    transport: None,
                },
                &[],
                &[],
                &mut out,
            );
        }
        audio.stop();
        assert_eq!(
            main.latency(),
            engine_unit(&spec).latency(),
            "{}: the hosted plug-in reports the native latency",
            info.name
        );
        drop(audio);
        drop(main);

        // Native: the insert Cantus runs for the same spec.
        let mut unit = engine_unit(&spec);
        let (mut nl, mut nr) = (in_l.clone(), in_r.clone());
        for (l, r) in nl.chunks_mut(BLOCK).zip(nr.chunks_mut(BLOCK)) {
            unit.process(l, r);
        }

        assert_identical(&format!("{} left", info.name), &hl, &nl);
        assert_identical(&format!("{} right", info.name), &hr, &nr);
    }
}
