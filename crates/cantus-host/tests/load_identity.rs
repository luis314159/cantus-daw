//! Which load of a hosted plug-in this is (`Hosted::load_id`, ADR 0073, second review #1):
//! a restart — in place (CLAP) or by a new instance where the backend has no restart of its
//! own (VST3, on `kLatencyChanged`) — is the same load, so the interface's state of the
//! instance (its view, its A/B, its frame) survives a latency change; a reload is another.

mod support;
use cantus_core::*;
use cantus_host::{Backend, PluginInfo};
use std::path::{Path, PathBuf};
use std::process::Command as Process;
use std::sync::OnceLock;
use support::*;

const VST3_GAIN_ID: &str = "43414E545553544741494E50524F4331";
const VST3_LATENCY: u32 = 1;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The VST3 test gain as a bundle of this test's own (the backend tests build theirs).
fn vst3_bundle() -> &'static Path {
    static BUNDLE: OnceLock<PathBuf> = OnceLock::new();
    BUNDLE.get_or_init(|| {
        let (built, arch, ext) = if cfg!(target_os = "windows") {
            ("cantus_test_vst3.dll", "x86_64-win", ".vst3")
        } else if cfg!(target_os = "macos") {
            ("libcantus_test_vst3.dylib", "MacOS", "")
        } else {
            ("libcantus_test_vst3.so", "x86_64-linux", ".so")
        };
        let target = workspace().join("target").join("cantus-test-plugins");
        let beside_tests = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|deps| deps.join(built)))
            .filter(|p| p.is_file());
        let source = beside_tests.unwrap_or_else(|| {
            let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
            let ok = Process::new(cargo)
                .current_dir(workspace())
                .args(["build", "--quiet", "-p", "cantus-test-vst3", "--target-dir"])
                .arg(&target)
                .status()
                .expect("cargo builds the test plug-in");
            assert!(ok.success());
            target.join("debug").join(built)
        });
        let bundle = target
            .join("bundles-identity")
            .join("CantusTestIdentity.vst3");
        let dir = bundle.join("Contents").join(arch);
        std::fs::create_dir_all(&dir).expect("bundle folders");
        std::fs::copy(&source, dir.join(format!("CantusTestIdentity{ext}")))
            .expect("copy the plug-in into its bundle");
        bundle
    })
}

fn vst3_gain() -> PluginInfo {
    cantus_host::vst3::Vst3Backend::new()
        .scan(vst3_bundle())
        .expect("scan")
        .into_iter()
        .find(|i| i.id == VST3_GAIN_ID)
        .expect("the gain is in the bundle")
}

fn insert(rig: &mut Rig, strip: StripId, info: &PluginInfo) -> EffectId {
    let slot = rig
        .host
        .make_slot(&mut rig.bus, strip, info)
        .expect("the plug-in loads");
    let id = slot.id;
    let index = rig
        .bus
        .project()
        .strip(strip)
        .map_or(0, |s| s.effects.len());
    rig.exec(Command::InsertEffect {
        strip,
        index,
        slot: Box::new(slot),
    });
    id
}

fn load_id(rig: &Rig, effect: EffectId) -> u64 {
    rig.host.hosted(effect).expect("loaded").load_id()
}

#[test]
fn a_restart_for_latency_is_the_same_load_and_a_reload_another() {
    let project = demo_project();
    let strip = strip_of(&project, 0);
    let mut rig = Rig::new(project, cantus_host::PluginHost::new(None, SR));
    // VST3: the backend restarts by a new instance (it has no restart in place).
    let vst3 = insert(&mut rig, strip, &vst3_gain());
    rig.settle();
    let before = load_id(&rig, vst3);
    rig.set_param(strip, vst3, VST3_LATENCY, 256.0);
    rig.settle();
    assert!(!rig.host.restarting(vst3));
    assert_eq!(
        rig.host.hosted(vst3).unwrap().latency(),
        256,
        "restarted with its new latency"
    );
    assert_eq!(load_id(&rig, vst3), before, "a restart is the same load");
    // A reload is another load.
    let project = rig.bus.project().clone();
    rig.host
        .reload(&project, &mut rig.engine, vst3)
        .expect("reloads");
    rig.settle();
    assert_ne!(load_id(&rig, vst3), before, "a reload is another");
    // CLAP: a restart in place, the same load.
    let mut host = host_with_plugin(shared_folder());
    std::mem::swap(&mut rig.host, &mut host);
    drop(host);
    let clap = rig.insert_plugin(strip);
    rig.settle();
    let before = load_id(&rig, clap);
    rig.set_param(strip, clap, P_LATENCY, 100.0);
    rig.settle();
    assert_eq!(rig.host.hosted(clap).unwrap().latency(), 100);
    assert_eq!(load_id(&rig, clap), before);
}
