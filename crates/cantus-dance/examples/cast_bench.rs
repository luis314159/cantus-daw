//! The cast's speed (ADR 0087 §3, ADR 0090): every character, every move at eight phases
//! and both sides, every mood, on a 600×600 stage at the standard size, drawn through the
//! one renderer into one reused pixmap (as the stage does). Run it in release:
//! `cargo run --release -p cantus-dance --example cast_bench`. The budget is ≤ 1.5 ms a
//! frame (mean) for one character; it exits with an error when a character is over.

use cantus_dance::render::{render_into, Background, Stage};
use cantus_dance::{cast, pose_at, Character, DanceFrame, Mood, Move, Pose};
use std::time::Instant;

const BUDGET_MS: f64 = 1.5;

fn main() {
    let stage = Stage::new(600, 600, Background::Transparent);
    let mut pixmap = tiny_skia::Pixmap::new(600, 600).expect("a stage");
    let mut over = false;
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        let base = Pose {
            from: None,
            blend: 1.0,
            ..pose_at(
                &DanceFrame {
                    playing: true,
                    ..DanceFrame::default()
                },
                ch,
                0.0,
            )
        };
        let mut poses: Vec<Pose> = Vec::new();
        for mv in Move::ALL {
            for k in 0..16 {
                let phase = (k % 8) as f32 / 8.0;
                poses.push(Pose {
                    mv,
                    phase,
                    cycle: k / 8,
                    energy: 1.0,
                    mood: Mood::from_index(k as usize % 8),
                    seconds: f64::from(k) * 0.37,
                    beat_phase: phase,
                    ..base
                });
            }
        }
        // Warm up, then time every pose several times.
        for p in &poses {
            render_into(&mut pixmap, &stage, &[(ch, *p)]);
        }
        let mut worst = (0.0f64, Move::Idle);
        let mut total = 0.0;
        let mut n = 0;
        let mut per_move = Vec::new();
        for mv in Move::ALL {
            let mine: Vec<&Pose> = poses.iter().filter(|p| p.mv == mv).collect();
            // Best of fifteen batches: the machine is shared, the least disturbed batch is
            // the drawing's own cost.
            let mut ms = f64::MAX;
            for _ in 0..15 {
                let t0 = Instant::now();
                for p in &mine {
                    render_into(&mut pixmap, &stage, &[(ch, **p)]);
                }
                ms = ms.min(t0.elapsed().as_secs_f64() * 1000.0 / mine.len() as f64);
            }
            total += ms * mine.len() as f64;
            n += mine.len();
            if ms > worst.0 {
                worst = (ms, mv);
            }
            per_move.push(format!("{}={ms:.3}", mv.name()));
        }
        let mean = total / n as f64;
        println!(
            "{}: mean {mean:.3} ms a frame, slowest move {} at {:.3} ms (budget {BUDGET_MS} ms)",
            ch.info().id,
            worst.1.name(),
            worst.0
        );
        println!("  {}", per_move.join(" "));
        over |= mean > BUDGET_MS;
    }
    if over {
        eprintln!("over the budget");
        std::process::exit(1);
    }
}
