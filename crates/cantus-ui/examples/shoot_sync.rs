//! Sync (round 33, ADR 0078–0079), screenshotted without a screen:
//!
//! - `bar-<state>`: the top bar at 1920 px with the Sync group — `off`; `link` (Link on,
//!   two peers, locked); `clock` (following a MIDI clock, pulling in); `failed` (Link could
//!   not open the network);
//! - `settings-<state>`: Settings ▸ MIDI — `clock` (an input followed, two outputs sending
//!   clock and time code, an offset) and `link` (Link chosen: its quantum and start/stop).
//!
//! Each in Cantus Night and Paper, English and Spanish, at 75, 100 and 200 % (the settings
//! at 100 and 150 %).
//!
//! `cargo run -p cantus-ui --example shoot_sync -- <out-dir> [filter…]`

use cantus_engine::transport::sync::Lock;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::views::transport;
use cantus_ui::views::transport::sync_service::SyncSource;
use cantus_ui::{welcome_project, CantusApp};
use egui::vec2;
use std::path::PathBuf;
use std::sync::atomic::Ordering;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let whats = [
        "bar-off",
        "bar-link",
        "bar-clock",
        "bar-failed",
        "settings-clock",
        "settings-link",
    ];
    for what in whats {
        for theme in ["night", "paper"] {
            for lang in [Locale::En, Locale::Es] {
                let scales: &[f32] = if what.starts_with("bar") {
                    &[0.75, 1.0, 2.0]
                } else {
                    &[1.0, 1.5]
                };
                for &scale in scales {
                    let name = format!(
                        "sync-{what}-{theme}-{}-{}",
                        lang.code(),
                        (scale * 100.0) as u32
                    );
                    if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(lang);
                    let path = dir.join(format!("{name}.png"));
                    shoot(&path, what, theme, scale);
                    println!("wrote {}", path.display());
                }
            }
        }
    }
}

fn context(theme: &str) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let t = if theme == "paper" {
        cantus_ui::theme::paper()
    } else {
        cantus_ui::theme_family::cantus_night()
    };
    cantus_ui::theme::apply(&t, &ctx);
    ctx
}

fn app_for(theme: &str, what: &str) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-sync");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let (preset, mode) = if theme == "paper" {
        ("paper", Mode::Light)
    } else {
        ("cantus_night", Mode::Dark)
    };
    app.appearance.mode = mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    let shared = cantus_ui_sync(&mut app);
    let st = app.sync_service().status.clone();
    let s = &mut app.settings.sync;
    match what.split('-').nth(1).unwrap_or("") {
        "link" => {
            s.source = SyncSource::Link;
            s.start_stop = true;
            st.link_on.store(true, Ordering::Relaxed);
            st.peers.store(2, Ordering::Relaxed);
            st.link_tempo.store(124.0f64.to_bits(), Ordering::Relaxed);
            shared.set_lock(Lock::Locked);
            shared
                .tempo_bits
                .store(124.0f64.to_bits(), Ordering::Relaxed);
        }
        "clock" => {
            s.source = SyncSource::Clock;
            s.source_port = Some("MIDI Clock Master".into());
            st.receiving.store(true, Ordering::Relaxed);
            shared.set_lock(Lock::Seeking);
            shared
                .tempo_bits
                .store(97.5f64.to_bits(), Ordering::Relaxed);
            s.mtc_rate = 2;
            s.mtc_offset = "01:00:00;00".into();
            s.output_mut("Drum Machine").clock = true;
            s.output_mut("Drum Machine").offset_ms = 2.5;
            s.output_mut("Video Sync").mtc = true;
            s.output_mut("Launch Control XL").feedback = true;
            s.output_mut("Microsoft GS Wavetable Synth").feedback = false;
            s.input_mut("Launch Control XL").notes = false;
        }
        "failed" => {
            s.source = SyncSource::Link;
            st.link_failed.store(true, Ordering::Relaxed);
        }
        _ => {}
    }
    app
}

/// The service's seam for a dummy engine (the shots only).
fn cantus_ui_sync(
    app: &mut CantusApp,
) -> std::sync::Arc<cantus_engine::transport::sync::SyncShared> {
    app.sync_service_mut().demo_seam()
}

fn shoot(path: &std::path::Path, what: &str, theme: &str, scale: f32) {
    let ctx = context(theme);
    let mut app = app_for(theme, what);
    app.attach(&ctx);
    if what.starts_with("settings") {
        cantus_ui::views::workspace_settings::demo_midi(
            &mut app,
            &["Keystation 49", "MIDI Clock Master", "Launch Control XL"],
            &[
                "Drum Machine",
                "Video Sync",
                "Launch Control XL",
                "Microsoft GS Wavetable Synth",
            ],
        );
        cantus_ui::raster::shoot(
            &ctx,
            vec2(1280.0 / scale.max(1.0) * scale.max(1.0), 900.0),
            scale,
            8,
            Some(path),
            |root| app.frame(root),
        );
        return;
    }
    let size = vec2(1920.0 / scale, 54.0);
    cantus_ui::raster::shoot(&ctx, size, scale, 6, Some(path), |root| {
        egui::Panel::top(egui::Id::new("toolbar"))
            .frame(egui::Frame::NONE)
            .exact_size(transport::HEIGHT)
            .show(root, |ui| transport::show(&mut app, ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
            .show(root, |_| {});
    });
}
