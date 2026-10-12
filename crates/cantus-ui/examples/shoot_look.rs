//! Round 25's look, headless (ADR 0046): every shipped theme's preview swatch side by side,
//! the Appearance page's taskbar-icon chips, and the mark at every icon size on a dark and
//! a light taskbar — in each theme asked for, at 75 %, 100 % and 200 %, English or Spanish.
//!
//! `cargo run -p cantus-ui --example shoot_look -- <out-dir> [es]`

use cantus_engine::EngineHandle;
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::appearance::{IconColour, IconPreset, Mode, PRESETS};
use cantus_ui::kit::mark::MarkColours;
use cantus_ui::kit::{with_brush, Style};
use cantus_ui::views::appearance_look;
use cantus_ui::{raster, welcome_project, CantusApp};
use egui::{Pos2, Rect, Vec2};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "target/shoot-look".into()));
    let spanish = args.any(|a| a == "es");
    std::fs::create_dir_all(&dir).expect("output dir");
    i18n::set_locale(if spanish { Locale::Es } else { Locale::En });
    let lang = if spanish { "-es" } else { "" };
    for (preset, make) in PRESETS {
        for zoom in [0.75f32, 1.0, 2.0] {
            if zoom != 1.0 && !preset.starts_with("cantus") {
                continue;
            }
            let ctx = egui::Context::default();
            cantus_ui::design::install_fonts(&ctx);
            cantus_ui::design::install_style(&ctx);
            let tmp = std::env::temp_dir().join("cantus-shoot-look");
            let mut app =
                CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
            app.settings_path = tmp.join("settings.json");
            app.appearance_path = tmp.join("appearance.json");
            let mode = if make().dark { Mode::Dark } else { Mode::Light };
            app.appearance.mode = mode;
            app.appearance.assign(mode, format!("preset:{preset}"));
            app.appearance.icon = IconColour::Preset(IconPreset::Verdigris);
            app.attach(&ctx);
            let path = dir.join(format!(
                "look-{preset}-z{}{lang}.png",
                (zoom * 100.0) as u32
            ));
            let size = Vec2::new(880.0, 520.0);
            raster::shoot(&ctx, size, zoom, 4, Some(&path), |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);
                    swatches(&mut app, ui);
                    appearance_look::icon_section(&mut app, ui);
                });
            });
            println!("{}", path.display());
        }
    }
    marks(&dir);
}

/// Every theme's preview, named.
fn swatches(app: &mut CantusApp, ui: &mut egui::Ui) {
    let s = Style::current();
    ui.horizontal_wrapped(|ui| {
        for (preset, _) in PRESETS {
            let base = format!("preset:{preset}");
            let t = appearance_look::preview_tokens(app, &base);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(136.0, 88.0), egui::Sense::hover());
            let chip = Rect::from_min_size(rect.min, Vec2::new(136.0, 64.0));
            with_brush(ui.painter(), |b| {
                appearance_look::paint_preview(b, chip, &t, s.k);
                let label = Rect::from_min_max(Pos2::new(rect.left(), chip.bottom()), rect.max);
                let _ = cantus_ui::kit::text::paint(
                    b,
                    label,
                    &cantus_ui::kit::text::Text::new(
                        &i18n::t(&cantus_ui::kit::appearance::preset_name_key(preset)),
                        cantus_ui::kit::text::Role::Label,
                        s.t.text,
                    )
                    .policy(cantus_ui::kit::text::Policy::Elide(
                        cantus_ui::kit::text::Elide::End,
                    )),
                    s.tk(),
                );
            });
        }
    });
}

/// The mark at every icon size, 1:1 and enlarged, in each icon colour, on a dark and a
/// light taskbar.
fn marks(dir: &std::path::Path) {
    use resvg::tiny_skia;
    let sizes = [16u32, 20, 24, 32, 40, 48, 64];
    let mut colours = vec![("original", MarkColours::ORIGINAL)];
    for p in IconPreset::ALL {
        colours.push((p.name_key(), MarkColours::from_one(p.rgb())));
    }
    let backs = [[0x20u8, 0x20, 0x20], [0xee, 0xee, 0xee]];
    let row_h = 72u32;
    let w = 8 + sizes.iter().map(|s| s + 12).sum::<u32>() + 16 + 64 * 3;
    let h = row_h * (colours.len() * backs.len()) as u32;
    let mut sheet = tiny_skia::Pixmap::new(w, h).unwrap();
    for (bi, bg) in backs.iter().enumerate() {
        for (ci, (_, c)) in colours.iter().enumerate() {
            let y0 = ((bi * colours.len() + ci) as u32 * row_h) as f32;
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(bg[0], bg[1], bg[2], 255);
            sheet.fill_rect(
                tiny_skia::Rect::from_xywh(0.0, y0, w as f32, row_h as f32).unwrap(),
                &paint,
                tiny_skia::Transform::identity(),
                None,
            );
            let mut x = 8i32;
            for &px in &sizes {
                let pix = cantus_ui::window_icon::render(c, px).unwrap();
                sheet.draw_pixmap(
                    x,
                    y0 as i32 + 4,
                    pix.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
                x += px as i32 + 12;
            }
            // 16 and 24 enlarged ×3 (nearest), to see the pixel grid.
            for px in [16u32, 24] {
                let pix = cantus_ui::window_icon::render(c, px).unwrap();
                let z = 64 / px.max(1);
                let mut big = tiny_skia::Pixmap::new(px * z, px * z).unwrap();
                for yy in 0..px * z {
                    for xx in 0..px * z {
                        big.pixels_mut()[(yy * px * z + xx) as usize] =
                            pix.pixel(xx / z, yy / z).unwrap();
                    }
                }
                sheet.draw_pixmap(
                    x + 8,
                    y0 as i32 + 4,
                    big.as_ref(),
                    &tiny_skia::PixmapPaint::default(),
                    tiny_skia::Transform::identity(),
                    None,
                );
                x += 64 + 12;
            }
        }
    }
    let path = dir.join("marks.png");
    sheet.save_png(&path).unwrap();
    println!("{}", path.display());
}
