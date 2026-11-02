//! Our sixteen moves ↔ a file's animations (ADR 0089 §4): guessed by name in English and
//! Spanish, every one editable on the card, kept in `character.json` beside the copy.

use cantus_core::inst::dancer::{Mood, Move};

/// One animation a file holds: a Lottie marker, a sheet's tag, a DragonBones animation.
#[derive(Clone, Debug, PartialEq)]
pub struct Clip {
    pub name: String,
    /// Its natural length at the file's own pace, in seconds.
    pub seconds: f32,
    /// How many frames it has (for the card).
    pub frames: u32,
}

/// The words that name each move, English then Spanish (accents dropped, as names are).
pub const MOVE_WORDS: [(Move, &[&str]); 16] = [
    (
        Move::Idle,
        &[
            "idle", "stand", "rest", "breathe", "wait", "reposo", "quieto", "parado", "esperar",
            "respirar",
        ],
    ),
    (
        Move::Bounce,
        &[
            "bounce", "dance", "groove", "bob", "nod", "rebote", "bailar", "baile",
        ],
    ),
    (
        Move::StepLeft,
        &[
            "step_left",
            "left",
            "walk_left",
            "paso_izquierda",
            "izquierda",
        ],
    ),
    (
        Move::StepRight,
        &[
            "step_right",
            "right",
            "walk",
            "walk_right",
            "paso_derecha",
            "derecha",
            "caminar",
        ],
    ),
    (Move::Sway, &["sway", "rock", "balanceo", "mecer", "vaiven"]),
    (
        Move::Clap,
        &[
            "clap", "applause", "aplauso", "aplaudir", "palmas", "palmada",
        ],
    ),
    (
        Move::Wave,
        &["wave", "hello", "hi", "greet", "saludo", "saludar", "hola"],
    ),
    (
        Move::Jump,
        &["jump", "hop", "leap", "salto", "saltar", "brinco"],
    ),
    (
        Move::Spin,
        &[
            "spin", "turn", "rotate", "giro", "girar", "vuelta", "pirueta",
        ],
    ),
    (Move::Kick, &["kick", "patada"]),
    (Move::Shuffle, &["shuffle", "run", "correr", "pasito"]),
    (Move::Point, &["point", "senalar", "apuntar"]),
    (
        Move::ArmsUp,
        &[
            "arms_up",
            "cheer",
            "hands_up",
            "celebrate",
            "victory",
            "brazos_arriba",
            "celebrar",
            "festejar",
        ],
    ),
    (Move::Twist, &["twist", "torsion"]),
    (Move::Bow, &["bow", "curtsy", "reverencia"]),
    (
        Move::Freeze,
        &["freeze", "pose", "stop", "congelar", "congelado"],
    ),
];

/// The words that name each mood (Neutral has none: it is Idle's own).
pub const MOOD_WORDS: [(Mood, &[&str]); 7] = [
    (Mood::Happy, &["happy", "feliz", "contento"]),
    (Mood::Excited, &["excited", "emocionado"]),
    (Mood::Calm, &["calm", "tranquilo"]),
    (Mood::Surprised, &["surprised", "sorprendido"]),
    (Mood::Sleepy, &["sleepy", "dormido", "sueno"]),
    (Mood::Cool, &["cool", "genial"]),
    (Mood::Shy, &["shy", "timido"]),
];

/// A name as words are compared: lower case, accents dropped, every run of separators
/// (`-`, `_`, space, `.`) one `_`, digits at the end dropped (`clap_02` is `clap`).
pub fn normalise(name: &str) -> String {
    let mut out = String::new();
    let mut sep = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            c => c,
        };
        if c.is_alphanumeric() {
            if sep && !out.is_empty() {
                out.push('_');
            }
            sep = false;
            out.push(c);
        } else {
            sep = true;
        }
    }
    let trimmed = out.trim_end_matches(|c: char| c.is_ascii_digit() || c == '_');
    if trimmed.is_empty() {
        out
    } else {
        trimmed.to_string()
    }
}

fn matches(name: &str, words: &[&str]) -> u8 {
    let n = normalise(name);
    if words.contains(&n.as_str()) {
        return 2; // the whole name
    }
    if n.split('_').any(|w| words.contains(&w)) {
        return 1; // one of its words
    }
    0
}

/// The move a name says, if it says one.
pub fn move_of(name: &str) -> Option<Move> {
    let mut best = None;
    for (m, words) in MOVE_WORDS {
        let s = matches(name, words);
        if s > 0 && best.is_none_or(|(_, b)| s > b) {
            best = Some((m, s));
        }
    }
    best.map(|(m, _)| m)
}

/// The mood a name says, if it says one.
pub fn mood_of(name: &str) -> Option<Mood> {
    MOOD_WORDS
        .iter()
        .find(|(_, w)| matches(name, w) == 2)
        .map(|(m, _)| *m)
}

/// The beats a clip spans: its length at 120 BPM rounded to 1, 2, 4 or 8.
pub fn natural_beats(seconds: f32) -> f32 {
    let beats = seconds * 2.0;
    [1.0f32, 2.0, 4.0, 8.0]
        .into_iter()
        .min_by(|a, b| {
            let (da, db) = ((beats / a).max(a / beats), (beats / b).max(b / beats));
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(1.0)
}

/// Which clip each move plays, and over how many beats.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Mapping {
    /// Per move, in `Move::ALL`'s order: the clip's index, or none.
    pub moves: Vec<Option<usize>>,
    /// Per move: its beats at speed 1.
    pub beats: Vec<f32>,
    /// Per mood (`Mood::ALL`'s order): a clip played in Idle's place.
    pub moods: Vec<Option<usize>>,
    /// Whether each move's guess came from its name (the card says "guessed").
    #[serde(default)]
    pub guessed: Vec<bool>,
}

impl Mapping {
    /// Guessed from the clips' names. A clip names at most one move by its whole name
    /// first; a move nobody names stays empty (its fallback plays).
    pub fn guess(clips: &[Clip]) -> Mapping {
        let mut moves = vec![None; 16];
        let mut guessed = vec![false; 16];
        let mut beats = vec![1.0f32; 16];
        for (i, m) in Move::ALL.iter().enumerate() {
            beats[i] = default_beats(*m);
        }
        // Whole-name matches first, then word matches, the first clip winning a move.
        for pass in [2u8, 1] {
            for (ci, c) in clips.iter().enumerate() {
                for (m, words) in MOVE_WORDS {
                    let k = m.index();
                    if moves[k].is_none() && matches(&c.name, words) == pass {
                        moves[k] = Some(ci);
                        guessed[k] = true;
                        beats[k] = natural_beats(c.seconds);
                        break;
                    }
                }
            }
        }
        let mut moods = vec![None; Mood::ALL.len()];
        for (ci, c) in clips.iter().enumerate() {
            if let Some(md) = mood_of(&c.name) {
                let k = md.index();
                if moods[k].is_none() {
                    moods[k] = Some(ci);
                }
            }
        }
        Mapping {
            moves,
            beats,
            moods,
            guessed,
        }
    }

    /// The clip `m` plays: its own, else its fallbacks', else the first clip.
    pub fn clip_for(&self, m: Move, clips: usize) -> Option<usize> {
        let mut m = m;
        for _ in 0..5 {
            if let Some(Some(c)) = self.moves.get(m.index()) {
                if *c < clips {
                    return Some(*c);
                }
            }
            m = m.fallback();
        }
        (clips > 0).then_some(0)
    }

    pub fn has(&self, m: Move, clips: usize) -> bool {
        matches!(self.moves.get(m.index()), Some(Some(c)) if *c < clips)
    }

    pub fn beats_of(&self, m: Move) -> f32 {
        self.beats
            .get(m.index())
            .copied()
            .filter(|b| b.is_finite() && *b > 0.0)
            .map(|b| b.clamp(0.25, 64.0))
            .unwrap_or_else(|| default_beats(m))
    }

    pub fn mood_clip(&self, md: Mood, clips: usize) -> Option<usize> {
        self.moods
            .get(md.index())
            .copied()
            .flatten()
            .filter(|c| *c < clips)
    }

    /// Made whole after reading it from a file someone could have edited.
    pub fn sanitised(mut self, clips: usize) -> Mapping {
        self.moves.resize(16, None);
        self.beats.resize(16, 1.0);
        self.moods.resize(Mood::ALL.len(), None);
        self.guessed.resize(16, false);
        for m in self.moves.iter_mut().chain(self.moods.iter_mut()) {
            if m.is_some_and(|c| c >= clips) {
                *m = None;
            }
        }
        for (i, b) in self.beats.iter_mut().enumerate() {
            if !(b.is_finite() && *b > 0.0) {
                *b = default_beats(Move::from_index(i));
            }
            *b = b.clamp(0.25, 64.0);
        }
        self
    }
}

/// A move's beats when nothing better is known (the trait's default).
pub fn default_beats(m: Move) -> f32 {
    match m {
        Move::Spin | Move::Bow | Move::Twist | Move::Wave => 2.0,
        Move::Freeze | Move::Idle => 4.0,
        _ => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(name: &str, seconds: f32) -> Clip {
        Clip {
            name: name.into(),
            seconds,
            frames: 10,
        }
    }

    #[test]
    fn every_word_of_every_move_is_guessed_in_both_languages() {
        for (m, words) in MOVE_WORDS {
            for w in words {
                assert_eq!(move_of(w), Some(m), "{w}");
                // As files spell them.
                assert_eq!(move_of(&format!("{}_01", w.to_uppercase())), Some(m), "{w}");
            }
        }
        assert_eq!(move_of("Señalar"), Some(Move::Point));
        assert_eq!(move_of("Brazos arriba"), Some(Move::ArmsUp));
        assert_eq!(move_of("Torsión"), Some(Move::Twist));
        assert_eq!(move_of("big-jump"), Some(Move::Jump));
        assert_eq!(move_of("xyzzy"), None);
        for (md, words) in MOOD_WORDS {
            for w in words {
                assert_eq!(mood_of(w), Some(md), "{w}");
            }
        }
        assert_eq!(mood_of("Tímido"), Some(Mood::Shy));
    }

    #[test]
    fn the_guess_maps_names_and_beats() {
        let clips = [
            clip("Idle", 2.0),
            clip("clap", 0.5),
            clip("Saludo", 1.0),
            clip("happy", 1.0),
            clip("walk_left", 0.5),
        ];
        let m = Mapping::guess(&clips);
        assert_eq!(m.moves[Move::Idle.index()], Some(0));
        assert_eq!(m.moves[Move::Clap.index()], Some(1));
        assert_eq!(m.moves[Move::Wave.index()], Some(2));
        assert_eq!(m.moves[Move::StepLeft.index()], Some(4));
        assert_eq!(m.moves[Move::Jump.index()], None);
        assert_eq!(m.moods[Mood::Happy.index()], Some(3));
        assert_eq!(m.beats_of(Move::Clap), 1.0);
        assert_eq!(m.beats_of(Move::Wave), 2.0);
        assert_eq!(m.beats_of(Move::Idle), 4.0);
        assert!(m.guessed[Move::Clap.index()]);
    }

    #[test]
    fn a_move_without_a_clip_plays_its_fallback_then_the_first() {
        let clips = [clip("dance", 0.5), clip("clap", 0.5)];
        let m = Mapping::guess(&clips);
        assert!(!m.has(Move::Kick, 2));
        // Kick falls back towards Bounce (dance).
        assert_eq!(m.clip_for(Move::Bounce, 2), Some(0));
        let none = Mapping::guess(&[clip("a", 1.0)]);
        assert_eq!(none.clip_for(Move::Freeze, 1), Some(0));
        assert_eq!(none.clip_for(Move::Freeze, 0), None);
    }

    #[test]
    fn natural_beats_round_to_a_power_of_two() {
        assert_eq!(natural_beats(0.5), 1.0);
        assert_eq!(natural_beats(1.1), 2.0);
        assert_eq!(natural_beats(2.0), 4.0);
        assert_eq!(natural_beats(9.0), 8.0);
        assert_eq!(natural_beats(0.01), 1.0);
    }

    #[test]
    fn a_hostile_mapping_is_made_whole() {
        let m = Mapping {
            moves: vec![Some(99)],
            beats: vec![f32::NAN, -1.0, 1e9],
            moods: vec![],
            guessed: vec![],
        }
        .sanitised(3);
        assert_eq!(m.moves.len(), 16);
        assert_eq!(m.moves[0], None);
        assert_eq!(m.beats[0], 4.0);
        assert_eq!(m.beats[2], 64.0);
        assert_eq!(m.moods.len(), 8);
    }
}
