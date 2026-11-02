//! Every rule of the choreography, as a pure function (`pose.rs`, `choreo*.rs`).
//!
//! A stub character with every move of its own and a known routine keeps these tests
//! about the rules, whatever the cast draws; the stand-in (or the cast's first character)
//! checks the contract's own promises.

use super::*;
use crate::cast;
use crate::character::{CharacterInfo, Licence, Origin};
use crate::render::Canvas;
use cantus_engine::dance::DanceNote;

/// Every move its own, the contract's cycles, a routine of four.
struct Stub {
    info: CharacterInfo,
    only: Option<&'static [Move]>,
}

const ROUTINE: [Move; 4] = [Move::Bounce, Move::Clap, Move::StepLeft, Move::Wave];

impl Stub {
    fn new() -> Stub {
        Stub {
            info: CharacterInfo {
                id: "builtin:stub".into(),
                name: "Stub".into(),
                author: "tests".into(),
                origin: Origin::Builtin,
                licence: Licence::Cantus,
            },
            only: None,
        }
    }
}

impl Character for Stub {
    fn info(&self) -> &CharacterInfo {
        &self.info
    }
    fn has_move(&self, m: Move) -> bool {
        self.only.is_none_or(|o| o.contains(&m))
    }
    fn routine(&self) -> &[Move] {
        &ROUTINE
    }
    fn draw(&self, _pose: &Pose, _canvas: &mut Canvas) {}
}

/// Playing at `beats`, 120 BPM in 4/4, the dancer at its defaults.
fn at(beats: f64) -> DanceFrame {
    DanceFrame {
        beats,
        seconds: beats * 0.5,
        playing: true,
        bar: (beats / 4.0).floor() as i32,
        bar_start_beats: (beats / 4.0).floor() * 4.0,
        ..DanceFrame::default()
    }
}

fn set(mut f: DanceFrame, key: &str, v: f32) -> DanceFrame {
    let i = dancer::KEYS.iter().position(|k| *k == key).unwrap();
    f.values[i] = v;
    f
}

fn key(m: Move) -> u8 {
    dancer::BASE_KEY + m.index() as u8
}

fn note(m: Move, beats: f64, released: Option<f64>, velocity: f32) -> DanceNote {
    DanceNote {
        key: key(m),
        velocity,
        beats,
        seconds: beats * 0.5,
        count: 1,
        released,
    }
}

fn hold(mut f: DanceFrame, k: u8) -> DanceFrame {
    f.held[usize::from(k) / 64] |= 1 << (k % 64);
    f
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

#[test]
fn the_pose_is_a_function_of_the_song() {
    let stub = Stub::new();
    let placeholder = cast::default_character();
    for ch in [&stub as &dyn Character, placeholder.as_ref()] {
        // The same song positions reached at 30 and at 144 frames a second, with notes,
        // automation and travel: the wall clock never matters while playing.
        for i in 0..1_000 {
            let b = f64::from(i) * 0.031;
            let mut f = set(set(at(b), "step", 2.0), "move", (i / 300) as f32);
            f.note = Some(note(Move::Jump, 5.0, Some(6.5), 0.8));
            let a = pose_at(&f, ch, 1.0);
            assert_eq!(a, pose_at(&f, ch, 99.0), "beat {b}");
            assert_eq!(a, pose_at(&f.clone(), ch, -5.0), "beat {b}");
        }
    }
}

#[test]
fn the_routine_is_one_move_a_bar_counted_from_the_bar_line() {
    let ch = Stub::new();
    for bar in 0..12 {
        let b = f64::from(bar) * 4.0;
        let p = pose_at(&at(b + 1.5), &ch, 0.0);
        assert_eq!(p.mv, ROUTINE[bar as usize % 4], "bar {bar}");
        let cycle = f64::from(ch.move_beats(p.mv));
        let want = ((1.5 / cycle).fract()) as f32;
        assert!(
            near(p.phase, want),
            "bar {bar}: phase {} vs {want}",
            p.phase
        );
        assert_eq!(p.bar, bar);
        assert_eq!((p.beat_in_bar, p.beats_in_bar), (1, 4));
    }
}

#[test]
fn a_bar_line_blends_from_the_bar_before() {
    let ch = Stub::new();
    let p = pose_at(&at(4.1), &ch, 0.0);
    assert_eq!(p.mv, Move::Clap);
    let (from, fp) = p.from.expect("a blend from Bounce");
    assert_eq!(from, Move::Bounce);
    assert!(near(fp, 0.1), "Bounce's phase runs on: {fp}");
    assert!(p.blend > 0.0 && p.blend < 1.0);
    let p = pose_at(&at(4.0 + BLEND_BEATS), &ch, 0.0);
    assert_eq!((p.from, p.blend), (None, 1.0), "over after a quarter beat");
    // The same move on both sides, its phase continuous: nothing to blend.
    let f = set(at(8.05), "move", dancer::move_value(Move::Clap));
    assert_eq!(pose_at(&f, &ch, 0.0).from, None);
}

#[test]
fn an_automated_move_counts_from_the_bar_and_blends_where_it_changed() {
    let ch = Stub::new();
    let wave = dancer::move_value(Move::Wave);
    let f = set(at(9.5), "move", wave);
    let p = pose_at(&f, &ch, 0.0);
    assert_eq!(p.mv, Move::Wave);
    assert!(
        near(p.phase, 0.75),
        "1.5 beats into a two-beat move: {}",
        p.phase
    );
    // Changed at beat 9 from the routine: a blend from the bar's routine move (Bounce,
    // bar 2), there and only there.
    let mut f = set(at(9.1), "move", wave);
    f.move_change = Some((9.0, 0.0));
    let p = pose_at(&f, &ch, 0.0);
    assert_eq!(p.from.map(|x| x.0), Some(ROUTINE[2]));
    f.beats = 9.3;
    assert_eq!(pose_at(&f, &ch, 0.0).from, None);
    // Changed on the bar line itself: the blend starts there, from the move it left.
    let mut f = set(at(8.1), "move", wave);
    f.move_change = Some((8.0, dancer::move_value(Move::Clap)));
    assert_eq!(pose_at(&f, &ch, 0.0).from.map(|x| x.0), Some(Move::Clap));
}

#[test]
fn a_note_plays_its_move_from_its_start_and_finishes_its_cycle() {
    let ch = Stub::new();
    // Held: Wave (two beats) from beat 10, blending in from the bar's move.
    let mut f = hold(at(10.5), key(Move::Wave));
    f.note = Some(note(Move::Wave, 10.0, None, 1.0));
    let p = pose_at(&f, &ch, 0.0);
    assert_eq!(p.mv, Move::Wave);
    assert!(near(p.phase, 0.25));
    f.beats = 10.1;
    let p = pose_at(&f, &ch, 0.0);
    assert_eq!(p.from.map(|x| x.0), Some(ROUTINE[2]), "from the bar's move");
    f.beats = 10.5;
    // Let go at 10.5: it finishes its cycle (to 12), then the bar's move takes over.
    f.held = [0; 2];
    f.note = Some(note(Move::Wave, 10.0, Some(10.5), 1.0));
    f.beats = 11.9;
    assert_eq!(pose_at(&f, &ch, 0.0).mv, Move::Wave);
    let mut g = at(12.1);
    g.note = f.note;
    let p = pose_at(&g, &ch, 0.0);
    assert_eq!(p.mv, ROUTINE[3]);
    assert_eq!(
        p.from.map(|x| x.0),
        Some(Move::Wave),
        "from the note's move"
    );
    // Let go a whole cycle in (a one-beat Kick let go at 11, exactly): over at 11.
    let mut g = at(11.5);
    g.note = Some(note(Move::Kick, 10.0, Some(11.0), 1.0));
    assert_eq!(pose_at(&g, &ch, 0.0).mv, ROUTINE[2]);
    g.beats = 10.99;
    assert_eq!(pose_at(&g, &ch, 0.0).mv, Move::Kick);
    // A tap plays one whole cycle.
    let mut g = at(11.8);
    g.note = Some(note(Move::Wave, 10.0, Some(10.01), 1.0));
    assert_eq!(pose_at(&g, &ch, 0.0).mv, Move::Wave);
}

#[test]
fn a_note_ahead_of_the_song_has_not_happened() {
    // After a loop wrap the last note started later in the loop than where the song is.
    let ch = Stub::new();
    let mut f = at(1.0);
    f.note = Some(note(Move::Jump, 14.0, Some(14.5), 1.0));
    assert_eq!(pose_at(&f, &ch, 0.0).mv, ROUTINE[0]);
}

#[test]
fn a_blend_knows_the_cycle_the_move_it_left_was_in() {
    let ch = Stub::new();
    // Bar 2's Clap (one beat) took over from bar 1's Bounce at beat 4: Bounce was in its
    // fifth cycle there (cycles 0…4 over beats 0…5 → at 4.1, cycle 4).
    let p = pose_at(&at(4.1), &ch, 0.0);
    assert_eq!(p.from.map(|f| f.0), Some(Move::Bounce));
    assert_eq!(p.from_cycle, 4);
    // A note taking over from a note: the outgoing one's own cycles since its start.
    let mut f = hold(at(7.1), key(Move::Kick));
    f.note = Some(note(Move::Kick, 7.0, None, 1.0));
    f.prev_note = Some(note(Move::StepLeft, 4.0, Some(7.0), 1.0));
    let p = pose_at(&f, &ch, 0.0);
    assert_eq!(p.from.map(|f| f.0), Some(Move::StepLeft));
    assert_eq!(p.from_cycle, 3, "three whole steps since beat 4");
    // No blend, no cycle.
    let p = pose_at(&at(5.5), &ch, 0.0);
    assert_eq!((p.from, p.from_cycle), (None, 0));
}

#[test]
fn a_note_blends_from_the_note_it_took_over_from() {
    let ch = Stub::new();
    let mut f = hold(at(5.1), key(Move::Kick));
    f.note = Some(note(Move::Kick, 5.0, None, 1.0));
    f.prev_note = Some(note(Move::Spin, 4.0, Some(5.0), 1.0));
    let p = pose_at(&f, &ch, 0.0);
    assert_eq!(p.mv, Move::Kick);
    let (from, fp) = p.from.unwrap();
    assert_eq!(from, Move::Spin);
    assert!(near(fp, 0.55), "Spin's phase at 5.1: {fp}");
    // A previous note long over: the blend is from the bar's move.
    f.prev_note = Some(note(Move::Spin, 0.0, Some(0.5), 1.0));
    assert_eq!(pose_at(&f, &ch, 0.0).from.map(|x| x.0), Some(ROUTINE[1]));
}

#[test]
fn a_key_still_held_takes_over_when_the_latest_note_is_over() {
    let ch = Stub::new();
    let mut f = hold(at(7.3), key(Move::Sway));
    f.note = Some(note(Move::Clap, 6.0, Some(6.2), 1.0));
    let p = pose_at(&f, &ch, 0.0);
    assert_eq!(p.mv, Move::Sway);
    // Its phase counts from where the clap ended (beat 7); a one-beat move.
    assert!(near(p.phase, 0.3), "{}", p.phase);
    assert_eq!(p.from, None, "the blend from the clap is over");
}

#[test]
fn velocity_scales_a_notes_energy() {
    let ch = Stub::new();
    let soft = {
        let mut f = hold(at(2.2), key(Move::Clap));
        f.note = Some(note(Move::Clap, 2.0, None, 0.2));
        pose_at(&f, &ch, 0.0).energy
    };
    let hard = {
        let mut f = hold(at(2.2), key(Move::Clap));
        f.note = Some(note(Move::Clap, 2.0, None, 1.0));
        pose_at(&f, &ch, 0.0).energy
    };
    assert!(soft < hard);
    assert!(near(hard, 0.7), "full velocity: the knob");
}

#[test]
fn react_hands_the_energy_to_the_level_it_hears() {
    let ch = Stub::new();
    let f = set(
        set(set(at(1.0), "react", 1.0), "level", 0.25),
        "energy",
        0.9,
    );
    assert!(near(pose_at(&f, &ch, 0.0).energy, 0.25));
    let f = set(set(at(1.0), "react", 0.0), "level", 0.25);
    assert!(near(pose_at(&f, &ch, 0.0).energy, 0.7));
}

#[test]
fn a_jump_lifts_it_and_lands_on_the_beat() {
    let ch = Stub::new();
    let jump = dancer::move_value(Move::Jump);
    let top = pose_at(&set(at(4.625), "move", jump), &ch, 0.0);
    assert!(top.lift > 0.2, "{}", top.lift);
    let landed = pose_at(&set(at(5.0), "move", jump), &ch, 0.0);
    assert!(landed.lift < 1e-3);
    // The `jump` knob lifts it whatever it dances.
    let f = set(at(1.3), "jump", 0.4);
    assert!(near(pose_at(&f, &ch, 0.0).lift, 0.4));
}

#[test]
fn a_spin_goes_right_round_and_the_knob_turns_it() {
    let ch = Stub::new();
    let spin = dancer::move_value(Move::Spin);
    let half = pose_at(&set(at(9.0), "move", spin), &ch, 0.0);
    assert!(near(half.turn, -1.0), "half way, its back: {}", half.turn);
    let f = set(set(at(1.0), "turn", -0.6), "spin", 90.0);
    let p = pose_at(&f, &ch, 0.0);
    assert!(near(p.turn, -0.6) && p.spin == 90.0);
}

#[test]
fn walking_travels_and_stopping_never_jumps() {
    let ch = Stub::new();
    let f = set(at(6.0), "step", 2.0);
    let p = pose_at(&f, &ch, 0.0);
    assert!(p.x > 0.5 && p.turn > 0.5, "walking right, facing right");
    let mut stopped = f;
    stopped.playing = false;
    assert!(
        near(pose_at(&stopped, &ch, 3.0).x, p.x),
        "stopped where it stood"
    );
}

#[test]
fn stopped_it_breathes_on_the_wall_clock() {
    let ch = Stub::new();
    let f = DanceFrame::default();
    let a = pose_at(&f, &ch, 3.0);
    assert_eq!(a.mv, Move::Idle);
    assert!(!a.playing && a.accent == 0.0);
    assert_ne!(a.phase, pose_at(&f, &ch, 4.0).phase, "it breathes");
}

#[test]
fn stopped_a_held_pad_plays_its_move_on_the_wall_clock() {
    let ch = Stub::new();
    let mut f = hold(DanceFrame::default(), key(Move::Clap));
    f.note = Some(note(Move::Clap, 0.0, None, 0.4));
    let a = pose_at(&f, &ch, 3.25);
    assert_eq!(a.mv, Move::Clap, "an audition while stopped");
    assert!(!a.playing);
    assert_ne!(
        a.phase,
        pose_at(&f, &ch, 3.5).phase,
        "it moves on the wall clock"
    );
    assert!(a.energy < pose_at(&hold(DanceFrame::default(), key(Move::Clap)), &ch, 3.25).energy);
    // Let go: it breathes again.
    f.held = [0; 2];
    assert_eq!(pose_at(&f, &ch, 3.25).mv, Move::Idle);
}

#[test]
fn a_move_the_character_lacks_plays_its_fallback() {
    let mut ch = Stub::new();
    ch.only = Some(&[Move::Idle, Move::Bounce]);
    let f = set(at(1.0), "move", dancer::move_value(Move::Kick));
    assert_eq!(pose_at(&f, &ch, 0.0).mv, Move::Bounce);
    let f = set(at(1.0), "move", dancer::move_value(Move::Bow));
    assert_eq!(pose_at(&f, &ch, 0.0).mv, Move::Idle);
}

#[test]
fn the_beat_accent_is_strongest_on_the_downbeat() {
    let ch = Stub::new();
    let down = pose_at(&at(8.0), &ch, 0.0).accent;
    let back = pose_at(&at(10.0), &ch, 0.0).accent;
    let off = pose_at(&at(9.0), &ch, 0.0).accent;
    assert!(down > back && back > off && off > 0.0);
    assert!(pose_at(&at(8.5), &ch, 0.0).accent < down);
}

#[test]
fn the_view_numbers_pass_through() {
    let ch = Stub::new();
    let f = set(
        set(set(set(at(1.0), "x", 0.2), "y", 0.6), "size", 2.0),
        "visible",
        0.0,
    );
    let p = pose_at(&set(set(f, "hue", 45.0), "mood", 6.0), &ch, 0.0);
    assert_eq!((p.x, p.y, p.size, p.visible), (0.2, 0.6, 2.0, false));
    assert_eq!((p.hue, p.mood), (45.0, Mood::Cool));
}

#[test]
fn the_pose_carries_the_songs_beat_position() {
    let ch = Stub::new();
    assert_eq!(pose_at(&at(37.25), &ch, 0.0).beats, 37.25);
    let mut stopped = at(12.5);
    stopped.playing = false;
    assert_eq!(
        pose_at(&stopped, &ch, 3.0).beats,
        12.5,
        "where the playhead stands"
    );
}

#[test]
fn a_pose_of_one_move_is_still() {
    let p = Pose::of(Move::Clap, 0.25);
    assert_eq!(
        (p.mv, p.phase, p.blend, p.from),
        (Move::Clap, 0.25, 1.0, None)
    );
    assert_eq!(Pose::default().mood, Mood::Happy);
}
