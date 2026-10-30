//! The project model: the single source of truth that the UI edits (through commands),
//! the project file stores, and the engine snapshots.
//!
//! This module is deliberately plain data. All behaviour lives in [`crate::command`].

use crate::effect::EffectSlot;
use crate::instrument::InstrumentSpec;
use crate::time::{Ticks, TimeSignature};
use cantus_dsp::math;
use serde::{Deserialize, Serialize};
use std::fmt;

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(
            Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}{}", $prefix, self.0)
            }
        }
    };
}

id_type!(TrackId, "t");
id_type!(ClipId, "c");
id_type!(NoteId, "n");
id_type!(EffectId, "fx");
id_type!(StripId, "s");
id_type!(GroupId, "g");
id_type!(AudioSourceId, "a");
id_type!(LinkId, "l");
id_type!(ModulatorId, "m");

impl StripId {
    /// The master, wherever an effect's host is named by a strip id (ADR 0026): effect
    /// commands, the device windows, the analyser. Ids come from `Project::alloc_id`,
    /// which counts up from 1, so this one is never handed out.
    pub const MASTER: StripId = StripId(u64::MAX);

    pub fn is_master(self) -> bool {
        self == StripId::MASTER
    }
}

/// Per-note expression: bend, pressure and timbre curves on a note (round 33, ADR 0080).
#[path = "note_expr.rs"]
pub mod note_expr;

/// MIDI-style note (0..=127). Middle C is 60.
pub type Pitch = u8;
/// 1..=127. Zero is never stored; a velocity-0 note-on is a note-off in MIDI.
pub type Velocity = u8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: NoteId,
    /// Start in the clip's *content* timeline, not on the arrangement — see [`Clip`].
    pub start: Ticks,
    pub length: Ticks,
    pub pitch: Pitch,
    pub velocity: Velocity,
    /// Muted notes stay in the clip but are not played (FL-style mute tool).
    #[serde(default)]
    pub muted: bool,
    /// The **release velocity** (schema 42, ADR 0068): how fast the key came up, 1…127,
    /// sent with the note-off to native instruments, CLAP and VST3. 0 — every note before
    /// schema 42, and every note drawn without one — is *none given*: the note-off carries
    /// 0, exactly what it carried before.
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub release: Velocity,
    /// The note's **per-note expression** (schema 47, ADR 0080): its pitch bend, pressure
    /// and timbre as curves over its length, from an MPE controller or drawn in the piano
    /// roll — `None` on an ordinary note, which then serialises and plays exactly as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<Box<note_expr::NoteExpr>>,
}

impl Note {
    pub fn end(&self) -> Ticks {
        self.start + self.length
    }
}

/// A clip is a **window onto its content**, not the content itself.
///
/// `notes` live in the clip's own *content* timeline, which is independent of where the
/// clip sits on the arrangement: the note at content tick `t` is heard at
/// `start + t - offset`. Everything that used to be impossible without that distinction
/// falls out of it:
///
/// - **Splitting** moves no notes at all. The right half is the same note list with
///   `start` and `offset` advanced by the same amount, which is why split then glue is an
///   exact round trip.
/// - **Trimming from the left** moves `start` and `offset` together, so the notes stay
///   where the musician put them instead of sliding.
/// - **Slipping** moves `offset` alone: the window stays, the content moves under it.
///
/// Notes whose position falls outside the window are *kept*, not discarded, so shrinking
/// a clip and growing it again is lossless. [`content_length`](Self::content_length) is
/// the opt-in on top of that: with it set, the window shows the content over and over.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: ClipId,
    pub name: String,
    /// Absolute start on the timeline.
    pub start: Ticks,
    /// How much of the timeline the clip occupies.
    pub length: Ticks,
    /// The content tick shown at the clip's left edge. Zero for a clip that has never
    /// been trimmed, split or slipped, which is why it defaults and why every schema-3
    /// project migrates to exactly its old behaviour.
    #[serde(default)]
    pub offset: Ticks,
    /// When set, the content repeats with this period, so a window longer than the
    /// content plays it again.
    ///
    /// `None` — the default — is a clip that plays its content once and simply reveals
    /// more of it as it grows. The two are genuinely different intents and must not be
    /// conflated: dragging the right edge of a clip you shrank has to give you back the
    /// notes you hid, and dragging the right edge of a looping clip has to repeat it.
    /// Ableton Live draws the same distinction with its Loop switch, and for the same
    /// reason.
    #[serde(default)]
    pub content_length: Option<Ticks>,
    /// A muted clip keeps its notes and its place; it just does not sound.
    #[serde(default)]
    pub muted: bool,
    /// Overrides the track colour for this clip alone.
    #[serde(default)]
    pub color: Option<Color>,
    /// The clip's content, in content ticks (see the type docs).
    pub notes: Vec<Note>,
    /// Set for an **audio clip** (ADR 0025): the content is this source, and `notes` is
    /// empty. `None` — every clip before schema 18 — is a note clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<crate::audio::AudioRegion>,
    /// The lane the clip sits on. **0 is the playing lane**; 1, 2, … are take lanes, kept
    /// and shown under the track but never heard (ADR 0025's comping: a comp is lane-0
    /// clips that point into the takes' sources).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub lane: u8,
    /// Set for an **automation clip** (ADR 0027): the content is this curve, and `notes`
    /// is empty. `None` — every clip before schema 19 — is a note or audio clip.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automation: Option<crate::automation::Automation>,
    /// The clip's **link group** (ADR 0037): every clip with the same id is *the same
    /// clip* in another place — FL's pattern clips, Live's linked clips. They share their
    /// content (`notes`, `audio`, `automation`) and how it is named and coloured, and an
    /// edit through any of them changes all of them (`Command::apply` carries it to every
    /// sibling). Where each one sits — `start`, `length`, `offset`, `content_length`,
    /// `muted`, `lane` — stays its own. `None` — every clip before schema 22 — is a clip of
    /// its own. See [`crate::links`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<LinkId>,
    /// Set on a **comp window** (ADR 0058, schema 32): a playing-lane clip that belongs to
    /// the track's comp — made by a swipe or a promote, or a recording laid over the
    /// takes — and so switches with the comp, is kept by an alternate comp and is lit on
    /// its take's lane. Said, never guessed from where the clip lies: a copy pasted over
    /// its takes is no window, a window nudged or slipped off them still is. Each
    /// placement's own, like `lane` (a link group does not share it); the halves of a split
    /// keep it, a copy or a paste does not, a flatten clears it. It means something on the
    /// playing lane and in an alternate comp's kept windows (which all carry it) only.
    /// `false` — every clip before schema 32, except the windows `migrate_31_to_32` found.
    #[serde(default, skip_serializing_if = "is_false")]
    pub comp: bool,
    /// A note clip's **controller lanes** (schema 42, ADR 0068): pitch bend, channel
    /// pressure and MIDI controllers, curves in content ticks drawn under the notes. Content
    /// a link group shares, like the notes. Empty — every clip before schema 42 — plays
    /// exactly as before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lanes: Vec<crate::clip_lanes::ClipLane>,
    /// The clip's groove (schema 43, ADR 0069): its notes played with a template's feel,
    /// non-destructively, until *Commit* writes it into them. Content: a link group shares
    /// it. `None` — every older clip — plays its notes as written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groove: Option<crate::groove::ClipGroove>,
    /// Its notes are already what the track's note effects play (a *Bake to notes*, ADR
    /// 0069): they reach the instrument past the chain. Content, like `groove`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub skip_note_fx: bool,
}

fn is_zero_u8(x: &u8) -> bool {
    *x == 0
}

fn is_false(x: &bool) -> bool {
    !*x
}

impl Clip {
    pub fn end(&self) -> Ticks {
        self.start + self.length
    }

    /// The content tick shown at the clip's left edge.
    pub fn content_start(&self) -> Ticks {
        self.offset
    }

    /// One past the last content tick the window shows, ignoring repeats.
    pub fn content_end(&self) -> Ticks {
        self.offset + self.length
    }

    /// The repeat period, or `None` when the clip plays its content once. A stored period
    /// of zero or less is treated as "no repeat" rather than as an infinite loop: a
    /// hand-edited project file must not be able to hang the compiler.
    pub fn period(&self) -> Option<Ticks> {
        self.content_length.filter(|l| *l > Ticks::ZERO)
    }

    /// The content tick the window would show at timeline tick `t`, ignoring both the
    /// clip's bounds and its repeat. This is the plain window mapping, and it is what an
    /// edit that has to keep the content still — trimming from the left, growing a clip
    /// to fit a recording — moves `offset` to preserve.
    pub fn content_of(&self, t: Ticks) -> Ticks {
        self.offset + (t - self.start)
    }

    /// The content tick actually heard at timeline tick `t`, or `None` outside the clip.
    /// The inverse of [`timeline_positions`](Self::timeline_positions).
    pub fn content_shown_at(&self, t: Ticks) -> Option<Ticks> {
        if t < self.start || t >= self.end() {
            return None;
        }
        let d = (t - self.start).0;
        Some(match self.period() {
            Some(period) => self.offset + Ticks(d.rem_euclid(period.0)),
            None => self.offset + Ticks(d),
        })
    }

    /// Where a content tick lands on the timeline. See [`ClipWindow::timeline_positions`].
    pub fn timeline_positions(&self, content: Ticks, out: &mut Vec<Ticks>) {
        self.window().timeline_positions(content, out)
    }

    /// Just the geometry, without the notes.
    pub fn window(&self) -> ClipWindow {
        ClipWindow {
            start: self.start,
            length: self.length,
            offset: self.offset,
            content_length: self.content_length,
        }
    }

    pub fn note(&self, id: NoteId) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }

    pub fn note_mut(&mut self, id: NoteId) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }
}

/// How much a centred signal is attenuated, relative to hard-panned.
///
/// **This is a property of the project, not a preference.** It decides what a mix
/// *renders as*, so it travels with the file (ADR 0012); changing it changes the balance
/// of every panned track in the piece, which is exactly why it must not live in a
/// settings dialog that follows the machine instead of the music.
///
/// All four laws agree at the extremes — hard left is the whole signal in the left
/// channel — and differ in the middle. The name is the gain at the centre:
///
/// | Law | Centre | Constant | Mono sum |
/// |---|---|---|---|
/// | [`ZeroDb`](Self::ZeroDb) | 0 dB | neither | rises 3 dB at centre |
/// | [`Minus3dB`](Self::Minus3dB) | −3.0 dB | power | rises 3 dB at centre |
/// | [`Minus4_5dB`](Self::Minus4_5dB) | −4.5 dB | (the compromise) | rises 1.5 dB |
/// | [`Minus6dB`](Self::Minus6dB) | −6.0 dB | amplitude | flat |
///
/// Constant *power* is right when the two speakers are heard as a stereo field, which is
/// how music is listened to; constant *amplitude* is right when the two channels will be
/// summed, which is what a mono club rig or a phone speaker does. −4.5 dB is the
/// geometric mean of the two and is a defensible hedge — it is Pro Tools's default.
///
/// `ZeroDb` is not a compromise but a compatibility guarantee: it is exactly what Cantus
/// did before this setting existed, so every project written by an earlier version
/// migrates to it and renders bit-identically. New projects start at −3 dB.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PanLaw {
    /// No centre attenuation: panning only lowers the far channel, and a centred track is
    /// unity. Logic calls this "0 dB". Cantus's behaviour before ADR 0012.
    #[serde(rename = "0")]
    ZeroDb,
    /// Constant power: `L² + R²` is the same at every pan position.
    #[default]
    #[serde(rename = "-3")]
    Minus3dB,
    /// Halfway between constant power and constant amplitude, in decibels.
    #[serde(rename = "-4.5")]
    Minus4_5dB,
    /// Constant amplitude: `L + R` is the same at every pan position, so a mono sum does
    /// not change level as a track is panned.
    #[serde(rename = "-6")]
    Minus6dB,
}

impl PanLaw {
    pub const ALL: [PanLaw; 4] = [
        PanLaw::ZeroDb,
        PanLaw::Minus3dB,
        PanLaw::Minus4_5dB,
        PanLaw::Minus6dB,
    ];

    /// Stable id for the project file and for the UI's combo box.
    pub fn id(self) -> &'static str {
        match self {
            PanLaw::ZeroDb => "0",
            PanLaw::Minus3dB => "-3",
            PanLaw::Minus4_5dB => "-4.5",
            PanLaw::Minus6dB => "-6",
        }
    }

    pub fn from_id(id: &str) -> Option<PanLaw> {
        PanLaw::ALL.into_iter().find(|l| l.id() == id)
    }

    pub fn label_key(self) -> &'static str {
        match self {
            PanLaw::ZeroDb => "panlaw.zero",
            PanLaw::Minus3dB => "panlaw.minus3",
            PanLaw::Minus4_5dB => "panlaw.minus45",
            PanLaw::Minus6dB => "panlaw.minus6",
        }
    }

    /// The `(left, right)` multipliers for a pan position in −1..=1.
    ///
    /// Hot path: this runs once per track per block, never per sample, and it allocates
    /// nothing.
    pub fn gains(self, pan: f32) -> (f32, f32) {
        let p = if pan.is_finite() {
            pan.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let angle = (p + 1.0) * 0.25 * std::f32::consts::PI;
        let (power_l, power_r) = (math::cos(angle), math::sin(angle));
        let (amp_l, amp_r) = ((1.0 - p) * 0.5, (1.0 + p) * 0.5);
        match self {
            // Equal-power taper on the far channel, and the near channel simply left
            // alone — a pan pot that can never make a track louder than its fader says.
            // The balance formulation: exactly unity at centre and on the near side
            // (`DSP_VERSION` 4; `cos(π/4)·√2` alone is 0.99999994 in `f32`).
            PanLaw::ZeroDb => balance_gains(p),
            PanLaw::Minus3dB => (power_l, power_r),
            PanLaw::Minus4_5dB => ((power_l * amp_l).sqrt(), (power_r * amp_r).sqrt()),
            PanLaw::Minus6dB => (amp_l, amp_r),
        }
    }
}

/// A clip's geometry with none of its content: where its window sits, and how the content
/// maps into it.
///
/// This exists so a view can ask "where would this note be if the clip started a bar later"
/// without cloning the clip — and a clip can hold twenty thousand notes, so cloning one per
/// frame to preview a drag is not a small mistake.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ClipWindow {
    pub start: Ticks,
    pub length: Ticks,
    pub offset: Ticks,
    pub content_length: Option<Ticks>,
}

impl ClipWindow {
    pub fn period(&self) -> Option<Ticks> {
        self.content_length.filter(|l| *l > Ticks::ZERO)
    }

    /// Where a content tick lands on the timeline, for every repetition that falls inside
    /// the window. Ascending, and empty when the content tick is never shown — because it
    /// is outside the window, or outside the repeat's brace.
    ///
    /// This is the one place the window/content mapping is written down; the snapshot, the
    /// piano roll and the arrangement all go through it, so they cannot disagree.
    pub fn timeline_positions(&self, content: Ticks, out: &mut Vec<Ticks>) {
        out.clear();
        let offset_in_window = content - self.offset;
        match self.period() {
            None => {
                if offset_in_window >= Ticks::ZERO && offset_in_window < self.length {
                    out.push(self.start + offset_in_window);
                }
            }
            Some(period) => {
                // The period is a brace around the content, not a modulus applied to it:
                // content outside `[offset, offset + period)` is not heard at all. The
                // alternative — wrapping every note into the first period — would mean
                // that switching the repeat on piled the whole rest of the clip onto its
                // first bar, which is nobody's intent. Live's loop brace works this way.
                let d0 = offset_in_window.0;
                if d0 < 0 || d0 >= period.0 {
                    return;
                }
                let mut d = d0;
                while d < self.length.0 {
                    out.push(self.start + Ticks(d));
                    d += period.0;
                }
            }
        }
    }
}

/// A track's colour, stored as sRGB so themes can derive contrast from it.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }

    /// A pleasant, distinguishable palette for new tracks.
    pub const PALETTE: [Color; 8] = [
        Color::rgb(0xE0, 0x8E, 0x3A),
        Color::rgb(0x4F, 0xA3, 0xC7),
        Color::rgb(0xB3, 0x6B, 0xD6),
        Color::rgb(0x5C, 0xB8, 0x6E),
        Color::rgb(0xD9, 0x5D, 0x5D),
        Color::rgb(0xD8, 0xC0, 0x4A),
        Color::rgb(0x4A, 0x8F, 0xD8),
        Color::rgb(0xC9, 0x6F, 0x9E),
    ];
}

/// A picture for a track, read faster than a name at 40 pixels.
///
/// `None` is not "no icon": it means **derive one from the instrument**, so a kit gets a
/// drum and a synth gets a waveform without anybody choosing. Storing a derived value
/// would freeze it the moment the track's instrument changed.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackIcon {
    Drums,
    Keys,
    Strings,
    Vocal,
    Synth,
    Fx,
}

impl TrackIcon {
    pub const ALL: [TrackIcon; 6] = [
        TrackIcon::Drums,
        TrackIcon::Keys,
        TrackIcon::Strings,
        TrackIcon::Vocal,
        TrackIcon::Synth,
        TrackIcon::Fx,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            TrackIcon::Drums => "icon.drums",
            TrackIcon::Keys => "icon.keys",
            TrackIcon::Strings => "icon.strings",
            TrackIcon::Vocal => "icon.vocal",
            TrackIcon::Synth => "icon.synth",
            TrackIcon::Fx => "icon.fx",
        }
    }
}

/// How tall a track is drawn, as a multiple of the arrangement's base row.
///
/// A multiple rather than a number of pixels, because the interface scales (round 10's
/// `ui_scale`) and a track stored as "72 points" would be the wrong size on the next
/// machine. The limits are what stays *usable*: below a third of a row a clip's name has
/// nowhere to go, and above four the divider is off screen before you have finished
/// dragging it.
pub const MIN_TRACK_HEIGHT: f32 = 0.34;
pub const MAX_TRACK_HEIGHT: f32 = 4.0;

fn unit_height() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub color: Color,
    /// How tall this track is drawn, as a multiple of the base row.
    ///
    /// **In the project, not in settings.** It does not change what the piece renders as,
    /// so ADR 0012's rule does not force it here — but it is a property of *this track in
    /// this song* (the drum track you keep tall so you can see the pattern), and a height
    /// that did not travel with the file would have to be set again on every machine and
    /// after every reopen, which is the same as not having it.
    #[serde(default = "unit_height")]
    pub height: f32,
    /// `None` derives one from the instrument; see [`TrackIcon`].
    #[serde(default)]
    pub icon: Option<TrackIcon>,
    pub instrument: InstrumentSpec,
    pub clips: Vec<Clip>,
    /// The channel's own level, in decibels, **before** its mixer strip — FL's channel
    /// volume. Unity by default; the strip's fader is the one a mix is balanced on.
    pub gain_db: f32,
    /// The channel's own placement, −1..=1, before the strip. A *balance*, not the pan
    /// law: the law is applied once, at the strip, and applying it twice was ADR 0012's
    /// master-pan bug. Centre is exactly unity.
    pub pan: f32,
    /// The mixer strip this track feeds. `None` goes straight to the master.
    ///
    /// Several tracks may feed one strip — that is a bus, and it is what lets a kick and a
    /// snare on their own tracks share one compressor. A new track takes the first strip
    /// no other track is using, so the mixer fills in left to right.
    #[serde(default)]
    pub strip: Option<StripId>,
    pub muted: bool,
    pub soloed: bool,
    /// The audio input this track records and monitors (ADR 0025). `None` has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<crate::audio::TrackInput>,
    /// Armed for recording: the record button records this track (its input when it has
    /// one, the keyboard and MIDI when it has an instrument).
    #[serde(default)]
    pub armed: bool,
    /// The take lanes are shown under the track.
    #[serde(default)]
    pub takes_open: bool,
    /// The automation lanes are shown under the track (ADR 0027). Shown unless hidden.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub automation_open: bool,
    /// How the track's automation behaves while the song plays (schema 42, ADR 0068):
    /// Read — every older project — plays it; Write, Touch and Latch write it.
    #[serde(default, skip_serializing_if = "crate::automation::AutoMode::is_read")]
    pub auto_mode: crate::automation::AutoMode,
    /// The pitch lane is shown under the track (schema 27, ADR 0054): its audio clips'
    /// notes, editable in place.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pitch_open: bool,
    /// The track's modulators (ADR 0057): LFOs, envelopes, followers, steps and random
    /// sources driving any parameter. Schema 28; empty is every older project.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modulators: Vec<crate::modulator::Modulator>,
    /// The track's alternate comps (schema 29, ADR 0058): Logic's A/B/C. Empty — every
    /// older project — is the one comp lane 0 holds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comps: Vec<crate::comp::Comp>,
    /// Which of `comps` plays (its windows are lane 0's).
    #[serde(default, skip_serializing_if = "is_zero_u8")]
    pub active_comp: u8,
    /// Frozen on its own (schema 37, ADR 0064): the instrument and the clips play from a
    /// render — the track's level, balance, mute and solo stay live. `None` plays live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<crate::freeze::Frozen>,
    /// How long this track delays its source before its level (schema 38, ADR 0064 §6): a
    /// bounce in place writes its audio on the grid and keeps here the latency what it
    /// replaced was heard with, so the track plays everything it holds — the bounce, any
    /// clip or take put on it later — that much later and reports it as its own: its
    /// level, its strip's fader, sends and keys meet the audio as late as they did before
    /// (review 2), while the audio itself moves, pastes and exports on the grid (review 3:
    /// the latency was in the bounce's file, and every other path missed it). `None`: no
    /// delay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lateness: Option<Lateness>,
    /// The note effects before its instrument, in order (schema 43, ADR 0069): an
    /// arpeggiator, chords, a scale. Empty — every older project — plays its notes as they
    /// are.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub note_fx: Vec<crate::notefx::NoteEffect>,
    /// How its part is written as a score (schema 45, ADR 0075): clef, staves, the
    /// hands' split, transposition, dynamics, reading grid, the part's name. The default —
    /// every older track — is everything automatic, and plays nothing differently.
    #[serde(
        default,
        skip_serializing_if = "crate::notation::ScoreSettings::is_default"
    )]
    pub score: crate::notation::ScoreSettings,
    /// The track's column of the session view (schema 46, ADR 0076): its slots that hold
    /// something, by scene id. Empty — every older track — has none. // r33:session
    #[serde(default, skip_serializing_if = "Vec::is_empty")] // r33:session
    pub slots: Vec<crate::session::Slot>, // r33:session
}

/// A delay a track keeps (`Track::lateness`): `frames` at `rate` (the engine converts it to
/// its own rate).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lateness {
    pub frames: u64,
    pub rate: u32,
}

impl Lateness {
    /// The longest delay a track keeps, in seconds (compensation's own ceiling is 2¹⁹
    /// samples: 10.9 s at 48 kHz).
    pub const MAX_SECONDS: f64 = 10.0;
    /// The rates it can be counted at.
    pub const RATES: std::ops::RangeInclusive<u32> = 8_000..=768_000;

    /// Whether a bounce can have kept it: a rate a device runs at, at most
    /// [`Self::MAX_SECONDS`] (review 3: a hand-edited file's went into the plan unchecked).
    pub fn is_valid(&self) -> bool {
        Self::RATES.contains(&self.rate)
            && self.frames as f64 <= Self::MAX_SECONDS * self.rate as f64
    }

    /// In frames at `rate`; 0 for one that is not valid.
    pub fn at(&self, rate: f64) -> usize {
        if !self.is_valid() || !rate.is_finite() || rate <= 0.0 {
            return 0;
        }
        (self.frames as f64 * rate / self.rate as f64).round() as usize
    }
}

fn is_true(x: &bool) -> bool {
    *x
}

/// A tap from one strip into another that does not replace the strip's output: the
/// strip still goes where `output` says, and a copy at `level` goes to `to` as well.
/// Taken where [`Send::tap`] says: **post-fader** by default — after the strip's gain, pan
/// and mute, which is FL's default and what a reverb return wants (turn the source down
/// and the reverb follows) — or pre-fader.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Send {
    pub to: StripId,
    /// Linear, 0…1. Unity sends the whole signal at the tap.
    pub level: f32,
    /// A send switched off keeps its level, so it can be switched back on where it was.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Where the copy is taken (schema 23, ADR 0041). Post-fader unless set, and not
    /// written to the file when post, so a project without a pre-fader send is the file it
    /// was before.
    #[serde(default, skip_serializing_if = "SendTap::is_post")]
    pub tap: SendTap,
}

/// Where on the source strip a send takes its copy (schema 23, ADR 0041).
///
/// **Built to grow.** Other tap points (pre-inserts, post-inserts-pre-EQ, …) may be added
/// as new variants. The rule for adding one: give it a name of its own (never rename or
/// reuse a variant — files store the name), handle it in every `match` on the tap (the
/// engine's send feed above all; no wildcard arm may hide a new one), and bump the schema
/// so an older build refuses a file that uses it instead of failing to read it. A file
/// that never uses the new variant keeps its meaning everywhere, because `Post` is not
/// written at all.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SendTap {
    /// After the inserts, the EQ, the fader and the pan law — FL's only kind, and what a
    /// reverb return wants.
    #[default]
    Post,
    /// After the inserts and the EQ, **before** the fader and the pan law: a cue mix, or a
    /// return that keeps its level while the source is faded out. Mute and solo still
    /// silence it (the owner's answer 4).
    Pre,
}

impl SendTap {
    pub const ALL: [SendTap; 2] = [SendTap::Post, SendTap::Pre];

    /// The default, which the file does not write.
    pub fn is_post(&self) -> bool {
        *self == SendTap::Post
    }

    /// The i18n key of its short name on a switch or a pill (`POST`, `PRE`).
    pub fn label_key(self) -> &'static str {
        match self {
            SendTap::Post => "routing.tap.post",
            SendTap::Pre => "routing.tap.pre",
        }
    }
}

impl Send {
    /// A unity (0 dB), post-fader send, switched on: what every new send is (the owner's
    /// answer 3 — a drag to a reverb return is heard at once).
    pub fn new(to: StripId) -> Self {
        Send {
            to,
            level: 1.0,
            enabled: true,
            tap: SendTap::Post,
        }
    }

    /// What actually reaches the destination.
    pub fn effective_level(&self) -> f32 {
        if self.enabled {
            self.level
        } else {
            0.0
        }
    }
}

/// How many sends a strip can carry. Every strip has this many preallocated on the audio
/// thread, so it is a limit rather than a taste.
pub const MAX_SENDS: usize = 16;

/// How many inserts a strip can carry. FL's ten; the audio thread has that many units'
/// worth of bookkeeping per strip preallocated.
pub const MAX_INSERTS: usize = 10;

/// One band of the strip's own equaliser.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EqBand {
    pub freq_hz: f32,
    /// Boost or cut, −18…+18. Zero is the identity for the band.
    pub gain_db: f32,
    /// Bell width (peak band) or shelf slope (shelving bands). 0.1…10, 0.71 by default.
    pub q: f32,
}

/// Which number of a band a command sets.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EqField {
    Freq,
    Gain,
    Q,
}

pub const EQ_MIN_HZ: f32 = 20.0;
pub const EQ_MAX_HZ: f32 = 20_000.0;
pub const EQ_MAX_GAIN_DB: f32 = 18.0;
pub const EQ_MIN_Q: f32 = 0.1;
pub const EQ_MAX_Q: f32 = 10.0;

/// The three-band equaliser every strip carries, after its inserts and before its width
/// and pan — where a console's channel EQ sits. Low and high bands are **shelves**, the
/// middle is a **bell**: the shape a channel EQ has had since channels had them, and the
/// one that does a tone correction in three moves instead of six.
///
/// All three at 0 dB is the identity, and the audio thread skips it entirely then, so a
/// project that never touches the EQ renders exactly as it did before there was one.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StripEq {
    pub bands: [EqBand; 3],
}

impl Default for StripEq {
    fn default() -> Self {
        StripEq {
            bands: [
                EqBand {
                    freq_hz: 100.0,
                    gain_db: 0.0,
                    q: 0.71,
                },
                EqBand {
                    freq_hz: 1_000.0,
                    gain_db: 0.0,
                    q: 0.71,
                },
                EqBand {
                    freq_hz: 8_000.0,
                    gain_db: 0.0,
                    q: 0.71,
                },
            ],
        }
    }
}

impl StripEq {
    /// Nothing to do: every band at 0 dB.
    pub fn is_flat(&self) -> bool {
        self.bands.iter().all(|b| b.gain_db == 0.0)
    }

    /// Set one number of one band, clamped to its range. `None` for a band that is not
    /// there or a value that is not a number.
    pub fn set(&mut self, band: usize, field: EqField, value: f32) -> Option<f32> {
        if !value.is_finite() {
            return None;
        }
        let b = self.bands.get_mut(band)?;
        let v = match field {
            EqField::Freq => {
                b.freq_hz = value.clamp(EQ_MIN_HZ, EQ_MAX_HZ);
                b.freq_hz
            }
            EqField::Gain => {
                b.gain_db = value.clamp(-EQ_MAX_GAIN_DB, EQ_MAX_GAIN_DB);
                b.gain_db
            }
            EqField::Q => {
                b.q = value.clamp(EQ_MIN_Q, EQ_MAX_Q);
                b.q
            }
        };
        Some(v)
    }

    pub fn get(&self, band: usize, field: EqField) -> f32 {
        let b = self.bands[band.min(2)];
        match field {
            EqField::Freq => b.freq_hz,
            EqField::Gain => b.gain_db,
            EqField::Q => b.q,
        }
    }
}

fn default_true() -> bool {
    true
}

/// Where a strip sits on the desk. FL's three docks: the **left** and **right** ones are
/// pinned — always on screen, whatever the scroll — and the **middle** one scrolls. A new
/// project docks its last three strips on the right, which is where returns and buses
/// live, so they stay in view while forty channels scroll past.
///
/// A view thing stored in the project, like a track's height: it changes nothing that
/// renders, but a desk laid out by hand must travel with the file.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dock {
    Left,
    #[default]
    Middle,
    Right,
}

impl Dock {
    pub const ALL: [Dock; 3] = [Dock::Left, Dock::Middle, Dock::Right];

    pub fn label_key(self) -> &'static str {
        match self {
            Dock::Left => "mixer.dock.left",
            Dock::Middle => "mixer.dock.middle",
            Dock::Right => "mixer.dock.right",
        }
    }
}

/// How many strips a new project docks on the right.
pub const DEFAULT_RIGHT_DOCK: usize = 3;

/// A named, coloured group of strips: a separator on the desk with the group's name, which
/// folds its members away when collapsed. A group is **how the desk is organised**, not how
/// the signal flows — that is a bus (ADR 0014) — so it changes nothing that renders, and a
/// strip can be in a group and feed any bus it likes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StripGroup {
    pub id: GroupId,
    pub name: String,
    pub color: Color,
    /// Members hidden behind the separator.
    #[serde(default)]
    pub collapsed: bool,
}

/// One channel of the mixer.
///
/// Strips exist **whether or not anything feeds them**, numbered from 1, the way a desk
/// has channels before anything is plugged in: a project opens with sixteen, and more can
/// be added. A track feeds a strip ([`Track::strip`]); a strip feeds the master or
/// another strip ([`MixerStrip::output`]), which is what a bus is. The inserts, the width
/// and the polarity live here, because they belong to the *channel of the mixer* and not
/// to whichever instrument happens to be playing into it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MixerStrip {
    pub id: StripId,
    /// Empty means "unnamed": the mixer then shows the name of the first track feeding
    /// the strip, or its number.
    pub name: String,
    pub gain_db: f32,
    /// −1..=1, through the project's pan law.
    pub pan: f32,
    /// Stereo width. 0 is mono, 1 is as recorded, 2 is twice as wide.
    ///
    /// Mono is modelled as width zero rather than as a switch of its own, because that is
    /// what mono *is*: one control, with mono at its end stop, instead of two controls
    /// that can disagree.
    #[serde(default = "unit_width")]
    pub width: f32,
    /// Flip the polarity of the channel (the console's Ø). The cheapest way to find out
    /// why two tracks together are quieter than either one alone.
    #[serde(default)]
    pub phase_invert: bool,
    #[serde(default)]
    pub muted: bool,
    /// Solo, FL's way: the soloed strip plays, and so does everything routed **into** it
    /// and everything it is routed **to** — a soloed bus is heard with its inputs, and a
    /// soloed channel is heard through its bus. Resolved in `Snapshot::compile`.
    #[serde(default)]
    pub soloed: bool,
    /// Swap left and right. An input-stage switch beside the polarity, as on FL's strip.
    #[serde(default)]
    pub swap_channels: bool,
    /// The whole insert chain in or out, without touching each slot's own bypass — FL's
    /// "disable all FX" lamp.
    #[serde(default = "default_true")]
    pub fx_enabled: bool,
    /// Insert effects, in processing order. At most [`MAX_INSERTS`].
    #[serde(default)]
    pub effects: Vec<EffectSlot>,
    /// The strip's own three-band EQ, after the inserts.
    #[serde(default)]
    pub eq: StripEq,
    /// Where the strip's output goes: another strip, or the master when `None`. A strip
    /// can never feed itself or anything that feeds it; the command refuses cycles.
    #[serde(default)]
    pub output: Option<StripId>,
    /// Post-fader taps into other strips, besides `output`. One per destination; the
    /// same cycle rule as `output`, since a send is a route too. At most [`MAX_SENDS`].
    #[serde(default)]
    pub sends: Vec<Send>,
    /// A colour of the strip's own; `None` borrows the first feeding track's (or, failing
    /// that, its group's).
    #[serde(default)]
    pub color: Option<Color>,
    /// Which dock the strip sits in.
    #[serde(default)]
    pub dock: Dock,
    /// The group whose separator the strip sits behind, if any.
    #[serde(default)]
    pub group: Option<GroupId>,
    /// The strip's modulators (ADR 0057, schema 28).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modulators: Vec<crate::modulator::Modulator>,
    /// Frozen (schema 37, ADR 0064): everything the strip gathers and its chain up to the
    /// width play from a render — width, pan, fader, mute, solo and the sends stay live.
    /// `None` plays live.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<crate::freeze::Frozen>,
}

impl MixerStrip {
    pub fn effect(&self, id: EffectId) -> Option<&EffectSlot> {
        self.effects.iter().find(|e| e.id == id)
    }

    pub fn effect_mut(&mut self, id: EffectId) -> Option<&mut EffectSlot> {
        self.effects.iter_mut().find(|e| e.id == id)
    }

    pub fn effect_index(&self, id: EffectId) -> Option<usize> {
        self.effects.iter().position(|e| e.id == id)
    }

    pub fn send_to(&self, to: StripId) -> Option<&Send> {
        self.sends.iter().find(|s| s.to == to)
    }

    pub fn send_to_mut(&mut self, to: StripId) -> Option<&mut Send> {
        self.sends.iter_mut().find(|s| s.to == to)
    }

    /// Every strip this one feeds: its output and every send, enabled or not — a switched
    /// off send is still a route for the purposes of ordering, so switching it on can
    /// never change the processing order under the audio thread's feet.
    pub fn feeds(&self) -> impl Iterator<Item = StripId> + '_ {
        self.output
            .into_iter()
            .chain(self.sends.iter().map(|s| s.to))
    }

    /// The strips this one's effects are keyed from (their sidechains).
    pub fn keys(&self) -> impl Iterator<Item = StripId> + '_ {
        self.effects.iter().filter_map(|e| e.sidechain)
    }
}

/// How many strips a new project opens with. Enough to give a full band a channel each
/// and a few buses without asking; the mixer's `+` adds more.
pub const DEFAULT_STRIPS: usize = 16;

fn unit_width() -> f32 {
    1.0
}

/// Widest a width control goes. Beyond this the side signal swamps the middle and the
/// mono sum falls apart, so it is a limit rather than a taste.
pub const MAX_WIDTH: f32 = 2.0;

/// Mid/side width, in place. `width == 1.0` is the identity and is skipped entirely, so
/// a project that never touches the control renders exactly as it did before there was
/// one — which is what the null test asserts.
#[inline]
pub fn apply_width(left: &mut [f32], right: &mut [f32], width: f32) {
    // Unity and nonsense both mean "leave the samples alone". Unity has to be an early
    // return rather than a `w = 1.0`, because `(l+r)/2 + (l-r)/2` is not bit-identical to
    // `l` in floating point, and "the control is at its default" must mean *untouched*.
    if width == 1.0 || !width.is_finite() {
        return;
    }
    let w = width.clamp(0.0, MAX_WIDTH);
    for (l, r) in left.iter_mut().zip(right.iter_mut()) {
        let mid = (*l + *r) * 0.5;
        let side = (*l - *r) * 0.5 * w;
        *l = mid + side;
        *r = mid - side;
    }
}

pub const MIN_GAIN_DB: f32 = -96.0;
pub const MAX_GAIN_DB: f32 = 12.0;

impl Track {
    /// The icon to draw: the one that was chosen, or the one the instrument implies.
    pub fn icon(&self) -> TrackIcon {
        self.icon.unwrap_or(match &self.instrument {
            InstrumentSpec::Pulsus(_) => TrackIcon::Drums,
            InstrumentSpec::Firmus(_) => TrackIcon::Synth,
            InstrumentSpec::Native(n) => n.kind.icon(),
            InstrumentSpec::External(_) => TrackIcon::Synth,
        })
    }

    /// The stored height, clamped — a hand-edited project file must not be able to make a
    /// track two pixels tall or taller than the window.
    pub fn height(&self) -> f32 {
        if self.height.is_finite() {
            self.height.clamp(MIN_TRACK_HEIGHT, MAX_TRACK_HEIGHT)
        } else {
            1.0
        }
    }

    /// The clip `id`: the arrangement's, or one of the track's session clips (ADR 0076: a
    /// session clip is edited by every clip command as an arrangement clip is).
    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips
            .iter()
            .find(|c| c.id == id)
            .or_else(|| crate::session::find_clip(self, id)) // r33:session
    }

    pub fn clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        if let Some(i) = self.clips.iter().position(|c| c.id == id) {
            return Some(&mut self.clips[i]);
        }
        crate::session::find_clip_mut(self, id) // r33:session
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LoopRegion {
    pub start: Ticks,
    pub end: Ticks,
}

impl LoopRegion {
    pub fn length(&self) -> Ticks {
        self.end - self.start
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Master {
    pub gain_db: f32,
    /// The master has the same three controls as a channel strip, for the same reasons:
    /// a mix can be off-centre, too wide, or out of polarity against a reference.
    #[serde(default)]
    pub pan: f32,
    #[serde(default = "unit_width")]
    pub width: f32,
    #[serde(default)]
    pub phase_invert: bool,
    /// The master's own insert chain (ADR 0026), FL's master track: run on the summed mix
    /// before the master's gain, balance and width. Addressed by every effect command as
    /// [`StripId::MASTER`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<EffectSlot>,
    /// The whole master chain in or out.
    #[serde(default = "yes")]
    pub fx_enabled: bool,
    /// The master's modulators (ADR 0057, schema 28).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub modulators: Vec<crate::modulator::Modulator>,
    /// How the master's automation behaves while the song plays (schema 42, ADR 0068).
    #[serde(default, skip_serializing_if = "crate::automation::AutoMode::is_read")]
    pub auto_mode: crate::automation::AutoMode,
}

fn yes() -> bool {
    true
}

impl Default for Master {
    fn default() -> Self {
        Master {
            gain_db: 0.0,
            pan: 0.0,
            width: 1.0,
            phase_invert: false,
            effects: Vec::new(),
            fx_enabled: true,
            modulators: Vec::new(),
            auto_mode: Default::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub tempo_bpm: f64,
    pub time_signature: TimeSignature,
    pub tracks: Vec<Track>,
    /// The mixer's channels, in the order the mixer shows them. See [`MixerStrip`].
    #[serde(default)]
    pub strips: Vec<MixerStrip>,
    /// The desk's groups, in the order they were made. See [`StripGroup`].
    #[serde(default)]
    pub strip_groups: Vec<StripGroup>,
    pub master: Master,
    pub loop_region: Option<LoopRegion>,
    pub loop_enabled: bool,
    pub metronome: bool,
    /// How panning is compensated. Part of the project because it changes what the
    /// project renders as (ADR 0012).
    #[serde(default)]
    pub pan_law: PanLaw,
    /// Every audio file the project's clips play (ADR 0025).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub audio_pool: Vec<crate::audio::AudioSource>,
    /// How recording behaves: count-in, punch, takes, the click (ADR 0025).
    #[serde(default)]
    pub recording: crate::audio::RecordSettings,
    /// FL's PAT / SONG switch (schema 26, ADR 0052): on, the transport plays the current
    /// pattern alone, looped (`crate::pattern`). Saved with the song as FL saves it; not
    /// written while it says *song*.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pattern_mode: bool,
    /// The tempo track (schema 33, ADR 0060): empty without one — the song plays at
    /// `tempo_bpm` throughout. With one, the first point is at tick 0 and agrees with
    /// `tempo_bpm`; each point holds its tempo or ramps to the next.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tempo_points: Vec<crate::tempo::TempoPoint>,
    /// The metre changes (schema 33, ADR 0060): empty with one metre throughout. With
    /// some, the first is at tick 0 and agrees with `time_signature`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metre_changes: Vec<crate::tempo::MetreChange>,
    /// The named markers (schema 33, ADR 0060), sorted by position, then id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<crate::tempo::Marker>,
    /// The project's MIDI controller mappings (schema 35, ADR 0062): empty without any.
    #[serde(
        default,
        skip_serializing_if = "crate::controller::ControllerMap::is_empty"
    )]
    pub controllers: crate::controller::ControllerMap,
    /// The song's key and scale (schema 43, ADR 0069): what the piano roll lights and snaps
    /// to, what the note effects read. `None` — every older project — says none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<crate::scale::Key>,
    /// The groove pool's own grooves (schema 43, ADR 0069), extracted from clips; the
    /// built-in swings are never stored.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grooves: Vec<crate::groove::Groove>,
    /// The session view's scenes and settings (schema 46, ADR 0076); each track holds its
    /// own column (`Track::slots`). The default — every older project — is no session, and
    /// is not written. // r33:session
    #[serde(default, skip_serializing_if = "crate::session::Session::is_default")]
    // r33:session
    pub session: crate::session::Session, // r33:session
    /// Monotonic id source for tracks, clips and notes. Never reused, never decremented.
    pub next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        let mut p = Project::empty("Untitled");
        for i in 0..DEFAULT_STRIPS {
            let mut strip = p.make_strip("");
            if i >= DEFAULT_STRIPS - DEFAULT_RIGHT_DOCK {
                strip.dock = Dock::Right;
            }
            p.strips.push(strip);
        }
        p
    }
}

impl Project {
    /// A project with **no** strips. `Default` gives the sixteen a session opens with;
    /// this is for migrations, which build the strips a file needs from what it holds.
    pub fn empty(name: impl Into<String>) -> Self {
        Project {
            name: name.into(),
            tempo_bpm: 120.0,
            time_signature: TimeSignature::default(),
            tracks: Vec::new(),
            strips: Vec::new(),
            strip_groups: Vec::new(),
            master: Master::default(),
            loop_region: Some(LoopRegion {
                start: Ticks::ZERO,
                end: Ticks::beats(16),
            }),
            loop_enabled: false,
            metronome: false,
            pan_law: PanLaw::default(),
            audio_pool: Vec::new(),
            recording: crate::audio::RecordSettings::default(),
            pattern_mode: false,
            tempo_points: Vec::new(),
            metre_changes: Vec::new(),
            markers: Vec::new(),
            controllers: crate::controller::ControllerMap::default(),
            key: None,
            grooves: Vec::new(),
            session: Default::default(), // r33:session
            next_id: 1,
        }
    }

    pub fn new(name: impl Into<String>) -> Self {
        Project {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Allocate a fresh id. Ids are shared across all entity types so a `u64` from any
    /// source can never collide with another.
    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.tracks.iter().find(|t| t.id == id)
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|t| t.id == id)
    }

    pub fn track_index(&self, id: TrackId) -> Option<usize> {
        self.tracks.iter().position(|t| t.id == id)
    }

    pub fn strip(&self, id: StripId) -> Option<&MixerStrip> {
        self.strips.iter().find(|s| s.id == id)
    }

    pub fn strip_mut(&mut self, id: StripId) -> Option<&mut MixerStrip> {
        self.strips.iter_mut().find(|s| s.id == id)
    }

    pub fn strip_index(&self, id: StripId) -> Option<usize> {
        self.strips.iter().position(|s| s.id == id)
    }

    /// The insert chain an effect host id names (ADR 0026): a strip's, or the master's for
    /// [`StripId::MASTER`]. The one resolver every effect command and view goes through,
    /// so the master needs no second path.
    pub fn effects_of(&self, host: StripId) -> Option<&Vec<EffectSlot>> {
        if host.is_master() {
            Some(&self.master.effects)
        } else {
            self.strip(host).map(|s| &s.effects)
        }
    }

    /// [`Self::effects_of`], to change.
    pub fn effects_of_mut(&mut self, host: StripId) -> Option<&mut Vec<EffectSlot>> {
        if host.is_master() {
            Some(&mut self.master.effects)
        } else {
            self.strip_mut(host).map(|s| &mut s.effects)
        }
    }

    /// One insert of a strip or of the master.
    pub fn effect(&self, host: StripId, id: EffectId) -> Option<&EffectSlot> {
        self.effects_of(host)?.iter().find(|e| e.id == id)
    }

    /// [`Self::effect`], to change.
    pub fn effect_mut(&mut self, host: StripId, id: EffectId) -> Option<&mut EffectSlot> {
        self.effects_of_mut(host)?.iter_mut().find(|e| e.id == id)
    }

    /// Whether a host's chain is switched on (a strip's FX lamp, or the master's).
    pub fn fx_enabled(&self, host: StripId) -> Option<bool> {
        if host.is_master() {
            Some(self.master.fx_enabled)
        } else {
            self.strip(host).map(|s| s.fx_enabled)
        }
    }

    /// Every insert of the project with its host: the strips' in desk order, then the
    /// master's (ADR 0026).
    pub fn all_effects(&self) -> impl Iterator<Item = (StripId, &EffectSlot)> {
        self.strips
            .iter()
            .flat_map(|s| s.effects.iter().map(move |e| (s.id, e)))
            .chain(self.master.effects.iter().map(|e| (StripId::MASTER, e)))
    }

    /// [`Self::all_effects`], to change (without their hosts).
    pub fn all_effects_mut(&mut self) -> impl Iterator<Item = &mut EffectSlot> {
        self.strips
            .iter_mut()
            .flat_map(|s| s.effects.iter_mut())
            .chain(self.master.effects.iter_mut())
    }

    /// Where an effect lives: the host (a strip, or [`StripId::MASTER`]) whose chain holds
    /// it.
    pub fn effect_host(&self, id: EffectId) -> Option<StripId> {
        self.strips
            .iter()
            .find(|s| s.effect(id).is_some())
            .map(|s| s.id)
            .or_else(|| {
                self.master
                    .effects
                    .iter()
                    .any(|e| e.id == id)
                    .then_some(StripId::MASTER)
            })
    }

    /// The strip's number as the mixer shows it, from 1.
    pub fn strip_number(&self, id: StripId) -> Option<usize> {
        self.strip_index(id).map(|i| i + 1)
    }

    /// The first strip no track feeds, which is where a new track goes. Strips in the
    /// middle of the desk are taken first: the pinned docks hold returns and buses, and a
    /// new instrument landing on one of them would be a surprise.
    pub fn first_free_strip(&self) -> Option<StripId> {
        let free = |s: &&MixerStrip| !self.tracks.iter().any(|t| t.strip == Some(s.id));
        self.strips
            .iter()
            .filter(|s| s.dock == Dock::Middle)
            .find(free)
            .or_else(|| self.strips.iter().find(free))
            .map(|s| s.id)
    }

    pub fn strip_group(&self, id: GroupId) -> Option<&StripGroup> {
        self.strip_groups.iter().find(|g| g.id == id)
    }

    pub fn strip_group_mut(&mut self, id: GroupId) -> Option<&mut StripGroup> {
        self.strip_groups.iter_mut().find(|g| g.id == id)
    }

    /// The strips of a group, in desk order.
    pub fn strips_in_group(&self, id: GroupId) -> impl Iterator<Item = &MixerStrip> {
        self.strips.iter().filter(move |s| s.group == Some(id))
    }

    pub fn make_strip_group(&mut self, name: impl Into<String>, color: Color) -> StripGroup {
        StripGroup {
            id: GroupId(self.alloc_id()),
            name: name.into(),
            color,
            collapsed: false,
        }
    }

    /// The tracks feeding a strip, in track order.
    pub fn tracks_on_strip(&self, id: StripId) -> impl Iterator<Item = &Track> {
        self.tracks.iter().filter(move |t| t.strip == Some(id))
    }

    /// Would routing `strip` into `output` — as its output or as a send — close a loop?
    /// Walks every route out of `output` (outputs and sends alike) and fails if it reaches
    /// `strip`, which also covers a strip feeding itself.
    pub fn routing_would_cycle(&self, strip: StripId, output: Option<StripId>) -> bool {
        let Some(start) = output else {
            return false;
        };
        let mut seen: Vec<StripId> = Vec::with_capacity(self.strips.len());
        let mut stack = vec![start];
        while let Some(id) = stack.pop() {
            if id == strip {
                return true;
            }
            if seen.contains(&id) {
                continue;
            }
            seen.push(id);
            stack.extend(self.downstream(id));
        }
        false
    }

    /// Every strip that has to be processed after `id`: the ones it feeds (output and
    /// sends) and the ones whose effects are keyed from it. The edges of the processing
    /// order.
    pub fn downstream(&self, id: StripId) -> Vec<StripId> {
        let mut out: Vec<StripId> = self
            .strip(id)
            .map(|s| s.feeds().collect())
            .unwrap_or_default();
        out.extend(
            self.strips
                .iter()
                .filter(|s| s.keys().any(|k| k == id))
                .map(|s| s.id),
        );
        out
    }

    /// The strips connected to `roots` by routing in either direction: everything that
    /// feeds one of them (directly or through other strips) and everything one of them
    /// feeds. This is what a solo keeps audible (FL's *solo with connected routes*), so
    /// soloing a channel is heard through its bus and soloing a bus is heard with its
    /// inputs. Returned as indices into `strips`.
    pub fn connected_strips(&self, roots: impl Iterator<Item = StripId>) -> Vec<usize> {
        let n = self.strips.len();
        let mut marked = vec![false; n];
        let mut stack: Vec<usize> = roots.filter_map(|r| self.strip_index(r)).collect();
        // Downstream: follow outputs and sends.
        let mut down = stack.clone();
        while let Some(i) = down.pop() {
            if marked[i] {
                continue;
            }
            marked[i] = true;
            for to in self.strips[i].feeds() {
                if let Some(j) = self.strip_index(to) {
                    down.push(j);
                }
            }
        }
        // Upstream: anything whose routes reach a marked strip.
        let mut up_marked = vec![false; n];
        while let Some(i) = stack.pop() {
            if up_marked[i] {
                continue;
            }
            up_marked[i] = true;
            let id = self.strips[i].id;
            for (j, s) in self.strips.iter().enumerate() {
                if s.feeds().any(|to| to == id) {
                    stack.push(j);
                }
            }
        }
        (0..n).filter(|i| marked[*i] || up_marked[*i]).collect()
    }

    pub fn any_strip_solo(&self) -> bool {
        self.strips.iter().any(|s| s.soloed)
    }

    /// Whether each strip sounds, given mutes and solos: a muted strip never does; with a
    /// solo somewhere, only the soloed strips and what they are connected to do. Indexed
    /// like `strips`.
    pub fn strip_audibility(&self) -> Vec<bool> {
        let mut audible: Vec<bool> = self.strips.iter().map(|s| !s.muted).collect();
        if self.any_strip_solo() {
            let keep = self.connected_strips(self.strips.iter().filter(|s| s.soloed).map(|s| s.id));
            for (i, a) in audible.iter_mut().enumerate() {
                *a = *a && keep.contains(&i);
            }
        }
        audible
    }

    /// Which strips send into `id`, by index.
    pub fn senders_to(&self, id: StripId) -> impl Iterator<Item = &MixerStrip> {
        self.strips
            .iter()
            .filter(move |s| s.sends.iter().any(|x| x.to == id))
    }

    /// The strips in processing order: every strip after all the strips that feed it,
    /// so the audio thread can run them front to back and each bus sees its inputs
    /// complete. Indices into `strips`. Strips caught in a cycle (which the commands
    /// refuse, but a hand-edited file can hold) come last and feed the master.
    pub fn strip_order(&self) -> Vec<usize> {
        let n = self.strips.len();
        let mut indegree = vec![0usize; n];
        let edges: Vec<Vec<usize>> = self
            .strips
            .iter()
            .map(|s| {
                self.downstream(s.id)
                    .into_iter()
                    .filter_map(|o| self.strip_index(o))
                    .collect()
            })
            .collect();
        for e in &edges {
            for out in e {
                indegree[*out] += 1;
            }
        }
        let mut order = Vec::with_capacity(n);
        let mut ready: Vec<usize> = (0..n).filter(|i| indegree[*i] == 0).collect();
        // Keep the mixer's own order among strips that are otherwise unordered, so the
        // processing order is stable from one snapshot to the next.
        ready.reverse();
        while let Some(i) = ready.pop() {
            order.push(i);
            let mut newly = false;
            for &out in &edges[i] {
                indegree[out] -= 1;
                if indegree[out] == 0 {
                    ready.push(out);
                    newly = true;
                }
            }
            if newly {
                ready.sort_unstable_by(|a, b| b.cmp(a));
            }
        }
        for i in 0..n {
            if !order.contains(&i) {
                order.push(i);
            }
        }
        order
    }

    /// Timeline end: the end of the last clip, or zero if the project is empty.
    pub fn content_end(&self) -> Ticks {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter().map(Clip::end))
            .max()
            .unwrap_or(Ticks::ZERO)
    }

    pub fn any_solo(&self) -> bool {
        self.tracks.iter().any(|t| t.soloed)
    }

    /// Whether the track is audible given mute/solo state.
    /// Drop every track delay a bounce could not have kept (a hand-edited or damaged file,
    /// `Lateness::is_valid`); how many went (review 3).
    pub fn repair_lateness(&mut self) -> usize {
        let mut n = 0;
        for t in &mut self.tracks {
            if t.lateness.is_some_and(|l| !l.is_valid()) {
                t.lateness = None;
                n += 1;
            }
        }
        n
    }

    pub fn track_audible(&self, track: &Track) -> bool {
        if track.muted {
            return false;
        }
        if self.any_solo() {
            track.soloed
        } else {
            true
        }
    }

    /// Build a default track with a fresh id, the next palette colour, and the first free
    /// mixer strip.
    pub fn make_track(&mut self, name: impl Into<String>) -> Track {
        let id = TrackId(self.alloc_id());
        let color = Color::PALETTE[self.tracks.len() % Color::PALETTE.len()];
        Track {
            id,
            name: name.into(),
            color,
            height: 1.0,
            icon: None,
            instrument: InstrumentSpec::default(),
            clips: Vec::new(),
            gain_db: 0.0,
            pan: 0.0,
            strip: self.first_free_strip(),
            muted: false,
            soloed: false,
            input: None,
            armed: false,
            takes_open: false,
            automation_open: true,
            auto_mode: Default::default(),
            pitch_open: false,
            modulators: Vec::new(),
            comps: Vec::new(),
            active_comp: 0,
            frozen: None,
            lateness: None,
            note_fx: Vec::new(),
            score: Default::default(),
            slots: Vec::new(), // r33:session
        }
    }

    pub fn make_strip(&mut self, name: impl Into<String>) -> MixerStrip {
        MixerStrip {
            id: StripId(self.alloc_id()),
            name: name.into(),
            gain_db: 0.0,
            pan: 0.0,
            width: 1.0,
            phase_invert: false,
            muted: false,
            soloed: false,
            swap_channels: false,
            fx_enabled: true,
            effects: Vec::new(),
            eq: StripEq::default(),
            output: None,
            sends: Vec::new(),
            color: None,
            dock: Dock::Middle,
            group: None,
            modulators: Vec::new(),
            frozen: None,
        }
    }

    pub fn make_clip(&mut self, name: impl Into<String>, start: Ticks, length: Ticks) -> Clip {
        Clip {
            id: ClipId(self.alloc_id()),
            name: name.into(),
            start,
            length,
            offset: Ticks::ZERO,
            content_length: None,
            muted: false,
            color: None,
            notes: Vec::new(),
            audio: None,
            lane: 0,
            automation: None,
            link: None,
            comp: false,
            lanes: Vec::new(),
            groove: None,
            skip_note_fx: false,
        }
    }

    /// An automation clip (ADR 0027) driving `target`, its window `start`..`start +
    /// length` showing the curve from content tick 0.
    pub fn make_automation_clip(
        &mut self,
        name: impl Into<String>,
        target: crate::automation::AutoTarget,
        start: Ticks,
        length: Ticks,
        points: Vec<crate::automation::AutoPoint>,
    ) -> Clip {
        let mut clip = self.make_clip(name, start, length);
        clip.automation = Some(crate::automation::Automation::new(target, points));
        clip
    }

    /// An audio clip over `source`, its window `start`..`start + length` showing the file
    /// from its first frame.
    pub fn make_audio_clip(
        &mut self,
        name: impl Into<String>,
        source: AudioSourceId,
        start: Ticks,
        length: Ticks,
    ) -> Clip {
        let mut clip = self.make_clip(name, start, length);
        clip.audio = Some(crate::audio::AudioRegion::of(source));
        clip
    }

    pub fn audio_source(&self, id: AudioSourceId) -> Option<&crate::audio::AudioSource> {
        self.audio_pool.iter().find(|s| s.id == id)
    }

    pub fn audio_source_mut(
        &mut self,
        id: AudioSourceId,
    ) -> Option<&mut crate::audio::AudioSource> {
        self.audio_pool.iter_mut().find(|s| s.id == id)
    }

    pub fn make_effect(&mut self, kind: crate::effect::EffectKind) -> EffectSlot {
        EffectSlot {
            id: EffectId(self.alloc_id()),
            bypass: false,
            mix: 1.0,
            sidechain: None,
            midi_from: None,
            spec: crate::effect::EffectSpec::new(kind),
        }
    }

    pub fn make_note(
        &mut self,
        start: Ticks,
        length: Ticks,
        pitch: Pitch,
        velocity: Velocity,
    ) -> Note {
        Note {
            id: NoteId(self.alloc_id()),
            start,
            length,
            pitch: pitch.min(127),
            velocity: velocity.clamp(1, 127),
            muted: false,
            release: 0,
            expr: None,
        }
    }
}

/// Convert a fader value in dB to a linear gain. `MIN_GAIN_DB` and below is silence.
pub fn db_to_linear(db: f32) -> f32 {
    if db <= MIN_GAIN_DB {
        0.0
    } else {
        math::powf(10f32, db / 20.0)
    }
}

pub fn linear_to_db(lin: f32) -> f32 {
    if lin <= 0.0 {
        MIN_GAIN_DB
    } else {
        (20.0 * math::log10(lin)).max(MIN_GAIN_DB)
    }
}

/// The raw equal-power taper. Prefer [`PanLaw::gains`], which is what the engine uses:
/// this is the shape, not the law, and a project decides which law applies to it.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    PanLaw::Minus3dB.gains(pan)
}

/// A **balance**: unity at the centre, the far channel tapered. For the master and for a
/// track's own placement before its strip, where a signal is being *balanced* rather
/// than *placed* and the pan law must not be applied a second time (ADR 0012). Centre
/// returns exactly `(1, 1)`, so a control at its default touches no sample.
pub fn balance_gains(pan: f32) -> (f32, f32) {
    let p = if pan.is_finite() {
        pan.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    if p == 0.0 {
        return (1.0, 1.0);
    }
    // Only the far channel tapers; the near one is exactly unity. (The taper alone rounds
    // to 0.99999994 on the near side for small pans — `DSP_VERSION` 4 made it exact.)
    let angle = (p + 1.0) * 0.25 * std::f32::consts::PI;
    if p < 0.0 {
        (1.0, (math::sin(angle) * std::f32::consts::SQRT_2).min(1.0))
    } else {
        ((math::cos(angle) * std::f32::consts::SQRT_2).min(1.0), 1.0)
    }
}

/// MIDI pitch → frequency in Hz with A4 = 440 Hz.
pub fn pitch_to_hz(pitch: f32) -> f32 {
    440.0 * math::powf(2f32, (pitch - 69.0) / 12.0)
}

pub fn pitch_name(pitch: Pitch) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (pitch / 12) as i32 - 1;
    format!("{}{}", NAMES[(pitch % 12) as usize], octave)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gains() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_linear(-6.0206) - 0.5).abs() < 1e-4);
        assert_eq!(db_to_linear(-200.0), 0.0);
        assert!((linear_to_db(0.5) + 6.0206).abs() < 1e-3);
        let (l, r) = pan_gains(0.0);
        assert!((l - r).abs() < 1e-6);
        assert!((l * l + r * r - 1.0).abs() < 1e-6);
        assert!((pan_gains(-1.0).1).abs() < 1e-6);
    }

    /// Each law must be attenuated at the centre by exactly the number in its name, and
    /// all of them must agree at the extremes. This is the whole contract.
    #[test]
    fn every_pan_law_says_what_it_does() {
        for (law, centre_db) in [
            (PanLaw::ZeroDb, 0.0),
            (PanLaw::Minus3dB, -3.0103),
            (PanLaw::Minus4_5dB, -4.5155),
            (PanLaw::Minus6dB, -6.0206),
        ] {
            let (l, r) = law.gains(0.0);
            assert!((l - r).abs() < 1e-6, "{law:?} is not centred at the centre");
            assert!(
                (linear_to_db(l) - centre_db).abs() < 0.02,
                "{law:?} attenuates the centre by {} dB, not {centre_db}",
                linear_to_db(l)
            );
            assert!((law.gains(-1.0).0 - 1.0).abs() < 1e-6, "{law:?} hard left");
            assert!(
                law.gains(-1.0).1.abs() < 1e-6,
                "{law:?} hard left leaks right"
            );
            assert!((law.gains(1.0).1 - 1.0).abs() < 1e-6, "{law:?} hard right");
            // Symmetry, and no channel ever exceeds unity.
            for i in 0..=100 {
                let p = i as f32 / 50.0 - 1.0;
                let (a, b) = law.gains(p);
                let (c, d) = law.gains(-p);
                assert!(
                    (a - d).abs() < 1e-6 && (b - c).abs() < 1e-6,
                    "{law:?} at {p}"
                );
                assert!(a <= 1.000_01 && b <= 1.000_01, "{law:?} boosts at {p}");
            }
        }
        // Constant power and constant amplitude are what those two names promise.
        for i in 0..=100 {
            let p = i as f32 / 50.0 - 1.0;
            let (l, r) = PanLaw::Minus3dB.gains(p);
            assert!((l * l + r * r - 1.0).abs() < 1e-5, "power at {p}");
            let (l, r) = PanLaw::Minus6dB.gains(p);
            assert!((l + r - 1.0).abs() < 1e-5, "amplitude at {p}");
        }
    }

    /// The 0 dB law's promise, to the bit: a centred strip is unity, and panning only ever
    /// lowers the far channel — the near one stays exactly 1. (`cos(π/4)·√2` is
    /// 0.99999994 in `f32`; until `DSP_VERSION` 4 every centred strip under this law was
    /// one rounding step quiet, and a pre send was not post to the bit — round 25.)
    #[test]
    fn the_zero_db_law_is_unity_at_centre_and_on_the_near_side() {
        assert_eq!(PanLaw::ZeroDb.gains(0.0), (1.0, 1.0));
        assert_eq!(PanLaw::ZeroDb.gains(-0.0), (1.0, 1.0));
        assert_eq!(balance_gains(0.0), (1.0, 1.0));
        for i in 0..=4000 {
            let p = i as f32 / 4000.0;
            for g in [PanLaw::ZeroDb.gains(p), balance_gains(p)] {
                assert_eq!(g.1, 1.0, "near (right) channel at pan {p}");
                assert!(g.0 <= 1.0, "far channel boosts at pan {p}");
            }
            for g in [PanLaw::ZeroDb.gains(-p), balance_gains(-p)] {
                assert_eq!(g.0, 1.0, "near (left) channel at pan {}", -p);
                assert!(g.1 <= 1.0, "far channel boosts at pan {}", -p);
            }
        }
        // Tiny pans, where the taper's rounding lived.
        for p in [f32::MIN_POSITIVE, 1e-7, 1e-6, 1e-5, 1e-4] {
            assert_eq!(PanLaw::ZeroDb.gains(p).1, 1.0, "pan {p}");
            assert_eq!(PanLaw::ZeroDb.gains(-p).0, 1.0, "pan {}", -p);
        }
    }

    /// The compatibility guarantee behind the migration: `ZeroDb` is the expression the
    /// engine used before the law existed, so a project written by an earlier Cantus
    /// renders as it did — bit for bit until `DSP_VERSION` 4, which made the near channel
    /// exactly unity where the old taper rounded to 0.99999994 (one step, ≤ 6e-8; the far
    /// channel is unchanged to the bit).
    #[test]
    fn the_zero_db_law_is_what_cantus_did_before_but_exact_on_the_near_side() {
        fn legacy(pan: f32) -> (f32, f32) {
            // Verbatim from `Processor::render`, before ADR 0012: equal-power taper,
            // scaled so the centre is unity, with the near channel clamped to the fader.
            let p = pan.clamp(-1.0, 1.0);
            let angle = (p + 1.0) * 0.25 * std::f32::consts::PI;
            let (pl, pr) = (math::cos(angle), math::sin(angle));
            let g = 1.0f32;
            let (gl, gr) = (
                g * pl * std::f32::consts::SQRT_2,
                g * pr * std::f32::consts::SQRT_2,
            );
            (gl.min(g), gr.min(g))
        }
        for i in 0..=2000 {
            let p = i as f32 / 1000.0 - 1.0;
            let now = PanLaw::ZeroDb.gains(p);
            let then = legacy(p);
            // The far channel is untouched; the near one is 1 where it was 1 or one
            // rounding step under it.
            let (near_now, near_then, far_now, far_then) = if p < 0.0 {
                (now.0, then.0, now.1, then.1)
            } else if p > 0.0 {
                (now.1, then.1, now.0, then.0)
            } else {
                assert_eq!(now, (1.0, 1.0), "centre");
                continue;
            };
            assert_eq!(far_now, far_then, "pan {p}: the far channel changed");
            assert_eq!(near_now, 1.0, "pan {p}: the near channel is not unity");
            assert!(
                1.0 - near_then <= 6e-8,
                "pan {p}: the old near channel was {near_then}"
            );
        }
    }

    #[test]
    fn width_one_is_the_identity_and_width_zero_is_mono() {
        let src_l: Vec<f32> = (0..64).map(|i| math::sin(i as f32 * 0.1)).collect();
        let src_r: Vec<f32> = (0..64).map(|i| math::cos(i as f32 * 0.17)).collect();

        let (mut l, mut r) = (src_l.clone(), src_r.clone());
        apply_width(&mut l, &mut r, 1.0);
        assert_eq!(
            (l, r),
            (src_l.clone(), src_r.clone()),
            "width 1 changed samples"
        );

        let (mut l, mut r) = (src_l.clone(), src_r.clone());
        apply_width(&mut l, &mut r, 0.0);
        assert_eq!(l, r, "width 0 is not mono");
        for i in 0..64 {
            assert!((l[i] - (src_l[i] + src_r[i]) * 0.5).abs() < 1e-6);
        }

        // Widening and re-narrowing is an exact round trip, so the control loses nothing.
        let (mut l, mut r) = (src_l.clone(), src_r.clone());
        apply_width(&mut l, &mut r, 2.0);
        apply_width(&mut l, &mut r, 0.5);
        for i in 0..64 {
            assert!((l[i] - src_l[i]).abs() < 1e-6 && (r[i] - src_r[i]).abs() < 1e-6);
        }
        // A clamp, not a panic, for a hand-edited project file.
        for bad in [f32::INFINITY, f32::NAN, -1.0] {
            let (mut l, mut r) = (src_l.clone(), src_r.clone());
            apply_width(&mut l, &mut r, bad);
            assert!(
                l.iter().chain(r.iter()).all(|v| v.is_finite()),
                "width {bad} produced samples that are not numbers"
            );
        }
    }

    #[test]
    fn pitches() {
        assert!((pitch_to_hz(69.0) - 440.0).abs() < 1e-4);
        assert!((pitch_to_hz(57.0) - 220.0).abs() < 1e-4);
        assert_eq!(pitch_name(60), "C4");
        assert_eq!(pitch_name(61), "C#4");
        assert_eq!(pitch_name(0), "C-1");
    }

    #[test]
    fn ids_never_collide() {
        let mut p = Project::default();
        let t = p.make_track("a");
        let c = p.make_clip("c", Ticks::ZERO, Ticks::beats(4));
        let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 100);
        assert_ne!(t.id.0, c.id.0);
        assert_ne!(c.id.0, n.id.0);
        // Sixteen strips took the first ids.
        assert_eq!(p.next_id, DEFAULT_STRIPS as u64 + 4);
    }
}

#[cfg(test)]
mod strip_tests {
    use super::*;

    /// The last three strips of a new desk are pinned on the right, and a new track never
    /// lands on one while a middle strip is free.
    #[test]
    fn the_right_dock_holds_three_strips_that_new_tracks_leave_alone() {
        let mut p = Project::default();
        let docked: Vec<usize> = p
            .strips
            .iter()
            .enumerate()
            .filter(|(_, s)| s.dock == Dock::Right)
            .map(|(i, _)| i)
            .collect();
        assert_eq!(docked, vec![13, 14, 15]);
        for n in 0..13 {
            let t = p.make_track(format!("t{n}"));
            assert_eq!(t.strip, Some(p.strips[n].id));
            p.tracks.push(t);
        }
        // The middle is full: only now does a track take a docked strip.
        let t = p.make_track("fourteenth");
        assert_eq!(t.strip, Some(p.strips[13].id));
    }

    #[test]
    fn a_new_project_has_numbered_strips_and_new_tracks_fill_them_in_order() {
        let mut p = Project::default();
        assert_eq!(p.strips.len(), DEFAULT_STRIPS);
        let a = p.make_track("a");
        p.tracks.push(a);
        let b = p.make_track("b");
        p.tracks.push(b);
        assert_eq!(p.tracks[0].strip, Some(p.strips[0].id));
        assert_eq!(p.tracks[1].strip, Some(p.strips[1].id));
        assert_eq!(p.strip_number(p.strips[1].id), Some(2));
    }

    #[test]
    fn strips_are_processed_after_everything_that_feeds_them() {
        let mut p = Project::default();
        // 1 → 3, 2 → 3, 3 → master, 4 → 1.
        let (s1, s2, s3, s4) = (
            p.strips[0].id,
            p.strips[1].id,
            p.strips[2].id,
            p.strips[3].id,
        );
        p.strips[0].output = Some(s3);
        p.strips[1].output = Some(s3);
        p.strips[3].output = Some(s1);
        let order = p.strip_order();
        let pos = |id: StripId| order.iter().position(|i| p.strips[*i].id == id).unwrap();
        assert!(pos(s4) < pos(s1), "4 feeds 1");
        assert!(pos(s1) < pos(s3) && pos(s2) < pos(s3), "1 and 2 feed 3");
        assert_eq!(order.len(), DEFAULT_STRIPS, "every strip is processed once");
        assert!(
            p.routing_would_cycle(s3, Some(s1)),
            "3 → 1 would loop through 1 → 3"
        );
        assert!(
            p.routing_would_cycle(s2, Some(s2)),
            "a strip cannot feed itself"
        );
        assert!(
            !p.routing_would_cycle(s2, Some(s4)),
            "2 → 4 → 1 → 3 is a chain, not a loop"
        );
        assert!(!p.routing_would_cycle(s2, None));
    }

    /// A send is a route: it orders the strips and it can close a loop, exactly as an
    /// output can, so the audio thread never meets a bus before its inputs.
    #[test]
    fn sends_order_strips_and_cannot_loop() {
        let mut p = Project::default();
        let (s1, s2, s3) = (p.strips[0].id, p.strips[1].id, p.strips[2].id);
        // 1 → master, with a send into 3; 3 → 2.
        p.strips[0].sends.push(Send::new(s3));
        p.strips[2].output = Some(s2);
        let order = p.strip_order();
        let pos = |id: StripId| order.iter().position(|i| p.strips[*i].id == id).unwrap();
        assert!(pos(s1) < pos(s3), "the send from 1 feeds 3");
        assert!(pos(s3) < pos(s2));
        assert!(p.routing_would_cycle(s2, Some(s1)), "2 → 1 ⇝ 3 → 2 loops");
        assert!(
            p.routing_would_cycle(s3, Some(s1)),
            "a send back into its source loops"
        );
        assert!(p.routing_would_cycle(s2, Some(s3)), "2 → 3 → 2 loops");
        assert!(!p.routing_would_cycle(s1, Some(s2)), "1 → 2 is fine");
    }

    /// Solo keeps the soloed strip, its inputs and its destinations; mute always wins.
    #[test]
    fn solo_keeps_the_connected_routes_and_nothing_else() {
        let mut p = Project::default();
        let (s1, s2, s3, s4, s5) = (
            p.strips[0].id,
            p.strips[1].id,
            p.strips[2].id,
            p.strips[3].id,
            p.strips[4].id,
        );
        // 1 → 3 (bus), 2 → 3, 4 alone; 1 also sends into 5.
        p.strips[0].output = Some(s3);
        p.strips[1].output = Some(s3);
        p.strips[0].sends.push(Send::new(s5));
        assert!(
            p.strip_audibility().iter().all(|a| *a),
            "no solo: everything sounds"
        );
        p.strips[0].soloed = true;
        let a = p.strip_audibility();
        let idx = |id: StripId| [s1, s2, s3, s4, s5].iter().position(|x| *x == id).unwrap();
        assert!(a[idx(s1)], "the soloed strip");
        assert!(a[idx(s3)], "heard through its bus");
        assert!(a[idx(s5)], "and through its send");
        assert!(
            !a[idx(s2)],
            "a sibling into the same bus is not connected to 1"
        );
        assert!(!a[idx(s4)], "an unrelated strip is silent");
        // Soloing the bus brings both inputs back.
        p.strips[0].soloed = false;
        p.strips[2].soloed = true;
        let a = p.strip_audibility();
        assert!(a[idx(s1)] && a[idx(s2)] && a[idx(s3)]);
        assert!(!a[idx(s4)]);
        // Mute wins over solo.
        p.strips[1].muted = true;
        assert!(!p.strip_audibility()[idx(s2)]);
    }

    /// A sidechain is a route: the key's strip is processed first, and a key that would
    /// close a loop is a cycle like any other.
    #[test]
    fn a_sidechain_orders_the_strips_and_cannot_loop() {
        let mut p = Project::default();
        let (s1, s2, s3) = (p.strips[0].id, p.strips[1].id, p.strips[2].id);
        // 2 → 3; the compressor on 1 is keyed from 3.
        p.strips[1].output = Some(s3);
        let mut comp = p.make_effect(crate::effect::EffectKind::Compressor);
        comp.sidechain = Some(s3);
        p.strips[0].effects.push(comp);
        let order = p.strip_order();
        let pos = |id: StripId| order.iter().position(|i| p.strips[*i].id == id).unwrap();
        assert!(
            pos(s3) < pos(s1),
            "the key is processed before the compressor"
        );
        assert!(pos(s2) < pos(s3));
        assert!(
            p.routing_would_cycle(s1, Some(s3)),
            "1 → 3 would loop: 3 keys 1, so 3 has to come first"
        );
        assert!(
            p.routing_would_cycle(s1, Some(s2)),
            "1 → 2 → 3 ⇢ 1 loops through the key as well"
        );
        assert!(
            !p.routing_would_cycle(s3, Some(s1)),
            "3 → 1 runs the same way as the key: no loop"
        );
    }

    #[test]
    fn the_eq_is_flat_by_default_and_clamps_what_it_is_given() {
        let mut eq = StripEq::default();
        assert!(eq.is_flat());
        assert_eq!(eq.set(1, EqField::Gain, 40.0), Some(EQ_MAX_GAIN_DB));
        assert!(!eq.is_flat());
        assert_eq!(eq.set(0, EqField::Freq, 1.0), Some(EQ_MIN_HZ));
        assert_eq!(eq.set(2, EqField::Q, f32::NAN), None);
        assert_eq!(eq.set(3, EqField::Q, 1.0), None, "there are three bands");
        assert_eq!(eq.get(2, EqField::Q), 0.71);
    }

    #[test]
    fn balance_is_unity_at_the_centre() {
        assert_eq!(balance_gains(0.0), (1.0, 1.0));
        let (l, r) = balance_gains(-1.0);
        assert!((l - 1.0).abs() < 1e-6 && r.abs() < 1e-6);
    }
}
