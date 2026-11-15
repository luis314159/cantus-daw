//! What a Tympanum kit **is** (round 40 phase 3, ADR 0124): its pieces, their articulations
//! and keys, its microphones and where they stand, the default bleed, its room. Data, not
//! project state: a modelled kit's definition is Cantus's table ([`modelled_kits`]); a
//! recorded kit's is read from its pack (DrumGizmo XML, SFZ — `tymp-engine`). The kit's
//! picture (`tymp-ui`) is drawn from the same numbers the engine's virtual mics use: one
//! 3-D model, in centimetres, so what is clicked is what is drawn and what is heard.
//! Owned by `tymp-engine`; the contract declares the shapes.

use super::tympanum::{KitRef, MicPattern, RoomChoice};
use crate::inst_out::{OutputFamily, OutputId};
use cantus_dsp::math;

/// What a piece is: its default family on the mixer, its picture, its key map.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PieceKind {
    Kick,
    Snare,
    HiHat,
    RackTom,
    FloorTom,
    Crash,
    Ride,
    Splash,
    China,
    Stack,
    Cowbell,
    Tambourine,
    Percussion,
}

impl PieceKind {
    pub fn family(self) -> OutputFamily {
        match self {
            PieceKind::Kick => OutputFamily::Kick,
            PieceKind::Snare => OutputFamily::Snare,
            PieceKind::HiHat => OutputFamily::HiHat,
            PieceKind::RackTom | PieceKind::FloorTom => OutputFamily::Toms,
            PieceKind::Crash
            | PieceKind::Ride
            | PieceKind::Splash
            | PieceKind::China
            | PieceKind::Stack => OutputFamily::Cymbals,
            PieceKind::Cowbell | PieceKind::Tambourine | PieceKind::Percussion => {
                OutputFamily::Other
            }
        }
    }
}

/// A place in the room, in centimetres, in the design's frame (`design-p3/kit-model.json`,
/// metres there): **x to the audience's right**, **y towards the audience** (the kit faces
/// +y), **z up**, the origin on the floor under the kick. With a direction: a piece's head
/// normal (the side it is struck from, azimuth and elevation of the batter's normal), a
/// mic's aim.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Place {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Degrees around the vertical: 0 points towards the audience (+y), 90 to the
    /// audience's right (+x).
    pub azimuth: f32,
    /// Degrees above the horizontal (a mic pointing down at a head is −90).
    pub elevation: f32,
}

impl Place {
    /// Where it stands, cm.
    pub fn at(&self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    /// Its direction as a unit vector (x, y, z).
    pub fn dir(&self) -> [f32; 3] {
        let (az, el) = (self.azimuth.to_radians(), self.elevation.to_radians());
        let (sa, ca) = math::sin_cos(az);
        let (se, ce) = math::sin_cos(el);
        [ce * sa, ce * ca, se]
    }

    /// A place from the design's metres (y up, z towards the audience) and a direction
    /// vector in the same frame.
    pub fn from_design(p: [f32; 3], dir: [f32; 3]) -> Place {
        let (x, y, z) = (p[0] * 100.0, p[2] * 100.0, p[1] * 100.0);
        let (dx, dy, dz) = (dir[0], dir[2], dir[1]);
        let h = (dx * dx + dy * dy).sqrt();
        Place {
            x,
            y,
            z,
            azimuth: math::atan2(dx, dy).to_degrees(),
            elevation: math::atan2(dz, h).to_degrees(),
        }
    }
}

/// How a hi-hat articulation sits on the openness controller (CC4, 0 closed … 127 open):
/// the recorded levels a continuous openness crossfades between.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Openness {
    /// 0 … 1, this articulation's recorded openness.
    pub at: f32,
}

/// One way of striking a piece, on its own key (General MIDI's where it has one; the
/// others on 88–106, ADR 0042's rule).
#[derive(Clone, Debug, PartialEq)]
pub struct Articulation {
    /// Stable within the piece (`center`, `edge`, `rimshot`, `sidestick`, `bell`, `bow`,
    /// `choke`, `pedal`, `open`…): what a song's keys keep when the kit changes.
    pub id: String,
    /// Its locale key (`tympanum.art.<id>`) — or, for a pack's own, its name.
    pub label: String,
    pub key: u8,
    /// A hi-hat's articulation: where it sits on the openness controller.
    pub openness: Option<Openness>,
}

/// One piece of the kit.
#[derive(Clone, Debug, PartialEq)]
pub struct PieceDef {
    /// The `n` of its `p{n}.` and `b{n}.` keys: stable for the kit.
    pub index: u16,
    pub name: String,
    /// The locale key of a modelled kit's piece (`tympanum.piece.kick`): what the views
    /// show in the interface's language. Empty for a recorded kit: its pack names it.
    pub label: String,
    pub kind: PieceKind,
    /// Diameter and depth, cm (a 22″ kick is 55.9 × 45.7).
    pub diameter: f32,
    pub depth: f32,
    pub place: Place,
    pub articulations: Vec<Articulation>,
    /// Pieces of one group choke each other (a hi-hat's states, a cymbal's choke).
    pub choke: Option<u8>,
}

/// One microphone (an output of the instrument: ADR 0123).
#[derive(Clone, Debug, PartialEq)]
pub struct MicDef {
    pub output: OutputId,
    pub name: String,
    pub family: OutputFamily,
    /// 1 mono, 2 a stereo pair heard as one strip (overheads, rooms).
    pub channels: u8,
    pub pattern: MicPattern,
    pub place: Place,
    /// The pieces this mic is the *main* mic of (DrumGizmo's `main`): the kit's bleed
    /// fader leaves them alone. Empty: it hears everything as its own (overheads, rooms).
    pub main_for: Vec<u16>,
    /// A modelled kit's mic moves (position, aim, pattern); a recorded kit's cannot.
    pub movable: bool,
    /// Where the kit's own mix puts a mono mic, −1 left … 1 right (the constant-power
    /// law: an SFZ region's `pan`, a modelled close mic's place); a pair's own stereo is
    /// its two channels. Part of what the kit *is*: `o{n}.pan` is a balance on top.
    pub pan: f32,
    /// The kit's own level for this mic, dB (an SFZ library's mic faders at their
    /// `set_ccN`, a modelled kit's balance): what `o{n}.level` 0 dB means.
    pub level_db: f32,
    /// A stereo pair's spacing, cm (an AB pair, a room pair): its two capsules stand
    /// ±half of it along the audience's x, aimed as the pair is. 0 for a mono mic.
    pub spread: f32,
}

/// A recorded kit's default bleed, piece × mic, dB (absent: 0 dB, as recorded).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BleedDefaults {
    pub cells: Vec<(u16, OutputId, f32)>,
}

/// The room a kit is drawn and (modelled) heard in, cm.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RoomDef {
    pub width: f32,
    pub depth: f32,
    pub height: f32,
    /// How far the wall behind the drummer stands from the kit's origin, cm (the kit
    /// faces the room's long side: the audience, +y).
    pub back: f32,
}

/// A whole kit.
#[derive(Clone, Debug, PartialEq)]
pub struct KitDef {
    pub kit: KitRef,
    pub name: String,
    pub modelled: bool,
    pub pieces: Vec<PieceDef>,
    pub mics: Vec<MicDef>,
    pub bleed: BleedDefaults,
    pub room: RoomDef,
    /// The room it plays in by default: its own (a recorded kit's), or one of Cantus's.
    pub default_room: RoomChoice,
    /// The pack's licence (SPDX) and credit line, for a recorded kit: what the kit's view
    /// and the About window show (ADR 0042).
    pub licence: Option<String>,
    pub credit: Option<String>,
    /// The shells' finish, as the scene draws them (Chorda's finishes: lacquers and woods).
    pub finish: crate::inst::chorda_look::Finish,
}

/// One of Cantus's modelled kits, as the kits list shows it (instant, no download).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ModelledKit {
    /// Forever once shipped (`model:<id>` in songs).
    pub id: &'static str,
    /// Its locale key (`tympanum.kit.<id>`).
    pub label_key: &'static str,
}

// The recorded kits' hits and the registry the app fills (`tymp-engine`).
#[path = "tympanum_hits.rs"]
mod hits;
pub use hits::*;

// Cantus's modelled kits: the design's studio kit and its mics, in centimetres.
#[path = "tympanum_studio.rs"]
mod studio;
pub use studio::{modelled_kit, modelled_kits};

// What a mic hears of a piece: the engine's and the views' one estimate.
#[path = "tympanum_heard.rs"]
pub mod heard;
pub use heard::heard_db;

// How a modelled kit's pieces sound (Pulsus's model 4, acoustic).
#[path = "tympanum_voicing.rs"]
mod voicing;
pub use voicing::{measured_set, piece_acoustic, piece_measured, piece_pad};
