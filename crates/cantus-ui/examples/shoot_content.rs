//! Screenshots of the content credits (ADR 0042, round 26) without a screen:
//! `cargo run -p cantus-ui --example shoot_content -- <out-dir> [scale] [locale]`.
//! From the CC BY fixture manifest (`cantus-content/tests/fixtures/manifest-cc-by.json`;
//! nothing is downloaded): *Sound packs…* on a CC BY pack before the download and once
//! installed, on a CC BY pack with its licensor's own line, the browser's starter offer
//! of a CC BY pack, and *About ▸ Credits*; from the bundled manifest, the Salamander Grand
//! Piano before and after its install and its entry in the instrument picker — in Cantus
//! Night and Paper.

use cantus_content::{Manifest, Pack, Receipt};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::{Path, PathBuf};

const FIXTURE: &str = include_str!("../../cantus-content/tests/fixtures/manifest-cc-by.json");

/// What an install leaves: the licence file and the receipt with the credit.
fn install(content: &Path, pack: &Pack) {
    let dir = content.join(&pack.id);
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join(&pack.licence_file), "CC BY 3.0").expect("licence");
    let r = Receipt {
        id: pack.id.clone(),
        version: pack.version,
        name: pack.name.clone(),
        licence: pack.licence.clone(),
        licence_url: pack.licence_url.clone(),
        licence_file: pack.licence_file.clone(),
        credits: pack.credits.clone(),
        attribution: pack.attribution.clone(),
        instruments: pack.instruments.clone(),
        installed_at: 0,
        files: 1,
        bytes: pack.installed_size,
    };
    std::fs::write(
        dir.join(cantus_content::install::RECEIPT),
        serde_json::to_string(&r).expect("json"),
    )
    .expect("receipt");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(1.0);
    let locale = match args.next().as_deref() {
        Some("es") => cantus_ui::i18n::Locale::Es,
        _ => cantus_ui::i18n::Locale::En,
    };
    std::fs::create_dir_all(&dir).expect("output dir");
    cantus_ui::i18n::set_locale(locale);
    let code = locale.code();
    let pct = (scale * 100.0).round() as u32;
    let manifest = Manifest::parse(FIXTURE).expect("the fixture manifest");
    let tmp = std::env::temp_dir().join(format!("cantus-shoot-content-{}", std::process::id()));
    let empty = tmp.join("Empty");
    let installed = tmp.join("Installed");
    install(
        &installed,
        manifest.pack("fixture-grand-piano").expect("the piano"),
    );
    let bundled = Manifest::bundled();
    let salamander = tmp.join("Salamander");
    install(
        &salamander,
        bundled
            .pack("salamander-grand-piano")
            .expect("the Salamander"),
    );
    let themes = [
        ("night", cantus_ui::theme::default_theme()),
        ("paper", cantus_ui::theme::paper()),
    ];
    let app_with = |root: &Path, m: &Manifest| {
        let mut app = CantusApp::from_parts(
            Some(cantus_ui::welcome_project()),
            EngineHandle::dummy(),
            false,
        );
        app.settings_path = tmp.join("settings.json");
        app.samples.content_root = Some(root.to_path_buf());
        app.samples.manifest = Some(m.clone());
        app.settings.sample_folders.clear();
        app
    };
    for (tn, theme) in &themes {
        let context = || {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(theme, &ctx);
            ctx
        };
        let name = |what: &str| dir.join(format!("content-{what}-{tn}-{code}-{pct}.png"));
        // Sound packs…: before the download, installed, and a licensor's own line.
        for (what, root, id) in [
            ("piano-before", &empty, "fixture-grand-piano"),
            ("piano-installed", &installed, "fixture-grand-piano"),
            ("choir", &empty, "fixture-choir"),
            ("cc0", &empty, "fixture-cc0-kit"),
        ] {
            let mut app = app_with(root, &manifest);
            app.samples.window.open = true;
            app.samples.window.selected = Some(id.into());
            let ctx = context();
            let path = name(what);
            // The window opens on the second frame: the first one at a scale other than
            // 100 % still has egui's provisional 5000-point screen, where the window would
            // take its default place, off the real screen.
            let mut frame = 0;
            shoot(
                &ctx,
                egui::vec2(900.0, 680.0),
                scale,
                6,
                Some(&path),
                |root| {
                    frame += 1;
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                        .show(root, |ui| {
                            if frame > 1 {
                                let ctx = ui.ctx().clone();
                                views::samples_content::window(&mut app, &ctx);
                            }
                        });
                },
            );
            println!("wrote {}", path.display());
        }
        // The Salamander Grand Piano: before its install, and installed.
        for (what, root) in [
            ("salamander-before", &empty),
            ("salamander-installed", &salamander),
        ] {
            let mut app = app_with(root, &bundled);
            app.samples.window.open = true;
            app.samples.window.selected = Some("salamander-grand-piano".into());
            let ctx = context();
            let path = name(what);
            let mut frame = 0;
            shoot(
                &ctx,
                // A 1280×720 screen at this scale: what a laptop shows.
                egui::vec2(1280.0, 720.0) / scale,
                scale,
                6,
                Some(&path),
                |root| {
                    frame += 1;
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                        .show(root, |ui| {
                            if frame > 1 {
                                let ctx = ui.ctx().clone();
                                views::samples_content::window(&mut app, &ctx);
                            }
                        });
                },
            );
            println!("wrote {}", path.display());
        }
        // Chosen: a new track's Exemplar holding it, its preset field naming it. The SFZ is
        // the real one when this machine has it unpacked (`CANTUS_SALAMANDER`).
        if let Ok(real) = std::env::var("CANTUS_SALAMANDER") {
            let sfz = "SalamanderGrandPiano-V3+20200602.sfz";
            let dest = salamander.join("salamander-grand-piano").join(sfz);
            let _ = std::fs::copy(Path::new(&real).join(sfz), &dest);
            let mut app = app_with(&salamander, &bundled);
            let ctx = context();
            app.appearance.mode = if *tn == "paper" {
                cantus_ui::kit::appearance::Mode::Light
            } else {
                cantus_ui::kit::appearance::Mode::Dark
            };
            app.attach(&ctx);
            let path = name("salamander-exemplar");
            let mut frame = 0;
            cantus_ui::raster::shoot_events(
                &ctx,
                egui::vec2(1280.0, 720.0) / scale,
                scale,
                8,
                Some(&path),
                |_, _| Vec::new(),
                |root| {
                    frame += 1;
                    if frame == 2 {
                        let sound = app
                            .sound_choices()
                            .into_iter()
                            .find(|s| s.0.instrument.id == "grand")
                            .expect("the 8-layer piano");
                        app.apply_sound(
                            cantus_ui::picker::PickPurpose::AddInstrument { index: None },
                            &sound,
                        );
                    }
                    app.frame(root);
                },
            );
            println!("wrote {}", path.display());
        }
        // Its entry in the instrument picker, found by a search, selected.
        {
            let mut app = app_with(&salamander, &bundled);
            let ctx = context();
            app.attach(&ctx);
            let path = name("picker");
            let mut frame = 0;
            cantus_ui::raster::shoot_events(
                &ctx,
                egui::vec2(1400.0, 860.0),
                scale,
                8,
                Some(&path),
                |f, _| {
                    if f == 3 {
                        vec![egui::Event::Text("salamander".into())]
                    } else {
                        Vec::new()
                    }
                },
                |root| {
                    frame += 1;
                    if frame == 2 {
                        app.open_picker(cantus_ui::picker::PickPurpose::AddInstrument {
                            index: None,
                        });
                    }
                    if frame == 5 {
                        app.picker_view.selected = Some("pack:salamander-grand-piano/grand".into());
                    }
                    app.frame(root);
                },
            );
            println!("wrote {}", path.display());
        }
        // A project whose Salamander is missing: not installed here, and installed at
        // another place (a project from another computer).
        for (what, root, base) in [
            ("missing", &empty, empty.join("salamander-grand-piano")),
            (
                "missing-elsewhere",
                &salamander,
                PathBuf::from(
                    "C:/Users/someone/AppData/Local/Cantus/Content/salamander-grand-piano",
                ),
            ),
        ] {
            let mut app = app_with(root, &bundled);
            let files: Vec<String> = ["C4v8.flac", "C4v16.flac"]
                .iter()
                .map(|f| {
                    base.join("samples")
                        .join(f)
                        .to_string_lossy()
                        .replace('\\', "/")
                })
                .collect();
            for f in ["C4v8.flac", "C4v16.flac"] {
                let p = salamander.join("salamander-grand-piano/samples").join(f);
                std::fs::create_dir_all(p.parent().expect("parent")).expect("dir");
                std::fs::write(&p, b"fLaC").expect("sample");
            }
            let mut spec = cantus_core::NativeSpec::new(cantus_core::InstrumentKind::Sampler);
            for (n, f) in files.iter().enumerate() {
                let z = cantus_core::inst::sampler::Zone::new(Some(f.clone()), 60, 72, 60);
                for (k, v) in z.values_for(n) {
                    spec.set(&k, v);
                }
                for (k, t) in z.texts_for(n) {
                    let _ = spec.set_text(&k, Some(t));
                }
            }
            for name in ["Piano", "Piano 2"] {
                let mut t = app.bus.new_track(name);
                t.instrument = cantus_core::InstrumentSpec::Native(spec.clone());
                let len = app.project().tracks.len();
                app.exec(cantus_core::Command::InsertTrack {
                    index: len,
                    track: Box::new(t),
                });
            }
            let project = app.project().clone();
            app.replace_project(project);
            // The whole window: the card over the workspace; then with *Sound packs…*
            // open from its button (the card steps aside). The scan runs on a worker:
            // frames wait for it.
            for (shot, open) in [(what.to_string(), false), (format!("{what}-window"), true)] {
                // A context of its own: a run's textures live only in that run.
                let ctx = context();
                app.appearance.mode = if *tn == "paper" {
                    cantus_ui::kit::appearance::Mode::Light
                } else {
                    cantus_ui::kit::appearance::Mode::Dark
                };
                app.attach(&ctx);
                let path = name(&shot);
                let mut frame = 0;
                cantus_ui::raster::shoot_events(
                    &ctx,
                    egui::vec2(1280.0, 720.0) / scale,
                    scale,
                    12,
                    Some(&path),
                    |_, _| Vec::new(),
                    |root| {
                        frame += 1;
                        if open && frame == 2 {
                            app.samples.window.open = true;
                            app.samples.window.selected = Some("salamander-grand-piano".into());
                        }
                        std::thread::sleep(std::time::Duration::from_millis(20));
                        app.frame(root);
                    },
                );
                println!("wrote {}", path.display());
            }
        }
        // The browser's one-click offer, when the recommended pack is CC BY.
        let mut starter = manifest.clone();
        for p in &mut starter.packs {
            p.recommended = p.id == "fixture-grand-piano";
        }
        let mut app = app_with(&empty, &starter);
        let ctx = context();
        let path = name("starter");
        shoot(
            &ctx,
            egui::vec2(300.0, 420.0),
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
        // About ▸ Credits, scrolled to them; and the top of About with its Credits button.
        for (what, to_credits) in [("about-credits", true), ("about-top", false)] {
            let mut app = app_with(&installed, &manifest);
            let ctx = context();
            if to_credits {
                views::about::show_credits(&mut app, &ctx);
            } else {
                app.show_about = true;
            }
            let path = name(what);
            shoot(
                &ctx,
                egui::vec2(760.0, 820.0),
                scale,
                10,
                Some(&path),
                |root| {
                    let ctx = root.ctx().clone();
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                        .show(root, |_| {});
                    let _ = views::about::show(&mut app, &ctx);
                },
            );
            println!("wrote {}", path.display());
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
