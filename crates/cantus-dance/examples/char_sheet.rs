//! A character's model sheet (round 37, ADR 0105), to choose and judge a design before it
//! is rigged: `cargo run -p cantus-dance --example char_sheet -- <out-dir> <id…>`.
//!
//! For each character (`mecha`, `ninja`) it writes, every panel on the checker, Cantus
//! Night (the dark theme) and Paper (the light one):
//! - `sheet-<id>-turnaround.png`: front, three-quarter, side, three-quarter back, back;
//! - `sheet-<id>-expressions.png`: the face in every mood, a wink and singing, close up;
//! - `sheet-<id>-poses.png`: key poses (arms up, point, kick, a jump in the air, a hero
//!   pose, a spin);
//! - `sheet-<id>-palette.png`: its colours (base, shade, light per part; then the accents);
//! - `sheet-<id>-hero.png`: one pose large on both themes.

#[path = "common/sheet.rs"]
mod sheet_common;

use cantus_dance::cast::{mecha::Mecha, ninja::Ninja};
use cantus_dance::rig::dance::Body;
use cantus_dance::{Canvas, Mood, Move, Pose};
use sheet_common::*;
use std::path::{Path, PathBuf};
use tiny_skia::{Color, Paint, Pixmap, PixmapPaint, Rect, Transform};

/// Draw `id` in `pose`, its body turned to `yaw` when given (and changed by `edit`).
fn draw(id: &str, pose: &Pose, yaw: Option<f32>, edit: &dyn Fn(&mut Body), c: &mut Canvas) {
    let fix = |b: &mut Body| {
        if let Some(y) = yaw {
            b.yaw = y;
        }
        edit(b);
    };
    match id {
        "mecha" => {
            let m = Mecha::new();
            let mut b = m.body(pose);
            fix(&mut b);
            m.draw_body(pose, &b, c);
        }
        "ninja" => {
            let m = Ninja::new();
            let mut b = m.body(pose);
            fix(&mut b);
            m.draw_body(pose, &b, c);
        }
        _ => panic!("no such character: {id}"),
    }
}

/// One panel: the region of the character's units centred on (`cx`, `cy`), `half` tall
/// each way, drawn into a `w`×`h` cell at (x, y) over `bg`.
#[allow(clippy::too_many_arguments)]
fn panel(
    sheet: &mut Pixmap,
    bg: Bg,
    (x, y, w, h): (u32, u32, u32, u32),
    (cx, cy, half): (f32, f32, f32),
    f: &dyn Fn(&mut Canvas),
) {
    paint_bg(sheet, bg, x, y, w, h);
    let mut p = Pixmap::new(w, h).expect("a cell");
    let scale = h as f32 / (2.0 * half);
    let t = Transform::from_translate(w as f32 * 0.5 - cx * scale, h as f32 * 0.5 - cy * scale)
        .pre_scale(scale, scale);
    let mut c = Canvas {
        pixmap: &mut p,
        transform: t,
        px_per_unit: scale,
        hue: 0.0,
        shadow: true,
    };
    f(&mut c);
    sheet.draw_pixmap(
        x as i32,
        y as i32,
        p.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

const THEMES: [Bg; 3] = [Bg::Checker, Bg::Solid(NIGHT), Bg::Solid(PAPER)];

fn still(mood: Mood) -> Pose {
    let mut p = Pose {
        mv: Move::Idle,
        mood,
        blink: 0.0,
        seconds: 0.6,
        energy: 0.7,
        playing: false,
        ..Pose::default()
    };
    p.from = None;
    p
}

fn turnaround(id: &str, out: &Path) {
    let yaws = [0.0, 40.0, 90.0, 140.0, 180.0];
    let (cw, chh) = (300, 380);
    let mut sheet = Pixmap::new(cw * yaws.len() as u32, chh * THEMES.len() as u32).unwrap();
    for (r, bg) in THEMES.iter().enumerate() {
        for (k, yaw) in yaws.iter().enumerate() {
            let pose = still(Mood::Neutral);
            panel(
                &mut sheet,
                *bg,
                (k as u32 * cw, r as u32 * chh, cw, chh),
                (0.0, -0.74, 0.86),
                &|c| draw(id, &pose, Some(*yaw), &|_| {}, c),
            );
        }
    }
    sheet
        .save_png(out.join(format!("sheet-{id}-turnaround.png")))
        .unwrap();
}

fn expressions(id: &str, out: &Path) {
    // Every mood, then a wink and singing.
    let mut faces: Vec<(Mood, f32, f32)> = Mood::ALL.iter().map(|m| (*m, 0.0, 0.0)).collect();
    faces.push((Mood::Neutral, 1.0, 0.0));
    faces.push((Mood::Happy, 0.0, 0.8));
    let cell = 260;
    let head_y = if id == "mecha" { -1.23 } else { -1.17 };
    let mut sheet = Pixmap::new(cell * faces.len() as u32, cell * 2).unwrap();
    for (r, bg) in [Bg::Solid(NIGHT), Bg::Solid(PAPER)].iter().enumerate() {
        for (k, (mood, wink, sing)) in faces.iter().enumerate() {
            let pose = still(*mood);
            let (wink, sing) = (*wink, *sing);
            panel(
                &mut sheet,
                *bg,
                (k as u32 * cell, r as u32 * cell, cell, cell),
                (0.0, head_y, 0.2),
                &|c| {
                    draw(
                        id,
                        &Pose {
                            seconds: 1.3,
                            ..pose
                        },
                        Some(14.0),
                        &|b| {
                            b.wink[1] = wink;
                            b.mouth = sing;
                            b.tilt = 0.0;
                            b.look = (0.0, 0.0);
                        },
                        c,
                    )
                },
            );
        }
    }
    sheet
        .save_png(out.join(format!("sheet-{id}-expressions.png")))
        .unwrap();
}

/// A key pose: a move at a phase, a mood, and what the sheet does to the body to show the
/// attitude the rig will give it.
type Key = (Move, f32, u32, Mood, fn(&mut Body));

fn poses(id: &str, cw: u32, keys: &[Key]) -> Pixmap {
    let chh = cw * 5 / 4;
    let mut sheet = Pixmap::new(cw * keys.len() as u32, chh * 2).unwrap();
    for (r, bg) in [Bg::Solid(NIGHT), Bg::Solid(PAPER)].iter().enumerate() {
        for (k, (mv, phase, cycle, mood, edit)) in keys.iter().enumerate() {
            let probe = Mecha::new();
            let ch: &dyn cantus_dance::Character = &probe;
            let mut pose = pose_of(ch, *mv, *phase, *cycle, *mood);
            pose.energy = 1.0;
            pose.playing = true;
            panel(
                &mut sheet,
                *bg,
                (k as u32 * cw, r as u32 * chh, cw, chh),
                (0.0, -0.86, 1.12),
                &|c| draw(id, &pose, None, edit, c),
            );
        }
    }
    sheet
}

fn none(_: &mut Body) {}

/// Fuga: a ready stance, a high kick, the aerial, the crouched landing, a point to the sky,
/// the bow.
fn ninja_keys() -> Vec<Key> {
    vec![
        (Move::Idle, 0.2, 0, Mood::Cool, |b| {
            b.foot = [(-0.16, 0.0), (0.17, 0.0)];
            b.y = 0.12;
            b.lean = 8.0;
            b.yaw = 28.0;
            b.arm = [34.0, 74.0];
            b.bend = [96.0, 64.0];
            b.spread = [0.2, 1.0];
            b.tilt = -5.0;
            b.tail = -10.0;
        }),
        (Move::Kick, 0.0, 1, Mood::Excited, |b| {
            b.foot[1] = (0.5, -0.5);
            b.foot[0] = (0.02, 0.0);
            b.lean = -22.0;
            b.x = -0.06;
            b.y = 0.02;
            b.arm = [118.0, 30.0];
            b.bend = [-10.0, 40.0];
            b.yaw = 24.0;
            b.tilt = 10.0;
            b.tail = 30.0;
        }),
        (Move::Jump, 0.74, 0, Mood::Excited, none),
        (Move::Freeze, 0.5, 1, Mood::Cool, none),
        (Move::Point, 0.05, 0, Mood::Happy, |b| {
            b.lean -= 8.0;
            b.foot[0] = (-0.14, 0.0);
            b.tilt += 6.0;
        }),
        (Move::Bow, 0.3, 0, Mood::Calm, none),
    ]
}

/// Volta: the hero stance, the rocket jump, a pop and lock, a front kick, a freeze, arms
/// up singing.
fn mecha_keys() -> Vec<Key> {
    vec![
        (Move::Idle, 0.2, 0, Mood::Cool, |b| {
            b.foot = [(-0.06, 0.0), (0.06, 0.0)];
            b.arm = [36.0, 36.0];
            b.bend = [112.0, 112.0];
            b.spread = [0.0, 0.0];
            b.nod = 8.0;
            b.yaw = 10.0;
        }),
        (Move::Jump, 0.66, 0, Mood::Excited, |b| {
            b.arm = [40.0, 172.0];
            b.bend = [40.0, 0.0];
            b.spread = [0.0, 0.0];
            b.lean = 6.0;
            b.foot = [(-0.02, -0.06), (0.04, -0.14)];
        }),
        (Move::Point, 0.05, 0, Mood::Cool, |b| {
            b.lean -= 6.0;
            b.foot[0] = (-0.12, 0.0);
        }),
        (Move::Kick, 0.0, 1, Mood::Happy, |b| {
            b.foot[1] = (0.36, -0.36);
            b.lean = -14.0;
        }),
        (Move::Freeze, 0.5, 1, Mood::Cool, none),
        (Move::ArmsUp, 0.0, 0, Mood::Excited, none),
    ]
}

fn palette(id: &str, out: &Path) {
    let sw: Vec<(&str, [u8; 4])> = match id {
        "mecha" => Mecha::swatches(),
        _ => Ninja::swatches(),
    };
    let (cw, chh) = (90, 90);
    let mut sheet = Pixmap::new(cw * sw.len() as u32, chh * 2).unwrap();
    for (r, bg) in [Bg::Solid(NIGHT), Bg::Solid(PAPER)].iter().enumerate() {
        for (k, (_, rgba)) in sw.iter().enumerate() {
            let (x, y) = (k as u32 * cw, r as u32 * chh);
            paint_bg(&mut sheet, *bg, x, y, cw, chh);
            let mut p = Paint::default();
            p.set_color(Color::from_rgba8(rgba[0], rgba[1], rgba[2], 255));
            let rect = Rect::from_xywh(x as f32 + 12.0, y as f32 + 12.0, 66.0, 66.0).unwrap();
            sheet.fill_rect(rect, &p, Transform::identity(), None);
        }
    }
    sheet
        .save_png(out.join(format!("sheet-{id}-palette.png")))
        .unwrap();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).map(String::as_str).unwrap_or("char-sheets"));
    std::fs::create_dir_all(&out).expect("the output folder");
    let ids: Vec<String> = if args.len() > 2 {
        args[2..].to_vec()
    } else {
        vec!["mecha".into(), "ninja".into()]
    };
    for id in &ids {
        let keys = if id == "mecha" {
            mecha_keys()
        } else {
            ninja_keys()
        };
        turnaround(id, &out);
        expressions(id, &out);
        poses(id, 260, &keys)
            .save_png(out.join(format!("sheet-{id}-poses.png")))
            .unwrap();
        palette(id, &out);
        let hero = poses(id, 520, &keys[..2]);
        hero.save_png(out.join(format!("sheet-{id}-hero.png")))
            .unwrap();
        println!("{id}: sheets in {}", out.display());
    }
}
