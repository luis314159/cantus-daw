//! The session view's model (round 33, ADR 0076, schema 46): Ableton Live's and Bitwig's
//! clip launcher — a grid of clips per track and scenes.
//!
//! ## Where it lives
//!
//! The session shares everything with the arrangement: the **same tracks** (a column of
//! the grid is a [`Track`]), the same instruments, the same mixer and the same automation.
//! So the grid is split in two, each half next to what it belongs to:
//!
//! - [`Project::session`] — the rows: the [`Scene`]s in their order, and the
//!   [`SessionSettings`] (the global launch and record quantisation, the seed the follow
//!   actions draw their chances from, a fixed recording length).
//! - [`Track::slots`] — the track's column: one [`Slot`] per scene that holds something (a
//!   clip, or an empty slot whose stop button was taken away). An absent slot is an empty
//!   one with its stop button, so a project with no session — every file before schema 46 —
//!   stores nothing, and a track's column travels with it: removing a track and undoing it,
//!   moving it, freezing it, all keep its column with no case of their own.
//!
//! A slot's clip is a [`SessionClip`]: an ordinary [`Clip`] — so the piano roll, the audio
//! editor and every content command (`InsertNotes`, `SetClipAudio`, `SetClipWarp`,
//! `SetClipLanes`, the name and colour, …) edit it exactly as they edit an arrangement clip,
//! found by its id through `Track::clip` — plus how it launches ([`LaunchSettings`]), what
//! follows it ([`FollowActions`]), its start marker and its own envelopes.
//!
//! ## A session clip's window
//!
//! The clip's window fields keep their arrangement meaning, read from the moment it is
//! launched instead of from `start` (which stays 0): it plays from content tick `offset`;
//! without a loop (`content_length` `None`) it plays `length` ticks and ends; with one it
//! repeats `[offset, offset + content_length)` until it is stopped — Live's *Loop* switch,
//! with `length` kept as the end marker the clip goes back to when the loop is switched off.
//! The start marker ([`SessionClip::start`]) lets a looping clip begin elsewhere than its
//! loop's start, as Live's does.
//!
//! What plays — which slot each track is playing, which are queued, whether a track
//! follows the arrangement (Live's *Back to Arrangement*) — is the engine's state, not the
//! document's: it is not undone, and the engine reports it (`cantus_engine::messages`).

use crate::automation::Automation;
use crate::model::{Clip, ClipId, Color, Project, Track, TrackId};
use crate::tempo::TempoMap;
use crate::time::{Ticks, TimeSignature, PPQ};
use serde::{Deserialize, Serialize};
use std::fmt;

#[path = "session_cmd.rs"]
mod cmd;
#[path = "session_edit.rs"]
pub mod edit;
#[path = "session_guard.rs"]
mod guard;
#[path = "session_play.rs"]
pub mod play;
#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;

pub(crate) use cmd::{apply, touches};
pub use cmd::{SessionCommand, SlotRegion};
pub(crate) use guard::refuse;

/// A scene: a row of the grid. Stable across saves; never reused.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneId(pub u64);

impl fmt::Display for SceneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sc{}", self.0)
    }
}

/// A slot of the grid: a track's cell in a scene's row.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SlotRef {
    pub track: TrackId,
    pub scene: SceneId,
}

impl SlotRef {
    pub fn new(track: TrackId, scene: SceneId) -> Self {
        SlotRef { track, scene }
    }
}

/// The rows of the grid and the session's own settings (`Project::session`). The default —
/// every project before schema 46 — is no session at all, and is not written.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    /// The scenes, top to bottom.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scenes: Vec<Scene>,
    #[serde(default, skip_serializing_if = "SessionSettings::is_default")]
    pub settings: SessionSettings,
}

impl Session {
    /// No scenes and the settings at their defaults: what is not saved.
    pub fn is_default(&self) -> bool {
        *self == Session::default()
    }

    pub fn scene(&self, id: SceneId) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.id == id)
    }

    pub fn scene_mut(&mut self, id: SceneId) -> Option<&mut Scene> {
        self.scenes.iter_mut().find(|s| s.id == id)
    }

    /// Where scene `id` sits, top to bottom.
    pub fn scene_index(&self, id: SceneId) -> Option<usize> {
        self.scenes.iter().position(|s| s.id == id)
    }
}

/// A row: its launch button's name and colour, and what launching it does besides
/// launching its clips — a tempo and a metre it sets, and what follows it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: SceneId,
    /// Shown on its launch button; may be empty (the view shows its number).
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// The song's tempo once the scene is launched (Live's scene tempo), in BPM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo: Option<f64>,
    /// The song's metre once the scene is launched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metre: Option<TimeSignature>,
    /// What follows the scene (Live 12's scene follow actions): off by default.
    #[serde(default, skip_serializing_if = "FollowActions::is_off")]
    pub follow: FollowActions,
}

impl Scene {
    pub fn new(id: SceneId, name: impl Into<String>) -> Self {
        Scene {
            id,
            name: name.into(),
            color: None,
            tempo: None,
            metre: None,
            follow: FollowActions::off(),
        }
    }

    /// Whether every field is one a command takes.
    pub fn is_valid(&self) -> bool {
        self.tempo.is_none_or(|b| {
            b.is_finite() && (crate::time::MIN_BPM..=crate::time::MAX_BPM).contains(&b)
        }) && self.metre.is_none_or(metre_is_valid)
            && self.follow.is_valid()
    }
}

/// A metre a scene may set: what `SetTimeSignature` takes.
pub fn metre_is_valid(m: TimeSignature) -> bool {
    m.numerator > 0 && matches!(m.denominator, 1 | 2 | 4 | 8 | 16 | 32)
}

/// A track's cell in a scene's row that holds something: a clip, or nothing with its stop
/// button taken away (launching the scene then leaves the track playing, as Live's
/// *Remove Stop Button*). Never stored when it is an empty slot with its stop button.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Slot {
    pub scene: SceneId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<SessionClip>,
    /// An empty slot stops its track when its scene is launched. Ignored while it holds a
    /// clip (launching plays the clip).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub stop: bool,
}

fn yes() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

impl Slot {
    /// An empty slot with its stop button: the one never stored.
    pub fn is_default(&self) -> bool {
        self.clip.is_none() && self.stop
    }
}

/// A clip in a slot: an ordinary [`Clip`] (see the module docs for what its window means
/// here) and how it launches, what follows it and its own envelopes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SessionClip {
    pub clip: Clip,
    #[serde(default, skip_serializing_if = "LaunchSettings::is_default")]
    pub launch: LaunchSettings,
    #[serde(default, skip_serializing_if = "FollowActions::is_off")]
    pub follow: FollowActions,
    /// The start marker: the content tick a launch plays from, when it is not `offset`
    /// (Live's start marker ahead of or inside the loop). With a loop the clip then plays to
    /// the loop's end and repeats the loop; without one it plays to the end marker
    /// (`offset + length`). Always in `[0, end)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<Ticks>,
    /// The clip's own envelopes — the session's automation, per clip (Live's clip
    /// envelopes): curves of mixer and device parameters in the clip's content ticks, played
    /// while the clip plays (they loop with it) and handing back to the track's own
    /// automation when it stops. At most one per target.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub envelopes: Vec<Automation>,
}

impl SessionClip {
    /// `clip` in a slot with the default launch settings and nothing following it.
    pub fn new(clip: Clip) -> Self {
        SessionClip {
            clip,
            launch: LaunchSettings::default(),
            follow: FollowActions::off(),
            start: None,
            envelopes: Vec::new(),
        }
    }

    pub fn id(&self) -> ClipId {
        self.clip.id
    }

    /// Whether it loops (Live's *Loop* switch).
    pub fn looping(&self) -> bool {
        self.clip.period().is_some()
    }

    /// One past the last content tick it plays before ending or wrapping: the loop's end
    /// with a loop, the end marker without one.
    pub fn end(&self) -> Ticks {
        match self.clip.period() {
            Some(p) => self.clip.offset + p,
            None => self.clip.offset + self.clip.length,
        }
    }

    /// The content tick a launch plays from.
    pub fn play_from(&self) -> Ticks {
        self.start.unwrap_or(self.clip.offset)
    }

    /// How long one play of the clip lasts from its start marker to its end (one pass of
    /// the loop, the first one when the start marker is not the loop's start): what a
    /// follow action *linked* to the clip waits for.
    pub fn first_pass(&self) -> Ticks {
        self.end() - self.play_from()
    }

    /// One pass of the loop (or the whole clip without one): what every later pass lasts.
    pub fn pass(&self) -> Ticks {
        self.clip.period().unwrap_or(self.clip.length)
    }

    /// The content tick heard `elapsed` ticks after the clip was launched (from its start
    /// marker): `None` once a clip that does not loop has ended.
    pub fn content_at(&self, elapsed: Ticks) -> Option<Ticks> {
        self.content_from(self.play_from(), elapsed)
    }

    /// The content tick heard `elapsed` ticks after the clip started playing from content
    /// tick `from` (its start marker, or where legato took over): it runs to the end, then
    /// repeats the loop. `None` once a clip that does not loop has ended.
    pub fn content_from(&self, from: Ticks, elapsed: Ticks) -> Option<Ticks> {
        if elapsed.is_negative() {
            return None;
        }
        let first = (self.end() - from).max(Ticks::ZERO);
        if elapsed < first {
            return Some(from + elapsed);
        }
        let period = self.clip.period()?;
        Some(self.clip.offset + Ticks((elapsed - first).0.rem_euclid(period.0)))
    }

    /// The envelope of `target`, if the clip has one.
    pub fn envelope(&self, target: &crate::automation::AutoTarget) -> Option<&Automation> {
        self.envelopes.iter().find(|e| &e.target == target)
    }
}

#[path = "session_types.rs"]
mod types;
pub use types::{
    FollowAction, FollowActions, FollowAfter, LaunchMode, LaunchSettings, QuantGrid, Quantize,
    RecordQuantize, SessionSettings, MAX_FIXED_BARS, MAX_FOLLOW_COUNT,
};

/// The next launch boundary of `grid` at or after `tick`, counted on `map`'s bar lines
/// (every metre change): a bar grid of `n` lands on bars 1, 1 + n, 1 + 2n …; a note grid
/// restarts at every bar line, so a triplet grid stays in step with the bars.
pub fn boundary_at_or_after(map: &TempoMap, tick: Ticks, grid: QuantGrid) -> Ticks {
    match grid {
        QuantGrid::None => tick,
        QuantGrid::Bars(n) => {
            let n = i64::from(n.max(1));
            let (bar, beat, t) = map.bbt(tick);
            let idx = bar - 1;
            if beat == 1 && t == 0 && idx.rem_euclid(n) == 0 {
                return tick;
            }
            let next = (idx.div_euclid(n) + 1) * n;
            map.bar_tick(next + 1)
        }
        QuantGrid::Ticks(g) => {
            let g = g.max(1);
            let bar = map.bar_start(tick);
            let rel = (tick - bar).0;
            let k = (rel + g - 1).div_euclid(g);
            let next = bar + Ticks(k * g);
            next.min(map.bar_end(tick))
        }
    }
}

/// A quarter note's ticks, for the note grids (`QuantGrid::Ticks`).
pub(crate) const QUARTER: i64 = PPQ;

// ---- lookups on the model ------------------------------------------------------------

impl Track {
    /// Clip `id` of the arrangement only (`Track::clip` finds session clips too): what an
    /// edit of the timeline — cut, split, link, crossfade, comp, pattern — reads.
    pub fn arranged(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id)
    }

    /// [`Self::arranged`], to change.
    pub fn arranged_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|c| c.id == id)
    }

    /// The track's slot in scene `scene`, if it holds something.
    pub fn slot(&self, scene: SceneId) -> Option<&Slot> {
        self.slots.iter().find(|s| s.scene == scene)
    }

    /// The clip in the track's slot of scene `scene`.
    pub fn slot_clip(&self, scene: SceneId) -> Option<&SessionClip> {
        self.slot(scene).and_then(|s| s.clip.as_ref())
    }

    /// Every session clip of the track's column, by scene id.
    pub fn session_clips(&self) -> impl Iterator<Item = &SessionClip> {
        self.slots.iter().filter_map(|s| s.clip.as_ref())
    }

    /// The scene whose slot holds clip `id`, when it is one of the track's session clips.
    pub fn scene_of_clip(&self, id: ClipId) -> Option<SceneId> {
        self.slots
            .iter()
            .find(|s| s.clip.as_ref().is_some_and(|c| c.clip.id == id))
            .map(|s| s.scene)
    }

    /// Every clip the track holds, the arrangement's then its session's: what a check of
    /// a source, a groove or an id in use reads.
    pub fn all_clips(&self) -> impl Iterator<Item = &Clip> {
        self.clips
            .iter()
            .chain(self.session_clips().map(|s| &s.clip))
    }
}

/// A frozen track's key (`freeze::track_part`) leaves out what is nothing to hear in its
/// column: a session clip's name and colour (as an arrangement clip's).
pub(crate) fn scrub_cosmetic(track: &mut serde_json::Value) {
    let Some(serde_json::Value::Array(slots)) = track.get_mut("slots") else {
        return;
    };
    for s in slots {
        if let Some(serde_json::Value::Object(c)) =
            s.get_mut("clip").and_then(|c| c.get_mut("clip"))
        {
            c.remove("name");
            c.remove("color");
        }
    }
}

/// `Track::clip`'s second half (`model.rs`): a session clip found by its id.
pub(crate) fn find_clip(track: &Track, id: ClipId) -> Option<&Clip> {
    track
        .slots
        .iter()
        .filter_map(|s| s.clip.as_ref())
        .map(|s| &s.clip)
        .find(|c| c.id == id)
}

/// `Track::clip_mut`'s second half.
pub(crate) fn find_clip_mut(track: &mut Track, id: ClipId) -> Option<&mut Clip> {
    track
        .slots
        .iter_mut()
        .filter_map(|s| s.clip.as_mut())
        .map(|s| &mut s.clip)
        .find(|c| c.id == id)
}

impl Project {
    /// The clip in a slot.
    pub fn slot_clip(&self, slot: SlotRef) -> Option<&SessionClip> {
        self.track(slot.track)?.slot_clip(slot.scene)
    }

    /// Whether the project has a session: a scene, or settings someone set.
    pub fn has_session(&self) -> bool {
        !self.session.scenes.is_empty()
    }

    /// What a command would refuse in the session (a hand edit), made what a command
    /// takes: scenes twice or with settings out of range, slots of scenes that are not
    /// there or twice, a session clip out of a session clip's form (it leaves its group,
    /// its comp and its take lane; an automation clip is dropped) or with settings out of
    /// range, and every column in its canonical order. Loading a file does it before
    /// anything reads the project. Returns how many things it changed.
    pub fn repair_session(&mut self) -> usize {
        let mut fixed = 0;
        let s = &mut self.session;
        if !s.settings.is_valid() {
            s.settings = SessionSettings::default();
            fixed += 1;
        }
        let mut seen = std::collections::HashSet::new();
        let before = s.scenes.len();
        s.scenes.retain(|x| seen.insert(x.id));
        fixed += before - s.scenes.len();
        for x in &mut s.scenes {
            if !x.is_valid() {
                x.tempo = x.tempo.filter(|b| {
                    b.is_finite() && (crate::time::MIN_BPM..=crate::time::MAX_BPM).contains(b)
                });
                x.metre = x.metre.filter(|m| metre_is_valid(*m));
                if !x.follow.is_valid() {
                    x.follow = FollowActions::off();
                }
                fixed += 1;
            }
        }
        let scenes = seen;
        for t in &mut self.tracks {
            let before = t.slots.len();
            for slot in &mut t.slots {
                if slot
                    .clip
                    .as_ref()
                    .is_some_and(|c| c.clip.automation.is_some())
                {
                    slot.clip = None;
                }
            }
            t.slots
                .retain(|s| scenes.contains(&s.scene) && !s.is_default());
            t.slots.sort_by_key(|s| s.scene);
            t.slots.dedup_by_key(|s| s.scene);
            fixed += before - t.slots.len();
            for c in t.slots.iter_mut().filter_map(|s| s.clip.as_mut()) {
                fixed += usize::from(repair_clip(c));
                // A groove a command would refuse (gone from the pool, out of range).
                let groove_ok = c.clip.groove.is_none_or(|g| {
                    c.clip.audio.is_none() && g.is_valid() && g.source.exists(&self.grooves)
                });
                if !groove_ok {
                    c.clip.groove = None;
                    fixed += 1;
                }
            }
        }
        fixed
    }
}

/// A session clip put back in a session clip's form, its settings in range: whether
/// anything changed.
fn repair_clip(c: &mut SessionClip) -> bool {
    let was = c.clone();
    let k = &mut c.clip;
    k.notes.retain(|n| crate::command::validate_note(n).is_ok());
    if let Some(a) = k.audio.as_mut() {
        if a.warp.as_ref().is_some_and(|w| !w.is_valid()) {
            a.warp = None;
        }
        if a.gain_env.as_ref().is_some_and(|e| !e.is_valid()) {
            a.gain_env = None;
        }
        a.xfade = a.xfade.clamp(Ticks::ZERO, crate::audio::MAX_XFADE);
    }
    if k.length <= Ticks::ZERO {
        k.length = Ticks::QUARTER;
    }
    if k.content_length.is_some_and(|l| l <= Ticks::ZERO) {
        k.content_length = None;
    }
    k.start = Ticks::ZERO;
    k.lane = 0;
    k.link = None;
    k.comp = false;
    k.offset = k.offset.max(Ticks::ZERO);
    if !c.launch.is_valid() {
        c.launch = LaunchSettings::default();
    }
    if !c.follow.is_valid() {
        c.follow = FollowActions::off();
    }
    if c.start.is_some_and(|s| s.is_negative() || s >= c.end()) {
        c.start = None;
    }
    let mut targets = Vec::new();
    c.envelopes.retain(|e| {
        let keep = e.is_valid() && !targets.contains(&e.target);
        targets.push(e.target.clone());
        keep
    });
    *c != was
}

/// A track's column as `InsertTrack` takes it (an undo of its removal, a copy): in its
/// canonical order, every slot of a scene that is there and worth storing, every clip one
/// `InsertSlotClip` takes, and no clip id the track's arrangement, its other slots or any
/// other track already holds.
pub(crate) fn check_column(project: &Project, track: &Track) -> Result<(), crate::CommandError> {
    let bad = |w: &str| crate::CommandError::InvalidValue(w.to_string());
    if track.slots.windows(2).any(|w| w[0].scene >= w[1].scene) {
        return Err(bad("session column order"));
    }
    let mut ids: Vec<ClipId> = track.clips.iter().map(|c| c.id).collect();
    for s in &track.slots {
        if project.session.scene(s.scene).is_none() || s.is_default() {
            return Err(bad("session slot"));
        }
        let Some(c) = &s.clip else {
            continue;
        };
        cmd::check_clip(project, c)?;
        let id = c.clip.id;
        if ids.contains(&id) || project.tracks.iter().any(|t| t.clip(id).is_some()) {
            return Err(crate::CommandError::DuplicateId(id.0));
        }
        ids.push(id);
    }
    Ok(())
}

/// The track an envelope's target belongs to, when it is one of a track's own parameters
/// (its level and balance, its instrument's, its note effects').
fn local_track(t: &crate::automation::AutoTarget) -> Option<TrackId> {
    use crate::automation::AutoTarget as A;
    match t {
        A::TrackGain(x) | A::TrackPan(x) => Some(*x),
        A::Instrument { track, .. }
        | A::Firmus { track, .. }
        | A::PluginInstrument { track, .. }
        | A::NoteFx { track, .. } => Some(*track),
        _ => None,
    }
}

/// Whether a clip whose envelopes are `envs` can move from track `a` to track `b`: its
/// envelopes of either track's own parameters are then the other's — a level and a
/// balance always, an instrument's when the two play the same kind of instrument, a note
/// effect's never (its id is its track's).
pub(crate) fn envelopes_move(
    project: &Project,
    envs: &[Automation],
    a: TrackId,
    b: TrackId,
) -> bool {
    use crate::automation::AutoTarget as A;
    if a == b {
        return true;
    }
    let same_kind = match (project.track(a), project.track(b)) {
        (Some(x), Some(y)) => {
            std::mem::discriminant(&x.instrument) == std::mem::discriminant(&y.instrument)
        }
        _ => false,
    };
    envs.iter()
        .all(|e| match (&e.target, local_track(&e.target)) {
            (_, None) => true,
            (_, Some(t)) if t != a && t != b => true,
            (A::TrackGain(_) | A::TrackPan(_), _) => true,
            (A::NoteFx { .. }, _) => false,
            _ => same_kind,
        })
}

/// Swap tracks `a` and `b` in the envelopes' track-local targets (its own inverse): what a
/// clip moved between the two plays on its new track.
pub(crate) fn swap_targets(envs: &mut [Automation], a: TrackId, b: TrackId) {
    use crate::automation::AutoTarget as A;
    let swap = |x: &mut TrackId| {
        if *x == a {
            *x = b;
        } else if *x == b {
            *x = a;
        }
    };
    for e in envs {
        match &mut e.target {
            A::TrackGain(x) | A::TrackPan(x) => swap(x),
            A::Instrument { track, .. }
            | A::Firmus { track, .. }
            | A::PluginInstrument { track, .. }
            | A::NoteFx { track, .. } => swap(track),
            _ => {}
        }
    }
}

/// A copy of a clip for track `to` from track `from`: its envelopes of `from`'s own
/// parameters retargeted to `to`'s — a level and a balance always; an instrument's only
/// when `to` plays the same instrument (the same native kind, Firmus, or the same plug-in),
/// so the parameter is there; a note effect's only when `to` has a note effect of the same
/// kind at that id — and the rest left out. The one rule for every copy of a session clip
/// to another track (duplicate, paste: round 33, ADR 0077). // r33:launcher
pub fn envelopes_for(project: &Project, c: &mut SessionClip, from: TrackId, to: TrackId) {
    // r33:launcher (pub)
    if from == to {
        return;
    }
    let keep = |t: &_| local_track(t) != Some(from) || envelope_follows(project, t, from, to); // r33:launcher
    c.envelopes.retain(|e| keep(&e.target)); // r33:launcher
    for e in &mut c.envelopes {
        if local_track(&e.target) == Some(from) {
            swap_targets(std::slice::from_mut(e), from, to);
        }
    }
}

// r33:launcher — from here to the end of `envelope_follows`, the launcher's (round 33).
/// Whether an envelope of `from`'s own parameter `t` has that parameter on track `to`.
fn envelope_follows(
    project: &Project,
    t: &crate::automation::AutoTarget,
    from: TrackId,
    to: TrackId,
) -> bool {
    use crate::automation::AutoTarget as A;
    use crate::instrument::InstrumentSpec as I;
    let (Some(x), Some(y)) = (project.track(from), project.track(to)) else {
        return false;
    };
    let note_fx = |tr: &Track, id| {
        tr.note_fx
            .iter()
            .find(|n| n.id == id)
            .map(|n| std::mem::discriminant(&n.kind))
    };
    match t {
        A::TrackGain(_) | A::TrackPan(_) => true,
        A::Instrument { .. } => match (&x.instrument, &y.instrument) {
            (I::Native(a), I::Native(b)) => a.kind == b.kind,
            _ => false,
        },
        A::Firmus { .. } => matches!(y.instrument, I::Firmus(_)),
        A::PluginInstrument { .. } => match (x.instrument.external(), y.instrument.external()) {
            (Some(a), Some(b)) => a.format == b.format && a.id == b.id,
            _ => false,
        },
        A::NoteFx { effect, .. } => {
            note_fx(x, *effect).is_some() && note_fx(x, *effect) == note_fx(y, *effect)
        }
        _ => true,
    }
}
// r33:launcher — end of `envelope_follows`.

/// The unwarped audio clips of the session before a tempo edit (`tempo::keep`): each slot,
/// its window, and the song's tempo they are heard at (the tempo at bar 1).
pub(crate) fn tempo_kept(project: &Project) -> Vec<(SlotRef, cmd::SlotRegion)> {
    let mut out = Vec::new();
    for t in &project.tracks {
        for s in &t.slots {
            let Some(c) = &s.clip else {
                continue;
            };
            let unwarped = c
                .clip
                .audio
                .as_ref()
                .is_some_and(|a| !a.warp.as_ref().is_some_and(|w| w.follow));
            if unwarped {
                out.push((SlotRef::new(t.id, s.scene), cmd::SlotRegion::of(c)));
            }
        }
    }
    out
}

/// After a tempo edit: each unwarped session audio clip keeps its stretch of its file in
/// seconds — its start marker, loop and end scaled by the change of the song's tempo at
/// bar 1 — so it plays the same audio, in time with its own loop (ADR 0076 §9).
pub(crate) fn tempo_keep_after(
    kept: &[(SlotRef, cmd::SlotRegion)],
    bpm_before: f64,
    project: &Project,
) -> Vec<crate::command::Command> {
    let bpm = project.tempo_map().bpm_at(0.0);
    if kept.is_empty() || bpm == bpm_before || bpm_before <= 0.0 {
        return Vec::new();
    }
    let f = bpm / bpm_before;
    let scale = |t: Ticks| Ticks((t.0 as f64 * f).round() as i64);
    let mut out = Vec::new();
    for (slot, r) in kept {
        if project.slot_clip(*slot).is_none() {
            continue;
        }
        let length = scale(r.length).max(Ticks(1));
        let loop_length = r.loop_length.map(|l| scale(l).max(Ticks(1)));
        let offset = scale(r.offset);
        let end = offset + loop_length.unwrap_or(length);
        let start = r.start.map(scale).filter(|s| *s < end);
        out.push(crate::command::Command::Session(
            SessionCommand::SetSlotRegion {
                slot: *slot,
                region: cmd::SlotRegion {
                    start,
                    offset,
                    length,
                    loop_length,
                },
            },
        ));
    }
    out
}
