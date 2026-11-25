//! r42 arcus-expr — the bow's state and the player (a child of `arcus`; ADR 0162): where the
//! bow is along the stick, which way it moves, where it came from, and the fingering that says
//! which string a note is played on and whether a slur is a shift. **The single definition**
//! of each, for every later layer (transitions, look-ahead, portamento kinds: the survey's
//! items 1, 7, 10 — `docs/round42/arcus-expr.md`, *Next*).
//!
//! **The bow along the stick** (`x`, 0 the frog … 1 the tip). A stroke starts where its
//! direction and part put it, moves by its speed over the hair's length, and the force the arm
//! gives falls from the frog to the tip — the bow's own weight and the arm's leverage over it,
//! which players only partly make up (Askenfelt 1989; Guettler 2003's "~3 N between frog and
//! tip" on the violin): `weight(x)` × the stroke's force. As the bow runs out the player slows
//! it and moves nearer the bridge to keep the level (the amplitude goes as `v/β`, Schelleng's
//! force limit too, so the force stays put); when it has run out, the bow changes.
//!
//! **Where the bow comes from.** *From the string*: the bow is set on the string first, its
//! force placed before any motion (Guettler 2002's "good" attacks start this way), then it
//! accelerates, and its force and speed settle into the stroke over a contour of 0.1–0.3 s —
//! a player's stroke has a shape (Maestre 2010's per-note Bézier contours; Demoucron 2008),
//! which is what makes the spectrum settle over the players' 0.24–0.42 s (`t_steady`) rather
//! than switching on. *From the air*: the bow lands already moving and the force arrives at
//! once, a little over the stroke's: the string is grabbed — the short crunch of an accent or
//! a martelé, settling fast.
//!
//! **The fingering** (a first, simple engine; the survey's item 2): first position on the
//! highest string that reaches the note, except that an expressive note at an open string's
//! pitch is stopped on the string below (a fourth finger, so it can vibrate) and a note may
//! name its string (*sul G*); a slur on one string by three semitones or more is a **shift**.
//! An open string has no vibrato.

use super::tables::{Instrument, InstrumentSpec};
use super::BowDir;

/// Which part of the bow a stroke is played with (a note's own, ADR 0162).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Frog,
    Middle,
    Tip,
    /// The whole bow: from one end.
    Whole,
}

/// Where the bow comes from at a note's start (a note's own, ADR 0162).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Start {
    /// Set on the string, the force placed before the motion.
    String,
    /// Landing from the air, already moving.
    Air,
}

/// The hair's length, m (bows: violin ~65 cm of hair, viola ~64, cello ~61, French bass ~57).
pub fn hair_m(inst: Instrument) -> f32 {
    match inst {
        Instrument::Violin => 0.65,
        Instrument::Viola => 0.64,
        Instrument::Cello => 0.61,
        Instrument::Bass => 0.57,
    }
}

/// How much more force the arm gives at the frog than at the middle, and less at the tip, at
/// `bow.weight` 1: ±7.5 % (a share of the bow's weight players do not make up). No open
/// measurement of bow force along the stroke was found; at ±17 % the held-out bowed pitch of
/// the violin's open G at pp moved 4 cents (the pitch tables are measured at pass 6's stroke
/// profile, ±5 % at a stroke's start, which this keeps), `arcus-expr.md` §3.
pub const WEIGHT: f32 = 0.15;

/// The force's factor at `x` (0 frog … 1 tip): `1 + WEIGHT·amount·(0.5 − x)`.
pub fn weight(x: f32, amount: f32) -> f32 {
    1.0 + WEIGHT * amount * (0.5 - x.clamp(0.0, 1.0))
}

/// Where a stroke from silence starts: by its part, or the player's choice — the frog for a
/// down-bow and the tip for an up-bow, or the middle in a fast passage (an inter-onset under
/// `FAST_S`: fast détaché moves to the middle and upper half, the survey's item 8).
pub fn start_x(dir: BowDir, part: Option<Part>, fast: bool) -> f32 {
    let down = dir == BowDir::Down;
    match part {
        Some(Part::Frog) => {
            if down {
                0.05
            } else {
                0.3
            }
        }
        Some(Part::Middle) => {
            if down {
                0.4
            } else {
                0.6
            }
        }
        Some(Part::Tip) => {
            if down {
                0.7
            } else {
                0.95
            }
        }
        Some(Part::Whole) => {
            if down {
                0.03
            } else {
                0.97
            }
        }
        None if fast => {
            if down {
                0.45
            } else {
                0.6
            }
        }
        // Near the frog, near the tip (0.15 / 0.85: the arm's weight there ×1.05 / ×0.95, pass
        // 6's stroke profile at a stroke's start, where every attack gate was measured).
        None => {
            if down {
                0.15
            } else {
                0.85
            }
        }
    }
}

/// A passage this fast (the time from the last note-on, s) is a fast one.
pub const FAST_S: f32 = 0.22;

/// What is left of the bow ahead of a stroke at `x` moving `dir` (+1 down, −1 up), m.
pub fn left_m(x: f32, dir: f32, hair: f32) -> f32 {
    hair * if dir >= 0.0 { 1.0 - x } else { x }.clamp(0.0, 1.0)
}

/// The player slows the bow when this much of it is left, m …
pub const SLOW_M: f32 = 0.12;
/// … to at most this share of its speed, and changes bow with this much left (m; negative:
/// never). **Built, measured, and off** (`arcus-expr.md` §3): a player who cannot know how long
/// a note is runs out of bow in 2–4 s at the model's speeds (0.10–0.30 m/s over 0.57–0.65 m of
/// hair), so the gates' single 3–5 s notes — the recordings behind them are single bows — got a
/// bow change in their windows: the slur's pitch 2.3 cents off at 5 s, the bass's vibrato extent
/// jitter 0.57 against the players' 0.19–0.36; the slow-down alone (to 0.4–0.8 × the speed)
/// moved the bowed pitch, its table measured at the dynamic's speed. Both wait for look-ahead
/// on playback (the survey's item 7), which knows a note's length and can plan the bow: its
/// speed, where it changes. Until then the bow, run out, stays at its end (pass 6's way).
pub const SLOW_MIN: f32 = 1.0;
pub const CHANGE_M: f32 = -1.0;

/// The speed's factor with `left` metres of bow ahead (1 with plenty, falling to
/// [`SLOW_MIN`] as it runs out), at the player's `amount`.
pub fn slow(left: f32, amount: f32) -> f32 {
    if left >= SLOW_M {
        return 1.0;
    }
    let k = (left / SLOW_M).clamp(0.0, 1.0);
    let s = SLOW_MIN + (1.0 - SLOW_MIN) * k.sqrt();
    1.0 + (s - 1.0) * amount.clamp(0.0, 1.0)
}

/// The stroke's start, from the string: the force placed (s) before the motion, at this share
/// of the attack's force …
pub const PLACE_S: f32 = 0.018;
pub const PLACE_SHARE: f32 = 0.6;
/// … and the contour over which the force settles into the stroke after the acceleration, when
/// the attack's force is heavier than the stroke's (the bow set in, then easing): its time
/// constant (s) at pp and at ff (a soft stroke takes longer). A light start rises to the
/// stroke at the classic rate: slowed, the bass's E2 at velocity 0.35 took 288 ms to settle
/// (Guettler's limit there 214). At (0.2, 0.08) the bowed pitch moved with the force for
/// longer than its tables (measured around the classic attack) hold — a bass point 1.09 cents
/// held out, the violin's top at ff — and the violin's G3 at 0.55 took 124 ms: a longer contour
/// needs the pitch tables and the attack region measured again with it (`arcus-expr.md`,
/// *Next*). (0.12, 0.05) keeps every gate: a mild contour (τ 0.078 at mf against 0.06).
pub const SETTLE_F: (f32, f32) = (0.12, 0.05);
pub const SETTLE_V: (f32, f32) = (0.07, 0.03);
/// From the string, the acceleration takes the bow to this share of the stroke's speed, the
/// contour doing the rest. Measured: at 0.6 the soft attacks on open strings (the viola's D
/// and A, the violin's A at velocity 0.15–0.35) never settled within Guettler's limits, so the
/// acceleration is the classic one, to the stroke's speed, and the contour is the force's.
pub const STRING_V0: f32 = 1.0;
/// From the air: the bow's speed on landing (a share of the stroke's), the force's arrival (s)
/// and its overshoot over the stroke's — riding the speed while the bow accelerates, as
/// Schelleng's limit does (at the stroke's force from the landing speed the violin's attacks
/// stayed raucous 150–415 ms, worse on the cello: not kept — the landing is the stroke's force,
/// and a start from the air is a note's own choice or a staccato's, never the default's).
pub const LAND_SPEED: f32 = 0.45;
pub const LAND_S: f32 = 0.005;
pub const LAND_FORCE: f32 = 1.12;

/// The start the player chooses when the note names none: from the air for a staccato, from
/// the string otherwise. (Measured: from the air at every velocity of 0.8 or more, the
/// violin's and the viola's attacks at 0.95 settled in 150–415 ms against Guettler and
/// Askenfelt's 90; a hard attack from the string keeps every gate.)
pub fn auto_start(velocity: f32, staccato: bool) -> Start {
    let _ = velocity;
    if staccato {
        Start::Air
    } else {
        Start::String
    }
}

/// The settle's time constants (force, speed), s, at the dynamic `d`.
pub fn settle(d: f32) -> (f32, f32) {
    let d = d.clamp(0.0, 1.0);
    (
        SETTLE_F.0 + (SETTLE_F.1 - SETTLE_F.0) * d,
        SETTLE_V.0 + (SETTLE_V.1 - SETTLE_V.0) * d,
    )
}

/// The fingering of a note: its string, whether that string is open, and (on a slur from
/// `from`, the key and string the finger was on) whether it is a shift.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Fingering {
    pub string: usize,
    pub open: bool,
    pub shift: bool,
}

/// How far up a string the engine plays a note (semitones over its open string): a seventh
/// position's reach.
pub const REACH: f32 = 12.0;

/// The fingering of `key` (semitones, tuned) on `spec`: `named` the string the note asks for
/// (0 the lowest; honoured when it reaches the key), `expressive` whether the note is one a
/// player would vibrate (not a fast passage's), `from` the slur's last key and string.
pub fn finger(
    spec: &InstrumentSpec,
    key: f32,
    named: Option<u8>,
    expressive: bool,
    from: Option<(f32, usize)>,
) -> Fingering {
    let first = spec.string_for(key);
    let open_of = |s: usize| spec.strings[s].open as f32;
    let reaches = |s: usize| key + 1e-3 >= open_of(s) && key <= open_of(s) + REACH + 1e-3;
    let mut s = first;
    // A string the note names (sul G) wins where it reaches. Otherwise first position: the
    // bowed-pitch tables are measured per key there, and a note moved to the string below
    // (an open string's pitch stopped by the fourth finger, a slur kept on its string) sat
    // 1.4 cents off its pitch on the slur gate — choosing the string by itself waits for the
    // tables per string (`arcus-expr.md`, *Next*: the survey's item 2). `expressive` and the
    // slur's string are the hooks it will use.
    let _ = expressive;
    if let Some(n) = named.map(|n| (n as usize).min(spec.strings.len() - 1)) {
        if reaches(n) {
            s = n;
        }
    } else if let Some((_, fs)) = from {
        let _ = fs;
    }
    let open = (key - open_of(s)).abs() < 0.05;
    let shift = from.is_some_and(|(k, fs)| fs == s && (key - k).abs() >= 3.0 - 1e-3 && !open);
    Fingering {
        string: s,
        open,
        shift,
    }
}

/// A shift's finger time over a plain change's, ms per semitone past two (the hand travels:
/// a step 20–35 ms, a shift of a fifth 60–100; the survey's item 10 refines the curve), and
/// its cap.
pub const SHIFT_MS_PER_ST: f32 = 7.0;
pub const SHIFT_MS_MAX: f32 = 110.0;

/// The vibrato's factor for a note: none on an open string in a passage (a held note at an
/// open string's pitch is one a player stops with the fourth finger: it keeps its vibrato until
/// the fingering plays it there), little in a fast détaché, less in a fast slur (it goes on
/// through the line).
pub fn vib_scale(open: bool, fast: bool, slurred: bool) -> f32 {
    if open && fast {
        0.0
    } else if fast && !slurred {
        0.3
    } else if fast {
        0.7
    } else {
        1.0
    }
}
