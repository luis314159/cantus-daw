//! The kit's layer atlas grows with what is drawn (round 40, ADR 0120 §1): it starts at
//! 512² and doubles, between passes, the first time a layer does not fit — a window draws
//! the same pixels as one whose atlas started at 1024² (the size it always had), the growth
//! happens once, and steady state never grows, shrinks or resets the atlas again.
//! Rendered on the CPU (`cantus_ui::raster`), so the comparison is of pixels.

use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::cache::{self, ATLAS, ATLAS_START};
use egui::{Color32, Pos2, Rect, Vec2};

/// A page of `n` distinct layers of 90 × 90 physical pixels, each drawn once: 36 of them
/// hold more than a 512² atlas can (an idle window's are far fewer; this one must grow).
fn page(ui: &mut egui::Ui, brush: &mut Brush, n: u64) {
    brush.begin(ui.ctx());
    let ppp = ui.ctx().pixels_per_point();
    let side = 90usize;
    let pt = side as f32 / ppp;
    for k in 0..n {
        let (col, row) = ((k % 9) as f32, (k / 9) as f32);
        let rect = Rect::from_min_size(
            Pos2::new(4.0 + col * (pt + 2.0), 4.0 + row * (pt + 2.0)),
            Vec2::splat(pt),
        );
        let shade = (40 + (k * 37) % 200) as u8;
        let uv = brush.layer(0xA71A5 + k, side, side, |pix| {
            let paint = tiny_skia::Paint {
                shader: tiny_skia::Shader::SolidColor(tiny_skia::Color::from_rgba8(
                    shade,
                    255 - shade,
                    128,
                    255,
                )),
                anti_alias: true,
                ..Default::default()
            };
            if let Some(circle) =
                tiny_skia::PathBuilder::from_circle(45.0, 45.0, 40.0 - (k % 7) as f32)
            {
                pix.fill_path(
                    &circle,
                    &paint,
                    tiny_skia::FillRule::Winding,
                    tiny_skia::Transform::identity(),
                    None,
                );
            }
        });
        if let Some(uv) = uv {
            brush.image(rect, uv, Color32::WHITE);
        }
    }
    brush.flush(ui.painter());
}

fn shoot(
    start: Option<usize>,
    n: u64,
    frames: usize,
) -> (cantus_ui::raster::Canvas, cache::Stats, usize) {
    let ctx = egui::Context::default();
    if let Some(s) = start {
        cache::with_raw(&ctx, |c| c.start_at(s));
    }
    let mut brush = Brush::default();
    let canvas = cantus_ui::raster::shoot(&ctx, Vec2::new(480.0, 400.0), 1.0, frames, None, |ui| {
        page(ui, &mut brush, n)
    });
    let (stats, side) = cache::with_raw(&ctx, |c| (c.stats, c.side()));
    (canvas, stats, side)
}

fn same(a: &cantus_ui::raster::Canvas, b: &cantus_ui::raster::Canvas) -> usize {
    assert_eq!((a.width, a.height), (b.width, b.height));
    let mut differ = 0;
    for y in 0..a.height {
        for x in 0..a.width {
            if a.pixel(x, y) != b.pixel(x, y) {
                differ += 1;
            }
        }
    }
    differ
}

#[test]
fn a_page_that_fills_the_start_grows_once_and_draws_as_the_full_atlas() {
    assert_eq!(ATLAS_START, 512);
    let (grown, g, side) = shoot(None, 40, 4);
    assert_eq!(g.atlas_grown, 1, "the atlas doubled once: {g:?}");
    assert_eq!(side, ATLAS);
    assert_eq!(g.atlas_resets, 0, "{g:?}");
    let (full, f, side) = shoot(Some(ATLAS), 40, 4);
    assert_eq!(
        (f.atlas_grown, f.atlas_resets, side),
        (0, 0, ATLAS),
        "{f:?}"
    );
    assert_eq!(
        same(&grown, &full),
        0,
        "a grown atlas draws other pixels than a 1024² one"
    );
}

#[test]
fn steady_state_never_grows_shrinks_or_resets_the_atlas() {
    let (_, s, side) = shoot(None, 40, 60);
    assert_eq!(
        (s.atlas_grown, s.atlas_resets, side),
        (1, 0, ATLAS),
        "{s:?}"
    );
    // The layers drawn after the growth are drawn once each: 40, none again.
    let (_, s2, _) = shoot(None, 40, 120);
    assert_eq!(
        s2.layers_rasterised, s.layers_rasterised,
        "a layer was drawn again in steady state"
    );
}

#[test]
fn a_page_that_fits_never_grows() {
    let (small, s, side) = shoot(None, 12, 4);
    assert_eq!(
        (s.atlas_grown, s.atlas_resets, side),
        (0, 0, ATLAS_START),
        "{s:?}"
    );
    let (full, _, _) = shoot(Some(ATLAS), 12, 4);
    assert_eq!(
        same(&small, &full),
        0,
        "a 512² atlas draws other pixels than a 1024² one"
    );
}

/// The application as the boot assembles it, on the demo, its mixer at 2 px/pt: the window
/// a 512² start draws (growing if it must) is the window a 1024² start draws, pixel for
/// pixel, and its atlas grew at most once.
#[test]
fn the_assembled_window_draws_the_same_from_either_start() {
    let shoot_app = |start: Option<usize>| {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        std::env::set_var("CANTUS_DATA_DIR", dir.path());
        let files = cantus_ui::app::Files::read(dir.path(), Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        if let Some(s) = start {
            cache::with_raw(&ctx, |c| c.start_at(s));
        }
        let (engine, _proc) = cantus_engine::EngineHandle::manual(48_000.0);
        let mut app = cantus_ui::CantusApp::assemble(
            &ctx,
            files,
            Some(cantus_ui::demos::startup_project()),
            engine,
            None,
            None,
            None,
            false,
        );
        app.show_central(cantus_ui::app::CentralView::Mixer);
        let canvas =
            cantus_ui::raster::shoot(&ctx, Vec2::new(960.0, 600.0), 2.0, 8, None, |root| {
                app.frame(root)
            });
        let stats = cache::with_raw(&ctx, |c| c.stats);
        (canvas, stats)
    };
    let (grown, g) = shoot_app(None);
    let (full, f) = shoot_app(Some(ATLAS));
    eprintln!("from 512²: {g:?}\nfrom 1024²: {f:?}");
    assert!(g.atlas_grown <= 1, "{g:?}");
    assert_eq!((g.atlas_resets, f.atlas_resets, f.atlas_grown), (0, 0, 0));
    assert_eq!(
        same(&grown, &full),
        0,
        "the window draws other pixels from a 512² start"
    );
}
