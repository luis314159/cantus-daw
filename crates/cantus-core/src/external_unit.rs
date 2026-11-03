//! External units (round 36, ADR 0094): hardware that is part of a track.
//!
//! An **External Instrument** is a synth that is not in the computer: the track's notes go
//! out of a MIDI port, and what the synth plays comes back on a pair of the interface's
//! inputs, to be mixed like any other track. An **External Effect** is the same idea for a
//! processor in a rack: the track's audio goes out of a pair of the interface's outputs and
//! comes back on a pair of inputs.
//!
//! The unit is project data (a [`Project::external_units`] entry, schema 51) and each unit
//! belongs to one track. What it stores is the *routing* (a port by name and ordinal, a
//! channel, interface pairs by number), the **measured latency** of the trip out and back
//! (the automatic ping: [`Measured`]) and a manual trim in milliseconds for what a ping
//! cannot know. The latency is what delay compensation (`cantus-engine::pdc`) is given, so
//! the rest of the mix waits for the hardware; an offline render and a bounce use the
//! stored number, never a fresh measurement, so they are repeatable.
//!
//! Nothing here touches a device: a unit whose interface or port is not there is *said*
//! ([`Availability`]), never loaded blindly.

use crate::model::{Project, TrackId};
use serde::{Deserialize, Serialize};

/// A unit's id (`Project::alloc_id`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ExternalUnitId(pub u64);

impl std::fmt::Display for ExternalUnitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Which side of the interface a unit is.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UnitKind {
    /// MIDI out, audio in.
    Instrument,
    /// Audio out, audio in.
    Effect,
}

impl UnitKind {
    pub const ALL: [UnitKind; 2] = [UnitKind::Instrument, UnitKind::Effect];

    /// The id agents and files name it by (forever).
    pub fn id(self) -> &'static str {
        match self {
            UnitKind::Instrument => "instrument",
            UnitKind::Effect => "effect",
        }
    }

    pub fn from_id(id: &str) -> Option<UnitKind> {
        UnitKind::ALL.into_iter().find(|k| k.id() == id)
    }
}

/// A MIDI output port as the project names it: by name and ordinal (the second
/// "MIDI Out" is ordinal 1), never by index, as a controller mapping does (ADR 0062).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MidiRoute {
    pub port: String,
    #[serde(default)]
    pub ordinal: u8,
    /// 0-based channel (shown as 1–16).
    #[serde(default)]
    pub channel: u8,
}

/// A mono or stereo pair of interface channels, counted from 0.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Pair {
    pub first: u16,
    pub stereo: bool,
}

impl Pair {
    pub fn stereo(first: u16) -> Pair {
        Pair {
            first,
            stereo: true,
        }
    }

    pub fn mono(first: u16) -> Pair {
        Pair {
            first,
            stereo: false,
        }
    }

    /// How many channels it spans.
    pub fn width(self) -> u16 {
        if self.stereo {
            2
        } else {
            1
        }
    }

    /// The channel past its last.
    pub fn end(self) -> u16 {
        self.first + self.width()
    }
}

/// The measured trip (the ping's result), kept with the rate it was counted at and what the
/// interface itself reported then — so a later buffer size re-bases it instead of leaving
/// the gear out of time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measured {
    /// Frames from the processor writing a sample out to the processor reading it back.
    pub frames: u32,
    pub rate: u32,
    /// How far the correlation's peak stood above its spread (the ping accepts ≥ 8), or the
    /// onset's signal over its noise reduced by the repeats' spread.
    pub confidence: f32,
    /// The interface's own reported round trip at the measurement (input + output latency +
    /// one buffer), frames at `rate`; 0 not known.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub rtt: u32,
    /// The buffer it ran then, frames.
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub buffer: u32,
    /// The interface it was measured on.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
}

fn is_zero_u32(x: &u32) -> bool {
    *x == 0
}

/// What the interface reports now (the interface layer fills it from the open device).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Now {
    pub device: String,
    /// Input + output latency + one buffer, frames at `rate`.
    pub rtt: u32,
    pub buffer: u32,
    pub rate: u32,
}

/// How a stored measurement stands against the interface now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Staleness {
    /// Measured with this interface, this buffer: as it is.
    Fresh,
    /// The same interface at another buffer: re-based by the reported round trip's change.
    Rebased {
        measured_buffer: u32,
        now_buffer: u32,
    },
    /// Another interface (or none recorded): the stored number is used as it is and a new
    /// measurement is suggested.
    OtherDevice {
        measured_buffer: u32,
    },
    NotMeasured,
}

impl Staleness {
    /// Whether the screen should suggest measuring again.
    pub fn suggests_ping(&self) -> bool {
        matches!(self, Staleness::OtherDevice { .. })
    }
}

impl Measured {
    /// The longest trip a unit keeps, in seconds (delay compensation's own ceiling is 2¹⁹
    /// samples: 10.9 s at 48 kHz).
    pub const MAX_SECONDS: f64 = 5.0;

    pub fn is_valid(&self) -> bool {
        crate::model::Lateness::RATES.contains(&self.rate)
            && self.frames as f64 <= Self::MAX_SECONDS * self.rate as f64
            && self.confidence.is_finite()
            && self.confidence >= 0.0
    }

    /// In milliseconds.
    pub fn ms(&self) -> f64 {
        self.frames as f64 * 1000.0 / self.rate.max(1) as f64
    }

    /// A measurement with nothing about the interface recorded (the tests, old files).
    pub fn bare(frames: u32, rate: u32, confidence: f32) -> Measured {
        Measured {
            frames,
            rate,
            confidence,
            rtt: 0,
            buffer: 0,
            device: String::new(),
        }
    }

    /// How it stands against `now`.
    pub fn staleness(&self, now: &Now) -> Staleness {
        if self.device.is_empty() || self.device != now.device || self.rtt == 0 {
            return Staleness::OtherDevice {
                measured_buffer: self.buffer,
            };
        }
        if self.buffer == now.buffer && self.rtt_at(now) == self.rtt {
            Staleness::Fresh
        } else {
            Staleness::Rebased {
                measured_buffer: self.buffer,
                now_buffer: now.buffer,
            }
        }
    }

    /// `now`'s round trip in frames at this measurement's rate.
    fn rtt_at(&self, now: &Now) -> u32 {
        if now.rate == 0 || now.rate == self.rate {
            now.rtt
        } else {
            (f64::from(now.rtt) * f64::from(self.rate) / f64::from(now.rate)).round() as u32
        }
    }

    /// The trip as it is with the interface `now`: on the same interface the stored trip
    /// less the round trip it reported then plus the one it reports now (a new buffer size,
    /// a new safety offset); on another interface the stored trip as it is.
    pub fn rebased(&self, now: &Now) -> Measured {
        match self.staleness(now) {
            Staleness::Rebased { .. } => {
                let frames = (i64::from(self.frames) - i64::from(self.rtt)
                    + i64::from(self.rtt_at(now)))
                .max(0) as u32;
                Measured {
                    frames,
                    rtt: self.rtt_at(now),
                    buffer: now.buffer,
                    ..self.clone()
                }
            }
            _ => self.clone(),
        }
    }
}

/// One piece of hardware on one track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExternalUnit {
    pub id: ExternalUnitId,
    pub name: String,
    pub kind: UnitKind,
    /// The track it belongs to: an instrument plays its notes, an effect its audio.
    pub track: TrackId,
    /// Where an instrument's notes go. An effect has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi: Option<MidiRoute>,
    /// Where an effect's audio goes (an instrument has none).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub send: Option<Pair>,
    /// Where the hardware's audio comes back.
    pub ret: Pair,
    /// The interface it was set up with, by name. Empty: whichever is open. A different
    /// one open now makes the unit unavailable ([`Availability::OtherInterface`]).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub device: String,
    /// The ping's result: `None` was never measured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency: Option<Measured>,
    /// What the user adds to the measurement, in ms (a synth's slow attack, a patch
    /// cable): −500…500.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub trim_ms: f32,
    /// The return's level in dB before the strip's own: −60…24, 0 touches no sample.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub return_db: f32,
    /// Off keeps the setup and the track plays as if the unit were not there.
    #[serde(default = "yes")]
    pub enabled: bool,
    /// The track's input as it was before the unit made its return the input, given back when
    /// the unit is removed (`None`: the unit did not change it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore_input: Option<RestoreInput>,
}

/// What a track's input was before a unit took it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestoreInput {
    #[serde(default)]
    pub input: Option<crate::audio::TrackInput>,
}

fn is_zero(x: &f32) -> bool {
    *x == 0.0
}

fn yes() -> bool {
    true
}

/// The limits of a stored unit (a hand-edited file's are checked against them).
pub const MAX_NAME: usize = 64;
pub const MAX_PORT: usize = 160;
pub const MAX_CHANNELS: u16 = 512;
pub const MAX_TRIM_MS: f32 = 500.0;
pub const RETURN_DB: std::ops::RangeInclusive<f32> = -60.0..=24.0;

impl ExternalUnit {
    /// A new instrument on `track`: notes to `route`, audio back on `ret`.
    pub fn instrument(id: ExternalUnitId, track: TrackId, route: MidiRoute, ret: Pair) -> Self {
        ExternalUnit {
            id,
            name: String::new(),
            kind: UnitKind::Instrument,
            track,
            midi: Some(route),
            send: None,
            ret,
            device: String::new(),
            latency: None,
            trim_ms: 0.0,
            return_db: 0.0,
            enabled: true,
            restore_input: None,
        }
    }

    /// A new effect on `track`: audio out on `send`, back on `ret`.
    pub fn effect(id: ExternalUnitId, track: TrackId, send: Pair, ret: Pair) -> Self {
        ExternalUnit {
            id,
            name: String::new(),
            kind: UnitKind::Effect,
            track,
            midi: None,
            send: Some(send),
            ret,
            device: String::new(),
            latency: None,
            trim_ms: 0.0,
            return_db: 0.0,
            enabled: true,
            restore_input: None,
        }
    }

    /// Whether every field is inside its limits and the routing says what the kind needs.
    pub fn is_valid(&self) -> bool {
        let pair_ok = |p: Pair| p.end() <= MAX_CHANNELS;
        self.name.chars().count() <= MAX_NAME
            && self.device.chars().count() <= MAX_PORT
            && pair_ok(self.ret)
            && self.trim_ms.is_finite()
            && self.trim_ms.abs() <= MAX_TRIM_MS
            && self.return_db.is_finite()
            && RETURN_DB.contains(&self.return_db)
            && self.latency.as_ref().is_none_or(|m| m.is_valid())
            && match self.kind {
                UnitKind::Instrument => {
                    self.send.is_none()
                        && self
                            .midi
                            .as_ref()
                            .is_some_and(|m| m.channel < 16 && m.port.chars().count() <= MAX_PORT)
                }
                UnitKind::Effect => self.midi.is_none() && self.send.is_some_and(pair_ok),
            }
    }

    /// The latency delay compensation is given, in frames at `rate`: the measurement plus
    /// the trim, at least zero. A unit never measured, or switched off, reports none (the
    /// screen says "not measured").
    pub fn latency_frames(&self, rate: f64) -> usize {
        if !self.enabled || !rate.is_finite() || rate <= 0.0 {
            return 0;
        }
        let Some(m) = self.latency.as_ref().filter(|m| m.is_valid()) else {
            return 0;
        };
        let measured = m.frames as f64 * rate / m.rate as f64;
        let trim = self.trim_ms as f64 * rate / 1000.0;
        (measured + trim).round().max(0.0) as usize
    }

    /// What the screen says about its latency: never measured, or the milliseconds.
    pub fn is_measured(&self) -> bool {
        self.latency.is_some()
    }
}

/// Whether a unit can be played with what is open now (a pure function of what the
/// interface layer reports; the engine never opens anything for it).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Ready,
    /// The unit is switched off.
    Off,
    /// No audio device is open.
    NoDevice,
    /// A different interface is open than the one the unit was set up with.
    OtherInterface,
    /// The return (or send) channels are beyond the open device's.
    ChannelsMissing,
    /// An effect's send would go out of outputs 1-2, where the master plays.
    SendOnMaster,
    /// The MIDI output is not among the ports present.
    PortMissing,
    /// Its track is gone.
    TrackMissing,
}

impl Availability {
    /// The locale key the screen says it with.
    pub fn key(self) -> &'static str {
        match self {
            Availability::Ready => "external.state.ready",
            Availability::Off => "external.state.off",
            Availability::NoDevice => "external.state.no_device",
            Availability::OtherInterface => "external.state.other_interface",
            Availability::ChannelsMissing => "external.state.channels",
            Availability::SendOnMaster => "external.state.send_master",
            Availability::PortMissing => "external.state.port",
            Availability::TrackMissing => "external.state.track",
        }
    }

    pub fn is_ready(self) -> bool {
        self == Availability::Ready
    }
}

/// What is present: the open audio device (its name and channel counts) and the MIDI output
/// ports (name per entry; the ordinal is the count of equal names before it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Present {
    pub device: Option<String>,
    pub in_channels: u16,
    pub out_channels: u16,
    pub midi_outputs: Vec<String>,
}

impl Present {
    /// Whether the port `route` names is among the outputs.
    pub fn has_port(&self, route: &MidiRoute) -> bool {
        self.midi_outputs
            .iter()
            .filter(|n| **n == route.port)
            .count()
            > usize::from(route.ordinal)
    }
}

/// Judge `unit` against what is present.
pub fn availability(project: &Project, unit: &ExternalUnit, present: &Present) -> Availability {
    if project.track(unit.track).is_none() {
        return Availability::TrackMissing;
    }
    if !unit.enabled {
        return Availability::Off;
    }
    let Some(device) = present.device.as_deref() else {
        return Availability::NoDevice;
    };
    if !unit.device.is_empty() && unit.device != device {
        return Availability::OtherInterface;
    }
    if unit.ret.end() > present.in_channels
        || unit.send.is_some_and(|s| s.end() > present.out_channels)
    {
        return Availability::ChannelsMissing;
    }
    if unit.send.is_some_and(|s| s.first < 2) {
        return Availability::SendOnMaster;
    }
    if unit.midi.as_ref().is_some_and(|m| !present.has_port(m)) {
        return Availability::PortMissing;
    }
    Availability::Ready
}

impl Project {
    /// The unit with this id.
    pub fn external_unit(&self, id: ExternalUnitId) -> Option<&ExternalUnit> {
        self.external_units.iter().find(|u| u.id == id)
    }

    /// The unit of `kind` that belongs to `track`.
    pub fn external_unit_of(&self, track: TrackId, kind: UnitKind) -> Option<&ExternalUnit> {
        self.external_units
            .iter()
            .find(|u| u.track == track && u.kind == kind)
    }

    /// The MIDI routes of the enabled instruments, in the order the units are stored:
    /// `(route, port, unit)`, route 1…255. The engine stamps a unit's notes with its route
    /// and the interface registers the same table with the MIDI-out thread, so both sides
    /// derive it from the project and need no message between them.
    pub fn midi_routes(&self) -> Vec<(u64, MidiRoute, ExternalUnitId)> {
        self.external_units
            .iter()
            .filter(|u| u.enabled && u.kind == UnitKind::Instrument)
            .filter_map(|u| u.midi.clone().map(|m| (route_for(u.id, &m), m, u.id)))
            .collect()
    }

    /// The route of the unit `id` ([`route_for`]); 0 when the unit sends no MIDI or is off.
    pub fn route_of(&self, id: ExternalUnitId) -> u64 {
        self.external_unit(id)
            .filter(|u| u.enabled && u.kind == UnitKind::Instrument)
            .and_then(|u| u.midi.as_ref().map(|m| route_for(u.id, m)))
            .unwrap_or(0)
    }

    /// A fresh id for a unit.
    pub fn new_external_unit_id(&mut self) -> ExternalUnitId {
        ExternalUnitId(self.alloc_id())
    }
}

/// The route a unit's notes are stamped with: its id (never reused) in the low 32 bits and a
/// hash of its port (name and ordinal) in the high 32, never 0. A unit that moves to another
/// port gets another route, so the note-offs for what it held on the old port still go to
/// the old port (the MIDI-out thread keeps a route that left its table for a while), and a
/// unit that is gone never shares a route with one that is new.
pub fn route_for(id: ExternalUnitId, m: &MidiRoute) -> u64 {
    let mut h: u32 = 0x811C_9DC5;
    for b in m.port.bytes().chain([0xFF, m.ordinal]) {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    (u64::from(h) << 32 | (id.0 & 0xFFFF_FFFF)).max(1)
}

/// What the engine takes of a unit with a track's snapshot (ADR 0094): the routing and
/// the latency already in frames, and nothing the engine would look up.
#[derive(Clone, Debug, PartialEq)]
pub struct ExternalLink {
    pub unit: ExternalUnitId,
    pub kind: UnitKind,
    pub midi: Option<MidiRoute>,
    /// The number the engine stamps this unit's notes with and the interface registers its
    /// port under ([`Project::midi_routes`]): the unit's id; 0 for a unit with no MIDI.
    pub route: u64,
    pub send: Option<Pair>,
    pub ret: Pair,
    /// Measured + trim, at the rate the project was last measured at (the engine converts
    /// with [`ExternalLink::latency_at`]).
    pub latency: Option<Measured>,
    pub trim_ms: f32,
    pub return_gain: f32,
}

impl ExternalLink {
    pub fn of(unit: &ExternalUnit, route: u64) -> Self {
        ExternalLink {
            unit: unit.id,
            kind: unit.kind,
            midi: unit.midi.clone(),
            route,
            send: unit.send,
            ret: unit.ret,
            latency: unit.latency.clone(),
            trim_ms: unit.trim_ms,
            return_gain: if unit.return_db == 0.0 {
                1.0
            } else {
                crate::model::db_to_linear(unit.return_db)
            },
        }
    }

    /// The latency in frames at `rate` (as [`ExternalUnit::latency_frames`]).
    pub fn latency_at(&self, rate: f64) -> usize {
        let mut u = ExternalUnit::effect(self.unit, TrackId(0), Pair::mono(0), self.ret);
        u.latency = self.latency.clone();
        u.trim_ms = self.trim_ms;
        u.latency_frames(rate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit() -> ExternalUnit {
        ExternalUnit::instrument(
            ExternalUnitId(9),
            TrackId(1),
            MidiRoute {
                port: "Synth".into(),
                ordinal: 0,
                channel: 0,
            },
            Pair::stereo(2),
        )
    }

    #[test]
    fn latency_is_the_measure_plus_the_trim_at_the_engines_rate() {
        let mut u = unit();
        assert_eq!(u.latency_frames(48_000.0), 0, "never measured: none");
        u.latency = Some(Measured::bare(480, 48_000, 30.0));
        assert_eq!(u.latency_frames(48_000.0), 480);
        assert_eq!(
            u.latency_frames(96_000.0),
            960,
            "a measure converts to the rate"
        );
        u.trim_ms = 2.0;
        assert_eq!(u.latency_frames(48_000.0), 576);
        u.trim_ms = -500.0;
        assert_eq!(u.latency_frames(48_000.0), 0, "never negative");
        u.enabled = false;
        u.trim_ms = 0.0;
        assert_eq!(u.latency_frames(48_000.0), 0, "off reports none");
    }

    #[test]
    fn validity_holds_every_limit_and_the_kind() {
        assert!(unit().is_valid());
        let mut u = unit();
        u.send = Some(Pair::mono(0));
        assert!(!u.is_valid(), "an instrument has no send");
        let mut u = unit();
        u.midi.as_mut().unwrap().channel = 16;
        assert!(!u.is_valid());
        let mut u = unit();
        u.trim_ms = f32::NAN;
        assert!(!u.is_valid());
        let mut u = unit();
        u.return_db = 30.0;
        assert!(!u.is_valid());
        let mut u = unit();
        u.latency = Some(Measured::bare(u32::MAX, 48_000, 9.0));
        assert!(!u.is_valid());
        let e = ExternalUnit::effect(
            ExternalUnitId(1),
            TrackId(1),
            Pair::stereo(0),
            Pair::stereo(0),
        );
        assert!(e.is_valid());
        let mut e2 = e.clone();
        e2.midi = unit().midi;
        assert!(!e2.is_valid(), "an effect has no MIDI");
    }

    #[test]
    fn a_unit_is_said_unavailable_never_loaded_blindly() {
        let mut p = Project::empty("t");
        let track = p.make_track("T");
        let tid = track.id;
        p.tracks.push(track);
        let mut u = unit();
        u.track = tid;
        u.device = "Studio Interface".into();
        let mut here = Present {
            device: Some("Studio Interface".into()),
            in_channels: 8,
            out_channels: 8,
            midi_outputs: vec!["Synth".into()],
        };
        assert_eq!(availability(&p, &u, &here), Availability::Ready);
        here.device = Some("Other".into());
        assert_eq!(availability(&p, &u, &here), Availability::OtherInterface);
        here.device = None;
        assert_eq!(availability(&p, &u, &here), Availability::NoDevice);
        here.device = Some("Studio Interface".into());
        here.in_channels = 3;
        assert_eq!(availability(&p, &u, &here), Availability::ChannelsMissing);
        here.in_channels = 8;
        here.midi_outputs.clear();
        assert_eq!(availability(&p, &u, &here), Availability::PortMissing);
        // The second port of the same name is ordinal 1.
        u.midi.as_mut().unwrap().ordinal = 1;
        here.midi_outputs = vec!["Synth".into()];
        assert_eq!(availability(&p, &u, &here), Availability::PortMissing);
        here.midi_outputs.push("Synth".into());
        assert_eq!(availability(&p, &u, &here), Availability::Ready);
        u.enabled = false;
        assert_eq!(availability(&p, &u, &here), Availability::Off);
        u.track = TrackId(u64::MAX);
        assert_eq!(availability(&p, &u, &here), Availability::TrackMissing);
    }
}
