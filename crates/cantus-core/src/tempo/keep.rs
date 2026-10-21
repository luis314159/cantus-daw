//! A tempo edit keeps unwarped audio where it is in its file (ADR 0060 §4, reviews 3–4).
//!
//! An unwarped audio window plays its source at natural speed from its content origin
//! (`start − offset`, `crate::audio_time`), so every window edit's tick arithmetic is
//! exact and all the windows of one take agree. Its hidden part before its start is
//! measured through the tempo map, though, and a tempo edit there would change which
//! source frame plays where the take comes in. The command bus therefore follows every
//! command that changes the tempo map with the window edits that keep it — one batch, one
//! undo step, its inverse exact.
//!
//! **By take, not by window** (review 4). The windows that read one line of a file — a
//! take's split pieces, its comp windows, an alternate comp's, glued halves: the same
//! source, the same direction, and the same source second at every tick before the edit —
//! are re-anchored as one: the line keeps the frame where it comes in (a forward line at
//! its earliest window's start; a reversed line, which plays backwards and lands at its
//! end, at its latest window's end) and runs on from there at natural speed through the new
//! map, so a split or a seam never opens a gap or a repeat. The windows that shared their
//! origin and fine offset get one shift (the same offset delta and the same fine offset),
//! so they still share them exactly: a split stays a split, glue still glues. A tempo edit
//! *inside* a take moves its later windows against the bars — natural speed; a warp is the
//! answer — never against each other.
//!
//! A window the command itself moved or changed is the command's. A linked window's fine
//! offset is its group's, so only its offset moves (to within half a tick). A warp that
//! follows the tempo reads content ticks and is left alone; a looping window repeats its
//! period in ticks and is left alone (as moves leave it, `audio_time::relocate`); a window
//! whose file is gone plays nothing and is left alone.

use crate::command::Command;
use crate::model::{AudioSourceId, Clip, ClipId, LinkId, Project, TrackId};
use crate::tempo::{TempoMap, TempoPoint};
use crate::time::Ticks;
use std::collections::HashMap;

/// One unwarped window before the edit.
#[derive(Clone, Debug)]
struct Window {
    track: TrackId,
    /// A clip of the track, or window `.1` of alternate comp `.0`.
    at: Where,
    source: AudioSourceId,
    reverse: bool,
    link: Option<LinkId>,
    start: Ticks,
    end: Ticks,
    offset: Ticks,
    slip: i32,
    rate: f64,
    /// The frame it plays where its line comes in — its start, or its end reversed: the
    /// forward frame under its start either way.
    keep: f64,
}

impl Window {
    /// The content origin and fine offset: windows sharing them shift as one.
    fn origin(&self) -> (Ticks, i32) {
        (self.start - self.offset, self.slip)
    }

    /// Where its line comes in: its start forwards, its end backwards.
    fn entry(&self) -> Ticks {
        if self.reverse {
            self.end
        } else {
            self.start
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Where {
    Clip(ClipId),
    Comp(usize, usize),
}

/// A track's windows re-anchored: each clip, its new offset and fine offset.
type Anchors = Vec<(ClipId, Ticks, i32)>;

/// What a tempo edit has to keep, taken before it runs.
#[derive(Clone, Debug)]
pub struct Kept {
    bpm: f64,
    points: Vec<TempoPoint>,
    windows: Vec<Window>,
    /// Each window's line (see [`line_of`]), and the song's frames to its entry, before the
    /// edit: a take is laid on an unlinked window's exact line (review 6).
    line: Vec<f64>,
    to_entry: Vec<f64>,
    /// The lines: indices into `windows`, one take each.
    lines: Vec<Vec<usize>>,
}

/// Whether `cmd` can change the tempo map (a tempo command, or a batch holding one).
pub fn changes_tempo(cmd: &Command) -> bool {
    match cmd {
        Command::SetTempo { .. }
        | Command::SetTempoTrack { .. }
        | Command::InsertTempoPoint { .. }
        | Command::RemoveTempoPoint { .. }
        | Command::SetTempoPoint { .. } => true,
        Command::Batch { commands, .. } => commands.iter().any(changes_tempo),
        _ => false,
    }
}

/// A window this keeps: unwarped audio (or warped without following the tempo) that does
/// not loop.
fn kept_window(c: &Clip) -> bool {
    c.audio
        .as_ref()
        .is_some_and(|a| !a.warp.as_ref().is_some_and(|w| w.follow))
        && !c.content_length.is_some_and(|p| p < c.length)
}

/// The line a window reads, as one number: the frame at its entry less (forwards) or plus
/// (backwards) the song's frames to its entry — the same for every window of one take.
fn line_of(map: &TempoMap, w: &Window) -> f64 {
    let to_entry = map.seconds_between_f64(0.0, w.entry().0 as f64) * w.rate;
    if w.reverse {
        w.keep + to_entry
    } else {
        w.keep - to_entry
    }
}

/// Take what `cmd` must keep, before it runs; `None` when it cannot change the tempo or
/// the project has no window to keep.
pub fn before(project: &Project, cmd: &Command) -> Option<Kept> {
    if !changes_tempo(cmd) {
        return None;
    }
    let map = project.tempo_map();
    let rates: HashMap<AudioSourceId, f64> = project
        .audio_pool
        .iter()
        .map(|s| (s.id, f64::from(s.sample_rate.max(1))))
        .collect();
    let mut windows = Vec::new();
    let mut add = |track: TrackId, at: Where, c: &Clip| {
        if !kept_window(c) {
            return;
        }
        let a = c.audio.as_ref().expect("an audio window");
        // A window whose file is gone plays nothing (review 4).
        let Some(&rate) = rates.get(&a.source) else {
            return;
        };
        // The forward frame under its start: what it plays there, or at its end reversed.
        let keep = crate::audio_time::offset_seconds(&map, c.start, c.offset) * rate
            + f64::from(a.slip_frames);
        windows.push(Window {
            track,
            at,
            source: a.source,
            reverse: a.reverse,
            link: c.link,
            start: c.start,
            end: c.end(),
            offset: c.offset,
            slip: a.slip_frames,
            rate,
            keep,
        });
    };
    for t in &project.tracks {
        for c in &t.clips {
            add(t.id, Where::Clip(c.id), c);
        }
        for (i, comp) in t.comps.iter().enumerate() {
            for (j, c) in comp.windows.iter().enumerate() {
                add(t.id, Where::Comp(i, j), c);
            }
        }
    }
    if windows.is_empty() {
        return None;
    }
    // One line a take: the same file, the same direction, the same line within a frame.
    let line: Vec<f64> = windows.iter().map(|w| line_of(&map, w)).collect();
    let to_entry: Vec<f64> = windows
        .iter()
        .map(|w| map.seconds_between_f64(0.0, w.entry().0 as f64) * w.rate)
        .collect();
    // A linked window is re-anchored by whole ticks (its fine offset is its group's), so
    // it sits up to half a tick off its take's line: within that of the line it is still
    // the take's (review 5: one edit left it out, and the next tore the take at its edges).
    // The tick is rounded at its content origin, so half a tick there — or at its entry,
    // whichever is longer (review 6: an origin in a slower section lost it again).
    let slack: Vec<f64> = windows.iter().map(|w| linked_slack(&map, w)).collect();
    let mut order: Vec<usize> = (0..windows.len()).collect();
    order.sort_by(|&a, &b| {
        let (x, y) = (&windows[a], &windows[b]);
        (x.source, x.reverse)
            .cmp(&(y.source, y.reverse))
            .then(line[a].total_cmp(&line[b]))
    });
    let mut lines: Vec<Vec<usize>> = Vec::new();
    for i in order {
        let joins = lines.last().and_then(|l| l.last()).is_some_and(|&p| {
            let (x, y) = (&windows[p], &windows[i]);
            x.source == y.source
                && x.reverse == y.reverse
                && (line[i] - line[p]).abs() <= 1.0 + slack[i].max(slack[p])
        });
        match lines.last_mut() {
            Some(l) if joins => l.push(i),
            _ => lines.push(vec![i]),
        }
    }
    Some(Kept {
        bpm: project.tempo_bpm,
        points: project.tempo_points.clone(),
        windows,
        line,
        to_entry,
        lines,
    })
}

/// How far off its take's line a linked window may sit: half a tick at its content origin
/// (where `offset_for` rounds it) or at its entry, the longer, in frames; 0 unlinked.
fn linked_slack(map: &TempoMap, w: &Window) -> f64 {
    if w.link.is_none() {
        return 0.0;
    }
    let half = |t: f64| 0.5 * map.seconds_between_f64(t, t + 1.0) * w.rate;
    let origin = (w.start - w.offset).0.max(0) as f64;
    half(origin).max(half(w.entry().0 as f64))
}

/// The window edits that keep, after the edit ran, every take's frame where it comes in
/// and its windows on one line; empty when the tempo map did not change.
pub fn after(kept: &Kept, project: &Project) -> Vec<Command> {
    if project.tempo_bpm == kept.bpm && project.tempo_points == kept.points {
        return Vec::new();
    }
    let map = project.tempo_map();
    // Every clip found once (review 4: a lookup a window was quadratic in a track's clips).
    let track_index: HashMap<TrackId, usize> = project
        .tracks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id, i))
        .collect();
    let clip_index: HashMap<ClipId, (usize, usize)> = project
        .tracks
        .iter()
        .enumerate()
        .flat_map(|(ti, t)| {
            t.clips
                .iter()
                .enumerate()
                .map(move |(ci, c)| (c.id, (ti, ci)))
        })
        .collect();
    let now = |w: &Window| -> Option<&Clip> {
        let ti = *track_index.get(&w.track)?;
        let t = &project.tracks[ti];
        let c = match w.at {
            Where::Clip(id) => {
                let (i, ci) = *clip_index.get(&id)?;
                if i != ti {
                    return None;
                }
                &t.clips[ci]
            }
            Where::Comp(i, j) => t.comps.get(i)?.windows.get(j)?,
        };
        // A window the command moved or changed is the command's.
        let unchanged = kept_window(c)
            && c.start == w.start
            && c.end() == w.end
            && c.offset == w.offset
            && c.link == w.link
            && c.audio
                .as_ref()
                .is_some_and(|a| a.slip_frames == w.slip && a.reverse == w.reverse);
        unchanged.then_some(c)
    };
    // (window, its new offset, its new fine offset)
    let mut moves: Vec<(usize, Ticks, i32)> = Vec::new();
    for line in &kept.lines {
        let live: Vec<usize> = line
            .iter()
            .copied()
            .filter(|&i| now(&kept.windows[i]).is_some())
            .collect();
        // Where the line comes in: its earliest start, or its latest end backwards.
        let Some(&head) = live.iter().min_by_key(|&&i| {
            let w = &kept.windows[i];
            if w.reverse {
                -w.end.0
            } else {
                w.start.0
            }
        }) else {
            continue;
        };
        let h = &kept.windows[head];
        // The frame the take plays where it comes in: its head's — or, when the head is
        // linked (whole ticks, up to half a tick off), the line of its earliest unlinked
        // window there, so a linked piece never drags the rest of its take (review 6).
        let exact = live
            .iter()
            .copied()
            .filter(|&i| kept.windows[i].link.is_none())
            .min_by_key(|&i| kept.windows[i].start);
        let h_keep = match (h.link, exact) {
            (Some(_), Some(u)) if h.reverse => kept.line[u] - kept.to_entry[head],
            (Some(_), Some(u)) => kept.line[u] + kept.to_entry[head],
            _ => h.keep,
        };
        // The frame the line plays at `tick` through the new map, its entry kept.
        let target = |tick: Ticks| -> f64 {
            let (a, b) = (h.entry().0 as f64, tick.0 as f64);
            if h.reverse {
                h_keep + map.seconds_between_f64(b, a) * h.rate
            } else {
                h_keep + map.seconds_between_f64(a, b) * h.rate
            }
        };
        // The windows that shared their origin and fine offset shift as one (forwards; a
        // reversed window is anchored on its own).
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for &i in &live {
            let w = &kept.windows[i];
            let with = groups.iter_mut().find(|g| {
                let o = &kept.windows[g[0]];
                !w.reverse
                    && !o.reverse
                    && o.origin() == w.origin()
                    && o.link.is_some() == w.link.is_some()
            });
            match with {
                Some(g) => g.push(i),
                None => groups.push(vec![i]),
            }
        }
        for g in groups {
            let s = g
                .iter()
                .copied()
                .min_by_key(|&i| kept.windows[i].start)
                .expect("a group has a window");
            let w = &kept.windows[s];
            // The forward frame its start must read: the line's at its entry (its end,
            // reversed, reads the frame under its start).
            let seconds = (target(w.entry()) - f64::from(w.slip)) / w.rate;
            let (offset, slip) = if w.link.is_some() {
                // The fine offset is the group's: the offset alone, to within half a tick.
                (
                    crate::audio_time::offset_for(&map, w.start, seconds),
                    w.slip,
                )
            } else {
                crate::audio_time::anchor(&map, w.start, seconds, w.slip, w.rate)
            };
            let delta = offset - w.offset;
            if delta == Ticks::ZERO && slip == w.slip {
                continue;
            }
            for i in g {
                moves.push((i, kept.windows[i].offset + delta, slip));
            }
        }
    }
    let mut out = Vec::new();
    // Clips: one re-anchoring of each track's windows (review 5: a command a window, each
    // looking its clip up, was quadratic in a sliced take). Alternate comps' windows: one
    // replacement of the track's comps each.
    let mut anchors: Vec<(TrackId, Anchors)> = Vec::new();
    let mut comps: Vec<(TrackId, Vec<crate::comp::Comp>)> = Vec::new();
    for (i, offset, slip) in moves {
        let w = &kept.windows[i];
        match w.at {
            Where::Clip(id) => {
                let at = match anchors.iter().position(|(t, _)| *t == w.track) {
                    Some(k) => k,
                    None => {
                        anchors.push((w.track, Vec::new()));
                        anchors.len() - 1
                    }
                };
                anchors[at].1.push((id, offset, slip));
            }
            Where::Comp(ci, j) => {
                let at = match comps.iter().position(|(t, _)| *t == w.track) {
                    Some(k) => k,
                    None => {
                        let t = &project.tracks[track_index[&w.track]];
                        comps.push((w.track, t.comps.clone()));
                        comps.len() - 1
                    }
                };
                let win = &mut comps[at].1[ci].windows[j];
                win.offset = offset;
                if let Some(a) = win.audio.as_mut() {
                    a.slip_frames = slip;
                }
            }
        }
    }
    for (track, anchors) in anchors {
        out.push(Command::SetClipAnchors { track, anchors });
    }
    for (track, list) in comps {
        let active = project.track(track).map_or(0, |t| t.active_comp);
        out.push(Command::SetTrackComps {
            track,
            comps: list,
            active,
        });
    }
    out
}
