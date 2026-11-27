//! r42:arcus-polish3 — the concert light measured (the owner, 2026-11-25: "Aún le falta lo
//! de la luz, compara con las imágenes reales"). The Arcus editor is shot through the app
//! with each realistic violinist on the concert stage (Concert, the default), Night and
//! Paper, and the picture is measured where the figure's mesh says its parts are: the face
//! (the head's skin), the hair, the clothes' edge against the hall, the hall itself. Each
//! measure must sit in the band measured from the photographs the owner chose (ref-04,
//! ref-06, ref-10; `tools/arcus-figures/light_measure.py refs`, linear light):
//!
//! | measure | ref-04 | ref-06 | ref-10 | band |
//! |---|---|---|---|---|
//! | the face's mean luminance | 0.28 | 0.19 | 0.09 | 0.08 … 0.36 |
//! | its warmth, R/B · R/G | 2.9 · 2.0 | 3.5 · 2.2 | 3.8 · 2.6 | 2.3 … 4.8 · 1.6 … 3.0 |
//! | its two sides (lit third over shadow third) | 3.3 | 1.4 | 1.5 | 1.35 … 4.5 |
//! | the face over the hall | 46 | 114 | 4.1 (a lit wall) | 10 … 200 |
//! | the picture's contrast (p99 / p5) | 248 | 2219 | 1812 | 150 … 5000 |
//! | the hair's edge over the hall (p95) | 100 | 57 | 1.8 (black hair) | ≥ 8 |
//! | the clothes' edge (p95 over the cloth's median) | 1.5 | — | 1.7 | ≥ 1.4 |
//! | the dark clothes' silhouette below the face (edge p90 against the hall beside it) | 4.7 | 2.2 | 2.0 | ≥ 2.0 (each third ≥ 1.6) |
//!
//! The Drawn picture's own stage light is held to the shape of it: the face lit over the
//! hall, a shadow side, a rim. `CANTUS_LIGHT_DIR=<folder>` saves each shot.

use cantus_core::inst::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::scene::bowed::Bowed;
use cantus_ui::scene::bowed_hold::{Hold, Holder};
use cantus_ui::scene::bowed_mesh::Role;
use cantus_ui::scene::{self, SceneBackend};
use cantus_ui::views::inst::arcus::probe;
use cantus_ui::{views, welcome_project, CantusApp};
use egui::Pos2;

struct Mask {
    w: usize,
    h: usize,
    m: Vec<bool>,
}

impl Mask {
    fn new(w: usize, h: usize) -> Mask {
        Mask {
            w,
            h,
            m: vec![false; w * h],
        }
    }

    fn get(&self, x: isize, y: isize) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.w
            && (y as usize) < self.h
            && self.m[y as usize * self.w + x as usize]
    }

    fn tri(&mut self, p: [Pos2; 3]) {
        let x0 = p
            .iter()
            .map(|q| q.x)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as usize;
        let x1 = p
            .iter()
            .map(|q| q.x)
            .fold(f32::MIN, f32::max)
            .ceil()
            .max(0.0) as usize;
        let y0 = p
            .iter()
            .map(|q| q.y)
            .fold(f32::MAX, f32::min)
            .floor()
            .max(0.0) as usize;
        let y1 = p
            .iter()
            .map(|q| q.y)
            .fold(f32::MIN, f32::max)
            .ceil()
            .max(0.0) as usize;
        let edge =
            |a: Pos2, b: Pos2, c: Pos2| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        let area = edge(p[0], p[1], p[2]);
        if area.abs() < 1e-6 {
            return;
        }
        for y in y0..y1.min(self.h) {
            for x in x0..x1.min(self.w) {
                let c = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                let (a, b, d) = (
                    edge(p[1], p[2], c) / area,
                    edge(p[2], p[0], c) / area,
                    edge(p[0], p[1], c) / area,
                );
                if a >= 0.0 && b >= 0.0 && d >= 0.0 {
                    self.m[y * self.w + x] = true;
                }
            }
        }
    }

    fn seg(&mut self, a: Pos2, b: Pos2, r: f32) {
        let d = b - a;
        let n = egui::vec2(-d.y, d.x).normalized() * r;
        self.tri([a - n, a + n, b + n]);
        self.tri([a - n, b + n, b - n]);
    }

    fn grow(&self, k: isize) -> Mask {
        let mut out = Mask::new(self.w, self.h);
        let r = k.abs();
        for y in 0..self.h as isize {
            for x in 0..self.w as isize {
                let (mut any, mut all) = (false, true);
                for dy in -r..=r {
                    for dx in -r..=r {
                        let g = self.get(x + dx, y + dy);
                        any |= g;
                        all &= g;
                    }
                }
                out.m[y as usize * self.w + x as usize] = if k > 0 { any } else { all };
            }
        }
        out
    }
}

fn lin(u: u8) -> f32 {
    let a = u as f32 / 255.0;
    if a <= 0.04045 {
        a / 12.92
    } else {
        ((a + 0.055) / 1.055).powf(2.4)
    }
}

/// What a shot's picture measures (linear light).
#[derive(Debug)]
struct Measures {
    face: f32,
    r_b: f32,
    r_g: f32,
    sides: f32,
    face_over_hall: f32,
    contrast: f32,
    hair_edge: f32,
    cloth_edge: f32,
    /// The dark clothes' silhouette against the hall right behind it, below the face (the
    /// shoulders, the arms, the bow arm): the brighter of the cloth's edge (p90) and the hall
    /// next to it (median) over the darker — the whole band, and its worst third across.
    silhouette: f32,
    silhouette_worst: f32,
}

fn pct(v: &mut [f32], p: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f32 * p).round() as usize]
}

fn measure(hold: Hold, dark: bool, be: SceneBackend) -> Measures {
    scene::backend::set_default(be);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::default_theme()
    } else {
        cantus_ui::theme::default_light_theme()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    let h = Holder::of(hold);
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.settings.arcus_holder = h;
    app.settings.figure_quality = cantus_ui::scene::figure::Quality::High;
    assert_eq!(
        app.settings.figure_light,
        cantus_ui::scene::figure::Lighting::Concert,
        "Concert is the default"
    );
    app.demo_editor("arcus");
    let t = app.inst.open.expect("an Arcus editor");
    app.set_inst_value(t, "inst", 2.0, false);
    views::inst::arcus::demo_feed(&mut app);
    let size = views::inst::editor_size(InstrumentKind::BowedString);
    let save = std::env::var_os("CANTUS_LIGHT_DIR").map(|d| {
        std::path::PathBuf::from(d).join(format!(
            "{hold:?}-{}-{be:?}.png",
            if dark { "night" } else { "paper" }
        ))
    });
    probe::record(true);
    let canvas = cantus_ui::raster::shoot(&ctx, size, 1.0, 8, save.as_deref(), |ui| {
        views::inst::window_in(&mut app, ui)
    });
    let arms = probe::arms();
    let bow = probe::bow();
    probe::record(false);
    let sc = &app.inst.arcus_scenes[&t.0].0;
    let built = sc.built().expect("a picture");
    let rect = sc.rect().expect("painted");
    let k = egui::vec2(rect.width() / built.size.x, rect.height() / built.size.y);
    let to_px = |w: cantus_ui::scene::geo::V3| {
        let q = built.cam.p(w);
        rect.min + egui::vec2(q.x * k.x, q.y * k.y)
    };
    let (w, hh) = (canvas.width, canvas.height);
    let mut subject = Mask::new(w, hh);
    let mut face = Mask::new(w, hh);
    let mut hair = Mask::new(w, hh);
    let mut cloth = Mask::new(w, hh);
    let mesh = cantus_ui::scene::bowed_mesh::mesh_for(Bowed::Violin, h);
    for (_, p) in &mesh.patches {
        for c in p.cells() {
            let q = c.map(|i| to_px(p.pts[i]));
            subject.tri([q[0], q[1], q[2]]);
            subject.tri([q[0], q[2], q[3]]);
        }
    }
    for (_, r) in &mesh.rods {
        let (a, e) = (to_px(r.a), to_px(r.b));
        subject.seg(a, e, 3.0);
    }
    // The head's skin: the skin above the chin (the forehead's top less 23 cm).
    let top = mesh
        .tris
        .iter()
        .filter(|(r, _)| matches!(r, Role::Skin(_)))
        .flat_map(|(_, t)| t.pts.iter().map(|p| p.y))
        .fold(f32::MIN, f32::max);
    for (role, t) in &mesh.tris {
        for tri in &t.idx {
            let q = tri.map(|i| to_px(t.pts[i as usize]));
            subject.tri(q);
            let head = tri.iter().all(|&i| t.pts[i as usize].y > top - 23.0);
            match role {
                Role::Skin(_) if head => face.tri(q),
                Role::Card(_) | Role::Scalp(_) | Role::Hair | Role::HairFair | Role::Shell(_) => {
                    hair.tri(q)
                }
                Role::Velvet | Role::Wool => cloth.tri(q),
                _ => {}
            }
        }
    }
    // The live arms (their sleeves and hands) and the bow, as drawn.
    let mut dark = Mask::new(w, hh);
    for arm in &arms {
        for t in &arm.tris {
            let q = t.map(|i| arm.screen[i as usize]);
            subject.tri(q);
            dark.tri(q);
        }
    }
    if let Some((f, tip, cm)) = bow {
        subject.seg(f, tip, 2.2 * cm);
    }
    for (d, c) in dark.m.iter_mut().zip(&cloth.m) {
        *d |= *c;
    }
    let near = subject.grow(10);
    let pic = rect;
    let inside = |x: usize, y: usize| {
        let (fx, fy) = (x as f32, y as f32);
        fx > pic.min.x + 3.0
            && fx < pic.max.x - 3.0
            && fy > pic.min.y + 3.0
            && fy < pic.min.y + pic.height() * 0.72
    };
    let px = |x: usize, y: usize| {
        let p = canvas.pixel(x, y);
        [lin(p[0]), lin(p[1]), lin(p[2])]
    };
    // (Floored at the 8-bit's first step above black, as a photograph's JPEG is: a pure 0
    // would make every ratio over it infinite.)
    let lum = |c: [f32; 3]| (0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]).max(lin(1));
    let face = face.grow(-1);
    let (mut fs, mut sum) = (Vec::new(), [0.0f32; 3]);
    let (mut fx0, mut fx1) = (usize::MAX, 0);
    let mut hall = Vec::new();
    let mut all = Vec::new();
    for y in 0..hh {
        for x in 0..w {
            if !inside(x, y) {
                continue;
            }
            let c = px(x, y);
            all.push(lum(c));
            if face.m[y * w + x] {
                fs.push((x, lum(c)));
                sum = [sum[0] + c[0], sum[1] + c[1], sum[2] + c[2]];
                fx0 = fx0.min(x);
                fx1 = fx1.max(x);
            } else if !near.m[y * w + x] {
                hall.push(lum(c));
            }
        }
    }
    assert!(fs.len() > 200, "{hold:?}: the face is {} px", fs.len());
    let n = fs.len() as f32;
    let face_y = fs.iter().map(|f| f.1).sum::<f32>() / n;
    let third = ((fx1 - fx0) / 3).max(1);
    let side = |a: usize| {
        let v: Vec<f32> = fs
            .iter()
            .filter(|f| f.0 >= a && f.0 < a + third)
            .map(|f| f.1)
            .collect();
        v.iter().sum::<f32>() / v.len().max(1) as f32
    };
    let (l, r) = (side(fx0), side(fx1 - third + 1));
    let hall_y = hall.iter().sum::<f32>() / hall.len().max(1) as f32;
    // The edges against the hall: within three pixels of it.
    let open = |x: usize, y: usize| {
        let (x, y) = (x as isize, y as isize);
        (-3..=3).any(|dy| (-3..=3).any(|dx| !subject.get(x + dx, y + dy)))
    };
    let (mut he, mut ce, mut cm) = (Vec::new(), Vec::new(), Vec::new());
    for y in 0..hh {
        for x in 0..w {
            if !inside(x, y) || y as f32 > pic.min.y + pic.height() * 0.5 {
                continue;
            }
            let i = y * w + x;
            if hair.m[i] && open(x, y) {
                he.push(lum(px(x, y)));
            }
            if cloth.m[i] {
                cm.push(lum(px(x, y)));
                if open(x, y) {
                    ce.push(lum(px(x, y)));
                }
            }
        }
    }
    // The silhouette below the face: the dark clothes' edge against the hall just outside.
    let (y0, y1) = (
        pic.min.y + pic.height() * 0.30,
        pic.min.y + pic.height() * 0.72,
    );
    let (sub2, dark6) = (subject.grow(2), dark.grow(6));
    let (mut edge, mut out) = (vec![Vec::new(); 3], vec![Vec::new(); 3]);
    let third = |x: usize| (((x as f32 - pic.min.x) / pic.width() * 3.0) as usize).min(2);
    for y in 0..hh {
        let fy = y as f32;
        if fy < y0 || fy > y1 {
            continue;
        }
        for x in 0..w {
            if !inside(x, y) {
                continue;
            }
            let i = y * w + x;
            let (xi, yi) = (x as isize, y as isize);
            if dark.m[i] && (-2..=2).any(|d| !subject.get(xi + d, yi) || !subject.get(xi, yi + d)) {
                edge[third(x)].push(lum(px(x, y)));
            } else if dark6.m[i] && !sub2.m[i] {
                out[third(x)].push(lum(px(x, y)));
            }
        }
    }
    let ratio = |e: &mut Vec<f32>, o: &mut Vec<f32>| {
        let (a, b) = (pct(e, 0.9), pct(o, 0.5));
        a.max(b) / a.min(b).max(1e-5)
    };
    let mut worst = f32::MAX;
    for k in 0..3 {
        if edge[k].len() > 30 && out[k].len() > 30 {
            worst = worst.min(ratio(&mut edge[k].clone(), &mut out[k].clone()));
        }
    }
    let mut all_e: Vec<f32> = edge.concat();
    let mut all_o: Vec<f32> = out.concat();
    let silhouette = ratio(&mut all_e, &mut all_o);
    Measures {
        silhouette,
        silhouette_worst: if worst == f32::MAX { 0.0 } else { worst },
        face: face_y,
        r_b: sum[0] / sum[2].max(1e-6),
        r_g: sum[0] / sum[1].max(1e-6),
        sides: l.max(r) / l.min(r).max(1e-5),
        face_over_hall: face_y / hall_y.max(1e-5),
        contrast: pct(&mut all.clone(), 0.99) / pct(&mut all, 0.05).max(1e-5),
        hair_edge: pct(&mut he, 0.95) / hall_y.max(1e-5),
        cloth_edge: pct(&mut ce, 0.95) / pct(&mut cm, 0.5).max(1e-5),
    }
}

#[test]
fn the_concert_light_measures_as_the_photographs() {
    views::inst::chorda::scene::hybrid::register();
    scene::set_blocking(true);
    let mut bad = Vec::new();
    let band = |bad: &mut Vec<String>, tag: &str, what: &str, x: f32, lo: f32, hi: f32| {
        if !(lo..=hi).contains(&x) {
            bad.push(format!("{tag}: {what} {x:.3} outside {lo} … {hi}"));
        }
    };
    for hold in [Hold::Woman, Hold::Man] {
        for dark in [true, false] {
            let tag = format!(
                "{hold:?} {} Photographic",
                if dark { "Night" } else { "Paper" }
            );
            let m = measure(hold, dark, SceneBackend::PerPixel);
            println!("{tag}: {m:?}");
            band(&mut bad, &tag, "the face", m.face, 0.08, 0.36);
            band(&mut bad, &tag, "its R/B", m.r_b, 2.3, 4.8);
            band(&mut bad, &tag, "its R/G", m.r_g, 1.6, 3.0);
            band(&mut bad, &tag, "its sides", m.sides, 1.35, 4.5);
            band(
                &mut bad,
                &tag,
                "the face over the hall",
                m.face_over_hall,
                10.0,
                200.0,
            );
            band(&mut bad, &tag, "the contrast", m.contrast, 150.0, 5000.0);
            band(
                &mut bad,
                &tag,
                "the hair's edge",
                m.hair_edge,
                8.0,
                f32::MAX,
            );
            band(
                &mut bad,
                &tag,
                "the clothes' edge",
                m.cloth_edge,
                1.4,
                f32::MAX,
            );
            band(
                &mut bad,
                &tag,
                "the silhouette",
                m.silhouette,
                2.0,
                f32::MAX,
            );
            band(
                &mut bad,
                &tag,
                "its worst third",
                m.silhouette_worst,
                1.6,
                f32::MAX,
            );
        }
        let tag = format!("{hold:?} Night Drawn");
        let m = measure(hold, true, SceneBackend::Vector);
        println!("{tag}: {m:?}");
        band(
            &mut bad,
            &tag,
            "the face over the hall",
            m.face_over_hall,
            6.0,
            300.0,
        );
        band(&mut bad, &tag, "its sides", m.sides, 1.25, 6.0);
        band(
            &mut bad,
            &tag,
            "the clothes' edge",
            m.cloth_edge,
            1.3,
            f32::MAX,
        );
    }
    scene::backend::set_default(SceneBackend::Vector);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
