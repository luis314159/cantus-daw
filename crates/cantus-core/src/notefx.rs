//! Note effects (round 31, ADR 0069): a track's chain of MIDI effects before its
//! instrument — an **arpeggiator**, **chords** and **scale** — FL's channel *Arp* and
//! *Chord*, Ableton's MIDI effects, Bitwig's note FX.
//!
//! This is the model: what a chain holds, its parameters (each with a key, a range and a
//! default, so automation and modulators reach every one of them like an effect's), and
//! the shapes and rates they choose from. The engine plays it
//! (`cantus_engine::notefx`): on clip notes and live input alike, sample-accurately, the
//! same offline as live. A track whose chain is empty — every track before schema 43 —
//! plays as it always did, to the bit.

use crate::scale::{Key, ScaleKind};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A note effect's id: unique in the project (`Project::alloc_id`), so automation names
/// it whatever its place in the chain.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NoteFxId(pub u64);

impl fmt::Display for NoteFxId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nf{}", self.0)
    }
}

/// The longest chain a track holds.
pub const MAX_NOTE_FX: usize = 8;

/// One note effect in a track's chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NoteEffect {
    pub id: NoteFxId,
    /// Off, it passes every note through untouched (and keeps its settings).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub enabled: bool,
    pub kind: NoteFxKind,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// What a note effect is, with its settings.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum NoteFxKind {
    Arp(Arp),
    Chord(Chord),
    Scale(ScaleFx),
}

/// Which kind, without its settings: what the picker offers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteFxType {
    Arp,
    Chord,
    Scale,
}

impl NoteFxType {
    pub const ALL: [NoteFxType; 3] = [NoteFxType::Arp, NoteFxType::Chord, NoteFxType::Scale];

    pub fn label_key(self) -> &'static str {
        match self {
            NoteFxType::Arp => "notefx.kind.arp",
            NoteFxType::Chord => "notefx.kind.chord",
            NoteFxType::Scale => "notefx.kind.scale",
        }
    }

    /// The kind at its defaults.
    pub fn make(self) -> NoteFxKind {
        match self {
            NoteFxType::Arp => NoteFxKind::Arp(Arp::default()),
            NoteFxType::Chord => NoteFxKind::Chord(Chord::default()),
            NoteFxType::Scale => NoteFxKind::Scale(ScaleFx::default()),
        }
    }
}

/// The arpeggiator's order.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ArpMode {
    #[default]
    Up,
    Down,
    UpDown,
    DownUp,
    /// In the order the notes were played.
    Played,
    /// Every held note at each step.
    Chord,
    /// A note drawn at each step, from the seed and the step: the same every time.
    Random,
}

impl ArpMode {
    pub const ALL: [ArpMode; 7] = [
        ArpMode::Up,
        ArpMode::Down,
        ArpMode::UpDown,
        ArpMode::DownUp,
        ArpMode::Played,
        ArpMode::Chord,
        ArpMode::Random,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            ArpMode::Up => "notefx.arp.up",
            ArpMode::Down => "notefx.arp.down",
            ArpMode::UpDown => "notefx.arp.up_down",
            ArpMode::DownUp => "notefx.arp.down_up",
            ArpMode::Played => "notefx.arp.played",
            ArpMode::Chord => "notefx.arp.chord",
            ArpMode::Random => "notefx.arp.random",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }
}

/// The arpeggiator's rates, longest first: a name and its length in ticks (960 a beat).
/// Whole numbers of ticks, so every step lands on a tick and the grid is exact.
pub const ARP_RATES: [(&str, i64); 20] = [
    ("1/1", 3840),
    ("1/2.", 2880),
    ("1/1T", 2560),
    ("1/2", 1920),
    ("1/4.", 1440),
    ("1/2T", 1280),
    ("1/4", 960),
    ("1/8.", 720),
    ("1/4T", 640),
    ("1/8", 480),
    ("1/16.", 360),
    ("1/8T", 320),
    ("1/16", 240),
    ("1/32.", 180),
    ("1/16T", 160),
    ("1/32", 120),
    ("1/64.", 90),
    ("1/32T", 80),
    ("1/64", 60),
    ("1/64T", 40),
];

/// 1/16, the arpeggiator's default rate.
pub const ARP_DEFAULT_RATE: u8 = 12;

/// The arpeggiator (FL's *Arp*, Ableton's *Arpeggiator*).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Arp {
    pub mode: ArpMode,
    /// 1…4 octaves the pattern climbs.
    pub octaves: u8,
    /// Index into [`ARP_RATES`]: synced to the tempo.
    pub rate: u8,
    /// How long each note sounds, as a share of a step (0.05…2: past 1 the notes overlap).
    pub gate: f32,
    /// MPC-style swing of every second step: 0.5 straight … 0.75.
    pub swing: f32,
    /// Latch: the notes keep playing once the keys are up, until a new chord is played.
    pub hold: bool,
    /// Every note at [`Self::velocity`] instead of the velocity it was played with.
    pub fixed_velocity: bool,
    pub velocity: u8,
    /// What *Random* draws from.
    pub seed: u32,
}

impl Default for Arp {
    fn default() -> Self {
        Arp {
            mode: ArpMode::Up,
            octaves: 1,
            rate: ARP_DEFAULT_RATE,
            gate: 0.8,
            swing: 0.5,
            hold: false,
            fixed_velocity: false,
            velocity: 100,
            seed: 1,
        }
    }
}

impl Arp {
    /// A step's length in ticks.
    pub fn rate_ticks(&self) -> i64 {
        ARP_RATES[(self.rate as usize).min(ARP_RATES.len() - 1)].1
    }
}

/// A chord's shape: the semitones above the played note, or the key's own chord.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChordShape {
    #[default]
    Major,
    Minor,
    Diminished,
    Augmented,
    Sus2,
    Sus4,
    Dominant7,
    Major7,
    Minor7,
    MinorMajor7,
    HalfDiminished7,
    Diminished7,
    Sixth,
    Minor6,
    Add9,
    MinorAdd9,
    Ninth,
    Major9,
    Minor9,
    Power,
    PowerOctave,
    Octave,
    /// The key's triad on the played note's degree (I, ii, iii … in a major key).
    ScaleTriad,
    /// The key's seventh chord on the played note's degree.
    ScaleSeventh,
}

impl ChordShape {
    pub const ALL: [ChordShape; 24] = [
        ChordShape::Major,
        ChordShape::Minor,
        ChordShape::Diminished,
        ChordShape::Augmented,
        ChordShape::Sus2,
        ChordShape::Sus4,
        ChordShape::Dominant7,
        ChordShape::Major7,
        ChordShape::Minor7,
        ChordShape::MinorMajor7,
        ChordShape::HalfDiminished7,
        ChordShape::Diminished7,
        ChordShape::Sixth,
        ChordShape::Minor6,
        ChordShape::Add9,
        ChordShape::MinorAdd9,
        ChordShape::Ninth,
        ChordShape::Major9,
        ChordShape::Minor9,
        ChordShape::Power,
        ChordShape::PowerOctave,
        ChordShape::Octave,
        ChordShape::ScaleTriad,
        ChordShape::ScaleSeventh,
    ];

    /// The semitones above the root; empty for the two that come from the key.
    pub fn intervals(self) -> &'static [i16] {
        match self {
            ChordShape::Major => &[0, 4, 7],
            ChordShape::Minor => &[0, 3, 7],
            ChordShape::Diminished => &[0, 3, 6],
            ChordShape::Augmented => &[0, 4, 8],
            ChordShape::Sus2 => &[0, 2, 7],
            ChordShape::Sus4 => &[0, 5, 7],
            ChordShape::Dominant7 => &[0, 4, 7, 10],
            ChordShape::Major7 => &[0, 4, 7, 11],
            ChordShape::Minor7 => &[0, 3, 7, 10],
            ChordShape::MinorMajor7 => &[0, 3, 7, 11],
            ChordShape::HalfDiminished7 => &[0, 3, 6, 10],
            ChordShape::Diminished7 => &[0, 3, 6, 9],
            ChordShape::Sixth => &[0, 4, 7, 9],
            ChordShape::Minor6 => &[0, 3, 7, 9],
            ChordShape::Add9 => &[0, 4, 7, 14],
            ChordShape::MinorAdd9 => &[0, 3, 7, 14],
            ChordShape::Ninth => &[0, 4, 7, 10, 14],
            ChordShape::Major9 => &[0, 4, 7, 11, 14],
            ChordShape::Minor9 => &[0, 3, 7, 10, 14],
            ChordShape::Power => &[0, 7],
            ChordShape::PowerOctave => &[0, 7, 12],
            ChordShape::Octave => &[0, 12],
            ChordShape::ScaleTriad | ChordShape::ScaleSeventh => &[],
        }
    }

    /// How many voices a chord from the key stacks (0 for a fixed shape).
    pub fn scale_voices(self) -> usize {
        match self {
            ChordShape::ScaleTriad => 3,
            ChordShape::ScaleSeventh => 4,
            _ => 0,
        }
    }

    /// The short name the menus print (chord symbols are not translated: "m7", "sus4").
    pub fn symbol(self) -> &'static str {
        match self {
            ChordShape::Major => "maj",
            ChordShape::Minor => "m",
            ChordShape::Diminished => "dim",
            ChordShape::Augmented => "aug",
            ChordShape::Sus2 => "sus2",
            ChordShape::Sus4 => "sus4",
            ChordShape::Dominant7 => "7",
            ChordShape::Major7 => "maj7",
            ChordShape::Minor7 => "m7",
            ChordShape::MinorMajor7 => "m(maj7)",
            ChordShape::HalfDiminished7 => "m7b5",
            ChordShape::Diminished7 => "dim7",
            ChordShape::Sixth => "6",
            ChordShape::Minor6 => "m6",
            ChordShape::Add9 => "add9",
            ChordShape::MinorAdd9 => "m(add9)",
            ChordShape::Ninth => "9",
            ChordShape::Major9 => "maj9",
            ChordShape::Minor9 => "m9",
            ChordShape::Power => "5",
            ChordShape::PowerOctave => "5+8",
            ChordShape::Octave => "8va",
            ChordShape::ScaleTriad | ChordShape::ScaleSeventh => "",
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }
}

/// Where a chord's or a scale's key comes from.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeyChoice {
    /// Its own key instead of the project's.
    pub own: bool,
    pub root: u8,
    pub scale: ScaleKind,
}

impl KeyChoice {
    /// The key in force, given the project's (`None` while neither says one).
    pub fn resolve(&self, project: Option<Key>) -> Option<Key> {
        if self.own {
            Some(Key::new(self.root, self.scale))
        } else {
            project
        }
    }
}

/// Chords from single notes (FL's *Chord*, Ableton's *Chord*).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Chord {
    pub shape: ChordShape,
    /// 0…3: how many of the lowest voices move up an octave.
    pub inversion: u8,
    /// 0 close, 1 open (the second voice up an octave), 2 wide (open, the root an
    /// octave down).
    pub spread: u8,
    /// The key the two key shapes read.
    pub key: KeyChoice,
}

impl Chord {
    /// The chord on `pitch` in key `key`, as pitches into `out` (lowest first, each once,
    /// inside 0…127): how many.
    pub fn voice(&self, pitch: u8, key: Option<Key>, out: &mut [u8; 8]) -> usize {
        let mut iv = [0i16; 6];
        let mut root = pitch as i16;
        let n = match (self.shape.scale_voices(), self.key.resolve(key)) {
            (0, _) => {
                let s = self.shape.intervals();
                iv[..s.len()].copy_from_slice(s);
                s.len()
            }
            (v, Some(k)) => {
                root = k.snap(pitch) as i16;
                k.diatonic(pitch, v, &mut iv)
            }
            // A key shape with no key: the note alone.
            (_, None) => 1,
        };
        let mut v = [0i16; 8];
        for i in 0..n {
            v[i] = root + iv[i];
        }
        let inv = (self.inversion as usize).min(n.saturating_sub(1));
        for x in v.iter_mut().take(inv) {
            *x += 12;
        }
        v[..n].sort_unstable();
        if self.spread >= 1 && n >= 3 {
            v[1] += 12;
        }
        if self.spread >= 2 && n >= 2 {
            v[0] -= 12;
        }
        v[..n].sort_unstable();
        let mut k = 0;
        for &p in &v[..n] {
            if (0..=127).contains(&p) && !out[..k].contains(&(p as u8)) {
                out[k] = p as u8;
                k += 1;
            }
        }
        k
    }
}

/// What the scale effect does with a note outside the key.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ScaleMode {
    /// Moved to the key's nearest note (ties down).
    #[default]
    Snap,
    /// Not played.
    Drop,
}

/// Notes kept in a key (Ableton's *Scale*).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ScaleFx {
    pub mode: ScaleMode,
    pub key: KeyChoice,
}

impl ScaleFx {
    /// What `pitch` becomes in `key` (the project's): `None` dropped. No key at all:
    /// played as it is.
    pub fn map(&self, pitch: u8, key: Option<Key>) -> Option<u8> {
        let Some(k) = self.key.resolve(key) else {
            return Some(pitch);
        };
        match self.mode {
            ScaleMode::Snap => Some(k.snap(pitch)),
            ScaleMode::Drop => k.contains(pitch).then_some(pitch),
        }
    }
}

/// How a parameter's value reads.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NoteFxUnit {
    /// A choice among names (`choice_text`).
    Choice,
    /// A whole number.
    Count,
    /// A share shown in per cent (0.8 → 80 %).
    Percent,
    /// A MIDI velocity, 1…127.
    Velocity,
    /// Off / on.
    Switch,
}

/// One parameter of a note effect: what automation, modulators and the editor read.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct NoteFxParam {
    pub key: &'static str,
    pub label_key: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub stepped: bool,
    pub unit: NoteFxUnit,
}

const fn param(
    key: &'static str,
    label_key: &'static str,
    min: f32,
    max: f32,
    default: f32,
    stepped: bool,
    unit: NoteFxUnit,
) -> NoteFxParam {
    NoteFxParam {
        key,
        label_key,
        min,
        max,
        default,
        stepped,
        unit,
    }
}

use NoteFxUnit as U;

const ARP_PARAMS: [NoteFxParam; 9] = [
    param("mode", "notefx.p.mode", 0.0, 6.0, 0.0, true, U::Choice),
    param("octaves", "notefx.p.octaves", 1.0, 4.0, 1.0, true, U::Count),
    param(
        "rate",
        "notefx.p.rate",
        0.0,
        (ARP_RATES.len() - 1) as f32,
        ARP_DEFAULT_RATE as f32,
        true,
        U::Choice,
    ),
    param("gate", "notefx.p.gate", 0.05, 2.0, 0.8, false, U::Percent),
    param("swing", "notefx.p.swing", 0.5, 0.75, 0.5, false, U::Percent),
    param("hold", "notefx.p.hold", 0.0, 1.0, 0.0, true, U::Switch),
    param("fixed", "notefx.p.fixed", 0.0, 1.0, 0.0, true, U::Switch),
    param(
        "velocity",
        "notefx.p.velocity",
        1.0,
        127.0,
        100.0,
        true,
        U::Velocity,
    ),
    param("seed", "notefx.p.seed", 0.0, 999.0, 1.0, true, U::Count),
];

const KEY_PARAMS: [NoteFxParam; 3] = [
    param(
        "own_key",
        "notefx.p.own_key",
        0.0,
        1.0,
        0.0,
        true,
        U::Switch,
    ),
    param("root", "notefx.p.root", 0.0, 11.0, 0.0, true, U::Choice),
    param(
        "scale",
        "notefx.p.scale",
        0.0,
        (ScaleKind::ALL.len() - 1) as f32,
        0.0,
        true,
        U::Choice,
    ),
];

const CHORD_PARAMS: [NoteFxParam; 6] = [
    param(
        "shape",
        "notefx.p.shape",
        0.0,
        (ChordShape::ALL.len() - 1) as f32,
        0.0,
        true,
        U::Choice,
    ),
    param(
        "inversion",
        "notefx.p.inversion",
        0.0,
        3.0,
        0.0,
        true,
        U::Count,
    ),
    param("spread", "notefx.p.spread", 0.0, 2.0, 0.0, true, U::Choice),
    KEY_PARAMS[0],
    KEY_PARAMS[1],
    KEY_PARAMS[2],
];

const SCALE_PARAMS: [NoteFxParam; 4] = [
    param(
        "mode",
        "notefx.p.scale_mode",
        0.0,
        1.0,
        0.0,
        true,
        U::Choice,
    ),
    KEY_PARAMS[0],
    KEY_PARAMS[1],
    KEY_PARAMS[2],
];

impl NoteFxKind {
    pub fn kind(&self) -> NoteFxType {
        match self {
            NoteFxKind::Arp(_) => NoteFxType::Arp,
            NoteFxKind::Chord(_) => NoteFxType::Chord,
            NoteFxKind::Scale(_) => NoteFxType::Scale,
        }
    }

    /// Every parameter, in the editor's order.
    pub fn params(&self) -> &'static [NoteFxParam] {
        Self::params_of(self.kind())
    }

    pub fn params_of(kind: NoteFxType) -> &'static [NoteFxParam] {
        match kind {
            NoteFxType::Arp => &ARP_PARAMS,
            NoteFxType::Chord => &CHORD_PARAMS,
            NoteFxType::Scale => &SCALE_PARAMS,
        }
    }

    /// The parameter called `key`.
    pub fn param(&self, key: &str) -> Option<&'static NoteFxParam> {
        self.params().iter().find(|p| p.key == key)
    }

    /// Its index among [`Self::params`] (what the engine addresses it by).
    pub fn param_index(&self, key: &str) -> Option<usize> {
        self.params().iter().position(|p| p.key == key)
    }

    /// The value of parameter `key`, in its own units.
    pub fn get(&self, key: &str) -> Option<f32> {
        self.param_index(key).map(|i| self.get_index(i))
    }

    /// The value of the parameter at `i` of [`Self::params`] (0 past the end).
    pub fn get_index(&self, i: usize) -> f32 {
        let key_of = |k: &KeyChoice, j: usize| match j {
            0 => k.own as u8 as f32,
            1 => k.root as f32,
            _ => k.scale.index() as f32,
        };
        match self {
            NoteFxKind::Arp(a) => match i {
                0 => a.mode.index() as f32,
                1 => a.octaves as f32,
                2 => a.rate as f32,
                3 => a.gate,
                4 => a.swing,
                5 => a.hold as u8 as f32,
                6 => a.fixed_velocity as u8 as f32,
                7 => a.velocity as f32,
                8 => a.seed as f32,
                _ => 0.0,
            },
            NoteFxKind::Chord(c) => match i {
                0 => c.shape.index() as f32,
                1 => c.inversion as f32,
                2 => c.spread as f32,
                3..=5 => key_of(&c.key, i - 3),
                _ => 0.0,
            },
            NoteFxKind::Scale(s) => match i {
                0 => (s.mode == ScaleMode::Drop) as u8 as f32,
                1..=3 => key_of(&s.key, i - 1),
                _ => 0.0,
            },
        }
    }

    /// Set parameter `key` (clamped to its range, a stepped one rounded); `false` for a
    /// key it does not have or a value that is not a number.
    pub fn set(&mut self, key: &str, v: f32) -> bool {
        match self.param_index(key) {
            Some(i) if v.is_finite() => {
                self.set_index(i, v);
                true
            }
            _ => false,
        }
    }

    /// Set the parameter at `i` of [`Self::params`] (clamped and rounded as [`Self::set`]).
    /// Allocation-free: the engine's automation writes through it.
    pub fn set_index(&mut self, i: usize, v: f32) {
        let Some(p) = self.params().get(i) else {
            return;
        };
        if !v.is_finite() {
            return;
        }
        let mut v = v.clamp(p.min, p.max);
        if p.stepped {
            v = v.round();
        }
        let n = v as usize;
        let set_key = |k: &mut KeyChoice, j: usize| match j {
            0 => k.own = n != 0,
            1 => k.root = n as u8,
            _ => k.scale = ScaleKind::from_index(n),
        };
        match self {
            NoteFxKind::Arp(a) => match i {
                0 => a.mode = ArpMode::ALL[n.min(ArpMode::ALL.len() - 1)],
                1 => a.octaves = n as u8,
                2 => a.rate = n as u8,
                3 => a.gate = v,
                4 => a.swing = v,
                5 => a.hold = n != 0,
                6 => a.fixed_velocity = n != 0,
                7 => a.velocity = n as u8,
                _ => a.seed = n as u32,
            },
            NoteFxKind::Chord(c) => match i {
                0 => c.shape = ChordShape::ALL[n.min(ChordShape::ALL.len() - 1)],
                1 => c.inversion = n as u8,
                2 => c.spread = n as u8,
                _ => set_key(&mut c.key, i - 3),
            },
            NoteFxKind::Scale(s) => match i {
                0 => {
                    s.mode = if n == 0 {
                        ScaleMode::Snap
                    } else {
                        ScaleMode::Drop
                    }
                }
                _ => set_key(&mut s.key, i - 1),
            },
        }
    }

    /// Whether every value is inside its range (a hand-edited file's may not be).
    pub fn is_valid(&self) -> bool {
        self.params().iter().enumerate().all(|(i, p)| {
            let v = self.get_index(i);
            v.is_finite() && v >= p.min && v <= p.max
        })
    }
}

/// The words of a choice parameter's value: an i18n key (`Ok`) or a literal that is the
/// same in every language (`Err`: a rate, a chord symbol, a note name).
pub fn choice_text(kind: NoteFxType, key: &str, v: f32) -> Result<&'static str, &'static str> {
    let n = v.round().max(0.0) as usize;
    match (kind, key) {
        (NoteFxType::Arp, "mode") => Ok(ArpMode::ALL[n.min(6)].label_key()),
        (NoteFxType::Arp, "rate") => Err(ARP_RATES[n.min(ARP_RATES.len() - 1)].0),
        (NoteFxType::Chord, "shape") => {
            let s = ChordShape::ALL[n.min(ChordShape::ALL.len() - 1)];
            match s {
                ChordShape::ScaleTriad => Ok("notefx.chord.scale_triad"),
                ChordShape::ScaleSeventh => Ok("notefx.chord.scale_seventh"),
                _ => Err(s.symbol()),
            }
        }
        (NoteFxType::Chord, "spread") => Ok(match n {
            0 => "notefx.spread.close",
            1 => "notefx.spread.open",
            _ => "notefx.spread.wide",
        }),
        (NoteFxType::Scale, "mode") => Ok(if n == 0 {
            "notefx.scale_mode.snap"
        } else {
            "notefx.scale_mode.drop"
        }),
        (_, "root") => Err(crate::scale::ROOT_NAMES[n.min(11)]),
        (_, "scale") => Ok(ScaleKind::from_index(n).label_key()),
        _ => Err(""),
    }
}

impl NoteEffect {
    pub fn new(id: NoteFxId, kind: NoteFxType) -> NoteEffect {
        NoteEffect {
            id,
            enabled: true,
            kind: kind.make(),
        }
    }
}

/// Whether a chain is well formed: no more than [`MAX_NOTE_FX`], ids each once, values in
/// range.
pub fn chain_is_valid(chain: &[NoteEffect]) -> bool {
    chain.len() <= MAX_NOTE_FX
        && chain.iter().all(|e| e.kind.is_valid())
        && chain
            .iter()
            .enumerate()
            .all(|(i, e)| chain[..i].iter().all(|o| o.id != e.id))
}

/// Whether a chain reads the project's key: a scale, or a chord from the key, on the
/// project's key (what makes a key change something to hear).
pub fn reads_project_key(chain: &[NoteEffect]) -> bool {
    chain.iter().any(|e| match &e.kind {
        NoteFxKind::Scale(s) => !s.key.own,
        NoteFxKind::Chord(c) => !c.key.own && c.shape.scale_voices() > 0,
        NoteFxKind::Arp(_) => false,
    })
}

/// Note effect `effect` of `track`'s chain.
pub fn find(
    project: &crate::model::Project,
    track: crate::model::TrackId,
    effect: NoteFxId,
) -> Option<&NoteEffect> {
    project
        .track(track)?
        .note_fx
        .iter()
        .find(|e| e.id == effect)
}

/// A chain with fresh ids from `ids`, for a copy of its track (a take exploded to a track of
/// its own, a track template inserted): ids are the project's, one owner each.
pub fn renew_ids(chain: &mut [NoteEffect], mut ids: impl FnMut() -> u64) {
    for e in chain {
        e.id = NoteFxId(ids());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_parameter_reads_back_what_was_set() {
        for t in NoteFxType::ALL {
            let mut k = t.make();
            for (i, p) in k.params().iter().enumerate() {
                assert_eq!(k.get_index(i), p.default, "{t:?} {} default", p.key);
                k.set_index(i, p.max);
                assert_eq!(k.get_index(i), p.max, "{t:?} {}", p.key);
                k.set_index(i, p.min);
                assert_eq!(k.get_index(i), p.min, "{t:?} {}", p.key);
                // Past the range: clamped.
                assert!(k.set(p.key, p.max + 100.0));
                assert_eq!(k.get(p.key), Some(p.max));
                assert!(!k.set(p.key, f32::NAN));
            }
            assert!(!k.set("nope", 1.0));
            assert!(k.is_valid());
        }
    }

    #[test]
    fn chords_invert_spread_and_stay_in_range() {
        let mut out = [0u8; 8];
        let mut c = Chord::default();
        assert_eq!(c.voice(60, None, &mut out), 3);
        assert_eq!(&out[..3], &[60, 64, 67]);
        c.inversion = 1; // E G C
        c.voice(60, None, &mut out);
        assert_eq!(&out[..3], &[64, 67, 72]);
        c.inversion = 2; // G C E
        c.voice(60, None, &mut out);
        assert_eq!(&out[..3], &[67, 72, 76]);
        c.inversion = 3; // clamped to 2 for a triad
        c.voice(60, None, &mut out);
        assert_eq!(&out[..3], &[67, 72, 76]);
        c.inversion = 0;
        c.spread = 1; // C G E'
        c.voice(60, None, &mut out);
        assert_eq!(&out[..3], &[60, 67, 76]);
        c.spread = 2; // C, G E'
        c.voice(60, None, &mut out);
        assert_eq!(&out[..3], &[48, 67, 76]);
        // At the top of the keyboard the voices past 127 are left out.
        c.spread = 0;
        assert_eq!(c.voice(125, None, &mut out), 1);
        // The key's chords.
        let key = Some(Key::new(0, ScaleKind::Major));
        let d = Chord {
            shape: ChordShape::ScaleTriad,
            ..Chord::default()
        };
        d.voice(62, key, &mut out);
        assert_eq!(&out[..3], &[62, 65, 69]);
        // A key shape with no key plays the note alone.
        assert_eq!(d.voice(62, None, &mut out), 1);
        // Its own key wins over the project's.
        let own = Chord {
            shape: ChordShape::ScaleSeventh,
            key: KeyChoice {
                own: true,
                root: 9,
                scale: ScaleKind::Minor,
            },
            ..Chord::default()
        };
        assert_eq!(own.voice(57, key, &mut out), 4);
        assert_eq!(&out[..4], &[57, 60, 64, 67]); // Am7
    }

    #[test]
    fn the_scale_snaps_or_drops_and_passes_without_a_key() {
        let key = Some(Key::new(0, ScaleKind::Major));
        let mut s = ScaleFx::default();
        assert_eq!(s.map(61, key), Some(60));
        assert_eq!(s.map(61, None), Some(61));
        s.mode = ScaleMode::Drop;
        assert_eq!(s.map(61, key), None);
        assert_eq!(s.map(62, key), Some(62));
    }

    #[test]
    fn a_chain_is_valid_with_unique_ids_and_in_range() {
        let a = NoteEffect::new(NoteFxId(1), NoteFxType::Arp);
        let mut b = NoteEffect::new(NoteFxId(1), NoteFxType::Chord);
        assert!(!chain_is_valid(&[a.clone(), b.clone()]));
        b.id = NoteFxId(2);
        assert!(chain_is_valid(&[a.clone(), b]));
        let mut bad = a;
        if let NoteFxKind::Arp(x) = &mut bad.kind {
            x.octaves = 9;
        }
        assert!(!chain_is_valid(&[bad]));
    }

    #[test]
    fn the_rates_are_longest_first_and_whole_ticks() {
        assert!(ARP_RATES.windows(2).all(|w| w[0].1 > w[1].1));
        assert_eq!(ARP_RATES[ARP_DEFAULT_RATE as usize], ("1/16", 240));
    }
}
