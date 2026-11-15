//! The Slate cast (round 40, ADR 0122): what the rig promises its characters beyond what
//! `tests/cast.rs` holds for every one of the cast — pose-to-pose holds that are still, the
//! theme's light, the gold the only colour the hue turns, a face for every mood.

use cantus_dance::render::{render, Background, Stage, StageInk};
use cantus_dance::{cast, Mood, Move, Pose};

const SLATE: [&str; 3] = ["builtin:compas", "builtin:tono", "builtin:bajo"];

fn stage(px: u32, ink: StageInk) -> Stage {
    let mut s = Stage::new(px, px, Background::Solid(ink.room));
    s.ink = ink;
    s
}

fn at(mv: Move, beats: f32) -> Pose {
    let beats_of = cantus_dance::slate::move_beats(mv);
    Pose {
        mv,
        phase: (beats / beats_of).fract(),
        cycle: (beats / beats_of).floor() as u32,
        beat_phase: beats.fract(),
        energy: 0.7,
        mood: Mood::Happy,
        playing: true,
        x: 0.5,
        y: 0.88,
        blink: 0.0,
        ..Pose::default()
    }
}

#[test]
fn a_pose_held_between_two_snaps_is_still() {
    // Compás dances pose to pose: between the snaps (and its motion marks, which look a
    // little back) the drawing does not move.
    let ch = cast::by_id("builtin:compas").expect("Compás");
    let st = stage(300, StageInk::NIGHT);
    let a = render(&st, &[(ch.as_ref(), at(Move::Bounce, 0.30))]).expect("a stage");
    let b = render(&st, &[(ch.as_ref(), at(Move::Bounce, 0.40))]).expect("a stage");
    assert_eq!(a.data(), b.data(), "held");
    let c = render(&st, &[(ch.as_ref(), at(Move::Bounce, 0.52))]).expect("a stage");
    assert_ne!(a.data(), c.data(), "and it snaps on the half beat");
}

#[test]
fn the_figure_is_drawn_in_the_themes_ink() {
    // Compás is one ink: the theme's ink 1, whatever the theme.
    let ch = cast::by_id("builtin:compas").expect("Compás");
    for ink in [StageInk::NIGHT, StageInk::PAPER] {
        let p = render(&stage(300, ink), &[(ch.as_ref(), at(Move::Idle, 0.0))]).expect("a stage");
        let [r, g, b, _] = ink.ink;
        let inked = p
            .pixels()
            .iter()
            .filter(|c| c.alpha() == 255 && (c.red(), c.green(), c.blue()) == (r, g, b))
            .count();
        assert!(inked > 2000, "{inked} pixels of ink 1");
    }
}

#[test]
fn only_the_gold_turns_with_the_hue() {
    // The ink and the room never turn: a hue of 120° leaves the ink's pixels as they were
    // and changes the headphones.
    let ch = cast::by_id("builtin:compas").expect("Compás");
    let st = stage(300, StageInk::NIGHT);
    let plain = at(Move::Idle, 0.0);
    let turned = Pose {
        hue: 120.0,
        ..plain
    };
    let a = render(&st, &[(ch.as_ref(), plain)]).expect("a stage");
    let b = render(&st, &[(ch.as_ref(), turned)]).expect("a stage");
    let [r, g, bl, _] = StageInk::NIGHT.ink;
    let ink = |p: &tiny_skia::Pixmap| {
        p.pixels()
            .iter()
            .filter(|c| (c.red(), c.green(), c.blue()) == (r, g, bl))
            .count()
    };
    assert_eq!(ink(&a), ink(&b), "the ink is the same");
    assert_ne!(a.data(), b.data(), "the gold turned");
}

#[test]
fn every_mood_has_a_face_of_its_own() {
    // Tono's visor and Bajo's lids: the moods at a 300-pixel stage, each different from
    // Neutral's (Happy, the default, keeps each move's own face — the designer's dancing
    // face — so standing it is Neutral's; Compás has no face: only Cool cuts its visor).
    for id in ["builtin:tono", "builtin:bajo"] {
        let ch = cast::by_id(id).expect("a character");
        let st = stage(300, StageInk::NIGHT);
        let draw = |mood: Mood| {
            let p = Pose {
                mood,
                ..at(Move::Idle, 0.0)
            };
            render(&st, &[(ch.as_ref(), p)]).expect("a stage")
        };
        let base = draw(Mood::Neutral);
        for mood in [
            Mood::Excited,
            Mood::Calm,
            Mood::Surprised,
            Mood::Sleepy,
            Mood::Cool,
            Mood::Shy,
        ] {
            assert_ne!(base.data(), draw(mood).data(), "{id} {mood:?}");
        }
    }
    let ch = cast::by_id("builtin:compas").expect("Compás");
    let st = stage(300, StageInk::NIGHT);
    let a = render(&st, &[(ch.as_ref(), at(Move::Idle, 0.0))]).expect("a stage");
    let cool = Pose {
        mood: Mood::Cool,
        ..at(Move::Idle, 0.0)
    };
    let b = render(&st, &[(ch.as_ref(), cool)]).expect("a stage");
    assert_ne!(a.data(), b.data(), "Compás's visor");
}

#[test]
fn the_signature_plays_on_the_drop_and_lands_on_the_beat() {
    // `Jump` is the bar on the drop: the signature in its first two beats, landing on the
    // third, then the groove.
    for id in SLATE {
        let ch = cast::by_id(id).expect("a character");
        assert_eq!(ch.move_beats(Move::Jump), 4.0);
        assert_eq!(
            ch.routine(),
            &[Move::Bounce, Move::StepRight, Move::Jump],
            "{id}: the designer's 12-beat routine"
        );
    }
}

#[test]
fn the_room_is_what_the_cuts_are_drawn_in() {
    // On a stage of another colour the cut follows the room: no Night base colour is left
    // round Compás on Paper.
    let ch = cast::by_id("builtin:compas").expect("Compás");
    let p = render(
        &stage(300, StageInk::PAPER),
        &[(ch.as_ref(), at(Move::Bounce, 0.0))],
    )
    .expect("a stage");
    let [r, g, b, _] = StageInk::NIGHT.room;
    assert!(
        !p.pixels()
            .iter()
            .any(|c| (c.red(), c.green(), c.blue()) == (r, g, b)),
        "no Night room on Paper"
    );
}

#[test]
fn the_lapis_is_the_designers_except_over_the_blue_key() {
    // Cinta's lapis cap and trousers are near the blue key by a keyer's measure: drawn as
    // designed on a stage, quietened over the blue key so the keyer leaves them, and as
    // designed over the green one (nowhere near it).
    let ch = cast::by_id("builtin:cinta").expect("Cinta");
    let lapis = (0x46, 0x64, 0xC0);
    let count = |key: Option<[u8; 4]>| {
        let mut st = stage(300, StageInk::NIGHT);
        st.background = Background::Transparent;
        st.ink.key = key;
        let p = render(&st, &[(ch.as_ref(), at(Move::Idle, 0.0))]).expect("a stage");
        p.pixels()
            .iter()
            .filter(|c| c.alpha() == 255 && (c.red(), c.green(), c.blue()) == lapis)
            .count()
    };
    assert!(count(None) > 200, "the designer's lapis");
    assert_eq!(
        count(Some(cantus_dance::render::CHROMA_BLUE)),
        0,
        "quietened"
    );
    assert_eq!(count(Some(cantus_dance::render::CHROMA_GREEN)), count(None));
}
