//! Take lanes and comping (ADR 0025 §5), as data: swipe a range of a take into the
//! playing lane, promote a whole take, delete the takes nothing uses, explode the takes to
//! tracks of their own.
//!
//! **A comp is lane-0 clips that are windows on the takes.** Nothing is copied: a window
//! shows the same file (audio) or the same notes, note ids and all (MIDI, the way a split
//! shares them), through another range. Which lane-0 clips are the comp's is said by the
//! clip itself (`Clip::comp`, schema 32): a swipe, a promote and a recording laid over the
//! takes make windows, a split keeps them, a copy or a paste is never one — wherever it
//! lands. Which take a window shows is what it plays: the take whose stretch of the same
//! content it plays, which is how the take lanes know which of their ranges to light.
//!
//! Every operation is a free function returning the commands (one undo step each), tested
//! without a window.
//!
//! **Round 28 (ADR 0058):** a new seam between two audio windows gets the project's seam
//! crossfade (`RecordSettings::comp_xfade_ms`, rendered by the engine); a window grown by a
//! swipe keeps what its absorbed window carried — its pitch edits, its gain; a swipe on a
//! track whose strip is in a mixer group swipes the group's tracks too
//! (`RecordSettings::comp_grouped`); seams are moved as a unit (`Command::SetCompSeam`)
//! and their crossfades set one by one or all at once; alternate comps are switched,
//! duplicated, deleted and renamed; a comp is flattened (the takes go, the windows stay),
//! or merged into one clip (`views::modulator`-free, in `comp_merge`).

use crate::clips::Ids;
use crate::record_pass::{clear_range, piece, Sim};
use cantus_core::comp::{letter, Comp};
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

/// The stretch of `take` (on its take lane, in timeline ticks) whose content `c` plays —
/// the same source, the same stretch of it, wherever `c` sits — or `None`. A shared tick
/// or so is a rounding's sliver (a loop recording's passes are cut from one file on whole
/// ticks), never a window: it shows nothing.
fn played_of(c: &Clip, take: &Clip) -> Option<(Ticks, Ticks)> {
    let (k, t) = (content_key(c)?, content_key(take)?);
    if (k.content, k.audio) != (t.content, t.audio) {
        return None;
    }
    let lo = c.offset.max(take.offset);
    let hi = (c.offset + c.length).min(take.offset + take.length);
    (hi - lo > SLIVER).then(|| {
        let at = |x: Ticks| take.start + (x - take.offset);
        (at(lo), at(hi))
    })
}

/// What [`played_of`] takes for a rounding's sliver.
const SLIVER: Ticks = Ticks(1);

/// Whether `c` (a playing-lane clip) is a window onto `take` (a take-lane clip): it is a
/// comp window (`Clip::comp`, schema 32 — said, never guessed from where it lies) that
/// plays some of the take's stretch of its content (ADR 0058). A window where its take put
/// it (the same origin) is that take's alone; one slipped or moved (an origin no take of
/// its content has) shows each take whose stretch it plays. A copy is no window, wherever
/// it is pasted — over its takes too.
fn shows_take(track: &Track, c: &Clip, take: &Clip) -> bool {
    if !c.comp || c.lane != 0 || take.lane == 0 || played_of(c, take).is_none() {
        return false;
    }
    let (Some(k), Some(t)) = (content_key(c), content_key(take)) else {
        return false;
    };
    k.origin == t.origin
        || !track
            .clips
            .iter()
            .any(|x| x.lane > 0 && content_key(x) == Some(k))
}

/// The take a comp window shows (the first one, by lane), for its number on the playing
/// lane.
pub fn take_of<'t>(track: &'t Track, w: &Clip) -> Option<&'t Clip> {
    track
        .clips
        .iter()
        .filter(|t| shows_take(track, w, t))
        .min_by_key(|t| (t.lane, t.start))
}

/// Whether any of the content of `take` (on a take lane) plays — on lane 0 or in an
/// alternate comp's kept windows (ADR 0058), which play when that comp does — wherever it
/// was slipped or moved to: a clip of the same source that plays some of the take's
/// stretch of it. What *Delete unused takes* keeps (review 2: a window moved off its take
/// lost its take). Any playing clip counts, comp window or not: a take is let go only
/// when nothing plays it.
fn in_comp(track: &Track, take: &Clip) -> bool {
    let plays = |c: &Clip| c.lane == 0 && played_of(c, take).is_some();
    track.clips.iter().any(plays) || track.comps.iter().flat_map(|c| c.windows.iter()).any(plays)
}

/// Call `f` with every range of `take` that the comp plays (for lighting the take lanes):
/// where on the take lane the content each of its windows plays lies. Allocates nothing.
pub fn for_each_comped(track: &Track, take: &Clip, mut f: impl FnMut(Ticks, Ticks)) {
    for c in track.clips.iter().filter(|c| shows_take(track, c, take)) {
        if let Some((a, b)) = played_of(c, take) {
            let (a, b) = (a.max(take.start), b.min(take.end()));
            if b > a {
                f(a, b);
            }
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
        // made in several strokes stays one clip. A copy is no window: it is not grown.
        let key = content_key(&take);
        for c in track.clips.iter().filter(|c| c.lane == 0 && c.comp) {
            if key.is_some() && content_key(c) == key {
                if c.start < a && c.end() >= a {
                    a = c.start;
                }
                if c.end() > b && c.start <= b {
                    b = c.end();
                }
            }
        }
        // What the windows it absorbs carried — pitch edits, gain — stays with the grown
        // window (the pitch review of round 28): the comp's own edits are not the take's.
        // Several absorbed windows' pitch edits are merged, each over its own stretch of
        // the take (review 1).
        let mut absorbed: Vec<&Clip> = track
            .clips
            .iter()
            .filter(|c| c.lane == 0 && c.comp && key.is_some() && content_key(c) == key)
            .filter(|c| c.start < b && c.end() > a)
            .collect();
        absorbed.sort_by_key(|c| c.start);
        let carried = absorbed.iter().find_map(|c| c.audio.clone()).map(|mut au| {
            au.pitch = merged_pitch(&sim.project, &absorbed);
            au
        });
        let mut cmds = clear_range(track, a, b, None, ids);
        let mut w = piece(&take, a, b, 0, ids);
        w.muted = false;
        // The comp's window, said (schema 32): a switch, a new comp and a merge know it.
        w.comp = true;
        if let (Some(region), Some(from)) = (w.audio.as_mut(), carried) {
            region.pitch = from.pitch;
            region.gain_db = from.gain_db;
            region.xfade = from.xfade;
        }
        cmds.push(Command::InsertClip {
            track: tid,
            clip: Box::new(w),
        });
        if sim.push(cmds).is_err() {
            return changed;
        }
        // The seams the swipe made crossfade (ADR 0058).
        let fresh = seam_defaults(&sim.project, tid, &[a, b]);
        if sim.push(fresh).is_err() {
            return changed;
        }
        changed = true;
    }
    changed
}

/// The pitch edits of several windows on one take as one edit (review 1): the first's,
/// with each later window's notes over its own stretch of the source in place of what was
/// there; renumbered so every id stays unique. The first window's edit alone when the
/// notes cannot make one valid edit (their output times would overlap).
fn merged_pitch(
    project: &Project,
    windows: &[&Clip],
) -> Option<Box<cantus_core::pitch::PitchEdit>> {
    let mut edits = windows.iter().filter_map(|c| {
        c.audio
            .as_ref()
            .and_then(|a| a.pitch.as_ref())
            .map(|e| (*c, e))
    });
    let (_, first) = edits.next()?;
    let mut out = (**first).clone();
    let bpm = project.tempo_bpm.max(1e-9);
    for (c, e) in edits {
        let rate = f64::from(e.rate.max(1));
        let slip = c.audio.as_ref().map_or(0.0, |a| f64::from(a.slip_frames));
        let frame = |t: Ticks| (t.to_seconds(bpm) * rate + slip).max(0.0) as u64;
        let (s0, s1) = (frame(c.offset), frame(c.offset + c.length));
        let incoming: Vec<_> = e
            .notes
            .iter()
            .filter(|n| n.src_start < s1 && n.src_end > s0)
            .cloned()
            .collect();
        if incoming.is_empty() {
            continue;
        }
        let lo = incoming
            .iter()
            .map(|n| n.src_start.min(n.start))
            .min()
            .unwrap_or(s0);
        let hi = incoming
            .iter()
            .map(|n| n.src_end.max(n.end))
            .max()
            .unwrap_or(s1);
        out.notes
            .retain(|n| n.src_end <= lo.min(s0) || n.src_start >= hi.max(s1));
        let next = out.notes.iter().map(|n| n.id).max().map_or(0, |m| m + 1);
        for (id, mut n) in (next..).zip(incoming) {
            n.id = id;
            out.notes.push(n);
        }
        out.notes.sort_by_key(|n| n.src_start);
    }
    if out.is_valid() {
        Some(Box::new(out))
    } else {
        Some(first.clone())
    }
}

/// The project's seam crossfade, in ticks at its tempo (ADR 0058).
pub fn xfade_ticks(project: &Project) -> Ticks {
    let ms = f64::from(project.recording.comp_xfade_ms.max(0.0));
    let t = (ms / 1000.0 * project.tempo_bpm / 60.0 * cantus_core::PPQ as f64).round() as i64;
    Ticks(t).min(cantus_core::audio::MAX_XFADE)
}

/// Every seam of a track's playing lane: (left window, right window, where, crossfade).
pub fn seams(track: &Track) -> Vec<(ClipId, ClipId, Ticks, Ticks)> {
    let audio = |c: &&Clip| !c.muted && cantus_core::comp::seam_window(c);
    let mut out = Vec::new();
    for r in track.clips.iter().filter(audio) {
        if let Some(l) = track
            .clips
            .iter()
            .filter(audio)
            .find(|l| cantus_core::comp::is_seam(l, r))
        {
            let x = r.audio.as_ref().map_or(Ticks::ZERO, |a| a.xfade);
            out.push((l.id, r.id, r.start, x));
        }
    }
    out.sort_by_key(|s| s.2);
    out
}

/// The seams at `at` that have no crossfade yet get the project's.
fn seam_defaults(project: &Project, tid: TrackId, at: &[Ticks]) -> Vec<Command> {
    let x = xfade_ticks(project);
    let Some(track) = project.track(tid) else {
        return Vec::new();
    };
    if x <= Ticks::ZERO {
        return Vec::new();
    }
    seams(track)
        .into_iter()
        .filter(|(_, _, s, xf)| at.contains(s) && *xf == Ticks::ZERO)
        .filter_map(|(_, r, _, _)| {
            let c = track.clip(r)?;
            let mut a = c.audio.clone()?;
            a.xfade = x;
            Some(Command::SetClipAudio {
                track: tid,
                clip: r,
                audio: Some(a),
            })
        })
        .collect()
}

/// Set the crossfade of the seam into window `right` (`None`: every seam of the track) —
/// and of the seam at the same place on every track that comps with it (review 1).
pub fn set_seam_xfade(
    project: &Project,
    tid: TrackId,
    right: Option<ClipId>,
    x: Ticks,
) -> Vec<Command> {
    let Some(track) = project.track(tid) else {
        return Vec::new();
    };
    let at = right.and_then(|r| track.clip(r)).map(|c| c.start);
    if right.is_some() && at.is_none() {
        return Vec::new();
    }
    let x = x.max(Ticks::ZERO).min(cantus_core::audio::MAX_XFADE);
    let mut out = Vec::new();
    for g in comp_group(project, tid) {
        let Some(track) = project.track(g) else {
            continue;
        };
        out.extend(
            seams(track)
                .into_iter()
                .filter(|(_, _, s, xf)| at.is_none_or(|a| a == *s) && *xf != x)
                .filter_map(|(_, r, _, _)| {
                    let mut a = track.clip(r)?.audio.clone()?;
                    a.xfade = x;
                    Some(Command::SetClipAudio {
                        track: g,
                        clip: r,
                        audio: Some(a),
                    })
                }),
        );
    }
    out
}

/// Move the seam between `left` and `right` to `at` — and the seam at the same place on
/// every track that comps with it, so a drum kit's microphones change takes together
/// (review 1). One `SetCompSeam` per track.
pub fn move_seam(
    project: &Project,
    tid: TrackId,
    left: ClipId,
    right: ClipId,
    at: Ticks,
) -> Vec<Command> {
    let Some(was) = project
        .track(tid)
        .and_then(|t| t.clip(right))
        .map(|c| c.start)
    else {
        return Vec::new();
    };
    let mut out = vec![Command::SetCompSeam {
        track: tid,
        left,
        right,
        at,
    }];
    for g in comp_group(project, tid).into_iter().filter(|g| *g != tid) {
        let Some(track) = project.track(g) else {
            continue;
        };
        if let Some((l, r, _, _)) = seams(track).into_iter().find(|s| s.2 == was) {
            let fits = match (track.clip(l), track.clip(r)) {
                (Some(lc), Some(rc)) => at > lc.start && at < rc.end(),
                _ => false,
            };
            if fits {
                out.push(Command::SetCompSeam {
                    track: g,
                    left: l,
                    right: r,
                    at,
                });
            }
        }
    }
    out
}

/// The tracks that comp with `tid`: its own, then those whose strips share its strip's
/// mixer group (when the project comps groups together).
pub fn comp_group(project: &Project, tid: TrackId) -> Vec<TrackId> {
    let mut out = vec![tid];
    if !project.recording.comp_grouped {
        return out;
    }
    let group = project
        .track(tid)
        .and_then(|t| t.strip)
        .and_then(|s| project.strip(s))
        .and_then(|s| s.group);
    if let Some(g) = group {
        for t in &project.tracks {
            let same = t
                .strip
                .and_then(|s| project.strip(s))
                .is_some_and(|s| s.group == Some(g));
            if same && t.id != tid {
                out.push(t.id);
            }
        }
    }
    out
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
    // The group's tracks too, each on the same lane (recorded together, a drum kit's
    // microphones comp together).
    for t in comp_group(project, tid) {
        swipe_in(&mut sim, t, lane, from, to, ids);
    }
    sim.commands
}

/// Make a whole take lane the comp, wherever it has material.
pub fn promote_lane(project: &Project, tid: TrackId, lane: u8, ids: Ids) -> Vec<Command> {
    let mut sim = Sim::new(project);
    for t in comp_group(project, tid) {
        let ranges: Vec<(Ticks, Ticks)> = project
            .track(t)
            .map(|t| {
                t.clips
                    .iter()
                    .filter(|c| c.lane == lane)
                    .map(|c| (c.start, c.end()))
                    .collect()
            })
            .unwrap_or_default();
        for (a, b) in ranges {
            swipe_in(&mut sim, t, lane, a, b, ids);
        }
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
                .chain(t.comps.iter().flat_map(|c| c.windows.iter()))
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
        // A modulator lives once: the copy takes none (ADR 0057); nor does it take the
        // track's alternate comps, whose kept windows are this track's (review 1).
        nt.modulators = Vec::new();
        nt.comps = Vec::new();
        nt.active_comp = 0;
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

// ---- alternate comps (ADR 0058) ---------------------------------------------------------

/// The windows of the comp playing: the playing lane's comp windows (`Clip::comp`,
/// schema 32) — a window slipped or moved is still the comp's (review 1), a copy is not,
/// wherever it was pasted (fix4: a copy pasted over its takes was taken for a window).
pub fn comp_windows(track: &Track) -> Vec<&Clip> {
    track
        .clips
        .iter()
        .filter(|c| c.lane == 0 && c.comp)
        .collect()
}

/// A window as an alternate comp keeps it: whole, but in no link group (ADR 0037's upkeep
/// walks the playing clips only).
fn kept(c: &Clip) -> Clip {
    Clip {
        link: None,
        ..c.clone()
    }
}

/// The comps as they are, or the one implicit comp ("A", playing) when there are none.
fn comps_now(track: &Track) -> (Vec<Comp>, u8) {
    if track.comps.is_empty() {
        (
            vec![Comp {
                name: letter(0),
                windows: Vec::new(),
            }],
            0,
        )
    } else {
        (track.comps.clone(), track.active_comp)
    }
}

/// Play comp `to`: the playing comp's windows are kept in its slot, whole, and `to`'s come
/// back to the playing lane. One undo step; nothing for the comp already playing.
pub fn switch_comp(project: &Project, tid: TrackId, to: u8) -> Vec<Command> {
    let Some(track) = project.track(tid) else {
        return Vec::new();
    };
    let (mut comps, active) = comps_now(track);
    if to == active || to as usize >= comps.len() {
        return Vec::new();
    }
    let playing: Vec<Clip> = comp_windows(track).into_iter().cloned().collect();
    let coming = std::mem::take(&mut comps[to as usize].windows);
    // A kept window owns its content: out of any link group, which no longer reaches it
    // (integration review: a group edited meanwhile made the switch back refused).
    comps[active as usize].windows = playing.iter().map(kept).collect();
    let mut cmds: Vec<Command> = playing
        .iter()
        .map(|c| Command::RemoveClip {
            track: tid,
            clip: c.id,
        })
        .collect();
    cmds.push(Command::SetTrackComps {
        track: tid,
        comps,
        active: to,
    });
    cmds.extend(coming.into_iter().map(|c| Command::InsertClip {
        track: tid,
        clip: Box::new(c),
    }));
    cmds
}

/// A new comp — the playing one copied — named by the next letter, and played; the comp it
/// was copied from keeps a copy of its windows. One undo step.
pub fn duplicate_comp(project: &Project, tid: TrackId, ids: Ids) -> Vec<Command> {
    let Some(track) = project.track(tid) else {
        return Vec::new();
    };
    let (mut comps, active) = comps_now(track);
    if comps.len() >= cantus_core::comp::MAX_COMPS {
        return Vec::new();
    }
    comps[active as usize].windows = comp_windows(track)
        .into_iter()
        .map(|c| Clip {
            id: ClipId(ids()),
            ..kept(c)
        })
        .collect();
    let name = (0..cantus_core::comp::MAX_COMPS)
        .map(letter)
        .find(|n| !comps.iter().any(|c| &c.name == n))
        .unwrap_or_else(|| letter(comps.len()));
    comps.push(Comp {
        name,
        windows: Vec::new(),
    });
    vec![Command::SetTrackComps {
        track: tid,
        active: (comps.len() - 1) as u8,
        comps,
    }]
}

/// Delete comp `which` (not the one playing). One undo step.
pub fn delete_comp(project: &Project, tid: TrackId, which: u8) -> Vec<Command> {
    let Some(track) = project.track(tid) else {
        return Vec::new();
    };
    let (mut comps, mut active) = comps_now(track);
    if which == active || which as usize >= comps.len() {
        return Vec::new();
    }
    comps.remove(which as usize);
    if which < active {
        active -= 1;
    }
    if comps.len() == 1 {
        comps.clear();
        active = 0;
    }
    vec![Command::SetTrackComps {
        track: tid,
        comps,
        active,
    }]
}

/// Rename comp `which`. One undo step.
pub fn rename_comp(project: &Project, tid: TrackId, which: u8, name: &str) -> Vec<Command> {
    let Some(track) = project.track(tid) else {
        return Vec::new();
    };
    let (mut comps, active) = comps_now(track);
    let name = name.trim();
    match comps.get_mut(which as usize) {
        Some(c) if !name.is_empty() && c.name != name => c.name = name.to_string(),
        _ => return Vec::new(),
    }
    vec![Command::SetTrackComps {
        track: tid,
        comps,
        active,
    }]
}

/// Flatten: the comp's windows stay on the playing lane as they are — plain clips now,
/// the comp is over (`Clip::comp` cleared) — the takes and the other comps go, the lanes
/// close, and recordings nothing plays any more leave the pool (their files stay on
/// disk). One undo step.
pub fn flatten(project: &Project, tid: TrackId) -> Vec<Command> {
    let Some(track) = project.track(tid) else {
        return Vec::new();
    };
    let mut sim = Sim::new(project);
    let mut cmds: Vec<Command> = track
        .clips
        .iter()
        .filter(|c| c.lane > 0)
        .map(|c| Command::RemoveClip {
            track: tid,
            clip: c.id,
        })
        .collect();
    cmds.extend(
        track
            .clips
            .iter()
            .filter(|c| c.lane == 0 && c.comp)
            .map(|c| Command::SetClipComp {
                track: tid,
                clip: c.id,
                comp: false,
            }),
    );
    if !track.comps.is_empty() {
        cmds.push(Command::SetTrackComps {
            track: tid,
            comps: Vec::new(),
            active: 0,
        });
    }
    if track.takes_open {
        cmds.push(Command::SetTrackTakesOpen {
            id: tid,
            open: false,
        });
    }
    if cmds.is_empty() || sim.push(cmds).is_err() {
        return Vec::new();
    }
    let gone: Vec<Command> = unused_sources(&sim.project, project)
        .into_iter()
        .map(|id| Command::RemoveAudioSource { id })
        .collect();
    let _ = sim.push(gone);
    sim.commands
}

/// Sources `before` had in its pool that nothing in `after` plays or keeps any more.
fn unused_sources(after: &Project, before: &Project) -> Vec<cantus_core::AudioSourceId> {
    before
        .audio_pool
        .iter()
        .map(|s| s.id)
        .filter(|id| {
            !after.tracks.iter().any(|t| {
                t.clips
                    .iter()
                    .chain(t.comps.iter().flat_map(|c| c.windows.iter()))
                    .any(|c| c.audio.as_ref().is_some_and(|a| a.source == *id))
            })
        })
        .collect()
}

/// A trim of a comp window's edge that is a seam (two windows meeting on the playing
/// lane of a track with takes) moves the seam: both windows at once, as Logic's comp
/// boundaries move (ADR 0058). `None` for any other edge — the trim stays a trim.
pub fn seam_trim(
    project: &Project,
    tid: TrackId,
    clip: &Clip,
    start_edge: bool,
    delta: Ticks,
) -> Option<Vec<Command>> {
    let track = project.track(tid)?;
    if clip.lane != 0 || !track.clips.iter().any(|c| c.lane > 0) {
        return None;
    }
    let (l, r, at, _) = seams(track).into_iter().find(|(l, r, _, _)| {
        if start_edge {
            *r == clip.id
        } else {
            *l == clip.id
        }
    })?;
    let left = track.clip(l)?;
    let right = track.clip(r)?;
    let to = at + delta;
    (to > left.start && to < right.end()).then(|| move_seam(project, tid, l, r, to))
}
