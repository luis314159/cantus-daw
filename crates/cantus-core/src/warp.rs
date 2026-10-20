//! **Warping** an audio clip (round 29, ADR 0061): where its content's beats fall in its
//! source, how it is stretched to follow the project, and its transposition — stored with
//! the clip (`AudioRegion::warp`, schema 34).
//!
//! **The warp map.** A warped clip's content is measured in **content ticks** (as every
//! clip's is: content tick `t` sits at `start − offset + t` on the timeline) and its source
//! in **source frames** at the file's own rate. The map between them runs through the
//! **warp markers** — each a source frame paired with a content tick — linearly between
//! two, and beyond the first and the last at the slope of the segment beside it (with one
//! marker, or none, at the source's own tempo [`Warp::bpm`]: one beat of content is
//! `60 / bpm` seconds of source). Positions are source frames and content ticks, never
//! timeline ticks, so split, trim, slip, glue, loops and linked copies keep every marker
//! valid, as the pitch editor's notes do (ADR 0054).
//!
//! **Playing.** A warped clip *follows the project's tempo*: content tick `t` is heard at
//! its timeline tick, and the source under it is `frame_at(t)` (`crate::audio_time::clip_frame_at`,
//! the one mapping the engine and the interface share) — the engine renders the
//! source through the map (`cantus_dsp::stretch`) and plays the render. With
//! [`Warp::follow`] off the clip plays at its natural speed (ADR 0025's rule) and only its
//! transposition applies. A warp that maps exactly as the unwarped clip does, untransposed
//! ([`Warp::is_identity`]), plays the source itself — the same bits as no warp at all.

use crate::time::{Ticks, PPQ};
use serde::{Deserialize, Serialize};

#[path = "warp_edit.rs"]
mod edit;
#[cfg(test)]
#[path = "warp_tests.rs"]
mod tests;

/// The source tempos a warp accepts (the project's own range).
pub const MIN_BPM: f64 = crate::time::MIN_BPM;
pub const MAX_BPM: f64 = crate::time::MAX_BPM;
/// The widest transposition, cents either way (two octaves).
pub const MAX_CENTS: i32 = 2_400;
/// The most markers a clip holds.
pub const MAX_MARKERS: usize = 20_000;
/// Two markers never closer than this, ticks (and a frame apart at least).
pub const MIN_GAP: Ticks = Ticks(1);
/// A marker frame this close to the unwarped clip's is the same frame (identity).
const SAME_FRAME: f64 = 1e-6;

/// How a warped clip is stretched (ADR 0061; `cantus_dsp::stretch::Mode`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WarpMode {
    /// Mixes and anything polyphonic: phase-locked STFT, transients copied.
    #[default]
    Complex,
    /// Drums and percussive loops: slices at the transients.
    Rhythm,
    /// A voice or a solo instrument: PSOLA on its epochs.
    Melodic,
    /// Tape: speed and pitch together.
    Repitch,
}

impl WarpMode {
    pub const ALL: [WarpMode; 4] = [
        WarpMode::Complex,
        WarpMode::Rhythm,
        WarpMode::Melodic,
        WarpMode::Repitch,
    ];

    /// The words' key (`warp.mode.<id>`).
    pub fn id(self) -> &'static str {
        match self {
            WarpMode::Complex => "complex",
            WarpMode::Rhythm => "rhythm",
            WarpMode::Melodic => "melodic",
            WarpMode::Repitch => "repitch",
        }
    }

    /// Whether the mode transposes apart from its speed (Re-pitch does not).
    pub fn transposes(self) -> bool {
        self != WarpMode::Repitch
    }

    /// Whether the mode keeps formants while it transposes.
    pub fn keeps_formants(self) -> bool {
        matches!(self, WarpMode::Complex | WarpMode::Melodic)
    }
}

/// What a Rhythm slice does when the clip is slowed and the slice runs out first.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WarpTail {
    #[default]
    Fade,
    Loop,
    PingPong,
}

impl WarpTail {
    pub const ALL: [WarpTail; 3] = [WarpTail::Fade, WarpTail::Loop, WarpTail::PingPong];

    pub fn id(self) -> &'static str {
        match self {
            WarpTail::Fade => "fade",
            WarpTail::Loop => "loop",
            WarpTail::PingPong => "pingpong",
        }
    }
}

/// One warp marker: this source frame is heard at this content tick.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WarpMarker {
    /// A frame of the source at its own rate (fractions kept).
    pub frame: f64,
    pub tick: Ticks,
    /// Laid by the warp itself, never by the user (schema 38, round 29's second review): a
    /// sample of the tempo curve a first warp edit keeps the clip on
    /// ([`Warp::natural_of`] — every tempo point and, inside a ramp, as many as keep the
    /// curve within a quarter of a frame). Never drawn, never grabbed, never removed by
    /// hand; a **soft** point of the render map (the stretcher bends through it); a
    /// user's marker moved past it carries it along (its place between the user's markers
    /// either side scaled, so the curve keeps its shape).
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto: bool,
}

impl WarpMarker {
    /// A user's marker: `frame` heard at content tick `tick`.
    pub fn at(frame: f64, tick: Ticks) -> Self {
        WarpMarker {
            frame,
            tick,
            auto: false,
        }
    }
}

/// What the analysis found in the source when it was warped (kept for the inspector and
/// *Set the project tempo from this clip*; the transients themselves stay in the cache).
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Detected {
    pub bpm: f64,
    /// 0 … 1.
    pub confidence: f32,
    /// The first downbeat, a source frame.
    pub downbeat: f64,
    /// Whole bars, when the file is a loop.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bars: Option<u32>,
}

/// An audio clip's warp (`AudioRegion::warp`, schema 34). See the module docs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Warp {
    /// The source's own rate: every frame here counts at it.
    pub rate: u32,
    pub mode: WarpMode,
    /// The source's tempo: outside the markers one beat of content is `60 / bpm` seconds of
    /// source.
    pub bpm: f64,
    /// In order: ticks and frames both strictly increasing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub markers: Vec<WarpMarker>,
    /// The transposition, cents.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cents: i32,
    /// Keep the formants while transposing (Complex, Melodic).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub formants: bool,
    /// Rhythm's slice ends.
    #[serde(default, skip_serializing_if = "is_fade")]
    pub tail: WarpTail,
    /// Follow the project's tempo; off, the clip plays at its natural speed, transposed.
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub follow: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected: Option<Detected>,
}

fn is_false(b: &bool) -> bool {
    !*b
}
fn is_zero(v: &i32) -> bool {
    *v == 0
}
fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}
fn is_fade(t: &WarpTail) -> bool {
    *t == WarpTail::Fade
}

/// Source frames per content tick at `bpm` and `rate`.
pub fn frames_per_tick(rate: u32, bpm: f64) -> f64 {
    rate as f64 * 60.0 / (bpm * PPQ as f64)
}

/// The unwarped clip's source frame under content tick `t` (ADR 0025's rule at a constant
/// tempo, with the region's fine offset): `seconds(t) · rate + slip`.
pub fn unwarped_frame(t: f64, project_bpm: f64, rate: u32, slip_frames: i32) -> f64 {
    t / PPQ as f64 * 60.0 / project_bpm * rate as f64 + slip_frames as f64
}

impl Warp {
    /// A warp of a source at `rate` played at its tempo `bpm`, content tick 0 on frame 0.
    pub fn new(rate: u32, bpm: f64) -> Self {
        Warp {
            rate,
            mode: WarpMode::Complex,
            bpm,
            markers: Vec::new(),
            cents: 0,
            formants: true,
            tail: WarpTail::Fade,
            follow: true,
            detected: None,
        }
    }

    /// The warp that plays exactly as the unwarped clip does at `project_bpm` (its fine
    /// offset `slip_frames` in a marker on content tick 0): where every warp edit starts.
    pub fn identity(rate: u32, project_bpm: f64, slip_frames: i32) -> Self {
        let mut w = Warp::new(rate, project_bpm);
        w.markers
            .push(WarpMarker::at(slip_frames as f64, Ticks::ZERO));
        w
    }

    /// The transposition in semitones.
    pub fn semitones(&self) -> f64 {
        self.cents as f64 / 100.0
    }

    /// Source frames per tick beside marker segment `i` (`i` from 0 before the first to
    /// `markers.len()` after the last): an inner segment's own, an edge the segment beside
    /// it, and the source's tempo with fewer than two markers.
    fn slope(&self, i: usize) -> f64 {
        let m = &self.markers;
        if m.len() < 2 {
            return frames_per_tick(self.rate, self.bpm);
        }
        let i = i.clamp(1, m.len() - 1);
        let (a, b) = (m[i - 1], m[i]);
        (b.frame - a.frame) / (b.tick.0 - a.tick.0) as f64
    }

    /// The source frame heard at content tick `t` (see the module docs).
    pub fn frame_at(&self, t: f64) -> f64 {
        let m = &self.markers;
        if m.is_empty() {
            return t * frames_per_tick(self.rate, self.bpm);
        }
        let i = m.partition_point(|k| (k.tick.0 as f64) <= t);
        let anchor = if i == 0 { m[0] } else { m[i - 1] };
        anchor.frame + (t - anchor.tick.0 as f64) * self.slope(i)
    }

    /// The content tick where source frame `f` is heard (the inverse of
    /// [`Warp::frame_at`]).
    pub fn tick_at(&self, f: f64) -> f64 {
        let m = &self.markers;
        if m.is_empty() {
            return f / frames_per_tick(self.rate, self.bpm);
        }
        let i = m.partition_point(|k| k.frame <= f);
        let anchor = if i == 0 { m[0] } else { m[i - 1] };
        anchor.tick.0 as f64 + (f - anchor.frame) / self.slope(i)
    }

    /// The tempo the source is played at around content tick `t`, relative to its own
    /// (1: as recorded; 2: twice as fast) at `project_bpm`.
    pub fn speed_at(&self, t: f64, project_bpm: f64) -> f64 {
        let i = self.markers.partition_point(|k| (k.tick.0 as f64) <= t);
        self.slope(i) / frames_per_tick(self.rate, project_bpm)
    }

    /// Whether the clip plays its source as it is under any tempo: not following the tempo
    /// and not transposed (nothing is ever rendered for it).
    pub fn natural(&self) -> bool {
        !self.follow && !(self.cents != 0 && self.mode.transposes())
    }

    /// Whether the clip plays exactly as without a warp at `project_bpm` with fine offset
    /// `slip_frames`: untransposed, and either not following the tempo or mapping every
    /// content tick to the unwarped clip's frame.
    pub fn is_identity(&self, project_bpm: f64, slip_frames: i32) -> bool {
        if self.cents != 0 && self.mode.transposes() {
            return false;
        }
        if !self.follow {
            return true;
        }
        let want = frames_per_tick(self.rate, project_bpm);
        let same_slope = |k: f64| ((k - want) / want).abs() <= 1e-12;
        let on_line = self.markers.iter().all(|m| {
            (m.frame - unwarped_frame(m.tick.0 as f64, project_bpm, self.rate, slip_frames)).abs()
                <= SAME_FRAME
        });
        if self.markers.is_empty() {
            return slip_frames == 0 && same_slope(self.slope(0));
        }
        on_line && same_slope(self.slope(0)) && same_slope(self.slope(self.markers.len()))
    }

    /// Whether the engine can play this warp: a rate, a tempo in range, markers in order
    /// (ticks and frames strictly increasing, finite, at most [`MAX_MARKERS`]), a
    /// transposition in range. A command refuses anything else.
    pub fn is_valid(&self) -> bool {
        self.rate > 0
            && self.bpm.is_finite()
            && (MIN_BPM..=MAX_BPM).contains(&self.bpm)
            && self.cents.abs() <= MAX_CENTS
            && self.markers.len() <= MAX_MARKERS
            && self.markers.iter().all(|m| m.frame.is_finite())
            && self
                .markers
                .windows(2)
                .all(|w| w[1].tick.0 - w[0].tick.0 >= MIN_GAP.0 && w[1].frame > w[0].frame)
            && self.detected.is_none_or(|d| {
                d.bpm.is_finite() && d.downbeat.is_finite() && d.confidence.is_finite()
            })
    }
}

/// The tempo `project` holds throughout, when it holds one (no tempo track, or one that
/// never changes it): under it a following warp that maps as the unwarped clip does plays
/// its source as it is.
pub fn held_tempo(project: &crate::Project) -> Option<f64> {
    (project.tempo_points.len() <= 1 || project.tempo_map().is_constant())
        .then_some(project.tempo_bpm)
}

/// Whether region `a` plays its source as it is in a project holding `held`
/// ([`held_tempo`]): no warp, a warp that neither follows nor transposes
/// ([`Warp::natural`]), or one that maps exactly as the unwarped clip does under the tempo
/// held ([`Warp::is_identity`]) — nothing rendered. One rule for the snapshot and the
/// comp seams (round 29's second review: a seam dropped its crossfade on a window that
/// played its source).
pub fn plays_source(a: &crate::audio::AudioRegion, held: Option<f64>) -> bool {
    a.warp
        .as_ref()
        .is_none_or(|w| w.natural() || held.is_some_and(|bpm| w.is_identity(bpm, a.slip_frames)))
}

/// What a warp edit did, for the undo menu (`cmd.warp.<id>`) and for merging a drag into
/// one step.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarpOp {
    /// The whole warp set (warped from the analysis, pasted, bounced back), or removed.
    Set,
    AddMarker,
    MoveMarker,
    /// The content under a marker moved (Alt-drag).
    MoveContent,
    RemoveMarker,
    Quantise,
    Mode,
    Transpose,
    Formants,
    Tail,
    Follow,
    Tempo,
    /// Every marker removed (the warp kept at the source's tempo).
    Reset,
}

impl WarpOp {
    pub const ALL: [WarpOp; 13] = [
        WarpOp::Set,
        WarpOp::AddMarker,
        WarpOp::MoveMarker,
        WarpOp::MoveContent,
        WarpOp::RemoveMarker,
        WarpOp::Quantise,
        WarpOp::Mode,
        WarpOp::Transpose,
        WarpOp::Formants,
        WarpOp::Tail,
        WarpOp::Follow,
        WarpOp::Tempo,
        WarpOp::Reset,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            WarpOp::Set => "cmd.warp.set",
            WarpOp::AddMarker => "cmd.warp.add_marker",
            WarpOp::MoveMarker => "cmd.warp.move_marker",
            WarpOp::MoveContent => "cmd.warp.move_content",
            WarpOp::RemoveMarker => "cmd.warp.remove_marker",
            WarpOp::Quantise => "cmd.warp.quantise",
            WarpOp::Mode => "cmd.warp.mode",
            WarpOp::Transpose => "cmd.warp.transpose",
            WarpOp::Formants => "cmd.warp.formants",
            WarpOp::Tail => "cmd.warp.tail",
            WarpOp::Follow => "cmd.warp.follow",
            WarpOp::Tempo => "cmd.warp.tempo",
            WarpOp::Reset => "cmd.warp.reset",
        }
    }

    /// Whether a run of this edit on one clip merges into one undo step (a drag).
    pub fn merges(self) -> bool {
        matches!(
            self,
            WarpOp::MoveMarker | WarpOp::MoveContent | WarpOp::Transpose | WarpOp::Tempo
        )
    }
}

/// Drop every warp a project holds that is not valid ([`Warp::is_valid`]: a hand-edited or
/// damaged file) — its clip plays at its natural speed — and return how many. Not a
/// command: it runs on load, before the project has a history (round 29's review: the
/// commands refuse such a warp, so a file must not bring one in).
pub fn repair(project: &mut crate::Project) -> usize {
    let mut dropped = 0;
    for t in &mut project.tracks {
        // The kept windows of its alternate comps too (round 29's second review: switching
        // to such a comp was refused for good).
        let windows = t.comps.iter_mut().flat_map(|c| c.windows.iter_mut());
        for c in t.clips.iter_mut().chain(windows) {
            if let Some(a) = c.audio.as_mut() {
                if a.warp.as_ref().is_some_and(|w| !w.is_valid()) {
                    a.warp = None;
                    dropped += 1;
                }
            }
        }
    }
    dropped
}
