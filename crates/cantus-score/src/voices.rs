//! Voices on one staff: notes that start and end together are one chord; a line that
//! sounds against another is a second voice (stems down), the higher line keeping the
//! first. A legato overlap no longer than a thirty-second is the same line, cut where the
//! next note starts.

use crate::model::WHOLE;
use crate::read::ReadNote;

/// A chord of one voice: notes with one start and one end.
#[derive(Clone, Debug, PartialEq)]
pub struct Chord {
    pub start: i64,
    pub end: i64,
    pub notes: Vec<ReadNote>,
    /// Notes (by id) tied on into the voice's next chord, and tied in from its last: a
    /// longer note sharing an onset with shorter ones in the second voice (review,
    /// finding 3).
    pub tie_out: Vec<u64>,
    pub tie_in: Vec<u64>,
}

impl Chord {
    fn top(&self) -> u8 {
        self.notes.iter().map(|n| n.pitch).max().unwrap_or(0)
    }
}

/// The longest overlap that is read as legato rather than as two voices.
const LEGATO: i64 = WHOLE / 32;

/// Split a staff's notes (sorted by start) into at most two voices of chords, each
/// voice's chords in order and never overlapping.
pub fn voices(notes: &[ReadNote]) -> [Vec<Chord>; 2] {
    // Chords: one start, one end.
    let mut chords: Vec<Chord> = Vec::new();
    for n in notes {
        match chords
            .iter_mut()
            .rev()
            .take_while(|c| c.start == n.start)
            .find(|c| c.end == n.end)
        {
            Some(c) => c.notes.push(*n),
            None => chords.push(Chord {
                start: n.start,
                end: n.end,
                notes: vec![*n],
                tie_out: Vec::new(),
                tie_in: Vec::new(),
            }),
        }
    }
    // At one onset, the chord holding the highest note first.
    chords.sort_by(|a, b| a.start.cmp(&b.start).then(b.top().cmp(&a.top())));
    let mut v: [Vec<Chord>; 2] = [Vec::new(), Vec::new()];
    for c in chords {
        let busy1 = v[0].last().map(|l| l.end > c.start).unwrap_or(false);
        if !busy1 {
            v[0].push(c);
            continue;
        }
        let last = v[0].last().cloned().unwrap();
        let overlap = last.end - c.start;
        if overlap <= LEGATO && last.start < c.start {
            // Legato: the same line, cut where the next note starts.
            let l = v[0].last_mut().unwrap();
            l.end = c.start;
            v[0].push(c);
            continue;
        }
        if c.top() > last.top() && last.start < c.start {
            // The new line is higher: it keeps voice 1, the held chord goes down.
            let held = v[0].pop().unwrap();
            push_v2(&mut v[1], held);
            v[0].push(c);
        } else {
            push_v2(&mut v[1], c);
        }
    }
    v[1] = second_voice(std::mem::take(&mut v[1]));
    for voice in v.iter_mut() {
        for c in voice.iter_mut() {
            c.notes.sort_by_key(|n| n.pitch);
            let end = c.end;
            for n in c.notes.iter_mut() {
                n.end = end;
            }
        }
    }
    v
}

/// Voice 2 in order and never overlapping itself, every note kept: chords sharing an
/// onset become one chord as long as the shortest, the longer notes tied on into chords of
/// their own (review, finding 3); otherwise the earlier chord ends where the next starts.
fn second_voice(mut v: Vec<Chord>) -> Vec<Chord> {
    v.sort_by_key(|c| (c.start, c.end));
    let mut i = 0;
    while i < v.len() {
        let s = v[i].start;
        let j = i + v[i..].iter().take_while(|c| c.start == s).count();
        if j - i > 1 {
            let group: Vec<Chord> = v.drain(i..j).collect();
            let end = group.iter().map(|c| c.end).min().unwrap_or(s);
            let mut head = Chord {
                start: s,
                end,
                notes: Vec::new(),
                tie_out: Vec::new(),
                tie_in: Vec::new(),
            };
            let mut rest: Vec<Chord> = Vec::new();
            for c in group {
                head.tie_in.extend(c.tie_in.iter().copied());
                for n in &c.notes {
                    head.notes.push(*n);
                    if c.end > end {
                        head.tie_out.push(n.id);
                        let cont = ReadNote { start: end, ..*n };
                        match rest.iter_mut().find(|r| r.end == c.end) {
                            Some(r) => {
                                r.notes.push(cont);
                                r.tie_in.push(n.id);
                            }
                            None => rest.push(Chord {
                                start: end,
                                end: c.end,
                                notes: vec![cont],
                                tie_out: Vec::new(),
                                tie_in: vec![n.id],
                            }),
                        }
                    }
                }
            }
            v.insert(i, head);
            v.extend(rest);
            v.sort_by_key(|c| (c.start, c.end));
        }
        i += 1;
    }
    // The earlier chord ends where the next starts (their starts now always differ). A
    // note it ties on keeps sounding: it joins the chord that cut it, tied in and on, so
    // the tie always lands on its own note where it really sounds (review 2, finding 1).
    for i in 1..v.len() {
        let s = v[i].start;
        if v[i - 1].end > s {
            v[i - 1].end = s;
            let carried: Vec<ReadNote> = v[i - 1]
                .notes
                .iter()
                .filter(|n| v[i - 1].tie_out.contains(&n.id))
                .map(|n| ReadNote { start: s, ..*n })
                .collect();
            for n in carried {
                if v[i].notes.iter().all(|m| m.pitch != n.pitch) {
                    v[i].notes.push(n);
                    v[i].tie_in.push(n.id);
                    v[i].tie_out.push(n.id);
                }
            }
        }
    }
    // A tie holds only between neighbours that meet and both hold its note.
    for i in 0..v.len() {
        let next_ids: Vec<u64> = match v.get(i + 1) {
            Some(n) if n.start == v[i].end => n.notes.iter().map(|m| m.id).collect(),
            _ => Vec::new(),
        };
        v[i].tie_out.retain(|id| next_ids.contains(id));
    }
    for i in 0..v.len() {
        let ins: Vec<u64> = if i > 0 && v[i - 1].end == v[i].start {
            v[i - 1].tie_out.clone()
        } else {
            Vec::new()
        };
        v[i].tie_in.retain(|id| ins.contains(id));
    }
    // A tie out that the next chord does not take in is no tie.
    for i in 0..v.len() {
        let ins: Vec<u64> = v.get(i + 1).map(|n| n.tie_in.clone()).unwrap_or_default();
        v[i].tie_out.retain(|id| ins.contains(id));
    }
    v
}

fn push_v2(v2: &mut Vec<Chord>, c: Chord) {
    v2.push(c);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(start: i64, end: i64, pitch: u8) -> ReadNote {
        ReadNote {
            id: (start * 1000) as u64 + pitch as u64,
            start,
            end,
            pitch,
            velocity: 80,
        }
    }

    #[test]
    fn a_chord_is_one_event() {
        let v = voices(&[n(0, 960, 60), n(0, 960, 64), n(0, 960, 67)]);
        assert_eq!(v[0].len(), 1);
        assert_eq!(v[0][0].notes.len(), 3);
        assert!(v[1].is_empty());
    }

    #[test]
    fn a_held_note_under_a_moving_line_is_a_second_voice() {
        let v = voices(&[
            n(0, 3840, 55),
            n(0, 960, 67),
            n(960, 1920, 69),
            n(1920, 3840, 71),
        ]);
        assert_eq!(
            v[0].iter().map(|c| c.start).collect::<Vec<_>>(),
            vec![0, 960, 1920]
        );
        assert_eq!(v[1].len(), 1);
        assert_eq!(v[1][0].notes[0].pitch, 55);
    }

    /// Review, finding 3: three notes at one onset with three ends — a whole, a half and
    /// a quarter — keep all three (the half and whole share voice 2, the whole tied on).
    #[test]
    fn three_ends_at_one_onset_keep_every_note() {
        let v = voices(&[n(0, 3840, 60), n(0, 1920, 64), n(0, 960, 67)]);
        let mut pitches: Vec<u8> = v
            .iter()
            .flatten()
            .flat_map(|c| c.notes.iter().map(|n| n.pitch))
            .collect();
        pitches.sort_unstable();
        pitches.dedup();
        assert_eq!(pitches, vec![60, 64, 67]);
        // Every note sounds as long as it did: the whole note's pieces add up to a bar.
        let c4: i64 = v
            .iter()
            .flatten()
            .filter(|c| c.notes.iter().any(|n| n.pitch == 60))
            .map(|c| c.end - c.start)
            .sum();
        assert_eq!(c4, 3840);
    }

    /// Review 2, finding 1: a tie only joins chords that hold its note — a later onset
    /// cutting the shared chord short never leaves a tie into a chord without the note.
    #[test]
    fn ties_join_only_chords_that_hold_the_note() {
        let v = voices(&[
            n(0, 3840, 72),
            n(0, 1920, 64),
            n(0, 960, 60),
            n(480, 960, 55),
        ]);
        for voice in &v {
            for (i, c) in voice.iter().enumerate() {
                for id in &c.tie_out {
                    let next = voice.get(i + 1).expect("a tie needs a next chord");
                    assert_eq!(next.start, c.end, "{c:?} → {next:?}");
                    assert!(
                        next.notes.iter().any(|n| n.id == *id),
                        "tie out of {c:?} into {next:?}"
                    );
                    assert!(
                        next.tie_in.contains(id),
                        "{next:?} does not take the tie in"
                    );
                }
                for id in &c.tie_in {
                    let prev = &voice[i.checked_sub(1).expect("a tie needs a previous chord")];
                    assert!(prev.tie_out.contains(id) && prev.notes.iter().any(|n| n.id == *id));
                }
            }
        }
        // Every note still sounds as long as it did.
        let e4: i64 = v
            .iter()
            .flatten()
            .filter(|c| c.notes.iter().any(|n| n.pitch == 64))
            .map(|c| c.end - c.start)
            .sum();
        assert_eq!(e4, 1920);
    }

    #[test]
    fn a_legato_overlap_is_one_line() {
        let v = voices(&[n(0, 1000, 60), n(960, 1920, 62)]);
        assert_eq!(v[0].len(), 2);
        assert_eq!(v[0][0].end, 960);
        assert!(v[1].is_empty());
    }
}
