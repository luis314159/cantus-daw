//! A staff's marks once its events are written: octave lines, accidentals, beams, stems
//! and tuplet brackets.

use crate::metre::Metre;
use crate::model::{Accidental, BeamMark, Dur, Event, Kind, Ottava, Staff, Stem, Voice};
use crate::read::{BarSpan, ReadNote};
use crate::spell::Speller;
use crate::voices::Chord;
use std::collections::HashMap;

/// Which chords sit under an octave line: `(voice, chord) → (shift, run)`. A run starts
/// from a chord needing four ledger lines or more (Gould: past three or so an octave line
/// reads better) and takes in the chords around it that still sit on or near the staff an
/// octave nearer; a rest of a bar or more ends it.
pub(crate) fn octave_shifts(
    vs: &[Vec<Chord>; 2],
    pos_of: &dyn Fn(&ReadNote) -> i32,
    spans: &[BarSpan],
) -> HashMap<(usize, usize), (i8, usize)> {
    let bar = spans.first().map(|b| b.metre.len).unwrap_or(3840);
    let mut all: Vec<(i64, i64, usize, usize, i32, i32)> = Vec::new();
    for (vi, v) in vs.iter().enumerate() {
        for (ci, c) in v.iter().enumerate() {
            let lo = c.notes.iter().map(pos_of).min().unwrap_or(0);
            let hi = c.notes.iter().map(pos_of).max().unwrap_or(0);
            all.push((c.start, c.end, vi, ci, lo, hi));
        }
    }
    all.sort_by_key(|a| (a.0, a.2));
    let mut out = HashMap::new();
    let mut run_id = 0usize;
    for shift in [1i8, -1] {
        let eligible = |lo: i32, hi: i32| {
            if shift > 0 {
                lo >= 5 || (hi >= 12 && lo >= 1)
            } else {
                hi <= -5 || (lo <= -12 && hi <= -1)
            }
        };
        let seed = |lo: i32, hi: i32| if shift > 0 { hi >= 12 } else { lo <= -12 };
        let mut i = 0;
        while i < all.len() {
            let mut j = i;
            let mut seeded = false;
            let mut last_end = all[i].0;
            while j < all.len() {
                let (s, e, vi, ci, lo, hi) = all[j];
                if !eligible(lo, hi) || s - last_end >= bar || out.contains_key(&(vi, ci)) {
                    break;
                }
                seeded |= seed(lo, hi);
                last_end = last_end.max(e);
                j += 1;
            }
            if seeded {
                for a in &all[i..j] {
                    out.insert((a.2, a.3), (shift, run_id));
                }
                run_id += 1;
            }
            i = if j > i { j } else { i + 1 };
        }
    }
    out
}

/// An event under an octave line: `(bar, voice, event, start, shift, run)`.
pub(crate) type Under = (usize, usize, usize, i64, i8, usize);

/// The octave lines from the events written under each run: `(bar, voice, event, shift,
/// run)` for every event of a run.
pub(crate) fn ottava_spans(under: &[Under]) -> Vec<Ottava> {
    // Per run: (bar, start, voice, event, shift), sorted into time order below.
    type Placed = (usize, i64, usize, usize, i8);
    let mut runs: HashMap<usize, Vec<Placed>> = HashMap::new();
    for &(bar, voice, event, start, shift, run) in under {
        runs.entry(run)
            .or_default()
            .push((bar, start, voice, event, shift));
    }
    let mut out: Vec<Ottava> = runs
        .into_values()
        .map(|mut v| {
            v.sort();
            let a = v[0];
            let b = v[v.len() - 1];
            Ottava {
                from: (a.0, a.2, a.3),
                to: (b.0, b.2, b.3),
                shift: a.4,
            }
        })
        .collect();
    out.sort_by_key(|o| o.from);
    out
}

/// Accidentals, per bar: one where the alteration differs from what the bar (or the key)
/// has in force on that line; none on a tie's continuation; a courtesy one, in
/// parentheses, on the first note after a bar that altered it and on a note whose letter
/// another octave altered in the same bar.
pub(crate) fn accidentals(staff: &mut Staff, speller: &Speller) {
    let mut prev_altered: HashMap<i32, i8> = HashMap::new();
    for bar in staff.bars.iter_mut() {
        let mut order: Vec<(i64, usize, usize, usize)> = Vec::new();
        for (vi, v) in bar.voices.iter().enumerate() {
            for (ei, e) in v.events.iter().enumerate() {
                for hi in 0..e.heads().len() {
                    order.push((e.start, vi, ei, hi));
                }
            }
        }
        order.sort_unstable();
        let mut state: HashMap<i32, i8> = HashMap::new();
        let mut seen: Vec<i32> = Vec::new();
        let mut letter_alter: HashMap<u8, (i32, i8)> = HashMap::new();
        for (_, vi, ei, hi) in order {
            let Kind::Chord(heads) = &mut bar.voices[vi].events[ei].kind else {
                continue;
            };
            let h = &mut heads[hi];
            if h.tied_from {
                continue;
            }
            let sp = h.spelled;
            let step = sp.step();
            let key = speller.key_alter(sp.letter);
            let current = *state.get(&step).unwrap_or(&key);
            if sp.alter != current {
                h.accidental = Some(Accidental {
                    alter: sp.alter,
                    courtesy: false,
                });
                state.insert(step, sp.alter);
            } else if !seen.contains(&step) {
                let after_bar = prev_altered.get(&step).is_some_and(|a| *a != sp.alter);
                let other_octave = letter_alter
                    .get(&sp.letter)
                    .is_some_and(|(s, a)| *s != step && *a != sp.alter);
                if after_bar || other_octave {
                    h.accidental = Some(Accidental {
                        alter: sp.alter,
                        courtesy: true,
                    });
                }
            }
            seen.push(step);
            letter_alter.insert(sp.letter, (step, sp.alter));
        }
        prev_altered = state
            .into_iter()
            .filter(|(step, a)| *a != speller.key_alter(step.rem_euclid(7) as u8))
            .collect();
    }
}

/// Beams by the metre's units, their marks per level, the stems of each group and chord,
/// and whether each tuplet needs a bracket.
pub(crate) fn beams_and_stems(v: &mut Voice, metre: &Metre, forced: Option<Stem>) {
    let groups = {
        let ev = &v.events;
        let plain_eighths = |s: i64, e: i64| {
            let inside: Vec<&Event> = ev.iter().filter(|x| x.start >= s && x.end() <= e).collect();
            inside.len() == 4
                && inside
                    .iter()
                    .all(|x| !x.is_rest() && x.dur == Dur::new(3, 0) && x.tuplet.is_none())
        };
        let units = metre.beam_units(plain_eighths);
        let unit_of = |e: &Event| -> (i64, i64) {
            match e.tuplet {
                Some(t) => (-1 - t as i64, 0),
                None => units
                    .iter()
                    .find(|u| e.start >= u.0 && e.start < u.0 + u.1)
                    .map(|u| (u.0, u.1))
                    .unwrap_or((e.start, 0)),
            }
        };
        let beamable = |e: &Event| !e.is_rest() && e.dur.log >= 3;
        let mut groups: Vec<(usize, usize)> = Vec::new();
        let mut i = 0;
        while i < ev.len() {
            if !beamable(&ev[i]) {
                i += 1;
                continue;
            }
            let u = unit_of(&ev[i]);
            let mut j = i;
            while j + 1 < ev.len() && beamable(&ev[j + 1]) && unit_of(&ev[j + 1]) == u {
                j += 1;
            }
            if j > i {
                groups.push((i, j));
            }
            i = j + 1;
        }
        groups
    };
    for (gi, &(a, b)) in groups.iter().enumerate() {
        let max_beams = (a..=b).map(|k| v.events[k].dur.beams()).max().unwrap_or(1);
        for k in a..=b {
            let mut marks = Vec::new();
            let bk = v.events[k].dur.beams();
            for level in 1..=max_beams {
                if bk < level {
                    break;
                }
                let left = k > a && v.events[k - 1].dur.beams() >= level;
                let right = k < b && v.events[k + 1].dur.beams() >= level;
                marks.push(match (left, right) {
                    (true, true) => BeamMark::Continue,
                    (true, false) => BeamMark::End,
                    (false, true) => BeamMark::Begin,
                    (false, false) => hook(v, k, a, b),
                });
            }
            v.events[k].beams = marks;
            v.events[k].group = Some(gi);
        }
        let stem = forced.unwrap_or_else(|| {
            farthest(
                v.events[a..=b]
                    .iter()
                    .flat_map(|e| e.heads().iter().map(|h| h.pos)),
            )
        });
        for k in a..=b {
            v.events[k].stem = stem;
        }
    }
    for e in v.events.iter_mut().filter(|e| e.group.is_none()) {
        e.stem = forced.unwrap_or_else(|| farthest(e.heads().iter().map(|h| h.pos)));
    }
    for t in v.tuplets.iter_mut() {
        t.bracket = !groups.contains(&(t.first, t.last));
    }
    v.groups = groups;
}

/// Which way a lone secondary beam points: toward the note it belongs to rhythmically
/// (after a dotted value, back to it; before one, on to it; else by its place in the
/// pair it completes).
fn hook(v: &Voice, k: usize, a: usize, b: usize) -> BeamMark {
    let e = &v.events[k];
    if k == a {
        BeamMark::HookForward
    } else if k == b || v.events[k - 1].dur.dots > 0 {
        BeamMark::HookBackward
    } else if v.events[k + 1].dur.dots > 0 {
        BeamMark::HookForward
    } else if (e.start / e.len.max(1)) % 2 == 1 {
        BeamMark::HookBackward
    } else {
        BeamMark::HookForward
    }
}

/// The stem direction of notes at these positions: away from the note farthest from the
/// middle line; down when they are as far either way.
pub(crate) fn farthest(pos: impl Iterator<Item = i32>) -> Stem {
    let (lo, hi) = pos.fold((0, 0), |(lo, hi), p| (lo.min(p), hi.max(p)));
    if hi + lo >= 0 {
        Stem::Down
    } else {
        Stem::Up
    }
}
