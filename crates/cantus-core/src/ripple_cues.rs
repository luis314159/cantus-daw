//! r40:fix-d — the mixer's scenes through a time edit (review 2 of round 40, R2-B2; ADR
//! 0116 §2). A cue (ADR 0093) is not only a point that moves with the music, as a marker
//! is: it **holds until the next one**, so a time edit that takes a cue away, or lays bars
//! somewhere new, changes the scene of everything after. The rule kept here is the sound's:
//! *every piece of music plays under the scene it played under before the edit*.
//!
//! An edit says where its music went as [`Piece`]s — this stretch of the new song plays
//! what began at that tick of the old one — and [`keep_scenes`] then looks at each piece's
//! start: where the scene in effect there differs from the one the music had, cues of their
//! own (fresh ids, at the list's end) put it back — the master and the whole desk with an
//! unscoped cue, the channels that still differ with one scoped cue per snapshot. Where the
//! scene is already right (a cue that moved with the music, nothing changed), nothing is
//! added. So deleting time whose last cue was inside the range leaves, at the seam, the
//! scene that cue gave what follows; bars laid elsewhere carry their scene in, and the music
//! after them gets its own back.
//!
//! The scene of a channel at a tick is the snapshot of the latest cue that sets it there —
//! in the cues' playing order (by tick, then id, as `mix_snap::cue_lanes` lays them): a cue
//! sets the channels its snapshot holds that its scope names (every one but the recall-safe
//! when the scope is empty), and the master only when unscoped.

use crate::command::Command;
use crate::mix_cmd::MixCommand;
use crate::mix_snap::{MixCue, MixCueId, MixSnapId};
use crate::model::{Project, StripId};
use crate::ripple::Edit;
use crate::time::Ticks;
use std::collections::BTreeMap;

/// What a channel of the scene is: a strip, or the master (`None`).
pub type Channel = Option<StripId>;

/// The snapshot each channel plays at a tick (a channel no cue has set yet is absent).
pub type Scene = BTreeMap<Channel, MixSnapId>;

/// A stretch of the new song that plays what began at `old` in the song before the edit,
/// from `new` on (until the next piece).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub new: Ticks,
    pub old: Ticks,
}

/// The channels `cue` sets in `p`, each with its snapshot.
fn sets(p: &Project, cue: &MixCue) -> Vec<Channel> {
    let Some(snap) = p.mix.snapshot(cue.snapshot) else {
        return Vec::new();
    };
    let mut out: Vec<Channel> = snap
        .state
        .strips
        .iter()
        .filter(|st| {
            p.strip(st.id).is_some_and(|s| {
                cue.scope.contains(&st.id) || (cue.scope.is_empty() && !s.recall_safe)
            })
        })
        .map(|st| Some(st.id))
        .collect();
    if cue.scope.is_empty() && snap.state.master.is_some() {
        out.push(None);
    }
    out
}

/// The scene of `p` at `at`: every cue at or before it, in playing order.
pub fn scene_at(p: &Project, at: Ticks) -> Scene {
    let mut cues: Vec<&MixCue> = p.mix.cues.iter().filter(|c| c.at <= at).collect();
    cues.sort_by_key(|c| (c.at, c.id));
    let mut scene = Scene::new();
    for c in cues {
        for ch in sets(p, c) {
            scene.insert(ch, c.snapshot);
        }
    }
    scene
}

/// Each piece of the edited song (`e.scratch`) under the scene its music had in `old`, in
/// order of where the pieces begin: cues added where it differs ([`self`]). `ids` makes the
/// new cues' ids, which must play after every cue already there at the same tick (a
/// project's ids only grow).
pub(crate) fn keep_scenes(
    e: &mut Edit,
    old: &Project,
    pieces: &[Piece],
    ids: crate::clipboard::Ids,
) -> Option<()> {
    if old.mix.cues.is_empty() {
        return Some(());
    }
    let mut pieces = pieces.to_vec();
    pieces.sort_by_key(|p| p.new);
    pieces.dedup_by_key(|p| p.new);
    for piece in pieces {
        let want = scene_at(old, piece.old);
        restore(e, piece.new, &want, ids)?;
    }
    Some(())
}

/// Cues at `at` that make the scene there `want` (only what differs).
fn restore(e: &mut Edit, at: Ticks, want: &Scene, ids: crate::clipboard::Ids) -> Option<()> {
    let differs = |e: &Edit| -> Vec<Channel> {
        let now = scene_at(&e.scratch, at);
        let mut d: Vec<Channel> = want
            .iter()
            .filter(|(ch, s)| now.get(*ch) != Some(*s))
            .map(|(ch, _)| *ch)
            .collect();
        // A channel set now that no cue had set for this music: no cue can unset it (the
        // edit moved no cue onto music that had none), so it is left as it is.
        d.dedup();
        d
    };
    let mut d = differs(e);
    if d.is_empty() {
        return Some(());
    }
    // The master is set by an unscoped cue only: its snapshot for the whole desk, then the
    // channels that still differ, below.
    if d.contains(&None) {
        let snapshot = *want.get(&None)?;
        add(e, at, snapshot, Vec::new(), ids)?;
        d = differs(e);
    }
    let mut by_snap: BTreeMap<MixSnapId, Vec<StripId>> = BTreeMap::new();
    for ch in d {
        if let (Some(s), Some(snap)) = (ch, want.get(&ch)) {
            by_snap.entry(*snap).or_default().push(s);
        }
    }
    for (snapshot, mut scope) in by_snap {
        scope.sort();
        add(e, at, snapshot, scope, ids)?;
    }
    Some(())
}

fn add(
    e: &mut Edit,
    at: Ticks,
    snapshot: MixSnapId,
    scope: Vec<StripId>,
    ids: crate::clipboard::Ids,
) -> Option<()> {
    e.run(Command::Mix(MixCommand::InsertCue {
        index: e.scratch.mix.cues.len(),
        cue: MixCue {
            id: MixCueId(ids()),
            at,
            snapshot,
            scope,
        },
    }))
}

#[cfg(test)]
#[path = "ripple_cues_tests.rs"]
mod tests;
