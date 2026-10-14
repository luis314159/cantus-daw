//! Screenshots of round 26's Exemplar work without a screen:
//! `cargo run -p cantus-ui --example shoot_sampler -- <out-dir> [scale] [en|es]`.
//!
//! - `sampler-cc-zone-*`: the Edit view with a zone the sustain pedal starts selected — its
//!   card shows the controller and its range where the key ranges were.
//! - `sampler-loading-*`: an Exemplar whose samples are still loading in the background
//!   (a slow decoder holds them): the status line and the instrument's footer say how far.
//!
//! In Cantus Night and Paper.

use cantus_core::inst::sampler::zf;
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::inst::View;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

fn app_in(tmp: &Path, dark: bool, ctx: &egui::Context) -> CantusApp {
    let mut app = CantusApp::from_parts(
        Some(cantus_ui::welcome_project()),
        EngineHandle::dummy(),
        false,
    );
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark {
        cantus_ui::kit::appearance::Mode::Dark
    } else {
        cantus_ui::kit::appearance::Mode::Light
    };
    app.attach(ctx);
    app
}

fn wav(path: &Path) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("wav");
    for i in 0..48_000 {
        w.write_sample(((i % 200) as i16 - 100) * 50)
            .expect("sample");
    }
    w.finalize().expect("wav");
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
    let tmp = std::env::temp_dir().join(format!("cantus-shoot-sampler-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).expect("tmp");
    // Its streamed samples' cache in its own folder, never the shared fallback.
    cantus_engine::inst::stream::hub().set_cache_dir(tmp.join("cache"));
    let files: Vec<PathBuf> = (0..12)
        .map(|i| {
            let p = tmp.join(format!("note{i}.wav"));
            wav(&p);
            p
        })
        .collect();
    // The loads take a minute each: the shots see them on their way.
    cantus_engine::samples::assets::set_load_delay(std::time::Duration::from_secs(60));
    for (tn, dark) in [("night", true), ("paper", false)] {
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let name = |what: &str| dir.join(format!("sampler-{what}-{tn}-{code}-{pct}.png"));

        // A zone the pedal starts, selected, in the Edit view.
        let mut app = app_in(&tmp, dark, &ctx);
        app.demo_editor("sampler");
        let track = app.inst.open.expect("the editor is open");
        let spec = app.native_of(track).expect("an Exemplar");
        cantus_ui::views::inst::sampler::set_zone_fields(
            &mut app,
            track,
            &spec,
            0,
            &[
                (zf::TRIG, 5.0),
                (zf::CC, 64.0),
                (zf::CCLO, 126.0),
                (zf::CCHI, 127.0),
            ],
            false,
        );
        app.inst.sampler.selected = Some(0);
        app.inst.view = View::Edit;
        let path = name("cc-zone");
        shoot(
            &ctx,
            egui::vec2(1500.0, 1100.0),
            scale,
            8,
            Some(&path),
            |root| app.frame(root),
        );
        println!("wrote {}", path.display());

        // Samples on their way (a context of its own: a run keeps only its own textures).
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let mut app = app_in(&tmp, dark, &ctx);
        app.demo_editor("sampler");
        let track = app.inst.open.expect("the editor is open");
        let mut spec = cantus_core::NativeSpec::new(cantus_core::InstrumentKind::Sampler);
        for (n, f) in files.iter().enumerate() {
            let key = 48.0 + n as f32 * 2.0;
            for (k, v) in [("lo", key), ("hi", key + 1.0), ("root", key)] {
                spec.set(&format!("z{n}.{k}"), v).expect("value");
            }
            spec.set_text(&format!("z{n}.sample"), Some(f.to_string_lossy().into()))
                .expect("file");
        }
        app.set_instrument(track, cantus_core::InstrumentSpec::Native(spec));
        app.engine.samples.set_background(true);
        app.engine.samples.set_streaming(true);
        app.sync_engine();
        let path = name("loading");
        shoot(
            &ctx,
            egui::vec2(1500.0, 940.0),
            scale,
            8,
            Some(&path),
            |root| app.frame(root),
        );
        println!("wrote {}", path.display());

        // Settings, scrolled to the sample cache (a context of its own).
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let mut app = app_in(&tmp, dark, &ctx);
        let path = name("cache-settings");
        shoot(
            &ctx,
            egui::vec2(600.0, 190.0),
            scale,
            4,
            Some(&path),
            |root| {
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::NONE
                            .fill(cantus_ui::kit::Style::current().t.panel)
                            .inner_margin(12.0),
                    )
                    .show(root, |ui| {
                        cantus_ui::views::workspace_settings::sample_cache(&mut app, ui)
                    });
            },
        );
        println!("wrote {}", path.display());
    }
    cantus_engine::inst::stream::hub().let_go_unheld();
    std::thread::sleep(std::time::Duration::from_millis(100));
    let _ = std::fs::remove_dir_all(&tmp);
    std::process::exit(0);
}
