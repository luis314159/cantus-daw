//! Fragor's window without a screen (the fx side round, ADR 0128): the design sheet.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_fragor -- [out-dir] [--colourways]
//! ```
//!
//! Two boards — three pedals (the green overdrive into the germanium fuzz into the
//! triangle muff), and two with a slot free (the silicon distortion and the op-amp
//! distortion, its footswitch off) — at rest and playing, in Slate (the default look)
//! and Paper, English and Spanish, at 75 %, 100 %, 150 % and 200 %; and the inspector's
//! strip. With `--colourways`, the three-pedal board playing in each colourway (ADR 0127
//! §Colourways), Slate English and Paper Spanish at 100 %, into `<out-dir>/colour/final/`.

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
    if state == "board" {
        // A mini, the round fuzz and a wide box on one board.
        views::fx::host::DawHost::new(&mut app, strip, effect, "fragor").set_many(
            &[("s1", 3.0), ("s2", 4.0), ("s3", 5.0), ("si.dist", 0.6)],
            "cmd.fx_preset",
        );
    }
    if state == "two" {
        // Two standard boxes, the second at the large view size, and a free slot.
        app.fx.fragor.view_sizes[1] = 2;
        views::fx::host::DawHost::new(&mut app, strip, effect, "fragor").set_many(
            &[
                ("s1", 1.0),
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

/// The frame budget (`FRAGOR_TIME=1`): the window's whole UI pass — layout, the display's
/// curves, the pedals, the tubes, every widget — timed over 300 frames while playing (the
/// telemetry fed afresh each frame so the meters and glows move), in Slate at 100 %, with
/// the tessellation; the GPU's work is not in it.
fn time_frames() {
    let ctx = context(true);
    let mut app = app_with("board", true);
    let kind = cantus_core::EffectKind::Pedal;
    views::fx::frame::shell_of(kind).set_scale(&ctx, 1.0);
    let size = views::fx::frame::designed_size(kind);
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    // Settle the context (fonts, the atlas) on frames that paint nothing.
    for _ in 0..3 {
        let mut out = ctx.run_ui(input(), |_| {});
        out.textures_delta.clear();
    }
    let mut times = vec![];
    for i in 0..330 {
        fragor::demo_feed_at(&mut app, if i % 2 == 0 { 0.35 } else { 0.2 });
        let t = std::time::Instant::now();
        let out = ctx.run_ui(input(), |root| {
            let c = root.ctx().clone();
            views::fx::window(&mut app, &c);
        });
        let mut out = out;
        let _ = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
        // No renderer here: the font atlas's uploads are taken as done.
        out.textures_delta.clear();
        if i >= 30 {
            times.push(t.elapsed().as_secs_f64() * 1e3);
        }
    }
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "Fragor's window, playing: UI pass mean {mean:.2} ms, median {:.2} ms, 95th {:.2} ms, worst {:.2} ms (of 16.7 ms at 60 Hz)",
        times[times.len() / 2],
        times[times.len() * 95 / 100],
        times[times.len() - 1]
    );
}

/// The cables' energy as frames (`FRAGOR_ANIM=1`): the board playing, Slate, 100 %, 30
/// frames at 30 per second, then an animated WebP of them (Python's PIL, if present).
fn anim_frames(dir: &std::path::Path) {
    std::fs::create_dir_all(dir).expect("anim dir");
    let mut app = app_with("board", true);
    let kind = cantus_core::EffectKind::Pedal;
    let size = views::fx::frame::designed_size(kind);
    for i in 0..30 {
        // A fresh context per frame: the raster renderer gets the whole glyph atlas only
        // on a context's first frame.
        let ctx = context(true);
        views::fx::frame::shell_of(kind).set_scale(&ctx, 1.0);
        app.fx.fragor.anim_time = Some(i as f64 / 30.0);
        let path = dir.join(format!("frame-{i:02}.png"));
        shoot(&ctx, size, 1.0, 2, Some(&path), |root| {
            let ctx = root.ctx().clone();
            views::fx::window(&mut app, &ctx);
        });
        println!("{}", path.display());
    }
}

fn main() {
    if std::env::var_os("FRAGOR_TIME").is_some() {
        time_frames();
        return;
    }
    if std::env::var_os("FRAGOR_ANIM").is_some() {
        let dir = std::env::args()
            .nth(1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("target/shots/fragor-anim"));
        anim_frames(&dir);
        return;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/fragor"));
    std::fs::create_dir_all(&dir).expect("output dir");
    if args.iter().any(|a| a == "--colourways") {
        colourways(&dir);
        return;
    }
    use cantus_ui::i18n::Locale;
    let kind = cantus_core::EffectKind::Pedal;
    let only = std::env::var("FRAGOR_ONLY").ok();
    let prefix = String::new();
    for state in ["board", "two"] {
        for playing in [false, true] {
            for dark in [true, false] {
                for lang in [Locale::En, Locale::Es] {
                    for k in [0.75f32, 1.0, 1.5, 2.0] {
                        // The sheet: everything at 100 %; the other scales for the board
                        // playing in Slate (and Spanish at 150 %).
                        let main = k == 1.0;
                        let extra = playing
                            && ((state == "board" && (k == 1.5 || (dark && lang == Locale::En)))
                                || (state == "two" && k == 2.0 && dark && lang == Locale::En));
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
                        if k == 2.0 {
                            for (i, r) in app.fx.fragor.pedal_rects.iter().flatten().enumerate() {
                                println!(
                                    "PEDAL {state} {i} {:.0} {:.0} {:.0} {:.0}",
                                    r.left(),
                                    r.top(),
                                    r.right(),
                                    r.bottom()
                                );
                            }
                        }
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

/// The colourways as the user chooses them (ADR 0127 §Colourways): the three-pedal board
/// playing in each, Slate English and Paper Spanish at 100 %, into `<dir>/colour/final/`.
fn colourways(dir: &std::path::Path) {
    use cantus_ui::i18n::Locale;
    use cantus_ui::kit::skin;
    let kind = cantus_core::EffectKind::Pedal;
    let dir = dir.join("colour").join("final");
    std::fs::create_dir_all(&dir).expect("output dir");
    let unit = skin::Unit::Effect(kind);
    for w in skin::colourways(unit) {
        for (dark, lang) in [(true, Locale::En), (false, Locale::Es)] {
            cantus_ui::i18n::set_locale(lang);
            let ctx = context(dark);
            skin::set_colourway(skin::colourway_unit(unit).expect("a unit"), w.id);
            let mut app = app_with("board", true);
            views::fx::frame::shell_of(kind).set_scale(&ctx, 1.0);
            let size = views::fx::frame::designed_size(kind);
            let path = dir.join(format!(
                "fragor-{}-{}-{}-k100.png",
                w.id,
                if dark { "slate" } else { "paper" },
                lang.code()
            ));
            shoot(&ctx, size, 1.0, 6, Some(&path), |root| {
                let ctx = root.ctx().clone();
                views::fx::window(&mut app, &ctx);
            });
            println!("{}", path.display());
        }
    }
    skin::set_colourways(std::iter::empty());
}
