//! Round 40 phase 3 (ADR 0126): the designer's two contact sheets drawn by the scene module,
//! tile for tile — `chorda-variants-*` (`contact.py`: every body in three finishes, the lid,
//! the rooms, Perspective, the light) and `rooms-*` (`rooms_sheet.py`: every floor under the
//! kit, the rugs, every wall behind the grand, combinations) — each tile 304 × 171 at 1×
//! with the designer's framing (margin 6), written as `tile-<sheet>-<theme>-<n>.png` with
//! the tile's place on the sheet in `tiles-<sheet>-<theme>.txt` (`n x y`), so a script can
//! paste them over a copy of the designer's sheet beside the original.
//!
//! `cargo run -p cantus-ui --example shoot_scene_sheets -- <out_dir>`

use cantus_core::inst::chorda_look::{Body, Finish, Look};
use cantus_core::inst::room_look::{Floor, RoomLook, Rug, Walls};
use cantus_core::inst::tympanum::{KitRef, RoomChoice};
use cantus_core::inst::tympanum_kit::{KitDef, RoomDef};
use cantus_ui::scene::compose::Frame;
use cantus_ui::scene::{self, Room, SceneView, Subject};
use std::io::Write;
use std::path::{Path, PathBuf};

const TW: f32 = 304.0;
const TH: f32 = 171.0;

fn save(path: &Path, img: &egui::ColorImage) {
    let f = std::fs::File::create(path).expect("png");
    let mut e = png::Encoder::new(
        std::io::BufWriter::new(f),
        img.size[0] as u32,
        img.size[1] as u32,
    );
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    e.write_header()
        .expect("header")
        .write_image_data(&rgba)
        .expect("data");
}

fn kit_def() -> KitDef {
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
        finish: Finish::Maple,
    }
}

/// One tile as the designer's `piano_scene` / `kit_scene` keywords say it.
#[derive(Clone)]
struct T {
    kit: bool,
    body: &'static str,
    finish: &'static str,
    /// Degrees, as the designer gives it.
    lid: f32,
    room: &'static str,
    persp: f32,
    room_level: f32,
    mics: bool,
    floor: Option<&'static str>,
    walls: Option<&'static str>,
    rug: Option<&'static str>,
    /// `Some(dark)`: the light of another theme than the sheet's.
    mood: Option<bool>,
}

const P: T = T {
    kit: false,
    body: "concert",
    finish: "black",
    lid: 36.0,
    room: "stage",
    persp: 0.35,
    room_level: 0.35,
    mics: true,
    floor: None,
    walls: None,
    rug: None,
    mood: None,
};

const K: T = T {
    kit: true,
    mics: false,
    ..P
};

fn room(k: &str) -> Room {
    match k {
        "booth" => Room::Booth,
        "studio" => Room::Studio,
        "hall" => Room::Hall,
        "church" => Room::Church,
        _ => Room::Stage,
    }
}

fn view(t: &T, dark: bool) -> SceneView {
    let body = Body::from_key(t.body).expect("body");
    let full = if body == Body::Upright { 24.0 } else { 36.0 };
    let subject = if t.kit {
        Subject::Kit {
            kit: KitRef::default_kit(),
            def: Box::new(kit_def()),
            finish: Finish::Maple,
            mics: t.mics,
        }
    } else {
        Subject::Piano {
            look: Look::new(body, Finish::from_key(t.finish).expect("finish")),
            lid: (t.lid / full).clamp(0.0, 1.0),
            persp: t.persp,
            width: 0.7,
            room_level: t.room_level,
            mics: t.mics,
        }
    };
    SceneView {
        subject,
        room: room(t.room),
        look: RoomLook {
            room: None,
            floor: t.floor.and_then(Floor::from_key),
            walls: t.walls.and_then(Walls::from_key),
            rug: t.rug.and_then(Rug::from_key),
        },
        dark: t.mood.unwrap_or(dark),
        selected: None,
        hovered: None,
        sounding: Vec::new(),
        accent: egui::Color32::from_rgb(92, 175, 233),
        ink: egui::Color32::from_rgb(227, 232, 237),
    }
}

/// The variants sheet's rows (`contact.py`), with whether each row has a title.
fn variants() -> Vec<(bool, Vec<T>)> {
    let fins = [
        ("concert", ["black", "white", "maple"]),
        ("grand", ["black", "walnut", "rosewood"]),
        ("parlour", ["rosewood", "maple", "white"]),
        ("upright", ["black", "walnut", "maple"]),
    ];
    let mut rows = Vec::new();
    for fi in 0..3 {
        rows.push((
            fi == 0,
            fins.iter()
                .map(|&(b, fs)| T {
                    body: b,
                    finish: fs[fi],
                    lid: if b == "upright" { 0.0 } else { 36.0 },
                    mics: false,
                    ..P
                })
                .collect(),
        ));
    }
    let nm = T { mics: false, ..P };
    rows.push((
        true,
        vec![
            T {
                lid: 0.0,
                ..nm.clone()
            },
            T {
                lid: 20.0,
                ..nm.clone()
            },
            nm.clone(),
            T {
                body: "upright",
                lid: 24.0,
                finish: "walnut",
                ..nm.clone()
            },
        ],
    ));
    rows.push((
        true,
        vec![
            T {
                body: "grand",
                room: "booth",
                ..nm.clone()
            },
            T {
                room: "studio",
                ..nm.clone()
            },
            T {
                room: "hall",
                ..nm.clone()
            },
            T {
                room: "church",
                ..nm.clone()
            },
        ],
    ));
    rows.push((
        true,
        vec![
            T {
                persp: 0.0,
                room_level: 0.0,
                ..P
            },
            T {
                room_level: 0.0,
                ..P
            },
            T {
                persp: 1.0,
                room_level: 0.0,
                ..P
            },
            T {
                room_level: 0.4,
                ..P
            },
        ],
    ));
    let gw = T {
        body: "grand",
        finish: "walnut",
        room: "hall",
        ..P
    };
    rows.push((
        true,
        vec![
            T {
                mood: Some(true),
                ..P
            },
            T {
                mood: Some(false),
                ..P
            },
            T {
                mood: Some(true),
                ..gw.clone()
            },
            T {
                mood: Some(false),
                ..gw
            },
        ],
    ));
    rows
}

/// The rooms sheet's rows (`rooms_sheet.py`).
fn rooms() -> Vec<(bool, Vec<T>)> {
    let floors = [
        "honey_oak",
        "light_oak",
        "dark_walnut",
        "ebony_stage",
        "concrete",
        "terrazzo",
        "stage_walnut",
        "flagstone",
    ];
    let ft: Vec<T> = floors
        .iter()
        .map(|&f| T {
            room: "studio",
            floor: Some(f),
            rug: Some("none"),
            ..K
        })
        .collect();
    let np = T { mics: false, ..P };
    let walls = [
        "warm_wood",
        "walnut_panels",
        "plaster",
        "charcoal",
        "blue_fabric",
        "green_fabric",
        "brick",
        "warm_plaster",
    ];
    let wt: Vec<T> = walls
        .iter()
        .map(|&w| T {
            room: "studio",
            walls: Some(w),
            ..np.clone()
        })
        .collect();
    vec![
        (true, ft[..4].to_vec()),
        (false, ft[4..].to_vec()),
        (
            true,
            vec![
                T {
                    room: "studio",
                    rug: Some("red"),
                    ..K
                },
                T {
                    room: "studio",
                    rug: Some("blue"),
                    ..np.clone()
                },
                T {
                    room: "studio",
                    rug: Some("grey"),
                    floor: Some("light_oak"),
                    ..K
                },
                T {
                    room: "studio",
                    rug: Some("none"),
                    ..np.clone()
                },
            ],
        ),
        (true, wt[..4].to_vec()),
        (false, wt[4..].to_vec()),
        (
            true,
            vec![
                T {
                    room: "booth",
                    floor: Some("concrete"),
                    walls: Some("charcoal"),
                    ..K
                },
                T {
                    room: "studio",
                    floor: Some("light_oak"),
                    walls: Some("plaster"),
                    body: "grand",
                    finish: "white",
                    ..np.clone()
                },
                T {
                    room: "studio",
                    floor: Some("terrazzo"),
                    walls: Some("brick"),
                    ..K
                },
                T {
                    room: "hall",
                    floor: Some("ebony_stage"),
                    walls: Some("walnut_panels"),
                    ..np.clone()
                },
            ],
        ),
        (
            false,
            vec![
                T { room: "hall", ..K },
                T {
                    room: "booth",
                    walls: Some("blue_fabric"),
                    body: "upright",
                    finish: "walnut",
                    lid: 20.0,
                    ..np.clone()
                },
                T {
                    room: "studio",
                    floor: Some("dark_walnut"),
                    walls: Some("green_fabric"),
                    ..K
                },
                T {
                    room: "church",
                    ..np
                },
            ],
        ),
    ]
}

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/shoot-scene-sheets".into()),
    );
    std::fs::create_dir_all(&out).expect("out dir");
    let size = egui::vec2(TW, TH);
    for (sheet, rows) in [("variants", variants()), ("rooms", rooms())] {
        for (theme, dark) in [("night", true), ("paper", false)] {
            let mut list = std::fs::File::create(out.join(format!("tiles-{sheet}-{theme}.txt")))
                .expect("list");
            let mut y = 56.0;
            let mut n = 0;
            for (titled, tiles) in &rows {
                if *titled {
                    y += 24.0;
                }
                for (i, t) in tiles.iter().enumerate() {
                    n += 1;
                    let x = 12.0 + i as f32 * (TW + 8.0);
                    let frame = Frame {
                        inset: [0.0; 4],
                        margin: 6.0,
                        anchor: if t.kit { (0.5, 0.5) } else { (0.5, 0.55) },
                    };
                    let img = scene::image_framed(&view(t, dark), size, 1.0, frame);
                    save(&out.join(format!("tile-{sheet}-{theme}-{n}.png")), &img);
                    writeln!(list, "{n} {x} {y}").expect("list");
                }
                y += TH + 30.0;
            }
            println!("{sheet} {theme}: {n} tiles");
        }
    }
}
