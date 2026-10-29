//! One staff of a part: its clef, its voices bar by bar, spelled and tied, with
//! accidentals, octave lines, stems, beams and tuplets.

use crate::model::{Clef, Dur, Event, Head, Kind, Spelled, Staff, StaffBar, Stem, Tuplet, Voice};
use crate::read::{BarSpan, ReadNote, Window, STRAIGHT_FINEST};
use crate::rhythm;
use crate::spell::{Motion, Speller};
use crate::staff_marks::{accidentals, beams_and_stems, octave_shifts, ottava_spans};
use crate::voices::{self, Chord};
use cantus_core::notation::ClefChoice;
use std::collections::HashMap;

/// What a staff is for.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StaffPlan {
    /// `None`: the clef that needs the fewest ledger lines.
    pub clef: Option<Clef>,
}

impl StaffPlan {
    pub fn upper() -> StaffPlan {
        StaffPlan {
            clef: Some(Clef::Treble),
        }
    }

    pub fn lower() -> StaffPlan {
        StaffPlan {
            clef: Some(Clef::Bass),
        }
    }

    pub fn single(choice: ClefChoice) -> StaffPlan {
        StaffPlan {
            clef: match choice {
                ClefChoice::Auto => None,
                ClefChoice::Treble => Some(Clef::Treble),
                ClefChoice::Bass => Some(Clef::Bass),
                ClefChoice::Alto => Some(Clef::Alto),
                ClefChoice::Tenor => Some(Clef::Tenor),
                ClefChoice::Treble8vb => Some(Clef::Treble8vb),
                ClefChoice::Treble8va => Some(Clef::Treble8va),
                ClefChoice::Bass8vb => Some(Clef::Bass8vb),
            },
        }
    }
}

/// Ledger lines a note at staff position `pos` needs.
pub fn ledgers(pos: i32) -> i32 {
    if pos >= 6 {
        (pos - 4) / 2
    } else if pos <= -6 {
        (-pos - 4) / 2
    } else {
        0
    }
}

/// The clef with the fewest ledger lines for these steps (an octave clef only when it
/// saves a good many).
fn auto_clef(steps: &[i32]) -> Clef {
    if steps.is_empty() {
        return Clef::Treble;
    }
    let cost = |c: Clef, bias: f64| {
        steps
            .iter()
            .map(|s| ledgers(s - c.middle_step()) as f64 + bias)
            .sum::<f64>()
    };
    let mut best = (cost(Clef::Treble, 0.0), Clef::Treble);
    for (c, bias) in [
        (Clef::Bass, 0.0),
        (Clef::Treble8va, 0.6),
        (Clef::Treble8vb, 0.6),
        (Clef::Bass8vb, 0.6),
    ] {
        let k = cost(c, bias);
        if k < best.0 - 1e-9 {
            best = (k, c);
        }
    }
    best.1
}

/// Build one staff from its notes (sorted by start).
pub fn build_staff(
    plan: &StaffPlan,
    notes: &[ReadNote],
    spans: &[BarSpan],
    windows: &[Window],
    speller: &Speller,
    transpose: i8,
) -> Staff {
    let written = |p: u8| (p as i32 + transpose as i32).clamp(0, 127) as u8;
    let steps: Vec<i32> = notes
        .iter()
        .map(|n| speller.spell(written(n.pitch), Motion::None).step())
        .collect();
    let clef = plan.clef.unwrap_or_else(|| auto_clef(&steps));
    let mut vs = voices::voices(notes);
    // Spell each note with the line it moves on.
    let mut spelled: HashMap<(u64, i64), Spelled> = HashMap::new();
    for v in vs.iter() {
        for (i, c) in v.iter().enumerate() {
            let next = v.get(i + 1).filter(|n| n.start == c.end);
            for n in &c.notes {
                let w = written(n.pitch) as i32;
                let motion = match next {
                    Some(nx) if nx.notes.iter().any(|m| written(m.pitch) as i32 == w + 1) => {
                        Motion::Up
                    }
                    Some(nx) if nx.notes.iter().any(|m| written(m.pitch) as i32 == w - 1) => {
                        Motion::Down
                    }
                    _ => Motion::None,
                };
                spelled.insert((n.id, n.start), speller.spell(written(n.pitch), motion));
            }
        }
    }
    let pos_of = |n: &ReadNote| spelled[&(n.id, n.start)].step() - clef.middle_step();
    // Octave lines: per chord, the shift it is written under.
    let shifts = octave_shifts(&vs, &pos_of, spans);
    let mut bars: Vec<StaffBar> = Vec::with_capacity(spans.len());
    // (bar, voice, event, start, shift, run) of every event under an octave line.
    let mut under: Vec<crate::staff_marks::Under> = Vec::new();
    for (bi, span) in spans.iter().enumerate() {
        let (bs, be) = (span.start, span.start + span.metre.len);
        let bar_windows: Vec<Window> = windows
            .iter()
            .filter(|w| w.start >= bs && w.end() <= be)
            .map(|w| Window {
                start: w.start - bs,
                ..*w
            })
            .collect();
        let mut sb = StaffBar::default();
        for (vi, v) in vs.iter_mut().enumerate() {
            let here: Vec<(usize, &Chord)> = v
                .iter()
                .enumerate()
                .filter(|(_, c)| c.start < be && c.end > bs)
                .collect();
            if here.is_empty() {
                if vi == 0 {
                    sb.voices.push(Voice {
                        events: vec![rest_event(0, span.metre.len, Dur::new(0, 0), true)],
                        ..Voice::default()
                    });
                }
                continue;
            }
            let mut voice = Voice::default();
            let needed: Vec<Window> = bar_windows
                .iter()
                .filter(|w| {
                    here.iter().any(|(_, c)| {
                        [c.start - bs, c.end - bs].iter().any(|&t| {
                            t > w.start && t < w.end() && (t - w.start) % STRAIGHT_FINEST != 0
                        })
                    })
                })
                .copied()
                .collect();
            let mut window_tuplet: HashMap<usize, usize> = HashMap::new();
            let mut at = 0i64;
            let mut push =
                |voice: &mut Voice,
                 pieces: Vec<rhythm::Piece>,
                 make: &dyn Fn(usize, usize, &rhythm::Piece) -> Event| {
                    let n = pieces.len();
                    for (k, p) in pieces.iter().enumerate() {
                        let mut e = make(k, n, p);
                        if let Some(wi) = p.window {
                            let w = needed[wi];
                            let ti = *window_tuplet.entry(wi).or_insert_with(|| {
                                voice.tuplets.push(Tuplet {
                                    actual: w.actual,
                                    normal: w.normal,
                                    unit: Dur::of(w.unit * w.actual as i64 / w.normal as i64)
                                        .unwrap_or(Dur::new(3, 0)),
                                    first: voice.events.len(),
                                    last: voice.events.len(),
                                    bracket: true,
                                });
                                voice.tuplets.len() - 1
                            });
                            voice.tuplets[ti].last = voice.events.len();
                            e.tuplet = Some(ti);
                        }
                        voice.events.push(e);
                    }
                };
            for (ci, c) in &here {
                let s = (c.start - bs).max(0);
                let e = (c.end - bs).min(span.metre.len);
                if s > at {
                    let pieces = rhythm::write(&span.metre, &needed, at, s, true);
                    push(&mut voice, pieces, &|_, _, p| {
                        rest_event(p.start, p.len, p.dur, false)
                    });
                }
                let tied_in = c.start < bs;
                let tie_out = c.end > be;
                let (shift, run) = shifts.get(&(vi, *ci)).copied().unwrap_or((0, 0));
                let heads: Vec<Head> = c
                    .notes
                    .iter()
                    .map(|n| Head {
                        pitch: n.pitch,
                        spelled: spelled[&(n.id, n.start)],
                        pos: pos_of(n) - 7 * shift as i32,
                        accidental: None,
                        tie: false,
                        tied_from: false,
                        id: n.id,
                        velocity: n.velocity,
                    })
                    .collect();
                let pieces = rhythm::write(&span.metre, &needed, s, e, false);
                let first = voice.events.len();
                let count = pieces.len();
                push(&mut voice, pieces, &|k, n, p| {
                    let mut hs = heads.clone();
                    for h in hs.iter_mut() {
                        h.tied_from = k > 0 || tied_in || (k == 0 && c.tie_in.contains(&h.id));
                        h.tie = k + 1 < n || tie_out || (k + 1 == n && c.tie_out.contains(&h.id));
                    }
                    Event {
                        start: p.start,
                        len: p.len,
                        dur: p.dur,
                        kind: Kind::Chord(hs),
                        tuplet: None,
                        stem: Stem::Up,
                        beams: Vec::new(),
                        group: None,
                        dynamic: None,
                    }
                });
                if shift != 0 {
                    for k in first..first + count {
                        under.push((bi, sb.voices.len(), k, voice.events[k].start, shift, run));
                    }
                }
                at = e;
            }
            if at < span.metre.len {
                let pieces = rhythm::write(&span.metre, &needed, at, span.metre.len, true);
                push(&mut voice, pieces, &|_, _, p| {
                    rest_event(p.start, p.len, p.dur, false)
                });
            }
            sb.voices.push(voice);
        }
        bars.push(sb);
    }
    let mut staff = Staff {
        clef,
        bars,
        ottavas: Vec::new(),
    };
    accidentals(&mut staff, speller);
    for (bi, bar) in staff.bars.iter_mut().enumerate() {
        let two = bar.voices.len() > 1;
        for (vi, v) in bar.voices.iter_mut().enumerate() {
            beams_and_stems(
                v,
                &spans[bi].metre,
                two.then_some(if vi == 0 { Stem::Up } else { Stem::Down }),
            );
        }
    }
    staff.ottavas = ottava_spans(&under);
    staff
}

fn rest_event(start: i64, len: i64, dur: Dur, whole_bar: bool) -> Event {
    Event {
        start,
        len,
        dur,
        kind: Kind::Rest { whole_bar },
        tuplet: None,
        stem: Stem::Up,
        beams: Vec::new(),
        group: None,
        dynamic: None,
    }
}
