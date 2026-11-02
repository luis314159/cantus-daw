//! Shared by `cast_sheet` (cantus-dance) and `shoot_cast` (cantus-ui): contact sheets of a
//! character drawn through the one renderer, composed with tiny-skia.

#![allow(dead_code)]

use cantus_dance::render::{render_into, Background, Stage};
use cantus_dance::{pose_at, Character, DanceFrame, Mood, Move, Pose};
use tiny_skia::{Color, Paint, Pixmap, PixmapPaint, Rect, Transform};

/// What a cell is drawn over.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Bg {
    /// A grey checker: the character on a transparent stage, as an editor would show it.
    Checker,
    Solid([u8; 4]),
}

/// Cantus Night's panel, Slate's panel, Paper's panel.
pub const NIGHT: [u8; 4] = [0x1f, 0x17, 0x12, 255];
pub const SLATE: [u8; 4] = [0x4e, 0x58, 0x5e, 255];
pub const PAPER: [u8; 4] = [0xf3, 0xf5, 0xf4, 255];
pub const BACKGROUNDS: [Bg; 4] = [
    Bg::Checker,
    Bg::Solid(NIGHT),
    Bg::Solid(SLATE),
    Bg::Solid(PAPER),
];

/// A playing frame at `beats` (4/4 at 120), with `size` and the rest at defaults.
pub fn frame_at(beats: f64, size: f32) -> DanceFrame {
    let bar = (beats / 4.0).floor();
    let mut f = DanceFrame {
        beats,
        seconds: beats * 0.5,
        tempo_bpm: 120.0,
        playing: true,
        bar: bar as i32,
        bar_start_beats: bar * 4.0,
        bar_beats: 4.0,
        ..DanceFrame::default()
    };
    set(&mut f, "size", size);
    f
}

pub fn set(f: &mut DanceFrame, key: &str, v: f32) {
    if let Some(i) = cantus_core::inst::dancer::KEYS
        .iter()
        .position(|k| *k == key)
    {
        f.values[i] = v;
    }
}

/// A pose of `mv` at `phase` of cycle `cycle`, standing in the middle.
pub fn pose_of(ch: &dyn Character, mv: Move, phase: f32, cycle: u32, mood: Mood) -> Pose {
    let base = pose_at(&frame_at(0.0, 1.0), ch, 0.0);
    Pose {
        mv,
        phase,
        cycle,
        from: None,
        blend: 1.0,
        beat_phase: (phase * ch.move_beats(mv)).rem_euclid(1.0),
        mood,
        seconds: f64::from(cycle as f32 + phase) * f64::from(ch.move_beats(mv)) * 0.5,
        blink: 0.0,
        ..base
    }
}

/// Paint a background into a `w`×`h` cell at (x, y) of `sheet`.
pub fn paint_bg(sheet: &mut Pixmap, bg: Bg, x: u32, y: u32, w: u32, h: u32) {
    let fill = |sheet: &mut Pixmap, rect: Option<Rect>, c: [u8; 4]| {
        if let Some(r) = rect {
            let mut p = Paint::default();
            p.set_color(Color::from_rgba8(c[0], c[1], c[2], c[3]));
            sheet.fill_rect(r, &p, Transform::identity(), None);
        }
    };
    match bg {
        Bg::Solid(c) => fill(
            sheet,
            Rect::from_xywh(x as f32, y as f32, w as f32, h as f32),
            c,
        ),
        Bg::Checker => {
            let sq = 12;
            for j in (0..h).step_by(sq) {
                for i in (0..w).step_by(sq) {
                    let dark = ((i / sq as u32) + (j / sq as u32)).is_multiple_of(2);
                    let c = if dark {
                        [0x9a, 0x9a, 0xa0, 255]
                    } else {
                        [0xc8, 0xc8, 0xcc, 255]
                    };
                    let (cw, chh) = ((w - i).min(sq as u32), (h - j).min(sq as u32));
                    fill(
                        sheet,
                        Rect::from_xywh((x + i) as f32, (y + j) as f32, cw as f32, chh as f32),
                        c,
                    );
                }
            }
        }
    }
}

/// Draw `pose` of `ch` into a `w`×`h` cell at (x, y) of `sheet` over `bg`.
#[allow(clippy::too_many_arguments)]
pub fn cell(
    sheet: &mut Pixmap,
    ch: &dyn Character,
    pose: &Pose,
    bg: Bg,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) {
    paint_bg(sheet, bg, x, y, w, h);
    let stage = Stage::new(w, h, Background::Transparent);
    let mut p = Pixmap::new(w, h).expect("a cell");
    render_into(&mut p, &stage, &[(ch, *pose)]);
    sheet.draw_pixmap(
        x as i32,
        y as i32,
        p.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

/// Every move (rows) at phases 0, ¼, ½, ¾ and the next cycle's ¼ (columns), at full
/// energy (the silhouettes that must tell the moves apart as thumbnails).
pub fn moves_sheet(ch: &dyn Character, cell_px: u32, bgs: &[Bg]) -> Pixmap {
    let cols = [(0.0, 0), (0.25, 0), (0.5, 0), (0.75, 0), (0.25, 1)];
    let w = cell_px * cols.len() as u32 * bgs.len() as u32;
    let h = cell_px * Move::ALL.len() as u32;
    let mut sheet = Pixmap::new(w, h).expect("a sheet");
    for (r, mv) in Move::ALL.iter().enumerate() {
        for (bi, bg) in bgs.iter().enumerate() {
            for (k, (ph, cy)) in cols.iter().enumerate() {
                let pose = Pose {
                    energy: 1.0,
                    ..pose_of(ch, *mv, *ph, *cy, Mood::Happy)
                };
                let x = (bi * cols.len() + k) as u32 * cell_px;
                cell(
                    &mut sheet,
                    ch,
                    &pose,
                    *bg,
                    x,
                    r as u32 * cell_px,
                    cell_px,
                    cell_px,
                );
            }
        }
    }
    sheet
}

/// Every mood (columns) standing and mid-bounce (two rows per background).
pub fn moods_sheet(ch: &dyn Character, cell_px: u32, bgs: &[Bg]) -> Pixmap {
    let w = cell_px * Mood::ALL.len() as u32;
    let h = cell_px * 2 * bgs.len() as u32;
    let mut sheet = Pixmap::new(w, h).expect("a sheet");
    for (bi, bg) in bgs.iter().enumerate() {
        for (k, mood) in Mood::ALL.iter().enumerate() {
            for (r, (mv, ph)) in [(Move::Idle, 0.1), (Move::Bounce, 0.5)].iter().enumerate() {
                let pose = pose_of(ch, *mv, *ph, 0, *mood);
                let y = (bi * 2 + r) as u32 * cell_px;
                cell(
                    &mut sheet,
                    ch,
                    &pose,
                    *bg,
                    k as u32 * cell_px,
                    y,
                    cell_px,
                    cell_px,
                );
            }
        }
    }
    sheet
}

/// The same pose with the character `heights` pixels tall, side by side.
pub fn sizes_sheet(ch: &dyn Character, heights: &[u32]) -> Pixmap {
    let pose = pose_of(ch, Move::Point, 0.1, 0, Mood::Happy);
    // A cell of height h shows a size-1 character 0.42 h tall.
    let cells: Vec<u32> = heights.iter().map(|h| h * 2).collect();
    let w: u32 = cells.iter().sum();
    let hmax = *cells.iter().max().unwrap_or(&1);
    let mut sheet = Pixmap::new(w, hmax).expect("a sheet");
    let mut x = 0;
    for (k, c) in cells.iter().enumerate() {
        let pose = Pose {
            size: heights[k] as f32 / (0.42 * *c as f32),
            ..pose
        };
        cell(&mut sheet, ch, &pose, Bg::Checker, x, hmax - c, *c, *c);
        x += c;
    }
    sheet
}

/// Every mood (columns) on every background (rows), mid-bounce.
pub fn moods_rows_sheet(ch: &dyn Character, cell_px: u32, bgs: &[Bg]) -> Pixmap {
    let w = cell_px * Mood::ALL.len() as u32;
    let h = cell_px * bgs.len() as u32;
    let mut sheet = Pixmap::new(w, h).expect("a sheet");
    for (r, bg) in bgs.iter().enumerate() {
        for (k, mood) in Mood::ALL.iter().enumerate() {
            let pose = pose_of(ch, Move::Bounce, 0.45, 0, *mood);
            cell(
                &mut sheet,
                ch,
                &pose,
                *bg,
                k as u32 * cell_px,
                r as u32 * cell_px,
                cell_px,
                cell_px,
            );
        }
    }
    sheet
}
