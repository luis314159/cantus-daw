//! Comping, as the model holds it (ADR 0058, extending ADR 0025 §5).
//!
//! **What plays is lane 0.** A comp is the playing lane's windows onto the takes (round
//! 19): nothing is copied, a window shows the take's own file through another range.
//!
//! **A seam is where two playing-lane audio windows meet**, and its crossfade is the right
//! window's `AudioRegion::xfade`: the engine overlaps the two by that much, centred on the
//! seam, equal-power (`snapshot::seams`). The seam is edited **as a unit** by
//! [`Command::SetCompSeam`](crate::Command::SetCompSeam): both sides move together, the
//! right window's content stays where it was on the timeline (its offset follows), and the
//! command refuses what is not a seam — two windows that do not meet, another lane, a
//! position outside the two.
//!
//! **Alternate comps** (Logic's A/B/C): `Track::comps` names them; the one playing
//! (`Track::active_comp`) has its windows on lane 0, every other keeps its windows here,
//! whole — pitch edits, fades and seams included — so switching back gives exactly what
//! was. [`Command::SetTrackComps`](crate::Command::SetTrackComps) replaces the list at
//! once; switching is that and the lane-0 windows swapped, one undo step.

use crate::command::{Command, CommandError};
use crate::model::{Clip, ClipId, Project, TrackId};
use crate::time::Ticks;
use serde::{Deserialize, Serialize};

/// Comps a track keeps at most.
pub const MAX_COMPS: usize = 26;

/// One alternate comp: its name and, while another plays, its windows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comp {
    pub name: String,
    /// Empty for the comp playing (its windows are lane 0's).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<Clip>,
}

/// The default name of the `n`th comp: A, B, C… Z.
pub fn letter(n: usize) -> String {
    char::from(b'A' + (n.min(25)) as u8).to_string()
}

/// Validate a list of comps for `track` in `project`: at most [`MAX_COMPS`], the active
/// one playing (no windows kept), every kept window a playing-lane clip whose id nothing
/// else in the project (nor another kept window) uses, its audio's source in the pool.
pub(crate) fn check(
    project: &Project,
    track: TrackId,
    comps: &[Comp],
    active: u8,
) -> Result<(), CommandError> {
    let bad = |what: &str| Err(CommandError::InvalidValue(format!("comps: {what}")));
    if comps.len() > MAX_COMPS {
        return bad("too many");
    }
    if comps.is_empty() {
        return if active == 0 { Ok(()) } else { bad("no comp to play") };
    }
    let Some(playing) = comps.get(active as usize) else {
        return bad("the playing comp is not there");
    };
    if !playing.windows.is_empty() {
        return bad("the playing comp's windows are lane 0's");
    }
    let mut seen: Vec<ClipId> = Vec::new();
    for w in comps.iter().flat_map(|c| c.windows.iter()) {
        if w.lane != 0 || w.length <= Ticks::ZERO {
            return bad("a kept window");
        }
        if seen.contains(&w.id) {
            return Err(CommandError::DuplicateId(w.id.0));
        }
        seen.push(w.id);
        let elsewhere = project.tracks.iter().any(|t| {
            t.clips.iter().any(|c| c.id == w.id)
                || (t.id != track
                    && t.comps
                        .iter()
                        .any(|c| c.windows.iter().any(|x| x.id == w.id)))
        });
        if elsewhere {
            return Err(CommandError::DuplicateId(w.id.0));
        }
        if let Some(a) = &w.audio {
            if project.audio_source(a.source).is_none() {
                return Err(CommandError::AudioSourceNotFound(a.source));
            }
        }
    }
    Ok(())
}

pub(crate) fn apply_set_comps(
    project: &mut Project,
    track: TrackId,
    comps: Vec<Comp>,
    active: u8,
) -> Result<Command, CommandError> {
    check(project, track, &comps, active)?;
    let t = project
        .track_mut(track)
        .ok_or(CommandError::TrackNotFound(track))?;
    let old = std::mem::replace(&mut t.comps, comps);
    let old_active = std::mem::replace(&mut t.active_comp, active);
    Ok(Command::SetTrackComps {
        track,
        comps: old,
        active: old_active,
    })
}

/// Move the seam between `left` and `right` (two playing-lane windows of `track` that
/// meet) to `at`, both sides at once; the inverse moves it back.
pub(crate) fn apply_seam(
    project: &mut Project,
    track: TrackId,
    left: ClipId,
    right: ClipId,
    at: Ticks,
) -> Result<Command, CommandError> {
    let t = project
        .track_mut(track)
        .ok_or(CommandError::TrackNotFound(track))?;
    let (l, r) = match (t.clip(left), t.clip(right)) {
        (Some(l), Some(r)) => (l.clone(), r.clone()),
        (None, _) => return Err(CommandError::ClipNotFound(left)),
        (_, None) => return Err(CommandError::ClipNotFound(right)),
    };
    let reversed = |c: &Clip| c.audio.as_ref().is_some_and(|a| a.reverse);
    if left == right
        || l.lane != 0
        || r.lane != 0
        || l.end() != r.start
        || reversed(&l)
        || reversed(&r)
    {
        return Err(CommandError::InvalidValue("not a comp seam".into()));
    }
    if at <= l.start || at >= r.end() {
        return Err(CommandError::InvalidValue("the seam leaves its windows".into()));
    }
    let was = r.start;
    let d = at - was;
    if let Some(c) = t.clip_mut(left) {
        c.length = at - c.start;
    }
    if let Some(c) = t.clip_mut(right) {
        let end = c.end();
        c.start = at;
        c.length = end - at;
        c.offset = c.offset + d;
    }
    // By start, then by id, as every clip edit keeps them.
    t.clips.sort_by_key(|c| (c.start, c.id));
    Ok(Command::SetCompSeam {
        track,
        left,
        right,
        at: was,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AudioSource, AudioSourceId};

    /// A track with two takes on lanes 1 and 2 and a comp on lane 0: the first take's
    /// window to beat 4, the second's after.
    fn comped() -> (Project, TrackId, ClipId, ClipId) {
        let mut p = Project::new("comp");
        let mut t = p.make_track("Vox");
        let tid = t.id;
        let mut srcs = Vec::new();
        for k in 0..2 {
            let id = AudioSourceId(p.alloc_id());
            p.audio_pool.push(AudioSource {
                id,
                name: format!("take {k}"),
                path: format!("t{k}.wav"),
                sample_rate: 48_000,
                channels: 1,
                frames: 48_000 * 8,
                sha256: None,
                recorded: true,
            });
            srcs.push(id);
        }
        for (k, s) in srcs.iter().enumerate() {
            let mut take = p.make_audio_clip("take", *s, Ticks::ZERO, Ticks::beats(8));
            take.lane = k as u8 + 1;
            t.clips.push(take);
        }
        let l = p.make_audio_clip("a", srcs[0], Ticks::ZERO, Ticks::beats(4));
        let mut r = p.make_audio_clip("b", srcs[1], Ticks::beats(4), Ticks::beats(4));
        r.offset = Ticks::beats(4);
        let (lid, rid) = (l.id, r.id);
        t.clips.extend([l, r]);
        t.clips.sort_by_key(|c| (c.start, c.id));
        p.tracks.push(t);
        (p, tid, lid, rid)
    }

    #[test]
    fn a_seam_moves_as_a_unit_and_back_exactly() {
        let (mut p, t, l, r) = comped();
        let before = p.clone();
        let undo = Command::SetCompSeam {
            track: t,
            left: l,
            right: r,
            at: Ticks::beats(3),
        }
        .apply(&mut p)
        .unwrap();
        let tr = p.track(t).unwrap();
        let (lc, rc) = (tr.clip(l).unwrap(), tr.clip(r).unwrap());
        assert_eq!(lc.end(), Ticks::beats(3));
        assert_eq!((rc.start, rc.end()), (Ticks::beats(3), Ticks::beats(8)));
        // The right window's content stays where it was on the timeline.
        assert_eq!(rc.start - rc.offset, Ticks::ZERO);
        undo.apply(&mut p).unwrap();
        assert_eq!(p, before);
    }

    #[test]
    fn what_is_not_a_seam_is_refused() {
        let (mut p, t, l, r) = comped();
        let seam = |left, right, at| Command::SetCompSeam {
            track: t,
            left,
            right,
            at,
        };
        let take = p.track(t).unwrap().clips.iter().find(|c| c.lane == 1).unwrap().id;
        assert!(seam(r, l, Ticks::beats(3)).apply(&mut p.clone()).is_err(), "order");
        assert!(seam(take, r, Ticks::beats(3)).apply(&mut p.clone()).is_err(), "lane");
        assert!(seam(l, r, Ticks::ZERO).apply(&mut p.clone()).is_err(), "outside");
        assert!(seam(l, r, Ticks::beats(8)).apply(&mut p.clone()).is_err(), "outside");
        // Two windows that no longer meet.
        p.track_mut(t).unwrap().clip_mut(l).unwrap().length = Ticks::beats(2);
        assert!(seam(l, r, Ticks::beats(3)).apply(&mut p).is_err());
    }

    #[test]
    fn comps_keep_their_windows_whole_and_refuse_a_bad_list() {
        let (mut p, t, l, _) = comped();
        let window = p.track(t).unwrap().clip(l).unwrap().clone();
        // A window kept while it is still on lane 0: its id is used.
        let dup = vec![
            Comp {
                name: letter(0),
                windows: vec![],
            },
            Comp {
                name: letter(1),
                windows: vec![window.clone()],
            },
        ];
        assert!(matches!(
            apply_set_comps(&mut p.clone(), t, dup, 0),
            Err(CommandError::DuplicateId(_))
        ));
        let mut kept = window.clone();
        kept.id = ClipId(p.alloc_id());
        let comps = vec![
            Comp {
                name: letter(0),
                windows: vec![],
            },
            Comp {
                name: letter(1),
                windows: vec![kept],
            },
        ];
        // The playing comp keeps no windows; the active index exists.
        assert!(apply_set_comps(&mut p.clone(), t, comps.clone(), 1).is_err());
        assert!(apply_set_comps(&mut p.clone(), t, comps.clone(), 5).is_err());
        let before = p.clone();
        let undo = apply_set_comps(&mut p, t, comps.clone(), 0).unwrap();
        assert_eq!(p.track(t).unwrap().comps, comps);
        undo.apply(&mut p).unwrap();
        assert_eq!(p, before);
        assert_eq!((letter(0), letter(2), letter(40)), ("A".into(), "C".into(), "Z".into()));
    }
}
