//! The music linter: what a reviewer hears wrong in a part, found from the notes alone.
//!
//! | Issue | What it is | Severity |
//! |---|---|---|
//! | [`Issue::RingsIntoChange`] | a note still sounding after a chord change into a chord that does not hold it | error when it is a semitone from a tone of the new chord (the demo's A♭ under E♭/G), warning otherwise |
//! | [`Issue::Clash`] | a note held at least [`LintOptions::min_clash`] under a chord, a semitone *above* one of its tones and not one itself (the avoid note; a dominant's ♭9 and ♭13 are tensions) | warning |
//! | [`Issue::Doubled`] | the same pitch twice at once in one part | error when both start together (written twice), warning when they only overlap |
//! | [`Issue::OutOfScale`] | a pitch class outside the key and outside the sounding chord | info |
//! | [`Issue::OutOfRange`] | a pitch the part's instrument cannot play | error |
//!
//! Unpitched parts (drum kits) are checked for doubling and range only.

use super::chart::Chart;
use super::chord::{Chord, Role};
use super::semitone_apart;
use crate::scale::Key;
use crate::time::Ticks;

/// One note to check. `part` groups notes that must not double each other (a track);
/// `id` is the caller's, handed back in findings.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct LintNote {
    pub part: u64,
    pub id: u64,
    pub start: Ticks,
    pub end: Ticks,
    pub pitch: u8,
}

/// What the check knows of the music around the notes.
#[derive(Clone, Debug)]
pub struct LintOptions<'a> {
    pub chart: Option<&'a Chart>,
    pub key: Option<Key>,
    /// Parts whose pitches are not notes of a scale (a drum kit): no harmony checks.
    pub unpitched: &'a [u64],
    /// Per part: the playable pitches as a 128-bit mask (bit `p` set: pitch `p` plays).
    pub playable: &'a [(u64, u128)],
    /// How long past a change a note may ring before it counts (a legato overlap).
    pub ring_tolerance: Ticks,
    /// The least time under a chord for a non-chord tone to count as a clash (shorter ones
    /// are passing and neighbour notes).
    pub min_clash: Ticks,
}

impl Default for LintOptions<'_> {
    fn default() -> Self {
        LintOptions {
            chart: None,
            key: None,
            unpitched: &[],
            playable: &[],
            // A thirty-second and an eighth note.
            ring_tolerance: Ticks(crate::time::PPQ / 8),
            min_clash: Ticks(crate::time::PPQ / 2),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

impl Severity {
    pub fn name(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Issue {
    /// Rings `overlap` past the change at `at` into chord `into` (an index of the chart),
    /// which does not hold it; `against` is the new chord's tone a semitone away, if any.
    RingsIntoChange {
        at: Ticks,
        into: usize,
        overlap: Ticks,
        against: Option<u8>,
    },
    /// A semitone from `against`, a tone of chord `chord` (an index of the chart), for
    /// `held` ticks.
    Clash {
        chord: usize,
        against: u8,
        held: Ticks,
    },
    /// The same pitch as note `other` of the same part, at the same time.
    Doubled { other: u64, same_start: bool },
    /// Outside the key, and not a tone of the chord sounding at its start.
    OutOfScale,
    /// Outside what the part's instrument plays.
    OutOfRange,
}

impl Issue {
    /// A stable name for the issue's kind.
    pub fn kind(&self) -> &'static str {
        match self {
            Issue::RingsIntoChange { .. } => "rings_into_change",
            Issue::Clash { .. } => "clash",
            Issue::Doubled { .. } => "doubled",
            Issue::OutOfScale => "out_of_scale",
            Issue::OutOfRange => "out_of_range",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub part: u64,
    pub note: u64,
    pub pitch: u8,
    /// Where the problem is heard (the change, the chord's start, the note's start).
    pub at: Ticks,
    pub issue: Issue,
    pub severity: Severity,
}

/// Check `notes`. Findings come in time order, then by part, pitch and kind.
pub fn lint(notes: &[LintNote], o: &LintOptions<'_>) -> Vec<Finding> {
    let mut out = Vec::new();
    let pitched = |part: u64| !o.unpitched.contains(&part);
    let ready = o.chart.map(prepare).unwrap_or_default();
    for n in notes {
        if let Some((_, mask)) = o.playable.iter().find(|(p, _)| *p == n.part) {
            if n.pitch > 127 || mask & (1u128 << n.pitch) == 0 {
                out.push(finding(n, n.start, Issue::OutOfRange, Severity::Error));
            }
        }
        if !pitched(n.part) {
            continue;
        }
        if o.chart.is_some() {
            harmony(n, &ready, o, &mut out);
        }
        if let Some(key) = o.key {
            let chord_holds = o
                .chart
                .and_then(|c| c.at(n.start))
                .and_then(|c| c.chord.as_ref())
                .is_some_and(|c| c.contains(n.pitch));
            if !key.contains(n.pitch) && !chord_holds {
                out.push(finding(n, n.start, Issue::OutOfScale, Severity::Info));
            }
        }
    }
    doubled(notes, &mut out);
    out.sort_by(|a, b| {
        (a.at, a.part, a.pitch, a.issue.kind(), a.note).cmp(&(
            b.at,
            b.part,
            b.pitch,
            b.issue.kind(),
            b.note,
        ))
    });
    out
}

fn finding(n: &LintNote, at: Ticks, issue: Issue, severity: Severity) -> Finding {
    Finding {
        part: n.part,
        note: n.id,
        pitch: n.pitch,
        at,
        issue,
        severity,
    }
}

/// One chord of the chart, ready for every note: its span, pitch classes, and for each
/// pitch class the tone it clashes with (or rings against), so a note's check allocates
/// nothing and costs a lookup (review #14).
struct Ready {
    start: Ticks,
    end: Ticks,
    /// `None` under N.C.
    mask: Option<u16>,
    /// It begins a change: the chord before ends where it starts.
    change: bool,
    avoid: [Option<u8>; 12],
    ring: [Option<u8>; 12],
}

fn prepare(chart: &Chart) -> Vec<Ready> {
    chart
        .chords
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let change = i > 0 && chart.chords[i - 1].end == c.start;
            let mut r = Ready {
                start: c.start,
                end: c.end,
                mask: None,
                change,
                avoid: [None; 12],
                ring: [None; 12],
            };
            if let Some(ch) = c.chord.as_ref() {
                r.mask = Some(ch.mask());
                for pc in 0..12u8 {
                    r.avoid[pc as usize] = avoid(pc, ch);
                    // A semitone over a tone of the new chord, or either side of its bass.
                    r.ring[pc as usize] = r.avoid[pc as usize].or_else(|| {
                        (!ch.contains(pc) && semitone_apart(pc, ch.bass)).then_some(ch.bass)
                    });
                }
            }
            r
        })
        .collect()
}

/// The chart's checks for one note: ringing into each change it outlasts, clashing with
/// each chord it is held under — only the chords it overlaps, found by bisection.
fn harmony(n: &LintNote, ready: &[Ready], o: &LintOptions<'_>, out: &mut Vec<Finding>) {
    let pc = n.pitch % 12;
    let first = ready.partition_point(|c| c.end <= n.start);
    for (index, c) in ready.iter().enumerate().skip(first) {
        if c.start >= n.end {
            break;
        }
        let Some(mask) = c.mask else { continue };
        let holds = mask & (1 << pc) != 0;
        let at = c.start;
        if c.change && n.start < at && n.end > at + o.ring_tolerance {
            if !holds {
                let against = c.ring[pc as usize];
                out.push(finding(
                    n,
                    at,
                    Issue::RingsIntoChange {
                        at,
                        into: index,
                        overlap: n.end.min(c.end) - at,
                        against,
                    },
                    if against.is_some() {
                        Severity::Error
                    } else {
                        Severity::Warning
                    },
                ));
            }
            continue;
        }
        let held = n.end.min(c.end) - n.start.max(c.start);
        if held < o.min_clash || holds {
            continue;
        }
        if let Some(against) = c.avoid[pc as usize] {
            out.push(finding(
                n,
                n.start.max(c.start),
                Issue::Clash {
                    chord: index,
                    against,
                    held,
                },
                Severity::Warning,
            ));
        }
    }
}

/// The chord tone a pitch class clashes with. Mainly one it sits **a semitone above** (the
/// avoid note: the 11th over a major third, the ♭6 over the fifth, the ♭9 over a
/// non-dominant's root); and the few a semitone *below* that grate as much — under a major
/// third (a ♭3 against it, but a dominant's ♯9 is a tension), under a major seventh, under
/// a natural 13th (fix review #4). A 9th under a minor third, a major 7th under the root, a
/// 13th under a dominant's 7th, a ♯11 under the fifth are the chord's colours. A dominant
/// takes its ♭9, and its ♭13 when its fifth is perfect, as altered tensions.
pub fn avoid(pc: u8, chord: &Chord) -> Option<u8> {
    if chord.contains(pc) {
        return None;
    }
    let iv = |t: u8| (t + 12 - chord.root) % 12;
    let rel = |t: Option<u8>| t.map(iv);
    let dominant =
        rel(chord.tone(Role::Third)) == Some(4) && rel(chord.tone(Role::Seventh)) == Some(10);
    let fifth = chord.fifth();
    let perfect_fifth = rel(fifth) == Some(7);
    let above = std::iter::once(chord.bass)
        .chain(chord.by_priority())
        .find(|t| {
            (t + 1) % 12 == pc
                && !(dominant && (*t == chord.root || (perfect_fifth && Some(*t) == fifth)))
        });
    above.or_else(|| {
        chord
            .tones
            .iter()
            .filter(|t| {
                (t.role == Role::Third && t.interval == 4)
                    || (t.role == Role::Seventh && t.interval == 11)
                    || (t.role == Role::Tension && t.interval == 21)
            })
            .map(|t| (chord.root + t.interval) % 12)
            .find(|t| (pc + 1) % 12 == *t && !(dominant && iv(pc) == 3))
    })
}

/// The same pitch twice at once in one part.
fn doubled(notes: &[LintNote], out: &mut Vec<Finding>) {
    let mut sorted: Vec<&LintNote> = notes.iter().collect();
    sorted.sort_by_key(|n| (n.part, n.pitch, n.start, n.id));
    for (i, a) in sorted.iter().enumerate() {
        for b in &sorted[i + 1..] {
            if b.part != a.part || b.pitch != a.pitch || b.start >= a.end {
                break;
            }
            let same_start = a.start == b.start;
            out.push(finding(
                b,
                b.start,
                Issue::Doubled {
                    other: a.id,
                    same_start,
                },
                if same_start {
                    Severity::Error
                } else {
                    Severity::Warning
                },
            ));
        }
    }
}
