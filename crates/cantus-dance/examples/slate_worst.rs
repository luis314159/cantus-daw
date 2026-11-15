//! The Slate cast's worst frames (round 40, ADR 0122), timed as `cast_bench` times a move:
//! a 600 × 600 stage at the standard size, one reused pixmap, best of fifteen batches.
//! Eco's Feedback at its fullest (the most copies), Glissando's Halo closing and dropping,
//! Bajo's blast, Onda's square and Espectro's peak when they are in the cast. Run it in
//! release: `cargo run --release -p cantus-dance --example slate_worst`.

use cantus_dance::render::{render_into, Background, Stage};
use cantus_dance::{cast, Mood, Move, Pose};
use std::time::Instant;

const BUDGET_MS: f64 = 1.5;

fn main() {
    let stage = Stage::new(600, 600, Background::Transparent);
    let mut pixmap = tiny_skia::Pixmap::new(600, 600).expect("a stage");
    // (character, what, beats into its `Jump` bar)
    let worst: &[(&str, &str, &[f32])] = &[
        ("builtin:eco", "Feedback, fullest", &[0.9, 0.95, 0.99]),
        (
            "builtin:glissando",
            "Halo closing and dropping",
            &[0.95, 1.05, 1.2],
        ),
        ("builtin:bajo", "the blast", &[1.1, 1.3, 1.5]),
        ("builtin:onda", "Square", &[1.0, 1.2, 1.5]),
        ("builtin:espectro", "Pico", &[1.0, 1.1, 1.3]),
    ];
    let mut over = false;
    for (id, what, beats) in worst {
        let Some(ch) = cast::by_id(id) else { continue };
        let poses: Vec<Pose> = beats
            .iter()
            .map(|b| Pose {
                mv: Move::Jump,
                phase: b / 4.0,
                energy: 1.0,
                mood: Mood::Happy,
                playing: true,
                beats: f64::from(*b),
                x: 0.5,
                y: 0.9,
                ..Pose::default()
            })
            .collect();
        for p in &poses {
            render_into(&mut pixmap, &stage, &[(ch.as_ref(), *p)]);
        }
        let mut ms = f64::MAX;
        for _ in 0..15 {
            let t0 = Instant::now();
            for _ in 0..10 {
                for p in &poses {
                    render_into(&mut pixmap, &stage, &[(ch.as_ref(), *p)]);
                }
            }
            ms = ms.min(t0.elapsed().as_secs_f64() * 1000.0 / (10 * poses.len()) as f64);
        }
        println!("{id}: {what} {ms:.3} ms a frame (budget {BUDGET_MS} ms)");
        over |= ms > BUDGET_MS;
    }
    if over {
        eprintln!("over the budget");
        std::process::exit(1);
    }
}
