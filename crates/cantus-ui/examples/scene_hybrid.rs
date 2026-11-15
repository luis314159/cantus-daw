//! r40:scene-px — the photographic renderer beside the scene module's vector picture of the
//! same scene, at the same size: `vector | photographic`.
//!
//! `cargo run --release -p cantus-ui --example scene_hybrid -- <out_dir> [<filter>] [<ppp>] [crop]`
//!
//! For every case whose name holds `<filter>` (all when absent or `all`): `vec-<case>.png`,
//! `px-<case>.png` and `sbs-<case>.png` at `<ppp>` (default 1.5), and with `crop` a 3× crop
//! of the instrument's middle of both (`crop-<case>.png`). Prints each bake's time, its peak
//! memory and its triangles.

use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::{Floor, RoomLook, Rug, Walls};
use cantus_core::inst::tympanum::{KitRef, RoomChoice};
use cantus_core::inst::tympanum_kit::{KitDef, RoomDef};
use cantus_ui::scene::{self, Room, SceneView, Subject};
use cantus_ui::views::inst::chorda::scene::hybrid;
use std::path::{Path, PathBuf};

type Img = (usize, usize, Vec<u8>);

fn save(path: &Path, img: &Img) {
    let f = std::fs::File::create(path).expect("png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), img.0 as u32, img.1 as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("header")
        .write_image_data(&img.2)
        .expect("data");
}

fn of(c: &egui::ColorImage) -> Img {
    (
        c.size[0],
        c.size[1],
        c.pixels.iter().flat_map(|p| p.to_array()).collect(),
    )
}

fn side_by_side(a: &Img, b: &Img) -> Img {
    let gap = 12;
    let (sw, sh) = (a.0 + gap + b.0, a.1.max(b.1));
    let mut s = vec![0u8; sw * sh * 4];
    for px in s.chunks_mut(4) {
        px.copy_from_slice(&[22, 24, 27, 255]);
    }
    for (img, x0) in [(a, 0), (b, a.0 + gap)] {
        for y in 0..img.1 {
            let o = (y * sw + x0) * 4;
            s[o..o + img.0 * 4].copy_from_slice(&img.2[y * img.0 * 4..(y + 1) * img.0 * 4]);
        }
    }
    (sw, sh, s)
}

/// The rect (fractions of the picture) of `img`.
fn crop(img: &Img, r: [f32; 4]) -> Img {
    let (ax, ay) = (
        (img.0 as f32 * r[0]) as usize,
        (img.1 as f32 * r[1]) as usize,
    );
    let (bx, by) = (
        (img.0 as f32 * r[2]) as usize,
        (img.1 as f32 * r[3]) as usize,
    );
    let (w, h) = (bx - ax, by - ay);
    let mut s = Vec::with_capacity(w * h * 4);
    for y in ay..by {
        s.extend_from_slice(&img.2[(y * img.0 + ax) * 4..(y * img.0 + bx) * 4]);
    }
    (w, h, s)
}

fn empty_kit() -> KitDef {
    KitDef {
        kit: KitRef::default_kit(),
        name: "Studio Maple".into(),
        modelled: true,
        pieces: Vec::new(),
        mics: Vec::new(),
        bleed: Default::default(),
        room: RoomDef {
            width: 600.0,
            depth: 800.0,
            height: 340.0,
            back: 270.0,
        },
        default_room: RoomChoice::Own,
        licence: None,
        credit: None,
        finish: cantus_core::inst::chorda_look::Finish::Maple,
    }
}

fn view(subject: Subject, room: Room, dark: bool, look: RoomLook) -> SceneView {
    SceneView {
        subject,
        room,
        look,
        dark,
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: egui::Color32::from_rgb(92, 175, 233),
        ink: egui::Color32::from_rgb(227, 232, 237),
    }
}

fn piano(body: Body, finish: Finish, lid: f32, mics: bool) -> Subject {
    Subject::Piano {
        look: Look::new(body, finish),
        lid,
        persp: 0.35,
        width: 0.7,
        room_level: 0.35,
        mics,
    }
}

fn kit(mics: bool) -> Subject {
    Subject::Kit {
        kit: KitRef::default_kit(),
        def: Box::new(empty_kit()),
        finish: Finish::Maple,
        mics,
    }
}

const PIANO_CROP: [f32; 4] = [0.25, 0.1, 0.8, 0.65];
const KIT_CROP: [f32; 4] = [0.2, 0.2, 0.75, 0.75];

fn cases() -> Vec<(String, SceneView, [f32; 4])> {
    let none = RoomLook::default();
    let concert = |f, lid| piano(Body::Concert, f, lid, false);
    let mut c = vec![
        (
            "chorda-stage-night".to_string(),
            view(concert(Finish::Black, 1.0), Room::Stage, true, none),
            PIANO_CROP,
        ),
        (
            "chorda-stage-paper".to_string(),
            view(concert(Finish::Black, 1.0), Room::Stage, false, none),
            PIANO_CROP,
        ),
        (
            "tympanum-stage-night".to_string(),
            view(kit(false), Room::Stage, true, none),
            KIT_CROP,
        ),
        (
            "tympanum-hall-night".to_string(),
            view(kit(false), Room::Hall, true, none),
            KIT_CROP,
        ),
    ];
    for room in [Room::Booth, Room::Studio, Room::Hall, Room::Church] {
        let name = format!("{room:?}").to_lowercase();
        c.push((
            format!("chorda-{name}-night"),
            view(
                piano(Body::Grand, Finish::Black, 1.0, true),
                room,
                true,
                none,
            ),
            PIANO_CROP,
        ));
        c.push((
            format!("chorda-{name}-paper"),
            view(
                piano(Body::Grand, Finish::Walnut, 1.0, true),
                room,
                false,
                none,
            ),
            PIANO_CROP,
        ));
        c.push((
            format!("tympanum-{name}-paper"),
            view(kit(true), room, false, none),
            KIT_CROP,
        ));
    }
    for (body, finish) in [
        (Body::Parlour, Finish::Rosewood),
        (Body::Upright, Finish::Walnut),
        (Body::Concert, Finish::White),
        (Body::Grand, Finish::Maple),
    ] {
        let name = format!("{body:?}-{finish:?}").to_lowercase();
        c.push((
            format!("chorda-{name}-stage-night"),
            view(piano(body, finish, 0.6, false), Room::Stage, true, none),
            PIANO_CROP,
        ));
    }
    // Round 40 (visual): every body at every lid state (closed, the short stick, the full
    // stick; the upright's leaf closed, standing, folded back).
    for (body, finish) in [
        (Body::Concert, Finish::Black),
        (Body::Grand, Finish::Maple),
        (Body::Parlour, Finish::Rosewood),
        (Body::Upright, Finish::Walnut),
    ] {
        for (state, lid) in [("closed", 0.0), ("short", 20.0 / 36.0), ("full", 1.0)] {
            let name = format!("lid-{body:?}-{finish:?}-{state}").to_lowercase();
            c.push((
                name,
                view(piano(body, finish, lid, false), Room::Stage, true, none),
                PIANO_CROP,
            ));
        }
    }
    let look = RoomLook {
        room: None,
        floor: Some(Floor::Terrazzo),
        walls: Some(Walls::Brick),
        rug: Some(Rug::Blue),
    };
    c.push((
        "chorda-studio-terrazzo-brick-night".to_string(),
        view(concert(Finish::Black, 0.0), Room::Studio, true, look),
        PIANO_CROP,
    ));
    c
}

/// The working set before and after each bake (Chorda on the stage, then Tympanum), the
/// pictures kept as an open scene keeps its picture, and the time of each full and quick
/// bake (`mem` as the filter).
fn memory(size: egui::Vec2, ppp: f32, threads: usize) {
    let ws = || cantus_rt::usage::working_set_bytes().unwrap_or(0) as f64 / 1e6;
    let base = ws();
    println!("working set at start: {base:.0} MB");
    let mut kept = Vec::new();
    for (name, v, _) in cases()
        .into_iter()
        .filter(|c| c.0 == "chorda-stage-night" || c.0 == "tympanum-stage-night")
    {
        for q in [hybrid::PREVIEW, hybrid::FULL] {
            let t = std::time::Instant::now();
            let (img, st) = hybrid::render(&v, size, ppp, q, threads);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            println!(
                "{name} {}: {ms:.0} ms, bake peak {:.0} MB, working set after {:.0} MB",
                if q.full { "full" } else { "quick" },
                st.peak as f64 / 1e6,
                ws()
            );
            if q.full {
                kept.push(img);
            }
        }
    }
    println!(
        "both pictures kept: working set {:.0} MB (+{:.0} MB over the start)",
        ws(),
        ws() - base
    );
    // Each pass of a full bake, the best of three, and the same without traced reflections.
    for (name, v, _) in cases()
        .into_iter()
        .filter(|c| c.0 == "chorda-stage-night" || c.0 == "tympanum-stage-night")
    {
        for (label, q) in [
            ("full", hybrid::FULL),
            (
                "no trace",
                hybrid::Quality {
                    trace: false,
                    ..hybrid::FULL
                },
            ),
        ] {
            let best = (0..3)
                .map(|_| hybrid::render(&v, size, ppp, q, threads).1)
                .min_by(|a, b| a.ms.total_cmp(&b.ms))
                .expect("three bakes");
            let p = best.passes;
            println!(
                "{name} {label}: {:.0} ms = shadow map {:.0} + mirror {:.0} + hierarchy {:.0} + raster {:.0} + shading {:.0} + wires {:.0} + post {:.0}",
                best.ms, p[0], p[1], p[2], p[3], p[4], p[5], p[6]
            );
        }
    }
    println!("after: working set {:.0} MB", ws());
    drop(kept);
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(
        a.get(1)
            .cloned()
            .unwrap_or_else(|| "target/scene-px".into()),
    );
    let filter = a.get(2).cloned().filter(|f| f != "all").unwrap_or_default();
    let ppp: f32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1.5);
    let crops = a.get(4).is_some_and(|s| s == "crop");
    std::fs::create_dir_all(&out).expect("out dir");
    let size = egui::vec2(756.0, 452.0);
    let threads = hybrid::threads();
    if filter == "mem" {
        memory(size, ppp, threads);
        return;
    }
    if filter == "loop" {
        // Bake Chorda and Tympanum back to back for `<seconds>` (the audio's stress test).
        let secs: f64 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(60.0);
        let cs: Vec<_> = cases().into_iter().take(3).collect();
        let t = std::time::Instant::now();
        let mut n = 0;
        while t.elapsed().as_secs_f64() < secs {
            let _ = hybrid::render(&cs[n % 3].1, size, ppp, hybrid::FULL, threads);
            n += 1;
        }
        println!("{n} bakes in {secs} s");
        return;
    }
    for (name, v, cr) in cases() {
        if !filter.is_empty() && !name.contains(&filter) {
            continue;
        }
        // Drawn's time: the best of five (round 40, visual: Drawn's budget).
        let mut vms = f64::MAX;
        let mut vec = None;
        for _ in 0..5 {
            let t = std::time::Instant::now();
            let img = scene::image(&v, size, ppp);
            vms = vms.min(t.elapsed().as_secs_f64() * 1e3);
            vec = Some(img);
        }
        let vec = of(&vec.expect("a picture"));
        let (px, st) = hybrid::render(&v, size, ppp, hybrid::FULL, threads);
        let px = of(&px);
        println!(
            "{name}: drawn {vms:.1} ms; {}x{} bake {:.0} ms, peak {:.0} MB, {} triangles",
            px.0,
            px.1,
            st.ms,
            st.peak as f64 / 1e6,
            st.tris
        );
        save(&out.join(format!("vec-{name}.png")), &vec);
        save(&out.join(format!("px-{name}.png")), &px);
        save(
            &out.join(format!("sbs-{name}.png")),
            &side_by_side(&vec, &px),
        );
        if crops {
            let v3x = of(&scene::image(&v, size, 3.0));
            let (w3, h3) = ((size.x * 3.0) as usize, (size.y * 3.0) as usize);
            let (p3x, st3) = hybrid::render_px(&v, size, w3, h3, hybrid::FULL, threads);
            println!(
                "  3x: bake {:.0} ms, peak {:.0} MB",
                st3.ms,
                st3.peak as f64 / 1e6
            );
            save(
                &out.join(format!("crop-{name}.png")),
                &side_by_side(&crop(&v3x, cr), &crop(&of(&p3x), cr)),
            );
        }
    }
}
