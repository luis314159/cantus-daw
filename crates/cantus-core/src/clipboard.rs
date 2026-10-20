//! The clipboard (ADR 0036): what cut and copy keep, and the commands a paste runs.
//!
//! Pure data and pure builders, like the rest of this crate: no I/O, no system clipboard,
//! no JSON. The UI turns a [`Clipboard`] into text for the system clipboard (so a copy
//! survives a restart and crosses projects) and decides how each audio file is found in
//! the project it lands in ([`SourcePlan`]); everything that changes the project is
//! built here, as plain [`Command`]s the caller runs as **one** `Command::Batch` — one
//! undo step for a whole paste or cut.
//!
//! Three rules run through it:
//!
//! 1. **A paste is a copy, linked or not** (ADR 0037). A *linked* paste is the same clip
//!    again — it joins the group of the clip it was copied from, keeping the note ids, as
//!    the halves of a split do — when that clip (or a member of its group) is still in
//!    this project with the same content; anything else is a copy of its own, with fresh
//!    clip and note ids ([`copy_of`]), so it can be glued to its original without the
//!    notes being taken for duplicates.
//! 2. **Shape is kept.** Notes keep their spacing from the earliest one; clips keep their
//!    spacing in time *and* in tracks (rows counted from the topmost one).
//! 3. **Nothing points into another project.** Ids are a small counter per project, so the
//!    same number means different things in two projects. An automation clip goes back to
//!    its own track only when it was copied from this very project in this run (the
//!    [`ClipOrigin`]); an audio clip plays a source the UI found or inserted for it.

use crate::{
    AudioSource, AudioSourceId, Clip, ClipId, Command, Note, NoteId, Project, Ticks, TrackId, PPQ,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// The text form's tag and version. A reader refuses any later version politely.
///
/// Version 2 (ADR 0037) is version 1 with a clip's link group (`Clip::link`) in the
/// copied clips: a reader of 2 reads 1 (no link: every clip its own), a reader of 1
/// refuses 2.
pub const FORMAT: &str = "cantus-clip/2";
/// The versions this build reads, oldest first.
pub const READS: [&str; 2] = ["cantus-clip/1", "cantus-clip/2"];
/// What every version of the format starts with.
pub const FORMAT_FAMILY: &str = "cantus-clip/";

/// Fresh ids from the project's monotonic counter.
pub type Ids<'a> = &'a mut dyn FnMut() -> u64;

/// A clip, identified the way the selection identifies it.
pub type ClipRef = (TrackId, ClipId);

/// Which project, in which run of the app, a clipboard was copied from. Two clipboards
/// from the same origin may refer to the same tracks, effects and audio sources by id.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipOrigin {
    /// A number drawn once per run of the app.
    pub session: u64,
    /// Bumped whenever the app replaces its project (New, Open, a recovery).
    pub epoch: u64,
}

/// What a cut or copy keeps.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clipboard {
    /// [`FORMAT`]. First, so a reader can tell the text is ours from its first bytes.
    pub format: String,
    /// The project schema the clips and notes below are written in
    /// (`cantus_project::SCHEMA_VERSION`, which this crate cannot see: whoever writes the
    /// text sets it). The payload is the live model, so a reader refuses a newer schema
    /// instead of silently dropping fields it does not know. 0: not said.
    #[serde(default)]
    pub schema: u32,
    pub origin: ClipOrigin,
    /// Ticks per quarter note of the copy ([`PPQ`]).
    pub ppq: i64,
    /// The source project's tempo: an audio clip pasted at another tempo is resized so it
    /// plays the same part of its file.
    pub tempo_bpm: f64,
    /// The source project's tempo track (ADR 0060), when it had one: with it and [`Self::at`]
    /// an audio clip is measured in seconds where it was copied and laid out again through
    /// the map where it lands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tempo_points: Vec<crate::tempo::TempoPoint>,
    /// Where the copy's origin was on the source's timeline (the starts in the payload are
    /// relative to it).
    #[serde(default, skip_serializing_if = "Ticks::is_zero")]
    pub at: Ticks,
    /// The audio clips' files, with **absolute** paths, so another project can find them.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<AudioSource>,
    pub payload: Payload,
}

/// Notes or clips.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Payload {
    /// Notes in content ticks, relative to the copy's origin (the earliest selected note,
    /// or the clip's left edge when the whole clip was copied).
    Notes { notes: Vec<Note> },
    /// Clips with their starts relative to the earliest (or to the time selection's start)
    /// and their rows relative to the topmost track.
    Clips {
        rows: Vec<ClipRow>,
        /// `Some(to - from)` when the copy was a time selection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        span: Option<Ticks>,
    },
}

/// One copied clip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipRow {
    /// Tracks below the topmost copied clip's track.
    pub row: usize,
    /// The track it was copied from: where an automation clip goes back to.
    pub track: TrackId,
    /// The clip, with `start` relative to the copy's origin.
    pub clip: Clip,
}

/// How a pasted audio clip finds its file in the project it lands in. The UI decides
/// (it can look at the files); this crate only carries out the decision.
#[derive(Clone, Debug, PartialEq)]
pub enum SourcePlan {
    /// The project already has this file.
    Reuse(AudioSourceId),
    /// Add this source (already given a fresh id) before the clips that play it.
    Insert(AudioSource),
}

/// What a clip paste produced.
#[derive(Debug, Default)]
pub struct Pasted {
    pub commands: Vec<Command>,
    /// The new clips, to select.
    pub made: Vec<ClipRef>,
    /// Automation clips left out: what they automate belongs to another project.
    pub skipped_automation: usize,
    /// Audio clips left out: no plan said where their file is.
    pub skipped_audio: usize,
}

impl Clipboard {
    /// A clipboard of `payload`, copied from `project` (no audio sources yet: the UI adds
    /// them with absolute paths).
    pub fn new(origin: ClipOrigin, project: &Project, payload: Payload) -> Self {
        Clipboard {
            format: FORMAT.to_string(),
            schema: 0,
            origin,
            ppq: PPQ,
            tempo_bpm: project.tempo_bpm,
            tempo_points: project.tempo_points.clone(),
            at: Ticks::ZERO,
            sources: Vec::new(),
            payload,
        }
    }

    /// This clipboard, its payload's starts relative to `origin` on the source's timeline.
    pub fn at(mut self, origin: Ticks) -> Self {
        self.at = origin;
        self
    }

    /// The source project's tempo map (its tempo and tempo track; the metre does not
    /// matter to time).
    pub fn tempo_map(&self) -> crate::tempo::TempoMap {
        crate::tempo::TempoMap::new(
            self.tempo_bpm.max(1e-9),
            crate::time::TimeSignature::default(),
            &self.tempo_points,
            &[],
        )
    }

    /// How many notes or clips it holds.
    pub fn len(&self) -> usize {
        match &self.payload {
            Payload::Notes { notes } => notes.len(),
            Payload::Clips { rows, .. } => rows.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The audio sources the copied clips play, each once, in order.
    pub fn audio_source_ids(&self) -> Vec<AudioSourceId> {
        let mut out = Vec::new();
        if let Payload::Clips { rows, .. } = &self.payload {
            for r in rows {
                if let Some(a) = &r.clip.audio {
                    if !out.contains(&a.source) {
                        out.push(a.source);
                    }
                }
            }
        }
        out
    }
}

/// Notes relative to `origin` (a content tick): the earliest selected note's start, or
/// the clip's left edge when the whole clip is copied (so a pattern that starts on its
/// second beat still does after a paste).
pub fn copy_notes(notes: &[Note], origin: Ticks) -> Payload {
    let mut notes: Vec<Note> = notes
        .iter()
        .map(|n| Note {
            start: n.start - origin,
            ..n.clone()
        })
        .collect();
    notes.sort_by_key(|n| (n.start, n.pitch, n.id.0));
    Payload::Notes { notes }
}

/// Where a copy of the clips `sel` starts on the timeline (the earliest clip's start): what
/// [`copy_clips`]'s starts are relative to, for [`Clipboard::at`].
pub fn copy_origin(project: &Project, sel: &[ClipRef]) -> Ticks {
    sel.iter()
        .filter_map(|(t, c)| project.track(*t)?.clip(*c).map(|c| c.start))
        .min()
        .unwrap_or(Ticks::ZERO)
}

/// The clips of `sel`, keeping their spacing in time and across tracks: rows from the
/// topmost track, starts from the earliest clip. Rows come out sorted top to bottom, then
/// by time, so a paste is the same whatever order the clips were selected in.
pub fn copy_clips(project: &Project, sel: &[ClipRef]) -> Payload {
    let picked: Vec<(usize, TrackId, Clip)> = sel
        .iter()
        .filter_map(|(t, c)| {
            let index = project.track_index(*t)?;
            let clip = project.tracks[index].clip(*c)?;
            Some((index, *t, clip.clone()))
        })
        .collect();
    let earliest = picked
        .iter()
        .map(|(_, _, c)| c.start)
        .min()
        .unwrap_or(Ticks::ZERO);
    clips_payload(picked, earliest, None)
}

/// A clip payload from `(track index, track, clip)` triples with timeline starts: starts
/// become relative to `origin`, rows relative to the topmost. The time-selection copy
/// (which crops its clips first) comes through here too.
pub fn clips_payload(
    picked: Vec<(usize, TrackId, Clip)>,
    origin: Ticks,
    span: Option<Ticks>,
) -> Payload {
    let top = picked.iter().map(|(i, _, _)| *i).min().unwrap_or(0);
    let mut rows: Vec<ClipRow> = picked
        .into_iter()
        .map(|(index, track, mut clip)| {
            clip.start -= origin;
            ClipRow {
                row: index - top,
                track,
                clip,
            }
        })
        .collect();
    rows.sort_by_key(|r| (r.row, r.clip.start, r.clip.id.0));
    rows.dedup_by_key(|r| (r.track, r.clip.id));
    Payload::Clips { rows, span }
}

/// Remove every clip of `sel` that exists.
pub fn cut_clips(project: &Project, sel: &[ClipRef]) -> Vec<Command> {
    let mut seen: Vec<ClipRef> = Vec::with_capacity(sel.len());
    for r in sel {
        let exists = project.track(r.0).and_then(|t| t.clip(r.1)).is_some();
        if exists && !seen.contains(r) {
            seen.push(*r);
        }
    }
    seen.into_iter()
        .map(|(track, clip)| Command::RemoveClip { track, clip })
        .collect()
}

/// Paste `notes` (relative content ticks) into `clip` at content tick `at`, with fresh
/// ids and their mute kept. A paste that runs past what the clip shows makes it show it:
///
/// - a clip that plays once grows its window;
/// - a looping clip grows its loop by **whole loops** (4 beats becomes 8, never 5, so the
///   repeats stay on the bar). The new room is filled with copies of the loop's notes, so
///   every repeat that was heard is still heard where it was. The window grows to show the
///   new loop at least once, or the paste would sit where the window never reaches.
///
/// Notes already stored past the old loop's end (hidden by it) become audible in the
/// grown loop, as they would if the loop were lengthened by hand.
pub fn paste_notes(
    track: TrackId,
    clip: &Clip,
    notes: &[Note],
    at: Ticks,
    ids: Ids,
) -> (Vec<Command>, Vec<NoteId>) {
    if notes.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let mut pasted: Vec<Note> = notes
        .iter()
        .map(|n| Note {
            id: NoteId(ids()),
            start: n.start + at,
            ..n.clone()
        })
        .collect();
    let made = pasted.iter().map(|n| n.id).collect();
    let end = pasted.iter().map(Note::end).max().unwrap_or(at);
    let mut commands = Vec::with_capacity(3);
    match clip.period() {
        Some(period) if clip.offset + period < end => {
            let need = (end - clip.offset).0;
            let loops = (need + period.0 - 1) / period.0;
            let grown = Ticks(loops * period.0);
            commands.push(Command::SetClipLoop {
                track,
                clip: clip.id,
                content_length: Some(grown),
            });
            if clip.length < grown {
                commands.push(Command::SetClipBounds {
                    track,
                    clip: clip.id,
                    start: clip.start,
                    length: grown,
                });
            }
            let brace = clip.offset..clip.offset + period;
            let looped: Vec<&Note> = clip
                .notes
                .iter()
                .filter(|n| brace.contains(&n.start))
                .collect();
            for k in 1..loops {
                for n in &looped {
                    pasted.push(Note {
                        id: NoteId(ids()),
                        start: n.start + Ticks(k * period.0),
                        ..(*n).clone()
                    });
                }
            }
        }
        Some(_) => {}
        None if clip.content_end() < end => commands.push(Command::SetClipBounds {
            track,
            clip: clip.id,
            start: clip.start,
            length: end - clip.offset,
        }),
        None => {}
    }
    commands.push(Command::InsertNotes {
        track,
        clip: clip.id,
        notes: pasted,
    });
    (commands, made)
}

/// A new note clip at `at` holding `notes` (fresh ids), as long as they are rounded up to
/// whole bars, and never shorter than one.
pub fn notes_clip(name: &str, notes: &[Note], at: Ticks, bar: Ticks, ids: Ids) -> Clip {
    let bar = bar.max(Ticks(1));
    let end = notes.iter().map(Note::end).max().unwrap_or(Ticks::ZERO);
    let bars = ((end.0 + bar.0 - 1) / bar.0).max(1);
    Clip {
        id: ClipId(ids()),
        name: name.to_string(),
        start: at.max(Ticks::ZERO),
        length: Ticks(bars * bar.0),
        offset: Ticks::ZERO,
        content_length: None,
        muted: false,
        color: None,
        notes: notes
            .iter()
            .map(|n| Note {
                id: NoteId(ids()),
                ..n.clone()
            })
            .collect(),
        audio: None,
        lane: 0,
        automation: None,
        link: None,
        comp: false,
    }
}

/// Paste the clips of `board` at `at`, the topmost on row `target_row`.
///
/// - Rows keep their distances. A block that would run past the last track moves up as a
///   whole; only a block taller than the project folds its extra rows onto the last track.
/// - Every clip lands on the playing lane. With `linked`, a clip copied from this project
///   in this run whose original (or a member of its original's group) is still here with
///   the same content is pasted as a **linked** copy, in that group (ADR 0037); every
///   other clip is a [`copy_of`] (fresh ids).
/// - An automation clip goes back to the track it came from, its target unchanged, when
///   `same_origin` says the ids mean the same things here and that track still exists;
///   otherwise it is left out and counted.
/// - An audio clip plays the source its [`SourcePlan`] names; an inserted source's
///   command comes before the first clip that plays it. With no plan, a clip from this
///   same project keeps a source that is still in the pool, and any other is left out.
///   From another project at another tempo, its window and fades are resized so it plays
///   the same seconds of its file (its start stays on the beat it had).
#[allow(clippy::too_many_arguments)]
pub fn paste_clips(
    project: &Project,
    board: &Clipboard,
    same_origin: bool,
    target_row: usize,
    at: Ticks,
    plans: &HashMap<AudioSourceId, SourcePlan>,
    linked: bool,
    ids: Ids,
) -> Pasted {
    let mut out = Pasted::default();
    let mut linker = crate::links::Linker::new();
    let Payload::Clips { rows, .. } = &board.payload else {
        return out;
    };
    if project.tracks.is_empty() {
        return out;
    }
    let last = project.tracks.len() - 1;
    let tallest = rows
        .iter()
        .filter(|r| r.clip.automation.is_none())
        .map(|r| r.row)
        .max()
        .unwrap_or(0);
    let top = target_row.min(last.saturating_sub(tallest));
    let at = at.max(Ticks::ZERO);
    // Through the tempo maps when either changes along its song (ADR 0060): each audio
    // clip measured in seconds where it was copied and laid out where it lands.
    let (src_map, dst_map) = (board.tempo_map(), project.tempo_map());
    let mapped = !(src_map.is_constant() && dst_map.is_constant());
    // Every clip lands at its own distance from the paste point (its bars kept); the
    // windows of one take pasted together keep their shape (reviews 3–4,
    // `audio_time::lay_chains`), so windows that met still meet.
    let laid: Vec<Option<crate::audio_time::Laid>> = if mapped {
        let rate_of = |a: &crate::audio::AudioRegion| {
            project
                .audio_source(a.source)
                .or_else(|| board.sources.iter().find(|s| s.id == a.source))
                .map_or(48_000.0, |s| f64::from(s.sample_rate.max(1)))
        };
        let members: Vec<crate::audio_time::Member> = rows
            .iter()
            .filter_map(|r| {
                let a = r.clip.audio.as_ref()?;
                Some(crate::audio_time::Member {
                    clip: &r.clip,
                    row: r.row as u64,
                    from: board.at + r.clip.start,
                    rate: rate_of(a),
                })
            })
            .collect();
        let mut laid_audio =
            crate::audio_time::lay_chains(&src_map, &dst_map, at - board.at, &members).into_iter();
        rows.iter()
            .map(|r| {
                r.clip
                    .audio
                    .as_ref()
                    .and_then(|_| laid_audio.next().flatten())
            })
            .collect()
    } else {
        vec![None; rows.len()]
    };
    let scale = if !mapped
        && !same_origin
        && board.tempo_bpm > 0.0
        && (board.tempo_bpm - project.tempo_bpm).abs() > 1e-9
    {
        Some(project.tempo_bpm / board.tempo_bpm)
    } else {
        None
    };
    let mut inserted: Vec<AudioSourceId> = Vec::new();
    for (r, laid) in rows.iter().zip(laid) {
        let track = if r.clip.automation.is_some() {
            if same_origin && project.track(r.track).is_some() {
                r.track
            } else {
                out.skipped_automation += 1;
                continue;
            }
        } else {
            project.tracks[(top + r.row).min(last)].id
        };
        let mut plays = None;
        // The source's rate, for the fine offset a move through the tempo map leaves.
        let mut rate = 48_000.0;
        if let Some(a) = &r.clip.audio {
            let dst = match plans.get(&a.source) {
                Some(SourcePlan::Reuse(id)) => *id,
                Some(SourcePlan::Insert(s)) => {
                    if !inserted.contains(&s.id) {
                        inserted.push(s.id);
                        out.commands
                            .push(Command::InsertAudioSource { source: s.clone() });
                    }
                    rate = s.sample_rate as f64;
                    s.id
                }
                None if same_origin && project.audio_source(a.source).is_some() => a.source,
                None => {
                    out.skipped_audio += 1;
                    continue;
                }
            };
            if let Some(src) = project.audio_source(dst) {
                rate = src.sample_rate as f64;
            }
            plays = Some(dst);
        }
        let start = laid.map_or(at + r.clip.start, |l| l.start);
        let mut copy = Clip {
            id: ClipId(ids()),
            start,
            lane: 0,
            link: None,
            // A paste is no comp window, linked or not, wherever it lands — over its takes
            // too (ADR 0058, schema 32).
            comp: false,
            ..r.clip.clone()
        };
        if let (Some(a), Some(dst)) = (copy.audio.as_mut(), plays) {
            a.source = dst;
            // A warp that follows the tempo counts in content ticks (ADR 0061): the paste
            // keeps them as they are, whatever the tempo where it lands (`relocate` below
            // leaves it too).
            let follows = a.warp.as_ref().is_some_and(|w| w.follow);
            if let Some(k) = scale.filter(|_| !follows) {
                let s = |t: Ticks| Ticks((t.0 as f64 * k).round() as i64);
                a.fade_in = s(a.fade_in);
                a.fade_out = s(a.fade_out);
                copy.offset = s(copy.offset);
                copy.length = s(copy.length).max(Ticks(1));
            }
        }
        // Where it lands in its file: (fades in, out, fine offset) a free copy takes.
        let mut region = None;
        if let (Some(l), true) = (laid, plays.is_some()) {
            copy.offset = l.offset;
            copy.length = l.length;
            region = Some((l.fade_in, l.fade_out, l.slip_frames));
        } else if plays.is_some() && scale.is_none() && mapped {
            // Only where the tempo under the clip differs (the same tempo function
            // over both spans keeps the ticks exactly): the same seconds of its file
            // (`audio_time::relocate`, which a move and a drag-copy share).
            let from = board.at + r.clip.start;
            if let Some(m) =
                crate::audio_time::relocate(&src_map, from, &copy, &dst_map, copy.start, rate)
            {
                copy.offset = m.offset;
                copy.length = m.length;
                region = Some((m.fade_in, m.fade_out, m.slip_frames));
            }
        }
        let group = if linked && same_origin {
            link_target(project, r, &copy)
        } else {
            None
        };
        // A paste that joins a group takes the group's fades and fine offset as they are
        // (they are shared): its window alone (review 3). Every other paste plays the
        // seconds it played (review 4: a cut and paste with *Link copies* on, which finds
        // no group, kept the old fades and dropped the fine offset).
        if let (None, Some((fade_in, fade_out, slip))) = (&group, region) {
            let a = copy.audio.as_mut().expect("an audio clip");
            a.fade_in = fade_in;
            a.fade_out = fade_out;
            a.slip_frames = slip;
        }
        match group {
            Some(Target::Clip(t, c, l)) => {
                let (pre, link) = linker.group_of(t, c, l, ids);
                out.commands.extend(pre);
                copy.link = Some(link);
            }
            Some(Target::Group(l)) => copy.link = Some(l),
            None => {
                for n in &mut copy.notes {
                    n.id = NoteId(ids());
                }
                // A pasted clip is no comp window: the seam crossfade a window carried is
                // not the paste's (integration review: a paste right after another clip
                // crossfaded with it). A linked paste keeps it: it is a shared field.
                if let Some(a) = copy.audio.as_mut() {
                    a.xfade = Ticks::ZERO;
                }
            }
        }
        out.made.push((track, copy.id));
        out.commands.push(Command::InsertClip {
            track,
            clip: Box::new(copy),
        });
    }
    out
}

/// What a linked paste joins.
enum Target {
    /// The clip it was copied from, still here (its group, or a new one for it).
    Clip(TrackId, ClipId, Option<crate::LinkId>),
    /// Another member of the group the copied clip was in.
    Group(crate::LinkId),
}

/// The group a linked paste of row `r` (made into `copy`) can join: the clip it was copied
/// from when that is still in the project with the same content, else a member of the
/// copied clip's group with the same content. `None`: nothing to be the same clip as.
fn link_target(project: &Project, r: &ClipRow, copy: &Clip) -> Option<Target> {
    let same = |c: &Clip| crate::links::same_content(c, copy);
    if let Some(src) = project
        .tracks
        .iter()
        .find_map(|t| t.clip(r.clip.id).map(|c| (t.id, c)))
    {
        if same(src.1) {
            return Some(Target::Clip(src.0, src.1.id, src.1.link));
        }
    }
    let l = r.clip.link?;
    crate::links::first_member(project, l)
        .filter(|c| same(c))
        .map(|_| Target::Group(l))
}

// ---- split and the time-range crop ---------------------------------------------------

/// Split a clip at a timeline tick. `None` when the tick is not strictly inside it.
///
/// No note moves and no note is copied into one half or the other: both halves carry the
/// whole note list and differ only in which part of it their window shows. That is why
/// the arrangement's glue can put them back together exactly. (The arrangement's
/// *Split* is this function; it lives here because the time-range crop is built on it.)
///
/// An unwarped audio clip's halves play what the whole played through `project`'s tempo
/// map (review 2): see [`split_audio`] (a reversed clip's need the map).
pub fn split(
    project: &Project,
    track: TrackId,
    clip: &Clip,
    at: Ticks,
    ids: Ids,
) -> Option<Vec<Command>> {
    if at <= clip.start || at >= clip.end() {
        return None;
    }
    let mut right = Clip {
        id: ClipId(ids()),
        name: clip.name.clone(),
        start: at,
        length: clip.end() - at,
        offset: clip.content_of(at),
        content_length: clip.content_length,
        muted: clip.muted,
        color: clip.color,
        notes: clip.notes.clone(),
        audio: clip.audio.clone(),
        lane: clip.lane,
        automation: clip.automation.clone(),
        link: clip.link,
        // Both halves of a comp window are the comp's (ADR 0058, schema 32).
        comp: clip.comp,
    };
    let audio = split_audio(project, track, clip, at, &mut right);
    // A linked clip's halves stay in its group — they show the same content through two
    // windows — unless the split changes that content (an audio clip's fades, which stay
    // at the outer edges): then both leave it, or the cut would reach every sibling.
    let mut out = Vec::with_capacity(4);
    let refaded = audio
        .iter()
        .any(|c| matches!(c, Command::SetClipAudio { .. }));
    if clip.link.is_some() && (refaded || !crate::links::same_content(&right, clip)) {
        right.link = None;
        out.push(Command::SetClipLink {
            track,
            clip: clip.id,
            link: None,
        });
    }
    out.extend([
        Command::SetClipBounds {
            track,
            clip: clip.id,
            start: clip.start,
            length: at - clip.start,
        },
        Command::InsertClip {
            track,
            clip: Box::new(right),
        },
    ]);
    out.extend(audio);
    Some(out)
}

/// Fix up a split of an audio clip at `at` (`right` is the new right half, built by
/// [`split`] as for notes). The left half keeps the fade in and loses the fade out, the
/// right half the opposite — the cut is a hard edge, as in every DAW. A reversed clip
/// maps its window backwards, so its halves' offsets swap. Returns the extra commands for
/// the left half.
///
/// Forward, both halves keep the content origin (`start − offset`): the tick arithmetic
/// is exact through any tempo map (ADR 0060 §5). A reversed clip reads backwards from its
/// end, so under a tempo track its halves re-derive their offsets in seconds, the rounding
/// in their fine offsets (`audio_time::reanchor_reversed`); inside one held tempo, the
/// tick arithmetic exactly. A clip warped to follow the tempo reads content ticks: the
/// tick arithmetic is its own.
pub fn split_audio(
    project: &Project,
    track: TrackId,
    clip: &Clip,
    at: Ticks,
    right: &mut Clip,
) -> Vec<Command> {
    let Some(a) = clip.audio.as_ref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let follows = a.warp.as_ref().is_some_and(|w| w.follow);
    let rate = project
        .audio_source(a.source)
        .map_or(48_000.0, |s| s.sample_rate as f64);
    let map = (!follows && !project.tempo_points.is_empty()).then(|| project.tempo_map());
    let mut left_slip = a.slip_frames;
    if let Some(r) = right.audio.as_mut() {
        r.fade_in = Ticks::ZERO;
        r.fade_out = a.fade_out.min(right.length);
        // The cut is no comp seam: the window's own crossfade stays on the left half, at
        // the seam it was set for (integration review).
        r.xfade = Ticks::ZERO;
    }
    let left_len = at - clip.start;
    if a.reverse {
        // Left plays the end of the window, right its beginning.
        let (end, slip) = (clip.end(), a.slip_frames);
        let (left_offset, right_at) = match &map {
            Some(m) => {
                use crate::audio_time::reanchor_reversed as re;
                let l = re(m, clip.start, end, clip.offset, slip, rate, clip.start, at);
                (l, re(m, clip.start, end, clip.offset, slip, rate, at, end))
            }
            None => ((clip.offset + (end - at), slip), (clip.offset, slip)),
        };
        right.offset = right_at.0;
        if let Some(r) = right.audio.as_mut() {
            r.slip_frames = right_at.1;
        }
        left_slip = left_offset.1;
        out.push(Command::SetClipOffset {
            track,
            clip: clip.id,
            offset: left_offset.0,
        });
    }
    let left = crate::AudioRegion {
        fade_out: Ticks::ZERO,
        fade_in: a.fade_in.min(left_len),
        slip_frames: left_slip,
        ..a.clone()
    };
    if left != *a {
        out.push(Command::SetClipAudio {
            track,
            clip: clip.id,
            audio: Some(left),
        });
    }
    out
}

/// One clip under a time range: the pieces of it inside the range, and the commands that
/// take them out of the project and leave the rest playing exactly what it played.
#[derive(Clone, Debug)]
pub struct Crop {
    /// The track's index, for the copy's rows.
    pub index: usize,
    pub track: TrackId,
    /// What was inside the range, as clips with timeline starts. One, except a looping
    /// automation clip cut where no repeat begins: see [`loop_pieces`].
    pub pieces: Vec<Clip>,
    /// What a cut runs, in order, as part of one `Command::Batch`.
    pub cut: Vec<Command>,
}

/// Every playing-lane clip on every track that overlaps `[from, to)`. Take lanes are left
/// alone (they are not heard, and the comp keeps its alternatives).
pub fn time_crops(project: &Project, from: Ticks, to: Ticks, ids: Ids) -> Vec<Crop> {
    let mut out = Vec::new();
    if to <= from {
        return out;
    }
    for (index, t) in project.tracks.iter().enumerate() {
        for c in t.clips.iter().filter(|c| c.lane == 0) {
            if c.start >= to || c.end() <= from || c.length <= Ticks::ZERO {
                continue;
            }
            if let Some((pieces, cut)) = crop(project, t.id, c, from, to, ids) {
                out.push(Crop {
                    index,
                    track: t.id,
                    pieces,
                    cut,
                });
            }
        }
    }
    out
}

/// The pieces of `clip` inside `[from, to)` and the commands that cut them out.
fn crop(
    project: &Project,
    track: TrackId,
    clip: &Clip,
    from: Ticks,
    to: Ticks,
    ids: Ids,
) -> Option<(Vec<Clip>, Vec<Command>)> {
    if clip.start >= from && clip.end() <= to {
        let remove = Command::RemoveClip {
            track,
            clip: clip.id,
        };
        return Some((vec![clip.clone()], vec![remove]));
    }
    // A loop brace shorter than the window: a split would set the right piece's offset
    // past the brace, where nothing plays (audio clips never loop, ADR 0025).
    let loops = clip.period().is_some_and(|p| p < clip.length);
    if loops && clip.audio.is_none() {
        return Some(crop_looping(track, clip, from, to, ids));
    }
    crop_by_split(project, track, clip, from, to, ids)
}

/// Crop with [`split`], run on a scratch copy of the clip so the second cut sees the
/// first: fades stay at the outer edges and a reversed audio clip's halves swap their
/// windows, exactly as a split at the playhead does.
fn crop_by_split(
    project: &Project,
    track: TrackId,
    clip: &Clip,
    from: Ticks,
    to: Ticks,
    ids: Ids,
) -> Option<(Vec<Clip>, Vec<Command>)> {
    let mut scratch = Project::empty("");
    scratch.audio_pool = project.audio_pool.clone();
    let mut t = scratch.make_track("");
    t.id = track;
    t.clips = vec![clip.clone()];
    scratch.tracks.push(t);
    let mut bus = crate::CommandBus::new(scratch);
    let mut commands = Vec::new();
    let mut middle = clip.id;
    let mut cut = |bus: &mut crate::CommandBus, c: Vec<Command>| -> Option<()> {
        bus.execute(
            Command::Batch {
                label_key: String::new(),
                commands: c.clone(),
            },
            crate::Origin::System,
        )
        .ok()?;
        commands.extend(c);
        Some(())
    };
    if from > clip.start {
        let c = split(project, track, clip, from, ids)?;
        middle = c.iter().find_map(|c| match c {
            Command::InsertClip { clip, .. } => Some(clip.id),
            _ => None,
        })?;
        cut(&mut bus, c)?;
    }
    let current = bus.project().track(track)?.clip(middle)?.clone();
    if to < current.end() {
        let c = split(project, track, &current, to, ids)?;
        cut(&mut bus, c)?;
    }
    let piece = bus.project().track(track)?.clip(middle)?.clone();
    commands.push(Command::RemoveClip {
        track,
        clip: middle,
    });
    Some((vec![piece], commands))
}

/// A looping clip: the piece before the range keeps the clip (shortened); the pieces
/// inside and after it are new clips playing exactly what was heard there.
fn crop_looping(
    track: TrackId,
    clip: &Clip,
    from: Ticks,
    to: Ticks,
    ids: Ids,
) -> (Vec<Clip>, Vec<Command>) {
    let middle = loop_pieces(clip, from.max(clip.start), to.min(clip.end()), ids);
    let mut cut = Vec::with_capacity(3);
    if clip.start < from {
        cut.push(Command::SetClipBounds {
            track,
            clip: clip.id,
            start: clip.start,
            length: from - clip.start,
        });
    } else {
        cut.push(Command::RemoveClip {
            track,
            clip: clip.id,
        });
    }
    if to < clip.end() {
        for piece in loop_pieces(clip, to, clip.end(), ids) {
            cut.push(Command::InsertClip {
                track,
                clip: Box::new(piece),
            });
        }
    }
    (middle, cut)
}

/// What a looping clip plays over `[a, b)`, as clips of their own (fresh ids).
///
/// The model has no loop *phase*: a looping clip always begins its window at the start of
/// its brace. So when `a` falls on a repeat the piece is still the same loop. Otherwise:
///
/// - a **note** clip's piece is the notes heard there, written out once (one clip);
/// - an **automation** clip's curve cannot be written out exactly (a shaped segment cut
///   at a repeat's seam has no exact shorter twin), so the piece is two clips: the rest of
///   the repeat `a` falls in, played once from the brace tick heard at `a`, then — if the
///   range runs past that repeat — the loop again, from its next repeat. Each reads only
///   inside the brace, so both play exactly what the loop played.
pub fn loop_pieces(clip: &Clip, a: Ticks, b: Ticks, ids: Ids) -> Vec<Clip> {
    let period = clip.period().unwrap_or(clip.length).max(Ticks(1));
    let piece = |id: u64, start: Ticks, end: Ticks| Clip {
        id: ClipId(id),
        start,
        length: end - start,
        ..clip.clone()
    };
    let phase = Ticks((a - clip.start).0.rem_euclid(period.0));
    if phase == Ticks::ZERO {
        return vec![piece(ids(), a, b)];
    }
    if clip.automation.is_none() {
        let mut out = piece(ids(), a, b);
        let mut at = Vec::new();
        let mut notes = Vec::new();
        for n in &clip.notes {
            clip.timeline_positions(n.start, &mut at);
            for p in at.iter().filter(|p| **p >= a && **p < b) {
                notes.push(Note {
                    id: NoteId(ids()),
                    start: *p - a,
                    ..n.clone()
                });
            }
        }
        notes.sort_by_key(|n| (n.start, n.pitch));
        out.offset = Ticks::ZERO;
        out.content_length = None;
        out.notes = notes;
        // Other notes than the loop's: a clip of its own (ADR 0037).
        out.link = None;
        return vec![out];
    }
    let seam = a + (period - phase);
    let mut head = piece(ids(), a, seam.min(b));
    head.offset = clip.offset + phase;
    head.content_length = None;
    let mut out = vec![head];
    if seam < b {
        out.push(piece(ids(), seam, b));
    }
    out
}

/// An independent copy of a clip: a fresh clip id **and** fresh note ids.
///
/// The new note ids are the point. Split deliberately shares them so the halves can be
/// glued back together; a copy must not, or gluing a clip to its own duplicate would see
/// every note as one it already had and throw the lot away.
pub fn copy_of(clip: &Clip, start: Ticks, ids: Ids) -> Clip {
    Clip {
        // A copy is no comp window: no seam crossfade (integration review).
        audio: clip.audio.clone().map(|a| crate::AudioRegion {
            xfade: Ticks::ZERO,
            ..a
        }),
        lane: clip.lane,
        automation: clip.automation.clone(),
        // A copy of its own is in no group (ADR 0037), and no comp window (ADR 0058).
        link: None,
        comp: false,
        id: ClipId(ids()),
        name: clip.name.clone(),
        start,
        length: clip.length,
        offset: clip.offset,
        content_length: clip.content_length,
        muted: clip.muted,
        color: clip.color,
        notes: clip
            .notes
            .iter()
            .map(|n| Note {
                id: NoteId(ids()),
                ..n.clone()
            })
            .collect(),
    }
}
