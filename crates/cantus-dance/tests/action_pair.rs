//! Fuga the ninja and Volta the mecha (round 37, ADR 0105), through the one renderer:
//! Volta's light never turns with the hue, each of its moods is its own face at the size
//! the stage shows it, and both draw the same twice. (Unit tests hold the rest: Fuga's
//! aerial lands upright on the beat, `ninja/draw.rs`; Volta's pop and lock and its jets,
//! `mecha/moves.rs`.)

use cantus_dance::render::{render, Background, Stage};
use cantus_dance::{cast, pose_at, Character, DanceFrame, Mood, Move, Pose};
use std::sync::Arc;

fn get(id: &str) -> Arc<dyn Character> {
    cast::by_id(id).expect("in the cast")
}

fn base(ch: &dyn Character) -> Pose {
    Pose {
        from: None,
        blend: 1.0,
        blink: 0.0,
        ..pose_at(
            &DanceFrame {
                playing: true,
                ..DanceFrame::default()
            },
            ch,
            0.0,
        )
    }
}

fn pixels(ch: &dyn Character, pose: Pose, px: u32) -> Vec<[u8; 4]> {
    let stage = Stage::new(px, px, Background::Transparent);
    let p = render(&stage, &[(ch, pose)]).expect("a stage");
    p.pixels()
        .iter()
        .map(|c| [c.red(), c.green(), c.blue(), c.alpha()])
        .collect()
}

/// Volta's light: the glow's own colour, opaque.
const GLOW: [u8; 4] = [255, 70, 74, 255];

#[test]
fn voltas_light_never_turns_with_the_hue() {
    let ch = get("builtin:mecha");
    let count = |hue: f32| {
        let pose = Pose {
            mv: Move::Bounce,
            mood: Mood::Neutral,
            hue,
            ..base(ch.as_ref())
        };
        pixels(ch.as_ref(), pose, 500)
            .iter()
            .filter(|c| **c == GLOW)
            .count()
    };
    let still = count(0.0);
    assert!(still > 40, "the light shows ({still} pixels)");
    for hue in [-120.0, 90.0, 180.0] {
        assert_eq!(count(hue), still, "at {hue}° the light is the same");
    }
}

#[test]
fn every_mood_of_volta_is_its_own_face_at_stage_size() {
    // A 300-pixel stage shows it about 120 pixels tall: each mood must differ from every
    // other in more than a speck.
    let ch = get("builtin:mecha");
    let face = |mood: Mood| {
        let pose = Pose {
            mv: Move::Idle,
            mood,
            seconds: 0.2,
            playing: false,
            ..base(ch.as_ref())
        };
        pixels(ch.as_ref(), pose, 300)
    };
    let faces: Vec<_> = Mood::ALL.iter().map(|m| face(*m)).collect();
    for i in 0..faces.len() {
        for j in i + 1..faces.len() {
            let differ = faces[i]
                .iter()
                .zip(&faces[j])
                .filter(|(a, b)| (0..3).any(|k| a[k].abs_diff(b[k]) > 40))
                .count();
            assert!(
                differ >= 8,
                "{:?} and {:?} differ in {differ} pixels",
                Mood::ALL[i],
                Mood::ALL[j]
            );
        }
    }
}

#[test]
fn the_pair_draw_the_same_twice() {
    for id in ["builtin:ninja", "builtin:mecha"] {
        let ch = get(id);
        for mv in Move::ALL {
            let pose = Pose {
                mv,
                phase: 0.37,
                energy: 1.0,
                ..base(ch.as_ref())
            };
            assert_eq!(
                pixels(ch.as_ref(), pose, 160),
                pixels(ch.as_ref(), pose, 160),
                "{id} {mv:?}"
            );
        }
    }
}
