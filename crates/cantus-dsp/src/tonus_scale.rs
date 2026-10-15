//! Tonus's targets: scales, the per-note states, and the choice of the note a sung pitch
//! is pulled to.

/// A scale, by index (the parameter `scale`). Keys are forever once shipped.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Scale {
    #[default]
    Chromatic,
    Major,
    Minor,
    HarmonicMinor,
    MelodicMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    Blues,
}

impl Scale {
    pub const ALL: [Scale; 13] = [
        Scale::Chromatic,
        Scale::Major,
        Scale::Minor,
        Scale::HarmonicMinor,
        Scale::MelodicMinor,
        Scale::Dorian,
        Scale::Phrygian,
        Scale::Lydian,
        Scale::Mixolydian,
        Scale::Locrian,
        Scale::MajorPentatonic,
        Scale::MinorPentatonic,
        Scale::Blues,
    ];

    pub fn from_index(i: usize) -> Scale {
        Scale::ALL.get(i).copied().unwrap_or_default()
    }

    /// The degrees, in semitones above the key.
    pub fn degrees(self) -> &'static [u8] {
        match self {
            Scale::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
            Scale::Major => &[0, 2, 4, 5, 7, 9, 11],
            Scale::Minor => &[0, 2, 3, 5, 7, 8, 10],
            Scale::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Scale::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            Scale::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Scale::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Scale::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Scale::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Scale::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Scale::MajorPentatonic => &[0, 2, 4, 7, 9],
            Scale::MinorPentatonic => &[0, 3, 5, 7, 10],
            Scale::Blues => &[0, 3, 5, 6, 7, 10],
        }
    }

    /// Whether pitch class `pc` (0 = C) is in this scale on `key`.
    pub fn contains(self, key: u8, pc: u8) -> bool {
        let d = (pc + 12 - key % 12) % 12;
        self.degrees().contains(&d)
    }
}

/// What one pitch class does (the parameters `n0` … `n11`, C … B).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum NoteState {
    /// As the scale says: a target if it is in the scale, removed if not.
    #[default]
    Scale,
    /// A target, whatever the scale.
    On,
    /// Never a target: a voice near it goes to the nearest note that is.
    Remove,
    /// A voice nearest this note is left alone.
    Bypass,
}

impl NoteState {
    pub fn from_index(i: usize) -> NoteState {
        match i {
            1 => NoteState::On,
            2 => NoteState::Remove,
            3 => NoteState::Bypass,
            _ => NoteState::Scale,
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }
}

/// What a pitch class finally is, once the scale and the overrides are read.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Allowed {
    #[default]
    Removed,
    Target,
    Bypass,
}

/// The twelve pitch classes' final states.
pub fn resolve(scale: Scale, key: u8, notes: &[NoteState; 12]) -> [Allowed; 12] {
    let mut out = [Allowed::Removed; 12];
    for (pc, o) in out.iter_mut().enumerate() {
        *o = match notes[pc] {
            NoteState::On => Allowed::Target,
            NoteState::Remove => Allowed::Removed,
            NoteState::Bypass => Allowed::Bypass,
            NoteState::Scale if scale.contains(key, pc as u8) => Allowed::Target,
            NoteState::Scale => Allowed::Removed,
        };
    }
    out
}

/// How much nearer a new note must be before the target changes, semitones: a voice
/// hovering on the boundary between two notes does not flip between them.
pub const HYSTERESIS: f32 = 0.12;

/// The note (an integer pitch in the same units as `p`) a voice at `p` is pulled to, and
/// whether it is a bypass note, given the current one (`current`, kept unless a new one is
/// nearer by more than [`HYSTERESIS`]). `None` when every class is removed.
pub fn choose(p: f32, allowed: &[Allowed; 12], current: Option<i32>) -> Option<(i32, bool)> {
    let base = p.round() as i32;
    let mut best: Option<(i32, f32)> = None;
    for o in -12..=12 {
        let n = base + o;
        if allowed[n.rem_euclid(12) as usize] == Allowed::Removed {
            continue;
        }
        let d = (p - n as f32).abs();
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((n, d));
        }
    }
    let (mut n, d) = best?;
    if let Some(c) = current {
        if c != n && allowed[c.rem_euclid(12) as usize] != Allowed::Removed {
            let dc = (p - c as f32).abs();
            if dc - d < HYSTERESIS {
                n = c;
            }
        }
    }
    Some((n, allowed[n.rem_euclid(12) as usize] == Allowed::Bypass))
}

/// The notes held on the slot's MIDI source, in the order they were pressed.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Held {
    notes: [u8; 32],
    len: usize,
}

impl Held {
    pub fn on(&mut self, pitch: u8) {
        self.off(pitch);
        if self.len == self.notes.len() {
            self.notes.copy_within(1.., 0);
            self.len -= 1;
        }
        self.notes[self.len] = pitch;
        self.len += 1;
    }

    pub fn off(&mut self, pitch: u8) {
        if let Some(i) = self.notes[..self.len].iter().position(|&n| n == pitch) {
            self.notes.copy_within(i + 1..self.len, i);
            self.len -= 1;
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The newest note held.
    pub fn last(&self) -> Option<u8> {
        self.len.checked_sub(1).map(|i| self.notes[i])
    }

    /// The pitch classes held, as twelve bits (bit 0 = C).
    pub fn classes(&self) -> u16 {
        self.notes[..self.len]
            .iter()
            .fold(0u16, |m, &n| m | (1 << (n % 12)))
    }

    /// The held pitch classes as targets, the rest removed.
    pub fn allowed(&self) -> [Allowed; 12] {
        let bits = self.classes();
        let mut out = [Allowed::Removed; 12];
        for (pc, o) in out.iter_mut().enumerate() {
            if bits & (1 << pc) != 0 {
                *o = Allowed::Target;
            }
        }
        out
    }
}
