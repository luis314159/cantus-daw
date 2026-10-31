//! Screenshots of the mastering assistant (ADR 0084) without a screen:
//! `cargo run -p cantus-ui --example shoot_mastering -- <out-dir> [scale] [locale]`.
//! The mixer with the panel over it, in Slate and Paper: just opened, listening, and with
//! its proposal (the numbers, the balance drawn, the chain and why, Apply and the A/B) —
//! the proposal made by the panel's own worker on the welcome song.

use cantus_engine::EngineHandle;
use cantus_ui::ai_tools::listen::panel;
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

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
    for kind in ["open", "list", "listening", "proposal", "proposal-end"] {
        for (theme_name, theme) in [
            ("slate", cantus_ui::theme::slate()),
            ("paper", cantus_ui::theme::paper()),
        ] {
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            cantus_ui::theme::apply(&theme, &ctx);
            let mut project = welcome_project();
            // A master chain of its own — six inserts, so *Apply* must name them without
            // overflowing (review 2) — and *Replace it / Add after it* shows.
            for kind in [
                cantus_core::EffectKind::Eq,
                cantus_core::EffectKind::Compressor,
                cantus_core::EffectKind::Saturator,
                cantus_core::EffectKind::Stereo,
                cantus_core::EffectKind::Multiband,
                cantus_core::EffectKind::Limiter,
            ] {
                let fx = project.make_effect(kind);
                project.master.effects.push(fx);
            }
            let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
            panel::open(&mut app);
            match kind {
                "list" => {
                    app.listen.panel.menu = Some((
                        panel::Pick::Platform,
                        egui::pos2(1440.0 - 620.0 * scale.min(1.0) + 140.0, 150.0),
                    ));
                }
                "listening" => {
                    panel::start(&mut app);
                }
                "proposal" | "proposal-end" => {
                    panel::start(&mut app);
                    // The worker's answer, as the frames collect it.
                    for _ in 0..6000 {
                        if app.listen.panel.done.is_some() || app.listen.panel.error.is_some() {
                            break;
                        }
                        panel::poll(&mut app, &ctx);
                        std::thread::sleep(std::time::Duration::from_millis(20));
                    }
                    assert!(
                        app.listen.panel.error.is_none(),
                        "{:?}",
                        app.listen.panel.error
                    );
                }
                _ => {}
            }
            if kind == "proposal-end" {
                // The body scrolled to its end: Apply, the A/B, the notes.
                app.listen.panel.scroll = Some(10_000.0);
            }
            let path = dir.join(format!(
                "mastering-{kind}-{theme_name}-{code}-{}.png",
                (scale * 100.0) as u32
            ));
            shoot(
                &ctx,
                egui::vec2(1440.0, 900.0),
                scale,
                6,
                Some(&path),
                |root| {
                    let ctx = root.ctx().clone();
                    egui::CentralPanel::default().show(root, |ui| views::mixer::show(&mut app, ui));
                    if kind == "listening" {
                        // The bar as it stands a moment in.
                        if let Some(r) = &app.listen.panel.running {
                            r.progress
                                .store(0.42f32.to_bits(), std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    panel::frame(&mut app, &ctx);
                },
            );
            if let Some(r) = &app.listen.panel.running {
                r.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            println!("{}", path.display());
        }
    }
}
