//! The phrases Chorda is compared on (round 37, ADR 0098): single notes across the range at
//! three dynamics, and the short pieces a listener judges a piano by — soft chords, a fast
//! run, a pedalled ballad, a fortissimo chord, bass notes alone and the top octave. Times
//! are in seconds (the projects run at 120 BPM: a beat is half a second).

/// A note: start (s), length (s), key, velocity.
pub type N = (f64, f64, u8, u8);

/// One phrase: its notes, the sustain pedal's points (time, 0…1: a step at each), its
/// length (s, the render's end) and whether a listener judges it (the ABX kit's).
#[derive(Clone, Debug)]
pub struct Phrase {
    pub id: &'static str,
    pub notes: Vec<N>,
    pub pedal: Vec<(f64, f32)>,
    pub len: f64,
    pub listen: bool,
}

/// The dynamics the single notes are played at (MIDI velocity).
pub const DYNAMICS: [(&str, u8); 3] = [("pp", 30), ("mf", 75), ("ff", 115)];

/// Where each single note starts, how long it is held, and the gap after it (s).
pub const NOTE_EVERY: f64 = 6.0;
pub const NOTE_HELD: f64 = 3.5;

/// A0 … C8 every minor third: 30 keys.
pub fn single_keys() -> Vec<u8> {
    (21u8..=108).step_by(3).collect()
}

/// The single notes at one dynamic: one key every [`NOTE_EVERY`] seconds.
fn singles(id: &'static str, vel: u8) -> Phrase {
    let keys = single_keys();
    let notes = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (i as f64 * NOTE_EVERY, NOTE_HELD, *k, vel))
        .collect();
    Phrase {
        id,
        notes,
        pedal: Vec::new(),
        len: keys.len() as f64 * NOTE_EVERY,
        listen: false,
    }
}

fn chord(at: f64, len: f64, keys: &[u8], vel: u8) -> Vec<N> {
    keys.iter()
        .enumerate()
        // A real hand never lands every finger in the same millisecond: 4 ms apart, low first.
        .map(|(i, k)| {
            let v = (vel as i32 + (i as i32 % 3) - 1).clamp(1, 127) as u8;
            (at + 0.004 * i as f64, len, *k, v)
        })
        .collect()
}

/// The pedal changed at each of `at` (lifted 60 ms before, down again 80 ms after), up at
/// `end`.
fn repedal(at: &[f64], end: f64) -> Vec<(f64, f32)> {
    let mut p = Vec::new();
    for (i, t) in at.iter().enumerate() {
        if i > 0 {
            p.push((t - 0.06, 0.0));
        }
        p.push((t + 0.08, 1.0));
    }
    p.push((end, 0.0));
    p
}

fn soft_chords() -> Phrase {
    // Cmaj7 – Am9 – Fmaj7/A – G13sus4, softly, re-pedalled at each change.
    let chords: [&[u8]; 4] = [
        &[48, 55, 59, 64, 67],
        &[45, 52, 59, 60, 64, 71],
        &[45, 53, 60, 64, 69],
        &[43, 50, 57, 60, 64, 65],
    ];
    let mut notes = Vec::new();
    for (i, c) in chords.iter().enumerate() {
        notes.extend(chord(i as f64 * 2.2, 2.1, c, 42));
    }
    Phrase {
        id: "soft_chords",
        notes,
        pedal: repedal(&[0.0, 2.2, 4.4, 6.6], 9.2),
        len: 12.0,
        listen: true,
    }
}

fn fast_run() -> Phrase {
    // C major, two octaves up and down in sixteenths at 120 BPM, accent on each beat.
    let scale = [0u8, 2, 4, 5, 7, 9, 11];
    let mut up: Vec<u8> = (0..15)
        .map(|i| 60 + 12 * (i / 7) as u8 + scale[i % 7])
        .collect();
    let down: Vec<u8> = up.iter().rev().skip(1).copied().collect();
    up.extend(down);
    let notes = up
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let vel = if i % 4 == 0 { 86 } else { 72 };
            (0.125 * i as f64, 0.115, *k, vel)
        })
        .collect();
    Phrase {
        id: "fast_run",
        notes,
        pedal: Vec::new(),
        len: 6.0,
        listen: true,
    }
}

fn ballad() -> Phrase {
    // Four bars at 72 BPM (a bar 3.33 s): the left hand's open arpeggios, a melody above.
    let bar = 60.0 / 72.0 * 4.0;
    let e = bar / 8.0;
    let left: [[u8; 4]; 4] = [
        [36, 43, 52, 55],
        [33, 40, 48, 52],
        [29, 36, 45, 48],
        [31, 38, 47, 50],
    ];
    let melody: [&[(f64, f64, u8, u8)]; 4] = [
        &[(0.0, 3.0, 76, 66), (3.0, 1.0, 74, 58), (4.0, 4.0, 72, 62)],
        &[(0.0, 2.0, 72, 60), (2.0, 2.0, 71, 56), (4.0, 4.0, 69, 64)],
        &[
            (0.0, 3.0, 69, 62),
            (3.0, 1.0, 72, 60),
            (4.0, 2.0, 76, 70),
            (6.0, 2.0, 77, 66),
        ],
        &[(0.0, 4.0, 74, 64), (4.0, 4.0, 71, 54)],
    ];
    let mut notes = Vec::new();
    for b in 0..4 {
        let t0 = b as f64 * bar;
        for (j, k) in left[b]
            .iter()
            .chain(left[b].iter().rev().skip(1).take(3))
            .enumerate()
        {
            let v = if j == 0 { 58 } else { 44 + (j as u8 % 2) * 4 };
            notes.push((t0 + j as f64 * e * 1.15, bar - j as f64 * e * 1.15, *k, v));
        }
        for (s, l, k, v) in melody[b] {
            notes.push((t0 + s * e, l * e, *k, *v));
        }
    }
    let changes: Vec<f64> = (0..4).map(|b| b as f64 * bar).collect();
    Phrase {
        id: "ballad",
        notes,
        pedal: repedal(&changes, 4.0 * bar + 1.0),
        len: 4.0 * bar + 3.5,
        listen: true,
    }
}

fn ff_chord() -> Phrase {
    let mut notes = chord(0.0, 4.0, &[24, 36, 43, 48, 52, 55, 60, 64, 67, 72], 118);
    notes.sort_by(|a, b| a.0.total_cmp(&b.0));
    Phrase {
        id: "ff_chord",
        notes,
        pedal: vec![(0.05, 1.0), (5.0, 0.0)],
        len: 7.5,
        listen: true,
    }
}

fn bass() -> Phrase {
    let keys = [21u8, 24, 28, 33, 36];
    let notes = keys
        .iter()
        .enumerate()
        .map(|(i, k)| (i as f64 * 3.0, 2.5, *k, 92))
        .collect();
    Phrase {
        id: "bass",
        notes,
        pedal: Vec::new(),
        len: 16.0,
        listen: true,
    }
}

fn top() -> Phrase {
    // C7 E7 G7 C8 short, then the same held, then a soft trill on B6–C7.
    let mut notes: Vec<N> = Vec::new();
    for (i, k) in [96u8, 100, 103, 108].iter().enumerate() {
        notes.push((i as f64 * 0.3, 0.12, *k, 80));
        notes.push((1.4 + i as f64 * 0.7, 0.65, *k, 70));
    }
    for i in 0..16 {
        let k = if i % 2 == 0 { 95 } else { 96 };
        notes.push((4.4 + i as f64 * 0.09, 0.085, k, 52 + (i % 3) as u8 * 4));
    }
    Phrase {
        id: "top",
        notes,
        pedal: Vec::new(),
        len: 7.5,
        listen: true,
    }
}

fn middle_c() -> Phrase {
    // One middle C at mf, held 3 s: the plainest thing a listener compares.
    Phrase {
        id: "middle_c",
        notes: vec![(0.0, 3.0, 60, 75)],
        pedal: Vec::new(),
        len: 5.0,
        listen: true,
    }
}

/// Every phrase, in the order the reports list them.
pub fn all() -> Vec<Phrase> {
    let mut v: Vec<Phrase> = DYNAMICS
        .iter()
        .map(|(d, vel)| {
            let id: &'static str = match *d {
                "pp" => "notes_pp",
                "mf" => "notes_mf",
                _ => "notes_ff",
            };
            singles(id, *vel)
        })
        .collect();
    v.extend([
        middle_c(),
        soft_chords(),
        fast_run(),
        ballad(),
        ff_chord(),
        bass(),
        top(),
    ]);
    v
}
