//! Take lanes and comping (ADR 0025 §5), as data: swipe a range of a take into the
//! playing lane, promote a whole take, delete the takes nothing uses, explode the takes to
//! tracks of their own.
//!
//! **A comp is lane-0 clips that are windows on the takes.** Nothing is copied: a window
//! shows the same file (audio) or the same notes, note ids and all (MIDI, the way a split
//! shares them), through another range. So a lane-0 clip is *from* a take when it shows
//! the same content at the same place — the same [`ContentKey`] — and that is how the take
//! lanes know which of their ranges are in the comp, to light them.
//!
//! Every operation is a free function returning the commands (one undo step each), tested
//! without a window.

use crate::clips::Ids;
use crate::record_pass::{clear_range, piece, Sim};
use cantus_core::{Clip, ClipId, Command, Project, Ticks, Track, TrackId};

/// What a clip shows and where: its content's identity (the audio source, or the first
/// note's id) and the timeline position of its content tick zero. Two clips with the same
/// key show the same thing at the same place.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ContentKey {
    pub content: u64,
    pub audio: bool,
    pub origin: i64,
}

pub fn content_key(c: &Clip) -> Option<ContentKey> {
    let origin = (c.start - c.offset).0;
    match &c.audio {
        Some(a) => Some(ContentKey {
            content: a.source.0,
            audio: true,
            origin,
        }),
        None => c.notes.first().map(|n| ContentKey {
            content: n.id.0,
            audio: false,
            origin,
        }),
    }
}

/// Whether the content of `take` (on a take lane) plays anywhere on lane 0.
fn in_comp(track: &Track, take: &Clip) -> bool {
    let Some(key) = content_key(take) else {
        return false;
    };
    track.clips.iter().any(|c| {
        c.lane == 0 && c.start < take.end() && c.end() > take.start && content_key(c) == Some(key)
    })
}

/// Call `f` with every range of `take` that the comp plays (for lighting the take lanes).
/// Allocates nothing.
pub fn for_each_comped(track: &Track, take: &Clip, mut f: impl FnMut(Ticks, Ticks)) {
    let Some(key) = content_key(take) else {
        return;
    };
    for c in track.clips.iter().filter(|c| c.lane == 0) {
        let (a, b) = (c.start.max(take.start), c.end().min(take.end()));
        if b > a && content_key(c) == Some(key) {
            f(a, b);
        }
    }
}

/// Swipe `from..to` of take lane `lane` into the comp, inside a [`Sim`] so that swipes
/// compose. Returns whether anything changed.
pub fn swipe_in(sim: &mut Sim, tid: TrackId, lane: u8, from: Ticks, to: Ticks, ids: Ids) -> bool {
    if lane == 0 || to <= from {
        return false;
    }
    let Some(track) = sim.track(tid) else {
        return false;
    };
    let takes: Vec<Clip> = track
        .clips
        .iter()
        .filter(|c| c.lane == lane && c.start < to && c.end() > from)
        .cloned()
        .collect();
    let mut changed = false;
    for take in takes {
        let (mut a, mut b) = (from.max(take.start), to.min(take.end()));
        if b <= a {
            continue;
        }
        let Some(track) = sim.track(tid) else {
            break;
        };
        // A lane-0 window on the same take that touches the swipe grows with it, so a comp
        // made in several strokes stays one clip.
        let key = content_key(&take);
        for c in track.clips.iter().filter(|c| c.lane == 0) {
            if key.is_some() && content_key(c) == key {
                if c.start < a && c.end() >= a {
                    a = c.start;
                }
                if c.end() > b && c.start <= b {
                    b = c.end();
                }
            }
        }
        let mut cmds = clear_range(track, a, b, None, ids);
        let mut w = piece(&take, a, b, 0, ids);
        w.muted = false;
        cmds.push(Command::InsertClip {
            track: tid,
            clip: Box::new(w),
        });
        if sim.push(cmds).is_err() {
            return changed;
        }
        changed = true;
    }
    changed
}

/// Swipe `from..to` of take lane `lane` into the comp: that range of the playing lane
/// becomes a window on the take. One undo step.
pub fn swipe(
    project: &Project,
    tid: TrackId,
    lane: u8,
    from: Ticks,
    to: Ticks,
    ids: Ids,
) -> Vec<Command> {
    let mut sim = Sim::new(project);
    swipe_in(&mut sim, tid, lane, from, to, ids);
    sim.commands
}

/// Make a whole take lane the comp, wherever it has material.
pub fn promote_lane(project: &Project, tid: TrackId, lane: u8, ids: Ids) -> Vec<Command> {
    let mut sim = Sim::new(project);
    let ranges: Vec<(Ticks, Ticks)> = project
        .track(tid)
        .map(|t| {
            t.clips
                .iter()
                .filter(|c| c.lane == lane)
                .map(|c| (c.start, c.end()))
                .collect()
        })
        .unwrap_or_default();
    for (a, b) in ranges {
        swipe_in(&mut sim, tid, lane, a, b, ids);
    }
    sim.commands
}

/// Make one take clip the comp over its whole range.
pub fn promote_clip(project: &Project, tid: TrackId, clip: ClipId, ids: Ids) -> Vec<Command> {
    let Some(c) = project.track(tid).and_then(|t| t.clip(clip)) else {
        return Vec::new();
    };
    swipe(project, tid, c.lane, c.start, c.end(), ids)
}

/// Delete every take clip none of whose content is in the comp, close the gaps that leaves
/// between lanes, and take out of the pool the recordings nothing plays any more.
pub fn delete_unused_takes(project: &Project, tid: TrackId) -> Vec<Command> {
    let Some(track) = project.track(tid) else {
        return Vec::new();
    };
    let mut sim = Sim::new(project);
    let unused: Vec<&Clip> = track
        .clips
        .iter()
        .filter(|c| c.lane > 0 && !in_comp(track, c))
        .collect();
    let mut cmds: Vec<Command> = unused
        .iter()
        .map(|c| Command::RemoveClip {
            track: tid,
            clip: c.id,
        })
        .collect();
    // The lanes left, renumbered 1, 2, … in their order.
    let mut lanes: Vec<u8> = track
        .clips
        .iter()
        .filter(|c| c.lane > 0 && !unused.iter().any(|u| u.id == c.id))
        .map(|c| c.lane)
        .collect();
    lanes.sort_unstable();
    lanes.dedup();
    for c in track.clips.iter().filter(|c| c.lane > 0) {
        if unused.iter().any(|u| u.id == c.id) {
            continue;
        }
        let to = lanes.iter().position(|l| *l == c.lane).unwrap_or(0) as u8 + 1;
        if to != c.lane {
            cmds.push(Command::SetClipLane {
                track: tid,
                clip: c.id,
                lane: to,
            });
        }
    }
    if sim.push(cmds).is_err() {
        return Vec::new();
    }
    // Recordings nobody plays any more leave the pool (the file stays on disk).
    let sources: Vec<_> = unused
        .iter()
        .filter_map(|c| c.audio.as_ref().map(|a| a.source))
        .collect();
    let mut gone = Vec::new();
    for s in sources {
        let used = sim.project.tracks.iter().any(|t| {
            t.clips
                .iter()
                .any(|c| c.audio.as_ref().is_some_and(|a| a.source == s))
        });
        if !used && !gone.contains(&s) {
            gone.push(s);
        }
    }
    let rm: Vec<Command> = gone
        .into_iter()
        .map(|id| Command::RemoveAudioSource { id })
        .collect();
    let _ = sim.push(rm);
    sim.commands
}

/// Each take lane becomes a track of its own under this one, on the same mixer strip,
/// with its clips **muted** so that what is heard does not change; the take lanes empty.
pub fn explode_takes(project: &Project, tid: TrackId, take_word: &str, ids: Ids) -> Vec<Command> {
    let Some((index, track)) = project.tracks.iter().enumerate().find(|(_, t)| t.id == tid) else {
        return Vec::new();
    };
    let mut lanes: Vec<u8> = track
        .clips
        .iter()
        .filter(|c| c.lane > 0)
        .map(|c| c.lane)
        .collect();
    lanes.sort_unstable();
    lanes.dedup();
    let mut sim = Sim::new(project);
    for (k, lane) in lanes.into_iter().enumerate() {
        let mut nt = track.clone();
        nt.id = TrackId(ids());
        nt.name = format!("{} \u{b7} {take_word} {}", track.name, k + 1);
        nt.clips = Vec::new();
        nt.armed = false;
        nt.takes_open = false;
        nt.soloed = false;
        // A modulator lives once: the copy takes none (ADR 0057).
        nt.modulators = Vec::new();
        let mut cmds = vec![Command::InsertTrack {
            index: index + 1 + k,
            track: Box::new(nt.clone()),
        }];
        for c in track.clips.iter().filter(|c| c.lane == lane) {
            cmds.push(Command::MoveClipToTrack {
                from: tid,
                to: nt.id,
                clip: c.id,
            });
            cmds.push(Command::SetClipLane {
                track: nt.id,
                clip: c.id,
                lane: 0,
            });
            if !c.muted {
                cmds.push(Command::SetClipMute {
                    track: nt.id,
                    clip: c.id,
                    muted: true,
                });
            }
        }
        if sim.push(cmds).is_err() {
            return Vec::new();
        }
    }
    // The lanes are empty now: close them.
    if track.takes_open {
        let _ = sim.push(vec![Command::SetTrackTakesOpen {
            id: tid,
            open: false,
        }]);
    }
    sim.commands
}
