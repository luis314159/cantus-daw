//! Round 40 phase 3 (ADR 0126): the projected-vector scenes on the CPU, beside the
//! designer's pictures at the same size.
//!
//! `cargo run -p cantus-ui --example shoot_scene -- <out_dir> [<design_dir>]`
//!
//! Writes `cantus-<case>.png` for every case and, when the designer's picture of the case is
//! in `<design_dir>` (default `F:\projects\daw-r40\design-p3`), `sbs-<case>.png`: designer
//! | Cantus, a gap between. Prints each build's time.

use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::RoomLook;
use cantus_core::inst::tympanum::{KitRef, RoomChoice};
use cantus_core::inst::tympanum_kit::{KitDef, RoomDef};
use cantus_ui::scene::{self, Picked, Room, SceneView, Subject};
use std::path::{Path, PathBuf};

fn save(path: &Path, w: usize, h: usize, rgba: &[u8]) {
    let f = std::fs::File::create(path).expect("png");
    let mut e = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.write_header()
        .expect("header")
        .write_image_data(rgba)
        .expect("data");
}

fn load(path: &Path) -> Option<(usize, usize, Vec<u8>)> {
    let d = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
    let mut r = d.read_info().ok()?;
    let mut buf = vec![0; r.output_buffer_size()?];
    let info = r.next_frame(&mut buf).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buf[..w * h * 3]
            .chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        _ => return None,
    };
    Some((w, h, rgba))
}

/// `a` | `b` side by side (b's height), a gap between.
fn side_by_side(
    a: &(usize, usize, Vec<u8>),
    b: &(usize, usize, Vec<u8>),
) -> (usize, usize, Vec<u8>) {
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

fn view(subject: Subject, room: Room, dark: bool) -> SceneView {
    SceneView {
        subject,
        room,
        look: RoomLook::default(),
        dark,
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: egui::Color32::from_rgb(92, 175, 233),
        ink: egui::Color32::from_rgb(227, 232, 237),
    }
}

fn piano(body: Body, finish: Finish, mics: bool) -> Subject {
    Subject::Piano {
        look: Look::new(body, finish),
        lid: 1.0,
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

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(
        a.get(1)
            .cloned()
            .unwrap_or_else(|| "target/shoot-scene".into()),
    );
    let design = PathBuf::from(
        a.get(2)
            .cloned()
            .unwrap_or_else(|| r"F:\projects\daw-r40\design-p3".into()),
    );
    std::fs::create_dir_all(&out).expect("out dir");
    let size = egui::vec2(756.0, 452.0);
    let mut cases: Vec<(String, SceneView, f32, Option<String>)> = Vec::new();
    for (theme, dark) in [("night", true), ("paper", false)] {
        for ppp in [1.0f32, 1.5, 3.0] {
            let tag = if ppp < 1.2 {
                "1x"
            } else if ppp < 2.0 {
                "1.5x"
            } else {
                "3x"
            };
            cases.push((
                format!("chorda-{theme}-{tag}"),
                view(
                    piano(Body::Concert, Finish::Black, false),
                    Room::Stage,
                    dark,
                ),
                ppp,
                Some(format!("pic-chorda-{theme}-{tag}.png")),
            ));
            for room in [Room::Stage, Room::Hall, Room::Studio, Room::Booth] {
                let name = format!("{room:?}").to_lowercase();
                let mut v = view(kit(false), room, dark);
                v.selected = Some(Picked::Piece {
                    piece: 1,
                    zone: None,
                });
                v.hovered = Some(Picked::Piece {
                    piece: 9,
                    zone: None,
                });
                cases.push((
                    format!("tympanum-{name}-{theme}-{tag}"),
                    v,
                    ppp,
                    Some(format!("tympanum-{name}-{theme}-{tag}.png")),
                ));
            }
        }
        for body in [Body::Grand, Body::Parlour, Body::Upright] {
            for finish in [Finish::White, Finish::Walnut, Finish::Maple] {
                let name = format!("chorda-{body:?}-{finish:?}-{theme}").to_lowercase();
                cases.push((
                    name,
                    view(piano(body, finish, true), Room::Stage, dark),
                    1.0,
                    None,
                ));
            }
        }
        for room in Room::ALL {
            let name = format!("chorda-room-{room:?}-{theme}").to_lowercase();
            cases.push((
                name,
                view(piano(Body::Concert, Finish::Black, true), room, dark),
                1.0,
                None,
            ));
        }
    }
    let only = a.get(3).cloned();
    for (name, v, ppp, refname) in cases {
        if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
            continue;
        }
        let t0 = std::time::Instant::now();
        // The designer's picture-alone sheets frame the piano wider than the play view.
        let img = if refname
            .as_deref()
            .is_some_and(|r| r.starts_with("pic-chorda"))
        {
            scene::image_framed(&v, size, ppp, scene::compose::PIANO_PICTURE_FRAME)
        } else {
            scene::image(&v, size, ppp)
        };
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let [w, h] = img.size;
        let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        save(&out.join(format!("cantus-{name}.png")), w, h, &rgba);
        let mut line = format!("{name}: {w}x{h} in {ms:.0} ms");
        if let Some(r) = refname.and_then(|r| load(&design.join(r))) {
            if (r.0, r.1) == (w, h) {
                let s = side_by_side(&r, &(w, h, rgba));
                save(&out.join(format!("sbs-{name}.png")), s.0, s.1, &s.2);
                line.push_str(" · side by side");
            }
        }
        println!("{line}");
    }
}
