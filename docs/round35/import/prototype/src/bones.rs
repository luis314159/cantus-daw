//! DragonBones-shaped skeleton: bones (a tree), slots with image displays from an atlas,
//! a rotation timeline per bone; and a textured mesh drawn as affine-mapped triangles
//! (tiny-skia has no triangle-mesh call: each triangle is a path filled with a pattern).
use crate::mean_ms;
use tiny_skia::{FilterQuality, Paint, PathBuilder, Pattern, Pixmap, Rect, SpreadMode, Transform};

struct Bone {
    parent: Option<usize>,
    x: f32,
    y: f32,
    rot: f32,
    len: f32,
}

fn atlas() -> Pixmap {
    let mut pm = Pixmap::new(512, 512).unwrap();
    for i in 0..16u32 {
        let (x, y) = ((i % 4) * 128, (i / 4) * 128);
        let mut p = Paint::default();
        p.set_color_rgba8(60 + (i * 12) as u8, 120, 220 - (i * 9) as u8, 255);
        p.anti_alias = true;
        let r = Rect::from_xywh(x as f32 + 8.0, y as f32 + 8.0, 112.0, 112.0).unwrap();
        let pth = PathBuilder::from_oval(r).unwrap();
        pm.fill_path(&pth, &p, tiny_skia::FillRule::Winding, Transform::identity(), None);
        p.set_color_rgba8(20, 20, 30, 255);
        let st = tiny_skia::Stroke { width: 6.0, ..Default::default() };
        pm.stroke_path(&pth, &p, &st, Transform::identity(), None);
    }
    pm
}

fn skeleton(n: usize) -> Vec<Bone> {
    (0..n)
        .map(|i| Bone {
            parent: if i == 0 { None } else { Some((i - 1) / 2) },
            x: if i == 0 { 300.0 } else { 40.0 },
            y: if i == 0 { 420.0 } else { 0.0 },
            rot: (i as f32 * 40.0) % 360.0,
            len: 40.0,
        })
        .collect()
}

fn world(bones: &[Bone], t: f32) -> Vec<Transform> {
    let mut w: Vec<Transform> = Vec::with_capacity(bones.len());
    for (i, b) in bones.iter().enumerate() {
        let swing = ((t * 2.0 + i as f32).sin()) * 25.0;
        let local = Transform::from_translate(b.x, b.y).pre_rotate(b.rot + swing);
        w.push(match b.parent {
            Some(p) => w[p].pre_concat(local),
            None => local,
        });
    }
    w
}

fn draw_region(stage: &mut Pixmap, atlas: &Pixmap, region: Rect, t: Transform) {
    let shader_t = t.pre_translate(-region.x() - region.width() / 2.0, -region.y() - region.height() / 2.0);
    let mut paint = Paint::default();
    paint.shader = Pattern::new(atlas.as_ref(), SpreadMode::Pad, FilterQuality::Bilinear, 1.0, shader_t);
    paint.anti_alias = true;
    let dst = Rect::from_xywh(-region.width() / 2.0, -region.height() / 2.0, region.width(), region.height()).unwrap();
    stage.fill_rect(dst, &paint, t, None);
}

/// The affine map taking triangle `s` onto triangle `d` (None when degenerate).
fn tri_map(s: [(f32, f32); 3], d: [(f32, f32); 3]) -> Option<Transform> {
    let (sx1, sy1) = (s[1].0 - s[0].0, s[1].1 - s[0].1);
    let (sx2, sy2) = (s[2].0 - s[0].0, s[2].1 - s[0].1);
    let det = sx1 * sy2 - sx2 * sy1;
    if det.abs() < 1e-6 {
        return None;
    }
    let src = Transform::from_row(sx1, sy1, sx2, sy2, s[0].0, s[0].1).invert()?;
    let (dx1, dy1) = (d[1].0 - d[0].0, d[1].1 - d[0].1);
    let (dx2, dy2) = (d[2].0 - d[0].0, d[2].1 - d[0].1);
    let dst = Transform::from_row(dx1, dy1, dx2, dy2, d[0].0, d[0].1);
    Some(dst.pre_concat(src))
}

fn draw_mesh(stage: &mut Pixmap, atlas: &Pixmap, n: usize, t: f32) -> usize {
    // A 128px region (0,0) as an (n+1)^2 grid, warped by a wave, scaled to ~400px.
    let mut tris = 0;
    let v = |i: usize, j: usize| -> ((f32, f32), (f32, f32)) {
        let (u, w) = (i as f32 / n as f32, j as f32 / n as f32);
        let src = (u * 128.0, w * 128.0);
        let dst = (100.0 + u * 400.0 + (w * 6.0 + t).sin() * 20.0, 100.0 + w * 400.0 + (u * 5.0 + t).cos() * 15.0);
        (src, dst)
    };
    for j in 0..n {
        for i in 0..n {
            let q = [v(i, j), v(i + 1, j), v(i + 1, j + 1), v(i, j + 1)];
            for tri in [[q[0], q[1], q[2]], [q[0], q[2], q[3]]] {
                let Some(m) = tri_map([tri[0].0, tri[1].0, tri[2].0], [tri[0].1, tri[1].1, tri[2].1]) else { continue };
                let mut pb = PathBuilder::new();
                pb.move_to(tri[0].1 .0, tri[0].1 .1);
                pb.line_to(tri[1].1 .0, tri[1].1 .1);
                pb.line_to(tri[2].1 .0, tri[2].1 .1);
                pb.close();
                let Some(path) = pb.finish() else { continue };
                let mut paint = Paint::default();
                paint.shader = Pattern::new(atlas.as_ref(), SpreadMode::Pad, FilterQuality::Bilinear, 1.0, m);
                paint.anti_alias = true;
                stage.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
                tris += 1;
            }
        }
    }
    tris
}

pub fn run(out: &str) {
    let at = atlas();
    let mut stage = Pixmap::new(600, 600).unwrap();
    for n in [16usize, 64] {
        let bones = skeleton(n);
        let mut t = 0.0f32;
        let solve = mean_ms(1000, || {
            let _ = world(&bones, t);
            t += 0.03;
        });
        let draw = mean_ms(200, || {
            stage.fill(tiny_skia::Color::TRANSPARENT);
            let w = world(&bones, t);
            for (i, wt) in w.iter().enumerate() {
                let k = (i % 16) as u32;
                let region = Rect::from_xywh(((k % 4) * 128) as f32, ((k / 4) * 128) as f32, 128.0, 128.0).unwrap();
                draw_region(&mut stage, &at, region, wt.pre_scale(0.5, 0.5));
            }
            t += 0.03;
        });
        println!("bones {n:3} bones+slots: solve {solve:.4} ms, frame 600x600 {draw:.3} ms");
        if n == 16 {
            stage.save_png(format!("{out}/ours-bones.png")).unwrap();
        }
    }
    for n in [4usize, 10, 20] {
        let mut t = 0.0;
        let mut tris = 0;
        let ms = mean_ms(100, || {
            stage.fill(tiny_skia::Color::TRANSPARENT);
            tris = draw_mesh(&mut stage, &at, n, t);
            t += 0.05;
        });
        println!("mesh {tris:4} triangles (~400x400 px): {ms:.3} ms");
        if n == 10 {
            stage.save_png(format!("{out}/ours-mesh.png")).unwrap();
        }
    }
}

pub fn run_own(out: &str) {
    use crate::raster::{quad, tri_fast as tri, V};
    let at = atlas();
    let mut stage = Pixmap::new(600, 600).unwrap();
    // A sprite frame, 128px region scaled to 540px.
    for bil in [false, true] {
        let ms = mean_ms(200, || {
            stage.fill(tiny_skia::Color::TRANSPARENT);
            quad(&mut stage, &at, [(30.0, 30.0), (570.0, 30.0), (570.0, 570.0), (30.0, 570.0)], [0, 0, 128, 128], bil);
        });
        println!("own sprite frame 540x540 {}: {ms:.3} ms", if bil { "bilinear" } else { "nearest" });
    }
    for bil in [false, true] {
        let ms = mean_ms(400, || {
            stage.fill(tiny_skia::Color::TRANSPARENT);
            quad(&mut stage, &at, [(174.0, 318.0), (426.0, 318.0), (426.0, 570.0), (174.0, 570.0)], [0, 0, 128, 128], bil);
        });
        println!("own sprite frame 252x252 (size 1) {}: {ms:.3} ms", if bil { "bilinear" } else { "nearest" });
    }
    stage.save_png(format!("{out}/own-sprite.png")).unwrap();
    for n in [16usize, 64] {
        let bones = skeleton(n);
        let mut t = 0.0f32;
        let ms = mean_ms(200, || {
            stage.fill(tiny_skia::Color::TRANSPARENT);
            let w = world(&bones, t);
            for (i, wt) in w.iter().enumerate() {
                let k = (i % 16) as i32;
                let (rx, ry) = ((k % 4) * 128, (k / 4) * 128);
                let m = wt.pre_scale(0.5, 0.5);
                let c = |x: f32, y: f32| {
                    let mut p = [tiny_skia::Point::from_xy(x, y)];
                    m.map_points(&mut p);
                    (p[0].x, p[0].y)
                };
                quad(&mut stage, &at, [c(-64.0, -64.0), c(64.0, -64.0), c(64.0, 64.0), c(-64.0, 64.0)], [rx, ry, rx + 128, ry + 128], true);
            }
            t += 0.03;
        });
        println!("own bones {n:3} regions bilinear: {ms:.3} ms");
        if n == 16 {
            stage.save_png(format!("{out}/own-bones.png")).unwrap();
        }
    }
    for n in [10usize, 20] {
        let mut t = 0.0f32;
        let ms = mean_ms(100, || {
            stage.fill(tiny_skia::Color::TRANSPARENT);
            let v = |i: usize, j: usize| {
                let (u, w) = (i as f32 / n as f32, j as f32 / n as f32);
                V { x: 100.0 + u * 400.0 + (w * 6.0 + t).sin() * 20.0, y: 100.0 + w * 400.0 + (u * 5.0 + t).cos() * 15.0, u: u * 128.0, v: w * 128.0 }
            };
            for j in 0..n {
                for i in 0..n {
                    tri(&mut stage, &at, [v(i, j), v(i + 1, j), v(i + 1, j + 1)], true, [0, 0, 128, 128]);
                    tri(&mut stage, &at, [v(i, j), v(i + 1, j + 1), v(i, j + 1)], true, [0, 0, 128, 128]);
                }
            }
            t += 0.05;
        });
        println!("own mesh {} triangles (~400x400): {ms:.3} ms", n * n * 2);
        if n == 10 {
            stage.save_png(format!("{out}/own-mesh.png")).unwrap();
        }
    }
}
