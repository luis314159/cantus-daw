//! MIDI controller mappings (round 29, ADR 0062): a knob, fader, button or pad of a
//! hardware controller bound to anything Cantus can set — a parameter of any device (native
//! or hosted), the mixer's switches, the tempo, the transport's actions — the way FL's
//! *Link to controller*, Ableton's MIDI Map mode, Bitwig's and Reaper's learn do it.
//!
//! A [`Mapping`] says where a message comes from ([`MidiSource`]: a port by name and
//! ordinal, a channel, a control), what it drives ([`MappingTarget`]), how it reads the
//! message ([`MapMode`]: absolute, relative in three encodings, toggle, momentary,
//! trigger), over which part of the target's travel and with which bend ([`Mapping::min`],
//! [`Mapping::max`], [`Mapping::curve`]), how it takes over a parameter that moved
//! elsewhere ([`Takeover`]), and whether it sends the value back ([`Mapping::feedback`]).
//! Positions are the automation's (0…1 of the control's travel, ADR 0027), so a fader's
//! taper, a frequency's log scale and a stepped switch come with the target.
//!
//! The project keeps its own map ([`ControllerMap`], `Project::controllers`, schema 35):
//! edits are commands with exact inverses. The user's global map (transport, master, *the
//! selected track*, *the focused device's first eight parameters*) is a settings file the
//! interface keeps; its targets are the [portable](MappingTarget::is_portable) ones. A
//! message that a project mapping takes is not looked up in the global map (the project
//! comes first). Nothing here runs on the audio thread: a mapped message becomes a command
//! on the interface thread, as a mouse drag does.

use crate::automation::range::range_of;
use crate::automation::{AutoTarget, Shape};
use crate::command::{Command, CommandError};
use crate::model::{EffectId, Project, StripId, TrackId};
use crate::time::Ticks;
use serde::{Deserialize, Serialize};

#[path = "controller_drive.rs"]
pub mod drive;
#[path = "controller_learn.rs"]
pub mod learn;

pub use drive::{Drive, DriveState, Input};

/// A mapping's identity in its map: stable across saves and edits.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MappingId(pub u64);

/// The port a mapping listens to: its name and, where two ports share a name, which one
/// (0 the first) — never its index, which changes when a device is plugged in. An empty
/// name listens to every port.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PortRef {
    pub name: String,
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub ordinal: u8,
}

fn is_zero_u8(v: &u8) -> bool {
    *v == 0
}

impl PortRef {
    /// Every port.
    pub fn any() -> PortRef {
        PortRef::default()
    }

    pub fn is_any(&self) -> bool {
        self.name.is_empty()
    }

    /// The port open at `index` among `names` (as they are open now).
    pub fn of(names: &[String], index: usize) -> PortRef {
        let name = names.get(index).cloned().unwrap_or_default();
        PortRef {
            ordinal: ordinal_of(names, index),
            name,
        }
    }

    /// Whether the port open at `index` among `names` is this one.
    pub fn matches(&self, names: &[String], index: usize) -> bool {
        if self.is_any() {
            return index < names.len();
        }
        names.get(index) == Some(&self.name) && ordinal_of(names, index) == self.ordinal
    }

    /// The index of this port among `names`, when it is open (the first, for any).
    pub fn index_in(&self, names: &[String]) -> Option<usize> {
        (0..names.len()).find(|i| self.matches(names, *i))
    }
}

/// How many ports before `index` share its name.
pub fn ordinal_of(names: &[String], index: usize) -> u8 {
    let Some(name) = names.get(index) else {
        return 0;
    };
    let n = names[..index].iter().filter(|n| *n == name).count();
    n.min(u8::MAX as usize) as u8
}

/// Which control on the hardware: a message's address, without its value.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Control {
    /// A 7-bit controller (0…119).
    Cc(u8),
    /// A 14-bit controller pair: the coarse value on `n` (0…31), the fine on `n + 32`.
    Cc14(u8),
    /// A key or a pad: its velocity, or pressed / let go.
    Note(u8),
    /// The pitch wheel (14 bits).
    PitchBend,
    /// A program change to this number: a button.
    Program(u8),
    /// Channel pressure (aftertouch).
    ChannelPressure,
    /// One key's pressure (polyphonic aftertouch).
    PolyPressure(u8),
}

impl Control {
    /// The largest value the control sends.
    pub fn max(self) -> u16 {
        match self {
            Control::Cc14(_) | Control::PitchBend => 16_383,
            _ => 127,
        }
    }

    /// Whether the address is one a message can carry (numbers within MIDI's).
    pub fn is_valid(self) -> bool {
        match self {
            Control::Cc(n) => n < 120,
            Control::Cc14(n) => n < 32,
            Control::Note(n) | Control::Program(n) | Control::PolyPressure(n) => n < 128,
            Control::PitchBend | Control::ChannelPressure => true,
        }
    }

    /// Whether it is a key, a pad or a program change: something pressed, not turned.
    pub fn is_button(self) -> bool {
        matches!(self, Control::Note(_) | Control::Program(_))
    }
}

/// Where a mapping's messages come from.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct MidiSource {
    pub port: PortRef,
    /// 0…15, or `None` for every channel.
    pub channel: Option<u8>,
    pub control: Control,
}

impl MidiSource {
    pub fn is_valid(&self) -> bool {
        self.control.is_valid() && self.channel.is_none_or(|c| c < 16)
    }

    /// Whether this source and `other` can take the same message (one port and channel
    /// covering the other's, the same control).
    pub fn overlaps(&self, other: &MidiSource) -> bool {
        let ports = self.port.is_any() || other.port.is_any() || self.port == other.port;
        let channels =
            self.channel.is_none() || other.channel.is_none() || self.channel == other.channel;
        let controls = match (self.control, other.control) {
            // A 14-bit pair takes both of its 7-bit controllers.
            (Control::Cc14(a), Control::Cc(b)) | (Control::Cc(b), Control::Cc14(a)) => {
                b == a || b == a + 32
            }
            (a, b) => a == b,
        };
        ports && channels && controls
    }
}

/// How a relative encoder writes a step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Encoding {
    /// 1…63 clockwise, 127…65 counter-clockwise (−1 is 127).
    TwosComplement,
    /// 64 is still: 65… clockwise, …63 counter-clockwise.
    BinaryOffset,
    /// The top bit is the sign: 1…63 clockwise, 65…127 counter-clockwise (65 is −1).
    SignBit,
}

impl Encoding {
    pub const ALL: [Encoding; 3] = [
        Encoding::TwosComplement,
        Encoding::BinaryOffset,
        Encoding::SignBit,
    ];

    /// The encoding that reads `value` as a small step where this one reads it as a jump of
    /// half the range — what an encoder learnt turning one way turns out to speak when it
    /// turns the other: a sign-bit encoder learnt clockwise looks like two's complement
    /// (1, 1, 1…), learnt counter-clockwise like binary offset (65, 65…).
    pub fn reinterpret(self, value: u8) -> Option<Encoding> {
        let v = value & 0x7F;
        match self {
            Encoding::TwosComplement if (65..=79).contains(&v) => Some(Encoding::SignBit),
            Encoding::BinaryOffset if (1..=15).contains(&v) => Some(Encoding::SignBit),
            Encoding::SignBit if (113..=127).contains(&v) => Some(Encoding::TwosComplement),
            Encoding::SignBit if (49..=63).contains(&v) => Some(Encoding::BinaryOffset),
            _ => None,
        }
    }

    /// The steps one value means (0 for none).
    pub fn steps(self, value: u8) -> i32 {
        let v = i32::from(value & 0x7F);
        match self {
            Encoding::TwosComplement => {
                if v < 64 {
                    v
                } else {
                    v - 128
                }
            }
            Encoding::BinaryOffset => v - 64,
            Encoding::SignBit => {
                if v & 64 != 0 {
                    -(v & 63)
                } else {
                    v & 63
                }
            }
        }
    }
}

/// How a mapping reads its control.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MapMode {
    /// The control's position is the target's (a knob, a fader, a pad's velocity).
    Absolute,
    /// An endless encoder: each message moves the target by its steps.
    Relative(Encoding),
    /// Each press flips the target between the two ends of its range.
    Toggle,
    /// Held: the top of the range; let go: the bottom.
    Momentary,
    /// Each press fires the target (an action), or sends it to the top of its range.
    Trigger,
}

/// How an absolute control takes over a target that moved elsewhere (the mouse, an undo,
/// automation) — so a knob never makes the sound jump.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Takeover {
    /// The target goes where the control is at once.
    Jump,
    /// Nothing moves until the control passes the target's value (FL's and Ableton's
    /// *pick up*).
    #[default]
    Pickup,
    /// The target moves with the control, scaled so both reach the end together (Reaper's
    /// and Ableton's *value scaling*).
    Scale,
}

/// One control of a mixer row that a relative target names (*the selected track's
/// volume*).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DeskParam {
    Volume,
    Pan,
    Mute,
    Solo,
    /// Recording arm (a track's; a strip has none).
    Arm,
}

impl DeskParam {
    pub const ALL: [DeskParam; 5] = [
        DeskParam::Volume,
        DeskParam::Pan,
        DeskParam::Mute,
        DeskParam::Solo,
        DeskParam::Arm,
    ];
}

/// The most parameters of the focused device a global map reaches (*Device 1…8*).
pub const DEVICE_PARAMS: u8 = 8;

/// The tempo's travel for a controller: what a knob from bottom to top covers (beats per
/// minute). A mapping narrows it with its range; typing reaches the rest.
pub const TEMPO_TRAVEL: (f64, f64) = (40.0, 240.0);

/// What a mapping drives.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MappingTarget {
    /// Any automatable parameter: a device's (native or hosted), a fader, a pan, a width, a
    /// send, the master's, a modulator's amount.
    Param(AutoTarget),
    TrackMute(TrackId),
    TrackSolo(TrackId),
    TrackArm(TrackId),
    StripMute(StripId),
    StripSolo(StripId),
    /// An insert in or out (a native or a hosted one).
    EffectBypass(EffectId),
    /// An insert's own dry/wet.
    EffectMix(EffectId),
    /// The tempo where the playhead is (ADR 0060 §2: the song's tempo, or the governing
    /// point's), over [`TEMPO_TRAVEL`].
    Tempo,
    /// A keymap action by its stable id (`keymap::Action::id`): play, stop, record, loop,
    /// the metronome, tap tempo, the markers, PAT / SONG, anything the keys can do.
    Action(String),
    /// The selected track's control (global maps).
    SelectedTrack(DeskParam),
    /// The selected mixer strip's control (global maps).
    SelectedStrip(DeskParam),
    /// The focused device's parameter `n` (0…7, global maps).
    DeviceParam(u8),
}

/// What the interface knows when a relative target is resolved.
#[derive(Clone, Copy, Debug, Default)]
pub struct Focus<'a> {
    pub track: Option<TrackId>,
    pub strip: Option<StripId>,
    /// The focused device's first parameters, in order.
    pub device: &'a [AutoTarget],
}

impl MappingTarget {
    /// Whether a global map may hold it: it means the same thing in every project.
    pub fn is_portable(&self) -> bool {
        match self {
            MappingTarget::Param(t) => matches!(
                t,
                AutoTarget::MasterGain | AutoTarget::MasterPan | AutoTarget::MasterWidth
            ),
            MappingTarget::Tempo
            | MappingTarget::Action(_)
            | MappingTarget::SelectedTrack(_)
            | MappingTarget::SelectedStrip(_)
            | MappingTarget::DeviceParam(_) => true,
            _ => false,
        }
    }

    /// Whether it names something only once the interface says what is selected or focused.
    pub fn is_relative(&self) -> bool {
        matches!(
            self,
            MappingTarget::SelectedTrack(_)
                | MappingTarget::SelectedStrip(_)
                | MappingTarget::DeviceParam(_)
        )
    }

    /// Whether it is on or off (a mute, a solo, an arm, a bypass) rather than a range.
    pub fn is_switch(&self) -> bool {
        match self {
            MappingTarget::TrackMute(_)
            | MappingTarget::TrackSolo(_)
            | MappingTarget::TrackArm(_)
            | MappingTarget::StripMute(_)
            | MappingTarget::StripSolo(_)
            | MappingTarget::EffectBypass(_) => true,
            MappingTarget::SelectedTrack(p) | MappingTarget::SelectedStrip(p) => {
                matches!(p, DeskParam::Mute | DeskParam::Solo | DeskParam::Arm)
            }
            _ => false,
        }
    }

    /// Whether a press fires it rather than setting a value.
    pub fn is_action(&self) -> bool {
        matches!(self, MappingTarget::Action(_))
    }

    /// Whether the address is well formed (what a command accepts).
    pub fn is_valid(&self) -> bool {
        match self {
            MappingTarget::Action(id) => !id.is_empty() && id.len() <= 64,
            MappingTarget::DeviceParam(n) => *n < DEVICE_PARAMS,
            MappingTarget::SelectedStrip(DeskParam::Arm) => false,
            _ => true,
        }
    }

    /// The concrete target a relative one names now (itself for the others); `None` when
    /// nothing is selected or the device has no such parameter.
    pub fn resolve(&self, focus: &Focus) -> Option<MappingTarget> {
        Some(match self {
            MappingTarget::SelectedTrack(p) => {
                let t = focus.track?;
                match p {
                    DeskParam::Volume => MappingTarget::Param(AutoTarget::TrackGain(t)),
                    DeskParam::Pan => MappingTarget::Param(AutoTarget::TrackPan(t)),
                    DeskParam::Mute => MappingTarget::TrackMute(t),
                    DeskParam::Solo => MappingTarget::TrackSolo(t),
                    DeskParam::Arm => MappingTarget::TrackArm(t),
                }
            }
            MappingTarget::SelectedStrip(p) => {
                let s = focus.strip?;
                match p {
                    DeskParam::Volume => MappingTarget::Param(AutoTarget::StripGain(s)),
                    DeskParam::Pan => MappingTarget::Param(AutoTarget::StripPan(s)),
                    DeskParam::Mute => MappingTarget::StripMute(s),
                    DeskParam::Solo => MappingTarget::StripSolo(s),
                    DeskParam::Arm => return None,
                }
            }
            MappingTarget::DeviceParam(n) => {
                MappingTarget::Param(focus.device.get(usize::from(*n))?.clone())
            }
            other => other.clone(),
        })
    }

    /// Whether what it names is in `project` (a relative or a portable target always is):
    /// a mapping whose target vanished is kept, dormant, and works again if undo brings
    /// the target back.
    pub fn exists(&self, project: &Project) -> bool {
        match self {
            MappingTarget::Param(t) => range_of(project, t).is_some(),
            MappingTarget::TrackMute(t)
            | MappingTarget::TrackSolo(t)
            | MappingTarget::TrackArm(t) => project.track(*t).is_some(),
            MappingTarget::StripMute(s) | MappingTarget::StripSolo(s) => {
                project.strip(*s).is_some()
            }
            MappingTarget::EffectBypass(e) | MappingTarget::EffectMix(e) => {
                project.effect_host(*e).is_some()
            }
            _ => true,
        }
    }

    /// The hosted plug-in a hosted parameter target belongs to now (`None` for any other).
    pub fn plugin_of(&self, project: &Project) -> Option<PluginRef> {
        match self {
            MappingTarget::Param(AutoTarget::Plugin { effect, .. }) => {
                let h = project.effect_host(*effect)?;
                project
                    .effect(h, *effect)?
                    .spec
                    .external
                    .as_deref()
                    .map(PluginRef::of)
            }
            MappingTarget::Param(AutoTarget::PluginInstrument { track, .. }) => project
                .track(*track)?
                .instrument
                .external()
                .map(PluginRef::of),
            _ => None,
        }
    }

    /// Where the target is now (0…1 of its travel), for what core knows; `None` for a
    /// hosted parameter (the plug-in owns its range: the interface asks it), an action, a
    /// relative target or one that is gone. `at` is the playhead (the tempo there).
    pub fn current_pos(&self, project: &Project, at: Ticks) -> Option<f32> {
        let on = |b: bool| if b { 1.0 } else { 0.0 };
        match self {
            MappingTarget::Param(t) => {
                if t.is_plugin() {
                    return None;
                }
                let r = range_of(project, t)?;
                let v = crate::automation::range::base_value(project, t)?;
                v.is_finite().then(|| r.pos(v))
            }
            MappingTarget::TrackMute(t) => Some(on(project.track(*t)?.muted)),
            MappingTarget::TrackSolo(t) => Some(on(project.track(*t)?.soloed)),
            MappingTarget::TrackArm(t) => Some(on(project.track(*t)?.armed)),
            MappingTarget::StripMute(s) => Some(on(project.strip(*s)?.muted)),
            MappingTarget::StripSolo(s) => Some(on(project.strip(*s)?.soloed)),
            MappingTarget::EffectBypass(e) => {
                let h = project.effect_host(*e)?;
                Some(on(project.effect(h, *e)?.bypass))
            }
            MappingTarget::EffectMix(e) => {
                let h = project.effect_host(*e)?;
                Some(project.effect(h, *e)?.mix.clamp(0.0, 1.0))
            }
            MappingTarget::Tempo => {
                // What the command writes (ADR 0060 §2): the governing point's own tempo,
                // not the tempo a ramp has reached at the playhead (review 1, #1).
                let bpm = match crate::tempo::edit::point_at(project, at) {
                    None | Some(0) => project.tempo_bpm,
                    Some(i) => project.tempo_points[i].bpm,
                };
                let (lo, hi) = TEMPO_TRAVEL;
                Some((((bpm - lo) / (hi - lo)) as f32).clamp(0.0, 1.0))
            }
            _ => None,
        }
    }

    /// The command that puts the target at `pos` (0…1 of its travel; a switch is on from
    /// one half), or `None` for what core cannot set alone: a hosted parameter (the
    /// interface converts through the plug-in's range and calls [`AutoTarget::command`]),
    /// an action, a relative target, a target that is gone. `at` is the playhead: the tempo
    /// is set where it plays (ADR 0060 §2).
    pub fn command(&self, project: &Project, pos: f32, at: Ticks) -> Option<Command> {
        if !pos.is_finite() {
            return None;
        }
        let pos = pos.clamp(0.0, 1.0);
        let on = pos >= 0.5;
        Some(match self {
            MappingTarget::Param(t) => {
                if t.is_plugin() {
                    return None;
                }
                let v = range_of(project, t)?.value(pos);
                t.command(project, f64::from(v))?
            }
            MappingTarget::TrackMute(id) => {
                project.track(*id)?;
                Command::SetTrackMute { id: *id, muted: on }
            }
            MappingTarget::TrackSolo(id) => {
                project.track(*id)?;
                Command::SetTrackSolo {
                    id: *id,
                    soloed: on,
                }
            }
            MappingTarget::TrackArm(id) => {
                project.track(*id)?;
                Command::SetTrackArmed { id: *id, armed: on }
            }
            MappingTarget::StripMute(id) => {
                project.strip(*id)?;
                Command::SetStripMute { id: *id, muted: on }
            }
            MappingTarget::StripSolo(id) => {
                project.strip(*id)?;
                Command::SetStripSolo {
                    id: *id,
                    soloed: on,
                }
            }
            MappingTarget::EffectBypass(effect) => Command::SetEffectBypass {
                strip: project.effect_host(*effect)?,
                effect: *effect,
                bypass: on,
            },
            MappingTarget::EffectMix(effect) => Command::SetEffectMix {
                strip: project.effect_host(*effect)?,
                effect: *effect,
                mix: pos,
            },
            MappingTarget::Tempo => {
                let (lo, hi) = TEMPO_TRAVEL;
                crate::tempo::edit::set_tempo_at(project, at, lo + f64::from(pos) * (hi - lo))
            }
            _ => return None,
        })
    }
}

/// One binding of a hardware control to a target.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mapping {
    pub id: MappingId,
    pub source: MidiSource,
    pub target: MappingTarget,
    pub mode: MapMode,
    /// The part of the target's travel the control covers, as positions (0…1): the
    /// control's bottom gives `min`, its top `max` — `min` above `max` turns it round.
    pub min: f32,
    pub max: f32,
    /// The response's bend (−1…1, the automation's single curve): 0 is straight, above
    /// slow at first, below fast at first.
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub curve: f32,
    #[serde(default)]
    pub takeover: Takeover,
    /// The value is sent back on the control when the target changes (motor faders, LED
    /// rings), on the output port of the same name. Off by default.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub feedback: bool,
    /// How far one step of a relative encoder moves, in 128ths of the range.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub sensitivity: f32,
    /// For a hosted plug-in's parameter: the plug-in it was learnt on. A parameter id means
    /// something only inside one plug-in, so another plug-in in its place leaves the
    /// mapping dormant (review 1, #56).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<PluginRef>,
}

/// A hosted plug-in's identity: its standard and its id (a CLAP id, a VST3 class id).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PluginRef {
    pub format: crate::external::PluginFormat,
    pub id: String,
}

impl PluginRef {
    pub fn of(p: &crate::external::ExternalPlugin) -> PluginRef {
        PluginRef {
            format: p.format,
            id: p.id.clone(),
        }
    }
}

fn is_zero_f32(v: &f32) -> bool {
    *v == 0.0
}

fn one() -> f32 {
    1.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

/// The most mappings one map keeps.
pub const MAX_MAPPINGS: usize = 1024;

/// The undo label of one controller gesture that moved several targets at once (a
/// `Command::Batch` merged per frame, ADR 0062 §3).
pub const GESTURE_LABEL: &str = "cmd.controllers.gesture";

impl Mapping {
    /// A mapping over the whole travel, straight, picking up, without feedback.
    pub fn new(id: MappingId, source: MidiSource, target: MappingTarget, mode: MapMode) -> Self {
        Mapping {
            id,
            source,
            target,
            mode,
            min: 0.0,
            max: 1.0,
            curve: 0.0,
            takeover: Takeover::default(),
            feedback: false,
            sensitivity: 1.0,
            plugin: None,
        }
    }

    /// Whether the mapping can drive its target in `project`: the target exists and, for a
    /// hosted parameter, it is still the plug-in it was learnt on. A mapping that cannot is
    /// dormant: shown, consuming nothing, alive again when undo brings its target back.
    pub fn alive(&self, project: &Project) -> bool {
        self.target.exists(project)
            && match &self.plugin {
                None => true,
                Some(p) => self.target.plugin_of(project).as_ref() == Some(p),
            }
    }

    /// What a command accepts: a well formed source and target, a range within the
    /// travel, a bend within −1…1, a positive sensitivity.
    pub fn is_valid(&self) -> bool {
        let unit = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        self.source.is_valid()
            && self.target.is_valid()
            && unit(self.min)
            && unit(self.max)
            && self.curve.is_finite()
            && (-1.0..=1.0).contains(&self.curve)
            && self.sensitivity.is_finite()
            && self.sensitivity > 0.0
            && self.sensitivity <= 64.0
    }

    /// The position a control at `x` (0…1 of its travel) asks for: the bend, then the
    /// range.
    pub fn shape(&self, x: f32) -> f32 {
        let x = if x.is_finite() {
            x.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let y = Shape::Curve.at(self.curve, x);
        self.min + (self.max - self.min) * y
    }

    /// The control's position that asks for `pos`: the inverse of [`Self::shape`] (for a
    /// relative step from where the target is, and for feedback). Within the range's ends;
    /// a range of one point answers 0.
    pub fn unshape(&self, pos: f32) -> f32 {
        let span = self.max - self.min;
        if span == 0.0 || !pos.is_finite() {
            return 0.0;
        }
        let y = ((pos - self.min) / span).clamp(0.0, 1.0);
        if self.curve == 0.0 {
            return y;
        }
        // The bend rises monotonically: bisect (deterministic, 32 halvings is below a
        // 14-bit step by far).
        let (mut lo, mut hi) = (0.0f32, 1.0f32);
        for _ in 0..32 {
            let mid = 0.5 * (lo + hi);
            if Shape::Curve.at(self.curve, mid) < y {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }
}

/// The controller mappings of a project (or the user's global map).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ControllerMap {
    #[serde(default)]
    pub mappings: Vec<Mapping>,
}

impl ControllerMap {
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }

    pub fn get(&self, id: MappingId) -> Option<&Mapping> {
        self.mappings.iter().find(|m| m.id == id)
    }

    pub fn position(&self, id: MappingId) -> Option<usize> {
        self.mappings.iter().position(|m| m.id == id)
    }

    /// The mappings that drive `target`.
    pub fn of_target<'a>(&'a self, target: &'a MappingTarget) -> impl Iterator<Item = &'a Mapping> {
        self.mappings.iter().filter(move |m| &m.target == target)
    }

    /// The mappings whose source overlaps `m`'s, other than `m` itself: one control on
    /// several targets, which Cantus allows (FL's *multilink*) and the mappings view flags.
    pub fn sharing<'a>(&'a self, m: &'a Mapping) -> impl Iterator<Item = &'a Mapping> {
        self.mappings
            .iter()
            .filter(move |o| o.id != m.id && o.source.overlaps(&m.source))
    }
}

fn invalid(what: &str) -> CommandError {
    CommandError::InvalidValue(what.to_string())
}

impl ControllerMap {
    /// Keep only what a command would accept: drop a malformed mapping and a second one
    /// with an id already used (a hand-edited or foreign file). Returns how many were
    /// dropped. Run on every project read, so every edit of the map has an exact inverse
    /// (review 1, #8).
    pub fn repair(&mut self) -> usize {
        let before = self.mappings.len();
        let mut seen = std::collections::BTreeSet::new();
        self.mappings.retain(|m| m.is_valid() && seen.insert(m.id));
        self.mappings.truncate(MAX_MAPPINGS);
        before - self.mappings.len()
    }
}

/// `Command::InsertMapping`: a new mapping at `index` (clamped to the end). Refused when
/// it is malformed, its id is taken or the map is full.
pub(crate) fn apply_insert(
    project: &mut Project,
    index: usize,
    mapping: Mapping,
) -> Result<Command, CommandError> {
    if !mapping.is_valid() {
        return Err(invalid("controller mapping"));
    }
    let map = &mut project.controllers;
    if map.get(mapping.id).is_some() {
        return Err(CommandError::DuplicateId(mapping.id.0));
    }
    if map.mappings.len() >= MAX_MAPPINGS {
        return Err(invalid("controller map full"));
    }
    let id = mapping.id;
    let index = index.min(map.mappings.len());
    map.mappings.insert(index, mapping);
    Ok(Command::RemoveMapping { id })
}

/// `Command::RemoveMapping`; the inverse puts it back where it was.
pub(crate) fn apply_remove(project: &mut Project, id: MappingId) -> Result<Command, CommandError> {
    let map = &mut project.controllers;
    let index = map
        .position(id)
        .ok_or_else(|| invalid("controller mapping not found"))?;
    let mapping = map.mappings.remove(index);
    Ok(Command::InsertMapping {
        index,
        mapping: Box::new(mapping),
    })
}

/// `Command::SetMapping`: the mapping with the same id replaced whole (its source learnt
/// again, its target, mode, range, bend, takeover, feedback or sensitivity changed).
pub(crate) fn apply_set(project: &mut Project, mapping: Mapping) -> Result<Command, CommandError> {
    if !mapping.is_valid() {
        return Err(invalid("controller mapping"));
    }
    let map = &mut project.controllers;
    let index = map
        .position(mapping.id)
        .ok_or_else(|| invalid("controller mapping not found"))?;
    let old = std::mem::replace(&mut map.mappings[index], mapping);
    Ok(Command::SetMapping {
        mapping: Box::new(old),
    })
}

#[cfg(test)]
#[path = "controller_tests.rs"]
mod tests;
