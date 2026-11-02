//! The cast holds its promises (ADR 0090), for every included character:
//! every move and mood draws; nothing is drawn outside its `bounds()`; no opaque colour is
//! near a chroma key; the same drawing at 48 and 2 160 pixels; the default is the first.

use cantus_dance::character::Bounds;
use cantus_dance::render::{
    placement, render, Background, Stage, CHROMA_BLUE, CHROMA_GREEN, CHROMA_MAGENTA,
};
use cantus_dance::{cast, pose_at, Character, DanceFrame, Mood, Move, Pose};
use std::sync::Arc;

/// A playing pose with no blend under way (`pose_at` at the song's start blends from
/// standing still, which would draw the old move whatever move a test sets).
fn base_pose(ch: &dyn Character) -> Pose {
    Pose {
        from: None,
        from_cycle: 0,
        blend: 1.0,
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

/// A spread of poses: every move at four phases on both sides, at full energy and at a
/// low one, facing us and turned; every mood standing, mid-bounce and over a few seconds.
fn poses(ch: &dyn Character) -> Vec<Pose> {
    let base = base_pose(ch);
    let mut out = Vec::new();
    for mv in Move::ALL {
        for k in 0..8 {
            let phase = (k % 4) as f32 / 4.0 + 0.03;
            for (energy, turn, mood) in [
                (1.0, 0.0, Mood::Excited),
                (0.2, 0.0, Mood::Happy),
                (1.0, 1.0, Mood::Cool),
                (1.0, -1.0, Mood::Sleepy),
            ] {
                out.push(Pose {
                    mv,
                    phase,
                    cycle: k / 4,
                    energy,
                    turn,
                    mood,
                    beat_phase: phase,
                    seconds: f64::from(k) * 0.61,
                    ..base
                });
            }
        }
    }
    for mood in Mood::ALL {
        for k in 0..6 {
            let phase = k as f32 / 6.0;
            for mv in [Move::Idle, Move::Bounce] {
                out.push(Pose {
                    mv,
                    phase,
                    mood,
                    energy: 1.0,
                    beat_phase: phase,
                    seconds: f64::from(k) * 0.53,
                    blink: if k == 3 { 1.0 } else { 0.0 },
                    ..base
                });
            }
        }
    }
    out
}

fn stage(px: u32) -> Stage {
    Stage::new(px, px, Background::Transparent)
}

#[test]
fn the_default_character_is_the_first_and_the_songbird() {
    let all = cast::builtin();
    assert_eq!(
        all[0].info().id,
        cantus_core::inst::dancer::DEFAULT_CHARACTER
    );
    assert_eq!(cast::default_character().info().id, "builtin:songbird");
    let mut ids: Vec<String> = all.iter().map(|c| c.info().id.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), all.len(), "ids are unique");
    // Every name and blurb is in both locales.
    let locales = env!("CARGO_MANIFEST_DIR").to_string() + "/../cantus-ui/locales/areas/";
    for lang in ["en", "es"] {
        let text =
            std::fs::read_to_string(format!("{locales}cast.{lang}.txt")).expect("the cast area");
        for c in &all {
            for key in [cast::name_key(&c.info().id), cast::blurb_key(&c.info().id)] {
                assert!(
                    text.lines()
                        .any(|l| l.split('=').next().map(str::trim) == Some(key.as_str())),
                    "{key} in cast.{lang}.txt"
                );
            }
        }
    }
    for c in &all {
        let id = c.info().id.trim_start_matches("builtin:");
        assert!(
            !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'),
            "{id}: ids are lowercase ASCII"
        );
    }
}

#[test]
fn every_move_and_mood_draws() {
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        for m in Move::ALL {
            assert!(ch.has_move(m) || ch.has_move(m.fallback()), "{m:?}");
            assert!(ch.move_beats(m) > 0.0);
        }
        assert!(!ch.routine().is_empty());
        for pose in poses(ch).iter().step_by(3) {
            let p = render(&stage(160), &[(ch, *pose)]).expect("a stage");
            let drawn = p.pixels().iter().filter(|c| c.alpha() > 0).count();
            assert!(
                drawn > 1500,
                "{:?} {:?}: only {drawn} pixels",
                pose.mv,
                pose.mood
            );
        }
    }
}

/// Dense poses for the bounds: every move at sixteen phases a cycle, and 0.99, on both
/// sides; the spread of energies, turns, moods, lifts (to the top) and spins rotated
/// through them; playing, on a downbeat, at full accent (every extra out).
fn dense_poses(ch: &dyn Character) -> Vec<Pose> {
    let base = Pose {
        playing: true,
        beat_in_bar: 0,
        accent: 1.0,
        ..base_pose(ch)
    };
    let configs = [
        (1.0, 0.0, Mood::Excited, 0.0, 0.0),
        (1.0, 1.0, Mood::Cool, 1.0, 0.0),
        (0.2, 0.0, Mood::Happy, 0.0, 0.0),
        (1.0, -1.0, Mood::Sleepy, 0.5, 200.0),
        (1.0, 0.0, Mood::Shy, 1.0, 90.0),
    ];
    let mut out = Vec::new();
    let mut n = 0usize;
    for mv in Move::ALL {
        for cycle in 0..2u32 {
            for k in 0..17 {
                let phase = if k == 16 { 0.99 } else { k as f32 / 16.0 };
                let (energy, turn, mood, lift, spin) = configs[n % configs.len()];
                n += 1;
                out.push(Pose {
                    mv,
                    phase,
                    cycle,
                    energy,
                    turn,
                    mood,
                    lift,
                    spin,
                    beat_phase: phase,
                    seconds: n as f64 * 0.29,
                    ..base
                });
            }
        }
    }
    out
}

#[test]
fn nothing_is_drawn_outside_the_bounds() {
    // Each drawn pixel lies inside the character's bounds as the renderer places them
    // (lifted, spun, turned), or as placed on the ground, unlifted (its shadow, dust and
    // confetti are drawn there): one pixel of anti-aliasing to spare.
    const SIDE: u32 = 340;
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        let Bounds {
            left,
            top,
            right,
            bottom,
        } = ch.bounds();
        assert!(left < right && top < bottom);
        let st = stage(SIDE);
        for pose in dense_poses(ch) {
            let pose = Pose { size: 0.45, ..pose };
            let (tf, px) = placement(&st, ch, &pose);
            let ground = Pose {
                lift: 0.0,
                spin: 0.0,
                ..pose
            };
            let (tg, _) = placement(&st, ch, &ground);
            let inv = [
                tf.invert().expect("invertible"),
                tg.invert().expect("invertible"),
            ];
            let slack = 1.5 / px;
            let inside = |x: f32, y: f32| {
                inv.iter().any(|t| {
                    let ux = t.sx * x + t.kx * y + t.tx;
                    let uy = t.ky * x + t.sy * y + t.ty;
                    ux >= left - slack
                        && ux <= right + slack
                        && uy >= top - slack
                        && uy <= bottom + slack
                })
            };
            let p = render(&st, &[(ch, pose)]).expect("a stage");
            for (i, c) in p.pixels().iter().enumerate() {
                if c.alpha() == 0 {
                    continue;
                }
                let (x, y) = (
                    (i as u32 % SIDE) as f32 + 0.5,
                    (i as u32 / SIDE) as f32 + 0.5,
                );
                assert!(
                    inside(x, y),
                    "{}: {:?} at {:.2} (cycle {}, energy {}, turn {}, lift {}, spin {}, {:?}) draws at ({x}, {y}), outside its bounds",
                    ch.info().id,
                    pose.mv,
                    pose.phase,
                    pose.cycle,
                    pose.energy,
                    pose.turn,
                    pose.lift,
                    pose.spin,
                    pose.mood
                );
            }
        }
    }
}

/// Chroma (Cb, Cr) of a straight colour, BT.709, in 0…255 units: what a keyer compares.
fn chroma(r: u8, g: u8, b: u8) -> (f32, f32) {
    let (r, g, b) = (f32::from(r), f32::from(g), f32::from(b));
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    ((b - y) / 1.8556, (r - y) / 1.5748)
}

#[test]
fn no_opaque_colour_is_near_a_chroma_key() {
    // Neutral greys sit about 75 from the keys; a cyan sits about 50 from blue and green,
    // too near. Every fully opaque pixel at hue 0 keeps at least 60.
    const MIN: f32 = 60.0;
    let keys = [CHROMA_GREEN, CHROMA_BLUE, CHROMA_MAGENTA].map(|k| chroma(k[0], k[1], k[2]));
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        let mut worst = f32::MAX;
        for pose in poses(ch).iter().step_by(2) {
            let pose = Pose { hue: 0.0, ..*pose };
            let p = render(&stage(200), &[(ch, pose)]).expect("a stage");
            for c in p.pixels() {
                if c.alpha() != 255 {
                    continue;
                }
                let (cb, cr) = chroma(c.red(), c.green(), c.blue());
                for (kb, kr) in keys {
                    let d = ((cb - kb) * (cb - kb) + (cr - kr) * (cr - kr)).sqrt();
                    worst = worst.min(d);
                    assert!(
                        d >= MIN,
                        "{}: ({}, {}, {}) is {d:.0} from a chroma key in {:?}",
                        ch.info().id,
                        c.red(),
                        c.green(),
                        c.blue(),
                        pose.mv
                    );
                }
            }
        }
        assert!(worst < f32::MAX);
    }
}

/// How much of a character's own square it covers, from its alpha.
fn coverage(ch: &dyn Character, pose: Pose, tall_px: f32) -> f32 {
    let side = (tall_px * 2.2).ceil() as u32;
    let pose = Pose {
        size: tall_px / (0.42 * side as f32),
        ..pose
    };
    let p = render(&stage(side), &[(ch, pose)]).expect("a stage");
    let sum: f32 = p
        .pixels()
        .iter()
        .map(|c| f32::from(c.alpha()) / 255.0)
        .sum();
    sum / (tall_px * tall_px)
}

#[test]
fn the_same_drawing_at_48_and_2160_pixels() {
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        // Low energy, a calm face, no accent: no translucent extras (notes, a lit bulb's
        // halo), only the drawing's own edges.
        let pose = Pose {
            mv: Move::Point,
            phase: 0.1,
            energy: 0.2,
            mood: Mood::Neutral,
            accent: 0.0,
            ..base_pose(ch)
        };
        let small = coverage(ch, pose, 48.0);
        let large = coverage(ch, pose, 2160.0);
        assert!(
            (small - large).abs() / large < 0.08,
            "{}: covers {small:.3} at 48 px and {large:.3} at 2160 px",
            ch.info().id
        );
        // And the edges as sharp at 2160 px as at 48: an edge is one or two pixels of
        // anti-aliasing at any size (a blur, or a small drawing scaled up, would widen it).
        let small = edge_width(ch, pose, 48.0);
        let large = edge_width(ch, pose, 2160.0);
        assert!(
            small <= 3 && large <= 3 && large <= small + 1,
            "{}: edges {small} px wide at 48 px and {large} px at 2160 px",
            ch.info().id
        );
    }
}

/// The alpha of a drawing with the character `tall_px` tall, rows above the feet only
/// (the soft contact shadow aside): (alphas, width, rows).
fn alphas(ch: &dyn Character, pose: Pose, tall_px: f32) -> (Vec<u8>, usize, usize) {
    let side = (tall_px * 2.2).ceil() as u32;
    let pose = Pose {
        size: tall_px / (0.42 * side as f32),
        ..pose
    };
    let p = render(&stage(side), &[(ch, pose)]).expect("a stage");
    let w = side as usize;
    let rows = ((0.85 * side as f32 - 0.1 * tall_px) as usize).min(w);
    let a = p.pixels()[..rows * w].iter().map(|c| c.alpha()).collect();
    (a, w, rows)
}

fn edge_width(ch: &dyn Character, pose: Pose, tall_px: f32) -> usize {
    let (a, w, rows) = alphas(ch, pose, tall_px);
    width_of(&a, w, rows)
}

/// How wide the edges are, in pixels: for each partly covered pixel, the run of partly
/// covered pixels across the edge (the shorter of its horizontal and vertical runs); the
/// 95th percentile of those.
fn width_of(a: &[u8], w: usize, rows: usize) -> usize {
    let soft: Vec<bool> = a.iter().map(|&v| v > 0 && v < 255).collect();
    let mut h = vec![0u16; rows * w];
    for y in 0..rows {
        let mut x = 0;
        while x < w {
            if !soft[y * w + x] {
                x += 1;
                continue;
            }
            let start = x;
            while x < w && soft[y * w + x] {
                x += 1;
            }
            for k in start..x {
                h[y * w + k] = (x - start).min(u16::MAX as usize) as u16;
            }
        }
    }
    let mut runs = Vec::new();
    for x in 0..w {
        let mut y = 0;
        while y < rows {
            if !soft[y * w + x] {
                y += 1;
                continue;
            }
            let start = y;
            while y < rows && soft[y * w + x] {
                y += 1;
            }
            for k in start..y {
                runs.push(usize::from(h[k * w + x]).min(y - start));
            }
        }
    }
    if runs.is_empty() {
        return 0;
    }
    runs.sort_unstable();
    runs[runs.len() * 95 / 100]
}

/// A box blur of radius `r` (what a soft, scaled-up drawing looks like).
fn blurred(a: &[u8], w: usize, rows: usize, r: usize) -> Vec<u8> {
    let pass = |src: &[u8], horizontal: bool| -> Vec<u8> {
        let mut out = vec![0u8; src.len()];
        for y in 0..rows {
            for x in 0..w {
                let (mut sum, mut n) = (0u32, 0u32);
                for d in 0..=2 * r {
                    let (xx, yy) = if horizontal {
                        ((x + d).wrapping_sub(r), y)
                    } else {
                        (x, (y + d).wrapping_sub(r))
                    };
                    if xx < w && yy < rows {
                        sum += u32::from(src[yy * w + xx]);
                        n += 1;
                    }
                }
                out[y * w + x] = (sum / n.max(1)) as u8;
            }
        }
        out
    };
    pass(&pass(a, true), false)
}

#[test]
fn a_blurred_drawing_fails_the_edge_measure() {
    // The measure itself: the songbird crisp at 480 px, then softened by a light blur.
    let ch = cast::default_character();
    let pose = Pose {
        mv: Move::Point,
        phase: 0.1,
        energy: 0.2,
        mood: Mood::Neutral,
        ..base_pose(ch.as_ref())
    };
    let (a, w, rows) = alphas(ch.as_ref(), pose, 480.0);
    let crisp = width_of(&a, w, rows);
    let soft = width_of(&blurred(&a, w, rows, 3), w, rows);
    assert!(
        crisp <= 3 && soft > 3,
        "crisp {crisp} px, blurred {soft} px"
    );
}

#[test]
fn a_blink_and_a_hue_change_the_drawing() {
    let ch: Arc<dyn Character> = cast::default_character();
    let pose = Pose {
        mv: Move::Idle,
        mood: Mood::Neutral,
        ..base_pose(ch.as_ref())
    };
    let open = render(&stage(200), &[(ch.as_ref(), pose)]).expect("a stage");
    let shut = render(&stage(200), &[(ch.as_ref(), Pose { blink: 1.0, ..pose })]).expect("a stage");
    assert_ne!(open.data(), shut.data(), "a blink shows");
    let turned =
        render(&stage(200), &[(ch.as_ref(), Pose { hue: 120.0, ..pose })]).expect("a stage");
    assert_ne!(open.data(), turned.data(), "the hue turns its colours");
}

#[test]
fn a_blend_starts_from_the_old_moves_own_side() {
    // Leaving a step to the left on its odd cycle (the foot coming back in) for a bounce:
    // the first frame of the blend is the step as it was, not its mirror.
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        let base = Pose {
            energy: 0.2,
            mood: Mood::Neutral,
            ..base_pose(ch)
        };
        let leaving = Pose {
            mv: Move::StepLeft,
            phase: 0.6,
            cycle: 1,
            from: None,
            from_cycle: 0,
            blend: 1.0,
            ..base
        };
        let blending = Pose {
            mv: Move::Bounce,
            phase: 0.0,
            cycle: 0,
            from: Some((Move::StepLeft, 0.6)),
            from_cycle: 1,
            blend: 0.0,
            ..base
        };
        let a = render(&stage(200), &[(ch, leaving)]).expect("a stage");
        let b = render(&stage(200), &[(ch, blending)]).expect("a stage");
        assert_eq!(a.data(), b.data(), "{}", ch.info().id);
    }
}

/// A frame of a song playing with `move` automated to `mv` at `energy`, at `beats`.
fn dancer_frame(mv: Move, energy: f32, beats: f64) -> DanceFrame {
    let mut f = DanceFrame {
        playing: true,
        beats,
        seconds: beats * 0.5,
        bar: (beats / 4.0).floor() as i32,
        bar_start_beats: (beats / 4.0).floor() * 4.0,
        ..DanceFrame::default()
    };
    let k = |key: &str| {
        cantus_core::inst::dancer::KEYS
            .iter()
            .position(|x| *x == key)
            .expect("a key")
    };
    f.values[k("move")] = (mv.index() + 1) as f32;
    f.values[k("energy")] = energy;
    f
}

#[test]
fn the_dancers_own_poses_stay_in_the_bounds() {
    // Poses as the dancer makes them (`pose_at`: the move automated, its blends and arcs),
    // at energy 0 and 1, through two bars at quarter beats, lifted to 0, 0.5 and 1.5: each
    // pixel within the character's bounds as placed, or on the floor (as the stage's box).
    const SIDE: u32 = 360;
    let st = stage(SIDE);
    for ch in cast::builtin() {
        let ch: &dyn Character = ch.as_ref();
        let b = ch.bounds();
        for mv in Move::ALL {
            for energy in [0.0, 1.0] {
                for q in 0..32 {
                    let beats = 4.0 + f64::from(q) * 0.25;
                    let lift = [0.0, 0.5, 1.5][q as usize % 3];
                    let pose = Pose {
                        lift,
                        size: 0.38,
                        ..pose_at(&dancer_frame(mv, energy, beats), ch, 0.0)
                    };
                    let mut grounded = pose;
                    grounded.lift = 0.0;
                    grounded.spin = 0.0;
                    let mut boxes = Vec::new();
                    let mut slack = 1.0;
                    for p in [pose, grounded] {
                        let (t, px) = placement(&st, ch, &p);
                        slack = 1.5 / px;
                        boxes.push(t.invert().expect("invertible"));
                    }
                    let p = render(&st, &[(ch, pose)]).expect("a stage");
                    let outside: Vec<(f32, f32)> = p
                        .pixels()
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| c.alpha() > 0)
                        .map(|(i, _)| {
                            (
                                (i as u32 % SIDE) as f32 + 0.5,
                                (i as u32 / SIDE) as f32 + 0.5,
                            )
                        })
                        .filter(|&(x, y)| {
                            !boxes.iter().any(|t| {
                                let ux = t.sx * x + t.kx * y + t.tx;
                                let uy = t.ky * x + t.sy * y + t.ty;
                                ux >= b.left - slack
                                    && ux <= b.right + slack
                                    && uy >= b.top - slack
                                    && uy <= b.bottom + slack
                            })
                        })
                        .collect();
                    // Where they are, in the character's units (as placed, unlifted).
                    let g = &boxes[1];
                    let units: Vec<(f32, f32)> = outside
                        .iter()
                        .map(|&(x, y)| (g.sx * x + g.kx * y + g.tx, g.ky * x + g.sy * y + g.ty))
                        .collect();
                    let ext = units
                        .iter()
                        .fold((f32::MAX, f32::MAX, f32::MIN, f32::MIN), |e, &(x, y)| {
                            (e.0.min(x), e.1.min(y), e.2.max(x), e.3.max(y))
                        });
                    assert!(
                        outside.is_empty(),
                        "{}: {mv:?} at beat {beats} (energy {energy}, lift {lift}, phase {:.2}, turn {:.2}, spin {:.0}, from {:?}): {} px outside its bounds, at {ext:?}; pose {pose:?}",
                        ch.info().id,
                        pose.phase,
                        pose.turn,
                        pose.spin,
                        pose.from,
                        outside.len()
                    );
                }
            }
        }
    }
}

/// Over a chroma key a contact shadow would come out as a darker shade of the key, which a
/// keyer leaves as a smudge under the feet: a stage over a key draws none, and every
/// character leaves its own out when the stage says so (the companion's green, a video's
/// key). Over anything else the shadow stays.
#[test]
fn over_a_chroma_key_no_character_draws_its_contact_shadow() {
    for key in [CHROMA_GREEN, CHROMA_BLUE, CHROMA_MAGENTA] {
        assert!(!Stage::new(64, 64, Background::Solid(key)).shadows);
    }
    assert!(Stage::new(64, 64, Background::Transparent).shadows);
    assert!(Stage::new(64, 64, Background::Solid([0x1f, 0x17, 0x12, 255])).shadows);
    for ch in cast::builtin() {
        let pose = Pose {
            mv: Move::Idle,
            ..base_pose(ch.as_ref())
        };
        let keyed = Stage::new(240, 240, Background::Solid(CHROMA_GREEN));
        let with = Stage {
            shadows: true,
            ..keyed
        };
        let drawn = |s: &Stage| render(s, &[(ch.as_ref(), pose)]).unwrap();
        let (none, shadowed) = (drawn(&keyed), drawn(&with));
        assert_ne!(
            none.data(),
            shadowed.data(),
            "{}: its shadow is drawn when asked",
            ch.info().id
        );
        // Nothing but the character itself touches the key: wherever it draws nothing (its
        // shadow left out, over a clear stage), the key is exact.
        let clear = Stage {
            shadows: false,
            ..Stage::new(240, 240, Background::Transparent)
        };
        let alone = drawn(&clear);
        let key = [CHROMA_GREEN[0], CHROMA_GREEN[1], CHROMA_GREEN[2]];
        let stray = none
            .pixels()
            .iter()
            .zip(alone.pixels())
            .filter(|(k, a)| {
                a.alpha() == 0 && {
                    let k = k.demultiply();
                    [k.red(), k.green(), k.blue()] != key
                }
            })
            .count();
        assert_eq!(stray, 0, "{}: pixels off the key outside it", ch.info().id);
    }
}
