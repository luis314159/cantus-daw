//! **Melody**: one line from Basic Pitch's notes (ADR 0086). Basic Pitch imposes no
//! monophony — a sung line comes with its octave and fifth ghosts, a held note under the
//! next one's attack. The line keeps the loudest note wherever two overlap: notes are
//! taken by their mean posterior, loudest first (the longer first among equals), each
//! keeping only the time no note already kept covers, and the pieces left shorter than
//! the shortest note are dropped. (Weighing length too lets a quiet held accompaniment
//! take the line from the tune over it.) ("The highest note", the other rule tried, keeps the ghosts an octave up: its
//! measured F-measure is in ADR 0086.)

use super::spec::NoteEvent;

/// The line: notes that never overlap, in time order.
pub fn line(notes: &[NoteEvent], shortest: f64) -> Vec<NoteEvent> {
    let mut order: Vec<&NoteEvent> = notes.iter().collect();
    order.sort_by(|a, b| {
        b.amplitude
            .total_cmp(&a.amplitude)
            .then((b.end - b.start).total_cmp(&(a.end - a.start)))
            .then(a.start.total_cmp(&b.start))
            .then(a.pitch.cmp(&b.pitch))
    });
    let mut kept: Vec<NoteEvent> = Vec::new();
    for n in order {
        // The parts of `n` no kept note covers.
        let mut parts = vec![(n.start, n.end)];
        for k in &kept {
            parts = parts
                .into_iter()
                .flat_map(|(a, b)| {
                    if k.end <= a || k.start >= b {
                        vec![(a, b)]
                    } else {
                        let mut v = Vec::new();
                        if k.start > a {
                            v.push((a, k.start));
                        }
                        if k.end < b {
                            v.push((k.end, b));
                        }
                        v
                    }
                })
                .collect();
        }
        for (a, b) in parts {
            if b - a >= shortest {
                kept.push(NoteEvent {
                    start: a,
                    end: b,
                    // The bends stay with the note only when it is kept whole: a piece's
                    // frames would no longer be evenly spaced over it.
                    bends: if a == n.start && b == n.end {
                        n.bends.clone()
                    } else {
                        Vec::new()
                    },
                    ..n.clone()
                });
            }
        }
    }
    kept.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    kept
}

/// The other rule, for the measurement: wherever notes overlap, the highest sounds.
pub fn highest(notes: &[NoteEvent], shortest: f64) -> Vec<NoteEvent> {
    // Pitch decides; the posterior only orders equal pitches.
    let ranked: Vec<NoteEvent> = notes
        .iter()
        .map(|n| NoteEvent {
            amplitude: n.pitch as f32 * 1000.0 + n.amplitude,
            ..n.clone()
        })
        .collect();
    let mut out = line(&ranked, shortest);
    for o in &mut out {
        o.amplitude = (o.amplitude - o.pitch as f32 * 1000.0).max(0.0);
    }
    out
}
