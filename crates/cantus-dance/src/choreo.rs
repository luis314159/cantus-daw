//! Which move plays, from when, and what it took over from (ADR 0087 §2): the
//! choreography's first half, a **pure function** of the engine's frame.
//!
//! **The rules**, in order:
//! 1. **A note** in the move map plays its move from the note's start. Held, it goes on;
//!    let go, it finishes the cycle it is in (a tap plays one whole cycle). A note
//!    cannot start before the song position (after a loop wrap it has not happened yet).
//! 2. **A key still held** in the map, once the latest note is over, plays its move from
//!    the moment it took over (the lowest such key: legato playing falls back to it).
//! 3. **`move`** (automation, a modulator, MIDI learn, the knob) ≥ 1 plays that move, its
//!    phase counted from the bar line — so a one-beat move steps on every beat.
//! 4. **`move` = 0** dances the character's routine: one move per bar, repeating.
//!
//! **Blends** come only where the start of the current move is known — a bar line, a
//! note, the end of a note — and blend from what played just before it (the previous
//! bar's move, the note this one took over from), over [`BLEND_BEATS`], that move's phase
//! still running. Moves are resolved to what the character has (its own or the fallback).

use crate::character::Character;
use cantus_core::inst::dancer::{self, Move};
use cantus_engine::dance::{DanceFrame, DanceNote};

/// How long a change of move blends, in quarter notes.
pub const BLEND_BEATS: f64 = 0.25;

/// The move a character plays for `m`: its own, or the first fallback it has.
pub fn resolve(ch: &dyn Character, m: Move) -> Move {
    let mut m = m;
    for _ in 0..4 {
        if ch.has_move(m) {
            return m;
        }
        m = m.fallback();
    }
    Move::Idle
}

/// One move's place in time: the move, where its phase counts from, and its pace.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Span {
    pub mv: Move,
    /// Quarter notes from the song start where its phase is 0.
    pub origin: f64,
    /// Quarter notes one cycle lasts (`move_beats` ÷ speed).
    pub cycle: f64,
}

impl Span {
    fn new(ch: &dyn Character, m: Move, origin: f64, speed: f32) -> Span {
        let mv = resolve(ch, m);
        Span {
            mv,
            origin,
            cycle: cycle_beats(ch, mv, speed),
        }
    }

    /// Cycles since its origin at `beats` (never negative).
    pub fn cycles_at(&self, beats: f64) -> f64 {
        ((beats - self.origin) / self.cycle).max(0.0)
    }

    /// Its phase (0…1) and whole cycles at `beats`.
    pub fn phase_at(&self, beats: f64) -> (f32, u32) {
        let c = self.cycles_at(beats);
        (c.fract() as f32, c.floor().min(f64::from(u32::MAX)) as u32)
    }
}

/// A move's cycle in quarter notes at a speed (never zero).
pub fn cycle_beats(ch: &dyn Character, mv: Move, speed: f32) -> f64 {
    (f64::from(ch.move_beats(mv).max(0.0625)) / f64::from(speed.max(0.0625))).max(1e-3)
}

/// What plays now, and what it took over from while the change blends.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Choice {
    pub now: Span,
    /// When it took over (a bar line, a note's start, a note's end).
    pub entered: f64,
    pub from: Option<Span>,
    /// The velocity of the note playing it (1 for `move` and the routine).
    pub velocity: f32,
    pub by_note: bool,
}

/// The song's grid as the frame gives it: the bar it is in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Grid {
    pub bar: i32,
    pub bar_start: f64,
    pub bar_beats: f64,
}

impl Grid {
    pub fn of(frame: &DanceFrame) -> Grid {
        Grid {
            bar: frame.bar,
            bar_start: frame.bar_start_beats,
            bar_beats: f64::from(frame.bar_beats).max(0.25),
        }
    }

    /// The bar `beats` is in and where it starts, counted back from this one at its metre
    /// (a time before the frame's bar is never more than a note's length ago).
    pub fn bar_at(&self, beats: f64) -> (i32, f64) {
        if beats >= self.bar_start {
            let n = ((beats - self.bar_start) / self.bar_beats).floor();
            (self.bar + n as i32, self.bar_start + n * self.bar_beats)
        } else {
            let n = ((self.bar_start - beats) / self.bar_beats).ceil();
            (self.bar - n as i32, self.bar_start - n * self.bar_beats)
        }
    }
}

/// The move without notes at a bar: `move` if it chooses one, else the routine's.
fn bar_move(ch: &dyn Character, chosen: Option<Move>, bar: i32) -> Move {
    chosen.unwrap_or_else(|| {
        let r = ch.routine();
        if r.is_empty() {
            Move::Bounce
        } else {
            r[bar.rem_euclid(r.len() as i32) as usize]
        }
    })
}

/// What `move` chose at `beats`: the frame's value, or before its last change the value it
/// left (older changes are not known: the value they led to stands for them).
fn chosen_at(frame: &DanceFrame, beats: f64) -> Option<Move> {
    let v = match frame.move_change {
        Some((at, before)) if beats < at => before,
        _ => frame.value("move").unwrap_or(0.0),
    };
    dancer::move_of(v)
}

/// The span the bar plays without notes at `beats`.
fn base_span(frame: &DanceFrame, ch: &dyn Character, grid: &Grid, beats: f64, speed: f32) -> Span {
    let (bar, start) = grid.bar_at(beats);
    Span::new(ch, bar_move(ch, chosen_at(frame, beats), bar), start, speed)
}

/// Where a note's move ends: held, never (`None`); let go, at the end of the cycle it was
/// let go in (one whole cycle at least).
pub fn note_end(n: &DanceNote, cycle: f64) -> Option<f64> {
    n.released.map(|r| {
        let cycles = ((r - n.beats) / cycle - 1e-9).ceil().max(1.0);
        n.beats + cycles * cycle
    })
}

fn note_span(ch: &dyn Character, n: &DanceNote, speed: f32) -> Option<Span> {
    Move::from_key(n.key).map(|m| Span::new(ch, m, n.beats, speed))
}

/// Whether a note's move is playing at `beats`.
fn plays_at(n: &DanceNote, span: &Span, beats: f64) -> bool {
    beats >= n.beats && note_end(n, span.cycle).is_none_or(|e| beats < e)
}

/// The lowest key of the move map held now, as a move.
pub fn held_move(frame: &DanceFrame) -> Option<Move> {
    (0..Move::ALL.len() as u8)
        .map(|i| dancer::BASE_KEY + i)
        .find(|k| frame.is_held(*k))
        .and_then(Move::from_key)
}

/// Whether `x` is a whole number of cycles (to rounding).
fn whole(x: f64) -> bool {
    let f = x.fract();
    !(1e-6..=1.0 - 1e-6).contains(&f)
}

/// What plays at the frame's position (playing), and what it took over from.
pub fn choose(frame: &DanceFrame, ch: &dyn Character, speed: f32) -> Choice {
    let beats = frame.beats;
    let grid = Grid::of(frame);
    let base_now = base_span(frame, ch, &grid, beats, speed);
    // What played before the latest note: the note before it, if it still played just
    // before (a note taken over is let go on the very sample the new one starts).
    let before_note = |n: &DanceNote| -> Span {
        frame
            .prev_note
            .and_then(|p| note_span(ch, &p, speed).filter(|s| plays_at(&p, s, n.beats - 1e-6)))
            .unwrap_or_else(|| base_span(frame, ch, &grid, n.beats, speed))
    };
    let mut after_note: Option<(Span, f64)> = None;
    if let Some(n) = frame.note {
        if let Some(span) = note_span(ch, &n, speed) {
            if plays_at(&n, &span, beats) {
                return Choice {
                    now: span,
                    entered: n.beats,
                    from: Some(before_note(&n)),
                    velocity: n.velocity,
                    by_note: true,
                };
            }
            // Over: what it hands back to starts where it ended.
            after_note = note_end(&n, span.cycle)
                .filter(|e| *e <= beats && *e >= n.beats)
                .map(|e| (span, e));
        }
    }
    if let (Some(m), Some((ended, at))) = (held_move(frame), after_note) {
        return Choice {
            now: Span::new(ch, m, at, speed),
            entered: at,
            from: Some(ended),
            velocity: frame.note.map_or(1.0, |n| n.velocity),
            by_note: true,
        };
    }
    // The bar's move: it entered at the bar line, or later in this bar where `move`
    // changed or the last note ended — the latest of them, blending from what left.
    let line = base_span(frame, ch, &grid, base_now.origin - 1e-6, speed);
    let seamless = line.mv == base_now.mv && whole(line.cycles_at(base_now.origin));
    let mut choice = Choice {
        now: base_now,
        entered: base_now.origin,
        from: (!seamless).then_some(line),
        velocity: 1.0,
        by_note: false,
    };
    if let Some((at, _)) = frame.move_change {
        if at > choice.entered && at <= beats {
            let left = base_span(frame, ch, &grid, at - 1e-6, speed);
            choice.entered = at;
            choice.from = (left.mv != base_now.mv).then_some(left);
        }
    }
    // A note that ended on the bar line was what showed before it.
    if let Some((ended, at)) = after_note {
        if at >= choice.entered {
            choice.entered = at;
            choice.from = Some(ended);
        }
    }
    choice
}
