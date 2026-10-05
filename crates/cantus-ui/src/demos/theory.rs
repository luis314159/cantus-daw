//! The music theory the demo songs are written in: note names, chord symbols, voice
//! leading, a melody line in text and a drum grid in text.
//!
//! The songs are code, so they are written the way a musician reads a chart — `"A/C#"`,
//! `"E5:1.5 D5:.5"`, `"x..gx.g."` — and turned into notes here. Voicings are chosen by
//! moving every voice as little as possible from the chord before, which is what a
//! keyboard player does without thinking and what makes a progression sound like music
//! rather than like blocks.

/// A note in time, relative to the start of whatever holds it, in beats.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Ev {
    pub at: f64,
    pub len: f64,
    pub pitch: u8,
    pub vel: u8,
}

impl Ev {
    pub fn new(at: f64, len: f64, pitch: u8, vel: u8) -> Ev {
        Ev {
            at,
            len,
            pitch,
            vel,
        }
    }
}

/// Shift every event by `beats` (a bar's offset inside a clip).
pub fn shift(evs: &[Ev], beats: f64) -> Vec<Ev> {
    evs.iter()
        .map(|e| Ev {
            at: e.at + beats,
            ..*e
        })
        .collect()
}

/// Transpose by semitones, staying inside MIDI.
pub fn transpose(evs: &[Ev], semis: i32) -> Vec<Ev> {
    evs.iter()
        .map(|e| Ev {
            pitch: (e.pitch as i32 + semis).clamp(0, 127) as u8,
            ..*e
        })
        .collect()
}

/// The pitch class of a note letter with its accidentals: `C` 0 … `B` 11.
pub fn pitch_class(name: &str) -> Option<u8> {
    let mut chars = name.chars();
    let base: i32 = match chars.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut pc = base;
    for c in chars {
        match c {
            '#' => pc += 1,
            'b' => pc -= 1,
            _ => return None,
        }
    }
    Some(pc.rem_euclid(12) as u8)
}

/// A note name with its octave, scientific pitch: `C4` is 60, `F#2` is 42, `Bb-1` works.
pub fn note(name: &str) -> u8 {
    let split = name
        .char_indices()
        .skip(1)
        .find(|(_, c)| c.is_ascii_digit() || *c == '-')
        .map(|(i, _)| i)
        .unwrap_or(name.len());
    let (letter, octave) = name.split_at(split);
    let pc = pitch_class(letter).unwrap_or_else(|| panic!("not a note: {name}")) as i32;
    let octave: i32 = octave
        .parse()
        .unwrap_or_else(|_| panic!("no octave: {name}"));
    // `Cb4` is B3 and `B#3` is C4: the letter's octave decides, the accidental moves it.
    let raw = pitch_class(&letter[..1]).unwrap_or(0) as i32;
    let acc = pc - raw;
    let acc = if acc > 6 {
        acc - 12
    } else if acc < -6 {
        acc + 12
    } else {
        acc
    };
    ((octave + 1) * 12 + raw + acc).clamp(0, 127) as u8
}

/// A chord: its root, the intervals above it, and the bass note under it.
#[derive(Clone, Debug, PartialEq)]
pub struct Chord {
    pub root: u8,
    pub tones: Vec<u8>,
    pub bass: u8,
}

impl Chord {
    /// Pitch classes of the chord, root first, without repeats.
    pub fn classes(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        for t in &self.tones {
            let pc = (self.root + t) % 12;
            if !out.contains(&pc) {
                out.push(pc);
            }
        }
        out
    }

    /// The classes a hand plays over the bass: the fifth goes first when there are more
    /// than `voices`, then the root (the bass has it), so the colour tones stay.
    pub fn upper(&self, voices: usize) -> Vec<u8> {
        let mut tones: Vec<u8> = self.tones.clone();
        let drop_order = [7u8, 0];
        for d in drop_order {
            if tones.len() <= voices {
                break;
            }
            if let Some(i) = tones.iter().position(|t| *t == d) {
                tones.remove(i);
            }
        }
        tones.truncate(voices);
        Chord {
            tones,
            ..self.clone()
        }
        .classes()
    }

    /// The bass note in the octave starting at `low`.
    pub fn bass_at(&self, low: u8) -> u8 {
        let mut p = low - low % 12 + self.bass;
        if p < low {
            p += 12;
        }
        p
    }
}

/// A chord symbol: `D`, `Bm`, `A/C#`, `Gmaj7`, `Em9`, `C7sus4`, `F#m7b5`, `G13`, `Ab6`.
pub fn chord(sym: &str) -> Chord {
    let (body, bass) = match sym.split_once('/') {
        Some((b, s)) => (b, Some(s)),
        None => (sym, None),
    };
    let root_len = if body.len() > 1 && matches!(&body[1..2], "#" | "b") {
        2
    } else {
        1
    };
    let root = pitch_class(&body[..root_len]).unwrap_or_else(|| panic!("bad chord {sym}"));
    let tones: &[u8] = match &body[root_len..] {
        "" => &[0, 4, 7],
        "m" => &[0, 3, 7],
        "5" => &[0, 7],
        "6" => &[0, 4, 7, 9],
        "m6" => &[0, 3, 7, 9],
        "7" => &[0, 4, 7, 10],
        "maj7" => &[0, 4, 7, 11],
        "m7" => &[0, 3, 7, 10],
        "mmaj7" => &[0, 3, 7, 11],
        "9" => &[0, 4, 7, 10, 14],
        "maj9" => &[0, 4, 7, 11, 14],
        "m9" => &[0, 3, 7, 10, 14],
        "m11" => &[0, 3, 7, 10, 14, 17],
        "11" => &[0, 7, 10, 14, 17],
        "13" => &[0, 4, 7, 10, 14, 21],
        "add9" => &[0, 4, 7, 14],
        "madd9" => &[0, 3, 7, 14],
        "sus2" => &[0, 2, 7],
        "sus4" => &[0, 5, 7],
        "7sus4" => &[0, 5, 7, 10],
        "dim" => &[0, 3, 6],
        "dim7" => &[0, 3, 6, 9],
        "m7b5" => &[0, 3, 6, 10],
        "aug" => &[0, 4, 8],
        "7b9" => &[0, 4, 7, 10, 13],
        "7#9" => &[0, 4, 7, 10, 15],
        "maj7#11" => &[0, 4, 7, 11, 18],
        q => panic!("unknown chord quality {q} in {sym}"),
    };
    let bass = bass
        .map(|b| pitch_class(b).unwrap_or_else(|| panic!("bad bass in {sym}")))
        .unwrap_or(root);
    Chord {
        root,
        tones: tones.to_vec(),
        bass,
    }
}

/// Every way to place `classes` between `lo` and `hi` (inclusive), each class once,
/// spanning no more than `span` semitones, sorted low to high.
fn voicings(classes: &[u8], lo: u8, hi: u8, span: u8) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut cur = Vec::with_capacity(classes.len());
    fn walk(i: usize, classes: &[u8], lo: u8, hi: u8, cur: &mut Vec<u8>, out: &mut Vec<Vec<u8>>) {
        if i == classes.len() {
            let mut v = cur.clone();
            v.sort_unstable();
            out.push(v);
            return;
        }
        let mut p = lo - lo % 12 + classes[i];
        if p < lo {
            p += 12;
        }
        while p <= hi {
            if !cur.contains(&p) {
                cur.push(p);
                walk(i + 1, classes, lo, hi, cur, out);
                cur.pop();
            }
            p += 12;
        }
    }
    walk(0, classes, lo, hi, &mut cur, &mut out);
    out.retain(|v| v.last().unwrap_or(&0) - v.first().unwrap_or(&0) <= span);
    out
}

/// The voicing of `classes` nearest to `prev` — the smallest total movement of the
/// voices, ties to the narrower one — or, with nothing before, the one centred on
/// `centre`.
pub fn lead(prev: &[u8], classes: &[u8], lo: u8, hi: u8, span: u8, centre: u8) -> Vec<u8> {
    let all = voicings(classes, lo, hi, span);
    let cost = |v: &Vec<u8>| -> i32 {
        let width = (*v.last().unwrap_or(&0) as i32 - *v.first().unwrap_or(&0) as i32) / 4;
        if prev.is_empty() {
            let mean = v.iter().map(|p| *p as i32).sum::<i32>() / v.len().max(1) as i32;
            return (mean - centre as i32).abs() * 4 + width;
        }
        let moved: i32 = v
            .iter()
            .map(|p| {
                prev.iter()
                    .map(|q| (*p as i32 - *q as i32).abs())
                    .min()
                    .unwrap_or(0)
            })
            .sum();
        let order: i32 = if v.len() == prev.len() {
            v.iter()
                .zip(prev)
                .map(|(a, b)| (*a as i32 - *b as i32).abs())
                .sum()
        } else {
            moved
        };
        // Drifting away from the centre costs a little, so a long progression cannot
        // walk off the keyboard one smooth step at a time.
        let mean = v.iter().map(|p| *p as i32).sum::<i32>() / v.len().max(1) as i32;
        moved * 2 + order + width + (mean - centre as i32).abs() / 3
    };
    all.into_iter()
        .min_by_key(|v| cost(v))
        .unwrap_or_else(|| panic!("no voicing of {classes:?} in {lo}..{hi}"))
}

/// Voice a whole progression: one voicing per symbol, each led from the last.
pub fn progression(symbols: &[&str], voices: usize, lo: u8, hi: u8, centre: u8) -> Vec<Vec<u8>> {
    let mut prev: Vec<u8> = Vec::new();
    symbols
        .iter()
        .map(|s| {
            let c = chord(s);
            let v = lead(&prev, &c.upper(voices), lo, hi, 14, centre);
            prev = v.clone();
            v
        })
        .collect()
}

/// A melody in text: `E5:1.5 D5:.5 r:1 >F#5:2 _A4:.5`. Each token is a note (or `r` for
/// a rest) and its length in beats; `>` accents, `_` plays it soft. Notes sound for
/// `gate` of their length. Velocities are around `vel`.
pub fn melody(text: &str, vel: u8, gate: f64) -> Vec<Ev> {
    let mut at = 0.0;
    let mut out = Vec::new();
    for tok in text.split_whitespace() {
        if tok == "|" {
            continue;
        }
        let (name, len) = tok
            .split_once(':')
            .unwrap_or_else(|| panic!("melody token without a length: {tok}"));
        let len: f64 = len
            .parse()
            .unwrap_or_else(|_| panic!("bad length in {tok}"));
        let (v, name) = if let Some(n) = name.strip_prefix('>') {
            (vel.saturating_add(16), n)
        } else if let Some(n) = name.strip_prefix('_') {
            (vel.saturating_sub(22), n)
        } else {
            (vel, name)
        };
        if name != "r" {
            out.push(Ev::new(at, len * gate, note(name), v.clamp(1, 127)));
        }
        at += len;
    }
    out
}

/// A drum line in text, one character per step of `step` beats: `X` accent, `x` a hit,
/// `o` a softer one, `g` a ghost note, `.` nothing. Spaces and `|` are for the eye.
pub fn grid(text: &str, pitch: u8, step: f64) -> Vec<Ev> {
    let mut out = Vec::new();
    let mut i = 0usize;
    for c in text.chars() {
        let vel = match c {
            'X' => Some(120),
            'x' => Some(100),
            'o' => Some(80),
            'g' => Some(44),
            '.' => None,
            ' ' | '|' => continue,
            other => panic!("unknown drum step {other:?}"),
        };
        if let Some(v) = vel {
            out.push(Ev::new(i as f64 * step, step.min(0.25), pitch, v));
        }
        i += 1;
    }
    out
}

/// A small deterministic generator: every demo is the same song on every machine.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    pub fn next_u32(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 32) as u32
    }

    /// Uniform in −1…1.
    pub fn bi(&mut self) -> f64 {
        self.next_u32() as f64 / u32::MAX as f64 * 2.0 - 1.0
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next_u32() as f64 / u32::MAX as f64) < p
    }
}
