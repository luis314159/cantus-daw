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
/// one playing (no windows kept), every kept window a playing-lane comp window
/// (`Clip::comp`, schema 32) whose id nothing else in the project (nor another kept
/// window) uses, its audio's source in the pool.
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
        return if active == 0 {
            Ok(())
        } else {
            bad("no comp to play")
        };
    }
    let Some(playing) = comps.get(active as usize) else {
        return bad("the playing comp is not there");
    };
    if !playing.windows.is_empty() {
        return bad("the playing comp's windows are lane 0's");
    }
    let mut seen: Vec<ClipId> = Vec::new();
    for w in comps.iter().flat_map(|c| c.windows.iter()) {
        if w.lane != 0 || w.length <= Ticks::ZERO || !w.comp {
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

/// Whether a clip can be one side of a comp seam (ADR 0058): a playing-lane audio window
/// with a length, neither reversed, looping nor warped (ADR 0061: a warped window plays a
/// render, whose seams would need the warp map; a warp that plays the source as it is —
/// [`crate::warp::Warp::natural`] — is no warp here) — the windows the engine crossfades. One
/// test for the snapshot, the seam command and the arrangement, so none of them shows or
/// moves a seam another does not play.
pub fn seam_window(c: &Clip) -> bool {
    c.lane == 0
        && c.length > Ticks::ZERO
        && c.period().is_none_or(|p| p >= c.length)
        && c.audio
            .as_ref()
            .is_some_and(|a| !a.reverse && a.warp.as_ref().is_none_or(|w| w.natural()))
}

/// Whether `left` and `right` make a comp seam (ADR 0058): two seam windows
/// ([`seam_window`]) where the left's end is the right's start — and not one stretch of
/// one file cut in two (a split window: the same source, the right going on from where
/// the left stops), which plays as one and has nothing to cross (integration review: a
/// split carried the window's crossfade and added a +3 dB swell at the cut). One test for
/// the snapshot, the seam command and the arrangement.
pub fn is_seam(left: &Clip, right: &Clip) -> bool {
    let continues = match (&left.audio, &right.audio) {
        (Some(a), Some(b)) => a.source == b.source && right.offset == left.offset + left.length,
        _ => false,
    };
    left.id != right.id
        && left.end() == right.start
        && seam_window(left)
        && seam_window(right)
        && !continues
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
    if !is_seam(&l, &r) {
        return Err(CommandError::InvalidValue("not a comp seam".into()));
    }
    if at <= l.start || at >= r.end() {
        return Err(CommandError::InvalidValue(
            "the seam leaves its windows".into(),
        ));
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
        c.offset += d;
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
        let mut l = p.make_audio_clip("a", srcs[0], Ticks::ZERO, Ticks::beats(4));
        let mut r = p.make_audio_clip("b", srcs[1], Ticks::beats(4), Ticks::beats(4));
        r.offset = Ticks::beats(4);
        (l.comp, r.comp) = (true, true);
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
    fn a_looping_window_has_no_seam_anywhere() {
        // Review 1: the arrangement showed (and moved) seams the engine never plays.
        let (mut p, t, l, r) = comped();
        let tr = p.track_mut(t).unwrap();
        let c = tr.clip_mut(r).unwrap();
        c.content_length = Some(Ticks::beats(1));
        c.audio.as_mut().unwrap().xfade = Ticks(20);
        assert!(!seam_window(p.track(t).unwrap().clip(r).unwrap()));
        let moved = Command::SetCompSeam {
            track: t,
            left: l,
            right: r,
            at: Ticks::beats(3),
        }
        .apply(&mut p);
        assert!(moved.is_err());
        let s = crate::Snapshot::compile(&p, 1);
        let tr = &s.tracks[0];
        assert!(tr.audio.iter().all(|a| a.xin.is_none() && a.xout.is_none()));
    }

    #[test]
    fn a_track_inserted_with_comps_another_track_keeps_is_refused() {
        // Review 1: explode copied a track's comps, kept-window ids and all.
        let (mut p, t, l, _) = comped();
        let mut w = p.track(t).unwrap().clip(l).unwrap().clone();
        w.id = ClipId(p.alloc_id());
        let comps = vec![
            Comp {
                name: letter(0),
                windows: Vec::new(),
            },
            Comp {
                name: letter(1),
                windows: vec![w],
            },
        ];
        Command::SetTrackComps {
            track: t,
            comps: comps.clone(),
            active: 0,
        }
        .apply(&mut p)
        .unwrap();
        let mut copy = p.make_track("Copy");
        copy.comps = comps;
        let refused = Command::InsertTrack {
            index: 1,
            track: Box::new(copy),
        }
        .apply(&mut p);
        assert!(
            matches!(refused, Err(CommandError::DuplicateId(_))),
            "{refused:?}"
        );
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
        let take = p
            .track(t)
            .unwrap()
            .clips
            .iter()
            .find(|c| c.lane == 1)
            .unwrap()
            .id;
        assert!(
            seam(r, l, Ticks::beats(3)).apply(&mut p.clone()).is_err(),
            "order"
        );
        assert!(
            seam(take, r, Ticks::beats(3))
                .apply(&mut p.clone())
                .is_err(),
            "lane"
        );
        assert!(
            seam(l, r, Ticks::ZERO).apply(&mut p.clone()).is_err(),
            "outside"
        );
        assert!(
            seam(l, r, Ticks::beats(8)).apply(&mut p.clone()).is_err(),
            "outside"
        );
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
        assert_eq!(
            (letter(0), letter(2), letter(40)),
            ("A".into(), "C".into(), "Z".into())
        );
    }

    /// Integration review: a split comp window is no seam — its right half does not carry
    /// the window's crossfade, and a cut found in a project (one made before) is not taken
    /// for a seam by the snapshot nor moved as one.
    #[test]
    fn a_split_window_is_no_seam() {
        let (mut p, tid, _, rid) = comped();
        if let Some(c) = p.track_mut(tid).and_then(|t| t.clip_mut(rid)) {
            c.audio.as_mut().unwrap().xfade = Ticks(20);
        }
        let right = p.track(tid).unwrap().clip(rid).unwrap().clone();
        let mut next = 9_000u64;
        let mut ids = || {
            next += 1;
            next
        };
        let cmds = crate::clipboard::split(&p, tid, &right, Ticks::beats(6), &mut ids).unwrap();
        let half = cmds
            .iter()
            .find_map(|c| match c {
                Command::InsertClip { clip, .. } => Some(clip.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(half.audio.as_ref().unwrap().xfade, Ticks::ZERO);
        // A cut whose right half still has one: not a seam.
        let mut carried = (*half).clone();
        carried.audio.as_mut().unwrap().xfade = Ticks(20);
        let mut left = right.clone();
        left.length = Ticks::beats(2);
        assert!(!is_seam(&left, &carried));
        p.track_mut(tid).unwrap().clip_mut(rid).unwrap().length = Ticks::beats(2);
        p.track_mut(tid).unwrap().clips.push(carried.clone());
        assert!(apply_seam(&mut p, tid, rid, carried.id, Ticks::beats(5)).is_err());
    }

    /// Schema 32: an alternate comp keeps comp windows only — a clip that is not one
    /// (`Clip::comp` false) is refused, so a switch back never brings a clip that would
    /// stay behind at the next switch.
    #[test]
    fn a_kept_window_is_a_comp_window() {
        let (mut p, t, l, _) = comped();
        let mut kept = p.track(t).unwrap().clip(l).unwrap().clone();
        kept.id = ClipId(p.alloc_id());
        let comps = |w: Clip| {
            vec![
                Comp {
                    name: letter(0),
                    windows: vec![],
                },
                Comp {
                    name: letter(1),
                    windows: vec![w],
                },
            ]
        };
        assert!(apply_set_comps(&mut p.clone(), t, comps(kept.clone()), 0).is_ok());
        kept.comp = false;
        assert!(matches!(
            apply_set_comps(&mut p, t, comps(kept), 0),
            Err(CommandError::InvalidValue(_))
        ));
    }

    /// Schema 32: the halves of a split comp window are the comp's; a copy of it — a
    /// duplicate, a linked copy, a paste, linked or not, even right over its takes — is
    /// not.
    #[test]
    fn a_split_keeps_a_window_in_its_comp_and_a_copy_or_paste_is_none() {
        let (mut p, t, l, _) = comped();
        let w = p.track(t).unwrap().clip(l).unwrap().clone();
        let mut next = 9_000u64;
        let mut ids = || {
            next += 1;
            next
        };
        let cmds = crate::clipboard::split(&p, t, &w, Ticks::beats(2), &mut ids).unwrap();
        let half = cmds
            .iter()
            .find_map(|c| match c {
                Command::InsertClip { clip, .. } => Some(clip.clone()),
                _ => None,
            })
            .unwrap();
        assert!(half.comp, "the right half of a window is the comp's");
        assert!(!crate::clipboard::copy_of(&w, w.start, &mut ids).comp);
        let (_, linked) = crate::links::Linker::new().copy(t, &w, w.start, &mut ids);
        assert!(!linked.comp, "a linked copy is no window");
        let origin = crate::clipboard::ClipOrigin::default();
        let payload = crate::clipboard::copy_clips(&p, &[(t, l)]);
        let board = crate::clipboard::Clipboard::new(origin, &p, payload);
        for linked in [false, true] {
            let pasted = crate::clipboard::paste_clips(
                &p,
                &board,
                true,
                0,
                w.start,
                &std::collections::HashMap::new(),
                linked,
                &mut ids,
            );
            let inserted: Vec<&Clip> = pasted
                .commands
                .iter()
                .filter_map(|c| match c {
                    Command::InsertClip { clip, .. } => Some(&**clip),
                    _ => None,
                })
                .collect();
            assert_eq!(inserted.len(), 1);
            assert!(
                !inserted[0].comp,
                "a paste over its takes is no window (linked: {linked})"
            );
        }
        // And the flag is a placement's own: a command sets it and takes it back exactly.
        let before = p.clone();
        let undo = Command::SetClipComp {
            track: t,
            clip: l,
            comp: false,
        }
        .apply(&mut p)
        .unwrap();
        assert!(!p.track(t).unwrap().clip(l).unwrap().comp);
        undo.apply(&mut p).unwrap();
        assert_eq!(p, before);
    }
}
