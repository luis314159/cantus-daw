//! A contact sheet of a character file's clips (ADR 0089): every clip at four moments,
//! drawn by the importer, over a checker — to look at what an import draws.
//!
//! `cargo run -p cantus-dance --example import_sheet -- <file> <out.png> [cell px]`
//! With `--samples <dir>` it writes our generated test characters into `<dir>` instead.

use cantus_dance::import::{self, samples, Source};
use std::path::Path;
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

fn checker(pm: &mut Pixmap, size: f32) {
    let mut paint = Paint::default();
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let mut y = 0.0;
    let mut row = 0;
    while y < h {
        let mut x = 0.0;
        let mut col = row;
        while x < w {
            let v = if col % 2 == 0 { 236 } else { 214 };
            paint.set_color(Color::from_rgba8(v, v, v, 255));
            if let Some(r) = Rect::from_xywh(x, y, size, size) {
                pm.fill_rect(r, &paint, Transform::identity(), None);
            }
            x += size;
            col += 1;
        }
        y += size;
        row += 1;
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--samples") {
        let dir = Path::new(args.get(2).map(String::as_str).unwrap_or("samples"));
        let mut all = samples::aseprite("figure-aseprite");
        all.extend(samples::texturepacker("figure-texturepacker", true));
        all.extend(samples::lottie("figure-lottie"));
        all.extend(samples::dotlottie("figure-package"));
        all.extend(samples::dragonbones("figure-dragonbones"));
        samples::write(dir, &all).expect("write the samples");
        println!("wrote {} files into {}", all.len(), dir.display());
        return;
    }
    let (Some(file), Some(out)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: import_sheet <file> <out.png> [cell px] | --samples <dir>");
        return;
    };
    let cell: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(160);
    let found = match import::inspect(Path::new(file)) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{file}: {e}");
            std::process::exit(1);
        }
    };
    for n in &found.notes {
        println!("note: {} {}", n.key, n.arg);
    }
    let src: &dyn Source = found.source.as_ref();
    let clips = src.clips().to_vec();
    let b = src.bounds();
    let cols = 4u32;
    let rows = clips.len() as u32;
    let mut pm = Pixmap::new(cols * cell, rows.max(1) * cell).expect("a sheet");
    checker(&mut pm, 8.0);
    let scale = (cell as f32 * 0.9) / b.width().max(b.height()).max(1e-3);
    for (r, c) in clips.iter().enumerate() {
        for k in 0..cols {
            let t = c.seconds * k as f32 / cols as f32;
            let ox = k as f32 * cell as f32 + cell as f32 * 0.5;
            let oy = r as f32 * cell as f32 + cell as f32 * 0.95;
            let transform = Transform::from_translate(ox, oy)
                .pre_scale(scale, scale)
                .pre_translate(-(b.left + b.right) * 0.5, -b.bottom);
            let mut canvas = cantus_dance::render::Canvas {
                pixmap: &mut pm,
                transform,
                px_per_unit: scale,
                hue: 0.0,
                shadow: true,
            };
            src.draw(r, t, found.default_filter, &mut canvas);
        }
        println!(
            "{r:2}: {} ({:.2} s, {} frames)",
            c.name, c.seconds, c.frames
        );
    }
    pm.save_png(out).expect("write the sheet");
    println!("{out}");
}
