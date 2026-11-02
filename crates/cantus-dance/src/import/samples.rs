//! Test characters made by code (ADR 0089: our own test files) — a little original figure
//! with every one of our sixteen moves, written as each format writes it: an Aseprite
//! sheet, a TexturePacker sheet (rotated and trimmed frames), a Lottie file (markers) and a
//! DragonBones skeleton (animations). Tests, the shoots and `examples/write_samples.rs`
//! use them; nothing here is drawn by anyone else.

use cantus_core::inst::dancer::Move;
use cantus_dsp::math;
use serde_json::{json, Value};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// A generated file: its name and bytes.
pub type File = (String, Vec<u8>);

/// The names our generated files give the moves (as people name them).
pub fn move_name(m: Move) -> &'static str {
    m.name()
}

/// Frames a move has in the generated sheets.
pub const SHEET_FRAMES: usize = 4;
/// A sheet frame's size (pixel art).
pub const CELL: (u32, u32) = (48, 64);

/// How far through a move (0…1) puts each limb where: (arm L, arm R, lean, hop) in
/// degrees and pixels — a pure function so every format draws the same dance.
pub fn limb_angles(m: Move, phase: f32) -> (f32, f32, f32, f32) {
    let (s, _) = math::sin_cos(phase * std::f32::consts::TAU);
    let up = (1.0 - (phase * 2.0 - 1.0).abs()).max(0.0);
    match m {
        Move::Idle => (10.0 + s * 3.0, -10.0 - s * 3.0, 0.0, 0.0),
        Move::Bounce => (30.0, -30.0, 0.0, up * 4.0),
        Move::StepLeft => (40.0 * s, -40.0 * s, -8.0, up * 2.0),
        Move::StepRight => (-40.0 * s, 40.0 * s, 8.0, up * 2.0),
        Move::Sway => (20.0, -20.0, s * 12.0, 0.0),
        Move::Clap => (70.0 * up, -70.0 * up, 0.0, 0.0),
        Move::Wave => (10.0, -130.0 + s * 30.0, 0.0, 0.0),
        Move::Jump => (120.0 * up, -120.0 * up, 0.0, up * 14.0),
        Move::Spin => (90.0 + s * 60.0, -90.0 + s * 60.0, s * 10.0, up * 3.0),
        Move::Kick => (30.0, -30.0, -10.0 * up, 0.0),
        Move::Shuffle => (50.0 * s, 50.0 * s, s * 6.0, up * 2.0),
        Move::Point => (10.0, -100.0, 5.0, 0.0),
        Move::ArmsUp => (160.0, -160.0, 0.0, up * 3.0),
        Move::Twist => (60.0 * s, 60.0 * s, s * 15.0, 0.0),
        Move::Bow => (20.0, -20.0, 30.0 * up, 0.0),
        Move::Freeze => (45.0, -120.0, 6.0, 0.0),
    }
}

/// Draw the figure for (move, phase) into `pm` with `t` mapping its 48×64 cell.
pub fn draw_figure(pm: &mut Pixmap, t: Transform, m: Move, phase: f32, hue_shift: u8) {
    let (al, ar, lean, hop) = limb_angles(m, phase);
    let mut paint = Paint {
        anti_alias: false,
        ..Paint::default()
    };
    let body = Transform::from_translate(24.0, 58.0 - hop);
    let (ls, lc) = math::sin_cos(lean.to_radians());
    let lean_t = Transform::from_row(lc, ls, -ls, lc, 0.0, 0.0);
    let base = t.pre_concat(body).pre_concat(lean_t);
    let outline = Color::from_rgba8(30, 24, 40, 255);
    // Legs.
    let mut pb = PathBuilder::new();
    pb.move_to(-5.0, -14.0);
    pb.line_to(-6.0, 0.0);
    pb.move_to(5.0, -14.0);
    pb.line_to(6.0 + if m == Move::Kick { 8.0 } else { 0.0 }, 0.0);
    if let Some(p) = pb.finish() {
        paint.set_color(outline);
        let st = Stroke {
            width: 4.0,
            ..Stroke::default()
        };
        pm.stroke_path(&p, &paint, &st, base, None);
    }
    // Body and head.
    let shirt = Color::from_rgba8(60u8.wrapping_add(hue_shift), 150, 230, 255);
    if let Some(r) = tiny_skia::Rect::from_xywh(-9.0, -34.0, 18.0, 22.0) {
        let p = PathBuilder::from_rect(r);
        paint.set_color(shirt);
        pm.fill_path(&p, &paint, FillRule::Winding, base, None);
    }
    if let Some(p) = PathBuilder::from_circle(0.0, -43.0, 9.0) {
        paint.set_color(Color::from_rgba8(250, 200, 120, 255));
        pm.fill_path(&p, &paint, FillRule::Winding, base, None);
        paint.set_color(outline);
        pm.stroke_path(
            &p,
            &paint,
            &Stroke {
                width: 2.0,
                ..Stroke::default()
            },
            base,
            None,
        );
    }
    // Arms from the shoulders, at their angles.
    for (x, a) in [(-9.0f32, al), (9.0, ar)] {
        let (s, c) = math::sin_cos(a.to_radians());
        let mut pb = PathBuilder::new();
        pb.move_to(x, -32.0);
        pb.line_to(x - s * 12.0 * x.signum(), -32.0 + c * 12.0);
        if let Some(p) = pb.finish() {
            paint.set_color(outline);
            pm.stroke_path(
                &p,
                &paint,
                &Stroke {
                    width: 3.0,
                    ..Stroke::default()
                },
                base,
                None,
            );
        }
    }
}

fn png(pm: &Pixmap) -> Vec<u8> {
    // Straight RGBA for the file (tiny-skia's encoder un-premultiplies).
    pm.encode_png().unwrap_or_default()
}

/// The frames of every move, each a 48×64 pixmap, in `Move::ALL` order.
pub fn frames() -> Vec<(Move, usize, Pixmap)> {
    let mut out = Vec::new();
    for m in Move::ALL {
        for i in 0..SHEET_FRAMES {
            let mut pm = Pixmap::new(CELL.0, CELL.1).unwrap_or_else(unreachable_pixmap);
            draw_figure(
                &mut pm,
                Transform::identity(),
                m,
                i as f32 / SHEET_FRAMES as f32,
                0,
            );
            out.push((m, i, pm));
        }
    }
    out
}

fn unreachable_pixmap() -> Pixmap {
    // 48×64 is never zero; a 1×1 keeps this total without a panic path.
    Pixmap::new(1, 1).unwrap_or_else(|| unreachable!("a 1×1 pixmap"))
}

/// An Aseprite sheet (hash form, `frameTags` per move, frame durations): the JSON and its
/// PNG, named `<base>.json` / `<base>.png`.
pub fn aseprite(base: &str) -> Vec<File> {
    let fr = frames();
    let cols = 8u32;
    let rows = (fr.len() as u32).div_ceil(cols);
    let mut atlas = Pixmap::new(cols * CELL.0, rows * CELL.1).unwrap_or_else(unreachable_pixmap);
    let mut frames_json = serde_json::Map::new();
    for (i, (_, _, pm)) in fr.iter().enumerate() {
        let (x, y) = ((i as u32 % cols) * CELL.0, (i as u32 / cols) * CELL.1);
        atlas.draw_pixmap(
            x as i32,
            y as i32,
            pm.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        frames_json.insert(
            format!("{base} {i}.aseprite"),
            json!({
                "frame": {"x": x, "y": y, "w": CELL.0, "h": CELL.1},
                "rotated": false, "trimmed": false,
                "spriteSourceSize": {"x": 0, "y": 0, "w": CELL.0, "h": CELL.1},
                "sourceSize": {"w": CELL.0, "h": CELL.1},
                "duration": 125
            }),
        );
    }
    let tags: Vec<Value> = Move::ALL
        .iter()
        .enumerate()
        .map(|(k, m)| {
            json!({"name": move_name(*m), "from": k * SHEET_FRAMES,
                   "to": k * SHEET_FRAMES + SHEET_FRAMES - 1, "direction": "forward", "color": "#000000ff"})
        })
        .collect();
    let doc = json!({
        "frames": frames_json,
        "meta": {
            "app": "https://www.aseprite.org/", "version": "1.3",
            "image": format!("{base}.png"), "format": "RGBA8888",
            "size": {"w": atlas.width(), "h": atlas.height()}, "scale": "1",
            "frameTags": tags, "layers": [{"name": "Layer", "opacity": 255, "blendMode": "normal"}],
            "slices": []
        }
    });
    vec![
        (
            format!("{base}.json"),
            serde_json::to_vec_pretty(&doc).unwrap_or_default(),
        ),
        (format!("{base}.png"), png(&atlas)),
    ]
}

/// The tight box of a pixmap's opaque pixels: (x, y, w, h).
fn trim(pm: &Pixmap) -> (u32, u32, u32, u32) {
    let (w, h) = (pm.width(), pm.height());
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            if pm.pixel(x, y).is_some_and(|p| p.alpha() > 0) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    if x1 <= x0 {
        (0, 0, 1, 1)
    } else {
        (x0, y0, x1 - x0, y1 - y0)
    }
}

/// A TexturePacker sheet (array form): every frame trimmed, every other one stored a
/// quarter turn clockwise, named `<move>_<n>.png`; with `animations` lists when asked,
/// else found from the numbered names.
pub fn texturepacker(base: &str, animations: bool) -> Vec<File> {
    let fr = frames();
    // Shelf packing into a 512-wide atlas.
    let mut placed = Vec::new();
    let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
    for (m, i, pm) in &fr {
        let (tx, ty, tw, th) = trim(pm);
        let rotated = (m.index() * SHEET_FRAMES + i) % 2 == 1;
        let (aw, ah) = if rotated { (th, tw) } else { (tw, th) };
        if x + aw > 512 {
            x = 0;
            y += shelf + 1;
            shelf = 0;
        }
        placed.push((*m, *i, (tx, ty, tw, th), rotated, (x, y)));
        x += aw + 1;
        shelf = shelf.max(ah);
    }
    let height = (y + shelf + 1).next_power_of_two();
    let mut atlas = Pixmap::new(512, height).unwrap_or_else(unreachable_pixmap);
    let mut list = Vec::new();
    for ((m, i, (tx, ty, tw, th), rotated, (ax, ay)), (_, _, pm)) in placed.iter().zip(&fr) {
        // Copy the trimmed pixels, turned when rotated: sprite (sx, sy) → atlas
        // (ax + th − 1 − sy, ay + sx).
        for sy in 0..*th {
            for sx in 0..*tw {
                let Some(p) = pm.pixel(tx + sx, ty + sy) else {
                    continue;
                };
                let (dx, dy) = if *rotated {
                    (ax + th - 1 - sy, ay + sx)
                } else {
                    (ax + sx, ay + sy)
                };
                let w = atlas.width();
                let k = ((dy * w + dx) * 4) as usize;
                if let Some(d) = atlas.data_mut().get_mut(k..k + 4) {
                    d.copy_from_slice(&[p.red(), p.green(), p.blue(), p.alpha()]);
                }
            }
        }
        list.push(json!({
            "filename": format!("{}_{:02}.png", move_name(*m), i + 1),
            "frame": {"x": ax, "y": ay, "w": tw, "h": th},
            "rotated": rotated, "trimmed": true,
            "spriteSourceSize": {"x": tx, "y": ty, "w": tw, "h": th},
            "sourceSize": {"w": CELL.0, "h": CELL.1},
            "pivot": {"x": 0.5, "y": 1.0}
        }));
    }
    let mut doc = json!({
        "frames": list,
        "meta": {"app": "https://www.codeandweb.com/texturepacker", "version": "1.0",
                 "image": format!("{base}.png"), "format": "RGBA8888",
                 "size": {"w": 512, "h": height}, "scale": "1"}
    });
    if animations {
        let mut a = serde_json::Map::new();
        for m in Move::ALL {
            let names: Vec<Value> = (0..SHEET_FRAMES)
                .map(|i| json!(format!("{}_{:02}.png", move_name(m), i + 1)))
                .collect();
            a.insert(move_name(m).to_string(), Value::Array(names));
        }
        doc["animations"] = Value::Object(a);
    }
    vec![
        (
            format!("{base}.json"),
            serde_json::to_vec_pretty(&doc).unwrap_or_default(),
        ),
        (format!("{base}.png"), png(&atlas)),
    ]
}

/// Write `files` into `dir`.
pub fn write(dir: &std::path::Path, files: &[File]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    for (name, bytes) in files {
        std::fs::write(dir.join(name), bytes)?;
    }
    Ok(())
}

/// Frames per move in the generated Lottie (half a second at 24 fps: one beat at 120).
pub const LOTTIE_FPM: f32 = 12.0;

fn st(v: Value) -> Value {
    json!({"a": 0, "k": v})
}

/// Keyframes over every move of `f(move, phase)`, four keys per move, eased.
fn keys(f: impl Fn(Move, f32) -> Vec<f32>) -> Value {
    let mut k = Vec::new();
    for (mi, m) in Move::ALL.iter().enumerate() {
        for q in 0..4 {
            let phase = q as f32 / 4.0;
            k.push(json!({
                "t": mi as f32 * LOTTIE_FPM + phase * LOTTIE_FPM,
                "s": f(*m, phase),
                "o": {"x": [0.33], "y": [0.0]}, "i": {"x": [0.67], "y": [1.0]}
            }));
        }
    }
    json!({"a": 1, "k": k})
}

fn tr() -> Value {
    json!({"ty": "tr", "p": st(json!([0, 0])), "a": st(json!([0, 0])), "s": st(json!([100, 100])),
           "r": st(json!(0)), "o": st(json!(100))})
}

fn fill(c: [f32; 3]) -> Value {
    json!({"ty": "fl", "c": st(json!([c[0], c[1], c[2], 1])), "o": st(json!(100)), "r": 1})
}

fn outline(w: f32) -> Value {
    json!({"ty": "st", "c": st(json!([0.12, 0.09, 0.16, 1])), "o": st(json!(100)), "w": st(json!(w)),
           "lc": 2, "lj": 2})
}

fn group(mut items: Vec<Value>) -> Value {
    items.push(tr());
    json!({"ty": "gr", "it": items})
}

fn layer(ind: i64, parent: Option<i64>, name: &str, ks: Value, shapes: Vec<Value>) -> Value {
    let mut l = json!({"ddd": 0, "ty": 4, "ind": ind, "nm": name, "ip": 0,
        "op": LOTTIE_FPM * 16.0, "st": 0, "ks": ks, "shapes": shapes});
    if let Some(p) = parent {
        l["parent"] = json!(p);
    }
    l
}

fn ks(p: Value, r: Value) -> Value {
    json!({"a": st(json!([0, 0])), "p": p, "s": st(json!([100, 100])), "r": r, "o": st(json!(100))})
}

/// The Lottie document of our figure: a null root that hops (a split position), a body
/// that leans (a gradient shirt, rounded rectangles), a head (an ellipse with eyes and a
/// path smile), arms swinging, a trimmed scarf; every move a marker.
pub fn lottie_doc(name: &str) -> Value {
    let skin = [0.98, 0.78, 0.47];
    let root_ks = json!({
        "a": st(json!([0, 0])),
        "p": {"s": true, "x": st(json!(256)), "y": keys(|m, ph| vec![470.0 - limb_angles(m, ph).3 * 3.0])},
        "s": st(json!([100, 100])), "o": st(json!(100))
    });
    let shirt = json!({"ty": "gf", "o": st(json!(100)), "r": 1, "t": 1,
        "s": st(json!([0, -60])), "e": st(json!([0, 60])),
        "g": {"p": 2, "k": st(json!([0, 0.30, 0.62, 0.95, 1, 0.16, 0.38, 0.80]))}});
    let head = layer(
        2,
        Some(1),
        "head",
        ks(
            st(json!([0, -190])),
            keys(|m, ph| vec![limb_angles(m, ph).2 * 0.5]),
        ),
        vec![
            group(vec![
                json!({"ty": "el", "p": st(json!([-16, -6])), "s": st(json!([12, 16]))}),
                fill([0.1, 0.08, 0.14]),
            ]),
            group(vec![
                json!({"ty": "el", "p": st(json!([16, -6])), "s": st(json!([12, 16]))}),
                fill([0.1, 0.08, 0.14]),
            ]),
            group(vec![
                json!({"ty": "sh", "ks": st(json!({"c": false, "v": [[-12, 14], [12, 14]], "i": [[0, 0], [-6, 8]], "o": [[6, 8], [0, 0]]}))}),
                outline(4.0),
            ]),
            group(vec![
                json!({"ty": "el", "p": st(json!([0, 0])), "s": st(json!([84, 78]))}),
                fill(skin),
                outline(5.0),
            ]),
        ],
    );
    let scarf = layer(
        3,
        Some(1),
        "scarf",
        ks(st(json!([0, -150])), st(json!(0))),
        vec![group(vec![
            json!({"ty": "sh", "ks": st(json!({"c": false, "v": [[-30, 0], [30, 0]], "i": [[0, 0], [-10, 10]], "o": [[10, 10], [0, 0]]}))}),
            json!({"ty": "tm", "s": st(json!(0)), "e": keys(|m, ph| vec![70.0 + 30.0 * (1.0 - limb_angles(m, ph).3 / 14.0)]), "o": st(json!(0)), "m": 1}),
            json!({"ty": "st", "c": st(json!([0.92, 0.3, 0.35, 1])), "o": st(json!(100)), "w": st(json!(9)), "lc": 2, "lj": 2}),
        ])],
    );
    let body = layer(
        4,
        Some(1),
        "body",
        ks(
            st(json!([0, -95])),
            keys(|m, ph| vec![limb_angles(m, ph).2]),
        ),
        vec![group(vec![
            json!({"ty": "rc", "p": st(json!([0, 0])), "s": st(json!([72, 110])), "r": st(json!(24))}),
            shirt,
            outline(5.0),
        ])],
    );
    // Lottie draws its first layer on top: the head, the scarf, the body, then the limbs.
    let mut layers = vec![head, scarf, body];
    for (k, side) in [(5i64, -1.0f32), (6, 1.0)] {
        let pick = move |m: Move, ph: f32| {
            let (l, r, _, _) = limb_angles(m, ph);
            vec![if side < 0.0 { l } else { r }]
        };
        layers.push(layer(
            k,
            Some(4),
            if side < 0.0 { "arm_l" } else { "arm_r" },
            ks(st(json!([side * 40.0, -40.0])), keys(pick)),
            vec![
                group(vec![json!({"ty": "el", "p": st(json!([0, 62])), "s": st(json!([22, 22]))}), fill(skin), outline(4.0)]),
                group(vec![
                    json!({"ty": "rc", "p": st(json!([0, 28])), "s": st(json!([18, 60])), "r": st(json!(9))}),
                    fill([0.3, 0.62, 0.95]),
                    outline(4.0),
                ]),
            ],
        ));
        layers.push(layer(
            k + 2,
            Some(1),
            if side < 0.0 { "leg_l" } else { "leg_r" },
            ks(st(json!([side * 18.0, -45.0])), keys(move |m, ph| vec![side * limb_angles(m, ph).2 * 0.3])),
            vec![
                group(vec![
                    json!({"ty": "rc", "p": st(json!([side * 5.0, 42])), "s": st(json!([34, 14])), "r": st(json!(7))}),
                    fill([0.2, 0.18, 0.26]),
                    outline(4.0),
                ]),
                group(vec![
                    json!({"ty": "rc", "p": st(json!([0, 18])), "s": st(json!([20, 46])), "r": st(json!(10))}),
                    fill([0.28, 0.28, 0.38]),
                    outline(4.0),
                ]),
            ],
        ));
    }
    layers.push(json!({"ddd": 0, "ty": 3, "ind": 1, "nm": "root", "ip": 0,
        "op": LOTTIE_FPM * 16.0, "st": 0, "ks": root_ks}));
    let markers: Vec<Value> = Move::ALL
        .iter()
        .enumerate()
        .map(|(i, m)| json!({"cm": move_name(*m), "tm": i as f32 * LOTTIE_FPM, "dr": LOTTIE_FPM}))
        .collect();
    json!({"v": "5.7.4", "fr": 24, "ip": 0, "op": LOTTIE_FPM * 16.0, "w": 512, "h": 512,
        "nm": name, "ddd": 0, "assets": [], "layers": layers, "markers": markers,
        "meta": {"g": "Cantus tests", "a": "Cantus", "d": "A test figure made by code", "k": "CC0-1.0"}})
}

/// The figure as a Lottie JSON file.
pub fn lottie(base: &str) -> Vec<File> {
    vec![(
        format!("{base}.json"),
        serde_json::to_vec(&lottie_doc(base)).unwrap_or_default(),
    )]
}

/// The figure as a `.lottie` package (a zip: the manifest, the animation, and an image
/// layer — a little sun from `images/` — riding on its head).
pub fn dotlottie(base: &str) -> Vec<File> {
    use std::io::Write;
    let mut doc = lottie_doc(base);
    let mut sun = Pixmap::new(32, 32).unwrap_or_else(unreachable_pixmap);
    if let Some(p) = PathBuilder::from_circle(16.0, 16.0, 12.0) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(255, 210, 60, 255));
        sun.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    }
    doc["assets"] =
        json!([{"id": "img_0", "w": 32, "h": 32, "u": "/images/", "p": "sun.png", "e": 0}]);
    if let Some(layers) = doc["layers"].as_array_mut() {
        layers.insert(
            0,
            json!({"ddd": 0, "ty": 2, "ind": 20, "nm": "sun", "refId": "img_0",
            "ip": 0, "op": LOTTIE_FPM * 16.0, "st": 0, "parent": 2,
            "ks": {"a": st(json!([16, 16])), "p": st(json!([40, -50])), "s": st(json!([100, 100])),
                   "r": st(json!(0)), "o": st(json!(100))}}),
        );
    }
    let manifest = json!({"version": "1", "generator": "Cantus tests", "author": "Cantus",
        "license": "CC0-1.0", "animations": [{"id": base}]});
    let mut out = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut out));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let entries: [(String, Vec<u8>); 3] = [
            (
                "manifest.json".into(),
                serde_json::to_vec(&manifest).unwrap_or_default(),
            ),
            (
                format!("animations/{base}.json"),
                serde_json::to_vec(&doc).unwrap_or_default(),
            ),
            ("images/sun.png".into(), png(&sun)),
        ];
        for (name, bytes) in entries {
            if zip.start_file(name, opts).is_ok() {
                let _ = zip.write_all(&bytes);
            }
        }
        let _ = zip.finish();
    }
    vec![(format!("{base}.lottie"), out)]
}

/// DragonBones frames per move (half a second at 24 fps).
pub const DB_FPM: f32 = 12.0;

fn fill_shape(pm: &mut Pixmap, path: Option<tiny_skia::Path>, rgba: [u8; 4], outline: bool) {
    let Some(p) = path else { return };
    let mut paint = Paint {
        anti_alias: true,
        ..Paint::default()
    };
    paint.set_color(Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
    pm.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
    if outline {
        paint.set_color(Color::from_rgba8(30, 24, 40, 255));
        let st = Stroke {
            width: 4.0,
            ..Stroke::default()
        };
        pm.stroke_path(&p, &paint, &st, Transform::identity(), None);
    }
}

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let k = r * 0.552_284_8;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

/// The figure as a DragonBones skeleton (`<base>_ske.json`) with its atlas
/// (`<base>_tex.json`, `<base>_tex.png`): image slots, a weighted mesh scarf (deformed in
/// `wave`), a second head display (a wink, in `freeze`), a z-order change (in `spin`), a
/// texture stored rotated, every move an animation.
pub fn dragonbones(base: &str) -> Vec<File> {
    // The atlas: (name, x, y, w, h, rotated).
    let tex: [(&str, f32, f32, f32, f32, bool); 7] = [
        ("head", 0.0, 0.0, 88.0, 82.0, false),
        ("head_wink", 88.0, 0.0, 88.0, 82.0, false),
        ("body", 176.0, 0.0, 80.0, 118.0, false),
        ("arm", 0.0, 84.0, 26.0, 88.0, false),
        ("leg", 28.0, 84.0, 40.0, 60.0, true),
        ("scarf", 100.0, 120.0, 64.0, 16.0, false),
        ("spare", 168.0, 120.0, 8.0, 8.0, false),
    ];
    let mut atlas = Pixmap::new(256, 256).unwrap_or_else(unreachable_pixmap);
    {
        let p = &mut atlas;
        for (k, (x, y)) in [(0.0f32, 0.0f32), (88.0, 0.0)].into_iter().enumerate() {
            let oval = tiny_skia::Rect::from_xywh(x + 3.0, y + 3.0, 82.0, 76.0)
                .and_then(PathBuilder::from_oval);
            fill_shape(p, oval, [250, 200, 120, 255], true);
            let eye = |p: &mut Pixmap, ex: f32, closed: bool| {
                if closed {
                    let mut pb = PathBuilder::new();
                    pb.move_to(x + ex - 6.0, y + 34.0);
                    pb.line_to(x + ex + 6.0, y + 34.0);
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color(Color::from_rgba8(30, 24, 40, 255));
                        p.stroke_path(
                            &path,
                            &paint,
                            &Stroke {
                                width: 3.0,
                                ..Stroke::default()
                            },
                            Transform::identity(),
                            None,
                        );
                    }
                } else {
                    fill_shape(
                        p,
                        PathBuilder::from_circle(x + ex, y + 34.0, 6.0),
                        [30, 24, 40, 255],
                        false,
                    );
                }
            };
            eye(p, 30.0, false);
            eye(p, 58.0, k == 1);
        }
        fill_shape(
            p,
            rounded(179.0, 3.0, 74.0, 112.0, 22.0),
            [76, 156, 240, 255],
            true,
        );
        fill_shape(
            p,
            rounded(3.0, 87.0, 20.0, 60.0, 9.0),
            [76, 156, 240, 255],
            true,
        );
        fill_shape(
            p,
            PathBuilder::from_circle(13.0, 158.0, 11.0),
            [250, 200, 120, 255],
            true,
        );
        // The leg, drawn upright into a scratch then stored a quarter turn clockwise.
        let mut leg = Pixmap::new(40, 60).unwrap_or_else(unreachable_pixmap);
        fill_shape(
            &mut leg,
            rounded(10.0, 2.0, 20.0, 44.0, 9.0),
            [70, 72, 98, 255],
            true,
        );
        fill_shape(
            &mut leg,
            rounded(3.0, 44.0, 34.0, 14.0, 6.0),
            [50, 46, 66, 255],
            true,
        );
        for sy in 0..60u32 {
            for sx in 0..40u32 {
                let Some(c) = leg.pixel(sx, sy) else { continue };
                // Sprite (sx, sy) → atlas (x + h − 1 − sy, y + sx), h = 60.
                let (dx, dy) = (28 + 59 - sy, 84 + sx);
                let i = ((dy * 256 + dx) * 4) as usize;
                if let Some(d) = p.data_mut().get_mut(i..i + 4) {
                    d.copy_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
                }
            }
        }
        fill_shape(
            p,
            rounded(101.0, 121.0, 62.0, 14.0, 6.0),
            [236, 76, 90, 255],
            false,
        );
    }
    let sub: Vec<Value> = tex
        .iter()
        .map(|(n, x, y, w, h, r)| {
            let mut t = json!({"name": n, "x": x, "y": y, "width": w, "height": h});
            if *r {
                t["rotated"] = json!(true);
            }
            t
        })
        .collect();
    let tex_json = json!({"name": base, "imagePath": format!("{base}_tex.png"), "width": 256, "height": 256, "SubTexture": sub});

    let bone = |n: &str, parent: Option<&str>, x: f32, y: f32| {
        let mut b = json!({"name": n, "length": 30, "transform": {"x": x, "y": y}});
        if let Some(p) = parent {
            b["parent"] = json!(p);
        }
        b
    };
    let bones = vec![
        bone("root", None, 0.0, 0.0),
        bone("body", Some("root"), 0.0, -95.0),
        bone("head", Some("body"), 0.0, -95.0),
        bone("arm_l", Some("body"), -40.0, -40.0),
        bone("arm_r", Some("body"), 40.0, -40.0),
        bone("leg_l", Some("root"), -18.0, -45.0),
        bone("leg_r", Some("root"), 18.0, -45.0),
    ];
    let slot_names = ["leg_l", "leg_r", "arm_l", "body", "scarf", "head", "arm_r"];
    let slots: Vec<Value> = slot_names
        .iter()
        .map(|s| {
            let parent = if *s == "scarf" { "body" } else { s };
            json!({"name": s, "parent": parent, "displayIndex": 0})
        })
        .collect();
    let img = |n: &str, x: f32, y: f32| json!({"name": n, "transform": {"x": x, "y": y}});
    // The scarf: a 5×2 grid across the neck, weighted from the body (bottom row) to the
    // head (top row); its vertices in the body bone's space at bind time.
    let (mut verts, mut uvs, mut weights) = (Vec::new(), Vec::new(), Vec::new());
    for row in 0..2 {
        for col in 0..5 {
            let (u, v) = (col as f32 / 4.0, row as f32);
            verts.push(json!(-32.0 + u * 64.0));
            verts.push(json!(-60.0 + v * 16.0));
            uvs.push(json!(u));
            uvs.push(json!(v));
            // Bones by index: body 1, head 2.
            let top = if row == 0 { 0.6 } else { 0.1 };
            weights.extend([json!(2), json!(1), json!(1.0 - top), json!(2), json!(top)]);
        }
    }
    let mut tris = Vec::new();
    for col in 0..4u32 {
        let (a, b, c, d) = (col, col + 1, col + 6, col + 5);
        tris.extend([a, b, c, a, c, d].map(|i| json!(i)));
    }
    // The bones' bind (setup world) matrices: body at (0, −95), head at (0, −190).
    let scarf = json!({"type": "mesh", "name": "scarf", "path": "scarf", "width": 64, "height": 16,
        "vertices": verts, "uvs": uvs, "triangles": tris, "weights": weights,
        "slotPose": [1, 0, 0, 1, 0, -95], "bonePose": [1, 1, 0, 0, 1, 0, -95, 2, 1, 0, 0, 1, 0, -190]});
    let skin = json!([{"name": "", "slot": [
        {"name": "leg_l", "display": [img("leg", 0.0, 22.0)]},
        {"name": "leg_r", "display": [img("leg", 0.0, 22.0)]},
        {"name": "arm_l", "display": [img("arm", 0.0, 32.0)]},
        {"name": "arm_r", "display": [img("arm", 0.0, 32.0)]},
        {"name": "body", "display": [img("body", 0.0, 0.0)]},
        {"name": "scarf", "display": [scarf]},
        {"name": "head", "display": [img("head", 0.0, 0.0), img("head_wink", 0.0, 0.0)]}
    ]}]);
    let rot_frames = |f: &dyn Fn(Move, f32) -> f32, m: Move| -> Value {
        let mut k: Vec<Value> = (0..4)
            .map(|q| json!({"duration": DB_FPM / 4.0, "tweenEasing": 0, "rotate": f(m, q as f32 / 4.0)}))
            .collect();
        k.push(json!({"duration": 0, "rotate": f(m, 0.0)}));
        Value::Array(k)
    };
    let animations: Vec<Value> = Move::ALL
        .iter()
        .map(|m| {
            let m = *m;
            let hop: Vec<Value> = (0..4)
                .map(|q| json!({"duration": DB_FPM / 4.0, "curve": [0.33, 0.0, 0.67, 1.0], "y": -limb_angles(m, q as f32 / 4.0).3 * 3.0}))
                .chain([json!({"duration": 0, "y": 0})])
                .collect();
            let mut a = json!({"name": move_name(m), "duration": DB_FPM, "playTimes": 0, "bone": [
                {"name": "root", "translateFrame": hop},
                {"name": "body", "rotateFrame": rot_frames(&|m, p| limb_angles(m, p).2, m)},
                {"name": "head", "rotateFrame": rot_frames(&|m, p| limb_angles(m, p).2 * 0.5, m)},
                {"name": "arm_l", "rotateFrame": rot_frames(&|m, p| limb_angles(m, p).0, m)},
                {"name": "arm_r", "rotateFrame": rot_frames(&|m, p| -limb_angles(m, p).1, m)}
            ]});
            if m == Move::Wave {
                let wobble: Vec<Value> = (0..3)
                    .map(|q| {
                        let o: Vec<Value> = (0..20).map(|i| json!(if i % 2 == 1 { (q as f32 - 1.0) * 4.0 } else { 0.0 })).collect();
                        json!({"duration": DB_FPM / 2.0, "tweenEasing": 0, "offset": 0, "vertices": o})
                    })
                    .collect();
                a["ffd"] = json!([{"name": "scarf", "slot": "scarf", "skin": "", "frame": wobble}]);
            }
            if m == Move::Freeze {
                a["slot"] = json!([{"name": "head", "displayFrame": [{"duration": DB_FPM / 2.0, "value": 0}, {"duration": DB_FPM / 2.0, "value": 1}]}]);
            }
            if m == Move::Spin {
                // arm_l (slot 2) moves above everything for the second half.
                a["zOrder"] = json!({"frame": [{"duration": DB_FPM / 2.0}, {"duration": DB_FPM / 2.0, "zOrder": [2, 4]}]});
            }
            a
        })
        .collect();
    let ske = json!({"frameRate": 24, "name": base, "version": "5.5", "compatibleVersion": "5.5",
        "armature": [{"type": "Armature", "frameRate": 24, "name": "figure",
            "aabb": {"x": -60, "y": -240, "width": 120, "height": 240},
            "bone": bones, "slot": slots, "skin": skin, "animation": animations,
            "defaultActions": [{"gotoAndPlay": "idle"}]}]});
    vec![
        (
            format!("{base}_ske.json"),
            serde_json::to_vec_pretty(&ske).unwrap_or_default(),
        ),
        (
            format!("{base}_tex.json"),
            serde_json::to_vec_pretty(&tex_json).unwrap_or_default(),
        ),
        (format!("{base}_tex.png"), png(&atlas)),
    ]
}
