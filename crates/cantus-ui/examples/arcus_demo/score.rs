//! The piece: *Andante cantabile* in D major for violin, viola, cello and double bass, an
//! original composition written for Arcus (round 42). A sixteen-bar theme (a period: a half
//! cadence, then a full one) is stated by the cello alone, then passed through the quartet
//! as the instruments enter one by one, carried to a climax and let go.
//!
//! The parts are written in a small text notation, one string per instrument and section,
//! the way a player reads a part: `A3:1.5` is A3 for a beat and a half, `r:1` a rest,
//! `(` … `)` a slur (one bow; the notes after the first marked *legato*), `|` a bar line
//! (checked: it must fall on a bar). After the length, the marks: `s` staccato, `i`
//! spiccato, `p` pizzicato, `t` tremolo (slurred tremolo notes overlap, so the strokes go
//! on while the finger moves), `n` a sustained stroke of its own, `d`/`u` the bow down or
//! up, `>` an accent.

use cantus_core::note_art::{Articulation, Bow, NoteArt};

/// Beats in a bar (4/4 throughout).
pub const BAR: f64 = 4.0;
/// The tempo at the start (the tempo track slows it at the two cadences).
pub const BPM: f64 = 74.0;
/// How far a slurred tremolo note runs into the next, in beats (about 30 ms).
const TREM_OVERLAP: f64 = 0.04;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Who {
    Violin,
    Viola,
    Cello,
    Bass,
}

impl Who {
    pub const ALL: [Who; 4] = [Who::Violin, Who::Viola, Who::Cello, Who::Bass];

    /// The track's name, in Spanish (the owner's song).
    pub fn name(self) -> &'static str {
        match self {
            Who::Violin => "Violín",
            Who::Viola => "Viola",
            Who::Cello => "Violonchelo",
            Who::Bass => "Contrabajo",
        }
    }

    /// The factory preset each plays (the cantabile ones: a delayed vibrato, the bow a
    /// little over the fingerboard, the body's ring).
    pub fn preset(self) -> &'static str {
        match self {
            Who::Violin => "arcus.preset.violin_cantabile",
            Who::Viola => "arcus.preset.viola_cantabile",
            Who::Cello => "arcus.preset.cello_cantabile",
            Who::Bass => "arcus.preset.bass_cantabile",
        }
    }

    /// The idiomatic range written for it (MIDI keys), checked on every note.
    pub fn range(self) -> (u8, u8) {
        match self {
            Who::Violin => (55, 88), // G3 … E6
            Who::Viola => (48, 76),  // C3 … E5
            Who::Cello => (36, 69),  // C2 … A4
            Who::Bass => (28, 55),   // E1 … G3
        }
    }
}

/// One section of the form.
pub struct Section {
    /// Its first bar, from 1.
    pub bar: u32,
    pub bars: u32,
    /// Its name and what happens in it, in Spanish (the page and the markers).
    pub name: &'static str,
    pub what: &'static str,
}

pub const SECTIONS: [Section; 7] = [
    Section {
        bar: 1,
        bars: 8,
        name: "I · El tema",
        what: "El violonchelo solo canta el tema, todo ligado, con un crescendo y un diminuendo (CC1) en cada frase.",
    },
    Section {
        bar: 9,
        bars: 8,
        name: "II · Entra el contrabajo",
        what: "El violonchelo sigue con una segunda frase en si menor (con un pasaje en détaché); el contrabajo entra debajo en pizzicato y, a mitad, toma el arco.",
    },
    Section {
        bar: 17,
        bars: 8,
        name: "III · La viola",
        what: "Vuelve el tema en el violonchelo; la viola canta encima una contramelodía ligada y el contrabajo sostiene el bajo con arco.",
    },
    Section {
        bar: 25,
        bars: 8,
        name: "IV · Entra el violín",
        what: "El violín toma el tema una octava arriba. El violonchelo pasa a la armonía: arpegios en spiccato, luego notas largas; la viola puntea a contratiempo en staccato y el contrabajo vuelve al pizzicato antes del arco.",
    },
    Section {
        bar: 33,
        bars: 6,
        name: "V · Clímax",
        what: "Tutti: el violín sube hasta el re agudo en ff; la viola y el violonchelo en trémolo debajo; un acorde de sol menor prestado antes de la cadencia, con ritardando.",
    },
    Section {
        bar: 39,
        bars: 4,
        name: "VI · Despedida",
        what: "El violín recuerda la segunda mitad del tema, piano. Se van retirando: primero el contrabajo, después la viola (cadencia rota a si menor).",
    },
    Section {
        bar: 43,
        bars: 4,
        name: "VII · Coda",
        what: "Violín y violonchelo solos, pianissimo, con ritardando: sol mayor a sol menor bajo un re tenido, y el acorde final —re en el violonchelo, fa♯ en el violín— sostenido hasta apagarse en la sala.",
    },
];

/// One instrument's part in one section: its notes, their base velocity, and its CC1 (the
/// dynamic) as `(beat from the section's start, 0…1)`, a straight line between points.
pub struct Part {
    pub who: Who,
    pub section: usize,
    pub vel: u8,
    pub notes: &'static str,
    pub cc1: &'static [(f64, f32)],
}

/// A note as written: beats from the clip's start.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub at: f64,
    pub len: f64,
    pub key: u8,
    pub vel: u8,
    pub art: Option<NoteArt>,
}

fn key_of(name: &str) -> Option<u8> {
    let mut c = name.chars();
    let pc: i32 = match c.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest: String = c.collect();
    let (acc, oct) = if let Some(r) = rest.strip_prefix('#') {
        (1, r)
    } else if let Some(r) = rest.strip_prefix('b') {
        (-1, r)
    } else {
        (0, rest.as_str())
    };
    let oct: i32 = oct.parse().ok()?;
    u8::try_from(12 * (oct + 1) + pc + acc).ok()
}

fn art(a: Articulation, bow: Option<Bow>) -> Option<NoteArt> {
    Some(NoteArt {
        articulation: a,
        bow,
    })
}

/// The part's notes, and its length in beats. Panics on a wrong bar line, a note out of
/// the instrument's range or a token it cannot read: the score is checked as it is built.
pub fn parse(part: &Part) -> (Vec<Written>, f64) {
    let (lo, hi) = part.who.range();
    let mut out: Vec<Written> = Vec::new();
    let mut at = 0.0f64;
    let mut slur = false;
    let mut first_in_slur = false;
    for (i, raw) in part.notes.split_whitespace().enumerate() {
        if raw == "|" {
            let off = at / BAR - (at / BAR).round();
            assert!(
                off.abs() < 1e-9,
                "{:?} section {}: a bar line at beat {at}",
                part.who,
                part.section
            );
            continue;
        }
        let mut tok = raw;
        if let Some(t) = tok.strip_prefix('(') {
            assert!(!slur, "{:?}: a slur inside a slur at {raw}", part.who);
            slur = true;
            first_in_slur = true;
            tok = t;
        }
        let closes = tok.ends_with(')');
        let tok = tok.trim_end_matches(')');
        let (name, rest) = tok
            .split_once(':')
            .unwrap_or_else(|| panic!("{:?}: no length in {raw}", part.who));
        let num_end = rest
            .find(|c: char| !(c.is_ascii_digit() || c == '.'))
            .unwrap_or(rest.len());
        let len: f64 = rest[..num_end]
            .parse()
            .unwrap_or_else(|_| panic!("{:?}: a length in {raw}", part.who));
        let flags = &rest[num_end..];
        if name == "r" {
            assert!(!slur, "{:?}: a rest inside a slur at {raw}", part.who);
            at += len;
            continue;
        }
        let key = key_of(name).unwrap_or_else(|| panic!("{:?}: a pitch in {raw}", part.who));
        assert!(
            (lo..=hi).contains(&key),
            "{:?}: {name} is outside its range",
            part.who
        );
        let bow = if flags.contains('d') {
            Some(Bow::Down)
        } else if flags.contains('u') {
            Some(Bow::Up)
        } else {
            None
        };
        let slurred = slur && !first_in_slur;
        let a = if flags.contains('p') {
            art(Articulation::Pizzicato, bow)
        } else if flags.contains('i') {
            art(Articulation::Spiccato, bow)
        } else if flags.contains('s') {
            art(Articulation::Staccato, bow)
        } else if flags.contains('t') {
            art(Articulation::Tremolo, bow)
        } else if slurred {
            art(Articulation::Legato, bow)
        } else if flags.contains('n') || bow.is_some() {
            art(Articulation::Sustain, bow)
        } else {
            None
        };
        // A slurred tremolo: the note before runs into this one, so the strokes go on.
        if slurred && flags.contains('t') {
            if let Some(prev) = out.last_mut() {
                prev.len += TREM_OVERLAP;
            }
        }
        // A player's velocities: the first note of a bow a little firmer, the slurred
        // ones lighter, an accent marked; a small fixed wobble so no two are alike.
        let wobble = [0i32, 2, -1, 1, -2, 1, 0, -1][i % 8];
        let mut v = i32::from(part.vel) + wobble;
        if slurred {
            v -= 4;
        }
        if flags.contains('>') {
            v += 14;
        }
        out.push(Written {
            at,
            len,
            key,
            vel: v.clamp(20, 127) as u8,
            art: a,
        });
        at += len;
        first_in_slur = false;
        if closes {
            slur = false;
        }
    }
    assert!(
        !slur,
        "{:?} section {}: a slur left open",
        part.who, part.section
    );
    (out, at)
}
