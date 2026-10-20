//! A tempo edit keeps unwarped audio where it is in its file (ADR 0060 §4, review 3).
//!
//! An unwarped audio window plays its source at natural speed from its content origin
//! (`start − offset`, `crate::audio_time`), so every window edit's tick arithmetic is
//! exact and all the windows of one take agree. Its hidden part before its start is
//! measured through the tempo map, though, and a tempo edit there would change which
//! source frame plays at the window's start. The command bus therefore follows every
//! command that changes the tempo map with the window edits that keep that frame: the
//! same seconds of source under each such window's start, in whole ticks where it now
//! starts and the rest in its fine offset ([`crate::audio_time::anchor`]) — one batch, one
//! undo step, its inverse exact. A window the command itself moved or changed is the
//! command's; a linked window's fine offset is its group's, so only its offset moves (to
//! within half a tick); a window warped to follow the tempo reads content ticks and is
//! left alone. The kept windows of a track's alternate comps are kept the same way.

use crate::command::Command;
use crate::model::{Clip, ClipId, Project, TrackId};
use crate::tempo::{TempoMap, TempoPoint};
use crate::time::Ticks;

/// One unwarped window before the edit: where it is and the source second at its start.
#[derive(Clone, Debug)]
struct Window {
    track: TrackId,
    /// A clip of the track, or window `.1` of alternate comp `.0`.
    at: Where,
    start: Ticks,
    offset: Ticks,
    slip: i32,
    seconds: f64,
    rate: f64,
}

#[derive(Copy, Clone, Debug, PartialEq)]
enum Where {
    Clip(ClipId),
    Comp(usize, usize),
}

/// What a tempo edit has to keep, taken before it runs.
#[derive(Clone, Debug)]
pub struct Kept {
    bpm: f64,
    points: Vec<TempoPoint>,
    windows: Vec<Window>,
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

/// A window this keeps: unwarped audio (or warped without following the tempo).
fn kept_window(c: &Clip) -> bool {
    c.audio
        .as_ref()
        .is_some_and(|a| !a.warp.as_ref().is_some_and(|w| w.follow))
}

fn window(project: &Project, map: &TempoMap, track: TrackId, at: Where, c: &Clip) -> Window {
    let a = c.audio.as_ref().expect("an audio window");
    let rate = project
        .audio_source(a.source)
        .map_or(48_000.0, |s| f64::from(s.sample_rate.max(1)));
    Window {
        track,
        at,
        start: c.start,
        offset: c.offset,
        slip: a.slip_frames,
        seconds: crate::audio_time::offset_seconds(map, c.start, c.offset),
        rate,
    }
}

/// Take what `cmd` must keep, before it runs; `None` when it cannot change the tempo or
/// the project has no window to keep.
pub fn before(project: &Project, cmd: &Command) -> Option<Kept> {
    if !changes_tempo(cmd) {
        return None;
    }
    let map = project.tempo_map();
    let mut windows = Vec::new();
    for t in &project.tracks {
        for c in t.clips.iter().filter(|c| kept_window(c)) {
            windows.push(window(project, &map, t.id, Where::Clip(c.id), c));
        }
        for (i, comp) in t.comps.iter().enumerate() {
            for (j, c) in comp
                .windows
                .iter()
                .enumerate()
                .filter(|(_, c)| kept_window(c))
            {
                windows.push(window(project, &map, t.id, Where::Comp(i, j), c));
            }
        }
    }
    (!windows.is_empty()).then(|| Kept {
        bpm: project.tempo_bpm,
        points: project.tempo_points.clone(),
        windows,
    })
}

/// The window edits that keep, after the edit ran, every window's source second at its
/// start; empty when the tempo map did not change.
pub fn after(kept: &Kept, project: &Project) -> Vec<Command> {
    if project.tempo_bpm == kept.bpm && project.tempo_points == kept.points {
        return Vec::new();
    }
    let map = project.tempo_map();
    let mut out = Vec::new();
    // Alternate comps' windows: one replacement of the track's comps each.
    let mut comps: Vec<(TrackId, Vec<crate::comp::Comp>)> = Vec::new();
    for w in &kept.windows {
        let Some(track) = project.track(w.track) else {
            continue;
        };
        let clip = match w.at {
            Where::Clip(id) => track.clip(id),
            Where::Comp(i, j) => track.comps.get(i).and_then(|c| c.windows.get(j)),
        };
        // A window the command moved or changed is the command's.
        let Some(c) = clip.filter(|c| {
            kept_window(c)
                && c.start == w.start
                && c.offset == w.offset
                && c.audio.as_ref().is_some_and(|a| a.slip_frames == w.slip)
        }) else {
            continue;
        };
        let now = crate::audio_time::offset_seconds(&map, c.start, c.offset);
        if (now - w.seconds).abs() * w.rate < 1e-6 {
            continue;
        }
        let linked = c.link.is_some();
        let (offset, slip) = if linked {
            // The fine offset is the group's: the offset alone, to within half a tick.
            (
                crate::audio_time::offset_for(&map, c.start, w.seconds),
                w.slip,
            )
        } else {
            crate::audio_time::anchor(&map, c.start, w.seconds, w.slip, w.rate)
        };
        if offset == c.offset && slip == w.slip {
            continue;
        }
        match w.at {
            Where::Clip(id) => {
                if offset != c.offset {
                    out.push(Command::SetClipOffset {
                        track: w.track,
                        clip: id,
                        offset,
                    });
                }
                if slip != w.slip {
                    let mut a = c.audio.clone().expect("an audio window");
                    a.slip_frames = slip;
                    out.push(Command::SetClipAudio {
                        track: w.track,
                        clip: id,
                        audio: Some(a),
                    });
                }
            }
            Where::Comp(i, j) => {
                let at = match comps.iter().position(|(t, _)| *t == w.track) {
                    Some(k) => k,
                    None => {
                        comps.push((w.track, track.comps.clone()));
                        comps.len() - 1
                    }
                };
                let win = &mut comps[at].1[i].windows[j];
                win.offset = offset;
                if let Some(a) = win.audio.as_mut() {
                    a.slip_frames = slip;
                }
            }
        }
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
