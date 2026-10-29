//! Notes in, the logical score out: bars and their metres, tempo marks, and each part's
//! staves (see `staff.rs` for one staff's work).

use crate::hands;
use crate::metre::Metre;
use crate::model::{Bar, Dur, Dynamic, Kind, Part, Score, TempoMark};
use crate::read::{self, BarSpan, ReadNote, SourceNote};
use crate::spell::Speller;
use crate::staff::{self, StaffPlan};
use cantus_core::notation::{ScoreSettings, StaffChoice};
use cantus_core::scale::Key;
use cantus_core::tempo::TempoMap;
use cantus_core::time::Ticks;

/// The song a score is made from.
pub struct Song<'a> {
    pub title: String,
    pub composer: String,
    pub tempo: &'a TempoMap,
    pub key: Option<Key>,
}

/// One part's notes and settings.
#[derive(Clone, Debug)]
pub struct PartSource {
    pub name: String,
    pub notes: Vec<SourceNote>,
    pub settings: ScoreSettings,
}

/// The bars from `from`'s bar to the one holding `to - 1` (at least one).
pub fn bars_between(tempo: &TempoMap, from: i64, to: i64) -> Vec<BarSpan> {
    let mut out = Vec::new();
    let mut t = tempo.bar_start(Ticks(from)).0;
    let to = to.max(t + 1);
    while t < to && out.len() < 100_000 {
        let end = tempo.bar_end(Ticks(t)).0;
        let sig = tempo.signature_at(Ticks(t));
        out.push(BarSpan {
            start: t,
            metre: Metre::new(sig, end - t),
        });
        t = end;
    }
    out
}

/// The tempo marks: the first bar's, and one where the tempo changes (rounded as written),
/// with *accel.* / *rit.* where a ramp starts.
fn tempo_marks(tempo: &TempoMap, bars: &mut [Bar]) {
    let mut last_written: Option<f64> = None;
    let points = tempo.points();
    let first = bars.first().map(|b| b.start).unwrap_or(0);
    let end = bars.last().map(|b| b.start + b.len).unwrap_or(0);
    let mark = |bar: &Bar, quarter_bpm: f64, ramp: Option<bool>| {
        let compound = Metre::new(bar.sig, bar.len).compound;
        let (unit, per_minute) = if compound {
            (Dur::new(2, 1), (quarter_bpm / 1.5 * 10.0).round() / 10.0)
        } else {
            (Dur::new(2, 0), (quarter_bpm * 10.0).round() / 10.0)
        };
        TempoMark {
            unit,
            per_minute,
            quarter_bpm,
            ramp,
        }
    };
    // What holds at the first bar, and whether a ramp is under way there.
    let ramp_at = |i: usize| -> Option<bool> {
        let p = points.get(i)?;
        if !p.ramp {
            return None;
        }
        points.get(i + 1).map(|n| n.bpm > p.bpm)
    };
    let at_first = points.iter().rposition(|p| p.tick.0 <= first);
    if let Some(b) = bars.first_mut() {
        let bpm = tempo.bpm_at(first as f64);
        let m = mark(b, bpm, at_first.and_then(ramp_at));
        last_written = Some(m.per_minute);
        b.tempo = Some(m);
    }
    for (i, p) in points.iter().enumerate() {
        if p.tick.0 <= first || p.tick.0 >= end {
            continue;
        }
        let Some(bi) = bars.iter().rposition(|b| b.start <= p.tick.0) else {
            continue;
        };
        let m = mark(&bars[bi], p.bpm, ramp_at(i));
        if last_written == Some(m.per_minute) && m.ramp.is_none() {
            continue;
        }
        last_written = Some(m.per_minute);
        bars[bi].tempo = Some(m);
    }
}

/// The logical score of `parts` over `[from, to)` (widened to whole bars), or over the
/// parts' notes when no range is given.
pub fn build(song: &Song, parts: &[PartSource], range: Option<(i64, i64)>) -> Score {
    let (from, to) = range.unwrap_or_else(|| {
        let lo = parts
            .iter()
            .flat_map(|p| p.notes.iter().map(|n| n.start))
            .min()
            .unwrap_or(0)
            .max(0);
        let hi = parts
            .iter()
            .flat_map(|p| p.notes.iter().map(|n| n.start + n.len.max(1)))
            .max()
            .unwrap_or(1);
        (song.tempo.bar_start(Ticks(lo)).0, hi)
    });
    let spans = bars_between(song.tempo, from, to);
    let mut bars: Vec<Bar> = spans
        .iter()
        .enumerate()
        .map(|(i, s)| Bar {
            start: s.start,
            len: s.metre.len,
            sig: s.metre.sig,
            show_sig: i == 0 || spans[i - 1].metre.sig != s.metre.sig,
            tempo: None,
            number: song.tempo.bbt(Ticks(s.start)).0.max(1) as u32,
        })
        .collect();
    tempo_marks(song.tempo, &mut bars);
    let key = crate::spell::key_signature(song.key);
    let start = spans.first().map(|s| s.start).unwrap_or(0);
    let end = spans.last().map(|s| s.start + s.metre.len).unwrap_or(0);
    let parts = parts
        .iter()
        .map(|p| build_part(song, p, &spans, start, end))
        .collect();
    Score {
        title: song.title.clone(),
        composer: song.composer.clone(),
        key,
        bars,
        parts,
    }
}

fn build_part(song: &Song, src: &PartSource, spans: &[BarSpan], start: i64, end: i64) -> Part {
    let s = &src.settings;
    let notes: Vec<SourceNote> = src
        .notes
        .iter()
        .filter(|n| n.start >= start && n.start < end && n.len > 0)
        .map(|n| SourceNote {
            len: n.len.min(end - n.start),
            ..*n
        })
        .collect();
    let (read, windows) = read::read(&notes, spans, s.grid);
    let grand = match s.staves {
        StaffChoice::Grand => true,
        StaffChoice::Single => false,
        StaffChoice::Auto => hands::wants_grand(&read),
    };
    let (speller, written_key) = Speller::transposed(song.key, s.transpose as i32);
    let mut plans: Vec<(StaffPlan, Vec<ReadNote>)> = Vec::new();
    if grand {
        let which = hands::split(&read, s.split);
        let upper: Vec<ReadNote> = read
            .iter()
            .zip(&which)
            .filter(|(_, w)| **w == 0)
            .map(|(n, _)| *n)
            .collect();
        let lower: Vec<ReadNote> = read
            .iter()
            .zip(&which)
            .filter(|(_, w)| **w == 1)
            .map(|(n, _)| *n)
            .collect();
        plans.push((StaffPlan::upper(), upper));
        plans.push((StaffPlan::lower(), lower));
    } else {
        plans.push((StaffPlan::single(s.clef), read));
    }
    let staves = plans
        .into_iter()
        .map(|(plan, notes)| {
            staff::build_staff(&plan, &notes, spans, &windows, &speller, s.transpose)
        })
        .collect();
    let mut part = Part {
        name: s.name.clone().unwrap_or_else(|| src.name.clone()),
        staves,
        transpose: s.transpose,
        key: written_key,
    };
    if s.dynamics {
        place_dynamics(&mut part, spans);
    }
    part
}

/// Dynamics from velocity: the first attack's, then a new marking where the level moves
/// two steps, or one step that the next attack confirms.
fn place_dynamics(part: &mut Part, spans: &[BarSpan]) {
    // (absolute time, velocity, staff, bar, voice, event)
    let mut onsets: Vec<(i64, u8, usize, usize, usize, usize)> = Vec::new();
    for (si, st) in part.staves.iter().enumerate() {
        for (bi, bar) in st.bars.iter().enumerate() {
            for (vi, v) in bar.voices.iter().enumerate() {
                for (ei, e) in v.events.iter().enumerate() {
                    if let Kind::Chord(h) = &e.kind {
                        let fresh: Vec<u8> = h
                            .iter()
                            .filter(|h| !h.tied_from)
                            .map(|h| h.velocity)
                            .collect();
                        if let Some(&vel) = fresh.iter().max() {
                            onsets.push((spans[bi].start + e.start, vel, si, bi, vi, ei));
                        }
                    }
                }
            }
        }
    }
    onsets.sort_by_key(|o| (o.0, o.2, o.4));
    let mut merged: Vec<(i64, u8, usize, usize, usize, usize)> = Vec::with_capacity(onsets.len());
    for o in onsets {
        match merged.last_mut() {
            Some(m) if m.0 == o.0 => m.1 = m.1.max(o.1),
            _ => merged.push(o),
        }
    }
    let levels: Vec<Dynamic> = merged.iter().map(|o| Dynamic::of_velocity(o.1)).collect();
    let mut current: Option<Dynamic> = None;
    for (i, &l) in levels.iter().enumerate() {
        let write = match current {
            None => true,
            Some(c) if c == l => false,
            Some(c) => {
                let step = (l as i32 - c as i32).abs();
                step >= 2
                    || levels.get(i + 1).is_some_and(|&n| {
                        n != c && (n as i32 - c as i32).signum() == (l as i32 - c as i32).signum()
                    })
            }
        };
        if write {
            current = Some(l);
            let (_, _, si, bi, vi, ei) = merged[i];
            part.staves[si].bars[bi].voices[vi].events[ei].dynamic = Some(l);
        }
    }
}
