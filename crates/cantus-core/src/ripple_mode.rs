//! r40:ripple — the arrangement's ripple mode (ADR 0116 §4).
//!
//! The leaders that ripple single edits do it as a **mode**: Pro Tools' *Shuffle* edit mode,
//! Logic's *Shuffle* drag modes, Cubase's *Shuffle*, Studio One's *Ripple Edit* button, and
//! REAPER's three-way switch — off, per track, all tracks (Alt+P) — which is the most
//! complete and the one copied here: **Off** (the default), **This track** (moving,
//! resizing or deleting a clip moves what follows it on its own track) and **Every track**
//! (what follows on every track moves, with the markers, sections and tempo, as the time
//! edits do). The mode is the view's, never the song's: switching it is not an undo step and
//! a project never opens with it on.

use crate::clipboard::Ids;
use crate::command::Command;
use crate::model::{ClipId, Project, TrackId};
use crate::ripple::{apply_shift_in, Edit, Scope, Shift};
use crate::time::Ticks;

use serde::{Deserialize, Serialize};

/// What a clip edit ripples.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RippleMode {
    /// Nothing moves but the clip (every DAW's default).
    #[default]
    Off,
    /// What follows on the clip's own track.
    Track,
    /// What follows on every track, with the song's marks and its tempo.
    All,
}

impl RippleMode {
    /// The next mode of the switch (REAPER's order).
    pub fn next(self) -> Self {
        match self {
            RippleMode::Off => RippleMode::Track,
            RippleMode::Track => RippleMode::All,
            RippleMode::All => RippleMode::Off,
        }
    }

    pub fn is_on(self) -> bool {
        self != RippleMode::Off
    }
}

/// What follows a clip edit moves: on `track` (or every track, in [`RippleMode::All`]),
/// what starts at or after `at` moves by `delta` (later when positive) — but the clips the
/// gesture moved itself (`exclude`).
#[derive(Clone, Debug, PartialEq)]
pub struct Follow {
    pub track: TrackId,
    pub at: Ticks,
    pub delta: Ticks,
    pub exclude: Vec<ClipId>,
}

/// A clip gesture's own commands (`gesture`: a delete, a move, a trim) with what follows it
/// moved as `mode` says — one command, labelled `label`. With the mode off, or nothing to
/// move, it is the gesture alone. `None` when the model refuses a step.
pub fn rippled(
    project: &Project,
    gesture: Vec<Command>,
    label: &str,
    mode: RippleMode,
    follows: &[Follow],
    ids: Ids,
) -> Option<Command> {
    let mut e = Edit::new(project);
    e.run_all(gesture)?;
    if mode.is_on() {
        // The latest first, so an earlier one's shift never moves a later one's point.
        let mut follows = follows.to_vec();
        follows.sort_by_key(|f| std::cmp::Reverse(f.at));
        for f in &follows {
            if f.delta == Ticks::ZERO {
                continue;
            }
            let shift = if f.delta > Ticks::ZERO {
                Shift::Insert {
                    at: f.at,
                    len: f.delta,
                }
            } else {
                Shift::Delete {
                    from: (f.at + f.delta).max(Ticks::ZERO),
                    to: f.at,
                }
            };
            let marks = mode == RippleMode::All;
            let scope = Scope {
                tracks: (mode == RippleMode::Track).then(|| vec![f.track]),
                cut: false,
                marks,
                exclude: f.exclude.clone(),
                keep_cues: still_played(&e.scratch, shift, &f.exclude),
            };
            // R2-B2: what moves up plays under the scene it had (the song before this shift).
            let before = (marks && !e.scratch.mix.cues.is_empty()).then(|| e.scratch.clone());
            apply_shift_in(&mut e, shift, ids, &scope)?;
            if let (Some(before), Shift::Delete { from, to }) = (before, shift) {
                let moved = crate::ripple_cues::Piece { new: from, old: to };
                crate::ripple_cues::keep_scenes(&mut e, &before, &[moved], ids)?;
            }
        }
    }
    e.finish(label)
}

/// The cues inside the time an every-track ripple closes that another track still plays
/// over — a clip that stays where it is (it starts before the time's end, and is not the
/// gesture's) sounding at the cue's place: the cue is that music's scene, and stays with it
/// (review 2 of round 40, R2-B2). Inserting, none.
fn still_played(p: &Project, shift: Shift, exclude: &[ClipId]) -> Vec<crate::mix_snap::MixCueId> {
    let Shift::Delete { from, to } = shift else {
        return Vec::new();
    };
    p.mix
        .cues
        .iter()
        .filter(|c| c.at >= from && c.at < to)
        .filter(|c| {
            p.tracks.iter().flat_map(|t| &t.clips).any(|k| {
                !exclude.contains(&k.id) && k.start < to && k.start <= c.at && c.at < k.end()
            })
        })
        .map(|c| c.id)
        .collect()
}
