//! The pose at a moment (ADR 0087 §2): a **pure function** of the engine's frame.
//!
//! Nothing here keeps state between calls. A move's phase is counted from the song's own
//! grid — from the bar's start for the routine and an automated `move`, from the note's
//! start for a note — so the pose at a song position is the same whether the screen asks
//! 30 or 144 times a second, and the same in a video. That is the contract every caller
//! relies on (`the_pose_is_a_function_of_the_song`).
//!
//! **Which move** and what it blends from is [`choreo`]'s (notes, then keys still
//! held, then `move`, then the routine — one move a bar); how the body travels, jumps,
//! turns and how hard it dances is [`choreo_motion`]'s. Stopped, it breathes
//! (Idle) on the wall clock where the playhead stands.

// The choreography, in files of its own beside this one (`pose::choreo`,
// `pose::choreo_motion`).
#[path = "choreo.rs"]
pub mod choreo;
#[path = "choreo_motion.rs"]
pub mod choreo_motion;

use crate::character::Character;
use cantus_core::inst::dancer::{self, Mood, Move};
use cantus_engine::dance::DanceFrame;
use choreo::Span;
use choreo_motion as motion;

pub use choreo::BLEND_BEATS;
pub use choreo_motion::STEP_TRAVEL;

/// Everything a character needs to draw one moment.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Pose {
    /// The move playing (already its fallback when the character has none of its own).
    pub mv: Move,
    /// 0…1 through the move's cycle; 0 is a beat (a step lands, a clap hits).
    pub phase: f32,
    /// Whole cycles of this move since it started: what alternates left and right.
    pub cycle: u32,
    /// The move it is leaving and that move's phase (still running), while the change
    /// blends.
    pub from: Option<(Move, f32)>,
    /// The outgoing move's whole cycles where it is now (what side it was on: a
    /// side-alternating move blends from the side it left); 0 with no `from`.
    pub from_cycle: u32,
    /// 0 = all `from`, 1 = all `mv` (eased).
    pub blend: f32,
    /// 0…1 through the current beat (a bob, a breath).
    pub beat_phase: f32,
    /// 0…1: how big the moves are (`energy`, with the level it hears by `react`, and a
    /// note's velocity).
    pub energy: f32,
    pub mood: Mood,
    /// Degrees around the colour wheel its colours turn by.
    pub hue: f32,
    /// −1 facing left … 1 facing right: the knob, the way it walks, a pirouette.
    pub turn: f32,
    /// Degrees of a spin (the renderer turns it about its middle).
    pub spin: f32,
    /// 0…1 of its height off the ground (the renderer lifts it): `jump` and a Jump's arc.
    pub lift: f32,
    /// Where on the stage (0…1, its feet) and how big (× the stage's standard height).
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub visible: bool,
    /// 0 eyes open … 1 closed: a blink, from the song's time.
    pub blink: f32,
    /// Song seconds (or the wall clock, stopped): for what moves on its own — a breath,
    /// hair, a tail. Never a reason for two frame rates to differ.
    pub seconds: f64,
    /// Whether the song plays (stopped, it idles and breathes).
    pub playing: bool,
    /// The song's position in quarter notes from its start (where the playhead stands,
    /// stopped): what swings across bars and metre changes on its own (a pendulum).
    pub beats: f64,
    /// The bar (0-based from bar 1), the beat in it (0-based) and its beats: a phrase's
    /// end (a hop on bar 4's last beat), a wink on the backbeat.
    pub bar: i32,
    pub beat_in_bar: u32,
    pub beats_in_bar: u32,
    /// 0…1: how strong the beat is now — the downbeat strongest, falling through the beat,
    /// bigger with energy. What beat-reactive extras read (a sparkle, a note, a puff).
    pub accent: f32,
}

impl Default for Pose {
    /// Standing still at its defaults: what a still preview draws.
    fn default() -> Self {
        let d = |k: &str| dancer::param(k).map_or(0.0, |p| p.default);
        Pose {
            mv: Move::Idle,
            phase: 0.0,
            cycle: 0,
            from: None,
            from_cycle: 0,
            blend: 1.0,
            beat_phase: 0.0,
            energy: d("energy"),
            mood: Mood::from_index(d("mood") as usize),
            hue: 0.0,
            turn: 0.0,
            spin: 0.0,
            lift: 0.0,
            x: d("x"),
            y: d("y"),
            size: 1.0,
            visible: true,
            blink: 0.0,
            seconds: 0.0,
            playing: false,
            beats: 0.0,
            bar: 0,
            beat_in_bar: 0,
            beats_in_bar: 4,
            accent: 0.0,
        }
    }
}

impl Pose {
    /// One move at one phase, everything else at rest: a preview, a shoot sheet.
    pub fn of(mv: Move, phase: f32) -> Pose {
        Pose {
            mv,
            phase,
            playing: true,
            ..Pose::default()
        }
    }
}

fn value(frame: &DanceFrame, key: &str) -> f32 {
    frame
        .value(key)
        .or_else(|| dancer::param(key).map(|p| p.default))
        .unwrap_or(0.0)
}

/// The move a character plays for `m` (its own or the fallback).
pub fn resolve(ch: &dyn Character, m: Move) -> Move {
    choreo::resolve(ch, m)
}

/// A number in 0…1 from an integer (splitmix64): the same on every system.
pub fn hash01(n: i64) -> f64 {
    let mut z = (n as u64).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// A blink every few seconds, a pure function of `t`.
pub fn blink_at(t: f64) -> f32 {
    const PERIOD: f64 = 3.7;
    const LENGTH: f64 = 0.16;
    let n = (t / PERIOD).floor();
    // A hash of the period's number shifts each blink a little, so it does not tick.
    let h = hash01(n as i64);
    let at = n * PERIOD + h * (PERIOD - LENGTH);
    let d = t - at;
    if (0.0..LENGTH).contains(&d) {
        (1.0 - ((d / LENGTH) * 2.0 - 1.0).abs()) as f32
    } else {
        0.0
    }
}

/// The pose of `ch` for `frame`. `wall_seconds` is used only while the song is stopped (an
/// idle breath on the wall clock); playing, the song's time is the only clock.
pub fn pose_at(frame: &DanceFrame, ch: &dyn Character, wall_seconds: f64) -> Pose {
    let speed = dancer::speed_of(value(frame, "speed"));
    let beats = frame.beats;
    let playing = frame.playing;
    let beat_phase = if playing {
        beats.rem_euclid(1.0) as f32
    } else {
        0.0
    };
    let seconds = if playing { frame.seconds } else { wall_seconds };

    // Which move, from when, and what it blends from. Stopped, it breathes: a beat a
    // second on the wall clock.
    let (span, entered, from, velocity) = if playing {
        let c = choreo::choose(frame, ch, speed);
        (c.now, c.entered, c.from, c.velocity)
    } else {
        // Stopped: a pad or a key held plays its move (an audition), else it breathes —
        // both on the wall clock, a beat a second.
        let held = choreo::held_move(frame);
        let mv = choreo::resolve(ch, held.unwrap_or(Move::Idle));
        let span = Span {
            mv,
            origin: 0.0,
            cycle: choreo::cycle_beats(ch, mv, 1.0),
        };
        let velocity = frame
            .note
            .filter(|n| held.is_some() && Move::from_key(n.key) == held)
            .map_or(1.0, |n| n.velocity);
        (span, f64::NEG_INFINITY, None, velocity)
    };
    let clock = if playing {
        beats
    } else {
        wall_seconds.max(0.0)
    };
    let (phase, cycle) = span.phase_at(clock);
    let since = clock - entered;
    let (from, from_cycle, blend) = match from {
        Some(f) if since < BLEND_BEATS => {
            let (p, c) = f.phase_at(clock);
            (
                Some((f.mv, p)),
                c,
                motion::smooth((since / BLEND_BEATS) as f32),
            )
        }
        _ => (None, 0, 1.0),
    };

    let energy = motion::energy(
        value(frame, "energy"),
        value(frame, "react"),
        value(frame, "level"),
        velocity,
    );

    // Travel and facing, from the song's position playing or not: stopping never jumps.
    let (x, walking) = motion::travel(
        value(frame, "step").round() as i32,
        value(frame, "x"),
        beats,
        speed,
    );
    let knob_turn = motion::facing(value(frame, "turn"), walking);
    let turn_of = |m: Move, p: f32| motion::pirouette(m, p).unwrap_or(knob_turn);
    let (turn, arc) = match from {
        Some((m, p)) => {
            let mix = |a: f32, b: f32| a + (b - a) * blend;
            (
                mix(turn_of(m, p), turn_of(span.mv, phase)),
                mix(
                    motion::arc(m, p, energy),
                    motion::arc(span.mv, phase, energy),
                ),
            )
        }
        None => (turn_of(span.mv, phase), motion::arc(span.mv, phase, energy)),
    };

    let (beat_in_bar, beats_in_bar) =
        motion::beat_in_bar(beats, frame.bar_start_beats, frame.bar_beats);
    Pose {
        mv: span.mv,
        phase,
        cycle,
        from,
        from_cycle,
        blend,
        beat_phase,
        energy,
        mood: Mood::from_index(value(frame, "mood").round().max(0.0) as usize),
        hue: value(frame, "hue"),
        turn: turn.clamp(-1.0, 1.0),
        spin: value(frame, "spin"),
        lift: (value(frame, "jump").clamp(0.0, 1.0) + arc).clamp(0.0, 1.5),
        x,
        y: value(frame, "y"),
        size: value(frame, "size").max(0.01),
        visible: value(frame, "visible") >= 0.5,
        blink: blink_at(seconds),
        seconds,
        playing,
        beats,
        bar: frame.bar,
        beat_in_bar,
        beats_in_bar,
        accent: motion::accent(playing, beat_in_bar, beats_in_bar, beat_phase, energy),
    }
}

#[cfg(test)]
#[path = "pose_tests.rs"]
mod tests;
