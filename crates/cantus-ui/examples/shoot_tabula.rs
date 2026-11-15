//! Tabula's window without a screen (the fx side round, wave 3, ADR 0134): the design sheet.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_tabula -- [out-dir]
//! ```
//!
//! The guitar rig (the starting preset) on the board at mixed sizes, playing, in Slate
//! English and Paper Spanish at 100 % and in Slate English at 75 %, 150 % and 200 %; the
//! rig at rest; the A/B split (the delay on A, the spring on B, the chorus after them); the
//! pedal picker open; and a pedal's full editor opened from the board. `TABULA_ONLY=a,b`
//! shoots only the names holding one of those words; `TABULA_ANIM=1` the cables' energy as
//! thirty frames.

use cantus_core::fx::tabula::{Path, PedalId};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::tabula;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::PathBuf;

fn app_with(state: &str, playing: bool) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor("tabula");
    let (strip, effect, _) = app.fx.open.expect("Tabula is open");
    if state == "split" {
        use cantus_core::Command as C;
        for (id, index, path) in [(2u16, 1usize, Path::A), (3, 2, Path::B), (1, 3, Path::Post)] {
            app.exec(C::MovePedal {
                strip,
                effect,
                pedal: PedalId(id),
                index,
                path,
            });
        }
        app.fx.tabula.selected = Some(PedalId(2));
    }
    if state == "many" {
        // A full board: the rig and five more, wrapped into rows.
        use cantus_core::fx::tabula::Pedal;
        use cantus_core::{Command as C, EffectKind as K, EffectSpec};
        for (i, kind) in [
            K::Compressor,
            K::AutoFilter,
            K::Phaser,
            K::Tremolo,
            K::Utility,
        ]
        .into_iter()
        .enumerate()
        {
            let pedal = Pedal::new(PedalId(4 + i as u16), EffectSpec::new(kind));
            app.exec(C::InsertPedal {
                strip,
                effect,
                index: [0, 1, 3, 5, 8][i],
                pedal: Box::new(pedal),
                maps: Vec::new(),
            });
        }
    }
    if state == "picker" {
        // The modulation shelf, its second pedal under the pointer.
        let shelf = tabula::picker_shelf(cantus_ui::picker_catalog::Category::Modulation);
        app.fx.tabula.picker = Some(shelf);
        app.fx.tabula.picker_hover = Some(1);
    }
    if state == "editor" {
        let spec = app.project().effect(strip, effect).unwrap().spec.clone();
        tabula::open_pedal(&mut app.fx.tabula, &spec, PedalId(1));
    }
    tabula::demo_feed_at(&mut app, if playing { 0.4 } else { 0.0 });
    app.fx.tabula.anim_time = Some(0.37);
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
    use cantus_ui::i18n::Locale;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/tabula"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let kind = cantus_core::EffectKind::Board;
    let only = std::env::var("TABULA_ONLY").ok();
    let anim = std::env::var_os("TABULA_ANIM").is_some();
    let mut shots: Vec<(&str, bool, bool, Locale, f32)> = vec![
        ("rig", true, true, Locale::En, 1.0),
        ("rig", true, false, Locale::Es, 1.0),
        ("rig", false, true, Locale::En, 1.0),
        ("rig", true, true, Locale::En, 0.75),
        ("rig", true, true, Locale::En, 1.5),
        ("rig", true, true, Locale::En, 2.0),
        ("many", true, true, Locale::En, 1.0),
        ("many", true, false, Locale::Es, 1.0),
        ("split", true, true, Locale::En, 1.0),
        ("split", true, false, Locale::Es, 1.0),
        ("picker", true, true, Locale::En, 1.0),
        ("picker", true, false, Locale::Es, 1.0),
        ("editor", true, true, Locale::En, 1.0),
        ("editor", true, false, Locale::Es, 1.0),
    ];
    if anim {
        shots.clear();
    }
    for (state, playing, dark, lang, k) in shots {
        let name = format!(
            "tabula-{state}-{}-{}-{}-k{}.png",
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
    if anim {
        let mut app = app_with("rig", true);
        let size = views::fx::frame::designed_size(kind);
        for i in 0..30 {
            let ctx = context(true);
            views::fx::frame::shell_of(kind).set_scale(&ctx, 1.0);
            app.fx.tabula.anim_time = Some(i as f64 / 30.0);
            let path = dir.join(format!("frame-{i:02}.png"));
            shoot(&ctx, size, 1.0, 2, Some(&path), |root| {
                let ctx = root.ctx().clone();
                views::fx::window(&mut app, &ctx);
            });
            println!("{}", path.display());
        }
    }
}
