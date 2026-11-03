//! `shoot <dir> ara`: ARA 2 on audio clips (round 36, ADR 0097) — the arrangement with a
//! clip bound to the repository's ARA test plug-in (live) and one bound to a plug-in that
//! is not installed (playing its source), and the clip menu's ARA section, bound and not,
//! in both themes.

use cantus_core::ara::AraClip;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The ARA test plug-in, built and bundled.
fn bundle() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = root.join("target").join("cantus-test-plugins");
    let (built, arch, ext) = if cfg!(windows) {
        ("cantus_test_ara.dll", "x86_64-win", ".vst3")
    } else if cfg!(target_os = "macos") {
        ("libcantus_test_ara.dylib", "MacOS", "")
    } else {
        ("libcantus_test_ara.so", "x86_64-linux", ".so")
    };
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let _ = std::process::Command::new(cargo)
        .args(["build", "-p", "cantus-test-ara", "--target-dir"])
        .arg(&target)
        .current_dir(&root)
        .status();
    let bundle = target.join("bundles-ara-shoot").join("CantusTestARA.vst3");
    let dir = bundle.join("Contents").join(arch);
    std::fs::create_dir_all(&dir).expect("bundle");
    std::fs::copy(
        target.join("debug").join(built),
        dir.join(format!("CantusTestARA{ext}")),
    )
    .expect("copy");
    bundle
}

pub fn shots(out: &Path, scale: f32) {
    use cantus_host::Backend;
    let tmp = std::env::temp_dir().join(format!("cantus-shoot-ara-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("temp dir");
    let info = cantus_host::vst3::Vst3Backend::new()
        .scan(&bundle())
        .expect("scan")
        .remove(0);
    let themes = [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    let context = |theme: &cantus_ui::theme::Theme| {
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(theme, &ctx);
        ctx
    };
    for (tn, theme) in &themes {
        let mut p = crate::audio_scene::project(&tmp);
        // The vocal bound to the test plug-in; the drum loop to one not installed here.
        let find = |name: &str| {
            p.tracks
                .iter()
                .position(|t| t.name == name)
                .map(|i| (i, p.tracks[i].clips[0].id))
        };
        let (vi, vocal) = find("Lead vocal").expect("vocal");
        let (di, _) = find("Drum loop").expect("drums");
        let mut ara = AraClip::new(cantus_host::ara::plugin_ref(
            &info,
            "org.cantus.test.ara.factory",
        ));
        ara.binding = 1;
        p.tracks[vi].clips[0].audio.as_mut().unwrap().ara = Some(Box::new(ara));
        let mut gone = AraClip::new(cantus_core::ara::AraPlugin {
            id: "0123456789ABCDEF0123456789ABCDEF".into(),
            name: "Pitch Editor".into(),
            vendor: "Elsewhere".into(),
            path: "missing.vst3".into(),
            factory_id: "com.elsewhere.pitch".into(),
        });
        gone.binding = 2;
        p.tracks[di].clips[0].audio.as_mut().unwrap().ara = Some(Box::new(gone));
        let vocal_track = p.tracks[vi].id;
        let drum_ref = (p.tracks[di].id, p.tracks[di].clips[0].id);
        let pad_ref = p
            .tracks
            .iter()
            .find(|t| t.name == "Pad")
            .map(|t| (t.id, t.clips[0].id))
            .expect("pad");
        let mut app = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
        app.plugins.db.put(cantus_host::DbEntry {
            path: info.path.clone(),
            format: cantus_host::Format::Vst3,
            modified: 1,
            size: 1,
            status: cantus_host::ScanStatus::Ok,
            plugins: vec![info.clone()],
        });
        for s in app.project().audio_pool.clone() {
            let _ = app.peaks.lookup(&s, app.audio_base(), None);
        }
        app.peaks.wait_idle(Duration::from_secs(30));
        for _ in 0..3 {
            app.sync_engine();
            app.plugins.ara.wait(Duration::from_secs(20));
        }
        app.select_clip(vocal_track, vocal);
        app.arrangement.px_per_beat = 34.0;
        app.arrangement.track_height = 52.0;
        let path = out.join(format!("ara-clips-{tn}.png"));
        shoot(
            &context(theme),
            egui::vec2(1500.0, 420.0),
            scale,
            4,
            Some(&path),
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::arrangement::show(&mut app, ui));
            },
        );
        println!("wrote {}", path.display());
        for (name, r) in [
            ("bound", (vocal_track, vocal)),
            ("missing", drum_ref),
            ("unbound", pad_ref),
        ] {
            let path = out.join(format!("ara-menu-{name}-{tn}.png"));
            shoot(
                &context(theme),
                egui::vec2(260.0, 220.0),
                scale,
                4,
                Some(&path),
                |root| {
                    egui::CentralPanel::default()
                        .show(root, |ui| views::ara_menu::clip_menu(&mut app, ui, r));
                },
            );
            println!("wrote {}", path.display());
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
