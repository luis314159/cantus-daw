//! The dancer — a character that dances on screen (round 35, ADR 0087): its parameter
//! model. It is a native instrument that makes **no sound**: a track hosts it, so its
//! numbers are ordinary automation and modulation targets (ADR 0057), MIDI learn reaches
//! them, notes on its track choose its moves (one key per move, like a drum map), and the
//! unit catalogue an agent reads lists them. What it draws is `cantus-dance`'s; what the
//! engine tells the screen is `cantus_engine::dance`.
//!
//! Every value is in the unit a person reads: a move and a mood by name (stepped indices
//! into [`Move::ALL`] and [`Mood::ALL`]), a multiplier, a share (shown as %), degrees, a
//! position on the stage (0 left/top … 1 right/bottom). Keys are forever once shipped.
//!
//! | Group | Keys |
//! |---|---|
//! | Dance | `move` (0 = dance to the beat, else a [`Move`]), `step` (0 in place, 1 left, 2 right, 3 wander), `speed` (× the beat: ¼ ½ 1 2 4), `energy` (%) |
//! | Body | `jump` (% of its height off the ground), `turn` (−1 facing left … 1 facing right), `spin` (°), `size` (×) |
//! | Stage | `x`, `y` (0…1 of the stage), `visible` (0/1) |
//! | Look | `mood` (a [`Mood`]), `hue` (° around the colour wheel) |
//! | Ears | `level` (0…1, what it hears — a follower modulator routes here), `react` (% of its energy the level decides: 0 = the level is ignored) |
//! | View (set, never automated) | `view.mode` (0 floating, 1 docked, 2 hidden), `view.top` (0/1 always on top) |
//! | Text | `character` (a [`CharacterRef`] string) |

use super::{find, InstParamInfo};
use crate::effect::p;
use crate::model::TrackIcon;

pub const NAME_KEY: &str = "instrument.dancer";
pub const ICON: TrackIcon = TrackIcon::Dance;
/// In the picker once the dancer builder has wired it end to end.
pub const READY: bool = true;

/// The first key of the move map: `BASE_KEY + i` plays `Move::ALL[i]` (C3 in Cantus's
/// naming, where 60 is C4) — sixteen keys, C3 to D♯4, a drum map's span.
pub const BASE_KEY: u8 = 48;

/// What a dancer can do. Every character does all of them (a character that has no move
/// of its own for one plays its [`Move::fallback`]); an imported character maps its own
/// animations onto them. The order is forever: automation and notes hold indices.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Move {
    Idle,
    Bounce,
    StepLeft,
    StepRight,
    Sway,
    Clap,
    Wave,
    Jump,
    Spin,
    Kick,
    Shuffle,
    Point,
    ArmsUp,
    Twist,
    Bow,
    Freeze,
}

impl Move {
    pub const ALL: [Move; 16] = [
        Move::Idle,
        Move::Bounce,
        Move::StepLeft,
        Move::StepRight,
        Move::Sway,
        Move::Clap,
        Move::Wave,
        Move::Jump,
        Move::Spin,
        Move::Kick,
        Move::Shuffle,
        Move::Point,
        Move::ArmsUp,
        Move::Twist,
        Move::Bow,
        Move::Freeze,
    ];

    pub fn index(self) -> usize {
        Move::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Move {
        Move::ALL[i.min(Move::ALL.len() - 1)]
    }

    /// The move a key of the dancer's track plays, if it is in the map.
    pub fn from_key(key: u8) -> Option<Move> {
        key.checked_sub(BASE_KEY)
            .map(usize::from)
            .filter(|&i| i < Move::ALL.len())
            .map(Move::from_index)
    }

    /// The i18n key of its name (`dance.move.<name>`).
    pub fn label_key(self) -> &'static str {
        match self {
            Move::Idle => "dance.move.idle",
            Move::Bounce => "dance.move.bounce",
            Move::StepLeft => "dance.move.step_left",
            Move::StepRight => "dance.move.step_right",
            Move::Sway => "dance.move.sway",
            Move::Clap => "dance.move.clap",
            Move::Wave => "dance.move.wave",
            Move::Jump => "dance.move.jump",
            Move::Spin => "dance.move.spin",
            Move::Kick => "dance.move.kick",
            Move::Shuffle => "dance.move.shuffle",
            Move::Point => "dance.move.point",
            Move::ArmsUp => "dance.move.arms_up",
            Move::Twist => "dance.move.twist",
            Move::Bow => "dance.move.bow",
            Move::Freeze => "dance.move.freeze",
        }
    }

    /// A stable machine name (agents, imported characters' mappings).
    pub fn name(self) -> &'static str {
        &self.label_key()["dance.move.".len()..]
    }

    /// What plays when a character has no move of its own for this one.
    pub fn fallback(self) -> Move {
        match self {
            Move::Idle | Move::Bounce => Move::Idle,
            Move::StepLeft | Move::StepRight | Move::Shuffle | Move::Kick | Move::Twist => {
                Move::Bounce
            }
            Move::Sway | Move::Bow | Move::Freeze => Move::Idle,
            Move::Clap | Move::Wave | Move::Point | Move::ArmsUp | Move::Jump | Move::Spin => {
                Move::Bounce
            }
        }
    }
}

/// A face. The order is forever, as [`Move`]'s.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mood {
    Neutral,
    Happy,
    Excited,
    Calm,
    Surprised,
    Sleepy,
    Cool,
    Shy,
}

impl Mood {
    pub const ALL: [Mood; 8] = [
        Mood::Neutral,
        Mood::Happy,
        Mood::Excited,
        Mood::Calm,
        Mood::Surprised,
        Mood::Sleepy,
        Mood::Cool,
        Mood::Shy,
    ];

    pub fn index(self) -> usize {
        Mood::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    pub fn from_index(i: usize) -> Mood {
        Mood::ALL[i.min(Mood::ALL.len() - 1)]
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Mood::Neutral => "dance.mood.neutral",
            Mood::Happy => "dance.mood.happy",
            Mood::Excited => "dance.mood.excited",
            Mood::Calm => "dance.mood.calm",
            Mood::Surprised => "dance.mood.surprised",
            Mood::Sleepy => "dance.mood.sleepy",
            Mood::Cool => "dance.mood.cool",
            Mood::Shy => "dance.mood.shy",
        }
    }

    pub fn name(self) -> &'static str {
        &self.label_key()["dance.mood.".len()..]
    }
}

/// The `speed` steps: multiples of the beat.
pub const SPEEDS: [f32; 5] = [0.25, 0.5, 1.0, 2.0, 4.0];

/// Which character a dancer shows, as stored under the `character` text key:
/// `builtin:<id>` (one of `cantus-dance`'s cast) or `file:<path>` (an imported character,
/// the path relative to the project's folder where the import copied it). A missing or
/// unreadable one draws the default character and says so; it is never an error.
pub const CHARACTER_KEY: &str = "character";
/// The cast's first character: what a new dancer shows.
pub const DEFAULT_CHARACTER: &str = "builtin:songbird"; // r35:cast

/// Keys that are set, never automated or modulated (where it is shown).
pub const VIEW_KEYS: &[&str] = &["view.mode", "view.top"];

pub const PARAMS: &[InstParamInfo] = &[
    p("move", "dance.p.move", 0.0, 16.0, 0.0, "", false, true),
    p("step", "dance.p.step", 0.0, 3.0, 0.0, "", false, true),
    p("speed", "dance.p.speed", 0.0, 4.0, 2.0, "", false, true),
    p("energy", "dance.p.energy", 0.0, 1.0, 0.7, "%", false, false),
    p("jump", "dance.p.jump", 0.0, 1.0, 0.0, "%", false, false),
    p("turn", "dance.p.turn", -1.0, 1.0, 0.0, "", false, false),
    p("spin", "dance.p.spin", 0.0, 360.0, 0.0, "°", false, false),
    p("size", "dance.p.size", 0.25, 4.0, 1.0, "×", true, false),
    p("x", "dance.p.x", 0.0, 1.0, 0.5, "", false, false),
    p("y", "dance.p.y", 0.0, 1.0, 0.85, "", false, false),
    p("visible", "dance.p.visible", 0.0, 1.0, 1.0, "", false, true),
    p("mood", "dance.p.mood", 0.0, 7.0, 1.0, "", false, true),
    p("hue", "dance.p.hue", -180.0, 180.0, 0.0, "°", false, false),
    p("level", "dance.p.level", 0.0, 1.0, 0.0, "%", false, false),
    p("react", "dance.p.react", 0.0, 1.0, 0.0, "%", false, false),
    p(
        "view.mode",
        "dance.p.view_mode",
        0.0,
        2.0,
        0.0,
        "",
        false,
        true,
    ),
    p(
        "view.top",
        "dance.p.view_top",
        0.0,
        1.0,
        0.0,
        "",
        false,
        true,
    ),
];

/// The keys in [`PARAMS`] order: what `cantus_engine::dance::DanceFrame::values` holds,
/// index for index.
pub const KEYS: [&str; 17] = [
    "move",
    "step",
    "speed",
    "energy",
    "jump",
    "turn",
    "spin",
    "size",
    "x",
    "y",
    "visible",
    "mood",
    "hue",
    "level",
    "react",
    "view.mode",
    "view.top",
];

pub fn params() -> &'static [InstParamInfo] {
    PARAMS
}

/// The modulation sources it produces (tags from `modulation::SOURCES`): a dancer makes no
/// sound and has no voices, so only what is global — the macros and the channel's
/// controllers (a mod wheel can drive its energy, a sustain pedal its jump).
pub const SOURCES: &[&str] = &[
    "macro1", "macro2", "macro3", "macro4", "macro5", "macro6", "macro7", "macro8", "mw", "expr",
    "at", "sus",
];

/// Its own keys first, then the shared model's (macros and the connection matrix), as
/// every kind's.
pub fn param(key: &str) -> Option<&'static InstParamInfo> {
    find(PARAMS, key).or_else(|| super::modulation::param(key))
}

pub fn is_text_key(key: &str) -> bool {
    key == CHARACTER_KEY || super::modulation::is_text_key(key)
}

/// Whether a track's instrument is a dancer.
pub fn is_dancer(spec: &crate::instrument::InstrumentSpec) -> bool {
    spec.native()
        .is_some_and(|n| n.kind == super::InstrumentKind::Dancer)
}

/// A `move` value as the move it plays: `None` for 0, the routine to the beat.
pub fn move_of(value: f32) -> Option<Move> {
    let i = value.round();
    (i >= 1.0).then(|| Move::from_index(i as usize - 1))
}

/// The `move` value that plays `m` (what automation, a knob and an agent write).
pub fn move_value(m: Move) -> f32 {
    (m.index() + 1) as f32
}

/// The `speed` multiplier a `speed` value stands for.
pub fn speed_of(value: f32) -> f32 {
    SPEEDS[(value.round().max(0.0) as usize).min(SPEEDS.len() - 1)]
}

pub fn is_asset_key(key: &str) -> bool {
    let _ = key;
    false
}

// ---- React to ‹a track› (ADR 0087 §1) ------------------------------------------------------

/// The key a dancer hears through (`level`) and the one that says how much it listens
/// (`react`).
pub const LEVEL_KEY: &str = "level";
pub const REACT_KEY: &str = "react";
/// How much a dancer listens when it is first told to react (`react`).
pub const REACT_DEPTH: f32 = 0.8;

/// The follower modulator "React to" put on a dancer's track: the one on its track routed
/// to its `level` (there is no second level path).
pub fn react_follower(
    project: &crate::model::Project,
    track: crate::model::TrackId,
) -> Option<&crate::modulator::Modulator> {
    use crate::modulator::{ModHost, ModSource};
    let level = crate::automation::AutoTarget::Instrument {
        track,
        key: LEVEL_KEY.into(),
    };
    project.modulators(ModHost::Track(track))?.iter().find(|m| {
        matches!(m.source, ModSource::Follower(_)) && m.routes.iter().any(|r| r.target == level)
    })
}

/// What a dancer reacts to now: the strip (or the master) its follower listens to, while
/// it listens at all (`react` above 0, the follower on).
pub fn reacting_to(
    project: &crate::model::Project,
    track: crate::model::TrackId,
) -> Option<crate::modulator::Listen> {
    use crate::modulator::{Listen, ModSource};
    let t = project.track(track)?;
    let spec = t
        .instrument
        .native()
        .filter(|n| n.kind == super::InstrumentKind::Dancer)?;
    let m = react_follower(project, track).filter(|m| !m.muted)?;
    if spec.get(REACT_KEY) <= 0.0 {
        return None;
    }
    match &m.source {
        ModSource::Follower(f) => Some(match f.listen {
            // Its own strip says nothing (a dancer is silent): read as the master.
            Listen::Host => Listen::Master,
            l => l,
        }),
        _ => None,
    }
}

/// The one undo step that makes a dancer react to `listen` (a strip or the master), with
/// `depth` of its energy decided by the level (`react`), or — `None` — stop: a follower
/// on its track routed to `level` (made, retuned, or taken away) and `react` set. `new_id`
/// names a follower it has to make (`Project::alloc_id`). `None` when the track hosts no
/// dancer, nothing would change, or its panel is full.
pub fn react_command(
    project: &crate::model::Project,
    track: crate::model::TrackId,
    listen: Option<crate::modulator::Listen>,
    depth: f32,
    new_id: u64,
) -> Option<crate::command::Command> {
    use crate::command::Command;
    use crate::modulator::{ModHost, ModKind, ModRoute, ModSource, Modulator};
    let t = project.track(track)?;
    let spec = t
        .instrument
        .native()
        .filter(|n| n.kind == super::InstrumentKind::Dancer)?;
    let follower = react_follower(project, track);
    let mut commands = Vec::new();
    let depth = depth.clamp(0.0, 1.0);
    let label = match listen {
        Some(l) => {
            match follower {
                Some(m) => {
                    let mut m = m.clone();
                    if let ModSource::Follower(f) = &mut m.source {
                        f.listen = l;
                    }
                    m.muted = false;
                    if Some(&m) != follower {
                        commands.push(Command::SetModulator {
                            modulator: Box::new(m),
                        });
                    }
                }
                None => {
                    let host = ModHost::Track(track);
                    let index = project.modulators(host)?.len();
                    if index >= crate::modulator::MAX_PER_HOST {
                        return None;
                    }
                    let mut m =
                        Modulator::new(crate::model::ModulatorId(new_id), ModKind::Follower);
                    if let ModSource::Follower(f) = &mut m.source {
                        f.listen = l;
                        f.attack_ms = 10.0;
                        f.release_ms = 180.0;
                        f.gain_db = 6.0;
                    }
                    m.routes.push(ModRoute {
                        target: crate::automation::AutoTarget::Instrument {
                            track,
                            key: LEVEL_KEY.into(),
                        },
                        depth: 1.0,
                    });
                    commands.push(Command::InsertModulator {
                        host,
                        index,
                        modulator: Box::new(m),
                    });
                }
            }
            if spec.get(REACT_KEY) != depth {
                commands.push(Command::SetInstrumentValue {
                    track,
                    key: REACT_KEY.into(),
                    value: depth,
                });
            }
            "cmd.dance_react"
        }
        None => {
            if let Some(m) = follower {
                commands.push(Command::RemoveModulator { id: m.id });
            }
            if spec.is_stored(REACT_KEY) {
                commands.push(Command::ClearInstrumentValue {
                    track,
                    key: REACT_KEY.into(),
                });
            }
            "cmd.dance_react_off"
        }
    };
    (!commands.is_empty()).then(|| Command::Batch {
        label_key: label.into(),
        commands,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keys_follow_the_table() {
        assert_eq!(KEYS.len(), PARAMS.len());
        for (k, p) in KEYS.iter().zip(PARAMS) {
            assert_eq!(*k, p.key);
            assert!(p.min <= p.default && p.default <= p.max, "{k}");
        }
        assert_eq!(param("move").unwrap().max as usize, Move::ALL.len());
        assert_eq!(param("mood").unwrap().max as usize + 1, Mood::ALL.len());
        assert_eq!(param("speed").unwrap().max as usize + 1, SPEEDS.len());
        assert!(VIEW_KEYS.iter().all(|k| param(k).is_some()));
    }

    #[test]
    fn move_values_round_trip_and_zero_is_the_routine() {
        assert_eq!(move_of(0.0), None);
        assert_eq!(move_of(0.4), None);
        for m in Move::ALL {
            assert_eq!(move_of(move_value(m)), Some(m));
        }
        assert_eq!(move_of(99.0), Some(Move::Freeze));
        assert_eq!(speed_of(param("speed").unwrap().default), 1.0);
        assert_eq!(speed_of(0.0), 0.25);
        assert_eq!(speed_of(9.0), 4.0);
        // The shared model's keys are a dancer's too (macros, the matrix).
        assert!(param("macro1").is_some() && is_text_key("mod1.dst"));
        assert!(is_text_key(CHARACTER_KEY) && !is_asset_key(CHARACTER_KEY));
    }

    #[test]
    fn react_to_is_one_step_that_makes_retunes_and_takes_away_a_follower() {
        use crate::instrument::InstrumentSpec;
        use crate::modulator::Listen;
        use crate::CommandBus;
        use crate::Origin;
        let mut p = crate::model::Project::new("React");
        let mut d = p.make_track("Dancer");
        d.instrument = InstrumentSpec::Native(super::super::NativeSpec::new(
            super::super::InstrumentKind::Dancer,
        ));
        let track = d.id;
        p.tracks.push(d);
        let drums_track = p.make_track("Drums");
        let (other, drums) = (drums_track.id, drums_track.strip.unwrap());
        p.tracks.push(drums_track);
        let mut bus = CommandBus::new(p);
        assert_eq!(reacting_to(bus.project(), track), None);
        // Not a dancer: nothing to do.
        assert!(react_command(bus.project(), other, Some(Listen::Master), 0.8, 999).is_none());
        let id = bus.alloc_id();
        let cmd = react_command(bus.project(), track, Some(Listen::Strip(drums)), 0.8, id).unwrap();
        assert_eq!(cmd.label_key(), "cmd.dance_react");
        bus.execute(cmd, Origin::User).unwrap();
        assert_eq!(
            reacting_to(bus.project(), track),
            Some(Listen::Strip(drums))
        );
        let spec = bus
            .project()
            .track(track)
            .unwrap()
            .instrument
            .native()
            .unwrap()
            .clone();
        assert_eq!(spec.get(REACT_KEY), 0.8);
        // Again, the same: nothing; to the master: the same follower retuned.
        assert!(react_command(bus.project(), track, Some(Listen::Strip(drums)), 0.8, 1).is_none());
        let cmd = react_command(bus.project(), track, Some(Listen::Master), 0.5, 1).unwrap();
        bus.execute(cmd, Origin::User).unwrap();
        assert_eq!(reacting_to(bus.project(), track), Some(Listen::Master));
        let host = crate::modulator::ModHost::Track(track);
        assert_eq!(
            bus.project().modulators(host).unwrap().len(),
            1,
            "one level path"
        );
        // Stop: the follower gone, `react` back to its default; undone, back as it was.
        let before = bus.project().clone();
        let cmd = react_command(bus.project(), track, None, 0.0, 1).unwrap();
        assert_eq!(cmd.label_key(), "cmd.dance_react_off");
        bus.execute(cmd, Origin::User).unwrap();
        assert_eq!(reacting_to(bus.project(), track), None);
        assert!(bus.project().modulators(host).unwrap().is_empty());
        assert!(bus.undo().is_some());
        assert_eq!(bus.project(), &before);
    }

    #[test]
    fn the_move_map_spans_sixteen_keys() {
        assert_eq!(Move::from_key(BASE_KEY), Some(Move::Idle));
        assert_eq!(Move::from_key(BASE_KEY + 15), Some(Move::Freeze));
        assert_eq!(Move::from_key(BASE_KEY + 16), None);
        assert_eq!(Move::from_key(BASE_KEY - 1), None);
        for m in Move::ALL {
            assert_eq!(Move::from_index(m.index()), m);
            assert!(!m.name().is_empty());
        }
        for m in Mood::ALL {
            assert_eq!(Mood::from_index(m.index()), m);
        }
    }
}
