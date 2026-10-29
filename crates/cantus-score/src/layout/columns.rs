//! Horizontal spacing: each bar's columns (every onset of every staff, so the parts line
//! up), how far each reaches left and right, and the space after it — by duration, a
//! doubling worth √2 more space (Dorico's default ratio; Gould's tables are close), never
//! less than what keeps neighbours from touching.

use super::chord::{self, Shape};
use crate::model::{Kind, Score};

/// A staff of the score, flattened: which part, which of its staves.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StaffRef {
    pub part: usize,
    pub staff: usize,
}

/// One event in a column.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The flattened staff.
    pub staff: usize,
    pub voice: usize,
    pub event: usize,
    pub shape: Shape,
    /// Moved right when two voices would collide.
    pub dx: f32,
}

/// One onset of a bar.
#[derive(Clone, Debug)]
pub struct Column {
    /// Bar-relative tick.
    pub t: i64,
    pub entries: Vec<Entry>,
    pub left: f32,
    pub right: f32,
    /// The space after it by duration, and the least that clears its neighbour.
    pub ideal: f32,
    pub min: f32,
}

/// A bar's columns and fixed widths.
#[derive(Clone, Debug)]
pub struct BarCols {
    pub columns: Vec<Column>,
    /// From the bar line to the first column.
    pub lead: f32,
    /// Staves (and voices) holding a whole-bar rest.
    pub whole_rests: Vec<(usize, usize)>,
}

impl BarCols {
    /// Its width at its ideal spacing, and at its tightest.
    pub fn widths(&self) -> (f32, f32) {
        if self.columns.is_empty() {
            return (WHOLE_REST_BAR, WHOLE_REST_BAR * 0.7);
        }
        let ideal: f32 = self.columns.iter().map(|c| c.ideal.max(c.min)).sum();
        let min: f32 = self.columns.iter().map(|c| c.min).sum();
        (self.lead + ideal, self.lead + min)
    }

    /// The column spaces at stretch `s` (each its ideal × s, never under its minimum).
    pub fn spaces(&self, s: f32) -> Vec<f32> {
        self.columns
            .iter()
            .map(|c| (c.ideal * s).max(c.min))
            .collect()
    }
}

/// The width of a bar holding only whole-bar rests.
pub const WHOLE_REST_BAR: f32 = 8.0;
/// Between a bar line and the first note's leftmost ink.
pub const AFTER_BARLINE: f32 = 1.2;
/// Between neighbours' inks.
const PAD: f32 = 0.35;
/// Between the last ink and the bar line.
const BEFORE_BARLINE: f32 = 0.9;
/// A quarter note's space.
pub const QUARTER_SPACE: f32 = 3.8;

/// The space a duration of `dt` ticks takes.
pub fn space(dt: i64) -> f32 {
    let q = cantus_core::time::PPQ as f32;
    QUARTER_SPACE * (dt.max(60) as f32 / q).sqrt()
}

/// Every bar's columns.
pub fn measure(score: &Score, staves: &[StaffRef]) -> Vec<BarCols> {
    let mut out = Vec::with_capacity(score.bars.len());
    for (bi, bar) in score.bars.iter().enumerate() {
        let mut cols: Vec<Column> = Vec::new();
        let mut whole_rests = Vec::new();
        for (si, r) in staves.iter().enumerate() {
            let sb = &score.parts[r.part].staves[r.staff].bars[bi];
            let two = sb.voices.len() > 1;
            for (vi, v) in sb.voices.iter().enumerate() {
                for (ei, e) in v.events.iter().enumerate() {
                    if matches!(e.kind, Kind::Rest { whole_bar: true }) {
                        whole_rests.push((si, vi));
                        continue;
                    }
                    let shape = chord::shape(e, vi, two);
                    let entry = Entry {
                        staff: si,
                        voice: vi,
                        event: ei,
                        shape,
                        dx: 0.0,
                    };
                    match cols.iter_mut().find(|c| c.t == e.start) {
                        Some(c) => c.entries.push(entry),
                        None => cols.push(Column {
                            t: e.start,
                            entries: vec![entry],
                            left: 0.0,
                            right: 0.0,
                            ideal: 0.0,
                            min: 0.0,
                        }),
                    }
                }
            }
        }
        cols.sort_by_key(|c| c.t);
        for c in cols.iter_mut() {
            offset_voices(c, score, staves, bi);
            c.left = c
                .entries
                .iter()
                .map(|e| e.shape.left + e.dx)
                .fold(0.0, f32::min);
            c.right = c
                .entries
                .iter()
                .map(|e| e.shape.right + e.dx)
                .fold(0.0, f32::max);
        }
        let n = cols.len();
        for i in 0..n {
            let next_t = if i + 1 < n { cols[i + 1].t } else { bar.len };
            cols[i].ideal = space(next_t - cols[i].t);
            cols[i].min = if i + 1 < n {
                cols[i].right + PAD - cols[i + 1].left
            } else {
                cols[i].right + BEFORE_BARLINE
            };
        }
        let lead = AFTER_BARLINE - cols.first().map_or(0.0, |c| c.left);
        out.push(BarCols {
            columns: cols,
            lead,
            whole_rests,
        });
    }
    out
}

/// Two voices of one staff at one column whose heads would touch: the second voice's
/// chord moves right by a notehead (Gould: at a second or unison the stem-down voice
/// stands to the right).
fn offset_voices(c: &mut Column, score: &Score, staves: &[StaffRef], bar: usize) {
    let idx: Vec<usize> = (0..c.entries.len()).collect();
    for &i in &idx {
        if c.entries[i].voice != 1 {
            continue;
        }
        let st = c.entries[i].staff;
        let Some(j) = idx
            .iter()
            .copied()
            .find(|&j| c.entries[j].staff == st && c.entries[j].voice == 0)
        else {
            continue;
        };
        let r = staves[st];
        let voices = &score.parts[r.part].staves[r.staff].bars[bar].voices;
        let up = &voices[0].events[c.entries[j].event];
        let down = &voices[1].events[c.entries[i].event];
        let (Kind::Chord(a), Kind::Chord(b)) = (&up.kind, &down.kind) else {
            continue;
        };
        let up_low = a.iter().map(|h| h.pos).min().unwrap_or(0);
        let down_high = b.iter().map(|h| h.pos).max().unwrap_or(0);
        if up_low - down_high <= 1 {
            let same = up_low == down_high
                && up.dur.log == down.dur.log
                && up.dur.dots == down.dur.dots
                && a.len() == 1
                && b.len() == 1;
            if !same {
                c.entries[i].dx = c.entries[j].shape.right.min(1.3) - 0.05;
            }
        }
    }
}
