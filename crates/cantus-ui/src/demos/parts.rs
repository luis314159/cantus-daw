//! The parts a band plays, as generators over a chord chart: sustained pads, rhythmic
//! comping, a bass line that follows the bass of each chord and walks into the next,
//! arpeggios, and drum bars from text grids.
//!
//! A chart is a list of `(symbol, beats)`; every generator works out which chord sounds at
//! each moment and voices it from the chord before, so all the parts of a song agree on
//! the harmony and none of them jumps.

use super::theory::{chord, lead, shift, Chord, Ev};

/// A chord chart: symbols and how many beats each lasts.
pub type Chart<'a> = &'a [(&'a str, f64)];

/// The chart laid out in time: `(start, length, chord, voicing)`.
pub struct Laid {
    pub items: Vec<(f64, f64, Chord, Vec<u8>)>,
    pub beats: f64,
}

/// Lay a chart out, voicing it with `voices` notes between `lo` and `hi`, led from chord
/// to chord around `centre`.
pub fn lay(chart: Chart, voices: usize, lo: u8, hi: u8, centre: u8) -> Laid {
    let mut at = 0.0;
    let mut prev: Vec<u8> = Vec::new();
    let mut items = Vec::new();
    for (sym, len) in chart {
        let c = chord(sym);
        let v = lead(&prev, &c.upper(voices), lo, hi, 14, centre);
        prev = v.clone();
        items.push((at, *len, c, v));
        at += len;
    }
    Laid { items, beats: at }
}

impl Laid {
    /// The item sounding at `beat` (the chart repeats past its end).
    pub fn at(&self, beat: f64) -> &(f64, f64, Chord, Vec<u8>) {
        let b = beat.rem_euclid(self.beats.max(1e-9));
        self.items
            .iter()
            .rev()
            .find(|i| i.0 <= b + 1e-9)
            .unwrap_or(&self.items[0])
    }

    /// The item after the one sounding at `beat`.
    pub fn next(&self, beat: f64) -> &(f64, f64, Chord, Vec<u8>) {
        let cur = self.at(beat);
        let i = self.items.iter().position(|x| x.0 == cur.0).unwrap_or(0);
        &self.items[(i + 1) % self.items.len()]
    }
}

/// Every chord held for its whole length.
pub fn hold(laid: &Laid, vel: u8, gate: f64) -> Vec<Ev> {
    let mut out = Vec::new();
    for (at, len, _, v) in &laid.items {
        for p in v {
            out.push(Ev::new(*at, len * gate, *p, vel));
        }
    }
    out
}

/// Chords struck on a rhythm that repeats every `cycle` beats: `(at, length, velocity)`.
/// A hit takes the voicing of the chord sounding at its moment.
pub fn comp(laid: &Laid, cycle: f64, rhythm: &[(f64, f64, u8)]) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut base = 0.0;
    while base < laid.beats - 1e-9 {
        for (at, len, vel) in rhythm {
            let t = base + at;
            if t >= laid.beats - 1e-9 {
                continue;
            }
            for p in &laid.at(t).3 {
                out.push(Ev::new(t, *len, *p, *vel));
            }
        }
        base += cycle;
    }
    out
}

/// What a bass note is, relative to the chord sounding.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum B {
    /// The chord's bass note.
    R,
    /// An octave above it.
    O,
    /// The fifth above the root.
    F,
    /// A step into the next chord's bass (a semitone below it when it is a fourth or
    /// more away, a whole step above otherwise).
    A,
    /// A number of semitones from the bass.
    S(i8),
}

/// A bass line: `(at, length, velocity, what)` repeating every `cycle` beats, its notes
/// in the octave from `low`.
pub fn bass(laid: &Laid, low: u8, cycle: f64, rhythm: &[(f64, f64, u8, B)]) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut base = 0.0;
    while base < laid.beats - 1e-9 {
        for (at, len, vel, what) in rhythm {
            let t = base + at;
            if t >= laid.beats - 1e-9 {
                continue;
            }
            let (_, _, c, _) = laid.at(t);
            let r = c.bass_at(low) as i32;
            let p = match what {
                B::R => r,
                B::O => r + 12,
                B::F => {
                    let f = r + ((c.root as i32 + 7 - c.bass as i32).rem_euclid(12));
                    if f > low as i32 + 14 {
                        f - 12
                    } else {
                        f
                    }
                }
                B::A => {
                    let n = laid.next(t).2.bass_at(low) as i32;
                    if n == r {
                        r + 7 - 12 * ((r + 7 > low as i32 + 14) as i32)
                    } else if n > r {
                        n - 1
                    } else {
                        n + 2
                    }
                }
                B::S(s) => r + *s as i32,
            };
            out.push(Ev::new(t, *len, p.clamp(0, 127) as u8, *vel));
        }
        base += cycle;
    }
    out
}

/// An arpeggio through each chord's voicing with its bass under it: `order` indexes the
/// notes low to high (bass first), one per `step` beats.
pub fn arp(laid: &Laid, low: u8, order: &[usize], step: f64, len: f64, vel: u8) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut t = 0.0;
    while t < laid.beats - 1e-9 {
        let (_, _, c, v) = laid.at(t);
        let mut notes = vec![c.bass_at(low)];
        notes.extend(v.iter().copied());
        let idx = order[i % order.len()] % notes.len();
        let accent = if i.is_multiple_of(4) { 10 } else { 0 };
        out.push(Ev::new(t, len, notes[idx], vel.saturating_add(accent)));
        i += 1;
        t += step;
    }
    out
}

/// `bars` bars of `beats` each, made one at a time (bar index in, its events out).
pub fn bars(n: usize, beats: f64, mut f: impl FnMut(usize) -> Vec<Ev>) -> Vec<Ev> {
    let mut out = Vec::new();
    for b in 0..n {
        out.extend(shift(&f(b), b as f64 * beats));
    }
    out
}

/// A drum bar from text lines: `(grid, pitch)`, one character per `step` beats.
pub fn drums(lines: &[(&str, u8)], step: f64) -> Vec<Ev> {
    lines
        .iter()
        .flat_map(|(g, p)| super::theory::grid(g, *p, step))
        .collect()
}

/// Repeat `evs` (a pattern `len` beats long) `times` times.
pub fn repeat(evs: &[Ev], len: f64, times: usize) -> Vec<Ev> {
    (0..times)
        .flat_map(|i| shift(evs, i as f64 * len))
        .collect()
}

/// Scale every velocity by `k` (a crescendo across a fill, a softer verse).
pub fn dynamics(evs: &[Ev], k: f64) -> Vec<Ev> {
    evs.iter()
        .map(|e| Ev {
            vel: (e.vel as f64 * k).round().clamp(1.0, 127.0) as u8,
            ..*e
        })
        .collect()
}

/// A crescendo: velocities from `from` to `to` times their value across `beats`.
pub fn swell(evs: &[Ev], beats: f64, from: f64, to: f64) -> Vec<Ev> {
    evs.iter()
        .map(|e| {
            let k = from + (to - from) * (e.at / beats).clamp(0.0, 1.0);
            Ev {
                vel: (e.vel as f64 * k).round().clamp(1.0, 127.0) as u8,
                ..*e
            }
        })
        .collect()
}
