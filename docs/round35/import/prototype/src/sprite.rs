//! Sprite sheets: decode a PNG atlas (Aseprite/TexturePacker shape) and draw one frame on
//! a 600x600 stage, nearest (pixel art) and bicubic, upright and rotated (TexturePacker).
use crate::mean_ms;
use tiny_skia::{
    FilterQuality, Paint, Pattern, Pixmap, PixmapPaint, Rect, SpreadMode, Transform,
};

fn atlas(size: u32, cell: u32) -> Pixmap {
    let mut pm = Pixmap::new(size, size).unwrap();
    let n = size / cell;
    for i in 0..n * n {
        let (x, y) = ((i % n) * cell, (i / n) * cell);
        let mut paint = Paint::default();
        paint.set_color_rgba8((i * 37 % 255) as u8, (i * 91 % 255) as u8, 200, 255);
        let c = cell as f32;
        let r = Rect::from_xywh(x as f32 + c * 0.25, y as f32 + c * 0.1 + (i % 8) as f32, c * 0.5, c * 0.8).unwrap();
        pm.fill_rect(r, &paint, Transform::identity(), None);
    }
    pm
}

pub fn draw_frame(stage: &mut Pixmap, atlas: &Pixmap, src: Rect, rotated: bool, scale: f32, q: FilterQuality) {
    // Map the frame's source rectangle onto the stage (feet at the bottom-centre).
    let (fw, fh) = if rotated { (src.height(), src.width()) } else { (src.width(), src.height()) };
    let dx = 300.0 - fw * scale / 2.0;
    let dy = 560.0 - fh * scale;
    // source px -> stage px
    let mut t = Transform::from_translate(dx, dy).pre_scale(scale, scale);
    if rotated {
        // TexturePacker stores the frame turned 90° clockwise in the atlas.
        t = t.pre_concat(Transform::from_row(0.0, -1.0, 1.0, 0.0, 0.0, fh));
    }
    let shader_t = t.pre_translate(-src.x(), -src.y());
    let pattern = Pattern::new(atlas.as_ref(), SpreadMode::Pad, q, 1.0, shader_t);
    let mut paint = Paint::default();
    paint.shader = pattern;
    paint.anti_alias = false;
    let dst = Rect::from_xywh(0.0, 0.0, src.width(), src.height()).unwrap();
    stage.fill_rect(dst, &paint, t, None);
    let _ = PixmapPaint::default();
}

pub fn run(out: &str) {
    for (size, cell) in [(1024u32, 64u32), (2048, 256), (4096, 512)] {
        let a = atlas(size, cell);
        let png = a.encode_png().unwrap();
        let dec = mean_ms(5, || {
            let _ = Pixmap::decode_png(&png).unwrap();
        });
        let mut stage = Pixmap::new(600, 600).unwrap();
        let scale = 540.0 / cell as f32;
        let mut i = 0u32;
        let n = size / cell;
        let mut frame = |q: FilterQuality, rot: bool, stage: &mut Pixmap| {
            let k = i % (n * n);
            let src = Rect::from_xywh(((k % n) * cell) as f32, ((k / n) * cell) as f32, cell as f32, cell as f32).unwrap();
            stage.fill(tiny_skia::Color::TRANSPARENT);
            draw_frame(stage, &a, src, rot, scale, q);
            i += 1;
        };
        let near = mean_ms(200, || frame(FilterQuality::Nearest, false, &mut stage));
        let bic = mean_ms(200, || frame(FilterQuality::Bicubic, false, &mut stage));
        let rot = mean_ms(200, || frame(FilterQuality::Nearest, true, &mut stage));
        println!(
            "sprite atlas {size}x{size} ({} frames of {cell}px): png {} KB decode {dec:.2} ms; frame x{scale:.2} on 600x600: nearest {near:.3} ms, bicubic {bic:.3} ms, rotated {rot:.3} ms",
            n * n,
            png.len() / 1024
        );
        if size == 1024 {
            stage.save_png(format!("{out}/ours-sprite-frame.png")).unwrap();
        }
    }
}
