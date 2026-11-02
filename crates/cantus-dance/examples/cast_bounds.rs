//! Measure what each character draws, in its own units (unlifted, unspun), over every move
//! at 18 phases on both sides, every mood, low and full energy, facing and turned: what its
//! `bounds()` must hold. `cargo run --release -p cantus-dance --example cast_bounds`.

use cantus_dance::render::{placement, render_into, Background, Stage};
use cantus_dance::{cast, pose_at, Character, DanceFrame, Mood, Move, Pose};

fn main() {
    let stage = Stage::new(900, 900, Background::Transparent);
    let mut pm = tiny_skia::Pixmap::new(900, 900).expect("a stage");
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        let base = Pose {
            size: 0.8,
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
        let (mut l, mut t, mut r, mut b) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        let mut who = [String::new(), String::new(), String::new(), String::new()];
        let mut n = 0;
        for mv in Move::ALL {
            for cycle in 0..2u32 {
                for k in 0..18 {
                    let phase = if k == 17 { 0.99 } else { k as f32 / 16.0 };
                    for (energy, turn) in [(1.0, 0.0), (1.0, 1.0), (1.0, -1.0), (0.2, 0.0)] {
                        let mood = Mood::from_index((k as usize + cycle as usize) % 8);
                        let pose = Pose {
                            mv,
                            phase,
                            cycle,
                            energy,
                            turn,
                            mood,
                            beat_phase: phase,
                            seconds: f64::from(k) * 0.37,
                            accent: 1.0,
                            ..base
                        };
                        render_into(&mut pm, &stage, &[(ch, pose)]);
                        let (tf, _) = placement(&stage, ch, &pose);
                        let inv = tf.invert().expect("invertible");
                        for (i, c) in pm.pixels().iter().enumerate() {
                            if c.alpha() == 0 {
                                continue;
                            }
                            let (x, y) = ((i % 900) as f32 + 0.5, (i / 900) as f32 + 0.5);
                            let ux = inv.sx * x + inv.kx * y + inv.tx;
                            let uy = inv.ky * x + inv.sy * y + inv.ty;
                            let tag = || format!("{mv:?}@{phase:.2}/e{energy}/t{turn}/{mood:?}");
                            if ux < l {
                                l = ux;
                                who[0] = tag();
                            }
                            if ux > r {
                                r = ux;
                                who[2] = tag();
                            }
                            if uy < t {
                                t = uy;
                                who[1] = tag();
                            }
                            if uy > b {
                                b = uy;
                                who[3] = tag();
                            }
                        }
                        n += 1;
                    }
                }
            }
        }
        println!(
            "{}: left {l:.3} top {t:.3} right {r:.3} bottom {b:.3} ({n} poses) — by {who:?}",
            ch.info().id
        );
    }
}
