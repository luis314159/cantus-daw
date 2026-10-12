//! `shoot <dir> samples`: the sample browser with a pack open, a sample selected and
//! playing, and the user's folder; the browser's starter offer when nothing is installed;
//! the Sound packs window with the starter installed. Packs are written into a temporary
//! folder; nothing is downloaded.

use cantus_content::manifest::{Manifest, Text};
use cantus_content::Receipt;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::Path;
use std::time::{Duration, Instant};

fn wav(path: &Path) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("wav");
    for i in 0..441 {
        w.write_sample(((i as f32 * 0.07).sin() * 9000.0) as i16)
            .expect("sample");
    }
    w.finalize().expect("finalize");
}

/// The starter pack's own layout, installed with a receipt.
fn install_fake_starter(content: &Path) {
    let m = Manifest::bundled();
    let pack = m.recommended().expect("a starter");
    let dir = content.join(&pack.id);
    if let cantus_content::Source::Files(files) = &pack.source {
        for f in files {
            let p = dir.join(&f.to);
            if f.to.starts_with("Licences/") {
                std::fs::create_dir_all(p.parent().expect("parent")).expect("dir");
                std::fs::write(&p, "CC0 1.0 Universal").expect("licence");
            } else {
                wav(&p);
            }
        }
    }
    let r = Receipt {
        id: pack.id.clone(),
        version: pack.version,
        name: pack.name.clone(),
        licence: pack.licence.clone(),
        licence_url: pack.licence_url.clone(),
        licence_file: pack.licence_file.clone(),
        credits: pack.credits.clone(),
        installed_at: 0,
        files: 81,
        bytes: pack.installed_size,
    };
    std::fs::write(
        dir.join(cantus_content::install::RECEIPT),
        serde_json::to_string(&r).expect("json"),
    )
    .expect("receipt");
    // A second small pack, to show more than one root.
    let afr = content.join("afr-kicks-2024");
    for n in [
        "606 classic.wav",
        "808 classic.wav",
        "909 classic.wav",
        "deephouse classic.wav",
    ] {
        wav(&afr.join(n));
    }
    std::fs::write(afr.join("CC0-license.txt"), "CC0").expect("licence");
    let r = Receipt {
        id: "afr-kicks-2024".into(),
        name: Text {
            en: "AFR Kicks 2024".into(),
            es: "AFR Kicks 2024".into(),
        },
        ..r
    };
    std::fs::write(
        afr.join(cantus_content::install::RECEIPT),
        serde_json::to_string(&r).expect("json"),
    )
    .expect("receipt");
}

pub fn shots(out: &Path, scale: f32) {
    let tmp = std::env::temp_dir().join(format!("cantus-shoot-samples-{}", std::process::id()));
    let content = tmp.join("Content");
    let empty = tmp.join("Empty");
    let user = tmp.join("My Sounds");
    install_fake_starter(&content);
    for n in ["Vocal chop 1.wav", "Vocal chop 2.wav", "Riser.wav"] {
        wav(&user.join("Chops").join(n));
    }
    wav(&user.join("Guitar stab.wav"));
    let themes = [
        ("night", cantus_ui::theme::default_theme()),
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
    let app_with = |root: &Path| {
        let mut app = CantusApp::from_parts(
            Some(cantus_ui::welcome_project()),
            EngineHandle::dummy(),
            false,
        );
        app.settings_path = tmp.join("settings.json");
        app.samples.content_root = Some(root.to_path_buf());
        app.settings.sample_folders = vec![user.clone()];
        app
    };
    for (tn, theme) in &themes {
        // The browser: the starter open at its kicks and snares, a kick playing.
        let mut app = app_with(&content);
        let starter = content.join("cantus-starter-drums");
        for p in [
            starter.clone(),
            starter.join("Kicks"),
            starter.join("Snares"),
            user.clone(),
        ] {
            app.samples.open.insert(p);
        }
        let kick = starter.join("Kicks/Acoustic Kick hard 1.flac");
        app.samples.selected = Some(kick.clone());
        app.samples.preview.playing = Some((kick, Instant::now(), Duration::from_secs(600)));
        let path = out.join(format!("samples-browser-{tn}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(300.0, 820.0),
            scale,
            4,
            Some(&path),
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::samples_browser::show(&mut app, ui));
            },
        );
        println!("wrote {}", path.display());

        // The browser with a search typed.
        app.samples.query = "kick".into();
        let path = out.join(format!("samples-search-{tn}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(300.0, 520.0),
            scale,
            40,
            Some(&path),
            |root| {
                std::thread::sleep(Duration::from_millis(10));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::samples_browser::show(&mut app, ui));
            },
        );
        println!("wrote {}", path.display());

        // Nothing installed: the one-click offer.
        let mut app = app_with(&empty);
        app.settings.sample_folders.clear();
        let path = out.join(format!("samples-starter-{tn}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(300.0, 520.0),
            scale,
            4,
            Some(&path),
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| views::samples_browser::show(&mut app, ui));
            },
        );
        println!("wrote {}", path.display());

        // The Sound packs window, the starter installed and selected.
        let mut app = app_with(&content);
        app.samples.window.open = true;
        let path = out.join(format!("samples-content-{tn}.png"));
        let ctx = context(theme);
        shoot(
            &ctx,
            egui::vec2(900.0, 680.0),
            scale,
            4,
            Some(&path),
            |root| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                    .show(root, |ui| {
                        let ctx = ui.ctx().clone();
                        views::samples_content::window(&mut app, &ctx);
                    });
            },
        );
        println!("wrote {}", path.display());
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
