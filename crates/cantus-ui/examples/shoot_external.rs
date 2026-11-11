//! External gear and control surfaces (round 36, ADR 0094, 0095), photographed without a
//! screen: Settings ▸ MIDI scrolled to the bands, with an external instrument measured and an
//! external effect not yet, a virtual Mackie Control with an extender and a HUI running.
//!
//! `cargo run -p cantus-ui --example shoot_external -- <out-dir> [filter…]`

use cantus_core::external_unit::{ExternalUnit, Measured, MidiRoute, Pair};
use cantus_core::Command;
use cantus_engine::EngineHandle;
use cantus_ui::i18n::Locale;
use cantus_ui::surface::SurfaceChoice;
use cantus_ui::{welcome_project, CantusApp};
use egui::vec2;
use std::path::PathBuf;

#[path = "shots_common/studio.rs"]
mod studio;

fn context(theme: &str) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let t = if theme == "paper" {
        cantus_ui::theme::slate_paper()
    } else {
        cantus_ui::theme::slate_night()
    };
    cantus_ui::theme::apply(&t, &ctx);
    ctx
}

fn app_for(theme: &str) -> CantusApp {
    use cantus_ui::kit::appearance::Mode;
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    let tmp = std::env::temp_dir().join("cantus-shoot-external");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let (preset, mode) = if theme == "paper" {
        ("slate_paper", Mode::Light)
    } else {
        ("slate_night", Mode::Dark)
    };
    app.appearance.mode = mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    // An instrument on the first track, measured; an effect on the second, not yet.
    let tracks: Vec<_> = app.project().tracks.iter().map(|t| t.id).collect();
    let mut inst = ExternalUnit::instrument(
        cantus_core::ExternalUnitId(900_001),
        tracks[0],
        MidiRoute {
            port: "Hardware Synth".into(),
            ordinal: 0,
            channel: 2,
        },
        Pair::stereo(2),
    );
    inst.name = "Bass synth".into();
    inst.latency = Some(Measured {
        rtt: 600,
        buffer: 256,
        device: "Studio Interface".into(),
        ..Measured::bare(612, 48_000, 41.0)
    });
    inst.trim_ms = 1.5;
    app.exec(Command::InsertExternalUnit {
        index: 0,
        unit: Box::new(inst),
    });
    if let Some(&second) = tracks.get(1) {
        let mut fx = ExternalUnit::effect(
            cantus_core::ExternalUnitId(900_002),
            second,
            Pair::stereo(0),
            Pair::stereo(0),
        );
        fx.name = "Outboard compressor".into();
        fx.return_db = -3.0;
        app.exec(Command::InsertExternalUnit {
            index: 1,
            unit: Box::new(fx),
        });
    }
    // An interface with eight ins and outs and the synth's MIDI port present.
    app.external.present_override = Some(cantus_core::external_unit::Present {
        device: Some("Studio Interface".into()),
        in_channels: 8,
        out_channels: 8,
        midi_outputs: vec!["Hardware Synth".into(), "Poly Synth".into()],
    });
    let units: Vec<_> = app.project().external_units.clone();
    for mut u in units {
        u.device = "Studio Interface".into();
        if u.kind == cantus_core::external_unit::UnitKind::Effect {
            u.send = Some(Pair::stereo(2));
        }
        app.exec(Command::SetExternalUnit { unit: Box::new(u) });
    }
    app.external.now_override = Some(cantus_core::external_unit::Now {
        device: "Studio Interface".into(),
        rtt: 600,
        buffer: 256,
        rate: 48_000,
    });
    app.settings.surfaces = vec![
        SurfaceChoice {
            kind: "mcu".into(),
            virtual_unit: true,
            ..Default::default()
        },
        SurfaceChoice {
            kind: "mcu-xt1".into(),
            virtual_unit: true,
            ..Default::default()
        },
        SurfaceChoice {
            kind: "hui".into(),
            input: "HUI port".into(),
            ..Default::default()
        },
    ];
    app
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots-external".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    for theme in ["night", "paper"] {
        for lang in [Locale::En, Locale::Es] {
            for scale in [1.0f32, 1.5] {
                let name = format!(
                    "external-{theme}-{}-{}",
                    lang.code(),
                    (scale * 100.0) as u32
                );
                if !filters.is_empty() && !filters.iter().any(|f| name.contains(f.as_str())) {
                    continue;
                }
                cantus_ui::i18n::set_locale(lang);
                let path = dir.join(format!("{name}.png"));
                let ctx = context(theme);
                let mut app = app_for(theme);
                app.attach(&ctx);
                // A frame first, so the virtual units are connected and the log has words.
                cantus_ui::surface::service::tick(&mut app);
                cantus_ui::surface::service::tick(&mut app);
                cantus_ui::raster::shoot(
                    &ctx,
                    vec2(1000.0, 560.0),
                    scale,
                    8,
                    Some(&path),
                    |root| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE.fill(cantus_ui::kit::Style::current().t.panel))
                            .show(root, |ui| {
                                cantus_ui::views::workspace_settings::external_bands::page(
                                    &mut app, ui,
                                )
                            });
                    },
                );
                // The crops the site and the README cut (`tools/shots/place.py`): the two
                // bands, the external units' and the control surfaces'.
                let bands = cantus_ui::views::workspace_settings::external_bands::last_bands(&ctx);
                studio::sidecar(
                    &path,
                    vec2(1000.0, 560.0),
                    scale,
                    &[
                        ("external", bands.map(|b| b[0])),
                        ("surfaces", bands.map(|b| b[1])),
                    ],
                );
                println!("wrote {}", path.display());
            }
        }
    }
}
