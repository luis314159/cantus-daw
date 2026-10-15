//! Screenshots of the acoustic drum kit pack (round 28: DRSKit, mixed at install) without
//! a screen: `cargo run -p cantus-ui --example shoot_drums -- <out-dir> [scale] [locale]`.
//!
//! From the bundled manifest: *Sound packs…* on DRSKit before its install and installed
//! (the credit and DrumGizmo's words), its entry in the instrument picker found by
//! "drums", and *About ▸ Credits* — in Cantus Night and Paper. With the built kit on this
//! machine (`CANTUS_DRSKIT`: a folder holding `DRSKit.sfz` and its `Samples`), also the
//! Exemplar holding it (its drums as groups, round robin in order, drum rows on), and the
//! piano roll and the channel rack on a groove, a named row per drum.

use cantus_content::{Manifest, Pack, Receipt};
use cantus_core::{InstrumentSpec, TrackIcon};
use cantus_engine::EngineHandle;
use cantus_ui::demos::song::{colors, Feel, Song};
use cantus_ui::demos::theory::Ev;
use cantus_ui::raster::shoot;
use cantus_ui::{views, CantusApp};
use std::path::{Path, PathBuf};

/// What an install leaves: the licence file and the receipt with the credit.
fn install(content: &Path, pack: &Pack) {
    let dir = content.join(&pack.id);
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join(&pack.licence_file), "CC BY 4.0").expect("licence");
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
        files: 527,
        bytes: pack.installed_size,
    };
    std::fs::write(
        dir.join(cantus_content::install::RECEIPT),
        serde_json::to_string(&r).expect("json"),
    )
    .expect("receipt");
}

/// A rock groove over four bars (the ignored real-kit test's, at its velocities).
fn groove() -> Vec<Ev> {
    let mut v = vec![Ev::new(0.0, 0.25, 49, 110)];
    for bar in 0..4 {
        let b = bar as f64 * 4.0;
        v.push(Ev::new(b, 0.25, 36, 112));
        v.push(Ev::new(b + 2.5, 0.25, 36, 100));
        v.push(Ev::new(b + 1.0, 0.25, 38, 115));
        v.push(Ev::new(b + 3.0, 0.25, 38, 116));
        for e in 0..8 {
            if bar == 1 && e == 7 {
                v.push(Ev::new(b + 3.5, 0.25, 46, 100));
            } else if bar < 3 || e < 4 {
                v.push(Ev::new(
                    b + e as f64 * 0.5,
                    0.25,
                    42,
                    if e % 2 == 0 { 92 } else { 70 },
                ));
            }
        }
    }
    for (i, k) in [48u8, 48, 47, 47, 43, 43, 41, 41].iter().enumerate() {
        v.push(Ev::new(14.0 + i as f64 * 0.25, 0.25, *k, 110));
    }
    v
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
    let bundled = Manifest::bundled();
    let pack = bundled
        .pack("drskit")
        .expect("DRSKit in the manifest")
        .clone();
    let tmp = std::env::temp_dir().join(format!("cantus-shoot-drums-{}", std::process::id()));
    let empty = tmp.join("Empty");
    let installed = tmp.join("Installed");
    std::fs::create_dir_all(&empty).expect("dir");
    install(&installed, &pack);
    // The built kit, when this machine has it: its SFZ beside the receipt, reading the
    // samples where they are.
    let real = std::env::var("CANTUS_DRSKIT").ok().map(PathBuf::from);
    if let Some(real) = &real {
        let text = std::fs::read_to_string(real.join("DRSKit.sfz")).expect("DRSKit.sfz");
        let samples = real.join("Samples").to_string_lossy().replace('\\', "/");
        let text = text.replace("default_path=Samples/", &format!("default_path={samples}/"));
        std::fs::write(installed.join("drskit").join("DRSKit.sfz"), text).expect("sfz");
    }
    let themes = [
        ("night", cantus_ui::theme::default_theme()),
        ("paper", cantus_ui::theme::paper()),
    ];
    let app_with = |root: &Path, project: Option<cantus_core::Project>| {
        let mut app = CantusApp::from_parts(
            Some(project.unwrap_or_else(cantus_ui::welcome_project)),
            EngineHandle::dummy(),
            false,
        );
        app.settings_path = tmp.join("settings.json");
        app.samples.content_root = Some(root.to_path_buf());
        app.samples.manifest = Some(bundled.clone());
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
        let mode = if *tn == "paper" {
            cantus_ui::kit::appearance::Mode::Light
        } else {
            cantus_ui::kit::appearance::Mode::Dark
        };
        let name = |what: &str| dir.join(format!("drums-{what}-{tn}-{code}-{pct}.png"));
        // Sound packs…: before the download, and installed.
        for (what, root) in [("pack-before", &empty), ("pack-installed", &installed)] {
            let mut app = app_with(root, None);
            app.samples.window.open = true;
            app.samples.window.selected = Some("drskit".into());
            let ctx = context();
            let path = name(what);
            let mut frame = 0;
            shoot(
                &ctx,
                egui::vec2(1280.0, 720.0) / scale,
                scale,
                6,
                Some(&path),
                |root| {
                    frame += 1;
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                        .show(root, |ui| {
                            // The window opens on the second frame (shoot_content's note).
                            if frame > 1 {
                                let ctx = ui.ctx().clone();
                                views::samples_content::window(&mut app, &ctx);
                            }
                        });
                },
            );
            println!("wrote {}", path.display());
        }
        // Its entry in the instrument picker, found by "drums", selected.
        {
            let mut app = app_with(&installed, None);
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
                        vec![egui::Event::Text("drums".into())]
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
                        app.picker_view.selected = Some("pack:drskit/kit".into());
                    }
                    app.frame(root);
                },
            );
            println!("wrote {}", path.display());
        }
        // About ▸ Credits with DRSKit installed.
        {
            let mut app = app_with(&installed, None);
            let ctx = context();
            views::about::show_credits(&mut app, &ctx);
            let path = name("about-credits");
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
        if real.is_none() {
            continue;
        }
        // Chosen: a new track's Exemplar holding the kit, in Play and in Edit.
        for (what, view) in [
            ("exemplar", views::inst::View::Play),
            ("exemplar-edit", views::inst::View::Edit),
        ] {
            let mut app = app_with(&installed, None);
            let ctx = context();
            app.appearance.mode = mode;
            app.attach(&ctx);
            let path = name(what);
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
                            .find(|s| s.0.pack == "drskit")
                            .expect("the kit");
                        app.apply_sound(
                            cantus_ui::picker::PickPurpose::AddInstrument { index: None },
                            &sound,
                        );
                    }
                    if frame == 3 {
                        app.inst.view = view;
                    }
                    app.frame(root);
                },
            );
            println!("wrote {}", path.display());
        }
        // The piano roll and the channel rack on a groove: a named row per drum.
        let spec = {
            let mut app = app_with(&installed, None);
            let sound = app
                .sound_choices()
                .into_iter()
                .find(|s| s.0.pack == "drskit")
                .expect("the kit");
            let imp = views::samples_content::sounds::import_of(&sound).expect("imports");
            InstrumentSpec::Native(views::samples_content::sounds::spec_of(&imp))
        };
        let mut song = Song::new("Groove", 120.0, 1);
        let part = song.part("DRSKit", spec, colors::BASS, TrackIcon::Drums);
        song.feel(part, Feel::TIGHT);
        song.clip(part, "Groove", 0.0, 4.0, &groove());
        let project = song.finish();
        for (what, rack) in [("pianoroll", false), ("rack", true)] {
            let mut app = app_with(&installed, Some(project.clone()));
            let (tid, cid) = {
                let t = &app.project().tracks[0];
                (t.id, t.clips[0].id)
            };
            app.select_clip(tid, cid);
            let ctx = context();
            let path = name(what);
            shoot(
                &ctx,
                egui::vec2(1280.0, 720.0) / scale,
                scale,
                4,
                Some(&path),
                |root| {
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE)
                        .show(root, |ui| {
                            if rack {
                                views::channel_rack::show(&mut app, ui)
                            } else {
                                views::piano_roll::show(&mut app, ui)
                            }
                        });
                },
            );
            println!("wrote {}", path.display());
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}
