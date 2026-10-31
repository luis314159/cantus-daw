//! Chord symbols: `Fm9`, `Dbmaj7`, `Eb/G`, `Bb7sus4`, `C7(b9,#11)`, `Gm7b5`, `A°7`, `N.C.`.
//!
//! A [`Chord`] is a root, a bass (the root unless a slash names another) and its tones as
//! semitones above the root, each tagged with what it is (third, seventh, tension…) so a
//! voicing can drop the least essential first. The grammar is the lead sheet's, read
//! leniently: case matters only where it does on a lead sheet (`m` minor, `M` major
//! seventh), spaces, parentheses and commas are ignored.

use std::fmt;

/// What a chord tone is to its chord, in the order a voicing keeps them when it must drop
/// some (the third and the seventh say what the chord is; the perfect fifth says least).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// The third, or the second or fourth of a suspended chord.
    Third,
    /// The seventh, or the sixth of a sixth chord.
    Seventh,
    /// A flat or sharp fifth: part of the chord's colour, not to be dropped.
    AlteredFifth,
    /// A ninth, eleventh or thirteenth (natural or altered), or an added tone.
    Tension,
    Root,
    /// The perfect fifth.
    Fifth,
}

/// One tone of a chord.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Tone {
    /// Semitones above the root, 0…23 (a ninth is 14).
    pub interval: u8,
    pub role: Role,
}

/// A parsed chord symbol.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    /// The symbol as written (trimmed).
    pub symbol: String,
    /// Pitch class of the root, 0 = C … 11 = B.
    pub root: u8,
    /// Pitch class of the lowest note: the root, or the slash's note (`Eb/G` → G).
    pub bass: u8,
    /// The tones above the root, ascending by interval, one per pitch class.
    pub tones: Vec<Tone>,
}

/// Why a symbol is not a chord.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChordError {
    pub symbol: String,
    pub reason: String,
}

impl fmt::Display for ChordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "chord {:?}: {}",
            spell(&self.symbol),
            spell(&self.reason)
        )
    }
}

impl std::error::Error for ChordError {}

impl Chord {
    /// The chord's pitch classes as a 12-bit mask over C … B, the bass included.
    pub fn mask(&self) -> u16 {
        let mut m = 1u16 << self.bass;
        for t in &self.tones {
            m |= 1 << ((self.root + t.interval) % 12);
        }
        m
    }

    /// Whether pitch class `pc` (or any pitch, by its class) belongs to the chord.
    pub fn contains(&self, pitch: u8) -> bool {
        self.mask() & (1 << (pitch % 12)) != 0
    }

    /// The pitch class of each tone, most essential first (see [`Role`]); within a role,
    /// lowest interval first.
    pub fn by_priority(&self) -> Vec<u8> {
        let mut tones = self.tones.clone();
        tones.sort_by_key(|t| (t.role, t.interval));
        tones
            .iter()
            .map(|t| (self.root + t.interval) % 12)
            .collect()
    }

    /// The pitch class of the tone playing `role`, if the chord has one.
    pub fn tone(&self, role: Role) -> Option<u8> {
        self.tones
            .iter()
            .find(|t| t.role == role)
            .map(|t| (self.root + t.interval) % 12)
    }

    /// The perfect or altered fifth's pitch class, if the chord has one.
    pub fn fifth(&self) -> Option<u8> {
        self.tone(Role::Fifth)
            .or_else(|| self.tone(Role::AlteredFifth))
    }
}

/// The pitch class of a note letter.
pub(crate) fn letter_pc(c: char) -> Option<u8> {
    Some(match c.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    })
}

/// A note name's letter (as a pitch class), the accidentals' shift in semitones and the
/// text after them. The letter must be upper case, as on a lead sheet (`b` is a flat).
pub(crate) fn parse_name(t: &str) -> Option<(i32, i32, &str)> {
    let mut chars = t.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_uppercase() {
        return None;
    }
    let letter = letter_pc(first)? as i32;
    let mut shift = 0;
    let mut end = first.len_utf8();
    for (i, c) in chars {
        match c {
            '#' | '♯' => shift += 1,
            'b' | '♭' => shift -= 1,
            _ => {
                end = i;
                break;
            }
        }
        end = i + c.len_utf8();
    }
    Some((letter, shift, &t[end..]))
}

/// A note name's pitch class and the text after it: `Eb` → 3, `F#m7` → (6, `m7`).
pub fn parse_pc(t: &str) -> Option<(u8, &str)> {
    let (letter, shift, rest) = parse_name(t)?;
    Some(((letter + shift).rem_euclid(12) as u8, rest))
}

/// Parse a chord symbol. `N.C.` (no chord) is not a chord: the chart reads it.
pub fn parse_chord(symbol: &str) -> Result<Chord, ChordError> {
    let symbol = symbol.trim();
    let err = |reason: &str| ChordError {
        symbol: symbol.to_string(),
        reason: reason.to_string(),
    };
    let (main, bass_text) = match symbol.rsplit_once('/') {
        // `6/9` is a quality, not a slash.
        Some((m, b)) if !b.starts_with('9') => (m, Some(b)),
        _ => (symbol, None),
    };
    let (root, quality) = parse_pc(main).ok_or_else(|| err("no root: start with A–G"))?;
    let bass = match bass_text {
        Some(b) => {
            let (pc, rest) =
                parse_pc(b.trim()).ok_or_else(|| err("the bass after / is not a note"))?;
            if !rest.trim().is_empty() {
                return Err(err("text after the bass note"));
            }
            pc
        }
        None => root,
    };
    let q = Quality::parse(quality).map_err(|r| err(&r))?;
    Ok(Chord {
        symbol: symbol.to_string(),
        root,
        bass,
        tones: q.tones(),
    })
}

/// A chord's quality as it is read, before it becomes tones.
#[derive(Default)]
struct Quality {
    /// The third's interval, or the suspension's (2, 5); `None` for a power chord or `no3`.
    third: Option<u8>,
    fifth: Option<u8>,
    seventh: bool,
    major_seventh: bool,
    diminished: bool,
    half_diminished: bool,
    sixth: bool,
    tensions: Vec<u8>,
}

impl Quality {
    fn parse(text: &str) -> Result<Quality, String> {
        let s: String = text
            .chars()
            .filter(|c| !matches!(c, ' ' | '(' | ')' | ','))
            .collect();
        let mut q = Quality {
            third: Some(4),
            fifth: Some(7),
            ..Quality::default()
        };
        let mut i = 0;
        let mut first = true;
        while i < s.len() {
            let rest = &s[i..];
            let took = q.token(rest, first)?;
            if took == 0 {
                return Err(format!("cannot read {rest:?}"));
            }
            i += took;
            first = false;
        }
        Ok(q)
    }

    /// Read one token at the start of `r`; how many bytes it took (0: none matched).
    fn token(&mut self, r: &str, first: bool) -> Result<usize, String> {
        let starts = |p: &str| r.starts_with(p);
        let digit_after = |n: usize| r[n..].starts_with(|c: char| c.is_ascii_digit());
        // Major-seventh words: `maj7`, `M7`, `ma9`, `j7`, and the triangle (`CΔ`, `CΔ9`).
        for w in ["maj", "Maj", "MAJ", "Ma", "M"] {
            if starts(w) && (w != "Ma" || digit_after(2)) {
                self.major_seventh = true;
                return Ok(w.len());
            }
        }
        let n = triangle(r);
        if n > 0 {
            self.major_seventh = true;
            if !digit_after(n) {
                self.seventh = true;
            }
            return Ok(n);
        }
        if starts("ma") && digit_after(2) {
            self.major_seventh = true;
            return Ok(2);
        }
        for (w, third) in [("min", 3), ("mi", 3), ("m", 3)] {
            if starts(w) && !starts("maj") {
                self.third = Some(third);
                return Ok(w.len());
            }
        }
        if first && starts("-") {
            self.third = Some(3);
            return Ok(1);
        }
        for w in ["dim", "°", "o"] {
            if starts(w) && (w != "o" || first) {
                self.third = Some(3);
                self.fifth = Some(6);
                self.diminished = true;
                return Ok(w.len());
            }
        }
        for w in ["ø", "Ø"] {
            if starts(w) {
                self.third = Some(3);
                self.fifth = Some(6);
                self.seventh = true;
                self.half_diminished = true;
                let n = w.len();
                return Ok(if r[n..].starts_with('7') { n + 1 } else { n });
            }
        }
        if starts("aug") || (first && starts("+")) {
            self.fifth = Some(8);
            return Ok(if starts("aug") { 3 } else { 1 });
        }
        if starts("sus") {
            let after = &r[3..];
            let (third, n) = if after.starts_with('2') {
                (2, 4)
            } else if after.starts_with('4') {
                (5, 4)
            } else {
                (5, 3)
            };
            self.third = Some(third);
            return Ok(n);
        }
        if starts("alt") {
            self.seventh = true;
            self.fifth = Some(6);
            self.tensions.extend([13, 15, 20]);
            return Ok(3);
        }
        for (w, third) in [
            ("no3", true),
            ("omit3", true),
            ("no5", false),
            ("omit5", false),
        ] {
            if starts(w) {
                if third {
                    self.third = None;
                } else {
                    self.fifth = None;
                }
                return Ok(w.len());
            }
        }
        if starts("add") {
            let (iv, n) = alteration(&r[3..]).ok_or("add what? (add9, add11, add13, addb9…)")?;
            self.tensions.push(iv);
            return Ok(3 + n);
        }
        if starts("6/9") || starts("69") {
            self.sixth = true;
            self.tensions.push(14);
            return Ok(if starts("6/9") { 3 } else { 2 });
        }
        // An accidental and a degree: an alteration (`b9`, `#11`, `b5`, `+5`, `-9`).
        if r.starts_with(accidental) {
            if let Some((iv, n)) = alteration(r) {
                match iv {
                    6 => self.fifth = Some(6),
                    8 => self.fifth = Some(8),
                    _ => self.tensions.push(iv),
                }
                return Ok(n);
            }
        }
        let n = r.bytes().take_while(u8::is_ascii_digit).count();
        if n > 0 {
            match &r[..n] {
                "6" => self.sixth = true,
                "7" => self.seventh = true,
                "9" => {
                    self.seventh = true;
                    self.tensions.push(14);
                }
                "11" => {
                    self.seventh = true;
                    self.tensions.extend([14, 17]);
                }
                "13" => {
                    self.seventh = true;
                    self.tensions.extend([14, 21]);
                }
                "5" => self.third = None,
                "2" => self.third = Some(2),
                "4" => self.third = Some(5),
                other => return Err(format!("no chord has a {other}")),
            }
            return Ok(n);
        }
        Ok(0)
    }

    fn tones(&self) -> Vec<Tone> {
        let mut out = vec![Tone {
            interval: 0,
            role: Role::Root,
        }];
        let dominant_11 = self.third == Some(4)
            && self.seventh
            && !self.major_seventh
            && self.tensions.contains(&17);
        if let Some(t) = self.third {
            // A dominant eleventh drops its third (the eleventh is a semitone over it).
            if !dominant_11 {
                out.push(Tone {
                    interval: t,
                    role: Role::Third,
                });
            }
        }
        if let Some(f) = self.fifth {
            out.push(Tone {
                interval: f,
                role: if f == 7 {
                    Role::Fifth
                } else {
                    Role::AlteredFifth
                },
            });
        }
        if self.seventh {
            let iv = if self.major_seventh {
                11
            } else if self.diminished && !self.half_diminished {
                9
            } else {
                10
            };
            out.push(Tone {
                interval: iv,
                role: Role::Seventh,
            });
        }
        if self.sixth {
            out.push(Tone {
                interval: 9,
                role: Role::Seventh,
            });
        }
        for &iv in &self.tensions {
            out.push(Tone {
                interval: iv,
                role: Role::Tension,
            });
        }
        // One tone per pitch class, the most essential role kept.
        out.sort_by_key(|t| (t.interval % 12, t.role));
        out.dedup_by_key(|t| t.interval % 12);
        out.sort_by_key(|t| t.interval);
        out
    }
}

/// Whether `c` starts an alteration: `b`, `#` (or the signs `♭`, `♯`), `+`, `-`.
fn accidental(c: char) -> bool {
    matches!(c, 'b' | '♭' | '#' | '♯' | '+' | '-')
}

/// The bytes a major-seventh triangle takes at the start of `r`: `Δ`, or the increment sign
/// `∆` typed for it (0: none). Read, never shown — [`spell`] writes it `maj7`.
fn triangle(r: &str) -> usize {
    match r.chars().next() {
        Some(c @ ('Δ' | '∆')) => c.len_utf8(),
        _ => 0,
    }
}

/// A chord symbol as Cantus shows it, in the interface fonts' glyphs: `♭` as `b`, `♯` as `#`,
/// the triangle as `maj7` (`maj` before a degree: `CΔ9` is `Cmaj9`). The parser reads both
/// spellings; the fonts (IBM Plex) have no music signs (round 34, `font_coverage`).
pub fn spell(symbol: &str) -> String {
    let mut out = String::with_capacity(symbol.len());
    let mut chars = symbol.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '♭' => out.push('b'),
            '♯' => out.push('#'),
            'Δ' | '∆' => {
                out.push_str("maj");
                if !chars.peek().is_some_and(char::is_ascii_digit) {
                    out.push('7');
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// An accidental (or none) and a degree at the start of `r`: the interval above the root
/// and the bytes taken. Degrees 2/9, 4/11, 5, 6/13.
fn alteration(r: &str) -> Option<(u8, usize)> {
    let mut shift: i32 = 0;
    let mut n = 0;
    for c in r.chars() {
        match c {
            'b' | '♭' | '-' => shift -= 1,
            '#' | '♯' | '+' => shift += 1,
            _ => break,
        }
        n += c.len_utf8();
    }
    let digits = r[n..].bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let base: i32 = match &r[n..n + digits] {
        "2" | "9" => 14,
        "4" | "11" => 17,
        "5" => 7,
        "6" | "13" => 21,
        _ => return None,
    };
    let iv = base + shift;
    // A fifth's alteration stays a fifth (6, 8); the rest are tensions above the octave.
    let iv = if base == 7 { iv } else { iv.max(13) };
    Some((iv as u8, n + digits))
}
