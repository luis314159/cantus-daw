//! A recording pass, start to end, as data (ADR 0025 §5–§7): what the engine hands back
//! when a pass ends — a WAV per armed track with its loop wraps, and the notes the MIDI
//! recorder closed — turned into the commands that put it in the project, **as one undo
//! step**.
//!
//! Pure functions over a [`Project`], no I/O and no engine, so every case is tested with
//! takes built by hand: loops, punch, count-in, both take modes, MIDI overdub.
//!
//! The rules, as the recording DAWs keep them:
//!
//! - **Nothing recorded is thrown away.** The whole pass is in the file from the moment
//!   the transport started (count-in included); punch and the record start only set the
//!   clip's *window*, so the count-in and anything after the punch-out are handles that a
//!   trim drags back into view (Pro Tools).
//! - **One clip per loop pass.** The last pass plays (lane 0); the earlier ones go to take
//!   lanes, oldest first. A last pass the user stopped in its first quarter is not a take —
//!   it stays in the file, the previous full pass plays.
//! - **Stack** moves what the new take covers on the playing lane to a take lane of its
//!   own; **Replace** cuts it away (tape). Audio treats Overdub as Stack.
//! - **MIDI** follows the same windows. Overdub merges the notes into the clip under the
//!   record start (and so does a loop recording with loop takes off: cycle overdub, the way
//!   a drum part is built).

use crate::clips::Ids;
use crate::recording::RecordedNote;
use cantus_core::audio::{AudioRegion, AudioSource, TakeMode};
use cantus_core::TempoMap;
use cantus_core::{
    AudioSourceId, Clip, ClipId, Command, CommandError, Note, NoteId, Project, Ticks, Track,
    TrackId, PPQ,
};
use cantus_engine::FinishedTake;

/// What was decided when the pass started, and where the playhead was when it ended.
#[derive(Clone, Debug, PartialEq)]
pub struct PassPlan {
    /// Where the take begins: the playhead when record was pressed. With a count-in the
    /// transport started earlier; what was recorded before this is a handle.
    pub record_start: Ticks,
    /// The punch range, when punch was on.
    pub punch: Option<(Ticks, Ticks)>,
    /// The loop, when the transport was looping.
    pub loop_region: Option<(Ticks, Ticks)>,
    pub loop_takes: bool,
    pub mode: TakeMode,
    /// Armed tracks with an input: they record audio.
    pub audio_tracks: Vec<TrackId>,
    /// Armed tracks without one: they record the notes played.
    pub midi_tracks: Vec<TrackId>,
    /// Nothing was armed: the notes go to the selected track, merged into this clip (or
    /// the one under the record start, or a new one).
    pub quick: Option<(TrackId, Option<ClipId>)>,
    /// The playhead when the pass ended.
    pub stop_at: Ticks,
    /// The MIDI recorder's pass when it ended (`Recorder::pass`).
    pub last_pass: u32,
    /// "Take", in the interface's language: clips are named "Vocal · Take 2".
    pub take_word: String,
}

impl PassPlan {
    /// A plan with no tracks, recording from `record_start`.
    pub fn new(record_start: Ticks) -> PassPlan {
        PassPlan {
            record_start,
            punch: None,
            loop_region: None,
            loop_takes: true,
            mode: TakeMode::Stack,
            audio_tracks: Vec::new(),
            midi_tracks: Vec::new(),
            quick: None,
            stop_at: record_start,
            last_pass: 0,
            take_word: "Take".into(),
        }
    }
}

/// A project being edited by a sequence of commands, so that each step of a compound
/// edit is computed against the result of the previous one — and so that a step the
/// project would refuse is found here, not halfway through the user's batch.
pub struct Sim {
    pub project: Project,
    pub commands: Vec<Command>,
}

impl Sim {
    pub fn new(project: &Project) -> Sim {
        Sim {
            project: project.clone(),
            commands: Vec::new(),
        }
    }

    pub fn push(&mut self, commands: Vec<Command>) -> Result<(), CommandError> {
        for c in commands {
            c.clone().apply(&mut self.project)?;
            self.commands.push(c);
        }
        Ok(())
    }

    pub fn track(&self, id: TrackId) -> Option<&Track> {
        self.project.track(id)
    }
}

/// Ticks at the project's tempo for a duration in seconds, unrounded.
pub fn ticks_f(seconds: f64, bpm: f64) -> f64 {
    seconds * bpm / 60.0 * PPQ as f64
}

/// The highest lane a track's clips use (0 when it has no take lanes).
pub fn max_lane(track: &Track) -> u8 {
    track.clips.iter().map(|c| c.lane).max().unwrap_or(0)
}

/// A clip that is `clip`'s content shown through another window, with a fresh id.
pub fn piece(clip: &Clip, start: Ticks, end: Ticks, lane: u8, ids: Ids) -> Clip {
    Clip {
        id: ClipId(ids()),
        name: clip.name.clone(),
        start,
        length: end - start,
        offset: clip.content_of(start),
        content_length: clip.content_length,
        muted: clip.muted,
        color: clip.color,
        notes: clip.notes.clone(),
        audio: clip.audio.clone(),
        lane,
        automation: clip.automation.clone(),
        // The same content through another window: still the same clip (ADR 0037).
        link: clip.link,
        // A comp window's piece on the playing lane is the comp's; on a take lane it is a
        // take (ADR 0058, schema 32).
        comp: lane == 0 && clip.comp,
    }
}

/// Clear `a..b` on the playing lane of `track`: every lane-0 clip that overlaps it is
/// trimmed or split so that nothing of it plays there. With `to_lane`, the pieces that
/// were covered move to that lane (Stack); without, they go (Replace).
///
/// The note ids of a split clip are shared between its pieces, as [`crate::clips::split`]
/// shares them: a piece is the same content through another window.
pub fn clear_range(
    track: &Track,
    a: Ticks,
    b: Ticks,
    to_lane: Option<u8>,
    ids: Ids,
) -> Vec<Command> {
    let mut out = Vec::new();
    if b <= a {
        return out;
    }
    let tid = track.id;
    // Automation clips sit in their own lanes (ADR 0027): a take never cuts them.
    for c in track
        .clips
        .iter()
        .filter(|c| c.lane == 0 && c.automation.is_none())
    {
        if c.start >= b || c.end() <= a {
            continue;
        }
        let (lo, hi) = (a.max(c.start), b.min(c.end()));
        let left = c.start < a;
        let right = c.end() > b;
        if !left && !right {
            out.push(match to_lane {
                Some(lane) => Command::SetClipLane {
                    track: tid,
                    clip: c.id,
                    lane,
                },
                None => Command::RemoveClip {
                    track: tid,
                    clip: c.id,
                },
            });
            // A window stacked onto a take lane is a take now, no longer the comp's.
            if to_lane.is_some() && c.comp {
                out.push(Command::SetClipComp {
                    track: tid,
                    clip: c.id,
                    comp: false,
                });
            }
            continue;
        }
        if let Some(lane) = to_lane {
            out.push(Command::InsertClip {
                track: tid,
                clip: Box::new(piece(c, lo, hi, lane, ids)),
            });
        }
        if right {
            if left {
                out.push(Command::InsertClip {
                    track: tid,
                    clip: Box::new(piece(c, b, c.end(), 0, ids)),
                });
            } else {
                out.extend(crate::clips::set_window(tid, c, b, c.end() - b));
            }
        }
        if left {
            out.extend(crate::clips::set_window(tid, c, c.start, a - c.start));
        }
    }
    out
}

/// One pass of a take on the timeline, in fractional ticks: the window to show and where
/// the file's (or the notes') content tick zero lies.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Window {
    pub start: f64,
    pub end: f64,
    pub origin: f64,
}

impl Window {
    /// The window cut to the punch range; `None` when nothing of it is left.
    fn punched(self, punch: Option<(Ticks, Ticks)>) -> Option<Window> {
        let (s, e) = match punch {
            Some((a, b)) => (self.start.max(a.0 as f64), self.end.min(b.0 as f64)),
            None => (self.start, self.end),
        };
        (e - s >= 1.0).then_some(Window {
            start: s,
            end: e,
            ..self
        })
    }

    /// The fine offset (`AudioRegion::slip_frames`, source frames at `rate`) that puts
    /// the file's frame 0 exactly at `origin` under a window placed in whole ticks by
    /// [`Self::ticks`]: the whole-tick content origin is `start − offset`, and the file is
    /// moved by the difference, to the frame (ADR 0025 §7).
    pub fn slip_frames(self, start: Ticks, offset: Ticks, bpm: f64, rate: u32) -> i32 {
        let ticks = (start - offset).0 as f64 - self.origin;
        let frames = ticks * 60.0 / (bpm.max(1e-9) * PPQ as f64) * rate as f64;
        frames.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }

    /// [`Self::slip_frames`] through a tempo map (ADR 0060): the source's seconds at the
    /// window's start less what the whole-tick offset says there (`cantus_core::audio_time`:
    /// the offset at the tempo where the clip starts), in frames. One tempo: exactly
    /// [`Self::slip_frames`].
    pub fn slip_frames_in(self, start: Ticks, offset: Ticks, map: &TempoMap, rate: u32) -> i32 {
        if map.is_constant() {
            return self.slip_frames(start, offset, map.bpm_at(0.0), rate);
        }
        let src = map.seconds_between_f64(self.origin, start.0 as f64);
        let said = cantus_core::audio_time::offset_seconds(map, start, offset);
        let frames = (src - said) * rate as f64;
        frames.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    }

    /// [`Self::ticks`] through a tempo map: the offset is the source's seconds at the
    /// window's start, in whole ticks at the tempo there. One tempo: exactly
    /// [`Self::ticks`].
    pub fn ticks_in(self, map: &TempoMap) -> Option<(Ticks, Ticks, Ticks)> {
        if map.is_constant() {
            return self.ticks();
        }
        let mut start = self.start.round() as i64;
        let end = self.end.round() as i64;
        let src = map.seconds_between_f64(self.origin, start as f64);
        let mut offset = cantus_core::audio_time::offset_for(map, Ticks(start), src).0;
        if offset < 0 {
            // The content begins later: the window starts where it does.
            start = self.origin.round() as i64;
            offset = 0;
        }
        (end > start).then_some((Ticks(start), Ticks(end - start), Ticks(offset)))
    }

    /// Whole ticks: the start and end rounded, the content offset from the origin. A
    /// window that would begin before its content begins later instead.
    pub fn ticks(self) -> Option<(Ticks, Ticks, Ticks)> {
        let mut start = self.start.round() as i64;
        let end = self.end.round() as i64;
        let mut offset = (start as f64 - self.origin).round() as i64;
        if offset < 0 {
            start -= offset;
            offset = 0;
        }
        (end > start).then_some((Ticks(start), Ticks(end - start), Ticks(offset)))
    }
}

/// The windows of an audio take, one per loop pass that is kept, oldest first.
///
/// Pass 0 begins where recording was asked to begin; pass *k* ≥ 1 begins at the loop's
/// start, the frame the engine rendered there being `wraps[k-1]`. A frame belongs
/// `latency` frames earlier than where it was rendered (ADR 0025 §7), which
/// `FinishedTake::start_tick` already says for frame 0 and which is applied here to the
/// first frame of every later pass.
pub fn audio_windows(
    take: &FinishedTake,
    plan: &PassPlan,
    map: &TempoMap,
    latency_frames: i64,
) -> Vec<Window> {
    let sr = take.sample_rate.max(1) as f64;
    // Frames after `from` (ticks) as ticks, through the tempo map (ADR 0060: a take plays
    // at its natural speed); one tempo, the old arithmetic.
    let bpm = map.bpm_at(0.0);
    let constant = map.is_constant();
    let after = |from: f64, frames: f64| {
        if constant {
            from + ticks_f(frames / sr, bpm)
        } else {
            map.tick_after_seconds(from, frames / sr)
        }
    };
    let lp = plan.loop_region.filter(|(a, b)| b > a);
    let wraps: &[u64] = if lp.is_some() { &take.wraps } else { &[] };
    let n = wraps.len() + 1;
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let first = if k == 0 { 0 } else { wraps[k - 1] } as f64;
        let next = if k + 1 < n { wraps[k] } else { take.frames } as f64;
        let origin = match (k, lp) {
            (0, _) | (_, None) => take.start_tick,
            (_, Some((a, _))) if constant => {
                a.0 as f64 - ticks_f((first + latency_frames as f64) / sr, bpm)
            }
            (_, Some((a, _))) => after(a.0 as f64, -(first + latency_frames as f64)),
        };
        let start = match (k, lp) {
            (0, _) | (_, None) => plan.record_start.0 as f64,
            (_, Some((a, _))) => a.0 as f64,
        };
        let end = match lp {
            Some((_, b)) if k + 1 < n => b.0 as f64,
            _ => after(origin, next),
        };
        if let Some(w) = (Window { start, end, origin }).punched(plan.punch) {
            out.push(w);
        }
    }
    keep_passes(out, plan)
}

/// Which passes become clips: all, with loop takes; the last, without; a last pass
/// stopped in its first quarter is dropped when there is a full one before it.
fn keep_passes(mut w: Vec<Window>, plan: &PassPlan) -> Vec<Window> {
    if let (Some((a, b)), true) = (plan.loop_region, w.len() >= 2) {
        // A full pass is the loop, or the part of it the punch keeps.
        let (a, b) = match plan.punch {
            Some((pa, pb)) => (a.max(pa), b.min(pb)),
            None => (a, b),
        };
        let last = w[w.len() - 1];
        if last.end - last.start < (b - a).0.max(0) as f64 * 0.25 {
            w.pop();
        }
    }
    if !plan.loop_takes && w.len() > 1 {
        w.drain(..w.len() - 1);
    }
    w
}

/// The pool entry a finished take becomes: a recorded file, named after it.
pub fn source_of(take: &FinishedTake, id: AudioSourceId) -> AudioSource {
    let name = take
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    AudioSource {
        id,
        name,
        path: take.path.to_string_lossy().into_owned(),
        sample_rate: take.sample_rate,
        channels: take.channels,
        frames: take.frames,
        sha256: None,
        recorded: true,
    }
}

/// "Vocal · Take ", the prefix every take of a track is named with.
pub fn take_prefix(track: &Track, word: &str) -> String {
    format!("{} \u{b7} {word} ", track.name)
}

/// The number the next take of a track gets: one past the highest already used.
pub fn next_take_number(track: &Track, prefix: &str) -> u32 {
    track
        .clips
        .iter()
        .filter_map(|c| c.name.strip_prefix(prefix)?.trim().parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

/// Lay a take's clips on its track, oldest first: what the last one covers on the playing
/// lane is stacked on a new take lane (or cut away, in Replace), the earlier passes go to
/// the take lanes after it, and the last plays.
fn lay(
    sim: &mut Sim,
    tid: TrackId,
    mut clips: Vec<Clip>,
    mode: TakeMode,
    ids: Ids,
) -> Result<(), CommandError> {
    let Some(mut last) = clips.pop() else {
        return Ok(());
    };
    let track = sim.track(tid).ok_or(CommandError::TrackNotFound(tid))?;
    let mut lane = max_lane(track).saturating_add(1);
    let replace = mode == TakeMode::Replace;
    let cover = clear_range(
        track,
        last.start,
        last.end(),
        (!replace).then_some(lane),
        ids,
    );
    if !cover.is_empty() && !replace {
        lane = lane.saturating_add(1);
    }
    sim.push(cover)?;
    for mut c in clips {
        c.lane = lane;
        lane = lane.saturating_add(1);
        sim.push(vec![Command::InsertClip {
            track: tid,
            clip: Box::new(c),
        }])?;
    }
    last.lane = 0;
    // Laid over takes — what it covered stacked, the loop's earlier passes, takes already
    // under its stretch — the pass that plays is the comp's (ADR 0058, schema 32): an
    // alternate comp keeps it and a switch takes it away and back. One laid where no take
    // lies is a plain clip that plays under every comp (review of fix4: any take lane
    // anywhere on the track made it a window).
    last.comp = sim.track(tid).is_some_and(|t| {
        t.clips
            .iter()
            .any(|c| c.lane > 0 && c.start < last.end() && c.end() > last.start)
    });
    sim.push(vec![Command::InsertClip {
        track: tid,
        clip: Box::new(last),
    }])
}

/// The audio part of a pass: a source and its clips for every take.
fn audio_takes(
    sim: &mut Sim,
    plan: &PassPlan,
    takes: &[FinishedTake],
    latency_frames: i64,
    ids: Ids,
) -> Result<(), CommandError> {
    let map = sim.project.tempo_map();
    let mode = match plan.mode {
        TakeMode::Overdub => TakeMode::Stack,
        m => m,
    };
    for take in takes {
        let Some(track) = sim.track(take.track) else {
            continue;
        };
        let windows = audio_windows(take, plan, &map, latency_frames);
        if windows.is_empty() {
            continue;
        }
        let prefix = take_prefix(track, &plan.take_word);
        let n0 = next_take_number(track, &prefix);
        let sid = AudioSourceId(ids());
        let mut clips = Vec::with_capacity(windows.len());
        for w in &windows {
            let Some((start, length, offset)) = w.ticks_in(&map) else {
                continue;
            };
            let mut region = AudioRegion::of(sid);
            region.slip_frames = w.slip_frames_in(start, offset, &map, take.sample_rate);
            clips.push(Clip {
                id: ClipId(ids()),
                name: format!("{prefix}{}", n0 + clips.len() as u32),
                start,
                length,
                offset,
                content_length: None,
                muted: false,
                color: None,
                notes: Vec::new(),
                audio: Some(region),
                lane: 0,
                automation: None,
                link: None,
                comp: false,
            });
        }
        if clips.is_empty() {
            continue;
        }
        sim.push(vec![Command::InsertAudioSource {
            source: source_of(take, sid),
        }])?;
        lay(sim, take.track, clips, mode, ids)?;
    }
    Ok(())
}

/// One MIDI pass: its window on the timeline and its notes.
struct MidiPass<'n> {
    start: Ticks,
    end: Ticks,
    notes: Vec<&'n RecordedNote>,
}

/// The notes grouped by loop pass, each with its window (cut to the punch range).
fn midi_passes<'n>(plan: &PassPlan, notes: &'n [RecordedNote]) -> Vec<MidiPass<'n>> {
    let lp = plan.loop_region.filter(|(a, b)| b > a);
    let mut passes: Vec<u32> = notes.iter().map(|n| n.pass).collect();
    passes.sort_unstable();
    passes.dedup();
    let mut out = Vec::new();
    for p in passes {
        let mine: Vec<&RecordedNote> = notes
            .iter()
            .filter(|n| n.pass == p)
            // A note played in a count-in before bar 1 has nowhere to go.
            .filter(|n| n.start >= Ticks::ZERO)
            .filter(|n| plan.punch.is_none_or(|(a, b)| n.start >= a && n.start < b))
            .collect();
        if mine.is_empty() {
            continue;
        }
        let last_end = mine
            .iter()
            .map(|n| n.start + n.length)
            .max()
            .unwrap_or(Ticks::ZERO);
        let mut start = match lp {
            Some((a, _)) if p > 0 => a,
            _ => plan.record_start,
        };
        let mut end = match lp {
            Some((_, b)) if p < plan.last_pass => b,
            _ => plan.stop_at.max(last_end),
        };
        if let Some((a, b)) = plan.punch {
            start = start.max(a);
            end = end.min(b);
        }
        // Notes played in the count-in stay in the clip, before its window: handles.
        if end > start {
            out.push(MidiPass {
                start,
                end,
                notes: mine,
            });
        }
    }
    out
}

fn new_note(n: &RecordedNote, content: Ticks, ids: Ids) -> Note {
    Note {
        id: NoteId(ids()),
        start: content,
        length: n.length.max(Ticks(1)),
        pitch: n.pitch.min(127),
        velocity: n.velocity.clamp(1, 127),
        muted: false,
    }
}

/// A MIDI take's window widened to the bar lines around it (Logic's regions), never past
/// the punch range.
fn bar_window(project: &Project, plan: &PassPlan, start: Ticks, end: Ticks) -> (Ticks, Ticks) {
    // The bar lines through the metre changes (ADR 0060).
    let map = project.tempo_map();
    let mut s = map.bar_start(start).max(Ticks::ZERO);
    let e_bar = map.bar_start(end);
    let mut e = if e_bar == end { end } else { map.bar_end(end) };
    if let Some((a, b)) = plan.punch {
        s = s.max(a);
        e = e.min(b);
    }
    (s, e.max(s + Ticks(1)))
}

/// A new clip holding one pass's notes, its content starting at the earlier of its window
/// and its first note (a note played in the count-in is a handle, not lost).
fn midi_clip(
    project: &Project,
    plan: &PassPlan,
    pass: &MidiPass<'_>,
    name: String,
    ids: Ids,
) -> Clip {
    let (start, end) = bar_window(project, plan, pass.start, pass.end);
    let first = pass.notes.iter().map(|n| n.start).min().unwrap_or(start);
    let origin = start.min(first).max(Ticks::ZERO);
    let mut notes: Vec<Note> = pass
        .notes
        .iter()
        .map(|n| new_note(n, n.start - origin, ids))
        .collect();
    notes.sort_by_key(|n| (n.start, n.pitch));
    Clip {
        id: ClipId(ids()),
        name,
        start,
        length: end - start,
        offset: start - origin,
        content_length: None,
        muted: false,
        color: None,
        notes,
        audio: None,
        lane: 0,
        automation: None,
        link: None,
        comp: false,
    }
}

/// Merge every pass's notes into one clip (Overdub, cycle overdub, quick record): `target`
/// when given, else the note clip on the playing lane under the record start, else a new
/// clip. The clip grows to hold every note; no note already there moves.
#[allow(clippy::too_many_arguments)]
fn overdub(
    sim: &mut Sim,
    plan: &PassPlan,
    tid: TrackId,
    passes: &[MidiPass<'_>],
    target: Option<ClipId>,
    name: String,
    ids: Ids,
) -> Result<(), CommandError> {
    let Some(track) = sim.track(tid) else {
        return Ok(());
    };
    let start = passes
        .iter()
        .map(|p| p.start)
        .min()
        .unwrap_or(plan.record_start);
    let end = passes.iter().map(|p| p.end).max().unwrap_or(start);
    let found = target.and_then(|id| track.clip(id)).cloned().or_else(|| {
        let note_clip = |c: &&Clip| c.lane == 0 && c.audio.is_none() && c.automation.is_none();
        track
            .clips
            .iter()
            .filter(note_clip)
            .find(|c| c.start <= start && c.end() > start)
            .or_else(|| {
                track
                    .clips
                    .iter()
                    .filter(note_clip)
                    .find(|c| c.start < end && c.end() > start)
            })
            .cloned()
    });
    let all: Vec<&RecordedNote> = passes
        .iter()
        .flat_map(|p| p.notes.iter().copied())
        .collect();
    match found {
        Some(clip) => {
            let lo = all.iter().map(|n| n.start).min().unwrap_or(clip.start);
            let hi = all
                .iter()
                .map(|n| n.start + n.length)
                .max()
                .unwrap_or(clip.end());
            let (s, e) = (clip.start.min(lo), clip.end().max(hi));
            let mut cmds = crate::clips::set_window(tid, &clip, s, e - s);
            cmds.push(Command::InsertNotes {
                track: tid,
                clip: clip.id,
                notes: all
                    .iter()
                    .map(|n| new_note(n, clip.content_of(n.start), ids))
                    .collect(),
            });
            // The copies of a linked clip that showed what it showed grow with it, so the
            // overdub is heard in each (ADR 0037).
            let follow = cantus_core::links::follow_window(&sim.project, (tid, clip.id), &cmds);
            cmds.extend(follow);
            sim.push(cmds)
        }
        None => {
            let merged = MidiPass {
                start,
                end,
                notes: all,
            };
            let clip = midi_clip(&sim.project, plan, &merged, name, ids);
            sim.push(vec![Command::InsertClip {
                track: tid,
                clip: Box::new(clip),
            }])
        }
    }
}

/// The MIDI part of a pass, for every armed instrument track (each gets its own copy of
/// the notes: Logic's multitrack MIDI recording) or the selected one (quick record).
fn midi_takes(
    sim: &mut Sim,
    plan: &PassPlan,
    notes: &[RecordedNote],
    ids: Ids,
) -> Result<(), CommandError> {
    let passes = midi_passes(plan, notes);
    if passes.is_empty() {
        return Ok(());
    }
    let merge = plan.mode == TakeMode::Overdub || (!plan.loop_takes && plan.loop_region.is_some());
    let name_for = |sim: &Sim, tid: TrackId, k: u32| {
        sim.track(tid)
            .map(|t| {
                let prefix = take_prefix(t, &plan.take_word);
                format!("{prefix}{}", next_take_number(t, &prefix) + k)
            })
            .unwrap_or_default()
    };
    if let Some((tid, clip)) = plan.quick {
        let name = name_for(sim, tid, 0);
        overdub(sim, plan, tid, &passes, clip, name, ids)?;
    }
    for &tid in &plan.midi_tracks {
        if merge {
            let name = name_for(sim, tid, 0);
            overdub(sim, plan, tid, &passes, None, name, ids)?;
            continue;
        }
        let clips: Vec<Clip> = passes
            .iter()
            .enumerate()
            .map(|(k, p)| {
                let name = name_for(sim, tid, k as u32);
                midi_clip(&sim.project, plan, p, name, ids)
            })
            .collect();
        lay(sim, tid, clips, plan.mode, ids)?;
    }
    Ok(())
}

/// Everything a finished pass does to the project, in order: each audio take's source and
/// clips, then the notes. Run the result as one `Command::Batch` — one undo step.
pub fn build(
    project: &Project,
    plan: &PassPlan,
    takes: &[FinishedTake],
    notes: &[RecordedNote],
    latency_frames: i64,
    ids: Ids,
) -> Result<Vec<Command>, CommandError> {
    let mut sim = Sim::new(project);
    audio_takes(&mut sim, plan, takes, latency_frames, ids)?;
    if !notes.is_empty() {
        midi_takes(&mut sim, plan, notes, ids)?;
    }
    Ok(sim.commands)
}
