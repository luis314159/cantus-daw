//! Fragor's window without a screen (the fx side round, ADR 0131): the design sheet.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_fragor -- [out-dir]
//! ```
//!
//! Two boards — three pedals (the green overdrive into the germanium fuzz into the
//! triangle muff), and two with a slot free (the silicon distortion and the op-amp
//! distortion, its footswitch off) — at rest and playing, in Slate (the default look)
//! and Paper, English and Spanish, at 75 %, 100 %, 150 % and 200 %; and the inspector's
//! strip. `FRAGOR_LOOK=a|b|c` renders in one of the palettes on offer (`variant-<x>-…`).

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::fragor;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

fn app_with(state: &str, playing: bool) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("fragor");
    let (strip, effect, _) = app.fx.open.expect("Fragor is open");
    if state == "two" {
        views::fx::host::DawHost::new(&mut app, strip, effect, "fragor").set_many(
            &[
                ("s1", 3.0),
                ("s2", 2.0),
                ("s3", 0.0),
                ("si.dist", 0.8),
                ("si.tone", 0.35),
                ("op.dist", 0.6),
                ("op.filter", 0.3),
                ("op.on", 0.0),
                ("gate", -90.0),
                ("mono", 1.0),
            ],
            "cmd.fx_preset",
        );
    }
    // Playing: a note at a guitar's level; at rest: the unit fed silence.
    fragor::demo_feed_at(&mut app, if playing { 0.35 } else { 0.0 });
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::slate()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/fragor"));
    std::fs::create_dir_all(&dir).expect("output dir");
    use cantus_ui::i18n::Locale;
    let kind = cantus_core::EffectKind::Pedal;
    let only = std::env::var("FRAGOR_ONLY").ok();
    // `FRAGOR_LOOK=a|b|c`: the window in one of the palettes on offer to the owner (the
    // files named `variant-<x>-…`).
    let look = std::env::var("FRAGOR_LOOK").ok();
    let prefix = match look.as_deref() {
        Some(l @ ("a" | "b" | "c")) => {
            let i = (l.as_bytes()[0] - b'a') as usize;
            cantus_ui::kit::skin::set_override(Some(cantus_ui::kit::skin::FRAGOR_LOOKS[i]));
            format!("variant-{l}-")
        }
        _ => String::new(),
    };
    for state in ["board", "two"] {
        for playing in [false, true] {
            for dark in [true, false] {
                for lang in [Locale::En, Locale::Es] {
                    for k in [0.75f32, 1.0, 1.5, 2.0] {
                        // The sheet: everything at 100 %; the other scales for the board
                        // playing in Slate (and Spanish at 150 %).
                        let main = k == 1.0;
                        let extra =
                            state == "board" && playing && dark && (lang == Locale::En || k == 1.5);
                        if !main && !extra {
                            continue;
                        }
                        let name = format!(
                            "{prefix}fragor-{state}-{}-{}-{}-k{}.png",
                            if playing { "playing" } else { "rest" },
                            if dark { "slate" } else { "paper" },
                            lang.code(),
                            (k * 100.0) as u32
                        );
                        if only
                            .as_ref()
                            .is_some_and(|o| !o.split(',').any(|p| name.contains(p)))
                        {
                            continue;
                        }
                        cantus_ui::i18n::set_locale(lang);
                        let ctx = context(dark);
                        let mut app = app_with(state, playing);
                        views::fx::frame::shell_of(kind).set_scale(&ctx, k);
                        let size = views::fx::frame::designed_size(kind) * k;
                        let path = dir.join(name);
                        shoot(&ctx, size, 1.0, 6, Some(&path), |root| {
                            let ctx = root.ctx().clone();
                            views::fx::window(&mut app, &ctx);
                        });
                        println!("{}", path.display());
                    }
                }
            }
        }
    }
    for dark in [true, false] {
        cantus_ui::i18n::set_locale(Locale::En);
        let ctx = context(dark);
        let mut spec = cantus_core::EffectSpec::new(kind);
        spec.set("s1", 1.0);
        spec.set("s2", 4.0);
        spec.set("s3", 5.0);
        let path = dir.join(format!(
            "fragor-preview-{}.png",
            if dark { "slate" } else { "paper" }
        ));
        shoot(&ctx, egui::vec2(420.0, 60.0), 1.0, 3, Some(&path), |ui| {
            fragor::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
