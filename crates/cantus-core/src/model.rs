//! The project model: the single source of truth that the UI edits (through commands),
//! the project file stores, and the engine snapshots.
//!
//! This module is deliberately plain data. All behaviour lives in [`crate::command`].

use crate::effect::EffectSlot;
use crate::instrument::InstrumentSpec;
use crate::time::{Ticks, TimeSignature};
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
        let (power_l, power_r) = (angle.cos(), angle.sin());
        let (amp_l, amp_r) = ((1.0 - p) * 0.5, (1.0 + p) * 0.5);
        match self {
            // Equal-power taper on the far channel, and the near channel simply left
            // alone — a pan pot that can never make a track louder than its fader says.
            PanLaw::ZeroDb => (
                (power_l * std::f32::consts::SQRT_2).min(1.0),
                (power_r * std::f32::consts::SQRT_2).min(1.0),
            ),
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
    /// Insert effects, processed in order after the instrument.
    #[serde(default)]
    pub effects: Vec<EffectSlot>,
    pub clips: Vec<Clip>,
    /// Fader gain in decibels. 0 dB = unity, `-inf` is represented by [`MIN_GAIN_DB`].
    pub gain_db: f32,
    /// -1.0 (hard left) ..= 1.0 (hard right).
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
    pub muted: bool,
    pub soloed: bool,
}

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

    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id)
    }

    pub fn clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|c| c.id == id)
    }

    pub fn effect(&self, id: EffectId) -> Option<&EffectSlot> {
        self.effects.iter().find(|e| e.id == id)
    }

    pub fn effect_mut(&mut self, id: EffectId) -> Option<&mut EffectSlot> {
        self.effects.iter_mut().find(|e| e.id == id)
    }

    pub fn effect_index(&self, id: EffectId) -> Option<usize> {
        self.effects.iter().position(|e| e.id == id)
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
}

impl Default for Master {
    fn default() -> Self {
        Master {
            gain_db: 0.0,
            pan: 0.0,
            width: 1.0,
            phase_invert: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub tempo_bpm: f64,
    pub time_signature: TimeSignature,
    pub tracks: Vec<Track>,
    pub master: Master,
    pub loop_region: Option<LoopRegion>,
    pub loop_enabled: bool,
    pub metronome: bool,
    /// How panning is compensated. Part of the project because it changes what the
    /// project renders as (ADR 0012).
    #[serde(default)]
    pub pan_law: PanLaw,
    /// Monotonic id source for tracks, clips and notes. Never reused, never decremented.
    pub next_id: u64,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            name: "Untitled".to_string(),
            tempo_bpm: 120.0,
            time_signature: TimeSignature::default(),
            tracks: Vec::new(),
            master: Master::default(),
            loop_region: Some(LoopRegion {
                start: Ticks::ZERO,
                end: Ticks::beats(16),
            }),
            loop_enabled: false,
            metronome: false,
            pan_law: PanLaw::default(),
            next_id: 1,
        }
    }
}

impl Project {
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

    /// Build a default track with a fresh id and the next palette colour.
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
            effects: Vec::new(),
            clips: Vec::new(),
            gain_db: 0.0,
            pan: 0.0,
            width: 1.0,
            phase_invert: false,
            muted: false,
            soloed: false,
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
        }
    }

    pub fn make_effect(&mut self, kind: crate::effect::EffectKind) -> EffectSlot {
        EffectSlot {
            id: EffectId(self.alloc_id()),
            bypass: false,
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
        }
    }
}

/// Convert a fader value in dB to a linear gain. `MIN_GAIN_DB` and below is silence.
pub fn db_to_linear(db: f32) -> f32 {
    if db <= MIN_GAIN_DB {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

pub fn linear_to_db(lin: f32) -> f32 {
    if lin <= 0.0 {
        MIN_GAIN_DB
    } else {
        (20.0 * lin.log10()).max(MIN_GAIN_DB)
    }
}

/// The raw equal-power taper. Prefer [`PanLaw::gains`], which is what the engine uses:
/// this is the shape, not the law, and a project decides which law applies to it.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    PanLaw::Minus3dB.gains(pan)
}

/// MIDI pitch → frequency in Hz with A4 = 440 Hz.
pub fn pitch_to_hz(pitch: f32) -> f32 {
    440.0 * 2f32.powf((pitch - 69.0) / 12.0)
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

    /// The compatibility guarantee behind the migration: `ZeroDb` is *exactly* the
    /// expression the engine used before the law existed, so a project written by an
    /// earlier Cantus renders sample for sample as it did.
    #[test]
    fn the_zero_db_law_is_bit_identical_to_what_cantus_did_before() {
        fn legacy(pan: f32) -> (f32, f32) {
            // Verbatim from `Processor::render`, before ADR 0012: equal-power taper,
            // scaled so the centre is unity, with the near channel clamped to the fader.
            let p = pan.clamp(-1.0, 1.0);
            let angle = (p + 1.0) * 0.25 * std::f32::consts::PI;
            let (pl, pr) = (angle.cos(), angle.sin());
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
            assert_eq!(now, then, "pan {p} changed: {then:?} became {now:?}");
        }
    }

    #[test]
    fn width_one_is_the_identity_and_width_zero_is_mono() {
        let src_l: Vec<f32> = (0..64).map(|i| (i as f32 * 0.1).sin()).collect();
        let src_r: Vec<f32> = (0..64).map(|i| (i as f32 * 0.17).cos()).collect();

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
        assert_eq!(p.next_id, 4);
    }
}
