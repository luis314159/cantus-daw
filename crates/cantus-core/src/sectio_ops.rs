//! Sectio's three clip gestures (round 37, ADR 0099), after FL Studio's Slicex and Fruity
//! Slicer: **Random** (the slices of a dumped clip shuffled, positions kept), **Stutter** (some
//! slices retriggered in quick repeats) and **Quantize** (onto a grid, with swing). Each is a
//! pure function from the clip's notes to the new ones — seeded, never random, so a
//! gesture's result is the same every time — and the editor sends it as one undo step.

use crate::model::Note;
use crate::time::Ticks;

/// A small deterministic generator (SplitMix64).
#[derive(Clone, Copy, Debug)]
pub struct Seeded(pub u64);

impl Seeded {
    pub fn draw(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// 0 … n − 1.
    pub fn below(&mut self, n: usize) -> usize {
        (self.draw() % n.max(1) as u64) as usize
    }
    /// 0 … 1.
    pub fn unit(&mut self) -> f64 {
        (self.draw() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Random: the notes keep their times, lengths and velocities; their pitches (the slices
/// they play) are shuffled among themselves.
pub fn random(notes: &[Note], seed: u64) -> Vec<Note> {
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by_key(|&i| (notes[i].start, notes[i].pitch));
    let mut pitches: Vec<u8> = order.iter().map(|&i| notes[i].pitch).collect();
    let mut rng = Seeded(seed);
    for i in (1..pitches.len()).rev() {
        let j = rng.below(i + 1);
        pitches.swap(i, j);
    }
    let mut out = notes.to_vec();
    for (k, &i) in order.iter().enumerate() {
        out[i].pitch = pitches[k];
    }
    out
}

/// Stutter: each note, with probability `amount` (0 … 1, seeded), becomes `repeats` quick
/// retriggers of its slice across its length (each `len / repeats`, the last taking the
/// rest). Returns the notes kept as they were, and the notes that replace the others —
/// new notes need new ids, which `id` gives.
pub fn stutter(
    notes: &[Note],
    amount: f64,
    repeats: u32,
    seed: u64,
    id: &mut dyn FnMut() -> crate::model::NoteId,
) -> (Vec<Note>, Vec<Note>) {
    let repeats = repeats.clamp(2, 16) as i64;
    let mut rng = Seeded(seed);
    let mut kept = Vec::new();
    let mut made = Vec::new();
    for n in notes {
        let hit = rng.unit() < amount.clamp(0.0, 1.0);
        let step = n.length.0 / repeats;
        if !hit || step < 1 {
            kept.push(n.clone());
            continue;
        }
        for r in 0..repeats {
            let start = n.start.0 + r * step;
            let len = if r == repeats - 1 {
                n.length.0 - r * step
            } else {
                step
            };
            let mut m = n.clone();
            m.id = id();
            m.start = Ticks(start);
            m.length = Ticks(len);
            // Each repeat a little softer, as a hand on a pad.
            m.velocity = (n.velocity as i32 - 6 * r as i32).clamp(1, 127) as u8;
            made.push(m);
        }
    }
    (kept, made)
}

/// Quantize: every note's start onto the nearest step of `grid` ticks, the odd steps
/// pushed later by `swing` (0 … 1 of half a step, as FL's swing), its end kept where it
/// was unless that would make it shorter than a tick.
pub fn quantize(notes: &[Note], grid: i64, swing: f64) -> Vec<Note> {
    let grid = grid.max(1);
    notes
        .iter()
        .map(|n| {
            let step = ((n.start.0 as f64) / grid as f64).round() as i64;
            let mut start = step * grid;
            if step % 2 == 1 {
                start += (swing.clamp(0.0, 1.0) * grid as f64 * 0.5).round() as i64;
            }
            let end = n.start.0 + n.length.0;
            let mut m = n.clone();
            m.start = Ticks(start.max(0));
            m.length = Ticks((end - m.start.0).max(1));
            m
        })
        .collect()
}
