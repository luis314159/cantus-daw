//! Modulators on any parameter (ADR 0057): Bitwig's modulators, in Cantus's model.
//!
//! A **modulator** is a source of movement — an LFO, an envelope, an envelope follower, a
//! step sequencer or a random source — that lives on a track, a mixer strip or the master
//! (its *host*: the panel it is shown in, and where its notes and its audio come from by
//! default) and drives **any number of parameters**, each through a [`ModRoute`] with its
//! own **bipolar depth**. Any parameter an automation clip can address can be modulated
//! ([`AutoTarget`]), hosted plug-ins included, and a modulator's own amount (one level of
//! nesting: [`AutoTarget::ModAmount`]).
//!
//! **What modulation does.** It offsets the parameter's base value — what its automation
//! gives where the song is, or its knob when nothing automates it — in the **normalised
//! domain** (the position along the control, 0…1, the same domain automation curves are
//! drawn in): `position = clamp(base + Σ depth × amount × output, 0, 1)`. Outputs are
//! bipolar (−1…1) for an LFO, a step sequence and a random source (unless set unipolar),
//! and unipolar (0…1) for an envelope and a follower.
//!
//! **How it is evaluated** is in [`eval`] (pure functions of the song position, the time
//! since playback started and the notes; shared by the engine and *Bake to automation*)
//! and [`compile`] (what the snapshot hands the engine).

use crate::automation::AutoTarget;
use crate::command::{Command, CommandError};
use crate::model::{ModulatorId, Project, StripId, TrackId};
use serde::{Deserialize, Serialize};

#[path = "modulator_bake.rs"]
pub mod bake;
#[path = "modulator_compile.rs"]
pub mod compile;
#[path = "modulator_eval.rs"]
pub mod eval;

/// Modulators one host holds at most.
pub const MAX_PER_HOST: usize = 16;
/// Parameters one modulator drives at most.
pub const MAX_ROUTES: usize = 32;
/// Steps a step sequencer holds at most.
pub const MAX_STEPS: usize = 32;

/// Where a modulator lives: the panel it is shown in.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ModHost {
    Track(TrackId),
    Strip(StripId),
    Master,
}

/// One parameter a modulator drives, and how far: `depth` is a share of the control's
/// whole travel (−1…1), so 0.25 on an LFO swings a quarter of the knob each way.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ModRoute {
    pub target: AutoTarget,
    pub depth: f32,
}

/// How a modulator keeps time.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ModRate {
    /// A cycle per division of the song's beats (`cantus_dsp::modfx_clock::DIVISIONS`,
    /// 8/1 … 1/32): the phase is a function of the song position.
    Sync { division: u8 },
    /// Cycles per second, from the start of playback.
    Hz { hz: f32 },
}

impl ModRate {
    pub const MIN_HZ: f32 = 0.01;
    pub const MAX_HZ: f32 = 40.0;

    fn is_valid(&self) -> bool {
        match *self {
            ModRate::Sync { division } => {
                (division as usize) < cantus_dsp::modfx_clock::DIVISIONS.len()
            }
            ModRate::Hz { hz } => hz.is_finite() && (Self::MIN_HZ..=Self::MAX_HZ).contains(&hz),
        }
    }
}

/// An LFO's wave (the modulation family's waves, `cantus_dsp::modfx_lfo::Shape`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LfoShape {
    #[default]
    Sine,
    Triangle,
    Square,
    SawUp,
    SawDown,
}

impl LfoShape {
    pub const ALL: [LfoShape; 5] = [
        LfoShape::Sine,
        LfoShape::Triangle,
        LfoShape::Square,
        LfoShape::SawUp,
        LfoShape::SawDown,
    ];
}

/// Where an LFO's cycle starts.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Retrigger {
    /// Synced: on the song position. Free: at the start of playback.
    #[default]
    Song,
    /// At every note-on of the trigger track.
    Note,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LfoSource {
    pub shape: LfoShape,
    pub rate: ModRate,
    /// Where in its cycle it starts, 0…1.
    #[serde(default)]
    pub phase: f32,
    #[serde(default)]
    pub retrigger: Retrigger,
    /// 0…1 instead of −1…1.
    #[serde(default)]
    pub unipolar: bool,
    /// The track whose notes retrigger it; `None` is the host's (the host track, or the
    /// first track feeding the host strip).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<TrackId>,
}

/// What opens an envelope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum EnvTrigger {
    /// The notes of a track (`None`: the host's).
    Notes { track: Option<TrackId> },
    /// The audio of a strip crossing a threshold (dBFS, peak; it closes 3 dB below).
    Audio { listen: Listen, threshold_db: f32 },
}

/// The audio a follower or an audio-triggered envelope listens to.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Listen {
    /// The host's own strip (a track's strip, the strip itself, the master).
    Host,
    Strip(StripId),
    Master,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EnvSource {
    pub attack_ms: f32,
    pub decay_ms: f32,
    /// 0…1.
    pub sustain: f32,
    pub release_ms: f32,
    pub trigger: EnvTrigger,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FollowSource {
    pub listen: Listen,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// Gain on the level before it is the output (dB): a quiet strip can still swing a
    /// parameter all the way.
    #[serde(default)]
    pub gain_db: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StepSource {
    /// −1…1 each, 1 to [`MAX_STEPS`] of them.
    pub steps: Vec<f32>,
    /// The length of one step: an index into the divisions (synced, always).
    pub division: u8,
    /// 0…1 of a step spent gliding into the next value.
    #[serde(default)]
    pub glide: f32,
    #[serde(default)]
    pub unipolar: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RandomSource {
    /// How often a new value is drawn.
    pub rate: ModRate,
    /// Glide between values (smooth) instead of holding each (sample and hold).
    #[serde(default)]
    pub smooth: bool,
    /// A render repeats: every value is a hash of the seed and the step's number.
    pub seed: u32,
    #[serde(default)]
    pub unipolar: bool,
}

/// What moves.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ModSource {
    Lfo(LfoSource),
    Envelope(EnvSource),
    Follower(FollowSource),
    Step(StepSource),
    Random(RandomSource),
}

/// The kinds, for menus.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModKind {
    Lfo,
    Envelope,
    Follower,
    Step,
    Random,
}

impl ModKind {
    pub const ALL: [ModKind; 5] = [
        ModKind::Lfo,
        ModKind::Envelope,
        ModKind::Follower,
        ModKind::Step,
        ModKind::Random,
    ];

    /// The i18n key of its name.
    pub fn name_key(self) -> &'static str {
        match self {
            ModKind::Lfo => "mods.kind.lfo",
            ModKind::Envelope => "mods.kind.envelope",
            ModKind::Follower => "mods.kind.follower",
            ModKind::Step => "mods.kind.step",
            ModKind::Random => "mods.kind.random",
        }
    }

    /// A new source of this kind, at its defaults.
    pub fn source(self) -> ModSource {
        match self {
            ModKind::Lfo => ModSource::Lfo(LfoSource {
                shape: LfoShape::Sine,
                rate: ModRate::Sync {
                    division: cantus_dsp::modfx_clock::DIV_QUARTER as u8,
                },
                phase: 0.0,
                retrigger: Retrigger::Song,
                unipolar: false,
                notes: None,
            }),
            ModKind::Envelope => ModSource::Envelope(EnvSource {
                attack_ms: 10.0,
                decay_ms: 300.0,
                sustain: 0.5,
                release_ms: 300.0,
                trigger: EnvTrigger::Notes { track: None },
            }),
            ModKind::Follower => ModSource::Follower(FollowSource {
                listen: Listen::Host,
                attack_ms: 5.0,
                release_ms: 150.0,
                gain_db: 0.0,
            }),
            ModKind::Step => ModSource::Step(StepSource {
                steps: vec![1.0, -0.5, 0.25, -1.0, 0.75, 0.0, -0.25, 0.5],
                division: DIV_SIXTEENTH,
                glide: 0.0,
                unipolar: false,
            }),
            ModKind::Random => ModSource::Random(RandomSource {
                rate: ModRate::Sync {
                    division: cantus_dsp::modfx_clock::DIV_EIGHTH as u8,
                },
                smooth: false,
                seed: 1,
                unipolar: false,
            }),
        }
    }
}

/// The index of 1/16 in the divisions.
pub const DIV_SIXTEENTH: u8 = 14;

impl ModSource {
    pub fn kind(&self) -> ModKind {
        match self {
            ModSource::Lfo(_) => ModKind::Lfo,
            ModSource::Envelope(_) => ModKind::Envelope,
            ModSource::Follower(_) => ModKind::Follower,
            ModSource::Step(_) => ModKind::Step,
            ModSource::Random(_) => ModKind::Random,
        }
    }

    /// Whether its output is −1…1 (else 0…1).
    pub fn bipolar(&self) -> bool {
        match self {
            ModSource::Lfo(l) => !l.unipolar,
            ModSource::Step(s) => !s.unipolar,
            ModSource::Random(r) => !r.unipolar,
            ModSource::Envelope(_) | ModSource::Follower(_) => false,
        }
    }

    /// Whether it follows audio (a follower, an envelope opened by a level): it can only
    /// be known as the audio plays, never ahead, and is never baked without a render.
    pub fn follows_audio(&self) -> bool {
        matches!(
            self,
            ModSource::Follower(_)
                | ModSource::Envelope(EnvSource {
                    trigger: EnvTrigger::Audio { .. },
                    ..
                })
        )
    }

    fn is_valid(&self) -> bool {
        let ms = |x: f32, lo: f32| x.is_finite() && (lo..=10_000.0).contains(&x);
        match self {
            ModSource::Lfo(l) => {
                l.rate.is_valid() && l.phase.is_finite() && (0.0..=1.0).contains(&l.phase)
            }
            ModSource::Envelope(e) => {
                ms(e.attack_ms, 0.0)
                    && ms(e.decay_ms, 0.0)
                    && ms(e.release_ms, 0.0)
                    && e.sustain.is_finite()
                    && (0.0..=1.0).contains(&e.sustain)
                    && match e.trigger {
                        EnvTrigger::Notes { .. } => true,
                        EnvTrigger::Audio { threshold_db, .. } => {
                            threshold_db.is_finite() && (-90.0..=0.0).contains(&threshold_db)
                        }
                    }
            }
            ModSource::Follower(f) => {
                ms(f.attack_ms, 0.0)
                    && ms(f.release_ms, 0.0)
                    && f.gain_db.is_finite()
                    && (-48.0..=48.0).contains(&f.gain_db)
            }
            ModSource::Step(s) => {
                (1..=MAX_STEPS).contains(&s.steps.len())
                    && s.steps
                        .iter()
                        .all(|v| v.is_finite() && (-1.0..=1.0).contains(v))
                    && (s.division as usize) < cantus_dsp::modfx_clock::DIVISIONS.len()
                    && s.glide.is_finite()
                    && (0.0..=1.0).contains(&s.glide)
            }
            ModSource::Random(r) => r.rate.is_valid(),
        }
    }
}

/// A modulator: a source and the parameters it drives.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Modulator {
    pub id: ModulatorId,
    /// Empty: the panel names it by kind and number ("LFO 1").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    pub source: ModSource,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<ModRoute>,
    /// Scales every route at once, 0…1 — a target of automation and of another modulator.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub amount: f32,
    /// Switched off: it drives nothing (and compiles to nothing).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub muted: bool,
}

fn one() -> f32 {
    1.0
}

fn is_one(x: &f32) -> bool {
    *x == 1.0
}

impl Modulator {
    pub fn new(id: ModulatorId, kind: ModKind) -> Self {
        Modulator {
            id,
            name: String::new(),
            source: kind.source(),
            routes: Vec::new(),
            amount: 1.0,
            muted: false,
        }
    }

    /// Everything a command accepts: finite and in range, at most [`MAX_ROUTES`] routes,
    /// one per target, none on its own amount.
    pub fn is_valid(&self) -> bool {
        self.source.is_valid()
            && self.amount.is_finite()
            && (0.0..=1.0).contains(&self.amount)
            && self.routes.len() <= MAX_ROUTES
            && self.routes.iter().enumerate().all(|(i, r)| {
                r.depth.is_finite()
                    && (-1.0..=1.0).contains(&r.depth)
                    && r.target != AutoTarget::ModAmount(self.id)
                    && !self.routes[..i].iter().any(|q| q.target == r.target)
            })
    }

    /// The route to `target`, if it drives it.
    pub fn route(&self, target: &AutoTarget) -> Option<&ModRoute> {
        self.routes.iter().find(|r| &r.target == target)
    }
}

impl Project {
    /// A host's modulators; `None` when the host is gone.
    pub fn modulators(&self, host: ModHost) -> Option<&Vec<Modulator>> {
        match host {
            ModHost::Track(t) => self.track(t).map(|t| &t.modulators),
            ModHost::Strip(s) => self.strip(s).map(|s| &s.modulators),
            ModHost::Master => Some(&self.master.modulators),
        }
    }

    pub fn modulators_mut(&mut self, host: ModHost) -> Option<&mut Vec<Modulator>> {
        match host {
            ModHost::Track(t) => self.track_mut(t).map(|t| &mut t.modulators),
            ModHost::Strip(s) => self.strip_mut(s).map(|s| &mut s.modulators),
            ModHost::Master => Some(&mut self.master.modulators),
        }
    }

    /// Every modulator, with its host: the tracks', the strips', the master's, in order.
    pub fn all_modulators(&self) -> impl Iterator<Item = (ModHost, &Modulator)> {
        self.tracks
            .iter()
            .flat_map(|t| t.modulators.iter().map(move |m| (ModHost::Track(t.id), m)))
            .chain(
                self.strips
                    .iter()
                    .flat_map(|s| s.modulators.iter().map(move |m| (ModHost::Strip(s.id), m))),
            )
            .chain(self.master.modulators.iter().map(|m| (ModHost::Master, m)))
    }

    /// A modulator, its host and its index there.
    pub fn modulator(&self, id: ModulatorId) -> Option<(ModHost, usize, &Modulator)> {
        let (host, m) = self.all_modulators().find(|(_, m)| m.id == id)?;
        let i = self.modulators(host)?.iter().position(|x| x.id == id)?;
        Some((host, i, m))
    }

    /// Every route into `target`, with its modulator.
    pub fn routes_into<'a>(
        &'a self,
        target: &'a AutoTarget,
    ) -> impl Iterator<Item = (&'a Modulator, &'a ModRoute)> + 'a {
        self.all_modulators()
            .map(|(_, m)| m)
            .filter_map(move |m| m.route(target).map(|r| (m, r)))
    }

    /// Whether any modulator drives `target`.
    pub fn is_modulated(&self, target: &AutoTarget) -> bool {
        self.routes_into(target).next().is_some()
    }

    /// The track whose notes a host's modulators hear by default: the track itself, or the
    /// first track feeding the strip; the master has none.
    pub fn host_notes_track(&self, host: ModHost) -> Option<TrackId> {
        match host {
            ModHost::Track(t) => self.track(t).map(|t| t.id),
            ModHost::Strip(s) => self.tracks_on_strip(s).next().map(|t| t.id),
            ModHost::Master => None,
        }
    }

    /// The strip a host's modulators listen to by default (`None`: the master).
    pub fn host_listen_strip(&self, host: ModHost) -> Option<StripId> {
        match host {
            ModHost::Track(t) => self.track(t).and_then(|t| t.strip),
            ModHost::Strip(s) => Some(s),
            ModHost::Master => None,
        }
    }
}

fn host_error(host: ModHost) -> CommandError {
    match host {
        ModHost::Track(t) => CommandError::TrackNotFound(t),
        ModHost::Strip(s) => CommandError::StripNotFound(s),
        ModHost::Master => CommandError::InvalidValue("master".into()),
    }
}

/// Modulators arriving with a track or a strip: each valid, and no id the project (or
/// another of them) already uses.
pub(crate) fn check_new(project: &Project, mods: &[Modulator]) -> Result<(), CommandError> {
    for (i, m) in mods.iter().enumerate() {
        if !m.is_valid() {
            return Err(CommandError::InvalidValue("modulator".into()));
        }
        if project.modulator(m.id).is_some() || mods[..i].iter().any(|o| o.id == m.id) {
            return Err(CommandError::DuplicateId(m.id.0));
        }
    }
    if mods.len() > MAX_PER_HOST {
        return Err(CommandError::InvalidValue("too many modulators".into()));
    }
    Ok(())
}

pub(crate) fn apply_insert(
    project: &mut Project,
    host: ModHost,
    index: usize,
    modulator: Modulator,
) -> Result<Command, CommandError> {
    check_new(project, std::slice::from_ref(&modulator))?;
    let list = project.modulators_mut(host).ok_or(host_error(host))?;
    let len = list.len();
    if index > len {
        return Err(CommandError::IndexOutOfRange { index, len });
    }
    if len >= MAX_PER_HOST {
        return Err(CommandError::InvalidValue("too many modulators".into()));
    }
    let id = modulator.id;
    list.insert(index, modulator);
    Ok(Command::RemoveModulator { id })
}

pub(crate) fn apply_remove(
    project: &mut Project,
    id: ModulatorId,
) -> Result<Command, CommandError> {
    let (host, index, _) = project
        .modulator(id)
        .ok_or(CommandError::InvalidValue(format!(
            "modulator {id} not found"
        )))?;
    let list = project.modulators_mut(host).ok_or(host_error(host))?;
    let m = list.remove(index);
    Ok(Command::InsertModulator {
        host,
        index,
        modulator: Box::new(m),
    })
}

pub(crate) fn apply_set(
    project: &mut Project,
    modulator: Modulator,
) -> Result<Command, CommandError> {
    if !modulator.is_valid() {
        return Err(CommandError::InvalidValue("modulator".into()));
    }
    let id = modulator.id;
    let (host, index, _) = project
        .modulator(id)
        .ok_or(CommandError::InvalidValue(format!(
            "modulator {id} not found"
        )))?;
    let list = project.modulators_mut(host).ok_or(host_error(host))?;
    let old = std::mem::replace(&mut list[index], modulator);
    Ok(Command::SetModulator {
        modulator: Box::new(old),
    })
}

#[cfg(test)]
#[path = "modulator_tests.rs"]
mod tests;
