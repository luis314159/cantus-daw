//! Frames of a Slate character drawn by the app's renderer, timed as the designer's GIFs
//! (`docs/round40/design/dancer/src/anim.py`: 125 BPM, 12 frames a beat), for the
//! side-by-side GIFs in `docs/round40/cast/` (`tools`: `docs/round40/cast/side_by_side.py`).
//!
//! `cargo run -p cantus-dance --example slate_frames -- <out-dir> <id> <night|paper> <moves|routine>`
//!
//! - `moves`: its groove, step-and-touch and drop side by side, four beats (48 frames), each
//!   panel 420 × 495 with the figure 0.56 of the height (the design's `moves_gif`);
//! - `routine`: the 12-beat routine through `pose_at` itself (the song playing from bar 1,
//!   the character's own routine — groove, step-and-touch, the drop), 300 × 440 at 0.52
//!   (the design's cast GIF).

use cantus_core::inst::dancer::{Mood, Move, KEYS};
use cantus_dance::render::{Background, Stage, StageInk};
use cantus_dance::{cast, pose_at, render, DanceFrame, Pose};
use std::path::PathBuf;

const BPM: f64 = 125.0;
const FPB: usize = 12;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).map(String::as_str).unwrap_or("frames"));
    let id = format!(
        "builtin:{}",
        args.get(2).map(String::as_str).unwrap_or("compas")
    );
    let ink = match args.get(3).map(String::as_str) {
        Some("paper") => StageInk::PAPER,
        _ => StageInk::NIGHT,
    };
    let mode = args.get(4).map(String::as_str).unwrap_or("moves");
    let ch = cast::by_id(&id).expect("a character of the cast");
    std::fs::create_dir_all(&out).expect("the output folder");
    let bg = Background::Solid(ink.room);
    if mode == "moves" {
        let mut stage = Stage::new(420, 495, bg);
        stage.ink = ink;
        stage.standard_height = 0.56;
        for k in 0..4 * FPB {
            let b = k as f32 / FPB as f32;
            for (panel, mv) in [Move::Bounce, Move::StepRight, Move::Jump]
                .into_iter()
                .enumerate()
            {
                // The drop's panel repeats its first two beats (the signature), as the
                // design's moves GIF does.
                let (cycle_beats, bb) = match mv {
                    Move::Bounce => (2.0, b),
                    Move::StepRight => (4.0, b),
                    _ => (4.0, b % 2.0),
                };
                let ph = bb / cycle_beats;
                let pose = Pose {
                    mv,
                    phase: ph.fract(),
                    cycle: ph.floor() as u32,
                    beat_phase: b.fract(),
                    energy: 0.7,
                    mood: Mood::Happy,
                    x: 0.5,
                    y: 0.86,
                    playing: true,
                    beats: f64::from(b),
                    seconds: f64::from(b) * 60.0 / BPM,
                    beat_in_bar: (b.floor() as u32) % 4,
                    ..Pose::default()
                };
                let p = render(&stage, &[(ch.as_ref(), pose)]).expect("a stage");
                p.save_png(out.join(format!("f{k:03}_{panel}.png")))
                    .expect("a PNG");
            }
        }
        println!("{} frames of 3 panels in {}", 4 * FPB, out.display());
    } else {
        let mut stage = Stage::new(300, 440, bg);
        stage.ink = ink;
        stage.standard_height = 0.52;
        for k in 0..12 * FPB {
            let beats = k as f64 / FPB as f64;
            let bar = (beats / 4.0).floor();
            let mut frame = DanceFrame {
                beats,
                seconds: beats * 60.0 / BPM,
                tempo_bpm: BPM as f32,
                playing: true,
                bar: bar as i32,
                bar_start_beats: bar * 4.0,
                bar_beats: 4.0,
                ..DanceFrame::default()
            };
            if let Some(i) = KEYS.iter().position(|k| *k == "y") {
                frame.values[i] = 0.86;
            }
            let pose = pose_at(&frame, ch.as_ref(), frame.seconds);
            let p = render(&stage, &[(ch.as_ref(), pose)]).expect("a stage");
            p.save_png(out.join(format!("f{k:03}.png"))).expect("a PNG");
        }
        println!("{} frames in {}", 12 * FPB, out.display());
    }
}
