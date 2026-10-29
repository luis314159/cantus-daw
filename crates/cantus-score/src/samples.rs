//! The passages the engraving is judged on — the golden tests, the round trip and the
//! shots all write these: a melody, a two-hand piano piece, triplets, syncopation, long
//! tied notes and a metre change (with a tempo change), each written for this test suite.

use crate::build::PartSource;
use crate::read::SourceNote;
use cantus_core::notation::ScoreSettings;
use cantus_core::scale::{Key, ScaleKind};
use cantus_core::tempo::{MetreChange, TempoMap, TempoPoint};
use cantus_core::time::{Ticks, TimeSignature};

const Q: i64 = 960;
const E: i64 = Q / 2;
const S: i64 = Q / 4;
const T: i64 = Q / 3;

/// A passage: its song, and its parts.
pub struct Passage {
    pub name: &'static str,
    pub title: &'static str,
    pub tempo: TempoMap,
    pub key: Option<Key>,
    pub parts: Vec<PartSource>,
}

fn notes(list: &[(i64, i64, u8)], velocity: u8, first_id: u64) -> Vec<SourceNote> {
    list.iter()
        .enumerate()
        .map(|(i, &(start, len, pitch))| SourceNote {
            id: first_id + i as u64,
            start,
            len,
            pitch,
            velocity,
        })
        .collect()
}

/// A line of `(length, pitch)` from `start` (pitch 0 is a rest).
fn line(start: i64, items: &[(i64, u8)]) -> Vec<(i64, i64, u8)> {
    let mut t = start;
    let mut out = Vec::new();
    for &(len, p) in items {
        if p > 0 {
            out.push((t, len, p));
        }
        t += len;
    }
    out
}

fn part(name: &str, notes: Vec<SourceNote>, settings: ScoreSettings) -> PartSource {
    PartSource {
        name: name.into(),
        notes,
        settings,
    }
}

fn four_four(bpm: f64) -> TempoMap {
    TempoMap::constant(bpm, TimeSignature::new(4, 4))
}

/// A melody in D major: two four-bar phrases, the second closing on the tonic; dotted
/// rhythms, an F♯ and a C natural to spell, a phrase end with a half note.
pub fn melody() -> Passage {
    let (fs, g, a, b, c, cs, d, e) = (66, 67, 69, 71, 72, 73, 62, 64);
    let d5 = 74;
    let items = [
        (Q, fs),
        (Q, fs),
        (Q, g),
        (Q, a),
        (Q, a),
        (Q, g),
        (Q, fs),
        (Q, e),
        (Q, d),
        (Q, d),
        (Q, e),
        (Q, fs),
        (Q + E, fs),
        (E, e),
        (2 * Q, e),
        (Q, fs),
        (Q, fs),
        (Q, g),
        (Q, a),
        (E, a),
        (E, b),
        (E, c),
        (E, b),
        (Q, a),
        (Q, fs),
        (S, d5),
        (S, cs),
        (E, b),
        (E, a),
        (E, g),
        (Q, fs),
        (Q, e),
        (3 * Q, d),
        (Q, 0),
    ];
    Passage {
        name: "melody",
        title: "Melody",
        tempo: four_four(108.0),
        key: Some(Key::new(2, ScaleKind::Major)),
        parts: vec![part(
            "Flute",
            notes(&line(0, &items), 88, 1),
            ScoreSettings::default(),
        )],
    }
}

/// A two-hand piano piece in C: a right-hand melody over an Alberti bass, the melody
/// stepping below middle C (the split follows the hands), a chord in each hand at the end.
pub fn piano() -> Passage {
    let rh = line(
        0,
        &[
            (2 * Q, 72),
            (Q, 76),
            (Q, 79),
            (Q + E, 71),
            (S, 72),
            (S, 74),
            (2 * Q, 72),
            (2 * Q, 81),
            (Q, 79),
            (Q, 84),
            (E, 79),
            (E, 77),
            (E, 76),
            (E, 74),
            (E, 72),
            (E, 71),
            (E, 69),
            (E, 67),
            (Q, 65),
            (Q, 64),
            (Q, 62),
            (Q, 59),
        ],
    );
    let mut rh = rh;
    // The cadence: a chord in each hand.
    for p in [60, 64, 67] {
        rh.push((5 * 4 * Q, 4 * Q, p));
    }
    let alberti = |t: i64, root: u8, third: u8, fifth: u8| {
        line(
            t,
            &[
                (E, root),
                (E, fifth),
                (E, third),
                (E, fifth),
                (E, root),
                (E, fifth),
                (E, third),
                (E, fifth),
            ],
        )
    };
    let mut lh = Vec::new();
    lh.extend(alberti(0, 48, 52, 55));
    lh.extend(alberti(4 * Q, 50, 53, 55));
    lh.extend(alberti(8 * Q, 53, 57, 60));
    lh.extend(alberti(12 * Q, 43, 50, 53));
    lh.extend(line(16 * Q, &[(2 * Q, 48), (2 * Q, 43)]));
    lh.extend([(20 * Q, 4 * Q, 36), (20 * Q, 4 * Q, 48)]);
    let mut all = rh;
    all.extend(lh);
    all.sort();
    Passage {
        name: "piano",
        title: "Sonatina",
        tempo: four_four(120.0),
        key: Some(Key::new(0, ScaleKind::Major)),
        parts: vec![part(
            "Piano",
            // Played: soft for two bars, loud for two, then moderately loud.
            notes(&all, 80, 1)
                .into_iter()
                .map(|mut n| {
                    n.velocity = match n.start / (4 * Q) {
                        0 | 1 => 50,
                        2 | 3 => 100,
                        _ => 76,
                    };
                    n
                })
                .collect(),
            ScoreSettings {
                dynamics: true,
                ..ScoreSettings::default()
            },
        )],
    }
}

/// Triplets: eighth triplets on a beat, a triplet with a rest in it, quarter triplets
/// over a half bar, a triplet tied over the beat, and sixteenth sextuplets.
pub fn triplets() -> Passage {
    let mut n = Vec::new();
    n.extend(line(
        0,
        &[
            (T, 67),
            (T, 69),
            (T, 71),
            (Q, 72),
            (T, 74),
            (T, 0),
            (T, 72),
            (Q, 71),
        ],
    ));
    n.extend(line(
        4 * Q,
        &[(2 * T, 69), (2 * T, 71), (2 * T, 72), (2 * Q, 74)],
    ));
    n.extend(line(
        8 * Q,
        &[(E, 76), (E, 74), (T, 72), (T, 71), (T + Q, 69), (Q, 67)],
    ));
    let x = Q / 6;
    n.extend(line(
        12 * Q,
        &[
            (x, 72),
            (x, 74),
            (x, 76),
            (x, 78),
            (x, 79),
            (x, 81),
            (Q, 83),
            (2 * Q, 84),
        ],
    ));
    Passage {
        name: "triplets",
        title: "Triplets",
        tempo: four_four(84.0),
        key: Some(Key::new(7, ScaleKind::Major)),
        parts: vec![part("Violin", notes(&n, 72, 1), ScoreSettings::default())],
    }
}

/// Syncopation: eighth–quarter–eighth, quarter–half–quarter, an off-beat line over the
/// half bar, an anticipation tied over the bar line, the Scotch snap.
pub fn syncopation() -> Passage {
    let mut n = Vec::new();
    n.extend(line(0, &[(E, 60), (Q, 62), (E, 64), (Q, 65), (Q, 67)]));
    n.extend(line(4 * Q, &[(Q, 69), (2 * Q, 67), (Q, 65)]));
    n.extend(line(
        8 * Q,
        &[
            (E, 64),
            (Q, 65),
            (Q, 67),
            (Q, 69),
            (E + 2 * Q, 70),
            (Q, 69),
            (Q, 67),
        ],
    ));
    n.extend(line(
        16 * Q,
        &[(S, 62), (E + S, 60), (S, 62), (E + S, 64), (2 * Q, 60)],
    ));
    Passage {
        name: "syncopation",
        title: "Syncopation",
        tempo: four_four(100.0),
        key: Some(Key::new(5, ScaleKind::Major)),
        parts: vec![part(
            "Saxophone",
            notes(&n, 96, 1),
            ScoreSettings::default(),
        )],
    }
}

/// Long tied notes in 3/4: a note over three bars, a dotted value over the bar line, a
/// note from the second beat over two bars, a low bass part (bass clef by range).
pub fn long_ties() -> Passage {
    let sig = TimeSignature::new(3, 4);
    let n = line(
        0,
        &[
            (7 * Q, 43),
            (2 * Q, 45),
            (Q, 47),
            (Q + E, 48),
            (E, 50),
            (Q, 52),
            (5 * Q, 50),
            (Q, 43),
            (3 * Q, 0),
        ],
    );
    Passage {
        name: "long_ties",
        title: "Long notes",
        tempo: TempoMap::constant(72.0, sig),
        key: Some(Key::new(7, ScaleKind::Major)),
        parts: vec![part("Cello", notes(&n, 64, 1), ScoreSettings::default())],
    }
}

/// A metre change: 4/4, a bar of 7/8 (2 + 2 + 3), two of 6/8, one of 3/4 and 4/4 again,
/// with the tempo going from ♩ = 96 to ♩ = 132 at the 6/8.
pub fn metre_change() -> Passage {
    let bar1 = 0;
    let bar2 = 4 * Q;
    let bar3 = bar2 + 7 * E;
    let bar5 = bar3 + 2 * 6 * E;
    let bar6 = bar5 + 3 * Q;
    let metres = vec![
        MetreChange {
            tick: Ticks(bar1),
            signature: TimeSignature::new(4, 4),
        },
        MetreChange {
            tick: Ticks(bar2),
            signature: TimeSignature::new(7, 8),
        },
        MetreChange {
            tick: Ticks(bar3),
            signature: TimeSignature::new(6, 8),
        },
        MetreChange {
            tick: Ticks(bar5),
            signature: TimeSignature::new(3, 4),
        },
        MetreChange {
            tick: Ticks(bar6),
            signature: TimeSignature::new(4, 4),
        },
    ];
    let points = vec![
        TempoPoint {
            tick: Ticks(0),
            bpm: 96.0,
            ramp: false,
        },
        TempoPoint {
            tick: Ticks(bar3),
            bpm: 132.0,
            ramp: false,
        },
    ];
    let tempo = TempoMap::new(96.0, TimeSignature::new(4, 4), &points, &metres);
    let mut n = Vec::new();
    n.extend(line(bar1, &[(Q, 64), (Q, 66), (Q, 68), (Q, 69)]));
    n.extend(line(
        bar2,
        &[
            (E, 71),
            (E, 69),
            (E, 68),
            (E, 66),
            (E, 64),
            (E, 66),
            (E, 68),
        ],
    ));
    n.extend(line(bar3, &[(Q, 69), (E, 71), (E, 73), (E, 71), (E, 69)]));
    n.extend(line(bar3 + 6 * E, &[(Q + E, 68), (Q + E, 64)]));
    n.extend(line(bar5, &[(Q, 66), (Q, 68), (Q, 69)]));
    n.extend(line(bar6, &[(4 * Q, 69)]));
    Passage {
        name: "metre_change",
        title: "Metres",
        tempo,
        key: Some(Key::new(9, ScaleKind::Major)),
        // A B♭ clarinet: written a tone above how it sounds.
        parts: vec![part(
            "Clarinet in B♭",
            notes(&n, 80, 1),
            ScoreSettings {
                transpose: 2,
                ..ScoreSettings::default()
            },
        )],
    }
}

/// A high line (a piccolo's): the passage over four ledger lines goes under an octave
/// line, and comes back down to the staff.
pub fn high() -> Passage {
    let n = line(
        0,
        &[
            (Q, 84),
            (Q, 86),
            (Q, 88),
            (Q, 91),
            (E, 93),
            (E, 95),
            (E, 96),
            (E, 98),
            (Q, 100),
            (Q, 96),
            (Q, 93),
            (Q, 91),
            (Q, 88),
            (Q, 84),
            (2 * Q, 79),
            (2 * Q, 76),
        ],
    );
    Passage {
        name: "high",
        title: "High",
        tempo: four_four(112.0),
        key: Some(Key::new(0, ScaleKind::Major)),
        // In the treble clef (not Auto's octave clef): the high bars take an 8va line.
        parts: vec![part(
            "Piccolo",
            notes(&n, 90, 1),
            ScoreSettings {
                clef: cantus_core::notation::ClefChoice::Treble,
                ..ScoreSettings::default()
            },
        )],
    }
}

/// Two voices on one staff (review, finding 3): a whole, a half and a quarter struck
/// together under a moving line, then a held note against a moving one.
pub fn voices() -> Passage {
    let mut n = vec![(0, 4 * Q, 60), (0, 2 * Q, 64)];
    n.extend(line(0, &[(Q, 67), (Q, 69), (Q, 71), (Q, 72)]));
    n.push((4 * Q, 4 * Q, 64));
    n.extend(line(4 * Q, &[(Q, 72), (E, 74), (E, 72), (2 * Q, 71)]));
    n.sort();
    Passage {
        name: "voices",
        title: "Voices",
        tempo: four_four(90.0),
        key: Some(Key::new(0, ScaleKind::Major)),
        parts: vec![part(
            "Guitar",
            notes(&n, 80, 1),
            ScoreSettings {
                staves: cantus_core::notation::StaffChoice::Single,
                ..ScoreSettings::default()
            },
        )],
    }
}

/// Every passage, in the order above.
pub fn all() -> Vec<Passage> {
    vec![
        melody(),
        piano(),
        triplets(),
        syncopation(),
        long_ties(),
        metre_change(),
        high(),
        voices(),
    ]
}
