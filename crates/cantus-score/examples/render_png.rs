//! The reference rasteriser: a display list filled from the fonts' own outlines with
//! tiny-skia — no egui, so the engraving can be looked at seconds after a change.
//!
//! ```text
//! cargo run -p cantus-score --example render_png -- <dir> [passage] [px-per-space]
//! ```
//!
//! Writes `<passage>-wrap.png` (systems wrapped to 120 spaces) and `<passage>-page.png`
//! (the first A4 page) for each golden passage.

use cantus_score::build::{build, Song};
use cantus_score::display::{Anchor, DisplayList, Item};
use cantus_score::layout::{engrave, Format, LayoutOptions, Paper};
use cantus_score::outline;
use cantus_score::text::{TextFaces, TextMeasure};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

static REGULAR: &[u8] = include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSans-Regular.ttf");
static BOLD: &[u8] = include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSans-SemiBold.ttf");
static ITALIC: &[u8] =
    include_bytes!("../../cantus-ui/assets/fonts/IBMPlexSansCondensed-Italic.ttf");

fn faces() -> TextFaces {
    TextFaces {
        regular: REGULAR,
        bold: BOLD,
        italic: ITALIC,
    }
}

fn fill_polys(pm: &mut Pixmap, polys: &[Vec<[f32; 2]>], k: f32, paint: &Paint) {
    let mut pb = PathBuilder::new();
    for c in polys {
        if c.len() < 3 {
            continue;
        }
        pb.move_to(c[0][0] * k, c[0][1] * k);
        for p in &c[1..] {
            pb.line_to(p[0] * k, p[1] * k);
        }
        pb.close();
    }
    if let Some(path) = pb.finish() {
        pm.fill_path(&path, paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn render(list: &DisplayList, k: f32, measure: &TextMeasure) -> Pixmap {
    let w = (list.width * k).ceil().max(1.0) as u32;
    let h = (list.height * k).ceil().max(1.0) as u32;
    let mut pm = Pixmap::new(w, h).expect("pixmap");
    pm.fill(Color::WHITE);
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK);
    paint.anti_alias = true;
    let faces = faces();
    for it in &list.items {
        let mut polys: Vec<Vec<[f32; 2]>> = Vec::new();
        match it {
            Item::Glyph {
                glyph, x, y, scale, ..
            } => {
                for c in outline::contours(*glyph) {
                    polys.push(
                        c.iter()
                            .map(|p| [x + p[0] * scale, y + p[1] * scale])
                            .collect(),
                    );
                }
            }
            Item::Line {
                x0, y0, x1, y1, w, ..
            } => {
                let (dx, dy) = (x1 - x0, y1 - y0);
                let l = (dx * dx + dy * dy).sqrt().max(1e-6);
                let (nx, ny) = (-dy / l * w / 2.0, dx / l * w / 2.0);
                polys.push(vec![
                    [x0 + nx, y0 + ny],
                    [x1 + nx, y1 + ny],
                    [x1 - nx, y1 - ny],
                    [x0 - nx, y0 - ny],
                ]);
            }
            Item::Dashed {
                x0,
                y0,
                x1,
                w,
                dash,
                gap,
            } => {
                let mut x = *x0;
                while x < *x1 {
                    let e = (x + dash).min(*x1);
                    polys.push(vec![
                        [x, y0 - w / 2.0],
                        [e, y0 - w / 2.0],
                        [e, y0 + w / 2.0],
                        [x, y0 + w / 2.0],
                    ]);
                    x = e + gap;
                }
            }
            Item::Beam {
                x0, y0, x1, y1, h, ..
            } => {
                polys.push(vec![[*x0, *y0], [*x1, *y1], [*x1, y1 + h], [*x0, y0 + h]]);
            }
            Item::Curve {
                p, end, mid, up, ..
            } => {
                let n = 32;
                let pt = |t: f32| {
                    let u = 1.0 - t;
                    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                    [
                        a * p[0][0] + b * p[1][0] + c * p[2][0] + d * p[3][0],
                        a * p[0][1] + b * p[1][1] + c * p[2][1] + d * p[3][1],
                    ]
                };
                let side = if *up { -1.0 } else { 1.0 };
                let mut outer = Vec::new();
                let mut inner = Vec::new();
                for i in 0..=n {
                    let t = i as f32 / n as f32;
                    let q = pt(t);
                    let a = pt((t - 0.01).max(0.0));
                    let b = pt((t + 0.01).min(1.0));
                    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                    let l = (dx * dx + dy * dy).sqrt().max(1e-6);
                    let (mut nx, mut ny) = (-dy / l, dx / l);
                    if ny * side < 0.0 {
                        nx = -nx;
                        ny = -ny;
                    }
                    let w = end + (mid - end) * 4.0 * t * (1.0 - t);
                    outer.push([q[0] + nx * w / 2.0, q[1] + ny * w / 2.0]);
                    inner.push([q[0] - nx * w / 2.0, q[1] - ny * w / 2.0]);
                }
                inner.reverse();
                outer.extend(inner);
                polys.push(outer);
            }
            Item::Text {
                text,
                x,
                y,
                size,
                face,
                anchor,
            } => {
                let bytes = faces.bytes(*face);
                let w = measure.width(text, *size, *face);
                let mut at = match anchor {
                    Anchor::Left => *x,
                    Anchor::Centre => x - w / 2.0,
                    Anchor::Right => x - w,
                };
                for c in text.chars() {
                    for cont in outline::char_contours(bytes, c, *size) {
                        polys.push(cont.iter().map(|p| [at + p[0], y + p[1]]).collect());
                    }
                    at += measure.width(&c.to_string(), *size, *face);
                }
            }
            Item::Brace { x, y0, y1 } => {
                for t in outline::brace(*x, *y0, *y1).chunks(3) {
                    polys.push(t.to_vec());
                }
            }
        }
        fill_polys(&mut pm, &polys, k, &paint);
    }
    pm
}

fn main() {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    let only = std::env::args().nth(2).filter(|s| s != "all");
    let k: f32 = std::env::args()
        .nth(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(12.0);
    std::fs::create_dir_all(&dir).expect("the folder");
    let measure = TextMeasure::new(Some(faces()));
    for p in cantus_score::samples::all() {
        if only.as_deref().is_some_and(|o| o != p.name) {
            continue;
        }
        let song = Song {
            title: p.title.into(),
            composer: "Cantus".into(),
            tempo: &p.tempo,
            key: p.key,
        };
        let score = build(&song, &p.parts, None);
        for (name, format, header) in [
            ("wrap", Format::Wrap { width: 120.0 }, false),
            ("page", Format::Pages(Paper::A4), true),
        ] {
            let e = engrave(
                &score,
                &LayoutOptions {
                    format,
                    text: measure.clone(),
                    header,
                },
            );
            let scale = if name == "page" { k * 0.6 } else { k };
            let pm = render(&e.pages[0].list, scale, &measure);
            let path = dir.join(format!("{}-{name}.png", p.name));
            pm.save_png(&path).expect("png");
            println!("{}", path.display());
        }
    }
}
