//! Audio in the model (ADR 0025): the project's pool of audio files, audio clips, a
//! track's input, and how recording behaves.
//!
//! Plain data, like the rest of the model. Nothing here reads a file: `cantus-core` does
//! no I/O. The engine turns an [`AudioSource`]'s path into samples (`SampleCache`) and the
//! project crate moves the files when a project is saved.
//!
//! **An audio clip is a [`Clip`](crate::Clip) with [`Clip::audio`](crate::Clip::audio)
//! set.** Everything a clip already knows — its window (`start`, `length`, `offset`),
//! splitting, trimming, slipping, muting, colour, moving between tracks — works for audio
//! unchanged. The content timeline of an audio clip is the source file itself: content
//! tick `t` is the source at `seconds(t)` at the project's tempo, so content tick zero is
//! the file's first frame — unless the clip is **warped** (`AudioRegion::warp`, ADR 0061):
//! then its warp markers say which source frame each content tick hears
//! (`crate::warp::source_frame`).
//!
//! **Any track can hold audio clips** next to note clips (a hybrid track, as Bitwig does
//! it): the instrument's output and the clips' audio are summed before the channel's own
//! level. An *audio track* is simply a track whose instrument is never played; the UI
//! offers one with [`InstrumentSpec`](crate::InstrumentSpec)'s default and no note clips.

use crate::model::AudioSourceId;
use crate::time::Ticks;
use cantus_dsp::math;
use serde::{Deserialize, Serialize};

/// One audio file the project uses. Clips point at it by id, so many clips (the takes of
/// a loop recording, the pieces of a comp, copies) share one file and one cache entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioSource {
    pub id: AudioSourceId,
    /// What the user sees ("Vocal take 3"). Not necessarily the file's name.
    pub name: String,
    /// Where the file is. **Relative to the project file's folder** once the project has
    /// been saved with it (`Audio/Vocal take 3.wav`); absolute while it still lives
    /// elsewhere (an imported file not yet collected, a recording in the session's
    /// scratch folder before the first save).
    pub path: String,
    /// The file's own format, as it was when it was added. The engine resamples to the
    /// device's rate on load; these numbers are for display and for a missing file.
    pub sample_rate: u32,
    pub channels: u8,
    pub frames: u64,
    /// SHA-256 of the file's bytes, hex, when known: lets a moved file be found again
    /// and tells a replaced file from the original.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Set for a file this project recorded (as opposed to imported): saving moves it
    /// into the project's audio folder rather than copying it.
    #[serde(default)]
    pub recorded: bool,
}

/// The shape of a fade. `Linear` is the default because it is the one that sums to unity
/// across a crossfade of equal length with its mirror; `EqualPower` is the one that keeps
/// loudness across a crossfade of uncorrelated material.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FadeCurve {
    #[default]
    Linear,
    EqualPower,
    /// Slow start, fast end (an exponential-ish curve): the natural fade *out*.
    Exponential,
    /// Fast start, slow end: the natural fade *in*.
    Logarithmic,
    /// An S-curve.
    SCurve,
}

impl FadeCurve {
    pub const ALL: [FadeCurve; 5] = [
        FadeCurve::Linear,
        FadeCurve::EqualPower,
        FadeCurve::Exponential,
        FadeCurve::Logarithmic,
        FadeCurve::SCurve,
    ];

    /// The gain at `x` in 0..=1 of a fade *in* (a fade out is `gain(1 - x)`). Exactly 0 at
    /// 0 and exactly 1 at 1 for every curve, so a fade at its ends touches no sample.
    pub fn gain(self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        if x >= 1.0 {
            return 1.0;
        }
        match self {
            FadeCurve::Linear => x,
            FadeCurve::EqualPower => math::sin(x * std::f32::consts::FRAC_PI_2),
            FadeCurve::Exponential => x * x,
            FadeCurve::Logarithmic => 1.0 - (1.0 - x) * (1.0 - x),
            FadeCurve::SCurve => x * x * (3.0 - 2.0 * x),
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            FadeCurve::Linear => "linear",
            FadeCurve::EqualPower => "equal_power",
            FadeCurve::Exponential => "exponential",
            FadeCurve::Logarithmic => "logarithmic",
            FadeCurve::SCurve => "s_curve",
        }
    }
}

/// What makes a clip an audio clip. The clip's window says *which part* of the source is
/// heard and where; this says *how*.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioRegion {
    pub source: AudioSourceId,
    /// Clip gain, in dB, before the track's own level. 0 touches no sample.
    #[serde(default)]
    pub gain_db: f32,
    /// Fade lengths, in timeline ticks from the clip's left and right edges. Zero is no
    /// fade. The two never overlap: the engine shortens both in proportion if they would.
    #[serde(default)]
    pub fade_in: Ticks,
    #[serde(default)]
    pub fade_out: Ticks,
    #[serde(default)]
    pub fade_in_curve: FadeCurve,
    #[serde(default)]
    pub fade_out_curve: FadeCurve,
    /// Play the source backwards (the window still says which part).
    #[serde(default)]
    pub reverse: bool,
    /// A fine offset under the tick, in **source frames**, added to the content position:
    /// the source frame under content tick `t` is `seconds(t) × rate + slip_frames`. A
    /// clip's window is in whole ticks (≈ 25 frames at 120 BPM and 48 kHz); this is what
    /// puts a recorded take where it was played to the sample (ADR 0025 §7). Split, trim
    /// and glue keep it: it moves the content, not the window.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub slip_frames: i32,
    /// The pitch editor's notes and edits (schema 27, ADR 0054): the clip plays a render of
    /// them, the source untouched. `None` — every clip before, and every clip never edited
    /// — plays the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pitch: Option<Box<crate::pitch::PitchEdit>>,
    /// The crossfade into this window from the playing-lane audio window that ends where
    /// it starts — a comp's **seam** (schema 29, ADR 0058): equal-power, this long, centred
    /// on the seam, the two windows each read past the seam into their sources by half of
    /// it. Zero (every clip before) is a butt join; a window with nothing ending at its
    /// start has no seam, and the length waits there unused.
    #[serde(default, skip_serializing_if = "Ticks::is_zero")]
    pub xfade: Ticks,
    /// The clip's warp (schema 34, ADR 0061): its markers, mode and transposition — the
    /// clip follows the project's tempo through them. `None` — every clip before, and
    /// every clip never warped — plays the source at its natural speed (ADR 0025 §1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warp: Option<Box<crate::warp::Warp>>,
}

/// The longest seam crossfade a window takes (a beat at any tempo is far more than a comp
/// wants; the command refuses more).
pub const MAX_XFADE: Ticks = Ticks(crate::time::PPQ);

fn is_zero(v: &i32) -> bool {
    *v == 0
}

impl AudioRegion {
    pub fn of(source: AudioSourceId) -> Self {
        AudioRegion {
            source,
            gain_db: 0.0,
            fade_in: Ticks::ZERO,
            fade_out: Ticks::ZERO,
            fade_in_curve: FadeCurve::Linear,
            fade_out_curve: FadeCurve::Linear,
            reverse: false,
            slip_frames: 0,
            pitch: None,
            xfade: Ticks::ZERO,
            warp: None,
        }
    }
}

/// When a track hears its input. Every DAW has these three; `Auto` is the default because
/// it is the one that does the right thing on its own (Cubase's and Pro Tools' "auto
/// input"): hear the input while armed, except while playing back over existing material
/// without recording — then hear the takes.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonitorMode {
    Off,
    #[default]
    Auto,
    /// Always hear the input, armed or not, playing or not.
    On,
}

/// Which of the audio device's inputs a track records and monitors. Channels count from
/// zero on the input device; a stereo input is `first` and `first + 1`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackInput {
    pub first: u16,
    pub stereo: bool,
    #[serde(default)]
    pub monitor: MonitorMode,
    /// Input trim in dB, before anything else (and before the recorder: what is recorded
    /// is what is heard). 0 touches no sample.
    #[serde(default)]
    pub trim_db: f32,
}

impl TrackInput {
    pub fn mono(channel: u16) -> Self {
        TrackInput {
            first: channel,
            stereo: false,
            monitor: MonitorMode::Auto,
            trim_db: 0.0,
        }
    }

    pub fn stereo(first: u16) -> Self {
        TrackInput {
            first,
            stereo: true,
            monitor: MonitorMode::Auto,
            trim_db: 0.0,
        }
    }
}

/// What a new recording does to clips already where it lands.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TakeMode {
    /// Keep everything: the old clips move to take lanes, the new take plays (Logic's and
    /// Cubase's default; nothing is ever lost).
    #[default]
    Stack,
    /// The new take replaces what it covers on the playing lane (tape).
    Replace,
    /// MIDI only: merge the new notes into the clip that is there. Audio treats it as
    /// `Stack`.
    Overdub,
}

/// When the click sounds.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClickMode {
    /// Whenever the metronome switch is on (what `Project::metronome` always meant).
    #[default]
    Always,
    /// Only while recording and counting in.
    Recording,
    /// Only during the count-in.
    CountInOnly,
}

/// How recording behaves, stored with the song.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordSettings {
    /// Bars of count-in before recording starts (0 = none). The transport starts that many
    /// bars early with the click on, and the take begins where the playhead was.
    #[serde(default)]
    pub count_in_bars: u8,
    /// Punch in / out: recording only keeps what falls inside this range. The whole pass is
    /// still written to disk (handles to extend the take into later); the clip's window is
    /// the range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub punch: Option<crate::model::LoopRegion>,
    #[serde(default)]
    pub punch_enabled: bool,
    #[serde(default)]
    pub take_mode: TakeMode,
    #[serde(default)]
    pub click: ClickMode,
    /// The click's level in dB, relative to its built-in level (0 = as every earlier
    /// version played it, so an old project renders unchanged).
    #[serde(default = "click_level")]
    pub click_db: f32,
    /// Loop recording: each pass of the loop becomes a take (one file, a clip per pass).
    #[serde(default = "yes")]
    pub loop_takes: bool,
    /// The click's voice (schema 26, ADR 0052). Not written while it is the classic one,
    /// so a project that never chose another saves as it did.
    #[serde(default, skip_serializing_if = "ClickSound::is_classic")]
    pub click_sound: ClickSound,
    /// Whether the first beat of a bar is accented (schema 26): on, as every earlier
    /// version played it; off, every beat sounds as an ordinary one.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub click_accent: bool,
    /// While a pass records, the parameters moved — by a knob in Cantus or in a hosted
    /// plug-in's own window — are written into automation clips (schema 28, ADR 0057).
    /// On by default, as FL records automation; not written while on.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub record_automation: bool,
    /// The crossfade a new comp seam gets, in milliseconds (schema 29, ADR 0058): 10 ms,
    /// equal-power — Logic's and Studio One's kind of default.
    #[serde(default = "comp_xfade", skip_serializing_if = "is_comp_xfade")]
    pub comp_xfade_ms: f32,
    /// Quick swipe comping (Logic's): a drag across a take lane comps. Off, the drag
    /// selects time and a click selects the take (schema 29).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub quick_swipe: bool,
    /// Tracks whose strips share a mixer group comp together: a swipe on one swipes the
    /// same range of the same lane on each (schema 29, ADR 0058).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub comp_grouped: bool,
}

fn comp_xfade() -> f32 {
    10.0
}

fn is_comp_xfade(x: &f32) -> bool {
    *x == 10.0
}

/// The metronome's voice (schema 26, ADR 0052). `Classic` is the sine burst every earlier
/// version played, so an older project renders unchanged.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClickSound {
    /// A short sine burst: 1 760 Hz on the downbeat, 1 174.7 Hz on the other beats.
    #[default]
    Classic,
    /// A woodblock: two damped modes struck hard — drier and shorter.
    Wood,
    /// A digital beep: a soft-edged square tone with a flat top and a short release.
    Beep,
}

impl ClickSound {
    pub const ALL: [ClickSound; 3] = [ClickSound::Classic, ClickSound::Wood, ClickSound::Beep];

    pub fn is_classic(&self) -> bool {
        *self == ClickSound::Classic
    }
}

fn click_level() -> f32 {
    0.0
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

impl Default for RecordSettings {
    fn default() -> Self {
        RecordSettings {
            count_in_bars: 0,
            punch: None,
            punch_enabled: false,
            take_mode: TakeMode::Stack,
            click: ClickMode::Always,
            click_db: click_level(),
            loop_takes: true,
            click_sound: ClickSound::Classic,
            click_accent: true,
            record_automation: true,
            comp_xfade_ms: comp_xfade(),
            quick_swipe: true,
            comp_grouped: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Schema 26: a project that never chose a click voice or turned the accent off saves
    /// exactly as before (no new keys), and one that did reads back what it chose.
    #[test]
    fn the_metronome_choices_are_written_only_when_chosen() {
        let plain = RecordSettings::default();
        let json = serde_json::to_string(&plain).unwrap();
        assert!(
            !json.contains("click_sound") && !json.contains("click_accent"),
            "{json}"
        );
        let old: RecordSettings = serde_json::from_str(r#"{"count_in_bars": 1}"#).unwrap();
        assert_eq!(old.click_sound, ClickSound::Classic);
        assert!(
            old.click_accent,
            "an older file keeps its accented downbeat"
        );
        let chosen = RecordSettings {
            click_sound: ClickSound::Wood,
            click_accent: false,
            ..RecordSettings::default()
        };
        let json = serde_json::to_string(&chosen).unwrap();
        assert_eq!(
            serde_json::from_str::<RecordSettings>(&json).unwrap(),
            chosen
        );
    }

    #[test]
    fn the_fine_offset_defaults_to_zero_and_round_trips() {
        let mut a = AudioRegion::of(AudioSourceId(3));
        let json = serde_json::to_string(&a).unwrap();
        // Nothing is written for a clip without one: files from before it read the same.
        assert!(!json.contains("slip_frames"), "{json}");
        assert_eq!(serde_json::from_str::<AudioRegion>(&json).unwrap(), a);
        a.slip_frames = -11;
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(serde_json::from_str::<AudioRegion>(&json).unwrap(), a);
    }
}
