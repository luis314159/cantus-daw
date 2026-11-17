//! Vetus's editor in its device frame, without a screen (fx side round, ADR 0130):
//! the design sheet's pictures.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_vetus -- [out-dir] [scales…] [--colourways]
//! ```
//!
//! Two states — at rest (nothing playing) and playing (its telemetry, the analyser's pairs
//! and the groove fed as a playing engine would) — in Slate and Paper, English and Spanish,
//! at the scales given (100 % unless named; the other scales Slate English only); a third,
//! the telephone, playing; the inspector's preview; with `--colourways`, each colourway
//! playing in Slate English and Paper Spanish.

use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::host::FxHost as _;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

const KIND: cantus_core::EffectKind = cantus_core::EffectKind::LoFi;

fn app_with(state: &str) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("vetus");
    let (strip, effect, _) = app.fx.open.expect("Vetus is open");
    if state == "telephone" {
        views::fx::host::DawHost::new(&mut app, strip, effect, "vetus").set_many(
            &[
                ("tone.medium", 2.0),
                ("tone.lo", 300.0),
                ("tone.hi", 3_400.0),
                ("tone.res", 0.45),
                ("bits.mode", 2.0),
                ("rate.hz", 8_000.0),
                ("rate.filter", 1.0),
                ("char", 0.6),
                ("wow", 8.0),
                // A module off: its card dimmed, its controls still answering.
                ("age.on", 0.0),
            ],
            "cmd.fx_preset",
        );
    }
    if state != "rest" {
        views::fx::demo_feed(&mut app, KIND);
    }
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

fn context(paper: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if paper {
        cantus_ui::theme::paper()
    } else {
        cantus_ui::theme::slate()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn one(dir: &Path, state: &str, paper: bool, lang: cantus_ui::i18n::Locale, k: f32, tag: &str) {
    cantus_ui::i18n::set_locale(lang);
    let ctx = context(paper);
    let mut app = app_with(state);
    views::fx::frame::shell_of(KIND).set_scale(&ctx, k);
    let size = views::fx::frame::designed_size(KIND) * k;
    let name = format!(
        "vetus-{tag}{state}-{}-{}-k{}.png",
        if paper { "paper" } else { "slate" },
        lang.code(),
        (k * 100.0) as u32
    );
    let path = dir.join(name);
    shoot(&ctx, size, 1.0, 6, Some(&path), |root| {
        let ctx = root.ctx().clone();
        views::fx::window(&mut app, &ctx);
    });
    println!("{}", path.display());
}

/// The frame budget (`VETUS_TIME=1`): the window's whole UI pass — layout, the cassette (or
/// the record with `--record`), the spectrum (a new analysis each frame: 800 new samples on
/// the tap), the cards — timed over 300 frames while playing, Slate at 100 %, with the
/// tessellation; the GPU's work is not in it.
fn time_frames() {
    let ctx = context(false);
    let mut app = app_with("playing");
    views::fx::frame::shell_of(KIND).set_scale(&ctx, 1.0);
    let size = views::fx::frame::designed_size(KIND);
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
        ..Default::default()
    };
    for _ in 0..3 {
        let mut out = ctx.run_ui(input(), |_| {});
        out.textures_delta.clear();
    }
    let mut tel = app.fx.injected.unwrap_or([0.0; 64]);
    let mut times = vec![];
    let mut n = 0u32;
    for i in 0..330 {
        // A frame's worth of new sound on the tap, the record turned on, a tick now and then.
        let pairs: Vec<f32> = (0..800)
            .flat_map(|j| {
                n = n.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let x = ((i * 800 + j) as f32 * 0.031).sin() * 0.3;
                let y = x + (n >> 9) as f32 / (1u32 << 23) as f32 * 0.01;
                [x, y]
            })
            .collect();
        app.fx.feed(&pairs);
        tel[cantus_dsp::vetus::t::TURN] = (i as f32 / 108.0).fract();
        tel[cantus_dsp::vetus::t::CLICKS] = (i % 3) as f32;
        tel[cantus_dsp::modfx_clock::tel::BLOCKS] = i as f32 + 1.0;
        app.fx.injected = Some(tel);
        let t = std::time::Instant::now();
        let mut out = ctx.run_ui(input(), |root| {
            let c = root.ctx().clone();
            views::fx::window(&mut app, &c);
        });
        let _ = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
        out.textures_delta.clear();
        if i >= 30 {
            times.push(t.elapsed().as_secs_f64() * 1e3);
        }
    }
    times.sort_by(f64::total_cmp);
    let mean = times.iter().sum::<f64>() / times.len() as f64;
    println!(
        "Vetus's window, playing: UI pass mean {mean:.2} ms, median {:.2} ms, 95th {:.2} ms, worst {:.2} ms (of 16.7 ms at 60 Hz)",
        times[times.len() / 2],
        times[times.len() * 95 / 100],
        times[times.len() - 1]
    );
}

fn main() {
    use cantus_ui::i18n::Locale;
    if std::env::args().any(|a| a == "--record") {
        views::fx::unit_view::set_view("vetus", "record");
    }
    if std::env::var_os("VETUS_TIME").is_some() {
        time_frames();
        return;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/vetus"));
    let scales: Vec<f32> = args
        .iter()
        .skip(1)
        .filter_map(|a| a.parse::<f32>().ok())
        .collect();
    let scales = if scales.is_empty() { vec![1.0] } else { scales };
    std::fs::create_dir_all(&dir).expect("output dir");
    // The object: the cassette unless `--record`.
    let record = args.iter().any(|a| a == "--record");
    if record {
        views::fx::unit_view::set_view("vetus", "record");
    }
    let obj = if record { "record-" } else { "cassette-" };
    if args.iter().any(|a| a == "--colourways") {
        use cantus_ui::kit::skin;
        let unit = skin::Unit::Effect(KIND);
        for w in skin::colourways(unit) {
            skin::set_colourway("vetus", w.id);
            for (paper, lang) in [(false, Locale::En), (true, Locale::Es)] {
                one(
                    &dir,
                    "playing",
                    paper,
                    lang,
                    1.0,
                    &format!("{obj}{}-", w.id),
                );
            }
        }
        skin::set_colourways(std::iter::empty());
        return;
    }
    for state in ["rest", "playing"] {
        for (paper, lang) in [(false, Locale::En), (true, Locale::Es)] {
            one(&dir, state, paper, lang, 1.0, obj);
        }
    }
    for &k in scales.iter().filter(|k| **k != 1.0) {
        one(&dir, "playing", false, Locale::En, k, obj);
    }
    one(&dir, "telephone", false, Locale::En, 1.0, obj);
    one(&dir, "telephone", true, Locale::Es, 1.0, obj);
    for paper in [false, true] {
        cantus_ui::i18n::set_locale(Locale::En);
        let ctx = context(paper);
        let mut spec = cantus_core::EffectSpec::new(KIND);
        for (k, v) in views::fx::vetus::DEMO {
            spec.set(k, *v);
        }
        let path = dir.join(format!(
            "vetus-preview-{}.png",
            if paper { "paper" } else { "slate" }
        ));
        shoot(&ctx, egui::vec2(420.0, 80.0), 1.0, 3, Some(&path), |ui| {
            views::fx::vetus::preview(ui, &spec);
        });
        println!("{}", path.display());
    }
}
