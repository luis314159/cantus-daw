//! Round 37 (ADR 0104): Chorda's scene, alone, on the CPU — every factory preset's piano
//! and room in both themes, the Lid, Mics and Room controls at their ends and middle, and
//! the default at 100 %, 150 % and 200 %; each as a PNG and gathered into contact sheets.
//!
//! `cargo run -p cantus-ui --example shoot_chorda_scene -- [out_dir] [--only=<preset>]`
//!
//! Writes into `out_dir` (default `target/shoot-chorda-scene`): `preset-<name>-<theme>.png`,
//! `controls-<control>-<value>-<theme>.png`, `zoom-<pct>-<theme>.png`, and the sheets
//! `sheet-presets-<theme>.png` and `sheet-controls-<theme>.png`.

use cantus_core::{InstrumentKind, NativeSpec};
use cantus_ui::raster::{self, Canvas};
use cantus_ui::views::inst::chorda::{scene, FACTORY_PRESETS};
use std::path::{Path, PathBuf};

/// One tile's size in points.
const TILE: egui::Vec2 = egui::vec2(800.0, 450.0);

fn shoot(path: Option<&Path>, inputs: scene::Inputs, size: egui::Vec2, zoom: f32) -> Canvas {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    scene::set_blocking(true);
    let mut s = scene::Scene::default();
    let t0 = std::time::Instant::now();
    let canvas = raster::shoot(&ctx, size, zoom, 3, path, |ui| {
        let rect = ui.max_rect();
        let painter = ui.painter().clone();
        // Settled: the same inputs every frame, so the glide is over at once.
        let _ = s.paint(&painter, rect, inputs, 100.0);
    });
    if let Some(p) = path {
        println!(
            "{} · {} triangles · {:.0} ms",
            p.display(),
            scene::triangles(&inputs, size),
            t0.elapsed().as_secs_f64() * 1e3
        );
    }
    canvas
}

/// Gather `tiles` (all one size, each shrunk by `shrink`: 1 or 2) into a sheet `cols`
/// wide, a gap between them.
fn sheet(path: &Path, tiles: &[Canvas], cols: usize, shrink: usize) {
    if tiles.is_empty() {
        return;
    }
    let k = shrink.max(1);
    let (tw, th) = (tiles[0].width / k, tiles[0].height / k);
    let gap = 8;
    let rows = tiles.len().div_ceil(cols);
    let (w, h) = (cols * tw + (cols + 1) * gap, rows * th + (rows + 1) * gap);
    let mut pix = tiny_skia::Pixmap::new(w as u32, h as u32).expect("a sheet");
    pix.fill(tiny_skia::Color::from_rgba8(24, 24, 26, 255));
    let data = pix.pixels_mut();
    for (n, t) in tiles.iter().enumerate() {
        let (cx, cy) = (gap + (n % cols) * (tw + gap), gap + (n / cols) * (th + gap));
        for y in 0..th {
            for x in 0..tw {
                let mut acc = [0u32; 3];
                for dy in 0..k {
                    for dx in 0..k {
                        let p = t.pixel(x * k + dx, y * k + dy);
                        for c in 0..3 {
                            acc[c] += p[c] as u32;
                        }
                    }
                }
                let n = (k * k) as u32;
                let px = tiny_skia::ColorU8::from_rgba(
                    (acc[0] / n) as u8,
                    (acc[1] / n) as u8,
                    (acc[2] / n) as u8,
                    255,
                );
                data[(cy + y) * w + cx + x] = px.premultiply();
            }
        }
    }
    std::fs::write(path, pix.encode_png().expect("encode")).expect("write the sheet");
    println!("{}", path.display());
}

fn name_of(label_key: &str) -> &str {
    label_key.rsplit('.').next().unwrap_or(label_key)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .filter(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shoot-chorda-scene")
        });
    let rest: Vec<String> = std::env::args().skip(1).collect();
    let only = rest.iter().find_map(|a| a.strip_prefix("--only="));
    std::fs::create_dir_all(&dir).expect("create the output directory");
    for (dark, theme) in [(true, "slate"), (false, "paper")] {
        let mut tiles = Vec::new();
        for p in FACTORY_PRESETS {
            let name = name_of(p.label_key);
            if only.is_some_and(|o| !o.split(',').any(|x| x == name)) {
                continue;
            }
            let spec = cantus_ui::views::inst::preset_spec(InstrumentKind::Chorda, p);
            let inputs = scene::Inputs::of(&spec, dark);
            let path = dir.join(format!("preset-{name}-{theme}.png"));
            tiles.push(shoot(Some(&path), inputs, TILE, 1.0));
        }
        if only.is_some() {
            continue;
        }
        sheet(
            &dir.join(format!("sheet-presets-{theme}.png")),
            &tiles,
            4,
            2,
        );
        // The controls at their ends and middle, on the default piano.
        let base = scene::Inputs::of(&NativeSpec::new(InstrumentKind::Chorda), dark);
        let mut ctl = Vec::new();
        for (key, set) in [
            (
                "lid",
                (|i: &mut scene::Inputs, v: f32| i.lid = v) as fn(&mut scene::Inputs, f32),
            ),
            ("persp", |i, v| i.persp = v),
            ("roomsize", |i, v| i.size = v),
            ("room", |i, v| i.room = v),
            ("width", |i, v| i.width = v),
        ] {
            for v in [0.0f32, 0.5, 1.0] {
                let mut i = base;
                set(&mut i, v);
                if key == "roomsize" {
                    i.room = 0.6;
                }
                let path = dir.join(format!("controls-{key}-{:.0}-{theme}.png", v * 100.0));
                ctl.push(shoot(Some(&path), i, TILE, 1.0));
            }
        }
        sheet(&dir.join(format!("sheet-controls-{theme}.png")), &ctl, 3, 2);
        for zoom in [1.0f32, 1.5, 2.0] {
            let path = dir.join(format!("zoom-{:.0}-{theme}.png", zoom * 100.0));
            shoot(Some(&path), base, TILE, zoom);
        }
    }
}
