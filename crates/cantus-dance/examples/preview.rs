//! Render a dancer's frames to PNGs, to look at a character move without the application:
//! `cargo run -p cantus-dance --example preview -- <out-dir> [bars] [bpm] [fps] [size] [id…]`.
//! The song is a plain 4/4 at `bpm`, playing from bar 1; each character (every one of the
//! cast when none is named) dances its routine (`move` = 0), one move per bar, through the
//! same `pose_at` and renderer the screen and a video use. With several characters, each
//! gets its own folder.

use cantus_dance::render::{Background, Stage};
use cantus_dance::{cast, pose_at, render, DanceFrame};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(args.get(1).map(String::as_str).unwrap_or("preview"));
    let num = |i: usize, d: f64| args.get(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (bars, bpm, fps, size) = (num(2, 8.0), num(3, 120.0), num(4, 24.0), num(5, 360.0));
    let wanted: Vec<String> = args.iter().skip(6).cloned().collect();
    let all = cast::builtin();
    let chosen: Vec<_> = all
        .iter()
        .filter(|c| {
            wanted.is_empty() || wanted.iter().any(|w| c.info().id == format!("builtin:{w}"))
        })
        .collect();
    for ch in &chosen {
        let dir = if chosen.len() > 1 {
            out.join(ch.info().id.trim_start_matches("builtin:"))
        } else {
            out.clone()
        };
        std::fs::create_dir_all(&dir).expect("the output folder");
        let stage = Stage::new(size as u32, size as u32, Background::Transparent);
        let seconds = bars * 4.0 * 60.0 / bpm;
        let frames = (seconds * fps).round() as usize;
        for i in 0..frames {
            let t = i as f64 / fps;
            let beats = t * bpm / 60.0;
            let bar = (beats / 4.0).floor();
            let mut frame = DanceFrame {
                beats,
                seconds: t,
                tempo_bpm: bpm as f32,
                playing: true,
                bar: bar as i32,
                bar_start_beats: bar * 4.0,
                bar_beats: 4.0,
                ..DanceFrame::default()
            };
            // Stand in the middle of the stage, a little larger than the default.
            if let Some(k) = cantus_core::inst::dancer::KEYS
                .iter()
                .position(|k| *k == "size")
            {
                frame.values[k] = 1.6;
            }
            let pose = pose_at(&frame, ch.as_ref(), t);
            let p = render(&stage, &[(ch.as_ref(), pose)]).expect("a stage");
            p.save_png(dir.join(format!("frame_{i:04}.png")))
                .expect("a PNG");
        }
        println!("{frames} frames in {}", dir.display());
    }
}
